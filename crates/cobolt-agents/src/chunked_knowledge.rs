// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The chunked Knowledge Base: one record per SUBJECT, not per document.
//!
//! Retrieval can only be as selective as its smallest indexable unit. A single
//! document covering all controls cannot answer "what events does DataGrid
//! support?" without dragging in every other control — so the documents are
//! converted into many small records: one per control, property, method,
//! event, or prose section. Each record's content is a COBOL-style
//! `PIC X(512)` field ([`CONTENT_PIC_CHARS`]); content that does not fit
//! continues in another record **linked to the previous one** through
//! [`ChunkRecord::parent`], forming a chain that search reassembles.
//!
//! Every record's content is embedded individually (the same
//! [`crate::knowledge_store::Embedder`] seam the document index uses), and the
//! vector is stored **on the record it describes** — the association between
//! the tokens and the original text is the record itself.
//!
//! Two stores of identical shape:
//!
//! | Store   | File                                           |
//! |---------|------------------------------------------------|
//! | IDE     | `~/PowerRustCOBOL/data/chunked.data`           |
//! | Project | `<project>/data/<name-no-spaces>-chunked.data` |
//!
//! Syncing preserves the source files: when a document is new or changed, its
//! existing records and embeddings are removed and the document is chunked and
//! embedded again; a document deleted on disk loses all its records, so search
//! can never return text that no longer exists.

use std::path::{Path, PathBuf};

use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};
use serde::{Deserialize, Serialize};

use crate::bert_embedder::EmbedderKind;
use crate::knowledge_store::{Embedder, HashingEmbedder, VECTOR_DIMENSIONS};
use crate::project_knowledge;

/// The semantic embedder stamp a SHIPPED store is built with. Records carrying
/// this stamp are never downgraded by a machine that only has the hashing
/// fallback — see [`sync_documents_with_progress`].
const SEMANTIC_STAMP: &str = "bert-multilingual-e5-small";

/// The record content field: `PIC X(512)` — at most 512 characters.
pub const CONTENT_PIC_CHARS: usize = 512;

/// chunk key → bincode [`ChunkRecord`]. Keys are `"<doc path>\u{1}<ordinal>"`,
/// so one document's records form a contiguous, prefix-addressable range.
const CHUNKS: TableDefinition<&str, &[u8]> = TableDefinition::new("chunks");
/// document path → content hash (mixed with the embedder stamp), for
/// skip-unchanged syncs.
const DOCS: TableDefinition<&str, u64> = TableDefinition::new("documents");

/// One stored record: a subject's text (≤ [`CONTENT_PIC_CHARS`] chars) with
/// the embedding of exactly that text.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChunkRecord {
    /// What the record describes, e.g. `DataGrid`, `DataGrid · onRowSelect`.
    subject: String,
    /// `control` | `property` | `method` | `event` | `section` | `document`.
    kind: String,
    /// The text itself — `PIC X(512)`.
    content: String,
    /// Key of the PREVIOUS record when this one continues an overflowing
    /// content chain; `None` for a chain head (or a record that fits).
    parent: Option<String>,
    /// Vector width stamp (must equal [`VECTOR_DIMENSIONS`]).
    dimensions: u32,
    /// The embedding of `content`.
    embedding: Vec<f32>,
    /// [`EmbedderKind::as_str`] of the embedder that produced `embedding`.
    embedder: String,
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

/// A logical chunk produced by the chunker, before the `PIC X(512)` split.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub subject: String,
    pub kind: String,
    pub content: String,
}

/// The IDE's chunked store: `~/PowerRustCOBOL/data/chunked.data`.
pub fn ide_chunked_path() -> PathBuf {
    crate::knowledge_store::ide_data_dir().join("chunked.data")
}

/// A project's chunked store: `<project>/data/<name-no-spaces>-chunked.data`.
pub fn project_chunked_path(project_root: &Path) -> PathBuf {
    let name: String = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let name = if name.is_empty() {
        "project".to_string()
    } else {
        name
    };
    project_root.join("data").join(format!("{name}-chunked.data"))
}

fn fnv1a(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn chunk_key(doc_path: &str, ordinal: usize) -> String {
    format!("{doc_path}\u{1}{ordinal:05}")
}

fn doc_of_key(key: &str) -> &str {
    key.split('\u{1}').next().unwrap_or(key)
}

/// Classify a `###` section heading into a record kind.
fn section_kind(heading: &str) -> &'static str {
    let lower = heading.to_ascii_lowercase();
    if lower.contains("propert") {
        "property"
    } else if lower.contains("event") {
        "event"
    } else if lower.contains("method") {
        "method"
    } else {
        "section"
    }
}

