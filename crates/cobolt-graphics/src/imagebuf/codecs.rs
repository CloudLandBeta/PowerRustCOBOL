// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Reading and writing image files, through the `image` crate's pure-Rust
//! codecs only (PNG, JPEG, GIF, WebP, BMP, QOI).
//!
//! Decoding is defensive: the file's own header gives the dimensions, and the
//! pixel limit is checked **before** any pixel memory is allocated; the decoder
//! is told the same limit; and a damaged file is an `Err`, never a panic.

use std::io::Cursor;

use image::{ImageDecoder, ImageReader};

use crate::error::{GraphicsError, Result};
use crate::limits::Limits;
use crate::pixels::PixelBuffer;

/// A file format ImageBuffer can write (and read).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    /// PNG, lossless, with alpha.
    Png,
    /// JPEG, lossy; `quality` 1 to 100; no alpha (composited over white).
    Jpeg,
    /// GIF, one frame, 256 colours.
    Gif,
    /// WebP, **lossless** encoding (the pure-Rust encoder has no lossy mode); decodes lossy and lossless.
    WebP,
    /// BMP, uncompressed, 32 bits with alpha.
    Bmp,
    /// QOI, lossless and very fast.
    Qoi,
}

impl ImageFormat {
    /// Every format.
    pub const ALL: [ImageFormat; 6] = [Self::Png, Self::Jpeg, Self::Gif, Self::WebP, Self::Bmp, Self::Qoi];

    /// The usual file extension, lower case.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Gif => "gif",
            Self::WebP => "webp",
            Self::Bmp => "bmp",
            Self::Qoi => "qoi",
        }
    }

    /// From a name or extension ("PNG", ".jpg", "jpeg", "webp" ...), case-insensitive.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().trim_start_matches('.').to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "gif" => Some(Self::Gif),
            "webp" => Some(Self::WebP),
            "bmp" => Some(Self::Bmp),
            "qoi" => Some(Self::Qoi),
            _ => None,
        }
    }

    fn to_image(self) -> image::ImageFormat {
        match self {
            Self::Png => image::ImageFormat::Png,
            Self::Jpeg => image::ImageFormat::Jpeg,
            Self::Gif => image::ImageFormat::Gif,
            Self::WebP => image::ImageFormat::WebP,
            Self::Bmp => image::ImageFormat::Bmp,
            Self::Qoi => image::ImageFormat::Qoi,
        }
    }
}

fn dec(e: impl std::fmt::Display) -> GraphicsError {
    GraphicsError::Decode(e.to_string())
}

/// Decode an image file (the format is recognised from its first bytes).
pub fn decode(bytes: &[u8], limits: &Limits) -> Result<PixelBuffer> {
    limits.check_encoded(bytes.len())?;
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(dec)?;
    match reader.format() {
        Some(f) if ImageFormat::ALL.iter().any(|k| k.to_image() == f) => {}
        Some(f) => return Err(GraphicsError::Decode(format!("the {f:?} format is not supported (PNG, JPEG, GIF, WebP, BMP and QOI are)"))),
        None => return Err(GraphicsError::Decode("the data is not a recognised image file".into())),
    }
    let mut decoder = reader.into_decoder().map_err(dec)?;
    let (w, h) = decoder.dimensions();
    // Refuse before allocating a single pixel.
    limits.check_image(w, h)?;
    let mut il = image::Limits::default();
    il.max_image_width = Some(w);
    il.max_image_height = Some(h);
    il.max_alloc = Some((w as u64 * h as u64).saturating_mul(16).saturating_add(64 * 1024 * 1024));
    decoder.set_limits(il).map_err(dec)?;
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = image::DynamicImage::from_decoder(decoder).map_err(dec)?;
    img.apply_orientation(orientation);
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    PixelBuffer::from_rgba(w, h, rgba.into_raw())
}

