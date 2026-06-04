// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! SORT runtime: procedure-based (RELEASE / RETURN via INPUT/OUTPUT PROCEDURE)
//! and file-based (USING / GIVING).

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
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
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

#[test]
fn sort_input_output_procedure() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SRT.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT SORT-FILE ASSIGN TO "sortwork".
       DATA DIVISION.
       FILE SECTION.
       SD SORT-FILE.
       01 SORT-REC.
          05 SORT-KEY  PIC 9(2).
          05 SORT-DATA PIC X(3).
       WORKING-STORAGE SECTION.
       01 DONE-FLAG PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           SORT SORT-FILE ON DESCENDING KEY SORT-KEY
               INPUT PROCEDURE IS FILL-PROC
               OUTPUT PROCEDURE IS SHOW-PROC
           STOP RUN.
       FILL-PROC.
           MOVE 30 TO SORT-KEY MOVE "CCC" TO SORT-DATA RELEASE SORT-REC
           MOVE 10 TO SORT-KEY MOVE "AAA" TO SORT-DATA RELEASE SORT-REC
           MOVE 20 TO SORT-KEY MOVE "BBB" TO SORT-DATA RELEASE SORT-REC.
       SHOW-PROC.
           PERFORM UNTIL DONE-FLAG = 1
               RETURN SORT-FILE
                   AT END MOVE 1 TO DONE-FLAG
                   NOT AT END DISPLAY SORT-DATA
               END-RETURN
           END-PERFORM.
    "#;
    // DESCENDING by key 30,20,10 → CCC, BBB, AAA
    assert_eq!(run_capture(src), vec!["CCC", "BBB", "AAA"]);
}

#[test]
fn sort_using_giving_files() {
    let dir = std::env::temp_dir();
    let inp = dir.join("prc_sort_in.dat");
    let out = dir.join("prc_sort_out.dat");
    std::fs::write(&inp, "30CCC\n10AAA\n20BBB\n").unwrap();
    let _ = std::fs::remove_file(&out);

    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUG.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT IN-FILE ASSIGN TO "{inp}"
               ORGANIZATION IS LINE SEQUENTIAL.
           SELECT OUT-FILE ASSIGN TO "{out}"
               ORGANIZATION IS LINE SEQUENTIAL.
           SELECT SORT-FILE ASSIGN TO "{wk}".
       DATA DIVISION.
       FILE SECTION.
       FD IN-FILE.
       01 IN-REC PIC X(5).
       FD OUT-FILE.
       01 OUT-REC PIC X(5).
       SD SORT-FILE.
       01 SORT-REC.
          05 SORT-KEY  PIC 9(2).
          05 SORT-DATA PIC X(3).
       WORKING-STORAGE SECTION.
       01 EOF-FLAG PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           SORT SORT-FILE ON ASCENDING KEY SORT-KEY
               USING IN-FILE GIVING OUT-FILE
           OPEN INPUT OUT-FILE
           PERFORM UNTIL EOF-FLAG = 1
               READ OUT-FILE
                   AT END MOVE 1 TO EOF-FLAG
                   NOT AT END DISPLAY OUT-REC
               END-READ
           END-PERFORM
           CLOSE OUT-FILE
           STOP RUN.
    "#,
        inp = inp.display(),
        out = out.display(),
        wk = dir.join("prc_sort_wk.dat").display(),
    );
    assert_eq!(run_capture(&src), vec!["10AAA", "20BBB", "30CCC"]);
}
