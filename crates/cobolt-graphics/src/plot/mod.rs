// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Plot (WASM-018 R12): charts for a COBOL program.
//!
//! A [`Plot`] holds titles, axes and **series**. A series is made with
//! [`Plot::add_series`] (a handle comes back, WASM-018 R2), filled point by point
//! (`AddPoint`) or **from a COBOL table passed as one block**
//! ([`Plot::set_series_from_table`]: the count, the stride and the numeric kind are
//! described once - see [`table`] - and a hundred thousand values are read in one
//! pass), and drawn with [`Plot::render`] into pixels (an `ImageBuffer`, or a
//! `Canvas2D` through its image), or with [`Plot::to_svg`] into SVG text.
//!
//! The charts are laid out by [`plotters`] and recorded by backends of this crate
//! ([`backend`]) as display-list commands - so the canvas rasteriser draws them,
//! in parallel bands when large - or written as SVG, with this crate's text engine
//! measuring every label: a chart is the same on every platform and its text is
//! shaped for all six languages. [`Plot::render_into`] puts a chart straight into
//! a `Canvas2D`'s list as vector commands.
//!
//! | `Kind` | what it draws | data |
//! |---|---|---|
//! | `LINE` | polyline | points (x, y) |
//! | `BAR` | vertical bars, grouped when several series | values, optional category names |
//! | `SCATTER` | markers | points |
//! | `AREA` | filled area under a line | points |
//! | `PIE` | pie with percentages | values with labels |
//! | `HISTOGRAM` | counts of raw values per bin | values |
//! | `CANDLESTICK` | open-high-low-close | rows (x, open, high, low, close) |
//! | `HEATMAP` | coloured cells | a rows x columns grid |
//! | `POLAR` | line on a polar grid | points (angle in degrees, radius) |
//! | `ERRORBARS` | y with +/- error | rows (x, y, error) |
//! | `SURFACE3D` | shaded surface z = f(x, y) | a grid over a rectangle |
//! | `SCATTER3D` | markers in 3D | points (x, y, z) |
//!
//! A plot is one *family*: Cartesian (any mix of line, bar, scatter, area,
//! histogram, candlestick, error bars and heatmap), pie, polar or 3D. Mixing families is
//! refused with a message, not silently dropped.
//!
//! # Differences from plotters, and why
//!
//! `plotters` is used as a layout and series library only. Its own font layer
//! needs C libraries (`font-kit`) on desktop and measures text through the
//! browser's DOM on `wasm32`; neither is acceptable (WASM-018 R4, R20), so every
//! backend here measures and draws text with [`TextEngine`].

pub mod backend;
mod render;
pub mod table;

use plotters::prelude::IntoDrawingArea;

pub use backend::{ListBackend, SvgPlotBackend};
pub use table::{parse_columns, read_table, Column, NumKind, Table, TableSpec, ZonedSign};

use std::sync::Arc;

use crate::canvas::{Surface, SurfaceConfig};
use crate::color::Rgba;
use crate::display_list::DisplayListBuilder;
use crate::error::{GraphicsError, Result};
use crate::handles::{Handle, HandleTable};
use crate::limits::Limits;
use crate::pixels::PixelBuffer;
use crate::resources::{NoResources, Resources};
use crate::textapi::TextEngine;

/// A [`Resources`] that has fonts and nothing else.
struct TextOnly<'a>(&'a dyn TextEngine);

impl Resources for TextOnly<'_> {
    fn image(&self, _id: u32) -> Option<Arc<PixelBuffer>> {
        None
    }

    fn text_engine(&self) -> Option<&dyn TextEngine> {
        Some(self.0)
    }
}

