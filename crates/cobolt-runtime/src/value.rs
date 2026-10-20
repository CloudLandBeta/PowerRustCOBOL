// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `CobolValue` type — the runtime representation of every COBOL data item.
//!
//! COBOL has three fundamental storage categories:
//!
//! | Category        | PIC clause examples      | Rust representation   |
//! |-----------------|--------------------------|-----------------------|
//! | Numeric         | `9(5)`, `9(7)V99`        | `i128` mantissa (scaled) |
//! | Floating-point  | COMP-1 / COMP-2          | `f64`                 |
//! | Alphanumeric    | `X(30)`, `A(10)`         | `Vec<u8>` (fixed-len) |
//!
//! Integer values are stored with an implicit decimal scale:
//! `9(5)V99` stores the integer `12345` to represent `123.45`.
//! The `decimals` field on `CobolNumeric` tracks this scale.

use std::fmt;

// ── CobolNumeric ──────────────────────────────────────────────────────────────

// The exact decimal kernel — `CobolNumeric` and its arithmetic — lives in
// `cobolt-decimal` (`no_std`, no allocator) so that this interpreter and the
// support routines linked into every generated WebAssembly module compute the
// same numbers from the same code (spec WASM-010 R6). It is re-exported here
// unchanged: nothing that names `value::CobolNumeric` notices.
pub use cobolt_decimal::CobolNumeric;
use cobolt_decimal::pow10;

/// Parse a decimal literal/string into an exact `CobolNumeric`, preserving the
/// number of fractional digits as the scale. Returns `None` if not numeric.
pub fn parse_decimal(s: &str) -> Option<CobolNumeric> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let (neg, body) = if let Some(r) = t.strip_prefix('-') {
        (true, r)
    } else {
        (false, t.strip_prefix('+').unwrap_or(t))
    };
    let body = body.trim();
    let (int_s, frac_s) = match body.split_once('.') {
        Some((i, f)) => (i, f),
        None => (body, ""),
    };
    // Cap fractional digits at the 31-digit extended precision.
    let frac_s = &frac_s[..frac_s.len().min(31)];
    let digits: String = format!("{int_s}{frac_s}");
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut mantissa: i128 = digits.parse().ok()?;
    if neg {
        mantissa = -mantissa;
    }
    Some(CobolNumeric::new(mantissa, frac_s.len() as u8))
}

// ── CobolValue ────────────────────────────────────────────────────────────────

/// The runtime value of a COBOL data item.
#[derive(Debug, Clone, PartialEq)]
pub enum CobolValue {
    /// A numeric value (integer or decimal, signed or unsigned).
    Numeric(CobolNumeric),

    /// A 64-bit float (COMP-1 / COMP-2 usage).
    Float(f64),

    /// An alphanumeric value stored as fixed-width bytes (padded with spaces).
    /// The `capacity` is the declared PIC X(n) width.
    String { bytes: Vec<u8>, capacity: usize },

    /// Uninitialized / default value (before VALUE clause is applied).
    Unset,
}

impl CobolValue {
    // ── Constructors ──────────────────────────────────────────────────────────

    /// Construct a zero-valued numeric with the given scale.
    pub fn zero(decimals: u8) -> Self {
        CobolValue::Numeric(CobolNumeric::new(0, decimals))
    }

    /// Construct a space-padded alphanumeric field of `capacity` bytes.
    pub fn spaces(capacity: usize) -> Self {
        CobolValue::String {
            bytes: vec![b' '; capacity],
            capacity,
        }
    }

    /// Construct from a Rust integer literal.
    pub fn from_i64(n: i64) -> Self {
        CobolValue::Numeric(CobolNumeric::integer(n))
    }

    /// Construct from a Rust float literal (stored as float).
    pub fn from_f64(v: f64) -> Self {
        CobolValue::Float(v)
    }

    /// Construct from a string literal, capped/padded to `capacity`.
    pub fn from_str(s: &str, capacity: usize) -> Self {
        let mut bytes = s.as_bytes().to_vec();
        bytes.truncate(capacity);
        bytes.resize(capacity, b' ');
        CobolValue::String { bytes, capacity }
    }

    // ── Conversions ───────────────────────────────────────────────────────────

