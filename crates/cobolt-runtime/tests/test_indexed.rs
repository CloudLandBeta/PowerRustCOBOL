// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Integration tests for INDEXED (ISAM) files.
//!
//! The flagship case is `idxbasic.cbl` — the project's indexed-file regression
//! suite at `tests/cobol/indexed-files/idxbasic.cbl` — executed end-to-end and
//! asserted to report `RESULT : PASS`. It exercises every indexed verb,
//! dispatched purely by `ORGANIZATION IS INDEXED` in the SELECT: OPEN
//! OUTPUT/INPUT/I-O, WRITE (incl. duplicate-key INVALID KEY), READ (random by
//! RECORD KEY and sequential NEXT with AT END), REWRITE, DELETE, and START with
//! relational key operators. The remaining tests pin focused behaviours.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// A unique temp path so parallel test runs never share an `.idx` container.
fn temp_idx(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("prc-idxtest-{tag}-{nanos}.idx"))
}

/// Tokenize, parse (asserting no errors), run, return captured DISPLAY lines.
fn run_capture(src: &str) -> Vec<String> {
    run_capture_fmt(src, SourceFormat::Free)
}

fn run_capture_fmt(src: &str, fmt: SourceFormat) -> Vec<String> {
    let tokens = tokenize(src, fmt);
    let result = parse(tokens);
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
    display_rx.try_iter().collect()
}

// ── The indexed-file regression suite ──────────────────────────────────────────

#[test]
fn idxbasic_suite_reports_pass() {
    let raw = include_str!("../../../tests/cobol/indexed-files/idxbasic.cbl");
    // Redirect the file's relative ASSIGN to a unique temp container so the test
    // is hermetic and never collides with another run.
    let path = temp_idx("basic");
    let _ = std::fs::remove_file(&path);
    let src = raw.replace("\"idxbasic.idx\"", &format!("\"{}\"", path.display()));

    let out = run_capture(&src).join("\n");
    let _ = std::fs::remove_file(&path);

    assert!(out.contains("RESULT       : PASS"), "idxbasic suite did not pass:\n{out}");
    assert!(!out.contains("FAIL T"), "idxbasic reported failures:\n{out}");
    assert_eq!(out.matches("PASS T").count(), 13, "expected 13 PASS lines:\n{out}");
}

#[test]
fn idxstorage_disk_suite_reports_pass() {
    // The STORAGE IS DISK WITH COMPRESSION regression suite, run end
    // to end on the on-disk B+tree backend (ASSIGN redirected to a temp file).
    let raw = include_str!("../../../tests/cobol/indexed-files/idxstorage.cbl");
    let path = temp_idx("storage");
    let _ = std::fs::remove_file(&path);
    let src = raw.replace("\"idxstorage.idx\"", &format!("\"{}\"", path.display()));

    let out = run_capture(&src).join("\n");
    let _ = std::fs::remove_file(&path);

    assert!(out.contains("RESULT       : PASS"), "idxstorage suite did not pass:\n{out}");
    assert!(!out.contains("FAIL T"), "idxstorage reported failures:\n{out}");
    assert_eq!(out.matches("PASS T").count(), 11, "expected 11 PASS lines:\n{out}");
}

// ── Focused behaviours ─────────────────────────────────────────────────────────

/// A minimal indexed program template with one numeric key + a name field.
fn prog(procedure: &str, path: &std::path::Path) -> String {
    format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. T.\n\
         \x20      ENVIRONMENT DIVISION.\n\
         \x20      INPUT-OUTPUT SECTION.\n\
         \x20      FILE-CONTROL.\n\
         \x20          SELECT F ASSIGN TO \"{path}\"\n\
         \x20              ORGANIZATION IS INDEXED\n\
         \x20              ACCESS MODE IS DYNAMIC\n\
         \x20              RECORD KEY IS R-ID\n\
         \x20              FILE STATUS IS FS.\n\
         \x20      DATA DIVISION.\n\
         \x20      FILE SECTION.\n\
         \x20      FD F.\n\
         \x20      01 R.\n\
         \x20         05 R-ID   PIC 9(4).\n\
         \x20         05 R-NAME PIC X(8).\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01 FS PIC XX.\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         {procedure}\n\
         \x20          STOP RUN.\n",
        path = path.display()
    )
}

