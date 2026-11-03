// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-016 T17 (rows 4, 6, 12): the headless `click` step is a real pointer
//! click (move, press, release) on the control's centre, so the host raises
//! the control's own events. Two buttons; each click is handled by the
//! program and the log shows which, in order.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use cobolt_form_host::state::CtrlState;
use cobolt_form_host::{flatten_controls, FormHostConfig, NoHooks, Surface};
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};
use serde_json::{json, Value};

const FORM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="CLICK-FORM" title="Pending" width="420" height="200" main-form="true">
  <Control id="BTN-CALL" type="Button" x="20" y="20" w="120" h="28" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">Call</Property>
    <Event name="onClick" paragraph="ON-CALL"><![CDATA[           CONTINUE.
]]></Event>
  </Control>
  <Control id="BTN-OTHER" type="Button" x="160" y="20" w="120" h="28" tab-order="1" z-order="1" visible="true" enabled="true">
    <Property name="Caption">Other</Property>
    <Event name="onClick" paragraph="ON-OTHER"><![CDATA[           CONTINUE.
]]></Event>
  </Control>
  <Control id="LBL-STATE" type="Label" x="20" y="70" w="380" h="28" tab-order="2" z-order="2" visible="true" enabled="true">
    <Property name="Caption">-</Property>
  </Control>
  <Control id="LBL-LOG" type="Label" x="20" y="110" w="380" h="28" tab-order="3" z-order="3" visible="true" enabled="true">
    <Property name="Caption">-</Property>
  </Control>
</Form>
"#;

const PROGRAM: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CLICK-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-QUIT        PIC 9      VALUE 0.
       01 COBOL-EVENT-ID    PIC X(64)  VALUE SPACES.
       01 COBOL-CONTROL-ID  PIC X(64)  VALUE SPACES.
       01 FORM-NAME         PIC X(64)  VALUE 'CLICK-FORM'.
       01 WS-LOG            PIC X(60)  VALUE SPACES.
       01 WS-PTR            PIC 99     VALUE 1.
       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::\"INIT-FORM\" ( FORM-NAME )
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN \"BTN-CALL\"  PERFORM ON-CALL
                   WHEN \"BTN-OTHER\" PERFORM ON-OTHER
               END-EVALUATE
           END-PERFORM
           STOP RUN.
       ON-CALL.
           STRING \"CALL \" DELIMITED BY SIZE INTO WS-LOG WITH POINTER WS-PTR
           MOVE WS-LOG TO LBL-LOG::Caption.
       ON-OTHER.
           STRING \"OTHER \" DELIMITED BY SIZE INTO WS-LOG WITH POINTER WS-PTR
           MOVE WS-LOG TO LBL-LOG::Caption.
";

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
            finished.store(true, Ordering::Relaxed);
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

#[test]
fn a_click_step_presses_the_control_under_the_pointer() {
    let script = vec![
        json!({"click": {"control": "BTN-OTHER"}}),
        json!({"click": {"control": "BTN-CALL"}}),
        json!({"click": {"control": "NO-SUCH"}}),
        json!({"read": {"control": "LBL-LOG", "property": "Caption"}}),
    ];
    let t0 = Instant::now();
    let report = cobolt_form_host::headless::run_headless(host_config(), None, &script, Duration::from_secs(30), None);
    let log = report["steps"][3]["value"].as_str().unwrap_or_default().trim().to_owned();
    assert_eq!(report["steps"][0]["done"], true, "{report}");
    assert_eq!(report["steps"][1]["done"], true, "{report}");
    assert_eq!(report["steps"][2]["done"], false, "a control that was not drawn is reported: {report}");
    assert_eq!(log, "OTHER CALL", "each click reached its own button, in order: {report}");
    println!("headless click: 2 real clicks reached their buttons in order ({log:?}), 1 absent control reported, run {:?}", t0.elapsed());
}
