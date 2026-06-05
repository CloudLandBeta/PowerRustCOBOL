// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Persistent, paged on-disk INDEXED (ISAM) engine — the `STORAGE IS DISK`
//! backend.
//!
//! Unlike the in-memory engine (`indexed.rs`), records and indexes live in a
//! single paged file (the `SELECT … ASSIGN TO` path) and are read on demand, so
//! RAM use is bounded by the page cache rather than the whole data set. The file
//! is built from fixed-size 4 KiB pages managed by a **free list** (freed pages
//! are reused), with:
//!
//! * **B+tree indexes** — one per key (primary + alternates). Variable
//!   byte-packed leaf / internal pages, split on insert, doubly-linked leaves
//!   for `START` + `READ NEXT/PREVIOUS`. Deletes are *lazy* (the key is removed
//!   from its leaf; index pages are not merged/rebalanced — an offline rebuild
//!   would compact them; data pages ARE reclaimed).
//! * a **RecordId directory** mapping a stable `RecordId` → physical record
//!   location, so a record that moves on `REWRITE` only updates the directory,
//!   not every index (the indexes store `RecordId`s).
//! * **slotted data pages** packing multiple records per page, with an overflow
//!   page chain for records larger than a page.
//!
//! Optional `WITH COMPRESSION` compresses each stored record via
//! [`crate::compress`]. Self-contained: no external dependencies.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::compress;
use crate::indexed::{
    status, Bytes, IndexedFileInfo, KeyDescriptor, KeyEncoding, KeyOrdering, KeyPart, KeySpec,
    OpenMode, ReadDir, RecordFormat, StartOp,
};

const PAGE_SIZE: usize = 4096;
const MAGIC: &[u8; 8] = b"PRCIDXD1";

// Page type tags (byte 0 of every non-header page).
const PT_INTERNAL: u8 = 1;
const PT_LEAF: u8 = 2;
const PT_DATA: u8 = 3;
const PT_OVERFLOW: u8 = 4;
const PT_DIR: u8 = 5;

/// A physical record location recorded in the RecordId directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RecLoc {
    /// 0 = free/tombstone, 1 = inline (slotted page), 2 = overflow chain head.
    kind: u8,
    page: u64,
    slot: u16,
    len: u32, // stored (possibly compressed) byte length
}

impl RecLoc {
    const FREE: RecLoc = RecLoc { kind: 0, page: 0, slot: 0, len: 0 };
    fn is_live(&self) -> bool {
        self.kind != 0
    }
}

// ── On-disk B+tree node (loaded into memory for an operation) ────────────────

enum Node {
    Leaf { next: u64, prev: u64, entries: Vec<(Bytes, u64)> },
    Internal { child0: u64, entries: Vec<(Bytes, u64)> },
}

impl Node {
    fn serialized_len(&self) -> usize {
        match self {
            Node::Leaf { entries, .. } => {
                19 + entries.iter().map(|(k, _)| 2 + k.len() + 8).sum::<usize>()
            }
            Node::Internal { entries, .. } => {
                11 + entries.iter().map(|(k, _)| 2 + k.len() + 8).sum::<usize>()
            }
        }
    }
    fn fits(&self) -> bool {
        self.serialized_len() <= PAGE_SIZE
    }
}

/// The persistent disk-backed indexed file.
pub struct DiskIndexedFile {
    path: PathBuf,
    file: Option<File>,

    record_len: usize,
    primary: KeySpec,
    alternates: Vec<KeySpec>,
    key_names: Vec<Option<String>>,
    strict_metadata: bool,
    compressing: bool,

    open: Option<OpenMode>,
    kor: usize,
    cursor: Option<(u64, usize)>, // (leaf page id, entry index) in the active tree
    current: Option<u64>,         // last-read RecordId, for REWRITE/DELETE current

    // Header state (persisted in page 0).
    next_page_id: u64,
    free_list_head: u64,
    record_count: u64,
    data_tail: u64,        // current slotted page accepting inline records (0 = none)
    primary_root: u64,
    alt_roots: Vec<u64>,
    dir_head: u64,         // first RecordId-directory page (0 = none yet)

    // RecordId directory, held in memory while open, persisted on close.
    directory: Vec<RecLoc>,

    // Transaction undo log (since the last COMMIT/OPEN) for ROLLBACK, plus a
    // guard so the inverse operations applied during a rollback don't re-log.
    undo: Vec<DiskUndo>,
    tx_replay: bool,
}

/// An undoable mutation recorded since the last `COMMIT`/`OPEN`.
enum DiskUndo {
    /// A `WRITE` — undone by deleting the record (carries its primary key).
    Insert(Bytes),
    /// A `REWRITE` — undone by rewriting the prior record image.
    Update(Bytes),
    /// A `DELETE` — undone by writing the prior record image back.
    Delete(Bytes),
}

type R<T> = std::io::Result<T>;

