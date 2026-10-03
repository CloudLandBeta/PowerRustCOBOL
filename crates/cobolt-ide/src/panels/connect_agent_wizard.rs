// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Connect a coding agent** — the first-run wizard (spec 084).
//!
//! Shown when the IDE starts and a coding agent is not connected, unless the
//! developer chose "Don't show again" for it. 800 × 400, fixed: the mascot at
//! a laptop on the left, facing a coding agent (a robot) at its own laptop,
//! a connection being made between them; the question on the right, with the
//! agent picker beside the title; Don't show again / Skip for now / Connect
//! 10 px from the bottom and right borders.
//!
//! Colours are fixed (the approved mockup), not the theme's: the illustration
//! must read the same over every IDE theme.

use egui::{pos2, vec2, Color32, CornerRadius, Pos2, Rect, Stroke};

use crate::coding_agents::{CodingAgent, Connection};
use crate::i18n::Tr;

pub const WIDTH: f32 = 800.0;
pub const HEIGHT: f32 = 400.0;
/// The illustration column.
const ART_W: f32 = 320.0;
/// The buttons' distance from the bottom and right borders.
pub const BUTTON_INSET: f32 = 10.0;
const BUTTON_H: f32 = 32.0;

const PANEL: Color32 = Color32::from_rgb(16, 18, 23);
const ART_BG: Color32 = Color32::from_rgb(18, 22, 32);
const EDGE: Color32 = Color32::from_rgba_premultiplied(26, 26, 26, 26);
const TEXT: Color32 = Color32::from_rgb(230, 235, 245);
const TEXT_DIM: Color32 = Color32::from_rgb(185, 190, 200);
const TEXT_FAINT: Color32 = Color32::from_rgb(138, 144, 156);
const ACCENT: Color32 = Color32::from_rgb(100, 160, 255);
const OK: Color32 = Color32::from_rgb(95, 207, 138);
const WARN: Color32 = Color32::from_rgb(232, 180, 90);

/// The mascot's head (256 px), embedded so the wizard never looks for files.
const CHIBI_HEAD: &[u8] = include_bytes!("../../../../assets/images/chibi-head-256.png");

/// What the developer chose this frame.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WizardAction {
    /// Connect the agent at this index of the list.
    pub connect: Option<usize>,
    /// Close until the IDE starts again.
    pub skip: bool,
    /// Never offer the agent at this index again.
    pub never: Option<usize>,
}

#[derive(Default)]
pub struct ConnectAgentWizard {
    pub open: bool,
    selected: usize,
    chibi: Option<egui::TextureHandle>,
}

/// Where the button strip sits inside the modal: its bottom-right corner
/// [`BUTTON_INSET`] from the modal's.
pub fn button_strip(modal: Rect) -> Rect {
    let max = modal.max - vec2(BUTTON_INSET, BUTTON_INSET);
    Rect::from_min_max(pos2(modal.min.x + ART_W + 24.0, max.y - BUTTON_H), max)
}

