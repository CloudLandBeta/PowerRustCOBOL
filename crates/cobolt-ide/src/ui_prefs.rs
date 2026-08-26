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
    /// Beautify verb casing: `leave` / `upper` / `lower` / `capitalize`.
    /// Empty or unknown ⇒ `upper` (spec 043 rule 10a).
    pub beautify_verbs: String,
    /// Beautify: align `*>` comments with the surrounding code (rule 10b).
    pub beautify_align_comments: bool,
    /// The first-run Rust question has been settled — a usable toolchain was
    /// found, one was installed, or the developer declined twice. False (the
    /// default, and what an older `ui.toml` reads as) means "not asked yet",
    /// which is what makes the *first* run the first run.
    pub rust_check_done: bool,
    /// Where the platform SDK — the Rust sources a built application compiles
    /// against — lives on this machine. Empty (the default) means "look for it
    /// yourself", which is what an install that ships the SDK beside the
    /// executable, and a source checkout, both want.
    ///
    /// It belongs here rather than in `cobolt.toml`: the folder is a property
    /// of *this machine's* installation, so a colleague opening the same
    /// project must not inherit a path that exists only on someone else's disk.
    pub workspace_root: String,
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

/// Remember `lang` as the language to start in next time. Loads-then-saves so
/// the other preferences in the file survive the write.
pub fn save_language(lang: Language) {
    let mut prefs = UiPrefs::load();
    prefs.language = lang.code().to_owned();
    prefs.save();
}

/// The remembered Beautify choices (spec 043 rule 10), defaulting to
/// UPPERCASE verbs and comments left as written.
pub fn load_beautify() -> (crate::panels::beautify::VerbCase, bool) {
    use crate::panels::beautify::VerbCase;
    let prefs = UiPrefs::load();
    let verbs = match prefs.beautify_verbs.as_str() {
        "leave" => VerbCase::Leave,
        "lower" => VerbCase::Lower,
        "capitalize" => VerbCase::Capitalize,
        _ => VerbCase::Upper,
    };
    (verbs, prefs.beautify_align_comments)
}

/// Remember the Beautify choices for next time (load-then-save, like
/// [`save_language`]).
pub fn save_beautify(verbs: crate::panels::beautify::VerbCase, align_comments: bool) {
    use crate::panels::beautify::VerbCase;
    let mut prefs = UiPrefs::load();
    prefs.beautify_verbs = match verbs {
        VerbCase::Leave => "leave",
        VerbCase::Upper => "upper",
        VerbCase::Lower => "lower",
        VerbCase::Capitalize => "capitalize",
    }
    .to_owned();
    prefs.beautify_align_comments = align_comments;
    prefs.save();
}

/// Has the first-run Rust toolchain question already been settled?
pub fn rust_check_done() -> bool {
    UiPrefs::load().rust_check_done
}

/// Settle it, so it is never asked again (load-then-save, like [`save_language`]).
pub fn mark_rust_check_done() {
    let mut prefs = UiPrefs::load();
    prefs.rust_check_done = true;
    prefs.save();
}

/// The configured platform SDK folder, or `None` to let the compiler search.
///
/// A blank entry is `None` rather than an empty path: an empty string would
/// resolve to the process's working directory and shadow the automatic search
/// with a folder nobody chose.
pub fn load_workspace_root() -> Option<std::path::PathBuf> {
    let prefs = UiPrefs::load();
    let trimmed = prefs.workspace_root.trim();
    (!trimmed.is_empty()).then(|| std::path::PathBuf::from(trimmed))
}

/// Remember where the platform SDK lives. `None` clears the setting and
/// restores the automatic search (load-then-save, like [`save_language`]).
pub fn save_workspace_root(root: Option<&std::path::Path>) {
    let mut prefs = UiPrefs::load();
    prefs.workspace_root = root
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    prefs.save();
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
            beautify_verbs: "capitalize".into(),
            beautify_align_comments: true,
            rust_check_done: true,
            workspace_root: "/opt/powerrustcobol-sdk".into(),
        };
        let back: UiPrefs = toml::from_str(&toml::to_string_pretty(&p).unwrap()).unwrap();
        assert_eq!(p, back);
    }

    /// A blank entry must mean "search automatically", not "use the process's
    /// working directory" — an empty path would resolve to the latter and
    /// shadow the compiler's own search with a folder nobody chose.
    #[test]
    fn a_blank_workspace_root_is_no_setting_at_all() {
        let blank: UiPrefs = toml::from_str("workspace_root = \"   \"\n").unwrap();
        assert!(blank.workspace_root.trim().is_empty());

        // An older ui.toml has no such key at all, and must read the same way.
        let older: UiPrefs = toml::from_str("language = \"fr\"\n").unwrap();
        assert!(older.workspace_root.is_empty());
    }

    /// A `ui.toml` written before the Rust check existed must read as
    /// "not asked yet" — otherwise upgrading the IDE would silently skip the
    /// one run the check gets.
    #[test]
    fn an_older_prefs_file_has_not_been_asked_yet() {
        let older: UiPrefs = toml::from_str("language = \"fr\"\n").unwrap();
        assert!(!older.rust_check_done);
        assert_eq!(older.language, "fr");
    }
}
