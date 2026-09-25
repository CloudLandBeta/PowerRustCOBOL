// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What a property does, for the Properties pane: the hover text of a
//! property's name, in the interface language.
//!
//! Each explanation was written from the code that gives the property its
//! effect (painter, renderer, runtime, host), not from its name. The same
//! name can mean different things on different controls — `Value` on a
//! ComboBox is not the `Value` of a Knob — so an entry may name a control
//! type; one with an empty type applies to every control that has the
//! property. Every text exists in all six languages: the table is the
//! `Tr` rule applied to a set of strings too large to be `Tr` fields, and
//! `prop_help_tests` holds it to that.

use crate::i18n::Language;
use std::collections::HashMap;
use std::sync::OnceLock;

/// One explanation. `ty` is a control type name as `ControlType`'s `Debug`
/// prints it (`"ComboBox"`), `"Form"` for a form property, or `""` for
/// every control that has the property. `text` is in [`Language`] order:
/// English, Spanish, Portuguese, Japanese, Chinese, French.
pub struct PropHelp {
    pub ty: &'static str,
    pub prop: &'static str,
    pub text: [&'static str; 6],
}

include!("prop_help_data.rs");

fn index() -> &'static HashMap<(String, String), &'static PropHelp> {
    static INDEX: OnceLock<HashMap<(String, String), &'static PropHelp>> = OnceLock::new();
    INDEX.get_or_init(|| {
        PROP_HELP
            .iter()
            .map(|h| ((h.ty.to_ascii_lowercase(), h.prop.to_ascii_lowercase()), h))
            .collect()
    })
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

/// The explanation of `prop` on a control of type `ty` (or `"Form"`), in
/// `lang`: the type's own entry first, then the one for every control.
pub fn lookup(lang: Language, ty: &str, prop: &str) -> Option<&'static str> {
    let idx = index();
    let p = prop.trim().to_ascii_lowercase();
    idx.get(&(ty.to_ascii_lowercase(), p.clone()))
        .or_else(|| idx.get(&(String::new(), p)))
        .map(|h| h.text[lang_index(lang)])
        .filter(|t| !t.trim().is_empty())
}

#[cfg(test)]
mod prop_help_tests {
    use super::*;

    /// Every property a control offers has an explanation, in all six
    /// languages, and no text is simply the English one copied.
    #[test]
    fn every_control_property_is_explained_in_six_languages() {
        let mut missing = Vec::new();
        for t in cobolt_forms::ControlType::ALL {
            let ty = format!("{t:?}");
            let c = cobolt_forms::Control::new("X".to_owned(), t.clone(), 0, 0);
            let mut keys: Vec<&String> = c.properties.keys().collect();
            keys.sort();
            for k in keys {
                for &lang in Language::ALL {
                    if lookup(lang, &ty, k).is_none() {
                        missing.push(format!("{ty}.{k} ({lang:?})"));
                    }
                }
            }
        }
        assert!(missing.is_empty(), "{} explanations missing:\n{}", missing.len(), missing.join("\n"));
        for h in PROP_HELP {
            for (i, t) in h.text.iter().enumerate().skip(1) {
                assert!(!t.trim().is_empty(), "{}.{} is empty in language {i}", h.ty, h.prop);
            }
            let distinct = h.text.iter().skip(1).filter(|t| **t != h.text[0]).count();
            assert!(distinct >= 3, "{}.{} looks untranslated: {:?}", h.ty, h.prop, h.text);
        }
    }

    /// Every property of the form itself has an explanation too.
    #[test]
    fn every_form_property_is_explained_in_six_languages() {
        let mut missing = Vec::new();
        for key in crate::panels::designer::FORM_PROP_KEYS {
            for &lang in Language::ALL {
                if lookup(lang, "Form", key).is_none() {
                    missing.push(format!("Form.{key} ({lang:?})"));
                }
            }
        }
        assert!(missing.is_empty(), "{}", missing.join("\n"));
    }

    #[test]
    fn a_type_entry_wins_over_the_general_one() {
        // Whatever the data says, lookup must prefer (type, prop) to ("", prop).
        if let Some(h) = PROP_HELP.iter().find(|h| !h.ty.is_empty()) {
            assert_eq!(lookup(Language::English, h.ty, h.prop), Some(h.text[0]));
        }
    }
}
