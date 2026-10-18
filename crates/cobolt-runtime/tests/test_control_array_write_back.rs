// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 024 R17–R19 — what the operator types into a member of a bound repeating group
//! is written back to the COBOL table the group is bound to.
//!
//! Each test runs a real event loop (`COBOL-WAIT-EVENT`). The host reports an edit of a
//! member the way it does for a drawn card — under the member's instance id,
//! `"<group>.<group>-<n>.<member>"` — then a click wakes the handler, which displays the
//! table and the group's `CommitStatus`, `LastError` and `PendingEdits`.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};

const PROGRAM: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ARRLOOP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 EV-ID      PIC X(64).
       01 CTL-ID     PIC X(64).
       01 COBOL-QUIT PIC X VALUE "0".
       01 WS-I       PIC 9.
       01 WS-DISCARD PIC 9.
       01 ACTORS.
          05 ACTOR-ROW OCCURS 3 TIMES.
             10 ACTOR-ID    PIC 9(4).
             10 ACTOR-NAME  PIC X(12).
             10 ACTOR-FEE   PIC 9(4).
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-INIT-FORM" USING "T"
           MOVE 1 TO ACTOR-ID(1)
           MOVE "Ana" TO ACTOR-NAME(1)
           MOVE 100 TO ACTOR-FEE(1)
           MOVE 2 TO ACTOR-ID(2)
           MOVE "Bo" TO ACTOR-NAME(2)
           MOVE 200 TO ACTOR-FEE(2)
           MOVE 3 TO ACTOR-ID(3)
           MOVE "Cy" TO ACTOR-NAME(3)
           MOVE 300 TO ACTOR-FEE(3)
           PERFORM UNTIL COBOL-QUIT = "1"
               CALL "COBOL-WAIT-EVENT" USING EV-ID CTL-ID
               EVALUATE TRUE
                   WHEN EV-ID = "onClick"
                       PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 3
                           DISPLAY "ROW " ACTOR-ID(WS-I) "|" ACTOR-NAME(WS-I) "|" ACTOR-FEE(WS-I)
                       END-PERFORM
                       DISPLAY "STATUS " CARDS::CommitStatus "|" CARDS::LastError
                       DISPLAY "PENDING " CARDS::PendingEdits
                       DISPLAY "END"
                   WHEN EV-ID = "onRefresh"
                       SET WS-DISCARD TO CARDS::RefreshBinding()
                   WHEN OTHER
                       CONTINUE
               END-EVALUATE
           END-PERFORM
           STOP RUN.
"#;

/// The mappings the way the host seeds them, one per line.
const HOST_MAPPINGS: &str = "ACTOR-NAME\ttxt-name\tText\nACTOR-FEE\tnud-fee\tValue";

fn seed(writable: &str) -> Vec<(String, String)> {
    seed_mapped(writable, HOST_MAPPINGS)
}

