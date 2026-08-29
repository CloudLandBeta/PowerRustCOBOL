// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Figurative constants, and the bytes they are made of. NIST CCVS85 NC211A
//! and NC217A.
//!
//! Two rules meet here, and both were missing.
//!
//! **A figurative constant has no length of its own.** It takes the size of
//! whatever it is written against: `VALUE ALL "ABC"` fills its item, and
//! `IF QT = QUOTE` compares against as many quotation marks as `QT` has
//! character positions. `SPACE` and `ZERO` never showed the gap — the space
//! padding a short operand already receives happens to produce what `SPACE`
//! would have repeated, and `ZERO` compares algebraically — so `QUOTE`,
//! `HIGH-VALUE` and `LOW-VALUE` carried it alone.
//!
//! **`HIGH-VALUE` is the byte `0xFF`, which is not a character.** It has no
//! UTF-8 spelling one byte wide, so every path that carried a record through a
//! Rust `String` turned it into a three-byte U+FFFD and shifted everything
//! after it by two. That is a *silent* corruption: the item still reads as
//! "some high value", and only a field further along the record shows it.

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

/// NC211A FIG-TEST-1 / FIG-TEST-2: `VALUE ALL "literal"` repeats its unit
/// across the whole item.
///
/// Every other figurative constant is one character and the item is filled with
/// it; `ALL` is the only one whose unit can be wider than a byte, and it fell
/// through to the item's default — spaces.
#[test]
fn value_all_literal_fills_the_item() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FIGVAL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 D PICTURE X(6) VALUE ALL "ABC".
       01 E PICTURE XXX  VALUE ALL "Z".
       01 W PICTURE X(9) VALUE ALL "XY".
       01 P PICTURE X(4) VALUE ALL "AB".
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "D=[" D "]".
           DISPLAY "E=[" E "]".
           DISPLAY "W=[" W "]".
           DISPLAY "P=[" P "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "D=[ABCABC]", "an exact multiple of the unit");
    assert_eq!(out[1], "E=[ZZZ]", "a one-character unit");
    assert_eq!(out[2], "W=[XYXYXYXYX]", "a partial unit at the end");
    assert_eq!(out[3], "P=[ABAB]");
}

/// `ALL` in front of another figurative constant is redundant, and the parser
/// folds it away — so these keep working exactly as the bare spelling does.
#[test]
fn value_all_figurative_is_the_bare_figurative() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FIGALL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A PICTURE XXX VALUE ALL QUOTES.
       01 B PICTURE XXX VALUE QUOTES.
       01 C PICTURE XXX VALUE ALL SPACES.
       01 D PICTURE XXX VALUE ALL ZEROES.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "A=[" A "]".
           DISPLAY "B=[" B "]".
           DISPLAY "C=[" C "]".
           DISPLAY "D=[" D "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "A=[\"\"\"]");
    assert_eq!(out[1], "B=[\"\"\"]");
    assert_eq!(out[2], "C=[   ]");
    assert_eq!(out[3], "D=[000]");
}

/// NC211A FIG-TEST-3: a figurative constant in a relation is repeated to the
/// size of the other operand.
///
/// At one character, `QUOTE` was compared against `QT`'s three as `"` padded
/// with spaces — `"  ` — which is not equal.
#[test]
fn a_figurative_constant_is_sized_to_the_other_operand() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FIGCMP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SP PICTURE XXX VALUE SPACES.
       01 ZR PICTURE XXX VALUE ZEROS.
       01 QT PICTURE XXX VALUE QUOTES.
       01 HV PICTURE XXX VALUE HIGH-VALUES.
       01 LV PICTURE XXX VALUE LOW-VALUES.
       PROCEDURE DIVISION.
       MAIN.
           IF SP = SPACE  DISPLAY "SP ok" ELSE DISPLAY "SP BAD".
           IF ZR = ZERO   DISPLAY "ZR ok" ELSE DISPLAY "ZR BAD".
           IF QT = QUOTE  DISPLAY "QT ok" ELSE DISPLAY "QT BAD".
           IF QT = QUOTES DISPLAY "QTS ok" ELSE DISPLAY "QTS BAD".
           IF HV = HIGH-VALUE  DISPLAY "HV ok" ELSE DISPLAY "HV BAD".
           IF LV = LOW-VALUE   DISPLAY "LV ok" ELSE DISPLAY "LV BAD".
           IF QT NOT EQUAL TO SPACE DISPLAY "NE ok" ELSE DISPLAY "NE BAD".
           STOP RUN.
"#,
    );
    assert_eq!(
        out,
        vec!["SP ok", "ZR ok", "QT ok", "QTS ok", "HV ok", "LV ok", "NE ok"],
        "every figurative constant repeats to the item's width, and an \
         unequal one still compares unequal"
    );
}

