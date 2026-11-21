// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `INSPECT`, `STRING` and `UNSTRING`.
//!
//! Each statement is described to the routine by a **spec record** the
//! generator lays out in the constant area (little-endian `u32`s, operand
//! records referred to by address; 0 means "absent"):
//!
//! ```text
//! INSPECT   n, then n × 6:  kind, counter, pattern, by, before, after
//!             kind: 0..3 TALLY chars/all/leading/trailing
//!                   4..8 REPLACE chars/all/leading/trailing/first
//!                   9    CONVERT  (pattern = FROM, by = TO)
//! STRING    n, into, pointer, then n × 2:  source, delimiter
//! UNSTRING  flags(bit0 ALL), source, pointer, tallying, nd, nd × delimiter,
//!           nt, then nt × 3:  receiver, delimiter-in, count-in
//! ```
//!
//! The algorithms are the interpreter's (`exec_inspect`, `exec_string`,
//! `exec_unstring`), which follow COBOL-85 6.17, 6.25 and 6.28: a series of
//! `TALLYING` operands shares one left-to-right scan, as does a series of
//! `REPLACING` operands; `UNSTRING` takes the earliest delimiter.

use crate::codec;
use crate::fd::*;
use crate::mem;
use crate::mv::{self, digit_chars};
use crate::nat::{Class, Text};
use crate::opd::*;
use crate::state::{err, st, SCRATCH_SIZE, WS_SCRATCH};
use crate::val::Val;
use cobolt_decimal::CobolNumeric;

const MAX_OPS: usize = 16;

/// Bump allocation in the workspace scratch area, for one statement.
struct Scratch {
    base: u32,
    used: u32,
}

impl Scratch {
    fn new() -> Scratch {
        Scratch { base: st().ws + WS_SCRATCH, used: 0 }
    }
    fn alloc(&mut self, n: u32) -> u32 {
        if self.used + n > SCRATCH_SIZE {
            crate::fail(err::SCRATCH_OVERFLOW);
        }
        let at = self.base + self.used;
        self.used += n;
        at
    }
    fn put(&mut self, bytes: &[u8]) -> (u32, u32) {
        let at = self.alloc(bytes.len().max(1) as u32);
        mem::bytes_mut(at, bytes.len() as u32).copy_from_slice(bytes);
        (at, bytes.len() as u32)
    }
    /// `n` code points, on a word boundary (T50a2).
    fn units(&mut self, n: usize) -> &'static mut [u32] {
        let pad = (4 - (self.base + self.used) % 4) % 4;
        self.alloc(pad);
        let at = self.alloc((n * 4).max(4) as u32);
        mem::words_mut(at, n as u32)
    }
    /// The characters of `t` as UTF-8 (T50a2): a national or UTF-8 item's text, as the interpreter's
    /// slot holds it.
    fn put_text(&mut self, t: Text) -> (u32, u32) {
        let n: u32 = t.chars().map(|c| c.len_utf8() as u32).sum();
        let at = self.alloc(n.max(1));
        let out = mem::bytes_mut(at, n);
        let mut i = 0usize;
        for c in t.chars() {
            i += c.encode_utf8(&mut out[i..]).len();
        }
        (at, n)
    }
}

/// The characters an operand contributes to `STRING`, `INSPECT` patterns and
/// `UNSTRING` delimiters: an item's or literal's bytes, a figurative's single
/// character, a number's decimal text (a numeric *item* gives its digits).
fn chars_of(base: u32, o: &Opd, sc: &mut Scratch) -> (u32, u32) {
    match o.tag {
        T_LIT_ALNUM | T_ALL => (o.off, o.len),
        T_FIG => sc.put(&[fig_byte(o.fig)]),
        T_LIT_NUM => {
            let v = value(base, o);
            let mut buf = [0u8; 64];
            let n = match v {
                Val::Dec(d) => crate::disp::format_dec(&d, &mut buf),
                Val::F(f) => crate::float::format_f64(f, &mut buf),
            };
            sc.put(&buf[..n])
        }
        _ => {
            let loc = locate(base, o);
            // A national or UTF-8 item gives its text, fitted and padded, in UTF-8 (T50a2).
            if let Some(class) = Class::of(&loc.fd) {
                return sc.put_text(Text::Item(class, mem::bytes(loc.addr, loc.len)));
            }
            if loc.fd.is_numeric() && !matches!(loc.fd.kind, K_NUM_DISPLAY) {
                let v = codec::load_value(&loc);
                let mut buf = [0u8; 40];
                let n = digit_chars(&v, &loc.fd, &mut buf);
                sc.put(&buf[..n])
            } else {
                (loc.addr, loc.len)
            }
        }
    }
}

