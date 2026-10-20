// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Blurs of RGBA images: the Gaussian and the box blur.
//!
//! Everything is integer arithmetic on **premultiplied** pixels (so a blurred edge
//! does not pick up the colour hidden under a transparent pixel), with the work
//! split into rows or bands of rows. The sum for an output pixel is exact, so the
//! result does not depend on how the rows are shared between threads - 1 thread
//! and 24 threads write the same bytes. Beyond the image the border pixels repeat,
//! so an opaque photograph stays opaque at its edges.
//!
//! * `sigma <= 6`: the exact Gaussian, a Q16 kernel of up to 37 taps.
//! * larger sigma: three box blurs (the same approximation browsers use), which cost
//!   the same per pixel whatever the radius - a 24 megapixel image blurs in
//!   a fraction of a second.

use crate::canvas::blur::{box_widths, kernel_q16, EXACT_KERNEL_MAX_SIGMA};
use crate::par;
use crate::pixels::PixelBuffer;

/// Rows per parallel job for row-wise passes.
const ROWS_PER_JOB: usize = 16;

fn premultiply(px: &mut [u8]) {
    par::for_each_chunk_mut(px, 4 * 4096, |_, chunk| {
        for p in chunk.chunks_exact_mut(4) {
            let a = p[3] as u32;
            if a != 255 {
                for c in &mut p[..3] {
                    *c = ((*c as u32 * a + 127) / 255) as u8;
                }
            }
        }
    });
}

