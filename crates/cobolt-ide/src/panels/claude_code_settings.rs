// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Help → Claude Code Settings** (spec 084 R22–R27, R37): IDE-wide, like
//! Debug Settings — whether Claude Code is installed, which plugin version it
//! carries against this IDE's, where the tool server listens, its port, the
//! access token (set or not — never shown) and the Configure / Update and
//! Renew token buttons.
//!
//! The window never resizes itself (CONVENTIONS.md): it owns `size`, is
//! `.resizable(false).fixed_size(size)`, lays out from that number, and one
//! grip in its own foreground `Area` is the only writer of it — the
//! `leaderboard_modal` pattern.

use std::path::PathBuf;

use crate::i18n::Tr;

const DEFAULT_W: f32 = 640.0;
const DEFAULT_H: f32 = 340.0;
const MIN_W: f32 = 520.0;
const MIN_H: f32 = 300.0;
const MAX_W: f32 = 1400.0;
const MAX_H: f32 = 900.0;
const GRIP: f32 = 14.0;
const GRIP_INSET: f32 = 3.0;
/// Width of the label column.
const LABEL_W: f32 = 170.0;

/// What the IDE knows of Claude Code, from the background probe.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PluginStatus {
    /// The probe has not answered yet.
    #[default]
    Checking,
    /// No `claude` executable was found.
    NoClaude,
    /// `claude` is at this path; the plugin version installed, if any.
    Claude { path: PathBuf, plugin: Option<String> },
}

/// Everything the window shows, gathered by the app each frame.
#[derive(Debug, Clone)]
pub struct View {
    pub status: PluginStatus,
    pub ide_version: String,
    /// The configured port, and the port the tools actually listen on.
    pub port: u16,
    pub listening: Option<u16>,
    pub token_set: bool,
    /// A Configure run is in progress.
    pub busy: bool,
}

/// What the developer asked for this frame; the app carries it out.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Action {
    pub configure: bool,
    pub renew_token: bool,
    pub apply_port: Option<u16>,
}

pub struct ClaudeCodeSettingsWindow {
    pub open: bool,
    size: egui::Vec2,
    port_draft: Option<u16>,
    /// The last confirmation (port changed, token renewed).
    message: Option<String>,
}

impl Default for ClaudeCodeSettingsWindow {
    fn default() -> Self {
        Self { open: false, size: egui::vec2(DEFAULT_W, DEFAULT_H), port_draft: None, message: None }
    }
}

