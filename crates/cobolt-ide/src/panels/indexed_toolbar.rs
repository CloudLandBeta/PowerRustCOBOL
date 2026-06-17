// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Vector-icon toolbar for the Indexed File Editor.

use egui::{Color32, Pos2, Rect, Shape, Stroke, Ui, Vec2};

use crate::theme::Theme;

use super::indexed_editor::IndexedEditorAction;

pub fn draw_indexed_toolbar(
    ui: &mut Ui,
    finalized: bool,
    has_selection: bool,
    tooltips: &IndexedToolbarTips,
    theme: &Theme,
) -> IndexedEditorAction {
    let mut action = IndexedEditorAction::None;
    let strip_rect = ui.clip_rect();
    ui.painter().rect_filled(strip_rect, 0.0, ui.visuals().panel_fill);

    let icon_ref = 22.0_f32;
    let icon_size = 26.0_f32;
    let btn_size = icon_ref + 10.0;
    let group_gap = btn_size * 0.5;
    let col_normal = Color32::from_rgba_unmultiplied(
        theme.text_bright.r(),
        theme.text_bright.g(),
        theme.text_bright.b(),
        210,
    );
    let col_dim = Color32::from_rgba_unmultiplied(
        theme.text_dim.r(),
        theme.text_dim.g(),
        theme.text_dim.b(),
        120,
    );
    let col_accent = theme.accent;

    let mut icon_btn = |ui: &mut Ui,
                        enabled: bool,
                        toggled: bool,
                        tooltip: &str,
                        draw: &dyn Fn(&mut Vec<Shape>, Rect, Color32)| -> bool {
        let (resp, painter) = ui.allocate_painter(Vec2::splat(btn_size), egui::Sense::click());
        let icon_rect = Rect::from_center_size(resp.rect.center(), Vec2::splat(icon_size));
        let col = if !enabled {
            col_dim
        } else if toggled {
            col_accent
        } else {
            col_normal
        };
        if resp.hovered() && enabled {
            painter.rect_filled(resp.rect, 6.0, theme.bg_hover);
        }
        let mut shapes = Vec::new();
        draw(&mut shapes, icon_rect, col);
        normalize_icon(&mut shapes, icon_rect.center(), icon_ref);
        painter.extend(shapes);
        let clicked = enabled && resp.clicked();
        if !tooltip.is_empty() {
            resp.on_hover_text(tooltip);
        }
        clicked
    };

    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(4.0);

        if icon_btn(ui, true, false, tooltips.save, &icon_save) {
            action = IndexedEditorAction::Save;
        }
        if icon_btn(ui, true, false, tooltips.save_generate, &icon_generate) {
            action = IndexedEditorAction::SaveAndGenerate;
        }

        group_sep(ui, group_gap, theme);

        if icon_btn(ui, !finalized, false, tooltips.add_field, &icon_add) {
            action = IndexedEditorAction::AddField;
        }
        if icon_btn(ui, !finalized && has_selection, false, tooltips.remove_field, &icon_delete) {
            action = IndexedEditorAction::RemoveField;
        }
        if icon_btn(ui, !finalized, false, tooltips.raw_edit, &icon_raw) {
            action = IndexedEditorAction::RawEdit;
        }
        if icon_btn(ui, !finalized, false, tooltips.finalize, &icon_lock) {
            action = IndexedEditorAction::Finalize;
        }

        group_sep(ui, group_gap, theme);

        if icon_btn(ui, finalized, false, tooltips.open_grid, &icon_grid) {
            action = IndexedEditorAction::OpenGrid;
        } else {
            let (resp, painter) =
                ui.allocate_painter(Vec2::splat(btn_size), egui::Sense::hover());
            let icon_rect =
                Rect::from_center_size(resp.rect.center(), Vec2::splat(icon_size));
            let mut shapes = Vec::new();
            icon_grid(&mut shapes, icon_rect, col_dim);
            normalize_icon(&mut shapes, icon_rect.center(), icon_ref);
            painter.extend(shapes);
            resp.on_hover_text(tooltips.grid_disabled);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(if finalized {
                tooltips.badge_finalized
            } else {
                tooltips.badge_draft
            });
        });
    });

    action
}

pub struct IndexedToolbarTips<'a> {
    pub save: &'a str,
    pub save_generate: &'a str,
    pub add_field: &'a str,
    pub remove_field: &'a str,
    pub raw_edit: &'a str,
    pub finalize: &'a str,
    pub open_grid: &'a str,
    pub grid_disabled: &'a str,
    pub badge_draft: &'a str,
    pub badge_finalized: &'a str,
}

fn group_sep(ui: &mut Ui, gap: f32, theme: &Theme) {
    ui.add_space(gap * 0.5);
    let cy = ui.max_rect().center().y;
    let x = ui.cursor().min.x;
    ui.painter().vline(
        x,
        cy - 14.0..=cy + 14.0,
        Stroke::new(1.0, theme.line()),
    );
    ui.add_space(gap * 0.5);
}

