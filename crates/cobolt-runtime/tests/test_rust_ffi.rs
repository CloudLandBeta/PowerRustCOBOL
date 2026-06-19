// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Rust-FFI dispatch (spec 005 T10 / AC6): a `REPOSITORY` binding maps a COBOL
//! class to a Rust type, a `USAGE OBJECT REFERENCE` item (seeded from its VALUE)
//! holds a live Rust object, and `INVOKE obj "method"` / `obj::method()` calls
//! into the curated Rust bridge, marshaling arguments and results.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run(src: &str) -> (Vec<String>, usize) {
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
    let out = display_rx.try_iter().map(|s| s.trim().to_owned()).collect();
    (out, interp.rust_object_count())
}

#[test]
fn invoke_rust_string_len_and_uppercase() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FFIDEMO.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-STRING IS "Rust.String"
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 S USAGE IS OBJECT REFERENCE RUST-STRING VALUE "hello".
       01 N PIC 9(4).
       01 T PIC X(10).
       PROCEDURE DIVISION.
           INVOKE S "len" RETURNING N.
           DISPLAY N.
           INVOKE S "to_uppercase" RETURNING T.
           DISPLAY T.
           STOP RUN.
"#;
    let (out, live) = run(src);
    assert_eq!(out, vec!["0005".to_string(), "HELLO".to_string()]);
    assert!(live >= 1, "the Rust.String object should be live during the run");
}

#[test]
fn invoke_with_using_argument_mutates() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FFIMUT.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-STRING IS "Rust.String"
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 S USAGE IS OBJECT REFERENCE RUST-STRING VALUE "ab".
       01 N PIC 9(4).
       PROCEDURE DIVISION.
           INVOKE S "push_str" USING "cde".
           INVOKE S "len" RETURNING N.
           DISPLAY N.
           STOP RUN.
"#;
    let (out, _live) = run(src);
    assert_eq!(out, vec!["0005".to_string()]); // "ab" + "cde" = 5 bytes
}
// NOTE: both the `INVOKE … RETURNING` form (tested above) and the inline
// `obj::method()` form are wired. Inline `::` as a **value operand** inside
// DISPLAY/MOVE/COMPUTE is covered by `test_inline_methodcall_009`
// (spec 009 R16 / AC9 — `DISPLAY S::len()`).
