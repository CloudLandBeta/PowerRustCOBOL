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

/// Reading *through* a `REDEFINES` of a LINKAGE item must see the caller's
/// bytes, not the redefining item's own untouched slot.
///
/// CCVS85 **IC237A** / **IC237A-1**, reduced. The subprogram declares
/// `01 L-A PIC 9.` and `01 L-A1 REDEFINES L-A PIC 9.` in its LINKAGE SECTION
/// and does `MOVE L-A1 TO L-C`; the caller then checks `WS-C = WS-A`.
///
/// The redefinition refresh is **write-driven**, and the caller's write
/// happened before the CALL installed the parameter alias — so nothing had ever
/// populated the redefining description and `L-A` read 1 while `L-A1` read 0.
const READ_THROUGH_A_LINKAGE_REDEFINES: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RRMAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-A PIC 9 VALUE 1.
       01  WS-C PIC 9 VALUE 5.
       PROCEDURE DIVISION.
       MAIN.
           CALL "RRSUB" USING WS-A WS-C
           DISPLAY "C=[" WS-C "]"
           DISPLAY "A=[" WS-A "]"
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RRSUB.
       DATA DIVISION.
       LINKAGE SECTION.
       01  L-A                PIC 9.
       01  L-A1 REDEFINES L-A PIC 9.
       01  L-C                PIC 9.
       PROCEDURE DIVISION USING L-A L-C.
       S-MAIN.
           MOVE L-A1 TO L-C
           EXIT PROGRAM.
       END PROGRAM RRSUB.
       END PROGRAM RRMAIN.
"#;

fn bracketed(out: &[String], name: &str) -> String {
    out.iter()
        .find_map(|l| {
            l.strip_prefix(&format!("{name}=["))
                .and_then(|r| r.strip_suffix(']'))
                .map(str::to_owned)
        })
        .unwrap_or_else(|| panic!("the program displays {name}=[…]; got {out:?}"))
}

#[test]
fn reading_through_a_linkage_redefines_sees_the_callers_data() {
    let out = run_capture(READ_THROUGH_A_LINKAGE_REDEFINES);
    assert_eq!(
        bracketed(&out, "C"),
        "1",
        "L-A1 redefines L-A, which is bound to WS-A holding 1, so MOVE L-A1 TO \
         L-C must carry 1 back into WS-C; 0 means the redefining description \
         was never populated from the caller's bytes"
    );
}

/// Pairs with the test above. Priming a redefinition from the caller's storage
/// must not write *back* into it: the subprogram never assigns to `L-A`, so the
/// caller's `WS-A` has to come through the call untouched. A prime that
/// resolved the wrong side of the alias would corrupt the source it read.
#[test]
fn priming_a_linkage_redefines_does_not_write_back_to_the_caller() {
    let out = run_capture(READ_THROUGH_A_LINKAGE_REDEFINES);
    assert_eq!(
        bracketed(&out, "A"),
        "1",
        "nothing in RRSUB assigns to L-A, so WS-A must be unchanged"
    );
}

/// An unnamed `FILLER` in a nested program's LINKAGE occupies bytes, so every
/// item after it must sit at the right offset.
///
/// CCVS85 **IC107** LINK-TEST-06, reduced. It declares
///
/// ```text
/// 01  GROUP-2.
///     02    GROUP-21.
///         06 DN2 PIC X OCCURS 10 TIMES.
///     02     GROUP-2-1 REDEFINES GROUP-21.
///         03  FILLER  PICTURE X(7).
///         03  DN3     PICTURE XXX.
/// ```
///
/// and writes `DN3`, which must land over the table's last three bytes. A
/// nested program's items reach the shared environment through
/// `push_local_scope`, whose snapshot came from `iter()` — and `iter()` hides
/// FILLER keys, because it exists for showing storage rather than copying it.
/// The seven-byte FILLER was therefore absent, reported width 0, and the write
/// landed at the front of the table: `0ABC456789` instead of `0123456ABC`.
///
/// The same description in WORKING-STORAGE was always correct, which is what
/// localised this to the nested-program path.
const FILLER_IN_A_LINKAGE_REDEFINES: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RMAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  GROUP-2.
           02  GROUP-21.
               06  DN2 PIC X OCCURS 10 TIMES.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "0123456789" TO GROUP-21
           CALL "RSUB" USING GROUP-2
           DISPLAY "G21=[" GROUP-21 "]"
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RSUB.
       DATA DIVISION.
       LINKAGE SECTION.
       01  GRP-2.
           02  GROUP-21X.
               06  DN2X PIC X OCCURS 10 TIMES.
           02  GROUP-2-1 REDEFINES GROUP-21X.
               03  FILLER  PIC X(7).
               03  DN3     PIC XXX.
       PROCEDURE DIVISION USING GRP-2.
       S-MAIN.
           MOVE "ABC" TO DN3
           EXIT PROGRAM.
       END PROGRAM RSUB.
       END PROGRAM RMAIN.
