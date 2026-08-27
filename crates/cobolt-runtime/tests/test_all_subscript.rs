// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `ALL` as a subscript — handing a whole table to a statistical intrinsic.
//!
//! COBOL-85 lets a table be passed entire by subscripting it with the reserved
//! word `ALL`:
//!
//! ```cobol
//! COMPUTE WS-NUM = FUNCTION MAX(IND(ALL)).
//! COMPUTE WS-NUM = FUNCTION SUM(TBL(ALL, 2)).
//! ```
//!
//! One written argument becomes as many actual arguments as the table has
//! elements. `ALL` was previously read as the figurative-constant prefix
//! (`ALL "X"`), so the parser demanded a literal after it and eleven NIST IF
//! programs stopped there — see `specs/nist/NIST-spec-intrinsic-function-gaps.md`.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

/// A one-dimensional table, seeded 10 20 30 40 50.
fn one_dim(body: &str) -> String {
    format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ALLSUB.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TBL.
           05  IND PIC 9(4) OCCURS 5 TIMES.
       01  WS-NUM  PIC S9(6)V99.
       01  WS-I    PIC 9(2).
       PROCEDURE DIVISION.
       MAIN.
           MOVE 10 TO IND (1).
           MOVE 20 TO IND (2).
           MOVE 30 TO IND (3).
           MOVE 40 TO IND (4).
           MOVE 50 TO IND (5).
{body}
           STOP RUN.
"#
    )
}

/// AC1 — `FUNCTION MAX(IND(ALL))` is the largest element, and `MEAN` their mean.
#[test]
fn ac1_max_and_mean_over_a_whole_table() {
    let out = run_capture(&one_dim(
        "           COMPUTE WS-NUM = FUNCTION MAX(IND(ALL)).\n\
         \x20          DISPLAY \"MAX=\" WS-NUM.\n\
         \x20          COMPUTE WS-NUM = FUNCTION MEAN(IND(ALL)).\n\
         \x20          DISPLAY \"MEAN=\" WS-NUM.",
    ));
    let joined = out.join("|");
    assert!(joined.contains("MAX="), "no MAX line: {out:?}");
    // 50 is the largest; 30 is the mean of 10..50.
    assert!(
        joined.contains("50") && joined.contains("30"),
        "expected MAX 50 and MEAN 30, got: {out:?}"
    );
}

/// The other variable-length statistical functions take the same expansion.
#[test]
fn sum_min_median_range_over_a_whole_table() {
    for (func, expect) in [
        ("SUM", "150"),
        ("MIN", "10"),
        ("MEDIAN", "30"),
        ("RANGE", "40"),
    ] {
        let out = run_capture(&one_dim(&format!(
            "           COMPUTE WS-NUM = FUNCTION {func}(IND(ALL)).\n\
             \x20          DISPLAY \"R=\" WS-NUM."
        )));
        assert!(
            out.join("|").contains(expect),
            "FUNCTION {func}(IND(ALL)) should contain {expect}, got {out:?}"
        );
    }
}

/// AC2 — `ALL` in one dimension of a 2-D table, ordinary subscripts elsewhere.
/// `TBL(ALL, 2)` is the second column: 2 + 4 + 6 = 12.
#[test]
fn ac2_all_in_one_dimension_of_a_two_dimensional_table() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ALLSUB2.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  GRID.
           05  ROW OCCURS 3 TIMES.
               10  CEL PIC 9(4) OCCURS 2 TIMES.
       01  WS-NUM  PIC S9(6)V99.
       PROCEDURE DIVISION.
       MAIN.
           MOVE 1 TO CEL (1, 1).
           MOVE 2 TO CEL (1, 2).
           MOVE 3 TO CEL (2, 1).
           MOVE 4 TO CEL (2, 2).
           MOVE 5 TO CEL (3, 1).
           MOVE 6 TO CEL (3, 2).
           COMPUTE WS-NUM = FUNCTION SUM(CEL(ALL, 2)).
           DISPLAY "COL2=" WS-NUM.
           COMPUTE WS-NUM = FUNCTION SUM(CEL(ALL, ALL)).
           DISPLAY "ALLCELLS=" WS-NUM.
           STOP RUN.
"#;
    let out = run_capture(src);
    let joined = out.join("|");
    assert!(joined.contains("12"), "column 2 should sum to 12: {out:?}");
    assert!(joined.contains("21"), "every cell should sum to 21: {out:?}");
}

/// AC4 — the figurative constant is untouched. `ALL "X"` is still `ALL "X"`,
/// which is the whole reason the decision is made in the parser on what
/// follows the word rather than in the lexer.
#[test]
fn ac4_the_figurative_constant_all_is_unchanged() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FIGALL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-T PIC X(5).
       01  WS-Z PIC X(5).
       PROCEDURE DIVISION.
       MAIN.
           MOVE ALL "X" TO WS-T.
           DISPLAY "T=" WS-T.
           MOVE ALL ZEROS TO WS-Z.
           DISPLAY "Z=" WS-Z.
           STOP RUN.
"#;
    let out = run_capture(src);
    assert!(
        out.join("|").contains("XXXXX"),
        "MOVE ALL \"X\" must still fill the field: {out:?}"
    );
}

/// A table subscripted with an ordinary index still reads one element — the
/// expansion must not fire for a normal reference.
#[test]
fn an_ordinary_subscript_is_not_expanded() {
    let out = run_capture(&one_dim(
        "           COMPUTE WS-NUM = FUNCTION MAX(IND(2)).\n\
         \x20          DISPLAY \"R=\" WS-NUM.",
    ));
    assert!(
        out.join("|").contains("20"),
        "FUNCTION MAX(IND(2)) is the second element, 20: {out:?}"
    );
}
