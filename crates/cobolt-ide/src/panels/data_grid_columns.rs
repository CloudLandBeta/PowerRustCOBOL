// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DataGrid Column Editor state helpers.
//!
//! The UI modal is intentionally built on this small state layer so column
//! width/order/style/filter/gauge/freeze edits mutate the same advanced metadata
//! consumed by the shared renderer and runtime.

use cobolt_forms::model::{
    DataGridAdvanced, DataGridCellFrame, DataGridColumn, DataGridGauge, DataGridGridLineStyle,
    PropValue, DATAGRID_ADVANCED_PROP,
};
use cobolt_forms::Control;

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct DataGridColumnEditorState {
    pub control_id: String,
    pub advanced: DataGridAdvanced,
}

#[allow(dead_code)]
impl DataGridColumnEditorState {
    pub fn from_control(control: &Control) -> Self {
        Self {
            control_id: control.id.clone(),
            advanced: DataGridAdvanced::from_control(control),
        }
    }

    pub fn set_column_width(&mut self, column_index: usize, width: f32) {
        self.advanced.set_column_width(column_index, width);
    }

    pub fn move_column_left(&mut self, column_index: usize) -> bool {
        self.advanced.move_column_left(column_index)
    }

    pub fn move_column_right(&mut self, column_index: usize) -> bool {
        self.advanced.move_column_right(column_index)
    }

    pub fn set_grid_line_style(&mut self, style: DataGridGridLineStyle) {
        self.advanced.grid_line_style = style;
    }

    pub fn set_frozen_panes(&mut self, columns: usize, rows: usize) {
        self.advanced.frozen_columns = columns;
        self.advanced.frozen_rows = rows;
    }

    pub fn set_filter(&mut self, column_id: impl Into<String>, value: impl Into<String>) {
        self.advanced.set_filter(column_id, value);
    }

    pub fn enable_frame(&mut self, column_index: usize, corner_radius: u16) {
        if let Some(column) = self.advanced.columns.get_mut(column_index) {
            column.frame = Some(DataGridCellFrame {
                enabled: true,
                corner_radius,
                ..DataGridCellFrame::default()
            });
        }
    }

    pub fn enable_gauge(&mut self, column_index: usize, min: f64, max: f64) {
        if let Some(column) = self.advanced.columns.get_mut(column_index) {
            column.gauge = Some(DataGridGauge {
                enabled: true,
                min,
                max,
                ..DataGridGauge::default()
            });
        }
    }

    pub fn apply_to_control(&self, control: &mut Control) {
        if let Ok(json) = self.advanced.to_json() {
            control.set_prop(DATAGRID_ADVANCED_PROP, PropValue::String(json));
            control.set_prop(
                "GridLineStyle",
                PropValue::String(self.advanced.grid_line_style.as_str().into()),
            );
            control.set_prop(
                "FrozenColumns",
                PropValue::Int(self.advanced.frozen_columns as i64),
            );
            control.set_prop(
                "FrozenRows",
                PropValue::Int(self.advanced.frozen_rows as i64),
            );
        }
    }

    pub fn columns(&self) -> &[DataGridColumn] {
        &self.advanced.columns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::{ControlType, PropValue};

    #[test]
    fn datagrid_column_editor_mutates_advanced_metadata_023() {
        let mut grid = Control::new("ActorGrid", ControlType::DataGrid, 0, 0);
        grid.set_prop(
            "Columns",
            PropValue::String("ACTOR-ID:number\nSTATUS:string\nSALARY:number".into()),
        );
        let mut editor = DataGridColumnEditorState::from_control(&grid);

        editor.set_column_width(1, 180.0);
        editor.enable_frame(1, 14);
        editor.enable_gauge(2, 0.0, 30_000_000.0);
        editor.set_filter("STATUS", "Active");
        editor.set_grid_line_style(DataGridGridLineStyle::Dots);
        editor.set_frozen_panes(1, 1);
        assert!(editor.move_column_left(1));
        assert!(editor.move_column_right(0));
        assert!(editor.move_column_left(1));
        assert_eq!(editor.columns()[0].source_name, "STATUS");
        editor.apply_to_control(&mut grid);

        let parsed = DataGridAdvanced::from_control(&grid);
        assert_eq!(parsed.columns[0].source_name, "STATUS");
        assert_eq!(parsed.columns[0].width, 180.0);
        assert_eq!(parsed.columns[0].frame.as_ref().unwrap().corner_radius, 14);
        assert!(parsed.columns[2].gauge.as_ref().unwrap().enabled);
        assert_eq!(parsed.filters[0].column_id, "STATUS");
        assert_eq!(parsed.grid_line_style, DataGridGridLineStyle::Dots);
        assert_eq!(parsed.frozen_columns, 1);
        assert_eq!(parsed.frozen_rows, 1);
    }
}
