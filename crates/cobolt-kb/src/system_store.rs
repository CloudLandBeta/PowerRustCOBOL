// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The read side of the IDE's chunked System KB store** (spec 084 T5).
//!
//! The IDE's assistant writes and searches `~/PowerRustCOBOL/data/chunked.data`
//! through `cobolt_agents::chunked_knowledge`, which links rig, tokio and
//! candle. A coding agent's `kb_search` (in the IDE's tool server and in
//! `rcrun mcp`) must answer from the same records without those, so the record
//! layout and the scoring live here, and `chunked_knowledge::search` calls
//! [`search_database`] — one implementation for every caller.
//!
//! The query vector is the caller's: the assistant passes its active embedder's
//! (semantic when the model is cached), a coding agent passes the hashing one.
//! A hashing query scores a semantic record by its text, so the shipped store
//! is useful without the model.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use redb::{ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};

use crate::embed::{HashingEmbedder, HASHING_DIMENSIONS, HASHING_STAMP};

/// chunk key → bincode [`ChunkRecord`]. Keys are `"<doc path>\u{1}<ordinal>"`.
pub const CHUNKS: TableDefinition<&str, &[u8]> = TableDefinition::new("chunks");

/// Width of every stored vector (the hashing embedder's and E5-small's).
pub const VECTOR_DIMENSIONS: usize = HASHING_DIMENSIONS;

/// One stored record: a subject's text with the embedding of exactly that text.
/// Field order is the bincode layout of the store — never reorder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRecord {
    /// What the record describes, e.g. `DataGrid`, `DataGrid · onRowSelect`.
    pub subject: String,
    /// `control` | `property` | `method` | `event` | `section` | `document`.
    pub kind: String,
    /// The text itself — at most 512 characters.
    pub content: String,
    /// Key of the PREVIOUS record when this one continues an overflowing
    /// content chain; `None` for a chain head (or a record that fits).
    pub parent: Option<String>,
    /// Vector width stamp (must equal [`VECTOR_DIMENSIONS`]).
    pub dimensions: u32,
    /// The embedding of `content`.
    pub embedding: Vec<f32>,
    /// Stamp of the embedder that produced `embedding` (`hashing`, or the
    /// semantic model's).
    pub embedder: String,
}

/// One retrieved subject: the record that matched, with its chain reassembled
/// so the caller sees the subject's complete text.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkHit {
    /// Source document, relative to the store root (`Knowledge Base/...`).
    pub source_path: String,
    pub subject: String,
    pub kind: String,
    pub score: f32,
    /// The full text of the subject (every record of its chain, in order).
    pub content: String,
}

/// The document a chunk key belongs to.
pub fn doc_of_key(key: &str) -> &str {
    key.split('\u{1}').next().unwrap_or(key)
}

/// The IDE's System KB store: `~/PowerRustCOBOL/data/chunked.data`.
pub fn ide_store_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("PowerRustCOBOL")
        .join("data")
        .join("chunked.data")
}

/// Rank the store's subjects against a query vector.
///
/// `query_stamp` names the embedder that produced `query_vector`. A record from
/// the same embedder is scored by dot product; under a hashing query a record
/// from another embedder is scored by its TEXT (lexically) rather than skipped;
/// under a semantic query a hashing record is skipped (incomparable spaces).
/// The best record of each chain surfaces the whole chain once.
pub fn search_database(
    database: &redb::ReadOnlyDatabase,
    query_vector: &[f32],
    query_stamp: &str,
    limit: usize,
) -> Result<Vec<ChunkHit>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let read = database.begin_read().map_err(|e| e.to_string())?;
    let chunks = match read.open_table(CHUNKS) {
        Ok(table) => table,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let hashing_query = query_stamp == HASHING_STAMP;
    struct Scored {
        key: String,
        record: ChunkRecord,
        score: f32,
    }
    let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(l, r)| l * r).sum::<f32>();
    let mut all: Vec<Scored> = Vec::new();
    for entry in chunks.iter().map_err(|e| e.to_string())? {
        let (key, value) = entry.map_err(|e| e.to_string())?;
        let Ok(record) = bincode::deserialize::<ChunkRecord>(value.value()) else {
            continue;
        };
        if record.embedding.len() != VECTOR_DIMENSIONS {
            continue;
        }
        let score = if record.embedder == query_stamp {
            dot(query_vector, &record.embedding)
        } else if hashing_query {
            dot(query_vector, &HashingEmbedder::vector(&record.content))
        } else {
            continue;
        };
        all.push(Scored { key: key.value().to_string(), record, score });
    }
    let head_of = |scored: &Scored| -> String {
        let mut key = scored.key.clone();
        let mut parent = scored.record.parent.clone();
        while let Some(previous) = parent {
            match all.iter().find(|s| s.key == previous) {
                Some(prev) => {
                    key = prev.key.clone();
                    parent = prev.record.parent.clone();
                }
                None => break,
            }
        }
        key
    };
    let mut best: BTreeMap<String, f32> = BTreeMap::new();
    for scored in &all {
        if scored.score <= 0.0 {
            continue;
        }
        let entry = best.entry(head_of(scored)).or_insert(scored.score);
        if scored.score > *entry {
            *entry = scored.score;
        }
    }
    let mut ranked: Vec<(String, f32)> = best.into_iter().collect();
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1));
    ranked.truncate(limit);
    let mut hits = Vec::new();
    for (head_key, score) in ranked {
        let Some(head) = all.iter().find(|s| s.key == head_key) else {
            continue;
        };
        let mut content = head.record.content.clone();
        let mut tail_key = head_key.clone();
        while let Some(next) = all
            .iter()
            .find(|s| s.record.parent.as_deref() == Some(tail_key.as_str()))
        {
            content.push_str(&next.record.content);
            tail_key = next.key.clone();
        }
        hits.push(ChunkHit {
            source_path: doc_of_key(&head_key).to_string(),
            subject: head.record.subject.clone(),
            kind: head.record.kind.clone(),
            score,
            content,
        });
    }
    Ok(hits)
}

