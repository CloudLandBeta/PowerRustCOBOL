// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Backend window of a Web or Mobile project: the counterpart of the
//! desktop's Building window. It opens when the Backend starts, shows what the
//! Backend prints, stays up while it runs, carries the **Stop Backend** button,
//! and closing it while the Backend runs stops the Backend — after a warning.
//!
//! Its size is its own: only the developer's drag on the grip changes it
//! (the model is `leaderboard_modal`).

use egui::{Color32, RichText};

use crate::i18n::Tr;
use crate::theme::Theme;

const DEFAULT_W: f32 = 640.0;
const DEFAULT_H: f32 = 380.0;
const MIN_W: f32 = 420.0;
const MIN_H: f32 = 240.0;
const MAX_W: f32 = 1800.0;
const MAX_H: f32 = 1200.0;
const GRIP: f32 = 14.0;
const GRIP_INSET: f32 = 3.0;
/// The title bar, the address line, the button row and the paddings: a constant
/// subtracted from the height to give the log, so the log never measures itself
/// against the space it was handed.
const CHROME_H: f32 = 118.0;
/// The log keeps the newest lines only.
const MAX_LINES: usize = 5000;

/// What the developer asked for in a frame.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BackendWindowAction {
    /// Stop the Backend (the button, or "Stop Backend and close" in the warning).
    pub stop: bool,
}

pub struct BackendWindow {
    /// Shown or hidden. Set when a Backend starts; cleared by a close.
    visible: bool,
    /// The close was asked while the Backend runs: the warning is up.
    confirm_close: bool,
    size: egui::Vec2,
    lines: Vec<String>,
    /// The address the Backend listens on, once it printed it.
    url: Option<String>,
}

impl Default for BackendWindow {
    fn default() -> Self {
        Self { visible: false, confirm_close: false, size: egui::vec2(DEFAULT_W, DEFAULT_H), lines: Vec::new(), url: None }
    }
}

impl BackendWindow {
    /// A Backend started: a fresh log, and the window up.
    pub fn begin(&mut self) {
        self.lines.clear();
        self.url = None;
        self.confirm_close = false;
        self.visible = true;
    }

    pub fn push_line(&mut self, line: String) {
        self.lines.push(line);
        if self.lines.len() > MAX_LINES {
            let drop = self.lines.len() - MAX_LINES;
            self.lines.drain(..drop);
        }
    }

