// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-005 AC6 (R9, Q6 decided: queue): while a `CALL … WITH WEBSOCKET
//! LINKAGE` is pending, the form keeps running frames, and a click on it is
//! queued and handled after the call, in order.
//!
//! The real host and the real interpreter, as `rcrun run-form` wires them,
//! driven off screen by [`cobolt_form_host::headless::run_headless`]. The
//! backend is a transport that holds the call until the test releases it.
//! Its own test binary: the transport is the process-wide default
//! (`set_default_transport_factory`), which registers once per process.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use cobolt_form_host::state::CtrlState;
use cobolt_form_host::{flatten_controls, FormHostConfig, NoHooks, Surface};
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};
use cobolt_wire::call::{CallReply, CallRequest, Transport};
use cobolt_wire::error::CallError;
use serde_json::{json, Value};

const FORM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="PENDING-FORM" title="Pending" width="420" height="200" main-form="true">
  <Control id="BTN-CALL" type="Button" x="20" y="20" w="120" h="28" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">Call</Property>
  </Control>
  <Control id="BTN-OTHER" type="Button" x="160" y="20" w="120" h="28" tab-order="1" z-order="1" visible="true" enabled="true">
    <Property name="Caption">Other</Property>
  </Control>
  <Control id="LBL-STATE" type="Label" x="20" y="70" w="380" h="28" tab-order="2" z-order="2" visible="true" enabled="true">
    <Property name="Caption">-</Property>
  </Control>
  <Control id="LBL-LOG" type="Label" x="20" y="110" w="380" h="28" tab-order="3" z-order="3" visible="true" enabled="true">
    <Property name="Caption">-</Property>
  </Control>
</Form>
"#;

/// The form's program: BTN-CALL calls the backend and records that the call
/// ended; BTN-OTHER records its own click.
const PROGRAM: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PENDING-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-QUIT        PIC 9      VALUE 0.
       01 COBOL-EVENT-ID    PIC X(64)  VALUE SPACES.
       01 COBOL-CONTROL-ID  PIC X(64)  VALUE SPACES.
       01 FORM-NAME         PIC X(64)  VALUE 'PENDING-FORM'.
       01 CUSTOMER-REC.
          05 CUST-ID        PIC 9(6)   VALUE 7.
          05 CUST-NAME      PIC X(20)  VALUE SPACES.
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
           MOVE \"CALLING\" TO LBL-STATE::Caption
      *>   The backend's GET-CUSTOMER fills CUSTOMER-REC by name.
           CALL \"GET-CUSTOMER\" WITH WEBSOCKET LINKAGE
               USING BY REFERENCE CUSTOMER-REC
               ON EXCEPTION MOVE \"FAILED\" TO CUST-NAME
           END-CALL
           MOVE CUST-NAME TO LBL-STATE::Caption
           STRING \"CALL-END \" DELIMITED BY SIZE INTO WS-LOG WITH POINTER WS-PTR
           MOVE WS-LOG TO LBL-LOG::Caption.
       ON-OTHER.
           STRING \"OTHER \" DELIMITED BY SIZE INTO WS-LOG WITH POINTER WS-PTR
           MOVE WS-LOG TO LBL-LOG::Caption.
";

/// What the held backend shares with the test.
#[derive(Default)]
struct Gate {
    entered: Mutex<Option<Instant>>,
    released: Mutex<Option<Instant>>,
    release: Mutex<Option<mpsc::Receiver<()>>>,
}

/// A backend that holds every call until the test releases it, then answers
/// GET-CUSTOMER with a name.
struct Held(Arc<Gate>);

