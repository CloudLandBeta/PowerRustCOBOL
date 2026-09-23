// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Ids for the items of a form's menus and toolbars: four lowercase letters
//! from `tiny_id`, fixed once given, and unique within the window — a handler
//! reads them back from `SelectedItemId` or `LastButton`, so two items of the
//! same window must never share one.

use cobolt_forms::{menu, toolbar::ToolbarDef, ControlType, Form};
use std::collections::HashSet;
use std::path::Path;

/// Whether `id` is in the generated format: four letters `a`–`z`.
pub fn is_generated(id: &str) -> bool {
    id.len() == 4 && id.bytes().all(|b| b.is_ascii_lowercase())
}

/// A generated id that is not in `taken`.
pub fn fresh_id(taken: &HashSet<String>) -> String {
    let mut generator = tiny_id::ShortCodeGenerator::with_alphabet(('a'..='z').collect(), 4);
    loop {
        let id = generator.next_string();
        if !taken.contains(&id) {
            return id;
        }
    }
}

/// Keep `id` if it is generated and not yet taken, otherwise replace it; then
/// mark it taken. What converts an older menu or toolbar when its editor opens.
pub fn claim(id: &mut String, taken: &mut HashSet<String>) {
    if !is_generated(id) || taken.contains(id.as_str()) {
        *id = fresh_id(taken);
    }
    taken.insert(id.clone());
}

/// Every menu-item, toolbar-group and toolbar-button id on the form, except
/// those of `except` — the control whose editor is open, which owns its own.
pub fn window_ids(form: &Form, cfrm_dir: Option<&Path>, except: &str) -> HashSet<String> {
    let mut ids = HashSet::new();
    for c in form.controls.iter().filter(|c| c.id != except) {
        match c.control_type {
            ControlType::MenuBar | ControlType::SideMenu => {
                if let Some(def) =
                    cfrm_dir.and_then(|d| menu::load_menu(&menu::menu_yaml_path(d, &c.id)).ok())
                {
                    ids.extend(menu::all_item_ids(&def.menu));
                }
            }
            ControlType::ToolBar => {
                let def = ToolbarDef::from_control(c);
                for g in &def.groups {
                    ids.insert(g.id.clone());
                    ids.extend(g.buttons.iter().map(|b| b.id.clone()));
                }
            }
            _ => {}
        }
    }
    ids
}

/// The read-only id with its Copy button, for an editor's properties pane.
pub fn id_with_copy(ui: &mut egui::Ui, tr: &crate::i18n::Tr, id: &str) {
    ui.label(egui::RichText::new(id).monospace());
    if ui
        .small_button(tr.menu_copy_id)
        .on_hover_text(tr.menu_copy_id_hover)
        .clicked()
    {
        ui.ctx().copy_text(id.to_owned());
    }
}
