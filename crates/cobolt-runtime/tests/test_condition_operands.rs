// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the two sides of a COBOL-85 relation actually are, from NIST CCVS85
//! NC250A (VI-89 6.15).
//!
//! Four separate rules, all of them about the *operand* rather than the
//! comparison:
//!
//! * an 88-level condition-name declared on a **group** tests that group's
//!   bytes, which are its children's — the group owns no slot of its own;
//! * a figurative constant, including one written as an 88's `VALUE`, is
//!   repeated to the size of the other operand;
//! * `ALL literal` is repeated to the other operand's size in both directions,
//!   not padded with spaces;
//! * a group operand is category alphanumeric, so pairing it with a numeric
//!   item takes the nonnumeric comparison.
//!
//! Plus one parsing rule: in an abbreviated combined relation condition, a
//! `NOT` that is followed by an *object* rather than by a relational operator
//! negates the implied relation, and is not a test of the object's own truth.

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

/// Wrap a PROCEDURE DIVISION body around the given WORKING-STORAGE.
fn program(storage: &str, body: &str) -> String {
    format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CONDOPS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
{storage}
       PROCEDURE DIVISION.
       MAIN.
{body}
           STOP RUN.
"#
    )
}

#[test]
fn a_condition_name_on_a_group_tests_the_groups_bytes() {
    // NC250A IF--TEST-86/87/88. `TABLE-86` is six bytes across two children;
    // reading its own (never written) slot made every one of its three 88s
    // false whatever the record held.
    let out = run(&program(
        r#"
       01 TABLE-86.
          88 A86 VALUE "ABC".
          88 B86 VALUE "ABCABC".
          88 C86 VALUE "   ABC".
          02 DATANAME-86 PIC XXX VALUE "ABC".
          02 DNAME-86.
             03 FILLER PIC X VALUE "A".
             03 FILLER PIC X VALUE "B".
             03 FILLER PIC X VALUE "C".
"#,
        r#"
           IF A86 DISPLAY "A86 T" ELSE DISPLAY "A86 F".
           IF B86 DISPLAY "B86 T" ELSE DISPLAY "B86 F".
           MOVE SPACES TO DATANAME-86.
           IF C86 DISPLAY "C86 T" ELSE DISPLAY "C86 F".
"#,
    ));
    // A86 is "ABC" against "ABCABC": the shorter operand pads with spaces, so
    // it stays false — the case that passed before, and must keep passing.
    assert_eq!(out, vec!["A86 F", "B86 T", "C86 T"]);
}

#[test]
fn a_figurative_or_all_as_an_88_value_is_sized_to_its_host() {
    // NC250A IF--TEST-26/27/28. `VALUE QUOTE` on a `PIC X(4)` host is four
    // quotes; `VALUE ALL "BAC"` is "BACB". Both were left at their written
    // length and padded with spaces, so only the `SPACE` one — whose padding
    // happens to be the character it repeats — came out right.
    let out = run(&program(
        r#"
       77 IF-D33 PICTURE X(4).
          88 B VALUE QUOTE.
          88 C VALUE SPACE.
          88 D VALUE ALL "BAC".
"#,
        r#"
           MOVE QUOTE TO IF-D33.
           IF B DISPLAY "B T" ELSE DISPLAY "B F".
           MOVE SPACE TO IF-D33.
           IF C DISPLAY "C T" ELSE DISPLAY "C F".
           MOVE "BACB" TO IF-D33.
           IF D DISPLAY "D T" ELSE DISPLAY "D F".
           MOVE "BACX" TO IF-D33.
           IF D DISPLAY "D T" ELSE DISPLAY "D F".
"#,
    ));
    assert_eq!(out, vec!["B T", "C T", "D T", "D F"]);
}

#[test]
fn all_literal_is_repeated_to_the_other_operands_size() {
    // NC250A IF--TEST-4 and IF--TEST-6. `ALL "BA"` against a ten-character item
    // is "BABABABABA"; left at two characters it was "BA" padded with spaces,
    // which is neither equal nor — since a space sorts below "B" — greater.
    let out = run(&program(
        r#"
       77 IF-D6 PICTURE A(10) VALUE "BABABABABA".
"#,
        r#"
           IF IF-D6 EQUAL TO ALL "BA" DISPLAY "EQ" ELSE DISPLAY "NE".
           IF ALL "BA" GREATER THAN IF-D6 DISPLAY "GT" ELSE DISPLAY "NGT".
           IF ALL "BB" GREATER THAN IF-D6 DISPLAY "GT" ELSE DISPLAY "NGT".
"#,
    ));
    assert_eq!(out, vec!["EQ", "NGT", "GT"]);
}

#[test]
fn a_group_operand_is_alphanumeric_against_a_numeric_item() {
    // NC250A IF--TEST-77 and IF--TEST-78. A group is category alphanumeric, so
    // the numeric side becomes its characters padded on the **right**:
    // `PIC 9(5) VALUE 12345` is "12345     " against the group's "0000012345",
    // and unequal. Comparing them algebraically made 12345 = 12345.
    let out = run(&program(
        r#"
       77 IF-D37 PICTURE 9(5) VALUE 12345.
       77 IF-D38 PICTURE X(9) VALUE "12345    ".
       01 IF-D21.
          02 D1 PICTURE 9(5) VALUE ZEROS.
          02 D2 PICTURE 9(5) VALUE 12345.
"#,
        r#"
           IF IF-D37 NOT EQUAL TO IF-D21 DISPLAY "NE" ELSE DISPLAY "EQ".
           IF IF-D37 EQUAL TO IF-D38 DISPLAY "EQ" ELSE DISPLAY "NE".
"#,
    ));
    assert_eq!(out, vec!["NE", "EQ"]);
}

