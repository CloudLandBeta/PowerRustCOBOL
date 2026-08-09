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
    TransporterII,
}

impl WindowEffect {
    pub const ALL: [WindowEffect; 15] = [
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
        WindowEffect::TransporterII,
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
            WindowEffect::TransporterII => "transporter-ii",
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
    /// Transporter qualifies for the same reason from the other direction: it
    /// dims the face's own opacity and ADDS beam and sparkles on top, so a
    /// half-materialised form is genuinely half-there rather than veiled.
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
                | WindowEffect::TransporterII
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
            // Transporter II is choreographed against its own 4 s clock: an
            // easing on top would slide the beam hand-over and the fade-out
            // off the beats they are cut to, so the sequence keeps real time
            // for the same reason the rain does.
            WindowEffect::TransporterII => raw.clamp(0.0, 1.0),
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
            // Transporter II is a choreographed two-phase sequence cut to a
            // fixed clock, not a wipe that can be sped up or slowed down: its
            // beam travel, hand-over and fade-out are all fractions of the
            // same 4 s. Offered at exactly that length — min == max, so the
            // settings spinner has nothing to adjust.
            WindowEffect::TransporterII => (BEAM_MS, BEAM_MS),
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

// ── Transporter II ───────────────────────────────────────────────────────────
//
// A cinematic materialisation reveal, in two phases over a fixed 4 s:
//
//   Phase 1 — two thin horizontal beams start overlapped at the vertical
//   centre, each half the form's width. They separate, one climbing to the top
//   edge and one falling to the bottom, and the space opening between them
//   fills with a dense cloud of shimmering white-and-yellow particles.
//
//   Phase 2 — as the horizontal beams land on the edges they fade out, and two
//   FULL-HEIGHT vertical beams fade in at the horizontal centre. Those sweep
//   outward to the left and right edges, and the form is revealed in the band
//   widening between them; the particle cloud dissolves as each beam passes
//   over it. Through the last stretch the particles, the glow and the beams
//   themselves ease down to nothing, so the light is gone exactly as the beams
//   reach the borders and the finished form stands alone.
//
// Nothing here is a solid fill. Every beam is a stack of translucent strips
// under a bell falloff (epaint has no gradient shader and no blur, so the
// gradient IS the stack) with a wide dim bloom around it, white at the core
// and warm yellow at the flanks. Reversed — an exit runs t 1→0 — the same math
// dematerialises the form: the reveal band closes, the cloud gathers, and the
// horizontal beams converge back to the centre line.

/// The effect's fixed length. It is a choreographed sequence, not a wipe: the
/// phases are cut to this clock, so it is offered at exactly one duration
/// rather than a band (the settings spinner has nothing to adjust).
pub const BEAM_MS: u32 = 4_000;

// Phase 1 — the horizontal pair.
const H_IN: (f32, f32) = (0.00, 0.05); // bloom in, overlapped at the centre
const H_TRAVEL: (f32, f32) = (0.04, 0.50); // centre → top and bottom edges
const H_OUT: (f32, f32) = (0.46, 0.56); // fade as they land

// The particle field, opening with the gap between the horizontal beams.
const FIELD_IN: (f32, f32) = (0.04, 0.50);

// Phase 2 — the vertical pair.
const V_IN: (f32, f32) = (0.48, 0.58); // fade in at the horizontal centre
const V_TRAVEL: (f32, f32) = (0.56, 0.97); // centre → left and right edges

/// The closing stretch: particles, glow and beams ease to nothing, reaching
/// exactly zero as the vertical beams arrive at the borders.
const FADE_OUT: (f32, f32) = (0.82, 1.00);

/// Beam thickness as a fraction of the form's shorter side, and how far the
/// bloom reaches past the core.
const BEAM_THICK: f32 = 0.055;
const BEAM_BLOOM: f32 = 4.0;
/// Strips per beam. The gradient is drawn, not shaded, so this is the
/// resolution of its soft edge — too few and the "beam" is a stack of bars.
const BEAM_STRIPS: usize = 16;

/// One particle per this much field area, capped for the paint budget.
const BEAM_MOTE_AREA_PX: f32 = 420.0;
const BEAM_MOTES_MAX: usize = 900;

/// A single particle of the materialisation field, in window coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mote {
    pub pos: Pos2,
    /// 0..=1 — the particle's current alpha, flicker included.
    pub glow: f32,
    pub radius: f32,
    /// 0 = white, 1 = warm yellow. Fixed per particle, so the cloud is mixed
    /// rather than uniformly tinted.
    pub warmth: f32,
}

/// A ramp that is 0 at or below `a`, 1 at or above `b`, and smooth between.
/// Every stage below is cut from this one shape, which is what keeps them
/// meeting smoothly instead of stepping.
fn ramp(t: f32, (a, b): (f32, f32)) -> f32 {
    if b <= a {
        return if t >= b { 1.0 } else { 0.0 };
    }
    let x = ((t - a) / (b - a)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x) // smoothstep
}

/// The closing dimmer: 1 for most of the run, easing to exactly 0 at t=1 so
/// no light survives the moment the vertical beams reach the borders.
fn closing(t: f32) -> f32 {
    1.0 - ramp(t, FADE_OUT)
}

/// How far the horizontal pair has separated, as a fraction of the half-height
/// (0 = overlapped on the centre line, 1 = landed on the top and bottom edges).
pub fn beam_h_offset(t: f32) -> f32 {
    ramp(t.clamp(0.0, 1.0), H_TRAVEL)
}

/// Brightness of the horizontal pair: blooms in over the centre line, holds
/// through the climb, fades out as the beams land on the edges.
pub fn beam_h_intensity(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    ramp(t, H_IN) * (1.0 - ramp(t, H_OUT))
}

/// How far the vertical pair has swept, as a fraction of the half-width
/// (0 = overlapped on the centre line, 1 = arrived at the left and right
/// edges).
pub fn beam_v_offset(t: f32) -> f32 {
    ramp(t.clamp(0.0, 1.0), V_TRAVEL)
}

/// Brightness of the vertical pair: fades in at the centre exactly as the
/// horizontal pair is fading out at the edges, then eases away with the rest
/// of the light at the end.
pub fn beam_v_intensity(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    ramp(t, V_IN) * closing(t)
}

/// The band the form is REVEALED in — the widening space between the two
/// vertical beams. `None` until they have started to separate; the whole form
/// at t=1. This is a clip, not a cover, so what it has not reached is simply
/// never painted and a see-through window stays see-through there.
pub fn beam_reveal(rect: Rect, t: f32) -> Option<Rect> {
    let half = rect.width() * 0.5 * beam_v_offset(t);
    if half <= 0.5 {
        return None;
    }
    let cx = rect.center().x;
    Some(Rect::from_min_max(
        Pos2::new((cx - half).max(rect.left()), rect.top()),
        Pos2::new((cx + half).min(rect.right()), rect.bottom()),
    ))
}

/// The band the particle field occupies — the space opening between the two
/// horizontal beams.
///
/// It starts at the beams' own width (half the form, centred) and widens to
/// the full form as the field energises: the cloud is what the reveal in phase
/// 2 uncovers, so by the time the vertical beams start their sweep it has to
/// stand across the whole form, not just the half the horizontal beams span.
pub fn beam_field(rect: Rect, t: f32) -> Option<Rect> {
    let t = t.clamp(0.0, 1.0);
    let open = ramp(t, FIELD_IN);
    let half_h = rect.height() * 0.5 * open;
    if half_h <= 0.5 {
        return None;
    }
    let half_w = rect.width() * 0.5 * (0.5 + 0.5 * open);
    let c = rect.center();
    Some(Rect::from_min_max(
        Pos2::new((c.x - half_w).max(rect.left()), (c.y - half_h).max(rect.top())),
        Pos2::new(
            (c.x + half_w).min(rect.right()),
            (c.y + half_h).min(rect.bottom()),
        ),
    ))
}

/// How many particles this window's field holds.
pub fn beam_mote_count(rect: Rect) -> usize {
    let area = (rect.width() * rect.height()).max(1.0);
    ((area / BEAM_MOTE_AREA_PX).round() as usize).clamp(40, BEAM_MOTES_MAX)
}

/// The particle field at progress `t`, `time` seconds into the run.
///
/// Each particle owns a fixed home in the form, keyed by `seed` so a window
/// always rebuilds the same cloud. It drifts around that home and flickers on
/// its own phase — keyed to `time`, so the shimmer keeps moving on its own
/// clock rather than freezing whenever the timeline does.
///
/// Two things take a particle away, and both are the spec's: it is DISSOLVED
/// once the vertical beam sweeping its side has passed over it (that is what
/// "the cloud dissolves as the beams pass" means geometrically), and whatever
/// survives that is eased out by the closing dimmer.
pub fn beam_motes(rect: Rect, t: f32, seed: u32, time: f64, count: usize) -> Vec<Mote> {
    let t = t.clamp(0.0, 1.0);
    let Some(field) = beam_field(rect, t) else {
        return Vec::new();
    };
    let density = ramp(t, FIELD_IN) * closing(t);
    if density <= 0.001 || count == 0 {
        return Vec::new();
    }
    // Everything nearer the centre line than this has been swept clean.
    let swept = rect.width() * 0.5 * beam_v_offset(t);
    let cx = rect.center().x;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let h = mix32(seed ^ mix32(i as u32 + 1));
        let hx = (h & 0xFFFF) as f32 / 65535.0;
        let hy = ((h >> 16) & 0xFFFF) as f32 / 65535.0;
        let phase = (mix32(h) & 0xFFFF) as f32 / 65535.0;
        // Homes are laid out across the FORM, then only those inside the
        // current field are drawn — so the cloud grows into the opening gap
        // instead of the whole cloud stretching with it.
        let home = Pos2::new(
            rect.left() + rect.width() * hx,
            rect.top() + rect.height() * hy,
        );
        if !field.contains(home) {
            continue;
        }
        if (home.x - cx).abs() < swept {
            continue; // a vertical beam has already passed over this one
        }
        // A slow wander, each particle on its own heading.
        let wob = time as f32 * 1.6 + phase * 6.283;
        let pos = Pos2::new(home.x + wob.sin() * 5.0, home.y + (wob * 0.73).cos() * 4.0);
        // Flicker: fast, per-particle, never quite out and never quite full.
        let flick = 0.45 + 0.55 * (time as f32 * 7.5 + phase * 12.566).sin().abs();
        out.push(Mote {
            pos,
            glow: (density * flick).clamp(0.0, 1.0),
            radius: 0.9 + 1.9 * phase,
            // Warmth mixes white through to yellow across the cloud.
            warmth: phase,
        });
    }
    out
}

