// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `CALL … RETURNING identifier` hands the callee's returning item back.
//!
//! The parser read the phrase and the interpreter never looked at it: the
//! caller's identifier kept whatever it held before the call, whichever
//! program was called and whatever that program's `PROCEDURE DIVISION …
//! RETURNING item` had set.
//!
//! The rule pinned here: when a called program returns, the value of its
//! RETURNING item is moved into the caller's identifier **as a `MOVE` would
//! put it** — the receiver's PICTURE converts, edits and truncates. A call that
//! returns no value (no RETURNING item in the callee, a paragraph, an
//! unresolved program) leaves the identifier exactly as it was.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Parse `src`, run it, and return what it DISPLAYed, trimmed line by line.
fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect()
}

/// The text between `NAME=[` and `]` on the line that carries it.
fn field(out: &[String], name: &str) -> String {
    out.iter()
        .find_map(|l| {
            l.strip_prefix(&format!("{name}=["))
                .and_then(|r| r.strip_suffix(']'))
                .map(str::to_owned)
        })
        .unwrap_or_else(|| panic!("expected {name}=[…]; got {out:?}"))
}

/// The caller, with `body` as its procedure and the data items every case
/// below shares; the callees follow it in the same source.
fn program(data: &str, body: &str, callees: &str) -> String {
    format!(
        "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
{data}
       PROCEDURE DIVISION.
       MAIN-PARA.
{body}
           STOP RUN.
       END PROGRAM CALLER.
{callees}
"
    )
}

/// A callee that doubles its argument into its RETURNING item.
const DOUBLE: &str = "
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DOUBLE.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-N   PIC 9(4).
       01  LK-R   PIC 9(5).
       PROCEDURE DIVISION USING LK-N RETURNING LK-R.
           COMPUTE LK-R = LK-N * 2
           GOBACK.
       END PROGRAM DOUBLE.
";

/// The plain case: the value the callee leaves in its RETURNING item arrives
/// in the caller's identifier.
#[test]
fn the_returning_item_arrives_in_the_callers_identifier() {
    let out = run_capture(&program(
        "       01  WS-N   PIC 9(4) VALUE 21.
       01  WS-R   PIC 9(5) VALUE 0.",
        "           CALL \"DOUBLE\" USING WS-N RETURNING WS-R
           DISPLAY \"R=[\" WS-R \"]\"
           DISPLAY \"N=[\" WS-N \"]\"",
        DOUBLE,
    ));
    assert_eq!(field(&out, "R"), "00042");
    assert_eq!(field(&out, "N"), "0021", "the USING argument is untouched");
}

/// The value is MOVEd, not copied byte for byte: the caller's PICTURE decides.
#[test]
fn the_value_is_moved_so_the_receivers_picture_applies() {
    let callee = "
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PRICE.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-PRICE   PIC 9(4)V99.
       PROCEDURE DIVISION RETURNING LK-PRICE.
           MOVE 12.34 TO LK-PRICE
           GOBACK.
       END PROGRAM PRICE.
";
    let out = run_capture(&program(
        "       01  WS-NUM    PIC 9(6)V99 VALUE 0.
       01  WS-SHORT  PIC 9(3)V9  VALUE 0.
       01  WS-WHOLE  PIC 99      VALUE 0.
       01  WS-EDIT   PIC ZZ9.99  VALUE 0.
       01  WS-TEXT   PIC X(8)    VALUE SPACES.",
        "           CALL \"PRICE\" RETURNING WS-NUM
           CALL \"PRICE\" RETURNING WS-SHORT
           CALL \"PRICE\" RETURNING WS-WHOLE
           CALL \"PRICE\" RETURNING WS-EDIT
           CALL \"PRICE\" RETURNING WS-TEXT
           DISPLAY \"NUM=[\" WS-NUM \"]\"
           DISPLAY \"SHORT=[\" WS-SHORT \"]\"
           DISPLAY \"WHOLE=[\" WS-WHOLE \"]\"
           DISPLAY \"EDIT=[\" WS-EDIT \"]\"
           DISPLAY \"TEXT=[\" WS-TEXT \"]\"",
        callee,
    ));
    assert_eq!(field(&out, "NUM"), "00001234", "wider receiver pads with zeros");
    assert_eq!(field(&out, "SHORT"), "0123", "narrower fraction truncates");
    assert_eq!(field(&out, "WHOLE"), "12", "integer receiver drops the fraction");
    assert_eq!(field(&out, "EDIT"), " 12.34", "an edited receiver edits");
    assert_eq!(field(&out, "TEXT"), "001234  ", "an alphanumeric receiver takes the digits, left-justified");
}