/// NC250A IF-TEST-123's condition, over operands supplied by the caller.
fn connectives(a: u32, b: u32, b3: u32, c1: u32, c2: u32, c3: u32) -> Vec<String> {
    run(&program(
        r#"
       01 WRK-DU-1V0-1 PIC 9.
       01 WRK-DU-1V0-2 PIC 9.
       01 WRK-DU-1V0-3 PIC 9.
       01 WRK-DU-2V0-1 PIC 99.
       01 WRK-DU-2V0-2 PIC 99.
       01 WRK-DU-2V0-3 PIC 99.
"#,
        &format!(
            r#"
           MOVE {a} TO WRK-DU-1V0-1.
           MOVE {b} TO WRK-DU-1V0-2.
           MOVE {b3} TO WRK-DU-1V0-3.
           MOVE {c1} TO WRK-DU-2V0-1.
           MOVE {c2} TO WRK-DU-2V0-2.
           MOVE {c3} TO WRK-DU-2V0-3.
           IF WRK-DU-1V0-1 > WRK-DU-1V0-2 AND NOT < WRK-DU-2V0-1 OR
                   WRK-DU-2V0-2 OR NOT WRK-DU-2V0-3 AND WRK-DU-1V0-3
                   DISPLAY "TRUE"
           ELSE
                   DISPLAY "FALSE".
"#
        ),
    ))
}

#[test]
fn not_before_an_abbreviation_object_negates_the_relation() {
    // NC250A IF-TEST-123, expanded:
    //   (a > b AND a NOT< c1) OR (a NOT< c2) OR (NOT (a NOT< c3) AND a NOT< b3)
    // With 9, 8, 7, 10, 11, 12 the first two disjuncts are false and the third
    // is true. Reading `NOT WRK-DU-2V0-3` as "the operand is zero" made the
    // third disjunct false as well.
    assert_eq!(connectives(9, 8, 7, 10, 11, 12), vec!["TRUE"]);
}

#[test]
fn the_negated_object_carries_the_whole_condition() {
    // The same condition with `a` too small for the first two disjuncts and for
    // `a NOT< b3`: everything now rests on the third term, which is false.
    // A `NOT` read as "the operand is zero" would make it true instead, since
    // 12 is not zero.
    assert_eq!(connectives(1, 8, 7, 10, 11, 12), vec!["FALSE"]);
}

#[test]
fn a_negated_object_under_a_not_greater_operator() {
    // NC250A IF-TEST-122, which passed throughout — under `NOT GREATER` with
    // the object at zero the two readings happen to agree, so it never showed
    // the defect. Pinned here because it is the case a future change to this
    // rule would most easily take with it:
    //   NOT (1 NOT> 2 AND 1 NOT> 3 AND NOT (1 NOT> 0))
    // is NOT(T AND T AND T) = false, so the guard is not taken.
    let out = run(&program(
        r#"
       01 WRK-DU-1V0-1 PIC 9.
       01 WRK-DU-1V0-2 PIC 9.
       01 WRK-DU-1V0-3 PIC 9.
       01 WRK-DU-1V0-4 PIC 9.
"#,
        r#"
           MOVE 1 TO WRK-DU-1V0-1.
           MOVE 2 TO WRK-DU-1V0-2.
           MOVE 3 TO WRK-DU-1V0-3.
           MOVE 0 TO WRK-DU-1V0-4.
           IF NOT (WRK-DU-1V0-1 NOT GREATER WRK-DU-1V0-2 AND
               WRK-DU-1V0-3 AND NOT WRK-DU-1V0-4) DISPLAY "GUARD"
               ELSE NEXT SENTENCE.
           DISPLAY "PASS".
"#,
    ));
    assert_eq!(out, vec!["PASS"]);
}

#[test]
fn not_before_a_full_relation_keeps_its_own_meaning() {
    // The new rule must not swallow a `NOT` that opens an ordinary condition:
    // `NOT X = Y` is a negated relation on its own subject, and `NOT X NUMERIC`
    // a negated class condition — neither reuses the preceding subject.
    let out = run(&program(
        r#"
       77 P PIC 9 VALUE 5.
       77 Q PIC 9 VALUE 5.
       77 R PIC X(3) VALUE "ABC".
"#,
        r#"
           IF P = 5 AND NOT Q = 9 DISPLAY "A T" ELSE DISPLAY "A F".
           IF P = 5 OR NOT Q = 5 DISPLAY "B T" ELSE DISPLAY "B F".
           IF P = 5 AND NOT R NUMERIC DISPLAY "C T" ELSE DISPLAY "C F".
"#,
    ));
    assert_eq!(out, vec!["A T", "B T", "C T"]);
}
