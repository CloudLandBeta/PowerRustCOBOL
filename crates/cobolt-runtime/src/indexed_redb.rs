// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Crash-safe INDEXED engine on a redb substrate (`STORAGE IS DISK`, opt-in via
//! `--indexed-engine redb`).
//!
//! redb is a pure-Rust embedded ACID key-value store (copy-on-write B+tree, dual
//! meta pages, per-page checksums). This engine maps the COBOL indexed-file model
//! onto it so it satisfies four operational goals the bespoke `PRCIDXD1` engine
//! could not at scale:
//!
//! * **OPEN is O(1)** — redb reads only its meta page; there is no in-RAM record
//!   directory to load and no recovery scan, even after a crash.
//! * **READ RANDOM / NEXT are B+tree / range operations** over redb's page cache.
//! * **Resident RAM is the working set**, not the record count (≥250 M records
//!   with a small cache).
//! * **Crash safety** — `COMMIT` is a durable redb transaction commit; `ROLLBACK`
//!   is a transaction abort. A power loss can never leave a torn index: redb
//!   falls back to the last good commit via its dual meta pages.
//!
//! ## Table layout
//!
//! | redb table | kind             | key → value                              |
//! |------------|------------------|------------------------------------------|
//! | `primary`  | table            | primary-key bytes → (maybe-compressed) record |
//! | `alt`      | multimap         | `[u16 idx][alt-key bytes]` → primary-key bytes |
//! | `meta`     | table            | `schema` / `compress` descriptors        |
//!
//! A single `alt` multimap holds *all* alternate keys, namespaced by a 2-byte
//! big-endian key index prefix, so the byte order is `(key index, alt value,
//! primary key)` — matching the in-memory engine's
//! `BTreeMap<alt, BTreeSet<primary>>` ordering exactly.
//!
//! ## Transaction model
//!
//! Writable opens (`OUTPUT`/`I-O`/`EXTEND`) hold one redb `WriteTransaction` open
//! from OPEN; reads see their own uncommitted writes. `COMMIT` commits and begins
//! a fresh transaction; `ROLLBACK` aborts and begins a fresh one; `CLOSE` commits
//! (implicit commit). `INPUT` uses short read transactions.

use std::ops::Bound::{Excluded, Included, Unbounded};
use std::path::{Path, PathBuf};
use std::time::Instant;

use redb::{
    Database, MultimapTableDefinition, ReadableMultimapTable, ReadableTable, TableDefinition,
    WriteTransaction,
};

use crate::compress;
use crate::indexed::{status, Bytes, IndexedStore, KeySpec, OpenMode, ReadDir, StartOp};
use crate::indexed_log::{now_iso, LogFormat, LogLevel, LogRecord, LogWriter};

type Slice = &'static [u8];
const PRIMARY: TableDefinition<Slice, Slice> = TableDefinition::new("primary");
const ALT: MultimapTableDefinition<Slice, Slice> = MultimapTableDefinition::new("alt");
const SEQ: TableDefinition<Slice, Slice> = TableDefinition::new("seq");
const META: TableDefinition<Slice, Slice> = TableDefinition::new("meta");

const META_SCHEMA: &[u8] = b"schema";
const META_COMPRESS: &[u8] = b"compress";
const META_NEXTSEQ: &[u8] = b"nextseq";

/// Each record carries a stable monotonic insertion sequence (the redb analogue
/// of the PRCIDXD1 RecordId). Alternate-index entries are valued
/// `[seq:u64 BE][primary key]`, so duplicates under one alternate value iterate
/// in **insertion order** — matching the disk engine's RecordId ordering and the
/// COBOL rule that duplicate alternates are read in creation order.
const SEQ_LEN: usize = 8;

/// Open a readable handle to the `primary` table from the held write txn (so
/// uncommitted writes are visible) or a fresh read txn, run `$body`, else `$empty`.
macro_rules! with_primary {
    ($self:ident, $t:ident => $body:expr, $empty:expr) => {{
        if let Some(w) = &$self.wtx {
            match w.open_table(PRIMARY) {
                Ok($t) => $body,
                Err(_) => $empty,
            }
        } else if let Some(db) = &$self.db {
            match db.begin_read().and_then(|r| Ok(r.open_table(PRIMARY))) {
                Ok(Ok($t)) => $body,
                _ => $empty,
            }
        } else {
            $empty
        }
    }};
}

/// Same as [`with_primary`] for the `alt` multimap table.
macro_rules! with_alt {
    ($self:ident, $t:ident => $body:expr, $empty:expr) => {{
        if let Some(w) = &$self.wtx {
            match w.open_multimap_table(ALT) {
                Ok($t) => $body,
                Err(_) => $empty,
            }
        } else if let Some(db) = &$self.db {
            match db.begin_read().and_then(|r| Ok(r.open_multimap_table(ALT))) {
                Ok(Ok($t)) => $body,
                _ => $empty,
            }
        } else {
            $empty
        }
    }};
}

/// The redb-backed indexed file.
pub struct RedbIndexedFile {
    path: PathBuf,
    record_len: usize,
    primary: KeySpec,
    alternates: Vec<KeySpec>,
    #[allow(dead_code)]
    key_names: Vec<Option<String>>,
    strict_metadata: bool,
    compressing: bool,

    db: Option<Database>,
    wtx: Option<WriteTransaction>,
    open: Option<OpenMode>,
    kor: usize,
    /// Current sequential position as `(key-of-reference value, primary key)`.
    cursor: Option<(Bytes, Bytes)>,
    /// Pending START position, consumed by the next sequential READ.
    start_at: Option<(Bytes, Bytes)>,
    /// Last successfully read primary key (for REWRITE/DELETE current).
    current: Option<Bytes>,

