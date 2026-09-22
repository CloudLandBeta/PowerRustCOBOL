// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Setting a form's tab order, the two ways a RAD designer offers it.
//!
//! - **Visual Tab Order** — a designer mode. Every click on a control that
//!   takes part in the tab order gives it the next number; each such control
//!   shows its number beside it while the mode is on, and toggling the mode off
//!   writes the order to the form as one undo step.
//! - **The Tab Order list** — a window listing the same controls in order. A
//!   row is moved by dragging it or with ▲ / ▼; selecting a row selects the
//!   control on the form. Apply writes the order, Cancel leaves the form as it
//!   was.
//!
//! Both edit a list of control ids and never the form itself, so neither has
//! anything to undo until it commits. The order written is 1, 2, 3 … in list
//! order. Which controls take part is `ControlType::takes_tab_order` — the
//! same predicate the running form walks — so the numbers shown here are the
//! order Tab will follow.

use cobolt_forms::model::{Control, Form};
use eframe::egui;

use crate::i18n::Tr;
use crate::theme::Theme;

/// The form's tab-order controls, first to last, exactly as the running form
/// orders them: by `TabOrder`, and among equal numbers by the order the form is
/// painted (container before its children, siblings by z-order).
pub fn tab_sequence(form: &Form) -> Vec<String> {
    let order = cobolt_forms::containers::render_order(&form.controls);
    let mut ranked: Vec<(u32, usize, String)> = order
        .iter()
        .enumerate()
        .filter_map(|(sequence, &idx)| {
            let c = &form.controls[idx];
            c.control_type
                .takes_tab_order()
                .then(|| (c.tab_order, sequence, c.id.clone()))
        })
        .collect();
    ranked.sort();
    ranked.into_iter().map(|(_, _, id)| id).collect()
}

/// The `TabOrder` each control should end up with for `order`: its 1-based
/// position.
pub fn numbering(order: &[String]) -> Vec<(String, u32)> {
    order
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i as u32 + 1))
        .collect()
}

// ── Visual Tab Order ─────────────────────────────────────────────────────────

/// The designer's Visual Tab Order mode, while it is on.
#[derive(Clone, Debug)]
pub struct VisualTabOrder {
    /// Every tab-order control, in the order being built.
    pub order: Vec<String>,
    /// How many controls have been clicked so far; they are `order[..picked]`,
    /// in click order. The rest keep the order they had, after them.
    picked: usize,
}

impl VisualTabOrder {
    pub fn start(form: &Form) -> Self {
        Self {
            order: tab_sequence(form),
            picked: 0,
        }
    }

    /// A click on `id`: it takes the next number. Clicking a control already
    /// numbered in this session moves it to the latest number, so a slip is
    /// corrected by clicking on in the right order. A control outside the tab
    /// order is ignored. Returns whether the click counted.
    pub fn click(&mut self, id: &str) -> bool {
        let Some(pos) = self.order.iter().position(|o| o == id) else {
            return false;
        };
        let id = self.order.remove(pos);
        if pos < self.picked {
            self.picked -= 1;
        }
        self.order.insert(self.picked, id);
        self.picked += 1;
        true
    }

    /// How many controls have been clicked in this session.
    pub fn picked(&self) -> usize {
        self.picked
    }
}

// ── The number beside each control ───────────────────────────────────────────

