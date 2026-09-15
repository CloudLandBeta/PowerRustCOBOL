// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Choose the project's default indexed-file engine, with the trade-offs shown
//! at the point of choice (operator, 2026-09-14).
//!
//! The setting governs **creation** only. A file that already exists names its
//! engine in its own container magic and always opens with that one, so nothing
//! chosen here can put existing data out of reach — which is what makes this
//! safe to expose as a project setting rather than a migration.
//!
//! The two engines differ in one respect no program can work around, and it is
//! the reason this dialog exists: how many processes may hold a file open.
//! PRCIDXD1 admits concurrent readers and does not protect concurrent writers;
//! redb is crash-safe and admits exactly one process. Neither is the right
//! answer for every file, so the developer picks.

use eframe::egui;

use crate::i18n::Tr;
use crate::theme::Theme;

const DEFAULT_W: f32 = 640.0;
const DEFAULT_H: f32 = 520.0;
const MIN_W: f32 = 480.0;
const MIN_H: f32 = 340.0;
const MAX_W: f32 = 1200.0;
const MAX_H: f32 = 1100.0;
const GRIP: f32 = 14.0;
const GRIP_INSET: f32 = 3.0;

/// The engine ids as they are stored in `cobolt.toml` and passed to the
/// runtime. Empty means "whatever the runtime's own default is".
pub const ENGINE_DEFAULT: &str = "";
pub const ENGINE_PRCIDXD1: &str = "prcidxd1";
pub const ENGINE_REDB: &str = "redb";

/// What the modal wants the app to do once it closes.
#[derive(Debug, Clone, Default)]
pub struct IndexedEngineAction {
    /// The developer pressed Save: store this engine id on the project.
    pub save: Option<String>,
}

pub struct IndexedEngineModal {
    pub open: bool,
    /// The window's size. Owned here and written by exactly one thing — the
    /// grip's drag delta. See the note on `resizable(false)` below.
    size: egui::Vec2,
    /// The engine id under consideration; committed only on Save.
    choice: String,
}

impl IndexedEngineModal {
    pub fn new(current: &str) -> Self {
        Self {
            open: true,
            size: egui::vec2(DEFAULT_W, DEFAULT_H),
            choice: normalize(current),
        }
    }

