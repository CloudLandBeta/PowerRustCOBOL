// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Exact decimal text (WASM-005 R5, rule N).
//!
//! A numeric item travels as its decimal text with its PICTURE scale. Nothing
//! here ever builds a binary floating-point value: a 30-digit amount is
//! written, read and compared as digits.
//!
//! The **canonical text** of a numeric item is `-?D+(.D+)?`: no `+`, no
//! exponent, no leading zeros except the single `0` before the point, exactly
//! `scale` fraction digits (none, and no point, when the scale is 0), and no
//! sign on zero.

/// Why a number does not fit its picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberError {
    /// Not a decimal number.
    Syntax,
    /// More fraction digits than the picture's scale (after dropping trailing zeros).
    TooManyDecimals,
    /// More integer digits than the picture holds.
    TooManyDigits,
    /// A negative number for an unsigned picture.
    NegativeUnsigned,
    /// An exponent or length beyond anything a picture could hold.
    OutOfRange,
}

/// The canonical text of the decimal number `text` (JSON number grammar plus a
/// leading `+`) for an item of `digits` integer digits, `scale` fraction digits
/// and sign `signed`, or why it does not fit.
pub fn canonical_from_decimal(text: &str, digits: u16, scale: u16, signed: bool) -> Result<String, NumberError> {
    let b = text.as_bytes();
    let mut i = 0usize;
    let mut negative = false;
    match b.first() {
        Some(b'-') => {
            negative = true;
            i = 1;
        }
        Some(b'+') => i = 1,
        _ => {}
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_part = &text[int_start..i];
    let mut frac_part = "";
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let fs = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        frac_part = &text[fs..i];
        if frac_part.is_empty() {
            return Err(NumberError::Syntax);
        }
    }
    if int_part.is_empty() && frac_part.is_empty() {
        return Err(NumberError::Syntax);
    }
    let mut exp: i64 = 0;
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        let mut eneg = false;
        match b.get(i) {
            Some(b'-') => {
                eneg = true;
                i += 1;
            }
            Some(b'+') => i += 1,
            _ => {}
        }
        let es = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if es == i {
            return Err(NumberError::Syntax);
        }
        // A bounded read: no exponent beyond what a 38-digit picture could use.
        let digits_txt = &text[es..i];
        if digits_txt.len() > 6 {
            return Err(NumberError::OutOfRange);
        }
        exp = digits_txt.parse::<i64>().map_err(|_| NumberError::Syntax)?;
        if eneg {
            exp = -exp;
        }
    }
    if i != b.len() {
        return Err(NumberError::Syntax);
    }
    if exp.abs() > 1000 || int_part.len() + frac_part.len() > 4000 {
        return Err(NumberError::OutOfRange);
    }

    // All the digits, and where the decimal point falls among them.
    let all: String = format!("{int_part}{frac_part}");
    let point = int_part.len() as i64 + exp; // digits before the point
    let (mut int_out, mut frac_out): (String, String);
    if point <= 0 {
        int_out = "0".to_string();
        frac_out = format!("{}{}", "0".repeat((-point) as usize), all);
    } else if point as usize >= all.len() {
        int_out = format!("{}{}", all, "0".repeat(point as usize - all.len()));
        frac_out = String::new();
    } else {
        int_out = all[..point as usize].to_string();
        frac_out = all[point as usize..].to_string();
    }
    // Normalise: no leading zeros in the integer part, no trailing zeros in the fraction.
    let trimmed = int_out.trim_start_matches('0');
    int_out = if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() };
    frac_out = frac_out.trim_end_matches('0').to_string();

    let is_zero = int_out == "0" && frac_out.is_empty();
    if frac_out.len() > scale as usize {
        return Err(NumberError::TooManyDecimals);
    }
    if !is_zero && int_out != "0" && int_out.len() > digits as usize {
        return Err(NumberError::TooManyDigits);
    }
    if negative && !is_zero && !signed {
        return Err(NumberError::NegativeUnsigned);
    }
    let mut out = String::with_capacity(int_out.len() + scale as usize + 2);
    if negative && !is_zero {
        out.push('-');
    }
    out.push_str(&int_out);
    if scale > 0 {
        out.push('.');
        out.push_str(&frac_out);
        for _ in frac_out.len()..scale as usize {
            out.push('0');
        }
    }
    Ok(out)
}

