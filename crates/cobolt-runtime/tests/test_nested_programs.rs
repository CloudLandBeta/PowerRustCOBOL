// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Integration tests for COBOL-85 nested program dispatch (Phase 4).
//!
//! Each test parses a complete COBOL source that contains one or more nested
//! programs and verifies the expected runtime behaviour through the
//! `Interpreter`.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Tokenize + parse source and return an `Interpreter`; panics on any error.
fn interp(src: &str) -> Interpreter {
    let tokens = tokenize(src, SourceFormat::Free);
    let result = parse(tokens);
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "Parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program returned");
    Interpreter::new(program)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// A CALL to a nested program's PROGRAM-ID runs its statements and returns
/// normally (GOBACK is not propagated to the outer program as an error).
#[test]
fn call_nested_program_runs_and_returns() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-RESULT  PIC 9(3) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           CALL "SET-RESULT".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SET-RESULT.
       PROCEDURE DIVISION.
           MOVE 42 TO WS-RESULT.
           GOBACK.
       END PROGRAM SET-RESULT.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    let val = i.env.get_i64("WS-RESULT").unwrap_or(0);
    assert_eq!(val, 42, "nested program should have set WS-RESULT to 42");
}

/// The nested program's own local WORKING-STORAGE items are visible during
/// execution but do not leak back into the outer program's environment after
/// the call returns.
#[test]
fn nested_local_ws_is_removed_after_goback() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-FLAG  PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           CALL "INNER".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. INNER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 LOCAL-ITEM  PIC X(10) VALUE "HELLO".
       PROCEDURE DIVISION.
           MOVE 1 TO WS-FLAG.
           GOBACK.
       END PROGRAM INNER.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");

    // WS-FLAG should have been set by the nested program (it's in outer's env).
    assert_eq!(i.env.get_i64("WS-FLAG").unwrap_or(0), 1);

    // LOCAL-ITEM was the nested program's own WS; it must not persist.
    assert!(
        !i.env.contains("LOCAL-ITEM"),
        "LOCAL-ITEM must be removed from env after GOBACK"
    );
}

/// GLOBAL data items in the outer program's WORKING-STORAGE are visible to
/// nested programs and mutations are seen back in the outer env.
#[test]
fn global_items_shared_with_nested_program() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-COUNTER  PIC 9(5) VALUE 10 GLOBAL.
       PROCEDURE DIVISION.
       MAIN.
           CALL "BUMP-COUNTER".
           CALL "BUMP-COUNTER".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUMP-COUNTER.
       PROCEDURE DIVISION.
           ADD 1 TO WS-COUNTER.
           GOBACK.
       END PROGRAM BUMP-COUNTER.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    assert_eq!(
        i.env.get_i64("WS-COUNTER").unwrap_or(0),
        12,
        "WS-COUNTER should be 10 + 2 calls"
    );
}

/// Calling a nested program that itself contains paragraphs — only the nested
/// program's own para_map is used for GO TO inside that program.
#[test]
fn nested_program_internal_goto() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-VAL  PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           CALL "NESTED".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. NESTED.
       PROCEDURE DIVISION.
       STEP-A.
           MOVE 1 TO WS-VAL.
           GO TO STEP-B.
       STEP-B.
           ADD 1 TO WS-VAL.
           GOBACK.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    assert_eq!(i.env.get_i64("WS-VAL").unwrap_or(0), 2);
}

/// Multiple nested programs — each CALL dispatches to the correct one.
#[test]
fn multiple_nested_programs_dispatch_independently() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A  PIC 9 VALUE 0.
       01 WS-B  PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           CALL "SET-A".
           CALL "SET-B".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SET-A.
       PROCEDURE DIVISION.
           MOVE 1 TO WS-A.
           GOBACK.
       END PROGRAM SET-A.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SET-B.
       PROCEDURE DIVISION.
           MOVE 2 TO WS-B.
           GOBACK.
       END PROGRAM SET-B.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    assert_eq!(i.env.get_i64("WS-A").unwrap_or(0), 1);
    assert_eq!(i.env.get_i64("WS-B").unwrap_or(0), 2);
}

/// PERFORM inside a nested program reaches the nested program's OWN
/// paragraphs. This is the documented handler pattern ("use PERFORM for
/// paragraphs you declared yourself, inside the body you are writing") and
/// the exact locality the 1.55.6 semantic analyzer enforces at compile time.
#[test]
fn nested_program_performs_its_own_paragraph() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-VAL  PIC 9(3) VALUE 0 GLOBAL.
       PROCEDURE DIVISION.
       MAIN.
           CALL "HANDLER".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. HANDLER.
       PROCEDURE DIVISION.
       ENTRY-POINT.
           PERFORM LOCAL-STEP.
           PERFORM LOCAL-STEP.
           GOBACK.
       LOCAL-STEP.
           ADD 7 TO WS-VAL.
       END PROGRAM HANDLER.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    assert_eq!(
        i.env.get_i64("WS-VAL").unwrap_or(0),
        14,
        "PERFORM LOCAL-STEP inside HANDLER must run HANDLER's own paragraph"
    );
}

