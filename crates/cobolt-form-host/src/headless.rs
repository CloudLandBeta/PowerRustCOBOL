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
//! | `{"event": {…}, "wait": false}` | clicked, without waiting for the handler (a click while another handler is busy) |
//! | `{"click": {"control": "BTN-OK"}}` | a real pointer click on the control's centre (move, press, release): focus and mouse events fire |
//! | `{"wait_ms": 300}` | lets timers and animations run |
//! | `{"read": {"control": "LBL-TOTAL", "property": "Caption"}}` | the live value |
//! | `{"dump": true}` | every control's live state at that point, in the step's `state` (`{"BTN-OK": {"visible": true, "enabled": true, "props": {…}}, …}`): the designed properties and what the program wrote (a property the program wrote is spelled as COBOL spells it, `CAPTION`, and replaces the designed `Caption`) |
//! | `{"open_form": "ORDERS-FORM"}` | a shell application: load a form into the ContentPane, as its menu item would |
//! | `{"size": [800, 600]}` | resizes the window: the next frames lay the form out at that size |
//! | `{"shot": "after-save"}` | writes the frame as it is now to `after-save.png` in the folder for pictures (`PRC_HEADLESS_SHOTS` in a built application) |
//! | `{"answer": {"path": "/tmp/out.csv"}}`, `{"answer": {"cancel": true}}` | queues the answer for the next file dialog (a program's open, save or folder dialog, a Viewer's Save As), which then shows no panel: put it before the step that opens the dialog. The report's `dialogs_asked` lists what each program's dialog so answered asked for (its kind, title, filters, folder and file name) |
//!
//! `set`, `event` and `read` act on the form on screen: in a shell, the form
//! in the ContentPane. After every `set`, `event` (unless `"wait": false`), `open_form` and `size` the form is
//! run until the program has handled everything sent to it (or the time limit
//! runs out).
//!
//! **Deterministic mode** (DESKTOP-WASM-001 R28): when `PRC_TEST_CLOCK` pins the clock, frames
//! are drawn at a virtual time that moves only when the script waits. `wait_ms` and the opening
//! wait move it by 1/60 s per frame; a frame that only waits for the program does not move it. So
//! the same script draws its frames at the same times in every run. The host's own clock (its
//! effects, snackbars, timers, the input hold) and the program's clock follow that time (T8), so
//! a wait lets no real time pass. Before time moves on, the form on screen finishes everything it
//! was sent (a long `onLoad`, a timer's tick, a canvas frame, `onShow`), unless a modal form it
//! opened holds it up; so does it at the end of every step that waits for the program.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::host::FormHost;
use crate::shell::ShellApp;
use crate::FormHostConfig;

/// What a headless run drives: the form's own host, or the shell around it.
enum Driven {
    Plain(FormHost),
    Shell(Box<ShellApp>),
}

impl Driven {
    fn frame(&mut self, ui: &mut egui::Ui) {
        match self {
            Driven::Plain(h) => h.ui_impl(ui),
            Driven::Shell(s) => s.frame(ui),
        }
    }

    fn host(&self) -> &FormHost {
        match self {
            Driven::Plain(h) => h,
            Driven::Shell(s) => s.host(),
        }
    }

    fn host_mut(&mut self) -> &mut FormHost {
        match self {
            Driven::Plain(h) => h,
            Driven::Shell(s) => s.host_mut(),
        }
    }
}

/// The marker the result line carries on stdout, so a caller can tell it
/// from the program's own DISPLAY lines, which the host prints as they come.
pub const RESULT_MARKER: &str = "@RUN-FORM-RESULT ";