/// Convert one Markdown document into logical chunks: one per control,
/// property, method, event, or prose section.
///
/// Recognised structure (the shape the System KB publisher emits):
/// - `## Control: X` opens a control chunk;
/// - `### Settable Properties` / `### Supported Events` / `### Methods`
///   under a control turn each `- `item`` bullet into its OWN record, with
///   self-describing content ("Control X — settable property: `Y`") so the
///   record's tokens carry their context;
/// - every other heading section becomes one prose chunk;
/// - text before the first heading becomes a `document` chunk.
pub fn chunk_markdown(doc_path: &str, text: &str) -> Vec<Chunk> {
    let doc_name = Path::new(doc_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(doc_path)
        .to_string();
    let mut chunks: Vec<Chunk> = Vec::new();
    // The chunk being accumulated: (subject, kind, lines).
    let mut current: Option<(String, String, Vec<String>)> = None;
    // The enclosing `## Control: X` name, if any.
    let mut control: Option<String> = None;
    // The active `###` section kind while under a control.
    let mut list_kind: Option<&'static str> = None;

    let flush = |current: &mut Option<(String, String, Vec<String>)>,
                     chunks: &mut Vec<Chunk>| {
        if let Some((subject, kind, lines)) = current.take() {
            let content = lines.join("\n").trim().to_string();
            if !content.is_empty() {
                chunks.push(Chunk {
                    subject,
                    kind,
                    content,
                });
            }
        }
    };

    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some(heading) = trimmed.strip_prefix("## ").map(str::trim) {
            flush(&mut current, &mut chunks);
            list_kind = None;
            if let Some(name) = heading.strip_prefix("Control:").map(str::trim) {
                control = Some(name.to_string());
                current = Some((
                    name.to_string(),
                    "control".to_string(),
                    vec![format!("Control {name}.")],
                ));
            } else {
                control = None;
                current = Some((
                    format!("{doc_name} › {heading}"),
                    "section".to_string(),
                    vec![line.to_string()],
                ));
            }
        } else if let Some(heading) = trimmed.strip_prefix("### ").map(str::trim) {
            flush(&mut current, &mut chunks);
            let kind = section_kind(heading);
            if control.is_some() && kind != "section" {
                // Bullets of this section become individual records.
                list_kind = Some(kind);
                current = None;
            } else {
                list_kind = None;
                let parent = control
                    .clone()
                    .unwrap_or_else(|| doc_name.clone());
                current = Some((
                    format!("{parent} › {heading}"),
                    kind.to_string(),
                    vec![line.to_string()],
                ));
            }
        } else if let Some(heading) = trimmed.strip_prefix("# ").map(str::trim) {
            flush(&mut current, &mut chunks);
            list_kind = None;
            control = None;
            current = Some((
                heading.to_string(),
                "document".to_string(),
                Vec::new(),
            ));
        } else if let (Some(kind), Some(owner)) = (list_kind, control.as_ref()) {
            // A bullet inside a Properties/Events/Methods section of a control.
            let item = trimmed
                .strip_prefix("- ")
                .or_else(|| trimmed.strip_prefix("* "))
                .map(|rest| rest.trim().trim_matches('`').trim());
            match item {
                Some(name) if !name.is_empty() && !name.eq_ignore_ascii_case("none") => {
                    let article = match kind {
                        "property" => "settable property",
                        "event" => "supported event",
                        "method" => "method",
                        _ => "entry",
                    };
                    chunks.push(Chunk {
                        subject: format!("{owner} · {name}"),
                        kind: kind.to_string(),
                        content: format!(
                            "Control {owner} — {article}: `{name}`.",
                        ),
                    });
                }
                Some(_) => {} // "- None" — nothing to record
                None => {}    // separator/prose inside the list section
            }
        } else if trimmed == "---" {
            // Horizontal rule: a document separator, never content.
        } else if let Some((_, _, lines)) = current.as_mut() {
            lines.push(line.to_string());
        } else if !trimmed.is_empty() {
            // Prose before any heading.
            current = Some((doc_name.clone(), "document".to_string(), vec![line.to_string()]));
        }
    }
    flush(&mut current, &mut chunks);
    chunks
}

