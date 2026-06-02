// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Animated-image decoding and egui playback for the **Animator** widget.
//!
//! Supports animated **GIF**, **WebP** and **APNG**, plus any still image as a
//! single-frame "animation". Decoding is delegated to the `image` crate (no
//! external/native dependencies). [`play`] caches the decoded frames + uploaded
//! textures in egui memory and returns the texture for the current moment,
//! requesting a repaint so the animation advances.

use std::sync::Arc;

/// One decoded frame: RGBA8 pixels and how long it is shown.
pub struct RgbaFrame {
    pub rgba: Vec<u8>,
    pub delay_ms: u32,
}

/// A decoded animation (one or more frames).
pub struct Animation {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<RgbaFrame>,
    /// Sum of all frame delays (one loop), in milliseconds.
    pub total_ms: u32,
}

/// Minimum per-frame delay. Many GIFs encode 0 (meaning "as fast as possible");
/// clamp so playback stays sane and `total_ms` is never zero for >1 frame.
const MIN_DELAY_MS: u32 = 20;

/// Largest frame dimension we keep. Frames larger than this are downscaled so an
/// uploaded texture can never exceed the GPU's maximum texture side (which would
/// abort the renderer). 2048 is safe on every target and ample for a UI widget.
const MAX_DIM: u32 = 2048;

/// Target `(w, h)` that fits within `MAX_DIM` while preserving aspect ratio.
fn fit_within(w: u32, h: u32) -> (u32, u32) {
    if w <= MAX_DIM && h <= MAX_DIM {
        return (w.max(1), h.max(1));
    }
    let scale = (MAX_DIM as f32 / w as f32).min(MAX_DIM as f32 / h as f32);
    (((w as f32 * scale) as u32).max(1), ((h as f32 * scale) as u32).max(1))
}

