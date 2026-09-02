// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Rebuild a SideMenu's **Samples** subtree from the demo forms on disk.
//!
//! Every `.cfrm` in the project becomes an entry, grouped into folders named
//! after the toolbox sections and ordered exactly as the toolbox orders its
//! controls — so finding the ListBox demo means looking where ListBox sits in
//! the toolbox, not scanning an alphabetical list.
//!
//! Written through `save_menu`, never as text: the file carries an HMAC over
//! its own content, and a hand-edited menu fails to load with
//! `MenuError::HmacMismatch` rather than showing the edit.
//!
//! **Nothing is duplicated and nothing is lost.** An existing entry is matched
//! by the form it opens and keeps its own label, icon and flags — a label
//! somebody wrote by hand ("Controls (Elegance)") is theirs, and this only
//! decides where it sits. Forms with no entry are added; entries pointing at a
//! form that no longer exists are reported and kept, because a menu item is
//! developer content and this is not the tool that deletes it.
//!
//! ```text
//! cargo run -p cobolt-forms --example rebuild_samples_menu -- <project-forms-dir> [--dry-run]
//! ```

use cobolt_forms::menu::{load_menu, save_menu, BadgeStyle, MenuItem, MenuItemType};
use cobolt_forms::{load_form, save_form};
use cobolt_forms::model::FormFormat;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The toolbox's own section order and display names (`panels/toolbox.rs`).
const SECTIONS: &[(&str, &str)] = &[
    ("Common", "Common"),
    ("Container", "Containers"),
    ("Data", "Data"),
    ("Graphics", "Graphics"),
    ("Menu", "Menus & Bars"),
    ("NonVisual", "Non-Visual"),
    ("Charts", "Charts"),
];

/// Every toolbox control, in toolbox order, as `(section, control, icon)`.
///
/// The icon names are the ones the menu editor already uses for these controls,
/// so a rebuilt menu looks like a hand-built one.
const CONTROLS: &[(&str, &str, &str)] = &[
    ("Common", "Button", "control-button"),
    ("Common", "Label", "control-label"),
    ("Common", "TextBox", "type-text"),
    ("Common", "CheckBox", "control-check-box"),
    ("Common", "RadioButton", "control-radio-button"),
    ("Common", "ComboBox", "control-combo-box"),
    ("Common", "ListBox", "control-list-box"),
    ("Common", "NumericUpDown", "control-numeric-up-down"),
    ("Common", "DateTimePicker", "control-date-time-picker"),
    ("Common", "Switch", "control-switch"),
    ("Common", "FileDropZone", "control-file-drop-zone"),
    ("Common", "Knob", "control-knob"),
    ("Common", "Gauge", "control-gauge"),
    ("Common", "Maps", "map-pin"),
    ("Container", "GroupBox", "control-group-box"),
    ("Container", "Panel", "control-panel"),
    ("Container", "TabControl", "control-tab-control"),
    ("Container", "Splitter", "control-splitter"),
    ("Data", "DataGrid", "control-data-grid"),
    ("Data", "TreeView", "control-tree-view"),
    ("Graphics", "PictureBox", "control-picture-box"),
    ("Graphics", "Animator", "control-animator"),
    ("Graphics", "ProgressBar", "control-progress-bar"),
    ("Graphics", "Slider", "control-slider"),
    ("Graphics", "Line", "control-line"),
    ("Graphics", "Shape", "control-shape"),
    ("Menu", "MenuBar", "control-menu-bar"),
    ("Menu", "SideMenu", "control-side-menu"),
    ("Menu", "ToolBar", "control-tool-bar"),
    ("Menu", "StatusBar", "control-status-bar"),
    ("NonVisual", "Timer", "control-timer"),
    ("NonVisual", "AgentObject", "control-agent"),
    ("NonVisual", "RestClient", "api"),
    ("NonVisual", "SqlDatabase", "control-sql-database"),
    ("NonVisual", "IndexedFile", "control-indexed-file"),
    ("NonVisual", "WebSearch", "control-web-search"),
    ("NonVisual", "Snackbar", "control-snackbar"),
    ("Charts", "BarChart", "control-bar-chart"),
    ("Charts", "LineChart", "control-line-chart"),
    ("Charts", "PieChart", "control-pie-chart"),
    ("Charts", "AreaChart", "control-area-chart"),
    ("Charts", "ScatterChart", "control-scatter-chart"),
    ("Charts", "DonutChart", "control-donut-chart"),
];

