// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Numeric-edited PICTURE engine.
//!
//! Applies COBOL editing symbols when a numeric value is moved into a
//! numeric-edited field (`PIC ZZZ,ZZ9.99`, `$$$,$$9.99`, `----9.99`, `9(6).99CR`…).
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

/// Expand a raw template (`"ZZ9(3)V99"`, `"$$,$$9.99"`, `"9(6).99CR"`) into a flat
/// symbol list, resolving `(n)` repeat counts and the two-letter `CR`/`DB`.
///
/// Under `decimal_comma`, the roles of `.` and `,` swap: `,` is the decimal point
/// (`Sym::Point`) and `.` is grouping insertion (`Sym::Comma`).
fn expand(template: &str, decimal_comma: bool) -> Vec<Sym> {
    let (point_ch, group_ch) = if decimal_comma {
        (',', '.')
    } else {
        ('.', ',')
    };
    let chars: Vec<char> = template.to_ascii_uppercase().chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // CR / DB are two-letter trailing symbols.
        if c == 'C' && chars.get(i + 1) == Some(&'R') {
            out.push(Sym::Cr);
            i += 2;
            continue;
        }
        if c == 'D' && chars.get(i + 1) == Some(&'B') {
            out.push(Sym::Db);
            i += 2;
            continue;
        }
        let sym = match c {
            '9' => Some(Sym::Nine),
            'Z' => Some(Sym::Z),
            '*' => Some(Sym::Star),
            '$' => Some(Sym::Dollar),
            '+' => Some(Sym::Plus),
            '-' => Some(Sym::Minus),
            'B' => Some(Sym::Blank),
            '0' => Some(Sym::InsZero),
            '/' => Some(Sym::Slash),
            'V' => Some(Sym::Point), // implied point acts as the int/frac split
            c if c == point_ch => Some(Sym::Point),
            c if c == group_ch => Some(Sym::Comma),
            _ => None,
        };
        i += 1;
        let Some(sym) = sym else { continue };
        // Optional repeat count: `Z(3)`, `9(5)`.
        if chars.get(i) == Some(&'(') {
            let mut j = i + 1;
            let mut n = 0usize;
            while j < chars.len() && chars[j].is_ascii_digit() {
                n = n * 10 + (chars[j] as usize - '0' as usize);
                j += 1;
            }
            if chars.get(j) == Some(&')') {
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
pub fn deedit(edited: &str, decimal_comma: bool) -> (i128, u8) {
    let dec_char = if decimal_comma { ',' } else { '.' };
    let upper = edited.to_ascii_uppercase();
    let negative = upper.contains("CR") || upper.contains("DB") || upper.contains('-');
    let mut mantissa: i128 = 0;
    let mut decimals: u8 = 0;
    let mut after_point = false;
    for c in upper.chars() {
        if c == dec_char {
            after_point = true;
            continue;
        }
        if let Some(d) = c.to_digit(10) {
            mantissa = mantissa.saturating_mul(10).saturating_add(d as i128);
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
fn trailing_scale(template: &str) -> u32 {
    let up = template.to_ascii_uppercase();
    let mut count = 0u32;
    // Walk back over `P`s and the repeat counts that may follow a symbol, and
    // stop at the first character that occupies a stored position.
    for c in up.chars().rev() {
        match c {
            'P' => count += 1,
            'V' | ')' | '(' | '0'..='9' => {}
            _ => break,
        }
    }
    // `P(3)` — a repeat count, which the loop above counted as a single `P`.
    if count > 0 {
        if let Some(open) = up.rfind("P(") {
            if let Some(close) = up[open..].find(')') {
                if let Ok(n) = up[open + 2..open + close].parse::<u32>() {
                    return n;
                }
            }
        }
    }
    count
}

/// Number of integer and fractional **digit positions** the picture represents.
pub fn digit_counts(template: &str, decimal_comma: bool) -> (usize, usize) {
    let syms = expand(template, decimal_comma);
    counts(&syms)
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
    // `PIC ++++.++` has two fractional digits. Counting only `9`/`Z`/`*` there
    // made the picture look integral, so the value was rescaled to zero
    // decimals and `12.34` printed as `12.00`.
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
pub fn edited_width(template: &str, decimal_comma: bool) -> usize {
    expand(template, decimal_comma)
        .iter()
        .map(|s| if matches!(s, Sym::Cr | Sym::Db) { 2 } else { 1 })
        .sum()
}

/// Format `mantissa × 10^-decimals` against the numeric-edited `template`.
///
/// `currency` is the character a currency position prints — `'$'` unless
/// `SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal` named another. The template
/// always spells such a position as `$` whatever the program calls it, so every
/// width and digit-count rule is written once and only the emission substitutes.
pub fn format_edited(
    template: &str,
    mantissa: i128,
    decimals: u8,
    decimal_comma: bool,
    currency: char,
) -> String {
    // Output characters for the decimal point and grouping insertion.
    let dec_char = if decimal_comma { ',' } else { '.' };
    let grp_char = if decimal_comma { '.' } else { ',' };
    let syms = expand(template, decimal_comma);
    let (int_digits, frac_digits) = counts(&syms);
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
    let all = scaled.to_string();
    // Split into integer / fractional digit strings of the required widths.
    let total = int_digits + frac_digits;
    let padded = if all.len() < total {
        format!("{}{}", "0".repeat(total - all.len()), all)
    } else {
        all[all.len() - total..].to_string() // truncate high-order on overflow
    };
    let int_src: Vec<u8> = padded[..int_digits].bytes().collect();
    let frac_src: Vec<u8> = padded[int_digits..].bytes().collect();

    let float_dollar = syms.iter().filter(|s| **s == Sym::Dollar).count() > 1;
    let float_plus = syms.iter().filter(|s| **s == Sym::Plus).count() > 1;
    let float_minus = syms.iter().filter(|s| **s == Sym::Minus).count() > 1;
    let floating = float_dollar || float_plus || float_minus;
    // "If all numeric character positions are represented by the floating
    // insertion symbol and the value is zero, the entire item is spaces"
    // (COBOL-85 floating insertion editing). `PIC ++++` holding zero is six
    // blanks, not `   +` — and `PIC ++.++` is blanks, not `  +.++`. The test is
    // that no `9` claims a position of its own: every digit position belongs to
    // the floating run.
    // Zero suppression that covers **every** digit position blanks the whole
    // item when the value is zero — the decimal point included. `PIC ZZZ.ZZ`
    // holding zero reads as spaces, not `   .00`; the same is true of a
    // floating `PIC ++++` or `PIC ++++.++`. A single `9` anywhere claims a
    // position that must always print, so the rule no longer applies.
    if mantissa == 0
        && (floating || syms.iter().any(|s| *s == Sym::Z))
        && !syms.iter().any(|s| *s == Sym::Nine)
    {
        return " ".repeat(edited_width(template, decimal_comma));
    }
    // The same rule for asterisk (check) protection, with `*` in place of the
    // blank: a zero value in a picture whose digit positions are all `*` fills
    // the item with asterisks, **including** the fractional positions and the
    // grouping commas, leaving only the decimal point itself. `PIC *,***.**`
    // holding zero reads `*****.**`, not `*****.00`.
    if mantissa == 0
        && syms.iter().any(|s| *s == Sym::Star)
        && !syms.iter().any(|s| *s == Sym::Nine)
    {
        return syms
            .iter()
            .map(|s| match s {
                Sym::Point => dec_char.to_string(),
                Sym::Slash => "/".to_string(),
                Sym::Blank => " ".to_string(),
                // `CR` and `DB` occupy **two** character positions, which is
                // what `edited_width` counts them as. Emitting one asterisk
                // apiece left `PIC $**.**CR` holding zero a character short of
                // its own width, and whatever padded it to width put a space
                // there: `***.***_` instead of `***.****` (NC175A
                // SUB-TEST-F2-28-5, -30-5, -32-5).
                Sym::Cr | Sym::Db => "**".to_string(),
                _ => "*".to_string(),
            })
            .collect();
    }
    let float_char = if float_dollar {
        currency
    } else if float_plus {
        if negative {
            '-'
        } else {
            '+'
        }
    } else {
        // floating minus
        if negative {
            '-'
        } else {
            ' '
        }
    };

    let point = syms.iter().position(|s| *s == Sym::Point);
    let int_syms: &[Sym] = match point {
        Some(p) => &syms[..p],
        None => &syms,
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
    // digit_for_tok[k] = Some(digit byte) or None (the reserved anchor slot).
    let mut digit_for_tok: Vec<Option<u8>> = Vec::with_capacity(dt_count);
    for _ in 0..anchor {
        digit_for_tok.push(None);
    }
    for &d in &int_src {
        digit_for_tok.push(Some(d));
    }
    // (digit_for_tok now has length dt_count == anchor + int_digits)

    // Suppression stops at the first `9` token or the first significant digit.
    let mut supp_end = dt_count; // dt index where digits start showing
    {
        let mut k = 0usize; // digit-token index
        let int_tok_syms: Vec<Sym> = int_syms
            .iter()
            .copied()
            .filter(|s| is_int_digit_tok(*s))
            .collect();
        for (idx, &s) in int_tok_syms.iter().enumerate() {
            let is_sig = matches!(digit_for_tok.get(idx), Some(Some(d)) if *d != b'0');
            if s == Sym::Nine || is_sig {
                supp_end = idx;
                break;
            }
            k = idx;
        }
        let _ = k;
    }
    // Position of the floating char: the **character** immediately left of the
    // first digit that shows — counted in template symbols, not in digit
    // tokens, because a grouping comma can sit between them. In `PIC --,---.--`
    // holding -123 the sign belongs on the comma's position (`  -123.00`);
    // placing it on the nearest digit token instead left the comma's space
    // between the sign and the number (` - 123.00`).
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

    let mut out = String::new();
    let mut seen_sig = false; // have we emitted a real digit yet (for commas)?
    let mut dt_idx = 0usize;
    for (si, &s) in int_syms.iter().enumerate() {
        if float_pos == Some(si) {
            out.push(float_char);
            if is_int_digit_tok(s) {
                dt_idx += 1;
            }
            continue;
        }
        if is_int_digit_tok(s) {
            let suppressed = dt_idx < supp_end;
            if suppressed {
                out.push(if s == Sym::Star { '*' } else { ' ' });
            } else {
                let d = digit_for_tok[dt_idx].unwrap_or(b'0');
                out.push(d as char);
                seen_sig = true;
            }
            dt_idx += 1;
        } else {
            match s {
                Sym::Comma => {
                    // Grouping insertion: shown once past the suppression zone.
                    if seen_sig {
                        out.push(grp_char);
                    } else {
                        out.push(if int_syms.iter().any(|x| *x == Sym::Star) {
                            '*'
                        } else {
                            ' '
                        });
                    }
                }
                // A simple-insertion character inside the **suppression zone**
                // is replaced by the suppression character, exactly as a
                // grouping comma is: `PIC -*B*99` holding -42 reads `-***42`,
                // not `-* *42`. Past the zone it prints itself.
                Sym::Blank | Sym::InsZero | Sym::Slash if !seen_sig && supp_end > 0 => {
                    out.push(if int_syms.iter().any(|x| *x == Sym::Star) {
                        '*'
                    } else {
                        ' '
                    });
                }
                Sym::Blank => out.push(' '),
                Sym::InsZero => out.push('0'),
                Sym::Slash => out.push('/'),
                Sym::Dollar => out.push(currency), // fixed currency
                Sym::Plus => out.push(if negative { '-' } else { '+' }), // fixed leading/trailing +
                Sym::Minus => out.push(if negative { '-' } else { ' ' }), // fixed sign
                _ => {}
            }
        }
    }

    // ── Decimal point + fractional region ──────────────────────────────────────
    if point.is_some() {
        // Emit the decimal-point character (',' under DECIMAL-POINT IS COMMA).
        out.push(dec_char);
        let frac_syms = &syms[point.unwrap() + 1..];
        let mut fi = 0usize;
        for &s in frac_syms {
            match s {
                Sym::Nine | Sym::Z | Sym::Star => {
                    out.push(*frac_src.get(fi).unwrap_or(&b'0') as char);
                    fi += 1;
                }
                // In a **floating** picture the symbols after the decimal point
                // are digit positions, not another sign: zero suppression never
                // crosses the point, so `PIC ++++.++` holding 12 reads
                // `  +12.00`, not `  +12.++`. Only a picture whose sign symbol
                // appears once (a fixed trailing sign) prints the sign here.
                Sym::Dollar if float_dollar => {
                    out.push(*frac_src.get(fi).unwrap_or(&b'0') as char);
                    fi += 1;
                }
                Sym::Plus if float_plus => {
                    out.push(*frac_src.get(fi).unwrap_or(&b'0') as char);
                    fi += 1;
                }
                Sym::Minus if float_minus => {
                    out.push(*frac_src.get(fi).unwrap_or(&b'0') as char);
                    fi += 1;
                }
                Sym::Comma => out.push(grp_char),
                Sym::Blank => out.push(' '),
                Sym::InsZero => out.push('0'),
                Sym::Slash => out.push('/'),
                Sym::Cr => out.push_str(if negative { "CR" } else { "  " }),
                Sym::Db => out.push_str(if negative { "DB" } else { "  " }),
                Sym::Plus => out.push(if negative { '-' } else { '+' }),
                Sym::Minus => out.push(if negative { '-' } else { ' ' }),
                _ => {}
            }
        }
    } else {
        // No point: any trailing CR/DB/sign tokens still apply.
        for &s in &syms {
            match s {
                Sym::Cr if frac_digits == 0 => {} // handled below
                _ => {}
            }
        }
    }

    // Trailing CR/DB or sign symbols that sit after the (possibly absent) point.
    // When there is no Point, scan the whole template tail.
    if point.is_none() {
        for &s in &syms {
            match s {
                Sym::Cr => out.push_str(if negative { "CR" } else { "  " }),
                Sym::Db => out.push_str(if negative { "DB" } else { "  " }),
                _ => {}
            }
        }
    }

    out
}

/// Rescale an unsigned mantissa from `from_scale` decimals to `to_scale` decimals,
/// truncating (toward zero) any excess fractional digits.
fn rescale(mantissa: u128, from_scale: i32, to_scale: i32) -> u128 {
    if to_scale >= from_scale {
        mantissa * 10u128.pow((to_scale - from_scale) as u32)
    } else {
        mantissa / 10u128.pow((from_scale - to_scale) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: edit `value` (given as mantissa+scale) against `template`.
    fn ed(template: &str, mantissa: i128, decimals: u8) -> String {
        format_edited(template, mantissa, decimals, false, '$')
    }

    #[test]
    fn counts_basic() {
        assert_eq!(digit_counts("ZZZ,ZZ9.99", false), (6, 2));
        assert_eq!(digit_counts("$$$,$$9.99", false), (5, 2));
        assert_eq!(digit_counts("9(6).99", false), (6, 2));
        assert_eq!(digit_counts("----9.99", false), (4, 2));
    }

    #[test]
    fn decimal_point_is_comma_swaps_roles() {
        // 1234.50 under comma mode: '.' groups, ',' is the decimal point.
        assert_eq!(format_edited("$ZZ.ZZ9,99-", 123450, 2, true, '$'), "$ 1.234,50 ");
        assert_eq!(
            format_edited("$ZZ.ZZ9,99-", -123450, 2, true, '$'),
            "$ 1.234,50-"
        );
        // PIC 9.999 in comma mode = 4 integer digits with period grouping.
        assert_eq!(digit_counts("9.999", true), (4, 0));
        assert_eq!(format_edited("9.999", 1234, 0, true, '$'), "1.234");
        // 999,99 → comma decimal point.
        assert_eq!(format_edited("999,99", 12345, 2, true, '$'), "123,45");
    }

    /// `SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal` — the template still spells
    /// a currency position `$`; only the emitted character changes. Both the
    /// fixed and the floating forms substitute (NIST NC108M).
    #[test]
    fn a_declared_currency_symbol_replaces_the_dollar() {
        // Fixed currency: one `$` in the picture, three integer positions.
        assert_eq!(format_edited("$ZZ9.99", 12345, 2, false, '<'), "<123.45");
        // Floating currency: the run drifts to the first significant digit.
        assert_eq!(
            format_edited("$(3),$$$.99", 111111, 2, false, '<'),
            " <1,111.11"
        );
        assert_eq!(format_edited("$(3),$$$.99", 0, 0, false, '<'), "      <.00");
        // The default is unchanged.
        assert_eq!(
            format_edited("$(3),$$$.99", 111111, 2, false, '$'),
            " $1,111.11"
        );
    }

    #[test]
    fn zero_suppression_with_comma() {
        // 1234.50 → "  1,234.50"
        assert_eq!(ed("ZZZ,ZZ9.99", 123450, 2), "  1,234.50");
        // 0.00 → "        .00"  (all integer Z suppressed, forced 9 shows 0)
        assert_eq!(ed("ZZZ,ZZ9.99", 0, 2), "      0.00");
    }

    #[test]
    fn check_protection_star() {
        // 12.34 → leading zeros (and the comma in the suppression zone) become '*'.
        assert_eq!(ed("***,**9.99", 1234, 2), "*****12.34");
    }

    /// A zero value under full check protection fills the item's **own width**,
    /// and `CR`/`DB` occupy two character positions of it — which is what
    /// `edited_width` counts them as. One asterisk apiece left the result a
    /// character short and something else padded it with a space:
    /// `PIC $**.**CR` read `***.***_` instead of `***.****`
    /// (NIST CCVS85 NC175A SUB-TEST-F2-28-5, -30-5, -32-5).
    #[test]
    fn check_protection_fills_a_two_character_cr_or_db() {
        for tpl in ["$**.**CR", "$**.**DB"] {
            let got = ed(tpl, 0, 2);
            assert_eq!(got, "***.****", "{tpl}");
            assert_eq!(
                got.chars().count(),
                edited_width(tpl, false),
                "{tpl}: a protected zero must fill the item's declared width"
            );
        }
        // A non-zero value keeps CR/DB doing their own job: printed when the
        // value is negative, two spaces when it is not. Both digit positions
        // are filled here, so there is no leading zero to protect and the `$`
        // prints as itself.
        assert_eq!(ed("$**.**CR", -1234, 2), "$12.34CR");
        assert_eq!(ed("$**.**CR", 1234, 2), "$12.34  ");
        // …and a value that does leave a leading zero protects that position
        // only. The lone `$` is a *fixed* insertion, so it keeps its own
        // position here — it is the all-asterisks rule above, and nothing else,
        // that turns it into a `*` when the value is zero.
        assert_eq!(ed("$**.**CR", -234, 2), "$*2.34CR");
    }

    #[test]
    fn floating_dollar() {
        // 1234.50 → " $1,234.50"
        assert_eq!(ed("$$$,$$9.99", 123450, 2), " $1,234.50");
        // 5.00 → "     $5.00" (10-wide field, '$' floats to the lone digit)
        assert_eq!(ed("$$$,$$9.99", 500, 2), "     $5.00");
    }

    #[test]
    fn fixed_dollar() {
        assert_eq!(ed("$9,999.99", 123450, 2), "$1,234.50");
    }

    #[test]
    fn floating_minus_sign() {
        // -12.30 → "  -12.30"
        assert_eq!(ed("----9.99", -1230, 2), "  -12.30");
        // +12.30 → "   12.30"
        assert_eq!(ed("----9.99", 1230, 2), "   12.30");
    }

    #[test]
    fn cr_db_suffix() {
        // negative → CR shown
        assert_eq!(ed("9(6).99CR", -1230, 2), "000012.30CR");
        // positive → two spaces
        assert_eq!(ed("9(6).99CR", 1230, 2), "000012.30  ");
        assert_eq!(ed("9(6).99DB", -1230, 2), "000012.30DB");
    }

    #[test]
    fn fixed_sign_leading() {
        assert_eq!(ed("+9999.99", 1230, 2), "+0012.30");
        assert_eq!(ed("+9999.99", -1230, 2), "-0012.30");
    }
}
