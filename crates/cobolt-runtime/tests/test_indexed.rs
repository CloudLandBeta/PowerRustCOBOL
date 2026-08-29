// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Focused integration tests for INDEXED (ISAM) files — small programs built
//! inline that pin specific behaviours (MEMORY + DISK storage, compression,
//! persistence across CLOSE/OPEN, START + sequential ordering).
//!
//! The comprehensive end-to-end indexed coverage lives in the File I/O suite
//! (`tests/cobol/fileio/`), driven by `test_fileio_storage.rs`.

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
    display_rx.try_iter().collect()
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
    assert_eq!(
        seqs,
        ["SEQ 00100", "SEQ 00200", "SEQ 00300"],
        "scan order:\n{out}"
    );
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
    assert!(
        out.contains("GOT ZIGGY"),
        "compressed disk round-trip failed:\n{out}"
    );
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

/// A program whose indexed file is `ACCESS MODE IS SEQUENTIAL`.
///
/// The sequential access mode is what puts a file under the ordering and
/// record-establishment rules the tests below exercise; `prog` above uses
/// DYNAMIC, where none of them apply.
fn prog_sequential(procedure: &str, path: &std::path::Path) -> String {
    format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. T.\n\
         \x20      ENVIRONMENT DIVISION.\n\
         \x20      INPUT-OUTPUT SECTION.\n\
         \x20      FILE-CONTROL.\n\
         \x20          SELECT F ASSIGN TO \"{path}\"\n\
         \x20              ORGANIZATION IS INDEXED\n\
         \x20              ACCESS MODE IS SEQUENTIAL\n\
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

/// Writing an indexed file sequentially requires ascending key order.
///
/// A key that is not greater than the one before it is status 21 and the
/// record is not written — the rule IX109A checks by writing 1…50 and then 49.
#[test]
fn sequential_write_out_of_key_order_is_21() {
    let path = temp_idx("seqwrite21");
    let _ = std::fs::remove_file(&path);
    let out = run_capture(&prog_sequential(
        "           OPEN OUTPUT F\n\
         \x20          MOVE 10 TO R-ID MOVE \"A\" TO R-NAME\n\
         \x20          WRITE R END-WRITE DISPLAY \"W1 \" FS\n\
         \x20          MOVE 20 TO R-ID MOVE \"B\" TO R-NAME\n\
         \x20          WRITE R END-WRITE DISPLAY \"W2 \" FS\n\
         \x20          MOVE 15 TO R-ID MOVE \"C\" TO R-NAME\n\
         \x20          WRITE R END-WRITE DISPLAY \"W3 \" FS\n\
         \x20          MOVE 20 TO R-ID MOVE \"D\" TO R-NAME\n\
         \x20          WRITE R END-WRITE DISPLAY \"W4 \" FS\n\
         \x20          MOVE 30 TO R-ID MOVE \"E\" TO R-NAME\n\
         \x20          WRITE R END-WRITE DISPLAY \"W5 \" FS\n\
         \x20          CLOSE F",
        &path,
    ));
    let joined = out.join("\n");
    assert!(joined.contains("W1 00"), "ascending first write:\n{joined}");
    assert!(joined.contains("W2 00"), "ascending second write:\n{joined}");
    assert!(
        joined.contains("W3 21"),
        "15 after 20 is out of sequence:\n{joined}"
    );
    assert!(
        joined.contains("W4 21"),
        "a key equal to the last is not greater:\n{joined}"
    );
    assert!(
        joined.contains("W5 00"),
        "a rejected write must not move the sequence forward, so 30 still \
         follows 20:\n{joined}"
    );

    // The two rejected records were not written.
    let out = run_capture(&prog_sequential(
        "           OPEN INPUT F\n\
         \x20          PERFORM 4 TIMES\n\
         \x20             READ F NEXT AT END EXIT PERFORM END-READ\n\
         \x20             DISPLAY \"ROW \" R-ID \" \" R-NAME\n\
         \x20          END-PERFORM\n\
         \x20          CLOSE F",
        &path,
    ));
    let _ = std::fs::remove_file(&path);
    let rows: Vec<&String> = out.iter().filter(|l| l.starts_with("ROW ")).collect();
    let joined = out.join("\n");
    assert_eq!(rows.len(), 3, "only the three accepted records:\n{joined}");
    assert!(rows[0].contains("0010"), "{joined}");
    assert!(rows[1].contains("0020"), "{joined}");
    assert!(rows[2].contains("0030"), "{joined}");
    // The duplicate 20 was rejected, so B — not D — is still there.
    assert!(rows[1].contains('B'), "record 20 must be unchanged:\n{joined}");
}

/// Under RANDOM or DYNAMIC access any write order is allowed.
///
/// The ordering rule belongs to the sequential access mode alone; a descending
/// write under DYNAMIC is a normal 00, and only a clash with an existing record
/// is an error (22).
#[test]
fn dynamic_write_ignores_key_order_but_still_rejects_duplicates() {
    let path = temp_idx("dynwriteorder");
    let _ = std::fs::remove_file(&path);
    let out = run_capture(&prog(
        "           OPEN OUTPUT F\n\
         \x20          MOVE 30 TO R-ID MOVE \"A\" TO R-NAME\n\
         \x20          WRITE R END-WRITE DISPLAY \"W1 \" FS\n\
         \x20          MOVE 10 TO R-ID MOVE \"B\" TO R-NAME\n\
         \x20          WRITE R END-WRITE DISPLAY \"W2 \" FS\n\
         \x20          MOVE 30 TO R-ID MOVE \"C\" TO R-NAME\n\
         \x20          WRITE R END-WRITE DISPLAY \"W3 \" FS\n\
         \x20          CLOSE F",
        &path,
    ));
    let _ = std::fs::remove_file(&path);
    let joined = out.join("\n");
    assert!(joined.contains("W1 00"), "{joined}");
    assert!(
        joined.contains("W2 00"),
        "descending is fine under DYNAMIC:\n{joined}"
    );
    assert!(
        joined.contains("W3 22"),
        "a duplicate primary key is 22, not 21:\n{joined}"
    );
}

/// A sequential REWRITE replaces the record the previous READ delivered, so
/// there must be one: without it the status is 43 (IX120A).
#[test]
fn sequential_rewrite_without_a_preceding_read_is_43() {
    let path = temp_idx("seqrewrite43");
    let _ = std::fs::remove_file(&path);
    run_capture(&prog_sequential(
        "           OPEN OUTPUT F\n\
         \x20          MOVE 10 TO R-ID MOVE \"A\" TO R-NAME\n\
         \x20          WRITE R END-WRITE\n\
         \x20          MOVE 20 TO R-ID MOVE \"B\" TO R-NAME\n\
         \x20          WRITE R END-WRITE\n\
         \x20          CLOSE F",
        &path,
    ));
    let out = run_capture(&prog_sequential(
        "           OPEN I-O F\n\
         \x20          MOVE 10 TO R-ID MOVE \"Z\" TO R-NAME\n\
         \x20          REWRITE R END-REWRITE DISPLAY \"NOREAD \" FS\n\
         \x20          READ F NEXT AT END CONTINUE END-READ\n\
         \x20          MOVE \"Y\" TO R-NAME\n\
         \x20          REWRITE R END-REWRITE DISPLAY \"AFTERREAD \" FS\n\
         \x20          REWRITE R END-REWRITE DISPLAY \"TWICE \" FS\n\
         \x20          CLOSE F",
        &path,
    ));
    let _ = std::fs::remove_file(&path);
    let joined = out.join("\n");
    assert!(
        joined.contains("NOREAD 43"),
        "a REWRITE with no READ before it has no record to replace:\n{joined}"
    );
    assert!(
        joined.contains("AFTERREAD 00"),
        "a REWRITE right after a successful READ is allowed:\n{joined}"
    );
    assert!(
        joined.contains("TWICE 43"),
        "the REWRITE consumes the record, so a second one is 43 again:\n{joined}"
    );
}

/// The same rule governs a sequential DELETE (IX119A).
#[test]
fn sequential_delete_without_a_preceding_read_is_43() {
    let path = temp_idx("seqdelete43");
    let _ = std::fs::remove_file(&path);
    run_capture(&prog_sequential(
        "           OPEN OUTPUT F\n\
         \x20          MOVE 10 TO R-ID MOVE \"A\" TO R-NAME\n\
         \x20          WRITE R END-WRITE\n\
         \x20          MOVE 20 TO R-ID MOVE \"B\" TO R-NAME\n\
         \x20          WRITE R END-WRITE\n\
         \x20          CLOSE F",
        &path,
    ));
    let out = run_capture(&prog_sequential(
        "           OPEN I-O F\n\
         \x20          DELETE F END-DELETE DISPLAY \"NOREAD \" FS\n\
         \x20          READ F NEXT AT END CONTINUE END-READ\n\
         \x20          DELETE F END-DELETE DISPLAY \"AFTERREAD \" FS\n\
         \x20          DELETE F END-DELETE DISPLAY \"TWICE \" FS\n\
         \x20          CLOSE F",
        &path,
    ));
    let _ = std::fs::remove_file(&path);
    let joined = out.join("\n");
    assert!(
        joined.contains("NOREAD 43"),
        "a DELETE with no READ before it has no record to remove:\n{joined}"
    );
    assert!(
        joined.contains("AFTERREAD 00"),
        "a DELETE right after a successful READ is allowed:\n{joined}"
    );
    assert!(
        joined.contains("TWICE 43"),
        "the DELETE consumes the record, so a second one is 43 again:\n{joined}"
    );
}

/// A START positions the file but delivers no record, so a REWRITE after it is
/// still 43 — the record has to come from a READ.
#[test]
fn start_does_not_establish_a_record_for_rewrite() {
    let path = temp_idx("startnorewrite");
    let _ = std::fs::remove_file(&path);
    run_capture(&prog_sequential(
        "           OPEN OUTPUT F\n\
         \x20          MOVE 10 TO R-ID MOVE \"A\" TO R-NAME\n\
         \x20          WRITE R END-WRITE\n\
         \x20          MOVE 20 TO R-ID MOVE \"B\" TO R-NAME\n\
         \x20          WRITE R END-WRITE\n\
         \x20          CLOSE F",
        &path,
    ));
    let out = run_capture(&prog_sequential(
        "           OPEN I-O F\n\
         \x20          MOVE 10 TO R-ID\n\
         \x20          START F KEY IS EQUAL TO R-ID END-START\n\
         \x20          DISPLAY \"START \" FS\n\
         \x20          MOVE \"Z\" TO R-NAME\n\
         \x20          REWRITE R END-REWRITE DISPLAY \"AFTERSTART \" FS\n\
         \x20          CLOSE F",
        &path,
    ));
    let _ = std::fs::remove_file(&path);
    let joined = out.join("\n");
    assert!(joined.contains("START 00"), "the START itself works:\n{joined}");
    assert!(
        joined.contains("AFTERSTART 43"),
        "START positions the file but establishes no record:\n{joined}"
    );
}

/// Three keys of one file may share a data-name and be told apart only by the
/// group each sits in.
///
/// IX215A declares `IX-FD3`'s prime key and both alternates as `IX-FD3-KEY`,
/// qualified into three different areas. Resolving on the bare name gave all
/// three the first field's offset, so the file had three indexes over one set
/// of bytes and a read by an alternate returned the wrong record.
#[test]
fn same_named_keys_are_told_apart_by_their_qualifier() {
    let path = temp_idx("qualkeys");
    let _ = std::fs::remove_file(&path);
    let src = format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. T.\n\
         \x20      ENVIRONMENT DIVISION.\n\
         \x20      INPUT-OUTPUT SECTION.\n\
         \x20      FILE-CONTROL.\n\
         \x20          SELECT F ASSIGN TO \"{path}\"\n\
         \x20              ORGANIZATION IS INDEXED\n\
         \x20              ACCESS MODE IS DYNAMIC\n\
         \x20              RECORD KEY IS R-KEY IN PRIME-AREA\n\
         \x20              ALTERNATE RECORD KEY IS R-KEY OF ALT1-AREA\n\
         \x20              FILE STATUS IS FS.\n\
         \x20      DATA DIVISION.\n\
         \x20      FILE SECTION.\n\
         \x20      FD F.\n\
         \x20      01 R.\n\
         \x20         05 PRIME-AREA.\n\
         \x20            10 R-KEY  PIC X(4).\n\
         \x20         05 ALT1-AREA.\n\
         \x20            10 R-KEY  PIC X(4).\n\
         \x20         05 R-NAME    PIC X(8).\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01 FS PIC XX.\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         \x20          OPEN OUTPUT F\n\
         \x20          MOVE \"P001\" TO R-KEY IN PRIME-AREA\n\
         \x20          MOVE \"A900\" TO R-KEY IN ALT1-AREA\n\
         \x20          MOVE \"FIRST\" TO R-NAME\n\
         \x20          WRITE R END-WRITE DISPLAY \"W1 \" FS\n\
         \x20          MOVE \"P002\" TO R-KEY IN PRIME-AREA\n\
         \x20          MOVE \"A800\" TO R-KEY IN ALT1-AREA\n\
         \x20          MOVE \"SECOND\" TO R-NAME\n\
         \x20          WRITE R END-WRITE DISPLAY \"W2 \" FS\n\
         \x20          CLOSE F\n\
         \x20          OPEN INPUT F\n\
         \x20          MOVE SPACES TO R-NAME\n\
         \x20          MOVE \"A800\" TO R-KEY IN ALT1-AREA\n\
         \x20          READ F KEY IS R-KEY IN ALT1-AREA\n\
         \x20             INVALID KEY DISPLAY \"ALTREAD BAD \" FS\n\
         \x20             NOT INVALID KEY DISPLAY \"ALTREAD \" R-NAME\n\
         \x20          END-READ\n\
         \x20          MOVE SPACES TO R-NAME\n\
         \x20          MOVE \"P001\" TO R-KEY IN PRIME-AREA\n\
         \x20          READ F KEY IS R-KEY IN PRIME-AREA\n\
         \x20             INVALID KEY DISPLAY \"PRIMEREAD BAD \" FS\n\
         \x20             NOT INVALID KEY DISPLAY \"PRIMEREAD \" R-NAME\n\
         \x20          END-READ\n\
         \x20          CLOSE F\n\
         \x20          STOP RUN.\n",
        path = path.display()
    );
    let out = run_capture(&src);
    let _ = std::fs::remove_file(&path);
    let joined = out.join("\n");
    assert!(joined.contains("W1 00"), "first write:\n{joined}");
    assert!(joined.contains("W2 00"), "second write:\n{joined}");
    // A800 is the SECOND record's alternate key. Reading by the alternate must
    // deliver that record — with the bare-name bug both keys indexed
    // PRIME-AREA, so A800 matched nothing.
    assert!(
        joined.contains("ALTREAD SECOND"),
        "the alternate key must index ALT1-AREA, not the prime area:\n{joined}"
    );
    assert!(
        joined.contains("PRIMEREAD FIRST"),
        "the prime key must still index PRIME-AREA:\n{joined}"
    );
}
