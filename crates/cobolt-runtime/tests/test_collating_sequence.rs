// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `ALPHABET` and `PROGRAM COLLATING SEQUENCE` — the rules the NIST CCVS85
//! Nucleus module measures in NC215A and NC219A.
//!
//! Each test is one rule of COBOL-85, written the way the suite writes it.

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
    display_rx
        .try_iter()
        .map(|s| s.trim_end().to_owned())
        .collect()
}

/// NC215A's alphabet, written exactly as the suite writes it — commas between
/// operands and all. The comma is a pure separator; stopping at one truncated
/// the sequence and left everything after it in native order.
const WILD_ONE: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. WILDONE.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
OBJECT-COMPUTER.
    XX-SYS
    PROGRAM COLLATING SEQUENCE IS THE-WILD-ONE.
SPECIAL-NAMES.
    ALPHABET
    THE-WILD-ONE IS "A" THRU "H" "I" ALSO "J", ALSO "K", ALSO
    "L" ALSO "M" ALSO "N" "O" THROUGH "Z" "0" THRU "9".
DATA DIVISION.
WORKING-STORAGE SECTION.
01 A-AN-1 PIC A VALUE "A".
01 B-AN-1 PIC A VALUE "B".
01 H-AN-1 PIC A VALUE "H".
01 I-AN-1 PIC A VALUE "I".
01 J-AN-1 PIC A VALUE "J".
01 K-AN-1 PIC A VALUE "K".
01 L-AN-1 PIC A VALUE "L".
01 M-AN-1 PIC A VALUE "M".
01 N-AN-1 PIC A VALUE "N".
01 O-AN-1 PIC A VALUE "O".
01 ZERO-DU-9V0-1 PIC 9 VALUE ZERO.
01 NINE-DU-9V0-1 PIC 9 VALUE 9.
PROCEDURE DIVISION.
MAIN-PARA.
"#;

fn wild_one(body: &str) -> Vec<String> {
    run_capture(&format!("{WILD_ONE}{body}\n    STOP RUN.\n"))
}

