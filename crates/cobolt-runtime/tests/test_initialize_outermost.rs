// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `INITIALIZE` on items the **outermost** program declares.
//!
//! It used to walk the program's DATA DIVISION by bare name, which left four
//! things wrong that the walk over the environment's symbol table (the one a
//! contained program's items already took) gets right:
//!
//! * a table kept every occurrence written before the statement;
//! * a name declared in two groups was reset under a key that holds nothing;
//! * a `REDEFINES` item reset after its target overwrote it;
//! * an elementary item carrying an 88-level condition-name was taken for a
//!   group of that one condition and never reset at all.
//!
//! `tests/cobol/call/test-initialize-outermost.cbl` is the quantified COBOL
//! counterpart; `test_initialize_contained.rs` covers a handler's own items.

use std::path::Path;
use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;
use pretty_assertions::assert_eq;

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
    display_rx
        .try_iter()
        .map(|s| s.trim_end().to_owned())
        .collect()
}

/// `data` over a one-paragraph program whose body is `body`.
fn outermost(data: &str, body: &str) -> Vec<String> {
    run_capture(&format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
{data}
       PROCEDURE DIVISION.
{body}
           STOP RUN.
    "#
    ))
}

#[test]
fn an_occurrence_written_before_the_statement_is_reset_too() {
    let out = outermost(
        "       01 T.\n          05 TE PIC X(2) OCCURS 3 VALUE \"ab\".",
        "           MOVE \"ZZ\" TO TE (2)\n           INITIALIZE T\n           DISPLAY \"[\" TE (1) TE (2) TE (3) \"]\"",
    );
    assert_eq!(out, vec!["[      ]"], "all three occurrences are spaces");
}

#[test]
fn a_table_of_groups_is_reset_in_every_occurrence() {
    let out = outermost(
        "       01 U.\n          05 UE OCCURS 2.\n             10 UC PIC 9 VALUE 4.\n             10 US PIC X VALUE \"q\".",
        "           MOVE 9 TO UC (2)\n           MOVE \"w\" TO US (1)\n           INITIALIZE U\n           DISPLAY \"[\" U \"]\"",
    );
    assert_eq!(out, vec!["[0 0 ]"]);
}

#[test]
fn one_occurrence_is_reset_alone() {
    let out = outermost(
        "       01 P.\n          05 PE PIC X(2) OCCURS 3 VALUE \"ab\".",
        "           MOVE \"XX\" TO PE (1)\n           MOVE \"YY\" TO PE (2)\n           MOVE \"ZZ\" TO PE (3)\n           INITIALIZE PE (2)\n           DISPLAY \"[\" PE (1) PE (2) PE (3) \"]\"",
    );
    assert_eq!(out, vec!["[XX  ZZ]"]);
}

/// A slice of a two-dimensional table is exact: its untouched cells are
/// materialised, because the table's template cannot stand in for part of it.
#[test]
fn a_row_of_a_two_dimensional_table_is_reset_and_the_other_row_is_not() {
    let out = outermost(
        "       01 M.\n          05 ROW OCCURS 2.\n             10 CELL PIC 9 OCCURS 3 VALUE 5.",
        "           MOVE 8 TO CELL (1, 1)\n           MOVE 8 TO CELL (2, 2)\n           INITIALIZE ROW (2)\n           DISPLAY \"[\" CELL (1, 1) CELL (1, 2) CELL (1, 3) \"][\" CELL (2, 1) CELL (2, 2) CELL (2, 3) \"]\"",
    );
    assert_eq!(out, vec!["[855][000]"]);
}

#[test]
fn a_child_name_shared_by_two_groups_is_reset_in_both() {
    let out = outermost(
        "       01 G1.\n          05 F PIC X(3) VALUE \"one\".\n       01 G2.\n          05 F PIC X(3) VALUE \"two\".",
        "           INITIALIZE G1 G2\n           DISPLAY \"[\" F OF G1 \"][\" F OF G2 \"]\"",
    );
    assert_eq!(out, vec!["[   ][   ]"]);
}

#[test]
fn a_qualified_child_resets_only_its_own_group() {
    let out = outermost(
        "       01 G3.\n          05 H PIC X(3) VALUE \"aaa\".\n       01 G4.\n          05 H PIC X(3) VALUE \"bbb\".",
        "           INITIALIZE H OF G4\n           DISPLAY \"[\" H OF G3 \"][\" H OF G4 \"]\"",
    );
    assert_eq!(out, vec!["[aaa][   ]"]);
}

#[test]
fn a_redefining_item_follows_its_target_instead_of_overwriting_it() {
    let out = outermost(
        "       01 R.\n          05 RN PIC 99 VALUE 42.\n          05 RR REDEFINES RN PIC XX.",
        "           INITIALIZE R\n           DISPLAY \"[\" RN \"][\" RR \"]\"",
    );
    assert_eq!(out, vec!["[00][00]"]);
}