fn find<U: PartialEq>(hay: &[U], needle: &[U], from: usize) -> Option<usize> {
    if needle.is_empty() || from > hay.len() || needle.len() > hay.len() - from {
        return None;
    }
    let mut i = from;
    while i + needle.len() <= hay.len() {
        if &hay[i..i + needle.len()] == needle {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn rd(addr: u32) -> u32 {
    mem::rd_u32(addr)
}

/// What `INSPECT` and `UNSTRING` scan: the bytes of an alphanumeric item, or the code points of a
/// national or UTF-8 item's text (T50a2: the interpreter's `inspect_units`). One algorithm serves
/// both, so the byte rules stay exactly what they were.
trait Unit: Copy + PartialEq + 'static {
    const SPACE: Self;
    /// The units of operand `o`'s characters.
    fn of_operand(base: u32, o: &Opd, sc: &mut Scratch) -> &'static [Self];
    /// A buffer of `n` units in the scratch area.
    fn buffer(sc: &mut Scratch, n: usize) -> &'static mut [Self];
    /// The UTF-8 text of `units`, for a receiver that stores text.
    fn text(units: &[Self], sc: &mut Scratch) -> (u32, u32);
    /// What a numeric receiver of an `UNSTRING` takes the field as.
    fn number(field: &[Self]) -> i128;
}

impl Unit for u8 {
    const SPACE: u8 = b' ';
    fn of_operand(base: u32, o: &Opd, sc: &mut Scratch) -> &'static [u8] {
        let (a, l) = chars_of(base, o, sc);
        mem::bytes(a, l)
    }
    fn buffer(sc: &mut Scratch, n: usize) -> &'static mut [u8] {
        let at = sc.alloc(n.max(1) as u32);
        mem::bytes_mut(at, n as u32)
    }
    fn text(units: &[u8], sc: &mut Scratch) -> (u32, u32) {
        sc.put(units)
    }
    fn number(field: &[u8]) -> i128 {
        // The digits after leading spaces, up to the first character that is not one.
        let mut m: i128 = 0;
        for &c in field.iter().skip_while(|c| **c == b' ') {
            if c.is_ascii_digit() {
                m = m.saturating_mul(10).saturating_add((c - b'0') as i128);
            } else {
                break;
            }
        }
        m
    }
}

impl Unit for u32 {
    const SPACE: u32 = 0x20;
    fn of_operand(base: u32, o: &Opd, sc: &mut Scratch) -> &'static [u32] {
        let (a, l) = chars_of(base, o, sc);
        decode(mem::bytes(a, l), sc)
    }
    fn buffer(sc: &mut Scratch, n: usize) -> &'static mut [u32] {
        sc.units(n)
    }
    fn text(units: &[u32], sc: &mut Scratch) -> (u32, u32) {
        sc.put_text(Text::Units(units))
    }
    fn number(field: &[u32]) -> i128 {
        // The interpreter's `field.trim().parse::<i128>().unwrap_or(0)`: a whole number, a sign allowed.
        let blank = |u: u32| char::from_u32(u).is_some_and(char::is_whitespace);
        let (mut a, mut b) = (0, field.len());
        while a < b && blank(field[a]) {
            a += 1;
        }
        while b > a && blank(field[b - 1]) {
            b -= 1;
        }
        let t = &field[a..b];
        let (neg, digits) = match t.first() {
            Some(0x2D) => (true, &t[1..]),
            Some(0x2B) => (false, &t[1..]),
            _ => (false, t),
        };
        if digits.is_empty() {
            return 0;
        }
        let mut m: i128 = 0;
        for &d in digits {
            if !(0x30..=0x39).contains(&d) {
                return 0;
            }
            m = match m.checked_mul(10).and_then(|v| v.checked_add((d - 0x30) as i128)) {
                Some(v) => v,
                None => return 0,
            };
        }
        if neg {
            -m
        } else {
            m
        }
    }
}

/// The code points of UTF-8 `bytes`, an ill-formed sequence one U+FFFD (`from_utf8_lossy`).
fn decode(bytes: &[u8], sc: &mut Scratch) -> &'static [u32] {
    let n = crate::nat::Lossy::new(bytes).count();
    let u = sc.units(n);
    for (slot, c) in u.iter_mut().zip(crate::nat::Lossy::new(bytes)) {
        *slot = c as u32;
    }
    u
}

/// The `[lo, hi)` window of `BEFORE`/`AFTER INITIAL` over `s`.
fn window<U: Unit>(base: u32, s: &[U], before: u32, after: u32, sc: &mut Scratch) -> (usize, usize) {
    let lo = if after != 0 {
        let d = U::of_operand(base, &parse(after), sc);
        match find(s, d, 0) {
            Some(p) => p + d.len(),
            None => s.len(),
        }
    } else {
        0
    };
    let hi = if before != 0 {
        let d = U::of_operand(base, &parse(before), sc);
        match find(s, d, lo.min(s.len())) {
            Some(p) => p,
            None => s.len(),
        }
    } else {
        s.len()
    };
    (lo.min(s.len()), hi.max(lo).min(s.len()))
}

/// The length of the run of `pat` that ends the window (`TRAILING`).
fn trailing_run<U: PartialEq>(window: &[U], pat: &[U]) -> usize {
    let mut n = 0;
    let mut end = window.len();
    while !pat.is_empty() && end >= pat.len() && &window[end - pat.len()..end] == pat {
        n += 1;
        end -= pat.len();
    }
    n
}

#[derive(Clone, Copy)]
struct Op<U: Unit> {
    kind: u32,
    counter: u32,
    pat: &'static [U],
    by: &'static [U],
    lo: usize,
    hi: usize,
    leading_next: usize,
    leading_live: bool,
    trail_start: usize,
    spent: bool,
}

const TALLY_CHARS: u32 = 0;
const TALLY_ALL: u32 = 1;
const TALLY_LEADING: u32 = 2;
const TALLY_TRAILING: u32 = 3;
const REPL_CHARS: u32 = 4;
const REPL_ALL: u32 = 5;
const REPL_LEADING: u32 = 6;
const REPL_TRAILING: u32 = 7;
const REPL_FIRST: u32 = 8;
const CONVERT: u32 = 9;

fn blank_op<U: Unit>() -> Op<U> {
    Op { kind: 0, counter: 0, pat: &[], by: &[], lo: 0, hi: 0, leading_next: 0, leading_live: true, trail_start: 0, spent: false }
}