/// NC211A FIG-TEST-3, the half that made it fail: a group `MOVE` carries the
/// record's **bytes**.
///
/// `EL-K` holds three `0xFF`. Read through a `String` those became three
/// U+FFFD — nine bytes — so `EL-L`, `EL-M` and `EL-N` were distributed out of
/// the middle of the mangled run and every one of them was wrong. `EL-N` came
/// out right by coincidence, which is exactly how a shifted record hides.
#[test]
fn a_group_move_carries_high_values_byte_for_byte() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FIGGRP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 GRP.
          02 SUB-A.
             03 EL-I PICTURE XXX VALUE QUOTES.
             03 EL-J PICTURE XXX VALUE ALL QUOTES.
             03 EL-K PICTURE XXX VALUE ALL HIGH-VALUES.
             03 EL-L PICTURE XXX VALUE ALL LOW-VALUES.
             03 EL-M PICTURE XXX VALUE HIGH-VALUES.
             03 EL-N PICTURE XXX VALUE LOW-VALUES.
          02 SUB-B.
             03 SUB-BC.
                04 EL-I PICTURE XXX.
                04 EL-J PICTURE XXX.
                04 EL-K PICTURE XXX.
                04 EL-L PICTURE XXX.
                04 EL-M PICTURE XXX.
                04 EL-N PICTURE XXX.
       PROCEDURE DIVISION.
       MAIN.
           MOVE SUB-A TO SUB-B.
           IF EL-I OF SUB-BC = QUOTE      DISPLAY "I ok" ELSE DISPLAY "I BAD".
           IF EL-J OF SUB-BC = QUOTE      DISPLAY "J ok" ELSE DISPLAY "J BAD".
           IF EL-K OF SUB-BC = HIGH-VALUE DISPLAY "K ok" ELSE DISPLAY "K BAD".
           IF EL-L OF SUB-BC = LOW-VALUE  DISPLAY "L ok" ELSE DISPLAY "L BAD".
           IF EL-M OF SUB-BC = HIGH-VALUE DISPLAY "M ok" ELSE DISPLAY "M BAD".
           IF EL-N OF SUB-BC = LOW-VALUE  DISPLAY "N ok" ELSE DISPLAY "N BAD".
           STOP RUN.
"#,
    );
    assert_eq!(
        out,
        vec!["I ok", "J ok", "K ok", "L ok", "M ok", "N ok"],
        "the six elements must land on their own byte positions"
    );
}

/// A group `MOVE` of a record with a `0xFF` in it keeps every **later** field
/// where it belongs — the shift, stated directly.
#[test]
fn a_high_value_does_not_shift_the_fields_after_it() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FIGSHIFT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SRC.
          02 S1 PICTURE XXX VALUE HIGH-VALUES.
          02 S2 PICTURE X(5) VALUE "TAIL!".
       01 DST.
          02 D1 PICTURE XXX.
          02 D2 PICTURE X(5).
       PROCEDURE DIVISION.
       MAIN.
           MOVE SRC TO DST.
           DISPLAY "D2=[" D2 "]".
           STOP RUN.
"#,
    );
    assert_eq!(
        out[0], "D2=[TAIL!]",
        "the three 0xFF bytes must consume three positions, not nine"
    );
}

/// NC217A STR-TEST-GF-9: `STRING HIGH-VALUE DELIMITED BY SIZE` moves **one**
/// character position, so four of the receiver's five survive.
///
/// The expected value is built as a record rather than written with reference
/// modification: `ID7-XN-5 (1:1)` still reads through the lossy text form, so a
/// refmod assertion here would be testing a path this change does not touch.
#[test]
fn string_high_value_moves_one_character_position() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. STRHV.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 ID7-XN-5  PICTURE X(5).
       01 ID8-DU-2V0 PICTURE 99.
       01 EXPECTED.
          02 E-HEAD PICTURE X    VALUE HIGH-VALUES.
          02 E-TAIL PICTURE X(4) VALUE "****".
       PROCEDURE DIVISION.
       MAIN.
           MOVE "*****" TO ID7-XN-5.
           MOVE 1 TO ID8-DU-2V0.
           STRING HIGH-VALUE DELIMITED BY SIZE INTO ID7-XN-5
               POINTER ID8-DU-2V0.
           IF ID8-DU-2V0 = 2 DISPLAY "PTR ok" ELSE DISPLAY "PTR BAD".
           IF ID7-XN-5 = EXPECTED DISPLAY "FIELD ok"
               ELSE DISPLAY "FIELD BAD".
           STOP RUN.
"#,
    );
    assert_eq!(
        out,
        vec!["PTR ok", "FIELD ok"],
        "one byte placed, the pointer advanced by one, four asterisks left"
    );
}

