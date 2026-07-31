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
//! **the window's size comes from state, never from its content.** Three
//! numbers are that state — the editor's row count, the window's width and the
//! editor's font size — and each one moves only when the developer moves it,
//! by dragging a grip or pressing a font button. No branch here derives a size
//! from how long a text is or how many passages Grace flagged, so there is no
//! path by which the window can grow itself (operator, 2026-07-31).

use crate::i18n::Tr;
use crate::prompt_polish::{locate, Note};

/// Editor size in text rows: where it opens, and how far a grip may take it.
pub const MIN_ROWS: f32 = 10.0;
pub const MAX_ROWS: f32 = 15.0;
/// Window width: where it opens, and the bounds the window grip honours.
pub const DEFAULT_WIDTH: f32 = 760.0;
pub const MIN_WIDTH: f32 = 620.0;
pub const MAX_WIDTH: f32 = 1200.0;
/// Editor font size, driven by the two buttons over the revision.
pub const DEFAULT_FONT: f32 = 14.0;
pub const MIN_FONT: f32 = 10.0;
pub const MAX_FONT: f32 = 26.0;
/// The image column, as a share of the window's width.
const GRACE_SHARE: f32 = 0.20;
/// Gap between the image column and the text column.
const COL_GAP: f32 = 10.0;
/// Rows of the read-only "your request" block. Fixed: it is for comparison,
/// not for reading a novel, and a content-sized block would size the window.
const ORIGINAL_ROWS: f32 = 3.0;
/// Side of a resize grip's hit area.
const GRIP: f32 = 14.0;

/// Line height for a given font size. One place, so the editor's viewport and
/// the window's height can never disagree about what a row costs.
pub fn row_height(font: f32) -> f32 {
    font * 1.35
}

/// What the developer decided.
pub enum ReviewAction {
    /// Run the workflow with this text — the revision as it now stands.
    Submit(String),
    /// Close and go back to the prompt box, unchanged.
    Cancel,
}

/// The modal's own state. `rows`, `width` and `font` are user-authoritative:
/// nothing but a grip drag or a font button writes them.
pub struct PromptReview {
    pub original: String,
    pub revised: String,
    pub notes: Vec<Note>,
    rows: f32,
    width: f32,
    font: f32,
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
            width: DEFAULT_WIDTH,
            font: DEFAULT_FONT,
            center_next_frame: true,
        }
    }
}

/// The window's size, from the three state numbers alone.
///
/// This signature IS the guarantee the operator asked for: there is no
/// argument here through which the content could speak. The window cannot grow
/// because a text got longer, because Grace flagged more passages, or because
/// a wrapped line appeared — only because the developer dragged a grip or
/// enlarged the font.
pub fn window_size(font: f32, rows: f32, width: f32) -> egui::Vec2 {
    let row = row_height(font.clamp(MIN_FONT, MAX_FONT));
    let rows = rows.clamp(MIN_ROWS, MAX_ROWS);
    let editor_h = rows * row + 8.0;
    let original_h = ORIGINAL_ROWS * row + 8.0;
    egui::vec2(
        width.clamp(MIN_WIDTH, MAX_WIDTH),
        34.0 + original_h + 30.0 + editor_h + 24.0 + 38.0 + 20.0,
    )
}

/// A resize grip: the three-line corner mark, its hit area, and its cursor.
/// Returns the drag delta, which the caller turns into state.
fn grip(ui: &egui::Ui, corner: egui::Pos2, id: &str, cursor: egui::CursorIcon) -> egui::Vec2 {
    let response = ui.interact(
        egui::Rect::from_min_size(corner - egui::vec2(GRIP, GRIP), egui::vec2(GRIP, GRIP)),
        egui::Id::new(id),
        egui::Sense::drag(),
    );
    let active = response.hovered() || response.dragged();
    if active {
        ui.ctx().set_cursor_icon(cursor);
    }
    let stroke = if active {
        ui.visuals().widgets.hovered.fg_stroke
    } else {
        ui.visuals().widgets.inactive.fg_stroke
    };
    let mark = corner - egui::vec2(3.0, 3.0);
    for step in 1..=3 {
        let off = 3.0 * step as f32;
        ui.painter().line_segment(
            [
                egui::pos2(mark.x - off, mark.y),
                egui::pos2(mark.x, mark.y - off),
            ],
            stroke,
        );
    }
    response.drag_delta()
}

