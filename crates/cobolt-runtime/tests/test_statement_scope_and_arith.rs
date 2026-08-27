// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Statement semantics the NIST CCVS85 Nucleus module leans on everywhere: the
//! scope a conditional phrase actually has, `DIVIDE`'s two operand orders and
//! its `REMAINDER`, `ROUNDED` into a numeric-edited receiver, the single record
//! area an `FD` owns, `PERFORM` of a section name, and qualification deeper
//! than one level.

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

/// The period that ends the sentence ends the `ON SIZE ERROR` imperative with
/// it. Reading past it made every following sentence part of the phrase, so
/// they ran **only** when the condition fired — silently, with no diagnostic.
#[test]
fn a_period_closes_a_conditional_phrase() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SCOPE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  A PIC 999V99.
       01  FLAG PIC X VALUE "A".
       PROCEDURE DIVISION.
       MAIN-PARA.
           DIVIDE 5 INTO 20 GIVING A ON SIZE ERROR
               MOVE "P" TO FLAG.
           DISPLAY "AFTER=" FLAG.
           STOP RUN.
"#;
    // No size error here, so the phrase must not run — but the DISPLAY after
    // the period must.
    assert_eq!(run_capture(src), vec!["AFTER=A"]);
}

/// `DIVIDE dividend BY divisor` and `DIVIDE divisor INTO dividend` name their
/// operands in opposite orders. Honouring only `BY` divided the wrong way
/// round, and in the receiver form stored the quotient into the divisor.
#[test]
fn divide_into_and_by_name_their_operands_in_opposite_orders() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DIVORD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  B PIC 999V99.
       01  C PIC 999V99.
       01  D PIC 999V99.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DIVIDE 20 BY 5 GIVING C.
           DISPLAY "BY=" C.
           DIVIDE 5 INTO 20 GIVING C.
           DISPLAY "INTO=" C.
           MOVE 20 TO B.
           DIVIDE 5 INTO B.
           DISPLAY "INPLACE=" B.
           MOVE 10 TO B.
           MOVE 30 TO D.
           DIVIDE 2 INTO B D.
           DISPLAY "SERIES=" B " " D.
           STOP RUN.
"#;
    assert_eq!(
        run_capture(src),
        vec![
            "BY=00400",
            "INTO=00400",
            "INPLACE=00400",
            "SERIES=00500 01500",
        ]
    );
}

/// COBOL-85 computes the remainder from the quotient **as stored in the GIVING
/// receiver** — truncated to that item's PICTURE. Using a bare integer quotient
/// reported an invalid remainder for every non-integer receiver.
#[test]
fn remainder_uses_the_quotient_the_receiver_holds() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DIVREM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  C PIC 999V99.
       01  R PIC 999V99.
       01  CI PIC 999.
       01  RI PIC 999.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DIVIDE 7 INTO 23 GIVING C REMAINDER R.
           DISPLAY "SCALED=" C " " R.
           DIVIDE 7 INTO 23 GIVING CI REMAINDER RI.
           DISPLAY "INTEGER=" CI " " RI.
           STOP RUN.
"#;
    // 23 − (3.28 × 7) = 0.04; with an integer receiver the same rule gives the
    // familiar 23 − (3 × 7) = 2.
    assert_eq!(
        run_capture(src),
        vec!["SCALED=00328 00004", "INTEGER=003 002"]
    );
}

/// The receiver's scale comes from its PICTURE, not from the value it happens
/// to hold: a numeric-edited item stores edited characters and reported no
/// scale at all, so `ROUNDED` silently truncated into one.
#[test]
fn rounded_reaches_a_numeric_edited_receiver() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ROUNDEDIT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  MULT1 PICTURE S99V99.
       01  MULT3 PICTURE $$$$.99.
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE 80.12 TO MULT1.
           MULTIPLY .9 BY MULT1 GIVING MULT3 ROUNDED.
           DISPLAY "EDITED=" MULT3.
           STOP RUN.
"#;
    assert_eq!(run_capture(src), vec!["EDITED= $72.11"]);
}

