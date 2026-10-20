// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A literal `CALL "NAME"` must name something that exists (operator report,
//! 2026-10-08).
//!
//! The runtime resolves a literal target against the program-names of the unit
//! it runs, the paragraphs and sections of those programs, and the built-in
//! `COBOL-…` calls. A name none of them answers is skipped with a log line and
//! nothing else, so the handler carries on as if the call had been made: a
//! mistyped procedure name compiled, ran and did nothing.
//!
//! This pass is the compile-time half of that resolution. It runs only when
//! the caller supplies [`crate::AnalyzeOptions::known_programs`] — the names it
//! can see beyond the unit, the project's Common Code — because without them
//! it would reject every call into Common Code. A target held in a data item
//! cannot be known and is not checked, a `CALL` that names `ON EXCEPTION` has
//! said the target is optional, and the built-in `COBOL-…` / `COBOLT-…`
//! namespace is left to the runtime's own table, which this crate cannot see.

use std::collections::HashSet;

use cobolt_ast::expr::{Expr, Literal};
use cobolt_ast::program::{ProcedureBody, Program};
use cobolt_ast::stmt::{CallConvention, Stmt};

use crate::{SemanticDiagnostic, Severity};

/// Check every `CALL` in `program` and in the programs it contains.
pub fn check(program: &Program, known_programs: &HashSet<String>, diagnostics: &mut Vec<SemanticDiagnostic>) {
    let mut defined = HashSet::new();
    collect_names(program, &mut defined);
    check_program(program, &defined, known_programs, diagnostics);
}

/// Every program-name, section name and paragraph name of the unit, in
/// UPPERCASE. Deliberately the whole unit rather than the calling program's
/// own: the runtime registers contained programs unit-wide, and a name it
/// might resolve must never be reported.
fn collect_names(program: &Program, out: &mut HashSet<String>) {
    out.insert(program.identification.program_id.trim().to_ascii_uppercase());
    match &program.procedure.body {
        ProcedureBody::Sections(sections) => {
            for sec in sections {
                out.insert(sec.name.trim().to_ascii_uppercase());
                for para in &sec.paragraphs {
                    out.insert(para.name.trim().to_ascii_uppercase());
                }
            }
        }
        ProcedureBody::Paragraphs(paras) => {
            for para in paras {
                out.insert(para.name.trim().to_ascii_uppercase());
            }
        }
    }
    for decl in &program.procedure.declaratives {
        out.insert(decl.section.trim().to_ascii_uppercase());
        for para in &decl.paras {
            out.insert(para.name.trim().to_ascii_uppercase());
        }
    }
    for nested in &program.nested_programs {
        collect_names(nested, out);
    }
}

fn check_program(
    program: &Program,
    defined: &HashSet<String>,
    known_programs: &HashSet<String>,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    let mut visit = |stmt: &Stmt| check_stmt(stmt, defined, known_programs, diagnostics);
    match &program.procedure.body {
        ProcedureBody::Sections(sections) => {
            for sec in sections {
                for para in &sec.paragraphs {
                    para.stmts.iter().for_each(|s| s.walk(&mut visit));
                }
            }
        }
        ProcedureBody::Paragraphs(paras) => {
            for para in paras {
                para.stmts.iter().for_each(|s| s.walk(&mut visit));
            }
        }
    }
    for decl in &program.procedure.declaratives {
        decl.stmts.iter().for_each(|s| s.walk(&mut visit));
        for para in &decl.paras {
            para.stmts.iter().for_each(|s| s.walk(&mut visit));
        }
    }
    for nested in &program.nested_programs {
        check_program(nested, defined, known_programs, diagnostics);
    }
}

