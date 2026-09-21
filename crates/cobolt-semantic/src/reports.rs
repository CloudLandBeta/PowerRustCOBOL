// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 062 — a report printed into a Viewer, checked before it runs.
//!
//! Two things are wrong at compile time rather than at run time, and both are
//! wrong *silently* without this pass:
//!
//! * **`ORGANIZATION IS MARKDOWN`/`HTML` on a file that is not a Viewer's.**
//!   These organizations say how a document is RENDERED; on a disk file there
//!   is nobody to render it. Every `match` on `FileOrganization` in the product
//!   has a `_` arm, so such a file would quietly behave as record `SEQUENTIAL`
//!   — a declaration that means one thing and does another.
//!
//! * **Page control on a rendered report.** `ADVANCING PAGE`, `LINAGE` and
//!   `AT END-OF-PAGE` describe a printed page. Markdown and HTML flow: they
//!   have no pages to break, and a report that quietly loses the breaks its
//!   program asked for is worse than one that will not compile (spec R20).
//!
//! `ADVANCING n LINES` is deliberately **not** here. Blank lines mean something
//! in every one of the three organizations — in Markdown they are what
//! separates one paragraph from the next — so a program is right to ask for
//! them and the runtime emits them.

use cobolt_ast::program::{DataSection, Program};
use cobolt_ast::stmt::Stmt;
use cobolt_lexer::Span;

use crate::{SemanticDiagnostic, Severity};

/// Check this program and all nested programs.
pub fn check(program: &Program, diagnostics: &mut Vec<SemanticDiagnostic>) {
    check_program(program, diagnostics);
    for nested in &program.nested_programs {
        check(nested, diagnostics);
    }
}

fn error(diagnostics: &mut Vec<SemanticDiagnostic>, span: Span, message: String) {
    diagnostics.push(SemanticDiagnostic {
        severity: Severity::Error,
        message,
        span,
    });
}

fn check_program(program: &Program, diagnostics: &mut Vec<SemanticDiagnostic>) {
    let Some(env) = &program.environment else { return };
    let Some(io) = &env.input_output else { return };

    // ── R4: a rendered organization belongs to a Viewer ───────────────────────
    let mut rendered: Vec<&str> = Vec::new();
    for fc in &io.file_controls {
        if !fc.organization.is_rendered() {
            continue;
        }
        if fc.viewer_target.is_some() {
            rendered.push(fc.name.as_str());
            continue;
        }
        error(
            diagnostics,
            fc.span,
            format!(
                "file '{}' is declared ORGANIZATION IS {:?}, which describes how a Viewer \
                 renders a document — it needs ASSIGN TO VIEWER \"<control-id>\" to say \
                 which Viewer reads it",
                fc.name, fc.organization
            ),
        );
    }
    if rendered.is_empty() {
        return;
    }
    let is_rendered = |name: &str| rendered.iter().any(|f| f.eq_ignore_ascii_case(name));

    // ── R20: page control on a rendered report ───────────────────────────────
    //
    // The FD carries LINAGE, the SELECT carries the organization, and they are
    // joined by the file's name — which is why this check lives here, where
    // both divisions are in hand.
    let mut record_owner: Vec<(String, String)> = Vec::new();
    if let Some(data) = &program.data {
        for section in &data.sections {
            let DataSection::FileSection(fds) = section else {
                continue;
            };
            for fd in fds {
                if !is_rendered(&fd.name) {
                    continue;
                }
                for rec in &fd.records {
                    // A FILLER record has no name to write by, so it cannot be
                    // the subject of a WRITE and needs no entry here.
                    if let Some(name) = &rec.name {
                        record_owner.push((name.to_uppercase(), fd.name.clone()));
                    }
                }
                if fd.linage.is_some() {
                    error(
                        diagnostics,
                        fd.span,
                        format!(
                            "file '{}' is a rendered report, so it has no printed page for \
                             LINAGE to measure — remove the clause, or declare the file \
                             ORGANIZATION IS SEQUENTIAL",
                            fd.name
                        ),
                    );
                }
            }
        }
    }

    let owner_of = |record: &str| -> Option<&str> {
        record_owner
            .iter()
            .find(|(r, _)| r == &record.to_uppercase())
            .map(|(_, f)| f.as_str())
    };

    crate::exec_rust::walk_stmts_in_program(program, &mut |stmt: &Stmt| {
        let Stmt::Write {
            record,
            advancing,
            at_eop,
            not_at_eop,
            span,
            ..
        } = stmt
        else {
            return;
        };
        let Some(name) = record_name(record) else { return };
        let Some(file) = owner_of(&name) else { return };

        if advancing.as_ref().is_some_and(is_advancing_page) {
            error(
                diagnostics,
                *span,
                format!(
                    "WRITE ... ADVANCING PAGE on '{file}', which is a rendered report: \
                     Markdown and HTML flow and have no page to advance to — declare the \
                     file ORGANIZATION IS SEQUENTIAL for a paged report"
                ),
            );
        }
        if !at_eop.is_empty() || !not_at_eop.is_empty() {
            error(
                diagnostics,
                *span,
                format!(
                    "AT END-OF-PAGE on '{file}', which is a rendered report: the condition \
                     is raised by a LINAGE page body, and a rendered report has none"
                ),
            );
        }
    });
}

/// The bare name a `WRITE` names, without resolving it.
fn record_name(record: &cobolt_ast::expr::Expr) -> Option<String> {
    match record {
        cobolt_ast::expr::Expr::Identifier(name, _) => Some(name.clone()),
        _ => None,
    }
}

/// `ADVANCING PAGE`, which the parser leaves as the ordinary word it is — the
/// same recognition `Interpreter::advancing_lines` does, and for the same
/// reason: an undeclared identifier evaluates to zero, so inferring PAGE from a
/// failed evaluation would silently mean "advance 0 lines".
fn is_advancing_page(a: &cobolt_ast::stmt::AdvancingClause) -> bool {
    matches!(
        &a.lines,
        cobolt_ast::expr::Expr::Identifier(name, _) if name.eq_ignore_ascii_case("PAGE")
    )
}