    /// One grip, in its own foreground `Area`, as the only writer of
    /// `self.size` — the window itself never negotiates its own rectangle.
    fn resize_grip(&mut self, ctx: &egui::Context, theme: &Theme, window: egui::Rect) {
        let corner = window.max - egui::vec2(GRIP_INSET, GRIP_INSET);
        let origin = corner - egui::vec2(GRIP, GRIP);
        egui::Area::new(egui::Id::new("indexed_engine_resize_grip"))
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
                let col = if response.hovered() || response.dragged() {
                    theme.accent
                } else {
                    theme.text_dim
                };
                let p = ui.painter();
                for i in 0..3 {
                    let o = 4.0 * i as f32;
                    p.line_segment(
                        [
                            egui::pos2(rect.right() - o, rect.bottom()),
                            egui::pos2(rect.right(), rect.bottom() - o),
                        ],
                        egui::Stroke::new(1.5, col),
                    );
                }
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
                }
            });
    }

    pub fn show(&mut self, ctx: &egui::Context, theme: &Theme, tr: &Tr) -> IndexedEngineAction {
        let mut action = IndexedEngineAction::default();
        let mut open = self.open;

        let window = egui::Window::new(
            egui::RichText::new(tr.idx_engine_title).size(18.0).strong(),
        )
        .id(egui::Id::new("indexed_engine_modal"))
        .open(&mut open)
        .collapsible(false)
        // NEVER `resizable(true)`. egui would negotiate this window's rectangle
        // against its contents every frame, and the pros/cons panel reports the
        // width it would like — so the two push each other outward and the
        // window walks to the screen edge on its own. The size is ours, exact,
        // and only the grip changes it.
        .resizable(false)
        .fixed_size(self.size)
        .default_pos([80.0, 80.0])
        .show(ctx, |ui| {
            // Lay children out from the STORED size, never from what the Ui
            // offers: a child measured against available space feeds its own
            // width back into the window and the pair inflate together.
            let margin = ui.style().spacing.window_margin.sum().x;
            let inner_w = (self.size.x - margin).max(120.0);

            ui.label(
                egui::RichText::new(tr.idx_engine_intro)
                    .color(theme.text_dim)
                    .italics(),
            );
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            ui.label(egui::RichText::new(tr.idx_engine_label).strong());
            ui.add_space(4.0);
            ui.radio_value(&mut self.choice, ENGINE_DEFAULT.to_owned(), tr.idx_engine_builtin);
            ui.radio_value(&mut self.choice, ENGINE_PRCIDXD1.to_owned(), "PRCIDXD1");
            ui.radio_value(&mut self.choice, ENGINE_REDB.to_owned(), "redb");

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            // The trade-offs of whatever is selected. The built-in default
            // shows the engine it currently resolves to, so the panel is never
            // blank and never leaves the developer guessing what they get.
            let showing = if self.choice == ENGINE_REDB {
                ENGINE_REDB
            } else {
                ENGINE_PRCIDXD1
            };
            let (pros, cons, name) = if showing == ENGINE_REDB {
                (tr.idx_engine_redb_pros, tr.idx_engine_redb_cons, "redb")
            } else {
                (
                    tr.idx_engine_prc_pros,
                    tr.idx_engine_prc_cons,
                    "PRCIDXD1",
                )
            };

            egui::ScrollArea::vertical()
                .max_width(inner_w)
                .show(ui, |ui| {
                    ui.set_width(inner_w);
                    ui.label(egui::RichText::new(name).strong().size(15.0));
                    ui.add_space(6.0);

                    ui.label(
                        egui::RichText::new(tr.idx_engine_pros)
                            .strong()
                            .color(theme.accent),
                    );
                    ui.label(pros);
                    ui.add_space(8.0);

                    ui.label(
                        egui::RichText::new(tr.idx_engine_cons)
                            .strong()
                            .color(theme.text_dim),
                    );
                    ui.label(cons);

                    // The one obligation a developer takes on by choosing
                    // PRCIDXD1. Shown only for that engine, because for redb it
                    // is not true — the engine refuses the second opener itself.
                    if showing == ENGINE_PRCIDXD1 {
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(tr.idx_engine_note_lock)
                                .color(theme.warn)
                                .strong(),
                        );
                    }
                });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(tr.btn_save).clicked() {
                    action.save = Some(self.choice.clone());
                    self.open = false;
                }
                if ui.button(tr.btn_cancel).clicked() {
                    self.open = false;
                }
            });
        });

        if let Some(r) = window {
            self.resize_grip(ctx, theme, r.response.rect);
        }
        // `open` is the window's own close button; Save/Cancel already cleared
        // `self.open`, so never resurrect it here.
        if !open {
            self.open = false;
        }
        action
    }
}

/// Accept what is on the project and reduce it to one of the three ids.
///
/// An unknown value — a hand-edited `cobolt.toml`, or a project written by a
/// later version that knows an engine this one does not — reads as the built-in
/// default rather than being silently rewritten to one of ours.
fn normalize(id: &str) -> String {
    match id.trim().to_ascii_lowercase().as_str() {
        "redb" => ENGINE_REDB.to_owned(),
        "prcidxd1" | "prcidx1" | "rust" | "native" => ENGINE_PRCIDXD1.to_owned(),
        _ => ENGINE_DEFAULT.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_spellings_reduce_to_the_three_stored_ids() {
        assert_eq!(normalize("redb"), ENGINE_REDB);
        assert_eq!(normalize("  REDB "), ENGINE_REDB);
        assert_eq!(normalize("prcidxd1"), ENGINE_PRCIDXD1);
        assert_eq!(normalize("Rust"), ENGINE_PRCIDXD1);
        assert_eq!(normalize("native"), ENGINE_PRCIDXD1);
        assert_eq!(normalize(""), ENGINE_DEFAULT);
    }

    /// An engine this version does not know must read as the built-in default,
    /// not be rewritten to one of ours — a project written by a later version
    /// keeps its own answer when it is opened there again.
    #[test]
    fn an_unknown_engine_reads_as_the_built_in_default() {
        assert_eq!(normalize("some-future-engine"), ENGINE_DEFAULT);
        assert_eq!(normalize("fujitsu-native"), ENGINE_DEFAULT);
    }
}
