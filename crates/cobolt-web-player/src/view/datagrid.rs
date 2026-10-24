// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The DataGrid as the desktop draws it** (WASM-016 R5: one look on every surface).
//!
//! The desktop painter (`cobolt-forms` `render.rs`, the DataGrid arm) puts a grid together from a caption row
//! (the CSV button and the `Title`), a band of column titles with the buttons that move a column, rows
//! with a tint on every other one, a hair line between columns, a frozen first column that casts a shadow
//! on what scrolls under it, and two pill-shaped scroll bars. This module builds the same thing from
//! elements, **with the same arithmetic**: the column widths, the row geometry and the scroll limits come
//! from `cobolt_forms::datagrid` (the very functions the painter calls), the text is placed where
//! `metrics` says the desktop's text layout would put it, and every colour and size below is read off the
//! painter's source, not guessed.
//!
//! The page scrolls the grid natively (wheel, touch, keyboard), so a scroll costs the page nothing but the
//! rows that come into view: the column titles and the frozen column are `position: sticky`, the scroll bars
//! are drawn from the offset the page last reported.
//!
//! What this port does not draw yet is listed in `specs/WASM-PARITY.md`: frozen rows, the column filter
//! row, and the cell kinds other than text.

use cobolt_forms::datagrid::{
    parse_row_height_overrides, parse_rows, resolve_column_widths, sort_datagrid_rows, DataGridColumnMeasure, DataGridLayout,
    DataGridLayoutInput, ROW_HEIGHT_OVERRIDES_PROP,
};
use cobolt_forms::fieldtext::format_cell_with_cobol_mask;
use cobolt_forms::model::{
    Control, ControlType, DataGridAdvanced, DataGridGridLineStyle, DataGridTextAlignment, DataGridWidthUnit, DEFAULT_BACKGROUND_COLOR,
    DEFAULT_FOREGROUND_COLOR,
};

use super::{VNode, ViewCtx};
use crate::metrics::{self, whole};
use crate::tree::Ctl;
use crate::value::{css_color, fmt_number, parse_color, parse_f64, truthy};

/// How many rows are drawn before and after the ones in view.
pub const OVERSCAN: usize = 12;

/// The scroll bar's track thickness and the shortest thumb it can be grabbed by (`render.rs`: `BAR`, `BAR_MIN`).
const BAR: f64 = 10.0;
const BAR_MIN: f64 = 28.0;
/// The soft shadow the frozen band casts: how far it reaches and how dark it starts (`frozen_shadow_shape`).
const SHADOW_TOP: f64 = 9.0;
const SHADOW_LEFT: f64 = 11.0;
/// The tint of a selected cell (`Color32::from_rgba_unmultiplied(80, 145, 255, 55)`).
const SELECTION: [u8; 4] = [80, 145, 255, 55];

/// The properties the painter reads through `DataGridAdvanced::from_control`, by their canonical spelling.
const ADVANCED_PROPS: [&str; 17] = [
    "AdvancedGrid",
    "Columns",
    "RowHeight",
    "FrozenColumns",
    "FrozenRows",
    "CSVExportMode",
    "CSVDelimiter",
    "GridLineStyle",
    "SelectableText",
    "ColumnFilters",
    "_RuntimeRowHeight",
    "_RuntimeFrozenColumns",
    "_RuntimeFrozenRows",
    "_RuntimeColumnFilters",
    "_RuntimeGridLineStyle",
    "_RuntimeColumnWidths",
    "_RuntimeColumnTitles",
];

fn px(v: f64) -> String {
    format!("{}px", fmt_number(v))
}

