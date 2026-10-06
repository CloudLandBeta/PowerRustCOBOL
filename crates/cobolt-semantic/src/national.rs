// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Check for national (`PIC N`, `USAGE NATIONAL`) and UTF-8 (`PIC U`,
//! `USAGE UTF-8`) data — spec 077, R16 and Q3.
//!
//! What is refused before the program runs:
//! - a declaration whose USAGE and PICTURE disagree (`PIC X USAGE NATIONAL`);
//! - national numeric and national-edited items, not supported yet (Q3);
//! - `BYTE-LENGTH` anywhere but on a single-`U` UTF-8 item;
//! - a `VALUE` longer than its national or UTF-8 item;
//! - a national or UTF-8 item as an operand or receiver of arithmetic.

use std::collections::HashMap;

use cobolt_ast::Span;
use cobolt_ast::data::{DataDecl, PicKind, Usage};
use cobolt_ast::expr::{Expr, Literal};
use cobolt_ast::program::{DataSection, ProcedureBody, Program};
use cobolt_ast::stmt::Stmt;

use crate::{SemanticDiagnostic, Severity};

/// Which character class a declared name has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    National,
    Utf8,
    Other,
}

pub fn check(program: &Program, diags: &mut Vec<SemanticDiagnostic>) {
    let mut classes: HashMap<String, Vec<Class>> = HashMap::new();
    gather(program, &mut classes, diags);
    check_arithmetic(program, &classes, diags);
}

fn error(diags: &mut Vec<SemanticDiagnostic>, message: String, span: Span) {
    diags.push(SemanticDiagnostic { severity: Severity::Error, message, span });
}

fn decls(p: &Program) -> Vec<&DataDecl> {
    let mut out = Vec::new();
    if let Some(data) = &p.data {
        for sec in &data.sections {
            match sec {
                DataSection::WorkingStorage(d) | DataSection::LocalStorage(d) | DataSection::Linkage(d) => {
                    out.extend(d.iter())
                }
                DataSection::FileSection(fds) => out.extend(fds.iter().flat_map(|f| f.records.iter())),
                DataSection::Screen(_) => {}
            }
        }
    }
    out
}

fn gather(p: &Program, classes: &mut HashMap<String, Vec<Class>>, diags: &mut Vec<SemanticDiagnostic>) {
    fn walk(d: &DataDecl, classes: &mut HashMap<String, Vec<Class>>, diags: &mut Vec<SemanticDiagnostic>) {
        let class = check_decl(d, diags);
        if let Some(n) = &d.name {
            classes.entry(n.to_ascii_uppercase()).or_default().push(class);
        }
        for c in &d.children {
            walk(c, classes, diags);
        }
    }
    for d in decls(p) {
        walk(d, classes, diags);
    }
    for n in &p.nested_programs {
        gather(n, classes, diags);
    }
}

/// Check one declaration; its class.
fn check_decl(d: &DataDecl, diags: &mut Vec<SemanticDiagnostic>) -> Class {
    let name = d.name.clone().unwrap_or_else(|| "FILLER".into());
    let pic = d.picture.as_ref();
    let kind = pic.map(|p| p.kind);
    if d.byte_length.is_some() {
        let single_u = pic.is_some_and(|p| p.kind == PicKind::Utf8 && p.digits == 1);
        if !single_u {
            error(diags, format!("{name}: BYTE-LENGTH goes with PIC U (a single U) only"), d.span);
        }
    }
    match d.usage {
        Usage::National => match kind {
            Some(PicKind::National) => {}
            Some(PicKind::Numeric) => error(
                diags,
                format!(
                    "{name}: national numeric items (PIC 9 USAGE NATIONAL) are not supported yet; \
                     declare it USAGE DISPLAY"
                ),
                d.span,
            ),
            Some(PicKind::NumericEdited | PicKind::AlphanumericEdited) => error(
                diags,
                format!("{name}: national-edited items are not supported yet; use PIC N(n) and edit into it"),
                d.span,
            ),
            Some(_) => error(diags, format!("{name}: USAGE NATIONAL needs PIC N"), d.span),
            None => {}
        },
        Usage::Utf8 => {
            if !matches!(kind, Some(PicKind::Utf8) | None) {
                error(diags, format!("{name}: USAGE UTF-8 needs PIC U"), d.span);
            }
        }
        _ => {}
    }
    let Some(p) = pic else { return Class::Other };
    let class = match p.kind {
        PicKind::National => Class::National,
        PicKind::Utf8 => Class::Utf8,
        _ => return Class::Other,
    };
    // A national or UTF-8 picture is all N, or all U: anything else mixed in
    // is an edited picture (Q3) or a mistake.
    let symbol = if class == Class::National { 'N' } else { 'U' };
    let mixed = expand(&p.template).into_iter().any(|c| c != symbol);
    if mixed {
        let what = if class == Class::National { "national-edited items are not supported yet" } else { "a UTF-8 picture holds only U" };
        error(diags, format!("{name}: PIC {} — {what}", p.template), d.span);
    }
    if let Some(v) = &d.value {
        let text = match v {
            Literal::String(s) | Literal::National(s) | Literal::Utf8(s) => Some(s),
            _ => None,
        };
        if let Some(t) = text {
            let (len, limit, unit) = match (class, d.byte_length) {
                (Class::Utf8, Some(b)) => (t.len(), b as usize, "bytes"),
                _ => (t.chars().count(), p.digits as usize, "characters"),
            };
            if len > limit {
                error(diags, format!("{name}: the VALUE has {len} {unit}; the item holds {limit}"), d.span);
            }
        }
    }
    class
}

