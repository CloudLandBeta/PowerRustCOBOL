// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Machine-local IDE preferences that belong to the developer rather than to a
//! project — today just the UI language.
//!
//! The language was previously held only in `CoboltApp`, so it reset to English
//! on every start. It cannot live in `cobolt.toml`: the selector is available
//! with no project open, and a colleague opening the same project should not
//! inherit someone else's language.
//!
//! Stored at `<data_dir>/cobolt/ui.toml`, the same convention as
//! `doc_viewer.toml` and `debug_settings.toml`.

use serde::{Deserialize, Serialize};

use crate::i18n::Language;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPrefs {
    /// Language code (`en`, `es`, `pt`, `ja`, `zh`, `fr`). Empty or unknown ⇒
    /// the default language.
    pub language: String,
}

fn prefs_path() -> std::path::PathBuf {
    crate::llm::base_dir().join("ui.toml")
}

impl UiPrefs {
    /// Load the preferences, falling back to defaults on any error.
    pub fn load() -> Self {
        std::fs::read_to_string(prefs_path())
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Write the preferences (best-effort).
    pub fn save(&self) {
        if let Ok(text) = toml::to_string_pretty(self) {
            let path = prefs_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, text);
        }
    }
}

/// The language to start in: the one last chosen, or the default.
pub fn load_language() -> Language {
    Language::from_code(&UiPrefs::load().language).unwrap_or_default()
}

/// Remember `lang` as the language to start in next time.
pub fn save_language(lang: Language) {
    UiPrefs {
        language: lang.code().to_owned(),
    }
    .save();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_for_every_language() {
        for &lang in Language::ALL {
            assert_eq!(
                Language::from_code(lang.code()),
                Some(lang),
                "{lang:?} does not survive a save/load cycle"
            );
        }
    }

    #[test]
    fn unknown_or_missing_code_falls_back_to_the_default() {
        assert_eq!(Language::from_code(""), None);
        assert_eq!(Language::from_code("klingon"), None);
        let prefs: UiPrefs = toml::from_str("").unwrap();
        assert_eq!(
            Language::from_code(&prefs.language).unwrap_or_default(),
            Language::default()
        );
    }

    #[test]
    fn prefs_round_trip_through_toml() {
        let p = UiPrefs {
            language: "pt".into(),
        };
        let back: UiPrefs = toml::from_str(&toml::to_string_pretty(&p).unwrap()).unwrap();
        assert_eq!(p, back);
    }
}
