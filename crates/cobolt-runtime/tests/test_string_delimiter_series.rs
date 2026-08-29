// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `STRING`: what a `DELIMITED BY` phrase governs, and where the result lands.
//! NIST CCVS85 NC217A.
//!
//! `STRING a b c DELIMITED BY d INTO x` delimits **all three** senders. The
//! phrase governs the whole series that precedes it, not the one it happens to
//! be written after — COBOL-85 6.24.3, where the format is a repeating
//! `{sender}… DELIMITED BY …` group.
//!
//! Attaching the delimiter to the last sender alone left the earlier ones with
//! none, which is `DELIMITED BY SIZE`, so the whole of each was appended. The
//! failure is quiet: the receiver fills up and truncates, so the result still
//! *looks* like a string built from the right pieces.

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

/// NC217A STR-TEST-GF-10: `DELIMITED BY ZERO` over a series of five literals.
///
/// Each sender contributes the characters before its first `"0"`, so the five
/// contribute `A`, `B`, `C`, `D`, `E`. With the delimiter bound to the last
/// sender only, the first two were appended whole and the receiver held
/// `"A0B0D"`.
#[test]
fn a_delimiter_governs_the_whole_preceding_series() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. STRSER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 ID7-XN-5   PICTURE X(5).
       01 ID8-DU-2V0 PICTURE 99.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "*****" TO ID7-XN-5.
           MOVE 1 TO ID8-DU-2V0.
           STRING "A0" "B0D" "C0LKJSD" "D0321" "E0987LKJALKJKLLKJSD"
               DELIMITED BY ZERO INTO ID7-XN-5 POINTER ID8-DU-2V0.
           DISPLAY "R=[" ID7-XN-5 "]".
           IF ID8-DU-2V0 = 6 DISPLAY "PTR ok" ELSE DISPLAY "PTR BAD".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "R=[ABCDE]");
    assert_eq!(out[1], "PTR ok", "five characters placed from position 1");
}

/// NC217A STR-TEST-GF-11: the same rule with `DELIMITED BY QUOTE` — a
/// figurative constant delimiter, matched byte for byte.
#[test]
fn a_figurative_delimiter_governs_the_series_too() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. STRQ.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 ID7-XN-5 PICTURE X(5).
       PROCEDURE DIVISION.
       MAIN.
           MOVE "*****" TO ID7-XN-5.
           STRING "A""" "B""KJHSF" "C""321654987LLKJHAF" "D""=,l."
               "E""********" DELIMITED BY QUOTE INTO ID7-XN-5.
           DISPLAY "R=[" ID7-XN-5 "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "R=[ABCDE]");
}

/// Two `DELIMITED BY` phrases in one statement: each governs only the senders
/// **since the previous one**.
#[test]
fn each_delimiter_governs_only_its_own_group() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. STRTWO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 R PICTURE X(10).
       PROCEDURE DIVISION.
       MAIN.
           MOVE SPACES TO R.
           STRING "A0" "B0" DELIMITED BY ZERO
                  "C-" "D-" DELIMITED BY "-"
                  INTO R.
           DISPLAY "R=[" R "]".
           STOP RUN.
"#,
    );
    assert_eq!(
        out[0], "R=[ABCD      ]",
        "the first pair splits on ZERO, the second on the hyphen"
    );
}

/// Senders written **after** the last `DELIMITED BY` take the whole of each,
/// and a statement with no phrase at all is unchanged.
#[test]
fn senders_after_the_last_delimiter_are_delimited_by_size() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. STRTAIL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 R PICTURE X(10).
       PROCEDURE DIVISION.
       MAIN.
           MOVE SPACES TO R.
           STRING "A0" DELIMITED BY ZERO "B0" INTO R.
           DISPLAY "R1=[" R "]".
           MOVE SPACES TO R.
           STRING "XY" "ZW" INTO R.
           DISPLAY "R2=[" R "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "R1=[AB0       ]", "the trailing sender keeps its zero");
    assert_eq!(out[1], "R2=[XYZW      ]", "no phrase at all is unchanged");
}

/// NC217A STR-TEST-GF-21: `STRING … INTO <group>` distributes across the
/// group's children.
///
/// A group owns no store slot — its value is synthesized from its children — so
/// writing the group's own slot left the record exactly as it was and the test
/// read back spaces.
#[test]
fn string_into_a_group_reaches_its_children() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. STRGRP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 TEST-21-GROUP.
          02 G-A PICTURE XX.
          02 G-B PICTURE XXX.
       01 ID8-DU-2V0 PICTURE 99.
       PROCEDURE DIVISION.
       MAIN.
           MOVE 1 TO ID8-DU-2V0.
           STRING "ABCDEF" DELIMITED BY SIZE INTO TEST-21-GROUP
               WITH POINTER ID8-DU-2V0.
           DISPLAY "G=[" TEST-21-GROUP "]".
           DISPLAY "A=[" G-A "]".
           DISPLAY "B=[" G-B "]".
           IF ID8-DU-2V0 = 6 DISPLAY "PTR ok" ELSE DISPLAY "PTR BAD".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "G=[ABCDE]", "truncated to the group's five bytes");
    assert_eq!(out[1], "A=[AB]", "and distributed across the children");
    assert_eq!(out[2], "B=[CDE]");
    assert_eq!(out[3], "PTR ok");
}
