// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The platform's icon catalogue, as a modal the operator picks from — **one
//! implementation**, for every surface that names an icon.
//!
//! It existed only inside the toolbar editor, tangled into that modal's own
//! state, so everywhere else an icon was a name you had to know and TYPE:
//! `folder-open` spelled right, from a catalogue of 660-odd, with a typo
//! costing you the icon and no way to find out what was on offer. The
//! properties pane now opens the same picker the toolbar editor does.

use std::hash::Hash;

/// A picker's own state — which surface opened it, and what has been typed in
/// its search box.
///
/// One of these per panel that offers icons; the `key` says which row is being
/// picked FOR, so a panel with three icon rows needs no three states.
#[derive(Default)]
pub struct IconPickerState {
    /// The row currently being picked for. `None` = the picker is shut.
    key: Option<String>,
    search: String,
    /// Bumped on every open so a fresh window id is used. Re-showing a window
    /// under an id egui already has geometry for reopens it wherever the
    /// operator last dragged it, which reads as the picker not opening at all
    /// when that was off-screen.
    generation: u32,
}

impl IconPickerState {
    /// Open the picker for a named row, clearing whatever was searched for last
    /// time — the previous search belongs to the previous choice.
    pub fn open(&mut self, key: impl Into<String>) {
        self.key = Some(key.into());
        self.search.clear();
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn is_open_for(&self, key: &str) -> bool {
        self.key.as_deref() == Some(key)
    }

    pub fn close(&mut self) {
        self.key = None;
    }
}

/// Show the picker if it is open, and answer with `(key, icon_name)` when the
/// operator chooses one.
///
/// The caller decides what to DO with the choice — write a property, set a
/// toolbar button's icon — which is the whole reason this returns the name
/// rather than writing anything itself.
pub fn show(ctx: &egui::Context, state: &mut IconPickerState) -> Option<(String, String)> {
    let key = state.key.clone()?;
    let mut chosen = None;
    let mut close = false;
    egui::Window::new("Icon")
        .id(egui::Id::new(("icon_picker", state.generation)))
        .collapsible(false)
        .resizable(true)
        .default_size([420.0, 380.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Find:");
                ui.text_edit_singleline(&mut state.search);
            });
            ui.separator();
            if let Some(name) = grid(ui, &state.search) {
                chosen = Some((key.clone(), name));
            }
            ui.separator();
            if ui.button("Close").clicked() {
                close = true;
            }
        });
    if chosen.is_some() || close {
        state.close();
    }
    chosen
}

/// The catalogue itself, filtered by `needle` — every icon drawn at its own
/// size with its name under it, because a name you cannot see is a name you
/// cannot type.
fn grid(ui: &mut egui::Ui, needle: &str) -> Option<String> {
    let needle = needle.trim().to_ascii_lowercase();
    let mut chosen = None;
    egui::ScrollArea::vertical()
        .max_height(300.0)
        .show(ui, |ui| {
            egui::Grid::new("icon_picker_grid").show(ui, |ui| {
                let mut n = 0;
                for name in cobolt_forms::icons::menu_icon_names() {
                    if !needle.is_empty() && !name.to_ascii_lowercase().contains(&needle) {
                        continue;
                    }
                    if cell(ui, name) {
                        chosen = Some(name.to_owned());
                    }
                    n += 1;
                    if n % 5 == 0 {
                        ui.end_row();
                    }
                }
            });
        });
    chosen
}

/// One icon in the grid; `true` when it was clicked.
fn cell(ui: &mut egui::Ui, name: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(72.0, 56.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, 6.0, ui.visuals().widgets.hovered.bg_fill);
    }
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + 20.0),
        egui::Vec2::splat(24.0),
    );
    cobolt_forms::icons::draw_menu_icon(ui.painter(), icon_rect, name, ui.visuals().text_color());
    ui.painter().text(
        egui::pos2(rect.center().x, rect.bottom() - 4.0),
        egui::Align2::CENTER_BOTTOM,
        name,
        egui::FontId::proportional(9.0),
        ui.visuals().weak_text_color(),
    );
    resp.clicked()
}

/// A small square showing what an icon name resolves to, and the two buttons
/// that go with it: **pick** opens the catalogue, **✕** clears the name back to
/// the platform's own default.
///
/// Returns `true` when the pick button was pressed — the caller opens the
/// picker, since only it knows which key it is picking for.
pub fn preview_row(ui: &mut egui::Ui, current: &str, fallback: &str) -> IconRowAction {
    let mut action = IconRowAction::None;
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
        // An empty name is not "no icon" — it is "the platform's own", which is
        // what the tree actually draws. Showing nothing there would say the row
        // was off.
        let shown = if current.trim().is_empty() {
            fallback
        } else {
            current.trim()
        };
        cobolt_forms::icons::draw_menu_icon(
            ui.painter(),
            egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(16.0)),
            shown,
            ui.visuals().text_color(),
        );
        if ui
            .add(egui::Button::new("…").small())
            .on_hover_text("Choose an icon")
            .clicked()
        {
            action = IconRowAction::Pick;
        }
        // Only offered when there is something to clear — a ✕ that does nothing
        // is a button that lies about the state.
        if !current.trim().is_empty()
            && ui
                .add(egui::Button::new("✕").small())
                .on_hover_text("Use the default icon")
                .clicked()
        {
            action = IconRowAction::Clear;
        }
    });
    action
}

/// What an icon row's buttons asked for.
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum IconRowAction {
    None,
    /// Open the catalogue for this row.
    Pick,
    /// Put the row back to the platform's own default.
    Clear,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ONE state serves a pane with several icon rows, because it carries WHICH
    /// row is being picked for. Three rows used to need three pickers, which is
    /// half the reason there was none here at all.
    #[test]
    fn one_state_tells_its_rows_apart() {
        let mut st = IconPickerState::default();
        assert!(!st.is_open_for("ParentIcon"), "shut to start with");

        st.open("ParentIcon");
        assert!(st.is_open_for("ParentIcon"));
        assert!(
            !st.is_open_for("LeafIcon"),
            "the OTHER rows must not think the picker is theirs"
        );

        st.open("LeafIcon");
        assert!(st.is_open_for("LeafIcon"));
        assert!(!st.is_open_for("ParentIcon"), "opening retargets it");

        st.close();
        assert!(!st.is_open_for("LeafIcon"));
    }

    /// Every open takes a new window id. Re-showing under an id egui already
    /// holds geometry for reopens the window wherever it was last dragged —
    /// which, if that was off-screen, reads as the picker not opening at all.
    #[test]
    fn every_open_gets_a_fresh_window_id() {
        let mut st = IconPickerState::default();
        let first = st.generation;
        st.open("a");
        let second = st.generation;
        st.close();
        st.open("a");
        assert_ne!(first, second, "opening moves the generation on");
        assert_ne!(second, st.generation, "and so does opening it again");
    }

    /// A search belongs to the choice being made, not to the picker.
    #[test]
    fn a_new_choice_starts_from_an_empty_search() {
        let mut st = IconPickerState::default();
        st.open("ParentIcon");
        st.search.push_str("fold");
        st.open("LeafIcon");
        assert!(
            st.search.is_empty(),
            "the last row's search must not narrow this row's catalogue"
        );
    }
}
