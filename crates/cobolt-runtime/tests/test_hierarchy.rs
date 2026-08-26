// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Integration tests for the occurrence-aware / hierarchical environment:
//! runtime table subscripting, qualified-name (`A OF B`) disambiguation,
//! `MOVE/ADD/SUBTRACT CORRESPONDING`, and functional `SEARCH`.

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
fn table_subscript_read_and_write() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUBS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-TBL.
          05 WS-ITEM PIC 9(3) OCCURS 5 TIMES.
       01 WS-I PIC 9(2) VALUE 3.
       PROCEDURE DIVISION.
       MAIN.
           MOVE 11 TO WS-ITEM(1)
           MOVE 22 TO WS-ITEM(2)
           MOVE 33 TO WS-ITEM(WS-I)
           DISPLAY WS-ITEM(1)
           DISPLAY WS-ITEM(2)
           DISPLAY WS-ITEM(3)
           STOP RUN.
    "#;
    let out = run_capture(src);
    assert_eq!(out, vec!["011", "022", "033"]);
}

#[test]
fn qualified_names_are_independent_storage() {
    // Two groups with a same-named child must not collide.
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUAL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 ACCOUNT.
          05 BALANCE PIC 9(4) VALUE 0100.
       01 SUMMARY.
          05 BALANCE PIC 9(4) VALUE 0200.
       PROCEDURE DIVISION.
       MAIN.
           MOVE 9999 TO BALANCE OF ACCOUNT
           ADD 1 TO BALANCE OF SUMMARY
           DISPLAY "ACC=" BALANCE OF ACCOUNT
           DISPLAY "SUM=" BALANCE OF SUMMARY
           STOP RUN.
    "#;
    let out = run_capture(src);
    assert_eq!(out, vec!["ACC=9999", "SUM=0201"]);
}

#[test]
fn move_corresponding_matches_by_name() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MCORR.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SRC.
          05 NAME   PIC X(5) VALUE "ALICE".
          05 AGE    PIC 9(3) VALUE 030.
          05 ONLY-A PIC 9(3) VALUE 111.
       01 DST.
          05 NAME   PIC X(5) VALUE "ZZZZZ".
          05 AGE    PIC 9(3) VALUE 005.
          05 ONLY-B PIC 9(3) VALUE 222.
       PROCEDURE DIVISION.
       MAIN.
           MOVE CORRESPONDING SRC TO DST
           DISPLAY "N=" NAME OF DST
           DISPLAY "A=" AGE OF DST
           DISPLAY "B=" ONLY-B OF DST
           STOP RUN.
    "#;
    let out = run_capture(src);
    // NAME and AGE copied; ONLY-B (no counterpart in SRC) untouched.
    assert_eq!(out, vec!["N=ALICE", "A=030", "B=222"]);
}

#[test]
fn add_and_subtract_corresponding() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ACORR.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SRC.
          05 X PIC 9(3) VALUE 010.
          05 Y PIC 9(3) VALUE 020.
       01 DST.
          05 X PIC 9(3) VALUE 100.
          05 Y PIC 9(3) VALUE 200.
       PROCEDURE DIVISION.
       MAIN.
           ADD CORRESPONDING SRC TO DST
           DISPLAY "AX=" X OF DST
           DISPLAY "AY=" Y OF DST
           SUBTRACT CORRESPONDING SRC FROM DST
           DISPLAY "SX=" X OF DST
           DISPLAY "SY=" Y OF DST
           STOP RUN.
    "#;
    let out = run_capture(src);
    assert_eq!(out, vec!["AX=110", "AY=220", "SX=100", "SY=200"]);
}

#[test]
fn search_finds_matching_occurrence() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SRCH.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-TBL.
          05 WS-ITEM PIC 9(3) OCCURS 5 TIMES
             INDEXED BY WS-IDX.
       PROCEDURE DIVISION.
       MAIN.
           MOVE 10 TO WS-ITEM(1)
           MOVE 20 TO WS-ITEM(2)
           MOVE 30 TO WS-ITEM(3)
           MOVE 40 TO WS-ITEM(4)
           MOVE 50 TO WS-ITEM(5)
           SET WS-IDX TO 1
           SEARCH WS-ITEM
               AT END DISPLAY "NOT-FOUND"
               WHEN WS-ITEM(WS-IDX) = 30
                   DISPLAY "FOUND"
           END-SEARCH
           SET WS-IDX TO 1
           SEARCH WS-ITEM
               AT END DISPLAY "MISS"
               WHEN WS-ITEM(WS-IDX) = 99
                   DISPLAY "HIT"
           END-SEARCH
           STOP RUN.
    "#;
    let out = run_capture(src);
    assert_eq!(out, vec!["FOUND", "MISS"]);
}

