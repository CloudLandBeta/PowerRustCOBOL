// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `ON` is optional before a CALL's `OVERFLOW` / `EXCEPTION` phrase.
//!
//! Motivated by CCVS85 **IC201A**, which writes both spellings in one
//! paragraph: CALL-TEST-03-01 with the `ON`, CALL-TEST-03-02 without it. Only
//! the second failed, and it failed in the worst available way — a *successful*
//! call ran its own overflow handler and reported "OVERFLOW SHOULD NOT OCCUR".
//!
//! The argument list was the culprit, not the phrase. `USING` collects operands
//! while `is_expr_start` says the next token can begin one, and a bare
//! `OVERFLOW` is just a word, so the list swallowed it and what followed became
//! ordinary statements after the CALL. `ON OVERFLOW` never had the problem:
//! `Token::On` is not an expression start, so the list stopped by itself.
//!
//! Neither word can be a data-name — both are reserved — so stopping on them
//! costs nothing.

use cobolt_ast::program::ProcedureBody;
use cobolt_ast::stmt::Stmt;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;

/// The statements of the one paragraph in a minimal program.
fn stmts_of(body: &str) -> Vec<Stmt> {
    let src = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
         DATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 A PIC X(4).\n01 B PIC X(4).\n\
         PROCEDURE DIVISION.\nMAIN.\n{body}\n"
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    match result.program.unwrap().procedure.body {
        ProcedureBody::Paragraphs(mut paras) => paras.pop().map(|p| p.stmts).unwrap_or_default(),
        ProcedureBody::Sections(secs) => secs
            .into_iter()
            .flat_map(|s| s.paragraphs)
            .flat_map(|p| p.stmts)
            .collect(),
    }
}

/// The CALL's argument list and its `on_exception` body, for whichever spelling.
fn call_shape(body: &str) -> (usize, usize) {
    let stmts = stmts_of(body);
    let call = stmts
        .iter()
        .find_map(|s| match s {
            Stmt::Call {
                using,
                on_exception,
                ..
            } => Some((using.len(), on_exception.len())),
            _ => None,
        })
        .expect("the fixture contains a CALL");
    call
}

/// The two spellings must parse to the same thing: one argument, and the
/// handler held by the CALL rather than trailing after it.
#[test]
fn the_optional_on_does_not_change_the_parse() {
    let with_on = call_shape("    CALL \"SUB\" USING A ON OVERFLOW MOVE \"X\" TO B END-CALL.");
    let without = call_shape("    CALL \"SUB\" USING A OVERFLOW MOVE \"X\" TO B END-CALL.");
    assert_eq!(
        with_on, without,
        "`ON` is optional, so both spellings must yield the same (arg count, \
         handler length); got {with_on:?} with the ON and {without:?} without it"
    );
    assert_eq!(with_on.0, 1, "one argument — OVERFLOW is not an operand");
    assert_eq!(with_on.1, 1, "the handler belongs to the CALL");
}

/// The same for `EXCEPTION`, whose `ON` is optional in the same way and which
/// had the identical hole.
#[test]
fn a_bare_exception_phrase_is_not_an_argument() {
    let (args, handler) =
        call_shape("    CALL \"SUB\" USING A EXCEPTION MOVE \"X\" TO B END-CALL.");
    assert_eq!(args, 1, "EXCEPTION introduces the phrase, not an operand");
    assert_eq!(handler, 1, "and its body belongs to the CALL");
}

/// A CCVS-shaped separator semicolon between the arguments and the phrase is
/// what made IC201A's line look unlike the textbook one; it must not matter.
#[test]
fn a_separator_semicolon_before_the_phrase_is_ignored() {
    let (args, handler) =
        call_shape("    CALL \"SUB\" USING A; OVERFLOW MOVE \"X\" TO B END-CALL.");
    assert_eq!(args, 1);
    assert_eq!(handler, 1);
}
