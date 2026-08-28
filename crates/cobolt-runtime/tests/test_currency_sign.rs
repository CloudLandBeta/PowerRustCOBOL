// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `SPECIAL-NAMES. CURRENCY SIGN`, and the two defects behind it — NIST NC108M.
//!
//! NC108M was the suite's last Nucleus compile failure, recorded as needing an
//! "implementor-defined editing picture". It needs no such thing. The program
//! declares `CURRENCY "<"` in SPECIAL-NAMES, so `PICTURE <(3),<<<.99` is an
//! ordinary floating-currency picture with `<` in the role `$` normally plays —
//! plain COBOL-85, and a gap rather than an extension.
//!
//! The template keeps spelling a currency position `$` whatever the program
//! calls it. `$` is the internal marker for "currency position", so every width
//! and digit-count rule stays written once and only the formatter substitutes.
//!
//! Making NC108M compile then exposed two defects that had nothing to do with
//! currency, and both are guarded here:
//!
//! * **A REDEFINES overlay mis-sized a numeric-edited item.** The record layout
//!   took `digits + decimals` as the item's byte width, which counts *digit
//!   positions* — for `PIC $$$,$$$.99` that is two, against ten characters
//!   actually stored, because `analyze_pic` splits on `V` and this picture's
//!   separator is a real `.`. Every field after it in the record shifted.
//! * **An alphanumeric `VALUE` on a numeric item changed the item's category.**
//!   `PICTURE IS 9 VALUE IS "5"` stored a string, so every rule that asks
//!   whether the item is numeric answered no — `BLANK WHEN ZERO` among them.

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

// ── CURRENCY SIGN ────────────────────────────────────────────────────────────

/// NC108M's own declaration: `CURRENCY "<"` with `PICTURE <(3),<<<.99`.
#[test]
fn a_declared_currency_symbol_edits_and_floats() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CURR.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           CURRENCY "<".
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 FL-LESS PICTURE <(3),<<<.99 VALUE " <1,111.11".
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "INIT=[" FL-LESS "]".
           MOVE ZERO TO FL-LESS.
           DISPLAY "ZERO=[" FL-LESS "]".
           MOVE 1234 TO FL-LESS.
           DISPLAY "MOVE=[" FL-LESS "]".
           STOP RUN.
"#);
    assert_eq!(out[0], "INIT=[ <1,111.11]", "{out:#?}");
    // NC108M ABR-TEST-GF-10 compares exactly this against "      <.00".
    assert_eq!(out[1], "ZERO=[      <.00]", "{out:#?}");
    assert_eq!(out[2], "MOVE=[ <1,234.00]", "{out:#?}");
}

/// `CURRENCY SIGN IS` in full, and a fixed (non-floating) currency position.
#[test]
fn the_sign_and_is_words_are_optional() {
    // `r##` — the currency symbol under test is `#`, which would close an `r#`.
    let out = run(r##"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CURRFULL.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           CURRENCY SIGN IS "#".
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 AMT PICTURE #ZZ9.99 VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN.
           MOVE 123.45 TO AMT.
           DISPLAY "[" AMT "]".
           STOP RUN.
"##);
    assert_eq!(out[0], "[#123.45]", "{out:#?}");
}

/// With no `CURRENCY` clause the symbol is `$`, exactly as before.
#[test]
fn the_default_currency_symbol_is_the_dollar() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CURRDEF.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 AMT PICTURE $(3),$$$.99 VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN.
           MOVE 1111.11 TO AMT.
           DISPLAY "[" AMT "]".
           STOP RUN.
"#);
    assert_eq!(out[0], "[ $1,111.11]", "{out:#?}");
}

// ── the REDEFINES width defect NC108M exposed ────────────────────────────────

