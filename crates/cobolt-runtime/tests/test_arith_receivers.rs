// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Multi-receiver `MULTIPLY`/`DIVIDE` (GIVING r1 r2 …) and per-receiver
//! `ROUNDED` on `ADD`/`SUBTRACT`/`MULTIPLY`/`DIVIDE`.

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
fn multiply_giving_multiple_receivers() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MUL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A  PIC 9(4) VALUE 100.
       01 B  PIC 9(4) VALUE 7.
       01 R1 PIC 9(5) VALUE 0.
       01 R2 PIC 9(5) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           MULTIPLY A BY B GIVING R1 R2
           DISPLAY R1
           DISPLAY R2
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["00700", "00700"]);
}

#[test]
fn divide_giving_per_receiver_rounded() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DIV.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A  PIC 9(4) VALUE 100.
       01 B  PIC 9(4) VALUE 7.
       01 Q1 PIC 9(4)V99 VALUE 0.
       01 Q2 PIC 9(4)V99 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           DIVIDE A BY B GIVING Q1 ROUNDED Q2
           DISPLAY Q1
           DISPLAY Q2
           STOP RUN.
    "#;
    // 100/7 = 14.2857… → Q1 ROUNDED = 14.29, Q2 truncated = 14.28
    assert_eq!(run_capture(src), vec!["001429", "001428"]);
}

#[test]
fn add_per_receiver_rounded() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ADDR.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 X  PIC 9V9  VALUE 0.
       01 R1 PIC 9    VALUE 1.
       01 R2 PIC 9    VALUE 1.
       PROCEDURE DIVISION.
       MAIN.
           MOVE 1.5 TO X
           ADD X TO R1 ROUNDED R2
           DISPLAY R1
           DISPLAY R2
           STOP RUN.
    "#;
    // 1 + 1.5 = 2.5 → R1 ROUNDED = 3 (half away from zero), R2 truncated = 2
    assert_eq!(run_capture(src), vec!["3", "2"]);
}
