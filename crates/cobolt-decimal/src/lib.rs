// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The exact decimal kernel of RustCOBOL: [`CobolNumeric`], an `i128` mantissa
//! with an implicit decimal scale, and the arithmetic, rounding and comparison
//! defined on it.
//!
//! # Why this is a crate of its own
//!
//! The tree-walking interpreter (`cobolt-runtime`) and the WebAssembly support
//! routines (`cobolt-wasm-rt`, linked into every generated program module,
//! spec WASM-010) must compute **the same numbers**. That is only true if there
//! is one implementation, so the kernel lives here and both depend on it. The
//! crate is `no_std` and uses no allocator, which is what lets it be compiled
//! into a module that has no heap and a memory that never grows.
//!
//! `value.rs` in the runtime re-exports [`CobolNumeric`] and keeps everything
//! that needs `std` (`CobolValue`, `parse_decimal`, string conversions).
//!
//! # Behaviour is frozen
//!
//! Every function here was moved, not rewritten: where `std` provided
//! something this crate cannot use (`f64::round`, `f64::powi`,
//! `i128::to_string`) the replacement is proven equal to the `std` function by
//! the tests in this file over a large sample, including the edges.

#![no_std]
#![forbid(unsafe_code)]

use core::cmp::Ordering;
use core::fmt;

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
pub fn pow10(exp: u32) -> Option<i128> {
    10_i128.checked_pow(exp)
}

/// `10^exp` as an `f64`, computed exactly as `f64::powi` does for a non-negative
/// exponent (square-and-multiply, the algorithm of compiler-rt's `__powidf2`).
///
/// `core` has no `powi`; this is the same function written out so that the
/// interpreter and the generated module round identically.
pub fn pow10_f64(exp: u32) -> f64 {
    let mut base = 10.0_f64;
    let mut n = exp;
    let mut r = 1.0_f64;
    loop {
        if n & 1 == 1 {
            r *= base;
        }
        n /= 2;
        if n == 0 {
            break;
        }
        base *= base;
    }
    r
}