fn check_stmt(
    stmt: &Stmt,
    defined: &HashSet<String>,
    known_programs: &HashSet<String>,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    let Stmt::Call { program: Expr::Literal(Literal::String(target), _), on_exception, span, convention, .. } = stmt else {
        return;
    };
    // A WEBSOCKET target is a program of the backend, never of this unit; the
    // remote-call pass judges its name (WASM-005 R4).
    if *convention == CallConvention::WebSocket {
        return;
    }
    // `ON EXCEPTION` says the target is optional; the program handles its
    // absence.
    if !on_exception.is_empty() {
        return;
    }
    let name = target.trim().to_ascii_uppercase();
    if name.starts_with("COBOL-") || name.starts_with("COBOLT-") {
        return;
    }
    if defined.contains(&name) || known_programs.contains(&name) {
        return;
    }
    let near = crate::resolver::closest_object(&name, defined.iter().chain(known_programs).map(String::as_str));
    let hint = near.map(|n| format!(" Did you mean \"{n}\"?")).unwrap_or_default();
    diagnostics.push(SemanticDiagnostic {
        severity: Severity::Error,
        message: format!(
            "CALL \"{}\": no program, paragraph, section or Common Code procedure has that name, \
             so at run time the call is skipped and the program carries on as if it had been made.{hint} \
             Name ON EXCEPTION on the CALL if the target is optional.",
            target.trim()
        ),
        span: *span,
    });
}

// ── CALL … RETURNING ──────────────────────────────────────────────────────────

/// A `CALL "NAME" … RETURNING receiver` whose target is known not to return a
/// value (2026-10-19).
///
/// The runtime moves the called program's RETURNING item into the receiver. A
/// target with no such item — a program without `PROCEDURE DIVISION …
/// RETURNING`, a paragraph or section, a built-in `COBOL-…` call (those hand
/// their results back through `USING`) — leaves the receiver as it was, and
/// the program carries on with the old value. The pass names the call.
///
/// Only what the unit itself holds can be judged: a target that is a Common
/// Code program, or one held in a data item, is not seen here and is left
/// alone. Unlike [`check`], this needs no project context.
pub fn check_returning(program: &Program, diagnostics: &mut Vec<SemanticDiagnostic>) {
    let mut programs = std::collections::HashMap::new();
    collect_returning(program, &mut programs);
    let mut defined = HashSet::new();
    collect_names(program, &mut defined);
    returning_in_program(program, &programs, &defined, diagnostics);
}

/// Every program-id of the unit, UPPERCASE, with whether it declares a
/// RETURNING item.
fn collect_returning(program: &Program, out: &mut std::collections::HashMap<String, bool>) {
    out.insert(
        program.identification.program_id.trim().to_ascii_uppercase(),
        program.procedure.returning.is_some(),
    );
    for nested in &program.nested_programs {
        collect_returning(nested, out);
    }
}

fn returning_in_program(
    program: &Program,
    programs: &std::collections::HashMap<String, bool>,
    defined: &HashSet<String>,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    let mut visit = |stmt: &Stmt| {
        let Stmt::Call {
            program: Expr::Literal(Literal::String(target), _),
            returning: Some(_),
            span,
            convention: CallConvention::Local,
            ..
        } = stmt
        else {
            return;
        };
        let name = target.trim().to_ascii_uppercase();
        let why = if name.starts_with("COBOL-") || name.starts_with("COBOLT-") {
            "a built-in call hands its results back through its USING items".to_string()
        } else {
            match programs.get(&name) {
                Some(true) => return,
                Some(false) => format!("program {name} declares no RETURNING item (PROCEDURE DIVISION … RETURNING item)"),
                None if defined.contains(&name) => format!("{name} is a paragraph or section, which returns no value"),
                None => return,
            }
        };
        diagnostics.push(SemanticDiagnostic {
            severity: Severity::Warning,
            message: format!(
                "CALL \"{}\" … RETURNING: {why}, so the receiver keeps the value it had before the call.",
                target.trim()
            ),
            span: *span,
        });
    };
    match &program.procedure.body {
        ProcedureBody::Sections(sections) => {
            for sec in sections {
                for para in &sec.paragraphs {
                    para.stmts.iter().for_each(|s| s.walk(&mut visit));
                }
            }
        }
        ProcedureBody::Paragraphs(paras) => {
            for para in paras {
                para.stmts.iter().for_each(|s| s.walk(&mut visit));
            }
        }
    }
    for decl in &program.procedure.declaratives {
        decl.stmts.iter().for_each(|s| s.walk(&mut visit));
        for para in &decl.paras {
            para.stmts.iter().for_each(|s| s.walk(&mut visit));
        }
    }
    for nested in &program.nested_programs {
        returning_in_program(nested, programs, defined, diagnostics);
    }
}
