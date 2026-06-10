// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Indexed (ISAM / key-sequenced) file engine.
//!
//! Self-contained, no external dependencies. Implements the COBOL indexed-file
//! model: a PRIMARY key (unique) plus any number of ALTERNATE keys (with or
//! without duplicates), ordered access for `START` / `READ NEXT` / `READ
//! PREVIOUS`, the `WRITE` / `REWRITE` / `DELETE` verbs, record-level locking,
//! and a write-ahead journal driving `COMMIT` / `ROLLBACK`.
//!
//! Records are opaque fixed-length byte buffers; a key is a `[offset, len)`
//! slice of the record. The engine is record-store-agnostic — the interpreter
//! supplies record bytes and key positions.
//!
//! Locking follows the key-sequenced-data-set sharing model: under `OPEN I-O` a
//! successful `READ` of an existing record takes an exclusive record lock,
//! `REWRITE`/`DELETE` operate on that locked current record, and `CLOSE`
//! releases all locks. The store is a single run unit, so locks are bookkeeping
//! that also enforces "REWRITE/DELETE need a prior READ" in sequential access.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

pub type Bytes = Vec<u8>;

/// The runtime surface every indexed-file backend exposes, so the interpreter
/// can drive either the in-memory engine ([`IndexedFile`], `STORAGE IS
/// MEMORY`) or the on-disk B+tree engine
/// ([`crate::indexed_disk::DiskIndexedFile`], `STORAGE IS DISK`) through a
/// single `Box<dyn IndexedStore>` handle.
pub trait IndexedStore {
    fn open(&mut self, mode: OpenMode) -> &'static str;
    fn close(&mut self) -> &'static str;
    fn write(&mut self, rec: &[u8]) -> &'static str;
    fn read_key(&mut self, key: &[u8]) -> (Option<Bytes>, &'static str);
    fn read_seq(&mut self, dir: ReadDir) -> (Option<Bytes>, &'static str);
    fn start(&mut self, op: StartOp, key: &[u8]) -> &'static str;
    fn rewrite(&mut self, rec: &[u8], random_key: Option<&[u8]>) -> &'static str;
    fn delete(&mut self, random_key: Option<&[u8]>) -> &'static str;
    fn set_key_of_reference(&mut self, kor: usize);
    fn is_open(&self) -> bool;
    /// Release all record locks held on the file (`UNLOCK`). Default: no-op.
    fn unlock(&mut self) {}
    /// `COMMIT` — make changes durable and start a new transaction. Default: no-op.
    fn commit(&mut self) {}
    /// `ROLLBACK` — undo changes since the last `COMMIT`/`OPEN`. Default: no-op.
    fn rollback(&mut self) {}
}

/// Status codes (the FILE STATUS two-character values this engine produces).
pub mod status {
    pub const OK: &str = "00";
    pub const DUP_ALT_OK: &str = "02"; // duplicate alternate key, allowed
    pub const EOF: &str = "10";
    pub const DUP_KEY: &str = "22"; // duplicate primary/no-dup alternate on WRITE
    pub const NOT_FOUND: &str = "23"; // record not found / no current record
    pub const BOUNDARY: &str = "24"; // boundary violation
    pub const FILE_NOT_FOUND: &str = "35"; // OPEN INPUT/I-O of a non-existent file
    pub const ATTR_MISMATCH: &str = "39"; // existing file attributes ≠ declared file
    pub const NO_NEXT: &str = "46"; // sequential READ with no current record established
    pub const NOT_OPEN_INPUT: &str = "47";
    pub const NOT_OPEN_OUTPUT: &str = "48";
    pub const NOT_OPEN_IO: &str = "49";
    pub const IO_ERROR: &str = "90";
    pub const LOGIC_ERROR: &str = "92";
    pub const UNAVAILABLE: &str = "93";
    pub const RECORD_LOCKED: &str = "99";
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpenMode {
    Input,
    Output,
    Io,
    Extend,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LockMode {
    Shared,
    Exclusive,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReadDir {
    Next,
    Previous,
}

/// Which indexed (ISAM) file engine backs `ORGANIZATION IS INDEXED` files.
///
/// Selectable per run via `rcrun --indexed-engine <name>` or the
/// `COBOL_INDEXED_ENGINE` environment variable. All engines are required to
/// present *identical* observable COBOL behaviour (file-status codes, key
/// ordering, locking, COMMIT/ROLLBACK); they differ only in their on-disk
/// container format. Only [`IndexedEngine::Rust`] has a native container today,
/// so `RmCobol85` / `Fujitsu` currently delegate to it (behaviour-compatible)
/// until their native formats land.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum IndexedEngine {
    /// The built-in, dependency-free Rust ISAM engine (KSDS-style, journaled).
    #[default]
    Rust,
    /// RM/COBOL-85 indexed files (delegates to the Rust engine for now).
    RmCobol85,
    /// Fujitsu COBOL-85 indexed files (delegates to the Rust engine for now).
    Fujitsu,
    /// Crash-safe redb substrate (`STORAGE IS DISK`): O(1) OPEN, working-set RAM,
    /// ACID COMMIT/ROLLBACK. See [`crate::indexed_redb`].
    Redb,
}

impl IndexedEngine {
    /// Parse an engine name (case-insensitive). Accepts a few common aliases.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace(['_', ' '], "-").as_str() {
            "rust" | "rstcobol" | "rustcobol" | "native" | "default" => Some(Self::Rust),
            "rm" | "rm-cobol" | "rm-cobol85" | "rmcobol" | "rmcobol85" => Some(Self::RmCobol85),
            "fujitsu" | "fujitsu-cobol" | "fujitsu-cobol85" | "fj" => Some(Self::Fujitsu),
            "redb" | "crash-safe" | "acid" => Some(Self::Redb),
            _ => None,
        }
    }

    /// Canonical lower-case name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::RmCobol85 => "rm-cobol85",
            Self::Fujitsu => "fujitsu",
            Self::Redb => "redb",
        }
    }
}

/// Relational operator for `START`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StartOp {
    Eq,
    Gt,
    Ge,
    Lt,
    Le,
}

// ── Self-describing schema metadata ─────────────────────────────────────────
//
// These types describe an indexed file richly enough that a future Fujitsu
// importer can reconstruct it faithfully. They mirror the metadata Fujitsu's
// File Access Subroutines expose via `cobfa_indexinfo()` — record format,
// record length, key count/total length, the primary key, and alternate keys
// (each as one or more byte-ranged parts, with encoding, ordering and a
// duplicates flag). They are persisted in the `PRCIDX1` container and surfaced
// by [`IndexedFile::inspect`] / [`IndexedFile::inspect_path`].
//
// Semantic model only — NOT a binary-compatible Fujitsu `cobidx`/`cobi64` file.

