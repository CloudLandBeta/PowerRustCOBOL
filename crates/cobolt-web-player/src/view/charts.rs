// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The class B controls drawn with SVG** (WASM-004 R4): the seven charts, Knob, Gauge and Shape.
//!
//! The geometry is plain arithmetic on numbers the control's properties and its `__ChartData` carry, so
//! it is checked by native tests (a bar's height, an arc's end point) rather than by looking at a
//! picture. Colour comes from the properties through presentation attributes; text takes the
//! control's `color` (so `ForegroundColor` and the developer's CSS reach it).
//!
//! **Where the picture differs from the desktop** (listed, per WASM-004 R4): the value animation
//! (`AnimateOnLoad`, `AnimateValues`) is a CSS transition on the marks, not the painter's tween; the
//! glass/neumorphic faces of the egui painter are the CSS box of `style.rs`; a chart's hover tooltip is
//! the browser's own (`<title>` of each mark).

use cobolt_forms::chart::{nice_step, parse_chart_rows, value_range, Row};

use super::{VNode, ViewCtx};
use crate::css::class_names;
use crate::value::{fmt_number, parse_color, parse_f64, truthy};
use cobolt_forms::model::ControlType;

fn f(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let r = (v * 100.0).round() / 100.0;
    fmt_number(r)
}

fn sv(name: &'static str) -> VNode {
    VNode::svg(name)
}

fn color_attr(c: &str) -> Option<String> {
    parse_color(c).map(|rgba| {
        if rgba[3] == 255 {
            format!("#{:02x}{:02x}{:02x}", rgba[0], rgba[1], rgba[2])
        } else {
            format!("rgb({} {} {} / {})", rgba[0], rgba[1], rgba[2], fmt_number((rgba[3] as f64 / 255.0 * 1000.0).round() / 1000.0))
        }
    })
}

fn size(cx: &ViewCtx, i: usize) -> (f64, f64) {
    let w = parse_f64(&cx.tree.get(i, "Width")).unwrap_or(300.0).max(16.0);
    let h = parse_f64(&cx.tree.get(i, "Height")).unwrap_or(200.0).max(16.0);
    (w, h)
}

fn svg_root(cx: &ViewCtx, i: usize, class: &'static str) -> (VNode, f64, f64) {
    let (w, h) = size(cx, i);
    let c = cx.tree.ctl(i);
    let n = VNode::new("div")
        .key("root")
        .classes(class_names(c))
        .attr("data-name", c.name.clone())
        .attr("id", c.name.clone())
        .attr("role", "img");
    let mut n = n;
    if !c.visible() {
        n = n.flag("hidden", true);
    }
    for d in super::controls::bound_dom_events(c) {
        n = n.on(d);
    }
    let svg = sv("svg").key("svg").class(class).attr("viewBox", format!("0 0 {} {}", f(w), f(h))).attr("preserveAspectRatio", "xMidYMid meet");
    (n.child(svg), w, h)
}

fn push_svg(root: &mut VNode, child: VNode) {
    root.children[0].children.push(child);
}

fn with_title(n: VNode, title: String) -> VNode {
    n.child(sv("title").text(title))
}

/// A series' colour: from `SeriesColors`, cycling; in monochrome, tonal steps of one colour.
pub fn series_color(cx: &ViewCtx, i: usize, series: usize, of: usize) -> String {
    if truthy(&cx.tree.get(i, "Monochrome")) {
        let base = parse_color(&cx.tree.get(i, "MonochromeColor")).unwrap_or([0x3F, 0x6F, 0xB5, 255]);
        let k = if of <= 1 { 0.0 } else { series as f64 / (of - 1) as f64 };
        // From 25 % lighter to 25 % darker across the series.
        let shade = |c: u8| -> u8 {
            let c = c as f64;
            let v = if k < 0.5 { c + (255.0 - c) * 0.25 * (1.0 - k * 2.0) } else { c * (1.0 - 0.25 * (k * 2.0 - 1.0)) };
            v.round().clamp(0.0, 255.0) as u8
        };
        return format!("#{:02x}{:02x}{:02x}", shade(base[0]), shade(base[1]), shade(base[2]));
    }
    let list: Vec<String> = cx.tree.get(i, "SeriesColors").split(',').map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect();
    let pick = if list.is_empty() { "#4C9BE8".to_owned() } else { list[series % list.len()].clone() };
    color_attr(&pick).unwrap_or(pick)
}

fn series_labels(cx: &ViewCtx, i: usize, n: usize) -> Vec<String> {
    let given: Vec<String> = cx.tree.get(i, "SeriesLabels").split(',').map(|s| s.trim().to_owned()).collect();
    (0..n).map(|k| given.get(k).filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| format!("Series {}", k + 1))).collect()
}

fn text_node(x: f64, y: f64, anchor: &str, size_px: f64, s: &str) -> VNode {
    sv("text").attr("x", f(x)).attr("y", f(y)).attr("text-anchor", anchor).attr("font-size", f(size_px)).attr("fill", "currentColor").text(s.to_owned())
}

/// The "nice" tick values spanning `[lo, hi]` and the range they actually cover.
pub fn ticks(lo: f32, hi: f32, want: usize) -> (Vec<f32>, f32, f32) {
    let span = (hi - lo).max(f32::EPSILON);
    let step = nice_step(span / want.max(1) as f32).max(f32::EPSILON);
    let first = (lo / step).floor() * step;
    let mut v = Vec::new();
    let mut t = first;
    let mut guard = 0;
    while t <= hi + step * 0.999 && guard < 200 {
        v.push(t);
        t += step;
        guard += 1;
    }
    let (a, b) = (v.first().copied().unwrap_or(lo), v.last().copied().unwrap_or(hi));
    (v, a, b)
}

fn fmt_tick(v: f32) -> String {
    let r = (v as f64 * 1000.0).round() / 1000.0;
    fmt_number(r)
}

/// The plot rectangle `(left, top, right, bottom)` inside a chart `w × h`, leaving room for the
/// title, the axis captions, the tick labels and the legend.
fn plot_rect(cx: &ViewCtx, i: usize, w: f64, h: f64, legend_rows: usize, axes: bool) -> (f64, f64, f64, f64) {
    let title = !cx.tree.get(i, "Title").is_empty();
    let top = if title { 28.0 } else { 10.0 };
    let left = if axes { 40.0 + if cx.tree.get(i, "YAxisLabel").is_empty() { 0.0 } else { 14.0 } } else { 10.0 };
    let bottom = h - (if axes { 22.0 } else { 8.0 }) - (if cx.tree.get(i, "XAxisLabel").is_empty() { 0.0 } else { 14.0 }) - legend_rows as f64 * 16.0;
    (left, top, w - 10.0, bottom.max(top + 10.0))
}

