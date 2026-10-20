// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Reading a number out of an item's bytes, and writing one back.
//!
//! This is the byte-level half of the data model (WASM-010 R3), which the
//! interpreter — keeping values, not bytes — has no counterpart for. The
//! conventions follow the interpreter's record images where it has one (the
//! ASCII overpunch of `files.rs`: `{` and `A`–`I` for +0…+9, `}` and `J`–`R`
//! for −0…−9; a positive embedded sign is the plain digit) and the standard
//! elsewhere (`COMP` big-endian, `COMP-5` native order, `COMP-3` with `C`/`F`
//! positive and `D`/`B` negative).
//!
//! Storing follows the interpreter's `store_arith` and `truncate_to_capacity`:
//! `ROUNDED` rounds half away from zero (to a power of ten above the units for
//! a trailing `P`); an unsigned receiver keeps the magnitude; the high-order
//! digits that do not fit are cut, unless the statement has a size-error
//! phrase, in which case the receiver is left untouched and the caller learns
//! of the overflow.

use crate::fd::*;
use crate::opd::Loc;
use crate::val::Val;
use cobolt_decimal::{pow10, CobolNumeric};

/// The digit and sign an embedded-sign character carries.
fn de_overpunch(b: u8) -> Option<(u8, bool)> {
    match b {
        b'0'..=b'9' => Some((b - b'0', false)),
        b'{' => Some((0, false)),
        b'A'..=b'I' => Some((b - b'A' + 1, false)),
        b'}' => Some((0, true)),
        b'J'..=b'R' => Some((b - b'J' + 1, true)),
        // GnuCOBOL's ASCII convention for a negative: `p`..`y`.
        b'p'..=b'y' => Some((b - b'p', true)),
        _ => None,
    }
}

fn overpunch(digit: u8, negative: bool) -> u8 {
    match (digit, negative) {
        (d, false) => b'0' + d,
        (0, true) => b'}',
        (d, true) => b'J' + d - 1,
    }
}

/// A digit character's value; blanks read as zero, anything else as its low
/// nibble (so a stray byte never panics).
fn digit_of(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b' ' => 0,
        _ => b & 0x0F,
    }
}

fn scaled(mantissa: i128, fd: &Fd) -> Val {
    if fd.scale >= 0 {
        Val::Dec(CobolNumeric::new(mantissa, fd.scale as u8))
    } else {
        let p = (-(fd.scale as i32)) as u32;
        let m = match pow10(p) {
            Some(f) => mantissa.saturating_mul(f),
            None => mantissa,
        };
        Val::Dec(CobolNumeric::new(m, 0))
    }
}

fn load_display(loc: &Loc) -> Val {
    let fd = &loc.fd;
    let b = crate::mem::bytes(loc.addr, loc.len);
    let mut neg = false;
    let digits: &[u8];
    if fd.sign_separate() && fd.signed() {
        if fd.sign_leading() {
            neg = b[0] == b'-';
            digits = &b[1..];
        } else {
            neg = b[b.len() - 1] == b'-';
            digits = &b[..b.len() - 1];
        }
        return scaled(fold(digits, neg), fd);
    }
    let mut m: i128 = 0;
    let n = digits_len(b.len());
    for (i, &c) in b.iter().enumerate() {
        let sign_pos = if fd.sign_leading() { i == 0 } else { i == n - 1 };
        let d = if fd.signed() && sign_pos {
            match de_overpunch(c) {
                Some((d, ng)) => {
                    neg = ng;
                    d
                }
                None => digit_of(c),
            }
        } else {
            digit_of(c)
        };
        m = m.saturating_mul(10).saturating_add(d as i128);
    }
    if neg {
        m = -m;
    }
    scaled(m, fd)
}

fn digits_len(n: usize) -> usize {
    n
}

fn fold(digits: &[u8], neg: bool) -> i128 {
    let mut m: i128 = 0;
    for &c in digits {
        m = m.saturating_mul(10).saturating_add(digit_of(c) as i128);
    }
    if neg {
        -m
    } else {
        m
    }
}

