// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `EXEC RUST` codegen — emit `src/exec_rust_blocks.rs` into the crate `cargo`
//! already builds (spec 041 T8).
//!
//! # Why there is no separate compilation step
//!
//! A built PowerRustCOBOL application **is** a generated Rust crate: `build_core`
//! writes a `Cargo.toml` and a `main.rs` and runs `cargo build --release`. So
//! compiling `EXEC RUST` is not "add a Rust compiler" — it is one more generated
//! module in that crate. No `rustc` shelling, no `cdylib`, no `dlopen`, no
//! `extern "C"`.
//!
//! # What is emitted
//!
//! ```text
//! banner + inner attributes
//! item-level blocks, verbatim, at module scope   ← types/impls/traits (R19, R20)
//! one `pub fn block_<id>` per statement-level block
//! `pub fn register(&mut ExecRustRegistry)`       ← the dispatch table (R2)
//! ```
//!
//! Item-level blocks come first and unindented so a `struct` a developer writes
//! is a genuine module-scope item, visible to every block in the program and
//! namable by a `REPOSITORY` `CLASS` (R22).
//!
//! # The shape of a generated block function
//!
//! Binding is **take-and-put-back**, not borrowing: a block may bind several
//! objects at once, and handing out two `&mut` from one `HashMap` is not
//! expressible. The order is deliberate and each step exists for a reason:
//!
//! 1. resolve every handle id from the environment;
//! 2. **check every binding before moving anything** — otherwise a block binding
//!    two objects could take the first, fail on the second, and leave the first
//!    slot empty;
//! 3. take the values;
//! 4. run the developer's code inside `catch_unwind`;
//! 5. put every value back **on both paths**, then re-raise the panic.
//!
//! Containment lives *inside* the generated function for step 5's sake: with
//! `catch_unwind` only at the call site, a panic would unwind past the taken
//! values and drop them, leaving live COBOL handles pointing at nothing. The
//! re-raise means the runtime's own containment still turns it into a
//! `RustPanic`, so `CATCH RUST-EXCEPTION` is unaffected.

use cobolt_ast::{
    program::Program,
    rust_types::block_binding,
    stmt::Stmt,
};
use cobolt_lexer::Span;
use cobolt_semantic::{exec_rust::collect_bindings, symbol_table::SymbolTable};

/// The generated module, plus what T10 needs to talk about it in the
/// developer's terms.
#[derive(Debug, Default)]
pub struct GeneratedBlocks {
    /// Full text of `src/exec_rust_blocks.rs`.
    pub source: String,
    /// Statement-level blocks emitted (one function each).
    pub block_count: usize,
    /// Item-level blocks emitted at module scope.
    pub item_count: usize,
    /// Where each generated line came from, for translating `rustc` diagnostics
    /// back to the developer's source (spec 041 R10).
    pub line_map: Vec<LineOrigin>,
}

/// One generated line that came from a developer's `EXEC RUST` block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineOrigin {
    /// 1-based line in the generated `exec_rust_blocks.rs`.
    pub generated_line: u32,
    /// 1-based line in the developer's COBOL source.
    pub cobol_line: u32,
    /// Bytes to add to a generated column to get the COBOL column.
    ///
    /// Zero for every line but the first of a block: the lexer trims the
    /// captured source, so only the first line lost its indentation.
    pub col_offset: u32,
}

impl GeneratedBlocks {
    /// The developer's `(line, column)` for a position in the generated file, or
    /// `None` when the line is generated scaffolding rather than their code.
    pub fn origin_of(&self, generated_line: u32, generated_col: u32) -> Option<(u32, u32)> {
        self.line_map
            .iter()
            .find(|o| o.generated_line == generated_line)
            .map(|o| (o.cobol_line, generated_col + o.col_offset))
    }
}

