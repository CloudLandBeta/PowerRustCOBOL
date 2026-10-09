// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `FILE STATUS IS data-name` must name a declared data item (operator report,
//! 2026-10-08: "se no FILE-STATUS eu informo uma variável não declarada o
//! compilador não avisa ... compila e executa normalmente sem acusar erro").
//!
//! The runtime made up an item of that name on first use, so the program ran;
//! every test of the status item the developer MEANT then passed unseen,
//! because the operations were filling in another.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, Severity};

fn analysed(status: &str, declare: &str, organization: &str) -> cobolt_semantic::SemanticResult {
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT CADEMP ASSIGN TO "cademp.dat"
               ORGANIZATION IS {organization}
               FILE STATUS IS {status}.
       DATA DIVISION.
       FILE SECTION.
       FD CADEMP.
       01 REG-EMP.
          05 EMP-CODIGO PIC 9(001).
       WORKING-STORAGE SECTION.
{declare}
       PROCEDURE DIVISION.
           STOP RUN.
"#
    );
    let program = parse(tokenize(&src, SourceFormat::Free)).program.expect("program should parse");
    analyze(&program)
}

fn file_status_errors(sem: &cobolt_semantic::SemanticResult) -> Vec<String> {
    sem.diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error && d.message.starts_with("FILE STATUS"))
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn a_file_status_that_is_not_declared_is_an_error() {
    for organization in ["LINE SEQUENTIAL", "SEQUENTIAL", "INDEXED"] {
        let declare = "       01 ST PIC XX.\n";
        let mut sem = analysed("XYZ", declare, organization);
        // INDEXED needs a record key to parse cleanly; its own message is not ours.
        sem.diagnostics.retain(|d| d.message.contains("XYZ") || !d.message.contains("RECORD KEY"));
        let e = file_status_errors(&sem);
        assert_eq!(e.len(), 1, "{organization}: {:?}", sem.diagnostics);
        assert!(
            e[0].contains("'XYZ'") && e[0].contains("'CADEMP'") && e[0].contains("is not declared in DATA DIVISION"),
            "{organization}: {e:?}"
        );
        assert!(!sem.is_ok(), "{organization}: the gates refuse the program");
    }
}

#[test]
fn a_declared_file_status_raises_nothing() {
    for status in ["ST", "st"] {
        let sem = analysed(status, "       01 ST PIC XX.\n", "LINE SEQUENTIAL");
        assert!(file_status_errors(&sem).is_empty(), "{status}: {:?}", sem.diagnostics);
        assert!(sem.is_ok(), "{status}: {:?}", sem.diagnostics);
    }
}
