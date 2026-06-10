// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Toolbox panel — MacPaint-style icon grid, two columns, vector-drawn icons.

use egui::{Color32, Pos2, RichText, Sense, Stroke, Ui, Vec2};
use cobolt_forms::ControlType;
use crate::i18n::Tr;

// ── Tool catalogue ─────────────────────────────────────────────────────────────

struct ToolEntry {
    label:    &'static str,   // shown as tooltip
    ct:       ControlType,
    category: &'static str,
}

const TOOLS: &[ToolEntry] = &[
    // ── Common ─────────────────────────────────────────────────────────────────
    ToolEntry { label: "Button",         ct: ControlType::Button,         category: "Common" },
    ToolEntry { label: "Label",          ct: ControlType::Label,          category: "Common" },
    ToolEntry { label: "TextBox",        ct: ControlType::TextBox,        category: "Common" },
    ToolEntry { label: "CheckBox",       ct: ControlType::CheckBox,       category: "Common" },
    ToolEntry { label: "RadioButton",    ct: ControlType::RadioButton,    category: "Common" },
    ToolEntry { label: "ComboBox",       ct: ControlType::ComboBox,       category: "Common" },
    ToolEntry { label: "ListBox",        ct: ControlType::ListBox,        category: "Common" },
    ToolEntry { label: "NumericUpDown",  ct: ControlType::NumericUpDown,  category: "Common" },
    ToolEntry { label: "DateTimePicker", ct: ControlType::DateTimePicker, category: "Common" },
    // ── Containers ─────────────────────────────────────────────────────────────
    ToolEntry { label: "GroupBox",       ct: ControlType::GroupBox,       category: "Container" },
    ToolEntry { label: "Panel",          ct: ControlType::Panel,          category: "Container" },
    ToolEntry { label: "TabControl",     ct: ControlType::TabControl,     category: "Container" },
    ToolEntry { label: "Splitter",       ct: ControlType::Splitter,       category: "Container" },
    // ── Data ───────────────────────────────────────────────────────────────────
    ToolEntry { label: "DataGrid",       ct: ControlType::DataGrid,       category: "Data" },
    ToolEntry { label: "TreeView",       ct: ControlType::TreeView,       category: "Data" },
    // ── Graphics ───────────────────────────────────────────────────────────────
    ToolEntry { label: "PictureBox",     ct: ControlType::PictureBox,     category: "Graphics" },
    ToolEntry { label: "Animator",       ct: ControlType::Animator,       category: "Graphics" },
    ToolEntry { label: "ProgressBar",    ct: ControlType::ProgressBar,    category: "Graphics" },
    ToolEntry { label: "Slider",         ct: ControlType::Slider,         category: "Graphics" },
    ToolEntry { label: "Line",           ct: ControlType::Line,           category: "Graphics" },
    ToolEntry { label: "Shape",          ct: ControlType::Shape,          category: "Graphics" },
    // ── Menus & Bars ───────────────────────────────────────────────────────────
    ToolEntry { label: "MenuBar",        ct: ControlType::MenuBar,        category: "Menu" },
    ToolEntry { label: "ToolBar",        ct: ControlType::ToolBar,        category: "Menu" },
    ToolEntry { label: "StatusBar",      ct: ControlType::StatusBar,      category: "Menu" },
    // ── Non-visual ─────────────────────────────────────────────────────────────
    ToolEntry { label: "Timer",          ct: ControlType::Timer,          category: "NonVisual" },
    ToolEntry { label: "AgentObject",    ct: ControlType::AgentObject,    category: "NonVisual" },
    ToolEntry { label: "RestClient",     ct: ControlType::RestClient,     category: "NonVisual" },
    ToolEntry { label: "SqlDatabase",    ct: ControlType::SqlDatabase,    category: "NonVisual" },
    // ── Charts ─────────────────────────────────────────────────────────────────
    ToolEntry { label: "BarChart",       ct: ControlType::BarChart,       category: "Charts" },
    ToolEntry { label: "LineChart",      ct: ControlType::LineChart,      category: "Charts" },
    ToolEntry { label: "PieChart",       ct: ControlType::PieChart,       category: "Charts" },
    ToolEntry { label: "AreaChart",      ct: ControlType::AreaChart,      category: "Charts" },
    ToolEntry { label: "ScatterChart",   ct: ControlType::ScatterChart,   category: "Charts" },
    ToolEntry { label: "DonutChart",     ct: ControlType::DonutChart,     category: "Charts" },
    // ── Dialogs ────────────────────────────────────────────────────────────────
    ToolEntry { label: "ModalWindow",    ct: ControlType::ModalWindow,    category: "Dialogs" },
];

/// The toolbox category a control type belongs to (internal key). Used by the
/// project tree to group a form's controls. Non-visual controls sort first.
pub fn category_of(ct: ControlType) -> &'static str {
    TOOLS
        .iter()
        .find(|t| t.ct == ct)
        .map(|t| t.category)
        .unwrap_or(if ct.is_non_visual() { "NonVisual" } else { "Common" })
}

