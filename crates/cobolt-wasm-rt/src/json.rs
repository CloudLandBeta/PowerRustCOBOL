// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The COBOL group ⇄ JSON mapping inside the module (WASM-005 R5, R5b, R6;
//! WASM-010 T15).
//!
//! The rules are `specs/WASM-005-websocket-call-and-remote-data/mapping-rules.md`;
//! the reference implementation is `cobolt-wire` (`mapping`, `json`, `number`).
//! This is the module's own implementation of the same rules, held to the same
//! golden vectors, working on the group's **bytes** in memory instead of a
//! value tree: `no_std`, no allocation, no function pointers.
//!
//! * [`encode`] writes a group's JSON text.
//! * [`decode`] reads JSON text into a **shadow copy** of the group in the
//!   workspace and copies it back only when the whole text was accepted, so
//!   an error writes nothing (R3).
//!
//! # The descriptor
//!
//! The generator describes the group in one position-independent blob (every
//! offset below is from the blob's start). Word 0 is the root node. A node is
//! sixteen words:
//!
//! ```text
//!  0 kind        K_GROUP | K_ALNUM | K_NATIONAL | K_UTF8 | K_NUMERIC | K_EDITED
//!  1 flags       NF_* below
//!  2 name        offset of the data-name as declared
//!  3 name_len
//!  4 rel_off     offset from the start of the enclosing group occurrence
//!  5 size        bytes of one occurrence
//!  6 occ_min     7 occ_max (a table; 0 otherwise)
//!  8 odo_up      the group occurrence holding the count item: 0 = the
//!                group the table is in, 1 = the one around it, …
//!  9 odo_rel     the count item's offset from that occurrence's start
//! 10 odo_node    the count item's node
//! 11 n_children  12 children (offset of n_children node offsets)
//! 13 limit       text: bytes (alphanumeric, edited), UTF-16 units (national),
//!                characters (UTF-8); numeric: digits | scale << 8 | signed << 16
//! 14 byte_limit  UTF-8 declared BYTE-LENGTH (0: none)
//! 15 fd          offset of the item's 12-byte field descriptor
//! ```
//!
//! A national item holds its text as the rest of the module holds it — bytes,
//! padded with spaces to its width (`WASM-PARITY.md`) — and its limit is
//! counted in UTF-16 units as the rule says.

use crate::codec;
use crate::fd::Fd;
use crate::mem;
use crate::opd::Loc;
use crate::state::{
    st, JSON_ODO_SIZE, JSON_PATH_SIZE, JSON_SHADOW_SIZE, WS_JSON_ODO, WS_JSON_PATH, WS_JSON_SHADOW,
};
use crate::val::Val;
use cobolt_decimal::CobolNumeric;

pub const K_GROUP: u32 = 0;
pub const K_ALNUM: u32 = 1;
pub const K_NATIONAL: u32 = 2;
pub const K_UTF8: u32 = 3;
pub const K_NUMERIC: u32 = 4;
pub const K_EDITED: u32 = 5;

/// A member of its group's object (not `FILLER`, not a `REDEFINES`).
pub const NF_MEMBER: u32 = 1;
pub const NF_TABLE: u32 = 2;
/// `OCCURS … DEPENDING ON`.
pub const NF_ODO: u32 = 4;
pub const NF_JUSTIFIED: u32 = 8;
/// The count item of some variable table.
pub const NF_ODO_OBJECT: u32 = 16;

pub const NODE_WORDS: u32 = 16;

/// The closed error list of the rules, as the codes the routines return
/// (negated).
pub const E_SYNTAX: i32 = 1;
pub const E_DUPLICATE_KEY: i32 = 2;
pub const E_UNKNOWN_KEY: i32 = 3;
pub const E_WRONG_TYPE: i32 = 4;
pub const E_OVERFLOW: i32 = 5;
pub const E_TOO_LONG: i32 = 6;
pub const E_TOO_MANY: i32 = 7;
pub const E_TOO_FEW: i32 = 8;
pub const E_NOT_UTF8: i32 = 9;
pub const E_ODO_MISMATCH: i32 = 10;
pub const E_ODO_RANGE: i32 = 11;
pub const E_INTERNAL: i32 = 12;
/// Not a mapping error: the encoded text does not fit where it is written.
pub const E_NO_ROOM: i32 = 20;

/// The word an error code stands for in the rules.
pub fn code_name(code: i32) -> &'static str {
    match code {
        E_SYNTAX => "syntax",
        E_DUPLICATE_KEY => "duplicate-key",
        E_UNKNOWN_KEY => "unknown-key",
        E_WRONG_TYPE => "wrong-type",
        E_OVERFLOW => "overflow",
        E_TOO_LONG => "too-long",
        E_TOO_MANY => "too-many-elements",
        E_TOO_FEW => "too-few-elements",
        E_NOT_UTF8 => "not-utf8",
        E_ODO_MISMATCH => "odo-mismatch",
        E_ODO_RANGE => "odo-out-of-range",
        E_NO_ROOM => "no-room",
        _ => "internal",
    }
}

/// The deepest JSON nesting read (J1).
const MAX_DEPTH: u32 = 64;
/// The deepest group nesting of a layout the routines follow.
const MAX_FRAMES: usize = 48;

// ── the descriptor ──────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Node {
    kind: u32,
    flags: u32,
    name: u32,
    name_len: u32,
    rel_off: u32,
    size: u32,
    occ_min: u32,
    occ_max: u32,
    odo_up: u32,
    odo_rel: u32,
    odo_node: u32,
    n_children: u32,
    children: u32,
    limit: u32,
    byte_limit: u32,
    fd: u32,
}

