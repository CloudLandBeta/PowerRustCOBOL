// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Output / console panel — displays program output and diagnostic messages.

use egui::{
    Color32, Context, RichText, ScrollArea, TopBottomPanel,
};

use crate::runner::{DiagMsg, DiagSeverity, RunMsg};
use crate::i18n::Tr;

// ── OutputLine ────────────────────────────────────────────────────────────────

/// A single line in the output panel.
#[derive(Debug, Clone)]
pub enum OutputLine {
    /// Normal program output (DISPLAY statement).
    Output(String),
    /// Diagnostic from the parser / semantic analyser.
    Diagnostic(DiagMsg),
    /// Status / separator line.
    Status(String),
    /// Error from the runtime.
    Error(String),
}

// ── OutputPanel ───────────────────────────────────────────────────────────────

/// State for the output console panel.
#[derive(Default)]
pub struct OutputPanel {
    /// All lines accumulated in this session.
    lines:           Vec<OutputLine>,
    /// If true the view scrolls to the bottom on next frame.
    scroll_to_bottom: bool,
}

impl OutputPanel {
    pub fn new() -> Self { Self::default() }

    /// Push a new line from the runner.
    pub fn push_msg(&mut self, msg: &RunMsg) {
        match msg {
            RunMsg::Output(s) => {
                self.lines.push(OutputLine::Output(s.clone()));
            }
            RunMsg::Diagnostic(d) => {
                self.lines.push(OutputLine::Diagnostic(d.clone()));
            }
            RunMsg::Finished => {
                self.lines.push(OutputLine::Status(
                    "── Program finished ──".to_owned(),
                ));
            }
            RunMsg::Stopped => {
                self.lines.push(OutputLine::Status(
                    "── Stopped by user ──".to_owned(),
                ));
            }
            RunMsg::Error(e) => {
                self.lines.push(OutputLine::Error(e.clone()));
            }
        }
        self.scroll_to_bottom = true;
    }

    /// Add a plain output line (e.g. DISPLAY from the form runtime engine).
    pub fn push_line(&mut self, line: impl Into<String>) {
        self.lines.push(OutputLine::Output(line.into()));
        self.scroll_to_bottom = true;
    }

    /// Add a status separator (e.g. "── Running myprogram.cbl ──").
    pub fn push_status(&mut self, msg: impl Into<String>) {
        self.lines.push(OutputLine::Status(msg.into()));
        self.scroll_to_bottom = true;
    }

    /// Clear all output.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll_to_bottom = false;
    }

    /// Render the output panel at the bottom.
    pub fn show(&mut self, ctx: &Context, tr: &Tr) {
        let frame = crate::theme::glass_panel_frame(
            ctx.style().visuals.panel_fill, &crate::theme::active());
        TopBottomPanel::bottom("output_panel")
            .resizable(true)
            .default_height(160.0)
            .min_height(60.0)
            .frame(frame)
            .show(ctx, |ui| {
                // Header bar
                ui.horizontal(|ui| {
                    ui.strong(tr.panel_output);
                    ui.separator();
                    if ui.small_button(tr.panel_clear).clicked() {
                        self.clear();
                    }
                });
                ui.separator();

                // Scrollable content
                let scroll = ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(self.scroll_to_bottom);

                scroll.show(ui, |ui| {
                    for line in &self.lines {
                        match line {
                            OutputLine::Output(s) => {
                                ui.label(
                                    RichText::new(s)
                                        .monospace()
                                        .color(crate::theme::active().ed_plain),
                                );
                            }
                            OutputLine::Diagnostic(d) => {
                                let (color, prefix) = match d.severity {
                                    DiagSeverity::Error   =>
                                        (Color32::from_rgb(240, 80, 80), "✖ error"),
                                    DiagSeverity::Warning =>
                                        (Color32::from_rgb(255, 200, 50), "⚠ warning"),
                                    DiagSeverity::Info    =>
                                        (Color32::from_gray(180), "ℹ note"),
                                };
                                ui.label(
                                    RichText::new(
                                        format!("{}:{}: {}: {}",
                                            d.line, d.col, prefix, d.message)
                                    )
                                    .monospace()
                                    .color(color),
                                );
                            }
                            OutputLine::Status(s) => {
                                ui.label(
                                    RichText::new(s)
                                        .color(Color32::from_gray(130))
                                        .italics(),
                                );
                            }
                            OutputLine::Error(e) => {
                                ui.label(
                                    RichText::new(format!("✖ {e}"))
                                        .monospace()
                                        .color(Color32::from_rgb(240, 80, 80)),
                                );
                            }
                        }
                    }
                });

                self.scroll_to_bottom = false;
            });
    }
}
