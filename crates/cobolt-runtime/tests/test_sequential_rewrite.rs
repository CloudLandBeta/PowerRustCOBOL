// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `REWRITE` on a record-SEQUENTIAL file opened `I-O`: it replaces the record
//! the last `READ` delivered, and the statuses it owes the program when it
//! cannot (NIST CCVS85 SQ116A, SQ133A, SQ134A).

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Tokenize, parse (asserting no errors), run, return captured DISPLAY lines.
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

/// A data file that removes itself, on a unique path so parallel runs of these
/// tests never share one. `ASSIGN TO` takes the absolute path.
struct TempData(std::path::PathBuf);

impl TempData {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("prc-rewrite-{tag}-{nanos}.dat"));
        let _ = std::fs::remove_file(&path);
        TempData(path)
    }

    fn path(&self) -> String {
        self.0.display().to_string()
    }
}

impl Drop for TempData {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// A fixed-length file, three records, with a `FILE STATUS` item and whatever
/// `PROCEDURE DIVISION` the test needs.
fn program(path: &str, body: &str) -> String {
    format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SEQRW.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "{path}"
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS FS.
       DATA DIVISION.
       FILE SECTION.
       FD  F
           RECORD CONTAINS 10 CHARACTERS.
       01  REC PIC X(10).
       WORKING-STORAGE SECTION.
       01  FS PIC XX.
       01  EOF-FLAG PIC 9 VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F.
           MOVE "AAAAAAAAAA" TO REC.
           WRITE REC.
           MOVE "BBBBBBBBBB" TO REC.
           WRITE REC.
           MOVE "CCCCCCCCCC" TO REC.
           WRITE REC.
           CLOSE F.
{body}
           STOP RUN.
"#
    )
}

/// The record the last `READ` delivered is the one replaced, in place — the
/// records around it are untouched and the next `READ` still gives the record
/// that follows (SQ116A).
#[test]
fn rewrite_replaces_the_record_just_read() {
    let data = TempData::new("inplace");
    let src = program(
        &data.path(),
        r#"           OPEN I-O F.
           READ F AT END CONTINUE END-READ.
           READ F AT END CONTINUE END-READ.
           MOVE "ZZZZZZZZZZ" TO REC.
           REWRITE REC.
           DISPLAY "RW=" FS.
           READ F AT END CONTINUE END-READ.
           DISPLAY "NEXT=" REC.
           CLOSE F.
           OPEN INPUT F.
           READ F AT END CONTINUE END-READ.
           DISPLAY "R1=" REC.
           READ F AT END CONTINUE END-READ.
           DISPLAY "R2=" REC.
           READ F AT END CONTINUE END-READ.
           DISPLAY "R3=" REC.
           CLOSE F."#,
    );
    let out = run_capture(&src);
    assert_eq!(out[0], "RW=00");
    // The rewrite did not disturb the read position.
    assert_eq!(out[1], "NEXT=CCCCCCCCCC");
    assert_eq!(out[2], "R1=AAAAAAAAAA");
    assert_eq!(out[3], "R2=ZZZZZZZZZZ");
    assert_eq!(out[4], "R3=CCCCCCCCCC");
}

/// `REWRITE` with no `READ` before it has no record to replace: status **43**.
/// The same applies to a second `REWRITE` with no `READ` between them — the
/// first one consumes the record.
#[test]
fn rewrite_without_a_preceding_read_is_43() {
    let data = TempData::new("noread");
    let src = program(
        &data.path(),
        r#"           OPEN I-O F.
           MOVE "ZZZZZZZZZZ" TO REC.
           REWRITE REC.
           DISPLAY "NOREAD=" FS.
           READ F AT END CONTINUE END-READ.
           REWRITE REC.
           DISPLAY "FIRST=" FS.
           REWRITE REC.
           DISPLAY "SECOND=" FS.
           CLOSE F."#,
    );
    let out = run_capture(&src);
    assert_eq!(out[0], "NOREAD=43");
    assert_eq!(out[1], "FIRST=00");
    assert_eq!(out[2], "SECOND=43");
}

/// `AT END` leaves no current record, so a `REWRITE` after it is **43** — not
/// a rewrite of the last record read before the end (SQ133A).
#[test]
fn rewrite_after_at_end_is_43() {
    let data = TempData::new("ateof");
    let src = program(
        &data.path(),
        r#"           OPEN I-O F.
           PERFORM UNTIL EOF-FLAG = 1
               READ F AT END MOVE 1 TO EOF-FLAG END-READ
           END-PERFORM.
           MOVE "ZZZZZZZZZZ" TO REC.
           REWRITE REC.
           DISPLAY "AFTEREOF=" FS.
           CLOSE F."#,
    );
    let out = run_capture(&src);
    assert_eq!(out[0], "AFTEREOF=43");
}

/// A sequential `REWRITE` may not change the record's length — the records
/// after it would have to move. Rewriting a 10-byte record over the 30-byte one
/// just read is **44** and leaves the file alone (SQ134A).
#[test]
fn rewrite_of_a_different_length_is_44() {
    let data = TempData::new("shorter");
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SEQRWVAR.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "{path}"
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS FS.
       DATA DIVISION.
       FILE SECTION.
       FD  F
           RECORD CONTAINS 10 TO 30 CHARACTERS.
       01  SHORT-REC PIC X(10).
       01  LONG-REC  PIC X(30).
       WORKING-STORAGE SECTION.
       01  FS PIC XX.
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F.
           MOVE "LONGLONGLONGLONGLONGLONGLONGLO" TO LONG-REC.
           WRITE LONG-REC.
           CLOSE F.
           OPEN I-O F.
           READ F AT END CONTINUE END-READ.
           MOVE "SHORTSHORT" TO SHORT-REC.
           REWRITE SHORT-REC.
           DISPLAY "SHORTER=" FS.
           CLOSE F.
           OPEN INPUT F.
           READ F AT END CONTINUE END-READ.
           DISPLAY "STILL=" LONG-REC.
           CLOSE F.
           STOP RUN.
"#,
        path = data.path()
    );
    let out = run_capture(&src);
    assert_eq!(out[0], "SHORTER=44");
    assert_eq!(out[1], "STILL=LONGLONGLONGLONGLONGLONGLONGLO");
}

/// `REWRITE` needs the file open `I-O`. On a file open `INPUT` it is **49**,
/// and the file is left as it was.
#[test]
fn rewrite_on_a_file_not_open_io_is_49() {
    let data = TempData::new("notio");
    let src = program(
        &data.path(),
        r#"           OPEN INPUT F.
           READ F AT END CONTINUE END-READ.
           MOVE "ZZZZZZZZZZ" TO REC.
           REWRITE REC.
           DISPLAY "INPUT=" FS.
           CLOSE F.
           OPEN INPUT F.
           READ F AT END CONTINUE END-READ.
           DISPLAY "R1=" REC.
           CLOSE F."#,
    );
    let out = run_capture(&src);
    assert_eq!(out[0], "INPUT=49");
    assert_eq!(out[1], "R1=AAAAAAAAAA");
}
