// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `INITIALIZE … REPLACING category DATA BY value`.

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
fn initialize_replacing_by_category() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. IR.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-GRP.
          05 WS-NUM PIC 9(3) VALUE 123.
          05 WS-TXT PIC X(3) VALUE "ABC".
       PROCEDURE DIVISION.
       MAIN.
           INITIALIZE WS-GRP REPLACING NUMERIC DATA BY 7
                                        ALPHANUMERIC DATA BY "ZZ"
           DISPLAY WS-NUM
           DISPLAY WS-TXT
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["007", "ZZ"]);
}

#[test]
fn renames_66_read_and_write() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. REN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-REC.
          05 WS-A PIC X(2) VALUE "AB".
          05 WS-B PIC X(2) VALUE "CD".
          05 WS-C PIC X(2) VALUE "EF".
       66 WS-AB RENAMES WS-A THRU WS-B.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY WS-AB
           MOVE "XYZW" TO WS-AB
           DISPLAY WS-A
           DISPLAY WS-B
           DISPLAY WS-C
           STOP RUN.
    "#;
    // RENAMES reads "ABCD"; writing "XYZW" distributes XY/ZW; WS-C untouched.
    assert_eq!(run_capture(src), vec!["ABCD", "XY", "ZW", "EF"]);
}

#[test]
fn initialize_plain_still_works() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. IP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NUM PIC 9(3) VALUE 123.
       01 WS-TXT PIC X(3) VALUE "ABC".
       PROCEDURE DIVISION.
       MAIN.
           INITIALIZE WS-NUM WS-TXT
           DISPLAY WS-NUM
           DISPLAY "[" WS-TXT "]"
           STOP RUN.
    "#;
    assert_eq!(run_capture(src), vec!["000", "[   ]"]);
}
