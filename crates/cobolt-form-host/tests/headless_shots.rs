// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **DESKTOP-WASM-001 T6 (R28): the headless steps `shot`, `size` and `answer`, and the
//! deterministic loop.**
//!
//! A script resizes the window and takes pictures at three sizes. It answers a program's open
//! dialog with a path and its save dialog with a cancel, so the run never waits on a person.
//! With the clock pinned, the frames are drawn at a virtual time that only the waits move.
//!
//! The real host and the real interpreter, as `rcrun run-form` wires them, driven off screen by
//! [`cobolt_form_host::headless::run_headless_with`]. The clock is pinned with `set_for_test`,
//! which holds for the whole process, so this file has one test.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use cobolt_form_host::file_dialog;
use cobolt_form_host::state::CtrlState;
use cobolt_form_host::{flatten_controls, FormHostConfig, NoHooks, Surface};
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};
use serde_json::{json, Value};

const FORM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="SHOTS-FORM" title="Shots" width="420" height="200" main-form="true">
  <Control id="BTN-OPEN" type="Button" x="20" y="20" w="120" h="28" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">Open</Property>
  </Control>
  <Control id="BTN-SAVE" type="Button" x="160" y="20" w="120" h="28" tab-order="1" z-order="1" visible="true" enabled="true">
    <Property name="Caption">Save</Property>
  </Control>
  <Control id="LBL-PATH" type="Label" x="20" y="70" w="380" h="28" tab-order="2" z-order="2" visible="true" enabled="true">
    <Property name="Caption">nothing opened</Property>
  </Control>
  <Control id="LBL-SAVE" type="Label" x="20" y="110" w="380" h="28" tab-order="3" z-order="3" visible="true" enabled="true">
    <Property name="Caption">nothing saved</Property>
  </Control>
</Form>
"#;

/// Open asks for a file and shows the path; Save asks where to save and says when it was cancelled.
const PROGRAM: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SHOTS-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-QUIT        PIC 9      VALUE 0.
       01 COBOL-EVENT-ID    PIC X(64)  VALUE SPACES.
       01 COBOL-CONTROL-ID  PIC X(64)  VALUE SPACES.
       01 FORM-NAME         PIC X(64)  VALUE 'SHOTS-FORM'.
       01 WS-PATH           PIC X(260) VALUE SPACES.
       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::\"INIT-FORM\" ( FORM-NAME )
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN \"BTN-OPEN\" PERFORM ON-OPEN
                   WHEN \"BTN-SAVE\" PERFORM ON-SAVE
               END-EVALUATE
           END-PERFORM
           STOP RUN.
       ON-OPEN.
           MOVE SPACES TO WS-PATH
           COBOL::\"OPEN-FILE-DIALOG\" ( \"Pick a file\" WS-PATH )
           MOVE WS-PATH TO LBL-PATH::Caption.
       ON-SAVE.
           MOVE SPACES TO WS-PATH
           COBOL::\"SAVE-FILE-DIALOG\" ( \"Save as\" WS-PATH )
           IF WS-PATH = SPACES
               MOVE \"cancelled\" TO LBL-SAVE::Caption
           ELSE
               MOVE WS-PATH TO LBL-SAVE::Caption
           END-IF.
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

/// The width and height a PNG's header gives (IHDR, right after the signature).
fn png_size(path: &Path) -> Option<(u32, u32)> {
    let bytes = std::fs::read(path).ok()?;
    let be = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    (bytes.get(1..4) == Some(b"PNG")).then_some(())?;
    Some((be(16)?, be(20)?))
}