/// Emit the module for `program`.
///
/// `cobol_source` is the text the program was parsed from; it is used only to
/// locate each block's first line exactly. Pass `""` and the mapping falls back
/// to the statement's own span, which is one line coarser.
pub fn generate(program: &Program, symbols: &SymbolTable, cobol_source: &str) -> GeneratedBlocks {
    let mut e = Emitter::default();

    e.line(
        "// ───────────────────────────────────────────────────────────────────",
    );
    e.line("//  This code was generated automatically by PowerRustCOBOL RAD.");
    e.line("//");
    e.line("//  DO NOT MODIFY IT DIRECTLY: it is regenerated on every build, so");
    e.line("//  manual edits are lost. Edit the EXEC RUST blocks in your COBOL");
    e.line("//  source instead.");
    e.line("//");
    e.line("//  PowerRustCOBOL and its components are distributed under the");
    e.line("//  Apache 2.0 License.");
    e.line(
        "// ───────────────────────────────────────────────────────────────────",
    );
    e.line("");
    e.line("//! Compiled `EXEC RUST` blocks (spec 041).");
    e.line("");
    // A developer's block is their code, not ours: it must not be nagged at for
    // house style, and an unused binding is normal — an item may be bound
    // because it is read, or bound and left alone on one path.
    e.line("#![allow(unused_variables, unused_mut, unused_imports, dead_code)]");
    e.line("#![allow(non_snake_case, non_camel_case_types, clippy::all)]");
    e.line("");

    // ── Item-level blocks, verbatim at module scope (R19, R20) ───────────────
    let items = cobolt_semantic::exec_rust::item_blocks(program);
    let item_count = items.len();
    for item in items {
        e.line(&format!(
            "// ── item-level EXEC RUST block (COBOL line {}) ──",
            item.span.line
        ));
        e.verbatim(&item.source, block_start(cobol_source, item.span, &item.source));
        e.line("");
    }

    // ── One function per statement-level block ───────────────────────────────
    // Each block is emitted against the program that *contains* it: a form event
    // handler is a nested program with its own DATA DIVISION and REPOSITORY, and
    // resolving its names against the outer program would bind the wrong items.
    let mut blocks: Vec<(u32, String, Span, Vec<Binding>)> = Vec::new();
    cobolt_semantic::exec_rust::for_each_block(program, &mut |owner, stmt| {
        if let Stmt::ExecRust {
            source,
            block_id,
            span,
            ..
        } = stmt
        {
            let bindings = if std::ptr::eq(owner, program) {
                resolve_bindings(owner, program, symbols, None, source)
            } else {
                // A nested program sees its own items, plus the containing
                // program's `GLOBAL` ones — which is how a form's data reaches
                // its event handlers.
                let nested_symbols = SymbolTable::build(owner);
                resolve_bindings(owner, program, &nested_symbols, Some(symbols), source)
            };
            blocks.push((*block_id, source.clone(), *span, bindings));
        }
    });

    for (block_id, source, span, bindings) in &blocks {
        emit_block(&mut e, cobol_source, *block_id, source, *span, bindings);
    }

    // ── The dispatch table (R2) ──────────────────────────────────────────────
    e.line("/// Register every compiled block in this program.");
    e.line("///");
    e.line("/// Called by the generated `main.rs` before the interpreter runs; an");
    e.line("/// id with nothing registered against it is a hard runtime error, never");
    e.line("/// a silent no-op.");
    e.line("pub fn register(registry: &mut cobolt_runtime::exec_rust::ExecRustRegistry) {");
    for (block_id, _, _, _) in &blocks {
        e.line(&format!("    registry.register({block_id}, block_{block_id});"));
    }
    e.line("}");

    GeneratedBlocks {
        source: e.buf,
        block_count: blocks.len(),
        item_count,
        line_map: e.line_map,
    }
}

// ── Translating `rustc` diagnostics back to COBOL (spec 041 R10) ──────────────

/// The generated module's file name, as it appears in cargo's diagnostics.
pub const BLOCKS_FILE: &str = "exec_rust_blocks.rs";