/// The common one: a flag with an 88 under it. The old walk saw a "group" whose
/// only child was the condition-name, reset that, and left the flag as it was.
#[test]
fn an_elementary_item_carrying_a_condition_name_is_reset() {
    let out = outermost(
        "       01 L.\n          05 FLAG PIC X VALUE \"Y\".\n             88 IS-Y VALUE \"Y\".\n          05 AFTER-IT PIC X VALUE \"z\".\n       01 SAW PIC X VALUE \"?\".",
        "           INITIALIZE L\n           IF IS-Y MOVE \"Y\" TO SAW ELSE MOVE \"n\" TO SAW END-IF\n           DISPLAY \"[\" FLAG \"][\" AFTER-IT \"][\" SAW \"]\"",
    );
    assert_eq!(out, vec!["[ ][ ][n]"]);

    let alone = outermost(
        "       01 FLAG PIC X VALUE \"Y\".\n          88 IS-Y VALUE \"Y\".\n       01 SAW PIC X VALUE \"?\".",
        "           INITIALIZE FLAG\n           IF IS-Y MOVE \"Y\" TO SAW ELSE MOVE \"n\" TO SAW END-IF\n           DISPLAY \"[\" FLAG \"][\" SAW \"]\"",
    );
    assert_eq!(alone, vec!["[ ][n]"]);
}

#[test]
fn replacing_reaches_every_occurrence_of_a_table_and_only_the_named_category() {
    let out = outermost(
        "       01 X.\n          05 XE OCCURS 2.\n             10 XN PIC 9(3) VALUE 123.\n             10 XT PIC X(3) VALUE \"ABC\".",
        "           MOVE 777 TO XN (2)\n           INITIALIZE X REPLACING NUMERIC DATA BY 5\n           DISPLAY \"[\" X \"]\"",
    );
    assert_eq!(out, vec!["[005ABC005ABC]"]);
}

#[test]
fn an_outermost_group_still_resets_numerics_text_and_spares_its_filler() {
    let out = outermost(
        "       01 O-X PIC X(5) VALUE \"abcde\".\n       01 O-N PIC 9(3)V9 VALUE 12.3.\n       01 O-G.\n          05 O-GA PIC X(3) VALUE \"xyz\".\n          05 FILLER PIC X VALUE \"-\".\n          05 O-GN PIC 99 VALUE 55.",
        "           INITIALIZE O-X O-N O-G\n           DISPLAY \"[\" O-X \"][\" O-N \"][\" O-G \"]\"",
    );
    assert_eq!(out, vec!["[     ][0000][   -00]"]);
}

/// A table is reset through its template, so a huge one costs the same as a
/// small one — the guard against expanding every occurrence.
#[test]
fn a_huge_untouched_table_is_reset_without_touching_each_occurrence() {
    let started = std::time::Instant::now();
    let out = outermost(
        "       01 H.\n          05 HE PIC 9(4) OCCURS 100000 VALUE 55.\n       01 PASS-NO PIC 99.",
        "           MOVE 9 TO HE (100000)\n           PERFORM VARYING PASS-NO FROM 1 BY 1 UNTIL PASS-NO > 50\n               INITIALIZE H\n           END-PERFORM\n           DISPLAY \"[\" HE (1) \"][\" HE (50000) \"][\" HE (100000) \"]\"",
    );
    assert_eq!(out, vec!["[0000][0000][0000]"]);
    // 50 passes x 100,000 occurrences is 5,000,000 resets if each is touched.
    // Through the template it is milliseconds; the bound is generous so that a
    // slow machine cannot trip it, and far under what the eager walk costs.
    assert!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "took {:?}",
        started.elapsed()
    );
}

/// `tests/cobol/call/test-initialize-outermost.cbl` prints a case table and a
/// tally; this runs it under `cargo test` so the table cannot rot unseen.
#[test]
fn the_quantified_cobol_program_passes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cobol/call/test-initialize-outermost.cbl");
    let src = std::fs::read_to_string(&path).expect("read the COBOL test");
    let out = run_capture(&src);
    let report = out.join("\n");
    let line = |label: &str| {
        out.iter()
            .find(|l| l.starts_with(label))
            .unwrap_or_else(|| panic!("no `{label}` line in:\n{report}"))
            .clone()
    };
    assert_eq!(line("CASES EXERCISED"), "CASES EXERCISED : 13", "{report}");
    assert_eq!(line("CHECKS FAILED"), "CHECKS FAILED   : 0000", "{report}");
    assert_eq!(line("OVERALL RESULT"), "OVERALL RESULT  : PASS", "{report}");
    println!("{report}");
}