/// Fixed- or variable-length record format. The Rust engine currently writes
/// fixed-length records; `Variable` is representable so a converter can record
/// a Fujitsu variable-length file's bounds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecordFormat {
    Fixed { length: u32 },
    Variable { min_length: u32, max_length: u32 },
}

impl RecordFormat {
    /// The maximum payload length this format admits.
    pub fn max_len(&self) -> u32 {
        match self {
            RecordFormat::Fixed { length } => *length,
            RecordFormat::Variable { max_length, .. } => *max_length,
        }
    }
}

/// How a key part's bytes are interpreted for ordering. Byte offsets/lengths are
/// always byte-based (never character counts), matching Fujitsu's Unicode-mode
/// rule. The engine orders bytewise today; richer collations are future work,
/// but the encoding is preserved so a converter round-trips it losslessly.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyEncoding {
    Bytes,
    DisplayAscii,
    DisplayUtf8,
    Ucs2Le,
    Ucs2Be,
    Utf32Le,
    Utf32Be,
    PackedDecimal,
    BinaryBigEndian,
    BinaryLittleEndian,
}

impl KeyEncoding {
    fn to_u8(self) -> u8 {
        match self {
            KeyEncoding::Bytes => 0,
            KeyEncoding::DisplayAscii => 1,
            KeyEncoding::DisplayUtf8 => 2,
            KeyEncoding::Ucs2Le => 3,
            KeyEncoding::Ucs2Be => 4,
            KeyEncoding::Utf32Le => 5,
            KeyEncoding::Utf32Be => 6,
            KeyEncoding::PackedDecimal => 7,
            KeyEncoding::BinaryBigEndian => 8,
            KeyEncoding::BinaryLittleEndian => 9,
        }
    }
    fn from_u8(b: u8) -> KeyEncoding {
        match b {
            1 => KeyEncoding::DisplayAscii,
            2 => KeyEncoding::DisplayUtf8,
            3 => KeyEncoding::Ucs2Le,
            4 => KeyEncoding::Ucs2Be,
            5 => KeyEncoding::Utf32Le,
            6 => KeyEncoding::Utf32Be,
            7 => KeyEncoding::PackedDecimal,
            8 => KeyEncoding::BinaryBigEndian,
            9 => KeyEncoding::BinaryLittleEndian,
            _ => KeyEncoding::Bytes,
        }
    }
}

/// Ascending or descending key ordering.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyOrdering {
    Ascending,
    Descending,
}

/// One contiguous byte range of a (possibly composite) key.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeyPart {
    pub offset: u32,
    pub length: u32,
    pub encoding: KeyEncoding,
}

/// A key: number 1 is the primary key; 2.. are alternates in declaration order.
/// `parts` is concatenated in order to form the full key value (composite keys).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeyDescriptor {
    pub key_number: u16,
    pub name: Option<String>,
    pub parts: Vec<KeyPart>,
    pub duplicates_allowed: bool,
    pub ordering: KeyOrdering,
}

impl KeyDescriptor {
    /// Total byte length of all parts.
    pub fn total_length(&self) -> u32 {
        self.parts.iter().map(|p| p.length).sum()
    }
}

/// The full key list of an indexed file (primary + ordered alternates).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeyList {
    pub primary: KeyDescriptor,
    pub alternates: Vec<KeyDescriptor>,
}

/// Discoverable description of an indexed file — the `cobfa_indexinfo()` analog.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IndexedFileInfo {
    pub record_format: RecordFormat,
    pub key_count: u16,
    pub total_key_length: u32,
    pub primary: KeyDescriptor,
    pub alternates: Vec<KeyDescriptor>,
}

/// A key: a `[offset, offset+len)` slice of the record, optionally allowing
/// duplicate values (alternate keys only). The engine's runtime key type —
/// single-part by construction; the richer [`KeyDescriptor`] is the persisted /
/// discoverable form.
#[derive(Clone, Debug)]
pub struct KeySpec {
    pub offset: usize,
    pub len: usize,
    pub duplicates: bool,
}

impl KeySpec {
    fn extract(&self, rec: &[u8]) -> Bytes {
        let end = (self.offset + self.len).min(rec.len());
        let start = self.offset.min(rec.len());
        let mut k = rec[start..end].to_vec();
        k.resize(self.len, b' '); // pad short records with spaces
        k
    }
}

#[derive(Clone)]
enum Journal {
    Insert(Bytes),               // primary key inserted
    Update(Bytes, Bytes),        // primary key, previous record bytes
    Delete(Bytes, Bytes),        // primary key, previous record bytes
}

/// One indexed file.
pub struct IndexedFile {
    path: PathBuf,
    record_len: usize,
    primary: KeySpec,
    alternates: Vec<KeySpec>,

    /// Primary key → record bytes (ordered for sequential access).
    records: BTreeMap<Bytes, Bytes>,
    /// Per alternate key: alt-key value → set of primary keys (ordered).
    alt_index: Vec<BTreeMap<Bytes, BTreeSet<Bytes>>>,

    open: Option<OpenMode>,
    /// Key of reference: 0 = primary, 1..=N = alternates[idx-1].
    kor: usize,
    /// Current sequential position (primary key), if positioned.
    cursor: Option<Bytes>,
    /// Last successfully read record's primary key (for REWRITE/DELETE).
    current: Option<Bytes>,
    locks: HashMap<Bytes, LockMode>,
    journal: Vec<Journal>,

    // ── Self-describing schema metadata (persisted in PRCIDX1) ──────────────
    /// Optional key-field names for the schema (`[primary, alt1, …]`); purely
    /// descriptive, surfaced via `inspect`. The engine never keys off names.
    key_names: Vec<Option<String>>,
    /// When `true`, OPEN validates the stored schema against the declared keys
    /// and returns FILE STATUS 39 on mismatch.
    strict_metadata: bool,
    /// `WITH COMPRESSION`: compress each record in the persisted container.
    compressing: bool,
    /// Creation timestamp (ms), preserved across load/save.
    created_ms: u64,
}

impl IndexedFile {
    pub fn new(path: impl AsRef<Path>, record_len: usize, primary: KeySpec, alternates: Vec<KeySpec>) -> Self {
        let n_alt = alternates.len();
        IndexedFile {
            path: path.as_ref().to_path_buf(),
            record_len,
            primary,
            alternates,
            records: BTreeMap::new(),
            alt_index: vec![BTreeMap::new(); n_alt],
            open: None,
            kor: 0,
            cursor: None,
            current: None,
            locks: HashMap::new(),
            journal: Vec::new(),
            key_names: Vec::new(),
            strict_metadata: true,
            compressing: false,
            created_ms: 0,
        }
    }