/// A `rustc` diagnostic that belongs to a developer's `EXEC RUST` block, stated
/// in their coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDiagnostic {
    /// `"error"` or `"warning"` — cargo's own level.
    pub level: String,
    /// `rustc`'s message, unchanged. It talks about Rust, which is what the
    /// developer wrote; only the *location* was ever wrong.
    pub message: String,
    /// The error code, e.g. `"E0308"`, when `rustc` gave one.
    pub code: Option<String>,
    /// 1-based line in the developer's COBOL source.
    pub line: u32,
    /// 1-based column in the developer's COBOL source.
    pub col: u32,
}

/// Translate cargo's `--message-format=json` stream into diagnostics about the
/// developer's COBOL source.
///
/// Only diagnostics whose primary span is inside the generated blocks module
/// **and** on a line that came from the developer are returned. Everything else
/// — an error in `main.rs`, or in the scaffolding this codegen wrote — is
/// deliberately dropped: those are our bugs, not theirs, and reporting them
/// against a COBOL line would be a lie. The caller falls back to the raw cargo
/// output when nothing maps, so no failure is ever swallowed.
///
/// Lines that are not JSON objects are skipped rather than erroring: cargo
/// interleaves other artefact records in the same stream.
pub fn map_cargo_json(stdout: &str, blocks: &GeneratedBlocks) -> Vec<BlockDiagnostic> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("reason").and_then(|r| r.as_str()) != Some("compiler-message") {
            continue;
        }
        let Some(msg) = v.get("message") else { continue };
        let level = msg
            .get("level")
            .and_then(|l| l.as_str())
            .unwrap_or("error")
            .to_string();
        let text = msg
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or_default()
            .to_string();
        let code = msg
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(|c| c.as_str())
            .map(str::to_owned);

        let Some(spans) = msg.get("spans").and_then(|s| s.as_array()) else {
            continue;
        };
        // Prefer the primary span; fall back to any span in our file, because a
        // borrow error's primary span is sometimes the borrow, not the use.
        let candidates = spans
            .iter()
            .filter(|s| {
                s.get("file_name")
                    .and_then(|f| f.as_str())
                    .is_some_and(|f| f.ends_with(BLOCKS_FILE))
            })
            .map(|s| {
                (
                    s.get("is_primary").and_then(|p| p.as_bool()).unwrap_or(false),
                    s.get("line_start").and_then(|l| l.as_u64()).unwrap_or(0) as u32,
                    s.get("column_start").and_then(|c| c.as_u64()).unwrap_or(1) as u32,
                )
            });
        let mut best: Option<(u32, u32)> = None;
        for (is_primary, line_start, col_start) in candidates {
            let Some(origin) = blocks.origin_of(line_start, col_start) else {
                continue;
            };
            if is_primary {
                best = Some(origin);
                break;
            }
            best.get_or_insert(origin);
        }
        let Some((line, col)) = best else { continue };
        out.push(BlockDiagnostic {
            level,
            message: text,
            code,
            line,
            col,
        });
    }
    out
}

