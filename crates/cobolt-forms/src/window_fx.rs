// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Window entrance/exit effects (spec 038).
//!
//! Every effect is a transform of the form's STATIC face: the caller hands a
//! `face` closure that paints the whole form face into an arbitrary target
//! rectangle (the shared `paint::draw_control` pipeline both the designer and
//! the run form use), and [`paint_window_fx`] decides geometry, clipping and
//! covers for the current progress `t ∈ 0..=1` (`0` = window absent, `1` =
//! fully shown). Entrances run t 0→1; exits reuse the same math with t 1→0.
//!
//! Masks (radar, iris, blinds, checkerboard) are painted as COVERS in the
//! window background colour over the fully-painted face — egui clip rects
//! are axis-aligned, so angular/circular masks cover rather than clip.
//! MatrixRain is the exception that covers nothing: it paints the face once
//! per falling line, clipped to the ground that line's tail has already
//! passed over, so everything else stays unpainted (see-through). All
//! geometry lives in pure helpers so the unit tests need no egui context.

use egui::{Color32, Pos2, Rect, Vec2};

/// The effect catalogue (spec 038 R4). Ids are stable English kebab-case —
/// they persist in `cobolt.toml` and travel as spawn args.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowEffect {
    #[default]
    None,
    Fade,
    Zoom,
    SlideLeft,
    SlideRight,
    SlideTop,
    SlideBottom,
    ExpandTitleBar,
    RadarWipe,
    IrisWipe,
    Blinds,
    Checkerboard,
    MatrixRain,
    Genie,
}

impl WindowEffect {
    pub const ALL: [WindowEffect; 14] = [
        WindowEffect::None,
        WindowEffect::Fade,
        WindowEffect::Zoom,
        WindowEffect::SlideLeft,
        WindowEffect::SlideRight,
        WindowEffect::SlideTop,
        WindowEffect::SlideBottom,
        WindowEffect::ExpandTitleBar,
        WindowEffect::RadarWipe,
        WindowEffect::IrisWipe,
        WindowEffect::Blinds,
        WindowEffect::Checkerboard,
        WindowEffect::MatrixRain,
        WindowEffect::Genie,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            WindowEffect::None => "none",
            WindowEffect::Fade => "fade",
            WindowEffect::Zoom => "zoom",
            WindowEffect::SlideLeft => "slide-left",
            WindowEffect::SlideRight => "slide-right",
            WindowEffect::SlideTop => "slide-top",
            WindowEffect::SlideBottom => "slide-bottom",
            WindowEffect::ExpandTitleBar => "expand-title-bar",
            WindowEffect::RadarWipe => "radar-wipe",
            WindowEffect::IrisWipe => "iris-wipe",
            WindowEffect::Blinds => "blinds",
            WindowEffect::Checkerboard => "checkerboard",
            WindowEffect::MatrixRain => "matrix-rain",
            WindowEffect::Genie => "genie",
        }
    }

    /// Lenient parse: unknown ids resolve to None so a stale project file
    /// can never crash a host.
    pub fn from_str(value: &str) -> Self {
        let v = value.trim().to_ascii_lowercase();
        Self::ALL
            .into_iter()
            .find(|e| e.as_str() == v)
            .unwrap_or(WindowEffect::None)
    }

    /// Whether this effect can play on a TRANSPARENT window — the form
    /// animating "loose" on the desktop, with no window background and no
    /// title bar behind it.
    ///
    /// True for the effects that only move, scale or fade the form's own
    /// face: whatever they do not paint can simply stay see-through. False
    /// for the mask effects (radar, iris, blinds, checkerboard), which hide
    /// the form by painting COVERS over an already-painted face — egui clip
    /// rects are axis-aligned, so angular masks must cover rather than clip,
    /// and painting "transparent" over pixels cannot erase them (epaint has
    /// one premultiplied blend and no way to write destination alpha).
    ///
    /// MatrixRain qualifies because it never covers anything: each falling
    /// line paints the form only down to its own tail, and its band is a
    /// plain rectangle, so ground no tail has reached is simply not painted.
    pub fn plays_over_desktop(self) -> bool {
        matches!(
            self,
            WindowEffect::Fade
                | WindowEffect::Zoom
                | WindowEffect::SlideLeft
                | WindowEffect::SlideRight
                | WindowEffect::SlideTop
                | WindowEffect::SlideBottom
                | WindowEffect::ExpandTitleBar
                | WindowEffect::Genie
                | WindowEffect::MatrixRain
        )
    }

    /// The progress this effect should be painted at, given the linear
    /// `raw` progress and the configured easing. MatrixRain runs on LINEAR
    /// time whatever the easing says: it schedules its falling lines in real
    /// milliseconds, and an easing on top would stretch that schedule and
    /// make the rain speed up or slow down as it fell.
    pub fn progress(self, easing: Easing, raw: f32) -> f32 {
        match self {
            WindowEffect::MatrixRain => raw.clamp(0.0, 1.0),
            _ => easing.apply(raw),
        }
    }

    /// Per-effect duration bounds in ms. MatrixRain needs a longer window
    /// than the snappy global clamp — its lines arrive in a staggered
    /// schedule and must all land before the end (operator, 2026-07-30:
    /// 1500–4000 ms).
    pub fn duration_bounds(self) -> (u32, u32) {
        match self {
            WindowEffect::MatrixRain => (MATRIX_MIN_MS, MATRIX_MAX_MS),
            _ => (FX_MIN_MS, FX_MAX_MS),
        }
    }
}

/// Easing curves. Monotonic over `0..=1` (unit-tested — a non-monotonic curve
/// would make exits stutter when played reversed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Easing {
    Linear,
    EaseIn,
    #[default]
    EaseOut,
    EaseInOut,
}

impl Easing {
    pub fn as_str(self) -> &'static str {
        match self {
            Easing::Linear => "linear",
            Easing::EaseIn => "ease-in",
            Easing::EaseOut => "ease-out",
            Easing::EaseInOut => "ease-in-out",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "linear" => Easing::Linear,
            "ease-in" => Easing::EaseIn,
            "ease-in-out" => Easing::EaseInOut,
            _ => Easing::EaseOut,
        }
    }

    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
        }
    }
}