    /// Attach descriptive key-field names (`[primary, alt1, alt2, …]`) for the
    /// persisted schema / `inspect`. Optional and never affects keying.
    pub fn set_key_names(&mut self, names: Vec<Option<String>>) {
        self.key_names = names;
    }

    /// Enable/disable strict schema validation on OPEN (default: enabled).
    pub fn set_strict_metadata(&mut self, strict: bool) {
        self.strict_metadata = strict;
    }

    /// Enable/disable `WITH COMPRESSION` for the persisted container.
    pub fn set_compressing(&mut self, on: bool) {
        self.compressing = on;
    }

    fn key_name(&self, idx: usize) -> Option<String> {
        self.key_names.get(idx).cloned().flatten()
    }

    /// Build a [`KeyDescriptor`] from a single-part runtime [`KeySpec`].
    fn descriptor(&self, key_number: u16, spec: &KeySpec, name_idx: usize) -> KeyDescriptor {
        KeyDescriptor {
            key_number,
            name: self.key_name(name_idx),
            parts: vec![KeyPart {
                offset: spec.offset as u32,
                length: spec.len as u32,
                encoding: KeyEncoding::Bytes,
            }],
            duplicates_allowed: spec.duplicates,
            ordering: KeyOrdering::Ascending,
        }
    }

    /// The file's discoverable description (record format + full key list).
    pub fn inspect(&self) -> IndexedFileInfo {
        let primary = self.descriptor(1, &self.primary, 0);
        let alternates: Vec<KeyDescriptor> = self
            .alternates
            .iter()
            .enumerate()
            .map(|(i, ks)| self.descriptor((i + 2) as u16, ks, i + 1))
            .collect();
        let total_key_length =
            primary.total_length() + alternates.iter().map(|d| d.total_length()).sum::<u32>();
        IndexedFileInfo {
            record_format: RecordFormat::Fixed { length: self.record_len as u32 },
            key_count: 1 + alternates.len() as u16,
            total_key_length,
            primary,
            alternates,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Set the key of reference for subsequent sequential access (0 = primary).
    pub fn set_key_of_reference(&mut self, kor: usize) {
        self.kor = kor.min(self.alternates.len());
    }

    // ── OPEN / CLOSE ────────────────────────────────────────────────────────

    pub fn open(&mut self, mode: OpenMode) -> &'static str {
        if self.open.is_some() {
            return status::LOGIC_ERROR;
        }
        match mode {
            OpenMode::Output => {
                self.records.clear();
                self.rebuild_alt_index();
            }
            OpenMode::Input | OpenMode::Io | OpenMode::Extend => {
                if self.path.exists() {
                    match self.load() {
                        Ok(stored) => {
                            // Strict mode: the declared SELECT/FD keys + record
                            // format must match the existing file's schema.
                            if self.strict_metadata {
                                if let Some(s) = stored {
                                    if !self.schema_matches(&s) {
                                        return status::ATTR_MISMATCH; // 39
                                    }
                                }
                            }
                        }
                        Err(_) => return status::IO_ERROR,
                    }
                } else if mode == OpenMode::Input {
                    // Reading a non-existent file → FILE STATUS 35.
                    return status::FILE_NOT_FOUND;
                } else {
                    // I-O / EXTEND create the file if absent.
                    self.records.clear();
                    self.rebuild_alt_index();
                }
            }
        }
        self.open = Some(mode);
        self.kor = 0;
        self.cursor = None;
        self.current = None;
        self.journal.clear();
        status::OK
    }

    pub fn close(&mut self) -> &'static str {
        if self.open.is_none() {
            return status::LOGIC_ERROR;
        }
        let writable = matches!(self.open, Some(OpenMode::Output | OpenMode::Io | OpenMode::Extend));
        self.open = None;
        self.locks.clear();
        self.journal.clear();
        self.cursor = None;
        self.current = None;
        if writable && self.save().is_err() {
            return status::IO_ERROR;
        }
        status::OK
    }

    // ── WRITE ───────────────────────────────────────────────────────────────

