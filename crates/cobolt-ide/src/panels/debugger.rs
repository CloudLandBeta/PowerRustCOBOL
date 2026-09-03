// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Debugger floating window — Phase 7+.
//!
//! Opened automatically when a debug session starts. Provides:
//!   • Debug toolbar: Stop / Continue (F5) / Step Over (F10) / Pause
//!   • Source viewer: line numbers, breakpoint gutter (●), current-line arrow (►),
//!     simple COBOL syntax colouring
//!   • Tabbed data panel: Variables (filterable), Call Stack, Breakpoints

use egui::{Color32, Context, Key, RichText, ScrollArea, TextEdit, Vec2};
use egui_extras::{Column, TableBuilder};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::i18n::Tr;
use crate::panels::empty_blocks::{folds, marker_text, FoldKind, HiddenRun};
use crate::runner::{DebugRunner, RunMsg};
use cobolt_runtime::{
    DebugAnswer, DebugEvent, DebugFrame, DebugQuery, ScopeInfo, SpecialValue, StopReason, VarInfo,
    VarSnapshot,
};

// ── Tab ───────────────────────────────────────────────────────────────────────

#[derive(PartialEq, Clone, Copy, Default)]
enum Tab {
    #[default]
    Variables,
    Watches,
    CallStack,
    Breakpoints,
}

/// The investigation dock along the bottom.
///
/// Separate tabs rather than one console, so "what did this program do to my
/// files" is a place to look rather than a grep through unrelated chatter.
#[derive(PartialEq, Clone, Copy, Default)]
enum DockTab {
    #[default]
    Console,
    Events,
    FileIo,
    Problems,
    Timeline,
}

/// One line in the investigation dock.
pub struct DockLine {
    pub channel: cobolt_runtime::OutputChannel,
    pub text: String,
    /// Milliseconds since the session started — the Timeline's ordering.
    pub at_ms: u64,
}

/// A persisted watch expression and its last answer.
pub struct Watch {
    pub expression: String,
    /// `None` until the first evaluation of this stop.
    pub value: Option<String>,
    pub error: Option<String>,
}

// ── DebugAction ───────────────────────────────────────────────────────────────

/// Action requested by the debug window in a single frame.
pub enum DebugAction {
    Stop,
    /// Ask the stopped debuggee a question. The answer arrives as
    /// `DebugEvent::Answer` and is folded back in by `apply_event`.
    Query(u64, DebugQuery),
    Continue,
    StepOver,
    StepIn,
    /// Run until the current PERFORM or CALL returns, then pause in the caller.
    StepOut,
    /// Run to a 1-based source line, then pause. Gives up if the frame it was
    /// issued from returns first.
    RunToCursor(u32),
    Pause,
    /// The developer clicked the gutter beside a line: add or remove a
    /// breakpoint there. Carries the 1-based line in the displayed source.
    ToggleBreakpoint(u32),
}

/// Height of one source line in the code pane.
///
/// The monospace glyphs are about 13.5 px tall, so this number IS the leading:
/// at 18 there were ~4.5 px between lines and the listing read double-spaced
/// (operator screenshot, 2026-09-02). At 15 the gap is ~1.4 px — 30 % of what
/// it was — which is what the operator asked for and what a code listing should
/// look like: dense enough to see a paragraph at once, still separated enough
/// to track along a line.
///
/// One constant, because the row, its gutter, the current-line highlight and a
/// blank line must agree or the pointer sits between two lines.
const CODE_LINE_H: f32 = 15.0;

/// A blank source line, at the same 30 % proportion.
const CODE_BLANK_H: f32 = 1.0;

// ── DebuggerPanel ─────────────────────────────────────────────────────────────

/// State for the floating debugger window.
pub struct DebuggerPanel {
    // Runtime state
    var_filter: String,
    vars: Vec<VarSnapshot>,
    current_para: String,
    current_line: u32,
    is_paused: bool,
    /// Why the program stopped, for the session strip. `None` while running —
    /// the strip then says Running rather than inventing a reason.
    stop_reason: Option<StopReason>,
    /// The logical COBOL call stack at the current stop, innermost first.
    frames: Vec<DebugFrame>,
    /// Which frame the inspector and watches evaluate against. Always a valid
    /// index into `frames`, or 0 when there is no stack.
    selected_frame: usize,
    pub pending_output: Vec<RunMsg>,

    // Source viewer
    source_lines: Vec<String>,
    source_path: String,
    breakpoints: HashSet<u32>,
    last_scrolled_line: u32,
    force_center_current: bool,

    // UI state
    active_tab: Tab,
    selected_var: Option<VarSnapshot>,
    /// "Only my code" — stepping crosses IDE-generated scaffolding instead of
    /// walking it. **On by default**: a form's generated `.cbl` is mostly the
    /// event loop and its plumbing, which is an internal construct the
    /// developer did not write and has no reason to single-step through.
    pub only_user_code: bool,
    animate: bool,
    animate_speed_lps: f32,
    last_animate_step: Option<Instant>,
    /// The line the developer last clicked in the source pane — the target for
    /// Run to Cursor. `None` until they pick one, which is why the button is
    /// disabled rather than guessing the current line.
    cursor_line: Option<u32>,
    // ── The data inspector (lazy) ────────────────────────────────────────
    /// Scopes of the selected frame, as answered at the current stop.
    scopes: Vec<ScopeInfo>,
    /// Rows already fetched, keyed by the handle they were fetched under.
    /// Cleared at every stop, because a handle never outlives its frame.
    rows: HashMap<i64, Vec<VarInfo>>,
    /// Handles the developer has opened.
    open: HashSet<i64>,
    /// Handles asked for but not yet answered, so one expand does not fire a
    /// query on every frame while the answer is in flight.
    inflight: HashSet<i64>,
    next_query_id: u64,
    /// Queries built while rendering, drained into `DebugAction`s afterwards —
    /// the tree is drawn inside a closure and cannot return an action itself.
    pending_queries: Vec<DebugQuery>,
    /// Queries that already carry the id their answer will be matched by — a
    /// watch or the console prompt. `pending_queries` is numbered on the way
    /// out; these cannot be, because the id is the correlation key.
    pending_ident_queries: Vec<(u64, DebugQuery)>,
    /// Watch expressions, in the order the developer added them. Persisted per
    /// project by the host.
    pub watches: Vec<Watch>,
    watch_input: String,
    /// Which watch each outstanding Evaluate belongs to, by query id. Watches
    /// are evaluated together at every stop, so several are in flight at once
    /// and the answers cannot be matched by "the only one outstanding".
    watch_pending: HashMap<u64, usize>,
    /// The console prompt's own outstanding evaluation.
    console_pending: Option<u64>,
    console_input: String,
    /// Entries typed at the prompt, newest last; ↑/↓ walk it.
    console_history: Vec<String>,
    history_pos: Option<usize>,
    dock_tab: DockTab,
    /// Height of the investigation dock, in points.
    ///
    /// An explicit stored number, not a fraction of what is available and never
    /// derived from the dock's own content — that is the feedback loop that
    /// makes a pane grow every frame until it fills the window. It changes only
    /// when the developer drags the grip.
    dock_height: f32,
    dock: Vec<DockLine>,
    session_started: Option<Instant>,
    /// Which row is being edited, and the text so far.
    editing: Option<(i64, String, String)>,
    /// The last refusal from the debuggee — a failed edit or a stale handle.
    inspect_error: Option<String>,
    /// Runs of lines the empty-block filter folds away, recomputed only when
    /// the source changes.
    hidden: Vec<HiddenRun>,
    /// Runs the developer has opened by clicking their marker, keyed by the
    /// run's first line. Nothing is ever destroyed — a fold is one click from
    /// giving the code back.
    expanded_runs: HashSet<u32>,
    /// Fold away divisions, sections and paragraphs with no executable
    /// statement. A VIEW filter only: real line numbers are preserved and
    /// stepping is untouched (operator ruling, 2026-09-02).
    hide_empty_blocks: bool,
    /// Fold the `*> <NAME>` regions codegen marks. On by default: the generated
    /// scaffolding is assumed to work, and scrolling past it to reach a handler
    /// is the developer's most common complaint about the pane.
    hide_generated: bool,
}

impl Default for DebuggerPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl DebuggerPanel {
    pub fn new() -> Self {
        Self {
            var_filter: String::new(),
            vars: Vec::new(),
            current_para: String::new(),
            current_line: 0,
            is_paused: false,
            stop_reason: None,
            frames: Vec::new(),
            selected_frame: 0,
            pending_output: Vec::new(),
            source_lines: Vec::new(),
            source_path: String::new(),
            breakpoints: HashSet::new(),
            last_scrolled_line: 0,
            force_center_current: false,
            active_tab: Tab::default(),
            selected_var: None,
            only_user_code: true,
            animate: false,
            animate_speed_lps: 4.0,
            last_animate_step: None,
            scopes: Vec::new(),
            rows: HashMap::new(),
            open: HashSet::new(),
            inflight: HashSet::new(),
            next_query_id: 1,
            pending_queries: Vec::new(),
            pending_ident_queries: Vec::new(),
            watches: Vec::new(),
            watch_input: String::new(),
            watch_pending: HashMap::new(),
            console_pending: None,
            console_input: String::new(),
            console_history: Vec::new(),
            history_pos: None,
            dock_tab: DockTab::default(),
            dock_height: 170.0,
            dock: Vec::new(),
            session_started: None,
            editing: None,
            inspect_error: None,
            cursor_line: None,
            hidden: Vec::new(),
            expanded_runs: HashSet::new(),
            hide_empty_blocks: true,
            hide_generated: true,
        }
    }

