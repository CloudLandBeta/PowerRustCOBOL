// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Keeping a collection's index in agreement with its documents (spec 068
//! §4.4).
//!
//! A refresh compares the documents folder with the index **by content** — a
//! hash and a length per document — and indexes only what was added, changed
//! or removed (R17). It therefore also notices what changed outside the
//! application: in the file manager, or by another user on the LAN (R18). No
//! file watcher is involved (R19).
//!
//! Documents are embedded outside the write transaction and committed in
//! batches of [`BATCH`], so the write lock other processes wait on is held
//! only briefly.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use redb::{ReadableTable, WriteTransaction};

use crate::chunk::{fnv1a, sections, split_content};
use crate::convert::{Converters, Skip, PATH_SEPARATOR};
use crate::embed::Embedder;
use crate::store::{store_err, Collection, KbError, Passage, Source, META, PASSAGES, SOURCES};

/// Documents committed per write transaction.
pub const BATCH: usize = 16;

/// Where an update stands, reported after each document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    /// The document just handled, relative to the documents folder.
    pub document: String,
    pub current: usize,
    pub total: usize,
}

/// What an update did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    /// Documents not indexed, each with why.
    pub skipped: Vec<(String, Skip)>,
    pub passages: usize,
    /// Stored without vectors, because this application's embedder differs from
    /// the collection's (R26a).
    pub text_only: bool,
    /// Why some documents were stored without vectors although the embedder
    /// matches: it could not be used (an unreachable server, a model not yet
    /// fetched). They are found lexically meanwhile, and the next refresh with
    /// a working embedder gives them vectors.
    pub embedder_error: Option<String>,
}

/// What to bring into agreement.
pub enum Scope<'a> {
    /// The whole documents folder.
    All,
    /// Only these documents (paths relative to the documents folder).
    Only(&'a [String]),
}

/// Bring the index into agreement with the documents folder, or with the
/// documents named in `scope`.
pub fn refresh(
    collection: &Collection,
    embedder: &dyn Embedder,
    converters: &Converters,
    scope: Scope<'_>,
    progress: &mut dyn FnMut(&Progress),
    cancel: &AtomicBool,
) -> Result<Outcome, KbError> {
    let docs_dir = collection.documents_dir();
    let on_disk: BTreeMap<String, PathBuf> = match scope {
        Scope::All => walk(&docs_dir)?,
        Scope::Only(names) => names
            .iter()
            .map(|n| (n.clone(), docs_dir.join(n)))
            .filter(|(_, p)| p.is_file())
            .collect(),
    };
    let known = collection.sources()?;
    let collection_stamp = collection.embedder_stamp()?;
    let ours = embedder.stamp();
    let text_only = collection_stamp.as_deref().is_some_and(|s| s != ours);

    // What to remove: known documents gone from disk (within the scope).
    let removed: Vec<String> = match scope {
        Scope::All => known.keys().filter(|k| !on_disk.contains_key(*k)).cloned().collect(),
        Scope::Only(names) => names
            .iter()
            .filter(|n| known.contains_key(*n) && !on_disk.contains_key(*n))
            .cloned()
            .collect(),
    };

    // What to index: new or changed, or indexed text-only and now embeddable.
    let mut work: Vec<(String, PathBuf, Vec<u8>)> = Vec::new();
    let mut outcome = Outcome {
        text_only,
        ..Outcome::default()
    };
    for (name, path) in &on_disk {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                outcome
                    .skipped
                    .push((name.clone(), Skip::new("unreadable", format!("could not be read: {e}"))));
                continue;
            }
        };
        let unchanged = known.get(name).is_some_and(|s| {
            s.hash == fnv1a(&bytes)
                && s.len == bytes.len() as u64
                && (text_only || s.stamp == ours)
        });
        if !unchanged {
            work.push((name.clone(), path.clone(), bytes));
        }
    }

    let total = work.len() + removed.len();
    let mut current = 0;
    let mut stamp_recorded = collection_stamp.is_some();

    for batch in work.chunks(BATCH) {
        let mut prepared: Vec<(String, Source, Vec<Passage>, bool)> = Vec::new();
        for (name, path, bytes) in batch {
            if cancel.load(Ordering::Relaxed) {
                return Err(KbError::Cancelled);
            }
            current += 1;
            match prepare(name, path, bytes, embedder, converters, text_only) {
                Ok((source, passages, embed_error, member_skips)) => {
                    // Documents inside an archive that could not be read,
                    // named through it (spec 074 R10).
                    outcome.skipped.extend(
                        member_skips
                            .into_iter()
                            .map(|(m, s)| (format!("{name}{PATH_SEPARATOR}{m}"), s)),
                    );
                    if outcome.embedder_error.is_none() {
                        outcome.embedder_error = embed_error;
                    }
                    prepared.push((name.clone(), source, passages, known.contains_key(name)));
                }
                Err(skip) => outcome.skipped.push((name.clone(), skip)),
            }
            progress(&Progress {
                document: name.clone(),
                current,
                total,
            });
        }
        let record_stamp = !stamp_recorded
            && !text_only
            && prepared.iter().any(|(_, source, _, _)| !source.stamp.is_empty());
        collection.write(|tx| {
            for (name, source, passages, _) in &prepared {
                put_document(tx, name, source, passages)?;
            }
            if record_stamp {
                let mut meta = tx.open_table(META).map_err(store_err)?;
                meta.insert("embedder", ours.as_bytes()).map_err(store_err)?;
            }
            Ok(())
        })?;
        stamp_recorded |= record_stamp;
        for (_, source, _, existed) in &prepared {
            outcome.passages += source.passages as usize;
            if *existed {
                outcome.updated += 1;
            } else {
                outcome.added += 1;
            }
        }
    }

    for batch in removed.chunks(BATCH) {
        if cancel.load(Ordering::Relaxed) {
            return Err(KbError::Cancelled);
        }
        collection.write(|tx| {
            for name in batch {
                remove_document(tx, name)?;
            }
            Ok(())
        })?;
        for name in batch {
            current += 1;
            outcome.removed += 1;
            progress(&Progress {
                document: name.clone(),
                current,
                total,
            });
        }
    }
    Ok(outcome)
}

