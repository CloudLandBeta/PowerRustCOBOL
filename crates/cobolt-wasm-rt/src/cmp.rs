// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Conditions: relations, class tests and sign tests.
//!
//! Condition-names (`88`) are not here: the generator expands each into
//! relations on the host item, which keeps one comparison routine.

use crate::codec;
use crate::fd::*;
use crate::mem;
use crate::mv::{digit_chars, scratch};
use crate::opd::*;
use crate::val::Val;
use core::cmp::Ordering;

pub const OP_EQ: u32 = 0;
pub const OP_NE: u32 = 1;
pub const OP_LT: u32 = 2;
pub const OP_LE: u32 = 3;
pub const OP_GT: u32 = 4;
pub const OP_GE: u32 = 5;

/// One side of a relation, resolved.
enum View {
    Num(Val, Fd),
    Mem(Loc),
    /// A figurative constant: repeats one byte as long as the other side needs.
    Fill(u8),
    /// `ALL literal`: repeats a pattern.
    Pattern(Loc),
}

fn view(base: u32, o: &Opd, other_numeric: bool) -> View {
    match o.tag {
        T_LIT_NUM => View::Num(value(base, o), o.fd),
        T_FIG => {
            if o.fig == FIG_ZERO && other_numeric {
                View::Num(Val::ZERO, o.fd)
            } else {
                View::Fill(fig_byte(o.fig))
            }
        }
        T_ALL => View::Pattern(locate(base, o)),
        T_LIT_ALNUM => View::Mem(locate(base, o)),
        _ => {
            let loc = locate(base, o);
            if loc.fd.is_numeric() {
                View::Num(codec::load_value(&loc), loc.fd)
            } else if loc.fd.kind == K_NUM_EDITED {
                View::Num(crate::edited::deedit_value(&loc), loc.fd)
            } else {
                View::Mem(loc)
            }
        }
    }
}

fn is_numeric_operand(base: u32, o: &Opd) -> bool {
    match o.tag {
        T_LIT_NUM => true,
        T_ITEM => {
            let loc = locate(base, o);
            loc.fd.is_numeric() || loc.fd.kind == K_NUM_EDITED
        }
        _ => false,
    }
}

fn holds(op: u32, ord: Ordering) -> bool {
    match op {
        OP_EQ => ord == Ordering::Equal,
        OP_NE => ord != Ordering::Equal,
        OP_LT => ord == Ordering::Less,
        OP_LE => ord != Ordering::Greater,
        OP_GT => ord == Ordering::Greater,
        _ => ord != Ordering::Less,
    }
}

fn numeric_relation(a: &Val, b: &Val, op: u32) -> bool {
    if let (Val::Dec(x), Val::Dec(y)) = (a, b) {
        return holds(op, x.cmp(y));
    }
    let (lf, rf) = (a.as_f64(), b.as_f64());
    match op {
        OP_EQ => (lf - rf).abs() < 1e-10,
        OP_NE => (lf - rf).abs() >= 1e-10,
        OP_LT => lf < rf,
        OP_LE => lf <= rf,
        OP_GT => lf > rf,
        _ => lf >= rf,
    }
}

/// The alphanumeric image of a number for comparison with characters: its
/// digit positions, in the scratch area.
fn num_as_chars(v: &Val, fd: &Fd) -> Loc {
    let mut buf = [0u8; 40];
    let n = digit_chars(v, fd, &mut buf);
    let (addr, _) = scratch();
    mem::bytes_mut(addr, n as u32).copy_from_slice(&buf[..n]);
    Loc { addr, len: n as u32, fd: Fd { kind: K_ALNUM, flags: 0, digits: 0, scale: 0, len: n as u32, aux: 0 } }
}

/// Byte `i` of a character operand, padded with spaces past its end.
fn byte_at(v: &View, i: u32) -> u8 {
    match v {
        View::Mem(l) => {
            if i < l.len {
                mem::rd_u8(l.addr + i)
            } else {
                b' '
            }
        }
        View::Fill(b) => *b,
        View::Pattern(l) => {
            if l.len == 0 {
                b' '
            } else {
                mem::rd_u8(l.addr + i % l.len)
            }
        }
        View::Num(..) => b' ',
    }
}

fn len_of(v: &View) -> Option<u32> {
    match v {
        View::Mem(l) => Some(l.len),
        _ => None,
    }
}

/// Collating weight of a byte under the program's collating sequence. The
/// native sequence is the byte order.
#[inline]
fn weight(b: u8) -> u8 {
    b
}

fn chars_relation(a: &View, b: &View, op: u32) -> bool {
    let n = match (len_of(a), len_of(b)) {
        (Some(x), Some(y)) => x.max(y),
        (Some(x), None) => x,
        (None, Some(y)) => y,
        (None, None) => 1,
    };
    let mut ord = Ordering::Equal;
    for i in 0..n {
        let (x, y) = (weight(byte_at(a, i)), weight(byte_at(b, i)));
        if x != y {
            ord = x.cmp(&y);
            break;
        }
    }
    holds(op, ord)
}