/// When the outer program and a nested program both declare a paragraph with
/// the same name, PERFORM inside the nested program runs the NESTED one —
/// procedure names are strictly program-local in COBOL-85.
#[test]
fn nested_perform_prefers_own_paragraph_over_outer_same_name() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-WHO  PIC X(6) VALUE SPACES GLOBAL.
       PROCEDURE DIVISION.
       MAIN.
           CALL "HANDLER".
           STOP RUN.
       SET-WHO.
           MOVE "OUTER" TO WS-WHO.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. HANDLER.
       PROCEDURE DIVISION.
       ENTRY-POINT.
           PERFORM SET-WHO.
           GOBACK.
       SET-WHO.
           MOVE "INNER" TO WS-WHO.
       END PROGRAM HANDLER.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    assert_eq!(
        i.env.get_string("WS-WHO").unwrap_or_default().trim(),
        "INNER",
        "PERFORM SET-WHO inside HANDLER must run HANDLER's own SET-WHO"
    );
}

/// A paragraph of the CONTAINING program is not reachable from a nested one.
/// COBOL-85 has no GLOBAL for procedure names, and since 1.55.6 the semantic
/// analyzer reports such a `PERFORM` as an error — the runtime must agree
/// rather than quietly resolving it against the outer program.
#[test]
fn nested_perform_cannot_reach_a_containing_program_paragraph() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-HIT  PIC 9 VALUE 0 GLOBAL.
       PROCEDURE DIVISION.
       MAIN.
           CALL "HANDLER".
           STOP RUN.
       OUTER-ONLY.
           MOVE 7 TO WS-HIT.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. HANDLER.
       PROCEDURE DIVISION.
       ENTRY-POINT.
           PERFORM OUTER-ONLY.
           GOBACK.
       END PROGRAM HANDLER.
    "#;

    let mut i = interp(src);
    let err = i.run().expect_err("cross-program PERFORM must not resolve");
    assert!(
        format!("{err:?}").contains("OUTER-ONLY"),
        "expected an undefined-procedure error for OUTER-ONLY, got {err:?}"
    );
    assert_eq!(
        i.env.get_i64("WS-HIT").unwrap_or(-1),
        0,
        "the containing program's paragraph must never have run"
    );
}

/// `GOBACK` returns from the nested program. It is a STATEMENT — a lone
/// `GOBACK.` used to lex as an identifier and be taken for a paragraph header,
/// which silently let control fall through into whatever the handler declared
/// after it.
#[test]
fn goback_returns_and_does_not_fall_through_to_trailing_paragraphs() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-FELL  PIC 9 VALUE 0 GLOBAL.
       01 WS-RAN   PIC 9 VALUE 0 GLOBAL.
       PROCEDURE DIVISION.
       MAIN.
           CALL "HANDLER".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. HANDLER.
       PROCEDURE DIVISION.
       ENTRY-POINT.
           MOVE 1 TO WS-RAN.
           GOBACK.
       TRAILING-PARA.
           MOVE 1 TO WS-FELL.
       END PROGRAM HANDLER.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    assert_eq!(i.env.get_i64("WS-RAN").unwrap_or(0), 1, "handler must run");
    assert_eq!(
        i.env.get_i64("WS-FELL").unwrap_or(-1),
        0,
        "GOBACK must return instead of falling through to TRAILING-PARA"
    );
}

/// A nested program with no END PROGRAM terminator (last nested program in the
/// file) is still registered and callable.
#[test]
fn nested_program_without_end_program_terminator() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-OK  PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           CALL "NO-TERM".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. NO-TERM.
       PROCEDURE DIVISION.
           MOVE 9 TO WS-OK.
           GOBACK.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    assert_eq!(i.env.get_i64("WS-OK").unwrap_or(0), 9);
}

// ── CALL … USING parameter binding ───────────────────────────────────────────

/// **BY REFERENCE binds the caller's storage, not a copy of it** — and the
/// parameter's own name is irrelevant to which storage that is.
///
/// This is IC201A's `CALL-TEST-02` in miniature. The third argument is `DN1`,
/// the callee calls its third parameter `DN3`, and the caller has an unrelated
/// `DN3` of its own. Binding by name wrote the caller's `DN3`, which the suite
/// reports as "DN3 VALUE CHANGED BY CALL".
#[test]
fn a_parameter_binds_to_its_argument_not_to_its_own_name() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 DN1 PIC S9(4) VALUE 1.
       01 DN2 PIC S9(4) VALUE 0.
       01 DN3 PIC S9(4) VALUE 0.
       01 DN4 PIC S9(4) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           CALL "CALLEE" USING DN1, DN2, DN1, DN4.
           STOP RUN.
       END PROGRAM CALLER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLEE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS1 PIC S9(4) VALUE 0.
       LINKAGE SECTION.
       01 DN1 PIC S9(4).
       01 DN2 PIC S9(4).
       01 DN3 PIC S9(4).
       01 DN4 PIC S9(4).
       PROCEDURE DIVISION USING DN1, DN2, DN3, DN4.
       MAIN-2.
           MOVE DN1 TO WS1.
           ADD 1 TO WS1.
           MOVE WS1 TO DN3.
           MOVE 5 TO DN4.
           EXIT PROGRAM.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    // Parameter three is bound to the caller's DN1, so writing it writes DN1.
    assert_eq!(i.env.get_i64("DN1").unwrap_or(-1), 2, "DN1");
    assert_eq!(i.env.get_i64("DN2").unwrap_or(-1), 0, "DN2 was not passed");
    // The caller's own DN3 was never an argument and must be untouched.
    assert_eq!(i.env.get_i64("DN3").unwrap_or(-1), 0, "DN3 changed by call");
    assert_eq!(i.env.get_i64("DN4").unwrap_or(-1), 5, "DN4");
}

