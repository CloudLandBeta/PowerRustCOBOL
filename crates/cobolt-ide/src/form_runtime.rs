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
use std::path::PathBuf;
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
        let rcrun_path = exe.with_file_name("rcrun");

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
            _ => false,
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
}
