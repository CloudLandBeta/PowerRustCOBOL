// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 009 R6 / R9 / AC3:
//! - A `GLOBAL` `FD` opened by the form (outer) program is usable from a nested
//!   procedure (`IS COMMON`) — it reads the same open file / record.
//! - A procedure-local data item is **private**: a `GLOBAL` clause on it does not
//!   share it with a sibling procedure.

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

fn tmp_path(name: &str) -> String {
    std::env::temp_dir()
        .join(format!("rcrun-009-{}-{}", std::process::id(), name))
        .to_string_lossy()
        .into_owned()
}

#[test]
fn global_fd_opened_in_form_is_read_by_nested_procedure() {
    let path = tmp_path("globalfd.txt");
    std::fs::write(&path, "HELLO\n").expect("seed file");

    // OUTER opens the GLOBAL file; the nested procedure READER (IS COMMON) reads a
    // record from it and displays the record area — proving the GLOBAL FD opened
    // by the form is usable from the procedure (R6).
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO WS-PATH
               ORGANIZATION IS LINE SEQUENTIAL
               FILE STATUS IS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD F IS GLOBAL.
       01 F-REC PIC X(20).
       WORKING-STORAGE SECTION.
       01 WS-PATH PIC X(80) VALUE "{path}".
       01 WS-FS   PIC X(2)  VALUE "  ".
       PROCEDURE DIVISION.
           OPEN INPUT F.
           CALL "READER".
           CLOSE F.
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. READER IS COMMON PROGRAM.
       PROCEDURE DIVISION.
           READ F.
           DISPLAY F-REC.
           GOBACK.
       END PROGRAM READER.

       END PROGRAM OUTER.
    "#,
        path = path
    );
    let out = run_capture(&src);
    assert!(
        out.iter().any(|l| l.starts_with("HELLO")),
        "nested procedure should read the GLOBAL FD opened by the form: {out:?}"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn procedure_local_global_item_is_not_shared_with_sibling() {
    // PROC-A sets its OWN local `P-LOCAL` (marked GLOBAL — a no-op for sharing
    // since it is a leaf procedure) to 7. PROC-B has its own `P-LOCAL`; it must
    // see its fresh 0, never PROC-A's 7 (R9 — procedure-local privacy).
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       PROCEDURE DIVISION.
           CALL "PROC-A".
           CALL "PROC-B".
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PROC-A IS COMMON PROGRAM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 P-LOCAL PIC 9(4) GLOBAL VALUE 0.
       PROCEDURE DIVISION.
           MOVE 7 TO P-LOCAL.
           GOBACK.
       END PROGRAM PROC-A.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PROC-B IS COMMON PROGRAM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 P-LOCAL PIC 9(4) VALUE 0.
       PROCEDURE DIVISION.
           DISPLAY P-LOCAL.
           GOBACK.
       END PROGRAM PROC-B.

       END PROGRAM OUTER.
"#;
    let out = run_capture(src);
    assert_eq!(
        out,
        vec!["0"],
        "PROC-A's local GLOBAL item must NOT be visible to sibling PROC-B (R9)"
    );
}
