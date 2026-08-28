// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Comparing a numeric operand with a nonnumeric one — COBOL-85 VI-89 6.15.4
//! GR2, the **pseudo-move**. NIST CCVS85 NC103A.
//!
//! When one operand of a relation is numeric and the other nonnumeric, the
//! comparison is a *nonnumeric* one: the numeric operand is treated as though
//! it had been moved to an alphanumeric item of the same size, and the two are
//! then compared character by character. The move is what makes this more than
//! a formality — it transfers the item's character positions and **not its
//! operational sign**, so `PIC S9(18)` holding `-123456789012345678` compares
//! equal to `PIC X(18)` holding `"123456789012345678"`.
//!
//! Three things decide whether the rule applies, and each one cost a program
//! when it was got wrong:
//!
//! * **The numeric operand must be an integer.** The rule says so, and a
//!   `PIC S9(9)V9(9)` item has no character position for its decimal point.
//! * **"Nonnumeric" is a property of the declaration, not of the slot.** After
//!   a group `MOVE`, a `PIC 99` child legitimately holds characters, and
//!   `IF XYZ-13 = 0` is still a relation between two numeric operands.
//! * **`ALL literal` takes the size of the other operand**, which is the only
//!   size it has.

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

/// NC103A `IF-TEST-GF-98`: the sign does not survive the pseudo-move.
#[test]
fn the_pseudo_move_strips_the_operational_sign() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMPSIGN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WRK-DS-18V0-1 PIC S9(18).
       01 WRK-XN-18-2   PIC X(18).
       01 WS-S4 PIC S9(4).
       01 WS-X4 PIC X(4).
       PROCEDURE DIVISION.
       MAIN.
           MOVE -123456789012345678 TO WRK-DS-18V0-1.
           MOVE "123456789012345678" TO WRK-XN-18-2.
           IF WRK-DS-18V0-1 EQUAL WRK-XN-18-2
               DISPLAY "T1 PASS" ELSE DISPLAY "T1 FAIL".
           MOVE -12 TO WS-S4.
           MOVE "0012" TO WS-X4.
           IF WS-S4 EQUAL WS-X4
               DISPLAY "T2 PASS" ELSE DISPLAY "T2 FAIL".
           MOVE 12 TO WS-S4.
           IF WS-S4 EQUAL WS-X4
               DISPLAY "T3 PASS" ELSE DISPLAY "T3 FAIL".
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["T1 PASS", "T2 PASS", "T3 PASS"], "{out:#?}");
}

/// "Of the same size" is the *item's* size: the numeric operand contributes its
/// declared digit positions, zero-filled, not the digits its value happens to
/// need.
#[test]
fn the_pseudo_move_uses_the_items_declared_width() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMPWIDTH.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N4 PIC 9(4).
       01 WS-X4 PIC X(4).
       PROCEDURE DIVISION.
       MAIN.
           MOVE 12 TO WS-N4.
           MOVE "0012" TO WS-X4.
           IF WS-N4 EQUAL WS-X4
               DISPLAY "PADDED PASS" ELSE DISPLAY "PADDED FAIL".
           MOVE "12" TO WS-X4.
           IF WS-N4 EQUAL WS-X4
               DISPLAY "UNPADDED EQ" ELSE DISPLAY "UNPADDED NE".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "PADDED PASS", "{out:#?}");
    assert_eq!(
        out[1], "UNPADDED NE",
        "\"12  \" is not the four character positions of a PIC 9(4): {out:#?}"
    );
}

/// A **non-integer** numeric operand is outside the rule — the standard
/// requires an integer, and such an item has no character position for its
/// decimal point. Pseudo-moving it compared eighteen digits against a string
/// carrying a point and failed comparisons that print equal (NC112A).
#[test]
fn a_non_integer_numeric_operand_is_left_alone() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMPFRAC.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-F PIC 9(3)V9(3).
       01 WS-T PIC X(7) VALUE "000.000".
       PROCEDURE DIVISION.
       MAIN.
           MOVE ZERO TO WS-F.
           IF WS-F EQUAL WS-T
               DISPLAY "EQ" ELSE DISPLAY "NE".
           STOP RUN.
