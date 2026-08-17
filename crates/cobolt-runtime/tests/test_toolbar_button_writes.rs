// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What a form's COBOL may change about a toolbar BUTTON while it runs.
//!
//! Its **colours** and its **tooltip**, and nothing else (operator, 2026-08-17).
//! The toolbar owns the layout — that is what keeps the buttons arranged the way
//! the developer built them, and there would be nothing to put a self-moving
//! button back.
//!
//! A refused write is a runtime **error**, not a no-op. That is the whole point:
//! a line that silently does nothing is how a developer loses an afternoon
//! wondering why the button did not move.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{Interpreter, StateUpdate};

/// Run a program with one toolbar button seeded the way a form host seeds it:
/// under its derived `<toolbar>-<group>-<button>` id and the class
/// `ToolbarButton`. Returns the run result and every state update it sent.
fn run_with_button(body: &str) -> (Result<(), String>, Vec<StateUpdate>) {
    let src = format!(
        "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-V PIC X(32).
       PROCEDURE DIVISION.
{body}
           STOP RUN.
"
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}\n{src}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, _display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.seed_objects(vec![(
        "TOOLBAR-1-GROUP-1-BUTTON-1".to_string(),
        "ToolbarButton".to_string(),
        vec![
            ("Tooltip".to_string(), "Save".to_string()),
            ("BackgroundColor".to_string(), String::new()),
            ("Width".to_string(), "32".to_string()),
        ],
    )]);
    let outcome = interp.run().map_err(|e| e.to_string());
    (outcome, state_rx.try_iter().collect())
}

#[test]
fn a_colour_and_a_tooltip_go_through() {
    let (outcome, updates) = run_with_button(
        "\
           MOVE \"#204080FF\" TO TOOLBAR-1-GROUP-1-BUTTON-1::BackgroundColor.
           MOVE \"Save the record\" TO TOOLBAR-1-GROUP-1-BUTTON-1::Tooltip.
           CALL \"COBOL-SET-PROPERTY\" USING
               \"TOOLBAR-1-GROUP-1-BUTTON-1\" \"IconColor\" \"#FFCC00FF\".
           INVOKE TOOLBAR-1-GROUP-1-BUTTON-1 \"SetProperty\"
               USING \"ForegroundColor\" \"#FFFFFFFF\".",
    );
    outcome.expect("every allowed write must run");

    let sent: Vec<(String, String)> = updates
        .iter()
        .map(|u| (u.prop.clone(), u.value.clone()))
        .collect();
    // `MOVE … TO x::Prop` arrives upper-cased (the member path is a COBOL word)
    // while the two CALL forms keep the case they were given. Property names are
    // matched case-insensitively everywhere for exactly that reason, so the
    // comparison here is too.
    for want in [
        ("BackgroundColor", "#204080FF"),
        ("Tooltip", "Save the record"),
        ("IconColor", "#FFCC00FF"),
        ("ForegroundColor", "#FFFFFFFF"),
    ] {
        assert!(
            sent.iter()
                .any(|(p, v)| p.eq_ignore_ascii_case(want.0) && v == want.1),
            "{want:?} never reached the host: {sent:?}"
        );
    }
    // Every update is addressed to the button, so the host knows where it goes.
    assert!(updates
        .iter()
        .all(|u| u.ctrl_id == "TOOLBAR-1-GROUP-1-BUTTON-1"));

    println!(
        "\n  Toolbar button writes, runtime — a colour and a tooltip through all three \
         doors (MOVE … TO x::Prop, CALL \"COBOL-SET-PROPERTY\", INVOKE … \"SetProperty\") \
         all run and reach the host: {} updates, every one addressed to the button\n",
        updates.len()
    );
}

#[test]
fn everything_else_is_refused_out_loud() {
    // Geometry, through each of the three doors.
    let cases: [(&str, &str); 3] = [
        (
            "MOVE … TO x::Width",
            "           MOVE \"200\" TO TOOLBAR-1-GROUP-1-BUTTON-1::Width.",
        ),
        (
            "CALL \"COBOL-SET-PROPERTY\"",
            "           CALL \"COBOL-SET-PROPERTY\" USING\n               \
             \"TOOLBAR-1-GROUP-1-BUTTON-1\" \"Height\" \"80\".",
        ),
        (
            "INVOKE … \"SetProperty\"",
            "           INVOKE TOOLBAR-1-GROUP-1-BUTTON-1 \"SetProperty\"\n               \
             USING \"CornerRadius\" \"0\".",
        ),
    ];
    for (door, body) in cases {
        let (outcome, updates) = run_with_button(body);
        let err = outcome.expect_err(&format!("{door} must be refused, not ignored"));
        assert!(
            err.contains("laid out by its toolbar"),
            "{door}: the error must say why: {err}"
        );
        assert!(
            err.contains("colours and its tooltip"),
            "{door}: …and what is allowed instead: {err}"
        );
        assert!(
            updates.is_empty(),
            "{door}: a refused write must not reach the host: {updates:?}"
        );
    }

    // A setter that writes something a button does not own at all is refused as a
    // METHOD, before it can pick a property.
    let (outcome, _) = run_with_button(
        "           INVOKE TOOLBAR-1-GROUP-1-BUTTON-1 \"SetCaption\" USING \"Nope\".",
    );
    let err = outcome.expect_err("a button has no caption to set");
    assert!(
        err.contains("not available on a toolbar button"),
        "the error must say the method is not the button's: {err}"
    );

    // Reading is never refused — a handler may ask a button anything.
    let (outcome, _) = run_with_button(
        "\
           MOVE TOOLBAR-1-GROUP-1-BUTTON-1::Tooltip TO WS-V.
           INVOKE TOOLBAR-1-GROUP-1-BUTTON-1 \"GetProperty\" USING \"Width\"
               RETURNING WS-V.",
    );
    outcome.expect("reads are always allowed");

    // And the rule is about BUTTONS: an ordinary control is untouched by it.
    let (outcome, updates) =
        run_with_button("           MOVE \"200\" TO LABEL-1::Width.");
    outcome.expect("a plain control still takes any property");
    assert_eq!(updates.len(), 1);

    println!(
        "\n  Toolbar button writes, refusals — Width/Height/CornerRadius refused through \
         all three doors with an error naming the reason and the allowed set, and NO state \
         update sent; a non-property setter (SetCaption) refused as a method; reads always \
         allowed; a plain control (LABEL-1::Width) unaffected\n"
    );
}
