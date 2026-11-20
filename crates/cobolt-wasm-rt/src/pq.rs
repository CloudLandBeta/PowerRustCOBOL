// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `COBOL::"PARTIQL"`, `"PARTIQL-FETCH"` and `"PARTIQL-ERROR"` — **a desktop module only** (ABI A56,
//! DESKTOP-WASM-001 T48).
//!
//! The runner runs the statements with the interpreter's own rules on the file the program's `SELECT`
//! declares and holds open, and keeps each file's results for `PARTIQL-FETCH` and `PARTIQL-ERROR`.
//! This routine does the program's half, as the interpreter's arms do it: each argument goes whole, as
//! the interpreter takes its value (a number as its text), and each answer goes into its receiver as
//! the interpreter sets it, in the interpreter's order.
//!
//! The routine is written without an index that can fail: such an index puts its place in the source
//! into the routines' data, which every module carries.

use crate::deskio::{arg_text, put_number, put_text};
use crate::host;
use crate::mem;
use crate::opd::{locate, parse, Loc, T_ALL, T_CTEMP, T_FIG, T_LIT_ALNUM, T_LIT_NUM, T_TEMP};
use crate::state::{st, SCRATCH_SIZE, WS_SCRATCH};

/// `cobolt_wasm_abi::partiql` (A56), held equal to it as `deskio::http` is.
pub const RUN: u32 = 1;
pub const FETCH: u32 = 2;
pub const ERROR: u32 = 3;
pub const TAIL: u32 = 0x100;
pub const NUMBER: u8 = b'N';
pub const TEXT: u8 = b'S';

/// The request starts after the two places where `arg_text` makes a number's text.
const REQUEST_AT: u32 = 128;

/// Where the runner writes a text that goes into receiver `r` (0: none): a receiver of characters
/// (`PIC X`, a group) takes it straight, its first bytes or, for a `JUSTIFIED RIGHT` one (`tail`), its
/// last; any other receiver gets it through `area`, as `set_str` puts it.
struct Place {
    direct: Option<Loc>,
    tail: bool,
    buf: u32,
    cap: u32,
}

fn place(base: u32, r: u32, area: (u32, u32)) -> Place {
    let direct = if r != 0 {
        let loc = locate(base, &parse(r));
        matches!(loc.fd.kind, crate::fd::K_ALNUM | crate::fd::K_GROUP).then_some(loc)
    } else {
        None
    };
    let tail = direct.as_ref().is_some_and(|l| l.fd.justified());
    let (buf, cap) = match &direct {
        Some(l) => (l.addr, l.len),
        None => area,
    };
    Place { direct, tail, buf, cap }
}

/// The `n` bytes the runner wrote at `p` (at most its `cap`) into receiver `r`: a receiver taken
/// straight is padded as a move pads it (spaces after, or for `JUSTIFIED RIGHT` before).
fn put_answer(base: u32, r: u32, p: &Place, n: u32) {
    let n = n.min(p.cap);
    match &p.direct {
        Some(l) if p.tail => {
            let pad = l.len - n;
            mem::copy(l.addr, l.addr + pad, n);
            mem::fill(l.addr, pad, b' ');
        }
        Some(l) => mem::fill(l.addr + n, l.len - n, b' '),
        None => put_text(base, r, mem::bytes(p.buf, n)),
    }
}

/// A `?` argument of kind `NUMBER` that is a numeric `DISPLAY` item whose characters are not a number
/// of its picture (a group `MOVE` left letters or spaces in it). The interpreter types such an
/// argument by its characters (`Interpreter::partiql_arg`, `Environment::numeric_by_picture`), not by
/// what decoding the item would make of them: [`by_characters`].
fn unspelled(base: u32, o: u32, kind: u32) -> Option<Loc> {
    if kind != u32::from(NUMBER) {
        return None;
    }
    let op = parse(o);
    if matches!(op.tag, T_FIG | T_LIT_NUM | T_LIT_ALNUM | T_ALL | T_TEMP | T_CTEMP) {
        return None;
    }
    let l = locate(base, &op);
    (l.fd.kind == crate::fd::K_NUM_DISPLAY && !crate::cmp::class_test(base, o, crate::cmp::CLASS_NUMERIC)).then_some(l)
}