/// Emit one statement-level block as a function.
fn emit_block(
    e: &mut Emitter,
    cobol_source: &str,
    block_id: u32,
    source: &str,
    span: Span,
    bindings: &[Binding],
) {
    e.line(&format!(
        "/// `EXEC RUST` block {block_id}, from COBOL line {}.",
        span.line
    ));
    e.line(&format!(
        "pub fn block_{block_id}(ctx: &mut cobolt_runtime::exec_rust::ExecRustContext<'_>) {{"
    ));

    // 1. Handle ids.
    for b in bindings {
        e.line(&format!(
            "    let __cobolt_h_{rust} : i64 = ctx.env.get(\"{cobol}\").and_then(|v| v.as_i64()).unwrap_or(0);",
            rust = b.rust_name,
            cobol = b.cobol_name,
        ));
    }
    // 2. Check every binding before anything is moved.
    for b in bindings {
        e.line(&format!(
            "    if let Err(__cobolt_e) = ctx.bridge.check_binding::<{ty}>(__cobolt_h_{rust}, \"{cobol}\", \"{class}\") {{ panic!(\"{{}}\", __cobolt_e); }}",
            ty = b.rust_type,
            rust = b.rust_name,
            cobol = b.cobol_name,
            class = b.class_path,
        ));
    }
    // 3. Take them.
    for b in bindings {
        e.line(&format!(
            "    let mut {rust} : {ty} = ctx.bridge.take_or_init(__cobolt_h_{rust}, || {init});",
            rust = b.rust_name,
            ty = b.rust_type,
            init = b.init_expr,
        ));
    }

    // 4. The developer's code, contained.
    e.line("    let __cobolt_outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {");
    e.line("        let cobolt_objects = &mut *ctx.objects;");
    e.line("        let cobolt_env = &mut *ctx.env;");
    // A block body is a function body returning `Result`, which is what makes
    // `?` usable inside it (AC2). An error that reaches here becomes a panic,
    // and therefore a catchable `RUST-EXCEPTION` — the same door every other
    // failure in a block goes through.
    e.line("        let mut __cobolt_body = || -> std::result::Result<(), Box<dyn std::error::Error>> {");
    e.verbatim(source, block_start(cobol_source, span, source));
    e.line("            #[allow(unreachable_code)]");
    e.line("            Ok(())");
    e.line("        };");
    e.line("        if let Err(__cobolt_err) = __cobolt_body() {");
    e.line("            panic!(\"EXEC RUST block returned an error: {}\", __cobolt_err);");
    e.line("        }");
    e.line("    }));");

    // 5. Put back on both paths, then re-raise.
    for b in bindings {
        e.line(&format!(
            "    ctx.bridge.put_value(__cobolt_h_{rust}, \"{class}\", {rust});",
            rust = b.rust_name,
            class = b.class_path,
        ));
    }
    e.line("    if let Err(__cobolt_payload) = __cobolt_outcome {");
    e.line("        std::panic::resume_unwind(__cobolt_payload);");
    e.line("    }");
    e.line("}");
    e.line("");
}

/// One resolved binding: the COBOL item, the Rust variable it becomes, and the
/// type and initial value it binds at.
struct Binding {
    cobol_name: String,
    rust_name: String,
    /// The `REPOSITORY` path, e.g. `"Rust.String"`.
    class_path: String,
    /// Rust source text for the bound variable's type.
    rust_type: String,
    /// Rust source text for its value when the handle is not yet initialised.
    init_expr: String,
}