/// `over` laid on `under`, both unmultiplied, the way two translucent fills stack (`under` taken as opaque).
fn flatten(over: [u8; 4], under: [u8; 3]) -> [u8; 3] {
    let a = over[3] as f64 / 255.0;
    let mix = |o: u8, u: u8| (o as f64 * a + u as f64 * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
    [mix(over[0], under[0]), mix(over[1], under[1]), mix(over[2], under[2])]
}

fn rgb(c: [u8; 3]) -> String {
    css_color([c[0], c[1], c[2], 255])
}

/// How far a rounded rectangle of `radius` has cut in from its side at distance `d` along the top or bottom edge
/// (`render.rs`: `rounded_edge_inset`, `rounded_edge_inset_v`): the scroll bar's track stops short of the arc.
fn arc_inset(extent: f64, radius: f64, at: f64) -> f64 {
    let r = radius.min(extent * 0.5);
    if r <= 0.0 {
        return 0.0;
    }
    let d = at.min(extent - at);
    if d < 0.0 || d >= r {
        return 0.0;
    }
    let k = r - d;
    r - (r * r - k * k).max(0.0).sqrt()
}

/// What the painter's `prop_bool(ctrl, name, default)` answers.
fn flag(c: &Ctl, name: &str, default: bool) -> bool {
    c.raw(name).map(truthy).unwrap_or(default)
}

/// The control as the painter's `DataGridAdvanced::from_control` reads it: a model control that holds the
/// stored properties and nothing else (a control loaded from a form has no seeded defaults).
fn model_control(c: &Ctl) -> Control {
    let mut m = Control::new(c.name.clone(), ControlType::DataGrid, 0, 0);
    m.properties.clear();
    for name in ADVANCED_PROPS {
        if let Some(v) = c.raw(name) {
            m.set_prop(name, v);
        }
    }
    m
}

// ── what the grid is made of ────────────────────────────────────────────────

/// One column as it is drawn.
#[derive(Debug, Clone)]
pub struct Col {
    /// The column's place among those shown (what an event calls the column).
    pub display: usize,
    /// Where its value is in a row (`usize::MAX`: a column with no data behind it).
    pub source: usize,
    pub title: String,
    pub ty: String,
    /// Where it starts in the sheet, and how wide it is.
    pub x: f64,
    pub w: f64,
    pub frozen: bool,
    pub align: DataGridTextAlignment,
    pub font: f64,
    pub header_font: f64,
    pub mask: String,
}

/// Every colour the grid is painted in.
#[derive(Debug, Clone)]
struct Look {
    header_bg: [u8; 4],
    header_fg: [u8; 4],
    cell_fg: [u8; 4],
    line: [u8; 4],
    /// The grid's own colour as the painter lays it under each cell, faded by the control's `Transparency`.
    underlay: Option<[u8; 4]>,
    alt: [u8; 4],
    alt_rows: bool,
    alt_cols: bool,
    /// How much of what is behind the grid its face lets through is `1 - this`.
    face_opacity: f64,
}

/// The whole picture: sizes, columns, rows to show and where each goes.
pub struct Grid {
    pub w: f64,
    pub h: f64,
    pub row_h: f64,
    pub font: f64,
    pub gutter: f64,
    pub number_font: f64,
    pub caption_h: f64,
    pub header_h: f64,
    pub band_h: f64,
    pub radius: f64,
    pub cols: Vec<Col>,
    pub rows: Vec<Vec<String>>,
    /// The rows shown, as indexes into `rows`: filtered, then sorted.
    pub displayed: Vec<usize>,
    pub layout: DataGridLayout,
    pub sheet_w: f64,
    pub sheet_h: f64,
    pub title: String,
    pub show_csv: bool,
    pub sort: Option<(usize, bool)>,
    pub grid_line: DataGridGridLineStyle,
    advanced: DataGridAdvanced,
    look: Look,
    reorder: bool,
    enabled: bool,
    selection_mode: String,
    selected_row: Option<usize>,
    selected_col: Option<usize>,
    frozen_shadow: bool,
}

fn color_prop(c: &Ctl, name: &str) -> Option<[u8; 4]> {
    c.raw(name).and_then(parse_color)
}

/// A colour property the developer actually chose: set, visible, and not the sentinel every new control starts on.
fn chosen(c: &Ctl, name: &str, sentinel: &str) -> Option<[u8; 4]> {
    let raw = c.raw(name)?;
    let colour = parse_color(raw)?;
    (colour[3] > 0 && !raw.trim().trim_start_matches('#').eq_ignore_ascii_case(sentinel.trim_start_matches('#'))).then_some(colour)
}

impl Grid {
    /// Work the grid out for control `i` at the size the page gave it and the scroll offset it last reported.
    pub fn new(cx: &ViewCtx, i: usize) -> Grid {
        let c = cx.tree.ctl(i);
        let (w, h) = cx.box_size(i);
        let (w32, h32) = (w as f32, h as f32);
        let model = model_control(c);
        let advanced = DataGridAdvanced::from_control(&model);
        let sv = |name: &str| c.raw(name).unwrap_or("");

        let columns_raw = sv("Columns");
        let cols_declared: Vec<(String, String)> = columns_raw
            .lines()
            .filter_map(|l| {
                let mut it = l.splitn(2, ':');
                let name = it.next().unwrap_or("").trim().to_owned();
                if name.is_empty() {
                    return None;
                }
                Some((name, it.next().unwrap_or("string").trim().to_lowercase()))
            })
            .collect();
        let rows = parse_rows(sv("Rows"));
        let row_h = sv("RowHeight").trim().parse::<f32>().unwrap_or(22.0).clamp(14.0, 120.0);

        // Which columns are shown: the advanced settings' own, else the ones `Columns` names.
        let display_cols: Vec<(usize, String, String)> = if advanced.columns.is_empty() {
            cols_declared.iter().enumerate().map(|(k, (n, t))| (k, n.clone(), t.clone())).collect()
        } else {
            advanced
                .columns
                .iter()
                .map(|column| {
                    let source = cols_declared
                        .iter()
                        .position(|(n, _)| n.eq_ignore_ascii_case(&column.source_name) || n.eq_ignore_ascii_case(&column.title) || n.eq_ignore_ascii_case(&column.id))
                        .unwrap_or(usize::MAX);
                    let ty = if source < cols_declared.len() { cols_declared[source].1.clone() } else { "string".to_owned() };
                    (source, column.title.clone(), ty)
                })
                .collect()
        };
        let display_cols = if display_cols.is_empty() { vec![(0, String::new(), "string".to_owned())] } else { display_cols };

        // The rows shown: filtered by the column filters, then ordered by the column the user sorted by.
        let source_names: Vec<String> = cols_declared.iter().map(|(n, _)| n.clone()).collect();
        let mut displayed = advanced.filtered_row_indices_for_sources(&rows, &source_names);
        let allow_sorting = flag(c, "AllowSorting", true);
        let sort = if allow_sorting { cx.ui.grid_sort.get(&i).copied() } else { None };
        if let Some((sort_col, descending)) = sort {
            if let Some((source, _, ty)) = display_cols.get(sort_col) {
                sort_datagrid_rows(&mut displayed, &rows, *source, ty, descending);
            }
        }

        let ncols = display_cols.len().max(1);
        let font = cx.tree.font_size_of(i).unwrap_or(12.0).clamp(6.0, 72.0) as f32;
        let show_row_numbers = flag(c, "ShowRowNumbers", false);
        let number_font = font;
        let gutter = if show_row_numbers {
            let digits = rows.len().max(displayed.len()).max(1).to_string().len().max(2) as f32;
            (digits * number_font * 0.62 + 12.0).min(w32 * 0.5)
        } else {
            0.0
        };
        let cw = (w32 - gutter).max(1.0);
        let col_w = cw / ncols as f32;
        let frozen_columns = advanced.frozen_columns.min(ncols);

        let declared: Vec<(f32, DataGridWidthUnit)> = (0..ncols)
            .map(|k| match advanced.columns.get(k) {
                Some(column) => (column.width.max(1.0), column.width_unit),
                None => (col_w, DataGridWidthUnit::Points),
            })
            .collect();
        let auto_fit = flag(c, "AutoFitColumns", false);
        let resolved = resolve_column_widths(&declared, cw, auto_fit);
        let measures: Vec<DataGridColumnMeasure> = (0..ncols)
            .map(|k| DataGridColumnMeasure {
                width: resolved.get(k).copied().unwrap_or(col_w).max(cobolt_forms::datagrid::MIN_COLUMN_WIDTH),
                frozen: advanced.columns.get(k).map(|column| column.frozen).unwrap_or(false),
            })
            .collect();

        // The look.
        let face_opacity = {
            let t = c.raw("transparency").and_then(parse_f64).unwrap_or(0.0).clamp(0.0, 100.0);
            1.0 - t / 100.0
        };
        let header_bg = color_prop(c, "HeaderBackgroundColor").unwrap_or([60, 66, 96, 255]);
        let header_fg = color_prop(c, "HeaderForegroundColor").unwrap_or([235, 238, 250, 255]);
        let cell_fg = chosen(c, "ForegroundColor", DEFAULT_FOREGROUND_COLOR).unwrap_or([225, 230, 250, 255]);
        let underlay = chosen(c, "BackgroundColor", DEFAULT_BACKGROUND_COLOR).map(|u| [u[0], u[1], u[2], (u[3] as f64 * face_opacity).round().clamp(0.0, 255.0) as u8]);
        let alt_base = color_prop(c, "AlternatingRowColor").unwrap_or([38, 44, 72, 255]);
        let alt_opacity = sv("AlternatingRowOpacity").trim().parse::<u8>().unwrap_or(20).min(100);
        let alt = [alt_base[0], alt_base[1], alt_base[2], ((alt_opacity as u16 * 255) / 100) as u8];
        let axis = sv("AlternatingMode").trim().to_ascii_lowercase();
        let alt_none = matches!(axis.as_str(), "none" | "off");
        let alt_cols = matches!(axis.as_str(), "columns" | "column");
        let line = chosen(c, "ForegroundColor", DEFAULT_FOREGROUND_COLOR).or_else(|| color_prop(c, "GridLineColor")).unwrap_or([255, 255, 255, 90]);
        let look = Look { header_bg, header_fg, cell_fg, line, underlay, alt, alt_rows: !alt_none && !alt_cols, alt_cols, face_opacity };

        // The bands above the rows.
        let show_filters = flag(c, "ShowColumnFilters", false);
        let header_h = if show_filters { (row_h * 1.85).max(row_h + 18.0) } else { row_h };
        let title = sv("Title").trim().to_owned();
        let show_csv = flag(c, "ShowCSVExportButton", false) && flag(c, "ExportCSV", true);
        let radius = {
            let r = c.raw("cornerradius").or_else(|| c.raw("borderradius")).and_then(parse_f64).unwrap_or(0.0) as f32;
            r.clamp(0.0, (0.5 * w32.min(h32)).max(0.0))
        };
        let title_font = (font + 2.0).clamp(10.0, 22.0);
        let caption_want = (title_font + 12.0).max(24.0).max(radius);
        let caption_h = if (show_csv || !title.is_empty()) && h32 >= caption_want + header_h + row_h { caption_want } else { 0.0 };
        let band_h = caption_h + header_h;

        // Row heights that differ (`RowHeightOverrides`).
        let overrides = parse_row_height_overrides(sv(ROW_HEIGHT_OVERRIDES_PROP));
        let heights: Vec<f32> = if overrides.is_empty() { Vec::new() } else { displayed.iter().map(|r| overrides.get(r).copied().unwrap_or(row_h)).collect() };

        let scroll = cx.ui.grid_scroll.get(&i).copied().unwrap_or((0.0, 0.0));
        let layout = DataGridLayout::compute(&DataGridLayoutInput {
            width: cw,
            height: h32.max(band_h),
            row_count: displayed.len(),
            columns: measures.clone(),
            row_height: row_h,
            row_heights: heights,
            header_height: band_h,
            frozen_columns,
            frozen_rows: 0,
            scroll_x: scroll.0 as f32,
            scroll_y: scroll.1 as f32,
            row_buffer: OVERSCAN,
        });

        // Where each column starts in the sheet: the frozen ones first, in the order they are laid out.
        let mut order: Vec<usize> = (0..ncols).filter(|k| *k < frozen_columns || measures[*k].frozen).collect();
        let frozen_count = order.len();
        order.extend((0..ncols).filter(|k| !(*k < frozen_columns || measures[*k].frozen)));
        let mut x = gutter as f64;
        let mut cols = Vec::with_capacity(ncols);
        for (pos, k) in order.iter().enumerate() {
            let meta = advanced.columns.get(*k);
            let (source, name, ty) = &display_cols[*k];
            let width = measures[*k].width as f64;
            cols.push(Col {
                display: *k,
                source: *source,
                title: name.clone(),
                ty: ty.clone(),
                x,
                w: width,
                frozen: pos < frozen_count,
                align: meta.map(|m| m.text_alignment).unwrap_or_default(),
                font: meta.map(|m| m.font_size).filter(|s| *s > 0).map(|s| s as f64).unwrap_or(font as f64),
                header_font: meta.map(|m| m.header_font_size.max(6) as f64).unwrap_or(12.0),
                mask: meta.map(|m| m.cobol_mask.clone()).unwrap_or_default(),
            });
            x += width;
        }

        let selected_row = c.raw("clickedrow").and_then(parse_f64).filter(|v| *v >= 1.0).map(|v| v as usize - 1);
        let selected_col = c
            .raw("clickedcolumn")
            .and_then(parse_f64)
            .filter(|v| *v >= 1.0)
            .and_then(|v| display_cols.iter().position(|(s, _, _)| *s == v as usize - 1));
        let sheet_w = (gutter as f64 + layout.total_columns_width as f64).max(w);
        let sheet_h = (band_h as f64 + layout.total_rows_height as f64).max(h);
        let grid_line = advanced.grid_line_style;
        Grid {
            w,
            h,
            row_h: row_h as f64,
            font: font as f64,
            gutter: gutter as f64,
            number_font: number_font as f64,
            caption_h: caption_h as f64,
            header_h: header_h as f64,
            band_h: band_h as f64,
            radius: radius as f64,
            cols,
            rows,
            displayed,
            layout,
            sheet_w,
            sheet_h,
            title,
            show_csv,
            sort,
            grid_line,
            advanced,
            look,
            reorder: c.enabled() && ncols > 1 && flag(c, "AllowColumnReorder", true),
            enabled: c.enabled(),
            selection_mode: sv("SelectionMode").trim().to_ascii_lowercase(),
            selected_row,
            selected_col,
            frozen_shadow: flag(c, "FrozenShadow", true),
        }
    }

    /// The width the frozen columns take, the gutter not counted.
    pub fn frozen_width(&self) -> f64 {
        self.layout.frozen_columns_width as f64
    }

    /// Is the cell of data row `row` at display column `col` the one (or one of those) the selection marks?
    fn is_selected(&self, row: usize, col: usize) -> bool {
        match self.selection_mode.as_str() {
            "row" => self.selected_row == Some(row),
            "column" => self.selected_col == Some(col),
            _ => self.selected_row == Some(row) && self.selected_col == Some(col),
        }
    }
}

// ── the elements ────────────────────────────────────────────────────────────

impl Grid {
    /// One line of text, placed with the arithmetic of the desktop's text layout.
    fn text(&self, key: impl Into<String>, text: &str, size: f64, left: f64, top: f64, ink: [u8; 4]) -> VNode {
        VNode::new("span")
            .key(key)
            .class("pr-dg-t")
            .text(text.to_owned())
            .style("left", px(left))
            .style("top", px(top))
            .style("font-size", px(size))
            .style("line-height", px(metrics::row_height(size)))
            .style("color", css_color(ink))
    }

    /// Where the left edge of `text` goes inside a frame `frame_w` wide for the alignment, in whole pixels from the frame's left edge
    /// (`inset` pixels in from each side for the text kept to a side, as the painter's `+ 6.0` and `- 6.0`).
    fn text_left(align: DataGridTextAlignment, text: &str, size: f64, frame_w: f64) -> f64 {
        let tw = metrics::text_width(text, size);
        match align {
            DataGridTextAlignment::Right => whole(frame_w - 8.0 - tw),
            DataGridTextAlignment::Center => whole(frame_w / 2.0 - tw / 2.0),
            DataGridTextAlignment::Left => 8.0,
        }
    }

    /// The colour a frozen cell is drawn in: made opaque, so that what scrolls under it does not show through.
    /// A translucent cell would let a tenth of the column that slid behind it come through; the desktop clips the scrolled
    /// column instead, which a page cannot do without a script, so the cell takes the colour the stack comes to.
    fn frozen_fill(&self, cell: Option<[u8; 4]>, tinted: bool) -> Option<[u8; 3]> {
        let u = self.look.underlay?;
        if self.look.face_opacity < 0.85 {
            return None;
        }
        let ground = [u[0], u[1], u[2]];
        let mut under = ground;
        if tinted {
            under = flatten(self.look.alt, under);
        }
        Some(flatten(cell.unwrap_or(u), under))
    }

    /// The classes that draw the hair line left of a column and, for the last frozen one, the line right of it.
    fn line_classes(&self, pos: usize, last_frozen: bool) -> &'static str {
        if self.grid_line == DataGridGridLineStyle::None {
            return "";
        }
        match (pos > 0 || self.gutter > 0.0, last_frozen) {
            (true, true) => "pr-dg-ln pr-dg-lf",
            (true, false) => "pr-dg-ln",
            (false, true) => "pr-dg-lf",
            (false, false) => "",
        }
    }

    /// The titles' band: one cell per column, each carrying the buttons that move it.
    fn header(&self) -> VNode {
        let l = &self.look;
        let mut head = VNode::new("div")
            .key("head")
            .class("pr-dg-head")
            .attr("role", "row")
            .style("top", px(self.caption_h))
            .style("width", px(self.sheet_w))
            .style("height", px(self.header_h))
            .style("background-color", css_color(l.header_bg));
        if self.gutter > 0.0 {
            head = head.child(
                VNode::new("div")
                    .key("hn")
                    .class("pr-dg-hc pr-dg-fz")
                    .style("width", px(self.gutter))
                    .style("height", px(self.header_h))
                    .style("left", "0px")
                    .style("background-color", css_color(l.header_bg)),
            );
        }
        let last_frozen = self.cols.iter().rposition(|c| c.frozen);
        for (pos, col) in self.cols.iter().enumerate() {
            let mut hc = VNode::new("div")
                .key(format!("h{}", col.display))
                .class("pr-dg-hc")
                .class(self.line_classes(pos, last_frozen == Some(pos)))
                .attr("role", "columnheader")
                .attr("data-part", "header")
                .attr("data-c", col.display.to_string())
                .style("width", px(col.w))
                .style("height", px(self.header_h))
                .style("background-color", css_color(l.header_bg));
            if col.frozen {
                hc = hc.class("pr-dg-fz").style("left", px(col.x));
            }
            if let Some((sc, desc)) = self.sort {
                if sc == col.display {
                    hc = hc.attr("aria-sort", if desc { "descending" } else { "ascending" }).class(if desc { "pr-dg-desc" } else { "pr-dg-asc" });
                }
            }
            let size = col.header_font;
            let title_y = if self.header_h > 0.0 { metrics::text_top(self.header_h, size) } else { 0.0 };
            hc = hc.child(self.text("t", &col.title, size, Self::text_left(DataGridTextAlignment::Center, &col.title, size, col.w), title_y, l.header_fg));
            // The two buttons that move the column left and right.
            if self.reorder && col.w >= 58.0 {
                let btn_h = (self.row_h - 6.0).max(10.0);
                let glyph = 12.0;
                for (dir, glyph_char, dx, show) in [("left", "\u{2039}", col.w - 34.0, col.display > 0), ("right", "\u{203A}", col.w - 18.0, col.display + 1 < self.cols.len())] {
                    if !show {
                        continue;
                    }
                    let tw = metrics::text_width(glyph_char, glyph);
                    // Centred on the button: the glyph's left edge in whole pixels from the button's.
                    let left = whole(14.0 / 2.0 - tw / 2.0);
                    let top = metrics::text_top(btn_h, glyph) + 0.0;
                    hc = hc.child(
                        VNode::new("div")
                            .key(format!("m{dir}"))
                            .class("pr-dg-mv")
                            .attr("data-part", "colmove")
                            .attr("data-id", dir)
                            .attr("data-c", col.display.to_string())
                            .attr("role", "button")
                            .attr("aria-label", if dir == "left" { "Move column left" } else { "Move column right" })
                            .style("left", px(dx))
                            .style("top", "3px")
                            .style("width", "14px")
                            .style("height", px(btn_h))
                            .child(self.text("g", glyph_char, glyph, left, top, l.header_fg)),
                    );
                }
            }
            head = head.child(hc);
        }
        head
    }

    /// The caption row: the grid's `Title` and the CSV button.
    fn caption(&self) -> VNode {
        let l = &self.look;
        let mut cap = VNode::new("div")
            .key("caption")
            .class("pr-dg-caption")
            .style("width", px(self.w))
            .style("height", px(self.caption_h))
            .style("background-color", css_color(l.header_bg));
        let title_font = (self.font + 2.0).clamp(10.0, 22.0);
        let badge_h = (self.caption_h - 8.0).clamp(14.0, 22.0);
        let badge_w = badge_h * 1.9;
        if !self.title.is_empty() {
            // Cut short of the button's gutter rather than drawn under it.
            let clip = if self.show_csv { (self.w - (badge_w + 14.0)).max(0.0) } else { self.w };
            let tw = metrics::text_width(&self.title, title_font);
            let left = whole(self.w / 2.0 - tw / 2.0);
            let top = metrics::text_top(self.caption_h, title_font);
            cap = cap.child(
                VNode::new("div")
                    .key("title")
                    .class("pr-dg-clip")
                    .style("width", px(clip))
                    .style("height", px(self.caption_h))
                    .child(self.text("t", &self.title, title_font, left, top, l.header_fg)),
            );
        }
        if self.show_csv && self.enabled && self.w >= 64.0 {
            let bx = self.w - (badge_w + 8.0);
            let by = (self.caption_h - badge_h) * 0.5;
            // The painter strokes the badge's outline shrunk by half a pixel and again drawn inside it, so that the line's
            // centre lands half a pixel in on each side and is put on the pixel it falls in: the box a page draws the
            // border of is those pixels, which is a pixel smaller at the top than the badge's own rectangle.
            let (left, top) = (whole(bx + 0.5), whole(by + 0.5));
            let (right, bottom) = (whole(bx + badge_w - 1.5), whole(by + badge_h - 1.5));
            // The painter's ink is the premultiplied colour (225, 233, 245) at an alpha of 190, which over the band comes to
            // (233, 253, 255) by the arithmetic of the painter's source; the picture it makes shows pure white, and that is what
            // is drawn here.
            let ink = [255, 255, 255, 255];
            let size = (badge_h * 0.56).clamp(8.0, 12.0);
            let tw = metrics::text_width("CSV", size);
            cap = cap
                .child(
                    VNode::new("div")
                        .key("csv")
                        .class("pr-dg-csv")
                        .attr("data-part", "csv")
                        .attr("role", "button")
                        .attr("aria-label", "Export CSV")
                        .attr("title", "Export CSV")
                        .style("left", px(left))
                        .style("top", px(top))
                        .style("width", px(right - left + 1.0))
                        .style("height", px(bottom - top + 1.0))
                        .style("border-color", css_color(ink)),
                )
                .child(self.text("csvt", "CSV", size, whole(bx + badge_w / 2.0 - tw / 2.0), whole(by + badge_h / 2.0 - metrics::row_height(size) / 2.0) + metrics::baseline_nudge(size), ink).attr("data-part", "csv"));
        }
        cap
    }

    /// One row of the body, for the data row `orig` shown at place `pos`, `rh` tall, `top` below the sheet's top.
    fn row(&self, pos: usize, orig: Option<usize>, top: f64, rh: f64) -> VNode {
        let l = &self.look;
        let tinted = l.alt_rows && pos % 2 == 1;
        let mut row = VNode::new("div")
            .key(match orig {
                Some(o) => format!("r{o}"),
                None => format!("b{pos}"),
            })
            .class("pr-dg-row")
            .style("top", px(top))
            .style("height", px(rh))
            .style("width", px(self.sheet_w));
        if let Some(o) = orig {
            row = row.attr("role", "row").attr("data-r", o.to_string()).attr("aria-rowindex", (pos + 2).to_string()).attr("aria-selected", if self.selected_row == Some(o) { "true" } else { "false" });
        } else {
            row = row.attr("aria-hidden", "true");
        }
        if tinted {
            row = row.style("background-color", css_color(l.alt));
        }
        if self.gutter > 0.0 {
            let mut n = VNode::new("div")
                .key("n")
                .class("pr-dg-c pr-dg-fz pr-dg-num")
                .style("left", "0px")
                .style("width", px(self.gutter))
                .style("height", px(rh))
                .style("background-color", css_color(l.header_bg));
            if let Some(o) = orig {
                let _ = o;
                let text = (pos + 1).to_string();
                let ink = [l.header_fg[0], l.header_fg[1], l.header_fg[2], (l.header_fg[3] as f64 * 0.8).round() as u8];
                let tw = metrics::text_width(&text, self.number_font);
                n = n.child(self.text("t", &text, self.number_font, whole(self.gutter - 6.0 - tw), metrics::text_top(rh, self.number_font), ink));
            }
            row = row.child(n);
        }
        let last_frozen = self.cols.iter().rposition(|c| c.frozen);
        for (p, col) in self.cols.iter().enumerate() {
            let meta = self.advanced.columns.get(col.display);
            let raw = orig.and_then(|o| self.rows.get(o)).and_then(|r| r.get(col.source)).map(String::as_str).unwrap_or("");
            let rule = meta.and_then(|m| m.value_style_rule_for(raw));
            let cell_bg = rule
                .and_then(|r| parse_color(&r.background_color))
                .filter(|c| c[3] > 0)
                .or_else(|| meta.and_then(|m| parse_color(&m.background_color)).filter(|c| c[3] > 0))
                .or(l.underlay);
            let mut cell = VNode::new("div")
                .key(format!("c{}", col.display))
                .class("pr-dg-c")
                .class(self.line_classes(p, last_frozen == Some(p)))
                .attr("role", "gridcell")
                .attr("data-c", col.display.to_string())
                .style("width", px(col.w))
                .style("height", px(rh));
            if l.alt_cols && col.display % 2 == 1 {
                cell = cell.style("background-color", css_color(l.alt));
            }
            if col.frozen {
                cell = cell.class("pr-dg-fz").style("left", px(col.x));
                match self.frozen_fill(cell_bg, tinted) {
                    Some(c) => cell = cell.style("background-color", rgb(c)),
                    None => {
                        if let Some(bg) = cell_bg {
                            cell = cell.style("background-color", css_color(bg));
                        }
                    }
                }
            } else if let Some(bg) = cell_bg {
                cell = cell.style("background-color", css_color(bg));
            }
            if let Some(o) = orig {
                if self.is_selected(o, col.display) {
                    cell = cell.class("pr-dg-sel");
                }
                let (text, _) = format_cell_with_cobol_mask(raw, &col.ty, &col.mask);
                if !text.is_empty() {
                    let ink = rule.and_then(|r| parse_color(&r.foreground_color)).or_else(|| meta.and_then(|m| parse_color(&m.foreground_color))).unwrap_or(l.cell_fg);
                    let left = Self::text_left(col.align, &text, col.font, col.w);
                    cell = cell.child(self.text("t", &text, col.font, left, metrics::text_top(rh, col.font), ink));
                }
            }
            row = row.child(cell);
        }
        row
    }

    /// The pill-shaped scroll bars: a track and a thumb that follow the offset the page last reported.
    fn bars(&self) -> Vec<VNode> {
        let mut out = Vec::new();
        let body_h = (self.h - self.band_h).max(0.0);
        let r = BAR / 2.0;
        let v_bar = self.layout.max_scroll_y > 0.0 && body_h > BAR * 2.0;
        let h_bar = self.layout.max_scroll_x > 0.0 && self.w > BAR * 2.0;
        // Each bar stops short of the other, so that they never cross in the corner.
        let v_gap = if h_bar { BAR + 2.0 } else { 2.0 };
        let h_gap = if v_bar { BAR + 2.0 } else { 2.0 };
        let (sx, sy) = (self.layout.scroll_x as f64, self.layout.scroll_y as f64);
        // A thumb is two layers: the painter's colour is premultiplied, so it darkens what is under it by what it lets
        // through and then adds itself — a black layer, and the colour added to it.
        let thumb = |key: &str, left: f64, top: f64, w: f64, h: f64| -> [VNode; 2] {
            let at = |k: String, extra: &str| VNode::new("div").key(k).class(format!("pr-dg-thumb {extra}")).style("left", px(left)).style("top", px(top)).style("width", px(w)).style("height", px(h));
            [at(format!("{key}a"), ""), at(format!("{key}b"), "pr-dg-thumb-add")]
        };
        let track = |key: &str, left: f64, top: f64, w: f64, h: f64| -> VNode {
            VNode::new("div").key(key.to_owned()).class("pr-dg-track").style("left", px(left)).style("top", px(top)).style("width", px(w)).style("height", px(h))
        };
        if v_bar {
            // The track hugs the right edge; its bottom is drawn in by how far the grid's arc has cut in at its place.
            let x1 = self.w - 2.0;
            let bottom = (self.h - v_gap).min(self.h - arc_inset(self.w, self.radius, x1 - r));
            let top = self.band_h + 2.0;
            let tr_h = bottom - top;
            if tr_h > BAR {
                let total = self.layout.total_rows_height as f64;
                let thumb_h = (body_h / total * tr_h).clamp(BAR_MIN.min(tr_h), tr_h);
                let travel = (tr_h - thumb_h).max(0.0);
                let ty = top + travel * (sy / self.layout.max_scroll_y as f64);
                out.push(track("vt", x1 - BAR, top, BAR, tr_h));
                out.extend(thumb("vh", x1 - BAR, ty, BAR, thumb_h));
            }
        }
        if h_bar {
            let y1 = self.h - 2.0;
            let right = (self.w - h_gap).min(self.w - arc_inset(self.h, self.radius, y1 - r));
            let left = 2.0;
            let tr_w = right - left;
            if tr_w > BAR {
                let visible = self.w.max(1.0);
                let total = visible + self.layout.max_scroll_x as f64;
                let thumb_w = (visible / total * tr_w).clamp(BAR_MIN.min(tr_w), tr_w);
                let travel = (tr_w - thumb_w).max(0.0);
                let tx = left + travel * (sx / self.layout.max_scroll_x as f64);
                out.push(track("ht", left, y1 - BAR, tr_w, BAR));
                out.extend(thumb("hh", tx, y1 - BAR, thumb_w, BAR));
            }
        }
        out
    }

    pub fn build(&self, cx: &ViewCtx, i: usize) -> VNode {
        let l = &self.look;
        let mut root = super::controls::root(cx, i, "div")
            .class("pr-dg")
            .attr("role", "grid")
            .attr("aria-rowcount", (self.rows.len() + 1).to_string())
            .attr("aria-colcount", self.cols.len().to_string())
            .attr("tabindex", "0")
            .style("--pr-dg-line", css_color(l.line))
            .style("--pr-dg-sel", css_color(SELECTION))
            .on("click")
            .on("dblclick")
            .on("keydown");

        // The sheet: the whole content, as tall and wide as the rows and columns make it.
        let mut sheet = VNode::new("div").key("sheet").class("pr-dg-sheet").style("width", px(self.sheet_w)).style("height", px(self.sheet_h));
        if self.caption_h > 0.0 {
            sheet = sheet.child(self.caption());
        }
        sheet = sheet.child(self.header());
        // The rows in view and the ones just outside it.
        let (first, last) = (self.layout.first_row, self.layout.last_row_exclusive.min(self.displayed.len()));
        for pos in first..last {
            let orig = self.displayed[pos];
            let top = self.band_h + self.layout.rows.top(pos) as f64;
            let rh = self.layout.rows.height(pos) as f64;
            sheet = sheet.child(self.row(pos, Some(orig), top, rh));
        }
        // Below the last row the grid goes on with the stripes of blank rows, as a spreadsheet does.
        let rows_bottom = self.band_h + self.layout.total_rows_height as f64;
        if last >= self.displayed.len() && rows_bottom < self.h - 0.5 {
            let mut top = rows_bottom;
            let mut pos = self.displayed.len();
            while top < self.h && pos < self.displayed.len() + 600 {
                sheet = sheet.child(self.row(pos, None, top, self.row_h));
                top += self.row_h;
                pos += 1;
            }
        }
        // The grid's colour carries on right of the last column.
        let x_end = self.gutter + self.layout.total_columns_width as f64;
        if x_end < self.w - 0.5 {
            if let Some(u) = l.underlay {
                root = root.child(
                    VNode::new("div")
                        .key("fill")
                        .class("pr-dg-fill")
                        .style("left", px(x_end))
                        .style("top", px(self.band_h))
                        .style("right", "0px")
                        .style("bottom", "0px")
                        .style("background-color", css_color(u)),
                );
            }
        }
        let scroller = VNode::new("div").key("scroller").class("pr-dg-scroll").on("scroll").child(sheet);
        root = root.child(scroller);
        // The lines, the outline, the shadows and the bars are drawn over the rows, in the painter's order.
        if self.grid_line != DataGridGridLineStyle::None {
            if x_end < self.w - 0.5 {
                root = root.child(VNode::new("div").key("vl").class("pr-dg-vl").style("left", px(x_end)).style("top", px(self.caption_h)).style("bottom", "0px"));
            }
            root = root.child(VNode::new("div").key("hl").class("pr-dg-hl").style("top", px(self.band_h)));
            if self.caption_h > 0.0 {
                root = root.child(VNode::new("div").key("hlc").class("pr-dg-hl").style("top", px(self.caption_h)));
            }
        }
        root = root.child(VNode::new("div").key("ol").class("pr-dg-ol").style("border-radius", px(self.radius)));
        if self.frozen_shadow {
            let fw = self.gutter + self.frozen_width();
            if self.frozen_width() > 0.0 && self.layout.max_scroll_x > 0.0 {
                root = root.child(
                    VNode::new("div")
                        .key("shl")
                        .class("pr-dg-sh pr-dg-sh-l")
                        .style("left", px(fw))
                        .style("top", "0px")
                        .style("width", px(SHADOW_LEFT.min((self.w - fw).max(0.0))))
                        .style("height", px(self.h)),
                );
            }
            if self.layout.max_scroll_y > 0.0 {
                root = root.child(VNode::new("div").key("sht").class("pr-dg-sh pr-dg-sh-t").style("left", "0px").style("top", px(self.band_h)).style("width", px(self.w)).style("height", px(SHADOW_TOP.min((self.h - self.band_h).max(0.0)))));
            }
        }
        for b in self.bars() {
            root = root.child(b);
        }
        root
    }
}

