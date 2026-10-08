// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A chart's **pure** parts: its data wire format, and the value tween.
//!
//! Live data reaches a chart as the `__ChartData` property — one
//! `label<TAB>value` per line — pushed by `AddPoint` / `Clear` / the
//! `COBOL-CHART-*` calls. Both the painter and the animating renderer read it,
//! so the parse lives here once rather than being written out twice with a
//! chance to disagree.
//!
//! There is **no clock and no state** here, for the reason [`crate::snackbar`]
//! has none: a tween's lifetime belongs to whoever owns cross-frame state, and
//! the arithmetic has to be drivable from a fabricated `t` in a test.

use crate::model::{Control, PropValue};

/// How long a value animation runs when nothing says otherwise (operator,
/// 2026-09-02).
pub const DEFAULT_ANIM_MS: i64 = 2000;

/// The shortest it may run. Below this a "tween" is a flicker: the eye reads a
/// jump, and the property would be honoured in name only.
pub const MIN_ANIM_MS: i64 = 250;

/// One point of a series.
pub type Point = (String, f32);

/// One label and its value in every series: a bar chart's category with its
/// bars, a line chart's x-position with each line's y.
pub type Row = (String, Vec<f32>);

/// Parse the `__ChartData` wire format: one `label<TAB>value[<TAB>value…]` per
/// line — the FIRST series only. See [`parse_chart_rows`] for all of them.
///
/// A line without a tab, or whose first value is not a number, is **skipped**
/// rather than defaulted to zero — a zero would be plotted, and a bar that is
/// not there is a better answer than a bar that is wrong.
pub fn parse_chart_data(raw: &str) -> Vec<Point> {
    parse_chart_rows(raw).into_iter().map(|(l, v)| (l, v[0])).collect()
}

/// Parse the `__ChartData` wire format with every series: a label, then one
/// value per series, tab-separated. Skipped like [`parse_chart_data`] when
/// the first value is missing or not a number; a LATER value that is not a
/// number is 0 in its series, so the rows stay aligned.
pub fn parse_chart_rows(raw: &str) -> Vec<Row> {
    raw.lines()
        .filter_map(|ln| {
            let mut it = ln.split('\t');
            let label = it.next()?.to_owned();
            let first: f32 = it.next()?.trim().parse().ok()?;
            let mut values = vec![first];
            values.extend(it.map(|v| v.trim().parse().unwrap_or(0.0)));
            Some((label, values))
        })
        .collect()
}

/// The range a chart's values are scaled into: `(lo, hi)` with
/// `lo <= 0 <= hi`, so zero always has a place on the plot (spec 052 R21).
/// Side by side, the smallest and largest value of any series; stacked, the
/// largest total of the negative values of a row and of its positive ones.
/// With no negative value it is `(0, max)` — the range charts always had.
pub fn value_range(rows: &[Row], stacked: bool) -> (f32, f32) {
    let (mut lo, mut hi) = (0.0_f32, 0.0_f32);
    for (_, vs) in rows {
        if stacked {
            hi = hi.max(vs.iter().map(|v| v.max(0.0)).sum::<f32>());
            lo = lo.min(vs.iter().map(|v| v.min(0.0)).sum::<f32>());
        } else {
            for &v in vs {
                hi = hi.max(v);
                lo = lo.min(v);
            }
        }
    }
    (lo, hi)
}

