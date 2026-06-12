// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Debugger panel — Phase 7.
//!
//! Shown on the right side of the IDE while a debug session is active.
//! Provides:
//!   • Step toolbar: ▶ Continue, ⤵ Step Over, ⏸ Pause, ■ Stop
//!   • Current paragraph / source location
//!   • Variable watch table with search filter

use egui::{
    Color32, Context, RichText, ScrollArea, SidePanel, TextEdit,
};

use crate::runner::{DebugRunner, RunMsg};
use crate::i18n::Tr;
use cobolt_runtime::{DebugCmd, DebugEvent, VarSnapshot};

// ── DebuggerPanel ─────────────────────────────────────────────────────────────

/// State for the IDE debugger panel.
pub struct DebuggerPanel {
    /// Variable search filter (typed by user).
    var_filter:     String,
    /// Most recent variable snapshot from the interpreter.
    vars:           Vec<VarSnapshot>,
    /// Name of the paragraph currently paused at.
    current_para:   String,
    /// Source line currently paused at (1-based, 0 = unknown).
    current_line:   u32,
    /// `true` when the interpreter is paused (waiting for a command).
    is_paused:      bool,
    /// Buffered output lines from the debug run (shown in the output panel).
    pub pending_output: Vec<RunMsg>,
}

impl Default for DebuggerPanel {
    fn default() -> Self { Self::new() }
}

impl DebuggerPanel {
    pub fn new() -> Self {
        Self {
            var_filter:     String::new(),
            vars:           Vec::new(),
            current_para:   String::new(),
            current_line:   0,
            is_paused:      false,
            pending_output: Vec::new(),
        }
    }

    /// Reset all state (call when starting a new session).
    pub fn reset(&mut self) {
        self.vars.clear();
        self.current_para.clear();
        self.current_line = 0;
        self.is_paused    = false;
        self.pending_output.clear();
    }

    /// Process events from `DebugRunner` and run messages; returns `true` if
    /// the UI needs to repaint.
    pub fn process(&mut self, runner: &mut DebugRunner) -> bool {
        let mut dirty = false;

        // Drain debug events.
        for ev in runner.drain_events() {
            dirty = true;
            match ev {
                DebugEvent::Paused { line, paragraph, vars, .. } => {
                    self.is_paused    = true;
                    self.current_line = line;
                    self.current_para = paragraph;
                    self.vars         = vars;
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

        // Drain run messages (pass to caller via pending_output).
        for msg in runner.drain_run() {
            dirty = true;
            self.pending_output.push(msg);
        }

        dirty
    }

    /// Render the debugger side panel.
    ///
    /// Returns the `DebugCmd` the user clicked, or `None`.
    pub fn show(&mut self, ctx: &Context, tr: &Tr, runner_active: bool) -> Option<DebugCmd> {
        let mut cmd: Option<DebugCmd> = None;

        SidePanel::right("debugger_panel")
            .resizable(true)
            .default_width(260.0)
            .min_width(160.0)
            .show(ctx, |ui| {
                ui.strong(tr.panel_debugger);
                ui.separator();

                // ── Step toolbar ──────────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.set_enabled(runner_active);

                    // Continue
                    if ui.add_enabled(self.is_paused,
                        egui::Button::new(RichText::new("▶").size(14.0))
                            .min_size([28.0, 28.0].into()))
                        .on_hover_text(tr.dbg_continue)
                        .clicked()
                    {
                        cmd = Some(DebugCmd::Continue);
                        self.is_paused = false;
                    }

                    // Step Over
                    if ui.add_enabled(self.is_paused,
                        egui::Button::new(RichText::new("⤵").size(14.0))
                            .min_size([28.0, 28.0].into()))
                        .on_hover_text(tr.dbg_step_over)
                        .clicked()
                    {
                        cmd = Some(DebugCmd::StepOver);
                        self.is_paused = false;
                    }

                    // Pause
                    if ui.add_enabled(!self.is_paused && runner_active,
                        egui::Button::new(RichText::new("⏸").size(14.0))
                            .min_size([28.0, 28.0].into()))
                        .on_hover_text(tr.dbg_pause)
                        .clicked()
                    {
                        cmd = Some(DebugCmd::Pause);
                    }
                });

                ui.separator();

                // ── Current location ──────────────────────────────────────────
                if !self.current_para.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("¶ {}", self.current_para))
                                .monospace()
                                .color(Color32::from_rgb(100, 200, 255)),
                        );
                        if self.current_line > 0 {
                            ui.label(
                                RichText::new(format!("  line {}", self.current_line))
                                    .color(Color32::from_gray(150)),
                            );
                        }
                    });
                    ui.label(
                        RichText::new(if self.is_paused { "● Paused" } else { "● Running" })
                            .color(if self.is_paused {
                                Color32::from_rgb(255, 200, 50)
                            } else {
                                Color32::from_rgb(80, 200, 80)
                            }),
                    );
                } else if runner_active {
                    ui.label(RichText::new("● Starting…").color(Color32::from_gray(150)));
                } else {
                    ui.label(RichText::new("No debug session").color(Color32::from_gray(110)));
                }

                ui.separator();

                // ── Variable watch ────────────────────────────────────────────
                ui.label(RichText::new(tr.dbg_variables).strong());
                ui.add(
                    TextEdit::singleline(&mut self.var_filter)
                        .hint_text(tr.dbg_filter_hint)
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(2.0);

                let filter = self.var_filter.to_ascii_lowercase();
                let filtered: Vec<&VarSnapshot> = self.vars.iter()
                    .filter(|v| {
                        filter.is_empty()
                            || v.name.to_ascii_lowercase().contains(&filter)
                    })
                    .collect();

                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Grid::new("var_watch")
                            .num_columns(2)
                            .striped(true)
                            .min_col_width(60.0)
                            .show(ui, |ui| {
                                let theme = crate::theme::active();
                                for v in &filtered {
                                    ui.label(
                                        RichText::new(&v.name)
                                            .monospace()
                                            .color(theme.ed_data),
                                    );
                                    ui.label(
                                        RichText::new(&v.value)
                                            .monospace()
                                            .color(theme.ed_plain),
                                    );
                                    ui.end_row();
                                }
                            });
                    });
            });

        cmd
    }

    /// Returns `true` if the interpreter is currently paused.
    pub fn is_paused(&self) -> bool { self.is_paused }

    /// Current paused source line (1-based), or 0 if not paused.
    pub fn current_line(&self) -> u32 { self.current_line }
}
