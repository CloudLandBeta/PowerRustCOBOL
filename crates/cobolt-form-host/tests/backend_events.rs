// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-014 T9 (R2, R6): the backend's version notice and a warning reach the application's MAIN form as
//! `onBackendVersion` and `onBackendWarning`, and its handlers read what the backend said with `WS-VERSION`
//! and `WS-WARNING`. The real host and interpreter, as `rcrun run-form` wires them, driven off screen.
//! The call is made by a button; the backend is a transport that reports a `warn` hello and a warning.
//! Its own test binary: the default transport and the main form's address are process-wide.

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
<Form name="EVENTS-FORM" title="Events" width="420" height="200" main-form="true">
  <Control id="BTN-CALL" type="Button" x="20" y="20" w="120" h="28" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">Call</Property>
  </Control>
  <Control id="LBL-VERSION" type="Label" x="20" y="70" w="380" h="28" tab-order="1" z-order="1" visible="true" enabled="true">
    <Property name="Caption">-</Property>
  </Control>
  <Control id="LBL-WARNING" type="Label" x="20" y="110" w="380" h="28" tab-order="2" z-order="2" visible="true" enabled="true">
    <Property name="Caption">-</Property>
  </Control>
</Form>
"#;

const PROGRAM: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EVENTS-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-QUIT        PIC 9      VALUE 0.
       01 COBOL-EVENT-ID    PIC X(64)  VALUE SPACES.
       01 COBOL-CONTROL-ID  PIC X(64)  VALUE SPACES.
       01 FORM-NAME         PIC X(64)  VALUE 'EVENTS-FORM'.
       01 CUSTOMER-REC.
          05 CUST-ID        PIC 9(6)   VALUE 7.
          05 CUST-NAME      PIC X(20)  VALUE SPACES.
       01 WS-STATE          PIC X(10)  VALUE SPACES.
       01 WS-MSG            PIC X(80)  VALUE SPACES.
       01 WS-UPD            PIC X(10)  VALUE SPACES.
       01 WS-WARN           PIC X(80)  VALUE SPACES.
       01 WS-PROG           PIC X(20)  VALUE SPACES.
       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::\"INIT-FORM\" ( FORM-NAME )
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-EVENT-ID
                   WHEN \"onClick\"           PERFORM ON-CALL
                   WHEN \"onBackendVersion\"  PERFORM ON-VERSION
                   WHEN \"onBackendWarning\"  PERFORM ON-WARNING
               END-EVALUATE
           END-PERFORM
           STOP RUN.
       ON-CALL.
           CALL \"GET-CUSTOMER\" WITH WEBSOCKET LINKAGE
               USING BY REFERENCE CUSTOMER-REC
               ON EXCEPTION MOVE \"FAILED\" TO CUST-NAME
           END-CALL.
       ON-VERSION.
           COBOL::\"WS-VERSION\" ( WS-STATE WS-MSG WS-UPD )
           MOVE WS-STATE TO LBL-VERSION::Caption.
       ON-WARNING.
           COBOL::\"WS-WARNING\" ( WS-WARN WS-PROG )
           MOVE WS-PROG TO LBL-WARNING::Caption.
";

/// The backend: the older application is told so, and the program is deprecated.
struct Backend;

impl Transport for Backend {
    fn call(&mut self, r: &CallRequest) -> Result<CallReply, CallError> {
        self.call_observed(r, &mut cobolt_wire::call::NoObserver)
    }
    fn call_observed(&mut self, _r: &CallRequest, o: &mut dyn CallObserver) -> Result<CallReply, CallError> {
        o.backend_version("warn", "update the application", "1.2.0");
        o.warning("GET-CUSTOMER is deprecated: use GET-CUSTOMER-V2");
        Ok(CallReply { args: vec![Some(r#"{"CUST-ID":7,"CUST-NAME":"Ana"}"#.to_owned())], returning: None, warnings: vec!["GET-CUSTOMER is deprecated: use GET-CUSTOMER-V2".into()] })
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
    // As `rcrun run-form` does: the main form hears the backend's notices.
    cobolt_runtime::interpreter::remote::set_main_form_events(ev_tx.clone(), &form_object);
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
fn version_warning_reaches_the_main_form_handler() {
    assert!(cobolt_runtime::interpreter::remote::set_default_transport_factory(|| Some(Box::new(Backend) as Box<dyn Transport>)));
    let script = vec![
        json!({"event": {"control": "BTN-CALL", "name": "onClick"}, "wait": false}),
        json!({"wait_ms": 1500}),
        json!({"read": {"control": "LBL-VERSION", "property": "Caption"}}),
        json!({"read": {"control": "LBL-WARNING", "property": "Caption"}}),
    ];
    let report = cobolt_form_host::headless::run_headless(host_config(), None, &script, Duration::from_secs(60), None);
    assert_eq!(report["timed_out"], false, "{report}");
    println!("onBackendVersion handler read state {:?}; onBackendWarning handler read program {:?}", read_at(&report, 2), read_at(&report, 3));
    assert_eq!(read_at(&report, 2), "warn", "WS-VERSION in onBackendVersion: {report}");
    assert_eq!(read_at(&report, 3), "GET-CUSTOMER", "WS-WARNING's program in onBackendWarning: {report}");
}