// ── MOVE HIGH-VALUE: the fill reaches the whole receiver ──────────────────────
//
// The two rules in this file's header meet again in `MOVE`, from NC105A.
// `HIGH-VALUE` fills its receiver exactly as `SPACE` does, but `0xFF` cannot
// ride the string-fill path, so it was left to `CobolValue::assign` — one byte
// laid down and the rest padded with **spaces**. A group receiver fared worse:
// the value went through `as_display_string` and arrived as U+FFFD's three
// bytes. Reading a group back had the same defect in the other direction, so a
// group filled with `HIGH-VALUE` compared unequal to `HIGH-VALUE`.

/// NC105A `MOVE-TEST-F1-67`: the fill reaches every byte of an elementary
/// receiver, and the item then equals a `VALUE HIGH-VALUE` item of its width.
#[test]
fn move_high_value_fills_an_elementary_receiver() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. HVFILL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 PLAIN-10 PICTURE X(10).
       01 HIGH-10  PICTURE X(10) VALUE HIGH-VALUE.
       PROCEDURE DIVISION.
       MAIN.
           MOVE HIGH-VALUE TO PLAIN-10.
           IF PLAIN-10 = HIGH-10 DISPLAY "TEN ok" ELSE DISPLAY "TEN BAD".
           IF PLAIN-10 = HIGH-VALUE DISPLAY "FIG ok" ELSE DISPLAY "FIG BAD".
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["TEN ok", "FIG ok"], "{out:#?}");
}

/// NC105A `MOVE-TEST-F1-67` proper: the receiver is a **group**, so the fill is
/// distributed across its children — ten bytes, not the three of one U+FFFD.
/// Reading the group back has to return those bytes, or the comparison against
/// the figurative constant sizes itself against a width three times too wide.
#[test]
fn move_high_value_fills_a_group_receiver() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. HVGRP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 GRP-WRK-DU-10V00.
          05 WRK-DU-10V00 PICTURE 9(10).
       01 HIGH-10 PICTURE X(10) VALUE HIGH-VALUE.
       PROCEDURE DIVISION.
       MAIN.
           MOVE HIGH-VALUE TO GRP-WRK-DU-10V00.
           IF GRP-WRK-DU-10V00 = HIGH-VALUE
               DISPLAY "FIG ok" ELSE DISPLAY "FIG BAD".
           IF GRP-WRK-DU-10V00 = HIGH-10
               DISPLAY "ITEM ok" ELSE DISPLAY "ITEM BAD".
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["FIG ok", "ITEM ok"], "{out:#?}");
}

/// NC105A `MOVE-TEST-F1-69`: an alphanumeric-**edited** receiver still places
/// its insertion characters — the fill only reaches the source positions.
/// `PIC XX0XXBXXX` holds `FF FF '0' FF FF ' ' FF FF FF`.
#[test]
fn move_high_value_into_an_edited_receiver_keeps_the_insertions() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. HVEDIT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 GRP-AE-0002.
          05 AE-0002 PICTURE XX0XXBXXX.
       01 HIGH-VALUE-EDIT.
          02 HIGH-1 PICTURE XX  VALUE HIGH-VALUE.
          02 FILLER PICTURE 9   VALUE 0.
          02 HIGH-2 PICTURE XX  VALUE HIGH-VALUE.
          02 FILLER PICTURE X   VALUE SPACE.
          02 HIGH-3 PICTURE XXX VALUE HIGH-VALUE.
       PROCEDURE DIVISION.
       MAIN.
           MOVE HIGH-VALUE TO AE-0002.
           IF GRP-AE-0002 = HIGH-VALUE-EDIT
               DISPLAY "EDIT ok" ELSE DISPLAY "EDIT BAD".
           IF GRP-AE-0002 = HIGH-VALUE
               DISPLAY "ALL-HIGH" ELSE DISPLAY "NOT ALL-HIGH".
           STOP RUN.
"#,
    );
    assert_eq!(
        out,
        vec!["EDIT ok", "NOT ALL-HIGH"],
        "the '0' and the space are the item's own, not the sender's"
    );
}

/// `LOW-VALUE` never showed the defect — `0x00` is one valid UTF-8 byte — and
/// must keep working through the byte path. Pinning the pair so a later change
/// to one is measured against the other.
#[test]
fn move_low_value_still_fills_its_receiver() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. LVFILL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 GRP-LOW.
          05 LOW-CHILD PICTURE 9(10).
       01 LOW-10 PICTURE X(10) VALUE LOW-VALUE.
       PROCEDURE DIVISION.
       MAIN.
           MOVE LOW-VALUE TO GRP-LOW.
           IF GRP-LOW = LOW-VALUE DISPLAY "FIG ok" ELSE DISPLAY "FIG BAD".
           IF GRP-LOW = LOW-10 DISPLAY "ITEM ok" ELSE DISPLAY "ITEM BAD".
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["FIG ok", "ITEM ok"], "{out:#?}");
}