/// The descriptor blob at `d`.
#[derive(Clone, Copy)]
struct Desc {
    d: u32,
}

impl Desc {
    fn node(&self, off: u32) -> Node {
        let w = |i: u32| mem::rd_u32(self.d + off + 4 * i);
        Node {
            kind: w(0),
            flags: w(1),
            name: w(2),
            name_len: w(3),
            rel_off: w(4),
            size: w(5),
            occ_min: w(6),
            occ_max: w(7),
            odo_up: w(8),
            odo_rel: w(9),
            odo_node: w(10),
            n_children: w(11),
            children: w(12),
            limit: w(13),
            byte_limit: w(14),
            fd: w(15),
        }
    }
    fn root(&self) -> u32 {
        mem::rd_u32(self.d)
    }
    fn child(&self, n: &Node, k: u32) -> u32 {
        mem::rd_u32(self.d + n.children + 4 * k)
    }
    fn name(&self, n: &Node) -> &'static [u8] {
        mem::bytes(self.d + n.name, n.name_len)
    }
    fn fd(&self, n: &Node) -> Fd {
        Fd::parse(mem::bytes(self.d + n.fd, crate::fd::FD_SIZE as u32))
    }
    fn loc(&self, n: &Node, addr: u32) -> Loc {
        let fd = self.fd(n);
        Loc {
            addr,
            len: fd.len,
            fd,
        }
    }
}

// ── the path an error names ─────────────────────────────────────────────────

fn path_at() -> u32 {
    st().ws + WS_JSON_PATH
}

fn path_len() -> u32 {
    st().json_path_len
}

fn path_truncate(n: u32) {
    st().json_path_len = n;
}

fn path_push(b: &[u8]) {
    let n = path_len();
    let room = JSON_PATH_SIZE - n;
    let k = (b.len() as u32).min(room);
    mem::bytes_mut(path_at() + n, k).copy_from_slice(&b[..k as usize]);
    st().json_path_len = n + k;
}

fn path_push_index(j: u32) {
    let mut buf = [0u8; 12];
    buf[0] = b'(';
    let n = write_u128(&mut buf[1..], j as u128 + 1);
    buf[1 + n] = b')';
    path_push(&buf[..n + 2]);
}

/// The path of the item the last error names.
pub fn error_path() -> &'static [u8] {
    mem::bytes(path_at(), path_len())
}

// ── numbers (rule N) ────────────────────────────────────────────────────────

/// The digits of `v`, most significant first; how many.
fn write_u128(out: &mut [u8], mut v: u128) -> usize {
    let mut tmp = [0u8; 40];
    let mut n = 0;
    loop {
        tmp[n] = b'0' + (v % 10) as u8;
        n += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    for i in 0..n {
        out[i] = tmp[n - 1 - i];
    }
    n
}

fn pow10(n: u32) -> Option<i128> {
    cobolt_decimal::pow10(n)
}

/// Why a JSON number does not fit an item (all are `overflow` on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumErr {
    Syntax,
    TooManyDecimals,
    TooManyDigits,
    NegativeUnsigned,
    OutOfRange,
}

/// The value of the decimal text `t` (JSON number grammar plus a leading `+`)
/// in an item of `digits` integer digits, `scale` fraction digits and sign
/// `signed`: the mantissa at `scale` (`number::canonical_from_decimal`).
pub fn number_at_scale(t: &[u8], digits: u32, scale: u32, signed: bool) -> Result<i128, NumErr> {
    let mut i = 0usize;
    let mut negative = false;
    match t.first() {
        Some(b'-') => {
            negative = true;
            i = 1;
        }
        Some(b'+') => i = 1,
        _ => {}
    }
    let int_start = i;
    while i < t.len() && t[i].is_ascii_digit() {
        i += 1;
    }
    let int_len = i - int_start;
    let mut frac_start = i;
    let mut frac_len = 0usize;
    if i < t.len() && t[i] == b'.' {
        i += 1;
        frac_start = i;
        while i < t.len() && t[i].is_ascii_digit() {
            i += 1;
        }
        frac_len = i - frac_start;
        if frac_len == 0 {
            return Err(NumErr::Syntax);
        }
    }
    if int_len == 0 && frac_len == 0 {
        return Err(NumErr::Syntax);
    }
    let mut exp: i64 = 0;
    if i < t.len() && (t[i] == b'e' || t[i] == b'E') {
        i += 1;
        let mut eneg = false;
        match t.get(i) {
            Some(b'-') => {
                eneg = true;
                i += 1;
            }
            Some(b'+') => i += 1,
            _ => {}
        }
        let es = i;
        while i < t.len() && t[i].is_ascii_digit() {
            i += 1;
        }
        if es == i {
            return Err(NumErr::Syntax);
        }
        if i - es > 6 {
            return Err(NumErr::OutOfRange);
        }
        for &c in &t[es..i] {
            exp = exp * 10 + (c - b'0') as i64;
        }
        if eneg {
            exp = -exp;
        }
    }
    if i != t.len() {
        return Err(NumErr::Syntax);
    }
    if exp.abs() > 1000 || int_len + frac_len > 4000 {
        return Err(NumErr::OutOfRange);
    }
    let all = (int_len + frac_len) as i64;
    let digit = |k: i64| -> u8 {
        // Virtual position `k` of the digits written, without the point.
        if k < 0 || k >= all {
            0
        } else if (k as usize) < int_len {
            t[int_start + k as usize] - b'0'
        } else {
            t[frac_start + k as usize - int_len] - b'0'
        }
    };
    let point = int_len as i64 + exp;
    // Integer digits: positions [0, point); fraction: [point, all), with the
    // zeros a negative point puts before them.
    let first_nonzero_int = (0..point.max(0)).find(|&k| digit(k) != 0);
    let int_count = first_nonzero_int.map_or(0, |f| point - f);
    let frac_end = (point.max(0)..all)
        .rev()
        .find(|&k| digit(k) != 0)
        .map_or(point, |k| k + 1);
    let frac_count = (frac_end - point).max(0);
    let is_zero = int_count == 0 && frac_count == 0;
    if frac_count > scale as i64 {
        return Err(NumErr::TooManyDecimals);
    }
    if !is_zero && int_count > digits as i64 {
        return Err(NumErr::TooManyDigits);
    }
    if negative && !is_zero && !signed {
        return Err(NumErr::NegativeUnsigned);
    }
    let mut m: i128 = 0;
    if let Some(f) = first_nonzero_int {
        for k in f..point {
            m = m * 10 + digit(k) as i128;
        }
    }
    for k in 0..scale as i64 {
        m = m * 10 + digit(point + k) as i128;
    }
    Ok(if negative { -m } else { m })
}

