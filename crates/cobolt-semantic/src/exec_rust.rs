// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! EXEC RUST binding resolution.
//!
//! For each `Stmt::ExecRust` block in the program this pass:
//!
//! 1. Scans the raw Rust source string for snake_case identifiers.
//! 2. Checks each identifier against the symbol table's data items
//!    (comparing the identifier to `DataItemInfo::rust_name`).
//! 3. Emits the resolved bindings as `(cobol_name, rust_name)` pairs and
//!    reports them in the diagnostics at `Info` level.
//!
//! **Important**: this pass only *reads* the AST — it does not mutate
//! `Stmt::ExecRust::referenced_data`.  Callers that need the binding list
//! should consult [`collect_bindings`] directly or use the diagnostics.
//! The runtime crate drives the actual injection of bindings into AST nodes
//! when it lowers to executable code.

use cobolt_ast::{
    data::Usage,
    program::{ProcedureBody, Program},
    rust_types::{is_shipped_rust_type, rust_type_name},
    stmt::Stmt,
};

use crate::{
    symbol_table::{DataItemInfo, SymbolTable},
    SemanticDiagnostic, Severity,
};

// ── Public API ────────────────────────────────────────────────────────────────

/// Walk every `Stmt::ExecRust` in the program and emit `Info` diagnostics
/// listing the COBOL data items referenced by each block.
///
/// Any snake_case word in the Rust source that corresponds to a known data
/// item is considered "referenced".
pub fn resolve_bindings(
    program: &Program,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    walk_stmts_in_program(program, &mut |stmt| {
        if let Stmt::ExecRust { source, span, .. } = stmt {
            // Spec 041 R5 — only `USAGE OBJECT REFERENCE RUST-<type>` items may
            // cross into a block. A PIC item has no Rust type to bind to: its
            // value is a scaled decimal or a fixed-width space-padded field, and
            // handing one to compiled Rust would mean re-implementing COBOL's
            // numeric and padding rules inside generated code. Move it through
            // an object with `INVOKE` / `::` instead, outside the block.
            // R16 — a crate the build does not link cannot compile, and saying
            // so here names the crate the developer wrote rather than leaving
            // `rustc` to complain about generated code.
            for krate in unlinked_crates(source) {
                diagnostics.push(SemanticDiagnostic {
                    severity: Severity::Error,
                    message: format!(
                        "EXEC RUST uses crate `{krate}`, which this build does not \
                         link — arbitrary crates are not yet supported; std, egui \
                         and eframe are available"
                    ),
                    span: *span,
                });
            }

            for (cobol, _rust) in collect_bindings(source, symbols) {
                let Some(info) = symbols.data_items().find_map(|(_, i)| {
                    (i.cobol_name == cobol).then_some(i)
                }) else {
                    continue;
                };
                if info.usage != Usage::ObjectReference {
                    diagnostics.push(SemanticDiagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "EXEC RUST block references {cobol}, which is not a \
                             USAGE OBJECT REFERENCE item — only Rust-typed objects \
                             may cross into a block"
                        ),
                        span: *span,
                    });
                    continue;
                }
                // R6 — the bound name becomes a Rust identifier verbatim, so it
                // has to BE one. `cobol_to_rust` lowercases and swaps hyphens
                // for underscores, which is enough for ordinary COBOL names;
                // what it cannot fix is a name that lands on a Rust keyword
                // (`TYPE` → `type`) or does not start like an identifier
                // (`1ST-FLAG` → `1st_flag`).
                if let Some(reason) = rust_name_problem(&info.rust_name) {
                    diagnostics.push(SemanticDiagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "EXEC RUST cannot bind {cobol}: its Rust name \
                             `{}` {reason} — rename the data item",
                            info.rust_name
                        ),
                        span: *span,
                    });
                }
            }

            let bindings = collect_bindings(source, symbols);
            if !bindings.is_empty() {
                let names: Vec<_> = bindings
                    .iter()
                    .map(|(cobol, rust)| format!("{cobol} → {rust}"))
                    .collect();
                diagnostics.push(SemanticDiagnostic {
                    severity: Severity::Info,
                    message: format!(
                        "EXEC RUST block references COBOL data items: {}",
                        names.join(", ")
                    ),
                    span: *span,
                });
            }
        }
    });
}

