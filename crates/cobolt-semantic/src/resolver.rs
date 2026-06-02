// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Name resolution pass.
//!
//! Walks all statements and checks that:
//!
//! * Every data-item reference (`Expr::Identifier`) names a declared item.
//! * Every PERFORM target names a declared paragraph or section.
//! * Every GO TO target names a declared paragraph.
//! * CALL targets that are literals are left unchecked (external programs).
//!
//! Unknown names produce [`Severity::Warning`] rather than hard errors
//! because COBOL programs commonly reference items from copybooks or
//! runtime libraries that are not present in the source being analysed.

use cobolt_ast::{
    expr::{Condition, Expr},
    program::{ProcedureBody, Program},
    stmt::{CallArg, PerformTarget, Stmt, WhenValue},
};

use crate::{symbol_table::SymbolTable, SemanticDiagnostic, Severity};

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn resolve(
    program: &Program,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    let mut ctx = ResolveCtx { symbols, diagnostics };
    match &program.procedure.body {
        ProcedureBody::Paragraphs(paras) => {
            for para in paras {
                for stmt in &para.stmts {
                    ctx.resolve_stmt(stmt);
                }
            }
        }
        ProcedureBody::Sections(secs) => {
            for sec in secs {
                for para in &sec.paragraphs {
                    for stmt in &para.stmts {
                        ctx.resolve_stmt(stmt);
                    }
                }
            }
        }
    }
}

// ── Internal context ──────────────────────────────────────────────────────────

struct ResolveCtx<'a> {
    symbols: &'a SymbolTable,
    diagnostics: &'a mut Vec<SemanticDiagnostic>,
}

impl<'a> ResolveCtx<'a> {
    fn warn(&mut self, msg: impl Into<String>, span: cobolt_lexer::Span) {
        self.diagnostics.push(SemanticDiagnostic {
            severity: Severity::Warning,
            message: msg.into(),
            span,
        });
    }