impl DiskIndexedFile {
    pub fn new(
        path: impl AsRef<Path>,
        record_len: usize,
        primary: KeySpec,
        alternates: Vec<KeySpec>,
    ) -> Self {
        let n = alternates.len();
        DiskIndexedFile {
            path: path.as_ref().to_path_buf(),
            file: None,
            record_len,
            primary,
            alternates,
            key_names: Vec::new(),
            strict_metadata: true,
            compressing: false,
            open: None,
            kor: 0,
            cursor: None,
            current: None,
            next_page_id: 1,
            free_list_head: 0,
            record_count: 0,
            data_tail: 0,
            primary_root: 0,
            alt_roots: vec![0; n],
            dir_head: 0,
            directory: Vec::new(),
            undo: Vec::new(),
            tx_replay: false,
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
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }
    pub fn set_key_of_reference(&mut self, kor: usize) {
        self.kor = kor.min(self.alternates.len());
    }

    // ── Pager ───────────────────────────────────────────────────────────────

    fn file_mut(&mut self) -> &mut File {
        self.file.as_mut().expect("file open")
    }

    fn read_page(&mut self, id: u64) -> R<Vec<u8>> {
        let f = self.file_mut();
        f.seek(SeekFrom::Start(id * PAGE_SIZE as u64))?;
        let mut buf = vec![0u8; PAGE_SIZE];
        // A page allocated past EOF reads as zeros.
        let mut filled = 0;
        while filled < PAGE_SIZE {
            match f.read(&mut buf[filled..]) {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(e) => return Err(e),
            }
        }
        Ok(buf)
    }

    fn write_page(&mut self, id: u64, buf: &[u8]) -> R<()> {
        debug_assert!(buf.len() <= PAGE_SIZE);
        let mut page = vec![0u8; PAGE_SIZE];
        page[..buf.len()].copy_from_slice(buf);
        let f = self.file_mut();
        f.seek(SeekFrom::Start(id * PAGE_SIZE as u64))?;
        f.write_all(&page)
    }

    fn alloc_page(&mut self) -> R<u64> {
        if self.free_list_head != 0 {
            let id = self.free_list_head;
            let p = self.read_page(id)?;
            self.free_list_head = u64::from_le_bytes(p[1..9].try_into().unwrap());
            self.write_page(id, &[0u8; 16])?; // wipe
            Ok(id)
        } else {
            let id = self.next_page_id;
            self.next_page_id += 1;
            self.write_page(id, &[0u8; 16])?; // extend file
            Ok(id)
        }
    }

    fn free_page(&mut self, id: u64) -> R<()> {
        let mut p = vec![0u8; 16];
        p[0] = 0; // type unused
        p[1..9].copy_from_slice(&self.free_list_head.to_le_bytes());
        self.write_page(id, &p)?;
        self.free_list_head = id;
        Ok(())
    }

    // ── B+tree node (de)serialization ────────────────────────────────────────

    fn load_node(&mut self, id: u64) -> R<Node> {
        let p = self.read_page(id)?;
        match p[0] {
            PT_LEAF => {
                let count = u16::from_le_bytes([p[1], p[2]]) as usize;
                let next = u64::from_le_bytes(p[3..11].try_into().unwrap());
                let prev = u64::from_le_bytes(p[11..19].try_into().unwrap());
                let mut i = 19;
                let mut entries = Vec::with_capacity(count);
                for _ in 0..count {
                    let kl = u16::from_le_bytes([p[i], p[i + 1]]) as usize;
                    i += 2;
                    let k = p[i..i + kl].to_vec();
                    i += kl;
                    let v = u64::from_le_bytes(p[i..i + 8].try_into().unwrap());
                    i += 8;
                    entries.push((k, v));
                }
                Ok(Node::Leaf { next, prev, entries })
            }
            PT_INTERNAL => {
                let count = u16::from_le_bytes([p[1], p[2]]) as usize;
                let child0 = u64::from_le_bytes(p[3..11].try_into().unwrap());
                let mut i = 11;
                let mut entries = Vec::with_capacity(count);
                for _ in 0..count {
                    let kl = u16::from_le_bytes([p[i], p[i + 1]]) as usize;
                    i += 2;
                    let k = p[i..i + kl].to_vec();
                    i += kl;
                    let c = u64::from_le_bytes(p[i..i + 8].try_into().unwrap());
                    i += 8;
                    entries.push((k, c));
                }
                Ok(Node::Internal { child0, entries })
            }
            other => Err(corrupt(&format!("bad node type {other} at page {id}"))),
        }
    }

    fn store_node(&mut self, id: u64, node: &Node) -> R<()> {
        let mut buf = Vec::with_capacity(PAGE_SIZE);
        match node {
            Node::Leaf { next, prev, entries } => {
                buf.push(PT_LEAF);
                buf.extend_from_slice(&(entries.len() as u16).to_le_bytes());
                buf.extend_from_slice(&next.to_le_bytes());
                buf.extend_from_slice(&prev.to_le_bytes());
                for (k, v) in entries {
                    buf.extend_from_slice(&(k.len() as u16).to_le_bytes());
                    buf.extend_from_slice(k);
                    buf.extend_from_slice(&v.to_le_bytes());
                }
            }
            Node::Internal { child0, entries } => {
                buf.push(PT_INTERNAL);
                buf.extend_from_slice(&(entries.len() as u16).to_le_bytes());
                buf.extend_from_slice(&child0.to_le_bytes());
                for (k, c) in entries {
                    buf.extend_from_slice(&(k.len() as u16).to_le_bytes());
                    buf.extend_from_slice(k);
                    buf.extend_from_slice(&c.to_le_bytes());
                }
            }
        }
        self.write_page(id, &buf)
    }

    fn new_leaf(&mut self, next: u64, prev: u64) -> R<u64> {
        let id = self.alloc_page()?;
        self.store_node(id, &Node::Leaf { next, prev, entries: Vec::new() })?;
        Ok(id)
    }

    // ── B+tree operations ────────────────────────────────────────────────────

    /// Insert (key → value), returning the (possibly new) root page id.
    fn bt_insert(&mut self, root: u64, key: &[u8], val: u64) -> R<u64> {
        if let Some((sep, right)) = self.insert_rec(root, key, val)? {
            // Root split — grow a new internal root.
            let id = self.alloc_page()?;
            self.store_node(id, &Node::Internal { child0: root, entries: vec![(sep, right)] })?;
            Ok(id)
        } else {
            Ok(root)
        }
    }

    fn insert_rec(&mut self, id: u64, key: &[u8], val: u64) -> R<Option<(Bytes, u64)>> {
        match self.load_node(id)? {
            Node::Leaf { next, prev, mut entries } => {
                match entries.binary_search_by(|(k, _)| k.as_slice().cmp(key)) {
                    Ok(pos) => entries[pos].1 = val, // replace
                    Err(pos) => entries.insert(pos, (key.to_vec(), val)),
                }
                let node = Node::Leaf { next, prev, entries };
                if node.fits() {
                    self.store_node(id, &node)?;
                    return Ok(None);
                }
                // Split leaf.
                let Node::Leaf { next, prev, entries } = node else { unreachable!() };
                let mid = entries.len() / 2;
                let right_entries = entries[mid..].to_vec();
                let left_entries = entries[..mid].to_vec();
                let sep = right_entries[0].0.clone();
                let right_id = self.alloc_page()?;
                // left.next = right ; right.next = old next ; fix prev links.
                self.store_node(id, &Node::Leaf { next: right_id, prev, entries: left_entries })?;
                self.store_node(
                    right_id,
                    &Node::Leaf { next, prev: id, entries: right_entries },
                )?;
                if next != 0 {
                    if let Node::Leaf { next: nn, entries: ne, .. } = self.load_node(next)? {
                        self.store_node(next, &Node::Leaf { next: nn, prev: right_id, entries: ne })?;
                    }
                }
                Ok(Some((sep, right_id)))
            }
            Node::Internal { child0, mut entries } => {
                // Choose child: largest separator <= key.
                let pos = entries.partition_point(|(k, _)| k.as_slice() <= key);
                let child = if pos == 0 { child0 } else { entries[pos - 1].1 };
                if let Some((sep, right)) = self.insert_rec(child, key, val)? {
                    let ip = entries.partition_point(|(k, _)| k.as_slice() < sep.as_slice());
                    entries.insert(ip, (sep, right));
                    let node = Node::Internal { child0, entries };
                    if node.fits() {
                        self.store_node(id, &node)?;
                        return Ok(None);
                    }
                    // Split internal — promote the median key.
                    let Node::Internal { child0, entries } = node else { unreachable!() };
                    let mid = entries.len() / 2;
                    let median = entries[mid].clone();
                    let left_entries = entries[..mid].to_vec();
                    let right_entries = entries[mid + 1..].to_vec();
                    let right_id = self.alloc_page()?;
                    self.store_node(id, &Node::Internal { child0, entries: left_entries })?;
                    self.store_node(
                        right_id,
                        &Node::Internal { child0: median.1, entries: right_entries },
                    )?;
                    Ok(Some((median.0, right_id)))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn bt_search(&mut self, root: u64, key: &[u8]) -> R<Option<u64>> {
        if root == 0 {
            return Ok(None);
        }
        let mut id = root;
        loop {
            match self.load_node(id)? {
                Node::Leaf { entries, .. } => {
                    return Ok(entries
                        .binary_search_by(|(k, _)| k.as_slice().cmp(key))
                        .ok()
                        .map(|pos| entries[pos].1));
                }
                Node::Internal { child0, entries } => {
                    let pos = entries.partition_point(|(k, _)| k.as_slice() <= key);
                    id = if pos == 0 { child0 } else { entries[pos - 1].1 };
                }
            }
        }
    }

    /// Lazy delete: remove `key` from its leaf (no merge/rebalance).
    fn bt_delete(&mut self, root: u64, key: &[u8]) -> R<bool> {
        if root == 0 {
            return Ok(false);
        }
        let mut id = root;
        loop {
            match self.load_node(id)? {
                Node::Leaf { next, prev, mut entries } => {
                    if let Ok(pos) = entries.binary_search_by(|(k, _)| k.as_slice().cmp(key)) {
                        entries.remove(pos);
                        self.store_node(id, &Node::Leaf { next, prev, entries })?;
                        return Ok(true);
                    }
                    return Ok(false);
                }
                Node::Internal { child0, entries } => {
                    let pos = entries.partition_point(|(k, _)| k.as_slice() <= key);
                    id = if pos == 0 { child0 } else { entries[pos - 1].1 };
                }
            }
        }
    }

    fn leftmost_leaf(&mut self, root: u64) -> R<u64> {
        let mut id = root;
        loop {
            match self.load_node(id)? {
                Node::Leaf { .. } => return Ok(id),
                Node::Internal { child0, .. } => id = child0,
            }
        }
    }

    fn rightmost_leaf(&mut self, root: u64) -> R<u64> {
        let mut id = root;
        loop {
            match self.load_node(id)? {
                Node::Leaf { .. } => return Ok(id),
                Node::Internal { child0, entries } => {
                    id = entries.last().map(|(_, c)| *c).unwrap_or(child0);
                }
            }
        }
    }

    /// First leaf position `(leaf, index)` whose key is `>= key`.
    fn find_ge(&mut self, root: u64, key: &[u8]) -> R<Option<(u64, usize)>> {
        if root == 0 {
            return Ok(None);
        }
        let mut id = root;
        let leaf = loop {
            match self.load_node(id)? {
                Node::Leaf { .. } => break id,
                Node::Internal { child0, entries } => {
                    let pos = entries.partition_point(|(k, _)| k.as_slice() <= key);
                    id = if pos == 0 { child0 } else { entries[pos - 1].1 };
                }
            }
        };
        // Scan forward across leaves to the first entry >= key.
        let mut lid = leaf;
        loop {
            let Node::Leaf { next, entries, .. } = self.load_node(lid)? else { unreachable!() };
            let idx = entries.partition_point(|(k, _)| k.as_slice() < key);
            if idx < entries.len() {
                return Ok(Some((lid, idx)));
            }
            if next == 0 {
                return Ok(None);
            }
            lid = next;
        }
    }

    /// Entry at a cursor position, or `None` if the leaf is exhausted there.
    fn entry_at(&mut self, leaf: u64, idx: usize) -> R<Option<(Bytes, u64)>> {
        let Node::Leaf { entries, .. } = self.load_node(leaf)? else { return Ok(None) };
        Ok(entries.get(idx).cloned())
    }

    /// Advance a cursor one entry forward, skipping empty leaves.
    fn step_forward(&mut self, leaf: u64, idx: usize) -> R<Option<(u64, usize)>> {
        let Node::Leaf { next, entries, .. } = self.load_node(leaf)? else { return Ok(None) };
        if idx + 1 < entries.len() {
            return Ok(Some((leaf, idx + 1)));
        }
        let mut nid = next;
        while nid != 0 {
            let Node::Leaf { next, entries, .. } = self.load_node(nid)? else { break };
            if !entries.is_empty() {
                return Ok(Some((nid, 0)));
            }
            nid = next;
        }
        Ok(None)
    }

    /// Move a cursor one entry backward, skipping empty leaves.
    fn step_back(&mut self, leaf: u64, idx: usize) -> R<Option<(u64, usize)>> {
        if idx > 0 {
            return Ok(Some((leaf, idx - 1)));
        }
        let Node::Leaf { prev, .. } = self.load_node(leaf)? else { return Ok(None) };
        let mut pid = prev;
        while pid != 0 {
            let Node::Leaf { prev, entries, .. } = self.load_node(pid)? else { break };
            if !entries.is_empty() {
                return Ok(Some((pid, entries.len() - 1)));
            }
            pid = prev;
        }
        Ok(None)
    }

    // ── Record storage (slotted pages + overflow chain) ──────────────────────

    fn store_record_bytes(&mut self, raw: &[u8]) -> R<RecLoc> {
        let payload = if self.compressing { compress::compress(raw) } else { raw.to_vec() };
        let max_inline = PAGE_SIZE - 5 /*hdr*/ - 4 /*one slot*/;
        if payload.len() <= max_inline {
            self.store_inline(&payload)
        } else {
            self.store_overflow(&payload)
        }
    }

    fn store_inline(&mut self, payload: &[u8]) -> R<RecLoc> {
        // Try the current tail page; if it can't fit, start a fresh one.
        let need = payload.len() + 4; // slot entry + data
        let page_id = if self.data_tail != 0 && self.inline_fits(self.data_tail, need)? {
            self.data_tail
        } else {
            let id = self.alloc_page()?;
            let mut buf = vec![0u8; 5];
            buf[0] = PT_DATA; // slot_count=0, free_top=0
            self.write_page(id, &buf)?;
            self.data_tail = id;
            id
        };
        let mut p = self.read_page(page_id)?;
        let slot_count = u16::from_le_bytes([p[1], p[2]]) as usize;
        let free_top = u16::from_le_bytes([p[3], p[4]]) as usize; // bytes used from page end
        let data_off = PAGE_SIZE - free_top - payload.len();
        p[data_off..data_off + payload.len()].copy_from_slice(payload);
        // Append slot (offset,len).
        let slot_pos = 5 + slot_count * 4;
        p[slot_pos..slot_pos + 2].copy_from_slice(&(data_off as u16).to_le_bytes());
        p[slot_pos + 2..slot_pos + 4].copy_from_slice(&(payload.len() as u16).to_le_bytes());
        p[1..3].copy_from_slice(&((slot_count + 1) as u16).to_le_bytes());
        p[3..5].copy_from_slice(&((free_top + payload.len()) as u16).to_le_bytes());
        self.write_page(page_id, &p)?;
        Ok(RecLoc { kind: 1, page: page_id, slot: slot_count as u16, len: payload.len() as u32 })
    }

    fn inline_fits(&mut self, page_id: u64, need: usize) -> R<bool> {
        let p = self.read_page(page_id)?;
        if p[0] != PT_DATA {
            return Ok(false);
        }
        let slot_count = u16::from_le_bytes([p[1], p[2]]) as usize;
        let free_top = u16::from_le_bytes([p[3], p[4]]) as usize;
        let slot_dir_end = 5 + (slot_count + 1) * 4;
        let data_start = PAGE_SIZE - free_top;
        Ok(slot_dir_end + need <= data_start + 4)
    }

    fn store_overflow(&mut self, payload: &[u8]) -> R<RecLoc> {
        // Chain of pages: [type][u64 next][u32 chunk_len][chunk].
        let cap = PAGE_SIZE - 13;
        let chunks: Vec<&[u8]> = payload.chunks(cap).collect();
        let mut page_ids = Vec::with_capacity(chunks.len());
        for _ in 0..chunks.len() {
            page_ids.push(self.alloc_page()?);
        }
        for (i, chunk) in chunks.iter().enumerate() {
            let next = if i + 1 < page_ids.len() { page_ids[i + 1] } else { 0 };
            let mut buf = vec![0u8; 13 + chunk.len()];
            buf[0] = PT_OVERFLOW;
            buf[1..9].copy_from_slice(&next.to_le_bytes());
            buf[9..13].copy_from_slice(&(chunk.len() as u32).to_le_bytes());
            buf[13..].copy_from_slice(chunk);
            self.write_page(page_ids[i], &buf)?;
        }
        Ok(RecLoc { kind: 2, page: page_ids[0], slot: 0, len: payload.len() as u32 })
    }

    fn load_record_bytes(&mut self, loc: RecLoc) -> R<Bytes> {
        let payload = match loc.kind {
            1 => {
                let p = self.read_page(loc.page)?;
                let slot_pos = 5 + loc.slot as usize * 4;
                let off = u16::from_le_bytes([p[slot_pos], p[slot_pos + 1]]) as usize;
                let len = u16::from_le_bytes([p[slot_pos + 2], p[slot_pos + 3]]) as usize;
                p[off..off + len].to_vec()
            }
            2 => {
                let mut out = Vec::with_capacity(loc.len as usize);
                let mut pid = loc.page;
                while pid != 0 {
                    let p = self.read_page(pid)?;
                    let next = u64::from_le_bytes(p[1..9].try_into().unwrap());
                    let clen = u32::from_le_bytes(p[9..13].try_into().unwrap()) as usize;
                    out.extend_from_slice(&p[13..13 + clen]);
                    pid = next;
                }
                out
            }
            _ => return Ok(Vec::new()),
        };
        Ok(if self.compressing { compress::decompress(&payload) } else { payload })
    }

    fn free_record_storage(&mut self, loc: RecLoc) -> R<()> {
        match loc.kind {
            1 => {
                // Mark the slot empty; free the page once every slot is empty.
                let mut p = self.read_page(loc.page)?;
                let slot_pos = 5 + loc.slot as usize * 4;
                p[slot_pos + 2..slot_pos + 4].copy_from_slice(&0u16.to_le_bytes()); // len = 0
                self.write_page(loc.page, &p)?;
                let slot_count = u16::from_le_bytes([p[1], p[2]]) as usize;
                let all_free = (0..slot_count).all(|s| {
                    let sp = 5 + s * 4;
                    u16::from_le_bytes([p[sp + 2], p[sp + 3]]) == 0
                });
                if all_free {
                    if self.data_tail == loc.page {
                        self.data_tail = 0;
                    }
                    self.free_page(loc.page)?;
                }
            }
            2 => {
                let mut pid = loc.page;
                while pid != 0 {
                    let p = self.read_page(pid)?;
                    let next = u64::from_le_bytes(p[1..9].try_into().unwrap());
                    self.free_page(pid)?;
                    pid = next;
                }
            }
            _ => {}
        }
        Ok(())
    }

    // ── Keys ─────────────────────────────────────────────────────────────────

    fn extract(spec: &KeySpec, rec: &[u8]) -> Bytes {
        let end = (spec.offset + spec.len).min(rec.len());
        let start = spec.offset.min(rec.len());
        let mut k = rec[start..end].to_vec();
        k.resize(spec.len, b' ');
        k
    }

    /// The B+tree key for an alternate index: the raw alt key, with the
    /// RecordId appended when duplicates are allowed (to keep entries unique).
    fn alt_tree_key(spec: &KeySpec, rec: &[u8], recid: u64) -> Bytes {
        let mut k = Self::extract(spec, rec);
        if spec.duplicates {
            k.extend_from_slice(&recid.to_be_bytes());
        }
        k
    }

    // ── OPEN / CLOSE ─────────────────────────────────────────────────────────

    pub fn open(&mut self, mode: OpenMode) -> &'static str {
        if self.open.is_some() {
            return status::LOGIC_ERROR;
        }
        let exists = self.path.exists();
        match mode {
            OpenMode::Output => {
                let f = match OpenOptions::new()
                    .read(true).write(true).create(true).truncate(true).open(&self.path)
                {
                    Ok(f) => f,
                    Err(_) => return status::IO_ERROR,
                };
                self.file = Some(f);
                if self.init_empty().is_err() {
                    return status::IO_ERROR;
                }
            }
            OpenMode::Input | OpenMode::Io | OpenMode::Extend => {
                if !exists {
                    if mode == OpenMode::Input {
                        return status::FILE_NOT_FOUND; // 35
                    }
                    // I-O / EXTEND create an empty file.
                    let f = match OpenOptions::new()
                        .read(true).write(true).create(true).truncate(true).open(&self.path)
                    {
                        Ok(f) => f,
                        Err(_) => return status::IO_ERROR,
                    };
                    self.file = Some(f);
                    if self.init_empty().is_err() {
                        return status::IO_ERROR;
                    }
                } else {
                    let f = match OpenOptions::new().read(true).write(true).open(&self.path) {
                        Ok(f) => f,
                        Err(_) => return status::IO_ERROR,
                    };
                    self.file = Some(f);
                    match self.load_header() {
                        Ok(Some(stored)) => {
                            if self.strict_metadata && !self.schema_matches(&stored) {
                                self.file = None;
                                return status::ATTR_MISMATCH; // 39
                            }
                        }
                        Ok(None) => {
                            self.file = None;
                            return status::ATTR_MISMATCH;
                        }
                        Err(_) => {
                            self.file = None;
                            return status::IO_ERROR;
                        }
                    }
                }
            }
        }
        self.open = Some(mode);
        self.kor = 0;
        self.cursor = None;
        self.current = None;
        self.undo.clear(); // a fresh transaction starts at OPEN
        status::OK
    }

    fn init_empty(&mut self) -> R<()> {
        self.next_page_id = 1;
        self.free_list_head = 0;
        self.record_count = 0;
        self.data_tail = 0;
        self.dir_head = 0;
        self.directory.clear();
        // Root leaf for the primary + each alternate index.
        self.primary_root = self.new_leaf(0, 0)?;
        let n = self.alternates.len();
        self.alt_roots = Vec::with_capacity(n);
        for _ in 0..n {
            let id = self.new_leaf(0, 0)?;
            self.alt_roots.push(id);
        }
        self.write_header()
    }

    pub fn close(&mut self) -> &'static str {
        if self.open.is_none() {
            return status::LOGIC_ERROR;
        }
        let writable = matches!(self.open, Some(OpenMode::Output | OpenMode::Io | OpenMode::Extend));
        let mut code = status::OK;
        if writable {
            if self.persist_directory().is_err() || self.write_header().is_err() {
                code = status::IO_ERROR;
            }
            if let Some(f) = self.file.as_mut() {
                let _ = f.flush();
                let _ = f.sync_all();
            }
        }
        self.open = None;
        self.file = None;
        self.cursor = None;
        self.current = None;
        code
    }

    // ── WRITE ────────────────────────────────────────────────────────────────

    pub fn write(&mut self, rec: &[u8]) -> &'static str {
        if !matches!(self.open, Some(OpenMode::Output | OpenMode::Io | OpenMode::Extend)) {
            return status::NOT_OPEN_OUTPUT;
        }
        let rec = self.fit(rec);
        let pkey = Self::extract(&self.primary, &rec);
        match self.bt_search(self.primary_root, &pkey) {
            Ok(Some(_)) => return status::DUP_KEY,
            Ok(None) => {}
            Err(_) => return status::IO_ERROR,
        }
        // A WITHOUT-DUPLICATES alternate must not already hold this value;
        // WITH-DUPLICATES alternates accept any value (a duplicate is a normal,
        // fully successful write — status 00, not the informational 02).
        let alts = self.alternates.clone();
        for (i, ks) in alts.iter().enumerate() {
            if ks.duplicates {
                continue;
            }
            match self.bt_search(self.alt_roots[i], &Self::extract(ks, &rec)) {
                Ok(Some(_)) => return status::DUP_KEY,
                Ok(None) => {}
                Err(_) => return status::IO_ERROR,
            }
        }
        // Allocate RecordId + store record bytes.
        let recid = self.directory.len() as u64;
        let loc = match self.store_record_bytes(&rec) {
            Ok(l) => l,
            Err(_) => return status::IO_ERROR,
        };
        self.directory.push(loc);
        // Index it.
        if self.index_insert(&rec, recid).is_err() {
            return status::IO_ERROR;
        }
        self.record_count += 1;
        if !self.tx_replay {
            self.undo.push(DiskUndo::Insert(pkey));
        }
        status::OK
    }

    fn index_insert(&mut self, rec: &[u8], recid: u64) -> R<()> {
        let pkey = Self::extract(&self.primary, rec);
        self.primary_root = self.bt_insert(self.primary_root, &pkey, recid)?;
        let alts = self.alternates.clone();
        for (i, ks) in alts.iter().enumerate() {
            let k = Self::alt_tree_key(ks, rec, recid);
            self.alt_roots[i] = self.bt_insert(self.alt_roots[i], &k, recid)?;
        }
        Ok(())
    }

    fn index_remove(&mut self, rec: &[u8], recid: u64) -> R<()> {
        let pkey = Self::extract(&self.primary, rec);
        self.bt_delete(self.primary_root, &pkey)?;
        let alts = self.alternates.clone();
        for (i, ks) in alts.iter().enumerate() {
            let k = Self::alt_tree_key(ks, rec, recid);
            self.bt_delete(self.alt_roots[i], &k)?;
        }
        Ok(())
    }

    // ── READ ─────────────────────────────────────────────────────────────────

    pub fn read_key(&mut self, key: &[u8]) -> (Option<Bytes>, &'static str) {
        if !matches!(self.open, Some(OpenMode::Input | OpenMode::Io)) {
            return (None, status::NOT_OPEN_INPUT);
        }
        let key = pad(key, self.kor_key_len());
        let (root, prefixed) = if self.kor == 0 {
            (self.primary_root, false)
        } else {
            (self.alt_roots[self.kor - 1], self.alternates[self.kor - 1].duplicates)
        };
        let found = if prefixed {
            // duplicates alt: first entry whose prefix == key
            match self.find_ge(root, &key) {
                Ok(Some((leaf, idx))) => match self.entry_at(leaf, idx) {
                    Ok(Some((k, recid))) if k.starts_with(&key) => Some((leaf, idx, recid)),
                    _ => None,
                },
                _ => None,
            }
        } else {
            match self.bt_search(root, &key) {
                Ok(Some(recid)) => {
                    // position the cursor at this key for a following READ NEXT
                    let pos = self.find_ge(root, &key).ok().flatten();
                    pos.map(|(l, i)| (l, i, recid))
                }
                _ => None,
            }
        };
        match found {
            Some((leaf, idx, recid)) => {
                self.cursor = Some((leaf, idx));
                self.current = Some(recid);
                match self.directory.get(recid as usize).copied() {
                    Some(loc) if loc.is_live() => match self.load_record_bytes(loc) {
                        Ok(b) => (Some(b), status::OK),
                        Err(_) => (None, status::IO_ERROR),
                    },
                    _ => (None, status::NOT_FOUND),
                }
            }
            None => (None, status::NOT_FOUND),
        }
    }

    pub fn read_seq(&mut self, dir: ReadDir) -> (Option<Bytes>, &'static str) {
        if !matches!(self.open, Some(OpenMode::Input | OpenMode::Io)) {
            return (None, status::NOT_OPEN_INPUT);
        }
        let root = self.active_root();
        let next_pos = match self.cursor {
            None => match dir {
                ReadDir::Next => match self.leftmost_leaf(root) {
                    Ok(leaf) => self.first_nonempty_from(leaf),
                    Err(_) => return (None, status::IO_ERROR),
                },
                ReadDir::Previous => match self.rightmost_leaf(root) {
                    Ok(leaf) => self.last_nonempty_from(leaf),
                    Err(_) => return (None, status::IO_ERROR),
                },
            },
            Some((leaf, idx)) => match dir {
                ReadDir::Next => self.step_forward(leaf, idx).unwrap_or(None),
                ReadDir::Previous => self.step_back(leaf, idx).unwrap_or(None),
            },
        };
        let Some((leaf, idx)) = next_pos else { return (None, status::EOF) };
        match self.entry_at(leaf, idx) {
            Ok(Some((_, recid))) => {
                self.cursor = Some((leaf, idx));
                self.current = Some(recid);
                match self.directory.get(recid as usize).copied() {
                    Some(loc) if loc.is_live() => match self.load_record_bytes(loc) {
                        Ok(b) => (Some(b), status::OK),
                        Err(_) => (None, status::IO_ERROR),
                    },
                    _ => (None, status::EOF),
                }
            }
            _ => (None, status::EOF),
        }
    }

    fn first_nonempty_from(&mut self, leaf: u64) -> Option<(u64, usize)> {
        let mut lid = leaf;
        loop {
            let Node::Leaf { next, entries, .. } = self.load_node(lid).ok()? else { return None };
            if !entries.is_empty() {
                return Some((lid, 0));
            }
            if next == 0 {
                return None;
            }
            lid = next;
        }
    }

    fn last_nonempty_from(&mut self, leaf: u64) -> Option<(u64, usize)> {
        let mut lid = leaf;
        loop {
            let Node::Leaf { prev, entries, .. } = self.load_node(lid).ok()? else { return None };
            if !entries.is_empty() {
                return Some((lid, entries.len() - 1));
            }
            if prev == 0 {
                return None;
            }
            lid = prev;
        }
    }

    // ── START ────────────────────────────────────────────────────────────────

    pub fn start(&mut self, op: StartOp, key: &[u8]) -> &'static str {
        if !matches!(self.open, Some(OpenMode::Input | OpenMode::Io)) {
            return status::NOT_OPEN_INPUT;
        }
        let klen = self.kor_key_len();
        let key = pad(key, klen);
        let root = self.active_root();
        let dup = self.kor != 0 && self.alternates[self.kor - 1].duplicates;

        // Position helper using key prefixes (alt-with-duplicates keys carry a
        // trailing RecordId, so compare on the leading `klen` bytes).
        let matched = match op {
            StartOp::Eq => match self.find_ge(root, &key) {
                Ok(Some((leaf, idx))) => self
                    .entry_at(leaf, idx)
                    .ok()
                    .flatten()
                    .filter(|(k, _)| key_prefix(k, dup, klen) == key.as_slice())
                    .map(|_| (leaf, idx)),
                _ => None,
            },
            StartOp::Ge => self.find_ge(root, &key).ok().flatten(),
            StartOp::Gt => self.first_gt(root, &key, dup, klen),
            StartOp::Le => self.last_le(root, &key, dup, klen),
            StartOp::Lt => self.last_lt(root, &key, dup, klen),
        };
        match matched {
            Some((leaf, idx)) => {
                // Position so the next READ NEXT yields this entry: store the
                // predecessor as the cursor (or None at the start).
                self.cursor = match self.step_back(leaf, idx).unwrap_or(None) {
                    Some(p) => Some(p),
                    None => None,
                };
                self.current = None;
                status::OK
            }
            None => status::NOT_FOUND,
        }
    }

    /// First entry whose key prefix is strictly greater than `key`.
    fn first_gt(&mut self, root: u64, key: &[u8], dup: bool, klen: usize) -> Option<(u64, usize)> {
        let mut pos = self.find_ge(root, key).ok().flatten();
        while let Some((leaf, idx)) = pos {
            match self.entry_at(leaf, idx).ok().flatten() {
                Some((k, _)) if key_prefix(&k, dup, klen) <= key => {
                    pos = self.step_forward(leaf, idx).unwrap_or(None);
                }
                Some(_) => break,
                None => return None,
            }
        }
        pos
    }

    /// Last entry whose key prefix is `<= key` (the entry just before the first
    /// entry `> key`; or the last entry overall when every key is `<= key`).
    fn last_le(&mut self, root: u64, key: &[u8], dup: bool, klen: usize) -> Option<(u64, usize)> {
        match self.first_gt(root, key, dup, klen) {
            Some((leaf, idx)) => self.step_back(leaf, idx).unwrap_or(None),
            None => {
                let rl = self.rightmost_leaf(root).ok()?;
                self.last_nonempty_from(rl)
            }
        }
    }

    /// Last entry whose key prefix is `< key` (just before the first `>= key`).
    fn last_lt(&mut self, root: u64, key: &[u8], _dup: bool, _klen: usize) -> Option<(u64, usize)> {
        match self.find_ge(root, key).ok().flatten() {
            Some((leaf, idx)) => self.step_back(leaf, idx).unwrap_or(None),
            None => {
                let rl = self.rightmost_leaf(root).ok()?;
                self.last_nonempty_from(rl)
            }
        }
    }

    // ── REWRITE / DELETE ─────────────────────────────────────────────────────

    pub fn rewrite(&mut self, rec: &[u8], random_key: Option<&[u8]>) -> &'static str {
        if self.open != Some(OpenMode::Io) {
            return status::NOT_OPEN_IO;
        }
        let rec = self.fit(rec);
        let pkey = Self::extract(&self.primary, &rec);
        let recid = match random_key {
            Some(_) => match self.bt_search(self.primary_root, &pkey) {
                Ok(Some(r)) => r,
                Ok(None) => return status::NOT_FOUND,
                Err(_) => return status::IO_ERROR,
            },
            None => match self.current {
                Some(r) => r,
                None => return status::NO_NEXT,
            },
        };
        // Old record (for alt-key diffing + primary-key invariance check).
        let old_loc = match self.directory.get(recid as usize).copied() {
            Some(l) if l.is_live() => l,
            _ => return status::NOT_FOUND,
        };
        let old = match self.load_record_bytes(old_loc) {
            Ok(b) => b,
            Err(_) => return status::IO_ERROR,
        };
        if Self::extract(&self.primary, &old) != pkey {
            return status::LOGIC_ERROR; // primary key may not change on REWRITE
        }
        // Update alternate indexes whose value changed.
        let alts = self.alternates.clone();
        for (i, ks) in alts.iter().enumerate() {
            let ko = Self::alt_tree_key(ks, &old, recid);
            let kn = Self::alt_tree_key(ks, &rec, recid);
            if ko != kn {
                if !ks.duplicates {
                    if let Ok(Some(_)) = self.bt_search(self.alt_roots[i], &kn) {
                        return status::DUP_KEY;
                    }
                }
                if self.bt_delete(self.alt_roots[i], &ko).is_err() {
                    return status::IO_ERROR;
                }
                match self.bt_insert(self.alt_roots[i], &kn, recid) {
                    Ok(r) => self.alt_roots[i] = r,
                    Err(_) => return status::IO_ERROR,
                }
            }
        }
        // Replace the record bytes; the RecordId (and thus all index entries)
        // is unchanged — only the directory location may move.
        if self.free_record_storage(old_loc).is_err() {
            return status::IO_ERROR;
        }
        match self.store_record_bytes(&rec) {
            Ok(loc) => self.directory[recid as usize] = loc,
            Err(_) => return status::IO_ERROR,
        }
        if !self.tx_replay {
            self.undo.push(DiskUndo::Update(old));
        }
        status::OK
    }

