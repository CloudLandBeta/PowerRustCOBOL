// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Literal-object abbreviated conditions (`A = 1 OR 2 OR 3`), EVALUATE with
//! ALSO (multi-subject AND), and `WHEN NOT value`.

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
fn literal_object_abbreviation() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. AB.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A PIC 9 VALUE 3.
       PROCEDURE DIVISION.
       MAIN.
           IF A = 1 OR 2 OR 3
               DISPLAY "Y"
           ELSE
               DISPLAY "N"
           END-IF
           IF A = 1 OR 2
               DISPLAY "Y2"
           ELSE
               DISPLAY "N2"
           END-IF
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["Y", "N2"]);
}

#[test]
fn evaluate_also_multi_subject() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EVA.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A PIC 9 VALUE 2.
       01 B PIC 9 VALUE 7.
       PROCEDURE DIVISION.
       MAIN.
           EVALUATE A ALSO B
               WHEN 1 ALSO 7 DISPLAY "W1"
               WHEN 2 ALSO 7 DISPLAY "W2"
               WHEN OTHER     DISPLAY "WO"
           END-EVALUATE
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["W2"]);
}

#[test]
fn evaluate_when_not_value() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EVN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A PIC 9 VALUE 2.
       PROCEDURE DIVISION.
       MAIN.
           EVALUATE A
               WHEN NOT 5 DISPLAY "NOT5"
               WHEN OTHER DISPLAY "IS5"
           END-EVALUATE
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["NOT5"]);
}
