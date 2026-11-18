// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! National (`PIC N`) and UTF-8 (`PIC U`) data in a desktop module (DESKTOP-WASM-001 T44).
//!
//! The interpreter (spec 077, `cobolt_runtime::national`) keeps such an item's **text** and works
//! out its byte image only where bytes show: UTF-16 big-endian for national, UTF-8 padded with
//! X'20' for UTF-8. A module has nothing but bytes, so it keeps the **image**, and every write fits
//! the sender's characters to the item by the interpreter's rules (its `fit_class`): a character
//! that does not fit whole is left out, and the positions left over are spaces. The image is then
//! always the interpreter's `class_image` of the text it holds, and every read decodes it back (its
//! `class_text`).
//!
//! The generator gives such items the kinds [`K_NATIONAL16`] and [`K_UTF8`] in a desktop module
//! only. A browser module keeps describing them as it always has, so nothing here changes what a
//! browser module does. An `N"…"` or `U"…"` literal is its text in UTF-8, flagged [`F_CLASSED`]:
//! it is a national sender for the rule that cuts on a whole character.

use core::cmp::Ordering;

use crate::fd::*;
use crate::mem;
use crate::mv::Sender;
use crate::opd::*;

/// How a national or UTF-8 item holds its characters (the interpreter's `CharClass`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// `PIC N(n)`: n UTF-16 code units, an image of 2·n bytes.
    National { chars: u32 },
    /// `PIC U(n)`: n characters, an image of 4·n bytes.
    Utf8 { chars: u32 },
    /// `PIC U BYTE-LENGTH n`: the whole characters that fit n bytes.
    Utf8Bytes { bytes: u32 },
}

impl Class {
    /// The class of an item described by `fd`, or `None` for any other item.
    pub fn of(fd: &Fd) -> Option<Class> {
        match fd.kind {
            K_NATIONAL16 => Some(Class::National { chars: fd.aux }),
            K_UTF8 if fd.aux == 0 => Some(Class::Utf8Bytes { bytes: fd.len }),
            K_UTF8 => Some(Class::Utf8 { chars: fd.aux }),
            _ => None,
        }
    }

    /// The item's size in bytes: what `BYTE-LENGTH` answers.
    pub fn width(self) -> u32 {
        match self {
            Class::National { chars } => 2 * chars,
            Class::Utf8 { chars } => 4 * chars,
            Class::Utf8Bytes { bytes } => bytes,
        }
    }

    /// The item's size in character positions: what `LENGTH` answers (a `BYTE-LENGTH` item's
    /// positions are its bytes).
    pub fn positions(self) -> u32 {
        match self {
            Class::National { chars } | Class::Utf8 { chars } => chars,
            Class::Utf8Bytes { bytes } => bytes,
        }
    }

    /// `HIGH-VALUE` in this class: U+FFFF for national, U+10FFFF for UTF-8.
    fn high_value(self) -> char {
        match self {
            Class::National { .. } => '\u{FFFF}',
            _ => '\u{10FFFF}',
        }
    }

    /// How many positions one character takes.
    fn cost(self, c: char) -> u32 {
        match self {
            Class::National { .. } => c.len_utf16() as u32,
            Class::Utf8 { .. } => 1,
            Class::Utf8Bytes { .. } => c.len_utf8() as u32,
        }
    }

    /// Room for characters, in the units [`Class::cost`] counts.
    fn capacity(self) -> u32 {
        self.positions()
    }
}

// ── text sources ─────────────────────────────────────────────────────────────

