// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `SET WS-INDEX TO 1` on a `USAGE OBJECT REFERENCE RUST-I64` item is a write
//! into a Rust integer: the item has no PIC, so it is not "an alphanumeric PIC
//! clause", and the warning that said so was false. A PIC X receiver still warns.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::analyze;

fn warnings(body: &str) -> Vec<String> {
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. T.\n       ENVIRONMENT DIVISION.\n\
         CONFIGURATION SECTION.\n       REPOSITORY.\n           CLASS RUST-I64 IS \"Rust.i64\".\n\
         DATA DIVISION.\n       WORKING-STORAGE SECTION.\n\
         01 WS-INDEX USAGE IS OBJECT REFERENCE RUST-I64.\n01 WS-T PIC X(10).\n\
         PROCEDURE DIVISION.\n       MAIN-P.\n{body}\n           STOP RUN.\n"
    );
    let program = parse(tokenize(&src, SourceFormat::Free)).program.expect("a program");
    analyze(&program)
        .diagnostics
        .iter()
        .map(|d| d.message.clone())
        .filter(|m| m.contains("moving a numeric literal"))
        .collect()
}

#[test]
fn a_numeric_literal_into_an_object_reference_is_not_warned() {
    assert!(warnings("           SET WS-INDEX TO 1\n           MOVE 0 TO WS-INDEX").is_empty());
}

#[test]
fn a_numeric_literal_into_pic_x_still_warns() {
    assert_eq!(warnings("           MOVE 1 TO WS-T").len(), 1);
}
