// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Places where the interpreter departed from COBOL-85, found when the
//! WebAssembly generator of the 2.x.x line was held to it (`WASM-PARITY.md`
//! #7, #11, #12, #18). Each program here is the generator's own differential test,
//! so both engines answer to the same lines.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Parse `src`, run it, and return what it DISPLAYed, trimmed line by line.
fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect()
}

/// #7 — `ALL` over a table with `OCCURS … DEPENDING ON` references the
/// occurrences the counter says there are now, not the maximum: with 3 of 5
/// occurrences current, `SUM` is 10 + 20 + 30, not 961.
#[test]
fn all_over_a_variable_length_table_uses_its_current_count() {
    let src = r#"IDENTIFICATION DIVISION.
PROGRAM-ID. ODO-ALL.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 N PIC 9 VALUE 3.
01 T.
   05 E PIC 9(3) OCCURS 1 TO 5 TIMES DEPENDING ON N.
01 S PIC 9(4).
01 R PIC 9(4).
PROCEDURE DIVISION.
MAIN.
    MOVE 5 TO N.
    MOVE 10 TO E (1). MOVE 20 TO E (2). MOVE 30 TO E (3).
    MOVE 900 TO E (4). MOVE 1 TO E (5).
    MOVE 3 TO N.
    COMPUTE S = FUNCTION SUM (E (ALL)).
    DISPLAY "sum of 3: " S.
    COMPUTE R = FUNCTION MAX (E (ALL)).
    DISPLAY "max of 3: " R.
    COMPUTE R = FUNCTION MIN (E (ALL)).
    DISPLAY "min of 3: " R.
    MOVE 5 TO N.
    COMPUTE S = FUNCTION SUM (E (ALL)).
    DISPLAY "sum of 5: " S.
    COMPUTE R = FUNCTION MIN (E (ALL)).
    DISPLAY "min of 5: " R.
    STOP RUN.
"#;
    assert_eq!(
        run_capture(src),
        ["sum of 3: 0060", "max of 3: 0030", "min of 3: 0010", "sum of 5: 0961", "min of 5: 0001"]
    );
}

/// #12 — a literal passed `BY VALUE` reaches the called program as its value
/// (5 into a `PIC 9(2)` parameter, tripled, is 15 — it arrived as 0), and a
/// data item passed `BY VALUE` keeps working as it did.
#[test]
fn a_literal_passed_by_value_reaches_the_called_program() {
    let src = r#"IDENTIFICATION DIVISION.
PROGRAM-ID. MAIN-PROG.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WHO PIC X(8) VALUE "TWICE".
01 X PIC 9(4) VALUE 21.
01 Y PIC 9(4) VALUE 0.
PROCEDURE DIVISION.
MAIN.
    CALL WHO USING BY VALUE X RETURNING Y.
    DISPLAY "dynamic returning: " Y " x stays " X.
    MOVE "THRICE" TO WHO.
    CALL WHO USING BY VALUE 5 RETURNING Y.
    DISPLAY "other callee: " Y.
    CALL "TWICE" USING BY VALUE X RETURNING Y.
    DISPLAY "static returning: " Y.
    CALL "THRICE" USING BY CONTENT 7 RETURNING Y.
    DISPLAY "by content literal: " Y.
    STOP RUN.
END PROGRAM MAIN-PROG.

IDENTIFICATION DIVISION.
PROGRAM-ID. TWICE.
DATA DIVISION.
LINKAGE SECTION.
01 L-N PIC S9(4) COMP.
WORKING-STORAGE SECTION.
01 L-R PIC 9(4).
PROCEDURE DIVISION USING L-N RETURNING L-R.
GO-ON.
    COMPUTE L-R = L-N * 2.
    MOVE 0 TO L-N.
    GOBACK.
END PROGRAM TWICE.

IDENTIFICATION DIVISION.
PROGRAM-ID. THRICE.
DATA DIVISION.
LINKAGE SECTION.
01 L-N PIC 9(2).
WORKING-STORAGE SECTION.
01 L-R PIC 9(4).
PROCEDURE DIVISION USING L-N RETURNING L-R.
GO-ON.
    COMPUTE L-R = L-N * 3.
    GOBACK.
END PROGRAM THRICE.
"#;
    assert_eq!(
        run_capture(src),
        [
            "dynamic returning: 0042 x stays 0021",
            "other callee: 0015",
            "static returning: 0042",
            "by content literal: 0021",
        ]
    );
}

/// #11 — a program declared `IS INITIAL` starts every call in its initial
/// state (COBOL-85 6.8): its counter shows 1 on both calls, while an ordinary
/// program beside it keeps its value between calls (1, then 2).
#[test]
fn an_initial_program_starts_every_call_afresh() {
    let src = r#"IDENTIFICATION DIVISION.
PROGRAM-ID. MAIN-PROG.
PROCEDURE DIVISION.
MAIN.
    CALL "COUNTER".
    CALL "COUNTER".
    CALL "KEEPER".
    CALL "KEEPER".
    STOP RUN.
END PROGRAM MAIN-PROG.
IDENTIFICATION DIVISION.
PROGRAM-ID. COUNTER IS INITIAL PROGRAM.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 CALLS PIC 9 VALUE 0.
PROCEDURE DIVISION.
GO-ON.
    ADD 1 TO CALLS.
    DISPLAY "initial: " CALLS.
    GOBACK.
END PROGRAM COUNTER.
IDENTIFICATION DIVISION.
PROGRAM-ID. KEEPER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 CALLS PIC 9 VALUE 0.
PROCEDURE DIVISION.
GO-ON.
    ADD 1 TO CALLS.
    DISPLAY "kept: " CALLS.
    GOBACK.
END PROGRAM KEEPER.
"#;
    assert_eq!(run_capture(src), ["initial: 1", "initial: 1", "kept: 1", "kept: 2"]);
}

/// #18 — a division by zero inside `COMPUTE` is the size error condition, as
/// it is for `DIVIDE`: `ON SIZE ERROR` runs, `NOT ON SIZE ERROR` does not, and
/// the receiver keeps its value. The run used to stop at the statement.
#[test]
fn compute_division_by_zero_runs_on_size_error() {
    let src = r#"IDENTIFICATION DIVISION.
PROGRAM-ID. DIV-ZERO.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 A PIC 9(3) VALUE 10.
01 C PIC 9(3) VALUE 0.
01 R0 PIC 9(7) VALUE 9.
PROCEDURE DIVISION.
MAIN.
    COMPUTE R0 = A / C
        ON SIZE ERROR DISPLAY "SIZE " R0
        NOT ON SIZE ERROR DISPLAY "NO"
    END-COMPUTE.
    DISPLAY "AFTER " R0.
    COMPUTE R0 = (A + 1) * 2 / C
        NOT ON SIZE ERROR DISPLAY "NO"
    END-COMPUTE.
    DISPLAY "NOT-ONLY " R0.
    MOVE 5 TO C.
    COMPUTE R0 = A / C
        ON SIZE ERROR DISPLAY "SIZE"
        NOT ON SIZE ERROR DISPLAY "OK " R0
    END-COMPUTE.
    STOP RUN.
"#;
    assert_eq!(
        run_capture(src),
        ["SIZE 0000009", "AFTER 0000009", "NOT-ONLY 0000009", "OK 0000002"]
    );
}