/// Draw the modal. Returns the developer's decision on the frame they make it.
pub fn show(ctx: &egui::Context, tr: &Tr, state: &mut PromptReview) -> Option<ReviewAction> {
    let row = row_height(state.font);
    let editor_h = state.rows * row + 8.0;
    let original_h = ORIGINAL_ROWS * row + 8.0;
    let size = window_size(state.font, state.rows, state.width);

    let mut action = None;
    let mut window = egui::Window::new(tr.review_title)
        .id(egui::Id::new("grace_prompt_review"))
        .collapsible(false)
        // egui's own resize is what inflates on its own; the grips below write
        // state instead, and the window is laid out from that state.
        .resizable(false)
        .fixed_size(size);
    if std::mem::take(&mut state.center_next_frame) {
        let c = ctx.content_rect().center();
        window = window.current_pos(c - size * 0.5);
    }
    window.show(ctx, |ui| {
        // The window's content rect. It IS `size` — egui gave it to us because
        // we asked for exactly that — so measuring it feeds nothing back: the
        // columns below are derived from state, through this rect, and never
        // from how much room the text would like. Taking it from egui rather
        // than recomputing it is what keeps the columns flush with the frame
        // (and the title bar above it) whatever margins the style applies.
        let content = ui.max_rect();
        let img_w = content.width() * GRACE_SHARE;
        let text_w = content.width() - img_w - COL_GAP;

        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = COL_GAP;
            ui.allocate_ui(egui::vec2(img_w, size.y - 30.0), |ui| {
                ui.add(
                    egui::Image::new(egui::include_image!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../assets/images/grace.png"
                    )))
                    .max_width(img_w),
                );
            });
            ui.vertical(|ui| {
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
                                    .font(egui::FontId::monospace(state.font))
                                    .desired_width(f32::INFINITY)
                                    .interactive(false),
                            );
                        });
                });

                ui.add_space(6.0);
                // ── Revision heading + font controls ──────────────────────
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(tr.review_revised).small().strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(state.font < MAX_FONT, egui::Button::new("A+").small())
                            .on_hover_text(tr.review_font_larger)
                            .clicked()
                        {
                            state.font = (state.font + 1.0).clamp(MIN_FONT, MAX_FONT);
                        }
                        if ui
                            .add_enabled(state.font > MIN_FONT, egui::Button::new("A−").small())
                            .on_hover_text(tr.review_font_smaller)
                            .clicked()
                        {
                            state.font = (state.font - 1.0).clamp(MIN_FONT, MAX_FONT);
                        }
                    });
                });

                // ── The revision, editable, with the flagged passages marked ──
                let highlights = locate(&state.revised, &state.notes);
                let hl_ranges: Vec<std::ops::Range<usize>> =
                    highlights.iter().map(|h| h.range.clone()).collect();
                let font = state.font;
                let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
                    let mut job = highlight_job(ui, buf.as_str(), &hl_ranges, font);
                    job.wrap.max_width = wrap;
                    ui.ctx().fonts_mut(|f| f.layout_job(job))
                };
                let rows = state.rows as usize;
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
                                    .font(egui::FontId::monospace(font))
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(rows)
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

                // The editor's own grip: taller or shorter text area, within
                // the ten-to-fifteen row bounds.
                let dy = grip(
                    ui,
                    // Inside the editor's frame, clear of the scrollbar.
                    out.response.rect.right_bottom() - egui::vec2(2.0, 2.0),
                    "grace_review_editor_grip",
                    egui::CursorIcon::ResizeVertical,
                )
                .y;
                if dy != 0.0 {
                    state.rows = (state.rows + dy / row).clamp(MIN_ROWS, MAX_ROWS);
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

        // The window's grip, in its bottom-right corner: width and height
        // together, each clamped to its own bounds.
        let delta = grip(
            ui,
            content.right_bottom(),
            "grace_review_window_grip",
            egui::CursorIcon::ResizeNwSe,
        );
        if delta != egui::Vec2::ZERO {
            state.width = (state.width + delta.x).clamp(MIN_WIDTH, MAX_WIDTH);
            state.rows = (state.rows + delta.y / row).clamp(MIN_ROWS, MAX_ROWS);
        }
    });
    action
}

