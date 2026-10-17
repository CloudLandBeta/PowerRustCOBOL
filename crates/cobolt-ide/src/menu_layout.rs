// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The IDE's menu bar, declared as data** (spec 099).
//!
//! The bar is **File, Settings, View, Help**, and **Run** joins it — between Settings and View —
//! only while a project or a source file is active. Every item that configures something sits
//! under **Settings**, wherever it used to live: *Configure Claude Code* and *Reindex Knowledge
//! Bases* came from File; *Coding Agent Settings*, *Debug Settings…* and *Platform SDK Location…*
//! from Help. *Build Folder…* (spec 100) was born here.
//!
//! Why data and not just code in `app.rs`: a `CoboltApp` cannot be built without an eframe window,
//! so a menu written only as widgets cannot be tested. Declared here, its order and its membership
//! can — and "a configuration item added to Help later" becomes a failing test instead of a
//! surprise. `app.rs` draws File, Settings and Help by walking [`entries`]; what each item *does*
//! is still written there, next to the state it touches.

/// A top-level menu of the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Menu {
    File,
    Settings,
    Run,
    View,
    Help,
}

/// One item of File, Settings or Help. Run and View keep their own code in `app.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    // File
    NewProject,
    OpenProject,
    SaveProject,
    PackageProject,
    BuildBinary,
    Save,
    Quit,
    // Settings
    ConfigureClaudeCode,
    CodingAgentSettings,
    DebugSettings,
    PlatformSdkLocation,
    BuildFolder,
    ReindexKnowledgeBases,
    // Help
    Documentation,
    Examples,
    IdeWalkthrough,
    TaskWizard,
    About,
}

impl Item {
    /// Every item, so a test can say "each one is in exactly one menu".
    pub const ALL: [Item; 18] = [
        Item::NewProject,
        Item::OpenProject,
        Item::SaveProject,
        Item::PackageProject,
        Item::BuildBinary,
        Item::Save,
        Item::Quit,
        Item::ConfigureClaudeCode,
        Item::CodingAgentSettings,
        Item::DebugSettings,
        Item::PlatformSdkLocation,
        Item::BuildFolder,
        Item::ReindexKnowledgeBases,
        Item::Documentation,
        Item::Examples,
        Item::IdeWalkthrough,
        Item::TaskWizard,
        Item::About,
    ];

    /// Whether the item is about configuring something — and so belongs under Settings.
    ///
    /// `IdeWalkthrough` is not: it replays the IDE tour, it configures nothing (spec 099 Q1). The
    /// `match` has no wildcard on purpose: a new item cannot be added without saying which side of
    /// this line it is on.
    pub fn is_configuration(self) -> bool {
        match self {
            Item::ConfigureClaudeCode
            | Item::CodingAgentSettings
            | Item::DebugSettings
            | Item::PlatformSdkLocation
            | Item::BuildFolder
            | Item::ReindexKnowledgeBases => true,
            Item::NewProject
            | Item::OpenProject
            | Item::SaveProject
            | Item::PackageProject
            | Item::BuildBinary
            | Item::Save
            | Item::Quit
            | Item::Documentation
            | Item::Examples
            | Item::IdeWalkthrough
            | Item::TaskWizard
            | Item::About => false,
        }
    }
}

/// An entry of a menu: an item, or the line between two groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Item(Item),
    Separator,
}

use Entry::{Item as I, Separator as S};

/// File: the project and the build. Nothing here configures anything.
pub const FILE: &[Entry] = &[
    I(Item::NewProject),
    I(Item::OpenProject),
    I(Item::SaveProject),
    I(Item::PackageProject),
    I(Item::BuildBinary),
    S,
    I(Item::Save),
    S,
    I(Item::Quit),
];

/// Settings: the coding agent, then the machine (debug, SDK, build folder), then the knowledge base.
pub const SETTINGS: &[Entry] = &[
    I(Item::ConfigureClaudeCode),
    I(Item::CodingAgentSettings),
    S,
    I(Item::DebugSettings),
    I(Item::PlatformSdkLocation),
    I(Item::BuildFolder),
    S,
    I(Item::ReindexKnowledgeBases),
];

/// Help: the documentation, the examples, the tour, and what this is.
pub const HELP: &[Entry] = &[
    I(Item::Documentation),
    I(Item::Examples),
    S,
    I(Item::IdeWalkthrough),
    I(Item::TaskWizard),
    S,
    I(Item::About),
];

/// The entries `app.rs` walks to draw `menu`. Run and View are not declared here — their items
/// are their own code — so they answer with none.
pub fn entries(menu: Menu) -> &'static [Entry] {
    match menu {
        Menu::File => FILE,
        Menu::Settings => SETTINGS,
        Menu::Help => HELP,
        Menu::Run | Menu::View => &[],
    }
}