    /// Try to interpret this value as an `i64` (lossless for integers).
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            CobolValue::Numeric(n) if n.decimals == 0 => Some(n.mantissa as i64),
            CobolValue::Numeric(n) => {
                // Truncate to integer part
                let int_part = pow10(n.decimals as u32)
                    .map(|p| n.mantissa / p)
                    .unwrap_or(0);
                Some(int_part as i64)
            }
            CobolValue::Float(f) => Some(*f as i64),
            CobolValue::String { bytes, .. } => {
                let s = String::from_utf8_lossy(bytes);
                s.trim().parse().ok()
            }
            CobolValue::Unset => Some(0),
        }
    }

    /// Convert to `f64`.
    pub fn as_f64(&self) -> f64 {
        match self {
            CobolValue::Numeric(n) => n.to_f64(),
            CobolValue::Float(f) => *f,
            CobolValue::String { bytes, .. } => {
                String::from_utf8_lossy(bytes).trim().parse().unwrap_or(0.0)
            }
            CobolValue::Unset => 0.0,
        }
    }

    /// Convert to a display string (space-padded or decimal-formatted).
    pub fn as_display_string(&self) -> String {
        match self {
            CobolValue::Numeric(n) => n.to_string(),
            CobolValue::Float(f) => f.to_string(),
            CobolValue::String { bytes, .. } => String::from_utf8_lossy(bytes).into_owned(),
            CobolValue::Unset => String::new(),
        }
    }

    /// `true` if this value is numeric (integer or float).
    pub fn is_numeric(&self) -> bool {
        matches!(self, CobolValue::Numeric(_) | CobolValue::Float(_))
    }

    /// `true` if the numeric value is zero, or the string is all spaces/zeros.
    pub fn is_zero(&self) -> bool {
        match self {
            CobolValue::Numeric(n) => n.mantissa == 0,
            CobolValue::Float(f) => *f == 0.0,
            CobolValue::String { bytes, .. } => bytes.iter().all(|&b| b == b' ' || b == b'0'),
            CobolValue::Unset => true,
        }
    }

    // ── Exact arithmetic ──────────────────────────────────────────────────────

    /// Interpret this value as an exact decimal, or `None` for true floats
    /// (COMP-1/COMP-2) which must use `f64` arithmetic.
    pub fn as_exact(&self) -> Option<CobolNumeric> {
        match self {
            CobolValue::Numeric(n) => Some(n.clone()),
            CobolValue::Unset => Some(CobolNumeric::integer(0)),
            CobolValue::String { bytes, .. } => parse_decimal(&String::from_utf8_lossy(bytes)),
            CobolValue::Float(_) => None,
        }
    }

    /// `self + other`, exact when both sides are decimals, else via `f64`.
    pub fn add_val(&self, other: &CobolValue) -> CobolValue {
        match (self.as_exact(), other.as_exact()) {
            (Some(a), Some(b)) => CobolValue::Numeric(a.add(&b)),
            _ => CobolValue::Float(self.as_f64() + other.as_f64()),
        }
    }

    /// `self - other`, exact when both sides are decimals, else via `f64`.
    pub fn sub_val(&self, other: &CobolValue) -> CobolValue {
        match (self.as_exact(), other.as_exact()) {
            (Some(a), Some(b)) => CobolValue::Numeric(a.sub(&b)),
            _ => CobolValue::Float(self.as_f64() - other.as_f64()),
        }
    }

    /// `self * other`, exact when both sides are decimals, else via `f64`.
    pub fn mul_val(&self, other: &CobolValue) -> CobolValue {
        match (self.as_exact(), other.as_exact()) {
            (Some(a), Some(b)) => CobolValue::Numeric(a.mul(&b)),
            _ => CobolValue::Float(self.as_f64() * other.as_f64()),
        }
    }

    /// `self / other`. `None` on division by zero.
    pub fn div_val(&self, other: &CobolValue) -> Option<CobolValue> {
        match (self.as_exact(), other.as_exact()) {
            (Some(a), Some(b)) => a.div(&b).map(CobolValue::Numeric),
            _ => {
                let d = other.as_f64();
                if d == 0.0 {
                    None
                } else {
                    Some(CobolValue::Float(self.as_f64() / d))
                }
            }
        }
    }

    /// Move `other` into `self`, respecting the receiving field's type.
    ///
    /// Alphanumeric → alphanumeric: copy bytes, truncate or pad with spaces.
    /// Numeric      → numeric:      convert and rescale.
    /// String       → numeric:      parse; numeric → string: format.
    pub fn assign(&mut self, other: &CobolValue) {
        match (self, other) {
            // Numeric ← Numeric
            (CobolValue::Numeric(dst), CobolValue::Numeric(src)) => {
                // Rescale (truncating) src to dst's PIC decimal places — exact.
                dst.mantissa = src.rescaled_to(dst.decimals);
            }
            // Numeric ← Float
            (CobolValue::Numeric(dst), CobolValue::Float(f)) => {
                dst.set_f64(*f);
            }
            // Float ← Numeric / Float
            (CobolValue::Float(dst), CobolValue::Numeric(src)) => {
                *dst = src.to_f64();
            }
            (CobolValue::Float(dst), CobolValue::Float(src)) => {
                *dst = *src;
            }
            // String ← String
            (
                CobolValue::String {
                    bytes: dst,
                    capacity,
                },
                CobolValue::String { bytes: src, .. },
            ) => {
                let cap = *capacity;
                dst.clear();
                dst.extend_from_slice(src);
                dst.truncate(cap);
                dst.resize(cap, b' ');
            }
            // String ← Numeric (right-justify)
            (
                CobolValue::String {
                    bytes: dst,
                    capacity,
                },
                CobolValue::Numeric(src),
            ) => {
                let cap = *capacity;
                let s = src.to_string();
                let sb = s.as_bytes();
                dst.clear();
                dst.resize(cap, b' ');
                let copy_len = sb.len().min(cap);
                let start = cap - copy_len;
                dst[start..].copy_from_slice(&sb[..copy_len]);
            }
            // Numeric ← String (parse)
            (CobolValue::Numeric(dst), CobolValue::String { bytes: src, .. }) => {
                let s = String::from_utf8_lossy(src);
                if let Some(parsed) = parse_decimal(&s) {
                    dst.mantissa = parsed.rescaled_to(dst.decimals);
                } else if let Ok(v) = s.trim().parse::<f64>() {
                    dst.set_f64(v);
                }
            }
            // Anything ← Unset: an uninitialised source moves as zeros (numeric)
            // or spaces (alphanumeric). It must NOT propagate Unset, or the
            // receiving field would silently swallow every later MOVE.
            (CobolValue::Numeric(dst), CobolValue::Unset) => dst.mantissa = 0,
            (CobolValue::Float(dst), CobolValue::Unset) => *dst = 0.0,
            (CobolValue::String { bytes, .. }, CobolValue::Unset) => bytes.fill(b' '),
            _ => {} // best-effort for edge cases
        }
    }
}

