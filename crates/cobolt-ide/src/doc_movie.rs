// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Screen-sequence recording → animated PNG, for the authoring tool in
//! [`crate::doc_shots`].
//!
//! # What it is for
//!
//! Some things in the IDE cannot be shown with one photograph: dragging a
//! control onto a form, a menu opening, an entrance effect, a build running to
//! completion. This records the same window [`crate::doc_shots`] photographs,
//! at a steady cadence, and writes the sequence as an **animated PNG** — which
//! is still a `.png`, so it drops into an existing `📷 Screenshot needed` slot
//! with no change to the document format, and browsers and GitHub animate it.
//!
//! # Why it runs on its own thread
//!
//! [`record`] is called from a worker, never from the egui update. A recording
//! must keep its cadence while the UI thread is busy — an agent request, a
//! build, a long markdown re-render — and the UI thread cannot promise a frame
//! every 125 ms. It also blocks in [`std::thread::sleep`] between captures,
//! which on the UI thread would freeze the IDE for the whole recording.
//!
//! # The cursor
//!
//! `screencapture -C` can bake the system cursor into each frame, and that is
//! *not* what this does. A capture costs ~120 ms, so the pointer would be
//! sampled barely eight times a second: correct at each instant, and visibly
//! jerky in playback.
//!
//! Instead the pointer is polled on its own thread at [`POINTER_INTERVAL`]
//! (~125 Hz) with a timestamp, the frames are captured without a cursor, and
//! the arrow is drawn afterwards at a **centred weighted average** of the
//! samples around each frame's own timestamp ([`smoothed`]). Centred, not
//! trailing: the whole track exists by the time the drawing happens, so the
//! filter removes tremor and sampling noise while adding **no lag** — a
//! trailing average would put the cursor permanently behind the click it is
//! about to make. The arrow is rasterized at the exact sub-pixel position, so
//! travel does not stair-step across pixel boundaries.
//!
//! # Size
//!
//! A full frame every 125 ms would put tens of megabytes into the repository
//! for a ten-second clip. Two things stop that: frames are scaled to [`WIDTH`]
//! (what the guide actually renders) before anything else, and each frame after
//! the first is written as **only the rectangle that changed** since the last
//! one, with `BlendOp::Source` — APNG's own inter-frame compression. A frame
//! identical to its predecessor is not written at all; its time is added to
//! that predecessor's delay instead, so a still window costs nothing.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui::Color32;
use image::{RgbImage, RgbaImage};

use crate::doc_shots::WindowTarget;

/// How often a frame is requested. `screencapture` needs ~120 ms for a window
/// (measured, 1280×828 on a 2× display), so this is a *target*, not a promise —
/// the real interval is whatever each capture took, and it is written into the
/// APNG per frame, so playback runs at the speed the recording actually had.
pub const TARGET_INTERVAL: Duration = Duration::from_millis(125);

/// How often the pointer position is sampled. Independent of the frame rate —
/// that is the entire point of sampling it separately.
pub const POINTER_INTERVAL: Duration = Duration::from_millis(8);

/// Width the frames are scaled to. [`crate::doc_shots::DOC_WIDTH`] is what the
/// guide renders an image at, and storing more than that multiplies the file
/// size for pixels no reader ever sees.
pub const WIDTH: u32 = 900;

/// Half-width of the cursor smoothing window, in seconds. Samples further than
/// this from a frame's timestamp do not contribute to it.
const SMOOTH_WINDOW: f64 = 0.09;

/// Ceilings. A recording holds every frame in memory until it is encoded, so
/// each of these is a real limit rather than a guard against the absurd.
pub const MAX_DURATION: Duration = Duration::from_secs(90);
/// Frame ceiling — 90 s at the target cadence, with room for a fast machine.
pub const MAX_FRAMES: usize = 900;
/// Memory ceiling for the retained frames. At [`WIDTH`] on a 16:10 window a
/// frame is ~1.6 MB, so this is roughly 240 frames — half a minute.
pub const MAX_BYTES: usize = 384 * 1024 * 1024;

/// Why the recorder stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// The operator pressed the key again — the ordinary case.
    Operator,
    /// [`MAX_DURATION`] reached.
    Duration,
    /// [`MAX_FRAMES`] reached.
    Frames,
    /// [`MAX_BYTES`] of retained frames reached.
    Memory,
    /// A capture failed part-way; what was recorded up to then is kept.
    Capture,
}

impl Stop {
    /// What to tell the operator, when the recording ended for a reason they
    /// did not choose. `None` for [`Stop::Operator`] — there is nothing to say.
    pub fn note(self) -> Option<&'static str> {
        match self {
            Self::Operator => None,
            Self::Duration => Some("stopped at the 90-second limit"),
            Self::Frames => Some("stopped at the frame limit"),
            Self::Memory => Some("stopped at the memory limit"),
            Self::Capture => Some("a capture failed — the frames up to it were kept"),
        }
    }
}

