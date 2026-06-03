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
    /// Scaled integer mantissa. An `i128` holds ~38 significant digits, covering
    /// COBOL's 18-digit standard limit and up to 31-digit extended precision,
    /// so ADD/SUBTRACT/MULTIPLY stay exact (no `f64` round-trips).
    pub mantissa: i128,
    /// Number of implied decimal digits (digits after the implied decimal point).
    pub decimals: u8,
}

/// `10^exp` as an `i128`, or `None` if it would overflow (~`10^38`).
fn pow10(exp: u32) -> Option<i128> {
    10_i128.checked_pow(exp)
}

/// 128×128→256-bit unsigned product, returned as `(high, low)` u128 limbs.
fn umul128(a: u128, b: u128) -> (u128, u128) {
    let mask = u64::MAX as u128;
    let (ah, al) = (a >> 64, a & mask);
    let (bh, bl) = (b >> 64, b & mask);
    let ll = al * bl;
    let lh = al * bh;
    let hl = ah * bl;
    let hh = ah * bh;
    // Sum the two middle partials (may carry past 128 bits).
    let (mid, carry_mid) = lh.overflowing_add(hl);
    let mut hi = hh + (mid >> 64) + if carry_mid { 1u128 << 64 } else { 0 };
    let (lo, carry_lo) = ll.overflowing_add(mid << 64);
    if carry_lo { hi += 1; }
    (hi, lo)
}

/// Divide a 256-bit unsigned `(hi, lo)` by 10, returning `((hi, lo), remainder)`.
fn div256_by10(hi: u128, lo: u128) -> ((u128, u128), u8) {
    let q_hi = hi / 10;
    let r_hi = hi % 10;
    // Fold the high remainder into the low limb, 64 bits at a time (so
    // `r * 2^64 + half` never overflows u128).
    let mask = u64::MAX as u128;
    let part1 = (r_hi << 64) | (lo >> 64);
    let (q1, r1) = (part1 / 10, part1 % 10);
    let part2 = (r1 << 64) | (lo & mask);
    let (q2, r2) = (part2 / 10, part2 % 10);
    let q_lo = (q1 << 64) | q2;
    ((q_hi, q_lo), r2 as u8)
}

impl CobolNumeric {
    pub fn new(mantissa: i128, decimals: u8) -> Self {
        Self { mantissa, decimals }
    }

    /// Construct from an integer (no decimal places).
    pub fn integer(n: i64) -> Self {
        Self { mantissa: n as i128, decimals: 0 }
    }

    /// Convert to `f64` for display / EXEC RUST interop. Lossy for >15-digit
    /// values — used only for floating-point interop, never for exact arithmetic.
    pub fn to_f64(&self) -> f64 {
        self.mantissa as f64 / 10_f64.powi(self.decimals as i32)
    }

    /// Round and store a `f64` into this field's scale.
    pub fn set_f64(&mut self, v: f64) {
        let scale = 10_f64.powi(self.decimals as i32);
        self.mantissa = (v * scale).round() as i128;
    }

    /// Rescale this value's mantissa to `scale` decimal places (saturating).
    fn rescaled_to(&self, scale: u8) -> i128 {
        if scale >= self.decimals {
            match pow10((scale - self.decimals) as u32) {
                Some(p) => self.mantissa.saturating_mul(p),
                None => self.mantissa,
            }
        } else {
            match pow10((self.decimals - scale) as u32) {
                Some(p) => self.mantissa / p,
                None => 0,
            }
        }
    }

    /// Add another numeric value, widening scale if needed (exact integer math).
    pub fn add(&self, other: &CobolNumeric) -> CobolNumeric {
        let scale = self.decimals.max(other.decimals);
        let a = self.rescaled_to(scale);
        let b = other.rescaled_to(scale);
        CobolNumeric::new(a.saturating_add(b), scale)
    }

    pub fn sub(&self, other: &CobolNumeric) -> CobolNumeric {
        let scale = self.decimals.max(other.decimals);
        let a = self.rescaled_to(scale);
        let b = other.rescaled_to(scale);
        CobolNumeric::new(a.saturating_sub(b), scale)
    }

    pub fn mul(&self, other: &CobolNumeric) -> CobolNumeric {
        // Full 256-bit product so 31-digit × 31-digit stays exact; then drop
        // surplus fractional digits until the magnitude fits back into i128
        // (COBOL fields never exceed 31 digits, so no significance is lost).
        let neg = (self.mantissa < 0) ^ (other.mantissa < 0);
        let (mut hi, mut lo) = umul128(self.mantissa.unsigned_abs(), other.mantissa.unsigned_abs());
        let mut scale = self.decimals.saturating_add(other.decimals);
        while hi != 0 || lo > i128::MAX as u128 {
            if scale == 0 {
                // Integer part alone exceeds ~38 digits — saturate (size error).
                return CobolNumeric::new(if neg { i128::MIN } else { i128::MAX }, 0);
            }
            let ((nhi, nlo), _r) = div256_by10(hi, lo);
            hi = nhi;
            lo = nlo;
            scale -= 1;
        }
        let mag = lo as i128;
        CobolNumeric::new(if neg { -mag } else { mag }, scale)
    }