/// Demo forms whose name does not spell out the control they demonstrate.
///
/// Kept as an explicit list rather than guessed: `maps-demo` is the Maps
/// sample, and no amount of string surgery would say so.
const SPECIAL: &[(&str, &str, &str, &str)] = &[
    // (form stem, section, control, label)
    ("maps-demo", "Common", "Maps", "Maps"),
    ("restapi-form", "NonVisual", "RestClient", "REST API"),
    ("charts-form", "Charts", "BarChart", "Charts (all six types)"),
    ("agent-form", "NonVisual", "AgentObject", "Agent"),
    ("sidebar-demo-form", "Menu", "SideMenu", "SideMenu"),
];

/// Samples that demonstrate no single toolbox control — a whole-form tour, a
/// Rust interop demo — with the label each should carry.
///
/// They are still samples and still belong in the menu; they simply have no
/// section to sort into, so they get one of their own at the end.
const GENERAL: &[(&str, &str, &str)] = &[
    // (form stem, label, icon)
    ("inner-form1", "Controls (Elegance)", "form"),
    ("inner-form2", "Form 2 (Neumorphic Dark)", "object"),
    ("ferris-says-form", "COBOL + Rust", "plugin"),
];

/// A form's declared `form-format`, read straight from the attribute.
///
/// A menu item loads its form into the shell's ContentPane, and spec 049 R17
/// only permits that for `Embedded` or `Both`. A **missing** attribute is a form
/// written before 049 and counts as `Standalone` — which is how three demos
/// came to be added to the menu and then rejected by the build.
///
/// Read as text rather than by parsing the whole form: this asks the question of
/// every candidate, and a full parse each is not worth one attribute.
fn form_format(path: &Path) -> FormFormat {
    let Ok(text) = std::fs::read_to_string(path) else {
        return FormFormat::Standalone;
    };
    let mut cut = text.len().min(4096);
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    match text[..cut].find("form-format=\"") {
        Some(at) => {
            let rest = &text[at + 13..];
            FormFormat::from_str(rest.split('"').next().unwrap_or(""))
        }
        None => FormFormat::Standalone,
    }
}