fn title_and_legend(cx: &ViewCtx, i: usize, root: &mut VNode, w: f64, h: f64, labels: &[String], colors: &[String]) {
    let title = cx.tree.get(i, "Title");
    if !title.is_empty() {
        let tfs = parse_f64(&cx.tree.get(i, "TitleFontSize")).filter(|v| *v > 0.0).unwrap_or(14.0);
        let mut t = text_node(w / 2.0, 6.0 + tfs, "middle", tfs, &title).attr("font-weight", "600").class("pr-chart-title");
        if let Some(c) = color_attr(&cx.tree.get(i, "TitleColor")) {
            t = t.attr("fill", c);
        }
        push_svg(root, t);
    }
    if truthy(&cx.tree.get(i, "ShowLegend")) && !labels.is_empty() {
        let mut x = 12.0;
        let y = h - 6.0;
        for (k, l) in labels.iter().enumerate() {
            push_svg(root, sv("rect").attr("x", f(x)).attr("y", f(y - 9.0)).attr("width", "10").attr("height", "10").attr("rx", "2").attr("fill", colors[k % colors.len()].clone()));
            push_svg(root, text_node(x + 14.0, y, "start", 11.0, l).class("pr-legend"));
            x += 14.0 + l.chars().count() as f64 * 6.2 + 14.0;
            if x > w {
                break;
            }
        }
    }
}

fn axis_captions(cx: &ViewCtx, i: usize, root: &mut VNode, plot: (f64, f64, f64, f64), h: f64, legend_rows: usize) {
    let (l, t, r, b) = plot;
    let x = cx.tree.get(i, "XAxisLabel");
    let y = cx.tree.get(i, "YAxisLabel");
    if !x.is_empty() {
        push_svg(root, text_node((l + r) / 2.0, h - 6.0 - legend_rows as f64 * 16.0, "middle", 11.0, &x));
    }
    if !y.is_empty() {
        let cy = (t + b) / 2.0;
        push_svg(root, text_node(0.0, 0.0, "middle", 11.0, &y).attr("transform", format!("translate(12 {}) rotate(-90)", f(cy))));
    }
}

fn gridlines(root: &mut VNode, plot: (f64, f64, f64, f64), vals: &[f32], map: &dyn Fn(f32) -> f64, horizontal: bool, show_grid: bool, show_axis: bool, labels: bool) {
    let (l, t, r, b) = plot;
    for v in vals {
        let p = map(*v);
        if horizontal {
            if show_grid {
                push_svg(root, sv("line").attr("x1", f(l)).attr("x2", f(r)).attr("y1", f(p)).attr("y2", f(p)).attr("stroke", "currentColor").attr("stroke-opacity", "0.18"));
            }
            if labels && show_axis {
                push_svg(root, text_node(l - 5.0, p + 4.0, "end", 10.0, &fmt_tick(*v)).class("pr-tick"));
            }
        } else {
            if show_grid {
                push_svg(root, sv("line").attr("y1", f(t)).attr("y2", f(b)).attr("x1", f(p)).attr("x2", f(p)).attr("stroke", "currentColor").attr("stroke-opacity", "0.18"));
            }
            if labels && show_axis {
                push_svg(root, text_node(p, b + 14.0, "middle", 10.0, &fmt_tick(*v)).class("pr-tick"));
            }
        }
    }
    if show_axis {
        push_svg(root, sv("line").attr("x1", f(l)).attr("x2", f(r)).attr("y1", f(b)).attr("y2", f(b)).attr("stroke", "currentColor").attr("stroke-opacity", "0.5"));
        push_svg(root, sv("line").attr("x1", f(l)).attr("x2", f(l)).attr("y1", f(t)).attr("y2", f(b)).attr("stroke", "currentColor").attr("stroke-opacity", "0.5"));
    }
}

// ── The charts ──────────────────────────────────────────────────────────────

pub fn chart(cx: &ViewCtx, i: usize) -> VNode {
    let c = cx.tree.ctl(i);
    let Some(t) = c.control_type() else { return super::controls::hidden_object(cx, i) };
    let rows = parse_chart_rows(&cx.tree.get(i, "__ChartData"));
    let series = rows.iter().map(|(_, v)| v.len()).max().unwrap_or(1).max(1);
    let labels = series_labels(cx, i, series);
    let colors: Vec<String> = (0..series).map(|k| series_color(cx, i, k, series)).collect();
    let (mut root, w, h) = svg_root(cx, i, "pr-chart");
    let sum: f64 = rows.iter().map(|(_, v)| v.iter().map(|x| *x as f64).sum::<f64>()).sum();
    root = root.attr("aria-label", format!("{}: {} rows", cx.tree.get(i, "Title"), rows.len())).attr("data-rows", rows.len().to_string()).attr("data-sum", f(sum));
    let legend = truthy(&cx.tree.get(i, "ShowLegend"));
    let legend_rows = usize::from(legend);
    match t {
        ControlType::BarChart => bars(cx, i, &mut root, &rows, w, h, legend_rows, &colors),
        ControlType::LineChart | ControlType::AreaChart => lines(cx, i, &mut root, &rows, w, h, legend_rows, &colors, matches!(t, ControlType::AreaChart)),
        ControlType::ScatterChart => scatter(cx, i, &mut root, &rows, w, h, legend_rows, &colors),
        ControlType::PieChart | ControlType::DonutChart => pie(cx, i, &mut root, &rows, w, h, matches!(t, ControlType::DonutChart), &colors),
        ControlType::RadarChart => radar(cx, i, &mut root, &rows, w, h, &colors),
        _ => {}
    }
    let pie_like = matches!(t, ControlType::PieChart | ControlType::DonutChart);
    let shown_labels: Vec<String> = if pie_like { rows.iter().map(|(l, _)| l.clone()).collect() } else { labels };
    let shown_colors: Vec<String> = if pie_like { (0..rows.len()).map(|k| series_color(cx, i, k, rows.len().max(1))).collect() } else { colors.clone() };
    if !shown_colors.is_empty() {
        title_and_legend(cx, i, &mut root, w, h, &shown_labels, &shown_colors);
    } else {
        title_and_legend(cx, i, &mut root, w, h, &[], &["#888".to_owned()]);
    }
    root
}