impl ConnectAgentWizard {
    fn chibi(&mut self, ctx: &egui::Context) -> Option<egui::TextureId> {
        if self.chibi.is_none() {
            let img = image::load_from_memory(CHIBI_HEAD).ok()?.to_rgba8();
            let size = [img.width() as usize, img.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
            self.chibi = Some(ctx.load_texture("connect-agent-chibi", color, egui::TextureOptions::LINEAR));
        }
        self.chibi.as_ref().map(|t| t.id())
    }

    /// Draw the wizard. `connections[i]` is what the IDE knows of `agents[i]`.
    pub fn show(&mut self, ctx: &egui::Context, agents: &[CodingAgent], connections: &[Connection], tr: &Tr) -> WizardAction {
        let mut action = WizardAction::default();
        if !self.open || agents.is_empty() {
            return action;
        }
        self.selected = self.selected.min(agents.len() - 1);
        let chibi = self.chibi(ctx);
        let time = ctx.input(|i| i.time);
        ctx.request_repaint(); // the connection is animated
        let agent = agents[self.selected];
        let connection = connections.get(self.selected).cloned().unwrap_or(Connection::Unknown);

        egui::Modal::new(egui::Id::new("connect_coding_agent"))
            .frame(egui::Frame::NONE)
            .backdrop_color(Color32::from_rgba_unmultiplied(4, 5, 8, 160))
            .show(ctx, |ui| {
                let (modal, _) = ui.allocate_exact_size(vec2(WIDTH, HEIGHT), egui::Sense::hover());
                let painter = ui.painter_at(modal);
                painter.rect_filled(modal, CornerRadius::same(14), PANEL);
                let art = Rect::from_min_size(modal.min, vec2(ART_W, HEIGHT));
                painter.rect_filled(
                    art,
                    CornerRadius { nw: 14, sw: 14, ne: 0, se: 0 },
                    ART_BG,
                );
                painter.line_segment([art.right_top(), art.right_bottom()], Stroke::new(1.0, EDGE));
                paint_scene(&painter, art, time, chibi);
                painter.rect_stroke(modal, CornerRadius::same(14), Stroke::new(1.0, EDGE), egui::StrokeKind::Inside);

                // ── The question ──
                let copy = Rect::from_min_max(
                    pos2(art.right() + 30.0, modal.top() + 28.0),
                    pos2(modal.right() - 30.0, modal.bottom() - BUTTON_H - 2.0 * BUTTON_INSET),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(copy), |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    ui.label(egui::RichText::new(tr.wiz_eyebrow.to_uppercase()).size(11.0).color(TEXT_FAINT));
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(tr.wiz_title).size(20.0).strong().color(TEXT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            egui::ComboBox::from_id_salt("connect_agent_pick")
                                .selected_text(agent.name)
                                .show_ui(ui, |ui| {
                                    for (i, a) in agents.iter().enumerate() {
                                        ui.selectable_value(&mut self.selected, i, a.name);
                                    }
                                })
                                .response
                                .on_hover_text(tr.wiz_eyebrow);
                        });
                    });
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(tr.wiz_lead.replacen("{}", agent.name, 1)).size(14.0).color(TEXT_DIM));
                    ui.add_space(6.0);
                    for point in [tr.wiz_point_once, tr.wiz_point_nothing, tr.wiz_point_local] {
                        ui.horizontal(|ui| {
                            let (dot, _) = ui.allocate_exact_size(vec2(14.0, 18.0), egui::Sense::hover());
                            ui.painter().circle_filled(dot.center(), 3.0, ACCENT);
                            ui.label(egui::RichText::new(point).size(13.5).color(TEXT_DIM));
                        });
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let (colour, text) = match &connection {
                            Connection::NotConnected { path } => (OK, tr.wiz_found.replacen("{}", agent.name, 1).replacen("{}", path, 1)),
                            Connection::Connected => (OK, tr.wiz_found.replacen("{}", agent.name, 1).replacen("{}", "", 1)),
                            Connection::NotInstalled => (WARN, tr.wiz_missing.replacen("{}", agent.name, 1)),
                            Connection::Unknown => (TEXT_FAINT, tr.wiz_checking.to_owned()),
                        };
                        let (dot, _) = ui.allocate_exact_size(vec2(14.0, 18.0), egui::Sense::hover());
                        ui.painter().circle_filled(dot.center(), 4.0, colour);
                        ui.label(egui::RichText::new(text).size(12.5).color(TEXT_FAINT));
                    });
                });

                // ── The buttons, 10 px from the bottom and right borders ──
                let strip = button_strip(modal);
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(strip).layout(egui::Layout::right_to_left(egui::Align::Center)),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let can_connect = matches!(connection, Connection::NotConnected { .. });
                        let connect = egui::Button::new(egui::RichText::new(tr.wiz_connect).strong().color(Color32::from_rgb(11, 18, 32)))
                            .fill(ACCENT)
                            .corner_radius(CornerRadius::same(8))
                            .min_size(vec2(96.0, BUTTON_H));
                        if ui.add_enabled(can_connect, connect).clicked() {
                            action.connect = Some(self.selected);
                        }
                        let skip = egui::Button::new(egui::RichText::new(tr.wiz_skip).color(TEXT))
                            .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 10))
                            .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 36)))
                            .corner_radius(CornerRadius::same(8))
                            .min_size(vec2(0.0, BUTTON_H));
                        if ui.add(skip).clicked() {
                            action.skip = true;
                        }
                        let never = egui::Button::new(egui::RichText::new(tr.wiz_never).color(TEXT_FAINT))
                            .frame(false)
                            .min_size(vec2(0.0, BUTTON_H));
                        if ui.add(never).clicked() {
                            action.never = Some(self.selected);
                        }
                    },
                );
            });
        if action != WizardAction::default() {
            self.open = false;
        }
        action
    }
}

