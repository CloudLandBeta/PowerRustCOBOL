// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `OPEN … SHARING / WITH LOCK`, `READ … WITH [NO] LOCK`, `UNLOCK`, and `CANCEL`.

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
fn open_lock_read_lock_unlock_flow() {
    let path = std::env::temp_dir().join("prc_lock_test.dat");
    let _ = std::fs::remove_file(&path);
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. LK.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT IDX ASSIGN TO "{p}"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS IDX-KEY
               FILE STATUS IS WS-ST.
       DATA DIVISION.
       FILE SECTION.
       FD IDX.
       01 IDX-REC.
          05 IDX-KEY  PIC X(3).
          05 IDX-DATA PIC X(5).
       WORKING-STORAGE SECTION.
       01 WS-ST PIC XX.
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT IDX WITH LOCK
           MOVE "001" TO IDX-KEY MOVE "AAAAA" TO IDX-DATA
           WRITE IDX-REC
           CLOSE IDX
           OPEN I-O IDX SHARING WITH ALL OTHER
           MOVE "001" TO IDX-KEY
           READ IDX WITH LOCK
               INVALID KEY DISPLAY "NF"
               NOT INVALID KEY DISPLAY IDX-DATA
           END-READ
           UNLOCK IDX
           READ IDX WITH NO LOCK
               NOT INVALID KEY DISPLAY IDX-DATA
           END-READ
           CLOSE IDX
           DISPLAY "DONE"
           STOP RUN.
    "#,
        p = path.display()
    );
    assert_eq!(run_capture(&src), vec!["AAAAA", "AAAAA", "DONE"]);
}

#[test]
fn cancel_runs_and_recalls() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CAN.
       PROCEDURE DIVISION.
       MAIN.
           CALL "SUBP"
           CANCEL "SUBP"
           CALL "SUBP"
           DISPLAY "DONE"
           STOP RUN.
       SUBP.
           DISPLAY "IN-SUB".
    "#;
    assert_eq!(run_capture(src), vec!["IN-SUB", "IN-SUB", "DONE"]);
}
