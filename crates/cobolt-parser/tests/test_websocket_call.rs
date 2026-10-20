// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `CALL … WITH WEBSOCKET LINKAGE` (WASM-005 R1): every combination of the
//! phrase parses into the AST as a calling convention, and every existing
//! `CALL` form still parses as a local call.

use cobolt_ast::expr::{Expr, Literal};
use cobolt_ast::program::ProcedureBody;
use cobolt_ast::stmt::{CallArg, CallConvention, Stmt};
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};

struct Parsed {
    stmts: Vec<Stmt>,
    errors: Vec<String>,
}

fn parse_body(body: &str) -> Parsed {
    let src = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 CUST.\n   05 NAME PIC X(8).\n01 RES.\n   05 CODE-X PIC 99.\n01 PROG-NAME PIC X(12).\n01 WEBSOCKET PIC 9.\n\
         PROCEDURE DIVISION.\nMAIN.\n{body}\n"
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    let errors = result.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| d.message.clone()).collect();
    let stmts = match result.program.expect("no program").procedure.body {
        ProcedureBody::Paragraphs(mut p) => p.pop().map(|p| p.stmts).unwrap_or_default(),
        ProcedureBody::Sections(s) => s.into_iter().flat_map(|s| s.paragraphs).flat_map(|p| p.stmts).collect(),
    };
    Parsed { stmts, errors }
}

fn the_call(p: &Parsed) -> &Stmt {
    p.stmts.iter().find(|s| matches!(s, Stmt::Call { .. })).expect("no CALL parsed")
}

fn convention(s: &Stmt) -> CallConvention {
    match s {
        Stmt::Call { convention, .. } => *convention,
        _ => panic!("not a CALL"),
    }
}

#[test]
fn the_phrase_makes_a_websocket_call_with_every_clause_kept() {
    let p = parse_body(
        "CALL \"GET-CUSTOMER\" WITH WEBSOCKET LINKAGE USING BY VALUE CUST BY REFERENCE RES RETURNING RES\n\
         ON EXCEPTION DISPLAY \"no\"\nNOT ON EXCEPTION DISPLAY \"yes\"\nEND-CALL.",
    );
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let Stmt::Call { program, using, returning, on_exception, not_on_exception, convention, .. } = the_call(&p) else { unreachable!() };
    assert_eq!(*convention, CallConvention::WebSocket);
    assert!(matches!(program, Expr::Literal(Literal::String(s), _) if s == "GET-CUSTOMER"));
    assert_eq!(using.len(), 2);
    assert!(matches!(using[0], CallArg::ByValue(_)));
    assert!(matches!(using[1], CallArg::ByReference(_)));
    assert!(returning.is_some());
    assert_eq!(on_exception.len(), 1);
    assert_eq!(not_on_exception.len(), 1);
}

#[test]
fn the_target_may_be_an_identifier() {
    let p = parse_body("CALL PROG-NAME WITH WEBSOCKET LINKAGE USING CUST.");
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let Stmt::Call { program, convention, .. } = the_call(&p) else { unreachable!() };
    assert_eq!(*convention, CallConvention::WebSocket);
    assert!(matches!(program, Expr::Identifier(n, _) if n == "PROG-NAME"));
}

#[test]
fn by_is_optional_and_the_mode_is_sticky_and_defaults_to_by_reference() {
    let p = parse_body("CALL \"X\" WITH WEBSOCKET LINKAGE USING CUST RES VALUE CUST.");
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let Stmt::Call { using, .. } = the_call(&p) else { unreachable!() };
    assert!(matches!(using[0], CallArg::ByReference(_)), "no BY phrase is BY REFERENCE");
    assert!(matches!(using[1], CallArg::ByReference(_)));
    assert!(matches!(using[2], CallArg::ByValue(_)), "BY is optional before VALUE");
}

#[test]
fn using_is_optional() {
    for body in ["CALL \"PING\" WITH WEBSOCKET LINKAGE.", "CALL \"PING\" WITH WEBSOCKET LINKAGE RETURNING RES."] {
        let p = parse_body(body);
        assert!(p.errors.is_empty(), "{body}: {:?}", p.errors);
        let Stmt::Call { using, convention, .. } = the_call(&p) else { unreachable!() };
        assert_eq!(*convention, CallConvention::WebSocket);
        assert!(using.is_empty());
    }
}

#[test]
fn the_exception_phrases_without_on_and_with_overflow_work_as_for_any_call() {
    let p = parse_body("CALL \"X\" WITH WEBSOCKET LINKAGE USING CUST EXCEPTION DISPLAY \"e\" NOT EXCEPTION DISPLAY \"n\" END-CALL.");
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let Stmt::Call { on_exception, not_on_exception, using, .. } = the_call(&p) else { unreachable!() };
    assert_eq!(using.len(), 1, "EXCEPTION ends the argument list");
    assert_eq!(on_exception.len(), 1);
    assert_eq!(not_on_exception.len(), 1);
}

#[test]
fn by_content_is_refused_in_a_websocket_call_and_named() {
    let p = parse_body("CALL \"X\" WITH WEBSOCKET LINKAGE USING BY CONTENT CUST.");
    assert_eq!(p.errors.len(), 1, "{:?}", p.errors);
    assert!(p.errors[0].contains("BY CONTENT") && p.errors[0].contains("BY VALUE"), "{}", p.errors[0]);
}

#[test]
fn websocket_without_linkage_is_named() {
    let p = parse_body("CALL \"X\" WITH WEBSOCKET USING CUST.");
    assert!(p.errors.iter().any(|e| e.contains("LINKAGE")), "{:?}", p.errors);
}

#[test]
fn every_existing_call_form_is_still_a_local_call() {
    for body in [
        "CALL \"SUB\".",
        "CALL \"SUB\" USING CUST.",
        "CALL \"SUB\" USING BY CONTENT CUST BY REFERENCE RES RETURNING RES.",
        "CALL PROG-NAME USING CUST ON EXCEPTION DISPLAY \"x\" END-CALL.",
        "CALL \"SUB\" USING CUST ON OVERFLOW DISPLAY \"x\" NOT ON OVERFLOW DISPLAY \"y\" END-CALL.",
        "COBOL::\"HTTP-GET\" ( PROG-NAME PROG-NAME PROG-NAME ).",
    ] {
        let p = parse_body(body);
        assert!(p.errors.is_empty(), "{body}: {:?}", p.errors);
        assert_eq!(convention(the_call(&p)), CallConvention::Local, "{body}");
    }
}

#[test]
fn websocket_remains_an_ordinary_data_name() {
    // `WEBSOCKET` is a word only after `CALL … WITH`; a data item may carry the name.
    let p = parse_body("MOVE 1 TO WEBSOCKET\nCALL \"SUB\" USING WEBSOCKET.");
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    assert_eq!(convention(the_call(&p)), CallConvention::Local);
}

#[test]
fn the_phrase_is_case_insensitive_and_the_statement_after_it_is_not_swallowed() {
    let p = parse_body("call \"x\" with websocket linkage using cust\nDISPLAY \"after\".");
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    assert_eq!(convention(the_call(&p)), CallConvention::WebSocket);
    assert!(p.stmts.iter().any(|s| matches!(s, Stmt::Display { .. })), "the DISPLAY after the CALL is its own statement");
}