    /// Divide using exact integer math, producing a quotient carried to enough
    /// guard digits that the receiving field's truncation/rounding is correct.
    pub fn div(&self, other: &CobolNumeric) -> Option<CobolNumeric> {
        if other.mantissa == 0 { return None; }
        // Working fractional precision (guard digits); the receiving field
        // rescales to its own PIC on assignment.
        let dr = self.decimals.max(other.decimals).saturating_add(9).min(31);
        let exp = dr as i32 + other.decimals as i32 - self.decimals as i32;
        let num = if exp >= 0 {
            pow10(exp as u32).and_then(|p| self.mantissa.checked_mul(p))
        } else {
            pow10((-exp) as u32).map(|p| self.mantissa / p)
        };
        match num {
            Some(n) => Some(CobolNumeric::new(n / other.mantissa, dr)),
            // Overflowed i128 → fall back to f64 (rare, very large magnitudes).
            None => {
                let mut out = CobolNumeric::new(0, self.decimals);
                out.set_f64(self.to_f64() / other.to_f64());
                Some(out)
            }
        }
    }

    /// Number of digits in the integer part (zero → 0, so it fits any field).
    /// Used to detect ON SIZE ERROR overflow against a field's PIC capacity.
    pub fn integer_digit_count(&self) -> u32 {
        let p = pow10(self.decimals as u32).unwrap_or(1);
        let int_part = (self.mantissa / p).unsigned_abs();
        if int_part == 0 { 0 } else { int_part.to_string().len() as u32 }
    }

    /// Return this value rounded (half away from zero) to `scale` decimal places.
    pub fn round_to(&self, scale: u8) -> CobolNumeric {
        if scale >= self.decimals {
            return CobolNumeric::new(self.rescaled_to(scale), scale);
        }
        let drop = (self.decimals - scale) as u32;
        let divisor = match pow10(drop) { Some(d) => d, None => return CobolNumeric::new(0, scale) };
        let q = self.mantissa / divisor;
        let rem = (self.mantissa % divisor).abs();
        // Round half away from zero.
        let bump = if rem * 2 >= divisor { self.mantissa.signum() } else { 0 };
        CobolNumeric::new(q + bump, scale)
    }

    /// Compare two values numerically (sign-sensitive, scale-normalised, exact).
    pub fn cmp(&self, other: &CobolNumeric) -> std::cmp::Ordering {
        let scale = self.decimals.max(other.decimals);
        self.rescaled_to(scale).cmp(&other.rescaled_to(scale))
    }
}

impl fmt::Display for CobolNumeric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.decimals == 0 {
            return write!(f, "{}", self.mantissa);
        }
        // Format straight from the integer mantissa so large values keep every
        // digit (no f64 rounding).
        let dec = self.decimals as usize;
        let digits = self.mantissa.unsigned_abs().to_string();
        let padded = if digits.len() <= dec {
            format!("{}{}", "0".repeat(dec + 1 - digits.len()), digits)
        } else {
            digits
        };
        let split = padded.len() - dec;
        let (int_part, frac) = padded.split_at(split);
        write!(
            f,
            "{}{}.{}",
            if self.mantissa < 0 { "-" } else { "" },
            int_part,
            frac
        )
    }
}

/// Parse a decimal literal/string into an exact `CobolNumeric`, preserving the
/// number of fractional digits as the scale. Returns `None` if not numeric.
pub fn parse_decimal(s: &str) -> Option<CobolNumeric> {
    let t = s.trim();
    if t.is_empty() { return None; }
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
    if neg { mantissa = -mantissa; }
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
            CobolValue::Numeric(n) if n.decimals == 0 => Some(n.mantissa as i64),
            CobolValue::Numeric(n) => {
                // Truncate to integer part
                let int_part = pow10(n.decimals as u32)
                    .map(|p| n.mantissa / p)
                    .unwrap_or(0);
                Some(int_part as i64)
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

    // ── Exact arithmetic ──────────────────────────────────────────────────────

    /// Interpret this value as an exact decimal, or `None` for true floats
    /// (COMP-1/COMP-2) which must use `f64` arithmetic.
    pub fn as_exact(&self) -> Option<CobolNumeric> {
        match self {
            CobolValue::Numeric(n) => Some(n.clone()),
            CobolValue::Unset      => Some(CobolNumeric::integer(0)),
            CobolValue::String { bytes, .. } => {
                parse_decimal(&String::from_utf8_lossy(bytes))
            }
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
                if d == 0.0 { None } else { Some(CobolValue::Float(self.as_f64() / d)) }
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
            (CobolValue::Float(dst), CobolValue::Unset)   => *dst = 0.0,
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
        let q = CobolNumeric::integer(10).div(&CobolNumeric::integer(3)).unwrap();
        let mut field = CobolNumeric::new(0, 4);
        field.mantissa = q.rescaled_to(4);
        assert_eq!(field.to_string(), "3.3333");
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