/// A RETURNING item declared in the callee's own WORKING-STORAGE is the
/// callee's private storage — it still comes back.
#[test]
fn a_working_storage_returning_item_comes_back_too() {
    let callee = "
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TAG.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-OUT   PIC X(6) VALUE SPACES.
       PROCEDURE DIVISION RETURNING WS-OUT.
           MOVE \"ABC\" TO WS-OUT
           GOBACK.
       END PROGRAM TAG.
";
    let out = run_capture(&program(
        "       01  WS-OUT   PIC X(6) VALUE \"CALLER\".
       01  WS-GOT   PIC X(6) VALUE SPACES.",
        "           CALL \"TAG\" RETURNING WS-GOT
           DISPLAY \"GOT=[\" WS-GOT \"]\"
           DISPLAY \"OUT=[\" WS-OUT \"]\"",
        callee,
    ));
    assert_eq!(field(&out, "GOT"), "ABC   ", "the callee's private item is what returns");
    assert_eq!(field(&out, "OUT"), "CALLER", "the caller's same-named item is not the callee's");
}

/// A callee whose RETURNING item is spelled like the caller's receiver: one
/// name, and the value still arrives.
#[test]
fn a_returning_item_spelled_like_the_receiver_still_arrives() {
    let callee = "
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SAME.
       DATA DIVISION.
       LINKAGE SECTION.
       01  WS-R   PIC 9(3).
       PROCEDURE DIVISION RETURNING WS-R.
           MOVE 7 TO WS-R
           GOBACK.
       END PROGRAM SAME.
";
    let out = run_capture(&program(
        "       01  WS-R   PIC 9(3) VALUE 0.",
        "           CALL \"SAME\" RETURNING WS-R
           DISPLAY \"R=[\" WS-R \"]\"",
        callee,
    ));
    assert_eq!(field(&out, "R"), "007");
}

/// A callee with no RETURNING item returns nothing: the identifier is exactly
/// what it was.
#[test]
fn a_callee_without_a_returning_item_leaves_the_identifier_alone() {
    let callee = "
       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUIET.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-X   PIC 9(3).
       PROCEDURE DIVISION USING LK-X.
           ADD 1 TO LK-X
           GOBACK.
       END PROGRAM QUIET.
";
    let out = run_capture(&program(
        "       01  WS-X   PIC 9(3) VALUE 10.
       01  WS-R   PIC 9(3) VALUE 77.",
        "           CALL \"QUIET\" USING WS-X RETURNING WS-R
           DISPLAY \"X=[\" WS-X \"]\"
           DISPLAY \"R=[\" WS-R \"]\"",
        callee,
    ));
    assert_eq!(field(&out, "X"), "011", "the call itself ran");
    assert_eq!(field(&out, "R"), "077", "nothing was returned, so nothing moved");
}

/// An unresolved program takes the ON EXCEPTION branch and the identifier is
/// untouched; the NOT ON EXCEPTION branch is skipped.
#[test]
fn an_unresolved_program_leaves_the_identifier_alone() {
    let out = run_capture(&program(
        "       01  WS-R    PIC 9(3) VALUE 55.
       01  WS-FLAG PIC XX   VALUE SPACES.",
        "           CALL \"NOSUCH\" RETURNING WS-R
               ON EXCEPTION     MOVE \"EX\" TO WS-FLAG
               NOT ON EXCEPTION MOVE \"OK\" TO WS-FLAG
           END-CALL
           DISPLAY \"R=[\" WS-R \"]\"
           DISPLAY \"FLAG=[\" WS-FLAG \"]\"",
        "",
    ));
    assert_eq!(field(&out, "R"), "055");
    assert_eq!(field(&out, "FLAG"), "EX");
}

/// `NOT ON EXCEPTION` runs after the value has arrived, so it can use it.
#[test]
fn not_on_exception_sees_the_returned_value() {
    let out = run_capture(&program(
        "       01  WS-N    PIC 9(4) VALUE 5.
       01  WS-R    PIC 9(5) VALUE 0.
       01  WS-SEEN PIC 9(5) VALUE 0.",
        "           CALL \"DOUBLE\" USING WS-N RETURNING WS-R
               NOT ON EXCEPTION MOVE WS-R TO WS-SEEN
           END-CALL
           DISPLAY \"SEEN=[\" WS-SEEN \"]\"",
        DOUBLE,
    ));
    assert_eq!(field(&out, "SEEN"), "00010");
}

