// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `INSPECT … REPLACING` as a series, and what an item's character positions
//! actually are — NIST CCVS85 NC216A (VI-94 6.17.3).
//!
//! COBOL-85 gives a *series* of replacing operands one shared left-to-right
//! inspection, the same as a tallying series: at each position the operands are
//! tried in the order written, the first that matches replaces those characters
//! and the scan resumes past them. Running each operand over the whole item in
//! turn is not the same thing — every operand after the first sees the previous
//! one's output, so a `BEFORE`/`AFTER` delimiter can be erased before the
//! operand anchored on it ever runs.
//!
//! Separately: INSPECT reads an item's **character positions**, and a signed
//! DISPLAY item carries its sign as an overpunch on a digit rather than in a
//! position of its own — so a minus sign is not among them.

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

#[test]
fn a_replacing_series_shares_one_scan() {
    // NC216A INS-TEST-F3-19 .05, trimmed to the tail the six operands act on.
    // `FIRST "L " BY "ZZ" AFTER INITIAL "AL"` erases the very `"L "` that the
    // next operand is anchored on; under one scan the delimiters were fixed
    // before any replacement, so `"BAD"` is still replaced and the tail reads
    // "ALZZZZZZ". Operand by operand it came out "ALZZBADZ".
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSSER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SUBJ PIC X(20).
       PROCEDURE DIVISION.
       MAIN.
           MOVE "CAN NOT BE ALL BAD." TO SUBJ.
           INSPECT SUBJ REPLACING
               FIRST "L " BY "ZZ" AFTER INITIAL "AL"
               FIRST "BAD" BY "ZZZ" AFTER "L "
               LEADING "BAD" BY "ZZZ" BEFORE INITIAL "Q"
               FIRST "BAD" BY "ZZZ" BEFORE INITIAL "Z"
               FIRST "BAD" BY "ZZZ" AFTER "ALL "
               ALL "." BY "Z" AFTER "AL".
           DISPLAY "[" SUBJ "]".
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["[CAN NOT BE ALZZZZZZ ]"]);
}

#[test]
fn an_earlier_operand_takes_the_position_from_a_later_one() {
    // The rule the shared scan exists for, on its own: `ALL "AA"` written first
    // takes both characters at position 0, so `ALL "A"` — which sweeps the same
    // field — only reaches the one at position 3.
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSORD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 S1 PIC X(4).
       01 S2 PIC X(4).
       PROCEDURE DIVISION.
       MAIN.
           MOVE "AABA" TO S1.
           INSPECT S1 REPLACING ALL "AA" BY "XY" ALL "A" BY "Z".
           DISPLAY "[" S1 "]".
           MOVE "AABA" TO S2.
           INSPECT S2 REPLACING ALL "A" BY "Z" ALL "AA" BY "XY".
           DISPLAY "[" S2 "]".
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["[XYBZ]", "[ZZBZ]"]);
}

#[test]
fn leading_and_trailing_keep_working_in_a_series() {
    // The other three operand forms through the same scan, so the rewrite is
    // pinned beyond the case that motivated it. `LEADING` stops at the first
    // position it does not take; `TRAILING` starts where its run does;
    // `CHARACTERS` takes whatever stands in its window.
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSFORMS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 S1 PIC X(8).
       01 S2 PIC X(8).
       01 S3 PIC X(8).
       PROCEDURE DIVISION.
       MAIN.
           MOVE "AABAABAA" TO S1.
           INSPECT S1 REPLACING LEADING "A" BY "Z".
           DISPLAY "[" S1 "]".
           MOVE "AABAABAA" TO S2.
           INSPECT S2 REPLACING TRAILING "A" BY "Z".
           DISPLAY "[" S2 "]".
           MOVE "AABAABAA" TO S3.
           INSPECT S3 REPLACING CHARACTERS BY "Z" AFTER INITIAL "B".
           DISPLAY "[" S3 "]".
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["[ZZBAABAA]", "[AABAABZZ]", "[AABZZZZZ]"]);
}

#[test]
fn a_signed_display_item_has_no_minus_sign_among_its_characters() {
    // NC216A INS-TEST-F1-23. `PIC S9(5)` holding -12345 has five character
    // positions, all digits: the operational sign is an overpunch, so counting
    // `"-"` gives 0 while counting `"5"` still gives 1. Rendering the item with
    // its leading minus made the first count 1.
    //
    // `SIGN IS … SEPARATE CHARACTER` is the case where the sign *does* occupy a
    // position, and it must still be counted.
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSSGN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 EMBEDDED PIC S9(5).
       01 SEPARATE-1 PIC S9(5) SIGN IS LEADING SEPARATE.
       01 EDITED-1 PIC -9(5).
       01 T1 PIC 999.
       01 T2 PIC 999.
       PROCEDURE DIVISION.
       MAIN.
           MOVE -12345 TO EMBEDDED.
           MOVE ZERO TO T1. MOVE ZERO TO T2.
           INSPECT EMBEDDED TALLYING T1 FOR ALL "-" T2 FOR ALL "5".
           DISPLAY "EMB T1=" T1 " T2=" T2.
           MOVE -12345 TO SEPARATE-1.
           MOVE ZERO TO T1. MOVE ZERO TO T2.
           INSPECT SEPARATE-1 TALLYING T1 FOR ALL "-" T2 FOR ALL "5".
           DISPLAY "SEP T1=" T1 " T2=" T2.
           MOVE -12345 TO EDITED-1.
           MOVE ZERO TO T1. MOVE ZERO TO T2.
           INSPECT EDITED-1 TALLYING T1 FOR ALL "-" T2 FOR ALL "5".
           DISPLAY "EDT T1=" T1 " T2=" T2.
           STOP RUN.
"#,
    );
    assert_eq!(
        out,
        vec!["EMB T1=000 T2=001", "SEP T1=001 T2=001", "EDT T1=001 T2=001"]
    );
}

#[test]
fn replacing_inside_a_signed_item_leaves_its_sign_alone() {
    // The sign is taken off the character positions on the way in, so it has to
    // go back on the way out: replacing a digit must not turn -12345 into
    // +12945.
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSSGNR.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 EMBEDDED PIC S9(5).
       PROCEDURE DIVISION.
       MAIN.
           MOVE -12345 TO EMBEDDED.
           INSPECT EMBEDDED REPLACING ALL "3" BY "9".
           DISPLAY "[" EMBEDDED "]".
           IF EMBEDDED < 0 DISPLAY "NEGATIVE" ELSE DISPLAY "POSITIVE".
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["[-12945]", "NEGATIVE"]);
}