impl Transport for Held {
    fn call(&mut self, request: &CallRequest) -> Result<CallReply, CallError> {
        *self.0.entered.lock().unwrap() = Some(Instant::now());
        if let Some(rx) = self.0.release.lock().unwrap().as_ref() {
            let _ = rx.recv_timeout(Duration::from_secs(20));
        }
        *self.0.released.lock().unwrap() = Some(Instant::now());
        assert_eq!(request.program, "GET-CUSTOMER");
        Ok(CallReply { args: vec![Some(r#"{"CUST-ID":7,"CUST-NAME":"Ana"}"#.to_owned())], returning: None, warnings: Vec::new() })
    }
}

/// The host and interpreter exactly as `rcrun run-form` wires them
/// (`cobolt-cli/src/form_gui.rs`), for one form and its program.
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

/// The value step `i` of the report read.
fn read_at(report: &Value, i: usize) -> String {
    report["steps"][i]["value"].as_str().unwrap_or_default().trim().to_owned()
}

#[test]
fn a_pending_call_keeps_the_form_live_and_queues_a_click_in_order() {
    let gate = Arc::new(Gate::default());
    let (release_tx, release_rx) = mpsc::channel::<()>();
    *gate.release.lock().unwrap() = Some(release_rx);
    {
        let gate = Arc::clone(&gate);
        let registered = cobolt_runtime::interpreter::remote::set_default_transport_factory(move || Some(Box::new(Held(Arc::clone(&gate))) as Box<dyn Transport>));
        assert!(registered, "this binary registers the only default transport");
    }
    // The test's side: once the call is in the backend, hold it long enough
    // for the script to click again and read the form, then release it.
    const HOLD: Duration = Duration::from_millis(2500);
    {
        let gate = Arc::clone(&gate);
        std::thread::spawn(move || {
            let start = Instant::now();
            while gate.entered.lock().unwrap().is_none() && start.elapsed() < Duration::from_secs(30) {
                std::thread::sleep(Duration::from_millis(10));
            }
            std::thread::sleep(HOLD);
            let _ = release_tx.send(());
        });
    }

    let script = vec![
        json!({"event": {"control": "BTN-CALL", "name": "onClick"}, "wait": false}), // 0
        json!({"wait_ms": 400}),                                                      // 1: frames while the call is held
        json!({"event": {"control": "BTN-OTHER", "name": "onClick"}, "wait": false}), // 2: a click during the call
        json!({"wait_ms": 300}),                                                      // 3
        json!({"read": {"control": "LBL-STATE", "property": "Caption"}}),             // 4
        json!({"read": {"control": "LBL-LOG", "property": "Caption"}}),               // 5
        json!({"wait_ms": 3500}),                                                     // 6: the release comes in here
        json!({"read": {"control": "LBL-STATE", "property": "Caption"}}),             // 7
        json!({"read": {"control": "LBL-LOG", "property": "Caption"}}),               // 8
    ];
    let t0 = Instant::now();
    let report = cobolt_form_host::headless::run_headless(host_config(), None, &script, Duration::from_secs(60), None);
    let elapsed = t0.elapsed();
    let entered = gate.entered.lock().unwrap().expect("the call reached the backend");
    let released = gate.released.lock().unwrap().expect("the call was answered");

    assert_eq!(report["timed_out"], false, "{report}");
    for (i, s) in report["steps"].as_array().unwrap().iter().enumerate() {
        assert_eq!(s["done"], true, "step {i}: {report}");
    }
    // While the call was held: the handler's first MOVE reached the form (the
    // host kept running frames and draining the program's updates), and the
    // second click's handler had not run.
    assert!(released.duration_since(entered) >= HOLD, "the call was held {:?}", released.duration_since(entered));
    assert_eq!(read_at(&report, 4), "CALLING", "the form showed the handler's state during the call: {report}");
    assert_eq!(read_at(&report, 5), "-", "BTN-OTHER's handler had not run during the call: {report}");
    // After the release: the call's handler finished first, then the queued click.
    assert_eq!(read_at(&report, 7), "Ana", "the answer landed: {report}");
    assert_eq!(read_at(&report, 8), "CALL-END OTHER", "call-end, then the queued click, in order: {report}");
    println!(
        "AC6: call held {:?}; during it LBL-STATE={:?} LBL-LOG={:?}; after it LBL-STATE={:?} LBL-LOG={:?}; run {:?}",
        released.duration_since(entered),
        read_at(&report, 4),
        read_at(&report, 5),
        read_at(&report, 7),
        read_at(&report, 8),
        elapsed
    );
}
