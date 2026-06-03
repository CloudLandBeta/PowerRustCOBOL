// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Integration tests for numeric-edited PICTUREs (the `numedit` edit engine):
//! `Z`/`*` suppression, fixed/floating `$`, floating sign, `,`/`.` insertion,
//! and `CR`/`DB`. The flagship case runs `tests/cobol/numedit.cbl` end-to-end.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let tokens = tokenize(src, SourceFormat::Free);
    let result = parse(tokens);
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().collect()
}

/// Move `value` into a field of `pic` and return the edited DISPLAY string.
fn edit(pic: &str, value_decl: &str, mover: &str) -> String {
    let src = format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. T.\n\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n\
         {value_decl}\n\
         \x20      01 E PIC {pic}.\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         {mover}\n\
         \x20          DISPLAY \"[\" E \"]\"\n\
         \x20          STOP RUN.\n"
    );
    let out = run_capture(&src);
    out.into_iter().next().unwrap_or_default()
}

#[test]
fn numedit_suite_reports_pass() {
    let src = include_str!("../../../tests/cobol/numedit.cbl");
    let out = run_capture(src).join("\n");
    assert!(out.contains("RESULT       : PASS"), "numedit suite failed:\n{out}");
    assert_eq!(out.matches("PASS T0").count(), 11, "expected 11 PASS lines:\n{out}");
}

#[test]
fn zero_suppression_and_comma() {
    assert_eq!(
        edit("ZZZ,ZZ9.99", "       01 S PIC 9(6)V99 VALUE 1234.50.", "           MOVE S TO E"),
        "[  1,234.50]"
    );
}

#[test]
fn floating_dollar() {
    assert_eq!(
        edit("$$$,$$9.99", "       01 S PIC 9(6)V99 VALUE 1234.50.", "           MOVE S TO E"),
        "[ $1,234.50]"
    );
}

#[test]
fn check_protection() {
    assert_eq!(
        edit("***,**9.99", "       01 S PIC 9(6)V99 VALUE 12.34.", "           MOVE S TO E"),
        "[*****12.34]"
    );
}

#[test]
fn floating_sign_negative_and_positive() {
    assert_eq!(
        edit("----9.99", "       01 S PIC S9(4)V99 VALUE -12.30.", "           MOVE S TO E"),
        "[  -12.30]"
    );
    assert_eq!(
        edit("----9.99", "       01 S PIC S9(4)V99 VALUE 12.30.", "           MOVE S TO E"),
        "[   12.30]"
    );
}

#[test]
fn cr_db_suffix() {
    assert_eq!(
        edit("9(6).99CR", "       01 S PIC S9(4)V99 VALUE -12.30.", "           MOVE S TO E"),
        "[000012.30CR]"
    );
    assert_eq!(
        edit("9(6).99CR", "       01 S PIC S9(4)V99 VALUE 12.30.", "           MOVE S TO E"),
        "[000012.30  ]"
    );
}

#[test]
fn edited_field_initialized_to_spaces() {
    // A numeric-edited field with no VALUE displays as spaces until moved into.
    assert_eq!(
        edit("ZZZ,ZZ9.99", "       01 S PIC 9 VALUE 0.", "           CONTINUE"),
        "[          ]"
    );
}
