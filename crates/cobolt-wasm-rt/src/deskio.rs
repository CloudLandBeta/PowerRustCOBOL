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
//!
//! The SQL built-ins (`OPEN-DB` and the five after it, ABI A42, T41b) are here too: the runner keeps the
//! connections and runs each call with the interpreter's own code; [`sql_db`] does the program's half.

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
pub(crate) fn arg_text(base: u32, o: u32, place: u32) -> (u32, u32) {
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
    whole_number_or(base, raw, 0)
}

/// [`whole_number`], with `none` for a text that is no whole number: the interpreter's
/// `as_i64().unwrap_or(none)`.
pub(crate) fn whole_number_or(base: u32, raw: u32, none: i64) -> i64 {
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
            parse_whole(mem::bytes(a, l)).unwrap_or(none)
        }
    }
}

/// A text as Rust's `str::trim().parse::<i64>()` reads it: white space around it, an optional sign,
/// then only digits, within an `i64`; anything else is `None`.
fn parse_whole(b: &[u8]) -> Option<i64> {
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
        return None;
    }
    let mut n: i64 = 0;
    for &c in s {
        if !c.is_ascii_digit() {
            return None;
        }
        let d = i64::from(c - b'0');
        let next = n.checked_mul(10).and_then(|v| if negative { v.checked_sub(d) } else { v.checked_add(d) });
        match next {
            Some(v) => n = v,
            None => return None,
        }
    }
    Some(n)
}