/// Characters to be fitted, compared or shown, wherever they come from. Each can be read more
/// than once ([`Text::chars`]), which a right-justified fit needs.
#[derive(Debug, Clone, Copy)]
pub enum Text<'a> {
    /// UTF-8 bytes; what is not UTF-8 reads as U+FFFD, as `String::from_utf8_lossy` reads it.
    Utf8(&'a [u8]),
    /// An item's image, read as its class reads it (`class_text`): decoded, fitted, padded.
    Item(Class, &'a [u8]),
    /// One character `n` times (a figurative constant).
    Fill(char, u32),
    /// UTF-8 text `n` times over (`ALL literal`).
    Repeat(&'a [u8], u32),
}

impl<'a> Text<'a> {
    pub fn chars(&self) -> Chars<'a> {
        match *self {
            Text::Utf8(b) => Chars::Utf8(Lossy::new(b)),
            Text::Item(class, b) => Chars::Item {
                inner: match class {
                    Class::National { .. } => Decode::Utf16(Utf16Be { b, at: 0, pending: None }),
                    _ => Decode::Utf8(Lossy::new(b)),
                },
                class,
                used: 0,
                cut: false,
            },
            Text::Fill(c, n) => Chars::Fill(c, n),
            Text::Repeat(unit, n) => Chars::Repeat { unit, left: n, cur: Lossy::new(unit) },
        }
    }
}

/// The characters of a [`Text`].
pub enum Chars<'a> {
    Utf8(Lossy<'a>),
    Item { inner: Decode<'a>, class: Class, used: u32, cut: bool },
    Fill(char, u32),
    Repeat { unit: &'a [u8], left: u32, cur: Lossy<'a> },
}

pub enum Decode<'a> {
    Utf8(Lossy<'a>),
    Utf16(Utf16Be<'a>),
}

impl Iterator for Decode<'_> {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        match self {
            Decode::Utf8(l) => l.next(),
            Decode::Utf16(u) => u.next(),
        }
    }
}

impl Iterator for Chars<'_> {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        match self {
            Chars::Utf8(l) => l.next(),
            Chars::Item { inner, class, used, cut } => {
                // `class_text`: the characters that fit, then spaces up to the capacity.
                let cap = class.capacity();
                if !*cut {
                    if let Some(c) = inner.next() {
                        let w = class.cost(c);
                        if *used + w <= cap {
                            *used += w;
                            return Some(c);
                        }
                    }
                    *cut = true;
                }
                if *used < cap {
                    *used += 1;
                    Some(' ')
                } else {
                    None
                }
            }
            Chars::Fill(c, n) => {
                if *n == 0 {
                    return None;
                }
                *n -= 1;
                Some(*c)
            }
            Chars::Repeat { unit, left, cur } => loop {
                if *left == 0 {
                    return None;
                }
                if let Some(c) = cur.next() {
                    return Some(c);
                }
                *left -= 1;
                *cur = Lossy::new(unit);
            },
        }
    }
}

/// The characters of UTF-8 bytes, each ill-formed sequence one U+FFFD (`from_utf8_lossy`).
pub struct Lossy<'a> {
    chunks: core::str::Utf8Chunks<'a>,
    valid: core::str::Chars<'a>,
    bad: bool,
}

impl<'a> Lossy<'a> {
    fn new(b: &'a [u8]) -> Lossy<'a> {
        Lossy { chunks: b.utf8_chunks(), valid: "".chars(), bad: false }
    }
}

impl Iterator for Lossy<'_> {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        loop {
            if let Some(c) = self.valid.next() {
                return Some(c);
            }
            if self.bad {
                self.bad = false;
                return Some('\u{FFFD}');
            }
            let chunk = self.chunks.next()?;
            self.valid = chunk.valid().chars();
            self.bad = !chunk.invalid().is_empty();
        }
    }
}

/// The characters of UTF-16 big-endian bytes: an unpaired surrogate, and an odd last byte, read as
/// U+FFFD (the interpreter's `from_utf16be`).
pub struct Utf16Be<'a> {
    b: &'a [u8],
    at: usize,
    pending: Option<u16>,
}

impl Utf16Be<'_> {
    fn unit(&mut self) -> Option<u16> {
        if let Some(u) = self.pending.take() {
            return Some(u);
        }
        let b = self.b;
        if self.at >= b.len() {
            return None;
        }
        let u = if self.at + 1 < b.len() { u16::from_be_bytes([b[self.at], b[self.at + 1]]) } else { 0xFFFD };
        self.at += 2;
        Some(u)
    }
}