    pub fn delete(&mut self, random_key: Option<&[u8]>) -> &'static str {
        if self.open != Some(OpenMode::Io) {
            return status::NOT_OPEN_IO;
        }
        let recid = match random_key {
            Some(k) => {
                let key = pad(k, self.primary.len);
                match self.bt_search(self.primary_root, &key) {
                    Ok(Some(r)) => r,
                    Ok(None) => return status::NOT_FOUND,
                    Err(_) => return status::IO_ERROR,
                }
            }
            None => match self.current {
                Some(r) => r,
                None => return status::NO_NEXT,
            },
        };
        let loc = match self.directory.get(recid as usize).copied() {
            Some(l) if l.is_live() => l,
            _ => return status::NOT_FOUND,
        };
        let rec = match self.load_record_bytes(loc) {
            Ok(b) => b,
            Err(_) => return status::IO_ERROR,
        };
        if self.index_remove(&rec, recid).is_err() {
            return status::IO_ERROR;
        }
        if self.free_record_storage(loc).is_err() {
            return status::IO_ERROR;
        }
        self.directory[recid as usize] = RecLoc::FREE;
        self.record_count = self.record_count.saturating_sub(1);
        self.current = None;
        if !self.tx_replay {
            self.undo.push(DiskUndo::Delete(rec));
        }
        status::OK
    }

    /// `COMMIT` — make all changes since the last `COMMIT`/`OPEN` durable and
    /// start a fresh transaction (drops the undo log).
    pub fn commit(&mut self) {
        self.undo.clear();
        let _ = self.persist_directory();
        let _ = self.write_header();
        if let Some(f) = self.file.as_mut() {
            let _ = f.sync_all();
        }
    }

    /// `ROLLBACK` — undo every `WRITE`/`REWRITE`/`DELETE` since the last
    /// `COMMIT`/`OPEN`, in reverse order, then persist the reverted state.
    pub fn rollback(&mut self) {
        let saved_open = self.open;
        self.open = Some(OpenMode::Io); // allow the inverse ops regardless of mode
        self.tx_replay = true;
        for entry in std::mem::take(&mut self.undo).into_iter().rev() {
            match entry {
                DiskUndo::Insert(pkey) => { self.delete(Some(&pkey)); }
                DiskUndo::Update(old) => {
                    let pk = Self::extract(&self.primary, &old);
                    self.rewrite(&old, Some(&pk));
                }
                DiskUndo::Delete(old) => { self.write(&old); }
            }
        }
        self.tx_replay = false;
        self.open = saved_open;
        self.current = None;
        self.cursor = None;
        let _ = self.persist_directory();
        let _ = self.write_header();
        if let Some(f) = self.file.as_mut() {
            let _ = f.sync_all();
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn fit(&self, rec: &[u8]) -> Bytes {
        let mut r = rec.to_vec();
        r.resize(self.record_len.max(rec.len()), b' ');
        r
    }

    fn active_root(&self) -> u64 {
        if self.kor == 0 { self.primary_root } else { self.alt_roots[self.kor - 1] }
    }

    fn kor_key_len(&self) -> usize {
        if self.kor == 0 { self.primary.len } else { self.alternates[self.kor - 1].len }
    }

    // ── Schema / inspect ─────────────────────────────────────────────────────

    fn key_name(&self, idx: usize) -> Option<String> {
        self.key_names.get(idx).cloned().flatten()
    }

    fn descriptor(&self, key_number: u16, spec: &KeySpec, name_idx: usize) -> KeyDescriptor {
        KeyDescriptor {
            key_number,
            name: self.key_name(name_idx),
            parts: vec![KeyPart { offset: spec.offset as u32, length: spec.len as u32, encoding: KeyEncoding::Bytes }],
            duplicates_allowed: spec.duplicates,
            ordering: KeyOrdering::Ascending,
        }
    }

    pub fn inspect(&self) -> IndexedFileInfo {
        let primary = self.descriptor(1, &self.primary, 0);
        let alternates: Vec<KeyDescriptor> = self
            .alternates
            .iter()
            .enumerate()
            .map(|(i, ks)| self.descriptor((i + 2) as u16, ks, i + 1))
            .collect();
        let total = primary.total_length() + alternates.iter().map(|d| d.total_length()).sum::<u32>();
        IndexedFileInfo {
            record_format: RecordFormat::Fixed { length: self.record_len as u32 },
            key_count: 1 + alternates.len() as u16,
            total_key_length: total,
            primary,
            alternates,
        }
    }

    fn schema_matches(&self, stored: &IndexedFileInfo) -> bool {
        let decl = self.inspect();
        fn key_eq(a: &KeyDescriptor, b: &KeyDescriptor) -> bool {
            a.duplicates_allowed == b.duplicates_allowed
                && a.parts.len() == b.parts.len()
                && a.parts.iter().zip(&b.parts).all(|(x, y)| x.offset == y.offset && x.length == y.length)
        }
        decl.record_format == stored.record_format
            && decl.key_count == stored.key_count
            && key_eq(&decl.primary, &stored.primary)
            && decl.alternates.len() == stored.alternates.len()
            && decl.alternates.iter().zip(&stored.alternates).all(|(a, b)| key_eq(a, b))
    }

    // ── Header + directory persistence (page 0 / dir chain) ──────────────────

    fn write_header(&mut self) -> R<()> {
        let mut b = Vec::with_capacity(PAGE_SIZE);
        b.extend_from_slice(MAGIC);
        b.extend_from_slice(&1u16.to_le_bytes()); // version
        b.extend_from_slice(&(PAGE_SIZE as u32).to_le_bytes());
        b.push(1u8); // record format: fixed
        b.push(if self.compressing { 1 } else { 0 });
        b.extend_from_slice(&(self.record_len as u32).to_le_bytes());
        b.extend_from_slice(&self.next_page_id.to_le_bytes());
        b.extend_from_slice(&self.free_list_head.to_le_bytes());
        b.extend_from_slice(&self.record_count.to_le_bytes());
        b.extend_from_slice(&self.data_tail.to_le_bytes());
        b.extend_from_slice(&self.primary_root.to_le_bytes());
        b.extend_from_slice(&self.dir_head.to_le_bytes());
        b.extend_from_slice(&(self.directory.len() as u64).to_le_bytes());
        b.extend_from_slice(&(self.alt_roots.len() as u16).to_le_bytes());
        for r in &self.alt_roots {
            b.extend_from_slice(&r.to_le_bytes());
        }
        // Key schema (for strict validation / inspect): primary + alternates.
        let info = self.inspect();
        let mut keys = vec![info.primary.clone()];
        keys.extend(info.alternates.iter().cloned());
        b.extend_from_slice(&(keys.len() as u16).to_le_bytes());
        for k in &keys {
            b.push(k.duplicates_allowed as u8);
            b.extend_from_slice(&(k.parts.len() as u16).to_le_bytes());
            for p in &k.parts {
                b.extend_from_slice(&p.offset.to_le_bytes());
                b.extend_from_slice(&p.length.to_le_bytes());
            }
        }
        self.write_page(0, &b)
    }

    /// Load page 0 into the header fields; returns the stored schema.
    fn load_header(&mut self) -> R<Option<IndexedFileInfo>> {
        let p = self.read_page(0)?;
        if &p[0..8] != MAGIC {
            return Ok(None);
        }
        let mut i = 8 + 2 + 4; // magic + version + page_size
        let _rf = p[i];
        i += 1;
        self.compressing = p[i] != 0;
        i += 1;
        let read_u32 = |p: &[u8], i: usize| u32::from_le_bytes(p[i..i + 4].try_into().unwrap());
        let read_u64 = |p: &[u8], i: usize| u64::from_le_bytes(p[i..i + 8].try_into().unwrap());
        self.record_len = read_u32(&p, i) as usize;
        i += 4;
        self.next_page_id = read_u64(&p, i);
        i += 8;
        self.free_list_head = read_u64(&p, i);
        i += 8;
        self.record_count = read_u64(&p, i);
        i += 8;
        self.data_tail = read_u64(&p, i);
        i += 8;
        self.primary_root = read_u64(&p, i);
        i += 8;
        self.dir_head = read_u64(&p, i);
        i += 8;
        let dir_len = read_u64(&p, i) as usize;
        i += 8;
        let n_alt = u16::from_le_bytes([p[i], p[i + 1]]) as usize;
        i += 2;
        self.alt_roots = Vec::with_capacity(n_alt);
        for _ in 0..n_alt {
            self.alt_roots.push(read_u64(&p, i));
            i += 8;
        }
        // Schema for validation.
        let key_count = u16::from_le_bytes([p[i], p[i + 1]]) as usize;
        i += 2;
        let mut descs = Vec::with_capacity(key_count);
        for kn in 0..key_count {
            let dup = p[i] != 0;
            i += 1;
            let parts_n = u16::from_le_bytes([p[i], p[i + 1]]) as usize;
            i += 2;
            let mut parts = Vec::with_capacity(parts_n);
            for _ in 0..parts_n {
                let off = read_u32(&p, i);
                i += 4;
                let len = read_u32(&p, i);
                i += 4;
                parts.push(KeyPart { offset: off, length: len, encoding: KeyEncoding::Bytes });
            }
            descs.push(KeyDescriptor {
                key_number: (kn + 1) as u16,
                name: None,
                parts,
                duplicates_allowed: dup,
                ordering: KeyOrdering::Ascending,
            });
        }
        // Load the RecordId directory chain.
        self.load_directory(dir_len)?;

        let primary = descs.first().cloned().unwrap_or(KeyDescriptor {
            key_number: 1, name: None, parts: vec![], duplicates_allowed: false, ordering: KeyOrdering::Ascending,
        });
        let alternates = if descs.len() > 1 { descs[1..].to_vec() } else { Vec::new() };
        let total = primary.total_length() + alternates.iter().map(|d| d.total_length()).sum::<u32>();
        Ok(Some(IndexedFileInfo {
            record_format: RecordFormat::Fixed { length: self.record_len as u32 },
            key_count: key_count as u16,
            total_key_length: total,
            primary,
            alternates,
        }))
    }

    fn persist_directory(&mut self) -> R<()> {
        // Free the previous directory chain.
        let mut pid = self.dir_head;
        while pid != 0 {
            let p = self.read_page(pid)?;
            let next = u64::from_le_bytes(p[1..9].try_into().unwrap());
            self.free_page(pid)?;
            pid = next;
        }
        self.dir_head = 0;
        if self.directory.is_empty() {
            return Ok(());
        }
        const ENTRY: usize = 15; // kind(1) + page(8) + slot(2) + len(4)
        let per_page = (PAGE_SIZE - 11) / ENTRY;
        // Snapshot into owned chunks so we can allocate pages while iterating.
        let chunks: Vec<Vec<RecLoc>> =
            self.directory.chunks(per_page).map(|c| c.to_vec()).collect();
        let mut ids = Vec::with_capacity(chunks.len());
        for _ in 0..chunks.len() {
            ids.push(self.alloc_page()?);
        }
        for (ci, chunk) in chunks.iter().enumerate() {
            let next = if ci + 1 < ids.len() { ids[ci + 1] } else { 0 };
            let mut b = Vec::with_capacity(PAGE_SIZE);
            b.push(PT_DIR);
            b.extend_from_slice(&next.to_le_bytes());
            b.extend_from_slice(&(chunk.len() as u16).to_le_bytes());
            for e in chunk.iter() {
                b.push(e.kind);
                b.extend_from_slice(&e.page.to_le_bytes());
                b.extend_from_slice(&e.slot.to_le_bytes());
                b.extend_from_slice(&e.len.to_le_bytes());
            }
            self.write_page(ids[ci], &b)?;
        }
        self.dir_head = ids[0];
        Ok(())
    }

    fn load_directory(&mut self, dir_len: usize) -> R<()> {
        self.directory = Vec::with_capacity(dir_len);
        let mut pid = self.dir_head;
        while pid != 0 {
            let p = self.read_page(pid)?;
            let next = u64::from_le_bytes(p[1..9].try_into().unwrap());
            let count = u16::from_le_bytes([p[9], p[10]]) as usize;
            let mut i = 11;
            for _ in 0..count {
                let kind = p[i];
                let page = u64::from_le_bytes(p[i + 1..i + 9].try_into().unwrap());
                let slot = u16::from_le_bytes([p[i + 9], p[i + 10]]);
                let len = u32::from_le_bytes(p[i + 11..i + 15].try_into().unwrap());
                i += 15;
                self.directory.push(RecLoc { kind, page, slot, len });
            }
            pid = next;
        }
        Ok(())
    }

    /// Read a file's schema without opening it for I/O (the disk equivalent of
    /// `IndexedFile::inspect_path`).
    pub fn inspect_path(path: impl AsRef<Path>) -> R<Option<IndexedFileInfo>> {
        let mut probe = DiskIndexedFile::new(
            path.as_ref(),
            0,
            KeySpec { offset: 0, len: 0, duplicates: false },
            Vec::new(),
        );
        if !path.as_ref().exists() {
            return Ok(None);
        }
        let f = OpenOptions::new().read(true).open(path.as_ref())?;
        probe.file = Some(f);
        probe.strict_metadata = false;
        let info = probe.load_header()?;
        Ok(info)
    }
}

impl crate::indexed::IndexedStore for DiskIndexedFile {
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
    fn commit(&mut self) { self.commit() }
    fn rollback(&mut self) { self.rollback() }
}

// ── Free helpers ─────────────────────────────────────────────────────────────

fn corrupt(msg: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, msg.to_string())
}

