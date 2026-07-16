// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `rcrun run-form` — standalone GUI form runner.
//!
//! Runs a form program (a `.cfrm` layout + its generated `.cbl`) with the same
//! unified render engine used by the IDE designer/preview and by compiled
//! binaries (spec 017: one renderer for every surface), but in **its own
//! process and event loop**. The IDE spawns this for Run Form, so the IDE
//! stays idle while the form does the work — and what you test is exactly
//! what `rcrun build` ships.
//!
//! ```text
//! rcrun run-form <form.cfrm> <program.cbl>
//! ```
//!
//! DISPLAY output goes to stdout (the IDE pipes it into its Output pane);
//! parse/semantic/runtime errors go to stderr and yield a non-zero exit code.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};
use cobolt_semantic::analyze;

fn load_run_form_icon(path: Option<&Path>) -> Option<egui::IconData> {
    path.and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| decode_icon(&bytes))
        .or_else(|| {
            decode_icon(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/images/powerrustcobol-icon.png"
            )))
        })
}

fn decode_icon(bytes: &[u8]) -> Option<egui::IconData> {
    let img = image::load_from_memory(bytes)
        .ok()?
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    })
}

// ── Control state (mirrors the compiled-binary template's CtrlState) ──────────

#[derive(Clone, Default)]
struct CtrlState {
    props: HashMap<String, String>,
    visible: bool,
    enabled: bool,
}

impl CtrlState {
    fn from_control(ctrl: &cobolt_forms::Control) -> Self {
        let mut props = HashMap::new();
        for (k, v) in &ctrl.properties {
            props.insert(k.clone(), v.to_xml_string());
        }
        CtrlState {
            props,
            visible: ctrl.visible,
            enabled: ctrl.enabled,
        }
    }

    fn set(&mut self, key: &str, value: String) {
        match key.to_ascii_uppercase().as_str() {
            "VISIBLE" => self.visible = value != "0" && value != "false",
            "ENABLED" => self.enabled = value != "0" && value != "false",
            _ => {}
        }
        // Overwrite any case-insensitive duplicate so a runtime update (arrives
        // upper-cased from COBOL) never shadows the designed-case key.
        if !self.props.contains_key(key) {
            if let Some(existing) = self
                .props
                .keys()
                .find(|k| k.eq_ignore_ascii_case(key))
                .cloned()
            {
                self.props.remove(&existing);
            }
        }
        self.props.insert(key.to_owned(), value);
    }
}

fn flatten_controls(controls: &[cobolt_forms::Control], out: &mut Vec<cobolt_forms::Control>) {
    for c in controls {
        out.push(c.clone());
        flatten_controls(&c.children, out);
    }
}

/// `FormState` over the live control-state map — merges runtime property values
/// onto each designed control so the unified engine paints the live state.
struct LiveState<'a> {
    state: &'a HashMap<String, CtrlState>,
}

impl<'a> cobolt_forms::render::FormState for LiveState<'a> {
    fn live(&self, base: &cobolt_forms::Control) -> cobolt_forms::Control {
        match self.state.get(&base.id) {
            Some(s) => cobolt_forms::render::merge_props(base, s.props.iter()),
            None => base.clone(),
        }
    }
    fn visible(&self, base: &cobolt_forms::Control) -> bool {
        self.state.get(&base.id).map(|s| s.visible).unwrap_or(true)
    }
    fn enabled(&self, base: &cobolt_forms::Control) -> bool {
        self.state.get(&base.id).map(|s| s.enabled).unwrap_or(true)
    }
}

// ── Entry point ────────────────────────────────────────────────────────────────

