// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Running a form off screen with a script** (spec 084 R31): the engine of a
//! coding agent's `run_form`.
//!
//! The real form host and the real interpreter — the very configuration
//! `rcrun run-form` builds — driven by frames on a headless `egui::Context`
//! instead of a window. A script sets properties and raises events as the
//! operator would, waits, and reads properties back; the last frame is
//! pictured by the CPU rasteriser. Nothing is shown and the IDE is not driven.
//!
//! A step is one JSON object:
//!
//! | Step | What it does |
//! |---|---|
//! | `{"set": {"control": "TXT-NAME", "property": "Text", "value": "Ann"}}` | as if typed |
//! | `{"event": {"control": "BTN-OK", "name": "onClick"}}` | as if clicked |
//! | `{"wait_ms": 300}` | lets timers and animations run |
//! | `{"read": {"control": "LBL-TOTAL", "property": "Caption"}}` | the live value |
//!
//! After every `set` and `event` the form is run until the program has
//! handled everything sent to it (or the time limit runs out).

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::host::FormHost;
use crate::FormHostConfig;

/// The marker the result line carries on stdout, so a caller can tell it
/// from the program's own DISPLAY lines, which the host prints as they come.
pub const RESULT_MARKER: &str = "@RUN-FORM-RESULT ";

/// Spec 084 R31: run the form described by `config` through `script`, for at
/// most `limit`, and say what happened. The picture of the last frame goes to
/// `png_out` when given.
pub fn run_headless(config: FormHostConfig, script: &[Value], limit: Duration, png_out: Option<&Path>) -> Value {
    let started = Instant::now();
    let deadline = started + limit;
    let (mut host, _form) = FormHost::new(config);
    let size = host.script_form_size();
    let ctx = egui::Context::default();
    ctx.set_fonts(cobolt_forms::fonts::base_font_definitions());
    let mut raster = cobolt_forms::raster::Rasterizer::new();

    let frame = |host: &mut FormHost, raster: &mut cobolt_forms::raster::Rasterizer| {
        let t = started.elapsed().as_secs_f64();
        cobolt_forms::raster::advance_frame(&ctx, raster, size, t, |ui| host.ui_impl(ui));
    };
    // Run frames for `ms` of real time (the host's clocks are real ones).
    let run_for = |host: &mut FormHost, raster: &mut cobolt_forms::raster::Rasterizer, ms: u64| {
        let until = (Instant::now() + Duration::from_millis(ms)).min(deadline);
        while Instant::now() < until {
            frame(host, raster);
            std::thread::sleep(Duration::from_millis(16));
        }
    };
    // Run until the program has handled everything sent to it.
    let settle = |host: &mut FormHost, raster: &mut cobolt_forms::raster::Rasterizer| -> bool {
        loop {
            frame(host, raster);
            if host.script_pending() == 0 || host.script_finished() {
                // A few more frames: what the handler wrote reaches the state.
                for _ in 0..3 {
                    frame(host, raster);
                    std::thread::sleep(Duration::from_millis(10));
                }
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    };

    // The host ignores input for its first 450 ms and fires onShow after it:
    // let the form open and its start-up handlers finish first.
    run_for(&mut host, &mut raster, 600);
    let mut timed_out = !settle(&mut host, &mut raster);
    let mut steps = Vec::new();
    let mut reads = serde_json::Map::new();

    for (i, step) in script.iter().enumerate() {
        if timed_out {
            steps.push(json!({"step": i, "done": false, "why": "time limit reached"}));
            continue;
        }
        if host.script_finished() {
            steps.push(json!({"step": i, "done": false, "why": "the program has ended"}));
            continue;
        }
        let field = |obj: &Value, k: &str| obj.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        if let Some(set) = step.get("set") {
            let (c, p, v) = (field(set, "control"), field(set, "property"), field(set, "value"));
            host.script_set_prop(&c, &p, &v);
            timed_out = !settle(&mut host, &mut raster);
            steps.push(json!({"step": i, "done": true, "set": format!("{c}::{p}")}));
        } else if let Some(ev) = step.get("event") {
            let (c, n) = (field(ev, "control"), field(ev, "name"));
            host.script_event(&c, &n);
            timed_out = !settle(&mut host, &mut raster);
            steps.push(json!({"step": i, "done": !timed_out, "event": format!("{c}.{n}")}));
        } else if let Some(ms) = step.get("wait_ms").and_then(Value::as_u64) {
            run_for(&mut host, &mut raster, ms.min(30_000));
            steps.push(json!({"step": i, "done": true, "waited_ms": ms}));
        } else if let Some(rd) = step.get("read") {
            let (c, p) = (field(rd, "control"), field(rd, "property"));
            let value = host.control_prop(&c, &p);
            reads.insert(format!("{c}::{p}"), value.clone().map(Value::String).unwrap_or(Value::Null));
            steps.push(json!({"step": i, "done": true, "read": format!("{c}::{p}"), "value": value}));
        } else {
            steps.push(json!({"step": i, "done": false, "why": "unknown step: use set, event, wait_ms or read"}));
        }
        timed_out |= Instant::now() >= deadline;
    }

    // The picture of where the script left the form.
    let mut picture = None;
    if let Some(out) = png_out {
        let t = started.elapsed().as_secs_f64();
        let img = cobolt_forms::raster::render_frame(&ctx, &mut raster, size, egui::Color32::TRANSPARENT, t, |ui| host.ui_impl(ui));
        if let Ok(png) = cobolt_forms::raster::to_png(&img) {
            if std::fs::write(out, png).is_ok() {
                picture = Some(json!({"width": img.size[0], "height": img.size[1]}));
            }
        }
    }
    json!({
        "steps": steps,
        "reads": reads,
        "program_ended": host.script_finished(),
        "timed_out": timed_out,
        "elapsed_ms": started.elapsed().as_millis() as u64,
        "picture": picture,
    })
}
