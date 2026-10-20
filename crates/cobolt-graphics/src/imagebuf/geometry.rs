// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Resizing, rotating, flipping and compositing.

use fast_image_resize as fir;
use tiny_skia::{FilterQuality, PixmapPaint, Transform};

use crate::canvas::paint::sk_blend;
use crate::color::Rgba;
use crate::display_list::consts::BlendMode;
use crate::error::{GraphicsError, Result};
use crate::par;
use crate::pixels::PixelBuffer;

/// How pixels are resampled when an image changes size.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ResizeFilter {
    /// The nearest source pixel: blocky, exact for whole-number scales.
    Nearest,
    /// Linear interpolation.
    Bilinear,
    /// Catmull-Rom cubic: sharper than bilinear.
    CatmullRom,
    /// Lanczos with three lobes: the best for shrinking photographs.
    Lanczos,
}

impl ResizeFilter {
    /// From a name ("nearest", "bilinear", "catmull-rom", "lanczos"), case-insensitive.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
            "nearest" | "box" => Some(Self::Nearest),
            "bilinear" | "linear" => Some(Self::Bilinear),
            "catmullrom" | "cubic" | "bicubic" => Some(Self::CatmullRom),
            "lanczos" | "lanczos3" => Some(Self::Lanczos),
            _ => None,
        }
    }
}

/// Resize to `nw` x `nh`. Premultiplied alpha is handled inside the resampler, so
/// transparent edges do not bleed colour.
pub(crate) fn resize(src: &PixelBuffer, nw: u32, nh: u32, filter: ResizeFilter) -> Result<PixelBuffer> {
    if nw == 0 || nh == 0 {
        return Err(GraphicsError::invalid("the new size needs a width and a height of at least 1 pixel"));
    }
    let err = |e: &dyn std::fmt::Display| GraphicsError::invalid(format!("resize failed: {e}"));
    let view = fir::images::ImageRef::new(src.width(), src.height(), src.as_bytes(), fir::PixelType::U8x4).map_err(|e| err(&e))?;
    let mut dst = fir::images::Image::new(nw, nh, fir::PixelType::U8x4);
    let alg = match filter {
        ResizeFilter::Nearest => fir::ResizeAlg::Nearest,
        ResizeFilter::Bilinear => fir::ResizeAlg::Convolution(fir::FilterType::Bilinear),
        ResizeFilter::CatmullRom => fir::ResizeAlg::Convolution(fir::FilterType::CatmullRom),
        ResizeFilter::Lanczos => fir::ResizeAlg::Convolution(fir::FilterType::Lanczos3),
    };
    let opts = fir::ResizeOptions::new().resize_alg(alg);
    let mut resizer = fir::Resizer::new();
    resizer.resize(&view, &mut dst, &opts).map_err(|e| err(&e))?;
    PixelBuffer::from_rgba(nw, nh, dst.into_vec())
}

/// Mirror left-right (`horizontal`) or top-bottom.
pub(crate) fn flip(img: &mut PixelBuffer, horizontal: bool) {
    let (w, h) = (img.width() as usize, img.height() as usize);
    if horizontal {
        par::for_each_chunk_mut(img.as_bytes_mut(), w * 4 * 16, |_, rows| {
            for row in rows.chunks_exact_mut(w * 4) {
                for x in 0..w / 2 {
                    for c in 0..4 {
                        row.swap(x * 4 + c, (w - 1 - x) * 4 + c);
                    }
                }
            }
        });
    } else {
        let bytes = img.as_bytes_mut();
        for y in 0..h / 2 {
            let (top, bottom) = bytes.split_at_mut((h - 1 - y) * w * 4);
            top[y * w * 4..(y + 1) * w * 4].swap_with_slice(&mut bottom[..w * 4]);
        }
    }
}

/// Rotate a quarter turn clockwise `quarters` times (1, 2 or 3): exact, no resampling.
pub(crate) fn rotate_quarters(src: &PixelBuffer, quarters: u32) -> Result<PixelBuffer> {
    let (w, h) = (src.width() as usize, src.height() as usize);
    let q = quarters % 4;
    if q == 0 {
        return Ok(src.clone());
    }
    let (nw, nh) = if q % 2 == 1 { (h, w) } else { (w, h) };
    let mut out = vec![0u8; nw * nh * 4];
    let s = src.as_bytes();
    par::for_each_chunk_mut(&mut out, nw * 4 * 16, |job, rows| {
        for (ri, row) in rows.chunks_exact_mut(nw * 4).enumerate() {
            let y = job * 16 + ri;
            for x in 0..nw {
                // Where does output (x, y) come from?
                let (sx, sy) = match q {
                    1 => (y, h - 1 - x),         // 90 clockwise
                    2 => (w - 1 - x, h - 1 - y), // 180
                    _ => (w - 1 - y, x),         // 270 clockwise
                };
                row[x * 4..x * 4 + 4].copy_from_slice(&s[(sy * w + sx) * 4..(sy * w + sx) * 4 + 4]);
            }
        }
    });
    PixelBuffer::from_rgba(nw as u32, nh as u32, out)
}

