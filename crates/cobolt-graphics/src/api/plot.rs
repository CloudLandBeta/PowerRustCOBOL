// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The methods of Plot (WASM-018 R12).

use super::{dispatch, done, method};
use crate::error::Result;
use crate::plot::{parse_columns, Plot, SeriesKind, TableSpec};
use crate::registry::{MethodInfo, ObjectKind, Registry};
use crate::value::Value;

fn range(a: &crate::value::Args<'_>) -> Result<Option<(f64, f64)>> {
    let (lo, hi) = (a.num(0)?, a.num(1)?);
    if lo >= hi {
        return Err(a.refuse(1, "the top of a range must be above its bottom"));
    }
    Ok(Some((lo, hi)))
}

#[allow(unused_variables)]
pub(crate) static METHODS: &[crate::value::Method<Plot>] = &[
    method!(Plot, "SetTitle", [text: Text], [], Nothing, "The title above the chart.", |o, r, a| {
        o.options.title = a.text(0)?.to_string();
        done()
    }),
    method!(Plot, "SetXLabel", [text: Text], [], Nothing, "The label of the x axis.", |o, r, a| {
        o.options.x_label = a.text(0)?.to_string();
        done()
    }),
    method!(Plot, "SetYLabel", [text: Text], [], Nothing, "The label of the y axis.", |o, r, a| {
        o.options.y_label = a.text(0)?.to_string();
        done()
    }),
    method!(Plot, "ShowLegend", [on: Flag], [], Nothing, "Draw the legend of the named series (1) or not (0).", |o, r, a| {
        o.options.legend = a.flag(0)?;
        done()
    }),
    method!(Plot, "ShowGrid", [on: Flag], [], Nothing, "Draw grid lines (1) or not (0).", |o, r, a| {
        o.options.grid = a.flag(0)?;
        done()
    }),
    method!(Plot, "SetBackground", [color: Color], [], Nothing, "The background colour.", |o, r, a| {
        o.options.background = a.color(0)?;
        done()
    }),
    method!(Plot, "SetForeground", [color: Color], [], Nothing, "The colour of axes and text.", |o, r, a| {
        o.options.foreground = a.color(0)?;
        done()
    }),
    method!(Plot, "SetFont", [family: Text], [size: Num], Nothing, "The CSS font family of every label and its size in pixels.", |o, r, a| {
        o.options.font_family = a.text(0)?.to_string();
        if a.has(1) {
            let s = a.f32(1)?;
            if !(6.0..=200.0).contains(&s) {
                return Err(a.refuse(1, "a font size is from 6 to 200 pixels"));
            }
            o.options.font_size = s;
        }
        done()
    }),
    method!(Plot, "SetXRange", [min: Num, max: Num], [], Nothing, "Fix the x range (otherwise it fits the data).", |o, r, a| {
        o.options.x_range = range(a)?;
        done()
    }),
    method!(Plot, "SetYRange", [min: Num, max: Num], [], Nothing, "Fix the y range (the z range of a 3D plot, the radius of a polar one).", |o, r, a| {
        o.options.y_range = range(a)?;
        done()
    }),
    method!(Plot, "ClearRanges", [], [], Nothing, "Back to fitting the data.", |o, r, a| {
        o.options.x_range = None;
        o.options.y_range = None;
        done()
    }),
    method!(Plot, "AddCategory", [name: Text], [], Nothing, "Name the next bar category (x = 0, 1, 2 ...).", |o, r, a| {
        if o.options.categories.len() >= 10_000 {
            return Err(a.refuse(0, "a plot has at most 10,000 categories"));
        }
        o.options.categories.push(a.text(0)?.to_string());
        done()
    }),
    method!(Plot, "ClearCategories", [], [], Nothing, "Forget the category names.", |o, r, a| {
        o.options.categories.clear();
        done()
    }),
    method!(Plot, "SetHistogramBins", [bins: Int], [], Nothing, "Bins of a histogram (0 chooses by Sturges' rule).", |o, r, a| {
        o.options.histogram_bins = a.int_in(0, 0, 10_000)? as u32;
        done()
    }),
    method!(Plot, "SetView3D", [yaw: Num, pitch: Num], [], Nothing, "The viewing angles of a 3D plot in degrees.", |o, r, a| {
        o.options.yaw = a.num(0)?.to_radians();
        o.options.pitch = a.num(1)?.to_radians();
        done()
    }),
    method!(Plot, "AddSeries", [kind: Text, name: Text], [], Handle, "A new series: LINE, BAR, SCATTER, AREA, PIE, HISTOGRAM, CANDLESTICK, HEATMAP, POLAR, ERRORBARS, SURFACE3D or SCATTER3D. The handle names it in later calls.", |o, r, a| {
        let k = SeriesKind::from_name(a.text(0)?).ok_or_else(|| a.refuse(0, format!("\"{}\" is not a series kind ({})", a.text(0).unwrap_or(""), SeriesKind::ALL.iter().map(|k| k.name()).collect::<Vec<_>>().join(", "))))?;
        Ok(o.add_series(k, a.text(1)?)?.into())
    }),
    method!(Plot, "RemoveSeries", [series: Handle], [], Nothing, "Remove a series (its handle is released).", |o, r, a| {
        o.remove_series(a.handle(0)?)?;
        done()
    }),
    method!(Plot, "ClearSeries", [series: Handle], [], Nothing, "Empty a series, keeping its settings.", |o, r, a| {
        o.clear_series(a.handle(0)?)?;
        done()
    }),
    method!(Plot, "GetSeriesCount", [], [], Int, "How many series there are.", |o, r, a| Ok(o.series_count().into())),
    method!(Plot, "GetSeriesPoints", [series: Handle], [], Int, "How many points (values, candles, cells) a series holds.", |o, r, a| Ok(o.series(a.handle(0)?)?.points().into())),
    method!(Plot, "SetSeriesName", [series: Handle, name: Text], [], Nothing, "The legend text of a series.", |o, r, a| {
        o.series_mut(a.handle(0)?)?.name = a.text(1)?.to_string();
        done()
    }),
    method!(Plot, "SetSeriesColor", [series: Handle, color: Color], [], Nothing, "The colour of a series (transparent takes one from the palette).", |o, r, a| {
        o.series_mut(a.handle(0)?)?.color = a.color(1)?;
        done()
    }),
    method!(Plot, "SetSeriesLineWidth", [series: Handle, width: Int], [], Nothing, "Line width in pixels.", |o, r, a| {
        o.series_mut(a.handle(0)?)?.line_width = a.int_in(1, 0, 100)? as u32;
        done()
    }),
    method!(Plot, "SetSeriesPointSize", [series: Handle, size: Int], [], Nothing, "Marker radius in pixels (0 draws none on a line).", |o, r, a| {
        o.series_mut(a.handle(0)?)?.point_size = a.int_in(1, 0, 100)? as u32;
        done()
    }),
    method!(Plot, "SetSeriesClosed", [series: Handle, closed: Flag], [], Nothing, "Close a polar line back to its first point.", |o, r, a| {
        o.series_mut(a.handle(0)?)?.closed = a.flag(1)?;
        done()
    }),
    method!(Plot, "AddPoint", [series: Handle, x: Num, y: Num], [], Nothing, "Add an (x, y) point to a line, scatter, area or polar series (polar: angle in degrees, radius).", |o, r, a| {
        o.add_point(a.handle(0)?, a.num(1)?, a.num(2)?)?;
        done()
    }),
    method!(Plot, "AddPoint3", [series: Handle, x: Num, y: Num, z: Num], [], Nothing, "Add an (x, y, z) point to a 3D scatter series.", |o, r, a| {
        o.add_point3(a.handle(0)?, a.num(1)?, a.num(2)?, a.num(3)?)?;
        done()
    }),
    method!(Plot, "AddValue", [series: Handle, label: Text, value: Num], [], Nothing, "Add a value (with a label) to a bar, pie or histogram series.", |o, r, a| {
        o.add_value(a.handle(0)?, a.text(1)?, a.num(2)?)?;
        done()
    }),
    method!(Plot, "AddCandle", [series: Handle, x: Num, open: Num, high: Num, low: Num, close: Num], [], Nothing, "Add a candle to a candlestick series.", |o, r, a| {
        o.add_candle(a.handle(0)?, a.num(1)?, a.num(2)?, a.num(3)?, a.num(4)?, a.num(5)?)?;
        done()
    }),
    method!(Plot, "AddErrorPoint", [series: Handle, x: Num, y: Num, error: Num], [], Nothing, "Add a point with a symmetric error to an error-bar series.", |o, r, a| {
        o.add_error_point(a.handle(0)?, a.num(1)?, a.num(2)?, a.num(3)?)?;
        done()
    }),
    method!(Plot, "SetSeriesGrid", [series: Handle, columns: Int, rows: Int, values: Floats], [], Nothing, "Fill a heat map with a rows x columns grid of values, row by row.", |o, r, a| {
        o.set_grid(a.handle(0)?, a.usize(1)?, a.usize(2)?, &a.floats(3)?)?;
        done()
    }),
    method!(Plot, "SetSeriesSurface", [series: Handle, x0: Num, x1: Num, y0: Num, y1: Num, columns: Int, rows: Int, values: Floats], [], Nothing, "Fill a 3D surface: z over a rectangle, sampled on a rows x columns grid.", |o, r, a| {
        o.set_surface(a.handle(0)?, a.num(1)?, a.num(2)?, a.num(3)?, a.num(4)?, a.usize(5)?, a.usize(6)?, &a.floats(7)?)?;
        done()
    }),
    method!(Plot, "SetSeriesFromTable", [series: Handle, block: Bytes, count: Int, stride: Int, columns: Text], [], Int, "Fill a series from a COBOL table passed as one block: the row count, the bytes per row and where each number is and how it is stored, e.g. \"0:zoned(6,0); 6:packed(7,2,s)\". The answer is how many rows were skipped for holding an unreadable number.", |o, r, a| {
        let spec = TableSpec { count: a.usize(2)?, stride: a.usize(3)?, columns: parse_columns(a.text(4)?)? };
        Ok(o.set_series_from_table(a.handle(0)?, a.bytes(1)?, &spec)?.into())
    }),
    method!(Plot, "Render", [width: Int, height: Int], [], Handle, "Draw the chart into a new ImageBuffer.", |o, r, a| {
        let engine = r.text_engine();
        let px = o.render(a.u32(0)?, a.u32(1)?, engine.as_deref())?;
        Ok(r.add_image(px)?.into())
    }),
    method!(Plot, "RenderToCanvas", [canvas: Handle, x: Num, y: Num, width: Int, height: Int], [], Nothing, "Record the chart into a Canvas2D as vector commands, with its corner at (x, y).", |o, r, a| {
        let engine = r.text_engine();
        let (x, y, w, h) = (a.num(1)?, a.num(2)?, a.u32(3)?, a.u32(4)?);
        let plot: &Plot = o;
        r.with_canvas2d(a.handle(0)?, |c, _| {
            c.list().save().translate(x as f32, y as f32);
            let out = plot.render_into(c.list(), w, h, engine.as_deref());
            c.list().restore();
            out
        })?;
        done()
    }),
    method!(Plot, "ToSvg", [width: Int, height: Int], [], Text, "The chart as SVG text.", |o, r, a| {
        let engine = r.text_engine();
        Ok(o.to_svg(a.u32(0)?, a.u32(1)?, engine.as_deref())?.into())
    }),
];

pub(crate) fn call(o: &mut Plot, reg: &mut Registry, method: &str, args: &[Value]) -> Result<Value> {
    dispatch(METHODS, "Plot", o, reg, method, args)
}

pub(crate) fn infos() -> Vec<MethodInfo> {
    super::infos(ObjectKind::Plot, METHODS)
}
