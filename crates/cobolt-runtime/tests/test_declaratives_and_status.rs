// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DECLARATIVES that keep their paragraph names, and the FILE STATUS codes the
//! sequential error paths owe the program: a group status item, 41 on an OPEN
//! of an open file, and 46 on a READ past AT END.

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
        let path = std::env::temp_dir().join(format!("prc-decltest-{tag}-{nanos}.dat"));
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

/// A declarative's paragraphs keep their names, so the handler can `PERFORM`
/// and `GO TO` them. Flattening the section threw the names away and
/// `PERFORM DECL-FAIL` died with "undefined paragraph" (SQ132A, SQ122A).
#[test]
fn a_declarative_performs_and_jumps_to_its_own_paragraphs() {
    let data = TempData::new("perform");
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DECLPERF.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "{path}"
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS FS.
       DATA DIVISION.
       FILE SECTION.
       FD F.
       01 F-REC PIC X(10).
       WORKING-STORAGE SECTION.
       01 FS PIC XX VALUE "  ".
       PROCEDURE DIVISION.
       DECLARATIVES.
       ERR-SECT SECTION.
           USE AFTER STANDARD ERROR PROCEDURE ON F.
       DECL-ENTRY.
           DISPLAY "ENTRY " FS
           PERFORM DECL-NOTE
           GO TO DECL-END.
       DECL-SKIPPED.
           DISPLAY "SKIPPED".
       DECL-NOTE.
           DISPLAY "NOTE".
       DECL-END.
           DISPLAY "END".
       END DECLARATIVES.
       MAIN SECTION.
       DO-IT.
           CLOSE F
           DISPLAY "AFTER " FS
           STOP RUN.
    "#,
        path = data.path()
    );
    // ENTRY/NOTE/END run; DECL-SKIPPED sits between the GO TO's origin and its
    // target and must not, and control returns to the statement after CLOSE.
    assert_eq!(
        run_capture(&src),
        vec!["ENTRY 42", "NOTE", "END", "AFTER 42"]
    );
}

/// Control never *falls* out of the main body into the declaratives: the
/// handler's paragraphs live in their own name space and the body ends where
/// it ends.
#[test]
fn the_main_body_never_falls_into_a_declarative() {
    let data = TempData::new("fallthrough");
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DECLFALL.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "{path}"
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS FS.
       DATA DIVISION.
       FILE SECTION.
       FD F.
       01 F-REC PIC X(10).
       WORKING-STORAGE SECTION.
       01 FS PIC XX VALUE "  ".
       PROCEDURE DIVISION.
       DECLARATIVES.
       ERR-SECT SECTION.
           USE AFTER STANDARD ERROR PROCEDURE ON F.
       DECL-BODY.
           DISPLAY "HANDLER".
       END DECLARATIVES.
       MAIN SECTION.
       DO-IT.
           DISPLAY "BODY".
    "#,
        path = data.path()
    );
    // No I/O error, so the handler never runs; running off the end of DO-IT
    // ends the program instead of entering ERR-SECT.
    assert_eq!(run_capture(&src), vec!["BODY"]);
}

/// COBOL-85 allows the FILE STATUS item to be a two-character **group**.
/// SQ132A declares `01 SQ-FS1-STATUS` over two `PIC X` children; writing the
/// code to the group's own slot left the children holding their seed, so every
/// status test on the file compared against that seed and failed.
#[test]
fn a_group_file_status_item_receives_the_code() {
    let data = TempData::new("groupstatus");
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GRPSTAT.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "{path}"
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS FS.
       DATA DIVISION.
       FILE SECTION.
       FD F.
       01 F-REC PIC X(10).
       WORKING-STORAGE SECTION.
       01 FS.
          03 FS-1 PIC X.
          03 FS-2 PIC X.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "**" TO FS
           CLOSE F
           DISPLAY FS
           DISPLAY FS-1
           DISPLAY FS-2
           STOP RUN.
    "#,
        path = data.path()
    );
    // The group reads back from its children, so the children must carry it.
    assert_eq!(run_capture(&src), vec!["42", "4", "2"]);
}

