// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Pictures from COBOL — spec 092 R10/R11.
//!
//! | CALL name           | Arguments                                        |
//! |---------------------|--------------------------------------------------|
//! | `COBOL-IMAGE-FIT`   | source in, target in, width in, height in, status out |
//! | `COBOL-IMAGE-INFO`  | path in, width out, height out, format out, status out |
//!
//! `IMAGE-FIT` reads a PNG, JPEG, GIF (its first frame), WebP, BMP or TIFF,
//! turns it the way its EXIF orientation says it is meant to be seen, **crops
//! it to fill** the requested shape — the centre of the picture stays the
//! centre — and resizes it to exactly `width` × `height`. The result is written
//! as PNG or JPEG (quality 85), chosen by the target's extension.
//!
//! `IMAGE-INFO` reads the size and the format from the file's header without
//! decoding a single pixel. The size is the picture **as it is meant to be
//! seen**: a camera photo stored on its side with an EXIF rotation reports its
//! upright size, which is the picture `IMAGE-FIT` works on — so
//! `IMAGE-INFO` followed by `IMAGE-FIT` at that size is an upright copy with
//! nothing cropped.
//!
//! Both answer with one status word, never a message: COBOL tests a word.
//!
//! # The `images` feature
//!
//! The decoding lives behind it. A build without it still compiles and still
//! answers: every call says [`NOFEATURE`], the way an HTTP verb in a build
//! without `http` reports status 0. The compiler turns the feature on for any
//! program that names either built-in, so `NOFEATURE` is reachable only through
//! a CALL target assembled at run time — which links everything anyway.

use std::path::Path;

/// The largest picture `IMAGE-FIT` will decode, in pixels (64 megapixels).
///
/// Read from the header **before** any pixel is decoded: a 64 MP RGBA picture
/// is already 256 MB in memory, and a header that claims more is either a
/// panorama nobody should be fitting into a thumbnail or a file built to
/// exhaust memory. `IMAGE-INFO` still reports such a picture's size.
pub const MAX_PIXELS: u64 = 64_000_000;

/// The JPEG quality `IMAGE-FIT` writes with: visually clean for a photo, about
/// a third of the bytes of quality 100.
pub const JPEG_QUALITY: u8 = 85;

/// The picture was fitted (or measured).
pub const OK: &str = "OK";
/// The source file does not exist or cannot be opened.
pub const NOFILE: &str = "NOFILE";
/// The source is not a picture in one of the six formats, or it is damaged.
pub const BADIMAGE: &str = "BADIMAGE";
/// The source is larger than [`MAX_PIXELS`].
pub const TOOBIG: &str = "TOOBIG";
/// The target's extension is not `.png`, `.jpg` or `.jpeg`.
pub const BADEXT: &str = "BADEXT";
/// The width or height is not a whole number of at least 1, or the target
/// would itself be larger than [`MAX_PIXELS`].
pub const BADSIZE: &str = "BADSIZE";
/// The target (or its folder) could not be written.
pub const WRITE: &str = "WRITE";
/// This program was built without the `images` feature.
pub const NOFEATURE: &str = "NOFEATURE";

/// What `IMAGE-INFO` reports for a picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
    /// Upright width, in pixels.
    pub width: u32,
    /// Upright height, in pixels.
    pub height: u32,
    /// `PNG`, `JPEG`, `GIF`, `WEBP`, `BMP` or `TIFF`.
    pub format: &'static str,
}

/// The two encodings `IMAGE-FIT` writes, from the target's extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Png,
    Jpeg,
}

/// The encoding a target path asks for, or `None` for any other extension.
pub fn target_kind(target: &Path) -> Option<TargetKind> {
    let ext = target.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some(TargetKind::Png),
        "jpg" | "jpeg" => Some(TargetKind::Jpeg),
        _ => None,
    }
}

