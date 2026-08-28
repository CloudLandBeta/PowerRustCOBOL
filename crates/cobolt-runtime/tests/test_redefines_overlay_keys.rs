// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Two ways a `REDEFINES` overlay lost its target's bytes — NIST CCVS85 NC204M.
//!
//! Both defects had the same shape on the surface — the redefining description
//! read back as **spaces** however the item it redescribes had been filled —
//! and nothing in common underneath.
//!
//! **A truncated qualification path.** `sync_redefines` seeds the initial copy
//! by walking the redefining declaration from an *empty* ancestor path, so the
//! storage keys it writes are missing every outer qualifier. That is invisible
//! while the names inside the overlay are unique, because `canon_key` hands a
//! unique leaf straight back whatever path it is given — and wrong the moment a
//! name is duplicated, which is precisely when the qualified key carries the
//! information. NC204M declares `TAB-A` under both `ACCEPT-D21` and
//! `ACCEPT-D23`, so its overlay wrote to a key nothing ever read.
//!
//! **An unnamed description has no key.** `02 FILLER REDEFINES <item>.` names
//! bytes its target already owns, under no name of its own, so the symbol table
//! gave it nothing and no overlay pair was ever recorded. It now gets the same
//! synthetic key an unnamed *leaf* gets, and the ordinary machinery takes over
//! from there.
//!
//! The two are independent: the first repro below needs no `FILLER` and the
//! second needs no duplicate name.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Run `src` and return its DISPLAY lines.
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

// ── a duplicated name inside the overlay ─────────────────────────────────────

/// NC204M `ACC-TEST-F1-10`, reduced: `TAB-A` is declared in two places, and the
/// overlay that contains one of them must still start life holding its target's
/// `VALUE`.
#[test]
fn a_duplicated_name_in_an_overlay_still_shares_the_redefined_bytes() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RDFDUP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 ACCEPT-DATA.
          02 ACCEPT-VALUE21 PICTURE X(12) VALUE "............".
          02 ACCEPT-D21 REDEFINES ACCEPT-VALUE21.
             03 TAB-ACCEPT OCCURS 3 TIMES.
                04 TAB-A PICTURE XXXX.
          02 ACCEPT-D23.
             03 TAB-A PICTURE XXXX OCCURS 5 TIMES.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "INIT=[" ACCEPT-D21 "]".
           MOVE "ABCD" TO TAB-ACCEPT (2).
           DISPLAY "OVER=[" ACCEPT-D21 "]".
           DISPLAY "BASE=[" ACCEPT-VALUE21 "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "INIT=[............]", "{out:#?}");
    assert_eq!(out[1], "OVER=[....ABCD....]", "{out:#?}");
    assert_eq!(out[2], "BASE=[....ABCD....]", "{out:#?}");
}

/// The same program with the duplicate removed — the case that always worked,
/// pinned so a future change to the key path cannot quietly swap which of the
/// two is broken.
#[test]
fn a_unique_name_in_an_overlay_shares_the_redefined_bytes() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RDFUNIQ.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 ACCEPT-DATA.
          02 ACCEPT-VALUE21 PICTURE X(12) VALUE "............".
          02 ACCEPT-D21 REDEFINES ACCEPT-VALUE21.
             03 TAB-ACCEPT OCCURS 3 TIMES.
                04 TAB-A PICTURE XXXX.
          02 ACCEPT-D23.
             03 TAB-B PICTURE XXXX OCCURS 5 TIMES.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "INIT=[" ACCEPT-D21 "]".
           MOVE "ABCD" TO TAB-ACCEPT (2).
           DISPLAY "OVER=[" ACCEPT-D21 "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "INIT=[............]", "{out:#?}");
    assert_eq!(out[1], "OVER=[....ABCD....]", "{out:#?}");
}

// ── an unnamed redefining description ────────────────────────────────────────

