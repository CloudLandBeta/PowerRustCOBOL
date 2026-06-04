// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `EXIT PERFORM [CYCLE]`, `EXIT PARAGRAPH`, `EXIT SECTION`, and the
//! `NEXT SENTENCE` / plain-`EXIT` no-ops.

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
fn exit_perform_breaks_loop() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 I PIC 9(2) VALUE 0.
       01 S PIC 9(4) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 10
               IF I = 4
                   EXIT PERFORM
               END-IF
               ADD I TO S
           END-PERFORM
           DISPLAY S
           STOP RUN.
    "#;
    // 1 + 2 + 3 = 6
    assert_eq!(run_capture(src), vec!["0006"]);
}

#[test]
fn exit_perform_cycle_skips_iteration() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EPC.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 I PIC 9(2) VALUE 0.
       01 S PIC 9(4) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 5
               IF I = 3
                   EXIT PERFORM CYCLE
               END-IF
               ADD I TO S
           END-PERFORM
           DISPLAY S
           STOP RUN.
    "#;
    // 1 + 2 + 4 + 5 = 12 (3 skipped)
    assert_eq!(run_capture(src), vec!["0012"]);
}

#[test]
fn exit_paragraph_returns_early() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EPARA.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 FLAG PIC 9 VALUE 1.
       PROCEDURE DIVISION.
       MAIN.
           PERFORM SUB
           DISPLAY "DONE"
           STOP RUN.
       SUB.
           DISPLAY "BEFORE"
           IF FLAG = 1
               EXIT PARAGRAPH
           END-IF
           DISPLAY "AFTER".
    "#;
    assert_eq!(run_capture(src), vec!["BEFORE", "DONE"]);
}

#[test]
fn call_not_on_exception_runs_on_success() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 D PIC X(4) VALUE SPACE.
       PROCEDURE DIVISION.
       MAIN.
           CALL "SUBP"
               ON EXCEPTION DISPLAY "EXC1"
               NOT ON EXCEPTION DISPLAY "OK1"
           END-CALL
           CALL "NO-SUCH-PROG"
               ON EXCEPTION DISPLAY "EXC2"
               NOT ON EXCEPTION DISPLAY "OK2"
           END-CALL
           STOP RUN.
       SUBP.
           DISPLAY "INSUB".
    "#;
    assert_eq!(run_capture(src), vec!["INSUB", "OK1", "EXC2"]);
}

#[test]
fn perform_paragraph_varying_executes_paragraph() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PV.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           PERFORM CALC VARYING WS-N FROM 1 BY 1 UNTIL WS-N > 3
           DISPLAY "AFTER"
           STOP RUN.
       CALC.
           DISPLAY WS-N.
    "#;
    assert_eq!(run_capture(src), vec!["1", "2", "3", "AFTER"]);
}

#[test]
fn next_sentence_skips_rest_of_sentence() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. NS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 X PIC 9 VALUE 1.
       PROCEDURE DIVISION.
       MAIN.
           IF X = 1 NEXT SENTENCE END-IF DISPLAY "SKIPPED".
           DISPLAY "NEXT".
           STOP RUN.
    "#;
    // NEXT SENTENCE jumps past the period → "SKIPPED" is not displayed.
    assert_eq!(run_capture(src), vec!["NEXT"]);
}

#[test]
fn alter_redirects_go_to() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ALT.
       PROCEDURE DIVISION.
       MAIN.
           ALTER GATE TO PROCEED TO TARGET-B
           GO TO GATE.
       GATE.
           GO TO TARGET-A.
       TARGET-A.
           DISPLAY "A" STOP RUN.
       TARGET-B.
           DISPLAY "B" STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["B"]);
}

#[test]
fn exit_perform_times_break() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EPT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 N PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           PERFORM 9 TIMES
               ADD 1 TO N
               IF N = 3
                   EXIT PERFORM
               END-IF
           END-PERFORM
           DISPLAY N
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["3"]);
}
