// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
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
fn data_binding_runtime_initial_load_does_not_mark_dirty() {
    let out = run_capture(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BIND-LOAD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STATUS PIC X(64) VALUE SPACES.
       01 WS-DIRTY  PIC 9 VALUE 9.
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-BINDING-LOAD" USING "BIND-1" WS-STATUS.
           CALL "COBOL-BINDING-POPULATE" USING "BIND-1" WS-STATUS.
           CALL "COBOL-BINDING-MARK-CLEAN" USING "BIND-1" WS-DIRTY.
           DISPLAY WS-STATUS.
           DISPLAY WS-DIRTY.
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["", "0"]);
}

#[test]
fn data_binding_runtime_writable_update_preserves_identity_and_clears_dirty() {
    let out = run_capture(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BIND-WRITE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STATUS PIC X(64) VALUE SPACES.
       01 WS-DIRTY  PIC 9 VALUE 0.
       01 WS-KEY    PIC X(16) VALUE "C001".
       01 WS-VALUE  PIC X(16) VALUE "Alice".
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-BINDING-SET-READ-ONLY" USING "BIND-1" "0".
           CALL "COBOL-BINDING-SET-PENDING" USING "BIND-1" WS-KEY WS-VALUE WS-DIRTY.
           DISPLAY WS-DIRTY.
           CALL "COBOL-BINDING-UPDATE" USING "BIND-1" WS-KEY WS-STATUS.
           CALL "COBOL-BINDING-MARK-CLEAN" USING "BIND-1" WS-DIRTY.
           DISPLAY WS-STATUS.
           DISPLAY WS-DIRTY.
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["1", "", "0"]);
}

#[test]
fn data_binding_runtime_read_only_never_writes_back() {
    let out = run_capture(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BIND-RO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STATUS PIC X(64) VALUE SPACES.
       01 WS-DIRTY  PIC 9 VALUE 0.
       01 WS-KEY    PIC X(16) VALUE "C001".
       01 WS-VALUE  PIC X(16) VALUE "Alice".
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-BINDING-SET-READ-ONLY" USING "BIND-1" "1".
           CALL "COBOL-BINDING-SET-PENDING" USING "BIND-1" WS-KEY WS-VALUE WS-DIRTY.
           CALL "COBOL-BINDING-UPDATE" USING "BIND-1" WS-KEY WS-STATUS.
           DISPLAY WS-STATUS.
           DISPLAY WS-DIRTY.
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["READ-ONLY", "1"]);
}

#[test]
fn data_binding_runtime_failed_update_keeps_pending_edits_recoverable() {
    let out = run_capture(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BIND-FAIL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STATUS PIC X(64) VALUE SPACES.
       01 WS-DIRTY  PIC 9 VALUE 0.
       01 WS-KEY    PIC X(16) VALUE SPACES.
       01 WS-VALUE  PIC X(16) VALUE "Alice".
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-BINDING-SET-READ-ONLY" USING "BIND-1" "0".
           CALL "COBOL-BINDING-SET-PENDING" USING "BIND-1" WS-KEY WS-VALUE WS-DIRTY.
           CALL "COBOL-BINDING-UPDATE" USING "BIND-1" WS-KEY WS-STATUS.
           DISPLAY WS-STATUS.
           DISPLAY WS-DIRTY.
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["MISSING-ROW-KEY", "1"]);
}