/// Split a logical chunk's content into `PIC X(512)` parts, preferring line
/// boundaries. Every part fits [`CONTENT_PIC_CHARS`].
fn split_content(content: &str) -> Vec<String> {
    if content.chars().count() <= CONTENT_PIC_CHARS {
        return vec![content.to_string()];
    }
    let mut parts = Vec::new();
    let mut part = String::new();
    let mut part_chars = 0_usize;
    for line in content.split_inclusive('\n') {
        let mut rest = line;
        while !rest.is_empty() {
            let rest_chars = rest.chars().count();
            let room = CONTENT_PIC_CHARS - part_chars;
            if rest_chars <= room {
                part.push_str(rest);
                part_chars += rest_chars;
                rest = "";
            } else if part_chars > 0 {
                parts.push(std::mem::take(&mut part));
                part_chars = 0;
            } else {
                // A single line longer than the field: hard split on a
                // character boundary.
                let byte = rest
                    .char_indices()
                    .nth(CONTENT_PIC_CHARS)
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

fn open(db_path: &Path) -> Result<Database, String> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Database::create(db_path).map_err(|e| e.to_string())
}

/// Remove every chunk record belonging to `doc_path`.
fn remove_doc_chunks(
    chunks: &mut redb::Table<&str, &[u8]>,
    doc_path: &str,
) -> Result<(), String> {
    let start = format!("{doc_path}\u{1}");
    let end = format!("{doc_path}\u{2}");
    let stale: Vec<String> = chunks
        .range(start.as_str()..end.as_str())
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok().map(|(key, _)| key.value().to_string()))
        .collect();
    for key in stale {
        chunks.remove(key.as_str()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Sync `documents` (path → full text) into the chunked store at `db_path`.
/// See [`sync_documents_with_progress`]; this variant reports no progress.
pub fn sync_documents(db_path: &Path, documents: &[(String, String)]) -> Result<usize, String> {
    sync_documents_with_progress(db_path, documents, &mut |_, _, _| {})
}

/// Sync `documents` (path → full text) into the chunked store at `db_path`,
/// reporting `(records_done, records_total, current_subject)` as each record
/// is embedded — the IDE renders this as a progress bar.
///
/// New or changed documents have their existing records and embeddings
/// removed, then are chunked and embedded again; documents absent from
/// `documents` lose their records; unchanged documents are skipped. A machine
/// running the hashing fallback never downgrades an unchanged document whose
/// records carry the shipped semantic stamp — a cloned IDE keeps its prebuilt
/// embeddings until a document actually changes, and search falls back to
/// lexical scoring over those records meanwhile. Returns the number of live
/// chunk records.
pub fn sync_documents_with_progress(
    db_path: &Path,
    documents: &[(String, String)],
    progress: &mut dyn FnMut(usize, usize, &str),
) -> Result<usize, String> {
    let database = open(db_path)?;
    let (embedder, kind) = project_knowledge::active_embedder();
    let stamp = kind.as_str();
    let write = database.begin_write().map_err(|e| e.to_string())?;
    {
        let mut chunks = write.open_table(CHUNKS).map_err(|e| e.to_string())?;
        let mut docs = write.open_table(DOCS).map_err(|e| e.to_string())?;
        // Documents deleted on disk lose their records.
        let known: Vec<String> = docs
            .iter()
            .map_err(|e| e.to_string())?
            .filter_map(|entry| entry.ok().map(|(key, _)| key.value().to_string()))
            .collect();
        for stale in known
            .iter()
            .filter(|path| !documents.iter().any(|(p, _)| p == *path))
        {
            remove_doc_chunks(&mut chunks, stale)?;
            docs.remove(stale.as_str()).map_err(|e| e.to_string())?;
        }
        // Pass 1 — decide what needs (re)indexing and chunk it, so the
        // progress bar knows its total before the first embedding runs.
        struct Pending<'a> {
            doc_path: &'a str,
            hash: u64,
            /// Each logical chunk with its `PIC X(512)` parts — the parts of
            /// ONE chunk form a parent-linked chain; chunks are independent.
            chunks: Vec<(Chunk, Vec<String>)>,
        }
        let mut pending: Vec<Pending> = Vec::new();
        for (doc_path, text) in documents {
            let hash = fnv1a(text) ^ fnv1a(stamp);
            // Unchanged under the ACTIVE embedder — or unchanged under the
            // shipped SEMANTIC stamp, which hashing must preserve, never
            // replace (the whole point of shipping a prebuilt store).
            let semantic_hash = fnv1a(text) ^ fnv1a(SEMANTIC_STAMP);
            let unchanged = docs
                .get(doc_path.as_str())
                .map_err(|e| e.to_string())?
                .map(|stored| {
                    stored.value() == hash
                        || (kind == EmbedderKind::Hashing && stored.value() == semantic_hash)
                })
                .unwrap_or(false);
            if unchanged {
                continue;
            }
            let doc_chunks: Vec<(Chunk, Vec<String>)> = chunk_markdown(doc_path, text)
                .into_iter()
                .map(|chunk| {
                    let parts = split_content(&chunk.content);
                    (chunk, parts)
                })
                .collect();
            pending.push(Pending {
                doc_path,
                hash,
                chunks: doc_chunks,
            });
        }
        // Pass 2 — embed and store, one progress tick per record.
        let total: usize = pending
            .iter()
            .map(|p| p.chunks.iter().map(|(_, parts)| parts.len()).sum::<usize>())
            .sum();
        let mut done = 0_usize;
        for entry in pending {
            // Preserve the file; replace its records and embeddings.
            remove_doc_chunks(&mut chunks, entry.doc_path)?;
            let mut ordinal = 0_usize;
            for (chunk, parts) in entry.chunks {
                // The chain is scoped to ONE chunk: its first record has no
                // parent, each continuation links to the previous record.
                // Chaining across chunks would fuse the whole document back
                // into one retrievable blob — the exact failure chunking
                // exists to prevent.
                let mut previous: Option<String> = None;
                for part in parts {
                    progress(done, total, &chunk.subject);
                    let key = chunk_key(entry.doc_path, ordinal);
                    let record = ChunkRecord {
                        subject: chunk.subject.clone(),
                        kind: chunk.kind.clone(),
                        embedding: embedder.embed(&part),
                        content: part,
                        parent: previous.clone(),
                        dimensions: VECTOR_DIMENSIONS as u32,
                        embedder: stamp.to_string(),
                    };
                    let bytes = bincode::serialize(&record).map_err(|e| e.to_string())?;
                    chunks
                        .insert(key.as_str(), bytes.as_slice())
                        .map_err(|e| e.to_string())?;
                    previous = Some(key);
                    ordinal += 1;
                    done += 1;
                }
            }
            docs.insert(entry.doc_path, entry.hash)
                .map_err(|e| e.to_string())?;
        }
        if total > 0 {
            progress(total, total, "");
        }
    }
    write.commit().map_err(|e| e.to_string())?;
    // Count live records.
    let read = database.begin_read().map_err(|e| e.to_string())?;
    let chunks = read.open_table(CHUNKS).map_err(|e| e.to_string())?;
    Ok(chunks.len().map_err(|e| e.to_string())? as usize)
}

/// Documents whose stored hash does not match `content ^ stamp` — i.e. what a
/// sync under that embedder would (re)index. The shipped-store freshness test
/// asserts this is empty for the published documents under the semantic stamp.
pub fn stale_documents(
    db_path: &Path,
    documents: &[(String, String)],
    stamp: &str,
) -> Result<Vec<String>, String> {
    let database = open(db_path)?;
    let read = database.begin_read().map_err(|e| e.to_string())?;
    let docs = match read.open_table(DOCS) {
        Ok(table) => table,
        Err(redb::TableError::TableDoesNotExist(_)) => {
            return Ok(documents.iter().map(|(p, _)| p.clone()).collect())
        }
        Err(e) => return Err(e.to_string()),
    };
    let mut stale = Vec::new();
    for (doc_path, text) in documents {
        let expected = fnv1a(text) ^ fnv1a(stamp);
        let current = docs
            .get(doc_path.as_str())
            .map_err(|e| e.to_string())?
            .map(|stored| stored.value() == expected)
            .unwrap_or(false);
        if !current {
            stale.push(doc_path.clone());
        }
    }
    Ok(stale)
}

/// Install the PREBUILT store shipped inside the IDE binary at `db_path`,
/// unless that exact revision is already installed. A sidecar `.rev` file
/// remembers which shipped bytes were installed, so a new IDE build replaces
/// the store once and an unchanged build never touches it (local re-syncs are
/// preserved between IDE updates). Returns whether an install happened.
pub fn install_prebuilt(db_path: &Path, bytes: &[u8]) -> Result<bool, String> {
    if bytes.is_empty() {
        return Ok(false);
    }
    let rev_path = db_path.with_extension("data.rev");
    let rev = format!("{:016x}", fnv1a_bytes(bytes));
    let installed = std::fs::read_to_string(&rev_path)
        .map(|current| current.trim() == rev)
        .unwrap_or(false);
    if installed && db_path.exists() {
        return Ok(false);
    }
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(db_path, bytes).map_err(|e| e.to_string())?;
    std::fs::write(&rev_path, rev).map_err(|e| e.to_string())?;
    Ok(true)
}

fn fnv1a_bytes(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// The textual documents under `<root>/Knowledge Base/`, as (relative path,
/// content) pairs — the sync input and the freshness-check input.
pub fn tree_documents(root: &Path) -> Result<Vec<(String, String)>, String> {
    let mut documents = Vec::new();
    for rel in project_knowledge::documentation_paths(root)? {
        let lower = rel.to_ascii_lowercase();
        if !(lower.ends_with(".md") || lower.ends_with(".markdown") || lower.ends_with(".txt")) {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(root.join(&rel)) {
            documents.push((rel, text));
        }
    }
    Ok(documents)
}

/// Sync every textual document under `<root>/Knowledge Base/` into the
/// chunked store at `db_path`. Returns the number of live chunk records.
pub fn sync_tree(root: &Path, db_path: &Path) -> Result<usize, String> {
    sync_tree_with_progress(root, db_path, &mut |_, _, _| {})
}

/// [`sync_tree`] with the per-record progress callback of
/// [`sync_documents_with_progress`].
pub fn sync_tree_with_progress(
    root: &Path,
    db_path: &Path,
    progress: &mut dyn FnMut(usize, usize, &str),
) -> Result<usize, String> {
    let documents = tree_documents(root)?;
    sync_documents_with_progress(db_path, &documents, progress)
}

/// The `limit` records closest to `query`, each with its overflow chain
/// reassembled so the hit carries the subject's complete text — and nothing
/// else.
pub fn search(db_path: &Path, query: &str, limit: usize) -> Result<Vec<ChunkHit>, String> {
    if query.trim().is_empty() || limit == 0 || !db_path.exists() {
        return Ok(Vec::new());
    }
    let database = open(db_path)?;
    let (embedder, kind) = project_knowledge::active_embedder();
    let query_vector = embedder.embed_query(query);
    let read = database.begin_read().map_err(|e| e.to_string())?;
    let chunks = match read.open_table(CHUNKS) {
        Ok(table) => table,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    // Score every record; keep the chain topology for reassembly.
    struct Scored {
        key: String,
        record: ChunkRecord,
        score: f32,
    }
    let mut all: Vec<Scored> = Vec::new();
    for entry in chunks.iter().map_err(|e| e.to_string())? {
        let (key, value) = entry.map_err(|e| e.to_string())?;
        let Ok(record) = bincode::deserialize::<ChunkRecord>(value.value()) else {
            continue;
        };
        if record.embedding.len() != VECTOR_DIMENSIONS {
            continue;
        }
        let score = if record.embedder == kind.as_str() {
            query_vector
                .iter()
                .zip(record.embedding.iter())
                .map(|(left, right)| left * right)
                .sum::<f32>()
        } else if kind == EmbedderKind::Hashing {
            // A shipped SEMANTIC record searched by a machine that only has
            // the hashing fallback: score the record's TEXT lexically instead
            // of skipping it, so the prebuilt store stays useful (and intact)
            // until the model arrives.
            let lexical = HashingEmbedder.embed(&record.content);
            query_vector
                .iter()
                .zip(lexical.iter())
                .map(|(left, right)| left * right)
                .sum::<f32>()
        } else {
            // A hashing record under a semantic embedder is stale by
            // definition — the next sync re-embeds it; scoring it now would
            // compare incomparable spaces.
            continue;
        };
        all.push(Scored {
            key: key.value().to_string(),
            record,
            score,
        });
    }
    // Best record per chain HEAD: a hit anywhere in a chain surfaces the whole
    // chain once, scored by its best member.
    let head_of = |scored: &Scored, all: &[Scored]| -> String {
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
    let mut best: std::collections::BTreeMap<String, f32> = std::collections::BTreeMap::new();
    for scored in &all {
        if scored.score <= 0.0 {
            continue;
        }
        let head = head_of(scored, &all);
        let entry = best.entry(head).or_insert(scored.score);
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
        // Reassemble: the head, then every record whose parent chain reaches it.
        let mut content = head.record.content.clone();
        let mut tail_key = head_key.clone();
        loop {
            match all
                .iter()
                .find(|s| s.record.parent.as_deref() == Some(tail_key.as_str()))
            {
                Some(next) => {
                    content.push_str(&next.record.content);
                    tail_key = next.key.clone();
                }
                None => break,
            }
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

/// Diagnostic view of every stored record: (key, subject, embedder stamp,
/// vector length, vector norm). For debugging only.
pub fn debug_records(db_path: &Path) -> Result<Vec<(String, String, String, usize, f32)>, String> {
    let database = open(db_path)?;
    let read = database.begin_read().map_err(|e| e.to_string())?;
    let chunks = match read.open_table(CHUNKS) {
        Ok(table) => table,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut out = Vec::new();
    for entry in chunks.iter().map_err(|e| e.to_string())? {
        let (key, value) = entry.map_err(|e| e.to_string())?;
        match bincode::deserialize::<ChunkRecord>(value.value()) {
            Ok(record) => out.push((
                key.value().to_string(),
                record.subject,
                record.embedder,
                record.embedding.len(),
                record.embedding.iter().map(|v| v * v).sum::<f32>().sqrt(),
            )),
            Err(e) => out.push((key.value().to_string(), format!("DECODE ERR: {e}"), String::new(), 0, 0.0)),
        }
    }
    Ok(out)
}

/// Total characters across every record — the corpus a push-everything
/// approach would inject. Empty/absent stores are 0, not failures.
pub fn corpus_chars(db_path: &Path) -> Result<u64, String> {
    if !db_path.exists() {
        return Ok(0);
    }
    let database = open(db_path)?;
    let read = database.begin_read().map_err(|e| e.to_string())?;
    let chunks = match read.open_table(CHUNKS) {
        Ok(table) => table,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(0),
        Err(e) => return Err(e.to_string()),
    };
    let mut total = 0_u64;
    for entry in chunks.iter().map_err(|e| e.to_string())? {
        let (_key, value) = entry.map_err(|e| e.to_string())?;
        if let Ok(record) = bincode::deserialize::<ChunkRecord>(value.value()) {
            total += record.content.chars().count() as u64;
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "prc-chunked-{tag}-{}-{}.data",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    const CATALOGUE: &str = "# Controls Reference\n\nAll controls.\n\n\
## Control: DataGrid\n\n### Settable Properties\n- `AllowColumnResize`\n- `FrozenColumns`\n\n\
### Supported Events\n- `onRowSelect`\n\n---\n\n\
## Control: Timer\n\n### Settable Properties\n- `Interval`\n\n### Supported Events\n- `onTick`\n";

    /// The catalogue explodes into one record per control, property, and
    /// event — each self-describing.
    #[test]
    fn a_catalogue_document_yields_one_record_per_subject() {
        let chunks = chunk_markdown("Knowledge Base/form_designer_controls.md", CATALOGUE);
        let subjects: Vec<(&str, &str)> = chunks
            .iter()
            .map(|c| (c.subject.as_str(), c.kind.as_str()))
            .collect();
        assert!(subjects.contains(&("DataGrid", "control")));
        assert!(subjects.contains(&("DataGrid · AllowColumnResize", "property")));
        assert!(subjects.contains(&("DataGrid · FrozenColumns", "property")));
        assert!(subjects.contains(&("DataGrid · onRowSelect", "event")));
        assert!(subjects.contains(&("Timer · Interval", "property")));
        assert!(subjects.contains(&("Timer · onTick", "event")));
        let ontick = chunks
            .iter()
            .find(|c| c.subject == "Timer · onTick")
            .unwrap();
        assert_eq!(ontick.content, "Control Timer — supported event: `onTick`.");
        println!(
            "catalogue chunking: {} records from 1 document — {:?}",
            chunks.len(),
            subjects
        );
    }

    /// PIC X(512): oversized content continues in records linked to the
    /// previous one, and search reassembles the chain.
    #[test]
    fn oversized_content_chains_records_and_search_reassembles() {
        let long_line = "COBOL keeps the ledger honest. ".repeat(60); // ~1860 chars
        let doc = format!("## Long Section\n\n{long_line}\n");
        let path = store("chain");
        let live = sync_documents(&path, &[("Knowledge Base/long.md".into(), doc)]).unwrap();
        assert!(
            live >= 4,
            "~1.9k chars must span at least 4 PIC X(512) records (got {live})"
        );
        let hits = search(&path, "ledger honest COBOL", 3).unwrap();
        assert_eq!(hits.len(), 1, "one chain, one hit");
        assert!(
            hits[0].content.chars().count() > CONTENT_PIC_CHARS,
            "the hit carries the reassembled chain, not one 512-char part"
        );
        assert!(hits[0].content.matches("ledger honest").count() >= 50);
        let _ = std::fs::remove_file(&path);
        println!(
            "chain: {live} records; reassembled hit is {} chars",
            hits[0].content.chars().count()
        );
    }

    /// Updated documents replace their records; deleted documents leave the
    /// index; unchanged documents are skipped (same record count, same store).
    #[test]
    fn updates_replace_records_and_deletions_remove_them() {
        let path = store("resync");
        let v1 = ("Knowledge Base/a.md".to_string(), CATALOGUE.to_string());
        let other = (
            "Knowledge Base/b.md".to_string(),
            "## Notes\n\nProject notes.\n".to_string(),
        );
        let live1 = sync_documents(&path, &[v1.clone(), other.clone()]).unwrap();
        // Unchanged re-sync: identical count.
        let live2 = sync_documents(&path, &[v1, other.clone()]).unwrap();
        assert_eq!(live1, live2, "unchanged documents must be skipped");
        // Update: DataGrid loses an event; the stale record must vanish.
        let v2 = (
            "Knowledge Base/a.md".to_string(),
            CATALOGUE.replace("- `onRowSelect`\n", ""),
        );
        sync_documents(&path, &[v2, other.clone()]).unwrap();
        let hits = search(&path, "onRowSelect", 5).unwrap();
        assert!(
            !hits.iter().any(|h| h.subject == "DataGrid · onRowSelect"),
            "records of the old version must be gone"
        );
        // Delete document a entirely: only b's records remain.
        let live4 = sync_documents(&path, &[other]).unwrap();
        let survivors = search(&path, "Project notes", 5).unwrap();
        assert!(survivors.iter().all(|h| h.source_path == "Knowledge Base/b.md"));
        let _ = std::fs::remove_file(&path);
        println!(
            "resync: {live1} → {live2} (unchanged) → {live4} records after deletion"
        );
    }

    /// Retrieval answers a subject question with THAT subject's records only.
    #[test]
    fn search_returns_the_asked_subject_not_the_catalogue() {
        let path = store("select");
        sync_documents(
            &path,
            &[("Knowledge Base/controls.md".into(), CATALOGUE.into())],
        )
        .unwrap();
        let hits = search(&path, "DataGrid onRowSelect event", 3).unwrap();
        assert!(!hits.is_empty());
        assert!(
            hits[0].subject.starts_with("DataGrid"),
            "top hit should be a DataGrid record, got {}",
            hits[0].subject
        );
        assert_eq!(
            hits.len(),
            3,
            "independent subjects must surface as separate hits, not fuse into one chain"
        );
        for hit in &hits {
            assert!(
                hit.content.chars().count() <= CONTENT_PIC_CHARS,
                "single-record subjects stay within PIC X(512)"
            );
        }
        let total = corpus_chars(&path).unwrap();
        let injected: u64 = hits
            .iter()
            .map(|h| h.content.chars().count() as u64)
            .sum();
        assert!(injected < total, "retrieval must inject less than the corpus");
        let _ = std::fs::remove_file(&path);
        println!(
            "selectivity: injected {injected} of {total} corpus chars across {} hits",
            hits.len()
        );
    }

    /// The two store locations follow the spec exactly.
    #[test]
    fn store_paths_follow_the_naming_contract() {
        assert!(ide_chunked_path().ends_with("PowerRustCOBOL/data/chunked.data"));
        let project = project_chunked_path(Path::new("/tmp/My Payroll App"));
        assert_eq!(
            project,
            Path::new("/tmp/My Payroll App/data/MyPayrollApp-chunked.data"),
            "project store strips spaces from the project name"
        );
    }
}