/// `INSPECT target <spec>`.
///
/// A signed numeric item with its sign in the last (or first) digit is
/// inspected as its digit characters — `-12345` holds "12345", and `ALL "5"`
/// finds the 5 (NC216A INS-TEST-F1-23). The sign position is made a plain digit
/// for the scan and given its sign back afterwards.
pub fn inspect(base: u32, target: u32, spec: u32) {
    let t = locate(base, &parse(target));
    let embedded = if t.fd.kind == K_NUM_DISPLAY && t.fd.signed() && !t.fd.sign_separate() && t.len > 0 {
        let at = t.addr + if t.fd.sign_leading() { 0 } else { t.len - 1 };
        codec::de_overpunch(mem::rd_u8(at)).map(|(d, neg)| {
            mem::wr_u8(at, b'0' + d);
            (at, neg)
        })
    } else {
        None
    };
    let mut sc = Scratch::new();
    let core = inspect_core::<u8>(base, mem::bytes_mut(t.addr, t.len), spec, &mut sc, t.len as usize, false);
    // Back into the item: left-justified, space-padded.
    if let Some(out) = core.replaced {
        mem::bytes_mut(t.addr, out.len() as u32).copy_from_slice(out);
        if (out.len() as u32) < t.len {
            mem::fill(t.addr + out.len() as u32, t.len - out.len() as u32, b' ');
        }
    }
    if let Some((at, true)) = embedded {
        let b = mem::rd_u8(at);
        if b.is_ascii_digit() {
            mem::wr_u8(at, codec::overpunch(b - b'0', true));
        }
    }
}

/// `INSPECT` of a national or UTF-8 item, by character (T50a2): the interpreter inspects its text,
/// so positions, `CHARACTERS` and `BEFORE`/`AFTER` count characters, and the result is stored
/// through the item's class. `rm_start` (0: none) and `rm_len` (0: to the end) are a reference
/// modification of the target, in characters.
pub fn inspect_text(base: u32, target: u32, spec: u32, rm_start: u32, rm_len: u32) {
    let t = locate(base, &parse(target));
    let Some(class) = Class::of(&t.fd) else { crate::fail(err::BAD_OPERAND) };
    let mut sc = Scratch::new();
    let text = Text::Item(class, mem::bytes(t.addr, t.len));
    let n = text.chars().count();
    let units = sc.units(n);
    for (slot, c) in units.iter_mut().zip(text.chars()) {
        *slot = c as u32;
    }
    // The interpreter's reference modification here: `start` from 1, a missing `length` the rest.
    let (begin, end) = if rm_start != 0 {
        let first = crate::deskio::whole_number_or(base, rm_start, 1).max(1) as usize;
        let begin = (first - 1).min(n);
        let len = if rm_len != 0 { crate::deskio::whole_number_or(base, rm_len, 0).max(0) as usize } else { n - begin };
        (begin, (begin + len).min(n))
    } else {
        (0, n)
    };
    let justified = t.fd.justified();
    // A left-justified result keeps its first `positions` characters, so no more are built; a
    // right-justified one keeps its last, so all of them are.
    let out_cap = if justified { (SCRATCH_SIZE as usize).saturating_sub(sc.used as usize + 16) / 4 } else { class.positions() as usize };
    let core = inspect_core::<u32>(base, &mut units[begin..end], spec, &mut sc, out_cap, justified);
    let store = |text: Text| crate::nat::write(class, text, justified, mem::bytes_mut(t.addr, t.len));
    match core.replaced {
        Some(out) if begin == 0 && end == n => store(Text::Units(out)),
        Some(out) => {
            let all = sc.units(begin + out.len() + (n - end));
            all[..begin].copy_from_slice(&units[..begin]);
            all[begin..begin + out.len()].copy_from_slice(out);
            all[begin + out.len()..].copy_from_slice(&units[end..]);
            store(Text::Units(all));
        }
        None if core.converted => store(Text::Units(units)),
        None => {}
    }
}

/// What an `INSPECT` did: the text a `REPLACING` made (`None` when there was none), and whether a
/// `CONVERTING` changed the operand in place.
struct Core<U: Unit> {
    replaced: Option<&'static [U]>,
    converted: bool,
}

