// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The windows of the IDE that can be drawn on their own, drawn in each of the six languages:
//! none of their words is left in English in every language. (The source scan,
//! `ui_text::no_english_word_reaches_a_widget_outside_the_tables_anywhere_in_the_ide`, holds the
//! literals; this holds what reaches a widget through a variable.)

use crate::ui_text::probe::left_in_english;
use std::collections::BTreeMap;

fn report(what: &str, found: BTreeMap<String, Vec<String>>) {
    for (word, states) in &found {
        println!("English in every language: {word:?}  ({})", states.join(", "));
    }
    println!("{what}: {} words left in English", found.len());
    assert!(found.is_empty(), "{what}: {} words are English in every language", found.len());
}

/// The New indexed file dialog — its two steps, and a refusal under the second.
#[test]
fn no_word_of_the_new_indexed_file_dialog_is_left_in_english_in_every_language() {
    use crate::panels::indexed_new_dialog::NewIndexedDialog;
    type Prepare = Box<dyn Fn(&mut NewIndexedDialog)>;
    let states: Vec<(&str, Prepare)> = vec![
        ("step 1", Box::new(|_| {})),
        ("step 2", Box::new(|d| d.raw_mode = true)),
        (
            "step 2 with a refusal",
            Box::new(|d| {
                d.raw_mode = true;
                d.set_raw_error(crate::ui_text::ide("Enter a COBOL-85 record description (01 group + key)."));
            }),
        ),
    ];
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, prepare) in &states {
        let words = left_in_english(
            &|| {
                let mut d = NewIndexedDialog::new();
                prepare(&mut d);
                d
            },
            &|d, ui, tr| {
                let _ = d.show(ui, tr);
            },
        );
        for w in words {
            found.entry(w).or_default().push((*name).to_owned());
        }
    }
    report("new indexed file dialog", found);
}

/// The Output panel's toolbar and its rows' severity words.
#[test]
fn no_word_of_the_output_panel_is_left_in_english_in_every_language() {
    use crate::panels::output::OutputPanel;
    let words = left_in_english(
        &OutputPanel::new,
        &|o, ui, tr| {
            let _ = o.show(ui, tr);
        },
    );
    report("output panel", words.into_iter().map(|w| (w, vec!["toolbar".to_owned()])).collect());
}

/// The Settings form, on the tab it opens on.
#[test]
fn no_word_of_the_settings_form_is_left_in_english_in_every_language() {
    use crate::llm::LlmConfig;
    use crate::panels::settings_form::SettingsForm;
    use crate::project_model::CoboltProject;
    let words = left_in_english(
        &|| SettingsForm::new(&CoboltProject::new("demo", "main"), &LlmConfig::defaults()),
        &|form, ui, tr| {
            let _ = form.show(ui, tr, &[], false, None, true, &[], false);
        },
    );
    report("settings form", words.into_iter().map(|w| (w, vec!["first tab".to_owned()])).collect());
}
