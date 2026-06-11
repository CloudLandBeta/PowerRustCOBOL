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
        error_messages(&sem).iter().any(|m| m.contains("paragraph 'PARA-A'")),
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
        error_messages(&sem).iter().any(|m| m.contains("section 'MAIN-SECTION'")),
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