/// The scan of an `INSPECT` over `buf`: `TALLYING` stores its counters, `REPLACING` builds its result
/// in a buffer of `out_cap` units (`strict`: more than that is an error, not a cut) and `CONVERTING`
/// changes `buf`.
fn inspect_core<U: Unit>(base: u32, buf: &mut [U], spec: u32, sc: &mut Scratch, out_cap: usize, strict: bool) -> Core<U> {
    let n = rd(spec) as usize;
    if n > MAX_OPS {
        crate::fail(err::BAD_OPERAND);
    }
    let s: &[U] = &*buf;
    let mut ops = [blank_op::<U>(); MAX_OPS];
    for (k, op) in ops.iter_mut().enumerate().take(n) {
        let r = spec + 4 + (k as u32) * 24;
        op.kind = rd(r);
        op.counter = rd(r + 4);
        let pat = rd(r + 8);
        let by = rd(r + 12);
        let (lo, hi) = window(base, s, rd(r + 16), rd(r + 20), sc);
        op.lo = lo;
        op.hi = hi;
        op.leading_next = lo;
        if pat != 0 {
            op.pat = U::of_operand(base, &parse(pat), sc);
        }
        if by != 0 {
            op.by = U::of_operand(base, &parse(by), sc);
        }
        op.trail_start = hi;
        if matches!(op.kind, TALLY_TRAILING | REPL_TRAILING) && !op.pat.is_empty() {
            op.trail_start = hi - trailing_run(&s[lo..hi], op.pat) * op.pat.len();
        }
    }
    // ── TALLYING: one shared scan ──────────────────────────────────────
    let mut counts = [0i64; MAX_OPS];
    let mut have_tally = false;
    for op in ops.iter().take(n) {
        if op.kind <= TALLY_TRAILING {
            have_tally = true;
        }
    }
    if have_tally {
        let mut p = 0usize;
        while p < s.len() {
            let mut taken = 0usize;
            for (k, op) in ops.iter_mut().enumerate().take(n) {
                if op.kind > TALLY_TRAILING || p < op.lo || p >= op.hi {
                    continue;
                }
                let pat = op.pat;
                let fits = !pat.is_empty() && p + pat.len() <= op.hi && &s[p..p + pat.len()] == pat;
                taken = match op.kind {
                    TALLY_CHARS => 1,
                    TALLY_ALL if fits => pat.len(),
                    TALLY_LEADING if fits && op.leading_live && p == op.leading_next => {
                        op.leading_next = p + pat.len();
                        pat.len()
                    }
                    TALLY_TRAILING if fits && p >= op.trail_start && (p - op.trail_start) % pat.len() == 0 => pat.len(),
                    _ => 0,
                };
                if taken > 0 {
                    counts[k] += 1;
                    break;
                }
            }
            p += taken.max(1);
            for op in ops.iter_mut().take(n) {
                if p > op.leading_next {
                    op.leading_live = false;
                }
            }
        }
        // Sum per counter operand and store once each: operands sharing one counter were tallied
        // separately above. The counter accumulates onto its value.
        let mut done = [0u32; MAX_OPS];
        for k in 0..n {
            if ops[k].kind > TALLY_TRAILING || done.contains(&ops[k].counter) {
                continue;
            }
            let c = ops[k].counter;
            let mut total = 0i64;
            for j in 0..n {
                if ops[j].kind <= TALLY_TRAILING && ops[j].counter == c {
                    total += counts[j];
                }
            }
            done[k] = c;
            let cl = locate(base, &parse(c));
            let start = match codec::load_value(&cl) {
                Val::Dec(d) => (d.mantissa / cobolt_decimal::pow10(d.decimals as u32).unwrap_or(1)) as i64,
                Val::F(f) => f as i64,
            };
            codec::store_value(&cl, Val::int(start + total), false, false);
        }
    }
    // ── REPLACING: one shared scan ─────────────────────────────────────
    // The tallying scan walked each LEADING run; the replacing scan starts its
    // own runs again from the left edge of each window.
    for op in ops.iter_mut().take(n) {
        op.leading_next = op.lo;
        op.leading_live = true;
    }
    let mut have_repl = false;
    for op in ops.iter().take(n) {
        if (REPL_CHARS..=REPL_FIRST).contains(&op.kind) {
            have_repl = true;
        }
    }
    let mut replaced = None;
    if have_repl {
        let out = U::buffer(sc, out_cap);
        let mut o = 0usize; // units written to `out`
        let cap = out.len();
        let mut put = |u: U, o: &mut usize| {
            if *o < cap {
                out[*o] = u;
                *o += 1;
            } else if strict {
                crate::fail(err::SCRATCH_OVERFLOW);
            }
        };
        let mut p = 0usize;
        while p < s.len() {
            let mut taken = 0usize;
            for op in ops.iter_mut().take(n) {
                if !(REPL_CHARS..=REPL_FIRST).contains(&op.kind) || p < op.lo || p >= op.hi || op.spent {
                    continue;
                }
                let pat = op.pat;
                let fits = !pat.is_empty() && p + pat.len() <= op.hi && &s[p..p + pat.len()] == pat;
                taken = match op.kind {
                    REPL_CHARS => 1,
                    REPL_ALL if fits => pat.len(),
                    REPL_FIRST if fits => {
                        op.spent = true;
                        pat.len()
                    }
                    REPL_LEADING if fits && op.leading_live && p == op.leading_next => {
                        op.leading_next = p + pat.len();
                        pat.len()
                    }
                    REPL_TRAILING if fits && p >= op.trail_start && (p - op.trail_start) % pat.len() == 0 => pat.len(),
                    _ => 0,
                };
                if taken > 0 {
                    if op.kind == REPL_CHARS {
                        // `BY` for CHARACTERS is a single character, written over each position in turn.
                        put(op.by.first().copied().unwrap_or(U::SPACE), &mut o);
                    } else {
                        for &u in op.by {
                            put(u, &mut o);
                        }
                    }
                    break;
                }
            }
            if taken == 0 {
                put(s[p], &mut o);
                p += 1;
            } else {
                p += taken;
            }
            for op in ops.iter_mut().take(n) {
                if p > op.leading_next {
                    op.leading_live = false;
                }
            }
        }
        replaced = Some(&out[..o]);
    }
    // ── CONVERTING ─────────────────────────────────────────────────────
    let mut converted = false;
    for k in 0..n {
        if ops[k].kind != CONVERT {
            continue;
        }
        converted = true;
        // The interpreter converts character by character in pair order: each `from` character is
        // replaced by its `to` character over the whole window.
        let (from, to) = (ops[k].pat, ops[k].by);
        let (lo, hi) = (ops[k].lo, ops[k].hi);
        for (fc, tc) in from.iter().zip(to.iter()) {
            for b in buf[lo..hi].iter_mut() {
                if *b == *fc {
                    *b = *tc;
                }
            }
        }
    }
    Core { replaced, converted }
}