fn stem(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dry_run = args.iter().any(|a| a == "--dry-run");
    // Opt-in, never implicit: promoting a form's `form-format` edits the
    // developer's form, not the menu, and that is a different decision from
    // "arrange the samples".
    let make_loadable = args.iter().any(|a| a == "--make-loadable");
    let Some(root) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("usage: rebuild_samples_menu <forms-dir> [--dry-run]");
        std::process::exit(2);
    };
    let root = PathBuf::from(root);

    // Every form on disk, by stem.
    let mut forms: Vec<PathBuf> = Vec::new();
    collect(&root, &mut forms);
    let stems: Vec<String> = forms.iter().map(|p| stem(p)).collect();
    // Only a form the shell can actually load may be a menu target. Refusing
    // here — with a reason — beats writing a menu the build then rejects.
    let mut refused: Vec<String> = Vec::new();

    // Promote the demos that cannot be loaded, when asked. `Both` is strictly
    // more permissive than `Standalone` — the form still opens in its own
    // window — and it is what every other demo in this project already
    // declares; these three simply predate the attribute.
    if make_loadable {
        let shell = |s: &str| s.starts_with("sidebar-form") || s.ends_with(".recovered");
        for path in forms.iter().filter(|p| !shell(&stem(p))) {
            if !matches!(form_format(path), FormFormat::Standalone) {
                continue;
            }
            match load_form(path) {
                Ok(mut form) => {
                    form.form_format = FormFormat::Both;
                    match save_form(&form, path) {
                        Ok(()) => println!("  ~ {} form-format -> Both", stem(path)),
                        Err(e) => eprintln!("  ! {} could not be written: {e}", stem(path)),
                    }
                }
                Err(e) => eprintln!("  ! {} could not be read: {e}", stem(path)),
            }
        }
    }

    // Re-read AFTER any promotion above, so a form just made loadable counts.
    let loadable: std::collections::BTreeSet<String> = forms
        .iter()
        .filter(|p| !matches!(form_format(p), FormFormat::Standalone))
        .map(|p| stem(p))
        .collect();

    // Which menu file to rewrite: the SideMenu beside the shell form.
    let menu_path = root.join("SideMenu-1.menu.yaml");
    let mut def = match load_menu(&menu_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("cannot read {}: {e}", menu_path.display());
            std::process::exit(1);
        }
    };

    // Existing entries, by the form they open, so a hand-written label survives.
    let mut existing: BTreeMap<String, MenuItem> = BTreeMap::new();
    let mut samples_idx = None;
    for (i, top) in def.menu.iter().enumerate() {
        if top.label.trim().eq_ignore_ascii_case("samples") {
            samples_idx = Some(i);
            harvest(&top.items, &mut existing);
        }
    }
    let Some(samples_idx) = samples_idx else {
        eprintln!("no top-level 'Samples' item in {}", menu_path.display());
        std::process::exit(1);
    };

    // Which form demonstrates which control.
    let mut by_control: BTreeMap<(&str, &str), Vec<(String, String)>> = BTreeMap::new();
    let mut general: Vec<(String, String, String)> = Vec::new();
    let mut unclaimed: Vec<String> = Vec::new();
    'form: for s in &stems {
        // The shell itself is not a sample of anything.
        if s.starts_with("sidebar-form") || s.ends_with(".recovered") {
            continue;
        }
        if !loadable.contains(s) {
            refused.push(s.clone());
            continue;
        }
        for (form, sec, ctl, label) in SPECIAL {
            if s == form {
                by_control
                    .entry((sec, ctl))
                    .or_default()
                    .push((s.clone(), (*label).to_owned()));
                continue 'form;
            }
        }
        for (form, label, icon) in GENERAL {
            if s == form {
                general.push((s.clone(), (*label).to_owned(), (*icon).to_owned()));
                continue 'form;
            }
        }
        // `buttons-form` demonstrates the Button, `labels-form` the Label: the
        // demos are named for what they SHOW, which is usually several of the
        // control, so a trailing plural is stripped before matching.
        let key = s.trim_end_matches("-form").replace('-', "").to_ascii_lowercase();
        let singular = key.strip_suffix("es").unwrap_or(key.strip_suffix('s').unwrap_or(&key));
        for (sec, ctl, _) in CONTROLS {
            let name = ctl.to_ascii_lowercase();
            if name == key || name == singular {
                by_control
                    .entry((sec, ctl))
                    // Two demos of one control both belong; the second is
                    // labelled by its file so the pair is tellable apart.
                    .or_default()
                    .push((s.clone(), (*ctl).to_owned()));
                continue 'form;
            }
        }
        unclaimed.push(s.clone());
    }

    // Rebuild: one folder per section, controls in toolbox order.
    let mut folders: Vec<MenuItem> = Vec::new();
    let mut next_id = 1000;
    let (mut kept, mut added) = (0usize, 0usize);
    for (key, display) in SECTIONS {
        let mut items: Vec<MenuItem> = Vec::new();
        for (sec, ctl, icon) in CONTROLS.iter().filter(|(s, _, _)| s == key) {
            let Some(demos) = by_control.get(&(*sec, *ctl)) else {
                continue; // no demo for this control yet
            };
            for (n, (form, label)) in demos.iter().enumerate() {
            let label = if n == 0 {
                label.clone()
            } else {
                format!("{label} ({form})")
            };
            let action = format!("open-form:{form}");
            match existing.remove(&action) {
                Some(prev) => {
                    kept += 1;
                    items.push(prev);
                }
                None => {
                    added += 1;
                    next_id += 1;
                    println!("  + {display} / {label}  ->  {form}");
                    items.push(MenuItem {
                        id: format!("sample-{next_id}"),
                        label: label.clone(),
                        item_type: MenuItemType::Action,
                        icon: Some((*icon).to_owned()),
                        action: Some(action),
                        enabled: true,
                        accelerator: None,
                        preserve_previous_form: false,
                        badge: None,
                        badge_style: BadgeStyle::default(),
                        items: Vec::new(),
                    });
                }
            }
            }
        }
        if items.is_empty() {
            continue;
        }
        next_id += 1;
        folders.push(MenuItem {
            id: format!("sample-folder-{next_id}"),
            label: (*display).to_owned(),
            item_type: MenuItemType::Action,
            icon: Some("folder".to_owned()),
            enabled: true,
            items,
            accelerator: None,
            action: None,
            preserve_previous_form: false,
            badge: None,
            badge_style: BadgeStyle::default(),
        });
    }

    if !general.is_empty() {
        let mut items = Vec::new();
        for (form, label, icon) in &general {
            let action = format!("open-form:{form}");
            match existing.remove(&action) {
                Some(prev) => {
                    kept += 1;
                    items.push(prev);
                }
                None => {
                    added += 1;
                    next_id += 1;
                    println!("  + General / {label}  ->  {form}");
                    items.push(MenuItem {
                        id: format!("sample-{next_id}"),
                        label: label.clone(),
                        item_type: MenuItemType::Action,
                        icon: Some(icon.clone()),
                        action: Some(action),
                        enabled: true,
                        accelerator: None,
                        preserve_previous_form: false,
                        badge: None,
                        badge_style: BadgeStyle::default(),
                        items: Vec::new(),
                    });
                }
            }
        }
        next_id += 1;
        folders.push(MenuItem {
            id: format!("sample-folder-{next_id}"),
            label: "General".to_owned(),
            item_type: MenuItemType::Action,
            icon: Some("folder".to_owned()),
            enabled: true,
            items,
            accelerator: None,
            action: None,
            preserve_previous_form: false,
            badge: None,
            badge_style: BadgeStyle::default(),
        });
    }

    // Entries that pointed at nothing on disk are KEPT, at the end, and named.
    // A menu item is developer content; this is not the tool that deletes it.
    if !existing.is_empty() {
        println!("\n  kept, but their form was not found on disk:");
        let orphans: Vec<MenuItem> = existing
            .into_values()
            .inspect(|i| println!("    ? {}  ({})", i.label, i.action.as_deref().unwrap_or("-")))
            .collect();
        next_id += 1;
        folders.push(MenuItem {
            id: format!("sample-folder-{next_id}"),
            label: "Other".to_owned(),
            item_type: MenuItemType::Action,
            icon: Some("folder".to_owned()),
            enabled: true,
            items: orphans,
            accelerator: None,
            action: None,
            preserve_previous_form: false,
            badge: None,
            badge_style: BadgeStyle::default(),
        });
    }
    if !refused.is_empty() {
        println!("\n  NOT added — a menu load needs Embedded or Both (049 R17):");
        for r in &refused {
            println!("    ! {r}  (form-format is Standalone, or absent)");
        }
        if !make_loadable {
            println!("    (pass --make-loadable to set these to Both and include them)");
        }
    }
    if !unclaimed.is_empty() {
        println!("\n  forms not matched to a toolbox control (left out):");
        for u in &unclaimed {
            println!("    - {u}");
        }
    }

    def.menu[samples_idx].items = folders;
    println!(
        "\n{} form(s) on disk · {kept} entry(ies) kept · {added} added · {} folder(s){}",
        stems.len(),
        def.menu[samples_idx].items.len(),
        if dry_run { "  [DRY RUN — nothing written]" } else { "" }
    );
    if !dry_run {
        // Through `save_menu`: it recomputes the HMAC the loader verifies.
        if let Err(e) = save_menu(&menu_path, &def) {
            eprintln!("failed to write {}: {e}", menu_path.display());
            std::process::exit(1);
        }
    }
}

/// Every action-carrying descendant of `items`, keyed by its action.
fn harvest(items: &[MenuItem], out: &mut BTreeMap<String, MenuItem>) {
    for it in items {
        if let Some(a) = it.action.as_deref() {
            if a.starts_with("open-form:") {
                out.insert(a.to_owned(), it.clone());
            }
        }
        harvest(&it.items, out);
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "cfrm") {
            out.push(p);
        }
    }
}