/// Visit every statement-level `EXEC RUST` block in the program **and in its
/// nested programs**, in source order, together with the program that contains
/// each one.
///
/// Source order is the order the parser assigned block ids in, so codegen can
/// rely on it (spec 041 T8). Exposed here rather than duplicated in
/// `cobolt-compiler` because the walk must stay identical to the one binding
/// resolution uses — a block this misses is a block that compiles into nothing.
///
/// **Nested programs are not an edge case here.** A form event handler *is* a
/// nested program (`cobolt-forms::EventBinding::paragraph` names one), so a walk
/// that stopped at the top level would silently drop every block a developer
/// wrote in a handler — which is where form logic lives. The containing program
/// comes with each block because it owns the `REPOSITORY` and the DATA DIVISION
/// the block's names resolve against.
pub fn for_each_block(program: &Program, f: &mut impl FnMut(&Program, &Stmt)) {
    walk_stmts_in_program(program, &mut |stmt| {
        if matches!(stmt, Stmt::ExecRust { .. }) {
            f(program, stmt);
        }
    });
    for nested in &program.nested_programs {
        for_each_block(nested, f);
    }
}

/// Every item-level block in the program and its nested programs, in source
/// order (spec 041 R19).
pub fn item_blocks(program: &Program) -> Vec<&cobolt_ast::program::RustItemBlock> {
    let mut out: Vec<_> = program.rust_items.iter().collect();
    for nested in &program.nested_programs {
        out.extend(item_blocks(nested));
    }
    out
}

/// Collect all COBOL data items referenced by a single EXEC RUST source block.
///
/// Returns a `Vec` of `(cobol_name, rust_name)` pairs in symbol-table order.
///
/// # Algorithm
///
/// For each data item in the symbol table we check whether its `rust_name`
/// appears as a whole word (surrounded by non-alphanumeric-underscore
/// characters, or at the string boundaries) inside `source`.
/// This avoids false positives like `ws_count_extra` matching `ws_count`.
pub fn collect_bindings(source: &str, symbols: &SymbolTable) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (_, info) in symbols.data_items() {
        if contains_whole_word(source, &info.rust_name) {
            out.push((info.cobol_name.clone(), info.rust_name.clone()));
        }
    }
    out
}

/// Crates a block may draw on (spec 041 R16).
///
/// Not `std` alone: **every** generated program already links these, because
/// `generate_cargo_toml` emits them into the crate `cargo` builds. egui is the
/// standard GUI for PowerRustCOBOL — the IDE *and* the application — so a block
/// using it needs no vendoring and compiles today. What v1 defers is an
/// arbitrary third-party dependency, which needs a manifest story first.
const LINKED_CRATES: &[&str] = &[
    // Always available to any Rust code.
    "std",
    "core",
    "alloc",
    // Emitted into every generated Cargo.toml.
    "eframe",
    "egui",
    "egui_extras",
    "cobolt_forms",
    "cobolt_runtime",
    // `crate`/`self`/`super` are intra-crate paths, not dependencies.
    "crate",
    "self",
    "super",
];

/// Report any `use <crate>::…` in `source` naming a crate the build does not
/// link (spec 041 R16).
///
/// Only `use` declarations are inspected — the clear, greppable signal. A bare
/// `some_crate::f()` with no `use` would slip through and be caught by `rustc`
/// instead, with a worse message but a correct verdict.
fn unlinked_crates(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("use ") else {
            continue;
        };
        // `use egui::Context;` → `egui`; `use std::{fs, io};` → `std`
        let root: String = rest
            .trim_start_matches("::")
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if root.is_empty() || LINKED_CRATES.contains(&root.as_str()) {
            continue;
        }
        if !out.contains(&root) {
            out.push(root);
        }
    }
    out
}

/// Every `Rust.*` type an item-level block declares (spec 041 R19/R22).
///
/// A lexical scan for `struct` / `enum` / `type` / `union` followed by a name.
/// Deliberately **permissive**: it is not a Rust parser, and the cost of the
/// two error directions is not symmetric. Accepting a name that turns out not
/// to exist costs a precise `rustc` error later; rejecting one that does exist
/// would refuse a valid program with a wrong explanation. So when in doubt this
/// says yes.
fn types_declared_in_item_blocks(program: &Program) -> Vec<String> {
    const DECLARERS: [&str; 4] = ["struct", "enum", "type", "union"];
    let mut out = Vec::new();
    for block in &program.rust_items {
        let mut words = block.source.split_whitespace().peekable();
        while let Some(word) = words.next() {
            if !DECLARERS.contains(&word) {
                continue;
            }
            let Some(next) = words.peek() else { continue };
            // `struct Point {`, `struct Point<T>`, `struct Point;`
            let name: String = next
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                out.push(name);
            }
        }
    }
    out
}