    /// Reset all runtime state (call when starting a new session).
    pub fn reset(&mut self) {
        self.vars.clear();
        self.current_para.clear();
        self.current_line = 0;
        self.is_paused = false;
        self.stop_reason = None;
        self.frames.clear();
        self.selected_frame = 0;
        self.pending_output.clear();
        self.last_scrolled_line = 0;
        self.force_center_current = false;
        self.animate = false;
        self.last_animate_step = None;
        self.cursor_line = None;
    }

    /// Supply the COBOL source text and initial breakpoint set at session start.
    pub fn set_source(&mut self, path: String, source: &str, bps: &HashSet<u32>) {
        // A new session: the dock's clock starts here, so every timestamp is
        // "since this run began" rather than since the IDE launched.
        self.session_started = Some(Instant::now());
        self.dock.clear();
        self.source_path = path;
        self.source_lines = source.lines().map(|l| l.to_owned()).collect();
        self.hidden = folds(&self.source_lines, self.hide_empty_blocks, self.hide_generated);
        self.expanded_runs.clear();
        self.breakpoints = bps.clone();
    }

    /// Sync the live breakpoint set from the editor gutter.
    pub fn set_breakpoints(&mut self, bps: &HashSet<u32>) {
        self.breakpoints = bps.clone();
    }

    /// Replace the watch list — the project's saved expressions, on open.
    pub fn set_watches(&mut self, expressions: &[String]) {
        self.watches = expressions
            .iter()
            .map(|e| Watch {
                expression: e.clone(),
                value: None,
                error: None,
            })
            .collect();
    }

    /// The watch expressions, for saving. Values are deliberately not saved:
    /// a value belongs to a stop, and restoring one next session would show a
    /// reading from a program run that has ended.
    pub fn watch_expressions(&self) -> Vec<String> {
        self.watches.iter().map(|w| w.expression.clone()).collect()
    }

    /// Has the watch list changed since `saved`? The host polls this rather than
    /// writing `cobolt.toml` on every frame.
    pub fn watches_differ_from(&self, saved: &[String]) -> bool {
        self.watches.len() != saved.len()
            || self
                .watches
                .iter()
                .zip(saved)
                .any(|(w, s)| w.expression != *s)
    }

    /// Take the queries the last frame's rendering queued.
    ///
    /// Drained by the host, which turns each into a `DebugAction` — the tree is
    /// painted inside a closure and cannot dispatch on its own.
    pub fn take_queries(&mut self) -> Vec<(u64, DebugQuery)> {
        let mut out: Vec<(u64, DebugQuery)> = self.pending_ident_queries.drain(..).collect();
        out.extend(self.pending_queries.drain(..).map(|q| {
            let id = self.next_query_id;
            self.next_query_id += 1;
            (id, q)
        }));
        out
    }

    /// The file this session is showing — the key its breakpoints are stored
    /// under in the editor. Empty before a session starts.
    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    /// Apply one interpreter event to the panel state. Shared by the in-IDE
    /// `DebugRunner` path and the remote (`rcrun run-form --debug`) path.
    pub fn apply_event(&mut self, ev: DebugEvent) {
        match ev {
            // The stop's *reason* and the logical stack. It arrives just before
            // the `Paused` snapshot below, so both are applied for one stop.
            DebugEvent::Stopped {
                line,
                paragraph,
                reason,
                frames,
                ..
            } => {
                self.is_paused = true;
                self.current_line = line;
                self.current_para = paragraph;
                self.stop_reason = Some(reason);
                self.frames = frames;
                // Every handle issued at the previous stop is now stale, so the
                // cache goes with them. What the developer had OPEN is kept:
                // re-expanding the same rows by hand at every step would make
                // single-stepping unusable.
                self.scopes.clear();
                self.rows.clear();
                self.inflight.clear();
                self.editing = None;
                self.inspect_error = None;
                self.watch_pending.clear();
                for w in &mut self.watches {
                    w.value = None;
                    w.error = None;
                }
                // A new stop is a new stack: anything the developer had
                // selected belonged to frames that may no longer exist.
                self.selected_frame = 0;
                self.force_center_current = true;
            }
            DebugEvent::Paused {
                line,
                paragraph,
                vars,
                ..
            } => {
                self.is_paused = true;
                self.current_line = line;
                self.current_para = paragraph;
                self.vars = vars;
                if let Some(selected) = self.selected_var.as_ref() {
                    self.selected_var = self
                        .vars
                        .iter()
                        .find(|v| {
                            v.name == selected.name
                                && v.scope == selected.scope
                                && v.origin == selected.origin
                        })
                        .cloned();
                }
                self.force_center_current = true;
            }
            DebugEvent::Output { text, channel } => {
                let at_ms = self
                    .session_started
                    .map(|t| t.elapsed().as_millis() as u64)
                    .unwrap_or(0);
                self.dock.push(DockLine {
                    channel,
                    text,
                    at_ms,
                });
                // Bounded: a logpoint in a tight loop would otherwise grow the
                // dock until the IDE is the thing that stops responding.
                const MAX_DOCK_LINES: usize = 5000;
                if self.dock.len() > MAX_DOCK_LINES {
                    self.dock.drain(..self.dock.len() - MAX_DOCK_LINES);
                }
            }
            DebugEvent::Answer { id, answer } => match answer {
                DebugAnswer::Scopes(scopes) => {
                    for sc in &scopes {
                        self.inflight.remove(&sc.reference);
                    }
                    self.scopes = scopes;
                }
                DebugAnswer::Variables(rows) => {
                    // The answer does not name the handle it belongs to, so the
                    // single outstanding request is matched by what is in
                    // flight. Requests are issued one at a time for exactly
                    // this reason.
                    if let Some(&r) = self.inflight.iter().next() {
                        self.inflight.remove(&r);
                        self.rows.insert(r, rows);
                    }
                }
                DebugAnswer::Set { .. } => {
                    // The written value is read back by refetching the row's
                    // parent, so the tree shows what the PROGRAM sees rather
                    // than what was typed.
                    self.editing = None;
                    self.rows.clear();
                    self.inflight.clear();
                }
                DebugAnswer::Evaluated { result, pic } => {
                    if let Some(i) = self.watch_pending.remove(&id) {
                        if let Some(w) = self.watches.get_mut(i) {
                            w.value = Some(result);
                            w.error = None;
                        }
                    } else if self.console_pending == Some(id) {
                        self.console_pending = None;
                        let shown = if pic.is_empty() {
                            result
                        } else {
                            format!("{result}    {pic}")
                        };
                        let at_ms = self
                            .session_started
                            .map(|t| t.elapsed().as_millis() as u64)
                            .unwrap_or(0);
                        self.dock.push(DockLine {
                            channel: cobolt_runtime::OutputChannel::Console,
                            text: shown,
                            at_ms,
                        });
                    }
                }
                DebugAnswer::Error(msg) => {
                    if let Some(i) = self.watch_pending.remove(&id) {
                        if let Some(w) = self.watches.get_mut(i) {
                            w.value = None;
                            w.error = Some(msg);
                        }
                    } else if self.console_pending == Some(id) {
                        self.console_pending = None;
                        let at_ms = self
                            .session_started
                            .map(|t| t.elapsed().as_millis() as u64)
                            .unwrap_or(0);
                        self.dock.push(DockLine {
                            channel: cobolt_runtime::OutputChannel::Problems,
                            text: msg,
                            at_ms,
                        });
                    } else {
                        self.inflight.clear();
                        self.inspect_error = Some(msg);
                    }
                }
            },
            DebugEvent::Resumed => {
                self.is_paused = false;
                self.stop_reason = None;
            }
            DebugEvent::Finished => {
                self.is_paused = false;
                self.stop_reason = None;
                self.scopes.clear();
                self.rows.clear();
                self.open.clear();
                self.inflight.clear();
                self.frames.clear();
                self.selected_frame = 0;
                self.vars.clear();
            }
        }
    }

    /// Process events from `DebugRunner`; returns `true` if the UI needs to repaint.
    pub fn process(&mut self, runner: &mut DebugRunner) -> bool {
        let mut dirty = false;
        for ev in runner.drain_events() {
            dirty = true;
            self.apply_event(ev);
        }
        for msg in runner.drain_run() {
            dirty = true;
            self.pending_output.push(msg);
        }
        dirty
    }

    /// Title for the standalone debugger OS window.
    pub fn window_title(&self) -> String {
        let debug_name = std::path::Path::new(&self.source_path)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("generated code");
        format!("🐞 Debugging {debug_name} generated code")
    }

    /// Render as the full content of a dedicated viewport — a standalone OS
    /// window the user can place next to the running form. The OS window is
    /// the sole size authority (the user drags its edges); content just fills
    /// it, so there is no self-inflation path.
    ///
    /// Returns a [`DebugAction`] when the user presses a control or shortcut.
    pub fn show_viewport_body(&mut self, panel_ui: &mut egui::Ui, tr: &Tr) -> Option<DebugAction> {
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        let mut action: Option<DebugAction> = None;

        // Global keyboard shortcuts — active even when the window is not focused.
        if self.is_paused {
            if ctx.input(|i| i.key_pressed(Key::F5)) {
                action = Some(DebugAction::Continue);
                self.is_paused = false;
            }
            if action.is_none() && ctx.input(|i| i.key_pressed(Key::F10)) {
                action = Some(DebugAction::StepOver);
                self.is_paused = false;
            }
            // Shift+F11 FIRST: `key_pressed` ignores extra modifiers, so a
            // plain-F11 test would also swallow the shifted chord and Step Out
            // would never fire.
            if action.is_none()
                && ctx.input(|i| i.key_pressed(Key::F11) && i.modifiers.shift)
                && self.is_paused
            {
                action = Some(DebugAction::StepOut);
                self.is_paused = false;
                self.last_animate_step = None;
            }
            if action.is_none() && ctx.input(|i| i.key_pressed(Key::F11) && !i.modifiers.shift) {
                action = Some(DebugAction::StepIn);
                self.is_paused = false;
            }
        }

        if action.is_none() {
            action = self.maybe_animate_step(ctx);
        }

        let need_scroll = self.should_center_current_line();

        egui::CentralPanel::default().show(panel_ui, |ui| {
            self.status_row(ui, tr);
            if let Some(a) = self.toolbar(ui, tr) {
                action = Some(a);
            }
            ui.separator();
            if let Some(line) = self.split_body(ui, tr, need_scroll) {
                action = Some(DebugAction::ToggleBreakpoint(line));
            }
        });

        self.variable_value_window(ctx);

        if need_scroll {
            self.last_scrolled_line = self.current_line;
            self.force_center_current = false;
        }

        action
    }