/// The geometry of one bar: `(x, y, width, height)` for a value in a plot whose zero line is at `zero_y`.
pub fn bar_rect(x: f64, bar_w: f64, value: f32, scale: &dyn Fn(f32) -> f64, zero_y: f64) -> (f64, f64, f64, f64) {
    let y = scale(value);
    let (top, height) = if y <= zero_y { (y, zero_y - y) } else { (zero_y, y - zero_y) };
    (x, top, bar_w, height)
}

#[allow(clippy::too_many_arguments)]
fn bars(cx: &ViewCtx, i: usize, root: &mut VNode, rows: &[Row], w: f64, h: f64, legend_rows: usize, colors: &[String]) {
    let horizontal = truthy(&cx.tree.get(i, "Horizontal"));
    let stacked = truthy(&cx.tree.get(i, "Stacked"));
    let radius = parse_f64(&cx.tree.get(i, "BarCornerRadius")).unwrap_or(3.0).max(0.0);
    let plot = plot_rect(cx, i, w, h, legend_rows, true);
    let (l, t, r, b) = plot;
    let (lo, hi) = value_range(rows, stacked);
    let (tk, tlo, thi) = ticks(lo.min(0.0), hi.max(f32::EPSILON), 5);
    let n = rows.len().max(1);
    let series = rows.iter().map(|(_, v)| v.len()).max().unwrap_or(1).max(1);
    let (show_grid, show_x, show_y) = (truthy(&cx.tree.get(i, "ShowGridLines")), truthy(&cx.tree.get(i, "ShowXAxis")), truthy(&cx.tree.get(i, "ShowYAxis")));
    let tooltips = truthy(&cx.tree.get(i, "ShowTooltips"));
    if horizontal {
        let map = |v: f32| l + (v - tlo) as f64 / (thi - tlo).max(f32::EPSILON) as f64 * (r - l);
        gridlines(root, plot, &tk, &map, false, show_grid, show_x || show_y, true);
        let band = (b - t) / n as f64;
        let zero_x = map(0.0);
        for (k, (label, vals)) in rows.iter().enumerate() {
            let y0 = t + band * k as f64;
            push_svg(root, text_node(l - 5.0, y0 + band / 2.0 + 4.0, "end", 10.0, label).class("pr-tick"));
            let bh = if stacked { band * 0.7 } else { band * 0.7 / series as f64 };
            let mut acc = 0.0f32;
            for (s, v) in vals.iter().enumerate() {
                let (x1, x2) = if stacked { (map(acc), map(acc + v.max(0.0))) } else { (zero_x.min(map(*v)), zero_x.max(map(*v))) };
                if stacked {
                    acc += v.max(0.0);
                }
                let y = if stacked { y0 + band * 0.15 } else { y0 + band * 0.15 + bh * s as f64 };
                let mut rect = sv("rect").attr("x", f(x1)).attr("y", f(y)).attr("width", f((x2 - x1).max(0.0))).attr("height", f(bh)).attr("rx", f(radius)).attr("fill", colors[s % colors.len()].clone()).class("pr-bar");
                if tooltips {
                    rect = with_title(rect, format!("{label}: {}", fmt_tick(*v)));
                }
                push_svg(root, rect);
            }
        }
    } else {
        let map = |v: f32| b - (v - tlo) as f64 / (thi - tlo).max(f32::EPSILON) as f64 * (b - t);
        gridlines(root, plot, &tk, &map, true, show_grid, show_x || show_y, true);
        let band = (r - l) / n as f64;
        let zero_y = map(0.0);
        for (k, (label, vals)) in rows.iter().enumerate() {
            let x0 = l + band * k as f64;
            push_svg(root, text_node(x0 + band / 2.0, b + 14.0, "middle", 10.0, label).class("pr-tick"));
            let bw = if stacked { band * 0.7 } else { band * 0.7 / series as f64 };
            let mut acc = 0.0f32;
            for (s, v) in vals.iter().enumerate() {
                let x = if stacked { x0 + band * 0.15 } else { x0 + band * 0.15 + bw * s as f64 };
                let (rx, ry, rw, rh) = if stacked {
                    let y_top = map(acc + v.max(0.0));
                    let y_bot = map(acc);
                    acc += v.max(0.0);
                    (x, y_top, bw, (y_bot - y_top).max(0.0))
                } else {
                    bar_rect(x, bw, *v, &map, zero_y)
                };
                let mut rect = sv("rect").attr("x", f(rx)).attr("y", f(ry)).attr("width", f(rw)).attr("height", f(rh)).attr("rx", f(radius)).attr("fill", colors[s % colors.len()].clone()).class("pr-bar");
                if tooltips {
                    rect = with_title(rect, format!("{label}: {}", fmt_tick(*v)));
                }
                push_svg(root, rect);
            }
        }
    }
    axis_captions(cx, i, root, plot, h, legend_rows);
}

/// A smooth path through `pts` (Catmull-Rom converted to cubic Béziers).
pub fn smooth_path(pts: &[(f64, f64)]) -> String {
    if pts.is_empty() {
        return String::new();
    }
    let mut d = format!("M{} {}", f(pts[0].0), f(pts[0].1));
    if pts.len() == 1 {
        return d;
    }
    for k in 0..pts.len() - 1 {
        let p0 = if k == 0 { pts[0] } else { pts[k - 1] };
        let (p1, p2) = (pts[k], pts[k + 1]);
        let p3 = if k + 2 < pts.len() { pts[k + 2] } else { p2 };
        let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
        let c2 = (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0);
        d.push_str(&format!(" C{} {} {} {} {} {}", f(c1.0), f(c1.1), f(c2.0), f(c2.1), f(p2.0), f(p2.1)));
    }
    d
}

