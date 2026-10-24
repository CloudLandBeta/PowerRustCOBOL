// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! In-memory INDEXED files and the `SORT` work file of a browser module
//! (WASM-010 R7, Q8).
//!
//! A browser module has no disk. An INDEXED file declared `STORAGE IS MEMORY`
//! keeps its records in the module's **file heap**, a range of memory 0 the
//! generator reserves after the form regions; each block of it belongs to the
//! form instance that wrote it and is released with that instance. There is no
//! record limit (operator, Q-new-1, 2026-10-22): when the heap cannot hold a
//! record, or the index that orders it, the verb ends with **FILE STATUS 89**
//! and every record already held stays.
//!
//! The behaviour is the interpreter's in-memory engine
//! (`cobolt-runtime/src/indexed.rs`, `IndexedFile`) together with what its
//! `exec_*` verbs add around it (`interpreter.rs`): status codes, the order of
//! records sharing an alternate value (the order they joined it), the cursor a
//! `START` leaves, the at-end latch (46), the record a `REWRITE` or `DELETE`
//! needs in the sequential access mode (43), and the ascending-key rule of a
//! sequential `WRITE` (21). Differential tests hold the two equal.
//!
//! # The heap
//!
//! Blocks of `[size u32][owner u32][bytes …]`, sizes a multiple of 8 including
//! the header; owner 0 is free, otherwise the instance number plus one. Next
//! fit with forward coalescing. No routine ever grows memory.
//!
//! # A file descriptor (constant, words)
//!
//! ```text
//!  0 rec_len     bytes of a record
//!  1 area_off    region offset of the record area
//!  2 status_opd  operand of the FILE STATUS item, or 0
//!  3 fcb_off     region offset of the file control block
//!  4 flags       bit 0 SELECT OPTIONAL, bit 1 ACCESS SEQUENTIAL
//!  5 n_keys      1 + alternate keys
//!  6 keys        address of n_keys × [dup, len, nparts, parts_addr]
//!                (parts: nparts × [offset in the record, len])
//! ```
//!
//! # The file control block (region)
//!
//! ```text
//!  +0 flags   bit 0 exists, 1 closed WITH LOCK, 2 at end (46 next),
//!             3 record established, 4 cursor set, 5 current set,
//!             6 last written key set
//!  +4 use     the OPEN mode + 1 the program asked for (USE procedures)
//!  +8 eng     the mode + 1 the file is open in
//! +12 kor     key of reference (0 primary)
//! +16 count   records
//! +20 cap     slots of every index vector
//! +24 seq     next join sequence
//! +28 vecs    one address per key: the primary order, then each alternate's
//!  …  cursor key, current key, last written key (primary-key bytes each)
//! ```
//!
//! A record block holds one join sequence per alternate key, then the record.

use crate::mem;
use crate::opd::{locate, parse};
use crate::state::{err, st};
use core::cmp::Ordering;

/// Bytes of the control block before the index vector addresses.
pub const FCB_HDR: u32 = 28;
/// The status a verb ends with when the heap is full (Q-new-1).
pub const MEMORY_FULL: u32 = 89;

const F_EXISTS: u32 = 1;
const F_LOCKED: u32 = 2;
const F_AT_END: u32 = 4;
const F_READ: u32 = 8;
const F_CURSOR: u32 = 16;
const F_CURRENT: u32 = 32;
const F_LAST_WRITE: u32 = 64;

pub const MODE_INPUT: u32 = 0;
pub const MODE_OUTPUT: u32 = 1;
pub const MODE_IO: u32 = 2;
pub const MODE_EXTEND: u32 = 3;

/// `START` relations (`KEY IS …`).
pub const START_EQ: u32 = 0;
pub const START_GT: u32 = 1;
pub const START_GE: u32 = 2;
pub const START_LT: u32 = 3;
pub const START_LE: u32 = 4;

/// Bytes of the control block of a file with `n_keys` keys and a primary key
/// of `pk_len` bytes (the generator reserves it in the region).
pub const fn fcb_size(n_keys: u32, pk_len: u32) -> u32 {
    FCB_HDR + 4 * n_keys + 3 * pk_len
}

// ── the heap ────────────────────────────────────────────────────────────────

const HDR: u32 = 8;

/// Give the routines the heap `[base, base + size)` (the module's `pr_init`).
pub fn heap_init(base: u32, size: u32) {
    let s = st();
    let base = (base + 7) & !7;
    let size = size & !7;
    s.heap_base = base;
    s.heap_end = base + size;
    s.heap_rover = base;
    if size >= HDR {
        mem::wr_u32(base, size);
        mem::wr_u32(base + 4, 0);
    }
}

fn blk_size(b: u32) -> u32 {
    mem::rd_u32(b)
}

fn blk_owner(b: u32) -> u32 {
    mem::rd_u32(b + 4)
}

