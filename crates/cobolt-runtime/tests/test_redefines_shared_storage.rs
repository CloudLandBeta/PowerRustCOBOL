// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Large `REDEFINES` overlays share storage — NIST CCVS85 NC234A.
//!
//! Two descriptions of the same bytes are kept consistent by *copying* one into
//! the other on every write, which is affordable for the flat twenty-byte
//! records that mechanism exists for and ruinous for a redefined 10×10×10
//! table: one `MOVE` would walk a thousand occurrences twice. Descriptions past
//! `REDEFINE_SYNC_BUDGET` therefore used to give up and keep separate storage —
//! so every name in the redefining description read as **spaces**, however the
//! redefined table had been filled.
//!
//! When the two descriptions have the *same* layout there is nothing to copy:
//! they are the same bytes read the same way, so they share the slots outright.
//! Free, exact, and no budget applies. Raising the budget instead was measured
//! at 20 s on NC234A — past the harness timeout — against 0.09 s for sharing.
//!
//! The sharing is **storage only**. Each description keeps its own symbol
//! entry, and in particular its own `INDEXED BY` names: `SEARCH GRP-ENTRY-1`
//! has to be driven by `IDX-1-1`, not by the redefined table's `IDX-1`. That is
//! what the last two tests here guard, and it is why the aliasing cannot live
//! in name resolution.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Run `src` and return its DISPLAY lines. Panics with the diagnostics when the
/// program does not parse — an unsupported form must fail loudly here.
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
    display_rx.try_iter().map(|l| l.trim().to_string()).collect()
}

/// NC234A's 10×10×10 table and its identically-shaped redefinition — 2221
/// expanded slots against a 256-slot copy budget.
fn prog(body: &str) -> String {
    format!(
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. RDFSHARE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  3-DIMENSION-TBL.
           02  GRP-ENTRY OCCURS 10 TIMES INDEXED BY IDX-1.
               03  ENTRY-1 PICTURE X(5).
               03  GRP2-ENTRY OCCURS 10 TIMES INDEXED BY IDX-2.
                   04  ENTRY-2 PICTURE X(11).
                   04  3-ENTRY OCCURS 10 TIMES INDEXED BY IDX-3.
                       05  ENTRY-3 PICTURE X(15).
       01  3-DEM-TBL REDEFINES 3-DIMENSION-TBL.
           02  GRP-ENTRY-1 OCCURS 10 TIMES INDEXED BY IDX-1-1.
               03  ENTRY-1-1 PIC X(5).
               03  GRP2-ENTRY-1 OCCURS 10 TIMES INDEXED BY IDX-2-1.
                   04  ENTRY-2-1 PIC X(11).
                   04  GRP3-ENTRY-1 OCCURS 10 TIMES INDEXED BY IDX-3-1.
                       05  ENTRY-3-1 PIC X(15).
       01  HOLD-AREA PIC X(5) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN-PARA.
{body}
           STOP RUN.
"
    )
}

/// The one-line statement of the bug: a write through the redefined table is
/// visible through the redefining one, at every level of the table.
#[test]
fn a_write_is_visible_through_the_redefining_description() {
    let out = run(&prog(
        "           MOVE \"GRP01\" TO ENTRY-1 (1).
           MOVE \"SEC (01,02)\" TO ENTRY-2 (1, 2).
           MOVE \"ELEM (01,02,03)\" TO ENTRY-3 (1, 2, 3).
           DISPLAY ENTRY-1-1 (1).
           DISPLAY ENTRY-2-1 (1, 2).
           DISPLAY ENTRY-3-1 (1, 2, 3).",
    ));
    assert_eq!(out, vec!["GRP01", "SEC (01,02)", "ELEM (01,02,03)"]);
}

/// And the other direction — sharing is symmetric, not a one-way copy.
#[test]
fn a_write_through_the_redefining_description_is_visible_too() {
    let out = run(&prog(
        "           MOVE \"ZZZZZ\" TO ENTRY-1-1 (4).
           DISPLAY ENTRY-1 (4).",
    ));
    assert_eq!(out, vec!["ZZZZZ"]);
}

/// Occurrences are addressed independently: sharing must not smear one
/// occurrence's value across the table.
#[test]
fn shared_storage_keeps_occurrences_distinct() {
    let out = run(&prog(
        "           MOVE \"AAAAA\" TO ENTRY-1 (1).
           MOVE \"BBBBB\" TO ENTRY-1 (2).
           DISPLAY ENTRY-1-1 (1) \" \" ENTRY-1-1 (2).",
    ));
    assert_eq!(out, vec!["AAAAA BBBBB"]);
}

/// A description holding an unnamed `FILLER` has no symbol entry to compare,
/// so it does not qualify for sharing and keeps the copying overlay. Under the
/// budget that overlay still works, which is what this guards — the new path
/// must not have displaced the old one.
#[test]
fn small_overlays_still_use_the_copying_path() {
    let out = run(
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. RDFSMALL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  FLAT-REC.
           02  FILLER PIC X(3).
           02  F-TAIL PIC X(4).
       01  FLAT-ALT REDEFINES FLAT-REC.
           02  A-ALL PIC X(7).
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE \"ABCDEFG\" TO A-ALL.
           DISPLAY F-TAIL.
           STOP RUN.
",
    );
    assert_eq!(out, vec!["DEFG"]);
}

// ── storage is shared, metadata is not ─────────────────────────────────────

/// NC234A TH1-TEST-F1-2. `SEARCH GRP-ENTRY-1` must be driven by that table's
/// own index `IDX-1-1` — the one the `WHEN` subscripts by — even though its
/// storage is shared with `GRP-ENTRY`, whose index is `IDX-1`. Aliasing in
/// name resolution would hand the search `IDX-1`, leave `IDX-1-1` at 1, and
/// take the `AT END` path.
#[test]
fn search_on_the_redefining_table_uses_its_own_index() {
    let out = run(&prog(
        "           MOVE \"GRP01\" TO ENTRY-1 (1).
           MOVE \"GRP07\" TO ENTRY-1 (7).
           MOVE \"GRP07\" TO HOLD-AREA.
           SET IDX-1-1 TO 1.
           SEARCH GRP-ENTRY-1
               AT END DISPLAY \"AT-END\"
               WHEN ENTRY-1-1 (IDX-1-1) = HOLD-AREA
                   DISPLAY \"FOUND\"
           END-SEARCH.",
    ));
    assert_eq!(out, vec!["FOUND"]);
}

/// The same search with `VARYING` naming an index of the *other* table
/// (COBOL-85 6.21.4 GR3): the table's own index still drives it, and the
/// named index is stepped alongside.
#[test]
fn search_varying_a_foreign_index_still_uses_the_tables_own() {
    let out = run(&prog(
        "           MOVE \"GRP01\" TO ENTRY-1 (1).
           MOVE \"GRP01\" TO HOLD-AREA.
           SET IDX-1-1 TO 1.
           SEARCH GRP-ENTRY-1 VARYING IDX-1
               AT END DISPLAY \"AT-END\"
               WHEN ENTRY-1-1 (IDX-1-1) = HOLD-AREA
                   DISPLAY \"FOUND\"
           END-SEARCH.",
    ));
    assert_eq!(out, vec!["FOUND"]);
}