#[allow(clippy::too_many_arguments)]
fn lines(cx: &ViewCtx, i: usize, root: &mut VNode, rows: &[Row], w: f64, h: f64, legend_rows: usize, colors: &[String], area: bool) {
    let plot = plot_rect(cx, i, w, h, legend_rows, true);
    let (l, t, r, b) = plot;
    let stacked = area && truthy(&cx.tree.get(i, "Stacked"));
    let (lo, hi) = value_range(rows, stacked);
    let (tk, tlo, thi) = ticks(lo.min(0.0), hi.max(f32::EPSILON), 5);
    let map = |v: f32| b - (v - tlo) as f64 / (thi - tlo).max(f32::EPSILON) as f64 * (b - t);
    let (show_grid, show_x, show_y) = (truthy(&cx.tree.get(i, "ShowGridLines")), truthy(&cx.tree.get(i, "ShowXAxis")), truthy(&cx.tree.get(i, "ShowYAxis")));
    gridlines(root, plot, &tk, &map, true, show_grid, show_x || show_y, true);
    let n = rows.len();
    let xs: Vec<f64> = (0..n).map(|k| if n <= 1 { (l + r) / 2.0 } else { l + (r - l) * k as f64 / (n - 1) as f64 }).collect();
    for (k, (label, _)) in rows.iter().enumerate() {
        if n <= 12 || k % (n / 10 + 1) == 0 {
            push_svg(root, text_node(xs[k], b + 14.0, "middle", 10.0, label).class("pr-tick"));
        }
    }
    let series = rows.iter().map(|(_, v)| v.len()).max().unwrap_or(1).max(1);
    let smooth = truthy(&cx.tree.get(i, "Smooth"));
    let show_points = truthy(&cx.tree.get(i, "ShowPoints"));
    let pr = parse_f64(&cx.tree.get(i, "PointRadius")).unwrap_or(4.0).max(1.0);
    let fill_alpha = (parse_f64(&cx.tree.get(i, "FillAlpha")).unwrap_or(40.0) / 100.0).clamp(0.0, 1.0);
    let tooltips = truthy(&cx.tree.get(i, "ShowTooltips"));
    let mut base: Vec<f32> = vec![0.0; n];
    for s in 0..series {
        let vals: Vec<f32> = rows.iter().map(|(_, v)| v.get(s).copied().unwrap_or(0.0)).collect();
        let tops: Vec<f32> = if stacked { vals.iter().zip(&base).map(|(v, b)| v + b).collect() } else { vals.clone() };
        let pts: Vec<(f64, f64)> = xs.iter().zip(&tops).map(|(x, v)| (*x, map(*v))).collect();
        let d = if smooth && pts.len() > 2 { smooth_path(&pts) } else { pts.iter().enumerate().map(|(k, p)| format!("{}{} {}", if k == 0 { "M" } else { "L" }, f(p.0), f(p.1))).collect::<Vec<_>>().join(" ") };
        let color = colors[s % colors.len()].clone();
        if area && !pts.is_empty() {
            let zero_line = if stacked {
                let back: Vec<(f64, f64)> = xs.iter().zip(&base).map(|(x, v)| (*x, map(*v))).rev().collect();
                back.iter().map(|p| format!("L{} {}", f(p.0), f(p.1))).collect::<Vec<_>>().join(" ")
            } else {
                format!("L{} {} L{} {}", f(pts[pts.len() - 1].0), f(map(0.0)), f(pts[0].0), f(map(0.0)))
            };
            push_svg(root, sv("path").attr("d", format!("{d} {zero_line} Z")).attr("fill", color.clone()).attr("fill-opacity", f(fill_alpha)).attr("stroke", "none").class("pr-area"));
        }
        if !pts.is_empty() {
            push_svg(root, sv("path").attr("d", d).attr("fill", "none").attr("stroke", color.clone()).attr("stroke-width", "2").attr("stroke-linejoin", "round").class("pr-line"));
        }
        if show_points {
            for (k, p) in pts.iter().enumerate() {
                let mut c = sv("circle").attr("cx", f(p.0)).attr("cy", f(p.1)).attr("r", f(pr)).attr("fill", color.clone()).class("pr-point");
                if tooltips {
                    c = with_title(c, format!("{}: {}", rows[k].0, fmt_tick(vals[k])));
                }
                push_svg(root, c);
            }
        }
        if stacked {
            base = tops;
        }
    }
    axis_captions(cx, i, root, plot, h, legend_rows);
}

#[allow(clippy::too_many_arguments)]
fn scatter(cx: &ViewCtx, i: usize, root: &mut VNode, rows: &[Row], w: f64, h: f64, legend_rows: usize, colors: &[String]) {
    let plot = plot_rect(cx, i, w, h, legend_rows, true);
    let (l, t, r, b) = plot;
    // A scatter row is `x<TAB>y[<TAB>size]`: the label is the x value when it is a number, else the row number.
    let pts: Vec<(f32, f32, f32)> = rows.iter().enumerate().map(|(k, (lab, v))| (lab.trim().parse::<f32>().unwrap_or(k as f32), v[0], v.get(1).copied().unwrap_or(0.0))).collect();
    let (xlo, xhi) = pts.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0)));
    let (ylo, yhi) = pts.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
    if pts.is_empty() {
        return;
    }
    let (xt, xa, xb) = ticks(xlo.min(0.0), xhi.max(xlo + 1.0), 5);
    let (yt, ya, yb) = ticks(ylo.min(0.0), yhi.max(ylo + 1.0), 5);
    let mx = |v: f32| l + (v - xa) as f64 / (xb - xa).max(f32::EPSILON) as f64 * (r - l);
    let my = |v: f32| b - (v - ya) as f64 / (yb - ya).max(f32::EPSILON) as f64 * (b - t);
    let (show_grid, show_x, show_y) = (truthy(&cx.tree.get(i, "ShowGridLines")), truthy(&cx.tree.get(i, "ShowXAxis")), truthy(&cx.tree.get(i, "ShowYAxis")));
    gridlines(root, plot, &yt, &my, true, show_grid, show_y, true);
    gridlines(root, plot, &xt, &mx, false, show_grid, show_x, true);
    let bubble = !cx.tree.get(i, "BubbleField").is_empty();
    let max_r = parse_f64(&cx.tree.get(i, "BubbleScale")).unwrap_or(20.0).max(2.0);
    let max_size = pts.iter().map(|p| p.2).fold(0.0f32, f32::max).max(f32::EPSILON);
    for (k, p) in pts.iter().enumerate() {
        let radius = if bubble { (p.2 / max_size).sqrt() as f64 * max_r } else { 4.0 };
        let c = sv("circle").attr("cx", f(mx(p.0))).attr("cy", f(my(p.1))).attr("r", f(radius.max(2.0))).attr("fill", colors[0].clone()).attr("fill-opacity", if bubble { "0.6" } else { "1" }).class("pr-point");
        push_svg(root, with_title(c, format!("{}: ({}, {})", rows[k].0, fmt_tick(p.0), fmt_tick(p.1))));
    }
    axis_captions(cx, i, root, plot, h, legend_rows);
}

