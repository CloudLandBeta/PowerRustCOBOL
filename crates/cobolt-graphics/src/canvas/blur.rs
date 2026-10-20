// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Gaussian blur behind shadows.
//!
//! Built from basic arithmetic only - no `exp` from the platform's libm - so the
//! same picture comes out on every platform: the kernel weights are computed
//! with a fixed series ([`det_exp`]) and the convolution is integer arithmetic.

/// `exp(x)` for `x <= 0`, using only `+ - * /` and bit construction, so it
/// gives the same bits everywhere.
pub(crate) fn det_exp(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < -700.0 {
        return 0.0;
    }
    const LN2: f64 = std::f64::consts::LN_2;
    let k = (x / LN2).round();
    let r = x - k * LN2; // |r| <= ln2 / 2
    // Taylor series, 14 terms: error below 1e-17 for |r| <= 0.35.
    let mut term = 1.0;
    let mut sum = 1.0;
    let mut i = 1.0;
    while i <= 14.0 {
        term = term * r / i;
        sum += term;
        i += 1.0;
    }
    let ki = k as i64;
    // 2^ki, exactly, for the range that matters here.
    let pow2 = if (-1022..=1023).contains(&ki) { f64::from_bits(((ki + 1023) as u64) << 52) } else { 0.0 };
    sum * pow2
}

/// The Q16 weights (sum exactly 65536) of a Gaussian with the given sigma, for
/// taps `-radius..=radius`.
pub(crate) fn kernel_q16(sigma: f64) -> Vec<u32> {
    let radius = (3.0 * sigma).ceil().max(1.0) as usize;
    let two_s2 = 2.0 * sigma * sigma;
    let mut w: Vec<f64> = (0..=radius)
        .map(|i| det_exp(-((i * i) as f64) / two_s2))
        .collect();
    let total: f64 = w[0] + 2.0 * w[1..].iter().sum::<f64>();
    for v in &mut w {
        *v /= total;
    }
    let mut q: Vec<u32> = w.iter().map(|v| (v * 65536.0).round() as u32).collect();
    let sum: i64 = q[0] as i64 + 2 * q[1..].iter().map(|&v| v as i64).sum::<i64>();
    // Put the rounding error into the centre tap so the kernel sums to 1.
    q[0] = (q[0] as i64 + (65536 - sum)).max(0) as u32;
    let mut full = Vec::with_capacity(2 * radius + 1);
    for i in (1..=radius).rev() {
        full.push(q[i]);
    }
    full.push(q[0]);
    for v in q.iter().skip(1) {
        full.push(*v);
    }
    full
}

/// Blur an 8-bit alpha plane in place; pixels outside the plane count as 0.
pub(crate) fn blur_alpha(plane: &mut [u8], w: usize, h: usize, sigma: f64) {
    if sigma <= 0.0 || w == 0 || h == 0 || plane.len() != w * h {
        return;
    }
    let k = kernel_q16(sigma);
    let r = (k.len() / 2) as isize;
    // Horizontal pass into Q8 (value * 256).
    let mut tmp = vec![0u16; w * h];
    for y in 0..h {
        let row = &plane[y * w..(y + 1) * w];
        let out = &mut tmp[y * w..(y + 1) * w];
        for x in 0..w as isize {
            let mut acc: u32 = 0;
            let lo = (x - r).max(0);
            let hi = (x + r).min(w as isize - 1);
            for sx in lo..=hi {
                acc += k[(sx - x + r) as usize] * row[sx as usize] as u32;
            }
            // acc is Q16 of 0..255; keep 8 more fractional bits than the u8 result.
            out[x as usize] = ((acc + 128) >> 8) as u16;
        }
    }
    // Vertical pass back to 8 bits.
    for y in 0..h as isize {
        for x in 0..w {
            let mut acc: u64 = 0;
            let lo = (y - r).max(0);
            let hi = (y + r).min(h as isize - 1);
            for sy in lo..=hi {
                acc += k[(sy - y + r) as usize] as u64 * tmp[sy as usize * w + x] as u64;
            }
            // acc is Q16 * Q8 -> divide by 2^24, rounding.
            plane[y as usize * w + x] = ((acc + (1 << 23)) >> 24).min(255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn det_exp_matches_the_real_exp_closely() {
        for x in [0.0, -0.5, -1.0, -2.5, -10.0, -50.0, -300.0] {
            let a = det_exp(x);
            let b = x.exp();
            // The reduction by ln 2 loses a few ulps per unit of k; far below anything a kernel needs.
            assert!(((a - b) / b).abs() < 1e-12, "exp({x}): {a} vs {b}");
        }
        assert_eq!(det_exp(-1000.0), 0.0);
    }

    #[test]
    fn kernel_sums_to_one_and_is_symmetric() {
        for sigma in [0.5, 1.0, 2.5, 7.0, 20.0] {
            let k = kernel_q16(sigma);
            assert_eq!(k.iter().map(|&v| v as u64).sum::<u64>(), 65536, "sigma {sigma}");
            for i in 0..k.len() / 2 {
                assert_eq!(k[i], k[k.len() - 1 - i]);
            }
        }
    }

    #[test]
    fn blur_spreads_a_dot_and_keeps_energy_inside_the_plane() {
        let (w, h) = (41usize, 41usize);
        let mut p = vec![0u8; w * h];
        p[20 * w + 20] = 255;
        blur_alpha(&mut p, w, h, 3.0);
        let total: u32 = p.iter().map(|&v| v as u32).sum();
        assert!((200..=310).contains(&total), "energy {total} should stay near 255");
        assert!(p[20 * w + 20] > p[20 * w + 23]);
        assert!(p[20 * w + 23] > p[20 * w + 30]);
        assert_eq!(p[20 * w + 21], p[20 * w + 19], "symmetric");
        assert_eq!(p[21 * w + 20], p[19 * w + 20], "symmetric");
    }

    #[test]
    fn a_flat_plane_stays_flat_away_from_the_edges() {
        let (w, h) = (60usize, 60usize);
        let mut p = vec![200u8; w * h];
        blur_alpha(&mut p, w, h, 2.0);
        assert_eq!(p[30 * w + 30], 200);
    }

    #[test]
    fn zero_sigma_and_bad_sizes_do_nothing() {
        let mut p = vec![9u8; 4];
        blur_alpha(&mut p, 2, 2, 0.0);
        assert_eq!(p, vec![9; 4]);
        blur_alpha(&mut p, 3, 3, 1.0);
        assert_eq!(p, vec![9; 4]);
    }
}