    /// Render the floating debugger window.
    ///
    /// Returns a [`DebugAction`] when the user presses a control or keyboard shortcut.
    ///
    /// # Sizing (anti self-inflation)
    ///
    /// The window frame itself is NOT resizable: the inner [`egui::Resize`] is
    /// the single size authority. Its size comes from a constant default seed
    /// plus the user's grip drag only — never from measured content — and the
    /// window auto-sizes to that box. Because no child size is derived from
    /// "remaining space" that the same subtree then fills, the window cannot
    /// grow on its own, on any egui context/viewport it is rendered in.
    #[allow(dead_code)]
    pub fn show(&mut self, ctx: &Context, tr: &Tr) -> Option<DebugAction> {
        let mut action: Option<DebugAction> = None;

        // Global keyboard shortcuts — active even when the window is not focused.
        if self.is_paused {
            if ctx.input(|i| i.key_pressed(Key::F5)) {
                action = Some(DebugAction::Continue);
                self.is_paused = false;
            }
            if action.is_none() && ctx.input(|i| i.key_pressed(Key::F10)) {
                action = Some(DebugAction::StepOver);
                self.is_paused = false;
            }
            // Shift+F11 FIRST: `key_pressed` ignores extra modifiers, so a
            // plain-F11 test would also swallow the shifted chord and Step Out
            // would never fire.
            if action.is_none()
                && ctx.input(|i| i.key_pressed(Key::F11) && i.modifiers.shift)
                && self.is_paused
            {
                action = Some(DebugAction::StepOut);
                self.is_paused = false;
                self.last_animate_step = None;
            }
            if action.is_none() && ctx.input(|i| i.key_pressed(Key::F11) && !i.modifiers.shift) {
                action = Some(DebugAction::StepIn);
                self.is_paused = false;
            }
        }

        if action.is_none() {
            action = self.maybe_animate_step(ctx);
        }

        let debug_name = std::path::Path::new(&self.source_path)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("generated code");

        let need_scroll = self.should_center_current_line();

        egui::Window::new(format!("Debugging {debug_name} generated code"))
            .id(egui::Id::new("debugger_window"))
            .resizable(false) // the inner `Resize` grip is the sole size control
            .collapsible(false)
            .show(ctx, |ui| {
                egui::Resize::default()
                    .id_salt("debugger_resize")
                    .resizable([true, true])
                    .min_size(egui::vec2(480.0, 320.0))
                    .max_size(egui::vec2(4000.0, 4000.0))
                    .default_size(egui::vec2(860.0, 460.0)) // seed only
                    .show(ui, |ui| {
                        // `sz` is the Resize box: user/default state, bounded —
                        // NOT "remaining space" of an auto-sizing container.
                        let sz = ui.available_size();
                        ui.allocate_ui(sz, |ui| {
                            // Fill the box exactly so the reported content
                            // min-size equals the box: the Resize can neither
                            // auto-grow nor auto-shrink to measured content.
                            ui.set_min_size(sz);

                            self.status_row(ui, tr);
                            if let Some(a) = self.toolbar(ui, tr) {
                                action = Some(a);
                            }
                            ui.separator();
                            if let Some(line) = self.split_body(ui, tr, need_scroll) {
                                action = Some(DebugAction::ToggleBreakpoint(line));
                            }
                        });
                    });
            });

        self.variable_value_window(ctx);

        if need_scroll {
            self.last_scrolled_line = self.current_line;
            self.force_center_current = false;
        }

        action
    }

    // ── Status row ────────────────────────────────────────────────────────────

    /// The stop reason in the developer's language.
    ///
    /// `StopReason` is a runtime type and carries an English `headline()` for
    /// logs; the UI must not show that — every user-facing string is a `Tr`
    /// field in six languages.
    fn reason_label(reason: &StopReason, tr: &Tr) -> String {
        match reason {
            StopReason::Entry => tr.dbg_reason_entry.to_owned(),
            StopReason::Breakpoint(_) => tr.dbg_reason_breakpoint.to_owned(),
            StopReason::Step => tr.dbg_reason_step.to_owned(),
            StopReason::Pause => tr.dbg_reason_pause.to_owned(),
            StopReason::DataChanged { name, .. } => {
                format!("{} · {name}", tr.dbg_reason_data_changed)
            }
            StopReason::Exception { filter, .. } => {
                format!("{} · {filter}", tr.dbg_reason_runtime_error)
            }
            StopReason::Goto => tr.dbg_reason_goto.to_owned(),
        }
    }

    fn status_row(&self, ui: &mut egui::Ui, tr: &Tr) {
        ui.horizontal(|ui| {
            // Amber for paused/current execution, green for connected/running —
            // the palette the spec fixes. Hardcoded, not read from
            // `ui.visuals()`: on a glass theme that renders dark-on-dark.
            let (text, colour) = if self.is_paused {
                let mut t = format!("● {}", tr.dbg_state_paused);
                if let Some(r) = &self.stop_reason {
                    t.push_str(" · ");
                    t.push_str(&Self::reason_label(r, tr));
                }
                (t, Color32::from_rgb(220, 180, 50))
            } else {
                (
                    format!("○ {}", tr.dbg_state_running),
                    Color32::from_rgb(80, 200, 80),
                )
            };
            ui.label(RichText::new(text).color(colour).size(12.0));

            if self.is_paused && !self.current_para.is_empty() {
                ui.label(
                    RichText::new(format!("  {}  ", self.current_para))
                        .monospace()
                        .color(Color32::from_rgb(120, 190, 255))
                        .size(12.0),
                );
                ui.label(
                    RichText::new(format!("line {}", self.current_line))
                        .color(Color32::from_gray(150))
                        .size(12.0),
                );
            }
        });
    }

    // ── Controls toolbar ──────────────────────────────────────────────────────

    fn toolbar(&mut self, ui: &mut egui::Ui, tr: &Tr) -> Option<DebugAction> {
        let mut action: Option<DebugAction> = None;
        let mut refold = false;

        ui.horizontal(|ui| {
            if ui
                .add(egui::Button::new(
                    RichText::new("■  Stop").color(Color32::from_rgb(220, 80, 80)),
                ))
                .on_hover_text(tr.dbg_stop)
                .clicked()
            {
                self.center_current_line_next_frame();
                action = Some(DebugAction::Stop);
            }

            ui.separator();

            if ui
                .add_enabled(self.is_paused, egui::Button::new("▶  Continue   F5"))
                .on_hover_text(tr.dbg_continue)
                .clicked()
            {
                action = Some(DebugAction::Continue);
                self.is_paused = false;
                self.last_animate_step = None;
            }

            if ui
                .add_enabled(self.is_paused, egui::Button::new("⤵  Step over   F10"))
                .on_hover_text(tr.dbg_step_over)
                .clicked()
            {
                action = Some(DebugAction::StepOver);
                self.is_paused = false;
                self.last_animate_step = None;
            }

            if ui
                .add_enabled(self.is_paused, egui::Button::new("↧  Step in   F11"))
                .on_hover_text(tr.dbg_step_into)
                .clicked()
            {
                action = Some(DebugAction::StepIn);
                self.is_paused = false;
                self.last_animate_step = None;
            }

            if ui
                .add_enabled(self.is_paused, egui::Button::new("↥  Step out   ⇧F11"))
                .on_hover_text(tr.dbg_step_out)
                .clicked()
            {
                action = Some(DebugAction::StepOut);
                self.is_paused = false;
                self.last_animate_step = None;
            }

            // Disabled until the developer clicks a line: Run to Cursor with no
            // cursor would have to guess a target, and guessing here means
            // running the program to somewhere they did not ask for.
            let target = self.cursor_line;
            if ui
                .add_enabled(
                    self.is_paused && target.is_some(),
                    egui::Button::new("⇥  Run to cursor"),
                )
                .on_hover_text(match target {
                    Some(l) => format!("{} — line {l}", tr.dbg_run_to_cursor),
                    None => tr.dbg_run_to_cursor.to_owned(),
                })
                .clicked()
            {
                if let Some(l) = target {
                    action = Some(DebugAction::RunToCursor(l));
                    self.is_paused = false;
                    self.last_animate_step = None;
                }
            }

            ui.separator();

            if ui
                .add_enabled(!self.is_paused, egui::Button::new("⏸  Pause"))
                .on_hover_text(tr.dbg_pause)
                .clicked()
            {
                self.center_current_line_next_frame();
                action = Some(DebugAction::Pause);
            }

            ui.separator();

            if ui
                .selectable_label(self.hide_empty_blocks, tr.dbg_hide_empty_blocks)
                .on_hover_text(
                    "Fold away divisions, sections and paragraphs that hold no \
                     executable statement. A view filter only — line numbers are \
                     unchanged and stepping is unaffected.",
                )
                .clicked()
            {
                self.hide_empty_blocks = !self.hide_empty_blocks;
                refold = true;
            }

            if ui
                .selectable_label(self.hide_generated, tr.dbg_hide_generated)
                .on_hover_text(
                    "Fold the blocks the IDE generated — the event loop and its \
                     plumbing. They are assumed to work; what is left is the code \
                     you wrote.",
                )
                .clicked()
            {
                self.hide_generated = !self.hide_generated;
                refold = true;
            }

            if ui
                .selectable_label(self.only_user_code, "Only my code")
                .on_hover_text(
                    "Step through your own handlers and procedures only. \
                     The generated event loop and the rest of the scaffolding \
                     are crossed without stopping. Breakpoints still fire \
                     wherever you set them.",
                )
                .clicked()
            {
                self.only_user_code = !self.only_user_code;
            }

            ui.separator();

            if ui
                .selectable_label(self.animate, "Animate")
                .on_hover_text("Follow execution one statement at a time")
                .clicked()
            {
                self.animate = !self.animate;
                self.last_animate_step = None;
            }
            if self.animate {
                ui.add(
                    egui::Slider::new(&mut self.animate_speed_lps, 1.0..=10.0)
                        .text("lines/s")
                        .step_by(1.0),
                );
            }
        });

        if refold {
            // Both filters feed one fold list, so a toggle recomputes it rather
            // than each filter keeping its own and the two disagreeing.
            self.hidden = folds(&self.source_lines, self.hide_empty_blocks, self.hide_generated);
        }
        action
    }

