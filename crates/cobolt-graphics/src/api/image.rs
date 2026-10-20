// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The methods of ImageBuffer (WASM-018 R11).

use super::{dispatch, done, method};
use crate::color::Rgba;
use crate::display_list::BlendMode;
use crate::error::{GraphicsError, Result};
use crate::imagebuf::{ImageBuffer, ImageFormat, ResizeFilter};
use crate::registry::{MethodInfo, ObjectKind, Registry};
use crate::value::{Args, Value};

fn format_of(a: &Args<'_>, i: usize) -> Result<ImageFormat> {
    let name = a.text(i)?;
    ImageFormat::from_name(name).ok_or_else(|| a.refuse(i, format!("\"{name}\" is not an image format (PNG, JPEG, GIF, WEBP, BMP or QOI)")))
}

fn quality_of(a: &Args<'_>, i: usize) -> Result<u8> {
    Ok(a.int_or(i, 90)?.clamp(1, 100) as u8)
}

#[allow(unused_variables)]
pub(crate) static METHODS: &[crate::value::Method<ImageBuffer>] = &[
    method!(ImageBuffer, "Create", [width: Int, height: Int], [color: Color], Nothing, "Make a blank image (transparent unless a colour is given).", |o, r, a| {
        *o = ImageBuffer::create(a.u32(0)?, a.u32(1)?, a.color_or(2, Rgba::TRANSPARENT)?, r.limits())?;
        done()
    }),
    method!(ImageBuffer, "LoadAsset", [name: Text], [], Nothing, "Load a bundled image by name (PNG, JPEG, GIF, WebP, BMP, QOI).", |o, r, a| {
        let host = r.assets().ok_or_else(|| GraphicsError::invalid("the host has no assets to load from"))?;
        *o = ImageBuffer::load_asset(a.text(0)?, host, r.limits())?;
        done()
    }),
    method!(ImageBuffer, "LoadFromBytes", [bytes: Bytes], [], Nothing, "Decode the bytes of an image file; the size limit is checked from its header before any pixel is made.", |o, r, a| {
        *o = ImageBuffer::load_from_bytes(a.bytes(0)?, r.limits())?;
        done()
    }),
    method!(ImageBuffer, "Encode", [format: Text], [quality: Int], Bytes, "The image as the bytes of a file in this format (quality 1 to 100 for JPEG).", |o, r, a| {
        Ok(o.encode(format_of(a, 0)?, quality_of(a, 1)?)?.into())
    }),
    method!(ImageBuffer, "Save", [name: Text, format: Text], [quality: Int], Int, "Encode and hand the bytes to the host under this name; the answer is how many bytes.", |o, r, a| {
        let host = r.assets().ok_or_else(|| GraphicsError::invalid("the host has no way to save"))?;
        Ok(o.save(a.text(0)?, format_of(a, 1)?, quality_of(a, 2)?, host)?.into())
    }),
    method!(ImageBuffer, "GetWidth", [], [], Int, "Width in pixels.", |o, r, a| Ok(o.width().into())),
    method!(ImageBuffer, "GetHeight", [], [], Int, "Height in pixels.", |o, r, a| Ok(o.height().into())),
    method!(ImageBuffer, "Clone", [], [], Handle, "A copy as a new ImageBuffer (the pixels are shared until one of them changes).", |o, r, a| Ok(r.add_image((**o.pixels()).clone())?.into())),
    method!(ImageBuffer, "Resize", [width: Int, height: Int], [filter: Text], Nothing, "Resample to a new size: nearest, bilinear (default), catmull-rom or lanczos.", |o, r, a| {
        let name = a.text_or(2, "bilinear")?;
        let f = ResizeFilter::from_name(name).ok_or_else(|| a.refuse(2, format!("\"{name}\" is not a filter (nearest, bilinear, catmull-rom, lanczos)")))?;
        o.resize(a.u32(0)?, a.u32(1)?, f, r.limits())?;
        done()
    }),
    method!(ImageBuffer, "Crop", [x: Int, y: Int, width: Int, height: Int], [], Nothing, "Keep a rectangle.", |o, r, a| {
        o.crop(a.int(0)?, a.int(1)?, a.u32(2)?, a.u32(3)?)?;
        done()
    }),
    method!(ImageBuffer, "Rotate", [degrees: Num], [background: Color], Nothing, "Rotate clockwise (quarter turns are exact; other angles grow the canvas and fill the corners).", |o, r, a| {
        o.rotate(a.f32(0)?, a.color_or(1, Rgba::TRANSPARENT)?, r.limits())?;
        done()
    }),
    method!(ImageBuffer, "Flip", [horizontal: Flag], [], Nothing, "Mirror left-right (1) or top-bottom (0).", |o, r, a| {
        o.flip(a.flag(0)?);
        done()
    }),
    method!(ImageBuffer, "Fill", [color: Color], [], Nothing, "Set every pixel.", |o, r, a| {
        o.fill(a.color(0)?);
        done()
    }),
    method!(ImageBuffer, "GetPixel", [x: Int, y: Int], [], Color, "The pixel as 0xRRGGBBAA, or -1 outside the image.", |o, r, a| {
        Ok(match o.get_pixel(a.int(0)?, a.int(1)?) {
            Some(c) => c.into(),
            None => Value::Int(-1),
        })
    }),
    method!(ImageBuffer, "SetPixel", [x: Int, y: Int, color: Color], [], Int, "Set one pixel; 1 if it was inside the image.", |o, r, a| Ok(o.set_pixel(a.int(0)?, a.int(1)?, a.color(2)?).into())),
    method!(ImageBuffer, "DrawOnto", [canvas: Handle, x: Num, y: Num], [width: Num, height: Num], Nothing, "Record a DrawImage of this image on a Canvas2D (scaled into width x height when given).", |o, r, a| {
        let me = r.current_handle().ok_or_else(|| GraphicsError::invalid("DrawOnto needs the image's own handle"))?;
        let canvas = a.handle(0)?;
        let (x, y) = (a.num(1)?, a.num(2)?);
        let scaled = if a.has(3) || a.has(4) { Some((a.num(3)?, a.num(4)?)) } else { None };
        r.with_canvas2d(canvas, |c, _| match scaled {
            None => c.record("DrawImage", &[me.into(), x.into(), y.into()]),
            Some((w, h)) => c.record("DrawImageScaled", &[me.into(), x.into(), y.into(), w.into(), h.into()]),
        })?;
        done()
    }),
    method!(ImageBuffer, "Composite", [other: Handle, x: Int, y: Int], [mode: Text, opacity: Num], Nothing, "Draw another image onto this one with a blend mode (source-over by default) and an opacity 0 to 1.", |o, r, a| {
        let mode_name = a.text_or(3, "source-over")?;
        let mode = BlendMode::from_css_name(mode_name).ok_or_else(|| a.refuse(3, format!("\"{mode_name}\" is not a blend mode (source-over, multiply, screen, ...)")))?;
        let other = a.handle(0)?;
        let top = if Some(other) == r.current_handle() { o.clone() } else { ImageBuffer::from_shared(r.image_pixels(other)?) };
        o.composite(&top, a.int_in(1, -1_000_000, 1_000_000)? as i32, a.int_in(2, -1_000_000, 1_000_000)? as i32, mode, a.num_or(4, 1.0)?.clamp(0.0, 1.0) as f32)?;
        done()
    }),
    method!(ImageBuffer, "Grayscale", [], [], Nothing, "Turn to grey.", |o, r, a| {
        o.grayscale();
        done()
    }),
    method!(ImageBuffer, "Invert", [], [], Nothing, "Invert the colours (alpha kept).", |o, r, a| {
        o.invert();
        done()
    }),
    method!(ImageBuffer, "Brightness", [amount: Num], [], Nothing, "Add -1 to 1 (of full scale) to every colour channel.", |o, r, a| {
        o.brightness(a.f32(0)?);
        done()
    }),
    method!(ImageBuffer, "Contrast", [amount: Num], [], Nothing, "Scale contrast about mid grey: 0 none, positive more, negative less.", |o, r, a| {
        o.contrast(a.f32(0)?);
        done()
    }),
    method!(ImageBuffer, "Hue", [degrees: Num], [], Nothing, "Rotate the hue.", |o, r, a| {
        o.hue(a.f32(0)?);
        done()
    }),
    method!(ImageBuffer, "Saturation", [factor: Num], [], Nothing, "Multiply the saturation (0 grey, 1 unchanged, above 1 stronger).", |o, r, a| {
        o.saturation(a.f32(0)?);
        done()
    }),
    method!(ImageBuffer, "Threshold", [level: Int], [], Nothing, "Black and white at a luma level 0 to 255.", |o, r, a| {
        o.threshold(a.int_in(0, 0, 255)? as u8);
        done()
    }),
    method!(ImageBuffer, "BlurGaussian", [sigma: Num], [], Nothing, "Gaussian blur of this standard deviation in pixels.", |o, r, a| {
        o.blur_gaussian(a.f32(0)?);
        done()
    }),
    method!(ImageBuffer, "BlurBox", [radius: Int], [], Nothing, "Box blur of this radius in pixels.", |o, r, a| {
        o.blur_box(a.u32(0)?);
        done()
    }),
    method!(ImageBuffer, "Sharpen", [amount: Num], [], Nothing, "Unsharp mask of this strength.", |o, r, a| {
        o.sharpen(a.f32(0)?);
        done()
    }),
    method!(ImageBuffer, "EdgeDetect", [], [], Nothing, "Sobel edges as a grey image.", |o, r, a| {
        o.edge_detect();
        done()
    }),
    method!(ImageBuffer, "Convolve", [kernel: Floats, divisor: Num], [offset: Num], Nothing, "Convolve with a square kernel (9, 25 ... numbers, row by row), then divide and add an offset.", |o, r, a| {
        o.convolve(&a.floats32(0)?, a.f32(1)?, a.num_or(2, 0.0)? as f32)?;
        done()
    }),
    method!(ImageBuffer, "Dilate", [radius: Int], [], Nothing, "Grow bright areas (maximum over a square window).", |o, r, a| {
        o.dilate(a.u32(0)?)?;
        done()
    }),
    method!(ImageBuffer, "Erode", [radius: Int], [], Nothing, "Shrink bright areas (minimum over a square window).", |o, r, a| {
        o.erode(a.u32(0)?)?;
        done()
    }),
    method!(ImageBuffer, "Histogram", [channel: Int], [], Ints, "256 counts for a channel: 0 red, 1 green, 2 blue, 3 alpha, 4 luma.", |o, r, a| {
        let h = o.histogram();
        let counts: &[u32; 256] = match a.int_in(0, 0, 4)? {
            0 => &h.red,
            1 => &h.green,
            2 => &h.blue,
            3 => &h.alpha,
            _ => &h.luma,
        };
        Ok(counts.iter().map(|c| *c as i64).collect::<Vec<i64>>().into())
    }),
];

pub(crate) fn call(o: &mut ImageBuffer, reg: &mut Registry, method: &str, args: &[Value]) -> Result<Value> {
    dispatch(METHODS, "ImageBuffer", o, reg, method, args)
}

pub(crate) fn infos() -> Vec<MethodInfo> {
    super::infos(ObjectKind::ImageBuffer, METHODS)
}
