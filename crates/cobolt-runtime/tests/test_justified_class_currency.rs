// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `JUSTIFIED`, the `NUMERIC` class test, user-defined `CLASS`, and the
//! `CURRENCY SIGN` clause. NIST CCVS85 NC107A, NC174A and NC211A.
//!
//! Four independent defects, related only in that each one was a clause the
//! front end *accepted* and then did nothing with — the most expensive kind,
//! because the program compiles and quietly computes the wrong answer.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_fmt(src: &str, format: SourceFormat) -> Vec<String> {
    let result = parse(tokenize(src, format));
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

fn run(src: &str) -> Vec<String> {
    run_fmt(src, SourceFormat::Free)
}

/// NC107A JUST-TEST-03 / JUST-TEST-04: `JUSTIFIED RIGHT` on an **alphabetic**
/// item.
///
/// The clause was only ever recorded for `PicKind::Alphanumeric`, so
/// `PICTURE A(5) JUSTIFIED RIGHT` parsed, was forgotten, and every `MOVE` into
/// it left-aligned. Both halves of the rule matter: a short sender is pushed
/// right, and a **long** one loses its leftmost characters rather than its
/// rightmost.
#[test]
fn justified_right_applies_to_an_alphabetic_item() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. JUSTA.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 NJUST-XN-3  PICTURE X(3)  VALUE "ABC".
       01 NJUST-XN-5  PICTURE X(5)  VALUE "CDEFG".
       01 NJUST-XN-15 PICTURE X(15) VALUE "ABCDEFGHIJKLMNO".
       01 AJ-00005 PICTURE A(5) JUSTIFIED RIGHT.
       01 XJ-00005 PICTURE X(5) JUSTIFIED RIGHT.
       PROCEDURE DIVISION.
       MAIN.
           MOVE NJUST-XN-3 TO AJ-00005.
           DISPLAY "A1=[" AJ-00005 "]".
           MOVE NJUST-XN-5 TO AJ-00005.
           DISPLAY "A2=[" AJ-00005 "]".
           MOVE NJUST-XN-15 TO AJ-00005.
           DISPLAY "A3=[" AJ-00005 "]".
           MOVE NJUST-XN-3 TO XJ-00005.
           DISPLAY "X1=[" XJ-00005 "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "A1=[  ABC]", "a short sender is pushed right");
    assert_eq!(out[1], "A2=[CDEFG]", "an exact fit is unchanged");
    assert_eq!(
        out[2], "A3=[KLMNO]",
        "a long sender keeps its RIGHT end — the leftmost characters go"
    );
    assert_eq!(out[3], "X1=[  ABC]", "the alphanumeric half still works");
}

/// NC211A CC--TEST-GF-48 and NC174A CLASS-TEST-GF-8/10: the `NUMERIC` class
/// test.
///
/// An item whose PICTURE carries no operational sign is numeric only when every
/// character position holds a digit. Reading it with `parse::<f64>` accepted a
/// sign, a decimal point, an exponent and surrounding spaces — so `PICTURE X(5)`
/// holding `"+1234"` answered "numeric" and `CLASS-1 NOT NUMERIC` was false.
#[test]
fn the_numeric_class_test_wants_digits_in_every_position() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CLSNUM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 CLASS-1 PICTURE X(5).
       01 PLAIN-NUM PICTURE S9(5) VALUE +123.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "+1234" TO CLASS-1.
           IF CLASS-1 NOT NUMERIC DISPLAY "SIGN ok" ELSE DISPLAY "SIGN BAD".
           MOVE "12345" TO CLASS-1.
           IF CLASS-1 NUMERIC DISPLAY "DIGITS ok" ELSE DISPLAY "DIGITS BAD".
           MOVE "12 45" TO CLASS-1.
           IF CLASS-1 NOT NUMERIC DISPLAY "SPACE ok" ELSE DISPLAY "SPACE BAD".
           MOVE "1.234" TO CLASS-1.
           IF CLASS-1 NOT NUMERIC DISPLAY "POINT ok" ELSE DISPLAY "POINT BAD".
           MOVE "ABCDE" TO CLASS-1.
           IF CLASS-1 NOT NUMERIC DISPLAY "ALPHA ok" ELSE DISPLAY "ALPHA BAD".
           IF PLAIN-NUM NUMERIC DISPLAY "NUM ok" ELSE DISPLAY "NUM BAD".
           STOP RUN.
