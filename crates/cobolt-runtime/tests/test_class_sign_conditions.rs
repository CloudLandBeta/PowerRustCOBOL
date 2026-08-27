// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Class and sign conditions — **the `IS` is optional**, and a condition may
//! be an `EVALUATE` subject.
//!
//! COBOL-85 writes `IF X IS NUMERIC` and `IF X NUMERIC` alike. `EVALUATE X
//! NUMERIC` has no `IS` at all — there is nowhere to put one — and is matched
//! against `WHEN TRUE` / `WHEN FALSE`.
//!
//! Requiring the `IS` meant `IF IF-D8 POSITIVE` (NC250A) stopped at the data
//! name, and `EVALUATE WRK-XN-00001-1 NUMERIC` (NC225A) left `NUMERIC` to be
//! read as a statement — which then swallowed the WHEN branches and the
//! `END-EVALUATE`.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let errs: Vec<&String> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| &d.message)
        .collect();
    assert!(errs.is_empty(), "parse errors: {errs:?}");
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

fn cond(body: &str) -> Vec<String> {
    run_capture(&format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. CLSSGN.\n\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01  POS-N  PIC S9(3) VALUE 5.\n\
         \x20      01  NEG-N  PIC S9(3) VALUE -5.\n\
         \x20      01  ZER-N  PIC S9(3) VALUE 0.\n\
         \x20      01  DIGITS PIC X(4) VALUE \"1234\".\n\
         \x20      01  LETTRS PIC X(4) VALUE \"ABCD\".\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         {body}\
         \x20          STOP RUN.\n"
    ))
}

/// The sign condition without `IS` (NC250A).
#[test]
fn a_sign_condition_needs_no_is() {
    let out = cond(
        "           IF POS-N POSITIVE DISPLAY \"P\" END-IF.\n\
         \x20          IF NEG-N NEGATIVE DISPLAY \"N\" END-IF.\n\
         \x20          IF ZER-N ZERO DISPLAY \"Z\" END-IF.\n",
    );
    assert_eq!(out, vec!["P", "N", "Z"], "{out:?}");
}

/// The class condition without `IS`.
#[test]
fn a_class_condition_needs_no_is() {
    let out = cond(
        "           IF DIGITS NUMERIC DISPLAY \"NUM\" END-IF.\n\
         \x20          IF LETTRS ALPHABETIC DISPLAY \"ALPHA\" END-IF.\n",
    );
    assert_eq!(out, vec!["NUM", "ALPHA"], "{out:?}");
}

/// `NOT` without `IS` negates the test, and only when a class or sign word
/// really follows.
#[test]
fn not_without_is_negates_the_test() {
    let out = cond(
        "           IF POS-N NOT NEGATIVE DISPLAY \"A\" END-IF.\n\
         \x20          IF LETTRS NOT NUMERIC DISPLAY \"B\" END-IF.\n",
    );
    assert_eq!(out, vec!["A", "B"], "{out:?}");
}

/// **A leading `NOT` before a relational operator is untouched.** The class
/// check must not consume it — `a NOT = b` is a negated comparison.
#[test]
fn a_leading_not_before_a_relop_still_parses() {
    let out = cond(
        "           IF POS-N NOT = 9 DISPLAY \"NE\" END-IF.\n\
         \x20          IF POS-N NOT GREATER THAN 9 DISPLAY \"LE\" END-IF.\n",
    );
    assert_eq!(out, vec!["NE", "LE"], "{out:?}");
}

/// The `IS` spelling still works — nothing was replaced.
#[test]
fn the_is_spelling_still_works() {
    let out = cond(
        "           IF POS-N IS POSITIVE DISPLAY \"P\" END-IF.\n\
         \x20          IF DIGITS IS NUMERIC DISPLAY \"NUM\" END-IF.\n\
         \x20          IF LETTRS IS NOT NUMERIC DISPLAY \"NN\" END-IF.\n",
    );
    assert_eq!(out, vec!["P", "NUM", "NN"], "{out:?}");
}

/// NC225A: a conditional expression as the `EVALUATE` subject, selected by
/// `WHEN TRUE` / `WHEN FALSE`.
#[test]
fn a_class_condition_may_be_an_evaluate_subject() {
    let out = cond(
        "           EVALUATE DIGITS NUMERIC\n\
         \x20              WHEN TRUE  DISPLAY \"IS-NUM\"\n\
         \x20              WHEN FALSE DISPLAY \"NOT-NUM\"\n\
         \x20          END-EVALUATE.\n\
         \x20          EVALUATE LETTRS NUMERIC\n\
         \x20              WHEN TRUE  DISPLAY \"IS-NUM-2\"\n\
         \x20              WHEN FALSE DISPLAY \"NOT-NUM-2\"\n\
         \x20          END-EVALUATE.\n",
    );
    assert_eq!(out, vec!["IS-NUM", "NOT-NUM-2"], "{out:?}");
}

/// **A plain data item is still an ordinary subject.** The condition attempt
/// must rewind, or `EVALUATE X WHEN 1 …` would stop working.
#[test]
fn a_plain_evaluate_subject_is_unaffected() {
    let out = cond(
        "           EVALUATE POS-N\n\
         \x20              WHEN 5 DISPLAY \"FIVE\"\n\
         \x20              WHEN OTHER DISPLAY \"OTHER\"\n\
         \x20          END-EVALUATE.\n",
    );
    assert_eq!(out, vec!["FIVE"], "{out:?}");
}
