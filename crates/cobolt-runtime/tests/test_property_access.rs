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
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
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
    for tag in ["inline=[Hello!", "quoted=[Hello!", "invoke=[Hello!", "getp=[Hello!"] {
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
    assert!(out.contains("GT"), "232 > 64 must hold algebraically: {out}");
}

#[test]
fn property_access_does_not_warn_on_control_names() {
    // Control names in `ctrl::prop` are form objects, not DATA DIVISION items, so
    // they must not produce "not declared" warnings.
    let result = parse(tokenize(GET_SRC, SourceFormat::Free));
    let program = result.program.expect("no program");
    let analysis = cobolt_semantic::analyze(&program);
    assert!(
        !analysis.diagnostics.iter().any(|d| d.message.contains("BUTTON-1")),
        "unexpected diagnostic for a control name: {:?}",
        analysis.diagnostics
    );
}