    pub fn write(&mut self, rec: &[u8]) -> &'static str {
        match self.open {
            Some(OpenMode::Output | OpenMode::Io | OpenMode::Extend) => {}
            Some(OpenMode::Input) => return status::NOT_OPEN_OUTPUT,
            None => return status::NOT_OPEN_OUTPUT,
        }
        let rec = self.fit(rec);
        let pkey = self.primary.extract(&rec);
        if self.records.contains_key(&pkey) {
            return status::DUP_KEY;
        }
        // A WITHOUT-DUPLICATES alternate must not already hold this value;
        // WITH-DUPLICATES alternates accept it (a duplicate is a normal,
        // fully successful write — status 00, not the informational 02).
        for (i, ks) in self.alternates.iter().enumerate() {
            if ks.duplicates {
                continue;
            }
            let ak = ks.extract(&rec);
            if let Some(set) = self.alt_index[i].get(&ak) {
                if !set.is_empty() {
                    return status::DUP_KEY;
                }
            }
        }
        self.records.insert(pkey.clone(), rec.clone());
        self.index_insert(&pkey, &rec);
        self.journal.push(Journal::Insert(pkey));
        status::OK
    }

    // ── READ (random by key of reference) ───────────────────────────────────

    /// Random read: find the record whose key-of-reference value equals `key`.
    /// Establishes the current record (and, under I-O, an exclusive lock).
    pub fn read_key(&mut self, key: &[u8]) -> (Option<Bytes>, &'static str) {
        if !matches!(self.open, Some(OpenMode::Input | OpenMode::Io)) {
            return (None, status::NOT_OPEN_INPUT);
        }
        let pkey = match self.resolve_primary(key) {
            Some(p) => p,
            None => return (None, status::NOT_FOUND),
        };
        let rec = self.records.get(&pkey).cloned();
        match rec {
            Some(r) => {
                self.cursor = Some(pkey.clone());
                self.current = Some(pkey.clone());
                if self.open == Some(OpenMode::Io) {
                    self.locks.insert(pkey, LockMode::Exclusive);
                }
                (Some(r), status::OK)
            }
            None => (None, status::NOT_FOUND),
        }
    }

    // ── READ NEXT / PREVIOUS (sequential / dynamic) ─────────────────────────

    pub fn read_seq(&mut self, dir: ReadDir) -> (Option<Bytes>, &'static str) {
        if !matches!(self.open, Some(OpenMode::Input | OpenMode::Io)) {
            return (None, status::NOT_OPEN_INPUT);
        }
        let order = self.ordered_primary_keys();
        if order.is_empty() {
            return (None, status::EOF);
        }
        let next_pkey = match &self.cursor {
            None => match dir {
                ReadDir::Next => Some(order[0].clone()),
                ReadDir::Previous => Some(order[order.len() - 1].clone()),
            },
            Some(cur) => {
                let pos = order.iter().position(|k| k == cur);
                match (pos, dir) {
                    (Some(p), ReadDir::Next) => order.get(p + 1).cloned(),
                    (Some(p), ReadDir::Previous) => if p == 0 { None } else { order.get(p - 1).cloned() },
                    (None, ReadDir::Next) => order.iter().find(|k| **k > *cur).cloned(),
                    (None, ReadDir::Previous) => order.iter().rev().find(|k| **k < *cur).cloned(),
                }
            }
        };
        match next_pkey {
            Some(p) => {
                let rec = self.records.get(&p).cloned();
                self.cursor = Some(p.clone());
                self.current = Some(p.clone());
                if self.open == Some(OpenMode::Io) {
                    self.locks.insert(p, LockMode::Exclusive);
                }
                (rec, status::OK)
            }
            None => (None, status::EOF),
        }
    }

    // ── START (positioning) ─────────────────────────────────────────────────

    pub fn start(&mut self, op: StartOp, key: &[u8]) -> &'static str {
        if !matches!(self.open, Some(OpenMode::Input | OpenMode::Io)) {
            return status::NOT_OPEN_INPUT;
        }
        let key = key.to_vec();
        // Iterate the key-of-reference ordering and find the first qualifying
        // record; position the cursor *before* it so the next READ NEXT returns
        // it (we store the qualifying primary key directly as the cursor and
        // back it up via a sentinel by recording it as "current position minus
        // one"). Simpler: store the matched primary key and let READ NEXT detect
        // that the cursor hasn't been consumed.
        let matched = self.find_start(op, &key);
        match matched {
            Some(pkey) => {
                // Position so the *next* READ NEXT yields this record: set cursor
                // to the predecessor in primary order.
                let order = self.ordered_primary_keys();
                let idx = order.iter().position(|k| *k == pkey).unwrap_or(0);
                self.cursor = if idx == 0 { None } else { Some(order[idx - 1].clone()) };
                self.current = None;
                status::OK
            }
            None => status::NOT_FOUND,
        }
    }

    // ── REWRITE / DELETE ────────────────────────────────────────────────────

    pub fn rewrite(&mut self, rec: &[u8], random_key: Option<&[u8]>) -> &'static str {
        if self.open != Some(OpenMode::Io) {
            return status::NOT_OPEN_IO;
        }
        let rec = self.fit(rec);
        let pkey = self.primary.extract(&rec);
        // Sequential access requires the record be the current (read) one;
        // random access (random_key given) addresses by primary key directly.
        let target = match random_key {
            Some(_) => pkey.clone(),
            None => match &self.current {
                Some(c) => c.clone(),
                None => return status::NO_NEXT,
            },
        };
        // The primary key may not change on REWRITE.
        if target != pkey {
            return status::LOGIC_ERROR;
        }
        let old = match self.records.get(&pkey) {
            Some(r) => r.clone(),
            None => return status::NOT_FOUND,
        };
        // No-duplicate alternate uniqueness must still hold (excluding self).
        for (i, ks) in self.alternates.iter().enumerate() {
            if ks.duplicates {
                continue;
            }
            let ak = ks.extract(&rec);
            if let Some(set) = self.alt_index[i].get(&ak) {
                if set.iter().any(|p| *p != pkey) {
                    return status::DUP_KEY;
                }
            }
        }
        self.index_remove(&pkey, &old);
        self.records.insert(pkey.clone(), rec.clone());
        self.index_insert(&pkey, &rec);
        self.locks.remove(&pkey);
        self.journal.push(Journal::Update(pkey, old));
        status::OK
    }

    pub fn delete(&mut self, random_key: Option<&[u8]>) -> &'static str {
        if self.open != Some(OpenMode::Io) {
            return status::NOT_OPEN_IO;
        }
        let pkey = match random_key {
            Some(k) => match self.resolve_primary(k) {
                Some(p) => p,
                None => return status::NOT_FOUND,
            },
            None => match &self.current {
                Some(c) => c.clone(),
                None => return status::NO_NEXT,
            },
        };
        let old = match self.records.remove(&pkey) {
            Some(r) => r,
            None => return status::NOT_FOUND,
        };
        self.index_remove(&pkey, &old);
        self.locks.remove(&pkey);
        if self.current.as_ref() == Some(&pkey) {
            self.current = None;
        }
        self.journal.push(Journal::Delete(pkey, old));
        status::OK
    }

    // ── COMMIT / ROLLBACK ───────────────────────────────────────────────────

    /// Make all changes since the last COMMIT/ROLLBACK permanent and release
    /// transaction-scoped locks.
    pub fn commit(&mut self) {
        self.journal.clear();
        self.locks.clear();
        let _ = self.save();
    }

    /// Undo all changes since the last COMMIT/ROLLBACK and release locks.
    pub fn rollback(&mut self) {
        while let Some(entry) = self.journal.pop() {
            match entry {
                Journal::Insert(pkey) => {
                    if let Some(old) = self.records.remove(&pkey) {
                        self.index_remove(&pkey, &old);
                    }
                }
                Journal::Update(pkey, old) => {
                    if let Some(cur) = self.records.get(&pkey).cloned() {
                        self.index_remove(&pkey, &cur);
                    }
                    self.records.insert(pkey.clone(), old.clone());
                    self.index_insert(&pkey, &old);
                }
                Journal::Delete(pkey, old) => {
                    self.records.insert(pkey.clone(), old.clone());
                    self.index_insert(&pkey, &old);
                }
            }
        }
        self.locks.clear();
        self.cursor = None;
        self.current = None;
    }

    // ── Internals ───────────────────────────────────────────────────────────

    fn fit(&self, rec: &[u8]) -> Bytes {
        let mut r = rec.to_vec();
        r.resize(self.record_len, b' ');
        r
    }

    /// Resolve a key-of-reference value to a primary key.
    fn resolve_primary(&self, key: &[u8]) -> Option<Bytes> {
        if self.kor == 0 {
            let k = pad(key, self.primary.len);
            if self.records.contains_key(&k) { Some(k) } else { None }
        } else {
            let idx = self.kor - 1;
            let k = pad(key, self.alternates[idx].len);
            self.alt_index[idx].get(&k).and_then(|set| set.iter().next().cloned())
        }
    }

    /// Ordered list of primary keys in the current key-of-reference ordering.
    #[doc(hidden)]
    pub fn debug_keys(&self) -> Vec<String> {
        self.records.keys().map(|k| String::from_utf8_lossy(k).to_string()).collect()
    }

    fn ordered_primary_keys(&self) -> Vec<Bytes> {
        if self.kor == 0 {
            self.records.keys().cloned().collect()
        } else {
            let idx = self.kor - 1;
            self.alt_index[idx]
                .values()
                .flat_map(|set| set.iter().cloned())
                .collect()
        }
    }

    fn find_start(&self, op: StartOp, key: &[u8]) -> Option<Bytes> {
        let ks_len = if self.kor == 0 { self.primary.len } else { self.alternates[self.kor - 1].len };
        let key = pad(key, ks_len);
        if self.kor == 0 {
            match op {
                StartOp::Eq => self.records.get(&key).map(|_| key.clone()),
                StartOp::Ge => self.records.range(key..).next().map(|(k, _)| k.clone()),
                StartOp::Gt => self.records.range(key.clone()..).find(|(k, _)| **k > key).map(|(k, _)| k.clone()),
                StartOp::Le => self.records.range(..=key.clone()).next_back().map(|(k, _)| k.clone()),
                StartOp::Lt => self.records.range(..key).next_back().map(|(k, _)| k.clone()),
            }
        } else {
            let idx = self.kor - 1;
            let map = &self.alt_index[idx];
            let akey = match op {
                StartOp::Eq => map.get(&key).map(|_| key.clone()),
                StartOp::Ge => map.range(key..).next().map(|(k, _)| k.clone()),
                StartOp::Gt => map.range(key.clone()..).find(|(k, _)| **k > key).map(|(k, _)| k.clone()),
                StartOp::Le => map.range(..=key.clone()).next_back().map(|(k, _)| k.clone()),
                StartOp::Lt => map.range(..key).next_back().map(|(k, _)| k.clone()),
            };
            akey.and_then(|ak| map.get(&ak).and_then(|set| set.iter().next().cloned()))
        }
    }

    fn index_insert(&mut self, pkey: &[u8], rec: &[u8]) {
        for (i, ks) in self.alternates.iter().enumerate() {
            let ak = ks.extract(rec);
            self.alt_index[i].entry(ak).or_default().insert(pkey.to_vec());
        }
    }

    fn index_remove(&mut self, pkey: &[u8], rec: &[u8]) {
        for (i, ks) in self.alternates.iter().enumerate() {
            let ak = ks.extract(rec);
            if let Some(set) = self.alt_index[i].get_mut(&ak) {
                set.remove(pkey);
                if set.is_empty() {
                    self.alt_index[i].remove(&ak);
                }
            }
        }
    }

    fn rebuild_alt_index(&mut self) {
        for m in &mut self.alt_index {
            m.clear();
        }
        let snapshot: Vec<(Bytes, Bytes)> = self.records.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for (pkey, rec) in snapshot {
            self.index_insert(&pkey, &rec);
        }
    }

    // ── Persistence (PowerRustCOBOL's own ISAM container) ───────────────────
    //
    // The current container is the self-describing `PRCIDX1` format: an 8-byte
    // magic, a header (version, record format + lengths, key count, created/
    // updated timestamps), the full key schema (primary + alternates, each with
    // composite byte-ranged parts, encoding, ordering and a duplicates flag),
    // then the records, then a CRC-32 trailer over everything preceding it.
    //
    // The legacy `PRCISAM1` container (magic + record_len + records, no schema)
    // is still read for backward compatibility.

    /// True if the declared keys + record format match the stored schema
    /// (names are descriptive only and intentionally ignored).
    fn schema_matches(&self, stored: &IndexedFileInfo) -> bool {
        fn key_eq(a: &KeyDescriptor, b: &KeyDescriptor) -> bool {
            a.duplicates_allowed == b.duplicates_allowed
                && a.ordering == b.ordering
                && a.parts.len() == b.parts.len()
                && a.parts.iter().zip(&b.parts).all(|(x, y)| {
                    x.offset == y.offset && x.length == y.length && x.encoding == y.encoding
                })
        }
        let decl = self.inspect();
        decl.record_format == stored.record_format
            && decl.key_count == stored.key_count
            && key_eq(&decl.primary, &stored.primary)
            && decl.alternates.len() == stored.alternates.len()
            && decl.alternates.iter().zip(&stored.alternates).all(|(a, b)| key_eq(a, b))
    }

    /// Load the container. Returns the stored schema when the file is a
    /// self-describing `PRCIDX1` (so OPEN can validate it); `None` for the
    /// legacy / unknown containers.
    fn load(&mut self) -> std::io::Result<Option<IndexedFileInfo>> {
        let data = std::fs::read(&self.path)?;
        self.records.clear();
        if data.len() >= 8 && &data[0..8] == b"PRCIDX1\0" {
            let info = self.load_prcidx(&data)?;
            self.rebuild_alt_index();
            return Ok(Some(info));
        }
        if data.len() >= 12 && &data[0..8] == b"PRCISAM1" {
            self.load_legacy(&data);
            self.rebuild_alt_index();
            return Ok(None);
        }
        // Unknown container — treat as empty rather than failing hard.
        self.rebuild_alt_index();
        Ok(None)
    }

    /// Parse the legacy `PRCISAM1` container (records only, no schema).
    fn load_legacy(&mut self, data: &[u8]) {
        let need = |i: usize, n: usize, len: usize| i + n <= len;
        let mut i = 12usize; // skip magic(8) + record_len(4)
        while need(i, 4, data.len()) {
            let rlen = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;
            i += 4;
            if !need(i, rlen, data.len()) {
                break;
            }
            let rec = data[i..i + rlen].to_vec();
            i += rlen;
            let pkey = self.primary.extract(&rec);
            self.records.insert(pkey, rec);
        }
    }

    /// Parse the self-describing `PRCIDX1` container, validating its CRC-32.
    fn load_prcidx(&mut self, data: &[u8]) -> std::io::Result<IndexedFileInfo> {
        if data.len() < 4 {
            return Err(trunc());
        }
        // Validate the CRC-32 trailer first.
        let body = &data[..data.len() - 4];
        let want = u32::from_le_bytes([
            data[data.len() - 4], data[data.len() - 3], data[data.len() - 2], data[data.len() - 1],
        ]);
        if crc32(body) != want {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "prcidx CRC mismatch (corrupt container)",
            ));
        }

        let mut c = Cur { d: body, i: 8 }; // skip magic
        let _version = c.u16()?;
        let flags = c.u16()?;
        self.compressing = flags & 1 != 0;
        let rf = c.u8()?;
        let _reserved = c.u8()?;
        let fixed_length = c.u32()?;
        let min_length = c.u32()?;
        let max_length = c.u32()?;
        let key_count = c.u16()?;
        self.created_ms = c.u64()?;
        let _updated = c.u64()?;

        let record_format = match rf {
            2 => RecordFormat::Variable { min_length, max_length },
            _ => RecordFormat::Fixed { length: fixed_length },
        };

        // Key descriptors: primary (number 1) then alternates in order.
        let mut keys: Vec<KeyDescriptor> = Vec::with_capacity(key_count as usize);
        for _ in 0..key_count {
            let key_number = c.u16()?;
            let duplicates_allowed = c.u8()? != 0;
            let ordering = if c.u8()? == 1 { KeyOrdering::Descending } else { KeyOrdering::Ascending };
            let part_count = c.u16()?;
            let name_len = c.u16()? as usize;
            let name_bytes = c.bytes(name_len)?;
            let name = if name_bytes.is_empty() {
                None
            } else {
                Some(String::from_utf8_lossy(name_bytes).into_owned())
            };
            let mut parts = Vec::with_capacity(part_count as usize);
            for _ in 0..part_count {
                let offset = c.u32()?;
                let length = c.u32()?;
                let encoding = KeyEncoding::from_u8(c.u8()?);
                let _reserved = c.u8()?;
                parts.push(KeyPart { offset, length, encoding });
            }
            keys.push(KeyDescriptor { key_number, name, parts, duplicates_allowed, ordering });
        }

        // Records (decompressed when the container is COMPRESSION-encoded).
        let record_count = c.u64()?;
        for _ in 0..record_count {
            let rlen = c.u32()? as usize;
            let stored = c.bytes(rlen)?;
            let rec = if self.compressing { crate::compress::decompress(stored) } else { stored.to_vec() };
            let pkey = self.primary.extract(&rec);
            self.records.insert(pkey, rec);
        }

        let primary = keys.first().cloned().unwrap_or(KeyDescriptor {
            key_number: 1, name: None, parts: vec![], duplicates_allowed: false,
            ordering: KeyOrdering::Ascending,
        });
        let alternates = if keys.len() > 1 { keys[1..].to_vec() } else { Vec::new() };
        let total_key_length =
            primary.total_length() + alternates.iter().map(|d| d.total_length()).sum::<u32>();
        Ok(IndexedFileInfo { record_format, key_count, total_key_length, primary, alternates })
    }

    fn save(&self) -> std::io::Result<()> {
        let info = self.inspect();
        let mut out = Vec::new();
        out.extend_from_slice(b"PRCIDX1\0"); // 8-byte magic
        out.extend_from_slice(&1u16.to_le_bytes()); // version
        // flags: bit0 = records are COMPRESSION-encoded.
        let flags: u16 = if self.compressing { 1 } else { 0 };
        out.extend_from_slice(&flags.to_le_bytes());
        let (rf, fixed, minl, maxl) = match info.record_format {
            RecordFormat::Fixed { length } => (1u8, length, length, length),
            RecordFormat::Variable { min_length, max_length } => (2u8, 0, min_length, max_length),
        };
        out.push(rf);
        out.push(0u8); // reserved
        out.extend_from_slice(&fixed.to_le_bytes());
        out.extend_from_slice(&minl.to_le_bytes());
        out.extend_from_slice(&maxl.to_le_bytes());
        out.extend_from_slice(&info.key_count.to_le_bytes());
        let now = now_ms();
        let created = if self.created_ms != 0 { self.created_ms } else { now };
        out.extend_from_slice(&created.to_le_bytes());
        out.extend_from_slice(&now.to_le_bytes());

        let mut keys = vec![info.primary.clone()];
        keys.extend(info.alternates.iter().cloned());
        for k in &keys {
            out.extend_from_slice(&k.key_number.to_le_bytes());
            out.push(k.duplicates_allowed as u8);
            out.push(match k.ordering { KeyOrdering::Descending => 1, KeyOrdering::Ascending => 0 });
            out.extend_from_slice(&(k.parts.len() as u16).to_le_bytes());
            let name = k.name.clone().unwrap_or_default();
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            for p in &k.parts {
                out.extend_from_slice(&p.offset.to_le_bytes());
                out.extend_from_slice(&p.length.to_le_bytes());
                out.push(p.encoding.to_u8());
                out.push(0u8); // reserved
            }
        }

        out.extend_from_slice(&(self.records.len() as u64).to_le_bytes());
        for rec in self.records.values() {
            let stored = if self.compressing { crate::compress::compress(rec) } else { rec.clone() };
            out.extend_from_slice(&(stored.len() as u32).to_le_bytes());
            out.extend_from_slice(&stored);
        }

        let crc = crc32(&out);
        out.extend_from_slice(&crc.to_le_bytes());
        std::fs::write(&self.path, out)
    }

    /// Read just the schema of an indexed file without opening it for I/O — the
    /// `cobfa_indexinfo()` analog for tooling / a future Fujitsu importer.
    /// Returns `None` for legacy/unknown containers (no embedded schema).
    pub fn inspect_path(path: impl AsRef<Path>) -> std::io::Result<Option<IndexedFileInfo>> {
        let data = std::fs::read(path.as_ref())?;
        if !(data.len() >= 8 && &data[0..8] == b"PRCIDX1\0") {
            return Ok(None);
        }
        // Reuse the parser with a throwaway engine (single dummy key spec).
        let mut probe = IndexedFile::new(
            path.as_ref(),
            0,
            KeySpec { offset: 0, len: 0, duplicates: false },
            Vec::new(),
        );
        probe.load_prcidx(&data).map(Some)
    }
}

