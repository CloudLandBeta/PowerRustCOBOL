// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `CALL … RETURNING` is checked where the analyser can see both sides.
//!
//! The interpreter moves the called program's RETURNING item into the
//! receiver (see the runtime's `test_call_returning`). What it cannot do is
//! say so when the phrase can never deliver anything, and a receiver that
//! keeps its old value looks exactly like a result:
//!
//! * the receiver must be a place a `MOVE` can write, not a literal or an
//!   expression (error);
//! * a program's own `PROCEDURE DIVISION … RETURNING item` must name a
//!   declared item (error);
//! * a literal target the unit knows has no RETURNING item — a program
//!   without one, a paragraph, a built-in `COBOL-…` call — returns nothing
//!   (warning). A target the unit cannot see (Common Code, a name held in a
//!   data item) is left alone.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, Severity};

/// A caller whose procedure is `body`, followed by `callees`.
fn unit(body: &str, callees: &str) -> String {
    format!(
        "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-N     PIC 9(4) VALUE 1.
       01  WS-R     PIC 9(5) VALUE 0.
       01  WS-T     PIC 9(5) OCCURS 3.
       01  WS-REC.
           05  WS-ROW   PIC 9(5).
       PROCEDURE DIVISION.
       MAIN-PARA.
{body}
           STOP RUN.
       HELPER.
           DISPLAY \"helper\".
       END PROGRAM CALLER.
{callees}
"
    )
}

const WITH_RETURNING: &str = "
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DOUBLE.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-N   PIC 9(4).
       01  LK-R   PIC 9(5).
       PROCEDURE DIVISION USING LK-N RETURNING LK-R.
           COMPUTE LK-R = LK-N * 2
           GOBACK.
       END PROGRAM DOUBLE.
";

const WITHOUT_RETURNING: &str = "
       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUIET.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-N   PIC 9(4).
       PROCEDURE DIVISION USING LK-N.
           GOBACK.
       END PROGRAM QUIET.
";

fn messages(src: &str, severity: Severity) -> Vec<String> {
    let parsed = parse(tokenize(src, SourceFormat::Free));
    assert!(!parsed.has_errors(), "the source must parse cleanly: {:?}", parsed.diagnostics);
    let program = parsed.program.expect("program should parse");
    analyze(&program)
        .diagnostics
        .into_iter()
        .filter(|d| d.severity == severity)
        .map(|d| d.message)
        .collect()
}

fn only(v: Vec<String>, needle: &str) -> Vec<String> {
    v.into_iter().filter(|m| m.contains(needle)).collect()
}

// ── the receiver ──────────────────────────────────────────────────────────────

#[test]
fn a_data_item_is_a_receiver_whatever_its_spelling() {
    let src = unit(
        "           CALL \"DOUBLE\" USING WS-N RETURNING WS-R
           CALL \"DOUBLE\" USING WS-N RETURNING WS-T (2)
           CALL \"DOUBLE\" USING WS-N RETURNING WS-ROW OF WS-REC",
        WITH_RETURNING,
    );
    let errors = messages(&src, Severity::Error);
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    assert!(messages(&src, Severity::Warning).is_empty(), "DOUBLE does return a value");
}

#[test]
fn a_literal_receiver_is_an_error() {
    let src = unit("           CALL \"DOUBLE\" USING WS-N RETURNING 5", WITH_RETURNING);
    let e = only(messages(&src, Severity::Error), "RETURNING");
    assert_eq!(e.len(), 1, "one error expected: {e:?}");
    assert!(e[0].contains("data item"), "says what is wanted: {}", e[0]);
}

#[test]
fn an_expression_receiver_is_an_error() {
    let src = unit("           CALL \"DOUBLE\" USING WS-N RETURNING WS-N + 1", WITH_RETURNING);
    assert_eq!(only(messages(&src, Severity::Error), "RETURNING").len(), 1);
}

