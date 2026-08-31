// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! An 88-level over a LINKAGE item tests the CALLER's data.
//!
//! Motivated by CCVS85 **IC207A**, whose LINK-TEST-03 says what it checks:
//! "THIS TEST VERIFIES THAT THE CONDITION NAMES DEFINED IN THE LINKAGE SECTION
//! OF THE SUBPROGRAM WERE PROCESSED CORRECTLY."
//!
//! Two things were wrong, and the first hid the second:
//!
//! Both are fixed. 1. A nested program's 88-levels were never registered. `cond_names` is built
//!    when an environment is constructed from a DATA DIVISION, and a nested
//!    program's items reach the shared environment through `push_local_scope`,
//!    which carried values and symbols and nothing else. `IF 88-name` then
//!    found no condition-name and fell back to "holds something non-zero" —
//!    false for a LINKAGE slot the callee has not written, whatever the caller
//!    put there.
//! 2. Once registered, the host was still read by raw key. A LINKAGE host IS
//!    the caller's storage, so it has to be reached through the parameter
//!    alias — the same composition failure as the FILE STATUS defect fixed at
//!    1.62.89.

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

/// The text between the brackets of a `DISPLAY "NAME=[" ITEM "]"` line.
///
/// Both delimiters have to come off. Stripping only the `NAME=[` prefix left
/// the closing bracket on the value, so `trim().is_empty()` was false for an
/// untouched item and the negative test below could not pass however the
/// condition evaluated — it failed identically whether the answer was right or
/// wrong, which is worth remembering before trusting a paired test.
fn field(out: &[String], name: &str) -> Option<String> {
    out.iter().find_map(|l| {
        l.strip_prefix(&format!("{name}=["))
            .and_then(|r| r.strip_suffix(']'))
            .map(str::to_owned)
    })
}

/// The caller fills a table; the callee declares the same table in LINKAGE with
/// an 88 on the item and tests two occurrences — one that holds the value and
/// one that does not.
const LINKAGE_88: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TBL.
           02  ITM PIC X OCCURS 3 TIMES.
       01  HIT  PIC X(6) VALUE SPACES.
       01  MISS PIC X(6) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "A" TO ITM (1)
           MOVE "B" TO ITM (2)
           MOVE "A" TO ITM (3)
           CALL "CSUB" USING TBL HIT MISS
           DISPLAY "HIT=[" HIT "]"
           DISPLAY "MISS=[" MISS "]"
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CSUB.
       DATA DIVISION.
       LINKAGE SECTION.
       01  L-TBL.
           02  L-ITM PIC X OCCURS 3 TIMES.
               88  L-IS-A VALUE "A".
       01  L-HIT  PIC X(6).
       01  L-MISS PIC X(6).
       PROCEDURE DIVISION USING L-TBL L-HIT L-MISS.
       S-MAIN.
           IF L-IS-A (1) MOVE "YES" TO L-HIT END-IF
           IF L-IS-A (2) MOVE "WRONG" TO L-MISS END-IF
           EXIT PROGRAM.
       END PROGRAM CSUB.
       END PROGRAM CMAIN.
"#;

/// Occurrence 1 holds `"A"`, so the condition is TRUE and the caller sees it.
#[test]
fn a_linkage_condition_name_sees_the_callers_data() {
    let out = run_capture(LINKAGE_88);
    let hit = field(&out, "HIT").expect("the program displays HIT=");
    assert!(
        hit.starts_with("YES"),
        "ITM(1) holds \"A\", so L-IS-A(1) is true; got {hit:?} — the 88 tested \
         the callee's own slot instead of the caller's table"
    );
}

/// Occurrence 2 holds `"B"`, so the condition is FALSE. Registering a callee's
/// 88s must not make them true by default — a fix that reported *everything*
/// as satisfied would pass the test above and fail this one.
#[test]
fn a_linkage_condition_name_is_still_false_when_it_should_be() {
    let out = run_capture(LINKAGE_88);
    let miss = field(&out, "MISS").expect("the program displays MISS=");
    assert!(
        miss.trim().is_empty(),
        "ITM(2) holds \"B\", so L-IS-A(2) is false and nothing should have \
         been written; got {miss:?}"
    );
}