/// The kind of a series.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SeriesKind {
    /// Polyline through points.
    Line,
    /// Vertical bars.
    Bar,
    /// Markers.
    Scatter,
    /// Filled area.
    Area,
    /// Pie chart.
    Pie,
    /// Histogram of raw values.
    Histogram,
    /// Open-high-low-close candles.
    Candlestick,
    /// Heat map of a grid.
    Heatmap,
    /// Line on a polar grid.
    Polar,
    /// Points with error bars.
    ErrorBars,
    /// 3D surface.
    Surface3d,
    /// 3D scatter.
    Scatter3d,
}

impl SeriesKind {
    /// Every kind.
    pub const ALL: [SeriesKind; 12] = [
        Self::Line,
        Self::Bar,
        Self::Scatter,
        Self::Area,
        Self::Pie,
        Self::Histogram,
        Self::Candlestick,
        Self::Heatmap,
        Self::Polar,
        Self::ErrorBars,
        Self::Surface3d,
        Self::Scatter3d,
    ];

    /// The COBOL name (`LINE`, `SURFACE3D`, ...).
    pub fn name(self) -> &'static str {
        match self {
            Self::Line => "LINE",
            Self::Bar => "BAR",
            Self::Scatter => "SCATTER",
            Self::Area => "AREA",
            Self::Pie => "PIE",
            Self::Histogram => "HISTOGRAM",
            Self::Candlestick => "CANDLESTICK",
            Self::Heatmap => "HEATMAP",
            Self::Polar => "POLAR",
            Self::ErrorBars => "ERRORBARS",
            Self::Surface3d => "SURFACE3D",
            Self::Scatter3d => "SCATTER3D",
        }
    }

    /// From the COBOL name, case-insensitive.
    pub fn from_name(name: &str) -> Option<Self> {
        let n = name.trim().to_ascii_uppercase().replace(['-', '_', ' '], "");
        Self::ALL.iter().copied().find(|k| k.name() == n)
    }
}

/// What the series holds.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SeriesData {
    Xy(Vec<(f64, f64)>),
    Xyz(Vec<(f64, f64, f64)>),
    Values { values: Vec<f64>, labels: Vec<String> },
    Candles(Vec<[f64; 5]>),
    Errors(Vec<(f64, f64, f64)>),
    Grid { cols: usize, rows: usize, z: Vec<f64> },
    Surface { x0: f64, x1: f64, y0: f64, y1: f64, cols: usize, rows: usize, z: Vec<f64> },
}

/// One series.
#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    /// The kind.
    pub kind: SeriesKind,
    /// Legend text; empty for none.
    pub name: String,
    /// Colour; transparent picks one from the palette.
    pub color: Rgba,
    /// Line width in pixels.
    pub line_width: u32,
    /// Marker radius in pixels (0 draws none on a line).
    pub point_size: u32,
    /// Close a polar line back to its first point.
    pub closed: bool,
    pub(crate) data: SeriesData,
}

impl Series {
    /// How many points (or values, candles, grid cells) the series holds.
    pub fn points(&self) -> usize {
        match &self.data {
            SeriesData::Xy(p) => p.len(),
            SeriesData::Xyz(p) => p.len(),
            SeriesData::Values { values, .. } => values.len(),
            SeriesData::Candles(p) => p.len(),
            SeriesData::Errors(p) => p.len(),
            SeriesData::Grid { z, .. } | SeriesData::Surface { z, .. } => z.len(),
        }
    }
}

/// The look of a plot as a whole.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotOptions {
    /// Title above the chart.
    pub title: String,
    /// Label of the x axis.
    pub x_label: String,
    /// Label of the y axis.
    pub y_label: String,
    /// Draw the legend when a series has a name.
    pub legend: bool,
    /// Draw grid lines.
    pub grid: bool,
    /// Background.
    pub background: Rgba,
    /// Axes and text.
    pub foreground: Rgba,
    /// CSS font family for every label.
    pub font_family: String,
    /// Label size in pixels (the title is larger).
    pub font_size: f32,
    /// Fixed x range; `None` fits the data.
    pub x_range: Option<(f64, f64)>,
    /// Fixed y range (the z range of a 3D plot, the radius of a polar one).
    pub y_range: Option<(f64, f64)>,
    /// Names of the bar categories (x = 0, 1, 2 ...).
    pub categories: Vec<String>,
    /// Histogram bins; 0 chooses by Sturges' rule.
    pub histogram_bins: u32,
    /// 3D: rotation about the vertical axis, radians.
    pub yaw: f64,
    /// 3D: tilt, radians.
    pub pitch: f64,
}

