// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Grace's prompt review: the request the developer wrote, rewritten into the
//! request the specialists will actually be held to — and shown back before
//! anything runs.
//!
//! The defects the Pedantic agents keep catching are born in the specification,
//! not in the model: an ambiguous sentence reaches four agents, each resolves it
//! differently, and the correction loop pays for it. So Grace rewrites the
//! request first — grammar, ambiguity against what PowerRustCOBOL can actually
//! do, ordering, completeness — while preserving the developer's intent exactly,
//! and marks the passages that remain open to more than one reading. The
//! developer reads both versions, edits the revised one, and only then does the
//! workflow start (operator, 2026-07-31).
//!
//! This module is the pure half: the instruction Grace is given, the parse of
//! what she returns, and the mapping of each flagged quote onto a range of the
//! revised text. The modal that shows it lives in the designer panel.

use std::ops::Range;

/// A passage the review wants the developer to look at, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// The exact passage, as it appears in the revised text.
    pub quote: String,
    /// One short sentence: why this passage deserves a second look.
    pub why: String,
}

/// What Grace returned: the rewritten request plus the passages she flagged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Review {
    pub revised: String,
    pub notes: Vec<Note>,
}

/// A flagged passage located inside the (possibly edited) revised text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Highlight {
    pub range: Range<usize>,
    pub why: String,
}

/// The instruction Grace answers with the review. Deliberately narrow: this
/// step must not design anything, must not plan, and must not add a
/// requirement the developer did not ask for — the failure it exists to
/// prevent is a specification the agents can read two ways, not a
/// specification that is too small.
pub const REVIEW_INSTRUCTION: &str = "\
PROMPT REVIEW (Grace-internal, shown to the developer before anything runs).

Rewrite the developer's request below into the request the specialists will be \
held to. Do NOT plan, do NOT delegate, do NOT answer it, and do NOT start any \
work — this step produces text and nothing else.

Do all of this, in this order:
1. Read the original request.
2. Fix grammar, spelling and punctuation.
3. Remove ambiguity about what PowerRustCOBOL can actually do, wherever the \
original allows more than one reading. Use the real names of controls, \
properties and events from the context when the developer clearly meant them.
4. Reorder and structure the content for clarity, objectivity and \
completeness, so no specialist has to guess and none can invent an invalid \
solution.
5. Preserve the developer's intent EXACTLY. Never add a requirement that was \
not asked for, never remove one that was, and never decide something the \
developer left open — flag it instead.

Then mark the passages of YOUR REVISED TEXT that remain ambiguous, incomplete, \
contradictory, or that two agents could read differently. Quote each passage \
verbatim from the revised text and give one short reason.

Write the revised text in the SAME LANGUAGE as the original request.

