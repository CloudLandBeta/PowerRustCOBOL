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
use crate::mv::scratch;
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

fn view(base: u32, o: &Opd, other_numeric: bool, other_strict: bool) -> View {
    match o.tag {
        T_LIT_NUM | T_TEMP => View::Num(value(base, o), o.fd),
        // A property read that spells a number is one against a number.
        T_CTEMP if other_numeric && crate::func::ctemp_number(o.off).is_some() => View::Num(value(base, o), o.fd),
        T_FIG => {
            // `ZERO` is the number against a number, and the character '0'
            // against anything else — an edited item included, which is
            // characters here (`IF EDIT = ZERO` with `0009999` holding
            // "0000000", NC126A LEV-TEST-GF-3-19).
            let _ = other_numeric;
            if o.fig == FIG_ZERO && other_strict {
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
            } else if loc.fd.kind == K_NUM_EDITED && other_numeric {
                // Against a number an edited item is its value; against
                // characters it is the characters it holds (`IF EDIT = SPACES`).
                View::Num(crate::edited::deedit_value(&loc), loc.fd)
            } else {
                View::Mem(loc)
            }
        }
    }
}

fn is_numeric_operand(base: u32, o: &Opd) -> bool {
    match o.tag {
        T_LIT_NUM | T_TEMP => true,
        T_CTEMP => crate::func::ctemp_number(o.off).is_some(),
        T_ITEM => {
            let loc = locate(base, o);
            loc.fd.is_numeric() || loc.fd.kind == K_NUM_EDITED
        }
        _ => false,
    }
}

/// A number proper: a numeric literal or an item of a numeric usage (not an
/// edited one, whose class is alphanumeric).
fn is_strict_numeric(base: u32, o: &Opd) -> bool {
    match o.tag {
        T_LIT_NUM | T_TEMP => true,
        T_CTEMP => crate::func::ctemp_number(o.off).is_some(),
        T_ITEM => locate(base, o).fd.is_numeric(),
        _ => false,
    }
}

