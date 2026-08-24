// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Output / console panel — displays program output and diagnostic messages.

use egui::{Color32, Context, Panel, RichText, ScrollArea};

use crate::i18n::Tr;
use crate::runner::{DiagMsg, DiagOrigin, DiagSeverity, RunMsg};

// ── Diagnostic row text (spec 053 R10/R11/R12) ────────────────────────────────

fn severity_prefix(severity: DiagSeverity) -> (&'static str, Color32) {
    match severity {
        DiagSeverity::Error => ("✖ error", Color32::from_rgb(240, 80, 80)),
        DiagSeverity::Warning => ("⚠ warning", Color32::from_rgb(255, 200, 50)),
        DiagSeverity::Info => ("ℹ note", Color32::from_gray(180)),
    }
}

/// The main text of a diagnostic row. A `Site` origin names the developer's
/// own place and position — `FORM ▸ SITE — line:col: ✖ error: …` — where line
/// and column are **within that site's text** (R10). A `Generated` origin says
/// so explicitly and names the artifact instead of blaming a site (R12).
/// A plain or file diagnostic reads exactly as it always has.
pub fn diag_row_text(d: &DiagMsg, tr: &Tr) -> String {
    let (prefix, _) = severity_prefix(d.severity);
    match &d.origin {
        Some(DiagOrigin::Site {
            form_name,
            site,
            line,
            col,
            ..
        }) => format!(
            "{} — {}:{}: {}: {}",
            site.display_path(form_name),
            line,
            col,
            prefix,
            d.message
        ),
        Some(DiagOrigin::Generated { gen_path, line }) => format!(
            "[{}] {}:{}: {}: {}",
            tr.diag_generated_code,
            gen_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            line,
            prefix,
            d.message
        ),
        _ => format!("{}:{}: {}: {}", d.line, d.col, prefix, d.message),
    }
}

/// The quoted offending line and its column marker, for a `Site` diagnostic
/// (R11): the message locates the fault on its own, with no link followed and
/// no file opened.
pub fn diag_quote(d: &DiagMsg) -> Option<(String, String)> {
    if let Some(DiagOrigin::Site {
        line,
        col,
        source_line,
        ..
    }) = &d.origin
    {
        let gutter = format!("{line:>5} │ ");
        let marker = format!(
            "{}│ {}^",
            " ".repeat(gutter.chars().count() - 2),
            " ".repeat(col.saturating_sub(1) as usize)
        );
        Some((format!("{gutter}{source_line}"), marker))
    } else {
        None
    }
}

