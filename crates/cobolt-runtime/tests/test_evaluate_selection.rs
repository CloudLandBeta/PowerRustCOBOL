// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! EVALUATE selection subjects and objects — NIST CCVS85 NC225A.
//!
//! Two readings of a **bare word** were wrong, and both failed silently in the
//! same direction: they made a WHEN match when it should not have.
//!
//! * As a selection **object** (`EVALUATE 1234 WHEN WRK-DU-08V00`) a bare name
//!   that is not a declared 88-level is an *operand* compared against the
//!   subject. Read as a condition it fell through to the "truthy if non-zero"
//!   fallback, so every WHEN with a non-zero object matched whatever the
//!   subject held — and only the expect-equal direction passed, by accident.
//! * As a selection **subject** (`EVALUATE IT-IS-81 WHEN TRUE`) a bare name
//!   that *is* a declared 88-level is a conditional subject. Read as a data
//!   item it resolved to no slot at all, evaluated to 0, and `WHEN TRUE` never
//!   matched.
//!
//! Reference: VI-84 6.12.4 GR1(a)(b)(c) and GR3.

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

/// Wrap `body` in a program that declares NC225A's data items.
fn prog(body: &str) -> String {
    format!(
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. EVASEL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WRK-DU-08V00    PIC 9(8).
       01  WRK-DU-08V00-1  PIC 9(8).
       01  WRK-DU-08V00-2  PIC 9(8).
           88 IT-IS-81     VALUE 81.
       01  WRK-DU-08V00-3  PIC 9(8).
       01  WRK-DU-08V00-4  PIC 9(8).
       01  WRK-XN-1        PIC X.
       01  WRK-XN-2        PIC X.
       01  WS-81           PIC S99 VALUE +81.
       PROCEDURE DIVISION.
       MAIN-PARA.
{body}
           STOP RUN.
"
    )
}

// ── selection OBJECT: a bare name is an operand, not a truth test ───────────

/// GF-16 / GF-26: `EVALUATE 1234 WHEN WRK-DU-08V00` with the item at 78 must
/// NOT match. This is the case that made the bug visible: 78 is non-zero, so
/// the truthy fallback matched every time.
#[test]
fn unequal_numeric_object_does_not_match() {
    let out = run(&prog(
        "           MOVE 78 TO WRK-DU-08V00.
           EVALUATE 1234
              WHEN WRK-DU-08V00 DISPLAY \"MATCHED\"
              WHEN OTHER        DISPLAY \"NO-MATCH\"
           END-EVALUATE.",
    ));
    assert_eq!(out, vec!["NO-MATCH"]);
}

/// GF-15: the equal direction still matches. It passed before the fix too —
/// by accident — so it is the guard that the fix did not simply invert.
#[test]
fn equal_numeric_object_matches() {
    let out = run(&prog(
        "           MOVE 26 TO WRK-DU-08V00.
           EVALUATE 26
              WHEN WRK-DU-08V00 DISPLAY \"MATCHED\"
              WHEN OTHER        DISPLAY \"NO-MATCH\"
           END-EVALUATE.",
    ));
    assert_eq!(out, vec!["MATCHED"]);
}

/// GF-4-1: alphanumeric operands. `"1"` and `"*"` are both non-zero and both
/// truthy, so nothing but a real comparison separates them.
#[test]
fn unequal_alphanumeric_object_does_not_match() {
    let out = run(&prog(
        "           MOVE \"1\" TO WRK-XN-1.
           MOVE \"*\" TO WRK-XN-2.
           EVALUATE WRK-XN-1
              WHEN WRK-XN-2 DISPLAY \"MATCHED\"
              WHEN OTHER    DISPLAY \"NO-MATCH\"
           END-EVALUATE.",
    ));
    assert_eq!(out, vec!["NO-MATCH"]);
}

/// GF-4-2: `WHEN NOT <bare name>` is the negation of the comparison, so an
/// unequal object makes it match.
#[test]
fn negated_unequal_object_matches() {
    let out = run(&prog(
        "           MOVE \"1\" TO WRK-XN-1.
           MOVE \"*\" TO WRK-XN-2.
           EVALUATE WRK-XN-1
              WHEN NOT WRK-XN-2 DISPLAY \"MATCHED\"
              WHEN OTHER        DISPLAY \"NO-MATCH\"
           END-EVALUATE.",
    ));
    assert_eq!(out, vec!["MATCHED"]);
}

