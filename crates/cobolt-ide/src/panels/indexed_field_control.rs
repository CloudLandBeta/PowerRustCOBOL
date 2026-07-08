// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Compact Form Designer–style controls for the indexed grid edit row (AC8).

use cobolt_forms::ControlType;
use cobolt_indexed::{
    encode_field_display, encode_indicator_bool, format_field_display, indicator_bool, parse_pic,
    IndexedField, PicCategory,
};

use crate::i18n::Tr;

/// One editable cell in the grid edit buffer.
#[derive(Debug, Clone)]
pub enum EditCell {
    Text(String),
    Bool(bool),
    Number(i64),
}

impl EditCell {
    pub fn from_field(field: &IndexedField, row: &[u8]) -> Self {
        let control = field
            .grid_control
            .clone()
            .unwrap_or_else(|| cobolt_indexed::default_control_for_field(field));
        let len = field.length.unwrap_or(1) as usize;
        let off = field.offset.unwrap_or(0) as usize;
        if off >= row.len() {
            match control {
                ControlType::CheckBox => return EditCell::Bool(false),
                ControlType::NumericUpDown => return EditCell::Number(0),
                _ => return EditCell::Text(String::new()),
            }
        }
        let end = (off + len).min(row.len());
        let slice = &row[off..end];
        match control {
            ControlType::CheckBox => EditCell::Bool(indicator_bool(slice)),
            ControlType::NumericUpDown => {
                let s = format_field_display(field, slice);
                let n = s.parse::<i64>().unwrap_or(0);
                EditCell::Number(n)
            }
            _ => EditCell::Text(format_field_display(field, slice)),
        }
    }

    pub fn display_text(&self, field: &IndexedField, tr: &Tr) -> String {
        match self {
            EditCell::Text(s) => s.clone(),
            EditCell::Bool(b) => {
                if *b {
                    tr.lbl_grid_yes.to_string()
                } else {
                    tr.lbl_grid_no.to_string()
                }
            }
            EditCell::Number(n) => n.to_string(),
        }
    }
}

/// Format a grid cell for read-only display (AC7).
pub fn format_cell(field: &IndexedField, row: &[u8], tr: &Tr) -> String {
    let len = field.length.unwrap_or(1) as usize;
    let off = field.offset.unwrap_or(0) as usize;
    if off >= row.len() {
        return String::new();
    }
    let end = (off + len).min(row.len());
    let slice = &row[off..end];
    let control = field
        .grid_control
        .clone()
        .unwrap_or_else(|| cobolt_indexed::default_control_for_field(field));
    if matches!(control, ControlType::CheckBox)
        || parse_pic(&field.pic).category == PicCategory::Indicator
    {
        if indicator_bool(slice) {
            return tr.lbl_grid_yes.to_string();
        }
        return tr.lbl_grid_no.to_string();
    }
    format_field_display(field, slice)
}

/// Render the edit control; returns true when the cell changed.
pub fn show_edit_control(
    ui: &mut egui::Ui,
    field: &IndexedField,
    cell: &mut EditCell,
    tr: &Tr,
) -> bool {
    let control = field
        .grid_control
        .clone()
        .unwrap_or_else(|| cobolt_indexed::default_control_for_field(field));
    match control {
        ControlType::CheckBox => {
            if !matches!(cell, EditCell::Bool(_)) {
                *cell = EditCell::Bool(false);
            }
            if let EditCell::Bool(b) = cell {
                ui.checkbox(b, "").changed()
            } else {
                false
            }
        }
        ControlType::NumericUpDown => {
            if !matches!(cell, EditCell::Number(_)) {
                let n = match cell {
                    EditCell::Text(s) => s.parse().unwrap_or(0),
                    EditCell::Number(n) => *n,
                    EditCell::Bool(b) => i64::from(*b),
                };
                *cell = EditCell::Number(n);
            }
            if let EditCell::Number(n) = cell {
                ui.add(egui::DragValue::new(n).speed(1)).changed()
            } else {
                false
            }
        }
        ControlType::DateTimePicker => {
            if !matches!(cell, EditCell::Text(_)) {
                *cell = EditCell::Text(cell.display_text(field, tr));
            }
            if let EditCell::Text(s) = cell {
                ui.add(egui::TextEdit::singleline(s).desired_width(300.0))
                    .changed()
            } else {
                false
            }
        }
        _ => {
            if !matches!(cell, EditCell::Text(_)) {
                *cell = EditCell::Text(cell.display_text(field, tr));
            }
            if let EditCell::Text(s) = cell {
                ui.add(egui::TextEdit::singleline(s).desired_width(300.0))
                    .changed()
            } else {
                false
            }
        }
    }
}

/// Validate and encode all edit cells into a full record buffer.
pub fn build_record(
    cells: &[EditCell],
    fields: &[&IndexedField],
    record_len: usize,
    tr: &Tr,
) -> Result<Vec<u8>, String> {
    let mut rec = vec![b' '; record_len];
    for (cell, field) in cells.iter().zip(fields.iter()) {
        let len = field.length.unwrap_or(1) as usize;
        let off = field.offset.unwrap_or(0) as usize;
        if off + len > rec.len() {
            continue;
        }
        let encoded = match cell {
            EditCell::Bool(b) => {
                let control = field
                    .grid_control
                    .clone()
                    .unwrap_or_else(|| cobolt_indexed::default_control_for_field(field));
                if matches!(control, ControlType::CheckBox)
                    || parse_pic(&field.pic).category == PicCategory::Indicator
                {
                    Ok(encode_indicator_bool(*b, len))
                } else {
                    encode_field_display(field, if *b { "1" } else { "0" }, len)
                        .map_err(|e| format!("{}: {}", field.name, e.as_str()))
                }
            }
            EditCell::Number(n) => {
                let s = n.to_string();
                encode_field_display(field, &s, len)
                    .map_err(|e| format!("{}: {}", field.name, e.as_str()))
            }
            EditCell::Text(s) => encode_field_display(field, s, len)
                .map_err(|e| format!("{}: {}", field.name, e.as_str())),
        }?;
        rec[off..off + len].copy_from_slice(&encoded);
    }
    let _ = tr;
    Ok(rec)
}