/// A finished recording, encoded and ready to be written into a slot.
pub struct Movie {
    /// Frames captured.
    pub captured: usize,
    /// Frames actually written — fewer when the window sat still, because an
    /// unchanged frame extends its predecessor's delay instead of being stored.
    pub written: usize,
    /// Wall-clock length.
    pub seconds: f32,
    pub width: u32,
    pub height: u32,
    /// The encoded animated PNG.
    pub apng: Vec<u8>,
    pub stop: Stop,
}

impl Movie {
    /// One line for the popup: what was recorded, how long, how big.
    pub fn report(&self) -> String {
        let fps = if self.seconds > 0.0 {
            self.captured as f32 / self.seconds
        } else {
            0.0
        };
        let mut line = format!(
            "{} frames over {:.1} s ({:.1} fps, {} written), {}×{}, {:.1} MB",
            self.captured,
            self.seconds,
            fps,
            self.written,
            self.width,
            self.height,
            self.apng.len() as f32 / (1024.0 * 1024.0),
        );
        if let Some(note) = self.stop.note() {
            line.push_str(" — ");
            line.push_str(note);
        }
        line
    }
}

/// One captured frame: opaque, already scaled to its final size.
struct Frame {
    /// Seconds since the recording started, taken when the capture began.
    at: f64,
    rgb: RgbImage,
}

/// One pointer sample: seconds since the recording started, and the position in
/// screen points.
type Sample = (f64, f64, f64);

// ── Recording ─────────────────────────────────────────────────────────────────

/// Record `target` until `stop` is set, then encode the result.
///
/// Blocks for the whole recording — call it on a worker. `backdrop` is the
/// colour the IDE's transparent window is flattened onto, the same one the
/// still path uses; it is applied here rather than at insert time because the
/// frames are scaled and discarded as they arrive.
///
/// The region is fixed when recording starts. Moving the window mid-recording
/// therefore records the *place* the window was, not the window — which is the
/// honest behaviour for a tool built on `screencapture -R`, and the reason the
/// recorded window should be left alone while it runs.
pub fn record(
    target: WindowTarget,
    backdrop: Color32,
    stop: Arc<AtomicBool>,
) -> Result<Movie, String> {
    let started = Instant::now();
    let track: Arc<Mutex<Vec<Sample>>> = Arc::new(Mutex::new(Vec::new()));
    let sampling = Arc::new(AtomicBool::new(true));
    let sampler = spawn_pointer_sampler(Arc::clone(&track), Arc::clone(&sampling), started);

    let mut frames: Vec<Frame> = Vec::new();
    let mut held = 0usize;
    let mut why = Stop::Operator;
    let mut first_error: Option<String> = None;
    let mut next = Instant::now();

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if started.elapsed() >= MAX_DURATION {
            why = Stop::Duration;
            break;
        }
        if frames.len() >= MAX_FRAMES {
            why = Stop::Frames;
            break;
        }
        if held >= MAX_BYTES {
            why = Stop::Memory;
            break;
        }

        let at = started.elapsed().as_secs_f64();
        match crate::doc_shots::capture_rgba(&target) {
            Ok(shot) => {
                let rgb = flatten_and_scale(&shot, backdrop, WIDTH);
                // Every frame of an APNG shares one canvas. A capture that
                // came back a different size cannot go on it, so it is dropped
                // rather than allowed to abort a recording already in progress.
                // Dropped, but still PACED — skipping the wait below would spin
                // this loop on `screencapture` for as long as the mismatch
                // lasted.
                let fits = frames
                    .first()
                    .is_none_or(|f| f.rgb.dimensions() == rgb.dimensions());
                if fits {
                    held += rgb.as_raw().len();
                    frames.push(Frame { at, rgb });
                }
            }
            Err(message) => {
                why = Stop::Capture;
                first_error = Some(message);
                break;
            }
        }

        // Pace from the intended start of the last frame, never from now, so a
        // slow capture does not compound. A capture that already overran the
        // interval starts the next one immediately instead of bursting to
        // catch up.
        next += TARGET_INTERVAL;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
    }

    let seconds = started.elapsed().as_secs_f64();
    sampling.store(false, Ordering::Relaxed);
    if let Some(handle) = sampler {
        let _ = handle.join();
    }

    if frames.is_empty() {
        return Err(first_error.unwrap_or_else(|| "nothing was captured".into()));
    }

    let (width, height) = frames[0].rgb.dimensions();
    // A poisoned lock means the sampler thread died mid-push. The frames are
    // still good; losing the cursor is a far better outcome than losing the
    // recording, so the track is read through the poison rather than unwrapped.
    let track = match track.lock() {
        Ok(track) => track.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };
    draw_pointer_track(&mut frames, &target, &track);

    let plan = plan(&frames, seconds);
    let apng = encode(&frames, &plan, width, height)?;
    Ok(Movie {
        captured: frames.len(),
        written: plan.len(),
        seconds: seconds as f32,
        width,
        height,
        apng,
        stop: why,
    })
}

