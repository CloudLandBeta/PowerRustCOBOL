// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Searching a collection (spec 068 R25, R26, R26a, R33).
//!
//! Scoring is lifted from the IDE Knowledge Base: vectors are unit length, so
//! the dot product is the cosine similarity; a split section is scored by its
//! best part and returned whole. The embedder is passed in, never read from a
//! global.
//!
//! Search is **semantic** when this application's embedder is a model and made
//! the collection's vectors. It is **lexical** — and says why — when the
//! embedder is the hashing one, when the model cannot be reached, or when the
//! collection was indexed by a different embedder than this application uses.

use std::collections::BTreeMap;

use crate::embed::{Embedder, HashingEmbedder, HASHING_STAMP};
use crate::refresh::all_passages;
use crate::store::{Collection, KbError};

/// How a search was scored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Semantic,
    Lexical,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Semantic => "Semantic",
            Mode::Lexical => "Lexical",
        }
    }
}

/// One passage found.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// The document, relative to the collection's documents folder — and,
    /// inside an archive, the path within it: `old.zip › legal/nda.docx`.
    pub document: String,
    /// Where in it: `"handbook › Leave"`.
    pub heading: String,
    /// The whole section, a split section reassembled.
    pub passage: String,
    pub score: f32,
}

/// A search's hits, best first, and how they were scored.
#[derive(Debug, Clone, PartialEq)]
pub struct Results {
    pub hits: Vec<Hit>,
    pub mode: Mode,
    /// Why the search is lexical, when it is.
    pub reason: Option<String>,
}

