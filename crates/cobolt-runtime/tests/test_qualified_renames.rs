// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! 66-level `RENAMES`, qualified and otherwise, from NIST CCVS85 NC252A/NC209A.
//!
//! A 66 is a *root* in the parse tree — its level number puts it outside the
//! ordinary hierarchy — but COBOL keeps it subordinate to the record whose
//! items it regroups, so `RENAME-5 OF T-RENAMES-DATA` is an ordinary qualified
//! reference. The environment held one entry per **bare** name and registered
//! none of them for name resolution, which cost four separate ways:
//!
//! * two records declaring the same 66 name resolved to whichever was parsed
//!   last, so a qualified write landed in the other record;
//! * a qualified *read* never consulted the RENAMES table at all and returned
//!   the storage lookup's zero;
//! * a covered `OCCURS` item contributed only its base slot, so a RENAMES over
//!   a table read one occurrence's width;
//! * a RENAMES over a single elementary item was treated as its own (unread)
//!   slot rather than as the item, so arithmetic on it did nothing.
//!
//! Order-independence is the property under test, not any single answer: two of
//! these run the same program with the records declared in the other order.

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

/// NC252A's first renamed record: `RENAME-5` over `TAG-1A`…`TAG-1B`.
const T_RECORD: &str = r#"
       01 T-RENAMES-DATA.
          02 TAG-1.
             03 TAG-1A     PICTURE XXXX.
             03 TAG-1B     PICTURE XXXXXX.
          02 NAME-2        PICTURE XXXXXXX.
       66 RENAME-5 RENAMES TAG-1A THRU TAG-1B.
       66 RENAME-6 RENAMES TAG-1A THRU NAME-2.
"#;

/// NC252A's second record, declaring the **same two** 66 names over its own
/// items — and its own `NAME-2`, so the renamed operands are duplicated too.
const U_RECORD: &str = r#"
       01 U-RENAMES-DATA.
          02 UNIT-1.
             03 UNIT-1A    PICTURE XXXXXXX VALUE "VERMONT".
             03 UNIT-1B    PICTURE XXXX    VALUE "OHIO".
          02 NAME-2        PICTURE XXXXX   VALUE "MAINE".
       66 RENAME-5 RENAMES UNIT-1A THROUGH UNIT-1B.
       66 RENAME-6 RENAMES UNIT-1A THRU NAME-2.
"#;

/// NC252A RENAM-TEST-8/9, with the two records in the given declaration order.
fn qualified_write(first: &str, second: &str) -> Vec<String> {
    run(&format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUALREN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
{first}{second}
       PROCEDURE DIVISION.
       MAIN.
           MOVE "IOWA" TO TAG-1A.
           MOVE "OREGON" TO TAG-1B.
           MOVE "CALIFORNIA" TO RENAME-5 OF T-RENAMES-DATA.
           DISPLAY "TAG-1=[" TAG-1 "]".
           DISPLAY "UNIT-1=[" UNIT-1 "]".
           STOP RUN.
"#
    ))
}

#[test]
fn qualified_renames_write_reaches_its_own_record() {
    // NC252A RENAM-TEST-8 and RENAM-TEST-9 are one test: the write must land in
    // `T-RENAMES-DATA` and leave `U-RENAMES-DATA` alone. Resolving to the last
    // declaration swapped them — TAG-1 kept "IOWAOREGON" and UNIT-1 became
    // "CALIFORNIA".
    assert_eq!(
        qualified_write(T_RECORD, U_RECORD),
        vec!["TAG-1=[CALIFORNIA]", "UNIT-1=[VERMONTOHIO]"]
    );
}

#[test]
fn qualified_renames_write_is_declaration_order_independent() {
    // The same program with the records the other way round. A resolver that
    // takes the first (or the last) declaration passes exactly one of these two.
    assert_eq!(
        qualified_write(U_RECORD, T_RECORD),
        vec!["TAG-1=[CALIFORNIA]", "UNIT-1=[VERMONTOHIO]"]
    );
}

