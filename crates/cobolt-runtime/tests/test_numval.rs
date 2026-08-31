// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `FUNCTION NUMVAL` and `FUNCTION NUMVAL-C` — reading a number out of a
//! character string.
//!
//! The argument is not a Rust float literal, and reading it as one returned
//! **zero** for most of the forms COBOL-85 allows. The sign may sit at either
//! end and need not touch the digits; `CR` and `DB` are the credit-debit
//! spelling of a trailing minus; NUMVAL-C additionally allows a currency string
//! and digit-group separators. IF125A and IF126A between them write every one
//! of these.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Evaluate one `FUNCTION` expression and return what it displayed.
fn eval(expr: &str) -> String {
    let src = format!(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. T.\n\
         DATA DIVISION.\n\
         WORKING-STORAGE SECTION.\n\
         01 WS-NUM PIC S9(9)V9(9).\n\
         PROCEDURE DIVISION.\n\
         MAIN.\n\
         \x20   COMPUTE WS-NUM = {expr}.\n\
         \x20   DISPLAY WS-NUM.\n\
         \x20   STOP RUN.\n"
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors in `{expr}`: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().collect::<Vec<_>>().join("")
}

/// `S9(9)V9(9)` displays as 18 digits with a leading sign when negative, so
/// this turns an expected number into the string the program prints.
fn shown(units: i64, frac_billionths: i64) -> String {
    let neg = units < 0 || frac_billionths < 0;
    let body = format!("{:09}{:09}", units.abs(), frac_billionths.abs());
    if neg {
        format!("-{body}")
    } else {
        body
    }
}

/// The plain forms: an integer, a decimal, and a value written with no digits
/// before the point.
#[test]
fn numval_reads_the_plain_forms() {
    assert_eq!(eval("FUNCTION NUMVAL (\"9\")"), shown(9, 0));
    assert_eq!(eval("FUNCTION NUMVAL (\"4738\")"), shown(4738, 0));
    assert_eq!(eval("FUNCTION NUMVAL (\".935\")"), shown(0, 935_000_000));
    assert_eq!(eval("FUNCTION NUMVAL (\"385.93\")"), shown(385, 930_000_000));
}

/// A **leading** sign, with or without space between it and the digits.
#[test]
fn numval_reads_a_leading_sign() {
    assert_eq!(eval("FUNCTION NUMVAL (\"+394.2\")"), shown(394, 200_000_000));
    assert_eq!(
        eval("FUNCTION NUMVAL (\"   -  4929.0323\")"),
        shown(-4929, -32_300_000)
    );
}

/// A **trailing** sign — the form that reads as zero if the argument is handed
/// straight to a float parser.
#[test]
fn numval_reads_a_trailing_sign() {
    assert_eq!(
        eval("FUNCTION NUMVAL (\"82.9312+\")"),
        shown(82, 931_200_000)
    );
    assert_eq!(
        eval("FUNCTION NUMVAL (\"   200.0002   - \")"),
        shown(-200, -200_000)
    );
    assert_eq!(eval("FUNCTION NUMVAL (\" 92.92  -\")"), shown(-92, -920_000_000));
}

/// `CR` and `DB` are the credit-debit spelling of a trailing minus.
#[test]
fn numval_reads_cr_and_db_as_negative() {
    assert_eq!(eval("FUNCTION NUMVAL (\"123.45CR\")"), shown(-123, -450_000_000));
    assert_eq!(eval("FUNCTION NUMVAL (\"123.45 DB\")"), shown(-123, -450_000_000));
}

/// NUMVAL-C ignores digit-group separators, and the currency string — which
/// defaults to the CURRENCY SIGN when the second argument is omitted.
#[test]
fn numval_c_ignores_groups_and_the_default_currency() {
    assert_eq!(eval("FUNCTION NUMVAL-C (\"92,483\")"), shown(92483, 0));
    assert_eq!(eval("FUNCTION NUMVAL-C (\"$5\")"), shown(5, 0));
}

/// …and the currency string may be given explicitly, before or after the sign.
#[test]
fn numval_c_takes_an_explicit_currency_string() {
    assert_eq!(
        eval("FUNCTION NUMVAL-C (\"$93,021\", \"$\")"),
        shown(93021, 0)
    );
    assert_eq!(
        eval("FUNCTION NUMVAL-C (\"-$34.03\", \"$\")"),
        shown(-34, -30_000_000)
    );
    assert_eq!(
        eval("FUNCTION NUMVAL-C (\"- $ 890.21\", \"$\")"),
        shown(-890, -210_000_000)
    );
    assert_eq!(
        eval("FUNCTION NUMVAL-C (\"  $  90.54 -  \", \"$\")"),
        shown(-90, -540_000_000)
    );
}

/// The result is a number like any other, so it takes part in arithmetic.
#[test]
fn a_numval_result_is_an_ordinary_operand() {
    assert_eq!(eval("FUNCTION NUMVAL (\"90\") + 10"), shown(100, 0));
    assert_eq!(
        eval("FUNCTION NUMVAL-C (\"2\") + FUNCTION NUMVAL-C (\"8\")"),
        shown(10, 0)
    );
}
