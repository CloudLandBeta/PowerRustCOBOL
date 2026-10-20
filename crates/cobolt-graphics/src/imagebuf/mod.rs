// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! ImageBuffer (WASM-018 R11): pixels you can load, change, save and draw.
//!
//! An [`ImageBuffer`] is `width x height` pixels of straight RGBA. It is made
//! from nothing ([`ImageBuffer::create`]), from the bytes of a file
//! ([`ImageBuffer::load_from_bytes`]) or from a bundled asset
//! ([`ImageBuffer::load_asset`]), changed in place by the operations below, and
//! written back to a file format ([`ImageBuffer::encode`], [`ImageBuffer::save`]).
//! A canvas draws it with `DrawImage` (the canvas takes a share of the pixels, so
//! later changes to the buffer do not alter what was already drawn).
//!
//! # Operations
//!
//! | COBOL method | Rust | notes |
//! |---|---|---|
//! | `Create` | [`ImageBuffer::create`] | width, height, optional colour |
//! | `LoadAsset` | [`ImageBuffer::load_asset`] | an asset by name, through the host |
//! | `LoadFromBytes` | [`ImageBuffer::load_from_bytes`] | PNG, JPEG, GIF (first frame), WebP, BMP, QOI; EXIF orientation applied |
//! | `Encode` | [`ImageBuffer::encode`] | PNG, JPEG (quality), GIF, WebP (lossless), BMP, QOI |
//! | `Save` | [`ImageBuffer::save`] | encodes and hands the bytes to the host |
//! | `Resize` | [`ImageBuffer::resize`] | nearest, bilinear, Catmull-Rom, Lanczos |
//! | `Crop`, `Rotate`, `Flip` | [`ImageBuffer::crop`], [`ImageBuffer::rotate`], [`ImageBuffer::flip`] | quarter turns are exact |
//! | `Fill`, `GetPixel`, `SetPixel` | [`ImageBuffer::fill`], [`ImageBuffer::get_pixel`], [`ImageBuffer::set_pixel`] | |
//! | `Composite` | [`ImageBuffer::composite`] | any of the 26 blend modes, with opacity |
//! | `Grayscale`, `Invert`, `Brightness`, `Contrast`, `Hue`, `Saturation`, `Threshold` | the methods of those names | colour only, alpha kept |
//! | `BlurGaussian`, `BlurBox`, `Sharpen`, `EdgeDetect`, `Convolve` | the methods of those names | |
//! | `Dilate`, `Erode` | [`ImageBuffer::dilate`], [`ImageBuffer::erode`] | per channel, square window |
//! | `Histogram` | [`ImageBuffer::histogram`] | 256 counts for R, G, B, A and luma |
//!
//! # Safety and determinism
//!
//! Every size is checked against the [`Limits`] *before* memory is allocated, and a
//! file's own header is read first, so a "decompression bomb" is refused cheaply.
//! Only pure-Rust codecs are linked. The operations are integer or fixed-point
//! and split into independent rows, so **1 thread and many give the same bytes**.
//!
//! # Differences and limits
//!
//! * WebP is written **lossless** only (the pure-Rust encoder has no lossy mode).
//! * GIF is written as one frame; animated files read as their first frame.
//! * Memory: a resize, rotate or blur needs a second copy of the pixels.

mod blur;
mod codecs;
mod filters;
mod geometry;

use std::sync::Arc;

pub use codecs::{decode, encode, ImageFormat};
pub use filters::Histogram;
pub use geometry::ResizeFilter;

use crate::color::Rgba;
use crate::display_list::consts::BlendMode;
use crate::error::{GraphicsError, Result};
use crate::limits::Limits;
use crate::pixels::PixelBuffer;
use crate::resources::AssetHost;

/// Pixels with the operations of the COBOL `ImageBuffer` object.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageBuffer {
    px: Arc<PixelBuffer>,
}

impl ImageBuffer {
    /// A new `width x height` image filled with `color` (transparent black when you
    /// pass [`Rgba::TRANSPARENT`]).
    pub fn create(width: u32, height: u32, color: Rgba, limits: &Limits) -> Result<Self> {
        limits.check_image(width, height)?;
        Ok(Self { px: Arc::new(PixelBuffer::filled(width, height, color)?) })
    }

    /// Wrap existing pixels (they are not checked against any limit).
    pub fn from_pixels(px: PixelBuffer) -> Self {
        Self { px: Arc::new(px) }
    }

    /// Share pixels that already exist (no copy; a change to either copies first).
    pub fn from_shared(px: Arc<PixelBuffer>) -> Self {
        Self { px }
    }

    /// Decode a file's bytes. The size limit is checked from the header, before
    /// any pixel memory exists.
    pub fn load_from_bytes(bytes: &[u8], limits: &Limits) -> Result<Self> {
        Ok(Self::from_pixels(codecs::decode(bytes, limits)?))
    }