/// `STRING … INTO …`; returns 1 on overflow.
pub fn string(base: u32, spec: u32) -> u32 {
    let n = rd(spec) as usize;
    let into = locate(base, &parse(rd(spec + 4)));
    let ptr_opd = rd(spec + 8);
    let mut sc = Scratch::new();
    let cap = into.len as usize;
    let mut start: usize = 1;
    let ptr_loc = if ptr_opd != 0 { Some(locate(base, &parse(ptr_opd))) } else { None };
    if let Some(pl) = &ptr_loc {
        let v = match codec::load_value(pl) {
            Val::Dec(d) => (d.mantissa / cobolt_decimal::pow10(d.decimals as u32).unwrap_or(1)) as i64,
            Val::F(f) => f as i64,
        };
        start = v.max(1) as usize;
    }
    let mut idx = start - 1;
    let mut overflow = false;
    let mut placed = 0usize;
    'sources: for k in 0..n {
        let r = spec + 12 + (k as u32) * 8;
        let so = parse(rd(r));
        let dopd = rd(r + 4);
        let (sa, sl) = chars_of(base, &so, &mut sc);
        let src = mem::bytes(sa, sl);
        // How much of the source is transferred.
        let take: usize = if dopd == 0 {
            src.len()
        } else {
            let d = parse(dopd);
            let size = d.tag == T_LIT_ALNUM && mem::bytes(d.off, d.len) == b"SIZE";
            if size {
                src.len()
            } else {
                let (da, dl) = chars_of(base, &d, &mut sc);
                let delim = mem::bytes(da, dl);
                match find(src, delim, 0) {
                    Some(p) => p,
                    None => src.len(),
                }
            }
        };
        for &b in &src[..take] {
            if idx >= cap {
                overflow = true;
                break 'sources;
            }
            mem::wr_u8(into.addr + idx as u32, b);
            idx += 1;
            placed += 1;
        }
    }
    // A pointer that starts past the receiver overflows when anything was to be moved.
    if start - 1 >= cap && n > 0 {
        overflow = true;
    }
    if let Some(pl) = &ptr_loc {
        codec::store_value(pl, Val::int((start + placed) as i64), false, false);
    }
    overflow as u32
}

/// `STRING … INTO <national or UTF-8 item>` (T50a2): the interpreter assembles the senders' bytes,
/// then fills the receiver by character, so the `POINTER`, the size and the overflow test count
/// characters. Returns 1 on overflow.
pub fn string_text(base: u32, spec: u32) -> u32 {
    let n = rd(spec) as usize;
    let into = locate(base, &parse(rd(spec + 4)));
    let Some(class) = Class::of(&into.fd) else { crate::fail(err::BAD_OPERAND) };
    let ptr_opd = rd(spec + 8);
    let cap = class.positions() as usize;
    let mut sc = Scratch::new();
    // The characters past the receiver's size are never stored, and a character is at most four
    // bytes, so the assembled bytes are kept up to that many: the rest only add to the overflow.
    // A right-justified receiver keeps the last characters of what is built, so it needs them all.
    let justified = into.fd.justified();
    let limit = if justified { (SCRATCH_SIZE as usize - 8 * cap - 64) / 2 } else { 4 * (cap + 1) };
    let res_at = sc.alloc(limit as u32);
    let mut res_len = 0usize;
    for k in 0..n {
        let r = spec + 12 + (k as u32) * 8;
        let so = parse(rd(r));
        let dopd = rd(r + 4);
        let (sa, sl) = chars_of(base, &so, &mut sc);
        let src = mem::bytes(sa, sl);
        let take = if dopd == 0 {
            // No `DELIMITED BY`: an alphanumeric item (a national or UTF-8 one is) drops its trailing
            // spaces; any other sender is moved whole.
            if so.tag == T_ITEM && (so.fd.kind == K_ALNUM || Class::of(&so.fd).is_some()) {
                trim_trailing_spaces(src).len()
            } else {
                src.len()
            }
        } else {
            let (da, dl) = chars_of(base, &parse(dopd), &mut sc);
            let delim = mem::bytes(da, dl);
            // SIZE and SPACE(S) are named delimiters, so they are recognised by their spelling.
            let named = core::str::from_utf8(delim).map(str::trim).unwrap_or("");
            if named.eq_ignore_ascii_case("SIZE") {
                src.len()
            } else if named.eq_ignore_ascii_case("SPACE") || named.eq_ignore_ascii_case("SPACES") {
                trim_trailing_spaces(src).len()
            } else {
                find(src, delim, 0).unwrap_or(src.len())
            }
        };
        let room = limit - res_len;
        let put = take.min(room);
        if put < take && justified {
            crate::fail(err::SCRATCH_OVERFLOW);
        }
        mem::bytes_mut(res_at + res_len as u32, put as u32).copy_from_slice(&src[..put]);
        res_len += put;
    }
    let result = mem::bytes(res_at, res_len as u32);
    let text_len = crate::nat::Lossy::new(result).count();
    let ptr_loc = if ptr_opd != 0 { Some(locate(base, &parse(ptr_opd))) } else { None };
    let mut overflow;
    let mut placed = 0usize;
    let mut start = 1usize;
    // The receiver's characters as they are, then the senders' over them from the pointer.
    let dest = sc.units(cap);
    let dest_text = Text::Item(class, mem::bytes(into.addr, into.len));
    let (mut have, mut cur) = (0usize, dest_text.chars());
    while have < cap {
        match cur.next() {
            Some(c) => dest[have] = c as u32,
            None => break,
        }
        have += 1;
    }
    for slot in dest.iter_mut().take(cap).skip(have) {
        *slot = 0x20;
    }
    let out_units: &[u32] = if let Some(pl) = &ptr_loc {
        start = (load_int(pl) as i64).max(1) as usize;
        let mut idx = start - 1;
        overflow = start - 1 >= cap && text_len > 0;
        for c in crate::nat::Lossy::new(result) {
            if idx >= cap {
                overflow = true;
                break;
            }
            dest[idx] = c as u32;
            idx += 1;
            placed += 1;
        }
        dest
    } else {
        overflow = text_len > cap;
        let text = sc.units(if justified { text_len } else { text_len.min(cap) });
        for (slot, c) in text.iter_mut().zip(crate::nat::Lossy::new(result)) {
            *slot = c as u32;
        }
        text
    };
    crate::nat::write(class, Text::Units(out_units), justified, mem::bytes_mut(into.addr, into.len));
    if let Some(pl) = &ptr_loc {
        codec::store_value(pl, Val::int((start + placed) as i64), false, false);
    }
    overflow as u32
}