impl fmt::Display for CobolValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_display_string())
    }
}

// ── Figurative constants ───────────────────────────────────────────────────────

impl CobolValue {
    pub fn figurative_spaces(capacity: usize) -> Self {
        Self::spaces(capacity)
    }
    pub fn figurative_zeros(capacity: usize, decimals: u8) -> Self {
        if capacity == 0 {
            Self::Numeric(CobolNumeric::integer(0))
        } else {
            Self::String {
                bytes: vec![b'0'; capacity],
                capacity,
            }
        }
    }
    pub fn figurative_high_values(capacity: usize) -> Self {
        Self::String {
            bytes: vec![0xFF; capacity],
            capacity,
        }
    }
    pub fn figurative_low_values(capacity: usize) -> Self {
        Self::String {
            bytes: vec![0x00; capacity],
            capacity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_add() {
        let a = CobolNumeric::new(1000, 2); // 10.00
        let b = CobolNumeric::new(250, 2); //  2.50
        let c = a.add(&b);
        assert_eq!(c.mantissa, 1250);
        assert_eq!(c.decimals, 2);
        assert!((c.to_f64() - 12.50).abs() < 1e-9);
    }

    #[test]
    fn numeric_div() {
        let a = CobolNumeric::new(1000, 2); // 10.00
        let b = CobolNumeric::new(4, 0); //  4
        let c = a.div(&b).unwrap();
        assert!((c.to_f64() - 2.5).abs() < 1e-6);
    }

    #[test]
    fn string_assign_truncates() {
        let mut dst = CobolValue::spaces(5);
        let src = CobolValue::from_str("HELLO WORLD", 11);
        dst.assign(&src);
        if let CobolValue::String { bytes, capacity } = &dst {
            assert_eq!(*capacity, 5);
            assert_eq!(bytes, b"HELLO");
        }
    }

    #[test]
    fn eighteen_digit_integer_is_exact() {
        // f64 cannot represent this value; i128 fixed-point can.
        let a = CobolNumeric::integer(123_456_789_012_345_678);
        let b = CobolNumeric::integer(1);
        let c = a.add(&b);
        assert_eq!(c.mantissa, 123_456_789_012_345_679);
        assert_eq!(c.to_string(), "123456789012345679");
    }

    #[test]
    fn exact_decimal_add_no_float_drift() {
        let a = CobolNumeric::new(10010, 2); // 100.10
        let b = CobolNumeric::new(20020, 2); // 200.20
        assert_eq!(a.add(&b).to_string(), "300.30");
    }

    #[test]
    fn large_multiply_stays_exact() {
        let a = CobolNumeric::integer(1_000_000_000); // 1e9
        let b = CobolNumeric::integer(1_000_000_000); // 1e9
        let c = a.mul(&b);
        assert_eq!(c.mantissa, 1_000_000_000_000_000_000); // 1e18, exact
    }

    #[test]
    fn divide_truncates_to_guard_then_field() {
        // 10 / 3 carried to guard digits, then rescaled (truncating) to 4 places.
        let q = CobolNumeric::integer(10)
            .div(&CobolNumeric::integer(3))
            .unwrap();
        let mut field = CobolNumeric::new(0, 4);
        field.mantissa = q.rescaled_to(4);
        assert_eq!(field.to_string(), "3.3333");
    }

    /// A divisor with many decimals must not cost the quotient its precision.
    ///
    /// `1 / SQRT3` with `SQRT3 PIC S9V9(17)` asks for a shift of 43 digits,
    /// which does not fit in an i128. The old code fell back to f64 *at the
    /// dividend's own scale* — and the dividend is the integer 1, so 0.577 was
    /// rounded into zero decimal places and came back as **1**. Every
    /// `FUNCTION ATAN(1 / x)` in IF104A then answered atan(1).
    #[test]
    fn a_divisor_with_many_decimals_keeps_the_quotient_exact() {
        let sqrt3 = CobolNumeric::new(173_205_080_800_000_000, 17); // 1.732050808
        let q = CobolNumeric::integer(1).div(&sqrt3).unwrap();
        let mut field = CobolNumeric::new(0, 9);
        field.mantissa = q.rescaled_to(9);
        assert_eq!(field.to_string(), "0.577350269");

        // The same quotient however the dividend's own scale is written.
        let q = CobolNumeric::new(10, 1).div(&sqrt3).unwrap(); // 1.0
        let mut field = CobolNumeric::new(0, 9);
        field.mantissa = q.rescaled_to(9);
        assert_eq!(field.to_string(), "0.577350269");
    }

    /// …and the ordinary quotients are untouched: the precision loop starts at
    /// the guard precision this always used, so anything that fitted before
    /// still takes the same path.
    #[test]
    fn ordinary_quotients_are_unchanged() {
        for (a, b, want) in [
            (CobolNumeric::integer(1), CobolNumeric::integer(3), "0.333333333"),
            (CobolNumeric::integer(10), CobolNumeric::integer(4), "2.500000000"),
            (CobolNumeric::new(-1, 0), CobolNumeric::integer(8), "-0.125000000"),
        ] {
            let q = a.div(&b).unwrap();
            let mut field = CobolNumeric::new(0, 9);
            field.mantissa = q.rescaled_to(9);
            assert_eq!(field.to_string(), want);
        }
    }

    #[test]
    fn display_formats_from_integer() {
        assert_eq!(CobolNumeric::new(-12345, 2).to_string(), "-123.45");
        assert_eq!(CobolNumeric::new(5, 3).to_string(), "0.005");
        assert_eq!(CobolNumeric::new(0, 2).to_string(), "0.00");
    }

    #[test]
    fn parse_decimal_preserves_scale() {
        let n = parse_decimal("  -12.340 ").unwrap();
        assert_eq!(n.mantissa, -12340);
        assert_eq!(n.decimals, 3);
        assert!(parse_decimal("abc").is_none());
    }

    #[test]
    fn numeric_assign_rescales() {
        let mut dst = CobolValue::zero(2); // PIC 9(5)V99
        let src = CobolValue::from_i64(42);
        dst.assign(&src);
        if let CobolValue::Numeric(n) = &dst {
            assert_eq!(n.mantissa, 4200);
            assert_eq!(n.decimals, 2);
        }
    }
}