/// Search `collection` for `query`, returning at most `limit` hits.
pub fn search(
    collection: &Collection,
    embedder: &dyn Embedder,
    query: &str,
    limit: usize,
) -> Result<Results, KbError> {
    let ours = embedder.stamp();
    let theirs = collection.embedder_stamp()?;
    let (mode, reason, query_vector) = if ours == HASHING_STAMP {
        (
            Mode::Lexical,
            Some("this application uses the lexical embedder".to_string()),
            None,
        )
    } else if theirs.as_deref().is_some_and(|s| s != ours) {
        (
            Mode::Lexical,
            Some(format!(
                "this collection was indexed with {}, and this application uses {ours}",
                theirs.unwrap_or_default()
            )),
            None,
        )
    } else {
        match embedder.embed_query(query) {
            Ok(v) => (Mode::Semantic, None, Some(v)),
            Err(e) => (Mode::Lexical, Some(e), None),
        }
    };
    let lexical_query = HashingEmbedder::vector(query);
    let dot = |a: &[f32], b: &[f32]| -> f32 { a.iter().zip(b).map(|(x, y)| x * y).sum() };

    // (document, chain root) → (best score, heading, parts by ordinal).
    type Group = (f32, String, BTreeMap<u32, String>);
    let mut groups: BTreeMap<(String, u32), Group> = BTreeMap::new();
    for (key, p) in all_passages(collection)? {
        let (document, ordinal) = match key.split_once('\u{1}') {
            Some((d, o)) => (d.to_string(), o.parse::<u32>().unwrap_or(0)),
            None => continue,
        };
        let document = match &p.part {
            Some(part) => format!("{document}{}{part}", crate::convert::PATH_SEPARATOR),
            None => document,
        };
        let score = match &query_vector {
            // Semantic: the stored vector when it is ours; a passage stored
            // text-only is still found, lexically.
            Some(q) if p.stamp == ours && p.vector.len() == q.len() => dot(q, &p.vector),
            _ => dot(&lexical_query, &HashingEmbedder::vector(&p.content)),
        };
        let root = p.chain.unwrap_or(ordinal);
        let entry = groups
            .entry((document, root))
            .or_insert_with(|| (f32::MIN, p.heading.clone(), BTreeMap::new()));
        entry.0 = entry.0.max(score);
        entry.2.insert(ordinal, p.content);
    }
    let mut hits: Vec<Hit> = groups
        .into_iter()
        .filter(|(_, (score, _, _))| *score > 0.0)
        .map(|((document, _), (score, heading, parts))| Hit {
            document,
            heading,
            passage: parts.into_values().collect::<Vec<_>>().concat(),
            score,
        })
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.document.cmp(&b.document))
    });
    hits.truncate(limit);
    Ok(Results { hits, mode, reason })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::Converters;
    use crate::refresh::{refresh, reindex, Progress, Scope};
    use std::sync::atomic::AtomicBool;

    fn quiet() -> impl FnMut(&Progress) {
        |_| {}
    }

    fn corpus(c: &Collection) {
        let docs = c.documents_dir();
        std::fs::write(docs.join("leave.md"), "# Leave\nAnnual leave is twenty working days.\n## Carry-over\nUp to five days carry over to next year.").unwrap();
        std::fs::write(docs.join("pay.md"), "# Pay\nSalaries are paid monthly on the last working day.").unwrap();
        std::fs::write(docs.join("travel.md"), "# Travel\nBook flights through the travel desk. Economy class for trips under six hours.").unwrap();
        let long = format!("# Security\n{}", "Badge access is required in every building. ".repeat(40));
        std::fs::write(docs.join("security.md"), long).unwrap();
    }

    const QUERIES: [&str; 10] = [
        "annual leave days", "carry over", "salary paid monthly", "flights travel desk",
        "economy class", "badge access building", "working day", "next year", "trips hours", "security",
    ];

    fn ranks(c: &Collection) -> Vec<Vec<(String, String)>> {
        QUERIES
            .iter()
            .map(|q| {
                search(c, &HashingEmbedder, q, 3)
                    .unwrap()
                    .hits
                    .into_iter()
                    .map(|h| (h.document, h.heading))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn hits_name_their_document_and_heading_and_reassemble_split_sections() {
        let root = tempfile::tempdir().unwrap();
        let c = Collection::open(root.path(), "hr").unwrap();
        corpus(&c);
        refresh(&c, &HashingEmbedder, &Converters::default(), Scope::All, &mut quiet(), &AtomicBool::new(false)).unwrap();
        let r = search(&c, &HashingEmbedder, "carry over five days", 5).unwrap();
        assert_eq!(r.mode, Mode::Lexical);
        assert_eq!(r.hits[0].document, "leave.md");
        assert_eq!(r.hits[0].heading, "leave › Leave › Carry-over");
        let sec = search(&c, &HashingEmbedder, "badge access", 1).unwrap();
        assert!(sec.hits[0].passage.len() > 1500, "the split section comes back whole");
    }

    /// Deleting the index and refreshing gives the same ranking (AC6).
    #[test]
    fn a_rebuilt_index_ranks_the_same() {
        let root = tempfile::tempdir().unwrap();
        let c = Collection::open(root.path(), "hr").unwrap();
        corpus(&c);
        let cancel = AtomicBool::new(false);
        refresh(&c, &HashingEmbedder, &Converters::default(), Scope::All, &mut quiet(), &cancel).unwrap();
        let before = ranks(&c);
        let index = c.index_path();
        drop(c);
        std::fs::remove_file(&index).unwrap();
        let c = Collection::open(root.path(), "hr").unwrap();
        refresh(&c, &HashingEmbedder, &Converters::default(), Scope::All, &mut quiet(), &cancel).unwrap();
        assert_eq!(ranks(&c), before);
        println!("rebuilt index: identical top-3 for {} queries", QUERIES.len());
    }

    struct Model(&'static str, bool);
    impl Embedder for Model {
        fn stamp(&self) -> String {
            format!("endpoint:ollama:{}", self.0)
        }
        fn embed(&self, t: &[&str]) -> Result<Vec<Vec<f32>>, String> {
            if self.1 {
                Ok(t.iter().map(|s| HashingEmbedder::vector(s)).collect())
            } else {
                Err("the embedding server at http://127.0.0.1:9 could not be reached".into())
            }
        }
    }

    /// An unreachable model searches lexically and says why (AC12); a
    /// mismatched application searches lexically and leaves the index as it
    /// was (AC13a); Reindex changes the collection's embedder (AC13).
    #[test]
    fn lexical_fallbacks_say_why_and_reindex_switches_the_embedder() {
        let root = tempfile::tempdir().unwrap();
        let c = Collection::open(root.path(), "hr").unwrap();
        corpus(&c);
        let conv = Converters::default();
        let cancel = AtomicBool::new(false);
        refresh(&c, &Model("a", true), &conv, Scope::All, &mut quiet(), &cancel).unwrap();
        let semantic = search(&c, &Model("a", true), "annual leave", 3).unwrap();
        assert_eq!(semantic.mode, Mode::Semantic);

        let down = search(&c, &Model("a", false), "annual leave", 3).unwrap();
        assert_eq!(down.mode, Mode::Lexical);
        assert!(down.reason.unwrap().contains("could not be reached"));
        assert_eq!(down.hits[0].document, "leave.md", "still answers");

        let index = std::fs::read(c.index_path()).unwrap();
        let other = search(&c, &Model("b", true), "annual leave", 3).unwrap();
        assert_eq!(other.mode, Mode::Lexical);
        assert!(other.reason.unwrap().contains("indexed with endpoint:ollama:a"));
        assert_eq!(std::fs::read(c.index_path()).unwrap(), index, "the index is untouched");

        reindex(&c, &Model("b", true), &conv, &mut quiet(), &cancel).unwrap();
        assert_eq!(c.embedder_stamp().unwrap().as_deref(), Some("endpoint:ollama:b"));
        assert_eq!(search(&c, &Model("b", true), "annual leave", 3).unwrap().mode, Mode::Semantic);
    }
}
