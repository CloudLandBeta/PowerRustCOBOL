// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The intrinsics that pick an argument out of a list: `MAX`, `MIN`,
//! `ORD-MAX`, `ORD-MIN`.
//!
//! Two things the obvious "read every argument as a float" implementation gets
//! wrong, and both are observable in the CCVS85 Conditional module:
//!
//! * the comparison is COBOL's own, so an alphanumeric argument list is ordered
//!   by the collating sequence rather than by a numeric reading that is zero
//!   for every one of them;
//! * the **first** of several equal arguments wins, which matters to `ORD-MAX`
//!   and `ORD-MIN` because they return a position.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Run a program body with `A`…`D` and a couple of alphanumeric items in scope,
/// and return what it displayed.
fn run(body: &str) -> Vec<String> {
    let src = format!(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. T.\n\
         DATA DIVISION.\n\
         WORKING-STORAGE SECTION.\n\
         01 A      PIC S9(10) VALUE 5.\n\
         01 B      PIC S9(10) VALUE 7.\n\
         01 I      PIC X      VALUE \"R\".\n\
         01 J      PIC X      VALUE \"U\".\n\
         01 WS-NUM PIC S9(9)V9(4).\n\
         01 WS-INT PIC S9(4).\n\
         01 WS-CHR PIC X.\n\
         PROCEDURE DIVISION.\n\
         MAIN.\n\
         {body}\n\
         \x20   STOP RUN.\n"
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
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

/// An all-alphanumeric argument list is ordered by the collating sequence, and
/// the result is the argument itself — a character, not a number.
#[test]
fn max_and_min_compare_alphanumeric_arguments_as_characters() {
    let out = run(
        "    MOVE FUNCTION MAX(\"R\", I, \"I\", \"a\") TO WS-CHR.\n\
         \x20   DISPLAY \"MAX \" WS-CHR.\n\
         \x20   MOVE FUNCTION MIN(\"R\", I, \"I\", \"a\") TO WS-CHR.\n\
         \x20   DISPLAY \"MIN \" WS-CHR.",
    );
    // Lower case sorts above upper case in the native sequence.
    assert_eq!(out, vec!["MAX a", "MIN I"]);
}

/// A numeric argument list still answers with a number.
#[test]
fn max_and_min_still_answer_numerically() {
    let out = run(
        "    COMPUTE WS-NUM = FUNCTION MAX(5, 6, 10, 3, 7).\n\
         \x20   DISPLAY \"MAX \" WS-NUM.\n\
         \x20   COMPUTE WS-NUM = FUNCTION MIN(-4.3, 10.2, -0.7, 3.9).\n\
         \x20   DISPLAY \"MIN \" WS-NUM.",
    );
    assert_eq!(out, vec!["MAX 0000000100000", "MIN -0000000043000"]);
}

/// `ORD-MAX` and `ORD-MIN` return the position of the **first** of several
/// equal arguments.
#[test]
fn ord_max_and_ord_min_keep_the_first_of_a_tie() {
    let out = run(
        "    COMPUTE WS-INT = FUNCTION ORD-MAX(A, 5, 5, A).\n\
         \x20   DISPLAY \"OMAX \" WS-INT.\n\
         \x20   COMPUTE WS-INT = FUNCTION ORD-MIN(A, 5, 5, A).\n\
         \x20   DISPLAY \"OMIN \" WS-INT.\n\
         \x20   COMPUTE WS-INT = FUNCTION ORD-MAX(1, 1).\n\
         \x20   DISPLAY \"TIE  \" WS-INT.",
    );
    assert_eq!(out, vec!["OMAX 0001", "OMIN 0001", "TIE  0001"]);
}

/// …and they order alphanumeric arguments by the collating sequence too.
#[test]
fn ord_max_orders_alphanumeric_arguments() {
    let out = run(
        "    COMPUTE WS-INT = FUNCTION ORD-MAX(\"A\", I, \"P\").\n\
         \x20   DISPLAY \"OMAX \" WS-INT.\n\
         \x20   COMPUTE WS-INT = FUNCTION ORD-MIN(\"S\", \"D\", J).\n\
         \x20   DISPLAY \"OMIN \" WS-INT.",
    );
    // I is "R", the greatest of A/R/P; D is the least of S/D/U.
    assert_eq!(out, vec!["OMAX 0002", "OMIN 0002"]);
}

/// The ordinary numeric case is unchanged.
#[test]
fn ord_max_still_finds_a_numeric_maximum() {
    let out = run(
        "    COMPUTE WS-INT = FUNCTION ORD-MAX(5, 3, 2, 8, 3, 1).\n\
         \x20   DISPLAY \"OMAX \" WS-INT.\n\
         \x20   COMPUTE WS-INT = FUNCTION ORD-MIN(3, 2, 7, 1, 5).\n\
         \x20   DISPLAY \"OMIN \" WS-INT.",
    );
    assert_eq!(out, vec!["OMAX 0004", "OMIN 0004"]);
}

/// A quotient whose divisor carries many decimals keeps its value, end to end.
///
/// `1 / SQRT3` used to come out as 1, so `FUNCTION ATAN(1 / SQRT3)` answered
/// atan(1) — a quarter turn instead of a sixth of one.
#[test]
fn a_quotient_by_a_long_decimal_divisor_keeps_its_value() {
    let src = "IDENTIFICATION DIVISION.\n\
               PROGRAM-ID. T.\n\
               DATA DIVISION.\n\
               WORKING-STORAGE SECTION.\n\
               01 SQRT3  PIC S9V9(17) VALUE 1.732050808.\n\
               01 WS-NUM PIC S9(5)V9(9).\n\
               PROCEDURE DIVISION.\n\
               MAIN.\n\
                   COMPUTE WS-NUM = 1 / SQRT3.\n\
                   DISPLAY \"Q \" WS-NUM.\n\
                   COMPUTE WS-NUM = FUNCTION ATAN(1 / SQRT3).\n\
                   DISPLAY \"A \" WS-NUM.\n\
                   STOP RUN.\n";
    let result = parse(tokenize(src, SourceFormat::Free));
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    let out: Vec<String> = display_rx.try_iter().collect();
    assert_eq!(out[0], "Q 00000577350269");
    // atan(0.5773502) = 0.5235987… — a sixth of a turn, not a quarter.
    assert_eq!(out[1], "A 00000523598775");
}
