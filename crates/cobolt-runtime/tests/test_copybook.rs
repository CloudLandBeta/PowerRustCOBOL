// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Integration test for COPY / REPLACE (the `copybook` preprocessor): a program
//! that COPYs copybooks (with REPLACING and nesting) and verifies the spliced
//! fields work at runtime. Runs `tests/cobol/copytest.cbl` end-to-end.

use std::path::PathBuf;
use std::sync::mpsc;

use cobolt_lexer::{expand_copybooks, tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn cobol_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol")
}

/// Read a `.cbl` from tests/cobol, expand copybooks (resolved from that dir),
/// run it, and return captured DISPLAY lines.
fn run_file(name: &str) -> Vec<String> {
    let dir = cobol_dir();
    let src = std::fs::read_to_string(dir.join(name)).expect("read .cbl");
    let expanded = expand_copybooks(&src, &dir, SourceFormat::Free);
    assert!(expanded.errors.is_empty(), "copybook errors: {:?}", expanded.errors);

    let tokens = tokenize(&expanded.text, SourceFormat::Free);
    let result = parse(tokens);
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

#[test]
fn copytest_suite_reports_pass() {
    let out = run_file("copytest.cbl").join("\n");
    assert!(out.contains("RESULT       : PASS"), "copytest failed:\n{out}");
    assert_eq!(out.matches("PASS T0").count(), 3, "expected 3 PASS lines:\n{out}");
}

#[test]
fn copy_replacing_and_nesting_resolve_fields() {
    // The program only runs if COPY spliced the fields (WS-NAME, WS-BALANCE from
    // CUSTREC via REPLACING; INNER-CODE via the nested OUTER->INNER copy).
    let out = run_file("copytest.cbl").join("\n");
    assert!(out.contains("PASS T001"), "REPLACING field missing:\n{out}");
    assert!(out.contains("PASS T003"), "nested COPY field missing:\n{out}");
}