/// Composite the capture onto `backdrop` and scale it to `width`.
///
/// Flattening happens **before** scaling on purpose: the IDE window is
/// transparent, and interpolating a straight-alpha image mixes the colour of
/// fully transparent pixels into their neighbours, which fringes every glass
/// edge. Once flattened there is no alpha left to fringe.
fn flatten_and_scale(shot: &RgbaImage, backdrop: Color32, width: u32) -> RgbImage {
    let (w, h) = shot.dimensions();
    let mut flat = RgbImage::new(w, h);
    for (out, pixel) in flat.pixels_mut().zip(shot.pixels()) {
        // The capture is STRAIGHT alpha — it came back through a PNG — not the
        // premultiplied Color32 the still path flattens, so the source has to
        // be multiplied here: src·a + bg·(1−a).
        let alpha = pixel.0[3] as u32;
        let inverse = 255 - alpha;
        let blend = |channel: u8, under: u8| {
            ((channel as u32 * alpha + under as u32 * inverse) / 255).min(255) as u8
        };
        out.0 = [
            blend(pixel.0[0], backdrop.r()),
            blend(pixel.0[1], backdrop.g()),
            blend(pixel.0[2], backdrop.b()),
        ];
    }
    if w <= width || w == 0 {
        return flat;
    }
    let scaled = (h as f32 * width as f32 / w as f32).round().max(1.0) as u32;
    // Triangle, not Lanczos3: this runs once per frame inside the capture
    // cadence, and Lanczos costs more than the interval budget allows.
    image::imageops::resize(&flat, width, scaled, image::imageops::FilterType::Triangle)
}

// ── Pointer ───────────────────────────────────────────────────────────────────

/// Where the pointer is, in screen points, top-left origin — the same space
/// [`WindowTarget::region`] hands to `screencapture`.
#[cfg(target_os = "macos")]
fn pointer_now() -> Option<(f64, f64)> {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreate(source: *const std::ffi::c_void) -> *mut std::ffi::c_void;
        fn CGEventGetLocation(event: *const std::ffi::c_void) -> CGPoint;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *const std::ffi::c_void);
    }
    // SAFETY: `CGEventCreate(NULL)` is the documented way to build an event
    // carrying the current pointer state; `CGEventGetLocation` reads it, and
    // the event is a CoreFoundation object this side owns and releases exactly
    // once. Nothing here touches memory owned elsewhere.
    unsafe {
        let event = CGEventCreate(std::ptr::null());
        if event.is_null() {
            return None;
        }
        let point = CGEventGetLocation(event);
        CFRelease(event as *const std::ffi::c_void);
        Some((point.x, point.y))
    }
}

/// No pointer anywhere else — the capture itself is macOS-only.
#[cfg(not(target_os = "macos"))]
fn pointer_now() -> Option<(f64, f64)> {
    None
}

/// Poll the pointer into `track` until `sampling` clears.
fn spawn_pointer_sampler(
    track: Arc<Mutex<Vec<Sample>>>,
    sampling: Arc<AtomicBool>,
    started: Instant,
) -> Option<std::thread::JoinHandle<()>> {
    pointer_now()?;
    Some(std::thread::spawn(move || {
        while sampling.load(Ordering::Relaxed) {
            if let Some((x, y)) = pointer_now() {
                if let Ok(mut samples) = track.lock() {
                    samples.push((started.elapsed().as_secs_f64(), x, y));
                }
            }
            std::thread::sleep(POINTER_INTERVAL);
        }
    }))
}

