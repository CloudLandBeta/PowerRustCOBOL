// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! INSPECT: combined TALLYING … REPLACING, and BEFORE/AFTER INITIAL regions.

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
fn tallying_and_replacing_combined() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 W PIC X(11) VALUE "MISSISSIPPI".
       01 C PIC 9(2) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           INSPECT W TALLYING C FOR ALL "S"
                     REPLACING ALL "S" BY "X"
           DISPLAY C
           DISPLAY W
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["04", "MIXXIXXIPPI"]);
}

#[test]
fn tally_after_initial() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSP2.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 W PIC X(11) VALUE "MISSISSIPPI".
       01 C PIC 9(2) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           INSPECT W TALLYING C FOR ALL "I" AFTER INITIAL "P"
           DISPLAY C
           STOP RUN.
    "#;
    // After the first P (…PPI), only one "I" remains.
    assert_eq!(run_capture(src), vec!["01"]);
}

#[test]
fn replace_before_initial() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSP3.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 W PIC X(11) VALUE "MISSISSIPPI".
       PROCEDURE DIVISION.
       MAIN.
           INSPECT W REPLACING ALL "I" BY "Y" BEFORE INITIAL "P"
           DISPLAY W
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["MYSSYSSYPPI"]);
}

// ── A series of TALLYING operands shares one scan ────────────────────────────
//
// From NIST CCVS85 NC216A. COBOL-85 6.17.3 inspects the item **once**, left to
// right; at each character position the operands are tried in the order they
// were written and the first that matches takes the position, the scan
// resuming past the characters it consumed. Each operand used to sweep the
// whole item on its own, so the same characters were tallied by several
// operands at once.

/// NC216A `INS-TEST-F1-27`, the smallest case: `ALL "AA"` takes positions 1-2
/// of `"AABA"`, so `ALL "A"` can only find the final one. Counting them
/// independently gives three.
#[test]
fn a_tallying_series_shares_one_left_to_right_scan() {
    let out = run_capture(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSPSER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 AABA-XN-4 PICTURE X(4) VALUE "AABA".
       01 T1 PICTURE 999 VALUE ZERO.
       01 T2 PICTURE 999 VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN.
           INSPECT AABA-XN-4 TALLYING T1 FOR ALL "AA"
                                      T2 FOR ALL "A".
           DISPLAY "T1=" T1 " T2=" T2.
           STOP RUN.
"#,
    );
    assert_eq!(out, ["T1=001 T2=001"], "{out:#?}");
}

/// The order the operands are written in decides who takes a position: with
/// `ALL "A"` first, it takes position 1 and `ALL "AA"` never matches.
#[test]
fn the_order_of_the_operands_decides_who_takes_a_position() {
    let out = run_capture(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSPORD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 AABA-XN-4 PICTURE X(4) VALUE "AABA".
       01 T1 PICTURE 999 VALUE ZERO.
       01 T2 PICTURE 999 VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN.
           INSPECT AABA-XN-4 TALLYING T1 FOR ALL "A"
                                      T2 FOR ALL "AA".
           DISPLAY "T1=" T1 " T2=" T2.
           STOP RUN.
"#,
    );
    assert_eq!(out, ["T1=003 T2=000"], "{out:#?}");
}

/// NC216A `INS-TEST-F3-19`: `LEADING` must match from its window's left edge
/// with no gap, so an earlier operand taking that very position ends the run
/// before it starts — and `CHARACTERS` counts only the positions no earlier
/// operand claimed. Independently, these were 1, 15 and 6.
#[test]
fn leading_and_characters_yield_to_an_earlier_operand() {
    let out = run_capture(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSPF319.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WRK-XN-83-1 PICTURE X(83).
       01 T1 PICTURE 999 VALUE ZERO.
       01 T2 PICTURE 999 VALUE ZERO.
       01 T3 PICTURE 999 VALUE ZERO.
       01 T4 PICTURE 999 VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "AH YES AH YES W.C. FRITOES HERE. ANYONE WHO HATES DOGS AND KIDS CAN NOT BE ALL BAD." TO WRK-XN-83-1.
           INSPECT WRK-XN-83-1 TALLYING T1 FOR ALL "A"
                   T2 FOR LEADING "AH"
                   T3 FOR CHARACTERS BEFORE "."
                   T4 FOR CHARACTERS AFTER "AL".
           DISPLAY "T1=" T1 " T2=" T2 " T3=" T3 " T4=" T4.
           STOP RUN.
"#,
    );
    assert_eq!(out, ["T1=008 T2=000 T3=013 T4=005"], "{out:#?}");
}

/// A `LEADING` operand written first still counts its whole run — the shared
/// scan must not cost it anything when nothing competes for those positions.
#[test]
fn a_leading_operand_written_first_counts_its_whole_run() {
    let out = run_capture(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSPLEAD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SUBJ PICTURE X(10) VALUE "ABABABXXYZ".
       01 T1 PICTURE 999 VALUE ZERO.
       01 T2 PICTURE 999 VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN.
           INSPECT SUBJ TALLYING T1 FOR LEADING "AB"
                                 T2 FOR ALL "X".
           DISPLAY "T1=" T1 " T2=" T2.
           STOP RUN.
"#,
    );
    assert_eq!(out, ["T1=003 T2=002"], "{out:#?}");
}

/// A single operand is unchanged by the rewrite — `TRAILING` still counts the
/// run at the end of the item, and one counter may carry several operands.
#[test]
fn a_single_operand_and_a_shared_counter_are_unchanged() {
    let out = run_capture(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSPONE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SUBJ PICTURE X(10) VALUE "XYABABABAB".
       01 T1 PICTURE 999 VALUE ZERO.
       01 T2 PICTURE 999 VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN.
           INSPECT SUBJ TALLYING T1 FOR TRAILING "AB".
           INSPECT SUBJ TALLYING T2 FOR ALL "X" ALL "Y".
           DISPLAY "T1=" T1 " T2=" T2.
           STOP RUN.
"#,
    );
    assert_eq!(out, ["T1=004 T2=002"], "{out:#?}");
}
