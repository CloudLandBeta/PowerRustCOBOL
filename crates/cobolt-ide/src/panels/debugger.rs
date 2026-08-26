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
use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::i18n::Tr;
use crate::runner::{DebugRunner, RunMsg};
use cobolt_runtime::{DebugEvent, VarSnapshot};

// ── Tab ───────────────────────────────────────────────────────────────────────

#[derive(PartialEq, Clone, Copy, Default)]
enum Tab {
    #[default]
    Variables,
    CallStack,
    Breakpoints,
}

// ── DebugAction ───────────────────────────────────────────────────────────────

/// Action requested by the debug window in a single frame.
pub enum DebugAction {
    Stop,
    Continue,
    StepOver,
    StepIn,
    Pause,
    /// The developer clicked the gutter beside a line: add or remove a
    /// breakpoint there. Carries the 1-based line in the displayed source.
    ToggleBreakpoint(u32),
}

// ── DebuggerPanel ─────────────────────────────────────────────────────────────

/// State for the floating debugger window.
pub struct DebuggerPanel {
    // Runtime state
    var_filter: String,
    vars: Vec<VarSnapshot>,
    current_para: String,
    current_line: u32,
    is_paused: bool,
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
        }
    }

    /// Reset all runtime state (call when starting a new session).
    pub fn reset(&mut self) {
        self.vars.clear();
        self.current_para.clear();
        self.current_line = 0;
        self.is_paused = false;
        self.pending_output.clear();
        self.last_scrolled_line = 0;
        self.force_center_current = false;
        self.animate = false;
        self.last_animate_step = None;
    }

    /// Supply the COBOL source text and initial breakpoint set at session start.
    pub fn set_source(&mut self, path: String, source: &str, bps: &HashSet<u32>) {
        self.source_path = path;
        self.source_lines = source.lines().map(|l| l.to_owned()).collect();
        self.breakpoints = bps.clone();
    }

    /// Sync the live breakpoint set from the editor gutter.
    pub fn set_breakpoints(&mut self, bps: &HashSet<u32>) {
        self.breakpoints = bps.clone();
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
            DebugEvent::Resumed => {
                self.is_paused = false;
            }
            DebugEvent::Finished => {
                self.is_paused = false;
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
            if action.is_none() && ctx.input(|i| i.key_pressed(Key::F11)) {
                action = Some(DebugAction::StepIn);
                self.is_paused = false;
            }
        }

        if action.is_none() {
            action = self.maybe_animate_step(ctx);
        }

        let need_scroll = self.should_center_current_line();

        egui::CentralPanel::default().show(panel_ui, |ui| {
            self.status_row(ui);
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
            if action.is_none() && ctx.input(|i| i.key_pressed(Key::F11)) {
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

                            self.status_row(ui);
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

    fn status_row(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (status_text, status_color) = if self.is_paused {
                let text = if self.current_para.is_empty() {
                    "● Paused".to_owned()
                } else {
                    format!(
                        "● Paused — {}   line {}",
                        self.current_para, self.current_line
                    )
                };
                (text, Color32::from_rgb(220, 180, 50))
            } else {
                ("○ Running".to_owned(), Color32::from_rgb(80, 200, 80))
            };
            ui.label(RichText::new(status_text).color(status_color).size(12.0));
        });
    }

    // ── Controls toolbar ──────────────────────────────────────────────────────

    fn toolbar(&mut self, ui: &mut egui::Ui, tr: &Tr) -> Option<DebugAction> {
        let mut action: Option<DebugAction> = None;

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
                .on_hover_text("Step into the next statement")
                .clicked()
            {
                action = Some(DebugAction::StepIn);
                self.is_paused = false;
                self.last_animate_step = None;
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
    fn split_body(&mut self, ui: &mut egui::Ui, tr: &Tr, need_scroll: bool) -> Option<u32> {
        let body_h = ui.available_height().max(180.0);
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
                        toggled = self.code_viewer(ui, need_scroll, pane_w);
                    });
                    row.col(|ui| {
                        self.data_tabs(ui, tr);
                    });
                });
            });

        toggled
    }

    // ── Code viewer ───────────────────────────────────────────────────────────

    /// Returns the line whose gutter was clicked, if any — the caller turns it
    /// into a [`DebugAction::ToggleBreakpoint`]. The viewer takes `&self`, so it
    /// reports the click rather than editing the set itself.
    fn code_viewer(&self, ui: &mut egui::Ui, need_scroll: bool, pane_w: f32) -> Option<u32> {
        // File path in muted monospace
        ui.label(
            RichText::new(&self.source_path)
                .monospace()
                .size(10.0)
                .color(Color32::from_gray(95)),
        );

        // The gutter click collected this frame. `code_viewer` only reads the
        // panel, so the toggle travels out as a return value instead of being
        // applied here — the breakpoint set lives in the editor, and both the
        // panel and the running debuggee are synced from there.
        let mut toggled: Option<u32> = None;

        ScrollArea::both()
            .id_salt("dbg_code_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let current = self.current_line;
                let bps = &self.breakpoints;

                for (idx, line_text) in self.source_lines.iter().enumerate() {
                    let line_num = (idx + 1) as u32;
                    let is_current = line_num == current;
                    let is_bp = bps.contains(&line_num);

                    if line_text.trim().is_empty() {
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(pane_w, 3.0), egui::Sense::hover());
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
                            Vec2::new(rect.width().max(pane_w), 19.0),
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
                            [32.0, 18.0],
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
                            ui.allocate_exact_size(Vec2::new(18.0, 18.0), egui::Sense::click());
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

                        // Syntax-highlighted source line
                        let job = build_cobol_layout_job(line_text);
                        ui.label(job);
                    });

                    // Auto-scroll when current line changes
                    if is_current && need_scroll {
                        ui.scroll_to_cursor(Some(egui::Align::Center));
                    }
                }
            });

        toggled
    }

    // ── Tabbed data panel ─────────────────────────────────────────────────────

    fn data_tabs(&mut self, ui: &mut egui::Ui, tr: &Tr) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.active_tab, Tab::Variables, tr.dbg_variables);
            ui.selectable_value(&mut self.active_tab, Tab::CallStack, "Call stack");
            ui.selectable_value(&mut self.active_tab, Tab::Breakpoints, "Breakpoints");
        });
        ui.separator();

        match self.active_tab {
            Tab::Variables => {
                ui.add(
                    TextEdit::singleline(&mut self.var_filter)
                        .hint_text("Filter data items")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(2.0);
                let filter = self.var_filter.to_ascii_lowercase();
                let filtered: Vec<&VarSnapshot> = self
                    .vars
                    .iter()
                    .filter(|v| !Self::is_generated_control_handler_var(v))
                    .filter(|v| {
                        filter.is_empty()
                            || v.name.to_ascii_lowercase().contains(&filter)
                            || v.scope.to_ascii_lowercase().contains(&filter)
                            || v.value.to_ascii_lowercase().contains(&filter)
                    })
                    .collect();

                let theme = crate::theme::active();
                TableBuilder::new(ui)
                    .id_salt("dbg_var_table")
                    .striped(true)
                    .resizable(true)
                    .auto_shrink([false, false])
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::initial(170.0).at_least(100.0).resizable(true))
                    .column(Column::initial(105.0).at_least(78.0).resizable(true))
                    .column(Column::remainder().at_least(80.0))
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.strong("Variable name");
                        });
                        header.col(|ui| {
                            ui.strong("Scope");
                        });
                        header.col(|ui| {
                            ui.strong("Value");
                        });
                    })
                    .body(|mut body| {
                        for v in &filtered {
                            body.row(24.0, |mut row| {
                                row.col(|ui| {
                                    if ui
                                        .button(
                                            RichText::new(&v.name).monospace().color(theme.ed_data),
                                        )
                                        .on_hover_text("Show data item value")
                                        .clicked()
                                    {
                                        self.selected_var = Some((*v).clone());
                                    }
                                });
                                row.col(|ui| {
                                    ui.label(RichText::new(&v.scope).monospace());
                                });
                                row.col(|ui| {
                                    let max_px = (ui.available_width() - 6.0).max(24.0);
                                    let preview = Self::fit_value_preview(ui, &v.value, max_px);
                                    ui.label(
                                        RichText::new(preview).monospace().color(theme.ed_plain),
                                    );
                                });
                            });
                        }
                    });
            }

            Tab::CallStack => {
                ScrollArea::vertical()
                    .id_salt("dbg_stack_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.current_para.is_empty() {
                            ui.label(
                                RichText::new("No active frame").color(Color32::from_gray(100)),
                            );
                        } else {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("►").color(Color32::from_rgb(220, 180, 50)));
                                ui.label(
                                    RichText::new(&self.current_para)
                                        .monospace()
                                        .color(Color32::from_rgb(100, 180, 255)),
                                );
                            });
                            if self.current_line > 0 {
                                ui.label(
                                    RichText::new(format!("   line {}", self.current_line))
                                        .monospace()
                                        .size(11.0)
                                        .color(Color32::from_gray(120)),
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
