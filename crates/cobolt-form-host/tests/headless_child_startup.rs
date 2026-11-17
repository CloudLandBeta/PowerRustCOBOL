// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **DESKTOP-WASM-001 R28: a deterministic headless run waits for a child window's start-up.**
//!
//! The main form's button works a while (a loop), then opens a form modally (`OpenFormSync`). The
//! opened form's start-up takes real time (a longer loop), then hands the main form a value
//! (`super::SetProperty`) and closes. The main form shows that value on a label when
//! `OpenFormSync` returns.
//!
//! The script raises the click without waiting (as for a modal form), waits 100 ms of virtual time
//! (less than the 450 ms after which the opened form's lifecycle events would start another round)
//! and reads the label. Virtual time costs almost no real time, so the run used to read the label
//! while the child was still starting: the harness waited only for the form on the pane, and that
//! form is held by its modal child. Now the open child windows are waited for too, and a window
//! opened while the harness waited (the click's loop comes first) is waited for in another round,
//! so the label holds the value. Found by the pixel run of 2.0.573 (PowerCobolCraft's title form:
//! the world it opens was pictured before its start-up had finished).
//!
//! The real host and the real interpreter, driven off screen by
//! [`cobolt_form_host::headless::run_headless_with`]. The clock is pinned with `set_for_test`,
//! which holds for the whole process, so this file has one test.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use cobolt_form_host::state::CtrlState;
use cobolt_form_host::{flatten_controls, FormHostConfig, FormSource, NoHooks, Surface};
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};
use serde_json::json;

const FORM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="TITLE-FORM" title="Title" width="420" height="200" main-form="true">
  <Control id="BTN-PLAY" type="Button" x="20" y="20" w="120" h="28" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">Play</Property>
  </Control>
  <Control id="LBL-OUT" type="Label" x="20" y="70" w="380" h="28" tab-order="1" z-order="1" visible="true" enabled="true">
    <Property name="Caption">not back yet</Property>
  </Control>
</Form>
"#;

/// The button opens WORLD modally and shows what it handed back.
const PROGRAM: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TITLE-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-QUIT        PIC 9      VALUE 0.
       01 COBOL-EVENT-ID    PIC X(64)  VALUE SPACES.
       01 COBOL-CONTROL-ID  PIC X(64)  VALUE SPACES.
       01 FORM-NAME         PIC X(64)  VALUE 'TITLE-FORM'.
       01 WS-H              PIC X(8).
       01 WS-RESULT         PIC X(40).
       01 WS-I              PIC 9(9)   VALUE 0.
       01 WS-N              PIC 9(9)   VALUE 0.
       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::\"INIT-FORM\" ( FORM-NAME )
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN \"BTN-PLAY\" PERFORM ON-PLAY
               END-EVALUATE
           END-PERFORM
           STOP RUN.
       ON-PLAY.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 50000
               ADD 1 TO WS-N
           END-PERFORM
           INVOKE ME::\"OpenFormSync\"(\"WORLD\") RETURNING WS-H
           INVOKE ME::\"GetProperty\"(\"Probe\") RETURNING WS-RESULT
           MOVE WS-RESULT TO LBL-OUT::Caption.
";

/// The opened form: a start-up that takes real time, then the value handed back, then the close.
const CHILD: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. WORLD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-I              PIC 9(9)   VALUE 0.
       01 WS-N              PIC 9(9)   VALUE 0.
       PROCEDURE DIVISION.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > LOOPS
               ADD 1 TO WS-N
           END-PERFORM
           INVOKE SUPER::\"SetProperty\"(\"Probe\", \"world started\").
           INVOKE ME::\"Close\"().
           STOP RUN.
";

/// The start-up loop's length: long enough that a run which does not wait reads the label first.
const LOOPS: u32 = 200_000;

fn program_from(src: &str) -> cobolt_ast::program::Program {
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<_> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("{}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "the program parses: {errors:?}");
    parsed.program.expect("a program")
}

fn host_config() -> FormHostConfig {
    let form = cobolt_forms::load_form_from_str(FORM).expect("the form loads");
    let program = program_from(PROGRAM);

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
    let source: FormSource = Box::new(|id: &str| {
        if id.eq_ignore_ascii_case("WORLD") {
            Ok((cobolt_forms::Form::new("WORLD", "World", 240, 160), program_from(&CHILD.replace("LOOPS", &LOOPS.to_string()))))
        } else {
            Err(format!("no form named '{id}'"))
        }
    });
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
        form_source: Some(source),
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
fn a_deterministic_run_waits_for_a_child_windows_start_up() {
    cobolt_runtime::test_pins::set_for_test(Some("2026-01-02T03:04:05-03:00"), Some(20_261_114));
    let script = vec![
        json!({"event": {"control": "BTN-PLAY", "name": "onClick"}, "wait": false}), // 0
        json!({"wait_ms": 100}),                                                      // 1
        json!({"read": {"control": "LBL-OUT", "property": "Caption"}}),               // 2
    ];
    let t = Instant::now();
    let report = cobolt_form_host::headless::run_headless_with(host_config(), None, &script, Duration::from_secs(120), None, None);
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    cobolt_runtime::test_pins::clear_for_test();

    let caption = report["steps"][2]["value"].as_str().unwrap_or_default().trim().to_owned();
    let all_done = report["steps"].as_array().is_some_and(|s| s.len() == 3 && s.iter().all(|s| s["done"] == true));
    let windows = report["windows"].as_array().map_or(usize::MAX, Vec::len);
    let checks = [
        ("every step was done", all_done),
        ("the run did not time out", report["timed_out"] == false),
        ("the label holds what the child handed back after its start-up", caption == "world started"),
        ("the child closed: no window is left open", windows == 0),
        ("the run took 100 ms of virtual time and 600 ms before the script (700 ms)", report["virtual_ms"] == 700),
    ];

    // The result block (GOLDEN RULE #7).
    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== headless_child_startup (DESKTOP-WASM-001 R28) ==");
    println!("fixture: a button opens a form modally; its start-up loops {LOOPS} times, hands a value back and closes");
    println!("run: {ms:.0} ms real, {} ms virtual; label: {caption:?}; windows left open: {windows}", report["virtual_ms"]);
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("checks: {passed} of {} passed", checks.len());
    assert_eq!(passed, checks.len(), "a check failed: see the result block\n{report}");
}
