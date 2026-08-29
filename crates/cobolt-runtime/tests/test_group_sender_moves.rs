// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A **group** operand carries bytes, not values — NIST CCVS85 NC104A/NC105A.
//!
//! COBOL-85 6.18.4 makes a move alphanumeric-to-alphanumeric whenever either
//! operand is a group item: the other operand's PICTURE contributes its *size*
//! and nothing else. A group *receiver* already followed that rule; a group
//! *sender* did not, so an elementary receiver still edited, de-edited or
//! parsed what arrived — `MOVE <group holding "123ABC">` left `"0123AB0"` in a
//! `PIC 0XXXXX0` and zero in a `PIC 9999V999`.
//!
//! The same rule reaches two places that are easy to read as unrelated:
//!
//! * **distributing a group's own bytes into its children**, where an
//!   alphanumeric-edited child had its insertion characters re-imposed on an
//!   already-edited slice and `"1 A05"` became `"1  0A"`; and
//! * **a `VALUE` clause written on a group**, which was stored in the group's
//!   own slot — and a group has no slot anyone reads back, so the child kept
//!   its default.
//!
//! `JUSTIFIED RIGHT` is the one thing the receiver still gets a say in: it is
//! an alignment rule for an alphanumeric move, and this move is alphanumeric.

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

/// NC105A `MOVE-TEST-F1-20`: a group sender into an **alphanumeric-edited**
/// receiver transfers the bytes; the receiver's `0` insertion positions are not
/// re-imposed.
#[test]
fn a_group_sender_does_not_edit_an_alphanumeric_edited_receiver() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GRPAE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SRC-GRP.
          05 SRC-N PIC 999 VALUE 123.
          05 SRC-A PIC AAA VALUE "ABC".
       01 RCV-AE PIC 0XXXXX0.
       PROCEDURE DIVISION.
       MAIN.
           MOVE SRC-GRP TO RCV-AE.
           DISPLAY "[" RCV-AE "]".
           STOP RUN.
"#,
    );
    assert_eq!(out, ["[123ABC ]"], "{out:#?}");
}

/// NC105A `MOVE-TEST-F1-17`: a group sender into a **numeric** receiver stores
/// the characters, which the receiver's `PIC X(7)` REDEFINES reads back. The
/// digits are not parsed and the item is not zeroed.
#[test]
fn a_group_sender_stores_characters_in_a_numeric_receiver() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GRPNUM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SRC-GRP.
          05 SRC-N PIC 999 VALUE 123.
          05 SRC-A PIC AAA VALUE "ABC".
       01 RCV-NUM  PIC 9999V999.
       01 RCV-CHAR REDEFINES RCV-NUM PIC X(7).
       PROCEDURE DIVISION.
       MAIN.
           MOVE SRC-GRP TO RCV-NUM.
           DISPLAY "[" RCV-CHAR "]".
           STOP RUN.
"#,
    );
    assert_eq!(out, ["[123ABC ]"], "{out:#?}");
}

/// NC105A `MOVE-TEST-F1-16`: the receiver is narrower than the group, so the
/// bytes are truncated on the right — `"123ABC"` into `PIC 99` leaves `"12"`,
/// which then reads as the number twelve.
#[test]
fn a_group_sender_truncates_on_the_right() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GRPTRUNC.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SRC-GRP.
          05 SRC-N PIC 999 VALUE 123.
          05 SRC-A PIC AAA VALUE "ABC".
       01 RCV-2 PIC 99.
       PROCEDURE DIVISION.
       MAIN.
           MOVE SRC-GRP TO RCV-2.
           IF RCV-2 EQUAL TO 12 DISPLAY "TWELVE" ELSE DISPLAY "NOT [" RCV-2 "]".
           STOP RUN.
"#,
    );
    assert_eq!(out, ["TWELVE"], "{out:#?}");
}

/// NC107A `JUST-TEST-04`: `JUSTIFIED RIGHT` still decides which end pads and
/// which end is lost. A short group is pushed right; a group wider than the
/// receiver keeps its **rightmost** characters.
#[test]
fn a_justified_receiver_still_aligns_a_group_sender() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GRPJUST.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SHORT-GRP.
          05 SHORT-A PIC AAA VALUE "ABC".
       01 LONG-GRP.
          05 LONG-A PIC A(15) VALUE "ABCDEFGHIJKLMNO".
       01 RCV-JUST PIC A(7) JUSTIFIED RIGHT.
       PROCEDURE DIVISION.
       MAIN.
           MOVE SHORT-GRP TO RCV-JUST.
           DISPLAY "[" RCV-JUST "]".
           MOVE LONG-GRP TO RCV-JUST.
           DISPLAY "[" RCV-JUST "]".
           STOP RUN.
"#,
    );
    assert_eq!(out, ["[    ABC]", "[IJKLMNO]"], "{out:#?}");
}

/// NC105A `MOVE-TEST-F1-13`: a group distributes its bytes into its children
/// as they stand. The child is `PIC XBA09`, already-edited text — running the
/// edit again turned `"1 A05"` into `"1  0A"`.
#[test]
fn a_group_does_not_re_edit_an_alphanumeric_edited_child() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GRPCHILD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 HOST-GRP.
          05 EDITED-CHILD PICTURE IS XBA09.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "1 A05" TO HOST-GRP.
           DISPLAY "[" EDITED-CHILD "]".
           STOP RUN.
"#,
    );
    assert_eq!(out, ["[1 A05]"], "{out:#?}");
}

/// A `MOVE` into an alphanumeric-edited item from an **elementary** sender is
/// unchanged: there the PICTURE does impose its insertion characters. Pinning
/// the boundary, so the fix above is not read as "editing never happens".
#[test]
fn an_elementary_sender_still_edits_an_alphanumeric_edited_receiver() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ELEMAE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SRC-X  PIC X(5) VALUE "1A5XY".
       01 RCV-AE PICTURE IS XBA09.
       PROCEDURE DIVISION.
       MAIN.
           MOVE SRC-X TO RCV-AE.
           DISPLAY "[" RCV-AE "]".
           STOP RUN.
"#,
    );
    assert_eq!(out, ["[1 A05]"], "{out:#?}");
}

/// NC104A `MOVE-TEST-F1-29`: a `VALUE` clause on a **group** initialises the
/// group's bytes, so the child holds them. Written to the group's own slot it
/// reached nothing, and the numeric-edited child stayed empty.
#[test]
fn a_value_clause_on_a_group_reaches_its_children() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GRPVALUE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 MONEY-GRP VALUE IS "$123.45".
          05 MONEY-EDITED PICTURE IS $999.99.
       01 RCV-X PIC X(7).
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "[" MONEY-EDITED "]".
           MOVE MONEY-EDITED TO RCV-X.
           DISPLAY "[" RCV-X "]".
           STOP RUN.
"#,
    );
    assert_eq!(out, ["[$123.45]", "[$123.45]"], "{out:#?}");
}

/// The group `VALUE` is spread by each child's own width, not dropped into the
/// first one — the same distribution a group MOVE performs.
#[test]
fn a_group_value_is_split_across_every_child() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GRPVSPLIT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SPLIT-GRP VALUE IS "AB12CDE".
          05 PART-1 PIC XX.
          05 PART-2 PIC 99.
          05 PART-3 PIC XXX.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "[" PART-1 "][" PART-2 "][" PART-3 "]".
           STOP RUN.
"#,
    );
    assert_eq!(out, ["[AB][12][CDE]"], "{out:#?}");
}
