// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Integration tests for RustCOBOL control property access (spec 010):
//!   GET — `ctrl::prop`, `ctrl::"prop"`, `INVOKE ctrl "prop" RETURNING x`,
//!         `INVOKE ctrl "GET-prop" RETURNING x`
//!   SET — `MOVE v TO ctrl::prop`, `SET ctrl::"prop" TO v`,
//!         `INVOKE ctrl "prop" USING v`, `INVOKE ctrl "SET-prop" USING v`
//! Property names are case-insensitive; numeric properties compare algebraically.

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
    display_rx.try_iter().collect()
}

fn run_capture_seeded(
    src: &str,
    seed: Vec<(String, String, Vec<(String, String)>)>,
) -> (Vec<String>, Vec<cobolt_runtime::StateUpdate>) {
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
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.seed_objects(seed);
    interp.run().expect("run failed");
    (
        display_rx.try_iter().collect(),
        state_rx.try_iter().collect(),
    )
}

/// Run a program returning `Ok` on success / `Err(message)` on a runtime error,
/// for the "invalid receiving field" negative test.
fn run_result(src: &str) -> Result<(), String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, _display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().map_err(|e| e.to_string())
}

// GET via every form, after a single SET.
const GET_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-X PIC X(20).
       PROCEDURE DIVISION.
           MOVE "Hello!" TO BUTTON-1::Caption.
           DISPLAY "inline=[" BUTTON-1::Caption "]".
           DISPLAY "quoted=[" BUTTON-1::"Caption" "]".
           INVOKE BUTTON-1 "Caption" RETURNING WS-X.
           DISPLAY "invoke=[" WS-X "]".
           INVOKE BUTTON-1 "GET-Caption" RETURNING WS-X.
           DISPLAY "getp=[" WS-X "]".
           STOP RUN.
"#;

#[test]
fn get_property_all_forms() {
    let out = run_capture(GET_SRC).join("\n");
    for tag in [
        "inline=[Hello!",
        "quoted=[Hello!",
        "invoke=[Hello!",
        "getp=[Hello!",
    ] {
        assert!(out.contains(tag), "missing {tag:?} in:\n{out}");
    }
}

// SET via every form, reading back each time.
const SET_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           MOVE "A" TO LBL-1::Text.
           DISPLAY "move=[" LBL-1::Text "]".
           SET LBL-1::"Text" TO "B".
           DISPLAY "set=[" LBL-1::Text "]".
           INVOKE LBL-1 "Text" USING "C".
           DISPLAY "using=[" LBL-1::Text "]".
           INVOKE LBL-1 "SET-Text" USING "D".
           DISPLAY "setp=[" LBL-1::Text "]".
           STOP RUN.
"#;

#[test]
fn set_property_all_forms() {
    let out = run_capture(SET_SRC).join("\n");
    for tag in ["move=[A", "set=[B", "using=[C", "setp=[D"] {
        assert!(out.contains(tag), "missing {tag:?} in:\n{out}");
    }
}

const USER_CONTROL_PROP_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-X PIC X(20).
       PROCEDURE DIVISION.
           INVOKE CARD-1 "GetProperty" USING "Button1.Caption"
               RETURNING WS-X.
           DISPLAY "before=[" WS-X "]".
           INVOKE CARD-1 "SetProperty" USING "Button1.Caption" "Updated".
           INVOKE CARD-1 "GetProperty" USING "Button1.Caption"
               RETURNING WS-X.
           DISPLAY "after=[" WS-X "]".
           STOP RUN.
"#;

#[test]
fn user_control_set_get_property_routes_to_qualified_child() {
    let seed = vec![
        (
            "CARD-1".to_owned(),
            "GroupBox".to_owned(),
            vec![("UserControl".to_owned(), "Card".to_owned())],
        ),
        (
            "CARD-1-Button1".to_owned(),
            "Button".to_owned(),
            vec![("Caption".to_owned(), "Default".to_owned())],
        ),
    ];
    let (out, updates) = run_capture_seeded(USER_CONTROL_PROP_SRC, seed);
    let out = out.join("\n");
    assert!(
        out.contains("before=[Default"),
        "missing default read:\n{out}"
    );
    assert!(
        out.contains("after=[Updated"),
        "missing updated read:\n{out}"
    );
    assert!(
        updates.iter().any(|upd| {
            upd.ctrl_id == "CARD-1-Button1" && upd.prop == "Caption" && upd.value == "Updated"
        }),
        "missing child StateUpdate: {updates:?}"
    );
}

// Numeric properties compare algebraically (not as digit strings).
const NUM_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           MOVE 232 TO BTN::Width.
           MOVE 64 TO LBL::Width.
           IF BTN::Width > LBL::Width
               DISPLAY "GT"
           ELSE
               DISPLAY "LE"
           END-IF.
           STOP RUN.
"#;

#[test]
fn numeric_property_comparison_is_algebraic() {
    let out = run_capture(NUM_SRC).join("\n");
    assert!(
        out.contains("GT"),
        "232 > 64 must hold algebraically: {out}"
    );
}

// ── Spec 011: member-access chains, nested model, all-verb receivers ─────────

// A nested grid cell is reachable and assignable through a chain with subscripts.
const NESTED_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           MOVE "hello" TO GRID::Rows(0)::Columns(1)::Value.
           DISPLAY "cell=[" GRID::Rows(0)::Columns(1)::Value "]".
           DISPLAY "up=[" GRID::Rows(0)::Columns(1)::Value::toUpperCase() "]".
           STOP RUN.
