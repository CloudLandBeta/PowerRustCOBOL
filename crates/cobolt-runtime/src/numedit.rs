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
    let (point_ch, group_ch) = if decimal_comma { (',', '.') } else { ('.', ',') };
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

    let frac_digits = frac_part
        .iter()
        .filter(|s| matches!(s, Sym::Nine | Sym::Z | Sym::Star))
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
pub fn format_edited(template: &str, mantissa: i128, decimals: u8, decimal_comma: bool) -> String {
    // Output characters for the decimal point and grouping insertion.
    let dec_char = if decimal_comma { ',' } else { '.' };
    let grp_char = if decimal_comma { '.' } else { ',' };
    let syms = expand(template, decimal_comma);
    let (int_digits, frac_digits) = counts(&syms);
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
    let float_char = if float_dollar {
        '$'
    } else if float_plus {
        if negative { '-' } else { '+' }
    } else {
        // floating minus
        if negative { '-' } else { ' ' }
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
        let int_tok_syms: Vec<Sym> = int_syms.iter().copied().filter(|s| is_int_digit_tok(*s)).collect();
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
    // Position of the floating char: immediately left of the first shown digit.
    let float_pos = if floating { supp_end.saturating_sub(1) } else { usize::MAX };

    let mut out = String::new();
    let mut seen_sig = false; // have we emitted a real digit yet (for commas)?
    let mut dt_idx = 0usize;
    for &s in int_syms {
        if is_int_digit_tok(s) {
            let suppressed = dt_idx < supp_end;
            if floating && dt_idx == float_pos {
                out.push(float_char);
            } else if suppressed {
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
                        out.push(if int_syms.iter().any(|x| *x == Sym::Star) { '*' } else { ' ' });
                    }
                }
                Sym::Blank => out.push(' '),
                Sym::InsZero => out.push('0'),
                Sym::Slash => out.push('/'),
                Sym::Dollar => out.push('$'),   // fixed currency
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
        format_edited(template, mantissa, decimals, false)
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
        assert_eq!(format_edited("$ZZ.ZZ9,99-", 123450, 2, true), "$ 1.234,50 ");
        assert_eq!(format_edited("$ZZ.ZZ9,99-", -123450, 2, true), "$ 1.234,50-");
        // PIC 9.999 in comma mode = 4 integer digits with period grouping.
        assert_eq!(digit_counts("9.999", true), (4, 0));
        assert_eq!(format_edited("9.999", 1234, 0, true), "1.234");
        // 999,99 → comma decimal point.
        assert_eq!(format_edited("999,99", 12345, 2, true), "123,45");
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
