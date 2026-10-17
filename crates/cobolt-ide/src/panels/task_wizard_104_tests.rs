// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 104 — the Task Wizard where it touches the IDE's own surfaces.

use crate::i18n::Language;
use crate::panels::designer::MenuEditorModal;
use cobolt_forms::menu::{MenuDefinition, MenuItem};

fn items(m: &MenuEditorModal) -> Vec<(String, Option<String>, usize)> {
    m.def
        .menu
        .iter()
        .map(|i| (i.label.clone(), i.action.clone(), i.items.len()))
        .collect()
}

/// R25/R26 — *Do this for me* fills an **empty** SideMenu with Home, and an empty MenuBar
/// with File (Exit), Edit (Copy, Paste) and Help (About), in the IDE's language, every id
/// unique.
#[test]
fn starter_items_fill_an_empty_menu_in_the_ides_language() {
    for lang in [Language::English, Language::Portuguese, Language::Japanese] {
        let tr = lang.tr();
        let mut side = MenuEditorModal::new("Side1".into(), MenuDefinition::new()).for_side_menu(true);
        assert!(side.add_starter_items(&tr));
        assert_eq!(items(&side), vec![(tr.task_wizard_starter_home.to_owned(), Some("home".to_owned()), 0)]);

        let mut bar = MenuEditorModal::new("Bar1".into(), MenuDefinition::new()).for_side_menu(false);
        assert!(bar.add_starter_items(&tr));
        assert_eq!(
            items(&bar),
            vec![
                (tr.task_wizard_starter_file.to_owned(), None, 1),
                (tr.task_wizard_starter_edit.to_owned(), None, 2),
                (tr.task_wizard_starter_help.to_owned(), None, 1),
            ]
        );
        assert_eq!(bar.def.menu[0].items[0].action.as_deref(), Some("close-application"));
        let ids = cobolt_forms::menu::all_item_ids(&bar.def.menu);
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "every starter item has its own id");
    }
}

/// R26 — never into a menu the developer has already started.
#[test]
fn starter_items_never_touch_a_menu_that_has_items() {
    let tr = Language::English.tr();
    let mut def = MenuDefinition::new();
    def.menu.push(MenuItem::new_action("mine", "Mine"));
    let mut m = MenuEditorModal::new("Bar1".into(), def).for_side_menu(false);
    let before = items(&m);
    assert!(!m.add_starter_items(&tr));
    assert_eq!(items(&m), before, "the developer's menu is untouched");
}

/// The glyphs the wizard's chrome uses exist in the IDE's fonts, so no button shows a box.
#[test]
fn the_wizards_glyphs_are_in_the_ides_fonts() {
    let ctx = egui::Context::default();
    ctx.set_fonts(crate::fonts::base_font_definitions());
    let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
    out.textures_delta.clear();
    let font = egui::FontId::proportional(14.0);
    // The switch 🗺, Do this for me ★, Next ⏵ / Back ⏴, done ✓, dismiss ✕, the ▾, and the
    // summary's • and the balloon head's ·. (🎯, ✨, ▶ and ◀ were the first choices; the
    // IDE's base fonts have none of them.)
    for g in ['🗺', '★', '⏵', '⏴', '✓', '✕', '▾', '·', '•'] {
        let ok = ctx.fonts_mut(|f| f.has_glyph(&font, g));
        assert!(ok, "no glyph for {g:?}");
    }
}
