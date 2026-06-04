// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! INSPECT: combined TALLYING … REPLACING, and BEFORE/AFTER INITIAL regions.

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

#[test]
fn tallying_and_replacing_combined() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 W PIC X(11) VALUE "MISSISSIPPI".
       01 C PIC 9(2) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           INSPECT W TALLYING C FOR ALL "S"
                     REPLACING ALL "S" BY "X"
           DISPLAY C
           DISPLAY W
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["04", "MIXXIXXIPPI"]);
}

#[test]
fn tally_after_initial() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSP2.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 W PIC X(11) VALUE "MISSISSIPPI".
       01 C PIC 9(2) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           INSPECT W TALLYING C FOR ALL "I" AFTER INITIAL "P"
           DISPLAY C
           STOP RUN.
    "#;
    // After the first P (…PPI), only one "I" remains.
    assert_eq!(run_capture(src), vec!["01"]);
}

#[test]
fn replace_before_initial() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSP3.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 W PIC X(11) VALUE "MISSISSIPPI".
       PROCEDURE DIVISION.
       MAIN.
           INSPECT W REPLACING ALL "I" BY "Y" BEFORE INITIAL "P"
           DISPLAY W
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["MYSSYSSYPPI"]);
}
