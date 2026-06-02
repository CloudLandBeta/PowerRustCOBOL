// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Integration tests for COBOL-85 nested program dispatch (Phase 4).
//!
//! Each test parses a complete COBOL source that contains one or more nested
//! programs and verifies the expected runtime behaviour through the
//! `Interpreter`.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Tokenize + parse source and return an `Interpreter`; panics on any error.
fn interp(src: &str) -> Interpreter {
    let tokens = tokenize(src, SourceFormat::Free);
    let result = parse(tokens);
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "Parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program returned");
    Interpreter::new(program)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// A CALL to a nested program's PROGRAM-ID runs its statements and returns
/// normally (GOBACK is not propagated to the outer program as an error).
#[test]
fn call_nested_program_runs_and_returns() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-RESULT  PIC 9(3) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           CALL "SET-RESULT".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SET-RESULT.
       PROCEDURE DIVISION.
           MOVE 42 TO WS-RESULT.
           GOBACK.
       END PROGRAM SET-RESULT.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    let val = i.env.get_i64("WS-RESULT").unwrap_or(0);
    assert_eq!(val, 42, "nested program should have set WS-RESULT to 42");
}

/// The nested program's own local WORKING-STORAGE items are visible during
/// execution but do not leak back into the outer program's environment after
/// the call returns.
#[test]
fn nested_local_ws_is_removed_after_goback() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-FLAG  PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           CALL "INNER".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. INNER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 LOCAL-ITEM  PIC X(10) VALUE "HELLO".
       PROCEDURE DIVISION.
           MOVE 1 TO WS-FLAG.
           GOBACK.
       END PROGRAM INNER.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");

    // WS-FLAG should have been set by the nested program (it's in outer's env).
    assert_eq!(i.env.get_i64("WS-FLAG").unwrap_or(0), 1);

    // LOCAL-ITEM was the nested program's own WS; it must not persist.
    assert!(
        !i.env.contains("LOCAL-ITEM"),
        "LOCAL-ITEM must be removed from env after GOBACK"
    );
}

/// GLOBAL data items in the outer program's WORKING-STORAGE are visible to
/// nested programs and mutations are seen back in the outer env.
#[test]
fn global_items_shared_with_nested_program() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-COUNTER  PIC 9(5) VALUE 10 GLOBAL.
       PROCEDURE DIVISION.
       MAIN.
           CALL "BUMP-COUNTER".
           CALL "BUMP-COUNTER".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUMP-COUNTER.
       PROCEDURE DIVISION.
           ADD 1 TO WS-COUNTER.
           GOBACK.
       END PROGRAM BUMP-COUNTER.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    assert_eq!(
        i.env.get_i64("WS-COUNTER").unwrap_or(0),
        12,
        "WS-COUNTER should be 10 + 2 calls"
    );
}

/// Calling a nested program that itself contains paragraphs — only the nested
/// program's own para_map is used for GO TO inside that program.
#[test]
fn nested_program_internal_goto() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-VAL  PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           CALL "NESTED".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. NESTED.
       PROCEDURE DIVISION.
       STEP-A.
           MOVE 1 TO WS-VAL.
           GO TO STEP-B.
       STEP-B.
           ADD 1 TO WS-VAL.
           GOBACK.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    assert_eq!(i.env.get_i64("WS-VAL").unwrap_or(0), 2);
}

/// Multiple nested programs — each CALL dispatches to the correct one.
#[test]
fn multiple_nested_programs_dispatch_independently() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A  PIC 9 VALUE 0.
       01 WS-B  PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           CALL "SET-A".
           CALL "SET-B".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SET-A.
       PROCEDURE DIVISION.
           MOVE 1 TO WS-A.
           GOBACK.
       END PROGRAM SET-A.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SET-B.
       PROCEDURE DIVISION.
           MOVE 2 TO WS-B.
           GOBACK.
       END PROGRAM SET-B.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    assert_eq!(i.env.get_i64("WS-A").unwrap_or(0), 1);
    assert_eq!(i.env.get_i64("WS-B").unwrap_or(0), 2);
}

/// A nested program with no END PROGRAM terminator (last nested program in the
/// file) is still registered and callable.
#[test]
fn nested_program_without_end_program_terminator() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-OK  PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           CALL "NO-TERM".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. NO-TERM.
       PROCEDURE DIVISION.
           MOVE 9 TO WS-OK.
           GOBACK.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    assert_eq!(i.env.get_i64("WS-OK").unwrap_or(0), 9);
}
