// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Leading-decimal-point literals — the **values**, not the parse.
//!
//! A parser test cannot settle this one. `77 A PICTURE SV9(5) VALUE .11111.`
//! produced no diagnostic before the feature existed either; it simply stored
//! the wrong number, in silence. So the assertion has to be made against what
//! the program prints when it runs.
//!
//! The case that matters most is scale. `.00001` and `.1` carry the same digit
//! value once parsed — only the count of digits written tells them apart — so
//! every check below compares against the same number written the ordinary way,
//! with an explicit leading zero.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Tokenize, parse (asserting no errors), run, and return captured DISPLAY lines.
fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let errs: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errs.is_empty(), "parse errors: {errs:#?}");
    let program = result.program.expect("no program");

    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().collect()
}

fn program(working_storage: &str, procedure: &str) -> String {
    format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. T.\n\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n\
         {working_storage}\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         {procedure}\n\
         \x20          STOP RUN.\n"
    )
}

// ── The COBOL-85 LEADDOT suite ────────────────────────────────────────────────

/// The suite at `tests/cobol/numeric/leading-decimal-point.cbl`, end to end.
/// It reports its own quantified summary (GOLDEN RULE #7); this asserts it.
#[test]
fn leaddot_suite_reports_pass() {
    let src = include_str!("../../../tests/cobol/numeric/leading-decimal-point.cbl");
    let out = run_capture(src).join("\n");
    assert!(
        out.contains("RESULT       : PASS"),
        "LEADDOT suite did not pass:\n{out}"
    );
    assert!(
        !out.contains("FAIL T0"),
        "LEADDOT reported individual failures:\n{out}"
    );
    assert_eq!(
        out.matches("PASS T0").count(),
        11,
        "expected all 11 cases to run and pass:\n{out}"
    );
}

// ── Individual values ─────────────────────────────────────────────────────────

/// AC1 — `.00001` keeps its four leading zeros. If the scale were taken from
/// the parsed token value instead of the digits written, this would store `.1`
/// and the field would print `10000` rather than `00001`.
#[test]
fn leading_zeros_are_not_lost() {
    let out = run_capture(&program(
        "       77  WS-NUM PICTURE SV9(5).",
        "           MOVE .00001 TO WS-NUM.\n\
         \x20          DISPLAY \"V=\" WS-NUM.",
    ));
    assert_eq!(out, vec!["V=00001".to_string()], "{out:?}");
}

/// AC3/AC8 — nine fractional digits survive exactly, with no rounding.
#[test]
fn nine_fractional_digits_are_exact() {
    let out = run_capture(&program(
        "       77  WS-NUM PICTURE SV9(9).",
        "           MOVE .000000001 TO WS-NUM.\n\
         \x20          DISPLAY \"V=\" WS-NUM.",
    ));
    assert_eq!(out, vec!["V=000000001".to_string()], "{out:?}");
}

/// The scale must survive arithmetic, not merely storage:
/// `.000000001 * 1000000000` is exactly 1.
#[test]
fn scale_survives_arithmetic() {
    let out = run_capture(&program(
        "       77  WS-NUM PICTURE S9(9)V9(9).",
        "           COMPUTE WS-NUM = .000000001 * 1000000000.\n\
         \x20          DISPLAY \"V=\" WS-NUM.",
    ));
    assert_eq!(out, vec!["V=000000001000000000".to_string()], "{out:?}");
}

/// R2 — a signed leading-point literal in a `VALUE` clause.
#[test]
fn signed_leading_point_value() {
    let out = run_capture(&program(
        "       77  A PICTURE S9V9 VALUE -.5.\n\
         \x20      77  B PICTURE S9V9 VALUE +.5.",
        "           DISPLAY \"A=\" A.\n\x20          DISPLAY \"B=\" B.",
    ));
    assert_eq!(out, vec!["A=-05".to_string(), "B=05".to_string()], "{out:?}");
}

/// The literal is equal to the same number written with a leading zero — the
/// direct statement of what "correct" means here.
#[test]
fn leading_point_equals_explicit_zero_form() {
    let out = run_capture(&program(
        "       77  A PICTURE SV9(5) VALUE .11111.\n\
         \x20      77  B PICTURE SV9(5) VALUE 0.11111.",
        "           IF A = B DISPLAY \"SAME\" ELSE DISPLAY \"DIFFERENT\" END-IF.",
    ));
    assert_eq!(out, vec!["SAME".to_string()], "{out:?}");
}
