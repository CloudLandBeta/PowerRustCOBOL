// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **DESKTOP-WASM-001 T8 (R28, AC20): a deterministic headless run draws the same frames twice.**
//!
//! The fixture form has the three things that used to move with the real clock:
//! - a MatrixRain entrance effect;
//! - a Timer whose handler counts its ticks on a label;
//! - a Snackbar a button raises.
//!
//! The script takes a picture while the effect plays, after it ends, while the snackbar arrives and
//! once it is up. The run is made twice with the clock pinned, and every picture must be the same
//! bytes both times. The pictures must also differ from each other where something moved, so the
//! test cannot pass on a form that draws nothing.
//!
//! The real host and the real interpreter, as `rcrun run-form` wires them, driven off screen by
//! [`cobolt_form_host::headless::run_headless_with`]. The clock is pinned with `set_for_test`,
//! which holds for the whole process, so this file has one test.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use cobolt_form_host::state::CtrlState;
use cobolt_form_host::{flatten_controls, FormHostConfig, NoHooks, Surface};
use cobolt_forms::window_fx::{Easing, FxSpec, WindowEffect};
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};
use serde_json::{json, Value};

const FORM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="CLOCK-FORM" title="Clock" width="420" height="240" main-form="true">
  <Control id="BTN-SHOW" type="Button" x="20" y="20" w="120" h="28" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">Show</Property>
  </Control>
  <Control id="LBL-TICKS" type="Label" x="20" y="70" w="380" h="28" tab-order="1" z-order="1" visible="true" enabled="true">
    <Property name="Caption">no tick yet</Property>
  </Control>
  <Control id="TMR-1" type="Timer" x="360" y="20" w="40" h="40" tab-order="2" z-order="2" visible="true" enabled="true">
    <Property name="Interval">100</Property>
  </Control>
  <Control id="SNB-1" type="Snackbar" x="0" y="0" w="24" h="24" tab-order="3" z-order="3" visible="true" enabled="true">
    <Property name="Text">Saved</Property>
  </Control>
</Form>
"#;

/// The timer counts its ticks on the label; the button raises the snackbar.
const PROGRAM: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CLOCK-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-QUIT        PIC 9      VALUE 0.
       01 COBOL-EVENT-ID    PIC X(64)  VALUE SPACES.
       01 COBOL-CONTROL-ID  PIC X(64)  VALUE SPACES.
       01 FORM-NAME         PIC X(64)  VALUE 'CLOCK-FORM'.
       01 WS-TICKS          PIC 9(4)   VALUE 0.
       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::\"INIT-FORM\" ( FORM-NAME )
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN \"TMR-1\" PERFORM ON-TICK
                   WHEN \"BTN-SHOW\" PERFORM ON-SHOW
               END-EVALUATE
           END-PERFORM
           STOP RUN.
       ON-TICK.
           ADD 1 TO WS-TICKS
           MOVE WS-TICKS TO LBL-TICKS::Caption.
       ON-SHOW.
           SNB-1::Show().
";

/// The host and interpreter exactly as `rcrun run-form` wires them (`cobolt-cli/src/form_gui.rs`),
/// with a MatrixRain entrance.
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
        fx_entrance: FxSpec { effect: WindowEffect::MatrixRain, duration_ms: 1500, easing: Easing::EaseOut },
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

/// The pictures the script takes, in order.
const SHOTS: [&str; 4] = ["effect-playing", "effect-done", "snackbar-arriving", "snackbar-up"];

