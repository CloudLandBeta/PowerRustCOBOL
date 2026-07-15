// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Output / console panel — displays program output and diagnostic messages.

use egui::{Color32, Context, RichText, ScrollArea, TopBottomPanel};

use crate::i18n::Tr;
use crate::runner::{DiagMsg, DiagSeverity, RunMsg};

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
    /// A line of AI model reasoning (chain-of-thought).
    Reasoning(String),
    /// AI activity — lifecycle milestone (sending, streaming, completed).
    AiInfo(String),
    /// AI activity — secondary detail (status, model, timings).
    AiDetail(String),
    /// AI activity — a failure.
    AiError(String),
    /// AI activity — a model question / prose answer (reply with no code block),
    /// shown at a larger font. Never applied to the editor.
    AiQuestion(String),
}

// ── OutputPanel ───────────────────────────────────────────────────────────────

const DEFAULT_LOG_FONT_SIZE: f32 = 14.0;
const MIN_LOG_FONT_SIZE: f32 = 9.0;
const MAX_LOG_FONT_SIZE: f32 = 28.0;

/// State for the output console panel.
pub struct OutputPanel {
    /// All lines accumulated in this session.
    lines: Vec<OutputLine>,
    /// If true the view scrolls to the bottom on next frame.
    scroll_to_bottom: bool,
    /// Font size, in screen points/pixels, used by the log body.
    font_size: f32,
}

impl Default for OutputPanel {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            scroll_to_bottom: false,
            font_size: DEFAULT_LOG_FONT_SIZE,
        }
    }
}

