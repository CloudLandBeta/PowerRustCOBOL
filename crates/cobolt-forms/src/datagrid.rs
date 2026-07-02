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
        let total_rows_height = input.row_count as f32 * row_height;
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

        let first_visible = (scroll_y / row_height).floor().max(0.0) as usize;
        let first_row = first_visible.saturating_sub(input.row_buffer);
        let visible_capacity = (body_height / row_height).ceil().max(0.0) as usize;
        let last_row_exclusive =
            (first_visible + visible_capacity + input.row_buffer + 1).min(input.row_count);
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
        row_height: f32,
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
            let edge_y = self.body_rect.y + row_height * (row_index + 1) as f32 - self.scroll_y;
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

    #[test]
    fn datagrid_layout_virtualizes_large_row_sets_023() {
        let layout = DataGridLayout::compute(&DataGridLayoutInput {
            width: 800.0,
            height: 240.0,
            row_count: 100_000,
            columns: vec![DataGridColumnMeasure::new(120.0); 6],
            row_height: 20.0,
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
}