/// NC108M `FMT-TEST-GF-1`: a `JUSTIFIED RIGHT` table redefines a group whose
/// tail is a ten-character numeric-edited item. Occurrence 3 covers the item's
/// first five characters.
///
/// The overlay used to size that item at **two** bytes — its digit-position
/// count — so it read `<` where ` <1,1` was stored.
#[test]
fn a_redefines_overlay_sizes_a_numeric_edited_item_by_its_edited_width() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RDFEDIT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COMPLETE-01.
          02 COMPLETE-F.
             03 FILLER PICTURE X(10) VALUE SPACE.
             03 FL-AMT PICTURE $(3),$$$.99 VALUE " $1,111.11".
          02 COMPLETE-FORMAT REDEFINES COMPLETE-F
             JUSTIFIED RIGHT PICTURE X(5) OCCURS 4 TIMES.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "F3=[" COMPLETE-FORMAT (3) "]".
           DISPLAY "F4=[" COMPLETE-FORMAT (4) "]".
           STOP RUN.
"#);
    assert_eq!(out[0], "F3=[ $1,1]", "{out:#?}");
    assert_eq!(out[1], "F4=[11.11]", "{out:#?}");
}

/// The plain-alphanumeric case was always right and must stay so: the fix
/// changes the width of numeric-edited items only.
#[test]
fn a_redefines_overlay_over_plain_text_is_unchanged() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RDFTEXT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 GRP-B.
          02 B-F.
             03 FILLER PICTURE X(10) VALUE SPACE.
             03 B-TEXT PICTURE X(10) VALUE " $1,111.11".
          02 B-FMT REDEFINES B-F PICTURE X(5) OCCURS 4 TIMES.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "F3=[" B-FMT (3) "]".
           STOP RUN.
"#);
    assert_eq!(out[0], "F3=[ $1,1]", "{out:#?}");
}

// ── the VALUE-category defect NC108M exposed ─────────────────────────────────

/// NC108M `FMT-TEST-GF-3`: `MORE-COMPLETE-FORMAT` is
/// `BLANK WHEN ZERO PICTURE IS 9 … VALUE IS "5"`, and after `MOVE ZERO` it must
/// compare equal to `SPACE`.
///
/// The alphanumeric `VALUE` used to store a string in place of the item's
/// number, and `BLANK WHEN ZERO` is applied on the numeric display path — so
/// the item kept its digit.
#[test]
fn an_alphanumeric_value_on_a_numeric_item_keeps_it_numeric() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. VALCAT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 MORE-COMPLETE-FORMAT
              BLANK WHEN ZERO
              PICTURE IS 9
              SYNCHRONIZED RIGHT
              DISPLAY
              VALUE IS "5".
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "INIT=[" MORE-COMPLETE-FORMAT "]".
           IF MORE-COMPLETE-FORMAT NOT EQUAL TO "5"
              DISPLAY "NOT-FIVE"
           ELSE
              DISPLAY "IS-FIVE".
           MOVE ZERO TO MORE-COMPLETE-FORMAT.
           IF MORE-COMPLETE-FORMAT EQUAL TO SPACE
              DISPLAY "BLANKED"
           ELSE
              DISPLAY "GOT=[" MORE-COMPLETE-FORMAT "]".
           STOP RUN.
"#);
    assert_eq!(out[0], "INIT=[5]", "{out:#?}");
    assert_eq!(out[1], "IS-FIVE", "{out:#?}");
    assert_eq!(out[2], "BLANKED", "{out:#?}");
}

/// The item stays arithmetic, not merely printable.
#[test]
fn an_alphanumeric_value_on_a_numeric_item_still_computes() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. VALARITH.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 N-ITEM PICTURE 999V99 VALUE IS "12.50".
       01 N-OUT  PICTURE 999V99 VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN.
           ADD 1 TO N-ITEM GIVING N-OUT.
           DISPLAY "SUM=" N-OUT.
           STOP RUN.
"#);
    // `V` is an *implied* point: the item stores five digits and displays them
    // without a separator. 12.50 + 1 = 13.50 → `01350`.
    assert_eq!(out[0], "SUM=01350", "{out:#?}");
}

/// A `VALUE` literal that spells no number leaves the item at its default
/// rather than guessing at one.
#[test]
fn a_non_numeric_value_literal_leaves_the_default() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. VALJUNK.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 N-ITEM PICTURE 999 VALUE IS "ABC".
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "[" N-ITEM "]".
           STOP RUN.
"#);
    assert_eq!(out[0], "[000]", "{out:#?}");
}