/// `a op b`.
pub fn relation(base: u32, a: u32, b: u32, op: u32) -> bool {
    let (oa, ob) = (parse(a), parse(b));
    let na = is_numeric_operand(base, &oa);
    let nb = is_numeric_operand(base, &ob);
    let va = view(base, &oa, nb);
    let vb = view(base, &ob, na);
    match (&va, &vb) {
        (View::Num(x, _), View::Num(y, _)) => numeric_relation(x, y, op),
        (View::Num(x, fx), other) => num_vs_chars(x, fx, other, op, false),
        (other, View::Num(y, fy)) => num_vs_chars(y, fy, other, op, true),
        _ => chars_relation(&va, &vb, op),
    }
}

/// A number against characters. As the interpreter does: when the characters
/// spell a number the comparison is numeric; otherwise it is alphanumeric, the
/// number standing for the digits it would have as an alphanumeric item.
fn num_vs_chars(n: &Val, nfd: &Fd, chars: &View, op: u32, swapped: bool) -> bool {
    if let View::Mem(l) = chars {
        if let Some(parsed) = codec::parse_text(mem::bytes(l.addr, l.len)) {
            let other = Val::Dec(parsed);
            return if swapped { numeric_relation(&other, n, op) } else { numeric_relation(n, &other, op) };
        }
    }
    let nv = View::Mem(num_as_chars(n, nfd));
    if swapped {
        chars_relation(chars, &nv, op)
    } else {
        chars_relation(&nv, chars, op)
    }
}

pub const CLASS_NUMERIC: u32 = 0;
pub const CLASS_ALPHABETIC: u32 = 1;
pub const CLASS_LOWER: u32 = 2;
pub const CLASS_UPPER: u32 = 3;

/// `x IS [NOT] class` (the `NOT` is the caller's).
pub fn class_test(base: u32, o: u32, class: u32) -> bool {
    let op = parse(o);
    let loc = locate(base, &op);
    let b = mem::bytes(loc.addr, loc.len);
    let fd = &loc.fd;
    match class {
        CLASS_NUMERIC => match fd.kind {
            K_NUM_DISPLAY => display_is_numeric(b, fd),
            K_PACKED => packed_is_numeric(b, fd),
            K_BINARY | K_COMP5 | K_FLOAT32 | K_FLOAT64 | K_INDEX => true,
            _ => !b.is_empty() && b.iter().all(|c| c.is_ascii_digit()),
        },
        CLASS_ALPHABETIC => !fd.is_numeric() && b.iter().all(|c| c.is_ascii_alphabetic() || *c == b' '),
        CLASS_LOWER => !fd.is_numeric() && b.iter().all(|c| c.is_ascii_lowercase() || *c == b' '),
        _ => !fd.is_numeric() && b.iter().all(|c| c.is_ascii_uppercase() || *c == b' '),
    }
}

fn display_is_numeric(b: &[u8], fd: &Fd) -> bool {
    let n = b.len();
    for (i, &c) in b.iter().enumerate() {
        if fd.sign_separate() && fd.signed() {
            let sign_pos = if fd.sign_leading() { i == 0 } else { i == n - 1 };
            if sign_pos {
                if c != b'+' && c != b'-' {
                    return false;
                }
                continue;
            }
        } else if fd.signed() {
            let sign_pos = if fd.sign_leading() { i == 0 } else { i == n - 1 };
            if sign_pos {
                if !matches!(c, b'0'..=b'9' | b'{' | b'}' | b'A'..=b'I' | b'J'..=b'R') {
                    return false;
                }
                continue;
            }
        }
        if !c.is_ascii_digit() {
            return false;
        }
    }
    true
}

fn packed_is_numeric(b: &[u8], fd: &Fd) -> bool {
    let last = b.len() - 1;
    for (i, &c) in b.iter().enumerate() {
        if (c >> 4) > 9 {
            return false;
        }
        if i != last && (c & 0x0F) > 9 {
            return false;
        }
    }
    let s = b[last] & 0x0F;
    if fd.signed() {
        matches!(s, 0x0A..=0x0F)
    } else {
        s == 0x0F || s == 0x0C
    }
}

pub const SIGN_POSITIVE: u32 = 0;
pub const SIGN_NEGATIVE: u32 = 1;
pub const SIGN_ZERO: u32 = 2;

/// `x IS [NOT] POSITIVE | NEGATIVE | ZERO`.
pub fn sign_test(base: u32, o: u32, sign: u32) -> bool {
    let v = value(base, &parse(o));
    let (pos, neg, zero) = match v {
        Val::Dec(n) => (n.mantissa > 0, n.mantissa < 0, n.mantissa == 0),
        Val::F(f) => (f > 0.0, f < 0.0, f == 0.0),
    };
    match sign {
        SIGN_POSITIVE => pos,
        SIGN_NEGATIVE => neg,
        _ => zero,
    }
}
