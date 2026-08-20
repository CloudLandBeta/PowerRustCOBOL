// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Embedded PowerRustCOBOL documentation.
//!
//! The repository `docs/` directory is baked into the IDE binary at build time
//! (via `include_dir!`) so the Documentation viewer works offline and always
//! ships with the app.
//!
//! Every document may ship in the six UI languages as `<base>-<code>.md`
//! (GOLDEN RULE #8), with the English canonical carrying either `-en` or no
//! suffix at all. The viewer must show **one entry per document**, so the whole
//! `<base>` family is collapsed to the file matching the current UI language,
//! falling back to English when that translation does not exist yet.

use include_dir::{include_dir, Dir};

use crate::i18n::Language;

/// The repository `docs/` directory, embedded at compile time.
static DOCS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../../docs");

/// One document shown in the viewer's list.
#[derive(Clone)]
pub struct DocEntry {
    /// File name (e.g. `developers-guide-en.md`) — a stable id.
    pub id: String,
    /// Human title — the document's first `# H1`, else a prettified file name.
    pub title: String,
    /// The Markdown source.
    pub source: String,
}

/// The documentation language codes, as they appear in the `-<code>.md` suffix.
///
/// Deliberately its own list rather than derived from `Language`: a stem that
/// merely *ends* in a hyphenated word (`indexed-file-format`, `database-runtime`)
/// must not be mistaken for a translation.
const LANG_CODES: [&str; 6] = ["en", "es", "pt", "fr", "jp", "cn"];

/// Documentation file suffix for a UI language.
fn lang_code(lang: Language) -> &'static str {
    match lang {
        Language::English => "en",
        Language::Spanish => "es",
        Language::Portuguese => "pt",
        Language::French => "fr",
        Language::Japanese => "jp",
        Language::Chinese => "cn",
    }
}

/// Split a documentation file name into its base and language code.
///
/// `observability-fr.md` → (`observability`, `fr`). A name with no recognised
/// language suffix is the English canonical, so `observability.md` →
/// (`observability`, `en`) — which puts it in the same family as its
/// translations.
fn split_lang(name: &str) -> (&str, &str) {
    let stem = name.strip_suffix(".md").unwrap_or(name);
    if let Some((base, code)) = stem.rsplit_once('-') {
        if LANG_CODES.contains(&code) {
            return (base, code);
        }
    }
    (stem, "en")
}

/// Build the documentation list for `lang` — **one entry per document**, in the
/// requested language where it exists and in English where it does not.
pub fn doc_list(lang: Language) -> Vec<DocEntry> {
    let want = lang_code(lang);

    // base → (file, is_exact_language_match). An exact match always wins over
    // the English fallback, whichever order the files arrive in.
    let mut best: std::collections::BTreeMap<&str, (&include_dir::File<'_>, bool)> =
        std::collections::BTreeMap::new();

    for f in DOCS.files() {
        let name = match f.path().file_name().and_then(|s| s.to_str()) {
            Some(n) if n.ends_with(".md") => n,
            _ => continue,
        };
        let (base, code) = split_lang(name);
        let exact = code == want;
        // Keep only the wanted language and the English fallback; the other
        // four translations never reach the list.
        if !exact && code != "en" {
            continue;
        }
        match best.get(base) {
            Some((_, true)) => {} // already have the requested language
            _ => {
                best.insert(base, (f, exact));
            }
        }
    }

    let mut out: Vec<DocEntry> = best
        .into_values()
        .map(|(f, _)| {
            let name = f
                .path()
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            let source = f.contents_utf8().unwrap_or_default().to_string();
            let title = first_heading(&source).unwrap_or_else(|| pretty_name(name));
            DocEntry {
                id: name.to_string(),
                title,
                source,
            }
        })
        .collect();
    // Developer's Guide first, then the rest alphabetically by title.
    out.sort_by(|a, b| {
        let ag = a.id.starts_with("developers-guide");
        let bg = b.id.starts_with("developers-guide");
        bg.cmp(&ag)
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
    out
}

/// First ATX `# ` heading in the markdown, if any.
fn first_heading(md: &str) -> Option<String> {
    for line in md.lines() {
        if let Some(h) = line.trim_start().strip_prefix("# ") {
            let h = h.trim();
            if !h.is_empty() {
                return Some(h.to_string());
            }
        }
    }
    None
}

/// Turn `indexed-file-format.md` into `Indexed file format`.
fn pretty_name(file: &str) -> String {
    let stem = file.strip_suffix(".md").unwrap_or(file);
    let mut s = stem.replace(['-', '_'], " ");
    if let Some(c) = s.get_mut(0..1) {
        c.make_ascii_uppercase();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hyphenated stem whose last word is not a language code is one document,
    /// not a translation of `cobol85-supported`.
    #[test]
    fn split_lang_only_splits_on_real_language_codes() {
        assert_eq!(split_lang("observability-fr.md"), ("observability", "fr"));
        assert_eq!(split_lang("BENCHMARKS-jp.md"), ("BENCHMARKS", "jp"));
        assert_eq!(split_lang("BENCHMARKS.md"), ("BENCHMARKS", "en"));
        assert_eq!(
            split_lang("cobol85-supported-syntax.md"),
            ("cobol85-supported-syntax", "en")
        );
        assert_eq!(
            split_lang("indexed-file-format.md"),
            ("indexed-file-format", "en")
        );
        assert_eq!(
            split_lang("database-runtime.md"),
            ("database-runtime", "en")
        );
    }

    /// The regression this module exists to prevent: before the `<base>-<lang>`
    /// families were collapsed, every translation of every doc was listed at
    /// once, so the viewer showed "Benchmarks" four times over.
    #[test]
    fn every_language_lists_each_document_exactly_once() {
        for lang in Language::ALL {
            let list = doc_list(*lang);
            let mut bases: Vec<&str> = list.iter().map(|d| split_lang(&d.id).0).collect();
            bases.sort_unstable();
            let mut unique = bases.clone();
            unique.dedup();
            assert_eq!(
                bases, unique,
                "{lang:?} lists the same document more than once: {bases:?}"
            );
        }
    }

    /// A translated doc resolves to its own file; a doc with no translation in
    /// that language falls back to English rather than disappearing.
    #[test]
    fn translation_is_picked_when_present_and_english_when_not() {
        let es: Vec<String> = doc_list(Language::Spanish)
            .into_iter()
            .map(|d| d.id)
            .collect();
        assert!(es.contains(&"observability-es.md".to_string()));
        assert!(es.contains(&"BENCHMARKS-es.md".to_string()));
        // No Spanish translation of this one yet → English canonical.
        assert!(es.contains(&"indexed-file-format.md".to_string()));

        // French has the five translated docs but no guide translation yet, so
        // the guide must still appear — in English.
        let fr: Vec<String> = doc_list(Language::French)
            .into_iter()
            .map(|d| d.id)
            .collect();
        assert!(fr.contains(&"BENCHMARKS-fr.md".to_string()));
        assert!(fr.contains(&"developers-guide-en.md".to_string()));
    }

    /// Whatever the language, the guide is the first row in the list.
    #[test]
    fn the_guide_sorts_first_in_every_language() {
        for lang in Language::ALL {
            let list = doc_list(*lang);
            assert!(
                list[0].id.starts_with("developers-guide"),
                "{lang:?} did not put the guide first, got {}",
                list[0].id
            );
        }
    }
}