/// The DataGrid control `i` as an element.
pub fn data_grid(cx: &ViewCtx, i: usize) -> VNode {
    Grid::new(cx, i).build(cx, i)
}

/// The data column (from 0) a display column shows, for what a click reports (`ClickedColumn`): `None` when no data is behind it.
pub fn source_of(cx: &ViewCtx, i: usize, display: usize) -> Option<usize> {
    Grid::new(cx, i).cols.iter().find(|c| c.display == display).map(|c| c.source).filter(|s| *s != usize::MAX)
}

/// Rows the grid has, as `Rows` is parsed: what a click on a row is checked against.
pub fn row_count(cx: &ViewCtx, i: usize) -> usize {
    parse_rows(cx.tree.ctl(i).raw("rows").unwrap_or("")).len()
}

/// The rows the grid shows, in the order it shows them: indexes into `Rows`, filtered and sorted (what the keyboard walks).
pub fn displayed_order(cx: &ViewCtx, i: usize) -> Vec<usize> {
    Grid::new(cx, i).displayed
}

/// May the grid be sorted by this column? (`sort_enabled`, and not a Button column.)
pub fn sortable(cx: &ViewCtx, i: usize, display: usize) -> bool {
    let g = Grid::new(cx, i);
    g.advanced.columns.get(display).map(|c| c.sort_enabled && !c.is_button()).unwrap_or(true)
}

