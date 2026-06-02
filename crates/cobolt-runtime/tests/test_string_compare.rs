// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! COBOL alphanumeric comparison must pad the shorter operand with spaces, so a
//! space-padded `PIC X(n)` field equals a short literal. Regression for event
//! dispatch (`EVALUATE control-id WHEN "BTN-OK"`).

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_get(src: &str, var: &str) -> String {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let mut i = Interpreter::new(result.program.expect("no program"));
    i.run().expect("run failed");
    i.env.get_string(var).unwrap_or_default().trim().to_owned()
}

#[test]
fn padded_field_equals_short_literal() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-ID  PIC X(64) VALUE SPACES.
       01 WS-OUT PIC X(20) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "BTN-OK" TO WS-ID
           EVALUATE WS-ID
               WHEN "BTN-OK"   MOVE "ok" TO WS-OUT
               WHEN "BTN-FAIL" MOVE "fail" TO WS-OUT
               WHEN OTHER      MOVE "none" TO WS-OUT
           END-EVALUATE
           STOP RUN.
    "#;
    assert_eq!(run_get(src, "WS-OUT"), "ok");
}

#[test]
fn padded_field_if_equality_and_inequality() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMP2.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-ID  PIC X(32) VALUE SPACES.
       01 WS-OUT PIC X(20) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "BTN-FAIL" TO WS-ID
           IF WS-ID = "BTN-OK"
               MOVE "wrong" TO WS-OUT
           ELSE
               MOVE "right" TO WS-OUT
           END-IF
           STOP RUN.
    "#;
    assert_eq!(run_get(src, "WS-OUT"), "right");
}