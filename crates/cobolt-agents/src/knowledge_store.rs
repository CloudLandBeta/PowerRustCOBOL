// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The vector Knowledge Base shared by the IDE and by every project.
//!
//! Two stores of identical shape, differing only in what they hold and where
//! they live:
//!
//! | Store   | Location                | Contents                                   |
//! |---------|-------------------------|--------------------------------------------|
//! | IDE     | `~/PowerRustCOBOL/data` | one document per control, per extension …   |
//! | Project | `<project>/data`        | the developer's own Knowledge Base folder   |
//!
//! Both are [`embedvec`] indexes — HNSW with E8 lattice quantization, cosine
//! similarity — persisted through Fjall. The vector index is the *retrieval*
//! surface; the authoritative text stays in the `.md` document on disk, and a
//! hit carries the path plus a short excerpt so a caller can decide whether to
//! load the full document into an agent's context.
//!
//! ## Why one subject per document
//!
//! Retrieval can only be as selective as its smallest indexable unit. A single
//! document covering all 34 control types cannot answer "what events does
//! DataGrid support?" without dragging in the other 33 — which is how the whole
//! catalogue ended up in every request in the first place. One control per
//! document means a hit *is* the answer, and the context grows by one control
//! rather than by the catalogue.
//!
//! ## Embeddings
//!
//! `embedvec` stores vectors; it does not create them. [`Embedder`] is that
//! seam. The default [`HashingEmbedder`] is a deterministic hashing
//! bag-of-words: offline, dependency-free, and effective for the keyed lookups
//! that dominate here ("DataGrid", "Timer", `onTick`), because an identifier
//! either occurs in the document or does not. It is NOT semantic — it cannot
//! match "grid control" to `DataGrid`, or a requirements document phrased in
//! prose to the schema it describes. Swapping in a real embedding model is a
//! matter of implementing this trait; nothing in the storage layer changes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use embedvec::{Distance, EmbedVec, Quantization};
use serde::{Deserialize, Serialize};

/// Vector width. 384 matches the previous SQLite index and the common
/// sentence-transformer sizes, so a future model swap needs no reindex format
/// change.
pub const VECTOR_DIMENSIONS: usize = 384;

/// HNSW search beam. Wider than `k` so quantization error and deleted records
/// still leave enough live candidates.
const EF_SEARCH: usize = 64;

/// How much document text a hit carries back. Enough for an agent to judge
/// relevance; the full text stays in the document.
const EXCERPT_CHARS: usize = 600;

/// Turns text into a vector. See the module note on embeddings.
pub trait Embedder: Send + Sync {
    fn dimensions(&self) -> usize;
    /// Embed a DOCUMENT (a passage being indexed).
    fn embed(&self, text: &str) -> Vec<f32>;
    /// Embed a QUERY (the text being searched for). Defaults to [`embed`]:
    /// symmetric embedders make no distinction. Asymmetric retrieval models
    /// (E5) are trained with distinct query/passage prefixes and override this
    /// — searching an E5 index with passage-embedded queries quietly costs
    /// most of the model's retrieval quality.
    ///
    /// [`embed`]: Embedder::embed
    fn embed_query(&self, text: &str) -> Vec<f32> {
        self.embed(text)
    }
}

/// Deterministic hashing bag-of-words — the offline default.
///
/// Each token is hashed to a bucket with a sign drawn from the same hash, so
/// collisions cancel rather than accumulate; the result is L2-normalised, which
/// makes the cosine distance a plain dot product.
#[derive(Debug, Clone, Default)]
pub struct HashingEmbedder;

impl HashingEmbedder {
    fn fnv1a(text: &str) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in text.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }
}

impl Embedder for HashingEmbedder {
    fn dimensions(&self) -> usize {
        VECTOR_DIMENSIONS
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0_f32; VECTOR_DIMENSIONS];
        for token in text
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .filter(|token| token.len() > 1)
        {
            let token = token.to_lowercase();
            let hash = Self::fnv1a(&token);
            vector[(hash as usize) % VECTOR_DIMENSIONS] +=
                if hash & (1_u64 << 63) == 0 { 1.0 } else { -1.0 };
        }
        let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > f32::EPSILON {
            for value in &mut vector {
                *value /= norm;
            }
        }
        vector
    }
}

/// One retrieved document: where it lives, how well it matched, and enough text
/// to decide whether to load the rest.
#[derive(Debug, Clone, PartialEq)]
pub struct KnowledgeHit {
    /// Path relative to the store root, e.g. `Knowledge Base/controls/DataGrid.md`.
    pub path: String,
    /// Short human-readable subject, e.g. `DataGrid`.
    pub subject: String,
    pub score: f32,
    pub excerpt: String,
}

