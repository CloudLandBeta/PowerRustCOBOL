// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Two unrelated defects, both from NIST CCVS85 NC208A.
//!
//! **`GO TO paragraph {OF|IN} section`.** The qualifier picks which of two
//! like-named paragraphs is meant. `PERFORM … OF …` already honoured it;
//! `GO TO` resolved against `para_order`, which is keyed by bare name and hands
//! back the **first** definition anywhere in the program. NC208A declares
//! `PAR-4B` in both `QUAL-SECTION-1` and `QUAL-SECTION-2`, jumps to the second,
//! and landed in the first — the copy whose comment says it should never be
//! entered.
//!
//! **`MOVE CORRESPONDING` where one side of a pair is a group.** COBOL-85 asks
//! only that **at least one** of a corresponding pair be elementary, so a group
//! may legitimately face an elementary item. A group owns no store slot — its
//! value is synthesized from its subordinate items — so reading one through
//! `get` yielded nothing and writing one through `set` went where nothing reads
//! back. Both directions were silently dropped.

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
    display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect()
}

// ── GO TO paragraph {OF|IN} section ──────────────────────────────────────────

/// NC208A `PAR-TEST-F2-4`, reduced: the jump must reach the copy in the section
/// it names, and the jump back must reach the copy in *its* section.
#[test]
fn a_qualified_go_to_reaches_the_named_sections_copy() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GTQUAL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 QT4 PIC X(4) VALUE "----".
       PROCEDURE DIVISION.
       DRIVER SECTION.
       D-1.
           GO TO PAR-4B IN QUAL-SECTION-2.
       PAR-4A.
           MOVE "BAD1" TO QT4.
           GO TO PAR-4C.
       PAR-4B.
           MOVE "BAD2" TO QT4.
       PAR-4C.
           DISPLAY "QT4=[" QT4 "]".
           STOP RUN.
       QUAL-SECTION-2 SECTION.
       PAR-4B.
           MOVE "GOOD" TO QT4.
           GO TO PAR-4C IN DRIVER.
       PAR-4C.
           DISPLAY "WRONG SECTION".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "QT4=[GOOD]", "{out:#?}");
}

/// `OF` is the same qualifier as `IN`.
#[test]
fn of_qualifies_a_go_to_as_in_does() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GTOF.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-R PIC X(4) VALUE "----".
       PROCEDURE DIVISION.
       SEC-A SECTION.
       A-1.
           GO TO TARGET OF SEC-B.
       TARGET.
           MOVE "BADA" TO WS-R.
           GO TO DONE.
       DONE.
           DISPLAY "R=[" WS-R "]".
           STOP RUN.
       SEC-B SECTION.
       TARGET.
           MOVE "OKAY" TO WS-R.
           GO TO DONE OF SEC-A.
"#,
    );
    assert_eq!(out[0], "R=[OKAY]", "{out:#?}");
}

/// The unqualified form is untouched, and an **unknown** qualifier falls back
/// to it rather than losing the jump — the same choice `PERFORM … OF …` makes.
#[test]
fn an_unqualified_or_unknown_qualifier_still_resolves() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GTPLAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-R PIC X(4) VALUE "----".
       PROCEDURE DIVISION.
       SEC-A SECTION.
       A-1.
           GO TO A-3.
       A-2.
           MOVE "BAD " TO WS-R.
       A-3.
           MOVE "PLN " TO WS-R.
           GO TO A-4 IN NO-SUCH-SECTION.
       A-4.
           DISPLAY "R=[" WS-R "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "R=[PLN ]", "{out:#?}");
}

/// `GO TO … DEPENDING ON` takes a bare list of names — the qualified form is
/// the single-target one, and the list must keep working.
#[test]
fn go_to_depending_on_is_unaffected() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GTDEP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N PIC 9 VALUE 2.
       PROCEDURE DIVISION.
       MAIN.
           GO TO P-1 P-2 P-3 DEPENDING ON WS-N.
           DISPLAY "FELL THROUGH".
           STOP RUN.
       P-1.
           DISPLAY "ONE".
           STOP RUN.
       P-2.
           DISPLAY "TWO".
           STOP RUN.
       P-3.
           DISPLAY "THREE".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "TWO", "{out:#?}");
}

// ── MOVE CORRESPONDING with a group on one side ──────────────────────────────