impl Iterator for Utf16Be<'_> {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        let u = self.unit()?;
        if !(0xD800..0xE000).contains(&u) {
            return Some(char::from_u32(u as u32).unwrap_or('\u{FFFD}'));
        }
        if u >= 0xDC00 {
            return Some('\u{FFFD}');
        }
        match self.unit() {
            Some(lo) if (0xDC00..0xE000).contains(&lo) => {
                let c = 0x10000 + (((u as u32) - 0xD800) << 10) + ((lo as u32) - 0xDC00);
                Some(char::from_u32(c).unwrap_or('\u{FFFD}'))
            }
            Some(other) => {
                self.pending = Some(other);
                Some('\u{FFFD}')
            }
            None => Some('\u{FFFD}'),
        }
    }
}

// ── fitting ──────────────────────────────────────────────────────────────────

/// Put one character into `out` at `at` in the class's encoding; the bytes written.
fn put(class: Class, c: char, out: &mut [u8], at: usize) -> usize {
    match class {
        Class::National { .. } => {
            let mut units = [0u16; 2];
            let n = c.encode_utf16(&mut units).len();
            for (k, u) in units[..n].iter().enumerate() {
                out[at + 2 * k..at + 2 * k + 2].copy_from_slice(&u.to_be_bytes());
            }
            2 * n
        }
        _ => c.encode_utf8(&mut out[at..]).len(),
    }
}

/// Write `text` fitted to `class` into `out` (the item's image, [`Class::width`] bytes): the
/// interpreter's `class_image(fit_class(class, text, justified))`. Left-justified, the characters
/// that fit whole, then spaces; `JUSTIFIED RIGHT`, the rightmost characters of the text without its
/// trailing spaces, after spaces. A `PIC U(n)` image is padded with X'20' beyond its n positions.
pub fn write(class: Class, text: Text, justified: bool, out: &mut [u8]) {
    let cap = class.capacity();
    let mut at = 0usize;
    let mut used = 0u32;
    if !justified {
        for c in text.chars() {
            let w = class.cost(c);
            if used + w > cap {
                break;
            }
            used += w;
            at += put(class, c, out, at);
        }
    } else {
        // The significant part ends at its last character that is not a space: its cost, and how
        // many characters it has.
        let (mut total, mut sig_cost, mut count, mut sig_count) = (0u32, 0u32, 0u32, 0u32);
        for c in text.chars() {
            total += class.cost(c);
            count += 1;
            if c != ' ' {
                sig_cost = total;
                sig_count = count;
            }
        }
        // The longest tail of it that fits: skip characters until what is left costs no more than
        // the capacity.
        let mut skip_cost = 0u32;
        let mut first = 0u32;
        for c in text.chars().take(sig_count as usize) {
            if sig_cost - skip_cost <= cap {
                break;
            }
            skip_cost += class.cost(c);
            first += 1;
        }
        let kept = sig_cost - skip_cost;
        for _ in 0..cap - kept {
            at += put(class, ' ', out, at);
        }
        for c in text.chars().skip(first as usize).take((sig_count - first) as usize) {
            at += put(class, c, out, at);
        }
        used = cap;
    }
    for _ in used..cap {
        at += put(class, ' ', out, at);
    }
    out[at..].fill(b' ');
}

/// The figurative constant a fill byte stands for, as text of `class` (the interpreter's
/// `figurative_text`): `ZERO` `0`, `SPACE` U+0020, `QUOTE` `"`, `LOW-VALUE` U+0000, `HIGH-VALUE`
/// the class's highest character; one per position.
fn fill_text(class: Class, byte: u8, zero: bool) -> Text<'static> {
    let c = if zero {
        '0'
    } else {
        match byte {
            0x00 => '\0',
            0xFF => class.high_value(),
            b if b < 0x80 => b as char,
            _ => '\u{FFFD}',
        }
    };
    Text::Fill(c, class.positions().max(1))
}

/// The text a sender gives a national or UTF-8 receiver of class `class`, and whether the
/// receiver's `JUSTIFIED RIGHT` applies to it (`MOVE` of a figurative constant ignores it; a
/// `VALUE` clause does not).
fn sender_text<'a>(snd: &Sender, class: Class, value_clause: bool) -> (Text<'a>, bool) {
    match snd {
        Sender::Bytes(l, _) => (text_of(l), true),
        Sender::Fill(b, zero) => (fill_text(class, *b, *zero), value_clause),
        Sender::All(l) => {
            let unit = mem::bytes(l.addr, l.len);
            let text = if unit.is_empty() { Text::Fill(' ', class.positions().max(1)) } else { Text::Repeat(unit, class.positions().max(1)) };
            (text, value_clause)
        }
        // A number reaches a national or UTF-8 item only through a VALUE clause, which the
        // interpreter reads as no text at all (the generator refuses such a MOVE).
        Sender::Num(..) => (Text::Fill(' ', 0), true),
    }
}