/// The scene: the mascot at a laptop facing a coding agent at its own laptop,
/// a connection being made between the two screens. Drawn on a 320 × 220
/// design grid, centred in `art`.
fn paint_scene(painter: &egui::Painter, art: Rect, time: f64, chibi: Option<egui::TextureId>) {
    let s = 0.95;
    let origin = art.center() - vec2(160.0, 110.0) * s;
    let p = |x: f32, y: f32| origin + vec2(x, y) * s;
    let r = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(p(x, y), vec2(w, h) * s);
    let pulse = (0.65 + 0.35 * (time * 2.8).sin()) as f32;

    // Floor shadows.
    for cx in [78.0, 244.0] {
        painter.add(egui::Shape::ellipse_filled(p(cx, 176.0), vec2(58.0, 6.0) * s, Color32::from_black_alpha(90)));
    }

    // ── The mascot (dark armour, orange trim), facing right ──
    let armour = Color32::from_rgb(28, 29, 34);
    let orange = Color32::from_rgb(242, 140, 40);
    painter.rect_filled(r(38.0, 118.0, 42.0, 44.0), CornerRadius::same(12), armour);
    painter.rect_stroke(r(38.0, 118.0, 42.0, 44.0), CornerRadius::same(12), Stroke::new(1.5 * s, orange), egui::StrokeKind::Inside);
    painter.rect_filled(r(38.0, 140.0, 42.0, 5.0), CornerRadius::same(2), orange); // the rope belt
    painter.rect_filled(r(66.0, 136.0, 34.0, 9.0), CornerRadius::same(4), armour); // arm to the keyboard
    painter.circle_filled(p(100.0, 140.5), 5.0 * s, armour);
    match chibi {
        Some(tex) => {
            // 256 × 214: the helmet with its crest, eyes glowing.
            let head = Rect::from_center_size(p(60.0, 82.0), vec2(104.0, 87.0) * s);
            painter.image(tex, head, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        None => {
            painter.circle_filled(p(60.0, 86.0), 31.0 * s, armour);
        }
    }

    // ── The coding agent (robot), facing left ──
    let steel = Color32::from_rgb(201, 210, 227);
    let steel_dark = Color32::from_rgb(170, 182, 204);
    let agent_glow = Color32::from_rgb(100, 160, 255).gamma_multiply(pulse);
    painter.line_segment([p(256.0, 56.0), p(256.0, 44.0)], Stroke::new(3.0 * s, steel));
    painter.circle_filled(p(256.0, 41.0), 4.5 * s, agent_glow);
    painter.rect_filled(r(228.0, 56.0, 56.0, 46.0), CornerRadius::same(13), steel);
    painter.rect_filled(r(232.0, 70.0, 42.0, 20.0), CornerRadius::same(9), Color32::from_rgb(27, 35, 51));
    for x in [238.0, 252.0] {
        painter.rect_filled(r(x, 76.0, 9.0, 8.0), CornerRadius::same(3), Color32::from_rgb(111, 227, 255));
    }
    painter.circle_filled(p(285.0, 80.0), 4.0 * s, Color32::from_rgb(154, 166, 189));
    painter.rect_filled(r(236.0, 110.0, 44.0, 50.0), CornerRadius::same(12), steel_dark);
    painter.circle_filled(p(258.0, 128.0), 4.0 * s, agent_glow);
    painter.rect_filled(r(214.0, 136.0, 34.0, 9.0), CornerRadius::same(4), steel_dark);
    painter.circle_filled(p(214.0, 140.5), 5.0 * s, steel);

    // ── The two laptops, each screen facing its user ──
    let desk = Color32::from_rgb(43, 49, 63);
    let base = Color32::from_rgb(154, 163, 180);
    let lid = Color32::from_rgb(125, 135, 154);
    let screen = Color32::from_rgb(156, 196, 255).gamma_multiply(pulse);
    painter.rect_filled(r(84.0, 156.0, 56.0, 5.0), CornerRadius::same(2), desk);
    painter.rect_filled(r(90.0, 150.0, 42.0, 6.0), CornerRadius::same(2), base);
    painter.add(egui::Shape::convex_polygon(vec![p(126.0, 150.0), p(133.0, 150.0), p(124.0, 108.0), p(117.0, 108.0)], lid, Stroke::NONE));
    painter.line_segment([p(117.5, 110.0), p(125.5, 148.0)], Stroke::new(2.4 * s, screen));
    painter.rect_filled(r(180.0, 156.0, 56.0, 5.0), CornerRadius::same(2), desk);
    painter.rect_filled(r(188.0, 150.0, 42.0, 6.0), CornerRadius::same(2), base);
    painter.add(egui::Shape::convex_polygon(vec![p(187.0, 150.0), p(194.0, 150.0), p(203.0, 108.0), p(196.0, 108.0)], lid, Stroke::NONE));
    painter.line_segment([p(202.5, 110.0), p(194.5, 148.0)], Stroke::new(2.4 * s, screen));

    // ── The connection being made: a dashed arc flowing between the screens,
    // with a pulse travelling each way ──
    let (a, c, b) = (p(126.0, 104.0), p(160.0, 62.0), p(194.0, 104.0));
    let at = |t: f32| -> Pos2 {
        let u = 1.0 - t;
        pos2(u * u * a.x + 2.0 * u * t * c.x + t * t * b.x, u * u * a.y + 2.0 * u * t * c.y + t * t * b.y)
    };
    let phase = (time * 0.9).fract() as f32;
    let dashes = 9;
    for i in 0..dashes {
        let t0 = (i as f32 + phase) / dashes as f32;
        let t1 = t0 + 0.45 / dashes as f32;
        if t1 <= 1.0 {
            painter.line_segment([at(t0), at(t1)], Stroke::new(2.2 * s, ACCENT.gamma_multiply(0.9)));
        }
    }
    let travel = (time / 1.6).fract() as f32;
    painter.circle_filled(at(travel), 3.4 * s, Color32::from_rgb(207, 225, 255));
    painter.circle_filled(at(1.0 - ((time / 1.6 + 0.5).fract() as f32)), 3.4 * s, Color32::from_rgb(207, 225, 255).gamma_multiply(0.7));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The buttons sit 10 px from the bottom and right borders.
    #[test]
    fn the_buttons_sit_ten_px_from_the_bottom_right_corner() {
        let modal = Rect::from_min_size(pos2(100.0, 50.0), vec2(WIDTH, HEIGHT));
        let strip = button_strip(modal);
        assert_eq!(strip.right(), modal.right() - BUTTON_INSET);
        assert_eq!(strip.bottom(), modal.bottom() - BUTTON_INSET);
        assert!(strip.left() > modal.left() + ART_W, "right of the illustration");
    }

    /// The wizard is exactly 800 × 400 and stays so, frame after frame.
    #[test]
    fn the_wizard_is_800_by_400_and_never_resizes() {
        let ctx = egui::Context::default();
        let mut w = ConnectAgentWizard { open: true, ..Default::default() };
        let tr = crate::i18n::Language::English.tr();
        let agents = crate::coding_agents::AGENTS;
        let conn = vec![Connection::NotConnected { path: "/opt/homebrew/bin/claude".into() }];
        let mut rects = Vec::new();
        for _ in 0..20 {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0)));
            let mut out = ctx.run_ui(input, |ui| {
                w.show(ui.ctx(), agents, &conn, &tr);
            });
            out.textures_delta.clear();
            rects.push(ctx.memory(|m| m.area_rect(egui::Id::new("connect_coding_agent"))).unwrap_or(Rect::NOTHING));
        }
        let first = rects[2];
        assert!((first.width() - WIDTH).abs() < 0.5 && (first.height() - HEIGHT).abs() < 0.5, "{first:?}");
        assert!(rects[2..].iter().all(|r| *r == first), "it never grows or shrinks");
        println!("wizard: {}x{} over 18 frames, unchanged; buttons {BUTTON_INSET} px from the bottom-right corner", first.width(), first.height());
    }
}
