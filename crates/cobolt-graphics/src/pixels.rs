// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The pixel container shared by every part of the crate.
//!
//! [`PixelBuffer`] is `width x height` pixels of **straight** (not
//! premultiplied) RGBA, 8 bits per channel, row after row with no padding -
//! exactly the layout `GetPixel`/`SetPixel`, the codecs and `ToImage` speak.
//! The rasteriser works on premultiplied data; [`PixelBuffer::to_pixmap`] and
//! [`PixelBuffer::from_pixmap`] convert with the rasteriser's own rounding so
//! a round trip through a canvas is stable.

use crate::color::Rgba;
use crate::error::{GraphicsError, Result};

/// Straight-alpha RGBA8 pixels.
#[derive(Clone, PartialEq, Eq)]
pub struct PixelBuffer {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl std::fmt::Debug for PixelBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PixelBuffer({}x{})", self.width, self.height)
    }
}

impl PixelBuffer {
    /// A transparent buffer. Zero sizes, or a size whose byte count overflows,
    /// are an error (the *limits* of R15 are checked by the callers that have a
    /// [`Limits`](crate::limits::Limits) in hand).
    pub fn new(width: u32, height: u32) -> Result<Self> {
        let len = Self::byte_len(width, height)?;
        Ok(Self { width, height, data: vec![0; len] })
    }

    /// A buffer filled with `color`.
    pub fn filled(width: u32, height: u32, color: Rgba) -> Result<Self> {
        let mut b = Self::new(width, height)?;
        b.fill(color);
        Ok(b)
    }

    /// Take ownership of RGBA8 bytes; the length must be `width * height * 4`.
    pub fn from_rgba(width: u32, height: u32, data: Vec<u8>) -> Result<Self> {
        let len = Self::byte_len(width, height)?;
        if data.len() != len {
            return Err(GraphicsError::invalid(format!(
                "{} bytes given for a {width}x{height} image, which needs {len}",
                data.len()
            )));
        }
        Ok(Self { width, height, data })
    }

    fn byte_len(width: u32, height: u32) -> Result<usize> {
        if width == 0 || height == 0 {
            return Err(GraphicsError::invalid("an image needs a width and a height of at least 1 pixel"));
        }
        (width as usize)
            .checked_mul(height as usize)
            .and_then(|p| p.checked_mul(4))
            .filter(|&l| l <= isize::MAX as usize / 2)
            .ok_or_else(|| GraphicsError::invalid("the image is too large for this platform"))
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The bytes, `width * height * 4`, row-major, straight alpha.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// The bytes, mutably.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Give the bytes back.
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    /// The memory this buffer holds, for the registry's accounting.
    pub fn memory_bytes(&self) -> u64 {
        self.data.len() as u64
    }

    /// One pixel; `None` outside the image.
    pub fn get(&self, x: i64, y: i64) -> Option<Rgba> {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        Some(Rgba::new(self.data[i], self.data[i + 1], self.data[i + 2], self.data[i + 3]))
    }

    /// Set one pixel; returns `false` (and does nothing) outside the image.
    pub fn set(&mut self, x: i64, y: i64, c: Rgba) -> bool {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return false;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        self.data[i..i + 4].copy_from_slice(&[c.r, c.g, c.b, c.a]);
        true
    }

    /// Fill every pixel.
    pub fn fill(&mut self, c: Rgba) {
        for px in self.data.chunks_exact_mut(4) {
            px.copy_from_slice(&[c.r, c.g, c.b, c.a]);
        }
    }

    /// The rows `y0..y1` as a new buffer (clamped to the image; error if empty).
    pub fn crop(&self, x: i64, y: i64, w: u32, h: u32) -> Result<PixelBuffer> {
        let x0 = x.clamp(0, self.width as i64) as usize;
        let y0 = y.clamp(0, self.height as i64) as usize;
        let x1 = (x.saturating_add(w as i64)).clamp(0, self.width as i64) as usize;
        let y1 = (y.saturating_add(h as i64)).clamp(0, self.height as i64) as usize;
        if x1 <= x0 || y1 <= y0 {
            return Err(GraphicsError::invalid("the crop rectangle is empty or outside the image"));
        }
        let (nw, nh) = (x1 - x0, y1 - y0);
        let mut out = Vec::with_capacity(nw * nh * 4);
        for row in y0..y1 {
            let s = (row * self.width as usize + x0) * 4;
            out.extend_from_slice(&self.data[s..s + nw * 4]);
        }
        PixelBuffer::from_rgba(nw as u32, nh as u32, out)
    }

    /// As a premultiplied rasteriser pixmap (what the canvas draws from).
    pub fn to_pixmap(&self) -> Option<tiny_skia::Pixmap> {
        let mut pm = tiny_skia::Pixmap::new(self.width, self.height)?;
        for (dst, src) in pm.pixels_mut().iter_mut().zip(self.data.chunks_exact(4)) {
            *dst = tiny_skia::ColorU8::from_rgba(src[0], src[1], src[2], src[3]).premultiply();
        }
        Some(pm)
    }

    /// From a premultiplied pixmap (what `ToImage` returns).
    pub fn from_pixmap(pm: &tiny_skia::Pixmap) -> PixelBuffer {
        let mut data = Vec::with_capacity(pm.pixels().len() * 4);
        for p in pm.pixels() {
            let c = p.demultiply();
            data.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
        }
        PixelBuffer { width: pm.width(), height: pm.height(), data }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_validates() {
        assert!(PixelBuffer::new(0, 5).is_err());
        assert!(PixelBuffer::new(5, 0).is_err());
        assert!(PixelBuffer::from_rgba(2, 2, vec![0; 15]).is_err());
        assert!(PixelBuffer::from_rgba(2, 2, vec![0; 16]).is_ok());
        assert!(PixelBuffer::new(u32::MAX, u32::MAX).is_err());
    }

    #[test]
    fn get_and_set_respect_bounds() {
        let mut b = PixelBuffer::new(3, 2).unwrap();
        assert!(b.set(2, 1, Rgba::new(1, 2, 3, 4)));
        assert!(!b.set(3, 0, Rgba::WHITE));
        assert!(!b.set(-1, 0, Rgba::WHITE));
        assert_eq!(b.get(2, 1), Some(Rgba::new(1, 2, 3, 4)));
        assert_eq!(b.get(3, 1), None);
        assert_eq!(b.get(0, -1), None);
    }

    #[test]
    fn crop_clamps_and_rejects_empty() {
        let mut b = PixelBuffer::new(4, 4).unwrap();
        b.set(1, 1, Rgba::WHITE);
        let c = b.crop(1, 1, 2, 2).unwrap();
        assert_eq!((c.width(), c.height()), (2, 2));
        assert_eq!(c.get(0, 0), Some(Rgba::WHITE));
        let c = b.crop(-2, -2, 4, 4).unwrap();
        assert_eq!((c.width(), c.height()), (2, 2));
        assert!(b.crop(10, 10, 2, 2).is_err());
        assert!(b.crop(0, 0, 0, 2).is_err());
    }

    #[test]
    fn opaque_pixels_survive_the_premultiplied_round_trip() {
        let mut b = PixelBuffer::new(4, 4).unwrap();
        for y in 0..4 {
            for x in 0..4 {
                b.set(x, y, Rgba::new((x * 60) as u8, (y * 60) as u8, 77, 255));
            }
        }
        let back = PixelBuffer::from_pixmap(&b.to_pixmap().unwrap());
        assert_eq!(back, b);
    }
}