/// The canonical text for an `i128` mantissa at `scale` fraction digits.
pub fn from_mantissa(mantissa: i128, scale: u16) -> String {
    let neg = mantissa < 0;
    let digits = mantissa.unsigned_abs().to_string();
    let scale = scale as usize;
    let padded = if digits.len() <= scale { format!("{}{}", "0".repeat(scale + 1 - digits.len()), digits) } else { digits };
    let split = padded.len() - scale;
    let mut out = String::with_capacity(padded.len() + 2);
    if neg {
        out.push('-');
    }
    out.push_str(&padded[..split]);
    if scale > 0 {
        out.push('.');
        out.push_str(&padded[split..]);
    }
    out
}

/// The mantissa of canonical text: `-12.50` is `-1250`. `None` when the text
/// is not canonical-shaped or does not fit an `i128`.
pub fn to_mantissa(canonical: &str) -> Option<i128> {
    let neg = canonical.starts_with('-');
    let body = canonical.strip_prefix('-').unwrap_or(canonical);
    let mut digits = String::with_capacity(body.len());
    let mut seen_point = false;
    for ch in body.chars() {
        match ch {
            '0'..='9' => digits.push(ch),
            '.' if !seen_point => seen_point = true,
            _ => return None,
        }
    }
    if digits.is_empty() {
        return None;
    }
    let v: i128 = digits.parse().ok()?;
    Some(if neg { -v } else { v })
}

/// Is `text` canonical for `digits`/`scale`/`signed`? (What the encoder demands
/// of a leaf it is handed.)
pub fn is_canonical(text: &str, digits: u16, scale: u16, signed: bool) -> bool {
    match canonical_from_decimal(text, digits, scale, signed) {
        Ok(c) => c == text,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_forms() {
        assert_eq!(canonical_from_decimal("12.5", 5, 2, true).unwrap(), "12.50");
        assert_eq!(canonical_from_decimal("-0.05", 3, 2, true).unwrap(), "-0.05");
        assert_eq!(canonical_from_decimal("-0.00", 3, 2, true).unwrap(), "0.00");
        assert_eq!(canonical_from_decimal("1e2", 3, 0, false).unwrap(), "100");
        assert_eq!(canonical_from_decimal("1.5E+1", 3, 0, false).unwrap(), "15");
        assert_eq!(canonical_from_decimal("12345678901234567890123456789012345678", 38, 0, false).unwrap(), "12345678901234567890123456789012345678");
        assert_eq!(canonical_from_decimal("007", 3, 0, false).unwrap(), "7");
        assert_eq!(canonical_from_decimal("0", 1, 0, false).unwrap(), "0");
        assert_eq!(canonical_from_decimal("1.500", 2, 2, false).unwrap(), "1.50");
    }

    #[test]
    fn what_does_not_fit() {
        assert_eq!(canonical_from_decimal("1.505", 3, 2, true), Err(NumberError::TooManyDecimals));
        assert_eq!(canonical_from_decimal("1000", 3, 0, true), Err(NumberError::TooManyDigits));
        assert_eq!(canonical_from_decimal("-1", 3, 0, false), Err(NumberError::NegativeUnsigned));
        assert_eq!(canonical_from_decimal("1e9999", 3, 0, true), Err(NumberError::OutOfRange));
        assert_eq!(canonical_from_decimal("1.", 3, 0, true), Err(NumberError::Syntax));
        assert_eq!(canonical_from_decimal("", 3, 0, true), Err(NumberError::Syntax));
        assert_eq!(canonical_from_decimal("0x10", 3, 0, true), Err(NumberError::Syntax));
    }

    #[test]
    fn mantissa_round_trip() {
        for (m, s, t) in [(1250i128, 2u16, "12.50"), (-5, 2, "-0.05"), (0, 0, "0"), (7, 0, "7"), (-123456789012345678901234567890i128, 3, "-123456789012345678901234567.890")] {
            assert_eq!(from_mantissa(m, s), t);
            assert_eq!(to_mantissa(t), Some(m));
        }
    }
}