/// One direction's effect setting: effect + duration + easing. Formats to and
/// parses from the `id:ms:easing` triple used by `--fx-entrance`/`--fx-exit`
/// spawn args and the project file accessors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxSpec {
    pub effect: WindowEffect,
    pub duration_ms: u32,
    pub easing: Easing,
}

impl Default for FxSpec {
    fn default() -> Self {
        Self {
            effect: WindowEffect::None,
            duration_ms: 600,
            easing: Easing::EaseOut,
        }
    }
}

/// Duration bounds (spec R1): animations stay snappy and can't be configured
/// into a minutes-long lockout.
pub const FX_MIN_MS: u32 = 100;
pub const FX_MAX_MS: u32 = 3000;

/// MatrixRain duration bounds — the fly-through needs room to read, so
/// 1500–4000 ms instead of the global clamp.
pub const MATRIX_MIN_MS: u32 = 1500;
pub const MATRIX_MAX_MS: u32 = 4000;

impl FxSpec {
    pub fn is_active(&self) -> bool {
        self.effect != WindowEffect::None
    }

    pub fn format(&self) -> String {
        format!(
            "{}:{}:{}",
            self.effect.as_str(),
            self.duration_ms,
            self.easing.as_str()
        )
    }

    /// Parse `id:ms:easing`; missing/broken parts fall back to defaults and
    /// the duration clamps into the effect's own bounds.
    pub fn parse(value: &str) -> Self {
        let mut parts = value.splitn(3, ':');
        let effect = WindowEffect::from_str(parts.next().unwrap_or(""));
        let (min_ms, max_ms) = effect.duration_bounds();
        let duration_ms = parts
            .next()
            .and_then(|p| p.trim().parse::<u32>().ok())
            .unwrap_or(600)
            .clamp(min_ms, max_ms);
        let easing = Easing::from_str(parts.next().unwrap_or(""));
        Self {
            effect,
            duration_ms,
            easing,
        }
    }
}

// ── Pure geometry (unit-tested without an egui context) ──────────────────────

/// Zoom: the face grows from the window centre (plan D4).
pub fn zoom_rect(rect: Rect, t: f32) -> Rect {
    let t = t.clamp(0.0, 1.0).max(0.001);
    Rect::from_center_size(rect.center(), rect.size() * t)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideFrom {
    Left,
    Right,
    Top,
    Bottom,
}

/// Slide: the face translates in from an edge.
pub fn slide_rect(rect: Rect, t: f32, from: SlideFrom) -> Rect {
    let t = t.clamp(0.0, 1.0);
    let d = 1.0 - t;
    let offset = match from {
        SlideFrom::Left => Vec2::new(-rect.width() * d, 0.0),
        SlideFrom::Right => Vec2::new(rect.width() * d, 0.0),
        SlideFrom::Top => Vec2::new(0.0, -rect.height() * d),
        SlideFrom::Bottom => Vec2::new(0.0, rect.height() * d),
    };
    rect.translate(offset)
}

/// ExpandFromTitleBar: full width, height grows downward from the top edge.
pub fn expand_title_rect(rect: Rect, t: f32) -> Rect {
    let t = t.clamp(0.0, 1.0).max(0.001);
    Rect::from_min_size(rect.min, Vec2::new(rect.width(), rect.height() * t))
}

/// Radar wipe cover: the polygon hiding the NOT-yet-revealed angular sector
/// (from `t`·2π to 2π, clockwise from 12 o'clock). Empty at t=1.
pub fn radar_cover_points(rect: Rect, t: f32, segments: usize) -> Vec<Pos2> {
    let t = t.clamp(0.0, 1.0);
    if t >= 1.0 {
        return Vec::new();
    }
    let c = rect.center();
    // Radius reaching every corner.
    let r = rect.size().length(); // > half-diagonal, always covers
    let start = t * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
    let end = std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
    let n = segments.max(2);
    let mut pts = Vec::with_capacity(n + 2);
    pts.push(c);
    for i in 0..=n {
        let a = start + (end - start) * (i as f32 / n as f32);
        pts.push(Pos2::new(c.x + r * a.cos(), c.y + r * a.sin()));
    }
    pts
}

/// Iris wipe cover: a triangulated ring between the revealed circle (radius
/// grows with t) and the window bounds. Returned as triangle strips expressed
/// as polygons (quad fans); empty at t=1.
pub fn iris_cover_quads(rect: Rect, t: f32, segments: usize) -> Vec<[Pos2; 4]> {
    let t = t.clamp(0.0, 1.0);
    if t >= 1.0 {
        return Vec::new();
    }
    let c = rect.center();
    let r_in = rect.size().length() * 0.5 * t; // reaches corners at t=1
    let r_out = rect.size().length(); // safely past every corner
    let n = segments.max(8);
    let mut quads = Vec::with_capacity(n);
    for i in 0..n {
        let a0 = std::f32::consts::TAU * (i as f32 / n as f32);
        let a1 = std::f32::consts::TAU * ((i + 1) as f32 / n as f32);
        let p = |r: f32, a: f32| Pos2::new(c.x + r * a.cos(), c.y + r * a.sin());
        quads.push([p(r_in, a0), p(r_out, a0), p(r_out, a1), p(r_in, a1)]);
    }
    quads
}

/// Venetian blinds cover: `n` horizontal strips, each hiding the bottom
/// `(1-t)` share of its slat. Empty at t=1.
pub fn blinds_covers(rect: Rect, t: f32, n: usize) -> Vec<Rect> {
    let t = t.clamp(0.0, 1.0);
    if t >= 1.0 {
        return Vec::new();
    }
    let n = n.max(1);
    let slat_h = rect.height() / n as f32;
    (0..n)
        .map(|i| {
            let top = rect.top() + slat_h * i as f32 + slat_h * t;
            Rect::from_min_max(
                Pos2::new(rect.left(), top),
                Pos2::new(rect.right(), rect.top() + slat_h * (i + 1) as f32),
            )
        })
        .collect()
}

/// Checkerboard cover: cells flip in a staggered diagonal order; each cell is
/// covered while its local progress has not arrived. Empty at t=1.
pub fn checker_covers(rect: Rect, t: f32, cols: usize, rows: usize) -> Vec<Rect> {
    let t = t.clamp(0.0, 1.0);
    if t >= 1.0 {
        return Vec::new();
    }
    let cols = cols.max(1);
    let rows = rows.max(1);
    let cw = rect.width() / cols as f32;
    let ch = rect.height() / rows as f32;
    let max_wave = (cols + rows - 2) as f32;
    let mut covers = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            // Diagonal stagger: earlier waves reveal first.
            let wave = (col + row) as f32 / max_wave.max(1.0);
            // Each cell reveals across a short local window of the timeline.
            let local = ((t * 1.6) - wave * 0.6).clamp(0.0, 1.0);
            if local < 1.0 {
                let cell = Rect::from_min_size(
                    Pos2::new(rect.left() + cw * col as f32, rect.top() + ch * row as f32),
                    Vec2::new(cw, ch),
                );
                // Cover shrinks toward the cell centre as local → 1.
                covers.push(Rect::from_center_size(
                    cell.center(),
                    cell.size() * (1.0 - local),
                ));
            }
        }
    }
    covers
}

