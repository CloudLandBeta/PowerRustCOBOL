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

// ── Block-owned windows ──────────────────────────────────────────────────────

/// The `cobolt_windows` module emitted alongside a program's blocks.
///
/// Emitted when the program has forms or any block — exactly when the generated
/// manifest links the GUI crates this module names.
///
/// Why a registry and not an `egui::Context` handed to the block: `Context` is
/// `Send + Sync` and would travel to the interpreter's worker thread perfectly
/// well, but `show_viewport_deferred` has to be called **on the UI thread, every
/// frame the window should exist** — egui marks the viewport used for the
/// current pass and drops it otherwise, and when viewports are embedded the
/// callback runs immediately, inside a frame. A block runs once, off the main
/// thread, so it cannot satisfy either condition. It registers what to draw
/// instead, and the form application replays the registration each frame.
const COBOLT_WINDOWS_MODULE: &str = r##"
/// Windows opened by an `EXEC RUST` block.
///
/// A block registers what to draw; the form application paints it every frame.
/// The drawing closure runs on the UI thread, so share state with the block
/// through an `Arc<Mutex<..>>`, as egui's own viewport documentation prescribes.
pub mod cobolt_windows {
    use eframe::egui;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};

    type UiFn = Arc<dyn Fn(&mut egui::Ui, egui::ViewportClass) + Send + Sync>;

    struct Entry {
        id: String,
        builder: egui::ViewportBuilder,
        ui: UiFn,
        open: Arc<AtomicBool>,
    }

    static REGISTRY: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();
    static PAINTER_READY: AtomicBool = AtomicBool::new(false);

    fn registry() -> &'static Mutex<Vec<Entry>> {
        REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
    }

    /// Called by the generated form application before the interpreter starts.
    pub fn set_painter_ready() {
        PAINTER_READY.store(true, Ordering::SeqCst);
    }

    /// A window opened from a block.
    pub struct Handle {
        open: Arc<AtomicBool>,
    }

    impl Handle {
        /// `true` until the operator closes the window, or `close` is called.
        pub fn is_open(&self) -> bool {
            self.open.load(Ordering::SeqCst)
        }

        /// Close the window.
        pub fn close(&self) {
            self.open.store(false, Ordering::SeqCst);
        }

        /// Wait for the window to close, then return.
        ///
        /// Safe to call from a block: the interpreter has a thread of its own,
        /// so the form keeps painting and stays responsive while this waits.
        pub fn wait(&self) {
            while self.open.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(16));
            }
        }
    }

    /// Open a window, drawn by `ui` on the UI thread for as long as it lives.
    ///
    /// Opening an `id` that is already open replaces what that window draws.
    ///
    /// Panics — and so raises a catchable `RUST-EXCEPTION` — in a program with
    /// no form, where nothing exists to paint it.
    pub fn open(
        id: &str,
        builder: egui::ViewportBuilder,
        ui: impl Fn(&mut egui::Ui, egui::ViewportClass) + Send + Sync + 'static,
    ) -> Handle {
        assert!(
            PAINTER_READY.load(Ordering::SeqCst),
            "cobolt_windows::open needs a form to paint the window, and this \
             program has none. In a console program call eframe::run_native \
             instead, which owns the main thread there."
        );
        let open = Arc::new(AtomicBool::new(true));
        let mut reg = registry().lock().unwrap();
        reg.retain(|e| e.id != id);
        reg.push(Entry {
            id: id.to_owned(),
            builder,
            ui: Arc::new(ui),
            open: Arc::clone(&open),
        });
        Handle { open }
    }

    /// `true` while a window with this id is open.
    pub fn is_open(id: &str) -> bool {
        registry()
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.id == id && e.open.load(Ordering::SeqCst))
    }

    /// Close a window by id. Does nothing if no such window is open.
    pub fn close(id: &str) {
        for e in registry().lock().unwrap().iter() {
            if e.id == id {
                e.open.store(false, Ordering::SeqCst);
            }
        }
    }

    /// Paint every open window. Called once per frame by the form application.
    pub fn show_all(ctx: &egui::Context) {
        // Copied out before painting so that a block reached from the UI thread
        // can open or close a window without deadlocking on the registry.
        let live: Vec<(String, egui::ViewportBuilder, UiFn, Arc<AtomicBool>)> = {
            let reg = registry().lock().unwrap();
            reg.iter()
                .filter(|e| e.open.load(Ordering::SeqCst))
                .map(|e| {
                    (
                        e.id.clone(),
                        e.builder.clone(),
                        Arc::clone(&e.ui),
                        Arc::clone(&e.open),
                    )
                })
                .collect()
        };
        for (id, builder, ui, open) in live {
            ctx.show_viewport_deferred(
                egui::ViewportId::from_hash_of(&id),
                builder,
                move |u, class| {
                    if u.ctx().input(|i| i.viewport().close_requested()) {
                        open.store(false, Ordering::SeqCst);
                    }
                    ui(u, class);
                },
            );
        }
        registry()
            .lock()
            .unwrap()
            .retain(|e| e.open.load(Ordering::SeqCst));
    }
}
"##;

