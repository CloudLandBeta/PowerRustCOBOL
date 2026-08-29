// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Variable-length records on a record-SEQUENTIAL file: the FD `RECORD` clause
//! in its three spellings, the length a `WRITE` takes and the length a `READ`
//! hands back (NIST CCVS85 SQ106A, SQ134A, SQ212A, SQ218A–SQ224A, SQ227A).

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
        let path = std::env::temp_dir().join(format!("prc-varying-{tag}-{nanos}.dat"));
        let _ = std::fs::remove_file(&path);
        TempData(path)
    }

    fn path(&self) -> String {
        self.0.display().to_string()
    }

    fn len(&self) -> u64 {
        std::fs::metadata(&self.0).map(|m| m.len()).unwrap_or(0)
    }
}

impl Drop for TempData {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// `RECORD IS VARYING … DEPENDING ON item` makes the data item the record
/// length in both directions: the program sets it before a `WRITE`, and a
/// `READ` sets it back to the length of the record it delivered (SQ220A read
/// 120 then 151 and got 0 both times, because nothing wrote the item back).
#[test]
fn depending_on_carries_the_length_both_ways() {
    let data = TempData::new("dep");
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. VARYDEP.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "{path}"
               ORGANIZATION IS SEQUENTIAL.
       DATA DIVISION.
       FILE SECTION.
       FD  F
           RECORD IS VARYING IN SIZE FROM 10 TO 30 CHARACTERS
             DEPENDING ON REC-LEN.
       01  SHORT-REC.
           02  SHORT-TEXT PIC X(10).
       01  LONG-REC.
           02  LONG-TEXT  PIC X(10).
           02  LONG-TAIL  PIC X(20).
       WORKING-STORAGE SECTION.
       01  REC-LEN PIC 999 VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F.
           MOVE "SHORTSHORT" TO SHORT-TEXT.
           MOVE 10 TO REC-LEN.
           WRITE SHORT-REC.
           MOVE "LONGLONGLO" TO LONG-TEXT.
           MOVE "TAILTAILTAILTAILTAIL" TO LONG-TAIL.
           MOVE 30 TO REC-LEN.
           WRITE LONG-REC.
           CLOSE F.
           MOVE ZERO TO REC-LEN.
           OPEN INPUT F.
           READ F AT END CONTINUE END-READ.
           DISPLAY "R1 LEN=" REC-LEN " T=" SHORT-TEXT.
           READ F AT END CONTINUE END-READ.
           DISPLAY "R2 LEN=" REC-LEN " T=" LONG-TEXT " TAIL=" LONG-TAIL.
           CLOSE F.
           STOP RUN.
"#,
        path = data.path()
    );
    let out = run_capture(&src);
    assert_eq!(out[0], "R1 LEN=010 T=SHORTSHORT");
    assert_eq!(
        out[1],
        "R2 LEN=030 T=LONGLONGLO TAIL=TAILTAILTAILTAILTAIL"
    );
}

/// Without `DEPENDING ON`, the record description the `WRITE` names is what
/// sizes the record — an FD with a 10-byte and a 30-byte 01 writes two
/// different lengths, and each `READ` gets back the one that was written
/// (SQ218A, SQ222A).
#[test]
fn the_written_record_name_sizes_the_record() {
    let data = TempData::new("named");
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. VARYNAME.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "{path}"
               ORGANIZATION IS SEQUENTIAL.
       DATA DIVISION.
       FILE SECTION.
       FD  F
           RECORD CONTAINS 10 TO 30 CHARACTERS.
       01  SHORT-REC.
           02  SHORT-TEXT PIC X(10).
       01  LONG-REC.
           02  LONG-TEXT  PIC X(10).
           02  LONG-TAIL  PIC X(20).
       WORKING-STORAGE SECTION.
       01  EOF-FLAG PIC 9 VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F.
           MOVE "SHORTSHORT" TO SHORT-TEXT.
           WRITE SHORT-REC.
           MOVE "LONGLONGLO" TO LONG-TEXT.
           MOVE "TAILTAILTAILTAILTAIL" TO LONG-TAIL.
           WRITE LONG-REC.
           CLOSE F.
           MOVE SPACES TO LONG-TAIL.
           OPEN INPUT F.
           READ F AT END CONTINUE END-READ.
           DISPLAY "R1 T=" SHORT-TEXT " TAIL=[" LONG-TAIL "]".
           READ F AT END CONTINUE END-READ.
           DISPLAY "R2 T=" LONG-TEXT " TAIL=[" LONG-TAIL "]".
           READ F AT END MOVE 1 TO EOF-FLAG END-READ.
           DISPLAY "EOF=" EOF-FLAG.
           CLOSE F.
           STOP RUN.
"#,
        path = data.path()
    );
    let out = run_capture(&src);
    // The short record leaves the long record's tail as it was — 10 bytes
    // arrived, and `distribute` has nothing to put past them.
    assert_eq!(out[0], "R1 T=SHORTSHORT TAIL=[                    ]");
    assert_eq!(out[1], "R2 T=LONGLONGLO TAIL=[TAILTAILTAILTAILTAIL]");
    assert_eq!(out[2], "EOF=1");
}