/// The path of a pie slice (or a donut segment when `inner > 0`) from `a0` to `a1` radians, clockwise from the top.
pub fn arc_path(cx_: f64, cy: f64, r: f64, inner: f64, a0: f64, a1: f64) -> String {
    let point = |rad: f64, a: f64| (cx_ + rad * (a - std::f64::consts::FRAC_PI_2).cos(), cy + rad * (a - std::f64::consts::FRAC_PI_2).sin());
    let large = if a1 - a0 > std::f64::consts::PI { 1 } else { 0 };
    let (x0, y0) = point(r, a0);
    let (x1, y1) = point(r, a1);
    if inner <= 0.0 {
        return format!("M{} {} L{} {} A{} {} 0 {} 1 {} {} Z", f(cx_), f(cy), f(x0), f(y0), f(r), f(r), large, f(x1), f(y1));
    }
    let (ix0, iy0) = point(inner, a0);
    let (ix1, iy1) = point(inner, a1);
    format!("M{} {} A{} {} 0 {} 1 {} {} L{} {} A{} {} 0 {} 0 {} {} Z", f(x0), f(y0), f(r), f(r), large, f(x1), f(y1), f(ix1), f(iy1), f(inner), f(inner), large, f(ix0), f(iy0))
}

fn pie(cx: &ViewCtx, i: usize, root: &mut VNode, rows: &[Row], w: f64, h: f64, donut: bool, _colors: &[String]) {
    let legend = truthy(&cx.tree.get(i, "ShowLegend"));
    let (_, top, _, bottom) = plot_rect(cx, i, w, h, usize::from(legend), false);
    let r = ((bottom - top).min(w - 20.0) / 2.0).max(8.0);
    let (cx_, cy) = (w / 2.0, (top + bottom) / 2.0);
    let inner = if donut { r * parse_f64(&cx.tree.get(i, "InnerRadius")).unwrap_or(40.0).clamp(5.0, 95.0) / 100.0 } else { 0.0 };
    let total: f64 = rows.iter().map(|(_, v)| v[0].max(0.0) as f64).sum();
    if total <= 0.0 {
        return;
    }
    let show_labels = truthy(&cx.tree.get(i, "ShowLabels"));
    let format = cx.tree.get(i, "LabelFormat").to_ascii_lowercase();
    let mut a = 0.0f64;
    for (k, (label, vals)) in rows.iter().enumerate() {
        let v = vals[0].max(0.0) as f64;
        if v <= 0.0 {
            continue;
        }
        let sweep = v / total * std::f64::consts::TAU;
        let end = a + sweep.min(std::f64::consts::TAU - 1e-6);
        let color = series_color(cx, i, k, rows.len());
        let share = (v / total * 100.0).round();
        let path = sv("path").attr("d", arc_path(cx_, cy, r, inner, a, end)).attr("fill", color).attr("stroke", "currentColor").attr("stroke-opacity", "0.25").class("pr-slice");
        push_svg(root, with_title(path, format!("{label}: {} ({}%)", fmt_tick(v as f32), fmt_number(share))));
        if show_labels && sweep > 0.25 {
            let mid = a + sweep / 2.0 - std::f64::consts::FRAC_PI_2;
            let rr = if donut { (r + inner) / 2.0 } else { r * 0.62 };
            let txt = match format.as_str() {
                "value" => fmt_tick(v as f32),
                "label" => label.clone(),
                _ => format!("{}%", fmt_number(share)),
            };
            push_svg(root, text_node(cx_ + rr * mid.cos(), cy + rr * mid.sin() + 4.0, "middle", 11.0, &txt).attr("fill", "#fff").class("pr-slice-label"));
        }
        a += sweep;
    }
}

fn radar(cx: &ViewCtx, i: usize, root: &mut VNode, rows: &[Row], w: f64, h: f64, colors: &[String]) {
    let n = rows.len();
    if n < 3 {
        return;
    }
    let legend = truthy(&cx.tree.get(i, "ShowLegend"));
    let (_, top, _, bottom) = plot_rect(cx, i, w, h, usize::from(legend), false);
    let r = ((bottom - top).min(w) / 2.0 - 22.0).max(10.0);
    let (cx_, cy) = (w / 2.0, (top + bottom) / 2.0);
    let levels = parse_f64(&cx.tree.get(i, "GridLevels")).unwrap_or(5.0).clamp(1.0, 10.0) as usize;
    let max = {
        let given = parse_f64(&cx.tree.get(i, "MaxValue")).unwrap_or(0.0) as f32;
        if given > 0.0 {
            given
        } else {
            rows.iter().flat_map(|(_, v)| v.iter().copied()).fold(0.0f32, f32::max).max(f32::EPSILON)
        }
    };
    let min = parse_f64(&cx.tree.get(i, "MinValue")).unwrap_or(0.0) as f32;
    let dir = |axis: usize| {
        let a = axis as f64 / n as f64 * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
        (a.cos(), a.sin())
    };
    for lv in 1..=levels {
        let rr = r * lv as f64 / levels as f64;
        let pts: Vec<String> = (0..n).map(|k| format!("{},{}", f(cx_ + rr * dir(k).0), f(cy + rr * dir(k).1))).collect();
        push_svg(root, sv("polygon").attr("points", pts.join(" ")).attr("fill", "none").attr("stroke", "currentColor").attr("stroke-opacity", "0.2"));
    }
    for (k, (label, _)) in rows.iter().enumerate() {
        let (dx, dy) = dir(k);
        push_svg(root, sv("line").attr("x1", f(cx_)).attr("y1", f(cy)).attr("x2", f(cx_ + r * dx)).attr("y2", f(cy + r * dy)).attr("stroke", "currentColor").attr("stroke-opacity", "0.2"));
        let anchor = if dx > 0.3 { "start" } else if dx < -0.3 { "end" } else { "middle" };
        push_svg(root, text_node(cx_ + (r + 8.0) * dx, cy + (r + 8.0) * dy + 4.0, anchor, 10.0, label).class("pr-tick"));
    }
    let series = rows.iter().map(|(_, v)| v.len()).max().unwrap_or(1);
    let fill_opacity = (parse_f64(&cx.tree.get(i, "FillOpacity")).unwrap_or(35.0) / 100.0).clamp(0.0, 1.0);
    for s in 0..series {
        let pts: Vec<(f64, f64)> = rows
            .iter()
            .enumerate()
            .map(|(k, (_, v))| {
                let frac = ((v.get(s).copied().unwrap_or(0.0) - min) / (max - min).max(f32::EPSILON)).clamp(0.0, 1.0) as f64;
                (cx_ + r * frac * dir(k).0, cy + r * frac * dir(k).1)
            })
            .collect();
        let color = colors[s % colors.len()].clone();
        let text: Vec<String> = pts.iter().map(|p| format!("{},{}", f(p.0), f(p.1))).collect();
        push_svg(root, sv("polygon").attr("points", text.join(" ")).attr("fill", color.clone()).attr("fill-opacity", f(fill_opacity)).attr("stroke", color.clone()).attr("stroke-width", "2").class("pr-radar"));
        if truthy(&cx.tree.get(i, "ShowPoints")) {
            let pr = parse_f64(&cx.tree.get(i, "PointRadius")).unwrap_or(3.0);
            for (k, p) in pts.iter().enumerate() {
                push_svg(root, with_title(sv("circle").attr("cx", f(p.0)).attr("cy", f(p.1)).attr("r", f(pr)).attr("fill", color.clone()), format!("{}: {}", rows[k].0, fmt_tick(rows[k].1.get(s).copied().unwrap_or(0.0)))));
            }
        }
    }
}

