// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The question asked when a property edit would change a stylesheet rule that styles other
//! controls too (WASM-036 R11, R15): change the rule, so all of them change, or give only this
//! control its own value. Nothing is written until it is answered.
//!
//! A window that never resizes itself: its size is its own, held here, `.resizable(false)
//! .fixed_size(size)`, laid out from that size, and the one grip in a foreground `Area` is its only
//! writer (model: `indexed_engine_modal.rs`).

use eframe::egui;

use crate::i18n::Tr;
use crate::theme::Theme;

const DEFAULT_W: f32 = 520.0;
const DEFAULT_H: f32 = 230.0;
const MIN_W: f32 = 380.0;
const MIN_H: f32 = 190.0;
const MAX_W: f32 = 1000.0;
const MAX_H: f32 = 700.0;
const GRIP: f32 = 14.0;
const GRIP_INSET: f32 = 3.0;

/// What the window says about the rule: its selector, where it is, how many other controls it
/// styles, and whether that count is only a lower bound (the project's other forms took too long).
#[derive(Debug, Clone, PartialEq)]
pub struct RuleInfo {
    pub selector: String,
    pub file: Option<String>,
    pub line: usize,
    pub others: usize,
    pub at_least: bool,
}

/// The developer's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Not answered yet.
    Pending,
    ChangeRule,
    OnlyThisControl,
    Cancel,
}

pub struct CssRuleQuestion {
    /// The window's size, owned here; the grip is its only writer.
    pub size: egui::Vec2,
}

impl Default for CssRuleQuestion {
    fn default() -> Self {
        Self::new()
    }
}

impl CssRuleQuestion {
    pub fn new() -> Self {
        Self { size: egui::vec2(DEFAULT_W, DEFAULT_H) }
    }

    /// The window's id, for a test that reads its rectangle.
    pub fn id() -> egui::Id {
        egui::Id::new("css_rule_question")
    }

    fn resize_grip(&mut self, ctx: &egui::Context, theme: &Theme, window: egui::Rect) {
        let corner = window.max - egui::vec2(GRIP_INSET, GRIP_INSET);
        let origin = corner - egui::vec2(GRIP, GRIP);
        egui::Area::new(egui::Id::new("css_rule_question_grip")).order(egui::Order::Foreground).fixed_pos(origin).show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(egui::vec2(GRIP, GRIP), egui::Sense::drag());
            if response.dragged() {
                self.size += response.drag_delta();
                self.size.x = self.size.x.clamp(MIN_W, MAX_W);
                self.size.y = self.size.y.clamp(MIN_H, MAX_H);
            }
            let col = if response.hovered() || response.dragged() { theme.accent } else { theme.text_dim };
            let p = ui.painter();
            for i in 0..3 {
                let o = 4.0 * i as f32;
                p.line_segment([egui::pos2(rect.right() - o, rect.bottom()), egui::pos2(rect.right(), rect.bottom() - o)], egui::Stroke::new(1.5, col));
            }
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
            }
        });
    }

    /// The count as the window says it: `3`, or `at least 3`.
    pub fn count_text(tr: &Tr, info: &RuleInfo) -> String {
        if info.at_least {
            tr.css_shared_at_least.replacen("{}", &info.others.to_string(), 1)
        } else {
            info.others.to_string()
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, theme: &Theme, tr: &Tr, info: &RuleInfo) -> Answer {
        let mut answer = Answer::Pending;
        let mut open = true;
        let at = info.file.as_deref().map_or_else(|| "CssStyle".to_owned(), |f| format!("{f}:{}", info.line));
        let window = egui::Window::new(egui::RichText::new(tr.css_shared_title).size(18.0).strong())
            .id(Self::id())
            .open(&mut open)
            .collapsible(false)
            // NEVER `resizable(true)`: egui would negotiate this window's rectangle against its contents
            // every frame, and the two push each other outward. The size is ours; only the grip changes it.
            .resizable(false)
            .fixed_size(self.size)
            .default_pos([120.0, 120.0])
            .show(ctx, |ui| {
                // Laid out from the STORED size, never from what the Ui offers.
                let margin = ui.style().spacing.window_margin.sum().x;
                let inner_w = (self.size.x - margin).max(120.0);
                ui.set_width(inner_w);
                let body = tr.css_shared_body.replacen("{}", &info.selector, 1).replacen("{}", &at, 1).replacen("{}", &Self::count_text(tr, info), 1);
                ui.add(egui::Label::new(body).wrap());
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.css_shared_change_rule).clicked() {
                        answer = Answer::ChangeRule;
                    }
                    if ui.button(tr.css_shared_only_this).clicked() {
                        answer = Answer::OnlyThisControl;
                    }
                    if ui.button(tr.btn_cancel).clicked() {
                        answer = Answer::Cancel;
                    }
                });
            });
        if let Some(r) = window {
            self.resize_grip(ctx, theme, r.response.rect);
        }
        if !open && answer == Answer::Pending {
            answer = Answer::Cancel;
        }
        answer
    }
}