/// NC208A `MOV-TEST-F1-4`: the receiver of a corresponding pair is a **group**
/// and the sender elementary. Writing the group's own name put the characters
/// where nothing reads them, so the receiver kept its spaces.
#[test]
fn corresponding_moves_into_a_group_receiver() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CORRTO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 GRP-MOVE-CORR-1.
          09 MOVE-CORR-1 PICTURE 999 VALUE 111.
          09 MOVE-CORR-4 PICTURE XXX VALUE "XYZ".
       01 GRP-MOVE-CORR-R.
          05 MOVE-CORR-1 PICTURE XXX.
          05 MOVE-CORR-4.
             06 FILLER PICTURE 999.
             06 FILLER PICTURE XXX.
       PROCEDURE DIVISION.
       MAIN.
           MOVE SPACE TO GRP-MOVE-CORR-R.
           MOVE CORRESPONDING GRP-MOVE-CORR-1 TO GRP-MOVE-CORR-R.
           DISPLAY "[" GRP-MOVE-CORR-R "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "[111XYZ   ]", "{out:#?}");
}

/// NC208A `MOV-TEST-F1-5`: the mirror — the **sender** is a group and the
/// receiver elementary. A group owns no slot, so reading one through `get`
/// yielded nothing and the receiver was left as it was.
#[test]
fn corresponding_moves_out_of_a_group_sender() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CORRFROM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SRC.
          05 MOVE-CORR-E3 PICTURE XXX VALUE "123".
          05 MOVE-CORR-G5.
             09 MOVE-CORR-E4 PICTURE XXX VALUE "ABC".
             09 MOVE-CORR-E5 PICTURE 99  VALUE 45.
       01 DST.
          06 MOVE-CORR-E3 PICTURE 999.
          06 MOVE-CORR-G5 PICTURE X(5).
       PROCEDURE DIVISION.
       MAIN.
           MOVE SPACE TO DST.
           MOVE CORRESPONDING SRC TO DST.
           DISPLAY "[" DST "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "[123ABC45]", "{out:#?}");
}

/// Two groups facing each other still **recurse** — that pairing is not the
/// elementary case and must not be flattened into one alphanumeric move.
#[test]
fn corresponding_still_recurses_through_two_groups() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CORRREC.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SRC.
          05 G1.
             09 A PICTURE 999 VALUE 111.
             09 B PICTURE 999 VALUE 222.
       01 DST.
          05 G1.
             09 B PICTURE 999.
             09 A PICTURE 999.
       PROCEDURE DIVISION.
       MAIN.
           MOVE ZERO TO DST.
           MOVE CORRESPONDING SRC TO DST.
           DISPLAY "[" DST "]".
           STOP RUN.
"#,
    );
    // Recursion matches by NAME inside G1, so B (222) lands first and A (111)
    // second — a flattened byte move would have given "111222".
    assert_eq!(out[0], "[222111]", "{out:#?}");
}

// ── What CORRESPONDING may not pair, and what it must reach ──────────────────
//
// From NIST CCVS85 NC209A. COBOL-85 6.18.4 GR1 leaves an item out of the
// correspondence when it is described with `REDEFINES` or `RENAMES`; both were
// being paired, so a 66-level regrouping received the sender's like-named item
// and overwrote the two items it renames. And either operand may name **one
// occurrence** of a table of groups — the symbol table is keyed by the base
// item, so a subscripted operand matched nothing and the statement moved
// nothing at all.

/// NC209A `MOV-TEST-F2-5`: `66 HARRY RENAMES HARRY-A THRU HARRY-B` is not a
/// corresponding item, so the sender's `HARRY` must not reach it — while the
/// items that *do* correspond still move.
#[test]
fn corresponding_skips_a_66_level_renames() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CORRREN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A-LEVEL.
          02 D-LEVEL.
             05 TOM   PICTURE XXX  VALUE "TOM".
             05 HARRY PICTURE X(5) VALUE "HARRY".
       01 A-GLOB.
          02 D-LEVEL.
             05 TOM     PICTURE XXX VALUE "UUU".
             05 HARRY-A PICTURE XX  VALUE "UU".
             05 HARRY-B PICTURE XXX VALUE "UUU".
          66 HARRY RENAMES HARRY-A THRU HARRY-B.
       PROCEDURE DIVISION.
       MAIN.
           MOVE CORRESPONDING A-LEVEL TO A-GLOB.
           DISPLAY "TOM=[" TOM OF A-GLOB "]".
           DISPLAY "AB=[" HARRY-A "][" HARRY-B "]".
           STOP RUN.