impl Default for PlotOptions {
    fn default() -> Self {
        Self {
            title: String::new(),
            x_label: String::new(),
            y_label: String::new(),
            legend: true,
            grid: true,
            background: Rgba::WHITE,
            foreground: Rgba::rgb(40, 40, 40),
            font_family: "sans-serif".into(),
            font_size: 14.0,
            x_range: None,
            y_range: None,
            categories: Vec::new(),
            histogram_bins: 0,
            yaw: 0.5,
            pitch: 0.35,
        }
    }
}

/// A chart.
#[derive(Debug)]
pub struct Plot {
    /// Titles, axes and colours.
    pub options: PlotOptions,
    pub(crate) series: HandleTable<Series>,
    limits: Limits,
}

impl Plot {
    /// An empty plot.
    pub fn new(limits: Limits) -> Self {
        Self { options: PlotOptions::default(), series: HandleTable::new("series"), limits }
    }

    /// Add a series; the handle names it in every later call.
    pub fn add_series(&mut self, kind: SeriesKind, name: &str) -> Result<Handle> {
        let data = match kind {
            SeriesKind::Line | SeriesKind::Scatter | SeriesKind::Area | SeriesKind::Polar => SeriesData::Xy(Vec::new()),
            SeriesKind::Bar | SeriesKind::Pie | SeriesKind::Histogram => SeriesData::Values { values: Vec::new(), labels: Vec::new() },
            SeriesKind::Candlestick => SeriesData::Candles(Vec::new()),
            SeriesKind::ErrorBars => SeriesData::Errors(Vec::new()),
            SeriesKind::Heatmap => SeriesData::Grid { cols: 0, rows: 0, z: Vec::new() },
            SeriesKind::Surface3d => SeriesData::Surface { x0: 0.0, x1: 1.0, y0: 0.0, y1: 1.0, cols: 0, rows: 0, z: Vec::new() },
            SeriesKind::Scatter3d => SeriesData::Xyz(Vec::new()),
        };
        self.series.insert(Series { kind, name: name.to_string(), color: Rgba::TRANSPARENT, line_width: 2, point_size: if kind == SeriesKind::Scatter { 3 } else { 0 }, closed: false, data })
    }

    /// Remove a series (its handle is released).
    pub fn remove_series(&mut self, series: Handle) -> Result<()> {
        self.series.remove(series).map(|_| ())
    }

    /// The series a handle names, for reading its settings.
    pub fn series(&self, series: Handle) -> Result<&Series> {
        self.series.get(series)
    }

    /// The series a handle names, for changing its colour, width and so on.
    pub fn series_mut(&mut self, series: Handle) -> Result<&mut Series> {
        self.series.get_mut(series)
    }

    /// How many series are live.
    pub fn series_count(&self) -> usize {
        self.series.len()
    }

    /// About how much memory the series hold, for the registry's accounting.
    pub fn memory_bytes(&self) -> u64 {
        self.series.iter().map(|(_, s)| s.points() as u64 * 40).sum::<u64>() + 1024
    }

