// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-014 T13 (R13, D-Progress): a form declares with `COBOL::"WS-SHOW-PROGRESS"` where the next long call shows its progress. The real
//! host and interpreter, as `rcrun run-form` wires them, driven off screen: the progress bar and the label follow the backend's progress
//! frames while the interpreter is blocked in the call, and a click on the declared cancel button ends the call with `application 8000`.
//! Its own test binary: the default transport is process-wide.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use cobolt_form_host::state::CtrlState;
use cobolt_form_host::{flatten_controls, FormHostConfig, NoHooks, Surface};
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};
use cobolt_wire::call::{CallObserver, CallReply, CallRequest, Transport};
use cobolt_wire::error::CallError;
use serde_json::{json, Value};

const FORM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="PROGRESS-FORM" title="Progress" width="420" height="220" main-form="true">
  <Control id="BTN-RUN" type="Button" x="20" y="20" w="120" h="28" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">Run</Property>
  </Control>
  <Control id="BTN-CANCEL" type="Button" x="160" y="20" w="120" h="28" tab-order="1" z-order="1" visible="true" enabled="true">
    <Property name="Caption">Cancel</Property>
  </Control>
  <Control id="PRG-1" type="ProgressBar" x="20" y="70" w="380" h="22" tab-order="2" z-order="2" visible="true" enabled="true"></Control>
  <Control id="LBL-MSG" type="Label" x="20" y="110" w="380" h="28" tab-order="3" z-order="3" visible="true" enabled="true">
    <Property name="Caption">-</Property>
  </Control>
  <Control id="LBL-END" type="Label" x="20" y="150" w="380" h="28" tab-order="4" z-order="4" visible="true" enabled="true">
    <Property name="Caption">-</Property>
  </Control>
</Form>
"#;

const PROGRAM: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PROGRESS-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-QUIT        PIC 9      VALUE 0.
       01 COBOL-EVENT-ID    PIC X(64)  VALUE SPACES.
       01 COBOL-CONTROL-ID  PIC X(64)  VALUE SPACES.
       01 FORM-NAME         PIC X(64)  VALUE 'PROGRESS-FORM'.
       01 CUSTOMER-REC.
          05 CUST-ID        PIC 9(6)   VALUE 7.
       01 WS-CLASS          PIC X(16)  VALUE SPACES.
       01 WS-CODE           PIC X(8)   VALUE SPACES.
       01 WS-MSG            PIC X(80)  VALUE SPACES.
       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::\"INIT-FORM\" ( FORM-NAME )
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN \"BTN-RUN\" PERFORM ON-RUN
               END-EVALUATE
           END-PERFORM
           STOP RUN.
       ON-RUN.
           COBOL::\"WS-SHOW-PROGRESS\" ( PRG-1 LBL-MSG BTN-CANCEL )
           CALL \"BIG-JOB\" WITH WEBSOCKET LINKAGE
               USING BY REFERENCE CUSTOMER-REC
               ON EXCEPTION
                   COBOL::\"WS-LAST-ERROR\" ( WS-CLASS WS-CODE WS-MSG )
                   MOVE WS-CODE TO LBL-END::Caption
           END-CALL.
";

/// A long job: it reports 40 % and then waits, up to 20 s, for the host to ask it to cancel.
struct Job;

impl Transport for Job {
    fn call(&mut self, r: &CallRequest) -> Result<CallReply, CallError> {
        self.call_observed(r, &mut cobolt_wire::call::NoObserver)
    }
    fn call_observed(&mut self, _r: &CallRequest, o: &mut dyn CallObserver) -> Result<CallReply, CallError> {
        o.progress(40, "halfway");
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(20) {
            if o.cancel_requested() {
                return Err(CallError::new(cobolt_wire::error::ErrorClass::Application, cobolt_wire::error::codes::CANCELLED, "the call was cancelled"));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(CallReply::default())
    }
}

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
    let form_object = form.name.trim().to_ascii_uppercase();
    {
        let (pending, finished, form_req_tx) = (Arc::clone(&pending), Arc::clone(&finished), form_req_tx.clone());
        let control_ids: Vec<String> = form.controls.iter().map(|c| c.id.clone()).collect();
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

fn read_at(report: &Value, i: usize) -> String {
    report["steps"][i]["value"].as_str().unwrap_or_default().trim().to_owned()
}

#[test]
fn progress_frames_reach_the_declared_controls_and_the_cancel_button_ends_the_call() {
    assert!(cobolt_runtime::interpreter::remote::set_default_transport_factory(|| Some(Box::new(Job) as Box<dyn Transport>)));
    let script = vec![
        json!({"event": {"control": "BTN-RUN", "name": "onClick"}, "wait": false}),
        json!({"wait_ms": 800}),
        json!({"read": {"control": "PRG-1", "property": "Value"}}),
        json!({"read": {"control": "LBL-MSG", "property": "Caption"}}),
        json!({"read": {"control": "LBL-END", "property": "Caption"}}),
        json!({"event": {"control": "BTN-CANCEL", "name": "onClick"}, "wait": false}),
        json!({"wait_ms": 1500}),
        json!({"read": {"control": "LBL-END", "property": "Caption"}}),
    ];
    let report = cobolt_form_host::headless::run_headless(host_config(), None, &script, Duration::from_secs(60), None);
    assert_eq!(report["timed_out"], false, "{report}");
    println!(
        "during the call: bar={:?} label={:?} end={:?}; after the cancel click: end={:?}",
        read_at(&report, 2),
        read_at(&report, 3),
        read_at(&report, 4),
        read_at(&report, 7)
    );
    assert_eq!(read_at(&report, 2), "40", "the bar followed the frame while the interpreter was blocked: {report}");
    assert_eq!(read_at(&report, 3), "halfway", "{report}");
    assert_eq!(read_at(&report, 4), "-", "the call was still pending: {report}");
    assert_eq!(read_at(&report, 7), "8000", "the click ended the call with application 8000: {report}");
}
