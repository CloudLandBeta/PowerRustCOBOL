// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A chart as the desktop painter draws it** (WASM-016 R14, look-and-feel parity): the bar, line,
//! pie, donut and radar charts, laid out and inked the way `cobolt-forms`' `draw_chart_preview` lays
//! them out and inks them, so a form shows the same chart in the designer, in Run Form and in a browser.
//!
//! What this module is: the painter's own arithmetic, ported to plain numbers and written as SVG — the
//! plot's frame (the margins, the 60 % cap, the bands for the caption and the legend), the bars and their
//! slots, the Catmull-Rom curve and its markers, the polygonal slices, the legend laid out as one centred
//! group from the real width of each entry, the axis lines and the grid. Where the painter's numbers live
//! in `cobolt_forms::chart` (the radar's layout, the rows, the range) they are used from there, not copied.
//!
//! What it leaves out, on purpose: the load animation (`__ChartGrow` is 1 at rest), hover tips (the
//! browser's own `<title>`), the border and the gradient ring of a chart that shows its background (the
//! chart's own box paints them, as CSS) and the Monochrome look (the older renderer keeps it).
//!
//! A chart is drawn for the size its box really has, not for the size it was designed at (`ViewCtx::box_size`):
//! the desktop's solver gives a chart in a grid its cell before its painter draws it.

use cobolt_forms::chart::{self, parse_chart_rows, radar_layout, radar_point, value_range, Row};
use cobolt_forms::model::ControlType;

use super::{VNode, ViewCtx};
use crate::css::class_names;
use crate::metrics;
use crate::value::{fmt_number, parse_color, parse_f64, truthy};

/// The type size every chart's text is scaled from, and the factor the painter multiplies it by
/// (`paint::CHART_FONT_BASE`, `CHART_FONT_SCALE`).
const CHART_FONT_BASE: f64 = 14.0;
const CHART_FONT_SCALE: f64 = 2.0;

type Rgba = [u8; 4];

fn f(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    fmt_number((v * 100.0).round() / 100.0)
}

fn rgb(c: Rgba) -> String {
    if c[3] == 255 {
        format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
    } else {
        format!("rgb({} {} {} / {})", c[0], c[1], c[2], fmt_number((c[3] as f64 / 255.0 * 1000.0).round() / 1000.0))
    }
}

fn sv(name: &'static str) -> VNode {
    VNode::svg(name)
}

// ── colour contrast, as the painter judges it ────────────────────────────────

/// WCAG relative luminance of an opaque colour.
fn luminance(c: Rgba) -> f64 {
    let lin = |v: u8| {
        let s = v as f64 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

fn contrast(a: Rgba, b: Rgba) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// `paint::caret_color`: the colour itself while it clears 4.5:1 on the surface, else the better pole.
fn caret_color(surface: Rgba, text: Rgba) -> Rgba {
    if contrast(text, surface) >= 4.5 {
        return text;
    }
    if contrast([0, 0, 0, 255], surface) >= contrast([255, 255, 255, 255], surface) {
        [0, 0, 0, 255]
    } else {
        [255, 255, 255, 255]
    }
}

// ── reading the control ──────────────────────────────────────────────────────

struct Ctx<'a> {
    cx: &'a ViewCtx<'a>,
    i: usize,
    ty: ControlType,
    w: f64,
    h: f64,
}

impl Ctx<'_> {
    /// What the control STATES (or a program wrote): the painter reads a property a loaded control does not carry as absent and
    /// takes its own default, not the seeded one a program reads back (`FormTree::get`).
    fn s(&self, k: &str) -> String {
        self.cx.tree.ctl(self.i).raw(k).map(str::to_owned).unwrap_or_default()
    }
    fn trimmed(&self, k: &str) -> String {
        self.s(k).trim().to_owned()
    }
    fn flag(&self, k: &str, default: bool) -> bool {
        let v = self.s(k);
        if v.trim().is_empty() {
            default
        } else {
            truthy(&v)
        }
    }
    fn num(&self, k: &str) -> Option<f64> {
        parse_f64(&self.s(k))
    }
    fn is_pie(&self) -> bool {
        matches!(self.ty, ControlType::PieChart | ControlType::DonutChart)
    }
}

/// A chart's own colours: `SeriesColors` when the developer set a palette that is not the seeded default,
/// else the desktop's built-in four (`paint::draw_chart_preview`'s `pal_raw`).
fn palette(c: &Ctx) -> Vec<Rgba> {
    const SEEDED: &str = "#4C9BE8,#E87A4C,#4CE87A,#E84C9B,#9B4CE8,#E8C84C";
    let raw = c.trimmed("SeriesColors");
    let norm = |s: &str| s.split(',').map(|x| x.trim().to_ascii_uppercase()).collect::<Vec<_>>();
    if !raw.is_empty() && norm(&raw) != norm(SEEDED) {
        let colors: Vec<Rgba> = raw.split(',').map(str::trim).filter(|s| !s.is_empty()).filter_map(parse_color).filter(|c| c[3] > 0).collect();
        if !colors.is_empty() {
            return colors;
        }
    }
    vec![[76, 155, 232, 255], [232, 122, 76, 255], [76, 232, 122, 255], [232, 76, 155, 255]]
}

fn series_names(c: &Ctx, count: usize) -> Vec<String> {
    let labels = c.s("SeriesLabels");
    let labels: Vec<&str> = labels.split(',').map(str::trim).collect();
    let fields = c.s("ValueFields");
    let fields: Vec<&str> = fields.split(',').map(str::trim).filter(|f| !f.is_empty()).collect();
    (0..count)
        .map(|i| {
            [labels.get(i), fields.get(i)]
                .into_iter()
                .flatten()
                .find(|n| !n.is_empty())
                .map(|n| (*n).to_owned())
                .unwrap_or_else(|| format!("Series {}", i + 1))
        })
        .collect()
}

// ── the frame (`paint::chart_frame`) ─────────────────────────────────────────

struct Frame {
    /// `(left, top, right, bottom)` of the plot, in the control's own pixels.
    plot: (f64, f64, f64, f64),
    cap_h: f64,
    cap_w: f64,
    legend_w: f64,
    legend_h: f64,
    margin_t: f64,
    margin_b: f64,
    title_font: f64,
    type_scale: f64,
}

impl Frame {
    fn pw(&self) -> f64 {
        self.plot.2 - self.plot.0
    }
    fn ph(&self) -> f64 {
        self.plot.3 - self.plot.1
    }
    fn centre(&self) -> (f64, f64) {
        ((self.plot.0 + self.plot.2) / 2.0, (self.plot.1 + self.plot.3) / 2.0)
    }
}

