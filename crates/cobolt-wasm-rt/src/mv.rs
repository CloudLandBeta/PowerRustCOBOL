// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `MOVE`.
//!
//! The matrix of COBOL-85 6.18.4, by what the *sender* and the *receiver* are:
//!
//! | sender \ receiver | alphanumeric / group | numeric |
//! |---|---|---|
//! | **group or alphanumeric item/literal** | the bytes, left-justified (right with `JUSTIFIED`), space-padded or cut | a group sender: the bytes as they stand; an alphanumeric one: the number its characters spell, if they spell one |
//! | **numeric item** | its digit positions as characters, unsigned, no point, left-justified (the interpreter's `deedited_digits`) | the value, rescaled to the receiver (low-order digits cut, high-order digits cut; an unsigned receiver keeps the magnitude) |
//! | **numeric literal** | the digits of its magnitude, left-justified | as a numeric item |
//! | **figurative constant** | the receiver filled with its character (`ZERO` into a numeric item is the number 0) | |
//! | **`ALL literal`** | the literal repeated to fill the receiver | |
//!
//! Numeric-edited receivers are in [`crate::edited`].

use crate::codec;
use crate::fd::*;
use crate::mem;
use crate::opd::*;
use crate::state::{err, st, WS_SCRATCH, SCRATCH_SIZE};
use crate::val::Val;

/// A `MOVE` sender, resolved once so that a `MOVE a TO b c` reads `a` the same
/// way for both receivers.
#[derive(Debug, Clone, Copy)]
pub enum Sender {
    /// A number (item or literal), already read.
    Num(Val, Fd),
    /// Characters at a location.
    Bytes(Loc, bool),
    /// A figurative constant's fill byte.
    Fill(u8, bool),
    /// `ALL literal`: the pattern's location.
    All(Loc),
}

/// Resolve the sender operand.
pub fn sender(base: u32, o: &Opd) -> Sender {
    match o.tag {
        T_LIT_NUM => Sender::Num(value(base, o), o.fd),
        T_LIT_ALNUM => Sender::Bytes(locate(base, o), false),
        T_FIG => Sender::Fill(fig_byte(o.fig), o.fig == FIG_ZERO),
        T_ALL => Sender::All(locate(base, o)),
        _ => {
            let loc = locate(base, o);
            if loc.fd.is_numeric() {
                Sender::Num(codec::load_value(&loc), loc.fd)
            } else {
                Sender::Bytes(loc, loc.fd.kind == K_GROUP)
            }
        }
    }
}

/// `MOVE sender TO receiver`.
pub fn move_to(base: u32, snd: &Sender, dst: &Opd) {
    let d = locate(base, dst);
    if dst.tag != T_ITEM {
        crate::fail(err::BAD_OPERAND);
    }
    match d.fd.kind {
        K_NUM_DISPLAY | K_BINARY | K_COMP5 | K_PACKED | K_FLOAT32 | K_FLOAT64 | K_INDEX => {
            move_to_numeric(snd, &d)
        }
        K_NUM_EDITED | K_ALNUM_EDITED => crate::edited::move_to_edited(snd, &d),
        _ => move_to_alnum(snd, &d),
    }
}

fn move_to_numeric(snd: &Sender, d: &Loc) {
    match snd {
        Sender::Num(v, _) => {
            codec::store_value(d, *v, false, false);
        }
        Sender::Fill(_, true) => {
            codec::store_value(d, Val::ZERO, false, false);
        }
        Sender::Fill(b, false) => mem::fill(d.addr, d.len, *b),
        Sender::Bytes(loc, is_group) => {
            if *is_group {
                // A group sender makes the move alphanumeric: the bytes land as
                // they stand (COBOL-85 6.18.4), even in a numeric receiver.
                put_bytes(d, mem::bytes(loc.addr, loc.len), false);
            } else if let Some(n) = codec::parse_text(mem::bytes(loc.addr, loc.len)) {
                codec::store_value(d, Val::Dec(n), false, false);
            }
            // Characters that spell no number leave the receiver as it was.
        }
        Sender::All(loc) => {
            let pat = mem::bytes(loc.addr, loc.len);
            fill_repeat(d.addr, d.len, pat);
        }
    }
}

