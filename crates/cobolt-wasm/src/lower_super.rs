// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **`super::` reads that wait** (ABI A24, WASM-011 R11, T17).
//!
//! In a standalone application `super::` can be the form that opened the window, which lives in
//! another window: its answer comes later, and a program's read is synchronous. The module therefore
//! asks with `GET_WAIT` / `CALLR_WAIT`, the host answers `SUSPENDED_QUERY` ("on its way"), the task
//! suspends and asks again after `pr_resume`.
//!
//! The wait is **not** taken in the middle of an expression: the run-time's scratch (the register
//! stack, the temporaries, the character temporaries' lengths, a `DISPLAY` line being built) is
//! shared by every task, and another instance's events run while this task waits. So the reads of a
//! statement are **hoisted before it**: all of them are asked in one pass, the task suspends once,
//! and on the resume the same pass asks again and every answer is at hand. The statement then runs
//! once, from the answers, which sit in character temporaries ([`Lower::super_reads`]) that its own
//! lowering finds by the read's place in the source.
//!
//! Which statements: those whose expressions are plain reads — `MOVE`, `ADD`, `SUBTRACT`,
//! `MULTIPLY`, `DIVIDE`, `COMPUTE`, `IF`, `EVALUATE` (the subjects), `DISPLAY`, `PERFORM n TIMES`
//! — and the conditions at the head of `PERFORM … UNTIL` / `VARYING`, each time round. A `super::`
//! read anywhere else keeps answering from the replica of the opener's properties, as it always did
//! (the module asks with the plain `GET`, which the host never answers "pending").

use crate::error::GenResult;
use crate::lower::Lower;
use crate::lower_form::{Seg, CTRL_SUPER};
use cobolt_ast::expr::{Condition, Expr};
use cobolt_ast::stmt::{EvalSubject, PerformTarget, Stmt};
use cobolt_wasm_rt::cmd::{Q_CALLRW, Q_GETW};

/// Is the chain `e` a member of `super`? (`super::Status`, `super::GetProperty("K")`.)
fn is_super_chain(e: &Expr) -> bool {
    let mut cur = e;
    while let Expr::Member { recv, .. } = cur {
        cur = recv.as_ref();
    }
    matches!(cur, Expr::Identifier(n, _) if n.eq_ignore_ascii_case("super"))
}

/// The `super::` member reads inside `e`, left to right. A read is a whole chain: what is inside its
/// arguments is not collected (the caller refuses one that holds another read).
fn reads_in<'e>(e: &'e Expr, out: &mut Vec<&'e Expr>) {
    match e {
        Expr::Member { recv, args, .. } => {
            if is_super_chain(e) {
                out.push(e);
            } else {
                reads_in(recv, out);
                args.iter().for_each(|a| reads_in(a, out));
            }
        }
        Expr::Qualified { of, .. } => reads_in(of, out),
        Expr::Subscript { base, indices, .. } => {
            reads_in(base, out);
            indices.iter().for_each(|i| reads_in(i, out));
        }
        Expr::RefMod {
            base,
            start,
            length,
            ..
        } => {
            reads_in(base, out);
            reads_in(start, out);
            if let Some(l) = length {
                reads_in(l, out);
            }
        }
        Expr::FunctionCall { args, .. } => args.iter().for_each(|a| reads_in(a, out)),
        Expr::Arithmetic { lhs, rhs, .. } => {
            reads_in(lhs, out);
            reads_in(rhs, out);
        }
        Expr::Unary { operand, .. } => reads_in(operand, out),
        _ => {}
    }
}

/// The `super::` reads inside a condition.
fn cond_reads<'e>(c: &'e Condition, out: &mut Vec<&'e Expr>) {
    match c {
        Condition::Comparison { lhs, rhs, .. } => {
            reads_in(lhs, out);
            reads_in(rhs, out);
        }
        Condition::Not(inner, _) => cond_reads(inner, out),
        Condition::And(a, b, _) | Condition::Or(a, b, _) => {
            cond_reads(a, out);
            cond_reads(b, out);
        }
        Condition::ClassTest { expr, .. } | Condition::SignTest { expr, .. } => reads_in(expr, out),
        Condition::NameOrAbbrev { subject, .. } => reads_in(subject, out),
        Condition::ConditionRef(e, _) => reads_in(e, out),
        _ => {}
    }
}

