// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 009 R16 / AC9 (folded from 005 AC6): the inline `obj::method()` call
//! works as a **value operand** — `DISPLAY S::len()` and `MOVE S::len() TO N`.

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
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

const SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-STRING IS "Rust.String"
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 S USAGE IS OBJECT REFERENCE RUST-STRING VALUE "hello".
       01 N PIC 9(4) VALUE 0.
       PROCEDURE DIVISION.
           DISPLAY S::len().
           MOVE S::len() TO N.
           DISPLAY N.
           STOP RUN.
"#;

#[test]
fn inline_method_call_as_value_operand() {
    let out = run_capture(SRC);
    // "hello".len() == 5 — once inline via DISPLAY, once via MOVE … TO N.
    assert_eq!(out.len(), 2, "expected two DISPLAY lines: {out:?}");
    assert_eq!(out[0], "5", "DISPLAY S::len() should output 5: {out:?}");
    assert_eq!(out[1].trim_start_matches('0'), "5",
        "MOVE S::len() TO N then DISPLAY N should be 5: {out:?}");
}