/// Human label for a toolbox category key (e.g. `"NonVisual"` → `"Non-Visual"`).
pub fn category_display(key: &str) -> &'static str {
    CATEGORIES.iter().find(|(k, _)| *k == key).map(|(_, d)| *d).unwrap_or("Other")
}

/// Category display order for the project tree — **Non-Visual first**, then the
/// rest of the toolbox order.
pub const TREE_CATEGORY_ORDER: &[&str] =
    &["NonVisual", "Common", "Container", "Data", "Graphics", "Menu", "Charts", "Dialogs"];

const CATEGORIES: &[(&str, &str)] = &[
    ("Common",    "Common"),
    ("Container", "Containers"),
    ("Data",      "Data"),
    ("Graphics",  "Graphics"),
    ("Menu",      "Menus & Bars"),
    ("NonVisual", "Non-Visual"),
    ("Charts",    "Charts"),
    ("Dialogs",   "Dialogs"),
];

// ── Sizes ──────────────────────────────────────────────────────────────────────

/// Side length of each icon button (+25 % over the previous 39 px).
const BTN: f32 = 49.0;
/// Gap between icon frames (horizontal and vertical).
const GAP: f32 = 4.0;
/// Extra padding on the top and left of every button (+5 px over the previous 5 px).
const BTN_PAD_TOP:   f32 = 10.0;
const BTN_PAD_RIGHT: f32 = 10.0;

// ── Public types ───────────────────────────────────────────────────────────────

pub struct ToolboxAction {
    pub dragged_type: Option<ControlType>,
}

pub struct ToolboxPanel {
    filter:    String,
    collapsed: std::collections::HashSet<String>,
}

impl ToolboxPanel {
    pub fn new() -> Self {
        Self {
            filter:    String::new(),
            collapsed: std::collections::HashSet::new(),
        }
    }

    pub fn show(&mut self, ui: &mut Ui, tr: &Tr) -> ToolboxAction {
        let mut action = ToolboxAction { dragged_type: None };

        ui.vertical(|ui| {
            ui.heading("Toolbox");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.text_edit_singleline(&mut self.filter);
                if !self.filter.is_empty() && ui.small_button("✕").clicked() {
                    self.filter.clear();
                }
            });
            ui.add_space(4.0);

            let filter_lo = self.filter.to_ascii_lowercase();

            egui::ScrollArea::vertical()
                .id_salt("toolbox_scroll")
                .show(ui, |ui| {
                    if filter_lo.is_empty() {
                        for &(cat_id, _) in CATEGORIES {
                            let cat_label = tr.category_name(cat_id);
                            let tools_in_cat: Vec<&ToolEntry> = TOOLS.iter()
                                .filter(|e| e.category == cat_id)
                                .collect();
                            if tools_in_cat.is_empty() { continue; }

                            let collapsed = self.collapsed.contains(cat_id);
                            let arrow = if collapsed { "▸ " } else { "▾ " };

                            let hdr = egui::Button::new(
                                RichText::new(format!("{arrow}{cat_label}"))
                                    .small()
                                    .strong()
                                    .color(Color32::from_rgb(150, 180, 255)),
                            )
                            .frame(false)
                            .min_size(Vec2::new(ui.available_width(), 16.0));

                            if ui.add(hdr).clicked() {
                                if collapsed {
                                    self.collapsed.remove(cat_id);
                                } else {
                                    self.collapsed.insert(cat_id.to_owned());
                                }
                            }

                            if !collapsed {
                                ui.add_space(1.0);
                                let sep_rect = ui.available_rect_before_wrap();
                                ui.painter().line_segment(
                                    [sep_rect.left_top(), Pos2::new(sep_rect.right(), sep_rect.top())],
                                    Stroke::new(1.0, Color32::from_rgba_premultiplied(100, 120, 180, 60)),
                                );
                                ui.add_space(4.0);   // ← 4 px top gap before first icon row

                                render_icon_grid(ui, &tools_in_cat, &mut action);
                                ui.add_space(4.0);
                            }
                        }
                    } else {
                        let filtered: Vec<&ToolEntry> = TOOLS.iter()
                            .filter(|e| e.label.to_ascii_lowercase().contains(&filter_lo))
                            .collect();
                        render_icon_grid(ui, &filtered, &mut action);
                    }
                });
        });

        action
    }
}

// ── Grid renderer ──────────────────────────────────────────────────────────────

fn render_icon_grid(ui: &mut Ui, entries: &[&ToolEntry], action: &mut ToolboxAction) {
    // Two square buttons per row, each padded BTN_PAD_TOP on top and BTN_PAD_RIGHT
    // on the right.  The allocated cell size is (BTN + BTN_PAD_RIGHT) × (BTN + BTN_PAD_TOP).
    // Layout per row:  [left_pad]  [cell]  [GAP]  [cell]
    // left_pad centres the pair in the available width.
    let cell_w  = BTN + BTN_PAD_RIGHT;
    let row_w   = cell_w * 2.0 + GAP;
    let avail   = ui.available_width();
    let padding = ((avail - row_w) / 2.0).max(0.0);

    // Vertical gap between rows (the top-pad is baked into each cell allocation).
    ui.spacing_mut().item_spacing.y = GAP;

    let mut i = 0;
    while i < entries.len() {
        let left  = entries[i];
        let right = entries.get(i + 1).copied();

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            ui.add_space(padding);

            if let Some(ct) = icon_btn(ui, left) {
                action.dragged_type = Some(ct);
            }
            if let Some(right) = right {
                if let Some(ct) = icon_btn(ui, right) {
                    action.dragged_type = Some(ct);
                }
            }
        });

        i += 2;
    }
}

