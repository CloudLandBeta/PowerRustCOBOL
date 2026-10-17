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
    /// Beautify: the PowerRustCOBOL layout style. Absent (an older `ui.toml`)
    /// reads as on — it is what the operator asked ✨ Beautify to do.
    pub beautify_style: Option<bool>,
    /// The first-run Rust question has been settled — a usable toolchain was
    /// found, one was installed, or the developer declined twice. False (the
    /// default, and what an older `ui.toml` reads as) means "not asked yet",
    /// which is what makes the *first* run the first run.
    pub rust_check_done: bool,
    /// The first-run model-provider question (Ollama) has been settled — Ollama
    /// was found or installed, or the developer chose another provider. Asked
    /// after the Rust question, once per machine; false (the default, and what
    /// an older `ui.toml` reads as) means "not asked yet".
    pub ollama_check_done: bool,
    /// The IDE Walkthrough has been shown on this machine (spec 059 R9).
    ///
    /// Machine-level, beside `rust_check_done` and for the same reason: a tour
    /// of the IDE is learned once, not once per project, and a colleague
    /// opening the same project must not inherit someone else's answer. False
    /// — the default, and what an older `ui.toml` reads as — means "not shown
    /// yet", which is what makes the first project open the first one.
    ///
    /// Unlike `rust_check_done` this one can be **cleared**: unchecking the
    /// Help-menu item is how a developer replays the tour (R11).
    pub walkthrough_shown: bool,
    /// Where the platform SDK — the Rust sources a built application compiles
    /// against — lives on this machine. Empty (the default) means "look for it
    /// yourself", which is what an install that ships the SDK beside the
    /// executable, and a source checkout, both want.
    ///
    /// It belongs here rather than in `cobolt.toml`: the folder is a property
    /// of *this machine's* installation, so a colleague opening the same
    /// project must not inherit a path that exists only on someone else's disk.
    pub workspace_root: String,
    /// The folder a build is staged in (`<folder>/cobolt-build-<name>`: the generated
    /// `Cargo.toml`, `Cargo.lock` and cargo's `target/`). Empty (the default) means "the system's
    /// own place" — the per-user cache folder every build used before spec 100.
    ///
    /// Machine-wide, beside `workspace_root` and for the same reason: where a machine keeps its
    /// builds is a property of that machine (its disks, its policies), so a colleague opening the
    /// same project must not inherit a path that exists only on someone else's drive.
    pub build_dir: String,
    /// The Task Wizard switch, on both toolbars (spec 104 R5). Off — the default, and what an
    /// older `ui.toml` reads as — means the IDE behaves exactly as it did before the wizard.
    pub task_wizard_on: bool,
    /// The tasks finished on this machine, by task key (R29). Machine-level for the same reason
    /// as `walkthrough_shown`: knowing how to bind a grid is learned once, not once per project.
    pub task_wizard_done: std::collections::BTreeSet<String>,
    /// Where an unfinished task stands, per project (R29): project key → task key → the key of
    /// the step to resume at. Keyed by step *key*, never index, so a table that grows a step
    /// still resumes in the right place; a key the table no longer has restarts the task.
    pub task_wizard_progress:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    /// Offers the developer dismissed (R30), keyed `project|form|control|task`, so dismissing
    /// one grid's offer never silences another's.
    pub task_wizard_dismissed: std::collections::BTreeSet<String>,
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
/// UPPERCASE verbs, comments left as written and the PowerRustCOBOL style on.
pub fn load_beautify() -> (crate::panels::beautify::VerbCase, bool, bool) {
    use crate::panels::beautify::VerbCase;
    let prefs = UiPrefs::load();
    let verbs = match prefs.beautify_verbs.as_str() {
        "leave" => VerbCase::Leave,
        "lower" => VerbCase::Lower,
        "capitalize" => VerbCase::Capitalize,
        _ => VerbCase::Upper,
    };
    (verbs, prefs.beautify_align_comments, prefs.beautify_style.unwrap_or(true))
}