"#;

#[test]
fn a_filler_in_a_nested_programs_linkage_still_occupies_its_bytes() {
    let out = run_capture(FILLER_IN_A_LINKAGE_REDEFINES);
    let g21 = out
        .iter()
        .find_map(|l| {
            l.strip_prefix("G21=[")
                .and_then(|r| r.strip_suffix(']'))
                .map(str::to_owned)
        })
        .expect("the caller displays G21=[…]");
    assert_eq!(
        g21, "0123456ABC",
        "DN3 follows a seven-byte FILLER, so \"ABC\" must overlay the table's \
         last three bytes; \"0ABC456789\" means the FILLER was missing from the \
         nested program's snapshot and reported width 0"
    );
}

/// IC106A's LINK-TEST-06, **faithfully**: the caller's parameter is a table and
/// the callee describes it as a group with a `REDEFINES` laid over it.
///
/// This is the shape the reductions above kept missing. The caller declares
/// `01 TABLE-2. 02 DN2 PIC X OCCURS 10.` — one subordinate, a table — while
/// IC107A's LINKAGE declares `01 GROUP-2. 02 GROUP-21. 06 DN2 PIC X OCCURS 10.
/// 02 GROUP-2-1 REDEFINES GROUP-21.` — a *group* where the caller has a table.
/// Pairing the two by position therefore aliases `GROUP-21` onto the bare
/// `DN2`, and a table's base name is not a slot: every reader addresses
/// `DN2(n)`. The overlay's ten bytes landed on a key nothing reads and the
/// caller saw its last three positions blank.
const REDEFINES_OVER_A_LINKAGE_TABLE: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TBMAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TABLE-2.
           02  DN2 PIC X OCCURS 10 TIMES.
       PROCEDURE DIVISION.
       MAIN.
           MOVE SPACE TO TABLE-2
           CALL "TBSUB" USING TABLE-2
           DISPLAY "T=[" DN2 (8) DN2 (9) DN2 (10) "]"
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TBSUB.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       77  AL-CON PIC XXX VALUE "XYZ".
       LINKAGE SECTION.
       01  GROUP-2.
           02  GROUP-21.
               06  DN2 PIC X OCCURS 10 TIMES.
           02  GROUP-2-1 REDEFINES GROUP-21.
               03  FILLER  PIC X(7).
               03  DN3     PIC XXX.
       PROCEDURE DIVISION USING GROUP-2.
       S-MAIN.
           MOVE AL-CON TO DN3
           EXIT PROGRAM.
       END PROGRAM TBSUB.
       END PROGRAM TBMAIN.
"#;

#[test]
fn a_redefines_over_a_linkage_table_reaches_the_callers_occurrences() {
    let out = run_capture(REDEFINES_OVER_A_LINKAGE_TABLE);
    let t = out
        .iter()
        .find_map(|l| {
            l.strip_prefix("T=[")
                .and_then(|r| r.strip_suffix(']'))
                .map(str::to_owned)
        })
        .expect("the caller displays T=[…]");
    assert_eq!(
        t, "XYZ",
        "the callee wrote \"XYZ\" through a REDEFINES of the group it lays over \
         the caller's table, so occurrences 8, 9 and 10 must carry it; \
         \"   \" means the alias landed on the table's unsubscripted base name, \
         which nothing reads"
    );
}
