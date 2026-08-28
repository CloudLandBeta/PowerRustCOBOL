// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `ACCEPT` into a group, and `VALUE QUOTE` — NIST CCVS85 NC109M.
//!
//! Two defects the suite could not reach until the harness supplied NC109M's
//! operator input; with stdin closed every one of its comparisons failed for
//! the same uninteresting reason, and the two real bugs underneath were
//! invisible.
//!
//! **A group owns no store slot.** Its value is synthesized from its children,
//! so writing to the group's own name puts the record somewhere nothing reads
//! back. `ACCEPT` did exactly that, which left NC109M's `ACCEPT-D1` (two
//! subordinate items) and `X80-CHARACTER-FIELD` (one `FILLER`) reading empty
//! after a successful read. This is the same defect INSPECT carried for its
//! whole life until 1.62.28 — the dispatch now lives in one place,
//! `Interpreter::store_text`, so the next verb to need it does not have to
//! rediscover it.
//!
//! **`VALUE QUOTE` fills the field**, exactly as `VALUE SPACE` does. It was
//! falling through to "keep the default", so `PICTURE X VALUE QUOTE` initialised
//! to a space and NC109M's `ACCEPT-D18` compared equal to nothing.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Run `src` with `args` as the program arguments and return its DISPLAY lines.
///
/// `ACCEPT … FROM COMMAND-LINE` stands in for Format 1 `ACCEPT` throughout:
/// both write their text through the same path, and the command line can be set
/// from a test where the process's real stdin cannot.
fn run_with_args(src: &str, args: &[&str]) -> Vec<String> {
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
    interp.set_program_args(args.iter().map(|s| s.to_string()).collect());
    interp.run().expect("run failed");
    drop(interp);
    display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect()
}

fn run(src: &str) -> Vec<String> {
    run_with_args(src, &[])
}

// ── ACCEPT into a group ──────────────────────────────────────────────────────

/// NC109M `ACC-TEST-GF-1`: `ACCEPT ACCEPT-D1` where `ACCEPT-D1` is a group of
/// `X(20)` and `X(7)`. The accepted line is cut across the children by width.
#[test]
fn accept_into_a_group_distributes_across_its_children() {
    let out = run_with_args(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ACCGRP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-GROUP.
          02 WS-HEAD PIC X(4).
          02 WS-TAIL PIC X(3).
       PROCEDURE DIVISION.
       MAIN.
           ACCEPT WS-GROUP FROM COMMAND-LINE.
           DISPLAY "HEAD=[" WS-HEAD "]".
           DISPLAY "TAIL=[" WS-TAIL "]".
           DISPLAY "WHOLE=[" WS-GROUP "]".
           STOP RUN.
"#,
        &["ABCDXYZ"],
    );
    assert_eq!(out[0], "HEAD=[ABCD]", "{out:#?}");
    assert_eq!(out[1], "TAIL=[XYZ]", "{out:#?}");
    assert_eq!(out[2], "WHOLE=[ABCDXYZ]", "{out:#?}");
}

/// NC109M `ACC-TEST-GF-11`: `X80-CHARACTER-FIELD` is an `01` whose only child is
/// an unnamed `FILLER PIC X(80)`, and the operator types fewer characters than
/// the field holds. The remainder is space-filled, not left as whatever the
/// field held before.
#[test]
fn accept_into_a_filler_only_group_pads_to_the_declared_width() {
    let out = run_with_args(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ACCFIL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-FIELD.
          02 FILLER PIC X(10).
       01 WS-WANT PIC X(10) VALUE "AB        ".
       PROCEDURE DIVISION.
       MAIN.
           MOVE ALL "Z" TO WS-FIELD.
           ACCEPT WS-FIELD FROM COMMAND-LINE.
           IF WS-FIELD = WS-WANT
              DISPLAY "PADDED"
           ELSE
              DISPLAY "GOT=[" WS-FIELD "]".
           STOP RUN.
"#,
        &["AB"],
    );
    assert_eq!(out[0], "PADDED", "{out:#?}");
}

/// A nested group is walked to its leaves — the distribution recurses rather
/// than stopping at the first level.
#[test]
fn accept_into_a_nested_group_reaches_the_leaves() {
    let out = run_with_args(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ACCNEST.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-OUTER.
          02 WS-INNER.
             03 WS-A PIC X(2).
             03 WS-B PIC X(2).
          02 WS-C PIC X(2).
       PROCEDURE DIVISION.
       MAIN.
           ACCEPT WS-OUTER FROM COMMAND-LINE.
           DISPLAY "A=" WS-A " B=" WS-B " C=" WS-C.
           STOP RUN.
"#,
        &["112233"],
    );
    assert_eq!(out[0], "A=11 B=22 C=33", "{out:#?}");
}

/// The elementary case must not regress: a plain receiver still takes the whole
/// value, space-padded to its own width.
#[test]
fn accept_into_an_elementary_item_is_unchanged() {
    let out = run_with_args(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ACCELEM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-FLAT PIC X(6).
       PROCEDURE DIVISION.
       MAIN.
           ACCEPT WS-FLAT FROM COMMAND-LINE.
           DISPLAY "[" WS-FLAT "]".
           STOP RUN.
"#,
        &["PQ"],
    );
    assert_eq!(out[0], "[PQ    ]", "{out:#?}");
}

// ── VALUE QUOTE ──────────────────────────────────────────────────────────────

/// NC109M `ACC-TEST-GF-9`: `ACCEPT-D18 PICTURE X VALUE QUOTE`. The item holds a
/// quotation mark, not a space.
#[test]
fn value_quote_on_a_single_character_item() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. VALQ.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-Q PIC X VALUE QUOTE.
       PROCEDURE DIVISION.
       MAIN.
           IF WS-Q = QUOTE
              DISPLAY "MATCHES-FIGURATIVE"
           ELSE
              DISPLAY "MISMATCH".
           DISPLAY "[" WS-Q "]".
           STOP RUN.
"#);
    assert_eq!(out[0], "MATCHES-FIGURATIVE", "{out:#?}");
    assert_eq!(out[1], "[\"]", "{out:#?}");
}

/// A figurative constant in a `VALUE` clause **fills** the item, exactly as
/// `VALUE SPACE` does — it is not one quote followed by blanks.
#[test]
fn value_quote_fills_a_wider_item() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. VALQW.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-QQ PIC X(3) VALUE QUOTE.
       01 WS-SP PIC X(3) VALUE SPACE.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "[" WS-QQ "]".
           DISPLAY "[" WS-SP "]".
           STOP RUN.
"#);
    assert_eq!(out[0], "[\"\"\"]", "{out:#?}");
    assert_eq!(out[1], "[   ]", "{out:#?}");
}

/// `QUOTES` is the same constant under its plural spelling.
#[test]
fn value_quotes_plural_spelling() {
    let out = run(r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. VALQS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-Q PIC X(2) VALUE QUOTES.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "[" WS-Q "]".
           STOP RUN.
"#);
    assert_eq!(out[0], "[\"\"]", "{out:#?}");
}
