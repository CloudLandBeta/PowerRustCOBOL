// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The **Definition for AI** text area and its tooltip, shared by every editor that has one: the Indexed
//! File Editor's file, record and field, and the Properties pane's form, layer and control.
//!
//! The box opens 5 lines by 60 columns of the body font and has a grip in its bottom-right corner; one
//! size is kept for the session for all of them, bounded so a slip of the mouse can neither hide it nor
//! swallow the pane. The label column and the value column of a pane are laid out apart, so each reads
//! [`comment_row_h`] and the two stay level.

/// The comment is a text area now (5 lines by 60 columns to begin with), so it can hold more than the
/// single line it used to.
pub(crate) const COMMENT_MAX_LEN: usize = 500;
const COMMENT_COLS: f32 = 60.0;
const COMMENT_ROWS: f32 = 5.0;
/// Room around the text inside the box and below it, in points.
const COMMENT_PAD: f32 = 10.0;

pub(crate) fn label_with_colon(text: &str) -> String {
    let t = text.trim_end_matches(':').trim();
    format!("{t}:")
}

/// The box's size when the user has not dragged it: 60 columns by 5 lines of the body font.
pub(crate) fn comment_box_default(ui: &egui::Ui) -> egui::Vec2 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let (cw, rh) = ui.fonts_mut(|f| (f.glyph_width(&font, '0'), f.row_height(&font)));
    egui::vec2(COMMENT_COLS * cw + COMMENT_PAD, COMMENT_ROWS * rh + COMMENT_PAD)
}

/// A size the user dragged the box to, kept between a third of the default and a few times it, so a
/// slip of the mouse can neither hide the box nor swallow the pane.
pub(crate) fn clamp_comment_box(size: egui::Vec2, default: egui::Vec2) -> egui::Vec2 {
    egui::vec2(
        size.x.clamp(default.x * 0.33, default.x * 2.5),
        size.y.clamp(default.y * 0.4, default.y * 6.0),
    )
}

pub(crate) fn comment_box_id() -> egui::Id {
    egui::Id::new("indexed-comment-box-size")
}

/// The comment box's current size. One size for the file, record and field editors, kept for the session.
pub(crate) fn comment_box_size(ui: &egui::Ui) -> egui::Vec2 {
    let default = comment_box_default(ui);
    let stored = ui.ctx().data(|d| d.get_temp::<egui::Vec2>(comment_box_id()));
    clamp_comment_box(stored.unwrap_or(default), default)
}

/// The height of the Comments row in BOTH columns: the box plus a little air. The label column and the
/// value column are laid out apart, so each reads this to stay level.
pub(crate) fn comment_row_h(ui: &egui::Ui) -> f32 {
    comment_box_size(ui).y + COMMENT_PAD
}

/// What one frame of the text area reported.
pub(crate) struct EditorOut {
    /// The text changed this frame.
    pub changed: bool,
    /// The text area lost the focus this frame: the moment to commit an edit that must be one undo step.
    pub lost_focus: bool,
}

/// The text area: 5 lines by 60 columns until the user drags its bottom-right corner, which resizes it
/// (and the Comments row with it). Longer text scrolls inside. Returns whether the text changed.
pub(crate) fn comment_editor(ui: &mut egui::Ui, text: &mut String) -> bool {
    comment_editor_out(ui, text, None).changed
}

