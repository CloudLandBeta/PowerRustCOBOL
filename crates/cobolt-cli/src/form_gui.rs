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

#[derive(Clone)]
struct CtrlState {
    props: HashMap<String, String>,
    visible: bool,
    enabled: bool,
}

/// A state entry created on the fly (a repeating-group card instance that the
/// interpreter writes to before it exists in `state`) must start VISIBLE and
/// ENABLED. `#[derive(Default)]` would make it `false`, so the very controls a
/// data binding populates would be the ones the renderer skips — databound card
/// members blank in the run form while the DataGrid (whose key is pre-seeded
/// from the design) paints fine. Same contract as the IDE's `FormRuntime`.
impl Default for CtrlState {
    fn default() -> Self {
        Self {
            props: HashMap::new(),
            visible: true,
            enabled: true,
        }
    }
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

/// See [`FormApp::state_entry_mut`] — free function so it can be unit-tested
/// without standing up a whole `FormApp` (channels, viewport, interpreter).
fn state_entry_mut<'a>(
    state: &'a mut HashMap<String, CtrlState>,
    controls: &[cobolt_forms::Control],
    key: &str,
) -> &'a mut CtrlState {
    if !state.contains_key(key) {
        let base_id = key.rsplit('.').next().unwrap_or(key);
        let seeded = controls
            .iter()
            .find(|c| c.id.eq_ignore_ascii_case(base_id))
            .map(CtrlState::from_control)
            .unwrap_or_default();
        state.insert(key.to_owned(), seeded);
    }
    state.get_mut(key).expect("entry present or just inserted")
}

fn flatten_controls(controls: &[cobolt_forms::Control], out: &mut Vec<cobolt_forms::Control>) {
    for c in controls {
        out.push(c.clone());
        flatten_controls(&c.children, out);
    }
}

/// What a COBOL animation verb asked for. The interpreter turns `PLAY ANIMATION`,
/// `STOP-ANIMATION` and `PAUSE` into writes of these pseudo-properties on the
/// control object, which reach the GUI as ordinary state updates.
#[derive(Clone, Copy, PartialEq, Debug)]
enum AnimCommand {
    Play,
    Stop,
    Pause,
}

/// Map a state-update property name to its animation verb, if it is one.
fn anim_command(prop: &str) -> Option<AnimCommand> {
    match prop.trim() {
        p if p.eq_ignore_ascii_case("_PlayAnimation") => Some(AnimCommand::Play),
        p if p.eq_ignore_ascii_case("_StopAnimation") => Some(AnimCommand::Stop),
        p if p.eq_ignore_ascii_case("_PauseAnimation") => Some(AnimCommand::Pause),
        _ => None,
    }
}

/// `FormState` over the live control-state map — merges runtime property values
/// onto each designed control so the unified engine paints the live state, and
/// supplies each control's current animation transform.
struct LiveState<'a> {
    state: &'a HashMap<String, CtrlState>,
    anim: &'a cobolt_forms::anim::AnimRuntime,
}

impl<'a> LiveState<'a> {
    /// Resolve `base.id` to its state entry case-INSENSITIVELY. Runtime updates
    /// arrive with COBOL-cased ids (unquoted identifiers are upper-cased) and
    /// databound repeating-group members are keyed by mixed-case mappings, so the
    /// designed-case id the renderer draws with does not always byte-match the
    /// state key. The in-IDE `RunState` (which drives Preview parity) already
    /// resolves case-insensitively; this external run-form path must match it, or
    /// a databound card's per-row values silently fail to merge and the card shows
    /// its designed defaults (spec 015/024 × the split run-form process).
    fn entry(&self, base: &cobolt_forms::Control) -> Option<&CtrlState> {
        self.state
            .keys()
            .find(|k| k.eq_ignore_ascii_case(&base.id))
            .and_then(|k| self.state.get(k))
    }
}