/// The headless run of a **built application** (spec 078 T-A16): the script
/// at `script_path` drives the application's own host for at most
/// `limit_secs`, exactly as `rcrun run-form --headless` drives Run Form, and
/// the result line is printed with [`RESULT_MARKER`]. This is what lets a test
/// prove a compiled binary behaves as Run Form does without opening a window.
///
/// Child processes the run started (an AWS control's server) are stopped
/// before returning; the caller ends the process. The `shot` steps' pictures
/// go into `shots`.
pub fn run_script_file(
    config: FormHostConfig,
    shell: Option<Option<(String, cobolt_forms::menu::MenuDefinition)>>,
    script_path: &Path,
    limit_secs: u64,
    shots: Option<&Path>,
) {
    let script: Vec<Value> = std::fs::read_to_string(script_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let report = run_headless_with(config, shell, &script, Duration::from_secs(limit_secs.clamp(1, 600)), None, shots);
    println!("{RESULT_MARKER}{report}");
    use std::io::Write as _;
    let _ = std::io::stdout().flush();
    cobolt_runtime::shutdown_child_processes();
}

/// Spec 084 R31: run the form described by `config` through `script`, for at
/// most `limit`, and say what happened. The picture of the last frame goes to
/// `png_out` when given. `shell` runs it as `rcrun run-form` runs a main form
/// with a SideMenu — inside the application shell, `root_menu` mounted.
pub fn run_headless(
    config: FormHostConfig,
    shell: Option<Option<(String, cobolt_forms::menu::MenuDefinition)>>,
    script: &[Value],
    limit: Duration,
    png_out: Option<&Path>,
) -> Value {
    run_headless_with(config, shell, script, limit, png_out, None)
}

/// Where the picture of a `shot` step goes: `<shots>/<name>.png`, `name` a plain file name.
fn picture_path(shots: Option<&Path>, name: &str) -> Result<PathBuf, String> {
    let Some(dir) = shots else {
        return Err("no folder for pictures: a built application takes it from PRC_HEADLESS_SHOTS".into());
    };
    let name = name.trim();
    if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\']) {
        return Err(format!("{name:?} is not a plain file name"));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir.join(format!("{name}.png")))
}