    // ── Optional observability log (see indexed_log.rs) ──────────────────────
    log_level: LogLevel,
    log_format: LogFormat,
    /// Operator/user from `OPEN … WITH REGISTERED USER`, logged on every event.
    registered_user: Option<String>,
    log: Option<LogWriter>,
    /// Per-transaction accumulators (reset at OPEN and after each COMMIT/ROLLBACK).
    tx_id: u64,
    tx_writes: u64,
    tx_rewrites: u64,
    tx_deletes: u64,
    tx_bytes: u64,
    tx_in_order: u64,
    tx_out_of_order: u64,
    tx_last_key: Option<Bytes>,
    tx_start: Instant,
}

impl RedbIndexedFile {
    pub fn new(
        path: impl AsRef<Path>,
        record_len: usize,
        primary: KeySpec,
        alternates: Vec<KeySpec>,
    ) -> Self {
        RedbIndexedFile {
            path: path.as_ref().to_path_buf(),
            record_len,
            primary,
            alternates,
            key_names: Vec::new(),
            strict_metadata: true,
            compressing: false,
            db: None,
            wtx: None,
            open: None,
            kor: 0,
            cursor: None,
            start_at: None,
            current: None,
            log_level: LogLevel::Off,
            log_format: LogFormat::Text,
            registered_user: None,
            log: None,
            tx_id: 0,
            tx_writes: 0,
            tx_rewrites: 0,
            tx_deletes: 0,
            tx_bytes: 0,
            tx_in_order: 0,
            tx_out_of_order: 0,
            tx_last_key: None,
            tx_start: Instant::now(),
        }
    }

    pub fn set_key_names(&mut self, names: Vec<Option<String>>) {
        self.key_names = names;
    }
    pub fn set_strict_metadata(&mut self, strict: bool) {
        self.strict_metadata = strict;
    }
    pub fn set_compressing(&mut self, on: bool) {
        self.compressing = on;
    }
    /// Enable the per-file transaction log at `<assign-path>.log`.
    pub fn set_log_level(&mut self, level: LogLevel) {
        self.log_level = level;
    }
    /// Choose the log line format (logfmt text or NDJSON).
    pub fn set_log_format(&mut self, format: LogFormat) {
        self.log_format = format;
    }

    // ── Observability log bookkeeping ────────────────────────────────────────

    /// Reset the per-transaction accumulators and start the clock.
    fn tx_reset(&mut self) {
        self.tx_writes = 0;
        self.tx_rewrites = 0;
        self.tx_deletes = 0;
        self.tx_bytes = 0;
        self.tx_in_order = 0;
        self.tx_out_of_order = 0;
        self.tx_last_key = None;
        self.tx_start = Instant::now();
    }

    /// Record one WRITE for the log (count, bytes, and key-ordering quality).
    fn note_write(&mut self, pkey: &[u8], bytes: usize) {
        if !self.log_level.is_on() {
            return;
        }
        self.tx_writes += 1;
        self.tx_bytes += bytes as u64;
        match &self.tx_last_key {
            Some(last) if pkey <= last.as_slice() => self.tx_out_of_order += 1,
            _ => self.tx_in_order += 1,
        }
        self.tx_last_key = Some(pkey.to_vec());
    }

    /// Redb index statistics from the live write transaction, as numeric fields
    /// (used for `full`-level CLOSE lines). Walks the index — cost scales with
    /// file size — so it is only populated at CLOSE under `LogLevel::Full`.
    fn full_stats_fields(&self) -> Vec<(&'static str, u64)> {
        if self.log_level != LogLevel::Full {
            return Vec::new();
        }
        let Some(Ok(s)) = self.wtx.as_ref().map(|w| w.stats()) else {
            return Vec::new();
        };
        vec![
            ("tree_height", s.tree_height() as u64),
            ("allocated_pages", s.allocated_pages()),
            ("leaf_pages", s.leaf_pages()),
            ("branch_pages", s.branch_pages()),
            ("stored_bytes", s.stored_bytes()),
            ("fragmented_bytes", s.fragmented_bytes()),
            ("page_size", s.page_size() as u64),
        ]
    }