    /// Load a bundled asset by name through the host (the crate opens no file).
    pub fn load_asset(name: &str, assets: &dyn AssetHost, limits: &Limits) -> Result<Self> {
        let bytes = assets.asset(name).ok_or_else(|| GraphicsError::Decode(format!("there is no asset named \"{name}\"")))?;
        Self::load_from_bytes(&bytes, limits)
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.px.width()
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.px.height()
    }

    /// The pixels, shared: cheap to clone, and what a canvas draws from.
    pub fn pixels(&self) -> &Arc<PixelBuffer> {
        &self.px
    }

    /// Memory held, for the registry's accounting.
    pub fn memory_bytes(&self) -> u64 {
        self.px.memory_bytes()
    }

    fn make_mut(&mut self) -> &mut PixelBuffer {
        Arc::make_mut(&mut self.px)
    }

    // ---- files -----------------------------------------------------------

    /// Encode as a file format (`quality` 1 to 100 matters for JPEG only).
    pub fn encode(&self, format: ImageFormat, quality: u8) -> Result<Vec<u8>> {
        codecs::encode(&self.px, format, quality)
    }

    /// Encode and hand the bytes to the host under `name` (a file on the desktop, a
    /// download in the browser). Returns the number of bytes written.
    pub fn save(&self, name: &str, format: ImageFormat, quality: u8, assets: &dyn AssetHost) -> Result<usize> {
        let bytes = self.encode(format, quality)?;
        assets.save(name, &bytes).map_err(|e| GraphicsError::Encode(format!("the host could not save \"{name}\": {e}")))?;
        Ok(bytes.len())
    }

    // ---- single pixels ---------------------------------------------------

    /// One pixel; `None` outside the image.
    pub fn get_pixel(&self, x: i64, y: i64) -> Option<Rgba> {
        self.px.get(x, y)
    }

    /// Set one pixel; `false` (and nothing happens) outside the image.
    pub fn set_pixel(&mut self, x: i64, y: i64, color: Rgba) -> bool {
        if x < 0 || y < 0 || x >= self.width() as i64 || y >= self.height() as i64 {
            return false;
        }
        self.make_mut().set(x, y, color)
    }

    /// Fill every pixel.
    pub fn fill(&mut self, color: Rgba) {
        self.make_mut().fill(color);
    }

    // ---- geometry --------------------------------------------------------

    /// Change the size. The new size is checked against the limit first.
    pub fn resize(&mut self, width: u32, height: u32, filter: ResizeFilter, limits: &Limits) -> Result<()> {
        limits.check_image(width, height)?;
        self.px = Arc::new(geometry::resize(&self.px, width, height, filter)?);
        Ok(())
    }

    /// Keep the rectangle `(x, y, w, h)` (clamped to the image).
    pub fn crop(&mut self, x: i64, y: i64, w: u32, h: u32) -> Result<()> {
        self.px = Arc::new(self.px.crop(x, y, w, h)?);
        Ok(())
    }

    /// Rotate clockwise by `degrees`. Multiples of 90 are exact; other angles grow
    /// the canvas to hold the result and fill the corners with `background`.
    pub fn rotate(&mut self, degrees: f32, background: Rgba, limits: &Limits) -> Result<()> {
        // Check the size the rotation will need before allocating it.
        let rad = (degrees as f64).rem_euclid(360.0).to_radians();
        let (s, c) = (rad.sin().abs(), rad.cos().abs());
        let nw = (self.width() as f64 * c + self.height() as f64 * s).ceil().max(1.0);
        let nh = (self.width() as f64 * s + self.height() as f64 * c).ceil().max(1.0);
        if nw.is_finite() && nh.is_finite() && nw <= u32::MAX as f64 && nh <= u32::MAX as f64 {
            limits.check_image(nw as u32, nh as u32)?;
        }
        self.px = Arc::new(geometry::rotate(&self.px, degrees, background)?);
        Ok(())
    }

    /// Mirror left-right (`horizontal`) or top-bottom.
    pub fn flip(&mut self, horizontal: bool) {
        geometry::flip(self.make_mut(), horizontal);
    }

    /// Draw `other` onto this image at `(x, y)` with a blend mode and an opacity (0 to 1).
    pub fn composite(&mut self, other: &ImageBuffer, x: i32, y: i32, mode: BlendMode, opacity: f32) -> Result<()> {
        geometry::composite(self.make_mut(), &other.px, x, y, mode, opacity)
    }

    // ---- colour ----------------------------------------------------------

    /// Grey by luma.
    pub fn grayscale(&mut self) {
        filters::grayscale(self.make_mut());
    }

    /// Negative.
    pub fn invert(&mut self) {
        filters::invert(self.make_mut());
    }

    /// Add `amount` percent (-100 to 100) of the full range.
    pub fn brightness(&mut self, amount: f32) {
        filters::brightness(self.make_mut(), amount);
    }

    /// Contrast from -100 to 100 (0 changes nothing).
    pub fn contrast(&mut self, amount: f32) {
        filters::contrast(self.make_mut(), amount);
    }