#[test]
fn an_undeclared_receiver_is_the_usual_undeclared_error() {
    let src = unit("           CALL \"DOUBLE\" USING WS-N RETURNING NO-SUCH-ITEM", WITH_RETURNING);
    let e = only(messages(&src, Severity::Error), "NO-SUCH-ITEM");
    assert_eq!(e.len(), 1, "{e:?}");
}

// ── the callee's own header ───────────────────────────────────────────────────

#[test]
fn a_returning_item_the_callee_does_not_declare_is_an_error() {
    let callee = "
       IDENTIFICATION DIVISION.
       PROGRAM-ID. LOST.
       DATA DIVISION.
       PROCEDURE DIVISION RETURNING LK-GONE.
           GOBACK.
       END PROGRAM LOST.
";
    let src = unit("           DISPLAY \"x\"", callee);
    let e = only(messages(&src, Severity::Error), "LK-GONE");
    assert_eq!(e.len(), 1, "one error expected: {e:?}");
    assert!(e[0].contains("LINKAGE"), "says where to declare it: {}", e[0]);
}

#[test]
fn a_declared_returning_item_is_fine_in_linkage_or_working_storage() {
    let callee = "
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OWN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-OWN   PIC X(4) VALUE SPACES.
       PROCEDURE DIVISION RETURNING WS-OWN.
           GOBACK.
       END PROGRAM OWN.
";
    let src = unit("           DISPLAY \"x\"", &format!("{callee}{WITH_RETURNING}"));
    let e = messages(&src, Severity::Error);
    assert!(e.is_empty(), "unexpected errors: {e:?}");
}

// ── a target that returns nothing ─────────────────────────────────────────────

#[test]
fn a_program_without_a_returning_item_is_named() {
    let src = unit("           CALL \"QUIET\" USING WS-N RETURNING WS-R", WITHOUT_RETURNING);
    let w = only(messages(&src, Severity::Warning), "RETURNING");
    assert_eq!(w.len(), 1, "one warning expected: {w:?}");
    assert!(w[0].contains("QUIET") && w[0].contains("no RETURNING item"), "{}", w[0]);
}

#[test]
fn a_program_with_a_returning_item_is_not_named() {
    let src = unit("           CALL \"DOUBLE\" USING WS-N RETURNING WS-R", WITH_RETURNING);
    assert!(only(messages(&src, Severity::Warning), "RETURNING").is_empty());
}

#[test]
fn the_name_is_matched_whatever_its_letter_case() {
    let src = unit("           CALL \"quiet\" USING WS-N RETURNING WS-R", WITHOUT_RETURNING);
    assert_eq!(only(messages(&src, Severity::Warning), "RETURNING").len(), 1);
}

#[test]
fn a_paragraph_returns_no_value() {
    let src = unit("           CALL \"HELPER\" RETURNING WS-R", "");
    let w = only(messages(&src, Severity::Warning), "RETURNING");
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("paragraph or section"), "{}", w[0]);
}

#[test]
fn a_built_in_call_returns_through_using() {
    let src = unit("           CALL \"COBOL-HTTP-GET\" USING WS-N RETURNING WS-R", "");
    let w = only(messages(&src, Severity::Warning), "RETURNING");
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("USING"), "{}", w[0]);
}

#[test]
fn a_target_the_unit_cannot_see_is_left_alone() {
    // Common Code, or a name held in a data item: nothing to judge it by.
    let src = unit(
        "           CALL \"IN-COMMON-CODE\" RETURNING WS-R
           CALL WS-N RETURNING WS-R",
        "",
    );
    assert!(only(messages(&src, Severity::Warning), "RETURNING").is_empty());
}

#[test]
fn a_call_without_returning_is_never_named() {
    let src = unit("           CALL \"QUIET\" USING WS-N", WITHOUT_RETURNING);
    assert!(only(messages(&src, Severity::Warning), "RETURNING").is_empty());
}
