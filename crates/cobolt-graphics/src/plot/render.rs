// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Drawing a [`Plot`] with plotters onto any backend.

use plotters::coord::Shift;
use plotters::prelude::*;
use plotters::style::text_anchor::{HPos, Pos, VPos};
use plotters_backend::DrawingBackend;

use super::{Plot, SeriesData, SeriesKind};
use crate::color::Rgba;
use crate::detmath::{radians, sin_cos};
use crate::error::{GraphicsError, Result};

/// The default series colours (distinct on a light and on a dark background).
pub(crate) const PALETTE: [Rgba; 10] = [
    Rgba::rgb(31, 119, 180),
    Rgba::rgb(255, 127, 14),
    Rgba::rgb(44, 160, 44),
    Rgba::rgb(214, 39, 40),
    Rgba::rgb(148, 103, 189),
    Rgba::rgb(140, 86, 75),
    Rgba::rgb(227, 119, 194),
    Rgba::rgb(127, 127, 127),
    Rgba::rgb(188, 189, 34),
    Rgba::rgb(23, 190, 207),
];

fn col(c: Rgba) -> RGBAColor {
    RGBAColor(c.r, c.g, c.b, c.a as f64 / 255.0)
}

fn pl<E: std::fmt::Display>(e: E) -> GraphicsError {
    GraphicsError::invalid(format!("the chart could not be drawn: {e}"))
}

/// A viridis-like colour map, `t` in 0..=1.
pub(crate) fn colormap(t: f64) -> Rgba {
    const STOPS: [(u8, u8, u8); 9] = [(68, 1, 84), (71, 44, 122), (59, 81, 139), (44, 113, 142), (33, 144, 141), (39, 173, 129), (92, 200, 99), (170, 220, 50), (253, 231, 37)];
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
    let x = t * 8.0;
    let i = (x.floor() as usize).min(7);
    let f = x - i as f64;
    let mix = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * f).round() as u8;
    let (a, b) = (STOPS[i], STOPS[i + 1]);
    Rgba::rgb(mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2))
}

/// A "nice" step for about `n` divisions of `span`.
pub(crate) fn nice_step(span: f64, n: f64) -> f64 {
    if !(span.is_finite() && span > 0.0) {
        return 1.0;
    }
    let raw = span / n.max(1.0);
    // The decade of `raw` by repeated scaling (no log10, whose last bit differs between platforms).
    let mut mag = 1.0f64;
    while raw / mag >= 10.0 && mag < 1e300 {
        mag *= 10.0;
    }
    while raw / mag < 1.0 && mag > 1e-300 {
        mag /= 10.0;
    }
    let f = raw / mag;
    // Round to the nearest of 1, 2, 5, 10 (thresholds 1.5, 3, 7).
    let nice = if f < 1.5 {
        1.0
    } else if f < 3.0 {
        2.0
    } else if f < 7.0 {
        5.0
    } else {
        10.0
    };
    nice * mag
}

fn pad(lo: f64, hi: f64) -> (f64, f64) {
    if !(lo.is_finite() && hi.is_finite()) {
        return (0.0, 1.0);
    }
    if (hi - lo).abs() < 1e-12 {
        return (lo - 1.0, hi + 1.0);
    }
    let p = (hi - lo) * 0.05;
    (lo - p, hi + p)
}

struct Bounds {
    x: (f64, f64),
    y: (f64, f64),
}

fn track(b: &mut Option<(f64, f64, f64, f64)>, x: f64, y: f64) {
    if !(x.is_finite() && y.is_finite()) {
        return;
    }
    *b = Some(match *b {
        None => (x, x, y, y),
        Some((x0, x1, y0, y1)) => (x0.min(x), x1.max(x), y0.min(y), y1.max(y)),
    });
}

impl Plot {
    pub(crate) fn series_color(&self, i: usize, c: Rgba) -> Rgba {
        if c == Rgba::TRANSPARENT {
            PALETTE[i % PALETTE.len()]
        } else {
            c
        }
    }