/// The characters at a location: an item of a class as that class reads it, anything else as
/// UTF-8.
pub fn text_of<'a>(l: &Loc) -> Text<'a> {
    let b = mem::bytes(l.addr, l.len);
    match Class::of(&l.fd) {
        Some(c) => Text::Item(c, b),
        None => Text::Utf8(b),
    }
}

/// Whether the two locations share a byte.
fn overlaps(a: &Loc, b: &Loc) -> bool {
    a.addr < b.addr + b.len && b.addr < a.addr + a.len
}

/// Store `snd` in the national or UTF-8 item at `d`, fitted to its class. `value_clause`: the
/// item's `VALUE`, which applies `JUSTIFIED RIGHT` to a figurative constant too.
pub fn store(snd: &Sender, d: &Loc, value_clause: bool) {
    let Some(class) = Class::of(&d.fd) else { return };
    let (text, justify) = sender_text(snd, class, value_clause);
    fit_into(class, text, justify && d.fd.justified(), d, snd);
}

/// The characters of a national or UTF-8 sender into an alphanumeric item: whole UTF-8
/// characters, as many as its bytes hold (the interpreter's cut on a whole character, spec 077 R8).
pub fn into_alnum(snd: &Sender, d: &Loc) {
    let Sender::Bytes(l, _) = snd else { return };
    let class = Class::Utf8Bytes { bytes: d.len };
    fit_into(class, text_of(l), d.fd.justified(), d, snd);
}

fn fit_into(class: Class, text: Text, justified: bool, d: &Loc, snd: &Sender) {
    // A sender that shares bytes with the receiver (a REDEFINES) is read whole before the
    // receiver is written: through the scratch area when it fits there.
    if let Sender::Bytes(l, _) = snd {
        if overlaps(l, d) {
            let (s, cap) = crate::mv::scratch();
            if d.len <= cap {
                write(class, text, justified, mem::bytes_mut(s, d.len));
                mem::copy(s, d.addr, d.len);
                return;
            }
        }
    }
    write(class, text, justified, mem::bytes_mut(d.addr, d.len));
}

/// Whether `o` is national or UTF-8: an item of a class, or an `N"…"`/`U"…"` literal.
pub fn is_classed(o: &Opd) -> bool {
    Class::of(&o.fd).is_some() || (o.tag == T_LIT_ALNUM && o.fd.flags & F_CLASSED != 0)
}

// ── comparison ───────────────────────────────────────────────────────────────

/// An operand of a comparison as text: an item of a class as its class reads it, characters as
/// UTF-8, `SPACE` as no characters (the shorter side is padded with spaces anyway).
fn cmp_text<'a>(base: u32, o: &Opd) -> Text<'a> {
    match o.tag {
        T_FIG => Text::Fill(' ', 0),
        _ => text_of(&locate(base, o)),
    }
}

/// A text as a boolean, and whether it is a WORD: `cobolt_forms::model::parse_bool_text` and
/// `is_bool_word` over its characters (`str::trim`, then `TRUE`/`FALSE` in any case, `1`, `0`).
fn boolean_of(t: &Text) -> (Option<bool>, bool) {
    let mut word = [0u8; 5];
    let (mut n, mut pending, mut started) = (0usize, 0usize, false);
    for c in t.chars() {
        if c.is_whitespace() {
            if started {
                pending += 1;
            }
            continue;
        }
        started = true;
        // Spaces inside the word are part of it.
        if n + pending + 1 > word.len() || !c.is_ascii() {
            return (None, false);
        }
        for _ in 0..pending {
            word[n] = b' ';
            n += 1;
        }
        pending = 0;
        word[n] = c as u8;
        n += 1;
    }
    let w = &word[..n];
    if w.eq_ignore_ascii_case(b"true") {
        (Some(true), true)
    } else if w.eq_ignore_ascii_case(b"false") {
        (Some(false), true)
    } else if w == b"1" {
        (Some(true), false)
    } else if w == b"0" {
        (Some(false), false)
    } else {
        (None, false)
    }
}