/// The pointer position for a frame taken at `at`, as a **centred** triangular
/// weighted average of the samples around it.
///
/// Centred is what makes this usable: an ordinary trailing average would smooth
/// just as well and put the arrow permanently behind where it actually was, so
/// every click in the recording would land next to the button. Because the
/// whole track exists before any frame is drawn, the filter can look forwards
/// as well as backwards and cost nothing in lag.
///
/// `None` only when nothing was ever sampled.
fn smoothed(track: &[Sample], at: f64) -> Option<(f64, f64)> {
    let (mut weight, mut x, mut y) = (0.0f64, 0.0f64, 0.0f64);
    for &(t, sx, sy) in track {
        let distance = (t - at).abs();
        if distance >= SMOOTH_WINDOW {
            continue;
        }
        let w = 1.0 - distance / SMOOTH_WINDOW;
        weight += w;
        x += sx * w;
        y += sy * w;
    }
    if weight > 0.0 {
        return Some((x / weight, y / weight));
    }
    // A gap in the track — a starved sampler thread, or a frame taken before
    // the first sample. The nearest sample is a better answer than no cursor.
    track
        .iter()
        .min_by(|a, b| {
            (a.0 - at)
                .abs()
                .partial_cmp(&(b.0 - at).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|&(_, x, y)| (x, y))
}

/// Stamp the smoothed pointer into every frame.
fn draw_pointer_track(frames: &mut [Frame], target: &WindowTarget, track: &[Sample]) {
    if track.is_empty() {
        return;
    }
    let Some(first) = frames.first() else { return };
    let (width, height) = first.rgb.dimensions();
    let (rect_w, rect_h) = (target.rect.width() as f64, target.rect.height() as f64);
    if rect_w <= 0.0 || rect_h <= 0.0 {
        return;
    }
    // Points → final image pixels, derived from the image the capture actually
    // produced rather than from the display scale, so the scaling step above
    // cannot put the arrow somewhere the operator's pointer was not.
    let (sx, sy) = (width as f64 / rect_w, height as f64 / rect_h);
    // The arrow is specified in points, so the same ratio sizes it: it comes
    // out exactly as large, relative to the window, as the operator saw it.
    let scale = sx as f32;
    for frame in frames {
        let Some((x, y)) = smoothed(track, frame.at) else {
            continue;
        };
        draw_cursor(
            &mut frame.rgb,
            ((x - target.rect.min.x as f64) * sx) as f32,
            ((y - target.rect.min.y as f64) * sy) as f32,
            scale,
        );
    }
}

/// The classic arrow pointer, in points, hotspot at the origin.
const ARROW: [(f32, f32); 7] = [
    (0.0, 0.0),
    (0.0, 16.0),
    (4.1, 12.4),
    (6.8, 18.6),
    (9.6, 17.4),
    (6.9, 11.4),
    (11.6, 11.4),
];

/// Draw the arrow at `(x, y)` pixels — a white body with a black rim, the way
/// every desktop draws it, so it reads against light and dark UI alike.
///
/// `x` and `y` are deliberately floats and are honoured to the sub-pixel: the
/// arrow is rasterized fresh for each frame at its exact position. Rounding to
/// whole pixels would undo the smoothing, replacing a smooth glide with a
/// one-pixel stair every few frames.
fn draw_cursor(image: &mut RgbImage, x: f32, y: f32, scale: f32) {
    let scale = scale.max(0.25);
    let rim = (scale * 0.9).max(1.0);
    let poly: Vec<(f32, f32)> = ARROW
        .iter()
        .map(|&(px, py)| (x + px * scale, y + py * scale))
        .collect();

    let (width, height) = image.dimensions();
    let min_x = poly.iter().fold(f32::MAX, |a, p| a.min(p.0)) - rim - 1.0;
    let max_x = poly.iter().fold(f32::MIN, |a, p| a.max(p.0)) + rim + 1.0;
    let min_y = poly.iter().fold(f32::MAX, |a, p| a.min(p.1)) - rim - 1.0;
    let max_y = poly.iter().fold(f32::MIN, |a, p| a.max(p.1)) + rim + 1.0;
    if max_x < 0.0 || max_y < 0.0 || min_x >= width as f32 || min_y >= height as f32 {
        return;
    }
    let x0 = min_x.floor().max(0.0) as u32;
    let y0 = min_y.floor().max(0.0) as u32;
    let x1 = (max_x.ceil() as i64).clamp(0, width as i64) as u32;
    let y1 = (max_y.ceil() as i64).clamp(0, height as i64) as u32;

    // 3×3 subsamples per pixel: enough to anti-alias an arrow ten pixels tall
    // without the cost showing up against a 125 ms capture.
    const SUB: u32 = 3;
    let step = 1.0 / SUB as f32;
    for py in y0..y1 {
        for px in x0..x1 {
            let (mut body, mut rimmed) = (0u32, 0u32);
            for sy in 0..SUB {
                for sx in 0..SUB {
                    let point = (
                        px as f32 + (sx as f32 + 0.5) * step,
                        py as f32 + (sy as f32 + 0.5) * step,
                    );
                    if inside(&poly, point) {
                        body += 1;
                        rimmed += 1;
                    } else if boundary_distance(&poly, point) <= rim {
                        rimmed += 1;
                    }
                }
            }
            if rimmed == 0 {
                continue;
            }
            let total = (SUB * SUB) as f32;
            let pixel = image.get_pixel_mut(px, py);
            // Rim first, body over it — the rim is drawn under the whole arrow
            // rather than only outside it, so no seam appears where the two
            // coverages meet.
            let mix = |under: u8, over: f32, coverage: f32| {
                (under as f32 * (1.0 - coverage) + over * coverage).round() as u8
            };
            let rim_cov = rimmed as f32 / total;
            let body_cov = body as f32 / total;
            for channel in 0..3 {
                let value = mix(pixel.0[channel], 0.0, rim_cov);
                pixel.0[channel] = mix(value, 255.0, body_cov);
            }
        }
    }
}

/// Ray-casting point-in-polygon.
fn inside(poly: &[(f32, f32)], point: (f32, f32)) -> bool {
    let (x, y) = point;
    let mut hit = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            hit = !hit;
        }
        j = i;
    }
    hit
}

