// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `COMMIT` / `ROLLBACK` for INDEXED files (program-controlled transactions),
//! on both the memory and the disk engines.

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

/// COMMIT makes a record durable; ROLLBACK then undoes a later WRITE/REWRITE/
/// DELETE — leaving exactly the pre-rollback-but-post-commit state.
fn tx_program(path: &std::path::Path, storage: &str) -> String {
    format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TX.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "{p}"
               ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC
               RECORD KEY IS R-COD  STORAGE IS {storage}
               FILE STATUS IS WS-ST.
       DATA DIVISION.
       FILE SECTION.
       FD F.
       01 R-REC.
          05 R-COD  PIC 9(4).
          05 R-NOME PIC X(8).
       WORKING-STORAGE SECTION.
       01 WS-ST PIC XX.
       01 WS-EOF PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F
           MOVE 0001 TO R-COD MOVE "ALPHA" TO R-NOME WRITE R-REC
           MOVE 0002 TO R-COD MOVE "BETA" TO R-NOME WRITE R-REC
           CLOSE F
           OPEN I-O F
           MOVE 0003 TO R-COD MOVE "GAMMA" TO R-NOME WRITE R-REC
           COMMIT
           MOVE 0004 TO R-COD MOVE "DELTA" TO R-NOME WRITE R-REC
           MOVE 0001 TO R-COD READ F END-READ
           MOVE "ALPHAX" TO R-NOME REWRITE R-REC
           MOVE 0002 TO R-COD DELETE F
           ROLLBACK
           MOVE 0000 TO R-COD
           START F KEY IS GREATER THAN R-COD END-START
           PERFORM UNTIL WS-EOF = 1
               READ F NEXT
                   AT END MOVE 1 TO WS-EOF
                   NOT AT END DISPLAY R-COD " " R-NOME
               END-READ
           END-PERFORM
           CLOSE F
           STOP RUN.
    "#,
        p = path.display()
    )
}

#[test]
fn commit_rollback_disk_engine() {
    let path = std::env::temp_dir().join("prc_tx_disk.idx");
    let _ = std::fs::remove_file(&path);
    let out = run_capture(&tx_program(&path, "DISK"));
    // GAMMA committed survives; ALPHA/BETA restored; DELTA gone.
    assert_eq!(out, vec!["0001 ALPHA", "0002 BETA", "0003 GAMMA"]);
}

#[test]
fn commit_rollback_memory_engine() {
    let path = std::env::temp_dir().join("prc_tx_mem.idx");
    let _ = std::fs::remove_file(&path);
    let out = run_capture(&tx_program(&path, "MEMORY"));
    assert_eq!(out, vec!["0001 ALPHA", "0002 BETA", "0003 GAMMA"]);
}
