// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A `REDEFINES` in a callee's LINKAGE SECTION describes the caller's bytes.
//!
//! Motivated by CCVS85 **IC106A**, whose LINK-TEST-06 says what it checks:
//! "THIS TEST VERIFIES THAT DATA WAS MOVED TO A REDEFINED ITEM IN THE LINKAGE
//! SECTION OF IC107." Its three assertions read **blank** where `X`, `Y` and
//! `Z` are expected — empty rather than wrong, which is what storage that was
//! never bound looks like.
//!
//! A redefinition in LINKAGE is not storage of its own any more than one in
//! WORKING-STORAGE is. It is a second description of a parameter's bytes, so a
//! write through it has to land in the caller's data.
//!
//! Two resolution paths meet here and it is their composition that matters:
//! `storage_key` follows `redefine_aliases`, `resolve_name` follows the
//! `addr_aliases` that bind a parameter, and a name written through a LINKAGE
//! redefinition needs BOTH — first to its redefinition target, then through
//! that target's binding to the caller. The same two-paths-that-must-compose
//! shape produced the FILE STATUS defect fixed at 1.62.89.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

/// The caller passes a three-byte group; the callee declares a redefinition of
/// it and writes through that, never touching the parameter by its own name.
const REDEFINES_IN_LINKAGE: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RDMAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  HOLDER.
           02  SLOT PIC X(3).
       PROCEDURE DIVISION.
       MAIN.
           MOVE "..." TO SLOT
           CALL "RDSUB" USING HOLDER
           DISPLAY "SLOT=" SLOT
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RDSUB.
       DATA DIVISION.
       LINKAGE SECTION.
       01  HOLDER.
           02  SLOT PIC X(3).
           02  SLOT-R REDEFINES SLOT.
               03  C1 PIC X.
               03  C2 PIC X.
               03  C3 PIC X.
       PROCEDURE DIVISION USING HOLDER.
       SUB-MAIN.
           MOVE "X" TO C1
           MOVE "Y" TO C2
           MOVE "Z" TO C3
           EXIT PROGRAM.
       END PROGRAM RDSUB.
       END PROGRAM RDMAIN.
"#;

/// A nested program's REDEFINES over its OWN storage — no LINKAGE, no
/// parameter, nothing to do with binding.
///
/// If this fails too, the defect is not about LINKAGE at all: it is that a
/// nested program's redefinitions are never established, and that reaches well
/// past the conformance suite. Every RAD event handler is a nested program.
const REDEFINES_IN_A_NESTED_PROGRAM: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. NSTMAIN.
       PROCEDURE DIVISION.
       MAIN.
           CALL "NSTSUB"
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. NSTSUB.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  OWNED.
           02  WHOLE PIC X(3).
           02  PARTS REDEFINES WHOLE.
               03  P1 PIC X.
               03  P2 PIC X.
               03  P3 PIC X.
       PROCEDURE DIVISION.
       SUB-MAIN.
           MOVE "P" TO P1
           MOVE "Q" TO P2
           MOVE "R" TO P3
           DISPLAY "WHOLE=" WHOLE
           EXIT PROGRAM.
       END PROGRAM NSTSUB.
       END PROGRAM NSTMAIN.
"#;

/// The same redefinition, in a nested program's own WORKING-STORAGE.
///
/// `push_local_scope` inserts a nested program's items into `store` and
/// `symbols` and does nothing else — it never builds the redefinition classes
/// that make two descriptions of one area share it. So this is expected to fail
/// alongside the LINKAGE case, and if it does the cause is one thing rather
/// than two.
#[test]
fn a_nested_programs_own_redefines_shares_its_storage() {
    let out = run_capture(REDEFINES_IN_A_NESTED_PROGRAM);
    let whole = out
        .iter()
        .find_map(|l| l.strip_prefix("WHOLE="))
        .expect("the nested program displays WHOLE=");
    assert_eq!(
        whole, "PQR",
        "PARTS redefines WHOLE, so writing its three bytes must be readable \
         through WHOLE; {whole:?} means the nested program's redefinition was \
         never established"
    );
}

/// IC106A's LINK-TEST-06, reduced. The callee writes `X`, `Y`, `Z` through a
/// redefinition of its parameter and the caller must see them.
///
/// A **blank** result is the signature of the defect: the redefinition bound to
/// nothing, so the write went to a slot no one reads. A result of `...` would
/// mean something different — bound, but not written through.
#[test]
fn a_callee_writing_through_a_linkage_redefines_reaches_the_caller() {
    let out = run_capture(REDEFINES_IN_LINKAGE);
    let slot = out
        .iter()
        .find_map(|l| l.strip_prefix("SLOT="))
        .expect("the program displays SLOT=");
    assert_eq!(
        slot, "XYZ",
        "a LINKAGE REDEFINES describes the caller's bytes, so writing its \
         parts must reach them; {slot:?} means the redefinition bound to \
         storage of its own"
    );
}