/// The `super::` reads in the expressions a statement evaluates before it does anything.
fn stmt_reads<'s>(s: &'s Stmt, out: &mut Vec<&'s Expr>) {
    match s {
        Stmt::Move { from, .. } => reads_in(from, out),
        Stmt::Add { operands, .. } | Stmt::Subtract { operands, .. } => {
            operands.iter().for_each(|e| reads_in(e, out))
        }
        Stmt::Multiply { lhs, by, .. } | Stmt::Divide { lhs, by, .. } => {
            reads_in(lhs, out);
            reads_in(by, out);
        }
        Stmt::Compute { expr, .. } => reads_in(expr, out),
        Stmt::If { condition, .. } => cond_reads(condition, out),
        Stmt::Evaluate { subjects, .. } => {
            for s in subjects {
                if let EvalSubject::Expr(e) = s {
                    reads_in(e, out);
                }
            }
        }
        Stmt::Display { operands, .. } => operands.iter().for_each(|e| reads_in(e, out)),
        Stmt::Perform {
            target: PerformTarget::Times { count, .. },
            ..
        } => reads_in(count, out),
        _ => {}
    }
}

impl Lower<'_, '_> {
    /// Before statement `s`: ask the host for every `super::` read it makes (see the module's text).
    pub(crate) fn hoist_super_reads_of(&mut self, s: &Stmt, line: u32) -> GenResult<()> {
        self.super_reads.clear();
        let mut found = Vec::new();
        stmt_reads(s, &mut found);
        self.hoist_super_reads(&found, line)
    }

    /// At the head of a loop, before its condition is evaluated (each time round): the same.
    pub(crate) fn hoist_super_cond(&mut self, c: &Condition, line: u32) -> GenResult<()> {
        self.super_reads.clear();
        let mut found = Vec::new();
        cond_reads(c, &mut found);
        if found.is_empty() {
            return Ok(());
        }
        // A loop head starts a fresh evaluation, whatever the body before it left in the temporaries.
        self.ctemp_next = 0;
        self.hoist_super_reads(&found, line)
    }

    fn hoist_super_reads(&mut self, found: &[&Expr], line: u32) -> GenResult<()> {
        if found.is_empty() || self.form.is_none() {
            return Ok(());
        }
        let mut asks: Vec<SuperAsk<'_>> = Vec::with_capacity(found.len());
        for e in found.iter().copied() {
            let Some((root, segs)) = Self::chain(e) else {
                continue;
            };
            if self.host_root(root) != Some(CTRL_SUPER) {
                continue;
            }
            for seg in &segs {
                let mut inner = Vec::new();
                seg.args.iter().for_each(|a| reads_in(a, &mut inner));
                if !inner.is_empty() {
                    return self.err(line, "a super:: read cannot take another super:: read as an argument; read the inner one into a data item first");
                }
            }
            // Only the segments the host answers are asked (`super::Status::Trim()` asks `Status`; the
            // value method is applied to the answer when the statement reads it, `member_value`).
            let mut segs = segs;
            segs.truncate(Self::host_prefix_len(&segs));
            let k = self.next_ctemp(line)?;
            let op = if segs.last().is_some_and(|s| s.parens) {
                Q_CALLRW
            } else {
                Q_GETW
            };
            self.super_reads.insert(e.span(), k);
            asks.push(SuperAsk {
                segs,
                op: op as i32,
                k,
            });
        }
        self.ask_super(asks)
    }

    /// The answer already waiting in a character temporary for the read `e` (hoisted before the statement).
    pub(crate) fn hoisted_super_read(&self, e: &Expr) -> Option<u32> {
        self.super_reads.get(&e.span()).copied()
    }
}

/// One read to ask: the chain, the waiting opcode and the character temporary for the answer.
pub(crate) struct SuperAsk<'e> {
    pub segs: Vec<Seg<'e>>,
    pub op: i32,
    pub k: u32,
}