fn seed_mapped(writable: &str, mappings: &str) -> Vec<(String, String)> {
    [
        ("IsRepeatingGroup", "true"),
        ("_BindingArray", "1"),
        ("_BindingKind", "CobolTable"),
        ("_BindingFields", "ACTOR-ID\nACTOR-NAME\nACTOR-FEE"),
        // member control \t property, per source field: the name on a TextBox, the fee on a NumericUpDown.
        ("_BindingMappings", mappings),
        ("_BindingWritable", writable),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect()
}

/// What one run displayed, line by line.
fn run(writable: &str, edits: &[(&str, &str, &str)]) -> Vec<String> {
    run_with(writable, edits, &[])
}

/// The same, with events sent before the click that shows the result.
fn run_with(writable: &str, edits: &[(&str, &str, &str)], events: &[&str]) -> Vec<String> {
    run_seeded(seed(writable), edits, events)
}

fn run_seeded(seed: Vec<(String, String)>, edits: &[(&str, &str, &str)], events: &[&str]) -> Vec<String> {
    let parsed = parse(tokenize(PROGRAM, SourceFormat::Free));
    assert!(parsed.diagnostics.iter().all(|d| d.severity != Severity::Error), "parse errors: {:?}", parsed.diagnostics);
    let (event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let (input_tx, input_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(parsed.program.expect("program"), event_rx, state_tx, display_tx);
    interp.set_input_channel(input_rx);
    interp.seed_objects([("Cards".to_owned(), "GroupBox".to_owned(), seed)]);
    for (id, prop, value) in edits {
        input_tx.send(StateUpdate::new((*id).to_owned(), (*prop).to_owned(), (*value).to_owned())).unwrap();
    }
    for event in events {
        event_tx.send(FormEvent::new("BTN", *event)).unwrap();
    }
    event_tx.send(FormEvent::new("BTN", "onClick")).unwrap();
    event_tx.send(FormEvent::quit()).unwrap();
    interp.run().expect("the program runs");
    display_rx.try_iter().collect()
}

fn rows(shown: &[String]) -> Vec<&str> {
    shown.iter().filter_map(|l| l.strip_prefix("ROW ")).collect()
}
/// The `CommitStatus` the handler saw, as a number: DISPLAY shows the code `00` as `0`.
fn status(shown: &[String]) -> u32 {
    line(shown, "STATUS ").split('|').next().unwrap().trim().parse().unwrap_or_else(|_| panic!("no status in {shown:?}"))
}
fn error(shown: &[String]) -> &str {
    line(shown, "STATUS ").split_once('|').map_or("", |(_, e)| e)
}
fn line<'a>(shown: &'a [String], prefix: &str) -> &'a str {
    shown.iter().find_map(|l| l.strip_prefix(prefix)).unwrap_or_else(|| panic!("no `{prefix}` line in {shown:?}"))
}

#[test]
fn a_member_edited_on_a_writable_binding_is_written_to_its_row_and_to_that_field_only() {
    let shown = run("1", &[("Cards.Cards-2.txt-name", "Text", "Bobby")]);
    assert_eq!(rows(&shown), ["0001|Ana         |0100", "0002|Bobby       |0200", "0003|Cy          |0300"], "{shown:?}");
    assert_eq!(status(&shown), 0, "{shown:?}");
}

#[test]
fn a_number_typed_into_a_numeric_member_lands_in_the_numeric_item() {
    let shown = run("1", &[("Cards.Cards-3.nud-fee", "Value", "450")]);
    assert_eq!(rows(&shown)[2], "0003|Cy          |0450", "{shown:?}");
    assert_eq!(status(&shown), 0);
}

#[test]
fn a_read_only_binding_never_writes() {
    let shown = run("0", &[("Cards.Cards-1.txt-name", "Text", "Nobody")]);
    assert_eq!(rows(&shown), ["0001|Ana         |0100", "0002|Bo          |0200", "0003|Cy          |0300"], "{shown:?}");
    assert_eq!(status(&shown), 91, "{shown:?}");
}

#[test]
fn a_value_the_field_cannot_hold_is_not_written_and_waits_for_repair() {
    let shown = run("1", &[("Cards.Cards-2.nud-fee", "Value", "lots")]);
    assert_eq!(rows(&shown)[1], "0002|Bo          |0200", "the table changed: {shown:?}");
    assert_eq!(status(&shown), 92);
    assert!(error(&shown).contains("ACTOR-FEE") && error(&shown).contains("lots"), "{shown:?}");
    assert_eq!(line(&shown, "PENDING "), "2\tnud-fee\tValue\tlots", "{shown:?}");
}

#[test]
fn repairing_the_member_writes_it_and_clears_what_was_waiting() {
    let shown = run("1", &[("Cards.Cards-2.nud-fee", "Value", "lots"), ("Cards.Cards-2.nud-fee", "Value", "250")]);
    assert_eq!(rows(&shown)[1], "0002|Bo          |0250", "{shown:?}");
    assert_eq!(status(&shown), 0);
    assert_eq!(line(&shown, "PENDING ").trim(), "", "{shown:?}");
}

#[test]
fn a_property_the_binding_does_not_map_and_a_card_that_is_not_a_row_are_left_alone() {
    // Mapped member, unmapped property; unmapped member; the fourth card of a three-row table.
    let shown = run(
        "1",
        &[("Cards.Cards-1.txt-name", "Visible", "false"), ("Cards.Cards-1.other", "Text", "x"), ("Cards.Cards-4.txt-name", "Text", "Dee")],
    );
    assert_eq!(rows(&shown), ["0001|Ana         |0100", "0002|Bo          |0200", "0003|Cy          |0300"], "{shown:?}");
    assert_eq!(status(&shown), 92);
    assert!(error(&shown).contains("room for 3"), "the fourth card has no row: {shown:?}");
}

#[test]
fn an_id_that_is_not_a_member_of_a_group_is_not_the_bindings_business() {
    let shown = run("1", &[("txt-name", "Text", "Plain"), ("Cards", "Text", "x")]);
    assert_eq!(rows(&shown), ["0001|Ana         |0100", "0002|Bo          |0200", "0003|Cy          |0300"], "{shown:?}");
    assert_eq!(line(&shown, "STATUS ").trim_matches('|').trim(), "", "nothing was reported: {shown:?}");
}

#[test]
fn re_reading_the_table_drops_what_was_waiting_for_repair() {
    let shown = run_with("1", &[("Cards.Cards-2.nud-fee", "Value", "lots")], &["onRefresh"]);
    assert_eq!(line(&shown, "PENDING ").trim(), "", "{shown:?}");
    assert_eq!(rows(&shown)[1], "0002|Bo          |0200");
}

/// Generated COBOL cannot break a line inside a literal, so it seeds the mappings
/// separated by `;`: every row of them is honoured — the second one too.
#[test]
fn mappings_seeded_by_generated_cobol_are_all_honoured() {
    let mappings = HOST_MAPPINGS.replace('\n', ";");
    let shown = run_seeded(
        seed_mapped("1", &mappings),
        &[("Cards.Cards-1.txt-name", "Text", "Alma"), ("Cards.Cards-3.nud-fee", "Value", "450")],
        &[],
    );
    assert_eq!(rows(&shown), ["0001|Alma        |0100", "0002|Bo          |0200", "0003|Cy          |0450"], "{shown:?}");
    assert_eq!(status(&shown), 0, "{shown:?}");
}
