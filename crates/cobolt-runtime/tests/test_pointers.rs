// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Pointer operations: USAGE POINTER, SET ptr TO ADDRESS OF / NULL,
//! SET ADDRESS OF item TO ptr (aliasing, read + write through), IF ptr = NULL.

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
fn pointer_null_address_of_and_aliasing() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PTR.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A   PIC X(5) VALUE "HELLO".
       01 WS-PTR USAGE POINTER.
       LINKAGE SECTION.
       01 LK-X PIC X(5).
       PROCEDURE DIVISION.
       MAIN.
           IF WS-PTR = NULL DISPLAY "INIT-NULL" END-IF
           SET WS-PTR TO ADDRESS OF WS-A
           SET ADDRESS OF LK-X TO WS-PTR
           DISPLAY LK-X
           MOVE "WORLD" TO LK-X
           DISPLAY WS-A
           SET WS-PTR TO NULL
           IF WS-PTR = NULL DISPLAY "PTR-NULL" END-IF
           STOP RUN.
    "#;
    assert_eq!(
        run_capture(src),
        vec!["INIT-NULL", "HELLO", "WORLD", "PTR-NULL"]
    );
}
