// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Colour filters, convolution, edge detection and morphology.
//!
//! Colour filters work on the straight RGB values and leave alpha alone. All of
//! them are integer or fixed-point per pixel, so the result is the same on every
//! platform and for any number of threads.

use imageproc::morphology::{grayscale_dilate, grayscale_erode, Mask};
use image::GrayImage;

use crate::detmath::{radians, sin_cos};
use crate::error::{GraphicsError, Result};
use crate::par;
use crate::pixels::PixelBuffer;

const ROWS: usize = 16;

fn each_pixel(img: &mut PixelBuffer, f: impl Fn(&mut [u8]) + Send + Sync) {
    let w = img.width() as usize;
    par::for_each_chunk_mut(img.as_bytes_mut(), w * 4 * ROWS, |_, rows| {
        for p in rows.chunks_exact_mut(4) {
            f(p);
        }
    });
}

fn lut(img: &mut PixelBuffer, table: &[u8; 256]) {
    each_pixel(img, |p| {
        p[0] = table[p[0] as usize];
        p[1] = table[p[1] as usize];
        p[2] = table[p[2] as usize];
    });
}

/// Rec. 601 luma of a straight RGB triple, in 0..=255.
pub(crate) fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((299 * r as u32 + 587 * g as u32 + 114 * b as u32 + 500) / 1000) as u8
}

/// Grey from the luma of each pixel.
pub(crate) fn grayscale(img: &mut PixelBuffer) {
    each_pixel(img, |p| {
        let y = luma(p[0], p[1], p[2]);
        p[0] = y;
        p[1] = y;
        p[2] = y;
    });
}

/// `255 - c` for each colour channel.
pub(crate) fn invert(img: &mut PixelBuffer) {
    let mut t = [0u8; 256];
    for (i, v) in t.iter_mut().enumerate() {
        *v = 255 - i as u8;
    }
    lut(img, &t);
}

/// Add `amount` percent of the full range (-100..=100) to every colour channel.
pub(crate) fn brightness(img: &mut PixelBuffer, amount: f32) {
    let a = amount.clamp(-100.0, 100.0);
    let add = (a * 255.0 / 100.0).round() as i32;
    let mut t = [0u8; 256];
    for (i, v) in t.iter_mut().enumerate() {
        *v = (i as i32 + add).clamp(0, 255) as u8;
    }
    lut(img, &t);
}

/// Contrast, -100 (flat grey) to 100 (extreme); 0 changes nothing.
pub(crate) fn contrast(img: &mut PixelBuffer, amount: f32) {
    let a = (amount.clamp(-100.0, 100.0) * 255.0 / 100.0).round() as i32;
    // The classic integer form: f = 259 (a + 255) / (255 (259 - a)) in Q8.
    let f_q8 = (259 * (a + 255) * 256) / (255 * (259 - a));
    let mut t = [0u8; 256];
    for (i, v) in t.iter_mut().enumerate() {
        let c = ((f_q8 * (i as i32 - 128) + 128) >> 8) + 128;
        *v = c.clamp(0, 255) as u8;
    }
    lut(img, &t);
}

/// Black and white by luma: at or above `level` (0..=255) is white.
pub(crate) fn threshold(img: &mut PixelBuffer, level: u8) {
    each_pixel(img, |p| {
        let v = if luma(p[0], p[1], p[2]) >= level { 255 } else { 0 };
        p[0] = v;
        p[1] = v;
        p[2] = v;
    });
}

fn apply_matrix(img: &mut PixelBuffer, m: [[f64; 3]; 3]) {
    // Q14 fixed point.
    let q: [[i32; 3]; 3] = m.map(|row| row.map(|v| (v * 16384.0).round() as i32));
    each_pixel(img, move |p| {
        let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
        for (i, row) in q.iter().enumerate() {
            let v = (row[0] * r + row[1] * g + row[2] * b + 8192) >> 14;
            p[i] = v.clamp(0, 255) as u8;
        }
    });
}

/// Rotate the hue by `degrees` (the CSS `hue-rotate` matrix).
pub(crate) fn hue(img: &mut PixelBuffer, degrees: f32) {
    if !degrees.is_finite() {
        return;
    }
    let (s, c) = sin_cos(radians(degrees as f64));
    apply_matrix(
        img,
        [
            [0.213 + c * 0.787 - s * 0.213, 0.715 - c * 0.715 - s * 0.715, 0.072 - c * 0.072 + s * 0.928],
            [0.213 - c * 0.213 + s * 0.143, 0.715 + c * 0.285 + s * 0.140, 0.072 - c * 0.072 - s * 0.283],
            [0.213 - c * 0.213 - s * 0.787, 0.715 - c * 0.715 + s * 0.715, 0.072 + c * 0.928 + s * 0.072],
        ],
    );
}

