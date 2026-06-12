// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `STRING` with smart default `DELIMITED BY` (no explicit clause):
//!   * literals                          → DELIMITED BY SIZE
//!   * alphanumeric (`PIC X`) data items → DELIMITED BY SPACES (trailing pad off)
//!   * numeric / numeric-edited items    → DELIMITED BY SIZE (field characters)
//!   * function results / expressions     → DELIMITED BY SIZE
//! Data items are rendered in their field form (PIC-width digits, edited chars).

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_get(src: &str, var: &str) -> String {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}", result.diagnostics
    );
    let mut i = Interpreter::new(result.program.expect("no program"));
    i.run().expect("run failed");
    // Receiving field is PIC X(n): strip the trailing space padding only.
    i.env.get_string(var).unwrap_or_default().trim_end().to_owned()
}

#[test]
fn smart_default_delimiters_compose_a_sentence() {
    // The motivating example: no DELIMITED BY anywhere — each operand picks its
    // own sensible default from its category.
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. STRDEF.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 NAME-X        PIC X(40) VALUE "Joe".
       01 SALARY        PIC S9(09) VALUE 100000.
       01 SALARY-EDITED PIC ZZZ,ZZZ,ZZ9.99.
       01 TEXT-OUT      PIC X(100) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN.
           MOVE SALARY TO SALARY-EDITED
           STRING NAME-X
                  " earns "
                  SALARY
                  " or US$"
                  FUNCTION TRIM(SALARY-EDITED)
             INTO TEXT-OUT
           STOP RUN.
    "#;
    // NAME-X  → "Joe"          (alphanumeric → spaces)
    // " earns " → " earns "    (literal → size)
    // SALARY  → "000100000"    (numeric → size, full PIC width)
    // " or US$" → " or US$"
    // TRIM(edited) → "100,000.00" (function → size)
    assert_eq!(run_get(src, "TEXT-OUT"), "Joe earns 000100000 or US$100,000.00");
}

#[test]
fn alphanumeric_default_drops_trailing_spaces_only() {
    // DELIMITED BY SPACES keeps internal spaces, drops trailing padding.
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. STRSP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 FULL-NAME PIC X(20) VALUE "Joe Smith".
       01 TEXT-OUT  PIC X(40) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN.
           STRING FULL-NAME "!" INTO TEXT-OUT
           STOP RUN.
    "#;
    assert_eq!(run_get(src, "TEXT-OUT"), "Joe Smith!");
}

#[test]
fn explicit_delimited_by_still_honoured() {
    // An explicit DELIMITED BY overrides the per-type default.
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. STREX.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 NAME-X   PIC X(10) VALUE "Joe".
       01 TEXT-OUT PIC X(40) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN.
           STRING NAME-X DELIMITED BY SIZE "|" INTO TEXT-OUT
           STOP RUN.
    "#;
    // DELIMITED BY SIZE keeps the full 10-char padded field, then "|".
    assert_eq!(run_get(src, "TEXT-OUT"), "Joe       |");
}