"#;

#[test]
fn nested_chain_get_set_and_transform() {
    let out = run_capture(NESTED_SRC).join("\n");
    assert!(out.contains("cell=[hello"), "nested set/get failed: {out}");
    assert!(out.contains("up=[HELLO"), "tail transform failed: {out}");
}

// `Items(n)` indexes the legacy newline-string list form.
const ITEMS_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           INVOKE LST "AddItem" USING "alpha".
           INVOKE LST "AddItem" USING "beta".
           INVOKE LST "AddItem" USING "gamma".
           DISPLAY "i1=[" LST::Items(1) "]".
           DISPLAY "n=[" LST::Items::Count() "]".
           STOP RUN.
"#;

#[test]
fn indexed_legacy_string_list() {
    let out = run_capture(ITEMS_SRC).join("\n");
    assert!(
        out.contains("i1=[beta"),
        "Items(1) line index failed: {out}"
    );
    assert!(out.contains("n=[3"), "Items::Count() failed: {out}");
}

// A collection element is removed via `::Delete()`.
const DELETE_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           MOVE "a" TO LST::Rows(0)::Value.
           MOVE "b" TO LST::Rows(1)::Value.
           LST::Rows(0)::Delete().
           DISPLAY "row0=[" LST::Rows(0)::Value "]".
           STOP RUN.
"#;

#[test]
fn collection_element_delete() {
    let out = run_capture(DELETE_SRC).join("\n");
    assert!(
        out.contains("row0=[b"),
        "delete did not shift elements: {out}"
    );
}

// Property as a receiving field for verbs beyond MOVE/SET (STRING, ADD).
const ALLVERB_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           MOVE "X" TO LBL::Text.
           STRING "AB" "CD" DELIMITED BY SIZE INTO LBL::Text.
           DISPLAY "str=[" LBL::Text "]".
           MOVE 10 TO N::Value.
           ADD 5 TO N::Value.
           DISPLAY "add=[" N::Value "]".
           STOP RUN.
"#;

#[test]
fn property_receiver_for_all_verbs() {
    let out = run_capture(ALLVERB_SRC).join("\n");
    assert!(
        out.contains("str=[ABCD"),
        "STRING INTO property failed: {out}"
    );
    assert!(out.contains("add=[15"), "ADD TO property failed: {out}");
}

// INITIALIZE rules: bare control → Value; explicit member; mixed with data item.
const INIT_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N PIC X(5) VALUE "keepX".
       PROCEDURE DIVISION.
           MOVE "data" TO OBJ::Value.
           MOVE "txt"  TO OBJ::Caption.
           INITIALIZE OBJ.
           DISPLAY "val=[" OBJ::Value "]".
           DISPLAY "cap=[" OBJ::Caption "]".
           MOVE "again" TO OBJ::Value.
           INITIALIZE OBJ::Value WS-N.
           DISPLAY "val2=[" OBJ::Value "]".
           DISPLAY "ws=[" WS-N "]".
           STOP RUN.
"#;

#[test]
fn initialize_member_and_control_rules() {
    let out = run_capture(INIT_SRC).join("\n");
    // INITIALIZE OBJ resets only the Value property, not Caption.
    assert!(
        out.contains("val=[]"),
        "INITIALIZE OBJ should clear Value: {out}"
    );
    assert!(
        out.contains("cap=[txt"),
        "INITIALIZE OBJ must not touch Caption: {out}"
    );
    // INITIALIZE OBJ::Value WS-N: Value cleared; WS-N reset to SPACES per PIC.
    assert!(
        out.contains("val2=[]"),
        "INITIALIZE OBJ::Value should clear it: {out}"
    );
    assert!(out.contains("ws=[ "), "WS-N should reset to spaces: {out}");
}

// A method-call result is not a receiving field (the user's "invalid" case).
const BAD_LVALUE_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-X PIC X(10) VALUE "hi".
       PROCEDURE DIVISION.
           MOVE WS-X TO OBJ::UpperCase().
           STOP RUN.
"#;

#[test]
fn method_call_is_not_a_receiving_field() {
    let err = run_result(BAD_LVALUE_SRC).expect_err("must be a runtime error");
    assert!(err.contains("receiving field"), "unexpected error: {err}");
}

// A method-call chain used as a statement is permitted but changes no data.
const NOEFFECT_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           MOVE "hello" TO OBJ::Value.
           OBJ::Value::toUpperCase().
           DISPLAY "v=[" OBJ::Value "]".
           STOP RUN.
"#;

#[test]
fn method_statement_has_no_effect_on_data() {
    let out = run_capture(NOEFFECT_SRC).join("\n");
    assert!(
        out.contains("v=[hello"),
        "method statement must not mutate the value: {out}"
    );
}

#[test]
fn property_access_does_not_warn_on_control_names() {
    // Control names in `ctrl::prop` are form objects, not DATA DIVISION items, so
    // they must not produce "not declared" warnings.
    let result = parse(tokenize(GET_SRC, SourceFormat::Free));
    let program = result.program.expect("no program");
    let analysis = cobolt_semantic::analyze(&program);
    assert!(
        !analysis
            .diagnostics
            .iter()
            .any(|d| d.message.contains("BUTTON-1")),
        "unexpected diagnostic for a control name: {:?}",
        analysis.diagnostics
    );
}
