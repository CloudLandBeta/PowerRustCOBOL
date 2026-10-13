// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The words of the Properties pane that are not property names: the descriptive
//! labels of its rows ("Corner radius"), its section and group titles, hints, notes,
//! buttons and the tooltips that are not a property's explanation (those are
//! `prop_help`'s).
//!
//! The pane has hundreds of them, written as English literals where they are drawn;
//! the rule that every IDE string exists in all six languages is applied here to a set
//! too large to be `Tr` fields, the way `prop_help` applies it to the property
//! explanations. A text is looked up **by its English wording**: [`tx`] answers the
//! translation for the language the pane is drawn in, and the wording itself where the
//! table has none — so a literal that is missing from the table shows in English and is
//! caught by `ui_text_tests`, never lost.
//!
//! What stays as written, by design: the **names of properties and events** (`Width`,
//! `BackgroundColor`, `onClick`) — they are the COBOL names the developer types, and
//! the pane explains each in the IDE's language on hover — control type names, option
//! values that are stored in the form (`Fill`, `Left`), sample data and examples.

use crate::i18n::Language;
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::OnceLock;

/// One text in the six languages, in [`Language`] order for tables: English, Spanish,
/// Portuguese, Japanese, Chinese, French. The English wording is the key.
pub struct UiText {
    pub text: [&'static str; 6],
}

include!("ui_text_data.rs");

fn index() -> &'static HashMap<&'static str, &'static UiText> {
    static INDEX: OnceLock<HashMap<&'static str, &'static UiText>> = OnceLock::new();
    INDEX.get_or_init(|| UI_TEXT.iter().map(|t| (t.text[0], t)).collect())
}

fn lang_index(lang: Language) -> usize {
    match lang {
        Language::English => 0,
        Language::Spanish => 1,
        Language::Portuguese => 2,
        Language::Japanese => 3,
        Language::Chinese => 4,
        Language::French => 5,
    }
}

thread_local! {
    /// The language the pane is being drawn in (set once per frame by the pane's entry
    /// points). English until something sets it.
    static CURRENT: Cell<Language> = const { Cell::new(Language::English) };
}

/// Draw the words that follow in `lang`.
pub fn begin(lang: Language) {
    CURRENT.with(|c| c.set(lang));
}

/// The language the pane is being drawn in.
pub fn current() -> Language {
    CURRENT.with(Cell::get)
}

/// `english` in `lang`; `english` itself when the table has no such text.
pub fn localized(lang: Language, english: &str) -> &str {
    index().get(english).map_or(english, |t| t.text[lang_index(lang)])
}

/// `english` in the language the pane is drawn in.
pub fn tx(english: &str) -> &str {
    localized(current(), english)
}

/// `english` in the language the pane is drawn in, looked up under a `context`: the table holds
/// it as `"<context>: <english>"`, so a word that is also a property name elsewhere (`Status`,
/// `Data`) can be a heading here without turning that property's row into another language.
/// English, and any word the table does not have, is `english`.
pub fn txc(context: &str, english: &str) -> String {
    let lang = current();
    if lang == Language::English {
        return english.to_owned();
    }
    index()
        .get(format!("{context}: {english}").as_str())
        .map_or_else(|| english.to_owned(), |t| t.text[lang_index(lang)].to_owned())
}

/// `tx` for a text with `{}` holes, filled in order: the translated wording may move them
/// around the sentence, but not drop or add one.
pub fn txf(english: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::new();
    let mut args = args.iter();
    let mut rest = tx(english);
    while let Some(at) = rest.find("{}") {
        out.push_str(&rest[..at]);
        match args.next() {
            Some(a) => out.push_str(&a.to_string()),
            None => out.push_str("{}"),
        }
        rest = &rest[at + 2..];
    }
    out.push_str(rest);
    out
}

/// Whether the table has `english`.
#[cfg(test)]
pub(crate) fn has(english: &str) -> bool {
    index().contains_key(english)
}

/// For the tests that draw a part of the IDE in each language and hold every word of it to
/// the table: what is drawn, and which words come out in English in every language.
#[cfg(test)]
pub(crate) mod probe {
    use super::Language;
    use std::collections::BTreeSet;

    /// Names, values, samples and examples that are the same in every language on purpose.
    const SAME_IN_EVERY_LANGUAGE: &[&str] = &[
        "(Local)",
        "/api/chat (override)",
        "/path/to/image.png",
        "Actual,Budget",
        "FILE SECTION",
        "Liquid Glass",
        "Node 1\n  Child 1\n  Child 2\nNode 2",
        "SALES-AMOUNT,SALES-BUDGET",
        "Sales by Region",
        "Tab1\nTab2",
        "X(256)",
        "You are a helpful assistant.",
        "assets/KB",
        "assets/logo.png",
        "assets/mark.png",
        "dd/MM/yyyy HH:mm",
        "http://localhost:11434",
        "https://api.example.com",
        "https://api.example.com/v1/products",
        "sqlite::memory:",
        // A source button: its icon, then the source's name, which is the same word everywhere.
        "(cloud) REST",
        "(db) SQL",
        // The friendly names the binding editor suggests for its sample fields: the field's name, spelt out.
        "Acme Corporation",
        "Category Id",
        "Credit Limit",
        "Customer ID",
        "Customer Id",
        "Customer Name",
        "Customer Tier",
        "Is Active",
        "Region Id",
    ];