/// An item's integer value, as the interpreter's `get_i64` reads it (the whole part).
fn load_int(l: &Loc) -> i64 {
    match codec::load_value(l) {
        Val::Dec(d) => (d.mantissa / cobolt_decimal::pow10(d.decimals as u32).unwrap_or(1)) as i64,
        Val::F(f) => f as i64,
    }
}

/// `s` with its trailing spaces removed.
fn trim_trailing_spaces(s: &[u8]) -> &[u8] {
    let end = s.iter().rposition(|&b| b != b' ').map_or(0, |i| i + 1);
    &s[..end]
}

/// The next delimiter at or after `from`: `(offset, run length, one length)`.
fn find_delimiter<U: Unit>(src: &[U], from: usize, delims: &[&[U]], all: bool) -> Option<(usize, usize, usize)> {
    let mut best: Option<(usize, usize, usize)> = None;
    for &d in delims {
        let mut i = from;
        while i + d.len() <= src.len() {
            if &src[i..i + d.len()] == d {
                let mut len = d.len();
                if all {
                    while i + len + d.len() <= src.len() && &src[i + len..i + len + d.len()] == d {
                        len += d.len();
                    }
                }
                if best.map_or(true, |(bi, _, _)| i < bi) {
                    best = Some((i, len, d.len()));
                }
                break;
            }
            i += 1;
        }
    }
    best
}

/// `UNSTRING … ` of an alphanumeric source; returns 1 on overflow.
pub fn unstring(base: u32, spec: u32) -> u32 {
    let from = locate(base, &parse(rd(spec + 4)));
    let mut sc = Scratch::new();
    unstring_core::<u8>(base, spec, mem::bytes(from.addr, from.len), &mut sc)
}

/// `UNSTRING … ` of a national or UTF-8 source (T50a2): the interpreter splits its text by
/// character, so the `POINTER`, `COUNT IN` and every position count characters. Returns 1 on overflow.
pub fn unstring_text(base: u32, spec: u32) -> u32 {
    let from = locate(base, &parse(rd(spec + 4)));
    let mut sc = Scratch::new();
    let text = match Class::of(&from.fd) {
        Some(class) => Text::Item(class, mem::bytes(from.addr, from.len)),
        // An `N"…"`/`U"…"` literal is its text.
        None => Text::Utf8(mem::bytes(from.addr, from.len)),
    };
    let n = text.chars().count();
    let units = sc.units(n);
    for (slot, c) in units.iter_mut().zip(text.chars()) {
        *slot = c as u32;
    }
    unstring_core::<u32>(base, spec, units, &mut sc)
}

fn unstring_core<U: Unit>(base: u32, spec: u32, src: &[U], sc: &mut Scratch) -> u32 {
    let all = rd(spec) & 1 != 0;
    let ptr_opd = rd(spec + 8);
    let tally_opd = rd(spec + 12);
    let nd = rd(spec + 16) as usize;
    let mut delims: [&[U]; 16] = [&[]; 16];
    let mut ndel = 0;
    for k in 0..nd.min(16) {
        let d = U::of_operand(base, &parse(rd(spec + 20 + (k as u32) * 4)), sc);
        if !d.is_empty() {
            delims[ndel] = d;
            ndel += 1;
        }
    }
    let delims = &delims[..ndel];
    let nt_at = spec + 20 + (nd as u32) * 4;
    let nt = rd(nt_at) as usize;
    let ptr_loc = if ptr_opd != 0 { Some(locate(base, &parse(ptr_opd))) } else { None };
    let cursor: i64 = match &ptr_loc {
        Some(pl) => load_int(pl),
        None => 1,
    };
    if cursor < 1 || cursor as usize > src.len() {
        return 1;
    }
    let mut pos = (cursor - 1) as usize;
    let mut filled = 0usize;
    for k in 0..nt {
        if pos >= src.len() {
            break;
        }
        let r = nt_at + 4 + (k as u32) * 12;
        let recv = locate(base, &parse(rd(r)));
        let (field_end, delim_len, unit_len) = if delims.is_empty() {
            // No `DELIMITED BY`: the source is split by the receivers' own sizes. A national or
            // UTF-8 source counts characters against a national or UTF-8 receiver's positions.
            let width = match Class::of(&recv.fd) {
                Some(class) if core::mem::size_of::<U>() == 4 => class.positions() as usize,
                _ => recv.len as usize,
            }
            .max(1);
            ((pos + width).min(src.len()), 0, 0)
        } else {
            match find_delimiter(src, pos, delims, all) {
                Some((at, len, unit)) => (at, len, unit),
                None => (src.len(), 0, 0),
            }
        };
        store_field(&recv, &src[pos..field_end], sc);
        let din = rd(r + 4);
        if din != 0 {
            store_field(&locate(base, &parse(din)), &src[field_end..field_end + unit_len], sc);
        }
        let cnt = rd(r + 8);
        if cnt != 0 {
            let cl = locate(base, &parse(cnt));
            codec::store_value(&cl, Val::int((field_end - pos) as i64), false, false);
        }
        pos = field_end + delim_len;
        filled += 1;
    }
    if let Some(pl) = &ptr_loc {
        codec::store_value(pl, Val::int(pos as i64 + 1), false, false);
    }
    if tally_opd != 0 {
        let tl = locate(base, &parse(tally_opd));
        codec::store_value(&tl, Val::int(load_int(&tl) + filled as i64), false, false);
    }
    (pos < src.len()) as u32
}