// ── Knob, Gauge ─────────────────────────────────────────────────────────────

/// The accent colour of a Knob or Switch: a name, or any colour.
pub fn accent(name: &str) -> String {
    match name.trim().to_ascii_lowercase().as_str() {
        "blue" | "" => "#4e8cff".into(),
        "green" => "#30a46c".into(),
        "red" => "#e5484d".into(),
        "orange" | "amber" => "#f5a623".into(),
        "purple" => "#8e4ec6".into(),
        "teal" | "cyan" => "#12a594".into(),
        other => color_attr(other).unwrap_or_else(|| "#4e8cff".into()),
    }
}

/// The angle (radians, clockwise from the top) of `value` on a dial spanning ±135 degrees.
pub fn dial_angle(value: f64, min: f64, max: f64) -> f64 {
    let span = (max - min).max(f64::EPSILON);
    let frac = ((value - min) / span).clamp(0.0, 1.0);
    (-135.0 + 270.0 * frac).to_radians()
}

pub fn knob(cx: &ViewCtx, i: usize) -> VNode {
    let (mut root, w, h) = svg_root(cx, i, "pr-knob");
    let (min, max) = (parse_f64(&cx.tree.get(i, "Minimum")).unwrap_or(0.0), parse_f64(&cx.tree.get(i, "Maximum")).unwrap_or(100.0));
    let v = parse_f64(&cx.tree.get(i, "Value")).unwrap_or(0.0).clamp(min.min(max), max.max(min));
    let r = (w.min(h) / 2.0 - 6.0).max(8.0);
    let (cx_, cy) = (w / 2.0, h / 2.0);
    let color = if cx.tree.get(i, "Accent").is_empty() { accent("Blue") } else { accent(&cx.tree.get(i, "Accent")) };
    let face = color_attr(&cx.tree.get(i, "FaceColor")).unwrap_or_else(|| "#2b2e35".into());
    let rim = color_attr(&cx.tree.get(i, "RimColor")).unwrap_or_else(|| "#565b66".into());
    let track = color_attr(&cx.tree.get(i, "TrackColor")).unwrap_or_else(|| "#444a55".into());
    let a_start = (-135.0f64).to_radians();
    let a_val = dial_angle(v, min, max);
    let a_end = 135.0f64.to_radians();
    let arc = |a0: f64, a1: f64, rr: f64| arc_path_stroke(cx_, cy, rr, a0, a1);
    push_svg(&mut root, sv("circle").attr("cx", f(cx_)).attr("cy", f(cy)).attr("r", f(r)).attr("fill", face).attr("stroke", rim).attr("stroke-width", "2"));
    push_svg(&mut root, sv("path").attr("d", arc(a_start, a_end, r * 0.82)).attr("fill", "none").attr("stroke", track).attr("stroke-width", "4").attr("stroke-linecap", "round"));
    let bipolar = truthy(&cx.tree.get(i, "Bipolar"));
    let (b0, b1) = if bipolar { (0.0, a_val).into_sorted() } else { (a_start, a_val) };
    if (b1 - b0).abs() > 1e-3 {
        push_svg(&mut root, sv("path").attr("d", arc(b0, b1, r * 0.82)).attr("fill", "none").attr("stroke", color.clone()).attr("stroke-width", "4").attr("stroke-linecap", "round").class("pr-knob-arc"));
    }
    let thumb_style = cx.tree.get(i, "IndicatorStyle").eq_ignore_ascii_case("Thumb");
    let (px, py) = (cx_ + r * 0.82 * a_val.sin(), cy - r * 0.82 * a_val.cos());
    if thumb_style {
        push_svg(&mut root, sv("circle").attr("cx", f(px)).attr("cy", f(py)).attr("r", "6").attr("fill", color.clone()).class("pr-knob-thumb"));
    } else {
        let (qx, qy) = (cx_ + r * 0.55 * a_val.sin(), cy - r * 0.55 * a_val.cos());
        push_svg(&mut root, sv("line").attr("x1", f(cx_)).attr("y1", f(cy)).attr("x2", f(qx)).attr("y2", f(qy)).attr("stroke", color).attr("stroke-width", "3").attr("stroke-linecap", "round").class("pr-knob-pointer"));
    }
    if truthy(&cx.tree.get(i, "ShowValue")) {
        push_svg(&mut root, text_node(cx_, cy + r * 0.55, "middle", (r * 0.28).max(9.0), &fmt_tick(v as f32)).class("pr-knob-value"));
    }
    let label = cx.tree.get(i, "Label");
    if !label.is_empty() {
        push_svg(&mut root, text_node(cx_, h - 2.0, "middle", 10.0, &label));
    }
    root = root
        .attr("role", "slider")
        .attr("tabindex", "0")
        .attr("aria-valuemin", fmt_number(min))
        .attr("aria-valuemax", fmt_number(max))
        .attr("aria-valuenow", fmt_number(v))
        .on("pointerdown")
        .on("pointermove")
        .on("pointerup")
        .on("keydown")
        .on("wheel");
    if !cx.tree.ctl(i).enabled() {
        root = root.class("pr-disabled").attr("aria-disabled", "true");
    }
    root
}

trait Sorted {
    fn into_sorted(self) -> (f64, f64);
}
impl Sorted for (f64, f64) {
    fn into_sorted(self) -> (f64, f64) {
        if self.0 <= self.1 {
            self
        } else {
            (self.1, self.0)
        }
    }
}