/// Work out what each name the block mentions binds to.
///
/// `owner` is the program the block sits in and `root` the outermost one: a
/// nested program (a form event handler) usually declares no `REPOSITORY` of its
/// own and relies on the outer one, so the class is looked up in the owner
/// first and the root second.
///
/// Silently skips an item that is not a `USAGE OBJECT REFERENCE` or whose class
/// is not in either `REPOSITORY`: semantic analysis has already rejected those
/// (R5, revised R8) and the build stops before codegen, so anything reaching
/// here is bindable.
fn resolve_bindings(
    owner: &Program,
    root: &Program,
    symbols: &SymbolTable,
    globals_from: Option<&SymbolTable>,
    source: &str,
) -> Vec<Binding> {
    // The owner's own items first; then, for a nested program, the containing
    // program's GLOBAL items. Non-GLOBAL outer items are deliberately left out:
    // COBOL does not make them visible, and binding one anyway would compile
    // code the language says should not resolve.
    let mut candidates = collect_bindings(source, symbols)
        .into_iter()
        .map(|(c, r)| (c, r, false))
        .collect::<Vec<_>>();
    if let Some(outer) = globals_from {
        for (cobol_name, rust_name) in collect_bindings(source, outer) {
            if candidates.iter().any(|(c, _, _)| *c == cobol_name) {
                continue;
            }
            if outer.data_item(&cobol_name).is_some_and(|i| i.is_global) {
                candidates.push((cobol_name, rust_name, true));
            }
        }
    }

    let mut out = Vec::new();
    for (cobol_name, rust_name, from_outer) in candidates {
        let table = if from_outer {
            globals_from.unwrap_or(symbols)
        } else {
            symbols
        };
        let Some(info) = table.data_item(&cobol_name) else {
            continue;
        };
        let Some(class) = &info.object_class else {
            continue;
        };
        let Some((_, path)) = owner
            .repository
            .iter()
            .chain(root.repository.iter())
            .find(|(k, _)| k.eq_ignore_ascii_case(class))
        else {
            continue;
        };
        // A shipped class binds at the type the bridge stores; a developer's
        // own type binds at the bare name their item-level block declared, from
        // `Default` (spec 041 R22).
        let (rust_type, init_expr) = match block_binding(path) {
            Some((ty, init)) => (ty.to_string(), init.to_string()),
            None => match cobolt_ast::rust_types::rust_type_name(path) {
                Some(name) => (name.to_string(), "Default::default()".to_string()),
                None => continue,
            },
        };
        out.push(Binding {
            cobol_name,
            rust_name,
            class_path: path.clone(),
            rust_type,
            init_expr,
        });
    }
    out
}

// ── Emitting with provenance ──────────────────────────────────────────────────

/// Accumulates the generated text while recording where each of the
/// developer's lines ended up.
#[derive(Default)]
struct Emitter {
    buf: String,
    /// 1-based line number of the next line to be written.
    next_line: u32,
    line_map: Vec<LineOrigin>,
}

impl Emitter {
    fn line(&mut self, text: &str) {
        if self.next_line == 0 {
            self.next_line = 1;
        }
        self.buf.push_str(text);
        self.buf.push('\n');
        self.next_line += 1;
    }

    /// Write developer source verbatim, recording each line's COBOL origin.
    ///
    /// Verbatim means **no added indentation**: a generated column is then the
    /// developer's column, so a `rustc` diagnostic maps across without arithmetic
    /// on every line but the first.
    fn verbatim(&mut self, source: &str, start: (u32, u32)) {
        let (start_line, start_col) = start;
        for (i, text) in source.lines().enumerate() {
            let generated_line = if self.next_line == 0 { 1 } else { self.next_line };
            self.line_map.push(LineOrigin {
                generated_line,
                cobol_line: start_line + i as u32,
                // Only the first line was trimmed of its indentation.
                col_offset: if i == 0 { start_col.saturating_sub(1) } else { 0 },
            });
            self.line(text);
        }
    }
}