/// A group parameter's fields are paired with the argument's **by position**,
/// because the two programs need not use the same names for them.
///
/// IC204A declares `SUB-TABLE-1` over `SUB-DN2 / SUB-DN3 / SUB-DN4` while its
/// caller passes `TABLE-1` over `DN2 / DN3 / DN4`. Same bytes, different names
/// — matching by name reaches nothing, and the callee wrote its own LINKAGE
/// slots while the caller saw no change at all (IC203A's `DN2 INCORRECT`).
#[test]
fn a_group_parameters_fields_are_paired_by_position() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 TABLE-1.
          05 DN2 PIC XXX   VALUE "AAA".
          05 DN3 PIC 99    VALUE 7.
          05 DN4 PIC X(5)  VALUE SPACE.
       PROCEDURE DIVISION.
       MAIN.
           CALL "CALLEE" USING TABLE-1.
           STOP RUN.
       END PROGRAM CALLER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLEE.
       DATA DIVISION.
       LINKAGE SECTION.
       01 SUB-TABLE-1.
          05 SUB-DN2 PIC XXX.
          05 SUB-DN3 PIC 99.
          05 SUB-DN4 PIC X(5).
       PROCEDURE DIVISION USING SUB-TABLE-1.
       MAIN-2.
           MOVE "YES" TO SUB-DN2.
           ADD 1 TO SUB-DN3.
           MOVE "EQUAL" TO SUB-DN4.
           EXIT PROGRAM.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    assert_eq!(i.env.get_string("DN2").unwrap_or_default(), "YES", "DN2");
    assert_eq!(i.env.get_i64("DN3").unwrap_or(-1), 8, "DN3");
    assert_eq!(i.env.get_string("DN4").unwrap_or_default(), "EQUAL", "DN4");
}

/// A callee reads the **fields of a record it was handed** by name, so a group
/// parameter's subordinate items must keep resolving to the caller's.
///
/// This is the case that broke the first attempt at the fix: shadowing every
/// name the callee declares gave those children fresh slots holding their
/// initial values.
#[test]
fn a_group_parameters_fields_reach_the_callers_record() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 GRP-01.
          05 FLD-A PIC S9(4) VALUE 7.
          05 FLD-B PIC S9(4) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           CALL "CALLEE" USING GRP-01.
           STOP RUN.
       END PROGRAM CALLER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLEE.
       DATA DIVISION.
       LINKAGE SECTION.
       01 GRP-01.
          05 FLD-A PIC S9(4).
          05 FLD-B PIC S9(4).
       PROCEDURE DIVISION USING GRP-01.
       MAIN-2.
           ADD 1 TO FLD-A.
           MOVE FLD-A TO FLD-B.
           EXIT PROGRAM.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    assert_eq!(i.env.get_i64("FLD-A").unwrap_or(-1), 8, "FLD-A");
    assert_eq!(i.env.get_i64("FLD-B").unwrap_or(-1), 8, "FLD-B");
}

/// The alias is released when the call returns: a later write to the callee's
/// parameter name must not reach the caller's argument.
#[test]
fn a_parameter_alias_does_not_outlive_its_call() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 DN1 PIC S9(4) VALUE 1.
       01 DN9 PIC S9(4) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           CALL "CALLEE" USING DN1.
           MOVE 99 TO DN9.
           STOP RUN.
       END PROGRAM CALLER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLEE.
       DATA DIVISION.
       LINKAGE SECTION.
       01 DN9 PIC S9(4).
       PROCEDURE DIVISION USING DN9.
       MAIN-2.
           MOVE 4 TO DN9.
           EXIT PROGRAM.
    "#;

    let mut i = interp(src);
    i.run().expect("run failed");
    // Inside the call, DN9 was DN1.
    assert_eq!(i.env.get_i64("DN1").unwrap_or(-1), 4, "DN1");
    // After it, the caller's own DN9 is its own again.
    assert_eq!(i.env.get_i64("DN9").unwrap_or(-1), 99, "DN9 after the call");
}