impl<'a> cobolt_forms::render::FormState for LiveState<'a> {
    fn live(&self, base: &cobolt_forms::Control) -> cobolt_forms::Control {
        match self.entry(base) {
            Some(s) => cobolt_forms::render::merge_props(base, s.props.iter()),
            None => base.clone(),
        }
    }
    fn visible(&self, base: &cobolt_forms::Control) -> bool {
        self.entry(base).map(|s| s.visible).unwrap_or(true)
    }
    fn enabled(&self, base: &cobolt_forms::Control) -> bool {
        self.entry(base).map(|s| s.enabled).unwrap_or(true)
    }
    fn transform(&self, base: &cobolt_forms::Control) -> cobolt_forms::render::RenderTransform {
        self.anim.transform(base)
    }
}

// ── Entry point ────────────────────────────────────────────────────────────────

/// `true` when the named env var holds a truthy value (`1`/`true`/`on`). Presence
/// alone is not enough: the IDE always sets these vars (to `0` when the matching
/// project diagnostic is off), so the value must be inspected.
fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on")
        })
        .unwrap_or(false)
}

fn databind_trace_enabled() -> bool {
    env_flag("COBOLT_DATABIND_TRACE")
}

/// Sanitize a project name into a safe file stem (no path separators / oddities).
fn sanitize_stem(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = s.trim_matches('_');
    if trimmed.is_empty() {
        "project".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Write the per-control diagnostics dump to `/tmp/<project>_diagnostics_dump.log`.
/// Called once at launch when any diagnostic is enabled. Best-effort: a failure to
/// write is reported on stderr but never blocks the form from running.
fn write_diagnostics_dump(project: &str, form: &cobolt_forms::Form) {
    let enabled = [
        ("frame_diagnostics", env_flag("COBOLT_FRAME_DIAGNOSTICS")),
        ("datagrid_diagnostics", env_flag("COBOLT_DATAGRID_DIAGNOSTICS")),
        ("databind_trace", env_flag("COBOLT_DATABIND_TRACE")),
    ];
    let body = cobolt_forms::diagnostics::dump_form_diagnostics(form, project, &enabled);
    // The user-facing contract is <diagnostics dir>/<project>_diagnostics_dump.log
    // — `/tmp` on Linux/macOS (deliberately not the per-process /var/folders path
    // std::env::temp_dir gives on macOS), `%TEMP%` on Windows, which has no /tmp.
    let path = cobolt_runtime::diag_path::diagnostics_file(&format!(
        "{}_diagnostics_dump.log",
        sanitize_stem(project)
    ));
    match std::fs::write(&path, body) {
        Ok(()) => eprintln!("run-form: wrote diagnostics dump to {}", path.display()),
        Err(e) => eprintln!("run-form: could not write diagnostics dump to {}: {e}", path.display()),
    }
}

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
    // When ANY diagnostic is enabled, the IDE forwards `--diagnostics-dump
    // <project>`; write the detailed per-control dump once, at launch.
    let diagnostics_dump_project: Option<String> = args
        .iter()
        .position(|a| a == "--diagnostics-dump")
        .and_then(|i| args.get(i + 1))
        .cloned();

    // ── Load the form layout ──────────────────────────────────────────────────
    let form = match cobolt_forms::load_form(&cfrm_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("run-form: cannot load {}: {e}", cfrm_path.display());
            process::exit(1);
        }
    };

    if let Some(project) = diagnostics_dump_project.as_deref() {
        write_diagnostics_dump(project, &form);
    }

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
                        // TODO(debug-user-scope): wire the interpreter's user-only
                        // stepping once `Interpreter::set_debug_user_scope` lands
                        // (parked "hide generated code" feature). Accepted here so
                        // the child never rejects the command; currently a no-op.
                        Ok(RemoteDebugCmd::SetUserScope { .. }) => {}
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
        bg_gradient_enabled: form.background_gradient_enabled,
        bg_gradient_start: form.background_gradient_start_color.clone(),
        bg_gradient_end: form.background_gradient_end_color.clone(),
        bg_gradient_direction: form.background_gradient_direction.clone(),
        transparency: form.transparency.clamp(0, 100) as u8,
        bg_image: form.background_image.clone(),
        bg_mode: form.bg_image_mode,
        use_theme_background: form.use_theme_background,
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
        db_dumped: false,
        anim: cobolt_forms::anim::AnimRuntime::new(fw, fh),
        anim_started: false,
        last_frame: None,
        hovered: std::collections::HashSet::new(),
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(&title)
        // Size the window exactly to the form. The previous +4 slack left a
        // strip of panel/scrollbar-gutter visible on the right and bottom edges.
        .with_inner_size([fw, fh])
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
    bg_gradient_enabled: bool,
    bg_gradient_start: String,
    bg_gradient_end: String,
    bg_gradient_direction: String,
    transparency: u8,
    bg_image: String,
    bg_mode: cobolt_forms::model::BgImageMode,
    /// The form's `UseThemeBackground` opt-in — the pack's background art
    /// replaces the form's own image when the active theme provides one.
    use_theme_background: bool,
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
    /// One-shot guard for the `COBOLT_DATABIND_TRACE` render-side dump.
    db_dumped: bool,
    /// Control animations (fly-in, fade, pulse, …). The run form used to have no
    /// clock at all, so every animated control simply drew in its final place —
    /// this runs the same effects the designer preview shows.
    anim: cobolt_forms::anim::AnimRuntime,
    /// One-shot guard for the load-time (`OnFormLoad` / `OnShow`) animations.
    anim_started: bool,
    /// Previous frame's timestamp — the animation clock's delta.
    last_frame: Option<std::time::Instant>,
    /// Controls the pointer was inside last frame, so `OnHover` animations fire
    /// on entry only. Hover/click triggers are derived from the rendered rects
    /// rather than from `RenderOutput::events`, which the engine emits only for
    /// events that have a bound COBOL handler.
    hovered: std::collections::HashSet<String>,
}

