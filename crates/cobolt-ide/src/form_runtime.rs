// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Form Runtime Engine — Phase 6.
//!
//! `FormRuntime` owns the background interpreter thread that executes a form's
//! generated COBOL, plus the three channels that connect it to the UI thread:
//!
//! ```text
//! UI thread (egui)                    Interpreter thread
//! ─────────────────────────────       ──────────────────────────────────────
//! FormRuntime.send_event()  ────────► COBOL-WAIT-EVENT (blocks on recv)
//! FormRuntime.drain_state() ◄───────  COBOL-SET-PROPERTY (sends StateUpdate)
//! FormRuntime.drain_display()◄──────  DISPLAY statement (sends String)
//! ```
//!
//! The interpreter thread terminates when:
//!  - COBOL executes `STOP RUN`
//!  - The UI sends `FormEvent::quit()` (closing the form window)
//!  - The channel is dropped (UI drops `FormRuntime`)

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cobolt_forms::{BindingSourceDescriptor, BindingTargetDescriptor, Form};
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};
use cobolt_semantic::analyze;

// ── FormRuntime ───────────────────────────────────────────────────────────────

/// Manages one live COBOL form execution.
pub struct FormRuntime {
    /// Path to the `.cfrm` file (used to identify which form is running).
    pub form_path: PathBuf,
    /// The form's id/name — the `COBOL-CONTROL-ID` for form-level events
    /// (onShow, onActivate, onResize, …) dispatched by the generated loop.
    pub form_name: String,
    /// Title shown in the running-form viewport.
    pub form_title: String,
    /// Current width/height of the form canvas.
    pub form_width: u32,
    pub form_height: u32,
    /// Form background colour (hex RGB, e.g. "141622") and transparency (0–100).
    pub background_color: String,
    pub background_gradient_enabled: bool,
    pub background_gradient_start_color: String,
    pub background_gradient_end_color: String,
    pub background_gradient_direction: String,
    pub transparency: u8,
    /// Optional form background image (path) + how it's scaled.
    pub background_image: String,
    pub bg_image_mode: cobolt_forms::model::BgImageMode,
    /// Controls snapshot (id → props map), populated at launch from the form
    /// model and updated by `drain_state()` as COBOL-SET-PROPERTY arrives.
    pub ctrl_state: HashMap<String, CtrlState>,
    /// Controls in z_order (for rendering order). Populated at launch.
    pub ctrl_order: Vec<CtrlMeta>,
    /// The designed controls (flat), used as the base for the unified render
    /// engine; live values are merged on top from `ctrl_state` (spec 017).
    pub controls: Vec<cobolt_forms::Control>,

    /// Sends UI events to the interpreter thread.
    event_tx: Sender<FormEvent>,
    /// Sends UI-driven property changes (slider drag, text edit, …) to the
    /// interpreter so event handlers read the live value, not the seeded default.
    input_tx: Sender<StateUpdate>,
    /// Receives property-change notifications from the interpreter.
    state_rx: Receiver<StateUpdate>,
    /// Receives DISPLAY output from the interpreter.
    display_rx: Receiver<String>,
    /// Set to true to request the interpreter thread to stop. Doubles as the
    /// interpreter's cooperative cancellation flag, so a looping/long-running
    /// handler aborts between statements instead of hanging on close.
    stop_flag: Arc<AtomicBool>,
    /// Depth of the UI→interpreter event queue. Incremented on `send_event`,
    /// decremented by the interpreter as it consumes each event. Used to
    /// coalesce timer ticks (skip a new `onTick` while the queue is non-empty).
    pending: Arc<AtomicUsize>,
    /// Set by the interpreter thread when `run()` returns a fatal error, so the
    /// UI can surface it in a modal dialog (the IDE stays open).
    error_slot: Arc<Mutex<Option<String>>>,
    /// Handle to the interpreter thread.
    handle: Option<JoinHandle<()>>,
    /// Tracks which ComboBox (by control ID) is currently open in the running form.
    pub combo_open: HashMap<String, bool>,
    /// Whether to render with the Liquid-Glass look. Mirrors the launching
    /// designer's glass toggle so the running form matches the canvas (WYSIWYG).
    pub glass: bool,
    /// Design-intent adjustments made interactively while the form runs (e.g. a
    /// DataGrid's column widths / row height), captured from `prop_updates` by a
    /// whitelist. The "Apply layout to design" button writes these back into the
    /// owning designer's form so they persist as the control's new defaults.
    /// `ctrl_id → (property → value)`. Runtime *data* is never captured here.
    pub pending_design_props:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,

    /// When running in isolated process mode (recommended path).
    child: Option<Child>,
    child_stdin: Option<Mutex<Option<ChildStdin>>>,

    /// True once the child process (or legacy thread) has exited.
    finished: Arc<AtomicBool>,

    /// Unique run ID for this form execution instance to reset animation clocks.
    pub run_id: u64,
}