"#,
    );
    assert_eq!(
        out,
        ["TOM=[TOM]", "AB=[UU][UUU]"],
        "TOM corresponds; the renamed pair is untouched — {out:#?}"
    );
}

/// NC209A `MOV-TEST-F2-6`: an item described with `REDEFINES` is excluded too,
/// and excluding the group excludes everything under it.
#[test]
fn corresponding_skips_a_redefining_item() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CORRRDF.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 C-LEVEL.
          04 DD-LEVEL.
             05 HARRY PICTURE X(5) VALUE "HARRY".
          04 DDD-LEVEL.
             05 JOE   PICTURE XXX  VALUE "JOE".
       01 C-COLLECTION.
          04 DD-LEVEL-FALSE PICTURE 9(5) VALUE 77777.
          04 DD-LEVEL REDEFINES DD-LEVEL-FALSE.
             05 HARRY PICTURE X(5).
          04 DDD-LEVEL.
             05 JOE PICTURE XXX VALUE "TTT".
       PROCEDURE DIVISION.
       MAIN.
           MOVE CORRESPONDING C-LEVEL TO C-COLLECTION.
           DISPLAY "HARRY=[" HARRY OF DD-LEVEL OF C-COLLECTION "]".
           DISPLAY "JOE=[" JOE OF C-COLLECTION "]".
           STOP RUN.
"#,
    );
    assert_eq!(
        out,
        ["HARRY=[77777]", "JOE=[JOE]"],
        "the redefining branch keeps its target's bytes; JOE still moves — {out:#?}"
    );
}

/// A plain item that merely *shares its name* with a 66 declared elsewhere is
/// an ordinary corresponding item. Excluding by name rather than by declaration
/// broke NC209A `MOV-TEST-F2-4` while fixing `-F2-5`.
#[test]
fn corresponding_excludes_the_declaration_not_the_name() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CORRNAME.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A-LEVEL.
          02 DD-LEVEL.
             05 HARRY PICTURE X(5) VALUE "HARRY".
       01 C-STACK.
          04 DD-LEVEL.
             05 HARRY PICTURE X(5) VALUE "VVVVV".
       01 A-GLOB.
          02 DD-LEVEL.
             05 HARRY-A PICTURE XX  VALUE "UU".
             05 HARRY-B PICTURE XXX VALUE "UUU".
          66 HARRY RENAMES HARRY-A THRU HARRY-B.
       PROCEDURE DIVISION.
       MAIN.
           MOVE CORRESPONDING A-LEVEL TO C-STACK.
           DISPLAY "STACK=[" HARRY OF C-STACK "]".
           STOP RUN.
"#,
    );
    assert_eq!(out, ["STACK=[HARRY]"], "{out:#?}");
}

/// NC209A `MOV-TEST-F2-7` / `-F2-8`: the receiving operand names one occurrence
/// of a table of groups. The subscript has to reach the paired children, or the
/// statement writes nothing.
#[test]
fn corresponding_reaches_one_occurrence_of_a_table() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CORRSUB.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 C-LEVEL.
          04 D-LEVEL.
             05 TOM  PICTURE XXX  VALUE "TOM".
             05 DICK PICTURE XXXX VALUE "DICK".
       01 A-FLOCK.
          02 B-FLOCK OCCURS 3 TIMES.
             03 C-FLOCK.
                04 D-LEVEL.
                   05 TOM  PICTURE XXX.
                   05 DICK PICTURE XXXX.
       PROCEDURE DIVISION.
       MAIN.
           MOVE ALL "S" TO A-FLOCK.
           MOVE CORRESPONDING C-LEVEL TO C-FLOCK (2).
           DISPLAY "2=[" TOM OF D-LEVEL OF C-FLOCK OF B-FLOCK (2)
                   "][" DICK OF D-LEVEL OF C-FLOCK OF B-FLOCK (2) "]".
           DISPLAY "1=[" TOM OF D-LEVEL OF C-FLOCK OF B-FLOCK (1) "]".
           STOP RUN.
"#,
    );
    assert_eq!(
        out,
        ["2=[TOM][DICK]", "1=[SSS]"],
        "only occurrence 2 receives — {out:#?}"
    );
}