fn move_to_alnum(snd: &Sender, d: &Loc) {
    let just = d.fd.justified();
    match snd {
        Sender::Num(v, fd) => {
            // The digit positions as characters.
            let mut buf = [0u8; 40];
            let n = digit_chars(v, fd, &mut buf);
            put_bytes(d, &buf[..n], just);
        }
        Sender::Bytes(loc, _) => {
            // Overlap-safe: copy straight between the two ranges when no
            // justification or padding differs, otherwise via put_bytes.
            let src = *loc;
            move_bytes(&src, d, just);
        }
        Sender::Fill(b, _) => mem::fill(d.addr, d.len, *b),
        Sender::All(loc) => {
            let pat = mem::bytes(loc.addr, loc.len);
            fill_repeat(d.addr, d.len, pat);
        }
    }
}

/// Fill `len` bytes at `addr` by repeating `pat`.
pub fn fill_repeat(addr: u32, len: u32, pat: &[u8]) {
    if pat.is_empty() {
        return;
    }
    let out = mem::bytes_mut(addr, len);
    for (i, o) in out.iter_mut().enumerate() {
        *o = pat[i % pat.len()];
    }
}

/// Alphanumeric to alphanumeric between two locations, left-justified (or
/// right-justified) with space padding, truncating on the other side.
pub fn move_bytes(src: &Loc, dst: &Loc, right_justified: bool) {
    let (sl, dl) = (src.len, dst.len);
    if !right_justified {
        let n = sl.min(dl);
        mem::copy(src.addr, dst.addr, n);
        if dl > n {
            mem::fill(dst.addr + n, dl - n, b' ');
        }
    } else if sl >= dl {
        // The rightmost `dl` characters.
        mem::copy(src.addr + (sl - dl), dst.addr, dl);
    } else {
        // Move the characters first (the ranges may overlap), then blank the
        // left margin.
        mem::copy(src.addr, dst.addr + (dl - sl), sl);
        mem::fill(dst.addr, dl - sl, b' ');
    }
}

/// Put literal characters into a receiver as an alphanumeric move would.
pub fn put_bytes(d: &Loc, src: &[u8], right_justified: bool) {
    let dl = d.len as usize;
    let out = mem::bytes_mut(d.addr, d.len);
    if !right_justified {
        let n = src.len().min(dl);
        out[..n].copy_from_slice(&src[..n]);
        out[n..].fill(b' ');
    } else if src.len() >= dl {
        out.copy_from_slice(&src[src.len() - dl..]);
    } else {
        let pad = dl - src.len();
        out[..pad].fill(b' ');
        out[pad..].copy_from_slice(src);
    }
}

/// The characters a number contributes to an alphanumeric receiver: its digit
/// positions, absolute, zero-padded to the item's digit count; for a literal
/// (no declared digits) the digits of its magnitude. Returns the length.
pub fn digit_chars(v: &Val, fd: &Fd, out: &mut [u8; 40]) -> usize {
    let mag: u128 = match v {
        Val::Dec(n) => {
            // The stored integer: undo the P scaling, keep the decimals as digits.
            let mut m = n.mantissa.unsigned_abs();
            if fd.scale < 0 {
                if let Some(p) = cobolt_decimal::pow10((-(fd.scale as i32)) as u32) {
                    m /= p as u128;
                }
            }
            m
        }
        Val::F(f) => {
            let a = if *f < 0.0 { -*f } else { *f };
            a as u128
        }
    };
    let width = if fd.digits > 0 { fd.digits as usize } else { 0 };
    let mut tmp = [0u8; 40];
    let mut i = tmp.len();
    let mut m = mag;
    loop {
        i -= 1;
        tmp[i] = b'0' + (m % 10) as u8;
        m /= 10;
        if m == 0 {
            break;
        }
    }
    let n = tmp.len() - i;
    let total = n.max(width).min(out.len());
    let pad = total - n.min(total);
    for k in 0..pad {
        out[k] = b'0';
    }
    out[pad..total].copy_from_slice(&tmp[i..i + (total - pad)]);
    total
}

/// Scratch space in the workspace, for snapshots.
pub fn scratch() -> (u32, u32) {
    (st().ws + WS_SCRATCH, SCRATCH_SIZE)
}
