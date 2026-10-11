// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `INITIALIZE` on an item a **contained** program declares in its own
//! WORKING-STORAGE — which is what every form event handler is.
//!
//! Such an item lives under `PROGRAM-ID` + `\u{4}` + `NAME` and is reached
//! through a divert alias for the length of the call, so the flat
//! `CobolEnvironment::contains` never saw it: `INITIALIZE LK-FILE` was taken
//! for a control object, reset a property nothing owns, and left the item as it
//! was. `tests/cobol/call/test-initialize-contained.cbl` is the quantified
//! COBOL counterpart; this file pins the same behaviour and the environment
//! pieces it stands on.

use std::path::Path;
use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::environment::{CobolEnvironment, ItemSym};
use cobolt_runtime::value::CobolValue;
use cobolt_runtime::Interpreter;
use pretty_assertions::assert_eq;

fn parse_ok(src: &str) -> cobolt_ast::program::Program {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    result.program.expect("no program")
}

fn run_capture(src: &str) -> Vec<String> {
    let program = parse_ok(src);
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

/// A program whose CALLed subprogram's body is `body`, declared over `data`.
/// The outermost program only calls it, so everything the body touches is the
/// contained program's own storage.
fn in_contained(data: &str, body: &str) -> Vec<String> {
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-TOP PIC X(4) VALUE "top".
       PROCEDURE DIVISION.
           CALL "SUB-ONE"
           STOP RUN.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUB-ONE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
{data}
       PROCEDURE DIVISION.
{body}
           GOBACK.
       END PROGRAM SUB-ONE.
       END PROGRAM OUTER.
    "#
    );
    run_capture(&src)
}

// ── The reported bug ───────────────────────────────────────────────────────────

/// Verbatim from the report: the outermost program's item was reset, the
/// contained program's two were not.
#[test]
fn the_reported_repro_resets_both_items_in_the_contained_program() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INITP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-TOP  PIC X(10) VALUE "top".
       PROCEDURE DIVISION.
           INITIALIZE WS-TOP
           DISPLAY "top: [" WS-TOP "]"
           CALL "SUB-ONE"
           STOP RUN.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUB-ONE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 LK-FILE PIC X(10) VALUE "abc".
       01 LK-N PIC 9(3) VALUE 7.
       PROCEDURE DIVISION.
           INITIALIZE LK-FILE LK-N
           DISPLAY "file: [" LK-FILE "] n: [" LK-N "]"
           GOBACK.
       END PROGRAM SUB-ONE.
       END PROGRAM INITP.
    "#;
    assert_eq!(
        run_capture(src),
        vec![
            "top: [          ]".to_string(),
            "file: [          ] n: [000]".to_string(),
        ]
    );
}

#[test]
fn an_item_beside_the_initialized_ones_is_left_alone() {
    let out = in_contained(
        "       01 A PIC X(3) VALUE \"aaa\".\n       01 B PIC 99 VALUE 12.\n       01 C PIC X(3) VALUE \"ccc\".",
        "           INITIALIZE A B\n           DISPLAY \"[\" A \"][\" B \"][\" C \"]\"",
    );
    assert_eq!(out, vec!["[   ][00][ccc]"]);
}

// ── Every kind of item ─────────────────────────────────────────────────────────

#[test]
fn numerics_with_decimals_and_a_sign_go_to_zero() {
    let out = in_contained(
        "       01 D PIC 9(3)V99 VALUE 12.34.\n       01 S PIC S9(3)V99 VALUE -56.78.\n       01 F PIC X VALUE \"?\".",
        "           INITIALIZE D S\n           IF S = ZERO MOVE \"Z\" TO F END-IF\n           DISPLAY \"[\" D \"][\" F \"]\"",
    );
    assert_eq!(out, vec!["[00000][Z]"]);
}

#[test]
fn a_group_is_reset_leaf_by_leaf_and_its_filler_keeps_its_value() {
    let out = in_contained(
        "       01 G.\n          05 GA PIC X(3) VALUE \"xyz\".\n          05 GN PIC 99 VALUE 55.\n          05 FILLER PIC X VALUE \"-\".\n          05 GD PIC 9V9 VALUE 5.5.",
        "           INITIALIZE G\n           DISPLAY \"[\" G \"]\"",
    );
    assert_eq!(out, vec!["[   00-00]"]);
}