/// Whether activating this diagnostic can open an owning editor (R16): a
/// developer site or a hand-written file can; generated code and plain
/// diagnostics cannot (R15).
pub fn diag_is_activatable(d: &DiagMsg) -> bool {
    matches!(
        d.origin,
        Some(DiagOrigin::Site { .. }) | Some(DiagOrigin::File { .. })
    )
}

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

    /// Clear all output. This is the explicit "Clear" the developer asks for.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll_to_bottom = false;
    }

    /// Clear the RUN log — program output, diagnostics, status separators and
    /// runtime errors — and keep the assistant's conversation.
    ///
    /// Starting a run used to clear the whole pane, which wiped the agent
    /// trace: the developer would ask Grace for something, run the form to see
    /// the result, and lose the request, the plan, the review findings and the
    /// generated code they were still reading (operator report, 2026-07-30).
    /// The two streams share one pane, so a run only clears its own half.
    pub fn clear_run_output(&mut self) {
        self.lines.retain(|line| {
            matches!(
                line,
                OutputLine::Reasoning(_)
                    | OutputLine::AiInfo(_)
                    | OutputLine::AiDetail(_)
                    | OutputLine::AiError(_)
                    | OutputLine::AiQuestion(_)
            )
        });
        self.scroll_to_bottom = true;
    }

    /// Render the output panel at the bottom.
    ///
    /// Returns the origin of a diagnostic row the developer activated this
    /// frame (spec 053 R13) — the caller routes it to `goto_code_location`,
    /// the same return-an-action pattern the inspector uses.
    pub fn show(&mut self, panel_ui: &mut egui::Ui, tr: &Tr) -> Option<DiagOrigin> {
        let mut activated: Option<DiagOrigin> = None;
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        let frame = crate::theme::glass_panel_frame(
            ctx.global_style().visuals.panel_fill,
            &crate::theme::active(),
        );
        Panel::bottom("output_panel")
            .resizable(true)
            .default_size(160.0)
            .min_size(60.0)
            .frame(frame)
            .show(panel_ui, |ui| {
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
                                let (_, color) = severity_prefix(d.severity);
                                let text = RichText::new(diag_row_text(d, tr))
                                    .monospace()
                                    .size(font_size)
                                    .color(color);
                                if diag_is_activatable(d) {
                                    // R16: an activatable row is visibly a link
                                    // (underline) and answers hover.
                                    let resp = ui
                                        .add(
                                            egui::Label::new(text.underline())
                                                .sense(egui::Sense::click()),
                                        )
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .on_hover_text(tr.diag_click_hint);
                                    if resp.clicked() {
                                        activated = d.origin.clone();
                                    }
                                } else {
                                    ui.label(text);
                                }
                                if let Some((quote, marker)) = diag_quote(d) {
                                    ui.label(
                                        RichText::new(quote)
                                            .monospace()
                                            .size(font_size)
                                            .color(Color32::from_gray(150)),
                                    );
                                    ui.label(
                                        RichText::new(marker)
                                            .monospace()
                                            .size(font_size)
                                            .color(color),
                                    );
                                }
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
        activated
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
                // A copied log carries the site too (the display path is COBOL
                // identifiers, English in every language).
                match &d.origin {
                    Some(DiagOrigin::Site {
                        form_name,
                        site,
                        line,
                        col,
                        ..
                    }) => format!(
                        "{} — {}:{}: {}: {}",
                        site.display_path(form_name),
                        line,
                        col,
                        prefix,
                        d.message
                    ),
                    _ => format!("{}:{}: {}: {}", d.line, d.col, prefix, d.message),
                }
            }
            OutputLine::Error(e) => format!("error: {e}"),
            OutputLine::AiError(s) => format!("ai error: {s}"),
            OutputLine::AiQuestion(s) => format!("ai: {s}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Operator report (2026-07-30): running a form wiped the assistant's
    /// whole conversation. The run log and the agent trace share one pane, so
    /// starting a run must clear only its own half — the request, the plan,
    /// the review findings and the generated code the developer is still
    /// reading have to survive.
    #[test]
    fn a_run_clears_its_own_log_and_keeps_the_assistant_trace() {
        let mut out = OutputPanel::new();
        out.push_ai_line(crate::llm::AiLogKind::Info, "=== AGENT REQUEST · Grace ===");
        out.push_ai_line(crate::llm::AiLogKind::Detail, "Grace · tokens: 5946 in");
        out.push_ai_line(crate::llm::AiLogKind::Question, "Qual formulário?");
        out.push_ai_line(crate::llm::AiLogKind::Error, "provider timeout");
        out.push_ai_line(crate::llm::AiLogKind::Reasoning, "thinking…");
        out.push_status("── Running form textboxes-form.cfrm ──");
        out.push_line("DISPLAY output");
        out.push_msg(&RunMsg::Error("runtime error".into()));

        out.clear_run_output();
        let text = out.all_text();
        assert!(text.contains("AGENT REQUEST"), "agent trace must survive");
        assert!(text.contains("tokens: 5946 in"));
        assert!(text.contains("Qual formulário?"));
        assert!(text.contains("provider timeout"));
        assert!(text.contains("thinking"));
        assert!(!text.contains("Running form"), "run status must be cleared");
        assert!(!text.contains("DISPLAY output"), "run output must be cleared");
        assert!(!text.contains("runtime error"), "run errors must be cleared");

        // The explicit Clear button still wipes everything.
        out.clear();
        assert!(out.all_text().is_empty());
    }

    fn site_diag() -> DiagMsg {
        let mut d = DiagMsg::plain(DiagSeverity::Error, "syntax error near DISPLYA", 842, 17);
        d.origin = Some(DiagOrigin::Site {
            form_name: "MAIN-FORM".into(),
            form_path: std::path::PathBuf::from("/proj/forms/main-form.cfrm"),
            site: cobolt_forms::code_site::CodeSite::ControlEvent {
                control_id: "BTN-GO".into(),
                event: "onClick".into(),
            },
            line: 3,
            col: 12,
            source_line: "           DISPLYA \"HELLO\".".into(),
        });
        d
    }

    /// Spec 053 AC5: the rendered row carries the form name, the site display
    /// path, the line and column WITHIN the site, and the offending source
    /// line with its column marked — asserted on the produced text.
    #[test]
    fn a_site_diagnostic_renders_its_location_and_quote() {
        let tr = crate::i18n::Language::English.tr();
        let d = site_diag();

        let row = diag_row_text(&d, &tr);
        assert!(row.contains("MAIN-FORM ▸ BTN-GO ▸ onClick"), "row: {row}");
        assert!(row.contains("3:12"), "site line:col, not 842:17 — row: {row}");
        assert!(!row.contains("842"), "the artifact's line must not show: {row}");
        assert!(row.contains("✖ error"), "row: {row}");
        assert!(row.contains("syntax error near DISPLYA"), "row: {row}");

        let (quote, marker) = diag_quote(&d).expect("a Site diagnostic quotes its line");
        assert!(quote.contains("DISPLYA \"HELLO\""), "quote: {quote}");
        assert!(quote.contains("3 │"), "quote carries the site line: {quote}");
        // The caret sits under column 12.
        let caret_col = marker.find('^').expect("marker has a caret");
        let bar_col = marker.find('│').unwrap();
        assert_eq!(caret_col - bar_col - "│".len() - 1, 11, "caret under col 12");

        assert!(diag_is_activatable(&d), "a Site diagnostic is a link (R16)");
    }

    /// Spec 053 AC7: a diagnostic on a codegen-authored line says so, names
    /// the generated file and line, is attributed to no site — and is not a
    /// link (R15).
    #[test]
    fn a_generated_diagnostic_is_labelled_and_not_activatable() {
        let tr = crate::i18n::Language::English.tr();
        let mut d = DiagMsg::plain(DiagSeverity::Error, "bad PERFORM", 57, 4);
        d.origin = Some(DiagOrigin::Generated {
            gen_path: std::path::PathBuf::from("/proj/generated/main-form.cbl"),
            line: 57,
        });
        let row = diag_row_text(&d, &tr);
        assert!(row.contains("generated code"), "row: {row}");
        assert!(row.contains("main-form.cbl"), "row names the artifact: {row}");
        assert!(row.contains("57"), "row: {row}");
        assert!(!row.contains('▸'), "no site path on a generated line: {row}");
        assert!(diag_quote(&d).is_none(), "nothing of the developer's to quote");
        assert!(!diag_is_activatable(&d), "generated code is not a link");
    }

    /// A plain diagnostic (no origin) renders exactly as it always has —
    /// nothing regresses for producers that never resolve one.
    #[test]
    fn a_plain_diagnostic_renders_as_before() {
        let tr = crate::i18n::Language::English.tr();
        let d = DiagMsg::plain(DiagSeverity::Warning, "unused item", 12, 8);
        assert_eq!(diag_row_text(&d, &tr), "12:8: ⚠ warning: unused item");
        assert!(diag_quote(&d).is_none());
        assert!(!diag_is_activatable(&d));
    }

    /// The copied log carries the site path too, so a pasted diagnostic still
    /// says where (the display path is COBOL identifiers — English always).
    #[test]
    fn all_text_carries_the_site_path() {
        let mut out = OutputPanel::new();
        out.push_msg(&RunMsg::Diagnostic(site_diag()));
        let text = out.all_text();
        assert!(text.contains("MAIN-FORM ▸ BTN-GO ▸ onClick"), "log: {text}");
    }
}
