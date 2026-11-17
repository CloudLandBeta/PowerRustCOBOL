// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The text files and pictures a built-in names by its path — **a desktop module only** (ABI A39,
//! DESKTOP-WASM-001 T39c).
//!
//! `COBOL::"APPEND-FILE"`, `"WRITE-FILE"`, `"IMAGE-FIT"` and `"IMAGE-INFO"` are the interpreter's
//! `CALL "COBOL-APPEND-FILE"` and its siblings. The runner does the file and picture work with the
//! interpreter's own code; these routines do the program's half, as the interpreter's arms do it:
//!
//! - each path and text goes to the runner whole, as the interpreter takes the argument's value (a
//!   number as its text); the runner trims it as the interpreter does (a path on both sides, a
//!   line's text at its end), so the white space that counts is decided by the same code;
//! - a width or a height is the argument's whole number, as the interpreter's `as_i64` takes it;
//! - each answer goes into its receiver as the interpreter sets it: a word as `set_str` does, a
//!   number as `set_i64` does (into an item of characters, right-justified).
//!
//! An operand of 0 is a receiver the program did not write (`APPEND-FILE`'s status is optional).

use crate::accept::put;
use crate::codec;
use crate::disp;
use crate::host;
use crate::mem;
use crate::opd::*;
use crate::state::{st, SCRATCH_SIZE, WS_SCRATCH};
use crate::val::Val;
use crate::fd::K_NUM_EDITED;
use cobolt_decimal::CobolNumeric;

/// The scratch area holds, from its start, the text of a number given as a path or a text (64 bytes
/// for each of the two arguments), then the runner's answer.
const PLACE: u32 = 64;
const ANSWER_AT: u32 = 2 * PLACE;

/// The text of argument `o`, as the interpreter's value of it: every character of a data item or a
/// literal, and a number's text. A number's text is made in place `place` (0 or 1) of the scratch
/// area; anything else is read where it is.
fn arg_text(base: u32, o: u32, place: u32) -> (u32, u32) {
    let at = st().ws + WS_SCRATCH + place * PLACE;
    let number = |v: &Val| -> (u32, u32) {
        let mut num = [0u8; 64];
        let n = match v {
            Val::Dec(d) => disp::format_dec(d, &mut num),
            Val::F(f) => crate::float::format_f64(*f, &mut num),
        };
        mem::bytes_mut(at, n as u32).copy_from_slice(&num[..n]);
        (at, n as u32)
    };
    let o = parse(o);
    match o.tag {
        // A figurative constant is the one character it names (`ZERO` and `NULL` the digit 0).
        T_FIG => {
            mem::wr_u8(at, if o.fig == FIG_NULL { b'0' } else { fig_byte(o.fig) });
            (at, 1)
        }
        T_LIT_NUM | T_TEMP => number(&value(base, &o)),
        T_LIT_ALNUM | T_ALL | T_CTEMP => {
            let l = locate(base, &o);
            (l.addr, l.len)
        }
        _ => {
            let l = locate(base, &o);
            if l.fd.is_numeric() {
                number(&codec::load_value(&l))
            } else {
                (l.addr, l.len)
            }
        }
    }
}

/// Argument `o` as a whole number, as the interpreter's `as_i64` takes a value: a number without its
/// decimals, and a text that is a whole number once trimmed (a sign allowed), else 0.
/// A text's characters are read where they are, but a figurative constant's are made in place 0, so
/// the numbers are read before the texts.
pub(crate) fn whole_number(base: u32, raw: u32) -> i64 {
    let o = parse(raw);
    let numeric = match o.tag {
        T_LIT_NUM | T_TEMP => Some(value(base, &o)),
        T_LIT_ALNUM | T_ALL | T_CTEMP | T_FIG => None,
        _ => {
            let l = locate(base, &o);
            l.fd.is_numeric().then(|| codec::load_value(&l))
        }
    };
    match numeric {
        Some(Val::Dec(d)) => d.mantissa.checked_div(cobolt_decimal::pow10(d.decimals as u32).unwrap_or(1)).unwrap_or(0) as i64,
        Some(Val::F(f)) => f as i64,
        None => {
            let (a, l) = arg_text(base, raw, 0);
            parse_whole(mem::bytes(a, l))
        }
    }
}

/// A text as Rust's `str::trim().parse::<i64>()` reads it: white space around it, an optional sign,
/// then only digits, within an `i64`; anything else is 0.
fn parse_whole(b: &[u8]) -> i64 {
    let blank = |c: &u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C);
    let start = b.iter().position(|c| !blank(c)).unwrap_or(b.len());
    let end = b.iter().rposition(|c| !blank(c)).map_or(start, |e| e + 1);
    let mut s = b.get(start..end).unwrap_or(&[]);
    let negative = match s.first() {
        Some(b'-') => {
            s = &s[1..];
            true
        }
        Some(b'+') => {
            s = &s[1..];
            false
        }
        _ => false,
    };
    if s.is_empty() {
        return 0;
    }
    let mut n: i64 = 0;
    for &c in s {
        if !c.is_ascii_digit() {
            return 0;
        }
        let d = i64::from(c - b'0');
        let next = n.checked_mul(10).and_then(|v| if negative { v.checked_sub(d) } else { v.checked_add(d) });
        match next {
            Some(v) => n = v,
            None => return 0,
        }
    }
    n
}