    /// Empty a series (its settings stay).
    pub fn clear_series(&mut self, series: Handle) -> Result<()> {
        let s = self.series.get_mut(series)?;
        s.data = match &s.data {
            SeriesData::Xy(_) => SeriesData::Xy(Vec::new()),
            SeriesData::Xyz(_) => SeriesData::Xyz(Vec::new()),
            SeriesData::Values { .. } => SeriesData::Values { values: Vec::new(), labels: Vec::new() },
            SeriesData::Candles(_) => SeriesData::Candles(Vec::new()),
            SeriesData::Errors(_) => SeriesData::Errors(Vec::new()),
            SeriesData::Grid { .. } => SeriesData::Grid { cols: 0, rows: 0, z: Vec::new() },
            SeriesData::Surface { .. } => SeriesData::Surface { x0: 0.0, x1: 1.0, y0: 0.0, y1: 1.0, cols: 0, rows: 0, z: Vec::new() },
        };
        Ok(())
    }

    fn room(&self, s: &Series, more: usize) -> Result<()> {
        self.limits.check_series_points(s.points() as u64 + more as u64)
    }

    fn wrong(&self, series: Handle, what: &str) -> GraphicsError {
        let kind = self.series.get(series).map(|s| s.kind.name()).unwrap_or("?");
        GraphicsError::invalid(format!("a {kind} series does not take {what}"))
    }

    /// `AddPoint`: one (x, y) point of a line, scatter, area or polar series (for
    /// a polar series x is the angle in degrees and y the radius).
    pub fn add_point(&mut self, series: Handle, x: f64, y: f64) -> Result<()> {
        let s = self.series.get(series)?;
        self.room(s, 1)?;
        match &mut self.series.get_mut(series)?.data {
            SeriesData::Xy(p) => {
                p.push((x, y));
                Ok(())
            }
            _ => Err(self.wrong(series, "AddPoint")),
        }
    }

    /// `AddPoint3`: one (x, y, z) point of a 3D scatter series.
    pub fn add_point3(&mut self, series: Handle, x: f64, y: f64, z: f64) -> Result<()> {
        let s = self.series.get(series)?;
        self.room(s, 1)?;
        match &mut self.series.get_mut(series)?.data {
            SeriesData::Xyz(p) => {
                p.push((x, y, z));
                Ok(())
            }
            _ => Err(self.wrong(series, "AddPoint3")),
        }
    }

    /// `AddValue`: one value (with a label for a pie) of a bar, pie or histogram series.
    pub fn add_value(&mut self, series: Handle, label: &str, value: f64) -> Result<()> {
        let s = self.series.get(series)?;
        self.room(s, 1)?;
        match &mut self.series.get_mut(series)?.data {
            SeriesData::Values { values, labels } => {
                values.push(value);
                labels.push(label.to_string());
                Ok(())
            }
            _ => Err(self.wrong(series, "AddValue")),
        }
    }

    /// `AddCandle`: one open-high-low-close row at `x`.
    pub fn add_candle(&mut self, series: Handle, x: f64, open: f64, high: f64, low: f64, close: f64) -> Result<()> {
        let s = self.series.get(series)?;
        self.room(s, 1)?;
        match &mut self.series.get_mut(series)?.data {
            SeriesData::Candles(p) => {
                p.push([x, open, high, low, close]);
                Ok(())
            }
            _ => Err(self.wrong(series, "AddCandle")),
        }
    }

    /// `AddErrorPoint`: a point with its +/- error.
    pub fn add_error_point(&mut self, series: Handle, x: f64, y: f64, error: f64) -> Result<()> {
        let s = self.series.get(series)?;
        self.room(s, 1)?;
        match &mut self.series.get_mut(series)?.data {
            SeriesData::Errors(p) => {
                p.push((x, y, error));
                Ok(())
            }
            _ => Err(self.wrong(series, "AddErrorPoint")),
        }
    }

    /// `SetGrid`: the cells of a heat map, `rows` x `cols` values row by row.
    pub fn set_grid(&mut self, series: Handle, cols: usize, rows: usize, values: &[f64]) -> Result<()> {
        if cols == 0 || rows == 0 || cols.checked_mul(rows) != Some(values.len()) {
            return Err(GraphicsError::invalid(format!("a grid of {rows} rows x {cols} columns needs {} value(s), but {} were given", rows.saturating_mul(cols), values.len())));
        }
        self.limits.check_series_points(values.len() as u64)?;
        match &mut self.series.get_mut(series)?.data {
            SeriesData::Grid { cols: c, rows: r, z } => {
                *c = cols;
                *r = rows;
                *z = values.to_vec();
                Ok(())
            }
            _ => Err(self.wrong(series, "SetGrid")),
        }
    }