    fn resolve_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Move { from, to, .. } => {
                self.resolve_expr(from);
                for t in to { self.resolve_expr(t); }
            }
            Stmt::MoveCorresponding { from, to, .. } => {
                self.resolve_expr(from);
                self.resolve_expr(to);
            }
            Stmt::Add { operands, to, giving, .. } => {
                for e in operands { self.resolve_expr(e); }
                for t in to       { self.resolve_expr(t); }
                if let Some(g) = giving { self.resolve_expr(g); }
            }
            Stmt::Subtract { operands, from, giving, .. } => {
                for e in operands { self.resolve_expr(e); }
                for f in from     { self.resolve_expr(f); }
                if let Some(g) = giving { self.resolve_expr(g); }
            }
            Stmt::Multiply { lhs, by, giving, .. } => {
                self.resolve_expr(lhs);
                self.resolve_expr(by);
                if let Some(g) = giving { self.resolve_expr(g); }
            }
            Stmt::Divide { lhs, by, giving, remainder, .. } => {
                self.resolve_expr(lhs);
                self.resolve_expr(by);
                if let Some(g) = giving    { self.resolve_expr(g); }
                if let Some(r) = remainder { self.resolve_expr(r); }
            }
            Stmt::Compute { target, expr, .. } => {
                self.resolve_expr(target);
                self.resolve_expr(expr);
            }
            Stmt::If { condition, then_stmts, else_stmts, .. } => {
                self.resolve_condition(condition);
                for s in then_stmts { self.resolve_stmt(s); }
                for s in else_stmts { self.resolve_stmt(s); }
            }
            Stmt::Evaluate { whens, other_stmts, .. } => {
                for w in whens {
                    for v in &w.values {
                        if let WhenValue::Condition(c) = v {
                            self.resolve_condition(c);
                        }
                    }
                    for s in &w.stmts { self.resolve_stmt(s); }
                }
                for s in other_stmts { self.resolve_stmt(s); }
            }
            Stmt::Perform { target, .. } => {
                match target {
                    PerformTarget::Paragraph(name, s) => self.check_procedure(name, *s),
                    PerformTarget::Section(name, s)   => self.check_procedure(name, *s),
                    PerformTarget::Thru { from, to, span } => {
                        self.check_procedure(from, *span);
                        self.check_procedure(to,   *span);
                    }
                    PerformTarget::Inline { stmts }
                    | PerformTarget::Times { stmts, .. } => {
                        for s in stmts { self.resolve_stmt(s); }
                    }
                    PerformTarget::Until { condition, stmts, .. } => {
                        self.resolve_condition(condition);
                        for s in stmts { self.resolve_stmt(s); }
                    }
                    PerformTarget::Varying { var, from, by, until, stmts, .. } => {
                        self.resolve_expr(var);
                        self.resolve_expr(from);
                        self.resolve_expr(by);
                        self.resolve_condition(until);
                        for s in stmts { self.resolve_stmt(s); }
                    }
                }
            }
            Stmt::GoTo { target, span } => self.check_procedure(target, *span),
            Stmt::GoToDepending { targets, depending, span } => {
                for t in targets { self.check_procedure(t, *span); }
                self.resolve_expr(depending);
            }
            Stmt::Display { operands, .. } => {
                for e in operands { self.resolve_expr(e); }
            }
            Stmt::Accept { target, .. } => self.resolve_expr(target),
            Stmt::Call { program, using, returning, on_exception, .. } => {
                self.resolve_expr(program);
                for arg in using {
                    let e = match arg {
                        CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e,
                    };
                    self.resolve_expr(e);
                }
                if let Some(r) = returning { self.resolve_expr(r); }
                for s in on_exception { self.resolve_stmt(s); }
            }
            Stmt::Write { record, from, .. } => {
                self.resolve_expr(record);
                if let Some(f) = from { self.resolve_expr(f); }
            }
            Stmt::Rewrite { record, from, .. } => {
                self.resolve_expr(record);
                if let Some(f) = from { self.resolve_expr(f); }
            }
            Stmt::Read { into, at_end, not_at_end, .. } => {
                if let Some(i) = into { self.resolve_expr(i); }
                for s in at_end     { self.resolve_stmt(s); }
                for s in not_at_end { self.resolve_stmt(s); }
            }
            // EXEC RUST — source is opaque Rust; the exec_rust pass handles it.
            Stmt::ExecRust { .. } => {}
            Stmt::TryCatch { try_stmts, catch_stmts, finally_stmts, .. } => {
                for s in try_stmts     { self.resolve_stmt(s); }
                for s in catch_stmts   { self.resolve_stmt(s); }
                for s in finally_stmts { self.resolve_stmt(s); }
            }
            Stmt::Throw { message, .. } => {
                self.resolve_expr(message);
            }
            _ => {}
        }
    }

    fn resolve_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Identifier(name, span) => {
                // Skip warnings for CoBolt runtime-injected identifiers.
                // These are declared by the code generator or provided by the
                // runtime as special registers; they will not appear in the
                // user-authored DATA DIVISION.
                let is_runtime = name.starts_with("COBOL-")
                    || name.starts_with("COBOLT-")
                    || matches!(
                        name.as_str(),
                        "RETURN-CODE"
                            | "WHEN-COMPILED"
                            | "LINAGE-COUNTER"
                            | "FORM-NAME"
                    );
                if !is_runtime && !self.symbols.has_data_item(name) && name.len() > 1 {
                    self.warn(
                        format!("identifier '{name}' is not declared in DATA DIVISION"),
                        *span,
                    );
                }
            }
            // Qualified: `A OF B`  — the `of` part is the qualifying group expr
            Expr::Qualified { of, .. } => {
                self.resolve_expr(of);
            }
            // Subscript: `TABLE-ITEM(index)`
            Expr::Subscript { base, indices, .. } => {
                self.resolve_expr(base);
                for idx in indices { self.resolve_expr(idx); }
            }
            Expr::Unary { operand, .. } => self.resolve_expr(operand),
            // Binary arithmetic
            Expr::Arithmetic { lhs, rhs, .. } => {
                self.resolve_expr(lhs);
                self.resolve_expr(rhs);
            }
            Expr::FunctionCall { args, .. } => {
                for a in args { self.resolve_expr(a); }
            }
            // Literals and figurative constants need no resolution.
            Expr::Literal(..) => {}
        }
    }

    fn resolve_condition(&mut self, cond: &Condition) {
        match cond {
            Condition::Comparison { lhs, rhs, .. } => {
                self.resolve_expr(lhs);
                self.resolve_expr(rhs);
            }
            Condition::And(a, b, _) => {
                self.resolve_condition(a);
                self.resolve_condition(b);
            }
            Condition::Or(a, b, _) => {
                self.resolve_condition(a);
                self.resolve_condition(b);
            }
            Condition::Not(inner, _) => self.resolve_condition(inner),
            Condition::ClassTest { expr, .. } => self.resolve_expr(expr),
            Condition::SignTest  { expr, .. } => self.resolve_expr(expr),
            Condition::ConditionName(name, span) => {
                if !self.symbols.has_data_item(name) {
                    self.warn(
                        format!("condition name '{name}' is not declared"),
                        *span,
                    );
                }
            }
        }
    }

    fn check_procedure(&mut self, name: &str, span: cobolt_lexer::Span) {
        if !self.symbols.has_procedure(name) {
            self.warn(
                format!("paragraph or section '{name}' is not defined"),
                span,
            );
        }
    }
}