/// One run of the script: its report, its real time in ms, and the bytes of each picture.
fn run_once(dir: &Path) -> (Value, f64, Vec<Vec<u8>>) {
    let _ = std::fs::remove_dir_all(dir);
    let script = vec![
        json!({"shot": SHOTS[0]}),                                       // 0: at 600 ms, MatrixRain playing
        json!({"wait_ms": 4000}),                                        // 1
        json!({"shot": SHOTS[1]}),                                       // 2: the effect is over
        json!({"event": {"control": "BTN-SHOW", "name": "onClick"}}),    // 3
        json!({"wait_ms": 100}),                                         // 4
        json!({"shot": SHOTS[2]}),                                       // 5: the snackbar on its way in
        json!({"wait_ms": 1000}),                                        // 6
        json!({"shot": SHOTS[3]}),                                       // 7: the snackbar up
        json!({"read": {"control": "LBL-TICKS", "property": "Caption"}}), // 8
    ];
    let t = Instant::now();
    let report = cobolt_form_host::headless::run_headless_with(host_config(), None, &script, Duration::from_secs(120), None, Some(dir));
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    let pictures = SHOTS.iter().map(|s| std::fs::read(dir.join(format!("{s}.png"))).unwrap_or_default()).collect();
    (report, ms, pictures)
}

#[test]
fn deterministic_mode_repeats_frames() {
    cobolt_runtime::test_pins::set_for_test(Some("2026-01-02T03:04:05-03:00"), Some(20_261_114));
    let base = std::env::temp_dir().join(format!("prc-deterministic-mode-{}", std::process::id()));
    let (first, first_ms, a) = run_once(&base.join("a"));
    let (second, second_ms, b) = run_once(&base.join("b"));
    // Virtual time ends with the run: the host's clock is the real one again.
    let real_again = cobolt_runtime::test_pins::virtual_elapsed().is_none();
    cobolt_runtime::test_pins::clear_for_test();

    let all_done = |r: &Value| r["steps"].as_array().is_some_and(|s| s.len() == 9 && s.iter().all(|s| s["done"] == true));
    let ticks = |r: &Value| r["steps"][8]["value"].as_str().unwrap_or_default().trim().to_owned();
    let identical = a.iter().zip(&b).filter(|(x, y)| !x.is_empty() && x == y).count();
    let differ = |i: usize, j: usize| !a[i].is_empty() && a[i] != a[j];

    let checks = [
        ("both runs did every step", all_done(&first) && all_done(&second)),
        ("neither run timed out", first["timed_out"] == false && second["timed_out"] == false),
        ("every picture was written", a.iter().chain(&b).all(|p| !p.is_empty())),
        ("every picture is the same in both runs", identical == SHOTS.len()),
        ("the effect moved: playing and done differ", differ(0, 1)),
        ("the snackbar moved: arriving and up differ", differ(2, 3)),
        ("the snackbar shows: up and before it differ", differ(1, 3)),
        ("the timer ticked, the same count in both runs", !ticks(&first).is_empty() && ticks(&first) != "no tick yet" && ticks(&first) == ticks(&second)),
        ("both runs ended at the same virtual time, 5 700 ms", first["virtual_ms"] == 5700 && second["virtual_ms"] == 5700),
        ("after the run the host's clock is real again", real_again),
    ];

    // The result block (GOLDEN RULE #7).
    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== deterministic_mode (DESKTOP-WASM-001 T8, R28, AC20) ==");
    println!("fixture: MatrixRain entrance, a 100 ms Timer counting on a label, a Snackbar; {} pictures per run", SHOTS.len());
    println!(
        "run 1: {first_ms:.0} ms real, {} ms virtual, ticks {}; run 2: {second_ms:.0} ms real, {} ms virtual, ticks {}",
        first["virtual_ms"],
        ticks(&first),
        second["virtual_ms"],
        ticks(&second)
    );
    for (i, s) in SHOTS.iter().enumerate() {
        println!("  {s}: {} bytes, {}", a[i].len(), if a[i] == b[i] { "identical" } else { "DIFFERS" });
    }
    println!("frames {}, identical {identical}", SHOTS.len());
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("checks: {passed} of {} passed", checks.len());
    let _ = std::fs::remove_dir_all(&base);
    assert_eq!(passed, checks.len(), "a check failed: see the result block\n{first}\n{second}");
}
