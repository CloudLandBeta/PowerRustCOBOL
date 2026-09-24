// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A collection on disk: its documents folder and its index (spec 068 §4.1–4.3).
//!
//! ```text
//! <location>/<collection>/
//!     documents/                  what the users own
//!     collection.kbindex          the index (redb)
//!     collection.kbindex.lock     taken while writing
//! ```
//!
//! **Shared use.** The index is opened in redb's multi-process mode
//! (`MultiWriter`), so several processes — on one machine or on a LAN share —
//! search and write it at once; redb serialises the writes (R8, R9, R11). redb
//! waits *indefinitely* for another process's write transaction, so the bound
//! R10 asks for comes from the `.lock` file: a writer polls it and gives up with
//! [`KbError::Busy`] instead of hanging. Readers never take it.

use std::collections::BTreeMap;
use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use redb::{
    ConcurrencyMode, Database, DatabaseError, ReadableDatabase, ReadableTable, TableDefinition,
    TableHandle, WriteTransaction,
};
use serde::{Deserialize, Serialize};

/// The index format this build writes. An index recording another version is
/// moved aside and rebuilt from the documents, never migrated (R13).
pub const SCHEMA_VERSION: u32 = 1;

/// The documents folder inside a collection.
pub const DOCUMENTS_DIR: &str = "documents";
/// The index file inside a collection. Deliberately unlike every IDE store
/// (`*-chunked.data`, `project-knowledge.redb`) (R1).
pub const INDEX_FILE: &str = "collection.kbindex";
const LOCK_SUFFIX: &str = ".lock";

/// How long a writer waits for another writer by default.
pub const DEFAULT_WRITE_WAIT: Duration = Duration::from_millis(5000);
const LOCK_POLL: Duration = Duration::from_millis(25);

/// Key → value metadata: `schema` (u32 LE) and `embedder` (the stamp).
pub(crate) const META: TableDefinition<&str, &[u8]> = TableDefinition::new("kb_meta");
/// `"<document>\u{1}<ordinal:05>"` → bincode [`Passage`].
pub(crate) const PASSAGES: TableDefinition<&str, &[u8]> = TableDefinition::new("kb_passages");
/// Document path (relative to `documents/`, `/`-separated) → bincode [`Source`].
pub(crate) const SOURCES: TableDefinition<&str, &[u8]> = TableDefinition::new("kb_sources");

/// Table names the IDE's own Knowledge Bases use. A file holding any of them
/// is Grace's, and is refused rather than read or merged (R2).
const FOREIGN_TABLES: [&str; 3] = ["chunks", "documents", "project_documents"];

/// One stored passage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Passage {
    /// Where it sits in its document: `"handbook › Leave › Carry-over"`.
    pub heading: String,
    pub content: String,
    /// For the second and later parts of a split section: the ordinal of the
    /// first part, so a search can return the whole section.
    pub chain: Option<u32>,
    /// Empty when the passage was stored **text-only** by an application whose
    /// embedder differs from the collection's (R26a).
    pub vector: Vec<f32>,
    /// The embedder that made `vector` (empty when there is none).
    pub stamp: String,
}

/// What the index knows about one document, to tell whether it changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub hash: u64,
    pub len: u64,
    pub passages: u32,
    /// The embedder its vectors came from; empty when stored text-only.
    pub stamp: String,
}

/// Why a Knowledge Base operation did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KbError {
    /// The file is not an application Knowledge Base index — typically one of
    /// the IDE's own stores (R2).
    NotAKnowledgeBase(PathBuf),
    /// Another process held the write lock for longer than the wait allowed.
    Busy(Duration),
    /// A collection name that could escape its location or is empty.
    InvalidName(String),
    /// A file or folder could not be read or written.
    Io(String),
    /// The index itself failed.
    Store(String),
    /// The operation was cancelled.
    Cancelled,
    /// The embedder could not produce vectors.
    Embedder(String),
}