/// MatrixRain column count for a window width — bounded so a huge window
/// cannot explode the glyph budget (plan perf constraint).
pub const MATRIX_COL_PX: f32 = 14.0;
pub const MATRIX_MAX_COLS: usize = 160;

pub fn matrix_columns(width: f32) -> usize {
    ((width / MATRIX_COL_PX).ceil() as usize).clamp(1, MATRIX_MAX_COLS)
}

/// MatrixRain choreography (operator, 2026-07-30). The window starts
/// COMPLETELY SEE-THROUGH — no black world, nothing of the form — and every
/// falling line enters from ABOVE the top edge, never mid-screen. Each line
/// owns a vertical band of the window, and it is its END OF TRAIL — the
/// topmost, faintest glyph — that, as it descends over a region, uncovers
/// what stands behind it: the form is painted only down to that tail, so
/// what has not been passed over yet is simply not painted at all.
///
/// Lines arrive staggered, one per beat: the first ones 25 ms apart, then as
/// many as fit, each 10–25 ms behind the last, at their own speeds. The
/// effect takes as long as that schedule needs — its configured duration is a
/// floor, not a ceiling — and every one of them
/// lands on the bottom edge before the animation is out, so the form is
/// complete exactly when the last character leaves the window.
///
/// Glyphs per falling line: the head plus nine dimmer glyphs behind it.
pub const MATRIX_TRAIL_N: usize = 10;

/// The rain's glyph size — 50% larger than the 13 px it started at
/// (operator, 2026-07-30). The row pitch and the band width are derived from
/// it, so a bigger glyph neither crowds the one above it nor overlaps the
/// line beside it.
pub const MATRIX_FONT_PX: f32 = 19.5;
/// Vertical pitch between the glyphs of one falling line.
pub const MATRIX_GLYPH_H: f32 = MATRIX_FONT_PX * 1.23;
/// Width of the band one line owns — its glyph plus breathing room.
pub const MATRIX_BAND_PX: f32 = MATRIX_FONT_PX * 1.46;

/// Share of the timeline a nominal-speed line takes to walk its tail from
/// above the top edge to the bottom (±15% per line). Half again as fast as
/// the first cut of the effect, which took ~0.85 of the timeline.
pub const MATRIX_FALL_SHARE: f32 = 0.55;
/// The slowest line's share (the −15% end), which bounds the start schedule.
pub const MATRIX_FALL_SHARE_MAX: f32 = MATRIX_FALL_SHARE / 0.85;

/// Start stagger between consecutive lines: 25 ms for the first ones,
/// tightening toward 10 ms as more of them pile in.
pub const MATRIX_STAGGER_MAX_MS: f32 = 25.0;
pub const MATRIX_STAGGER_MIN_MS: f32 = 10.0;

/// Lines per band of width — 1.5 is half again the rain of the first cut
/// (operator, 2026-07-31: 100% was the ask, 50% the accepted price for
/// keeping ONE line per beat; launching them in pairs was tried and rejected).
/// The band still holds the glyph: at 19.5 px monospace the advance is ~12 px
/// and the denser band ~19 px, so columns stand side by side instead of
/// smearing into each other.
pub const MATRIX_DENSITY: f32 = 1.5;

/// The time this effect actually needs, which may exceed the configured
/// duration (operator, 2026-07-31: "mesmo que ultrapassasse o tempo total").
///
/// One line per beat at 25–50 ms is a schedule of its own length: `n` lines
/// cannot start in less than `n × 37.5 ms`, and each still has to fall. More
/// lines and a wider beat both push that out, so with the duration fixed the
/// only way to honour them is to drop lines. The duration gives way instead —
/// the configured value is the FLOOR, never the ceiling — and the count keeps
/// what the width asked for. Bounded so a very wide window cannot turn an
/// entrance into a minute of rain.
pub const MATRIX_HARD_MAX_MS: u32 = 6_000;

pub fn matrix_effective_duration_ms(width: f32, configured_ms: u32) -> u32 {
    let wanted = matrix_lines_by_width(width);
    configured_ms
        .max(matrix_schedule_ms(wanted))
        .min(MATRIX_HARD_MAX_MS)
}

/// Lines the width alone asks for, before the clock has its say.
fn matrix_lines_by_width(width: f32) -> usize {
    ((width / MATRIX_BAND_PX * MATRIX_DENSITY).round() as usize).clamp(3, 72)
}

/// The timeline `n` lines need at the 10–25 ms beat, one line per beat: the
/// starts, plus the room the last one needs to fall.
fn matrix_schedule_ms(n: usize) -> u32 {
    let avg_stagger = (MATRIX_STAGGER_MIN_MS + MATRIX_STAGGER_MAX_MS) * 0.5;
    let starts_ms = n.saturating_sub(1) as f32 * avg_stagger;
    (starts_ms / (1.0 - MATRIX_FALL_SHARE_MAX)).ceil() as u32
}

