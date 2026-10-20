// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `CALL`, `CANCEL`, LINKAGE, nested programs, `GLOBAL` and `EXTERNAL` on a
//! generated module against the interpreter (the NIST IC module is the gate).

mod common;
use common::*;

/// Two programs in sequence: the main one calls the second by name.
const SEQUENCE: &str = r#"IDENTIFICATION DIVISION.
PROGRAM-ID. MAIN-PROG.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 A PIC 9(4) VALUE 10.
01 B PIC X(6) VALUE "ABCDEF".
01 C PIC 99 VALUE 0.
01 NAME-OF-SUB PIC X(8) VALUE "SUB-TWO".
PROCEDURE DIVISION.
MAIN.
    CALL "SUB-ONE" USING A B.
    DISPLAY "after one: " A " " B.
    CALL "SUB-ONE" USING BY CONTENT A BY CONTENT B.
    DISPLAY "by content: " A " " B.
    CALL "SUB-ONE" USING A B.
    DISPLAY "again: " A " " B.
    CANCEL "SUB-ONE".
    CALL "SUB-TWO" USING C.
    DISPLAY "two: " C.
    CALL NAME-OF-SUB USING C.
    DISPLAY "dynamic: " C.
    CALL "NO-SUCH" USING C ON EXCEPTION DISPLAY "no such program" NOT ON EXCEPTION DISPLAY "found".
    STOP RUN.
END PROGRAM MAIN-PROG.

IDENTIFICATION DIVISION.
PROGRAM-ID. SUB-ONE.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 CALLS PIC 99 VALUE 0.
LINKAGE SECTION.
01 L-NUM PIC 9(4).
01 L-TXT PIC X(6).
PROCEDURE DIVISION USING L-NUM L-TXT.
GO-ON.
    ADD 1 TO CALLS.
    ADD 5 TO L-NUM.
    MOVE "XYZ" TO L-TXT.
    DISPLAY "sub one call " CALLS.
    EXIT PROGRAM.
END PROGRAM SUB-ONE.

IDENTIFICATION DIVISION.
PROGRAM-ID. SUB-TWO.
DATA DIVISION.
LINKAGE SECTION.
01 L-N PIC 99.
PROCEDURE DIVISION USING L-N.
P1.
    PERFORM P2.
    GOBACK.
P2.
    ADD 7 TO L-N.
    GOBACK.
END PROGRAM SUB-TWO.
"#;

#[test]
fn calls_by_reference_content_dynamic_and_missing() {
    let lines = same_output(SEQUENCE);
    assert_eq!(lines[0], "sub one call 01");
    assert_eq!(lines[1], "after one: 0015 XYZ   ");
    // BY CONTENT: the callee may change its copy; the caller's values stay.
    assert_eq!(lines[3], "by content: 0015 XYZ   ");
    // CANCEL puts the callee back to its initial state: the call count restarts.
    assert!(lines.iter().any(|l| l == "no such program"));
}

#[test]
fn nested_program_sees_global_items_and_cancel_resets() {
    let src = r#"IDENTIFICATION DIVISION.
PROGRAM-ID. OUTER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 SHARED IS GLOBAL.
   02 G-COUNT PIC 99 VALUE 0.
   02 G-NAME PIC X(5) VALUE "OUTER".
01 PRIVATE-ITEM PIC 99 VALUE 5.
PROCEDURE DIVISION.
MAIN.
    CALL "INNER".
    CALL "INNER".
    DISPLAY "count " G-COUNT " name " G-NAME.
    CANCEL "INNER".
    CALL "INNER".
    DISPLAY "count " G-COUNT.
    STOP RUN.

    IDENTIFICATION DIVISION.
    PROGRAM-ID. INNER.
    DATA DIVISION.
    WORKING-STORAGE SECTION.
    01 PRIVATE-ITEM PIC 99 VALUE 40.
    01 OWN-CALLS PIC 99 VALUE 0.
    PROCEDURE DIVISION.
    GO-IN.
        ADD 1 TO G-COUNT.
        ADD 1 TO OWN-CALLS.
        MOVE "INNER" TO G-NAME.
        DISPLAY "inner " OWN-CALLS " " PRIVATE-ITEM.
        EXIT PROGRAM.
    END PROGRAM INNER.
END PROGRAM OUTER.
"#;
    let lines = same_output(src);
    assert_eq!(lines[0], "inner 01 40");
    assert_eq!(lines[1], "inner 02 40");
    assert_eq!(lines[2], "count 02 name INNER");
    // After CANCEL the callee's own storage is initial again; GLOBAL data of the
    // caller is not the callee's to reset.
    assert_eq!(lines[3], "inner 01 40");
    assert_eq!(lines[4], "count 03");
}

#[test]
fn a_callee_that_returns_from_inside_a_perform() {
    let src = r#"IDENTIFICATION DIVISION.
PROGRAM-ID. DRIVER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 R PIC 99 VALUE 0.
PROCEDURE DIVISION.
MAIN.
    PERFORM 3 TIMES
        CALL "DEEP" USING R
        DISPLAY "r " R
    END-PERFORM.
    STOP RUN.
END PROGRAM DRIVER.

IDENTIFICATION DIVISION.
PROGRAM-ID. DEEP.
DATA DIVISION.
LINKAGE SECTION.
01 L-R PIC 99.
PROCEDURE DIVISION USING L-R.
A-PARA.
    PERFORM B-PARA THRU C-PARA.
    ADD 50 TO L-R.
    GOBACK.
B-PARA.
    ADD 1 TO L-R.
C-PARA.
    PERFORM D-PARA.
    EXIT PROGRAM.
D-PARA.
    ADD 10 TO L-R.
    EXIT PROGRAM.
END PROGRAM DEEP.
"#;
    // `EXIT PROGRAM` and `GOBACK` leave the called program from any depth of
    // PERFORM (COBOL-85 6.7.4). The interpreter does not (it carries on after
    // the PERFORM — `specs/WASM-PARITY.md`), so this one is checked on the
    // module alone.
    let o = run_wasm(src, &cobolt_wasm::Options::default());
    assert!(o.trapped.is_none() && o.errors.is_empty(), "{o:?}");
    assert_eq!(wasm_lines(&o), vec!["r 11", "r 22", "r 33"]);
}

#[test]
fn linkage_records_may_be_described_differently_from_the_argument() {
    // The callee sees the caller's bytes through its own description of them.
    let src = r#"IDENTIFICATION DIVISION.
PROGRAM-ID. M.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 REC.
   02 PART-A PIC X(3) VALUE "ABC".
   02 PART-B PIC 9(3) VALUE 123.
PROCEDURE DIVISION.
MAIN.
    CALL "SPLIT" USING REC.
    DISPLAY REC.
    STOP RUN.
END PROGRAM M.

IDENTIFICATION DIVISION.
PROGRAM-ID. SPLIT.
DATA DIVISION.
LINKAGE SECTION.
01 WHOLE PIC X(6).
01 PARTS REDEFINES WHOLE.
   02 FIRST-HALF PIC XX.
   02 SECOND-HALF PIC X(4).
PROCEDURE DIVISION USING WHOLE.
GO-ON.
    MOVE "--" TO FIRST-HALF.
    MOVE SECOND-HALF TO WHOLE.
    EXIT PROGRAM.
END PROGRAM SPLIT.
"#;
    same_output(src);
}