/// Search a store file with the hashing embedder — no model, no GPU, instant.
/// A missing file is no results, not an error.
pub fn search_lexical(db_path: &Path, query: &str, limit: usize) -> Result<Vec<ChunkHit>, String> {
    if query.trim().is_empty() || limit == 0 || !db_path.exists() {
        return Ok(Vec::new());
    }
    let database = redb::ReadOnlyDatabase::open(db_path).map_err(|e| e.to_string())?;
    search_database(&database, &HashingEmbedder::vector(query), HASHING_STAMP, limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(subject: &str, content: &str, parent: Option<&str>, embedder: &str) -> ChunkRecord {
        let embedding = if embedder == HASHING_STAMP {
            HashingEmbedder::vector(content)
        } else {
            // A "semantic" vector the hashing query cannot compare with.
            let mut v = vec![0.0; VECTOR_DIMENSIONS];
            v[0] = 1.0;
            v
        };
        ChunkRecord {
            subject: subject.into(),
            kind: "control".into(),
            content: content.into(),
            parent: parent.map(str::to_owned),
            dimensions: VECTOR_DIMENSIONS as u32,
            embedding,
            embedder: embedder.into(),
        }
    }

    /// A hashing query ranks hashing AND semantic records (the latter by their
    /// text), reassembles a chain into one hit, and returns the source path.
    #[test]
    fn a_hashing_search_ranks_reassembles_and_reads_semantic_records() {
        let path = std::env::temp_dir().join(format!("prc-084-store-{}.data", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let db = redb::Database::create(&path).unwrap();
            let tx = db.begin_write().unwrap();
            {
                let mut t = tx.open_table(CHUNKS).unwrap();
                let rows = [
                    ("Knowledge Base/controls.md\u{1}00001", record("SideMenu", "SideMenu is the navigation rail; items open forms in the ", None, HASHING_STAMP)),
                    ("Knowledge Base/controls.md\u{1}00002", record("SideMenu", "ContentPane of the shell.", Some("Knowledge Base/controls.md\u{1}00001"), HASHING_STAMP)),
                    ("Knowledge Base/layout.md\u{1}00001", record("ContentPane", "ContentPane hosts the form a SideMenu item opens.", None, "bert-multilingual-e5-small")),
                    ("Knowledge Base/controls.md\u{1}00003", record("Timer", "Timer raises onTick at its Interval.", None, HASHING_STAMP)),
                ];
                for (k, r) in rows {
                    t.insert(k, bincode::serialize(&r).unwrap().as_slice()).unwrap();
                }
            }
            tx.commit().unwrap();
        }
        let hits = search_lexical(&path, "SideMenu ContentPane", 5).unwrap();
        let subjects: Vec<&str> = hits.iter().map(|h| h.subject.as_str()).collect();
        assert_eq!(subjects.len(), 2, "Timer does not match: {subjects:?}");
        assert!(subjects.contains(&"SideMenu") && subjects.contains(&"ContentPane"));
        let side = hits.iter().find(|h| h.subject == "SideMenu").unwrap();
        assert_eq!(side.content, "SideMenu is the navigation rail; items open forms in the ContentPane of the shell.");
        assert_eq!(side.source_path, "Knowledge Base/controls.md");
        assert!(search_lexical(&path, "", 5).unwrap().is_empty());
        assert!(search_lexical(&path.with_extension("missing"), "x", 5).unwrap().is_empty());
        let _ = std::fs::remove_file(&path);
        println!("system store: 4 records, 'SideMenu ContentPane' -> 2 subjects (1 chain of 2 reassembled, 1 semantic record scored by text), empty query and missing file -> none");
    }
}
