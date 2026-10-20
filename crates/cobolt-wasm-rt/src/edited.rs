// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Edited pictures: numeric-edited receivers (the engine is `cobolt-numedit`,
//! shared with the interpreter and the form painter) and alphanumeric-edited
//! ones (insertion characters around the source characters).
//!
//! The picture lives in the constant area; an edited item's descriptor holds
//! its address in `aux`:
//!
//! ```text
//! 0 tlen u16   length of the template
//! 2 decimal_comma u8
//! 3 currency u8
//! 4 template bytes (as written: `ZZ,ZZ9.99CR`, `X(3)B99`)
//! ```

use crate::codec;
use crate::fd::*;
use crate::mem;
use crate::mv::{self, Sender};
use crate::opd::Loc;
use crate::state::err;
use crate::val::Val;
use cobolt_decimal::CobolNumeric;
use cobolt_numedit as engine;

#[derive(Clone, Copy)]
pub struct Spec {
    pub template: &'static [u8],
    pub decimal_comma: bool,
    pub currency: u8,
}

pub fn spec(fd: &Fd) -> Spec {
    if fd.aux == 0 {
        crate::fail(err::BAD_OPERAND);
    }
    let h = mem::bytes(fd.aux, 4);
    let tlen = u16::from_le_bytes([h[0], h[1]]) as u32;
    Spec { template: mem::bytes(fd.aux + 4, tlen), decimal_comma: h[2] != 0, currency: h[3] }
}

/// The number an edited item's characters spell.
pub fn deedit_value(loc: &Loc) -> Val {
    let sp = spec(&loc.fd);
    let (m, d) = engine::deedit(mem::bytes(loc.addr, loc.len), sp.decimal_comma);
    Val::Dec(CobolNumeric::new(m, d))
}

/// Write `v` into the numeric-edited item at `loc`; `true` on a size error.
pub fn store_numeric_edited(loc: &Loc, v: Val, rounded: bool, has_phrase: bool) -> bool {
    let sp = spec(&loc.fd);
    let (int_d, frac_d) = engine::digit_counts(sp.template, sp.decimal_comma);
    let n = match v {
        Val::Dec(n) => n,
        Val::F(f) => {
            let mut t = CobolNumeric::new(0, frac_d.min(255) as u8);
            t.set_f64(f);
            t
        }
    };
    let stored = if rounded && matches!(v, Val::Dec(_)) { n.round_to(frac_d.min(255) as u8) } else { n };
    let overflow = stored.integer_digit_count() as usize > int_d;
    if overflow && has_phrase {
        return true;
    }
    let mut buf = [0u8; engine::MAX_OUT];
    let w = engine::format_edited(sp.template, stored.mantissa, stored.decimals, sp.decimal_comma, sp.currency, &mut buf);
    let out = mem::bytes_mut(loc.addr, loc.len);
    let k = w.min(out.len());
    out[..k].copy_from_slice(&buf[..k]);
    out[k..].fill(b' ');
    overflow
}

/// `MOVE` into an edited receiver.
pub fn move_to_edited(snd: &Sender, d: &Loc) {
    match d.fd.kind {
        K_NUM_EDITED => match snd {
            Sender::Num(v, _) => {
                store_numeric_edited(d, *v, false, false);
            }
            Sender::Fill(_, true) => {
                store_numeric_edited(d, Val::ZERO, false, false);
            }
            Sender::Fill(b, false) => mem::fill(d.addr, d.len, *b),
            Sender::Bytes(loc, is_group) => {
                if loc.fd.kind == K_NUM_EDITED && !*is_group {
                    let v = deedit_value(loc);
                    store_numeric_edited(d, v, false, false);
                } else if *is_group {
                    mv::put_bytes(d, mem::bytes(loc.addr, loc.len), false);
                } else if let Some(n) = codec::parse_text(mem::bytes(loc.addr, loc.len)) {
                    store_numeric_edited(d, Val::Dec(n), false, false);
                }
            }
            Sender::All(loc) => mv::fill_repeat(d.addr, d.len, mem::bytes(loc.addr, loc.len)),
        },
        _ => move_to_alnum_edited(snd, d),
    }
}

/// Alphanumeric-edited: the source characters fill the `X`/`A`/`9` positions
/// in order; `B`, `0` and `/` insert themselves.
fn move_to_alnum_edited(snd: &Sender, d: &Loc) {
    let sp = spec(&d.fd);
    // The source characters.
    let mut digits = [0u8; 40];
    let src: &[u8] = match snd {
        Sender::Num(v, fd) => {
            let n = mv::digit_chars(v, fd, &mut digits);
            &digits[..n]
        }
        Sender::Bytes(loc, _) => mem::bytes(loc.addr, loc.len),
        Sender::All(loc) => mem::bytes(loc.addr, loc.len),
        Sender::Fill(b, _) => {
            // Fill every character position with the figurative's character.
            digits[0] = *b;
            &digits[..1]
        }
    };
    let fill_all = matches!(snd, Sender::Fill(..));
    let out = mem::bytes_mut(d.addr, d.len);
    let mut si = 0usize; // next source character
    let mut oi = 0usize;
    let t = sp.template;
    let mut i = 0usize;
    while i < t.len() && oi < out.len() {
        let c = t[i].to_ascii_uppercase();
        let mut count = 1usize;
        // `X(5)`: a repeat count follows a symbol.
        if i + 1 < t.len() && t[i + 1] == b'(' {
            let mut j = i + 2;
            let mut n = 0usize;
            while j < t.len() && t[j].is_ascii_digit() {
                n = n * 10 + (t[j] - b'0') as usize;
                j += 1;
            }
            if j < t.len() && t[j] == b')' {
                count = n.max(1);
                i = j;
            }
        }
        i += 1;
        for _ in 0..count {
            if oi >= out.len() {
                break;
            }
            match c {
                b'X' | b'A' | b'9' => {
                    out[oi] = if fill_all { src[0] } else if si < src.len() { src[si] } else { b' ' };
                    si += 1;
                    oi += 1;
                }
                b'B' => {
                    out[oi] = b' ';
                    oi += 1;
                }
                b'0' => {
                    out[oi] = b'0';
                    oi += 1;
                }
                b'/' => {
                    out[oi] = b'/';
                    oi += 1;
                }
                _ => {}
            }
        }
    }
    while oi < out.len() {
        out[oi] = b' ';
        oi += 1;
    }
}