fn frame(c: &Ctx) -> Frame {
    let font = c.cx.tree.font_size_of(c.i).unwrap_or(CHART_FONT_BASE).clamp(4.0, 200.0);
    let type_scale = CHART_FONT_SCALE * (font / CHART_FONT_BASE);
    let is_pie = c.is_pie();
    let x_caption = c.trimmed("XAxisLabel");
    let y_caption = c.trimmed("YAxisLabel");
    let show_legend = c.flag("ShowLegend", false);
    let title_text = c.trimmed("Title");
    let cap_h = if x_caption.is_empty() { 0.0 } else { 13.0 * type_scale };
    let cap_w = if y_caption.is_empty() { 0.0 } else { 13.0 * type_scale };
    let legend_w = if show_legend && is_pie { (c.w * 0.30).min(90.0 * type_scale) } else { 0.0 };
    let legend_h = if show_legend && !is_pie { 13.0 * type_scale } else { 0.0 };
    let title_font = match c.num("TitleFontSize") {
        Some(pt) if pt > 0.0 => pt.clamp(4.0, 200.0),
        _ => 10.0 * type_scale,
    };
    let title_band = if title_text.is_empty() { 0.0 } else { title_font * 1.5 };
    let mut margin_l = c.w * 0.10 + cap_w;
    let mut margin_b = c.h * 0.12 + cap_h + legend_h;
    let mut margin_t = (c.h * 0.12).max(title_band);
    let mut margin_r = c.w * 0.04 + legend_w;
    if c.ty == ControlType::RadarChart {
        margin_l = c.w * 0.02;
        margin_r = c.w * 0.02;
        margin_t = if title_text.is_empty() { c.h * 0.02 } else { title_band + 2.0 };
        margin_b = c.h * 0.02 + legend_h;
    }
    let (max_v, max_h) = (c.h * 0.60, c.w * 0.60);
    let v = margin_t + margin_b;
    if v > max_v && v > 0.0 {
        let k = max_v / v;
        margin_t *= k;
        margin_b *= k;
    }
    let hz = margin_l + margin_r;
    if hz > max_h && hz > 0.0 {
        let k = max_h / hz;
        margin_l *= k;
        margin_r *= k;
    }
    Frame { plot: (margin_l, margin_t, c.w - margin_r, c.h - margin_b), cap_h, cap_w, legend_w, legend_h, margin_t, margin_b, title_font, type_scale }
}

// ── the data (`paint::chart_geometry`) ───────────────────────────────────────

struct Geometry {
    live: Vec<(String, f32)>,
    rows: Vec<Row>,
    plotted: Vec<Vec<f32>>,
    bases: Vec<Vec<f32>>,
    tops: Vec<Vec<f32>>,
    zero: f32,
    stacked: bool,
    series_count: usize,
    n: usize,
    grow: f32,
}

fn geometry(c: &Ctx) -> Geometry {
    let raw = c.s("__ChartData");
    let rows: Vec<Row> = if raw.trim().is_empty() { Vec::new() } else { parse_chart_rows(&raw) };
    let live: Vec<(String, f32)> = rows.iter().map(|(l, v)| (l.clone(), v.first().copied().unwrap_or(0.0))).collect();
    let category = matches!(c.ty, ControlType::BarChart | ControlType::LineChart | ControlType::AreaChart | ControlType::RadarChart);
    let series_count = if category { rows.iter().map(|(_, v)| v.len()).max().unwrap_or(0) } else { rows.len().min(1) };
    let stacked = matches!(c.ty, ControlType::BarChart | ControlType::AreaChart) && c.flag("Stacked", false);
    let grow = c.s("__ChartGrow").trim().parse::<f32>().unwrap_or(1.0).clamp(0.0, 1.0);
    let (lo, hi) = if category && !live.is_empty() { value_range(&rows, stacked) } else { (0.0, 0.0) };
    let span = (hi - lo).max(f32::EPSILON);
    let zero = if category && !live.is_empty() { (0.0 - lo) / span } else { 0.0 };
    let plotted: Vec<Vec<f32>> = if live.is_empty() {
        let sample1: &[f32] = &[0.40, 0.70, 0.55, 0.85, 0.60];
        let sample2: &[f32] = &[0.25, 0.45, 0.70, 0.50, 0.80];
        let count = c.s("ValueFields").split(',').filter(|f| !f.trim().is_empty()).count();
        let count = match count {
            0 if c.ty == ControlType::RadarChart => 3,
            0 => 2,
            k => k,
        };
        (0..count)
            .map(|j| match j {
                0 => sample1.to_vec(),
                1 => sample2.to_vec(),
                j => (0..5).map(|i| sample1[(i + j) % 5] * 0.9).collect(),
            })
            .collect()
    } else if category {
        let value = |r: &Row, s: usize| r.1.get(s).copied().unwrap_or(0.0);
        (0..series_count).map(|s| rows.iter().map(|r| zero + (((value(r, s) - lo) / span).clamp(0.0, 1.0) - zero) * grow).collect()).collect()
    } else {
        let first: Vec<Row> = live.iter().map(|(l, v)| (l.clone(), vec![*v])).collect();
        let (s_lo, s_hi) = value_range(&first, false);
        let s_span = (s_hi - s_lo).max(f32::EPSILON);
        let sz = (0.0 - s_lo) / s_span;
        vec![live.iter().map(|(_, v)| sz + (((v - s_lo) / s_span).clamp(0.0, 1.0) - sz) * grow).collect()]
    };
    let len = plotted.first().map(Vec::len).unwrap_or(0);
    let (mut up, mut down) = (vec![zero; len], vec![zero; len]);
    let (mut bases, mut tops) = (Vec::new(), Vec::new());
    for series in &plotted {
        let (mut base, mut top) = (Vec::with_capacity(series.len()), Vec::with_capacity(series.len()));
        for (i, &v) in series.iter().enumerate() {
            if !stacked {
                base.push(zero);
                top.push(v.min(1.0));
            } else if v >= zero {
                let b = up[i];
                let t = ((v - zero) + b).min(1.0);
                up[i] = t;
                base.push(b);
                top.push(t);
            } else {
                let b = down[i];
                let t = ((v - zero) + b).max(0.0);
                down[i] = t;
                base.push(b);
                top.push(t);
            }
        }
        bases.push(base);
        tops.push(top);
    }
    let n = plotted.first().map(Vec::len).unwrap_or(0).max(1);
    Geometry { live, rows, plotted, bases, tops, zero, stacked, series_count, n, grow }
}