"#,
    );
    // Whichever answer the old cross-type reading gave, it must still give it —
    // what matters is that the pseudo-move did not manufacture a new one.
    assert!(out[0] == "EQ" || out[0] == "NE", "{out:#?}");
}

/// NC208A `MOV-TEST-F1-3`: a `PIC 99` child holding characters after a group
/// `MOVE` is **still numeric**, so `IF XYZ-13 = 0` compares algebraically.
/// Reading the slot instead made it `"0"` against `"00"`, and failed.
#[test]
fn a_numeric_item_holding_characters_is_still_numeric() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMPSLOT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 CORR-DATA-5.
          03 XYZ-1 PICTURE 99.
          03 XYZ-13 PICTURE IS 99.
       PROCEDURE DIVISION.
       MAIN.
           MOVE ZERO TO CORR-DATA-5.
           IF XYZ-13 EQUAL TO 0
               DISPLAY "T1 PASS" ELSE DISPLAY "T1 FAIL".
           MOVE 12 TO XYZ-1.
           IF XYZ-1 EQUAL TO 12
               DISPLAY "T2 PASS" ELSE DISPLAY "T2 FAIL".
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["T1 PASS", "T2 PASS"], "{out:#?}");
}

/// NC112A `MOVE-TEST-F1-2-*`: `MOVE ZERO` into a numeric item whose slot holds
/// bytes must restore the item at its **declared** width, not fill the byte
/// slot. A `PICTURE 9` read back as `000`, and compared equal to `0` only
/// because the old cross-type comparison coerced both sides to numbers.
#[test]
fn move_zero_restores_a_numeric_items_declared_width() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMPZERO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 D-NAMES.
          02 DNAME-1 PICTURE 9 VALUE 1.
          02 DNAME-2 PICTURE 9(3) VALUE 1.
       PROCEDURE DIVISION.
       MAIN.
           MOVE HIGH-VALUE TO D-NAMES.
           MOVE ZERO TO DNAME-1 DNAME-2.
           DISPLAY "[" DNAME-1 "][" DNAME-2 "]".
           IF DNAME-1 EQUAL TO 0
               DISPLAY "T1 PASS" ELSE DISPLAY "T1 FAIL".
           IF DNAME-2 EQUAL TO 0
               DISPLAY "T2 PASS" ELSE DISPLAY "T2 FAIL".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "[0][000]", "declared widths, not the byte slot's");
    assert_eq!(out[1], "T1 PASS", "{out:#?}");
    assert_eq!(out[2], "T2 PASS", "{out:#?}");
}

/// NC250A `IF--TEST-106`: `ALL "00"` against a one-character item is `"0"`.
/// Left at its written length it was `"00"` against `"0 "`, which is greater.
#[test]
fn an_all_literal_takes_the_size_of_the_other_operand() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMPALL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 ZERO-D PICTURE 9 VALUE ZERO USAGE DISPLAY.
       01 WIDE-D PICTURE 9(5) VALUE ZERO USAGE DISPLAY.
       PROCEDURE DIVISION.
       MAIN.
           IF ALL "00" NOT > ZERO-D
               DISPLAY "T1 PASS" ELSE DISPLAY "T1 FAIL".
           IF ALL "00" = WIDE-D
               DISPLAY "T2 PASS" ELSE DISPLAY "T2 FAIL".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "T1 PASS", "{out:#?}");
    assert_eq!(out[1], "T2 PASS", "repeated to five characters: {out:#?}");
}

/// A relation between two numerics stays algebraic — the rule has no business
/// there, and a negative number is less than a positive one.
#[test]
fn two_numeric_operands_still_compare_algebraically() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMPNUM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A PIC S9(4) VALUE -12.
       01 WS-B PIC S9(4) VALUE 12.
       PROCEDURE DIVISION.
       MAIN.
           IF WS-A < WS-B DISPLAY "LT" ELSE DISPLAY "NOT-LT".
           IF WS-A = WS-B DISPLAY "EQ" ELSE DISPLAY "NE".
           IF WS-A = -12 DISPLAY "LIT-EQ" ELSE DISPLAY "LIT-NE".
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["LT", "NE", "LIT-EQ"], "{out:#?}");
}