/// Distance from `point` to the polygon's outline — the rim's thickness is
/// measured with it, so the rim is uniform all the way round instead of
/// thinning at the sharp tip the way a scaled-up copy of the arrow would.
fn boundary_distance(poly: &[(f32, f32)], point: (f32, f32)) -> f32 {
    let mut best = f32::MAX;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        best = best.min(segment_distance(poly[j], poly[i], point));
        j = i;
    }
    best
}

fn segment_distance(a: (f32, f32), b: (f32, f32), p: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx * dx + dy * dy;
    let t = if length <= f32::EPSILON {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length).clamp(0.0, 1.0)
    };
    let (cx, cy) = (a.0 + dx * t, a.1 + dy * t);
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}

// ── Encoding ──────────────────────────────────────────────────────────────────

/// One frame as it will be written: which capture it is, the rectangle of it
/// that actually changed, and how long it stays on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Emit {
    index: usize,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    delay_ms: u16,
}

/// Decide what to write.
///
/// A frame identical to the one before it is **not written**; its time is added
/// to that frame's delay instead. A window that sat still for two seconds
/// therefore costs one frame and a two-second delay, not sixteen copies of
/// itself — which is most of why a recording of a mostly-static IDE stays small
/// enough to commit.
fn plan(frames: &[Frame], total: f64) -> Vec<Emit> {
    let mut out: Vec<Emit> = Vec::new();
    for (index, frame) in frames.iter().enumerate() {
        // How long this capture is on screen: until the next one, or — for the
        // last — until the recording stopped.
        let until = frames
            .get(index + 1)
            .map(|next| next.at)
            .unwrap_or_else(|| total.max(frame.at));
        let ms = (((until - frame.at) * 1000.0).round() as i64).clamp(1, u16::MAX as i64) as u16;

        let changed = match out.last() {
            None => Some((0, 0, frame.rgb.width(), frame.rgb.height())),
            Some(previous) => changed_box(&frames[previous.index].rgb, &frame.rgb),
        };
        match changed {
            Some((x, y, w, h)) => out.push(Emit { index, x, y, w, h, delay_ms: ms }),
            None => {
                if let Some(previous) = out.last_mut() {
                    previous.delay_ms = previous.delay_ms.saturating_add(ms);
                }
            }
        }
    }
    out
}

/// The smallest rectangle covering every pixel that differs, or `None` when the
/// two frames are identical.
fn changed_box(a: &RgbImage, b: &RgbImage) -> Option<(u32, u32, u32, u32)> {
    let (width, height) = a.dimensions();
    if b.dimensions() != (width, height) {
        return Some((0, 0, width, height));
    }
    let stride = width as usize * 3;
    let (ra, rb) = (a.as_raw(), b.as_raw());
    let (mut x0, mut y0, mut x1, mut y1) = (width, height, 0u32, 0u32);
    for y in 0..height {
        let row = y as usize * stride;
        let (sa, sb) = (&ra[row..row + stride], &rb[row..row + stride]);
        if sa == sb {
            continue;
        }
        let mut first = 0usize;
        while first < stride && sa[first] == sb[first] {
            first += 1;
        }
        let mut last = stride;
        while last > first && sa[last - 1] == sb[last - 1] {
            last -= 1;
        }
        x0 = x0.min((first / 3) as u32);
        x1 = x1.max(last.div_ceil(3) as u32);
        y0 = y0.min(y);
        y1 = y1.max(y + 1);
    }
    (y1 > y0).then(|| (x0, y0, x1 - x0, y1 - y0))
}

/// Copy one rectangle out of a frame, row by row, as raw RGB.
fn crop(image: &RgbImage, x: u32, y: u32, w: u32, h: u32) -> Vec<u8> {
    let stride = image.width() as usize * 3;
    let raw = image.as_raw();
    let mut out = Vec::with_capacity(w as usize * h as usize * 3);
    for row in y..y + h {
        let start = row as usize * stride + x as usize * 3;
        out.extend_from_slice(&raw[start..start + w as usize * 3]);
    }
    out
}