/// The digits of `n`, without leading zeros.
fn digits(n: u64, out: &mut [u8; 20]) -> &[u8] {
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
pub(crate) fn put_number(base: u32, target: u32, n: u64) {
    let loc = locate(base, &parse(target));
    if loc.fd.is_numeric() || loc.fd.kind == K_NUM_EDITED {
        codec::store_value(&loc, Val::Dec(CobolNumeric::new(i128::from(n), 0)), false, false);
        return;
    }
    let mut out = [0u8; 20];
    let d = digits(n, &mut out);
    let field = mem::bytes_mut(loc.addr, loc.len);
    let k = d.len().min(field.len());
    let start = field.len() - k;
    field.fill(b' ');
    field[start..].copy_from_slice(&d[..k]);
}

/// Where the runner writes its answer, and how much it may write.
pub(crate) fn answer_area() -> (u32, u32) {
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
    put_number(base, width, u64::from(w));
    put_number(base, height, u64::from(h));
    put(base, format, fmt);
    put(base, status, word);
}

/// Characters into receiver `target` as the interpreter's `set_str` puts them (`Environment::set`,
/// then `CobolValue::assign`):
///
/// - an item of characters takes them as a `MOVE` of characters does; a `JUSTIFIED` one takes them
///   without their trailing spaces, at its right end;
/// - a numeric item takes the number they spell (`parse_decimal`, then a floating form such as `1E3`),
///   as a `MOVE` puts it, and keeps its value when they spell none;
/// - a numeric-edited item takes the number they spell, edited, and the characters themselves when
///   they spell none;
/// - a floating-point item keeps its value: the interpreter has no rule for text into a float.
pub(crate) fn put_text(base: u32, target: u32, text: &[u8]) {
    let loc = locate(base, &parse(target));
    let fd = &loc.fd;
    if fd.is_float() {
        return;
    }
    if fd.is_numeric() || fd.kind == K_NUM_EDITED {
        let edited = fd.kind == K_NUM_EDITED;
        let number = parse_decimal(text).map(Val::Dec).or_else(|| {
            // A numeric-edited receiver edits only an exact number (the interpreter's `as_exact`).
            if edited {
                return None;
            }
            core::str::from_utf8(trim_space(text)).ok()?.parse::<f64>().ok().map(Val::F)
        });
        match number {
            Some(v) => {
                codec::store_value(&loc, v, false, false);
            }
            None if edited => crate::mv::put_bytes(&loc, text, false),
            None => {}
        }
        return;
    }
    if fd.justified() {
        let end = text.iter().rposition(|&c| c != b' ').map_or(0, |e| e + 1);
        crate::mv::put_bytes(&loc, &text[..end], true);
    } else {
        crate::mv::put_bytes(&loc, text, false);
    }
}

/// A text as the interpreter's `parse_decimal` reads it: white space around it, an optional sign (white
/// space after it allowed), digits with at most one point, at most 31 decimals kept, within an `i128`.
fn parse_decimal(b: &[u8]) -> Option<CobolNumeric> {
    let t = trim_space(b);
    let (negative, body) = match t.first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let body = trim_space(body);
    let (int, frac) = match body.iter().position(|&c| c == b'.') {
        Some(p) => (&body[..p], &body[p + 1..]),
        None => (body, &[][..]),
    };
    let frac = &frac[..frac.len().min(31)];
    if int.len() + frac.len() == 0 {
        return None;
    }
    let mut m: i128 = 0;
    for &c in int.iter().chain(frac) {
        if !c.is_ascii_digit() {
            return None;
        }
        m = m.checked_mul(10)?.checked_add(i128::from(c - b'0'))?;
    }
    Some(CobolNumeric::new(if negative { -m } else { m }, frac.len() as u8))
}

/// How many bytes of white space `b` starts with: one character of Rust's `char::is_whitespace`, in
/// UTF-8, as `str::trim` takes it; 0 when it starts with none.
fn space_at(b: &[u8]) -> usize {
    match b {
        [0x09..=0x0D | b' ', ..] => 1,
        [0xC2, 0x85 | 0xA0, ..] => 2,
        [0xE1, 0x9A, 0x80, ..] | [0xE2, 0x80, 0x80..=0x8A | 0xA8 | 0xA9 | 0xAF, ..] | [0xE2, 0x81, 0x9F, ..] | [0xE3, 0x80, 0x80, ..] => 3,
        _ => 0,
    }
}

/// `b` without the white space around it, as `str::trim` cuts it.
fn trim_space(mut b: &[u8]) -> &[u8] {
    loop {
        match space_at(b) {
            0 => break,
            n => b = &b[n..],
        }
    }
    loop {
        let len = b.len();
        match (1..=3).find(|&k| len >= k && space_at(&b[len - k..]) == k) {
            Some(k) => b = &b[..len - k],
            None => return b,
        }
    }
}

/// `COBOL::"OPEN-DB" ( connection handle status )` and the five built-ins after it (ABI A42,
/// DESKTOP-WASM-001 T41b), each with its operands in the order the program wrote them (`op` is
/// `crate::sql::db`; an operand of 0 is one the built-in does not take). The runner runs the call with
/// the interpreter's own code over the form instance's connections and answers each receiver's value,
/// which goes into it as the interpreter sets it: a number as `set_i64` does, a text as `set_str` does.
pub fn sql_db(base: u32, op: u32, a: u32, b: u32, c: u32, d: u32) {
    use crate::sql::db;
    // The numbers are read before the text: a figurative constant's characters are made in place 0.
    let whole = |o: u32| whole_number(base, o);
    let (handle, number, text, receivers) = match op {
        db::OPEN => (0, 0, a, [b, c]),
        db::EXEC => (whole(a), 0, b, [c, d]),
        db::FETCH_ROW => (whole(a), whole_number_or(base, b, 1), 0, [c, d]),
        db::NEXT_ROW | db::ROW_COUNT => (whole(a), 0, 0, [b, 0]),
        _ => (whole(a), 0, 0, [0, 0]),
    };
    let (ta, tl) = if text != 0 { arg_text(base, text, 1) } else { (0, 0) };
    let (buf, cap) = answer_area();
    // SAFETY: the runner reads the text and writes at most `cap` bytes at `buf`.
    let n = unsafe { host::sql_db(op, handle, number, ta, tl, buf, cap) };
    if n < 0 {
        return;
    }
    let end = buf + (n as u32).min(cap);
    let mut at = buf;
    for r in receivers {
        if r == 0 || at >= end {
            break;
        }
        match mem::rd_u8(at) {
            db::NUMBER if at + 9 <= end => {
                let v = u64::from(mem::rd_u32(at + 1)) | (u64::from(mem::rd_u32(at + 5)) << 32);
                // A handle or a count: never below 0.
                put_number(base, r, (v as i64).max(0) as u64);
                at += 9;
            }
            db::TEXT if at + 5 <= end => {
                let len = mem::rd_u32(at + 1).min(end - at - 5);
                put_text(base, r, mem::bytes(at + 5, len));
                at += 5 + len;
            }
            _ => break,
        }
    }
}

/// `cobolt_wasm_abi::http` (A45), held equal to it as `sql::db` is.
pub mod http {
    pub const GET: u32 = 1;
    pub const POST: u32 = 2;
    pub const PUT: u32 = 3;
    pub const DELETE: u32 = 4;
    pub const SET_HEADER: u32 = 5;
    pub const CLEAR_HEADERS: u32 = 6;
    pub const DOWNLOAD: u32 = 7;
    pub const TAIL: u32 = 0x100;
}

/// `COBOL::"HTTP-GET" ( url response http-status )`, `"HTTP-POST"` and `"HTTP-PUT" ( url body response
/// http-status )`, `"HTTP-DELETE" ( url response http-status )`, `"HTTP-SET-HEADER" ( name value )`,
/// `"HTTP-CLEAR-HEADERS"` and `"HTTP-DOWNLOAD" ( url file http-status status )` (ABI A45): the runner
/// runs each with the interpreter's own client. `a` and `b` are the operands sent (0: none); `text` and
/// `number` receive the answer's text and number (0: none).
///
/// A body has no limit of its own, so the runner writes it straight into a receiver of characters
/// (`PIC X`, a group), its first bytes or, for a `JUSTIFIED RIGHT` one, its last, and the module pads
/// it as a move does. Any other receiver gets the text through the scratch area, as `set_str` puts it.
pub fn http(base: u32, op: u32, a: u32, b: u32, text: u32, number: u32) {
    let (aa, al) = if a != 0 { arg_text(base, a, 0) } else { (0, 0) };
    let (ba, bl) = if b != 0 { arg_text(base, b, 1) } else { (0, 0) };
    let direct = if text != 0 {
        let loc = locate(base, &parse(text));
        matches!(loc.fd.kind, crate::fd::K_ALNUM | crate::fd::K_GROUP).then_some(loc)
    } else {
        None
    };
    let tail = direct.as_ref().is_some_and(|l| l.fd.justified());
    let (buf, cap) = match &direct {
        Some(l) => (l.addr, l.len),
        None => answer_area(),
    };
    let op = if tail { op | http::TAIL } else { op };
    // SAFETY: the runner reads the two texts and writes at most `cap` bytes at `buf`.
    let got = unsafe { host::http_call(op, aa, al, ba, bl, buf, cap) };
    if got < 0 {
        return;
    }
    let n = ((got as u64 & 0xFFFF_FFFF) as u32).min(cap) as usize;
    if text != 0 {
        match &direct {
            Some(l) => {
                let field = mem::bytes_mut(l.addr, l.len);
                if !tail {
                    field[n..].fill(b' ');
                } else if n < field.len() {
                    let pad = field.len() - n;
                    field.copy_within(0..n, pad);
                    field[..pad].fill(b' ');
                }
            }
            None => put_text(base, text, mem::bytes(buf, n as u32)),
        }
    }
    if number != 0 {
        put_number(base, number, (got as u64 >> 32) & 0xFFFF);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `trim_space` cuts what `str::trim` cuts (the interpreter trims a fetched value so), for every
    /// character Rust counts as white space, two at either end, and leaves a character that is not.
    #[test]
    fn trim_space_cuts_what_str_trim_cuts() {
        let spaces: std::vec::Vec<char> = (0..=0x10FFFFu32).filter_map(char::from_u32).filter(|c| c.is_whitespace()).collect();
        let mut pairs = 0;
        for &a in &spaces {
            for &b in &spaces {
                let s = std::format!("{a}{b}1.5{b}{a}");
                assert_eq!(trim_space(s.as_bytes()), s.trim().as_bytes(), "{s:?}");
                pairs += 1;
            }
        }
        let kept = ["\u{200B}7", "7\u{FEFF}", "\u{180E}7", "x", ""];
        for s in kept {
            assert_eq!(trim_space(s.as_bytes()), s.trim().as_bytes(), "{s:?}");
        }
        std::println!("trim_space: {} white-space characters, {pairs} pairs and {} others cut as str::trim cuts them", spaces.len(), kept.len());
    }

    /// `parse_decimal` reads a text as the interpreter's `parse_decimal` does.
    #[test]
    fn parse_decimal_reads_as_the_interpreter_does() {
        let digits31 = "1".repeat(31);
        let long = std::format!("0.{digits31}99");
        let cases: [(&str, Option<(i128, u8)>); 14] = [
            ("42", Some((42, 0))),
            (" -  3.50 ", Some((-350, 2))),
            ("+7", Some((7, 0))),
            (".5", Some((5, 1))),
            ("5.", Some((5, 0))),
            ("\u{a0}7\u{3000}", Some((7, 0))),
            (&long, Some((digits31.parse().unwrap(), 31))),
            ("1e3", None),
            ("n/a", None),
            ("", None),
            (".", None),
            ("1.2.3", None),
            ("1 2", None),
            ("9999999999999999999999999999999999999999", None),
        ];
        for (text, want) in cases {
            let got = parse_decimal(text.as_bytes()).map(|n| (n.mantissa, n.decimals));
            assert_eq!(got, want, "{text:?}");
        }
        std::println!("parse_decimal: {} of {} texts read as the interpreter reads them", cases.len(), cases.len());
    }
}