impl std::fmt::Display for KbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KbError::NotAKnowledgeBase(p) => write!(
                f,
                "{} is not an application Knowledge Base (it may be one of the IDE's own stores)",
                p.display()
            ),
            KbError::Busy(d) => write!(
                f,
                "the Knowledge Base is busy: another application has been writing it for more than {} ms",
                d.as_millis()
            ),
            KbError::InvalidName(n) => write!(f, "\"{n}\" is not a valid collection name"),
            KbError::Io(e) => write!(f, "{e}"),
            KbError::Store(e) => write!(f, "the Knowledge Base index failed: {e}"),
            KbError::Cancelled => write!(f, "cancelled"),
            KbError::Embedder(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for KbError {}

pub(crate) fn store_err(e: impl std::fmt::Display) -> KbError {
    KbError::Store(e.to_string())
}

fn io_err(what: &str, path: &Path, e: std::io::Error) -> KbError {
    KbError::Io(format!("{what} {}: {e}", path.display()))
}

/// A collection name must stay inside its location: no separators, no `..`,
/// not hidden, not empty.
pub fn validate_name(name: &str) -> Result<(), KbError> {
    let bad = name.trim().is_empty()
        || name != name.trim()
        || name.starts_with('.')
        || name.contains(['/', '\\', ':'])
        || name.contains(".removed-");
    if bad {
        Err(KbError::InvalidName(name.to_string()))
    } else {
        Ok(())
    }
}

/// The collections under `location`, sorted by name. A folder is a collection
/// when it holds a `documents/` folder or an index.
pub fn list_collections(location: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(location) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| {
            let p = e.path();
            p.join(DOCUMENTS_DIR).is_dir() || p.join(INDEX_FILE).is_file()
        })
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| validate_name(n).is_ok())
        .collect();
    names.sort();
    names
}