/// The `AdvancedGrid` text with a column moved one place left or right, or `None` when it cannot move that way.
pub fn moved_columns(cx: &ViewCtx, i: usize, display: usize, left: bool) -> Option<String> {
    let mut advanced = Grid::new(cx, i).advanced;
    let moved = if left { advanced.move_column_left(display) } else { advanced.move_column_right(display) };
    if moved {
        advanced.to_json().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Empty, UiState};
    use super::*;
    use crate::tree::testing::tree_with;

    /// The grid of the PowerAnalytics Dados page, as the desktop's headless run left it, at the size it was drawn:
    /// 16 columns of the widths its program set, the first one frozen, 300 rows, the CSV button on.
    fn dados_props() -> Vec<(&'static str, String)> {
        let columns = ["Nome Comercial", "Código ANBIMA", "Estrutura", "Nível 1", "Nível 2", "Nível 3", "Início", "PL Atual", "Cotistas", "Valor da Cota", "Foco", "CNPJ da Classe", "CNPJ do Fundo", "Status", "Subclasses", "Data"];
        let kinds = ["string", "string", "string", "string", "string", "string", "string", "number", "number", "number", "string", "string", "string", "string", "number", "string"];
        let cols: Vec<String> = columns.iter().zip(kinds).map(|(c, k)| format!("{c}:{k}")).collect();
        let rows: Vec<String> = (0..300).map(|r| (0..16).map(|c| format!("r{r}c{c}")).collect::<Vec<_>>().join("\t")).collect();
        vec![
            ("Columns", cols.join("\n")),
            ("Rows", rows.join("\n")),
            ("RowHeight", "30".into()),
            ("FontSize", "12".into()),
            ("CornerRadius", "16".into()),
            ("BackgroundColor", "#FFFFFF".into()),
            ("Transparency", "10".into()),
            ("ForegroundColor", "#1B2A4E".into()),
            ("HeaderBackgroundColor", "#1F4E9A".into()),
            ("HeaderForegroundColor", "#FFFFFF".into()),
            ("AlternatingRowColor", "#9CC4F2".into()),
            ("AlternatingRowOpacity", "40".into()),
            ("FrozenColumns", "1".into()),
            ("ShowCSVExportButton", "1".into()),
            ("_RuntimeColumnWidths", "1=330\n2=110\n3=90\n4=120\n5=170\n6=170\n7=92\n8=120\n9=90\n10=110\n11=190\n12=130\n13=130\n14=70\n15=90\n16=130".into()),
        ]
    }

    fn dados(ui: impl FnOnce(&mut UiState, usize)) -> VNode {
        let props = dados_props();
        let borrowed: Vec<(&str, &str)> = props.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let (t, i) = tree_with("DataGrid", "GRD-FUNDOS", &borrowed);
        let mut e = Empty::default();
        e.ui.box_sizes.insert(i, (1086.0, 538.0));
        ui(&mut e.ui, i);
        data_grid(&e.cx(&t), i)
    }

    fn style<'a>(n: &'a VNode, k: &str) -> Option<&'a str> {
        n.styles.iter().rev().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
    }

    fn px_of(n: &VNode, k: &str) -> f64 {
        style(n, k).unwrap_or_else(|| panic!("{} has no {k}", n.key)).trim_end_matches("px").parse().unwrap()
    }

    #[test]
    fn the_bands_and_the_columns_are_laid_out_with_the_painters_numbers() {
        let n = dados(|_, _| {});
        let caption = n.find(&|x| x.has_class("pr-dg-caption")).expect("the CSV button asks for a caption row");
        let head = n.find(&|x| x.has_class("pr-dg-head")).unwrap();
        assert_eq!((px_of(caption, "height"), px_of(head, "top"), px_of(head, "height")), (26.0, 26.0, 30.0), "a caption of the title's size + 12, then one row of titles");
        let mut cells = Vec::new();
        head.find_all(&|x| x.has_class("pr-dg-hc") && x.attr_of("data-c").is_some(), &mut cells);
        assert_eq!(cells.len(), 16);
        let widths: Vec<f64> = cells.iter().map(|c| px_of(c, "width")).collect();
        assert_eq!(widths[..4], [330.0, 110.0, 90.0, 120.0], "the widths the program set");
        assert!(cells[0].has_class("pr-dg-fz") && px_of(cells[0], "left") == 0.0, "the first column is frozen at the grid's left edge");
        assert!(!cells[1].has_class("pr-dg-fz"));
        assert!(cells[0].has_class("pr-dg-lf") && !cells[0].has_class("pr-dg-ln"), "the frozen column draws the line on its right, none on its left");
        assert!(cells[1].has_class("pr-dg-ln"), "every other column draws one on its left");
        let sheet = n.find(&|x| x.has_class("pr-dg-sheet")).unwrap();
        assert_eq!(px_of(sheet, "width"), widths.iter().sum::<f64>(), "the sheet is as wide as the columns");
        assert_eq!(px_of(sheet, "height"), 56.0 + 300.0 * 30.0, "and as tall as the bands and the rows");
    }

    #[test]
    fn only_the_rows_in_view_are_drawn_and_each_sits_where_the_painter_puts_it() {
        let n = dados(|_, _| {});
        let mut rows = Vec::new();
        n.find_all(&|x| x.has_class("pr-dg-row") && x.attr_of("data-r").is_some(), &mut rows);
        // 538 - 56 = 482 px of body: 17 rows, and the overscan after them.
        assert!(rows.len() < 40, "{} rows drawn for 300", rows.len());
        assert_eq!(rows[0].attr_of("data-r"), Some("0"));
        assert_eq!(px_of(rows[0], "top"), 56.0);
        assert_eq!(px_of(rows[1], "top"), 86.0);
        assert!(rows[1].styles.iter().any(|(k, v)| k == "background-color" && v.starts_with("rgb(156 196 242")), "every other row carries the tint, at 40 %");
        assert!(!rows[0].styles.iter().any(|(k, _)| k == "background-color"));
        // Scrolled a long way down, the rows drawn are the ones there.
        let far = dados(|ui, i| {
            ui.grid_scroll.insert(i, (0.0, 30.0 * 200.0));
        });
        let mut rows = Vec::new();
        far.find_all(&|x| x.has_class("pr-dg-row") && x.attr_of("data-r").is_some(), &mut rows);
        let first: usize = rows[0].attr_of("data-r").unwrap().parse().unwrap();
        assert!((185..=200).contains(&first), "the first row drawn is near 200: {first}");
        assert_eq!(px_of(rows[0], "top"), 56.0 + first as f64 * 30.0, "its place in the sheet, which the page has scrolled");
    }

    #[test]
    fn a_cells_text_is_put_where_the_desktops_layout_puts_it() {
        let n = dados(|_, _| {});
        let row = n.find(&|x| x.has_class("pr-dg-row") && x.attr_of("data-r").is_some()).unwrap();
        let cell = &row.children[0];
        let t = cell.children.iter().find(|c| c.has_class("pr-dg-t")).expect("the text of the first cell");
        assert_eq!(t.text.as_deref(), Some("r0c0"));
        // 2 px of the cell's own inset and 6 more (`+ 6.0` in the painter); the row's 30 px centres a 14 px line 8 px down.
        assert_eq!((px_of(t, "left"), px_of(t, "top")), (8.0, 8.0));
        assert_eq!(style(t, "font-size"), Some("12px"));
        assert_eq!(style(t, "color"), Some("rgb(0 0 0)"), "a column that names no colour is drawn in black, not in the grid's ForegroundColor");
    }

    #[test]
    fn the_csv_button_is_on_the_pixels_the_painters_stroke_lands_on() {
        let n = dados(|_, _| {});
        let b = n.find(&|x| x.has_class("pr-dg-csv")).expect("the button");
        // The badge is 18 x 34.2 at 8 px from the right; the stroke is half a pixel in and drawn inside, so its line falls
        // on whole pixels one in from the top and 34 px wide.
        assert_eq!((px_of(b, "left"), px_of(b, "top"), px_of(b, "width"), px_of(b, "height")), (1044.0, 5.0, 34.0, 17.0));
    }

    #[test]
    fn the_bars_follow_the_scroll_offset_the_page_reported() {
        let at = |left: f64, top: f64| {
            let n = dados(|ui, i| {
                ui.grid_scroll.insert(i, (left, top));
            });
            let mut thumbs = Vec::new();
            n.find_all(&|x| x.key == "vha" || x.key == "hha", &mut thumbs);
            (px_of(thumbs[0], "top"), px_of(thumbs[1], "left"))
        };
        let (top0, left0) = at(0.0, 0.0);
        assert_eq!(top0, 58.0, "the track starts 2 px below the bands");
        assert_eq!(left0, 2.0);
        let (top1, left1) = at(500.0, 3000.0);
        assert!(top1 > top0 && left1 > left0, "scrolled: the thumbs have moved ({top1}, {left1})");
        // The desktop's own clamp: more than the content has is the end of it.
        let (top2, _) = at(0.0, 1.0e9);
        let (top3, _) = at(0.0, 3.0e9);
        assert_eq!(top2, top3);
    }

    #[test]
    fn a_sorted_grid_shows_its_rows_in_order_and_each_keeps_its_number() {
        let (t, i) = tree_with("DataGrid", "G", &[("Columns", "Name:string\nValue:number"), ("Rows", "b\t2\na\t10\nc\t1")]);
        let mut e = Empty::default();
        e.ui.box_sizes.insert(i, (300.0, 200.0));
        e.ui.grid_sort.insert(i, (1, false));
        let n = data_grid(&e.cx(&t), i);
        let mut rows = Vec::new();
        n.find_all(&|x| x.attr_of("data-r").is_some() && x.has_class("pr-dg-row"), &mut rows);
        let order: Vec<&str> = rows.iter().map(|r| r.attr_of("data-r").unwrap()).collect();
        assert_eq!(order, ["2", "0", "1"], "numeric order 1, 2, 10; data-r is the row's number in Rows");
        let sorted = n.find(&|x| x.attr_of("aria-sort").is_some()).unwrap();
        assert_eq!(sorted.attr_of("aria-sort"), Some("ascending"));
        assert_eq!(sorted.attr_of("data-c"), Some("1"));
    }

    #[test]
    fn the_selected_row_is_the_clicked_one_and_its_cells_wear_the_tint() {
        let (t, i) = tree_with("DataGrid", "G", &[("Columns", "A:string\nB:string"), ("Rows", "x\t1\ny\t2\nz\t3"), ("ClickedRow", "2"), ("ClickedColumn", "2"), ("SelectionMode", "Row")]);
        let mut e = Empty::default();
        e.ui.box_sizes.insert(i, (300.0, 200.0));
        let n = data_grid(&e.cx(&t), i);
        let mut sel = Vec::new();
        n.find_all(&|x| x.attr_of("aria-selected") == Some("true"), &mut sel);
        assert_eq!(sel.len(), 1);
        assert_eq!(sel[0].attr_of("data-r"), Some("1"));
        assert_eq!(sel[0].children.iter().filter(|c| c.has_class("pr-dg-sel")).count(), 2, "the whole row, as SelectionMode says");
        // One cell mode: the one cell.
        let (t, i) = tree_with("DataGrid", "G", &[("Columns", "A:string\nB:string"), ("Rows", "x\t1\ny\t2"), ("ClickedRow", "2"), ("ClickedColumn", "2"), ("SelectionMode", "Cell")]);
        let n = data_grid(&e.cx(&t), i);
        let mut cells = Vec::new();
        n.find_all(&|x| x.has_class("pr-dg-sel"), &mut cells);
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].attr_of("data-c"), Some("1"));
    }

    #[test]
    fn a_grid_without_a_caption_row_starts_its_titles_at_the_top() {
        let (t, i) = tree_with("DataGrid", "G", &[("Columns", "A:string"), ("Rows", "x"), ("RowHeight", "30"), ("ShowCSVExportButton", "0")]);
        let mut e = Empty::default();
        e.ui.box_sizes.insert(i, (300.0, 200.0));
        let n = data_grid(&e.cx(&t), i);
        assert!(n.find(&|x| x.has_class("pr-dg-caption")).is_none());
        assert_eq!(px_of(n.find(&|x| x.has_class("pr-dg-head")).unwrap(), "top"), 0.0);
        assert!(n.find(&|x| x.has_class("pr-dg-csv")).is_none(), "no button unless ShowCSVExportButton");
    }
}
