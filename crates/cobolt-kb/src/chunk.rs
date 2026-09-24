// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Splitting a document into passages by subject (spec 068 R15).
//!
//! Lifted from the IDE Knowledge Base's chunker (`cobolt-agents`
//! `chunked_knowledge`), minus its `## Control:` handling, which exists only
//! for the System KB's generated reference: users' documents are sectioned by
//! their headings, at every level. The size limit and the way an over-long
//! section is split are unchanged.

use std::path::Path;

/// The most characters one passage holds. A longer section is split into parts
/// chained to one another, and a search reassembles the chain.
pub const PASSAGE_CHARS: usize = 512;

/// One section of a document, before it is split to [`PASSAGE_CHARS`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Where it sits: `"<document> › <heading> › <sub-heading>"`.
    pub heading: String,
    /// The section's text, heading line included.
    pub content: String,
}

/// FNV-1a, 64-bit — the hash both the hashing embedder and change detection
/// use. Not cryptographic: it detects edits, which are not adversarial.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Split Markdown (or plain text) into sections at its headings.
///
/// A `#`…`######` heading opens a section named by the path of headings above
/// it, so `## Leave` under `# Handbook` is `"handbook › Handbook › Leave"`.
/// Text before the first heading is a section named by the document alone.
/// Empty sections are dropped. Plain text, having no headings, is one section.
pub fn sections(doc_path: &str, text: &str) -> Vec<Section> {
    let doc_name = Path::new(doc_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(doc_path)
        .to_string();
    let mut out = Vec::new();
    // The heading at each level currently open, `trail[0]` for `#`.
    let mut trail: Vec<Option<String>> = vec![None; 6];
    let mut heading = doc_name.clone();
    let mut lines: Vec<&str> = Vec::new();
    let mut in_fence = false;

    let flush = |heading: &str, lines: &mut Vec<&str>, out: &mut Vec<Section>| {
        let content = lines.join("\n").trim().to_string();
        if !content.is_empty() {
            out.push(Section {
                heading: heading.to_string(),
                content,
            });
        }
        lines.clear();
    };

    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        }
        let level = if in_fence {
            0
        } else {
            trimmed.chars().take_while(|c| *c == '#').count()
        };
        let title = trimmed.get(level..).map(str::trim).unwrap_or("");
        let is_heading = (1..=6).contains(&level)
            && trimmed.as_bytes().get(level) == Some(&b' ')
            && !title.is_empty();
        if is_heading {
            flush(&heading, &mut lines, &mut out);
            trail[level - 1] = Some(title.to_string());
            for deeper in trail.iter_mut().skip(level) {
                *deeper = None;
            }
            heading = std::iter::once(doc_name.as_str())
                .chain(trail.iter().flatten().map(String::as_str))
                .collect::<Vec<_>>()
                .join(" › ");
        } else if trimmed == "---" && !in_fence {
            // A horizontal rule separates; it is never content.
            continue;
        }
        lines.push(line);
    }
    flush(&heading, &mut lines, &mut out);
    out
}

/// Split one section's text into parts of at most [`PASSAGE_CHARS`]
/// characters, preferring line boundaries; a single longer line is split on a
/// character boundary.
pub fn split_content(content: &str) -> Vec<String> {
    if content.chars().count() <= PASSAGE_CHARS {
        return vec![content.to_string()];
    }
    let mut parts = Vec::new();
    let mut part = String::new();
    let mut part_chars = 0_usize;
    for line in content.split_inclusive('\n') {
        let mut rest = line;
        while !rest.is_empty() {
            let rest_chars = rest.chars().count();
            let room = PASSAGE_CHARS - part_chars;
            if rest_chars <= room {
                part.push_str(rest);
                part_chars += rest_chars;
                rest = "";
            } else if part_chars > 0 {
                parts.push(std::mem::take(&mut part));
                part_chars = 0;
            } else {
                let byte = rest
                    .char_indices()
                    .nth(PASSAGE_CHARS)
                    .map(|(byte, _)| byte)
                    .unwrap_or(rest.len());
                part.push_str(&rest[..byte]);
                parts.push(std::mem::take(&mut part));
                part_chars = 0;
                rest = &rest[byte..];
            }
        }
    }
    if !part.trim().is_empty() {
        parts.push(part);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_at_every_level_open_sections_named_by_their_path() {
        let text = "Intro line.\n# Handbook\nWelcome.\n## Leave\nTwenty days.\n### Carry-over\nFive days.\n## Pay\nMonthly.\n";
        let got: Vec<(String, String)> = sections("docs/handbook.md", text)
            .into_iter()
            .map(|s| (s.heading, s.content))
            .collect();
        assert_eq!(
            got,
            vec![
                ("handbook".into(), "Intro line.".into()),
                ("handbook › Handbook".into(), "# Handbook\nWelcome.".into()),
                ("handbook › Handbook › Leave".into(), "## Leave\nTwenty days.".into()),
                (
                    "handbook › Handbook › Leave › Carry-over".into(),
                    "### Carry-over\nFive days.".into()
                ),
                ("handbook › Handbook › Pay".into(), "## Pay\nMonthly.".into()),
            ]
        );
    }

    #[test]
    fn plain_text_is_one_section_and_code_fences_are_not_headings() {
        let got = sections("notes.txt", "line one\n```\n# not a heading\n```\nline two");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].heading, "notes");
        assert!(got[0].content.contains("# not a heading"));
    }

    #[test]
    fn a_long_section_splits_at_lines_into_parts_that_fit() {
        let line = "x".repeat(300);
        let text = format!("{line}\n{line}\n{line}");
        let parts = split_content(&text);
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.chars().count() <= PASSAGE_CHARS));
        assert_eq!(parts.concat(), text, "nothing is lost or reordered");
        let one = "y".repeat(1300);
        let hard = split_content(&one);
        assert_eq!(hard.iter().map(|p| p.chars().count()).collect::<Vec<_>>(), vec![512, 512, 276]);
    }
}
