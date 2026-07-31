// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The modal that shows Grace's rewrite of the request before the workflow
//! runs: the developer's own words on top, the revision below and editable,
//! and the passages that still read two ways marked in place.
//!
//! Sizing rule, from the operator and from every resize regression before it:
//! **the window's size comes from state, never from its content.** The editor
//! opens at ten rows and only the developer's drag on its grip changes that,
//! bounded at fifteen; the window's height is computed from that number. No
//! branch here reads `available_size`, so there is no path by which the window
//! can grow itself (operator, 2026-07-31).

use crate::i18n::Tr;
use crate::prompt_polish::{locate, Note};

/// Editor size in text rows: where it opens, and how far the grip may take it.
pub const MIN_ROWS: f32 = 10.0;
pub const MAX_ROWS: f32 = 15.0;
/// The image column, as a share of the window's width.
const GRACE_SHARE: f32 = 0.20;
const WINDOW_W: f32 = 760.0;
/// Rows of the read-only "your request" block. Fixed: it is for comparison,
/// not for reading a novel, and a content-sized block would size the window.
const ORIGINAL_ROWS: f32 = 3.0;

/// What the developer decided.
pub enum ReviewAction {
    /// Run the workflow with this text — the revision as it now stands.
    Submit(String),
    /// Close and go back to the prompt box, unchanged.
    Cancel,
}

/// The modal's own state. `rows` is user-authoritative.
pub struct PromptReview {
    pub original: String,
    pub revised: String,
    pub notes: Vec<Note>,
    rows: f32,
    /// Set when the modal opens, so it appears centred on the RAD window and
    /// can then be dragged anywhere.
    center_next_frame: bool,
}

impl PromptReview {
    pub fn new(original: String, revised: String, notes: Vec<Note>) -> Self {
        Self {
            original,
            revised,
            notes,
            rows: MIN_ROWS,
            center_next_frame: true,
        }
    }
}

/// The window's size, from `rows` and the row height alone.
///
/// This signature IS the guarantee the operator asked for: there is no
/// argument here through which the content could speak. The window cannot
/// grow because a text got longer, because Grace flagged more passages, or
/// because a wrapped line appeared — only because the developer dragged the
/// grip, which is the only writer of `rows`.
pub fn window_size(row_height: f32, rows: f32) -> egui::Vec2 {
    let rows = rows.clamp(MIN_ROWS, MAX_ROWS);
    let editor_h = rows * row_height + 8.0;
    let original_h = ORIGINAL_ROWS * row_height + 8.0;
    egui::vec2(
        WINDOW_W,
        34.0 + original_h + 26.0 + editor_h + 24.0 + 38.0 + 20.0,
    )
}