/// Write the canonical text of mantissa `m` at `scale` (N1).
fn write_canonical(o: &mut Out, m: i128, scale: u32) {
    let mut buf = [0u8; 48];
    let neg = m < 0;
    let n = write_u128(&mut buf, m.unsigned_abs());
    if neg {
        o.byte(b'-');
    }
    let s = scale as usize;
    if n <= s {
        o.byte(b'0');
        if s > 0 {
            o.byte(b'.');
            for _ in 0..s - n {
                o.byte(b'0');
            }
            o.bytes(&buf[..n]);
        }
    } else {
        o.bytes(&buf[..n - s]);
        if s > 0 {
            o.byte(b'.');
            o.bytes(&buf[n - s..n]);
        }
    }
}

/// The value of numeric item `n` at `addr`, as a mantissa at its scale.
fn load_number(desc: &Desc, n: &Node, addr: u32) -> Option<i128> {
    let scale = (n.limit >> 8) & 0xFF;
    match codec::load_value(&desc.loc(n, addr)) {
        Val::Dec(v) => {
            let d = v.decimals as u32;
            if d == scale {
                Some(v.mantissa)
            } else if d < scale {
                v.mantissa.checked_mul(pow10(scale - d)?)
            } else {
                Some(v.mantissa / pow10(d - scale)?)
            }
        }
        Val::F(_) => None,
    }
}

fn store_number(desc: &Desc, n: &Node, addr: u32, m: i128) {
    let scale = (n.limit >> 8) & 0xFF;
    codec::store_value(
        &desc.loc(n, addr),
        Val::Dec(CobolNumeric::new(m, scale as u8)),
        false,
        false,
    );
}

// ── text ────────────────────────────────────────────────────────────────────

/// The UTF-8 characters of `b`, and their UTF-16 units; `None` when `b` is
/// not UTF-8.
fn utf8_counts(b: &[u8]) -> Option<(u32, u32)> {
    let (mut chars, mut units, mut i) = (0u32, 0u32, 0usize);
    while i < b.len() {
        let c = b[i];
        let (len, min) = match c {
            0x00..=0x7F => (1, 0),
            0xC2..=0xDF => (2, 0x80),
            0xE0..=0xEF => (3, 0x800),
            0xF0..=0xF4 => (4, 0x10000),
            _ => return None,
        };
        if i + len > b.len() {
            return None;
        }
        let mut cp: u32 = if len == 1 {
            c as u32
        } else {
            (c as u32) & (0x7F >> len)
        };
        for k in 1..len {
            let cc = b[i + k];
            if cc & 0xC0 != 0x80 {
                return None;
            }
            cp = (cp << 6) | (cc & 0x3F) as u32;
        }
        if cp < min || cp > 0x10FFFF || (0xD800..0xE000).contains(&cp) {
            return None;
        }
        chars += 1;
        units += if cp >= 0x10000 { 2 } else { 1 };
        i += len;
    }
    Some((chars, units))
}

/// Where the encoder writes.
pub(crate) struct Out {
    pub(crate) at: u32,
    pub(crate) cap: u32,
    pub(crate) n: u32,
    pub(crate) full: bool,
}

impl Out {
    pub(crate) fn new(at: u32, cap: u32) -> Out {
        Out {
            at,
            cap,
            n: 0,
            full: false,
        }
    }
    pub(crate) fn byte(&mut self, b: u8) {
        if self.n < self.cap {
            mem::wr_u8(self.at + self.n, b);
            self.n += 1;
        } else {
            self.full = true;
        }
    }
    pub(crate) fn bytes(&mut self, b: &[u8]) {
        for &c in b {
            self.byte(c);
        }
    }
    /// A JSON string by the pinned rule (S3).
    pub(crate) fn string(&mut self, s: &[u8]) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.byte(b'"');
        for &c in s {
            match c {
                b'"' => self.bytes(b"\\\""),
                b'\\' => self.bytes(b"\\\\"),
                0x08 => self.bytes(b"\\b"),
                0x0C => self.bytes(b"\\f"),
                b'\n' => self.bytes(b"\\n"),
                b'\r' => self.bytes(b"\\r"),
                b'\t' => self.bytes(b"\\t"),
                c if c < 0x20 => {
                    self.bytes(b"\\u00");
                    self.byte(HEX[(c >> 4) as usize]);
                    self.byte(HEX[(c & 15) as usize]);
                }
                c => self.byte(c),
            }
        }
        self.byte(b'"');
    }
}

// ── encode ──────────────────────────────────────────────────────────────────