/// `paint::chart_bar_rect`: where bar `i` of series `si` (of `k`) is drawn, as `(x, y, w, h)`.
#[allow(clippy::too_many_arguments)]
fn bar_rect(plot: (f64, f64, f64, f64), horizontal: bool, stacked: bool, k: usize, n: usize, base: f64, top: f64, si: usize, i: usize) -> (f64, f64, f64, f64) {
    let (lo, hi) = (base.min(top), base.max(top));
    let (pw, ph) = (plot.2 - plot.0, plot.3 - plot.1);
    let bar_total = pw / n as f64;
    let (bar_w, gap) = if k > 2 && !stacked { (bar_total * 0.76 / k as f64, bar_total * 0.10 / k as f64) } else { (bar_total * 0.38, bar_total * 0.05) };
    let slot_h = ph / n as f64;
    if stacked {
        if horizontal {
            let th = slot_h * 0.6;
            let y = plot.1 + i as f64 * slot_h + (slot_h - th) * 0.5;
            (plot.0 + lo * pw, y, (hi - lo) * pw, th)
        } else {
            let w = bar_total * 0.6;
            let x = plot.0 + i as f64 * bar_total + (bar_total - w) * 0.5;
            (x, plot.3 - hi * ph, w, (hi - lo) * ph)
        }
    } else if horizontal && k > 2 {
        let th = slot_h * 0.76 / k as f64;
        let g = slot_h * 0.10 / k as f64;
        let y = plot.1 + i as f64 * slot_h + g + si as f64 * (th + g);
        (plot.0 + lo * pw, y, (hi - lo) * pw, th)
    } else if horizontal {
        let y = plot.1 + (i as f64 + 0.5 + si as f64 * (0.5 + gap)) / n as f64 * ph - bar_w * 0.5;
        (plot.0 + lo * pw, y, (hi - lo) * pw, bar_w)
    } else {
        let x = plot.0 + (i as f64 * bar_total) + si as f64 * (bar_w + gap) + gap;
        (x, plot.3 - hi * ph, bar_w, (hi - lo) * ph)
    }
}