    /// Which family of chart the series belong to; an error when they cannot share a chart.
    fn mode(&self) -> Result<Mode> {
        let mut mode: Option<Mode> = None;
        for (_, s) in self.series.iter() {
            let m = match s.kind {
                SeriesKind::Pie => Mode::Pie,
                SeriesKind::Polar => Mode::Polar,
                SeriesKind::Surface3d | SeriesKind::Scatter3d => Mode::ThreeD,
                _ => Mode::Cartesian,
            };
            match mode {
                None => mode = Some(m),
                Some(prev) if prev == m => {}
                Some(prev) => {
                    return Err(GraphicsError::invalid(format!(
                        "a plot cannot mix {prev:?} and {m:?} series: use one plot for each"
                    )))
                }
            }
        }
        mode.ok_or_else(|| GraphicsError::invalid("the plot has no series to draw"))
    }

    pub(crate) fn draw<DB: DrawingBackend>(&self, root: &DrawingArea<DB, Shift>) -> Result<()> {
        root.fill(&col(self.options.background)).map_err(pl)?;
        match self.mode()? {
            Mode::Cartesian => self.draw_cartesian(root),
            Mode::Pie => self.draw_pie(root),
            Mode::Polar => self.draw_polar(root),
            Mode::ThreeD => self.draw_3d(root),
        }?;
        root.present().map_err(pl)?;
        Ok(())
    }