/// Paint each control's position in `order` beside it — a small numbered badge
/// just left of the control's top-left corner. `highlight` controls (the ones a
/// Visual Tab Order session has already numbered, or the row selected in the
/// list) are drawn in the accent colour; the rest in a neutral one.
pub fn paint_numbers(
    painter: &egui::Painter,
    control_rects: &std::collections::HashMap<String, egui::Rect>,
    order: &[String],
    highlight: &dyn Fn(usize, &str) -> bool,
) {
    let theme = crate::theme::active();
    let font = egui::FontId::proportional(11.0);
    for (i, id) in order.iter().enumerate() {
        let Some(rect) = control_rects.get(id) else {
            continue;
        };
        let text = (i + 1).to_string();
        let galley = painter.layout_no_wrap(text, font.clone(), egui::Color32::WHITE);
        let w = (galley.size().x + 8.0).max(18.0);
        // Just outside the control's left edge, level with its top: inside the
        // corner it would sit under the selection handle.
        let badge = egui::Rect::from_min_size(
            egui::pos2(rect.left() - w - 3.0, rect.top()),
            egui::vec2(w, 18.0),
        );
        let fill = if highlight(i, id) {
            theme.accent
        } else {
            egui::Color32::from_rgb(90, 96, 110)
        };
        painter.rect_filled(badge, 9.0, fill);
        painter.rect_stroke(
            badge,
            9.0,
            egui::Stroke::new(1.0, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );
        painter.galley(badge.center() - galley.size() / 2.0, galley, egui::Color32::WHITE);
    }
}

// ── The Tab Order list ───────────────────────────────────────────────────────

const DEFAULT_W: f32 = 460.0;
const DEFAULT_H: f32 = 420.0;
const MIN_W: f32 = 340.0;
const MIN_H: f32 = 260.0;
const MAX_W: f32 = 1400.0;
const MAX_H: f32 = 1400.0;
const GRIP: f32 = 14.0;
const GRIP_INSET: f32 = 3.0;
const ROW_H: f32 = 24.0;
/// Title bar, hint, button row and spacing: everything that is not the rows.
const CHROME_H: f32 = 118.0;

/// What one frame of the list decided.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TabOrderOutcome {
    Open,
    /// A row was selected: select that control on the form.
    Selected(String),
    Cancelled,
    /// Write this order to the form.
    Apply(Vec<String>),
}

/// One listed control: its id and a short description to recognise it by.
#[derive(Clone, Debug)]
struct Row {
    id: String,
    kind: String,
    caption: String,
}

/// The Tab Order window. Edits its own copy of the order, so Cancel cancels.
pub struct TabOrderModal {
    rows: Vec<Row>,
    /// The row selected, by position in `rows`.
    pub selected: Option<usize>,
    /// The row being dragged, by position when the drag began.
    dragging: Option<usize>,
    /// The window's size. The grip is its only writer.
    size: egui::Vec2,
}

impl TabOrderModal {
    pub fn new(form: &Form) -> Self {
        let rows = tab_sequence(form)
            .into_iter()
            .filter_map(|id| form.find_control(&id).map(row_for))
            .collect();
        Self {
            rows,
            selected: None,
            dragging: None,
            size: egui::vec2(DEFAULT_W, DEFAULT_H),
        }
    }

    /// The order as it stands in the window.
    pub fn order(&self) -> Vec<String> {
        self.rows.iter().map(|r| r.id.clone()).collect()
    }

    /// The selected row's control, if any.
    pub fn selected_id(&self) -> Option<&str> {
        self.selected.and_then(|i| self.rows.get(i)).map(|r| r.id.as_str())
    }

    /// Select the row for `id`, when the form's selection changes under it.
    pub fn select_id(&mut self, id: Option<&str>) {
        self.selected = id.and_then(|id| self.rows.iter().position(|r| r.id == id));
    }

    /// Move the row at `from` so it ends up at position `to` (both in the list
    /// as it is before the move). The selection follows the row.
    pub fn move_row(&mut self, from: usize, to: usize) {
        if from >= self.rows.len() {
            return;
        }
        let to = to.min(self.rows.len() - 1);
        let row = self.rows.remove(from);
        self.rows.insert(to, row);
        self.selected = Some(to);
    }