/// Rotate by any angle (degrees, clockwise) into a canvas large enough to hold
/// the result, filling the corners with `background`; quarter turns are exact.
pub(crate) fn rotate(src: &PixelBuffer, degrees: f32, background: Rgba) -> Result<PixelBuffer> {
    if !degrees.is_finite() {
        return Err(GraphicsError::invalid("the angle must be a number"));
    }
    let d = (degrees as f64).rem_euclid(360.0);
    for (k, q) in [(0.0, 0u32), (90.0, 1), (180.0, 2), (270.0, 3)] {
        if (d - k).abs() < 1e-6 {
            return rotate_quarters(src, q);
        }
    }
    let (w, h) = (src.width(), src.height());
    // Work in premultiplied space so the interpolation does not mix in hidden colour.
    let pm = src.to_pixmap().ok_or_else(|| GraphicsError::invalid("the image is too large to rotate"))?;
    let rad = d.to_radians();
    let (s, c) = (rad.sin().abs(), rad.cos().abs());
    let nw = (w as f64 * c + h as f64 * s).ceil().max(1.0) as u32;
    let nh = (w as f64 * s + h as f64 * c).ceil().max(1.0) as u32;
    let mut out = tiny_skia::Pixmap::new(nw, nh).ok_or_else(|| GraphicsError::invalid("the rotated image is too large"))?;
    out.fill(background.to_skia());
    let ts = Transform::from_translate(-(w as f32) / 2.0, -(h as f32) / 2.0)
        .post_rotate(d as f32)
        .post_translate(nw as f32 / 2.0, nh as f32 / 2.0);
    // Fill the rotated rectangle with the image as a pattern: smooth edges against the background.
    let paint = tiny_skia::Paint {
        shader: tiny_skia::Pattern::new(pm.as_ref(), tiny_skia::SpreadMode::Pad, FilterQuality::Bilinear, 1.0, Transform::identity()),
        anti_alias: true,
        ..tiny_skia::Paint::default()
    };
    let rect = tiny_skia::Rect::from_xywh(0.0, 0.0, w as f32, h as f32).ok_or_else(|| GraphicsError::invalid("the image has no area"))?;
    out.fill_rect(rect, &paint, ts, None);
    Ok(PixelBuffer::from_pixmap(&out))
}