/// Little-endian byte cursor for parsing the `PRCIDX1` container.
struct Cur<'a> {
    d: &'a [u8],
    i: usize,
}

fn trunc() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "truncated prcidx container")
}

impl<'a> Cur<'a> {
    fn bytes(&mut self, n: usize) -> std::io::Result<&'a [u8]> {
        if self.i + n > self.d.len() {
            return Err(trunc());
        }
        let s = &self.d[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }
    fn u8(&mut self) -> std::io::Result<u8> {
        Ok(self.bytes(1)?[0])
    }
    fn u16(&mut self) -> std::io::Result<u16> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> std::io::Result<u32> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u64(&mut self) -> std::io::Result<u64> {
        let b = self.bytes(8)?;
        Ok(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }
}

/// CRC-32 (IEEE 802.3, reflected) — self-contained, no external crate.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Current wall-clock time in milliseconds since the Unix epoch (0 on failure).
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// Bridge the in-memory engine to the shared backend trait. Inherent methods of
// the same name take resolution priority, so each body calls the inherent one.
impl IndexedStore for IndexedFile {
    fn open(&mut self, mode: OpenMode) -> &'static str { self.open(mode) }
    fn close(&mut self) -> &'static str { self.close() }
    fn write(&mut self, rec: &[u8]) -> &'static str { self.write(rec) }
    fn read_key(&mut self, key: &[u8]) -> (Option<Bytes>, &'static str) { self.read_key(key) }
    fn read_seq(&mut self, dir: ReadDir) -> (Option<Bytes>, &'static str) { self.read_seq(dir) }
    fn start(&mut self, op: StartOp, key: &[u8]) -> &'static str { self.start(op, key) }
    fn rewrite(&mut self, rec: &[u8], random_key: Option<&[u8]>) -> &'static str {
        self.rewrite(rec, random_key)
    }
    fn delete(&mut self, random_key: Option<&[u8]>) -> &'static str { self.delete(random_key) }
    fn set_key_of_reference(&mut self, kor: usize) { self.set_key_of_reference(kor) }
    fn is_open(&self) -> bool { self.is_open() }
    fn unlock(&mut self) { self.locks.clear(); }
    fn commit(&mut self) { self.commit() }
    fn rollback(&mut self) { self.rollback() }
}

fn pad(key: &[u8], len: usize) -> Bytes {
    let mut k = key.to_vec();
    k.resize(len, b' ');
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("isam-{}-{}.dat", std::process::id(), name))
    }

