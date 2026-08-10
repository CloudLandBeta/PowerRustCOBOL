// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 049 R30/AC15 — the FORM as a member-chain receiver: `me::<property>`
//! reads and assigns the form object's properties (they did neither before
//! 049), `me` and the form's own name address the SAME registry entry, and
//! `INVOKE ME "SetProperty"` lands on the form object instead of a phantom
//! "ME" control.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::form_host::{FormRequest, ROOT_HANDLE};
use cobolt_runtime::{Interpreter, StateUpdate};

/// Run `src` with a seeded DEMO-FORM form object and `me` bound to it.
/// No host thread: property access is registry-local in this task.
fn run_with_form(src: &str) -> (Vec<String>, Vec<StateUpdate>) {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let (req_tx, _req_rx) = mpsc::channel::<FormRequest>();
    let (_closed_tx, closed_rx) = mpsc::channel::<String>();

    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.set_form_host(req_tx, ROOT_HANDLE, "DEMO-FORM", closed_rx);
    // The universal-surface seed the host glue builds from the designed form
    // (build_object_seed's form entry).
    interp.seed_objects(vec![(
        "DEMO-FORM".to_string(),
        "Form".to_string(),
        vec![
            ("Title".to_string(), "Hello".to_string()),
            ("Width".to_string(), "640".to_string()),
            ("Height".to_string(), "480".to_string()),
            ("FormState".to_string(), "Ready".to_string()),
        ],
    )]);
    interp.run().expect("run failed");
    (
        display_rx.try_iter().collect(),
        state_rx.try_iter().collect(),
    )
}

#[test]
fn me_property_reads_the_designed_form_values() {
    // AC15 (read half): me::Width reads the designed width. Failed before
    // 049 — the root "ME" hit no registry entry and read empty.
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-W PIC 9(4).
       01 WS-T PIC X(10).
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-W.
           DISPLAY "W=" WS-W.
           MOVE me::Title TO WS-T.
           DISPLAY "T=" WS-T.
           STOP RUN.
"#;
    let (display, _) = run_with_form(src);
    assert!(
        display.iter().any(|l| l.contains("W=0640")),
        "me::Width must read the designed 640: {display:?}"
    );
    assert!(
        display.iter().any(|l| l.contains("T=Hello")),
        "me::Title must read the designed title: {display:?}"
    );
    println!("049 AC15 read — me::Width => 640, me::Title => Hello (2/2 designed values)");
}

#[test]
fn me_property_assignment_and_form_name_alias_share_one_entry() {
    // AC15 (assign half) + R30: MOVE into me::Title is applied, readable back
    // through BOTH spellings, and the StateUpdate carries the FORM name (the
    // key the host's form-level taps match) — never a phantom "ME".
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-T PIC X(10).
       PROCEDURE DIVISION.
           MOVE "Changed" TO me::Title.
           MOVE me::Title TO WS-T.
           DISPLAY "ME=" WS-T.
           MOVE DEMO-FORM::Title TO WS-T.
           DISPLAY "FORM=" WS-T.
           STOP RUN.
"#;
    let (display, updates) = run_with_form(src);
    assert!(
        display.iter().any(|l| l.contains("ME=Changed")),
        "assignment must be readable through me:: — {display:?}"
    );
    assert!(
        display.iter().any(|l| l.contains("FORM=Changed")),
        "the form-name spelling must see the same entry — {display:?}"
    );
    let title_updates: Vec<&StateUpdate> = updates
        .iter()
        .filter(|u| u.prop.eq_ignore_ascii_case("Title"))
        .collect();
    assert!(
        title_updates
            .iter()
            .all(|u| u.ctrl_id.eq_ignore_ascii_case("DEMO-FORM")),
        "StateUpdates must carry the form object name, not 'ME': {updates:?}"
    );
    assert!(
        !title_updates.is_empty(),
        "the write must notify the host: {updates:?}"
    );
    println!(
        "049 AC15 assign — me::Title write visible via me:: and DEMO-FORM:: \
         (1 shared entry); {} StateUpdate(s) keyed DEMO-FORM, 0 keyed ME",
        title_updates.len()
    );
}