    /// A text that is a word of the IDE — not an identifier, a number or a sample.
    pub(crate) fn is_word(t: &str) -> bool {
        let t = t.trim();
        let identifier = t.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && t.trim_end_matches(':').chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
        // A COBOL name or an acronym has no lower-case letter; a JSON or JSONPath sample starts with
        // a quote or `$`; a picture is `PIC …` (alone or under its data type).
        !identifier
            && t.chars().filter(|c| c.is_alphabetic()).count() >= 3
            && t.chars().any(char::is_lowercase)
            && !t.starts_with(['#', '[', '"', '$'])
            && !t.ends_with("\",")
            && !t.starts_with("PIC ")
            && !t.contains("\nPIC ")
            && !t.contains("INVOKE ")
            && !SAME_IN_EVERY_LANGUAGE.contains(&t)
    }

    /// Every text painted in `lang` by `draw` — called on a state `setup` makes, once per frame.
    /// Three frames settle the layout; then the pointer sits over the middle of the screen, where a
    /// window anchored at the centre is, and the wheel turns, so a window's rows below its fold are
    /// drawn too. What each frame paints is kept.
    pub(crate) fn texts<S>(
        lang: Language,
        setup: &dyn Fn() -> S,
        draw: &dyn Fn(&mut S, &mut egui::Ui, &crate::i18n::Tr),
    ) -> Vec<String> {
        let tr = &lang.tr();
        let ctx = egui::Context::default();
        crate::i18n::set_language(&ctx, lang);
        // A state that makes words while it is set up (a refusal, say) makes them in this language.
        super::begin(lang);
        let mut state = setup();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 20000.0));
        let mut out = BTreeSet::new();
        for frame in 0..18 {
            let mut events = Vec::new();
            if frame >= 3 {
                if frame == 3 {
                    events.push(egui::Event::PointerMoved(screen.center()));
                }
                events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -300.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: Default::default(),
                });
            }
            let mut full = ctx.run_ui(
                egui::RawInput { screen_rect: Some(screen), time: Some(f64::from(frame) * 0.1), events, ..Default::default() },
                |ui| {
                    egui::CentralPanel::default().show_inside(ui, |ui| draw(&mut state, ui, tr));
                },
            );
            full.textures_delta.clear();
            fn collect(shape: &egui::Shape, out: &mut BTreeSet<String>) {
                match shape {
                    egui::Shape::Text(t) => {
                        out.insert(t.galley.text().to_owned());
                    }
                    egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
                    _ => {}
                }
            }
            for cs in &full.shapes {
                collect(&cs.shape, &mut out);
            }
        }
        // (The language it was last drawn in is a per-thread setting.)
        super::begin(Language::English);
        out.into_iter().collect()
    }

    /// The words that come out in English in every language.
    pub(crate) fn left_in_english<S>(
        setup: &dyn Fn() -> S,
        draw: &dyn Fn(&mut S, &mut egui::Ui, &crate::i18n::Tr),
    ) -> BTreeSet<String> {
        let mut same: BTreeSet<String> =
            texts(Language::English, setup, draw).into_iter().filter(|t| is_word(t)).collect();
        for &lang in Language::ALL.iter().filter(|l| **l != Language::English) {
            let other: BTreeSet<String> = texts(lang, setup, draw).into_iter().collect();
            same.retain(|t| other.contains(t));
        }
        same
    }
}

#[cfg(test)]
mod ui_text_tests {
    use super::*;

    fn holes(s: &str) -> usize {
        s.matches("{}").count()
    }

    fn has_cjk(s: &str) -> bool {
        s.chars().any(|c| matches!(c as u32, 0x3040..=0x30ff | 0x4e00..=0x9fff | 0xff00..=0xffef))
    }

    /// Every text exists in all six languages, none is empty, the Japanese and the Chinese are
    /// really in those scripts, a text of several words is not just the English copied, and the
    /// holes of a template are the same in every language.
    #[test]
    fn every_text_exists_in_six_languages() {
        let mut seen = std::collections::HashSet::new();
        let mut bad = Vec::new();
        for t in UI_TEXT {
            let en = t.text[0];
            if !seen.insert(en) {
                bad.push(format!("{en:?}: listed twice"));
            }
            for (i, text) in t.text.iter().enumerate() {
                if text.trim().is_empty() {
                    bad.push(format!("{en:?}: language {i} is empty"));
                }
                if holes(text) != holes(en) {
                    bad.push(format!("{en:?}: language {i} has {} holes, English has {}", holes(text), holes(en)));
                }
            }
            // An acronym ("URL:") is the same in every script.
            let acronym = !en.chars().any(char::is_lowercase);
            if !acronym && (!has_cjk(t.text[3]) || !has_cjk(t.text[4])) {
                bad.push(format!("{en:?}: the Japanese or the Chinese is not in its script"));
            }
            let words = en.split_whitespace().count();
            let copied = t.text.iter().skip(1).filter(|x| **x == en).count();
            if words >= 3 && copied > 1 {
                bad.push(format!("{en:?}: looks untranslated ({copied} of 5 copied)"));
            }
        }
        assert!(bad.is_empty(), "{} problems:\n{}", bad.len(), bad.join("\n"));
        println!("ui_text: {} texts × 6 languages", UI_TEXT.len());
    }

    #[test]
    fn a_text_is_found_by_its_english_wording_and_a_template_is_filled_in_order() {
        assert_eq!(localized(Language::Spanish, "Corner radius"), "Radio de esquina");
        assert_eq!(localized(Language::English, "Corner radius"), "Corner radius");
        assert_eq!(localized(Language::French, "no such text in the table"), "no such text in the table");
        begin(Language::Spanish);
        assert_eq!(txf("{} columns, {} frozen column(s), {} frozen row(s)", &[&3, &1, &0]), "3 columnas, 1 columna(s) inmovilizada(s), 0 fila(s) inmovilizada(s)");
        begin(Language::English);
    }
}