/// Take a collection out of use by moving its folder aside to
/// `<name>.removed-<unix seconds>`. Its documents may be the only copy the
/// users have, so nothing is deleted.
pub fn remove_collection(location: &Path, name: &str) -> Result<PathBuf, KbError> {
    validate_name(name)?;
    let dir = location.join(name);
    if !dir.is_dir() {
        return Err(KbError::Io(format!("there is no collection {}", dir.display())));
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut aside = location.join(format!("{name}.removed-{stamp}"));
    let mut n = 1;
    while aside.exists() {
        aside = location.join(format!("{name}.removed-{stamp}-{n}"));
        n += 1;
    }
    std::fs::rename(&dir, &aside).map_err(|e| io_err("could not move aside", &dir, e))?;
    Ok(aside)
}

/// An open collection.
pub struct Collection {
    dir: PathBuf,
    db: Database,
    write_wait: Duration,
}

impl Collection {
    /// Open the collection `name` under `location`, creating its folders and an
    /// empty index when they are absent (R7). An existing document is never
    /// touched. An index from another schema version is moved aside and
    /// rebuilt (R13); one of the IDE's stores is refused (R2).
    pub fn open(location: &Path, name: &str) -> Result<Self, KbError> {
        validate_name(name)?;
        let dir = location.join(name);
        let docs = dir.join(DOCUMENTS_DIR);
        std::fs::create_dir_all(&docs).map_err(|e| io_err("could not create", &docs, e))?;
        let index = dir.join(INDEX_FILE);
        let db = open_index(&index)?;
        let mut c = Collection {
            dir,
            db,
            write_wait: DEFAULT_WRITE_WAIT,
        };
        match c.check_tables()? {
            Some(version) if version == SCHEMA_VERSION => {}
            Some(other) => {
                drop(c.db);
                move_aside(&index, other)?;
                c.db = open_index(&index)?;
                c.initialise()?;
            }
            None => c.initialise()?,
        }
        Ok(c)
    }

    /// The collection's folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where the users' documents live.
    pub fn documents_dir(&self) -> PathBuf {
        self.dir.join(DOCUMENTS_DIR)
    }

    /// The index file.
    pub fn index_path(&self) -> PathBuf {
        self.dir.join(INDEX_FILE)
    }

    /// How long a write waits for another writer before answering busy.
    pub fn set_write_wait(&mut self, wait: Duration) {
        self.write_wait = wait;
    }

    pub(crate) fn db(&self) -> &Database {
        &self.db
    }

    /// The embedder the collection's vectors come from, once anything has been
    /// indexed with vectors.
    pub fn embedder_stamp(&self) -> Result<Option<String>, KbError> {
        let tx = self.db.begin_read().map_err(store_err)?;
        let table = tx.open_table(META).map_err(store_err)?;
        let stamp = table
            .get("embedder")
            .map_err(store_err)?
            .map(|v| String::from_utf8_lossy(v.value()).into_owned())
            .filter(|s| !s.is_empty());
        Ok(stamp)
    }

    /// Every document the index knows, by path.
    pub fn sources(&self) -> Result<BTreeMap<String, Source>, KbError> {
        let tx = self.db.begin_read().map_err(store_err)?;
        let table = tx.open_table(SOURCES).map_err(store_err)?;
        let mut out = BTreeMap::new();
        for row in table.iter().map_err(store_err)? {
            let (k, v) = row.map_err(store_err)?;
            if let Ok(source) = bincode::deserialize::<Source>(v.value()) {
                out.insert(k.value().to_string(), source);
            }
        }
        Ok(out)
    }

    /// Run `f` in a write transaction, holding the collection's write lock. The
    /// lock is polled for at most the write wait, then [`KbError::Busy`] is
    /// answered instead of blocking (R10).
    pub(crate) fn write<R>(
        &self,
        f: impl FnOnce(&WriteTransaction) -> Result<R, KbError>,
    ) -> Result<R, KbError> {
        let _lock = self.take_write_lock()?;
        let tx = self.db.begin_write().map_err(store_err)?;
        let out = f(&tx)?;
        tx.commit().map_err(store_err)?;
        Ok(out)
    }

    fn lock_path(&self) -> PathBuf {
        let mut p = self.index_path().into_os_string();
        p.push(LOCK_SUFFIX);
        PathBuf::from(p)
    }

    fn take_write_lock(&self) -> Result<File, KbError> {
        let path = self.lock_path();
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|e| io_err("could not open the lock file", &path, e))?;
        let deadline = Instant::now() + self.write_wait;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(file),
                Err(TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        return Err(KbError::Busy(self.write_wait));
                    }
                    std::thread::sleep(LOCK_POLL);
                }
                Err(TryLockError::Error(e)) => {
                    return Err(io_err("could not lock", &path, e));
                }
            }
        }
    }

    /// redb's own consistency check over the whole index.
    pub fn check_integrity(&mut self) -> Result<bool, KbError> {
        self.db.check_integrity().map_err(store_err)
    }

    /// The schema version recorded in the index, `None` for an empty file.
    /// Refuses a file holding the IDE's tables, or tables without `kb_meta`.
    fn check_tables(&self) -> Result<Option<u32>, KbError> {
        let tx = self.db.begin_read().map_err(store_err)?;
        let names: Vec<String> = tx
            .list_tables()
            .map_err(store_err)?
            .map(|t| t.name().to_string())
            .collect();
        if names.iter().any(|n| FOREIGN_TABLES.contains(&n.as_str()))
            || (!names.is_empty() && !names.iter().any(|n| n == META.name()))
        {
            return Err(KbError::NotAKnowledgeBase(self.index_path()));
        }
        if names.is_empty() {
            return Ok(None);
        }
        let table = tx.open_table(META).map_err(store_err)?;
        let version = table
            .get("schema")
            .map_err(store_err)?
            .and_then(|v| v.value().try_into().ok().map(u32::from_le_bytes));
        Ok(version)
    }

    /// Create the tables and record the schema version.
    fn initialise(&self) -> Result<(), KbError> {
        self.write(|tx| {
            let mut meta = tx.open_table(META).map_err(store_err)?;
            meta.insert("schema", SCHEMA_VERSION.to_le_bytes().as_slice())
                .map_err(store_err)?;
            drop(meta);
            tx.open_table(PASSAGES).map_err(store_err)?;
            tx.open_table(SOURCES).map_err(store_err)?;
            Ok(())
        })
    }
}

/// Open (or create) an index in redb's multi-process mode. A file in a redb
/// format this build cannot read is moved aside and replaced (R13).
fn open_index(path: &Path) -> Result<Database, KbError> {
    let open = |p: &Path| {
        Database::builder()
            .set_concurrency_mode(ConcurrencyMode::MultiWriter)
            .create(p)
    };
    match open(path) {
        Ok(db) => Ok(db),
        Err(DatabaseError::UpgradeRequired(v)) => {
            move_aside(path, u32::from(v))?;
            open(path).map_err(store_err)
        }
        Err(e) => {
            // A file that is not a redb database at all is not ours to read.
            if path.is_file() && !is_redb_file(path) {
                Err(KbError::NotAKnowledgeBase(path.to_path_buf()))
            } else {
                Err(store_err(e))
            }
        }
    }
}

