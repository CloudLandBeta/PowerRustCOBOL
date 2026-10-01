// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! An INDEXED file's RECORD KEY and ALTERNATE RECORD KEYs must be fields of
//! its own record (operator report, 2026-09-30, LugSys: the FD's key field
//! was renamed, the SELECT kept the old name, the program compiled, WRITE
//! answered 00 and no READ by key ever found the record).

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, Severity};

fn errors(record_key: &str, alternate: &str) -> Vec<String> {
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT CADEMP ASSIGN TO "cademp.dat"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS {record_key}
               {alternate}
               FILE STATUS IS ST.
       DATA DIVISION.
       FILE SECTION.
       FD CADEMP.
       01 REG-EMP.
          05 EMP-CODIGO PIC 9(001).
          05 EMP-DADOS.
             10 EMP-NOME PIC X(060).
       WORKING-STORAGE SECTION.
       01 ST PIC XX.
       PROCEDURE DIVISION.
           STOP RUN.
"#
    );
    let program = parse(tokenize(&src, SourceFormat::Free)).program.expect("program should parse");
    analyze(&program)
        .diagnostics
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.message)
        .collect()
}

#[test]
fn a_record_key_that_is_not_in_the_record_is_an_error() {
    let e = errors("EMP-CHAVE", "");
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].contains("RECORD KEY EMP-CHAVE") && e[0].contains("FD CADEMP"), "{e:?}");
}

#[test]
fn an_alternate_key_that_is_not_in_the_record_is_an_error() {
    let e = errors("EMP-CODIGO", "ALTERNATE RECORD KEY IS EMP-RAZAO WITH DUPLICATES");
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].contains("ALTERNATE RECORD KEY EMP-RAZAO"), "{e:?}");
}

#[test]
fn keys_anywhere_in_the_record_are_accepted() {
    assert!(errors("EMP-CODIGO", "ALTERNATE RECORD KEY IS EMP-NOME WITH DUPLICATES").is_empty());
    assert!(errors("emp-codigo", "").is_empty(), "case does not matter");
}
