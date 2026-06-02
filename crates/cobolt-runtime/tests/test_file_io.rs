// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Integration tests for COBOL SEQUENTIAL / LINE SEQUENTIAL file I/O
//! (SELECT … ASSIGN … ORGANIZATION, OPEN/WRITE/READ/CLOSE, FILE STATUS).

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_get(src: &str, var: &str) -> String {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let mut i = Interpreter::new(result.program.expect("no program"));
    i.run().expect("run failed");
    i.env.get_string(var).unwrap_or_default().trim().to_owned()
}

fn tmp_path(name: &str) -> String {
    std::env::temp_dir()
        .join(format!("rcrun-fio-{}-{}", std::process::id(), name))
        .to_string_lossy()
        .into_owned()
}

#[test]
fn line_sequential_write_creates_newline_terminated_file() {
    let path = tmp_path("write.txt");
    let _ = std::fs::remove_file(&path);
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. W.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO WS-PATH
               ORGANIZATION IS LINE SEQUENTIAL
               FILE STATUS IS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD F.
       01 F-REC PIC X(40).
       WORKING-STORAGE SECTION.
       01 WS-PATH PIC X(80) VALUE "{path}".
       01 WS-FS   PIC X(2)  VALUE "  ".
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F
           WRITE F-REC FROM "alpha"
           WRITE F-REC FROM "beta"
           CLOSE F
           STOP RUN.
    "#,
        path = path
    );
    let fs = run_get(&src, "WS-FS");
    assert_eq!(fs, "00", "FILE STATUS should be 00 after a clean write");
    // Trailing spaces of the 40-char record are not stored; each record ends \n.
    let contents = std::fs::read_to_string(&path).expect("file written");
    assert_eq!(contents, "alpha\nbeta\n");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn extend_appends_and_read_loop_counts_all_records() {
    let path = tmp_path("loop.txt");
    let _ = std::fs::remove_file(&path);
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RW.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO WS-PATH
               ORGANIZATION IS LINE SEQUENTIAL
               FILE STATUS IS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD F.
       01 F-REC PIC X(20).
       WORKING-STORAGE SECTION.
       01 WS-PATH PIC X(80) VALUE "{path}".
       01 WS-FS   PIC X(2)  VALUE "  ".
       01 WS-EOF  PIC 9     VALUE 0.
       01 WS-N    PIC 9(3)  VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F
           WRITE F-REC FROM "one"
           CLOSE F
           OPEN EXTEND F
           WRITE F-REC FROM "two"
           WRITE F-REC FROM "three"
           CLOSE F
           OPEN INPUT F
           PERFORM UNTIL WS-EOF = 1
               READ F
                   AT END MOVE 1 TO WS-EOF
                   NOT AT END ADD 1 TO WS-N
               END-READ
           END-PERFORM
           CLOSE F
           STOP RUN.
    "#,
        path = path
    );
    assert_eq!(run_get(&src, "WS-N"), "3", "should read all 3 appended records");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn open_input_missing_file_sets_status_35() {
    let path = tmp_path("does-not-exist.txt");
    let _ = std::fs::remove_file(&path);
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. M.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO WS-PATH
               ORGANIZATION IS LINE SEQUENTIAL
               FILE STATUS IS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD F.
       01 F-REC PIC X(10).
       WORKING-STORAGE SECTION.
       01 WS-PATH PIC X(80) VALUE "{path}".
       01 WS-FS   PIC X(2)  VALUE "  ".
       PROCEDURE DIVISION.
       MAIN.
           OPEN INPUT F
           STOP RUN.
    "#,
        path = path
    );
    assert_eq!(run_get(&src, "WS-FS"), "35", "missing INPUT file → status 35");
}

#[test]
fn record_sequential_writes_without_newlines() {
    let path = tmp_path("recseq.bin");
    let _ = std::fs::remove_file(&path);
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RS.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO WS-PATH
               ORGANIZATION IS SEQUENTIAL.
       DATA DIVISION.
       FILE SECTION.
       FD F.
       01 F-REC PIC X(5).
       WORKING-STORAGE SECTION.
       01 WS-PATH PIC X(80) VALUE "{path}".
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F
           WRITE F-REC FROM "AB"
           WRITE F-REC FROM "CD"
           CLOSE F
           STOP RUN.
    "#,
        path = path
    );
    run_get(&src, "WS-PATH");
    // Record sequential keeps the full fixed-length record, no newline:
    // "AB   " + "CD   " (each padded to PIC X(5)).
    let contents = std::fs::read_to_string(&path).expect("file written");
    assert_eq!(contents, "AB   CD   ");
    let _ = std::fs::remove_file(&path);
}
