// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The headless script's `dump` step: what the form holds for every control at that point — the
//! designed properties and what the program wrote — so an agent (or a parity check) can read the
//! settled state of a whole form in one step instead of one `read` per property.
//!
//! The real host and the real interpreter, as `rcrun run-form` wires them, driven off screen by
//! [`cobolt_form_host::headless::run_headless`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use cobolt_form_host::state::CtrlState;
use cobolt_form_host::{flatten_controls, FormHostConfig, NoHooks, Surface};
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};
use serde_json::{json, Value};

const FORM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="DUMP-FORM" title="Dump" width="420" height="200" main-form="true">
  <Control id="BTN-GO" type="Button" x="20" y="20" w="120" h="28" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">Go</Property>
  </Control>
  <Control id="LBL-OUT" type="Label" x="20" y="70" w="380" h="28" tab-order="1" z-order="1" visible="true" enabled="true">
    <Property name="Caption">before</Property>
  </Control>
  <Control id="LBL-HIDDEN" type="Label" x="20" y="110" w="380" h="28" tab-order="2" z-order="2" visible="false" enabled="true">
    <Property name="Caption">hidden by design</Property>
  </Control>
</Form>
"#;

/// A click on BTN-GO writes LBL-OUT's caption and shows the hidden label.
const PROGRAM: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DUMP-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-QUIT        PIC 9      VALUE 0.
       01 COBOL-EVENT-ID    PIC X(64)  VALUE SPACES.
       01 COBOL-CONTROL-ID  PIC X(64)  VALUE SPACES.
       01 FORM-NAME         PIC X(64)  VALUE 'DUMP-FORM'.
       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::\"INIT-FORM\" ( FORM-NAME )
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN \"BTN-GO\" PERFORM ON-GO
               END-EVALUATE
           END-PERFORM
           STOP RUN.
       ON-GO.
           MOVE \"after\" TO LBL-OUT::Caption
           SET LBL-HIDDEN::Visible TO TRUE.
";

/// The host and interpreter exactly as `rcrun run-form` wires them (`cobolt-cli/src/form_gui.rs`).
fn host_config() -> FormHostConfig {
    let form = cobolt_forms::load_form_from_str(FORM).expect("the form loads");
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(PROGRAM, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<_> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("{}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "the program parses: {errors:?}");
    let program = parsed.program.expect("a program");

    let mut flat: Vec<cobolt_forms::Control> = Vec::new();
    flatten_controls(&form.controls, &mut flat);
    flat.sort_by_key(|c| c.z_order);
    let state: HashMap<String, CtrlState> = cobolt_form_host::state::initial_state(&flat, &form.layers);
    let seed = cobolt_form_host::seeding::build_object_seed(&form, &flat, None, None);

    let (ev_tx, ev_rx) = mpsc::channel::<FormEvent>();
    let (input_tx, input_rx) = mpsc::channel::<StateUpdate>();
    let (state_tx, state_rx) = mpsc::channel::<StateUpdate>();
    let (display_tx, display_rx) = mpsc::channel::<String>();
    let pending = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicBool::new(false));
    let (form_req_tx, form_req_rx) = mpsc::channel::<cobolt_runtime::form_host::FormRequest>();
    let (closed_tx, closed_rx) = mpsc::channel::<String>();
    {
        let (pending, finished, form_req_tx) = (Arc::clone(&pending), Arc::clone(&finished), form_req_tx.clone());
        let control_ids: Vec<String> = form.controls.iter().map(|c| c.id.clone()).collect();
        let form_object = form.name.trim().to_ascii_uppercase();
        std::thread::spawn(move || {
            let mut interp = Interpreter::new_with_channels(program, ev_rx, state_tx, display_tx);
            interp.set_control_ids(control_ids);
            interp.set_input_channel(input_rx);
            interp.set_event_counter(pending);
            interp.set_form_host(form_req_tx, cobolt_runtime::form_host::ROOT_HANDLE, &form_object, closed_rx);
            interp.seed_objects(seed);
            let _ = interp.run();
            finished.store(true, std::sync::atomic::Ordering::Relaxed);
        });
    }
    let theme = cobolt_forms::theme::resolve_theme_id(None, None);
    FormHostConfig {
        form,
        flat,
        state,
        ev_tx,
        input_tx,
        state_rx,
        display_rx,
        pending,
        finished,
        form_req_rx,
        closed_tx,
        form_req_tx,
        form_source: None,
        child_theme: None,
        child_interpreter_setup: None,
        indexed_engine: Default::default(),
        shared_rust_bridge: None,
        fx_entrance: Default::default(),
        fx_exit: Default::default(),
        fx_restore: false,
        theme_pack: None,
        surface_theme: cobolt_forms::surface_theme::for_theme_id(&theme),
        icon_path: None,
        title_fallback: String::new(),
        surface: Surface::Window,
        hooks: Box::new(NoHooks),
    }
}

/// A property of a dumped control, found without regard to case: a property the program wrote arrives
/// upper-cased (`CAPTION`), as COBOL names do, and replaces the designed spelling (`Caption`).
fn prop<'a>(state: &'a Value, control: &str, name: &str) -> &'a Value {
    state[control]["props"]
        .as_object()
        .and_then(|p| p.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v))
        .unwrap_or(&Value::Null)
}

#[test]
fn a_dump_step_answers_every_controls_live_state_designed_and_written() {
    let script = vec![
        json!({"dump": true}),                                                        // 0: before the click
        json!({"event": {"control": "BTN-GO", "name": "onClick"}}),                   // 1
        json!({"dump": true}),                                                        // 2: after the handler
    ];
    let report = cobolt_form_host::headless::run_headless(host_config(), None, &script, Duration::from_secs(60), None);
    assert_eq!(report["timed_out"], false, "{report}");
    for (i, s) in report["steps"].as_array().unwrap().iter().enumerate() {
        assert_eq!(s["done"], true, "step {i}: {report}");
    }
    let before = &report["steps"][0]["state"];
    let after = &report["steps"][2]["state"];

    // Every control of the form is in the dump, with the designed properties.
    for id in ["BTN-GO", "LBL-OUT", "LBL-HIDDEN"] {
        assert!(before.get(id).is_some(), "{id} is in the dump: {before}");
    }
    assert_eq!(prop(before, "BTN-GO", "Caption"), "Go");
    assert_eq!(prop(before, "LBL-OUT", "Caption"), "before");
    assert_eq!((before["LBL-HIDDEN"]["visible"].as_bool(), before["LBL-OUT"]["visible"].as_bool()), (Some(false), Some(true)));

    // What the program wrote is there after it ran, and what it did not touch is as designed.
    assert_eq!(prop(after, "LBL-OUT", "Caption"), "after");
    assert_eq!(after["LBL-HIDDEN"]["visible"], true, "the handler showed the hidden label");
    assert_eq!(prop(after, "BTN-GO", "Caption"), "Go");

    // Two dumps of one state are the same text: keys and properties come in name order.
    let again = serde_json::to_string(after).unwrap();
    assert_eq!(again, serde_json::to_string(&serde_json::from_str::<Value>(&again).unwrap()).unwrap());
    let names: Vec<&String> = after.as_object().unwrap().keys().collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "controls in name order");
    println!("dump: {} controls; LBL-OUT.Caption {:?} -> {:?}; LBL-HIDDEN.visible {} -> {}", after.as_object().unwrap().len(), prop(before, "LBL-OUT", "Caption"), prop(after, "LBL-OUT", "Caption"), before["LBL-HIDDEN"]["visible"], after["LBL-HIDDEN"]["visible"]);
}