fn unpremultiply(px: &mut [u8]) {
    par::for_each_chunk_mut(px, 4 * 4096, |_, chunk| {
        for p in chunk.chunks_exact_mut(4) {
            let a = p[3] as u32;
            if a == 0 {
                p[0] = 0;
                p[1] = 0;
                p[2] = 0;
            } else if a != 255 {
                for c in &mut p[..3] {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
    });
}

/// Horizontal box pass, one row at a time with a running sum over the window
/// `[x - r, x + r]`, indexes clamped to the row.
fn h_box(src: &[u8], dst: &mut [u8], w: usize, width: usize) {
    let r = width / 2;
    let div = width as u32;
    par::for_each_chunk_mut(dst, w * 4 * ROWS_PER_JOB, |job, out| {
        let first_row = job * ROWS_PER_JOB;
        for (ri, row_out) in out.chunks_exact_mut(w * 4).enumerate() {
            let row = &src[(first_row + ri) * w * 4..(first_row + ri + 1) * w * 4];
            let at = |i: isize, c: usize| row[(i.clamp(0, w as isize - 1) as usize) * 4 + c] as u32;
            let mut sum = [0u32; 4];
            for i in -(r as isize)..=(r as isize) {
                for c in 0..4 {
                    sum[c] += at(i, c);
                }
            }
            for x in 0..w as isize {
                for c in 0..4 {
                    row_out[x as usize * 4 + c] = ((sum[c] + div / 2) / div) as u8;
                }
                for c in 0..4 {
                    sum[c] += at(x + r as isize + 1, c);
                    sum[c] -= at(x - r as isize, c);
                }
            }
        }
    });
}

/// Vertical box pass: bands of rows, each starting its running sums from scratch,
/// so every output row is the exact sum of its window whatever the band size.
fn v_box(src: &[u8], dst: &mut [u8], w: usize, h: usize, width: usize) {
    let r = width / 2;
    let div = width as u32;
    let band = (4 * r).max(128);
    let row_of = |y: isize| {
        let y = y.clamp(0, h as isize - 1) as usize;
        &src[y * w * 4..(y + 1) * w * 4]
    };
    par::for_each_chunk_mut(dst, w * 4 * band, |job, out| {
        let y0 = (job * band) as isize;
        let rows = out.len() / (w * 4);
        let mut acc = vec![0u32; w * 4];
        for sy in (y0 - r as isize)..=(y0 + r as isize) {
            for (a, v) in acc.iter_mut().zip(row_of(sy)) {
                *a += *v as u32;
            }
        }
        for yi in 0..rows {
            let y = y0 + yi as isize;
            let row_out = &mut out[yi * w * 4..(yi + 1) * w * 4];
            for (o, a) in row_out.iter_mut().zip(&acc) {
                *o = ((*a + div / 2) / div) as u8;
            }
            for (a, v) in acc.iter_mut().zip(row_of(y + r as isize + 1)) {
                *a += *v as u32;
            }
            for (a, v) in acc.iter_mut().zip(row_of(y - r as isize)) {
                *a -= *v as u32;
            }
        }
    });
}

/// Horizontal pass with an explicit kernel (Q16, odd length).
fn h_kernel(src: &[u8], dst: &mut [u8], w: usize, k: &[u32]) {
    let r = (k.len() / 2) as isize;
    par::for_each_chunk_mut(dst, w * 4 * ROWS_PER_JOB, |job, out| {
        let first_row = job * ROWS_PER_JOB;
        for (ri, row_out) in out.chunks_exact_mut(w * 4).enumerate() {
            let row = &src[(first_row + ri) * w * 4..(first_row + ri + 1) * w * 4];
            for x in 0..w as isize {
                let mut acc = [0u32; 4];
                for (ki, wt) in k.iter().enumerate() {
                    let sx = (x + ki as isize - r).clamp(0, w as isize - 1) as usize;
                    for c in 0..4 {
                        acc[c] += wt * row[sx * 4 + c] as u32;
                    }
                }
                for c in 0..4 {
                    row_out[x as usize * 4 + c] = ((acc[c] + 32768) >> 16).min(255) as u8;
                }
            }
        }
    });
}

/// Vertical pass with an explicit kernel, in bands of rows.
fn v_kernel(src: &[u8], dst: &mut [u8], w: usize, h: usize, k: &[u32]) {
    let r = (k.len() / 2) as isize;
    par::for_each_chunk_mut(dst, w * 4 * 32, |job, out| {
        let y0 = job * 32;
        let rows = out.len() / (w * 4);
        let mut acc = vec![0u32; w * 4];
        for yi in 0..rows {
            let y = (y0 + yi) as isize;
            acc.iter_mut().for_each(|a| *a = 0);
            for (ki, wt) in k.iter().enumerate() {
                let sy = (y + ki as isize - r).clamp(0, h as isize - 1) as usize;
                let row = &src[sy * w * 4..(sy + 1) * w * 4];
                for (a, v) in acc.iter_mut().zip(row) {
                    *a += wt * *v as u32;
                }
            }
            let row_out = &mut out[yi * w * 4..(yi + 1) * w * 4];
            for (o, a) in row_out.iter_mut().zip(&acc) {
                *o = ((*a + 32768) >> 16).min(255) as u8;
            }
        }
    });
}

/// Gaussian blur with the given standard deviation in pixels.
pub(crate) fn gaussian(img: &mut PixelBuffer, sigma: f32) {
    let (w, h) = (img.width() as usize, img.height() as usize);
    if !(sigma.is_finite() && sigma > 0.0) {
        return;
    }
    let sigma = (sigma as f64).min(1000.0);
    premultiply(img.as_bytes_mut());
    let mut tmp = vec![0u8; img.as_bytes().len()];
    if sigma <= EXACT_KERNEL_MAX_SIGMA {
        let k = kernel_q16(sigma);
        h_kernel(img.as_bytes(), &mut tmp, w, &k);
        v_kernel(&tmp, img.as_bytes_mut(), w, h, &k);
    } else {
        for width in box_widths(sigma) {
            h_box(img.as_bytes(), &mut tmp, w, width);
            v_box(&tmp, img.as_bytes_mut(), w, h, width);
        }
    }
    unpremultiply(img.as_bytes_mut());
}

/// Box blur of the given radius (window `2 * radius + 1`), one pass each way.
pub(crate) fn boxed(img: &mut PixelBuffer, radius: u32) {
    let (w, h) = (img.width() as usize, img.height() as usize);
    if radius == 0 {
        return;
    }
    let width = (radius as usize).min(w.max(h)) * 2 + 1;
    premultiply(img.as_bytes_mut());
    let mut tmp = vec![0u8; img.as_bytes().len()];
    h_box(img.as_bytes(), &mut tmp, w, width);
    v_box(&tmp, img.as_bytes_mut(), w, h, width);
    unpremultiply(img.as_bytes_mut());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba;

    /// A 7 x 7 white square in the middle (enough light to survive a wide blur at 8 bits).
    fn dot(w: u32, h: u32) -> PixelBuffer {
        let mut b = PixelBuffer::new(w, h).unwrap();
        for dy in -3i64..=3 {
            for dx in -3i64..=3 {
                b.set((w / 2) as i64 + dx, (h / 2) as i64 + dy, Rgba::WHITE);
            }
        }
        b
    }

    #[test]
    fn a_blurred_dot_spreads_symmetrically_and_keeps_its_light() {
        for sigma in [1.5f32, 4.0, 9.0] {
            let mut b = dot(61, 61);
            gaussian(&mut b, sigma);
            let a = |x: i64, y: i64| b.get(x, y).unwrap().a as i64;
            // A 3 x sigma + 1 pixel step from the centre, both ways.
            let d = (sigma as i64) + 1;
            assert_eq!(a(30 + d, 30), a(30 - d, 30), "sigma {sigma}");
            assert_eq!(a(30, 30 + d), a(30, 30 - d), "sigma {sigma}");
            assert!(a(30, 30) > a(30, 30 + d) && a(30, 30 + d) > a(30, 30 + 2 * d), "sigma {sigma}: {} {} {}", a(30, 30), a(30, 30 + d), a(30, 30 + 2 * d));
            let total: i64 = b.as_bytes().chunks_exact(4).map(|p| p[3] as i64).sum();
            assert!((total - 49 * 255).abs() < 49 * 255 / 10, "sigma {sigma}: total alpha {total}");
        }
    }

    #[test]
    fn a_flat_image_stays_flat_edges_included_and_zero_sigma_does_nothing() {
        for sigma in [3.0f32, 20.0] {
            let mut b = PixelBuffer::filled(40, 30, Rgba::rgb(10, 200, 99)).unwrap();
            gaussian(&mut b, sigma);
            for (x, y) in [(20, 15), (0, 0), (39, 29), (0, 29), (39, 0)] {
                assert_eq!(b.get(x, y), Some(Rgba::rgb(10, 200, 99)), "sigma {sigma} at ({x},{y}): an opaque image stays opaque at its border");
            }
        }
        let mut b = PixelBuffer::filled(40, 30, Rgba::rgb(10, 200, 99)).unwrap();
        let before = b.clone();
        gaussian(&mut b, 0.0);
        gaussian(&mut b, f32::NAN);
        assert_eq!(b, before);
    }

    #[test]
    fn transparent_pixels_do_not_darken_the_edge() {
        let mut b = PixelBuffer::new(40, 10).unwrap();
        for y in 0..10 {
            for x in 0..20 {
                b.set(x, y, Rgba::rgb(255, 255, 255));
            }
        }
        gaussian(&mut b, 3.0);
        // Next to the edge the colour is still white; only the alpha fades.
        let p = b.get(21, 5).unwrap();
        assert!(p.a > 0 && p.a < 255);
        assert!(p.r >= 250 && p.g >= 250, "{p:?}");
    }

    #[test]
    fn the_box_blur_averages_over_the_window() {
        let mut b = PixelBuffer::new(9, 9).unwrap();
        b.set(4, 4, Rgba::new(255, 255, 255, 255));
        boxed(&mut b, 1);
        // Premultiplied alpha spreads over the 3 x 3 window: 255 / 9 = 28 each.
        for (x, y) in [(3, 3), (4, 4), (5, 5), (3, 5)] {
            assert_eq!(b.get(x, y).unwrap().a, 28, "({x},{y})");
        }
        assert_eq!(b.get(6, 4).unwrap().a, 0);
        assert_eq!(b.get(4, 6).unwrap().a, 0);
    }

    #[test]
    fn one_thread_and_many_write_the_same_bytes() {
        let mut src = PixelBuffer::new(97, 83).unwrap();
        for y in 0..83 {
            for x in 0..97 {
                src.set(x, y, Rgba::new((x * 5) as u8, (y * 3) as u8, ((x ^ y) * 9) as u8, (50 + (x * y) % 200) as u8));
            }
        }
        for sigma in [2.0f32, 7.5, 30.0] {
            let run = || {
                let mut b = src.clone();
                gaussian(&mut b, sigma);
                b.into_bytes()
            };
            let one = par::with_threads(1, run);
            let many = par::with_threads(8, run);
            assert!(one == many, "sigma {sigma}: threads changed the result");
        }
    }
}