    /// Record: 5-byte ID (primary) + 10-byte NAME (alt, no dup) = 15 bytes.
    fn rec(id: &str, name: &str) -> Bytes {
        let mut r = format!("{id:0>5}{name:<10}").into_bytes();
        r.truncate(15);
        r.resize(15, b' ');
        r
    }
    fn newfile(p: PathBuf, dup: bool) -> IndexedFile {
        IndexedFile::new(
            p, 15,
            KeySpec { offset: 0, len: 5, duplicates: false },
            vec![KeySpec { offset: 5, len: 10, duplicates: dup }],
        )
    }

    #[test]
    fn write_read_random_and_duplicate() {
        let p = tmp("wr"); let _ = std::fs::remove_file(&p);
        let mut f = newfile(p, false);
        assert_eq!(f.open(OpenMode::Output), status::OK);
        assert_eq!(f.write(&rec("1", "ALICE")), status::OK);
        assert_eq!(f.write(&rec("2", "BOB")), status::OK);
        assert_eq!(f.write(&rec("1", "EVE")), status::DUP_KEY); // dup primary
        assert_eq!(f.close(), status::OK);

        let mut f = newfile(f.path.clone(), false);
        assert_eq!(f.open(OpenMode::Input), status::OK);
        let (r, s) = f.read_key(b"00002");
        assert_eq!(s, status::OK);
        assert_eq!(&r.unwrap()[5..8], b"BOB");
        let (_, s) = f.read_key(b"00009");
        assert_eq!(s, status::NOT_FOUND);
    }

