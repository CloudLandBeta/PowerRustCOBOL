// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Animated-image decoding and egui playback for the **Animator** control.
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
/// abort the renderer). 2048 is safe on every target and ample for a UI control.
const MAX_DIM: u32 = 2048;

/// Target `(w, h)` that fits within `MAX_DIM` while preserving aspect ratio.
fn fit_within(w: u32, h: u32) -> (u32, u32) {
    if w <= MAX_DIM && h <= MAX_DIM {
        return (w.max(1), h.max(1));
    }
    let scale = (MAX_DIM as f32 / w as f32).min(MAX_DIM as f32 / h as f32);
    (
        ((w as f32 * scale) as u32).max(1),
        ((h as f32 * scale) as u32).max(1),
    )
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
                let delay = if den == 0 {
                    MIN_DELAY_MS
                } else {
                    (num / den).max(MIN_DELAY_MS)
                };
                total = total.saturating_add(delay);
                let buf = f.into_buffer();
                let buf = if (buf.width(), buf.height()) != (tw, th) {
                    image::imageops::resize(&buf, tw, th, image::imageops::FilterType::Triangle)
                } else {
                    buf
                };
                out.push(RgbaFrame {
                    rgba: buf.into_raw(),
                    delay_ms: delay,
                });
            }
            return Ok(Animation {
                width: tw,
                height: th,
                frames: out,
                total_ms: total.max(1),
            });
        }
    }

    // Fall back to a single still frame.
    let img = image::load_from_memory(bytes)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let (tw, th) = fit_within(img.width(), img.height());
    let img = if (img.width(), img.height()) != (tw, th) {
        image::imageops::resize(&img, tw, th, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    Ok(Animation {
        width: tw,
        height: th,
        frames: vec![RgbaFrame {
            rgba: img.into_raw(),
            delay_ms: MIN_DELAY_MS,
        }],
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
    /// `ctx.input().time` at first display — the playback clock origin while
    /// no live command has ever touched this control (see [`resolve_clock`]).
    start: f64,
}

/// A playback command asked for out-of-band — COBOL's `PLAY ANIMATION`,
/// `PAUSE` and `STOP-ANIMATION` on an **Animator** control — applied the next
/// time [`play`] or [`playback_position`] runs for the same control.
///
/// Until a control ever receives one of these, `auto_play` (the design-time
/// property) is the only thing that governs it, unchanged from before this
/// existed — a control nobody calls a method on behaves exactly as it always
/// has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlaybackCommand {
    Play,
    Pause,
    Stop,
}

/// Ask a control's animation to play, pause or stop, from outside the paint
/// loop that actually drives it.
///
/// Keyed by `ctrl_id` alone — never the [`play`] cache key, which also
/// carries the source path — so a command survives a `Source` change and can
/// be issued before the control has ever been painted a first time.
pub fn command(ctx: &egui::Context, ctrl_id: &str, cmd: PlaybackCommand) {
    let id = egui::Id::new(("cobolt_anim_cmd", ctrl_id));
    ctx.memory_mut(|m| m.data.insert_temp(id, cmd));
}

/// The live PLAY/PAUSE/STOP override for one control, once a command has ever
/// touched it. Kept separate from [`AnimCache`] (which is keyed by control id
/// **and** source) so pausing survives a `Source` change instead of quietly
/// resetting to "never paused" the moment the clip does.
#[derive(Clone, Copy, Debug)]
struct PlaybackState {
    /// `None` until a command arrives.
    live_playing: Option<bool>,
    /// Playback time already banked before the most recent resume, in ms.
    accumulated_ms: f64,
    /// Wall-clock instant playback last resumed, or `None` while paused or
    /// stopped. Meaningless while `live_playing` is `None`.
    resumed_at: Option<f64>,
}

/// Apply any pending [`command`] for `ctrl_id` and answer `(elapsed_ms,
/// playing)` — the one place the pause/stop clock is computed, so [`play`]
/// and [`playback_position`] can never disagree about where the animation
/// actually is.
///
/// Safe to call more than once in the same frame for the same control (`play`
/// then `playback_position`, as the Animator's own render arm does): the
/// first call consumes and applies the pending command and persists the
/// result; the second sees no command left, reads the now-current state back,
/// and answers identically.
fn resolve_clock(
    ctx: &egui::Context,
    ctrl_id: &str,
    cache_start: f64,
    auto_play: bool,
    now: f64,
) -> (f64, bool) {
    let cmd_id = egui::Id::new(("cobolt_anim_cmd", ctrl_id));
    let pending = ctx.memory(|m| m.data.get_temp::<PlaybackCommand>(cmd_id));
    if pending.is_some() {
        ctx.memory_mut(|m| m.data.remove::<PlaybackCommand>(cmd_id));
    }

    let state_id = egui::Id::new(("cobolt_anim_state", ctrl_id));
    let mut state = ctx
        .memory(|m| m.data.get_temp::<PlaybackState>(state_id))
        .unwrap_or(PlaybackState {
            live_playing: None,
            accumulated_ms: 0.0,
            resumed_at: None,
        });

    if let Some(cmd) = pending {
        // The first live command ever hands control from the design-time
        // `auto_play` clock to this state machine — bank whatever it had
        // already played so the handover itself never jumps the frame.
        if state.live_playing.is_none() {
            state.accumulated_ms = if auto_play {
                (now - cache_start) * 1000.0
            } else {
                0.0
            };
            state.resumed_at = if auto_play { Some(now) } else { None };
            state.live_playing = Some(auto_play);
        }
        let was_playing = state.live_playing.unwrap_or(false);
        match cmd {
            PlaybackCommand::Play => {
                if !was_playing {
                    state.resumed_at = Some(now);
                }
                state.live_playing = Some(true);
            }
            PlaybackCommand::Pause => {
                if was_playing {
                    state.accumulated_ms +=
                        state.resumed_at.map_or(0.0, |t| (now - t) * 1000.0);
                }
                state.resumed_at = None;
                state.live_playing = Some(false);
            }
            PlaybackCommand::Stop => {
                // `accumulated_ms = 0` alone is enough to land on frame 0 —
                // `frame_at` with zero elapsed always does, no separate
                // "force index 0" branch needed.
                state.accumulated_ms = 0.0;
                state.resumed_at = None;
                state.live_playing = Some(false);
            }
        }
        ctx.memory_mut(|m| m.data.insert_temp(state_id, state));
    }

    match state.live_playing {
        // Untouched by any command: `auto_play` alone decides, exactly as
        // before this existed. `elapsed = 0` when it is false rather than the
        // raw (unused) wall-clock reading — every caller may now treat
        // "not playing" as "elapsed is frozen" uniformly, on this branch and
        // the paused/stopped one below alike, with no separate "force frame
        // 0" case of its own.
        None if auto_play => ((now - cache_start) * 1000.0, true),
        None => (0.0, false),
        Some(playing) => {
            let elapsed =
                state.accumulated_ms + state.resumed_at.map_or(0.0, |t| (now - t) * 1000.0);
            (elapsed, playing)
        }
    }
}

/// Decode (once) and play `bytes` under the given `key`, returning the texture
/// and native pixel size for the current moment.
///
/// * `auto_play` — when false and no live [`command`] has ever been issued
///   for `ctrl_id`, frame 0 is shown (paused).
/// * `looping`   — when false, playback stops on the final frame.
///
/// The result is cached in egui memory keyed by `key`, so callers should pass a
/// stable key (e.g. the control id + source path). A repaint is requested while
/// animating so the control keeps advancing.
pub fn play(
    ctx: &egui::Context,
    key: &str,
    load: impl FnOnce() -> Option<Vec<u8>>,
    auto_play: bool,
    looping: bool,
    ctrl_id: &str,
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

    let (elapsed, playing) = resolve_clock(ctx, ctrl_id, cache.start, auto_play, now);
    let idx = frame_at(&cache.delays_ms, cache.total_ms, elapsed.max(0.0), looping);

    // Keep animating: schedule next repaint at the next frame time (or half the current delay
    // as a safe upper bound). This prevents pegging the CPU at max FPS for every Animator.
    if playing && cache.textures.len() > 1 && (looping || elapsed < cache.total_ms as f64) {
        let delay_ms = cache.delays_ms.get(idx).copied().unwrap_or(16) as f64;
        let remaining = (delay_ms / 1000.0 * 0.6).max(0.008); // ~60% of frame time, min 8ms
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(remaining));
    }

    let tex = &cache.textures[idx.min(cache.textures.len() - 1)];
    Some((tex.id(), cache.size))
}

/// The current playback position of an animation started by [`play`]:
/// `(frame index, completed loop count)`. `None` until the animation has been
/// decoded (or when it has no frames). Lets the host fire frame/loop
/// lifecycle events without owning the media clock.
pub fn playback_position(
    ctx: &egui::Context,
    key: &str,
    auto_play: bool,
    looping: bool,
    ctrl_id: &str,
) -> Option<(usize, u32, bool)> {
    let id = egui::Id::new(("cobolt_anim", key));
    let cache = ctx.memory(|m| m.data.get_temp::<Arc<AnimCache>>(id))?;
    if cache.textures.is_empty() || cache.total_ms == 0 {
        return None;
    }
    let now = ctx.input(|i| i.time);
    // The render arm calls `play()` immediately before this on every frame,
    // which already consumed and applied any pending command — this reads
    // that same, now-current state back (see `resolve_clock`'s doc comment),
    // so the two can never disagree about where the animation actually is.
    let (elapsed, playing) = resolve_clock(ctx, ctrl_id, cache.start, auto_play, now);
    let elapsed = elapsed.max(0.0);
    let idx = frame_at(&cache.delays_ms, cache.total_ms, elapsed, looping);
    let loops = if looping && playing {
        (elapsed / cache.total_ms as f64) as u32
    } else {
        0
    };
    // A non-looping animation has ended once the clock passes its total time.
    let ended = playing && !looping && elapsed >= cache.total_ms as f64;
    Some((idx.min(cache.textures.len() - 1), loops, ended))
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
            enc.set_repeat(image::codecs::gif::Repeat::Infinite)
                .unwrap();
            let red = RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
            let blue = RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 255, 255]));
            enc.encode_frame(Frame::from_parts(
                red,
                0,
                0,
                Delay::from_numer_denom_ms(100, 1),
            ))
            .unwrap();
            enc.encode_frame(Frame::from_parts(
                blue,
                0,
                0,
                Delay::from_numer_denom_ms(100, 1),
            ))
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
            enc.encode_frame(Frame::from_parts(
                big,
                0,
                0,
                Delay::from_numer_denom_ms(100, 1),
            ))
            .unwrap();
        }
        let anim = decode_animation(&buf).expect("decode oversized gif");
        assert!(
            anim.width <= MAX_DIM,
            "width {} must be ≤ {}",
            anim.width,
            MAX_DIM
        );
        assert!(anim.height <= MAX_DIM);
        // Buffer length must match the (downscaled) declared size.
        assert_eq!(
            anim.frames[0].rgba.len(),
            (anim.width * anim.height * 4) as usize
        );
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

    // ── PLAY / PAUSE / STOP-ANIMATION ───────────────────────────────────────
    //
    // Pause/Stop appeared to do nothing at all: the frame index came purely
    // from wall-clock elapsed time (`now - cache.start`) with no notion of a
    // live command anywhere in this crate, so a control kept advancing
    // forever regardless of what COBOL asked for (operator, 2026-09-03,
    // watching an Animator's loop counter climb well past 300 after pressing
    // Stop). These drive `play`/`playback_position` across simulated frames —
    // the same two calls the running form makes every frame — and assert on
    // `playback_position`'s frame index, which is the plain, unambiguous
    // signal (a `TextureId` proves nothing by itself).

    /// One simulated frame at wall-clock `t` seconds: calls `play` (which
    /// consumes any pending command) then `playback_position` (which reads
    /// the result back), exactly as the Animator's own render arm does every
    /// frame, and returns `playback_position`'s frame index.
    fn tick(ctx: &egui::Context, t: f64, key: &str, ctrl_id: &str, auto_play: bool, looping: bool) -> usize {
        let mut input = egui::RawInput::default();
        input.time = Some(t);
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(50.0, 50.0),
        ));
        let mut frame = None;
        let mut full = ctx.run_ui(input, |_ui| {
            let gif = make_gif();
            let _ = play(ctx, key, move || Some(gif), auto_play, looping, ctrl_id);
            frame = playback_position(ctx, key, auto_play, looping, ctrl_id)
                .map(|(idx, _, _)| idx);
        });
        full.textures_delta.clear();
        frame.expect("a 2-frame gif always has a playback position")
    }

    /// Baseline: nobody has ever called Play/Pause/Stop. `auto_play` alone
    /// governs, exactly as before this machinery existed — the 100 ms/frame,
    /// 2-frame clip advances 0→1→0 on the wall clock.
    #[test]
    fn untouched_by_any_command_auto_play_alone_still_governs() {
        let ctx = egui::Context::default();
        assert_eq!(tick(&ctx, 0.00, "k1", "A1", true, true), 0);
        assert_eq!(tick(&ctx, 0.05, "k1", "A1", true, true), 0); // 50ms: still frame 0
        assert_eq!(tick(&ctx, 0.15, "k1", "A1", true, true), 1); // 150ms: frame 1
        assert_eq!(tick(&ctx, 0.25, "k1", "A1", true, true), 0); // 250ms: looped back
    }

    /// …and an Animator whose `AutoPlay` is off, never touched by a command,
    /// stays on frame 0 forever — the historical "paused by design" case.
    #[test]
    fn untouched_and_auto_play_off_never_advances() {
        let ctx = egui::Context::default();
        assert_eq!(tick(&ctx, 0.00, "k2", "A2", false, true), 0);
        assert_eq!(tick(&ctx, 5.00, "k2", "A2", false, true), 0);
    }

    /// Pause freezes at the CURRENT frame — not frame 0, and not a jump
    /// forward or back — and Play resumes from exactly there.
    #[test]
    fn pause_freezes_the_current_frame_and_play_resumes_from_it() {
        let ctx = egui::Context::default();
        let (key, id) = ("k3", "A3");
        assert_eq!(tick(&ctx, 0.00, key, id, true, true), 0);
        assert_eq!(tick(&ctx, 0.15, key, id, true, true), 1, "150ms in: frame 1");

        command(&ctx, id, PlaybackCommand::Pause);
        // Frozen on frame 1, however much simulated time passes.
        assert_eq!(tick(&ctx, 0.16, key, id, true, true), 1);
        assert_eq!(tick(&ctx, 3.00, key, id, true, true), 1, "still frame 1 at t=3s");
        assert_eq!(tick(&ctx, 9.00, key, id, true, true), 1, "still frame 1 at t=9s");

        // Resume: picks up from frame 1, not frame 0 — Play is a resume, not
        // a restart.
        command(&ctx, id, PlaybackCommand::Play);
        assert_eq!(tick(&ctx, 9.00, key, id, true, true), 1, "resumes exactly where it paused");
        // 100ms of PLAYING time later (elapsed banked at pause + 100ms run
        // since resume), it has advanced to the next frame.
        assert_eq!(tick(&ctx, 9.10, key, id, true, true), 0, "advanced one frame after resuming");
    }

    /// Stop resets to frame 0 and halts — unlike Pause, which freezes in
    /// place. A later Play restarts fresh rather than resuming a stale
    /// mid-clip position.
    #[test]
    fn stop_resets_to_frame_zero_and_halts_play_restarts_fresh() {
        let ctx = egui::Context::default();
        let (key, id) = ("k4", "A4");
        assert_eq!(tick(&ctx, 0.00, key, id, true, true), 0);
        assert_eq!(tick(&ctx, 0.15, key, id, true, true), 1, "150ms in: frame 1");

        command(&ctx, id, PlaybackCommand::Stop);
        assert_eq!(tick(&ctx, 0.16, key, id, true, true), 0, "Stop lands on frame 0 immediately");
        assert_eq!(tick(&ctx, 5.00, key, id, true, true), 0, "…and stays there, unlike Pause");

        command(&ctx, id, PlaybackCommand::Play);
        assert_eq!(tick(&ctx, 5.00, key, id, true, true), 0, "restarts at frame 0, not mid-clip");
        assert_eq!(tick(&ctx, 5.15, key, id, true, true), 1, "…and advances normally from there");
    }

    /// Two Animators never share a command or a clock — the whole point of
    /// keying by `ctrl_id`.
    #[test]
    fn two_animators_do_not_share_commands() {
        let ctx = egui::Context::default();
        assert_eq!(tick(&ctx, 0.00, "k5a", "A5a", true, true), 0);
        assert_eq!(tick(&ctx, 0.00, "k5b", "A5b", true, true), 0);
        assert_eq!(tick(&ctx, 0.15, "k5a", "A5a", true, true), 1);
        assert_eq!(tick(&ctx, 0.15, "k5b", "A5b", true, true), 1);

        command(&ctx, "A5a", PlaybackCommand::Stop);
        assert_eq!(tick(&ctx, 0.16, "k5a", "A5a", true, true), 0, "A5a stopped");
        assert_eq!(tick(&ctx, 0.16, "k5b", "A5b", true, true), 1, "A5b untouched, still frame 1");
    }
}
