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

/// Status codes (the FILE STATUS two-character values this engine produces).
pub mod status {
    pub const OK: &str = "00";
    pub const DUP_ALT_OK: &str = "02"; // duplicate alternate key, allowed
    pub const EOF: &str = "10";
    pub const DUP_KEY: &str = "22"; // duplicate primary/no-dup alternate on WRITE
    pub const NOT_FOUND: &str = "23"; // record not found / no current record
    pub const BOUNDARY: &str = "24"; // boundary violation
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
}

impl IndexedEngine {
    /// Parse an engine name (case-insensitive). Accepts a few common aliases.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace(['_', ' '], "-").as_str() {
            "rust" | "rstcobol" | "rustcobol" | "native" | "default" => Some(Self::Rust),
            "rm" | "rm-cobol" | "rm-cobol85" | "rmcobol" | "rmcobol85" => Some(Self::RmCobol85),
            "fujitsu" | "fujitsu-cobol" | "fujitsu-cobol85" | "fj" => Some(Self::Fujitsu),
            _ => None,
        }
    }

    /// Canonical lower-case name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::RmCobol85 => "rm-cobol85",
            Self::Fujitsu => "fujitsu",
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

/// A key: a `[offset, offset+len)` slice of the record, optionally allowing
/// duplicate values (alternate keys only).
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
                    if self.load().is_err() {
                        return status::IO_ERROR;
                    }
                } else if mode == OpenMode::Input {
                    // Reading a non-existent file.
                    return status::UNAVAILABLE;
                } else {
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
        // No-duplicates alternates must not already hold this alt value.
        let mut dup_alt_ok = false;
        for (i, ks) in self.alternates.iter().enumerate() {
            let ak = ks.extract(&rec);
            if let Some(set) = self.alt_index[i].get(&ak) {
                if !set.is_empty() {
                    if ks.duplicates {
                        dup_alt_ok = true;
                    } else {
                        return status::DUP_KEY;
                    }
                }
            }
        }
        self.records.insert(pkey.clone(), rec.clone());
        self.index_insert(&pkey, &rec);
        self.journal.push(Journal::Insert(pkey));
        if dup_alt_ok { status::DUP_ALT_OK } else { status::OK }
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

    fn load(&mut self) -> std::io::Result<()> {
        let data = std::fs::read(&self.path)?;
        self.records.clear();
        // Format: magic "PRCISAM1", u32 record_len, then repeated u32 len + bytes.
        let mut i = 0usize;
        let need = |i: usize, n: usize, len: usize| i + n <= len;
        if data.len() < 12 || &data[0..8] != b"PRCISAM1" {
            // Unknown container — treat as empty rather than failing hard.
            self.rebuild_alt_index();
            return Ok(());
        }
        i = 8;
        let _rec_len = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
        i = 12;
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
        self.rebuild_alt_index();
        Ok(())
    }

    fn save(&self) -> std::io::Result<()> {
        let mut out = Vec::new();
        out.extend_from_slice(b"PRCISAM1");
        out.extend_from_slice(&(self.record_len as u32).to_le_bytes());
        for rec in self.records.values() {
            out.extend_from_slice(&(rec.len() as u32).to_le_bytes());
            out.extend_from_slice(rec);
        }
        std::fs::write(&self.path, out)
    }
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

        // with-dup alt allows it (status 02), and read by alt finds one.
        let p = tmp("altdup"); let _ = std::fs::remove_file(&p);
        let mut f = newfile(p, true);
        f.open(OpenMode::Output);
        assert_eq!(f.write(&rec("1", "ACME")), status::OK);
        assert_eq!(f.write(&rec("2", "ACME")), status::DUP_ALT_OK);
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