/// GF-20: an arithmetic subject against a signed data-item object —
/// `(8 * 9)` is 72 and must not match `WS-81`.
#[test]
fn arithmetic_subject_against_unequal_item_object() {
    let out = run(&prog(
        "           MOVE 8 TO WRK-DU-08V00.
           EVALUATE (WRK-DU-08V00 * 9)
              WHEN WS-81 DISPLAY \"MATCHED\"
              WHEN OTHER DISPLAY \"NO-MATCH\"
           END-EVALUATE.",
    ));
    assert_eq!(out, vec!["NO-MATCH"]);
}

/// A declared 88-level as the *object* stays a condition — the fix must not
/// turn every bare word into an operand. `IT-IS-81` holds, so it matches.
#[test]
fn declared_condition_name_object_stays_a_condition() {
    let out = run(&prog(
        "           MOVE 81 TO WRK-DU-08V00-2.
           EVALUATE TRUE
              WHEN IT-IS-81 DISPLAY \"MATCHED\"
              WHEN OTHER    DISPLAY \"NO-MATCH\"
           END-EVALUATE.",
    ));
    assert_eq!(out, vec!["MATCHED"]);
}

// ── selection SUBJECT: a bare 88-level is a conditional subject ─────────────

/// GF-31 subject 4: `EVALUATE IT-IS-81 WHEN TRUE` selects on whether the
/// condition holds, not on the value of a data item called `IT-IS-81` — there
/// is no such item, which is why the subject used to evaluate to 0.
#[test]
fn condition_name_subject_matches_when_true() {
    let out = run(&prog(
        "           MOVE 81 TO WRK-DU-08V00-2.
           EVALUATE IT-IS-81
              WHEN TRUE  DISPLAY \"TRUE-ARM\"
              WHEN OTHER DISPLAY \"OTHER-ARM\"
           END-EVALUATE.",
    ));
    assert_eq!(out, vec!["TRUE-ARM"]);
}

/// The same subject when the condition does not hold.
#[test]
fn condition_name_subject_matches_when_false() {
    let out = run(&prog(
        "           MOVE 7 TO WRK-DU-08V00-2.
           EVALUATE IT-IS-81
              WHEN TRUE  DISPLAY \"TRUE-ARM\"
              WHEN FALSE DISPLAY \"FALSE-ARM\"
           END-EVALUATE.",
    ));
    assert_eq!(out, vec!["FALSE-ARM"]);
}

/// NC225A EVA-TEST-GF-31 in full: six subjects joined by `ALSO`, mixing data
/// items, an arithmetic expression, a condition-name, `TRUE`, and `FALSE`.
/// The first WHEN matches on every column and must win; before the fix the
/// condition-name column failed and selection fell through to the third WHEN.
#[test]
fn six_subject_also_evaluate_selects_the_first_when() {
    let out = run(&prog(
        "           MOVE 81  TO WRK-DU-08V00.
           MOVE \"*\" TO WRK-XN-1.
           MOVE \"*\" TO WRK-XN-2.
           MOVE 987 TO WRK-DU-08V00-1.
           MOVE 81  TO WRK-DU-08V00-2.
           MOVE 0   TO WRK-DU-08V00-3.
           MOVE 567 TO WRK-DU-08V00-4.
           EVALUATE     WRK-DU-08V00
                   ALSO 81
                   ALSO (WRK-DU-08V00 * 9)
                   ALSO IT-IS-81
                   ALSO TRUE
                   ALSO FALSE
              WHEN NOT  WRK-DU-08V00-1
                   ALSO WRK-DU-08V00-2
                   ALSO 729
                   ALSO TRUE
                   ALSO WRK-DU-08V00-3 = 0
                   ALSO WRK-DU-08V00-4 < 9
                        MOVE \"A\" TO WRK-XN-1
                        MOVE \"B\" TO WRK-XN-2
              WHEN      81
                   ALSO WRK-DU-08V00
                   ALSO (9 * 9 * 9)
                   ALSO FALSE
                   ALSO WRK-XN-2 = \"*\"
                   ALSO WRK-DU-08V00 > 8
                        MOVE \"C\" TO WRK-XN-1
                        MOVE \"D\" TO WRK-XN-2
              WHEN      ANY
                   ALSO ANY
                   ALSO ANY
                   ALSO ANY
                   ALSO ANY
                   ALSO WRK-DU-08V00 = 6
                        MOVE \"E\" TO WRK-XN-1
                        MOVE \"F\" TO WRK-XN-2
              WHEN      OTHER
                        MOVE \"G\" TO WRK-XN-1
                        MOVE \"H\" TO WRK-XN-2
           END-EVALUATE.
           DISPLAY WRK-XN-1 WRK-XN-2.",
    ));
    assert_eq!(out, vec!["AB"]);
}
