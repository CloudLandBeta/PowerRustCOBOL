// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The generator gaps of `specs/WASM-PARITY.md` closed by WASM-010 T13: #7
//! (`ALL` over an `OCCURS … DEPENDING ON` table), #9 (`RETURNING` on a dynamic
//! `CALL`, `BY VALUE`) and #10 (`INITIAL`, `COMMON`), one differential test each.

mod common;
use common::*;

/// #7: `FUNCTION SUM/MAX/MIN (T (ALL))` count the table's current occurrences.
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
    // COBOL-85: ALL over a table with OCCURS DEPENDING ON references the
    // occurrences the counter says there are now. The module does; the
    // interpreter expands over the maximum (WASM-PARITY #7, a 1.x.x defect).
    let want = ["sum of 3: 0060", "max of 3: 0030", "min of 3: 0010", "sum of 5: 0961", "min of 5: 0001"];
    for multi in [true, false] {
        let o = run_wasm(src, &cobolt_wasm::Options { multi_memory: multi, ..cobolt_wasm::Options::default() });
        assert!(o.trapped.is_none() && o.errors.is_empty(), "{o:?}");
        assert_eq!(wasm_lines(&o), want, "multi={multi}");
    }
    let interp = run_interpreter(src);
    let differ = interp.iter().zip(want.iter()).filter(|(a, b)| a != b).count();
    eprintln!("parity #7: the module follows the current count ({} lines); the interpreter differs on {differ}: {interp:?}", want.len());
}

/// #9: `RETURNING` on a `CALL identifier` reaches the caller; `BY VALUE`
/// passes a copy in the callee's own description, as the interpreter does.
#[test]
fn returning_on_a_dynamic_call_and_by_value() {
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
    let want = ["dynamic returning: 0042 x stays 0021", "other callee: 0015", "static returning: 0042"];
    for multi in [true, false] {
        let o = run_wasm(src, &cobolt_wasm::Options { multi_memory: multi, ..cobolt_wasm::Options::default() });
        assert!(o.trapped.is_none() && o.errors.is_empty(), "{o:?}");
        assert_eq!(wasm_lines(&o), want, "multi={multi}");
    }
    // The interpreter agrees on the items; a literal passed BY VALUE reaches its
    // callee as zero there (WASM-PARITY #12, a 1.x.x defect).
    let interp = run_interpreter(src);
    assert_eq!((interp[0].as_str(), interp[2].as_str()), (want[0], want[2]));
    eprintln!("parity #9: the module gives {want:?}; the interpreter {interp:?}");
}

/// #10: a `COMMON` program is called by its sibling (both engines agree); an
/// `INITIAL` program starts every call in its initial state (COBOL-85 6.8).
/// The interpreter ignores `INITIAL`: that is a 1.x.x defect, recorded in
/// `WASM-PARITY.md`, and the module follows the standard.
#[test]
fn common_and_initial_programs() {
    let common_src = r#"IDENTIFICATION DIVISION.
PROGRAM-ID. OUTER.
PROCEDURE DIVISION.
MAIN.
    CALL "FIRST-ONE".
    STOP RUN.
IDENTIFICATION DIVISION.
PROGRAM-ID. FIRST-ONE.
PROCEDURE DIVISION.
GO-ON.
    DISPLAY "first".
    CALL "HELPER".
    GOBACK.
END PROGRAM FIRST-ONE.
IDENTIFICATION DIVISION.
PROGRAM-ID. HELPER IS COMMON PROGRAM.
PROCEDURE DIVISION.
GO-ON.
    DISPLAY "helper, called by its sibling".
    GOBACK.
END PROGRAM HELPER.
END PROGRAM OUTER.
"#;
    let p = parse(common_src);
    assert!(p.nested_programs.iter().any(|n| n.identification.program_id == "HELPER" && n.is_common), "COMMON is kept in the AST");
    let out = same_output(common_src);
    assert_eq!(out, ["first", "helper, called by its sibling"]);

    let initial_src = r#"IDENTIFICATION DIVISION.
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
    let p = parse(initial_src);
    assert!(p.nested_programs.iter().any(|n| n.identification.program_id == "COUNTER" && n.is_initial), "INITIAL is kept in the AST");
    for multi in [true, false] {
        let o = run_wasm(initial_src, &cobolt_wasm::Options { multi_memory: multi, ..cobolt_wasm::Options::default() });
        assert!(o.trapped.is_none() && o.errors.is_empty(), "{o:?}");
        assert_eq!(wasm_lines(&o), ["initial: 1", "initial: 1", "kept: 1", "kept: 2"], "multi={multi}");
    }
    let interp = run_interpreter(initial_src);
    eprintln!("parity #10: COMMON equal on both engines; INITIAL on the module 1,1 — the interpreter prints {interp:?} (WASM-PARITY #11)");
}