/// Render rows back to the `__ChartData` wire format.
pub fn format_chart_rows(rows: &[Row]) -> String {
    rows.iter()
        .map(|(l, vs)| {
            let mut line = l.replace(['\t', '\n'], " ");
            for v in vs {
                line.push('\t');
                line.push_str(&v.to_string());
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// [`tween_series`] for every series at once: value `j` of row `i` travels
/// from value `j` of the row shown there before (0 when there was none).
pub fn tween_rows(from: &[Row], to: &[Row], t: f32) -> Vec<Row> {
    let k = ease_out(t);
    to.iter()
        .enumerate()
        .map(|(i, (label, targets))| {
            let values = targets
                .iter()
                .enumerate()
                .map(|(j, target)| {
                    if !target.is_finite() {
                        return *target;
                    }
                    let start = from
                        .get(i)
                        .and_then(|(_, vs)| vs.get(j))
                        .copied()
                        .filter(|v| v.is_finite())
                        .unwrap_or(0.0);
                    start + (target - start) * k
                })
                .collect();
            (label.clone(), values)
        })
        .collect()
}

/// Render a series back to the `__ChartData` wire format.
pub fn format_chart_data(points: &[Point]) -> String {
    points
        .iter()
        .map(|(l, v)| format!("{}\t{}", l.replace(['\t', '\n'], " "), v))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Is this chart set to animate its values, and for how long?
///
/// `None` when the animation is off. The duration is clamped up to
/// [`MIN_ANIM_MS`]: the property is the developer's to set, but a number that
/// would produce a jump instead of a movement is not honoured as written
/// (operator, 2026-09-02 — "always >= 250ms").
pub fn value_anim_ms(ctrl: &Control) -> Option<i64> {
    let on = ctrl
        .get_prop("AnimateValues")
        .map(|v| v.as_bool())
        .unwrap_or(false);
    if !on {
        return None;
    }
    let ms = ctrl
        .get_prop("AnimationDuration")
        .map(|v| v.as_i64())
        .unwrap_or(DEFAULT_ANIM_MS);
    Some(ms.max(MIN_ANIM_MS))
}

/// Decelerating ease, the same shape a notification's entrance uses: motion
/// that starts fast and settles reads as physical.
fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// The series to draw `t` of the way from `from` to `to`, `t` in `0.0..=1.0`.
///
/// **The labels are `to`'s**, always: the new data decides what the chart is
/// about, and a half-played tween must never leave a point captioned with the
/// name it used to have.
///
/// Values are matched **by position**, which is what makes a series that grew
/// or shrank still animate:
///
/// * a point with a predecessor moves from that predecessor's value;
/// * a point with none — the series got longer — **grows from zero**, so it
///   rises into place rather than appearing at full height;
/// * points that were dropped simply stop being drawn, since the frame only
///   ever has as many points as `to`.
///
/// A value that is not finite is left at its target rather than interpolated:
/// there is no sensible half-way to a NaN, and one would poison the auto-scale
/// for every other point on the chart.
pub fn tween_series(from: &[Point], to: &[Point], t: f32) -> Vec<Point> {
    let k = ease_out(t);
    to.iter()
        .enumerate()
        .map(|(i, (label, target))| {
            if !target.is_finite() {
                return (label.clone(), *target);
            }
            let start = from
                .get(i)
                .map(|(_, v)| *v)
                .filter(|v| v.is_finite())
                .unwrap_or(0.0);
            (label.clone(), start + (target - start) * k)
        })
        .collect()
}

// ── RadarChart: the geometry ────────────────────────────────────────────────
//
// A radar has one spoke per LABEL (a row of the chart's data) and one polygon
// per SERIES (a value column), all on one shared scale. Everything the painter
// and the hover hit-test need to agree on lives here, free of any UI types, so
// a tooltip finds the vertex that was drawn and the whole thing can be driven
// from plain numbers in a test.

/// Rings the grid draws when `GridLevels` says nothing usable.
pub const RADAR_DEFAULT_LEVELS: usize = 5;

/// The most rings `GridLevels` may ask for; the fewest is 1.
pub const RADAR_MAX_LEVELS: usize = 10;

/// `FillOpacity` (percent) when the property is absent: see-through enough that
/// polygons laid over each other can all be read.
pub const RADAR_DEFAULT_FILL_OPACITY: f32 = 35.0;

/// Pixels between the rim and an axis caption.
pub const RADAR_LABEL_GAP: f32 = 4.0;

/// Pixels kept clear between the rim and the edge of the plot, so the outer
/// ring and a vertex marker on it are not cut off.
pub const RADAR_RIM_PAD: f32 = 4.0;

/// An axis caption's type, as a fraction of the legend's: a little quieter, so
/// the circle gets the room.
pub const RADAR_LABEL_SCALE: f32 = 0.85;

/// Average glyph width, as a fraction of the font size, used to size a caption
/// before it is laid out. The layout has to be computable without a font (the
/// hover hit-test has none), so it works from this estimate and the painter
/// keeps each caption inside the plot with the real measurement.
const RADAR_GLYPH_W: f32 = 0.58;

/// A number property that may be stored as an integer or as text with
/// decimals: `MaxValue` written from COBOL as `1.5` arrives as the text `1.5`.
fn number_prop(ctrl: &Control, key: &str) -> Option<f32> {
    match ctrl.get_prop(key)? {
        PropValue::Int(n) => Some(*n as f32),
        PropValue::Bool(_) => None,
        PropValue::String(s) => s.trim().parse::<f32>().ok().filter(|v| v.is_finite()),
    }
}

/// How many rings the grid draws: `GridLevels`, 1 to 10, 5 by default.
pub fn radar_levels(ctrl: &Control) -> usize {
    number_prop(ctrl, "GridLevels")
        .map(|n| n.round().clamp(1.0, RADAR_MAX_LEVELS as f32) as usize)
        .unwrap_or(RADAR_DEFAULT_LEVELS)
}

/// `FillOpacity` as a fraction, `0.0..=1.0`: how solid a polygon's fill is.
pub fn radar_fill_opacity(ctrl: &Control) -> f32 {
    number_prop(ctrl, "FillOpacity")
        .unwrap_or(RADAR_DEFAULT_FILL_OPACITY)
        .clamp(0.0, 100.0)
        / 100.0
}

/// The radius of a vertex marker: `PointRadius`, 3 by default.
pub fn radar_marker_radius(ctrl: &Control) -> f32 {
    number_prop(ctrl, "PointRadius").unwrap_or(3.0).clamp(0.5, 40.0)
}

/// The scale a radar draws on, shared by every axis: `min` sits at the centre,
/// `max` on the rim.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadarScale {
    pub min: f32,
    pub max: f32,
}

impl RadarScale {
    /// Where `value` falls between the centre (`0.0`) and the rim (`1.0`).
    ///
    /// A value below `min` — a negative one on the default scale — sits on the
    /// centre and one above `max` on the rim; a value that is not a number sits
    /// on the centre, since there is no honest place to draw it.
    pub fn fraction(&self, value: f32) -> f32 {
        if value.is_nan() {
            return 0.0;
        }
        ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }

    /// The value ring `level` of `levels` stands for (`level` 0 is the
    /// centre, `levels` the rim).
    pub fn ring_value(&self, level: usize, levels: usize) -> f32 {
        self.min + (self.max - self.min) * level as f32 / levels.max(1) as f32
    }
}

/// The smallest "nice" step — 1, 2, 2.5, 5 or 10 times a power of ten — that
/// is at least `raw`. Rings spaced by one read 20, 40, 60 rather than
/// 17.4, 34.8, 52.2.
pub fn nice_step(raw: f32) -> f32 {
    if !(raw.is_finite() && raw > 0.0) {
        return 1.0;
    }
    let base = 10f32.powf(raw.log10().floor());
    for m in [1.0, 2.0, 2.5, 5.0] {
        let step = base * m;
        // A hair of tolerance: 100 / 5 must stay 20, not tip over to 25
        // because the division came out as 20.000002.
        if raw <= step * (1.0 + 1e-4) {
            return step;
        }
    }
    base * 10.0
}

/// The scale for `rows`. `min` is `MinValue`; `max` is `MaxValue`, used as it
/// stands when it is above `min`. Otherwise — 0, the default, always means
/// automatic, and so does a top that is not above the bottom — the top of the
/// scale comes from the data: the largest value of any series, rounded up so
/// each of the `levels` rings is a [`nice_step`] apart.
pub fn radar_scale(rows: &[Row], min: f32, max: f32, levels: usize) -> RadarScale {
    let min = if min.is_finite() { min } else { 0.0 };
    if max.is_finite() && max != 0.0 && max > min {
        return RadarScale { min, max };
    }
    let largest = rows
        .iter()
        .flat_map(|(_, vs)| vs.iter().copied())
        .filter(|v| v.is_finite())
        .fold(min, f32::max);
    let span = largest - min;
    // Nothing above the floor (no data, or all of it at or below `min`): a
    // scale of one, so the rings are still drawn at sensible places.
    let span = if span > 0.0 { span } else { 1.0 };
    let levels = levels.max(1) as f32;
    RadarScale { min, max: min + nice_step(span / levels) * levels }
}

/// `MinValue` and `MaxValue` as the control carries them.
pub fn radar_bounds(ctrl: &Control) -> (f32, f32) {
    (
        number_prop(ctrl, "MinValue").unwrap_or(0.0),
        number_prop(ctrl, "MaxValue").unwrap_or(0.0),
    )
}

/// How many series `rows` carry: the widest row.
pub fn radar_series_count(rows: &[Row]) -> usize {
    rows.iter().map(|(_, vs)| vs.len()).max().unwrap_or(0)
}

/// The data a radar shows before it has any: six axes and `series` series
/// (three, unless a bound chart names more), the same believable shape on
/// every surface.
pub fn radar_sample_rows(series: usize) -> Vec<Row> {
    const AXES: [&str; 6] = ["Speed", "Power", "Range", "Comfort", "Safety", "Value"];
    const BASE: [[f32; 6]; 3] = [
        [80.0, 60.0, 70.0, 90.0, 55.0, 75.0],
        [55.0, 85.0, 45.0, 60.0, 80.0, 50.0],
        [35.0, 40.0, 90.0, 45.0, 65.0, 85.0],
    ];
    AXES.iter()
        .enumerate()
        .map(|(i, label)| {
            let values = (0..series.max(1))
                .map(|j| BASE[j % 3][(i + j / 3) % 6] * (1.0 - 0.1 * (j / 3) as f32))
                .collect();
            ((*label).to_owned(), values)
        })
        .collect()
}

/// A ring's value as text, with just the decimals needed to tell one ring from
/// the next: `step` is the gap between rings, so 20 gives `40`, 2.5 gives `7.5`
/// and 0.25 gives `0.75`.
pub fn radar_value_text(value: f32, step: f32) -> String {
    let mut decimals = 0usize;
    let mut scaled = step.abs();
    while decimals < 4 && (scaled - scaled.round()).abs() > 1e-3 * scaled.max(1.0) {
        scaled *= 10.0;
        decimals += 1;
    }
    let text = format!("{value:.decimals$}");
    // A value that rounds to nothing is 0, not -0.
    if text.trim_start_matches('-').chars().all(|c| c == '0' || c == '.') {
        text.trim_start_matches('-').to_owned()
    } else {
        text
    }
}

/// The unit direction, in screen space (y down), axis `axis` of `axes` points
/// in. Axis 0 is straight up and the rest follow clockwise.
pub fn radar_dir(axis: usize, axes: usize) -> (f32, f32) {
    let angle = std::f32::consts::TAU * axis as f32 / axes.max(1) as f32;
    (angle.sin(), -angle.cos())
}

/// The point `frac` of the way out along an axis: `0.0` is the centre, `1.0`
/// the rim.
pub fn radar_point(
    centre: (f32, f32),
    radius: f32,
    axis: usize,
    axes: usize,
    frac: f32,
) -> (f32, f32) {
    let (dx, dy) = radar_dir(axis, axes);
    (centre.0 + dx * radius * frac, centre.1 + dy * radius * frac)
}

/// Which part of a caption sits on the point it is anchored to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// The caption begins at the anchor and runs right / down.
    Start,
    /// The caption is centred on the anchor.
    Middle,
    /// The caption ends at the anchor and runs left / up.
    End,
}

/// How the caption of an axis pointing in `dir` hangs off the rim: always
/// outward, so it never lands on the polygon it labels.
pub fn radar_label_anchor(dir: (f32, f32)) -> (Anchor, Anchor) {
    const SIDE: f32 = 0.25;
    let pick = |d: f32| {
        if d > SIDE {
            Anchor::Start
        } else if d < -SIDE {
            Anchor::End
        } else {
            Anchor::Middle
        }
    };
    (pick(dir.0), pick(dir.1))
}

/// The box a caption of `font_px` type takes, before it is laid out.
pub fn radar_label_size(label: &str, font_px: f32) -> (f32, f32) {
    (label.chars().count() as f32 * font_px * RADAR_GLYPH_W, font_px * 1.2)
}

/// `label` cut to fit `max_w` pixels, ending in an ellipsis where it was cut.
pub fn radar_fit_label(label: &str, font_px: f32, max_w: f32) -> String {
    let fits = (max_w / (font_px * RADAR_GLYPH_W)).floor().max(1.0) as usize;
    if label.chars().count() <= fits {
        return label.to_owned();
    }
    let mut cut: String = label.chars().take(fits.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

/// Where a radar sits in its plot: the centre, the rim's radius, and each
/// axis's caption as it will be drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct RadarLayout {
    pub centre: (f32, f32),
    pub radius: f32,
    pub labels: Vec<String>,
}

/// The extent of a caption `len` long on either side of its anchor.
fn extent(anchor: Anchor, len: f32) -> (f32, f32) {
    match anchor {
        Anchor::Start => (0.0, len),
        Anchor::Middle => (-len * 0.5, len * 0.5),
        Anchor::End => (-len, 0.0),
    }
}

/// How far from the centre, along a direction with component `d`, an anchor
/// can be before a caption reaching `off_lo..off_hi` around it leaves `lo..hi`.
fn room(d: f32, centre: f32, lo: f32, hi: f32, off_lo: f32, off_hi: f32) -> f32 {
    if d > 1e-3 {
        (hi - off_hi - centre) / d
    } else if d < -1e-3 {
        (lo - off_lo - centre) / d
    } else {
        f32::INFINITY
    }
}

/// Lay a radar out in `plot` (`[min_x, min_y, max_x, max_y]`): the centre of the
/// plot, and the largest rim that still leaves every axis caption — hung
/// outward off its spoke — inside the plot.
///
/// Captions are only shortened when they cost the circle room. The circle is
/// limited by the height the top and bottom captions take however short the
/// captions are; so the captions are tried from whole to very short, and the
/// longest set that still leaves the circle within 15 % of the best it could
/// have is the one used. A radar with short axis names therefore keeps them whole
/// and uses the control, and one with long names shortens them with an
/// ellipsis, and only as far as it must. Even then the circle is not allowed
/// below 45 % of what the plot could hold.
///
/// Nothing here measures text (see `RADAR_GLYPH_W`), so the painter and the
/// hit-test, which has no font, get the same circle.
pub fn radar_layout(plot: [f32; 4], labels: &[String], font_px: f32) -> RadarLayout {
    let [x0, y0, x1, y1] = plot;
    let centre = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
    let (w, h) = ((x1 - x0).max(0.0), (y1 - y0).max(0.0));
    let half = (w.min(h) * 0.5 - RADAR_RIM_PAD).max(0.0);
    let floor = half * 0.45;
    let n = labels.len();
    let attempt = |cap: f32| -> RadarLayout {
        let shown: Vec<String> =
            labels.iter().map(|l| radar_fit_label(l.trim(), font_px, w * cap)).collect();
        let mut reach = f32::INFINITY;
        for (i, text) in shown.iter().enumerate().filter(|(_, t)| !t.is_empty()) {
            let (lw, lh) = radar_label_size(text, font_px);
            let dir = radar_dir(i, n);
            let (ax, ay) = radar_label_anchor(dir);
            let (xl, xr) = extent(ax, lw);
            let (yt, yb) = extent(ay, lh);
            reach = reach
                .min(room(dir.0, centre.0, x0, x1, xl, xr))
                .min(room(dir.1, centre.1, y0, y1, yt, yb));
        }
        RadarLayout { centre, radius: (reach - RADAR_LABEL_GAP).min(half).max(0.0), labels: shown }
    };
    let attempts: Vec<RadarLayout> = [0.5, 0.34, 0.28, 0.24, 0.20, 0.17, 0.14, 0.12, 0.10].into_iter().map(attempt).collect();
    // What the circle could be with the shortest captions: the target.
    let best = attempts.last().map(|l| l.radius).unwrap_or(half);
    let mut chosen = attempts
        .iter()
        .find(|l| l.radius >= best * 0.85 && l.radius >= floor)
        .cloned()
        .unwrap_or_else(|| attempts.last().cloned().unwrap_or(RadarLayout { centre, radius: half, labels: Vec::new() }));
    if chosen.radius < floor {
        chosen.radius = floor.min(half);
    }
    chosen
}

/// Each series' fractions of the way to the rim, `[series][axis]`, before any
/// load animation: 0 at `MinValue` or below it (or not a number), 1 at the top
/// of the scale. A row shorter than the series count is 0 in the rest.
pub fn radar_fractions(rows: &[Row], scale: &RadarScale) -> Vec<Vec<f32>> {
    (0..radar_series_count(rows))
        .map(|s| rows.iter().map(|(_, vs)| scale.fraction(vs.get(s).copied().unwrap_or(0.0))).collect())
        .collect()
}

/// Every series' polygon on `layout`: `[series][axis]`, each vertex
/// `fraction` of the way to the rim (scaled by `grow`, the load animation's
/// progress). A row shorter than the series count is 0 in the rest.
pub fn radar_vertices(
    layout: &RadarLayout,
    scale: &RadarScale,
    rows: &[Row],
    grow: f32,
) -> Vec<Vec<(f32, f32)>> {
    let axes = rows.len();
    radar_fractions(rows, scale)
        .into_iter()
        .map(|series| {
            series
                .into_iter()
                .enumerate()
                .map(|(a, frac)| radar_point(layout.centre, layout.radius, a, axes, frac * grow))
                .collect()
        })
        .collect()
}

/// The vertex nearest `at` within `reach` pixels, as `(series, axis)`. Where
/// vertices coincide the later series wins — it is the one drawn on top.
pub fn radar_nearest(
    vertices: &[Vec<(f32, f32)>],
    at: (f32, f32),
    reach: f32,
) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize, f32)> = None;
    for (s, polygon) in vertices.iter().enumerate() {
        for (a, p) in polygon.iter().enumerate() {
            let d = ((p.0 - at.0).powi(2) + (p.1 - at.1).powi(2)).sqrt();
            if d <= reach && best.map_or(true, |(_, _, b)| d <= b) {
                best = Some((s, a, d));
            }
        }
    }
    best.map(|(s, a, _)| (s, a))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Several series travel on one line: the first alone is what a
    /// single-series reader sees, a bad later value is 0, and a one-series
    /// row writes back exactly as it always did.
    #[test]
    fn a_row_carries_every_series() {
        let rows = parse_chart_rows("Jan\t10\t20\t5\nFeb\t7\tx\nbad\tno\t3");
        assert_eq!(rows, vec![("Jan".into(), vec![10.0, 20.0, 5.0]), ("Feb".into(), vec![7.0, 0.0])]);
        assert_eq!(parse_chart_data("Jan\t10\t20"), vec![("Jan".to_owned(), 10.0)]);
        assert_eq!(format_chart_rows(&[("A".into(), vec![1.5])]), format_chart_data(&[("A".into(), 1.5)]));
        let mid = tween_rows(&[("A".into(), vec![0.0, 10.0])], &[("A".into(), vec![10.0, 10.0, 4.0])], 1.0);
        assert_eq!(mid, vec![("A".into(), vec![10.0, 10.0, 4.0])]);
    }

    fn pts(v: &[(&str, f32)]) -> Vec<Point> {
        v.iter().map(|(l, x)| ((*l).to_owned(), *x)).collect()
    }

    #[test]
    fn the_wire_format_round_trips_and_skips_what_it_cannot_read() {
        let raw = "Q1\t128\nQ2\t174.5\nbroken line\nQ3\tnot-a-number\nQ4\t209";
        let p = parse_chart_data(raw);
        eprintln!("\n  line                  parsed");
        eprintln!("  -------------------   ------");
        for ln in raw.lines() {
            let got = parse_chart_data(ln);
            eprintln!("  {:<19}   {}", ln, if got.is_empty() { "skipped" } else { "yes" });
        }
        assert_eq!(p.len(), 3, "3 of 5 lines are readable: {p:?}");
        assert_eq!(p[0], ("Q1".to_owned(), 128.0));
        assert_eq!(p[1], ("Q2".to_owned(), 174.5));
        assert_eq!(p[2], ("Q4".to_owned(), 209.0));
        // And back out again, unchanged for the lines that survived.
        assert_eq!(parse_chart_data(&format_chart_data(&p)), p);
        eprintln!("  → 3/5 lines parsed, round trip exact\n");
    }

    #[test]
    fn a_tween_starts_at_the_old_values_and_lands_on_the_new() {
        let from = pts(&[("Q1", 100.0), ("Q2", 200.0)]);
        let to = pts(&[("Q1", 300.0), ("Q2", 100.0)]);
        eprintln!("\n     t     Q1        Q2");
        eprintln!("  ----   ------   -------");
        let mut last = tween_series(&from, &to, 0.0);
        for t in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let f = tween_series(&from, &to, t);
            eprintln!("  {t:>4.2}   {:>6.1}   {:>7.1}", f[0].1, f[1].1);
            if t > 0.0 {
                assert!(f[0].1 > last[0].1, "Q1 must rise throughout, stalled at {t}");
                assert!(f[1].1 < last[1].1, "Q2 must fall throughout, stalled at {t}");
            }
            last = f;
        }
        let start = tween_series(&from, &to, 0.0);
        let end = tween_series(&from, &to, 1.0);
        assert_eq!((start[0].1, start[1].1), (100.0, 200.0), "t=0 is the OLD set");
        assert_eq!((end[0].1, end[1].1), (300.0, 100.0), "t=1 is the NEW set");
        eprintln!("  → 100→300 and 200→100, monotonic, exact at both ends\n");
    }

    #[test]
    fn labels_are_the_new_ones_and_a_new_point_grows_from_zero() {
        let from = pts(&[("Jan", 40.0)]);
        let to = pts(&[("Feb", 80.0), ("Mar", 60.0)]);
        let half = tween_series(&from, &to, 0.5);
        eprintln!("\n  half-way: {half:?}");
        assert_eq!(half[0].0, "Feb", "a half-played tween must not show the OLD label");
        assert_eq!(half[1].0, "Mar");
        assert!(
            half[0].1 > 40.0 && half[0].1 < 80.0,
            "the surviving point moves from 40 towards 80, got {}",
            half[0].1
        );
        assert!(
            half[1].1 > 0.0 && half[1].1 < 60.0,
            "the NEW point grows from 0 towards 60, got {}",
            half[1].1
        );
        // And a series that shrank simply draws fewer points.
        let shrunk = tween_series(&to, &from, 0.5);
        assert_eq!(shrunk.len(), 1, "the frame has as many points as the TARGET");
        eprintln!("  → labels follow the new data; a new point rises from 0\n");
    }

    #[test]
    fn the_duration_is_the_developers_but_never_below_the_floor() {
        use crate::model::ControlType;
        let mk = |on: bool, ms: Option<i64>| {
            let mut c = Control::new("CHART-1", ControlType::BarChart, 0, 0);
            c.set_prop("AnimateValues", PropValue::Bool(on));
            if let Some(ms) = ms {
                c.set_prop("AnimationDuration", PropValue::Int(ms));
            }
            c
        };
        eprintln!("\n  AnimateValues   AnimationDuration   effective");
        eprintln!("  -------------   -----------------   ---------");
        let cases: [(bool, Option<i64>, Option<i64>); 6] = [
            (false, None, None),
            (false, Some(5000), None),
            (true, None, Some(DEFAULT_ANIM_MS)),
            (true, Some(5000), Some(5000)),
            (true, Some(250), Some(250)),
            (true, Some(10), Some(MIN_ANIM_MS)),
        ];
        for (on, set, want) in cases {
            let got = value_anim_ms(&mk(on, set));
            eprintln!(
                "  {:<13}   {:<17}   {}",
                on,
                set.map(|v| v.to_string()).unwrap_or_else(|| "(default)".into()),
                got.map(|v| v.to_string()).unwrap_or_else(|| "off".into())
            );
            assert_eq!(got, want, "AnimateValues={on}, AnimationDuration={set:?}");
        }
        eprintln!("  → 6/6: off means off, the default is {DEFAULT_ANIM_MS} ms, the floor is {MIN_ANIM_MS} ms\n");
    }

    // ── RadarChart ──────────────────────────────────────────────────────────

    fn rows(data: &[(&str, &[f32])]) -> Vec<Row> {
        data.iter().map(|(l, v)| ((*l).to_owned(), v.to_vec())).collect()
    }

    fn near(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
    }

    /// Axis 1 points straight up and the rest follow clockwise, on screen
    /// (y grows downward): with four axes that is up, right, down, left.
    #[test]
    fn a_radar_starts_straight_up_and_goes_clockwise() {
        let dirs: Vec<(f32, f32)> = (0..4).map(|i| radar_dir(i, 4)).collect();
        assert!(near(dirs[0], (0.0, -1.0)), "axis 1 up: {:?}", dirs[0]);
        assert!(near(dirs[1], (1.0, 0.0)), "axis 2 right: {:?}", dirs[1]);
        assert!(near(dirs[2], (0.0, 1.0)), "axis 3 down: {:?}", dirs[2]);
        assert!(near(dirs[3], (-1.0, 0.0)), "axis 4 left: {:?}", dirs[3]);
        // Six axes: 60 degrees apart, the second one up and to the right.
        let second = radar_point((100.0, 100.0), 50.0, 1, 6, 1.0);
        assert!(near(second, (100.0 + 50.0 * 0.866_025_4, 100.0 - 25.0)), "{second:?}");
        // Half way out is half the radius; the centre is the centre.
        assert!(near(radar_point((100.0, 100.0), 50.0, 0, 6, 0.5), (100.0, 75.0)));
        assert!(near(radar_point((100.0, 100.0), 50.0, 3, 6, 0.0), (100.0, 100.0)));
        eprintln!("  radar axes: 4 -> up, right, down, left; 6 -> axis 2 at 60 degrees, clockwise");
    }

    #[test]
    fn nice_steps_are_one_two_two_and_a_half_five_or_ten() {
        let cases: [(f32, f32); 12] = [
            (17.4, 20.0), (20.0, 20.0), (46.0, 50.0), (2.1, 2.5), (2.5, 2.5), (4.0, 5.0),
            (0.07, 0.1), (1.0, 1.0), (101.0, 200.0), (0.0, 1.0), (-3.0, 1.0), (f32::NAN, 1.0),
        ];
        for (raw, want) in cases {
            let got = nice_step(raw);
            assert!((got - want).abs() <= want * 1e-4, "nice_step({raw}) = {got}, wanted {want}");
        }
    }

    /// The scale is shared by every axis: automatic from the largest value of
    /// any series, rounded so the rings are nice numbers; `MaxValue` as set when
    /// it is above `MinValue`.
    #[test]
    fn the_scale_is_shared_and_rounded_to_a_nice_number() {
        let data = rows(&[("A", &[80.0, 20.0]), ("B", &[87.0, 40.0]), ("C", &[10.0, 99.0])]);
        // Largest value 99, five rings: 99 / 5 = 19.8 rounds up to 20 a ring.
        let auto = radar_scale(&data, 0.0, 0.0, 5);
        assert_eq!((auto.min, auto.max), (0.0, 100.0));
        assert_eq!(auto.ring_value(2, 5), 40.0);
        // Four rings of 25 reach the same top.
        assert_eq!(radar_scale(&data, 0.0, 0.0, 4).max, 100.0);
        assert_eq!(radar_scale(&rows(&[("A", &[10.5])]), 0.0, 0.0, 5).max, 12.5);
        // Explicit bounds are used as they stand...
        let fixed = radar_scale(&data, 10.0, 200.0, 5);
        assert_eq!((fixed.min, fixed.max), (10.0, 200.0));
        // ...unless MaxValue is not above MinValue, which means automatic.
        assert_eq!(radar_scale(&data, 0.0, -5.0, 5).max, 100.0);
        assert_eq!((radar_scale(&data, 50.0, 50.0, 5).min, radar_scale(&data, 50.0, 50.0, 5).max), (50.0, 100.0));
        // A floor below zero scales from there: -50..50 in rings of 20.
        let below = radar_scale(&rows(&[("A", &[30.0]), ("B", &[-20.0])]), -50.0, 0.0, 5);
        assert_eq!((below.min, below.max), (-50.0, 50.0), "MaxValue 0 is automatic even above a negative MinValue");
        // No data above the floor: a unit scale, so the rings still have places.
        let flat = radar_scale(&rows(&[("A", &[0.0]), ("B", &[0.0])]), 0.0, 0.0, 5);
        assert!((flat.max - 1.0).abs() < 1e-6 && flat.min == 0.0, "{flat:?}");
        let junk = radar_scale(&rows(&[("A", &[f32::NAN, f32::INFINITY])]), 0.0, f32::NAN, 5);
        assert!((junk.max - 1.0).abs() < 1e-6, "values that are not numbers do not set the scale: {junk:?}");
        eprintln!("  radar scale: 99 -> 0..100 (rings of 20), 10.5 -> 0..12.5, fixed bounds kept, junk ignored");
    }

    /// Negative values sit on the centre, values past the top on the rim, and a
    /// value that is not a number on the centre -- never off the plot.
    #[test]
    fn a_value_off_the_scale_is_held_at_the_centre_or_the_rim() {
        let scale = RadarScale { min: 0.0, max: 100.0 };
        for (value, want) in [
            (50.0, 0.5),
            (0.0, 0.0),
            (-30.0, 0.0),
            (250.0, 1.0),
            (f32::NAN, 0.0),
            (f32::INFINITY, 1.0),
            (f32::NEG_INFINITY, 0.0),
        ] {
            assert_eq!(scale.fraction(value), want, "fraction({value})");
        }
        let signed = RadarScale { min: -50.0, max: 50.0 };
        assert_eq!(signed.fraction(0.0), 0.5, "zero is half way on a -50..50 scale");
        assert_eq!(signed.fraction(-80.0), 0.0, "below MinValue clamps to MinValue");
    }

    /// Polygons are the data at the vertices: `[series][axis]`, each vertex
    /// its fraction of the way out; a short row is 0 in the series it leaves out;
    /// `grow` pulls every vertex towards the centre (the load animation).
    #[test]
    fn vertices_sit_at_their_fraction_of_the_radius() {
        let layout = RadarLayout { centre: (100.0, 100.0), radius: 80.0, labels: vec![] };
        let scale = RadarScale { min: 0.0, max: 100.0 };
        let data = rows(&[("N", &[100.0, 50.0]), ("E", &[50.0]), ("S", &[100.0, 100.0]), ("W", &[0.0, -10.0])]);
        let v = radar_vertices(&layout, &scale, &data, 1.0);
        assert_eq!(v.len(), 2, "two series");
        assert!(near(v[0][0], (100.0, 20.0)), "series 1, axis 1: rim, straight up: {:?}", v[0][0]);
        assert!(near(v[0][1], (140.0, 100.0)), "half way out to the right: {:?}", v[0][1]);
        assert!(near(v[0][2], (100.0, 180.0)));
        assert!(near(v[0][3], (100.0, 100.0)), "0 is the centre");
        assert!(near(v[1][1], (100.0, 100.0)), "the short row is 0 in series 2");
        assert!(near(v[1][3], (100.0, 100.0)), "a negative value sits on the centre");
        let grown = radar_vertices(&layout, &scale, &data, 0.5);
        assert!(near(grown[0][0], (100.0, 60.0)), "half grown: half way: {:?}", grown[0][0]);
        let seed = radar_vertices(&layout, &scale, &data, 0.0);
        assert!(seed.iter().flatten().all(|&p| near(p, (100.0, 100.0))), "not grown at all: all at the centre");
    }

    /// A value at `MinValue`, below it or not a number is 0 -- which is how the
    /// painter knows to put no marker on the centre for it.
    #[test]
    fn a_value_at_the_floor_has_a_zero_fraction() {
        let scale = RadarScale { min: 0.0, max: 100.0 };
        let data = rows(&[("A", &[0.0, 50.0]), ("B", &[-5.0, f32::NAN]), ("C", &[100.0])]);
        assert_eq!(radar_fractions(&data, &scale), vec![vec![0.0, 0.0, 1.0], vec![0.5, 0.0, 0.0]]);
    }

    /// Captions are shortened only when they cost the circle room: ten short
    /// ones stay whole and the circle takes the height the top and bottom
    /// captions leave; ten long ones are cut just as far as they must be.
    #[test]
    fn captions_are_shortened_only_as_far_as_the_circle_needs() {
        let plot = [6.0, 4.0, 305.0, 194.0]; // 311 x 198, no title, no legend
        let font = 15.3;
        let short: Vec<String> = (1..=10).map(|i| format!("Axis {i}")).collect();
        let whole = radar_layout(plot, &short, font);
        assert_eq!(whole.labels, short, "short captions stay whole");
        let (_, lh) = radar_label_size("Axis 1", font);
        let vertical_limit = 95.0 - lh - RADAR_LABEL_GAP;
        assert!(whole.radius >= vertical_limit - 0.5, "the circle takes all the height there is: {} vs {vertical_limit}", whole.radius);
        let long: Vec<String> = (1..=10).map(|i| format!("A rather long axis caption number {i}")).collect();
        let cut = radar_layout(plot, &long, font);
        assert!(cut.labels.iter().all(|t| t.ends_with('\u{2026}') && t.chars().count() < 14), "{:?}", cut.labels);
        assert!(cut.radius >= whole.radius * 0.9, "cutting them keeps the circle: {} vs {}", cut.radius, whole.radius);
        eprintln!("  radar 311x198, 10 axes: radius {:.1} with whole captions, {:.1} with long ones cut", whole.radius, cut.radius);
    }

    /// Nothing to draw, one axis, two axes: no panic, no garbage.
    #[test]
    fn degenerate_radars_stay_sane() {
        let plot = [0.0, 0.0, 280.0, 260.0];
        let scale = RadarScale { min: 0.0, max: 100.0 };
        // No axes.
        let none = radar_layout(plot, &[], 9.0);
        assert!(none.radius > 0.0 && none.radius <= 130.0 && none.labels.is_empty());
        assert!(radar_vertices(&none, &scale, &[], 1.0).is_empty());
        assert_eq!(radar_series_count(&[]), 0);
        assert_eq!(radar_nearest(&[], (1.0, 1.0), 50.0), None);
        // One axis: a single vertex per series, straight up, at its value.
        let one_rows = rows(&[("Only", &[50.0, 100.0])]);
        let one = radar_layout(plot, &["Only".to_owned()], 9.0);
        let v = radar_vertices(&one, &scale, &one_rows, 1.0);
        assert_eq!((v.len(), v[0].len()), (2, 1));
        assert!(near(v[0][0], (one.centre.0, one.centre.1 - one.radius * 0.5)));
        assert!(near(v[1][0], (one.centre.0, one.centre.1 - one.radius)));
        // Two axes: opposite ends of one line.
        let two_rows = rows(&[("Up", &[100.0]), ("Down", &[100.0])]);
        let two = radar_layout(plot, &["Up".to_owned(), "Down".to_owned()], 9.0);
        let v = radar_vertices(&two, &scale, &two_rows, 1.0);
        assert!(near(v[0][0], (two.centre.0, two.centre.1 - two.radius)));
        assert!(near(v[0][1], (two.centre.0, two.centre.1 + two.radius)));
        // A plot with no room at all, or less than none.
        for tiny in [[0.0, 0.0, 0.0, 0.0], [10.0, 10.0, 4.0, 4.0], [0.0, 0.0, 3.0, 300.0]] {
            let l = radar_layout(tiny, &["Alpha".to_owned(), "Beta".to_owned(), "Gamma".to_owned()], 9.0);
            assert!(l.radius.is_finite() && l.radius >= 0.0, "{tiny:?} -> {l:?}");
            let three = rows(&[("A", &[1.0]), ("B", &[2.0]), ("C", &[3.0])]);
            assert!(radar_vertices(&l, &scale, &three, 1.0).iter().flatten().all(|p| p.0.is_finite() && p.1.is_finite()));
        }
        eprintln!("  radar degenerate data: 0 axes empty, 1 axis a point, 2 axes a line, no room -> finite");
    }

    /// Every caption, hung outward off its spoke, lies inside the plot -- for
    /// any number of axes and for captions far too long for the room -- and
    /// the circle keeps most of the plot it was given.
    #[test]
    fn captions_stay_inside_the_plot() {
        let plot = [10.0, 30.0, 290.0, 270.0]; // 280 x 240, below a title
        let font = 9.0;
        let long = "An axis caption that is much too long to fit beside the rim";
        let mut checked = 0;
        for axes in 1..=16usize {
            for label in ["Speed", long] {
                let labels: Vec<String> = (0..axes).map(|i| format!("{label} {i}")).collect();
                let layout = radar_layout(plot, &labels, font);
                assert_eq!(layout.labels.len(), axes);
                let half = ((plot[2] - plot[0]).min(plot[3] - plot[1]) * 0.5 - RADAR_RIM_PAD).max(0.0);
                assert!(
                    layout.radius >= half * 0.45 - 1e-3 && layout.radius <= half + 1e-3,
                    "{axes} axes: radius {}",
                    layout.radius
                );
                for (i, text) in layout.labels.iter().enumerate() {
                    let dir = radar_dir(i, axes);
                    let anchor = radar_point(layout.centre, layout.radius + RADAR_LABEL_GAP, i, axes, 1.0);
                    let (w, h) = radar_label_size(text, font);
                    let (ax, ay) = radar_label_anchor(dir);
                    let (xl, xr) = extent(ax, w);
                    let (yt, yb) = extent(ay, h);
                    let (x0, x1, y0, y1) = (anchor.0 + xl, anchor.0 + xr, anchor.1 + yt, anchor.1 + yb);
                    assert!(
                        x0 >= plot[0] - 0.5 && x1 <= plot[2] + 0.5 && y0 >= plot[1] - 0.5 && y1 <= plot[3] + 0.5,
                        "{axes} axes, axis {i} `{text}`: box ({x0:.1},{y0:.1})-({x1:.1},{y1:.1}) leaves the plot {plot:?}"
                    );
                    checked += 1;
                }
            }
        }
        eprintln!("  radar captions: {checked} captions over 1-16 axes, short and over-long, all inside the plot");
    }

    #[test]
    fn a_long_caption_is_cut_with_an_ellipsis() {
        assert_eq!(radar_fit_label("Speed", 9.0, 200.0), "Speed");
        let cut = radar_fit_label("A very long axis caption", 9.0, 50.0);
        assert!(cut.ends_with('\u{2026}') && cut.chars().count() < 24, "{cut}");
        assert_eq!(radar_fit_label("x", 9.0, 0.0), "x", "one character always fits");
    }

    /// The nearest vertex of any polygon within reach; of two on the same
    /// spot the later series, which is drawn on top.
    #[test]
    fn hover_finds_the_nearest_vertex() {
        let layout = RadarLayout { centre: (100.0, 100.0), radius: 80.0, labels: vec![] };
        let scale = RadarScale { min: 0.0, max: 100.0 };
        let data = rows(&[
            ("N", &[100.0, 100.0, 40.0]),
            ("E", &[100.0, 20.0, 100.0]),
            ("S", &[100.0, 100.0, 100.0]),
            ("W", &[100.0, 50.0, 100.0]),
        ]);
        let v = radar_vertices(&layout, &scale, &data, 1.0);
        // Series 1 and 2 meet at the top of axis 1; the later one wins there.
        assert_eq!(radar_nearest(&v, (100.0, 21.0), 12.0), Some((1, 0)));
        // Series 3's own vertex on that axis, lower down.
        assert_eq!(radar_nearest(&v, (100.0, 68.0), 12.0), Some((2, 0)));
        // Right of the centre, series 2's vertex is the close one.
        assert_eq!(radar_nearest(&v, (100.0 + 16.0, 101.0), 12.0), Some((1, 1)));
        // Far from everything.
        assert_eq!(radar_nearest(&v, (500.0, 500.0), 12.0), None);
        // A vertex that is not a number is never picked.
        assert_eq!(radar_nearest(&[vec![(f32::NAN, f32::NAN)]], (0.0, 0.0), 1e9), None);
    }

    #[test]
    fn ring_values_are_written_with_just_the_decimals_they_need() {
        for (value, step, want) in [
            (40.0, 20.0, "40"),
            (7.5, 2.5, "7.5"),
            (0.75, 0.25, "0.75"),
            (0.2, 0.2, "0.2"),
            (-0.0, 20.0, "0"),
            (-50.0, 25.0, "-50"),
            (1500.0, 500.0, "1500"),
        ] {
            assert_eq!(radar_value_text(value, step), want, "value {value}, step {step}");
        }
    }

    #[test]
    fn the_sample_is_a_believable_three_series_six_axis_radar() {
        let sample = radar_sample_rows(3);
        assert_eq!((sample.len(), radar_series_count(&sample)), (6, 3));
        assert!(sample.iter().flat_map(|(_, v)| v).all(|&x| (0.0..=100.0).contains(&x)));
        assert_ne!(sample[0].1[0], sample[0].1[1], "the series differ");
        assert_eq!(radar_series_count(&radar_sample_rows(5)), 5, "a bound chart's field count");
        assert_eq!(radar_scale(&sample, 0.0, 0.0, 5).max, 100.0);
    }

    /// `GridLevels` is 1 to 10 (5 by default), `FillOpacity` 0 to 100 (35), the
    /// scale bounds numbers that may carry decimals.
    #[test]
    fn the_radar_properties_are_read_leniently() {
        use crate::model::ControlType;
        let mut c = Control::new("R", ControlType::RadarChart, 0, 0);
        assert_eq!((radar_levels(&c), radar_fill_opacity(&c), radar_marker_radius(&c)), (5, 0.35, 3.0));
        for (set, want) in [
            (PropValue::Int(0), 1),
            (PropValue::Int(99), 10),
            (PropValue::Int(7), 7),
            (PropValue::String("3".into()), 3),
            (PropValue::String("lots".into()), 5),
        ] {
            c.set_prop("GridLevels", set.clone());
            assert_eq!(radar_levels(&c), want, "GridLevels = {set:?}");
        }
        c.set_prop("FillOpacity", PropValue::Int(150));
        assert_eq!(radar_fill_opacity(&c), 1.0);
        c.set_prop("FillOpacity", PropValue::Int(0));
        assert_eq!(radar_fill_opacity(&c), 0.0);
        c.set_prop("MinValue", PropValue::String("-2.5".into()));
        c.set_prop("MaxValue", PropValue::String("1.5".into()));
        assert_eq!(radar_bounds(&c), (-2.5, 1.5));
        c.set_prop("MaxValue", PropValue::String("n/a".into()));
        assert_eq!(radar_bounds(&c), (-2.5, 0.0), "text that is not a number means automatic");
    }
}