/// How many falling lines the window gets — each owns one reveal band. One
/// per ~19 px of width, capped for the paint budget (each band paints the
/// form face under its own clip), and never more than the start schedule can
/// launch while every line still lands before the animation ends.
pub fn matrix_line_count(width: f32, duration_ms: u32) -> usize {
    let by_width = matrix_lines_by_width(width);
    // The width asks; the clock answers. Past the stretch ceiling the count
    // gives way instead of the invariant — every line still lands before the
    // effect ends.
    let window_ms = (1.0 - MATRIX_FALL_SHARE_MAX) * duration_ms.max(1) as f32;
    let avg_stagger = (MATRIX_STAGGER_MIN_MS + MATRIX_STAGGER_MAX_MS) * 0.5;
    let by_time = (window_ms / avg_stagger).floor().max(3.0) as usize;
    by_width.min(by_time)
}

/// Line `k` of `n`: when it starts and how long its tail takes to cross the
/// window, both as timeline fractions. Starts accumulate a stagger that runs
/// from [`MATRIX_STAGGER_MAX_MS`] down to [`MATRIX_STAGGER_MIN_MS`], so the
/// first lines arrive alone and the rest pile in; speeds vary ±15%; and the
/// start is clamped so even the slowest line lands by `t = 1`.
pub fn matrix_line_timing(hash: u32, k: usize, n: usize, duration_ms: u32) -> (f32, f32) {
    let speed = 0.85 + ((hash & 0xFFFF) as f32 / 65535.0) * 0.30; // 0.85..=1.15
    let dur = (MATRIX_FALL_SHARE / speed).min(1.0);
    // One line per beat: each start is its own, 25–50 ms behind the one
    // before it.
    // Sum of the first k increments, each shrinking linearly with its index.
    let span = (n.max(2) - 1) as f32;
    let drop = MATRIX_STAGGER_MAX_MS - MATRIX_STAGGER_MIN_MS;
    let sum_to = |i: f32| i * MATRIX_STAGGER_MAX_MS - drop * (i * (i - 1.0) * 0.5) / span;
    // On a long animation the window is wider than the schedule needs (the
    // line count is capped by the window's width), which would leave the form
    // revealed but still waiting. Stretch the stagger to fill the room
    // instead — never compress it, since `matrix_line_count` already sized
    // the schedule to fit.
    let room_ms = (1.0 - MATRIX_FALL_SHARE_MAX) * duration_ms.max(1) as f32;
    let needed_ms = sum_to(span).max(1.0);
    let stretch = (room_ms / needed_ms).max(1.0);
    let ms = sum_to(k as f32) * stretch;
    let delay = (ms / duration_ms.max(1) as f32).clamp(0.0, (1.0 - dur).max(0.0));
    (delay, dur)
}

/// How far down its band the reveal front — the end of trail — has come:
/// 0 before the line has entered, 1 once the tail has reached the bottom
/// edge and the band stands fully uncovered.
pub fn matrix_wipe_front(t: f32, delay: f32, dur: f32) -> f32 {
    ((t.clamp(0.0, 1.0) - delay) / dur.max(0.001)).clamp(0.0, 1.0)
}

/// Genie approximation rows: horizontal slices of the source face, each
/// mapped to a target strip squashed toward the bottom-right corner with a
/// quadratic bend. Returns `(source_clip, target_rect)` pairs; at t=1 each
/// row maps onto itself.
pub fn genie_rows(rect: Rect, t: f32, n: usize) -> Vec<(Rect, Rect)> {
    let t = t.clamp(0.0, 1.0);
    let n = n.clamp(2, 16);
    let row_h = rect.height() / n as f32;
    let mut rows = Vec::with_capacity(n);
    for i in 0..n {
        let y0 = rect.top() + row_h * i as f32;
        let src = Rect::from_min_size(
            Pos2::new(rect.left(), y0),
            Vec2::new(rect.width(), row_h),
        );
        // Depth: lower rows keep position longest (they "leave last").
        let depth = 1.0 - (i as f32 / (n - 1) as f32); // 1 top … 0 bottom
        let squash = (1.0 - t) * depth;
        // Width shrinks and the strip drifts toward the bottom-right corner,
        // with a quadratic bend so the silhouette curves like the genie.
        let w = rect.width() * (1.0 - squash * 0.9);
        let x = rect.left() + (rect.width() - w) * (squash * squash + squash) * 0.5;
        let y = y0 + (rect.bottom() - y0) * squash * 0.35;
        rows.push((
            src,
            Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, row_h * (1.0 - squash * 0.3))),
        ));
    }
    rows
}

// ── Painting ────────────────────────────────────────────────────────────────

/// Deterministic tiny PRNG (xorshift) so MatrixRain is reproducible per
/// window seed — no rand dependency, no global state.
fn xorshift(state: &mut u32) -> u32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    x
}

/// Full-avalanche mix (murmur3 fmix32): every input bit flips ~half the
/// output bits, so ADJACENT column indices produce independent values —
/// plain xorshift over near-identical seeds made neighbouring columns fall
/// visibly in pairs (operator report, 2026-07-30).
fn mix32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 16;
    x
}

/// Classic Matrix glyph pool: katakana + digits. No easter eggs (spec Q2).
///
/// `katakana` is how many of the U+30A1..=U+30FA block may be drawn: the
/// FULL block at reading sizes, a short slice once the camera has magnified
/// the glyphs (rasterising 90 distinct 76 px glyphs would blow up the font
/// atlas), and **zero** on a host whose monospace family cannot render
/// katakana at all — the rain then falls in digits instead of tofu boxes.
pub fn matrix_glyph(rng: &mut u32, katakana: u32) -> char {
    const DIGITS: &[char] = &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];
    let roll = xorshift(rng);
    if katakana == 0 || roll % 4 == 0 {
        DIGITS[(roll as usize / 4) % DIGITS.len()]
    } else {
        // Katakana block U+30A1..=U+30FA.
        char::from_u32(0x30A1 + (roll % katakana.min(0x5A))).unwrap_or('ｱ')
    }
}