fn pad(key: &[u8], len: usize) -> Bytes {
    let mut k = key.to_vec();
    k.resize(len, b' ');
    k
}

/// The comparable prefix of a B+tree key: for a duplicates-allowed alternate
/// the trailing 8-byte RecordId is dropped so comparisons use the alt value.
fn key_prefix(k: &[u8], dup: bool, klen: usize) -> &[u8] {
    if dup && k.len() >= klen {
        &k[..klen]
    } else {
        k
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("prcidxd-{}-{}.idx", std::process::id(), name))
    }

    /// Record: 5-byte ID (primary @0) + 10-byte NAME (alt @5) = 15 bytes.
    fn rec(id: &str, name: &str) -> Bytes {
        let mut r = format!("{id:0>5}{name:<10}").into_bytes();
        r.truncate(15);
        r.resize(15, b' ');
        r
    }
    fn newfile(p: PathBuf, dup: bool, compress: bool) -> DiskIndexedFile {
        let mut f = DiskIndexedFile::new(
            p,
            15,
            KeySpec { offset: 0, len: 5, duplicates: false },
            vec![KeySpec { offset: 5, len: 10, duplicates: dup }],
        );
        f.set_compressing(compress);
        f
    }
    /// Primary-key-only file (no alternate) for tests that reuse the NAME field.
    fn newfile_pk(p: PathBuf) -> DiskIndexedFile {
        DiskIndexedFile::new(p, 15, KeySpec { offset: 0, len: 5, duplicates: false }, Vec::new())
    }

    #[test]
    fn pager_free_list_reuses_pages() {
        let p = tmp("pager");
        let _ = std::fs::remove_file(&p);
        let mut f = newfile(p.clone(), false, false);
        assert_eq!(f.open(OpenMode::Output), status::OK);
        let a = f.alloc_page().unwrap();
        let b = f.alloc_page().unwrap();
        assert_ne!(a, b);
        f.free_page(b).unwrap();
        let c = f.alloc_page().unwrap();
        assert_eq!(c, b, "freed page should be reused");
        f.close();
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn write_read_random_and_duplicate_primary() {
        let p = tmp("wr");
        let _ = std::fs::remove_file(&p);
        let mut f = newfile(p.clone(), false, false);
        f.open(OpenMode::Output);
        assert_eq!(f.write(&rec("1", "ALICE")), status::OK);
        assert_eq!(f.write(&rec("2", "BOB")), status::OK);
        assert_eq!(f.write(&rec("1", "EVE")), status::DUP_KEY);
        f.close();

        let mut g = newfile(p.clone(), false, false);
        assert_eq!(g.open(OpenMode::Input), status::OK);
        let (r, s) = g.read_key(b"00002");
        assert_eq!(s, status::OK);
        assert_eq!(&r.unwrap()[5..10], b"BOB  ");
        assert_eq!(g.read_key(b"09999").1, status::NOT_FOUND);
        g.close();
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn sequential_next_previous_and_start() {
        let p = tmp("seq");
        let _ = std::fs::remove_file(&p);
        let mut f = newfile(p.clone(), false, false);
        f.open(OpenMode::Output);
        // Insert out of order.
        for (id, nm) in [("3", "C"), ("1", "A"), ("2", "B")] {
            assert_eq!(f.write(&rec(id, nm)), status::OK);
        }
        f.close();

        let mut g = newfile(p.clone(), false, false);
        g.open(OpenMode::Input);
        let ids: Vec<String> = (0..3)
            .map(|_| {
                let (r, _) = g.read_seq(ReadDir::Next);
                String::from_utf8_lossy(&r.unwrap()[0..5]).into_owned()
            })
            .collect();
        assert_eq!(ids, ["00001", "00002", "00003"]);
        assert_eq!(g.read_seq(ReadDir::Next).1, status::EOF);

        // START >= "00002", READ NEXT → 00002, READ PREVIOUS → 00001.
        assert_eq!(g.start(StartOp::Ge, b"00002"), status::OK);
        let (r, _) = g.read_seq(ReadDir::Next);
        assert_eq!(&r.unwrap()[0..5], b"00002");
        let (r, _) = g.read_seq(ReadDir::Previous);
        assert_eq!(&r.unwrap()[0..5], b"00001");
        g.close();
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn start_relations() {
        let p = tmp("startrel");
        let _ = std::fs::remove_file(&p);
        let mut f = newfile_pk(p.clone());
        f.open(OpenMode::Output);
        for id in ["10", "20", "30", "40"] {
            f.write(&rec(id, "x"));
        }
        f.close();
        let mut g = newfile_pk(p.clone());
        g.open(OpenMode::Input);

        assert_eq!(g.start(StartOp::Gt, b"00020"), status::OK);
        assert_eq!(&g.read_seq(ReadDir::Next).0.unwrap()[0..5], b"00030");

        assert_eq!(g.start(StartOp::Eq, b"00030"), status::OK);
        assert_eq!(&g.read_seq(ReadDir::Next).0.unwrap()[0..5], b"00030");
        assert_eq!(g.start(StartOp::Eq, b"00025"), status::NOT_FOUND);

        assert_eq!(g.start(StartOp::Le, b"00025"), status::OK);
        assert_eq!(&g.read_seq(ReadDir::Next).0.unwrap()[0..5], b"00020");

        assert_eq!(g.start(StartOp::Lt, b"00020"), status::OK);
        assert_eq!(&g.read_seq(ReadDir::Next).0.unwrap()[0..5], b"00010");
        g.close();
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn rewrite_and_delete_under_io() {
        let p = tmp("io");
        let _ = std::fs::remove_file(&p);
        let mut f = newfile(p.clone(), false, false);
        f.open(OpenMode::Output);
        f.write(&rec("1", "ALICE"));
        f.write(&rec("2", "BOB"));
        f.close();

        let mut g = newfile(p.clone(), false, false);
        assert_eq!(g.open(OpenMode::Io), status::OK);
        // REWRITE current after a READ.
        let (_, s) = g.read_key(b"00001");
        assert_eq!(s, status::OK);
        assert_eq!(g.rewrite(&rec("1", "ALICE2"), None), status::OK);
        // DELETE by random key.
        assert_eq!(g.delete(Some(b"00002")), status::OK);
        g.close();

        let mut h = newfile(p.clone(), false, false);
        h.open(OpenMode::Input);
        let (r, s) = h.read_key(b"00001");
        assert_eq!(s, status::OK);
        assert_eq!(&r.unwrap()[5..11], b"ALICE2");
        assert_eq!(h.read_key(b"00002").1, status::NOT_FOUND);
        h.close();
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn alternate_key_with_and_without_duplicates() {
        // No-dup alt rejects a second record with the same NAME.
        let p = tmp("altnodup");
        let _ = std::fs::remove_file(&p);
        let mut f = newfile(p.clone(), false, false);
        f.open(OpenMode::Output);
        assert_eq!(f.write(&rec("1", "ACME")), status::OK);
        assert_eq!(f.write(&rec("2", "ACME")), status::DUP_KEY);
        f.close();
        let _ = std::fs::remove_file(&p);

        // With-dup alt allows it; read by alt key finds one and READ NEXT the other.
        let p = tmp("altdup");
        let _ = std::fs::remove_file(&p);
        let mut f = newfile(p.clone(), true, false);
        f.open(OpenMode::Output);
        assert_eq!(f.write(&rec("1", "ACME")), status::OK); // first
        assert_eq!(f.write(&rec("2", "ACME")), status::OK); // duplicate alt → still 00
        f.close();

        let mut g = newfile(p.clone(), true, false);
        g.open(OpenMode::Input);
        g.set_key_of_reference(1); // alternate
        let (r, s) = g.read_key(b"ACME");
        assert_eq!(s, status::OK);
        let id1 = String::from_utf8_lossy(&r.unwrap()[0..5]).into_owned();
        let (r2, s2) = g.read_seq(ReadDir::Next);
        assert_eq!(s2, status::OK);
        let id2 = String::from_utf8_lossy(&r2.unwrap()[0..5]).into_owned();
        let mut got = [id1, id2];
        got.sort();
        assert_eq!(got, ["00001", "00002"]);
        g.close();
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn many_records_force_btree_splits_and_persist() {
        // Enough records to split leaves and grow internal nodes, then reopen.
        let p = tmp("splits");
        let _ = std::fs::remove_file(&p);
        let mut f = newfile_pk(p.clone());
        f.open(OpenMode::Output);
        let n = 2000u32;
        for i in 0..n {
            let id = format!("{i:05}");
            assert_eq!(f.write(&rec(&id, "x")), status::OK, "write {i}");
        }
        f.close();

        let mut g = newfile_pk(p.clone());
        assert_eq!(g.open(OpenMode::Input), status::OK);
        // Full ascending scan (from the start) returns all n in order.
        let mut count = 0u32;
        let mut last = String::new();
        loop {
            let (r, s) = g.read_seq(ReadDir::Next);
            if s == status::EOF {
                break;
            }
            assert_eq!(s, status::OK);
            let id = String::from_utf8_lossy(&r.unwrap()[0..5]).into_owned();
            assert!(id > last, "order broken at {id} after {last}");
            last = id;
            count += 1;
        }
        assert_eq!(count, n, "scan should see every record");
        // Random lookups (after the scan, so the cursor move doesn't matter).
        for i in [0u32, 1, 999, 1000, 1999] {
            let key = format!("{i:05}");
            let (r, s) = g.read_key(key.as_bytes());
            assert_eq!(s, status::OK, "lookup {i}");
            assert_eq!(&r.unwrap()[0..5], key.as_bytes());
        }
        g.close();
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn open_input_missing_is_35() {
        let p = tmp("missing");
        let _ = std::fs::remove_file(&p);
        let mut f = newfile(p, false, false);
        assert_eq!(f.open(OpenMode::Input), status::FILE_NOT_FOUND);
    }

    #[test]
    fn compression_round_trips_large_records() {
        // 600-byte padded records (force >50% compression) through the disk path.
        let p = tmp("zip");
        let _ = std::fs::remove_file(&p);
        let mut f = DiskIndexedFile::new(
            p.clone(),
            600,
            KeySpec { offset: 0, len: 5, duplicates: false },
            Vec::new(),
        );
        f.set_compressing(true);
        f.open(OpenMode::Output);
        for i in 0..50u32 {
            let mut r = format!("{i:05}").into_bytes();
            r.resize(600, b' ');
            assert_eq!(f.write(&r), status::OK);
        }
        f.close();
        // The file must be far smaller than 50 records × 600 bytes uncompressed.
        let file_len = std::fs::metadata(&p).unwrap().len();
        assert!(file_len < 50 * 600 / 2 + 4096 * 4, "compression ineffective: {file_len} bytes");

        let mut g = DiskIndexedFile::new(
            p.clone(),
            600,
            KeySpec { offset: 0, len: 5, duplicates: false },
            Vec::new(),
        );
        // compressing flag is read back from the header.
        assert_eq!(g.open(OpenMode::Input), status::OK);
        let (r, s) = g.read_key(b"00042");
        assert_eq!(s, status::OK);
        let r = r.unwrap();
        assert_eq!(r.len(), 600);
        assert_eq!(&r[0..5], b"00042");
        g.close();
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn strict_schema_mismatch_is_39() {
        let p = tmp("mismatch");
        let _ = std::fs::remove_file(&p);
        let mut f = newfile(p.clone(), false, false);
        f.open(OpenMode::Output);
        f.write(&rec("1", "X"));
        f.close();
        // Declare the alternate WITH DUPLICATES → schema differs.
        let mut g = newfile(p.clone(), true, false);
        assert_eq!(g.open(OpenMode::Input), status::ATTR_MISMATCH);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn inspect_path_reports_schema() {
        let p = tmp("inspect");
        let _ = std::fs::remove_file(&p);
        let mut f = newfile(p.clone(), true, false);
        f.set_key_names(vec![Some("ID".into()), Some("NAME".into())]);
        f.open(OpenMode::Output);
        f.write(&rec("1", "A"));
        f.close();
        let info = DiskIndexedFile::inspect_path(&p).unwrap().expect("schema");
        assert_eq!(info.key_count, 2);
        assert_eq!(info.record_format, RecordFormat::Fixed { length: 15 });
        assert_eq!((info.primary.parts[0].offset, info.primary.parts[0].length), (0, 5));
        assert!(info.alternates[0].duplicates_allowed);
        let _ = std::fs::remove_file(&p);
    }
}