/// Decode an animation from in-memory bytes. Animated GIF/WebP/APNG yield all
/// their frames; any other (still) image yields a single frame. Returns an error
/// string only if the bytes can't be decoded as an image at all.
pub fn decode_animation(bytes: &[u8]) -> Result<Animation, String> {
    use image::AnimationDecoder;
    use std::io::Cursor;

    let fmt = image::guess_format(bytes).map_err(|e| e.to_string())?;

    // Try the animated decoders first.
    let animated: Option<Vec<image::Frame>> = match fmt {
        image::ImageFormat::Gif => image::codecs::gif::GifDecoder::new(Cursor::new(bytes))
            .and_then(|d| d.into_frames().collect_frames())
            .ok(),
        image::ImageFormat::WebP => image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))
            .and_then(|d| d.into_frames().collect_frames())
            .ok(),
        image::ImageFormat::Png => image::codecs::png::PngDecoder::new(Cursor::new(bytes))
            .and_then(|d| d.apng())
            .and_then(|a| a.into_frames().collect_frames())
            .ok(),
        _ => None,
    };

    if let Some(frames) = animated {
        if !frames.is_empty() {
            // Uniform target size for every frame (downscaled to fit MAX_DIM).
            let (tw, th) = fit_within(frames[0].buffer().width(), frames[0].buffer().height());
            let mut out = Vec::with_capacity(frames.len());
            let mut total = 0u32;
            for f in frames {
                let (num, den) = f.delay().numer_denom_ms();
                let delay = if den == 0 { MIN_DELAY_MS } else { (num / den).max(MIN_DELAY_MS) };
                total = total.saturating_add(delay);
                let buf = f.into_buffer();
                let buf = if (buf.width(), buf.height()) != (tw, th) {
                    image::imageops::resize(&buf, tw, th, image::imageops::FilterType::Triangle)
                } else {
                    buf
                };
                out.push(RgbaFrame { rgba: buf.into_raw(), delay_ms: delay });
            }
            return Ok(Animation { width: tw, height: th, frames: out, total_ms: total.max(1) });
        }
    }

    // Fall back to a single still frame.
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?.to_rgba8();
    let (tw, th) = fit_within(img.width(), img.height());
    let img = if (img.width(), img.height()) != (tw, th) {
        image::imageops::resize(&img, tw, th, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    Ok(Animation {
        width: tw,
        height: th,
        frames: vec![RgbaFrame { rgba: img.into_raw(), delay_ms: MIN_DELAY_MS }],
        total_ms: MIN_DELAY_MS,
    })
}

/// Index of the frame visible at `elapsed_ms` into the loop.
fn frame_at(delays: &[u32], total_ms: u32, elapsed_ms: f64, looping: bool) -> usize {
    if delays.len() <= 1 || total_ms == 0 {
        return 0;
    }
    let t = if looping {
        (elapsed_ms.rem_euclid(total_ms as f64)) as u32
    } else if elapsed_ms >= total_ms as f64 {
        return delays.len() - 1; // hold last frame
    } else {
        elapsed_ms as u32
    };
    let mut acc = 0u32;
    for (i, d) in delays.iter().enumerate() {
        acc = acc.saturating_add(*d);
        if t < acc {
            return i;
        }
    }
    delays.len() - 1
}

/// Cached decoded animation + its uploaded textures (stored in egui memory).
#[derive(Clone)]
struct AnimCache {
    textures: Vec<egui::TextureHandle>,
    delays_ms: Vec<u32>,
    total_ms: u32,
    size: egui::Vec2,
    /// `ctx.input().time` at first display — the playback clock origin.
    start: f64,
}

/// Decode (once) and play `bytes` under the given `key`, returning the texture
/// and native pixel size for the current moment.
///
/// * `auto_play` — when false, frame 0 is shown (paused).
/// * `looping`   — when false, playback stops on the final frame.
///
/// The result is cached in egui memory keyed by `key`, so callers should pass a
/// stable key (e.g. the control id + source path). A repaint is requested while
/// animating so the widget keeps advancing.
pub fn play(
    ctx: &egui::Context,
    key: &str,
    load: impl FnOnce() -> Option<Vec<u8>>,
    auto_play: bool,
    looping: bool,
) -> Option<(egui::TextureId, egui::Vec2)> {
    let id = egui::Id::new(("cobolt_anim", key));

    let now = ctx.input(|i| i.time);

    // Look up the cache WITHOUT holding the memory lock across the texture
    // upload: `ctx.load_texture` re-enters egui's own locks, so calling it from
    // inside `ctx.memory_mut(...)` would dead-lock.
    let cache = match ctx.memory(|m| m.data.get_temp::<Arc<AnimCache>>(id)) {
        Some(c) => c,
        None => {
            // Decode + upload textures on first use (bytes loaded lazily).
            let bytes = load()?;
            let anim = decode_animation(&bytes).ok()?;
            let size = egui::Vec2::new(anim.width as f32, anim.height as f32);
            let mut textures = Vec::with_capacity(anim.frames.len());
            let mut delays = Vec::with_capacity(anim.frames.len());
            let expected = anim.width as usize * anim.height as usize * 4;
            for (i, f) in anim.frames.iter().enumerate() {
                // Defensive: never feed a mismatched buffer to ColorImage (it
                // would panic). Decoded frames already match, but guard anyway.
                if f.rgba.len() != expected {
                    continue;
                }
                let color = egui::ColorImage::from_rgba_unmultiplied(
                    [anim.width as usize, anim.height as usize],
                    &f.rgba,
                );
                textures.push(ctx.load_texture(
                    format!("{key}#{i}"),
                    color,
                    egui::TextureOptions::LINEAR,
                ));
                delays.push(f.delay_ms);
            }
            let entry = Arc::new(AnimCache {
                textures,
                delays_ms: delays,
                total_ms: anim.total_ms,
                size,
                start: now,
            });
            ctx.memory_mut(|m| m.data.insert_temp(id, entry.clone()));
            entry
        }
    };

    if cache.textures.is_empty() {
        return None;
    }

    let elapsed = (now - cache.start) * 1000.0;
    let idx = if auto_play {
        frame_at(&cache.delays_ms, cache.total_ms, elapsed.max(0.0), looping)
    } else {
        0
    };

    // Keep animating: repaint until the last frame of a non-looping clip.
    if auto_play && cache.textures.len() > 1 && (looping || elapsed < cache.total_ms as f64) {
        ctx.request_repaint();
    }

    let tex = &cache.textures[idx.min(cache.textures.len() - 1)];
    Some((tex.id(), cache.size))
}

/// Drop any cached decode/textures for `key` (e.g. when the source path changes).
pub fn forget(ctx: &egui::Context, key: &str) {
    let id = egui::Id::new(("cobolt_anim", key));
    ctx.memory_mut(|m| m.data.remove::<Arc<AnimCache>>(id));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a tiny 2-frame animated GIF in memory (red then blue).
    fn make_gif() -> Vec<u8> {
        use image::{codecs::gif::GifEncoder, Delay, Frame, RgbaImage};
        let mut buf = Vec::new();
        {
            let mut enc = GifEncoder::new(&mut buf);
            enc.set_repeat(image::codecs::gif::Repeat::Infinite).unwrap();
            let red = RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
            let blue = RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 255, 255]));
            enc.encode_frame(Frame::from_parts(red, 0, 0, Delay::from_numer_denom_ms(100, 1)))
                .unwrap();
            enc.encode_frame(Frame::from_parts(blue, 0, 0, Delay::from_numer_denom_ms(100, 1)))
                .unwrap();
        }
        buf
    }

    #[test]
    fn decodes_two_frame_gif() {
        let gif = make_gif();
        let anim = decode_animation(&gif).expect("decode gif");
        assert_eq!(anim.frames.len(), 2, "expected 2 frames");
        assert_eq!((anim.width, anim.height), (4, 4));
        assert_eq!(anim.total_ms, 200);
        // First frame red, second blue (RGBA8, top-left pixel).
        assert_eq!(&anim.frames[0].rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&anim.frames[1].rgba[0..4], &[0, 0, 255, 255]);
    }

    #[test]
    fn oversized_frames_are_downscaled_to_max_dim() {
        // A frame wider than MAX_DIM must be downscaled so the texture can never
        // exceed the GPU's maximum side (which would abort the renderer).
        use image::{codecs::gif::GifEncoder, Delay, Frame, Rgba, RgbaImage};
        let mut buf = Vec::new();
        {
            let mut enc = GifEncoder::new(&mut buf);
            let big = RgbaImage::from_pixel(MAX_DIM + 200, 4, Rgba([0, 255, 0, 255]));
            enc.encode_frame(Frame::from_parts(big, 0, 0, Delay::from_numer_denom_ms(100, 1)))
                .unwrap();
        }
        let anim = decode_animation(&buf).expect("decode oversized gif");
        assert!(anim.width <= MAX_DIM, "width {} must be ≤ {}", anim.width, MAX_DIM);
        assert!(anim.height <= MAX_DIM);
        // Buffer length must match the (downscaled) declared size.
        assert_eq!(anim.frames[0].rgba.len(), (anim.width * anim.height * 4) as usize);
    }

    #[test]
    fn still_image_is_single_frame() {
        use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
        let mut png = Vec::new();
        let pixels = vec![10u8, 20, 30, 255]; // 1x1 RGBA
        PngEncoder::new(&mut png)
            .write_image(&pixels, 1, 1, ExtendedColorType::Rgba8)
            .unwrap();
        let anim = decode_animation(&png).expect("decode png");
        assert_eq!(anim.frames.len(), 1);
    }

    #[test]
    fn frame_selection_walks_delays_and_loops() {
        let delays = [100u32, 100, 100];
        let total = 300;
        assert_eq!(frame_at(&delays, total, 0.0, true), 0);
        assert_eq!(frame_at(&delays, total, 150.0, true), 1);
        assert_eq!(frame_at(&delays, total, 250.0, true), 2);
        // Looping wraps around.
        assert_eq!(frame_at(&delays, total, 350.0, true), 0);
        // Non-looping holds the last frame past the end.
        assert_eq!(frame_at(&delays, total, 999.0, false), 2);
    }
}