/// Paint the window face under `effect` at progress `t` (already eased by the
/// caller via [`Easing::apply`]). `bg` is the window background used for
/// mask covers; `face` paints the full form face into the given target rect
/// with the given painter (whose clip may be narrowed per call). `seed`
/// keys MatrixRain's deterministic rain; `time` (seconds) animates it.
#[allow(clippy::too_many_arguments)]
pub fn paint_window_fx(
    painter: &egui::Painter,
    rect: Rect,
    bg: Color32,
    t: f32,
    effect: WindowEffect,
    seed: u32,
    time: f64,
    // The window is see-through behind the effect (the form plays loose on
    // the desktop). Anything the effect would paint in the window background
    // colour is then left unpainted instead — and a fade dims the face's own
    // opacity rather than veiling it with the background.
    transparent: bool,
    // The animation's configured length. MatrixRain schedules its falling
    // lines in real milliseconds (25 ms apart at first, then 10–25 ms), so it
    // needs to know how much wall clock the timeline stands for.
    duration_ms: u32,
    face: &mut dyn FnMut(&egui::Painter, Rect),
) {
    let t = t.clamp(0.0, 1.0);
    match effect {
        WindowEffect::None => face(painter, rect),
        WindowEffect::Fade => {
            if transparent {
                let mut faded = painter.clone();
                faded.set_opacity(t);
                face(&faded, rect);
            } else {
                face(painter, rect);
                let a = ((1.0 - t) * 255.0) as u8;
                if a > 0 {
                    painter.rect_filled(rect, 0.0, bg.gamma_multiply(a as f32 / 255.0));
                }
            }
        }
        WindowEffect::Zoom => {
            if !transparent {
                painter.rect_filled(rect, 0.0, bg);
            }
            face(painter, zoom_rect(rect, t));
        }
        WindowEffect::SlideLeft
        | WindowEffect::SlideRight
        | WindowEffect::SlideTop
        | WindowEffect::SlideBottom => {
            let from = match effect {
                WindowEffect::SlideLeft => SlideFrom::Left,
                WindowEffect::SlideRight => SlideFrom::Right,
                WindowEffect::SlideTop => SlideFrom::Top,
                _ => SlideFrom::Bottom,
            };
            if !transparent {
                painter.rect_filled(rect, 0.0, bg);
            }
            let clipped = painter.with_clip_rect(rect);
            face(&clipped, slide_rect(rect, t, from));
        }
        WindowEffect::ExpandTitleBar => {
            if !transparent {
                painter.rect_filled(rect, 0.0, bg);
            }
            let target = expand_title_rect(rect, t);
            let clipped = painter.with_clip_rect(target);
            face(&clipped, target);
        }
        WindowEffect::RadarWipe => {
            face(painter, rect);
            let pts = radar_cover_points(rect, t, 64);
            if pts.len() >= 3 {
                // The sector fan is not convex as a whole; triangulate from
                // the centre (pts[0]) so every emitted polygon is convex.
                let clipped = painter.with_clip_rect(rect);
                for i in 1..pts.len() - 1 {
                    clipped.add(egui::Shape::convex_polygon(
                        vec![pts[0], pts[i], pts[i + 1]],
                        bg,
                        egui::Stroke::NONE,
                    ));
                }
            }
        }
        WindowEffect::IrisWipe => {
            face(painter, rect);
            let clipped = painter.with_clip_rect(rect);
            for q in iris_cover_quads(rect, t, 64) {
                clipped.add(egui::Shape::convex_polygon(
                    q.to_vec(),
                    bg,
                    egui::Stroke::NONE,
                ));
            }
        }
        WindowEffect::Blinds => {
            face(painter, rect);
            for r in blinds_covers(rect, t, 12) {
                painter.rect_filled(r, 0.0, bg);
            }
        }
        WindowEffect::Checkerboard => {
            face(painter, rect);
            for r in checker_covers(rect, t, 10, 8) {
                painter.rect_filled(r, 0.0, bg);
            }
        }
        WindowEffect::MatrixRain => {
            // The face is NOT painted up front here: each falling line paints
            // the form only down to its own tail, so everything no tail has
            // reached yet stays see-through instead of hidden under black.
            let _ = (bg, time, transparent); // rain is t-driven, world is empty
            paint_matrix_rain(painter, rect, t, seed, duration_ms, face);
        }
        WindowEffect::Genie => {
            if !transparent {
                painter.rect_filled(rect, 0.0, bg);
            }
            for (src, target) in genie_rows(rect, t, 16) {
                // Clip to the TARGET strip and paint the whole face scaled so
                // this row's share lands inside the strip.
                let scale_y = target.height() / src.height().max(0.001);
                let scale_x = target.width() / rect.width().max(0.001);
                let full_h = rect.height() * scale_y;
                let offset_y = (src.top() - rect.top()) * scale_y;
                let face_rect = Rect::from_min_size(
                    Pos2::new(target.left(), target.top() - offset_y),
                    Vec2::new(rect.width() * scale_x, full_h),
                );
                let clipped = painter.with_clip_rect(target);
                face(&clipped, face_rect);
            }
        }
    }
}

