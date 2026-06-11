// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tests: a program may not redeclare a unique element.
//!
//! A second PROGRAM-ID, or a second ENVIRONMENT / DATA / PROCEDURE DIVISION
//! header inside the same program unit, must produce a hard parse error so the
//! source cannot be run until it is corrected. A legitimate second program unit
//! (sequential sibling or true nesting) must NOT be flagged.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};

fn error_count(src: &str) -> usize {
    let result = parse(tokenize(src, SourceFormat::Free));
    result.diagnostics.iter().filter(|d| d.severity == Severity::Error).count()
}

fn has_error_containing(src: &str, needle: &str) -> bool {
    let result = parse(tokenize(src, SourceFormat::Free));
    result.diagnostics.iter().any(|d| {
        d.severity == Severity::Error && d.message.contains(needle)
    })
}

#[test]
fn duplicate_program_id_is_an_error() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MYPROG.
       PROCEDURE DIVISION.
           DISPLAY "HELLO".
       PROGRAM-ID. MYPROGNEWNAME.
           STOP RUN.
"#;
    assert!(
        has_error_containing(src, "PROGRAM-ID is declared more than once"),
        "a redeclared PROGRAM-ID must raise a parse error"
    );
}

#[test]
fn duplicate_data_division_is_an_error() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A PIC X(4).
       DATA DIVISION.
       PROCEDURE DIVISION.
           STOP RUN.
"#;
    assert!(
        has_error_containing(src, "DATA DIVISION is declared more than once"),
        "a redeclared DATA DIVISION must raise a parse error"
    );
}

#[test]
fn duplicate_procedure_division_is_an_error() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           DISPLAY "A".
       PROCEDURE DIVISION.
           DISPLAY "B".
"#;
    assert!(
        has_error_containing(src, "PROCEDURE DIVISION is declared more than once"),
        "a redeclared PROCEDURE DIVISION must raise a parse error"
    );
}

#[test]
fn well_formed_single_program_has_no_redeclaration_error() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A PIC X(4).
       PROCEDURE DIVISION.
           DISPLAY "OK".
           STOP RUN.
"#;
    assert_eq!(error_count(src), 0, "a well-formed program must not be flagged");
}

#[test]
fn nested_and_sibling_programs_are_not_flagged() {
    // Two legitimate program units, each with its own PROGRAM-ID / divisions.
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       PROCEDURE DIVISION.
           CALL "INNER".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. INNER.
       PROCEDURE DIVISION.
           DISPLAY "INNER".
       END PROGRAM INNER.
"#;
    assert_eq!(
        error_count(src), 0,
        "distinct program units must not be treated as redeclarations"
    );
}
