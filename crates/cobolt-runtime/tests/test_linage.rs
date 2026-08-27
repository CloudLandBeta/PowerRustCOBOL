// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `LINAGE` — the printed page, and `AT END-OF-PAGE`.
//!
//! COBOL-85 divides a LINAGE file's page into a top margin, a body of `n`
//! lines, and a bottom margin. `LINAGE-COUNTER` counts lines written into the
//! body; reaching `FOOTING` raises the end-of-page condition, which is how a
//! report knows to print its page trailer.
//!
//! The clause used to be skipped wholesale by the FD parser and
//! `AT END-OF-PAGE` was a parse error, so the SQ module's report tests could
//! not run at all. Parsing it WITHOUT implementing the counter was deliberately
//! not done: the phrase would have compiled and then never fired, which is a
//! wrong answer in silence rather than an honest failure.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let errs: Vec<&String> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| &d.message)
        .collect();
    assert!(errs.is_empty(), "parse errors: {errs:?}");
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

/// `writes` records into a page of 10 with FOOTING at `footing`.
fn report(footing: u32, writes: u32, advancing: &str) -> Vec<String> {
    let dir = std::env::temp_dir().join(format!("rcbl-linage-{footing}-{writes}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rep.txt").to_string_lossy().replace('\\', "/");
    let out = run_capture(&format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. LINRPT.\n\
         \x20      ENVIRONMENT DIVISION.\n\
         \x20      INPUT-OUTPUT SECTION.\n\
         \x20      FILE-CONTROL.\n\
         \x20          SELECT PRT ASSIGN TO \"{path}\"\n\
         \x20              ORGANIZATION IS LINE SEQUENTIAL.\n\
         \x20      DATA DIVISION.\n\
         \x20      FILE SECTION.\n\
         \x20      FD  PRT\n\
         \x20          LINAGE IS 10 LINES\n\
         \x20              WITH FOOTING AT {footing}\n\
         \x20              LINES AT TOP 2\n\
         \x20              LINES AT BOTTOM 3.\n\
         \x20      01  PRT-REC PIC X(20).\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01  WS-I PIC 9(2) VALUE 0.\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         \x20          OPEN OUTPUT PRT.\n\
         \x20          PERFORM {writes} TIMES\n\
         \x20              ADD 1 TO WS-I\n\
         \x20              MOVE \"line\" TO PRT-REC\n\
         \x20              WRITE PRT-REC {advancing}\n\
         \x20                  AT END-OF-PAGE DISPLAY \"EOP=\" WS-I\n\
         \x20                  NOT AT END-OF-PAGE DISPLAY \"OK=\" WS-I\n\
         \x20              END-WRITE\n\
         \x20          END-PERFORM.\n\
         \x20          CLOSE PRT.\n\
         \x20          STOP RUN.\n"
    ));
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// End of page begins AT the footing line, and every line before it is not.
#[test]
fn end_of_page_is_raised_from_the_footing_line() {
    let out = report(8, 9, "BEFORE ADVANCING 1 LINE");
    assert_eq!(
        out,
        vec![
            "OK=01", "OK=02", "OK=03", "OK=04", "OK=05", "OK=06", "OK=07", "EOP=08", "EOP=09",
        ],
        "footing 8 of a 10-line body: {out:?}"
    );
}

/// Without a FOOTING clause the condition waits for the body to fill —
/// `footing` defaults to the page size.
#[test]
fn without_footing_the_condition_waits_for_a_full_page() {
    let dir = std::env::temp_dir().join("rcbl-linage-nofoot");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rep.txt").to_string_lossy().replace('\\', "/");
    let out = run_capture(&format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. NOFOOT.\n\
         \x20      ENVIRONMENT DIVISION.\n\
         \x20      INPUT-OUTPUT SECTION.\n\
         \x20      FILE-CONTROL.\n\
         \x20          SELECT PRT ASSIGN TO \"{path}\"\n\
         \x20              ORGANIZATION IS LINE SEQUENTIAL.\n\
         \x20      DATA DIVISION.\n\
         \x20      FILE SECTION.\n\
         \x20      FD  PRT LINAGE IS 3 LINES.\n\
         \x20      01  PRT-REC PIC X(20).\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01  WS-I PIC 9(2) VALUE 0.\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         \x20          OPEN OUTPUT PRT.\n\
         \x20          PERFORM 3 TIMES\n\
         \x20              ADD 1 TO WS-I\n\
         \x20              MOVE \"x\" TO PRT-REC\n\
         \x20              WRITE PRT-REC AT END-OF-PAGE DISPLAY \"EOP=\" WS-I\n\
         \x20              END-WRITE\n\
         \x20          END-PERFORM.\n\
         \x20          CLOSE PRT.\n\
         \x20          STOP RUN.\n"
    ));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out, vec!["EOP=03"], "only the last line of a 3-line body: {out:?}");
}