/// What a field of an `UNSTRING` becomes in its receiver: under `MOVE` rules, a number converts, a
/// national or UTF-8 item takes the characters through its class, any other is padded or cut.
fn store_field<U: Unit>(recv: &Loc, field: &[U], sc: &mut Scratch) {
    if recv.fd.is_numeric() {
        codec::store_value(recv, Val::Dec(CobolNumeric::new(U::number(field), 0)), false, false);
    } else if let Some(class) = Class::of(&recv.fd) {
        let (a, l) = U::text(field, sc);
        crate::nat::write(class, Text::Utf8(mem::bytes(a, l)), recv.fd.justified(), mem::bytes_mut(recv.addr, recv.len));
    } else {
        let (a, l) = U::text(field, sc);
        mv::put_bytes(recv, mem::bytes(a, l), recv.fd.justified());
    }
}

/// `INITIALIZE`: the leaves are listed in a spec of 11-word entries
/// `(first-occurrence operand, replacement operand or 0, ndims, 4 × (stride, count))`.
pub fn initialize(base: u32, spec: u32) {
    let n = rd(spec) as usize;
    for k in 0..n {
        let e = spec + 4 + (k as u32) * 44;
        let first = locate(base, &parse(rd(e)));
        let repl = rd(e + 4);
        let ndims = (rd(e + 8) as usize).min(4);
        let snd = if repl != 0 {
            mv::sender(base, &parse(repl))
        } else if first.fd.is_numeric() || first.fd.kind == K_NUM_EDITED {
            mv::Sender::Fill(b'0', true)
        } else {
            mv::Sender::Fill(b' ', false)
        };
        let mut dims = [(0u32, 1u32); 4];
        for (d, slot) in dims.iter_mut().enumerate().take(ndims) {
            *slot = (rd(e + 12 + (d as u32) * 8), rd(e + 16 + (d as u32) * 8));
        }
        let mut idx = [0u32; 4];
        'occ: loop {
            let mut addr = first.addr;
            for d in 0..ndims {
                addr += idx[d] * dims[d].0;
            }
            mv::move_to_loc(&snd, &Loc { addr, ..first });
            let mut d = ndims;
            loop {
                if d == 0 {
                    break 'occ;
                }
                d -= 1;
                idx[d] += 1;
                if idx[d] < dims[d].1 {
                    break;
                }
                idx[d] = 0;
            }
        }
    }
}

// ── value methods of data items (`X::Trim`, `X::Upper`, …) ──────────────────
//
// The interpreter's `eval_member` (`cobolt-runtime`, the `"REPLACE"` / `"TRIM"`
// arms) applies these to the receiver's `as_display_string()`: an item's
// characters as they are, a number as its decimal text. The results here are
// those of Rust's `str::trim`, `to_uppercase`, `to_lowercase`, `replace`,
// `chars().count()` and `split(..).next()`, computed without an allocator.

pub const VM_TRIM: u32 = 1;
pub const VM_UPPER: u32 = 2;
pub const VM_LOWER: u32 = 3;
pub const VM_REPLACE: u32 = 4;
pub const VM_LEN: u32 = 5;
pub const VM_SPLIT: u32 = 6;

/// Room in the scratch area for the text of an operand that is a number or a
/// figurative constant (the receiver and the two arguments); an operand that
/// is characters is read where it is.
const VM_RECV: u32 = 0;
const VM_NUM_CAP: u32 = 64;
const VM_A: u32 = VM_RECV + VM_NUM_CAP;
const VM_B: u32 = VM_A + VM_NUM_CAP;
const VM_ALIAS: u32 = VM_B + VM_NUM_CAP;

/// Where the display text of operand `o` (0: none) is: the characters of an
/// item, a literal or a character temporary where they are — whole, however
/// long (the desktop applies the method to every byte) — and a number or a
/// figurative constant as its text at `dst`. The result is written into
/// character temporary `k`, so an operand that is that temporary (a chain
/// `X::Trim()::Upper()`) is read from a copy: the answer region for the
/// receiver (`alias_at`; no query runs inside a value method), the scratch
/// area for an argument.
fn vm_text(base: u32, o: u32, dst: u32, k: u32, alias_at: u32, alias_cap: u32) -> (u32, u32) {
    if o == 0 {
        return (dst, 0);
    }
    let op = parse(o);
    let mut num = [0u8; 64];
    let number = |v: Val, num: &mut [u8; 64]| -> usize {
        match v {
            Val::Dec(d) => crate::disp::format_dec(&d, num),
            Val::F(f) => crate::float::format_f64(f, num),
        }
    };
    let n = match op.tag {
        T_LIT_NUM | T_TEMP => number(value(base, &op), &mut num),
        T_FIG => {
            num[0] = fig_byte(op.fig);
            1
        }
        T_CTEMP | T_LIT_ALNUM | T_ALL => {
            let l = locate(base, &op);
            if op.tag == T_CTEMP && (op.off as usize) % crate::state::NCTEMP == k as usize {
                if l.len > alias_cap {
                    too_long(l.len, alias_cap);
                }
                mem::copy(l.addr, alias_at, l.len);
                return (alias_at, l.len);
            }
            return (l.addr, l.len);
        }
        _ => {
            let l = locate(base, &op);
            if l.fd.is_numeric() {
                number(codec::load_value(&l), &mut num)
            } else {
                return (l.addr, l.len);
            }
        }
    };
    let n = (n as u32).min(VM_NUM_CAP);
    mem::bytes_mut(dst, n).copy_from_slice(&num[..n as usize]);
    (dst, n)
}

/// Run-time error [`err::TEXT_TOO_LARGE`]: a value method's text of `n` bytes
/// where `cap` fit.
#[cold]
fn too_long(n: u32, cap: u32) -> ! {
    use crate::Part::{N, T};
    crate::fail_text(err::TEXT_TOO_LARGE, &[T(b"a value method's text is "), N(n), T(b" bytes or more; it can hold at most "), N(cap), T(b" bytes")])
}

/// Rust's `char::is_whitespace` for the ASCII range (what `str::trim` drops).
fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// The result buffer being filled.
struct Out {
    at: u32,
    len: u32,
    cap: u32,
}