    pub fn show(&mut self, ctx: &egui::Context, theme: &Theme, tr: &Tr) -> TabOrderOutcome {
        let mut outcome = TabOrderOutcome::Open;
        let mut open = true;
        let window = egui::Window::new(egui::RichText::new(tr.tab_order_title).strong())
            .id(egui::Id::new("designer_tab_order_list"))
            .open(&mut open)
            .collapsible(false)
            // Never `resizable(true)`: the size is ours and only the grip
            // changes it (see `leaderboard_modal.rs`).
            .resizable(false)
            .fixed_size(self.size)
            .default_pos([80.0, 120.0])
            .show(ctx, |ui| {
                let margin = ui.style().spacing.window_margin.sum().x;
                let stroke = 2.0 * ui.style().visuals.window_stroke.width;
                let width = self.size.x - margin - stroke;
                ui.set_width(width);
                ui.label(egui::RichText::new(tr.tab_order_hint).color(theme.text_dim));
                ui.add_space(4.0);
                let rows_h = (self.size.y - CHROME_H).max(ROW_H * 2.0);
                egui::ScrollArea::both()
                    .id_salt("tab_order_rows")
                    .max_height(rows_h)
                    .min_scrolled_height(rows_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if let Some(id) = self.rows_ui(ui, width, theme) {
                            outcome = TabOrderOutcome::Selected(id);
                        }
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let sel = self.selected;
                    let can_up = sel.is_some_and(|i| i > 0);
                    let can_down = sel.is_some_and(|i| i + 1 < self.rows.len());
                    if ui
                        .add_enabled(can_up, egui::Button::new("▲"))
                        .on_hover_text(tr.menu_move_up)
                        .clicked()
                    {
                        if let Some(i) = sel {
                            self.move_row(i, i - 1);
                        }
                    }
                    if ui
                        .add_enabled(can_down, egui::Button::new("▼"))
                        .on_hover_text(tr.menu_move_down)
                        .clicked()
                    {
                        if let Some(i) = sel {
                            self.move_row(i, i + 1);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(tr.btn_cancel).clicked() {
                            outcome = TabOrderOutcome::Cancelled;
                        }
                        if ui.button(tr.tab_order_apply).clicked() {
                            outcome = TabOrderOutcome::Apply(self.order());
                        }
                    });
                });
            });
        if let Some(window) = window {
            self.resize_grip(ctx, theme, window.response.rect);
        }
        if !open {
            outcome = TabOrderOutcome::Cancelled;
        }
        outcome
    }

    /// The rows. Returns the id of a row clicked this frame.
    fn rows_ui(&mut self, ui: &mut egui::Ui, width: f32, theme: &Theme) -> Option<String> {
        let mut clicked = None;
        let mut rects: Vec<egui::Rect> = Vec::with_capacity(self.rows.len());
        let mut drop_from: Option<usize> = None;
        for (i, row) in self.rows.iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(
                egui::vec2(width - 16.0, ROW_H),
                egui::Sense::click_and_drag(),
            );
            rects.push(rect);
            let painter = ui.painter();
            let selected = self.selected == Some(i);
            if selected {
                painter.rect_filled(rect, 4.0, theme.accent.gamma_multiply(0.35));
            } else if resp.hovered() {
                painter.rect_filled(rect, 4.0, theme.text_dim.gamma_multiply(0.15));
            }
            let text = theme.text_bright;
            painter.text(
                rect.left_center() + egui::vec2(8.0, 0.0),
                egui::Align2::LEFT_CENTER,
                format!("{:>3}", i + 1),
                egui::FontId::monospace(13.0),
                theme.accent,
            );
            let label = if row.caption.is_empty() {
                format!("{}  ({})", row.id, row.kind)
            } else {
                format!("{}  ({})  “{}”", row.id, row.kind, row.caption)
            };
            painter.text(
                rect.left_center() + egui::vec2(44.0, 0.0),
                egui::Align2::LEFT_CENTER,
                label,
                egui::FontId::proportional(13.0),
                text,
            );
            if resp.drag_started() {
                self.dragging = Some(i);
            }
            if resp.drag_stopped() {
                drop_from = self.dragging.take();
            }
            if resp.clicked() {
                self.selected = Some(i);
                clicked = Some(row.id.clone());
            }
            if resp.dragged() || resp.hovered() && self.dragging.is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }
        }
        // Where a dragged row would land: the gap nearest the pointer.
        let pointer = ui.ctx().pointer_latest_pos();
        let target = pointer.map(|p| insertion_index(&rects, p.y));
        if let (Some(_), Some(gap)) = (self.dragging, target) {
            let y = if gap < rects.len() {
                rects[gap].top()
            } else {
                rects.last().map_or(0.0, |r| r.bottom())
            };
            if let Some(first) = rects.first() {
                ui.painter().hline(
                    first.left()..=first.right(),
                    y,
                    egui::Stroke::new(2.0, theme.accent),
                );
            }
        }
        if let (Some(from), Some(gap)) = (drop_from, target) {
            // `gap` counts positions BEFORE the move; removing the row first
            // shifts every later gap up by one.
            let to = if gap > from { gap - 1 } else { gap };
            if to != from {
                self.move_row(from, to);
                clicked = self.selected_id().map(str::to_owned);
            }
        }
        clicked
    }

    /// The grip, in its own foreground area at the window's outer corner. Its
    /// drag delta is the only writer of `self.size`.
    fn resize_grip(&mut self, ctx: &egui::Context, theme: &Theme, window: egui::Rect) {
        let corner = window.max - egui::vec2(GRIP_INSET, GRIP_INSET);
        let origin = corner - egui::vec2(GRIP, GRIP);
        egui::Area::new(egui::Id::new("designer_tab_order_grip"))
            .order(egui::Order::Foreground)
            .fixed_pos(origin)
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(GRIP, GRIP), egui::Sense::drag());
                if response.dragged() {
                    self.size += response.drag_delta();
                    self.size.x = self.size.x.clamp(MIN_W, MAX_W);
                    self.size.y = self.size.y.clamp(MIN_H, MAX_H);
                }
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNorthWest);
                }
                let colour = if response.hovered() || response.dragged() {
                    theme.accent
                } else {
                    theme.text_dim
                };
                for offset in [2.0_f32, 6.0, 10.0] {
                    let len = GRIP - offset - 1.0;
                    ui.painter().line_segment(
                        [
                            egui::pos2(rect.right() - len, rect.bottom()),
                            egui::pos2(rect.right(), rect.bottom() - len),
                        ],
                        egui::Stroke::new(1.2, colour),
                    );
                }
            });
    }
}