/// Spec 041 R8 (revised) — a `REPOSITORY` `CLASS` must name a Rust type that
/// exists: one of the shipped set, or one an item-level block declares.
///
/// The shipped 48 are a **floor**, not a ceiling. Catching an unknown name here
/// means the diagnostic can point at the developer's `CLASS`; left to `rustc`
/// it would name the generated type instead, which is not the text they wrote.
pub fn check_repository_classes(program: &Program, diagnostics: &mut Vec<SemanticDiagnostic>) {
    let declared = types_declared_in_item_blocks(program);
    for (class_name, path) in &program.repository {
        // Only `Rust.*` is ours; another external hierarchy is left alone.
        let Some(type_name) = rust_type_name(path) else {
            continue;
        };
        if is_shipped_rust_type(path) || declared.iter().any(|d| d == type_name) {
            continue;
        }
        diagnostics.push(SemanticDiagnostic {
            severity: Severity::Error,
            message: format!(
                "CLASS {class_name} names \"{path}\", which is neither a shipped \
                 Rust type nor declared by an item-level EXEC RUST block"
            ),
            span: program.span,
        });
    }
}

/// Why `name` cannot be used as a Rust identifier, or `None` if it can
/// (spec 041 R6).
///
/// Deliberately narrow: `cobol_to_rust` already lowercases and replaces
/// hyphens, so an ordinary COBOL name arrives as valid `snake_case`. Only two
/// things survive that and still break — a Rust keyword, and a name that does
/// not start like an identifier.
fn rust_name_problem(name: &str) -> Option<&'static str> {
    /// Rust's reserved words, including the ones reserved for future use.
    /// COBOL programs use several of these as data names quite naturally
    /// (`TYPE`, `REF`, `BOX`, `MATCH`), so this is not a theoretical list.
    const KEYWORDS: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "union", "unsafe", "use", "where", "while", "abstract", "become", "box",
        "do", "final", "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
    ];

    if name.is_empty() {
        return Some("is empty");
    }
    if KEYWORDS.contains(&name) {
        return Some("is a Rust keyword");
    }
    let mut chars = name.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Some("does not start with a letter or underscore");
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Some("contains a character that is not a letter, digit or underscore");
    }
    None
}

/// Check whether `haystack` contains `needle` as a whole identifier word.
///
/// A "whole word" boundary is any character that is NOT `[A-Za-z0-9_]`.
fn contains_whole_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    let nlen = needle_bytes.len();

    let mut i = 0usize;
    while i + nlen <= bytes.len() {
        if bytes[i..i + nlen] == *needle_bytes {
            // Check left boundary
            let left_ok = i == 0 || !is_word_char(bytes[i - 1]);
            // Check right boundary
            let right_ok = i + nlen == bytes.len() || !is_word_char(bytes[i + nlen]);
            if left_ok && right_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

// ── AST walk helpers ──────────────────────────────────────────────────────────

fn walk_stmts_in_program(program: &Program, visitor: &mut impl FnMut(&Stmt)) {
    // Walk DATA DIVISION — no statements there, but keep structure symmetric.
    // Walk PROCEDURE DIVISION.
    match &program.procedure.body {
        ProcedureBody::Paragraphs(paras) => {
            for para in paras {
                for stmt in &para.stmts {
                    walk_stmt(stmt, visitor);
                }
            }
        }
        ProcedureBody::Sections(secs) => {
            for sec in secs {
                for para in &sec.paragraphs {
                    for stmt in &para.stmts {
                        walk_stmt(stmt, visitor);
                    }
                }
            }
        }
    }
}

/// Visit `stmt` and every statement nested inside it.
///
/// Delegates to [`Stmt::walk`], whose child list is an **exhaustive** match in
/// `cobolt-ast`. This function used to carry its own `match` with a `_ => {}`
/// arm, and that arm silently swallowed `TRY … END-TRY` — so a block written in
/// the one place the guide tells you to put it, inside a `TRY` with a
/// `CATCH RUST-EXCEPTION`, was never compiled and failed at run time as "no
/// compiled function". `ON SIZE ERROR`, `INVALID KEY`, `ON OVERFLOW`,
/// `SEARCH … WHEN` and `AT END` had the same hole.
fn walk_stmt(stmt: &Stmt, visitor: &mut impl FnMut(&Stmt)) {
    stmt.walk(visitor);
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_word_match() {
        assert!(contains_whole_word("let ws_count = 0;", "ws_count"));
        assert!(contains_whole_word("*ws_count += 1;", "ws_count"));
        assert!(contains_whole_word("ws_count", "ws_count")); // exact match
    }

    #[test]
    fn whole_word_no_partial_match() {
        // ws_count should NOT match ws_count_extra or pre_ws_count
        assert!(!contains_whole_word("ws_count_extra += 1;", "ws_count"));
        assert!(!contains_whole_word("pre_ws_count += 1;", "ws_count"));
    }

    #[test]
    fn whole_word_multiple_occurrences() {
        let src = "let a = ws_total; let b = ws_total + 1;";
        assert!(contains_whole_word(src, "ws_total"));
    }
}