impl OutputPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Plain-text snapshot of the current log, suitable for clipboard/export.
    pub fn all_text(&self) -> String {
        self.lines
            .iter()
            .map(OutputLine::as_text)
            .collect::<Vec<_>>()
            .join("\n")
    }

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
                self.lines
                    .push(OutputLine::Status("── Program finished ──".to_owned()));
            }
            RunMsg::Stopped => {
                self.lines
                    .push(OutputLine::Status("── Stopped by user ──".to_owned()));
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

    /// Append one AI activity line of the given kind. Routed from the AI log
    /// side-channel so the developer can watch each request unfold.
    pub fn push_ai_line(&mut self, kind: crate::llm::AiLogKind, text: impl Into<String>) {
        let line = match kind {
            crate::llm::AiLogKind::Info => OutputLine::AiInfo(text.into()),
            crate::llm::AiLogKind::Detail => OutputLine::AiDetail(text.into()),
            crate::llm::AiLogKind::Reasoning => OutputLine::Reasoning(text.into()),
            crate::llm::AiLogKind::Question => OutputLine::AiQuestion(text.into()),
            crate::llm::AiLogKind::Error => OutputLine::AiError(text.into()),
        };
        self.lines.push(line);
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
            ctx.style().visuals.panel_fill,
            &crate::theme::active(),
        );
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
                    if ui
                        .small_button("−")
                        .on_hover_text("Decrease log font size")
                        .clicked()
                    {
                        self.font_size = (self.font_size - 1.0).max(MIN_LOG_FONT_SIZE);
                    }
                    ui.label(
                        RichText::new(format!("{} px", self.font_size.round() as i32))
                            .small()
                            .color(Color32::from_gray(170)),
                    );
                    if ui
                        .small_button("+")
                        .on_hover_text("Increase log font size")
                        .clicked()
                    {
                        self.font_size = (self.font_size + 1.0).min(MAX_LOG_FONT_SIZE);
                    }
                    ui.separator();
                    if ui
                        .add_enabled(!self.lines.is_empty(), egui::Button::new("Copy log"))
                        .on_hover_text("Copy the entire current log to the clipboard")
                        .clicked()
                    {
                        ui.ctx().copy_text(self.all_text());
                    }
                    if ui
                        .add_enabled(!self.lines.is_empty(), egui::Button::new("Save log"))
                        .on_hover_text("Save the entire current log to an .ai.log file")
                        .clicked()
                    {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("AI log", &["ai.log", "log", "txt"])
                            .set_file_name("powerrustcobol.ai.log")
                            .save_file()
                        {
                            let path = ensure_ai_log_extension(path);
                            let _ = std::fs::write(path, self.all_text());
                        }
                    }
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
                    let font_size = self.font_size;
                    for line in &self.lines {
                        match line {
                            OutputLine::Output(s) => {
                                ui.label(
                                    RichText::new(s)
                                        .monospace()
                                        .size(font_size)
                                        .color(crate::theme::active().ed_plain),
                                );
                            }
                            OutputLine::Diagnostic(d) => {
                                let (color, prefix) = match d.severity {
                                    DiagSeverity::Error => {
                                        (Color32::from_rgb(240, 80, 80), "✖ error")
                                    }
                                    DiagSeverity::Warning => {
                                        (Color32::from_rgb(255, 200, 50), "⚠ warning")
                                    }
                                    DiagSeverity::Info => (Color32::from_gray(180), "ℹ note"),
                                };
                                ui.label(
                                    RichText::new(format!(
                                        "{}:{}: {}: {}",
                                        d.line, d.col, prefix, d.message
                                    ))
                                    .monospace()
                                    .size(font_size)
                                    .color(color),
                                );
                            }
                            OutputLine::Status(s) => {
                                ui.label(
                                    RichText::new(s)
                                        .size(font_size)
                                        .color(Color32::from_gray(130))
                                        .italics(),
                                );
                            }
                            OutputLine::Error(e) => {
                                ui.label(
                                    RichText::new(format!("✖ {e}"))
                                        .monospace()
                                        .size(font_size)
                                        .color(Color32::from_rgb(240, 80, 80)),
                                );
                            }
                            OutputLine::Reasoning(s) => {
                                ui.label(
                                    RichText::new(s)
                                        .monospace()
                                        .italics()
                                        .size(font_size)
                                        .color(Color32::from_rgb(150, 140, 190)),
                                );
                            }
                            OutputLine::AiInfo(s) => {
                                ui.label(
                                    RichText::new(s)
                                        .monospace()
                                        .size(font_size)
                                        .color(Color32::from_rgb(110, 190, 220)),
                                );
                            }
                            OutputLine::AiDetail(s) => {
                                ui.label(
                                    RichText::new(s)
                                        .monospace()
                                        .size((font_size - 1.0).max(MIN_LOG_FONT_SIZE))
                                        .color(Color32::from_gray(140)),
                                );
                            }
                            OutputLine::AiError(s) => {
                                ui.label(
                                    RichText::new(s)
                                        .monospace()
                                        .size(font_size)
                                        .color(Color32::from_rgb(240, 120, 90)),
                                );
                            }
                            OutputLine::AiQuestion(s) => {
                                // 2× the monospace body size — a model question must
                                // stand out in the log (and never lands in code).
                                ui.label(
                                    RichText::new(format!("💬 {s}"))
                                        .size(font_size * 2.0)
                                        .color(Color32::from_rgb(150, 210, 160)),
                                );
                            }
                        }
                    }
                });

                self.scroll_to_bottom = false;
            });
    }
}

fn ensure_ai_log_extension(path: std::path::PathBuf) -> std::path::PathBuf {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if name.ends_with(".ai.log") {
        path
    } else {
        path.with_file_name(format!("{name}.ai.log"))
    }
}

impl OutputLine {
    fn as_text(&self) -> String {
        match self {
            OutputLine::Output(s)
            | OutputLine::Status(s)
            | OutputLine::Reasoning(s)
            | OutputLine::AiInfo(s)
            | OutputLine::AiDetail(s) => s.clone(),
            OutputLine::Diagnostic(d) => {
                let prefix = match d.severity {
                    DiagSeverity::Error => "error",
                    DiagSeverity::Warning => "warning",
                    DiagSeverity::Info => "note",
                };
                format!("{}:{}: {}: {}", d.line, d.col, prefix, d.message)
            }
            OutputLine::Error(e) => format!("error: {e}"),
            OutputLine::AiError(s) => format!("ai error: {s}"),
            OutputLine::AiQuestion(s) => format!("ai: {s}"),
        }
    }
}
