// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A value on its way to or from a database (spec 087 R15).
//!
//! Typed, so a decimal amount keeps its scale and NULL stays distinct from an
//! empty value — the text-only `COBOL::"…-DB"` bridge cannot do either.

/// One SQL value.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Int(i64),
    /// An exact decimal: `mantissa × 10^-scale`.
    Decimal { mantissa: i128, scale: u8 },
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
    Bool(bool),
}

impl SqlValue {
    /// An exact decimal, reduced to an `Int` when it has no fraction and fits.
    pub fn decimal(mantissa: i128, scale: u8) -> Self {
        if scale == 0 {
            if let Ok(v) = i64::try_from(mantissa) {
                return SqlValue::Int(v);
            }
        }
        SqlValue::Decimal { mantissa, scale }
    }

    /// The value as text, the way a COBOL item or a trace shows it: decimals
    /// exact, floats in their shortest exact decimal form, NULL as `NULL`.
    pub fn display(&self) -> String {
        match self {
            SqlValue::Null => "NULL".into(),
            SqlValue::Int(v) => v.to_string(),
            SqlValue::Decimal { mantissa, scale } => decimal_text(*mantissa, *scale),
            SqlValue::Float(f) => float_text(*f),
            SqlValue::Text(s) => s.clone(),
            SqlValue::Bytes(b) => b.iter().map(|x| format!("{x:02X}")).collect(),
            SqlValue::Bool(b) => if *b { "1" } else { "0" }.into(),
        }
    }

    /// The number of significant digits of a decimal (its mantissa's digits).
    pub fn significant_digits(mantissa: i128) -> usize {
        mantissa.unsigned_abs().to_string().trim_start_matches('0').len().max(1)
    }
}

/// `mantissa × 10^-scale` written out exactly: `-12345, 2` → `-123.45`.
pub fn decimal_text(mantissa: i128, scale: u8) -> String {
    let neg = mantissa < 0;
    let digits = mantissa.unsigned_abs().to_string();
    let scale = scale as usize;
    let body = if scale == 0 {
        digits
    } else if digits.len() > scale {
        format!("{}.{}", &digits[..digits.len() - scale], &digits[digits.len() - scale..])
    } else {
        format!("0.{}{}", "0".repeat(scale - digits.len()), digits)
    };
    if neg && mantissa != 0 {
        format!("-{body}")
    } else {
        body
    }
}

/// A float in its shortest form that reads back as the same float — Rust's
/// `Display` already guarantees that round trip — never in exponent form.
pub fn float_text(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e18 {
        return format!("{}", f as i64);
    }
    format!("{f}")
}

/// Parse decimal text (`-123.45`, `+7`, `0.5`, `1e3`) into an exact mantissa
/// and scale. `None` when it is not a number.
pub fn parse_decimal(text: &str) -> Option<(i128, u8)> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if t.contains(['e', 'E']) {
        let f: f64 = t.parse().ok()?;
        return parse_decimal(&float_text(f));
    }
    let (neg, body) = match t.as_bytes()[0] {
        b'-' => (true, &t[1..]),
        b'+' => (false, &t[1..]),
        _ => (false, t),
    };
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    if int.is_empty() && frac.is_empty() {
        return None;
    }
    if !int.bytes().all(|b| b.is_ascii_digit()) || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let digits = format!("{int}{frac}");
    let m: i128 = if digits.is_empty() { 0 } else { digits.parse().ok()? };
    Some((if neg { -m } else { m }, u8::try_from(frac.len()).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_are_written_exactly() {
        assert_eq!(decimal_text(-12345, 2), "-123.45");
        assert_eq!(decimal_text(5, 3), "0.005");
        assert_eq!(decimal_text(0, 2), "0.00");
        assert_eq!(decimal_text(700, 0), "700");
        assert_eq!(parse_decimal("-123.45"), Some((-12345, 2)));
        assert_eq!(parse_decimal("+7"), Some((7, 0)));
        assert_eq!(parse_decimal(".5"), Some((5, 1)));
        assert_eq!(parse_decimal("1e3"), Some((1000, 0)));
        assert_eq!(parse_decimal("12a"), None);
        assert_eq!(parse_decimal(""), None);
        assert_eq!(float_text(0.1), "0.1");
        assert_eq!(float_text(3.0), "3");
        assert_eq!(SqlValue::decimal(42, 0), SqlValue::Int(42));
    }
}
