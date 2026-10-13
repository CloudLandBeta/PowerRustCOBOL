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
