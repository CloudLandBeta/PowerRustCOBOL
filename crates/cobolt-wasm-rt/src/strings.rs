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

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
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

/// The `[lo, hi)` window of `BEFORE`/`AFTER INITIAL` over `s`.
fn window(base: u32, s: &[u8], before: u32, after: u32, sc: &mut Scratch) -> (usize, usize) {
    let lo = if after != 0 {
        let (a, l) = chars_of(base, &parse(after), sc);
        let d = mem::bytes(a, l);
        match find(s, d, 0) {
            Some(p) => p + d.len(),
            None => s.len(),
        }
    } else {
        0
    };
    let hi = if before != 0 {
        let (a, l) = chars_of(base, &parse(before), sc);
        let d = mem::bytes(a, l);
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
fn trailing_run(window: &[u8], pat: &[u8]) -> usize {
    let mut n = 0;
    let mut end = window.len();
    while !pat.is_empty() && end >= pat.len() && &window[end - pat.len()..end] == pat {
        n += 1;
        end -= pat.len();
    }
    n
}

#[derive(Clone, Copy)]
struct Op {
    kind: u32,
    counter: u32,
    pat: (u32, u32),
    by: (u32, u32),
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

fn blank_op() -> Op {
    Op {
        kind: 0,
        counter: 0,
        pat: (0, 0),
        by: (0, 0),
        lo: 0,
        hi: 0,
        leading_next: 0,
        leading_live: true,
        trail_start: 0,
        spent: false,
    }
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
    inspect_chars(base, &t, spec);
    if let Some((at, true)) = embedded {
        let b = mem::rd_u8(at);
        if b.is_ascii_digit() {
            mem::wr_u8(at, codec::overpunch(b - b'0', true));
        }
    }
}

fn inspect_chars(base: u32, t: &Loc, spec: u32) {
    let t = *t;
    let n = rd(spec) as usize;
    if n > MAX_OPS {
        crate::fail(err::BAD_OPERAND);
    }
    let mut sc = Scratch::new();
    let s = mem::bytes(t.addr, t.len);
    let mut ops = [blank_op(); MAX_OPS];
    for (k, op) in ops.iter_mut().enumerate().take(n) {
        let r = spec + 4 + (k as u32) * 24;
        op.kind = rd(r);
        op.counter = rd(r + 4);
        let pat = rd(r + 8);
        let by = rd(r + 12);
        let (lo, hi) = window(base, s, rd(r + 16), rd(r + 20), &mut sc);
        op.lo = lo;
        op.hi = hi;
        op.leading_next = lo;
        if pat != 0 {
            op.pat = chars_of(base, &parse(pat), &mut sc);
        }
        if by != 0 {
            op.by = chars_of(base, &parse(by), &mut sc);
        }
        op.trail_start = hi;
        if matches!(op.kind, TALLY_TRAILING | REPL_TRAILING) && op.pat.1 > 0 {
            let w = &s[lo..hi];
            let p = mem::bytes(op.pat.0, op.pat.1);
            op.trail_start = hi - trailing_run(w, p) * p.len();
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
                let pat = mem::bytes(op.pat.0, op.pat.1);
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
        for (k, op) in ops.iter().enumerate().take(n) {
            if op.kind <= TALLY_TRAILING {
                // The counter accumulates onto its value; operands sharing one
                // counter were tallied separately above and are summed here.
                let _ = k;
            }
        }
        // Sum per counter operand and store once each.
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
    if have_repl {
        let out_at = sc.alloc(t.len.max(1));
        let mut o = 0usize; // bytes written to `out`
        let cap = t.len as usize;
        let mut p = 0usize;
        while p < s.len() {
            let mut taken = 0usize;
            for op in ops.iter_mut().take(n) {
                if !(REPL_CHARS..=REPL_FIRST).contains(&op.kind) || p < op.lo || p >= op.hi || op.spent {
                    continue;
                }
                let pat = mem::bytes(op.pat.0, op.pat.1);
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
                    let by = mem::bytes(op.by.0, op.by.1);
                    let emit: &[u8] = if op.kind == REPL_CHARS { &by[..by.len().min(1)] } else { by };
                    let emit_one = [b' '];
                    let emit = if op.kind == REPL_CHARS && emit.is_empty() { &emit_one[..] } else { emit };
                    for &b in emit {
                        if o < cap {
                            mem::wr_u8(out_at + o as u32, b);
                            o += 1;
                        }
                    }
                    break;
                }
            }
            if taken == 0 {
                if o < cap {
                    mem::wr_u8(out_at + o as u32, s[p]);
                    o += 1;
                }
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
        // Back into the item: left-justified, space-padded.
        mem::copy(out_at, t.addr, o as u32);
        if (o as u32) < t.len {
            mem::fill(t.addr + o as u32, t.len - o as u32, b' ');
        }
    }
    // ── CONVERTING ─────────────────────────────────────────────────────
    for k in 0..n {
        if ops[k].kind != CONVERT {
            continue;
        }
        let from = mem::bytes(ops[k].pat.0, ops[k].pat.1);
        let to = mem::bytes(ops[k].by.0, ops[k].by.1);
        let buf = mem::bytes_mut(t.addr, t.len);
        // The interpreter converts character by character in pair order:
        // each `from` char is replaced by its `to` char over the whole window.
        let (lo, hi) = (ops[k].lo, ops[k].hi);
        for (fc, tc) in from.iter().zip(to.iter()) {
            for b in buf[lo..hi].iter_mut() {
                if *b == *fc {
                    *b = *tc;
                }
            }
        }
    }
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

/// The next delimiter at or after `from`: `(offset, run length, one length)`.
fn find_delimiter(src: &[u8], from: usize, delims: &[(u32, u32)], all: bool) -> Option<(usize, usize, usize)> {
    let mut best: Option<(usize, usize, usize)> = None;
    for &(da, dl) in delims {
        let d = mem::bytes(da, dl);
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

/// `UNSTRING … ` ; returns 1 on overflow.
pub fn unstring(base: u32, spec: u32) -> u32 {
    let all = rd(spec) & 1 != 0;
    let from = locate(base, &parse(rd(spec + 4)));
    let ptr_opd = rd(spec + 8);
    let tally_opd = rd(spec + 12);
    let nd = rd(spec + 16) as usize;
    let mut sc = Scratch::new();
    let mut delims = [(0u32, 0u32); 16];
    let mut ndel = 0;
    for k in 0..nd.min(16) {
        let (a, l) = chars_of(base, &parse(rd(spec + 20 + (k as u32) * 4)), &mut sc);
        if l > 0 {
            delims[ndel] = (a, l);
            ndel += 1;
        }
    }
    let delims = &delims[..ndel];
    let nt_at = spec + 20 + (nd as u32) * 4;
    let nt = rd(nt_at) as usize;
    let src = mem::bytes(from.addr, from.len);
    let ptr_loc = if ptr_opd != 0 { Some(locate(base, &parse(ptr_opd))) } else { None };
    let cursor: i64 = match &ptr_loc {
        Some(pl) => match codec::load_value(pl) {
            Val::Dec(d) => (d.mantissa / cobolt_decimal::pow10(d.decimals as u32).unwrap_or(1)) as i64,
            Val::F(f) => f as i64,
        },
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
            let width = (recv.len as usize).max(1);
            ((pos + width).min(src.len()), 0, 0)
        } else {
            match find_delimiter(src, pos, delims, all) {
                Some((at, len, unit)) => (at, len, unit),
                None => (src.len(), 0, 0),
            }
        };
        let field = &src[pos..field_end];
        // The receiver takes the field under MOVE rules.
        if recv.fd.is_numeric() {
            let mut m: i128 = 0;
            let mut seen = false;
            for &c in field.iter().skip_while(|c| **c == b' ') {
                if c.is_ascii_digit() {
                    m = m.saturating_mul(10).saturating_add((c - b'0') as i128);
                    seen = true;
                } else {
                    break;
                }
            }
            let _ = seen;
            codec::store_value(&recv, Val::Dec(CobolNumeric::new(m, 0)), false, false);
        } else {
            mv::put_bytes(&recv, field, recv.fd.justified());
        }
        let din = rd(r + 4);
        if din != 0 {
            let dl = locate(base, &parse(din));
            mv::put_bytes(&dl, &src[field_end..field_end + unit_len], false);
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
        let prior = match codec::load_value(&tl) {
            Val::Dec(d) => (d.mantissa / cobolt_decimal::pow10(d.decimals as u32).unwrap_or(1)) as i64,
            Val::F(f) => f as i64,
        };
        codec::store_value(&tl, Val::int(prior + filled as i64), false, false);
    }
    (pos < src.len()) as u32
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