/// The rain (operator choreography, 2026-07-30). The window opens BLACK —
/// always black, on any theme — and the rain falls CONTINUOUSLY (wall-clock
/// `time` drives it, so it rains for however long the animation lasts,
/// 1500–4000 ms). Nothing of the form shows until the camera starts moving
/// at t = 0.5. From there the camera accelerates forward: every column, at
/// its own depth, rushes outward past the observer, growing as it comes.
/// A column's home strip of black is lifted in step with that column's
/// flight off the window — the form is uncovered strip by strip, by the
/// passage of the characters themselves, the outer strips first and the
/// ones dead ahead last, complete at t = 1 when the last column has left
/// the frame. The characters themselves are NEVER faded out: they keep
/// their brightness and leave only by flying out of the window.
fn paint_matrix_rain(
    painter: &egui::Painter,
    rect: Rect,
    t: f32,
    seed: u32,
    duration_ms: u32,
    face: &mut dyn FnMut(&egui::Painter, Rect),
) {
    let clipped = painter.with_clip_rect(rect);
    let n = matrix_line_count(rect.width(), duration_ms);
    let band_w = rect.width() / n as f32;
    let glyph_h = MATRIX_GLYPH_H;
    let trail_px = glyph_h * MATRIX_TRAIL_N as f32;
    // The tail enters one trail ABOVE the top edge, so a line arrives
    // gradually from off-screen instead of appearing mid-window, and lands
    // on the bottom edge at the end of its run.
    let travel = rect.height() + trail_px;
    let font = egui::FontId::monospace(MATRIX_FONT_PX);
    // A host with no katakana in its monospace family would draw tofu boxes
    // (operator report, 2026-07-30: correct in the IDE preview, boxes in the
    // run form) — probe once per frame and fall back to digits.
    let katakana = if painter.ctx().fonts_mut(|f| f.has_glyph(&font, 'ア')) {
        0x5A
    } else {
        0
    };

    // Lines start in a scattered order — left-to-right would march a curtain
    // across the window — so rank the bands by a well-mixed hash.
    let mut order: Vec<(u32, usize)> = (0..n)
        .map(|b| (mix32(seed ^ mix32(b as u32 + 1)), b))
        .collect();
    order.sort_unstable();

    let mut fronts = vec![rect.top() - trail_px; n];
    for (k, &(hash, band)) in order.iter().enumerate() {
        let (delay, dur) = matrix_line_timing(hash, k, n, duration_ms);
        fronts[band] = rect.top() - trail_px + matrix_wipe_front(t, delay, dur) * travel;
    }

    // Pass 1 — the form, painted ONLY down to each band's tail. There is no
    // cover to erase: what no tail has passed over yet is simply never
    // painted, so the window stays see-through there.
    for (band, &front) in fronts.iter().enumerate() {
        if front <= rect.top() {
            continue; // this line has not entered the window yet
        }
        let band_rect = Rect::from_min_max(
            Pos2::new(rect.left() + band_w * band as f32, rect.top()),
            Pos2::new(
                rect.left() + band_w * (band + 1) as f32,
                front.min(rect.bottom()),
            ),
        );
        face(&painter.with_clip_rect(band_rect), rect);
    }

    // Pass 2 — the rain itself. A line is drawn from its TAIL downward:
    // `k = 0` is the topmost, faintest glyph, sitting exactly on the reveal
    // front, and the bright head leads below it over untouched ground.
    for (band, &front) in fronts.iter().enumerate() {
        let hash = mix32(seed ^ mix32(band as u32 + 1));
        if front >= rect.bottom() {
            continue; // done: this line has left through the bottom edge
        }
        let gx = rect.left() + band_w * (band as f32 + 0.5);
        for k in 0..MATRIX_TRAIL_N {
            let y = front + glyph_h * k as f32;
            if y < rect.top() || y > rect.bottom() {
                continue; // still entering from above, or already gone
            }
            // Glyph choice keys on the CELL the glyph currently occupies, so
            // characters mutate naturally while falling.
            let cell = (y - rect.top()).div_euclid(glyph_h) as i32;
            let mut grng = mix32(hash ^ mix32(cell as u32).wrapping_add(k as u32 * 97)) | 1;
            let ch = matrix_glyph(&mut grng, katakana);
            let color = if k + 1 == MATRIX_TRAIL_N {
                Color32::from_rgb(200, 255, 200) // the head, leading below
            } else {
                let fade = (k + 1) as f32 / MATRIX_TRAIL_N as f32;
                Color32::from_rgba_unmultiplied(40, 200, 80, (fade * 230.0) as u8)
            };
            clipped.text(
                Pos2::new(gx, y),
                egui::Align2::CENTER_TOP,
                ch,
                font.clone(),
                color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 038 (operator, 2026-07-30): an entrance plays with no chrome, and the
    /// effects that only move / scale / fade the form's own face additionally
    /// get a SEE-THROUGH window so the form animates loose on the desktop.
    /// The mask effects hide the form by painting covers — "transparent"
    /// cannot erase already-painted pixels — and MatrixRain's black world is
    /// the effect itself, so both keep an opaque window.
    #[test]
    fn only_face_effects_play_over_the_desktop() {
        for e in [
            WindowEffect::Fade,
            WindowEffect::Zoom,
            WindowEffect::SlideLeft,
            WindowEffect::SlideRight,
            WindowEffect::SlideTop,
            WindowEffect::SlideBottom,
            WindowEffect::ExpandTitleBar,
            WindowEffect::Genie,
        ] {
            assert!(e.plays_over_desktop(), "{e:?} should play over the desktop");
        }
        // MatrixRain qualifies too: it covers nothing, it just paints the
        // form down to each falling line's tail.
        assert!(WindowEffect::MatrixRain.plays_over_desktop());
        for e in [
            WindowEffect::None,
            WindowEffect::RadarWipe,
            WindowEffect::IrisWipe,
            WindowEffect::Blinds,
            WindowEffect::Checkerboard,
        ] {
            assert!(!e.plays_over_desktop(), "{e:?} needs an opaque window");
        }
        let over: Vec<&str> = WindowEffect::ALL
            .into_iter()
            .filter(|e| e.plays_over_desktop())
            .map(|e| e.as_str())
            .collect();
        println!("plays over the desktop: {}", over.join(", "));
    }

    #[test]
    fn effect_ids_round_trip_and_unknown_is_none() {
        for e in WindowEffect::ALL {
            assert_eq!(WindowEffect::from_str(e.as_str()), e, "{e:?}");
            println!("effect id: {:?} ↔ {}", e, e.as_str());
        }
        assert_eq!(WindowEffect::from_str("wormhole"), WindowEffect::None);
        assert_eq!(WindowEffect::from_str("  ZOOM "), WindowEffect::Zoom);
    }

    #[test]
    fn fx_spec_parses_and_clamps() {
        let s = FxSpec::parse("matrix-rain:1600:ease-out");
        assert_eq!(s.effect, WindowEffect::MatrixRain);
        assert_eq!(s.duration_ms, 1600);
        assert_eq!(s.easing, Easing::EaseOut);
        assert_eq!(FxSpec::parse(&s.format()), s, "format→parse round-trip");
        // MatrixRain has its own duration band (1500–4000 ms).
        assert_eq!(FxSpec::parse("matrix-rain:600").duration_ms, MATRIX_MIN_MS);
        assert_eq!(FxSpec::parse("matrix-rain:9999").duration_ms, MATRIX_MAX_MS);
        // Broken/out-of-range input degrades safely.
        let d = FxSpec::parse("zoom:99999");
        assert_eq!(d.duration_ms, FX_MAX_MS);
        let n = FxSpec::parse("");
        assert_eq!(n.effect, WindowEffect::None);
        println!("spec round-trip: {}", s.format());
    }

    #[test]
    fn easings_are_monotonic_and_anchored() {
        for e in [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
        ] {
            assert!(e.apply(0.0).abs() < 1e-6, "{e:?} starts at 0");
            assert!((e.apply(1.0) - 1.0).abs() < 1e-6, "{e:?} ends at 1");
            let mut prev = -1.0_f32;
            for i in 0..=100 {
                let v = e.apply(i as f32 / 100.0);
                assert!(v >= prev - 1e-6, "{e:?} not monotonic at {i}");
                prev = v;
            }
            println!("easing {:?}: mid = {:.3}", e, e.apply(0.5));
        }
        // MatrixRain ignores the easing: its own choreography sets the
        // pacing, so the halfway point stays at half the wall time.
        for e in [Easing::EaseIn, Easing::EaseOut, Easing::EaseInOut] {
            assert_eq!(WindowEffect::MatrixRain.progress(e, 0.5), 0.5, "{e:?}");
            assert_eq!(
                WindowEffect::Fade.progress(e, 0.5),
                e.apply(0.5),
                "other effects still ease"
            );
        }
    }

    #[test]
    fn mask_covers_shrink_to_nothing_at_t1() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 480.0));
        // t=0 fully covered, t=1 empty; area decreases monotonically-ish at
        // the sampled points for every mask family.
        let blinds_area = |t: f32| -> f32 {
            blinds_covers(rect, t, 12)
                .iter()
                .map(|r| r.area())
                .sum()
        };
        assert!((blinds_area(0.0) - rect.area()).abs() < 1.0, "blinds t=0 covers all");
        assert!(blinds_area(0.5) < blinds_area(0.0));
        assert_eq!(blinds_covers(rect, 1.0, 12).len(), 0);

        // Checker covers SHRINK before they disappear: area falls from the
        // start, the cell count only drops once cells fully reveal.
        let checker_area = |t: f32| -> f32 {
            checker_covers(rect, t, 10, 8).iter().map(|r| r.area()).sum()
        };
        let checker_n = |t: f32| checker_covers(rect, t, 10, 8).len();
        assert_eq!(checker_n(0.0), 80, "checker t=0 all cells covered");
        assert!(checker_area(0.5) < checker_area(0.0));
        assert!(checker_n(0.8) < 80, "cells fully revealed by t=0.8");
        assert_eq!(checker_n(1.0), 0);

        assert!(radar_cover_points(rect, 0.0, 64).len() > 3);
        assert!(radar_cover_points(rect, 1.0, 64).is_empty());

        assert_eq!(iris_cover_quads(rect, 1.0, 64).len(), 0);
        assert!(!iris_cover_quads(rect, 0.3, 64).is_empty());
        println!(
            "cover sanity: blinds area {:.0}→{:.0}→0, checker area {:.0}→{:.0}, cells {}→{}→0",
            blinds_area(0.0),
            blinds_area(0.5),
            checker_area(0.0),
            checker_area(0.5),
            checker_n(0.0),
            checker_n(0.8)
        );
    }

    #[test]
    fn geometry_anchors_at_t_extremes() {
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(400.0, 300.0));
        assert!((zoom_rect(rect, 1.0).area() - rect.area()).abs() < 1.0);
        assert!(zoom_rect(rect, 0.1).area() < rect.area() * 0.02);
        assert_eq!(slide_rect(rect, 1.0, SlideFrom::Left), rect);
        assert!((expand_title_rect(rect, 1.0).height() - rect.height()).abs() < 0.5);
        let rows = genie_rows(rect, 1.0, 16);
        for (src, dst) in &rows {
            assert!((src.left() - dst.left()).abs() < 0.5, "genie t=1 identity x");
            assert!((src.top() - dst.top()).abs() < 0.5, "genie t=1 identity y");
        }
        println!("anchors: zoom/slide/expand/genie identity at t=1 hold");
    }

    #[test]
    fn matrix_columns_are_capped() {
        assert_eq!(matrix_columns(140.0), 10);
        assert_eq!(matrix_columns(100_000.0), MATRIX_MAX_COLS);
        assert_eq!(matrix_columns(1.0), 1);
        // Glyphs come from the classic pool only: katakana block or digits.
        let mut rng = 42u32;
        for _ in 0..200 {
            let ch = matrix_glyph(&mut rng, 0x5A);
            let ok = ch.is_ascii_digit()
                || (0x30A1..=0x30FA).contains(&(ch as u32))
                || ch == 'ｱ';
            assert!(ok, "glyph {ch:?} outside the classic pool");
        }
        // A host without katakana rains DIGITS — never tofu boxes.
        let mut rng = 7u32;
        for _ in 0..200 {
            let ch = matrix_glyph(&mut rng, 0);
            assert!(ch.is_ascii_digit(), "no-katakana host must rain digits");
        }
        // Magnified glyphs come from a short slice of the block (atlas cap).
        let mut rng = 9u32;
        for _ in 0..200 {
            let ch = matrix_glyph(&mut rng, 16);
            let ok = ch.is_ascii_digit() || (0x30A1..0x30A1 + 16).contains(&(ch as u32));
            assert!(ok, "glyph {ch:?} outside the magnified slice");
        }
        println!(
            "matrix: 140px → {} cols, capped at {}",
            matrix_columns(140.0),
            MATRIX_MAX_COLS
        );
    }

    /// Operator choreography (2026-07-30): no line may be on screen at the
    /// start — each enters from ABOVE the top edge — the first ones arrive
    /// 25 ms apart and the rest pile in 10–25 ms behind each other, at their
    /// own speeds, and every one of them lands on the bottom edge before the
    /// animation is out. The reveal follows each line's END OF TRAIL down its
    /// band, progressively, never all at once.
    #[test]
    fn matrix_lines_enter_from_the_top_and_all_land_in_time() {
        for configured_ms in [MATRIX_MIN_MS, 2000, MATRIX_MAX_MS] {
            let width = 936.0_f32;
            // The width sets the line count; the duration gives way to fit the
            // 25–50 ms beat, so the schedule is read against the EFFECTIVE
            // duration the host actually plays.
            let duration_ms = matrix_effective_duration_ms(width, configured_ms);
            let n = matrix_line_count(width, duration_ms);
            assert!(n >= 3, "at least a few lines ({n})");
            let mut starts = Vec::new();
            let mut durs = Vec::new();
            for k in 0..n {
                let hash = mix32(0xBADC0DE ^ mix32(k as u32 + 1));
                let (delay, dur) = matrix_line_timing(hash, k, n, duration_ms);

                // Nothing is uncovered before that line has entered, and at
                // t = 0 no line is on screen at all.
                assert_eq!(matrix_wipe_front(0.0, delay, dur), 0.0, "line {k} at t=0");
                if delay > 0.0 {
                    assert_eq!(matrix_wipe_front(delay, delay, dur), 0.0, "line {k} start");
                }

                // Progressive and monotonic — never a jump to fully revealed.
                let mut prev = 0.0_f32;
                let mut partials = 0;
                for i in 0..=200 {
                    let f = matrix_wipe_front(i as f32 / 200.0, delay, dur);
                    assert!(f >= prev, "line {k} front went back up");
                    if f > 0.0 && f < 1.0 {
                        partials += 1;
                    }
                    prev = f;
                }
                assert!(partials >= 50, "line {k} uncovers in a jump ({partials})");

                // Every line lands before the animation is out.
                assert!(
                    delay + dur <= 1.0 + 1e-4,
                    "line {k} would still be falling at the end ({delay} + {dur})"
                );
                assert!(
                    matrix_wipe_front(1.0, delay, dur) >= 0.999,
                    "line {k} had not landed at the end"
                );
                starts.push(delay * duration_ms as f32);
                durs.push(dur);
            }

            // Starts are staggered one line per beat: the first gap is the
            // widest and they tighten from there. The band is 25→10 ms,
            // stretched (never compressed)
            // when a long animation leaves room to spare, so the last line
            // still lands as the animation ends instead of leaving the form
            // revealed but waiting.
            let gaps: Vec<f32> = starts.windows(2).map(|w| w[1] - w[0]).collect();
            let stretch = gaps[0] / MATRIX_STAGGER_MAX_MS;
            assert!(stretch >= 1.0 - 1e-3, "stagger is never compressed");
            for (i, g) in gaps.iter().enumerate() {
                let lo = MATRIX_STAGGER_MIN_MS * stretch - 0.51;
                let hi = MATRIX_STAGGER_MAX_MS * stretch + 0.51;
                assert!(
                    (lo..=hi).contains(g),
                    "gap {i} of {g:.1} ms outside the 10–25 ms band (×{stretch:.2})"
                );
                if i > 0 {
                    assert!(*g <= gaps[i - 1] + 1e-3, "stagger must tighten, not widen");
                }
            }
            // No dead tail: the reveal finishes as the animation ends. The
            // bound is not 1.0 because the last line to START may be a fast
            // one — the schedule is stretched against the SLOWEST possible
            // span so that no line can ever overrun the end. Lines share
            // beats, so the last line to land is not the last index.
            let last_landing = starts
                .iter()
                .zip(durs.iter())
                .map(|(s, d)| s / duration_ms as f32 + d)
                .fold(0.0_f32, f32::max);
            assert!(
                last_landing > 0.80,
                "reveal finished at {last_landing:.2} of the timeline — dead tail"
            );

            // Speeds vary, and are half again faster than the effect's first
            // cut (which took ~0.85 of the timeline per line).
            let (min_d, max_d) = durs.iter().fold((f32::MAX, 0.0_f32), |(lo, hi), &d| {
                (lo.min(d), hi.max(d))
            });
            assert!(max_d - min_d > 0.01, "lines must not share one speed");
            assert!(max_d <= MATRIX_FALL_SHARE_MAX + 1e-4, "slowest line bounded");
            // Nominal speed is half again the effect's first cut, which took
            // 0.85 of the timeline to walk one line down the window.
            assert!(
                MATRIX_FALL_SHARE <= 0.85 / 1.45,
                "lines must be ~50% faster than the old 0.85 share"
            );
            println!(
                "matrix {duration_ms} ms: {n} lines, starts {:.0}..{:.0} ms, spans {min_d:.2}..{max_d:.2} of the timeline",
                starts[0],
                starts[n - 1]
            );
        }
    }
}