/// Group occurrences being walked, innermost last (the count items' frames).
struct Frames {
    addr: [u32; MAX_FRAMES],
    n: usize,
}

impl Frames {
    fn new() -> Frames {
        Frames {
            addr: [0; MAX_FRAMES],
            n: 0,
        }
    }
    fn push(&mut self, a: u32) -> Result<(), i32> {
        if self.n >= MAX_FRAMES {
            return Err(E_INTERNAL);
        }
        self.addr[self.n] = a;
        self.n += 1;
        Ok(())
    }
    fn pop(&mut self) {
        self.n -= 1;
    }
    /// The address of the count item of table node `n`.
    fn count_addr(&self, n: &Node) -> Result<u32, i32> {
        let up = n.odo_up as usize;
        if up >= self.n {
            return Err(E_INTERNAL);
        }
        Ok(self.addr[self.n - 1 - up] + n.odo_rel)
    }
}

/// The current count of variable table `n`, checked against its bounds.
fn odo_count(desc: &Desc, n: &Node, frames: &Frames) -> Result<u32, i32> {
    let at = frames.count_addr(n)?;
    let cn = desc.node(n.odo_node);
    let v = load_number(desc, &cn, at).ok_or(E_INTERNAL)?;
    if v < 0 || v < n.occ_min as i128 || v > n.occ_max as i128 {
        return Err(E_ODO_RANGE);
    }
    Ok(v as u32)
}

/// Write the JSON text of the group at `addr` (described by the blob at `desc`)
/// at `out` (at most `cap` bytes). The length written, or a negated error code
/// with [`error_path`] naming the item.
pub fn encode(desc: u32, addr: u32, out: u32, cap: u32) -> i32 {
    let mut o = Out::new(out, cap);
    match encode_into(desc, addr, &mut o) {
        Err(e) => -e,
        Ok(()) if o.full => -E_NO_ROOM,
        Ok(()) => o.n as i32,
    }
}

/// [`encode`], appending to `o` (a remote call's request frame).
pub(crate) fn encode_into(desc: u32, addr: u32, o: &mut Out) -> Result<(), i32> {
    let d = Desc { d: desc };
    let root = d.node(d.root());
    path_truncate(0);
    path_push(d.name(&root));
    let mut frames = Frames::new();
    enc_item(&d, &root, addr, o, &mut frames)
}

fn enc_item(d: &Desc, n: &Node, addr: u32, o: &mut Out, frames: &mut Frames) -> Result<(), i32> {
    if n.flags & NF_TABLE == 0 {
        return enc_base(d, n, addr, o, frames);
    }
    let count = if n.flags & NF_ODO != 0 {
        odo_count(d, n, frames)?
    } else {
        n.occ_max
    };
    o.byte(b'[');
    for j in 0..count {
        if j > 0 {
            o.byte(b',');
        }
        let mark = path_len();
        path_push_index(j);
        enc_base(d, n, addr + j * n.size, o, frames)?;
        path_truncate(mark);
    }
    o.byte(b']');
    Ok(())
}

fn enc_base(d: &Desc, n: &Node, addr: u32, o: &mut Out, frames: &mut Frames) -> Result<(), i32> {
    match n.kind {
        K_GROUP => {
            o.byte(b'{');
            frames.push(addr)?;
            let mut first = true;
            for k in 0..n.n_children {
                let c = d.node(d.child(n, k));
                if c.flags & NF_MEMBER == 0 {
                    continue;
                }
                if !first {
                    o.byte(b',');
                }
                first = false;
                o.string(d.name(&c));
                o.byte(b':');
                let mark = path_len();
                path_push(b".");
                path_push(d.name(&c));
                enc_item(d, &c, addr + c.rel_off, o, frames)?;
                path_truncate(mark);
            }
            frames.pop();
            o.byte(b'}');
            Ok(())
        }
        K_NUMERIC => {
            let digits = n.limit & 0xFF;
            let scale = (n.limit >> 8) & 0xFF;
            let m = load_number(d, n, addr).ok_or(E_INTERNAL)?;
            // A value wider than the PICTURE is not canonical for it.
            match pow10(digits + scale) {
                Some(limit) if m >= limit || m <= -limit => return Err(E_INTERNAL),
                _ => {}
            }
            write_canonical(o, m, scale);
            Ok(())
        }
        _ => {
            let fd = d.fd(n);
            let b = mem::bytes(addr, fd.len);
            let end = b.iter().rposition(|c| *c != b' ').map_or(0, |p| p + 1);
            if utf8_counts(&b[..end]).is_none() {
                return Err(E_NOT_UTF8);
            }
            o.string(&b[..end]);
            Ok(())
        }
    }
}

// ── the strict reader (J1) ──────────────────────────────────────────────────

/// A JSON text in memory, already checked by [`check`] where noted.
#[derive(Clone, Copy)]
pub(crate) struct Text {
    pub(crate) at: u32,
    pub(crate) len: u32,
}