/// A paragraph reached through CALL is not a program and has no RETURNING item.
#[test]
fn calling_a_paragraph_returns_nothing() {
    let out = run_capture(
        "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FLAT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-R   PIC 9(3) VALUE 31.
       PROCEDURE DIVISION.
       MAIN-PARA.
           CALL \"HELPER\" RETURNING WS-R
           DISPLAY \"R=[\" WS-R \"]\"
           STOP RUN.
       HELPER.
           DISPLAY \"in helper\".
",
    );
    assert_eq!(field(&out, "R"), "031");
}

/// The receiver can be a table element, a qualified name or a reference-modified
/// item — anything `MOVE` accepts as a receiver.
#[test]
fn the_receiver_can_be_any_data_reference() {
    let out = run_capture(&program(
        "       01  WS-N     PIC 9(4) VALUE 4.
       01  WS-TABLE.
           05  WS-T     PIC 9(5) OCCURS 3 VALUE 0.
       01  WS-REC.
           05  WS-ROW   PIC 9(5) VALUE 0.
       01  WS-BUF       PIC X(10) VALUE \"0123456789\".
       01  WS-MOVED     PIC X(10) VALUE \"0123456789\".
       01  WS-EIGHT     PIC 9(5)  VALUE 8.
       01  WS-I         PIC 9     VALUE 2.",
        "           CALL \"DOUBLE\" USING WS-N RETURNING WS-T (WS-I)
           CALL \"DOUBLE\" USING WS-N RETURNING WS-ROW OF WS-REC
           CALL \"DOUBLE\" USING WS-N RETURNING WS-BUF (4:3)
           MOVE WS-EIGHT TO WS-MOVED (4:3)
           DISPLAY \"T1=[\" WS-T (1) \"]\"
           DISPLAY \"T2=[\" WS-T (2) \"]\"
           DISPLAY \"ROW=[\" WS-ROW OF WS-REC \"]\"
           DISPLAY \"BUF=[\" WS-BUF \"]\"
           DISPLAY \"MOVED=[\" WS-MOVED \"]\"",
        DOUBLE,
    ));
    assert_eq!(field(&out, "T1"), "00000", "the other occurrences are untouched");
    assert_eq!(field(&out, "T2"), "00008");
    assert_eq!(field(&out, "ROW"), "00008");
    assert_eq!(
        field(&out, "BUF"),
        field(&out, "MOVED"),
        "a reference-modified receiver gets what the same MOVE would give it"
    );
    assert_ne!(field(&out, "BUF"), "0123456789", "and the call did write it");
}

/// A group RETURNING item is a record: it goes to the receiver as a group does.
#[test]
fn a_group_returning_item_moves_as_a_group() {
    let callee = "
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MKREC.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-REC.
           05  LK-NAME   PIC X(5).
           05  LK-QTY    PIC 9(3).
       PROCEDURE DIVISION RETURNING LK-REC.
           MOVE \"PEARS\" TO LK-NAME
           MOVE 12 TO LK-QTY
           GOBACK.
       END PROGRAM MKREC.
";
    let out = run_capture(&program(
        "       01  WS-REC.
           05  WS-NAME   PIC X(5) VALUE SPACES.
           05  WS-QTY    PIC 9(3) VALUE 0.",
        "           CALL \"MKREC\" RETURNING WS-REC
           DISPLAY \"NAME=[\" WS-NAME \"]\"
           DISPLAY \"QTY=[\" WS-QTY \"]\"",
        callee,
    ));
    assert_eq!(field(&out, "NAME"), "PEARS");
    assert_eq!(field(&out, "QTY"), "012");
}

/// Every call returns what that call's callee left: a static program keeps
/// its WORKING-STORAGE between calls, but each RETURNING is delivered afresh.
#[test]
fn each_call_delivers_its_own_value() {
    let callee = "
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TICKET.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-LAST   PIC 9(3) VALUE 0.
       LINKAGE SECTION.
       01  LK-NEXT   PIC 9(3).
       PROCEDURE DIVISION RETURNING LK-NEXT.
           ADD 1 TO WS-LAST
           MOVE WS-LAST TO LK-NEXT
           GOBACK.
       END PROGRAM TICKET.
";
    let out = run_capture(&program(
        "       01  WS-A   PIC 9(3) VALUE 0.
       01  WS-B   PIC 9(3) VALUE 0.
       01  WS-C   PIC 9(3) VALUE 0.",
        "           CALL \"TICKET\" RETURNING WS-A
           CALL \"TICKET\" RETURNING WS-B
           CALL \"TICKET\" RETURNING WS-C
           DISPLAY \"A=[\" WS-A \"]\"
           DISPLAY \"B=[\" WS-B \"]\"
           DISPLAY \"C=[\" WS-C \"]\"",
        callee,
    ));
    assert_eq!(
        (field(&out, "A"), field(&out, "B"), field(&out, "C")),
        ("001".into(), "002".into(), "003".into())
    );
}

