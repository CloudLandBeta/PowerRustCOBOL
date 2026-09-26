// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Shared DataGrid layout helpers.
//!
//! This module deliberately avoids `egui` types so layout can be tested without
//! a UI context and reused by designer, preview, runtime, and compiled surfaces.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl GridRect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn max_x(self) -> f32 {
        self.x + self.w
    }

    pub fn max_y(self) -> f32 {
        self.y + self.h
    }

    fn intersects_x(self, min_x: f32, max_x: f32) -> bool {
        self.max_x() > min_x && self.x < max_x
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataGridLayoutColumn {
    pub index: usize,
    pub x: f32,
    pub width: f32,
    pub frozen: bool,
}

impl DataGridLayoutColumn {
    pub fn rect(&self, y: f32, height: f32) -> GridRect {
        GridRect::new(self.x, y, self.width, height)
    }
}

/// The narrowest a column is ever drawn. Matches the floor the resize drag
/// already enforces, so a percentage cannot produce a column too thin to grab.
pub const MIN_COLUMN_WIDTH: f32 = 32.0;

/// Resolve each column's drawn width from what it declared.
///
/// A column states its width either in points or as a percentage of the grid's
/// usable width, and the two mix freely: the narrow, predictable columns — a
/// flag, a code, a date — hold their size while the ones carrying prose take a
/// share of whatever width the form ends up at.
///
/// `adaptive` decides what happens to the difference between what the columns
/// asked for and the width there actually is. Off (the default, and what every
/// existing grid does) the columns keep their asked-for widths and the grid
/// scrolls or leaves a gap. On, the columns are made to fill the grid exactly:
/// the shortfall or surplus is absorbed by the **points** columns in proportion
/// to their size, because a percentage column has already been told what share
/// of the total it gets and moving it would contradict its own declaration. A
/// grid whose columns are *all* percentages spreads the difference over all of
/// them, since there is nothing else to absorb it.
///
/// No column is ever drawn below [`MIN_COLUMN_WIDTH`], which means a set of
/// columns can still exceed `available` when there are more of them than the
/// grid has room for — the caller scrolls, exactly as it does today.
pub fn resolve_column_widths(
    declared: &[(f32, crate::model::DataGridWidthUnit)],
    available: f32,
    adaptive: bool,
) -> Vec<f32> {
    use crate::model::DataGridWidthUnit as Unit;
    if declared.is_empty() {
        return Vec::new();
    }
    // Nothing sensible to take a percentage OF, so every column keeps its
    // number and a percentage is treated as the points it nominally is. This is
    // the zero-size first frame, not a state anything is drawn in.
    if !available.is_finite() || available <= 0.0 {
        return declared
            .iter()
            .map(|(w, _)| w.max(MIN_COLUMN_WIDTH))
            .collect();
    }

    let mut widths: Vec<f32> = declared
        .iter()
        .map(|(w, unit)| match unit {
            Unit::Points => *w,
            Unit::Percent => available * (w.clamp(0.0, 100.0) / 100.0),
        })
        .map(|w| w.max(MIN_COLUMN_WIDTH))
        .collect();

    if !adaptive {
        return widths;
    }

    let total: f32 = widths.iter().sum();
    let difference = available - total;
    if difference.abs() < 0.5 {
        return widths;
    }

    // Who absorbs it: the points columns, or everyone when they are all
    // percentages. Only columns with room to give are counted, so shrinking
    // cannot push one under the floor and lose the difference silently.
    let absorbing: Vec<usize> = {
        let points: Vec<usize> = declared
            .iter()
            .enumerate()
            .filter(|(_, (_, unit))| matches!(unit, Unit::Points))
            .map(|(i, _)| i)
            .collect();
        if points.is_empty() {
            (0..declared.len()).collect()
        } else {
            points
        }
    };
    let absorbing: Vec<usize> = if difference < 0.0 {
        absorbing
            .into_iter()
            .filter(|i| widths[*i] > MIN_COLUMN_WIDTH)
            .collect()
    } else {
        absorbing
    };
    if absorbing.is_empty() {
        return widths;
    }

    let share_base: f32 = absorbing.iter().map(|i| widths[*i]).sum();
    if share_base <= 0.0 {
        return widths;
    }
    for i in absorbing {
        let share = widths[i] / share_base;
        widths[i] = (widths[i] + difference * share).max(MIN_COLUMN_WIDTH);
    }
    widths
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataGridColumnMeasure {
    pub width: f32,
    pub frozen: bool,
}

impl DataGridColumnMeasure {
    pub fn new(width: f32) -> Self {
        Self {
            width,
            frozen: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataGridLayoutInput {
    pub width: f32,
    pub height: f32,
    pub row_count: usize,
    pub columns: Vec<DataGridColumnMeasure>,
    pub row_height: f32,
    /// One height per row, for a grid whose rows differ
    /// (`RowHeightOverrides`). Empty — the usual case — means every row is
    /// `row_height`, laid out exactly as before per-row heights existed.
    pub row_heights: Vec<f32>,
    pub header_height: f32,
    pub frozen_columns: usize,
    pub frozen_rows: usize,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub row_buffer: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataGridLayout {
    pub viewport: GridRect,
    pub header_rect: GridRect,
    pub body_rect: GridRect,
    pub frozen_columns_width: f32,
    pub total_columns_width: f32,
    pub total_rows_height: f32,
    pub max_scroll_x: f32,
    pub max_scroll_y: f32,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub first_row: usize,
    pub last_row_exclusive: usize,
    pub frozen_rows: usize,
    pub frozen_columns: Vec<DataGridLayoutColumn>,
    pub scrollable_columns: Vec<DataGridLayoutColumn>,
    /// Where each scrollable row starts, relative to the body's top at
    /// scroll 0.
    pub rows: RowGeometry,
}

/// The property holding per-row heights: `row=height` pairs, the row being
/// the DATA row numbered from 1 as COBOL numbers it (`GetCellValue(1, …)` is
/// the first row), so a sorted or filtered grid keeps each height with its
/// row. Separated by `;`, `,` or new lines — e.g. `1=40;8=64`. The parsed map
/// is keyed by the 0-based index into `Rows`.
pub const ROW_HEIGHT_OVERRIDES_PROP: &str = "RowHeightOverrides";

/// The range a per-row height is kept in: the uniform `RowHeight`'s floor,
/// and room for a row several lines tall.
pub const ROW_HEIGHT_OVERRIDE_RANGE: (f32, f32) = (14.0, 400.0);

/// Read `RowHeightOverrides` into 0-based data-row indices. Malformed pairs,
/// and row 0, are skipped.
pub fn parse_row_height_overrides(text: &str) -> std::collections::BTreeMap<usize, f32> {
    text.split([';', ',', '\n'])
        .filter_map(|pair| {
            let (row, height) = pair.split_once('=')?;
            let row = row.trim().parse::<usize>().ok()?.checked_sub(1)?;
            let height = height.trim().parse::<f32>().ok()?;
            (height > 0.0).then(|| {
                (row, height.clamp(ROW_HEIGHT_OVERRIDE_RANGE.0, ROW_HEIGHT_OVERRIDE_RANGE.1))
            })
        })
        .collect()
}

/// Write `RowHeightOverrides`, in row order.
pub fn format_row_height_overrides(map: &std::collections::BTreeMap<usize, f32>) -> String {
    map.iter()
        .map(|(row, h)| format!("{}={}", row + 1, h.round() as i64))
        .collect::<Vec<_>>()
        .join(";")
}

/// Where each of a run of rows starts, when they need not share a height.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RowGeometry {
    /// `tops[i]` is row i's top; one extra entry holds the total height.
    tops: Vec<f32>,
}

impl RowGeometry {
    /// `count` rows, row i `heights[i]` tall when given (and positive),
    /// otherwise `base`.
    pub fn new(count: usize, base: f32, heights: &[f32]) -> Self {
        let base = base.max(1.0);
        let mut tops = Vec::with_capacity(count + 1);
        let mut y = 0.0;
        tops.push(0.0);
        for i in 0..count {
            y += heights.get(i).copied().filter(|h| *h > 0.0).unwrap_or(base).max(1.0);
            tops.push(y);
        }
        Self { tops }
    }

    pub fn len(&self) -> usize {
        self.tops.len().saturating_sub(1)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Row `i`'s top; past the last row, the total height.
    pub fn top(&self, i: usize) -> f32 {
        self.tops.get(i).or(self.tops.last()).copied().unwrap_or(0.0)
    }

    pub fn height(&self, i: usize) -> f32 {
        self.top(i + 1) - self.top(i)
    }

    pub fn total(&self) -> f32 {
        self.top(self.len())
    }

    /// The row whose span holds `y`; `len()` when `y` is past the last row.
    pub fn index_at(&self, y: f32) -> usize {
        // The first top strictly greater than y, minus one.
        self.tops.partition_point(|t| *t <= y).saturating_sub(1).min(self.len())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DataGridColumnResizeHit {
    pub column_index: usize,
    pub edge_x: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DataGridRowResizeHit {
    pub row_index: Option<usize>,
    pub edge_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataGridCellSelection {
    pub row_index: usize,
    pub display_column_index: usize,
}

pub fn datagrid_copy_text(
    rows: &[Vec<String>],
    visible_source_columns: &[usize],
    selection: DataGridCellSelection,
    selection_mode: &str,
    delimiter: &str,
) -> Option<String> {
    let row = rows.get(selection.row_index)?;
    let delimiter = if delimiter.is_empty() {
        "\t"
    } else {
        delimiter
    };
    if selection_mode.eq_ignore_ascii_case("row") {
        let cells: Vec<&str> = visible_source_columns
            .iter()
            .map(|source_index| row.get(*source_index).map(String::as_str).unwrap_or(""))
            .collect();
        return Some(cells.join(delimiter));
    }

    let source_index = *visible_source_columns.get(selection.display_column_index)?;
    Some(row.get(source_index).cloned().unwrap_or_default())
}

/// What Ctrl+C copies from a grid whose `SelectionMode` is `Column`: the
/// selected column's value in every row the grid shows, in the order shown,
/// one per line.
pub fn datagrid_copy_column_text(
    rows: &[Vec<String>],
    shown_rows: &[usize],
    visible_source_columns: &[usize],
    display_column_index: usize,
) -> Option<String> {
    let source_index = *visible_source_columns.get(display_column_index)?;
    let cells: Vec<&str> = shown_rows
        .iter()
        .filter_map(|r| rows.get(*r))
        .map(|row| row.get(source_index).map(String::as_str).unwrap_or(""))
        .collect();
    Some(cells.join("\n"))
}

impl DataGridLayout {
    pub fn compute(input: &DataGridLayoutInput) -> Self {
        let width = input.width.max(0.0);
        let height = input.height.max(0.0);
        let row_height = input.row_height.max(1.0);
        let header_height = input.header_height.max(0.0).min(height);
        let body_height = (height - header_height).max(0.0);
        let viewport = GridRect::new(0.0, 0.0, width, height);
        let header_rect = GridRect::new(0.0, 0.0, width, header_height);
        let body_rect = GridRect::new(0.0, header_height, width, body_height);

        let normalized_columns: Vec<DataGridColumnMeasure> = if input.columns.is_empty() {
            vec![DataGridColumnMeasure::new(width.max(1.0))]
        } else {
            input
                .columns
                .iter()
                .map(|c| DataGridColumnMeasure {
                    width: c.width.max(1.0),
                    frozen: c.frozen,
                })
                .collect()
        };

        let explicit_frozen = input.frozen_columns.min(normalized_columns.len());
        let rows = RowGeometry::new(input.row_count, row_height, &input.row_heights);
        let total_rows_height = rows.total();
        let total_columns_width = normalized_columns.iter().map(|c| c.width).sum::<f32>();
        let frozen_columns_width = normalized_columns
            .iter()
            .enumerate()
            .filter(|(idx, col)| *idx < explicit_frozen || col.frozen)
            .map(|(_, col)| col.width)
            .sum::<f32>()
            .min(width);
        let scrollable_width = (total_columns_width - frozen_columns_width).max(0.0);
        let scrollable_view_width = (width - frozen_columns_width).max(0.0);
        let max_scroll_x = (scrollable_width - scrollable_view_width).max(0.0);
        let max_scroll_y = (total_rows_height - body_height).max(0.0);
        let scroll_x = input.scroll_x.clamp(0.0, max_scroll_x);
        let scroll_y = input.scroll_y.clamp(0.0, max_scroll_y);

        let (first_visible, last_row_exclusive) = if input.row_heights.is_empty() {
            let first_visible = (scroll_y / row_height).floor().max(0.0) as usize;
            let visible_capacity = (body_height / row_height).ceil().max(0.0) as usize;
            (
                first_visible,
                (first_visible + visible_capacity + input.row_buffer + 1).min(input.row_count),
            )
        } else {
            let first_visible = rows.index_at(scroll_y);
            let last_visible = rows.index_at(scroll_y + body_height);
            (first_visible, (last_visible + input.row_buffer + 1).min(input.row_count))
        };
        let first_row = first_visible.saturating_sub(input.row_buffer);
        let frozen_rows = input.frozen_rows.min(input.row_count);

        let mut frozen_columns = Vec::new();
        let mut scrollable_columns = Vec::new();
        let mut frozen_x = 0.0;
        let mut scrollable_x = frozen_columns_width - scroll_x;

        for (index, column) in normalized_columns.iter().enumerate() {
            let is_frozen = index < explicit_frozen || column.frozen;
            if is_frozen {
                let layout = DataGridLayoutColumn {
                    index,
                    x: frozen_x,
                    width: column.width,
                    frozen: true,
                };
                frozen_x += column.width;
                if layout.rect(0.0, height).intersects_x(0.0, width) {
                    frozen_columns.push(layout);
                }
            } else {
                let layout = DataGridLayoutColumn {
                    index,
                    x: scrollable_x,
                    width: column.width,
                    frozen: false,
                };
                scrollable_x += column.width;
                if layout
                    .rect(0.0, height)
                    .intersects_x(frozen_columns_width, width)
                {
                    scrollable_columns.push(layout);
                }
            }
        }

        Self {
            viewport,
            header_rect,
            body_rect,
            frozen_columns_width,
            total_columns_width,
            total_rows_height,
            max_scroll_x,
            max_scroll_y,
            scroll_x,
            scroll_y,
            first_row,
            last_row_exclusive,
            frozen_rows,
            frozen_columns,
            scrollable_columns,
            rows,
        }
    }

    pub fn hit_test_column_resize(
        &self,
        x: f32,
        y: f32,
        handle_radius: f32,
    ) -> Option<DataGridColumnResizeHit> {
        if y < self.header_rect.y || y > self.viewport.max_y() {
            return None;
        }
        let mut best: Option<DataGridColumnResizeHit> = None;
        let mut best_distance = handle_radius.max(1.0);
        for column in self
            .frozen_columns
            .iter()
            .chain(self.scrollable_columns.iter())
        {
            let edge_x = column.x + column.width;
            if edge_x <= 0.0 || edge_x >= self.viewport.max_x() {
                continue;
            }
            let distance = (x - edge_x).abs();
            if distance <= best_distance {
                best_distance = distance;
                best = Some(DataGridColumnResizeHit {
                    column_index: column.index,
                    edge_x,
                });
            }
        }
        best
    }

    pub fn hit_test_row_resize(
        &self,
        x: f32,
        y: f32,
        _row_height: f32,
        handle_radius: f32,
    ) -> Option<DataGridRowResizeHit> {
        if x < self.viewport.x || x > self.viewport.max_x() {
            return None;
        }
        let radius = handle_radius.max(1.0);
        if (y - self.header_rect.max_y()).abs() <= radius {
            return Some(DataGridRowResizeHit {
                row_index: None,
                edge_y: self.header_rect.max_y(),
            });
        }
        if y < self.body_rect.y || y > self.body_rect.max_y() {
            return None;
        }

        let first = self.first_row;
        let last = self.last_row_exclusive;
        for row_index in first..last {
            let edge_y = self.body_rect.y + self.rows.top(row_index + 1) - self.scroll_y;
            if edge_y < self.body_rect.y || edge_y > self.body_rect.max_y() {
                continue;
            }
            if (y - edge_y).abs() <= radius {
                return Some(DataGridRowResizeHit {
                    row_index: Some(row_index),
                    edge_y,
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `RowHeightOverrides` numbers rows from 1, as COBOL does, and survives
    /// a round trip; junk is skipped, heights are kept in range.
    #[test]
    fn row_height_overrides_read_and_write() {
        let m = parse_row_height_overrides("1=40; 8=64,junk,0=50\n3=2000");
        assert_eq!(m.into_iter().collect::<Vec<_>>(), vec![(0, 40.0), (2, 400.0), (7, 64.0)]);
        let m = parse_row_height_overrides("1=40;8=64");
        assert_eq!(format_row_height_overrides(&m), "1=40;8=64");
    }

    /// Rows of different heights: tops accumulate, a y finds its row, and the
    /// layout scrolls and culls by them.
    #[test]
    fn rows_of_different_heights_lay_out_by_their_own_heights() {
        let g = RowGeometry::new(4, 20.0, &[0.0, 60.0]);
        assert_eq!((g.top(0), g.top(1), g.top(2), g.top(3), g.total()), (0.0, 20.0, 80.0, 100.0, 120.0));
        assert_eq!((g.index_at(19.9), g.index_at(20.0), g.index_at(79.0), g.index_at(500.0)), (0, 1, 1, 4));
        let mut heights = vec![20.0; 100];
        heights[50] = 300.0;
        let layout = DataGridLayout::compute(&DataGridLayoutInput {
            width: 200.0,
            height: 220.0,
            row_count: 100,
            columns: vec![DataGridColumnMeasure::new(200.0)],
            row_height: 20.0,
            row_heights: heights,
            header_height: 20.0,
            frozen_columns: 0,
            frozen_rows: 0,
            scroll_x: 0.0,
            scroll_y: 50.0 * 20.0 + 10.0,
            row_buffer: 0,
        });
        assert_eq!(layout.total_rows_height, 99.0 * 20.0 + 300.0);
        assert_eq!(layout.first_row, 50, "inside the tall row");
        assert_eq!(layout.last_row_exclusive, 51, "which fills the whole body");
    }

    #[test]
    fn datagrid_layout_virtualizes_large_row_sets_023() {
        let layout = DataGridLayout::compute(&DataGridLayoutInput {
            width: 800.0,
            height: 240.0,
            row_count: 100_000,
            columns: vec![DataGridColumnMeasure::new(120.0); 6],
            row_height: 20.0,
            row_heights: Vec::new(),
            header_height: 24.0,
            frozen_columns: 0,
            frozen_rows: 0,
            scroll_x: 0.0,
            scroll_y: 10_000.0,
            row_buffer: 2,
        });

        assert!(layout.first_row > 400);
        assert!(layout.last_row_exclusive < 530);
        assert!(layout.last_row_exclusive - layout.first_row <= 16);
        assert_eq!(layout.header_rect.h, 24.0);
        assert_eq!(layout.body_rect.y, 24.0);
        assert!(layout.max_scroll_y > 1_900_000.0);
    }

    #[test]
    fn datagrid_layout_clamps_scroll_and_body_bounds_023() {
        let layout = DataGridLayout::compute(&DataGridLayoutInput {
            width: 320.0,
            height: 100.0,
            row_count: 3,
            columns: vec![DataGridColumnMeasure::new(80.0); 2],
            row_height: 24.0,
            row_heights: Vec::new(),
            header_height: 30.0,
            frozen_columns: 0,
            frozen_rows: 0,
            scroll_x: 99_999.0,
            scroll_y: 99_999.0,
            row_buffer: 2,
        });

        assert_eq!(layout.body_rect.max_y(), 100.0);
        assert_eq!(layout.scroll_y, 2.0);
        assert_eq!(layout.first_row, 0);
        assert_eq!(layout.last_row_exclusive, 3);
        assert_eq!(layout.scroll_x, 0.0);
    }

    #[test]
    fn datagrid_layout_keeps_frozen_columns_aligned_023() {
        let layout = DataGridLayout::compute(&DataGridLayoutInput {
            width: 300.0,
            height: 160.0,
            row_count: 20,
            columns: vec![
                DataGridColumnMeasure::new(90.0),
                DataGridColumnMeasure::new(100.0),
                DataGridColumnMeasure::new(110.0),
                DataGridColumnMeasure::new(120.0),
            ],
            row_height: 20.0,
            row_heights: Vec::new(),
            header_height: 24.0,
            frozen_columns: 1,
            frozen_rows: 1,
            scroll_x: 80.0,
            scroll_y: 0.0,
            row_buffer: 1,
        });

        assert_eq!(layout.frozen_rows, 1);
        assert_eq!(layout.frozen_columns.len(), 1);
        assert_eq!(layout.frozen_columns[0].index, 0);
        assert_eq!(layout.frozen_columns[0].x, 0.0);
        assert_eq!(layout.frozen_columns_width, 90.0);
        assert!(layout.max_scroll_x > 0.0);
        assert!(layout.scrollable_columns.iter().all(|c| !c.frozen));
        assert!(layout
            .scrollable_columns
            .iter()
            .all(|c| c.x + c.width > layout.frozen_columns_width));
    }

    #[test]
    fn datagrid_virtual_scroll_limits_render_window_023() {
        let layout = DataGridLayout::compute(&DataGridLayoutInput {
            width: 1024.0,
            height: 360.0,
            row_count: 100_000,
            columns: vec![DataGridColumnMeasure::new(128.0); 12],
            row_height: 24.0,
            row_heights: Vec::new(),
            header_height: 24.0,
            frozen_columns: 2,
            frozen_rows: 1,
            scroll_x: 512.0,
            scroll_y: 250_000.0,
            row_buffer: 2,
        });

        assert_eq!(layout.frozen_rows, 1);
        assert_eq!(layout.frozen_columns.len(), 2);
        assert!(layout.max_scroll_y > 2_000_000.0);
        assert!(layout.scroll_y <= layout.max_scroll_y);
        assert!(layout.first_row > 10_000);
        assert!(layout.last_row_exclusive - layout.first_row <= 19);
        assert!(layout.scrollable_columns.len() < 12);
    }

    #[test]
    fn datagrid_virtual_scroll_keeps_frozen_columns_visible_023() {
        let layout = DataGridLayout::compute(&DataGridLayoutInput {
            width: 260.0,
            height: 180.0,
            row_count: 500,
            columns: vec![
                DataGridColumnMeasure::new(80.0),
                DataGridColumnMeasure::new(90.0),
                DataGridColumnMeasure::new(100.0),
                DataGridColumnMeasure::new(110.0),
                DataGridColumnMeasure::new(120.0),
            ],
            row_height: 18.0,
            row_heights: Vec::new(),
            header_height: 24.0,
            frozen_columns: 1,
            frozen_rows: 0,
            scroll_x: 300.0,
            scroll_y: 700.0,
            row_buffer: 1,
        });

        assert_eq!(layout.frozen_columns[0].index, 0);
        assert_eq!(layout.frozen_columns[0].x, 0.0);
        assert!(layout.scroll_x > 0.0);
        assert!(layout
            .scrollable_columns
            .iter()
            .all(|column| column.index > 0));
        assert!(layout
            .scrollable_columns
            .iter()
            .all(|column| column.x + column.width > layout.frozen_columns_width));
    }

    #[test]
    fn datagrid_resize_hit_tests_column_edges_023() {
        let layout = DataGridLayout::compute(&DataGridLayoutInput {
            width: 360.0,
            height: 160.0,
            row_count: 10,
            columns: vec![
                DataGridColumnMeasure::new(80.0),
                DataGridColumnMeasure::new(120.0),
                DataGridColumnMeasure::new(140.0),
            ],
            row_height: 22.0,
            row_heights: Vec::new(),
            header_height: 24.0,
            frozen_columns: 1,
            frozen_rows: 0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            row_buffer: 1,
        });

        let hit = layout
            .hit_test_column_resize(80.5, 12.0, 4.0)
            .expect("column edge should be hit");
        assert_eq!(hit.column_index, 0);
        assert_eq!(hit.edge_x, 80.0);
        assert!(layout.hit_test_column_resize(50.0, 12.0, 4.0).is_none());
    }

    #[test]
    fn datagrid_resize_hit_tests_row_edges_023() {
        let layout = DataGridLayout::compute(&DataGridLayoutInput {
            width: 300.0,
            height: 150.0,
            row_count: 20,
            columns: vec![DataGridColumnMeasure::new(100.0); 3],
            row_height: 20.0,
            row_heights: Vec::new(),
            header_height: 24.0,
            frozen_columns: 0,
            frozen_rows: 0,
            scroll_x: 0.0,
            scroll_y: 40.0,
            row_buffer: 1,
        });

        let header_hit = layout
            .hit_test_row_resize(100.0, 24.5, 20.0, 4.0)
            .expect("header edge should be hit");
        assert_eq!(header_hit.row_index, None);

        let row_hit = layout
            .hit_test_row_resize(100.0, 44.0, 20.0, 4.0)
            .expect("visible row edge should be hit");
        assert_eq!(row_hit.row_index, Some(2));
        assert!(layout.hit_test_row_resize(100.0, 31.0, 20.0, 2.0).is_none());
    }

    #[test]
    fn datagrid_selection_copy_cell_mode_023() {
        let rows = vec![
            vec!["1".into(), "Leonardo DiCaprio".into(), "30000000".into()],
            vec!["2".into(), "Joe Pesci".into(), "12000000".into()],
        ];
        let visible_columns = vec![0, 1, 2];

        assert_eq!(
            datagrid_copy_text(
                &rows,
                &visible_columns,
                DataGridCellSelection {
                    row_index: 1,
                    display_column_index: 1,
                },
                "Cell",
                ",",
            ),
            Some("Joe Pesci".into())
        );
    }

    #[test]
    fn datagrid_selection_copy_row_mode_023() {
        let rows = vec![vec![
            "1".into(),
            "assets/images/photo000000001.jpg".into(),
            "Leonardo DiCaprio".into(),
            "30000000".into(),
        ]];
        let visible_columns = vec![0, 2, 3];

        assert_eq!(
            datagrid_copy_text(
                &rows,
                &visible_columns,
                DataGridCellSelection {
                    row_index: 0,
                    display_column_index: 2,
                },
                "Row",
                "\t",
            ),
            Some("1\tLeonardo DiCaprio\t30000000".into())
        );
    }

    use crate::model::DataGridWidthUnit::{Percent, Points};

    /// Points and percentages in one grid, which is the whole point: the narrow
    /// predictable columns hold their size and the prose column takes a share.
    #[test]
    fn points_and_percentages_mix_in_one_grid() {
        // 1000 wide: a 120pt code column, a 50% description, a 80pt date.
        let w = resolve_column_widths(&[(120.0, Points), (50.0, Percent), (80.0, Points)], 1000.0, false);
        assert_eq!(w, vec![120.0, 500.0, 80.0]);
    }

    /// A percentage follows the grid, a points column does not.
    #[test]
    fn a_percentage_column_follows_the_grid_width() {
        let cols = [(100.0, Points), (25.0, Percent)];
        assert_eq!(resolve_column_widths(&cols, 400.0, false), vec![100.0, 100.0]);
        assert_eq!(resolve_column_widths(&cols, 800.0, false), vec![100.0, 200.0]);
    }

    /// Adaptive off is today's behaviour: the columns keep what they asked for
    /// and the grid scrolls or leaves a gap. This is the compatibility pin.
    #[test]
    fn without_auto_fit_the_columns_keep_their_declared_widths() {
        let cols = [(120.0, Points), (120.0, Points)];
        assert_eq!(resolve_column_widths(&cols, 1000.0, false), vec![120.0, 120.0]);
        assert_eq!(resolve_column_widths(&cols, 100.0, false), vec![120.0, 120.0]);
    }

    /// Adaptive on: the columns fill the grid exactly.
    #[test]
    fn auto_fit_makes_the_columns_fill_the_grid() {
        for available in [300.0_f32, 640.0, 1000.0, 1913.0] {
            let w = resolve_column_widths(
                &[(120.0, Points), (30.0, Percent), (200.0, Points)],
                available,
                true,
            );
            let total: f32 = w.iter().sum();
            assert!(
                (total - available).abs() < 0.5,
                "available={available}: columns summed to {total} ({w:?})"
            );
        }
    }

    /// …and the percentage column still gets exactly the share it asked for.
    /// The surplus is the points columns' business, not its.
    #[test]
    fn auto_fit_leaves_a_percentage_column_its_declared_share() {
        let w = resolve_column_widths(&[(100.0, Points), (40.0, Percent)], 1000.0, true);
        assert_eq!(w[1], 400.0, "the 40% column moved: {w:?}");
        assert_eq!(w[0], 600.0, "the points column did not absorb the rest: {w:?}");
    }

    /// A grid that is all percentages has nothing else to absorb the
    /// difference, so it spreads across them.
    #[test]
    fn auto_fit_with_only_percentages_spreads_across_them() {
        // 30 + 30 = 60% of 1000 = 600, leaving 400 to distribute.
        let w = resolve_column_widths(&[(30.0, Percent), (30.0, Percent)], 1000.0, true);
        let total: f32 = w.iter().sum();
        assert!((total - 1000.0).abs() < 0.5, "got {w:?}");
        assert!((w[0] - w[1]).abs() < 0.5, "equal declarations must stay equal: {w:?}");
    }

    /// No column is ever drawn too thin to grab, whatever the percentage says.
    #[test]
    fn no_column_is_drawn_below_the_minimum() {
        let w = resolve_column_widths(&[(1.0, Percent), (1.0, Percent), (98.0, Percent)], 200.0, false);
        for (i, got) in w.iter().enumerate() {
            assert!(*got >= MIN_COLUMN_WIDTH, "column {i} came out at {got}");
        }
    }

    /// The zero-width first frame must not turn every percentage into nothing.
    #[test]
    fn a_grid_with_no_width_yet_keeps_its_numbers() {
        let w = resolve_column_widths(&[(120.0, Points), (50.0, Percent)], 0.0, true);
        assert_eq!(w, vec![120.0, 50.0]);
    }
}