    /// `SetSurface`: the heights of a 3D surface over the rectangle `x0..x1` by
    /// `y0..y1`, `rows` x `cols` values row by row.
    #[allow(clippy::too_many_arguments)]
    pub fn set_surface(&mut self, series: Handle, x0: f64, x1: f64, y0: f64, y1: f64, cols: usize, rows: usize, values: &[f64]) -> Result<()> {
        if cols == 0 || rows == 0 || cols.checked_mul(rows) != Some(values.len()) {
            return Err(GraphicsError::invalid(format!("a surface of {rows} rows x {cols} columns needs {} value(s), but {} were given", rows.saturating_mul(cols), values.len())));
        }
        if !(x0.is_finite() && x1.is_finite() && y0.is_finite() && y1.is_finite()) || x0 == x1 && cols > 1 || y0 == y1 && rows > 1 {
            return Err(GraphicsError::invalid("the surface rectangle must have a width and a height"));
        }
        self.limits.check_series_points(values.len() as u64)?;
        match &mut self.series.get_mut(series)?.data {
            SeriesData::Surface { x0: a, x1: b, y0: c, y1: d, cols: nc, rows: nr, z } => {
                (*a, *b, *c, *d, *nc, *nr) = (x0, x1, y0, y1, cols, rows);
                *z = values.to_vec();
                Ok(())
            }
            _ => Err(self.wrong(series, "SetSurface")),
        }
    }

    /// `SetSeriesFromTable`: fill a series from a COBOL table block in one pass.
    ///
    /// How the columns are used depends on the kind: **Line, Scatter, Area, Polar**:
    /// one column is y with x = 0, 1, 2 ..., two columns are (x, y). **Bar, Pie,
    /// Histogram**: one column of values. **Candlestick**: five columns (x, open,
    /// high, low, close). **ErrorBars**: three (x, y, error). **Scatter3d**: three
    /// (x, y, z). Rows with a value that cannot be decoded are skipped; the number of
    /// skipped values is returned.
    pub fn set_series_from_table(&mut self, series: Handle, block: &[u8], spec: &TableSpec) -> Result<usize> {
        let kind = self.series.get(series)?.kind;
        let need: &[usize] = match kind {
            SeriesKind::Line | SeriesKind::Scatter | SeriesKind::Area | SeriesKind::Polar => &[1, 2],
            SeriesKind::Bar | SeriesKind::Pie | SeriesKind::Histogram => &[1],
            SeriesKind::Candlestick => &[5],
            SeriesKind::ErrorBars | SeriesKind::Scatter3d => &[3],
            SeriesKind::Heatmap | SeriesKind::Surface3d => {
                return Err(GraphicsError::invalid(format!("a {} series takes a grid: use SetGrid or SetSurface", kind.name())));
            }
        };
        if !need.contains(&spec.columns.len()) {
            return Err(GraphicsError::invalid(format!("a {} series takes {} column(s) per row, but the table spec names {}", kind.name(), need.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(" or "), spec.columns.len())));
        }
        let t = read_table(block, spec, &self.limits)?;
        let rows = spec.count;
        let ok = |r: usize| t.columns.iter().all(|c| c[r].is_finite());
        let mut skipped = 0usize;
        let data = match kind {
            SeriesKind::Line | SeriesKind::Scatter | SeriesKind::Area | SeriesKind::Polar => {
                let mut p = Vec::with_capacity(rows);
                for r in 0..rows {
                    if !ok(r) {
                        skipped += 1;
                        continue;
                    }
                    p.push(if t.columns.len() == 1 { (r as f64, t.columns[0][r]) } else { (t.columns[0][r], t.columns[1][r]) });
                }
                SeriesData::Xy(p)
            }
            SeriesKind::Bar | SeriesKind::Pie | SeriesKind::Histogram => {
                let mut v = Vec::with_capacity(rows);
                for r in 0..rows {
                    if ok(r) {
                        v.push(t.columns[0][r]);
                    } else {
                        skipped += 1;
                    }
                }
                let n = v.len();
                SeriesData::Values { values: v, labels: vec![String::new(); n] }
            }
            SeriesKind::Candlestick => {
                let mut v = Vec::with_capacity(rows);
                for r in 0..rows {
                    if ok(r) {
                        v.push([t.columns[0][r], t.columns[1][r], t.columns[2][r], t.columns[3][r], t.columns[4][r]]);
                    } else {
                        skipped += 1;
                    }
                }
                SeriesData::Candles(v)
            }
            SeriesKind::ErrorBars => {
                let mut v = Vec::with_capacity(rows);
                for r in 0..rows {
                    if ok(r) {
                        v.push((t.columns[0][r], t.columns[1][r], t.columns[2][r]));
                    } else {
                        skipped += 1;
                    }
                }
                SeriesData::Errors(v)
            }
            SeriesKind::Scatter3d => {
                let mut v = Vec::with_capacity(rows);
                for r in 0..rows {
                    if ok(r) {
                        v.push((t.columns[0][r], t.columns[1][r], t.columns[2][r]));
                    } else {
                        skipped += 1;
                    }
                }
                SeriesData::Xyz(v)
            }
            SeriesKind::Heatmap | SeriesKind::Surface3d => unreachable!("refused above"),
        };
        self.series.get_mut(series)?.data = data;
        Ok(skipped)
    }