/// A group item **is** its subordinate items — read and written.
///
/// A group used to own an independent slot that nothing kept in step with the
/// children, so it behaved like a separate variable that merely happened to sit
/// above them: `DISPLAY` of a group printed whatever had been moved to the group
/// itself (usually nothing at all), a group MOVE left every child untouched, and
/// a child MOVE was invisible from the group. Operator, 2026-08-24: *"um grupo é
/// como um item cujo tamanho é definido pelos seus filhos, sendo alfanumérico
/// por natureza"*.
#[test]
fn a_group_is_the_concatenation_of_its_children() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GRP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-INIT.
          05 WS-P PIC 99 VALUE 77.
          05 WS-Q PIC 99 VALUE 88.
       01 WS-G.
          05 WS-A PIC 99.
          05 WS-B PIC 99.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "[" WS-INIT "]"
           MOVE "1234" TO WS-G
           DISPLAY "[" WS-G "]"
           DISPLAY "[" WS-A "][" WS-B "]"
           MOVE 11 TO WS-A
           DISPLAY "[" WS-G "]"
           MOVE WS-G TO WS-INIT
           DISPLAY "[" WS-INIT "]"
           STOP RUN.
    "#;
    let out = run_capture(src);
    assert_eq!(
        out,
        vec![
            "[7788]",  // children's VALUE clauses build the group
            "[1234]",  // a group MOVE lands…
            "[12][34]", // …distributed across the children
            "[1134]",  // a child MOVE shows through the group
            "[1134]",  // group-to-group carries the bytes
        ]
    );
}

/// FILLER holds bytes in the group, and the word itself is optional.
///
/// `children`/`child_keys` exclude FILLER on purpose — they are the
/// `CORRESPONDING` list, and an unnamed item has no name to correspond by. The
/// group's *layout* is a different set, and reading the group from the
/// CORRESPONDING list dropped every separator: the classic edited-time record
/// came back as "23473536" instead of "23:47:35_36".
#[test]
fn filler_holds_its_place_in_a_group_named_or_not() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FILL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NAMED.
          05 WS-C PIC 99 VALUE 33.
          05 FILLER PIC X VALUE ":".
          05 WS-D PIC 99 VALUE 44.
       01 WS-BARE.
          05 WS-E PIC 99 VALUE 55.
          05    PIC X VALUE ":".
          05 WS-F PIC 99 VALUE 66.
       01 WS-NEST.
          05 WS-IN.
             10 WS-X PIC 9 VALUE 1.
             10 FILLER PIC X VALUE "-".
             10 WS-Y PIC 9 VALUE 2.
          05 FILLER PIC X VALUE "|".
          05 WS-Z PIC 9 VALUE 3.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "[" WS-NAMED "]"
           DISPLAY "[" WS-BARE "]"
           DISPLAY "[" WS-NEST "]"
           DISPLAY "[" WS-IN "]"
           STOP RUN.
    "#;
    let out = run_capture(src);
    assert_eq!(
        out,
        vec![
            "[33:44]",  // FILLER written out
            "[55:66]",  // …and with the word omitted, as COBOL-85 allows
            "[1-2|3]",  // a group of groups flattens whole
            "[1-2]",    // and the inner group reads on its own
        ]
    );
}

/// Reference modification addresses CHARACTER POSITIONS, so a numeric sender is
/// taken at its full PIC width — leading zeros included.
///
/// Rendering the *value* instead dropped the padding, so `PIC 9(8)` holding
/// 00224845 came back as "224845" and the classic
/// `MOVE T(1:2) … T(3:2) … T(5:2) … T(7:2)` unpack slid two places left: every
/// field took its neighbour's digits and the last one fell off the end.
#[test]
fn reference_modification_reads_a_numeric_at_its_pic_width() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. REFM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-T PIC 9(8) VALUE 00224845.
       01 WS-EDIT.
          05 WS-HH PIC 99.
          05    PIC X VALUE ":".
          05 WS-MM PIC 99.
          05    PIC X VALUE ":".
          05 WS-SS PIC 99.
          05    PIC X VALUE "_".
          05 WS-CC PIC 99.
       PROCEDURE DIVISION.
       MAIN.
           MOVE WS-T(1:2) TO WS-HH(1:)
           MOVE WS-T(3:2) TO WS-MM(1:)
           MOVE WS-T(5:2) TO WS-SS(1:)
           MOVE WS-T(7:2) TO WS-CC(1:)
           DISPLAY "[" WS-T(1:2) "]"
           DISPLAY "[" WS-T(7:) "]"
           DISPLAY "[" WS-EDIT "]"
           STOP RUN.
    "#;
    let out = run_capture(src);
    assert_eq!(out, vec!["[00]", "[45]", "[00:22:48_45]"]);
}