/// A numeric item with `BLANK WHEN ZERO` that holds zero (and so is blanks).
fn is_blank_zero(base: u32, o: &Opd) -> bool {
    if o.tag != T_ITEM {
        return false;
    }
    let loc = locate(base, o);
    loc.fd.is_numeric() && loc.fd.blank_when_zero() && matches!(codec::load_value(&loc), Val::Dec(d) if d.mantissa == 0)
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

pub fn numeric_relation(a: &Val, b: &Val, op: u32) -> bool {
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
    let n = crate::mv::image_chars(v, fd, &mut buf);
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
fn weight(b: u8) -> u16 {
    let c = crate::state::st().collate;
    if c == 0 {
        b as u16
    } else {
        let w = mem::bytes(c + 2 + 2 * b as u32, 2);
        u16::from_le_bytes([w[0], w[1]])
    }
}

/// The length a figurative constant or `ALL literal` keeps when the other
/// operand has none to give it: one character, or the literal once.
fn natural_len(v: &View) -> u32 {
    match v {
        View::Mem(l) | View::Pattern(l) => l.len,
        View::Fill(_) => 1,
        View::Num(..) => 0,
    }
}

fn chars_relation(a: &View, b: &View, op: u32) -> bool {
    let n = match (len_of(a), len_of(b)) {
        (Some(x), Some(y)) => x.max(y),
        (Some(x), None) => x,
        (None, Some(y)) => y,
        (None, None) => 1,
    };
    // An EMPTY operand (a property that reads "") gives a figurative constant
    // no size: the interpreter's `size_figurative` leaves it at its own one
    // character, or `ALL literal` at the literal, and pads the empty side
    // with spaces — so `= ZERO`, `= QUOTE`, `= ALL "x"` do not hold. Sized to
    // nothing, no byte was compared and everything was equal.
    let n = if n == 0 { natural_len(a).max(natural_len(b)) } else { n };
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

/// The truth an operand stands for in an equality between booleans, and
/// whether it is spelled as a boolean WORD (`TRUE`/`FALSE`, any case) — the
/// interpreter's `compare_values` rule: a number is a boolean only as 0 or 1,
/// characters only as `TRUE`/`FALSE`/`1`/`0` around spaces.
///
/// A figurative constant is the value the interpreter's `literal_to_value`
/// gives it: `ZERO` and `NULL` are the number 0 (so `IF SW::Checked = ZERO`
/// holds for a switch that is off), `SPACE`, `QUOTE`, `HIGH-VALUE` and
/// `LOW-VALUE` are characters no boolean is spelled with. `ALL literal` is its
/// literal repeated to the other operand's length (`other_len`), as
/// `size_figuratives` sizes it against a nonnumeric operand.
fn boolean_of(base: u32, o: &Opd, other_len: Option<u32>) -> (Option<bool>, bool) {
    let number = |v: Val| -> (Option<bool>, bool) {
        let f = v.as_f64();
        (if f == 0.0 { Some(false) } else if f == 1.0 { Some(true) } else { None }, false)
    };
    match o.tag {
        T_LIT_NUM | T_TEMP => number(value(base, o)),
        T_FIG if o.fig == FIG_ZERO || o.fig == FIG_NULL => (Some(false), false),
        T_ALL => {
            let l = locate(base, o);
            let n = other_len.unwrap_or(l.len) as usize;
            let mut buf = [b' '; 8];
            if l.len == 0 || n == 0 || n > buf.len() {
                // Longer than any boolean spelling: it cannot be one.
                return (None, false);
            }
            let unit = mem::bytes(l.addr, l.len);
            for (i, b) in buf[..n].iter_mut().enumerate() {
                *b = unit[i % unit.len()];
            }
            boolean_text(&buf[..n])
        }
        T_LIT_ALNUM | T_CTEMP => {
            let l = locate(base, o);
            boolean_text(mem::bytes(l.addr, l.len))
        }
        T_ITEM => {
            let l = locate(base, o);
            if l.fd.is_numeric() {
                number(codec::load_value(&l))
            } else {
                boolean_text(mem::bytes(l.addr, l.len))
            }
        }
        _ => (None, false),
    }
}

/// Characters as a boolean: `TRUE`/`FALSE` (any case, a word) or `1`/`0`,
/// around spaces (`cobolt_forms::model::parse_bool_text`).
fn boolean_text(b: &[u8]) -> (Option<bool>, bool) {
    let s = b.iter().position(|&x| x != b' ').unwrap_or(b.len());
    let e = b.iter().rposition(|&x| x != b' ').map_or(s, |i| i + 1).max(s);
    let t = &b[s..e];
    if t.eq_ignore_ascii_case(b"true") {
        (Some(true), true)
    } else if t.eq_ignore_ascii_case(b"false") {
        (Some(false), true)
    } else if t == b"1" {
        (Some(true), false)
    } else if t == b"0" {
        (Some(false), false)
    } else {
        (None, false)
    }
}

/// The length a figurative `ALL literal` is sized to against `o`: the length of
/// a nonnumeric operand, none against a number or another figurative.
fn char_len(base: u32, o: &Opd) -> Option<u32> {
    match o.tag {
        T_LIT_ALNUM | T_CTEMP => Some(locate(base, o).len),
        T_ITEM => {
            let l = locate(base, o);
            (!l.fd.is_numeric()).then_some(l.len)
        }
        _ => None,
    }
}

/// `TRUE`/`FALSE` spelled out on one side of `=` or `NOT =` make it a
/// comparison of booleans, as on the desktop: a Switch's `Checked` reads
/// `true` and `IF SW::Checked = 1` holds. `None` when the rule does not apply.
fn boolean_relation(base: u32, oa: &Opd, ob: &Opd, op: u32) -> Option<bool> {
    if op != OP_EQ && op != OP_NE {
        return None;
    }
    let (a, wa) = boolean_of(base, oa, char_len(base, ob));
    let (b, wb) = boolean_of(base, ob, char_len(base, oa));
    if !(wa || wb) {
        return None;
    }
    let (a, b) = (a?, b?);
    Some(if op == OP_EQ { a == b } else { a != b })
}

// ── a property against a figurative constant ──────────────────────────────

/// A character temporary the host filled with a property's (or a method's)
/// text.
fn is_property(o: &Opd) -> bool {
    o.tag == T_CTEMP && crate::func::ctemp_is_property(o.off)
}

/// The figurative side, as the interpreter's `literal_to_value` gives it and
/// `size_figurative` sizes it: `ZERO`/`NULL` the number 0; any other constant
/// its one byte repeated; `ALL literal` the literal repeated; `len` bytes.
enum Fig {
    Zero,
    Byte(u8, u32),
    Pat(Loc, u32),
}

impl Fig {
    fn len(&self) -> u32 {
        match self {
            Fig::Zero => 1,
            Fig::Byte(_, n) | Fig::Pat(_, n) => *n,
        }
    }
    /// Byte `i` of its display text (`"0"` for the number 0).
    fn at(&self, i: u32) -> u8 {
        match self {
            Fig::Zero => b'0',
            Fig::Byte(b, _) => *b,
            Fig::Pat(l, _) => mem::rd_u8(l.addr + i % l.len),
        }
    }
}

/// Rust's `char::is_whitespace` for ASCII: what `str::trim` drops.
fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// A text as a boolean (`cobolt_forms::model::parse_bool_text`: `true`/`false`
/// in any case, `1`, `0`, around white space), and whether it is a WORD
/// (`is_bool_word`), read through `at` over `len` bytes.
fn bool_of_text(len: u32, at: impl Fn(u32) -> u8) -> (Option<bool>, bool) {
    let mut s = 0;
    while s < len && is_ws(at(s)) {
        s += 1;
    }
    let mut e = len;
    while e > s && is_ws(at(e - 1)) {
        e -= 1;
    }
    if e - s > 5 {
        return (None, false);
    }
    let mut w = [0u8; 5];
    for i in s..e {
        w[(i - s) as usize] = at(i);
    }
    boolean_text(&w[..(e - s) as usize])
}

/// The text read through `at` as Rust's `str::trim().parse::<f64>()` reads it.
fn f64_of_text(len: u32, at: impl Fn(u32) -> u8) -> Option<f64> {
    let mut buf = [0u8; 64];
    let mut s = 0;
    while s < len && is_ws(at(s)) {
        s += 1;
    }
    let mut e = len;
    while e > s && is_ws(at(e - 1)) {
        e -= 1;
    }
    if e - s > buf.len() as u32 {
        return None;
    }
    for i in s..e {
        buf[(i - s) as usize] = at(i);
    }
    core::str::from_utf8(&buf[..(e - s) as usize]).ok()?.parse::<f64>().ok()
}

/// A property read against a figurative constant or `ALL literal`, as the
/// desktop compares them (`eval_condition`: `prop_to_value`, then
/// `size_figuratives`, then `compare_values`). The property is a number when
/// its text spells one, else its characters; `ZERO` and `NULL` are the number
/// 0; another constant or `ALL literal` is sized to a property that is
/// characters — but never against a number, so `ALL "TRUE"` stays `TRUE`
/// against a toggle that reads `1` and is equal to it.
fn property_vs_figurative(base: u32, oa: &Opd, ob: &Opd, op: u32) -> Option<bool> {
    let fig_like = |o: &Opd| o.tag == T_FIG || o.tag == T_ALL;
    let (p, f, swapped) = if is_property(oa) && fig_like(ob) {
        (oa, ob, false)
    } else if is_property(ob) && fig_like(oa) {
        (ob, oa, true)
    } else {
        return None;
    };
    let (pa, pl) = crate::func::ctemp_loc(p.off);
    let ptext = mem::bytes(pa, pl);
    let pnum = codec::parse_text(ptext).map(Val::Dec);
    let width = if pnum.is_some() { 0 } else { pl };
    let fig = if f.tag == T_FIG && (f.fig == FIG_ZERO || f.fig == FIG_NULL) {
        Fig::Zero
    } else if f.tag == T_FIG {
        Fig::Byte(fig_byte(f.fig), width.max(1))
    } else {
        let l = locate(base, f);
        if l.len == 0 {
            Fig::Byte(b' ', 1)
        } else {
            let n = if width == 0 { l.len } else { width };
            Fig::Pat(l, n)
        }
    };
    let p_at = |i: u32| ptext[i as usize];
    let f_at = |i: u32| fig.at(i);
    let flen = fig.len();
    // Booleans first, for = and NOT = with a WORD on one side.
    if op == OP_EQ || op == OP_NE {
        let (pb, pw) = match &pnum {
            Some(v) => {
                let x = v.as_f64();
                (if x == 0.0 { Some(false) } else if x == 1.0 { Some(true) } else { None }, false)
            }
            None => bool_of_text(pl, p_at),
        };
        let (fb, fw) = match fig {
            Fig::Zero => (Some(false), false),
            _ => bool_of_text(flen, f_at),
        };
        if pw || fw {
            if let (Some(a), Some(b)) = (pb, fb) {
                return Some(if op == OP_EQ { a == b } else { a != b });
            }
        }
    }
    let order = |pv: Val, fv: Val| -> bool {
        if swapped {
            numeric_relation(&fv, &pv, op)
        } else {
            numeric_relation(&pv, &fv, op)
        }
    };
    match (&pnum, &fig) {
        (Some(v), Fig::Zero) => return Some(order(*v, Val::ZERO)),
        (Some(v), _) => {
            if let Some(x) = f64_of_text(flen, f_at) {
                return Some(order(Val::F(v.as_f64()), Val::F(x)));
            }
        }
        (None, Fig::Zero) => {
            if let Some(x) = f64_of_text(pl, p_at) {
                return Some(order(Val::F(x), Val::F(0.0)));
            }
        }
        (None, _) => {}
    }
    // Characters: the property's text (a number as the interpreter writes
    // it) against the constant's, the shorter padded with spaces.
    let mut num = [0u8; 64];
    let (pt, pn): (&[u8], u32) = match &pnum {
        Some(Val::Dec(d)) => {
            let n = crate::disp::format_dec(d, &mut num);
            (&num[..n], n as u32)
        }
        _ => (ptext, pl),
    };
    let n = pn.max(flen);
    let mut ord = Ordering::Equal;
    for i in 0..n {
        let x = weight(if i < pn { pt[i as usize] } else { b' ' });
        let y = weight(if i < flen { fig.at(i) } else { b' ' });
        if x != y {
            ord = x.cmp(&y);
            break;
        }
    }
    Some(holds(op, if swapped { ord.reverse() } else { ord }))
}

/// `a op b`.
pub fn relation(base: u32, a: u32, b: u32, op: u32) -> bool {
    let (oa, ob) = (parse(a), parse(b));
    if let Some(r) = property_vs_figurative(base, &oa, &ob, op) {
        return r;
    }
    if let Some(r) = boolean_relation(base, &oa, &ob, op) {
        return r;
    }
    let na = is_numeric_operand(base, &oa);
    let nb = is_numeric_operand(base, &ob);
    let (sa, sb) = (is_strict_numeric(base, &oa), is_strict_numeric(base, &ob));
    // A `BLANK WHEN ZERO` item holding zero is spaces to a character operand
    // (its bytes are blanks) and zero to a number (NC107A BZERO-TEST-2).
    let against_chars = |o: &Opd| !is_numeric_operand(base, o) && !(o.tag == T_FIG && o.fig == FIG_ZERO);
    let va = if is_blank_zero(base, &oa) && against_chars(&ob) { View::Mem(locate(base, &oa)) } else { view(base, &oa, nb, sb) };
    let vb = if is_blank_zero(base, &ob) && against_chars(&oa) { View::Mem(locate(base, &ob)) } else { view(base, &ob, na, sa) };
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
    // COBOL-85 6.3.4.2: against an alphanumeric operand the number is compared
    // as though it had been moved to an alphanumeric item of its own size — its
    // digits, no sign (NC103A IF-TEST-GF-98, "pseudo-move to strip minus sign").
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

/// `x IS [NOT] class-name` for a `SPECIAL-NAMES. CLASS` of the program: the
/// 32-byte bitmap at `table` lists the characters that belong. True when the
/// operand is non-empty and every character belongs.
pub fn user_class(base: u32, o: u32, table: u32) -> bool {
    let loc = locate(base, &parse(o));
    let b = mem::bytes(loc.addr, loc.len);
    let t = mem::bytes(table, 32);
    !b.is_empty() && b.iter().all(|c| t[(*c >> 3) as usize] & (1 << (*c & 7)) != 0)
}
