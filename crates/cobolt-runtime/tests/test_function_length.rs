// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `FUNCTION LENGTH` measures the DECLARATION, not the value (BUG-002).
//!
//! COBOL-85 defines LENGTH as the number of character positions **in the
//! argument** — fixed by its PICTURE, constant at run time for a fixed-size
//! item. Before the fix the interpreter evaluated the argument first and
//! measured what came back, so `PIC 9(7)V99` answered **10** holding
//! 1234567.89 and **4** holding nothing, where both are 9.
//!
//! Alphanumerics were right only by coincidence — a `CobolValue::String` keeps
//! its bytes padded to the declared width — which is why a test written with
//! `PIC X` alone would have passed throughout and proved nothing. Every
//! expectation below was checked against `cobc (GnuCOBOL) 3.2` and matches it
//! exactly.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
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
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

/// Every case, with the width GnuCOBOL 3.2 reports for the same declaration.
#[test]
fn length_reports_declared_character_positions() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. LEN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A     PIC X(10).
       01 WS-B     PIC X(10) VALUE "AB".
       01 WS-N     PIC 9(7)V99 VALUE 1234567.89.
       01 WS-M     PIC 9(7)V99.
       01 WS-S     PIC S9(4) VALUE -123.
       01 WS-SEP   PIC S9(4) SIGN IS LEADING SEPARATE VALUE -12.
       01 WS-ED    PIC ZZZ,ZZ9.99.
       01 WS-GRP.
          03 G-A   PIC X(3).
          03 G-B   PIC 9(4).
       01 WS-TBL.
          03 T-E   PIC X(4) OCCURS 5 TIMES.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY FUNCTION LENGTH(WS-A)
           DISPLAY FUNCTION LENGTH(WS-B)
           DISPLAY FUNCTION LENGTH(WS-N)
           DISPLAY FUNCTION LENGTH(WS-M)
           DISPLAY FUNCTION LENGTH(WS-S)
           DISPLAY FUNCTION LENGTH(WS-SEP)
           DISPLAY FUNCTION LENGTH(WS-ED)
           DISPLAY FUNCTION LENGTH(WS-GRP)
           DISPLAY FUNCTION LENGTH(WS-TBL)
           DISPLAY FUNCTION LENGTH("ABC")
           STOP RUN.
    "#;

    let out = run_capture(src);
    let got: Vec<i64> = out
        .iter()
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();

    // X(10) · X(10) VALUE "AB" · 9(7)V99 valued · 9(7)V99 uninitialised ·
    // S9(4) holding -123 · S9(4) SIGN LEADING SEPARATE · ZZZ,ZZ9.99 ·
    // group 3+4 · table 5x4 · the literal "ABC".
    assert_eq!(got, vec![10, 10, 9, 9, 4, 5, 10, 7, 20, 3], "got {out:?}");
}

/// The two that regressed, isolated — a numeric item's length may not move
/// when its value does.
#[test]
fn a_numeric_items_length_does_not_follow_its_value() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. LENV.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N     PIC 9(7)V99.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY FUNCTION LENGTH(WS-N)
           MOVE 1 TO WS-N
           DISPLAY FUNCTION LENGTH(WS-N)
           MOVE 1234567.89 TO WS-N
           DISPLAY FUNCTION LENGTH(WS-N)
           STOP RUN.
    "#;

    let out = run_capture(src);
    let got: Vec<i64> = out
        .iter()
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();
    assert_eq!(got, vec![9, 9, 9], "length moved with the value: {out:?}");
}