    /// Emit one log line for a transaction event and reset the accumulators.
    /// `extra` carries optional pre-computed numeric fields (e.g. CLOSE stats).
    fn log_event(&mut self, kind: &str, extra: &[(&'static str, u64)]) {
        if !self.log_level.is_on() {
            return;
        }
        self.tx_id += 1;
        let dur = self.tx_start.elapsed();
        let records = self.tx_writes + self.tx_rewrites + self.tx_deletes;
        let secs = dur.as_secs_f64().max(1e-9);
        let order = if self.tx_writes == 0 {
            "n/a"
        } else if self.tx_out_of_order == 0 {
            "ordered"
        } else {
            "unordered"
        };
        let fname = self
            .path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        let mut rec = LogRecord::new();
        rec.str("ts", now_iso()).str("file", fname);
        if let Some(user) = &self.registered_user {
            rec.str("user", user.clone());
        }
        rec.num("tx", self.tx_id)
            .str("kind", kind)
            .num("writes", self.tx_writes)
            .num("rewrites", self.tx_rewrites)
            .num("deletes", self.tx_deletes)
            .num("records", records)
            .num("bytes", self.tx_bytes)
            .num("dur_ms", dur.as_millis() as u64)
            .num("rec_per_s", (records as f64 / secs) as u64)
            .num("bytes_per_s", (self.tx_bytes as f64 / secs) as u64)
            .str("order", order)
            .num("in_order", self.tx_in_order)
            .num("out_of_order", self.tx_out_of_order);
        for (k, v) in extra {
            rec.num(k, *v);
        }

        let line = rec.render(self.log_format);
        if let Some(log) = self.log.as_mut() {
            log.line(&line);
        }
        self.tx_reset();
    }

    // ── small helpers ────────────────────────────────────────────────────────

    fn fit(&self, rec: &[u8]) -> Bytes {
        let mut r = rec.to_vec();
        r.resize(self.record_len, b' ');
        r
    }

    fn encode_value(&self, rec: &[u8]) -> Bytes {
        if self.compressing {
            compress::compress(rec)
        } else {
            rec.to_vec()
        }
    }

    fn decode_value(&self, stored: &[u8]) -> Bytes {
        let mut v = if self.compressing {
            compress::decompress(stored)
        } else {
            stored.to_vec()
        };
        v.resize(self.record_len, b' ');
        v
    }

    /// The key-of-reference value for a record (primary key when KOR = 0).
    fn ref_value(&self, pkey: &[u8], rec: &[u8]) -> Bytes {
        if self.kor == 0 {
            pkey.to_vec()
        } else {
            extract(&self.alternates[self.kor - 1], rec)
        }
    }

    fn schema_blob(&self) -> Bytes {
        let mut b = Vec::new();
        b.extend_from_slice(&(self.record_len as u32).to_le_bytes());
        b.extend_from_slice(&((1 + self.alternates.len()) as u16).to_le_bytes());
        for k in std::iter::once(&self.primary).chain(self.alternates.iter()) {
            b.extend_from_slice(&(k.offset as u32).to_le_bytes());
            b.extend_from_slice(&(k.len as u32).to_le_bytes());
            b.push(k.duplicates as u8);
        }
        b
    }

    // ── redb read primitives (work in either read or write txn) ──────────────

    fn lookup_primary(&self, pk: &[u8]) -> Option<Bytes> {
        with_primary!(self, t => {
            t.get(pk).ok().flatten().map(|g| self.decode_value(g.value()))
        }, None)
    }

    /// Primary keys stored under one alternate composite key, in **insertion
    /// order** (the multimap values are `[seq][pkey]`, so ascending byte order is
    /// ascending seq). The `[seq]` prefix is stripped from each returned key.
    fn alt_values(&self, comp: &[u8]) -> Vec<Bytes> {
        with_alt!(self, mt => {
            let mut out = Vec::new();
            if let Ok(vals) = mt.get(comp) {
                for v in vals.flatten() {
                    let raw = v.value();
                    out.push(raw.get(SEQ_LEN..).unwrap_or(&[]).to_vec());
                }
            }
            out
        }, Vec::new())
    }

    /// The stored insertion sequence for a primary key, if present.
    fn seq_of(&self, pkey: &[u8]) -> Option<u64> {
        fn parse(b: &[u8]) -> Option<u64> {
            b.get(..SEQ_LEN).map(|s| {
                let mut a = [0u8; SEQ_LEN];
                a.copy_from_slice(s);
                u64::from_be_bytes(a)
            })
        }
        if let Some(w) = &self.wtx {
            let t = w.open_table(SEQ).ok()?;
            let g = t.get(pkey).ok().flatten()?;
            parse(g.value())
        } else if let Some(db) = &self.db {
            let r = db.begin_read().ok()?;
            let t = r.open_table(SEQ).ok()?;
            let g = t.get(pkey).ok().flatten()?;
            parse(g.value())
        } else {
            None
        }
    }

    /// The alternate-index multimap value for `(seq, pkey)`: `[seq BE][pkey]`.
    fn alt_value(seq: u64, pkey: &[u8]) -> Bytes {
        let mut v = seq.to_be_bytes().to_vec();
        v.extend_from_slice(pkey);
        v
    }

    /// First (or last) composite alt key in index `idx`'s namespace.
    fn alt_edge_composite(&self, idx: usize, dir: ReadDir) -> Option<Bytes> {
        let (lo, hi) = prefix_bounds(idx);
        with_alt!(self, mt => {
            let mut r = match mt.range::<&[u8]>((Included(lo.as_slice()), Excluded(hi.as_slice()))) {
                Ok(r) => r,
                Err(_) => return None,
            };
            let item = match dir { ReadDir::Next => r.next(), ReadDir::Previous => r.next_back() };
            item.and_then(|x| x.ok()).map(|(k, _)| k.value().to_vec())
        }, None)
    }

    /// Next composite alt key strictly after `comp` within `idx` (or before, for Previous).
    fn alt_step_composite(&self, idx: usize, comp: &[u8], dir: ReadDir) -> Option<Bytes> {
        let (lo, hi) = prefix_bounds(idx);
        with_alt!(self, mt => {
            match dir {
                ReadDir::Next => {
                    let mut r = mt.range::<&[u8]>((Excluded(comp), Excluded(hi.as_slice()))).ok()?;
                    r.next().and_then(|x| x.ok()).map(|(k, _)| k.value().to_vec())
                }
                ReadDir::Previous => {
                    let mut r = mt.range::<&[u8]>((Included(lo.as_slice()), Excluded(comp))).ok()?;
                    r.next_back().and_then(|x| x.ok()).map(|(k, _)| k.value().to_vec())
                }
            }
        }, None)
    }

    /// First/last `(key, raw value)` of the primary table. Returning the value
    /// here lets a primary-key `READ NEXT` skip a second lookup (one descent).
    fn primary_edge(&self, dir: ReadDir) -> Option<(Bytes, Bytes)> {
        with_primary!(self, t => {
            let mut r = t.range::<&[u8]>(..).ok()?;
            let item = match dir { ReadDir::Next => r.next(), ReadDir::Previous => r.next_back() };
            item.and_then(|x| x.ok()).map(|(k, v)| (k.value().to_vec(), v.value().to_vec()))
        }, None)
    }

    fn primary_step(&self, pk: &[u8], dir: ReadDir) -> Option<(Bytes, Bytes)> {
        with_primary!(self, t => {
            match dir {
                ReadDir::Next => {
                    let mut r = t.range::<&[u8]>((Excluded(pk), Unbounded)).ok()?;
                    r.next().and_then(|x| x.ok()).map(|(k, v)| (k.value().to_vec(), v.value().to_vec()))
                }
                ReadDir::Previous => {
                    let mut r = t.range::<&[u8]>((Unbounded, Excluded(pk))).ok()?;
                    r.next_back().and_then(|x| x.ok()).map(|(k, v)| (k.value().to_vec(), v.value().to_vec()))
                }
            }
        }, None)
    }

    fn primary_bound(&self, op: StartOp, key: &[u8]) -> Option<Bytes> {
        with_primary!(self, t => {
            let res = match op {
                StartOp::Eq => return t.get(key).ok().flatten().map(|_| key.to_vec()),
                StartOp::Ge => t.range::<&[u8]>((Included(key), Unbounded)).ok()?.next(),
                StartOp::Gt => t.range::<&[u8]>((Excluded(key), Unbounded)).ok()?.next(),
                StartOp::Le => t.range::<&[u8]>((Unbounded, Included(key))).ok()?.next_back(),
                StartOp::Lt => t.range::<&[u8]>((Unbounded, Excluded(key))).ok()?.next_back(),
            };
            res.and_then(|x| x.ok()).map(|(k, _)| k.value().to_vec())
        }, None)
    }

    /// The composite alt key in index `idx` matching `op` against `key`.
    fn alt_bound_composite(&self, idx: usize, op: StartOp, key: &[u8]) -> Option<Bytes> {
        let comp = composite(idx, key);
        let (lo, hi) = prefix_bounds(idx);
        with_alt!(self, mt => {
            let res = match op {
                StartOp::Eq => {
                    let has = mt.get(comp.as_slice()).map(|mut it| it.next().is_some()).unwrap_or(false);
                    return if has { Some(comp.clone()) } else { None };
                }
                StartOp::Ge => mt.range::<&[u8]>((Included(comp.as_slice()), Excluded(hi.as_slice()))).ok()?.next(),
                StartOp::Gt => mt.range::<&[u8]>((Excluded(comp.as_slice()), Excluded(hi.as_slice()))).ok()?.next(),
                StartOp::Le => mt.range::<&[u8]>((Included(lo.as_slice()), Included(comp.as_slice()))).ok()?.next_back(),
                StartOp::Lt => mt.range::<&[u8]>((Included(lo.as_slice()), Excluded(comp.as_slice()))).ok()?.next_back(),
            };
            res.and_then(|x| x.ok()).map(|(k, _)| k.value().to_vec())
        }, None)
    }

    // ── key-of-reference resolution / stepping ───────────────────────────────

    fn resolve_primary(&self, key: &[u8]) -> Option<Bytes> {
        if self.kor == 0 {
            let k = pad(key, self.primary.len);
            self.lookup_primary(&k).map(|_| k)
        } else {
            let idx = self.kor - 1;
            let k = pad(key, self.alternates[idx].len);
            self.alt_values(&composite(idx, &k)).into_iter().next()
        }
    }

    /// First record in key-of-reference order → `(ref value, primary key, record?)`.
    /// The record is returned directly for the primary key of reference (the
    /// range cursor already yields the value), so the caller skips a lookup.
    fn first_in_kor(&self, dir: ReadDir) -> Option<(Bytes, Bytes, Option<Bytes>)> {
        if self.kor == 0 {
            self.primary_edge(dir).map(|(k, v)| (k.clone(), k, Some(self.decode_value(&v))))
        } else {
            let idx = self.kor - 1;
            let comp = self.alt_edge_composite(idx, dir)?;
            let rv = comp[2..].to_vec();
            let vals = self.alt_values(&comp);
            let pk = match dir {
                ReadDir::Next => vals.first().cloned(),
                ReadDir::Previous => vals.last().cloned(),
            }?;
            Some((rv, pk, None))
        }
    }

    /// Successor / predecessor of `(rv, pk)` in key-of-reference order, with the
    /// record bytes when stepping by the primary key of reference.
    fn step_kor(&self, rv: &[u8], pk: &[u8], dir: ReadDir) -> Option<(Bytes, Bytes, Option<Bytes>)> {
        if self.kor == 0 {
            return self.primary_step(pk, dir).map(|(k, v)| (k.clone(), k, Some(self.decode_value(&v))));
        }
        let idx = self.kor - 1;
        let comp = composite(idx, rv);
        let vals = self.alt_values(&comp); // insertion order
        // Same alternate value: the adjacent duplicate in insertion order.
        if let Some(pos) = vals.iter().position(|v| v.as_slice() == pk) {
            let same = match dir {
                ReadDir::Next => vals.get(pos + 1).cloned(),
                ReadDir::Previous => pos.checked_sub(1).and_then(|i| vals.get(i)).cloned(),
            };
            if let Some(p) = same {
                return Some((rv.to_vec(), p, None));
            }
        }
        // Move to the adjacent alternate value.
        let comp2 = self.alt_step_composite(idx, &comp, dir)?;
        let rv2 = comp2[2..].to_vec();
        let vals2 = self.alt_values(&comp2);
        let p = match dir {
            ReadDir::Next => vals2.first().cloned(),
            ReadDir::Previous => vals2.last().cloned(),
        }?;
        Some((rv2, p, None))
    }

    fn find_start(&self, op: StartOp, key: &[u8]) -> Option<(Bytes, Bytes)> {
        if self.kor == 0 {
            let key = pad(key, self.primary.len);
            self.primary_bound(op, &key).map(|pk| (pk.clone(), pk))
        } else {
            let idx = self.kor - 1;
            let key = pad(key, self.alternates[idx].len);
            let comp = self.alt_bound_composite(idx, op, &key)?;
            let rv = comp[2..].to_vec();
            let pk = self.alt_values(&comp).into_iter().next()?;
            Some((rv, pk))
        }
    }

    // ── OPEN helpers ─────────────────────────────────────────────────────────

    /// Read the stored schema/compress descriptors from META (write or read txn).
    fn read_meta(&self) -> Option<(Bytes, bool)> {
        let read = |t: &dyn ReadMeta| -> Option<(Bytes, bool)> {
            let schema = t.meta_get(META_SCHEMA)?;
            let comp = t.meta_get(META_COMPRESS).map(|v| v.first() == Some(&1)).unwrap_or(false);
            Some((schema, comp))
        };
        if let Some(w) = &self.wtx {
            let t = w.open_table(META).ok()?;
            read(&t)
        } else if let Some(db) = &self.db {
            let r = db.begin_read().ok()?;
            let t = r.open_table(META).ok()?;
            read(&t)
        } else {
            None
        }
    }

    fn init_tables_and_meta(&self) -> Result<(), ()> {
        let w = self.wtx.as_ref().ok_or(())?;
        {
            let _ = w.open_table(PRIMARY).map_err(|_| ())?;
        }
        {
            let _ = w.open_multimap_table(ALT).map_err(|_| ())?;
        }
        {
            let _ = w.open_table(SEQ).map_err(|_| ())?;
        }
        {
            let mut m = w.open_table(META).map_err(|_| ())?;
            let blob = self.schema_blob();
            m.insert(META_SCHEMA, blob.as_slice()).map_err(|_| ())?;
            m.insert(META_COMPRESS, [self.compressing as u8].as_slice()).map_err(|_| ())?;
            m.insert(META_NEXTSEQ, 0u64.to_be_bytes().as_slice()).map_err(|_| ())?;
        }
        Ok(())
    }

    /// Allocate the next monotonic insertion sequence (persisted in META).
    fn alloc_seq(&self) -> Result<u64, ()> {
        let w = self.wtx.as_ref().ok_or(())?;
        let mut m = w.open_table(META).map_err(|_| ())?;
        let cur = m
            .get(META_NEXTSEQ)
            .map_err(|_| ())?
            .and_then(|g| {
                let v = g.value();
                v.get(..SEQ_LEN).map(|b| {
                    let mut a = [0u8; SEQ_LEN];
                    a.copy_from_slice(b);
                    u64::from_be_bytes(a)
                })
            })
            .unwrap_or(0);
        m.insert(META_NEXTSEQ, (cur + 1).to_be_bytes().as_slice()).map_err(|_| ())?;
        Ok(cur)
    }

    fn open_db(&self) -> Result<Database, ()> {
        Database::create(&self.path).map_err(|_| ())
    }
}

/// Tiny abstraction so `read_meta` can share code across table flavors.
trait ReadMeta {
    fn meta_get(&self, key: &[u8]) -> Option<Bytes>;
}
impl<T: ReadableTable<Slice, Slice>> ReadMeta for T {
    fn meta_get(&self, key: &[u8]) -> Option<Bytes> {
        self.get(key).ok().flatten().map(|g| g.value().to_vec())
    }
}

// ── free helpers ─────────────────────────────────────────────────────────────

fn extract(spec: &KeySpec, rec: &[u8]) -> Bytes {
    let end = (spec.offset + spec.len).min(rec.len());
    let start = spec.offset.min(rec.len());
    let mut k = rec[start..end].to_vec();
    k.resize(spec.len, b' ');
    k
}

fn pad(key: &[u8], len: usize) -> Bytes {
    let mut k = key.to_vec();
    k.resize(len, b' ');
    k
}

fn idx_prefix(idx: usize) -> [u8; 2] {
    (idx as u16).to_be_bytes()
}

fn composite(idx: usize, val: &[u8]) -> Bytes {
    let mut c = idx_prefix(idx).to_vec();
    c.extend_from_slice(val);
    c
}

/// `[lo, hi)` byte bounds covering all composite keys of alternate index `idx`.
fn prefix_bounds(idx: usize) -> (Bytes, Bytes) {
    let lo = idx_prefix(idx).to_vec();
    let hi = ((idx as u32) + 1).to_be_bytes()[2..].to_vec(); // 2-byte BE of idx+1
    (lo, hi)
}

// ── IndexedStore impl ────────────────────────────────────────────────────────

impl IndexedStore for RedbIndexedFile {
    fn open(&mut self, mode: OpenMode) -> &'static str {
        if self.open.is_some() {
            return status::LOGIC_ERROR;
        }
        let exists = self.path.exists();
        match mode {
            OpenMode::Output => {
                let _ = std::fs::remove_file(&self.path);
                let db = match self.open_db() {
                    Ok(d) => d,
                    Err(()) => return status::IO_ERROR,
                };
                self.db = Some(db);
                self.wtx = match self.db.as_ref().unwrap().begin_write() {
                    Ok(w) => Some(w),
                    Err(_) => return status::IO_ERROR,
                };
                if self.init_tables_and_meta().is_err() {
                    return status::IO_ERROR;
                }
            }
            OpenMode::Input => {
                if !exists {
                    return status::FILE_NOT_FOUND;
                }
                let db = match self.open_db() {
                    Ok(d) => d,
                    Err(()) => return status::IO_ERROR,
                };
                self.db = Some(db);
                self.wtx = None;
                if let Some((schema, comp)) = self.read_meta() {
                    self.compressing = comp;
                    if self.strict_metadata && schema != self.schema_blob() {
                        self.db = None;
                        return status::ATTR_MISMATCH;
                    }
                }
            }
            OpenMode::Io | OpenMode::Extend => {
                let db = match self.open_db() {
                    Ok(d) => d,
                    Err(()) => return status::IO_ERROR,
                };
                self.db = Some(db);
                self.wtx = match self.db.as_ref().unwrap().begin_write() {
                    Ok(w) => Some(w),
                    Err(_) => return status::IO_ERROR,
                };
                if exists {
                    if let Some((schema, comp)) = self.read_meta() {
                        self.compressing = comp;
                        if self.strict_metadata && schema != self.schema_blob() {
                            self.wtx = None;
                            self.db = None;
                            return status::ATTR_MISMATCH;
                        }
                    } else if self.init_tables_and_meta().is_err() {
                        return status::IO_ERROR;
                    }
                } else if self.init_tables_and_meta().is_err() {
                    return status::IO_ERROR;
                }
            }
        }
        self.open = Some(mode);
        self.kor = 0;
        self.cursor = None;
        self.start_at = None;
        self.current = None;
        if self.log_level.is_on() {
            self.log = Some(LogWriter::new(&self.path, self.log_level));
            self.tx_reset();
            self.log_event("OPEN", &[]);
        }
        status::OK
    }

    fn close(&mut self) -> &'static str {
        if self.open.is_none() {
            return status::LOGIC_ERROR;
        }
        // `full`-level index stats must be read from the live write transaction
        // before it is consumed by commit.
        let stats = self.full_stats_fields();
        let mut st = status::OK;
        if let Some(wtx) = self.wtx.take() {
            if wtx.commit().is_err() {
                st = status::IO_ERROR;
            }
        }
        self.log_event("CLOSE", &stats);
        self.log = None;
        self.db = None;
        self.open = None;
        self.cursor = None;
        self.start_at = None;
        self.current = None;
        st
    }

