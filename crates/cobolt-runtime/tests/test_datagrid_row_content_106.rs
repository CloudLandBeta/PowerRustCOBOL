// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 106 — how `onRowButtonClick` hands a row to its handler.
//!
//! The generated main program owns one plain buffer, `DATAGRID-ROW-BUFFER PIC X(n)`; the
//! runtime writes the clicked row's record image into it; the dispatcher calls the handler
//! `USING` it; and the handler describes the same bytes as `01 DATAGRID-ROW-CONTENT`, with a
//! field per column. That is ordinary COBOL — a group parameter over an elementary argument
//! of another name — and this pins that the runtime reads it field by field, numbers and
//! signs included, and that a handler's writes reach the buffer.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let errors: Vec<String> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("{}:{}: {}", d.span.line, d.span.col, d.message))
        .collect();
    assert!(errors.is_empty(), "parse errors: {errors:#?}");
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    drop(interp);
    display_rx.try_iter().map(|l| l.trim_end().to_string()).collect()
}

#[test]
fn a_handler_reads_the_row_through_its_own_description_of_the_buffer() {
    // 9 + 10 + 5 + 12 + 7 = 43 bytes: ROW-NUMBER 9(9), ROW-NAME X(10), ACTOR-ID 9(5),
    // ACTOR-NAME X(12), SALARY S9(5)V99 (trailing overpunch: -123.45 is 001234N).
    let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. MAINP.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 DATAGRID-ROW-BUFFER PIC X(43).
PROCEDURE DIVISION.
    MOVE "000000002EDIT      00042Ann Smith   001234N" TO DATAGRID-ROW-BUFFER.
    CALL "GRID1--ONROWBUTTONCLICK" USING DATAGRID-ROW-BUFFER.
    DISPLAY "BACK " DATAGRID-ROW-BUFFER.
    STOP RUN.

IDENTIFICATION DIVISION.
PROGRAM-ID. GRID1--ONROWBUTTONCLICK IS COMMON PROGRAM.
DATA DIVISION.
LINKAGE SECTION.
01 DATAGRID-ROW-CONTENT.
   05 ROW-NUMBER PIC 9(9).
   05 ROW-NAME PIC X(10).
   05 ROW-COLUMNS.
      10 ACTOR-ID PIC 9(5).
      10 ACTOR-NAME PIC X(12).
      10 SALARY PIC S9(5)V99.
PROCEDURE DIVISION USING DATAGRID-ROW-CONTENT.
    DISPLAY "ROW " ROW-NUMBER.
    DISPLAY "NAME " ROW-NAME.
    DISPLAY "ID " ACTOR-ID.
    DISPLAY "WHO " ACTOR-NAME.
    IF SALARY < 0
        DISPLAY "NEGATIVE"
    END-IF.
    COMPUTE ACTOR-ID = ACTOR-ID + 1.
    GOBACK.
END PROGRAM GRID1--ONROWBUTTONCLICK.

END PROGRAM MAINP.
"#;
    let out = run(src);
    println!("{out:#?}");
    assert!(out.iter().any(|l| l == "ROW 000000002"), "{out:?}");
    assert!(out.iter().any(|l| l == "NAME EDIT"), "{out:?}");
    assert!(out.iter().any(|l| l == "ID 00042"), "{out:?}");
    assert!(out.iter().any(|l| l == "WHO Ann Smith"), "{out:?}");
    assert!(out.iter().any(|l| l == "NEGATIVE"), "the signed field reads as negative: {out:?}");
    assert!(
        out.iter().any(|l| l.starts_with("BACK 000000002EDIT      00043")),
        "a handler's write lands in the buffer it was handed: {out:?}"
    );
}

/// A record view reads a numeric field with an implied decimal point, and a signed one,
/// exactly as a group MOVE of the same bytes does: `0012345` in `PIC 9(5)V99` is 123.45, and
/// `001234N` in `PIC S9(5)V99` is −123.45. The view used to store the raw slice and read
/// 12345 and a positive number.
#[test]
fn a_view_decodes_decimals_and_signs_as_a_group_move_does() {
    let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. MAINP.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 BUF PIC X(14).
01 PLAIN.
   05 P-PRICE PIC 9(5)V99.
   05 P-DEBT PIC S9(5)V99.
PROCEDURE DIVISION.
    MOVE "0012345001234N" TO BUF.
    MOVE BUF TO PLAIN.
    COMPUTE P-PRICE = P-PRICE + 1.
    DISPLAY "MOVE PRICE+1 " P-PRICE.
    IF P-DEBT < 0 DISPLAY "MOVE DEBT NEGATIVE" END-IF.
    CALL "SUB" USING BUF.
    STOP RUN.
IDENTIFICATION DIVISION.
PROGRAM-ID. SUB IS COMMON PROGRAM.
DATA DIVISION.
LINKAGE SECTION.
01 VIEW.
   05 PRICE PIC 9(5)V99.
   05 DEBT PIC S9(5)V99.
PROCEDURE DIVISION USING VIEW.
    COMPUTE PRICE = PRICE + 1.
    DISPLAY "VIEW PRICE+1 " PRICE.
    IF DEBT < 0 DISPLAY "VIEW DEBT NEGATIVE" END-IF.
    GOBACK.
END PROGRAM SUB.
END PROGRAM MAINP.
"#;
    let out = run(src);
    println!("{out:#?}");
    assert!(out.iter().any(|l| l == "MOVE PRICE+1 0012445"), "{out:?}");
    assert!(out.iter().any(|l| l == "MOVE DEBT NEGATIVE"), "{out:?}");
    assert!(out.iter().any(|l| l == "VIEW PRICE+1 0012445"), "the view reads 123.45, as the MOVE does: {out:?}");
    assert!(out.iter().any(|l| l == "VIEW DEBT NEGATIVE"), "the view keeps the sign, as the MOVE does: {out:?}");
}
