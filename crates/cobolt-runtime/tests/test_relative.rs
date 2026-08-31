// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Integration tests for `ORGANIZATION IS RELATIVE`, driven through whole COBOL
//! programs rather than the engine's own API.
//!
//! The engine's unit tests (`cobolt-runtime/src/relative.rs`) pin the container
//! behaviour. What is tested here is everything that lives *between* the
//! program and the engine: which access mode makes a verb random, where the
//! record number comes from and goes back to, and the file statuses that depend
//! on how the RELATIVE KEY item was declared.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// A unique temp path so parallel test runs never share a container.
fn temp_rel(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("prc-reltest-{tag}-{nanos}.rel"))
}

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
    display_rx.try_iter().collect()
}

/// A relative-file program: `access` and the RELATIVE KEY item's PICTURE are
/// the two things these tests vary.
fn prog(path: &std::path::Path, access: &str, key_pic: &str, procedure: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. T.\n\
         ENVIRONMENT DIVISION.\n\
         INPUT-OUTPUT SECTION.\n\
         FILE-CONTROL.\n\
         \x20   SELECT F ASSIGN TO \"{path}\"\n\
         \x20       ORGANIZATION IS RELATIVE\n\
         \x20       ACCESS MODE IS {access}\n\
         \x20       RELATIVE KEY IS RK\n\
         \x20       FILE STATUS IS FS.\n\
         DATA DIVISION.\n\
         FILE SECTION.\n\
         FD F.\n\
         01 R.\n\
         \x20  05 R-TEXT PIC X(8).\n\
         WORKING-STORAGE SECTION.\n\
         01 FS PIC XX.\n\
         01 RK PIC {key_pic}.\n\
         01 WS-I PIC 9(4).\n\
         PROCEDURE DIVISION.\n\
         MAIN.\n\
         {procedure}\n\
         \x20   STOP RUN.\n",
        path = path.display()
    )
}

/// A sequential `WRITE` numbers the records itself and reports each number in
/// the RELATIVE KEY item. Without that, a program creating a file has no way to
/// learn where its own records went.
#[test]
fn sequential_write_reports_the_slot_it_assigned() {
    let p = temp_rel("seqwrite");
    let out = run_capture(&prog(
        &p,
        "SEQUENTIAL",
        "9(4)",
        "    OPEN OUTPUT F.\n\
         \x20   PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 3\n\
         \x20       MOVE \"REC\" TO R-TEXT\n\
         \x20       WRITE R\n\
         \x20       DISPLAY \"W \" FS \" \" RK\n\
         \x20   END-PERFORM.\n\
         \x20   CLOSE F.",
    ));
    assert_eq!(out, vec!["W 00 0001", "W 00 0002", "W 00 0003"]);
    let _ = std::fs::remove_file(&p);
}

/// Under RANDOM access the program chooses the slot. Writing onto one that
/// already holds a record is 22; slot zero is a boundary violation, 24.
#[test]
fn random_write_reports_22_on_an_occupied_slot_and_24_on_slot_zero() {
    let p = temp_rel("random");
    let out = run_capture(&prog(
        &p,
        "RANDOM",
        "9(4)",
        "    OPEN OUTPUT F.\n\
         \x20   MOVE 7 TO RK. MOVE \"SEVEN\" TO R-TEXT. WRITE R. DISPLAY \"A \" FS.\n\
         \x20   MOVE 7 TO RK. MOVE \"AGAIN\" TO R-TEXT. WRITE R. DISPLAY \"B \" FS.\n\
         \x20   MOVE 0 TO RK. MOVE \"ZERO\"  TO R-TEXT. WRITE R. DISPLAY \"C \" FS.\n\
         \x20   CLOSE F.",
    ));
    assert_eq!(out, vec!["A 00", "B 22", "C 24"]);
    let _ = std::fs::remove_file(&p);
}

