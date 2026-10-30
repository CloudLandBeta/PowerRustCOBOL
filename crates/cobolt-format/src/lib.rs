// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Locale-aware formatting of a decimal value (spec 109 §5.2), `no_std`.
//!
//! The value comes in as an exact decimal — `mantissa` and `scale`, the shape
//! of `cobolt-decimal`'s `CobolNumeric` — and goes out as text. Nothing here
//! reads the operating system: the locales are the closed set of [`LOCALES`],
//! their separators and patterns are written below, and the tests fix their
//! output, so every host that compiles this file prints the same characters
//! (R9, R23).
//!
//! **Formats** (names are case-insensitive): `number`, `currency`, `compact`,
//! `currency-short`, `percent`.
//!
//! | Format | Rule |
//! |---|---|
//! | `number` | the value's own scale, digits grouped |
//! | `currency` | the currency's decimals (2; 0 for JPY), symbol placed by the locale; needs a currency |
//! | `compact` | thousands/millions/billions/trillions with one decimal at most (`1,2 mi`) |
//! | `currency-short` | `compact` with the currency symbol; needs a currency |
//! | `percent` | the value is a **ratio**: 0.425 is 42,5 %; trailing zeros of the result are dropped |
//!
//! **Rounding** is half away from zero. A negative value carries a leading
//! minus sign. Zero is formatted like any other value.

#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};

/// What can go wrong; each case names what was refused (R8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    UnknownFormat(String),
    UnknownLocale(String),
    /// `currency` and `currency-short` need a currency code, from the format
    /// call or from the project's default.
    NoCurrency,
    InvalidCurrency(String),
}

impl core::fmt::Display for FormatError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FormatError::UnknownFormat(n) => write!(
                f,
                "unknown format '{n}': the formats are {}",
                FORMATS.join(", ")
            ),
            FormatError::UnknownLocale(n) => write!(
                f,
                "unknown locale '{n}': the locales are {}",
                LOCALES.iter().map(|l| l.tag).collect::<alloc::vec::Vec<_>>().join(", ")
            ),
            FormatError::NoCurrency => {
                write!(f, "this format needs a currency: name one, or set the project's default currency")
            }
            FormatError::InvalidCurrency(c) => {
                write!(f, "'{c}' is not a currency code (three letters, such as BRL)")
            }
        }
    }
}

/// The format names, canonical spelling.
pub const FORMATS: &[&str] = &["number", "currency", "compact", "currency-short", "percent"];

const NBSP: &str = "\u{a0}";
const NNBSP: &str = "\u{202f}";

/// One locale's separators and patterns.
pub struct Locale {
    pub tag: &'static str,
    decimal: &'static str,
    group: &'static str,
    /// Where the currency symbol goes: before the number (`R$ 1,00`, `$1.00`) or after (`1,00 €`).
    symbol_first: bool,
    /// Between the symbol and the number (`R$ 1,00`: a no-break space; `$1.00`: nothing).
    symbol_gap: &'static str,
    percent_gap: &'static str,
    /// Compact suffixes for 10^3, 10^6, 10^9, 10^12, each with its leading gap.
    compact: [&'static str; 4],
}

/// The closed set of locales.
pub const LOCALES: &[Locale] = &[
    Locale {
        tag: "pt-BR",
        decimal: ",",
        group: ".",
        symbol_first: true,
        symbol_gap: NBSP,
        percent_gap: "",
        compact: ["\u{a0}mil", "\u{a0}mi", "\u{a0}bi", "\u{a0}tri"],
    },
    Locale {
        tag: "en-US",
        decimal: ".",
        group: ",",
        symbol_first: true,
        symbol_gap: "",
        percent_gap: "",
        compact: ["K", "M", "B", "T"],
    },
    Locale {
        tag: "fr-FR",
        decimal: ",",
        group: NNBSP,
        symbol_first: false,
        symbol_gap: NBSP,
        percent_gap: NNBSP,
        compact: ["\u{a0}k", "\u{a0}M", "\u{a0}Md", "\u{a0}Bn"],
    },
];

/// The locale named `tag`, case-insensitively (`PT-br` is `pt-BR`); the tag's
/// surrounding blanks are not part of it.
pub fn locale(tag: &str) -> Result<&'static Locale, FormatError> {
    let t = tag.trim();
    LOCALES
        .iter()
        .find(|l| l.tag.eq_ignore_ascii_case(t))
        .ok_or_else(|| FormatError::UnknownLocale(t.to_string()))
}

/// The canonical spelling of a format name, case-insensitively.
pub fn canonical_format(name: &str) -> Result<&'static str, FormatError> {
    let n = name.trim();
    FORMATS
        .iter()
        .copied()
        .find(|f| f.eq_ignore_ascii_case(n))
        .ok_or_else(|| FormatError::UnknownFormat(n.to_string()))
}

/// Check a currency code: three ASCII letters, case-insensitive; answers it in
/// upper case.
pub fn currency_code(code: &str) -> Result<String, FormatError> {
    let c = code.trim();
    if c.len() == 3 && c.bytes().all(|b| b.is_ascii_alphabetic()) {
        Ok(c.to_ascii_uppercase())
    } else {
        Err(FormatError::InvalidCurrency(c.to_string()))
    }
}