    pub fn set_url(&mut self, url: String) {
        self.url = Some(url);
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// The close button (or Esc) while `running`: a running Backend asks first;
    /// a finished one just closes. Returns whether the window is now closed.
    pub fn request_close(&mut self, running: bool) -> bool {
        if running {
            self.confirm_close = true;
            false
        } else {
            self.visible = false;
            self.confirm_close = false;
            true
        }
    }

    /// The warning was answered "Keep running".
    pub fn keep(&mut self) {
        self.confirm_close = false;
    }

    /// The Backend was stopped (by the button or by the close): the window goes
    /// if the stop came from the close, stays with its log if it came from the
    /// button.
    pub fn stopped(&mut self, close: bool) {
        self.confirm_close = false;
        if close {
            self.visible = false;
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, theme: &Theme, tr: &Tr, running: bool) -> BackendWindowAction {
        let mut action = BackendWindowAction::default();
        if !self.visible {
            return action;
        }
        let mut open = true;
        let window = egui::Window::new(RichText::new(tr.backend_window_title).size(18.0).strong())
            .id(egui::Id::new("backend_window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .fixed_size(self.size)
            .default_pos([60.0, 80.0])
            .show(ctx, |ui| {
                let margin = ui.style().spacing.window_margin.sum().x;
                let stroke = 2.0 * ui.style().visuals.window_stroke.width;
                ui.set_width(self.size.x - margin - stroke);
                ui.horizontal(|ui| {
                    let (label, colour) = if running {
                        (self.url.clone().map(|u| tr.backend_running.replacen("{}", &u, 1)).unwrap_or_else(|| "…".to_owned()), Color32::from_rgb(60, 190, 100))
                    } else {
                        (tr.backend_stopped.to_owned(), theme.text_dim)
                    };
                    if running && self.url.is_none() {
                        ui.add(egui::Spinner::new());
                    }
                    ui.label(RichText::new(label).color(colour));
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(running, egui::Button::new(tr.backend_stop)).clicked() {
                        action.stop = true;
                    }
                    if ui.button(tr.clipboard_copy).clicked() {
                        ui.ctx().copy_text(self.lines.join("\n"));
                    }
                });
                ui.separator();
                let log_h = (self.size.y - CHROME_H).max(60.0);
                let weak = ui.visuals().weak_text_color();
                egui::ScrollArea::vertical().max_height(log_h).min_scrolled_height(log_h).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                    for line in &self.lines {
                        ui.label(RichText::new(line).monospace().color(weak));
                    }
                });
            });
        if let Some(window) = window {
            self.grip(ctx, theme, window.response.rect);
        }
        if !open {
            self.request_close(running);
        }
        if self.confirm_close {
            self.warning(ctx, tr, &mut action);
        }
        action
    }

    fn warning(&mut self, ctx: &egui::Context, tr: &Tr, action: &mut BackendWindowAction) {
        let mut stop_and_close = false;
        let mut keep = false;
        egui::Modal::new(egui::Id::new("backend_window_close_warning")).backdrop_color(Color32::from_black_alpha(150)).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading(tr.backend_window_title);
            ui.add_space(8.0);
            ui.label(tr.backend_window_close_warning);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(tr.backend_window_stop_close).clicked() {
                    stop_and_close = true;
                }
                if ui.button(tr.backend_window_keep).clicked() {
                    keep = true;
                }
            });
        });
        if stop_and_close {
            action.stop = true;
            self.stopped(true);
        } else if keep {
            self.keep();
        }
    }

    fn grip(&mut self, ctx: &egui::Context, theme: &Theme, window: egui::Rect) {
        let origin = window.max - egui::vec2(GRIP_INSET + GRIP, GRIP_INSET + GRIP);
        egui::Area::new(egui::Id::new("backend_window_resize_grip")).order(egui::Order::Foreground).fixed_pos(origin).show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(egui::vec2(GRIP, GRIP), egui::Sense::drag());
            if response.dragged() {
                self.size += response.drag_delta();
                self.size.x = self.size.x.clamp(MIN_W, MAX_W);
                self.size.y = self.size.y.clamp(MIN_H, MAX_H);
            }
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNorthWest);
            }
            let colour = if response.hovered() || response.dragged() { theme.accent } else { theme.text_dim };
            for (i, offset) in [2.0_f32, 6.0, 10.0].iter().enumerate() {
                let len = GRIP - offset - 1.0;
                let stroke = egui::Stroke::new(if i == 0 { 1.6 } else { 1.2 }, colour);
                ui.painter().line_segment([egui::pos2(rect.right() - len, rect.bottom()), egui::pos2(rect.right(), rect.bottom() - len)], stroke);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_a_running_backend_asks_first_and_a_finished_one_just_closes() {
        let mut w = BackendWindow::default();
        w.begin();
        assert!(w.is_visible());
        assert!(!w.request_close(true), "running: the warning, not a close");
        assert!(w.confirm_close && w.is_visible());
        w.keep();
        assert!(!w.confirm_close && w.is_visible(), "Keep running leaves everything as it was");
        w.request_close(true);
        w.stopped(true);
        assert!(!w.is_visible(), "Stop Backend and close closes");
        w.begin();
        assert!(w.request_close(false) && !w.is_visible(), "a Backend that ended closes without a warning");
    }

    #[test]
    fn the_log_keeps_the_newest_lines_and_a_new_run_starts_clean() {
        let mut w = BackendWindow::default();
        w.begin();
        for i in 0..MAX_LINES + 10 {
            w.push_line(format!("line {i}"));
        }
        assert_eq!(w.lines.len(), MAX_LINES);
        assert_eq!(w.lines.first().unwrap(), "line 10");
        w.set_url("ws://127.0.0.1:8443".into());
        w.begin();
        assert!(w.lines.is_empty() && w.url.is_none());
    }
}
