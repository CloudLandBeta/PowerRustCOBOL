// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Completion for the AI assistant's PROMPT box — the names, not the language.
//!
//! The developer writing a prompt has to spell the project's own words exactly:
//! a control id, a data item, a property, an event. Those are precisely the
//! words the model cannot guess and the ones a typo turns into a wasted
//! workflow — the agents bind to what the prompt says. So the prompt box
//! completes names from the SAME catalogue the task context is built from.
//!
//! What it must NOT complete is COBOL itself: a prompt is prose, and popping a
//! list of verbs over `MOVE` or `DISPLAY` would fire on ordinary English and
//! Spanish sentences ("display", "set", "start"). Reserved words are therefore
//! excluded outright (operator, 2026-07-31) — [`is_reserved`] asks the lexer,
//! so the exclusion cannot drift from the language.
//!
//! Two triggers, both from the operator's brief:
//! - `<control>::` — the members of THAT control: its properties and events.
//! - a bare word of two or more characters — control ids, data items,
//!   procedures, and the property/event names of the form's control types.
//!
//! The geometry lives here as pure functions so the popup's behaviour is
//! testable without an egui context.

use std::collections::HashSet;
use std::ops::Range;

use cobolt_forms::{ControlType, Form};

/// Longest list the popup shows — a prompt box is not a browser.
pub const MAX_ITEMS: usize = 12;
/// Shortest bare word that opens the popup. One character matches half the
/// catalogue and pops over ordinary prose.
pub const MIN_PREFIX: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Control,
    DataItem,
    Procedure,
    Property,
    Event,
}

impl Kind {
    /// The word shown next to the name in the popup.
    pub fn detail(self) -> &'static str {
        match self {
            Kind::Control => "control",
            Kind::DataItem => "data item",
            Kind::Procedure => "procedure",
            Kind::Property => "property",
            Kind::Event => "event",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub label: String,
    pub kind: Kind,
    /// The control type a property or event came from, for the popup's hint.
    pub owner: Option<String>,
}

/// What the caret is on and what may replace it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    /// Byte range in the prompt text that accepting an item replaces.
    pub replace: Range<usize>,
    pub items: Vec<Suggestion>,
}

/// The project's own words, read from the open form — the same source the
/// delegated task context is sliced from.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    controls: Vec<(String, ControlType)>,
    data_items: Vec<String>,
    procedures: Vec<String>,
}