impl Text {
    pub(crate) fn get(&self, i: u32) -> Option<u8> {
        (i < self.len).then(|| mem::rd_u8(self.at + i))
    }
    pub(crate) fn ws(&self, mut i: u32) -> u32 {
        while matches!(self.get(i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            i += 1;
        }
        i
    }
}

/// Is `t` acceptable JSON? `Err(E_SYNTAX)` or `Err(E_DUPLICATE_KEY)`.
pub(crate) fn check(t: &Text) -> Result<(), i32> {
    let i = t.ws(0);
    let i = check_value(t, i, 0)?;
    if t.ws(i) != t.len {
        return Err(E_SYNTAX);
    }
    Ok(())
}

fn check_value(t: &Text, i: u32, depth: u32) -> Result<u32, i32> {
    if depth > MAX_DEPTH {
        return Err(E_SYNTAX);
    }
    match t.get(i) {
        None => Err(E_SYNTAX),
        Some(b'{') => check_object(t, i, depth),
        Some(b'[') => check_array(t, i, depth),
        Some(b'"') => check_string(t, i),
        Some(b't') => check_word(t, i, b"true"),
        Some(b'f') => check_word(t, i, b"false"),
        Some(b'n') => check_word(t, i, b"null"),
        Some(b'-' | b'0'..=b'9') => check_number(t, i),
        Some(_) => Err(E_SYNTAX),
    }
}

fn check_word(t: &Text, i: u32, w: &[u8]) -> Result<u32, i32> {
    for (k, &c) in w.iter().enumerate() {
        if t.get(i + k as u32) != Some(c) {
            return Err(E_SYNTAX);
        }
    }
    Ok(i + w.len() as u32)
}

fn digits_from(t: &Text, mut i: u32) -> u32 {
    while matches!(t.get(i), Some(b'0'..=b'9')) {
        i += 1;
    }
    i
}

fn check_number(t: &Text, mut i: u32) -> Result<u32, i32> {
    if t.get(i) == Some(b'-') {
        i += 1;
    }
    match t.get(i) {
        Some(b'0') => {
            i += 1;
            if matches!(t.get(i), Some(b'0'..=b'9')) {
                return Err(E_SYNTAX);
            }
        }
        Some(b'1'..=b'9') => i = digits_from(t, i),
        _ => return Err(E_SYNTAX),
    }
    if t.get(i) == Some(b'.') {
        let fs = i + 1;
        i = digits_from(t, fs);
        if i == fs {
            return Err(E_SYNTAX);
        }
    }
    if matches!(t.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(t.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let es = i;
        i = digits_from(t, es);
        if i == es {
            return Err(E_SYNTAX);
        }
    }
    Ok(i)
}

fn hex4(t: &Text, i: u32) -> Option<u32> {
    let mut v = 0u32;
    for k in 0..4 {
        let c = t.get(i + k)?;
        let d = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => return None,
        };
        v = v * 16 + d as u32;
    }
    Some(v)
}

/// One character of a string at `i` (after the opening quote): the code
/// point (`None` at the closing quote) and where the next one starts.
pub(crate) fn string_char(t: &Text, i: u32) -> Result<(Option<u32>, u32), i32> {
    let c = t.get(i).ok_or(E_SYNTAX)?;
    match c {
        b'"' => Ok((None, i + 1)),
        b'\\' => {
            let e = t.get(i + 1).ok_or(E_SYNTAX)?;
            let simple = match e {
                b'"' => Some(b'"' as u32),
                b'\\' => Some(b'\\' as u32),
                b'/' => Some(b'/' as u32),
                b'b' => Some(8),
                b'f' => Some(12),
                b'n' => Some(10),
                b'r' => Some(13),
                b't' => Some(9),
                b'u' => None,
                _ => return Err(E_SYNTAX),
            };
            if let Some(cp) = simple {
                return Ok((Some(cp), i + 2));
            }
            let hi = hex4(t, i + 2).ok_or(E_SYNTAX)?;
            if (0xD800..0xDC00).contains(&hi) {
                if t.get(i + 6) != Some(b'\\') || t.get(i + 7) != Some(b'u') {
                    return Err(E_SYNTAX);
                }
                let lo = hex4(t, i + 8).ok_or(E_SYNTAX)?;
                if !(0xDC00..0xE000).contains(&lo) {
                    return Err(E_SYNTAX);
                }
                Ok((
                    Some(0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)),
                    i + 12,
                ))
            } else if (0xDC00..0xE000).contains(&hi) {
                Err(E_SYNTAX)
            } else {
                Ok((Some(hi), i + 6))
            }
        }
        0x00..=0x1F => Err(E_SYNTAX),
        _ => {
            // One UTF-8 character; a text that is not UTF-8 is not JSON.
            let len = match c {
                0x00..=0x7F => 1,
                0xC2..=0xDF => 2,
                0xE0..=0xEF => 3,
                0xF0..=0xF4 => 4,
                _ => return Err(E_SYNTAX),
            };
            if i + len > t.len {
                return Err(E_SYNTAX);
            }
            let ch = utf8_counts(mem::bytes(t.at + i, len)).ok_or(E_SYNTAX)?;
            if ch.0 != 1 {
                return Err(E_SYNTAX);
            }
            let mut cp: u32 = if len == 1 {
                c as u32
            } else {
                (c as u32) & (0x7F >> len)
            };
            for k in 1..len {
                cp = (cp << 6) | (t.get(i + k).unwrap_or(0) & 0x3F) as u32;
            }
            Ok((Some(cp), i + len))
        }
    }
}

fn check_string(t: &Text, mut i: u32) -> Result<u32, i32> {
    i += 1;
    loop {
        let (c, next) = string_char(t, i)?;
        i = next;
        if c.is_none() {
            return Ok(i);
        }
    }
}

/// Are the strings at `a` and `b` (opening quotes) the same text?
fn same_string(t: &Text, mut a: u32, mut b: u32) -> bool {
    a += 1;
    b += 1;
    loop {
        let (Ok((ca, na)), Ok((cb, nb))) = (string_char(t, a), string_char(t, b)) else {
            return false;
        };
        if ca != cb {
            return false;
        }
        if ca.is_none() {
            return true;
        }
        a = na;
        b = nb;
    }
}

