// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Numeric-edited PICTURE engine, `no_std` and allocation-free.
//!
//! Applies COBOL editing symbols when a numeric value is moved into a
//! numeric-edited field (`PIC ZZZ,ZZ9.99`, `$$$,$$9.99`, `----9.99`,
//! `9(6).99CR`…).
//!
//! It is the engine of `cobolt-forms/src/numedit.rs` moved to a crate of its
//! own, so that the form painter, the interpreter and the WebAssembly routines
//! (`cobolt-wasm-rt`, spec WASM-010 R6) edit with the same code. The algorithm
//! is the original's, statement for statement; only the containers changed —
//! fixed-size arrays in place of `Vec` and `String`. `tests/differential.rs`
//! runs the original (kept as a test oracle) against this on a large random
//! sample and requires identical output.
//!
//! Supported symbols:
//!
//! | Symbol | Meaning |
//! |--------|---------|
//! | `9`    | digit position (always shown) |
//! | `Z`    | zero-suppress leading zeros → space |
//! | `*`    | check-protect leading zeros → `*` |
//! | `$`    | currency — fixed (one) or floating (many) |
//! | `+`    | sign — `+`/`-`; fixed or floating |
//! | `-`    | sign — space/`-`; fixed or floating |
//! | `,`    | comma insertion (suppressed in the suppression zone) |
//! | `.`    | decimal point |
//! | `B`    | space insertion |
//! | `0`    | zero insertion |
//! | `/`    | slash insertion |
//! | `CR`   | trailing `CR` when negative, else two spaces |
//! | `DB`   | trailing `DB` when negative, else two spaces |

#![no_std]
#![forbid(unsafe_code)]

/// The longest picture (in expanded symbols) this engine edits.
pub const MAX_SYMS: usize = 256;
/// The longest edited result.
pub const MAX_OUT: usize = 512;
/// The most digit positions a picture may spell.
const MAX_DIGITS: usize = 128;

/// One expanded picture symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sym {
    Nine,
    Z,
    Star,
    Dollar,
    Plus,
    Minus,
    Comma,
    Point,
    Blank,
    InsZero,
    Slash,
    Cr,
    Db,
}

struct Syms {
    s: [Sym; MAX_SYMS],
    n: usize,
}

impl Syms {
    fn as_slice(&self) -> &[Sym] {
        &self.s[..self.n]
    }

    fn push(&mut self, sym: Sym) {
        if self.n < MAX_SYMS {
            self.s[self.n] = sym;
            self.n += 1;
        }
    }
}

/// Expand a raw template (`"ZZ9(3)V99"`, `"$$,$$9.99"`, `"9(6).99CR"`) into a
/// flat symbol list, resolving `(n)` repeat counts and the two-letter `CR`/`DB`.
///
/// Under `decimal_comma`, the roles of `.` and `,` swap: `,` is the decimal
/// point (`Sym::Point`) and `.` is grouping insertion (`Sym::Comma`).
fn expand(template: &[u8], decimal_comma: bool) -> Syms {
    let (point_ch, group_ch) = if decimal_comma { (b',', b'.') } else { (b'.', b',') };
    let mut out = Syms { s: [Sym::Nine; MAX_SYMS], n: 0 };
    let up = |i: usize| -> Option<u8> { template.get(i).map(|c| c.to_ascii_uppercase()) };
    let mut i = 0;
    while i < template.len() {
        let c = template[i].to_ascii_uppercase();
        // CR / DB are two-letter trailing symbols.
        if c == b'C' && up(i + 1) == Some(b'R') {
            out.push(Sym::Cr);
            i += 2;
            continue;
        }
        if c == b'D' && up(i + 1) == Some(b'B') {
            out.push(Sym::Db);
            i += 2;
            continue;
        }
        let sym = match c {
            b'9' => Some(Sym::Nine),
            b'Z' => Some(Sym::Z),
            b'*' => Some(Sym::Star),
            b'$' => Some(Sym::Dollar),
            b'+' => Some(Sym::Plus),
            b'-' => Some(Sym::Minus),
            b'B' => Some(Sym::Blank),
            b'0' => Some(Sym::InsZero),
            b'/' => Some(Sym::Slash),
            b'V' => Some(Sym::Point), // implied point acts as the int/frac split
            c if c == point_ch => Some(Sym::Point),
            c if c == group_ch => Some(Sym::Comma),
            _ => None,
        };
        i += 1;
        let Some(sym) = sym else { continue };
        // Optional repeat count: `Z(3)`, `9(5)`.
        if up(i) == Some(b'(') {
            let mut j = i + 1;
            let mut n = 0usize;
            while j < template.len() && template[j].is_ascii_digit() {
                n = n.saturating_mul(10).saturating_add((template[j] - b'0') as usize);
                j += 1;
            }
            if up(j) == Some(b')') {
                for _ in 0..n.max(1) {
                    out.push(sym);
                }
                i = j + 1;
                continue;
            }
        }
        out.push(sym);
    }
    out
}