/// The COBOL `(line, column)` where a block's captured source actually begins.
///
/// The lexer trims what it captures, so the source does not start at the
/// `EXEC` keyword the span points at. Locating the trimmed text inside the
/// statement's own slice gives the exact position; when that is not possible
/// — no source text to hand, or a preprocessed form whose offsets no longer
/// line up — it falls back to "the line after `EXEC RUST`, column 1", which is
/// right for the ordinary layout and never worse than a guess at the span.
fn block_start(cobol_source: &str, span: Span, source: &str) -> (u32, u32) {
    let fallback = (span.line.saturating_add(1), 1);
    if cobol_source.is_empty() || source.is_empty() {
        return fallback;
    }
    let Some(slice) = cobol_source.get(span.start..span.end) else {
        return fallback;
    };
    // Skip the `EXEC` keyword so a one-character block cannot match inside it.
    let from = slice.len().min(4);
    let Some(rel) = slice[from..].find(source) else {
        return fallback;
    };
    let prefix = &slice[..from + rel];
    let newlines = prefix.matches('\n').count() as u32;
    let col = match prefix.rfind('\n') {
        Some(nl) => (prefix.len() - nl) as u32,
        None => span.col + prefix.len() as u32,
    };
    (span.line + newlines, col)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_lexer::{tokenize, SourceFormat};
    use cobolt_parser::parse;

    fn compile(src: &str) -> (GeneratedBlocks, String) {
        let parsed = parse(tokenize(src, SourceFormat::Free));
        assert!(
            parsed
                .diagnostics
                .iter()
                .all(|d| d.severity != cobolt_parser::Severity::Error),
            "the fixture should parse cleanly: {:?}",
            parsed
                .diagnostics
                .iter()
                .filter(|d| d.severity == cobolt_parser::Severity::Error)
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );
        let program = parsed.program.expect("the fixture should parse");
        let symbols = SymbolTable::build(&program);
        (generate(&program, &symbols, src), src.to_string())
    }

    const WITH_ITEM_BLOCK: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. DEMO.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
REPOSITORY.
    CLASS RUST-STRING IS "Rust.String"
    CLASS MY-POINT IS "Rust.Point".
EXEC RUST
#[derive(Default)]
pub struct Point { pub x: i64, pub y: i64 }
impl Point { pub fn shift(&mut self, d: i64) { self.x += d; } }
END-EXEC.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-TEXT USAGE OBJECT REFERENCE RUST-STRING.
01 WS-ORIGIN USAGE OBJECT REFERENCE MY-POINT.
PROCEDURE DIVISION.
MAIN-PARA.
    EXEC RUST
    ws_text.push_str("hello");
    ws_origin.shift(3);
    END-EXEC.
    STOP RUN.
"#;

    /// **T8's verification.** Item-level blocks are at module scope and the
    /// block functions come below them — the order that makes a developer's
    /// `struct` visible to every block (R19, R20).
    #[test]
    fn items_are_at_module_scope_and_functions_below_them() {
        let (gen, _) = compile(WITH_ITEM_BLOCK);
        assert_eq!(gen.item_count, 1, "the item-level block should be emitted");
        assert_eq!(gen.block_count, 1, "the statement-level block should be emitted");

        let struct_at = gen
            .source
            .find("pub struct Point")
            .expect("the developer's struct should be emitted verbatim");
        let fn_at = gen
            .source
            .find("pub fn block_0")
            .expect("the block function should be emitted");
        assert!(struct_at < fn_at, "items must precede the block functions");

        // Module scope means column 1 — not nested inside a function.
        let line = gen
            .source
            .lines()
            .find(|l| l.contains("pub struct Point"))
            .unwrap();
        assert!(
            !line.starts_with(' '),
            "the item was indented, so it is not at module scope: {line:?}"
        );
    }

    /// The preamble binds each referenced item at its real Rust type, checks
    /// every binding before taking any value, and puts them all back.
    #[test]
    fn bindings_are_checked_then_taken_then_put_back() {
        let (gen, _) = compile(WITH_ITEM_BLOCK);
        let src = &gen.source;

        assert!(
            src.contains("check_binding::<String>(__cobolt_h_ws_text, \"WS-TEXT\", \"Rust.String\")"),
            "a shipped class should bind at the type the bridge stores:\n{src}"
        );
        assert!(
            src.contains("let mut ws_text : String = ctx.bridge.take_or_init"),
            "the bound variable should be the developer's snake_case name:\n{src}"
        );
        assert!(
            src.contains("let mut ws_origin : Point = ctx.bridge.take_or_init")
                && src.contains("|| Default::default()"),
            "a developer-defined class binds at its own type, from Default:\n{src}"
        );
        assert!(
            src.contains("ctx.bridge.put_value(__cobolt_h_ws_text, \"Rust.String\", ws_text);"),
            "every taken value must be put back:\n{src}"
        );

        // Order: check before take before put-back.
        let check = src.find("check_binding::<String>").unwrap();
        let take = src.find("let mut ws_text : String").unwrap();
        let put = src.find("put_value(__cobolt_h_ws_text").unwrap();
        assert!(check < take && take < put, "the preamble is out of order");
    }

    /// Containment is inside the generated function, with the put-backs after
    /// it and a re-raise last, so a panicking block cannot strand a handle.
    #[test]
    fn a_panicking_block_still_puts_its_values_back() {
        let (gen, _) = compile(WITH_ITEM_BLOCK);
        let src = &gen.source;
        let catch = src.find("catch_unwind").expect("containment is emitted");
        let put = src.find("put_value(__cobolt_h_ws_text").unwrap();
        let resume = src.find("resume_unwind").expect("the panic is re-raised");
        assert!(
            catch < put && put < resume,
            "put-backs must sit between containment and the re-raise"
        );
    }

    /// Every registered id is a function that exists.
    #[test]
    fn the_dispatch_table_names_emitted_functions() {
        let (gen, _) = compile(WITH_ITEM_BLOCK);
        assert!(gen.source.contains("registry.register(0, block_0);"));
        assert!(gen.source.contains("pub fn block_0("));
    }

    /// A `GLOBAL` item — how a form's data reaches its event handlers — still
    /// binds, and a block in a nested program binds against it too.
    #[test]
    fn a_global_object_reference_binds_in_both_programs() {
        let (gen, _) = compile(
            r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. OUTER.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
REPOSITORY.
    CLASS RUST-STRING IS "Rust.String".
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-TEXT USAGE IS OBJECT REFERENCE RUST-STRING GLOBAL.
PROCEDURE DIVISION.
    EXEC RUST
    ws_text.push_str("outer");
    END-EXEC.
    STOP RUN.

IDENTIFICATION DIVISION.
PROGRAM-ID. HANDLER.
PROCEDURE DIVISION.
    EXEC RUST
    ws_text.push_str("handler");
    END-EXEC.
    GOBACK.
END PROGRAM HANDLER.
END PROGRAM OUTER.
"#,
        );
        assert_eq!(gen.block_count, 2, "both blocks should be emitted");
        let takes = gen
            .source
            .matches("let mut ws_text : String")
            .count();
        assert_eq!(
            takes, 2,
            "the GLOBAL item should bind in the outer program and in the nested one:\n{}",
            gen.source
        );
    }

    /// A program with no blocks still yields a module that compiles and
    /// registers nothing — `main.rs` refers to it unconditionally.
    #[test]
    fn a_program_without_blocks_yields_an_empty_table() {
        let (gen, _) = compile(
            r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. PLAIN.
PROCEDURE DIVISION.
MAIN-PARA.
    STOP RUN.
"#,
        );
        assert_eq!(gen.block_count, 0);
        assert_eq!(gen.item_count, 0);
        assert!(gen.source.contains("pub fn register("));
        assert!(!gen.source.contains("registry.register("));
    }

    /// The line map is what T10 translates diagnostics through: a generated
    /// line carrying the developer's code reports the developer's COBOL line.
    #[test]
    fn generated_lines_map_back_to_cobol_lines() {
        let (gen, src) = compile(WITH_ITEM_BLOCK);

        // The COBOL line holding `ws_text.push_str` — counted from the fixture.
        let cobol_line = src
            .lines()
            .position(|l| l.contains("ws_text.push_str"))
            .expect("the fixture has that line") as u32
            + 1;
        let generated_line = gen
            .source
            .lines()
            .position(|l| l.contains("ws_text.push_str"))
            .expect("it was emitted") as u32
            + 1;

        let (mapped_line, _col) = gen
            .origin_of(generated_line, 1)
            .expect("a developer line must have an origin");
        assert_eq!(
            mapped_line, cobol_line,
            "generated line {generated_line} should map to COBOL line {cobol_line}"
        );

        // Scaffolding has no origin — a diagnostic there is ours, not theirs.
        let register_line = gen
            .source
            .lines()
            .position(|l| l.contains("pub fn register("))
            .unwrap() as u32
            + 1;
        assert_eq!(gen.origin_of(register_line, 1), None);
    }
}
