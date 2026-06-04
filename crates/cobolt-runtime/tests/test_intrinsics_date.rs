// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Date/day-conversion and financial intrinsic functions:
//! INTEGER-OF-DATE, DATE-OF-INTEGER, INTEGER-OF-DAY, DAY-OF-INTEGER, ANNUITY.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
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
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

#[test]
fn integer_of_date_base_is_one() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 N PIC 9(7) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           COMPUTE N = FUNCTION INTEGER-OF-DATE(16010101)
           DISPLAY N
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["0000001"]);
}

#[test]
fn date_integer_round_trips() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DT2.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 N PIC 9(7) VALUE 0.
       01 D PIC 9(8) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           COMPUTE N = FUNCTION INTEGER-OF-DATE(20240229)
           COMPUTE D = FUNCTION DATE-OF-INTEGER(N)
           DISPLAY D
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["20240229"]);
}

#[test]
fn present_value_and_byte_length() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PV.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 R PIC 9(5)V99 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           COMPUTE R = FUNCTION PRESENT-VALUE(0.10, 100, 100, 100)
           DISPLAY R
           DISPLAY FUNCTION BYTE-LENGTH("HELLO")
           STOP RUN.
    "#;
    // 100/1.1 + 100/1.21 + 100/1.331 ≈ 248.685 → 248.69; BYTE-LENGTH = 5.
    assert_eq!(run_capture(src), vec!["0024869", "5"]);
}

#[test]
fn annuity_zero_rate_is_reciprocal_of_periods() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ANN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 R PIC 9V9(4) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           COMPUTE R = FUNCTION ANNUITY(0, 4)
           DISPLAY R
           STOP RUN.
    "#;
    // rate 0 → 1/periods = 0.25; DISPLAY of 9V9(4) shows raw digits "02500".
    assert_eq!(run_capture(src), vec!["02500"]);
}
