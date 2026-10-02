// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tests: redeclared procedure names (paragraphs and sections) are hard errors.
//!
//! A program that defines the same paragraph or section name twice must not be
//! allowed to run; semantic analysis reports a [`Severity::Error`] so the caller
//! blocks execution until the conflict is resolved.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, Severity};

fn analyze_src(src: &str) -> cobolt_semantic::SemanticResult {
    let program = parse(tokenize(src, SourceFormat::Free))
        .program
        .expect("program should parse");
    analyze(&program)
}

fn error_messages(sem: &cobolt_semantic::SemanticResult) -> Vec<String> {
    sem.diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn duplicate_paragraph_is_an_error() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
       PARA-A.
           DISPLAY "FIRST".
       PARA-A.
           DISPLAY "SECOND".
"#;
    let sem = analyze_src(src);
    assert!(!sem.is_ok(), "a redeclared paragraph must fail analysis");
    assert!(
        error_messages(&sem)
            .iter()
            .any(|m| m.contains("paragraph 'PARA-A'")),
        "missing duplicate-paragraph error: {:?}",
        error_messages(&sem)
    );
}

#[test]
fn duplicate_section_is_an_error() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
       MAIN-SECTION SECTION.
           DISPLAY "A".
       MAIN-SECTION SECTION.
           DISPLAY "B".
"#;
    let sem = analyze_src(src);
    assert!(!sem.is_ok(), "a redeclared section must fail analysis");
    assert!(
        error_messages(&sem)
            .iter()
            .any(|m| m.contains("section 'MAIN-SECTION'")),
        "missing duplicate-section error: {:?}",
        error_messages(&sem)
    );
}

#[test]
fn distinct_paragraphs_are_accepted() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
       PARA-A.
           DISPLAY "A".
       PARA-B.
           DISPLAY "B".
"#;
    let sem = analyze_src(src);
    assert!(
        error_messages(&sem).is_empty(),
        "distinct paragraphs must not be flagged: {:?}",
        error_messages(&sem)
    );
}

/// Two contained programs with one name - in a RAD form, two user procedures
/// called the same - were accepted, and `CALL "DO-IT"` silently ran the LAST
/// one. COBOL-85 requires every program of a source to have its own name.
#[test]
fn two_contained_programs_with_one_name_are_an_error() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-PROG.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-X GLOBAL PIC 9 VALUE 0.
       PROCEDURE DIVISION.
           CALL "DO-IT"
           STOP RUN.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DO-IT IS COMMON PROGRAM.
       PROCEDURE DIVISION.
           MOVE 1 TO WS-X
           GOBACK.
       END PROGRAM DO-IT.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DO-IT IS COMMON PROGRAM.
       PROCEDURE DIVISION.
           MOVE 2 TO WS-X
           GOBACK.
       END PROGRAM DO-IT.
       END PROGRAM MAIN-PROG.
"#;
    let sem = analyze_src(src);
    assert!(!sem.is_ok(), "two programs named DO-IT must fail analysis");
    let errs = error_messages(&sem);
    let hits = errs.iter().filter(|m| m.contains("program 'DO-IT' is declared more than once")).count();
    assert_eq!(hits, 1, "exactly one duplicate-program error expected: {errs:?}");
    // The error points at the SECOND declaration's IDENTIFICATION DIVISION.
    let at = sem
        .diagnostics
        .iter()
        .find(|d| d.message.contains("program 'DO-IT'"))
        .map(|d| d.span.line)
        .unwrap();
    let second = src.lines().enumerate().filter(|(_, l)| l.contains("PROGRAM-ID. DO-IT")).nth(1).unwrap().0;
    assert_eq!(at as usize, second, "reported on the line before the second PROGRAM-ID (its IDENTIFICATION DIVISION)");
    println!("duplicate contained program: {hits} error - {:?}", errs);
}

/// A contained program may not take the outermost program's name either, and
/// names compare without regard to case.
#[test]
fn a_contained_program_named_like_its_container_is_an_error() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ORDERS.
       PROCEDURE DIVISION.
           STOP RUN.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. orders.
       PROCEDURE DIVISION.
           GOBACK.
       END PROGRAM orders.
       END PROGRAM ORDERS.
"#;
    let sem = analyze_src(src);
    assert!(
        // The parser upper-cases a PROGRAM-ID, so the second one reads ORDERS.
        error_messages(&sem).iter().any(|m| m.contains("program 'ORDERS' is declared more than once")),
        "missing error: {:?}",
        error_messages(&sem)
    );
}

/// Distinct names stay valid.
#[test]
fn contained_programs_with_distinct_names_are_fine() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-PROG.
       PROCEDURE DIVISION.
           CALL "ONE"
           CALL "TWO"
           STOP RUN.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ONE IS COMMON PROGRAM.
       PROCEDURE DIVISION.
           GOBACK.
       END PROGRAM ONE.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TWO IS COMMON PROGRAM.
       PROCEDURE DIVISION.
           GOBACK.
       END PROGRAM TWO.
       END PROGRAM MAIN-PROG.
"#;
    let sem = analyze_src(src);
    assert!(
        !error_messages(&sem).iter().any(|m| m.contains("declared more than once")),
        "distinct names must not be reported: {:?}",
        error_messages(&sem)
    );
}