/// Merge the free blocks that follow free block `b` into it.
fn coalesce(b: u32) {
    let end = st().heap_end;
    let mut size = blk_size(b);
    loop {
        let n = b + size;
        if n >= end || blk_owner(n) != 0 {
            break;
        }
        size += blk_size(n);
    }
    mem::wr_u32(b, size);
}

/// `n` bytes owned by the running instance, or 0 when the heap cannot hold them.
pub fn heap_alloc(n: u32) -> u32 {
    let (base, end, rover) = {
        let s = st();
        (s.heap_base, s.heap_end, s.heap_rover)
    };
    if end <= base || n == 0 {
        return 0;
    }
    let need = (n + HDR + 7) & !7;
    // Next fit: from the rover to the end, then from the start to the rover.
    for (from, to) in [(rover, end), (base, rover)] {
        let mut b = from;
        while b < to && b < end {
            if blk_owner(b) == 0 {
                coalesce(b);
                let size = blk_size(b);
                if size >= need {
                    if size - need >= 16 {
                        mem::wr_u32(b + need, size - need);
                        mem::wr_u32(b + need + 4, 0);
                        mem::wr_u32(b, need);
                    }
                    mem::wr_u32(b + 4, st().inst + 1);
                    st().heap_rover = b;
                    return b + HDR;
                }
            }
            b += blk_size(b);
        }
    }
    0
}

/// Give back a block [`heap_alloc`] returned.
pub fn heap_free(p: u32) {
    if p == 0 {
        return;
    }
    mem::wr_u32(p - HDR + 4, 0);
}

/// Free every block instance `inst` owns: its form instance is being released.
pub fn release(inst: u32) {
    let (base, end) = (st().heap_base, st().heap_end);
    let mut b = base;
    while b < end {
        if blk_owner(b) == inst + 1 {
            mem::wr_u32(b + 4, 0);
        }
        b += blk_size(b);
    }
    st().heap_rover = base;
}

/// Bytes of the heap in use, headers included.
pub fn used() -> u32 {
    let (base, end) = (st().heap_base, st().heap_end);
    let mut b = base;
    let mut n = 0;
    while b < end {
        if blk_owner(b) != 0 {
            n += blk_size(b);
        }
        b += blk_size(b);
    }
    n
}

// ── descriptors ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Desc {
    rec_len: u32,
    area_off: u32,
    status_opd: u32,
    fcb_off: u32,
    optional: bool,
    sequential: bool,
    n_keys: u32,
    keys: u32,
}

fn desc(a: u32) -> Desc {
    let w = |i: u32| mem::rd_u32(a + 4 * i);
    Desc {
        rec_len: w(0),
        area_off: w(1),
        status_opd: w(2),
        fcb_off: w(3),
        optional: w(4) & 1 != 0,
        sequential: w(4) & 2 != 0,
        n_keys: w(5),
        keys: w(6),
    }
}

#[derive(Clone, Copy)]
struct Key {
    dup: bool,
    len: u32,
    nparts: u32,
    parts: u32,
}

fn key(d: &Desc, k: u32) -> Key {
    let a = d.keys + 16 * k;
    Key {
        dup: mem::rd_u32(a) != 0,
        len: mem::rd_u32(a + 4),
        nparts: mem::rd_u32(a + 8),
        parts: mem::rd_u32(a + 12),
    }
}

/// The open file: its descriptor, the region it lives in and its control block.
struct File {
    d: Desc,
    base: u32,
    fcb: u32,
    pk: Key,
}

impl File {
    fn new(base: u32, d: u32) -> File {
        let d = desc(d);
        let pk = key(&d, 0);
        File {
            d,
            base,
            fcb: base + d.fcb_off,
            pk,
        }
    }
    fn word(&self, off: u32) -> u32 {
        mem::rd_u32(self.fcb + off)
    }
    fn set_word(&self, off: u32, v: u32) {
        mem::wr_u32(self.fcb + off, v)
    }
    fn flags(&self) -> u32 {
        self.word(0)
    }
    fn has(&self, f: u32) -> bool {
        self.flags() & f != 0
    }
    fn set(&self, f: u32, on: bool) {
        let v = self.flags();
        self.set_word(0, if on { v | f } else { v & !f });
    }
    fn eng(&self) -> u32 {
        self.word(8)
    }
    fn kor(&self) -> u32 {
        self.word(12)
    }
    fn count(&self) -> u32 {
        self.word(16)
    }
    fn n_alt(&self) -> u32 {
        self.d.n_keys - 1
    }
    fn vec(&self, k: u32) -> u32 {
        self.word(FCB_HDR + 4 * k)
    }
    fn at(&self, k: u32, i: u32) -> u32 {
        mem::rd_u32(self.vec(k) + 4 * i)
    }
    /// Where key slot `n` (0 cursor, 1 current, 2 last written) is kept.
    fn slot(&self, n: u32) -> u32 {
        self.fcb + FCB_HDR + 4 * self.d.n_keys + n * self.pk.len
    }
    /// The record bytes of block `p`.
    fn rec(&self, p: u32) -> u32 {
        p + 4 * self.n_alt()
    }
    fn seq(&self, p: u32, alt: u32) -> u32 {
        mem::rd_u32(p + 4 * alt)
    }
    fn area(&self) -> u32 {
        self.base + self.d.area_off
    }
    fn status(&self, code: u32) -> u32 {
        if self.d.status_opd != 0 {
            let loc = locate(self.base, &parse(self.d.status_opd));
            let digits = [b'0' + ((code / 10) % 10) as u8, b'0' + (code % 10) as u8];
            let n = loc.len.min(2);
            mem::bytes_mut(loc.addr, n).copy_from_slice(&digits[..n as usize]);
        }
        code
    }
}