    fn write(&mut self, rec: &[u8]) -> &'static str {
        if !matches!(self.open, Some(OpenMode::Output | OpenMode::Io | OpenMode::Extend)) {
            return status::NOT_OPEN_OUTPUT;
        }
        if self.wtx.is_none() {
            return status::NOT_OPEN_OUTPUT;
        }
        let rec = self.fit(rec);
        let pkey = extract(&self.primary, &rec);
        let stored = self.encode_value(&rec);
        // A record only needs a stored insertion sequence when there are
        // alternate keys (it orders their duplicates). Keyed-only files skip all
        // of that and pay just one B+tree insert per WRITE. Allocate the seq up
        // front so the table handles below are each opened exactly once (a
        // duplicate-key rejection just leaves a harmless gap in the sequence).
        let has_alts = !self.alternates.is_empty();
        let seq = if has_alts {
            match self.alloc_seq() {
                Ok(s) => s,
                Err(()) => return status::IO_ERROR,
            }
        } else {
            0
        };
        let alt_val = Self::alt_value(seq, &pkey);
        // The redb table handles borrow the transaction, so do all of the table
        // work in a scoped block; the handles drop at its end, freeing `self` to
        // update the observability counters afterward.
        let st = 'redb: {
            let w = self.wtx.as_ref().unwrap();
            // One `primary` handle for both the duplicate check and the insert.
            let mut tp = match w.open_table(PRIMARY) {
                Ok(t) => t,
                Err(_) => break 'redb status::IO_ERROR,
            };
            match tp.get(pkey.as_slice()) {
                Ok(Some(_)) => break 'redb status::DUP_KEY,
                Ok(None) => {}
                Err(_) => break 'redb status::IO_ERROR,
            }
            // One `alt` multimap handle for the duplicate check and the inserts.
            let mut mt = if has_alts {
                match w.open_multimap_table(ALT) {
                    Ok(m) => Some(m),
                    Err(_) => break 'redb status::IO_ERROR,
                }
            } else {
                None
            };
            if let Some(mt) = mt.as_ref() {
                for (i, ks) in self.alternates.iter().enumerate() {
                    if ks.duplicates {
                        continue;
                    }
                    let comp = composite(i, &extract(ks, &rec));
                    let occupied = mt.get(comp.as_slice()).map(|mut it| it.next().is_some()).unwrap_or(false);
                    if occupied {
                        break 'redb status::DUP_KEY;
                    }
                }
            }
            // Insert the record; then (if needed) its sequence and alt entries.
            if tp.insert(pkey.as_slice(), stored.as_slice()).is_err() {
                break 'redb status::IO_ERROR;
            }
            if has_alts {
                if match w.open_table(SEQ) {
                    Ok(mut t) => t.insert(pkey.as_slice(), seq.to_be_bytes().as_slice()).is_err(),
                    Err(_) => true,
                } {
                    break 'redb status::IO_ERROR;
                }
                let mt = mt.as_mut().unwrap();
                for (i, ks) in self.alternates.iter().enumerate() {
                    let comp = composite(i, &extract(ks, &rec));
                    if mt.insert(comp.as_slice(), alt_val.as_slice()).is_err() {
                        break 'redb status::IO_ERROR;
                    }
                }
            }
            status::OK
        };
        if st == status::OK {
            self.note_write(&pkey, rec.len());
        }
        st
    }

    fn read_key(&mut self, key: &[u8]) -> (Option<Bytes>, &'static str) {
        if !matches!(self.open, Some(OpenMode::Input | OpenMode::Io)) {
            return (None, status::NOT_OPEN_INPUT);
        }
        let pkey = match self.resolve_primary(key) {
            Some(p) => p,
            None => return (None, status::NOT_FOUND),
        };
        match self.lookup_primary(&pkey) {
            Some(rec) => {
                let rv = self.ref_value(&pkey, &rec);
                self.cursor = Some((rv, pkey.clone()));
                self.current = Some(pkey);
                self.start_at = None;
                (Some(rec), status::OK)
            }
            None => (None, status::NOT_FOUND),
        }
    }

    fn read_seq(&mut self, dir: ReadDir) -> (Option<Bytes>, &'static str) {
        if !matches!(self.open, Some(OpenMode::Input | OpenMode::Io)) {
            return (None, status::NOT_OPEN_INPUT);
        }
        if let Some((rv, pk)) = self.start_at.take() {
            let rec = self.lookup_primary(&pk);
            self.cursor = Some((rv, pk.clone()));
            self.current = Some(pk);
            return match rec {
                Some(r) => (Some(r), status::OK),
                None => (None, status::NOT_FOUND),
            };
        }
        let next = match &self.cursor {
            None => self.first_in_kor(dir),
            Some((rv, pk)) => {
                let (rv, pk) = (rv.clone(), pk.clone());
                self.step_kor(&rv, &pk, dir)
            }
        };
        match next {
            Some((rv, pk, rec_opt)) => {
                // For the primary key of reference the record came straight from
                // the range cursor; otherwise fetch it (alternate key of reference).
                let rec = rec_opt.or_else(|| self.lookup_primary(&pk));
                self.cursor = Some((rv, pk.clone()));
                self.current = Some(pk);
                (rec, status::OK)
            }
            None => (None, status::EOF),
        }
    }

    fn start(&mut self, op: StartOp, key: &[u8]) -> &'static str {
        if !matches!(self.open, Some(OpenMode::Input | OpenMode::Io)) {
            return status::NOT_OPEN_INPUT;
        }
        match self.find_start(op, key) {
            Some((rv, pk)) => {
                self.cursor = None;
                self.current = None;
                self.start_at = Some((rv, pk));
                status::OK
            }
            None => status::NOT_FOUND,
        }
    }

    fn rewrite(&mut self, rec: &[u8], random_key: Option<&[u8]>) -> &'static str {
        if self.open != Some(OpenMode::Io) {
            return status::NOT_OPEN_IO;
        }
        let rec = self.fit(rec);
        let pkey = extract(&self.primary, &rec);
        let target = match random_key {
            Some(_) => pkey.clone(),
            None => match &self.current {
                Some(c) => c.clone(),
                None => return status::NO_NEXT,
            },
        };
        if target != pkey {
            return status::LOGIC_ERROR;
        }
        let old = match self.lookup_primary(&pkey) {
            Some(r) => r,
            None => return status::NOT_FOUND,
        };
        // WITHOUT-DUPLICATES alternates must stay unique (excluding self).
        for (i, ks) in self.alternates.iter().enumerate() {
            if ks.duplicates {
                continue;
            }
            let comp = composite(i, &extract(ks, &rec));
            if self.alt_values(&comp).iter().any(|p| p.as_slice() != pkey.as_slice()) {
                return status::DUP_KEY;
            }
        }
        // The record keeps its original insertion sequence across a REWRITE, so
        // its position among duplicate alternates is preserved (disk parity).
        let has_alts = !self.alternates.is_empty();
        let seq = if has_alts { self.seq_of(&pkey).unwrap_or(0) } else { 0 };
        let stored = self.encode_value(&rec);
        let w = self.wtx.as_ref().unwrap();
        // Replace the primary record.
        if match w.open_table(PRIMARY) {
            Ok(mut t) => t.insert(pkey.as_slice(), stored.as_slice()).is_err(),
            Err(_) => true,
        } {
            return status::IO_ERROR;
        }
        // Re-point alternate indexes: remove old entries, add new ones (same seq).
        if has_alts {
            match w.open_multimap_table(ALT) {
                Ok(mut mt) => {
                    let val = Self::alt_value(seq, &pkey);
                    for (i, ks) in self.alternates.iter().enumerate() {
                        let oc = composite(i, &extract(ks, &old));
                        let nc = composite(i, &extract(ks, &rec));
                        if oc != nc {
                            let _ = mt.remove(oc.as_slice(), val.as_slice());
                            if mt.insert(nc.as_slice(), val.as_slice()).is_err() {
                                return status::IO_ERROR;
                            }
                        }
                    }
                }
                Err(_) => return status::IO_ERROR,
            }
        }
        if self.log_level.is_on() {
            self.tx_rewrites += 1;
            self.tx_bytes += rec.len() as u64;
        }
        status::OK
    }

    fn delete(&mut self, random_key: Option<&[u8]>) -> &'static str {
        if self.open != Some(OpenMode::Io) {
            return status::NOT_OPEN_IO;
        }
        // DELETE is always by PRIMARY key (random) or the current record — never
        // by the alternate key of reference.
        let pkey = match random_key {
            Some(k) => pad(k, self.primary.len),
            None => match &self.current {
                Some(c) => c.clone(),
                None => return status::NO_NEXT,
            },
        };
        let old = match self.lookup_primary(&pkey) {
            Some(r) => r,
            None => return status::NOT_FOUND,
        };
        let has_alts = !self.alternates.is_empty();
        let seq = if has_alts { self.seq_of(&pkey).unwrap_or(0) } else { 0 };
        let w = self.wtx.as_ref().unwrap();
        if match w.open_table(PRIMARY) {
            Ok(mut t) => t.remove(pkey.as_slice()).is_err(),
            Err(_) => true,
        } {
            return status::IO_ERROR;
        }
        if has_alts {
            if let Ok(mut t) = w.open_table(SEQ) {
                let _ = t.remove(pkey.as_slice());
            }
            match w.open_multimap_table(ALT) {
                Ok(mut mt) => {
                    let val = Self::alt_value(seq, &pkey);
                    for (i, ks) in self.alternates.iter().enumerate() {
                        let comp = composite(i, &extract(ks, &old));
                        let _ = mt.remove(comp.as_slice(), val.as_slice());
                    }
                }
                Err(_) => return status::IO_ERROR,
            }
        }
        if self.current.as_deref() == Some(pkey.as_slice()) {
            self.current = None;
        }
        if self.log_level.is_on() {
            self.tx_deletes += 1;
        }
        status::OK
    }

    fn set_key_of_reference(&mut self, kor: usize) {
        self.kor = kor.min(self.alternates.len());
    }

    fn set_registered_user(&mut self, user: Option<String>) {
        self.registered_user = user.map(|u| u.trim_end().to_string()).filter(|u| !u.is_empty());
    }

    fn is_open(&self) -> bool {
        self.open.is_some()
    }

    fn commit(&mut self) {
        if let Some(wtx) = self.wtx.take() {
            let _ = wtx.commit();
        }
        self.log_event("COMMIT", &[]);
        if matches!(self.open, Some(OpenMode::Output | OpenMode::Io | OpenMode::Extend)) {
            if let Some(db) = &self.db {
                self.wtx = db.begin_write().ok();
            }
        }
    }

    fn rollback(&mut self) {
        if let Some(wtx) = self.wtx.take() {
            let _ = wtx.abort();
        }
        self.log_event("ROLLBACK", &[]);
        if let Some(db) = &self.db {
            self.wtx = db.begin_write().ok();
        }
        self.cursor = None;
        self.start_at = None;
        self.current = None;
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    use std::time::Instant;

    fn tmp(tag: &str) -> PathBuf {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("prc-redb-bench-{tag}-{n}.rdb"))
    }

    /// Micro-benchmark: cost of opening the table per insert (×2, current), once
    /// per insert (×1), and once for all inserts (cached). Decides whether the
    /// cross-call cached handle is worth the self-referential complexity.
    /// `cargo test -p cobolt-runtime --lib bench::open_table_cost -- --ignored --nocapture`
    #[test]
    #[ignore = "micro-benchmark"]
    fn open_table_cost() {
        let n: u64 = std::env::var("PRC_BENCH_N").ok().and_then(|s| s.parse().ok()).unwrap_or(200_000);

        // (a) open twice per insert
        let p = tmp("a");
        let db = Database::create(&p).unwrap();
        let t0 = Instant::now();
        let w = db.begin_write().unwrap();
        for i in 0..n {
            let k = i.to_be_bytes();
            { let t = w.open_table(PRIMARY).unwrap(); let _ = t.get(k.as_slice()).unwrap(); }
            { let mut t = w.open_table(PRIMARY).unwrap(); t.insert(k.as_slice(), k.as_slice()).unwrap(); }
        }
        w.commit().unwrap();
        let a = t0.elapsed();
        drop(db); let _ = std::fs::remove_file(&p);

        // (b) open once per insert
        let p = tmp("b");
        let db = Database::create(&p).unwrap();
        let t0 = Instant::now();
        let w = db.begin_write().unwrap();
        for i in 0..n {
            let k = i.to_be_bytes();
            let mut t = w.open_table(PRIMARY).unwrap();
            let _ = t.get(k.as_slice()).unwrap();
            t.insert(k.as_slice(), k.as_slice()).unwrap();
        }
        w.commit().unwrap();
        let b = t0.elapsed();
        drop(db); let _ = std::fs::remove_file(&p);

        // (c) cached: open once for all inserts
        let p = tmp("c");
        let db = Database::create(&p).unwrap();
        let t0 = Instant::now();
        let w = db.begin_write().unwrap();
        {
            let mut t = w.open_table(PRIMARY).unwrap();
            for i in 0..n {
                let k = i.to_be_bytes();
                let _ = t.get(k.as_slice()).unwrap();
                t.insert(k.as_slice(), k.as_slice()).unwrap();
            }
        }
        w.commit().unwrap();
        let c = t0.elapsed();
        drop(db); let _ = std::fs::remove_file(&p);

        eprintln!(
            "open_table_cost n={n}: (a)2-opens/ins={a:?} {:.1}us  (b)1-open/ins={b:?} {:.1}us  (c)cached={c:?} {:.1}us",
            a.as_micros() as f64 / n as f64,
            b.as_micros() as f64 / n as f64,
            c.as_micros() as f64 / n as f64,
        );
    }
}