impl FormApp {
    fn send_event(&mut self, ev: FormEvent) {
        if self.ev_tx.send(ev).is_ok() {
            self.pending.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Opt-in diagnostic (`COBOLT_DATABIND_TRACE=1`). For each repeating-group
    /// member of instance 1, write the exact id the renderer looks up and whether
    /// that id is present in `state` byte-exact vs. case-insensitively. A CI-only
    /// hit means the value landed under a differently-cased key than the render
    /// draws with — the classic run-form databind blank. Written once to
    /// `cobolt-databind-render.log` in the platform's diagnostics directory.
    fn dump_databind_trace(&self) {
        use std::io::Write;
        let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(cobolt_runtime::diag_path::diagnostics_file(
                "cobolt-databind-render.log",
            ))
        else {
            return;
        };
        let _ = writeln!(
            f,
            "\n=== RENDER-SIDE DATABIND TRACE ({}) ===",
            self.form_name
        );
        let inst_keys = self.state.keys().filter(|k| k.contains('.')).count();
        let _ = writeln!(f, "state has {inst_keys} instanced ('.') keys total");
        for g in &self.controls {
            let is_rep = matches!(g.control_type, cobolt_forms::ControlType::GroupBox)
                && g.get_prop("IsRepeatingGroup")
                    .map(|v| v.as_bool())
                    .unwrap_or(false);
            if !is_rep {
                continue;
            }
            let members: Vec<&cobolt_forms::Control> = self
                .controls
                .iter()
                .filter(|c| c.parent.as_deref().map(|p| p.eq_ignore_ascii_case(&g.id)).unwrap_or(false))
                .collect();
            let _ = writeln!(
                f,
                "group '{}' members=[{}]",
                g.id,
                members.iter().map(|m| m.id.as_str()).collect::<Vec<_>>().join(", ")
            );
            for m in &members {
                let id = cobolt_forms::render::member_instance_id(&g.id, &m.id, 1);
                let exact = self.state.contains_key(&id);
                let ci = self.state.keys().find(|k| k.eq_ignore_ascii_case(&id)).cloned();
                let _ = writeln!(
                    f,
                    "  lookup '{id}' -> exact={exact} ci_key={:?}",
                    ci.filter(|k| *k != id)
                );
            }
        }
        let _ = writeln!(f, "sample instanced keys:");
        for k in self.state.keys().filter(|k| k.contains('.')).take(8) {
            let _ = writeln!(f, "  {k}");
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

    /// The mutable state entry for a DRAWN control id, created when missing.
    /// Repeating-group instances (`group.group-N.member`) never exist in the
    /// initial map — it is seeded from the designed controls only — so a card
    /// member's first databind write would otherwise land in a bare default
    /// entry. Seed such an entry from the designed template control (last
    /// dotted segment) so the instance inherits its designed visibility,
    /// enablement, and properties.
    fn state_entry_mut(&mut self, key: &str) -> &mut CtrlState {
        state_entry_mut(&mut self.state, &self.controls, key)
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
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Form windows render through Context-level panels; only the Context
        // is needed per frame.
        let ctx = root_ui.ctx().clone();
        let ctx = &ctx;
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

        // ── Animation clock ──────────────────────────────────────────────────
        // Load-time animations start with the window; everything after is driven
        // by triggers below. `tick` returns true while something is moving, which
        // keeps the frame scheduler awake at the end of this method.
        if !self.anim_started {
            self.anim_started = true;
            self.anim.start_form_load(&self.controls);
        }
        let now = std::time::Instant::now();
        let dt = self
            .last_frame
            .map(|t| now.duration_since(t).as_secs_f32())
            .unwrap_or(0.0);
        self.last_frame = Some(now);
        let animating = self.anim.tick(dt);

        // Apply property changes coming from the COBOL interpreter. Route each
        // update to the designer-case state key (COBOL upper-cases ids), and
        // repeating-group member writes to the drawn card-instance id — the
        // same resolution the IDE's FormRuntime::drain_state performs.
        let mut drained = 0usize;
        while let Ok(u) = self.state_rx.try_recv() {
            // COBOL's PLAY ANIMATION / STOP-ANIMATION / PAUSE arrive as writes to
            // these pseudo-properties; act on the write, don't store it.
            if let Some(cmd) = anim_command(&u.prop) {
                match cmd {
                    AnimCommand::Play => {
                        self.anim
                            .play_programmatic(&self.controls, &u.ctrl_id, &u.value)
                    }
                    AnimCommand::Stop => self.anim.stop_all(&u.ctrl_id),
                    AnimCommand::Pause => self.anim.pause_all(&u.ctrl_id),
                }
                drained += 1;
                continue;
            }
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
            self.state_entry_mut(&key).set(&u.prop, u.value);
            drained += 1;
        }

        // ── One-shot databind diagnostic (opt-in) ────────────────────────────
        // COBOLT_DATABIND_TRACE=1 (also true/on) writes, once, the mismatch
        // between the state keys the interpreter populated and the instanced ids
        // the renderer will look up for each repeating-group member. Decisive for
        // "cards show designed defaults in run-form but not in preview". The IDE
        // sets this from the project's Data-bind trace setting — always (incl.
        // "0"), so test the value rather than mere presence.
        if !self.db_dumped
            && databind_trace_enabled()
            && self.state.keys().any(|k| k.contains('.'))
        {
            self.db_dumped = true;
            self.dump_databind_trace();
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
            let st = LiveState {
                state: &self.state,
                anim: &self.anim,
            };
            let active_tabs = cobolt_forms::containers::ActiveTabs::default();
            let backdrop = cobolt_forms::render::Backdrop {
                color_hex: self.bg_hex.clone(),
                transparency: self.transparency,
                gradient_enabled: self.bg_gradient_enabled,
                gradient_start_hex: self.bg_gradient_start.clone(),
                gradient_end_hex: self.bg_gradient_end.clone(),
                gradient_direction: self.bg_gradient_direction.clone(),
                image: backdrop_image,
                image_mode: self.bg_mode,
                use_theme_background: self.use_theme_background,
            };
            let mut out = cobolt_forms::render::RenderOutput::default();
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(bg_fill))
                .show(root_ui, |ui| {
                    // Floating scrollbars overlay the content instead of
                    // reserving a gutter, so no light track strip shows on the
                    // right/bottom edges when the form fits (only appears, as an
                    // overlay, if the user shrinks the resizable window).
                    ui.style_mut().spacing.scroll = egui::style::ScrollStyle::floating();
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

        // ── Animation triggers from this frame's interaction ─────────────────
        // Pointer triggers come from the rendered rects: the engine only emits
        // onClick/onHoverEnter for controls that have a bound COBOL handler, but
        // an animation is reason enough on its own. Focus and timer triggers do
        // come from the event stream (`onTick` always fires; `onGotFocus` fires
        // when bound).
        if armed {
            let (clicked, pointer) =
                ctx.input(|i| (i.pointer.primary_clicked(), i.pointer.interact_pos()));
            let mut still_hovered = std::collections::HashSet::new();
            for (id, rect) in &output.control_rects {
                // Repeating-group card instances are drawn under a composite id
                // and carry their own placement effect; leave them alone.
                if id.contains('.') {
                    continue;
                }
                let over = pointer.map(|p| rect.contains(p)).unwrap_or(false);
                if over {
                    still_hovered.insert(id.clone());
                    if !self.hovered.contains(id) {
                        self.anim.fire_event(&self.controls, id, "onHoverEnter");
                    }
                    if clicked {
                        self.anim.fire_event(&self.controls, id, "onClick");
                    }
                }
            }
            self.hovered = still_hovered;
            for ev in &output.events {
                // Pointer events are already covered by the rect pass above —
                // taking them from here too would restart the same animation twice.
                if ev.event.eq_ignore_ascii_case("onClick")
                    || ev.event.eq_ignore_ascii_case("onHoverEnter")
                {
                    continue;
                }
                self.anim.fire_event(&self.controls, &ev.ctrl_id, &ev.event);
            }
        }

        // Apply value updates locally, sync them to the interpreter (so
        // handlers read the live value), and forward UI events — once armed.
        let mut interacted = false;
        if armed {
            for (id, key, val) in &output.prop_updates {
                self.state_entry_mut(id).set(key, val.clone());
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
        // A running animation needs frames of its own: without this the form
        // sleeps between interpreter traffic and a fly-in would advance in 200 ms
        // jumps (or freeze mid-flight on an idle form).
        let busy = drained > 0
            || interacted
            || animating
            || self.anim.is_animating()
            || self.pending.load(Ordering::Relaxed) > 0;
        let ms = if busy { 16 } else { 200 };
        ctx.request_repaint_after(std::time::Duration::from_millis(ms));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::{Control, ControlType};

    /// A repeating-group card member the interpreter writes to before the id
    /// exists in `state` must stay VISIBLE. With a derived `Default` the entry
    /// was born `visible: false`, so the renderer skipped exactly the controls a
    /// data binding had just populated — databound cards blank in the run form
    /// while the DataGrid (pre-seeded key) painted fine.
    #[test]
    fn new_instance_entry_is_visible_and_keeps_designed_props() {
        let mut label = Control::new("Label-1", ControlType::Label, 0, 0);
        label.set_prop("Caption", cobolt_forms::PropValue::from("designed"));
        label.parent = Some("GroupBox-2".into());
        let controls = vec![label];

        let mut state: HashMap<String, CtrlState> = HashMap::new();
        let key = cobolt_forms::render::member_instance_id("GroupBox-2", "Label-1", 1);
        state_entry_mut(&mut state, &controls, &key).set("Caption", "Leonardo DiCaprio".into());

        let entry = &state[&key];
        assert!(entry.visible, "card-instance state entry must start visible");
        assert!(entry.enabled, "card-instance state entry must start enabled");
        assert_eq!(entry.props.get("Caption").map(String::as_str), Some("Leonardo DiCaprio"));
    }

    /// An id with no matching designed control (an unknown write) still lands in
    /// a visible entry rather than a silently hidden one.
    #[test]
    fn unknown_id_entry_defaults_to_visible() {
        let mut state: HashMap<String, CtrlState> = HashMap::new();
        state_entry_mut(&mut state, &[], "Nope-1").set("Caption", "x".into());
        assert!(state["Nope-1"].visible);
        assert!(state["Nope-1"].enabled);
    }

    /// A member designed hidden stays hidden when its instance entry is created.
    #[test]
    fn instance_entry_inherits_designed_visibility() {
        let mut label = Control::new("Label-9", ControlType::Label, 0, 0);
        label.visible = false;
        let controls = vec![label];
        let mut state: HashMap<String, CtrlState> = HashMap::new();
        let key = cobolt_forms::render::member_instance_id("GroupBox-2", "Label-9", 3);
        state_entry_mut(&mut state, &controls, &key).set("Caption", "v".into());
        assert!(!state[&key].visible);
    }
}