/// `RECORD VARYING.` — the bare form, with neither bounds nor a `DEPENDING ON`
/// item — still makes the file variable-length (SQ221A, SQ222A, SQ227A).
#[test]
fn bare_record_varying_is_still_variable_length() {
    let data = TempData::new("bare");
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. VARYBARE.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "{path}"
               ORGANIZATION IS SEQUENTIAL.
       DATA DIVISION.
       FILE SECTION.
       FD  F
           RECORD VARYING.
       01  SHORT-REC PIC X(10).
       01  LONG-REC  PIC X(30).
       WORKING-STORAGE SECTION.
       01  FILLER PIC X.
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F.
           MOVE "SHORTSHORT" TO SHORT-REC.
           WRITE SHORT-REC.
           MOVE "LONGLONGLONGLONGLONGLONGLONGLO" TO LONG-REC.
           WRITE LONG-REC.
           CLOSE F.
           STOP RUN.
"#,
        path = data.path()
    );
    run_capture(&src);
    // Two records of 10 and 30 bytes, each behind a 4-byte length prefix.
    assert_eq!(data.len(), 4 + 10 + 4 + 30);
}

/// `RECORD CONTAINS n CHARACTERS` is the *fixed*-length spelling, so the file
/// stays a plain run of equal records with no length prefix. Reading `RECORD`
/// as a size clause must not turn every FD that mentions the word into a
/// variable-length file (SQ110A, SQ114A and most of SQ write fixed records).
#[test]
fn record_contains_one_size_stays_fixed_length() {
    let data = TempData::new("fixed");
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FIXEDLEN.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "{path}"
               ORGANIZATION IS SEQUENTIAL.
       DATA DIVISION.
       FILE SECTION.
       FD  F
           LABEL RECORDS ARE STANDARD
           RECORD CONTAINS 10 CHARACTERS
           DATA RECORD IS THE-REC.
       01  THE-REC PIC X(10).
       WORKING-STORAGE SECTION.
       01  FILLER PIC X.
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F.
           MOVE "AAAAAAAAAA" TO THE-REC.
           WRITE THE-REC.
           MOVE "BBBBBBBBBB" TO THE-REC.
           WRITE THE-REC.
           CLOSE F.
           OPEN INPUT F.
           READ F AT END CONTINUE END-READ.
           DISPLAY "R1=" THE-REC.
           READ F AT END CONTINUE END-READ.
           DISPLAY "R2=" THE-REC.
           CLOSE F.
           STOP RUN.
"#,
        path = data.path()
    );
    let out = run_capture(&src);
    assert_eq!(data.len(), 20, "no length prefix on a fixed-length file");
    assert_eq!(out[0], "R1=AAAAAAAAAA");
    assert_eq!(out[1], "R2=BBBBBBBBBB");
}

/// A `DEPENDING ON` length outside the FD's declared `FROM … TO` range is a
/// boundary violation — status **44**, nothing written. Clamping it into range
/// looked protective and hid the very error the program asked to be told about
/// (SQ212A rewrites with 15 on a `FROM 18` file and expects 44).
#[test]
fn a_length_outside_the_declared_range_is_44() {
    let data = TempData::new("range");
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. VARYRANGE.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "{path}"
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS FS.
       DATA DIVISION.
       FILE SECTION.
       FD  F
           RECORD IS VARYING IN SIZE FROM 10 TO 30 CHARACTERS
             DEPENDING ON REC-LEN.
       01  REC PIC X(30).
       WORKING-STORAGE SECTION.
       01  FS      PIC XX.
       01  REC-LEN PIC 999 VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F.
           MOVE "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" TO REC.
           MOVE 5 TO REC-LEN.
           WRITE REC.
           DISPLAY "TOOSHORT=" FS.
           MOVE 40 TO REC-LEN.
           WRITE REC.
           DISPLAY "TOOLONG=" FS.
           MOVE 20 TO REC-LEN.
           WRITE REC.
           DISPLAY "INRANGE=" FS.
           CLOSE F.
           STOP RUN.
"#,
        path = data.path()
    );
    let out = run_capture(&src);
    assert_eq!(out[0], "TOOSHORT=44");
    assert_eq!(out[1], "TOOLONG=44");
    assert_eq!(out[2], "INRANGE=00");
    // Only the one in-range record reached the file, behind its length prefix.
    assert_eq!(data.len(), 4 + 20);
}