pub fn cmd_run_form(args: &[String]) {
    let (cfrm_path, cbl_path) = match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => (PathBuf::from(a), PathBuf::from(b)),
        _ => {
            eprintln!(
                "usage: rcrun run-form <form.cfrm> <program.cbl> [--theme-default <id>] [--icon <image>] [--debug]"
            );
            process::exit(2);
        }
    };
    // Project-level default theme id, forwarded by the IDE (per-form overrides
    // in the .cfrm still win — same resolution the designer canvas uses).
    let theme_default: Option<String> = args
        .iter()
        .position(|a| a == "--theme-default")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let icon_path: Option<PathBuf> = args
        .iter()
        .position(|a| a == "--icon")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    // Debug mode: the IDE (or any future remote host — Android/iOS) controls
    // the interpreter over stdin/stdout. Commands arrive as `@DBG <json>` lines
    // on stdin; DebugEvents leave as `@DBG <json>` lines on stdout. Plain
    // stdout lines remain DISPLAY output. The program starts paused at line 1.
    let debug_mode = args.iter().any(|a| a == "--debug");

    // ── Load the form layout ──────────────────────────────────────────────────
    let form = match cobolt_forms::load_form(&cfrm_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("run-form: cannot load {}: {e}", cfrm_path.display());
            process::exit(1);
        }
    };

    // ── Parse + analyse the COBOL program ─────────────────────────────────────
    let source = match std::fs::read_to_string(&cbl_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("run-form: cannot read {}: {e}", cbl_path.display());
            process::exit(1);
        }
    };
    let fmt = SourceFormat::detect(&source);
    let tokens = tokenize(&source, fmt);
    let parse_result = parse(tokens);
    let mut hard_errors = false;
    for d in &parse_result.diagnostics {
        let sev = match d.severity {
            cobolt_parser::Severity::Error => {
                hard_errors = true;
                "error"
            }
            cobolt_parser::Severity::Warning => "warning",
        };
        eprintln!(
            "{}:{}:{}: {sev}: {}",
            cbl_path.display(),
            d.span.line,
            d.span.col,
            d.message
        );
    }
    let program = match parse_result.program {
        Some(p) if !hard_errors => p,
        _ => {
            eprintln!("run-form: aborting — parse errors found.");
            process::exit(1);
        }
    };
    let sem = analyze(&program);
    for d in &sem.diagnostics {
        use cobolt_semantic::Severity;
        let sev = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        eprintln!(
            "{}:{}:{}: {sev}: {}",
            cbl_path.display(),
            d.span.line,
            d.span.col,
            d.message
        );
    }
    if !sem.is_ok() {
        eprintln!("run-form: aborting — semantic errors found.");
        process::exit(1);
    }

    // ── Flatten controls + initial state + object-registry seed ──────────────
    let mut flat: Vec<cobolt_forms::Control> = Vec::new();
    flatten_controls(&form.controls, &mut flat);
    flat.sort_by_key(|c| c.z_order);

    let mut state: HashMap<String, CtrlState> = HashMap::new();
    for c in &flat {
        state.insert(c.id.clone(), CtrlState::from_control(c));
    }

    // Seed the interpreter's visual-object registry with every control's
    // designed properties, so property references and method getters return
    // the configured values before any setter runs (same as the IDE runtime).
    let seed: Vec<(String, String, Vec<(String, String)>)> = flat
        .iter()
        .map(|c| {
            let mut props: Vec<(String, String)> = c
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), v.to_xml_string()))
                .collect();
            let b = |v: bool| if v { "1" } else { "0" }.to_string();
            props.push(("Name".into(), c.id.clone()));
            props.push(("Visible".into(), b(c.visible)));
            props.push(("Enabled".into(), b(c.enabled)));
            props.push(("X".into(), c.rect.x.to_string()));
            props.push(("Y".into(), c.rect.y.to_string()));
            props.push(("Width".into(), c.rect.w.to_string()));
            props.push(("Height".into(), c.rect.h.to_string()));
            props.push(("TabOrder".into(), c.tab_order.to_string()));
            (c.id.clone(), c.control_type.as_str().to_string(), props)
        })
        .collect();

    // ── Interpreter thread ────────────────────────────────────────────────────
    let (ev_tx, ev_rx) = mpsc::channel::<FormEvent>();
    let (input_tx, input_rx) = mpsc::channel::<StateUpdate>();
    let (state_tx, state_rx) = mpsc::channel::<StateUpdate>();
    let (display_tx, display_rx) = mpsc::channel::<String>();

    let pending = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicBool::new(false));
    let error_slot: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    // ── Remote debug wiring (`--debug`) ──────────────────────────────────────
    // stdin `@DBG <json RemoteDebugCmd>` lines → interpreter debug channels;
    // interpreter DebugEvents → stdout `@DBG <json DebugEvent>` lines. The
    // same line protocol can later ride adb/ssh for Android/iOS debuggees.
    let debug_wiring = if debug_mode {
        use cobolt_runtime::{new_breakpoints, DebugEvent, RemoteDebugCmd};

        let (dbg_cmd_tx, dbg_cmd_rx) = mpsc::channel::<cobolt_runtime::DebugCmd>();
        let (dbg_ev_tx, dbg_ev_rx) = mpsc::channel::<DebugEvent>();
        let breakpoints = new_breakpoints();

        // stdin reader: parse and dispatch remote debug commands.
        {
            let bps = Arc::clone(&breakpoints);
            std::thread::spawn(move || {
                use std::io::BufRead;
                let stdin = std::io::stdin();
                for line in stdin.lock().lines().map_while(Result::ok) {
                    let Some(json) = line.strip_prefix("@DBG ") else {
                        continue;
                    };
                    match serde_json::from_str::<RemoteDebugCmd>(json) {
                        Ok(RemoteDebugCmd::Cmd(c)) => {
                            if dbg_cmd_tx.send(c).is_err() {
                                break;
                            }
                        }
                        Ok(RemoteDebugCmd::SetBreakpoints(lines)) => {
                            if let Ok(mut guard) = bps.lock() {
                                *guard = lines.into_iter().collect();
                            }
                        }
                        Err(e) => eprintln!("run-form: bad @DBG command: {e}"),
                    }
                }
            });
        }

        // event pump: interpreter → stdout (whole lines; println! locks stdout,
        // so interleaving with DISPLAY output stays line-atomic).
        std::thread::spawn(move || {
            use std::io::Write;
            for ev in dbg_ev_rx.iter() {
                match serde_json::to_string(&ev) {
                    Ok(json) => {
                        println!("@DBG {json}");
                        let _ = std::io::stdout().flush();
                    }
                    Err(e) => eprintln!("run-form: cannot serialize DebugEvent: {e}"),
                }
            }
        });

        Some((dbg_cmd_rx, dbg_ev_tx, breakpoints))
    } else {
        None
    };

    {
        let finished = Arc::clone(&finished);
        let error_slot = Arc::clone(&error_slot);
        let pending = Arc::clone(&pending);
        std::thread::spawn(move || {
            let mut interp = Interpreter::new_with_channels(program, ev_rx, state_tx, display_tx);
            interp.set_input_channel(input_rx);
            interp.set_event_counter(pending);
            interp.seed_objects(seed);
            if let Some((cmd_rx, ev_tx, bps)) = debug_wiring {
                interp.attach_debug_channels(cmd_rx, ev_tx, bps);
            }
            match interp.run() {
                Ok(()) => {}
                Err(e) if e.is_exit_signal() => {}
                Err(e) => {
                    if let Ok(mut slot) = error_slot.lock() {
                        *slot = Some(e.to_string());
                    }
                }
            }
            finished.store(true, Ordering::Relaxed);
        });
    }

    // ── Theme + glass parity with the designer canvas (spec 017/007) ──────────
    // Resolve the form's theme (per-form override ?? project default ?? Liquid
    // Glass) against packs discovered next to the executable — the same lookup
    // the IDE performs — so the standalone window paints identically.
    let theme_pack: Option<Arc<cobolt_forms::theme_pack::ThemePack>> = {
        let themes_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("assets/themes")))
            .filter(|d| d.is_dir())
            .unwrap_or_else(|| PathBuf::from("assets/themes"));
        let id =
            cobolt_forms::theme::resolve_theme_id(form.theme.as_deref(), theme_default.as_deref());
        cobolt_forms::theme_pack::discover_packs(&themes_dir)
            .into_iter()
            .find(|p| p.id == id)
            .map(Arc::new)
    };
    let glass_style = form.glass_style;

    // ── GUI event loop (own process — the IDE stays idle) ─────────────────────
    let (fw, fh) = (form.width as f32, form.height as f32);
    let title = form.title.clone();
    let app = FormApp {
        form_name: form.name.clone(),
        theme_pack,
        glass_style,
        visuals_set: false,
        controls: flat,
        state,
        bg_hex: form.background_color.clone(),
        transparency: form.transparency.clamp(0, 100) as u8,
        bg_image: form.background_image.clone(),
        bg_mode: form.bg_image_mode,
        form_size: egui::vec2(fw, fh),
        ev_tx,
        input_tx,
        state_rx,
        display_rx,
        pending,
        finished: Arc::clone(&finished),
        start: std::time::Instant::now(),
        lifecycle_sent: false,
        quit_sent: false,
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(&title)
        .with_inner_size([fw + 4.0, fh + 4.0])
        .with_resizable(true);
    if let Some(icon) = load_run_form_icon(icon_path.as_deref()) {
        viewport = viewport.with_icon(icon);
    }
    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let _ = eframe::run_native(
        &title,
        native_options,
        Box::new(move |_cc| Ok(Box::new(app) as Box<dyn eframe::App>)),
    );

    // Surface a runtime error (if any) after the window closes.
    let runtime_error = error_slot.lock().ok().and_then(|mut s| s.take());
    if let Some(e) = runtime_error {
        eprintln!("run-form: runtime error: {e}");
        process::exit(1);
    }
}