impl Catalog {
    pub fn from_form(form: &Form, data_items: Vec<String>) -> Self {
        Self {
            controls: form
                .controls
                .iter()
                .map(|c| (c.id.clone(), c.control_type.clone()))
                .collect(),
            data_items,
            procedures: form
                .user_procedures
                .iter()
                .map(|p| p.name.clone())
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.controls.is_empty() && self.data_items.is_empty() && self.procedures.is_empty()
    }

    fn control_type(&self, id: &str) -> Option<&ControlType> {
        self.controls
            .iter()
            .find(|(cid, _)| cid.eq_ignore_ascii_case(id))
            .map(|(_, t)| t)
    }
}

/// Whether `word` is a COBOL reserved word — asked of the lexer's own table, so
/// the prompt's exclusion list can never drift from the language's.
pub fn is_reserved(word: &str) -> bool {
    cobolt_lexer::keywords::lookup(&word.to_ascii_uppercase()).is_some()
}

/// A character that may appear inside a COBOL word or a control id.
fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// The word the caret sits at the end of, as a byte range.
fn word_before(text: &str, caret: usize) -> Range<usize> {
    let start = text[..caret]
        .char_indices()
        .rev()
        .find(|(_, c)| !is_word_char(*c))
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    start..caret
}

/// The completions offered at `caret` (a byte index), or `None` when the caret
/// is not on something worth completing.
pub fn complete(text: &str, caret: usize, cat: &Catalog) -> Option<Completion> {
    if caret > text.len() || !text.is_char_boundary(caret) || cat.is_empty() {
        return None;
    }
    let word = word_before(text, caret);
    let prefix = &text[word.clone()];

    // `<control>::` — the members of that control, and nothing else. The
    // receiver is whatever word precedes the `::`.
    if text[..word.start].ends_with("::") {
        let recv = word_before(text, word.start - 2);
        let recv = &text[recv];
        if let Some(ct) = cat.control_type(recv) {
            let items = members_of(ct, prefix);
            return (!items.is_empty()).then_some(Completion {
                replace: word,
                items,
            });
        }
        return None;
    }

    // A bare word: names only, never the language. `is_reserved` is what keeps
    // the popup out of ordinary prose — "display the total" must stay prose.
    if prefix.chars().count() < MIN_PREFIX || is_reserved(prefix) {
        return None;
    }
    let items = names_matching(cat, prefix);
    (!items.is_empty()).then_some(Completion {
        replace: word,
        items,
    })
}

/// Properties and events of one control type, filtered by `prefix`.
fn members_of(ct: &ControlType, prefix: &str) -> Vec<Suggestion> {
    let up = prefix.to_ascii_uppercase();
    let owner = ct.as_str().to_string();
    let mut out: Vec<Suggestion> = Vec::new();
    for e in ct.supported_events() {
        if e.to_ascii_uppercase().starts_with(&up) {
            out.push(Suggestion {
                label: (*e).to_string(),
                kind: Kind::Event,
                owner: Some(owner.clone()),
            });
        }
    }
    for p in cobolt_forms::model::property_names_for(&owner) {
        if p.to_ascii_uppercase().starts_with(&up) {
            out.push(Suggestion {
                label: p,
                kind: Kind::Property,
                owner: Some(owner.clone()),
            });
        }
    }
    out.truncate(MAX_ITEMS);
    out
}

/// Everything the catalogue knows whose name starts with `prefix`, most
/// specific first: the ids and data names the developer is most likely typing,
/// then the members shared by the form's control types.
fn names_matching(cat: &Catalog, prefix: &str) -> Vec<Suggestion> {
    let up = prefix.to_ascii_uppercase();
    let starts = |s: &str| s.to_ascii_uppercase().starts_with(&up);
    let mut out: Vec<Suggestion> = Vec::new();

    for (id, ct) in &cat.controls {
        if starts(id) {
            out.push(Suggestion {
                label: id.clone(),
                kind: Kind::Control,
                owner: Some(ct.as_str().to_string()),
            });
        }
    }
    for d in &cat.data_items {
        if starts(d) {
            out.push(Suggestion {
                label: d.clone(),
                kind: Kind::DataItem,
                owner: None,
            });
        }
    }
    for p in &cat.procedures {
        if starts(p) {
            out.push(Suggestion {
                label: p.clone(),
                kind: Kind::Procedure,
                owner: None,
            });
        }
    }
    // Members are offered bare too — "onGot…" is a fair thing to type in a
    // prompt — but only once per name, however many types share it.
    let mut seen: HashSet<String> = out.iter().map(|s| s.label.to_ascii_uppercase()).collect();
    let types: Vec<ControlType> = {
        let mut t: Vec<ControlType> = Vec::new();
        for (_, ct) in &cat.controls {
            if !t.contains(ct) {
                t.push(ct.clone());
            }
        }
        t
    };
    for ct in &types {
        for m in members_of(ct, prefix) {
            if seen.insert(m.label.to_ascii_uppercase()) {
                out.push(m);
            }
        }
    }
    out.truncate(MAX_ITEMS);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::Control;

    fn catalog() -> Catalog {
        let mut form = Form::new("TEXTBOXES-FORM", "T", 800, 600);
        form.controls
            .push(Control::new("TXT-NAME", ControlType::TextBox, 0, 0));
        form.controls
            .push(Control::new("SAVE-BUTTON", ControlType::Button, 0, 0));
        Catalog::from_form(&form, vec!["WS-CUSTOMER-NAME".into(), "WS-TOTAL".into()])
    }

    fn labels(text: &str) -> Vec<String> {
        complete(text, text.len(), &catalog())
            .map(|c| c.items.into_iter().map(|i| i.label).collect())
            .unwrap_or_default()
    }

    #[test]
    fn a_control_id_completes_from_two_characters() {
        assert!(labels("mude o TX").contains(&"TXT-NAME".to_string()));
        assert!(labels("mude o sav").contains(&"SAVE-BUTTON".to_string()));
        // One character is not a request for the catalogue.
        assert!(labels("mude o T").is_empty());
    }

    #[test]
    fn a_data_item_and_a_procedure_complete_too() {
        assert!(labels("guarde em WS-CUS").contains(&"WS-CUSTOMER-NAME".to_string()));
        let mut form = Form::new("F", "T", 800, 600);
        form.controls
            .push(Control::new("TXT-1", ControlType::TextBox, 0, 0));
        form.user_procedures.push(cobolt_forms::model::UserProcedure {
            name: "UPDATE-TOTAL".into(),
            code: String::new(),
        });
        let cat = Catalog::from_form(&form, vec![]);
        let got = complete("chame UPDA", 10, &cat).expect("procedure completes");
        assert_eq!(got.items[0].label, "UPDATE-TOTAL");
        assert_eq!(got.items[0].kind, Kind::Procedure);
    }

    /// The rule the operator set: a prompt is prose, so the language itself is
    /// never completed — otherwise the popup fires on ordinary sentences.
    #[test]
    fn cobol_reserved_words_never_trigger() {
        for word in ["MOVE", "display", "Perform", "SET", "start", "COMPUTE"] {
            assert!(is_reserved(word), "{word} should be reserved");
            assert!(
                labels(&format!("por favor {word}")).is_empty(),
                "{word} opened the popup"
            );
        }
        // …while a name that merely CONTAINS a keyword still completes.
        assert!(labels("veja o SAVE-B").contains(&"SAVE-BUTTON".to_string()));
    }

    #[test]
    fn the_double_colon_lists_that_controls_members() {
        let got = complete("em TXT-NAME::on", 15, &catalog()).expect("member list");
        assert!(got.items.iter().all(|i| i.kind == Kind::Event));
        let names: Vec<&str> = got.items.iter().map(|i| i.label.as_str()).collect();
        assert!(names.contains(&"onGotFocus"), "{names:?}");
        // Properties come through the same trigger.
        let props = complete("em TXT-NAME::Fore", 17, &catalog()).expect("property list");
        assert_eq!(props.items[0].label, "ForegroundColor");
        assert_eq!(props.items[0].kind, Kind::Property);
        // A Button has no onGotFocus-only TextBox property: the list belongs to
        // the receiver, not to the form.
        let btn = complete("em SAVE-BUTTON::Multi", 21, &catalog());
        assert!(btn.is_none(), "Multiline is not a Button property");
    }

    #[test]
    fn an_unknown_receiver_offers_nothing() {
        assert!(complete("em NOPE::on", 11, &catalog()).is_none());
    }

    #[test]
    fn the_replaced_range_is_the_typed_word_only() {
        let cat = catalog();
        let got = complete("mude o TX", 9, &cat).unwrap();
        assert_eq!(got.replace, 7..9);
        let member = complete("em TXT-NAME::onGo", 17, &cat).unwrap();
        assert_eq!(member.replace, 13..17, "the `::` stays, only the member is replaced");
        // Accepting replaces exactly that span.
        let mut text = String::from("em TXT-NAME::onGo");
        text.replace_range(member.replace, "onGotFocus");
        assert_eq!(text, "em TXT-NAME::onGotFocus");
    }
}