/// Encode to a file format. `quality` (1-100) matters for JPEG only.
pub fn encode(px: &PixelBuffer, format: ImageFormat, quality: u8) -> Result<Vec<u8>> {
    use image::codecs::{bmp::BmpEncoder, gif::GifEncoder, jpeg::JpegEncoder, png::PngEncoder, qoi::QoiEncoder, webp::WebPEncoder};
    use image::ExtendedColorType as Ct;
    use image::ImageEncoder;
    let enc = |e: image::ImageError| GraphicsError::Encode(e.to_string());
    let (w, h) = (px.width(), px.height());
    let mut out = Vec::new();
    match format {
        ImageFormat::Png => PngEncoder::new(&mut out).write_image(px.as_bytes(), w, h, Ct::Rgba8).map_err(enc)?,
        ImageFormat::Jpeg => {
            // JPEG has no alpha: composite over white.
            let mut rgb = Vec::with_capacity(w as usize * h as usize * 3);
            for p in px.as_bytes().chunks_exact(4) {
                let a = p[3] as u32;
                for c in &p[..3] {
                    rgb.push(((*c as u32 * a + 255 * (255 - a) + 127) / 255) as u8);
                }
            }
            if w > 65535 || h > 65535 {
                return Err(GraphicsError::Encode("JPEG images are at most 65535 pixels on a side".into()));
            }
            JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100)).write_image(&rgb, w, h, Ct::Rgb8).map_err(enc)?
        }
        ImageFormat::Gif => {
            let mut enc_gif = GifEncoder::new_with_speed(&mut out, 10);
            let frame = image::Frame::new(image::RgbaImage::from_raw(w, h, px.as_bytes().to_vec()).ok_or_else(|| GraphicsError::Encode("bad buffer".into()))?);
            enc_gif.encode_frame(frame).map_err(enc)?;
        }
        ImageFormat::WebP => WebPEncoder::new_lossless(&mut out).write_image(px.as_bytes(), w, h, Ct::Rgba8).map_err(enc)?,
        ImageFormat::Bmp => BmpEncoder::new(&mut out).write_image(px.as_bytes(), w, h, Ct::Rgba8).map_err(enc)?,
        ImageFormat::Qoi => QoiEncoder::new(&mut out).write_image(px.as_bytes(), w, h, Ct::Rgba8).map_err(enc)?,
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba;

    fn sample() -> PixelBuffer {
        let mut b = PixelBuffer::new(37, 23).unwrap();
        for y in 0..23 {
            for x in 0..37 {
                b.set(x, y, Rgba::new((x * 7) as u8, (y * 11) as u8, ((x * y) % 251) as u8, 255));
            }
        }
        b
    }

    #[test]
    fn lossless_formats_round_trip_exactly() {
        let src = sample();
        for f in [ImageFormat::Png, ImageFormat::WebP, ImageFormat::Bmp, ImageFormat::Qoi] {
            let bytes = encode(&src, f, 90).unwrap();
            let back = decode(&bytes, &Limits::default()).unwrap_or_else(|e| panic!("{f:?}: {e}"));
            assert_eq!(back, src, "{f:?}");
        }
    }

    #[test]
    fn lossy_and_palette_formats_come_back_close() {
        let src = sample();
        for f in [ImageFormat::Jpeg, ImageFormat::Gif] {
            let bytes = encode(&src, f, 95).unwrap();
            let back = decode(&bytes, &Limits::default()).unwrap_or_else(|e| panic!("{f:?}: {e}"));
            assert_eq!((back.width(), back.height()), (37, 23));
            let mut worst = 0u8;
            for (a, b) in src.as_bytes().chunks_exact(4).zip(back.as_bytes().chunks_exact(4)) {
                for i in 0..3 {
                    worst = worst.max(a[i].abs_diff(b[i]));
                }
            }
            assert!(worst < 128, "{f:?}: worst channel difference {worst} (JPEG and the 256-colour GIF palette are lossy)");
        }
    }

    #[test]
    fn transparency_survives_png_and_is_flattened_for_jpeg() {
        let mut b = PixelBuffer::new(4, 4).unwrap();
        b.set(1, 1, Rgba::new(200, 100, 50, 128));
        let png = decode(&encode(&b, ImageFormat::Png, 0).unwrap(), &Limits::default()).unwrap();
        assert_eq!(png.get(1, 1), Some(Rgba::new(200, 100, 50, 128)));
        let jpg = decode(&encode(&b, ImageFormat::Jpeg, 100).unwrap(), &Limits::default()).unwrap();
        assert!(jpg.get(0, 0).unwrap().r > 240, "a transparent pixel becomes white");
        assert_eq!(jpg.get(0, 0).unwrap().a, 255);
    }

    #[test]
    fn the_pixel_limit_is_checked_before_decoding() {
        let big = PixelBuffer::new(300, 300).unwrap();
        let bytes = encode(&big, ImageFormat::Png, 0).unwrap();
        let lim = Limits { max_image_pixels: 10_000, ..Limits::default() };
        let e = decode(&bytes, &lim).unwrap_err();
        assert_eq!(e.code(), "limit-exceeded", "{e}");
        assert!(e.to_string().contains("max_image_pixels"), "{e}");
        let lim = Limits { max_encoded_bytes: 10, ..Limits::default() };
        assert!(decode(&bytes, &lim).is_err());
    }

    #[test]
    fn garbage_is_an_error() {
        for junk in [&b""[..], &b"hello"[..], &[0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 0][..], &[0xFF, 0xD8, 0xFF][..]] {
            assert!(decode(junk, &Limits::default()).is_err());
        }
        let mut png = encode(&sample(), ImageFormat::Png, 0).unwrap();
        let n = png.len();
        png.truncate(n / 2);
        assert!(decode(&png, &Limits::default()).is_err());
    }

    #[test]
    fn formats_have_names() {
        for f in ImageFormat::ALL {
            assert_eq!(ImageFormat::from_name(f.extension()), Some(f));
        }
        assert_eq!(ImageFormat::from_name(".JPEG"), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_name("tiff"), None);
    }
}
