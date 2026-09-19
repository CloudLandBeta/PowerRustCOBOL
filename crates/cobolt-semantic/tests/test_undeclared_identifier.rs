// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! An undeclared data-item reference is an ERROR (1.70.77), so every gate
//! that stops on `SemanticResult::is_ok()` — the IDE's Run Form and Build,
//! `rcrun run-form`, `rcrun build` — refuses the program.
//!
//! It was a warning, and the interpreter then created the item on its first
//! write, sized to that value, truncating every later one: `"ButtonOk
//! clicked"` (16) once, and `"ButtonCancel clicked"` came back as
//! `ButtonCancel cli` for the rest of the run (PowerDemo3, 2026-09-19).

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, Severity};

fn analyze_src(src: &str) -> cobolt_semantic::SemanticResult {
    let parsed = parse(tokenize(src, SourceFormat::Free));
    assert!(
        !parsed.has_errors(),
        "the source must parse cleanly: {:?}",
        parsed.diagnostics
    );
    analyze(&parsed.program.expect("program should parse"))
}

#[test]
fn an_undeclared_identifier_is_an_error_and_fails_is_ok() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-H PIC X(8).
       PROCEDURE DIVISION.
       MAIN.
           MOVE "ButtonCancel clicked" TO WS-RESULT
           STOP RUN.
"#;
    let sem = analyze_src(src);
    let hits: Vec<_> = sem
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("'WS-RESULT' is not declared in DATA DIVISION"))
        .collect();
    assert_eq!(hits.len(), 1, "exactly one report for the one bad name: {:?}", sem.diagnostics);
    assert_eq!(
        hits[0].severity,
        Severity::Error,
        "an undeclared identifier must be an ERROR, so the gates refuse the program"
    );
    assert!(!sem.is_ok(), "is_ok() is what every gate checks — it must be false");
    println!("undeclared WS-RESULT: 1 diagnostic, severity Error, is_ok() = false");
}

#[test]
fn a_declared_identifier_and_the_runtime_registers_raise_nothing() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-RESULT PIC X(60).
       PROCEDURE DIVISION.
       MAIN.
           MOVE "ButtonCancel clicked" TO WS-RESULT
           MOVE ZERO TO RETURN-CODE
           MOVE SPACES TO COBOL-EVENT-ID
           STOP RUN.
"#;
    let sem = analyze_src(src);
    let undeclared: Vec<_> = sem
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("is not declared in DATA DIVISION"))
        .collect();
    assert!(
        undeclared.is_empty(),
        "a declared item and the runtime's own registers are not undeclared: {undeclared:?}"
    );
    assert!(sem.is_ok(), "{:?}", sem.diagnostics);
    println!("declared WS-RESULT + RETURN-CODE + COBOL-EVENT-ID: 0 undeclared, is_ok() = true");
}