impl Out {
    fn push(&mut self, bytes: &[u8]) {
        let n = bytes.len() as u32;
        if n > self.cap - self.len {
            too_long(self.len + n, self.cap);
        }
        mem::bytes_mut(self.at + self.len, n).copy_from_slice(bytes);
        self.len += n;
    }
}

/// Case-map `t` into `out`: by `char` when `t` is UTF-8, by byte otherwise.
fn case_map(t: &[u8], upper: bool, out: &mut Out) {
    match core::str::from_utf8(t) {
        Ok(s) => {
            let mut buf = [0u8; 4];
            for c in s.chars() {
                if upper {
                    for u in c.to_uppercase() {
                        out.push(u.encode_utf8(&mut buf).as_bytes());
                    }
                } else {
                    for u in c.to_lowercase() {
                        out.push(u.encode_utf8(&mut buf).as_bytes());
                    }
                }
            }
        }
        Err(_) => {
            for &b in t {
                out.push(&[if upper { b.to_ascii_uppercase() } else { b.to_ascii_lowercase() }]);
            }
        }
    }
}

/// The byte offsets of `t`'s characters (UTF-8 when it is, bytes otherwise),
/// for `replace("", x)`, which inserts between every two characters.
fn char_starts(t: &[u8]) -> impl Iterator<Item = usize> + '_ {
    let utf8 = core::str::from_utf8(t).is_ok();
    (0..t.len()).filter(move |&i| !utf8 || (t[i] & 0xC0) != 0x80)
}

/// `recv::Method(a, b)` into character temporary `k` (WASM-010 T11). `Len`
/// leaves its count as decimal text marked as a number.
pub fn value_method(base: u32, method: u32, recv: u32, a: u32, b: u32, k: u32) {
    let (sc, _) = mv::scratch();
    let k = (k as usize % crate::state::NCTEMP) as u32;
    let (ws, cap) = (st().ws, crate::state::CTEMP_SIZE);
    let (ra, rn) = vm_text(base, recv, sc + VM_RECV, k, ws + crate::state::WS_ANSWER, crate::state::ANSWER_SIZE);
    let (aa, an) = vm_text(base, a, sc + VM_A, k, sc + VM_ALIAS, (SCRATCH_SIZE - VM_ALIAS) / 2);
    let (ba, bn) = vm_text(base, b, sc + VM_B, k, sc + VM_ALIAS + (SCRATCH_SIZE - VM_ALIAS) / 2, (SCRATCH_SIZE - VM_ALIAS) / 2);
    let t = mem::bytes(ra, rn);
    let ta = mem::bytes(aa, an);
    let tb = mem::bytes(ba, bn);
    // The result goes straight into its temporary; no operand is read from it.
    let mut out = Out { at: ws + crate::state::WS_CTEMP + k * cap, len: 0, cap };
    match method {
        VM_TRIM => {
            let s = t.iter().position(|&c| !is_space(c)).unwrap_or(t.len());
            let e = t.iter().rposition(|&c| !is_space(c)).map_or(s, |i| i + 1).max(s);
            out.push(&t[s..e]);
        }
        VM_UPPER => case_map(t, true, &mut out),
        VM_LOWER => case_map(t, false, &mut out),
        VM_REPLACE => {
            if ta.is_empty() {
                let mut last = 0;
                for i in char_starts(t) {
                    out.push(&t[last..i]);
                    out.push(tb);
                    last = i;
                }
                out.push(&t[last..]);
                out.push(tb);
            } else {
                let mut i = 0;
                while let Some(j) = find(t, ta, i) {
                    out.push(&t[i..j]);
                    out.push(tb);
                    i = j + ta.len();
                }
                out.push(&t[i..]);
            }
        }
        VM_LEN => {
            // `chars().count()`, counted here: core's word-at-a-time count
            // reaches a panic path the module's linker stubs, so a text of a
            // few words trapped (`WS-X::Len` on a `PIC X(100)`).
            let n = match core::str::from_utf8(t) {
                Ok(_) => t.iter().filter(|&&c| (c & 0xC0) != 0x80).count(),
                Err(_) => t.len(),
            };
            let mut num = [0u8; 64];
            let l = crate::disp::format_dec(&CobolNumeric::integer(n as i64), &mut num);
            out.push(&num[..l]);
        }
        VM_SPLIT => {
            // The separator is a space when none is given. `b`, when given, is the part's index
            // (`X::Split(sep)(i)`), counted from 1 as the interpreter counts it: an index of 0 or
            // less reads as 1, and a part past the last is empty. Parts are cut as Rust's
            // `str::split` cuts them, which the interpreter uses.
            let sep: &[u8] = if a == 0 { b" " } else { ta };
            let want = if b == 0 { 0 } else { crate::deskio::whole_number(base, b).saturating_sub(1).clamp(0, i64::from(u32::MAX)) as usize };
            if sep.is_empty() {
                // `split("")`: an empty part, then each character, then an empty part.
                if want > 0 {
                    if let Some(s) = char_starts(t).nth(want - 1) {
                        let e = char_starts(t).nth(want).unwrap_or(t.len());
                        out.push(&t[s..e]);
                    }
                }
            } else {
                let mut from = 0;
                for part in 0.. {
                    let end = find(t, sep, from);
                    if part == want {
                        out.push(&t[from..end.unwrap_or(t.len())]);
                        break;
                    }
                    match end {
                        Some(e) => from = e + sep.len(),
                        None => break,
                    }
                }
            }
        }
        _ => crate::fail(err::BAD_OPERAND),
    }
    let s = st();
    let k = k as usize;
    s.ctemp_len[k] = out.len;
    if method == VM_LEN {
        s.ctemp_dynamic |= 1 << k;
    } else {
        s.ctemp_dynamic &= !(1 << k);
    }
}