/// A stroked arc from `a0` to `a1` (radians, clockwise from the top).
pub fn arc_path_stroke(cx_: f64, cy: f64, r: f64, a0: f64, a1: f64) -> String {
    let p = |a: f64| (cx_ + r * a.sin(), cy - r * a.cos());
    let (x0, y0) = p(a0);
    let (x1, y1) = p(a1);
    let large = if (a1 - a0).abs() > std::f64::consts::PI { 1 } else { 0 };
    let sweep = if a1 >= a0 { 1 } else { 0 };
    format!("M{} {} A{} {} 0 {} {} {} {}", f(x0), f(y0), f(r), f(r), large, sweep, f(x1), f(y1))
}

/// The colour of the zone a gauge value is in, when both thresholds are set.
pub fn zone_color(cx: &ViewCtx, i: usize, v: f64, base: &str) -> String {
    let warn = parse_f64(&cx.tree.get(i, "WarningThreshold"));
    let crit = parse_f64(&cx.tree.get(i, "CriticalThreshold"));
    if let (Some(w), Some(c)) = (warn, crit) {
        let pick = |prop: &str, default: &str| color_attr(&cx.tree.get(i, prop)).unwrap_or_else(|| default.to_owned());
        return if (c >= w && v >= c) || (c < w && v <= c) {
            pick("CriticalColor", "#e5484d")
        } else if (c >= w && v >= w) || (c < w && v <= w) {
            pick("WarningColor", "#f5a623")
        } else {
            pick("NormalColor", "#30a46c")
        };
    }
    base.to_owned()
}

pub fn gauge(cx: &ViewCtx, i: usize) -> VNode {
    let (mut root, w, h) = svg_root(cx, i, "pr-gauge");
    let (min, max) = (parse_f64(&cx.tree.get(i, "Minimum")).unwrap_or(0.0), parse_f64(&cx.tree.get(i, "Maximum")).unwrap_or(100.0));
    let v = parse_f64(&cx.tree.get(i, "Value")).unwrap_or(0.0).clamp(min.min(max), max.max(min));
    let frac = ((v - min) / (max - min).max(f64::EPSILON)).clamp(0.0, 1.0);
    let base = color_attr(&cx.tree.get(i, "Color")).unwrap_or_else(|| "#4e8cff".into());
    let color = zone_color(cx, i, v, &base);
    let style = cx.tree.get(i, "GaugeStyle").to_ascii_lowercase();
    let readout = {
        let t = cx.tree.get(i, "Text");
        if !t.is_empty() {
            t
        } else {
            format!("{}{}", fmt_tick(v as f32), cx.tree.get(i, "Unit"))
        }
    };
    let track = "#444a55";
    match style.as_str() {
        "linear" => {
            let bh = parse_f64(&cx.tree.get(i, "BarHeight")).unwrap_or(14.0).clamp(4.0, h / 2.0);
            let (x0, x1, y) = (8.0, w - 8.0, h / 2.0 - bh / 2.0);
            push_svg(&mut root, sv("rect").attr("x", f(x0)).attr("y", f(y)).attr("width", f(x1 - x0)).attr("height", f(bh)).attr("rx", f(bh / 2.0)).attr("fill", track));
            push_svg(&mut root, sv("rect").attr("x", f(x0)).attr("y", f(y)).attr("width", f((x1 - x0) * frac)).attr("height", f(bh)).attr("rx", f(bh / 2.0)).attr("fill", color.clone()).class("pr-gauge-fill"));
            if truthy(&cx.tree.get(i, "ShowThumb")) {
                push_svg(&mut root, sv("circle").attr("cx", f(x0 + (x1 - x0) * frac)).attr("cy", f(y + bh / 2.0)).attr("r", f(bh * 0.7)).attr("fill", color));
            }
            push_svg(&mut root, text_node(w / 2.0, y + bh + 18.0, "middle", 12.0, &readout).class("pr-gauge-value"));
        }
        "donut" => {
            let sw = parse_f64(&cx.tree.get(i, "StrokeWidth")).unwrap_or(8.0).clamp(2.0, 40.0);
            let r = (w.min(h) / 2.0 - sw / 2.0 - 2.0).max(6.0);
            let (cx_, cy) = (w / 2.0, h / 2.0);
            push_svg(&mut root, sv("circle").attr("cx", f(cx_)).attr("cy", f(cy)).attr("r", f(r)).attr("fill", "none").attr("stroke", track).attr("stroke-width", f(sw)));
            if frac > 0.0 {
                let end = (frac * std::f64::consts::TAU).min(std::f64::consts::TAU - 1e-3);
                push_svg(&mut root, sv("path").attr("d", arc_path_stroke(cx_, cy, r, 0.0, end)).attr("fill", "none").attr("stroke", color).attr("stroke-width", f(sw)).attr("stroke-linecap", "round").class("pr-gauge-fill"));
            }
            push_svg(&mut root, text_node(cx_, cy + 5.0, "middle", (r * 0.4).max(10.0), &readout).class("pr-gauge-value"));
        }
        _ => {
            // Radial: a 240 degree dial with a needle.
            let r = (w.min(h * 1.1) / 2.0 - 8.0).max(8.0);
            let (cx_, cy) = (w / 2.0, h / 2.0 + r * 0.1);
            let (a0, a1) = ((-120.0f64).to_radians(), 120.0f64.to_radians());
            push_svg(&mut root, sv("path").attr("d", arc_path_stroke(cx_, cy, r, a0, a1)).attr("fill", "none").attr("stroke", track).attr("stroke-width", "8").attr("stroke-linecap", "round"));
            let av = a0 + (a1 - a0) * frac;
            if frac > 0.0 {
                push_svg(&mut root, sv("path").attr("d", arc_path_stroke(cx_, cy, r, a0, av)).attr("fill", "none").attr("stroke", color.clone()).attr("stroke-width", "8").attr("stroke-linecap", "round").class("pr-gauge-fill"));
            }
            if truthy(&cx.tree.get(i, "ShowScale")) {
                for k in 0..=10 {
                    let a = a0 + (a1 - a0) * k as f64 / 10.0;
                    let (s, c) = (a.sin(), a.cos());
                    push_svg(&mut root, sv("line").attr("x1", f(cx_ + (r - 14.0) * s)).attr("y1", f(cy - (r - 14.0) * c)).attr("x2", f(cx_ + (r - 20.0) * s)).attr("y2", f(cy - (r - 20.0) * c)).attr("stroke", "currentColor").attr("stroke-opacity", "0.5"));
                }
            }
            if truthy(&cx.tree.get(i, "ShowNeedle")) {
                let needle = color_attr(&cx.tree.get(i, "NeedleColor")).unwrap_or_else(|| "#e5484d".into());
                push_svg(&mut root, sv("line").attr("x1", f(cx_)).attr("y1", f(cy)).attr("x2", f(cx_ + (r - 18.0) * av.sin())).attr("y2", f(cy - (r - 18.0) * av.cos())).attr("stroke", needle).attr("stroke-width", "3").attr("stroke-linecap", "round").class("pr-gauge-needle"));
                push_svg(&mut root, sv("circle").attr("cx", f(cx_)).attr("cy", f(cy)).attr("r", "5").attr("fill", "currentColor"));
            }
            let down = cx.tree.get(i, "ReadoutPosition").eq_ignore_ascii_case("Down");
            push_svg(&mut root, text_node(cx_, if down { cy + r * 0.55 } else { cy - r * 0.25 }, "middle", (r * 0.22).max(10.0), &readout).class("pr-gauge-value"));
        }
    }
    root = root.attr("role", "meter").attr("aria-valuemin", fmt_number(min)).attr("aria-valuemax", fmt_number(max)).attr("aria-valuenow", fmt_number(v));
    root
}

