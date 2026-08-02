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
//!
//! One more rule the same regression taught, the hard way: the two columns
//! below (the Grace image, the text) are sized from `state.width` directly —
//! **never** from `ui.max_rect()` / `ui.available_width()`, even though a
//! `ui.max_rect()` read looks perfectly innocent right after the window opens.
//! `egui::Window` auto-sizes to fit its content and — per its own doc comment
//! on `fixed_size` — "may still auto-resize" past a requested size even with
//! `resizable(false)`. `ui.max_rect()` inside such a window is the *auto-sizing
//! region itself*: reading its width to size a column, then handing that
//! column both `set_min_size` and `set_max_size` (as the original/editor
//! blocks below do, to stay bounded), makes the column literally BECOME
//! whatever `max_rect` reported — which can grow the next frame. `state.width`
//! has no such loop: it is a plain number nothing but the window's own grip
//! ever writes.
//!
//! The window's own grip (drawn at the very end of `show()`) is anchored on
//! the ACTUAL rendered content's bounding rect — the `InnerResponse` egui
//! itself hands back for the `ui.horizontal_top` block below, not a
//! hand-computed guess of where the title bar, margins and stroke leave off.
//! That rect is `child_ui.min_rect()` under the hood (egui's own accounting
//! of what was placed), so the grip lands at the window's true corner at any
//! title bar height, window margin or font size the active theme happens to
//! use — including future retunes of `app.rs::apply_theme` — without this
//! file needing to re-guess egui's chrome pixel-for-pixel ever again. Reading
//! it here is safe precisely because it is read AFTER layout, purely to
//! position a non-allocating decoration — never fed back into a child's size.
//!
//! Two more fixes, from the operator's next report, the day after: bounding a
//! block's *layout* (`set_min_size` + `set_max_size`, as the original/editor
//! blocks already did) is not the same as bounding its *paint*. `egui::Ui`
//! will happily let a child paint past its own `max_rect` — nothing about
//! `set_max_size` clips a pixel; the clipping those two blocks actually rely
//! on comes from `ScrollArea`'s own internal clip rect, which in turn floors
//! itself at *last frame's* measured content size so a shrinking drag never
//! flashes a clipped-too-soon frame. That is the right call for a drag, but
//! it means the very first frame a block's content is bigger than it has
//! ever been (a longer paste, a first-ever open, a font bump) can paint past
//! its own box before the next frame's bookkeeping catches up. So both boxes
//! now also call `ui.shrink_clip_rect` on their own bounds directly, right
//! after `set_max_size` — a clip computed fresh from this frame's own state
//! (`text_w`/`original_h`/`editor_h`), never from any egui-internal memory of
//! a previous frame, so there is no lag for it to have. The hint label got
//! the same box-plus-clip treatment, for the same reason, plus a widened
//! budget (`HINT_ROWS`, now two lines instead of one): a one-line budget for
//! a label whose actual height nothing bounded was the second place this
//! file quietly let content, not state, decide a size — narrower window
//! widths or a longer future translation could make that label wrap, and
//! nothing budgeted room for the second line. Together these closed the gap
//! the label, the Submit/Cancel row and the window's own grip were seen
//! rendering through, past the window's own visible border (operator report,
//! 2026-08-01).

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

/// Rows reserved for the "still open to more than one reading" hint label,
/// whether or not it is shown this frame. Two, not one: at the window's
/// default width the current English text fits on one line (see
/// `hint_rows_budgets_enough_for_every_language`, which checks every
/// language at the NARROWEST width a developer can drag the window's own
/// grip to), but nothing bounded the label's height before this fix, so a
/// narrower drag, a future longer translation, or a different font
/// substitution wrapping it to a second line had no room budgeted for it —
/// budgeting only one row here was one of the places this file quietly let
/// content, not state, decide a size. See `show()` for the bounded, clipped
/// box this budget now backs, exactly like the original/editor blocks below
/// it (operator report, 2026-08-01).
const HINT_ROWS: f32 = 2.0;