/// A gap left by a random write is part of the file: reading it is 23, and a
/// sequential walk steps over it rather than renumbering anything.
#[test]
fn a_gap_reads_as_not_found_and_a_walk_skips_it() {
    let p = temp_rel("gap");
    run_capture(&prog(
        &p,
        "RANDOM",
        "9(4)",
        "    OPEN OUTPUT F.\n\
         \x20   MOVE 1 TO RK. MOVE \"ONE\"  TO R-TEXT. WRITE R.\n\
         \x20   MOVE 4 TO RK. MOVE \"FOUR\" TO R-TEXT. WRITE R.\n\
         \x20   CLOSE F.",
    ));
    let out = run_capture(&prog(
        &p,
        "DYNAMIC",
        "9(4)",
        "    OPEN INPUT F.\n\
         \x20   MOVE 2 TO RK. READ F. DISPLAY \"GAP \" FS.\n\
         \x20   READ F NEXT RECORD AT END CONTINUE END-READ.\n\
         \x20   DISPLAY \"N1 \" FS \" \" RK \" \" R-TEXT.\n\
         \x20   READ F NEXT RECORD AT END CONTINUE END-READ.\n\
         \x20   DISPLAY \"N2 \" FS \" \" RK \" \" R-TEXT.\n\
         \x20   READ F NEXT RECORD AT END DISPLAY \"EOF \" FS END-READ.\n\
         \x20   CLOSE F.",
    ));
    assert_eq!(
        out,
        vec![
            "GAP 23",
            "N1 00 0001 ONE     ",
            "N2 00 0004 FOUR    ",
            "EOF 10",
        ]
    );
    let _ = std::fs::remove_file(&p);
}

/// `START` positions without delivering, so the next `READ NEXT` returns the
/// record it found.
#[test]
fn start_positions_for_the_next_sequential_read() {
    let p = temp_rel("start");
    run_capture(&prog(
        &p,
        "SEQUENTIAL",
        "9(4)",
        "    OPEN OUTPUT F.\n\
         \x20   PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 6\n\
         \x20       MOVE \"X\" TO R-TEXT WRITE R\n\
         \x20   END-PERFORM.\n\
         \x20   CLOSE F.",
    ));
    let out = run_capture(&prog(
        &p,
        "DYNAMIC",
        "9(4)",
        "    OPEN INPUT F.\n\
         \x20   MOVE 4 TO RK.\n\
         \x20   START F KEY IS NOT LESS THAN RK\n\
         \x20       INVALID KEY DISPLAY \"BAD\" END-START.\n\
         \x20   DISPLAY \"S \" FS.\n\
         \x20   READ F NEXT RECORD AT END CONTINUE END-READ.\n\
         \x20   DISPLAY \"R \" FS \" \" RK.\n\
         \x20   MOVE 99 TO RK.\n\
         \x20   START F KEY IS EQUAL TO RK\n\
         \x20       INVALID KEY DISPLAY \"MISS \" FS END-START.\n\
         \x20   CLOSE F.",
    ));
    assert_eq!(out, vec!["S 00", "R 00 0004", "MISS 23"]);
    let _ = std::fs::remove_file(&p);
}

/// `DELETE` empties the slot and leaves its number addressable; the records
/// after it do not move down.
#[test]
fn delete_empties_the_slot_without_renumbering() {
    let p = temp_rel("delete");
    run_capture(&prog(
        &p,
        "SEQUENTIAL",
        "9(4)",
        "    OPEN OUTPUT F.\n\
         \x20   PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 3\n\
         \x20       MOVE \"X\" TO R-TEXT WRITE R\n\
         \x20   END-PERFORM.\n\
         \x20   CLOSE F.",
    ));
    let out = run_capture(&prog(
        &p,
        "RANDOM",
        "9(4)",
        "    OPEN I-O F.\n\
         \x20   MOVE 2 TO RK. DELETE F RECORD. DISPLAY \"D \" FS.\n\
         \x20   MOVE 2 TO RK. READ F. DISPLAY \"G \" FS.\n\
         \x20   MOVE 3 TO RK. READ F. DISPLAY \"H \" FS.\n\
         \x20   MOVE 2 TO RK. DELETE F RECORD. DISPLAY \"E \" FS.\n\
         \x20   CLOSE F.",
    ));
    assert_eq!(out, vec!["D 00", "G 23", "H 00", "E 23"]);
    let _ = std::fs::remove_file(&p);
}

/// A sequential `REWRITE` replaces the record the last `READ` delivered, and
/// with no such read there is nothing to replace: **43**.
#[test]
fn sequential_rewrite_needs_a_read_before_it() {
    let p = temp_rel("rewrite");
    run_capture(&prog(
        &p,
        "SEQUENTIAL",
        "9(4)",
        "    OPEN OUTPUT F.\n\
         \x20   MOVE \"FIRST\" TO R-TEXT. WRITE R.\n\
         \x20   CLOSE F.",
    ));
    let out = run_capture(&prog(
        &p,
        "SEQUENTIAL",
        "9(4)",
        "    OPEN I-O F.\n\
         \x20   MOVE \"NOREAD\" TO R-TEXT. REWRITE R. DISPLAY \"A \" FS.\n\
         \x20   READ F AT END CONTINUE END-READ.\n\
         \x20   MOVE \"SECOND\" TO R-TEXT. REWRITE R. DISPLAY \"B \" FS.\n\
         \x20   CLOSE F.\n\
         \x20   OPEN INPUT F.\n\
         \x20   READ F AT END CONTINUE END-READ.\n\
         \x20   DISPLAY \"C \" R-TEXT.\n\
         \x20   CLOSE F.",
    ));
    assert_eq!(out, vec!["A 43", "B 00", "C SECOND  "]);
    let _ = std::fs::remove_file(&p);
}