#[test]
fn a_nested_group_recurses() {
    let out = in_contained(
        "       01 OUTER-G.\n          05 INNER-G.\n             10 IA PIC X(2) VALUE \"ab\".\n             10 IN1 PIC 9 VALUE 7.\n          05 TAIL PIC X VALUE \"t\".",
        "           INITIALIZE OUTER-G\n           DISPLAY \"[\" OUTER-G \"]\"",
    );
    assert_eq!(out, vec!["[  0 ]"]);
}

#[test]
fn a_table_is_reset_in_every_occurrence_including_ones_written_before() {
    let out = in_contained(
        "       01 T.\n          05 TE PIC X(2) OCCURS 3 VALUE \"ab\".\n       01 M.\n          05 MN PIC 99 OCCURS 3 VALUE 11.",
        "           MOVE \"ZZ\" TO TE (2)\n           MOVE 99 TO MN (3)\n           INITIALIZE T M\n           DISPLAY \"[\" TE (1) TE (2) TE (3) \"][\" MN (1) MN (2) MN (3) \"]\"",
    );
    assert_eq!(out, vec!["[      ][000000]"]);
}

#[test]
fn a_table_of_groups_is_reset_in_every_occurrence() {
    let out = in_contained(
        "       01 U.\n          05 UE OCCURS 2.\n             10 UC PIC 9 VALUE 4.\n             10 US PIC X VALUE \"q\".",
        "           MOVE 9 TO UC (2)\n           MOVE \"w\" TO US (1)\n           INITIALIZE U\n           DISPLAY \"[\" U \"]\"",
    );
    assert_eq!(out, vec!["[0 0 ]"]);
}

#[test]
fn one_occurrence_can_be_reset_alone() {
    let out = in_contained(
        "       01 T.\n          05 TE PIC X(2) OCCURS 3 VALUE \"ab\".",
        "           MOVE \"XX\" TO TE (1)\n           MOVE \"YY\" TO TE (2)\n           MOVE \"ZZ\" TO TE (3)\n           INITIALIZE TE (2)\n           DISPLAY \"[\" TE (1) TE (2) TE (3) \"]\"",
    );
    assert_eq!(out, vec!["[XX  ZZ]"]);
}

#[test]
fn a_redefining_item_follows_its_target() {
    let out = in_contained(
        "       01 G.\n          05 N PIC 99 VALUE 42.\n          05 R REDEFINES N PIC XX.",
        "           INITIALIZE G\n           DISPLAY \"[\" N \"][\" R \"]\"",
    );
    assert_eq!(out, vec!["[00][00]"]);
}

/// A contained program's storage is static — its values persist between calls
/// (Guide, "Procedures are static") — so a reset made on one call is what the
/// next call finds, not the `VALUE` it was declared with.
#[test]
fn a_reset_persists_into_the_next_call() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       PROCEDURE DIVISION.
           CALL "SUB-ONE"
           CALL "SUB-ONE"
           CALL "SUB-ONE"
           STOP RUN.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUB-ONE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 L-N PIC 99 VALUE 7.
       01 L-CALLS PIC 9 VALUE 0.
       01 L-T.
          05 L-TE PIC X OCCURS 2 VALUE "x".
       PROCEDURE DIVISION.
           ADD 1 TO L-CALLS
           DISPLAY "call " L-CALLS ": n=" L-N " t=" L-TE (1) L-TE (2)
           IF L-CALLS = 1
              INITIALIZE L-N L-T
           END-IF
           GOBACK.
       END PROGRAM SUB-ONE.
       END PROGRAM OUTER.
    "#;
    assert_eq!(
        run_capture(src),
        vec![
            "call 1: n=07 t=xx".to_string(),
            // (`run_capture` trims the end of a line: both slots are spaces.)
            "call 2: n=00 t=".to_string(),
            "call 3: n=00 t=".to_string(),
        ]
    );
}

// ── REPLACING ──────────────────────────────────────────────────────────────────

#[test]
fn replacing_touches_only_the_named_categories() {
    let out = in_contained(
        "       01 R.\n          05 RN PIC 9(3) VALUE 123.\n          05 RX PIC X(3) VALUE \"ABC\".",
        "           INITIALIZE R REPLACING NUMERIC DATA BY 7\n           DISPLAY \"[\" R \"]\"",
    );
    assert_eq!(out, vec!["[007ABC]"]);
}