/// Recover the numeric value a numeric-**edited** item's characters spell out.
///
/// This is COBOL-85 *de-editing* (6.18.4 GR4b): moving an edited item to a
/// numeric one transfers the value, not the characters. Digits are collected,
/// the decimal point fixes the scale, and `CR`, `DB` or a `-` anywhere in the
/// item makes it negative; currency, grouping, `*` protection, `/` and `B`
/// insertions and blanks carry no value and are dropped. `"$ 123.45CR"` under
/// `PIC $(4)9.99CR` therefore yields `-123.45`.
///
/// Returns `(mantissa, decimals)`.
pub fn deedit(edited: &[u8], decimal_comma: bool) -> (i128, u8) {
    let dec_char = if decimal_comma { b',' } else { b'.' };
    let mut negative = false;
    for (i, c) in edited.iter().enumerate() {
        let u = c.to_ascii_uppercase();
        if u == b'-' {
            negative = true;
        }
        if (u == b'C' && edited.get(i + 1).map(|n| n.to_ascii_uppercase()) == Some(b'R'))
            || (u == b'D' && edited.get(i + 1).map(|n| n.to_ascii_uppercase()) == Some(b'B'))
        {
            negative = true;
        }
    }
    let mut mantissa: i128 = 0;
    let mut decimals: u8 = 0;
    let mut after_point = false;
    for &c in edited {
        if c == dec_char {
            after_point = true;
            continue;
        }
        if c.is_ascii_digit() {
            mantissa = mantissa.saturating_mul(10).saturating_add((c - b'0') as i128);
            if after_point {
                decimals = decimals.saturating_add(1);
            }
        }
    }
    (if negative { -mantissa } else { mantissa }, decimals)
}

/// How many **trailing `P`** scaling positions an edited picture carries.
///
/// `P` marks a digit position the item does not store: `PIC ZZZPP` holds three
/// digits standing for hundreds. Leading `P`s (`PPZZ`, a purely fractional
/// item) are not handled here — no CCVS85 member uses one on an edited item,
/// and guessing at the scale would be worse than leaving it alone.
pub fn trailing_scale(template: &[u8]) -> u32 {
    let mut count = 0u32;
    // Walk back over `P`s and the repeat counts that may follow a symbol, and
    // stop at the first character that occupies a stored position.
    for &c in template.iter().rev() {
        match c.to_ascii_uppercase() {
            b'P' => count += 1,
            b'V' | b')' | b'(' | b'0'..=b'9' => {}
            _ => break,
        }
    }
    // `P(3)` — a repeat count, which the loop above counted as a single `P`.
    if count > 0 {
        // The last "P(" and the first ")" after it.
        let mut open = None;
        let mut k = 0;
        while k + 1 < template.len() {
            if template[k].to_ascii_uppercase() == b'P' && template[k + 1] == b'(' {
                open = Some(k);
            }
            k += 1;
        }
        if let Some(open) = open {
            if let Some(rel) = template[open..].iter().position(|c| *c == b')') {
                let close = open + rel;
                let digits = &template[open + 2..close];
                if !digits.is_empty() && digits.iter().all(|c| c.is_ascii_digit()) {
                    let mut n: u32 = 0;
                    for d in digits {
                        n = n.saturating_mul(10).saturating_add((d - b'0') as u32);
                    }
                    return n;
                }
            }
        }
    }
    count
}

/// Number of integer and fractional **digit positions** the picture represents.
pub fn digit_counts(template: &[u8], decimal_comma: bool) -> (usize, usize) {
    let syms = expand(template, decimal_comma);
    counts(syms.as_slice())
}