/// Round half **away from zero** to a whole number — `f64::round` without `std`.
///
/// Below 2^52 every `f64` has a fractional part that a cast through `i64`
/// exposes exactly; at or above it the value is already integral.
pub fn round_half_away(x: f64) -> f64 {
    const TWO_52: f64 = 4503599627370496.0;
    if x != x || x >= TWO_52 || x <= -TWO_52 {
        return x; // NaN, infinities and already-integral values
    }
    let t = (x as i64) as f64; // truncation toward zero
    let frac = x - t; // exact: |frac| < 1 and shares t's sign
    if frac >= 0.5 {
        t + 1.0
    } else if frac <= -0.5 {
        t - 1.0
    } else if t == 0.0 && x.to_bits() >> 63 == 1 {
        -0.0 // keep the sign of a negative (or -0.0) that rounds to zero, as `f64::round` does
    } else {
        t
    }
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
    if carry_lo {
        hi += 1;
    }
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
        Self {
            mantissa: n as i128,
            decimals: 0,
        }
    }

    /// Convert to `f64` for display / EXEC RUST interop. Lossy for >15-digit
    /// values — used only for floating-point interop, never for exact arithmetic.
    pub fn to_f64(&self) -> f64 {
        self.mantissa as f64 / pow10_f64(self.decimals as u32)
    }

    /// Round and store a `f64` into this field's scale.
    pub fn set_f64(&mut self, v: f64) {
        let scale = pow10_f64(self.decimals as u32);
        self.mantissa = round_half_away(v * scale) as i128;
    }

    /// Rescale this value's mantissa to `scale` decimal places (saturating).
    pub fn rescaled_to(&self, scale: u8) -> i128 {
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
        if other.mantissa == 0 {
            return None;
        }
        // Working fractional precision (guard digits); the receiving field
        // rescales to its own PIC on assignment.
        let want = self.decimals.max(other.decimals).saturating_add(9).min(31);
        // Giving the quotient `dr` fractional digits means shifting the
        // dividend by `dr + other.decimals - self.decimals`, and that shift has
        // to fit beside it in an i128. A divisor with many decimals makes it
        // large on its own: `1 / SQRT3`, with `SQRT3 PIC S9V9(17)`, asks for a
        // shift of 43 and 10^43 does not fit.
        //
        // **Give up precision rather than the answer.** Dropping guard digits
        // until the shift fits keeps the exact integer path, which is where a
        // quotient belongs; only a genuinely enormous magnitude reaches the
        // float fallback below. Nothing that fits today changes: the loop
        // starts at the precision this always used.
        let mut dr = want;
        loop {
            let exp = dr as i32 + other.decimals as i32 - self.decimals as i32;
            let num = if exp >= 0 {
                pow10(exp as u32).and_then(|p| self.mantissa.checked_mul(p))
            } else {
                pow10((-exp) as u32).map(|p| self.mantissa / p)
            };
            if let Some(n) = num {
                return Some(CobolNumeric::new(n / other.mantissa, dr));
            }
            if dr == 0 {
                break;
            }
            dr -= 1;
        }
        // Even an unscaled quotient overflows — fall back to f64, at the
        // **working** precision rather than the dividend's own scale. Taking
        // the dividend's scale meant `1 / SQRT3` was rounded into a value with
        // no decimals at all and came out as 1, so every `FUNCTION ATAN(1 / x)`
        // in IF104A answered atan(1) (IF101A, IF103A and IF104A between them).
        let mut out = CobolNumeric::new(0, want.min(18));
        out.set_f64(self.to_f64() / other.to_f64());
        Some(out)
    }

    /// Number of digits in the integer part (zero → 0, so it fits any field).
    /// Used to detect ON SIZE ERROR overflow against a field's PIC capacity.
    pub fn integer_digit_count(&self) -> u32 {
        let p = pow10(self.decimals as u32).unwrap_or(1);
        let mut int_part = (self.mantissa / p).unsigned_abs();
        let mut digits = 0;
        while int_part != 0 {
            digits += 1;
            int_part /= 10;
        }
        digits
    }

    /// Return this value **truncated** (toward zero) to `scale` decimal places
    /// — COBOL's default when `ROUNDED` is absent.
    pub fn truncate_to(&self, scale: u8) -> CobolNumeric {
        CobolNumeric::new(self.rescaled_to(scale), scale)
    }

    /// Return this value rounded (half away from zero) to `scale` decimal places.
    pub fn round_to(&self, scale: u8) -> CobolNumeric {
        if scale >= self.decimals {
            return CobolNumeric::new(self.rescaled_to(scale), scale);
        }
        let drop = (self.decimals - scale) as u32;
        let divisor = match pow10(drop) {
            Some(d) => d,
            None => return CobolNumeric::new(0, scale),
        };
        let q = self.mantissa / divisor;
        let rem = (self.mantissa % divisor).abs();
        // Round half away from zero.
        let bump = if rem * 2 >= divisor {
            self.mantissa.signum()
        } else {
            0
        };
        CobolNumeric::new(q + bump, scale)
    }

    /// Round to a whole power of ten **above** the units position.
    ///
    /// `PIC 99P` stores two digits standing for tens, so its least significant
    /// digit position is the tens — `ROUNDED` there means "to the nearest 10",
    /// which [`Self::round_to`] cannot express because its scale counts
    /// positions *after* the point and stops at zero. Truncating instead left
    /// `-99` as `-90` where the standard requires `-100`.
    pub fn round_to_power(&self, power: u32) -> CobolNumeric {
        let units = self.round_to(0);
        let divisor = match pow10(power) {
            Some(d) => d,
            None => return CobolNumeric::new(0, 0),
        };
        let q = units.mantissa / divisor;
        let rem = (units.mantissa % divisor).abs();
        let bump = if rem * 2 >= divisor {
            units.mantissa.signum()
        } else {
            0
        };
        CobolNumeric::new((q + bump) * divisor, 0)
    }

    /// Compare two values numerically (sign-sensitive, scale-normalised, exact).
    pub fn cmp(&self, other: &CobolNumeric) -> Ordering {
        let scale = self.decimals.max(other.decimals);
        self.rescaled_to(scale).cmp(&other.rescaled_to(scale))
    }
}

/// The decimal digits of `n`, most significant first, written into the tail of
/// `buf`; returns the index of the first digit. 39 bytes hold any `u128`.
fn digits_u128(mut n: u128, buf: &mut [u8; 40]) -> usize {
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    i
}