#[test]
fn replacing_two_categories_replaces_both() {
    let out = in_contained(
        "       01 Q.\n          05 QN PIC 9(3) VALUE 123.\n          05 QX PIC X(3) VALUE \"ABC\".",
        "           INITIALIZE Q REPLACING NUMERIC DATA BY 5\n                                  ALPHANUMERIC DATA BY \"ZZ\"\n           DISPLAY \"[\" Q \"]\"",
    );
    assert_eq!(out, vec!["[005ZZ ]"]);
}

// ── Whose item it is ───────────────────────────────────────────────────────────

/// `SHARED` is declared in the outermost program and in both contained ones,
/// `TWIN` only in the two contained ones. A reset in one program must reach that
/// program's own item and no other — `TWIN` is the half the flat lookup missed.
#[test]
fn a_name_declared_in_two_contained_programs_resets_each_programs_own_item() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SHARED PIC X(7) VALUE "outer-1".
       PROCEDURE DIVISION.
           CALL "SUB-A"
           DISPLAY "outer: [" SHARED "]"
           STOP RUN.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUB-A.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SHARED PIC X(7) VALUE "inner-a".
       01 TWIN PIC X(6) VALUE "twin-a".
       PROCEDURE DIVISION.
           CALL "SUB-B"
           DISPLAY "a before: [" SHARED "][" TWIN "]"
           INITIALIZE SHARED TWIN
           DISPLAY "a after:  [" SHARED "][" TWIN "]"
           GOBACK.
       END PROGRAM SUB-A.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUB-B.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SHARED PIC X(7) VALUE "inner-b".
       01 TWIN PIC X(6) VALUE "twin-b".
       PROCEDURE DIVISION.
           INITIALIZE SHARED TWIN
           DISPLAY "b after:  [" SHARED "][" TWIN "]"
           GOBACK.
       END PROGRAM SUB-B.
       END PROGRAM OUTER.
    "#;
    assert_eq!(
        run_capture(src),
        vec![
            "b after:  [       ][      ]".to_string(),
            "a before: [inner-a][twin-a]".to_string(),
            "a after:  [       ][      ]".to_string(),
            "outer: [outer-1]".to_string(),
        ]
    );
}

/// A LINKAGE parameter is an alias onto the caller's storage. When the caller
/// is itself a contained program, the alias target is an activation key.
#[test]
fn a_parameter_bound_to_a_contained_programs_group_is_reset_through_the_alias() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       PROCEDURE DIVISION.
           CALL "SUB-A"
           STOP RUN.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUB-A.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 L-GRP.
          05 L-GA PIC X(3) VALUE "abc".
          05 L-GN PIC 99 VALUE 42.
       PROCEDURE DIVISION.
           CALL "SUB-B" USING L-GRP
           DISPLAY "[" L-GRP "]"
           GOBACK.
       END PROGRAM SUB-A.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUB-B.
       DATA DIVISION.
       LINKAGE SECTION.
       01 LK-GRP.
          05 LK-GA PIC X(3).
          05 LK-GN PIC 99.
       PROCEDURE DIVISION USING LK-GRP.
           INITIALIZE LK-GRP
           GOBACK.
       END PROGRAM SUB-B.
       END PROGRAM OUTER.
    "#;
    assert_eq!(run_capture(src), vec!["[   00]"]);
}

/// Not a declared item at all: the statement still reads it as a control
/// object, and it must not disturb any storage.
#[test]
fn an_undeclared_name_is_still_a_control_and_touches_no_data() {
    let out = in_contained(
        "       01 A PIC X(3) VALUE \"aaa\".",
        "           INITIALIZE NO-SUCH-THING\n           DISPLAY \"[\" A \"]\"",
    );
    assert_eq!(out, vec!["[aaa]"]);
}

/// The outermost program is walked by its own DATA DIVISION, as before.
#[test]
fn the_outermost_program_still_resets_groups_numerics_and_text() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TOP-ONLY.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 O-X PIC X(5) VALUE "abcde".
       01 O-N PIC 9(3)V9 VALUE 12.3.
       01 O-G.
          05 O-GA PIC X(3) VALUE "xyz".
          05 O-GN PIC 99 VALUE 55.
       PROCEDURE DIVISION.
           INITIALIZE O-X O-N O-G
           DISPLAY "[" O-X "][" O-N "][" O-G "]"
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["[     ][0000][   00]"]);
}