Reply with ONLY one fenced JSON block of this exact shape and nothing else:
{\"revised\": \"<the rewritten request>\", \"notes\": [{\"quote\": \"<passage, verbatim from the revised text>\", \"why\": \"<one short sentence>\"}]}
When nothing needs flagging, \"notes\" is an empty array.";

/// Parse Grace's reply. Tolerant about the wrapping (fenced block or bare
/// JSON) and about a missing `notes`; strict about the one thing that matters
/// — a revised text that is actually there.
pub fn parse_review(reply: &str) -> Option<Review> {
    let json = extract_json(reply)?;
    let value: serde_json::Value = serde_json::from_str(&json).ok()?;
    let revised = value.get("revised")?.as_str()?.trim().to_string();
    if revised.is_empty() {
        return None;
    }
    let notes = value
        .get("notes")
        .and_then(|n| n.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|n| {
                    let quote = n.get("quote")?.as_str()?.trim().to_string();
                    let why = n.get("why")?.as_str()?.trim().to_string();
                    (!quote.is_empty() && !why.is_empty()).then_some(Note { quote, why })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Review { revised, notes })
}

/// The fenced JSON block, or the outermost braces when the model skipped the
/// fence.
fn extract_json(reply: &str) -> Option<String> {
    if let Some(start) = reply.find("```") {
        let rest = &reply[start + 3..];
        let rest = rest.strip_prefix("json").unwrap_or(rest);
        if let Some(end) = rest.find("```") {
            let body = rest[..end].trim();
            if !body.is_empty() {
                return Some(body.to_string());
            }
        }
    }
    let start = reply.find('{')?;
    let end = reply.rfind('}')?;
    (end > start).then(|| reply[start..=end].to_string())
}

/// Locate each note inside `text`, which is the revised text AS IT STANDS —
/// the developer may have edited it since the review. A quote that no longer
/// occurs is simply not highlighted: the edit answered it. Overlapping
/// highlights are dropped rather than nested, so the painting stays a flat
/// list of ranges.
pub fn locate(text: &str, notes: &[Note]) -> Vec<Highlight> {
    let mut out: Vec<Highlight> = Vec::new();
    for note in notes {
        let mut from = 0;
        while let Some(rel) = text[from..].find(&note.quote) {
            let start = from + rel;
            let range = start..start + note.quote.len();
            let overlaps = out
                .iter()
                .any(|h| range.start < h.range.end && h.range.start < range.end);
            if !overlaps {
                out.push(Highlight {
                    range,
                    why: note.why.clone(),
                });
                break;
            }
            from = range.end;
        }
    }
    out.sort_by_key(|h| h.range.start);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fenced_review_parses_into_text_and_notes() {
        let reply = r#"Aqui está.
```json
{"revised": "Adicione 15 TextBoxes em 3 colunas.", "notes": [
  {"quote": "3 colunas", "why": "A distribuição das 15 caixas pelas 3 colunas não foi dita."}
]}
```"#;
        let got = parse_review(reply).expect("parses");
        assert_eq!(got.revised, "Adicione 15 TextBoxes em 3 colunas.");
        assert_eq!(got.notes.len(), 1);
        assert_eq!(got.notes[0].quote, "3 colunas");
    }

    #[test]
    fn a_bare_json_reply_still_parses_and_notes_may_be_absent() {
        let got = parse_review(r#"{"revised": "Texto revisado."}"#).expect("parses");
        assert_eq!(got.revised, "Texto revisado.");
        assert!(got.notes.is_empty());
        // An empty revision is not a review.
        assert!(parse_review(r#"{"revised": "   "}"#).is_none());
        assert!(parse_review("desculpe, não consegui").is_none());
    }

    #[test]
    fn notes_are_located_in_the_text_the_developer_is_looking_at() {
        let text = "Adicione 15 TextBoxes em 3 colunas com cores variadas.";
        let notes = vec![
            Note {
                quote: "3 colunas".into(),
                why: "distribuição não dita".into(),
            },
            Note {
                quote: "cores variadas".into(),
                why: "quais cores?".into(),
            },
        ];
        let hl = locate(text, &notes);
        assert_eq!(hl.len(), 2);
        assert_eq!(&text[hl[0].range.clone()], "3 colunas");
        assert_eq!(&text[hl[1].range.clone()], "cores variadas");
        // …and they come in reading order, whatever order the model sent them.
        assert!(hl[0].range.start < hl[1].range.start);
    }

    /// The developer edits the revised text; a quote that no longer occurs was
    /// answered by that edit and must simply stop being highlighted.
    #[test]
    fn an_edited_away_quote_is_not_highlighted() {
        let notes = vec![Note {
            quote: "cores variadas".into(),
            why: "quais cores?".into(),
        }];
        assert!(locate("Adicione 15 TextBoxes azuis.", &notes).is_empty());
    }

    #[test]
    fn overlapping_notes_do_not_nest() {
        let text = "faça todos do mesmo tamanho exceto o último";
        let notes = vec![
            Note {
                quote: "todos do mesmo tamanho".into(),
                why: "a".into(),
            },
            Note {
                quote: "do mesmo tamanho exceto".into(),
                why: "b".into(),
            },
        ];
        let hl = locate(text, &notes);
        assert_eq!(hl.len(), 1, "the second overlaps the first: {hl:?}");
    }
}