fn load_binary(loc: &Loc, little: bool) -> Val {
    let fd = &loc.fd;
    let b = crate::mem::bytes(loc.addr, loc.len);
    let mut u: u128 = 0;
    if little {
        for &c in b.iter().rev() {
            u = (u << 8) | c as u128;
        }
    } else {
        for &c in b {
            u = (u << 8) | c as u128;
        }
    }
    let bits = (b.len() * 8) as u32;
    let m: i128 = if fd.signed() && bits < 128 && (u >> (bits - 1)) & 1 == 1 {
        (u as i128) - (1i128 << bits)
    } else {
        u as i128
    };
    scaled(m, fd)
}

fn load_packed(loc: &Loc) -> Val {
    let fd = &loc.fd;
    let b = crate::mem::bytes(loc.addr, loc.len);
    let mut m: i128 = 0;
    let last = b.len() - 1;
    for (i, &c) in b.iter().enumerate() {
        let hi = c >> 4;
        let lo = c & 0x0F;
        m = m.saturating_mul(10).saturating_add((hi.min(9)) as i128);
        if i != last {
            m = m.saturating_mul(10).saturating_add((lo.min(9)) as i128);
        }
    }
    let sign = b[last] & 0x0F;
    if fd.signed() && (sign == 0x0D || sign == 0x0B) {
        m = -m;
    }
    scaled(m, fd)
}

/// Parse the number an alphanumeric item's characters spell, as the
/// interpreter's `parse_decimal` does: optional sign, digits, optional point.
pub fn parse_text(b: &[u8]) -> Option<CobolNumeric> {
    let mut i = 0;
    let mut e = b.len();
    while i < e && b[i] == b' ' {
        i += 1;
    }
    while e > i && b[e - 1] == b' ' {
        e -= 1;
    }
    if i == e {
        return None;
    }
    let mut neg = false;
    if b[i] == b'-' {
        neg = true;
        i += 1;
    } else if b[i] == b'+' {
        i += 1;
    }
    let mut m: i128 = 0;
    let mut scale: u8 = 0;
    let mut seen_point = false;
    let mut any = false;
    for &c in &b[i..e] {
        match c {
            b'0'..=b'9' => {
                any = true;
                if seen_point {
                    if scale >= 31 {
                        continue; // cap fractional digits at 31, like the interpreter
                    }
                    scale += 1;
                }
                m = m.saturating_mul(10).saturating_add((c - b'0') as i128);
            }
            b'.' if !seen_point => seen_point = true,
            _ => return None,
        }
    }
    if !any {
        return None;
    }
    Some(CobolNumeric::new(if neg { -m } else { m }, scale))
}

/// The number an item holds. An alphanumeric item reads as the number its
/// characters spell, or zero.
pub fn load_value(loc: &Loc) -> Val {
    match loc.fd.kind {
        K_NUM_DISPLAY => load_display(loc),
        K_BINARY => load_binary(loc, false),
        K_COMP5 => load_binary(loc, true),
        K_INDEX => load_binary(loc, true),
        K_PACKED => load_packed(loc),
        K_FLOAT32 => {
            let b = crate::mem::bytes(loc.addr, 4);
            Val::F(f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64)
        }
        K_FLOAT64 => {
            let b = crate::mem::bytes(loc.addr, 8);
            let mut a = [0u8; 8];
            a.copy_from_slice(b);
            Val::F(f64::from_le_bytes(a))
        }
        K_NUM_EDITED => crate::edited::deedit_value(loc),
        _ => match parse_text(crate::mem::bytes(loc.addr, loc.len)) {
            Some(n) => Val::Dec(n),
            None => Val::F(0.0),
        },
    }
}