/// What the manifest remembers about an indexed document, so an unchanged file
/// is skipped and a changed one replaces its old vector instead of joining it.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexedRecord {
    id: usize,
    hash: u64,
    subject: String,
    updated_unix: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Manifest {
    documents: BTreeMap<String, IndexedRecord>,
}

/// The IDE's own Knowledge Base directory: `~/PowerRustCOBOL/data`.
///
/// IDE-owned reference material belongs to the IDE, not to whichever project
/// happens to be open, so it is written once per machine and shared by every
/// project rather than copied into each one.
pub fn ide_data_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("PowerRustCOBOL").join("data")
}

/// A project's Knowledge Base directory: `<project>/data`.
pub fn project_data_dir(project_root: &Path) -> PathBuf {
    project_root.join("data")
}

/// A vector Knowledge Base rooted at one data directory.
pub struct KnowledgeStore {
    db: EmbedVec,
    embedder: Arc<dyn Embedder>,
    data_dir: PathBuf,
    manifest: Manifest,
}

impl KnowledgeStore {
    /// Open (creating if absent) the store under `data_dir`, with the default
    /// hashing embedder.
    pub fn open(data_dir: &Path) -> Result<Self, String> {
        Self::open_with(data_dir, Arc::new(HashingEmbedder))
    }

    /// Open with a specific embedder. The store's vector width follows the
    /// embedder, so a model swap is a one-line change at the call site.
    pub fn open_with(data_dir: &Path, embedder: Arc<dyn Embedder>) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| format!("knowledge data directory {}: {e}", data_dir.display()))?;
        let index_path = data_dir.join("knowledge");
        let db = EmbedVec::builder()
            .dimension(embedder.dimensions())
            .metric(Distance::Cosine)
            .quantization(Quantization::e8_default())
            .persistence(index_path.to_string_lossy().to_string())
            .build()
            .map_err(|e| format!("knowledge index could not be opened: {e}"))?;
        let manifest = Self::load_manifest(data_dir);
        Ok(Self {
            db,
            embedder,
            data_dir: data_dir.to_path_buf(),
            manifest,
        })
    }

    fn manifest_path(data_dir: &Path) -> PathBuf {
        data_dir.join("knowledge-manifest.json")
    }

    fn load_manifest(data_dir: &Path) -> Manifest {
        std::fs::read_to_string(Self::manifest_path(data_dir))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn save_manifest(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.manifest).map_err(|e| e.to_string())?;
        std::fs::write(Self::manifest_path(&self.data_dir), json).map_err(|e| e.to_string())
    }

    /// Index one document under `path` (a store-relative identifier). Content
    /// unchanged since the last index is skipped; changed content replaces the
    /// previous vector so a re-index never leaves a stale duplicate behind.
    /// Returns whether the index was actually written.
    pub fn index_document(
        &mut self,
        path: &str,
        subject: &str,
        content: &str,
    ) -> Result<bool, String> {
        let hash = HashingEmbedder::fnv1a(content);
        if let Some(existing) = self.manifest.documents.get(path) {
            if existing.hash == hash {
                return Ok(false); // unchanged — nothing to do
            }
            // Changed: drop the superseded vector before adding the new one.
            let _ = self.db.delete_internal(existing.id);
        }
        let vector = self.embedder.embed(content);
        let payload = serde_json::json!({
            "path": path,
            "subject": subject,
            "excerpt": excerpt_of(content),
        });
        let id = self
            .db
            .add_internal(&vector, payload)
            .map_err(|e| format!("indexing {path}: {e}"))?;
        let updated_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        self.manifest.documents.insert(
            path.to_string(),
            IndexedRecord {
                id,
                hash,
                subject: subject.to_string(),
                updated_unix,
            },
        );
        Ok(true)
    }

    /// Drop every indexed document whose path is not in `keep` — the documents
    /// that were deleted on disk since the last sync.
    pub fn retain_only(&mut self, keep: &[String]) -> usize {
        let stale: Vec<String> = self
            .manifest
            .documents
            .keys()
            .filter(|path| !keep.iter().any(|k| k == *path))
            .cloned()
            .collect();
        for path in &stale {
            if let Some(record) = self.manifest.documents.remove(path) {
                let _ = self.db.delete_internal(record.id);
            }
        }
        stale.len()
    }

    /// Persist the manifest. Call once after a batch of `index_document` calls.
    pub fn commit(&self) -> Result<(), String> {
        self.save_manifest()
    }

    /// The `k` documents closest to `query` by cosine similarity.
    pub fn search(&self, query: &str, k: usize) -> Result<Vec<KnowledgeHit>, String> {
        if k == 0 || self.manifest.documents.is_empty() {
            return Ok(Vec::new());
        }
        let vector = self.embedder.embed(query);
        let hits = self
            .db
            .search_internal(&vector, k, EF_SEARCH.max(k), None)
            .map_err(|e| format!("knowledge search failed: {e}"))?;
        Ok(hits
            .into_iter()
            .map(|hit| KnowledgeHit {
                path: string_field(&hit.payload, "path"),
                subject: string_field(&hit.payload, "subject"),
                score: hit.score,
                excerpt: string_field(&hit.payload, "excerpt"),
            })
            .collect())
    }

    /// How many documents the store currently indexes.
    pub fn len(&self) -> usize {
        self.manifest.documents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.manifest.documents.is_empty()
    }
}