// ── keys ────────────────────────────────────────────────────────────────────

/// Compare the first `n` bytes of key `k` of the record at `r` with the bytes at `s`.
fn key_vs_bytes(r: u32, k: &Key, s: u32, n: u32) -> Ordering {
    let mut done = 0;
    for i in 0..k.nparts {
        if done >= n {
            break;
        }
        let off = mem::rd_u32(k.parts + 8 * i);
        let len = mem::rd_u32(k.parts + 8 * i + 4).min(n - done);
        let o = mem::bytes(r + off, len).cmp(mem::bytes(s + done, len));
        if o != Ordering::Equal {
            return o;
        }
        done += len;
    }
    Ordering::Equal
}

/// Compare key `k` of two records.
fn key_vs_key(a: u32, b: u32, k: &Key) -> Ordering {
    for i in 0..k.nparts {
        let off = mem::rd_u32(k.parts + 8 * i);
        let len = mem::rd_u32(k.parts + 8 * i + 4);
        let o = mem::bytes(a + off, len).cmp(mem::bytes(b + off, len));
        if o != Ordering::Equal {
            return o;
        }
    }
    Ordering::Equal
}

/// Copy key `k` of the record at `r` to `dst`.
fn copy_key(r: u32, k: &Key, dst: u32) {
    let mut done = 0;
    for i in 0..k.nparts {
        let off = mem::rd_u32(k.parts + 8 * i);
        let len = mem::rd_u32(k.parts + 8 * i + 4);
        mem::copy(r + off, dst + done, len);
        done += len;
    }
}

/// The scratch place a search key is built in.
fn scratch() -> u32 {
    crate::mv::scratch().0
}

/// Where the first entry of vector `k` not ordered before `pred` is: a binary
/// search over the count, `pred(record) == Less` meaning "before".
fn lower(f: &File, k: u32, pred: impl Fn(u32) -> Ordering) -> u32 {
    lower_n(f, k, f.count(), pred)
}