#[cfg(test)]
mod matrix_density_tests {
    use super::*;

    /// Half again the lines of the first cut, one per 10–25 ms beat, and the
    /// duration gives way to fit them — never the count.
    #[test]
    fn the_duration_stretches_to_fit_the_lines() {
        for width in [640.0_f32, 936.0, 1440.0, 1920.0] {
            let n = matrix_line_count(width, MATRIX_MIN_MS);
            // The band still holds a 19.5 px monospace glyph (~12 px advance).
            assert!(
                width / n as f32 >= 12.0 || n == 72,
                "{n} lines over {width} px smears the columns"
            );
            for configured in [MATRIX_MIN_MS, 2000, MATRIX_MAX_MS] {
                let eff = matrix_effective_duration_ms(width, configured);
                assert!(eff >= configured, "the configured duration is the floor");
                assert!(eff <= MATRIX_HARD_MAX_MS, "bounded: {eff} ms");
                // Every line still lands inside the (stretched) timeline, and
                // the reveal finishes as it ends.
                let mut last = 0.0_f32;
                for k in 0..n {
                    let hash = mix32(0x5EED ^ mix32(k as u32 + 1));
                    let (delay, dur) = matrix_line_timing(hash, k, n, eff);
                    assert!(
                        delay + dur <= 1.0 + 1e-4,
                        "line {k} still falling at the end ({delay} + {dur})"
                    );
                    last = last.max(delay + dur);
                }
                assert!(
                    last > 0.80,
                    "reveal finished at {last:.2} of the timeline — dead tail"
                );
            }
        }
    }
}