/// Drop every passage and source, and the collection's embedder stamp, then
/// index everything again with `embedder` — how a collection changes embedder
/// (spec 068 R26; only ever on request).
pub fn reindex(
    collection: &Collection,
    embedder: &dyn Embedder,
    converters: &Converters,
    progress: &mut dyn FnMut(&Progress),
    cancel: &AtomicBool,
) -> Result<Outcome, KbError> {
    collection.write(|tx| {
        tx.delete_table(PASSAGES).map_err(store_err)?;
        tx.delete_table(SOURCES).map_err(store_err)?;
        tx.open_table(PASSAGES).map_err(store_err)?;
        tx.open_table(SOURCES).map_err(store_err)?;
        let mut meta = tx.open_table(META).map_err(store_err)?;
        meta.remove("embedder").map_err(store_err)?;
        Ok(())
    })?;
    refresh(collection, embedder, converters, Scope::All, progress, cancel)
}

/// Save a document into the collection's folder (creating its sub-folders) and
/// index it — `AddDocument` and `UpdateDocument` (R16).
pub fn put(
    collection: &Collection,
    name: &str,
    bytes: &[u8],
    embedder: &dyn Embedder,
    converters: &Converters,
    progress: &mut dyn FnMut(&Progress),
    cancel: &AtomicBool,
) -> Result<Outcome, KbError> {
    let path = document_path(collection, name)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| KbError::Io(format!("could not create {}: {e}", parent.display())))?;
    }
    std::fs::write(&path, bytes)
        .map_err(|e| KbError::Io(format!("could not write {}: {e}", path.display())))?;
    let only = [normalise(name)];
    refresh(collection, embedder, converters, Scope::Only(&only), progress, cancel)
}

/// Delete a document from the collection's folder and from the index —
/// `DeleteDocument` (R16). Only ever on the program's explicit request.
pub fn delete(
    collection: &Collection,
    name: &str,
    embedder: &dyn Embedder,
    converters: &Converters,
    progress: &mut dyn FnMut(&Progress),
    cancel: &AtomicBool,
) -> Result<Outcome, KbError> {
    let path = document_path(collection, name)?;
    if path.is_file() {
        std::fs::remove_file(&path)
            .map_err(|e| KbError::Io(format!("could not delete {}: {e}", path.display())))?;
    }
    let only = [normalise(name)];
    refresh(collection, embedder, converters, Scope::Only(&only), progress, cancel)
}

/// The documents the collection holds on disk, relative and `/`-separated.
pub fn list_documents(collection: &Collection) -> Result<Vec<String>, KbError> {
    Ok(walk(&collection.documents_dir())?.into_keys().collect())
}