/// What the interpreter makes of item `l`'s characters (`numeric_by_picture`), with the kind it sends:
/// a signed item's digits whose last one carries the sign, or digits with a sign before or after
/// them, are a number, its text written at `next` with the picture's decimals; anything else is text,
/// the characters as they are, and the statement fails naming the argument.
fn by_characters(l: &Loc, next: &mut u32) -> (u32, u32, u32) {
    let b = mem::bytes(l.addr, l.len);
    let mut number: Option<(bool, &[u8], Option<u8>)> = None;
    if l.fd.signed() && !l.fd.sign_separate() {
        if let Some((&last, head)) = b.split_last() {
            let digit = match last {
                b'{' => Some((0, false)),
                b'A'..=b'I' => Some((last - b'A' + 1, false)),
                b'}' => Some((0, true)),
                b'J'..=b'R' => Some((last - b'J' + 1, true)),
                _ => None,
            };
            if let Some((d, negative)) = digit.filter(|_| head.iter().all(u8::is_ascii_digit)) {
                number = Some((negative, head, Some(b'0' + d)));
            }
        }
    }
    if number.is_none() {
        let (negative, digits) = match b {
            [b'-', rest @ ..] | [rest @ .., b'-'] => (true, rest),
            [b'+', rest @ ..] | [rest @ .., b'+'] => (false, rest),
            _ => (false, b),
        };
        if !digits.is_empty() && digits.iter().all(u8::is_ascii_digit) {
            number = Some((negative, digits, None));
        }
    }
    let Some((negative, head, last)) = number else {
        return (u32::from(TEXT), l.addr, l.len);
    };
    let start = *next;
    let mut put = |c: u8| {
        mem::wr_u8(*next, c);
        *next += 1;
    };
    if negative {
        put(b'-');
    }
    let n = head.len() as u32 + u32::from(last.is_some());
    let scale = l.fd.scale.max(0) as u32;
    if scale >= n {
        put(b'0');
        put(b'.');
        for _ in n..scale {
            put(b'0');
        }
    }
    for (i, c) in head.iter().copied().chain(last).enumerate() {
        if scale > 0 && scale < n && i as u32 == n - scale {
            put(b'.');
        }
        put(c);
    }
    (u32::from(NUMBER), start, *next - start)
}

/// The routine: `spec` is a static record of words.
///
/// ```text
///  0 op      RUN, FETCH or ERROR
///  4 file    the file argument
///  8 a       RUN: the statement · FETCH: the row's receiver · ERROR: the text's receiver
/// 12 b       RUN: the count's receiver · FETCH: the status's receiver
/// 16 c       RUN: the status's receiver
/// 20 files   the program's files: a record of n, then n × (id, region offset of its handle)
/// 24 nargs   RUN: the `?` arguments,
/// 28         nargs × (operand, kind)
/// ```
///
/// The request (`cobolt_wasm_abi::partiql`) goes out in the scratch area, each number's text after
/// it, and the answer comes back after them. The receivers are set in the interpreter's order: the
/// count as `set_i64` sets it, then the status (which therefore always comes through the area); the
/// row, then the status.
pub fn partiql(base: u32, spec: u32) {
    let w = |k: u32| mem::rd_u32(spec + 4 * k);
    let (op, file, a, b, c, table, nargs) = (w(0), w(1), w(2), w(3), w(4), w(5), w(6));
    let run = op == RUN;
    let scratch = st().ws + WS_SCRATCH;
    let start = scratch + REQUEST_AT;
    let nfiles = mem::rd_u32(table);
    let words = 1 + 2 * nfiles + 2 + if run { 3 + 3 * nargs } else { 0 };
    let mut next = start + 4 * words;
    let mut at = start;
    let mut word = |v: u32| {
        mem::wr_u32(at, v);
        at += 4;
    };
    word(nfiles);
    for i in 0..nfiles {
        word(mem::rd_u32(table + 4 + 8 * i));
        word(base + mem::rd_u32(table + 8 + 8 * i));
    }
    // A number's text (or a figurative constant's character) is made at the scratch area's start and
    // moved after the request, so each argument keeps its own.
    let text_of = |o: u32, next: &mut u32| -> (u32, u32) {
        let (t, l) = arg_text(base, o, 0);
        if t != scratch {
            return (t, l);
        }
        mem::copy(t, *next, l);
        *next += l;
        (*next - l, l)
    };
    let (fa, fl) = text_of(file, &mut next);
    word(fa);
    word(fl);
    if run {
        let (ta, tl) = text_of(a, &mut next);
        word(ta);
        word(tl);
        word(nargs);
        for i in 0..nargs {
            let (o, kind) = (w(7 + 2 * i), w(8 + 2 * i));
            let (kind, xa, xl) = match unspelled(base, o, kind) {
                Some(l) => by_characters(&l, &mut next),
                None => {
                    let (xa, xl) = text_of(o, &mut next);
                    (kind, xa, xl)
                }
            };
            word(kind);
            word(xa);
            word(xl);
        }
    }
    let area = (next, (scratch + SCRATCH_SIZE).saturating_sub(next));
    let p = place(base, if run { 0 } else { a }, area);
    let op = if p.tail { op | TAIL } else { op };
    // SAFETY: the runner reads the request and the texts it names, and writes at most `cap` bytes at `buf`.
    let got = unsafe { host::pq_file(op, start, 4 * words, p.buf, p.cap) };
    if got < 0 {
        return;
    }
    let number = (got as u64 >> 32) & 0x7FFF_FFFF;
    let n = (got as u64 & 0xFFFF_FFFF) as u32;
    match op & !TAIL {
        RUN => {
            put_number(base, b, number);
            put_text(base, c, mem::bytes(p.buf, n.min(p.cap)));
        }
        FETCH => {
            put_answer(base, a, &p, n);
            // `00`, or `10` past the last; made here, not a literal, which would join the data every module carries.
            let code = [if number != 0 { b'0' } else { b'1' }, b'0'];
            put_text(base, b, &code);
        }
        _ => put_answer(base, a, &p, n),
    }
}