fn string_field(payload: &serde_json::Value, key: &str) -> String {
    payload
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// The leading slice of a document, cut on a character boundary.
fn excerpt_of(content: &str) -> String {
    let trimmed = content.trim();
    match trimmed.char_indices().nth(EXCERPT_CHARS) {
        None => trimmed.to_string(),
        Some((byte, _)) => format!("{}…", &trimmed[..byte]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (tempfile::TempDir, KnowledgeStore) {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = KnowledgeStore::open(dir.path()).expect("store opens");
        (dir, store)
    }

    #[test]
    fn a_document_is_retrievable_by_its_subject() {
        let (_dir, mut store) = temp_store();
        store
            .index_document(
                "Knowledge Base/controls/DataGrid.md",
                "DataGrid",
                "# DataGrid\n\nEvents: onCellClick, onRowSelect, onSelectionChanged.",
            )
            .expect("indexes");
        store
            .index_document(
                "Knowledge Base/controls/Timer.md",
                "Timer",
                "# Timer\n\nEvents: onTick. Properties: Interval, Enabled.",
            )
            .expect("indexes");
        store.commit().expect("commits");

        let hits = store.search("onCellClick DataGrid", 1).expect("searches");
        assert_eq!(hits.len(), 1);
        assert_eq!(
            hits[0].subject, "DataGrid",
            "the control asked about is the one returned, not the whole catalogue"
        );
        assert!(hits[0].excerpt.contains("onCellClick"));
    }

    /// Re-indexing changed content must REPLACE the old vector. If it merely
    /// added another, a control would accumulate one stale vector per edit and
    /// searches would start returning superseded text.
    #[test]
    fn reindexing_replaces_rather_than_duplicates() {
        let (_dir, mut store) = temp_store();
        let path = "Knowledge Base/controls/Slider.md";
        assert!(store
            .index_document(path, "Slider", "# Slider\n\nEvents: onChange.")
            .expect("indexes"));
        // Unchanged content is skipped entirely.
        assert!(
            !store
                .index_document(path, "Slider", "# Slider\n\nEvents: onChange.")
                .expect("indexes"),
            "unchanged content must not be re-embedded"
        );
        // Changed content replaces.
        assert!(store
            .index_document(path, "Slider", "# Slider\n\nEvents: onChange, onValueChanged.")
            .expect("indexes"));
        assert_eq!(store.len(), 1, "one document, not two");
    }

    /// A document deleted on disk must leave the index, or retrieval keeps
    /// serving text that no longer exists.
    #[test]
    fn documents_removed_on_disk_leave_the_index() {
        let (_dir, mut store) = temp_store();
        store
            .index_document("a.md", "A", "alpha content")
            .expect("indexes");
        store
            .index_document("b.md", "B", "bravo content")
            .expect("indexes");
        assert_eq!(store.len(), 2);
        let dropped = store.retain_only(&["a.md".to_string()]);
        assert_eq!(dropped, 1);
        assert_eq!(store.len(), 1);
    }

    /// The index must survive a restart — it is persisted through Fjall, and
    /// the manifest must come back with it.
    #[test]
    fn the_store_reopens_with_its_documents() {
        let dir = tempfile::tempdir().expect("temp dir");
        {
            let mut store = KnowledgeStore::open(dir.path()).expect("opens");
            store
                .index_document("Knowledge Base/controls/Button.md", "Button", "onClick")
                .expect("indexes");
            store.commit().expect("commits");
        }
        let store = KnowledgeStore::open(dir.path()).expect("reopens");
        assert_eq!(store.len(), 1, "the manifest survived the restart");
        let hits = store.search("onClick", 1).expect("searches");
        assert_eq!(hits[0].subject, "Button");
    }

    #[test]
    fn the_ide_store_lives_outside_any_project() {
        let ide = ide_data_dir();
        assert!(ide.ends_with("PowerRustCOBOL/data"));
        let project = project_data_dir(Path::new("/tmp/SomeProject"));
        assert_eq!(project, Path::new("/tmp/SomeProject/data"));
    }

    #[test]
    fn an_excerpt_is_cut_on_a_character_boundary() {
        let content = "á".repeat(EXCERPT_CHARS + 50);
        let excerpt = excerpt_of(&content); // must not panic on a multi-byte cut
        assert!(excerpt.ends_with('…'));
    }
}