/// **Status 14** — a sequential READ whose relative record number needs more
/// digits than the RELATIVE KEY item has.
///
/// The width of that PICTURE is part of the file's behaviour, not just its
/// storage. With `PIC 99` the tenth record still fits and the hundredth does
/// not, so the read is unsuccessful and the `AT END` phrase handles it.
#[test]
fn a_record_number_too_big_for_the_key_item_is_14() {
    let p = temp_rel("status14");
    run_capture(&prog(
        &p,
        "SEQUENTIAL",
        "9(4)",
        "    OPEN OUTPUT F.\n\
         \x20   PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 100\n\
         \x20       MOVE \"X\" TO R-TEXT WRITE R\n\
         \x20   END-PERFORM.\n\
         \x20   CLOSE F.",
    ));
    // The same file read back through a two-digit key item.
    let out = run_capture(&prog(
        &p,
        "SEQUENTIAL",
        "99",
        "    OPEN INPUT F.\n\
         \x20   PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 100\n\
         \x20       READ F AT END DISPLAY \"STOP \" FS \" AT \" WS-I END-READ\n\
         \x20   END-PERFORM.\n\
         \x20   CLOSE F.",
    ));
    assert_eq!(out, vec!["STOP 14 AT 0100"]);
    let _ = std::fs::remove_file(&p);
}

/// A key item wide enough for every record number never raises 14.
#[test]
fn a_wide_enough_key_item_reads_the_whole_file() {
    let p = temp_rel("nostatus14");
    run_capture(&prog(
        &p,
        "SEQUENTIAL",
        "9(4)",
        "    OPEN OUTPUT F.\n\
         \x20   PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 100\n\
         \x20       MOVE \"X\" TO R-TEXT WRITE R\n\
         \x20   END-PERFORM.\n\
         \x20   CLOSE F.",
    ));
    let out = run_capture(&prog(
        &p,
        "SEQUENTIAL",
        "9(4)",
        "    OPEN INPUT F.\n\
         \x20   PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 101\n\
         \x20       READ F AT END DISPLAY \"STOP \" FS \" AT \" WS-I END-READ\n\
         \x20   END-PERFORM.\n\
         \x20   CLOSE F.",
    ));
    assert_eq!(out, vec!["STOP 10 AT 0101"]);
    let _ = std::fs::remove_file(&p);
}

/// A verb used against the wrong open mode reports it rather than misbehaving.
#[test]
fn the_open_mode_a_verb_needs_is_reported() {
    let p = temp_rel("modes");
    run_capture(&prog(
        &p,
        "RANDOM",
        "9(4)",
        "    OPEN OUTPUT F.\n\
         \x20   MOVE 1 TO RK. MOVE \"ONE\" TO R-TEXT. WRITE R.\n\
         \x20   CLOSE F.",
    ));
    let out = run_capture(&prog(
        &p,
        "RANDOM",
        "9(4)",
        "    OPEN INPUT F.\n\
         \x20   MOVE 2 TO RK. MOVE \"TWO\" TO R-TEXT. WRITE R. DISPLAY \"W \" FS.\n\
         \x20   MOVE 1 TO RK. REWRITE R. DISPLAY \"R \" FS.\n\
         \x20   MOVE 1 TO RK. DELETE F RECORD. DISPLAY \"D \" FS.\n\
         \x20   CLOSE F.",
    ));
    assert_eq!(out, vec!["W 48", "R 49", "D 49"]);
    let _ = std::fs::remove_file(&p);
}

/// `OPEN INPUT` of a file that is not there is 35, and the file stays absent.
#[test]
fn opening_a_missing_file_for_input_is_35() {
    let p = temp_rel("missing");
    let out = run_capture(&prog(
        &p,
        "SEQUENTIAL",
        "9(4)",
        "    OPEN INPUT F. DISPLAY \"O \" FS.",
    ));
    assert_eq!(out, vec!["O 35"]);
    assert!(!p.exists(), "OPEN INPUT must not create the file");
}