// ── Event-loop calls in a form application (build-time rejection) ────────────

/// A call in a developer's block that would build a second winit event loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventLoopCall {
    /// 1-based line in the developer's COBOL source.
    pub cobol_line: u32,
    /// 1-based column in the developer's COBOL source.
    pub cobol_col: u32,
    /// The call as written, e.g. `run_native`.
    pub call: String,
}

/// Calls that build a winit event loop, across every `EXEC RUST` block.
///
/// A process gets exactly one event loop. A form application builds it on the
/// main thread before the interpreter starts, and the interpreter — so every
/// block in an event handler — runs on a worker thread. winit's
/// `EVENT_LOOP_CREATED` guard is process-global and is checked ahead of any
/// platform code, so a second call returns `Err(EventLoopError::RecreationAttempt)`.
/// It does **not** panic, which is what makes this worth rejecting at build
/// time: `CATCH RUST-EXCEPTION` never fires, the customary `let _ =` throws the
/// `Err` away, and the developer is left with a window that never opens and no
/// message anywhere. Console programs are unaffected — there the interpreter
/// owns the main thread and the call is the normal way to open a window.
pub fn find_event_loop_calls(program: &Program, cobol_source: &str) -> Vec<EventLoopCall> {
    // Matched only where followed by `(`, so prose in a comment does not fail a
    // build.
    const CALLS: [&str; 2] = ["run_native", "EventLoop::new"];

    let mut found = Vec::new();
    let mut scan = |source: &str, span: Span| {
        let (base_line, base_col) = block_start(cobol_source, span, source);
        for call in CALLS {
            for (at, _) in source.match_indices(call) {
                let rest = source[at + call.len()..].trim_start();
                if !rest.starts_with('(') {
                    continue;
                }
                let prefix = &source[..at];
                let newlines = prefix.matches('\n').count() as u32;
                let col = match prefix.rfind('\n') {
                    Some(nl) => (prefix.len() - nl) as u32,
                    None => base_col + prefix.len() as u32,
                };
                found.push(EventLoopCall {
                    cobol_line: base_line + newlines,
                    cobol_col: col,
                    call: call.to_owned(),
                });
            }
        }
    };

    for item in cobolt_semantic::exec_rust::item_blocks(program) {
        scan(&item.source, item.span);
    }
    cobolt_semantic::exec_rust::for_each_block(program, &mut |_owner, stmt| {
        if let Stmt::ExecRust { source, span, .. } = stmt {
            scan(source, *span);
        }
    });

    found.sort_by_key(|c| (c.cobol_line, c.cobol_col));
    found
}

