// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `COBOL::"NAME"( args )` — the inline spelling of a built-in
//! `CALL "COBOL-NAME" USING args` (operator, 2026-09-28). It must BE that
//! call: the same result written back into a BY REFERENCE argument, the same
//! warning for a name that does not exist — and it must run wherever it
//! sits, including straight after a statement whose operand list could
//! otherwise swallow it.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Both tests move the process's working directory to a scratch folder for
/// their files; the lock keeps them from doing it at the same time.
static CWD: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
fn an_inline_cobol_call_is_the_call_wherever_it_sits() {
    let t = std::time::Instant::now();
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INLINECALL.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F1 ASSIGN TO "cobol-object-call.tmp"
               ORGANIZATION IS LINE SEQUENTIAL.
       DATA DIVISION.
       FILE SECTION.
       FD  F1.
       01  F1-REC PIC X(10).
       WORKING-STORAGE SECTION.
       01 WS-REF  PIC 9(4) VALUE 0.
       01 N1      PIC 9(4) VALUE 0.
       01 N2      PIC 9(4) VALUE 0.
       01 N3      PIC 9(4) VALUE 0.
       01 N4      PIC 9(4) VALUE 0.
       01 N5      PIC 9(4) VALUE 0.
       01 N6      PIC 9(4) VALUE 0.
       01 N7      PIC 9(4) VALUE 0.
       01 WS-A    PIC X(10).
       01 WS-B    PIC X(10).
       01 WS-S    PIC X(20).
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-PROVIDER-COUNT" USING WS-REF
           DISPLAY "ref " WS-REF
           COBOL::"PROVIDER-COUNT" ( N1 )
           MOVE "x" TO WS-A WS-B
           COBOL::"PROVIDER-COUNT" ( N2 )
           COMPUTE N7 = 1 + 2
           COBOL::"PROVIDER-COUNT" ( N3 )
           STRING "a" "b" DELIMITED BY SIZE INTO WS-S
           OPEN OUTPUT F1
           CLOSE F1
           COBOL::"PROVIDER-COUNT" ( N4 )
           CALL "COBOL-PROVIDER-COUNT" USING N7
           COBOL::PROVIDER-COUNT ( N5 )
           IF N1 > 0
               COBOL::"COBOL-PROVIDER-COUNT" ( N6 )
           END-IF
           DISPLAY N1 " " N2 " " N3 " " N4 " " N5 " " N6
           STOP RUN.
    "#;
    let dir = std::env::temp_dir().join(format!("cobol-object-call-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let _cwd = CWD.lock().unwrap_or_else(|e| e.into_inner());
    let here = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    let out = run_capture(src);
    std::env::set_current_dir(here).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    let reference = out[0].trim_start_matches("ref ").to_string();
    let got: Vec<&str> = out[1].split_whitespace().collect();
    let forms = [
        "COBOL::\"PROVIDER-COUNT\" ( N1 )           after DISPLAY",
        "COBOL::\"PROVIDER-COUNT\" ( N2 )           after MOVE … TO a b",
        "COBOL::\"PROVIDER-COUNT\" ( N3 )           after COMPUTE",
        "COBOL::\"PROVIDER-COUNT\" ( N4 )           after CLOSE f",
        "COBOL::PROVIDER-COUNT ( N5 )             unquoted, after CALL … USING",
        "COBOL::\"COBOL-PROVIDER-COUNT\" ( N6 )     full name, inside IF",
    ];
    println!("\n  ── COBOL:: inline calls ──");
    println!("  CALL \"COBOL-PROVIDER-COUNT\" USING WS-REF → {reference}");
    for (f, v) in forms.iter().zip(&got) {
        println!("  {f:<46} → {v}");
    }
    println!("  {} forms, all equal to the CALL: {}; {ms:.1} ms\n", forms.len(), got.iter().all(|v| *v == reference));
    assert_ne!(reference, "0000", "the built-in answers");
    assert_eq!(got, vec![reference.as_str(); 6], "every inline call wrote the CALL's result back");
}

/// The generated IndexedFile facade reads the engine's FILE STATUS inside
/// `INVALID KEY … NOT INVALID KEY` with `COBOL::"FILE-STATUS"( … )`. As a
/// `CALL` it needed `END-CALL`, or it took the `NOT` for its own
/// `NOT ON EXCEPTION`; the inline form ends at its `)` — each branch must
/// still run its own call and report the real status.
#[test]
fn an_inline_file_status_inside_invalid_key_reports_each_branch() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INLINEFS.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT CUST ASSIGN TO "inline-fs.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CUST-ID
               STORAGE IS MEMORY WITH PERSISTENCE.
       DATA DIVISION.
       FILE SECTION.
       FD  CUST.
       01  CUST-REC.
           05 CUST-ID   PIC 9(4).
           05 CUST-NAME PIC X(10).
       WORKING-STORAGE SECTION.
       01 WS-FS   PIC XX.
       01 WS-PATH PIC X VALUE SPACE.
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT CUST
           MOVE 1 TO CUST-ID
           MOVE "ADA" TO CUST-NAME
           WRITE CUST-REC
               INVALID KEY
                   COBOL::"FILE-STATUS" ( "CUST" WS-FS )
                   MOVE "I" TO WS-PATH
               NOT INVALID KEY
                   COBOL::"FILE-STATUS" ( "CUST" WS-FS )
                   MOVE "N" TO WS-PATH
           END-WRITE
           DISPLAY "first " WS-PATH " " WS-FS
           WRITE CUST-REC
               INVALID KEY
                   COBOL::"FILE-STATUS" ( "CUST" WS-FS )
                   MOVE "I" TO WS-PATH
               NOT INVALID KEY
                   COBOL::"FILE-STATUS" ( "CUST" WS-FS )
                   MOVE "N" TO WS-PATH
           END-WRITE
           DISPLAY "duplicate " WS-PATH " " WS-FS
           CLOSE CUST
           STOP RUN.
    "#;
    let dir = std::env::temp_dir().join(format!("inline-fs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let _cwd = CWD.lock().unwrap_or_else(|e| e.into_inner());
    let here = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    let out = run_capture(src);
    std::env::set_current_dir(here).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    println!("  WRITE, then the same key again: {out:?}");
    assert_eq!(out, vec!["first N 00", "duplicate I 22"]);
}