/// `OPEN` of a file that is already open is status 41, and the file is left
/// exactly as it was — re-opening it truncated an OUTPUT file the program had
/// just written and reported success (SQ139A/SQ140A, SQ131A).
#[test]
fn opening_an_open_file_is_41_and_does_not_reopen_it() {
    let data = TempData::new("open41");
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OPEN41.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "{path}"
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS FS.
       DATA DIVISION.
       FILE SECTION.
       FD F.
       01 F-REC PIC X(5).
       WORKING-STORAGE SECTION.
       01 FS PIC XX VALUE "  ".
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F
           MOVE "AAAAA" TO F-REC
           WRITE F-REC
           OPEN OUTPUT F
           DISPLAY "REOPEN " FS
           CLOSE F
           OPEN INPUT F
           READ F
               AT END DISPLAY "EMPTY"
           END-READ
           DISPLAY "REC " F-REC
           CLOSE F
           STOP RUN.
    "#,
        path = data.path()
    );
    // The record written before the second OPEN survives it.
    assert_eq!(run_capture(&src), vec!["REOPEN 41", "REC AAAAA"]);
}

/// A sequential `READ` once `AT END` has been reached is 46, not a second 10:
/// the AT END left no valid next record (SQ136A–SQ138A). 46 is class 4, so the
/// `AT END` phrase does not run for it.
#[test]
fn reading_past_at_end_is_46() {
    let data = TempData::new("read46");
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. READ46.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "{path}"
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS FS.
       DATA DIVISION.
       FILE SECTION.
       FD F.
       01 F-REC PIC X(5).
       WORKING-STORAGE SECTION.
       01 FS PIC XX VALUE "  ".
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F
           MOVE "AAAAA" TO F-REC
           WRITE F-REC
           CLOSE F
           OPEN INPUT F
           READ F AT END DISPLAY "END1" END-READ
           DISPLAY "R1 " FS
           READ F AT END DISPLAY "END2" END-READ
           DISPLAY "R2 " FS
           READ F AT END DISPLAY "END3" END-READ
           DISPLAY "R3 " FS
           CLOSE F
           OPEN INPUT F
           READ F AT END DISPLAY "END4" END-READ
           DISPLAY "R4 " FS
           CLOSE F
           STOP RUN.
    "#,
        path = data.path()
    );
    // R2 is the AT END (10); R3 reads on and is 46 with no AT END phrase run;
    // re-OPENing positions before the first record again, so R4 is 00.
    assert_eq!(
        run_capture(&src),
        vec!["R1 00", "END2", "R2 10", "R3 46", "R4 00"]
    );
}

/// COBOL-85's multi-phrase `OPEN` — `OPEN INPUT f1 OUTPUT f2` — opens each
/// group in its own mode. Reading exactly one mode left `OUTPUT` unconsumed and
/// the statement was rejected outright (SQ128A, SQ206A).
#[test]
fn one_open_carries_several_mode_groups() {
    let data1 = TempData::new("multi1");
    let data2 = TempData::new("multi2");
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MULTIOPEN.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F1 ASSIGN TO "{p1}"
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS FS1.
           SELECT F2 ASSIGN TO "{p2}"
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS FS2.
       DATA DIVISION.
       FILE SECTION.
       FD F1.
       01 F1-REC PIC X(5).
       FD F2.
       01 F2-REC PIC X(5).
       WORKING-STORAGE SECTION.
       01 FS1 PIC XX VALUE "  ".
       01 FS2 PIC XX VALUE "  ".
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F1
           MOVE "HELLO" TO F1-REC
           WRITE F1-REC
           CLOSE F1
           OPEN INPUT F1
                OUTPUT F2
           DISPLAY "S1 " FS1
           DISPLAY "S2 " FS2
           READ F1 AT END DISPLAY "EMPTY" END-READ
           DISPLAY "GOT " F1-REC
           MOVE "WORLD" TO F2-REC
           WRITE F2-REC
           DISPLAY "W2 " FS2
           CLOSE F1
           CLOSE F2
           STOP RUN.
    "#,
        p1 = data1.path(),
        p2 = data2.path()
    );
    assert_eq!(
        run_capture(&src),
        vec!["S1 00", "S2 00", "GOT HELLO", "W2 00"]
    );
}