/// `LOW-VALUE` names the character at the lowest position of the *program's*
/// sequence, not `0x00` (NC215A SEQ-TEST-GF-1).
#[test]
fn low_value_is_the_first_character_of_the_sequence() {
    assert_eq!(
        wild_one(r#"    IF A-AN-1 EQUAL TO LOW-VALUE DISPLAY "YES" ELSE DISPLAY "NO" END-IF."#),
        vec!["YES"]
    );
}

/// `ALSO` folds its operands into one position, so they compare **equal**
/// (NC215A SEQ-TEST-GF-3).
#[test]
fn also_makes_the_joined_characters_equal() {
    assert_eq!(
        wild_one(
            r#"    IF I-AN-1 = J-AN-1 AND K-AN-1 AND L-AN-1 AND M-AN-1
        AND N-AN-1 DISPLAY "YES" ELSE DISPLAY "NO" END-IF."#
        ),
        vec!["YES"]
    );
}

/// Ordinary ordering follows the written positions (NC215A SEQ-TEST-GF-2/GF-4).
#[test]
fn ordering_follows_the_written_positions() {
    assert_eq!(
        wild_one(
            r#"    IF H-AN-1 < I-AN-1 AND J-AN-1 > B-AN-1 DISPLAY "YES"
        ELSE DISPLAY "NO" END-IF.
    IF O-AN-1 > N-AN-1 DISPLAY "YES2" ELSE DISPLAY "NO2" END-IF."#
        ),
        vec!["YES", "YES2"]
    );
}

/// The digits are written after `Z`, so a letter sorts **below** a digit — the
/// opposite of ASCII (NC215A SEQ-TEST-GF-5).
#[test]
fn the_sequence_overrides_native_order() {
    assert_eq!(
        wild_one(r#"    IF A-AN-1 < ZERO-DU-9V0-1 DISPLAY "YES" ELSE DISPLAY "NO" END-IF."#),
        vec!["YES"]
    );
}

/// A character the alphabet never mentions sorts after every one it does, so
/// `9` is below both `SPACE` and `QUOTE` here (NC215A SEQ-TEST-GF-6/GF-7).
#[test]
fn unlisted_characters_sort_after_every_listed_one() {
    assert_eq!(
        wild_one(
            r#"    IF NINE-DU-9V0-1 < SPACE DISPLAY "YES" ELSE DISPLAY "NO" END-IF.
    IF NINE-DU-9V0-1 < QUOTE DISPLAY "YES2" ELSE DISPLAY "NO2" END-IF."#
        ),
        vec!["YES", "YES2"]
    );
}

/// NC219A's alphabet: a figurative constant used as an alphabet operand names
/// its **native** character, so `ALSO HIGH-VALUE` gives `0xFF` the position it
/// is written at — while the program's own `LOW-VALUE` becomes `"F"`, the
/// character written first (NC219A SEQ-TEST-GF-2/GF-4).
#[test]
fn figurative_operands_name_native_characters() {
    let out = run_capture(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. COLLSEQ1.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
OBJECT-COMPUTER.
    XX-SYS
    PROGRAM COLLATING SEQUENCE IS COLLATING-SEQ-1.
SPECIAL-NAMES.
    ALPHABET
    COLLATING-SEQ-1 IS "F" "U" "N"
        ALSO HIGH-VALUE
        ALSO LOW-VALUE
        "Y".
DATA DIVISION.
WORKING-STORAGE SECTION.
01 F-AN-1 PIC A VALUE "F".
01 U-AN-1 PIC A VALUE "U".
01 N-AN-1 PIC A VALUE "N".
PROCEDURE DIVISION.
MAIN-PARA.
    IF U-AN-1 < N-AN-1 DISPLAY "YES" ELSE DISPLAY "NO" END-IF.
    IF F-AN-1 = LOW-VALUE DISPLAY "YES2" ELSE DISPLAY "NO2" END-IF.
    STOP RUN.
"#,
    );
    assert_eq!(out, vec!["YES", "YES2"]);
}

/// Without a `PROGRAM COLLATING SEQUENCE` clause nothing changes: native
/// ordering stays in force and `LOW-VALUE` is still `0x00`.
#[test]
fn native_ordering_is_untouched_without_the_clause() {
    let out = run_capture(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. NATORD.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 A-AN-1 PIC A VALUE "A".
01 ZERO-DU-9V0-1 PIC 9 VALUE ZERO.
PROCEDURE DIVISION.
MAIN-PARA.
    IF A-AN-1 > ZERO-DU-9V0-1 DISPLAY "YES" ELSE DISPLAY "NO" END-IF.
    IF A-AN-1 = LOW-VALUE DISPLAY "YES2" ELSE DISPLAY "NO2" END-IF.
    STOP RUN.
"#,
    );
    assert_eq!(out, vec!["YES", "NO2"]);
}

/// A comparison with one alphanumeric operand is an alphanumeric comparison —
/// the numeric one is read as its characters. Coercing the text side to a
/// number instead made every non-numeric string equal to zero.
#[test]
fn text_that_is_not_a_number_compares_alphanumerically() {
    let out = run_capture(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. XTYPE.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-TEXT PIC X(6) VALUE "banana".
01 WS-DIGITS PIC X(3) VALUE "042".
01 WS-NUM PIC 9(3) VALUE 42.
PROCEDURE DIVISION.
MAIN-PARA.
    IF WS-TEXT = 0 DISPLAY "EQ" ELSE DISPLAY "NE" END-IF.
    IF WS-DIGITS = WS-NUM DISPLAY "EQ2" ELSE DISPLAY "NE2" END-IF.
    STOP RUN.
"#,
    );
    assert_eq!(out, vec!["NE", "EQ2"]);
}