    #[test]
    fn sequential_next_previous_and_start() {
        let p = tmp("seq"); let _ = std::fs::remove_file(&p);
        let mut f = newfile(p, false);
        f.open(OpenMode::Output);
        for (id, nm) in [("3", "C"), ("1", "A"), ("2", "B")] { f.write(&rec(id, nm)); }
        f.close();

        let mut f = newfile(f.path.clone(), false);
        f.open(OpenMode::Input);
        // READ NEXT returns ascending primary order.
        let ids: Vec<String> = (0..3).map(|_| {
            let (r, _) = f.read_seq(ReadDir::Next);
            String::from_utf8_lossy(&r.unwrap()[0..5]).into_owned()
        }).collect();
        assert_eq!(ids, ["00001", "00002", "00003"]);
        let (_, s) = f.read_seq(ReadDir::Next);
        assert_eq!(s, status::EOF);

        // START ≥ "00002" then READ NEXT yields 00002.
        assert_eq!(f.start(StartOp::Ge, b"00002"), status::OK);
        let (r, _) = f.read_seq(ReadDir::Next);
        assert_eq!(&r.unwrap()[0..5], b"00002");
        // READ PREVIOUS from 00002 yields 00001.
        let (r, _) = f.read_seq(ReadDir::Previous);
        assert_eq!(&r.unwrap()[0..5], b"00001");
    }

    #[test]
    fn rewrite_delete_under_io() {
        let p = tmp("io"); let _ = std::fs::remove_file(&p);
        let mut f = newfile(p, false);
        f.open(OpenMode::Output);
        f.write(&rec("1", "ALICE"));
        f.write(&rec("2", "BOB"));
        f.close();

        let mut f = newfile(f.path.clone(), false);
        assert_eq!(f.open(OpenMode::Io), status::OK);
        // REWRITE without a prior READ (sequential) → no current record.
        assert_eq!(f.rewrite(&rec("1", "ZZZ"), None), status::NO_NEXT);
        // READ establishes current + exclusive lock, then REWRITE.
        let (_, s) = f.read_key(b"00001");
        assert_eq!(s, status::OK);
        assert_eq!(f.rewrite(&rec("1", "ALICE2"), None), status::OK);
        // DELETE by random key.
        assert_eq!(f.delete(Some(b"00002")), status::OK);
        f.close();

        let mut f = newfile(f.path.clone(), false);
        f.open(OpenMode::Input);
        let (r, s) = f.read_key(b"00001");
        assert_eq!(s, status::OK);
        assert_eq!(&r.unwrap()[5..11], b"ALICE2");
        assert_eq!(f.read_key(b"00002").1, status::NOT_FOUND); // deleted
    }

    #[test]
    fn alternate_key_no_duplicates_and_with_duplicates() {
        // no-dup alt rejects a second record with the same NAME.
        let p = tmp("altnodup"); let _ = std::fs::remove_file(&p);
        let mut f = newfile(p, false);
        f.open(OpenMode::Output);
        assert_eq!(f.write(&rec("1", "ACME")), status::OK);
        assert_eq!(f.write(&rec("2", "ACME")), status::DUP_KEY);
        f.close();

        // with-dup alt allows it (a duplicate is a successful 00 write), and
        // read by alt finds one.
        let p = tmp("altdup"); let _ = std::fs::remove_file(&p);
        let mut f = newfile(p, true);
        f.open(OpenMode::Output);
        assert_eq!(f.write(&rec("1", "ACME")), status::OK);
        assert_eq!(f.write(&rec("2", "ACME")), status::OK);
        f.close();

        // Reopen for input and read by the alternate (NAME) key of reference.
        let mut f = newfile(f.path.clone(), true);
        f.open(OpenMode::Input);
        f.set_key_of_reference(1); // read by NAME
        let (r, s) = f.read_key(b"ACME");
        assert_eq!(s, status::OK);
        assert!(r.is_some());
        f.close();
    }