    pub fn center_current_line_next_frame(&mut self) {
        self.force_center_current = true;
        self.last_scrolled_line = 0;
    }

    fn should_center_current_line(&self) -> bool {
        self.current_line > 0
            && (self.force_center_current || self.current_line != self.last_scrolled_line)
    }

    fn maybe_animate_step(&mut self, ctx: &Context) -> Option<DebugAction> {
        if !self.animate || !self.is_paused {
            if !self.animate {
                self.last_animate_step = None;
            }
            return None;
        }

        let speed = self.animate_speed_lps.clamp(1.0, 10.0);
        let interval = Duration::from_secs_f32(1.0 / speed);
        ctx.request_repaint_after(interval);

        let now = Instant::now();
        if self
            .last_animate_step
            .map(|last| now.duration_since(last) < interval)
            .unwrap_or(false)
        {
            return None;
        }

        self.last_animate_step = Some(now);
        self.force_center_current = true;
        self.is_paused = false;
        Some(DebugAction::StepOver)
    }

    // ── Split body ────────────────────────────────────────────────────────────

    /// Two-pane split (code viewer left, variables right), with a draggable
    /// divider whose position is persisted by egui's table state.
    /// Returns the gutter line the developer clicked, if any.
    /// The file tab and the breadcrumb: `generated › switch-form.cbl ›
    /// SWITCH-FORM--ONLOAD › PROCEDURE DIVISION`.
    ///
    /// The old header was the absolute path on one line, which is the least
    /// useful thing to show: the developer knows which project they opened and
    /// cannot read a 70-character path at a glance anyway. What they need is
    /// WHERE IN THE PROGRAM the pointer is, which is the trail.
    fn file_strip(&self, ui: &mut egui::Ui) {
        let path = std::path::Path::new(&self.source_path);
        let file = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("(no source)");
        let folder = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // The tab. One file today — a session debugs one program — so it is
        // drawn as the tab it will be rather than promising a strip that does
        // not exist yet.
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            let (rect, _) = ui.allocate_exact_size(
                // Sized from the label's own width, so a long file name is not
                // clipped and a short one does not leave a wide empty tab.
                Vec2::new(file.chars().count() as f32 * 7.0 + 22.0, 22.0),
                egui::Sense::hover(),
            );
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius {
                    nw: 4,
                    ne: 4,
                    sw: 0,
                    se: 0,
                },
                Color32::from_rgb(28, 46, 58),
            );
            ui.painter().text(
                rect.left_center() + Vec2::new(9.0, 0.0),
                egui::Align2::LEFT_CENTER,
                file,
                egui::FontId::proportional(12.0),
                Color32::from_rgb(215, 225, 240),
            );
            // The amber underline is the "this is the active tab" marker, the
            // same amber the current-line pointer uses.
            ui.painter().hline(
                rect.x_range(),
                rect.max.y - 1.0,
                egui::Stroke::new(2.0, Color32::from_rgb(230, 180, 40)),
            );
        });

        // The trail. Each crumb is what the debugger actually knows: the
        // folder, the file, the program (paragraph) it stopped in, and the
        // division that paragraph lives in.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_space(4.0);
            let mut crumbs: Vec<String> = Vec::new();
            if !folder.is_empty() {
                crumbs.push(folder.to_owned());
            }
            crumbs.push(file.to_owned());
            if let Some(f) = self.frames.get(self.selected_frame) {
                if !f.program.is_empty() {
                    crumbs.push(f.program.clone());
                }
                if let Some(sec) = &f.section {
                    crumbs.push(sec.clone());
                }
                if !f.paragraph.is_empty() {
                    crumbs.push(f.paragraph.clone());
                }
            } else if !self.current_para.is_empty() {
                crumbs.push(self.current_para.clone());
            }
            let last = crumbs.len().saturating_sub(1);
            for (i, c) in crumbs.iter().enumerate() {
                if i > 0 {
                    ui.label(
                        RichText::new("›")
                            .size(11.0)
                            .color(Color32::from_gray(90)),
                    );
                }
                ui.label(
                    RichText::new(c)
                        .size(11.0)
                        // The last crumb is where you ARE; the rest are context.
                        .color(if i == last {
                            Color32::from_rgb(215, 225, 240)
                        } else {
                            Color32::from_gray(125)
                        }),
                );
            }
        });
    }

    fn split_body(&mut self, ui: &mut egui::Ui, tr: &Tr, need_scroll: bool) -> Option<u32> {
        // The dock takes its OWN stored height off the top; the split gets what
        // is left. Deriving either from the content is what makes a pane creep.
        const GRIP_H: f32 = 6.0;
        let total = ui.available_height();
        let dock_h = self.dock_height.clamp(80.0, (total - 160.0).max(80.0));
        let body_h = (total - dock_h - GRIP_H).max(160.0);
        let mut toggled: Option<u32> = None;

        TableBuilder::new(ui)
            .id_salt("dbg_split_table")
            .resizable(true)
            .vscroll(false)
            .auto_shrink([false, false])
            .cell_layout(egui::Layout::top_down(egui::Align::Min))
            .column(Column::remainder().at_least(240.0).resizable(true))
            .column(Column::initial(330.0).at_least(240.0).resizable(true))
            .body(|mut body| {
                body.row(body_h, |mut row| {
                    row.col(|ui| {
                        let pane_w = ui.available_width();
                        toggled = self.code_viewer(ui, need_scroll, pane_w, tr);
                    });
                    row.col(|ui| {
                        self.data_tabs(ui, tr);
                    });
                });
            });

        // The grip: the ONE writer of `dock_height`, and it only ever moves by
        // the drag the developer performed.
        let (grip, grip_resp) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), GRIP_H), egui::Sense::drag());
        if grip_resp.hovered() || grip_resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if grip_resp.dragged() {
            self.dock_height =
                (self.dock_height - grip_resp.drag_delta().y).clamp(80.0, (total - 160.0).max(80.0));
        }
        ui.painter().hline(
            grip.x_range(),
            grip.center().y,
            egui::Stroke::new(1.0, Color32::from_gray(70)),
        );

        ui.allocate_ui(Vec2::new(ui.available_width(), dock_h), |ui| {
            self.investigation_dock(ui, tr);
        });

        toggled
    }

    // ── Code viewer ───────────────────────────────────────────────────────────

    /// Returns the line whose gutter was clicked, if any — the caller turns it
    /// into a [`DebugAction::ToggleBreakpoint`]. The viewer takes `&self`, so it
    /// reports the click rather than editing the set itself.
    fn code_viewer(
        &mut self,
        ui: &mut egui::Ui,
        need_scroll: bool,
        pane_w: f32,
        tr: &Tr,
    ) -> Option<u32> {
        // The file tab and the breadcrumb trail, in place of the bare absolute
        // path this used to print.
        self.file_strip(ui);
        ui.add_space(2.0);

        // The gutter click collected this frame. `code_viewer` only reads the
        // panel, so the toggle travels out as a return value instead of being
        // applied here — the breakpoint set lives in the editor, and both the
        // panel and the running debuggee are synced from there.
        let mut toggled: Option<u32> = None;
        // Collected inside the closure and applied after it, so nothing borrows
        // `self` mutably while the source list is being read.
        let mut expand_run: Option<u32> = None;
        let mut picked_line: Option<u32> = None;

        ScrollArea::both()
            .id_salt("dbg_code_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let current = self.current_line;
                let bps = &self.breakpoints;

                let folding = self.hide_empty_blocks;
                let runs = &self.hidden;
                let expanded = &self.expanded_runs;

                for (idx, line_text) in self.source_lines.iter().enumerate() {
                    let line_num = (idx + 1) as u32;
                    let is_current = line_num == current;
                    let is_bp = bps.contains(&line_num);

                    // The empty-block filter. Purely visual: the line numbers
                    // below are the file's own, the breakpoint set is untouched,
                    // and the debuggee never hears about it. A folded run that
                    // holds the current statement or a breakpoint is shown
                    // anyway — hiding where the program actually stopped would
                    // be the one thing worse than the clutter.
                    if folding {
                        if let Some(run) = runs.iter().find(|r| r.contains(line_num)) {
                            let forced = run.contains(current)
                                || bps.iter().any(|b| run.contains(*b))
                                || expanded.contains(&run.start);
                            if !forced {
                                if line_num == run.start {
                                    let resp = ui.add(
                                        egui::Label::new(
                                            RichText::new(match run.kind {
                                                // A generated region says WHAT
                                                // it is; an empty run says how
                                                // many blocks it swallowed.
                                                FoldKind::Generated => format!(
                                                    "     ⌄  {}  ({} lines)",
                                                    run.label.as_deref().unwrap_or("generated"),
                                                    run.lines()
                                                ),
                                                FoldKind::Empty => format!(
                                                    "     ⌄  {}",
                                                    marker_text(tr.dbg_empty_blocks_hidden, run)
                                                ),
                                            })
                                            .monospace()
                                            .size(11.0)
                                            .color(Color32::from_gray(110)),
                                        )
                                        .sense(egui::Sense::click()),
                                    );
                                    if resp.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if resp.clicked() {
                                        expand_run = Some(run.start);
                                    }
                                }
                                continue;
                            }
                        }
                    }

                    if line_text.trim().is_empty() {
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(pane_w, CODE_BLANK_H), egui::Sense::hover());
                        if is_current {
                            ui.painter().rect_filled(
                                rect,
                                0.0,
                                Color32::from_rgba_premultiplied(70, 55, 0, 180),
                            );
                            if need_scroll {
                                ui.scroll_to_cursor(Some(egui::Align::Center));
                            }
                        }
                        continue;
                    }

                    // Amber background for the current line
                    if is_current {
                        let rect = ui.available_rect_before_wrap();
                        let bg = egui::Rect::from_min_size(
                            rect.min,
                            Vec2::new(rect.width().max(pane_w), CODE_LINE_H),
                        );
                        ui.painter().rect_filled(
                            bg,
                            0.0,
                            Color32::from_rgba_premultiplied(70, 55, 0, 180),
                        );
                    }

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;

                        // Line number
                        ui.add_sized(
                            [32.0, CODE_LINE_H],
                            egui::Label::new(
                                RichText::new(format!("{:>4}", line_num))
                                    .monospace()
                                    .size(11.0)
                                    .color(Color32::from_gray(80)),
                            ),
                        );

                        // Gutter: breakpoint dot and/or ► arrow. Clickable — a
                        // developer sets a breakpoint where they are reading the
                        // code, which is here, not in a separate editor tab.
                        let (gut_rect, gut_resp) =
                            ui.allocate_exact_size(
                                Vec2::new(18.0, CODE_LINE_H),
                                egui::Sense::click(),
                            );
                        if gut_resp.clicked() {
                            toggled = Some(line_num);
                        }
                        if gut_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if is_bp {
                            ui.painter().circle_filled(
                                gut_rect.center(),
                                4.5,
                                Color32::from_rgb(210, 50, 50),
                            );
                        } else if gut_resp.hovered() {
                            // A hollow ghost so an empty gutter shows it can be
                            // clicked at all.
                            ui.painter().circle_stroke(
                                gut_rect.center(),
                                4.5,
                                egui::Stroke::new(1.0, Color32::from_rgb(150, 70, 70)),
                            );
                        }
                        if is_current {
                            ui.painter().text(
                                gut_rect.center() + Vec2::new(2.0, 0.0),
                                egui::Align2::CENTER_CENTER,
                                "►",
                                egui::FontId::monospace(11.0),
                                Color32::from_rgb(230, 180, 40),
                            );
                        }

                        // Syntax-highlighted source line. Clickable: clicking
                        // picks the Run-to-Cursor target, which is why that
                        // button stays disabled until there is one.
                        let job = build_cobol_layout_job(line_text);
                        let resp = ui.add(egui::Label::new(job).sense(egui::Sense::click()));
                        if resp.clicked() {
                            picked_line = Some(line_num);
                        }
                    });

                    // Auto-scroll when current line changes
                    if is_current && need_scroll {
                        ui.scroll_to_cursor(Some(egui::Align::Center));
                    }
                }
            });

        if let Some(start) = expand_run {
            self.expanded_runs.insert(start);
        }
        if let Some(l) = picked_line {
            self.cursor_line = Some(l);
        }
        toggled
    }


    // ── Investigation dock ────────────────────────────────────────────────────

    /// The bottom dock: debugger output, split by channel, plus a prompt.
    ///
    /// Returns any query the prompt produced. The prompt is the same evaluator
    /// the watches use, so anything that works in one works in the other.
    fn investigation_dock(&mut self, ui: &mut egui::Ui, tr: &Tr) {
        ui.horizontal(|ui| {
            let counts = |c: cobolt_runtime::OutputChannel| {
                self.dock.iter().filter(|l| l.channel == c).count()
            };
            use cobolt_runtime::OutputChannel as Ch;
            for (tab, label, ch) in [
                (DockTab::Console, tr.dbg_console, Ch::Console),
                (DockTab::Events, tr.dbg_events, Ch::Events),
                (DockTab::FileIo, tr.dbg_file_io, Ch::FileIo),
                (DockTab::Problems, tr.dbg_problems, Ch::Problems),
                (DockTab::Timeline, tr.dbg_timeline, Ch::Timeline),
            ] {
                let n = if tab == DockTab::Timeline {
                    self.dock.len()
                } else {
                    counts(ch)
                };
                let text = if n > 0 {
                    format!("{label}  {n}")
                } else {
                    label.to_owned()
                };
                ui.selectable_value(&mut self.dock_tab, tab, text);
            }
            if ui
                .add(egui::Label::new(RichText::new("🗑").size(12.0)).sense(egui::Sense::click()))
                .on_hover_text("Clear")
                .clicked()
            {
                self.dock.clear();
            }
        });
        ui.separator();

        use cobolt_runtime::OutputChannel as Ch;
        let want = match self.dock_tab {
            DockTab::Console => Some(Ch::Console),
            DockTab::Events => Some(Ch::Events),
            DockTab::FileIo => Some(Ch::FileIo),
            DockTab::Problems => Some(Ch::Problems),
            // The Timeline is every channel in the order it happened — that is
            // what makes it a timeline rather than a sixth console.
            DockTab::Timeline => None,
        };

        let rows = ui.available_height() - 26.0;
        ScrollArea::vertical()
            .id_salt("dbg_dock_scroll")
            .max_height(rows.max(40.0))
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                let mut any = false;
                for line in self.dock.iter().filter(|l| want.is_none_or(|c| l.channel == c)) {
                    any = true;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.label(
                            RichText::new(format!("{:>7}.{:03}", line.at_ms / 1000, line.at_ms % 1000))
                                .monospace()
                                .size(10.0)
                                .color(Color32::from_gray(100)),
                        );
                        if want.is_none() {
                            ui.label(
                                RichText::new(match line.channel {
                                    Ch::Console => "con",
                                    Ch::Events => "evt",
                                    Ch::FileIo => "i/o",
                                    Ch::Problems => "prb",
                                    Ch::Timeline => "tml",
                                })
                                .monospace()
                                .size(10.0)
                                .color(Color32::from_gray(120)),
                            );
                        }
                        ui.label(
                            RichText::new(&line.text)
                                .monospace()
                                .size(11.0)
                                .color(if line.channel == Ch::Problems {
                                    Color32::from_rgb(230, 140, 140)
                                } else {
                                    Color32::from_rgb(210, 216, 232)
                                }),
                        );
                    });
                }
                if !any {
                    ui.label(
                        RichText::new(tr.dbg_dock_empty)
                            .size(11.0)
                            .color(Color32::from_gray(110)),
                    );
                }
            });

        // The prompt. Only on the console, and only while stopped: evaluating
        // against a running program would answer about a moment that has passed.
        if self.dock_tab == DockTab::Console {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(">")
                        .monospace()
                        .color(Color32::from_rgb(120, 190, 255)),
                );
                let resp = ui.add_enabled(
                    self.is_paused,
                    TextEdit::singleline(&mut self.console_input)
                        .hint_text(tr.dbg_console_hint)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
                );
                if resp.has_focus() {
                    // ↑/↓ walk the history, as every console does.
                    let (up, down) = ui.input(|i| {
                        (
                            i.key_pressed(egui::Key::ArrowUp),
                            i.key_pressed(egui::Key::ArrowDown),
                        )
                    });
                    if up && !self.console_history.is_empty() {
                        let pos = match self.history_pos {
                            None => self.console_history.len() - 1,
                            Some(0) => 0,
                            Some(p) => p - 1,
                        };
                        self.history_pos = Some(pos);
                        self.console_input = self.console_history[pos].clone();
                    } else if down {
                        match self.history_pos {
                            Some(p) if p + 1 < self.console_history.len() => {
                                self.history_pos = Some(p + 1);
                                self.console_input = self.console_history[p + 1].clone();
                            }
                            Some(_) => {
                                self.history_pos = None;
                                self.console_input.clear();
                            }
                            None => {}
                        }
                    }
                }
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let expression = self.console_input.trim().to_owned();
                    if !expression.is_empty() {
                        let at_ms = self
                            .session_started
                            .map(|t| t.elapsed().as_millis() as u64)
                            .unwrap_or(0);
                        self.dock.push(DockLine {
                            channel: cobolt_runtime::OutputChannel::Console,
                            text: format!("> {expression}"),
                            at_ms,
                        });
                        let id = self.next_query_id;
                        self.next_query_id += 1;
                        self.console_pending = Some(id);
                        let frame = self.selected_frame;
                        self.pending_ident_queries
                            .push((id, DebugQuery::Evaluate { frame, expression: expression.clone() }));
                        self.console_history.push(expression);
                        self.history_pos = None;
                        self.console_input.clear();
                    }
                }
            });
        }
    }

    // ── Tabbed data panel ─────────────────────────────────────────────────────

    fn data_tabs(&mut self, ui: &mut egui::Ui, tr: &Tr) {
        // Collected in the closure, applied after: every one of these mutates
        // state the tree is being read from.
        let mut pick_frame: Option<usize> = None;
        let mut toggle: Option<i64> = None;
        let mut fetch: Option<i64> = None;
        let mut want_scopes = false;
        let mut begin_edit: Option<(i64, String, String)> = None;
        let mut edit_buf: Option<String> = None;
        let mut commit: Option<(i64, String, String)> = None;
        let mut add_watch: Option<String> = None;
        let mut drop_watch: Option<usize> = None;
        let mut evaluate_watches: Vec<(usize, String)> = Vec::new();
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.active_tab, Tab::Variables, tr.dbg_variables);
            ui.selectable_value(&mut self.active_tab, Tab::Watches, tr.dbg_watches);
            ui.selectable_value(&mut self.active_tab, Tab::CallStack, tr.dbg_call_stack);
            ui.selectable_value(&mut self.active_tab, Tab::Breakpoints, tr.dbg_breakpoints);
        });
        ui.separator();

        match self.active_tab {
            Tab::Variables => {
                ui.add(
                    TextEdit::singleline(&mut self.var_filter)
                        .hint_text(tr.dbg_filter_hint)
                        .desired_width(f32::INFINITY),
                );
                if let Some(err) = &self.inspect_error {
                    ui.label(
                        RichText::new(format!("⚠ {err}"))
                            .color(Color32::from_rgb(230, 120, 120))
                            .size(11.0),
                    );
                }
                ui.add_space(2.0);

                if !self.is_paused {
                    ui.label(
                        RichText::new(tr.dbg_state_running)
                            .color(Color32::from_gray(110))
                            .size(11.0),
                    );
                    return;
                }
                if self.scopes.is_empty() {
                    want_scopes = true;
                }

                // The visible tree, flattened to rows first. A table needs a
                // row COUNT before it draws, and flattening is also what lets
                // it virtualise: only the rows on screen are built, so a
                // WORKING-STORAGE of thousands of items costs what fits.
                let filter = self.var_filter.to_ascii_lowercase();
                let rows = self.flatten_visible(&filter);

                TableBuilder::new(ui)
                    .id_salt("dbg_inspect_table")
                    .striped(true)
                    // Draggable dividers, as the mockup asks. `allocate_ui`
                    // reserves width but does not make a child FILL it, so the
                    // hand-laid version collapsed every cell onto the next and
                    // read `COBOL-CONTROL-IDPIC X(64)SPACES`.
                    .resizable(true)
                    .column(Column::initial(190.0).at_least(90.0).resizable(true))
                    .column(Column::initial(110.0).at_least(60.0).resizable(true))
                    .column(Column::remainder().at_least(60.0))
                    .header(18.0, |mut header| {
                        for label in [tr.dbg_col_name, tr.dbg_col_pic, tr.dbg_col_value] {
                            header.col(|ui| {
                                ui.label(
                                    RichText::new(label)
                                        .size(11.0)
                                        .color(Color32::from_gray(150)),
                                );
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(18.0, rows.len(), |mut row| {
                            let r = &rows[row.index()];
                            row.col(|ui| {
                                let marker = if !r.expandable {
                                    "  "
                                } else if r.open {
                                    "⌄"
                                } else {
                                    "›"
                                };
                                let resp = ui.add(
                                    egui::Label::new(
                                        RichText::new(format!(
                                            "{}{marker} {}",
                                            "   ".repeat(r.depth),
                                            r.name
                                        ))
                                        .monospace()
                                        .size(11.0)
                                        .color(r.name_colour),
                                    )
                                    .sense(egui::Sense::click()),
                                );
                                if r.expandable {
                                    if resp.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if resp.clicked() {
                                        toggle = Some(r.reference);
                                    }
                                }
                            });
                            row.col(|ui| {
                                ui.label(
                                    RichText::new(&r.type_text)
                                        .monospace()
                                        .size(10.0)
                                        .color(Color32::from_gray(140)),
                                );
                            });
                            row.col(|ui| {
                                let editing_this = matches!(
                                    &self.editing,
                                    Some((pr, n, _)) if *pr == r.parent && *n == r.name
                                );
                                if editing_this {
                                    if let Some((_, _, buf)) = self.editing.as_ref() {
                                        let mut text = buf.clone();
                                        let resp = ui.add(
                                            TextEdit::singleline(&mut text)
                                                .desired_width(f32::INFINITY)
                                                .font(egui::TextStyle::Monospace),
                                        );
                                        if text != *buf {
                                            edit_buf = Some(text.clone());
                                        }
                                        if resp.lost_focus()
                                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                        {
                                            commit = Some((r.parent, r.name.clone(), text));
                                        }
                                    }
                                } else {
                                    let mut rt = RichText::new(&r.value_text)
                                        .monospace()
                                        .size(11.0)
                                        .color(r.value_colour);
                                    if r.value_italic {
                                        rt = rt.italics();
                                    }
                                    let v =
                                        ui.add(egui::Label::new(rt).sense(egui::Sense::click()));
                                    if r.editable {
                                        if v.hovered() {
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                                        }
                                        if v.clicked() {
                                            begin_edit = Some((
                                                r.parent,
                                                r.name.clone(),
                                                r.raw_value.clone(),
                                            ));
                                        }
                                    }
                                }
                            });
                        });
                    });
            }

            Tab::Watches => {
                ui.horizontal(|ui| {
                    let resp = ui.add(
                        TextEdit::singleline(&mut self.watch_input)
                            .hint_text(tr.dbg_watch_hint)
                            .desired_width(f32::INFINITY),
                    );
                    let entered =
                        resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if entered && !self.watch_input.trim().is_empty() {
                        add_watch = Some(self.watch_input.trim().to_owned());
                    }
                });
                ui.add_space(2.0);
                ScrollArea::vertical()
                    .id_salt("dbg_watch_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.watches.is_empty() {
                            ui.label(
                                RichText::new(tr.dbg_watch_empty)
                                    .color(Color32::from_gray(110))
                                    .size(11.0),
                            );
                        }
                        for (i, w) in self.watches.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                if ui
                                    .add(
                                        egui::Label::new(
                                            RichText::new("✕")
                                                .size(10.0)
                                                .color(Color32::from_gray(120)),
                                        )
                                        .sense(egui::Sense::click()),
                                    )
                                    .clicked()
                                {
                                    drop_watch = Some(i);
                                }
                                ui.label(
                                    RichText::new(&w.expression)
                                        .monospace()
                                        .size(11.0)
                                        .color(Color32::from_rgb(215, 220, 235)),
                                );
                                match (&w.value, &w.error) {
                                    (_, Some(e)) => {
                                        ui.label(
                                            RichText::new(e)
                                                .size(11.0)
                                                .italics()
                                                .color(Color32::from_rgb(230, 120, 120)),
                                        );
                                    }
                                    (Some(v), _) => {
                                        ui.label(
                                            RichText::new(v.trim())
                                                .monospace()
                                                .size(11.0)
                                                .color(Color32::from_rgb(240, 200, 120)),
                                        );
                                    }
                                    // Not evaluated yet at this stop. Say so
                                    // rather than show the PREVIOUS stop's
                                    // value, which would be a stale reading
                                    // presented as a current one.
                                    (None, None) => {
                                        ui.label(
                                            RichText::new(if self.is_paused { "…" } else { "—" })
                                                .size(11.0)
                                                .color(Color32::from_gray(110)),
                                        );
                                    }
                                }
                            });
                        }
                    });
                if self.is_paused {
                    for (i, w) in self.watches.iter().enumerate() {
                        if w.value.is_none() && w.error.is_none() {
                            evaluate_watches.push((i, w.expression.clone()));
                        }
                    }
                }
            }

            Tab::CallStack => {
                ScrollArea::vertical()
                    .id_salt("dbg_stack_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.frames.is_empty() {
                            ui.label(
                                RichText::new(tr.dbg_no_frame).color(Color32::from_gray(100)),
                            );
                            return;
                        }
                        // Innermost first, as the interpreter sends it. Inline
                        // PERFORM loops are already filtered out upstream: they
                        // carry step depth, not a call.
                        let selected = self.selected_frame;
                        for (i, f) in self.frames.iter().enumerate() {
                            let is_top = i == selected;
                            let resp = ui.add(
                                egui::Label::new(
                                    RichText::new(format!(
                                        "{} {}",
                                        if is_top { "►" } else { " " },
                                        f.display_name()
                                    ))
                                    .monospace()
                                    .size(12.0)
                                    .color(if is_top {
                                        Color32::from_rgb(230, 180, 40)
                                    } else if f.generated {
                                        // Generated scaffolding is greyed, not
                                        // hidden: the developer can still see
                                        // the path their call actually took.
                                        Color32::from_gray(110)
                                    } else {
                                        Color32::from_rgb(120, 190, 255)
                                    }),
                                )
                                .sense(egui::Sense::click()),
                            );
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if resp.clicked() {
                                pick_frame = Some(i);
                            }
                            if f.line > 0 {
                                ui.label(
                                    RichText::new(format!(
                                        "     {:?}  line {}",
                                        f.kind, f.line
                                    ))
                                    .monospace()
                                    .size(10.0)
                                    .color(Color32::from_gray(110)),
                                );
                            }
                        }
                    });
            }

            Tab::Breakpoints => {
                ScrollArea::vertical()
                    .id_salt("dbg_bp_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.breakpoints.is_empty() {
                            ui.label(
                                RichText::new("No breakpoints set").color(Color32::from_gray(100)),
                            );
                        } else {
                            let mut sorted: Vec<u32> = self.breakpoints.iter().cloned().collect();
                            sorted.sort_unstable();
                            egui::Grid::new("dbg_bp_grid")
                                .num_columns(2)
                                .striped(true)
                                .show(ui, |ui| {
                                    for line in &sorted {
                                        ui.label(
                                            RichText::new("●")
                                                .color(Color32::from_rgb(210, 50, 50)),
                                        );
                                        ui.label(
                                            RichText::new(format!("line {line}"))
                                                .monospace()
                                                .color(Color32::from_rgb(100, 180, 255)),
                                        );
                                        ui.end_row();
                                    }
                                });
                        }
                    });
            }
        }

        if let Some(i) = pick_frame {
            // Changing the selected frame changes what the inspector,
            // watches and evaluations resolve against, so its answers go too.
            self.selected_frame = i;
            self.scopes.clear();
            self.rows.clear();
            self.inflight.clear();
        }
        if let Some(r) = toggle {
            if self.open.contains(&r) {
                self.open.remove(&r);
            } else {
                self.open.insert(r);
                // Opening a row it has never seen is what triggers the fetch —
                // this is the whole "lazy" in lazy expansion.
                if !self.rows.contains_key(&r) {
                    fetch = Some(r);
                }
            }
        }
        if let Some((r, name, seed)) = begin_edit {
            self.editing = Some((r, name, seed));
        }
        if let (Some(text), Some(cur)) = (edit_buf, self.editing.as_mut()) {
            cur.2 = text;
        }
        if let Some((reference, name, value)) = commit {
            self.pending_queries.push(DebugQuery::SetVariable {
                reference,
                name,
                value,
            });
        }
        if want_scopes && !self.inflight.contains(&0) {
            self.inflight.insert(0);
            let frame = self.selected_frame;
            self.pending_queries.push(DebugQuery::Scopes { frame });
        }
        if let Some(reference) = fetch {
            if self.inflight.insert(reference) {
                self.pending_queries.push(DebugQuery::Variables { reference });
            }
        }
        if let Some(expr) = add_watch {
            self.watches.push(Watch {
                expression: expr,
                value: None,
                error: None,
            });
            self.watch_input.clear();
        }
        if let Some(i) = drop_watch {
            if i < self.watches.len() {
                self.watches.remove(i);
            }
        }
        for (i, expression) in evaluate_watches {
            // One query per watch, tracked by id: several are in flight at once
            // at every stop, so "the only outstanding one" cannot match them.
            if self.watch_pending.values().any(|p| *p == i) {
                continue;
            }
            let id = self.next_query_id;
            self.next_query_id += 1;
            self.watch_pending.insert(id, i);
            let frame = self.selected_frame;
            self.pending_ident_queries.push((id, DebugQuery::Evaluate { frame, expression }));
        }
    }

    pub fn is_paused(&self) -> bool {
        self.is_paused
    }

    pub fn current_line(&self) -> u32 {
        self.current_line
    }

    fn is_generated_control_handler_var(var: &VarSnapshot) -> bool {
        let name = var.name.to_ascii_uppercase();
        name.starts_with("COBOL-")
            || name == "FORM-NAME"
            || name.starts_with("WS-ANIM-")
            || (name.starts_with("WS-") && name.contains("-SELECTED-"))
    }

    fn variable_value_window(&mut self, ctx: &Context) {
        let Some(var) = self.selected_var.clone() else {
            return;
        };

        let mut open = true;
        egui::Window::new(format!("Data item ({})", var.name))
            .id(egui::Id::new("debugger_data_item_value"))
            .collapsible(false)
            .resizable(true)
            .default_size(egui::vec2(760.0, 460.0))
            .min_size(egui::vec2(460.0, 300.0))
            .open(&mut open)
            .show(ctx, |ui| {
                egui::Grid::new("debugger_data_item_details")
                    .num_columns(2)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        ui.strong("PIC:");
                        ui.label(if var.pic.is_empty() { "-" } else { &var.pic });
                        ui.end_row();
                        ui.strong("Scope:");
                        ui.label(&var.scope);
                        ui.end_row();
                        ui.strong("Origin:");
                        ui.label(&var.origin);
                        ui.end_row();
                    });

                ui.separator();

                let body_h = ui.available_height().max(160.0);
                TableBuilder::new(ui)
                    .id_salt("debugger_data_item_value_split")
                    .resizable(true)
                    .vscroll(false)
                    .auto_shrink([false, false])
                    .cell_layout(egui::Layout::top_down(egui::Align::Min))
                    .column(Column::remainder().at_least(180.0).resizable(true))
                    .column(Column::remainder().at_least(180.0).resizable(true))
                    .header(22.0, |mut header| {
                        header.col(|ui| {
                            ui.strong("Value");
                        });
                        header.col(|ui| {
                            ui.strong("Hex representation");
                        });
                    })
                    .body(|mut body| {
                        body.row(body_h, |mut row| {
                            row.col(|ui| {
                                ScrollArea::vertical()
                                    .id_salt("debugger_data_item_value_scroll")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.add(
                                            egui::Label::new(RichText::new(&var.value).monospace())
                                                .wrap(),
                                        );
                                    });
                            });
                            row.col(|ui| {
                                ScrollArea::vertical()
                                    .id_salt("debugger_data_item_hex_scroll")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(Self::hex_dump(&var.value)).monospace(),
                                        );
                                    });
                            });
                        });
                    });
            });

        if !open {
            self.selected_var = None;
        }
    }

    fn fit_value_preview(ui: &egui::Ui, value: &str, max_px: f32) -> String {
        let font_id = egui::FontId::monospace(11.0);
        let text_width = |text: &str| {
            ui.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap(text.to_owned(), font_id.clone(), Color32::WHITE)
                    .size()
                    .x
            })
        };

        if text_width(value) <= max_px {
            return value.to_owned();
        }

        let mut out = String::new();
        for ch in value.chars() {
            let candidate = format!("{out}{ch}...");
            if text_width(&candidate) > max_px {
                break;
            }
            out.push(ch);
        }
        if out.is_empty() {
            "...".to_owned()
        } else {
            format!("{out}...")
        }
    }

    fn hex_dump(value: &str) -> String {
        value
            .as_bytes()
            .chunks(16)
            .map(|chunk| {
                chunk
                    .iter()
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ── COBOL syntax highlighter ──────────────────────────────────────────────────

const COBOL_KEYWORDS: &[&str] = &[
    "ACCEPT",
    "ADD",
    "ALL",
    "AND",
    "BINARY",
    "BY",
    "CALL",
    "CLOSE",
    "COMP",
    "COMP-3",
    "COMP-4",
    "COMPUTE",
    "CONFIGURATION",
    "DATA",
    "DELETE",
    "DEPENDING",
    "DISPLAY",
    "DIVIDE",
    "END-CALL",
    "END-COMPUTE",
    "END-EVALUATE",
    "END-IF",
    "END-PERFORM",
    "END-READ",
    "END-WRITE",
    "ENVIRONMENT",
    "EQUAL",
    "ERROR",
    "EVALUATE",
    "EXIT",
    "EXTEND",
    "FROM",
    "GIVING",
    "GLOBAL",
    "GOBACK",
    "GREATER",
    "HIGH-VALUE",
    "HIGH-VALUES",
    "I-O",
    "IDENTIFICATION",
    "IF",
    "INITIALIZE",
    "INPUT",
    "INPUT-OUTPUT",
    "INSPECT",
    "IS",
    "LESS",
    "LINKAGE",
    "LOW-VALUE",
    "LOW-VALUES",
    "MOVE",
    "MULTIPLY",
    "NOT",
    "OCCURS",
    "OF",
    "ON",
    "OPEN",
    "OR",
    "OTHER",
    "OUTPUT",
    "OVERFLOW",
    "PACKED-DECIMAL",
    "PERFORM",
    "PIC",
    "PICTURE",
    "PROCEDURE",
    "PROGRAM",
    "PROGRAM-ID",
    "READ",
    "REDEFINES",
    "REMAINDER",
    "REWRITE",
    "ROUNDED",
    "RUN",
    "SECTION",
    "SET",
    "SIZE",
    "SPACE",
    "SPACES",
    "START",
    "STOP",
    "STRING",
    "SUBTRACT",
    "THAN",
    "THEN",
    "TIMES",
    "TO",
    "UNSTRING",
    "USING",
    "VALUE",
    "VALUES",
    "WHEN",
    "WORKING-STORAGE",
    "WRITE",
    "ZERO",
    "ZEROES",
    "ZEROS",
    "COMMON",
    "DIVISION",
    "ELSE",
];

fn is_cobol_keyword(word: &str) -> bool {
    let upper = word.to_ascii_uppercase();
    COBOL_KEYWORDS.iter().any(|&kw| kw == upper.as_str())
}

fn build_cobol_layout_job(line: &str) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    // Disable wrapping — code lines extend to the right.
    job.wrap.max_width = f32::INFINITY;

    let mono = egui::FontId::monospace(12.0);
    let col_kw = Color32::from_rgb(86, 156, 214);
    let col_str = Color32::from_rgb(206, 145, 120);
    let col_cmt = Color32::from_rgb(87, 166, 74);
    let col_num = Color32::from_rgb(181, 206, 168);
    let col_def = Color32::from_gray(210);

    let trimmed = line.trim_start();
    if trimmed.starts_with("*>") {
        job.append(
            line,
            0.0,
            egui::text::TextFormat {
                font_id: mono,
                color: col_cmt,
                ..Default::default()
            },
        );
        return job;
    }

    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    macro_rules! fmt {
        ($color:expr) => {
            egui::text::TextFormat {
                font_id: mono.clone(),
                color: $color,
                ..Default::default()
            }
        };
    }

    while i < len {
        if chars[i] == '"' {
            let start = i;
            i += 1;
            while i < len && chars[i] != '"' {
                i += 1;
            }
            if i < len {
                i += 1;
            }
            job.append(
                &chars[start..i].iter().collect::<String>(),
                0.0,
                fmt!(col_str),
            );
        } else if chars[i].is_alphabetic() {
            let start = i;
            i += 1;
            while i < len {
                if chars[i].is_alphanumeric() {
                    i += 1;
                } else if chars[i] == '-' && i + 1 < len && chars[i + 1].is_alphanumeric() {
                    i += 1;
                } else {
                    break;
                }
            }
            let word: String = chars[start..i].iter().collect();
            let color = if is_cobol_keyword(&word) {
                col_kw
            } else {
                col_def
            };
            job.append(&word, 0.0, fmt!(color));
        } else if chars[i].is_ascii_digit()
            || (chars[i] == '-' && i + 1 < len && chars[i + 1].is_ascii_digit())
        {
            let start = i;
            if chars[i] == '-' {
                i += 1; // sign of a negative literal, e.g. BY -1
            }
            while i < len && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            job.append(
                &chars[start..i].iter().collect::<String>(),
                0.0,
                fmt!(col_num),
            );
        } else {
            let start = i;
            while i < len
                && !chars[i].is_alphabetic()
                && chars[i] != '"'
                && !chars[i].is_ascii_digit()
            {
                if chars[i] == '-' && i + 1 < len && chars[i + 1].is_alphanumeric() {
                    break;
                }
                i += 1;
            }
            // Guarantee forward progress: if the loop broke on its first char
            // (e.g. a `-` right before an alphanumeric), consume that char —
            // otherwise the outer loop would never advance and the UI would
            // spin forever appending empty sections.
            if i == start {
                i += 1;
            }
            job.append(
                &chars[start..i].iter().collect::<String>(),
                0.0,
                fmt!(col_def),
            );
        }
    }

    job
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: a `-` directly followed by an alphanumeric (negative literal
    /// like `BY -1`) used to make the tokenizer loop forever with zero progress,
    /// freezing the whole IDE the moment the debugger rendered that line.
    #[test]
    fn layout_job_terminates_on_negative_literal() {
        let job = build_cobol_layout_job(
            "PERFORM VARYING I FROM FUNCTION LENGTH(WS-TXT) BY -1 UNTIL I < 1",
        );
        assert!(!job.text.is_empty());
    }

    #[test]
    fn layout_job_highlights_negative_number() {
        let job = build_cobol_layout_job("MOVE -12.5 TO WS-X");
        assert_eq!(job.text, "MOVE -12.5 TO WS-X");
    }

    #[test]
    fn layout_job_terminates_on_dash_before_letter() {
        let job = build_cobol_layout_job("COMPUTE X = Y -Z");
        assert_eq!(job.text, "COMPUTE X = Y -Z");
    }

    /// The window knows which file its breakpoints belong to.
    ///
    /// The debugger shows the **generated** `.cbl`, which is rarely the tab in
    /// front — so a gutter click has to be recorded against this path, not the
    /// active editor tab. `set_breakpoints` also has to actually take, because
    /// it had no callers at all and the panel's set went stale the moment a
    /// breakpoint moved.
    #[test]
    fn the_window_reports_the_file_its_breakpoints_belong_to() {
        let mut p = DebuggerPanel::new();
        assert_eq!(p.source_path(), "", "no session yet");

        let mut bps = HashSet::new();
        bps.insert(7u32);
        p.set_source("/proj/generated/form1.cbl".into(), "A\nB\nC\n", &bps);
        assert_eq!(p.source_path(), "/proj/generated/form1.cbl");
        assert!(p.breakpoints.contains(&7));

        // A later sync replaces the set wholesale — add and remove both land.
        let mut moved = HashSet::new();
        moved.insert(12u32);
        p.set_breakpoints(&moved);
        assert!(p.breakpoints.contains(&12));
        assert!(!p.breakpoints.contains(&7), "the old line is gone");

        // Reset keeps the source and its breakpoints: it clears the RUN, not
        // the developer's marks.
        p.reset();
        assert_eq!(p.source_path(), "/proj/generated/form1.cbl");
        assert!(p.breakpoints.contains(&12));
    }
}

/// One line of the flattened inspector tree, ready for a table row.
///
/// Built fresh each frame from the scopes, the fetched rows and which handles
/// are open. Flattening first is what lets the table virtualise — it needs a
/// row COUNT before it draws — and it keeps every column's content decided in
/// one place rather than inside three separate cell closures.
struct FlatRow {
    depth: usize,
    name: String,
    /// The handle this row expands to, 0 when it does not expand.
    reference: i64,
    /// The handle this row was LISTED under — what an edit is addressed by.
    parent: i64,
    expandable: bool,
    open: bool,
    editable: bool,
    type_text: String,
    value_text: String,
    raw_value: String,
    value_italic: bool,
    name_colour: Color32,
    value_colour: Color32,
}

impl DebuggerPanel {
    /// Flatten the open parts of the tree, applying the filter.
    ///
    /// A scope always shows: hiding an empty WORKING-STORAGE because its rows
    /// have not arrived yet would make the pane look broken while it loads.
    fn flatten_visible(&self, filter: &str) -> Vec<FlatRow> {
        let mut out = Vec::new();
        for sc in &self.scopes {
            let open = self.open.contains(&sc.reference);
            out.push(FlatRow {
                depth: 0,
                name: sc.name.clone(),
                reference: sc.reference,
                parent: 0,
                expandable: true,
                open,
                editable: false,
                type_text: format!("({})", sc.count),
                value_text: String::new(),
                raw_value: String::new(),
                value_italic: false,
                name_colour: Color32::from_rgb(150, 175, 215),
                value_colour: Color32::from_gray(140),
            });
            if open {
                self.flatten_children(sc.reference, 1, filter, &mut out);
            }
        }
        out
    }

    fn flatten_children(&self, reference: i64, depth: usize, filter: &str, out: &mut Vec<FlatRow>) {
        let Some(rows) = self.rows.get(&reference) else {
            // Asked for, not yet answered. Say so rather than render nothing,
            // which reads as "this group is empty".
            out.push(FlatRow {
                depth,
                name: "…".into(),
                reference: 0,
                parent: reference,
                expandable: false,
                open: false,
                editable: false,
                type_text: String::new(),
                value_text: String::new(),
                raw_value: String::new(),
                value_italic: false,
                name_colour: Color32::from_gray(110),
                value_colour: Color32::from_gray(110),
            });
            return;
        };
        for row in rows {
            if !filter.is_empty()
                && !row.name.to_ascii_lowercase().contains(filter)
                && !row.value.to_ascii_lowercase().contains(filter)
            {
                continue;
            }
            let expandable = row.reference != 0;
            let open = expandable && self.open.contains(&row.reference);

            // The PIC / Type column says what the row IS when it has no PICTURE
            // of its own — a column that is blank on half its rows is not a
            // column.
            let type_text = if !row.pic.is_empty() {
                format!("PIC {}", row.pic)
            } else if row.category == "group" {
                "Group".to_owned()
            } else if row.category == "condition" {
                "Condition".to_owned()
            } else if let Some(n) = row.occurs {
                format!("OCCURS {n} TIMES")
            } else {
                String::new()
            };

            // A "non-value" is named, never blank: an empty string, SPACES,
            // LOW-VALUES and HIGH-VALUES all look like nothing and mean four
            // different things.
            let (value_text, value_colour, value_italic) = match row.special {
                Some(SpecialValue::EmptyString) => {
                    ("(empty)".to_owned(), Color32::from_gray(120), true)
                }
                Some(SpecialValue::Spaces) => ("SPACES".to_owned(), Color32::from_gray(120), true),
                Some(SpecialValue::LowValues) => {
                    ("LOW-VALUES".to_owned(), Color32::from_gray(120), true)
                }
                Some(SpecialValue::HighValues) => {
                    ("HIGH-VALUES".to_owned(), Color32::from_gray(120), true)
                }
                Some(SpecialValue::Unset) => ("(unset)".to_owned(), Color32::from_gray(120), true),
                Some(SpecialValue::EvaluationError) => {
                    (row.value.clone(), Color32::from_rgb(230, 120, 120), true)
                }
                None => (
                    row.value.trim().to_owned(),
                    if row.value.trim() == "TRUE" {
                        Color32::from_rgb(120, 210, 140)
                    } else {
                        Color32::from_rgb(240, 200, 120)
                    },
                    false,
                ),
            };

            out.push(FlatRow {
                depth,
                name: row.name.clone(),
                reference: row.reference,
                parent: reference,
                expandable,
                open,
                editable: row.editable,
                type_text,
                value_text,
                raw_value: row.value.trim().to_owned(),
                value_italic,
                name_colour: if row.category == "condition" {
                    Color32::from_rgb(190, 160, 230)
                } else if row.category == "group" {
                    Color32::from_rgb(150, 175, 215)
                } else {
                    Color32::from_rgb(215, 220, 235)
                },
                value_colour,
            });
            if open {
                self.flatten_children(row.reference, depth + 1, filter, out);
            }
        }
    }
}