    fn check_size(&self, width: u32, height: u32) -> Result<()> {
        if width < 40 || height < 40 {
            return Err(GraphicsError::invalid("a plot needs at least 40 x 40 pixels"));
        }
        self.limits.check_canvas(width, height, 1.0)
    }

    /// Record the chart as display-list commands, `width` x `height` pixels with its
    /// top-left corner at the list's current origin (`Translate` first to place it
    /// elsewhere on a canvas). The state is saved and restored around it.
    pub fn render_into(&self, list: &mut DisplayListBuilder, width: u32, height: u32, text: Option<&dyn TextEngine>) -> Result<()> {
        self.check_size(width, height)?;
        list.save();
        {
            let backend = ListBackend::new(list, (width, height), text);
            let root = backend.into_drawing_area();
            self.draw(&root)?;
        }
        list.restore();
        Ok(())
    }

    /// Draw the chart into new pixels (what an `ImageBuffer` or a canvas takes). The
    /// chart is recorded as commands and drawn by the canvas rasteriser, so large
    /// charts are rasterised in parallel bands like any large canvas.
    pub fn render(&self, width: u32, height: u32, text: Option<&dyn TextEngine>) -> Result<PixelBuffer> {
        let mut list = DisplayListBuilder::with_limits(self.limits.clone());
        self.render_into(&mut list, width, height, text)?;
        list.present();
        let bytes = list.take()?;
        let mut surface = Surface::new(&SurfaceConfig::new(width, height), &self.limits)?;
        match text {
            Some(t) => surface.execute(&bytes, &TextOnly(t))?,
            None => surface.execute(&bytes, &NoResources)?,
        };
        Ok(surface.to_image())
    }

    /// Draw the chart as SVG text.
    pub fn to_svg(&self, width: u32, height: u32, text: Option<&dyn TextEngine>) -> Result<String> {
        self.check_size(width, height)?;
        let mut out = String::new();
        {
            let backend = SvgPlotBackend::with_string(&mut out, (width, height), text);
            let root = backend.into_drawing_area();
            self.draw(&root)?;
        }
        Ok(out)
    }
}
