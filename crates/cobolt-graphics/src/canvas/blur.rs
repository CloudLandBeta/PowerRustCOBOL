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

/// Above this sigma the exact kernel would cost too much per pixel and three
/// box blurs (which approximate a Gaussian to within a few percent, the way
/// browsers and the CSS specification do it) take over.
pub(crate) const EXACT_KERNEL_MAX_SIGMA: f64 = 6.0;

/// The three box widths whose repeated blur approximates a Gaussian of `sigma`
/// ("ideal averaging filter widths", Kovesi); each is odd.
pub(crate) fn box_widths(sigma: f64) -> [usize; 3] {
    let n = 3.0f64;
    let ideal = (12.0 * sigma * sigma / n + 1.0).sqrt();
    let mut wl = ideal.floor() as i64;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wl = wl.max(1);
    let wu = wl + 2;
    let m = ((12.0 * sigma * sigma - n * (wl * wl) as f64 - 4.0 * n * wl as f64 - 3.0 * n) / (-4.0 * wl as f64 - 4.0)).round();
    let m = m.clamp(0.0, 3.0) as usize;
    let mut out = [wu as usize; 3];
    for slot in out.iter_mut().take(m) {
        *slot = wl as usize;
    }
    out
}

/// One box blur pass along rows (`stride` 1) or columns (`stride` = width), on values
/// scaled by 256, with a running sum: the cost per pixel does not depend on the width.
fn box_pass(src: &[u32], dst: &mut [u32], len: usize, lines: usize, line_stride: usize, step: usize, width: usize) {
    let r = width / 2;
    let div = width as u64;
    for line in 0..lines {
        let base = line * line_stride;
        let at = |i: usize| src[base + i * step] as u64;
        let mut sum: u64 = 0;
        for i in 0..=r.min(len.saturating_sub(1)) {
            sum += at(i);
        }
        for i in 0..len {
            dst[base + i * step] = ((sum + div / 2) / div) as u32;
            let add = i + r + 1;
            if add < len {
                sum += at(add);
            }
            if i >= r {
                sum -= at(i - r);
            }
        }
    }
}

fn blur_alpha_box3(plane: &mut [u8], w: usize, h: usize, sigma: f64) {
    let mut a: Vec<u32> = plane.iter().map(|&v| v as u32 * 256).collect();
    let mut b = vec![0u32; a.len()];
    for width in box_widths(sigma) {
        box_pass(&a, &mut b, w, h, w, 1, width);
        box_pass(&b, &mut a, h, w, 1, w, width);
    }
    for (o, v) in plane.iter_mut().zip(a.iter()) {
        *o = ((*v + 128) >> 8).min(255) as u8;
    }
}

/// Blur an 8-bit alpha plane in place; pixels outside the plane count as 0.
pub(crate) fn blur_alpha(plane: &mut [u8], w: usize, h: usize, sigma: f64) {
    if sigma <= 0.0 || w == 0 || h == 0 || plane.len() != w * h {
        return;
    }
    if sigma > EXACT_KERNEL_MAX_SIGMA {
        blur_alpha_box3(plane, w, h, sigma);
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
    fn large_sigmas_use_box_blurs_that_keep_the_energy_and_the_symmetry() {
        let (w, h) = (121usize, 121usize);
        let mut p = vec![0u8; w * h];
        for y in 58..63 {
            for x in 58..63 {
                p[y * w + x] = 255;
            }
        }
        let before: u32 = p.iter().map(|&v| v as u32).sum();
        blur_alpha(&mut p, w, h, 10.0);
        let after: u32 = p.iter().map(|&v| v as u32).sum();
        assert!((after as f64 / before as f64 - 1.0).abs() < 0.03, "energy {before} -> {after}");
        assert_eq!(p[60 * w + 50], p[60 * w + 70], "symmetric");
        assert_eq!(p[50 * w + 60], p[70 * w + 60], "symmetric");
        assert!(p[60 * w + 60] > p[60 * w + 75]);
    }

    #[test]
    fn the_box_approximation_tracks_the_exact_kernel() {
        let (w, h) = (101usize, 101usize);
        let mut dot = vec![0u8; w * h];
        for y in 45..56 {
            for x in 45..56 {
                dot[y * w + x] = 255;
            }
        }
        let mut exact = dot.clone();
        // Just under the switch-over, the exact kernel; just over it, the boxes.
        blur_alpha(&mut exact, w, h, 5.99);
        let mut boxed = dot.clone();
        blur_alpha(&mut boxed, w, h, 6.01);
        let worst = exact.iter().zip(&boxed).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
        assert!(worst <= 14, "the two methods agree to within {worst} levels at the switch-over (5 %)");
    }

    #[test]
    fn box_widths_are_odd_and_grow_with_sigma() {
        for s in [6.1, 10.0, 50.0, 256.0] {
            let w = box_widths(s);
            assert!(w.iter().all(|x| x % 2 == 1), "{w:?}");
        }
        assert!(box_widths(50.0)[0] > box_widths(10.0)[0]);
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