/// The soft-edge profile of a beam: `(offset from the centre line as a
/// fraction of the half-thickness, weight 0..=1)` per strip, outward.
///
/// A bell falloff, so the beam is brightest on its axis and dies away into
/// nothing at its edge — there is no hard boundary anywhere in it. Pure, so
/// the shape is testable without a painter.
pub fn beam_profile(steps: usize) -> Vec<(f32, f32)> {
    let steps = steps.max(1);
    (0..steps)
        .map(|i| {
            let f = (i as f32 + 0.5) / steps as f32;
            // 1 at the axis, 0 at the rim, with shoulders rather than a cone.
            (f, (1.0 - f * f) * (1.0 - f * f))
        })
        .collect()
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
        WindowEffect::TransporterII => {
            let _ = duration_ms; // the beam's stages are timeline fractions
            if !transparent {
                painter.rect_filled(rect, 0.0, bg);
            }
            paint_transporter(painter, rect, t, seed, time, face);
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
/// The beam palette: white on the axis, warm yellow at the flanks.
const BEAM_CORE: Color32 = Color32::from_rgb(255, 253, 242);
const BEAM_WARM: Color32 = Color32::from_rgb(255, 206, 104);

fn mix_rgb(a: Color32, b: Color32, f: f32) -> Color32 {
    let f = f.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * f) as u8;
    Color32::from_rgb(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
    )
}

/// Paint one soft energy beam as a stack of translucent strips under
/// [`beam_profile`]'s bell — white on the axis, warm at the flanks — wrapped
/// in a wider, dimmer pass that stands in for bloom.
///
/// `axis` is the beam's centre line (y for a horizontal beam, x for a
/// vertical one) and `span` its extent along its own length. Nothing is drawn
/// opaque and no strip has a hard edge: the outermost carries almost no alpha,
/// so the beam dies into the background rather than ending at a line.
fn paint_soft_beam(
    painter: &egui::Painter,
    bounds: Rect,
    horizontal: bool,
    axis: f32,
    span: (f32, f32),
    thickness: f32,
    intensity: f32,
) {
    if intensity <= 0.004 || thickness <= 0.0 {
        return;
    }
    let profile = beam_profile(BEAM_STRIPS);
    // Two passes: the wide dim bloom first, the tight bright core over it.
    for (scale, gain) in [(BEAM_BLOOM, 0.16_f32), (1.0, 0.75)] {
        let half = thickness * 0.5 * scale;
        let step = half / profile.len() as f32;
        for (f, w) in &profile {
            let a = intensity * gain * w;
            if a <= 0.002 {
                continue;
            }
            let colour = mix_rgb(BEAM_CORE, BEAM_WARM, *f).gamma_multiply(a);
            let off = half * f;
            // One strip either side of the axis (they meet on it).
            for sign in [-1.0_f32, 1.0] {
                let c = axis + off * sign;
                let strip = if horizontal {
                    Rect::from_min_max(
                        Pos2::new(span.0, c - step * 0.5),
                        Pos2::new(span.1, c + step * 0.5),
                    )
                } else {
                    Rect::from_min_max(
                        Pos2::new(c - step * 0.5, span.0),
                        Pos2::new(c + step * 0.5, span.1),
                    )
                };
                if strip.intersects(bounds) {
                    painter.rect_filled(strip, 0.0, colour);
                }
            }
        }
    }
}

/// Transporter II. Nothing here COVERS: the form is revealed by CLIPPING to
/// the band between the vertical beams, and every beam and particle is added
/// over it. What the reveal has not reached is simply never painted, so a
/// see-through window stays see-through there.
///
/// Painted back to front — form, particles, beams — so the light always stands
/// in front of what it is revealing.
fn paint_transporter(
    painter: &egui::Painter,
    rect: Rect,
    t: f32,
    seed: u32,
    time: f64,
    face: &mut dyn FnMut(&egui::Painter, Rect),
) {
    let clipped = painter.with_clip_rect(rect);
    let c = rect.center();
    let thickness = rect.width().min(rect.height()) * BEAM_THICK;

    // 1. The form, in the band the vertical beams have opened.
    if let Some(band) = beam_reveal(rect, t) {
        face(&clipped.with_clip_rect(band), rect);
    }

    // 2. The particle field, over whatever is behind it and under the beams.
    for m in beam_motes(rect, t, seed, time, beam_mote_count(rect)) {
        if m.glow <= 0.004 {
            continue;
        }
        let tint = mix_rgb(BEAM_CORE, BEAM_WARM, m.warmth);
        // A wide faint halo under a small bright centre — the particle's own
        // little bloom, which is what keeps the cloud reading as light rather
        // than as dots.
        clipped.circle_filled(m.pos, m.radius * 3.0, tint.gamma_multiply(m.glow * 0.13));
        clipped.circle_filled(m.pos, m.radius * 1.7, tint.gamma_multiply(m.glow * 0.28));
        clipped.circle_filled(m.pos, m.radius, tint.gamma_multiply(m.glow * 0.85));
    }

    // 3. Phase 1 — the horizontal pair, half the form's width, centred,
    // separating toward the top and bottom edges.
    let hi = beam_h_intensity(t);
    if hi > 0.004 {
        let reach = rect.height() * 0.5 * beam_h_offset(t);
        let half_w = rect.width() * 0.25; // "approximately 50% of the width"
        let span = (c.x - half_w, c.x + half_w);
        for sign in [-1.0_f32, 1.0] {
            paint_soft_beam(
                &clipped,
                rect,
                true,
                c.y + reach * sign,
                span,
                thickness,
                hi,
            );
        }
    }

    // 4. Phase 2 — the full-height vertical pair sweeping out to the borders.
    let vi = beam_v_intensity(t);
    if vi > 0.004 {
        let reach = rect.width() * 0.5 * beam_v_offset(t);
        let span = (rect.top(), rect.bottom());
        for sign in [-1.0_f32, 1.0] {
            paint_soft_beam(
                &clipped,
                rect,
                false,
                c.x + reach * sign,
                span,
                thickness,
                vi,
            );
        }
    }
}

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
        // form down to each falling line's tail. Transporter likewise: it
        // dims the face and adds light, so it never has pixels to erase.
        assert!(WindowEffect::MatrixRain.plays_over_desktop());
        assert!(WindowEffect::TransporterII.plays_over_desktop());
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

    /// Transporter II runs at exactly 4 s, and on real time.
    ///
    /// The phases are cut to that clock — beam travel, hand-over, fade-out are
    /// all fractions of it — so it is offered at one length rather than a band,
    /// and an easing would slide the beats off the beams. Whatever a stored
    /// project says, the parse lands on 4000.
    #[test]
    fn transporter_ii_runs_at_exactly_four_seconds_on_real_time() {
        assert_eq!(
            WindowEffect::TransporterII.duration_bounds(),
            (BEAM_MS, BEAM_MS)
        );
        for stored in ["transporter-ii:200", "transporter-ii:9999", "transporter-ii"] {
            assert_eq!(
                FxSpec::parse(stored).duration_ms,
                BEAM_MS,
                "{stored} did not land on the fixed length"
            );
        }
        let s = FxSpec::parse("transporter-ii:4000:ease-in-out");
        assert_eq!(s.effect, WindowEffect::TransporterII);
        assert_eq!(FxSpec::parse(&s.format()), s, "format→parse round-trip");
        // Real time, whatever easing is configured.
        for e in [Easing::Linear, Easing::EaseIn, Easing::EaseOut, Easing::EaseInOut] {
            assert_eq!(
                WindowEffect::TransporterII.progress(e, 0.25),
                0.25,
                "{e:?} warped the choreography"
            );
        }
        println!("transporter-ii: fixed {BEAM_MS} ms, linear");
    }

    /// Both ENDS of the timeline are dark and empty: no beams, no particles,
    /// and the form either wholly unrevealed or wholly revealed. An exit
    /// replays this same math from 1 down to 0, so light left over at an
    /// endpoint would flash the instant the window appears or closes.
    #[test]
    fn transporter_ii_is_quiet_at_both_ends() {
        let r = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0));

        assert_eq!(beam_h_intensity(0.0), 0.0, "horizontal beams lit at t=0");
        assert_eq!(beam_h_intensity(1.0), 0.0, "horizontal beams lit at t=1");
        assert_eq!(beam_v_intensity(0.0), 0.0, "vertical beams lit at t=0");
        assert_eq!(beam_v_intensity(1.0), 0.0, "vertical beams still lit at t=1");
        assert!(beam_reveal(r, 0.0).is_none(), "the form shows at t=0");
        assert_eq!(
            beam_reveal(r, 1.0).map(|b| b.width()),
            Some(r.width()),
            "the whole form must stand revealed at t=1"
        );
        for t in [0.0, 1.0] {
            assert!(
                beam_motes(r, t, 7, 0.0, beam_mote_count(r)).is_empty(),
                "particles at t={t}"
            );
        }
        println!(
            "ends quiet; mid-run particles: {}",
            beam_motes(r, 0.45, 7, 0.0, beam_mote_count(r)).len()
        );
    }

    /// Phase 1 — the horizontal pair starts overlapped on the centre line and
    /// separates to the top and bottom edges, and the particle field opens
    /// with the gap between them.
    #[test]
    fn phase_one_splits_the_horizontal_pair_and_opens_the_field() {
        let r = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0));

        assert_eq!(beam_h_offset(0.0), 0.0, "the pair starts overlapped");
        assert!(beam_h_intensity(0.03) > 0.0, "the pair must bloom in early");
        let mut prev = -1.0;
        for i in 0..=100 {
            let o = beam_h_offset(i as f32 / 100.0);
            assert!(o >= prev - 1e-6, "the split reversed at t={i}");
            prev = o;
        }
        assert_eq!(beam_h_offset(1.0), 1.0, "the pair must reach the edges");
        assert!(
            beam_h_intensity(0.60) <= 0.0,
            "the pair must be gone once it has landed"
        );

        // The field opens with the gap, from nothing to the whole form.
        assert!(beam_field(r, 0.0).is_none(), "field open before the split");
        let early = beam_field(r, 0.12).expect("field should be open at t=0.12");
        let late = beam_field(r, 0.50).expect("field should be open at t=0.50");
        assert!(early.height() < late.height(), "the field did not grow");
        assert!(
            (late.height() - r.height()).abs() < 1.0,
            "by the hand-over the field spans the form: {}",
            late.height()
        );
        println!(
            "field: {:.0}x{:.0} at t=0.12 → {:.0}x{:.0} at t=0.50",
            early.width(),
            early.height(),
            late.width(),
            late.height()
        );
    }

    /// Phase 2 — the vertical pair fades in as the horizontal pair fades out,
    /// then sweeps outward, and the form is revealed in the band between them.
    #[test]
    fn phase_two_hands_over_and_sweeps_the_form_into_view() {
        let r = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0));

        // The hand-over overlaps: neither pair is ever alone in the dark.
        let cross = (0..=100)
            .map(|i| i as f32 / 100.0)
            .any(|t| beam_h_intensity(t) > 0.05 && beam_v_intensity(t) > 0.05);
        assert!(cross, "the two pairs never overlap — the hand-over is a cut");

        // The reveal only ever widens, from nothing to the whole form.
        let mut prev = 0.0_f32;
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let w = beam_reveal(r, t).map(|b| b.width()).unwrap_or(0.0);
            assert!(w >= prev - 1e-3, "the reveal narrowed at t={t}");
            prev = w;
        }
        assert!(
            beam_reveal(r, 0.50).is_none(),
            "nothing may be revealed before the vertical pair moves"
        );
        println!(
            "reveal: {:.0}px at t=0.70, {:.0}px at t=0.90, {:.0}px at t=1.0",
            beam_reveal(r, 0.70).map(|b| b.width()).unwrap_or(0.0),
            beam_reveal(r, 0.90).map(|b| b.width()).unwrap_or(0.0),
            beam_reveal(r, 1.0).map(|b| b.width()).unwrap_or(0.0),
        );
    }

    /// The cloud dissolves where a vertical beam has passed: no particle ever
    /// survives inside the revealed band, and the count only falls once the
    /// sweep is under way.
    #[test]
    fn the_cloud_dissolves_behind_the_vertical_beams() {
        let r = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0));
        let n = beam_mote_count(r);
        let cx = r.center().x;

        for t in [0.62_f32, 0.75, 0.88] {
            let swept = r.width() * 0.5 * beam_v_offset(t);
            for m in beam_motes(r, t, 5, 0.0, n) {
                assert!(
                    (m.pos.x - cx).abs() >= swept - 6.0, // 6px of drift wander
                    "a particle survived inside the revealed band at t={t}"
                );
            }
        }
        let full = beam_motes(r, 0.50, 5, 0.0, n).len();
        let swept = beam_motes(r, 0.80, 5, 0.0, n).len();
        assert!(
            swept < full,
            "the cloud did not thin as the beams passed: {full} → {swept}"
        );
        println!("cloud: {full} particles at t=0.50 → {swept} at t=0.80");
    }

    /// Every beam is drawn as a soft gradient, never a solid bar: brightest on
    /// its axis, dying to nothing at its rim.
    #[test]
    fn beams_are_gradients_not_solid_bars() {
        let p = beam_profile(BEAM_STRIPS);
        assert_eq!(p.len(), BEAM_STRIPS);
        assert!(p[0].1 > 0.9, "the axis strip should be near-full: {}", p[0].1);
        assert!(
            p[BEAM_STRIPS - 1].1 < 0.02,
            "the rim strip must be all but invisible: {}",
            p[BEAM_STRIPS - 1].1
        );
        let mut prev = f32::MAX;
        for (off, w) in &p {
            assert!(*w <= prev + 1e-6, "the falloff rose again at offset {off}");
            assert!((0.0..=1.0).contains(w));
            prev = *w;
        }
        println!(
            "beam profile: axis {:.2} → rim {:.3} over {BEAM_STRIPS} strips",
            p[0].1,
            p[BEAM_STRIPS - 1].1
        );
    }

    /// The cloud scales with the window but is bounded at both ends: a tiny
    /// form must not look bare, and a huge one must not paint 20 000 circles.
    #[test]
    fn the_cloud_scales_with_the_window_within_bounds() {
        let small = beam_mote_count(Rect::from_min_size(Pos2::ZERO, Vec2::new(60.0, 40.0)));
        let normal = beam_mote_count(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
        let huge = beam_mote_count(Rect::from_min_size(Pos2::ZERO, Vec2::new(3840.0, 2160.0)));
        assert!(small >= 40, "a small form got a bare cloud: {small}");
        assert!(small < normal && normal < huge, "{small} {normal} {huge}");
        assert_eq!(huge, BEAM_MOTES_MAX, "the paint budget cap did not hold");
        println!("particles: 60x40 → {small}, 400x300 → {normal}, 4K → {huge}");
    }

    /// The shimmer is driven by the wall clock, not the timeline, so it keeps
    /// moving even where progress is momentarily flat.
    #[test]
    fn the_shimmer_moves_on_the_clock() {
        let r = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0));
        let n = beam_mote_count(r);
        let a = beam_motes(r, 0.4, 9, 0.00, n);
        let b = beam_motes(r, 0.4, 9, 0.13, n);
        assert!(
            a.iter().zip(&b).any(|(x, y)| (x.glow - y.glow).abs() > 0.01),
            "the swarm did not twinkle between frames at the same progress"
        );
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


