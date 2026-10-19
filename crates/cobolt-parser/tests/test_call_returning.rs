// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `RETURNING` on both sides of a call: `CALL … RETURNING receiver` in the
//! caller and `PROCEDURE DIVISION [USING …] RETURNING item` in the callee.
//!
//! The runtime used to ignore the phrase (the interpreter read it into a
//! parameter named `_returning`), so the AST shape is pinned here: what the
//! interpreter now acts on has to be what the parser produces.

use cobolt_ast::expr::Expr;
use cobolt_ast::program::{ProcedureBody, Program};
use cobolt_ast::stmt::{CallArg, Stmt};
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;

fn program_of(src: &str) -> Program {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(result.diagnostics.is_empty(), "unexpected diagnostics: {:?}", result.diagnostics);
    result.program.expect("no program")
}

/// The statements of a minimal caller whose paragraph holds `body`.
fn stmts_of(body: &str) -> Vec<Stmt> {
    let src = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
         DATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 A PIC 9(4).\n01 B PIC 9(4).\n01 TBL PIC 9(4) OCCURS 3.\n01 I PIC 9.\n\
         PROCEDURE DIVISION.\nMAIN.\n{body}\n"
    );
    match program_of(&src).procedure.body {
        ProcedureBody::Paragraphs(mut paras) => paras.pop().map(|p| p.stmts).unwrap_or_default(),
        ProcedureBody::Sections(secs) => secs.into_iter().flat_map(|s| s.paragraphs).flat_map(|p| p.stmts).collect(),
    }
}

/// The one CALL in `body`: its argument count, its RETURNING receiver, and
/// whether it carries ON EXCEPTION / NOT ON EXCEPTION bodies.
fn the_call(body: &str) -> (usize, Option<Expr>, bool, bool) {
    stmts_of(body)
        .into_iter()
        .find_map(|s| match s {
            Stmt::Call { using, returning, on_exception, not_on_exception, .. } => {
                Some((using.len(), returning, !on_exception.is_empty(), !not_on_exception.is_empty()))
            }
            _ => None,
        })
        .expect("no CALL parsed")
}

fn name_of(e: &Expr) -> &str {
    match e {
        Expr::Identifier(n, _) => n,
        other => panic!("expected a plain identifier, got {other:?}"),
    }
}

#[test]
fn a_call_with_using_and_returning_keeps_both() {
    let (args, ret, _, _) = the_call("CALL \"SUB\" USING A RETURNING B.");
    assert_eq!(args, 1);
    assert_eq!(name_of(&ret.expect("RETURNING lost")), "B");
}

#[test]
fn returning_needs_no_using() {
    let (args, ret, _, _) = the_call("CALL \"SUB\" RETURNING B.");
    assert_eq!(args, 0);
    assert_eq!(name_of(&ret.expect("RETURNING lost")), "B");
}

#[test]
fn a_call_without_returning_has_none() {
    let (_, ret, _, _) = the_call("CALL \"SUB\" USING A.");
    assert!(ret.is_none());
}

/// `BY CONTENT` governs the operands after it; RETURNING ends the list and is
/// not swallowed as one more argument.
#[test]
fn returning_ends_the_argument_list() {
    let stmts = stmts_of("CALL \"SUB\" USING BY CONTENT A BY REFERENCE B RETURNING TBL (I).");
    let Some(Stmt::Call { using, returning, .. }) = stmts.into_iter().next() else {
        panic!("no CALL parsed");
    };
    assert!(matches!(using[0], CallArg::ByContent(_)));
    assert!(matches!(using[1], CallArg::ByReference(_)));
    assert_eq!(using.len(), 2, "RETURNING TBL (I) is not an argument");
    assert!(matches!(returning, Some(Expr::Subscript { .. })));
}

#[test]
fn returning_precedes_the_exception_phrases() {
    let (_, ret, on_exc, not_on_exc) = the_call(
        "CALL \"SUB\" USING A RETURNING B\n    ON EXCEPTION DISPLAY \"no\"\n    NOT ON EXCEPTION DISPLAY \"yes\"\nEND-CALL.",
    );
    assert_eq!(name_of(&ret.expect("RETURNING lost")), "B");
    assert!(on_exc && not_on_exc, "both phrases follow RETURNING");
}

#[test]
fn a_qualified_receiver_is_kept_whole() {
    let (_, ret, _, _) = the_call("CALL \"SUB\" RETURNING A OF B.");
    assert!(matches!(ret, Some(Expr::Qualified { .. })));
}

fn procedure_header(header: &str) -> (Vec<String>, Option<String>) {
    let src = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. SUB.\n\
         DATA DIVISION.\nLINKAGE SECTION.\n01 LK-A PIC 9(4).\n01 LK-B PIC 9(4).\n01 LK-R PIC 9(4).\n\
         {header}\n    GOBACK.\n"
    );
    let p = program_of(&src).procedure;
    (p.using, p.returning)
}

#[test]
fn the_callees_header_names_its_returning_item() {
    let (using, ret) = procedure_header("PROCEDURE DIVISION USING LK-A LK-B RETURNING LK-R.");
    assert_eq!(using, ["LK-A", "LK-B"]);
    assert_eq!(ret.as_deref(), Some("LK-R"));
}

#[test]
fn the_callees_header_needs_no_using() {
    let (using, ret) = procedure_header("PROCEDURE DIVISION RETURNING LK-R.");
    assert!(using.is_empty());
    assert_eq!(ret.as_deref(), Some("LK-R"));
}

#[test]
fn a_header_without_returning_has_none() {
    let (using, ret) = procedure_header("PROCEDURE DIVISION USING LK-A.");
    assert_eq!(using, ["LK-A"]);
    assert!(ret.is_none());
}