fn counts(syms: &[Sym]) -> (usize, usize) {
    let point = syms.iter().position(|s| *s == Sym::Point);
    let (int_part, frac_part): (&[Sym], &[Sym]) = match point {
        Some(p) => (&syms[..p], &syms[p + 1..]),
        None => (syms, &[]),
    };
    let float_dollar = syms.iter().filter(|s| **s == Sym::Dollar).count() > 1;
    let float_plus = syms.iter().filter(|s| **s == Sym::Plus).count() > 1;
    let float_minus = syms.iter().filter(|s| **s == Sym::Minus).count() > 1;

    let mut int_digits = 0usize;
    for s in int_part {
        match s {
            Sym::Nine | Sym::Z | Sym::Star => int_digits += 1,
            Sym::Dollar if float_dollar => int_digits += 1,
            Sym::Plus if float_plus => int_digits += 1,
            Sym::Minus if float_minus => int_digits += 1,
            _ => {}
        }
    }
    // A floating run reserves one leading position for the symbol itself.
    let anchor = (float_dollar as usize) + (float_plus as usize) + (float_minus as usize);
    let int_digits = int_digits.saturating_sub(anchor);

    // A floating symbol after the decimal point is a digit position too —
    // `PIC ++++.++` has two fractional digits.
    let frac_digits = frac_part
        .iter()
        .filter(|s| match s {
            Sym::Nine | Sym::Z | Sym::Star => true,
            Sym::Dollar => float_dollar,
            Sym::Plus => float_plus,
            Sym::Minus => float_minus,
            _ => false,
        })
        .count();
    (int_digits, frac_digits)
}

/// Total output width (characters) of the edited field.
pub fn edited_width(template: &[u8], decimal_comma: bool) -> usize {
    expand(template, decimal_comma)
        .as_slice()
        .iter()
        .map(|s| if matches!(s, Sym::Cr | Sym::Db) { 2 } else { 1 })
        .sum()
}

/// A growing output with a fixed capacity.
struct Out<'a> {
    buf: &'a mut [u8],
    n: usize,
}

impl Out<'_> {
    fn push(&mut self, c: u8) {
        if self.n < self.buf.len() {
            self.buf[self.n] = c;
            self.n += 1;
        }
    }
    fn push_str(&mut self, s: &[u8]) {
        for &c in s {
            self.push(c);
        }
    }
}

/// Rescale an unsigned mantissa from `from_scale` decimals to `to_scale`
/// decimals, truncating (toward zero) any excess fractional digits.
fn rescale(mantissa: u128, from_scale: i32, to_scale: i32) -> u128 {
    if to_scale >= from_scale {
        match 10u128.checked_pow((to_scale - from_scale) as u32) {
            Some(p) => mantissa.wrapping_mul(p),
            None => 0,
        }
    } else {
        match 10u128.checked_pow((from_scale - to_scale) as u32) {
            Some(p) => mantissa / p,
            None => 0,
        }
    }
}