fn is_redb_file(path: &Path) -> bool {
    use std::io::Read;
    let mut magic = [0u8; 9];
    File::open(path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .map(|()| &magic == b"redb\x1A\x0A\xA9\x0D\x0A")
        .unwrap_or(false)
}

/// Move an index aside as `<file>.v<N>-obsolete` — never delete it.
fn move_aside(path: &Path, version: u32) -> Result<PathBuf, KbError> {
    let base = format!("{}.v{version}-obsolete", path.display());
    let mut aside = PathBuf::from(&base);
    let mut n = 1;
    while aside.exists() {
        aside = PathBuf::from(format!("{base}-{n}"));
        n += 1;
    }
    std::fs::rename(path, &aside).map_err(|e| io_err("could not move aside", path, e))?;
    Ok(aside)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_creates_the_folders_and_leaves_existing_documents_alone() {
        let root = tempfile::tempdir().unwrap();
        let c = Collection::open(root.path(), "hr").unwrap();
        assert!(c.documents_dir().is_dir());
        assert!(c.index_path().is_file());
        std::fs::write(c.documents_dir().join("policy.md"), "# Leave\nTwenty days.").unwrap();
        drop(c);
        let again = Collection::open(root.path(), "hr").unwrap();
        assert_eq!(
            std::fs::read_to_string(again.documents_dir().join("policy.md")).unwrap(),
            "# Leave\nTwenty days."
        );
        assert_eq!(list_collections(root.path()), vec!["hr".to_string()]);
    }

    #[test]
    fn an_ide_store_is_refused_not_read() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("grace");
        std::fs::create_dir_all(&dir).unwrap();
        {
            let db = Database::create(dir.join(INDEX_FILE)).unwrap();
            let tx = db.begin_write().unwrap();
            let chunks: TableDefinition<&str, &[u8]> = TableDefinition::new("chunks");
            tx.open_table(chunks).unwrap().insert("x", b"y".as_slice()).unwrap();
            tx.commit().unwrap();
        }
        let err = Collection::open(root.path(), "grace").err().unwrap();
        assert!(matches!(err, KbError::NotAKnowledgeBase(_)), "{err}");
        assert!(err.to_string().contains("not an application Knowledge Base"));
    }

    #[test]
    fn another_schema_version_is_moved_aside_and_rebuilt() {
        let root = tempfile::tempdir().unwrap();
        let c = Collection::open(root.path(), "legal").unwrap();
        c.write(|tx| {
            let mut meta = tx.open_table(META).map_err(store_err)?;
            meta.insert("schema", 0u32.to_le_bytes().as_slice()).map_err(store_err)?;
            Ok(())
        })
        .unwrap();
        let index = c.index_path();
        drop(c);
        let reopened = Collection::open(root.path(), "legal").unwrap();
        assert!(PathBuf::from(format!("{}.v0-obsolete", index.display())).is_file(), "kept, not deleted");
        assert_eq!(reopened.check_tables().unwrap(), Some(SCHEMA_VERSION));
    }

    #[test]
    fn names_cannot_escape_the_location_and_removal_moves_aside() {
        for bad in ["", "..", "../x", "a/b", "a\\b", ".hidden", " x"] {
            assert!(validate_name(bad).is_err(), "{bad:?} must be refused");
        }
        let root = tempfile::tempdir().unwrap();
        Collection::open(root.path(), "orders").unwrap();
        let aside = remove_collection(root.path(), "orders").unwrap();
        assert!(aside.join(DOCUMENTS_DIR).is_dir(), "documents kept");
        assert!(list_collections(root.path()).is_empty());
    }

    /// Another holder of the write lock makes a writer answer busy after the
    /// wait, instead of hanging (R10, AC5).
    #[test]
    fn a_held_write_lock_answers_busy_after_the_wait() {
        let root = tempfile::tempdir().unwrap();
        let mut c = Collection::open(root.path(), "busy").unwrap();
        c.set_write_wait(Duration::from_millis(200));
        let holder = c.take_write_lock().unwrap();
        let started = Instant::now();
        let err = c.write(|_| Ok(())).err().unwrap();
        let waited = started.elapsed();
        assert_eq!(err, KbError::Busy(Duration::from_millis(200)));
        assert!(waited >= Duration::from_millis(200) && waited < Duration::from_millis(1500));
        drop(holder);
        c.write(|_| Ok(())).unwrap();
        println!("busy after {} ms; write succeeds once the lock is free", waited.as_millis());
    }
}
