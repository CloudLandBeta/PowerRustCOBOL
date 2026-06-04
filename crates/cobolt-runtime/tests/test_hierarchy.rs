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
    display_rx
        .try_iter()
        .map(|s| s.trim().to_owned())
        .collect()
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