// ── Single icon button ─────────────────────────────────────────────────────────

fn icon_btn(ui: &mut Ui, entry: &ToolEntry) -> Option<ControlType> {
    // Allocate a cell that is BTN_PAD_RIGHT wider (right padding) and BTN_PAD_TOP
    // taller (top padding).  The visible button lives in the bottom-left BTN×BTN
    // portion of the cell, leaving the padding areas empty.
    let cell_size = Vec2::new(BTN + BTN_PAD_RIGHT, BTN + BTN_PAD_TOP);
    let (cell, resp) = ui.allocate_exact_size(cell_size, Sense::click_and_drag());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    // Inner button rect: offset down by top-pad, keep left edge, width/height = BTN.
    let rect = egui::Rect::from_min_size(
        egui::Pos2::new(cell.min.x, cell.min.y + BTN_PAD_TOP),
        Vec2::splat(BTN),
    );

    // Confine hover/press state to the inner button rect, not the full padded cell.
    let pointer_in_btn = ui.ctx().input(|i| {
        i.pointer.latest_pos().map(|p| rect.contains(p)).unwrap_or(false)
    });
    let hovered  = resp.hovered() && pointer_in_btn;
    let pressed  = resp.is_pointer_button_down_on() && pointer_in_btn;
    let dragging = resp.dragged();

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&resp);

        let bg = if pressed || dragging {
            Color32::from_rgba_premultiplied(80, 120, 220, 100)
        } else if hovered {
            Color32::from_rgba_premultiplied(80, 120, 220, 45)
        } else {
            Color32::TRANSPARENT
        };

        let border_color = if pressed || dragging {
            Color32::from_rgba_premultiplied(120, 160, 255, 200)
        } else if hovered {
            Color32::from_rgba_premultiplied(120, 160, 255, 100)
        } else {
            Color32::from_rgba_premultiplied(120, 120, 140, 25)
        };

        let rounding = visuals.rounding;
        ui.painter().rect_filled(rect, rounding, bg);
        ui.painter().rect_stroke(rect, rounding, Stroke::new(1.0, border_color));

        let icon_color = if pressed || dragging {
            Color32::WHITE
        } else if hovered {
            Color32::from_rgb(200, 220, 255)
        } else {
            Color32::from_rgb(210, 210, 220)
        };

        paint_control_icon(ui.painter(), rect, entry.ct.clone(), icon_color);
    }

    if hovered || dragging {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }

    let clicked      = resp.clicked()      && pointer_in_btn;
    let drag_started = resp.drag_started() && pointer_in_btn;
    if pointer_in_btn {
        let tooltip = match entry.ct {
            ControlType::Animator => "PlayGIF/WebP/APNG files",
            _ => entry.label,
        };
        resp.on_hover_text(tooltip);
    }

    if clicked || drag_started {
        Some(entry.ct.clone())
    } else {
        None
    }
}

// ── Vector icon painter ────────────────────────────────────────────────────────

