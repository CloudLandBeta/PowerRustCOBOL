// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A group `MOVE` carries bytes, not values — NIST CCVS85 NC252A RDF-TEST-11.
//!
//! When the receiving item is a group the standard makes the whole move
//! alphanumeric: each slice lands in its subordinate item exactly as it stands,
//! whatever that item's PICTURE says. A numeric child therefore has to be able
//! to hold characters that are not digits.
//!
//! `set_group` used to **drop** any slice that was not a number, so the child
//! kept whatever it held before. `MOVE REDEF13 TO REDEF12` — 120 bytes of `A`
//! into a group whose children include `PIC 9(5)` and six `PIC 9` — left the
//! record reading `AAA    0AA     0AAAA`, with the old digits showing through
//! wherever a numeric child sat.
//!
//! ## What is deliberately *not* byte-exact
//!
//! A slice that **does** spell a number is still stored as that number, so
//! `"  123"` into a `PIC 9(5)` child reads back `00123` rather than `"  123"`.
//! Strictly the standard changes no bytes at all. Storing the characters in
//! that case too was measured at **NC 74 programs clean against 77**: a great
//! many programs move a numeric group and then compute with its children, and
//! the normalisation is what keeps those working. The stricter reading is
//! recorded in `set_group` and in `last_case_documents_the_remaining_gap`
//! below, not applied.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let errors: Vec<String> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("{}:{}: {}", d.span.line, d.span.col, d.message))
        .collect();
    assert!(errors.is_empty(), "parse errors: {errors:#?}");
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    drop(interp);
    display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect()
}

/// A numeric child takes non-numeric bytes rather than keeping its old digits.
#[test]
fn a_numeric_child_takes_the_bytes_it_is_given() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GMOVE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SRC.
          02 FILLER PICTURE X(16) VALUE "AAAAAAAAAAAAAAAA".
       01 DST.
          02 D-A PICTURE X(3).
          02 D-N PICTURE 9(5).
          02 D-B PICTURE X(8).
       PROCEDURE DIVISION.
       MAIN.
           MOVE SRC TO DST.
           DISPLAY "DST=[" DST "]".
           DISPLAY "D-N=[" D-N "]".
           STOP RUN.
"#);
    assert_eq!(out[0], "DST=[AAAAAAAAAAAAAAAA]", "{out:#?}");
    assert_eq!(out[1], "D-N=[AAAAA]", "{out:#?}");
}

/// NC252A RDF-TEST-11: the receiving group **redefines** another record, so the
/// bytes have to show through the redefined description too.
#[test]
fn the_bytes_show_through_a_redefined_description() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GMOVERDF.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 REDEF10.
          02 RDFDATA1 PICTURE X(10) VALUE "ABC9876543".
          02 RDFDATA2 PICTURE 9(4)V99 VALUE 9116.44.
       01 REDEF12 REDEFINES REDEF10.
          02 RDFDATA9  PICTURE A(3).
          02 RDFDATA10 PICTURE 9(5).
          02 RDFDATA15 PICTURE X(8).
       01 REDEF13.
          02 FILLER PICTURE X(16) VALUE "AAAAAAAAAAAAAAAA".
       PROCEDURE DIVISION.
       MAIN.
           MOVE REDEF13 TO REDEF12.
           IF REDEF10 EQUAL TO "AAAAAAAAAAAAAAAA"
              DISPLAY "ALL-A"
           ELSE
              DISPLAY "GOT=[" REDEF10 "]".
           STOP RUN.
"#);
    assert_eq!(out[0], "ALL-A", "{out:#?}");
}

/// A nested numeric child, and an `OCCURS` run of them, are reached too —
/// NC252A's `RDFDATA14 OCCURS 6 TIMES PICTURE 9` sits two levels down.
#[test]
fn nested_and_repeating_numeric_children_are_reached() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GMOVENEST.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SRC.
          02 FILLER PICTURE X(10) VALUE "AAAAAAAAAA".
       01 DST.
          02 D-OUTER.
             03 D-TEXT PICTURE XX.
             03 D-DIGIT OCCURS 6 TIMES PICTURE 9.
          02 D-TAIL PICTURE XX.
       PROCEDURE DIVISION.
       MAIN.
           MOVE SRC TO DST.
           DISPLAY "DST=[" DST "]".
           DISPLAY "D3=[" D-DIGIT (3) "]".
           STOP RUN.
"#);
    assert_eq!(out[0], "DST=[AAAAAAAAAA]", "{out:#?}");
    assert_eq!(out[1], "D3=[A]", "{out:#?}");
}

/// The all-blank slice kept working — it was the one non-numeric case the old
/// code already handled, and `MOVE SPACE TO <group of PIC 99>` depends on it.
#[test]
fn spaces_still_blank_a_numeric_child() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GMOVESP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 DST.
          02 D-N1 PICTURE 99 VALUE 42.
          02 D-N2 PICTURE 99 VALUE 17.
       PROCEDURE DIVISION.
       MAIN.
           MOVE SPACE TO DST.
           DISPLAY "DST=[" DST "]".
           STOP RUN.
"#);
    assert_eq!(out[0], "DST=[    ]", "{out:#?}");
}

/// A numeric group move still leaves its children computable — this is what
/// the `Ok` arm's normalisation protects, and why the byte-exact reading was
/// not applied.
#[test]
fn a_numeric_group_move_leaves_its_children_computable() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GMOVENUM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SRC.
          02 S-N1 PICTURE 999 VALUE 120.
          02 S-N2 PICTURE 999 VALUE 034.
       01 DST.
          02 D-N1 PICTURE 999.
          02 D-N2 PICTURE 999.
       01 TOTAL PICTURE 9999 VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN.
           MOVE SRC TO DST.
           ADD D-N1 D-N2 GIVING TOTAL.
           DISPLAY "TOTAL=" TOTAL.
           STOP RUN.
"#);
    assert_eq!(out[0], "TOTAL=0154", "{out:#?}");
}

/// The remaining gap, recorded rather than asserted away: a slice that spells a
/// number is re-rendered through the child's PICTURE, so a leading blank
/// becomes a leading zero. See this file's header for why.
#[test]
fn last_case_documents_the_remaining_gap() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GMOVEGAP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SRC.
          02 FILLER PICTURE X(11) VALUE "XX  123YYYY".
       01 DST.
          02 D-A PICTURE X(2).
          02 D-N PICTURE 9(5).
          02 D-B PICTURE X(4).
       PROCEDURE DIVISION.
       MAIN.
           MOVE SRC TO DST.
           DISPLAY "DST=[" DST "]".
           STOP RUN.
"#);
    // Byte-exact would be `XX  123YYYY`. Change this assertion only alongside a
    // fresh NC measurement — the last attempt cost three clean programs.
    assert_eq!(out[0], "DST=[XX00123YYYY]", "{out:#?}");
}