"#,
    );
    assert_eq!(
        out,
        vec![
            "SIGN ok", "DIGITS ok", "SPACE ok", "POINT ok", "ALPHA ok", "NUM ok"
        ],
        "only all-digits is numeric for an unsigned item, and a genuinely \
         numeric item is still numeric"
    );
}

/// NC174A CLASS-TEST-GF-8 / GF-10: a `REDEFINES` overlay carries the target's
/// bytes into a **numeric** peer.
///
/// The sync filtered every non-digit out of the characters and padded what was
/// left, so `"00ABCDEFGHI  4321 "` read back as the number `004321000000000000`
/// — and `IS NUMERIC` then answered yes about an item full of letters.
#[test]
fn a_redefines_overlay_keeps_the_bytes_in_a_numeric_peer() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RDFNUM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 XDATA-XN-00018   PICTURE X(18) VALUE "00ABCDEFGHI  4321 ".
       01 XDATA-DS-18V00-S REDEFINES XDATA-XN-00018 PICTURE S9(18).
       01 DIGITS-XN        PICTURE X(6) VALUE "123456".
       01 DIGITS-DS        REDEFINES DIGITS-XN PICTURE 9(6).
       PROCEDURE DIVISION.
       MAIN.
           MOVE "00ABCDEFGHI  4321 " TO XDATA-XN-00018.
           DISPLAY "OV=[" XDATA-DS-18V00-S "]".
           IF XDATA-DS-18V00-S NOT NUMERIC
                DISPLAY "CLS ok" ELSE DISPLAY "CLS BAD".
           IF DIGITS-DS = 123456 DISPLAY "NUM ok" ELSE DISPLAY "NUM BAD".
           STOP RUN.
"#,
    );
    assert_eq!(
        out[0], "OV=[00ABCDEFGHI  4321 ]",
        "the overlay reads the target's own characters"
    );
    assert_eq!(out[1], "CLS ok");
    assert_eq!(
        out[2], "NUM ok",
        "an overlay whose bytes DO spell digits still reads as that number"
    );
}

/// NC107A RDF-TEST-9 / RDF-TEST-10: a **01-level** `REDEFINES` may describe
/// more storage than the item it redefines, and the bytes past that item's end
/// belong to whichever description is long enough to name them.
///
/// Rendering the shorter description onto the longer peer padded the tail with
/// spaces and erased it.
#[test]
fn a_wider_01_level_redefines_keeps_its_tail() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RDFWIDE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SHORT-ONE.
          02 S-A PICTURE X(10).
       01 MEDIUM-ONE REDEFINES SHORT-ONE.
          02 M-A PICTURE X(10).
          02 M-B PICTURE X(10).
       01 LONG-ONE REDEFINES SHORT-ONE.
          02 L-A PICTURE X(10).
          02 L-B PICTURE X(10).
          02 L-C PICTURE X(10).
       PROCEDURE DIVISION.
       MAIN.
           MOVE ALL "Z" TO LONG-ONE.
           MOVE ALL "Q" TO SHORT-ONE.
      *> SHORT-ONE covers only the first ten bytes; the rest must survive.
           DISPLAY "LB=[" L-B "]".
           DISPLAY "LC=[" L-C "]".
           MOVE ALL "W" TO MEDIUM-ONE.
      *> MEDIUM-ONE covers twenty; L-C is past its end and must survive.
           DISPLAY "LB2=[" L-B "]".
           DISPLAY "LC2=[" L-C "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "LB=[ZZZZZZZZZZ]", "past SHORT-ONE's 10 bytes");
    assert_eq!(out[1], "LC=[ZZZZZZZZZZ]");
    assert_eq!(out[2], "LB2=[WWWWWWWWWW]", "inside MEDIUM-ONE's 20 bytes");
    assert_eq!(out[3], "LC2=[ZZZZZZZZZZ]", "past MEDIUM-ONE's end");
}