// ── The environment pieces ─────────────────────────────────────────────────────

fn env_of(src: &str) -> CobolEnvironment {
    let program = parse_ok(src);
    CobolEnvironment::from_data_division(program.data.as_ref().expect("no data division"))
}

#[test]
fn is_declared_finds_what_a_divert_alias_reaches_and_contains_cannot() {
    let mut env = env_of(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. P.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 OUTER-ITEM PIC X(3) VALUE "out".
       PROCEDURE DIVISION.
           STOP RUN.
    "#,
    );
    let key = "SUB-ONE\u{4}L-X".to_string();
    env.push_local_scope(&[(key.clone(), CobolValue::from_str("abc", 3))], &[]);

    // Before the call installs its alias, the bare name is nobody's.
    assert!(!env.contains("L-X"));
    assert!(!env.is_declared("L-X", &[]));

    env.set_alias("L-X", &key);
    assert!(!env.contains("L-X"), "the flat lookup still cannot see it");
    assert!(env.is_declared("L-X", &[]), "the resolved lookup can");

    // Nothing `contains` finds is lost.
    assert!(env.contains("OUTER-ITEM"));
    assert!(env.is_declared("OUTER-ITEM", &[]));
    assert!(!env.is_declared("NOT-A-NAME", &[]));
}

#[test]
fn is_declared_counts_a_group_that_owns_no_storage_slot() {
    let mut env = env_of(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. P.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 OUTER-ITEM PIC X(3) VALUE "out".
       PROCEDURE DIVISION.
           STOP RUN.
    "#,
    );
    let key = "SUB-ONE\u{4}L-G".to_string();
    let group = ItemSym {
        is_group: true,
        layout_keys: vec!["SUB-ONE\u{4}L-GA".to_string()],
        ..ItemSym::default()
    };
    env.push_local_scope(&[], &[(key.clone(), group)]);
    env.set_alias("L-G", &key);
    assert!(!env.contains("L-G"));
    assert!(env.is_declared("L-G", &[]));
}

#[test]
fn initialize_leaves_walks_groups_tables_and_skips_filler_and_redefines() {
    let env = env_of(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. P.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 G.
          05 A PIC X(2).
          05 FILLER PIC X.
          05 N PIC 99.
          05 R REDEFINES N PIC XX.
       01 T.
          05 TE PIC X OCCURS 3.
       01 U.
          05 UE OCCURS 2.
             10 UC PIC 9.
             10 US PIC X.
       01 S PIC X.
       PROCEDURE DIVISION.
           STOP RUN.
    "#,
    );
    let leaves = |name: &str| env.initialize_leaves(name);
    assert_eq!(leaves("G"), vec!["A", "N"]);
    assert_eq!(leaves("T"), vec!["TE(1)", "TE(2)", "TE(3)"]);
    assert_eq!(leaves("TE(2)"), vec!["TE(2)"]);
    assert_eq!(leaves("U"), vec!["UC(1)", "US(1)", "UC(2)", "US(2)"]);
    assert_eq!(leaves("UE(2)"), vec!["UC(2)", "US(2)"]);
    assert_eq!(leaves("S"), vec!["S"]);
    // A key the symbol table has never heard of is answered as itself.
    assert_eq!(leaves("NO-SUCH"), vec!["NO-SUCH"]);
}

// ── The quantified COBOL program ───────────────────────────────────────────────

/// `tests/cobol/call/test-initialize-contained.cbl` prints a case table and a
/// tally; this runs it under `cargo test` so the table cannot rot unseen.
#[test]
fn the_quantified_cobol_program_passes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cobol/call/test-initialize-contained.cbl");
    let src = std::fs::read_to_string(&path).expect("read the COBOL test");
    let out = run_capture(&src);
    let report = out.join("\n");
    let line = |label: &str| {
        out.iter()
            .find(|l| l.starts_with(label))
            .unwrap_or_else(|| panic!("no `{label}` line in:\n{report}"))
            .clone()
    };
    assert_eq!(line("CASES EXERCISED"), "CASES EXERCISED : 14", "{report}");
    assert_eq!(line("CHECKS FAILED"), "CHECKS FAILED   : 0000", "{report}");
    assert_eq!(line("OVERALL RESULT"), "OVERALL RESULT  : PASS", "{report}");
    println!("{report}");
}