fn lower_n(f: &File, k: u32, n: u32, pred: impl Fn(u32) -> Ordering) -> u32 {
    let (mut lo, mut hi) = (0, n);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if pred(f.at(k, mid)) == Ordering::Less {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// The first entry of vector `k` ordered after `pred` (`Greater`).
fn upper(f: &File, k: u32, pred: impl Fn(u32) -> Ordering) -> u32 {
    upper_n(f, k, f.count(), pred)
}

fn upper_n(f: &File, k: u32, n: u32, pred: impl Fn(u32) -> Ordering) -> u32 {
    let (mut lo, mut hi) = (0, n);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if pred(f.at(k, mid)) != Ordering::Greater {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Where record block `p` sits in vector `k` (it is there).
fn position_of(f: &File, k: u32, p: u32) -> u32 {
    if k == 0 {
        let r = f.rec(p);
        return lower(f, 0, &|q| key_vs_key(f.rec(q), r, &f.pk));
    }
    let ak = key(&f.d, k);
    let (r, s) = (f.rec(p), f.seq(p, k - 1));
    lower(f, k, &|q| {
        key_vs_key(f.rec(q), r, &ak).then(f.seq(q, k - 1).cmp(&s))
    })
}

/// The record block whose primary key is the `pk.len` bytes at `s`.
fn find_primary(f: &File, s: u32) -> Option<u32> {
    let pk = f.pk;
    let i = lower(f, 0, &|q| key_vs_bytes(f.rec(q), &pk, s, pk.len));
    (i < f.count() && key_vs_bytes(f.rec(f.at(0, i)), &pk, s, pk.len) == Ordering::Equal)
        .then(|| f.at(0, i))
}

/// The record a key-of-reference value names: the primary key's, or the first
/// to join the alternate value (`IndexedFile::resolve_primary`). The value is
/// `n` bytes at `s`, padded with spaces to the key's length here.
fn resolve(f: &File, s: u32, n: u32) -> Option<u32> {
    let kor = f.kor();
    let k = key(&f.d, kor);
    if n < k.len {
        mem::fill(s + n, k.len - n, b' ');
    }
    if kor == 0 {
        return find_primary(f, s);
    }
    let i = lower(f, kor, &|q| key_vs_bytes(f.rec(q), &k, s, k.len));
    (i < f.count() && key_vs_bytes(f.rec(f.at(kor, i)), &k, s, k.len) == Ordering::Equal)
        .then(|| f.at(kor, i))
}

/// Remember record block `p` as the cursor (`n` = 0) or the current record (1).
fn remember(f: &File, n: u32, p: Option<u32>) {
    let flag = if n == 0 { F_CURSOR } else { F_CURRENT };
    match p {
        Some(p) => {
            copy_key(f.rec(p), &f.pk, f.slot(n));
            f.set(flag, true);
        }
        None => f.set(flag, false),
    }
}

// ── the index vectors ───────────────────────────────────────────────────────

/// Make room for one more entry in every vector; false when the heap is full.
fn reserve(f: &File) -> bool {
    let (count, cap) = (f.count(), f.word(20));
    if count < cap {
        return true;
    }
    let new_cap = if cap == 0 { 16 } else { cap * 2 };
    let mut fresh = [0u32; 64];
    let n = f.d.n_keys as usize;
    if n > fresh.len() {
        return false;
    }
    for slot in fresh.iter_mut().take(n) {
        *slot = heap_alloc(4 * new_cap);
        if *slot == 0 {
            for p in fresh.iter().take(n) {
                heap_free(*p);
            }
            return false;
        }
    }
    for (k, &p) in fresh.iter().enumerate().take(n) {
        let old = f.vec(k as u32);
        mem::copy(old, p, 4 * count);
        heap_free(old);
        f.set_word(FCB_HDR + 4 * k as u32, p);
    }
    f.set_word(20, new_cap);
    true
}

/// Insert `p` at `i` of vector `k`, which holds `n` entries.
fn insert_at(f: &File, k: u32, n: u32, i: u32, p: u32) {
    let v = f.vec(k);
    mem::copy(v + 4 * i, v + 4 * i + 4, 4 * (n - i));
    mem::wr_u32(v + 4 * i, p);
}

/// Remove entry `i` of vector `k`, which holds `n` entries.
fn remove_at(f: &File, k: u32, n: u32, i: u32) {
    let v = f.vec(k);
    mem::copy(v + 4 * i + 4, v + 4 * i, 4 * (n - i - 1));
}

/// Index block `p` under alternate `k` (≥ 1) with its join sequence; the
/// vector holds `n` entries before it.
fn alt_insert(f: &File, k: u32, n: u32, p: u32) {
    let ak = key(&f.d, k);
    let (r, s) = (f.rec(p), f.seq(p, k - 1));
    let i = upper_n(f, k, n, &|q| {
        key_vs_key(f.rec(q), r, &ak).then(f.seq(q, k - 1).cmp(&s))
    });
    insert_at(f, k, n, i, p);
}

fn next_seq(f: &File) -> u32 {
    let s = f.word(24);
    f.set_word(24, s + 1);
    s
}

/// Drop every record and vector of the file.
fn clear(f: &File) {
    for i in 0..f.count() {
        heap_free(f.at(0, i));
    }
    for k in 0..f.d.n_keys {
        heap_free(f.vec(k));
        f.set_word(FCB_HDR + 4 * k, 0);
    }
    f.set_word(16, 0);
    f.set_word(20, 0);
    f.set_word(24, 0);
}

// ── the verbs ───────────────────────────────────────────────────────────────

/// `OPEN mode file` (`MODE_*`); the status.
pub fn open(base: u32, d: u32, mode: u32) -> u32 {
    let f = File::new(base, d);
    if f.has(F_LOCKED) {
        return f.status(38);
    }
    if f.eng() != 0 {
        return f.status(41);
    }
    // A fresh OPEN is before the first record, with none established.
    f.set(F_AT_END | F_READ | F_LAST_WRITE, false);
    let existed = f.has(F_EXISTS);
    let optional_absent = f.d.optional && !existed && mode != MODE_OUTPUT;
    if mode == MODE_INPUT && !existed && !optional_absent {
        return f.status(35);
    }
    if mode == MODE_OUTPUT || !existed {
        clear(&f);
    }
    // An OPTIONAL file that was not there is opened I-O whatever was asked
    // (the interpreter's materialisation), and says so with 05.
    let eng = if optional_absent && mode == MODE_INPUT {
        MODE_IO
    } else {
        mode
    };
    f.set(F_EXISTS, true);
    f.set_word(4, mode + 1);
    f.set_word(8, eng + 1);
    f.set_word(12, 0);
    remember(&f, 0, None);
    remember(&f, 1, None);
    f.status(if optional_absent { 5 } else { 0 })
}

/// `CLOSE file [WITH LOCK | REEL]`: `how` 0 plain, 1 `WITH LOCK`, 2 `REEL`/`UNIT`.
pub fn close(base: u32, d: u32, how: u32) -> u32 {
    let f = File::new(base, d);
    if how == 1 {
        f.set(F_LOCKED, true);
    }
    if how == 2 {
        // A disk has no volumes: 07, and the file stays open.
        return f.status(if f.eng() != 0 { 7 } else { 42 });
    }
    f.set(F_READ | F_LAST_WRITE, false);
    if f.eng() == 0 {
        return f.status(42);
    }
    f.set_word(4, 0);
    f.set_word(8, 0);
    remember(&f, 0, None);
    remember(&f, 1, None);
    f.status(0)
}

/// `WRITE record`: the record area's first `wlen` bytes, padded with spaces.
pub fn write(base: u32, d: u32, wlen: u32) -> u32 {
    let f = File::new(base, d);
    let code = write_(&f, wlen);
    f.set(F_READ, false);
    f.status(code)
}

fn write_(f: &File, wlen: u32) -> u32 {
    let rl = f.d.rec_len;
    let s = scratch();
    // The image as stored: the written record, padded to the record length.
    let n = wlen.min(rl);
    mem::copy(f.area(), s, n);
    mem::fill(s + n, rl - n, b' ');
    if f.d.sequential
        && f.has(F_LAST_WRITE)
        && key_vs_bytes(s, &f.pk, f.slot(2), f.pk.len) != Ordering::Greater
    {
        return 21;
    }
    if !matches!(f.eng(), 2 | 3 | 4) {
        return 48;
    }
    let pk = f.pk;
    let i = lower(f, 0, &|q| key_vs_key(f.rec(q), s, &pk));
    if i < f.count() && key_vs_key(f.rec(f.at(0, i)), s, &pk) == Ordering::Equal {
        return 22;
    }
    for k in 1..f.d.n_keys {
        let ak = key(&f.d, k);
        if ak.dup {
            continue;
        }
        let j = lower(f, k, &|q| key_vs_key(f.rec(q), s, &ak));
        if j < f.count() && key_vs_key(f.rec(f.at(k, j)), s, &ak) == Ordering::Equal {
            return 22;
        }
    }
    let p = heap_alloc(4 * f.n_alt() + rl);
    if p == 0 {
        return MEMORY_FULL;
    }
    if !reserve(f) {
        heap_free(p);
        return MEMORY_FULL;
    }
    mem::copy(s, f.rec(p), rl);
    for k in 1..f.d.n_keys {
        let q = next_seq(f);
        mem::wr_u32(p + 4 * (k - 1), q);
    }
    let count = f.count();
    insert_at(f, 0, count, i, p);
    for k in 1..f.d.n_keys {
        alt_insert(f, k, count, p);
    }
    f.set_word(16, count + 1);
    if f.d.sequential {
        copy_key(f.rec(p), &f.pk, f.slot(2));
        f.set(F_LAST_WRITE, true);
    }
    0
}

/// The record of the next or previous entry in key-of-reference order
/// (`IndexedFile::read_seq`).
fn read_seq(f: &File, prev: bool) -> Option<u32> {
    let (kor, count) = (f.kor(), f.count());
    if count == 0 {
        return None;
    }
    if !f.has(F_CURSOR) {
        return Some(f.at(kor, if prev { count - 1 } else { 0 }));
    }
    let cur = f.slot(0);
    let pk = f.pk;
    match find_primary(f, cur) {
        Some(p) => {
            let i = position_of(f, kor, p);
            if prev {
                (i > 0).then(|| f.at(kor, i - 1))
            } else {
                (i + 1 < count).then(|| f.at(kor, i + 1))
            }
        }
        // The cursor's record is gone: the first (last) entry whose primary
        // key is past it, in the order of the key of reference.
        None => {
            if prev {
                (0..count)
                    .rev()
                    .map(|i| f.at(kor, i))
                    .find(|&q| key_vs_bytes(f.rec(q), &pk, cur, pk.len) == Ordering::Less)
            } else {
                (0..count)
                    .map(|i| f.at(kor, i))
                    .find(|&q| key_vs_bytes(f.rec(q), &pk, cur, pk.len) == Ordering::Greater)
            }
        }
    }
}

/// `READ file`. `how`: bit 0 random (by key), bit 1 `PREVIOUS`. A random read
/// takes the key of reference `kor` and the value from the key spec `ks`
/// (`[nparts, (region offset, len) × nparts]`, 0 = no value).
pub fn read(base: u32, d: u32, how: u32, kor: u32, ks: u32) -> u32 {
    let f = File::new(base, d);
    let random = how & 1 != 0;
    if !random && f.has(F_AT_END) {
        return f.status(46);
    }
    let found: Result<u32, u32> = if !matches!(f.eng(), 1 | 3) {
        Err(47)
    } else if random {
        f.set_word(12, kor);
        match key_value(&f, ks) {
            Some(n) => resolve(&f, scratch(), n).ok_or(23),
            None => Err(23),
        }
    } else {
        read_seq(&f, how & 2 != 0).ok_or(10)
    };
    let code = match found {
        Ok(p) => {
            remember(&f, 0, Some(p));
            remember(&f, 1, Some(p));
            mem::copy(f.rec(p), f.area(), f.d.rec_len);
            0
        }
        Err(c) => c,
    };
    if !random && code == 10 {
        f.set(F_AT_END, true);
    } else if code == 0 {
        f.set(F_AT_END, false);
    }
    f.set(F_READ, code == 0);
    f.status(code)
}

/// The value a key spec names, copied to the scratch place; its length.
fn key_value(f: &File, ks: u32) -> Option<u32> {
    if ks == 0 {
        return None;
    }
    let s = scratch();
    let n = mem::rd_u32(ks);
    let mut done = 0;
    for i in 0..n {
        let off = mem::rd_u32(ks + 4 + 8 * i);
        let len = mem::rd_u32(ks + 8 + 8 * i);
        mem::copy(f.base + off, s + done, len);
        done += len;
    }
    Some(done)
}

/// `REWRITE record`; `random` when the access mode is RANDOM or DYNAMIC.
pub fn rewrite(base: u32, d: u32, random: u32) -> u32 {
    let f = File::new(base, d);
    let code = if f.eng() == 0 {
        49
    } else if random == 0 && !f.has(F_READ) {
        43
    } else {
        rewrite_(&f, random != 0)
    };
    f.set(F_READ, false);
    f.status(code)
}

fn rewrite_(f: &File, random: bool) -> u32 {
    if f.eng() != 3 {
        return 49;
    }
    let s = scratch();
    let rl = f.d.rec_len;
    mem::copy(f.area(), s, rl);
    if !random {
        if !f.has(F_CURRENT) {
            return 46;
        }
        // The record the last READ delivered must keep its key.
        if key_vs_bytes(s, &f.pk, f.slot(1), f.pk.len) != Ordering::Equal {
            return 21;
        }
    }
    let Some(p) = find_primary(f, s_key(f, s)) else {
        return 23;
    };
    for k in 1..f.d.n_keys {
        let ak = key(&f.d, k);
        if ak.dup {
            continue;
        }
        let j = lower(f, k, &|q| key_vs_key(f.rec(q), s, &ak));
        let mut i = j;
        while i < f.count() && key_vs_key(f.rec(f.at(k, i)), s, &ak) == Ordering::Equal {
            if f.at(k, i) != p {
                return 22;
            }
            i += 1;
        }
    }
    // An entry whose alternate value is unchanged keeps its place in its
    // duplicate set; one the REWRITE moves joins its new set at the end.
    let count = f.count();
    let mut moved: u64 = 0;
    for k in 1..f.d.n_keys.min(64) {
        let ak = key(&f.d, k);
        if key_vs_key(f.rec(p), s, &ak) != Ordering::Equal {
            moved |= 1 << k;
            let i = position_of(f, k, p);
            remove_at(f, k, count, i);
        }
    }
    mem::copy(s, f.rec(p), rl);
    for k in 1..f.d.n_keys.min(64) {
        if moved & (1 << k) != 0 {
            let q = next_seq(f);
            mem::wr_u32(p + 4 * (k - 1), q);
            alt_insert(f, k, count - 1, p);
        }
    }
    0
}

/// The primary key of the image at `s`, at the end of the scratch place.
fn s_key(f: &File, s: u32) -> u32 {
    let at = s + f.d.rec_len;
    copy_key(s, &f.pk, at);
    at
}

/// `DELETE file`; `random` as for [`rewrite`].
pub fn delete(base: u32, d: u32, random: u32) -> u32 {
    let f = File::new(base, d);
    let code = if f.eng() == 0 {
        49
    } else if random == 0 && !f.has(F_READ) {
        43
    } else {
        delete_(&f, random != 0)
    };
    f.set(F_READ, false);
    f.status(code)
}

fn delete_(f: &File, random: bool) -> u32 {
    if f.eng() != 3 {
        return 49;
    }
    let p = if random {
        // The RECORD KEY's value, looked up under the key of reference.
        let s = scratch();
        copy_key(f.area(), &f.pk, s);
        resolve(f, s, f.pk.len)
    } else if f.has(F_CURRENT) {
        find_primary(f, f.slot(1))
    } else {
        return 46;
    };
    let Some(p) = p else { return 23 };
    let count = f.count();
    for k in 0..f.d.n_keys {
        let i = position_of(f, k, p);
        remove_at(f, k, count, i);
    }
    f.set_word(16, count - 1);
    if f.has(F_CURRENT) && key_vs_bytes(f.rec(p), &f.pk, f.slot(1), f.pk.len) == Ordering::Equal {
        f.set(F_CURRENT, false);
    }
    heap_free(p);
    0
}

/// `START file KEY rel item`: position before the first record satisfying it.
pub fn start(base: u32, d: u32, rel: u32, kor: u32, ks: u32) -> u32 {
    let f = File::new(base, d);
    let code = if f.eng() == 0 {
        47
    } else {
        f.set_word(12, kor);
        if !matches!(f.eng(), 1 | 3) {
            47
        } else {
            match key_value(&f, ks) {
                Some(n) => start_(&f, rel, n),
                None => 23,
            }
        }
    };
    if code == 0 {
        f.set(F_AT_END, false);
    }
    f.set(F_READ, false);
    f.status(code)
}

fn start_(f: &File, rel: u32, n: u32) -> u32 {
    let kor = f.kor();
    let k = key(&f.d, kor);
    let count = f.count();
    let s = scratch();
    // The bounds of a generic key: the prefix padded low and high
    // (`generic_key_bounds`). `lo` at s, `hi` after it.
    let n = n.min(k.len);
    let hi = s + k.len;
    mem::copy(s, hi, n);
    mem::fill(s + n, k.len - n, 0x00);
    mem::fill(hi + n, k.len - n, 0xFF);
    let cmp_lo = |q: u32| key_vs_bytes(f.rec(q), &k, s, k.len);
    let cmp_hi = |q: u32| key_vs_bytes(f.rec(q), &k, hi, k.len);
    let pick = match rel {
        START_EQ => {
            let i = lower(f, kor, &cmp_lo);
            (i < count && cmp_hi(f.at(kor, i)) != Ordering::Greater).then_some(i)
        }
        START_GE => {
            let i = lower(f, kor, &cmp_lo);
            (i < count).then_some(i)
        }
        // Past every key sharing the prefix.
        START_GT => {
            let i = upper(f, kor, &|q| key_vs_bytes(f.rec(q), &k, s, n));
            (i < count).then_some(i)
        }
        START_LE => {
            let i = upper(f, kor, &cmp_hi);
            (i > 0).then(|| i - 1)
        }
        _ => {
            let i = lower(f, kor, &cmp_lo);
            (i > 0).then(|| i - 1)
        }
    };
    let Some(mut i) = pick else { return 23 };
    // A value shared by several records names the first to join it.
    while i > 0 && key_vs_key(f.rec(f.at(kor, i - 1)), f.rec(f.at(kor, i)), &k) == Ordering::Equal {
        i -= 1;
    }
    // Position so that the next READ NEXT delivers entry `i`.
    remember(f, 0, (i > 0).then(|| f.at(kor, i - 1)));
    remember(f, 1, None);
    0
}

// ── SORT ────────────────────────────────────────────────────────────────────
//
// A sort descriptor (constant, words): `rec_len`, `area_off` (the SD record),
// `b_off` (a second record area the comparison reads the other record from),
// `state_off` (16 region bytes: vector, count, capacity, next to RETURN),
// `n_keys`, `keys` (n_keys × [operand in the area, operand in the second area,
// ascending]).

struct Sort {
    base: u32,
    rec_len: u32,
    area: u32,
    b: u32,
    state: u32,
    n_keys: u32,
    keys: u32,
}

impl Sort {
    fn new(base: u32, d: u32) -> Sort {
        let w = |i: u32| mem::rd_u32(d + 4 * i);
        Sort {
            base,
            rec_len: w(0),
            area: base + w(1),
            b: base + w(2),
            state: base + w(3),
            n_keys: w(4),
            keys: w(5),
        }
    }
    fn vec(&self) -> u32 {
        mem::rd_u32(self.state)
    }
    fn count(&self) -> u32 {
        mem::rd_u32(self.state + 4)
    }
    /// Records `a` before `b` by the SORT keys (0 when equal).
    fn cmp(&self, a: u32, b: u32) -> Ordering {
        mem::copy(a, self.area, self.rec_len);
        mem::copy(b, self.b, self.rec_len);
        for i in 0..self.n_keys {
            let k = self.keys + 12 * i;
            let o = match crate::rt_cmp3(self.base, mem::rd_u32(k), mem::rd_u32(k + 4)) {
                0 => Ordering::Equal,
                x if x < 0 => Ordering::Less,
                _ => Ordering::Greater,
            };
            if o != Ordering::Equal {
                return if mem::rd_u32(k + 8) != 0 {
                    o
                } else {
                    o.reverse()
                };
            }
        }
        Ordering::Equal
    }
}

/// Start a `SORT`: an empty work file.
pub fn sort_begin(base: u32, d: u32) {
    sort_end(base, d);
}

/// `RELEASE record`: the SD record area joins the work file.
pub fn sort_release(base: u32, d: u32) {
    let s = Sort::new(base, d);
    let (count, cap) = (s.count(), mem::rd_u32(s.state + 8));
    if count == cap {
        let new_cap = if cap == 0 { 64 } else { cap * 2 };
        let v = heap_alloc(4 * new_cap);
        if v == 0 {
            crate::fail(err::MEMORY_FULL);
        }
        mem::copy(s.vec(), v, 4 * count);
        heap_free(s.vec());
        mem::wr_u32(s.state, v);
        mem::wr_u32(s.state + 8, new_cap);
    }
    let p = heap_alloc(s.rec_len);
    if p == 0 {
        crate::fail(err::MEMORY_FULL);
    }
    mem::copy(s.area, p, s.rec_len);
    mem::wr_u32(s.vec() + 4 * count, p);
    mem::wr_u32(s.state + 4, count + 1);
}

/// Order the work file by the keys, keeping the release order of equal
/// records (a stable merge sort; an insertion sort when the heap has no room
/// for the merge's second vector).
pub fn sort_sort(base: u32, d: u32) {
    let s = Sort::new(base, d);
    let (v, n) = (s.vec(), s.count());
    if n < 2 {
        return;
    }
    let tmp = heap_alloc(4 * n);
    if tmp == 0 {
        for i in 1..n {
            let p = mem::rd_u32(v + 4 * i);
            let mut j = i;
            while j > 0 && s.cmp(mem::rd_u32(v + 4 * (j - 1)), p) == Ordering::Greater {
                mem::wr_u32(v + 4 * j, mem::rd_u32(v + 4 * (j - 1)));
                j -= 1;
            }
            mem::wr_u32(v + 4 * j, p);
        }
        return;
    }
    let mut width = 1;
    let (mut src, mut dst) = (v, tmp);
    while width < n {
        let mut lo = 0;
        while lo < n {
            let mid = (lo + width).min(n);
            let hi = (lo + 2 * width).min(n);
            let (mut i, mut j, mut o) = (lo, mid, lo);
            while i < mid && j < hi {
                let (a, b) = (mem::rd_u32(src + 4 * i), mem::rd_u32(src + 4 * j));
                if s.cmp(a, b) != Ordering::Greater {
                    mem::wr_u32(dst + 4 * o, a);
                    i += 1;
                } else {
                    mem::wr_u32(dst + 4 * o, b);
                    j += 1;
                }
                o += 1;
            }
            mem::copy(src + 4 * i, dst + 4 * o, 4 * (mid - i));
            o += mid - i;
            mem::copy(src + 4 * j, dst + 4 * o, 4 * (hi - j));
            lo = hi;
        }
        core::mem::swap(&mut src, &mut dst);
        width *= 2;
    }
    if src != v {
        mem::copy(src, v, 4 * n);
    }
    heap_free(tmp);
}

/// `RETURN file`: the next sorted record into the SD record area; 0, or 10 at the end.
pub fn sort_return(base: u32, d: u32) -> u32 {
    let s = Sort::new(base, d);
    let next = mem::rd_u32(s.state + 12);
    if next >= s.count() {
        return 10;
    }
    mem::copy(mem::rd_u32(s.vec() + 4 * next), s.area, s.rec_len);
    mem::wr_u32(s.state + 12, next + 1);
    0
}

/// The end of a `SORT`: its work file is given back.
pub fn sort_end(base: u32, d: u32) {
    let s = Sort::new(base, d);
    for i in 0..s.count() {
        heap_free(mem::rd_u32(s.vec() + 4 * i));
    }
    heap_free(s.vec());
    for off in [0, 4, 8, 12] {
        mem::wr_u32(s.state + off, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::TestMem;

    #[test]
    fn the_heap_allocates_frees_and_releases_by_owner() {
        let _m = TestMem::new(1 << 16);
        crate::state::reset();
        heap_init(1024, 4096);
        st().inst = 0;
        let a = heap_alloc(100);
        st().inst = 1;
        let b = heap_alloc(100);
        assert!(a != 0 && b != 0 && a != b);
        assert_eq!(used(), 2 * 112);
        release(0);
        assert_eq!(used(), 112);
        heap_free(b);
        assert_eq!(used(), 0);
        // Freed space coalesces: one block as large as the heap fits again.
        assert!(heap_alloc(4096 - 8) != 0);
        assert_eq!(heap_alloc(8), 0, "the heap is full");
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;
    use crate::testkit::Kit;

    /// A file of 8-byte records keyed on the first 3 bytes, with an alternate
    /// key (duplicates) on bytes 3..5.
    fn file(k: &mut Kit) -> u32 {
        let words =
            |v: &[u32]| -> std::vec::Vec<u8> { v.iter().flat_map(|w| w.to_le_bytes()).collect() };
        let p0 = k.put(&words(&[0, 3]));
        let p1 = k.put(&words(&[3, 2]));
        let keys = k.put(&words(&[0, 3, 1, p0, 1, 2, 1, p1]));
        let fcb_off = 64;
        k.put(&words(&[8, 0, 0, fcb_off, 0, 2, keys]))
    }

    #[test]
    fn write_read_and_start_natively() {
        let mut k = Kit::new();
        let heap = k.alloc(64 * 1024);
        heap_init(heap, 64 * 1024);
        let d = file(&mut k);
        assert_eq!(open(k.base, d, MODE_OUTPUT), 0);
        for r in [b"030AAxyz", b"010BBxyz", b"020AAxyz", b"010CCxyz"] {
            k.poke(0, r);
            let _ = write(k.base, d, 8);
        }
        assert_eq!(close(k.base, d, 0), 0);
        assert_eq!(open(k.base, d, MODE_INPUT), 0);
        let mut got = std::vec::Vec::new();
        while read(k.base, d, 0, 0, 0) == 0 {
            got.push(std::string::String::from_utf8(k.peek(0, 8).to_vec()).unwrap());
        }
        assert_eq!(got, ["010BBxyz", "020AAxyz", "030AAxyz"]);
    }
}