/// The size a fit is asked for, checked: both sides at least 1 and the result
/// no larger than [`MAX_PIXELS`].
#[cfg_attr(not(feature = "images"), allow(dead_code))]
fn checked_size(width: i64, height: i64) -> Option<(u32, u32)> {
    let w = u32::try_from(width).ok().filter(|w| *w >= 1)?;
    let h = u32::try_from(height).ok().filter(|h| *h >= 1)?;
    (u64::from(w) * u64::from(h) <= MAX_PIXELS).then_some((w, h))
}

/// The centred rectangle `(x, y, width, height)` of a `sw` × `sh` picture that
/// has the target's shape — what "crop to fill" keeps.
///
/// The shapes are compared as `sw·h` against `sh·w`, in integers, so a picture
/// already the target's shape is kept whole rather than losing a pixel to a
/// rounded ratio.
#[cfg_attr(not(feature = "images"), allow(dead_code))]
fn fill_crop(sw: u32, sh: u32, w: u32, h: u32) -> (u32, u32, u32, u32) {
    let (sw64, sh64, w64, h64) = (u64::from(sw), u64::from(sh), u64::from(w), u64::from(h));
    let (cw, ch) = if sw64 * h64 > sh64 * w64 {
        // Wider than the target: the full height, the sides trimmed.
        ((sh64 * w64 + h64 / 2) / h64, sh64)
    } else {
        // Taller (or the same shape): the full width, top and bottom trimmed.
        (sw64, (sw64 * h64 + w64 / 2) / w64)
    };
    let cw = cw.clamp(1, sw64) as u32;
    let ch = ch.clamp(1, sh64) as u32;
    ((sw - cw) / 2, (sh - ch) / 2, cw, ch)
}

/// `<file>.part` — the name a file is written under until it is complete.
///
/// The suffix is APPENDED, never swapped for the extension: `a.jpg` and
/// `a.png` written at once must not share `a.part`. `HTTP-DOWNLOAD` names its
/// file the same way.
#[cfg_attr(not(any(feature = "images", feature = "http")), allow(dead_code))]
pub(crate) fn part_path(target: &Path) -> std::path::PathBuf {
    let mut name = target.as_os_str().to_owned();
    name.push(".part");
    std::path::PathBuf::from(name)
}

// ── Without the `images` feature ───────────────────────────────────────────
//
// Adjacent to the real implementation on purpose, like `http_runtime`'s shim:
// a function added below must be added here too.

/// `IMAGE-FIT` in a build without the `images` feature.
#[cfg(not(feature = "images"))]
pub fn fit(_source: &Path, _target: &Path, _width: i64, _height: i64) -> Result<(), &'static str> {
    Err(NOFEATURE)
}

/// `IMAGE-INFO` in a build without the `images` feature.
#[cfg(not(feature = "images"))]
pub fn info(_path: &Path) -> Result<ImageInfo, &'static str> {
    Err(NOFEATURE)
}

// ── With it ─────────────────────────────────────────────────────────────────

