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
fn identifier_object_abbreviation() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. IDOBJ.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A PIC 9 VALUE 3.
       01 B PIC 9 VALUE 5.
       01 C PIC 9 VALUE 3.
       01 WS-FLAG PIC 9 VALUE 0.
          88 IS-SET VALUE 1.
       PROCEDURE DIVISION.
       MAIN.
           IF A = B OR C DISPLAY "DATAITEM" ELSE DISPLAY "NO" END-IF
           SET IS-SET TO TRUE
           IF A = B OR IS-SET DISPLAY "COND88" ELSE DISPLAY "NO88" END-IF
           STOP RUN.
    "#;
    // OR C → resolved as A = C (3 = 3) → DATAITEM.
    // OR IS-SET → resolved as the 88-level condition (true) → COND88.
    assert_eq!(run_capture(src), vec!["DATAITEM", "COND88"]);
}

#[test]
fn condition_name_88_set_and_test() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C88.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-GRADE PIC 9(3) VALUE 0.
          88 PASSING VALUE 60 THRU 100.
          88 FAILING VALUE 0 THRU 59.
       PROCEDURE DIVISION.
       MAIN.
           MOVE 75 TO WS-GRADE
           IF PASSING DISPLAY "PASS" ELSE DISPLAY "FAIL" END-IF
           MOVE 40 TO WS-GRADE
           IF FAILING DISPLAY "FAILING" END-IF
           SET PASSING TO TRUE
           DISPLAY WS-GRADE
           STOP RUN.
    "#;
    // 75 → PASSING; 40 → FAILING; SET PASSING TO TRUE → 60 (range start).
    assert_eq!(run_capture(src), vec!["PASS", "FAILING", "060"]);
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

// ── EVALUATE: the subject is evaluated once, and every form still matches ────
//
// `exec_evaluate` used to call `eval_expr` on the subject inside EVERY WHEN, so
// an EVALUATE over N branches evaluated its subject N times. COBOL-85 evaluates
// each subject ONCE and compares that value to every WHEN, which is both the
// standard's rule and far cheaper — on the generated event loop's
// `EVALUATE COBOL-CONTROL-ID` it was one re-read per wired control, per event.
//
// The matching itself must not have shifted, so these cover the paths that now
// read the pre-computed value (literal, range, ALSO, NOT) and the one that
// still evaluates per branch (a TRUE/FALSE subject, whose conditions are
// inherently per-WHEN).

#[test]
fn evaluate_matches_a_literal_after_the_subject_is_hoisted() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EV1.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 C PIC X(8) VALUE "CTRL-3".
       PROCEDURE DIVISION.
       MAIN-PARA.
           EVALUATE C
               WHEN "CTRL-1" DISPLAY "ONE"
               WHEN "CTRL-2" DISPLAY "TWO"
               WHEN "CTRL-3" DISPLAY "THREE"
               WHEN OTHER    DISPLAY "OTHER"
           END-EVALUATE.
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["THREE"]);
}

#[test]
fn evaluate_falls_to_other_when_nothing_matches() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EV2.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 C PIC X(8) VALUE "NOPE".
       PROCEDURE DIVISION.
       MAIN-PARA.
           EVALUATE C
               WHEN "CTRL-1" DISPLAY "ONE"
               WHEN "CTRL-2" DISPLAY "TWO"
               WHEN OTHER    DISPLAY "OTHER"
           END-EVALUATE.
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["OTHER"]);
}

/// A THRU range reads the same hoisted value as a literal does.
#[test]
fn evaluate_matches_a_thru_range() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EV3.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 N PIC 9(3) VALUE 42.
       PROCEDURE DIVISION.
       MAIN-PARA.
           EVALUATE N
               WHEN 1 THRU 10   DISPLAY "LOW"
               WHEN 11 THRU 40  DISPLAY "MID"
               WHEN 41 THRU 99  DISPLAY "HIGH"
               WHEN OTHER       DISPLAY "OUT"
           END-EVALUATE.
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["HIGH"]);
}

/// Every subject in an ALSO list is hoisted, and each column still matches its
/// own subject — a mix-up here would pair column 2 against subject 1.
#[test]
fn evaluate_also_matches_each_column_against_its_own_subject() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EV4.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A PIC X(4) VALUE "B".
       01 B PIC X(4) VALUE "A".
       PROCEDURE DIVISION.
       MAIN-PARA.
           EVALUATE A ALSO B
               WHEN "A" ALSO "B" DISPLAY "AB"
               WHEN "B" ALSO "A" DISPLAY "BA"
               WHEN OTHER        DISPLAY "NEITHER"
           END-EVALUATE.
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["BA"]);
}

/// A TRUE subject keeps evaluating its conditions per WHEN — there is no value
/// to hoist, and each branch asks a different question.
#[test]
fn evaluate_true_still_tests_each_condition() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EV5.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 N PIC 9(3) VALUE 7.
       PROCEDURE DIVISION.
       MAIN-PARA.
           EVALUATE TRUE
               WHEN N > 100 DISPLAY "BIG"
               WHEN N > 5   DISPLAY "SMALL"
               WHEN OTHER   DISPLAY "TINY"
           END-EVALUATE.
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["SMALL"]);
}

/// The shape the generated event loop actually runs: a long chain where the
/// LAST branch is the one that matches, nested one level deep.
#[test]
fn evaluate_matches_the_last_branch_of_a_long_nested_chain() {
    let mut src = String::from(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. EV6.\n\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01 C PIC X(12) VALUE \"CTRL-40\".\n\
         \x20      01 E PIC X(12) VALUE \"onClick\".\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN-PARA.\n\
         \x20          EVALUATE C\n",
    );
    for i in 1..=40 {
        src.push_str(&format!("               WHEN \"CTRL-{i}\"\n"));
        src.push_str("                   EVALUATE E\n");
        src.push_str(&format!(
            "                       WHEN \"onClick\" DISPLAY \"HIT-{i}\"\n"
        ));
        src.push_str("                   END-EVALUATE\n");
    }
    src.push_str("           END-EVALUATE.\n           STOP RUN.\n");
    assert_eq!(run_capture(&src), vec!["HIT-40"]);
}
