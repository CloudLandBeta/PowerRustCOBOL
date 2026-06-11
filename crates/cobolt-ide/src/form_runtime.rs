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
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
    Arc,
};
use std::thread::{self, JoinHandle};

use cobolt_forms::Form;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};
use cobolt_semantic::analyze;

// ── FormRuntime ───────────────────────────────────────────────────────────────

/// Manages one live COBOL form execution.
pub struct FormRuntime {
    /// Path to the `.cfrm` file (used to identify which form is running).
    pub form_path: PathBuf,
    /// Title shown in the running-form viewport.
    pub form_title: String,
    /// Current width/height of the form canvas.
    pub form_width:  u32,
    pub form_height: u32,
    /// Form background colour (hex RGB, e.g. "141622") and transparency (0–100).
    pub background_color: String,
    pub transparency:     u8,
    /// Optional form background image (path) + how it's scaled.
    pub background_image: String,
    pub bg_image_mode:    cobolt_forms::model::BgImageMode,
    /// Controls snapshot (id → props map), populated at launch from the form
    /// model and updated by `drain_state()` as COBOL-SET-PROPERTY arrives.
    pub ctrl_state: HashMap<String, CtrlState>,
    /// Controls in z_order (for rendering order). Populated at launch.
    pub ctrl_order: Vec<CtrlMeta>,

    /// Sends UI events to the interpreter thread.
    event_tx: Sender<FormEvent>,
    /// Receives property-change notifications from the interpreter.
    state_rx: Receiver<StateUpdate>,
    /// Receives DISPLAY output from the interpreter.
    display_rx: Receiver<String>,
    /// Set to true to request the interpreter thread to stop.
    stop_flag: Arc<AtomicBool>,
    /// Handle to the interpreter thread.
    handle: Option<JoinHandle<()>>,
    /// Tracks which ComboBox (by control ID) is currently open in the running form.
    pub combo_open: HashMap<String, bool>,
}

/// Per-control metadata needed for rendering (type + rect + initial props).
#[derive(Clone, Debug)]
pub struct CtrlMeta {
    pub id:           String,
    pub control_type: cobolt_forms::ControlType,
    pub rect:         cobolt_forms::model::Rect,
    pub z_order:      i32,
    pub animations:   Vec<cobolt_forms::model::AnimationDef>,
}

/// Mutable state of a single control as seen by the UI thread.
#[derive(Clone, Debug, Default)]
pub struct CtrlState {
    pub props:   HashMap<String, String>,
    pub visible: bool,
    pub enabled: bool,
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

    pub fn get(&self, key: &str) -> &str {
        self.props.get(key).map(|s| s.as_str()).unwrap_or("")
    }

    pub fn set(&mut self, key: &str, value: String) {
        match key {
            "Visible" => self.visible = value != "0" && value != "false",
            "Enabled" => self.enabled = value != "0" && value != "false",
            _ => {}
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

        let parse_has_errors = parse_result.diagnostics.iter()
            .any(|d| d.severity == cobolt_parser::Severity::Error);
        if parse_result.program.is_none() || parse_has_errors {
            let msgs: Vec<_> = parse_result.diagnostics.iter()
                .map(|d| format!("{}:{} {}", d.span.line, d.span.col, d.message))
                .collect();
            return Err(format!("Parse failed:\n{}", msgs.join("\n")));
        }
        let program = parse_result.program.unwrap();

        let sem = analyze(&program);
        if !sem.is_ok() {
            let msgs: Vec<_> = sem.diagnostics.iter()
                .map(|d| format!("{}:{} {}", d.span.line, d.span.col, d.message))
                .collect();
            return Err(format!("Semantic errors:\n{}", msgs.join("\n")));
        }

        // Build channel pairs.
        let (event_tx, event_rx)   = mpsc::channel::<FormEvent>();
        let (state_tx, state_rx)   = mpsc::channel::<StateUpdate>();
        let (display_tx, display_rx) = mpsc::channel::<String>();

        // Snapshot the form layout for the UI renderer.
        let ctrl_state: HashMap<String, CtrlState> = collect_controls(&form.controls)
            .into_iter()
            .map(|c| (c.id.clone(), CtrlState::from_control(c)))
            .collect();

        let mut ctrl_order: Vec<CtrlMeta> = collect_controls(&form.controls)
            .into_iter()
            .map(|c| CtrlMeta {
                id:           c.id.clone(),
                control_type: c.control_type.clone(),
                rect:         c.rect,
                z_order:      c.z_order,
                animations:   c.animations.clone(),
            })
            .collect();
        ctrl_order.sort_by_key(|m| m.z_order);

        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop_flag);

        // Spawn interpreter thread.
        let handle = thread::spawn(move || {
            let mut interp = Interpreter::new_with_channels(
                program, event_rx, state_tx, display_tx,
            );
            if stop_clone.load(Ordering::Relaxed) { return; }
            let _ = interp.run();
        });

        Ok(Self {
            form_path,
            form_title:       form.title.clone(),
            form_width:       form.width,
            form_height:      form.height,
            background_color: form.background_color.clone(),
            transparency:     form.transparency.clamp(0, 100) as u8,
            background_image: form.background_image.clone(),
            bg_image_mode:    form.bg_image_mode,
            ctrl_state,
            ctrl_order,
            event_tx,
            state_rx,
            display_rx,
            stop_flag,
            handle:     Some(handle),
            combo_open: HashMap::new(),
        })
    }

    /// Send a UI event to the interpreter.
    pub fn send_event(&self, event: FormEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Drain all pending `StateUpdate` messages and apply them to `ctrl_state`.
    /// Returns `true` if any updates were applied (UI should repaint).
    pub fn drain_state(&mut self) -> bool {
        let mut changed = false;
        loop {
            match self.state_rx.try_recv() {
                Ok(upd) => {
                    let entry = self.ctrl_state
                        .entry(upd.ctrl_id.clone())
                        .or_default();
                    entry.set(&upd.prop, upd.value);
                    changed = true;
                }
                Err(_) => break,
            }
        }
        changed
    }

    /// Drain all pending DISPLAY output lines. Caller pushes them to the
    /// IDE output panel.
    pub fn drain_display(&self) -> Vec<String> {
        let mut lines = Vec::new();
        loop {
            match self.display_rx.try_recv() {
                Ok(line) => lines.push(line),
                Err(_)   => break,
            }
        }
        lines
    }

    /// `true` while the interpreter thread is still running.
    pub fn is_running(&self) -> bool {
        self.handle.as_ref().map(|h| !h.is_finished()).unwrap_or(false)
    }

    /// Request the interpreter to stop and clean up (non-blocking).
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        // Unblock COBOL-WAIT-EVENT by sending a quit sentinel.
        let _ = self.event_tx.send(FormEvent::quit());
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for FormRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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
