// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `CobolValue` type — the runtime representation of every COBOL data item.
//!
//! COBOL has three fundamental storage categories:
//!
//! | Category        | PIC clause examples      | Rust representation  |
//! |-----------------|--------------------------|----------------------|
//! | Numeric         | `9(5)`, `9(7)V99`        | `i64` (scaled)       |
//! | Floating-point  | COMP-1 / COMP-2          | `f64`                |
//! | Alphanumeric    | `X(30)`, `A(10)`         | `Vec<u8>` (fixed-len)|
//!
//! Integer values are stored with an implicit decimal scale:
//! `9(5)V99` stores the integer `12345` to represent `123.45`.
//! The `decimals` field on `CobolNumeric` tracks this scale.

use std::fmt;

// ── CobolNumeric ──────────────────────────────────────────────────────────────

/// A COBOL numeric value: an integer mantissa with an implicit decimal scale.
///
/// `value = mantissa × 10^(-decimals)`
///
/// Examples:
/// * `PIC 9(5)`      → decimals = 0, `123`    represents `123`
/// * `PIC 9(5)V99`   → decimals = 2, `12345`  represents `123.45`
/// * `PIC S9(4)V9`   → decimals = 1, `-12345` represents `-1234.5`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CobolNumeric {
    /// Scaled integer mantissa.
    pub mantissa: i64,
    /// Number of implied decimal digits (digits after the implied decimal point).
    pub decimals: u8,
}

impl CobolNumeric {
    pub fn new(mantissa: i64, decimals: u8) -> Self {
        Self { mantissa, decimals }
    }

    /// Construct from an integer (no decimal places).
    pub fn integer(n: i64) -> Self {
        Self { mantissa: n, decimals: 0 }
    }

    /// Convert to `f64` for display / EXEC RUST interop.
    pub fn to_f64(&self) -> f64 {
        self.mantissa as f64 / 10_f64.powi(self.decimals as i32)
    }

    /// Round and store a `f64` into this field's scale.
    pub fn set_f64(&mut self, v: f64) {
        let scale = 10_f64.powi(self.decimals as i32);
        self.mantissa = (v * scale).round() as i64;
    }

    /// Add another numeric value, widening scale if needed.
    pub fn add(&self, other: &CobolNumeric) -> CobolNumeric {
        if self.decimals == other.decimals {
            CobolNumeric::new(self.mantissa + other.mantissa, self.decimals)
        } else {
            // Normalise to the larger scale.
            let scale = self.decimals.max(other.decimals);
            let a = self.mantissa  * 10_i64.pow((scale - self.decimals) as u32);
            let b = other.mantissa * 10_i64.pow((scale - other.decimals) as u32);
            CobolNumeric::new(a + b, scale)
        }
    }

    pub fn sub(&self, other: &CobolNumeric) -> CobolNumeric {
        if self.decimals == other.decimals {
            CobolNumeric::new(self.mantissa - other.mantissa, self.decimals)
        } else {
            let scale = self.decimals.max(other.decimals);
            let a = self.mantissa  * 10_i64.pow((scale - self.decimals) as u32);
            let b = other.mantissa * 10_i64.pow((scale - other.decimals) as u32);
            CobolNumeric::new(a - b, scale)
        }
    }

    pub fn mul(&self, other: &CobolNumeric) -> CobolNumeric {
        CobolNumeric::new(
            self.mantissa * other.mantissa,
            self.decimals + other.decimals,
        )
    }

    pub fn div(&self, other: &CobolNumeric) -> Option<CobolNumeric> {
        if other.mantissa == 0 { return None; }
        // Use f64 for division to preserve fractional precision.
        let result = self.to_f64() / other.to_f64();
        let mut out = CobolNumeric::new(0, self.decimals);
        out.set_f64(result);
        Some(out)
    }

    /// Compare two values numerically (sign-sensitive, scale-normalised).
    pub fn cmp(&self, other: &CobolNumeric) -> std::cmp::Ordering {
        self.to_f64().partial_cmp(&other.to_f64())
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl fmt::Display for CobolNumeric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.decimals == 0 {
            write!(f, "{}", self.mantissa)
        } else {
            write!(f, "{:.prec$}", self.to_f64(), prec = self.decimals as usize)
        }
    }
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
    String {
        bytes: Vec<u8>,
        capacity: usize,
    },

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
            CobolValue::Numeric(n) if n.decimals == 0 => Some(n.mantissa),
            CobolValue::Numeric(n) => {
                // Truncate to integer part
                Some(n.mantissa / 10_i64.pow(n.decimals as u32))
            }
            CobolValue::Float(f)  => Some(*f as i64),
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
            CobolValue::Float(f)   => *f,
            CobolValue::String { bytes, .. } => {
                String::from_utf8_lossy(bytes).trim().parse().unwrap_or(0.0)
            }
            CobolValue::Unset => 0.0,
        }
    }

    /// Convert to a display string (space-padded or decimal-formatted).
    pub fn as_display_string(&self) -> String {
        match self {
            CobolValue::Numeric(n)           => n.to_string(),
            CobolValue::Float(f)             => f.to_string(),
            CobolValue::String { bytes, .. } => {
                String::from_utf8_lossy(bytes).into_owned()
            }
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
            CobolValue::Float(f)   => *f == 0.0,
            CobolValue::String { bytes, .. } => {
                bytes.iter().all(|&b| b == b' ' || b == b'0')
            }
            CobolValue::Unset => true,
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
                // Rescale src to dst's decimal places
                let scaled = if dst.decimals == src.decimals {
                    src.mantissa
                } else if dst.decimals > src.decimals {
                    src.mantissa * 10_i64.pow((dst.decimals - src.decimals) as u32)
                } else {
                    src.mantissa / 10_i64.pow((src.decimals - dst.decimals) as u32)
                };
                dst.mantissa = scaled;
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
            (CobolValue::String { bytes: dst, capacity }, CobolValue::String { bytes: src, .. }) => {
                let cap = *capacity;
                dst.clear();
                dst.extend_from_slice(src);
                dst.truncate(cap);
                dst.resize(cap, b' ');
            }
            // String ← Numeric (right-justify)
            (CobolValue::String { bytes: dst, capacity }, CobolValue::Numeric(src)) => {
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
                if let Ok(v) = s.trim().parse::<f64>() {
                    dst.set_f64(v);
                }
            }
            // Anything ← Unset → zero-fill
            (dst, CobolValue::Unset) => {
                *dst = CobolValue::Unset;
            }
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
            Self::String { bytes: vec![b'0'; capacity], capacity }
        }
    }
    pub fn figurative_high_values(capacity: usize) -> Self {
        Self::String { bytes: vec![0xFF; capacity], capacity }
    }
    pub fn figurative_low_values(capacity: usize) -> Self {
        Self::String { bytes: vec![0x00; capacity], capacity }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_add() {
        let a = CobolNumeric::new(1000, 2); // 10.00
        let b = CobolNumeric::new(250,  2); //  2.50
        let c = a.add(&b);
        assert_eq!(c.mantissa, 1250);
        assert_eq!(c.decimals, 2);
        assert!((c.to_f64() - 12.50).abs() < 1e-9);
    }

    #[test]
    fn numeric_div() {
        let a = CobolNumeric::new(1000, 2); // 10.00
        let b = CobolNumeric::new(4,    0); //  4
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
