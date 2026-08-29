// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Qualified 88-level condition-names, from NIST CCVS85 NC246A.
//!
//! A condition-name may be declared under more than one group — CCVS85 declares
//! `EQUALS-A` under three separate tables — and a reference tells them apart the
//! same way a data reference does, with `OF`/`IN`. The environment held one
//! entry per name, so the **last** declaration silently won every lookup, and
//! the subscript written on the reference was then applied to the wrong host:
//! `EQUALS-M OF … OF GROUP-1-TABLE (13)` tested occurrence 13 of a table that
//! has only four, read nothing, and came out false.
//!
//! The tell was that the *harder* case passed: NC246A's three-dimensional
//! `QUAL-TEST-09` was clean while the one-dimensional `QUAL-TEST-08` failed
//! outright — because the three-dimensional table happened to be declared last.
//! Two of these tests therefore assert the same program in **both** declaration
//! orders; order-independence is the property, not any single answer.

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

/// The one-dimensional table of NC246A `QUAL-TEST-08`, holding `A…O`.
const ONE_DIM: &str = r#"
       01 GROUP-1-TABLE.
          02 TABLE-LEVEL-2.
             03 TABLE-LEVEL-3.
                04 TABLE-LEVEL-4.
                   05 TABLE-LEVEL-5.
                      06 TABLE-ITEM PIC X OCCURS 15 TIMES INDEXED BY IN1.
                      88 EQUALS-A VALUE "A".
                      88 EQUALS-M VALUE "M".
"#;

/// The three-dimensional table of NC246A `QUAL-TEST-09`, holding `A…P`.
const THREE_DIM: &str = r#"
       01 GROUP-3-TABLE.
          02 TABLE-LEVEL-2.
             03 TABLE-LEVEL-3.
                04 TABLE-LEVEL-4 OCCURS 2 TIMES INDEXED BY IN3.
                   05 TABLE-LEVEL-5 OCCURS 2 TIMES INDEXED BY IN4.
                      06 TABLE-ITEM PIC X OCCURS 4 TIMES INDEXED BY IN5.
                      88 EQUALS-A VALUE "A".
                      88 EQUALS-M VALUE "M".
"#;

/// Both tables, in the given order, with the NC246A tests over each.
fn two_tables(first: &str, second: &str) -> String {
    format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUAL88.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
{first}{second}
       PROCEDURE DIVISION.
       MAIN.
           MOVE "ABCDEFGHIJKLMNO" TO TABLE-LEVEL-5 OF TABLE-LEVEL-4
                OF TABLE-LEVEL-3 OF TABLE-LEVEL-2 OF GROUP-1-TABLE.
           MOVE "ABCDEFGHIJKLMNOP" TO TABLE-LEVEL-3 OF TABLE-LEVEL-2
                OF GROUP-3-TABLE.
           IF EQUALS-M OF TABLE-LEVEL-5 OF TABLE-LEVEL-4
                    IN TABLE-LEVEL-3 OF TABLE-LEVEL-2
                    OF GROUP-1-TABLE (13)
               DISPLAY "ONE-13 TRUE"
           ELSE
               DISPLAY "ONE-13 FALSE".
           IF EQUALS-A OF TABLE-LEVEL-5 OF TABLE-LEVEL-4
                    IN TABLE-LEVEL-3 OF TABLE-LEVEL-2
                    OF GROUP-1-TABLE (1)
               DISPLAY "ONE-1 TRUE"
           ELSE
               DISPLAY "ONE-1 FALSE".
           IF EQUALS-A OF TABLE-LEVEL-5 OF TABLE-LEVEL-4
                    IN TABLE-LEVEL-3 OF TABLE-LEVEL-2
                    OF GROUP-1-TABLE (13)
               DISPLAY "ONE-13-A TRUE"
           ELSE
               DISPLAY "ONE-13-A FALSE".
           IF EQUALS-M OF TABLE-LEVEL-5 IN TABLE-LEVEL-4
                    IN TABLE-LEVEL-3 IN TABLE-LEVEL-2
                    OF GROUP-3-TABLE (2, 2, 1)
               DISPLAY "THREE-221 TRUE"
           ELSE
               DISPLAY "THREE-221 FALSE".
           IF EQUALS-A OF TABLE-LEVEL-5 IN TABLE-LEVEL-4
                    IN TABLE-LEVEL-3 IN TABLE-LEVEL-2
                    OF GROUP-3-TABLE (1, 1, 1)
               DISPLAY "THREE-111 TRUE"
           ELSE
               DISPLAY "THREE-111 FALSE".
           STOP RUN.
"#
    )
}