/// Draw `src` onto `dst` at `(x, y)` with a blend mode and an opacity (0 to 1).
pub(crate) fn composite(dst: &mut PixelBuffer, src: &PixelBuffer, x: i32, y: i32, mode: BlendMode, opacity: f32) -> Result<()> {
    let mut base = dst.to_pixmap().ok_or_else(|| GraphicsError::invalid("the image is too large to composite"))?;
    let top = src.to_pixmap().ok_or_else(|| GraphicsError::invalid("the image is too large to composite"))?;
    let opacity = if opacity.is_finite() { opacity.clamp(0.0, 1.0) } else { 1.0 };
    base.draw_pixmap(x, y, top.as_ref(), &PixmapPaint { opacity, blend_mode: sk_blend(mode), quality: FilterQuality::Nearest }, Transform::identity(), None);
    *dst = PixelBuffer::from_pixmap(&base);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: u32, h: u32) -> PixelBuffer {
        let mut b = PixelBuffer::new(w, h).unwrap();
        for y in 0..h {
            for x in 0..w {
                b.set(x as i64, y as i64, Rgba::new((x * 255 / w.max(1)) as u8, (y * 255 / h.max(1)) as u8, 128, 255));
            }
        }
        b
    }

    #[test]
    fn quarter_turns_move_pixels_where_they_should() {
        let mut b = PixelBuffer::new(3, 2).unwrap();
        b.set(0, 0, Rgba::rgb(1, 0, 0)); // top-left
        b.set(2, 0, Rgba::rgb(2, 0, 0)); // top-right
        b.set(0, 1, Rgba::rgb(3, 0, 0)); // bottom-left
        let r = rotate_quarters(&b, 1).unwrap();
        assert_eq!((r.width(), r.height()), (2, 3));
        assert_eq!(r.get(1, 0).unwrap().r, 1, "top-left goes to top-right");
        assert_eq!(r.get(1, 2).unwrap().r, 2, "top-right goes to bottom-right");
        assert_eq!(r.get(0, 0).unwrap().r, 3, "bottom-left goes to top-left");
        let r2 = rotate_quarters(&b, 2).unwrap();
        assert_eq!(r2.get(2, 1).unwrap().r, 1);
        assert_eq!(r2.get(0, 1).unwrap().r, 2);
        // Four quarter turns come back.
        let mut again = b.clone();
        for _ in 0..4 {
            again = rotate_quarters(&again, 1).unwrap();
        }
        assert_eq!(again, b);
        assert_eq!(rotate_quarters(&rotate_quarters(&b, 1).unwrap(), 3).unwrap(), b);
    }

    #[test]
    fn rotating_by_ninety_through_the_general_function_is_exact() {
        let b = gradient(7, 5);
        assert_eq!(rotate(&b, 90.0, Rgba::TRANSPARENT).unwrap(), rotate_quarters(&b, 1).unwrap());
        assert_eq!(rotate(&b, -90.0, Rgba::TRANSPARENT).unwrap(), rotate_quarters(&b, 3).unwrap());
        assert_eq!(rotate(&b, 360.0, Rgba::TRANSPARENT).unwrap(), b);
        assert_eq!(rotate(&b, 450.0, Rgba::TRANSPARENT).unwrap(), rotate_quarters(&b, 1).unwrap());
    }

    #[test]
    fn an_oblique_angle_grows_the_canvas_and_fills_the_corners() {
        let b = PixelBuffer::filled(20, 20, Rgba::rgb(255, 0, 0)).unwrap();
        let r = rotate(&b, 45.0, Rgba::rgb(0, 0, 255)).unwrap();
        assert!(r.width() >= 28 && r.width() <= 30, "{}", r.width());
        assert_eq!(r.get(0, 0), Some(Rgba::rgb(0, 0, 255)), "the corner shows the background");
        let mid = r.get((r.width() / 2) as i64, (r.height() / 2) as i64).unwrap();
        assert_eq!(mid, Rgba::rgb(255, 0, 0));
        assert!(rotate(&b, f32::NAN, Rgba::TRANSPARENT).is_err());
    }

    #[test]
    fn flips_are_involutions() {
        let b = gradient(9, 6);
        let mut h = b.clone();
        flip(&mut h, true);
        assert_eq!(h.get(0, 0), b.get(8, 0));
        flip(&mut h, true);
        assert_eq!(h, b);
        let mut v = b.clone();
        flip(&mut v, false);
        assert_eq!(v.get(0, 0), b.get(0, 5));
        flip(&mut v, false);
        assert_eq!(v, b);
    }

    #[test]
    fn resize_changes_the_size_and_keeps_a_flat_colour() {
        let b = PixelBuffer::filled(40, 30, Rgba::rgb(10, 20, 30)).unwrap();
        for f in [ResizeFilter::Nearest, ResizeFilter::Bilinear, ResizeFilter::CatmullRom, ResizeFilter::Lanczos] {
            let r = resize(&b, 17, 11, f).unwrap();
            assert_eq!((r.width(), r.height()), (17, 11));
            let p = r.get(8, 5).unwrap();
            assert!(p.r.abs_diff(10) <= 1 && p.g.abs_diff(20) <= 1 && p.b.abs_diff(30) <= 1 && p.a == 255, "{f:?}: {p:?}");
        }
        assert!(resize(&b, 0, 5, ResizeFilter::Lanczos).is_err());
    }

    #[test]
    fn nearest_doubling_repeats_pixels() {
        let b = gradient(4, 4);
        let r = resize(&b, 8, 8, ResizeFilter::Nearest).unwrap();
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(r.get(x, y), b.get(x / 2, y / 2), "({x},{y})");
            }
        }
    }

    #[test]
    fn transparent_edges_do_not_bleed_colour_when_resizing() {
        let mut b = PixelBuffer::new(8, 8).unwrap();
        for y in 0..8 {
            for x in 0..4 {
                b.set(x, y, Rgba::rgb(255, 255, 255));
            }
        }
        let r = resize(&b, 4, 4, ResizeFilter::Bilinear).unwrap();
        for y in 0..4 {
            let p = r.get(1, y).unwrap();
            if p.a > 0 {
                assert!(p.r >= 250, "the colour of a half-covered pixel stays white: {p:?}");
            }
        }
    }

    #[test]
    fn composite_blends_with_the_chosen_mode() {
        let mut base = PixelBuffer::filled(4, 4, Rgba::rgb(200, 100, 50)).unwrap();
        let top = PixelBuffer::filled(2, 2, Rgba::rgb(128, 128, 128)).unwrap();
        composite(&mut base, &top, 1, 1, BlendMode::Multiply, 1.0).unwrap();
        let p = base.get(1, 1).unwrap();
        assert!(p.r.abs_diff(100) <= 1 && p.g.abs_diff(50) <= 1, "{p:?}");
        assert_eq!(base.get(0, 0), Some(Rgba::rgb(200, 100, 50)));
        assert_eq!(base.get(3, 3), Some(Rgba::rgb(200, 100, 50)));
        // Opacity 0.5 of opaque white over black is mid grey.
        let mut b = PixelBuffer::filled(2, 2, Rgba::BLACK).unwrap();
        composite(&mut b, &PixelBuffer::filled(2, 2, Rgba::WHITE).unwrap(), 0, 0, BlendMode::SourceOver, 0.5).unwrap();
        assert!(b.get(0, 0).unwrap().r.abs_diff(128) <= 1);
        // Off the edge draws nothing and does not fail.
        composite(&mut b, &top, 100, 100, BlendMode::SourceOver, 1.0).unwrap();
    }

    #[test]
    fn filter_names_parse() {
        assert_eq!(ResizeFilter::from_name("Catmull-Rom"), Some(ResizeFilter::CatmullRom));
        assert_eq!(ResizeFilter::from_name("LANCZOS"), Some(ResizeFilter::Lanczos));
        assert_eq!(ResizeFilter::from_name("sinc"), None);
    }
}