    /// Rotate the hue by `degrees`.
    pub fn hue(&mut self, degrees: f32) {
        filters::hue(self.make_mut(), degrees);
    }

    /// Saturation: 0 grey, 1 unchanged, above 1 more vivid.
    pub fn saturation(&mut self, factor: f32) {
        filters::saturation(self.make_mut(), factor);
    }

    /// Black and white: luma at or above `level` (0 to 255) becomes white.
    pub fn threshold(&mut self, level: u8) {
        filters::threshold(self.make_mut(), level);
    }

    // ---- neighbourhood ---------------------------------------------------

    /// Gaussian blur, `sigma` in pixels.
    pub fn blur_gaussian(&mut self, sigma: f32) {
        blur::gaussian(self.make_mut(), sigma);
    }

    /// Box blur, window `2 * radius + 1`.
    pub fn blur_box(&mut self, radius: u32) {
        blur::boxed(self.make_mut(), radius);
    }

    /// Unsharp mask, `amount` 0 to 10.
    pub fn sharpen(&mut self, amount: f32) {
        filters::sharpen(self.make_mut(), amount);
    }

    /// Edge strength (Sobel on the luma) drawn as grey.
    pub fn edge_detect(&mut self) {
        filters::edges(self.make_mut());
    }

    /// Convolve the colour channels with a 3x3, 5x5, 7x7 or 9x9 kernel (row by
    /// row), divide by `divisor`, add `offset`.
    pub fn convolve(&mut self, kernel: &[f32], divisor: f32, offset: f32) -> Result<()> {
        filters::convolve(self.make_mut(), kernel, divisor, offset)
    }

    /// Dilate every channel with a square window of `radius` (1 to 20).
    pub fn dilate(&mut self, radius: u32) -> Result<()> {
        filters::morphology(self.make_mut(), radius, false)
    }

    /// Erode every channel with a square window of `radius` (1 to 20).
    pub fn erode(&mut self, radius: u32) -> Result<()> {
        filters::morphology(self.make_mut(), radius, true)
    }

    /// 256 counts for each of R, G, B, A and luma.
    pub fn histogram(&self) -> Histogram {
        filters::histogram(&self.px)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::MapAssets;

    #[test]
    fn create_checks_the_limit() {
        let lim = Limits { max_image_pixels: 100, ..Limits::default() };
        assert!(ImageBuffer::create(10, 10, Rgba::WHITE, &lim).is_ok());
        let e = ImageBuffer::create(11, 10, Rgba::WHITE, &lim).unwrap_err();
        assert!(e.to_string().contains("max_image_pixels"), "{e}");
    }

    #[test]
    fn a_shared_buffer_is_copied_on_write() {
        let mut a = ImageBuffer::create(4, 4, Rgba::BLACK, &Limits::default()).unwrap();
        let snapshot = a.pixels().clone();
        a.set_pixel(1, 1, Rgba::WHITE);
        assert_eq!(snapshot.get(1, 1), Some(Rgba::BLACK), "what a canvas already took does not change");
        assert_eq!(a.get_pixel(1, 1), Some(Rgba::WHITE));
        assert!(!a.set_pixel(4, 0, Rgba::WHITE) && !a.set_pixel(-1, 0, Rgba::WHITE));
    }

    #[test]
    fn assets_are_loaded_and_saved_through_the_host() {
        let src = ImageBuffer::create(5, 3, Rgba::rgb(1, 2, 3), &Limits::default()).unwrap();
        let png = src.encode(ImageFormat::Png, 0).unwrap();
        let host = MapAssets::new().with_asset("logo.png", png);
        let loaded = ImageBuffer::load_asset("logo.png", &host, &Limits::default()).unwrap();
        assert_eq!(loaded, src);
        let e = ImageBuffer::load_asset("nope.png", &host, &Limits::default()).unwrap_err();
        assert!(e.to_string().contains("nope.png"), "{e}");
        let n = loaded.save("out.webp", ImageFormat::WebP, 0, &host).unwrap();
        let saved = host.saved();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].0, "out.webp");
        assert_eq!(saved[0].1.len(), n);
    }

    #[test]
    fn resize_rotate_crop_chain() {
        let mut b = ImageBuffer::create(40, 20, Rgba::rgb(9, 9, 9), &Limits::default()).unwrap();
        b.resize(20, 10, ResizeFilter::Lanczos, &Limits::default()).unwrap();
        b.rotate(90.0, Rgba::TRANSPARENT, &Limits::default()).unwrap();
        assert_eq!((b.width(), b.height()), (10, 20));
        b.crop(2, 2, 5, 5).unwrap();
        assert_eq!((b.width(), b.height()), (5, 5));
        assert!(b.crop(100, 100, 5, 5).is_err());
        let lim = Limits { max_image_pixels: 50, ..Limits::default() };
        assert!(b.resize(10, 10, ResizeFilter::Nearest, &lim).is_err());
        assert!(b.rotate(45.0, Rgba::TRANSPARENT, &Limits { max_image_pixels: 30, ..Limits::default() }).is_err());
    }
}