// ── FormApp ───────────────────────────────────────────────────────────────────

struct FormApp {
    form_name: String,
    /// Resolved asset-pack theme (None = built-in Liquid Glass) + the form's
    /// glass style — pushed into the egui context so the unified painter reads
    /// the same theme state as under the IDE (spec 017 parity).
    theme_pack: Option<Arc<cobolt_forms::theme_pack::ThemePack>>,
    glass_style: cobolt_forms::model::GlassStyle,
    visuals_set: bool,
    controls: Vec<cobolt_forms::Control>,
    state: HashMap<String, CtrlState>,
    bg_hex: String,
    transparency: u8,
    bg_image: String,
    bg_mode: cobolt_forms::model::BgImageMode,
    form_size: egui::Vec2,
    ev_tx: mpsc::Sender<FormEvent>,
    input_tx: mpsc::Sender<StateUpdate>,
    state_rx: mpsc::Receiver<StateUpdate>,
    display_rx: mpsc::Receiver<String>,
    /// Events queued for the interpreter — lets the render Timer arm coalesce
    /// ticks against a backlog and drives the fast-repaint branch below.
    pending: Arc<AtomicUsize>,
    /// Set by the interpreter thread when the program ends (STOP RUN or error).
    finished: Arc<AtomicBool>,
    /// When the window opened. Input is ignored for a short warm-up so a click
    /// in progress as the window appears can't fire a phantom event.
    start: std::time::Instant,
    lifecycle_sent: bool,
    quit_sent: bool,
}