/// What both declaration orders must report: each reference tests the table it
/// names, at the occurrence it names.
const EXPECTED: &[&str] = &[
    "ONE-13 TRUE",
    "ONE-1 TRUE",
    "ONE-13-A FALSE",
    "THREE-221 TRUE",
    "THREE-111 TRUE",
];

#[test]
fn a_qualified_condition_name_picks_the_table_it_names() {
    let out = run(&two_tables(ONE_DIM, THREE_DIM));
    assert_eq!(out, EXPECTED, "{out:#?}");
}

/// The same program with the two tables declared the other way round. Before
/// the fix this order passed the three-dimensional tests and failed the
/// one-dimensional ones; the other order did the reverse.
#[test]
fn declaration_order_does_not_decide_which_88_is_meant() {
    let out = run(&two_tables(THREE_DIM, ONE_DIM));
    assert_eq!(out, EXPECTED, "{out:#?}");
}

/// COBOL-85 lets a qualification chain skip intermediate levels: NC246A names
/// `TABLE-LEVEL-5` and above but never `TABLE-ITEM`, which is the actual host.
#[test]
fn a_qualification_chain_may_skip_intermediate_levels() {
    let out = run(&format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. QSKIP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
{ONE_DIM}{THREE_DIM}
       PROCEDURE DIVISION.
       MAIN.
           MOVE "ABCDEFGHIJKLMNO" TO TABLE-LEVEL-5 OF TABLE-LEVEL-4
                OF TABLE-LEVEL-3 OF TABLE-LEVEL-2 OF GROUP-1-TABLE.
           IF EQUALS-M OF GROUP-1-TABLE (13)
               DISPLAY "OUTERMOST TRUE"
           ELSE
               DISPLAY "OUTERMOST FALSE".
           IF EQUALS-M OF TABLE-ITEM OF TABLE-LEVEL-5
                    OF GROUP-1-TABLE (13)
               DISPLAY "WITH-HOST TRUE"
           ELSE
               DISPLAY "WITH-HOST FALSE".
           STOP RUN.
"#
    ));
    assert_eq!(out, ["OUTERMOST TRUE", "WITH-HOST TRUE"], "{out:#?}");
}

/// A condition-name declared once is reached without any qualifier at all —
/// the flat-store fast path this change must not disturb.
#[test]
fn an_unqualified_unique_condition_name_still_resolves() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUNIQ.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 FLAGS.
          05 STATE-CODE PIC X VALUE "Y".
             88 IS-YES VALUE "Y".
             88 IS-NO  VALUE "N".
       PROCEDURE DIVISION.
       MAIN.
           IF IS-YES DISPLAY "YES" ELSE DISPLAY "NOT YES".
           SET IS-NO TO TRUE.
           IF IS-NO DISPLAY "NO" ELSE DISPLAY "NOT NO".
           STOP RUN.
"#,
    );
    assert_eq!(out, ["YES", "NO"], "{out:#?}");
}

/// A duplicated 88 written with **no** qualifier is ambiguous. COBOL-85 makes
/// that an error; the runtime instead takes the first declaration, which is the
/// rule `resolve_canonical` already applies to an ambiguous data name. Recorded
/// so the choice is deliberate rather than incidental — it used to be the last.
#[test]
fn an_ambiguous_condition_name_takes_the_first_declaration() {
    let out = run(&format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. QAMBIG.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 FIRST-GROUP.
          05 FIRST-CODE PIC X VALUE "Y".
             88 SAME-NAME VALUE "Y".
       01 SECOND-GROUP.
          05 SECOND-CODE PIC X VALUE "N".
             88 SAME-NAME VALUE "Y".
       PROCEDURE DIVISION.
       MAIN.
           IF SAME-NAME DISPLAY "TRUE" ELSE DISPLAY "FALSE".
           IF SAME-NAME OF SECOND-GROUP
               DISPLAY "SECOND TRUE"
           ELSE
               DISPLAY "SECOND FALSE".
           STOP RUN.
"#
    ));
    assert_eq!(out, ["TRUE", "SECOND FALSE"], "{out:#?}");
}