impl ClaudeCodeSettingsWindow {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.port_draft = None;
        self.message = None;
    }

    pub fn say(&mut self, message: String) {
        self.message = Some(message);
    }

    fn grip(&mut self, ctx: &egui::Context, window: egui::Rect) {
        let corner = window.max - egui::vec2(GRIP_INSET, GRIP_INSET);
        egui::Area::new(egui::Id::new("claude_code_settings_grip"))
            .order(egui::Order::Foreground)
            .fixed_pos(corner - egui::vec2(GRIP, GRIP))
            .show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(egui::vec2(GRIP, GRIP), egui::Sense::drag());
                if response.dragged() {
                    self.size += response.drag_delta();
                    self.size.x = self.size.x.clamp(MIN_W, MAX_W);
                    self.size.y = self.size.y.clamp(MIN_H, MAX_H);
                }
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNorthWest);
                }
                let colour = ui.visuals().weak_text_color();
                for offset in [2.0_f32, 6.0, 10.0] {
                    let len = GRIP - offset - 1.0;
                    ui.painter().line_segment(
                        [egui::pos2(rect.right() - len, rect.bottom()), egui::pos2(rect.right(), rect.bottom() - len)],
                        egui::Stroke::new(1.2, colour),
                    );
                }
            });
    }

    pub fn show(&mut self, ctx: &egui::Context, view: &View, tr: &Tr) -> Action {
        let mut action = Action::default();
        if !self.open {
            return action;
        }
        let mut open = self.open;
        let port_draft = self.port_draft.get_or_insert(view.port);
        let message = self.message.clone();
        let size = self.size;
        let window = egui::Window::new(tr.cc_settings_title)
            .id(egui::Id::new("claude_code_settings"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .fixed_size(size)
            .default_pos([120.0, 90.0])
            .show(ctx, |ui| {
                let row = |ui: &mut egui::Ui, label: &str, body: &mut dyn FnMut(&mut egui::Ui)| {
                    ui.horizontal(|ui| {
                        ui.add_sized([LABEL_W, 20.0], egui::Label::new(egui::RichText::new(label).strong()));
                        body(ui);
                    });
                    ui.add_space(6.0);
                };
                row(ui, tr.cc_claude, &mut |ui| {
                    ui.label(match &view.status {
                        PluginStatus::Checking => tr.cc_checking.to_owned(),
                        PluginStatus::NoClaude => tr.cc_claude_missing.to_owned(),
                        PluginStatus::Claude { path, .. } => tr.cc_claude_found.replacen("{}", &path.display().to_string(), 1),
                    });
                });
                row(ui, tr.cc_plugin, &mut |ui| {
                    ui.label(match &view.status {
                        PluginStatus::Checking => tr.cc_checking.to_owned(),
                        PluginStatus::NoClaude | PluginStatus::Claude { plugin: None, .. } => tr.cc_plugin_none.to_owned(),
                        PluginStatus::Claude { plugin: Some(v), .. } if *v == view.ide_version => {
                            tr.cc_plugin_current.replacen("{}", v, 1)
                        }
                        PluginStatus::Claude { plugin: Some(v), .. } => {
                            tr.cc_plugin_stale.replacen("{}", v, 1).replacen("{}", &view.ide_version, 1)
                        }
                    });
                });
                row(ui, tr.cc_address, &mut |ui| {
                    ui.label(match view.listening {
                        Some(p) => tr.cc_listening.replacen("{}", &format!("http://127.0.0.1:{p}/mcp"), 1),
                        None => tr.cc_not_listening.replacen("{}", &view.port.to_string(), 1),
                    });
                });
                row(ui, tr.cc_port, &mut |ui| {
                    ui.add(egui::DragValue::new(port_draft).range(1024..=65535));
                    if ui
                        .add_enabled(*port_draft != view.port, egui::Button::new(tr.cc_port_apply))
                        .clicked()
                    {
                        action.apply_port = Some(*port_draft);
                    }
                });
                row(ui, tr.cc_token, &mut |ui| {
                    ui.label(if view.token_set { tr.cc_token_set } else { tr.cc_token_unset });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    let stale = matches!(&view.status, PluginStatus::Claude { plugin: Some(v), .. } if *v != view.ide_version);
                    let label = if stale { tr.cc_update } else { tr.cc_configure };
                    let can = !view.busy && !matches!(view.status, PluginStatus::NoClaude);
                    if ui.add_enabled(can, egui::Button::new(label)).on_hover_text(tr.menu_configure_claude_code_hint).clicked() {
                        action.configure = true;
                    }
                    if ui.add_enabled(!view.busy, egui::Button::new(tr.cc_renew)).clicked() {
                        action.renew_token = true;
                    }
                    if view.busy {
                        ui.spinner();
                    }
                });
                if let Some(m) = &message {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(m).italics());
                }
            });
        if let Some(window) = window {
            self.grip(ctx, window.response.rect);
        }
        self.open = open;
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View {
        View {
            status: PluginStatus::Claude { path: "/opt/homebrew/bin/claude".into(), plugin: Some("1.80.100".into()) },
            ide_version: "1.80.116".into(),
            port: 5720,
            listening: Some(5720),
            token_set: true,
            busy: false,
        }
    }

    fn frame(ctx: &egui::Context, w: &mut ClaudeCodeSettingsWindow) -> egui::Rect {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0)));
        let tr = crate::i18n::Language::English.tr();
        let mut rect = egui::Rect::NOTHING;
        let mut out = ctx.run_ui(input, |ui| {
            w.show(ui.ctx(), &view(), &tr);
            rect = ui.ctx().memory(|m| m.area_rect(egui::Id::new("claude_code_settings"))).unwrap_or(egui::Rect::NOTHING);
        });
        out.textures_delta.clear();
        rect
    }

    /// Spec 084 R27 (AC12): nothing but a drag moves the size, and the window
    /// is the same rectangle frame after frame while nobody touches it.
    #[test]
    fn the_window_never_resizes_itself() {
        let ctx = egui::Context::default();
        let mut w = ClaudeCodeSettingsWindow::default();
        w.toggle();
        let first = (0..3).map(|_| frame(&ctx, &mut w)).last().unwrap();
        let later = (0..30).map(|_| frame(&ctx, &mut w)).last().unwrap();
        assert_eq!(w.size, egui::vec2(DEFAULT_W, DEFAULT_H), "the stored size is untouched");
        assert!(first.is_positive() && first.width() >= DEFAULT_W, "the window was measured: {first:?}");
        assert_eq!(first, later, "the window did not grow or shrink by itself");
        w.size += egui::vec2(5000.0, -5000.0);
        w.size.x = w.size.x.clamp(MIN_W, MAX_W);
        w.size.y = w.size.y.clamp(MIN_H, MAX_H);
        assert_eq!(w.size, egui::vec2(MAX_W, MIN_H), "a drag is clamped at both ends");
        println!("claude code settings: same rect over 30 untouched frames; drag clamped to {MAX_W}x{MIN_H}");
    }

    /// The Configure button reads Update while an older plugin is installed.
    #[test]
    fn an_older_plugin_offers_update() {
        let v = view();
        let stale = matches!(&v.status, PluginStatus::Claude { plugin: Some(p), .. } if *p != v.ide_version);
        assert!(stale);
    }
}
