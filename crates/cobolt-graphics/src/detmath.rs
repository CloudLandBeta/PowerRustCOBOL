// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Elementary functions built from `+ - * /` only.
//!
//! The standard library's `sin`/`cos`/`exp` call the platform's maths library,
//! which is allowed to differ in the last bit from one platform to the next (macOS,
//! glibc, the `libm` crate that `wasm32` uses). Where a coefficient computed from
//! them ends up in a pixel - a hue rotation matrix, a filter kernel - this module
//! gives the same bits everywhere, so the picture is the same on every platform.

use std::f64::consts::{FRAC_PI_2, PI};

// pi/2 split in three parts (Cody-Waite) so the argument reduction keeps its precision.
const PIO2_HI: f64 = 1.570_796_326_734_125_6;
const PIO2_MID: f64 = 6.077_100_506_303_966e-11;
const PIO2_LO: f64 = 2.022_266_248_795_950_6e-21;

fn sin_poly(r: f64) -> f64 {
    // Taylor series to r^17 on |r| <= pi/4: error < 1e-17.
    let r2 = r * r;
    let mut term = r;
    let mut sum = r;
    let mut k = 1.0;
    while k <= 8.0 {
        term = -term * r2 / ((2.0 * k) * (2.0 * k + 1.0));
        sum += term;
        k += 1.0;
    }
    sum
}

fn cos_poly(r: f64) -> f64 {
    let r2 = r * r;
    let mut term = 1.0;
    let mut sum = 1.0;
    let mut k = 1.0;
    while k <= 8.0 {
        term = -term * r2 / ((2.0 * k - 1.0) * (2.0 * k));
        sum += term;
        k += 1.0;
    }
    sum
}

/// `(sin x, cos x)` for any finite `x` (NaN in, NaN out).
pub(crate) fn sin_cos(x: f64) -> (f64, f64) {
    if !x.is_finite() {
        return (f64::NAN, f64::NAN);
    }
    // Reduce to a multiple of pi/2 and a remainder in [-pi/4, pi/4].
    let k = (x / FRAC_PI_2).round();
    let r = ((x - k * PIO2_HI) - k * PIO2_MID) - k * PIO2_LO;
    let (s, c) = (sin_poly(r), cos_poly(r));
    match (k.rem_euclid(4.0)) as i64 {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

/// Degrees to radians.
pub(crate) fn radians(deg: f64) -> f64 {
    deg * (PI / 180.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_library_closely_everywhere_that_matters() {
        let mut worst: f64 = 0.0;
        let mut x = -50.0;
        while x < 50.0 {
            let (s, c) = sin_cos(x);
            worst = worst.max((s - x.sin()).abs()).max((c - x.cos()).abs());
            x += 0.0137;
        }
        assert!(worst < 1e-12, "worst error {worst}");
    }

    #[test]
    fn exact_at_the_quadrants() {
        for (deg, s, c) in [(0.0, 0.0, 1.0), (90.0, 1.0, 0.0), (180.0, 0.0, -1.0), (270.0, -1.0, 0.0), (360.0, 0.0, 1.0)] {
            let (rs, rc) = sin_cos(radians(deg));
            assert!((rs - s).abs() < 1e-15 && (rc - c).abs() < 1e-15, "{deg}: {rs} {rc}");
        }
    }

    #[test]
    fn non_finite_input_is_nan() {
        assert!(sin_cos(f64::NAN).0.is_nan());
        assert!(sin_cos(f64::INFINITY).1.is_nan());
    }
}