/// Fit `source` into exactly `width` × `height` and write it to `target`.
///
/// Nothing is written unless the whole fit succeeds: the picture is encoded to
/// `<target>.part` and renamed over `target` at the end, so a failure leaves an
/// existing target exactly as it was. A missing folder on the way to `target`
/// is created.
#[cfg(feature = "images")]
pub fn fit(source: &Path, target: &Path, width: i64, height: i64) -> Result<(), &'static str> {
    use image::{ImageDecoder, ImageEncoder as _};

    let kind = target_kind(target).ok_or(BADEXT)?;
    let (w, h) = checked_size(width, height).ok_or(BADSIZE)?;

    let (mut decoder, _format) = open_decoder(source)?;
    let (sw, sh) = decoder.dimensions();
    if u64::from(sw) * u64::from(sh) > MAX_PIXELS {
        return Err(TOOBIG);
    }
    // Read before the pixels: the decoder gives up its metadata once decoding
    // starts. A tag it cannot read is no rotation, not a broken picture.
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut picture = image::DynamicImage::from_decoder(decoder).map_err(|e| {
        tracing::warn!("IMAGE-FIT: '{}' could not be decoded: {e}", source.display());
        BADIMAGE
    })?;
    picture.apply_orientation(orientation);

    // Crop first, then resize. `resize_to_fill` does it the other way round —
    // it scales the WHOLE picture until it covers the target, then crops — and
    // for a sliver (1 × 60 000 into 400 × 400) that intermediate is 400 ×
    // 24 000 000 pixels. Cropping first keeps every buffer no larger than the
    // source or the target.
    let (x, y, cw, ch) = fill_crop(picture.width(), picture.height(), w, h);
    let fitted = picture
        .crop_imm(x, y, cw, ch)
        .resize_exact(w, h, image::imageops::FilterType::CatmullRom);
    debug_assert_eq!((fitted.width(), fitted.height()), (w, h));

    let encoded = match kind {
        TargetKind::Png => {
            let mut bytes = Vec::new();
            fitted
                .write_with_encoder(image::codecs::png::PngEncoder::new(&mut bytes))
                .map(|()| bytes)
        }
        TargetKind::Jpeg => {
            // JPEG has no transparency. Dropping the alpha channel would show
            // whatever colour the transparent pixels happen to hold — usually
            // black — so a transparent area is laid on white first.
            let opaque = on_white(&fitted);
            let mut bytes = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY)
                .write_image(opaque.as_raw(), w, h, image::ExtendedColorType::Rgb8)
                .map(|()| bytes)
        }
    }
    .map_err(|e| {
        tracing::warn!("IMAGE-FIT: '{}' could not be encoded: {e}", target.display());
        WRITE
    })?;

    write_whole(target, &encoded).map_err(|e| {
        tracing::warn!("IMAGE-FIT: '{}' could not be written: {e}", target.display());
        WRITE
    })
}

/// Size and format of the picture at `path`, from its header alone.
#[cfg(feature = "images")]
pub fn info(path: &Path) -> Result<ImageInfo, &'static str> {
    use image::ImageDecoder;

    let (mut decoder, format) = open_decoder(path)?;
    let (w, h) = decoder.dimensions();
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    // A quarter turn swaps the sides; a flip or a half turn does not.
    use image::metadata::Orientation as O;
    let (width, height) = match orientation {
        O::Rotate90 | O::Rotate270 | O::Rotate90FlipH | O::Rotate270FlipH => (h, w),
        _ => (w, h),
    };
    Ok(ImageInfo { width, height, format: format_name(format) })
}

/// The decoder for the picture at `path`, its headers read and nothing more.
///
/// The format comes from the CONTENT, never the extension — a PNG saved as
/// `photo.jpg` is still a PNG — and only the six formats this built-in
/// promises are accepted, whatever else the `image` crate happens to have
/// compiled in for another part of the application. The same file answers the
/// same way in a console program, in Run Form and in a built binary.
#[cfg(feature = "images")]
fn open_decoder(path: &Path) -> Result<(impl image::ImageDecoder, image::ImageFormat), &'static str> {
    use image::ImageFormat as F;

    let file = std::fs::File::open(path).map_err(|_| NOFILE)?;
    if !file.metadata().map(|m| m.is_file()).unwrap_or(false) {
        return Err(NOFILE);
    }
    let reader = image::ImageReader::new(std::io::BufReader::new(file))
        .with_guessed_format()
        .map_err(|_| NOFILE)?;
    let format = reader.format().ok_or(BADIMAGE)?;
    if !matches!(format, F::Png | F::Jpeg | F::Gif | F::WebP | F::Bmp | F::Tiff) {
        return Err(BADIMAGE);
    }
    let decoder = reader.into_decoder().map_err(|e| match e {
        image::ImageError::Limits(_) => TOOBIG,
        _ => BADIMAGE,
    })?;
    Ok((decoder, format))
}