/// `AT EOP` is the same phrase — SQ209M writes it where SQ201M writes
/// `AT END-OF-PAGE`.
#[test]
fn at_eop_is_the_same_phrase_as_at_end_of_page() {
    let dir = std::env::temp_dir().join("rcbl-linage-eop");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rep.txt").to_string_lossy().replace('\\', "/");
    let out = run_capture(&format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. EOPFORM.\n\
         \x20      ENVIRONMENT DIVISION.\n\
         \x20      INPUT-OUTPUT SECTION.\n\
         \x20      FILE-CONTROL.\n\
         \x20          SELECT PRT ASSIGN TO \"{path}\"\n\
         \x20              ORGANIZATION IS LINE SEQUENTIAL.\n\
         \x20      DATA DIVISION.\n\
         \x20      FILE SECTION.\n\
         \x20      FD  PRT LINAGE IS 2 LINES.\n\
         \x20      01  PRT-REC PIC X(20).\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01  WS-I PIC 9(2) VALUE 0.\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         \x20          OPEN OUTPUT PRT.\n\
         \x20          PERFORM 2 TIMES\n\
         \x20              ADD 1 TO WS-I\n\
         \x20              MOVE \"x\" TO PRT-REC\n\
         \x20              WRITE PRT-REC BEFORE ADVANCING 1 LINE\n\
         \x20                  AT EOP DISPLAY \"EOP=\" WS-I\n\
         \x20              END-WRITE\n\
         \x20          END-PERFORM.\n\
         \x20          CLOSE PRT.\n\
         \x20          STOP RUN.\n"
    ));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out, vec!["EOP=02"], "AT EOP must behave as AT END-OF-PAGE: {out:?}");
}

/// A file WITHOUT a LINAGE clause has no page, so no end-of-page condition —
/// the phrase must not fire on an ordinary file.
#[test]
fn a_file_without_linage_never_raises_end_of_page() {
    let dir = std::env::temp_dir().join("rcbl-linage-none");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("plain.txt").to_string_lossy().replace('\\', "/");
    let out = run_capture(&format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. NOLIN.\n\
         \x20      ENVIRONMENT DIVISION.\n\
         \x20      INPUT-OUTPUT SECTION.\n\
         \x20      FILE-CONTROL.\n\
         \x20          SELECT PRT ASSIGN TO \"{path}\"\n\
         \x20              ORGANIZATION IS LINE SEQUENTIAL.\n\
         \x20      DATA DIVISION.\n\
         \x20      FILE SECTION.\n\
         \x20      FD  PRT.\n\
         \x20      01  PRT-REC PIC X(20).\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         \x20          OPEN OUTPUT PRT.\n\
         \x20          MOVE \"x\" TO PRT-REC.\n\
         \x20          WRITE PRT-REC AT END-OF-PAGE DISPLAY \"SHOULD NOT FIRE\"\n\
         \x20          END-WRITE.\n\
         \x20          CLOSE PRT.\n\
         \x20          DISPLAY \"DONE\".\n\
         \x20          STOP RUN.\n"
    ));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out, vec!["DONE"], "no LINAGE means no page to end: {out:?}");
}
