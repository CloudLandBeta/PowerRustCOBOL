// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A numeric literal moved to an alphanumeric receiver transfers the characters
//! **as written** — NIST CCVS85 NC202A.
//!
//! `MOVE 060820000200 TO CORR-DATA-2`, where the receiver is six `PIC 99`
//! children, fills them `06 08 20 00 02 00`. The lexer kept only the literal's
//! *value*, so the leading zero was gone and the eleven remaining digits
//! shifted every child one position left: `60 82 00 00 20 0`, and the
//! `ADD CORRESPONDING` that followed computed 63 where 09 was correct.
//!
//! The written width now rides along — as a **digit count**, not text, so
//! nothing allocates: `Token::IntegerLiteral(i64, u8)` and, for the literals
//! that need it, `Literal::IntegerDigits(i64, u8)`. A literal whose value
//! renders back to what was written stays `Literal::Integer`, so the ordinary
//! case keeps the shape everything already handles.
//!
//! ⚠️ The receiver's width never enters into it. Padding to the *receiver*
//! would turn `MOVE 2 TO <PIC X(4)>` into `"0002"`; COBOL requires `"2   "`.

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

/// NC202A `ADD-INIT-F3-7`, reduced to the move that feeds it.
#[test]
fn a_literal_with_a_leading_zero_keeps_it_across_a_group() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. NUMLIT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 CORR-DATA-2.
          03 XYZ-1 PICTURE IS 99 VALUE IS ZERO.
          03 XYZ-2 PICTURE IS 99 VALUE IS ZERO.
          03 XYZ-3 PICTURE IS 99 VALUE IS ZERO.
          03 XYZ-4 PICTURE IS 99 VALUE IS ZERO.
          03 XYZ-5 PICTURE IS 99 VALUE IS ZERO.
          03 XYZ-6 PICTURE IS 99 VALUE IS ZERO.
       PROCEDURE DIVISION.
       MAIN.
           MOVE 060820000200 TO CORR-DATA-2.
           DISPLAY XYZ-1 "/" XYZ-2 "/" XYZ-3 "/"
                   XYZ-4 "/" XYZ-5 "/" XYZ-6.
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "06/08/20/00/02/00", "{out:#?}");
}

/// The trap on the other side of it: a literal that needs no padding must not
/// acquire any. Padding to the *receiver* is what would break this.
#[test]
fn a_literal_without_leading_zeros_is_left_justified_and_space_padded() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. NUMLIT2.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-X4 PIC X(4).
       PROCEDURE DIVISION.
       MAIN.
           MOVE 2 TO WS-X4.
           DISPLAY "[" WS-X4 "]".
           MOVE 0012 TO WS-X4.
           DISPLAY "[" WS-X4 "]".
           MOVE 12 TO WS-X4.
           DISPLAY "[" WS-X4 "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "[2   ]", "a bare 2 must not become 0002: {out:#?}");
    assert_eq!(out[1], "[0012]", "{out:#?}");
    assert_eq!(out[2], "[12  ]", "{out:#?}");
}

/// The digit count is presentation, not value: arithmetic is unaffected, and a
/// numeric receiver takes the number.
#[test]
fn the_written_width_never_changes_the_value() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. NUMLIT3.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N PIC 9(4).
       01 WS-R PIC 9(6).
       PROCEDURE DIVISION.
       MAIN.
           MOVE 0012 TO WS-N.
           DISPLAY "N=" WS-N.
           COMPUTE WS-R = 0012 * 10.
           DISPLAY "R=" WS-R.
           IF 0012 = 12 DISPLAY "EQ" ELSE DISPLAY "NE".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "N=0012", "{out:#?}");
    assert_eq!(out[1], "R=000120", "{out:#?}");
    assert_eq!(out[2], "EQ", "{out:#?}");
}

/// A `VALUE` clause is the same move: an alphanumeric item initialised from a
/// literal written with leading zeros keeps them.
#[test]
fn a_value_clause_keeps_the_literals_written_digits() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. NUMLIT4.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A PIC X(4) VALUE 0012.
       01 WS-B PIC X(4) VALUE 12.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "A=[" WS-A "]".
           DISPLAY "B=[" WS-B "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "A=[0012]", "{out:#?}");
    assert_eq!(out[1], "B=[12  ]", "{out:#?}");
}

/// A picture built from digits is one integer token too, and its leading zeros
/// are picture characters. This is the same defect in the DATA DIVISION, fixed
/// long before by reading the token's span; it now reads the digit count and
/// must keep working.
#[test]
fn a_picture_of_digit_characters_keeps_its_leading_zeros() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. NUMLIT5.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-E PIC 090909.
       01 WS-W PIC 9(06).
       PROCEDURE DIVISION.
       MAIN.
           MOVE 123 TO WS-E.
           DISPLAY "E=[" WS-E "]".
           MOVE 123456 TO WS-W.
           DISPLAY "W=[" WS-W "]".
           STOP RUN.
"#,
    );
    // Six character positions: three digits with a `0` insertion between each.
    assert_eq!(out[0].len(), "E=[".len() + 6 + 1, "{out:#?}");
    // `9(06)` is a repeat count, not a picture of digits — six digit positions.
    assert_eq!(out[1], "W=[123456]", "{out:#?}");
}