/// Per-control metadata needed for rendering (type + rect + initial props).
#[derive(Clone, Debug)]
pub struct CtrlMeta {
    pub id: String,
    pub control_type: cobolt_forms::ControlType,
    pub rect: cobolt_forms::model::Rect,
    pub z_order: i32,
    pub animations: Vec<cobolt_forms::model::AnimationDef>,
    /// Containment (spec 012) so the running form clips children to their
    /// container and scopes tab pages, exactly like the designer/preview.
    pub parent: Option<String>,
    pub tab: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct CtrlState {
    pub props: HashMap<String, String>,
    pub visible: bool,
    pub enabled: bool,
}

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
        Self {
            props,
            visible: ctrl.visible,
            enabled: ctrl.enabled,
        }
    }

    /// Read a property — **case-insensitive** (the inline `::Caption` arrives
    /// upper-cased, while designed values keep their model case — spec 010).
    pub fn get(&self, key: &str) -> &str {
        if let Some(v) = self.props.get(key) {
            return v.as_str();
        }
        self.props
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
            .unwrap_or("")
    }

    pub fn set(&mut self, key: &str, value: String) {
        match key.to_ascii_uppercase().as_str() {
            "VISIBLE" => self.visible = value != "0" && value != "false",
            "ENABLED" => self.enabled = value != "0" && value != "false",
            _ => {}
        }
        // Overwrite any existing case-insensitive entry so a runtime update
        // (e.g. `::Caption` → "CAPTION") never shadows the designed "Caption".
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

impl FormRuntime {
    /// Launch a new form runtime from a `Form` model.
    ///
    /// Generates COBOL from the form, parses it, runs semantic analysis,
    /// and spawns the interpreter in a background thread.
    ///
    /// Returns `Err(String)` if parse/semantic fails.
    pub fn launch(form: &Form, form_path: PathBuf) -> Result<Self, String> {
        // Generate COBOL source from the form model.
        let cobol_source = cobolt_codegen::generate(form);

        // Lex → parse → semantic.
        let tokens = tokenize(&cobol_source, SourceFormat::Free);
        let parse_result = parse(tokens);

        let parse_has_errors = parse_result
            .diagnostics
            .iter()
            .any(|d| d.severity == cobolt_parser::Severity::Error);
        if parse_result.program.is_none() || parse_has_errors {
            let msgs: Vec<_> = parse_result
                .diagnostics
                .iter()
                .map(|d| format!("{}:{} {}", d.span.line, d.span.col, d.message))
                .collect();
            return Err(format!("Parse failed:\n{}", msgs.join("\n")));
        }
        let program = parse_result.program.unwrap();

        let sem = analyze(&program);
        if !sem.is_ok() {
            let msgs: Vec<_> = sem
                .diagnostics
                .iter()
                .map(|d| format!("{}:{} {}", d.span.line, d.span.col, d.message))
                .collect();
            return Err(format!("Semantic errors:\n{}", msgs.join("\n")));
        }

        // Snapshot the form layout for the UI renderer.
        let ctrl_state: HashMap<String, CtrlState> = collect_controls(&form.controls)
            .into_iter()
            .map(|c| (c.id.clone(), CtrlState::from_control(c)))
            .collect();

        let mut ctrl_order: Vec<CtrlMeta> = collect_controls(&form.controls)
            .into_iter()
            .map(|c| CtrlMeta {
                id: c.id.clone(),
                control_type: c.control_type.clone(),
                rect: c.rect,
                z_order: c.z_order,
                animations: c.animations.clone(),
                parent: c.parent.clone(),
                tab: c.tab,
            })
            .collect();
        ctrl_order.sort_by_key(|m| m.z_order);

        // Flattened designed controls — the engine's render base (spec 017).
        let controls: Vec<cobolt_forms::Control> = collect_controls(&form.controls)
            .into_iter()
            .cloned()
            .collect();

        // Seed the interpreter's visual-object registry with every control's
        // designed properties, so property references and method getters return
        // the configured values before any setter runs.
        let seed: Vec<(String, String, Vec<(String, String)>)> = collect_controls(&form.controls)
            .into_iter()
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
                append_data_binding_seed_props(form, &c.id, &mut props);
                // Run-form only: detailed dump for repeating GroupBoxes (ControlArray databind) to debug why no cards generated.
                if matches!(c.control_type, cobolt_forms::ControlType::GroupBox) {
                    // (debug instrumentation for databind removed; RefreshBinding support is now in place)
                }
                (c.id.clone(), c.control_type.as_str().to_string(), props)
            })
            .collect();

        let stop_flag = Arc::new(AtomicBool::new(false));
        let pending = Arc::new(AtomicUsize::new(0));
        let error_slot = Arc::new(Mutex::new(None));
        let finished = Arc::new(AtomicBool::new(false));

        // Find the rcrun binary next to the current executable (works in debug + release)
        let exe = std::env::current_exe().map_err(|e| format!("failed to get current exe: {e}"))?;
        let rcrun_path = sibling_rcrun(&exe);

        let mut child = Command::new(&rcrun_path)
            .arg("run-form-ipc")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .or_else(|_| Command::new("rcrun")
                .arg("run-form-ipc")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn())
            .map_err(|e| {
                format!(
                    "failed to spawn rcrun run-form-ipc: {e}. Make sure `cargo build -p cobolt-cli` (produces target/debug/rcrun) or rcrun is in PATH."
                )
            })?;

        let mut child_stdin = child.stdin.take().expect("stdin");
        let mut child_stdout = child.stdout.take().expect("stdout");

        // Send init (cobol_source, seed) -- runner will parse
        let cobol_source_for_runner = cobolt_codegen::generate(form);
        let init = (cobol_source_for_runner, seed);
        let init_bytes = bincode::serialize(&init).map_err(|e| format!("serialize init: {e}"))?;
        write_framed(&mut child_stdin, &init_bytes).map_err(|e| format!("write init: {e}"))?;

        // Create the channels the IDE code expects
        let (event_tx, _event_rx) = mpsc::channel::<FormEvent>();
        let (input_tx, _input_rx) = mpsc::channel::<StateUpdate>();
        let (state_tx, state_rx) = mpsc::channel::<StateUpdate>();
        let (display_tx, display_rx) = mpsc::channel::<String>();

        // Pump: child stdout → local channels
        let p_state = state_tx.clone();
        let p_display = display_tx.clone();
        let p_err = Arc::clone(&error_slot);
        let p_stop = Arc::clone(&stop_flag);
        let p_finished = Arc::clone(&finished);
        std::thread::spawn(move || loop {
            if p_stop.load(Ordering::Relaxed) {
                p_finished.store(true, Ordering::Relaxed);
                break;
            }
            if let Ok(bytes) = read_framed(&mut child_stdout) {
                if let Ok(msg) = bincode::deserialize::<cobolt_runtime::FormIpcMessage>(&bytes) {
                    match msg {
                        cobolt_runtime::FormIpcMessage::State(s) => {
                            let _ = p_state.send(s);
                        }
                        cobolt_runtime::FormIpcMessage::Display(d) => {
                            let _ = p_display.send(d);
                        }
                        cobolt_runtime::FormIpcMessage::Error(e) => {
                            if let Ok(mut slot) = p_err.lock() {
                                *slot = Some(e);
                            }
                            p_finished.store(true, Ordering::Relaxed);
                            break;
                        }
                        cobolt_runtime::FormIpcMessage::Done => {
                            p_finished.store(true, Ordering::Relaxed);
                            break;
                        }
                        _ => {}
                    }
                }
            } else {
                p_finished.store(true, Ordering::Relaxed);
                break;
            }
        });

        Ok(Self {
            form_path,
            form_name: form.name.clone(),
            form_title: form.title.clone(),
            form_width: form.width,
            form_height: form.height,
            background_color: form.background_color.clone(),
            background_gradient_enabled: form.background_gradient_enabled,
            background_gradient_start_color: form.background_gradient_start_color.clone(),
            background_gradient_end_color: form.background_gradient_end_color.clone(),
            background_gradient_direction: form.background_gradient_direction.clone(),
            transparency: form.transparency.clamp(0, 100) as u8,
            background_image: form.background_image.clone(),
            bg_image_mode: form.bg_image_mode,
            ctrl_state,
            ctrl_order,
            controls,
            event_tx,
            input_tx,
            state_rx,
            display_rx,
            stop_flag,
            pending,
            error_slot,
            handle: None,
            combo_open: HashMap::new(),
            glass: true,
            pending_design_props: std::collections::BTreeMap::new(),
            child: Some(child),
            child_stdin: Some(Mutex::new(Some(child_stdin))),
            finished,
            run_id: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
        })
    }

    /// Send a UI event to the interpreter. Tracks queue depth so timer ticks
    /// can be coalesced against a backlog (see [`pending_events`]).
    pub fn send_event(&mut self, event: FormEvent) {
        if let Some(ref stdin_mutex) = self.child_stdin {
            if let Ok(mut guard) = stdin_mutex.lock() {
                if let Some(ref mut stdin) = *guard {
                    let msg = cobolt_runtime::FormIpcMessage::Event(event);
                    if let Ok(bytes) = bincode::serialize(&msg) {
                        let _ = write_framed(stdin, &bytes);
                        self.pending.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            return;
        }
        // legacy in-process
        if self.event_tx.send(event).is_ok() {
            self.pending.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Number of UI events still queued for the interpreter. The caller skips
    /// enqueuing another timer `onTick` while this is non-zero, so a handler
    /// slower than the tick interval can never flood the queue (WinForms-style
    /// tick coalescing) — which is what kept the UI responsive.
    pub fn pending_events(&self) -> usize {
        if self.child.is_some() {
            // For isolated process, we don't have accurate cross-process queue depth.
            // Return 0 so that timer ticks are never coalesced (they will be sent).
            // This keeps forms with Timers working. If a handler is very slow the
            // queue may grow, but stop_flag still aborts it.
            0
        } else {
            self.pending.load(Ordering::Relaxed)
        }
    }

    /// Take any fatal runtime error reported by the interpreter thread. Returns
    /// `Some(msg)` once; the UI shows it in a modal dialog without closing.
    pub fn take_error(&self) -> Option<String> {
        self.error_slot.lock().ok().and_then(|mut s| s.take())
    }

    /// Forward a UI-driven property change (a dragged slider value, typed text,
    /// combo selection, …) to the interpreter so an event handler reads the live
    /// value. Send this BEFORE the matching event so `COBOL-WAIT-EVENT` folds it
    /// into the object registry ahead of dispatch.
    pub fn send_input(&mut self, ctrl_id: &str, prop: &str, value: &str) {
        if let Some(ref stdin_mutex) = self.child_stdin {
            if let Ok(mut guard) = stdin_mutex.lock() {
                if let Some(ref mut stdin) = *guard {
                    let msg = cobolt_runtime::FormIpcMessage::Input(StateUpdate::new(
                        ctrl_id, prop, value,
                    ));
                    if let Ok(bytes) = bincode::serialize(&msg) {
                        let _ = write_framed(stdin, &bytes);
                    }
                }
            }
            return;
        }
        let _ = self.input_tx.send(StateUpdate::new(ctrl_id, prop, value));
    }

    /// Drain all pending `StateUpdate` messages and apply them to `ctrl_state`.
    /// Returns `true` if any updates were applied (UI should repaint).
    pub fn drain_state(&mut self) -> bool {
        let mut changed = false;
        loop {
            match self.state_rx.try_recv() {
                Ok(upd) => {
                    // A repeating-group member write (`Member(idx)::Prop`) carries a
                    // 1-based instance index. Route it to the cloned instance's id
                    // (`<group>.<group>-<idx>.<member>`) that the renderer produced,
                    // so each card shows its own row's value.
                    let key = if upd.instance_index > 0 {
                        match self.array_member_group(&upd.ctrl_id) {
                            Some((member_id, group_id)) => {
                                cobolt_forms::render::member_instance_id(
                                    &group_id,
                                    &member_id,
                                    upd.instance_index,
                                )
                            }
                            // Not resolvable as an array member — fall back to base.
                            None => self.resolve_ctrl_key(&upd.ctrl_id),
                        }
                    } else {
                        // COBOL upper-cases unquoted control identifiers (`Label-1`
                        // becomes `LABEL-1`), but `ctrl_state` is keyed by the
                        // designer's original-case id. Resolve case-insensitively.
                        self.resolve_ctrl_key(&upd.ctrl_id)
                    };
                    let log_key = key.clone();
                    let log_prop = upd.prop.clone();
                    let log_val = upd.value.clone();
                    let log_inst = upd.instance_index;
                    let entry = self.ctrl_state.entry(key).or_default();
                    entry.set(&upd.prop, upd.value);
                    // Run-form detail (kept minimal)
                    if log_key.to_ascii_lowercase().contains("groupbox")
                        || log_prop.eq_ignore_ascii_case("ItemCount")
                        || log_prop.eq_ignore_ascii_case("DataSource")
                    {
                        tracing::debug!(target: "databinding", "RUN-FORM STATE_UPDATE {} {}", log_key, log_prop);
                    }
                    changed = true;
                }
                Err(_) => break,
            }
        }
        changed
    }

    fn resolve_ctrl_key(&self, id: &str) -> String {
        if let Some(k) = self.ctrl_state.keys().find(|k| k.eq_ignore_ascii_case(id)) {
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

    /// For a repeating-group member id (case-insensitive), return its original-case
    /// id and the id of its repeating-GroupBox ancestor. `None` when the control
    /// isn't a member of a repeating group.
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

    /// Drain all pending DISPLAY output lines. Caller pushes them to the
    /// IDE output panel.
    pub fn drain_display(&self) -> Vec<String> {
        let mut lines = Vec::new();
        loop {
            match self.display_rx.try_recv() {
                Ok(line) => lines.push(line),
                Err(_) => break,
            }
        }
        lines
    }

    /// `true` while the interpreter thread is still running.
    pub fn is_running(&self) -> bool {
        if let Some(h) = &self.handle {
            return !h.is_finished();
        }
        if self.child.is_some() {
            return !self.finished.load(Ordering::Relaxed);
        }
        false
    }

    /// Non-blocking stop request: raise the cooperative cancellation flag and
    /// send the quit sentinel, then return immediately. Use this from a
    /// per-frame UI callback (e.g. the form window's close button) so a stuck
    /// or looping handler is aborted and the window can actually close; the
    /// finished runtime is reaped (and joined, cheaply) on a later frame.
    pub fn request_stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        self.finished.store(true, Ordering::Relaxed);
        if let Some(ref stdin_mutex) = self.child_stdin {
            if let Ok(mut guard) = stdin_mutex.lock() {
                if let Some(ref mut stdin) = *guard {
                    let quit = cobolt_runtime::FormIpcMessage::Quit;
                    if let Ok(bytes) = bincode::serialize(&quit) {
                        let _ = write_framed(stdin, &bytes);
                    }
                }
            }
        } else {
            let _ = self.event_tx.send(FormEvent::quit());
        }
    }

    /// Request the interpreter to stop and clean up.
    ///
    /// Sets the cooperative cancellation flag (aborting any running/looping
    /// handler between statements) and sends a quit sentinel to unblock an idle
    /// `COBOL-WAIT-EVENT`. Then it waits only a bounded grace period for the
    /// thread to finish before **detaching** it, so a genuinely stuck statement
    /// (e.g. a large blocking file read) can never freeze the UI thread on
    /// close, relaunch, or exit. With cancellation, the thread almost always
    /// exits within a couple of milliseconds.
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        self.finished.store(true, Ordering::Relaxed);

        // Send quit via IPC if we have a child
        if let Some(ref stdin_mutex) = self.child_stdin {
            if let Ok(mut guard) = stdin_mutex.lock() {
                if let Some(ref mut stdin) = *guard {
                    let quit = cobolt_runtime::FormIpcMessage::Quit;
                    if let Ok(bytes) = bincode::serialize(&quit) {
                        let _ = write_framed(stdin, &bytes);
                    }
                }
            }
        } else {
            // legacy in-process path
            let _ = self.event_tx.send(FormEvent::quit());
        }

        if let Some(h) = self.handle.take() {
            let deadline = Instant::now() + Duration::from_millis(300);
            loop {
                if h.is_finished() {
                    let _ = h.join();
                    return;
                }
                if Instant::now() >= deadline {
                    return;
                }
                thread::sleep(Duration::from_millis(2));
            }
        }

        // Kill child process if present
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        self.child_stdin = None;
    }
}

impl Drop for FormRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn append_data_binding_seed_props(
    form: &Form,
    control_id: &str,
    props: &mut Vec<(String, String)>,
) {
    // Seeds _Binding* props for DataGrid and for databound repeating GroupBoxes (ControlArray).
    // This allows RefreshBinding() at runtime for both.
    let binding = form.data_bindings.iter().find(|binding| {
        match &binding.target {
            BindingTargetDescriptor::DataGrid {
                control_id: target_id,
            } => target_id.eq_ignore_ascii_case(control_id),
            BindingTargetDescriptor::ControlArray { array_id, .. } => {
                // The control_id here may be the group id or we match by checking if this control is the array host
                // For simplicity, if the control looks like a repeating group and array_id matches its id or explicit
                array_id.eq_ignore_ascii_case(control_id)
                    || collect_controls(&form.controls).iter().any(|c| {
                        c.id.eq_ignore_ascii_case(control_id)
                            && c.explicit_control_array_id().as_deref() == Some(array_id.as_str())
                    })
            }
            // Spec 039 R21: a standalone Knob/Gauge/Switch.
            BindingTargetDescriptor::ScalarControl {
                control_id: target_id,
            } => target_id.eq_ignore_ascii_case(control_id),
            // Spec 039 R22: a Maps control's Markers collection.
            BindingTargetDescriptor::MarkerCollection {
                control_id: target_id,
            } => target_id.eq_ignore_ascii_case(control_id),
            BindingTargetDescriptor::Chart { .. }
            | BindingTargetDescriptor::ComboBox { .. }
            | BindingTargetDescriptor::ListBox { .. } => false,
        }
    });
    let Some(binding) = binding else {
        return;
    };
    let BindingSourceDescriptor::CobolTable { fields, .. } = &binding.source else {
        return;
    };
    props.push(("_BindingKind".into(), "CobolTable".into()));
    props.push((
        "_BindingFields".into(),
        fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    ));
    if matches!(
        &binding.target,
        BindingTargetDescriptor::ControlArray { .. }
    ) {
        props.push(("_BindingArray".into(), "1".into()));
        // Seed mappings (sourceField<TAB>memberId<TAB>prop) so RefreshBinding can
        // hydrate live row values into the instanced member controls.
        let maps: Vec<String> = binding
            .mappings
            .iter()
            .filter_map(|m| {
                if let cobolt_forms::BindingTargetPath::ControlProperty {
                    control_id: member,
                    property_name: prop,
                    ..
                } = &m.target
                {
                    Some(format!("{}\t{}\t{}", m.source_field, member, prop))
                } else {
                    None
                }
            })
            .collect();
        if !maps.is_empty() {
            props.push(("_BindingMappings".into(), maps.join("\n")));
        }
    }
    if let BindingTargetDescriptor::ScalarControl { .. } = &binding.target {
        // Spec 039 R21: seed the single mapped field + which property it
        // writes (Value for Knob/Gauge, Checked for Switch), mirroring the
        // ControlArray seeding above — `refresh_binding` (interpreter.rs)
        // reads these the same way it already reads `_BindingArray`/
        // `_BindingFields` for the DataGrid/ControlArray case.
        if let Some(scalar_field) = binding
            .mappings
            .iter()
            .find(|m| matches!(&m.target, cobolt_forms::BindingTargetPath::ScalarValue { .. }))
            .map(|m| m.source_field.clone())
        {
            let property = form
                .find_control(control_id)
                .and_then(|c| c.scalar_binding_property())
                .unwrap_or("Value");
            props.push(("_BindingScalarField".into(), scalar_field));
            props.push(("_BindingScalarProperty".into(), property.to_owned()));
        }
    }
    if let BindingTargetDescriptor::MarkerCollection { .. } = &binding.target {
        // Spec 039 T13/R22: seed one source field per marker attribute (in a
        // fixed order — refresh_marker_binding in interpreter.rs reads them
        // positionally), same sibling relationship to the DataGrid/
        // ScalarControl seeding above.
        if let Some(spec) = marker_binding_seed(binding) {
            props.push(("_BindingMarkerFields".into(), spec));
        }
    }
}

/// Build the `_BindingMarkerFields` seed value (`id\tlat\tlng\tlabel\tinfo`,
/// any entry empty except lat/lng — enforced by the Guardian before a binding
/// can be saved) from a `MarkerCollection` binding's field mappings.
fn marker_binding_seed(binding: &cobolt_forms::DataBindingDef) -> Option<String> {
    let field_for = |target: cobolt_forms::MapMarkerField| -> String {
        binding
            .mappings
            .iter()
            .find_map(|m| match &m.target {
                cobolt_forms::BindingTargetPath::MarkerField { field, .. } if *field == target => {
                    Some(m.source_field.clone())
                }
                _ => None,
            })
            .unwrap_or_default()
    };
    let lat = field_for(cobolt_forms::MapMarkerField::Lat);
    let lng = field_for(cobolt_forms::MapMarkerField::Lng);
    if lat.is_empty() || lng.is_empty() {
        return None;
    }
    let id = field_for(cobolt_forms::MapMarkerField::Id);
    let label = field_for(cobolt_forms::MapMarkerField::Label);
    let info = field_for(cobolt_forms::MapMarkerField::Info);
    Some(format!("{id}\t{lat}\t{lng}\t{label}\t{info}"))
}

/// Flatten nested control tree into a flat Vec (pre-order).
fn collect_controls(controls: &[cobolt_forms::Control]) -> Vec<&cobolt_forms::Control> {
    let mut out = Vec::new();
    for c in controls {
        collect_rec(c, &mut out);
    }
    out
}

fn collect_rec<'a>(ctrl: &'a cobolt_forms::Control, out: &mut Vec<&'a cobolt_forms::Control>) {
    out.push(ctrl);
    for child in &ctrl.children {
        collect_rec(child, out);
    }
}

// ── Generated-form pipeline regression test ─────────────────────────────────────
// Framed I/O helpers for the isolated runner (length u32 LE + bincode payload).
fn write_framed<W: std::io::Write>(w: &mut W, data: &[u8]) -> std::io::Result<()> {
    let len = (data.len() as u32).to_le_bytes();
    w.write_all(&len)?;
    w.write_all(data)?;
    w.flush()?;
    Ok(())
}

fn read_framed<R: std::io::Read>(r: &mut R) -> std::io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

// ── ExternalFormRun ───────────────────────────────────────────────────────────

/// A form running as a fully external `rcrun run-form` process: its own window,
/// its own event loop, its own interpreter. The IDE only spawns it, pipes its
/// stdout into the Output pane, and can kill it — the IDE render loop stays
/// idle while the form runs (and what runs is exactly what `rcrun build`
/// ships: same renderer, same interpreter, separate process).
pub struct ExternalFormRun {
    /// Path to the `.cfrm` file (identifies which form is running).
    pub form_path: PathBuf,
    /// The form's id/name — for Output pane labels.
    pub form_name: String,
    /// True when spawned with `--debug`: the IDE debugger controls the
    /// process over stdin (`@DBG` JSON commands) / stdout (`@DBG` events).
    pub debug: bool,
    child: Child,
    /// Kept only in debug mode, for sending `@DBG` command lines.
    child_stdin: Option<Mutex<ChildStdin>>,
    stdout_rx: Receiver<String>,
    stderr_buf: Arc<Mutex<Vec<String>>>,
    exit_status: Option<std::process::ExitStatus>,
}

/// Diagnostics forwarded to a `rcrun run-form` child, derived from the IDE's
/// debug settings at launch. `env` comes from
/// [`DebugSettings::child_env`](crate::debug_settings::DebugSettings::child_env)
/// — one pair per switch the child understands; `dump_project` is `Some(name)`
/// whenever ANY switch is on, which triggers the per-control diagnostics dump at
/// `/tmp/<name>_diagnostics_dump.log`.
#[derive(Clone, Default)]
pub struct RunDiagnostics {
    pub env: Vec<(&'static str, String)>,
    pub dump_project: Option<String>,
}

/// The bundled `rcrun` sitting next to the running executable.
///
/// `EXE_SUFFIX` is empty on Unix and `".exe"` on Windows, where the sibling is
/// `rcrun.exe`: spelling the name without it missed the bundled binary entirely,
/// so Run Form fell back to a bare PATH lookup and only worked if the developer
/// happened to have rcrun on PATH. The rest of the tree already handles both
/// names (see `project_model::…` and the compiler's `rcrun_name`).
fn sibling_rcrun(exe: &std::path::Path) -> PathBuf {
    exe.with_file_name(format!("rcrun{}", std::env::consts::EXE_SUFFIX))
}

/// Window-effect spawn args (spec 038, plan D2): the IDE resolves project
/// settings × the form's opt-out × the kill-switch BEFORE spawning, so the
/// child stays project-file-agnostic. `entrance`/`exit` are
/// `FxSpec::format()` triples (`id:ms:easing`).
pub struct FormFxArgs {
    pub entrance: String,
    pub exit: String,
    pub restore: bool,
}

/// Resolve the effect spawn args from project settings × the form's opt-out
/// × the machine kill-switch (038 R3/R14, plan D2). `None` when nothing
/// would play — the child then receives no `--fx-*` args at all.
pub fn resolve_fx_args(
    project: Option<&crate::project_model::CoboltProject>,
    form_effects_on: bool,
    kill_switch: bool,
) -> Option<FormFxArgs> {
    let p = project?;
    if !form_effects_on || kill_switch {
        return None;
    }
    let entrance = p.entrance_fx();
    let exit = p.exit_fx();
    if !entrance.is_active() && !exit.is_active() {
        return None;
    }
    Some(FormFxArgs {
        entrance: entrance.format(),
        exit: exit.format(),
        restore: p.forms.entrance_on_restore,
    })
}

/// `rcrun run-form`'s env var name for a resolved Google Maps API key (spec
/// 039 T12/R23) — kept in sync with `cobolt-cli/src/form_gui.rs`'s constant
/// of the same name, since the two crates don't share this module.
pub const GOOGLE_MAPS_API_KEY_ENV: &str = "COBOLT_GOOGLE_MAPS_API_KEY";

/// `Some((env var, key))` when `form` has at least one Maps control and the
/// project has a Google Maps API key configured — the secret the IDE passes
/// to the `rcrun run-form` child's environment, never to disk.
pub fn resolve_maps_api_key_secret(
    form: &Form,
    llm: &crate::llm::LlmConfig,
) -> Option<(&'static str, String)> {
    let key = llm.api_keys.get(crate::llm::GOOGLE_MAPS_API_KEY_SLOT)?;
    if key.trim().is_empty() {
        return None;
    }
    let has_maps = collect_controls(&form.controls)
        .iter()
        .any(|c| c.control_type == cobolt_forms::ControlType::Maps);
    has_maps.then(|| (GOOGLE_MAPS_API_KEY_ENV, key.clone()))
}

/// `rcrun run-form`'s env var name for a resolved Google Custom Search API
/// key (spec 039 T15/R30) — kept in sync with `cobolt-cli/src/form_gui.rs`'s
/// constant of the same name.
pub const GOOGLE_SEARCH_API_KEY_ENV: &str = "COBOLT_GOOGLE_SEARCH_API_KEY";

/// `Some((env var, key))` when `form` has at least one WebSearch control and
/// the project has a Google Custom Search API key configured. Mirrors
/// `resolve_maps_api_key_secret` above.
pub fn resolve_search_api_key_secret(
    form: &Form,
    llm: &crate::llm::LlmConfig,
) -> Option<(&'static str, String)> {
    let key = llm
        .api_keys
        .get(crate::llm::GOOGLE_CUSTOM_SEARCH_API_KEY_SLOT)?;
    if key.trim().is_empty() {
        return None;
    }
    let has_web_search = collect_controls(&form.controls)
        .iter()
        .any(|c| c.control_type == cobolt_forms::ControlType::WebSearch);
    has_web_search.then(|| (GOOGLE_SEARCH_API_KEY_ENV, key.clone()))
}

impl ExternalFormRun {
    /// Spawn `rcrun run-form <cfrm> <cbl>`. Looks for `rcrun` next to the
    /// current executable first (bundle + target/debug layouts), then in PATH.
    pub fn spawn(
        form_path: PathBuf,
        form_name: String,
        cbl_path: &std::path::Path,
        theme_default: Option<&str>,
        project_icon: Option<&std::path::Path>,
        debug: bool,
        diagnostics: &RunDiagnostics,
        fx: Option<&FormFxArgs>,
        secrets: &[(&'static str, String)],
    ) -> Result<Self, String> {
        let exe = std::env::current_exe().map_err(|e| format!("failed to get current exe: {e}"))?;
        let rcrun_path = sibling_rcrun(&exe);

        let spawn_with = |program: &std::path::Path| {
            let mut cmd = Command::new(program);
            cmd.arg("run-form").arg(&form_path).arg(cbl_path);
            if let Some(id) = theme_default {
                cmd.arg("--theme-default").arg(id);
            }
            if let Some(icon) = project_icon {
                cmd.arg("--icon").arg(icon);
            }
            // 038 — window effects, already resolved by the IDE.
            if let Some(fx) = fx {
                cmd.arg("--fx-entrance").arg(&fx.entrance);
                cmd.arg("--fx-exit").arg(&fx.exit);
                if fx.restore {
                    cmd.arg("--fx-restore");
                }
            }
            if debug {
                cmd.arg("--debug");
            }
            // Drive the child's diagnostics from the IDE's debug settings. The
            // booleans are set explicitly (including "0") so the setting is
            // authoritative over any value inherited from the IDE's own env.
            for (name, value) in &diagnostics.env {
                cmd.env(name, value);
            }
            // Credentials resolved IDE-side (e.g. the Maps API key, spec 039
            // T12/R23) — never written to the `.cfrm`/`.cbl`/project file, so
            // they only ever exist in this child's environment.
            for (name, value) in secrets {
                cmd.env(name, value);
            }
            // When ANY diagnostic is on, ask the child to write the per-control
            // diagnostics dump; pass the project name so it lands at
            // /tmp/<project>_diagnostics_dump.log.
            if let Some(project) = diagnostics.dump_project.as_deref() {
                cmd.arg("--diagnostics-dump").arg(project);
            }
            // stdin carries `@DBG` command lines in debug mode only.
            cmd.stdin(if debug { Stdio::piped() } else { Stdio::null() })
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
        };
        let mut child = spawn_with(&rcrun_path)
            .or_else(|_| spawn_with(std::path::Path::new("rcrun")))
            .map_err(|e| {
                format!(
                    "failed to spawn rcrun run-form: {e}. Make sure rcrun is next to the \
                     IDE executable or in PATH."
                )
            })?;
        let child_stdin = child.stdin.take().map(Mutex::new);

        // stdout → line channel (drained into the Output pane each frame).
        let (out_tx, stdout_rx) = mpsc::channel::<String>();
        if let Some(stdout) = child.stdout.take() {
            thread::spawn(move || {
                use std::io::BufRead;
                let reader = std::io::BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    if out_tx.send(line).is_err() {
                        break;
                    }
                }
            });
        }

        // stderr → buffered (surfaced in a modal if the process exits non-zero).
        let stderr_buf: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        if let Some(stderr) = child.stderr.take() {
            let buf = Arc::clone(&stderr_buf);
            thread::spawn(move || {
                use std::io::BufRead;
                let reader = std::io::BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if let Ok(mut b) = buf.lock() {
                        b.push(line);
                    }
                }
            });
        }

        Ok(Self {
            form_path,
            form_name,
            debug,
            child,
            child_stdin,
            stdout_rx,
            stderr_buf,
            exit_status: None,
        })
    }

    /// Send a debug command to the process as an `@DBG <json>` stdin line
    /// (debug mode only; silently ignored otherwise).
    pub fn send_debug(&self, cmd: &cobolt_runtime::RemoteDebugCmd) {
        let Some(stdin) = &self.child_stdin else {
            return;
        };
        let Ok(json) = serde_json::to_string(cmd) else {
            return;
        };
        if let Ok(mut guard) = stdin.lock() {
            let _ = writeln!(guard, "@DBG {json}");
            let _ = guard.flush();
        }
    }

    /// Drain DISPLAY / stdout lines produced since the last call.
    pub fn drain_output(&self) -> Vec<String> {
        self.stdout_rx.try_iter().collect()
    }

    /// OS process id of the `rcrun run-form` child — for the Run-Form
    /// Inspector's per-form CPU chart.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Poll the child: `true` while the process is alive. Caches the exit
    /// status once the process ends.
    pub fn is_running(&mut self) -> bool {
        if self.exit_status.is_some() {
            return false;
        }
        match self.child.try_wait() {
            Ok(Some(status)) => {
                self.exit_status = Some(status);
                false
            }
            Ok(None) => true,
            Err(_) => false,
        }
    }

    /// If the process exited with a failure, return its stderr as one message.
    pub fn take_exit_error(&mut self) -> Option<String> {
        let failed = self.exit_status.map(|s| !s.success()).unwrap_or(false);
        if !failed {
            return None;
        }
        let lines = self
            .stderr_buf
            .lock()
            .map(|b| b.clone())
            .unwrap_or_default();
        // Warnings alone don't constitute the error message; but on a failed
        // exit everything stderr said is the best diagnostic we have.
        Some(if lines.is_empty() {
            format!("Form '{}' exited with an error.", self.form_name)
        } else {
            lines.join("\n")
        })
    }

    /// Kill the external process (Stop button / window replaced by a re-run).
    pub fn stop(&mut self) {
        if self.exit_status.is_none() {
            let _ = self.child.kill();
            if let Ok(status) = self.child.wait() {
                self.exit_status = Some(status);
            }
        }
    }
}

impl Drop for ExternalFormRun {
    fn drop(&mut self) {
        // Never leave an orphaned form window running after the IDE exits.
        if self.exit_status.is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

// ── A launched BUILT binary (spec 041 T13 run path) ──────────────────────────

/// A compiled application started by Run, tracked for its whole life.
///
/// This exists because `launch_built_binary` used to `spawn()` and drop the
/// `Child` on the floor: no stderr, no exit detection, stdio inherited into the
/// IDE where nobody can see it. A built program that died at startup left the
/// IDE saying "starting the built program" with a green semaphore — the exact
/// "nothing anywhere" the operator reported, undiagnosable because the evidence
/// was discarded at the moment it was produced. Same shape as
/// [`ExternalFormRun`], minus the debug stdin.
pub struct BuiltAppRun {
    /// The form whose Run started this (tree semaphore + replace-on-re-Run).
    pub form_path: PathBuf,
    /// Display name for Output-panel lines.
    pub name: String,
    child: Child,
    /// Live stdout+stderr lines, merged (stderr prefixed) — drained per frame.
    output_rx: mpsc::Receiver<String>,
    /// stderr kept separately too, for the exit report.
    stderr_buf: Arc<Mutex<Vec<String>>>,
    exit_status: Option<std::process::ExitStatus>,
}

impl BuiltAppRun {
    /// Spawn `binary` with both pipes captured and reader threads attached.
    pub fn spawn(
        binary: &Path,
        form_path: PathBuf,
        envs: Vec<(&'static str, String)>,
    ) -> std::io::Result<Self> {
        let mut child = Command::new(binary)
            .current_dir(binary.parent().unwrap_or(Path::new(".")))
            .envs(envs)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let (tx, output_rx) = mpsc::channel::<String>();
        if let Some(stdout) = child.stdout.take() {
            let tx = tx.clone();
            thread::spawn(move || {
                use std::io::BufRead;
                for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
        }
        let stderr_buf: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        if let Some(stderr) = child.stderr.take() {
            let buf = Arc::clone(&stderr_buf);
            thread::spawn(move || {
                use std::io::BufRead;
                for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
                    if let Ok(mut b) = buf.lock() {
                        b.push(line.clone());
                    }
                    if tx.send(format!("⚠ {line}")).is_err() {
                        break;
                    }
                }
            });
        }

        let name = binary
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "built program".to_owned());
        Ok(Self {
            form_path,
            name,
            child,
            output_rx,
            stderr_buf,
            exit_status: None,
        })
    }

    /// OS process id, reported so the operator can SEE something started.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Everything the program printed since the last call.
    pub fn drain_output(&self) -> Vec<String> {
        self.output_rx.try_iter().collect()
    }

    /// `true` while the process is alive; caches the exit status once it ends.
    pub fn is_running(&mut self) -> bool {
        if self.exit_status.is_some() {
            return false;
        }
        match self.child.try_wait() {
            Ok(Some(status)) => {
                self.exit_status = Some(status);
                false
            }
            Ok(None) => true,
            Err(_) => false,
        }
    }

    /// After exit: `None` for success, `Some(report)` for a failure — the exit
    /// code plus whatever stderr said, which is the best diagnostic there is.
    pub fn exit_error(&self) -> Option<String> {
        let status = self.exit_status?;
        if status.success() {
            return None;
        }
        let code = match status.code() {
            Some(c) => c.to_string(),
            None => {
                // Name the signal — "killed by signal" alone cost a diagnostic
                // round-trip when the answer (SIGKILL: macOS code-signing kill
                // after an in-place binary overwrite) was in the number.
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    match status.signal() {
                        Some(9) => "killed by SIGKILL (9) — if this happened right \
                                    after a build, the binary was overwritten in \
                                    place (fixed in 1.60.19: rebuild)"
                            .to_owned(),
                        Some(n) => format!("killed by signal {n}"),
                        None => "killed by signal".to_owned(),
                    }
                }
                #[cfg(not(unix))]
                {
                    "killed by signal".to_owned()
                }
            }
        };
        let lines = self.stderr_buf.lock().map(|b| b.clone()).unwrap_or_default();
        Some(if lines.is_empty() {
            format!("exit {code} — the program printed nothing on stderr")
        } else {
            format!("exit {code}\n{}", lines.join("\n"))
        })
    }

    /// Kill the process (a re-Run replaces the previous instance).
    pub fn stop(&mut self) {
        if self.exit_status.is_none() {
            let _ = self.child.kill();
            if let Ok(status) = self.child.wait() {
                self.exit_status = Some(status);
            }
        }
    }
}

impl Drop for BuiltAppRun {
    fn drop(&mut self) {
        // Never leave an orphaned built application running after the IDE exits.
        if self.exit_status.is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(test)]
mod built_app_run_tests {
    use super::BuiltAppRun;
    use std::path::PathBuf;

    fn wait_exit(run: &mut BuiltAppRun) {
        for _ in 0..200 {
            if !run.is_running() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the child never exited");
    }

    /// The whole reason the struct exists: a child that fails at startup must
    /// hand back its exit code AND its stderr, not vanish.
    #[test]
    fn a_failing_child_reports_exit_code_and_stderr() {
        let mut run = BuiltAppRun::spawn(
            std::path::Path::new("/bin/sh"),
            PathBuf::from("form.cfrm"),
            vec![],
        )
        .expect("spawn sh");
        // No args — sh on stdin=null exits 0 immediately; so use a scripted one.
        run.stop();

        let dir = std::env::temp_dir().join(format!("prc-builtrun-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fails.sh");
        std::fs::write(&script, "#!/bin/sh\necho out-line\necho err-line 1>&2\nexit 3\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let mut run =
            BuiltAppRun::spawn(&script, PathBuf::from("form.cfrm"), vec![]).expect("spawn script");
        assert!(run.pid() > 0);
        wait_exit(&mut run);

        let err = run.exit_error().expect("exit 3 must be reported");
        assert!(err.contains("exit 3"), "code missing: {err}");
        assert!(err.contains("err-line"), "stderr missing: {err}");

        // Output lines were streamed too — both streams, stderr marked.
        let all: Vec<String> = run.drain_output();
        assert!(all.iter().any(|l| l == "out-line"), "{all:?}");
        assert!(all.iter().any(|l| l.contains("err-line")), "{all:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A clean exit reports nothing — silence is only correct when true.
    #[test]
    fn a_clean_exit_reports_no_error() {
        let mut run = BuiltAppRun::spawn(
            std::path::Path::new("/usr/bin/true"),
            PathBuf::from("form.cfrm"),
            vec![],
        )
        .expect("spawn true");
        wait_exit(&mut run);
        assert!(run.exit_error().is_none());
    }
}

#[cfg(test)]
mod form_codegen_roundtrip_tests {
    use super::CtrlState;
    use cobolt_forms::render::merge_props;
    use cobolt_forms::PropValue;
    use cobolt_forms::{Control, ControlType, EventBinding, Form};
    use cobolt_lexer::{tokenize, SourceFormat};
    use cobolt_parser::{parse, Severity as PSev};
    use cobolt_runtime::{FormEvent, Interpreter};
    use cobolt_semantic::analyze;
    use std::sync::mpsc;
    use std::thread;

    /// 038 T5 — fx spawn args resolve from project × form opt-out ×
    /// kill-switch, and are omitted entirely when nothing would play.
    #[test]
    fn fx_spawn_args_resolution_matrix() {
        let project = crate::project_model::CoboltProject::new("Fx", "src/main.cbl");
        // New project ⇒ matrix-rain entrance ⇒ args present.
        let args = super::resolve_fx_args(Some(&project), true, false)
            .expect("effects on, kill-switch off ⇒ Some");
        assert!(args.entrance.starts_with("matrix-rain:2000:"), "{}", args.entrance);
        assert!(args.exit.starts_with("none:"), "{}", args.exit);
        assert!(!args.restore);
        println!("fx args: entrance={} exit={} restore={}", args.entrance, args.exit, args.restore);

        // Form opted out ⇒ None; kill-switch on ⇒ None; no project ⇒ None.
        assert!(super::resolve_fx_args(Some(&project), false, false).is_none());
        assert!(super::resolve_fx_args(Some(&project), true, true).is_none());
        assert!(super::resolve_fx_args(None, true, false).is_none());

        // Both effects inactive ⇒ None even with everything enabled.
        let mut quiet = crate::project_model::CoboltProject::new("Q", "src/main.cbl");
        quiet.forms.entrance_effect.clear();
        quiet.forms.exit_effect.clear();
        assert!(super::resolve_fx_args(Some(&quiet), true, false).is_none());
        println!("fx args suppressed: opt-out, kill-switch, no-project, all-none");
    }

    #[test]
    fn generated_form_with_handler_parses_and_dispatches() {
        let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
        let mut btn = Control::new("Button-1", ControlType::Button, 10, 10);
        let mut ev = EventBinding::for_control("Button-1", "onClick");
        ev.code = "\
       ENVIRONMENT DIVISION.\n\
       DATA DIVISION.\n\
       WORKING-STORAGE SECTION.\n\
       LINKAGE SECTION.\n\n\
       PROCEDURE DIVISION.\n\
           MOVE 1 TO COBOL-QUIT."
            .into();
        btn.events.push(ev);
        form.controls.push(btn);

        let src = cobolt_codegen::generate(&form);

        assert!(
            src.contains("CALL \"BUTTON-1--ONCLICK\""),
            "missing dispatch:\n{src}"
        );
        // Spec 009 R4: handlers are generated `IS COMMON PROGRAM`.
        assert!(
            src.contains("PROGRAM-ID. BUTTON-1--ONCLICK IS COMMON PROGRAM."),
            "missing handler program"
        );
        assert!(src.contains("MOVE 1 TO COBOL-QUIT"), "missing handler body");

        let pr = parse(tokenize(&src, SourceFormat::Free));
        let perrs: Vec<_> = pr
            .diagnostics
            .iter()
            .filter(|d| d.severity == PSev::Error)
            .collect();
        assert!(
            perrs.is_empty(),
            "parse errors in generated form:\n{perrs:#?}\n--- src ---\n{src}"
        );

        let program = pr.program.expect("no program recovered");
        let sem = analyze(&program);
        let serrs: Vec<_> = sem.errors().collect();
        assert!(
            serrs.is_empty(),
            "semantic errors in generated form:\n{serrs:#?}"
        );
    }

    /// The generated IndexedFile helpers live at OUTER-program scope, so a
    /// handler — a nested program — cannot `PERFORM` them. This is the shape
    /// the Knowledge Base used to document, and it stopped compiling in 1.55.6
    /// when the analyzer began recursing into contained programs. Locked in as
    /// a test so the constraint is visible: whoever makes IndexedFile reachable
    /// from a handler (nested COMMON programs, or `::` methods) will see this
    /// fail and can retire it deliberately.
    #[test]
    fn indexedfile_paragraphs_are_not_reachable_from_a_handler() {
        let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
        let mut ixf = Control::new("IXF-1", ControlType::IndexedFile, 0, 0);
        ixf.set_prop("IndexedFile", PropValue::String("CUSTOMERS".into()));
        form.controls.push(ixf);

        let mut btn = Control::new("Button-1", ControlType::Button, 10, 10);
        let mut ev = EventBinding::for_control("Button-1", "onClick");
        ev.code = "\
       ENVIRONMENT DIVISION.\n\
       DATA DIVISION.\n\
       WORKING-STORAGE SECTION.\n\n\
       PROCEDURE DIVISION.\n\
           PERFORM IXF-1-OPEN."
            .into();
        btn.events.push(ev);
        form.controls.push(btn);

        let src = cobolt_codegen::generate(&form);
        let pr = parse(tokenize(&src, SourceFormat::Free));
        let program = pr.program.expect("no program");
        let sem = analyze(&program);
        let serrs: Vec<String> = sem.errors().map(|d| d.message.clone()).collect();
        assert!(
            serrs.iter().any(|m| m.contains("IXF-1-OPEN")
                && m.contains("not a paragraph or section of this program")),
            "expected the cross-program PERFORM to be rejected, got {serrs:#?}"
        );
    }

    #[test]
    fn runtime_uppercase_property_update_overwrites_designed_caption() {
        let mut label = Control::new("label-5", ControlType::Label, 10, 10);
        label.set_prop("Caption", PropValue::String("old".into()));

        let mut state = CtrlState::from_control(&label);
        state.set("CAPTION", "42".into());

        let merged = merge_props(&label, state.props.iter());
        assert_eq!(merged.get_prop("Caption").unwrap().as_str(), "42");
    }

    #[test]
    fn generated_form_click_event_runs_handler_in_live_runtime() {
        let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
        let mut btn = Control::new("Button-1", ControlType::Button, 10, 10);
        let mut ev = EventBinding::for_control("Button-1", "onClick");
        ev.code = "\
       ENVIRONMENT DIVISION.\n\
       DATA DIVISION.\n\
       WORKING-STORAGE SECTION.\n\
       LINKAGE SECTION.\n\n\
       PROCEDURE DIVISION.\n\
           DISPLAY \"CLICKED\"."
            .into();
        btn.events.push(ev);
        form.controls.push(btn);

        let src = cobolt_codegen::generate(&form);
        let pr = parse(tokenize(&src, SourceFormat::Free));
        let perrs: Vec<_> = pr
            .diagnostics
            .iter()
            .filter(|d| d.severity == PSev::Error)
            .collect();
        assert!(
            perrs.is_empty(),
            "parse errors in generated form:\n{perrs:#?}\n--- src ---\n{src}"
        );

        let (event_tx, event_rx) = mpsc::channel::<FormEvent>();
        let (state_tx, _state_rx) = mpsc::channel();
        let (display_tx, display_rx) = mpsc::channel();
        let program = pr.program.expect("no program recovered");
        let handle = thread::spawn(move || {
            let mut interp =
                Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
            let _ = interp.run();
        });

        event_tx.send(FormEvent::click("Button-1")).unwrap();
        event_tx.send(FormEvent::quit()).unwrap();
        handle.join().expect("interpreter thread panicked");

        let display: Vec<String> = display_rx.try_iter().collect();
        assert!(
            display.iter().any(|line| line.trim() == "CLICKED"),
            "onClick handler did not run; DISPLAY output was {display:?}"
        );
    }

    /// The handler pattern the Knowledge Base recommends, end to end: declare a
    /// paragraph, `PERFORM` it, and end the main flow with `GOBACK.` before it.
    ///
    /// Both halves of this were broken until 1.55.7. `PERFORM` resolved against
    /// the OUTER program, so a handler could not reach a paragraph it declared
    /// itself; and `GOBACK` was not a keyword, so the terminator parsed as yet
    /// another paragraph and control fell through and ran STEP again.
    #[test]
    fn handler_performs_its_own_paragraph_and_goback_stops_the_fall_through() {
        let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
        let mut btn = Control::new("Button-1", ControlType::Button, 10, 10);
        let mut ev = EventBinding::for_control("Button-1", "onClick");
        ev.code = "\
       ENVIRONMENT DIVISION.\n\
       DATA DIVISION.\n\
       WORKING-STORAGE SECTION.\n\n\
       PROCEDURE DIVISION.\n\
           PERFORM SHOW-STEP.\n\
           GOBACK.\n\
       SHOW-STEP.\n\
           DISPLAY \"STEP\"."
            .into();
        btn.events.push(ev);
        form.controls.push(btn);

        let src = cobolt_codegen::generate(&form);
        let pr = parse(tokenize(&src, SourceFormat::Free));
        let perrs: Vec<_> = pr
            .diagnostics
            .iter()
            .filter(|d| d.severity == PSev::Error)
            .collect();
        assert!(perrs.is_empty(), "parse errors:\n{perrs:#?}\n--- src ---\n{src}");

        let program = pr.program.expect("no program recovered");
        let sem = analyze(&program);
        let serrs: Vec<_> = sem.errors().collect();
        assert!(
            serrs.is_empty(),
            "a handler PERFORMing its own paragraph must analyze cleanly:\n{serrs:#?}"
        );

        let (event_tx, event_rx) = mpsc::channel::<FormEvent>();
        let (state_tx, _state_rx) = mpsc::channel();
        let (display_tx, display_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let mut interp =
                Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
            let _ = interp.run();
        });

        event_tx.send(FormEvent::click("Button-1")).unwrap();
        event_tx.send(FormEvent::quit()).unwrap();
        handle.join().expect("interpreter thread panicked");

        let steps = display_rx
            .try_iter()
            .filter(|line| line.trim() == "STEP")
            .count();
        assert_eq!(
            steps, 1,
            "SHOW-STEP must run exactly once — 0 means PERFORM could not reach \
             the handler's own paragraph, 2 means GOBACK failed to stop the \
             fall-through into it"
        );
    }
}
