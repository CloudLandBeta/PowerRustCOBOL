// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A number in flight: an exact decimal or a float.
//!
//! This mirrors the numeric half of the interpreter's `CobolValue`: arithmetic
//! is exact while both sides are decimals and falls to `f64` as soon as either
//! is `COMP-1`/`COMP-2` (`add_val`, `sub_val`, `mul_val`, `div_val`), and
//! exponentiation is always floating point. The decimal arithmetic itself is
//! [`cobolt_decimal::CobolNumeric`], shared with the interpreter.

use cobolt_decimal::CobolNumeric;
use core::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Val {
    Dec(CobolNumeric),
    F(f64),
}

impl Val {
    pub const ZERO: Val = Val::Dec(CobolNumeric { mantissa: 0, decimals: 0 });

    pub fn int(n: i64) -> Val {
        Val::Dec(CobolNumeric::integer(n))
    }

    pub fn as_f64(&self) -> f64 {
        match self {
            Val::Dec(n) => n.to_f64(),
            Val::F(f) => *f,
        }
    }

    pub fn is_zero(&self) -> bool {
        match self {
            Val::Dec(n) => n.mantissa == 0,
            Val::F(f) => *f == 0.0,
        }
    }

    pub fn add(&self, o: &Val) -> Val {
        match (self, o) {
            (Val::Dec(a), Val::Dec(b)) => Val::Dec(a.add(b)),
            _ => Val::F(self.as_f64() + o.as_f64()),
        }
    }

    pub fn sub(&self, o: &Val) -> Val {
        match (self, o) {
            (Val::Dec(a), Val::Dec(b)) => Val::Dec(a.sub(b)),
            _ => Val::F(self.as_f64() - o.as_f64()),
        }
    }

    pub fn mul(&self, o: &Val) -> Val {
        match (self, o) {
            (Val::Dec(a), Val::Dec(b)) => Val::Dec(a.mul(b)),
            _ => Val::F(self.as_f64() * o.as_f64()),
        }
    }

    /// `None` on division by zero.
    pub fn div(&self, o: &Val) -> Option<Val> {
        match (self, o) {
            (Val::Dec(a), Val::Dec(b)) => a.div(b).map(Val::Dec),
            _ => {
                let d = o.as_f64();
                if d == 0.0 {
                    None
                } else {
                    Some(Val::F(self.as_f64() / d))
                }
            }
        }
    }

    pub fn neg(&self) -> Val {
        Val::ZERO.sub(self)
    }

    /// `self ** o`, always in floating point (as the interpreter does).
    pub fn pow(&self, o: &Val) -> Val {
        Val::F(libm_pow(self.as_f64(), o.as_f64()))
    }

    /// Numeric comparison: exact for two decimals, `f64` otherwise. NaN
    /// compares equal to nothing, so it orders as `None`.
    pub fn compare(&self, o: &Val) -> Option<Ordering> {
        match (self, o) {
            (Val::Dec(a), Val::Dec(b)) => Some(a.cmp(b)),
            _ => self.as_f64().partial_cmp(&o.as_f64()),
        }
    }
}

/// `f64::powf` without `std`.
fn libm_pow(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}