fn row_for(c: &Control) -> Row {
    let caption = c
        .get_prop("Caption")
        .or_else(|| c.get_prop("Text"))
        .map(|v| v.as_str().lines().next().unwrap_or("").to_owned())
        .unwrap_or_default();
    Row {
        id: c.id.clone(),
        kind: c.control_type.as_str().to_owned(),
        caption,
    }
}

/// The gap a pointer at `y` points at: 0 is before the first row, `rects.len()`
/// after the last.
fn insertion_index(rects: &[egui::Rect], y: f32) -> usize {
    rects
        .iter()
        .position(|r| y < r.center().y)
        .unwrap_or(rects.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::model::ControlType;

    fn form_of(controls: &[(&str, ControlType, u32)]) -> Form {
        let mut form = Form::new("F", "F", 400, 300);
        for (i, (id, ct, tab)) in controls.iter().enumerate() {
            let mut c = Control::new(id.to_string(), ct.clone(), 0, i as i32 * 30);
            c.tab_order = *tab;
            c.z_order = i as i32;
            form.controls.push(c);
        }
        form
    }

    #[test]
    fn the_sequence_is_the_one_the_running_form_walks() {
        let form = form_of(&[
            ("Name", ControlType::TextBox, 2),
            ("Logo", ControlType::PictureBox, 0),
            ("NameLabel", ControlType::Label, 1),
            ("Save", ControlType::Button, 0),
        ]);
        // TabOrder first; equal numbers keep paint order; a PictureBox is not
        // in the tab order at all.
        assert_eq!(tab_sequence(&form), ["Save", "NameLabel", "Name"]);
    }

    #[test]
    fn clicks_number_the_controls_in_click_order_and_the_rest_follow() {
        let form = form_of(&[
            ("A", ControlType::TextBox, 1),
            ("B", ControlType::TextBox, 2),
            ("C", ControlType::TextBox, 3),
            ("D", ControlType::TextBox, 4),
        ]);
        let mut v = VisualTabOrder::start(&form);
        assert!(v.click("C"));
        assert!(v.click("A"));
        assert_eq!(v.order, ["C", "A", "B", "D"]);
        // A slip is corrected by clicking again: C moves to the latest number.
        assert!(v.click("C"));
        assert_eq!(v.order, ["A", "C", "B", "D"]);
        assert_eq!(v.picked(), 2);
        assert!(!v.click("Nowhere"), "a control outside the order is ignored");
        assert_eq!(
            numbering(&v.order),
            [("A".into(), 1), ("C".into(), 2), ("B".into(), 3), ("D".into(), 4)]
        );
    }

    #[test]
    fn moving_a_row_carries_the_selection_with_it() {
        let form = form_of(&[
            ("A", ControlType::TextBox, 1),
            ("B", ControlType::TextBox, 2),
            ("C", ControlType::TextBox, 3),
        ]);
        let mut m = TabOrderModal::new(&form);
        m.move_row(0, 2);
        assert_eq!(m.order(), ["B", "C", "A"]);
        assert_eq!(m.selected_id(), Some("A"));
        m.move_row(2, 1);
        assert_eq!(m.order(), ["B", "A", "C"]);
    }

    #[test]
    fn a_pointer_picks_the_gap_nearest_it() {
        let rects: Vec<egui::Rect> = (0..3)
            .map(|i| {
                egui::Rect::from_min_size(egui::pos2(0.0, i as f32 * 24.0), egui::vec2(100.0, 24.0))
            })
            .collect();
        assert_eq!(insertion_index(&rects, 2.0), 0);
        assert_eq!(insertion_index(&rects, 30.0), 1);
        assert_eq!(insertion_index(&rects, 500.0), 3);
    }
}