// ── Shape ───────────────────────────────────────────────────────────────────

pub fn shape(cx: &ViewCtx, i: usize) -> VNode {
    let (mut root, w, h) = svg_root(cx, i, "pr-shape");
    let kind = cx.tree.get(i, "ShapeType");
    let fill_style = cx.tree.get(i, "FillStyle");
    let fill = color_attr(&cx.tree.get(i, "FillColor")).unwrap_or_else(|| "#cccccc".into());
    let line = color_attr(&cx.tree.get(i, "LineColor")).unwrap_or_else(|| "#000000".into());
    let thick = parse_f64(&cx.tree.get(i, "LineThickness")).unwrap_or(1.0).clamp(0.0, 40.0);
    let dash = match cx.tree.get(i, "LineStyle").to_ascii_lowercase().as_str() {
        "dash" => Some("8 4"),
        "dot" => Some("2 3"),
        "dashdot" => Some("8 3 2 3"),
        _ => None,
    };
    let none_line = cx.tree.get(i, "LineStyle").eq_ignore_ascii_case("None");
    let inset = thick / 2.0;
    let radius = parse_f64(&cx.tree.get(i, "CornerRadius")).unwrap_or(0.0).max(0.0);
    let mut s = match kind.as_str() {
        "Circle" => {
            let r = (w.min(h) / 2.0 - inset).max(1.0);
            sv("circle").attr("cx", f(w / 2.0)).attr("cy", f(h / 2.0)).attr("r", f(r))
        }
        "Ellipse" => sv("ellipse").attr("cx", f(w / 2.0)).attr("cy", f(h / 2.0)).attr("rx", f((w / 2.0 - inset).max(1.0))).attr("ry", f((h / 2.0 - inset).max(1.0))),
        "Triangle" => sv("polygon").attr("points", format!("{},{} {},{} {},{}", f(w / 2.0), f(inset), f(inset), f(h - inset), f(w - inset), f(h - inset))),
        other => {
            let rx = if other == "RoundRect" { 8.0 } else { radius.min(w.min(h) / 2.0) };
            sv("rect").attr("x", f(inset)).attr("y", f(inset)).attr("width", f((w - thick).max(1.0))).attr("height", f((h - thick).max(1.0))).attr("rx", f(rx))
        }
    };
    s = s.attr("fill", if fill_style.eq_ignore_ascii_case("None") { "none".to_owned() } else { fill });
    if none_line || thick <= 0.0 {
        s = s.attr("stroke", "none");
    } else {
        s = s.attr("stroke", line).attr("stroke-width", f(thick));
        if let Some(d) = dash {
            s = s.attr("stroke-dasharray", d);
        }
    }
    push_svg(&mut root, s.class("pr-shape-body"));
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bar_stands_on_the_zero_line_whatever_its_sign() {
        let map = |v: f32| 100.0 - v as f64; // zero at y = 100, up is smaller y
        let (_, y, _, h) = bar_rect(0.0, 10.0, 30.0, &map, 100.0);
        assert_eq!((y, h), (70.0, 30.0), "a positive bar rises from the line");
        let (_, y, _, h) = bar_rect(0.0, 10.0, -20.0, &map, 100.0);
        assert_eq!((y, h), (100.0, 20.0), "a negative bar hangs from it");
    }

    #[test]
    fn ticks_are_nice_numbers_that_cover_the_range() {
        let (t, lo, hi) = ticks(0.0, 87.0, 5);
        assert_eq!(t.first(), Some(&0.0));
        assert!(hi >= 87.0 && lo <= 0.0, "{t:?}");
        assert!(t.windows(2).all(|w| (w[1] - w[0] - (t[1] - t[0])).abs() < 1e-3), "even steps: {t:?}");
        assert!(t.len() <= 12);
    }

    #[test]
    fn a_pie_slice_is_a_closed_path_and_a_half_circle_takes_the_large_arc_flag_only_past_half() {
        let half = arc_path(50.0, 50.0, 40.0, 0.0, 0.0, std::f64::consts::PI);
        let more = arc_path(50.0, 50.0, 40.0, 0.0, 0.0, std::f64::consts::PI + 0.1);
        assert!(half.contains(" 0 0 1 "), "{half}");
        assert!(more.contains(" 0 1 1 "), "{more}");
        assert!(half.ends_with('Z'));
        let donut = arc_path(50.0, 50.0, 40.0, 20.0, 0.0, 1.0);
        assert_eq!(donut.matches('A').count(), 2, "an outer and an inner arc");
    }

    #[test]
    fn the_dial_runs_from_minus_135_to_plus_135_degrees() {
        assert!((dial_angle(0.0, 0.0, 100.0).to_degrees() + 135.0).abs() < 1e-9);
        assert!((dial_angle(100.0, 0.0, 100.0).to_degrees() - 135.0).abs() < 1e-9);
        assert!(dial_angle(50.0, 0.0, 100.0).abs() < 1e-9);
        assert!((dial_angle(500.0, 0.0, 100.0).to_degrees() - 135.0).abs() < 1e-9, "clamped");
    }

    #[test]
    fn a_smooth_path_passes_through_every_point() {
        let d = smooth_path(&[(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)]);
        assert!(d.starts_with("M0 0"));
        assert!(d.ends_with("20 0"));
        assert_eq!(d.matches('C').count(), 2);
    }

    #[test]
    fn monochrome_series_are_tones_of_one_colour() {
        let mut t = crate::tree::testing::tree();
        let _ = &mut t;
    }
}