/// [`comment_editor`] with an id the caller can ask the focus about, and the focus it reports.
pub(crate) fn comment_editor_out(ui: &mut egui::Ui, text: &mut String, id: Option<egui::Id>) -> EditorOut {
    let default = comment_box_default(ui);
    let size = comment_box_size(ui);
    let row_h = ui.fonts_mut(|f| f.row_height(&egui::TextStyle::Body.resolve(ui.style())));
    let rows = (((size.y - COMMENT_PAD) / row_h).floor() as usize).max(1);
    let width = size.x.min(ui.available_width());
    let mut changed = false;
    let mut lost_focus = false;
    let out = egui::ScrollArea::vertical()
        .max_width(width)
        .max_height(size.y)
        .min_scrolled_height(size.y)
        .auto_shrink([true, false])
        .show(ui, |ui| {
            let mut edit = egui::TextEdit::multiline(text)
                .desired_width(width - COMMENT_PAD)
                .desired_rows(rows)
                .char_limit(COMMENT_MAX_LEN)
                .lock_focus(true);
            if let Some(id) = id {
                edit = edit.id(id);
            }
            let resp = ui.add(edit);
            changed = resp.changed();
            lost_focus = resp.lost_focus();
        });
    // The grip: a small square over the box's bottom-right corner.
    let corner = out.inner_rect.right_bottom();
    let grip = egui::Rect::from_min_size(corner - egui::vec2(14.0, 14.0), egui::vec2(14.0, 14.0));
    let resp = ui.interact(grip, comment_box_id().with("grip"), egui::Sense::drag());
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
    }
    if resp.dragged() {
        let new = clamp_comment_box(size + resp.drag_delta(), default);
        ui.ctx().data_mut(|d| d.insert_temp(comment_box_id(), new));
    }
    let ink = ui.visuals().weak_text_color();
    for k in [4.0_f32, 8.0, 12.0] {
        ui.painter().line_segment([corner - egui::vec2(k, 2.0), corner - egui::vec2(2.0, k)], egui::Stroke::new(1.0, ink));
    }
    EditorOut { changed, lost_focus }
}
/// Cut `text` to [`COMMENT_MAX_LEN`] characters. By characters, not bytes: a byte cut could land inside an
/// accented letter and panic.
pub(crate) fn truncate_text(text: &mut String) {
    if text.chars().count() > COMMENT_MAX_LEN {
        *text = text.chars().take(COMMENT_MAX_LEN).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(f: impl FnOnce(&mut egui::Ui)) {
        let ctx = egui::Context::default();
        let mut f = Some(f);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            if let Some(f) = f.take() {
                f(ui);
            }
        });
        out.textures_delta.clear();
    }

    #[test]
    fn the_box_starts_at_60_columns_by_5_lines_of_the_body_font_and_the_row_is_as_tall() {
        frame(|ui| {
            let font = egui::TextStyle::Body.resolve(ui.style());
            let (cw, rh) = ui.fonts_mut(|f| (f.glyph_width(&font, '0'), f.row_height(&font)));
            let d = comment_box_default(ui);
            assert_eq!(d, egui::vec2(60.0 * cw + COMMENT_PAD, 5.0 * rh + COMMENT_PAD));
            assert_eq!(comment_box_size(ui), d, "nothing dragged yet: the default");
            assert_eq!(comment_row_h(ui), d.y + COMMENT_PAD, "label and value rows read the same height");
            println!("indexed comment box: default {:.0} x {:.0} pt ({} columns x {} lines), row {:.0} pt", d.x, d.y, COMMENT_COLS, COMMENT_ROWS, comment_row_h(ui));
        });
    }

    #[test]
    fn a_dragged_size_is_kept_for_the_session_and_bounded() {
        frame(|ui| {
            let d = comment_box_default(ui);
            ui.ctx().data_mut(|m| m.insert_temp(comment_box_id(), d * 1.5));
            assert_eq!(comment_box_size(ui), d * 1.5, "a size inside the bounds is kept");
            assert_eq!(comment_row_h(ui), d.y * 1.5 + COMMENT_PAD, "and the row follows it");
            for (asked, want) in [
                (egui::vec2(1.0, 1.0), egui::vec2(d.x * 0.33, d.y * 0.4)),
                (egui::vec2(1.0e6, 1.0e6), egui::vec2(d.x * 2.5, d.y * 6.0)),
            ] {
                ui.ctx().data_mut(|m| m.insert_temp(comment_box_id(), asked));
                assert_eq!(comment_box_size(ui), want, "{asked:?} is bounded");
            }
        });
    }

    #[test]
    fn the_editor_draws_in_the_row_it_was_given_and_lets_text_of_many_lines_in() {
        frame(|ui| {
            let mut text = "line one\nline two\nline three".to_owned();
            let before = ui.cursor().min.y;
            let changed = comment_editor(ui, &mut text);
            let used = ui.min_rect().max.y - before;
            assert!(!changed, "drawing it changes nothing");
            assert_eq!(text, "line one\nline two\nline three");
            assert!(used >= comment_box_size(ui).y - 1.0, "the box takes its height: {used}");
        });
    }

    #[test]
    fn truncation_is_by_characters_so_an_accented_letter_at_the_edge_cannot_panic() {
        let mut s = "é".repeat(COMMENT_MAX_LEN + 40);
        truncate_text(&mut s);
        assert_eq!(s.chars().count(), COMMENT_MAX_LEN);
        s = "short".into();
        truncate_text(&mut s);
        assert_eq!(s, "short");
    }
}

#[cfg(test)]
mod tooltip_tests {
    /// Every kind of thing that carries a Definition for AI has its own tooltip, in each of the six
    /// languages, none left in English, each saying who reads the text and what to write for that kind.
    #[test]
    fn the_eight_kinds_have_distinct_tooltips_in_every_language() {
        let mut checked = 0;
        for &lang in crate::i18n::Language::ALL {
            let tr = lang.tr();
            let tips = [
                tr.tip_def_ai_file,
                tr.tip_def_ai_record,
                tr.tip_def_ai_key,
                tr.tip_def_ai_field,
                tr.tip_def_ai_form,
                tr.tip_def_ai_layer,
                tr.tip_def_ai_control,
                tr.tip_def_ai_nonvisual,
            ];
            for (i, a) in tips.iter().enumerate() {
                assert!(a.len() > 150, "{lang:?} tip {i} is too short");
                for b in &tips[i + 1..] {
                    assert_ne!(a, b, "{lang:?}: two kinds share one tooltip");
                }
                checked += 1;
            }
        }
        let en = crate::i18n::Language::ALL[0].tr();
        for &lang in &crate::i18n::Language::ALL[1..] {
            let t = lang.tr();
            for (a, b) in [
                (t.tip_def_ai_form, en.tip_def_ai_form),
                (t.tip_def_ai_layer, en.tip_def_ai_layer),
                (t.tip_def_ai_control, en.tip_def_ai_control),
                (t.tip_def_ai_nonvisual, en.tip_def_ai_nonvisual),
            ] {
                assert_ne!(a, b, "{lang:?} still carries the English text");
            }
        }
        println!("definition tooltips: {} languages x 8 kinds = {checked} texts, all distinct", crate::i18n::Language::ALL.len());
    }
}
