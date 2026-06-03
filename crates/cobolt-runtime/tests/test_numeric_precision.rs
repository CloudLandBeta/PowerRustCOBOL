// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Integration tests for fixed-point decimal arithmetic (exact 18-digit and
//! 31-digit semantics, ROUNDED, and ON SIZE ERROR).
//!
//! The flagship case is `numprec.cbl` — the project's COBOL-85 numeric
//! precision suite at `tests/cobol/numprec.cbl` — which is executed end-to-end
//! and asserted to report `RESULT : PASS`. The remaining tests pin individual
//! behaviours by capturing `DISPLAY` output.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Tokenize, parse (asserting no errors), run, and return captured DISPLAY lines.
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
    // The interpreter sends DISPLAY lines synchronously during run(); collect them.
    display_rx.try_iter().collect()
}

/// Wrap PROCEDURE statements in a minimal program with the given WORKING-STORAGE.
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

// ── The COBOL-85 NUMPREC suite ─────────────────────────────────────────────────

#[test]
fn numprec_suite_reports_pass() {
    let src = include_str!("../../../tests/cobol/numprec.cbl");
    let out = run_capture(src).join("\n");
    assert!(
        out.contains("RESULT       : PASS"),
        "NUMPREC suite did not pass:\n{out}"
    );
    assert!(
        !out.contains("FAIL T0"),
        "NUMPREC reported individual failures:\n{out}"
    );
    // All ten cases must have run and passed.
    assert_eq!(out.matches("PASS T0").count(), 10, "expected 10 PASS lines:\n{out}");
}

// ── Focused behaviours ─────────────────────────────────────────────────────────

#[test]
fn exact_decimal_addition_no_float_drift() {
    let out = run_capture(&program(
        "       01 A PIC 9(5)V99 VALUE ZERO.\n\
         \x20      01 B PIC 9(5)V99 VALUE ZERO.\n\
         \x20      01 C PIC 9(5)V99 VALUE ZERO.",
        "           MOVE 0.10 TO A\n\
         \x20          MOVE 0.20 TO B\n\
         \x20          COMPUTE C = A + B\n\
         \x20          DISPLAY C",
    ));
    assert_eq!(out, vec!["0.30"], "0.10 + 0.20 must be exactly 0.30");
}

#[test]
fn eighteen_digit_integer_is_exact() {
    // f64 cannot represent 123456789012345678; the i128 fixed-point can.
    let out = run_capture(&program(
        "       01 A PIC 9(18) VALUE 123456789012345678.\n\
         \x20      01 C PIC 9(18) VALUE ZERO.",
        "           ADD 1 TO A GIVING C\n\
         \x20          DISPLAY C",
    ));
    assert_eq!(out, vec!["123456789012345679"]);
}

#[test]
fn thirty_one_digit_addition_is_exact() {
    let out = run_capture(&program(
        "       01 A PIC S9(18)V9(13) VALUE ZERO.\n\
         \x20      01 B PIC S9(18)V9(13) VALUE ZERO.\n\
         \x20      01 C PIC S9(18)V9(13) VALUE ZERO.",
        "           MOVE 123456789012345678.1234567890123 TO A\n\
         \x20          MOVE 1.0000000000001 TO B\n\
         \x20          COMPUTE C = A + B\n\
         \x20          DISPLAY C",
    ));
    assert_eq!(out, vec!["123456789012345679.1234567890124"]);
}

#[test]
fn thirty_one_digit_multiply_uses_256bit_intermediate() {
    // The intermediate product would overflow i128 at scale 26; the 256-bit
    // path reduces scale so the field-level result stays exact.
    let out = run_capture(&program(
        "       01 A PIC S9(18)V9(13) VALUE ZERO.\n\
         \x20      01 B PIC S9(18)V9(13) VALUE ZERO.\n\
         \x20      01 C PIC S9(18)V9(13) VALUE ZERO.",
        "           MOVE 1000000000000.0000000000001 TO A\n\
         \x20          MOVE 2.0000000000000 TO B\n\
         \x20          COMPUTE C = A * B\n\
         \x20          DISPLAY C",
    ));
    assert_eq!(out, vec!["2000000000000.0000000000002"]);
}

#[test]
fn rounded_division_rounds_half_away_from_zero() {
    let out = run_capture(&program(
        "       01 C PIC 9V999 VALUE ZERO.",
        // 2 / 3 = 0.6666… → rounded to 3 places = 0.667
        "           COMPUTE C ROUNDED = 2 / 3\n\
         \x20          DISPLAY C",
    ));
    assert_eq!(out, vec!["0.667"]);
}

#[test]
fn truncated_division_truncates() {
    let out = run_capture(&program(
        "       01 C PIC 9V999 VALUE ZERO.",
        // Without ROUNDED, 2 / 3 truncates to 0.666.
        "           COMPUTE C = 2 / 3\n\
         \x20          DISPLAY C",
    ));
    assert_eq!(out, vec!["0.666"]);
}

#[test]
fn add_to_giving_sums_all_operands() {
    let out = run_capture(&program(
        "       01 A PIC 9(3) VALUE 100.\n\
         \x20      01 B PIC 9(3) VALUE 50.\n\
         \x20      01 C PIC 9(3) VALUE ZERO.",
        // `ADD A TO B GIVING C` → C = A + B = 150.
        "           ADD A TO B GIVING C\n\
         \x20          DISPLAY C",
    ));
    assert_eq!(out, vec!["150"]);
}

#[test]
fn on_size_error_fires_and_leaves_field_unchanged() {
    let out = run_capture(&program(
        "       01 B PIC 9(3) VALUE 200.\n\
         \x20      01 C PIC 9(3) VALUE 7.\n\
         \x20      01 F PIC X VALUE \"N\".",
        // 900 + 200 = 1100 → exceeds PIC 9(3); ON SIZE ERROR fires, C unchanged.
        "           ADD 900 TO B GIVING C\n\
         \x20              ON SIZE ERROR MOVE \"Y\" TO F\n\
         \x20          END-ADD\n\
         \x20          DISPLAY F\n\
         \x20          DISPLAY C",
    ));
    assert_eq!(out, vec!["Y".to_string(), "7".to_string()]);
}

#[test]
fn not_on_size_error_fires_when_within_range() {
    let out = run_capture(&program(
        "       01 B PIC 9(3) VALUE 50.\n\
         \x20      01 C PIC 9(3) VALUE ZERO.\n\
         \x20      01 F PIC X VALUE \"-\".",
        // 100 + 50 = 150 fits PIC 9(3); the NOT branch runs, C is updated.
        "           ADD 100 TO B GIVING C\n\
         \x20              ON SIZE ERROR MOVE \"Y\" TO F\n\
         \x20              NOT ON SIZE ERROR MOVE \"N\" TO F\n\
         \x20          END-ADD\n\
         \x20          DISPLAY F\n\
         \x20          DISPLAY C",
    ));
    assert_eq!(out, vec!["N".to_string(), "150".to_string()]);
}
