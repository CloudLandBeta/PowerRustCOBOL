// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `SPECIAL-NAMES. DECIMAL-POINT IS COMMA.` and comma decimal literals.
//!
//! In much of the world — Brazil among them — the comma IS the decimal point,
//! and COBOL-85 supports that through `DECIMAL-POINT IS COMMA`. What must not
//! happen is the middle case: a comma decimal written WITHOUT the clause. It is
//! then neither a numeric literal (the decimal point is `.`) nor a separator
//! comma (COBOL-85 requires a space after one), and taking the integer part in
//! silence gives the item a wrong value that nothing reports.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};

fn program(special_names: &str, value: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. TESTPROG.\n\
         ENVIRONMENT DIVISION.\n\
         {special_names}\
         DATA DIVISION.\n\
         WORKING-STORAGE SECTION.\n\
         01  WS-PRICE  PIC 9(5)V99 VALUE {value}.\n\
         PROCEDURE DIVISION.\n\
         MAIN.\n\
             STOP RUN.\n"
    )
}

fn errors(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.message.clone())
        .collect()
}

const CLAUSE: &str = "CONFIGURATION SECTION.\nSPECIAL-NAMES.\n    DECIMAL-POINT IS COMMA.\n";

#[test]
fn a_comma_decimal_without_the_clause_is_an_error_that_names_the_cause() {
    let errs = errors(&program("", "8,49"));
    assert_eq!(errs.len(), 1, "expected exactly one error, got {errs:?}");
    let msg = &errs[0];
    // The developer needs three things: what was read, why it is wrong, and
    // WHERE the fix goes — the outermost program, which in the RAD case is the
    // form and never a nested handler.
    assert!(msg.contains("8,49"), "must quote the literal: {msg}");
    assert!(
        msg.contains("DECIMAL-POINT IS COMMA"),
        "must name the clause: {msg}"
    );
    assert!(
        msg.contains("OUTERMOST"),
        "must say where the clause belongs: {msg}"
    );
}

#[test]
fn the_same_literal_is_accepted_once_the_clause_is_declared() {
    assert!(
        errors(&program(CLAUSE, "8,49")).is_empty(),
        "with DECIMAL-POINT IS COMMA the literal is a decimal"
    );
}

#[test]
fn a_dot_decimal_is_untouched_either_way() {
    assert!(errors(&program("", "8.49")).is_empty());
}

/// Only the SPACE-LESS form is diagnosed. A comma followed by a space is a
/// separator, never a decimal point, so the new diagnostic must stay silent on
/// it — whatever else the parser may think of the surrounding statement.
#[test]
fn a_comma_followed_by_a_space_never_raises_the_decimal_diagnostic() {
    for value in ["8, 49", "8 , 49"] {
        let errs = errors(&program("", value));
        assert!(
            !errs.iter().any(|e| e.contains("DECIMAL-POINT IS COMMA")),
            "a separator comma must not be read as a decimal point: {errs:?}"
        );
    }
}