/// A layout job that paints the flagged ranges on a warning background — the
/// text stays exactly the text, so editing it is unaffected.
fn highlight_job(
    ui: &egui::Ui,
    text: &str,
    ranges: &[std::ops::Range<usize>],
    font_size: f32,
) -> egui::text::LayoutJob {
    let font = egui::FontId::monospace(font_size);
    let fg = ui.visuals().text_color();
    let mark = egui::Color32::from_rgb(120, 88, 20);
    let mut job = egui::text::LayoutJob::default();
    let mut at = 0usize;
    for r in ranges {
        if r.start > text.len() || r.end > text.len() || r.start < at {
            continue;
        }
        if r.start > at {
            job.append(
                &text[at..r.start],
                0.0,
                egui::TextFormat::simple(font.clone(), fg),
            );
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

    /// The modal opens at ten rows and a grip is bounded at fifteen.
    #[test]
    fn the_editor_opens_at_ten_rows_and_stops_at_fifteen() {
        let state = PromptReview::new("orig".into(), "rev".into(), vec![]);
        assert_eq!(state.rows, MIN_ROWS);
        assert_eq!(state.width, DEFAULT_WIDTH);
        assert_eq!(state.font, DEFAULT_FONT);

        let f = DEFAULT_FONT;
        let at_min = window_size(f, MIN_ROWS, DEFAULT_WIDTH);
        let at_max = window_size(f, MAX_ROWS, DEFAULT_WIDTH);
        assert!(
            (at_max.y - at_min.y - (MAX_ROWS - MIN_ROWS) * row_height(f)).abs() < 0.01,
            "five more rows must cost exactly five row heights"
        );
        // Beyond the bounds the window does not follow.
        assert_eq!(window_size(f, 40.0, DEFAULT_WIDTH), at_max);
        assert_eq!(window_size(f, 1.0, DEFAULT_WIDTH), at_min);
    }

    /// The window grip moves the width, within its own bounds, and the width
    /// never touches the height.
    #[test]
    fn the_window_width_is_the_developers_and_is_bounded() {
        let f = DEFAULT_FONT;
        let narrow = window_size(f, MIN_ROWS, MIN_WIDTH);
        let wide = window_size(f, MIN_ROWS, MAX_WIDTH);
        assert_eq!(narrow.x, MIN_WIDTH);
        assert_eq!(wide.x, MAX_WIDTH);
        assert_eq!(narrow.y, wide.y, "width must not disturb height");
        // A drag past either end stops at the end.
        assert_eq!(window_size(f, MIN_ROWS, 10.0).x, MIN_WIDTH);
        assert_eq!(window_size(f, MIN_ROWS, 9_000.0).x, MAX_WIDTH);
    }

    /// A bigger font makes the window taller — the developer asked for that by
    /// pressing the button. It never changes the width.
    #[test]
    fn the_font_buttons_are_bounded_and_only_change_the_height() {
        let small = window_size(MIN_FONT, MIN_ROWS, DEFAULT_WIDTH);
        let large = window_size(MAX_FONT, MIN_ROWS, DEFAULT_WIDTH);
        assert!(large.y > small.y);
        assert_eq!(large.x, small.x);
        assert_eq!(window_size(2.0, MIN_ROWS, DEFAULT_WIDTH), small);
        assert_eq!(window_size(99.0, MIN_ROWS, DEFAULT_WIDTH), large);
    }

    /// The size cannot depend on the content: it is not an input.
    #[test]
    fn the_window_size_is_a_function_of_its_state_alone() {
        let base = window_size(DEFAULT_FONT, 12.0, 800.0);
        for _ in 0..3 {
            assert_eq!(window_size(DEFAULT_FONT, 12.0, 800.0), base);
        }
    }
}