/// A `STORAGE IS DISK [WITH COMPRESSION]` program with a primary key,
/// an alternate key WITH DUPLICATES, and a roomy record (so compression bites).
fn prog_disk(procedure: &str, path: &std::path::Path, compress: bool) -> String {
    let storage = if compress {
        "STORAGE IS DISK WITH COMPRESSION"
    } else {
        "STORAGE IS DISK"
    };
    format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. T.\n\
         \x20      ENVIRONMENT DIVISION.\n\
         \x20      INPUT-OUTPUT SECTION.\n\
         \x20      FILE-CONTROL.\n\
         \x20          SELECT CUSTOMER-FILE\n\
         \x20              {storage}\n\
         \x20              ASSIGN TO \"{path}\"\n\
         \x20              ORGANIZATION IS INDEXED\n\
         \x20              ACCESS MODE IS DYNAMIC\n\
         \x20              RECORD KEY IS CUSTOMER-ID\n\
         \x20              ALTERNATE RECORD KEY IS CUSTOMER-ZIP WITH DUPLICATES\n\
         \x20              FILE STATUS IS FS.\n\
         \x20      DATA DIVISION.\n\
         \x20      FILE SECTION.\n\
         \x20      FD CUSTOMER-FILE.\n\
         \x20      01 CUSTOMER-REC.\n\
         \x20         05 CUSTOMER-ID    PIC 9(5).\n\
         \x20         05 CUSTOMER-NAME  PIC X(40).\n\
         \x20         05 CUSTOMER-ZIP   PIC X(8).\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01 FS PIC XX.\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         {procedure}\n\
         \x20          STOP RUN.\n",
        storage = storage,
        path = path.display()
    )
}

#[test]
fn disk_mode_persists_writes_random_and_sequential() {
    // Full pipeline: parse STORAGE IS DISK, run on the paged B+tree engine,
    // then prove a fresh OPEN INPUT reads records back (random + ascending scan).
    let path = temp_idx("diskmode");
    let _ = std::fs::remove_file(&path);
    let out = run_capture(&prog_disk(
        "           OPEN OUTPUT CUSTOMER-FILE\n\
         \x20          MOVE 300 TO CUSTOMER-ID MOVE \"CAROL\" TO CUSTOMER-NAME\n\
         \x20          MOVE \"30000\" TO CUSTOMER-ZIP WRITE CUSTOMER-REC\n\
         \x20          MOVE 100 TO CUSTOMER-ID MOVE \"ALICE\" TO CUSTOMER-NAME\n\
         \x20          MOVE \"10000\" TO CUSTOMER-ZIP WRITE CUSTOMER-REC\n\
         \x20          MOVE 200 TO CUSTOMER-ID MOVE \"BOB\" TO CUSTOMER-NAME\n\
         \x20          MOVE \"20000\" TO CUSTOMER-ZIP WRITE CUSTOMER-REC\n\
         \x20          CLOSE CUSTOMER-FILE\n\
         \x20          OPEN INPUT CUSTOMER-FILE\n\
         \x20          MOVE 200 TO CUSTOMER-ID\n\
         \x20          READ CUSTOMER-FILE\n\
         \x20              INVALID KEY DISPLAY \"MISS\"\n\
         \x20              NOT INVALID KEY DISPLAY \"GOT \" CUSTOMER-NAME END-READ\n\
         \x20          MOVE 0 TO CUSTOMER-ID\n\
         \x20          START CUSTOMER-FILE KEY IS GREATER THAN CUSTOMER-ID END-START\n\
         \x20          READ CUSTOMER-FILE NEXT AT END CONTINUE END-READ\n\
         \x20          DISPLAY \"SEQ \" CUSTOMER-ID\n\
         \x20          READ CUSTOMER-FILE NEXT AT END CONTINUE END-READ\n\
         \x20          DISPLAY \"SEQ \" CUSTOMER-ID\n\
         \x20          READ CUSTOMER-FILE NEXT AT END CONTINUE END-READ\n\
         \x20          DISPLAY \"SEQ \" CUSTOMER-ID\n\
         \x20          CLOSE CUSTOMER-FILE",
        &path,
        false,
    ))
    .join("\n");
    let _ = std::fs::remove_file(&path);
    assert!(out.contains("GOT BOB"), "random read failed:\n{out}");
    // Ascending primary-key order, regardless of write order.
    let seqs: Vec<&str> = out.lines().filter(|l| l.starts_with("SEQ ")).collect();
    assert_eq!(seqs, ["SEQ 00100", "SEQ 00200", "SEQ 00300"], "scan order:\n{out}");
}