    fn font(&self, scale: f32) -> FontDesc<'_> {
        (self.options.font_family.as_str(), (self.options.font_size * scale) as f64).into_font()
    }

    fn text_style(&self, scale: f32) -> TextStyle<'_> {
        self.font(scale).color(&col(self.options.foreground))
    }

    fn bounds(&self) -> Bounds {
        let mut b: Option<(f64, f64, f64, f64)> = None;
        let mut bars = 0usize;
        for (_, s) in self.series.iter() {
            match &s.data {
                SeriesData::Xy(p) => {
                    for (x, y) in p {
                        track(&mut b, *x, *y);
                    }
                }
                SeriesData::Errors(p) => {
                    for (x, y, e) in p {
                        track(&mut b, *x, y - e.abs());
                        track(&mut b, *x, y + e.abs());
                    }
                }
                SeriesData::Candles(p) => {
                    for c in p {
                        track(&mut b, c[0], c[3]);
                        track(&mut b, c[0], c[2]);
                    }
                }
                SeriesData::Values { values, .. } => {
                    if s.kind == SeriesKind::Histogram {
                        let (lo, hi) = values.iter().filter(|v| v.is_finite()).fold((f64::MAX, f64::MIN), |a, v| (a.0.min(*v), a.1.max(*v)));
                        if lo <= hi {
                            track(&mut b, lo, 0.0);
                            track(&mut b, hi, 0.0);
                        }
                        let bins = self.histogram_bins(values);
                        for (_, _, n) in bins {
                            track(&mut b, lo, n as f64);
                        }
                    } else {
                        bars = bars.max(values.len());
                        for (i, v) in values.iter().enumerate() {
                            track(&mut b, i as f64 - 0.5, 0.0);
                            track(&mut b, i as f64 + 0.5, *v);
                        }
                    }
                }
                SeriesData::Grid { cols, rows, .. } => {
                    track(&mut b, 0.0, 0.0);
                    track(&mut b, *cols as f64, *rows as f64);
                }
                SeriesData::Xyz(_) | SeriesData::Surface { .. } => {}
            }
            if s.kind == SeriesKind::Bar || s.kind == SeriesKind::Area {
                track(&mut b, 0.0, 0.0);
            }
        }
        let (x0, x1, y0, y1) = b.unwrap_or((0.0, 1.0, 0.0, 1.0));
        let (mut x, mut y) = ((x0, x1), (y0, y1));
        // Bars and areas sit on a zero line: keep it in view without padding below it.
        let has_base = self.series.iter().any(|(_, s)| matches!(s.kind, SeriesKind::Bar | SeriesKind::Area | SeriesKind::Histogram));
        if bars == 0 {
            x = pad(x.0, x.1);
        }
        y = if has_base { (if y.0 < 0.0 { pad(y.0, y.1).0 } else { 0.0 }, pad(y.0, y.1).1) } else { pad(y.0, y.1) };
        if let Some(r) = self.options.x_range {
            x = r;
        }
        if let Some(r) = self.options.y_range {
            y = r;
        }
        if x.0 >= x.1 {
            x = (x.0, x.0 + 1.0);
        }
        if y.0 >= y.1 {
            y = (y.0, y.0 + 1.0);
        }
        Bounds { x, y }
    }

    /// `(low edge, high edge, count)` per bin.
    pub(crate) fn histogram_bins(&self, values: &[f64]) -> Vec<(f64, f64, u32)> {
        let v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
        if v.is_empty() {
            return Vec::new();
        }
        let (lo, hi) = v.iter().fold((f64::MAX, f64::MIN), |a, x| (a.0.min(*x), a.1.max(*x)));
        let n = if self.options.histogram_bins > 0 {
            self.options.histogram_bins as usize
        } else {
            // Sturges' rule.
            ((v.len() as f64).log2().ceil() as usize + 1).clamp(1, 200)
        };
        let width = if hi > lo { (hi - lo) / n as f64 } else { 1.0 };
        let mut counts = vec![0u32; n];
        for x in &v {
            let i = (((x - lo) / width) as usize).min(n - 1);
            counts[i] += 1;
        }
        (0..n).map(|i| (lo + i as f64 * width, lo + (i + 1) as f64 * width, counts[i])).collect()
    }

    fn draw_cartesian<DB: DrawingBackend>(&self, root: &DrawingArea<DB, Shift>) -> Result<()> {
        let o = &self.options;
        let b = self.bounds();
        let mut cb = ChartBuilder::on(root);
        cb.margin(12).x_label_area_size((o.font_size * 2.6) as u32).y_label_area_size((o.font_size * 3.6) as u32);
        if !o.title.is_empty() {
            cb.caption(&o.title, self.text_style(1.4));
        }
        let mut chart = cb.build_cartesian_2d(b.x.0..b.x.1, b.y.0..b.y.1).map_err(pl)?;
        let fg = col(o.foreground);
        {
            let mut mesh = chart.configure_mesh();
            mesh.label_style(self.text_style(1.0)).axis_style(fg).x_desc(o.x_label.as_str()).y_desc(o.y_label.as_str());
            if o.grid {
                mesh.light_line_style(fg.mix(0.10)).bold_line_style(fg.mix(0.25)).max_light_lines(1);
            } else {
                mesh.disable_mesh();
            }
            let categories = &o.categories;
            if !categories.is_empty() && self.series.iter().any(|(_, s)| s.kind == SeriesKind::Bar) {
                let cats = categories.clone();
                let formatter = move |v: &f64| {
                    let i = v.round();
                    if (v - i).abs() < 1e-9 && i >= 0.0 && (i as usize) < cats.len() {
                        cats[i as usize].clone()
                    } else {
                        String::new()
                    }
                };
                mesh.x_label_formatter(&formatter).x_labels(categories.len().clamp(1, 40));
                mesh.draw().map_err(pl)?;
            } else {
                mesh.draw().map_err(pl)?;
            }
        }
        let bar_series: Vec<usize> = self.series.iter().enumerate().filter(|(_, (_, s))| s.kind == SeriesKind::Bar).map(|(i, _)| i).collect();
        let mut any_legend = false;
        for (i, (_, s)) in self.series.iter().enumerate() {
            let color = self.series_color(i, s.color);
            let rc = col(color);
            let lw = s.line_width.max(1);
            let stroke = ShapeStyle { color: rc, filled: false, stroke_width: lw };
            let label = s.name.clone();
            let legend_line = move |(x, y): (i32, i32)| PathElement::new(vec![(x, y), (x + 20, y)], ShapeStyle { color: rc, filled: false, stroke_width: lw });
            let legend_box = move |(x, y): (i32, i32)| Rectangle::new([(x + 2, y - 5), (x + 16, y + 5)], rc.filled());
            macro_rules! labelled {
                ($anno:expr, $legend:expr) => {{
                    let anno = $anno.map_err(pl)?;
                    if !label.is_empty() {
                        any_legend = true;
                        anno.label(label.clone()).legend($legend);
                    }
                }};
            }
            match (&s.kind, &s.data) {
                (SeriesKind::Line, SeriesData::Xy(p)) => {
                    let pts: Vec<(f64, f64)> = p.iter().copied().filter(|(x, y)| x.is_finite() && y.is_finite()).collect();
                    let ps = s.point_size;
                    labelled!(
                        chart.draw_series(LineSeries::new(pts.clone(), stroke).point_size(ps)),
                        legend_line
                    );
                }
                (SeriesKind::Scatter, SeriesData::Xy(p)) => {
                    let ps = s.point_size.max(1);
                    let pts: Vec<(f64, f64)> = p.iter().copied().filter(|(x, y)| x.is_finite() && y.is_finite()).collect();
                    labelled!(chart.draw_series(pts.iter().map(|&(x, y)| Circle::new((x, y), ps, rc.filled()))), move |(x, y): (i32, i32)| Circle::new((x + 10, y), 4, rc.filled()));
                }
                (SeriesKind::Area, SeriesData::Xy(p)) => {
                    let pts: Vec<(f64, f64)> = p.iter().copied().filter(|(x, y)| x.is_finite() && y.is_finite()).collect();
                    let base = 0f64.max(b.y.0);
                    labelled!(chart.draw_series(AreaSeries::new(pts, base, rc.mix(0.3)).border_style(stroke)), legend_box);
                }
                (SeriesKind::Bar, SeriesData::Values { values, .. }) => {
                    let n = bar_series.len().max(1) as f64;
                    let slot = bar_series.iter().position(|k| *k == i).unwrap_or(0) as f64;
                    let w = 0.8 / n;
                    let left = -0.4 + slot * w;
                    let rects: Vec<_> = values
                        .iter()
                        .enumerate()
                        .filter(|(_, v)| v.is_finite())
                        .map(|(k, v)| Rectangle::new([(k as f64 + left, 0.0), (k as f64 + left + w, *v)], rc.filled()))
                        .collect();
                    labelled!(chart.draw_series(rects), legend_box);
                }
                (SeriesKind::Histogram, SeriesData::Values { values, .. }) => {
                    let rects: Vec<_> = self
                        .histogram_bins(values)
                        .into_iter()
                        .map(|(x0, x1, n)| Rectangle::new([(x0, 0.0), (x1, n as f64)], rc.mix(0.75).filled()))
                        .collect();
                    labelled!(chart.draw_series(rects), legend_box);
                }
                (SeriesKind::Candlestick, SeriesData::Candles(c)) => {
                    let width = (s.point_size.max(1) * 4).clamp(3, 40);
                    let up = RGBColor(44, 160, 44);
                    let down = RGBColor(214, 39, 40);
                    let items: Vec<_> = c
                        .iter()
                        .filter(|r| r.iter().all(|v| v.is_finite()))
                        .map(|r| CandleStick::new(r[0], r[1], r[2], r[3], r[4], up.filled(), down.filled(), width))
                        .collect();
                    labelled!(chart.draw_series(items), legend_box);
                }
                (SeriesKind::ErrorBars, SeriesData::Errors(e)) => {
                    let items: Vec<_> = e
                        .iter()
                        .filter(|(x, y, er)| x.is_finite() && y.is_finite() && er.is_finite())
                        .map(|(x, y, er)| ErrorBar::new_vertical(*x, y - er.abs(), *y, y + er.abs(), rc.filled(), 7))
                        .collect();
                    labelled!(chart.draw_series(items), legend_line);
                }
                (SeriesKind::Heatmap, SeriesData::Grid { cols, rows, z }) => {
                    let (lo, hi) = z.iter().filter(|v| v.is_finite()).fold((f64::MAX, f64::MIN), |a, v| (a.0.min(*v), a.1.max(*v)));
                    let span = if hi > lo { hi - lo } else { 1.0 };
                    let mut cells = Vec::with_capacity(z.len());
                    for r in 0..*rows {
                        for c in 0..*cols {
                            let v = z[r * cols + c];
                            if v.is_finite() {
                                let t = (v - lo) / span;
                                // Row 0 is drawn at the top.
                                let y = (*rows - 1 - r) as f64;
                                cells.push(Rectangle::new([(c as f64, y), (c as f64 + 1.0, y + 1.0)], col(colormap(t)).filled()));
                            }
                        }
                    }
                    chart.draw_series(cells).map_err(pl)?;
                }
                _ => {}
            }
        }
        if any_legend && o.legend {
            chart
                .configure_series_labels()
                .label_font(self.font(0.9).color(&fg))
                .background_style(col(o.background).mix(0.8))
                .border_style(fg.mix(0.5))
                .position(SeriesLabelPosition::UpperRight)
                .draw()
                .map_err(pl)?;
        }
        Ok(())
    }

    fn draw_pie<DB: DrawingBackend>(&self, root: &DrawingArea<DB, Shift>) -> Result<()> {
        let o = &self.options;
        let (w, h) = root.dim_in_pixel();
        let s = self.series.iter().map(|(_, s)| s).next().ok_or_else(|| GraphicsError::invalid("the plot has no series to draw"))?;
        let SeriesData::Values { values, labels } = &s.data else {
            return Err(GraphicsError::invalid("a pie series takes values (AddValue) with labels"));
        };
        let sizes: Vec<f64> = values.iter().map(|v| if v.is_finite() && *v > 0.0 { *v } else { 0.0 }).collect();
        if sizes.iter().sum::<f64>() <= 0.0 {
            return Err(GraphicsError::invalid("a pie needs at least one positive value"));
        }
        let colors: Vec<RGBColor> = (0..sizes.len()).map(|i| {
            let c = PALETTE[i % PALETTE.len()];
            RGBColor(c.r, c.g, c.b)
        }).collect();
        let names: Vec<String> = (0..sizes.len()).map(|i| labels.get(i).cloned().unwrap_or_default()).collect();
        let title_h = if o.title.is_empty() { 0 } else { (o.font_size * 2.2) as i32 };
        if !o.title.is_empty() {
            root.draw(&Text::new(o.title.clone(), (w as i32 / 2, (o.font_size * 0.6) as i32), self.text_style(1.4).pos(Pos::new(HPos::Center, VPos::Top)))).map_err(pl)?;
        }
        let radius = (w.min(h.saturating_sub(title_h as u32)) as f64 * 0.32).max(8.0);
        let center = (w as i32 / 2, title_h + (h as i32 - title_h) / 2);
        let mut pie = Pie::new(&center, &radius, &sizes, &colors, &names);
        pie.label_style(self.text_style(1.0));
        pie.percentages(self.text_style(0.9));
        root.draw(&pie).map_err(pl)?;
        Ok(())
    }

    fn draw_polar<DB: DrawingBackend>(&self, root: &DrawingArea<DB, Shift>) -> Result<()> {
        let o = &self.options;
        let mut rmax = 0f64;
        for (_, s) in self.series.iter() {
            if let SeriesData::Xy(p) = &s.data {
                for (_, r) in p {
                    if r.is_finite() {
                        rmax = rmax.max(r.abs());
                    }
                }
            }
        }
        if let Some((_, hi)) = o.y_range {
            rmax = hi;
        }
        let step = nice_step(rmax.max(1e-9), 4.0);
        let rmax = (rmax / step).ceil().max(1.0) * step;
        let lim = rmax * 1.18;
        let mut cb = ChartBuilder::on(root);
        cb.margin(10);
        if !o.title.is_empty() {
            cb.caption(&o.title, self.text_style(1.4));
        }
        let mut chart = cb.build_cartesian_2d(-lim..lim, -lim..lim).map_err(pl)?;
        let fg = col(o.foreground);
        // Rings and spokes.
        let ring = |r: f64| -> Vec<(f64, f64)> {
            (0..=120)
                .map(|i| {
                    let (s, c) = sin_cos(radians(i as f64 * 3.0));
                    (r * c, r * s)
                })
                .collect()
        };
        let mut r = step;
        while r <= rmax + step * 1e-6 {
            chart.draw_series(std::iter::once(PathElement::new(ring(r), fg.mix(0.25)))).map_err(pl)?;
            chart
                .draw_series(std::iter::once(Text::new(format!("{}", trim_num(r)), (0.0, r), self.text_style(0.8).pos(Pos::new(HPos::Left, VPos::Bottom)))))
                .map_err(pl)?;
            r += step;
        }
        for k in 0..12 {
            let (s, c) = sin_cos(radians(k as f64 * 30.0));
            chart.draw_series(std::iter::once(PathElement::new(vec![(0.0, 0.0), (rmax * c, rmax * s)], fg.mix(0.2)))).map_err(pl)?;
            chart
                .draw_series(std::iter::once(Text::new(
                    format!("{}", k * 30),
                    (rmax * 1.08 * c, rmax * 1.08 * s),
                    self.text_style(0.8).pos(Pos::new(HPos::Center, VPos::Center)),
                )))
                .map_err(pl)?;
        }
        let mut any_legend = false;
        for (i, (_, s)) in self.series.iter().enumerate() {
            let SeriesData::Xy(p) = &s.data else { continue };
            let rc = col(self.series_color(i, s.color));
            let mut pts: Vec<(f64, f64)> = p
                .iter()
                .filter(|(a, r)| a.is_finite() && r.is_finite())
                .map(|(a, r)| {
                    let (sn, cs) = sin_cos(radians(*a));
                    (r * cs, r * sn)
                })
                .collect();
            if pts.len() > 2 && s.closed {
                pts.push(pts[0]);
            }
            let lw = s.line_width.max(1);
            let anno = chart.draw_series(LineSeries::new(pts.clone(), ShapeStyle { color: rc, filled: false, stroke_width: lw })).map_err(pl)?;
            if !s.name.is_empty() {
                any_legend = true;
                anno.label(s.name.clone()).legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], ShapeStyle { color: rc, filled: false, stroke_width: lw }));
            }
            let ps = s.point_size;
            if ps > 0 {
                chart.draw_series(pts.iter().map(|&(x, y)| Circle::new((x, y), ps, rc.filled()))).map_err(pl)?;
            }
        }
        if any_legend && o.legend {
            chart
                .configure_series_labels()
                .label_font(self.font(0.9).color(&fg))
                .background_style(col(o.background).mix(0.8))
                .border_style(fg.mix(0.5))
                .position(SeriesLabelPosition::UpperRight)
                .draw()
                .map_err(pl)?;
        }
        Ok(())
    }

    fn draw_3d<DB: DrawingBackend>(&self, root: &DrawingArea<DB, Shift>) -> Result<()> {
        let o = &self.options;
        // Ranges: x and y (depth) from the data, z (height) likewise.
        let (mut x, mut y, mut z) = ((f64::MAX, f64::MIN), (f64::MAX, f64::MIN), (f64::MAX, f64::MIN));
        let ext = |r: &mut (f64, f64), v: f64| {
            if v.is_finite() {
                r.0 = r.0.min(v);
                r.1 = r.1.max(v);
            }
        };
        for (_, s) in self.series.iter() {
            match &s.data {
                SeriesData::Xyz(p) => {
                    for (a, b, c) in p {
                        ext(&mut x, *a);
                        ext(&mut y, *b);
                        ext(&mut z, *c);
                    }
                }
                SeriesData::Surface { x0, x1, y0, y1, z: g, .. } => {
                    ext(&mut x, *x0);
                    ext(&mut x, *x1);
                    ext(&mut y, *y0);
                    ext(&mut y, *y1);
                    for v in g {
                        ext(&mut z, *v);
                    }
                }
                _ => {}
            }
        }
        let fix = |r: (f64, f64), user: Option<(f64, f64)>| -> (f64, f64) {
            if let Some(u) = user {
                return u;
            }
            if r.0 > r.1 {
                (0.0, 1.0)
            } else if r.0 == r.1 {
                (r.0 - 1.0, r.1 + 1.0)
            } else {
                r
            }
        };
        let (x, y, z) = (fix(x, o.x_range), fix(y, None), fix(z, o.y_range));
        let mut cb = ChartBuilder::on(root);
        cb.margin(18);
        if !o.title.is_empty() {
            cb.caption(&o.title, self.text_style(1.4));
        }
        let mut chart = cb.build_cartesian_3d(x.0..x.1, z.0..z.1, y.0..y.1).map_err(pl)?;
        let (yaw, pitch) = (o.yaw, o.pitch);
        chart.with_projection(|mut p| {
            p.yaw = yaw;
            p.pitch = pitch;
            p.scale = 0.82;
            p.into_matrix()
        });
        let fg = col(o.foreground);
        chart
            .configure_axes()
            .label_style(self.text_style(0.9))
            .axis_panel_style(fg.mix(0.04))
            .light_grid_style(fg.mix(0.15))
            .max_light_lines(3)
            .draw()
            .map_err(pl)?;
        for (i, (_, s)) in self.series.iter().enumerate() {
            let rc = col(self.series_color(i, s.color));
            match &s.data {
                SeriesData::Xyz(p) => {
                    let ps = s.point_size.max(2);
                    chart
                        .draw_series(p.iter().filter(|(a, b, c)| a.is_finite() && b.is_finite() && c.is_finite()).map(|&(a, b, c)| Circle::new((a, c, b), ps, rc.filled())))
                        .map_err(pl)?;
                }
                SeriesData::Surface { x0, x1, y0, y1, cols, rows, z: g } => {
                    let (nx, ny) = (*cols, *rows);
                    let dx = if nx > 1 { (x1 - x0) / (nx - 1) as f64 } else { 1.0 };
                    let dy = if ny > 1 { (y1 - y0) / (ny - 1) as f64 } else { 1.0 };
                    let xs: Vec<f64> = (0..nx).map(|i| x0 + i as f64 * dx).collect();
                    let ys: Vec<f64> = (0..ny).map(|j| y0 + j as f64 * dy).collect();
                    let value = |xv: &f64, yv: &f64| -> f64 {
                        let i = (((xv - x0) / dx).round().max(0.0) as usize).min(nx - 1);
                        let j = (((yv - y0) / dy).round().max(0.0) as usize).min(ny - 1);
                        let v = g[j * nx + i];
                        if v.is_finite() {
                            v
                        } else {
                            z.0
                        }
                    };
                    chart
                        .draw_series(SurfaceSeries::xoz(xs.into_iter(), ys.into_iter(), |a: f64, b: f64| value(&a, &b)).style_func(&|v: &f64| {
                            let t = (v - z.0) / (z.1 - z.0).max(1e-12);
                            col(colormap(t)).mix(0.85).filled()
                        }))
                        .map_err(pl)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Mode {
    Cartesian,
    Pie,
    Polar,
    ThreeD,
}

fn trim_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.3}").trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colormap_runs_from_dark_purple_to_yellow() {
        assert_eq!(colormap(0.0), Rgba::rgb(68, 1, 84));
        assert_eq!(colormap(1.0), Rgba::rgb(253, 231, 37));
        assert_eq!(colormap(f64::NAN), colormap(0.0));
        assert_eq!(colormap(7.0), colormap(1.0));
    }

    #[test]
    fn nice_steps() {
        assert_eq!(nice_step(100.0, 4.0), 20.0);
        assert_eq!(nice_step(7.0, 4.0), 2.0);
        assert_eq!(nice_step(0.0, 4.0), 1.0);
        assert_eq!(nice_step(f64::NAN, 4.0), 1.0);
        assert!((nice_step(0.3, 3.0) - 0.1).abs() < 1e-12);
    }

    #[test]
    fn numbers_print_without_noise() {
        assert_eq!(trim_num(20.0), "20");
        assert_eq!(trim_num(0.25), "0.25");
        assert_eq!(trim_num(0.1 + 0.2), "0.3");
    }
}