/// Remember the Beautify choices for next time (load-then-save, like
/// [`save_language`]).
pub fn save_beautify(verbs: crate::panels::beautify::VerbCase, align_comments: bool, style: bool) {
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
    prefs.beautify_style = Some(style);
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

/// Has the first-run Ollama question already been settled?
pub fn ollama_check_done() -> bool {
    UiPrefs::load().ollama_check_done
}

/// Settle it, so it is never asked again.
pub fn mark_ollama_check_done() {
    let mut prefs = UiPrefs::load();
    prefs.ollama_check_done = true;
    prefs.save();
}

/// Has the IDE Walkthrough already been shown on this machine? (Spec 059 R7.)
pub fn walkthrough_shown() -> bool {
    UiPrefs::load().walkthrough_shown
}

/// Record whether the Walkthrough has been shown (load-then-save).
///
/// Takes a value rather than only setting it: R11 requires the Help-menu item
/// to *clear* the flag, which is what replays the tour.
pub fn set_walkthrough_shown(shown: bool) {
    let mut prefs = UiPrefs::load();
    prefs.walkthrough_shown = shown;
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

/// The configured build folder, or `None` for the system default. A blank entry is `None`, never
/// an empty path (which would be the process's working directory) — as for [`load_workspace_root`].
pub fn load_build_dir() -> Option<std::path::PathBuf> {
    let prefs = UiPrefs::load();
    let trimmed = prefs.build_dir.trim();
    (!trimmed.is_empty()).then(|| std::path::PathBuf::from(trimmed))
}

/// Remember where builds are kept. `None` clears the setting and restores the system default
/// (load-then-save, like [`save_workspace_root`]).
pub fn save_build_dir(root: Option<&std::path::Path>) {
    let mut prefs = UiPrefs::load();
    prefs.build_dir = root.map(|p| p.display().to_string()).unwrap_or_default();
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
            beautify_style: Some(false),
            rust_check_done: true,
            ollama_check_done: true,
            walkthrough_shown: true,
            workspace_root: "/opt/powerrustcobol-sdk".into(),
            build_dir: "/data/builds".into(),
            task_wizard_on: true,
            task_wizard_done: ["models".to_owned(), "datagrid".to_owned()].into(),
            task_wizard_progress: [(
                "/home/dev/Inventory".to_owned(),
                [("binding".to_owned(), "mappings".to_owned())].into(),
            )]
            .into(),
            task_wizard_dismissed: ["/home/dev/Inventory|Main|Grid1|datagrid".to_owned()].into(),
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

    /// An older `ui.toml` has no `build_dir` and must read as "the system default" — upgrading the
    /// IDE must not move anybody's builds — and a blank entry is the same thing, never an empty
    /// path that would mean the working directory (spec 100 R5).
    #[test]
    fn no_build_dir_or_a_blank_one_is_the_system_default() {
        let older: UiPrefs = toml::from_str("language = \"fr\"\n").unwrap();
        assert!(older.build_dir.is_empty());
        let blank: UiPrefs = toml::from_str("build_dir = \"  \"\n").unwrap();
        assert!(blank.build_dir.trim().is_empty());
        let set: UiPrefs = toml::from_str("build_dir = \"/data/builds\"\n").unwrap();
        assert_eq!(set.build_dir, "/data/builds");
    }

    /// A `ui.toml` written before the Rust check existed must read as
    /// "not asked yet" — otherwise upgrading the IDE would silently skip the
    /// one run the check gets.
    #[test]
    fn an_older_prefs_file_has_not_been_asked_yet() {
        let older: UiPrefs = toml::from_str("language = \"fr\"\n").unwrap();
        assert!(!older.rust_check_done);
        assert!(!older.ollama_check_done, "an older ui.toml has not been asked about Ollama");
        assert!(
            !older.walkthrough_shown,
            "a ui.toml written before the Walkthrough existed must read as \
             'not shown yet', or upgrading the IDE would silently skip the one \
             showing it gets"
        );
        assert_eq!(older.language, "fr");
    }

    /// Upgrading the IDE must not switch the Task Wizard on, and must find no task done, none in
    /// progress and no offer dismissed (spec 104 R5, R29, R30).
    #[test]
    fn an_older_prefs_file_has_the_task_wizard_off_and_empty() {
        let older: UiPrefs = toml::from_str("language = \"fr\"\nwalkthrough_shown = true\n").unwrap();
        assert!(!older.task_wizard_on, "the Task Wizard must be off on a machine that never set it");
        assert!(older.task_wizard_done.is_empty());
        assert!(older.task_wizard_progress.is_empty());
        assert!(older.task_wizard_dismissed.is_empty());
    }
}