/// Draw a miniature vector icon centred inside `rect`.
/// `r` is the icon scaling unit = 25 % of the button's logical size, giving ~3.25 px
/// for a 26 px button — ensures all icons stay comfortably inside the frame.
fn paint_control_icon(painter: &egui::Painter, rect: egui::Rect, ct: ControlType, color: Color32) {
    let c   = rect.center();
    let r   = rect.size().min_elem() * 0.25;   // ≈ 6.5 px for a 26 px button → max span ~18 px
    let s   = Stroke::new(1.2, color);
    let th  = Stroke::new(0.8, color);
    let dim = Color32::from_rgba_premultiplied(
        (color.r() as f32 * 0.38) as u8,
        (color.g() as f32 * 0.38) as u8,
        (color.b() as f32 * 0.38) as u8,
        (color.a() as f32 * 0.38) as u8,
    );

    match ct {
        // ── Common ─────────────────────────────────────────────────────────────
        ControlType::Button => {
            let b = egui::Rect::from_center_size(c, Vec2::new(r*2.8, r*1.6));
            painter.rect_stroke(b, r*0.4, s);
            painter.line_segment(
                [Pos2::new(c.x-r*0.5, c.y), Pos2::new(c.x+r*0.5, c.y)],
                Stroke::new(1.0, dim));
        }
        ControlType::Label => {
            painter.text(c, egui::Align2::CENTER_CENTER, "A",
                egui::FontId::proportional(r*2.2), color);
        }
        ControlType::TextBox => {
            let b = egui::Rect::from_center_size(c, Vec2::new(r*2.8, r*1.4));
            painter.rect_stroke(b, 1.2, s);
            let ix = b.min.x + b.width()*0.20;
            let (iy0, iy1) = (c.y - r*0.38, c.y + r*0.38);
            painter.line_segment([Pos2::new(ix, iy0), Pos2::new(ix, iy1)], s);
            painter.line_segment([Pos2::new(ix-r*0.10, iy0), Pos2::new(ix+r*0.10, iy0)], th);
            painter.line_segment([Pos2::new(ix-r*0.10, iy1), Pos2::new(ix+r*0.10, iy1)], th);
        }
        ControlType::CheckBox => {
            let bsz = r*1.2;
            let bc  = Pos2::new(c.x - r*0.75, c.y);
            let b   = egui::Rect::from_center_size(bc, Vec2::splat(bsz));
            painter.rect_stroke(b, 1.2, s);
            painter.line_segment([Pos2::new(b.min.x+bsz*0.18, b.center().y),
                                   Pos2::new(b.min.x+bsz*0.42, b.max.y -bsz*0.18)], s);
            painter.line_segment([Pos2::new(b.min.x+bsz*0.42, b.max.y-bsz*0.18),
                                   Pos2::new(b.max.x-bsz*0.12, b.min.y+bsz*0.18)], s);
            painter.line_segment([Pos2::new(c.x-r*0.05, c.y), Pos2::new(c.x+r*0.95, c.y)], th);
        }
        ControlType::RadioButton => {
            let rc = Pos2::new(c.x - r*0.75, c.y);
            painter.circle_stroke(rc, r*0.62, s);
            painter.circle_filled(rc, r*0.27, color);
            painter.line_segment([Pos2::new(c.x-r*0.05, c.y), Pos2::new(c.x+r*0.95, c.y)], th);
        }
        ControlType::ComboBox => {
            let b    = egui::Rect::from_center_size(c, Vec2::new(r*2.8, r*1.4));
            painter.rect_stroke(b, 1.2, s);
            let divx = b.max.x - r*0.75;
            painter.line_segment([Pos2::new(divx, b.min.y+1.0), Pos2::new(divx, b.max.y-1.0)], th);
            let ac   = Pos2::new((divx+b.max.x)*0.5, c.y);
            painter.line_segment([Pos2::new(ac.x-r*0.18, ac.y-r*0.13), Pos2::new(ac.x, ac.y+r*0.13)], s);
            painter.line_segment([Pos2::new(ac.x, ac.y+r*0.13), Pos2::new(ac.x+r*0.18, ac.y-r*0.13)], s);
        }
        ControlType::ListBox => {
            let b  = egui::Rect::from_center_size(c, Vec2::new(r*2.6, r*2.4));
            painter.rect_stroke(b, 1.2, s);
            let rh = b.height()/4.0;
            painter.rect_filled(egui::Rect::from_min_size(b.min, Vec2::new(b.width(), rh)), 0.0, dim);
            for i in 1..4 {
                let y = b.min.y + rh * i as f32;
                painter.line_segment([Pos2::new(b.min.x, y), Pos2::new(b.max.x, y)], th);
            }
        }
        ControlType::NumericUpDown => {
            let b   = egui::Rect::from_center_size(c, Vec2::new(r*2.6, r*1.4));
            painter.rect_stroke(b, 1.2, s);
            let dvx = b.max.x - r*0.78;
            let midy= b.center().y;
            painter.line_segment([Pos2::new(dvx, b.min.y), Pos2::new(dvx, b.max.y)], th);
            painter.line_segment([Pos2::new(dvx, midy), Pos2::new(b.max.x, midy)], th);
            let ax  = (dvx+b.max.x)*0.5;
            painter.line_segment([Pos2::new(ax-r*0.15, midy-r*0.07), Pos2::new(ax, b.min.y+r*0.20)], th);
            painter.line_segment([Pos2::new(ax+r*0.15, midy-r*0.07), Pos2::new(ax, b.min.y+r*0.20)], th);
            painter.line_segment([Pos2::new(ax-r*0.15, midy+r*0.07), Pos2::new(ax, b.max.y-r*0.20)], th);
            painter.line_segment([Pos2::new(ax+r*0.15, midy+r*0.07), Pos2::new(ax, b.max.y-r*0.20)], th);
        }
        ControlType::DateTimePicker => {
            let b   = egui::Rect::from_center_size(c, Vec2::new(r*2.4, r*2.2));
            painter.rect_stroke(b, 1.2, s);
            let hry = b.min.y + b.height()*0.28;
            painter.line_segment([Pos2::new(b.min.x, hry), Pos2::new(b.max.x, hry)], th);
            for bx in [b.min.x+b.width()*0.28, b.min.x+b.width()*0.72] {
                painter.line_segment([Pos2::new(bx, b.min.y-1.0), Pos2::new(bx, b.min.y+2.5)], s);
            }
            for row in 0..2 {
                for col in 0..3 {
                    let dx = b.min.x + b.width()*(0.18+col as f32*0.28);
                    let dy = hry + (b.max.y-hry)*(0.28+row as f32*0.42);
                    painter.circle_filled(Pos2::new(dx, dy), 1.2, color);
                }
            }
        }

        // ── Containers ─────────────────────────────────────────────────────────
        ControlType::GroupBox => {
            let b  = egui::Rect::from_center_size(c, Vec2::new(r*2.8, r*2.4));
            let g0 = b.min.x + r*0.3;
            let g1 = b.min.x + r*1.4;
            painter.line_segment([b.min, Pos2::new(g0, b.min.y)], s);
            painter.line_segment([Pos2::new(g1, b.min.y), Pos2::new(b.max.x, b.min.y)], s);
            painter.line_segment([b.min, b.left_bottom()], s);
            painter.line_segment([b.right_top(), b.max], s);
            painter.line_segment([b.left_bottom(), b.max], s);
            painter.text(Pos2::new((g0+g1)*0.5, b.min.y), egui::Align2::CENTER_CENTER,
                "G", egui::FontId::proportional(r*0.85), color);
        }
        ControlType::Panel => {
            let shd = egui::Rect::from_center_size(c+Vec2::new(1.5,1.5), Vec2::new(r*2.6,r*2.2));
            painter.rect_stroke(shd, 0.0, Stroke::new(0.8, dim));
            let fr  = egui::Rect::from_center_size(c, Vec2::new(r*2.6, r*2.2));
            painter.rect_filled(fr, 0.0, Color32::from_rgba_premultiplied(
                (color.r() as f32*0.12) as u8, (color.g() as f32*0.12) as u8,
                (color.b() as f32*0.12) as u8, (color.a() as f32*0.12) as u8));
            painter.rect_stroke(fr, 0.0, s);
        }
        ControlType::TabControl => {
            let body = egui::Rect::from_center_size(c+Vec2::new(0.0,r*0.3), Vec2::new(r*2.8,r*1.8));
            painter.rect_stroke(body, 2.0, s);
            let tab  = egui::Rect::from_min_size(
                Pos2::new(body.min.x, body.min.y-r*0.7), Vec2::new(r*1.2, r*0.7));
            painter.line_segment([Pos2::new(tab.min.x,tab.max.y), tab.min], s);
            painter.line_segment([tab.min, Pos2::new(tab.max.x,tab.min.y)], s);
            painter.line_segment([Pos2::new(tab.max.x,tab.min.y), Pos2::new(tab.max.x,tab.max.y)], s);
            let t2min = Pos2::new(tab.max.x+1.5, body.min.y-r*0.55);
            painter.line_segment([Pos2::new(t2min.x,body.min.y), t2min], th);
            painter.line_segment([t2min, Pos2::new(t2min.x+r*0.9,t2min.y)], th);
            painter.line_segment([Pos2::new(t2min.x+r*0.9,t2min.y), Pos2::new(t2min.x+r*0.9,body.min.y)], th);
        }
        ControlType::Splitter => {
            let b = egui::Rect::from_center_size(c, Vec2::new(r*2.8, r*2.2));
            painter.rect_stroke(b, 0.0, th);
            painter.line_segment([Pos2::new(c.x, b.min.y+2.5), Pos2::new(c.x, b.max.y-2.5)], s);
            for i in 0i32..3 {
                painter.circle_filled(Pos2::new(c.x, c.y+(i-1) as f32*r*0.42), 1.2, color);
            }
        }

        // ── Data ───────────────────────────────────────────────────────────────
        ControlType::DataGrid => {
            let b  = egui::Rect::from_center_size(c, Vec2::new(r*2.6, r*2.4));
            painter.rect_stroke(b, 0.0, s);
            let hh = b.height()*0.28;
            painter.rect_filled(egui::Rect::from_min_size(b.min, Vec2::new(b.width(), hh)), 0.0, dim);
            for i in 1..3 {
                let x = b.min.x + b.width()*i as f32/3.0;
                painter.line_segment([Pos2::new(x,b.min.y), Pos2::new(x,b.max.y)], th);
            }
            for i in 1..3 {
                let y = b.min.y + hh + (b.height()-hh)*i as f32/3.0;
                painter.line_segment([Pos2::new(b.min.x,y), Pos2::new(b.max.x,y)], th);
            }
        }
        ControlType::TreeView => {
            let (ox, oy) = (c.x-r*0.8, c.y-r*0.85);
            painter.circle_filled(Pos2::new(ox,oy), 2.2, color);
            let trunk_b  = c.y + r*0.65;
            painter.line_segment([Pos2::new(ox,oy+2.2), Pos2::new(ox,trunk_b)], th);
            let children: &[(f32,f32)] = &[(c.x+r*0.2, c.y-r*0.3), (c.x+r*0.2, c.y+r*0.45)];
            for &(cx,cy) in children {
                painter.line_segment([Pos2::new(ox,cy), Pos2::new(cx-2.2,cy)], th);
                painter.circle_filled(Pos2::new(cx,cy), 1.8, color);
            }
            let gc = (c.x+r*0.95, c.y+r*0.08);
            painter.line_segment([Pos2::new(children[0].0,children[0].1+1.8), Pos2::new(children[0].0,gc.1)], th);
            painter.line_segment([Pos2::new(children[0].0,gc.1), Pos2::new(gc.0-1.8,gc.1)], th);
            painter.circle_filled(Pos2::new(gc.0,gc.1), 1.3, color);
        }

        // ── Graphics ───────────────────────────────────────────────────────────
        ControlType::PictureBox => {
            let b  = egui::Rect::from_center_size(c, Vec2::new(r*2.6, r*2.2));
            painter.rect_stroke(b, 0.0, s);
            painter.circle_stroke(
                Pos2::new(b.min.x+b.width()*0.28, b.min.y+b.height()*0.28), r*0.30, th);
            let my  = b.min.y + b.height()*0.60;
            painter.line_segment([Pos2::new(b.min.x,b.max.y), Pos2::new(b.min.x+b.width()*0.38,my)], s);
            painter.line_segment([Pos2::new(b.min.x+b.width()*0.38,my), Pos2::new(b.min.x+b.width()*0.62,b.max.y)], s);
            let my2 = b.min.y + b.height()*0.46;
            painter.line_segment([Pos2::new(b.min.x+b.width()*0.50,b.max.y), Pos2::new(b.min.x+b.width()*0.76,my2)], s);
            painter.line_segment([Pos2::new(b.min.x+b.width()*0.76,my2), Pos2::new(b.max.x,b.max.y)], s);
        }
        ControlType::Animator => {
            // Film/play motif: a frame with sprocket ticks + a centred play triangle.
            let b = egui::Rect::from_center_size(c, Vec2::new(r*2.8, r*2.0));
            painter.rect_stroke(b, r*0.25, s);
            for i in 0..3 {
                let sx = b.min.x + b.width()*(0.25 + i as f32*0.25);
                painter.line_segment([Pos2::new(sx, b.min.y), Pos2::new(sx, b.min.y + b.height()*0.16)], th);
                painter.line_segment([Pos2::new(sx, b.max.y), Pos2::new(sx, b.max.y - b.height()*0.16)], th);
            }
            let t = r*0.55;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(c.x - t*0.6, c.y - t),
                    Pos2::new(c.x - t*0.6, c.y + t),
                    Pos2::new(c.x + t*0.9, c.y),
                ],
                color,
                Stroke::NONE,
            ));
        }
        ControlType::ProgressBar => {
            let b    = egui::Rect::from_center_size(c, Vec2::new(r*2.8, r*0.95));
            painter.rect_stroke(b, 2.0, s);
            let fill = egui::Rect::from_min_size(b.min, Vec2::new(b.width()*0.62, b.height()));
            painter.rect_filled(fill, 2.0, dim);
            for i in 0..3 {
                let sx = fill.min.x + fill.width()*(0.20+i as f32*0.28);
                painter.line_segment([Pos2::new(sx,fill.min.y), Pos2::new(sx-fill.height()*0.5,fill.max.y)], th);
            }
        }
        ControlType::Slider => {
            let (tl, tr) = (c.x-r*1.2, c.x+r*1.2);
            let thumb_x  = c.x + r*0.25;
            painter.line_segment([Pos2::new(tl,c.y), Pos2::new(tr,c.y)], s);
            painter.circle_stroke(Pos2::new(thumb_x,c.y), r*0.42, s);
            painter.circle_filled(Pos2::new(thumb_x,c.y), r*0.20, color);
            for i in 0..5 {
                let tx = tl + (tr-tl)*i as f32/4.0;
                painter.line_segment([Pos2::new(tx,c.y+2.5), Pos2::new(tx,c.y+4.5)], th);
            }
        }
        ControlType::Line => {
            painter.line_segment([
                Pos2::new(c.x-r*1.1, c.y+r*0.70),
                Pos2::new(c.x+r*1.1, c.y-r*0.70),
            ], Stroke::new(1.8, color));
        }
        ControlType::Shape => {
            let pts = [
                Pos2::new(c.x,        c.y-r*1.15),
                Pos2::new(c.x+r*0.90, c.y),
                Pos2::new(c.x,        c.y+r*1.15),
                Pos2::new(c.x-r*0.90, c.y),
            ];
            for i in 0..4 { painter.line_segment([pts[i], pts[(i+1)%4]], s); }
        }

        // ── Menus & Bars ───────────────────────────────────────────────────────
        ControlType::MenuBar => {
            for i in 0i32..3 {
                let y = c.y + (i-1) as f32*r*0.62;
                painter.line_segment([Pos2::new(c.x-r*1.1,y), Pos2::new(c.x+r*1.1,y)], s);
            }
        }
        ControlType::ToolBar => {
            // 4 mini-buttons centred inside the BTN×BTN frame.
            // bw, gap chosen so total width = 4*bw + 3*gap ≤ BTN − 4px margin.
            let bw  = r * 0.58;   // button width
            let bh  = r * 0.78;   // button height
            let gap = r * 0.34;   // gap between buttons
            let total_w = 4.0 * bw + 3.0 * gap;
            let start_x = c.x - total_w * 0.5;
            for i in 0..4i32 {
                let bx = start_x + i as f32 * (bw + gap);
                let br = egui::Rect::from_min_size(
                    Pos2::new(bx, c.y - bh * 0.5), Vec2::new(bw, bh));
                painter.rect_filled(br, 1.0, dim);
                painter.rect_stroke(br, 1.0, th);
            }
        }
        ControlType::StatusBar => {
            let b = egui::Rect::from_center_size(c, Vec2::new(r*2.8, r*0.85));
            painter.rect_filled(b, 0.0, dim);
            painter.rect_stroke(b, 0.0, s);
            for i in 1..3 {
                let x = b.min.x + b.width()*i as f32/3.0;
                painter.line_segment([Pos2::new(x,b.min.y), Pos2::new(x,b.max.y)], th);
            }
        }

        // ── Non-visual ─────────────────────────────────────────────────────────
        ControlType::Timer => {
            painter.circle_stroke(c, r*1.02, s);
            painter.line_segment([Pos2::new(c.x,c.y-r*0.82), Pos2::new(c.x,c.y-r*1.02)], th);
            painter.line_segment([c, Pos2::new(c.x+r*0.36, c.y-r*0.58)], s);
            painter.line_segment([c, Pos2::new(c.x+r*0.10, c.y-r*0.42)], Stroke::new(1.8, color));
            painter.circle_filled(c, 1.3, color);
            painter.line_segment([Pos2::new(c.x-r*0.16,c.y-r*1.02), Pos2::new(c.x+r*0.16,c.y-r*1.02)], s);
        }
        ControlType::AgentObject => {
            let hc   = c + Vec2::new(0.0, r*0.14);
            let head = egui::Rect::from_center_size(hc, Vec2::new(r*1.75, r*1.28));
            painter.rect_stroke(head, r*0.28, s);
            let ey   = head.center().y - r*0.08;
            painter.circle_stroke(Pos2::new(hc.x-r*0.36, ey), r*0.20, th);
            painter.circle_stroke(Pos2::new(hc.x+r*0.36, ey), r*0.20, th);
            painter.line_segment([
                Pos2::new(hc.x-r*0.35, head.center().y+r*0.28),
                Pos2::new(hc.x+r*0.35, head.center().y+r*0.28),
            ], th);
            painter.line_segment([Pos2::new(c.x,head.min.y), Pos2::new(c.x,c.y-r*1.08)], th);
            painter.circle_filled(Pos2::new(c.x, c.y-r*1.12), 1.8, color);
        }
        ControlType::RestClient => {
            painter.circle_stroke(c, r*1.02, s);
            painter.line_segment([Pos2::new(c.x-r*1.02,c.y), Pos2::new(c.x+r*1.02,c.y)], th);
            painter.line_segment([Pos2::new(c.x,c.y-r*1.02), Pos2::new(c.x,c.y+r*1.02)], th);
            let lw = r*0.52;
            painter.line_segment([Pos2::new(c.x-lw,c.y-r*0.85), Pos2::new(c.x-lw*0.3,c.y-r*1.0)], th);
            painter.line_segment([Pos2::new(c.x-lw,c.y+r*0.85), Pos2::new(c.x-lw*0.3,c.y+r*1.0)], th);
            painter.line_segment([Pos2::new(c.x+lw,c.y-r*0.85), Pos2::new(c.x+lw*0.3,c.y-r*1.0)], th);
            painter.line_segment([Pos2::new(c.x+lw,c.y+r*0.85), Pos2::new(c.x+lw*0.3,c.y+r*1.0)], th);
        }

        // ── Dialogs ────────────────────────────────────────────────────────────
        ControlType::ModalWindow => {
            let win = egui::Rect::from_center_size(c+Vec2::new(0.0,r*0.14), Vec2::new(r*2.6,r*2.2));
            painter.rect_stroke(win, 2.0, s);
            let tbh = (win.height()*0.30).max(6.0);
            let tb  = egui::Rect::from_min_size(win.min, Vec2::new(win.width(), tbh));
            painter.rect_filled(tb, 2.0, dim);
            let bc  = Pos2::new(tb.max.x-tbh*0.52, tb.center().y);
            painter.circle_stroke(bc, tbh*0.30, th);
            let dx  = tbh*0.16;
            painter.line_segment([Pos2::new(bc.x-dx,bc.y-dx), Pos2::new(bc.x+dx,bc.y+dx)], th);
            painter.line_segment([Pos2::new(bc.x+dx,bc.y-dx), Pos2::new(bc.x-dx,bc.y+dx)], th);
            let by0 = tb.max.y + 1.5;
            for i in 0..2 {
                let ly = by0 + (win.max.y-by0)*(0.3+i as f32*0.38);
                painter.line_segment([Pos2::new(win.min.x+2.5,ly), Pos2::new(win.max.x-2.5,ly)], th);
            }
        }

        // ── Charts ─────────────────────────────────────────────────────────────
        ControlType::BarChart => {
            // 4 vertical bars of increasing height
            let base_y = c.y + r * 1.1;
            let bar_w  = r * 0.45;
            let heights = [r*0.55, r*1.0, r*0.75, r*1.3];
            let total_w = bar_w * 4.0 + r * 0.2 * 3.0;
            let mut bx  = c.x - total_w * 0.5;
            for h in &heights {
                let br = egui::Rect::from_min_size(
                    Pos2::new(bx, base_y - h), Vec2::new(bar_w, *h));
                painter.rect_filled(br, 1.0, dim);
                painter.rect_stroke(br, 1.0, s);
                bx += bar_w + r * 0.2;
            }
            painter.line_segment([Pos2::new(c.x - total_w*0.55, base_y),
                                   Pos2::new(c.x + total_w*0.55, base_y)], th);
        }
        ControlType::LineChart => {
            let pts: &[(f32, f32)] = &[(-1.1, 0.5), (-0.55, -0.2), (0.0, 0.8),
                                       (0.55, -0.6), (1.1, 0.1)];
            let map = |&(px, py): &(f32, f32)| Pos2::new(c.x + px*r, c.y + py*r);
            for w in pts.windows(2) {
                painter.line_segment([map(&w[0]), map(&w[1])], s);
            }
            for &pt in pts {
                painter.circle_filled(map(&pt), 1.8, color);
            }
            // x-axis
            painter.line_segment([Pos2::new(c.x - r*1.2, c.y + r*1.0),
                                   Pos2::new(c.x + r*1.2, c.y + r*1.0)], th);
        }
        ControlType::PieChart => {
            // 4 pie sectors using filled circles + white "cut" lines
            let rad = r * 1.05;
            painter.circle_stroke(c, rad, s);
            // Draw 4 radial lines at 0°, 110°, 210°, 290° (unequal slices)
            for angle_deg in [0.0_f32, 110.0, 210.0, 290.0] {
                let a = angle_deg.to_radians();
                painter.line_segment([c, Pos2::new(c.x + a.cos()*rad, c.y + a.sin()*rad)], th);
            }
            // shade one slice
            let a0 = 0.0_f32.to_radians();
            let a1 = 110.0_f32.to_radians();
            let mid = (a0 + a1) / 2.0;
            let mr  = rad * 0.55;
            painter.circle_filled(Pos2::new(c.x + mid.cos()*mr, c.y + mid.sin()*mr), r*0.3, dim);
        }
        ControlType::AreaChart => {
            let pts: &[(f32, f32)] = &[(-1.1, 0.4), (-0.55, -0.3), (0.0, 0.6),
                                       (0.55, -0.5), (1.1, 0.2)];
            let base_y = c.y + r * 1.0;
            let map    = |&(px, py): &(f32, f32)| Pos2::new(c.x + px*r, c.y + py*r);
            // Filled area (triangle fan approximation)
            for w in pts.windows(2) {
                let p0 = map(&w[0]);
                let p1 = map(&w[1]);
                let mesh = egui::epaint::Mesh::default();
                let _ = mesh; // draw 2-tri quads
                painter.line_segment([p0, Pos2::new(p0.x, base_y)], Stroke::new(0.6, dim));
                painter.line_segment([p0, p1], s);
            }
            painter.line_segment([Pos2::new(c.x - r*1.2, base_y),
                                   Pos2::new(c.x + r*1.2, base_y)], th);
        }
        ControlType::ScatterChart => {
            // Scattered dots of 3 sizes
            let pts: &[(f32, f32, f32)] = &[
                (-0.8,  0.3, 2.8), (-0.3, -0.7, 1.8), ( 0.1,  0.6, 3.5),
                ( 0.6,  0.0, 2.0), ( 1.0, -0.4, 2.5), (-0.5,  0.9, 1.5),
                ( 0.3, -0.2, 4.0),
            ];
            for &(px, py, pr) in pts {
                painter.circle_stroke(Pos2::new(c.x + px*r, c.y + py*r), pr, s);
            }
            // axes
            painter.line_segment([Pos2::new(c.x - r*1.2, c.y + r*1.0),
                                   Pos2::new(c.x + r*1.2, c.y + r*1.0)], th);
            painter.line_segment([Pos2::new(c.x - r*1.2, c.y - r*1.1),
                                   Pos2::new(c.x - r*1.2, c.y + r*1.0)], th);
        }
        ControlType::DonutChart => {
            let outer = r * 1.05;
            let inner = r * 0.52;
            painter.circle_stroke(c, outer, s);
            painter.circle_stroke(c, inner, th);
            // Radial cuts
            for angle_deg in [0.0_f32, 120.0, 240.0] {
                let a = angle_deg.to_radians();
                let p_i = Pos2::new(c.x + a.cos()*inner, c.y + a.sin()*inner);
                let p_o = Pos2::new(c.x + a.cos()*outer, c.y + a.sin()*outer);
                painter.line_segment([p_i, p_o], th);
            }
            // shade middle ring segment
            let mid = (60.0_f32).to_radians();
            let mr  = (inner + outer) * 0.5;
            painter.circle_filled(Pos2::new(c.x + mid.cos()*mr, c.y + mid.sin()*mr), r*0.22, dim);
        }

        _ => {
            painter.rect_stroke(
                egui::Rect::from_center_size(c, Vec2::new(r*2.0, r*1.6)), 2.0, s);
        }
    }
}