impl fmt::Display for CobolNumeric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.decimals == 0 {
            return write!(f, "{}", self.mantissa);
        }
        // Format straight from the integer mantissa so large values keep every
        // digit (no f64 rounding).
        let dec = self.decimals as usize;
        let mut buf = [0u8; 40];
        let start = digits_u128(self.mantissa.unsigned_abs(), &mut buf);
        let digits = &buf[start..];
        if self.mantissa < 0 {
            f.write_str("-")?;
        }
        // Integer part: the digits left of the point, or a single "0" when the
        // magnitude has `dec` digits or fewer.
        if digits.len() <= dec {
            f.write_str("0")?;
            f.write_str(".")?;
            for _ in 0..(dec - digits.len()) {
                f.write_str("0")?;
            }
            f.write_str(core::str::from_utf8(digits).unwrap_or(""))
        } else {
            let split = digits.len() - dec;
            f.write_str(core::str::from_utf8(&digits[..split]).unwrap_or(""))?;
            f.write_str(".")?;
            f.write_str(core::str::from_utf8(&digits[split..]).unwrap_or(""))
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::string::{String, ToString};
    use std::vec::Vec;

    /// A tiny deterministic generator, so the sample is the same on every run.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            self.0 ^ (self.0 >> 29)
        }
        fn i128(&mut self) -> i128 {
            (((self.next() as u128) << 64) | self.next() as u128) as i128
        }
    }

    #[test]
    fn round_half_away_equals_std_round() {
        let mut cases: Vec<f64> = std::vec![
            0.0, -0.0, 0.5, -0.5, 1.5, -1.5, 2.5, -2.5, 0.49999999999999994, -0.49999999999999994,
            4503599627370495.5, 4503599627370496.0, -4503599627370495.5, 1e300, -1e300,
            f64::INFINITY, f64::NEG_INFINITY, f64::MAX, f64::MIN, f64::MIN_POSITIVE,
            123456789.5, -123456789.5, 1e15 + 0.5, 0.1 + 0.2,
        ];
        let mut g = Lcg(7);
        for _ in 0..2_000_000 {
            cases.push(f64::from_bits(g.next()));
            let m = (g.next() % 2_000_000) as f64 - 1_000_000.0;
            cases.push(m / 2.0);
            cases.push(m / 1000.0);
        }
        for x in cases {
            let a = round_half_away(x);
            let b = x.round();
            assert!(
                a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()),
                "round({x:?}): ours {a:?}, std {b:?}"
            );
        }
    }

    #[test]
    fn pow10_f64_equals_std_powi() {
        for n in 0..=40u32 {
            assert_eq!(pow10_f64(n).to_bits(), 10_f64.powi(n as i32).to_bits(), "10^{n}");
        }
    }

    #[test]
    fn display_equals_the_old_string_implementation() {
        // The formatter this crate replaced, kept here as the reference.
        fn old(n: &CobolNumeric) -> String {
            if n.decimals == 0 {
                return n.mantissa.to_string();
            }
            let dec = n.decimals as usize;
            let digits = n.mantissa.unsigned_abs().to_string();
            let padded = if digits.len() <= dec {
                std::format!("{}{}", "0".repeat(dec + 1 - digits.len()), digits)
            } else {
                digits
            };
            let split = padded.len() - dec;
            let (int_part, frac) = padded.split_at(split);
            std::format!("{}{}.{}", if n.mantissa < 0 { "-" } else { "" }, int_part, frac)
        }
        let mut g = Lcg(11);
        for _ in 0..300_000 {
            let mantissa = match g.next() % 4 {
                0 => g.i128(),
                1 => (g.next() % 100_000) as i128 - 50_000,
                2 => (g.next() % 10) as i128,
                _ => -((g.next() % 1000) as i128),
            };
            let decimals = (g.next() % 40) as u8;
            let n = CobolNumeric::new(mantissa, decimals);
            assert_eq!(n.to_string(), old(&n), "{mantissa} / {decimals}");
        }
        for (m, d) in [(0, 0), (0, 2), (5, 3), (-12345, 2), (i128::MIN, 0), (i128::MIN, 5), (i128::MAX, 38)] {
            let n = CobolNumeric::new(m, d);
            assert_eq!(n.to_string(), old(&n), "{m} / {d}");
        }
    }

    #[test]
    fn integer_digit_count_equals_the_old_implementation() {
        fn old(n: &CobolNumeric) -> u32 {
            let p = pow10(n.decimals as u32).unwrap_or(1);
            let int_part = (n.mantissa / p).unsigned_abs();
            if int_part == 0 { 0 } else { int_part.to_string().len() as u32 }
        }
        let mut g = Lcg(13);
        for _ in 0..300_000 {
            let n = CobolNumeric::new(g.i128() >> (g.next() % 120), (g.next() % 40) as u8);
            assert_eq!(n.integer_digit_count(), old(&n), "{:?}", n);
        }
    }

    #[test]
    fn arithmetic_identities_hold() {
        let a = CobolNumeric::new(1000, 2);
        let b = CobolNumeric::new(250, 2);
        assert_eq!(a.add(&b).mantissa, 1250);
        assert_eq!(a.sub(&b).mantissa, 750);
        assert_eq!(a.mul(&b).mantissa, 250_000);
        assert_eq!(a.mul(&b).decimals, 4);
        assert_eq!(a.cmp(&b), Ordering::Greater);
        assert!(a.div(&CobolNumeric::integer(0)).is_none());
    }
}