/// Format `mantissa × 10^-decimals` against the numeric-edited `template`,
/// into `out`; returns the number of bytes written.
///
/// `currency` is the character a currency position prints — `'$'` unless
/// `SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal` named another. The template
/// always spells such a position as `$` whatever the program calls it, so every
/// width and digit-count rule is written once and only the emission substitutes.
pub fn format_edited(
    template: &[u8],
    mantissa: i128,
    decimals: u8,
    decimal_comma: bool,
    currency: u8,
    out: &mut [u8],
) -> usize {
    // Output characters for the decimal point and grouping insertion.
    let dec_char = if decimal_comma { b',' } else { b'.' };
    let grp_char = if decimal_comma { b'.' } else { b',' };
    let syms_s = expand(template, decimal_comma);
    let syms = syms_s.as_slice();
    let (int_digits, frac_digits) = counts(syms);
    // Trailing `P`s are digit positions the item spans but does not store, so
    // its digits stand for tens/hundreds/… and the value has to be brought down
    // to them before it is edited. `PIC ZZZPP` receiving 900 shows `  9`, not
    // `900`. (`expand` drops `P` from the symbol list, which is right for the
    // width — those positions print nothing.)
    let trailing_p = trailing_scale(template);
    let mantissa = match 10i128.checked_pow(trailing_p) {
        Some(d) if trailing_p > 0 => mantissa / d,
        _ => mantissa,
    };
    let negative = mantissa < 0;

    // Rescale the source value (truncating) to the picture's fractional width.
    let scaled = rescale(mantissa.unsigned_abs(), decimals as i32, frac_digits as i32);
    // The digit string of `scaled`.
    let mut all = [0u8; 40];
    let mut al = all.len();
    {
        let mut m = scaled;
        loop {
            al -= 1;
            all[al] = b'0' + (m % 10) as u8;
            m /= 10;
            if m == 0 {
                break;
            }
        }
    }
    let all = &all[al..];
    // Split into integer / fractional digit strings of the required widths.
    let total = (int_digits + frac_digits).min(MAX_DIGITS);
    let mut padded = [b'0'; MAX_DIGITS];
    if all.len() < total {
        padded[total - all.len()..total].copy_from_slice(all);
    } else {
        // truncate high-order on overflow
        padded[..total].copy_from_slice(&all[all.len() - total..]);
    }
    let int_digits = int_digits.min(total);
    let int_src: &[u8] = &padded[..int_digits];
    let frac_src: &[u8] = &padded[int_digits..total];

    let mut o = Out { buf: out, n: 0 };

    let float_dollar = syms.iter().filter(|s| **s == Sym::Dollar).count() > 1;
    let float_plus = syms.iter().filter(|s| **s == Sym::Plus).count() > 1;
    let float_minus = syms.iter().filter(|s| **s == Sym::Minus).count() > 1;
    let floating = float_dollar || float_plus || float_minus;
    // "If all numeric character positions are represented by the floating
    // insertion symbol and the value is zero, the entire item is spaces"
    // (COBOL-85 floating insertion editing). Zero suppression that covers
    // **every** digit position blanks the whole item when the value is zero —
    // the decimal point included. A single `9` anywhere claims a position that
    // must always print, so the rule no longer applies.
    if mantissa == 0
        && (floating || syms.iter().any(|s| *s == Sym::Z))
        && !syms.iter().any(|s| *s == Sym::Nine)
    {
        for _ in 0..edited_width_of(syms) {
            o.push(b' ');
        }
        return o.n;
    }
    // The same rule for asterisk (check) protection, with `*` in place of the
    // blank: a zero value in a picture whose digit positions are all `*` fills
    // the item with asterisks, **including** the fractional positions and the
    // grouping commas, leaving only the decimal point itself.
    if mantissa == 0 && syms.iter().any(|s| *s == Sym::Star) && !syms.iter().any(|s| *s == Sym::Nine) {
        for s in syms {
            match s {
                Sym::Point => o.push(dec_char),
                Sym::Slash => o.push(b'/'),
                Sym::Blank => o.push(b' '),
                // `CR` and `DB` occupy **two** character positions.
                Sym::Cr | Sym::Db => o.push_str(b"**"),
                _ => o.push(b'*'),
            }
        }
        return o.n;
    }
    let float_char = if float_dollar {
        currency
    } else if float_plus {
        if negative {
            b'-'
        } else {
            b'+'
        }
    } else {
        // floating minus
        if negative {
            b'-'
        } else {
            b' '
        }
    };

    let point = syms.iter().position(|s| *s == Sym::Point);
    let int_syms: &[Sym] = match point {
        Some(p) => &syms[..p],
        None => syms,
    };

    // ── Integer region ─────────────────────────────────────────────────────────
    // digit-bearing token positions in the integer region.
    let is_int_digit_tok = |s: Sym| {
        matches!(s, Sym::Nine | Sym::Z | Sym::Star)
            || (float_dollar && s == Sym::Dollar)
            || (float_plus && s == Sym::Plus)
            || (float_minus && s == Sym::Minus)
    };
    let dt_count = int_syms.iter().filter(|s| is_int_digit_tok(**s)).count();
    let anchor = floating as usize; // leading reserve slot for the float char

    // Map digits onto the rightmost (dt_count - anchor) digit tokens.
    // digit_for_tok[k] = the digit byte, or 0 for the reserved anchor slot.
    let mut digit_for_tok = [0u8; MAX_SYMS + 4];
    let mut dft_len = 0usize;
    for _ in 0..anchor {
        digit_for_tok[dft_len] = 0;
        dft_len += 1;
    }
    for &d in int_src {
        if dft_len < digit_for_tok.len() {
            digit_for_tok[dft_len] = d;
            dft_len += 1;
        }
    }
    let digit_at = |idx: usize| -> Option<u8> {
        if idx < dft_len && digit_for_tok[idx] != 0 {
            Some(digit_for_tok[idx])
        } else {
            None
        }
    };

    // Suppression stops at the first `9` token or the first significant digit.
    let mut supp_end = dt_count; // dt index where digits start showing
    {
        let mut idx = 0usize;
        for &s in int_syms.iter().filter(|s| is_int_digit_tok(**s)) {
            let is_sig = matches!(digit_at(idx), Some(d) if d != b'0');
            if s == Sym::Nine || is_sig {
                supp_end = idx;
                break;
            }
            idx += 1;
        }
    }
    // Position of the floating char: the **character** immediately left of the
    // first digit that shows — counted in template symbols, not in digit
    // tokens, because a grouping comma can sit between them.
    let float_pos = if floating {
        let mut dt = 0usize;
        let mut first_shown = None;
        for (i, &s) in int_syms.iter().enumerate() {
            if is_int_digit_tok(s) {
                if dt >= supp_end {
                    first_shown = Some(i);
                    break;
                }
                dt += 1;
            }
        }
        match first_shown {
            Some(i) => Some(i.saturating_sub(1)),
            // Every integer position is suppressed but the item still prints
            // (its fraction has `9`s): the symbol goes in the **last** integer
            // position, so `PIC $$$$$.99` holding zero reads `    $.00`.
            None => int_syms.iter().rposition(|s| is_int_digit_tok(*s)),
        }
    } else {
        None
    };

    let mut seen_sig = false; // have we emitted a real digit yet (for commas)?
    let mut dt_idx = 0usize;
    let has_star = int_syms.iter().any(|x| *x == Sym::Star);
    for (si, &s) in int_syms.iter().enumerate() {
        if float_pos == Some(si) {
            o.push(float_char);
            if is_int_digit_tok(s) {
                dt_idx += 1;
            }
            continue;
        }
        if is_int_digit_tok(s) {
            let suppressed = dt_idx < supp_end;
            if suppressed {
                o.push(if s == Sym::Star { b'*' } else { b' ' });
            } else {
                let d = digit_at(dt_idx).unwrap_or(b'0');
                o.push(d);
                seen_sig = true;
            }
            dt_idx += 1;
        } else {
            match s {
                Sym::Comma => {
                    // Grouping insertion: shown once past the suppression zone.
                    if seen_sig {
                        o.push(grp_char);
                    } else {
                        o.push(if has_star { b'*' } else { b' ' });
                    }
                }
                // A simple-insertion character inside the **suppression zone**
                // is replaced by the suppression character, exactly as a
                // grouping comma is. Past the zone it prints itself.
                Sym::Blank | Sym::InsZero | Sym::Slash if !seen_sig && supp_end > 0 => {
                    o.push(if has_star { b'*' } else { b' ' });
                }
                Sym::Blank => o.push(b' '),
                Sym::InsZero => o.push(b'0'),
                Sym::Slash => o.push(b'/'),
                Sym::Dollar => o.push(currency), // fixed currency
                Sym::Plus => o.push(if negative { b'-' } else { b'+' }), // fixed leading/trailing +
                Sym::Minus => o.push(if negative { b'-' } else { b' ' }), // fixed sign
                _ => {}
            }
        }
    }

    // ── Decimal point + fractional region ──────────────────────────────────────
    if let Some(p) = point {
        // Emit the decimal-point character (',' under DECIMAL-POINT IS COMMA).
        o.push(dec_char);
        let frac_syms = &syms[p + 1..];
        let mut fi = 0usize;
        let frac_at = |fi: usize| -> u8 { frac_src.get(fi).copied().unwrap_or(b'0') };
        for &s in frac_syms {
            match s {
                Sym::Nine | Sym::Z | Sym::Star => {
                    o.push(frac_at(fi));
                    fi += 1;
                }
                // In a **floating** picture the symbols after the decimal point
                // are digit positions, not another sign: zero suppression never
                // crosses the point.
                Sym::Dollar if float_dollar => {
                    o.push(frac_at(fi));
                    fi += 1;
                }
                Sym::Plus if float_plus => {
                    o.push(frac_at(fi));
                    fi += 1;
                }
                Sym::Minus if float_minus => {
                    o.push(frac_at(fi));
                    fi += 1;
                }
                Sym::Comma => o.push(grp_char),
                Sym::Blank => o.push(b' '),
                Sym::InsZero => o.push(b'0'),
                Sym::Slash => o.push(b'/'),
                Sym::Cr => o.push_str(if negative { b"CR" } else { b"  " }),
                Sym::Db => o.push_str(if negative { b"DB" } else { b"  " }),
                Sym::Plus => o.push(if negative { b'-' } else { b'+' }),
                Sym::Minus => o.push(if negative { b'-' } else { b' ' }),
                _ => {}
            }
        }
    }

    // Trailing CR/DB symbols when there is no point: scan the whole template.
    if point.is_none() {
        for &s in syms {
            match s {
                Sym::Cr => o.push_str(if negative { b"CR" } else { b"  " }),
                Sym::Db => o.push_str(if negative { b"DB" } else { b"  " }),
                _ => {}
            }
        }
    }

    o.n
}

fn edited_width_of(syms: &[Sym]) -> usize {
    syms.iter().map(|s| if matches!(s, Sym::Cr | Sym::Db) { 2 } else { 1 }).sum()
}