/// An `FD` owns **one** record area however many `01` entries describe it: they
/// are implicit redefinitions of one another, so a write through any of them is
/// visible through the rest.
#[test]
fn every_fd_record_describes_one_record_area() {
    let dir = std::env::temp_dir().join("rustcobol-fd-record-area");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("cannot create the work directory");
    let path = dir.join("OUT.TXT");
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FDAREA.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT PRINT-FILE ASSIGN TO "{}".
       DATA DIVISION.
       FILE SECTION.
       FD  PRINT-FILE.
       01  PRINT-REC     PICTURE X(14).
       01  DUMMY-RECORD  PICTURE X(14).
       PROCEDURE DIVISION.
       MAIN-PARA.
           OPEN OUTPUT PRINT-FILE.
           MOVE "FROM-PRINT-REC" TO PRINT-REC.
           WRITE DUMMY-RECORD AFTER ADVANCING 1 LINES.
           MOVE "FROM-DUMMY-REC" TO DUMMY-RECORD.
           WRITE PRINT-REC AFTER ADVANCING 1 LINES.
           CLOSE PRINT-FILE.
           STOP RUN.
"#,
        path.display()
    );
    run_capture(&src);
    // A record-SEQUENTIAL file holds fixed-width records with no separator, so
    // the two 14-byte records sit end to end.
    let written = std::fs::read_to_string(&path).expect("no output file");
    assert_eq!(
        written, "FROM-PRINT-RECFROM-DUMMY-REC",
        "a value moved through one 01 must be visible through the other"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A section header carries no statements of its own: it *is* the paragraphs
/// that follow it, up to the next header. `PERFORM` used to find the empty
/// header entry and return without running anything.
#[test]
fn performing_a_section_runs_its_paragraphs() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SECPERF.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  N PIC 999 VALUE ZERO.
       PROCEDURE DIVISION.
       DRIVER SECTION.
       START-PARA.
           PERFORM WORK-SECTION.
           DISPLAY "AFTER-SECTION=" N.
           PERFORM WORK-A.
           DISPLAY "AFTER-PARA=" N.
           STOP RUN.
       WORK-SECTION SECTION.
       WORK-A.
           ADD 1 TO N.
       WORK-B.
           ADD 10 TO N.
       TAIL-SECTION SECTION.
       TAIL-A.
           ADD 100 TO N.
"#;
    // The section runs both of its paragraphs and stops at the next header;
    // performing one paragraph by name runs only that paragraph.
    assert_eq!(
        run_capture(src),
        vec!["AFTER-SECTION=011", "AFTER-PARA=012"]
    );
}

/// `A OF B OF C` keeps every qualifier. Nesting the chain as it was read put
/// the growing chain where only a plain name fits, so all but the last level
/// were dropped and a duplicated name resolved to whichever was declared first.
#[test]
fn qualification_resolves_through_every_level() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUALDEEP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  ACC PIC 9(9) VALUE ZERO.
       01  LEVEL-5A.
           02  LEVEL-4A.
               03  LEVEL-3A.
                   04  LEVEL-2A.
                       05  LEVEL-1A.
                           06  ITEM-1 PIC 9    VALUE 1.
                       05  LEVEL-1B.
                           06  ITEM-1 PIC 9(2) VALUE 2.
                   04  LEVEL-2B.
                       05  LEVEL-1A.
                           06  ITEM-1 PIC 9(3) VALUE 3.
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE ZERO TO ACC.
           ADD ITEM-1 OF LEVEL-1A IN LEVEL-2A OF LEVEL-3A IN LEVEL-4A
               OF LEVEL-5A TO ACC.
           DISPLAY "FIVE=" ACC.
           MOVE ZERO TO ACC.
           ADD ITEM-1 OF LEVEL-1B TO ACC.
           DISPLAY "ONE=" ACC.
           MOVE ZERO TO ACC.
           ADD ITEM-1 OF LEVEL-1A OF LEVEL-2B TO ACC.
           DISPLAY "TWO=" ACC.
           STOP RUN.
"#;
    assert_eq!(
        run_capture(src),
        vec!["FIVE=000000001", "ONE=000000002", "TWO=000000003"]
    );
}

/// `ADD`/`SUBTRACT CORRESPONDING` carry `ON SIZE ERROR` and `ROUNDED` like any
/// other arithmetic statement; both phrases used to be parsed and discarded.
#[test]
fn corresponding_arithmetic_honours_size_error() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CORRSE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  FLAG PIC X VALUE "A".
       01  SRC-G.
           02  FLD-A PIC 9(3) VALUE 900.
           02  FLD-B PIC 9(3) VALUE 1.
       01  DST-G.
           02  FLD-A PIC 9(3) VALUE 200.
           02  FLD-B PIC 9(3) VALUE 5.
       PROCEDURE DIVISION.
       MAIN-PARA.
           ADD CORRESPONDING SRC-G TO DST-G
               ON SIZE ERROR MOVE "P" TO FLAG.
           DISPLAY "FLAG=" FLAG.
           DISPLAY "A=" FLD-A OF DST-G " B=" FLD-B OF DST-G.
           STOP RUN.
"#;
    // FLD-A overflows `PIC 9(3)` (1100) and keeps its old value; FLD-B does not
    // and receives its result. The imperative runs once, for the statement.
    assert_eq!(
        run_capture(src),
        vec!["FLAG=P", "A=200 B=006"]
    );
}