/// A document's path inside the documents folder, refusing any name that
/// would leave it.
fn document_path(collection: &Collection, name: &str) -> Result<PathBuf, KbError> {
    let rel = normalise(name);
    let escapes = rel.is_empty()
        || rel.split('/').any(|seg| seg.is_empty() || seg == "." || seg == "..")
        || Path::new(name).is_absolute();
    if escapes {
        return Err(KbError::InvalidName(name.to_string()));
    }
    Ok(collection.documents_dir().join(rel))
}

fn normalise(name: &str) -> String {
    name.trim().replace('\\', "/").trim_start_matches('/').to_string()
}

/// Every file under `dir`, by `/`-separated relative path. Hidden entries and
/// partial downloads are left out.
fn walk(dir: &Path) -> Result<BTreeMap<String, PathBuf>, KbError> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries = std::fs::read_dir(&d)
            .map_err(|e| KbError::Io(format!("could not list {}: {e}", d.display())))?;
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry.file_name();
            let n = file_name.to_string_lossy();
            if n.starts_with('.') || n.ends_with(".part") {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(rel) = path.strip_prefix(dir) {
                let key = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                out.insert(key, path);
            }
        }
    }
    Ok(out)
}

/// What [`prepare`] makes of one document.
type Prepared = (Source, Vec<Passage>, Option<String>, Vec<(String, Skip)>);

/// Convert, section, split and embed one document — each document inside it,
/// when it is an archive.
fn prepare(
    name: &str,
    path: &Path,
    bytes: &[u8],
    embedder: &dyn Embedder,
    converters: &Converters,
    text_only: bool,
) -> Result<Prepared, Skip> {
    let converted = converters.convert(path, bytes)?;
    let mut passages: Vec<Passage> = Vec::new();
    for doc in converted.parts {
        let section_name = doc.name.as_deref().unwrap_or(name);
        for section in sections(section_name, &doc.markdown) {
            let first = passages.len() as u32;
            for (i, part) in split_content(&section.content).into_iter().enumerate() {
                passages.push(Passage {
                    heading: section.heading.clone(),
                    content: part,
                    chain: (i > 0).then_some(first),
                    vector: Vec::new(),
                    stamp: String::new(),
                    part: doc.name.clone(),
                });
            }
        }
    }
    let mut stamp = String::new();
    let mut embed_error = None;
    if !text_only && !passages.is_empty() {
        let texts: Vec<&str> = passages.iter().map(|p| p.content.as_str()).collect();
        match embedder.embed(&texts) {
            Ok(vectors) => {
                stamp = embedder.stamp();
                for (p, v) in passages.iter_mut().zip(vectors) {
                    p.vector = v;
                    p.stamp = stamp.clone();
                }
            }
            // Stored text-only rather than dropped: still found lexically.
            Err(e) => embed_error = Some(e),
        }
    }
    let source = Source {
        hash: fnv1a(bytes),
        len: bytes.len() as u64,
        passages: passages.len() as u32,
        stamp,
    };
    Ok((source, passages, embed_error, converted.skipped))
}

fn passage_key(name: &str, ordinal: usize) -> String {
    format!("{name}\u{1}{ordinal:05}")
}

fn remove_passages(tx: &WriteTransaction, name: &str) -> Result<(), KbError> {
    let mut table = tx.open_table(PASSAGES).map_err(store_err)?;
    let from = format!("{name}\u{1}");
    let to = format!("{name}\u{2}");
    let keys: Vec<String> = table
        .range(from.as_str()..to.as_str())
        .map_err(store_err)?
        .filter_map(|r| r.ok().map(|(k, _)| k.value().to_string()))
        .collect();
    for k in keys {
        table.remove(k.as_str()).map_err(store_err)?;
    }
    Ok(())
}

fn put_document(
    tx: &WriteTransaction,
    name: &str,
    source: &Source,
    passages: &[Passage],
) -> Result<(), KbError> {
    remove_passages(tx, name)?;
    {
        let mut table = tx.open_table(PASSAGES).map_err(store_err)?;
        for (i, p) in passages.iter().enumerate() {
            let bytes = bincode::serialize(p).map_err(store_err)?;
            table
                .insert(passage_key(name, i).as_str(), bytes.as_slice())
                .map_err(store_err)?;
        }
    }
    let mut sources = tx.open_table(SOURCES).map_err(store_err)?;
    let bytes = bincode::serialize(source).map_err(store_err)?;
    sources.insert(name, bytes.as_slice()).map_err(store_err)?;
    Ok(())
}