#[test]
fn shot_and_size_steps_write_pictures() {
    cobolt_runtime::test_pins::set_for_test(Some("2026-01-02T03:04:05-03:00"), None);
    let dir = std::env::temp_dir().join(format!("prc-headless-shots-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let shots = dir.join("shots");
    let picked = dir.join("picked.csv");
    let script = vec![
        json!({"shot": "designed"}),                                         // 0
        json!({"size": [640, 360]}),                                         // 1
        json!({"shot": "wide"}),                                             // 2
        json!({"answer": {"path": picked.to_string_lossy()}}),               // 3
        json!({"event": {"control": "BTN-OPEN", "name": "onClick"}}),        // 4
        json!({"read": {"control": "LBL-PATH", "property": "Caption"}}),     // 5
        json!({"answer": {"cancel": true}}),                                 // 6
        json!({"event": {"control": "BTN-SAVE", "name": "onClick"}}),        // 7
        json!({"read": {"control": "LBL-SAVE", "property": "Caption"}}),     // 8
        json!({"size": [320, 240]}),                                         // 9
        json!({"shot": "narrow"}),                                           // 10
        json!({"shot": "../outside"}),                                       // 11: refused
    ];
    let t = Instant::now();
    let report = cobolt_form_host::headless::run_headless_with(host_config(), None, &script, Duration::from_secs(60), None, Some(&shots));
    let run_ms = t.elapsed().as_secs_f64() * 1000.0;

    // A Save As panel of the host itself (a Viewer's, a grid's export) takes a queued answer too.
    let ctx = egui::Context::default();
    let viewer_pdf = dir.join("viewer.pdf");
    file_dialog::queue_answer(Some(viewer_pdf.clone()));
    file_dialog::begin(&ctx, "SHOTS-TEST-SAVE-AS", file_dialog::DialogSpec::save());
    let save_as = file_dialog::take("SHOTS-TEST-SAVE-AS");
    cobolt_runtime::test_pins::clear_for_test();

    let steps = report["steps"].as_array().cloned().unwrap_or_default();
    let read = |i: usize| steps.get(i).and_then(|s| s["value"].as_str()).unwrap_or_default().trim().to_owned();
    let size_of = |name: &str| png_size(&shots.join(format!("{name}.png")));
    let (designed, wide, narrow) = (size_of("designed"), size_of("wide"), size_of("narrow"));
    let bytes = |name: &str| std::fs::metadata(shots.join(format!("{name}.png"))).map_or(0, |m| m.len());

    let checks = [
        ("every step but the last is done", steps.len() == 12 && steps[..11].iter().all(|s| s["done"] == true)),
        ("the run did not time out", report["timed_out"] == false),
        ("the first shot is the form's designed size, 420 x 200", designed == Some((420, 200))),
        ("after size [640, 360] the shot is 640 x 360", wide == Some((640, 360))),
        ("after size [320, 240] the shot is 320 x 240", narrow == Some((320, 240))),
        ("the open dialog took the scripted path", read(5) == picked.to_string_lossy()),
        ("the save dialog took the scripted cancel", read(8) == "cancelled"),
        ("a host Save As panel took the queued answer", save_as == Some(Some(viewer_pdf.clone()))),
        ("a shot named with a folder is refused", steps.get(11).is_some_and(|s| s["done"] == false) && !dir.join("outside.png").exists()),
        ("no answer was left unused", report.get("answers_unused").is_none()),
        ("deterministic: only the opening wait moved the clock (600 ms)", report["virtual_ms"] == 600),
    ];

    // The result block (GOLDEN RULE #7).
    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== headless_shots (DESKTOP-WASM-001 T6, R28) ==");
    println!("run: {} steps in {run_ms:.0} ms (virtual {} ms, real {} ms)", steps.len(), report["virtual_ms"], report["elapsed_ms"]);
    println!(
        "shots: designed {designed:?} {} bytes, wide {wide:?} {} bytes, narrow {narrow:?} {} bytes",
        bytes("designed"),
        bytes("wide"),
        bytes("narrow")
    );
    println!("dialogs: open -> {:?}, save -> {:?}, host Save As -> {save_as:?}", read(5), read(8));
    println!("refused shot: {}", steps.get(11).map_or(Value::Null, |s| s["why"].clone()));
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("checks: {passed} of {} passed", checks.len());
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(passed, checks.len(), "a check failed: see the result block\n{report}");
}