    #[test]
    fn commit_and_rollback() {
        let p = tmp("tx"); let _ = std::fs::remove_file(&p);
        let mut f = newfile(p, false);
        f.open(OpenMode::Output);
        f.write(&rec("1", "ALICE"));
        f.close();

        // Transaction under I-O: changes since last commit are undone by ROLLBACK.
        let mut f = newfile(f.path.clone(), false);
        assert_eq!(f.open(OpenMode::Io), status::OK);
        f.write(&rec("2", "BOB"));
        f.read_key(b"00001");
        f.rewrite(&rec("1", "CHANGED"), None);
        f.rollback();
        // 00002 gone, 00001 restored to ALICE.
        assert_eq!(f.read_key(b"00002").1, status::NOT_FOUND);
        let (r, _) = f.read_key(b"00001");
        assert_eq!(&r.unwrap()[5..10], b"ALICE");
    }

    #[test]
    fn prcidx_schema_round_trips_and_inspect_discovers_it() {
        // Write a file with a named primary + a WITH DUPLICATES alternate, then
        // discover its schema from disk (the cobfa_indexinfo analog).
        let p = tmp("schema"); let _ = std::fs::remove_file(&p);
        let mut f = newfile(p.clone(), true);
        f.set_key_names(vec![Some("CUST-ID".into()), Some("CUST-NAME".into())]);
        f.open(OpenMode::Output);
        f.write(&rec("1", "ALICE"));
        f.write(&rec("2", "BOB"));
        f.close();

        let info = IndexedFile::inspect_path(&p).unwrap().expect("PRCIDX1 schema");
        assert_eq!(info.record_format, RecordFormat::Fixed { length: 15 });
        assert_eq!(info.key_count, 2);
        assert_eq!(info.total_key_length, 15); // 5 + 10
        // Primary: number 1, offset 0 len 5, unique, ascending, named.
        assert_eq!(info.primary.key_number, 1);
        assert_eq!(info.primary.name.as_deref(), Some("CUST-ID"));
        assert_eq!(info.primary.parts.len(), 1);
        assert_eq!((info.primary.parts[0].offset, info.primary.parts[0].length), (0, 5));
        assert!(!info.primary.duplicates_allowed);
        assert_eq!(info.primary.ordering, KeyOrdering::Ascending);
        // Alternate: number 2, offset 5 len 10, duplicates allowed, named.
        assert_eq!(info.alternates.len(), 1);
        assert_eq!(info.alternates[0].key_number, 2);
        assert_eq!(info.alternates[0].name.as_deref(), Some("CUST-NAME"));
        assert_eq!((info.alternates[0].parts[0].offset, info.alternates[0].parts[0].length), (5, 10));
        assert!(info.alternates[0].duplicates_allowed);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn open_input_missing_file_is_status_35() {
        let p = tmp("missing"); let _ = std::fs::remove_file(&p);
        let mut f = newfile(p, false);
        assert_eq!(f.open(OpenMode::Input), status::FILE_NOT_FOUND);
    }

    #[test]
    fn strict_metadata_mismatch_is_status_39() {
        // Create with a 10-byte unique alternate; reopen declaring that same
        // alternate WITH DUPLICATES → attribute mismatch (39).
        let p = tmp("mismatch"); let _ = std::fs::remove_file(&p);
        let mut f = newfile(p.clone(), false);
        f.open(OpenMode::Output);
        f.write(&rec("1", "ALICE"));
        f.close();

        // Declared alt now allows duplicates → schema differs.
        let mut g = newfile(p.clone(), true);
        assert_eq!(g.open(OpenMode::Input), status::ATTR_MISMATCH);

        // Same declaration as on disk → opens fine.
        let mut h = newfile(p.clone(), false);
        assert_eq!(h.open(OpenMode::Input), status::OK);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn legacy_prcisam1_container_still_loads() {
        // Hand-build a legacy container (magic + record_len + records) and prove
        // it loads (no schema → strict validation is skipped).
        let p = tmp("legacy"); let _ = std::fs::remove_file(&p);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PRCISAM1");
        bytes.extend_from_slice(&15u32.to_le_bytes());
        for r in [rec("1", "ALICE"), rec("2", "BOB")] {
            bytes.extend_from_slice(&(r.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&r);
        }
        std::fs::write(&p, &bytes).unwrap();

        let mut f = newfile(p.clone(), false);
        assert_eq!(f.open(OpenMode::Input), status::OK);
        let (r, s) = f.read_key(b"00002");
        assert_eq!(s, status::OK);
        assert_eq!(&r.unwrap()[5..10], b"BOB  ");
        // inspect_path reports None for legacy (no embedded schema).
        assert!(IndexedFile::inspect_path(&p).unwrap().is_none());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn corrupt_prcidx_crc_is_io_error() {
        let p = tmp("corrupt"); let _ = std::fs::remove_file(&p);
        let mut f = newfile(p.clone(), false);
        f.open(OpenMode::Output);
        f.write(&rec("1", "ALICE"));
        f.close();
        // Flip a byte in the middle of the records region.
        let mut bytes = std::fs::read(&p).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xFF;
        std::fs::write(&p, &bytes).unwrap();

        let mut g = newfile(p.clone(), false);
        assert_eq!(g.open(OpenMode::Input), status::IO_ERROR);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn engine_name_parsing_and_aliases() {
        use IndexedEngine as E;
        assert_eq!(IndexedEngine::default(), E::Rust);
        assert_eq!(E::parse("rust"), Some(E::Rust));
        assert_eq!(E::parse("RUST"), Some(E::Rust));
        assert_eq!(E::parse("default"), Some(E::Rust));
        assert_eq!(E::parse("rm-cobol85"), Some(E::RmCobol85));
        assert_eq!(E::parse("RM_COBOL85"), Some(E::RmCobol85));
        assert_eq!(E::parse("rmcobol"), Some(E::RmCobol85));
        assert_eq!(E::parse("fujitsu"), Some(E::Fujitsu));
        assert_eq!(E::parse("Fujitsu COBOL85"), Some(E::Fujitsu));
        assert_eq!(E::parse("bogus"), None);
        assert_eq!(E::Rust.name(), "rust");
        assert_eq!(E::RmCobol85.name(), "rm-cobol85");
        assert_eq!(E::Fujitsu.name(), "fujitsu");
    }
}
