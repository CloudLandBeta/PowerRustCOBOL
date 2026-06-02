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
    program::{ProcedureBody, Program},
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

/// Visit `stmt` and recurse into nested statement lists.
fn walk_stmt(stmt: &Stmt, visitor: &mut impl FnMut(&Stmt)) {
    visitor(stmt);
    match stmt {
        Stmt::If { then_stmts, else_stmts, .. } => {
            for s in then_stmts { walk_stmt(s, visitor); }
            for s in else_stmts { walk_stmt(s, visitor); }
        }
        Stmt::Evaluate { whens, other_stmts, .. } => {
            for w in whens {
                for s in &w.stmts { walk_stmt(s, visitor); }
            }
            for s in other_stmts { walk_stmt(s, visitor); }
        }
        Stmt::Perform { target, .. } => {
            use cobolt_ast::stmt::PerformTarget;
            match target {
                PerformTarget::Inline { stmts }
                | PerformTarget::Times { stmts, .. }
                | PerformTarget::Until { stmts, .. }
                | PerformTarget::Varying { stmts, .. } => {
                    for s in stmts { walk_stmt(s, visitor); }
                }
                PerformTarget::Paragraph(..)
                | PerformTarget::Section(..)
                | PerformTarget::Thru { .. } => {}
            }
        }
        Stmt::Read { at_end, not_at_end, .. } => {
            for s in at_end     { walk_stmt(s, visitor); }
            for s in not_at_end { walk_stmt(s, visitor); }
        }
        Stmt::Call { on_exception, .. } => {
            for s in on_exception { walk_stmt(s, visitor); }
        }
        // Leaf statements — no nested statements.
        _ => {}
    }
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