/// [`run_headless`], with the pictures of the `shot` steps written into the folder `shots`.
pub fn run_headless_with(
    config: FormHostConfig,
    shell: Option<Option<(String, cobolt_forms::menu::MenuDefinition)>>,
    script: &[Value],
    limit: Duration,
    png_out: Option<&Path>,
    shots: Option<&Path>,
) -> Value {
    let started = Instant::now();
    let deadline = started + limit;
    // DESKTOP-WASM-001 R28: with the clock pinned, the run is deterministic. Its time is virtual,
    // counted in 1/60 s ticks that only a wait adds, and the host's own clock (`crate::clock`) and
    // the program's follow it (T8). It starts before the form is built, so every instant the host
    // keeps is on that clock.
    let deterministic = cobolt_runtime::test_pins::clock_pinned();
    if deterministic {
        cobolt_runtime::test_pins::start_virtual_time();
    }
    let (mut host, size) = match shell {
        Some(root_menu) => {
            let (app, _title, viewport) = ShellApp::new(config, root_menu);
            let size = viewport.inner_size.unwrap_or(egui::vec2(1024.0, 768.0));
            (Driven::Shell(Box::new(app)), size)
        }
        None => {
            let (host, _form) = FormHost::new(config);
            let size = host.script_form_size();
            (Driven::Plain(host), size)
        }
    };
    let size = Cell::new(size);
    let ticks = Cell::new(0u64);
    let now = || if deterministic { ticks.get() as f64 / 60.0 } else { started.elapsed().as_secs_f64() };
    // One tick of virtual time, for the frames, the host's clock and the program's alike.
    let tick = || {
        ticks.set(ticks.get() + 1);
        cobolt_runtime::test_pins::set_virtual_time(Duration::from_nanos(ticks.get() * 1_000_000_000 / 60));
    };
    // Answers an earlier run in this process left are not this script's, nor what it asked.
    crate::file_dialog::clear_answers();
    crate::file_dialog::take_asked();
    let ctx = egui::Context::default();
    ctx.set_fonts(cobolt_forms::fonts::base_font_definitions());
    let mut raster = cobolt_forms::raster::Rasterizer::new();

    let frame = |host: &mut Driven, raster: &mut cobolt_forms::raster::Rasterizer| {
        cobolt_forms::raster::advance_frame(&ctx, raster, size.get(), now(), |ui| host.frame(ui));
    };
    // One frame that receives the pointer `events`.
    let frame_with = |host: &mut Driven, raster: &mut cobolt_forms::raster::Rasterizer, events: Vec<egui::Event>| {
        cobolt_forms::raster::advance_frame_with(&ctx, raster, size.get(), now(), events, |ui| host.frame(ui));
    };
    // The count of events sent when the form on the pane was last found at rest.
    let quiet = Cell::new(crate::host::events_sent());
    // The count of windows opened when the open windows were last sent their wakes.
    let quiet_windows = Cell::new(crate::host::windows_opened());
    // Deterministic mode: let the form on the pane finish everything it was sent (a long onLoad,
    // a timer's tick, a canvas frame) before the virtual clock moves on, so what the next frame
    // draws never depends on how fast the program ran. The program takes an event off its queue
    // before handling it, so a wake goes behind what it was sent (`script_barrier`); the wake is
    // taken only once the handlers before it have returned. The open child windows are waited for
    // the same way, so a form opened by a click has finished its start-up (its `onLoad`) before the
    // next frame is drawn. A form that a modal form holds up, or whose program has ended, is not
    // waited for.
    let quiesce = |host: &mut Driven, raster: &mut cobolt_forms::raster::Rasterizer| {
        let held = |h: &Driven| h.host().script_finished() || h.host().root_modal_blocked();
        let busy = |h: &Driven| (!held(h) && h.host().script_pending() > 0) || h.host().script_children_pending() > 0;
        // A frame that raises an event at every frame would never rest: a bound on the rounds.
        for _ in 0..16 {
            if Instant::now() >= deadline {
                return;
            }
            let calm = crate::host::events_sent() == quiet.get() && crate::host::windows_opened() == quiet_windows.get();
            if calm && !busy(host) {
                return;
            }
            // A window opened from here on, after its wake would have been sent, is another round.
            quiet_windows.set(crate::host::windows_opened());
            if !held(host) {
                host.host_mut().script_barrier();
            }
            host.host_mut().script_children_barrier();
            while busy(host) && Instant::now() < deadline {
                frame(host, raster);
                std::thread::sleep(Duration::from_millis(1));
            }
            quiet.set(crate::host::events_sent());
            // What the handlers wrote reaches the state. An event these frames raise is another
            // round.
            for _ in 0..3 {
                frame(host, raster);
            }
        }
    };
    // Run until the program has handled everything sent to it.
    let settle = |host: &mut Driven, raster: &mut cobolt_forms::raster::Rasterizer| -> bool {
        loop {
            frame(host, raster);
            if host.host().script_pending() == 0 || host.host().script_finished() {
                // A few more frames: what the handler wrote reaches the state.
                for _ in 0..3 {
                    frame(host, raster);
                    std::thread::sleep(Duration::from_millis(10));
                }
                // And what those frames raised is handled too, before the next step.
                if deterministic {
                    quiesce(host, raster);
                }
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    // Run frames for `ms`. In deterministic mode nothing sleeps: each frame is one tick of virtual
    // time, and before and after each the form on the pane finishes what it was sent (`quiesce`).
    let run_for = |host: &mut Driven, raster: &mut cobolt_forms::raster::Rasterizer, ms: u64| {
        if deterministic {
            quiesce(host, raster);
            for _ in 0..(ms * 60).div_ceil(1000) {
                if Instant::now() >= deadline {
                    break;
                }
                frame(host, raster);
                quiesce(host, raster);
                tick();
            }
            return;
        }
        let until = (Instant::now() + Duration::from_millis(ms)).min(deadline);
        while Instant::now() < until {
            frame(host, raster);
            std::thread::sleep(Duration::from_millis(16));
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
        if host.host().script_finished() {
            steps.push(json!({"step": i, "done": false, "why": "the program has ended"}));
            continue;
        }
        let field = |obj: &Value, k: &str| obj.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        if let Some(set) = step.get("set") {
            let (c, p, v) = (field(set, "control"), field(set, "property"), field(set, "value"));
            host.host_mut().script_set_prop(&c, &p, &v);
            timed_out = !settle(&mut host, &mut raster);
            steps.push(json!({"step": i, "done": true, "set": format!("{c}::{p}")}));
        } else if let Some(ev) = step.get("event") {
            let (c, n) = (field(ev, "control"), field(ev, "name"));
            host.host_mut().script_event(&c, &n);
            // `"wait": false` raises the event and goes on, as a second click
            // does while a handler is still busy (WASM-005 AC6).
            if step.get("wait").and_then(Value::as_bool) == Some(false) {
                frame(&mut host, &mut raster);
                steps.push(json!({"step": i, "done": true, "event": format!("{c}.{n}"), "waited": false}));
            } else {
                timed_out = !settle(&mut host, &mut raster);
                steps.push(json!({"step": i, "done": !timed_out, "event": format!("{c}.{n}")}));
            }
        } else if let Some(cl) = step.get("click") {
            // A real pointer click on the control's centre: the host sees the
            // move, the press and the release, so focus and mouse events fire
            // as they do under an operator's mouse.
            let c = field(cl, "control");
            let rect = host.host().last_control_rects().get(&c).copied();
            match rect {
                None => steps.push(json!({"step": i, "done": false, "why": format!("{c} was not drawn on the form")})),
                Some(r) => {
                    let at = r.center();
                    let button = egui::PointerButton::Primary;
                    let mods = egui::Modifiers::NONE;
                    frame_with(&mut host, &mut raster, vec![egui::Event::PointerMoved(at), egui::Event::PointerButton { pos: at, button, pressed: true, modifiers: mods }]);
                    frame_with(&mut host, &mut raster, vec![egui::Event::PointerButton { pos: at, button, pressed: false, modifiers: mods }]);
                    timed_out = !settle(&mut host, &mut raster);
                    steps.push(json!({"step": i, "done": !timed_out, "click": c}));
                }
            }
        } else if let Some(ms) = step.get("wait_ms").and_then(Value::as_u64) {
            run_for(&mut host, &mut raster, ms.min(30_000));
            steps.push(json!({"step": i, "done": true, "waited_ms": ms}));
        } else if let Some(rd) = step.get("read") {
            let (c, p) = (field(rd, "control"), field(rd, "property"));
            let value = host.host().script_read(&c, &p);
            reads.insert(format!("{c}::{p}"), value.clone().map(Value::String).unwrap_or(Value::Null));
            steps.push(json!({"step": i, "done": true, "read": format!("{c}::{p}"), "value": value}));
        } else if step.get("dump").is_some() {
            steps.push(json!({"step": i, "done": true, "dump": true, "state": host.host().script_dump()}));
        } else if let Some(form) = step.get("open_form").and_then(Value::as_str) {
            match &mut host {
                Driven::Shell(app) => {
                    app.script_open_form(form);
                    // The click is taken on the next frame; the form's own
                    // program then opens, held 450 ms like any form.
                    run_for(&mut host, &mut raster, 600);
                    timed_out = !settle(&mut host, &mut raster);
                    let on_pane = host.host().active_occupant_form().map(str::to_owned);
                    let done = on_pane.as_deref().is_some_and(|f| f.eq_ignore_ascii_case(form.trim()));
                    steps.push(json!({"step": i, "done": done, "open_form": form, "on_pane": on_pane}));
                }
                Driven::Plain(_) => {
                    steps.push(json!({"step": i, "done": false, "why": "open_form needs a main form with a SideMenu (an application shell)"}));
                }
            }
        } else if let Some(wh) = step.get("size").and_then(Value::as_array) {
            let dim = |k: usize| wh.get(k).and_then(Value::as_f64).filter(|v| (1.0..=8192.0).contains(v));
            match (dim(0), dim(1)) {
                (Some(w), Some(h)) if wh.len() == 2 => {
                    size.set(egui::vec2(w as f32, h as f32));
                    timed_out = !settle(&mut host, &mut raster);
                    steps.push(json!({"step": i, "done": !timed_out, "size": [w, h]}));
                }
                _ => steps.push(json!({"step": i, "done": false, "why": "size takes [width, height], each from 1 to 8192"})),
            }
        } else if let Some(name) = step.get("shot").and_then(Value::as_str) {
            match picture_path(shots, name) {
                Err(why) => steps.push(json!({"step": i, "done": false, "why": why})),
                Ok(path) => {
                    let img = cobolt_forms::raster::render_frame(&ctx, &mut raster, size.get(), egui::Color32::TRANSPARENT, now(), |ui| host.frame(ui));
                    match cobolt_forms::raster::to_png(&img).and_then(|png| std::fs::write(&path, png).map_err(|e| e.to_string())) {
                        Ok(()) => steps.push(json!({"step": i, "done": true, "shot": name, "width": img.size[0], "height": img.size[1]})),
                        Err(e) => steps.push(json!({"step": i, "done": false, "why": format!("{}: {e}", path.display())})),
                    }
                }
            }
        } else if let Some(ans) = step.get("answer") {
            if ans.get("cancel").and_then(Value::as_bool) == Some(true) {
                crate::file_dialog::queue_answer(None);
                steps.push(json!({"step": i, "done": true, "answer": "cancel"}));
            } else if let Some(p) = ans.get("path").and_then(Value::as_str) {
                crate::file_dialog::queue_answer(Some(PathBuf::from(p)));
                steps.push(json!({"step": i, "done": true, "answer": p}));
            } else {
                steps.push(json!({"step": i, "done": false, "why": "answer takes {\"path\": \"…\"} or {\"cancel\": true}"}));
            }
        } else {
            steps.push(json!({"step": i, "done": false, "why": "unknown step: use set, event, click, wait_ms, read, dump, open_form, size, shot or answer"}));
        }
        timed_out |= Instant::now() >= deadline;
    }

    // The picture of where the script left the form.
    let mut picture = None;
    if let Some(out) = png_out {
        let img = cobolt_forms::raster::render_frame(&ctx, &mut raster, size.get(), egui::Color32::TRANSPARENT, now(), |ui| host.frame(ui));
        if let Ok(png) = cobolt_forms::raster::to_png(&img) {
            if std::fs::write(out, png).is_ok() {
                picture = Some(json!({"width": img.size[0], "height": img.size[1]}));
            }
        }
    }
    // The run is over: the host's clock is real again.
    if deterministic {
        cobolt_runtime::test_pins::stop_virtual_time();
    }
    let unused_answers = crate::file_dialog::clear_answers();
    let dialogs_asked = crate::file_dialog::take_asked();
    let mut report = json!({
        "steps": steps,
        "reads": reads,
        "program_ended": host.host().script_finished(),
        "on_pane": host.host().active_occupant_form(),
        "windows": host.host().script_windows().into_iter().map(|(form, shell, on_pane)| {
            json!({"form": form, "shell": shell, "on_pane": on_pane})
        }).collect::<Vec<_>>(),
        "timed_out": timed_out,
        "elapsed_ms": started.elapsed().as_millis() as u64,
        "picture": picture,
    });
    // Only when they apply, so a run without them reports what it always did.
    if deterministic {
        report["virtual_ms"] = json!(ticks.get() * 1000 / 60);
    }
    if unused_answers > 0 {
        report["answers_unused"] = json!(unused_answers);
    }
    if !dialogs_asked.is_empty() {
        report["dialogs_asked"] = json!(dialogs_asked);
    }
    report
}