#[test]
fn disk_mode_with_data_compressing_round_trips() {
    // COMPRESSION on the disk backend: write padded records, reopen, read.
    let path = temp_idx("diskzip");
    let _ = std::fs::remove_file(&path);
    let out = run_capture(&prog_disk(
        "           OPEN OUTPUT CUSTOMER-FILE\n\
         \x20          MOVE 4242 TO CUSTOMER-ID MOVE \"ZIGGY\" TO CUSTOMER-NAME\n\
         \x20          MOVE \"99999\" TO CUSTOMER-ZIP WRITE CUSTOMER-REC\n\
         \x20          CLOSE CUSTOMER-FILE\n\
         \x20          OPEN INPUT CUSTOMER-FILE\n\
         \x20          MOVE 4242 TO CUSTOMER-ID\n\
         \x20          READ CUSTOMER-FILE\n\
         \x20              INVALID KEY DISPLAY \"MISS\"\n\
         \x20              NOT INVALID KEY DISPLAY \"GOT \" CUSTOMER-NAME END-READ\n\
         \x20          CLOSE CUSTOMER-FILE",
        &path,
        true,
    ))
    .join("\n");
    let _ = std::fs::remove_file(&path);
    assert!(out.contains("GOT ZIGGY"), "compressed disk round-trip failed:\n{out}");
}

#[test]
fn records_persist_across_close_and_reopen() {
    // Write in one OPEN session, then prove a *fresh* OPEN INPUT reads them back
    // — i.e. CLOSE flushed the container to disk.
    let path = temp_idx("persist");
    let _ = std::fs::remove_file(&path);
    let out = run_capture(&prog(
        "           OPEN OUTPUT F\n\
         \x20          MOVE 0009 TO R-ID MOVE \"NINE\" TO R-NAME WRITE R\n\
         \x20          CLOSE F\n\
         \x20          OPEN INPUT F\n\
         \x20          MOVE 0009 TO R-ID\n\
         \x20          READ F INVALID KEY DISPLAY \"MISS\"\n\
         \x20              NOT INVALID KEY DISPLAY \"GOT \" R-NAME END-READ\n\
         \x20          CLOSE F",
        &path,
    ))
    .join("\n");
    let _ = std::fs::remove_file(&path);
    assert!(out.contains("GOT NINE"), "record did not persist:\n{out}");
}

#[test]
fn start_then_sequential_reads_in_key_order() {
    // Records written out of order must come back ascending after START.
    let path = temp_idx("order");
    let _ = std::fs::remove_file(&path);
    let out = run_capture(&prog(
        "           OPEN OUTPUT F\n\
         \x20          MOVE 0030 TO R-ID MOVE \"C\" TO R-NAME WRITE R\n\
         \x20          MOVE 0010 TO R-ID MOVE \"A\" TO R-NAME WRITE R\n\
         \x20          MOVE 0020 TO R-ID MOVE \"B\" TO R-NAME WRITE R\n\
         \x20          CLOSE F\n\
         \x20          OPEN INPUT F\n\
         \x20          MOVE 0 TO R-ID\n\
         \x20          START F KEY IS GREATER THAN R-ID END-START\n\
         \x20          READ F NEXT AT END CONTINUE END-READ\n\
         \x20          DISPLAY \"ROW \" R-ID\n\
         \x20          READ F NEXT AT END CONTINUE END-READ\n\
         \x20          DISPLAY \"ROW \" R-ID\n\
         \x20          READ F NEXT AT END CONTINUE END-READ\n\
         \x20          DISPLAY \"ROW \" R-ID\n\
         \x20          CLOSE F",
        &path,
    ));
    let _ = std::fs::remove_file(&path);
    let joined = out.join("\n");
    let rows: Vec<&String> = out.iter().filter(|l| l.starts_with("ROW ")).collect();
    assert_eq!(rows.len(), 3, "expected 3 rows:\n{joined}");
    assert!(rows[0].contains("0010"), "row1 not 0010:\n{joined}");
    assert!(rows[1].contains("0020"), "row2 not 0020:\n{joined}");
    assert!(rows[2].contains("0030"), "row3 not 0030:\n{joined}");
}
