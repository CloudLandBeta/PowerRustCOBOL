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
//! The engine itself lives in the `cobolt-numedit` crate (`no_std`, no
//! allocation) so that the form painter, the interpreter and the WebAssembly
//! routines edit with the same code (spec WASM-010 R6); this module keeps the
//! `String` API the rest of the workspace uses. Its tests are unchanged and
//! `cobolt-numedit/tests/differential.rs` proves the engine equal to the
//! implementation this file used to hold.

use cobolt_numedit as engine;

/// Recover the numeric value a numeric-**edited** item's characters spell out
/// (COBOL-85 de-editing, 6.18.4 GR4b). Returns `(mantissa, decimals)`.
pub fn deedit(edited: &str, decimal_comma: bool) -> (i128, u8) {
    engine::deedit(edited.as_bytes(), decimal_comma)
}

/// Number of integer and fractional **digit positions** the picture represents.
pub fn digit_counts(template: &str, decimal_comma: bool) -> (usize, usize) {
    engine::digit_counts(template.as_bytes(), decimal_comma)
}

/// Total output width (characters) of the edited field.
pub fn edited_width(template: &str, decimal_comma: bool) -> usize {
    engine::edited_width(template.as_bytes(), decimal_comma)
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
    // The engine works in bytes; a currency sign that is not ASCII travels as a
    // placeholder and is substituted in the result.
    const PLACEHOLDER: u8 = 0x01;
    let cur = if currency.is_ascii() { currency as u8 } else { PLACEHOLDER };
    let mut buf = [0u8; engine::MAX_OUT];
    let n = engine::format_edited(template.as_bytes(), mantissa, decimals, decimal_comma, cur, &mut buf);
    let text = String::from_utf8_lossy(&buf[..n]).into_owned();
    if currency.is_ascii() {
        text
    } else {
        text.replace('\u{1}', &currency.to_string())
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