/// The bar, left to right. Run is there only while a project or a source file is active — the
/// operator's "basic" bar is File, Settings, View, Help.
pub fn top_level(has_active: bool) -> Vec<Menu> {
    let mut bar = vec![Menu::File, Menu::Settings];
    if has_active {
        bar.push(Menu::Run);
    }
    bar.push(Menu::View);
    bar.push(Menu::Help);
    bar
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;
    use std::collections::HashMap;

    fn names(entries: &[Entry]) -> Vec<Item> {
        entries.iter().filter_map(|e| if let Entry::Item(i) = e { Some(*i) } else { None }).collect()
    }

    /// AC1 (R1, R2) — File, Settings, View, Help; Run joins between Settings and View.
    #[test]
    fn the_bar_is_file_settings_view_help_and_run_joins_only_while_something_is_active() {
        let idle = top_level(false);
        let active = top_level(true);
        println!("\n  nothing open : {idle:?}\n  project open : {active:?}");
        assert_eq!(idle, [Menu::File, Menu::Settings, Menu::View, Menu::Help]);
        assert_eq!(active, [Menu::File, Menu::Settings, Menu::Run, Menu::View, Menu::Help]);
    }

    /// AC2 (R3, R4, R8; spec 100 R1) — Settings holds the six items in the order the spec gives, and nothing
    /// that configures is left in File or Help.
    #[test]
    fn settings_holds_every_configuration_item_in_order_and_file_and_help_hold_none() {
        assert_eq!(
            names(SETTINGS),
            [
                Item::ConfigureClaudeCode,
                Item::CodingAgentSettings,
                Item::DebugSettings,
                Item::PlatformSdkLocation,
                Item::BuildFolder,
                Item::ReindexKnowledgeBases
            ]
        );
        for (menu, list) in [(Menu::File, FILE), (Menu::Help, HELP)] {
            let stray: Vec<_> = names(list).into_iter().filter(|i| i.is_configuration()).collect();
            assert!(stray.is_empty(), "{menu:?} still holds configuration: {stray:?}");
        }
        // The other way: nothing under Settings is not a configuration.
        assert!(names(SETTINGS).into_iter().all(Item::is_configuration));
        // …and every configuration item is under Settings.
        let in_settings = names(SETTINGS);
        let missing: Vec<_> =
            Item::ALL.iter().filter(|i| i.is_configuration() && !in_settings.contains(i)).collect();
        assert!(missing.is_empty(), "configuration items outside Settings: {missing:?}");
        println!(
            "\n  File {} items, Settings {} items, Help {} items; the walkthrough stays in Help: {}",
            names(FILE).len(),
            names(SETTINGS).len(),
            names(HELP).len(),
            names(HELP).contains(&Item::IdeWalkthrough)
        );
    }

    /// AC2 — each item is in exactly one menu, and no menu starts or ends with a separator or has
    /// two in a row.
    #[test]
    fn every_item_is_in_exactly_one_menu_and_the_separators_are_tidy() {
        let mut seen: HashMap<Item, Vec<Menu>> = HashMap::new();
        for menu in [Menu::File, Menu::Settings, Menu::Help] {
            let list = entries(menu);
            assert_ne!(list.first(), Some(&Entry::Separator), "{menu:?} starts with a separator");
            assert_ne!(list.last(), Some(&Entry::Separator), "{menu:?} ends with a separator");
            assert!(
                !list.windows(2).any(|w| w == [Entry::Separator, Entry::Separator]),
                "{menu:?} has two separators in a row"
            );
            for item in names(list) {
                seen.entry(item).or_default().push(menu);
            }
        }
        for item in Item::ALL {
            let places = seen.get(&item).cloned().unwrap_or_default();
            assert_eq!(places.len(), 1, "{item:?} is in {places:?}, not in exactly one menu");
        }
        assert_eq!(seen.len(), Item::ALL.len(), "a menu holds an item that `Item::ALL` does not list");
        assert!(entries(Menu::Run).is_empty() && entries(Menu::View).is_empty());
    }

    /// AC3 (R6) — the menu's name exists in the six languages, and the six differ.
    #[test]
    fn the_settings_menu_is_named_in_six_languages() {
        let mut names: Vec<(Language, &str)> = Vec::new();
        for &lang in Language::ALL.iter() {
            let name = lang.tr().menu_settings;
            assert!(!name.trim().is_empty(), "{lang:?} has no name for the Settings menu");
            names.push((lang, name));
        }
        println!("\n  {}", names.iter().map(|(l, n)| format!("{l:?}: {n}")).collect::<Vec<_>>().join(" · "));
        let mut distinct: Vec<&str> = names.iter().map(|(_, n)| *n).collect();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), 6, "the six names must differ: {names:?}");
    }

    /// Spec 104 R6 — the Task Wizard switch is in Help, right after the IDE Walkthrough, for
    /// the same reason: it switches a way of working, it configures nothing.
    #[test]
    fn the_task_wizard_sits_in_help_right_after_the_walkthrough() {
        let help = names(HELP);
        let at = help.iter().position(|i| *i == Item::IdeWalkthrough).expect("the walkthrough");
        assert_eq!(help.get(at + 1), Some(&Item::TaskWizard));
        assert!(!Item::TaskWizard.is_configuration());
    }
}
