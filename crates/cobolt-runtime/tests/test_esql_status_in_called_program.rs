// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A CALLed program that declares its own `SQLSTATE` sees its own status.
//!
//! Reported as "FETCH does not answer 02000 past the last row after a
//! COMMIT": a loop `PERFORM UNTIL SQLSTATE NOT = "00000"` around a FETCH ran
//! for ever, the last row left in the host variables. The sequence itself
//! (SELECT INTO, UPDATE, COMMIT, OPEN, FETCH) answers 02000 on SQLite and on
//! MySQL when it runs in the outermost program; what made it loop is the
//! program it ran in. The status items were found once, at the run's first
//! statement, as the outermost program's; a CALLed program reads its own
//! items through the call's activation divert, so its SQLSTATE never changed
//! — and a Backend nests every program it CALLs in the exported one.
//!
//! `tests/cobol/esql/status_in_called_program.cbl` is the quantified COBOL
//! counterpart, run here on SQLite and (ignored, with `PRC_TEST_MYSQL_URL`)
//! on MySQL.

use std::path::Path;
use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let errors: Vec<_> = result.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(result.program.unwrap(), event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|l| l.trim_end().to_string()).collect()
}

fn cobol_test() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/esql/status_in_called_program.cbl");
    std::fs::read_to_string(path).expect("read the COBOL test")
}

fn assert_passed(out: &[String]) {
    let report = out.join("\n");
    println!("{report}");
    let verdict = out.iter().rev().find(|l| l.starts_with("PASS ")).expect("no PASS/FAIL line");
    assert_eq!(verdict, "PASS 008 FAIL 000", "{report}");
}

/// The report's sequence in a CALLed program, with a guard on the loop so a
/// regression fails instead of hanging.
#[test]
fn a_called_programs_fetch_loop_ends_with_02000() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAINX.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  SQLSTATE           PIC X(5).
       PROCEDURE DIVISION.
           EXEC SQL CONNECT TO ':memory:' END-EXEC
           EXEC SQL CREATE TABLE t (n INTEGER) END-EXEC
           EXEC SQL INSERT INTO t VALUES (1), (2), (3) END-EXEC
           EXEC SQL COMMIT END-EXEC
           MOVE "ABCDE" TO SQLSTATE
           CALL "PERMS"
           DISPLAY "caller [" SQLSTATE "]"
           STOP RUN.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PERMS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  SQLSTATE           PIC X(5) VALUE "00000".
       01  H-N                PIC 9(3).
       01  WS-ROWS            PIC 9(3) VALUE 0.
       01  WS-LOOPS           PIC 9(3) VALUE 0.
           EXEC SQL DECLARE C1 CURSOR FOR SELECT n FROM t ORDER BY n
           END-EXEC.
       PROCEDURE DIVISION.
           EXEC SQL UPDATE t SET n = n WHERE n = 1 END-EXEC
           EXEC SQL COMMIT END-EXEC
           EXEC SQL OPEN C1 END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000" OR WS-LOOPS > 50
               ADD 1 TO WS-LOOPS
               EXEC SQL FETCH C1 INTO :H-N END-EXEC
               IF SQLSTATE = "00000"
                   ADD 1 TO WS-ROWS
               END-IF
           END-PERFORM
           DISPLAY "rows " WS-ROWS " loops " WS-LOOPS " [" SQLSTATE "]"
           EXEC SQL CLOSE C1 END-EXEC
           GOBACK.
       END PROGRAM PERMS.
       END PROGRAM MAINX.
"#;
    assert_eq!(
        run_capture(src),
        vec!["rows 003 loops 004 [02000]".to_string(), "caller [ABCDE]".to_string()]
    );
}

#[test]
fn the_quantified_cobol_program_passes_on_sqlite() {
    assert_passed(&run_capture(&cobol_test()));
}

/// The reporter's database. `source` the MySQL test environment first.
#[test]
#[ignore]
fn the_quantified_cobol_program_passes_on_mysql() {
    let Ok(url) = std::env::var("PRC_TEST_MYSQL_URL") else {
        println!("SKIPPED: PRC_TEST_MYSQL_URL is not set");
        return;
    };
    let src = cobol_test().replace("':memory:'", &format!("'{url}'"));
    assert_passed(&run_capture(&src));
}