/// Emit the module for `program`.
///
/// `cobol_source` is the text the program was parsed from; it is used only to
/// locate each block's first line exactly. Pass `""` and the mapping falls back
/// to the statement's own span, which is one line coarser.
///
/// `has_forms` decides, together with the block count, whether the
/// `cobolt_windows` module is emitted: it names `eframe` and `egui`, and the
/// generated manifest links those exactly when the program has forms or any
/// block. A form application references the module unconditionally, so it must
/// be there even for a form with no blocks at all.
pub fn generate(
    program: &Program,
    symbols: &SymbolTable,
    cobol_source: &str,
    has_forms: bool,
) -> GeneratedBlocks {
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

    // ── Block-owned windows ──────────────────────────────────────────────────
    // Emitted exactly when the generated manifest links the GUI crates this
    // module names — forms, or any block.
    if has_forms || item_count > 0 || !blocks.is_empty() {
        for l in COBOLT_WINDOWS_MODULE.lines() {
            e.line(l);
        }
        e.line("");
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
/// Fold every error-level block diagnostic into one deterministic report:
/// `(line, col, message)` of the FIRST error by source position, with any
/// further errors appended to the message.
///
/// The build used to surface whichever error cargo's JSON stream mentioned
/// first — and with parallel rustc that order changes run to run, so two
/// builds of identical source showed *different* single errors. To the
/// developer that read as new bugs appearing on every rebuild. Sorting by the
/// developer's own (line, column) makes rebuilds repeat themselves, and
/// appending the rest stops the one-at-a-time whack-a-mole.
pub fn block_errors_report(diags: Vec<BlockDiagnostic>) -> Option<(u32, u32, String)> {
    let mut errors: Vec<&BlockDiagnostic> = diags.iter().filter(|d| d.level == "error").collect();
    if errors.is_empty() {
        return None;
    }
    errors.sort_by(|a, b| (a.line, a.col, &a.message).cmp(&(b.line, b.col, &b.message)));
    errors.dedup_by(|a, b| a.line == b.line && a.col == b.col && a.message == b.message);

    let render = |d: &BlockDiagnostic| match &d.code {
        Some(code) => format!("{} [{code}]", d.message),
        None => d.message.clone(),
    };
    let first = errors[0];
    let mut message = render(first);
    if errors.len() > 1 {
        message.push_str(&format!(
            "\n\n─ {} more error(s) in EXEC RUST blocks ─",
            errors.len() - 1
        ));
        for d in &errors[1..] {
            message.push_str(&format!("\nline {}, column {}: {}", d.line, d.col, render(d)));
        }
    }
    Some((first.line, first.col, message))
}

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
    // 3. Take them into locals the generated function owns.
    for b in bindings {
        e.line(&format!(
            "    let mut __cobolt_v_{rust} : {ty} = ctx.bridge.take_or_init(__cobolt_h_{rust}, || {init});",
            rust = b.rust_name,
            ty = b.rust_type,
            init = b.init_expr,
        ));
    }

    // 4. The developer's code, contained.
    e.line("    let __cobolt_outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {");
    e.line("        let cobolt_objects = &mut *ctx.objects;");
    e.line("        let cobolt_env = &mut *ctx.env;");
    // The developer's name is a `&mut T`, not a `T`. That is the convention
    // this language already had — `*ws_count += 1;` — and it is what lets a
    // block assign through the name (`*clicked_button = ask(...)`) as well as
    // call methods on it (`window_title.as_str()`), which auto-deref handles.
    // The owned value stays in the enclosing function so it can be put back
    // even when the body panics.
    for b in bindings {
        e.line(&format!(
            "        let {rust} = &mut __cobolt_v_{rust};",
            rust = b.rust_name
        ));
    }
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
            "    ctx.bridge.put_value(__cobolt_h_{rust}, \"{class}\", __cobolt_v_{rust});",
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
        (generate(&program, &symbols, src, false), src.to_string())
    }

    /// `run_native` is found in both kinds of block, at the developer's own
    /// line, and prose that merely names it does not fail a build.
    #[test]
    fn event_loop_calls_are_found_at_the_developers_line() {
        const SRC: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. DEMO.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
REPOSITORY.
    CLASS RUST-STRING IS "Rust.String".
EXEC RUST
pub fn ask() -> i64 {
    // never call run_native from a handler
    let _ = eframe::run_native("t", Default::default(), Box::new(|_| unimplemented!()));
    0
}
END-EXEC.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC 9(4) VALUE 0.
PROCEDURE DIVISION.
MAIN.
    EXEC RUST
    let _ = EventLoop::new();
    END-EXEC.
    STOP RUN.
"#;
        let program = parse(tokenize(SRC, SourceFormat::Free))
            .program
            .expect("the fixture should parse");
        let found = find_event_loop_calls(&program, SRC);

        assert_eq!(
            found.len(),
            2,
            "the comment must not count, the two calls must: {found:?}"
        );
        assert_eq!(found[0].call, "run_native");
        assert_eq!(found[1].call, "EventLoop::new");

        // The lines reported are the developer's, not the generated file's.
        let line_of = |needle: &str| {
            SRC.lines()
                .position(|l| l.contains(needle) && !l.trim_start().starts_with("//"))
                .map(|i| i as u32 + 1)
                .unwrap()
        };
        assert_eq!(found[0].cobol_line, line_of("eframe::run_native"));
        assert_eq!(found[1].cobol_line, line_of("EventLoop::new"));
    }

    /// `cobolt_windows` is emitted exactly when the GUI crates are linked:
    /// with forms, or with any block. A console program with neither must not
    /// get it — nothing would be there for it to name.
    #[test]
    fn the_windows_module_follows_the_gui_crates() {
        const NO_BLOCKS: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. PLAIN.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC 9(4) VALUE 0.
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
"#;
        let program = parse(tokenize(NO_BLOCKS, SourceFormat::Free))
            .program
            .expect("the fixture should parse");
        let symbols = SymbolTable::build(&program);

        let console = generate(&program, &symbols, NO_BLOCKS, false);
        assert!(
            !console.source.contains("pub mod cobolt_windows"),
            "a console program with no block does not link egui, so the module \
             must not be emitted"
        );

        let with_form = generate(&program, &symbols, NO_BLOCKS, true);
        assert!(
            with_form.source.contains("pub mod cobolt_windows"),
            "a form application references the module every frame, so it must be \
             emitted even when the program has no blocks at all"
        );

        // And a console program that *does* have a block links the GUI crates.
        let (blocks, _) = compile(WITH_ITEM_BLOCK);
        assert!(
            blocks.source.contains("pub mod cobolt_windows"),
            "any block links egui, so the module comes with it"
        );
    }

    /// The reported error must not depend on cargo's parallel emission order.
    ///
    /// Two builds of identical source used to show *different* single errors,
    /// which read as new bugs appearing on every rebuild.
    #[test]
    fn block_errors_are_deterministic_and_complete() {
        let d = |line: u32, msg: &str| BlockDiagnostic {
            level: "error".to_owned(),
            message: msg.to_owned(),
            code: Some("E0597".to_owned()),
            line,
            col: 5,
        };
        let a = vec![d(129, "`clicked` does not live long enough"), d(90, "first by position")];
        let b = vec![d(90, "first by position"), d(129, "`clicked` does not live long enough")];

        let ra = block_errors_report(a).expect("errors present");
        let rb = block_errors_report(b).expect("errors present");
        assert_eq!(ra, rb, "emission order must not change the report");
        assert_eq!(ra.0, 90, "the headline is the first error by source line");
        assert!(
            ra.2.contains("does not live long enough"),
            "every error appears: {}",
            ra.2
        );

        // Warnings alone produce no error report.
        let warn = BlockDiagnostic {
            level: "warning".to_owned(),
            message: "unused".to_owned(),
            code: None,
            line: 1,
            col: 1,
        };
        assert!(block_errors_report(vec![warn]).is_none());
    }

    /// A program with no blocks, and one whose block opens no window, are clean.
    #[test]
    fn a_block_without_an_event_loop_is_not_flagged() {
        let (_, src) = compile(WITH_ITEM_BLOCK);
        let program = parse(tokenize(&src, SourceFormat::Free)).program.unwrap();
        assert!(find_event_loop_calls(&program, &src).is_empty());
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
            src.contains("let mut __cobolt_v_ws_text : String = ctx.bridge.take_or_init"),
            "the owned value lives in the generated function:\n{src}"
        );
        assert!(
            src.contains("let mut __cobolt_v_ws_origin : Point = ctx.bridge.take_or_init")
                && src.contains("|| Default::default()"),
            "a developer-defined class binds at its own type, from Default:\n{src}"
        );
        // What the developer's own code sees is a `&mut T` under their name —
        // the convention this language already had (`*ws_count += 1;`), and
        // what lets a block assign through the name as well as call methods on
        // it.
        assert!(
            src.contains("let ws_text = &mut __cobolt_v_ws_text;"),
            "the developer's name must be a mutable reference:\n{src}"
        );
        assert!(
            src.contains(
                "ctx.bridge.put_value(__cobolt_h_ws_text, \"Rust.String\", __cobolt_v_ws_text);"
            ),
            "every taken value must be put back:\n{src}"
        );

        // Order: check before take before put-back.
        let check = src.find("check_binding::<String>").unwrap();
        let take = src.find("let mut __cobolt_v_ws_text : String").unwrap();
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
            .matches("let mut __cobolt_v_ws_text : String")
            .count();
        assert_eq!(
            takes, 2,
            "the GLOBAL item should bind in the outer program and in the nested one:\n{}",
            gen.source
        );
    }

    /// **A block inside `TRY … END-TRY` must be compiled.**
    ///
    /// This is the one place the guide tells a developer to put a block — it is
    /// how `CATCH RUST-EXCEPTION` is used — and the statement walker's old
    /// `_ => {}` arm swallowed `TryCatch` whole. The block compiled into
    /// nothing and the program failed at run time with "no compiled function",
    /// which reads like a build problem and is not one.
    ///
    /// The other conditional phrases are here for the same reason: each was a
    /// separate silent hole.
    #[test]
    fn blocks_inside_conditional_phrases_are_compiled() {
        let (gen, _) = compile(
            r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. NESTED.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC 9(4).
01 WS-E PIC X(80).
PROCEDURE DIVISION.
MAIN-PARA.
    TRY
        EXEC RUST
        println!("in try");
        END-EXEC
    CATCH RUST-EXCEPTION WS-E
        EXEC RUST
        println!("in rust catch");
        END-EXEC
    FINALLY
        EXEC RUST
        println!("in finally");
        END-EXEC
    END-TRY.
    COMPUTE WS-N = 1 / 1
        ON SIZE ERROR
            EXEC RUST
            println!("in size error");
            END-EXEC
    END-COMPUTE.
    IF WS-N = 1
        EXEC RUST
        println!("in if");
        END-EXEC
    END-IF.
    STOP RUN.
"#,
        );

        assert_eq!(
            gen.block_count, 5,
            "every nested block must compile — TRY, CATCH RUST-EXCEPTION, \
             FINALLY, ON SIZE ERROR and IF:\n{}",
            gen.source
        );
        for needle in [
            "in try",
            "in rust catch",
            "in finally",
            "in size error",
            "in if",
        ] {
            assert!(
                gen.source.contains(needle),
                "the block containing {needle:?} was dropped"
            );
        }
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