fn check_array(t: &Text, i: u32, depth: u32) -> Result<u32, i32> {
    let mut i = t.ws(i + 1);
    if t.get(i) == Some(b']') {
        return Ok(i + 1);
    }
    loop {
        i = t.ws(i);
        i = check_value(t, i, depth + 1)?;
        i = t.ws(i);
        match t.get(i) {
            Some(b',') => i += 1,
            Some(b']') => return Ok(i + 1),
            _ => return Err(E_SYNTAX),
        }
    }
}

fn check_object(t: &Text, i: u32, depth: u32) -> Result<u32, i32> {
    let open = i;
    let mut i = t.ws(i + 1);
    if t.get(i) == Some(b'}') {
        return Ok(i + 1);
    }
    loop {
        i = t.ws(i);
        if t.get(i) != Some(b'"') {
            return Err(E_SYNTAX);
        }
        let key = i;
        let after_key = check_string(t, i)?;
        // A repeated key: compare with every earlier key of this object.
        let mut k = members(t, open);
        while let Some((prev, _)) = k.next_member(t) {
            if prev >= key {
                break;
            }
            if same_string(t, prev, key) {
                return Err(E_DUPLICATE_KEY);
            }
        }
        i = t.ws(after_key);
        if t.get(i) != Some(b':') {
            return Err(E_SYNTAX);
        }
        i = t.ws(i + 1);
        i = check_value(t, i, depth + 1)?;
        i = t.ws(i);
        match t.get(i) {
            Some(b',') => i += 1,
            Some(b'}') => return Ok(i + 1),
            _ => return Err(E_SYNTAX),
        }
    }
}