/// The digits of `n`, without leading zeros.
fn digits(n: u32, out: &mut [u8; 10]) -> &[u8] {
    let mut v = n;
    let mut at = out.len();
    loop {
        at -= 1;
        out[at] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 || at == 0 {
            break;
        }
    }
    &out[at..]
}

/// A whole number into receiver `target` as the interpreter's `set_i64` puts it: a numeric or
/// numeric-edited item takes the number as a `MOVE` does; an item of characters takes its digits
/// at its right end, spaces before them, and the digits' left end when they do not fit (the
/// interpreter's `CobolValue::assign` of a number into text, which ignores `JUSTIFIED`).
fn put_number(base: u32, target: u32, n: u32) {
    let loc = locate(base, &parse(target));
    if loc.fd.is_numeric() || loc.fd.kind == K_NUM_EDITED {
        codec::store_value(&loc, Val::Dec(CobolNumeric::integer(i64::from(n))), false, false);
        return;
    }
    let mut out = [0u8; 10];
    let d = digits(n, &mut out);
    let field = mem::bytes_mut(loc.addr, loc.len);
    let k = d.len().min(field.len());
    let start = field.len() - k;
    field.fill(b' ');
    field[start..].copy_from_slice(&d[..k]);
}

/// Where the runner writes its answer, and how much it may write.
fn answer_area() -> (u32, u32) {
    (st().ws + WS_SCRATCH + ANSWER_AT, SCRATCH_SIZE - ANSWER_AT)
}

/// `COBOL::"APPEND-FILE" ( path text [status] )` (`append` 1) and `"WRITE-FILE"` (0): `text` as one
/// line of the file, added to it or replacing it; `status` gets spaces, or why the line was not written.
pub fn text_write(base: u32, append: u32, path: u32, text: u32, status: u32) {
    let (pa, pl) = arg_text(base, path, 0);
    let (ta, tl) = arg_text(base, text, 1);
    let (buf, cap) = answer_area();
    // SAFETY: the runner reads the two texts and writes at most `cap` bytes at `buf`.
    let n = unsafe { host::t_write(append, pa, pl, ta, tl, buf, cap) };
    if status != 0 {
        let why = if n > 0 { mem::bytes(buf, (n as u32).min(cap)) } else { &[] };
        put(base, status, why);
    }
}

/// `COBOL::"IMAGE-FIT" ( source target width height status )`: the picture fitted and written;
/// `status` gets the runner's word (`OK`, `NOFILE`, `BADIMAGE`, `TOOBIG`, `BADEXT`, `BADSIZE`,
/// `WRITE` or `NOFEATURE`).
pub fn image_fit(base: u32, source: u32, target: u32, width: u32, height: u32, status: u32) {
    let (w, h) = (whole_number(base, width), whole_number(base, height));
    let (sa, sl) = arg_text(base, source, 0);
    let (da, dl) = arg_text(base, target, 1);
    let (buf, cap) = answer_area();
    // SAFETY: the runner reads the two texts and writes at most `cap` bytes at `buf`.
    let n = unsafe { host::img_fit(sa, sl, da, dl, w, h, buf, cap) };
    let word = if n > 0 { mem::bytes(buf, (n as u32).min(cap)) } else { &[] };
    put(base, status, word);
}

/// `COBOL::"IMAGE-INFO" ( path width height format status )`: the picture's upright size and its
/// format, read from its header; on a refusal 0 × 0 and no format. The receivers are set in the
/// interpreter's order.
pub fn image_info(base: u32, path: u32, width: u32, height: u32, format: u32, status: u32) {
    let (pa, pl) = arg_text(base, path, 0);
    let (buf, cap) = answer_area();
    // SAFETY: the runner reads the path and writes at most `cap` bytes at `buf`.
    let n = unsafe { host::img_info(pa, pl, buf, cap) };
    let (w, h) = (mem::rd_u32(buf), mem::rd_u32(buf + 4));
    let rest = if n > 0 { mem::bytes(buf + 8, (n as u32).min(cap - 8)) } else { &[] };
    let gap = rest.iter().position(|&c| c == b' ').unwrap_or(rest.len());
    let (fmt, word) = (&rest[..gap], rest.get(gap + 1..).unwrap_or(&[]));
    put_number(base, width, w);
    put_number(base, height, h);
    put(base, format, fmt);
    put(base, status, word);
}