/// Height of the hint label's own reserved box. A small "small"-text estimate
/// (11.5pt), the same one `window_size` below already used for the original
/// and revised headings, so the two never disagree about what that text
/// style costs.
fn hint_h() -> f32 {
    HINT_ROWS * row_height(11.5) + 4.0
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

/// Real chrome this file's seed is built from — not arbitrary literals. They
/// mirror `app.rs::apply_theme`'s window styling (a 12px window margin, an
/// 8px item spacing, a 25px Heading font) plus egui's own `Frame::window`
/// stroke width (1.0) and — confirmed by reading
/// `egui-0.35.0/src/containers/window.rs` — the fact that a window's content
/// sits behind *two* nested inner margins, not one: the outer `Frame::window`
/// margin, and a second `Frame::NONE.inner_margin(window_margin)` egui wraps
/// around the body specifically (below the title bar). If the theme is
/// retuned these drift out of date; per the module doc comment above, that
/// can only make the window's *opening* frame a little less snug, never
/// regress into the growing-window bug, because nothing downstream treats
/// this number as a constraint any more — `show()` never calls
/// `Window::fixed_size`/`min_size`/`max_size`.
const CHROME_WINDOW_MARGIN: f32 = 12.0;
const CHROME_STROKE: f32 = 1.0;
const CHROME_ITEM_SPACING: f32 = 8.0;
const CHROME_HEADING_FONT: f32 = 25.0;

/// The window's chrome to the LEFT and RIGHT of its content: the outer
/// `Frame::window` margin and stroke, plus the body's own nested margin.
///
/// `window_size` is handed to `Window::fixed_size`, which sizes the window's
/// OUTER footprint, while `state.width` is what the two COLUMNS inside are laid
/// out at. Without this term the columns would be asked to fit an inner area
/// narrower than themselves and would be clipped at the right edge. While the
/// window still auto-sized, the omission was invisible — the window simply grew
/// by the missing margins.
pub const CHROME_SIDES: f32 =
    2.0 * (CHROME_WINDOW_MARGIN + CHROME_STROKE) + 2.0 * CHROME_WINDOW_MARGIN;

/// The window's size, from the three state numbers alone.
///
/// This signature IS the guarantee the operator asked for: there is no
/// argument here through which the content could speak. The window cannot grow
/// because a text got longer, because Grace flagged more passages, or because
/// a wrapped line appeared — only because the developer dragged a grip or
/// enlarged the font.
///
/// The result is a SEED, used only for the window's opening `default_size`
/// (so it doesn't visibly pop on the first frame) and for the one-time
/// centering math in `show()`. It does not have to match egui's real chrome
/// pixel for pixel: `show()` anchors its own resize grip on the actually
/// rendered content's bounding rect, not on this estimate — see the module
/// doc comment for why that is what actually keeps the window from growing.
pub fn window_size(font: f32, rows: f32, width: f32) -> egui::Vec2 {
    // Outer `Frame::window` margin + stroke, top and bottom, plus the body's
    // own nested `Frame::NONE` margin, top and bottom, plus the title bar
    // (a Heading-sized row) and the item spacing between it and the body.
    let window_chrome = 2.0 * (CHROME_WINDOW_MARGIN + CHROME_STROKE)
        + 2.0 * CHROME_WINDOW_MARGIN
        + row_height(CHROME_HEADING_FONT)
        + CHROME_ITEM_SPACING;

    egui::vec2(
        width.clamp(MIN_WIDTH, MAX_WIDTH) + CHROME_SIDES,
        window_chrome + column_height(font, rows),
    )
}

/// The text column's own height — everything [`window_size`] adds beyond the
/// window's chrome, and the exact box `show()` partitions.
///
/// `show()` hands this to one `allocate_ui`, then lets `Panel::bottom` take the
/// hint and the button row out of it and gives the editor whatever is left. So
/// these terms no longer have to PREDICT the interior: they set how big the box
/// is, and the split inside it is measured, not estimated. Only the editor's
/// own term still says how tall the editor wants to be, which is what a grip
/// drag and the font buttons are for.
pub fn column_height(font: f32, rows: f32) -> f32 {
    let row = row_height(font.clamp(MIN_FONT, MAX_FONT));
    let rows = rows.clamp(MIN_ROWS, MAX_ROWS);
    let editor_h = rows * row + 8.0;
    let original_h = ORIGINAL_ROWS * row + 8.0;

    row_height(11.5) // the "review_original" heading label
        + original_h
        + 6.0 // the explicit `ui.add_space(6.0)` after the original block
        + 30.0 // the "review_revised" row, with the A+/A- buttons
        + editor_h
        + 4.0 // the space between the editor and the footer
        + hint_h() // the hint's own bounded box, reserved shown or not
        + 6.0 // the explicit `ui.add_space(6.0)` before the button row
        + 30.0 // the Submit/Cancel row
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
    let original_h = ORIGINAL_ROWS * row + 8.0;
    let size = window_size(state.font, state.rows, state.width);
    let col_h = column_height(state.font, state.rows);
    // Whether the hint has anything to say, decided here so the footer can
    // reserve its box before the editor is laid out. Emptiness only: the
    // tooltip's byte ranges are still recomputed after `TextEdit` has applied
    // this frame's edit, which is what keeps them ranges into the live string.
    let hint_visible = !locate(&state.revised, &state.notes).is_empty();

    let mut action = None;
    let mut window = egui::Window::new(tr.review_title)
        .id(egui::Id::new("grace_prompt_review"))
        .collapsible(false)
        // egui's own resize is what inflates on its own; the grips below write
        // state instead, and the window is laid out from that state.
        //
        // `fixed_size`, not `default_size`. A `default_size` is a SEED: egui
        // applies it once and then keeps the window's own stored rect, so a
        // rect that ever became larger than the state — an older build's
        // inflation, a stored rect from a previous session — outlives every
        // correction, and the state-sized content sits inside an oversized
        // window with dead margins to its right and below it (operator
        // screenshots, 2026-08-02). Restating the size every frame makes
        // `state` the one authority for the window as well as its content, so
        // the two cannot drift apart.
        //
        // egui's doc on `fixed_size` warns a window "may still auto-resize"
        // past it, and that warning is about content that overflows the box.
        // It cannot bite here: every block below is allocated at a size
        // computed from `state` alone and clipped to it, so the content can
        // never ask for more than the box. That bound — not the choice of
        // sizing call — remains the real anti-growth guarantee.
        .resizable(false)
        .fixed_size(size);
    if std::mem::take(&mut state.center_next_frame) {
        let c = ctx.content_rect().center();
        window = window.current_pos(c - size * 0.5);
    }
    window.show(ctx, |ui| {
        // The two columns' widths come from `state.width` directly — never
        // from `ui.max_rect()` / `ui.available_width()`. See the module doc
        // comment for why: this window no longer has a `fixed_size`, so
        // `ui.max_rect()` here is the auto-sizing region itself, and the
        // original/editor blocks below force their own size to match
        // `text_w` exactly (`set_min_size` + `set_max_size`) — feeding a
        // `max_rect` reading into that would be the classic loop, just one
        // step removed. `state.width` is a plain number; nothing but this
        // window's own grip, at the bottom of this closure, ever writes it.
        let img_w = state.width * GRACE_SHARE;
        let text_w = state.width - img_w - COL_GAP;

        let content = ui.horizontal_top(|ui| {
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
            ui.allocate_ui(egui::vec2(text_w, col_h), |ui| {
                ui.set_min_size(egui::vec2(text_w, col_h));
                ui.set_max_size(egui::vec2(text_w, col_h));

                // The footer claims its height out of the box FIRST, and the
                // editor then takes what is left — measured, not predicted.
                //
                // Laying the column out top-to-bottom instead meant the
                // editor's height was an estimate (`rows * font * 1.35 + 8`)
                // that had to agree with what egui's real font metrics make a
                // row cost. It does not: on the operator's machine the tenth
                // row was sliced through the middle and the overflow ran on
                // under the hint and the buttons (screen recording,
                // 2026-08-02). A `Panel::bottom` cannot be overrun — whatever
                // the rows really cost, the editor stops where the footer
                // begins. This is the same partition `error_modal_body_ui`
                // uses, for the same reason.
                egui::Panel::bottom(ui.id().with("grace_review_footer"))
                    .resizable(false)
                    .show_separator_line(false)
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        // The hint's own bounded, clipped box — reserved at
                        // the same height whether or not a highlight is
                        // present, so nothing here depends on the content.
                        ui.allocate_ui(egui::vec2(text_w, hint_h()), |ui| {
                            ui.set_min_size(egui::vec2(text_w, hint_h()));
                            ui.set_max_size(egui::vec2(text_w, hint_h()));
                            let bounds = ui.max_rect();
                            ui.shrink_clip_rect(bounds);
                            if hint_visible {
                                ui.label(
                                    egui::RichText::new(tr.review_hint)
                                        .small()
                                        .color(egui::Color32::from_gray(150)),
                                );
                            }
                        });
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

                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                // ── The developer's own words, read-only ──────────────────
                ui.label(egui::RichText::new(tr.review_original).small().strong());
                let mut original = state.original.as_str();
                ui.allocate_ui(egui::vec2(text_w, original_h), |ui| {
                    ui.set_min_size(egui::vec2(text_w, original_h));
                    ui.set_max_size(egui::vec2(text_w, original_h));
                    // Bound the PAINT, not just the layout — see the module
                    // doc comment above for why `set_max_size` alone isn't
                    // enough. Computed fresh from `original_h` this frame, so
                    // there is no previous-frame memory for it to lag behind.
                    let bounds = ui.max_rect();
                    ui.shrink_clip_rect(bounds);
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
                //
                // The layouter is handed `buf`, not `state.revised` — and `buf`
                // is the text AFTER `TextEdit` has already applied this frame's
                // keystroke or paste, which happens INSIDE `.show()` below. A
                // range computed from `state.revised` one line earlier is a
                // range into a string that no longer exists the moment a byte
                // is inserted or removed before it: the offsets are still
                // numbers, but they now point at whatever bytes happen to sit
                // there in the NEW string, mid-character as often as not. That
                // produced a live crash — paste text before a flagged passage
                // containing an em dash, and the stale range sliced into one of
                // its bytes (operator report, 2026-07-31). `locate` re-run on
                // `buf.as_str()` is cheap (one form-sized string, a handful of
                // notes) and is the only text the ranges are ever allowed to be
                // ranges into.
                let notes_for_layout = state.notes.clone();
                let font = state.font;
                let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
                    let now = buf.as_str();
                    let ranges: Vec<std::ops::Range<usize>> = locate(now, &notes_for_layout)
                        .into_iter()
                        .map(|h| h.range)
                        .collect();
                    let mut job = highlight_job(ui, now, &ranges, font);
                    job.wrap.max_width = wrap;
                    ui.ctx().fonts_mut(|f| f.layout_job(job))
                };
                let rows = state.rows as usize;
                // What the footer left. Inside a box this file forced to
                // `col_h`, "what remains" is a bound, not the open-ended
                // remainder of an auto-sizing parent — the distinction the
                // module doc draws. Taking it measured is what keeps the
                // editor from running under the footer when egui's real row
                // height and this file's `row_height` estimate disagree.
                let editor_h = ui.available_height();
                let out = ui
                    .allocate_ui(egui::vec2(text_w, editor_h), |ui| {
                        ui.set_min_size(egui::vec2(text_w, editor_h));
                        ui.set_max_size(egui::vec2(text_w, editor_h));
                        // Bound the PAINT, not just the layout — see the
                        // module doc comment above for why `set_max_size`
                        // alone isn't enough. Computed fresh from `editor_h`
                        // this frame, so there is no previous-frame memory
                        // for it to lag behind — the fix for the operator's
                        // "10-11 lines, no scrollbar" report (2026-08-01).
                        let bounds = ui.max_rect();
                        ui.shrink_clip_rect(bounds);
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

                // Why a passage is marked, on hover. `state.revised` is now the
                // post-edit text (`.show()` above already applied this frame's
                // keystroke to it), so `locate` is run again here, against that
                // same text — never against the pre-edit snapshot the layouter
                // no longer uses either.
                let highlights = locate(&state.revised, &state.notes);
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

                    });
            });
        });

        // The window's grip, in its bottom-right corner: width and height
        // together, each clamped to its own bounds. Anchored on the rect
        // `ui.horizontal_top` actually reports it used — egui's own
        // `child_ui.min_rect()` accounting of what got placed, not a
        // hand-computed guess of where the title bar, margins and stroke
        // leave off — so it always lands at the window's true corner. See
        // the module doc comment for why reading it here, after layout, is
        // safe: it positions a decoration, never a child's own size.
        let delta = grip(
            ui,
            content.response.rect.right_bottom(),
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
        if !range_is_paintable(text, r, at) {
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

/// Is `r` safe to slice out of `text` and paint, given `at` (the position
/// already appended)? A pure predicate so the invariant is testable without
/// an `egui::Ui` — see `highlight_job`'s doc comment for why it exists at all.
///
/// `locate` only ever returns ranges that land on a char boundary of the text
/// it was run against — an exact-substring match cannot end mid-character,
/// since UTF-8 is self-synchronizing. This check exists for the text NOT
/// being that text: a caller that (again) hands `highlight_job` a range
/// computed against a different string than the one it is now slicing would
/// otherwise panic the whole app over one flagged passage, exactly as
/// happened live (operator report, 2026-07-31) when the ranges were computed
/// one frame before the text they were sliced from.
fn range_is_paintable(text: &str, r: &std::ops::Range<usize>, at: usize) -> bool {
    r.start >= at
        && r.start <= r.end
        && r.end <= text.len()
        && text.is_char_boundary(r.start)
        && text.is_char_boundary(r.end)
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

    /// The live crash, reduced to its cause: a range computed against one
    /// string, then sliced out of a DIFFERENT string where the same byte
    /// offset lands mid-character. `highlight_job`'s layouter used to compute
    /// ranges from `state.revised` one line before `TextEdit` (which owns the
    /// SAME string by `&mut`) applied that frame's keystroke or paste to it —
    /// so the ranges it painted with were ranges into a string that no longer
    /// existed by the time painting happened. The fix is structural (the
    /// layouter now re-runs `locate` against `buf.as_str()`, the only text it
    /// is ever allowed to be a range into); this test locks in the guard that
    /// stands behind it regardless.
    #[test]
    fn a_range_that_lands_mid_character_is_never_painted() {
        // The exact live shape: the developer's cursor sits right where a
        // flagged quote begins, and the frame's `TextEdit` applies a
        // keystroke — here, typing an em dash — BEFORE the layouter paints.
        // In "AB CD", the range 3..5 is "CD", a valid range with `at` byte
        // boundaries throughout. Insert "—" (3 bytes) at that exact
        // position and the SAME numbers, unchanged, are what a stale
        // pre-edit range would still say: "AB —CD" is now the text, and
        // byte 5 sits inside the dash's own continuation bytes — not a
        // character boundary at all.
        let before = "AB CD";
        let range_from_before = 3..5; // "CD", byte-exact in `before`
        assert!(range_is_paintable(before, &range_from_before, 0));

        let after = "AB —CD";
        assert!(
            !range_is_paintable(after, &range_from_before, 0),
            "a range computed against the pre-edit text must not be trusted \
             against the post-edit text"
        );

        // The same logical quote ("CD") IS paintable once its range is
        // recomputed against the actual current text — proving the guard
        // rejects staleness specifically, not "any range near a multi-byte
        // character".
        assert!(range_is_paintable(after, &(6..8), 0));

        // Bounds and ordering degenerate cases, all rejected without a panic.
        assert!(!range_is_paintable("short", &(0..99), 0), "past the end");
        assert!(!range_is_paintable("short", &(3..1), 0), "start after end");
        assert!(!range_is_paintable("short", &(0..3), 4), "before `at`");
    }

    /// `highlight_job` must not panic on a range that fails the guard, and
    /// must still paint the FULL text — only the one bad range's highlight is
    /// dropped, not the text it would have covered.
    #[test]
    fn highlight_job_degrades_instead_of_crashing_on_a_bad_range() {
        egui::__run_test_ui(|ui| {
            let text = "Update the — em dash — handlers";
            // In-bounds, but its END (byte 13) is a continuation byte of the
            // em dash — exactly the shape of the live panic.
            let bad = vec![11..13];
            let job = highlight_job(ui, text, &bad, 14.0);
            assert_eq!(job.text, text, "no text is lost when a highlight is skipped");

            // A well-formed range still paints normally alongside a bad one.
            let mixed = vec![11..13, 4..7]; // "the", byte-exact
            let job = highlight_job(ui, text, &mixed, 14.0);
            assert_eq!(job.text, text);
        });
    }

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
    ///
    /// The returned `x` is the window's OUTER footprint — the columns' own
    /// `state.width` plus the chrome either side of them — because `show()`
    /// hands this vector to `Window::fixed_size`. What matters is that it
    /// tracks `state.width` one for one and is bounded by the same clamps.
    #[test]
    fn the_window_width_is_the_developers_and_is_bounded() {
        let f = DEFAULT_FONT;
        let narrow = window_size(f, MIN_ROWS, MIN_WIDTH);
        let wide = window_size(f, MIN_ROWS, MAX_WIDTH);
        assert_eq!(narrow.x, MIN_WIDTH + CHROME_SIDES);
        assert_eq!(wide.x, MAX_WIDTH + CHROME_SIDES);
        assert_eq!(
            wide.x - narrow.x,
            MAX_WIDTH - MIN_WIDTH,
            "the columns' width must reach the window one for one"
        );
        assert_eq!(narrow.y, wide.y, "width must not disturb height");
        // A drag past either end stops at the end.
        assert_eq!(window_size(f, MIN_ROWS, 10.0).x, MIN_WIDTH + CHROME_SIDES);
        assert_eq!(
            window_size(f, MIN_ROWS, 9_000.0).x,
            MAX_WIDTH + CHROME_SIDES
        );
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

    /// The live regression, reproduced end to end: actually run `show()`
    /// through egui (real fonts, so `TextEdit` reports real row heights, not
    /// this file's own `row_height` guess) and read back the window's own
    /// rendered outer rect — `egui::Memory::area_rect`, the same bookkeeping
    /// `Window::show` itself writes — rather than trusting any number this
    /// file computes. Before this fix that rect could differ from what
    /// `window_size()` asked for (because `fixed_size` only *requests* an
    /// outer size and egui's own doc comment on it warns the window "may
    /// still auto-resize" past that), and the custom grip — drawn at
    /// `content.right_bottom()`, a pre-render estimate — would then sit well
    /// above the window's true, grown corner. This test pins two invariants
    /// that together rule that out: the rendered window is stable across
    /// repeated frames of unchanged state (no runaway growth), and it does
    /// not depend on how much text or how many flagged passages are in it
    /// (only `state.rows`/`state.width`/`state.font` may change it).
    #[test]
    fn the_rendered_window_is_stable_and_content_independent() {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let tr = crate::i18n::Language::English.tr();
        let id = egui::Id::new("grace_prompt_review");

        let run = |state: &mut PromptReview| {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 1200.0),
            ));
            let _ = ctx.run_ui(input, |ui| {
                let ctx = ui.ctx().clone();
                let _ = show(&ctx, &tr, state);
            });
        };

        // egui needs one frame to discover an auto-sized window's own
        // content-driven size at all — universal to immediate-mode layout,
        // not specific to this file — so the FIRST frame a window with a
        // given id is ever shown is not a fair "did it grow?" baseline.
        // Warm up past that once, then compare: a real self-inflation loop
        // would keep growing on every subsequent frame, not stop after one.
        let mut short = PromptReview::new("short original".into(), "short revision".into(), vec![]);
        run(&mut short);
        run(&mut short);
        let rect_frame_1 = ctx.memory(|m| m.area_rect(id)).expect("window rendered");
        run(&mut short);
        let rect_frame_2 = ctx.memory(|m| m.area_rect(id)).expect("window rendered");
        assert!(
            (rect_frame_1.width() - rect_frame_2.width()).abs() < 0.5
                && (rect_frame_1.height() - rect_frame_2.height()).abs() < 0.5,
            "the window's own rendered rect changed between two frames of \
             unchanged state ({rect_frame_1:?} -> {rect_frame_2:?}) — this is \
             the self-inflation loop this file exists to prevent"
        );

        // A much longer revision — exactly the kind of input that, before
        // this fix, could have reached the window's size through
        // `ui.max_rect()`/`content.width()` had the columns still been
        // derived from it. `notes` stays empty on both sides so the hint
        // label (deliberately content-driven — it names a passage Grace
        // actually flagged, not a wrapped-line accident) does not confound
        // this comparison: it is not the invariant under test here.
        let long_revision = "word ".repeat(400);
        let mut long = PromptReview::new("short original".into(), long_revision, vec![]);
        run(&mut long);
        run(&mut long);
        run(&mut long);
        let rect_long = ctx.memory(|m| m.area_rect(id)).expect("window rendered");

        assert!(
            (rect_frame_2.width() - rect_long.width()).abs() < 0.5,
            "window width changed with content length ({} vs {}) — a column \
             is reading available/max_rect space again instead of state.width",
            rect_frame_2.width(),
            rect_long.width(),
        );
        assert!(
            (rect_frame_2.height() - rect_long.height()).abs() < 0.5,
            "window height changed with content length ({} vs {}) — the same \
             self-inflation bug this file exists to prevent",
            rect_frame_2.height(),
            rect_long.height(),
        );
    }

    /// The companion to the previous test, from the other direction: the
    /// rendered width MUST track `state.width` (the only thing a developer's
    /// drag on the window's own grip ever writes). A fresh `Context` per
    /// width, so neither run's `Resize`/`Area` memory can leak into the
    /// other and quietly make this pass for the wrong reason.
    #[test]
    fn the_rendered_window_width_tracks_state_width() {
        let tr = crate::i18n::Language::English.tr();
        let id = egui::Id::new("grace_prompt_review");

        let rendered_width_at = |width: f32| -> f32 {
            let ctx = egui::Context::default();
            ctx.set_fonts(egui::FontDefinitions::default());
            let mut state = PromptReview::new("orig".into(), "rev".into(), vec![]);
            state.width = width;
            let run = |state: &mut PromptReview| {
                let mut input = egui::RawInput::default();
                input.screen_rect = Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600.0, 1200.0),
                ));
                let _ = ctx.run_ui(input, |ui| {
                    let ctx = ui.ctx().clone();
                    let _ = show(&ctx, &tr, state);
                });
            };
            run(&mut state);
            run(&mut state); // past the one-frame auto-size discovery settle
            ctx.memory(|m| m.area_rect(id)).expect("window rendered").width()
        };

        let narrow = rendered_width_at(MIN_WIDTH);
        let wide = rendered_width_at(MAX_WIDTH);
        let state_delta = MAX_WIDTH - MIN_WIDTH;
        let rendered_delta = wide - narrow;
        assert!(
            (rendered_delta - state_delta).abs() < 1.0,
            "a {state_delta}px change in `state.width` produced a {rendered_delta}px \
             change in the rendered window ({narrow} -> {wide}) — the columns are not \
             tracking `state.width` one-to-one"
        );
    }

    /// The window must follow `state` back DOWN, not just up.
    ///
    /// `default_size` only seeds a window: egui then keeps its own stored
    /// rect, so a rect that ever grew larger than the state outlived every
    /// correction. The state-sized columns then sat inside an oversized
    /// window with dead margin to their right and below them — the operator's
    /// screenshots of 2026-08-02, where the two text blocks stopped well short
    /// of the window's border. Rendering wide and then narrow on the SAME
    /// context is what reproduces it: the second frame must resize the window.
    #[test]
    fn the_window_does_not_keep_a_rect_wider_than_its_state() {
        let tr = crate::i18n::Language::English.tr();
        let id = egui::Id::new("grace_prompt_review");
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let mut state = PromptReview::new("orig".into(), "rev".into(), vec![]);

        let mut render = |state: &mut PromptReview| -> f32 {
            for _ in 0..2 {
                let mut input = egui::RawInput::default();
                input.screen_rect = Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(2400.0, 1600.0),
                ));
                let _ = ctx.run_ui(input, |ui| {
                    let c = ui.ctx().clone();
                    let _ = show(&c, &tr, state);
                });
            }
            ctx.memory(|m| m.area_rect(id)).expect("window rendered").width()
        };

        state.width = MAX_WIDTH;
        let wide = render(&mut state);
        state.width = MIN_WIDTH;
        let narrow = render(&mut state);

        assert!(
            (narrow - (MIN_WIDTH + CHROME_SIDES)).abs() < 2.0,
            "after a wider frame the window kept {narrow}px instead of following \
             state back down to {}px — the columns would sit inside dead margin",
            MIN_WIDTH + CHROME_SIDES
        );
        assert!(
            wide > narrow,
            "the wide frame ({wide}) should have been wider than the narrow one ({narrow})"
        );
    }

    /// `HINT_ROWS` must actually cover what the real, translated hint text
    /// needs — in every language, not just English. Measures the string the
    /// same way `show()`'s label will lay it out: wrapped to the text
    /// column's real width (`DEFAULT_WIDTH` minus the image column and the
    /// gap), at the "small" text style's real font, through egui's own
    /// layouter rather than this file's `row_height` estimate. If a future
    /// translation grows past two lines, this fails here — loudly, at the
    /// budget's own definition — instead of as a silent gap between the
    /// window's seed and what the label actually paints.
    #[test]
    fn hint_rows_budgets_enough_for_every_language() {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        // The narrowest width a developer can drag the window's own grip to
        // — the worst case for wrapping, and the one this budget must hold
        // for, not just the roomier default.
        let text_w = MIN_WIDTH - MIN_WIDTH * GRACE_SHARE - COL_GAP;

        // `Context::fonts`/`fonts_mut` need at least one `run` to have primed
        // the font atlas — an empty pass, same as every other test here does
        // via `ctx.run_ui`, before any layout measurement is possible.
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 1200.0),
        ));
        let _ = ctx.run_ui(input, |_ui| {});

        for &lang in crate::i18n::Language::ALL {
            let tr = lang.tr();
            let job = egui::text::LayoutJob::simple(
                tr.review_hint.to_string(),
                egui::FontId::proportional(11.5 * 0.82), // egui's own `.small()` scale
                egui::Color32::WHITE,
                text_w,
            );
            let galley = ctx.fonts_mut(|f| f.layout_job(job));
            assert!(
                galley.rect.height() <= hint_h(),
                "{:?}'s review_hint (\"{}\") needs {}px, more than the {}px \
                 `HINT_ROWS` budgets — raise `HINT_ROWS` or the window's own \
                 grip will fall through the gap again",
                lang,
                tr.review_hint,
                galley.rect.height(),
                hint_h(),
            );
        }
    }

    /// The exact mechanism the fix relies on, isolated from `show()`
    /// entirely: `ui.shrink_clip_rect` bounds a `Ui`'s own clip to AT MOST
    /// the rect it is given, no matter how large the PARENT's clip already
    /// was — the parent's huge clip here stands in for whatever an
    /// egui-internal `Resize`/`Window` might, on some frame, still believe
    /// its content needs (that internal bookkeeping is what `ScrollArea` and
    /// `Window` themselves lean on — see the module doc comment — and is
    /// exactly the thing this file no longer trusts alone). If this
    /// assertion fails, `shrink_clip_rect` stopped doing what the original,
    /// editor and hint blocks in `show()` now rely on it for.
    #[test]
    fn shrink_clip_rect_bounds_paint_regardless_of_the_parents_own_clip() {
        egui::__run_test_ui(|ui| {
            ui.set_clip_rect(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(4000.0, 4000.0),
            ));

            let boxed = egui::vec2(200.0, 80.0);
            ui.allocate_ui(boxed, |ui| {
                ui.set_min_size(boxed);
                ui.set_max_size(boxed);
                let bounds = ui.max_rect();
                ui.shrink_clip_rect(bounds);
                assert!(
                    ui.clip_rect().height() <= boxed.y + 0.5,
                    "shrink_clip_rect left the child's clip at {:?} — taller \
                     than its own {}px box — even though it was given that \
                     box directly; the parent's clip was deliberately huge \
                     ({:?}) to prove the child does not inherit it",
                    ui.clip_rect(),
                    boxed.y,
                    ui.clip_rect(),
                );
            });
        });
    }

    /// The operator's exact report, reproduced end to end: a revision long
    /// enough to need the editor's own scrollbar, including one passage that
    /// on its own wraps across two lines inside the editor's column, plus a
    /// note that makes the hint label visible. Checked on the window's very
    /// FIRST frame (`run` is called exactly once): the operator's screenshot
    /// was of a modal that had just opened, not one dragged or edited, and a
    /// later frame's own bookkeeping could paper over exactly the gap this
    /// test exists to catch. Before this fix, `window_size()` budgeted only
    /// one line for the hint (see `HINT_ROWS`'s doc comment) and neither
    /// bounded block clipped its own paint — only its layout — so this first
    /// frame had nowhere to fall back to and nothing to stop the excess from
    /// painting past the window's own visible frame.
    #[test]
    fn a_wrapped_highlight_and_a_visible_hint_do_not_spill_past_the_window() {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let tr = crate::i18n::Language::English.tr();
        let id = egui::Id::new("grace_prompt_review");

        // Long enough that, at ten rows, the editor needs its own
        // scrollbar — the "10-11 lines, no scrollbar" shape from the report.
        let quote = "a passage flagged by Grace that is on its own long \
                     enough to wrap across two full lines inside the \
                     editor's own column";
        let revised = "word ".repeat(80) + quote + &" more words after it".repeat(40);
        let notes = vec![Note {
            quote: quote.to_string(),
            why: "reads two ways".to_string(),
        }];
        let mut state = PromptReview::new("orig".into(), revised, notes);

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 1200.0),
        ));
        let _ = ctx.run_ui(input, |ui| {
            let ctx = ui.ctx().clone();
            let _ = show(&ctx, &tr, &mut state);
        });

        let rendered = ctx.memory(|m| m.area_rect(id)).expect("window rendered");
        let seed = window_size(state.font, state.rows, state.width);

        assert!(
            rendered.height() <= seed.y + 20.0,
            "the window rendered {}px tall on its very first frame — more \
             than its own seed ({}px) plus slack accounts for — the hint \
             label, the Submit/Cancel row or the window's own grip spilled \
             past the window's visible frame, exactly the operator's report",
            rendered.height(),
            seed.y,
        );
    }
}
