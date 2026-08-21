// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `TRUE` / `FALSE` as ordinary operands (operator, 2026-08-20).
//!
//! They are sugar for **1** and **0** and nothing else: `SET X TO TRUE` is
//! `MOVE 1 TO X`, `IF X = FALSE` is `IF X = 0`. Two of the forms already
//! worked before this — `SET X TO TRUE/FALSE`, and the standard COBOL-85
//! `EVALUATE TRUE` case statement — and the tests for them are here too, so a
//! later change cannot quietly take them away again.
//!
//! The regression that matters most is at the bottom: **88-level condition
//! names**, where `TRUE` already meant something specific long before this
//! sugar existed. `SET <88-name> TO TRUE` must still set the host item to a
//! value satisfying the condition, not store the number 1 in it.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Run `src` and return its DISPLAY lines. Panics with the diagnostics when the
/// program does not parse — an unsupported form must fail loudly here.
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
    display_rx.try_iter().map(|l| l.trim().to_string()).collect()
}

/// Wrap `storage` and `body` in the smallest program that can hold them.
fn program(storage: &str, body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. TF.\n\
         DATA DIVISION.\n\
         WORKING-STORAGE SECTION.\n\
         {storage}\n\
         PROCEDURE DIVISION.\n\
         {body}\n\
             STOP RUN.\n"
    )
}

/// The stated rule, checked directly: these keywords ARE 1 and 0.
#[test]
fn set_true_and_false_store_one_and_zero() {
    let out = run(&program(
        "01 X PIC 9 VALUE 5.",
        r#"    SET X TO TRUE
    DISPLAY X
    SET X TO FALSE
    DISPLAY X"#,
    ));
    assert_eq!(out, vec!["1", "0"]);
}

/// `MOVE` and arithmetic see the same values — they are operands, not a
/// statement-specific spelling.
#[test]
fn true_and_false_are_operands_anywhere_a_value_is_allowed() {
    let out = run(&program(
        "01 X PIC 9 VALUE 0.\n01 N PIC 9(4) VALUE 10.",
        r#"    MOVE TRUE TO X
    DISPLAY X
    COMPUTE N = N + TRUE + FALSE
    DISPLAY N"#,
    ));
    assert_eq!(out, vec!["1", "0011"]);
}

/// Every comparison form the operator asked for, in one program: with `=`,
/// with `IS`, with a bare `NOT`, and with `NOT =`.
#[test]
fn every_if_form_reads_the_flag_the_same_way() {
    let out = run(&program(
        "01 X PIC 9 VALUE 0.",
        r#"    SET X TO TRUE
    IF X = TRUE
        DISPLAY "eq"
    END-IF
    IF X IS TRUE
        DISPLAY "is"
    END-IF
    IF X NOT FALSE
        DISPLAY "bare-not"
    END-IF
    IF X NOT = FALSE
        DISPLAY "not-eq"
    END-IF
    IF X IS NOT FALSE
        DISPLAY "is-not"
    END-IF"#,
    ));
    assert_eq!(out, vec!["eq", "is", "bare-not", "not-eq", "is-not"]);
}

/// …and they are genuinely evaluated, not just accepted: the same forms on a
/// FALSE flag must take the other branch.
#[test]
fn the_forms_are_evaluated_not_merely_parsed() {
    let out = run(&program(
        "01 X PIC 9 VALUE 1.",
        r#"    SET X TO FALSE
    IF X = TRUE
        DISPLAY "wrong"
    ELSE
        DISPLAY "eq-else"
    END-IF
    IF X IS FALSE
        DISPLAY "is-false"
    END-IF
    IF X NOT TRUE
        DISPLAY "not-true"
    END-IF"#,
    ));
    assert_eq!(out, vec!["eq-else", "is-false", "not-true"]);
}

/// A bare `TRUE`/`FALSE` as the whole condition.
#[test]
fn a_bare_truth_value_is_a_condition_on_its_own() {
    let out = run(&program(
        "01 X PIC 9 VALUE 0.",
        r#"    IF TRUE
        DISPLAY "always"
    END-IF
    IF FALSE
        DISPLAY "wrong"
    ELSE
        DISPLAY "never"
    END-IF"#,
    ));
    assert_eq!(out, vec!["always", "never"]);
}

/// `EVALUATE TRUE` / `EVALUATE FALSE` — the standard COBOL-85 case statement,
/// where the subject selects the WHEN whose CONDITION has that truth value.
/// This is not the operand sugar and must not become it.
#[test]
fn evaluate_true_and_false_select_by_the_conditions_truth() {
    let out = run(&program(
        "01 A PIC 9(4) VALUE 5.\n01 B PIC 9(4) VALUE 7.",
        r#"    EVALUATE TRUE
        WHEN A > B
            DISPLAY "t-wrong"
        WHEN A < B
            DISPLAY "t-ok"
    END-EVALUATE
    EVALUATE FALSE
        WHEN A < B
            DISPLAY "f-wrong"
        WHEN A > B
            DISPLAY "f-ok"
    END-EVALUATE"#,
    ));
    assert_eq!(out, vec!["t-ok", "f-ok"]);
}

/// `EVALUATE <value>` with `WHEN TRUE` is the other reading — the subject is
/// matched against the VALUE 1. Both spellings live in the same statement and
/// must not be confused for one another.
#[test]
fn when_true_against_a_value_subject_matches_one() {
    let out = run(&program(
        "01 X PIC 9 VALUE 0.",
        r#"    SET X TO TRUE
    EVALUATE X
        WHEN FALSE
            DISPLAY "wrong"
        WHEN TRUE
            DISPLAY "when-true"
    END-EVALUATE"#,
    ));
    assert_eq!(out, vec!["when-true"]);
}

/// PERFORM, both ways: as part of a comparison and as the whole condition.
#[test]
fn perform_until_accepts_both_shapes() {
    let out = run(&program(
        "01 X PIC 9 VALUE 0.\n01 N PIC 9(4) VALUE 0.",
        r#"    SET X TO TRUE
    PERFORM UNTIL X = FALSE
        SET X TO FALSE
        ADD 1 TO N
    END-PERFORM
    PERFORM UNTIL TRUE
        ADD 100 TO N
    END-PERFORM
    DISPLAY N"#,
    ));
    assert_eq!(
        out,
        vec!["0001"],
        "the body ran once, and never for UNTIL TRUE"
    );
}

/// **The regression that matters.** `TRUE` meant something in COBOL long before
/// this sugar: `SET <88-name> TO TRUE` sets the HOST item to a value that
/// satisfies the condition — here the letter `Y`, not the number 1. Reading an
/// 88 by name is likewise a condition, not a comparison against 1.
#[test]
fn eighty_eight_level_condition_names_are_untouched() {
    let out = run(&program(
        "01 STATUS-FLAG PIC X VALUE \"N\".\n   88 IS-DONE     VALUE \"Y\".\n   88 IS-PENDING  VALUE \"N\".",
        r#"    IF IS-PENDING
        DISPLAY "pending"
    END-IF
    SET IS-DONE TO TRUE
    DISPLAY STATUS-FLAG
    IF IS-DONE
        DISPLAY "done"
    END-IF
    IF NOT IS-PENDING
        DISPLAY "not-pending"
    END-IF"#,
    ));
    assert_eq!(
        out,
        vec!["pending", "Y", "done", "not-pending"],
        "SET 88 TO TRUE must satisfy the condition, not store 1"
    );
}