/// NC204M `ACC-TEST-F1-14-1`, reduced: a write to the redefined item must reach
/// the child of the `FILLER` group that redescribes it.
#[test]
fn a_filler_redefines_sees_writes_to_the_item_it_redescribes() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RDFFIL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 G.
          02 ACCEPT-TEST-14-DATA PIC X(15).
          02 FILLER REDEFINES ACCEPT-TEST-14-DATA.
             03 ACC-14-CHARS-1-10 PIC X(10).
       PROCEDURE DIVISION.
       MAIN.
           MOVE "ABCDEFGHIJKLMNO" TO ACCEPT-TEST-14-DATA.
           DISPLAY "C110=[" ACC-14-CHARS-1-10 "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "C110=[ABCDEFGHIJ]", "{out:#?}");
}

/// Two `FILLER REDEFINES` of the same item, as NC204M writes them. Each is its
/// own description starting at the target's first byte — `ACC-14-CHARS-11-15`
/// is bytes 1–5 in spite of its name.
#[test]
fn two_filler_redefines_of_one_item_each_start_at_its_first_byte() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RDFFIL2.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 G.
          02 ACCEPT-TEST-14-DATA PIC X(15).
          02 FILLER REDEFINES ACCEPT-TEST-14-DATA.
             03 ACC-14-CHARS-1-10 PIC X(10).
          02 FILLER REDEFINES ACCEPT-TEST-14-DATA.
             03 ACC-14-CHARS-11-15 PIC X(5).
       PROCEDURE DIVISION.
       MAIN.
           MOVE "ABCDEFGHIJ" TO ACCEPT-TEST-14-DATA.
           DISPLAY "A=[" ACC-14-CHARS-1-10 "]".
           MOVE "KLMNO" TO ACCEPT-TEST-14-DATA.
           DISPLAY "B=[" ACC-14-CHARS-11-15 "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "A=[ABCDEFGHIJ]", "{out:#?}");
    assert_eq!(out[1], "B=[KLMNO]", "{out:#?}");
}

/// The overlay is a *description*, not an alias of its first child: several
/// children divide the target's bytes between them, and a write through either
/// side lands where the layout says it does.
#[test]
fn a_multi_child_filler_redefines_divides_the_bytes() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RDFMULT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 G.
          02 T PIC X(10) VALUE "ABCDEFGHIJ".
          02 FILLER REDEFINES T.
             03 C1 PIC X(5).
             03 C2 PIC X(5).
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "INIT=[" C1 "][" C2 "]".
           MOVE "0123456789" TO T.
           DISPLAY "FWD =[" C1 "][" C2 "]".
           MOVE "XXXXX" TO C2.
           DISPLAY "BACK=[" T "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "INIT=[ABCDE][FGHIJ]", "{out:#?}");
    assert_eq!(out[1], "FWD =[01234][56789]", "{out:#?}");
    assert_eq!(out[2], "BACK=[01234XXXXX]", "{out:#?}");
}

/// A `FILLER` group that redefines nothing keeps the behaviour it had: it is
/// storage of its own inside the parent, not a second reading of a sibling.
/// Without this the overlay branch would claim every unnamed group.
///
/// The group's own reading is deliberately not asserted here. `01 G` renders as
/// `AAAAA` alone — an unnamed group **with children** contributes a synthetic
/// FILLER slot to its parent's layout and then stores nothing in it, so the
/// parent reads short. That is a separate, pre-existing gap in group
/// serialization (the overlay branch below requires a `REDEFINES` and never
/// fires here), and it is recorded rather than repaired inside a module pass.
#[test]
fn a_plain_filler_group_is_not_treated_as_an_overlay() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RDFPLAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 G.
          02 T PIC X(5) VALUE "AAAAA".
          02 FILLER.
             03 C1 PIC X(5) VALUE "BBBBB".
       PROCEDURE DIVISION.
       MAIN.
           MOVE "ZZZZZ" TO T.
           DISPLAY "C1=[" C1 "]".
           DISPLAY "T =[" T "]".
           STOP RUN.
"#,
    );
    assert_eq!(
        out[0], "C1=[BBBBB]",
        "a plain FILLER group owns its own bytes — a write to T must not reach it"
    );
    assert_eq!(out[1], "T =[ZZZZZ]", "{out:#?}");
}