/// A picture string, one symbol per position (`N(3)` → `NNN`).
fn expand(template: &str) -> Vec<char> {
    let mut out = Vec::new();
    let mut chars = template.to_ascii_uppercase().chars().collect::<Vec<_>>().into_iter().peekable();
    while let Some(c) = chars.next() {
        if chars.peek() == Some(&'(') {
            chars.next();
            let n: String = chars.by_ref().take_while(|d| *d != ')').collect();
            let n = n.trim().parse::<usize>().unwrap_or(1);
            out.extend(std::iter::repeat_n(c, n));
        } else {
            out.push(c);
        }
    }
    out
}

/// R16: no national or UTF-8 item in arithmetic, as an operand or a receiver.
fn check_arithmetic(p: &Program, classes: &HashMap<String, Vec<Class>>, diags: &mut Vec<SemanticDiagnostic>) {
    let paragraphs: Vec<&cobolt_ast::program::Paragraph> = match &p.procedure.body {
        ProcedureBody::Sections(ss) => ss.iter().flat_map(|s| s.paragraphs.iter()).collect(),
        ProcedureBody::Paragraphs(ps) => ps.iter().collect(),
    };
    let check = |e: &Expr, verb: &str, diags: &mut Vec<SemanticDiagnostic>| {
        let mut found = Vec::new();
        arithmetic_names(e, &mut found);
        for (name, span) in found {
            let c = classes.get(&name.to_ascii_uppercase());
            // Only a name every declaration of which is classed: a duplicate
            // name with an ordinary declaration is not guessed at.
            let all = |k: Class| c.is_some_and(|v| !v.is_empty() && v.iter().all(|x| *x == k));
            let what = if all(Class::National) {
                "a national item"
            } else if all(Class::Utf8) {
                "a UTF-8 item"
            } else {
                continue;
            };
            error(diags, format!("'{name}' is {what}; {verb} takes numeric operands only"), span);
        }
    };
    for pg in paragraphs {
        for s in &pg.stmts {
            s.walk(&mut |st| match st {
                Stmt::Add { operands, to, giving, .. } => {
                    for e in operands.iter().chain(to.iter().map(|(e, _)| e)).chain(giving.iter().map(|(e, _)| e)) {
                        check(e, "ADD", diags);
                    }
                }
                Stmt::Subtract { operands, from, giving, .. } => {
                    for e in operands.iter().chain(from.iter().map(|(e, _)| e)).chain(giving.iter().map(|(e, _)| e)) {
                        check(e, "SUBTRACT", diags);
                    }
                }
                Stmt::Multiply { lhs, by, giving, .. } => {
                    for e in [lhs, by].into_iter().chain(giving.iter().map(|(e, _)| e)) {
                        check(e, "MULTIPLY", diags);
                    }
                }
                Stmt::Divide { lhs, by, giving, remainder, .. } => {
                    for e in [lhs, by].into_iter().chain(giving.iter().map(|(e, _)| e)).chain(remainder.iter()) {
                        check(e, "DIVIDE", diags);
                    }
                }
                Stmt::Compute { targets, expr, .. } => {
                    for e in std::iter::once(expr).chain(targets.iter().map(|(e, _)| e)) {
                        check(e, "COMPUTE", diags);
                    }
                }
                _ => {}
            });
        }
    }
    for n in &p.nested_programs {
        check_arithmetic(n, classes, diags);
    }
}

/// The data names an arithmetic expression computes with. A function's
/// arguments are its own (`FUNCTION ULENGTH(WS-U)` is a number), and a
/// subscript is a position, not an operand.
fn arithmetic_names(e: &Expr, out: &mut Vec<(String, Span)>) {
    match e {
        Expr::Identifier(n, s) => out.push((n.clone(), *s)),
        Expr::Qualified { name, span, .. } => out.push((name.clone(), *span)),
        Expr::Subscript { base, .. } | Expr::RefMod { base, .. } => arithmetic_names(base, out),
        Expr::Arithmetic { lhs, rhs, .. } => {
            arithmetic_names(lhs, out);
            arithmetic_names(rhs, out);
        }
        Expr::Unary { operand, .. } => arithmetic_names(operand, out),
        _ => {}
    }
}