/// NC107A CURR-TEST-1: `CURRENCY SIGN IS "W"` makes `PICTURE WWWWW` a floating
/// currency string.
///
/// A run of the symbol arrives as **one identifier** when the symbol is a
/// letter — `WWWWW`, not five tokens — so testing for a one-character
/// identifier rejected every floating currency picture that did not use `$`.
#[test]
fn a_letter_currency_symbol_floats_across_its_whole_run() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CURRW.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           CURRENCY SIGN IS "W".
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 DATA-I PICTURE 9999 VALUE 12.
       01 DATA-J PICTURE WWWWW.
       PROCEDURE DIVISION.
       MAIN.
           MOVE DATA-I TO DATA-J.
           DISPLAY "J=[" DATA-J "]".
           IF DATA-J = "  W12" DISPLAY "CURR ok" ELSE DISPLAY "CURR BAD".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "J=[  W12]");
    assert_eq!(out[1], "CURR ok");
}

/// NC174A CLASS-TEST-GF-39/41/43: a `CLASS` operand written as an **ordinal
/// position** on its own source line.
///
/// A number that opens a line is lexed as a level number whenever it could be
/// one — and 1-49, 66, 77 and 88 all can. The ordinal of `A` is 66, so a class
/// declared that way arrived as `LevelNumber(66)`, matched nothing, and
/// described no character at all. Only the DATA DIVISION has level numbers; the
/// clause itself says what the number is. Fixed format, because that is the
/// shape the deck is written in.
#[test]
fn a_class_ordinal_operand_survives_a_line_break() {
    let out = run_fmt(
        "000100 IDENTIFICATION DIVISION.\n\
         000200 PROGRAM-ID. ORDCLS.\n\
         000300 ENVIRONMENT DIVISION.\n\
         000400 CONFIGURATION SECTION.\n\
         000500 SPECIAL-NAMES.\n\
         000600     CLASS   ORDINAL-A-ONLY IS\n\
         000700     66\n\
         000800     CLASS   ORDINAL-A-THROUGH-D IS\n\
         000900     66\n\
         001000     THROUGH\n\
         001100     69\n\
         001200     CLASS   ORDINAL-D-THRU-A\n\
         001300     69\n\
         001400     THRU\n\
         001500     66\n\
         001600     CLASS   ACTUAL-A-ONLY \"A\".\n\
         001700 DATA DIVISION.\n\
         001800 WORKING-STORAGE SECTION.\n\
         001900 01  WS-A PIC X    VALUE \"A\".\n\
         002000 01  WS-B PIC X(5) VALUE \"ADCBA\".\n\
         002100 01  WS-Z PIC X    VALUE \"Z\".\n\
         002200 PROCEDURE DIVISION.\n\
         002300 MAIN.\n\
         002400     IF WS-A ORDINAL-A-ONLY\n\
         002500        DISPLAY \"39 ok\" ELSE DISPLAY \"39 BAD\".\n\
         002600     IF WS-Z NOT ORDINAL-A-ONLY\n\
         002700        DISPLAY \"40 ok\" ELSE DISPLAY \"40 BAD\".\n\
         002800     IF WS-B ORDINAL-A-THROUGH-D\n\
         002900        DISPLAY \"41 ok\" ELSE DISPLAY \"41 BAD\".\n\
         003000     IF WS-B ORDINAL-D-THRU-A\n\
         003100        DISPLAY \"43 ok\" ELSE DISPLAY \"43 BAD\".\n\
         003200     STOP RUN.\n",
        SourceFormat::Fixed,
    );
    assert_eq!(
        out,
        vec!["39 ok", "40 ok", "41 ok", "43 ok"],
        "66 is the ordinal of A and 69 of D, in both range directions"
    );
}