/// The end of the (valid) value at `i`.
pub(crate) fn skip(t: &Text, i: u32) -> u32 {
    match t.get(i) {
        Some(b'"') => check_string(t, i).unwrap_or(t.len),
        Some(b'{' | b'[') => {
            let mut depth = 0u32;
            let mut j = i;
            loop {
                match t.get(j) {
                    None => return t.len,
                    Some(b'"') => {
                        j = check_string(t, j).unwrap_or(t.len);
                        continue;
                    }
                    Some(b'{' | b'[') => depth += 1,
                    Some(b'}' | b']') => {
                        depth -= 1;
                        if depth == 0 {
                            return j + 1;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
        }
        Some(b't') => i + 4,
        Some(b'f') => i + 5,
        Some(b'n') => i + 4,
        _ => check_number(t, i).unwrap_or(t.len),
    }
}

/// Walks the members of an object (or the elements of an array) whose
/// opening bracket is at `i`; the text has been checked.
pub(crate) struct Members {
    i: u32,
    done: bool,
}

pub(crate) fn members(t: &Text, open: u32) -> Members {
    let i = t.ws(open + 1);
    let done = matches!(t.get(i), Some(b'}' | b']'));
    Members { i, done }
}

impl Members {
    /// The next member: `(key, value)` for an object, `(value, value)` for an array.
    pub(crate) fn next_member(&mut self, t: &Text) -> Option<(u32, u32)> {
        if self.done {
            return None;
        }
        let i = t.ws(self.i);
        let (key, value) = if t.get(i) == Some(b'"') && {
            // An object member is a string followed by ':'.
            let e = t.ws(check_string(t, i).unwrap_or(t.len));
            t.get(e) == Some(b':')
        } {
            let e = t.ws(check_string(t, i).unwrap_or(t.len));
            (i, t.ws(e + 1))
        } else {
            (i, i)
        };
        let end = t.ws(skip(t, value));
        match t.get(end) {
            Some(b',') => self.i = end + 1,
            _ => self.done = true,
        }
        Some((key, value))
    }
}

// ── decode ──────────────────────────────────────────────────────────────────

/// A variable table a decode read: where its count item is, the length of
/// the array, its count item's node, and its path.
#[derive(Clone, Copy, Default)]
struct OdoRec {
    count_addr: u32,
    count: u32,
    node: u32,
    path_off: u32,
    path_len: u32,
}

const MAX_ODO: usize = 32;
const MAX_SUPPLIED: usize = 64;

struct Ctx {
    odo: [OdoRec; MAX_ODO],
    n_odo: usize,
    odo_text: u32,
    supplied: [u32; MAX_SUPPLIED],
    n_supplied: usize,
}

/// Read the JSON text at `text` (`len` bytes) into the group at `addr`. 0, or
/// a negated error code with [`error_path`] naming the item; on an error the
/// group is untouched.
pub fn decode(desc: u32, addr: u32, text: u32, len: u32) -> i32 {
    decode_with(desc, addr, text, len, true)
}

/// [`decode`]; with `commit` false the group is left as it is whatever the
/// answer (a remote call checks every group of an answer before it writes any).
pub fn decode_with(desc: u32, addr: u32, text: u32, len: u32, commit: bool) -> i32 {
    let d = Desc { d: desc };
    let root = d.node(d.root());
    path_truncate(0);
    path_push(d.name(&root));
    let t = Text { at: text, len };
    if let Err(e) = check(&t) {
        return -e;
    }
    let size = root.size
        * if root.flags & NF_TABLE != 0 {
            root.occ_max
        } else {
            1
        };
    if size > JSON_SHADOW_SIZE {
        return -E_INTERNAL;
    }
    let shadow = st().ws + WS_JSON_SHADOW;
    mem::copy(addr, shadow, size);
    let mut ctx = Ctx {
        odo: [OdoRec::default(); MAX_ODO],
        n_odo: 0,
        odo_text: 0,
        supplied: [0; MAX_SUPPLIED],
        n_supplied: 0,
    };
    let mut frames = Frames::new();
    let v = t.ws(0);
    let r = dec_item(&d, &root, shadow, &t, v, &mut frames, &mut ctx)
        .and_then(|()| finish_counts(&d, &ctx))
        .and_then(|()| {
            path_truncate(0);
            path_push(d.name(&root));
            let mut frames = Frames::new();
            check_counts(&d, &root, shadow, &mut frames)
        });
    match r {
        Ok(()) => {
            if commit {
                mem::copy(shadow, addr, size);
            }
            path_truncate(0);
            0
        }
        Err(e) => -e,
    }
}

fn dec_item(
    d: &Desc,
    n: &Node,
    addr: u32,
    t: &Text,
    v: u32,
    frames: &mut Frames,
    ctx: &mut Ctx,
) -> Result<(), i32> {
    if n.flags & NF_TABLE == 0 {
        return dec_base(d, n, addr, t, v, frames, ctx);
    }
    if t.get(v) != Some(b'[') {
        return Err(E_WRONG_TYPE);
    }
    let mut count = 0u32;
    let mut m = members(t, v);
    while m.next_member(t).is_some() {
        count += 1;
    }
    if count > n.occ_max {
        return Err(E_TOO_MANY);
    }
    if n.flags & NF_ODO != 0 && count < n.occ_min {
        return Err(E_TOO_FEW);
    }
    let mut m = members(t, v);
    let mut j = 0u32;
    while let Some((_, e)) = m.next_member(t) {
        let mark = path_len();
        path_push_index(j);
        dec_base(d, n, addr + j * n.size, t, e, frames, ctx)?;
        path_truncate(mark);
        j += 1;
    }
    if n.flags & NF_ODO != 0 {
        if ctx.n_odo >= MAX_ODO || ctx.odo_text + path_len() > JSON_ODO_SIZE {
            return Err(E_INTERNAL);
        }
        let at = st().ws + WS_JSON_ODO + ctx.odo_text;
        mem::copy(path_at(), at, path_len());
        ctx.odo[ctx.n_odo] = OdoRec {
            count_addr: frames.count_addr(n)?,
            count,
            node: n.odo_node,
            path_off: ctx.odo_text,
            path_len: path_len(),
        };
        ctx.odo_text += path_len();
        ctx.n_odo += 1;
    }
    Ok(())
}

fn dec_base(
    d: &Desc,
    n: &Node,
    addr: u32,
    t: &Text,
    v: u32,
    frames: &mut Frames,
    ctx: &mut Ctx,
) -> Result<(), i32> {
    let first = t.get(v);
    match n.kind {
        K_GROUP => {
            if first != Some(b'{') {
                return Err(E_WRONG_TYPE);
            }
            frames.push(addr)?;
            let mut m = members(t, v);
            while let Some((key, val)) = m.next_member(t) {
                // The key, into the path: it names the member, or the error.
                let mark = path_len();
                path_push(b".");
                let key_at = path_len();
                let mut i = key + 1;
                let mut buf = [0u8; 4];
                loop {
                    let (c, next) = string_char(t, i)?;
                    i = next;
                    let Some(cp) = c else { break };
                    let k = encode_utf8(cp, &mut buf);
                    path_push(&buf[..k]);
                }
                let key_bytes = mem::bytes(path_at() + key_at, path_len() - key_at);
                let mut found = None;
                for k in 0..n.n_children {
                    let co = d.child(n, k);
                    let c = d.node(co);
                    if c.flags & NF_MEMBER != 0 && d.name(&c) == key_bytes {
                        found = Some(c);
                        break;
                    }
                }
                let Some(c) = found else {
                    return Err(E_UNKNOWN_KEY);
                };
                dec_item(d, &c, addr + c.rel_off, t, val, frames, ctx)?;
                path_truncate(mark);
            }
            frames.pop();
            Ok(())
        }
        K_NUMERIC => {
            if !matches!(first, Some(b'-' | b'0'..=b'9')) {
                return Err(E_WRONG_TYPE);
            }
            let end = check_number(t, v)?;
            let digits = n.limit & 0xFF;
            let scale = (n.limit >> 8) & 0xFF;
            let signed = (n.limit >> 16) & 1 != 0;
            let m = number_at_scale(mem::bytes(t.at + v, end - v), digits, scale, signed)
                .map_err(|_| E_OVERFLOW)?;
            store_number(d, n, addr, m);
            if n.flags & NF_ODO_OBJECT != 0 {
                if ctx.n_supplied >= MAX_SUPPLIED {
                    return Err(E_INTERNAL);
                }
                ctx.supplied[ctx.n_supplied] = addr;
                ctx.n_supplied += 1;
            }
            Ok(())
        }
        _ => {
            if first != Some(b'"') {
                return Err(E_WRONG_TYPE);
            }
            dec_text(d, n, addr, t, v)
        }
    }
}

/// The UTF-8 bytes of code point `cp`.
fn encode_utf8(cp: u32, out: &mut [u8; 4]) -> usize {
    if cp < 0x80 {
        out[0] = cp as u8;
        1
    } else if cp < 0x800 {
        out[0] = 0xC0 | (cp >> 6) as u8;
        out[1] = 0x80 | (cp & 0x3F) as u8;
        2
    } else if cp < 0x10000 {
        out[0] = 0xE0 | (cp >> 12) as u8;
        out[1] = 0x80 | ((cp >> 6) & 0x3F) as u8;
        out[2] = 0x80 | (cp & 0x3F) as u8;
        3
    } else {
        out[0] = 0xF0 | (cp >> 18) as u8;
        out[1] = 0x80 | ((cp >> 12) & 0x3F) as u8;
        out[2] = 0x80 | ((cp >> 6) & 0x3F) as u8;
        out[3] = 0x80 | (cp & 0x3F) as u8;
        4
    }
}

/// A string into a text item (S2, S5, S6): its bytes, characters and UTF-16
/// units against the item's limits, stored padded with spaces (on the left
/// for `JUSTIFIED RIGHT`).
fn dec_text(d: &Desc, n: &Node, addr: u32, t: &Text, v: u32) -> Result<(), i32> {
    let cap = d.fd(n).len;
    let (mut bytes, mut chars, mut units) = (0u32, 0u32, 0u32);
    let mut i = v + 1;
    let mut buf = [0u8; 4];
    // The characters go straight into the item; the shadow is discarded if
    // they do not fit.
    loop {
        let (c, next) = string_char(t, i)?;
        i = next;
        let Some(cp) = c else { break };
        let k = encode_utf8(cp, &mut buf) as u32;
        if bytes + k > cap {
            return Err(E_TOO_LONG);
        }
        mem::bytes_mut(addr + bytes, k).copy_from_slice(&buf[..k as usize]);
        bytes += k;
        chars += 1;
        units += if cp >= 0x10000 { 2 } else { 1 };
    }
    let fits = match n.kind {
        K_NATIONAL => units <= n.limit,
        K_UTF8 => chars <= n.limit && (n.byte_limit == 0 || bytes <= n.byte_limit),
        _ => bytes <= n.limit,
    };
    if !fits {
        return Err(E_TOO_LONG);
    }
    if n.flags & NF_JUSTIFIED != 0 && bytes < cap {
        mem::copy(addr, addr + cap - bytes, bytes);
        mem::fill(addr, cap - bytes, b' ');
    } else {
        mem::fill(addr + bytes, cap - bytes, b' ');
    }
    Ok(())
}

/// The counts of the variable tables the text gave (R2): a count the text
/// also gave must equal the array's length; otherwise the length is stored.
fn finish_counts(d: &Desc, ctx: &Ctx) -> Result<(), i32> {
    for r in &ctx.odo[..ctx.n_odo] {
        let cn = d.node(r.node);
        let supplied = ctx.supplied[..ctx.n_supplied].contains(&r.count_addr);
        let fail = |e: i32| -> Result<(), i32> {
            path_truncate(0);
            mem::copy(st().ws + WS_JSON_ODO + r.path_off, path_at(), r.path_len);
            path_truncate(r.path_len);
            Err(e)
        };
        if supplied {
            let held = load_number(d, &cn, r.count_addr).unwrap_or(-1);
            if held != r.count as i128 {
                return fail(E_ODO_MISMATCH);
            }
        } else {
            let digits = cn.limit & 0xFF;
            let scale = (cn.limit >> 8) & 0xFF;
            match pow10(digits) {
                Some(limit) if (r.count as i128) < limit => {
                    let m = r.count as i128 * pow10(scale).unwrap_or(1);
                    store_number(d, &cn, r.count_addr, m);
                }
                _ => return fail(E_ODO_RANGE),
            }
        }
    }
    Ok(())
}

/// Every variable table's count, in every occurrence, lies in its bounds.
fn check_counts(d: &Desc, n: &Node, addr: u32, frames: &mut Frames) -> Result<(), i32> {
    if n.flags & NF_TABLE != 0 {
        if n.flags & NF_ODO != 0 {
            odo_count(d, n, frames)?;
        }
        for j in 0..n.occ_max {
            let mark = path_len();
            path_push_index(j);
            check_base(d, n, addr + j * n.size, frames)?;
            path_truncate(mark);
        }
        return Ok(());
    }
    check_base(d, n, addr, frames)
}

fn check_base(d: &Desc, n: &Node, addr: u32, frames: &mut Frames) -> Result<(), i32> {
    if n.kind != K_GROUP {
        return Ok(());
    }
    frames.push(addr)?;
    for k in 0..n.n_children {
        let c = d.node(d.child(n, k));
        if c.flags & NF_MEMBER == 0 {
            continue;
        }
        let mark = path_len();
        path_push(b".");
        path_push(d.name(&c));
        check_counts(d, &c, addr + c.rel_off, frames)?;
        path_truncate(mark);
    }
    frames.pop();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_take_the_reference_forms() {
        assert_eq!(number_at_scale(b"12.5", 5, 2, true), Ok(1250));
        assert_eq!(number_at_scale(b"-0.05", 3, 2, true), Ok(-5));
        assert_eq!(number_at_scale(b"-0.00", 3, 2, false), Ok(0));
        assert_eq!(number_at_scale(b"1e2", 3, 0, false), Ok(100));
        assert_eq!(number_at_scale(b"1.5E+1", 3, 0, false), Ok(15));
        assert_eq!(number_at_scale(b"007", 3, 0, false), Ok(7));
        assert_eq!(number_at_scale(b"1.500", 2, 2, false), Ok(150));
        assert_eq!(
            number_at_scale(b"12345678901234567890123456789012345678", 38, 0, false),
            Ok(12345678901234567890123456789012345678)
        );
        assert_eq!(
            number_at_scale(b"1.505", 3, 2, true),
            Err(NumErr::TooManyDecimals)
        );
        assert_eq!(
            number_at_scale(b"1000", 3, 0, true),
            Err(NumErr::TooManyDigits)
        );
        assert_eq!(
            number_at_scale(b"-1", 3, 0, false),
            Err(NumErr::NegativeUnsigned)
        );
        assert_eq!(
            number_at_scale(b"1e9999", 3, 0, true),
            Err(NumErr::OutOfRange)
        );
        assert_eq!(number_at_scale(b"5e-3", 3, 3, false), Ok(5));
    }
}