/// Write `v` into the numeric item at `loc`.
///
/// Returns `true` when the value did not fit (a size error). With
/// `has_phrase`, an overflowing receiver is left as it was; without, the
/// high-order digits are cut.
pub fn store_value(loc: &Loc, v: Val, rounded: bool, has_phrase: bool) -> bool {
    let fd = &loc.fd;
    if fd.kind == K_NUM_EDITED {
        return crate::edited::store_numeric_edited(loc, v, rounded, has_phrase);
    }
    if fd.is_float() {
        let f = v.as_f64();
        match fd.kind {
            K_FLOAT32 => crate::mem::bytes_mut(loc.addr, 4).copy_from_slice(&(f as f32).to_le_bytes()),
            _ => crate::mem::bytes_mut(loc.addr, 8).copy_from_slice(&f.to_le_bytes()),
        }
        return false;
    }
    // A float into a decimal receiver is rounded to the receiver's scale
    // whether or not ROUNDED was written, as the interpreter's assign does.
    let (n, rounded) = match v {
        Val::Dec(n) => (n, rounded),
        Val::F(f) => {
            let mut t = CobolNumeric::new(0, if fd.scale > 0 { fd.scale as u8 } else { 0 });
            t.set_f64(f);
            (t, false)
        }
    };
    // Bring the value to the item's scale.
    let (stored, p) = if fd.scale >= 0 {
        let s = fd.scale as u8;
        (if rounded { n.round_to(s) } else { n.truncate_to(s) }, 0u32)
    } else {
        let p = (-(fd.scale as i32)) as u32;
        let units = if rounded { n.round_to_power(p) } else { n.truncate_to(0) };
        (units, p)
    };
    let mut m = stored.mantissa;
    if p > 0 {
        if let Some(f) = pow10(p) {
            m /= f;
        }
    }
    let digits = fd.digits as u32;
    let overflow = match pow10(digits) {
        Some(limit) => m >= limit || m <= -limit,
        None => false,
    };
    if overflow && has_phrase {
        return true;
    }
    if overflow {
        if let Some(limit) = pow10(digits) {
            m %= limit;
        }
    }
    if !fd.signed() {
        m = m.abs();
    }
    write_integer(loc, m);
    overflow
}

/// Write an already-scaled integer into the item's bytes.
fn write_integer(loc: &Loc, m: i128) {
    let fd = &loc.fd;
    match fd.kind {
        K_NUM_DISPLAY => {
            let out = crate::mem::bytes_mut(loc.addr, loc.len);
            let neg = m < 0;
            let mut mag = m.unsigned_abs();
            let ndig = fd.digits as usize;
            let (start, end) = if fd.sign_separate() && fd.signed() {
                if fd.sign_leading() {
                    out[0] = if neg { b'-' } else { b'+' };
                    (1, out.len())
                } else {
                    let l = out.len();
                    out[l - 1] = if neg { b'-' } else { b'+' };
                    (0, l - 1)
                }
            } else {
                (0, out.len())
            };
            let w = end - start;
            debug_assert!(w == ndig);
            for i in (start..end).rev() {
                out[i] = b'0' + (mag % 10) as u8;
                mag /= 10;
            }
            if fd.signed() && neg && !fd.sign_separate() {
                let at = if fd.sign_leading() { start } else { end - 1 };
                out[at] = overpunch(out[at] - b'0', true);
            }
        }
        K_BINARY | K_COMP5 | K_INDEX => {
            let out = crate::mem::bytes_mut(loc.addr, loc.len);
            let n = out.len();
            let bits = (n * 8) as u32;
            let mut u: u128 = if m < 0 && bits < 128 { (m as u128) & ((1u128 << bits) - 1) } else { m as u128 };
            if fd.kind == K_BINARY {
                for i in (0..n).rev() {
                    out[i] = (u & 0xFF) as u8;
                    u >>= 8;
                }
            } else {
                for o in out.iter_mut() {
                    *o = (u & 0xFF) as u8;
                    u >>= 8;
                }
            }
        }
        K_PACKED => {
            let out = crate::mem::bytes_mut(loc.addr, loc.len);
            let neg = m < 0;
            let mut mag = m.unsigned_abs();
            let n = out.len();
            // The last nibble is the sign; digits fill nibbles from the right.
            out[n - 1] = ((mag % 10) as u8) << 4 | if !fd.signed() { 0x0F } else if neg { 0x0D } else { 0x0C };
            mag /= 10;
            for i in (0..n - 1).rev() {
                let lo = (mag % 10) as u8;
                mag /= 10;
                let hi = (mag % 10) as u8;
                mag /= 10;
                out[i] = hi << 4 | lo;
            }
        }
        _ => crate::fail(crate::state::err::BAD_OPERAND),
    }
}