/// Saturation: 0 is grey, 1 unchanged, above 1 more vivid (the CSS `saturate` matrix).
pub(crate) fn saturation(img: &mut PixelBuffer, factor: f32) {
    if !factor.is_finite() {
        return;
    }
    let s = (factor as f64).clamp(0.0, 16.0);
    apply_matrix(
        img,
        [
            [0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s],
            [0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s],
            [0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s],
        ],
    );
}

/// Convolve the colour channels with an `n x n` kernel (`n` odd, 3 to 9), divide
/// by `divisor` and add `offset`. Edges repeat the border pixels; alpha is kept.
pub(crate) fn convolve(img: &mut PixelBuffer, kernel: &[f32], divisor: f32, offset: f32) -> Result<()> {
    let n = (kernel.len() as f64).sqrt().round() as usize;
    if n * n != kernel.len() || n % 2 == 0 || !(3..=9).contains(&n) {
        return Err(GraphicsError::invalid("a convolution kernel must be 3x3, 5x5, 7x7 or 9x9 (9, 25, 49 or 81 numbers)"));
    }
    if !divisor.is_finite() || divisor == 0.0 || !offset.is_finite() || kernel.iter().any(|v| !v.is_finite()) {
        return Err(GraphicsError::invalid("the divisor must be a non-zero number and every kernel entry a number"));
    }
    let (w, h) = (img.width() as usize, img.height() as usize);
    let src = img.as_bytes().to_vec();
    let r = (n / 2) as isize;
    par::for_each_chunk_mut(img.as_bytes_mut(), w * 4 * ROWS, |job, out| {
        let y0 = job * ROWS;
        for (ri, row_out) in out.chunks_exact_mut(w * 4).enumerate() {
            let y = (y0 + ri) as isize;
            for x in 0..w as isize {
                let mut acc = [0f32; 3];
                for ky in 0..n as isize {
                    let sy = (y + ky - r).clamp(0, h as isize - 1) as usize;
                    for kx in 0..n as isize {
                        let sx = (x + kx - r).clamp(0, w as isize - 1) as usize;
                        let wt = kernel[(ky as usize) * n + kx as usize];
                        let p = &src[(sy * w + sx) * 4..];
                        acc[0] += wt * p[0] as f32;
                        acc[1] += wt * p[1] as f32;
                        acc[2] += wt * p[2] as f32;
                    }
                }
                for c in 0..3 {
                    row_out[x as usize * 4 + c] = (acc[c] / divisor + offset).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    });
    Ok(())
}

/// Unsharp mask: add `amount` times the difference from a blurred copy (sigma 1).
pub(crate) fn sharpen(img: &mut PixelBuffer, amount: f32) {
    if !amount.is_finite() {
        return;
    }
    let amount = amount.clamp(0.0, 10.0);
    let mut blurred = img.clone();
    super::blur::gaussian(&mut blurred, 1.0);
    let q = (amount * 256.0).round() as i32;
    let w = img.width() as usize;
    let blur_bytes = blurred.into_bytes();
    par::for_each_chunk_mut(img.as_bytes_mut(), w * 4 * ROWS, |job, rows| {
        let base = job * ROWS * w * 4;
        for (i, p) in rows.chunks_exact_mut(4).enumerate() {
            let b = &blur_bytes[base + i * 4..base + i * 4 + 4];
            for c in 0..3 {
                let v = p[c] as i32 + ((q * (p[c] as i32 - b[c] as i32)) >> 8);
                p[c] = v.clamp(0, 255) as u8;
            }
        }
    });
}

fn channel(img: &PixelBuffer, c: usize) -> GrayImage {
    let (w, h) = (img.width(), img.height());
    let data: Vec<u8> = img.as_bytes().chunks_exact(4).map(|p| p[c]).collect();
    GrayImage::from_raw(w, h, data).expect("sizes match")
}

fn put_channel(img: &mut PixelBuffer, c: usize, g: &GrayImage) {
    for (p, v) in img.as_bytes_mut().chunks_exact_mut(4).zip(g.as_raw()) {
        p[c] = *v;
    }
}

/// Grayscale dilation (`erode == false`) or erosion of every channel with a square
/// window of the given radius (1 to 20).
pub(crate) fn morphology(img: &mut PixelBuffer, radius: u32, erode: bool) -> Result<()> {
    if !(1..=20).contains(&radius) {
        return Err(GraphicsError::invalid("the radius must be from 1 to 20"));
    }
    let mask = Mask::square(radius as u8);
    for c in 0..4 {
        let g = channel(img, c);
        let out = if erode { grayscale_erode(&g, &mask) } else { grayscale_dilate(&g, &mask) };
        put_channel(img, c, &out);
    }
    Ok(())
}

/// Edge strength: the Sobel gradient of the luma, scaled so that a full-contrast
/// step is 255, drawn as grey; alpha is kept.
pub(crate) fn edges(img: &mut PixelBuffer) {
    let (w, h) = (img.width(), img.height());
    let luma_bytes: Vec<u8> = img.as_bytes().chunks_exact(4).map(|p| luma(p[0], p[1], p[2])).collect();
    let g = GrayImage::from_raw(w, h, luma_bytes).expect("sizes match");
    let grad = imageproc::gradients::sobel_gradients(&g);
    for (p, v) in img.as_bytes_mut().chunks_exact_mut(4).zip(grad.as_raw()) {
        let m = (*v / 4).min(255) as u8;
        p[0] = m;
        p[1] = m;
        p[2] = m;
    }
}

/// Counts of each value 0..=255 per channel and of the luma.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Histogram {
    /// Red.
    pub red: [u32; 256],
    /// Green.
    pub green: [u32; 256],
    /// Blue.
    pub blue: [u32; 256],
    /// Alpha.
    pub alpha: [u32; 256],
    /// Rec. 601 luma.
    pub luma: [u32; 256],
}

/// Count the values.
pub(crate) fn histogram(img: &PixelBuffer) -> Histogram {
    let mut h = Histogram { red: [0; 256], green: [0; 256], blue: [0; 256], alpha: [0; 256], luma: [0; 256] };
    for p in img.as_bytes().chunks_exact(4) {
        h.red[p[0] as usize] += 1;
        h.green[p[1] as usize] += 1;
        h.blue[p[2] as usize] += 1;
        h.alpha[p[3] as usize] += 1;
        h.luma[luma(p[0], p[1], p[2]) as usize] += 1;
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba;

    fn px(c: Rgba) -> PixelBuffer {
        PixelBuffer::filled(2, 2, c).unwrap()
    }

    #[test]
    fn grayscale_uses_the_luma_weights() {
        let mut b = px(Rgba::new(255, 0, 0, 200));
        grayscale(&mut b);
        assert_eq!(b.get(0, 0), Some(Rgba::new(76, 76, 76, 200)));
        let mut b = px(Rgba::new(0, 255, 0, 255));
        grayscale(&mut b);
        assert_eq!(b.get(0, 0).unwrap().r, 150);
    }

    #[test]
    fn invert_is_its_own_inverse_and_keeps_alpha() {
        let mut b = px(Rgba::new(10, 100, 250, 33));
        invert(&mut b);
        assert_eq!(b.get(0, 0), Some(Rgba::new(245, 155, 5, 33)));
        invert(&mut b);
        assert_eq!(b.get(0, 0), Some(Rgba::new(10, 100, 250, 33)));
    }

    #[test]
    fn brightness_and_contrast_have_neutral_points() {
        let src = px(Rgba::new(10, 100, 250, 255));
        let mut b = src.clone();
        brightness(&mut b, 0.0);
        assert_eq!(b, src);
        let mut b = src.clone();
        contrast(&mut b, 0.0);
        assert_eq!(b, src);
        let mut b = src.clone();
        brightness(&mut b, 100.0);
        assert_eq!(b.get(0, 0), Some(Rgba::WHITE));
        let mut b = src.clone();
        brightness(&mut b, -100.0);
        assert_eq!(b.get(0, 0), Some(Rgba::BLACK));
        // Full contrast pushes 100 (below the middle) to 0 and 250 to 255.
        let mut b = src;
        contrast(&mut b, 100.0);
        let p = b.get(0, 0).unwrap();
        assert_eq!((p.r, p.g, p.b), (0, 0, 255));
    }

    #[test]
    fn threshold_makes_black_and_white() {
        let mut b = PixelBuffer::new(2, 1).unwrap();
        b.set(0, 0, Rgba::rgb(100, 100, 100));
        b.set(1, 0, Rgba::rgb(160, 160, 160));
        threshold(&mut b, 128);
        assert_eq!(b.get(0, 0).unwrap().r, 0);
        assert_eq!(b.get(1, 0).unwrap().r, 255);
    }

    #[test]
    fn hue_of_zero_degrees_and_saturation_of_one_change_almost_nothing() {
        let src = px(Rgba::new(200, 120, 40, 255));
        let mut b = src.clone();
        hue(&mut b, 0.0);
        saturation(&mut b, 1.0);
        let (p, q) = (src.get(0, 0).unwrap(), b.get(0, 0).unwrap());
        assert!(p.r.abs_diff(q.r) <= 1 && p.g.abs_diff(q.g) <= 1 && p.b.abs_diff(q.b) <= 1, "{p:?} {q:?}");
    }

    #[test]
    fn hue_rotates_red_towards_green_at_120_degrees() {
        let mut b = px(Rgba::rgb(255, 0, 0));
        hue(&mut b, 120.0);
        let p = b.get(0, 0).unwrap();
        assert!(p.g > p.r && p.g > p.b, "{p:?}");
        let mut b = px(Rgba::rgb(255, 0, 0));
        saturation(&mut b, 0.0);
        let p = b.get(0, 0).unwrap();
        assert!(p.r.abs_diff(p.g) <= 1 && p.g.abs_diff(p.b) <= 1, "fully desaturated is grey: {p:?}");
    }

    #[test]
    fn the_identity_kernel_changes_nothing_and_bad_kernels_are_refused() {
        let mut b = PixelBuffer::new(5, 5).unwrap();
        for y in 0..5 {
            for x in 0..5 {
                b.set(x, y, Rgba::rgb((x * 40) as u8, (y * 40) as u8, 99));
            }
        }
        let src = b.clone();
        convolve(&mut b, &[0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0], 1.0, 0.0).unwrap();
        assert_eq!(b, src);
        assert!(convolve(&mut b, &[1.0; 4], 1.0, 0.0).is_err());
        assert!(convolve(&mut b, &[1.0; 9], 0.0, 0.0).is_err());
        assert!(convolve(&mut b, &[f32::NAN; 9], 1.0, 0.0).is_err());
        assert!(convolve(&mut b, &vec![1.0; 121], 1.0, 0.0).is_err(), "11x11 is too large");
    }

    #[test]
    fn a_box_kernel_averages() {
        let mut b = PixelBuffer::new(3, 3).unwrap();
        b.set(1, 1, Rgba::rgb(90, 90, 90));
        convolve(&mut b, &[1.0; 9], 9.0, 0.0).unwrap();
        assert_eq!(b.get(1, 1).unwrap().r, 10);
        assert_eq!(b.get(0, 0).unwrap().r, 10);
    }

    #[test]
    fn dilate_grows_and_erode_shrinks() {
        let mut b = PixelBuffer::new(9, 9).unwrap();
        b.set(4, 4, Rgba::WHITE);
        let mut d = b.clone();
        morphology(&mut d, 1, false).unwrap();
        assert_eq!(d.get(3, 4).unwrap().r, 255);
        assert_eq!(d.get(5, 5).unwrap().r, 255);
        assert_eq!(d.get(6, 4).unwrap().r, 0);
        let mut e = d.clone();
        morphology(&mut e, 1, true).unwrap();
        assert_eq!(e.get(4, 4).unwrap().r, 255);
        assert_eq!(e.get(3, 4).unwrap().r, 0);
        assert!(morphology(&mut e, 0, true).is_err() && morphology(&mut e, 21, false).is_err());
    }

    #[test]
    fn edges_find_a_step() {
        let mut b = PixelBuffer::filled(16, 8, Rgba::BLACK).unwrap();
        for y in 0..8 {
            for x in 8..16 {
                b.set(x, y, Rgba::WHITE);
            }
        }
        edges(&mut b);
        assert!(b.get(8, 4).unwrap().r > 200 || b.get(7, 4).unwrap().r > 200, "the step is the edge");
        assert_eq!(b.get(2, 4).unwrap().r, 0);
        assert_eq!(b.get(13, 4).unwrap().r, 0);
    }

    #[test]
    fn sharpen_increases_contrast_at_an_edge() {
        let mut b = PixelBuffer::filled(16, 4, Rgba::rgb(100, 100, 100)).unwrap();
        for y in 0..4 {
            for x in 8..16 {
                b.set(x, y, Rgba::rgb(160, 160, 160));
            }
        }
        sharpen(&mut b, 1.0);
        assert!(b.get(7, 1).unwrap().r < 100, "overshoot on the dark side");
        assert!(b.get(8, 1).unwrap().r > 160, "overshoot on the light side");
        assert_eq!(b.get(0, 1).unwrap().r, 100);
    }

    #[test]
    fn the_histogram_counts_every_pixel_once_per_channel() {
        let mut b = PixelBuffer::new(4, 4).unwrap();
        b.set(0, 0, Rgba::new(255, 0, 0, 255));
        let h = histogram(&b);
        assert_eq!(h.red.iter().sum::<u32>(), 16);
        assert_eq!(h.red[255], 1);
        assert_eq!(h.red[0], 15);
        assert_eq!(h.alpha[255], 1);
        assert_eq!(h.alpha[0], 15);
        assert_eq!(h.luma[76], 1);
    }
}