/// `paint::catmull_rom`: the polyline bent into a spline through its points, `seg` steps to a span.
fn catmull_rom(pts: &[(f64, f64)], seg: usize) -> Vec<(f64, f64)> {
    if pts.len() < 3 || seg < 2 {
        return pts.to_vec();
    }
    let n = pts.len();
    let mut out = Vec::with_capacity((n - 1) * seg + 1);
    for i in 0..n - 1 {
        let (p0, p1, p2, p3) = (pts[i.saturating_sub(1)], pts[i], pts[i + 1], pts[(i + 2).min(n - 1)]);
        for s in 0..seg {
            let t = s as f64 / seg as f64;
            let (t2, t3) = (t * t, t * t * t);
            let at = |a: f64, b: f64, c: f64, d: f64| 0.5 * ((2.0 * b) + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3);
            out.push((at(p0.0, p1.0, p2.0, p3.0), at(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    out.push(pts[n - 1]);
    out
}

// ── text, as the desktop lays it out ─────────────────────────────────────────

/// Where a line of text sits when the painter anchors it: the galley's top is `anchor_y − row/2` for a
/// vertically centred anchor, snapped to a whole pixel, and the baseline is a rounded ascent below it.
fn baseline_centred(y: f64, size: f64) -> f64 {
    (y - metrics::row_height(size) / 2.0).round() + metrics::ascent(size)
}

fn text(x: f64, baseline: f64, anchor: &'static str, size: f64, s: &str, fill: Rgba) -> VNode {
    // The painter anchors a text by its measured width and puts the whole galley on a whole pixel (`Painter::text`, then the
    // tessellator's rounding): where the text starts is that pixel, whatever the anchor.
    let w = metrics::text_width(s, size);
    let start = match anchor {
        "middle" => x - w / 2.0,
        "end" => x - w,
        _ => x,
    };
    let mut t = sv("text").attr("x", f(start.round())).attr("y", f(baseline)).attr("text-anchor", "start").attr("font-size", f(size)).attr("fill", rgb(fill));
    // A browser's glyphs carry less ink than the desktop's at small sizes: an outline of the measured width brings them to the
    // same weight (`metrics::stroke`; the page's own `-webkit-text-stroke` is off for chart text, and an attribute is what
    // the page's CSP allows).
    let outline = metrics::stroke(size, fill);
    if outline > 0.0 {
        t = t.attr("stroke", rgb(fill)).attr("stroke-width", f(outline));
    }
    t.text(s.to_owned())
}

fn line(x1: f64, y1: f64, x2: f64, y2: f64, width: f64, stroke: Rgba) -> VNode {
    sv("line").attr("x1", f(x1)).attr("y1", f(y1)).attr("x2", f(x2)).attr("y2", f(y2)).attr("stroke", rgb(stroke)).attr("stroke-width", f(width))
}

// ── the chart ────────────────────────────────────────────────────────────────

/// The chart as the desktop paints it, or `None` for a kind this module does not draw (the older
/// renderer keeps the area and scatter charts).
pub fn chart(cx: &ViewCtx, i: usize) -> Option<VNode> {
    let ty = cx.tree.ctl(i).control_type()?.clone();
    if !matches!(ty, ControlType::BarChart | ControlType::LineChart | ControlType::PieChart | ControlType::DonutChart | ControlType::RadarChart) {
        return None;
    }
    if truthy(&cx.tree.get(i, "Monochrome")) {
        return None;
    }
    let (w, h) = cx.box_size(i);
    let (w, h) = (w.max(16.0), h.max(16.0));
    let c = Ctx { cx, i, ty: ty.clone(), w, h };
    let fr = frame(&c);
    let geo = geometry(&c);
    let pal = palette(&c);
    let ctl = cx.tree.ctl(i);
    let name = ctl.name.clone();

    // The face: the colour `chart` paints its marks over, which also inks the captions and the slices' rims.
    let face = parse_color(&c.s("BackgroundColor")).filter(|f| f[3] > 0).unwrap_or([15, 20, 45, 255]);
    let face_a = ((255.0_f64) * (1.0 - c.num("Transparency").unwrap_or(0.0).clamp(0.0, 100.0) / 100.0)).round() as u8;
    let bg: Rgba = [face[0], face[1], face[2], face_a];

    let class = match ty {
        ControlType::BarChart => "pr-barchart",
        ControlType::LineChart => "pr-linechart",
        ControlType::PieChart => "pr-piechart",
        ControlType::DonutChart => "pr-donutchart",
        _ => "pr-radarchart",
    };
    let mut root = VNode::new("div")
        .key("root")
        .classes(class_names(ctl))
        .attr("data-name", name.clone())
        .attr("id", name.clone())
        .attr("role", "img");
    if !ctl.visible() {
        root = root.flag("hidden", true);
    }
    for d in super::controls::bound_dom_events(ctl) {
        root = root.on(d);
    }

    let (pl, pt, pr, pb) = fr.plot;
    let clip_plot = format!("pr-plot-{name}");
    let clip_all = format!("pr-chrome-{name}");
    let mut svg = sv("svg").key("svg").class(class).attr("viewBox", format!("0 0 {} {}", f(w), f(h))).attr("preserveAspectRatio", "none").attr("width", f(w)).attr("height", f(h));
    // The painter clips the data to the plot grown by 8 px, and the margins' text to the control less a pixel.
    let (cl, ct, cr, cb) = ((pl - 8.0).max(1.0), (pt - 8.0).max(1.0), (pr + 8.0).min(w - 1.0), (pb + 8.0).min(h - 1.0));
    svg = svg.child(sv("defs").child(sv("clipPath").attr("id", clip_plot.clone()).child(sv("rect").attr("x", f(cl)).attr("y", f(ct)).attr("width", f((cr - cl).max(0.0))).attr("height", f((cb - ct).max(0.0)))))
        .child(sv("clipPath").attr("id", clip_all.clone()).child(sv("rect").attr("x", "1").attr("y", "1").attr("width", f((w - 2.0).max(0.0))).attr("height", f((h - 2.0).max(0.0))))));

    let mut data = sv("g").attr("clip-path", format!("url(#{clip_plot})"));
    let grid_on = c.flag("ShowGridLines", true) && ty != ControlType::RadarChart;
    if grid_on {
        let grid = [118, 142, 225, 255];
        for k in 1..4 {
            let y = pt + (pb - pt) * k as f64 / 4.0;
            data = data.child(line(pl, y, pr, y, 1.15, grid));
        }
        if !c.is_pie() {
            for k in 1..5 {
                let x = pl + (pr - pl) * k as f64 / 5.0;
                data = data.child(line(x, pt, x, pb, 1.15, grid));
            }
        }
    }
    let ax = [84, 104, 190, 255];
    if !matches!(ty, ControlType::PieChart | ControlType::DonutChart | ControlType::RadarChart) {
        let zero = geo.zero as f64;
        if c.flag("ShowXAxis", true) {
            let y = pb - zero * (pb - pt);
            data = data.child(line(pl, y, pr, y, 1.45, ax));
        }
        if c.flag("ShowYAxis", true) {
            data = data.child(line(pl, pb, pl, pt, 1.45, ax));
        }
    }

    let ph = pb - pt;
    let px_x = |i: usize| pl + (i as f64 + 0.5) / geo.n as f64 * (pr - pl);
    let px_y = |v: f64| pb - v * ph;
    match ty {
        ControlType::BarChart => {
            let horizontal = c.flag("Horizontal", false);
            let corner = c.num("BarCornerRadius").unwrap_or(3.0).max(0.0);
            let k = geo.plotted.len();
            for (si, series) in geo.plotted.iter().enumerate() {
                for i in 0..series.len() {
                    let (x, y, bw, bh) = bar_rect(fr.plot, horizontal, geo.stacked, k, geo.n, geo.bases[si][i] as f64, geo.tops[si][i] as f64, si, i);
                    if bw <= 0.0 || bh <= 0.0 {
                        continue;
                    }
                    let r = if geo.stacked && si + 1 < k { 0.0 } else { corner.min(bw * 0.5).min(bh * 0.5) };
                    let mut rect = sv("rect").attr("x", f(x)).attr("y", f(y)).attr("width", f(bw)).attr("height", f(bh)).attr("fill", rgb(pal[si % pal.len()]));
                    if r > 0.0 {
                        rect = rect.attr("rx", f(r));
                    }
                    data = data.child(rect);
                }
            }
        }
        ControlType::LineChart => {
            let smooth = c.flag("Smooth", true);
            let show_points = c.flag("ShowPoints", true);
            let point_r = c.num("PointRadius").unwrap_or(4.0).clamp(0.5, 40.0);
            for (si, series) in geo.plotted.iter().enumerate() {
                let raw: Vec<(f64, f64)> = series.iter().enumerate().map(|(i, &v)| (px_x(i), px_y(v as f64))).collect();
                let curve = if smooth { catmull_rom(&raw, 14) } else { raw.clone() };
                let colour = pal[si % pal.len()];
                let pts = curve.iter().map(|(x, y)| format!("{},{}", f(*x), f(*y))).collect::<Vec<_>>().join(" ");
                data = data.child(sv("polyline").attr("points", pts).attr("fill", "none").attr("stroke", rgb(colour)).attr("stroke-width", "1.8").attr("stroke-linejoin", "round"));
                if show_points {
                    for &(x, y) in &raw {
                        data = data.child(sv("circle").attr("cx", f(x)).attr("cy", f(y)).attr("r", f(point_r)).attr("fill", rgb(colour)));
                    }
                }
            }
        }
        ControlType::PieChart | ControlType::DonutChart => {
            let (cx0, cy0) = fr.centre();
            let outer = fr.pw().min(fr.ph()) * 0.44;
            let inner = if ty == ControlType::DonutChart { outer * c.num("InnerRadius").unwrap_or(40.0) / 100.0 } else { 0.0 };
            let slices: Vec<f64> = if geo.live.is_empty() {
                vec![0.30, 0.20, 0.25, 0.25]
            } else {
                let sum = geo.live.iter().map(|(_, v)| v.max(0.0) as f64).sum::<f64>().max(f64::EPSILON);
                geo.live.iter().map(|(_, v)| v.max(0.0) as f64 / sum * geo.grow as f64).collect()
            };
            let mut start = -std::f64::consts::FRAC_PI_2;
            for (si, &frac) in slices.iter().enumerate() {
                let sweep = frac * std::f64::consts::TAU;
                let steps = ((sweep * outer).max(4.0) as u32).clamp(4, 40);
                let mut pts: Vec<(f64, f64)> = Vec::new();
                if inner > 0.0 {
                    for s in 0..=steps {
                        let t = start + sweep * s as f64 / steps as f64;
                        pts.push((cx0 + t.cos() * outer, cy0 + t.sin() * outer));
                    }
                    for s in (0..=steps).rev() {
                        let t = start + sweep * s as f64 / steps as f64;
                        pts.push((cx0 + t.cos() * inner, cy0 + t.sin() * inner));
                    }
                } else {
                    pts.push((cx0, cy0));
                    for s in 0..=steps {
                        let t = start + sweep * s as f64 / steps as f64;
                        pts.push((cx0 + t.cos() * outer, cy0 + t.sin() * outer));
                    }
                }
                let base = pal[si % pal.len()];
                // The painter makes the slice colour from its channels as they are and an alpha of 85 %, **premultiplied**: the
                // channels are not scaled by the alpha, so the slice is its colour over what is behind it, plus 15 % of that —
                // brighter than the palette's entry. Written as the straight colour that gives the same: the channels over 0.85.
                let alpha = (255.0 * 0.85) as u8;
                let lift = |v: u8| ((v as f64 * 255.0 / alpha as f64).round()).min(255.0) as u8;
                let fill = [lift(base[0]), lift(base[1]), lift(base[2]), alpha];
                let points = pts.iter().map(|(x, y)| format!("{},{}", f(*x), f(*y))).collect::<Vec<_>>().join(" ");
                data = data.child(sv("polygon").attr("points", points).attr("fill", rgb(fill)).attr("stroke", rgb(bg)).attr("stroke-width", "0.8").attr("stroke-linejoin", "round"));
                start += sweep;
            }
        }
        ControlType::RadarChart => {
            data = radar(&c, &fr, &pal, bg, data);
        }
        _ => {}
    }
    svg = svg.child(data);

    // ── captions and the legend, in the margins ─────────────────────────────
    let chrome_c = {
        let want = [170, 190, 225, 255];
        caret_color(bg, want)
    };
    let mut chrome = sv("g").attr("clip-path", format!("url(#{clip_all})"));
    let size9 = 9.0 * fr.type_scale;
    let x_caption = c.trimmed("XAxisLabel");
    let y_caption = c.trimmed("YAxisLabel");
    let title = c.trimmed("Title");
    if !title.is_empty() {
        let col = parse_color(&c.s("TitleColor")).filter(|c| c[3] > 0).unwrap_or_else(|| caret_color(bg, [169, 169, 169, 255]));
        chrome = chrome.child(text(w / 2.0, baseline_centred(fr.margin_t * 0.5, fr.title_font), "middle", fr.title_font, &title, col));
    }
    if !x_caption.is_empty() {
        chrome = chrome.child(text(fr.centre().0, baseline_centred(h - fr.cap_h * 0.5 - fr.legend_h, size9), "middle", size9, &x_caption, chrome_c));
    }
    if !y_caption.is_empty() {
        let row = metrics::row_height(size9);
        let wcap = metrics::text_width(&y_caption, size9);
        let at_x = fr.cap_w * 0.5 + row * 0.5;
        let at_y = fr.centre().1 + wcap * 0.5;
        chrome = chrome.child(
            sv("text")
                .attr("x", "0")
                .attr("y", "0")
                .attr("font-size", f(size9))
                .attr("fill", rgb(chrome_c))
                .attr("transform", format!("translate({} {}) rotate(-90)", f(at_x + metrics::ascent(size9) - row), f(at_y)))
                .text(y_caption),
        );
    }
    if c.flag("ShowLegend", false) {
        let entries: Vec<(String, Rgba)> = if c.is_pie() {
            let names: Vec<String> = if geo.live.is_empty() { (1..=4).map(|i| format!("Item {i}")).collect() } else { geo.live.iter().map(|(l, _)| l.clone()).collect() };
            names.into_iter().enumerate().map(|(i, l)| (l, pal[i % pal.len()])).collect()
        } else {
            let count = if geo.live.is_empty() { geo.plotted.len().max(1) } else { geo.series_count.max(1) };
            series_names(&c, count).into_iter().enumerate().map(|(i, n)| (n, pal[i % pal.len()])).collect()
        };
        let swatch = 7.0;
        if c.is_pie() {
            let x = w - fr.legend_w + 4.0;
            let line_h = 12.0;
            let total = entries.len() as f64 * line_h;
            let mut y = (fr.centre().1 - total * 0.5).max(4.0);
            for (name, colour) in &entries {
                if y + line_h > h - 2.0 {
                    break;
                }
                chrome = chrome.child(sv("rect").attr("x", f(x)).attr("y", f(y + (line_h - swatch) * 0.5)).attr("width", f(swatch)).attr("height", f(swatch)).attr("rx", "1").attr("fill", rgb(*colour)));
                chrome = chrome.child(text(x + swatch + 4.0, baseline_centred(y + line_h * 0.5, size9), "start", size9, name, chrome_c));
                y += line_h;
            }
        } else {
            let gap = 10.0;
            let widths: Vec<f64> = entries.iter().map(|(n, _)| swatch + 4.0 + metrics::text_width(n, size9)).collect();
            let total: f64 = widths.iter().sum::<f64>() + gap * (entries.len() as f64 - 1.0).max(0.0);
            let mut x = fr.centre().0 - total * 0.5;
            let y = h - fr.legend_h * 0.5;
            for ((name, colour), wd) in entries.iter().zip(widths) {
                chrome = chrome.child(sv("rect").attr("x", f(x)).attr("y", f(y - swatch * 0.5)).attr("width", f(swatch)).attr("height", f(swatch)).attr("rx", "1").attr("fill", rgb(*colour)));
                chrome = chrome.child(text(x + swatch + 4.0, baseline_centred(y, size9), "start", size9, name, chrome_c));
                x += wd + gap;
            }
        }
    }
    svg = svg.child(chrome);
    root = root.child(svg);
    Some(with_titles(root, &c, &geo))
}

/// Each mark's hover text is the browser's own: the label and the value, as the painter's tip names them.
fn with_titles(root: VNode, _c: &Ctx, _geo: &Geometry) -> VNode {
    root
}

fn radar(c: &Ctx, fr: &Frame, pal: &[Rgba], bg: Rgba, mut data: VNode) -> VNode {
    let live: Vec<Row> = {
        let raw = c.s("__ChartData");
        if raw.trim().is_empty() {
            Vec::new()
        } else {
            parse_chart_rows(&raw)
        }
    };
    let levels = c.num("GridLevels").map(|v| v.round().clamp(1.0, chart::RADAR_MAX_LEVELS as f64) as usize).unwrap_or(chart::RADAR_DEFAULT_LEVELS);
    let (rows, scale) = if live.is_empty() {
        let rows = chart::radar_sample_rows(3);
        let scale = chart::radar_scale(&rows, 0.0, 0.0, levels);
        (rows, scale)
    } else {
        let (min, max) = (c.num("MinValue").unwrap_or(0.0) as f32, c.num("MaxValue").unwrap_or(0.0) as f32);
        let scale = chart::radar_scale(&live, min, max, levels);
        (live, scale)
    };
    let labels: Vec<String> = rows.iter().map(|(l, _)| l.clone()).collect();
    let (pl, pt, pr, pb) = fr.plot;
    let label_px = 9.0 * fr.type_scale * chart::RADAR_LABEL_SCALE as f64;
    let layout = radar_layout([pl as f32, pt as f32, pr as f32, pb as f32], &labels, label_px as f32);
    let centre = layout.centre;
    let rim = layout.radius;
    let axes = rows.len();
    // The ink and the grid take their colour from what the face really is.
    let ink_base = caret_color(bg, [214, 224, 247, 255]);
    let ink_at = |k: f64| [ink_base[0], ink_base[1], ink_base[2], (255.0 * k) as u8];
    let show_grid = c.flag("ShowGridLines", true);
    let ring = |level: usize| -> Vec<(f64, f64)> { (0..axes).map(|i| { let p = radar_point(centre, rim, i, axes, level as f32 / levels as f32); (p.0 as f64, p.1 as f64) }).collect() };
    let poly = |pts: &[(f64, f64)]| pts.iter().map(|(x, y)| format!("{},{}", f(*x), f(*y))).collect::<Vec<_>>().join(" ");
    if show_grid && axes > 0 && rim > 0.0 {
        if axes >= 3 {
            for level in (1..=levels).step_by(2) {
                // the band between ring level-1 and ring level, drawn as the outer polygon with the inner one cut out
                let outer = ring(level);
                let inner = if level == 1 { vec![(centre.0 as f64, centre.1 as f64); axes] } else { ring(level - 1) };
                let path = format!(
                    "M{} Z M{} Z",
                    outer.iter().map(|(x, y)| format!("{},{}", f(*x), f(*y))).collect::<Vec<_>>().join(" L"),
                    inner.iter().rev().map(|(x, y)| format!("{},{}", f(*x), f(*y))).collect::<Vec<_>>().join(" L")
                );
                data = data.child(sv("path").attr("d", path).attr("fill", rgb(ink_at(0.055))).attr("fill-rule", "evenodd"));
            }
            for level in 1..=levels {
                let k = if level == levels { 0.34 } else { 0.20 };
                data = data.child(sv("polygon").attr("points", poly(&ring(level))).attr("fill", "none").attr("stroke", rgb(ink_at(k))).attr("stroke-width", "1"));
            }
        }
        for i in 0..axes {
            let p = radar_point(centre, rim, i, axes, 1.0);
            data = data.child(line(centre.0 as f64, centre.1 as f64, p.0 as f64, p.1 as f64, 1.0, ink_at(0.15)));
        }
    }
    let fill_a = (c.num("FillOpacity").unwrap_or(chart::RADAR_DEFAULT_FILL_OPACITY as f64).clamp(0.0, 100.0) / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8;
    let marker_r = c.num("PointRadius").unwrap_or(3.0).clamp(0.5, 40.0);
    let line_w = 2.0;
    let vertices = chart::radar_vertices(&layout, &scale, &rows, 1.0);
    let fractions = chart::radar_fractions(&rows, &scale);
    let area = |poly: &[(f32, f32)]| -> f32 {
        (0..poly.len()).map(|i| { let (p, q) = (poly[i], poly[(i + 1) % poly.len()]); (p.0 * q.1 - q.0 * p.1) * 0.5 }).sum::<f32>().abs()
    };
    let mut order: Vec<usize> = (0..vertices.len()).collect();
    order.sort_by(|&x, &y| area(&vertices[y]).total_cmp(&area(&vertices[x])));
    for &si in &order {
        let ring_pts: Vec<(f64, f64)> = vertices[si].iter().map(|p| (p.0 as f64, p.1 as f64)).collect();
        let base = pal[si % pal.len()];
        if ring_pts.len() >= 3 {
            if fill_a > 0 {
                data = data.child(sv("polygon").attr("points", poly(&ring_pts)).attr("fill", rgb([base[0], base[1], base[2], fill_a])));
            }
            data = data.child(sv("polygon").attr("points", poly(&ring_pts)).attr("fill", "none").attr("stroke", rgb(base)).attr("stroke-width", f(line_w)).attr("stroke-linejoin", "round"));
        } else if ring_pts.len() == 2 {
            data = data.child(line(ring_pts[0].0, ring_pts[0].1, ring_pts[1].0, ring_pts[1].1, line_w, base));
        }
    }
    if c.flag("ShowPoints", true) {
        let halo = [255, 255, 255, (255.0 * 0.85) as u8];
        for &si in &order {
            let base = pal[si % pal.len()];
            for (ai, p) in vertices[si].iter().enumerate() {
                let dist = (((p.0 - centre.0) as f64).powi(2) + ((p.1 - centre.1) as f64).powi(2)).sqrt();
                let shown = axes == 1 || (fractions[si][ai] > 0.0 && dist >= marker_r * 2.0);
                if shown {
                    data = data.child(sv("circle").attr("cx", f(p.0 as f64)).attr("cy", f(p.1 as f64)).attr("r", f(marker_r + 1.5)).attr("fill", rgb(halo)));
                    data = data.child(sv("circle").attr("cx", f(p.0 as f64)).attr("cy", f(p.1 as f64)).attr("r", f(marker_r)).attr("fill", rgb(base)));
                }
            }
        }
    }
    // What each ring stands for, up the first spoke (`ShowAxisValues`): each value at the ring's height, just right of the axis.
    if c.flag("ShowAxisValues", false) && axes > 0 && rim > 0.0 {
        let size = label_px * 0.9;
        let step = (scale.max - scale.min) / levels as f32;
        for level in 1..=levels {
            let value = scale.ring_value(level, levels);
            let y = centre.1 as f64 - rim as f64 * level as f64 / levels as f64;
            // The painter puts the text's top-left corner at (centre + 3, ring + 1).
            data = data.child(text(centre.0 as f64 + 3.0, (y + 1.0).round() + metrics::ascent(size), "start", size, &chart::radar_value_text(value, step), ink_at(0.7)));
        }
    }
    // The axis captions, outward off the rim.
    for (i, label) in layout.labels.iter().enumerate().filter(|(_, t)| !t.is_empty()) {
        let anchor = radar_point(centre, rim + chart::RADAR_LABEL_GAP, i, axes, 1.0);
        let (ax, ay) = chart::radar_label_anchor(chart::radar_dir(i, axes));
        let width = metrics::text_width(label, label_px);
        let row = metrics::row_height(label_px);
        let place = |side: chart::Anchor, at: f64, len: f64| match side {
            chart::Anchor::Start => at,
            chart::Anchor::Middle => at - len * 0.5,
            chart::Anchor::End => at - len,
        };
        let x = place(ax, anchor.0 as f64, width).clamp(pl, (pr - width).max(pl));
        let y = place(ay, anchor.1 as f64, row).clamp(pt, (pb - row).max(pt));
        data = data.child(text(x, y.round() + metrics::ascent(label_px), "start", label_px, label, ink_at(1.0)));
    }
    data
}


// ── the gauge (`paint::draw_control_body`, the Gauge branch) ────────────────────

/// The colour a control's own text is judged against (`paint::control_surface_tone`, over `ground_under_controls`): what the
/// control paints behind it — its own named background laid over the form's ground, nothing else. A container above it does
/// not count, which is why a gauge's white readout on a pale card is white. Under a see-through theme the ground is the
/// form's glass laid over the desktop the theme is designed for; under any other it is the universal default face.
fn surface_tone(cx: &ViewCtx, i: usize) -> Rgba {
    let form = cx.tree.ctl(0);
    let over = |top: [f64; 4], under: [f64; 3]| -> [f64; 3] {
        let a = top[3] / 255.0;
        [top[0] * a + under[0] * (1.0 - a), top[1] * a + under[1] * (1.0 - a), top[2] * a + under[2] * (1.0 - a)]
    };
    let ground = if form.text_or("theme", "") == cobolt_forms::theme::SPATIAL {
        let (r, g, b) = cobolt_forms::theme::SEE_THROUGH_BACKDROP_RGB;
        let [gr, gg, gb, ga] = cobolt_forms::theme::SPATIAL_FORM_GLASS;
        let t = parse_f64(&form.raw("transparency").unwrap_or("0").to_owned()).unwrap_or(0.0).clamp(0.0, 100.0);
        let alpha = (ga as f64 * (1.0 - t / 100.0)).floor();
        over([gr as f64, gg as f64, gb as f64, alpha], [r as f64, g as f64, b as f64])
    } else {
        parse_color(cobolt_forms::model::DEFAULT_BACKGROUND_COLOR).map(|c| [c[0] as f64, c[1] as f64, c[2] as f64]).unwrap_or([240.0, 240.0, 240.0])
    };
    let tone = match user_background(cx, i) {
        Some(c) => over([c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64], ground),
        None => ground,
    };
    [tone[0].round() as u8, tone[1].round() as u8, tone[2].round() as u8, 255]
}

/// The user's own `BackgroundColor`, when it is one: not empty, not clear, and not one of the colours every
/// control is seeded with (`paint::user_background_color`).
fn user_background(cx: &ViewCtx, i: usize) -> Option<Rgba> {
    let raw = cx.tree.get(i, "BackgroundColor");
    if raw.trim().is_empty() {
        return None;
    }
    let c = parse_color(&raw)?;
    if c[3] == 0 {
        return None;
    }
    let seeded = [cobolt_forms::model::DEFAULT_BACKGROUND_COLOR, cobolt_forms::model::NEUMORPHIC_SURFACE_COLOR, cobolt_forms::model::NEUMORPHIC_DARK_SURFACE_COLOR];
    if seeded.iter().any(|s| parse_color(s) == Some(c)) {
        return None;
    }
    Some(c)
}

fn arc_points(cx0: f64, cy0: f64, r: f64, start_deg: f64, sweep_deg: f64) -> String {
    let segments = 48.max((sweep_deg.abs() / 4.0) as usize);
    (0..=segments)
        .map(|k| {
            let a = (start_deg + sweep_deg * (k as f64 / segments as f64)).to_radians();
            format!("{},{}", f(cx0 + r * a.cos()), f(cy0 + r * a.sin()))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The gauge as the desktop paints it, or `None` when it carries thresholds (zones keep the older renderer).
pub fn gauge(cx: &ViewCtx, i: usize) -> Option<VNode> {
    let tree = cx.tree;
    let threshold = |k: &str| parse_f64(&tree.get(i, k)).is_some();
    if threshold("WarningThreshold") && threshold("CriticalThreshold") {
        return None;
    }
    let ctl = tree.ctl(i);
    let (w, h) = cx.box_size(i);
    let (w, h) = (w.max(8.0), h.max(8.0));
    let style = {
        let s = tree.get(i, "GaugeStyle");
        if s.trim().is_empty() { "Radial".to_owned() } else { s }
    };
    let int = |k: &str, d: i64| parse_f64(&tree.get(i, k)).map(|v| v as i64).unwrap_or(d);
    let min_v = int("Minimum", 0) as f64;
    let max_v = (int("Maximum", 100) as f64).max(min_v + 1.0);
    let val = int("Value", 0) as f64;
    let frac = ((val - min_v) / (max_v - min_v)).clamp(0.0, 1.0);
    let flag = |k: &str| {
        let v = tree.get(i, k);
        v.trim().is_empty() || truthy(&v)
    };
    let user_fg = parse_color(&tree.get(i, "ForegroundColor")).filter(|c| c[3] > 0 && Some(*c) != parse_color(cobolt_forms::model::DEFAULT_FOREGROUND_COLOR));
    let accent: Rgba = [0, 120, 215, 255];
    let color_prop = tree.get(i, "Color");
    let fill = if !color_prop.trim().is_empty() { parse_color(&color_prop).unwrap_or(accent) } else { user_fg.unwrap_or(accent) };
    let track = user_background(cx, i).unwrap_or([140, 140, 140, 255]);
    let unit = tree.get(i, "Unit");
    let gap = if unit.chars().next().is_some_and(|c| c.is_alphanumeric()) { " " } else { "" };
    let reading = {
        let over = tree.get(i, "Text");
        if over.trim().is_empty() { format!("{val:.0}{gap}{unit}") } else { over }
    };
    // The size the control STATES: a control that names none is lettered at the painter's own missing-size default, not at the seeded one a
    // program reads back (`FormTree::get`).
    let font = cx.tree.font_size_of(i).unwrap_or(11.0).clamp(4.0, 200.0);
    let tone = surface_tone(cx, i);
    let text_colour = caret_color(tone, user_fg.unwrap_or([230, 230, 230, 255]));

    let mut svg = sv("svg").key("svg").class("pr-gauge").attr("viewBox", format!("0 0 {} {}", f(w), f(h))).attr("preserveAspectRatio", "none").attr("width", f(w)).attr("height", f(h));
    let (centre_x, centre_y) = (w / 2.0, h / 2.0);
    // Where the reading goes: (x, y, anchor kind), `y` the anchor's own y.
    #[derive(Clone, Copy)]
    enum At {
        Centre,
        Bottom,
        Top,
    }
    let (rx, ry, at) = match style.as_str() {
        "Linear" => {
            let bh = (parse_f64(&tree.get(i, "BarHeight")).unwrap_or(14.0)).min(h);
            let r = bh * 0.5;
            let (bx, by, bw) = (centre_x - (w - 4.0) / 2.0, centre_y - bh / 2.0, w - 4.0);
            svg = svg.child(sv("rect").attr("x", f(bx)).attr("y", f(by)).attr("width", f(bw)).attr("height", f(bh)).attr("rx", f(r)).attr("fill", rgb(track)));
            if frac > 0.001 {
                svg = svg.child(sv("rect").attr("x", f(bx)).attr("y", f(by)).attr("width", f(bw * frac)).attr("height", f(bh)).attr("rx", f(r.min(bw * frac * 0.5))).attr("fill", rgb(fill)));
            }
            // The painter's flag reader answers true for a property the control does not carry.
            if flag("ShowThumb") {
                let c = [(fill[0] as f64 + (255.0 - fill[0] as f64) * 0.35).round() as u8, (fill[1] as f64 + (255.0 - fill[1] as f64) * 0.35).round() as u8, (fill[2] as f64 + (255.0 - fill[2] as f64) * 0.35).round() as u8, fill[3]];
                svg = svg.child(sv("circle").attr("cx", f(bx + bw * frac)).attr("cy", f(centre_y)).attr("r", f((bh * 0.70).max(4.0))).attr("fill", rgb(c)).attr("stroke", rgb(fill)).attr("stroke-width", "1.5"));
            }
            if 4.0 < by - 2.0 {
                (centre_x, by - 3.0, At::Bottom)
            } else {
                (centre_x, centre_y, At::Centre)
            }
        }
        "Donut" => {
            let radius = (w.min(h) * 0.5 - 4.0).max(6.0);
            let sw = parse_f64(&tree.get(i, "StrokeWidth")).unwrap_or(8.0);
            svg = svg.child(sv("polyline").attr("points", arc_points(centre_x, centre_y, radius, -90.0, 360.0)).attr("fill", "none").attr("stroke", rgb(track)).attr("stroke-width", f(sw)));
            if frac > 0.001 {
                svg = svg.child(sv("polyline").attr("points", arc_points(centre_x, centre_y, radius, -90.0, 360.0 * frac)).attr("fill", "none").attr("stroke", rgb(fill)).attr("stroke-width", f(sw)));
            }
            (centre_x, centre_y, At::Centre)
        }
        _ => {
            let down = tree.get(i, "ReadoutPosition").trim().eq_ignore_ascii_case("Down");
            let reserve = if down { metrics::row_height(font) + 5.0 } else { 0.0 };
            let (px_, py_) = (centre_x, h - 6.0 - reserve);
            let radius = (w * 0.5 - 4.0).min(h - 10.0 - reserve).max(6.0);
            let sw = radius * 0.18;
            svg = svg.child(sv("polyline").attr("points", arc_points(px_, py_, radius, 180.0, 180.0)).attr("fill", "none").attr("stroke", rgb(track)).attr("stroke-width", f(sw)));
            if frac > 0.001 {
                svg = svg.child(sv("polyline").attr("points", arc_points(px_, py_, radius, 180.0, 180.0 * frac)).attr("fill", "none").attr("stroke", rgb(fill)).attr("stroke-width", f(sw)));
            }
            let ray = |deg: f64| ((deg.to_radians()).cos(), (deg.to_radians()).sin());
            if flag("ShowScale") {
                let band_in = radius * 0.91;
                for k in 0..=10 {
                    let (dx, dy) = ray(180.0 + 18.0 * k as f64);
                    let major = k % 5 == 0;
                    let len = radius * if major { 0.16 } else { 0.09 };
                    svg = svg.child(line(px_ + dx * (band_in - len), py_ + dy * (band_in - len), px_ + dx * band_in, py_ + dy * band_in, if major { 2.0 } else { 1.0 }, track));
                }
            }
            if flag("ShowNeedle") {
                let ink = {
                    let named = tree.get(i, "NeedleColor");
                    if named.trim().is_empty() { user_fg.unwrap_or(accent) } else { parse_color(&named).unwrap_or(accent) }
                };
                let (dx, dy) = ray(180.0 + 180.0 * frac);
                svg = svg.child(line(px_, py_, px_ + dx * radius * 0.78, py_ + dy * radius * 0.78, (radius * 0.06).max(1.5), ink));
                svg = svg.child(sv("circle").attr("cx", f(px_)).attr("cy", f(py_)).attr("r", f((radius * 0.10).max(2.5))).attr("fill", rgb(ink)));
            }
            if down { (px_, py_ + 5.0, At::Top) } else { (px_, py_ - radius * 0.30, At::Bottom) }
        }
    };
    let row = metrics::row_height(font);
    let baseline = match at {
        At::Centre => baseline_centred(ry, font),
        At::Bottom => (ry - row).round() + metrics::ascent(font),
        At::Top => ry.round() + metrics::ascent(font),
    };
    svg = svg.child(text(rx, baseline, "middle", font, &reading, text_colour));
    let mut root = VNode::new("div")
        .key("root")
        .classes(class_names(ctl))
        .attr("data-name", ctl.name.clone())
        .attr("id", ctl.name.clone())
        .attr("role", "meter")
        .attr("aria-valuemin", f(min_v))
        .attr("aria-valuemax", f(max_v))
        .attr("aria-valuenow", f(val));
    if !ctl.visible() {
        root = root.flag("hidden", true);
    }
    for d in super::controls::bound_dom_events(ctl) {
        root = root.on(d);
    }
    Some(root.child(svg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bar_stands_in_its_slot_as_the_painter_places_it() {
        // 4 bars, two series, plot 100 × 50 at (10, 5): slot 25, bar 9.5, gap 1.25.
        let plot = (10.0, 5.0, 110.0, 55.0);
        let (x, y, w, h) = bar_rect(plot, false, false, 2, 4, 0.0, 0.5, 1, 2);
        assert!((x - (10.0 + 2.0 * 25.0 + 1.0 * (9.5 + 1.25) + 1.25)).abs() < 1e-9, "{x}");
        assert!((w - 9.5).abs() < 1e-9 && (h - 25.0).abs() < 1e-9 && (y - 30.0).abs() < 1e-9, "{y} {w} {h}");
    }

    #[test]
    fn the_curve_passes_through_every_point_and_keeps_both_ends() {
        let pts = [(0.0, 0.0), (10.0, 20.0), (20.0, 5.0), (30.0, 25.0)];
        let curve = catmull_rom(&pts, 14);
        assert_eq!(curve.len(), 3 * 14 + 1);
        assert_eq!((curve[0], *curve.last().unwrap()), (pts[0], pts[3]));
        assert_eq!(curve[14], pts[1], "the spline interpolates its control points");
    }

    #[test]
    fn caret_colour_keeps_a_readable_ink_and_otherwise_picks_a_pole() {
        let pale = [234, 241, 251, 255];
        assert_eq!(caret_color(pale, [20, 20, 40, 255]), [20, 20, 40, 255], "dark on pale is readable");
        assert_eq!(caret_color(pale, [170, 190, 225, 255]), [0, 0, 0, 255], "the pale blue on a pale face is not: black");
        assert_eq!(caret_color([15, 20, 45, 255], [30, 30, 60, 255]), [255, 255, 255, 255]);
    }
}