/// Write the planned frames as an animated PNG.
fn encode(frames: &[Frame], plan: &[Emit], width: u32, height: u32) -> Result<Vec<u8>, String> {
    if plan.is_empty() {
        return Err("nothing to encode".into());
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(std::io::Cursor::new(&mut out), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    // Fast, not Default: this compresses a few hundred frames while the
    // operator waits at the popup, and the frames are already reduced to their
    // changed rectangles, which is where the real saving is.
    encoder.set_compression(png::Compression::Fast);
    encoder
        .set_animated(plan.len() as u32, 0)
        .map_err(|error| format!("apng header: {error}"))?;
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("apng header: {error}"))?;

    for (position, emit) in plan.iter().enumerate() {
        writer
            .set_frame_delay(emit.delay_ms, 1000)
            .map_err(|error| format!("apng frame {position}: {error}"))?;
        if position > 0 {
            // Order matters: each setter validates against the values already
            // stored, so the rectangle has to be shrunk from the origin before
            // it is moved, or a frame near the right edge is rejected as out
            // of bounds.
            writer
                .reset_frame_position()
                .and_then(|()| writer.set_frame_dimension(emit.w, emit.h))
                .and_then(|()| writer.set_frame_position(emit.x, emit.y))
                // Source, not Over: the rectangle *replaces* what was under it.
                // Blending would leave the previous frame showing through
                // anything drawn with partial alpha.
                .and_then(|()| writer.set_blend_op(png::BlendOp::Source))
                // None: the canvas is left as it is for the next frame, which
                // is what makes writing only the changed rectangle correct.
                .and_then(|()| writer.set_dispose_op(png::DisposeOp::None))
                .map_err(|error| format!("apng frame {position}: {error}"))?;
        }
        let data = crop(
            &frames[emit.index].rgb,
            emit.x,
            emit.y,
            emit.w,
            emit.h,
        );
        writer
            .write_image_data(&data)
            .map_err(|error| format!("apng frame {position}: {error}"))?;
    }
    writer
        .finish()
        .map_err(|error| format!("apng finish: {error}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{AnimationDecoder, Rgb};

    /// A frame of a flat colour with filled squares painted on it, so a test
    /// can say exactly which pixels changed between two of them.
    fn frame(at: f64, w: u32, h: u32, base: [u8; 3], patches: &[(u32, u32, u32, u32)]) -> Frame {
        let mut rgb = RgbImage::from_pixel(w, h, Rgb(base));
        for &(x, y, pw, ph) in patches {
            for py in y..y + ph {
                for px in x..x + pw {
                    rgb.put_pixel(px, py, Rgb([255, 0, 0]));
                }
            }
        }
        Frame { at, rgb }
    }

    /// Decode an APNG back into its frames and per-frame delays.
    fn decode(apng: &[u8]) -> Vec<(u32, u32, image::RgbaImage)> {
        let decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(apng))
            .expect("the bytes must be a PNG")
            .apng()
            .expect("and an animated one");
        decoder
            .into_frames()
            .map(|frame| {
                let frame = frame.expect("every frame must decode");
                let (numer, denom) = frame.delay().numer_denom_ms();
                (numer, denom, frame.into_buffer())
            })
            .collect()
    }

    /// **The compression that makes a recording committable.** A window that
    /// did not change between two captures must not be stored twice; the second
    /// capture's time is added to the first one's delay instead.
    #[test]
    fn an_unchanged_frame_extends_its_predecessor_rather_than_being_stored() {
        let frames = vec![
            frame(0.0, 40, 30, [10, 20, 30], &[]),
            frame(0.125, 40, 30, [10, 20, 30], &[]),
            frame(0.25, 40, 30, [10, 20, 30], &[]),
            frame(0.375, 40, 30, [10, 20, 30], &[(5, 5, 4, 4)]),
        ];
        let plan = plan(&frames, 0.5);
        assert_eq!(plan.len(), 2, "three identical captures are one frame: {plan:?}");
        assert_eq!(
            plan[0].delay_ms, 375,
            "the three identical captures' time belongs to the frame that is shown"
        );
        assert_eq!(plan[1].index, 3);
        assert_eq!(plan[1].delay_ms, 125, "the last frame runs to the end of the recording");
    }

    /// Only the rectangle that changed is written. A one-pixel edit in the
    /// middle of a frame must not cost a whole frame.
    #[test]
    fn a_frame_is_written_as_only_the_rectangle_that_changed() {
        let frames = vec![
            frame(0.0, 40, 30, [10, 20, 30], &[]),
            frame(0.1, 40, 30, [10, 20, 30], &[(7, 11, 3, 2)]),
        ];
        let plan = plan(&frames, 0.2);
        assert_eq!(plan.len(), 2);
        assert_eq!(
            (plan[0].x, plan[0].y, plan[0].w, plan[0].h),
            (0, 0, 40, 30),
            "the first frame is always the whole canvas"
        );
        assert_eq!(
            (plan[1].x, plan[1].y, plan[1].w, plan[1].h),
            (7, 11, 3, 2),
            "and the second is exactly the square that moved"
        );
    }

    /// The bounding box is tight on every side, including a change that touches
    /// the right and bottom edges — the case an off-by-one in the exclusive
    /// byte index would silently clip.
    #[test]
    fn the_changed_box_reaches_the_last_row_and_column() {
        let a = frame(0.0, 8, 6, [0, 0, 0], &[]);
        let b = frame(0.1, 8, 6, [0, 0, 0], &[(6, 4, 2, 2)]);
        assert_eq!(changed_box(&a.rgb, &b.rgb), Some((6, 4, 2, 2)));
        assert_eq!(changed_box(&a.rgb, &a.rgb), None, "identical frames report no change");
    }

    /// **The whole recording, end to end**: what comes back must be a real
    /// animated PNG whose frames rebuild the captures, with the delays the
    /// recording actually had. Partial frames only work if the decoder composes
    /// them onto the previous canvas, which is exactly what `DisposeOp::None`
    /// plus `BlendOp::Source` is for — so this also proves those two.
    #[test]
    fn the_encoded_apng_replays_the_captures_with_their_own_timings() {
        let frames = vec![
            frame(0.0, 24, 16, [10, 20, 30], &[]),
            frame(0.2, 24, 16, [10, 20, 30], &[(4, 4, 3, 3)]),
            // Keeps the first square AND adds a second, so the frame written
            // for it is only the new square — which is what makes the
            // composition assertions below mean anything.
            frame(0.5, 24, 16, [10, 20, 30], &[(4, 4, 3, 3), (9, 6, 2, 2)]),
        ];
        let plan = plan(&frames, 0.7);
        let apng = encode(&frames, &plan, 24, 16).expect("encode");
        let played = decode(&apng);

        assert_eq!(played.len(), 3, "every planned frame reaches the file");
        for (position, (numer, denom, image)) in played.iter().enumerate() {
            assert_eq!(image.dimensions(), (24, 16), "frame {position} is composed onto the canvas");
            // `numer_denom_ms` is already a duration in milliseconds, as a
            // ratio — not a rate to be inverted.
            let ms = *numer as f32 / *denom as f32;
            let expected = plan[position].delay_ms as f32;
            assert!(
                (ms - expected).abs() < 1.0,
                "frame {position} plays for {ms} ms, recorded {expected} ms"
            );
        }
        // Frame 2 was written as a small rectangle. Its full picture must still
        // come back — the square it added AND the square frame 1 left behind.
        let last = &played[2].2;
        assert_eq!(last.get_pixel(9, 6).0[..3], [255, 0, 0], "the new square is drawn");
        assert_eq!(
            last.get_pixel(4, 4).0[..3],
            [255, 0, 0],
            "and the previous frame's square survives, because the frame did not dispose it"
        );
        assert_eq!(last.get_pixel(0, 0).0[..3], [10, 20, 30], "the background is untouched");
    }

    /// **Centred, not trailing.** A pointer moving at a constant speed must be
    /// reported exactly where it was at the frame's own timestamp. A trailing
    /// average — the obvious way to smooth — would report it behind, and every
    /// click in the recording would land next to the button it hit.
    #[test]
    fn smoothing_a_steady_move_adds_no_lag() {
        // 500 points per second, sampled every 5 ms, straight along x.
        let track: Vec<Sample> = (0..400)
            .map(|i| {
                let t = i as f64 * 0.005;
                (t, 100.0 + t * 500.0, 250.0)
            })
            .collect();
        for at in [0.25, 0.5, 1.0, 1.5] {
            let (x, y) = smoothed(&track, at).expect("a sampled track answers");
            assert!(
                (x - (100.0 + at * 500.0)).abs() < 0.5,
                "at {at}s the arrow must be at {}, not {x}",
                100.0 + at * 500.0
            );
            assert!((y - 250.0).abs() < 0.001);
        }
    }

    /// And it must actually smooth: sampling noise on a straight path is
    /// removed rather than reproduced.
    #[test]
    fn smoothing_removes_the_jitter_it_is_there_for() {
        let track: Vec<Sample> = (0..400)
            .map(|i| {
                let t = i as f64 * 0.005;
                // ±3 points of alternating tremor on a straight path.
                let jitter = if i % 2 == 0 { 3.0 } else { -3.0 };
                (t, 100.0 + t * 500.0 + jitter, 250.0)
            })
            .collect();
        let (x, _) = smoothed(&track, 1.0).expect("a sampled track answers");
        let raw = track
            .iter()
            .min_by(|a, b| (a.0 - 1.0).abs().partial_cmp(&(b.0 - 1.0).abs()).unwrap())
            .unwrap()
            .1;
        assert!(
            (x - 600.0).abs() < 0.5,
            "the smoothed position must sit on the true path (600), not at {x}"
        );
        assert!(
            (raw - 600.0).abs() > 2.0,
            "the test's own fixture must actually be noisy, or it proves nothing"
        );
    }

    /// A frame taken before the sampler produced anything still gets a cursor —
    /// the nearest sample — rather than none.
    #[test]
    fn a_frame_outside_the_smoothing_window_falls_back_to_the_nearest_sample() {
        let track = vec![(5.0, 42.0, 43.0)];
        assert_eq!(smoothed(&track, 0.0), Some((42.0, 43.0)));
        assert_eq!(smoothed(&[], 0.0), None);
    }

    /// The arrow is drawn where it was asked for, white-bodied with a dark rim,
    /// and it leaves the rest of the frame alone.
    #[test]
    fn the_cursor_is_a_light_arrow_with_a_dark_rim_at_the_hotspot() {
        let mut image = RgbImage::from_pixel(60, 60, Rgb([120, 120, 120]));
        draw_cursor(&mut image, 20.0, 10.0, 2.0);

        // Down the arrow's left edge, a few pixels below the tip: body.
        let body = image.get_pixel(22, 20).0;
        assert!(
            body.iter().all(|&c| c > 200),
            "the arrow's body must be light, found {body:?}"
        );
        // Just outside that edge: the rim.
        let rim = image.get_pixel(18, 20).0;
        assert!(
            rim.iter().all(|&c| c < 100),
            "the arrow must carry a dark rim so it reads on light UI, found {rim:?}"
        );
        // Far away: untouched.
        assert_eq!(image.get_pixel(55, 55).0, [120, 120, 120]);
        assert_eq!(image.get_pixel(2, 2).0, [120, 120, 120]);
    }

    /// **Sub-pixel placement is the point.** Two frames a third of a pixel apart
    /// must not be identical: rounding the arrow to whole pixels would undo the
    /// smoothing and turn a glide into a stair.
    #[test]
    fn a_third_of_a_pixel_of_travel_changes_the_frame() {
        let render = |x: f32| {
            let mut image = RgbImage::from_pixel(40, 40, Rgb([0, 0, 0]));
            draw_cursor(&mut image, x, 10.0, 1.0);
            image
        };
        let a = render(12.0);
        let b = render(12.33);
        assert_ne!(
            a.as_raw(),
            b.as_raw(),
            "a third of a pixel of movement must show, or the cursor stair-steps"
        );
    }

    /// An arrow that has walked off the frame must not panic or wrap around to
    /// the far edge.
    #[test]
    fn a_cursor_outside_the_frame_is_simply_not_drawn() {
        let blank = RgbImage::from_pixel(20, 20, Rgb([7, 7, 7]));
        for (x, y) in [(-50.0, 5.0), (5.0, -50.0), (100.0, 5.0), (5.0, 100.0)] {
            let mut image = blank.clone();
            draw_cursor(&mut image, x, y, 1.0);
            assert_eq!(image.as_raw(), blank.as_raw(), "nothing is drawn at ({x}, {y})");
        }
    }

    /// Transparency is gone by the time a frame is stored: the recorder writes
    /// RGB, and the IDE's see-through window must land on the theme's colour
    /// rather than on whatever the document's own background happens to be.
    #[test]
    fn a_transparent_capture_is_flattened_onto_the_backdrop_before_it_is_stored() {
        let mut shot = RgbaImage::new(2, 1);
        shot.put_pixel(0, 0, image::Rgba([255, 255, 255, 0])); // fully see-through
        shot.put_pixel(1, 0, image::Rgba([200, 100, 50, 255])); // fully opaque
        let flat = flatten_and_scale(&shot, Color32::from_rgb(20, 30, 40), WIDTH);
        assert_eq!(flat.get_pixel(0, 0).0, [20, 30, 40], "the desktop shows the backdrop");
        assert_eq!(flat.get_pixel(1, 0).0, [200, 100, 50], "opaque pixels are untouched");
    }

    /// A capture wider than [`WIDTH`] is scaled down, keeping its aspect ratio;
    /// one narrower is left alone rather than blown up.
    #[test]
    fn frames_are_scaled_to_the_width_the_guide_renders() {
        let wide = RgbaImage::from_pixel(2560, 1600, image::Rgba([9, 9, 9, 255]));
        let scaled = flatten_and_scale(&wide, Color32::BLACK, WIDTH);
        assert_eq!(scaled.dimensions(), (WIDTH, 563));

        let small = RgbaImage::from_pixel(320, 200, image::Rgba([9, 9, 9, 255]));
        assert_eq!(
            flatten_and_scale(&small, Color32::BLACK, WIDTH).dimensions(),
            (320, 200),
            "a small window is not upscaled into a blurry one"
        );
    }

    /// The report is what the operator reads in the popup, so it must carry the
    /// numbers that decide whether to keep the take.
    #[test]
    fn the_report_names_the_frames_the_length_and_the_size() {
        let movie = Movie {
            captured: 96,
            written: 31,
            seconds: 12.0,
            width: 900,
            height: 563,
            apng: vec![0; 2 * 1024 * 1024],
            stop: Stop::Duration,
        };
        let report = movie.report();
        for expected in ["96 frames", "12.0 s", "8.0 fps", "31 written", "900×563", "2.0 MB"] {
            assert!(report.contains(expected), "the report must say {expected:?}: {report}");
        }
        assert!(
            report.contains("90-second limit"),
            "a recording the operator did not stop must say why it ended: {report}"
        );
        assert!(Stop::Operator.note().is_none(), "an ordinary stop needs no excuse");
    }
}
