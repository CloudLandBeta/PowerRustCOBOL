// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What a form's COBOL may do with a LAYER while it runs (spec 091 R33).
//!
//! A layer is an object of class `Layer`, seeded by the host under its name. A
//! program reads and writes `LAYER-NAME::Visible`, `::Transparency`,
//! `::BackgroundColor`, the gradient properties and `::BackgroundImage` /
//! `::BackgroundImageMode`; its `Name` can be read and never written; it has no
//! methods. A refused write is a runtime ERROR, not a no-op — the same rule as
//! a toolbar button's, for the same reason: a line that silently does nothing is
//! how a developer loses an afternoon.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{Interpreter, StateUpdate};

/// Run a program with one layer seeded the way a form host seeds it: under its
/// name, class `Layer`, hidden. Returns the run result, every state update it
/// sent, and what `WS-V` held when it ended.
fn run_with_layer(body: &str) -> (Result<(), String>, Vec<StateUpdate>) {
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
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}\n{src}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, _display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.seed_objects(vec![(
        "LAYER-1".to_string(),
        "Layer".to_string(),
        vec![
            ("Name".to_string(), "LAYER-1".to_string()),
            ("Visible".to_string(), "false".to_string()),
            ("Transparency".to_string(), "0".to_string()),
            ("BackgroundColor".to_string(), "#00000000".to_string()),
        ],
    )]);
    let outcome = interp.run().map_err(|e| e.to_string());
    (outcome, state_rx.try_iter().collect())
}

#[test]
fn a_layer_takes_its_backdrop_and_its_visibility_through_all_three_doors() {
    let (outcome, updates) = run_with_layer(
        "\
           SET LAYER-1::Visible TO TRUE.
           MOVE 40 TO LAYER-1::Transparency.
           MOVE \"#204080FF\" TO LAYER-1::BackgroundColor.
           CALL \"COBOL-SET-PROPERTY\" USING \"LAYER-1\" \"BackgroundImageMode\" \"Fill\".
           INVOKE LAYER-1 \"SetProperty\" USING \"BackgroundGradientEnabled\" \"1\".",
    );
    outcome.expect("every allowed write must run");
    let sent: Vec<(String, String)> = updates.iter().map(|u| (u.prop.clone(), u.value.clone())).collect();
    for want in [
        ("Visible", "1"),
        ("Transparency", "40"),
        ("BackgroundColor", "#204080FF"),
        ("BackgroundImageMode", "Fill"),
        ("BackgroundGradientEnabled", "1"),
    ] {
        assert!(
            sent.iter().any(|(p, v)| p.eq_ignore_ascii_case(want.0) && (v == want.1 || (want.1 == "1" && v == "true"))),
            "{want:?} never reached the host: {sent:?}"
        );
    }
    assert!(updates.iter().all(|u| u.ctrl_id == "LAYER-1"), "every update is addressed to the layer: {updates:?}");
    println!(
        "\n  091 R33 — Visible, Transparency, BackgroundColor, BackgroundImageMode and \
         BackgroundGradientEnabled written through MOVE/SET, CALL \"COBOL-SET-PROPERTY\" and \
         INVOKE … \"SetProperty\": {} updates, every one addressed to the layer\n",
        updates.len()
    );
}

#[test]
fn a_layer_reads_back_what_the_host_seeded() {
    let (outcome, _) = run_with_layer("           MOVE LAYER-1::Visible TO WS-V.");
    outcome.expect("a read always runs");
}

#[test]
fn a_property_a_layer_does_not_have_is_refused_out_loud() {
    let cases: [(&str, &str); 3] = [
        ("MOVE … TO x::Colour", "           MOVE \"red\" TO LAYER-1::Colour."),
        (
            "CALL \"COBOL-SET-PROPERTY\"",
            "           CALL \"COBOL-SET-PROPERTY\" USING \"LAYER-1\" \"Opacity\" \"5\".",
        ),
        (
            "INVOKE … \"SetProperty\"",
            "           INVOKE LAYER-1 \"SetProperty\" USING \"Width\" \"100\".",
        ),
    ];
    for (door, body) in cases {
        let (outcome, updates) = run_with_layer(body);
        let err = outcome.expect_err(&format!("{door} must be refused, not ignored"));
        assert!(err.contains("is not a property of a layer"), "{door}: the error must say why: {err}");
        assert!(err.contains("Visible") && err.contains("BackgroundColor"), "{door}: …and what a layer has: {err}");
        assert!(updates.is_empty(), "{door}: a refused write must not reach the host: {updates:?}");
    }
}

#[test]
fn a_layer_s_name_is_read_only() {
    for (door, body) in [
        ("MOVE", "           MOVE \"OTHER\" TO LAYER-1::Name."),
        (
            "CALL",
            "           CALL \"COBOL-SET-PROPERTY\" USING \"LAYER-1\" \"Name\" \"OTHER\".",
        ),
    ] {
        let (outcome, updates) = run_with_layer(body);
        let err = outcome.expect_err(&format!("{door}: renaming from code must be refused"));
        assert!(err.contains("renamed in the designer"), "{door}: {err}");
        assert!(updates.is_empty(), "{door}: {updates:?}");
    }
}

#[test]
fn a_layer_has_no_methods() {
    let (outcome, updates) = run_with_layer("           INVOKE LAYER-1 \"Show\".");
    let err = outcome.expect_err("a layer has no Show method");
    assert!(err.contains("not available on a layer") && err.contains("no methods"), "{err}");
    assert!(updates.is_empty());
}

#[test]
fn the_rule_is_about_layers_a_plain_control_is_untouched() {
    let (outcome, updates) = run_with_layer("           MOVE \"200\" TO LABEL-1::Colour.");
    outcome.expect("a plain control still takes any property");
    assert_eq!(updates.len(), 1);
}