fn normalize_icon(shapes: &mut [Shape], center: Pos2, target_ext: f32) {
    use egui::emath::TSTransform;
    let mut bbox = Rect::NOTHING;
    for s in shapes.iter() {
        bbox = bbox.union(s.visual_bounding_rect());
    }
    if !bbox.is_finite() {
        return;
    }
    let cur = bbox.size().max_elem();
    if cur <= 0.01 || target_ext <= 0.01 {
        return;
    }
    let k = target_ext / cur;
    let translation = center.to_vec2() - k * bbox.center().to_vec2();
    let t = TSTransform::new(translation, k);
    for s in shapes.iter_mut() {
        s.transform(t);
    }
}

fn stroke(shapes: &mut Vec<Shape>, pts: Vec<Pos2>, col: Color32, w: f32) {
    shapes.push(Shape::line(pts, Stroke::new(w, col)));
}

fn icon_save(shapes: &mut Vec<Shape>, r: Rect, c: Color32) {
    let w = 1.8;
    let body = Rect::from_min_size(
        r.min + Vec2::new(r.width() * 0.18, r.height() * 0.22),
        Vec2::new(r.width() * 0.64, r.height() * 0.58),
    );
    shapes.push(Shape::rect_stroke(body, 2.0, Stroke::new(w, c)));
    let tab = Rect::from_min_size(
        body.min - Vec2::new(0.0, r.height() * 0.12),
        Vec2::new(body.width() * 0.45, r.height() * 0.14),
    );
    shapes.push(Shape::rect_filled(tab, 1.0, c));
}

fn icon_generate(shapes: &mut Vec<Shape>, r: Rect, c: Color32) {
    icon_save(shapes, r, c);
    let cx = r.center().x;
    stroke(
        shapes,
        vec![
            Pos2::new(cx, r.center().y - 3.0),
            Pos2::new(cx, r.center().y + 5.0),
        ],
        c,
        1.6,
    );
    stroke(
        shapes,
        vec![
            Pos2::new(cx - 4.0, r.center().y + 1.0),
            Pos2::new(cx, r.center().y + 5.0),
            Pos2::new(cx + 4.0, r.center().y + 1.0),
        ],
        c,
        1.6,
    );
}

fn icon_add(shapes: &mut Vec<Shape>, r: Rect, c: Color32) {
    let cx = r.center().x;
    let cy = r.center().y;
    stroke(
        shapes,
        vec![Pos2::new(cx - 7.0, cy), Pos2::new(cx + 7.0, cy)],
        c,
        2.0,
    );
    stroke(
        shapes,
        vec![Pos2::new(cx, cy - 7.0), Pos2::new(cx, cy + 7.0)],
        c,
        2.0,
    );
}

fn icon_delete(shapes: &mut Vec<Shape>, r: Rect, c: Color32) {
    stroke(
        shapes,
        vec![
            r.left_top() + Vec2::new(4.0, 4.0),
            r.right_bottom() - Vec2::new(4.0, 4.0),
        ],
        c,
        2.0,
    );
    stroke(
        shapes,
        vec![
            r.right_top() + Vec2::new(-4.0, 4.0),
            r.left_bottom() + Vec2::new(4.0, -4.0),
        ],
        c,
        2.0,
    );
}

fn icon_raw(shapes: &mut Vec<Shape>, r: Rect, c: Color32) {
    let body = Rect::from_center_size(r.center(), Vec2::new(r.width() * 0.72, r.height() * 0.78));
    shapes.push(Shape::rect_stroke(body, 2.0, Stroke::new(1.6, c)));
    for i in 0..3 {
        let y = body.min.y + body.height() * (0.28 + i as f32 * 0.22);
        stroke(
            shapes,
            vec![Pos2::new(body.min.x + 4.0, y), Pos2::new(body.max.x - 4.0, y)],
            c,
            1.4,
        );
    }
}

fn icon_lock(shapes: &mut Vec<Shape>, r: Rect, c: Color32) {
    let arch = egui::epaint::PathShape::line(
        vec![
            Pos2::new(r.center().x - 6.0, r.center().y + 2.0),
            Pos2::new(r.center().x - 6.0, r.center().y - 4.0),
            Pos2::new(r.center().x, r.center().y - 8.0),
            Pos2::new(r.center().x + 6.0, r.center().y - 4.0),
            Pos2::new(r.center().x + 6.0, r.center().y + 2.0),
        ],
        Stroke::new(1.8, c),
    );
    shapes.push(Shape::Path(arch));
    let body = Rect::from_center_size(r.center() + Vec2::new(0.0, 4.0), Vec2::new(14.0, 10.0));
    shapes.push(Shape::rect_stroke(body, 2.0, Stroke::new(1.6, c)));
}

fn icon_grid(shapes: &mut Vec<Shape>, r: Rect, c: Color32) {
    let body = Rect::from_center_size(r.center(), Vec2::splat(r.width().min(r.height()) * 0.78));
    for i in 0..=2 {
        let t = i as f32 / 2.0;
        let x = body.min.x + body.width() * t;
        let y = body.min.y + body.height() * t;
        stroke(
            shapes,
            vec![Pos2::new(x, body.min.y), Pos2::new(x, body.max.y)],
            c,
            1.2,
        );
        stroke(
            shapes,
            vec![Pos2::new(body.min.x, y), Pos2::new(body.max.x, y)],
            c,
            1.2,
        );
    }
}