#[test]
fn qualified_renames_read_consults_the_renames_table() {
    // NC252A RENAM-TEST-10. `Expr::Qualified` went straight to the storage
    // lookup, which a 66 has no slot in, so the read came back as 0.
    let out = run(&format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUALREAD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
{T_RECORD}{U_RECORD}
       PROCEDURE DIVISION.
       MAIN.
           MOVE "IOWAOREGONFLORIDA" TO T-RENAMES-DATA.
           DISPLAY "R6=[" RENAME-6 IN T-RENAMES-DATA "]".
           STOP RUN.
"#
    ));
    assert_eq!(out, vec!["R6=[IOWAOREGONFLORIDA]"]);
}

#[test]
fn renames_over_a_table_covers_every_occurrence() {
    // NC252A RENAM-TEST-11. `elem_order` holds one entry per *declaration*, so
    // the covered `TABLE-ITEM-2` contributed a single slot and the RENAMES read
    // back as "BOSTO" — `ITEM-1` alone.
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RENTAB.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 V-RENAMES-DATA.
          02 ITEM-1        PICTURE X(5).
          02 TABLE-2.
             03 TABLE-ITEM-2 PICTURE XXX OCCURS 5 TIMES.
       66 RENAME-7 RENAMES ITEM-1 THRU TABLE-2.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "BOSTO" TO ITEM-1.
           MOVE "N M" TO TABLE-ITEM-2 (1).
           MOVE "ASS" TO TABLE-ITEM-2 (2).
           MOVE "ACH" TO TABLE-ITEM-2 (3).
           MOVE "USE" TO TABLE-ITEM-2 (4).
           MOVE "TTS" TO TABLE-ITEM-2 (5).
           IF RENAME-7 EQUAL TO "BOSTON MASSACHUSETTS"
               DISPLAY "MATCH" ELSE DISPLAY "COMPUTED=[" RENAME-7 "]".
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["MATCH"]);
}

#[test]
fn renames_of_one_item_is_that_item_for_arithmetic() {
    // NC252A RENAM-TEST-16 and RENAM-TEST-17. `66 RENAME-12 RENAMES WIDGET-4`
    // has WIDGET-4's description — four digits — so 8000 + 3500 overflows and
    // the receiver is left alone. Writing the RENAMES' own key instead raised
    // no size error *and* stored nothing, which made TEST-17 pass for the wrong
    // reason.
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RENADD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 W-RENAMES-DATA.
          02 WIDGET-4      PICTURE 9(4).
          02 WIDGET-5      PICTURE 9(4).
       66 RENAME-12 RENAMES WIDGET-4.
       PROCEDURE DIVISION.
       MAIN.
           MOVE 8000 TO WIDGET-4.
           ADD 3500 TO RENAME-12 ON SIZE ERROR DISPLAY "SIZE ERROR".
           DISPLAY "R12=[" RENAME-12 "]".
           DISPLAY "W4=[" WIDGET-4 "]".
           ADD 1000 TO RENAME-12 ON SIZE ERROR DISPLAY "SIZE ERROR".
           DISPLAY "W4=[" WIDGET-4 "]".
           STOP RUN.
"#,
    );
    assert_eq!(
        out,
        vec!["SIZE ERROR", "R12=[8000]", "W4=[8000]", "W4=[9000]"]
    );
}

#[test]
fn a_renames_wins_over_a_data_item_that_shares_its_name() {
    // NC209A MOV-TEST-F2-5 .03. `66 HARRY RENAMES HARRY-A THRU HARRY-B` under
    // `A-GLOB` shares its name with two ordinary data items in other records.
    // A 66 reached neither `by_leaf` nor `symbols`, so `HARRY OF A-GLOB`
    // resolved to the first candidate — an unrelated item holding "HARRY".
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. RENDUP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A-LEVEL.
          02 DD-LEVEL.
             05 HARRY      PICTURE X(5) VALUE "VVVVV".
       01 A-GLOB.
          02 B-LEVEL.
             03 DD-LEVEL.
                05 HARRY-A PICTURE XX   VALUE "UU".
                05 HARRY-B PICTURE XXX  VALUE "UUU".
       66 HARRY RENAMES HARRY-A THRU HARRY-B.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "H=[" HARRY OF A-GLOB "]".
           DISPLAY "A=[" HARRY OF A-LEVEL "]".
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["H=[UUUUU]", "A=[VVVVV]"]);
}