/// Draw the modal. Returns the developer's decision on the frame they make it.
pub fn show(ctx: &egui::Context, tr: &Tr, state: &mut PromptReview) -> Option<ReviewAction> {
    let row = ctx.style_of(ctx.theme()).text_styles[&egui::TextStyle::Body].size * 1.35;
    let editor_h = state.rows * row + 8.0;
    let original_h = ORIGINAL_ROWS * row + 8.0;
    let size = window_size(row, state.rows);
    let window_h = size.y;

    let mut action = None;
    let mut window = egui::Window::new(tr.review_title)
        .id(egui::Id::new("grace_prompt_review"))
        .collapsible(false)
        .resizable(false)
        .fixed_size(size);
    if std::mem::take(&mut state.center_next_frame) {
        let c = ctx.content_rect().center();
        window = window.current_pos(c - size * 0.5);
    }
    window.show(ctx, |ui| {
        ui.horizontal_top(|ui| {
            // Grace, at one fifth of the width.
            let img_w = WINDOW_W * GRACE_SHARE;
            ui.allocate_ui(egui::vec2(img_w, window_h - 30.0), |ui| {
                ui.add(
                    egui::Image::new(egui::include_image!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../assets/images/grace.png"
                    )))
                    .max_width(img_w),
                );
            });
            ui.add_space(10.0);
            ui.vertical(|ui| {
                let text_w = WINDOW_W - img_w - 34.0;

                // ── The developer's own words, read-only ──────────────────
                ui.label(egui::RichText::new(tr.review_original).small().strong());
                let mut original = state.original.as_str();
                ui.allocate_ui(egui::vec2(text_w, original_h), |ui| {
                    ui.set_min_size(egui::vec2(text_w, original_h));
                    ui.set_max_size(egui::vec2(text_w, original_h));
                    egui::ScrollArea::vertical()
                        .id_salt("review_original_scroll")
                        .auto_shrink([false, false])
                        .min_scrolled_height(original_h)
                        .max_height(original_h)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut original)
                                    .desired_width(f32::INFINITY)
                                    .interactive(false),
                            );
                        });
                });

                ui.add_space(6.0);
                ui.label(egui::RichText::new(tr.review_revised).small().strong());

                // ── The revision, editable, with the flagged passages marked ──
                let highlights = locate(&state.revised, &state.notes);
                let hl_ranges: Vec<std::ops::Range<usize>> =
                    highlights.iter().map(|h| h.range.clone()).collect();
                let mut layouter =
                    |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
                    let mut job = highlight_job(ui, buf.as_str(), &hl_ranges);
                    job.wrap.max_width = wrap;
                    ui.ctx().fonts_mut(|f| f.layout_job(job))
                };
                let out = ui
                    .allocate_ui(egui::vec2(text_w, editor_h), |ui| {
                        ui.set_min_size(egui::vec2(text_w, editor_h));
                        ui.set_max_size(egui::vec2(text_w, editor_h));
                        egui::ScrollArea::vertical()
                            .id_salt("review_revised_scroll")
                            .auto_shrink([false, false])
                            .min_scrolled_height(editor_h)
                            .max_height(editor_h)
                            .show(ui, |ui| {
                                egui::TextEdit::multiline(&mut state.revised)
                                    .id(egui::Id::new("grace_review_editor"))
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(state.rows as usize)
                                    .layouter(&mut layouter)
                                    .show(ui)
                            })
                            .inner
                    })
                    .inner;

                // Why a passage is marked, on hover.
                if let Some(pos) = ctx.pointer_hover_pos() {
                    if out.response.rect.contains(pos) {
                        let cursor = out.galley.cursor_from_pos(pos - out.galley_pos);
                        let byte = char_to_byte(&state.revised, cursor.index.0);
                        if let Some(h) = highlights.iter().find(|h| h.range.contains(&byte)) {
                            let why = h.why.clone();
                            egui::containers::Tooltip::always_open(
                                ctx.clone(),
                                ui.layer_id(),
                                egui::Id::new("grace_review_tip"),
                                egui::PopupAnchor::Pointer,
                            )
                            .show(|ui| {
                                ui.label(why);
                            });
                        }
                    }
                }

                // The grip: the ONLY writer of `rows`, and therefore of the
                // window's height.
                let grip = ui.interact(
                    egui::Rect::from_min_size(
                        out.response.rect.right_bottom() - egui::vec2(14.0, 14.0),
                        egui::vec2(14.0, 14.0),
                    ),
                    egui::Id::new("grace_review_grip"),
                    egui::Sense::drag(),
                );
                if grip.dragged() {
                    state.rows = (state.rows + grip.drag_delta().y / row).clamp(MIN_ROWS, MAX_ROWS);
                }
                if grip.hovered() || grip.dragged() {
                    ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }
                let stroke = if grip.hovered() || grip.dragged() {
                    ui.visuals().widgets.hovered.fg_stroke
                } else {
                    ui.visuals().widgets.inactive.fg_stroke
                };
                let corner = out.response.rect.right_bottom() - egui::vec2(4.0, 4.0);
                for step in 1..=3 {
                    let off = 3.0 * step as f32;
                    ui.painter().line_segment(
                        [
                            egui::pos2(corner.x - off, corner.y),
                            egui::pos2(corner.x, corner.y - off),
                        ],
                        stroke,
                    );
                }

                ui.add_space(4.0);
                if !highlights.is_empty() {
                    ui.label(
                        egui::RichText::new(tr.review_hint)
                            .small()
                            .color(egui::Color32::from_gray(150)),
                    );
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !state.revised.trim().is_empty(),
                            egui::Button::new(tr.review_submit),
                        )
                        .clicked()
                    {
                        action = Some(ReviewAction::Submit(state.revised.clone()));
                    }
                    if ui.button(tr.review_cancel).clicked() {
                        action = Some(ReviewAction::Cancel);
                    }
                });
            });
        });
    });
    action
}

/// A layout job that paints the flagged ranges on a warning background — the
/// text stays exactly the text, so editing it is unaffected.
fn highlight_job(ui: &egui::Ui, text: &str, ranges: &[std::ops::Range<usize>]) -> egui::text::LayoutJob {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let fg = ui.visuals().text_color();
    let mark = egui::Color32::from_rgb(120, 88, 20);
    let mut job = egui::text::LayoutJob::default();
    let mut at = 0usize;
    for r in ranges {
        if r.start > text.len() || r.end > text.len() || r.start < at {
            continue;
        }
        if r.start > at {
            job.append(&text[at..r.start], 0.0, egui::TextFormat::simple(font.clone(), fg));
        }
        let mut fmt = egui::TextFormat::simple(font.clone(), fg);
        fmt.background = mark;
        job.append(&text[r.clone()], 0.0, fmt);
        at = r.end;
    }
    if at < text.len() {
        job.append(&text[at..], 0.0, egui::TextFormat::simple(font, fg));
    }
    job
}

fn char_to_byte(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(b, _)| b)
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The modal opens at ten rows and the grip is bounded at fifteen.
    #[test]
    fn the_editor_opens_at_ten_rows_and_stops_at_fifteen() {
        let state = PromptReview::new("orig".into(), "rev".into(), vec![]);
        assert_eq!(state.rows, MIN_ROWS);
        let row = 19.0;
        let at_min = window_size(row, MIN_ROWS);
        let at_max = window_size(row, MAX_ROWS);
        assert!((at_max.y - at_min.y - (MAX_ROWS - MIN_ROWS) * row).abs() < 0.01);
        // Beyond the bounds the window does not follow.
        assert_eq!(window_size(row, 40.0), at_max);
        assert_eq!(window_size(row, 1.0), at_min);
        // The width never varies at all.
        assert_eq!(at_min.x, at_max.x);
    }

    /// The size cannot depend on the content: it is not an input.
    #[test]
    fn the_window_size_is_a_function_of_rows_alone() {
        let row = 19.0;
        let base = window_size(row, 12.0);
        for _ in 0..3 {
            assert_eq!(window_size(row, 12.0), base);
        }
    }
}
