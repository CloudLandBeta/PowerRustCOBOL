// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Type-checking pass.
//!
//! COBOL is a weakly typed language, but certain operations require compatible
//! types.  This pass checks the most important rules:
//!
//! | Rule | Diagnostic |
//! |------|-----------|
//! | `COMPUTE` receiving field must be numeric | Error |
//! | `ADD`/`SUBTRACT`/`MULTIPLY`/`DIVIDE` receiving fields must be numeric | Error |
//! | `MOVE` numeric literal to alphanumeric field | Warning |
//! | `PERFORM … TIMES` count must be numeric | Error |
//!
//! Identifiers that are not in the symbol table are skipped (the resolver
//! already warned about them).

use cobolt_ast::{
    data::{PicKind, Usage},
    expr::{Expr, Literal},
    program::{ProcedureBody, Program},
    stmt::{PerformTarget, Stmt},
};

use crate::{symbol_table::SymbolTable, SemanticDiagnostic, Severity};

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn check(
    program: &Program,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    let mut ctx = TypeCtx { symbols, diagnostics };
    match &program.procedure.body {
        ProcedureBody::Paragraphs(paras) => {
            for para in paras {
                for stmt in &para.stmts {
                    ctx.check_stmt(stmt);
                }
            }
        }
        ProcedureBody::Sections(secs) => {
            for sec in secs {
                for para in &sec.paragraphs {
                    for stmt in &para.stmts {
                        ctx.check_stmt(stmt);
                    }
                }
            }
        }
    }
}

// ── Internal context ──────────────────────────────────────────────────────────

struct TypeCtx<'a> {
    symbols: &'a SymbolTable,
    diagnostics: &'a mut Vec<SemanticDiagnostic>,
}

impl<'a> TypeCtx<'a> {
    fn error(&mut self, msg: impl Into<String>, span: cobolt_lexer::Span) {
        self.diagnostics.push(SemanticDiagnostic {
            severity: Severity::Error,
            message: msg.into(),
            span,
        });
    }

    fn warn(&mut self, msg: impl Into<String>, span: cobolt_lexer::Span) {
        self.diagnostics.push(SemanticDiagnostic {
            severity: Severity::Warning,
            message: msg.into(),
            span,
        });
    }

    /// Returns `Some(true)` if `expr` is a known numeric data item,
    /// `Some(false)` if it's a known non-numeric item, `None` if unknown.
    fn is_numeric_expr(&self, expr: &Expr) -> Option<bool> {
        if let Expr::Identifier(name, _) = expr {
            self.symbols.data_item(name).map(|info| {
                // Numeric by PIC category, or by a numeric USAGE that needs no PIC
                // (COMP-1/COMP-2 are floating-point numeric items).
                matches!(info.pic_kind, Some(PicKind::Numeric | PicKind::NumericEdited))
                    || matches!(
                        info.usage,
                        Usage::Comp1
                            | Usage::Comp2
                            | Usage::Binary
                            | Usage::Comp
                            | Usage::Comp3
                            | Usage::Comp5
                            | Usage::PackedDecimal
                    )
            })
        } else {
            None
        }
    }

    fn require_numeric_receiver(&mut self, expr: &Expr, context: &str) {
        if let Some(false) = self.is_numeric_expr(expr) {
            if let Expr::Identifier(name, span) = expr {
                self.error(
                    format!("'{name}' is not numeric; {context} requires a numeric receiver"),
                    *span,
                );
            }
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            // ── Arithmetic verbs ──────────────────────────────────────────────
            Stmt::Compute { targets, .. } => {
                for (t, _) in targets { self.require_numeric_receiver(t, "COMPUTE"); }
            }
            Stmt::Add { to, giving, .. } => {
                if let Some(g) = giving {
                    self.require_numeric_receiver(g, "ADD GIVING");
                } else {
                    for t in to { self.require_numeric_receiver(t, "ADD TO"); }
                }
            }
            Stmt::Subtract { from, giving, .. } => {
                if let Some(g) = giving {
                    self.require_numeric_receiver(g, "SUBTRACT GIVING");
                } else {
                    for f in from { self.require_numeric_receiver(f, "SUBTRACT FROM"); }
                }
            }
            Stmt::Multiply { giving, .. } => {
                if let Some(g) = giving {
                    self.require_numeric_receiver(g, "MULTIPLY GIVING");
                }
            }
            Stmt::Divide { giving, remainder, .. } => {
                if let Some(g) = giving {
                    self.require_numeric_receiver(g, "DIVIDE GIVING");
                }
                if let Some(r) = remainder {
                    self.require_numeric_receiver(r, "DIVIDE REMAINDER");
                }
            }

            // ── MOVE: warn if moving a numeric literal to a non-numeric field ─
            Stmt::Move { from, to, .. } => {
                let from_is_numeric_lit = matches!(
                    from,
                    Expr::Literal(Literal::Integer(_), _)
                        | Expr::Literal(Literal::Float(_), _)
                        | Expr::Literal(Literal::Decimal(_, _), _)
                );
                if from_is_numeric_lit {
                    for t in to {
                        if let (Some(false), Expr::Identifier(name, span)) =
                            (self.is_numeric_expr(t), t)
                        {
                            self.warn(
                                format!(
                                    "moving a numeric literal to '{name}' \
                                     which has an alphanumeric PIC clause"
                                ),
                                *span,
                            );
                        }
                    }
                }
            }

            // ── PERFORM ───────────────────────────────────────────────────────
            Stmt::Perform { target, .. } => {
                match target {
                    PerformTarget::Times { count, stmts } => {
                        if let Some(false) = self.is_numeric_expr(count) {
                            if let Expr::Identifier(name, span) = count {
                                self.error(
                                    format!(
                                        "'{name}' is not numeric; \
                                         PERFORM … TIMES requires a numeric count"
                                    ),
                                    *span,
                                );
                            }
                        }
                        for s in stmts { self.check_stmt(s); }
                    }
                    PerformTarget::Inline { stmts }
                    | PerformTarget::Until { stmts, .. }
                    | PerformTarget::Varying { stmts, .. } => {
                        for s in stmts { self.check_stmt(s); }
                    }
                    _ => {}
                }
            }

            // ── IF / EVALUATE — recurse into branches ─────────────────────────
            Stmt::If { then_stmts, else_stmts, .. } => {
                for s in then_stmts { self.check_stmt(s); }
                for s in else_stmts { self.check_stmt(s); }
            }
            Stmt::Evaluate { whens, other_stmts, .. } => {
                for w in whens { for s in &w.stmts { self.check_stmt(s); } }
                for s in other_stmts { self.check_stmt(s); }
            }

            // ── EXEC RUST — source is opaque Rust, no COBOL type checks ───────
            Stmt::ExecRust { .. } => {}

            _ => {}
        }
    }
}
