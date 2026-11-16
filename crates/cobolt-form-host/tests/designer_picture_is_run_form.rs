// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The picture of a form is the form as Run Form draws it, pixel for pixel.**
//!
//! `cobolt_forms::snapshot::render_form_image` makes the picture the designer's preview and the
//! coding agent's `render_form` tool show. Its promise (spec 084 R30) is the form as Run Form
//! draws it when it opens. Run Form's host starts every window on egui's light visuals; the
//! picture was made on a fresh context, which is in dark mode, and dark mode inks the edge of
//! every glyph and of every anti-aliased line darker. So text and chart lines differed at their
//! edges while every rectangle matched (DESKTOP-WASM-001 T35 found 1 486 such pixels on a
//! 400 by 300 form).
//!
//! Here the same form is pictured both ways: by the snapshot, and by the real host and the real
//! interpreter, as `rcrun run-form` wires them, driven off screen by
//! [`cobolt_form_host::headless::run_headless`]. The two pictures must be equal in every pixel.
//! The form has no handler, so nothing a program does at open moves it.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use cobolt_form_host::state::CtrlState;
use cobolt_form_host::{flatten_controls, FormHostConfig, NoHooks, Surface};
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};
use serde_json::json;

const FORM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="PICTURE-FORM" title="Picture" width="400" height="300" main-form="true">
  <Control id="LBL-TITLE" type="Label" x="20" y="16" w="300" h="24" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">The same form, pictured twice</Property>
  </Control>
  <Control id="TXT-NAME" type="TextBox" x="20" y="52" w="200" h="26" tab-order="1" z-order="1" visible="true" enabled="true">
    <Property name="Text">Ann</Property>
  </Control>
  <Control id="BTN-GO" type="Button" x="236" y="52" w="120" h="26" tab-order="2" z-order="2" visible="true" enabled="true">
    <Property name="Caption">Go</Property>
  </Control>
  <Control id="CHT-AREA" type="AreaChart" x="20" y="96" w="360" h="180" tab-order="3" z-order="3" visible="true" enabled="true" />
</Form>
"#;

/// A form program that waits for events and handles none.
const PROGRAM: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PICTURE-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-QUIT        PIC 9      VALUE 0.
       01 COBOL-EVENT-ID    PIC X(64)  VALUE SPACES.
       01 COBOL-CONTROL-ID  PIC X(64)  VALUE SPACES.
       01 FORM-NAME         PIC X(64)  VALUE 'PICTURE-FORM'.
       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::\"INIT-FORM\" ( FORM-NAME )
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
           END-PERFORM
           STOP RUN.
";

/// The host and interpreter exactly as `rcrun run-form` wires them (`cobolt-cli/src/form_gui.rs`).
fn host_config(form: cobolt_forms::Form) -> FormHostConfig {
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

/// The RGBA pixels of a PNG file, and its size.
fn pixels(png: &Path) -> (Vec<u8>, (u32, u32)) {
    let img = image::open(png).expect("the picture decodes").to_rgba8();
    let size = img.dimensions();
    (img.into_raw(), size)
}

#[test]
fn the_designer_picture_is_the_form_run_form_draws() {
    let form = cobolt_forms::load_form_from_str(FORM).expect("the form loads");
    let dir = std::env::temp_dir().join(format!("prc-picture-is-run-form-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // ── The picture the designer's preview and the agent's render_form show ──
    let t = Instant::now();
    let image = cobolt_forms::snapshot::render_form_image(&form, &cobolt_forms::snapshot::SnapshotOptions::default());
    let designer_png = dir.join("designer.png");
    std::fs::write(&designer_png, cobolt_forms::raster::to_png(&image).unwrap()).unwrap();
    let designer_ms = t.elapsed().as_secs_f64() * 1000.0;

    // ── The form as Run Form draws it, once it has settled ──
    let run_png = dir.join("run.png");
    let t = Instant::now();
    let report = cobolt_form_host::headless::run_headless(host_config(form), None, &[json!({"wait_ms": 1500})], Duration::from_secs(60), Some(&run_png));
    let run_ms = t.elapsed().as_secs_f64() * 1000.0;

    let (a, size_a) = pixels(&designer_png);
    let (b, size_b) = pixels(&run_png);
    let compared = u64::from(size_a.0) * u64::from(size_a.1);
    let differing = if size_a == size_b { a.chunks(4).zip(b.chunks(4)).filter(|(p, q)| p != q).count() as u64 } else { compared };
    let inked = a.chunks(4).filter(|p| p[3] != 0).count();
    let checks = [
        ("the run did not reach its limit", report["timed_out"] == json!(false)),
        ("both pictures are 400x300", size_a == (400, 300) && size_b == (400, 300)),
        ("the designer's picture holds the form", inked > 0),
        ("0 differing pixels", differing == 0),
    ];
    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== the_designer_picture_is_the_form_run_form_draws (snapshot against Run Form) ==");
    println!("form PICTURE-FORM 400x300: Label, TextBox, Button, AreaChart; no handler; theme {}", cobolt_forms::theme::resolve_theme_id(None, None));
    println!("designer picture {designer_ms:.0} ms; Run Form, headless, settled 1 500 ms, {run_ms:.0} ms");
    println!("pixels compared {compared}, inked {inked}, differing {differing}");
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("result: {passed} of {} checks passed", checks.len());
    if passed == checks.len() {
        let _ = std::fs::remove_dir_all(&dir);
    } else {
        println!("kept for inspection: {}", dir.display());
    }
    assert_eq!(passed, checks.len(), "{report}");
}