fn currency_symbol(loc: &Locale, code: &str) -> String {
    match (code, loc.tag) {
        ("BRL", _) => "R$".to_string(),
        ("USD", "pt-BR") => "US$".to_string(),
        ("USD", "fr-FR") => "$US".to_string(),
        ("USD", _) => "$".to_string(),
        ("EUR", _) => "€".to_string(),
        ("GBP", _) => "£".to_string(),
        ("JPY", _) => "¥".to_string(),
        _ => code.to_string(),
    }
}

fn currency_decimals(code: &str) -> u8 {
    if code == "JPY" {
        0
    } else {
        2
    }
}

fn pow10(n: u32) -> Option<i128> {
    10i128.checked_pow(n)
}

/// `mantissa / 10^drop`, rounded half away from zero.
fn round_div(mantissa: i128, drop: u32) -> i128 {
    let Some(p) = pow10(drop) else { return 0 };
    let q = mantissa / p;
    let r = mantissa % p;
    if r.abs() * 2 >= p {
        q + mantissa.signum()
    } else {
        q
    }
}

/// `mantissa` at `scale` expressed at `target` decimals, rounded.
fn rescale(mantissa: i128, scale: u8, target: u8) -> i128 {
    if target >= scale {
        pow10((target - scale) as u32).map_or(mantissa, |p| mantissa.saturating_mul(p))
    } else {
        round_div(mantissa, (scale - target) as u32)
    }
}

/// `digits` of a non-negative integer with `decimals` fraction digits, grouped
/// and with the locale's separators.
fn spell(loc: &Locale, magnitude: i128, decimals: u8) -> String {
    let p = pow10(decimals as u32).unwrap_or(1);
    let int = magnitude / p;
    let frac = magnitude % p;
    let int_s = int.to_string();
    let mut out = String::new();
    let len = int_s.len();
    for (i, ch) in int_s.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push_str(loc.group);
        }
        out.push(ch);
    }
    if decimals > 0 {
        out.push_str(loc.decimal);
        let f = frac.to_string();
        for _ in f.len()..decimals as usize {
            out.push('0');
        }
        out.push_str(&f);
    }
    out
}

/// The compact spelling of `|value|`: the number and the locale's suffix.
fn compact(loc: &Locale, mantissa: i128, scale: u8) -> String {
    let mag = mantissa.abs();
    // How many thousands: the largest j (0..=4) with |value| >= 1000^j.
    let mut j = 0u32;
    while j < 4 {
        let need = 3 * (j + 1) + scale as u32;
        match pow10(need) {
            Some(p) if mag >= p => j += 1,
            _ => break,
        }
    }
    loop {
        // One decimal, rounded: tenths = |value| / 1000^j * 10.
        let tenths = round_div(mag.saturating_mul(10), scale as u32 + 3 * j);
        // 999.95 K rounds to 1000.0 K, which is 1.0 M.
        if tenths >= 10_000 && j < 4 {
            j += 1;
            continue;
        }
        let whole = tenths / 10;
        let tenth = tenths % 10;
        let mut out = whole.to_string();
        if tenth != 0 {
            out.push_str(loc.decimal);
            out.push_str(&tenth.to_string());
        }
        if j > 0 {
            out.push_str(loc.compact[j as usize - 1]);
        }
        return out;
    }
}

fn wrap_currency(loc: &Locale, number: &str, symbol: &str) -> String {
    let mut out = String::new();
    if loc.symbol_first {
        out.push_str(symbol);
        out.push_str(loc.symbol_gap);
        out.push_str(number);
    } else {
        out.push_str(number);
        out.push_str(loc.symbol_gap);
        out.push_str(symbol);
    }
    out
}

/// Format an exact decimal (`mantissa` × 10^-`scale`).
///
/// `format` and `locale` are matched case-insensitively; `currency` is the
/// explicit code, or the project's default, or `None`.
pub fn format_decimal(
    mantissa: i128,
    scale: u8,
    format: &str,
    locale_tag: &str,
    currency: Option<&str>,
) -> Result<String, FormatError> {
    let fmt = canonical_format(format)?;
    let loc = locale(locale_tag)?;
    let neg = mantissa < 0;
    let code = match currency {
        Some(c) if !c.trim().is_empty() => Some(currency_code(c)?),
        _ => None,
    };
    let need_currency = || code.clone().ok_or(FormatError::NoCurrency);
    let body = match fmt {
        "number" => spell(loc, mantissa.abs(), scale),
        "percent" => {
            // A ratio: ×100, then the trailing zeros of the fraction go.
            let m = mantissa.abs().saturating_mul(100);
            let mut s = scale;
            let mut m = m;
            while s > 0 && m % 10 == 0 {
                m /= 10;
                s -= 1;
            }
            let mut out = spell(loc, m, s);
            out.push_str(loc.percent_gap);
            out.push('%');
            out
        }
        "currency" => {
            let code = need_currency()?;
            let d = currency_decimals(&code);
            let m = rescale(mantissa.abs(), scale, d);
            wrap_currency(loc, &spell(loc, m, d), &currency_symbol(loc, &code))
        }
        "compact" => compact(loc, mantissa, scale),
        "currency-short" => {
            let code = need_currency()?;
            wrap_currency(loc, &compact(loc, mantissa, scale), &currency_symbol(loc, &code))
        }
        _ => unreachable!("canonical_format answers one of FORMATS"),
    };
    // A value that rounds to zero is not negative.
    let is_zero = body.chars().all(|c| !c.is_ascii_digit() || c == '0');
    Ok(if neg && !is_zero { alloc::format!("-{body}") } else { body })
}
