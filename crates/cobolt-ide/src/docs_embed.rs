// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Embedded PowerRustCOBOL documentation.
//!
//! The repository `docs/` directory is baked into the IDE binary at build time
//! (via `include_dir!`) so the Documentation viewer works offline and always
//! ships with the app. The five `developers-guide-<lang>.md` translations are
//! collapsed to the one matching the current UI language.

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

/// Developer's-Guide file suffix for a UI language.
fn guide_suffix(lang: Language) -> &'static str {
    match lang {
        Language::English => "en",
        Language::Spanish => "es",
        Language::Portuguese => "pt",
        Language::Japanese => "jp",
        Language::Chinese => "cn",
    }
}

/// Build the documentation list for `lang`. The Developer's Guide appears once,
/// in the requested language; all other `*.md` docs are included as-is.
pub fn doc_list(lang: Language) -> Vec<DocEntry> {
    let want = guide_suffix(lang);
    let mut out: Vec<DocEntry> = Vec::new();
    for f in DOCS.files() {
        let name = match f.path().file_name().and_then(|s| s.to_str()) {
            Some(n) if n.ends_with(".md") => n,
            _ => continue,
        };
        // Collapse the guide translations to the language in use.
        if let Some(rest) = name.strip_prefix("developers-guide-") {
            if rest.trim_end_matches(".md") != want {
                continue;
            }
        }
        let source = f.contents_utf8().unwrap_or_default().to_string();
        let title = first_heading(&source).unwrap_or_else(|| pretty_name(name));
        out.push(DocEntry { id: name.to_string(), title, source });
    }
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
