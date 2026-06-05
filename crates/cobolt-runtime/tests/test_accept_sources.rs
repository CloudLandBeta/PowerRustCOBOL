// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `ACCEPT … FROM {COMMAND-LINE | ARGUMENT-NUMBER | ARGUMENT-VALUE |
//! ENVIRONMENT-VALUE | ESCAPE KEY}` plus the paired `DISPLAY … UPON
//! {ARGUMENT-NUMBER | ENVIRONMENT-NAME}` registers.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_with_args(src: &str, args: &[&str]) -> Vec<String> {
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
    interp.set_program_args(args.iter().map(|s| s.to_string()).collect());
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

#[test]
fn argument_number_value_and_command_line() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ACC.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-CNT PIC 9(3) VALUE 0.
       01 WS-ARG PIC X(10) VALUE SPACES.
       01 WS-CL  PIC X(20) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN.
           ACCEPT WS-CNT FROM ARGUMENT-NUMBER
           DISPLAY WS-CNT
           DISPLAY 2 UPON ARGUMENT-NUMBER
           ACCEPT WS-ARG FROM ARGUMENT-VALUE
           DISPLAY WS-ARG
           ACCEPT WS-CL FROM COMMAND-LINE
           DISPLAY WS-CL
           STOP RUN.
    "#;
    let out = run_with_args(src, &["alpha", "beta", "gamma"]);
    assert_eq!(out, vec!["003", "beta", "alpha beta gamma"]);
}

#[test]
fn environment_value_and_escape_key() {
    std::env::set_var("PRC_ACCEPT_TEST", "the-value");
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ENV.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-ENV PIC X(12) VALUE SPACES.
       01 WS-ESC PIC X(2)  VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "PRC_ACCEPT_TEST" UPON ENVIRONMENT-NAME
           ACCEPT WS-ENV FROM ENVIRONMENT-VALUE
           DISPLAY WS-ENV
           ACCEPT WS-ESC FROM ESCAPE KEY
           DISPLAY WS-ESC
           STOP RUN.
    "#;
    let out = run_with_args(src, &[]);
    assert_eq!(out, vec!["the-value", "00"]);
}