/// `a op b` when either is national or UTF-8: by code point, the shorter side padded with spaces,
/// as the interpreter's `compare_values` compares the two texts (UTF-8 order is code point order).
/// `TRUE`/`FALSE` spelled out on one side of `=` or `NOT =` make it a comparison of booleans, as
/// there.
pub fn relation(base: u32, oa: &Opd, ob: &Opd, op: u32) -> bool {
    let (ta, tb) = (cmp_text(base, oa), cmp_text(base, ob));
    if op == crate::cmp::OP_EQ || op == crate::cmp::OP_NE {
        let ((a, wa), (b, wb)) = (boolean_of(&ta), boolean_of(&tb));
        if wa || wb {
            if let (Some(a), Some(b)) = (a, b) {
                return if op == crate::cmp::OP_EQ { a == b } else { a != b };
            }
        }
    }
    let (mut ca, mut cb) = (ta.chars(), tb.chars());
    let ord = loop {
        match (ca.next(), cb.next()) {
            (None, None) => break Ordering::Equal,
            (x, y) => {
                let (x, y) = (x.unwrap_or(' '), y.unwrap_or(' '));
                if x != y {
                    break x.cmp(&y);
                }
            }
        }
    };
    crate::cmp::holds(op, ord)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(class: Class, text: &str, justified: bool) -> [u8; 64] {
        let mut out = [0u8; 64];
        write(class, Text::Utf8(text.as_bytes()), justified, &mut out[..class.width() as usize]);
        out
    }

    #[test]
    fn a_national_item_is_utf16_and_cuts_on_a_whole_character() {
        let c = Class::National { chars: 3 };
        assert_eq!(&image(c, "Ação", false)[..6], &[0, b'A', 0, 0xE7, 0, 0xE3]);
        // A supplementary character takes two positions: it does not fit in the last one.
        assert_eq!(&image(c, "ab😀", false)[..6], &[0, b'a', 0, b'b', 0, b' ']);
        assert_eq!(&image(c, "Ação", true)[..6], &[0, 0xE7, 0, 0xE3, 0, b'o']);
        assert_eq!(&image(c, "a  ", true)[..6], &[0, b' ', 0, b' ', 0, b'a']);
    }

    #[test]
    fn a_utf8_item_counts_characters_or_bytes() {
        let u = Class::Utf8 { chars: 3 };
        let img = image(u, "Configuração", false);
        assert_eq!(&img[..12], b"Con         ");
        let b = Class::Utf8Bytes { bytes: 5 };
        assert_eq!(&image(b, "Ação", false)[..5], "Açã".as_bytes());
        assert_eq!(&image(b, "Aço", false)[..5], "Aço ".as_bytes());
    }

    #[test]
    fn an_image_reads_back_as_its_class_reads_it() {
        let c = Class::National { chars: 4 };
        let img = image(c, "Ab", false);
        let t: String = Text::Item(c, &img[..8]).chars().collect();
        assert_eq!(t, "Ab  ");
        // An unpaired surrogate reads as U+FFFD.
        let bad = [0xD8u8, 0x00, 0x00, b'x'];
        let t: String = Text::Item(Class::National { chars: 2 }, &bad).chars().collect();
        assert_eq!(t, "\u{FFFD}x");
        // Bytes that are not UTF-8 read as U+FFFD, one per ill-formed sequence.
        let t: String = Text::Utf8(b"a\xFFb\xE2\x82").chars().collect();
        assert_eq!(t, "a\u{FFFD}b\u{FFFD}");
    }

    #[test]
    fn a_text_is_a_boolean_only_when_spelled_as_one() {
        assert_eq!(boolean_of(&Text::Utf8(b"  True  ")), (Some(true), true));
        assert_eq!(boolean_of(&Text::Utf8(b"0")), (Some(false), false));
        assert_eq!(boolean_of(&Text::Utf8(b"tr ue")), (None, false));
        assert_eq!(boolean_of(&Text::Utf8("ação".as_bytes())), (None, false));
        assert_eq!(boolean_of(&Text::Fill(' ', 0)), (None, false));
    }
}