#[test]
fn invoke_me_set_property_lands_on_the_form_object() {
    // R30 — the statement path: INVOKE ME "SetProperty" must write the form
    // object (the host's FormState mirror matches ctrl_id == form object).
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-S PIC X(10).
       PROCEDURE DIVISION.
           INVOKE ME "SetProperty" USING "FormState" "Waiting".
           MOVE me::FormState TO WS-S.
           DISPLAY "FS=" WS-S.
           STOP RUN.
"#;
    let (display, updates) = run_with_form(src);
    assert!(
        display.iter().any(|l| l.contains("FS=Waiting")),
        "the SetProperty write must be readable through me:: — {display:?}"
    );
    let fs: Vec<&StateUpdate> = updates
        .iter()
        .filter(|u| u.prop.eq_ignore_ascii_case("FormState"))
        .collect();
    assert!(
        !fs.is_empty() && fs.iter().all(|u| u.ctrl_id.eq_ignore_ascii_case("DEMO-FORM")),
        "FormState StateUpdate must be keyed by the form object: {updates:?}"
    );
    println!(
        "049 R30 statement path — INVOKE ME SetProperty(FormState) lands on \
         DEMO-FORM ({} update(s)), readable back as me::FormState=Waiting",
        fs.len()
    );
}

#[test]
fn embedded_geometry_reports_designed_values_and_stays_inert() {
    // AC17 (runtime half) — an Embedded form's geometry is its DESIGNED
    // value; assigning changes only the reported value (the registry), never
    // a window (the Pane surface drops every window command — T12's test
    // pins that side). FormFormat itself is readable, so COBOL can branch.
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-W PIC 9(4).
       01 WS-F PIC X(10).
       PROCEDURE DIVISION.
           MOVE me::FormFormat TO WS-F.
           DISPLAY "FMT=" WS-F.
           MOVE me::Width TO WS-W.
           DISPLAY "W1=" WS-W.
           MOVE 800 TO me::Width.
           MOVE me::Width TO WS-W.
           DISPLAY "W2=" WS-W.
           STOP RUN.
"#;
    let result = parse(tokenize(src, SourceFormat::Free));
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let (req_tx, _req_rx) = mpsc::channel::<FormRequest>();
    let (_closed_tx, closed_rx) = mpsc::channel::<String>();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.set_form_host(req_tx, ROOT_HANDLE, "EMB-FORM", closed_rx);
    interp.seed_objects(vec![(
        "EMB-FORM".to_string(),
        "Form".to_string(),
        vec![
            ("Width".to_string(), "300".to_string()),
            ("FormFormat".to_string(), "Embedded".to_string()),
        ],
    )]);
    interp.run().expect("run failed");
    let display: Vec<String> = display_rx.try_iter().collect();
    assert!(display.iter().any(|l| l.contains("FMT=Embedded")));
    assert!(
        display.iter().any(|l| l.contains("W1=0300")),
        "designed width: {display:?}"
    );
    assert!(
        display.iter().any(|l| l.contains("W2=0800")),
        "assigned value is REPORTED back: {display:?}"
    );
    // The write became a StateUpdate for the host — which, on a Pane
    // surface, applies no window command (pinned by the T12 host test).
    let w_updates: Vec<StateUpdate> = state_rx
        .try_iter()
        .filter(|u| u.prop.eq_ignore_ascii_case("Width"))
        .collect();
    assert_eq!(w_updates.len(), 1);
    assert_eq!(w_updates[0].value, "800");
    println!(
        "049 AC17 (runtime half) — FormFormat readable ('Embedded'), \
         me::Width designed 300 → assigned 800 reported back; 1 Width \
         StateUpdate for the host, which the Pane surface applies to no \
         window (T12). Standalone behaviour is spec 037's, untouched."
    );
}

#[test]
fn without_a_form_context_me_stays_inert() {
    // A console program (no set_form_host): me:: must not explode — the root
    // passes through unmapped and reads empty, exactly as before 049.
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PLAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-T PIC X(10).
       PROCEDURE DIVISION.
           MOVE me::Title TO WS-T.
           DISPLAY "T=[" WS-T "]".
           STOP RUN.
"#;
    let result = parse(tokenize(src, SourceFormat::Free));
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    let display: Vec<String> = display_rx.try_iter().collect();
    assert!(
        display.iter().any(|l| l.contains("T=[")),
        "console-mode me:: must stay a harmless empty read: {display:?}"
    );
    println!("049 — console mode (no form host): me::Title reads empty, no error");
}
