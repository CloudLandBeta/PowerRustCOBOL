// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Behaviour tests for two constructs that were previously only *parse*-tested.
//!
//! `specs/nist/NIST-tasks-statement-grammar-gaps.md` recorded AC5 and AC6 as
//! "parses, behaviour not asserted" — which this project's own rule says is not
//! done. These are the missing assertions:
//!
//! * **AC5** — `WHEN -0.000020 THRU 0.000020` must *match* the right values,
//!   not merely parse. A signed range that parsed but compared wrongly would
//!   pass a parse test and still give the wrong answer.
//! * **AC6** — `CLOSE … WITH LOCK` must actually prevent a reopen, reporting
//!   file status 38. A phrase that parses and does nothing is decoration.

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

/// AC5 — a signed `THRU` range selects on VALUE, at both ends and outside.
#[test]
fn ac5_a_signed_when_range_matches_by_value() {
    // -0.000020 THRU 0.000020, tested with values inside, on each boundary and
    // outside on each side.
    for (value, expect) in [
        ("-0.000010", "IN"),
        ("0.000010", "IN"),
        ("0", "IN"),
        ("-0.000020", "IN"),  // lower boundary is inclusive
        ("0.000020", "IN"),   // upper boundary is inclusive
        ("-0.000100", "OUT"), // below
        ("0.000100", "OUT"),  // above
    ] {
        let src = format!(
            "       IDENTIFICATION DIVISION.\n\
             \x20      PROGRAM-ID. WHENRANGE.\n\
             \x20      DATA DIVISION.\n\
             \x20      WORKING-STORAGE SECTION.\n\
             \x20      01  WS-V PIC S9(1)V9(6) VALUE {value}.\n\
             \x20      PROCEDURE DIVISION.\n\
             \x20      MAIN.\n\
             \x20          EVALUATE WS-V\n\
             \x20              WHEN -0.000020 THRU 0.000020 DISPLAY \"IN\"\n\
             \x20              WHEN OTHER DISPLAY \"OUT\"\n\
             \x20          END-EVALUATE.\n\
             \x20          STOP RUN.\n"
        );
        let out = run_capture(&src);
        assert_eq!(
            out.first().map(String::as_str),
            Some(expect),
            "value {value} should be {expect}, got {out:?}"
        );
    }
}

/// AC6 — `CLOSE … WITH LOCK` prevents a reopen in the same run unit, and the
/// standard's file status for that is **38**.
#[test]
fn ac6_close_with_lock_refuses_a_reopen_with_status_38() {
    let dir = std::env::temp_dir().join("rcbl-close-lock-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("LOCKED.DAT");
    let path_s = path.to_string_lossy().replace('\\', "/");

    let src = format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. LOCKTEST.\n\
         \x20      ENVIRONMENT DIVISION.\n\
         \x20      INPUT-OUTPUT SECTION.\n\
         \x20      FILE-CONTROL.\n\
         \x20          SELECT OUTF ASSIGN TO \"{path_s}\"\n\
         \x20              ORGANIZATION IS LINE SEQUENTIAL\n\
         \x20              FILE STATUS IS WS-FS.\n\
         \x20      DATA DIVISION.\n\
         \x20      FILE SECTION.\n\
         \x20      FD  OUTF.\n\
         \x20      01  OUT-REC PIC X(10).\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01  WS-FS PIC XX.\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         \x20          OPEN OUTPUT OUTF.\n\
         \x20          DISPLAY \"OPEN1=\" WS-FS.\n\
         \x20          MOVE \"HELLO\" TO OUT-REC.\n\
         \x20          WRITE OUT-REC.\n\
         \x20          CLOSE OUTF WITH LOCK.\n\
         \x20          DISPLAY \"CLOSE=\" WS-FS.\n\
         \x20          OPEN INPUT OUTF.\n\
         \x20          DISPLAY \"OPEN2=\" WS-FS.\n\
         \x20          STOP RUN.\n"
    );
    let out = run_capture(&src);
    let joined = out.join("|");
    assert!(joined.contains("OPEN1=00"), "first OPEN should succeed: {out:?}");
    assert!(
        joined.contains("OPEN2=38"),
        "reopening a file closed WITH LOCK must report file status 38: {out:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A plain CLOSE locks nothing — the file reopens normally. Without this, the
/// test above would pass even if every CLOSE locked the file.
#[test]
fn a_plain_close_does_not_lock_the_file() {
    let dir = std::env::temp_dir().join("rcbl-plain-close-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("OPEN.DAT");
    let path_s = path.to_string_lossy().replace('\\', "/");

    let src = format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. NOLOCK.\n\
         \x20      ENVIRONMENT DIVISION.\n\
         \x20      INPUT-OUTPUT SECTION.\n\
         \x20      FILE-CONTROL.\n\
         \x20          SELECT OUTF ASSIGN TO \"{path_s}\"\n\
         \x20              ORGANIZATION IS LINE SEQUENTIAL\n\
         \x20              FILE STATUS IS WS-FS.\n\
         \x20      DATA DIVISION.\n\
         \x20      FILE SECTION.\n\
         \x20      FD  OUTF.\n\
         \x20      01  OUT-REC PIC X(10).\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01  WS-FS PIC XX.\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         \x20          OPEN OUTPUT OUTF.\n\
         \x20          MOVE \"HELLO\" TO OUT-REC.\n\
         \x20          WRITE OUT-REC.\n\
         \x20          CLOSE OUTF.\n\
         \x20          OPEN INPUT OUTF.\n\
         \x20          DISPLAY \"OPEN2=\" WS-FS.\n\
         \x20          STOP RUN.\n"
    );
    let out = run_capture(&src);
    assert!(
        out.join("|").contains("OPEN2=00"),
        "a plain CLOSE must not lock the file: {out:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