fn remove_document(tx: &WriteTransaction, name: &str) -> Result<(), KbError> {
    remove_passages(tx, name)?;
    let mut sources = tx.open_table(SOURCES).map_err(store_err)?;
    sources.remove(name).map_err(store_err)?;
    Ok(())
}

/// Every stored passage of the collection, by key. Used by search.
pub(crate) fn all_passages(collection: &Collection) -> Result<Vec<(String, Passage)>, KbError> {
    use redb::ReadableDatabase;
    let tx = collection.db().begin_read().map_err(store_err)?;
    let table = tx.open_table(PASSAGES).map_err(store_err)?;
    let mut out = Vec::new();
    for row in table.iter().map_err(store_err)? {
        let (k, v) = row.map_err(store_err)?;
        if let Ok(p) = bincode::deserialize::<Passage>(v.value()) {
            out.push((k.value().to_string(), p));
        }
    }
    Ok(out)
}

/// The set of documents with at least one passage — for tests and reports.
pub fn indexed_documents(collection: &Collection) -> Result<BTreeSet<String>, KbError> {
    Ok(collection.sources()?.into_keys().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::HashingEmbedder;
    use std::time::Instant;

    fn quiet() -> impl FnMut(&Progress) {
        |_| {}
    }

    #[test]
    fn a_refresh_indexes_only_what_changed_including_outside_edits() {
        let root = tempfile::tempdir().unwrap();
        let c = Collection::open(root.path(), "hr").unwrap();
        let docs = c.documents_dir();
        for i in 0..20 {
            std::fs::write(docs.join(format!("doc{i:02}.md")), format!("# Doc {i}\nText {i}.")).unwrap();
        }
        let cancel = AtomicBool::new(false);
        let conv = Converters::default();
        let started = Instant::now();
        let first = refresh(&c, &HashingEmbedder, &conv, Scope::All, &mut quiet(), &cancel).unwrap();
        let first_ms = started.elapsed().as_millis();
        assert_eq!((first.added, first.updated, first.removed), (20, 0, 0));

        // Outside the application: one added, one changed, one removed.
        std::fs::write(docs.join("new.md"), "# New\nHello.").unwrap();
        std::fs::write(docs.join("doc03.md"), "# Doc 3\nChanged.").unwrap();
        std::fs::remove_file(docs.join("doc07.md")).unwrap();
        let mut seen = Vec::new();
        let second = refresh(&c, &HashingEmbedder, &conv, Scope::All, &mut |p| seen.push(p.document.clone()), &cancel).unwrap();
        assert_eq!((second.added, second.updated, second.removed), (1, 1, 1));
        seen.sort();
        assert_eq!(seen, vec!["doc03.md", "doc07.md", "new.md"], "only those three touched");

        let third = refresh(&c, &HashingEmbedder, &conv, Scope::All, &mut quiet(), &cancel).unwrap();
        assert_eq!((third.added, third.updated, third.removed), (0, 0, 0), "nothing to do");
        println!(
            "refresh: 20 documents / {} passages in {first_ms} ms; outside edits → 3 touched, 17 left alone",
            first.passages
        );
    }

    #[test]
    fn an_unreadable_document_is_skipped_by_name_and_the_rest_indexed() {
        let root = tempfile::tempdir().unwrap();
        let c = Collection::open(root.path(), "mixed").unwrap();
        let docs = c.documents_dir();
        std::fs::write(docs.join("good.md"), "# Good\nFine.").unwrap();
        std::fs::write(docs.join("binary.md"), [0xff_u8, 0xfe, 0x00, 0x80]).unwrap();
        std::fs::write(docs.join("photo.jpg"), [1_u8, 2, 3]).unwrap();
        let out = refresh(&c, &HashingEmbedder, &Converters::default(), Scope::All, &mut quiet(), &AtomicBool::new(false)).unwrap();
        assert_eq!(out.added, 1);
        let reasons: Vec<(&str, &str)> = out.skipped.iter().map(|(n, s)| (n.as_str(), s.code)).collect();
        assert_eq!(reasons, vec![("binary.md", "damaged"), ("photo.jpg", "unsupported")]);
    }

    #[test]
    fn put_and_delete_update_the_index_and_refuse_escaping_names() {
        let root = tempfile::tempdir().unwrap();
        let c = Collection::open(root.path(), "legal").unwrap();
        let conv = Converters::default();
        let cancel = AtomicBool::new(false);
        let out = put(&c, "contracts/nda.md", b"# NDA\nConfidential.", &HashingEmbedder, &conv, &mut quiet(), &cancel).unwrap();
        assert_eq!(out.added, 1);
        assert!(c.documents_dir().join("contracts/nda.md").is_file());
        let upd = put(&c, "contracts/nda.md", b"# NDA\nRevised.", &HashingEmbedder, &conv, &mut quiet(), &cancel).unwrap();
        assert_eq!(upd.updated, 1);
        let del = delete(&c, "contracts/nda.md", &HashingEmbedder, &conv, &mut quiet(), &cancel).unwrap();
        assert_eq!(del.removed, 1);
        assert!(indexed_documents(&c).unwrap().is_empty());
        for bad in ["../x.md", "/etc/passwd", "a/../../b.md", ""] {
            assert!(put(&c, bad, b"x", &HashingEmbedder, &conv, &mut quiet(), &cancel).is_err(), "{bad:?}");
        }
    }

    /// An application whose embedder differs from the collection's stores new
    /// documents text-only and leaves the collection's embedder alone (R26a).
    #[test]
    fn a_mismatched_embedder_stores_text_only() {
        struct Other;
        impl Embedder for Other {
            fn stamp(&self) -> String {
                "endpoint:ollama:other".into()
            }
            fn embed(&self, t: &[&str]) -> Result<Vec<Vec<f32>>, String> {
                Ok(t.iter().map(|_| vec![1.0, 0.0]).collect())
            }
        }
        let root = tempfile::tempdir().unwrap();
        let c = Collection::open(root.path(), "shared").unwrap();
        let conv = Converters::default();
        let cancel = AtomicBool::new(false);
        put(&c, "a.md", b"# A\nAlpha.", &HashingEmbedder, &conv, &mut quiet(), &cancel).unwrap();
        assert_eq!(c.embedder_stamp().unwrap().as_deref(), Some("hashing"));
        let out = put(&c, "b.md", b"# B\nBeta.", &Other, &conv, &mut quiet(), &cancel).unwrap();
        assert!(out.text_only);
        assert_eq!(c.embedder_stamp().unwrap().as_deref(), Some("hashing"), "not taken over");
        let b = all_passages(&c).unwrap().into_iter().find(|(k, _)| k.starts_with("b.md")).unwrap().1;
        assert!(b.vector.is_empty() && b.stamp.is_empty(), "stored text-only");
        // The matching application's next refresh fills the vector in.
        refresh(&c, &HashingEmbedder, &conv, Scope::All, &mut quiet(), &cancel).unwrap();
        let b = all_passages(&c).unwrap().into_iter().find(|(k, _)| k.starts_with("b.md")).unwrap().1;
        assert_eq!(b.stamp, "hashing");
    }
}

#[cfg(test)]
mod embedder_down {
    use super::*;
    use crate::embed::Embedder;

    struct Down;
    impl Embedder for Down {
        fn stamp(&self) -> String {
            "endpoint:ollama:m".into()
        }
        fn embed(&self, _: &[&str]) -> Result<Vec<Vec<f32>>, String> {
            Err("the embedding server at http://127.0.0.1:9 could not be reached".into())
        }
    }

    /// An embedder that cannot be used stores documents text-only — still
    /// found lexically — and says why; it never drops them.
    #[test]
    fn an_unusable_embedder_stores_text_only_and_says_why() {
        let root = tempfile::tempdir().unwrap();
        let c = Collection::open(root.path(), "hr").unwrap();
        std::fs::write(c.documents_dir().join("a.md"), "# A\nAlpha beta.").unwrap();
        let out = refresh(&c, &Down, &Converters::default(), Scope::All, &mut |_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(out.added, 1);
        assert!(out.skipped.is_empty());
        assert!(out.embedder_error.unwrap().contains("could not be reached"));
        assert_eq!(c.embedder_stamp().unwrap(), None, "no stamp without vectors");
        let hits = crate::search::search(&c, &Down, "alpha", 3).unwrap();
        assert_eq!(hits.hits[0].document, "a.md", "found lexically");
    }
}