#[cfg(feature = "images")]
fn format_name(format: image::ImageFormat) -> &'static str {
    use image::ImageFormat as F;
    match format {
        F::Png => "PNG",
        F::Jpeg => "JPEG",
        F::Gif => "GIF",
        F::WebP => "WEBP",
        F::Bmp => "BMP",
        F::Tiff => "TIFF",
        _ => "",
    }
}

/// The picture with any transparency laid on white, as 8-bit RGB.
#[cfg(feature = "images")]
fn on_white(picture: &image::DynamicImage) -> image::RgbImage {
    if !picture.has_alpha() {
        return picture.to_rgb8();
    }
    let rgba = picture.to_rgba8();
    image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let a = u16::from(a);
        let blend = |c: u8| ((u16::from(c) * a + 255 * (255 - a) + 127) / 255) as u8;
        image::Rgb([blend(r), blend(g), blend(b)])
    })
}

/// Write `bytes` to `target` through `<target>.part`, creating the folder.
#[cfg(feature = "images")]
fn write_whole(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let part = part_path(target);
    let result = std::fs::write(&part, bytes).and_then(|()| std::fs::rename(&part, target));
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_png_and_jpeg_targets_are_accepted_whatever_the_case() {
        let cases = [
            ("a.png", Some(TargetKind::Png)),
            ("a.PNG", Some(TargetKind::Png)),
            ("dir/a.jpg", Some(TargetKind::Jpeg)),
            ("a.JPEG", Some(TargetKind::Jpeg)),
            ("a.gif", None),
            ("a.webp", None),
            ("a", None),
            ("a.png.txt", None),
        ];
        for (path, want) in cases {
            println!("  {path:<12} → {:?}", target_kind(Path::new(path)));
            assert_eq!(target_kind(Path::new(path)), want, "{path}");
        }
    }

    #[test]
    fn a_size_is_whole_positive_and_bounded() {
        assert_eq!(checked_size(400, 400), Some((400, 400)));
        assert_eq!(checked_size(1, 1), Some((1, 1)));
        assert_eq!(checked_size(0, 400), None, "zero wide");
        assert_eq!(checked_size(400, -1), None, "negative");
        assert_eq!(checked_size(8000, 8000), Some((8000, 8000)), "64 MP exactly is allowed");
        assert_eq!(checked_size(8001, 8000), None, "over 64 MP");
        assert_eq!(checked_size(i64::MAX, 1), None, "does not wrap");
    }

    /// Crop to fill keeps the centre and never a buffer larger than the source.
    #[test]
    fn crop_to_fill_keeps_the_centred_shape_of_the_target() {
        let cases = [
            // (source, target, expected crop (x, y, w, h))
            ((1200, 800), (400, 400), (200, 0, 800, 800)),
            ((800, 1200), (400, 400), (0, 200, 800, 800)),
            ((1200, 800), (40, 40), (200, 0, 800, 800)),
            ((800, 1200), (400, 600), (0, 0, 800, 1200)),
            ((100, 100), (400, 400), (0, 0, 100, 100)),
            ((1, 60_000), (400, 400), (0, 29_999, 1, 1)),
            ((60_000, 1), (400, 400), (29_999, 0, 1, 1)),
            ((1201, 800), (400, 400), (200, 0, 800, 800)),
        ];
        for ((sw, sh), (w, h), want) in cases {
            let got = fill_crop(sw, sh, w, h);
            println!("  {sw:>6}×{sh:<6} → {w}×{h}: crop {got:?}");
            assert_eq!(got, want, "{sw}×{sh} → {w}×{h}");
        }
    }

    #[test]
    fn the_part_file_keeps_the_whole_name() {
        assert_eq!(part_path(Path::new("cache/a.jpg")), Path::new("cache/a.jpg.part"));
        assert_ne!(part_path(Path::new("a.jpg")), part_path(Path::new("a.png")));
    }
}