impl FormApp {
    fn send_event(&mut self, ev: FormEvent) {
        if self.ev_tx.send(ev).is_ok() {
            self.pending.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Resolve a control id arriving from COBOL (upper-cased by the compiler)
    /// to the designer's original-case state key — otherwise a handler's
    /// property writes land in an orphan "LABEL-1" entry the renderer (which
    /// looks up by the designed "Label-1" id) never reads.
    fn resolve_ctrl_key(&self, id: &str) -> String {
        if let Some(k) = self.state.keys().find(|k| k.eq_ignore_ascii_case(id)) {
            return k.clone();
        }
        if let Some(c) = self.controls.iter().find(|c| {
            c.explicit_control_array_id()
                .map(|aid| aid.eq_ignore_ascii_case(id))
                .unwrap_or(false)
        }) {
            return c.id.clone();
        }
        id.to_owned()
    }

    /// For a repeating-group member id (case-insensitive), return its
    /// original-case id and the id of its repeating-GroupBox ancestor.
    fn array_member_group(&self, ctrl_id: &str) -> Option<(String, String)> {
        let member = self
            .controls
            .iter()
            .find(|c| c.id.eq_ignore_ascii_case(ctrl_id))?;
        let member_id = member.id.clone();
        let mut cur = member;
        loop {
            let parent_id = cur.parent.as_deref()?;
            let parent = self
                .controls
                .iter()
                .find(|c| c.id.eq_ignore_ascii_case(parent_id))?;
            let is_repeating = matches!(parent.control_type, cobolt_forms::ControlType::GroupBox)
                && parent
                    .get_prop("IsRepeatingGroup")
                    .map(|v| v.as_bool())
                    .unwrap_or(false);
            if is_repeating {
                return Some((member_id, parent.id.clone()));
            }
            cur = parent;
        }
    }
}

impl eframe::App for FormApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Light visuals baseline — a fresh egui context defaults to DARK mode,
        // which leaks dark widget fills (labels, text boxes) into the form and
        // breaks parity with the designer canvas. Set once.
        if !self.visuals_set {
            self.visuals_set = true;
            ctx.set_visuals(egui::Visuals::light());
        }
        // Theme pack + glass style for the unified painter (per frame — same
        // contract as the IDE's running-form viewport).
        cobolt_forms::paint::set_active_theme(ctx, self.theme_pack.clone());
        cobolt_forms::paint::set_glass_style(ctx, self.glass_style);

        // Program ended (STOP RUN / runtime error) → close the window.
        if self.finished.load(Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        // Window close button → tell the COBOL event loop to quit, then close.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quit_sent {
            self.quit_sent = true;
            let _ = self.ev_tx.send(FormEvent::quit());
        }

        // Apply property changes coming from the COBOL interpreter. Route each
        // update to the designer-case state key (COBOL upper-cases ids), and
        // repeating-group member writes to the drawn card-instance id — the
        // same resolution the IDE's FormRuntime::drain_state performs.
        let mut drained = 0usize;
        while let Ok(u) = self.state_rx.try_recv() {
            let key = if u.instance_index > 0 {
                match self.array_member_group(&u.ctrl_id) {
                    Some((member_id, group_id)) => cobolt_forms::render::member_instance_id(
                        &group_id,
                        &member_id,
                        u.instance_index,
                    ),
                    None => self.resolve_ctrl_key(&u.ctrl_id),
                }
            } else {
                self.resolve_ctrl_key(&u.ctrl_id)
            };
            self.state.entry(key).or_default().set(&u.prop, u.value);
            drained += 1;
        }
        // DISPLAY output → stdout (the IDE pipes this into its Output pane).
        // Explicit flush: stdout is BLOCK-buffered when piped, so without it
        // DISPLAY lines sit in the buffer instead of reaching the IDE live.
        {
            let mut any = false;
            while let Ok(line) = self.display_rx.try_recv() {
                println!("{line}");
                any = true;
            }
            if any {
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
        }

        // Ignore input for a brief warm-up after the window appears.
        let armed = self.start.elapsed().as_millis() > 450;

        // Form-level lifecycle: onShow / onActivate fire once after warm-up
        // (parity with the IDE's running form; unknown events are ignored by
        // the generated dispatch loop, so this is safe for any form).
        if armed && !self.lifecycle_sent {
            self.lifecycle_sent = true;
            let name = self.form_name.clone();
            self.send_event(FormEvent::new(&name, "onShow"));
            self.send_event(FormEvent::new(&name, "onActivate"));
        }

        // Background image texture (cached in egui memory by path).
        let backdrop_image = if self.bg_image.trim().is_empty() {
            None
        } else {
            let path = self.bg_image.clone();
            let id = egui::Id::new(("run_form_bg", path.as_str()));
            let cached = ctx.memory(|m| m.data.get_temp::<Option<egui::TextureHandle>>(id));
            let tex = match cached {
                Some(t) => t,
                None => {
                    let loaded = cobolt_forms::paint::load_image_texture(ctx, &path);
                    ctx.memory_mut(|m| m.data.insert_temp(id, loaded.clone()));
                    loaded
                }
            };
            tex.map(|t| (t.id(), t.size_vec2()))
        };

        let bg_fill = cobolt_forms::render::backdrop_color(&self.bg_hex, self.transparency);
        let form_size = self.form_size;

        // Render the whole form through the unified engine (one renderer for
        // the designer, preview, running form, compiled binary — and this).
        let output = {
            let controls = self.controls.clone();
            let st = LiveState { state: &self.state };
            let active_tabs = cobolt_forms::containers::ActiveTabs::default();
            let backdrop = cobolt_forms::render::Backdrop {
                color_hex: self.bg_hex.clone(),
                transparency: self.transparency,
                image: backdrop_image,
                image_mode: self.bg_mode,
            };
            let mut out = cobolt_forms::render::RenderOutput::default();
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(bg_fill))
                .show(ctx, |ui| {
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_min_size(form_size);
                            let input = cobolt_forms::render::RenderInput {
                                controls: &controls,
                                state: &st,
                                form_size,
                                glass: true,
                                mode: cobolt_forms::render::RenderMode::Interactive,
                                active_tabs: &active_tabs,
                                backdrop,
                            };
                            out = cobolt_forms::render::render_form(ui, &input);
                        });
                });
            out
        };

        // Apply value updates locally, sync them to the interpreter (so
        // handlers read the live value), and forward UI events — once armed.
        let mut interacted = false;
        if armed {
            for (id, key, val) in &output.prop_updates {
                self.state
                    .entry(id.clone())
                    .or_default()
                    .set(key, val.clone());
                let _ = self
                    .input_tx
                    .send(StateUpdate::new(id.clone(), key.clone(), val.clone()));
                interacted = true;
            }
            // Coalesce timer ticks against a still-queued backlog (WinForms
            // semantics) so a slow handler can't flood the event queue. User
            // events (clicks, edits, focus, quit) are never dropped.
            let backlog = self.pending.load(Ordering::Relaxed) > 0;
            for ev in output.events {
                let is_tick = ev.event.eq_ignore_ascii_case("onTick");
                if is_tick && backlog {
                    continue;
                }
                // Instanced repeating-group members are drawn with the id
                // "group.group-N.member" — dispatch to the designed (base)
                // member id, forwarding the 1-based instance index so the
                // handler receives CONTROL-ARRAY-INDEX.
                let (dispatch_id, inst) = if ev.ctrl_id.contains('.') {
                    let base = ev
                        .ctrl_id
                        .rsplit('.')
                        .next()
                        .unwrap_or(&ev.ctrl_id)
                        .to_string();
                    let inst = {
                        let parts: Vec<&str> = ev.ctrl_id.split('.').collect();
                        if parts.len() >= 2 {
                            parts[1]
                                .rsplit('-')
                                .next()
                                .and_then(|s| s.parse::<usize>().ok())
                                .unwrap_or(0)
                        } else {
                            0
                        }
                    };
                    (base, inst)
                } else {
                    (ev.ctrl_id.clone(), 0)
                };
                self.send_event(FormEvent::new(dispatch_id, ev.event).with_index(inst));
                interacted = true;
            }
        }

        // Reactive frame scheduling — never spin at max FPS. While interpreter
        // traffic is flowing (state drained, events sent, or a backlog is
        // queued), poll fast; otherwise a slow heartbeat keeps DISPLAY output
        // and end-of-program detection timely. Timer controls schedule their
        // own precise wake-ups inside the render engine, and user input wakes
        // egui automatically — between all of those, the process sleeps.
        let busy = drained > 0 || interacted || self.pending.load(Ordering::Relaxed) > 0;
        let ms = if busy { 16 } else { 200 };
        ctx.request_repaint_after(std::time::Duration::from_millis(ms));
    }
}