/// RETURNING travels with the other phrases: BY CONTENT, BY REFERENCE and
/// BY VALUE arguments are bound as always and the value still comes back.
#[test]
fn it_combines_with_by_content_and_by_reference() {
    let callee = "
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MIXED.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-IN    PIC 9(3).
       01  LK-OUT   PIC 9(3).
       01  LK-SUM   PIC 9(4).
       PROCEDURE DIVISION USING LK-IN LK-OUT RETURNING LK-SUM.
           ADD 100 TO LK-IN
           MOVE LK-IN TO LK-OUT
           COMPUTE LK-SUM = LK-IN + LK-OUT
           GOBACK.
       END PROGRAM MIXED.
";
    let out = run_capture(&program(
        "       01  WS-IN    PIC 9(3) VALUE 5.
       01  WS-OUT   PIC 9(3) VALUE 0.
       01  WS-SUM   PIC 9(4) VALUE 0.",
        "           CALL \"MIXED\" USING BY CONTENT WS-IN BY REFERENCE WS-OUT
               RETURNING WS-SUM
           DISPLAY \"IN=[\" WS-IN \"]\"
           DISPLAY \"OUT=[\" WS-OUT \"]\"
           DISPLAY \"SUM=[\" WS-SUM \"]\"",
        callee,
    ));
    assert_eq!(field(&out, "IN"), "005", "BY CONTENT is put back");
    assert_eq!(field(&out, "OUT"), "105", "BY REFERENCE is written through");
    assert_eq!(field(&out, "SUM"), "0210");
}

/// A callee that itself calls another with RETURNING: each level delivers to
/// its own caller, and every alias unwinds.
#[test]
fn returning_chains_through_nested_calls() {
    let callees = "
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER-ONE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-MID   PIC 9(5) VALUE 0.
       LINKAGE SECTION.
       01  LK-N     PIC 9(4).
       01  LK-R     PIC 9(5).
       PROCEDURE DIVISION USING LK-N RETURNING LK-R.
           CALL \"DOUBLE\" USING LK-N RETURNING WS-MID
           ADD 1 TO WS-MID
           MOVE WS-MID TO LK-R
           GOBACK.
       END PROGRAM OUTER-ONE.
";
    let out = run_capture(&program(
        "       01  WS-N   PIC 9(4) VALUE 20.
       01  WS-R   PIC 9(5) VALUE 0.",
        &format!(
            "           CALL \"OUTER-ONE\" USING WS-N RETURNING WS-R
           DISPLAY \"R=[\" WS-R \"]\"
           DISPLAY \"N=[\" WS-N \"]\""
        ),
        &format!("{callees}{DOUBLE}"),
    ));
    assert_eq!(field(&out, "R"), "00041", "DOUBLE(20) = 40, plus 1");
    assert_eq!(field(&out, "N"), "0020");
}

/// A control's property is a receiver like any data item: what arrives is what
/// `MOVE` of the same item to that property would give.
#[test]
fn a_control_property_can_receive_the_value() {
    let callee = "
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TAG.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-OUT   PIC X(6) VALUE SPACES.
       PROCEDURE DIVISION RETURNING WS-OUT.
           MOVE \"ABC\" TO WS-OUT
           GOBACK.
       END PROGRAM TAG.
";
    let out = run_capture(&program(
        "       01  WS-SAME   PIC X(6) VALUE \"ABC\".",
        "           CALL \"TAG\" RETURNING LBL-1::Text
           MOVE WS-SAME TO LBL-2::Text
           DISPLAY \"CALLED=[\" LBL-1::Text \"]\"
           DISPLAY \"MOVED=[\" LBL-2::Text \"]\"",
        callee,
    ));
    assert_eq!(field(&out, "CALLED"), field(&out, "MOVED"));
    assert!(field(&out, "CALLED").starts_with("ABC"), "the property was written: {out:?}");
}
