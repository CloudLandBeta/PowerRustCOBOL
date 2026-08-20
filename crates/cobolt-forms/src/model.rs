// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Core data model: `Form`, `Control`, `ControlType`, `EventBinding`, `AnimationDef`.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

// ── Geometry ──────────────────────────────────────────────────────────────────

/// Bounding rectangle of a control in form-space pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

impl Default for Rect {
    fn default() -> Self {
        Self::new(0, 0, 100, 30)
    }
}

// ── PropValue ─────────────────────────────────────────────────────────────────

/// The value of a control property.
#[derive(Debug, Clone, PartialEq)]
pub enum PropValue {
    String(String),
    Int(i64),
    Bool(bool),
}

impl PropValue {
    pub fn as_str(&self) -> &str {
        if let PropValue::String(s) = self {
            s
        } else {
            ""
        }
    }
    pub fn as_i64(&self) -> i64 {
        match self {
            PropValue::Int(n) => *n,
            PropValue::Bool(b) => *b as i64,
            PropValue::String(s) => s.parse().unwrap_or(0),
        }
    }
    pub fn as_bool(&self) -> bool {
        match self {
            PropValue::Bool(b) => *b,
            PropValue::Int(n) => *n != 0,
            PropValue::String(s) => !s.is_empty() && s != "0" && s != "false",
        }
    }
    pub fn to_xml_string(&self) -> String {
        match self {
            PropValue::String(s) => s.clone(),
            PropValue::Int(n) => n.to_string(),
            PropValue::Bool(b) => {
                if *b {
                    "1".to_owned()
                } else {
                    "0".to_owned()
                }
            }
        }
    }
}

impl From<&str> for PropValue {
    fn from(s: &str) -> Self {
        PropValue::String(s.to_owned())
    }
}
impl From<String> for PropValue {
    fn from(s: String) -> Self {
        PropValue::String(s)
    }
}
impl From<i64> for PropValue {
    fn from(n: i64) -> Self {
        PropValue::Int(n)
    }
}
impl From<i32> for PropValue {
    fn from(n: i32) -> Self {
        PropValue::Int(n as i64)
    }
}
impl From<bool> for PropValue {
    fn from(b: bool) -> Self {
        PropValue::Bool(b)
    }
}

impl std::fmt::Display for PropValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_xml_string())
    }
}

// ── Advanced DataGrid model (spec 023) ───────────────────────────────────────

pub const DATAGRID_ADVANCED_PROP: &str = "AdvancedGrid";
pub const DATAGRID_ADVANCED_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataGridCsvExportMode {
    Filtered,
    AllRows,
}

impl Default for DataGridCsvExportMode {
    fn default() -> Self {
        Self::Filtered
    }
}

impl DataGridCsvExportMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Filtered => "Filtered",
            Self::AllRows => "AllRows",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "all" | "allrows" | "all_rows" => Self::AllRows,
            _ => Self::Filtered,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataGridGridLineStyle {
    Solid,
    Dash,
    Dots,
    None,
}

impl Default for DataGridGridLineStyle {
    fn default() -> Self {
        Self::Solid
    }
}

impl DataGridGridLineStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Solid => "Solid",
            Self::Dash => "Dash",
            Self::Dots => "Dots",
            Self::None => "None",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "dash" | "dashed" => Self::Dash,
            "dot" | "dots" | "dotted" => Self::Dots,
            "none" | "off" | "false" => Self::None,
            _ => Self::Solid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataGridTextAlignment {
    Left,
    Center,
    Right,
}

impl Default for DataGridTextAlignment {
    fn default() -> Self {
        Self::Left
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataGridCellFrame {
    pub enabled: bool,
    pub background_color: String,
    pub foreground_color: String,
    pub padding: u16,
    pub corner_radius: u16,
    #[serde(default = "DataGridCellFrame::default_shape")]
    pub shape: String,
}

impl Default for DataGridCellFrame {
    fn default() -> Self {
        Self {
            enabled: false,
            background_color: "#1BC47D".into(),
            foreground_color: "#FFFFFF".into(),
            padding: 6,
            corner_radius: 8,
            shape: Self::default_shape(),
        }
    }
}

impl DataGridCellFrame {
    fn default_shape() -> String {
        "Pill".into()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataGridGauge {
    pub enabled: bool,
    pub min: f64,
    pub max: f64,
    pub fill_color: String,
    pub background_color: String,
    pub show_text: bool,
}

impl Default for DataGridGauge {
    fn default() -> Self {
        Self {
            enabled: false,
            min: 0.0,
            max: 100.0,
            fill_color: "#3F86F5".into(),
            background_color: "#22314A".into(),
            show_text: true,
        }
    }
}

impl DataGridGauge {
    pub fn fraction_for_value(&self, value: &str) -> Option<f32> {
        if !self.enabled {
            return None;
        }
        let parsed = value.trim().parse::<f64>().ok()?;
        let span = self.max - self.min;
        if span.abs() <= f64::EPSILON {
            return Some(0.0);
        }
        Some(((parsed - self.min) / span).clamp(0.0, 1.0) as f32)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataGridValueStyleRule {
    pub value: String,
    pub foreground_color: String,
    pub background_color: String,
    pub frame_background_color: String,
    pub frame_foreground_color: String,
}

impl Default for DataGridValueStyleRule {
    fn default() -> Self {
        Self {
            value: String::new(),
            foreground_color: "#FFFFFF".into(),
            background_color: "#00000000".into(),
            frame_background_color: "#1BC47D".into(),
            frame_foreground_color: "#FFFFFF".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataGridColumn {
    pub id: String,
    pub title: String,
    pub source_name: String,
    pub value_type: String,
    #[serde(default)]
    pub cobol_mask: String,
    #[serde(default = "DataGridColumn::default_edit_control")]
    pub edit_control: String,
    #[serde(default)]
    pub control_kind: String,
    pub width: f32,
    pub visible: bool,
    pub frozen: bool,
    pub filter_enabled: bool,
    pub sort_enabled: bool,
    #[serde(default = "DataGridColumn::default_header_font_size")]
    pub header_font_size: u16,
    pub text_alignment: DataGridTextAlignment,
    pub foreground_color: String,
    pub background_color: String,
    #[serde(default)]
    pub background_pattern: String,
    #[serde(default)]
    pub background_image: String,
    /// Corner radius (px) applied to an `Image` edit-control cell's picture.
    #[serde(default)]
    pub image_corner_radius: f32,
    /// Draw a soft drop shadow behind an `Image` edit-control cell's picture.
    #[serde(default)]
    pub image_shadow: bool,
    #[serde(default)]
    pub font_name: String,
    #[serde(default)]
    pub font_size: u16,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    pub frame: Option<DataGridCellFrame>,
    pub gauge: Option<DataGridGauge>,
    pub value_style_rules: Vec<DataGridValueStyleRule>,
}

impl Default for DataGridColumn {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            source_name: String::new(),
            value_type: "string".into(),
            cobol_mask: String::new(),
            edit_control: Self::default_edit_control(),
            control_kind: String::new(),
            width: 120.0,
            visible: true,
            frozen: false,
            filter_enabled: false,
            sort_enabled: true,
            header_font_size: Self::default_header_font_size(),
            text_alignment: DataGridTextAlignment::Left,
            foreground_color: "#000000".into(),
            background_color: "#00000000".into(),
            background_pattern: String::new(),
            background_image: String::new(),
            image_corner_radius: 0.0,
            image_shadow: false,
            font_name: String::new(),
            font_size: 0,
            bold: false,
            italic: false,
            underline: false,
            frame: None,
            gauge: None,
            value_style_rules: Vec::new(),
        }
    }
}

impl DataGridColumn {
    fn default_edit_control() -> String {
        "Textbox".into()
    }

    fn default_header_font_size() -> u16 {
        12
    }

    pub fn value_style_rule_for(&self, value: &str) -> Option<&DataGridValueStyleRule> {
        self.value_style_rules
            .iter()
            .find(|rule| rule.value.eq_ignore_ascii_case(value.trim()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataGridRowHeightOverride {
    pub row_index: usize,
    pub height: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataGridFilter {
    pub column_id: String,
    pub value: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataGridAdvanced {
    pub schema_version: u16,
    pub columns: Vec<DataGridColumn>,
    pub frozen_columns: usize,
    pub frozen_rows: usize,
    pub row_height: u16,
    pub row_overrides: Vec<DataGridRowHeightOverride>,
    pub filters: Vec<DataGridFilter>,
    pub csv_export_mode: DataGridCsvExportMode,
    pub csv_delimiter: String,
    pub grid_line_style: DataGridGridLineStyle,
    pub selectable_text: bool,
}

impl Default for DataGridAdvanced {
    fn default() -> Self {
        Self {
            schema_version: DATAGRID_ADVANCED_SCHEMA_VERSION,
            columns: Vec::new(),
            frozen_columns: 0,
            frozen_rows: 0,
            row_height: 22,
            row_overrides: Vec::new(),
            filters: Vec::new(),
            csv_export_mode: DataGridCsvExportMode::Filtered,
            csv_delimiter: ",".into(),
            grid_line_style: DataGridGridLineStyle::Solid,
            selectable_text: true,
        }
    }
}

impl DataGridAdvanced {
    pub fn from_control(control: &Control) -> Self {
        if let Some(raw) = control
            .get_prop(DATAGRID_ADVANCED_PROP)
            .map(PropValue::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if let Ok(mut advanced) = serde_json::from_str::<Self>(raw) {
                if advanced.schema_version == 0 {
                    advanced.schema_version = DATAGRID_ADVANCED_SCHEMA_VERSION;
                }
                advanced.apply_runtime_overrides(control);
                return advanced;
            }
        }

        let mut advanced = Self::default();
        advanced.apply_property_defaults(control);
        advanced.columns = control
            .get_prop("Columns")
            .map(PropValue::as_str)
            .map(Self::columns_from_legacy_property)
            .unwrap_or_default();
        advanced.apply_runtime_overrides(control);
        advanced
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn set_column_width(&mut self, column_index: usize, width: f32) {
        if let Some(column) = self.columns.get_mut(column_index) {
            column.width = width.max(32.0);
        }
    }

    pub fn column_width(&self, column_index: usize) -> Option<f32> {
        self.columns
            .get(column_index)
            .map(|column| column.width.max(1.0))
    }

    pub fn set_column_width_by_key(&mut self, column_key: &str, width: f32) -> bool {
        if let Some(index) = self.column_index_for_key(column_key) {
            self.set_column_width(index, width);
            return true;
        }
        false
    }

    pub fn move_column(&mut self, from: usize, to: usize) -> bool {
        if from >= self.columns.len() || to >= self.columns.len() || from == to {
            return false;
        }
        let column = self.columns.remove(from);
        self.columns.insert(to, column);
        true
    }

    pub fn move_column_left(&mut self, column_index: usize) -> bool {
        if column_index == 0 {
            return false;
        }
        self.move_column(column_index, column_index - 1)
    }

    pub fn move_column_right(&mut self, column_index: usize) -> bool {
        self.move_column(column_index, column_index + 1)
    }

    pub fn set_filter(&mut self, column_id: impl Into<String>, value: impl Into<String>) {
        let column_id = column_id.into();
        let value = value.into();
        if let Some(filter) = self
            .filters
            .iter_mut()
            .find(|filter| filter.column_id.eq_ignore_ascii_case(&column_id))
        {
            filter.value = value;
            filter.active = !filter.value.trim().is_empty();
            return;
        }
        self.filters.push(DataGridFilter {
            column_id,
            active: !value.trim().is_empty(),
            value,
        });
    }

    pub fn filtered_row_indices(&self, rows: &[Vec<String>]) -> Vec<usize> {
        let source_names: Vec<String> = self
            .columns
            .iter()
            .map(|column| column.source_name.clone())
            .collect();
        self.filtered_row_indices_for_sources(rows, &source_names)
    }

    pub fn filtered_row_indices_for_sources(
        &self,
        rows: &[Vec<String>],
        source_names: &[String],
    ) -> Vec<usize> {
        let active_filters: Vec<&DataGridFilter> = self
            .filters
            .iter()
            .filter(|filter| filter.active && !filter.value.trim().is_empty())
            .collect();
        if active_filters.is_empty() {
            return (0..rows.len()).collect();
        }

        rows.iter()
            .enumerate()
            .filter_map(|(row_index, row)| {
                let keep = active_filters.iter().all(|filter| {
                    let Some(column_index) =
                        self.source_index_for_filter(filter.column_id.as_str(), source_names)
                    else {
                        return false;
                    };
                    row.get(column_index)
                        .map(|value| {
                            value
                                .to_ascii_lowercase()
                                .contains(&filter.value.to_ascii_lowercase())
                        })
                        .unwrap_or(false)
                });
                keep.then_some(row_index)
            })
            .collect()
    }

    fn source_index_for_filter(&self, column_id: &str, source_names: &[String]) -> Option<usize> {
        let column = self.columns.iter().find(|column| {
            column.id.eq_ignore_ascii_case(column_id)
                || column.source_name.eq_ignore_ascii_case(column_id)
                || column.title.eq_ignore_ascii_case(column_id)
        })?;
        source_names.iter().position(|source_name| {
            source_name.eq_ignore_ascii_case(&column.source_name)
                || source_name.eq_ignore_ascii_case(&column.title)
                || source_name.eq_ignore_ascii_case(&column.id)
        })
    }

    fn apply_property_defaults(&mut self, control: &Control) {
        self.row_height = control
            .get_prop("RowHeight")
            .map(PropValue::as_i64)
            .filter(|h| *h > 0)
            .map(|h| h.min(u16::MAX as i64) as u16)
            .unwrap_or(self.row_height);
        self.frozen_columns = control
            .get_prop("FrozenColumns")
            .map(PropValue::as_i64)
            .filter(|n| *n >= 0)
            .map(|n| n as usize)
            .unwrap_or(self.frozen_columns);
        self.frozen_rows = control
            .get_prop("FrozenRows")
            .map(PropValue::as_i64)
            .filter(|n| *n >= 0)
            .map(|n| n as usize)
            .unwrap_or(self.frozen_rows);
        self.csv_export_mode = control
            .get_prop("CSVExportMode")
            .map(PropValue::as_str)
            .map(DataGridCsvExportMode::from_str)
            .unwrap_or(self.csv_export_mode);
        self.csv_delimiter = control
            .get_prop("CSVDelimiter")
            .map(PropValue::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.csv_delimiter)
            .to_string();
        self.grid_line_style = control
            .get_prop("GridLineStyle")
            .map(PropValue::as_str)
            .map(DataGridGridLineStyle::from_str)
            .unwrap_or(self.grid_line_style);
        self.selectable_text = control
            .get_prop("SelectableText")
            .map(PropValue::as_bool)
            .unwrap_or(self.selectable_text);
    }

    fn apply_runtime_overrides(&mut self, control: &Control) {
        if let Some(height) = control
            .get_prop("_RuntimeRowHeight")
            .map(PropValue::as_i64)
            .filter(|height| *height > 0)
        {
            self.row_height = height.min(u16::MAX as i64) as u16;
        }
        if let Some(columns) = control
            .get_prop("_RuntimeFrozenColumns")
            .map(PropValue::as_i64)
            .filter(|columns| *columns >= 0)
        {
            self.frozen_columns = columns as usize;
        }
        if let Some(rows) = control
            .get_prop("_RuntimeFrozenRows")
            .map(PropValue::as_i64)
            .filter(|rows| *rows >= 0)
        {
            self.frozen_rows = rows as usize;
        }
        if let Some(raw) = control
            .get_prop("_RuntimeColumnFilters")
            .map(PropValue::as_str)
        {
            self.filters.clear();
            for line in raw.lines() {
                let Some((column, value)) = line.split_once('=') else {
                    continue;
                };
                let column = column.trim();
                if !column.is_empty() {
                    self.set_filter(column, value.trim());
                }
            }
        }
        if let Some(raw) = control
            .get_prop("_RuntimeColumnWidths")
            .map(PropValue::as_str)
        {
            for line in raw.lines() {
                let Some((column, width)) = line.split_once('=') else {
                    continue;
                };
                let Ok(width) = width.trim().parse::<f32>() else {
                    continue;
                };
                self.set_column_width_by_key(column.trim(), width);
            }
        }
    }

    fn column_index_for_key(&self, column_key: &str) -> Option<usize> {
        let trimmed = column_key.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Ok(index) = trimmed.parse::<usize>() {
            if index < self.columns.len() {
                return Some(index);
            }
            let one_based = index.saturating_sub(1);
            if one_based < self.columns.len() {
                return Some(one_based);
            }
        }
        self.columns.iter().position(|column| {
            column.id.eq_ignore_ascii_case(trimmed)
                || column.source_name.eq_ignore_ascii_case(trimmed)
                || column.title.eq_ignore_ascii_case(trimmed)
        })
    }

    fn columns_from_legacy_property(raw: &str) -> Vec<DataGridColumn> {
        raw.lines()
            .filter_map(|line| {
                let spec = line.trim();
                if spec.is_empty() {
                    return None;
                }
                let (name, value_type) = spec
                    .split_once(':')
                    .map(|(name, ty)| (name.trim(), ty.trim()))
                    .unwrap_or((spec, "string"));
                if name.is_empty() {
                    return None;
                }
                let id = name
                    .chars()
                    .map(|ch| {
                        if ch.is_ascii_alphanumeric() {
                            ch.to_ascii_uppercase()
                        } else {
                            '_'
                        }
                    })
                    .collect();
                Some(DataGridColumn {
                    id,
                    title: name.to_string(),
                    source_name: name.to_string(),
                    value_type: if value_type.is_empty() {
                        "string".into()
                    } else {
                        value_type.to_ascii_lowercase()
                    },
                    ..DataGridColumn::default()
                })
            })
            .collect()
    }
}

// ── Animation ─────────────────────────────────────────────────────────────────

/// What event triggers an animation.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimTrigger {
    OnFormLoad,
    OnShow,
    OnHide,
    OnClick,
    OnHover,
    OnFocus,
    Programmatic,    // invoked by name via COBOL PERFORM or code
    OnTimer(String), // a specific Timer control ID fires it
}

impl AnimTrigger {
    pub fn as_str(&self) -> &str {
        match self {
            AnimTrigger::OnFormLoad => "OnFormLoad",
            AnimTrigger::OnShow => "OnShow",
            AnimTrigger::OnHide => "OnHide",
            AnimTrigger::OnClick => "OnClick",
            AnimTrigger::OnHover => "OnHover",
            AnimTrigger::OnFocus => "OnFocus",
            AnimTrigger::Programmatic => "Programmatic",
            AnimTrigger::OnTimer(_) => "OnTimer",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "OnFormLoad" => AnimTrigger::OnFormLoad,
            "OnShow" => AnimTrigger::OnShow,
            "OnHide" => AnimTrigger::OnHide,
            "OnClick" => AnimTrigger::OnClick,
            "OnHover" => AnimTrigger::OnHover,
            "OnFocus" => AnimTrigger::OnFocus,
            "Programmatic" => AnimTrigger::Programmatic,
            _ => AnimTrigger::OnFormLoad,
        }
    }
    pub const ALL: &'static [&'static str] = &[
        "OnFormLoad",
        "OnShow",
        "OnHide",
        "OnClick",
        "OnHover",
        "OnFocus",
        "Programmatic",
        "OnTimer",
    ];
}

/// The animation motion/effect kind.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimKind {
    None,
    FlyFromLeft,
    FlyFromRight,
    FlyFromTop,
    FlyFromBottom,
    FlyFromTopLeft,
    FlyFromTopRight,
    FlyFromBottomLeft,
    FlyFromBottomRight,
    FadeIn,
    FadeOut,
    ZoomIn,
    ZoomOut,
    Bounce,
    Shake,
    Pulse,
    Spin,
    Flip,
    Slide { dx: i32, dy: i32 },
    Custom(String), // name of a keyframe set defined in project
}

impl AnimKind {
    pub fn as_str(&self) -> &str {
        match self {
            AnimKind::None => "None",
            AnimKind::FlyFromLeft => "FlyFromLeft",
            AnimKind::FlyFromRight => "FlyFromRight",
            AnimKind::FlyFromTop => "FlyFromTop",
            AnimKind::FlyFromBottom => "FlyFromBottom",
            AnimKind::FlyFromTopLeft => "FlyFromTopLeft",
            AnimKind::FlyFromTopRight => "FlyFromTopRight",
            AnimKind::FlyFromBottomLeft => "FlyFromBottomLeft",
            AnimKind::FlyFromBottomRight => "FlyFromBottomRight",
            AnimKind::FadeIn => "FadeIn",
            AnimKind::FadeOut => "FadeOut",
            AnimKind::ZoomIn => "ZoomIn",
            AnimKind::ZoomOut => "ZoomOut",
            AnimKind::Bounce => "Bounce",
            AnimKind::Shake => "Shake",
            AnimKind::Pulse => "Pulse",
            AnimKind::Spin => "Spin",
            AnimKind::Flip => "Flip",
            AnimKind::Slide { .. } => "Slide",
            AnimKind::Custom(n) => n.as_str(),
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "None" => AnimKind::None,
            "FlyFromLeft" => AnimKind::FlyFromLeft,
            "FlyFromRight" => AnimKind::FlyFromRight,
            "FlyFromTop" => AnimKind::FlyFromTop,
            "FlyFromBottom" => AnimKind::FlyFromBottom,
            "FlyFromTopLeft" => AnimKind::FlyFromTopLeft,
            "FlyFromTopRight" => AnimKind::FlyFromTopRight,
            "FlyFromBottomLeft" => AnimKind::FlyFromBottomLeft,
            "FlyFromBottomRight" => AnimKind::FlyFromBottomRight,
            "FadeIn" => AnimKind::FadeIn,
            "FadeOut" => AnimKind::FadeOut,
            "ZoomIn" => AnimKind::ZoomIn,
            "ZoomOut" => AnimKind::ZoomOut,
            "Bounce" => AnimKind::Bounce,
            "Shake" => AnimKind::Shake,
            "Pulse" => AnimKind::Pulse,
            "Spin" => AnimKind::Spin,
            "Flip" => AnimKind::Flip,
            _ => AnimKind::None,
        }
    }
    pub const ALL: &'static [&'static str] = &[
        "None",
        "FlyFromLeft",
        "FlyFromRight",
        "FlyFromTop",
        "FlyFromBottom",
        "FlyFromTopLeft",
        "FlyFromTopRight",
        "FlyFromBottomLeft",
        "FlyFromBottomRight",
        "FadeIn",
        "FadeOut",
        "ZoomIn",
        "ZoomOut",
        "Bounce",
        "Shake",
        "Pulse",
        "Spin",
        "Flip",
    ];
}

/// Easing function for animations.
#[derive(Debug, Clone, PartialEq)]
pub enum EasingKind {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Bounce,
    Elastic,
    Back,
    Spring,
}

impl EasingKind {
    pub fn as_str(&self) -> &str {
        match self {
            EasingKind::Linear => "Linear",
            EasingKind::EaseIn => "EaseIn",
            EasingKind::EaseOut => "EaseOut",
            EasingKind::EaseInOut => "EaseInOut",
            EasingKind::Bounce => "Bounce",
            EasingKind::Elastic => "Elastic",
            EasingKind::Back => "Back",
            EasingKind::Spring => "Spring",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "Linear" => EasingKind::Linear,
            "EaseIn" => EasingKind::EaseIn,
            "EaseOut" => EasingKind::EaseOut,
            "EaseInOut" => EasingKind::EaseInOut,
            "Bounce" => EasingKind::Bounce,
            "Elastic" => EasingKind::Elastic,
            "Back" => EasingKind::Back,
            "Spring" => EasingKind::Spring,
            _ => EasingKind::EaseOut,
        }
    }
    pub const ALL: &'static [&'static str] = &[
        "Linear",
        "EaseIn",
        "EaseOut",
        "EaseInOut",
        "Bounce",
        "Elastic",
        "Back",
        "Spring",
    ];
    /// Evaluate easing at t ∈ [0,1].
    pub fn apply(&self, t: f32) -> f32 {
        match self {
            EasingKind::Linear => t,
            EasingKind::EaseIn => t * t,
            EasingKind::EaseOut => t * (2.0 - t),
            EasingKind::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
            EasingKind::Bounce => {
                let t = 1.0 - t;
                let r = if t < 1.0 / 2.75 {
                    7.5625 * t * t
                } else if t < 2.0 / 2.75 {
                    let t = t - 1.5 / 2.75;
                    7.5625 * t * t + 0.75
                } else if t < 2.5 / 2.75 {
                    let t = t - 2.25 / 2.75;
                    7.5625 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / 2.75;
                    7.5625 * t * t + 0.984375
                };
                1.0 - r
            }
            EasingKind::Elastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    2.0_f32.powf(-10.0 * t) * ((t - 0.075) * std::f32::consts::TAU / 0.3).sin()
                        + 1.0
                }
            }
            EasingKind::Back => {
                let c = 1.70158;
                t * t * ((c + 1.0) * t - c)
            }
            EasingKind::Spring => {
                // damped spring approximation
                (1.0 - (-6.0 * t).exp() * (8.0 * t).cos()).clamp(0.0, 1.0)
            }
        }
    }
}

/// How many times an animation repeats.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimRepeat {
    Once,
    Loop,
    PingPong,
    Count(u32),
}

impl AnimRepeat {
    pub fn as_str(&self) -> &str {
        match self {
            AnimRepeat::Once => "Once",
            AnimRepeat::Loop => "Loop",
            AnimRepeat::PingPong => "PingPong",
            AnimRepeat::Count(_) => "Count",
        }
    }
    pub const ALL: &'static [&'static str] = &["Once", "Loop", "PingPong", "Count"];
}

/// A single animation definition attached to a control or form.
#[derive(Debug, Clone)]
pub struct AnimationDef {
    /// Unique name for this animation (used by COBOL PERFORM to trigger it).
    pub name: String,
    pub trigger: AnimTrigger,
    pub kind: AnimKind,
    pub duration_ms: u64,
    pub delay_ms: u64,
    pub easing: EasingKind,
    pub repeat: AnimRepeat,
    /// When kind=Slide, the pixel offset from which the control enters.
    pub slide_dx: i32,
    pub slide_dy: i32,
}

impl AnimationDef {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            trigger: AnimTrigger::OnFormLoad,
            kind: AnimKind::FlyFromLeft,
            duration_ms: 400,
            delay_ms: 0,
            easing: EasingKind::EaseOut,
            repeat: AnimRepeat::Once,
            slide_dx: 0,
            slide_dy: 0,
        }
    }
}

// ── EventBinding ──────────────────────────────────────────────────────────────

/// Maps a UI event to a COBOL nested-program handler.
///
/// `paragraph` is auto-derived via `derive_paragraph_name()` as `"CTRL-ID--EVENT-NAME"`.
///
/// `code` holds the **complete editable handler body** — from `ENVIRONMENT
/// DIVISION` through the `PROCEDURE DIVISION` and its statements. The code
/// generator only wraps it with the `IDENTIFICATION DIVISION` / `PROGRAM-ID`
/// header and the closing `GOBACK` / `END PROGRAM`. When `code` is empty the
/// handler is considered unwritten and the editor opens it with a fresh
/// [`event_handler_template`].
#[derive(Debug, Clone, PartialEq)]
pub struct EventBinding {
    pub event: String,
    /// The handler's nested-program name (`CONTROL-ID--EVENTNAME`), auto-derived.
    /// One nested COBOL program per event (PowerCOBOL 3 paradigm). The field —
    /// and its `.cfrm` XML attribute — keep the historical name "paragraph"
    /// for file-format compatibility only.
    pub paragraph: String,
    /// CDATA body — the full handler source (`ENVIRONMENT DIVISION` …
    /// `PROCEDURE DIVISION` + statements). Empty means "no handler yet".
    pub code: String,
}

impl EventBinding {
    /// Create a new binding with an empty code body.
    pub fn new(event: impl Into<String>, paragraph: impl Into<String>) -> Self {
        Self {
            event: event.into(),
            paragraph: paragraph.into(),
            code: String::new(),
        }
    }

    /// Create a binding and derive the paragraph name automatically from
    /// the control ID and event name: `"BTN-OK--CLICK"`.
    pub fn for_control(control_id: &str, event: impl Into<String>) -> Self {
        let ev = event.into();
        let para = derive_paragraph_name(control_id, &ev);
        Self {
            event: ev,
            paragraph: para,
            code: String::new(),
        }
    }

    /// True if the user has written any code in this handler.
    pub fn has_code(&self) -> bool {
        !self.code.trim().is_empty()
    }

    /// Count non-blank lines in the code body (for UI display: "3 lines").
    pub fn code_line_count(&self) -> usize {
        self.code.lines().filter(|l| !l.trim().is_empty()).count()
    }
}

/// The canonical property names a control of `type_name` exposes — exactly the
/// keys [`Control::new`] populates, so the editor's IntelliSense always reflects
/// what the runtime can `SET`/`GET` with no separate catalogue to drift.
/// `type_name` is the `ControlType` debug name (e.g. `"Button"`, `"ListBox"`).
pub fn property_names_for(type_name: &str) -> Vec<String> {
    let ct = ControlType::from_str(type_name);
    let mut names: Vec<String> = Control::new("_", ct, 0, 0)
        .properties
        .keys()
        .cloned()
        .collect();
    // Field-backed properties: stored on the `Control` struct (not the property
    // map) but still settable/gettable by name via `set_property`.
    for f in [
        "Name", "Visible", "Enabled", "X", "Y", "Width", "Height", "TabOrder", "Parent", "Tab",
    ] {
        names.push(f.to_string());
    }
    names.sort_unstable();
    names.dedup();
    names
}

/// The LINKAGE-SECTION data items and the matching `PROCEDURE DIVISION USING`
/// names for one event — i.e. the data the runtime delivers to the handler.
///
/// Returns `(linkage_items, using_names)`:
/// * `linkage_items` — COBOL `01/05` declarations placed in the LINKAGE SECTION
///   (each line already indented), or empty when the event carries no data.
/// * `using_names`    — the operands for `PROCEDURE DIVISION USING …`.
///
/// Most events carry no data, so the default is empty. This is the single
/// extension point: an event that delivers, say, the clicked node index would
/// return
///
/// ```text
/// ("       01 COBOL-EVENT-DATA.\n            05 COBOL-ARRAY-INDEX PIC S9(9) COMP-5.\n",
///  vec!["COBOL-ARRAY-INDEX".into()])
/// ```
///
/// which the template renders as a LINKAGE SECTION block plus
/// `PROCEDURE DIVISION USING COBOL-ARRAY-INDEX.`
pub fn event_linkage(_event: &str) -> (String, Vec<String>) {
    // No event delivers data to its handler yet; the mechanism is in place so
    // events can declare their payload here as the runtime gains support.
    (String::new(), Vec::new())
}

/// Build the first-time handler skeleton shown in the editor (and emitted as the
/// stub for an unwritten handler). It spans `ENVIRONMENT DIVISION` through
/// `PROCEDURE DIVISION` — the part the developer owns — with the event's data
/// (if any) declared in the LINKAGE SECTION and bound via `USING`.
pub fn event_handler_template(event: &str) -> String {
    build_event_handler_template(event, None)
}

/// Like [`event_handler_template`] but for a control that belongs to a repeating
/// group (control array). The handler additionally receives the **1-based array
/// index** of the item that fired (`CONTROL-ARRAY-INDEX`), so it can address that
/// item's controls as `Name(CONTROL-ARRAY-INDEX)::Property`.
pub fn event_handler_template_indexed(event: &str, control_id: &str) -> String {
    build_event_handler_template(event, Some(control_id))
}

fn build_event_handler_template(event: &str, array_control: Option<&str>) -> String {
    let (linkage_items, using) = event_linkage(event);

    let mut using_names: Vec<String> = Vec::new();
    if array_control.is_some() {
        using_names.push("CONTROL-ARRAY-INDEX".to_string());
    }
    using_names.extend(using.iter().cloned());
    let using_clause = if using_names.is_empty() {
        String::new()
    } else {
        format!(" USING {}", using_names.join(" "))
    };

    let mut t = String::new();
    t.push_str("       ENVIRONMENT DIVISION.\n");
    t.push_str("       DATA DIVISION.\n");
    t.push_str("       WORKING-STORAGE SECTION.\n");
    t.push_str("       LINKAGE SECTION.\n");
    if array_control.is_some() {
        t.push_str("       01 CONTROL-ARRAY-INDEX              PIC S9(4) COMP-5.\n");
    }
    let items = linkage_items.trim_end();
    if !items.is_empty() {
        for line in items.lines() {
            t.push_str(line);
            t.push('\n');
        }
    }
    t.push('\n');
    t.push_str(&format!("       PROCEDURE DIVISION{using_clause}.\n"));
    if let Some(control_id) = array_control {
        t.push_str(
            "      *>    This control belongs to a repeating group (array). \
             CONTROL-ARRAY-INDEX\n",
        );
        t.push_str(
            "      *>    is the 1-based index of the item that fired — address that item's\n",
        );
        t.push_str("      *>    controls as Name(CONTROL-ARRAY-INDEX)::Property, e.g.\n");
        t.push_str(&format!(
            "      *>    DISPLAY {control_id}(CONTROL-ARRAY-INDEX)::BackgroundColor\n"
        ));
    }
    t.push_str("           CONTINUE.\n");
    t
}

/// Derive the nested-program name for an event handler.
/// e.g. control_id="BTN-OK", event="Click"  →  "BTN-OK--CLICK"
pub fn derive_paragraph_name(control_id: &str, event: &str) -> String {
    format!(
        "{}--{}",
        control_id.to_ascii_uppercase(),
        event.to_ascii_uppercase().replace(' ', "-")
    )
}

/// A control id valid as a COBOL paragraph-name prefix / member-access root:
/// starts with a letter, then letters / digits / hyphens.
///
/// `me` and `super` are reserved (any case): both are object receivers in
/// RustCOBOL member access (spec 037 D4 / spec 049 R28-R30), so a control
/// under either name could never be addressed.
pub fn is_valid_control_id(id: &str) -> bool {
    if id.eq_ignore_ascii_case("me") || id.eq_ignore_ascii_case("super") {
        return false;
    }
    let mut chars = id.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn is_id_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

/// Every control id `code` addresses as `receiver::member`, uppercased and
/// deduplicated. The `::` is what makes a name a control reference — a bare
/// word could be any COBOL identifier.
pub fn control_refs_in_code(code: &str) -> Vec<String> {
    let bytes = code.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if bytes[i] == b':' && bytes[i + 1] == b':' {
            let mut start = i;
            while start > 0 && is_id_byte(bytes[start - 1]) {
                start -= 1;
            }
            if start < i {
                let name = code[start..i].to_ascii_uppercase();
                if !out.contains(&name) {
                    out.push(name);
                }
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    out
}

/// Is `word` mentioned in `code` as a whole word (case-insensitive)? Used to
/// tell a procedure that something still calls from one nothing refers to.
fn code_mentions_word(code: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let hay = code.to_ascii_uppercase();
    let needle = word.to_ascii_uppercase();
    let (hay, needle) = (hay.as_bytes(), needle.as_bytes());
    let mut i = 0usize;
    while i + needle.len() <= hay.len() {
        if &hay[i..i + needle.len()] == needle {
            let before = i == 0 || !is_id_byte(hay[i - 1]);
            let j = i + needle.len();
            let after = j >= hay.len() || !is_id_byte(hay[j]);
            if before && after {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Rewrite control references in handler/procedure COBOL where the control id is
/// a member-access root: `Old::…` or `Old(idx)…`. Case-insensitive on the id;
/// requires a non-identifier byte before it so `Old` inside a longer name is
/// left alone. Bytes outside matches are copied verbatim (UTF-8 safe — matches
/// only ever start on ASCII bytes).
fn rename_control_refs_in_code(code: &mut String, old: &str, new: &str) {
    let old_up = old.to_ascii_uppercase();
    let hay_up = code.to_ascii_uppercase();
    if old_up.is_empty() || !hay_up.contains(&old_up) {
        return;
    }
    let bytes = code.as_bytes();
    let hay = hay_up.as_bytes();
    let needle = old_up.as_bytes();
    let mut out = String::with_capacity(code.len());
    let mut i = 0usize;
    let mut last = 0usize;
    while i + needle.len() <= bytes.len() {
        if &hay[i..i + needle.len()] == needle {
            let before_ok = i == 0 || !is_id_byte(bytes[i - 1]);
            let j = i + needle.len();
            let after_ok = (j + 1 < bytes.len() && &hay[j..j + 2] == b"::")
                || (j < bytes.len() && bytes[j] == b'(');
            if before_ok && after_ok {
                out.push_str(&code[last..i]);
                out.push_str(new);
                i = j;
                last = j;
                continue;
            }
        }
        i += 1;
    }
    if last == 0 {
        return;
    }
    out.push_str(&code[last..]);
    *code = out;
}

/// Update every control-id reference inside a data binding (source control,
/// target control / array / members, and each mapping's target).
fn rename_binding_control_refs(binding: &mut DataBindingDef, old: &str, new: &str) {
    let ren = |s: &mut String| {
        if s.eq_ignore_ascii_case(old) {
            *s = new.to_owned();
        }
    };
    match &mut binding.source {
        BindingSourceDescriptor::Sql {
            source_control_id, ..
        }
        | BindingSourceDescriptor::RestApi {
            source_control_id, ..
        }
        | BindingSourceDescriptor::AgentAi {
            source_control_id, ..
        } => ren(source_control_id),
        _ => {}
    }
    match &mut binding.target {
        BindingTargetDescriptor::DataGrid { control_id }
        | BindingTargetDescriptor::Chart { control_id, .. }
        | BindingTargetDescriptor::ComboBox { control_id }
        | BindingTargetDescriptor::ListBox { control_id }
        | BindingTargetDescriptor::ScalarControl { control_id }
        | BindingTargetDescriptor::MarkerCollection { control_id } => ren(control_id),
        BindingTargetDescriptor::ControlArray {
            array_id,
            member_control_ids,
        } => {
            ren(array_id);
            for m in member_control_ids.iter_mut() {
                ren(m);
            }
        }
    }
    for mapping in &mut binding.mappings {
        match &mut mapping.target {
            BindingTargetPath::GridColumn { control_id, .. }
            | BindingTargetPath::ChartCategory { control_id }
            | BindingTargetPath::ChartValueSeries { control_id, .. }
            | BindingTargetPath::ChartSeriesLabel { control_id, .. }
            | BindingTargetPath::ListDisplayItem { control_id }
            | BindingTargetPath::ListValue { control_id }
            | BindingTargetPath::ScalarValue { control_id }
            | BindingTargetPath::MarkerField { control_id, .. } => ren(control_id),
            BindingTargetPath::ControlProperty {
                array_id,
                control_id,
                ..
            } => {
                ren(array_id);
                ren(control_id);
            }
        }
    }
}

fn is_legacy_groupbox_generated_caption(value: &str) -> bool {
    let value = value.trim();
    let Some(suffix) = value
        .strip_prefix("GroupBox-")
        .or_else(|| value.strip_prefix("Groupbox-"))
        .or_else(|| value.strip_prefix("groupbox-"))
    else {
        return false;
    };
    !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit())
}

// ── DeletedControlCode ────────────────────────────────────────────────────────

/// Preserves event code from a control that was deleted by the user.
/// Stored in the .cfrm XML under <deleted-controls> so it can be recovered.
/// Never emitted into the generated .cbl.
#[derive(Debug, Clone, PartialEq)]
pub struct DeletedControlCode {
    /// Original control ID (e.g. "BTN-OK").
    pub control_id: String,
    /// ISO 8601 timestamp of when the control was deleted.
    pub deleted_at: String,
    /// All event bindings that had code at the time of deletion.
    pub events: Vec<EventBinding>,
}

// ── Data binding ─────────────────────────────────────────────────────────────

pub const DATA_BINDING_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingMode {
    ReadOnly,
    Writable,
}

impl Default for BindingMode {
    fn default() -> Self {
        Self::ReadOnly
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingSourceKind {
    IndexedFile,
    Sql,
    CobolTable,
    RestApi,
    AgentAi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingDataType {
    Text,
    Integer,
    Decimal,
    Boolean,
    Date,
    DateTime,
    Json,
    Unknown,
}

impl Default for BindingDataType {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingField {
    pub name: String,
    pub display_name: String,
    pub data_type: BindingDataType,
    #[serde(default)]
    pub cobol_mask: String,
    #[serde(default = "BindingField::default_edit_control")]
    pub edit_control: String,
    pub nullable: bool,
    pub key: bool,
    pub multiple: bool,
}

impl BindingField {
    pub fn new(name: impl Into<String>, data_type: BindingDataType) -> Self {
        let name = name.into();
        Self {
            display_name: name.clone(),
            name,
            data_type,
            cobol_mask: String::new(),
            edit_control: Self::default_edit_control(),
            nullable: true,
            key: false,
            multiple: false,
        }
    }

    fn default_edit_control() -> String {
        "Textbox".into()
    }

    pub fn key(mut self) -> Self {
        self.key = true;
        self.nullable = false;
        self
    }

    pub fn required(mut self) -> Self {
        self.nullable = false;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BindingUpdateMetadata {
    pub request_schema_name: String,
    pub key_fields: Vec<String>,
    pub approved_target_ids: Vec<String>,
}

impl BindingUpdateMetadata {
    pub fn new(request_schema_name: impl Into<String>, key_fields: Vec<String>) -> Self {
        Self {
            request_schema_name: request_schema_name.into(),
            key_fields,
            approved_target_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingSourceDescriptor {
    IndexedFile {
        definition_path: String,
        record_name: String,
        fields: Vec<BindingField>,
        key_field: Option<String>,
        writable: bool,
    },
    Sql {
        source_control_id: String,
        query_name: String,
        result_set_name: String,
        fields: Vec<BindingField>,
        key_fields: Vec<String>,
        writable: bool,
    },
    CobolTable {
        table_name: String,
        occurs_item: String,
        fields: Vec<BindingField>,
        key_fields: Vec<String>,
        writable: bool,
    },
    RestApi {
        source_control_id: String,
        endpoint_name: String,
        response_data_item: String,
        fields: Vec<BindingField>,
        update: Option<BindingUpdateMetadata>,
    },
    AgentAi {
        source_control_id: String,
        output_name: String,
        fields: Vec<BindingField>,
        update: Option<BindingUpdateMetadata>,
    },
}

impl BindingSourceDescriptor {
    pub fn kind(&self) -> BindingSourceKind {
        match self {
            BindingSourceDescriptor::IndexedFile { .. } => BindingSourceKind::IndexedFile,
            BindingSourceDescriptor::Sql { .. } => BindingSourceKind::Sql,
            BindingSourceDescriptor::CobolTable { .. } => BindingSourceKind::CobolTable,
            BindingSourceDescriptor::RestApi { .. } => BindingSourceKind::RestApi,
            BindingSourceDescriptor::AgentAi { .. } => BindingSourceKind::AgentAi,
        }
    }

    pub fn fields(&self) -> &[BindingField] {
        match self {
            BindingSourceDescriptor::IndexedFile { fields, .. }
            | BindingSourceDescriptor::Sql { fields, .. }
            | BindingSourceDescriptor::CobolTable { fields, .. }
            | BindingSourceDescriptor::RestApi { fields, .. }
            | BindingSourceDescriptor::AgentAi { fields, .. } => fields,
        }
    }

    pub fn is_writable(&self) -> bool {
        match self {
            BindingSourceDescriptor::IndexedFile { writable, .. }
            | BindingSourceDescriptor::Sql { writable, .. }
            | BindingSourceDescriptor::CobolTable { writable, .. } => *writable,
            BindingSourceDescriptor::RestApi { update, .. }
            | BindingSourceDescriptor::AgentAi { update, .. } => update.is_some(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingChartKind {
    Bar,
    Line,
    Pie,
    Area,
    Scatter,
    Donut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovedBindingTargetKind {
    DataGrid,
    Chart(BindingChartKind),
    ComboBox,
    ListBox,
    ControlArray,
    /// A standalone Knob/Gauge/Switch (spec 039 R21) — no control array
    /// required, unlike every target above spec 022 originally approved.
    ScalarControl,
    /// A `Maps` control's `Markers` collection (spec 039 R22) — one source
    /// field per marker attribute (lat, lng, label, and optionally id/info),
    /// the same shape as `DataGrid` binding `Rows`/`Columns`.
    MarkerCollection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingTargetDescriptor {
    DataGrid {
        control_id: String,
    },
    Chart {
        control_id: String,
        chart_kind: BindingChartKind,
    },
    ComboBox {
        control_id: String,
    },
    ListBox {
        control_id: String,
    },
    ControlArray {
        array_id: String,
        member_control_ids: Vec<String>,
    },
    /// A standalone Knob/Gauge/Switch (spec 039 R21) — binds one source
    /// field to the control's own scalar `Value` (Knob/Gauge) or `Checked`
    /// (Switch); which property depends on `control_id`'s `ControlType` at
    /// resolution time, not on this descriptor (mirrors how `Chart`'s
    /// `chart_kind` is carried but `DataGrid`/`ComboBox`/`ListBox` derive
    /// their shape from the control itself).
    ScalarControl {
        control_id: String,
    },
    /// A `Maps` control's `Markers` collection (spec 039 R22).
    MarkerCollection {
        control_id: String,
    },
}

impl BindingTargetDescriptor {
    pub fn primary_control_id(&self) -> &str {
        match self {
            BindingTargetDescriptor::DataGrid { control_id }
            | BindingTargetDescriptor::Chart { control_id, .. }
            | BindingTargetDescriptor::ComboBox { control_id }
            | BindingTargetDescriptor::ListBox { control_id }
            | BindingTargetDescriptor::ScalarControl { control_id }
            | BindingTargetDescriptor::MarkerCollection { control_id } => control_id,
            BindingTargetDescriptor::ControlArray { array_id, .. } => array_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingTargetPath {
    GridColumn {
        control_id: String,
        column_id: String,
    },
    ChartCategory {
        control_id: String,
    },
    ChartValueSeries {
        control_id: String,
        series_id: String,
    },
    ChartSeriesLabel {
        control_id: String,
        series_id: String,
    },
    ListDisplayItem {
        control_id: String,
    },
    ListValue {
        control_id: String,
    },
    ControlProperty {
        array_id: String,
        control_id: String,
        property_name: String,
    },
    /// A standalone Knob/Gauge/Switch's own scalar property (spec 039 R21)
    /// — `Value` for Knob/Gauge, `Checked` for Switch, resolved from
    /// `control_id`'s `ControlType` at apply time.
    ScalarValue {
        control_id: String,
    },
    /// One marker attribute of a `Maps` control (spec 039 R22) — `field`
    /// says which (`Lat`/`Lng`/`Label` required, `Id`/`Info` optional).
    MarkerField {
        control_id: String,
        field: MapMarkerField,
    },
}

impl BindingTargetPath {
    fn stable_key(&self) -> String {
        match self {
            BindingTargetPath::GridColumn {
                control_id,
                column_id,
            } => format!("grid:{control_id}:{column_id}"),
            BindingTargetPath::ChartCategory { control_id } => {
                format!("chart-category:{control_id}")
            }
            BindingTargetPath::ChartValueSeries {
                control_id,
                series_id,
            } => format!("chart-value:{control_id}:{series_id}"),
            BindingTargetPath::ChartSeriesLabel {
                control_id,
                series_id,
            } => format!("chart-label:{control_id}:{series_id}"),
            BindingTargetPath::ListDisplayItem { control_id } => {
                format!("list-display:{control_id}")
            }
            BindingTargetPath::ListValue { control_id } => format!("list-value:{control_id}"),
            BindingTargetPath::ControlProperty {
                array_id,
                control_id,
                property_name,
            } => format!("array:{array_id}:{control_id}:{property_name}"),
            BindingTargetPath::ScalarValue { control_id } => {
                format!("scalar:{control_id}")
            }
            BindingTargetPath::MarkerField { control_id, field } => {
                format!("marker:{control_id}:{}", field.as_str())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MappingCompatibility {
    Exact,
    CoercibleWarning,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldMapping {
    pub source_field: String,
    pub target: BindingTargetPath,
    pub compatibility: MappingCompatibility,
}

impl FieldMapping {
    pub fn new(source_field: impl Into<String>, target: BindingTargetPath) -> Self {
        Self {
            source_field: source_field.into(),
            target,
            compatibility: MappingCompatibility::Exact,
        }
    }

    fn stable_key(&self) -> String {
        format!("{}:{}", self.source_field, self.target.stable_key())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardianSeverity {
    Blocker,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardianFinding {
    pub severity: GuardianSeverity,
    pub code: String,
    pub message: String,
    pub binding_id: String,
    pub source_field: Option<String>,
    pub target_control_id: Option<String>,
}

impl GuardianFinding {
    pub fn new(
        severity: GuardianSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        binding_id: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            binding_id: binding_id.into(),
            source_field: None,
            target_control_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BindingValidationSnapshot {
    pub findings: Vec<GuardianFinding>,
    pub source_signature: String,
    pub validated_with_schema_version: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BindingSourceMetadata {
    pub fields: Vec<BindingField>,
    pub schema_text: String,
    pub sample_payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataBindingDef {
    pub schema_version: u16,
    pub id: String,
    pub display_name: String,
    pub source: BindingSourceDescriptor,
    pub target: BindingTargetDescriptor,
    pub mappings: Vec<FieldMapping>,
    pub mode: BindingMode,
    pub validation: BindingValidationSnapshot,
    pub saved_source_metadata: BindingSourceMetadata,
}

impl DataBindingDef {
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        source: BindingSourceDescriptor,
        target: BindingTargetDescriptor,
    ) -> Self {
        let source_fields = source.fields().to_vec();
        Self {
            schema_version: DATA_BINDING_SCHEMA_VERSION,
            id: id.into(),
            display_name: display_name.into(),
            source,
            target,
            mappings: Vec::new(),
            mode: BindingMode::ReadOnly,
            validation: BindingValidationSnapshot {
                validated_with_schema_version: DATA_BINDING_SCHEMA_VERSION,
                ..BindingValidationSnapshot::default()
            },
            saved_source_metadata: BindingSourceMetadata {
                fields: source_fields,
                ..BindingSourceMetadata::default()
            },
        }
    }

    pub fn with_mappings(mut self, mappings: Vec<FieldMapping>) -> Self {
        self.mappings = mappings;
        self
    }

    pub fn sorted_mapping_refs(&self) -> Vec<&FieldMapping> {
        let mut mappings: Vec<&FieldMapping> = self.mappings.iter().collect();
        mappings.sort_by_key(|mapping| mapping.stable_key());
        mappings
    }
}

// ── Maps marker model (spec 039) ────────────────────────────────────────────

/// A `Maps` control's marker attribute a data-binding mapping can target
/// (spec 039 T13/R22) — `Lat`/`Lng`/`Label` are required by the Guardian,
/// `Id`/`Info` are optional (an unmapped `Id` falls back to the row index,
/// an unmapped `Info` to an empty string).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MapMarkerField {
    Id,
    Lat,
    Lng,
    Label,
    Info,
}

impl MapMarkerField {
    pub fn as_str(&self) -> &'static str {
        match self {
            MapMarkerField::Id => "Id",
            MapMarkerField::Lat => "Lat",
            MapMarkerField::Lng => "Lng",
            MapMarkerField::Label => "Label",
            MapMarkerField::Info => "Info",
        }
    }

    pub const ALL: [MapMarkerField; 5] = [
        MapMarkerField::Id,
        MapMarkerField::Lat,
        MapMarkerField::Lng,
        MapMarkerField::Label,
        MapMarkerField::Info,
    ];
}

/// One marker in a `Maps` control's `Markers` property. Stored as one line
/// per marker, tab-separated (`id\tlat\tlng\tlabel\tinfo`) — the same
/// convention other multi-row properties already use (e.g. DataGrid's
/// `Rows`), rather than a new `PropValue` variant just for this.
#[derive(Debug, Clone, PartialEq)]
pub struct MapMarkerRecord {
    pub id: String,
    pub lat: f64,
    pub lng: f64,
    pub label: String,
    pub info: String,
}

/// Parse a `Markers` property's raw text. A malformed line (not enough
/// fields, or a lat/lng that fails to parse) is skipped rather than
/// aborting the whole list — one bad row from a partially-typed edit or a
/// bad data-bound value should not blank the rest of the map.
pub fn parse_map_markers(raw: &str) -> Vec<MapMarkerRecord> {
    raw.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(5, '\t');
            let id = parts.next()?.to_owned();
            let lat: f64 = parts.next()?.trim().parse().ok()?;
            let lng: f64 = parts.next()?.trim().parse().ok()?;
            let label = parts.next().unwrap_or("").to_owned();
            let info = parts.next().unwrap_or("").to_owned();
            Some(MapMarkerRecord {
                id,
                lat,
                lng,
                label,
                info,
            })
        })
        .collect()
}

/// The inverse of [`parse_map_markers`].
pub fn serialize_map_markers(markers: &[MapMarkerRecord]) -> String {
    markers
        .iter()
        .map(|m| format!("{}\t{}\t{}\t{}\t{}", m.id, m.lat, m.lng, m.label, m.info))
        .collect::<Vec<_>>()
        .join("\n")
}

// ── ControlType ───────────────────────────────────────────────────────────────

/// The type of a visual (or non-visual) control.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ControlType {
    // Core controls
    Button,
    TextBox,
    Label,
    CheckBox,
    RadioButton,
    ListBox,
    ComboBox,
    GroupBox,
    Panel,
    TabControl,
    DataGrid,
    PictureBox,
    ProgressBar,
    MenuBar,
    ToolBar,
    StatusBar,
    // Extended controls
    Line,
    DateTimePicker,
    NumericUpDown,
    TreeView,
    Splitter,
    Timer,
    Shape,
    // New controls
    Animator,    // Plays animated images (GIF / WebP / APNG)
    AgentObject, // AI Agent (non-visual) — connects to local LLM
    RestClient,  // REST API client (non-visual) — INVOKE-based HTTP calls
    SqlDatabase, // SQL database client (non-visual) — SQLx-backed open/query/fetch
    IndexedFile, // Indexed file object (non-visual) — generated COBOL file-method facade
    Slider,      // Horizontal or vertical slider with min/max/step/tick marks
    // Charts — each binds to a COBOL data structure (table/array) and supports INVOKE
    BarChart,     // Vertical / horizontal bar chart
    LineChart,    // Line / area line chart
    PieChart,     // Pie chart (360° sectors)
    AreaChart,    // Stacked or overlapping area chart
    ScatterChart, // Scatter / bubble plot
    DonutChart,   // Donut (ring) chart
    // Batch 039 (spec 039), phase 1: Knob/Gauge/Switch/FileDropZone.
    // Maps and WebSearch join this enum in later 039 tasks (T8, T14).
    Knob,         // Rotary dial setting a numeric Value within Minimum..Maximum
    Gauge,        // Read-only KPI display: Radial | Linear | Donut (GaugeStyle)
    Switch,       // Boolean On/Off visual toggle
    FileDropZone, // Drag-and-drop / click-to-browse file intake (non-visual result)
    // Batch 039, phase 2 (T8): embedded, pannable/zoomable OpenStreetMap
    // view + google_maps-backed location data (Directions/Geocoding/
    // Places/Distance-Matrix).
    Maps,
    // Batch 039, phase 3 (T14): non-visual Google Custom Search JSON API
    // client — INVOKE 'SEARCH' (T15), same async lifecycle as RestClient.
    WebSearch,
    // Spec 049: the application shell's sidebar menu. Deliberately a type of its
    // own rather than a mode of `MenuBar` — a form that already carries a
    // MenuBar must keep opening in its own window, so the shell can only be
    // triggered by a control that no existing project has (049 R3, R45).
    SideMenu,
    // Plugin-provided
    Custom {
        plugin_id: String,
        control_id: String,
    },
}

pub const BASE_MOUSE: &[&str] = &[
    "onClick",
    "onDblClick",
    "onDoubleClick",
    "onRightClick",
    "onMiddleClick",
    "onMouseDown",
    "onMouseUp",
    "onMouseMove",
    "onMouseEnter",
    "onMouseLeave",
    "onMouseWheel",
    "onContextMenu",
];
pub const BASE_FOCUS: &[&str] = &["onGotFocus", "onLostFocus"];
pub const BASE_KEYBOARD: &[&str] = &[
    "onKeyDown",
    "onKeyUp",
    "onKeyPress",
    "onEnterPressed",
    "onEscapePressed",
];
pub const BASE_HOVER: &[&str] = &["onHoverEnter", "onHoverLeave"];
pub const BASE_GEOMETRY: &[&str] = &[
    "onResize",
    "onResized",
    "onMove",
    "onMoved",
    "onVisibleChanged",
    "onEnabledChanged",
];
pub const BASE_LIFECYCLE: &[&str] = &["onLoad"];

impl ControlType {
    pub fn chart_binding_kind(&self) -> Option<BindingChartKind> {
        match self {
            ControlType::BarChart => Some(BindingChartKind::Bar),
            ControlType::LineChart => Some(BindingChartKind::Line),
            ControlType::PieChart => Some(BindingChartKind::Pie),
            ControlType::AreaChart => Some(BindingChartKind::Area),
            ControlType::ScatterChart => Some(BindingChartKind::Scatter),
            ControlType::DonutChart => Some(BindingChartKind::Donut),
            _ => None,
        }
    }

    pub fn approved_binding_target_kind(&self) -> Option<ApprovedBindingTargetKind> {
        match self {
            ControlType::DataGrid => Some(ApprovedBindingTargetKind::DataGrid),
            ControlType::ComboBox => Some(ApprovedBindingTargetKind::ComboBox),
            ControlType::ListBox => Some(ApprovedBindingTargetKind::ListBox),
            ControlType::Knob | ControlType::Gauge | ControlType::Switch => {
                Some(ApprovedBindingTargetKind::ScalarControl)
            }
            ControlType::Maps => Some(ApprovedBindingTargetKind::MarkerCollection),
            _ => self
                .chart_binding_kind()
                .map(ApprovedBindingTargetKind::Chart),
        }
    }

    /// Every built-in control type, once, in toolbox order.
    ///
    /// Anything that must cover "all controls" — above all the knowledge base
    /// the IDE assistant reads — iterates THIS list instead of keeping its own.
    /// Hand-kept copies are how the SideMenu came to be described in detail and
    /// published nowhere: the description existed, the type was simply absent
    /// from the array the generator walked. `Custom` is excluded: plugin
    /// controls are discovered at run time, not enumerable here.
    pub const ALL: &'static [ControlType] = &[
        ControlType::Button,
        ControlType::TextBox,
        ControlType::Label,
        ControlType::CheckBox,
        ControlType::RadioButton,
        ControlType::ListBox,
        ControlType::ComboBox,
        ControlType::GroupBox,
        ControlType::Panel,
        ControlType::TabControl,
        ControlType::DataGrid,
        ControlType::PictureBox,
        ControlType::Animator,
        ControlType::ProgressBar,
        ControlType::MenuBar,
        ControlType::SideMenu,
        ControlType::ToolBar,
        ControlType::StatusBar,
        ControlType::Line,
        ControlType::DateTimePicker,
        ControlType::NumericUpDown,
        ControlType::TreeView,
        ControlType::Splitter,
        ControlType::Timer,
        ControlType::Shape,
        ControlType::AgentObject,
        ControlType::RestClient,
        ControlType::SqlDatabase,
        ControlType::IndexedFile,
        ControlType::Slider,
        ControlType::BarChart,
        ControlType::LineChart,
        ControlType::PieChart,
        ControlType::AreaChart,
        ControlType::ScatterChart,
        ControlType::DonutChart,
        ControlType::Knob,
        ControlType::Gauge,
        ControlType::Switch,
        ControlType::FileDropZone,
        ControlType::Maps,
        ControlType::WebSearch,
    ];

    pub fn as_str(&self) -> &str {
        match self {
            ControlType::Button => "Button",
            ControlType::TextBox => "TextBox",
            ControlType::Label => "Label",
            ControlType::CheckBox => "CheckBox",
            ControlType::RadioButton => "RadioButton",
            ControlType::ListBox => "ListBox",
            ControlType::ComboBox => "ComboBox",
            ControlType::GroupBox => "GroupBox",
            ControlType::Panel => "Panel",
            ControlType::TabControl => "TabControl",
            ControlType::DataGrid => "DataGrid",
            ControlType::PictureBox => "PictureBox",
            ControlType::Animator => "Animator",
            ControlType::ProgressBar => "ProgressBar",
            ControlType::MenuBar => "MenuBar",
            ControlType::SideMenu => "SideMenu",
            ControlType::ToolBar => "ToolBar",
            ControlType::StatusBar => "StatusBar",
            ControlType::Line => "Line",
            ControlType::DateTimePicker => "DateTimePicker",
            ControlType::NumericUpDown => "NumericUpDown",
            ControlType::TreeView => "TreeView",
            ControlType::Splitter => "Splitter",
            ControlType::Timer => "Timer",
            ControlType::Shape => "Shape",
            ControlType::AgentObject => "AgentObject",
            ControlType::RestClient => "RestClient",
            ControlType::SqlDatabase => "SqlDatabase",
            ControlType::IndexedFile => "IndexedFile",
            ControlType::Slider => "Slider",
            ControlType::BarChart => "BarChart",
            ControlType::LineChart => "LineChart",
            ControlType::PieChart => "PieChart",
            ControlType::AreaChart => "AreaChart",
            ControlType::ScatterChart => "ScatterChart",
            ControlType::DonutChart => "DonutChart",
            ControlType::Knob => "Knob",
            ControlType::Gauge => "Gauge",
            ControlType::Switch => "Switch",
            ControlType::FileDropZone => "FileDropZone",
            ControlType::Maps => "Maps",
            ControlType::WebSearch => "WebSearch",
            ControlType::Custom {
                plugin_id,
                control_id,
            } => Box::leak(format!("{plugin_id}:{control_id}").into_boxed_str()),
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Button" => ControlType::Button,
            "TextBox" => ControlType::TextBox,
            "Label" => ControlType::Label,
            "CheckBox" => ControlType::CheckBox,
            "RadioButton" => ControlType::RadioButton,
            "ListBox" => ControlType::ListBox,
            "ComboBox" => ControlType::ComboBox,
            "GroupBox" => ControlType::GroupBox,
            "Panel" => ControlType::Panel,
            "TabControl" => ControlType::TabControl,
            "DataGrid" => ControlType::DataGrid,
            "PictureBox" => ControlType::PictureBox,
            "Animator" => ControlType::Animator,
            "ProgressBar" => ControlType::ProgressBar,
            "MenuBar" => ControlType::MenuBar,
            "SideMenu" => ControlType::SideMenu,
            "ToolBar" => ControlType::ToolBar,
            "StatusBar" => ControlType::StatusBar,
            "Line" => ControlType::Line,
            "DateTimePicker" => ControlType::DateTimePicker,
            "NumericUpDown" => ControlType::NumericUpDown,
            "TreeView" => ControlType::TreeView,
            "Splitter" => ControlType::Splitter,
            "Timer" => ControlType::Timer,
            "Shape" => ControlType::Shape,
            "AgentObject" => ControlType::AgentObject,
            "RestClient" => ControlType::RestClient,
            "SqlDatabase" => ControlType::SqlDatabase,
            "IndexedFile" => ControlType::IndexedFile,
            "Slider" => ControlType::Slider,
            "BarChart" => ControlType::BarChart,
            "LineChart" => ControlType::LineChart,
            "PieChart" => ControlType::PieChart,
            "AreaChart" => ControlType::AreaChart,
            "ScatterChart" => ControlType::ScatterChart,
            "DonutChart" => ControlType::DonutChart,
            "Knob" => ControlType::Knob,
            "Gauge" => ControlType::Gauge,
            "Switch" => ControlType::Switch,
            "FileDropZone" => ControlType::FileDropZone,
            "Maps" => ControlType::Maps,
            "WebSearch" => ControlType::WebSearch,
            other => {
                if let Some((p, c)) = other.split_once(':') {
                    ControlType::Custom {
                        plugin_id: p.to_owned(),
                        control_id: c.to_owned(),
                    }
                } else {
                    ControlType::Custom {
                        plugin_id: "unknown".to_owned(),
                        control_id: other.to_owned(),
                    }
                }
            }
        }
    }

    pub fn default_size(&self) -> (i32, i32) {
        match self {
            ControlType::Button => (80, 28),
            ControlType::TextBox => (160, 24),
            ControlType::Label => (120, 20),
            ControlType::CheckBox => (120, 22),
            ControlType::RadioButton => (120, 22),
            ControlType::ListBox => (160, 100),
            ControlType::ComboBox => (160, 24),
            ControlType::GroupBox => (200, 120),
            ControlType::Panel => (200, 150),
            ControlType::TabControl => (300, 200),
            ControlType::DataGrid => (300, 200),
            ControlType::PictureBox => (120, 120),
            ControlType::Animator => (160, 120),
            ControlType::ProgressBar => (200, 22),
            ControlType::MenuBar => (400, 24),
            // A sidebar rail: tall and narrow, the mirror of MenuBar's strip.
            ControlType::SideMenu => (200, 400),
            ControlType::ToolBar => (400, 32),
            ControlType::StatusBar => (400, 22),
            ControlType::Line => (200, 4),
            ControlType::DateTimePicker => (200, 24),
            ControlType::NumericUpDown => (120, 24),
            ControlType::TreeView => (200, 200),
            ControlType::Splitter => (200, 8),
            ControlType::Timer => (48, 48),
            ControlType::Shape => (120, 80),
            ControlType::AgentObject => (56, 56),
            ControlType::RestClient => (56, 56),
            ControlType::SqlDatabase => (64, 64),
            ControlType::IndexedFile => (64, 64),
            ControlType::Slider => (200, 36),
            ControlType::BarChart => (320, 220),
            ControlType::LineChart => (320, 220),
            ControlType::PieChart => (240, 240),
            ControlType::AreaChart => (320, 220),
            ControlType::ScatterChart => (320, 220),
            ControlType::DonutChart => (240, 240),
            ControlType::Knob => (80, 96),
            ControlType::Gauge => (140, 90),
            ControlType::Switch => (52, 28),
            ControlType::FileDropZone => (220, 100),
            ControlType::Maps => (320, 240),
            ControlType::WebSearch => (56, 56),
            ControlType::Custom { .. } => (100, 30),
        }
    }

    pub fn primary_event(&self) -> &str {
        match self {
            ControlType::Button => "onClick",
            ControlType::TextBox => "onChange",
            ControlType::CheckBox => "onClick",
            ControlType::RadioButton => "onClick",
            ControlType::ListBox => "onClick",
            ControlType::ComboBox => "onChange",
            ControlType::DateTimePicker => "onChange",
            ControlType::NumericUpDown => "onChange",
            ControlType::TreeView => "onNodeClick",
            ControlType::Timer => "onTick",
            ControlType::AgentObject => "onResponse",
            ControlType::RestClient => "onResponseReceived",
            ControlType::SqlDatabase => "onQueryComplete",
            ControlType::IndexedFile => "onComplete",
            ControlType::Slider => "onChange",
            ControlType::BarChart
            | ControlType::LineChart
            | ControlType::PieChart
            | ControlType::AreaChart
            | ControlType::ScatterChart
            | ControlType::DonutChart => "onDataChanged",
            ControlType::Knob => "onChange",
            ControlType::Switch => "onClick",
            ControlType::FileDropZone => "onFilesDropped",
            ControlType::Maps => "onMapClick",
            ControlType::WebSearch => "onResultsReceived",
            // Gauge is read-only (no interactive primary event, R10); the
            // catch-all below applies but is functionally inert since
            // Gauge's supported_events() never lists onClick.
            _ => "onClick",
        }
    }

    /// The events that must fire when `property` CHANGES, whoever changed it.
    ///
    /// A form's events divide in two (operator, 2026-08-17):
    ///
    /// * **Observer** events report that something *is now different* —
    ///   `onChange`, `onValueChanged`, `onTextChanged`, `onVisibleChanged`. They
    ///   are about the value, not about who touched it, so a Timer handler doing
    ///   `MOVE 5 TO KNOB-1::Value` must fire them exactly as a drag does.
    /// * **Passive** events report a user's ACT — `onClick`, `onMouseDown`,
    ///   `onGotFocus`, `onDblClick`. There is no act to report when COBOL writes
    ///   a property, so they must NOT fire. This function never returns one.
    ///
    /// Only the observer half lives here. The interaction paths in
    /// `render::render_form` fire both halves for real input; this is what the
    /// running host consults when the INTERPRETER writes a property, which fired
    /// nothing at all before — the reported bug being a Timer raising a Knob's
    /// `Value` with the Knob's `onValueChanged` never running.
    ///
    /// The result is filtered against [`Self::supported_events`], so a control
    /// can never be handed an event it does not declare.
    pub fn observer_events_for(&self, property: &str) -> Vec<&'static str> {
        let p = property.trim();
        let eq = |name: &str| p.eq_ignore_ascii_case(name);

        // Shared by every control that has them: visibility and availability are
        // observable on anything.
        let mut names: Vec<&'static str> = if eq("Visible") {
            vec!["onVisibleChanged"]
        } else if eq("Enabled") {
            vec!["onEnabledChanged"]
        } else {
            match self {
                // Numeric value controls. `onChange` is the historical name and
                // `onValueChanged` the explicit one; both are declared, so both
                // fire — a form may have bound either.
                ControlType::Knob
                | ControlType::Slider
                | ControlType::NumericUpDown
                | ControlType::ProgressBar
                | ControlType::Gauge
                | ControlType::DateTimePicker => {
                    if eq("Value") {
                        vec!["onChange", "onValueChanged"]
                    } else {
                        vec![]
                    }
                }
                // Text.
                ControlType::TextBox => {
                    if eq("Text") {
                        vec!["onChange", "onTextChanged"]
                    } else {
                        vec![]
                    }
                }
                // Checked things.
                ControlType::CheckBox | ControlType::RadioButton | ControlType::Switch => {
                    if eq("Checked") {
                        vec!["onChange", "onCheckedChanged", "onValueChanged"]
                    } else {
                        vec![]
                    }
                }
                // Lists: the selection, and the list itself.
                ControlType::ComboBox | ControlType::ListBox => {
                    if eq("SelectedIndex") {
                        vec!["onChange", "onSelectedIndexChanged"]
                    } else if eq("Text") {
                        vec!["onChange", "onTextChanged"]
                    } else if eq("Items") {
                        vec!["onDataChanged"]
                    } else {
                        vec![]
                    }
                }
                // Data surfaces.
                ControlType::DataGrid => {
                    if eq("Rows") || eq("Columns") || eq("DataSource") {
                        vec!["onDataChanged"]
                    } else {
                        vec![]
                    }
                }
                ControlType::TreeView => {
                    if eq("Items") {
                        vec!["onDataChanged"]
                    } else {
                        vec![]
                    }
                }
                ControlType::BarChart
                | ControlType::LineChart
                | ControlType::PieChart
                | ControlType::AreaChart
                | ControlType::ScatterChart
                | ControlType::DonutChart => {
                    if eq("Data") || eq("Series") || eq("DataSource") {
                        vec!["onDataChanged"]
                    } else {
                        vec![]
                    }
                }
                // Captions and labels.
                ControlType::Label | ControlType::Button | ControlType::GroupBox => {
                    if eq("Caption") {
                        vec!["onTextChanged"]
                    } else {
                        vec![]
                    }
                }
                _ => vec![],
            }
        };

        // A control only ever hears an event it declares.
        let declared = self.supported_events();
        names.retain(|n| declared.iter().any(|d| d.eq_ignore_ascii_case(n)));
        names.dedup();
        names
    }

    pub fn supported_events(&self) -> &'static [&'static str] {
        match self {
            ControlType::Button => &[
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseWheel",
                "onContextMenu",
                "onGotFocus",
                "onLostFocus",
                "onKeyDown",
                "onKeyUp",
                "onKeyPress",
                "onEnterPressed",
                "onEscapePressed",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
            ],
            ControlType::TextBox => &[
                "onChange",
                "onTextChanged",
                "onKeyPress",
                "onKeyDown",
                "onKeyUp",
                "onEnterPressed",
                "onEscapePressed",
                "onGotFocus",
                "onLostFocus",
                "onEnter",
                "onLeave",
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
            ],
            ControlType::Label => &[
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
            ],
            // Gauge is a read-only display (R10) — the same baseline as
            // Label: no focus/keyboard, since it never accepts input.
            ControlType::Gauge => &[
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
            ],
            ControlType::FileDropZone => &[
                "onFilesDropped",
                // Fired when a drop carried files the zone would not take, so a
                // form can say WHY rather than appear to have ignored them.
                "onFilesRejected",
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseWheel",
                "onContextMenu",
                "onGotFocus",
                "onLostFocus",
                "onKeyDown",
                "onKeyUp",
                "onKeyPress",
                "onEnterPressed",
                "onEscapePressed",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
            ],
            ControlType::Maps => &[
                // The five data methods (Geocode/ReverseGeocode/Directions/
                // DistanceMatrix/PlacesSearch) are ALWAYS async — they return
                // immediately and deliver through the uniform async lifecycle
                // events (spec 032), exactly like RestClient. Without these
                // four listed here the Designer offered no way to bind them,
                // so `ResponseBody` was unreachable and every lookup had to be
                // polled from a Timer: the result was computed and then had
                // nowhere to go.
                "onComplete",
                "onError",
                "onTimeout",
                "onCancelled",
                "onMapClick",
                "onMarkerClick",
                "onBoundsChanged",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseWheel",
                "onContextMenu",
                "onGotFocus",
                "onLostFocus",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
            ],
            // Spec 039 T14: same uniform async lifecycle as RestClient
            // (onError/onTimeout double as the async error/timeout events),
            // plus the primary `onResultsReceived` — unlike RestClient's
            // `supported_events()`, which omits its own primary event, T14's
            // task text calls for the primary to appear here explicitly.
            ControlType::WebSearch => &[
                "onResultsReceived",
                "onError",
                "onTimeout",
                "onComplete",
                "onCancelled",
            ],
            ControlType::CheckBox | ControlType::RadioButton | ControlType::Switch => &[
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onGotFocus",
                "onLostFocus",
                "onKeyDown",
                "onKeyUp",
                "onKeyPress",
                "onEnterPressed",
                "onEscapePressed",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
                // The three ways a toggle's state is worth reacting to:
                // it went ON, it went OFF, or it moved at all. `onCheckedChanged`
                // was already the "moved at all" one, so the two directional
                // events join it rather than a third name for the same thing.
                "onCheck",
                "onUncheck",
                "onCheckedChanged",
                "onValueChanged",
            ],
            ControlType::ListBox => &[
                // Fired when a row's tick box is clicked, carrying the whole
                // checked set — the selection events below report the active
                // row, which a tick never moves.
                "onItemChecked",
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onGotFocus",
                "onLostFocus",
                "onKeyDown",
                "onKeyUp",
                "onKeyPress",
                "onEnterPressed",
                "onEscapePressed",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
                "onChange",
                "onSelectedIndexChanged",
                "onItemDoubleClick",
                "onSelectionChanged",
                "onScroll",
            ],
            ControlType::ComboBox => &[
                "onChange",
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onGotFocus",
                "onLostFocus",
                "onKeyDown",
                "onKeyUp",
                "onKeyPress",
                "onEnterPressed",
                "onEscapePressed",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
                "onSelectedIndexChanged",
                "onDropDown",
                "onDropDownClosed",
            ],
            ControlType::DateTimePicker
            | ControlType::NumericUpDown
            | ControlType::Slider
            | ControlType::Knob => &[
                "onChange",
                "onValueChanged",
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onGotFocus",
                "onLostFocus",
                "onKeyDown",
                "onKeyUp",
                "onKeyPress",
                "onEnterPressed",
                "onEscapePressed",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
            ],
            ControlType::TreeView => &[
                "onNodeClick",
                "onNodeDblClick",
                "onNodeDoubleClick",
                "onNodeSelect",
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onGotFocus",
                "onLostFocus",
                "onKeyDown",
                "onKeyUp",
                "onKeyPress",
                "onEnterPressed",
                "onEscapePressed",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
            ],
            ControlType::Timer => &["onTick"],
            ControlType::PictureBox => &[
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
                "onImageLoaded",
                "onImageError",
            ],
            ControlType::Animator => &[
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
                "onStarted",
                "onEnded",
                "onFrameChanged",
                "onLooped",
            ],
            ControlType::DataGrid => &[
                "onCellClick",
                "onCellDoubleClick",
                "onRowSelect",
                "onRowDoubleClick",
                "onColumnClick",
                "onSelectionChanged",
                "onScroll",
                "onExportCSV",
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onGotFocus",
                "onLostFocus",
                "onKeyDown",
                "onKeyUp",
                "onKeyPress",
                "onEnterPressed",
                "onEscapePressed",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
            ],
            ControlType::AgentObject => &["onResponse", "onError"],
            ControlType::RestClient => {
                // The last two are the uniform async lifecycle events (spec 032);
                // onError/onTimeout already double as the async error/timeout events.
                &[
                    "onError",
                    "onTimeout",
                    "onComplete",
                    "onCancelled",
                ]
            }
            ControlType::SqlDatabase => &[
                "onQueryComplete",
                "onConnectOk",
                "onConnectError",
                "onQueryError",
                "onRowFetched",
                // Uniform async lifecycle events (spec 032).
                "onComplete",
                "onError",
                "onCancelled",
                "onTimeout",
            ],
            ControlType::IndexedFile => &[
                "onError",
                // Uniform async lifecycle events (spec 032).
                "onComplete",
                "onCancelled",
                "onTimeout",
            ],
            ControlType::GroupBox | ControlType::Panel => &[
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
                "onChildAdded",
                "onChildRemoved",
            ],
            ControlType::TabControl => &[
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onGotFocus",
                "onLostFocus",
                "onKeyDown",
                "onKeyUp",
                "onKeyPress",
                "onEnterPressed",
                "onEscapePressed",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
                "onTabChanged",
                "onTabClick",
            ],
            ControlType::ProgressBar => &[
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
                "onValueChanged",
                "onCompleted",
            ],
            ControlType::BarChart
            | ControlType::LineChart
            | ControlType::PieChart
            | ControlType::AreaChart
            | ControlType::ScatterChart
            | ControlType::DonutChart => &[
                "onDataChanged",
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
            ],
            ControlType::MenuBar | ControlType::SideMenu => &[
                "onMenuClick",
                "onMenuItemClick",
                "onMenuOpen",
                "onMenuClose",
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onGotFocus",
                "onLostFocus",
                "onKeyDown",
                "onKeyUp",
                "onKeyPress",
                "onEnterPressed",
                "onEscapePressed",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
            ],
            ControlType::ToolBar
            | ControlType::StatusBar
            | ControlType::Line
            | ControlType::Splitter
            | ControlType::Shape => &[
                "onClick",
                "onDblClick",
                "onDoubleClick",
                "onRightClick",
                "onMiddleClick",
                "onMouseDown",
                "onMouseUp",
                "onMouseMove",
                "onMouseEnter",
                "onMouseLeave",
                "onMouseWheel",
                "onContextMenu",
                "onHoverEnter",
                "onHoverLeave",
                "onResize",
                "onResized",
                "onMove",
                "onMoved",
                "onVisibleChanged",
                "onEnabledChanged",
                "onLoad",
            ],
            ControlType::Custom { .. } => &["onClick"],
        }
    }

    /// Returns true for controls that are invisible at runtime (shown as icon boxes in designer).
    pub fn is_non_visual(&self) -> bool {
        matches!(
            self,
            ControlType::Timer
                | ControlType::AgentObject
                | ControlType::RestClient
                | ControlType::SqlDatabase
                | ControlType::IndexedFile
                | ControlType::WebSearch
        )
    }
}

// ── Form events ─────────────────────────────────────────────────────────────────

/// The events the **form** itself supports, grouped by category (display order).
/// A handler binding is created lazily when the user first attaches code to one;
/// `onLoad` / `onClose` are pre-stubbed by `Form::new`. (Not all are wired into
/// the runtime/codegen yet — they are designable now, fired as support lands.)
pub const FORM_EVENT_GROUPS: &[(&str, &[&str])] = &[
    (
        "Lifecycle",
        &[
            "onCreate",
            "onInitialize",
            "onLoad",
            "onOpened",
            "onShow",
            "onHide",
            "onClose",
            "onClosing",
            // 037 R17 — a close attempt was refused because FormState is
            // Waiting (or a Sync child of this form is Waiting).
            "onCloseRejected",
            "onClosed",
            // 049 R26 — fired immediately before the form's storage is
            // released (a navigation-chain pop, a non-preserved sibling
            // replacement, a root-slot unwind). The teardown point: close
            // files, COMMIT, free resources. NEVER fired on a mere
            // swap-out — that is onDeactivate.
            "onDestroy",
        ],
    ),
    (
        "Activation & Focus",
        &[
            "onActivate",
            "onActivated",
            // 049 R26/R27 — in the shell, fired when the form's body leaves
            // the ContentPane while the form STAYS RESIDENT (it became an
            // ancestor, or was parked by PreservePreviousForm). Not a
            // teardown point: storage and menu handlers stay live.
            "onDeactivate",
            "onDeactivated",
            "onGotFocus",
            "onLostFocus",
        ],
    ),
    (
        "Window State",
        &[
            "onResize",
            "onResizing",
            "onMove",
            "onMoving",
            "onMinimize",
            "onMaximize",
            "onRestore",
            "onFullscreen",
            "onExitFullscreen",
            // 037 R14 — the ACTUAL fullscreen state changed (either
            // direction); read `me`'s FullScreen for the new value.
            "onFullScreenChanged",
        ],
    ),
    (
        "Layout & Painting",
        &[
            "onLayout",
            "onPaint",
            "onRepaint",
            "onThemeChanged",
            "onDpiChanged",
            "onFontChanged",
        ],
    ),
    (
        "Mouse",
        &[
            "onClick",
            "onDoubleClick",
            "onMouseDown",
            "onMouseUp",
            "onMouseMove",
            "onMouseEnter",
            "onMouseLeave",
            "onMouseWheel",
            "onContextMenu",
        ],
    ),
    (
        "Touch & Pointer",
        &[
            "onPointerDown",
            "onPointerUp",
            "onPointerMove",
            "onPointerEnter",
            "onPointerLeave",
            "onPointerCancel",
            "onGesture",
        ],
    ),
    (
        "Scrolling",
        &[
            "onScroll",
            "onScrollStart",
            "onScrollEnd",
            "onHorizontalScroll",
            "onVerticalScroll",
        ],
    ),
    (
        "Drag & Drop",
        &["onDragEnter", "onDragLeave", "onDragOver", "onDrop"],
    ),
    ("Clipboard", &["onCut", "onCopy", "onPaste"]),
    (
        "System / OS",
        &[
            "onSystemColorChanged",
            "onDisplayChanged",
            "onPowerSuspend",
            "onPowerResume",
            "onSessionLock",
            "onSessionUnlock",
        ],
    ),
    ("Error Handling", &["onUnhandledException"]),
];

/// Flat iterator over every supported form event name (across all groups).
pub fn form_supported_events() -> impl Iterator<Item = &'static str> {
    FORM_EVENT_GROUPS
        .iter()
        .flat_map(|(_, evs)| evs.iter().copied())
}

// ── Control ───────────────────────────────────────────────────────────────────

/// A single visual (or non-visual) control on a form.
#[derive(Debug, Clone)]
pub struct Control {
    pub id: String,
    pub control_type: ControlType,
    pub rect: Rect,
    pub tab_order: u32,
    /// Z-order: higher = drawn on top. 0 = bottommost. Negative values allowed.
    pub z_order: i32,
    pub visible: bool,
    pub enabled: bool,
    pub properties: IndexMap<String, PropValue>,
    pub events: Vec<EventBinding>,
    pub children: Vec<Control>,
    /// Animation definitions for this control.
    pub animations: Vec<AnimationDef>,
    /// Id of the enclosing container control (`None` = a direct child of the
    /// form). The editor keeps controls in one flat list and derives nesting from
    /// this link; the `.cfrm` `<Children>` tree is (re)built from it at save
    /// (spec 012).
    pub parent: Option<String>,
    /// For a control whose `parent` is a `TabControl`: which tab page (0-based) it
    /// belongs to. `None` otherwise.
    pub tab: Option<u32>,
}

/// Default `BackgroundColor` assigned to every new control. The glass renderer
/// treats a control still on this value as having *no* explicit background. For
/// the DataGrid (the one control with a solid grid background) this means the
/// grid stays translucent Liquid Glass until the user picks a colour, which then
/// paints solid beneath the frost. Kept as a named constant so `Control::new`
/// and the renderer's gate can never drift apart.
pub const DEFAULT_BACKGROUND_COLOR: &str = "#F0F0F0";

/// Default `ForegroundColor` assigned to every new control. A DataGrid still on
/// this value uses the subtle built-in grid-line colour; any other colour set in
/// the Appearance section becomes the grid-line colour. Named so `Control::new`
/// and the DataGrid renderer's gate can never drift apart.
pub const DEFAULT_FOREGROUND_COLOR: &str = "#FFFFFF";

/// A ProgressBar's seeded `BarColor`. Like [`DEFAULT_FOREGROUND_COLOR`], this
/// value means "the developer has not chosen": a bar still carrying it takes
/// the active theme's green, and only a different value is a real choice.
pub const DEFAULT_BAR_COLOR: &str = "#00AA00";

/// Default background of a NEW form — opaque graphite, the surface
/// [`DEFAULT_FOREGROUND_COLOR`] (white) was chosen to read against.
///
/// It used to be `00000000`: fully transparent, so the desktop showed through
/// and every control's legibility depended on whatever window or wallpaper was
/// behind the form. White default text was readable when the form opened over
/// a dark desktop and invisible when it opened over the IDE — the same build
/// appearing to work and then not, run to run, with nothing changed. A form's
/// appearance must be a property of the form. A developer who wants the glass
/// look lowers the alpha deliberately.
///
/// Only new forms are affected: the value is written into every `.cfrm` at
/// creation, so existing designs keep exactly what they have.
pub const DEFAULT_FORM_BACKGROUND_COLOR: &str = "#2E3138FF";

/// 049 — the default height of a SideMenu's footer pane, in points. Mirrors
/// the renderer's `sidebar::FOOTER_H`; both must agree or the Panel would be
/// pinned somewhere the rail does not paint.
pub const DEFAULT_SIDE_MENU_FOOTER_H: i32 = 72;

/// The breadcrumb frame's default height, in points — what a rail with no
/// `BreadcrumbHeight` is drawn at. Re-exported as `breadcrumb::HEIGHT`, which
/// is the name the renderer and its callers use; it lives here because the
/// model is not behind the `render` feature and the property default is.
pub const DEFAULT_BREADCRUMB_HEIGHT: f32 = 28.0;

/// The shortest a breadcrumb frame may be drawn, in points. Below this the
/// strip has no room for its own text and the sidebar's Open/Collapsed control
/// stops being a target anyone can hit; a height under it is read as "not set".
pub const MIN_BREADCRUMB_HEIGHT: f32 = 16.0;

/// Marks the Panel a SideMenu owns in its footer pane. A property rather than
/// an id convention, so the Panel survives being renamed.
pub const SIDE_MENU_FOOTER_PROP: &str = "IsSideMenuFooter";

/// The id of the Panel a SideMenu owns in its footer pane.
pub fn side_menu_footer_id(side_id: &str) -> String {
    format!("{side_id}-Footer")
}

/// Default `FillColor` assigned to every new Shape — the legacy silver face. A
/// Shape still on this value takes its face from the Appearance **Back colour**
/// instead, so that section is not dead for Shapes; any other `FillColor` is an
/// explicit type-specific choice and wins. Named so `Control::new` and the
/// renderer's gate can never drift apart.
pub const DEFAULT_SHAPE_FILL_COLOR: &str = "#C0C0C0";

pub const NEUMORPHIC_SURFACE_COLOR: &str = "#E1E6F8FF";
/// Form-level colours are stored **without** the leading `#`; control-level
/// ones keep it. Both spellings below are deliberate.
pub const NEUMORPHIC_FORM_BACKGROUND: &str = "EAEBEFFF";
/// Neumorphic Light seeds its controls with a South gradient, mirroring what
/// Neumorphic Dark already does — the flat `NEUMORPHIC_SURFACE_COLOR` stays as
/// the surface underneath, visible if the developer turns the gradient off.
pub const NEUMORPHIC_GRADIENT_START: &str = "#F8F8F8FF";
pub const NEUMORPHIC_GRADIENT_END: &str = "#DFE0E1FF";
pub const NEUMORPHIC_DARK_SURFACE_COLOR: &str = "#36383EFF";
pub const NEUMORPHIC_DARK_FORM_BACKGROUND: &str = "36383EFF";
pub const NEUMORPHIC_DARK_LIGHT_SHADOW: &str = "#4E4E4EFF";
pub const NEUMORPHIC_DARK_GRADIENT_START: &str = "#4E4E4EFF";
pub const NEUMORPHIC_DARK_GRADIENT_END: &str = "#000000FF";

const TAB_CONTROL_MCP_TOOL: &str = r#"{"name":"manage_tab_control_tabs","description":"Creates, updates, reorders, selects, or removes tabs that belong to a TabControl in the form designer. A tab is a child container owned by exactly one TabControl and represents one selectable page within that control. Tabs must not be created as independent top-level form controls. Controls placed on a tab belong to that tab page and are visible only when the tab is active, unless the designer is explicitly displaying inactive pages for editing.","inputSchema":{"type":"object","required":["operation","tab_control_id"],"properties":{"operation":{"type":"string","enum":["create","update","remove","reorder","select"],"description":"The operation to perform on a tab belonging to the specified TabControl."},"tab_control_id":{"type":"string","description":"The unique identifier of the parent TabControl that owns the tab. The referenced control must exist and must be a TabControl."},"tab_id":{"type":"string","description":"The stable unique identifier of the tab page. Required for update, remove, reorder, and select operations. The tab must belong to the specified TabControl."},"caption":{"type":"string","description":"The text displayed in the tab header. Changing the caption does not change the tab identifier or the ownership of controls placed inside the tab."},"index":{"type":"integer","minimum":0,"description":"The zero-based position of the tab within the parent TabControl. Tabs are displayed according to this order. Reordering a tab must preserve its identifier and child controls."},"selected":{"type":"boolean","description":"Determines whether this tab becomes the active page of the TabControl. Only one tab within the same TabControl may be selected at a time."},"enabled":{"type":"boolean","description":"Determines whether the user can activate the tab at runtime. A disabled tab remains part of the TabControl and retains its child controls."},"visible":{"type":"boolean","description":"Determines whether the tab header and its page are available at runtime. Hiding a tab must not delete the tab or its child controls."},"tooltip":{"type":"string","description":"Optional explanatory text displayed when the user points to the tab header."},"icon":{"type":["string","null"],"description":"Optional icon resource associated with the tab header. The value must reference a valid project resource or be null to remove the icon."},"confirm_remove_with_children":{"type":"boolean","default":false,"description":"Confirms removal of a tab that contains child controls. Removing a tab may also remove or orphan its contained controls, depending on the designer policy. The tool must reject destructive removal unless this value is true."}},"allOf":[{"if":{"properties":{"operation":{"const":"create"}}},"then":{"required":["caption"]}},{"if":{"properties":{"operation":{"enum":["update","remove","reorder","select"]}}},"then":{"required":["tab_id"]}},{"if":{"properties":{"operation":{"const":"reorder"}}},"then":{"required":["index"]}}]}}"#;

/// The `Transparency` a freshly dropped control starts with, 0–100.
///
/// Almost everything starts opaque. A **CheckBox** starts fully transparent:
/// it is a tick and a caption, not a card, and a painted face behind it only
/// fights whatever it was dropped onto — a GroupBox, a Panel, the form itself.
/// Air above and below a list row's text.
pub const LIST_ROW_PAD: f32 = 2.0;
/// The list's own inner margin, between its border and its rows.
pub const LIST_FRAME_PAD: f32 = 3.0;

/// One line of a control's own text, at its own font size.
pub fn text_line_height(ctrl: &Control) -> f32 {
    let fs = ctrl
        .get_prop("FontSize")
        .map(|v| v.as_i64())
        .unwrap_or(14)
        .clamp(4, 200) as f32;
    fs * 1.35
}

/// The smallest a control may be dragged to.
///
/// 8×8 for most things — but a ListBox that cannot show one line of its own
/// text is not a list, it is a sliver. The floor follows the control's own
/// `FontSize`, so raising the font raises the floor with it.
pub fn min_control_size(ctrl: &Control) -> (i32, i32) {
    match ctrl.control_type {
        ControlType::ListBox => {
            let h = text_line_height(ctrl) + LIST_ROW_PAD * 2.0 + LIST_FRAME_PAD * 2.0;
            (24, h.ceil() as i32)
        }
        _ => (8, 8),
    }
}

pub fn default_transparency(control_type: &ControlType) -> i64 {
    match control_type {
        ControlType::CheckBox => 100,
        _ => 0,
    }
}

/// A control's `Transparency` (0 = opaque … 100 = invisible).
///
/// Falls back to the legacy `Opacity` (which ran the other way) so a form saved
/// before the rename still reads correctly even if it was never migrated.
pub fn transparency_of(ctrl: &Control) -> i64 {
    if let Some(v) = ctrl.get_prop("Transparency") {
        return v.as_i64().clamp(0, 100);
    }
    if let Some(v) = ctrl.get_prop("Opacity") {
        return (100 - v.as_i64().clamp(0, 100)).clamp(0, 100);
    }
    default_transparency(&ctrl.control_type)
}

/// A control's transparency as the alpha multiplier the painters want:
/// `1.0` fully opaque, `0.0` invisible.
pub fn alpha_multiplier(ctrl: &Control) -> f32 {
    1.0 - (transparency_of(ctrl) as f32 / 100.0)
}

/// Rewrite a control loaded from a pre-rename form: `Opacity` becomes its
/// complement in `Transparency`, and the old key is dropped.
///
/// Only fires when the file actually carried `Opacity` and no `Transparency`,
/// so a form saved by a newer build is left exactly as it is. Without this a
/// control saved at `Opacity = 40` would silently come back fully opaque, since
/// the seeded `Transparency` default would answer first.
pub fn migrate_legacy_opacity(ctrl: &mut Control) {
    let Some(old) = ctrl.get_prop("Opacity").map(|v| v.as_i64()) else {
        return;
    };
    if ctrl.get_prop("Transparency").is_some() {
        // Both present: the file is newer than the rename and simply still
        // carries the old key. Transparency wins; drop the stale one.
        ctrl.properties.shift_remove("Opacity");
        return;
    }
    ctrl.set_prop(
        "Transparency",
        PropValue::Int((100 - old.clamp(0, 100)).clamp(0, 100)),
    );
    ctrl.properties.shift_remove("Opacity");
}

impl Control {
    pub fn new(id: impl Into<String>, control_type: ControlType, x: i32, y: i32) -> Self {
        let (w, h) = control_type.default_size();
        let mut props = IndexMap::new();

        let id_str = id.into();
        // Controls whose control intrinsically shows a text label.
        let has_caption = matches!(
            control_type,
            ControlType::Label
                | ControlType::Button
                | ControlType::CheckBox
                | ControlType::RadioButton
        );
        if has_caption {
            props.insert("Caption".to_owned(), PropValue::from(id_str.clone()));
        }

        // ── Universal appearance props ─────────────────────────────────────────
        props.insert(
            "BackgroundColor".into(),
            PropValue::String(DEFAULT_BACKGROUND_COLOR.into()),
        );
        props.insert("BackgroundGradientEnabled".into(), PropValue::Bool(false));
        props.insert(
            "BackgroundGradientStartColor".into(),
            PropValue::String(DEFAULT_BACKGROUND_COLOR.into()),
        );
        props.insert(
            "BackgroundGradientEndColor".into(),
            PropValue::String("#C8D0DC".into()),
        );
        props.insert(
            "BackgroundGradientDirection".into(),
            PropValue::String("South".into()),
        );
        props.insert(
            "ForegroundColor".into(),
            PropValue::String(DEFAULT_FOREGROUND_COLOR.into()),
        );
        props.insert("FontName".into(), PropValue::String("Arial".into()));
        props.insert("FontSize".into(), PropValue::Int(14));
        props.insert("Bold".into(), PropValue::Bool(false));
        props.insert("Italic".into(), PropValue::Bool(false));
        props.insert("Underline".into(), PropValue::Bool(false));
        props.insert("Strikethrough".into(), PropValue::Bool(false));

        // ── Layout & behaviour ────────────────────────────────────────────────
        props.insert("Tooltip".into(), PropValue::String("".into()));
        props.insert("Cursor".into(), PropValue::String("Default".into()));
        // How long the pointer must rest on the control before onHoverEnter
        // fires (was a hardcoded 200 ms).
        props.insert("HoverDelayMs".into(), PropValue::Int(200));
        // Anchor is a boolean lock: when true, the control's X/Y can't be changed
        // by dragging it with the mouse on the canvas (keyboard/property-pane entry
        // still works). See `Control::is_anchored`.
        props.insert("Anchor".into(), PropValue::Bool(false));
        props.insert("Padding".into(), PropValue::Int(0));
        // How much of what is BEHIND the control shows through, 0–100:
        // 0 = opaque, 100 = the control's own face is not painted at all and
        // the form (or whatever control sits under it) shows in full. This
        // replaced `Opacity`, which ran the other way round and read as a
        // double negative every time transparency was what you actually wanted.
        // Forms saved with `Opacity` are migrated on load — see
        // [`migrate_legacy_opacity`].
        props.insert(
            "Transparency".into(),
            PropValue::Int(default_transparency(&control_type)),
        );

        // ── Drop shadow ───────────────────────────────────────────────────────
        props.insert("ShadowEnabled".into(), PropValue::Bool(false));
        props.insert("ShadowOpacity".into(), PropValue::Int(6)); // 0-100 %
        props.insert("ShadowColor".into(), PropValue::String("#000000".into()));
        props.insert(
            "ShadowLightColor".into(),
            PropValue::String("#FFFFFFFF".into()),
        );
        props.insert(
            "ShadowDirection".into(),
            PropValue::String("SouthEast".into()),
        ); // N/NE/E/SE/S/SW/W/NW
        props.insert("ShadowDistance".into(), PropValue::Int(7)); // px
        props.insert("ShadowBlur".into(), PropValue::Bool(true)); // enable soft-blur falloff
        props.insert("ShadowBlurStrength".into(), PropValue::Int(8)); // 0-20, blur radius in layers

        // ── Identification ────────────────────────────────────────────────────
        props.insert("ZOrder".into(), PropValue::Int(0));

        // ── Data binding (all controls) ────────────────────────────────────────
        props.insert("DataItem".into(), PropValue::String("".into()));
        props.insert("DataFormat".into(), PropValue::String("".into()));

        // ── Type-specific props ────────────────────────────────────────────────
        match &control_type {
            ControlType::TextBox => {
                props.insert("Text".into(), PropValue::String("".into()));
                props.insert("HintText".into(), PropValue::String("".into()));
                props.insert("TextAlignment".into(), PropValue::String("Left".into()));
                props.insert(
                    "VerticalAlignment".into(),
                    PropValue::String("Middle".into()),
                );
                props.insert("InnerPadding".into(), PropValue::Int(3));
                props.insert("MaximumLength".into(), PropValue::Int(0));
                props.insert("Multiline".into(), PropValue::Bool(false));
                props.insert("PasswordCharacter".into(), PropValue::String("".into()));
                props.insert("ReadOnly".into(), PropValue::Bool(false));
                props.insert("ScrollBars".into(), PropValue::String("None".into()));
                props.insert("WordWrap".into(), PropValue::Bool(true));
                props.insert("BorderStyle".into(), PropValue::String("Fixed3D".into()));
                props.insert("BorderColor".into(), PropValue::String("#AAAAAA".into()));
            }
            ControlType::Label => {
                props.insert("TextAlignment".into(), PropValue::String("Left".into()));
                props.insert(
                    "VerticalAlignment".into(),
                    PropValue::String("Middle".into()),
                );
                props.insert("WordWrap".into(), PropValue::Bool(false));
                props.insert("AutoSize".into(), PropValue::Bool(false));
                props.insert("BorderStyle".into(), PropValue::String("None".into()));
            }
            ControlType::CheckBox | ControlType::RadioButton => {
                props.insert("Checked".into(), PropValue::Bool(false));
                props.insert("GroupName".into(), PropValue::String("".into()));
                props.insert("CheckAlignment".into(), PropValue::String("Left".into()));
                props.insert("CheckColor".into(), PropValue::String("#0078D7".into()));
                // 0-100: how much of the check glyph's own box the checkmark
                // stroke fills.
                props.insert("CheckSize".into(), PropValue::Int(70));
                // The frame around the WHOLE control, not the check glyph — the
                // glyph is drawn by the CheckBox branch of `draw_control` and is
                // governed by CheckColor/CheckSize. `None` like Label, the
                // closest analogue: a checkbox is a glyph plus a caption, not a
                // surface. Absent these keys `draw_control` fell back to
                // "Single"/1px and boxed every checkbox with nothing able to
                // turn it off (operator, 2026-08-01).
                props.insert("BorderStyle".into(), PropValue::String("None".into()));
                props.insert("BorderColor".into(), PropValue::String("#8C8CA0".into()));
                props.insert("BorderWidth".into(), PropValue::Int(1));
            }
            ControlType::PictureBox => {
                props.insert("ImagePath".into(), PropValue::String("".into()));
                props.insert("SizeMode".into(), PropValue::String("Normal".into()));
                props.insert(
                    "ImageAlignment".into(),
                    PropValue::String("MiddleCenter".into()),
                );
                props.insert("BorderStyle".into(), PropValue::String("None".into()));
                props.insert("BorderColor".into(), PropValue::String("#888888".into()));
                // When false, the surrounding frame/background is not drawn — only
                // the image shows (transparent PNG areas reveal what's behind).
                props.insert("ShowFrame".into(), PropValue::Bool(true));
            }
            ControlType::Animator => {
                // Plays an animated image (GIF / WebP / APNG) or a still image.
                props.insert("Source".into(), PropValue::String("".into()));
                props.insert("AutoPlay".into(), PropValue::Bool(true));
                props.insert("Loop".into(), PropValue::Bool(true));
                props.insert("SizeMode".into(), PropValue::String("Fit".into()));
                props.insert(
                    "BackgroundColor".into(),
                    PropValue::String("#00000000".into()),
                );
                props.insert("BorderStyle".into(), PropValue::String("None".into()));
                props.insert("BorderColor".into(), PropValue::String("#888888".into()));
            }
            ControlType::ProgressBar => {
                props.insert("Minimum".into(), PropValue::Int(0));
                props.insert("Maximum".into(), PropValue::Int(100));
                props.insert("Value".into(), PropValue::Int(0));
                props.insert("BarColor".into(), PropValue::String(DEFAULT_BAR_COLOR.into()));
                props.insert("Orientation".into(), PropValue::String("Horizontal".into()));
                props.insert("Style".into(), PropValue::String("Continuous".into()));
                // How long one block is under `Style = Blocks`, along the axis
                // the bar travels. 0 = automatic: sized from the bar's own
                // thickness, so a tall bar gets long blocks and a thin one short.
                props.insert("BlockSize".into(), PropValue::Int(0));
                props.insert("ShowValue".into(), PropValue::Bool(false));
                // The frame was painted from constants no property could reach.
                // These three keep the bar looking exactly as it did while
                // putting it in the developer's hands, as on every other
                // bordered control.
                props.insert("BorderStyle".into(), PropValue::String("Single".into()));
                props.insert("BorderColor".into(), PropValue::String("#8C8CA0".into()));
                props.insert("BorderWidth".into(), PropValue::Int(1));
            }
            ControlType::ListBox => {
                props.insert("Items".into(), PropValue::String("".into()));
                props.insert("SelectedIndex".into(), PropValue::Int(-1));
                props.insert("MultiSelect".into(), PropValue::Bool(false));
                // The two selections a list carries: `Value`/`SelectedIndex` is
                // the ACTIVE row (the one the cursor is on, fully highlighted),
                // `SelectedItems` the set the user built with Ctrl/Cmd, drawn
                // in a dimmed version of the same highlight. `CheckedItems` is
                // separate again — the ticked set, in any order and with gaps.
                props.insert("SelectedItems".into(), PropValue::String("".into()));
                props.insert("ShowCheckBoxes".into(), PropValue::Bool(false));
                props.insert("CheckedItems".into(), PropValue::String("".into()));
                props.insert("Sorted".into(), PropValue::Bool(false));
                // The colours those two selections are drawn in. EMPTY means
                // "the developer has not chosen": the active row takes the
                // theme's own selection colour, and the rest of the set takes
                // that colour half lit — exactly what a list drew before these
                // properties existed, so an old form is unchanged.
                //
                // Empty rather than a seeded hex sentinel (the `BarColor` rule)
                // because a highlight IS a blue: any hex chosen as the sentinel
                // is one a developer might legitimately pick, and picking it
                // would silently mean "unset".
                props.insert("ActiveItemColor".into(), PropValue::String("".into()));
                props.insert("SelectedItemsColor".into(), PropValue::String("".into()));
                props.insert("BorderStyle".into(), PropValue::String("Single".into()));
                props.insert("BorderColor".into(), PropValue::String("#888888".into()));
            }
            ControlType::ComboBox => {
                props.insert("Items".into(), PropValue::String("".into()));
                props.insert("SelectedIndex".into(), PropValue::Int(-1));
                props.insert("Sorted".into(), PropValue::Bool(false));
                // The two highlights the OPEN dropdown draws, on the same
                // empty-means-unchosen rule as the ListBox's: the selected item
                // (`ActiveItemColor`, the same property name it carries there,
                // for the same thing) and the item the pointer is over.
                //
                // A ComboBox has no `SelectedItems` — one item is selected, or
                // none — so the list's second selection colour has no meaning
                // here and is not offered. Its second HIGHLIGHT is the hover,
                // which was equally hardcoded.
                //
                // Left empty each falls back to the constant the popup always
                // painted, not to the theme: unlike a list's, these two were
                // never theme-derived, and "unset" has to mean "what it drew
                // before".
                props.insert("ActiveItemColor".into(), PropValue::String("".into()));
                props.insert("HoverItemColor".into(), PropValue::String("".into()));
                props.insert("DropDownStyle".into(), PropValue::String("DropDown".into()));
                props.insert("DropDownHeight".into(), PropValue::Int(200));
                props.insert("Editable".into(), PropValue::Bool(true));
            }
            ControlType::Button => {
                props.insert("IsDefault".into(), PropValue::Bool(false));
                props.insert("BorderColor".into(), PropValue::String("#888888".into()));
                props.insert("BorderStyle".into(), PropValue::String("Single".into()));
                props.insert("BorderWidth".into(), PropValue::Int(1));
                props.insert("IconPath".into(), PropValue::String("".into()));
                props.insert("IconAlignment".into(), PropValue::String("Left".into()));
                props.insert("IconPadding".into(), PropValue::Int(10));
                props.insert("IconSize".into(), PropValue::String("32".into()));
                props.insert(
                    "TextAlignment".into(),
                    PropValue::String("MiddleCenter".into()),
                );
            }
            ControlType::Panel => {
                props.insert("BorderStyle".into(), PropValue::String("Single".into()));
                props.insert("BorderColor".into(), PropValue::String("#888888".into()));
                props.insert("BorderWidth".into(), PropValue::Int(1));
                // Container behaviour (spec 012): rounded corners + clip radius,
                // and optional auto-scroll of overflowing children.
                props.insert("HScroll".into(), PropValue::Bool(false));
                props.insert("VScroll".into(), PropValue::Bool(false));
                // Panel shares the same visual model as GroupBox (minus caption).
                props.insert("HideBackground".into(), PropValue::Bool(false));
                props.insert("mcp_tool".into(), PropValue::String("".into()));
            }
            ControlType::GroupBox => {
                props.insert("Caption".into(), PropValue::String(String::new()));
                props.insert("BorderStyle".into(), PropValue::String("Single".into()));
                props.insert("BorderColor".into(), PropValue::String("#888888".into()));
                props.insert("BorderWidth".into(), PropValue::Int(1));
                // Container behaviour (spec 012).
                props.insert("HScroll".into(), PropValue::Bool(false));
                props.insert("VScroll".into(), PropValue::Bool(false));
                props.insert("mcp_tool".into(), PropValue::String("".into()));
                // Project-scoped composite-control marker (spec 020). Empty for
                // normal GroupBoxes; deployed User Controls store the definition
                // name here.
                props.insert("UserControl".into(), PropValue::String("".into()));

                // ── Visual appearance (spec 015, Phase 1) ──────────────────────
                // Hide the caption text but stay a container; hide the box
                // fill/border while keeping children visible (cf. chart
                // HideBackground); optional directional background gradient.
                props.insert("HideCaption".into(), PropValue::Bool(false));
                props.insert("CaptionEnabled".into(), PropValue::Bool(true));
                props.insert("HideBackground".into(), PropValue::Bool(false));
                // ── Repeating group / array template (spec 015, Phase 2) ───────
                // Inert until IsRepeatingGroup is turned on (existing forms stay
                // unchanged). ArrayName empty ⇒ use the control id.
                props.insert("IsRepeatingGroup".into(), PropValue::Bool(false));
                props.insert("ArrayName".into(), PropValue::String("".into()));
                props.insert("ItemCount".into(), PropValue::Int(0));
                props.insert("DataSource".into(), PropValue::String("".into()));
                // Vertical | Horizontal | Grid
                props.insert(
                    "LayoutDirection".into(),
                    PropValue::String("Vertical".into()),
                );
                props.insert("ItemSpacing".into(), PropValue::Int(8));
                props.insert("ItemsPerRow".into(), PropValue::Int(1));
                // How each card appears as the data binds: None (instant), Deal
                // (stacked on the first card, then dealt to final spots), FadeIn,
                // ZoomIn, or ZoomOut. See render's card-appear logic.
                props.insert("PlacementEffect".into(), PropValue::String("None".into()));
                props.insert("CardAppearDuration".into(), PropValue::Int(200));
                props.insert("CloneEvents".into(), PropValue::Bool(true));
                props.insert("PreviewItemCount".into(), PropValue::Int(1));
            }
            ControlType::DataGrid => {
                // "Name:Type" per line (Type ∈ string|number|datetime; default string).
                props.insert("Columns".into(), PropValue::String("".into()));
                // Cell data: rows separated by '\n', cells within a row by TAB.
                // Populated at runtime (e.g. from a bound COBOL table via SET-PROPERTY).
                props.insert("Rows".into(), PropValue::String("".into()));
                props.insert("ReadOnly".into(), PropValue::Bool(false));
                props.insert(
                    "AlternatingRowColor".into(),
                    PropValue::String("#F0F8FF".into()),
                );
                props.insert("AlternatingRowOpacity".into(), PropValue::Int(20));
                // Axis the alternating highlight applies to: Rows (default,
                // legacy), Columns, or None.
                props.insert("AlternatingMode".into(), PropValue::String("Rows".into()));
                props.insert(
                    "HeaderBackgroundColor".into(),
                    PropValue::String("#E0E0E0".into()),
                );
                props.insert(
                    "HeaderForegroundColor".into(),
                    PropValue::String("#000000".into()),
                );
                props.insert("GridLineColor".into(), PropValue::String("#CCCCCC".into()));
                props.insert("GridBackgroundImage".into(), PropValue::String("".into()));
                props.insert(
                    "GridBackgroundPattern".into(),
                    PropValue::String("None".into()),
                );
                props.insert(
                    "RowBackgroundPattern".into(),
                    PropValue::String("None".into()),
                );
                props.insert(
                    "GridBackgroundImageMode".into(),
                    PropValue::String("Fill".into()),
                );
                props.insert("SelectionMode".into(), PropValue::String("Row".into()));
                props.insert("RowHeight".into(), PropValue::Int(22));
                props.insert("AllowSorting".into(), PropValue::Bool(true));
                props.insert("AllowColumnResize".into(), PropValue::Bool(true));
                props.insert("AllowColumnReorder".into(), PropValue::Bool(true));
                props.insert("AllowRowResize".into(), PropValue::Bool(true));
                props.insert(DATAGRID_ADVANCED_PROP.into(), PropValue::String("".into()));
                props.insert("ShowRowNumbers".into(), PropValue::Bool(false));
                props.insert("ShowColumnFilters".into(), PropValue::Bool(false));
                props.insert("ExportCSV".into(), PropValue::Bool(true));
                props.insert("ShowCSVExportButton".into(), PropValue::Bool(true));
                props.insert("CSVDelimiter".into(), PropValue::String(",".into()));
                props.insert("CSVExportMode".into(), PropValue::String("Filtered".into()));
                props.insert("FrozenColumns".into(), PropValue::Int(0));
                props.insert("FrozenRows".into(), PropValue::Int(0));
                // Soft shadow cast by the frozen rows/columns onto the scrolling
                // content (a spreadsheet-style freeze cue).
                props.insert("FrozenShadow".into(), PropValue::Bool(true));
                props.insert("GridLineStyle".into(), PropValue::String("Solid".into()));
                props.insert("RowHeightOverrides".into(), PropValue::String("".into()));
                props.insert("ColumnFilters".into(), PropValue::String("".into()));
                props.insert("SelectableText".into(), PropValue::Bool(true));
            }
            ControlType::TabControl => {
                props.insert("Tabs".into(), PropValue::String("Tab1\nTab2".into()));
                props.insert("TabPosition".into(), PropValue::String("Top".into()));
                props.insert("SelectedTab".into(), PropValue::Int(0));
                props.insert(
                    "ActiveTabColor".into(),
                    PropValue::String("#2C6FD2FF".into()),
                );
                props.insert("TabPadding".into(), PropValue::Int(7));
                // Container behaviour (spec 012).
                props.insert("HScroll".into(), PropValue::Bool(false));
                props.insert("VScroll".into(), PropValue::Bool(false));
                props.insert(
                    "mcp_tool".into(),
                    PropValue::String(TAB_CONTROL_MCP_TOOL.into()),
                );
            }
            ControlType::MenuBar | ControlType::SideMenu => {
                props.insert(
                    "BackgroundColor".into(),
                    PropValue::String("#00000000".into()),
                );
                props.insert(
                    "ForegroundColor".into(),
                    PropValue::String("#E1E6FA".into()),
                );
                props.insert(
                    "HighlightBgColor".into(),
                    PropValue::String("#4488FF".into()),
                );
                props.insert(
                    "HighlightFgColor".into(),
                    PropValue::String("#FFFFFF".into()),
                );
                props.insert(
                    "SelectedBgColor".into(),
                    PropValue::String("#3366CC".into()),
                );
                props.insert(
                    "SelectedFgColor".into(),
                    PropValue::String("#FFFFFF".into()),
                );
                // 049 — a SideMenu fills the window's whole vertical extent by
                // default. A MenuBar is a horizontal strip and has no such
                // choice, so the property belongs to the SideMenu alone.
                if matches!(control_type, ControlType::SideMenu) {
                    props.insert("FullHeight".into(), PropValue::Bool(true));
                    // The pane state the application OPENS in. At run time the
                    // operator's own last choice is remembered per application
                    // and wins over this; this is the starting point.
                    props.insert("Collapsed".into(), PropValue::Bool(false));
                    // How menu-item icons are painted: None | Shadow | Neumorphic.
                    props.insert("IconEffect".into(), PropValue::String("None".into()));
                    // Menu-item icon size, in points, per rail state. Icons are
                    // vectors, so this is a clean scale rather than a resample.
                    //
                    // Two sizes because the two states are two designs: OPEN,
                    // an icon sits beside a label and must not overpower it;
                    // COLLAPSED, the icon IS the row, and the size that read
                    // correctly next to text is small alone on a rail. A form
                    // saved before the collapsed size existed falls back to the
                    // open one, so nothing that was designed changes.
                    props.insert("IconSize".into(), PropValue::Int(22));
                    props.insert("IconSizeCollapsed".into(), PropValue::Int(22));
                    // The three panes: a header carrying the logo, the menu,
                    // and a footer holding the developer's Panel. The menu
                    // pane is whatever height is left between them.
                    props.insert("AppTitle".into(), PropValue::String(String::new()));
                    // The header is the developer's to size and is never
                    // resized at run time; 120 gives the logo box room to
                    // breathe without them having to discover the property.
                    props.insert("HeaderHeight".into(), PropValue::Int(120));
                    props.insert("FooterHeight".into(), PropValue::Int(72));
                    // The breadcrumb frame the rail owns: how tall it is, and
                    // what colour it is. An empty colour keeps the historical
                    // behaviour — the strip follows the content pane's own
                    // backdrop. A taller frame is room for the controls the
                    // developer drops over it (it is chrome, not a container).
                    props.insert(
                        "BreadcrumbHeight".into(),
                        PropValue::Int(DEFAULT_BREADCRUMB_HEIGHT as i64),
                    );
                    props.insert(
                        "BreadcrumbBackgroundColor".into(),
                        PropValue::String(String::new()),
                    );
                    // Where the chain sits in that frame. The height owes
                    // nothing to the font, so a frame taller than its text has
                    // room the developer chooses how to use.
                    props.insert(
                        "BreadcrumbTextAlign".into(),
                        PropValue::String("Middle".into()),
                    );
                    // The chain's own text size and toggle size. 0 means
                    // "as before": the text follows the rail's FontSize and
                    // the toggle is a square of the frame's height. Anything
                    // else is the developer taking one of them over, without
                    // disturbing the other two.
                    props.insert("BreadcrumbFontSize".into(), PropValue::Int(0));
                    props.insert("BreadcrumbIconSize".into(), PropValue::Int(0));
                    // The header logo. Empty = no logo drawn; the header is
                    // the developer's, not a placeholder's.
                    props.insert("HeaderImage".into(), PropValue::String(String::new()));
                    // The COLLAPSED rail's mark. A logo drawn for a 200pt
                    // header cannot be read squeezed into a 72pt strip, so the
                    // rail shows this icon instead of shrinking the image.
                    props.insert("HeaderIcon".into(), PropValue::String(String::new()));
                }
            }
            ControlType::StatusBar => {
                props.insert("Items".into(), PropValue::String("".into()));
            }
            // A ToolBar's own frame, which had no properties at all — it drew a
            // hard-wired card and there was no way to change it (operator,
            // 2026-08-17). The defaults are its decision: rounded at 10, NO
            // border, and a fully transparent background, so a new toolbar reads
            // as buttons sitting on the form rather than as a panel laid over it.
            ControlType::ToolBar => {
                props.insert("Items".into(), PropValue::String("".into()));
                props.insert("CornerRadius".into(), PropValue::Int(10));
                props.insert("BorderStyle".into(), PropValue::String("None".into()));
                props.insert("BorderColor".into(), PropValue::String("#888888".into()));
                props.insert("BorderWidth".into(), PropValue::Int(1));
                // 100 % transparent. The frame is there to be turned ON, not off.
                props.insert("Transparency".into(), PropValue::Int(100));
                // One group, one button, folder-open — so a dropped ToolBar shows
                // what a toolbar is instead of an empty strip.
                props.insert(
                    crate::toolbar::TOOLBAR_DEF_PROP.into(),
                    PropValue::String(
                        crate::toolbar::ToolbarDef::example()
                            .to_json()
                            .unwrap_or_default(),
                    ),
                );
            }
            ControlType::Line => {
                props.insert("LineColor".into(), PropValue::String("#000000".into()));
                props.insert("LineThickness".into(), PropValue::Int(1));
                props.insert(
                    "LineDirection".into(),
                    PropValue::String("Horizontal".into()),
                );
                props.insert("DashStyle".into(), PropValue::String("Solid".into()));
                props.insert("RoundedEnds".into(), PropValue::Bool(false));
            }
            ControlType::DateTimePicker => {
                props.insert("Value".into(), PropValue::String("".into()));
                props.insert("Format".into(), PropValue::String("Short".into()));
                props.insert("CustomFormat".into(), PropValue::String("".into()));
                props.insert("ShowUpDown".into(), PropValue::Bool(false));
                props.insert("MinimumDate".into(), PropValue::String("".into()));
                props.insert("MaximumDate".into(), PropValue::String("".into()));
                props.insert("BorderColor".into(), PropValue::String("#888888".into()));
            }
            ControlType::NumericUpDown => {
                props.insert("Value".into(), PropValue::Int(0));
                props.insert("Minimum".into(), PropValue::Int(0));
                props.insert("Maximum".into(), PropValue::Int(100));
                props.insert("Step".into(), PropValue::Int(1));
                props.insert("DecimalPlaces".into(), PropValue::Int(0));
                props.insert("ThousandsSeparator".into(), PropValue::Bool(false));
                props.insert("ReadOnly".into(), PropValue::Bool(false));
                props.insert("BorderColor".into(), PropValue::String("#888888".into()));
            }
            ControlType::TreeView => {
                props.insert(
                    "Items".into(),
                    PropValue::String("Node 1\n  Child 1\n  Child 2\nNode 2".into()),
                );
                props.insert("AllowEdit".into(), PropValue::Bool(false));
                props.insert("CheckBoxes".into(), PropValue::Bool(false));
                props.insert("ShowLines".into(), PropValue::Bool(true));
                props.insert("ShowRootLines".into(), PropValue::Bool(true));
                props.insert("Sorted".into(), PropValue::Bool(false));
                props.insert("HotTracking".into(), PropValue::Bool(false));
                props.insert("LineColor".into(), PropValue::String("#AAAAAA".into()));
                props.insert("BorderColor".into(), PropValue::String("#888888".into()));
            }
            ControlType::Splitter => {
                props.insert("Orientation".into(), PropValue::String("Horizontal".into()));
                props.insert("MinimumSize".into(), PropValue::Int(25));
                props.insert("SplitPosition".into(), PropValue::Int(100));
                props.insert("BorderColor".into(), PropValue::String("#CCCCCC".into()));
            }
            ControlType::Timer => {
                props.insert("Interval".into(), PropValue::Int(1000)); // milliseconds
                props.insert("Enabled".into(), PropValue::Bool(true));
            }
            ControlType::Shape => {
                props.insert("ShapeType".into(), PropValue::String("Rectangle".into()));
                props.insert("FormStyle".into(), PropValue::Bool(true));
                props.insert(
                    "FillColor".into(),
                    PropValue::String(DEFAULT_SHAPE_FILL_COLOR.into()),
                );
                props.insert("FillStyle".into(), PropValue::String("Solid".into()));
                props.insert("LineColor".into(), PropValue::String("#000000".into()));
                props.insert("LineThickness".into(), PropValue::Int(1));
                props.insert("LineStyle".into(), PropValue::String("Solid".into()));
            }
            ControlType::AgentObject => {
                // Network / LLM connection
                props.insert(
                    "AgentURL".into(),
                    PropValue::String("http://localhost:11434".into()),
                );
                props.insert("AgentModel".into(), PropValue::String("llama3.2".into()));
                props.insert("AgentAPI".into(), PropValue::String("Ollama".into())); // Ollama | LMStudio | OpenAI | Custom
                props.insert("AgentAPIKey".into(), PropValue::String("".into()));
                props.insert("AgentEndpoint".into(), PropValue::String("".into())); // override default endpoint
                                                                                    // Behaviour
                props.insert(
                    "SystemPrompt".into(),
                    PropValue::String("You are a helpful assistant.".into()),
                );
                props.insert("Temperature".into(), PropValue::Int(70)); // stored as int 0-100 (0.0-1.0)
                props.insert("MaximumTokens".into(), PropValue::Int(1024));
                props.insert("Stream".into(), PropValue::Bool(true));
                props.insert("TimeoutSeconds".into(), PropValue::Int(30));
                // Target controls — comma-sep list of IDs this agent is allowed to modify
                props.insert("TargetControls".into(), PropValue::String("".into()));
                props.insert("ResponseDataItem".into(), PropValue::String("".into()));
            }
            ControlType::Slider => {
                props.insert("Minimum".into(), PropValue::Int(0));
                props.insert("Maximum".into(), PropValue::Int(100));
                props.insert("Value".into(), PropValue::Int(0));
                props.insert("Step".into(), PropValue::Int(10));
                props.insert("LargeChange".into(), PropValue::Int(20)); // Page Up/Down increment
                props.insert("Orientation".into(), PropValue::String("Horizontal".into())); // Horizontal | Vertical
                props.insert("TickFrequency".into(), PropValue::Int(10)); // Draw a tick every N units
                props.insert("TickStyle".into(), PropValue::String("Bottom".into())); // None | Top | Bottom | Both
                                                                                      // Back color → track body (along the scale); Fore color → knob.
                                                                                      // Defaulting to the standard sentinels keeps the Liquid Glass look
                                                                                      // until the user picks a colour (the renderer only overrides on a
                                                                                      // non-default value). Exposing these makes the Appearance section's
                                                                                      // Back/Fore colour rows appear for the Slider.
                props.insert(
                    "BackgroundColor".into(),
                    PropValue::String(DEFAULT_BACKGROUND_COLOR.into()),
                );
                props.insert(
                    "ForegroundColor".into(),
                    PropValue::String(DEFAULT_FOREGROUND_COLOR.into()),
                );
                props.insert("TrackColor".into(), PropValue::String("#AAAAAA".into()));
                props.insert("ThumbColor".into(), PropValue::String("#0078D7".into()));
                props.insert("FillColor".into(), PropValue::String("#0078D7".into())); // filled portion of track
                props.insert("ShowValue".into(), PropValue::Bool(false)); // label current value
                props.insert("DataItem".into(), PropValue::String("".into()));
            }
            // Knob (spec 039). The shared painter draws the dial at whatever
            // size the control was given, so there is no Size preset to pick —
            // a knob is the size it was drawn. Accent takes any colour from the
            // picker; the six names it was once limited to still resolve, so
            // forms saved with one keep their colour (`paint::knob_accent`).
            ControlType::Knob => {
                props.insert("Minimum".into(), PropValue::Int(0));
                props.insert("Maximum".into(), PropValue::Int(100));
                props.insert("Value".into(), PropValue::Int(0));
                props.insert("Step".into(), PropValue::Int(1));
                props.insert("Accent".into(), PropValue::String("Blue".into()));
                // The dial's own parts, the way the Gauge names its Color and
                // NeedleColor: empty means the developer chose nothing, so the
                // theme paints what it has always painted.
                props.insert("FaceColor".into(), PropValue::String("".into()));
                props.insert("RimColor".into(), PropValue::String("".into()));
                props.insert("TrackColor".into(), PropValue::String("".into()));
                props.insert("Bipolar".into(), PropValue::Bool(false));
                props.insert("ShowValue".into(), PropValue::Bool(true));
                props.insert("DefaultValue".into(), PropValue::Int(0));
                props.insert("Label".into(), PropValue::String("".into()));
            }
            // Gauge (spec 039): egui-elegance's RadialGauge/LinearGauge/
            // ProgressRing, selected by GaugeStyle. Read-only (R10) — no
            // drag/click ever changes Value. WarningThreshold/
            // CriticalThreshold drive GaugeZones auto-colouring when both
            // are set (spec.md R8; plan.md §4 Decision 4's "free feature").
            ControlType::Gauge => {
                props.insert("GaugeStyle".into(), PropValue::String("Radial".into())); // Radial | Linear | Donut
                props.insert("Minimum".into(), PropValue::Int(0));
                props.insert("Maximum".into(), PropValue::Int(100));
                props.insert("Value".into(), PropValue::Int(0));
                props.insert("Color".into(), PropValue::String("".into())); // empty = theme accent
                props.insert("WarningThreshold".into(), PropValue::String("".into())); // empty = zones off
                props.insert("CriticalThreshold".into(), PropValue::String("".into()));
                props.insert("Unit".into(), PropValue::String("".into()));
                props.insert("Text".into(), PropValue::String("".into())); // empty = widget's own readout
                // Radial + Donut (the needle); the scale is Radial-only
                props.insert("ShowNeedle".into(), PropValue::Bool(true));
                props.insert("ShowScale".into(), PropValue::Bool(true));
                // The needle's own ink. Empty = whatever colour the meter is
                // drawn in (its `Color`, a zone, or the theme accent), which is
                // how the needle has always been painted.
                props.insert("NeedleColor".into(), PropValue::String("".into()));
                // Where the readout sits, Radial only: `Up` inside the dial as
                // it always has, `Down` under the needle's pivot. A Donut reads
                // out in the hole and a Linear under its bar — neither has two
                // places to choose between.
                props.insert("ReadoutPosition".into(), PropValue::String("Up".into())); // Up | Down
                // Linear-only
                props.insert("BarHeight".into(), PropValue::Int(14));
                props.insert("ShowThumb".into(), PropValue::Bool(true));
                // Donut-only
                props.insert("StrokeWidth".into(), PropValue::Int(8));
            }
            // Switch (spec 039): egui-elegance's `Switch` widget — Checked
            // + a fixed theme Accent only (no arbitrary OnColor/OffColor;
            // the widget has no such properties — plan.md §4 Decision 4).
            ControlType::Switch => {
                props.insert("Checked".into(), PropValue::Bool(false));
                props.insert("Accent".into(), PropValue::String("Blue".into()));
            }
            // FileDropZone (spec 039): egui-elegance's `FileDropZone`.
            // DroppedFiles is runtime-only (populated by a drop or the
            // native picker, R14/R15) — not a designer-editable default.
            ControlType::FileDropZone => {
                props.insert("Hint".into(), PropValue::String("".into()));
                // What the zone takes, and where it puts it (see `dropzone`).
                // Empty/0 keeps the original behaviour: take anything, of any
                // size, and leave it where it lies.
                props.insert("AllowedExtensions".into(), PropValue::String("".into()));
                props.insert("MaximumFileSizeKB".into(), PropValue::Int(0));
                props.insert("DestinationFolder".into(), PropValue::String("".into()));
                // Off = a drop copies then and there, which is what every zone
                // has always done. On = the drop only STAGES, the files are
                // listed for review, and the form's own COBOL calls
                // `CommitFiles()` when the person doing the dropping is happy.
                props.insert("StageOnly".into(), PropValue::Bool(false));
                // The ListBox that shows what is staged, one tick-boxed row per
                // file. Seeded with the companion the designer creates next to a
                // new zone; empty (or naming a control that is gone) simply
                // means no list, and the zone still works.
                props.insert("FileListControl".into(), PropValue::String("".into()));
            }
            // Maps (spec 039 T8): the OpenStreetMap basemap needs no key at
            // all (R33) — `ApiKeySource` only gates the google_maps-backed
            // Directions/Geocoding/Places/Distance-Matrix calls (R17, R20).
            // `Markers` is a serialized list (id/lat/lng/label/info — see
            // plan.md §3), not a `PropValue` variant of its own, matching
            // how DataGrid's advanced column metadata is already stored.
            ControlType::Maps => {
                props.insert("CenterLat".into(), PropValue::String("0".into()));
                props.insert("CenterLng".into(), PropValue::String("0".into()));
                props.insert("Zoom".into(), PropValue::Int(2));
                props.insert("Markers".into(), PropValue::String("".into()));
                props.insert("ApiKeySource".into(), PropValue::String("".into()));
            }
            ControlType::RestClient => {
                props.insert(
                    "BaseURL".into(),
                    PropValue::String("https://api.example.com".into()),
                );
                props.insert("DefaultMethod".into(), PropValue::String("GET".into())); // GET | POST | PUT | PATCH | DELETE
                props.insert("AuthType".into(), PropValue::String("None".into())); // None | Bearer | Basic | APIKey
                props.insert("AuthToken".into(), PropValue::String("".into()));
                props.insert("DefaultHeaders".into(), PropValue::String("".into())); // key:value pairs, newline-separated
                props.insert("TimeoutSeconds".into(), PropValue::Int(30));
                props.insert("FollowRedirects".into(), PropValue::Bool(true));
                props.insert("VerifyTLS".into(), PropValue::Bool(true));
                // COBOL data items
                props.insert("RequestDataItem".into(), PropValue::String("".into())); // JSON body source
                props.insert("ResponseDataItem".into(), PropValue::String("".into())); // where response goes
                props.insert("StatusDataItem".into(), PropValue::String("".into()));
                // HTTP status code
                // Async I/O (spec 032): REST is async by default. Mode = Sync
                // restores blocking same-statement results. Busy is a runtime
                // flag; TimeoutMs is the async operation timeout (0 falls back to
                // TimeoutSeconds).
                props.insert("Mode".into(), PropValue::String("Async".into())); // Async | Sync
                props.insert("Busy".into(), PropValue::Bool(false));
                props.insert("TimeoutMs".into(), PropValue::Int(30000));
            }
            // WebSearch (spec 039 T14): Google Custom Search JSON API client.
            // No key property here — like Maps (R31/R33), the resolved
            // "google-custom-search" key (T7) is a runtime-only seed
            // property, never a design-time literal (R30).
            ControlType::WebSearch => {
                props.insert("SearchEngineId".into(), PropValue::String("".into()));
                props.insert("Query".into(), PropValue::String("".into()));
                props.insert("NumResults".into(), PropValue::Int(10));
                props.insert("SafeSearch".into(), PropValue::String("Off".into())); // Off | Medium | High
                // Async I/O (spec 032): same Mode/Busy/TimeoutMs shape as
                // RestClient above.
                props.insert("Mode".into(), PropValue::String("Async".into())); // Async | Sync
                props.insert("Busy".into(), PropValue::Bool(false));
                props.insert("TimeoutMs".into(), PropValue::Int(30000));
            }
            ControlType::SqlDatabase => {
                // Connection
                props.insert("Driver".into(), PropValue::String("sqlite".into())); // sqlite | postgres | mysql | mssql
                props.insert(
                    "ConnectionString".into(),
                    PropValue::String("sqlite::memory:".into()),
                );
                props.insert("AutoConnect".into(), PropValue::Bool(false));
                props.insert("MaximumConnections".into(), PropValue::Int(5));
                // COBOL object data items generated in WORKING-STORAGE
                props.insert("ConnectionDataItem".into(), PropValue::String("".into())); // e.g. conn1
                props.insert("ResultSetDataItem".into(), PropValue::String("".into()));
                // e.g. resultset1
                // COBOL paragraphs
                // Async I/O (spec 032): Sync by default (fast local ops, max
                // speed); opt into Async per control. Busy/TimeoutMs mirror REST.
                props.insert("Mode".into(), PropValue::String("Sync".into())); // Sync | Async
                props.insert("Busy".into(), PropValue::Bool(false));
                props.insert("TimeoutMs".into(), PropValue::Int(0));
            }
            ControlType::IndexedFile => {
                props.insert("IndexedFile".into(), PropValue::String("".into()));
                props.insert("OpenMode".into(), PropValue::String("INPUT".into()));
                props.insert("LoadStrategy".into(), PropValue::String("Disk".into()));
                props.insert("AutoOpen".into(), PropValue::Bool(false));
                props.insert("RecordName".into(), PropValue::String("".into()));
                props.insert("KeyName".into(), PropValue::String("".into()));
                props.insert("CurrentKeyDataItem".into(), PropValue::String("".into()));
                props.insert("StatusDataItem".into(), PropValue::String("".into()));
                props.insert("CurrentRecordDataItem".into(), PropValue::String("".into()));
                props.insert("OperatorName".into(), PropValue::String("".into()));
                // Async I/O (spec 032): Sync by default; opt into Async per control.
                props.insert("Mode".into(), PropValue::String("Sync".into())); // Sync | Async
                props.insert("Busy".into(), PropValue::Bool(false));
                props.insert("TimeoutMs".into(), PropValue::Int(0));
            }

            // ── Charts ────────────────────────────────────────────────────────
            ControlType::BarChart
            | ControlType::LineChart
            | ControlType::PieChart
            | ControlType::AreaChart
            | ControlType::ScatterChart
            | ControlType::DonutChart => {
                // Visual
                props.insert("Title".into(), PropValue::String("".into()));
                props.insert("ShowLegend".into(), PropValue::Bool(true));
                props.insert("ShowGridLines".into(), PropValue::Bool(true));
                // Independent X/Y axis-line visibility (default on).
                props.insert("ShowXAxis".into(), PropValue::Bool(true));
                props.insert("ShowYAxis".into(), PropValue::Bool(true));
                props.insert("ShowTooltips".into(), PropValue::Bool(true));
                props.insert("AnimateOnLoad".into(), PropValue::Bool(true));
                // When true, the panel background fill and border frame are not
                // drawn — only the chart content (grid, axes, labels, data) shows,
                // letting the chart sit transparently on the form.
                props.insert("HideBackground".into(), PropValue::Bool(false));
                // Monochrome mode (spec 013): render data in tonal variations of a
                // single base colour instead of the multi-colour palette. Grid
                // visibility stays on the existing `ShowGridLines` prop.
                props.insert("Monochrome".into(), PropValue::Bool(false));
                props.insert(
                    "MonochromeColor".into(),
                    PropValue::String("#3F6FB5".into()),
                ); // medium blue
                   // Diagonal gradient: when on, data elements shade from ~20% lighter
                   // (top-left) to ~20% darker (bottom-right) of MonochromeColor.
                props.insert("MonochromeGradient".into(), PropValue::Bool(false));
                props.insert("XAxisLabel".into(), PropValue::String("".into()));
                props.insert("YAxisLabel".into(), PropValue::String("".into()));
                props.insert(
                    "SeriesColors".into(),
                    PropValue::String("#4C9BE8,#E87A4C,#4CE87A,#E84C9B,#9B4CE8,#E8C84C".into()),
                ); // comma-sep hex
                   // Data binding — COBOL table
                props.insert("DataSource".into(), PropValue::String("".into())); // WS data-item name
                props.insert("DataCount".into(), PropValue::String("".into())); // count / tally item
                props.insert("LabelField".into(), PropValue::String("".into())); // sub-field for X labels
                props.insert("ValueFields".into(), PropValue::String("".into())); // comma-sep sub-fields for Y series
                props.insert("SeriesLabels".into(), PropValue::String("".into())); // display names for series
                                                                                   // COBOL paragraphs
                                                                                   // Bar/Line/Area specifics
                if matches!(control_type, ControlType::BarChart) {
                    props.insert("Horizontal".into(), PropValue::Bool(false));
                    props.insert("Stacked".into(), PropValue::Bool(false));
                    props.insert("BarCornerRadius".into(), PropValue::Int(3));
                }
                if matches!(
                    control_type,
                    ControlType::LineChart | ControlType::AreaChart
                ) {
                    props.insert("Smooth".into(), PropValue::Bool(true));
                    props.insert("ShowPoints".into(), PropValue::Bool(true));
                    props.insert("PointRadius".into(), PropValue::Int(4));
                    if matches!(control_type, ControlType::AreaChart) {
                        props.insert("FillAlpha".into(), PropValue::Int(40)); // 0-100%
                        props.insert("Stacked".into(), PropValue::Bool(false));
                    }
                }
                if matches!(
                    control_type,
                    ControlType::PieChart | ControlType::DonutChart
                ) {
                    props.insert("ShowLabels".into(), PropValue::Bool(true));
                    props.insert("LabelFormat".into(), PropValue::String("percent".into())); // percent | value | label
                    if matches!(control_type, ControlType::DonutChart) {
                        props.insert("InnerRadius".into(), PropValue::Int(40)); // % of outer radius
                    }
                }
                if matches!(control_type, ControlType::ScatterChart) {
                    props.insert("BubbleField".into(), PropValue::String("".into())); // field for bubble size
                    props.insert("BubbleScale".into(), PropValue::Int(20)); // max bubble radius px
                }
            }

            _ => {}
        }

        // Unified corner radius on every bordered visual control (spec 016).
        // Canonical key `CornerRadius`; the renderer also reads the legacy
        // container `BorderRadius` as an alias. Default preserves each control's
        // current look (Button 3, charts 8, everything else 0).
        let corner_default: Option<i64> = match control_type {
            ControlType::Button => Some(3),
            ControlType::BarChart
            | ControlType::LineChart
            | ControlType::PieChart
            | ControlType::AreaChart
            | ControlType::ScatterChart
            | ControlType::DonutChart => Some(8),
            // A deliberate choice, not the old look (operator, 2026-08-16):
            // the bar's artwork was hard-wired to a 2 px round, so once
            // `CornerRadius` actually reached the paint a seeded 0 would have
            // squared off every existing bar. 10 rounds it properly.
            ControlType::ProgressBar => Some(10),
            ControlType::TextBox
            | ControlType::ComboBox
            | ControlType::ListBox
            | ControlType::TreeView
            | ControlType::PictureBox
            | ControlType::DataGrid
            | ControlType::NumericUpDown
            | ControlType::DateTimePicker
            | ControlType::Slider
            | ControlType::Shape
            | ControlType::CheckBox
            | ControlType::RadioButton
            | ControlType::GroupBox
            | ControlType::Panel
            | ControlType::TabControl
            | ControlType::Switch
            | ControlType::FileDropZone => Some(0),
            _ => None,
        };
        if let Some(d) = corner_default {
            props
                .entry("CornerRadius".to_owned())
                .or_insert(PropValue::Int(d));
        }
        if control_type.is_data_input_control() {
            props.insert(
                "ForegroundColor".into(),
                PropValue::String("#000000".into()),
            );
        }

        Self {
            id: id_str,
            control_type,
            rect: Rect::new(x, y, w, h),
            tab_order: 0,
            z_order: 0,
            visible: true,
            enabled: true,
            properties: props,
            events: Vec::new(),
            children: Vec::new(),
            animations: Vec::new(),
            parent: None,
            tab: None,
        }
    }

    /// `true` if this control can contain other controls (spec 012).
    pub fn is_container(&self) -> bool {
        matches!(
            self.control_type,
            ControlType::GroupBox | ControlType::Panel | ControlType::TabControl
        )
    }

    pub fn explicit_control_array_id(&self) -> Option<String> {
        if !matches!(self.control_type, ControlType::GroupBox) {
            return None;
        }
        if !self
            .get_prop("IsRepeatingGroup")
            .map(PropValue::as_bool)
            .unwrap_or(false)
        {
            return None;
        }

        let array_name = self
            .get_prop("ArrayName")
            .map(PropValue::as_str)
            .unwrap_or("")
            .trim();
        if array_name.is_empty() {
            Some(self.id.clone())
        } else {
            Some(array_name.to_owned())
        }
    }

    pub fn approved_binding_target_kind(&self) -> Option<ApprovedBindingTargetKind> {
        if self.explicit_control_array_id().is_some() {
            return Some(ApprovedBindingTargetKind::ControlArray);
        }
        self.control_type.approved_binding_target_kind()
    }

    pub fn binding_target_descriptor(&self) -> Option<BindingTargetDescriptor> {
        match self.approved_binding_target_kind()? {
            ApprovedBindingTargetKind::DataGrid => Some(BindingTargetDescriptor::DataGrid {
                control_id: self.id.clone(),
            }),
            ApprovedBindingTargetKind::Chart(chart_kind) => Some(BindingTargetDescriptor::Chart {
                control_id: self.id.clone(),
                chart_kind,
            }),
            ApprovedBindingTargetKind::ComboBox => Some(BindingTargetDescriptor::ComboBox {
                control_id: self.id.clone(),
            }),
            ApprovedBindingTargetKind::ListBox => Some(BindingTargetDescriptor::ListBox {
                control_id: self.id.clone(),
            }),
            ApprovedBindingTargetKind::ControlArray => {
                Some(BindingTargetDescriptor::ControlArray {
                    array_id: self
                        .explicit_control_array_id()
                        .unwrap_or_else(|| self.id.clone()),
                    member_control_ids: self
                        .children
                        .iter()
                        .map(|child| child.id.clone())
                        .collect(),
                })
            }
            ApprovedBindingTargetKind::ScalarControl => {
                Some(BindingTargetDescriptor::ScalarControl {
                    control_id: self.id.clone(),
                })
            }
            ApprovedBindingTargetKind::MarkerCollection => {
                Some(BindingTargetDescriptor::MarkerCollection {
                    control_id: self.id.clone(),
                })
            }
        }
    }

    /// The property a `ScalarControl` binding writes to: `Value` for
    /// Knob/Gauge, `Checked` for Switch. `None` for any other control type
    /// (spec 039 R21) — a caller checking this first is how a Guardian
    /// validation or an apply step tells "not a scalar target" from "a
    /// scalar target with no known property," which should never happen for
    /// an approved `ScalarControl` kind but is safer to make explicit than
    /// to default silently to one property name.
    pub fn scalar_binding_property(&self) -> Option<&'static str> {
        match self.control_type {
            ControlType::Knob | ControlType::Gauge => Some("Value"),
            ControlType::Switch => Some("Checked"),
            _ => None,
        }
    }

    /// The interior rectangle into which child controls are placed and clipped.
    /// Insets the control's `rect` for the border. GroupBox captions are painted
    /// later as overlays, while TabControl tabs reserve space based on
    /// `TabPosition`. Non-containers return their plain `rect` (spec 012).
    pub fn content_rect(&self) -> Rect {
        let r = self.rect;
        match self.control_type {
            ControlType::GroupBox => {
                // Children are clipped to the inner area: just inside the border on
                // every side. The caption is painted later as an overlay, so it
                // must not shrink the clipping area; otherwise the child clip
                // leaves a rectangular band that does not follow the rounded path.
                let b = 2;
                Rect::new(r.x + b, r.y + b, (r.w - 2 * b).max(0), (r.h - 2 * b).max(0))
            }
            ControlType::Panel => Rect::new(r.x + 2, r.y + 2, (r.w - 4).max(0), (r.h - 4).max(0)),
            ControlType::TabControl => {
                let inset = self.tab_strip_extent();
                match self.tab_position().as_str() {
                    "bottom" => {
                        Rect::new(r.x + 2, r.y + 2, (r.w - 4).max(0), (r.h - inset - 4).max(0))
                    }
                    "left" => Rect::new(
                        r.x + inset + 2,
                        r.y + 2,
                        (r.w - inset - 4).max(0),
                        (r.h - 4).max(0),
                    ),
                    "right" => {
                        Rect::new(r.x + 2, r.y + 2, (r.w - inset - 4).max(0), (r.h - 4).max(0))
                    }
                    _ => Rect::new(
                        r.x + 2,
                        r.y + inset + 2,
                        (r.w - 4).max(0),
                        (r.h - inset - 4).max(0),
                    ),
                }
            }
            _ => r,
        }
    }

    pub fn tab_position(&self) -> String {
        self.get_prop("TabPosition")
            .map(|v| v.as_str().to_ascii_lowercase())
            .unwrap_or_else(|| "top".to_string())
    }

    pub fn tab_strip_height(&self) -> i32 {
        26
    }

    pub fn tab_padding(&self) -> i32 {
        self.get_prop("TabPadding")
            .map(|v| v.as_i64())
            .unwrap_or(7)
            .clamp(0, 64) as i32
    }

    pub fn tab_content_top_inset(&self) -> i32 {
        self.tab_strip_height() + self.tab_padding()
    }

    pub fn tab_strip_extent(&self) -> i32 {
        match self.tab_position().as_str() {
            "left" | "right" => {
                let tabs = self
                    .get_prop("Tabs")
                    .map(|v| v.as_str())
                    .unwrap_or_default();
                tabs.lines()
                    .map(|t| (t.chars().count() as i32 * 7 + 18).clamp(56, 160))
                    .max()
                    .unwrap_or(80)
                    + self.tab_padding().max(0)
            }
            _ => self.tab_content_top_inset(),
        }
    }

    /// Whether the control's position is anchored (locked against mouse dragging).
    /// Only an explicit boolean/integer `Anchor` counts as anchored; legacy string
    /// values (e.g. the old `"Top,Left"` anchor edges) are treated as unanchored so
    /// existing forms don't silently lock every control on load.
    pub fn is_anchored(&self) -> bool {
        match self.get_prop("Anchor") {
            Some(PropValue::Bool(b)) => *b,
            Some(PropValue::Int(n)) => *n != 0,
            _ => false,
        }
    }

    pub fn get_prop(&self, name: &str) -> Option<&PropValue> {
        self.properties.get(name).or_else(|| {
            let lower = name.to_ascii_lowercase();
            self.properties
                .iter()
                .find(|(k, _)| k.to_ascii_lowercase() == lower)
                .map(|(_, v)| v)
        })
    }

    /// 049 — does this SideMenu fill the window's whole vertical extent?
    /// **Absent means yes**: the property defaults to on, and a `.cfrm`
    /// written before it existed must behave like a freshly dropped SideMenu.
    /// Meaningless (and always `false`) on any other control type.
    pub fn side_menu_full_height(&self) -> bool {
        if self.control_type != ControlType::SideMenu {
            return false;
        }
        self.get_prop("FullHeight").map(|v| v.as_bool()).unwrap_or(true)
    }

    /// 049 — does this SideMenu start collapsed? The designed state the
    /// application opens in; at run time the operator's remembered choice
    /// takes precedence. Absent means open. Always `false` off a SideMenu.
    pub fn side_menu_collapsed(&self) -> bool {
        self.control_type == ControlType::SideMenu
            && self.get_prop("Collapsed").map(|v| v.as_bool()).unwrap_or(false)
    }

    /// The height of the breadcrumb frame this SideMenu owns, in points.
    ///
    /// The strip is the SideMenu's chrome — it exists because the rail does,
    /// and it is styled from the rail's own palette — so its height is the
    /// rail's property too. Absent (a form written before the property
    /// existed) means [`DEFAULT_BREADCRUMB_HEIGHT`].
    pub fn breadcrumb_height(&self) -> f32 {
        if self.control_type != ControlType::SideMenu {
            return DEFAULT_BREADCRUMB_HEIGHT;
        }
        self.get_prop("BreadcrumbHeight")
            .map(|v| v.as_i64() as f32)
            .filter(|h| *h >= MIN_BREADCRUMB_HEIGHT)
            .unwrap_or(DEFAULT_BREADCRUMB_HEIGHT)
    }

    /// What a freshly created control of `kind` carries for `key`.
    ///
    /// An inspector row for a form saved before a property existed has nothing
    /// to show, and showing 0 is a lie: it reads as "off" while the renderer is
    /// happily using the real default. Reading the answer back from
    /// [`Control::new`] keeps ONE list of defaults rather than a second one
    /// that drifts.
    pub fn default_prop(kind: ControlType, key: &str) -> Option<PropValue> {
        Control::new("", kind, 0, 0).get_prop(key).cloned()
    }

    /// The breadcrumb text's OWN size, when the developer set one.
    ///
    /// `None` means the chain keeps following the rail's `FontSize`, which is
    /// what it always did — but that made the two impossible to set apart: the
    /// menu labels and the navigation chain are different text at different
    /// sizes, and one property could only ever move both.
    pub fn breadcrumb_font_size(&self) -> Option<f32> {
        if self.control_type != ControlType::SideMenu {
            return None;
        }
        self.get_prop("BreadcrumbFontSize")
            .map(|v| v.as_i64() as f32)
            .filter(|s| *s > 0.0)
            .map(|s| s.clamp(4.0, 200.0))
    }

    /// The breadcrumb toggle's OWN size, when the developer set one.
    ///
    /// `None` keeps the historical rule — the toggle is a square of the frame's
    /// height — which quietly tied the arrow to a property that is about the
    /// band, not about the control in it: raising the frame to make room for
    /// your own controls grew the arrow along with it.
    pub fn breadcrumb_icon_size(&self) -> Option<f32> {
        if self.control_type != ControlType::SideMenu {
            return None;
        }
        self.get_prop("BreadcrumbIconSize")
            .map(|v| v.as_i64() as f32)
            .filter(|s| *s > 0.0)
            .map(|s| s.clamp(8.0, 200.0))
    }

    /// The breadcrumb frame's own background colour, when the developer chose
    /// one. `None` — the default — means the strip keeps following the content
    /// pane's backdrop, so it reads as the top of the content area rather than
    /// as a band bolted above it.
    pub fn breadcrumb_background(&self) -> Option<String> {
        if self.control_type != ControlType::SideMenu {
            return None;
        }
        self.get_prop("BreadcrumbBackgroundColor")
            .map(|v| v.as_str().trim().to_owned())
            .filter(|s| !s.is_empty())
    }

    /// 049 — the height of a SideMenu's footer pane, in points.
    pub fn side_menu_footer_height(&self) -> i32 {
        self.get_prop("FooterHeight")
            .map(|v| v.as_i64() as i32)
            .filter(|h| *h >= 0)
            .unwrap_or(DEFAULT_SIDE_MENU_FOOTER_H)
    }

    /// 049 — is this the Panel a SideMenu owns in its footer pane?
    ///
    /// It is a normal container in every way that matters to the developer —
    /// selectable, editable, a drop target — but the sidebar owns its position
    /// and size, so the designer refuses to move, resize or delete it.
    pub fn is_side_menu_footer(&self) -> bool {
        self.control_type == ControlType::Panel
            && self
                .get_prop(SIDE_MENU_FOOTER_PROP)
                .map(|v| v.as_bool())
                .unwrap_or(false)
    }

    pub fn set_prop(&mut self, name: impl Into<String>, value: impl Into<PropValue>) {
        let name = name.into();
        if !self.properties.contains_key(&name) {
            if let Some(existing) = self
                .properties
                .keys()
                .find(|k| k.eq_ignore_ascii_case(&name))
                .cloned()
            {
                self.properties.shift_remove(&existing);
            }
        }
        self.properties.insert(name, value.into());
    }

    pub fn display_text(&self) -> String {
        self.get_prop("Caption")
            .or_else(|| self.get_prop("Text"))
            .map(|v| v.to_string())
            .unwrap_or_else(|| self.id.clone())
    }

    /// Bind an event to a paragraph name (legacy API — paragraph is auto-derived in v1.0).
    pub fn bind_event(&mut self, event: impl Into<String>, paragraph: impl Into<String>) {
        let event_s = event.into();
        let para = paragraph.into();
        self.events.retain(|e| e.event != event_s);
        self.events.push(EventBinding::new(event_s, para));
    }

    /// Ensure the control has an `EventBinding` for the given event, creating
    /// one with an auto-derived paragraph name if absent. Returns a mutable ref.
    pub fn ensure_event(&mut self, event: &str) -> &mut EventBinding {
        if !self.events.iter().any(|e| e.event == event) {
            self.events.push(EventBinding::for_control(&self.id, event));
        }
        self.events.iter_mut().find(|e| e.event == event).unwrap()
    }

    /// Add an animation definition. Replaces any existing animation with the same name.
    pub fn add_animation(&mut self, anim: AnimationDef) {
        self.animations.retain(|a| a.name != anim.name);
        self.animations.push(anim);
    }

    pub fn apply_neumorphic_defaults(&mut self) {
        self.set_prop(
            "BackgroundColor",
            PropValue::String(NEUMORPHIC_SURFACE_COLOR.into()),
        );
        if self.control_type.uses_neumorphic_black_foreground() {
            self.set_prop("ForegroundColor", PropValue::String("#000000".into()));
        } else if self
            .get_prop("ForegroundColor")
            .map(|v| {
                let t = v.as_str().trim().trim_start_matches('#');
                t.eq_ignore_ascii_case("FFFFFF") || t.eq_ignore_ascii_case("FFFFFFFF")
            })
            .unwrap_or(false)
        {
            // Coming from Neumorphic Dark (which forces every foreground to
            // white): white text on the light surface has no contrast. Only the
            // dark style's own default is remapped — a user-chosen colour
            // (red, blue, …) is left alone.
            self.set_prop("ForegroundColor", PropValue::String("#000000".into()));
        }
        self.set_prop("CornerRadius", PropValue::Int(15));
        self.set_prop("ShadowEnabled", PropValue::Bool(true));
        self.set_prop("ShadowLightColor", PropValue::String("#FFFFFFFF".into()));
        self.set_prop("ShadowOpacity", PropValue::Int(6));
        self.set_prop("ShadowDirection", PropValue::String("SouthEast".into()));
        self.set_prop("ShadowDistance", PropValue::Int(7));
        self.set_prop("ShadowBlur", PropValue::Bool(true));
        self.set_prop("ShadowBlurStrength", PropValue::Int(8));
        self.set_prop("BackgroundGradientEnabled", PropValue::Bool(true));
        self.set_prop(
            "BackgroundGradientStartColor",
            PropValue::String(NEUMORPHIC_GRADIENT_START.into()),
        );
        self.set_prop(
            "BackgroundGradientEndColor",
            PropValue::String(NEUMORPHIC_GRADIENT_END.into()),
        );
        self.set_prop(
            "BackgroundGradientDirection",
            PropValue::String("South".into()),
        );
    }

    pub fn apply_neumorphic_dark_defaults(&mut self) {
        self.set_prop(
            "BackgroundColor",
            PropValue::String(NEUMORPHIC_DARK_SURFACE_COLOR.into()),
        );
        self.set_prop("ForegroundColor", PropValue::String("#FFFFFFFF".into()));
        self.set_prop("CornerRadius", PropValue::Int(15));
        self.set_prop("ShadowEnabled", PropValue::Bool(true));
        self.set_prop("ShadowOpacity", PropValue::Int(6));
        self.set_prop("ShadowColor", PropValue::String("#000000FF".into()));
        self.set_prop(
            "ShadowLightColor",
            PropValue::String(NEUMORPHIC_DARK_LIGHT_SHADOW.into()),
        );
        self.set_prop("ShadowDirection", PropValue::String("SouthEast".into()));
        self.set_prop("ShadowDistance", PropValue::Int(7));
        self.set_prop("ShadowBlur", PropValue::Bool(true));
        self.set_prop("ShadowBlurStrength", PropValue::Int(8));
        self.set_prop("BackgroundGradientEnabled", PropValue::Bool(true));
        self.set_prop(
            "BackgroundGradientStartColor",
            PropValue::String(NEUMORPHIC_DARK_GRADIENT_START.into()),
        );
        self.set_prop(
            "BackgroundGradientEndColor",
            PropValue::String(NEUMORPHIC_DARK_GRADIENT_END.into()),
        );
        self.set_prop(
            "BackgroundGradientDirection",
            PropValue::String("South".into()),
        );
    }

    pub fn apply_glass_style_defaults(&mut self, style: GlassStyle) {
        match style {
            GlassStyle::Neumorphic => self.apply_neumorphic_defaults(),
            GlassStyle::NeumorphicDark => self.apply_neumorphic_dark_defaults(),
            GlassStyle::Classic | GlassStyle::Enhanced => {}
        }
    }

    /// Remove an animation by name.
    pub fn remove_animation(&mut self, name: &str) {
        self.animations.retain(|a| a.name != name);
    }
}

// ── BgImageMode ───────────────────────────────────────────────────────────────

/// Which Liquid Glass recipe to use for control surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlassStyle {
    /// Original frosted-glass look (shadow + gradient fill + rim stroke).
    #[default]
    Classic,
    /// Enhanced stack: adds inner stroke, full highlight band, micro-noise,
    /// and structural state changes per the Liquid Glass spec.
    Enhanced,
    /// Neumorphic ("soft UI"): dual opposing soft shadows (light top-left,
    /// dark bottom-right) on a flat matte surface. Depth from illumination,
    /// not transparency.
    Neumorphic,
    /// Dark soft-UI variant with charcoal surfaces and non-white highlights.
    NeumorphicDark,
}

impl GlassStyle {
    /// Every selectable style, in the exact spelling `from_str` accepts and
    /// `as_str` returns. Agents are shown this list so they never invent an
    /// identifier — `from_str` silently falls back to `Classic`, so a wrong
    /// spelling is not an error the caller can observe.
    pub const ALL: &'static [&'static str] = &[
        "Classic",
        "Enhanced",
        "Neumorphic Light",
        "Neumorphic Dark",
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            GlassStyle::Classic => "Classic",
            GlassStyle::Enhanced => "Enhanced",
            GlassStyle::Neumorphic => "Neumorphic Light",
            GlassStyle::NeumorphicDark => "Neumorphic Dark",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "Enhanced" => GlassStyle::Enhanced,
            "Neumorphic" | "Neumorphic Light" => GlassStyle::Neumorphic,
            "Neumorphic Dark" => GlassStyle::NeumorphicDark,
            _ => GlassStyle::Classic,
        }
    }

    pub fn is_neumorphic(self) -> bool {
        matches!(self, GlassStyle::Neumorphic | GlassStyle::NeumorphicDark)
    }
}

impl ControlType {
    pub fn is_data_input_control(&self) -> bool {
        matches!(
            self,
            ControlType::TextBox
                | ControlType::CheckBox
                | ControlType::RadioButton
                | ControlType::ListBox
                | ControlType::ComboBox
                | ControlType::DataGrid
                | ControlType::DateTimePicker
                | ControlType::NumericUpDown
                | ControlType::TreeView
                | ControlType::Slider
                | ControlType::Knob
                | ControlType::Switch
        )
    }

    pub fn uses_neumorphic_black_foreground(&self) -> bool {
        self.is_data_input_control()
            || matches!(self, ControlType::Button | ControlType::TabControl)
    }
}

/// How the form background image is scaled / tiled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BgImageMode {
    /// Stretch to fill the form rectangle (may distort).
    #[default]
    Stretch,
    /// Tile the image across the form (repeat like wallpaper).
    Tile,
    /// Center the image without scaling.
    Center,
    /// Scale uniformly to cover the entire form (may clip edges).
    Fill,
    /// Scale uniformly to fit fully inside the form (may leave empty margins).
    Fit,
}

impl BgImageMode {
    pub fn as_str(self) -> &'static str {
        match self {
            BgImageMode::Stretch => "Stretch",
            BgImageMode::Tile => "Tile",
            BgImageMode::Center => "Center",
            BgImageMode::Fill => "Fill",
            BgImageMode::Fit => "Fit",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "Tile" => BgImageMode::Tile,
            "Center" => BgImageMode::Center,
            "Fill" => BgImageMode::Fill,
            "Fit" => BgImageMode::Fit,
            _ => BgImageMode::Stretch,
        }
    }
    pub fn all() -> &'static [&'static str] {
        &["Stretch", "Tile", "Center", "Fill", "Fit"]
    }
}

// ── Form COBOL structure (spec 005) ───────────────────────────────────────────

/// Editable raw-COBOL blocks woven into the form's generated outer program,
/// besides WORKING-STORAGE (which is [`Form::user_ws_source`]). The developer
/// writes normal COBOL — including `GLOBAL`/`EXTERNAL` clauses; codegen inserts
/// each non-empty block into the matching division/section. (BASED-STORAGE and
/// CONSTANT are intentionally out of scope.)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CobolStructure {
    /// ENVIRONMENT DIVISION → CONFIGURATION SECTION → `SPECIAL-NAMES`.
    pub special_names: String,
    /// ENVIRONMENT DIVISION → CONFIGURATION SECTION → `REPOSITORY` (COBOL-2002;
    /// the Rust-FFI type bindings live here).
    pub repository: String,
    /// ENVIRONMENT DIVISION → INPUT-OUTPUT SECTION → `FILE-CONTROL`.
    pub file_control: String,
    /// DATA DIVISION → `FILE SECTION`.
    pub file_section: String,
}

/// The curated "first-cut" Rust-FFI bridge: basic Rust types declared as COBOL
/// classes in `REPOSITORY` form. The literal is the type's path in the Rust
/// hierarchy (analogous to `System.String` in .NET), so a data item can be
/// `USAGE OBJECT REFERENCE RUST-STRING`. New forms start with these.
/// The table itself lives in [`cobolt_ast::rust_types`] — semantic analysis
/// needs the same answer about which `Rust.*` types exist (spec 041 R8), and a
/// second copy here would drift the moment either side gained a type.
pub fn default_repository() -> String {
    cobolt_ast::rust_types::SHIPPED_RUST_TYPES
        .iter()
        .map(|(name, path)| format!("           CLASS {name} IS \"{path}\""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A developer-written named procedure on a form. Woven as a nested program in
/// the form's outer program, callable by name from event handlers and from other
/// user procedures, and able to see the form's `GLOBAL` data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserProcedure {
    /// Procedure name (the nested PROGRAM-ID / CALL target).
    pub name: String,
    /// Full COBOL body (`ENVIRONMENT DIVISION` … `PROCEDURE DIVISION` +
    /// statements), like an event handler's `code`.
    pub code: String,
}

// ── Form ─────────────────────────────────────────────────────────────────────

/// Initial window state of a running form (spec 037 R13). Orthogonal to
/// `Form::full_screen` (R14): leaving fullscreen returns to this state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowState {
    #[default]
    Normal,
    Minimized,
    Maximized,
}

impl WindowState {
    pub fn as_str(self) -> &'static str {
        match self {
            WindowState::Normal => "Normal",
            WindowState::Minimized => "Minimized",
            WindowState::Maximized => "Maximized",
        }
    }

    /// Lenient parse; anything unrecognised is Normal so old/hand-edited
    /// `.cfrm` files never fail to load over this field.
    pub fn from_str(value: &str) -> Self {
        let v = value.trim();
        if v.eq_ignore_ascii_case("Minimized") {
            WindowState::Minimized
        } else if v.eq_ignore_ascii_case("Maximized") {
            WindowState::Maximized
        } else {
            WindowState::Normal
        }
    }
}

/// How a form may be loaded (spec 049 R1).
///
/// `Standalone` is the default, and it is what every `.cfrm` written before this
/// field parses to — so an existing project keeps opening one window per form
/// and nothing about it changes (049 R3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FormFormat {
    /// Its own OS window, per spec 037. Reached by `OpenFormSync`/`OpenFormAsync`.
    #[default]
    Standalone,
    /// Loaded into the application shell's ContentPane by a menu item.
    Embedded,
    /// Valid on either path — a reusable lookup screen that is a modal dialog in
    /// one place and a pane occupant in another.
    Both,
}

impl FormFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            FormFormat::Standalone => "Standalone",
            FormFormat::Embedded => "Embedded",
            FormFormat::Both => "Both",
        }
    }

    /// Lenient parse; anything unrecognised is `Standalone`, so an old or
    /// hand-edited `.cfrm` never fails to load over this field.
    pub fn from_str(value: &str) -> Self {
        let v = value.trim();
        if v.eq_ignore_ascii_case("Embedded") {
            FormFormat::Embedded
        } else if v.eq_ignore_ascii_case("Both") {
            FormFormat::Both
        } else {
            FormFormat::Standalone
        }
    }

    /// May a menu item load this form into the ContentPane? (049 R17)
    pub fn allows_embedded(self) -> bool {
        matches!(self, FormFormat::Embedded | FormFormat::Both)
    }

    /// May `OpenFormSync`/`OpenFormAsync` open this form as a window? (049 R17)
    pub fn allows_standalone(self) -> bool {
        matches!(self, FormFormat::Standalone | FormFormat::Both)
    }
}

/// The shell MenuPane's own background (spec 049 R39), persisted on the main
/// form — the shell's owner (spec Q7). Deliberately the same field shapes as the
/// form background so `paint_backdrop` renders both: one background dialect.
///
/// This is a model-side mirror of `render::Backdrop`, which cannot be persisted
/// directly because it holds a resolved egui `TextureId` instead of the image
/// path. `None` on the form ⇒ the shell's default chrome fill.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuPaneBackground {
    /// `#RRGGBB[AA]`.
    pub color: String,
    pub gradient_enabled: bool,
    pub gradient_start_color: String,
    pub gradient_end_color: String,
    pub gradient_direction: String,
    /// 0–100 (0 = opaque), the form-background convention.
    pub transparency: u8,
    /// Image path; empty = none.
    pub image: String,
    pub image_mode: BgImageMode,
}

impl Default for MenuPaneBackground {
    fn default() -> Self {
        Self {
            color: DEFAULT_FORM_BACKGROUND_COLOR.into(),
            gradient_enabled: false,
            gradient_start_color: String::new(),
            gradient_end_color: String::new(),
            gradient_direction: "South".into(),
            transparency: 0,
            image: String::new(),
            image_mode: BgImageMode::default(),
        }
    }
}

/// Where a form's window opens on screen (operator, 2026-07-31). `Form::x` /
/// `Form::y` are the design-time coordinates; whether they are ever USED
/// depends on this.
///
/// `System` is the default, and it means exactly what happens today for
/// every form that predates this field: the OS/window manager places the
/// window, and `x`/`y` are not applied at all. A `.cfrm` written before this
/// existed has no `start-position` attribute, parses to `System`, and opens
/// exactly where it always has — this field cannot change a single existing
/// form's behavior on its own. `Custom` is the only variant that reads
/// `x`/`y`; the eight edge/corner positions and `Center` instead compute a
/// position from the screen and window size at launch, which `x`/`y` cannot
/// express and are ignored for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FormStartPosition {
    #[default]
    System,
    Custom,
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    Center,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl FormStartPosition {
    pub const ALL: [FormStartPosition; 11] = [
        FormStartPosition::System,
        FormStartPosition::Custom,
        FormStartPosition::TopLeft,
        FormStartPosition::TopCenter,
        FormStartPosition::TopRight,
        FormStartPosition::MiddleLeft,
        FormStartPosition::Center,
        FormStartPosition::MiddleRight,
        FormStartPosition::BottomLeft,
        FormStartPosition::BottomCenter,
        FormStartPosition::BottomRight,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            FormStartPosition::System => "System",
            FormStartPosition::Custom => "Custom",
            FormStartPosition::TopLeft => "TopLeft",
            FormStartPosition::TopCenter => "TopCenter",
            FormStartPosition::TopRight => "TopRight",
            FormStartPosition::MiddleLeft => "MiddleLeft",
            FormStartPosition::Center => "Center",
            FormStartPosition::MiddleRight => "MiddleRight",
            FormStartPosition::BottomLeft => "BottomLeft",
            FormStartPosition::BottomCenter => "BottomCenter",
            FormStartPosition::BottomRight => "BottomRight",
        }
    }

    /// Lenient parse; anything unrecognised is `System` — the one value that
    /// changes nothing for a form that does not (or no longer) name a real
    /// variant, exactly like `WindowState::from_str`.
    pub fn from_str(value: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|v| v.as_str().eq_ignore_ascii_case(value.trim()))
            .unwrap_or(FormStartPosition::System)
    }

    /// Does this variant compute its position from the screen at launch,
    /// rather than from `Form::x`/`Form::y` or the OS default?
    pub fn is_screen_relative(self) -> bool {
        !matches!(self, FormStartPosition::System | FormStartPosition::Custom)
    }
}

/// The window's top-left corner for `pos`, given the screen's and the
/// window's own size — pure geometry, unit-tested without an egui context or
/// a real monitor.
///
/// `None` for `System` (do not touch the OS-placed position at all) and for
/// `Custom` (the caller already has `Form::x`/`Form::y` and does not need
/// this function to tell it what they are). Every screen-relative variant
/// clamps into `0.0..=(screen - window)`, so a window larger than the screen
/// is pinned to the near edge instead of computing a negative, off-screen
/// origin.
pub fn resolved_start_position(
    pos: FormStartPosition,
    screen: (f32, f32),
    window: (f32, f32),
) -> Option<(f32, f32)> {
    if !pos.is_screen_relative() {
        return None;
    }
    let axis = |screen: f32, window: f32, near: bool, mid: bool| {
        if mid {
            ((screen - window) / 2.0).max(0.0)
        } else if near {
            0.0
        } else {
            (screen - window).max(0.0)
        }
    };
    let (sw, sh) = screen;
    let (ww, wh) = window;
    let (left, center_x, right) = (
        axis(sw, ww, true, false),
        axis(sw, ww, false, true),
        axis(sw, ww, false, false),
    );
    let (top, middle_y, bottom) = (
        axis(sh, wh, true, false),
        axis(sh, wh, false, true),
        axis(sh, wh, false, false),
    );
    Some(match pos {
        FormStartPosition::TopLeft => (left, top),
        FormStartPosition::TopCenter => (center_x, top),
        FormStartPosition::TopRight => (right, top),
        FormStartPosition::MiddleLeft => (left, middle_y),
        FormStartPosition::Center => (center_x, middle_y),
        FormStartPosition::MiddleRight => (right, middle_y),
        FormStartPosition::BottomLeft => (left, bottom),
        FormStartPosition::BottomCenter => (center_x, bottom),
        FormStartPosition::BottomRight => (right, bottom),
        FormStartPosition::System | FormStartPosition::Custom => unreachable!(
            "is_screen_relative() already excluded these two variants"
        ),
    })
}

#[derive(Debug, Clone)]
pub struct Form {
    pub name: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub background_color: String,
    /// Optional eight-direction linear background gradient.
    pub background_gradient_enabled: bool,
    pub background_gradient_start_color: String,
    pub background_gradient_end_color: String,
    pub background_gradient_direction: String,
    /// Window-level transparency: 0 = fully opaque, 100 = fully transparent.
    pub transparency: u8,
    /// Optional background image path (empty = none).
    pub background_image: String,
    /// How the background image is scaled / tiled.
    pub bg_image_mode: BgImageMode,
    pub controls: Vec<Control>,
    /// Form-level data bindings. Missing in old `.cfrm` files means no bindings.
    pub data_bindings: Vec<DataBindingDef>,
    /// Form-level animations (e.g. form entrance effect).
    pub animations: Vec<AnimationDef>,
    /// Designer grid dot spacing in pixels (4–64). Default 8.
    pub grid_size: u8,
    /// Whether controls snap to the grid when moved or resized. Default true.
    pub snap_to_grid: bool,
    /// Target device preset name (e.g. "iPhone 15", "Custom"). Controls default width/height.
    pub target: String,

    // ── v1.0 nested-program fields ────────────────────────────────────────────
    /// Raw COBOL text for the WORKING-STORAGE section — emitted verbatim into the
    /// outer program's WS after the generated control-bound items.
    /// The user writes normal COBOL declarations here, including GLOBAL / EXTERNAL.
    pub user_ws_source: String,

    /// Editable COBOL-structure blocks (SPECIAL-NAMES, REPOSITORY, FILE-CONTROL,
    /// FILE SECTION) woven into the generated outer program (spec 005).
    pub cobol_structure: CobolStructure,

    /// Developer-written named procedures, woven as nested programs callable from
    /// event handlers (spec 005).
    pub user_procedures: Vec<UserProcedure>,

    /// Form-level lifecycle event handlers (OnLoad, OnClose).
    /// Uses the same `EventBinding` struct as control events; `control_id` is "".
    pub form_events: Vec<EventBinding>,

    /// Recycle bin: code preserved from deleted controls.
    /// Never emitted into generated .cbl — only stored in .cfrm.
    pub deleted_code: Vec<DeletedControlCode>,

    // ── 007 Form themes ─────────────────────────────────────────────────────────
    /// Per-form theme override (a catalog id, e.g. `"stainless-steel"`).
    /// `None` ⇒ inherit the project default; absent in old `.cfrm` ⇒ `None` ⇒
    /// Liquid Glass, so existing forms render exactly as before.
    pub theme: Option<String>,
    /// Opt into the theme's optional background image. When `false`, the form's
    /// own `background_color` / `background_image` apply (007 R8).
    pub use_theme_background: bool,
    /// Which Liquid Glass recipe to apply to control surfaces.
    pub glass_style: GlassStyle,

    // ── 037 Main form & window lifecycle ────────────────────────────────────
    /// Exactly one form per project holds this (spec 037 R1–R3); enforced by
    /// the IDE (normalisation + reassignment), not by this crate.
    pub main_form: bool,
    /// Taskbar/dock icon image path — only meaningful on the main form (R9).
    pub taskbar_icon: String,
    /// Native minimize / maximize-restore title-bar controls (R12).
    pub can_minimize: bool,
    pub can_maximize: bool,
    /// State the window opens in; runtime-settable thereafter (R13).
    pub window_state: WindowState,
    /// Open in fullscreen; orthogonal to `window_state` (R14).
    pub full_screen: bool,
    /// Show the native title bar; false = chromeless window (R15).
    pub title_visible: bool,

    // ── 049 Application shell ───────────────────────────────────────────────
    /// How this form may be loaded: its own window, the shell's ContentPane, or
    /// either (spec 049 R1). Defaults to `Standalone`.
    pub form_format: FormFormat,
    /// The shell MenuPane's background (049 R39) — only meaningful on the main
    /// form, which owns the shell. `None` = the default chrome fill.
    pub menu_pane_background: Option<MenuPaneBackground>,

    // ── 038 Window effects ──────────────────────────────────────────────────
    /// Play the PROJECT's window entrance/exit effects (spec 038 R3). Forms
    /// never choose effects — only this on/off; false opens/closes instantly.
    pub window_effects: bool,

    // ── Window start position (operator, 2026-07-31) ─────────────────────────
    /// Design-time window coordinates, in screen pixels. Only ever APPLIED
    /// when `start_position` is `Custom` — see [`FormStartPosition`].
    pub x: i32,
    pub y: i32,
    /// Where the window opens. Defaults to `System` (OS/window manager
    /// decides, `x`/`y` unused) so a form that predates this field opens
    /// exactly where it always has.
    pub start_position: FormStartPosition,
}

impl Form {
    pub fn new(name: impl Into<String>, title: impl Into<String>, width: u32, height: u32) -> Self {
        let form_name = name.into();
        // Pre-populate onLoad and onClose with empty stubs so the Code View
        // always shows them even before the user writes anything.
        let form_events = vec![
            EventBinding {
                event: "onLoad".into(),
                paragraph: derive_paragraph_name(&form_name, "onLoad"),
                code: String::new(),
            },
            EventBinding {
                event: "onClose".into(),
                paragraph: derive_paragraph_name(&form_name, "onClose"),
                code: String::new(),
            },
        ];
        let mut form = Self {
            name: form_name,
            title: title.into(),
            width,
            height,
            // OPAQUE, deliberately. A fully transparent form (`00000000`) shows
            // the desktop through it, so the legibility of every control on it
            // depends on whatever window or wallpaper happens to be behind —
            // white default text was readable when the form opened over a dark
            // desktop and invisible when it opened over the IDE, flipping run
            // to run with no code change and looking exactly like a bug in the
            // COBOL. A form's own appearance must not be decided by what is
            // behind it; a developer who wants the glass look sets the alpha
            // themselves. Matches DEFAULT_FOREGROUND_COLOR (white) at AA.
            background_color: DEFAULT_FORM_BACKGROUND_COLOR.to_owned(),
            background_gradient_enabled: false,
            background_gradient_start_color: "#F0F0F0FF".to_owned(),
            background_gradient_end_color: "#C8D0DCFF".to_owned(),
            background_gradient_direction: "South".to_owned(),
            transparency: 0,
            background_image: String::new(),
            bg_image_mode: BgImageMode::Stretch,
            controls: Vec::new(),
            data_bindings: Vec::new(),
            animations: Vec::new(),
            grid_size: 8,
            snap_to_grid: true,
            target: "Custom".to_owned(),
            user_ws_source: String::new(),
            cobol_structure: CobolStructure::default(),
            user_procedures: Vec::new(),
            form_events,
            deleted_code: Vec::new(),
            theme: None,
            use_theme_background: false,
            glass_style: GlassStyle::default(),
            main_form: false,
            taskbar_icon: String::new(),
            can_minimize: true,
            can_maximize: true,
            window_state: WindowState::default(),
            full_screen: false,
            title_visible: true,
            form_format: FormFormat::default(),
            menu_pane_background: None,
            window_effects: true,
            x: 0,
            y: 0,
            start_position: FormStartPosition::default(),
        };
        form.seed_repository_if_empty();
        form
    }

    /// 049 R2 — does this form carry a SideMenu control (anywhere in its
    /// tree)? On the MAIN form this is what puts the application in shell
    /// mode; a `MenuBar` deliberately does NOT count (R3/R45 — an existing
    /// project must never become a shell app by accident).
    pub fn has_side_menu(&self) -> bool {
        fn walk(controls: &[Control]) -> bool {
            controls
                .iter()
                .any(|c| c.control_type == ControlType::SideMenu || walk(&c.children))
        }
        walk(&self.controls)
    }

    /// 049 R2 — the first SideMenu control's id, for mounting its sidecar
    /// menu and routing its events.
    pub fn side_menu_control_id(&self) -> Option<String> {
        fn walk(controls: &[Control]) -> Option<String> {
            for c in controls {
                if c.control_type == ControlType::SideMenu {
                    return Some(c.id.clone());
                }
                if let Some(id) = walk(&c.children) {
                    return Some(id);
                }
            }
            None
        }
        walk(&self.controls)
    }

    /// 049 — pin every `FullHeight` SideMenu to the form's whole vertical
    /// extent. `FullHeight` says the sidebar IS the window's full height, so
    /// its `Y` and `Height` are not the developer's to place: this keeps the
    /// designed geometry telling the truth instead of leaving a 400 px stub
    /// that renders full-height anyway.
    ///
    /// Idempotent, and cheap enough to call every designer frame — that is
    /// what keeps the control following a form resize. A SideMenu whose
    /// `FullHeight` is off keeps whatever the developer placed.
    /// 049 — every SideMenu owns a Panel in its footer pane, and this is what
    /// creates and pins it.
    ///
    /// The Panel is the developer's: they drop controls into it and style it
    /// through the ordinary inspector. What they do NOT own is where it sits —
    /// the sidebar's footer band decides that, so the rect is re-pinned here
    /// every frame rather than being dragged around. Idempotent, and the one
    /// call site is what makes the Panel follow a form resize, a `FooterHeight`
    /// edit and a `Collapsed` toggle without any of those knowing about it.
    ///
    /// A collapsed rail has no footer pane, so the Panel is pinned to zero
    /// height: nothing to see, and nothing that can be dropped into.
    pub fn sync_side_menu_footer_panels(&mut self) {
        let sides: Vec<(String, Rect, i32, bool)> = self
            .controls
            .iter()
            .filter(|c| c.control_type == ControlType::SideMenu)
            .map(|c| {
                (
                    c.id.clone(),
                    c.rect,
                    c.side_menu_footer_height(),
                    c.side_menu_collapsed(),
                )
            })
            .collect();

        for (side_id, side_rect, footer_h, _collapsed) in sides {
            // The footer keeps the developer's height in BOTH rail states, so
            // the Panel does not vanish (and its children with it) the moment
            // the operator collapses the rail.
            let h = footer_h;
            let pinned = Rect::new(side_rect.x, side_rect.y + side_rect.h - h, side_rect.w, h);
            let footer_id = side_menu_footer_id(&side_id);
            if let Some(p) = self.controls.iter_mut().find(|c| c.id == footer_id) {
                p.rect = pinned;
                p.parent = Some(side_id.clone());
                continue;
            }
            let mut p = Control::new(&footer_id, ControlType::Panel, pinned.x, pinned.y);
            p.rect = pinned;
            p.parent = Some(side_id.clone());
            // The marker the designer's locks and the renderers key off, so a
            // footer Panel stays recognisable after a rename or a reload.
            p.set_prop(SIDE_MENU_FOOTER_PROP, true);
            self.controls.push(p);
        }
    }

    pub fn sync_side_menu_full_height(&mut self) {
        let form_h = self.height as i32;
        fn walk(controls: &mut [Control], form_h: i32) {
            for c in controls {
                if c.control_type == ControlType::SideMenu && c.side_menu_full_height() {
                    c.rect.y = 0;
                    c.rect.h = form_h.max(1);
                }
                walk(&mut c.children, form_h);
            }
        }
        walk(&mut self.controls, form_h);
    }

    pub fn apply_neumorphic_defaults(&mut self) {
        self.glass_style = GlassStyle::Neumorphic;
        self.background_color = NEUMORPHIC_FORM_BACKGROUND.into();
        self.background_gradient_enabled = false;
        for ctrl in &mut self.controls {
            ctrl.apply_neumorphic_defaults();
        }
    }

    pub fn apply_neumorphic_dark_defaults(&mut self) {
        self.glass_style = GlassStyle::NeumorphicDark;
        self.background_color = NEUMORPHIC_DARK_FORM_BACKGROUND.into();
        self.background_gradient_enabled = true;
        self.background_gradient_start_color = NEUMORPHIC_DARK_GRADIENT_START.into();
        self.background_gradient_end_color = NEUMORPHIC_DARK_GRADIENT_END.into();
        self.background_gradient_direction = "South".into();
        for ctrl in &mut self.controls {
            ctrl.apply_neumorphic_dark_defaults();
        }
    }

    pub fn apply_glass_style_defaults(&mut self, style: GlassStyle) {
        match style {
            GlassStyle::Neumorphic => self.apply_neumorphic_defaults(),
            GlassStyle::NeumorphicDark => self.apply_neumorphic_dark_defaults(),
            GlassStyle::Classic | GlassStyle::Enhanced => self.glass_style = style,
        }
    }

    /// Fill the `REPOSITORY` block with the curated Rust-FFI type bridge
    /// ([`default_repository`]) **only when it is empty**. A developer who has
    /// written their own entries — even after removing the Rust types — is never
    /// overwritten. Called on form creation and on load.
    pub fn seed_repository_if_empty(&mut self) {
        if self.cobol_structure.repository.trim().is_empty() {
            self.cobol_structure.repository = default_repository();
        }
    }

    pub fn find_control(&self, id: &str) -> Option<&Control> {
        let upper = id.to_ascii_uppercase();
        self.controls.iter().find_map(|c| find_in(c, &upper))
    }

    pub fn find_control_mut(&mut self, id: &str) -> Option<&mut Control> {
        let upper = id.to_ascii_uppercase();
        self.controls
            .iter_mut()
            .find_map(|c| find_in_mut(c, &upper))
    }

    /// The existing data binding whose target is `control_id` (or, for a control
    /// array, the GroupBox hosting `control_id`'s array). Lets the binding editor
    /// re-open an already-configured control's settings for editing instead of
    /// starting blank. `None` when the control has no binding.
    pub fn binding_for_control(&self, control_id: &str) -> Option<&DataBindingDef> {
        let array_id = self
            .find_control(control_id)
            .and_then(|c| c.explicit_control_array_id());
        self.data_bindings.iter().find(|b| match &b.target {
            BindingTargetDescriptor::DataGrid { control_id: c }
            | BindingTargetDescriptor::Chart { control_id: c, .. }
            | BindingTargetDescriptor::ComboBox { control_id: c }
            | BindingTargetDescriptor::ListBox { control_id: c }
            | BindingTargetDescriptor::ScalarControl { control_id: c }
            | BindingTargetDescriptor::MarkerCollection { control_id: c } => {
                c.eq_ignore_ascii_case(control_id)
            }
            BindingTargetDescriptor::ControlArray { array_id: a, .. } => array_id
                .as_deref()
                .is_some_and(|aid| a.eq_ignore_ascii_case(aid)),
        })
    }

    pub fn binding_target_descriptor_for_control(
        &self,
        id: &str,
    ) -> Option<BindingTargetDescriptor> {
        let control = self.find_control(id)?;
        match control.approved_binding_target_kind()? {
            ApprovedBindingTargetKind::ControlArray => {
                let array_id = control.explicit_control_array_id()?;
                let parent_upper = control.id.to_ascii_uppercase();
                let mut member_control_ids: Vec<String> = self
                    .controls
                    .iter()
                    .filter(|candidate| {
                        candidate
                            .parent
                            .as_deref()
                            .map(|parent| parent.eq_ignore_ascii_case(&parent_upper))
                            .unwrap_or(false)
                    })
                    .map(|candidate| candidate.id.clone())
                    .collect();
                if member_control_ids.is_empty() {
                    member_control_ids = control
                        .children
                        .iter()
                        .map(|child| child.id.clone())
                        .collect();
                }
                member_control_ids.sort_by_key(|id| id.to_ascii_uppercase());
                Some(BindingTargetDescriptor::ControlArray {
                    array_id,
                    member_control_ids,
                })
            }
            _ => control.binding_target_descriptor(),
        }
    }

    pub fn array_binding_context_for_member(&self, id: &str) -> Option<(String, String)> {
        let control = self.find_control(id)?;
        let parent_id = control.parent.as_deref()?;
        let parent = self.find_control(parent_id)?;
        let array_id = parent.explicit_control_array_id()?;
        Some((array_id, control.id.clone()))
    }

    pub fn add_control(&mut self, mut ctrl: Control) {
        ctrl.tab_order = self.controls.len() as u32;
        ctrl.z_order = self.controls.len() as i32;
        self.controls.push(ctrl);
    }

    /// Remove a control unconditionally (no code preservation).
    /// Call `remove_control_with_code_check` from the IDE for interactive deletion.
    pub fn remove_control(&mut self, id: &str) {
        let upper = id.to_ascii_uppercase();
        self.controls.retain(|c| c.id.to_ascii_uppercase() != upper);
    }

    /// Check whether a control has any non-empty event code.
    /// Returns the list of (event_name, line_count) pairs that have code.
    pub fn control_has_code(&self, id: &str) -> Vec<(String, usize)> {
        let Some(ctrl) = self.find_control(id) else {
            return Vec::new();
        };
        ctrl.events
            .iter()
            .filter(|ev| ev.has_code())
            .map(|ev| (ev.event.clone(), ev.code_line_count()))
            .collect()
    }

    /// Rename a control's id throughout the form: children `parent` links,
    /// `LabelFor` associations, the control's own event handler ids + paragraph
    /// names, data-binding target/source references, and control references in
    /// handler / procedure code (`Old::…` / `Old(i)…`). Returns `false` (no
    /// change) when `new` is empty, unchanged, invalid, or already taken
    /// (case-insensitive).
    pub fn rename_control(&mut self, old: &str, new: &str) -> bool {
        let new = new.trim();
        if new.is_empty()
            || new.eq_ignore_ascii_case(old)
            || !is_valid_control_id(new)
            || self
                .controls
                .iter()
                .any(|c| c.id.eq_ignore_ascii_case(new) && !c.id.eq_ignore_ascii_case(old))
        {
            return false;
        }
        let new = new.to_owned();

        for ctrl in &mut self.controls {
            // Events live on their control, so the renamed control's handlers are
            // exactly this control's events — re-derive their paragraph names.
            let is_target = ctrl.id.eq_ignore_ascii_case(old);
            if is_target {
                if matches!(ctrl.control_type, ControlType::GroupBox)
                    && ctrl
                        .get_prop("Caption")
                        .map(|v| {
                            let caption = v.as_str();
                            caption.eq_ignore_ascii_case(old)
                                || is_legacy_groupbox_generated_caption(caption)
                        })
                        .unwrap_or(false)
                {
                    ctrl.set_prop("Caption", PropValue::String(String::new()));
                }
                ctrl.id = new.clone();
            }
            if ctrl
                .parent
                .as_deref()
                .map(|p| p.eq_ignore_ascii_case(old))
                .unwrap_or(false)
            {
                ctrl.parent = Some(new.clone());
            }
            if ctrl
                .get_prop("LabelFor")
                .map(|v| v.as_str().eq_ignore_ascii_case(old))
                .unwrap_or(false)
            {
                ctrl.set_prop("LabelFor", PropValue::String(new.clone()));
            }
            for ev in &mut ctrl.events {
                if is_target {
                    ev.paragraph = derive_paragraph_name(&new, &ev.event);
                }
                rename_control_refs_in_code(&mut ev.code, old, &new);
            }
        }
        for ev in &mut self.form_events {
            rename_control_refs_in_code(&mut ev.code, old, &new);
        }
        for up in &mut self.user_procedures {
            rename_control_refs_in_code(&mut up.code, old, &new);
        }
        for binding in &mut self.data_bindings {
            rename_binding_control_refs(binding, old, &new);
        }
        true
    }

    /// Move a control's event code to the recycle bin, then remove the control.
    /// Called when the user chooses "Preserve in Recycle" in the deletion dialog.
    pub fn recycle_control(&mut self, id: &str, deleted_at: impl Into<String>) {
        let upper = id.to_ascii_uppercase();
        if let Some(ctrl) = self.find_control(&upper) {
            let events_with_code: Vec<EventBinding> = ctrl
                .events
                .iter()
                .filter(|ev| ev.has_code())
                .cloned()
                .collect();
            if !events_with_code.is_empty() {
                self.deleted_code.push(DeletedControlCode {
                    control_id: ctrl.id.clone(),
                    deleted_at: deleted_at.into(),
                    events: events_with_code,
                });
            }
        }
        self.controls.retain(|c| c.id.to_ascii_uppercase() != upper);
        // A deleted control must not leave dangling data-binding info behind:
        // drop any binding whose target control (or its host GroupBox for a
        // control array) or control-based source no longer exists, and drop any
        // per-field mapping pointing at a now-deleted member/column control.
        self.prune_orphaned_data_bindings();
    }

    /// Indices of the user procedures left orphaned by control deletions —
    /// the ones nothing can reach and that can no longer compile.
    ///
    /// A procedure is orphaned when all three hold:
    /// 1. it addresses at least one control (`receiver::member`) — a procedure
    ///    of pure COBOL depends on no control and is never orphaned;
    /// 2. NONE of the controls it addresses still exists;
    /// 3. no surviving handler, form event or other procedure names it, so
    ///    nothing calls it.
    ///
    /// All three are required because this decides the removal of code the
    /// developer may have written: a procedure that still has one live control
    /// or one live caller is a procedure with a defect, not an orphan, and the
    /// developer must be the one to resolve it.
    ///
    /// Run to a fixpoint: removing a procedure can leave the only procedure it
    /// called with no caller either. Indices are ascending and refer to the
    /// list as it stands, so remove them from the highest down.
    ///
    /// Observed live: an agent created `UPDATE-CONCATENATION` over fifteen
    /// TextBoxes; the developer deleted every control, and the procedure —
    /// which is form-level, not control-level — stayed behind. Its body ended
    /// with its own `GOBACK.`, so the form could no longer be launched at all
    /// ("paragraph 'GOBACK' is declared more than once"), and nothing on
    /// screen explained why (operator, 2026-07-31).
    pub fn orphaned_user_procedures(&self) -> Vec<usize> {
        // Every control, at every depth. `controls` is a **tree**: a Button
        // inside a GroupBox lives in that GroupBox's `children`, so a scan of
        // the top level alone misses most of a real form — and then a procedure
        // addressing a nested control looks like one whose controls were all
        // deleted. Since a newly created procedure has no caller yet either, it
        // was removed the first time the developer pressed Save.
        let mut live: std::collections::HashSet<String> = std::collections::HashSet::new();
        for ctrl in &self.controls {
            collect_control_ids(ctrl, &mut live);
        }
        let mut doomed: std::collections::HashSet<usize> = std::collections::HashSet::new();
        loop {
            let mut grew = false;
            for (i, proc) in self.user_procedures.iter().enumerate() {
                if doomed.contains(&i) {
                    continue;
                }
                let refs = control_refs_in_code(&proc.code);
                if refs.is_empty() || refs.iter().any(|r| live.contains(r)) {
                    continue;
                }
                // Handlers at every depth too: the button that calls a common
                // procedure is usually inside a GroupBox or a panel, and reading
                // only top-level handlers made such a procedure look uncalled.
                let called = self
                    .controls
                    .iter()
                    .flat_map(control_events)
                    .chain(self.form_events.iter())
                    .any(|ev| code_mentions_word(&ev.code, &proc.name))
                    || self
                        .user_procedures
                        .iter()
                        .enumerate()
                        .any(|(j, other)| {
                            j != i
                                && !doomed.contains(&j)
                                && code_mentions_word(&other.code, &proc.name)
                        });
                if !called {
                    doomed.insert(i);
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
        let mut out: Vec<usize> = doomed.into_iter().collect();
        out.sort_unstable();
        out
    }

    /// Remove data bindings orphaned by a control deletion. A whole binding is
    /// dropped when its **target** control is gone (for a control array, its host
    /// GroupBox — resolved by array id), or when its control-based **source**
    /// (SQL / REST / Agent host control) is gone. Surviving bindings additionally
    /// lose any field mapping that points at a deleted member/column control, so
    /// a repeating-group or grid binding never keeps a dangling mapping after one
    /// of its member controls is removed. Idempotent — safe to call after every
    /// single-control delete in a cascade, and safe to call before Run/Save to
    /// self-heal a form whose orphan was created before delete-time pruning
    /// existed (the guardian would otherwise block on `missing-target-control`).
    ///
    /// Returns the number of orphaned items removed (whole bindings + dangling
    /// mappings on survivors); `0` means nothing changed.
    pub fn prune_orphaned_data_bindings(&mut self) -> usize {
        use std::collections::HashSet;
        // Every control at every depth — `controls` is a tree. A DataGrid or a
        // ComboBox inside a GroupBox is the normal case, not the exotic one, and
        // reading only the top level made every binding onto one look like a
        // binding whose target had been deleted.
        let mut ctrl_ids: HashSet<String> = HashSet::new();
        for ctrl in &self.controls {
            collect_control_ids(ctrl, &mut ctrl_ids);
        }
        let mut array_ids: HashSet<String> = HashSet::new();
        for ctrl in &self.controls {
            collect_control_array_ids(ctrl, &mut array_ids);
        }
        let has_ctrl = |id: &str| ctrl_ids.contains(&id.to_ascii_uppercase());
        let has_array = |id: &str| array_ids.contains(&id.to_ascii_uppercase());

        // 1. Drop whole bindings whose target (or control-based source) is gone.
        let before_bindings = self.data_bindings.len();
        self.data_bindings.retain(|binding| {
            let target_ok = match &binding.target {
                BindingTargetDescriptor::DataGrid { control_id }
                | BindingTargetDescriptor::Chart { control_id, .. }
                | BindingTargetDescriptor::ComboBox { control_id }
                | BindingTargetDescriptor::ListBox { control_id }
                | BindingTargetDescriptor::ScalarControl { control_id }
                | BindingTargetDescriptor::MarkerCollection { control_id } => has_ctrl(control_id),
                BindingTargetDescriptor::ControlArray { array_id, .. } => has_array(array_id),
            };
            let source_ok = match &binding.source {
                BindingSourceDescriptor::Sql {
                    source_control_id, ..
                }
                | BindingSourceDescriptor::RestApi {
                    source_control_id, ..
                }
                | BindingSourceDescriptor::AgentAi {
                    source_control_id, ..
                } => source_control_id.trim().is_empty() || has_ctrl(source_control_id),
                _ => true,
            };
            target_ok && source_ok
        });
        let mut removed = before_bindings - self.data_bindings.len();

        // 2. On surviving bindings, drop dangling per-field mappings (a
        //    member/column control deleted while its host grid/array survives).
        for binding in &mut self.data_bindings {
            let before = binding.mappings.len();
            binding.mappings.retain(|m| match &m.target {
                BindingTargetPath::GridColumn { control_id, .. }
                | BindingTargetPath::ChartCategory { control_id }
                | BindingTargetPath::ChartValueSeries { control_id, .. }
                | BindingTargetPath::ChartSeriesLabel { control_id, .. }
                | BindingTargetPath::ListDisplayItem { control_id }
                | BindingTargetPath::ListValue { control_id }
                | BindingTargetPath::ScalarValue { control_id }
                | BindingTargetPath::MarkerField { control_id, .. }
                | BindingTargetPath::ControlProperty { control_id, .. } => has_ctrl(control_id),
            });
            removed += before - binding.mappings.len();
        }

        removed
    }

    /// Restore a recycled control's code entries back into an existing control
    /// (e.g. if the user re-added the control and wants its old code back).
    pub fn restore_from_recycle(&mut self, deleted_at: &str, target_control_id: &str) {
        let Some(pos) = self
            .deleted_code
            .iter()
            .position(|d| d.deleted_at == deleted_at)
        else {
            return;
        };
        let recycled = self.deleted_code.remove(pos);
        let upper = target_control_id.to_ascii_uppercase();
        if let Some(ctrl) = self.find_control_mut(&upper) {
            for recycled_ev in recycled.events {
                if let Some(existing) = ctrl
                    .events
                    .iter_mut()
                    .find(|e| e.event == recycled_ev.event)
                {
                    existing.code = recycled_ev.code;
                } else {
                    ctrl.events.push(recycled_ev);
                }
            }
        }
    }

    /// Return controls sorted by z_order ascending (for rendering back-to-front).
    pub fn controls_by_z(&self) -> Vec<&Control> {
        let mut v: Vec<&Control> = self.controls.iter().collect();
        v.sort_by_key(|c| c.z_order);
        v
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn find_in<'a>(ctrl: &'a Control, id: &str) -> Option<&'a Control> {
    if ctrl.id.to_ascii_uppercase() == id {
        return Some(ctrl);
    }
    ctrl.children.iter().find_map(|c| find_in(c, id))
}

/// Collect `ctrl`'s id and every descendant's, upper-cased.
fn collect_control_ids(ctrl: &Control, out: &mut std::collections::HashSet<String>) {
    out.insert(ctrl.id.to_ascii_uppercase());
    for child in &ctrl.children {
        collect_control_ids(child, out);
    }
}

/// Collect the explicit control-array id of `ctrl` and every descendant.
fn collect_control_array_ids(ctrl: &Control, out: &mut std::collections::HashSet<String>) {
    if let Some(id) = ctrl.explicit_control_array_id() {
        out.insert(id.to_ascii_uppercase());
    }
    for child in &ctrl.children {
        collect_control_array_ids(child, out);
    }
}

/// Every event binding on `ctrl` and its descendants.
///
/// A form is a tree, so "the handlers of this form" is never
/// `controls.iter().flat_map(|c| c.events.iter())` — that reads the top level
/// and stops.
fn control_events(ctrl: &Control) -> Vec<&EventBinding> {
    let mut out: Vec<&EventBinding> = ctrl.events.iter().collect();
    for child in &ctrl.children {
        out.extend(control_events(child));
    }
    out
}

fn find_in_mut<'a>(ctrl: &'a mut Control, id: &str) -> Option<&'a mut Control> {
    if ctrl.id.to_ascii_uppercase() == id {
        return Some(ctrl);
    }
    ctrl.children.iter_mut().find_map(|c| find_in_mut(c, id))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod orphan_procedure_tests {
    use super::*;

    fn proc(name: &str, code: &str) -> UserProcedure {
        UserProcedure {
            name: name.into(),
            code: code.into(),
        }
    }

    fn form_with(controls: &[&str], procs: Vec<UserProcedure>) -> Form {
        let mut form = Form::new("F", "T", 400, 300);
        for (i, id) in controls.iter().enumerate() {
            form.controls
                .push(Control::new(*id, ControlType::TextBox, 0, i as i32 * 30));
        }
        form.user_procedures = procs;
        form
    }

    /// The shape that broke PowerDemo3: a procedure over fifteen TextBoxes,
    /// every one of them deleted. It is form-level, so deleting the controls
    /// left it behind, invisible on the canvas — and its body was enough to
    /// stop the form from launching at all.
    #[test]
    fn a_procedure_whose_every_control_is_gone_is_orphaned() {
        let body = "       PROCEDURE DIVISION.\n           STRING txt1::Text DELIMITED BY SIZE \
                    txt2::Text DELIMITED BY SIZE INTO WS-X\n           MOVE WS-X TO \
                    lblConcat::Caption.";
        let form = form_with(&[], vec![proc("UPDATE-CONCATENATION", body)]);
        assert_eq!(form.orphaned_user_procedures(), vec![0]);
        // One surviving control it addresses is enough to keep it: that is a
        // defect for the developer to resolve, not an orphan to remove.
        let form = form_with(&["TXT2"], vec![proc("UPDATE-CONCATENATION", body)]);
        assert!(form.orphaned_user_procedures().is_empty());
    }

    /// The same blind spot, in the code that deletes **data bindings**: it runs
    /// on the same Save, and a DataGrid inside a GroupBox is the normal case.
    /// Losing a binding costs the developer a whole configuration dialog's
    /// worth of work, silently.
    #[test]
    fn a_binding_onto_a_nested_control_is_not_pruned() {
        let mut form = Form::new("F", "T", 400, 300);
        let mut group = Control::new("PANEL-1", ControlType::GroupBox, 0, 0);
        group
            .children
            .push(Control::new("GRID-1", ControlType::DataGrid, 10, 10));
        form.controls.push(group);
        form.data_bindings.push(DataBindingDef::new(
            "b1",
            "Customers",
            BindingSourceDescriptor::CobolTable {
                table_name: "WS-CUSTOMERS".into(),
                occurs_item: "WS-CUST".into(),
                fields: Vec::new(),
                key_fields: Vec::new(),
                writable: false,
            },
            BindingTargetDescriptor::DataGrid {
                control_id: "GRID-1".into(),
            },
        ));

        let removed = form.prune_orphaned_data_bindings();
        assert_eq!(
            removed, 0,
            "a binding onto a control inside a container was pruned as orphaned"
        );
        assert_eq!(form.data_bindings.len(), 1);
    }

    /// **A control inside a container is still a control.** `Form::controls` is
    /// a tree — a Button inside a GroupBox lives in that GroupBox's `children`,
    /// not at the top level — so a scan that only walks the top level cannot see
    /// most of a real form's controls.
    ///
    /// The cost is not cosmetic: a procedure that addresses a nested control
    /// looks like one whose every control was deleted, and a brand-new procedure
    /// is never called by anything yet, so it is removed the first time the
    /// developer presses Save.
    #[test]
    fn a_procedure_addressing_a_nested_control_is_not_orphaned() {
        let mut form = Form::new("F", "T", 400, 300);
        let mut group = Control::new("PANEL-1", ControlType::GroupBox, 0, 0);
        group
            .children
            .push(Control::new("TXT1", ControlType::TextBox, 10, 10));
        form.controls.push(group);
        form.user_procedures = vec![proc(
            "UPDATE-TOTAL",
            "       PROCEDURE DIVISION.\n           MOVE txt1::Text TO WS-X.",
        )];

        assert!(
            form.orphaned_user_procedures().is_empty(),
            "a procedure addressing a control inside a container was treated as \
             an orphan — its control exists, it is just not at the top level"
        );
    }

    /// The same blind spot from the other direction: the handler that calls a
    /// procedure usually belongs to a control inside a container.
    #[test]
    fn a_procedure_called_from_a_nested_controls_handler_is_not_orphaned() {
        let mut form = Form::new("F", "T", 400, 300);
        let mut group = Control::new("PANEL-1", ControlType::GroupBox, 0, 0);
        let mut button = Control::new("SAVE-BTN", ControlType::Button, 10, 10);
        button.events.push(EventBinding {
            event: "onClick".into(),
            paragraph: "SAVE-BTN--onClick".into(),
            code: "       PROCEDURE DIVISION.\n           CALL \"UPDATE-TOTAL\".".into(),
        });
        group.children.push(button);
        form.controls.push(group);
        // Its own controls are all gone, so only the CALL can save it.
        form.user_procedures = vec![proc(
            "UPDATE-TOTAL",
            "       PROCEDURE DIVISION.\n           MOVE txtGone::Text TO WS-X.",
        )];

        assert!(
            form.orphaned_user_procedures().is_empty(),
            "a procedure called from a nested control's handler was treated as \
             uncalled — the CALL is there, just not on a top-level control"
        );
    }

    /// Deleting code is not a cleanup when someone still calls it.
    #[test]
    fn a_procedure_something_still_calls_is_never_removed() {
        let body = "       PROCEDURE DIVISION.\n           MOVE txt1::Text TO WS-X.";
        let caller = "       PROCEDURE DIVISION.\n           PERFORM UPDATE-CONCATENATION.";

        // Called from a surviving control's handler.
        let mut form = form_with(&["BTN"], vec![proc("UPDATE-CONCATENATION", body)]);
        form.controls[0].events.push(EventBinding {
            event: "onClick".into(),
            paragraph: "BTN--ONCLICK".into(),
            code: caller.into(),
        });
        assert!(form.orphaned_user_procedures().is_empty());

        // Called from a form event.
        let mut form = form_with(&[], vec![proc("UPDATE-CONCATENATION", body)]);
        form.form_events.push(EventBinding {
            event: "onLoad".into(),
            paragraph: "F--ONLOAD".into(),
            code: caller.into(),
        });
        assert!(form.orphaned_user_procedures().is_empty());

        // Called from another procedure that is itself still reachable.
        let mut form = form_with(&["TXT9"], vec![proc("UPDATE-CONCATENATION", body)]);
        form.user_procedures
            .push(proc("RECALC", "       PROCEDURE DIVISION.\n           PERFORM UPDATE-CONCATENATION.\n           MOVE txt9::Text TO WS-Y."));
        assert!(form.orphaned_user_procedures().is_empty());
    }

    /// A procedure of pure COBOL depends on no control, so no deletion can
    /// orphan it — however many controls disappear around it.
    #[test]
    fn a_procedure_that_addresses_no_control_is_left_alone() {
        let form = form_with(
            &[],
            vec![proc(
                "ROUND-TOTAL",
                "       PROCEDURE DIVISION.\n           COMPUTE WS-T = WS-A + WS-B.",
            )],
        );
        assert!(form.orphaned_user_procedures().is_empty());
    }

    /// Removing an orphan can strand the only procedure it called: the sweep
    /// runs to a fixpoint, so one deletion does not need a second sweep.
    #[test]
    fn an_orphan_takes_what_only_it_called_with_it() {
        let form = form_with(
            &[],
            vec![
                proc(
                    "TOP",
                    "       PROCEDURE DIVISION.\n           MOVE txt1::Text TO WS-X.\n           PERFORM HELPER.",
                ),
                proc(
                    "HELPER",
                    "       PROCEDURE DIVISION.\n           MOVE txt2::Text TO WS-Y.",
                ),
            ],
        );
        assert_eq!(form.orphaned_user_procedures(), vec![0, 1]);
    }

    #[test]
    fn control_references_are_read_from_the_member_operator() {
        let refs = control_refs_in_code(
            "MOVE txt1::Text TO WS-X\nSET Save-Button::Enabled TO 0\nMOVE WS-X TO WS-Y",
        );
        assert_eq!(refs, vec!["TXT1", "SAVE-BUTTON"]);
        // A bare word is any COBOL identifier, not a control.
        assert!(control_refs_in_code("PERFORM UPDATE-CONCATENATION.").is_empty());
    }
}

#[cfg(test)]
mod start_position_tests {
    use super::*;

    #[test]
    fn round_trips_every_variant_through_as_str_and_from_str() {
        for pos in FormStartPosition::ALL {
            assert_eq!(FormStartPosition::from_str(pos.as_str()), pos);
        }
        // Anything unrecognised — including an empty/absent attribute — is
        // System, the one variant a form that never opted in already has.
        assert_eq!(FormStartPosition::from_str(""), FormStartPosition::System);
        assert_eq!(FormStartPosition::from_str("bogus"), FormStartPosition::System);
        assert_eq!(FormStartPosition::default(), FormStartPosition::System);
    }

    #[test]
    fn only_the_nine_screen_relative_variants_compute_a_position() {
        for pos in [FormStartPosition::System, FormStartPosition::Custom] {
            assert!(
                resolved_start_position(pos, (1920.0, 1080.0), (800.0, 600.0)).is_none(),
                "{pos:?} must not compute a screen position — the caller \
                 already has its own answer (OS default, or Form::x/y)"
            );
        }
        let relative = [
            FormStartPosition::TopLeft,
            FormStartPosition::TopCenter,
            FormStartPosition::TopRight,
            FormStartPosition::MiddleLeft,
            FormStartPosition::Center,
            FormStartPosition::MiddleRight,
            FormStartPosition::BottomLeft,
            FormStartPosition::BottomCenter,
            FormStartPosition::BottomRight,
        ];
        assert_eq!(relative.len(), 9, "8 directions plus Screen Center");
        for pos in relative {
            assert!(resolved_start_position(pos, (1920.0, 1080.0), (800.0, 600.0)).is_some());
        }
    }

    /// The nine positions on a concrete screen/window pair — the numbers a
    /// developer would actually see, not just "some value came back".
    #[test]
    fn computes_the_expected_corner_edge_and_center_coordinates() {
        let screen = (1920.0, 1080.0);
        let window = (800.0, 600.0);
        let at = |pos| resolved_start_position(pos, screen, window).unwrap();

        assert_eq!(at(FormStartPosition::TopLeft), (0.0, 0.0));
        assert_eq!(at(FormStartPosition::TopRight), (1120.0, 0.0));
        assert_eq!(at(FormStartPosition::BottomLeft), (0.0, 480.0));
        assert_eq!(at(FormStartPosition::BottomRight), (1120.0, 480.0));
        assert_eq!(at(FormStartPosition::Center), (560.0, 240.0));
        assert_eq!(at(FormStartPosition::TopCenter), (560.0, 0.0));
        assert_eq!(at(FormStartPosition::BottomCenter), (560.0, 480.0));
        assert_eq!(at(FormStartPosition::MiddleLeft), (0.0, 240.0));
        assert_eq!(at(FormStartPosition::MiddleRight), (1120.0, 240.0));
    }

    /// A window bigger than the screen (an ultra-wide design opened on a
    /// laptop) must not compute a negative, off-screen origin — it pins to
    /// the near edge instead.
    #[test]
    fn a_window_larger_than_the_screen_clamps_to_the_near_edge() {
        let got = resolved_start_position(
            FormStartPosition::BottomRight,
            (800.0, 600.0),
            (1200.0, 900.0),
        )
        .unwrap();
        assert_eq!(got, (0.0, 0.0), "clamped, never negative: {got:?}");
        let center = resolved_start_position(
            FormStartPosition::Center,
            (800.0, 600.0),
            (1200.0, 900.0),
        )
        .unwrap();
        assert_eq!(center, (0.0, 0.0));
    }

    /// A brand-new form and one loaded from a `.cfrm` that never named the
    /// feature must be indistinguishable: `System`, `(0, 0)`.
    #[test]
    fn a_fresh_form_defaults_to_system_at_the_origin() {
        let form = Form::new("F", "T", 400, 300);
        assert_eq!(form.start_position, FormStartPosition::System);
        assert_eq!((form.x, form.y), (0, 0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Operator, 2026-07-28: switching a form from Neumorphic Dark to Light
    /// left labels with the dark style's white foreground on the light surface
    /// — no contrast. The light applier remaps the dark default to black but
    /// leaves a developer-chosen colour alone.
    #[test]
    fn dark_to_light_style_switch_keeps_label_contrast() {
        let mut form = Form::new("F", "T", 400, 300);
        form.controls
            .push(Control::new("L1", ControlType::Label, 0, 0));
        form.controls
            .push(Control::new("L2", ControlType::Label, 0, 40));

        form.apply_glass_style_defaults(GlassStyle::NeumorphicDark);
        assert_eq!(
            form.find_control("L1")
                .unwrap()
                .get_prop("ForegroundColor")
                .unwrap()
                .as_str(),
            "#FFFFFFFF",
            "dark forces white on every control"
        );
        // The developer picks their own colour on L2 after the dark switch.
        form.find_control_mut("L2")
            .unwrap()
            .set_prop("ForegroundColor", PropValue::String("#7A1F1F".into()));

        form.apply_glass_style_defaults(GlassStyle::Neumorphic);
        assert_eq!(
            form.find_control("L1")
                .unwrap()
                .get_prop("ForegroundColor")
                .unwrap()
                .as_str(),
            "#000000",
            "the dark default remaps to black on the light surface"
        );
        assert_eq!(
            form.find_control("L2")
                .unwrap()
                .get_prop("ForegroundColor")
                .unwrap()
                .as_str(),
            "#7A1F1F",
            "a developer-chosen foreground survives the switch"
        );
    }

    #[test]
    fn rename_control_updates_all_references() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        let mut label = Control::new("Label-1", ControlType::Label, 0, 0);
        // A handler on the control that also references it in code.
        let mut ev = EventBinding::for_control("Label-1", "onClick");
        ev.code = "       PROCEDURE DIVISION.\n           MOVE 1 TO Label-1::Caption.".to_owned();
        label.events.push(ev);
        let mut child = Control::new("Inner", ControlType::TextBox, 5, 5);
        child.parent = Some("Label-1".into());
        let mut assoc = Control::new("Assoc", ControlType::Label, 0, 40);
        assoc.set_prop("LabelFor", PropValue::String("Label-1".into()));
        let other = Control::new("Other", ControlType::Button, 0, 80);
        form.controls = vec![label, child, assoc, other];

        // Reject a taken name and an invalid name; accept a fresh one.
        assert!(
            !form.rename_control("Label-1", "Other"),
            "taken name rejected"
        );
        assert!(
            !form.rename_control("Label-1", "1bad"),
            "invalid name rejected"
        );
        assert!(form.rename_control("Label-1", "NameLabel"));

        assert!(form.find_control("NameLabel").is_some());
        assert!(form.find_control("Label-1").is_none());
        // Child parent, LabelFor, event paragraph, and code ref all updated.
        assert_eq!(
            form.find_control("Inner").unwrap().parent.as_deref(),
            Some("NameLabel")
        );
        assert_eq!(
            form.find_control("Assoc")
                .unwrap()
                .get_prop("LabelFor")
                .unwrap()
                .as_str(),
            "NameLabel"
        );
        let lbl = form.find_control("NameLabel").unwrap();
        assert_eq!(lbl.events[0].paragraph, "NAMELABEL--ONCLICK");
        assert!(lbl.events[0].code.contains("NameLabel::Caption"));
        assert!(!lbl.events[0].code.contains("Label-1::"));
    }

    #[test]
    fn new_controls_do_not_expose_label_for_property() {
        for control_type in [
            ControlType::Label,
            ControlType::TextBox,
            ControlType::Button,
            ControlType::GroupBox,
            ControlType::Panel,
            ControlType::DataGrid,
            ControlType::BarChart,
        ] {
            let type_name = format!("{control_type:?}");
            let ctrl = Control::new("C", control_type, 0, 0);
            assert!(
                ctrl.get_prop("LabelFor").is_none(),
                "{type_name} should not expose LabelFor by default"
            );
        }
    }

    fn sample_fields() -> Vec<BindingField> {
        vec![
            BindingField::new("CUSTOMER-ID", BindingDataType::Integer).key(),
            BindingField::new("CUSTOMER-NAME", BindingDataType::Text).required(),
        ]
    }

    #[test]
    fn data_binding_model_covers_all_source_variants() {
        let fields = sample_fields();
        let sources = vec![
            BindingSourceDescriptor::IndexedFile {
                definition_path: "data/customers.cidx".into(),
                record_name: "CUSTOMER-REC".into(),
                fields: fields.clone(),
                key_field: Some("CUSTOMER-ID".into()),
                writable: true,
            },
            BindingSourceDescriptor::Sql {
                source_control_id: "SQL-1".into(),
                query_name: "CUSTOMERS".into(),
                result_set_name: "CUSTOMERS-RS".into(),
                fields: fields.clone(),
                key_fields: vec!["CUSTOMER-ID".into()],
                writable: true,
            },
            BindingSourceDescriptor::CobolTable {
                table_name: "CUSTOMER-TABLE".into(),
                occurs_item: "CUSTOMER-ROW".into(),
                fields: fields.clone(),
                key_fields: vec!["CUSTOMER-ID".into()],
                writable: true,
            },
            BindingSourceDescriptor::RestApi {
                source_control_id: "REST-1".into(),
                endpoint_name: "GET-CUSTOMERS".into(),
                response_data_item: "REST-RESPONSE".into(),
                fields: fields.clone(),
                update: None,
            },
            BindingSourceDescriptor::AgentAi {
                source_control_id: "AGENT-1".into(),
                output_name: "CUSTOMERS".into(),
                fields: fields.clone(),
                update: None,
            },
        ];

        let kinds: Vec<BindingSourceKind> =
            sources.iter().map(BindingSourceDescriptor::kind).collect();
        assert_eq!(
            kinds,
            vec![
                BindingSourceKind::IndexedFile,
                BindingSourceKind::Sql,
                BindingSourceKind::CobolTable,
                BindingSourceKind::RestApi,
                BindingSourceKind::AgentAi,
            ]
        );

        for source in sources {
            let binding = DataBindingDef::new(
                format!("{:?}", source.kind()),
                "Customers",
                source,
                BindingTargetDescriptor::DataGrid {
                    control_id: "GRID-1".into(),
                },
            );
            assert_eq!(binding.schema_version, DATA_BINDING_SCHEMA_VERSION);
            assert_eq!(binding.mode, BindingMode::ReadOnly);
            assert_eq!(binding.saved_source_metadata.fields.len(), 2);
            assert_eq!(
                binding.validation.validated_with_schema_version,
                DATA_BINDING_SCHEMA_VERSION
            );
        }
    }

    #[test]
    fn data_binding_model_covers_all_target_variants() {
        let source = BindingSourceDescriptor::CobolTable {
            table_name: "SALES-TABLE".into(),
            occurs_item: "SALES-ROW".into(),
            fields: sample_fields(),
            key_fields: vec!["CUSTOMER-ID".into()],
            writable: true,
        };
        let targets = vec![
            BindingTargetDescriptor::DataGrid {
                control_id: "GRID-1".into(),
            },
            BindingTargetDescriptor::Chart {
                control_id: "CHART-1".into(),
                chart_kind: BindingChartKind::Bar,
            },
            BindingTargetDescriptor::ComboBox {
                control_id: "COMBO-1".into(),
            },
            BindingTargetDescriptor::ListBox {
                control_id: "LIST-1".into(),
            },
            BindingTargetDescriptor::ControlArray {
                array_id: "CUSTOMER-ROWS".into(),
                member_control_ids: vec!["CUSTOMER-NAME".into(), "CUSTOMER-ID".into()],
            },
        ];

        let ids: Vec<String> = targets
            .into_iter()
            .map(|target| {
                DataBindingDef::new("BINDING-1", "Sales", source.clone(), target)
                    .target
                    .primary_control_id()
                    .to_string()
            })
            .collect();

        assert_eq!(
            ids,
            vec!["GRID-1", "CHART-1", "COMBO-1", "LIST-1", "CUSTOMER-ROWS"]
        );
    }

    #[test]
    fn data_binding_model_sorts_mappings_deterministically() {
        let source = BindingSourceDescriptor::IndexedFile {
            definition_path: "data/customers.cidx".into(),
            record_name: "CUSTOMER-REC".into(),
            fields: sample_fields(),
            key_field: Some("CUSTOMER-ID".into()),
            writable: true,
        };
        let binding = DataBindingDef::new(
            "CUSTOMER-BINDING",
            "Customers",
            source,
            BindingTargetDescriptor::DataGrid {
                control_id: "GRID-1".into(),
            },
        )
        .with_mappings(vec![
            FieldMapping::new(
                "CUSTOMER-NAME",
                BindingTargetPath::GridColumn {
                    control_id: "GRID-1".into(),
                    column_id: "NAME".into(),
                },
            ),
            FieldMapping::new(
                "CUSTOMER-ID",
                BindingTargetPath::GridColumn {
                    control_id: "GRID-1".into(),
                    column_id: "ID".into(),
                },
            ),
        ]);

        let sorted: Vec<&str> = binding
            .sorted_mapping_refs()
            .iter()
            .map(|mapping| mapping.source_field.as_str())
            .collect();
        assert_eq!(sorted, vec!["CUSTOMER-ID", "CUSTOMER-NAME"]);

        let mut form = Form::new("MAIN-FORM", "Main", 800, 600);
        form.data_bindings.push(binding);
        assert_eq!(form.data_bindings.len(), 1);
    }

    #[test]
    fn data_binding_targets_accept_only_approved_control_types() {
        assert_eq!(
            ControlType::DataGrid.approved_binding_target_kind(),
            Some(ApprovedBindingTargetKind::DataGrid)
        );
        assert_eq!(
            ControlType::ComboBox.approved_binding_target_kind(),
            Some(ApprovedBindingTargetKind::ComboBox)
        );
        assert_eq!(
            ControlType::ListBox.approved_binding_target_kind(),
            Some(ApprovedBindingTargetKind::ListBox)
        );

        for (control_type, chart_kind) in [
            (ControlType::BarChart, BindingChartKind::Bar),
            (ControlType::LineChart, BindingChartKind::Line),
            (ControlType::PieChart, BindingChartKind::Pie),
            (ControlType::AreaChart, BindingChartKind::Area),
            (ControlType::ScatterChart, BindingChartKind::Scatter),
            (ControlType::DonutChart, BindingChartKind::Donut),
        ] {
            assert_eq!(
                control_type.approved_binding_target_kind(),
                Some(ApprovedBindingTargetKind::Chart(chart_kind))
            );
        }

        for control_type in [
            ControlType::TextBox,
            ControlType::Label,
            ControlType::Button,
            ControlType::Slider,
            ControlType::Panel,
            ControlType::RestClient,
            ControlType::AgentObject,
            ControlType::SqlDatabase,
        ] {
            assert_eq!(control_type.approved_binding_target_kind(), None);
        }
    }

    #[test]
    fn data_binding_targets_accept_explicit_arrays() {
        let mut group = Control::new("CUSTOMER-ROWS", ControlType::GroupBox, 0, 0);
        assert_eq!(group.approved_binding_target_kind(), None);

        group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        assert_eq!(
            group.approved_binding_target_kind(),
            Some(ApprovedBindingTargetKind::ControlArray)
        );
        assert_eq!(
            group.explicit_control_array_id().as_deref(),
            Some("CUSTOMER-ROWS")
        );

        group.set_prop("ArrayName", PropValue::String("CUSTOMERS".into()));
        assert_eq!(
            group.explicit_control_array_id().as_deref(),
            Some("CUSTOMERS")
        );
        assert_eq!(
            group.binding_target_descriptor(),
            Some(BindingTargetDescriptor::ControlArray {
                array_id: "CUSTOMERS".into(),
                member_control_ids: Vec::new(),
            })
        );
    }

    #[test]
    fn data_binding_targets_resolve_array_member_context_without_scalar_target() {
        let mut form = Form::new("MAIN-FORM", "Main", 800, 600);
        let mut group = Control::new("CUSTOMER-ROWS", ControlType::GroupBox, 0, 0);
        group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        group.set_prop("ArrayName", PropValue::String("CUSTOMERS".into()));

        let mut text = Control::new("CUSTOMER-NAME", ControlType::TextBox, 10, 10);
        text.parent = Some("CUSTOMER-ROWS".into());
        let mut combo = Control::new("CUSTOMER-STATUS", ControlType::ComboBox, 10, 40);
        combo.parent = Some("CUSTOMER-ROWS".into());

        form.add_control(group);
        form.add_control(combo);
        form.add_control(text);

        assert_eq!(
            form.binding_target_descriptor_for_control("CUSTOMER-ROWS"),
            Some(BindingTargetDescriptor::ControlArray {
                array_id: "CUSTOMERS".into(),
                member_control_ids: vec!["CUSTOMER-NAME".into(), "CUSTOMER-STATUS".into()],
            })
        );
        assert_eq!(
            form.array_binding_context_for_member("customer-name"),
            Some(("CUSTOMERS".into(), "CUSTOMER-NAME".into()))
        );
        assert_eq!(
            form.find_control("CUSTOMER-NAME")
                .unwrap()
                .approved_binding_target_kind(),
            None
        );
    }

    #[test]
    fn property_names_for_reflects_control_model() {
        // Type-specific properties are surfaced, not just the universal ones.
        let lb = property_names_for("ListBox");
        assert!(lb.contains(&"Items".to_string()));
        assert!(lb.contains(&"SelectedIndex".to_string()));
        assert!(lb.contains(&"Visible".to_string())); // universal

        assert!(property_names_for("Timer").contains(&"Interval".to_string()));
        assert!(property_names_for("ProgressBar").contains(&"BarColor".to_string()));
        assert!(property_names_for("TreeView").contains(&"ShowLines".to_string()));
        assert!(property_names_for("TextBox").contains(&"InnerPadding".to_string()));
        // sorted + non-empty for every named type
        for t in [
            "Button",
            "BarChart",
            "DateTimePicker",
            "NumericUpDown",
            "Shape",
        ] {
            let p = property_names_for(t);
            assert!(!p.is_empty(), "{t} has no properties");
            assert!(p.windows(2).all(|w| w[0] <= w[1]), "{t} not sorted");
        }
    }

    /// `draw_control` falls back to `"Single"`/1px when a control carries no
    /// `BorderStyle`, and CheckBox is not in the frameless skip-list — so with
    /// no border keys of its own every checkbox was boxed in a grey rectangle
    /// the developer could not reach (operator, 2026-08-01). The keys must
    /// exist, and default to `None` so the fallback box is gone.
    /// A ListBox may not be dragged below one line of its own text — a list
    /// that cannot show a single item is a sliver, not a control — and the
    /// floor follows its `FontSize`, so raising the font raises the floor.
    #[test]
    fn a_listbox_cannot_be_dragged_below_one_line() {
        let mut lb = Control::new("ListBox-1", ControlType::ListBox, 0, 0);
        let (w14, h14) = min_control_size(&lb);
        assert!(w14 >= 24, "a list needs room for its text and a scrollbar");
        assert!(
            h14 as f32 >= text_line_height(&lb) + LIST_ROW_PAD * 2.0 + LIST_FRAME_PAD * 2.0 - 1.0,
            "the floor must hold one row: {h14}px"
        );

        lb.set_prop("FontSize", PropValue::Int(28));
        let (_, h28) = min_control_size(&lb);
        assert!(
            h28 > h14,
            "a bigger font needs a taller floor: {h14}px at 14pt, {h28}px at 28pt"
        );

        // Everything else keeps the universal 8x8.
        let button = Control::new("Button-1", ControlType::Button, 0, 0);
        assert_eq!(min_control_size(&button), (8, 8));

        println!(
            "\n  minimum sizes — a ListBox floors at {h14}px tall at 14pt and {h28}px at 28pt; \
             other controls stay at 8x8\n"
        );
    }

    #[test]
    fn checkbox_and_radio_button_expose_border_properties() {
        for t in ["CheckBox", "RadioButton"] {
            let names = property_names_for(t);
            for key in ["BorderStyle", "BorderColor", "BorderWidth"] {
                assert!(names.contains(&key.to_string()), "{t} is missing {key}");
            }
            let ctrl = Control::new("_", ControlType::from_str(t), 0, 0);
            assert_eq!(
                ctrl.get_prop("BorderStyle").unwrap().as_str(),
                "None",
                "{t} must not draw a frame by default"
            );
            // The check glyph keeps its own, separate colour.
            assert_eq!(ctrl.get_prop("CheckColor").unwrap().as_str(), "#0078D7");
        }
    }

    #[test]
    fn new_controls_use_neumorphic_shadow_baseline_defaults() {
        let c = Control::new("AnyControl", ControlType::Button, 0, 0);
        assert_eq!(c.get_prop("FontSize").unwrap().as_i64(), 14);
        assert!(!c.get_prop("ShadowEnabled").unwrap().as_bool());
        assert_eq!(c.get_prop("ShadowOpacity").unwrap().as_i64(), 6);
        assert_eq!(c.get_prop("ShadowColor").unwrap().as_str(), "#000000");
        assert_eq!(c.get_prop("ShadowDirection").unwrap().as_str(), "SouthEast");
        assert_eq!(c.get_prop("ShadowDistance").unwrap().as_i64(), 7);
        assert!(c.get_prop("ShadowBlur").unwrap().as_bool());
        assert_eq!(c.get_prop("ShadowBlurStrength").unwrap().as_i64(), 8);
    }

    #[test]
    fn data_input_controls_default_to_black_foreground() {
        for t in [
            ControlType::TextBox,
            ControlType::CheckBox,
            ControlType::RadioButton,
            ControlType::ListBox,
            ControlType::ComboBox,
            ControlType::DataGrid,
            ControlType::DateTimePicker,
            ControlType::NumericUpDown,
            ControlType::TreeView,
            ControlType::Slider,
            ControlType::Knob,
            ControlType::Switch,
        ] {
            let c = Control::new("Input", t, 0, 0);
            assert_eq!(c.get_prop("ForegroundColor").unwrap().as_str(), "#000000");
        }
    }

    #[test]
    fn applying_neumorphic_defaults_updates_form_and_controls() {
        let mut form = Form::new("MAIN", "Main", 320, 200);
        form.controls
            .push(Control::new("Button-1", ControlType::Button, 10, 10));
        let mut text = Control::new("TextBox-1", ControlType::TextBox, 10, 50);
        text.set_prop("ForegroundColor", PropValue::String("#FFFFFF".into()));
        form.controls.push(text);
        form.controls.push(Control::new(
            "TabControl-1",
            ControlType::TabControl,
            10,
            90,
        ));
        form.apply_neumorphic_defaults();

        assert_eq!(form.glass_style, GlassStyle::Neumorphic);
        assert_eq!(form.background_color, NEUMORPHIC_FORM_BACKGROUND);
        for c in &form.controls {
            assert_eq!(
                c.get_prop("BackgroundColor").unwrap().as_str(),
                NEUMORPHIC_SURFACE_COLOR
            );
            assert_eq!(c.get_prop("ForegroundColor").unwrap().as_str(), "#000000");
            assert_eq!(c.get_prop("CornerRadius").unwrap().as_i64(), 15);
            assert!(c.get_prop("ShadowEnabled").unwrap().as_bool());
            assert_eq!(c.get_prop("ShadowOpacity").unwrap().as_i64(), 6);
            assert_eq!(c.get_prop("ShadowDistance").unwrap().as_i64(), 7);
            assert_eq!(c.get_prop("ShadowDirection").unwrap().as_str(), "SouthEast");
            assert!(c.get_prop("ShadowBlur").unwrap().as_bool());
            assert_eq!(c.get_prop("ShadowBlurStrength").unwrap().as_i64(), 8);
        }
    }

    /// The seeded values are ordinary property values the developer can edit
    /// afterwards, not painting constants — the form takes a solid background
    /// and every control takes a South gradient, mirroring Neumorphic Dark.
    #[test]
    fn neumorphic_light_seeds_the_form_colour_and_a_south_control_gradient() {
        let mut form = Form::new("F".to_string(), "F".to_string(), 400, 300);
        form.controls
            .push(Control::new("Button-1", ControlType::Button, 10, 10));
        form.apply_neumorphic_defaults();

        // Form colours are stored without the leading '#'.
        assert_eq!(form.background_color, "EAEBEFFF");
        // The form itself takes a solid colour, not a gradient.
        assert!(!form.background_gradient_enabled);

        for c in &form.controls {
            assert!(
                c.get_prop("BackgroundGradientEnabled").unwrap().as_bool(),
                "a neumorphic-light control is seeded with its gradient on"
            );
            assert_eq!(
                c.get_prop("BackgroundGradientStartColor").unwrap().as_str(),
                "#F8F8F8FF"
            );
            assert_eq!(
                c.get_prop("BackgroundGradientEndColor").unwrap().as_str(),
                "#DFE0E1FF"
            );
            assert_eq!(
                c.get_prop("BackgroundGradientDirection").unwrap().as_str(),
                "South"
            );
        }
    }

    /// Seeding must go through the same per-style entry point the designer
    /// calls on a toolbox drop, or a dropped control is styled by one path and
    /// a generated one by another.
    #[test]
    fn a_control_seeded_through_the_style_entry_point_gets_the_gradient() {
        let mut c = Control::new("Button-1", ControlType::Button, 0, 0);
        c.apply_glass_style_defaults(GlassStyle::Neumorphic);
        assert!(c.get_prop("BackgroundGradientEnabled").unwrap().as_bool());
        assert_eq!(
            c.get_prop("BackgroundGradientStartColor").unwrap().as_str(),
            NEUMORPHIC_GRADIENT_START
        );

        // The dark variant keeps its own, different gradient.
        let mut d = Control::new("Button-2", ControlType::Button, 0, 0);
        d.apply_glass_style_defaults(GlassStyle::NeumorphicDark);
        assert_eq!(
            d.get_prop("BackgroundGradientStartColor").unwrap().as_str(),
            NEUMORPHIC_DARK_GRADIENT_START
        );

        // Classic seeds nothing — it is not a neumorphic style.
        let mut plain = Control::new("Button-3", ControlType::Button, 0, 0);
        plain.apply_glass_style_defaults(GlassStyle::Classic);
        assert!(!plain.get_prop("BackgroundGradientEnabled").unwrap().as_bool());
    }

    #[test]
    fn charts_have_hide_background_defaulting_off() {
        // Every chart type exposes a HideBackground bool defaulting to false.
        for t in [
            ControlType::BarChart,
            ControlType::LineChart,
            ControlType::PieChart,
            ControlType::AreaChart,
            ControlType::ScatterChart,
            ControlType::DonutChart,
        ] {
            let c = Control::new("C1", t, 0, 0);
            let v = c
                .get_prop("HideBackground")
                .expect("chart missing HideBackground");
            assert!(!v.as_bool(), "HideBackground must default to false");
        }
        // Non-chart controls do not gain the property.
        assert!(Control::new("B", ControlType::Button, 0, 0)
            .get_prop("HideBackground")
            .is_none());
    }

    #[test]
    fn charts_have_monochrome_props_defaulting_off() {
        // Spec 013: every chart exposes Monochrome (off) + a MonochromeColor.
        for t in [
            ControlType::BarChart,
            ControlType::LineChart,
            ControlType::PieChart,
            ControlType::AreaChart,
            ControlType::ScatterChart,
            ControlType::DonutChart,
        ] {
            let c = Control::new("C1", t, 0, 0);
            assert!(!c
                .get_prop("Monochrome")
                .expect("missing Monochrome")
                .as_bool());
            let col = c
                .get_prop("MonochromeColor")
                .expect("missing MonochromeColor");
            assert!(
                col.as_str().starts_with('#'),
                "MonochromeColor should be a hex colour"
            );
            // Grid visibility is the existing ShowGridLines (not a new ShowGrid prop).
            assert!(c.get_prop("ShowGridLines").is_some());
            assert!(
                c.get_prop("ShowGrid").is_none(),
                "no duplicate ShowGrid prop"
            );
        }
        assert!(Control::new("B", ControlType::Button, 0, 0)
            .get_prop("Monochrome")
            .is_none());
    }

    #[test]
    fn containers_expose_container_props_and_helpers() {
        // GroupBox, Panel, TabControl are containers with a corner radius +
        // AutoScroll and a working Opacity; content_rect insets for chrome.
        for t in [
            ControlType::GroupBox,
            ControlType::Panel,
            ControlType::TabControl,
        ] {
            let is_tab = t == ControlType::TabControl;
            let c = Control::new("C1", t, 10, 20);
            assert!(
                c.is_container(),
                "{:?} should be a container",
                c.control_type
            );
            // Unified corner radius (spec 016) replaces the old BorderRadius default.
            assert!(c.get_prop("CornerRadius").is_some(), "missing CornerRadius");
            assert_eq!(
                c.get_prop("HScroll").unwrap().as_bool(),
                false,
                "HScroll default off"
            );
            assert_eq!(
                c.get_prop("VScroll").unwrap().as_bool(),
                false,
                "VScroll default off"
            );
            assert!(
                c.get_prop("Transparency").is_some(),
                "missing Transparency"
            );
            let cr = c.content_rect();
            assert!(
                cr.y > c.rect.y && cr.h < c.rect.h,
                "content_rect must inset for chrome"
            );
            if is_tab {
                assert_eq!(c.get_prop("TabPadding").unwrap().as_i64(), 7);
                assert_eq!(c.get_prop("ActiveTabColor").unwrap().as_str(), "#2C6FD2FF");
                assert_eq!(c.tab_strip_height(), 26);
                assert_eq!(c.tab_content_top_inset(), 33);
                assert_eq!(cr.y, c.rect.y + 35);
            }
        }
        // A non-container keeps a plain content_rect and gains no container props.
        let b = Control::new("B", ControlType::Button, 10, 20);
        assert!(!b.is_container());
        assert!(b.get_prop("HScroll").is_none());
        assert!(b.get_prop("VScroll").is_none());
        assert_eq!(b.content_rect(), b.rect);
        // parent/tab default to None.
        assert!(b.parent.is_none() && b.tab.is_none());
    }

    #[test]
    fn tabcontrol_content_rect_obeys_tab_position() {
        let mut c = Control::new("Tabs", ControlType::TabControl, 10, 20);
        c.rect.w = 300;
        c.rect.h = 200;

        c.set_prop("TabPosition", PropValue::String("Top".into()));
        assert_eq!(c.content_rect(), Rect::new(12, 55, 296, 163));

        c.set_prop("TabPosition", PropValue::String("Bottom".into()));
        assert_eq!(c.content_rect(), Rect::new(12, 22, 296, 163));

        c.set_prop("TabPosition", PropValue::String("Left".into()));
        let left = c.content_rect();
        assert!(left.x > c.rect.x + 2, "left tabs reserve horizontal chrome");
        assert_eq!(left.y, 22);
        assert_eq!(left.h, 196);

        c.set_prop("TabPosition", PropValue::String("Right".into()));
        let right = c.content_rect();
        assert_eq!(right.x, 12);
        assert_eq!(right.y, 22);
        assert!(right.w < 296, "right tabs reserve horizontal chrome");
    }

    #[test]
    fn groupbox_caption_defaults_empty_not_control_id() {
        let group = Control::new("GroupBox-1", ControlType::GroupBox, 10, 20);
        assert_eq!(group.get_prop("Caption").unwrap().as_str(), "");
    }

    #[test]
    fn renaming_groupbox_clears_legacy_generated_caption() {
        let mut form = Form::new("F", "F", 320, 200);
        let mut group = Control::new("GroupBox-1", ControlType::GroupBox, 10, 20);
        group.set_prop("Caption", PropValue::String("GroupBox-1".into()));
        form.controls.push(group);

        assert!(form.rename_control("GroupBox-1", "Menu"));

        let renamed = form.find_control("Menu").unwrap();
        assert_eq!(renamed.get_prop("Caption").unwrap().as_str(), "");
    }

    #[test]
    fn bordered_controls_expose_corner_radius_016() {
        // Every bordered visual control carries CornerRadius with a default that
        // preserves its current look (Button 3, charts 8, others 0) — the
        // progress bar excepted, rounded at 10 by operator decision.
        assert_eq!(
            Control::new("P", ControlType::ProgressBar, 0, 0)
                .get_prop("CornerRadius")
                .unwrap()
                .as_i64(),
            10
        );
        assert_eq!(
            Control::new("B", ControlType::Button, 0, 0)
                .get_prop("CornerRadius")
                .unwrap()
                .as_i64(),
            3
        );
        assert_eq!(
            Control::new("C", ControlType::BarChart, 0, 0)
                .get_prop("CornerRadius")
                .unwrap()
                .as_i64(),
            8
        );
        for t in [
            ControlType::TextBox,
            ControlType::ComboBox,
            ControlType::ListBox,
            ControlType::PictureBox,
            ControlType::DataGrid,
            ControlType::NumericUpDown,
            ControlType::DateTimePicker,
            ControlType::Slider,
            ControlType::Shape,
            ControlType::GroupBox,
            ControlType::Panel,
            ControlType::TabControl,
        ] {
            let c = Control::new("X", t, 0, 0);
            assert_eq!(
                c.get_prop("CornerRadius").unwrap().as_i64(),
                0,
                "{:?} CornerRadius should default to 0",
                c.control_type
            );
        }
        // Non-bordered / non-visual controls do not get a corner radius.
        assert!(Control::new("L", ControlType::Label, 0, 0)
            .get_prop("CornerRadius")
            .is_none());
        assert!(Control::new("T", ControlType::Timer, 0, 0)
            .get_prop("CornerRadius")
            .is_none());
    }

    #[test]
    fn datagrid_alternating_row_opacity_defaults_to_subtle_highlight() {
        let grid = Control::new("DG", ControlType::DataGrid, 0, 0);
        assert_eq!(
            grid.get_prop("AlternatingRowOpacity")
                .expect("DataGrid missing alternating row opacity")
                .as_i64(),
            20
        );
        assert!(property_names_for("DataGrid").contains(&"AlternatingRowOpacity".to_string()));
    }

    #[test]
    fn datagrid_advanced_model_defaults_023() {
        let grid = Control::new("DG", ControlType::DataGrid, 0, 0);
        let advanced = DataGridAdvanced::from_control(&grid);

        assert_eq!(advanced.schema_version, DATAGRID_ADVANCED_SCHEMA_VERSION);
        assert!(advanced.columns.is_empty());
        assert_eq!(advanced.row_height, 22);
        assert_eq!(advanced.frozen_columns, 0);
        assert_eq!(advanced.frozen_rows, 0);
        assert_eq!(advanced.csv_export_mode, DataGridCsvExportMode::Filtered);
        assert_eq!(advanced.csv_delimiter, ",");
        assert_eq!(advanced.grid_line_style, DataGridGridLineStyle::Solid);
        assert!(advanced.selectable_text);

        let names = property_names_for("DataGrid");
        for expected in [
            DATAGRID_ADVANCED_PROP,
            "AllowColumnReorder",
            "AllowRowResize",
            "ColumnFilters",
            "CSVExportMode",
            "FrozenColumns",
            "FrozenRows",
            "GridLineStyle",
            "RowBackgroundPattern",
            "RowHeightOverrides",
            "SelectableText",
            "ShowColumnFilters",
            "ShowCSVExportButton",
        ] {
            assert!(
                names.contains(&expected.to_string()),
                "DataGrid property list missing {expected}"
            );
        }
    }

    #[test]
    fn datagrid_advanced_model_preserves_legacy_columns_023() {
        let mut grid = Control::new("DG", ControlType::DataGrid, 0, 0);
        grid.set_prop(
            "Columns",
            PropValue::String("Actor Id:number\nActor Thumb:string\nActor Caption:string".into()),
        );
        grid.set_prop("RowHeight", PropValue::Int(32));
        grid.set_prop("FrozenColumns", PropValue::Int(1));
        grid.set_prop("GridLineStyle", PropValue::String("Dots".into()));

        let advanced = DataGridAdvanced::from_control(&grid);

        assert_eq!(advanced.columns.len(), 3);
        assert_eq!(advanced.columns[0].id, "ACTOR_ID");
        assert_eq!(advanced.columns[0].title, "Actor Id");
        assert_eq!(advanced.columns[0].source_name, "Actor Id");
        assert_eq!(advanced.columns[0].value_type, "number");
        assert_eq!(advanced.columns[1].title, "Actor Thumb");
        assert_eq!(advanced.row_height, 32);
        assert_eq!(advanced.frozen_columns, 1);
        assert_eq!(advanced.grid_line_style, DataGridGridLineStyle::Dots);
    }

    #[test]
    fn datagrid_advanced_model_reads_json_metadata_023() {
        let mut grid = Control::new("DG", ControlType::DataGrid, 0, 0);
        let mut advanced = DataGridAdvanced::default();
        advanced.columns.push(DataGridColumn {
            id: "STATUS".into(),
            title: "Status".into(),
            source_name: "STATUS".into(),
            value_type: "string".into(),
            width: 160.0,
            frame: Some(DataGridCellFrame {
                enabled: true,
                corner_radius: 14,
                ..DataGridCellFrame::default()
            }),
            value_style_rules: vec![DataGridValueStyleRule {
                value: "Active".into(),
                frame_background_color: "#10B981".into(),
                ..DataGridValueStyleRule::default()
            }],
            ..DataGridColumn::default()
        });
        advanced.frozen_rows = 1;
        advanced.grid_line_style = DataGridGridLineStyle::Dash;
        grid.set_prop(
            DATAGRID_ADVANCED_PROP,
            PropValue::String(advanced.to_json().unwrap()),
        );

        let parsed = DataGridAdvanced::from_control(&grid);

        assert_eq!(parsed.columns.len(), 1);
        assert_eq!(parsed.columns[0].id, "STATUS");
        assert_eq!(parsed.columns[0].width, 160.0);
        assert_eq!(parsed.columns[0].frame.as_ref().unwrap().corner_radius, 14);
        assert_eq!(parsed.columns[0].value_style_rules[0].value, "Active");
        assert_eq!(parsed.frozen_rows, 1);
        assert_eq!(parsed.grid_line_style, DataGridGridLineStyle::Dash);
    }

    #[test]
    fn datagrid_advanced_model_applies_runtime_overrides_023() {
        let mut grid = Control::new("DG", ControlType::DataGrid, 0, 0);
        let mut advanced = DataGridAdvanced::default();
        advanced.columns.push(DataGridColumn {
            id: "ACTOR_CAPTION".into(),
            title: "Actor Caption".into(),
            source_name: "Actor Caption".into(),
            width: 120.0,
            ..DataGridColumn::default()
        });
        advanced.filters.push(DataGridFilter {
            column_id: "ACTOR_CAPTION".into(),
            value: "old".into(),
            active: true,
        });
        grid.set_prop(
            DATAGRID_ADVANCED_PROP,
            PropValue::String(
                advanced
                    .to_json()
                    .expect("advanced metadata should serialize"),
            ),
        );
        grid.set_prop("_RuntimeRowHeight", PropValue::Int(28));
        grid.set_prop("_RuntimeFrozenColumns", PropValue::Int(1));
        grid.set_prop("_RuntimeFrozenRows", PropValue::Int(2));
        grid.set_prop(
            "_RuntimeColumnFilters",
            PropValue::String("Actor Caption=Joe".into()),
        );
        grid.set_prop(
            "_RuntimeColumnWidths",
            PropValue::String("Actor Caption=220".into()),
        );

        let parsed = DataGridAdvanced::from_control(&grid);

        assert_eq!(parsed.row_height, 28);
        assert_eq!(parsed.frozen_columns, 1);
        assert_eq!(parsed.frozen_rows, 2);
        assert_eq!(parsed.columns[0].width, 220.0);
        assert_eq!(parsed.filters.len(), 1);
        assert_eq!(parsed.filters[0].column_id, "Actor Caption");
        assert_eq!(parsed.filters[0].value, "Joe");
        assert!(parsed.filters[0].active);
    }

    #[test]
    fn datagrid_resize_updates_width_without_losing_column_identity_023() {
        let mut grid = Control::new("DG", ControlType::DataGrid, 0, 0);
        grid.set_prop(
            "Columns",
            PropValue::String("CUSTOMER-ID:number\nCUSTOMER-NAME:string".into()),
        );

        let mut advanced = DataGridAdvanced::from_control(&grid);
        advanced.set_column_width(1, 220.0);
        grid.set_prop(
            DATAGRID_ADVANCED_PROP,
            PropValue::String(advanced.to_json().unwrap()),
        );

        let parsed = DataGridAdvanced::from_control(&grid);
        assert_eq!(parsed.columns[0].id, "CUSTOMER_ID");
        assert_eq!(parsed.columns[0].source_name, "CUSTOMER-ID");
        assert_eq!(parsed.columns[0].width, 120.0);
        assert_eq!(parsed.columns[1].id, "CUSTOMER_NAME");
        assert_eq!(parsed.columns[1].source_name, "CUSTOMER-NAME");
        assert_eq!(parsed.columns[1].width, 220.0);
    }

    #[test]
    fn datagrid_filter_reorder_chains_active_filters_023() {
        let mut grid = Control::new("DG", ControlType::DataGrid, 0, 0);
        grid.set_prop(
            "Columns",
            PropValue::String("STATUS:string\nREGION:string\nNAME:string".into()),
        );
        let mut advanced = DataGridAdvanced::from_control(&grid);
        advanced.set_filter("STATUS", "Active");
        advanced.set_filter("REGION", "North");

        let rows = vec![
            vec!["Active".into(), "North".into(), "Acme".into()],
            vec!["Active".into(), "South".into(), "Beta".into()],
            vec!["Trial".into(), "North".into(), "Coda".into()],
            vec!["Active".into(), "Northwest".into(), "Delta".into()],
        ];
        let sources = vec!["STATUS".into(), "REGION".into(), "NAME".into()];

        assert_eq!(
            advanced.filtered_row_indices_for_sources(&rows, &sources),
            vec![0, 3]
        );

        advanced.set_filter("REGION", "");
        assert_eq!(
            advanced.filtered_row_indices_for_sources(&rows, &sources),
            vec![0, 1, 3]
        );
    }

    #[test]
    fn datagrid_filter_reorder_preserves_metadata_on_move_023() {
        let mut grid = Control::new("DG", ControlType::DataGrid, 0, 0);
        grid.set_prop(
            "Columns",
            PropValue::String("ID:number\nSTATUS:string\nAMOUNT:number".into()),
        );
        let mut advanced = DataGridAdvanced::from_control(&grid);
        advanced.columns[1].width = 180.0;
        advanced.columns[1].filter_enabled = true;
        advanced.columns[1].frame = Some(DataGridCellFrame {
            enabled: true,
            corner_radius: 10,
            ..DataGridCellFrame::default()
        });
        advanced.set_filter("STATUS", "Active");

        assert!(advanced.move_column_left(1));

        assert_eq!(advanced.columns[0].id, "STATUS");
        assert_eq!(advanced.columns[0].source_name, "STATUS");
        assert_eq!(advanced.columns[0].width, 180.0);
        assert!(advanced.columns[0].filter_enabled);
        assert_eq!(
            advanced.columns[0].frame.as_ref().unwrap().corner_radius,
            10
        );
        assert_eq!(advanced.filters[0].column_id, "STATUS");

        let rows = vec![
            vec!["1".into(), "Active".into(), "10".into()],
            vec!["2".into(), "Trial".into(), "20".into()],
        ];
        let sources = vec!["ID".into(), "STATUS".into(), "AMOUNT".into()];
        assert_eq!(
            advanced.filtered_row_indices_for_sources(&rows, &sources),
            vec![0]
        );
    }

    #[test]
    fn datagrid_rich_cells_resolve_value_rules_and_gauges_023() {
        let column = DataGridColumn {
            id: "STATUS".into(),
            title: "Status".into(),
            source_name: "STATUS".into(),
            frame: Some(DataGridCellFrame {
                enabled: true,
                corner_radius: 18,
                ..DataGridCellFrame::default()
            }),
            value_style_rules: vec![
                DataGridValueStyleRule {
                    value: "Active".into(),
                    frame_background_color: "#10B981".into(),
                    frame_foreground_color: "#FFFFFF".into(),
                    ..DataGridValueStyleRule::default()
                },
                DataGridValueStyleRule {
                    value: "Churned".into(),
                    frame_background_color: "#EF4444".into(),
                    frame_foreground_color: "#FFFFFF".into(),
                    ..DataGridValueStyleRule::default()
                },
            ],
            ..DataGridColumn::default()
        };

        assert_eq!(
            column
                .value_style_rule_for("active")
                .expect("active rule")
                .frame_background_color,
            "#10B981"
        );
        assert_eq!(
            column
                .value_style_rule_for("Churned")
                .expect("churned rule")
                .frame_background_color,
            "#EF4444"
        );
        assert!(column.value_style_rule_for("Trial").is_none());

        let gauge = DataGridGauge {
            enabled: true,
            min: 100.0,
            max: 300.0,
            ..DataGridGauge::default()
        };
        assert_eq!(gauge.fraction_for_value("100"), Some(0.0));
        assert_eq!(gauge.fraction_for_value("200"), Some(0.5));
        assert_eq!(gauge.fraction_for_value("500"), Some(1.0));
    }

    #[test]
    fn datagrid_rich_cells_grid_line_style_defaults_023() {
        assert_eq!(
            DataGridGridLineStyle::from_str("solid"),
            DataGridGridLineStyle::Solid
        );
        assert_eq!(
            DataGridGridLineStyle::from_str("dashed"),
            DataGridGridLineStyle::Dash
        );
        assert_eq!(
            DataGridGridLineStyle::from_str("dotted"),
            DataGridGridLineStyle::Dots
        );
        assert_eq!(
            DataGridGridLineStyle::from_str("none"),
            DataGridGridLineStyle::None
        );
        assert_eq!(
            DataGridGridLineStyle::from_str("unknown"),
            DataGridGridLineStyle::Solid
        );
    }

    #[test]
    fn deleting_a_bound_control_prunes_its_data_binding() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        form.controls
            .push(Control::new("DG", ControlType::DataGrid, 0, 0));
        form.controls
            .push(Control::new("KEEP", ControlType::DataGrid, 0, 0));

        let table = |name: &str| BindingSourceDescriptor::CobolTable {
            table_name: name.into(),
            occurs_item: "ROW".into(),
            fields: vec![BindingField::new("F", BindingDataType::Text)],
            key_fields: vec![],
            writable: false,
        };
        form.data_bindings.push(DataBindingDef::new(
            "b1",
            "Grid binding",
            table("T1"),
            BindingTargetDescriptor::DataGrid {
                control_id: "DG".into(),
            },
        ));
        form.data_bindings.push(DataBindingDef::new(
            "b2",
            "Keep binding",
            table("T2"),
            BindingTargetDescriptor::DataGrid {
                control_id: "KEEP".into(),
            },
        ));
        assert_eq!(form.data_bindings.len(), 2);

        // Deleting DG must remove b1 but leave b2 (targets a surviving control).
        form.recycle_control("DG", "test-delete");
        assert_eq!(form.data_bindings.len(), 1);
        assert_eq!(form.data_bindings[0].id, "b2");
    }

    #[test]
    fn deleting_an_array_member_prunes_its_mapping_but_keeps_binding() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        let mut group = Control::new("CARD", ControlType::GroupBox, 0, 0);
        group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        form.controls.push(group);
        form.controls
            .push(Control::new("LBL-A", ControlType::Label, 0, 0));
        form.controls
            .push(Control::new("LBL-B", ControlType::Label, 0, 0));

        let mut binding = DataBindingDef::new(
            "arr",
            "Array binding",
            BindingSourceDescriptor::CobolTable {
                table_name: "T".into(),
                occurs_item: "ROW".into(),
                fields: vec![
                    BindingField::new("FA", BindingDataType::Text),
                    BindingField::new("FB", BindingDataType::Text),
                ],
                key_fields: vec![],
                writable: false,
            },
            BindingTargetDescriptor::ControlArray {
                array_id: "CARD".into(),
                member_control_ids: vec!["LBL-A".into(), "LBL-B".into()],
            },
        );
        binding.mappings = vec![
            FieldMapping::new(
                "FA",
                BindingTargetPath::ControlProperty {
                    array_id: "CARD".into(),
                    control_id: "LBL-A".into(),
                    property_name: "Caption".into(),
                },
            ),
            FieldMapping::new(
                "FB",
                BindingTargetPath::ControlProperty {
                    array_id: "CARD".into(),
                    control_id: "LBL-B".into(),
                    property_name: "Caption".into(),
                },
            ),
        ];
        form.data_bindings.push(binding);

        // Delete one member: its mapping goes, the binding (array host survives) stays.
        form.recycle_control("LBL-A", "d1");
        assert_eq!(form.data_bindings.len(), 1);
        assert_eq!(form.data_bindings[0].mappings.len(), 1);
        assert_eq!(form.data_bindings[0].mappings[0].source_field, "FB");

        // Delete the host GroupBox: the whole array binding is pruned.
        form.recycle_control("CARD", "d2");
        assert!(form.data_bindings.is_empty());
    }

    #[test]
    fn groupbox_exposes_visual_and_repeating_props_015() {
        // Phase-1 visual props + Phase-2 repeating-group metadata default safely
        // (the box looks/behaves unchanged until they are turned on).
        let g = Control::new("CustomerCard", ControlType::GroupBox, 0, 0);
        // Visual (Phase 1)
        assert_eq!(g.get_prop("HideCaption").unwrap().as_bool(), false);
        assert_eq!(g.get_prop("HideBackground").unwrap().as_bool(), false);
        assert_eq!(
            g.get_prop("BackgroundGradientEnabled").unwrap().as_bool(),
            false
        );
        assert!(g.get_prop("BackgroundGradientStartColor").is_some());
        assert!(g.get_prop("BackgroundGradientEndColor").is_some());
        assert_eq!(
            g.get_prop("BackgroundGradientDirection").unwrap().as_str(),
            "South"
        );
        // Repeating (Phase 2)
        assert_eq!(g.get_prop("UserControl").unwrap().as_str(), "");
        assert_eq!(g.get_prop("IsRepeatingGroup").unwrap().as_bool(), false);
        assert_eq!(g.get_prop("ArrayName").unwrap().as_str(), "");
        assert_eq!(g.get_prop("ItemCount").unwrap().as_i64(), 0);
        assert_eq!(g.get_prop("LayoutDirection").unwrap().as_str(), "Vertical");
        assert_eq!(g.get_prop("ItemSpacing").unwrap().as_i64(), 8);
        assert_eq!(g.get_prop("ItemsPerRow").unwrap().as_i64(), 1);
        // AutoScrollParent removed per user request; parent decides scroll via its HScroll/VScroll.
        assert_eq!(g.get_prop("CloneEvents").unwrap().as_bool(), true);
        assert_eq!(g.get_prop("PreviewItemCount").unwrap().as_i64(), 1);
        // Not leaked onto Panel/TabControl or plain controls.
        let p = Control::new("P", ControlType::Panel, 0, 0);
        assert!(p.get_prop("IsRepeatingGroup").is_none());
        assert!(p.get_prop("HideCaption").is_none());
        let b = Control::new("B", ControlType::Button, 0, 0);
        assert!(b.get_prop("PreviewItemCount").is_none());
    }

    #[test]
    fn set_prop_replaces_existing_key_case_insensitively() {
        let mut c = Control::new("Label-5", ControlType::Label, 0, 0);
        c.set_prop("CAPTION", PropValue::String("42".into()));

        assert_eq!(c.get_prop("Caption").unwrap().as_str(), "42");
        assert!(
            !c.properties.contains_key("Caption"),
            "runtime uppercase update must not leave stale designed Caption"
        );
    }

    #[cfg(feature = "render")]
    #[test]
    fn opacity_of_reads_property_012() {
        // The render walk uses this to fade a container subtree (spec 012).
        // Transparency runs the other way from the Opacity it replaced: 0 is
        // opaque, 100 is invisible.
        let mut c = Control::new("C", ControlType::Panel, 0, 0);
        assert_eq!(crate::paint::opacity_of(&c), 1.0); // default 0 % transparent
        c.set_prop("Transparency", PropValue::Int(50));
        assert!((crate::paint::opacity_of(&c) - 0.5).abs() < 1e-6);
        c.set_prop("Transparency", PropValue::Int(100));
        assert_eq!(crate::paint::opacity_of(&c), 0.0);
    }

    /// A form saved before the rename still reads correctly: `Opacity` is the
    /// complement of `Transparency`, so a control faded to 40 % opacity must
    /// come back as 60 % transparent and not as fully opaque.
    #[cfg(feature = "render")]
    #[test]
    fn a_legacy_opacity_still_reads_as_its_complement() {
        let mut c = Control::new("C", ControlType::Panel, 0, 0);
        c.properties.shift_remove("Transparency");
        c.set_prop("Opacity", PropValue::Int(40));
        assert_eq!(transparency_of(&c), 60);
        assert!((crate::paint::opacity_of(&c) - 0.4).abs() < 1e-6);
    }

    /// A CheckBox is a tick and a caption, not a card: it starts with no face
    /// at all, so it sits on whatever it was dropped onto instead of punching
    /// a rectangle through it.
    #[test]
    fn a_checkbox_starts_fully_transparent() {
        let cb = Control::new("chk", ControlType::CheckBox, 0, 0);
        assert_eq!(transparency_of(&cb), 100, "a CheckBox has no background");

        // Everything else still starts opaque.
        for t in [ControlType::Panel, ControlType::Button, ControlType::TextBox] {
            let c = Control::new("c", t.clone(), 0, 0);
            assert_eq!(transparency_of(&c), 0, "{t:?} must start opaque");
        }
    }

    #[test]
    fn form_events_unique_and_include_lifecycle() {
        let all: Vec<&str> = form_supported_events().collect();
        // No duplicates across groups.
        let mut seen = std::collections::HashSet::new();
        for ev in &all {
            assert!(seen.insert(*ev), "duplicate form event: {ev}");
            assert!(ev.starts_with("on"), "form event not 'on'-prefixed: {ev}");
        }
        // Pre-stubbed lifecycle events are present.
        assert!(all.contains(&"onLoad"));
        assert!(all.contains(&"onClose"));
        // A representative sample from later groups.
        for ev in [
            "onResize",
            "onDoubleClick",
            "onPaste",
            "onUnhandledException",
            // Spec 037 lifecycle additions (wired, not just designable).
            "onCloseRejected",
            "onFullScreenChanged",
        ] {
            assert!(all.contains(&ev), "missing form event: {ev}");
        }
        assert_eq!(all.len(), 68, "expected 68 form events (66 + 2 from 037)");
    }

    #[test]
    fn button_supported_events_include_expanded_visual_events() {
        let events = ControlType::Button.supported_events();
        for ev in [
            "onClick",
            "onDblClick",
            "onDoubleClick",
            "onRightClick",
            "onContextMenu",
            "onMouseMove",
            "onKeyDown",
            "onEnterPressed",
            "onHoverEnter",
            "onResize",
            "onResized",
            "onVisibleChanged",
            "onLoad",
        ] {
            assert!(events.contains(&ev), "missing Button event: {ev}");
        }
        // Phantom events (drag family, onTooltipShow, onPropertyChanged) are
        // deliberately NOT advertised — the runtime has no engine for them.
        for phantom in ["onDragStart", "onDrop", "onTooltipShow", "onPropertyChanged"] {
            assert!(!events.contains(&phantom), "phantom Button event: {phantom}");
        }
        assert!(
            events.len() >= 25,
            "Button Events panel should expose the expanded event list"
        );
    }

    #[test]
    fn textbox_supported_events_include_keyboard_and_text_events() {
        let events = ControlType::TextBox.supported_events();
        for ev in [
            "onChange",
            "onTextChanged",
            "onKeyDown",
            "onKeyUp",
            "onKeyPress",
            "onEnterPressed",
            "onEscapePressed",
            "onGotFocus",
            "onLostFocus",
            "onHoverEnter",
        ] {
            assert!(events.contains(&ev), "missing TextBox event: {ev}");
        }
    }

    #[test]
    fn non_visual_supported_events_include_async_lifecycle() {
        // Timer / AgentObject are unaffected by the async I/O work (spec 032).
        assert_eq!(ControlType::Timer.supported_events(), &["onTick"]);
        assert_eq!(
            ControlType::AgentObject.supported_events(),
            &["onResponse", "onError"]
        );
        // RestClient / SqlDatabase / IndexedFile gain the uniform async lifecycle
        // events onComplete/onError/onCancelled/onTimeout (skipping any the
        // control already declared).
        assert_eq!(
            ControlType::RestClient.supported_events(),
            &["onError", "onTimeout", "onComplete", "onCancelled"]
        );
        assert_eq!(
            ControlType::SqlDatabase.supported_events(),
            &[
                "onQueryComplete",
                "onConnectOk",
                "onConnectError",
                "onQueryError",
                "onRowFetched",
                "onComplete",
                "onError",
                "onCancelled",
                "onTimeout",
            ]
        );
    }

    #[test]
    fn async_controls_have_mode_busy_timeout_and_no_duplicate_events() {
        for (ct, default_mode) in [
            (ControlType::RestClient, "Async"),
            (ControlType::SqlDatabase, "Sync"),
            (ControlType::IndexedFile, "Sync"),
        ] {
            // The async lifecycle events must be present and unique.
            let evs = ct.supported_events();
            for ev in ["onComplete", "onError", "onCancelled", "onTimeout"] {
                assert!(evs.contains(&ev), "{ct:?} missing {ev}");
            }
            let mut seen = std::collections::HashSet::new();
            for ev in evs {
                assert!(seen.insert(*ev), "{ct:?} duplicate event {ev}");
            }

            // `ct` is consumed here (ControlType is not Copy), so this comes last.
            let c = Control::new("X", ct, 0, 0);
            assert_eq!(
                c.properties.get("Mode"),
                Some(&PropValue::String(default_mode.into())),
                "default Mode for {default_mode}"
            );
            assert!(matches!(c.properties.get("Busy"), Some(PropValue::Bool(false))));
            assert!(matches!(c.properties.get("TimeoutMs"), Some(PropValue::Int(_))));
        }
    }

    #[test]
    fn picturebox_supported_events_do_not_include_keyboard_events() {
        let events = ControlType::PictureBox.supported_events();
        assert!(events.contains(&"onImageLoaded"));
        assert!(events.contains(&"onHoverEnter"));
        assert!(!events.contains(&"onKeyDown"));
        assert!(!events.contains(&"onEnterPressed"));
    }

    #[test]
    fn form_add_and_find() {
        let mut form = Form::new("MAIN-FORM", "My App", 800, 600);
        let ctrl = Control::new("BTN-OK", ControlType::Button, 10, 10);
        form.add_control(ctrl);
        assert!(form.find_control("BTN-OK").is_some());
        assert!(form.find_control("btn-ok").is_some());
        assert!(form.find_control("NONEXISTENT").is_none());
    }

    #[test]
    fn control_default_size_button() {
        let (w, h) = ControlType::Button.default_size();
        assert_eq!(w, 80);
        assert_eq!(h, 28);
    }

    #[test]
    fn button_icon_properties_default_to_left_with_padding_and_size() {
        let b = Control::new("Button1", ControlType::Button, 0, 0);
        assert_eq!(b.get_prop("IconPath").unwrap().as_str(), "");
        assert_eq!(b.get_prop("IconAlignment").unwrap().as_str(), "Left");
        assert_eq!(b.get_prop("IconPadding").unwrap().as_i64(), 10);
        assert_eq!(b.get_prop("IconSize").unwrap().as_i64(), 32);
        assert_eq!(b.get_prop("BorderWidth").unwrap().as_i64(), 1);
    }

    #[test]
    fn prop_value_roundtrip() {
        let v = PropValue::Int(42);
        assert_eq!(v.as_i64(), 42);
        assert_eq!(v.to_xml_string(), "42");
    }

    #[test]
    fn animation_def_basic() {
        let mut ctrl = Control::new("BTN-1", ControlType::Button, 10, 10);
        let anim = AnimationDef::new("fly-in");
        ctrl.add_animation(anim);
        assert_eq!(ctrl.animations.len(), 1);
        assert_eq!(ctrl.animations[0].name, "fly-in");
    }

    #[test]
    fn easing_linear() {
        let e = EasingKind::Linear;
        assert!((e.apply(0.5) - 0.5).abs() < 0.001);
    }

    #[test]
    fn z_order_sort() {
        let mut form = Form::new("F", "T", 800, 600);
        let mut a = Control::new("A", ControlType::Label, 0, 0);
        a.z_order = 5;
        let mut b = Control::new("B", ControlType::Label, 0, 0);
        b.z_order = 1;
        form.controls.push(a);
        form.controls.push(b);
        let sorted = form.controls_by_z();
        assert_eq!(sorted[0].id, "B");
        assert_eq!(sorted[1].id, "A");
    }

    #[test]
    fn agent_object_defaults() {
        let ctrl = Control::new("AGT-1", ControlType::AgentObject, 0, 0);
        assert!(ctrl.get_prop("AgentURL").is_some());
        assert!(ctrl.get_prop("AgentModel").is_some());
        assert!(ctrl.get_prop("SystemPrompt").is_some());
    }

    // ── Spec 039 T2: Knob/Gauge/Switch/FileDropZone ────────────────────────

    #[test]
    fn knob_gauge_switch_file_drop_zone_round_trip_through_as_str_and_from_str() {
        for t in [
            ControlType::Knob,
            ControlType::Gauge,
            ControlType::Switch,
            ControlType::FileDropZone,
        ] {
            let s = t.as_str();
            assert_eq!(ControlType::from_str(s), t, "{s} did not round-trip");
        }
        assert_eq!(ControlType::Knob.as_str(), "Knob");
        assert_eq!(ControlType::Gauge.as_str(), "Gauge");
        assert_eq!(ControlType::Switch.as_str(), "Switch");
        assert_eq!(ControlType::FileDropZone.as_str(), "FileDropZone");
    }

    #[test]
    fn knob_defaults_match_egui_elegance_real_surface() {
        let c = Control::new("KNB-1", ControlType::Knob, 0, 0);
        assert_eq!(c.get_prop("Minimum").unwrap().as_i64(), 0);
        assert_eq!(c.get_prop("Maximum").unwrap().as_i64(), 100);
        assert_eq!(c.get_prop("Value").unwrap().as_i64(), 0);
        assert_eq!(c.get_prop("Step").unwrap().as_i64(), 1);
        // No Size preset: the painter draws the dial at the control's own size.
        assert!(c.get_prop("Size").is_none());
        assert_eq!(c.get_prop("Accent").unwrap().as_str(), "Blue");
        assert!(!c.get_prop("Bipolar").unwrap().as_bool());
        assert!(c.get_prop("ShowValue").unwrap().as_bool());
        assert_eq!(c.get_prop("DefaultValue").unwrap().as_i64(), 0);
        assert!(c.get_prop("Label").is_some());
        // The original ask's track-thickness/inner-track-colour/gradient
        // properties are deliberately absent — egui-elegance's Knob widget
        // has no such API (plan.md §4 Decision 4).
        assert!(c.get_prop("TrackThickness").is_none());
        assert!(c.get_prop("InnerTrackColor").is_none());
        assert!(c.get_prop("InnerTrackColorEffect").is_none());
    }

    #[test]
    fn gauge_defaults_cover_all_three_styles_and_stays_non_interactive() {
        let c = Control::new("GAU-1", ControlType::Gauge, 0, 0);
        assert_eq!(c.get_prop("GaugeStyle").unwrap().as_str(), "Radial");
        assert_eq!(c.get_prop("Minimum").unwrap().as_i64(), 0);
        assert_eq!(c.get_prop("Maximum").unwrap().as_i64(), 100);
        assert_eq!(c.get_prop("Value").unwrap().as_i64(), 0);
        assert!(c.get_prop("WarningThreshold").unwrap().as_str().is_empty());
        assert!(c.get_prop("CriticalThreshold").unwrap().as_str().is_empty());
        // Radial-only
        assert!(c.get_prop("ShowNeedle").unwrap().as_bool());
        assert!(c.get_prop("ShowScale").unwrap().as_bool());
        // Linear-only
        assert_eq!(c.get_prop("BarHeight").unwrap().as_i64(), 14);
        assert!(c.get_prop("ShowThumb").unwrap().as_bool());
        // Donut-only
        assert_eq!(c.get_prop("StrokeWidth").unwrap().as_i64(), 8);
        // R10: Gauge's primary_event is never a click-driven value change —
        // it is not in Gauge's own supported_events(), so the fallback
        // primary_event() value is functionally inert.
        assert!(
            !ControlType::Gauge
                .supported_events()
                .contains(&"onValueChanged")
        );
    }

    #[test]
    fn switch_defaults_have_checked_and_accent_only() {
        let c = Control::new("SWT-1", ControlType::Switch, 0, 0);
        assert!(!c.get_prop("Checked").unwrap().as_bool());
        assert_eq!(c.get_prop("Accent").unwrap().as_str(), "Blue");
        // No OnColor/OffColor — egui-elegance's Switch has no such API.
        assert!(c.get_prop("OnColor").is_none());
        assert!(c.get_prop("OffColor").is_none());
    }

    #[test]
    fn file_drop_zone_defaults_have_hint_and_no_design_time_dropped_files() {
        let c = Control::new("FDZ-1", ControlType::FileDropZone, 0, 0);
        assert!(c.get_prop("Hint").unwrap().as_str().is_empty());
        // DroppedFiles is runtime-only (R13/R14) — never a designer default.
        assert!(c.get_prop("DroppedFiles").is_none());
    }

    #[test]
    fn knob_switch_file_drop_zone_default_sizes_are_reasonable() {
        assert_eq!(ControlType::Knob.default_size(), (80, 96));
        assert_eq!(ControlType::Gauge.default_size(), (140, 90));
        assert_eq!(ControlType::Switch.default_size(), (52, 28));
        assert_eq!(ControlType::FileDropZone.default_size(), (220, 100));
    }

    #[test]
    /// The reported bug, and the audit of every other control for the same
    /// thing: an OBSERVER event must fire when its property changes, whoever
    /// changed it — and a PASSIVE event must never fire from a property write.
    #[test]
    fn observer_events_fire_for_a_written_property_and_passive_ones_never_do() {
        // The bug as reported: a Timer raising a Knob's Value.
        let knob = ControlType::Knob.observer_events_for("Value");
        assert!(
            knob.contains(&"onValueChanged"),
            "a Knob's Value must be observable — this is the reported bug: {knob:?}"
        );
        assert!(knob.contains(&"onChange"), "…and its historical name too");

        // Every value-bearing control, audited. A control that DECLARES a value
        // event must fire it on a write; one that declares none has nothing to
        // fire, and that is a finding in its own right — recorded here rather
        // than papered over.
        let mut silent: Vec<String> = Vec::new();
        for ct in [
            ControlType::Slider,
            ControlType::NumericUpDown,
            ControlType::ProgressBar,
            ControlType::Gauge,
            ControlType::DateTimePicker,
        ] {
            let declares = ct
                .supported_events()
                .iter()
                .any(|e| *e == "onValueChanged" || *e == "onChange");
            let got = ct.observer_events_for("Value");
            if declares {
                assert!(
                    !got.is_empty(),
                    "{ct:?} declares a value event but a Value write does not fire it"
                );
            } else {
                assert!(
                    got.is_empty(),
                    "{ct:?} declares no value event, so nothing may be fired: {got:?}"
                );
                silent.push(ct.as_str().to_owned());
            }
        }
        // The Gauge is the one that matters: it is READ-ONLY to the user, so a
        // Timer or a handler is the ONLY thing that ever moves it — and it has no
        // value event to announce that with. Giving it one adds a capability to
        // the control, which is the operator's call, not a bug fix's.
        assert_eq!(
            silent,
            vec!["Gauge"],
            "the set of value controls with no value event changed"
        );
        // The rest of the state-bearing controls, by the same rule: declared ⇒
        // must fire, not declared ⇒ recorded as a gap rather than asserted away.
        let mut gaps: Vec<String> = Vec::new();
        for (ct, prop, candidates) in [
            (ControlType::TextBox, "Text", &["onChange", "onTextChanged"][..]),
            (ControlType::CheckBox, "Checked", &["onChange", "onCheckedChanged"][..]),
            (ControlType::RadioButton, "Checked", &["onChange", "onCheckedChanged"][..]),
            (ControlType::Switch, "Checked", &["onChange", "onCheckedChanged"][..]),
            (ControlType::ComboBox, "SelectedIndex", &["onChange", "onSelectedIndexChanged"][..]),
            (ControlType::ListBox, "SelectedIndex", &["onChange", "onSelectedIndexChanged"][..]),
            (ControlType::DataGrid, "Rows", &["onDataChanged"][..]),
            (ControlType::TreeView, "Items", &["onDataChanged"][..]),
        ] {
            let declares = ct
                .supported_events()
                .iter()
                .any(|e| candidates.iter().any(|c| e.eq_ignore_ascii_case(c)));
            let got = ct.observer_events_for(prop);
            if declares {
                assert!(
                    !got.is_empty(),
                    "{ct:?} declares one of {candidates:?} but a {prop} write fires nothing"
                );
            } else {
                assert!(got.is_empty(), "{ct:?} may not fire what it does not declare");
                gaps.push(format!("{}.{prop}", ct.as_str()));
            }
        }

        // Visible/Enabled are observable on anything that declares them.
        let mut vis = 0usize;
        for ct in ControlType::ALL {
            if ct.supported_events().contains(&"onVisibleChanged") {
                assert!(
                    ct.observer_events_for("Visible")
                        .contains(&"onVisibleChanged"),
                    "{ct:?} declares onVisibleChanged but a Visible write does not fire it"
                );
                vis += 1;
            }
        }
        assert!(vis > 10, "expected most controls to observe Visible, got {vis}");

        // ── The other half of the rule ───────────────────────────────────
        // A property write is not a user act, so no passive event may EVER come
        // out of this — on any control, for any property.
        const PASSIVE: &[&str] = &[
            "onClick",
            "onDblClick",
            "onDoubleClick",
            "onRightClick",
            "onMiddleClick",
            "onMouseDown",
            "onMouseUp",
            "onMouseMove",
            "onMouseEnter",
            "onMouseLeave",
            "onMouseWheel",
            "onContextMenu",
            "onGotFocus",
            "onLostFocus",
            "onKeyPress",
            "onEnterPressed",
            "onEscapePressed",
            "onHoverEnter",
            "onHoverLeave",
            "onLoad",
            "onDestroy",
            "onFilesDropped",
            "onFilesRejected",
        ];
        let probed = [
            "Value", "Text", "Checked", "Visible", "Enabled", "Caption", "Items",
            "SelectedIndex", "Rows", "Columns", "Data", "Width", "Height", "X", "Y",
            "BackgroundColor", "FontSize", "Interval",
        ];
        let mut checks = 0usize;
        for ct in ControlType::ALL {
            for prop in probed {
                for got in ct.observer_events_for(prop) {
                    assert!(
                        !PASSIVE.contains(&got),
                        "{ct:?} would fire the PASSIVE event {got} for a write to \
                         {prop} — a property write is not a user action"
                    );
                    // And it must be an event the control actually declares.
                    assert!(
                        ct.supported_events().iter().any(|d| d.eq_ignore_ascii_case(got)),
                        "{ct:?} would fire {got}, which it does not declare"
                    );
                    checks += 1;
                }
            }
        }

        // Geometry and styling are not observable: nothing declares an event for
        // them, so a resize from COBOL must stay silent here.
        for prop in ["Width", "BackgroundColor", "FontSize"] {
            assert!(
                ControlType::Knob.observer_events_for(prop).is_empty(),
                "{prop} is not an observable value"
            );
        }
        // An unknown property is silent, not a panic.
        assert!(ControlType::Knob.observer_events_for("NoSuchProp").is_empty());
        assert!(ControlType::Knob.observer_events_for("").is_empty());

        println!(
            "\n  Observer/passive audit — {checks} (control, property) ⇒ event pairs across \
             {} control types: every one is an observer the control declares, none is \
             passive. Knob Value ⇒ {knob:?}; {vis} controls observe Visible; geometry and \
             colour observe nothing.\n  FINDINGS — no event declared to announce a \
             change, so a write to these can tell the form nothing: value controls \
             {silent:?}, others {gaps:?}\n",
            ControlType::ALL.len()
        );
    }

    #[test]
    fn knob_primary_event_is_on_change_switch_and_file_drop_zone_have_their_own() {
        assert_eq!(ControlType::Knob.primary_event(), "onChange");
        assert_eq!(ControlType::Switch.primary_event(), "onClick");
        assert_eq!(ControlType::FileDropZone.primary_event(), "onFilesDropped");
    }

    // ── Spec 039 T8: Maps scaffolding ───────────────────────────────────────

    #[test]
    fn maps_round_trips_through_as_str_and_from_str() {
        assert_eq!(ControlType::Maps.as_str(), "Maps");
        assert_eq!(ControlType::from_str("Maps"), ControlType::Maps);
    }

    #[test]
    fn maps_default_properties_need_no_api_key_for_the_basemap() {
        let c = Control::new("MAP-1", ControlType::Maps, 0, 0);
        assert_eq!(c.get_prop("CenterLat").unwrap().as_str(), "0");
        assert_eq!(c.get_prop("CenterLng").unwrap().as_str(), "0");
        assert_eq!(c.get_prop("Zoom").unwrap().as_i64(), 2);
        assert!(c.get_prop("Markers").unwrap().as_str().is_empty());
        // R33: ApiKeySource is present but starts empty — the OSM basemap
        // renders with no key at all; only Directions/Geocoding/Places
        // calls need one, resolved from the project secret store, not this
        // property carrying a literal (R17/R31).
        assert!(c.get_prop("ApiKeySource").unwrap().as_str().is_empty());
    }

    #[test]
    fn maps_default_size_and_events() {
        assert_eq!(ControlType::Maps.default_size(), (320, 240));
        assert_eq!(ControlType::Maps.primary_event(), "onMapClick");
        let events = ControlType::Maps.supported_events();
        for want in ["onMapClick", "onMarkerClick", "onBoundsChanged"] {
            assert!(events.contains(&want), "Maps missing {want}: {events:?}");
        }
    }

    /// A Maps control's five data methods are always async, so it MUST offer
    /// the same four uniform async lifecycle events (spec 032) that every
    /// other async control offers. Without them the Designer cannot bind a
    /// handler, and the interpreter's `onComplete` — carrying the geocode
    /// result in `ResponseBody` — is dropped for want of a binding.
    #[test]
    fn maps_offers_the_async_lifecycle_events_like_every_other_async_control() {
        let maps = ControlType::Maps.supported_events();
        for want in ["onComplete", "onError", "onTimeout", "onCancelled"] {
            assert!(
                maps.contains(&want),
                "Maps runs async ops but does not offer {want}: {maps:?}"
            );
        }
        // The same contract RestClient states, held side by side so the two
        // cannot drift apart again.
        let rest = ControlType::RestClient.supported_events();
        for want in ["onComplete", "onError", "onTimeout", "onCancelled"] {
            assert!(
                rest.contains(&want) && maps.contains(&want),
                "async event {want} must be offered by BOTH RestClient and Maps"
            );
        }
    }

    // ── Spec 039 T14: WebSearch scaffolding ─────────────────────────────────

    #[test]
    fn web_search_round_trips_through_as_str_and_from_str() {
        assert_eq!(ControlType::WebSearch.as_str(), "WebSearch");
        assert_eq!(ControlType::from_str("WebSearch"), ControlType::WebSearch);
    }

    #[test]
    fn web_search_is_non_visual_like_rest_client() {
        assert!(ControlType::WebSearch.is_non_visual());
    }

    #[test]
    fn web_search_default_properties_carry_no_api_key() {
        let c = Control::new("SEARCH-1", ControlType::WebSearch, 0, 0);
        assert!(c.get_prop("SearchEngineId").unwrap().as_str().is_empty());
        assert!(c.get_prop("Query").unwrap().as_str().is_empty());
        assert_eq!(c.get_prop("NumResults").unwrap().as_i64(), 10);
        assert_eq!(c.get_prop("SafeSearch").unwrap().as_str(), "Off");
        assert_eq!(c.get_prop("Mode").unwrap().as_str(), "Async");
        assert!(!c.get_prop("Busy").unwrap().as_bool());
        assert_eq!(c.get_prop("TimeoutMs").unwrap().as_i64(), 30000);
        // R30/R31: no key property here at all — resolved runtime-only,
        // same discipline as Maps's ApiKeySource.
        assert!(c.get_prop("ApiKeySource").is_none());
    }

    #[test]
    fn web_search_default_size_primary_and_supported_events() {
        assert_eq!(ControlType::WebSearch.default_size(), (56, 56));
        assert_eq!(ControlType::WebSearch.primary_event(), "onResultsReceived");
        let events = ControlType::WebSearch.supported_events();
        for want in [
            "onResultsReceived",
            "onError",
            "onTimeout",
            "onComplete",
            "onCancelled",
        ] {
            assert!(events.contains(&want), "WebSearch missing {want}: {events:?}");
        }
    }
}

#[cfg(test)]
mod default_legibility_tests {
    use super::*;

    /// A brand-new form and a brand-new label must be legible TOGETHER.
    ///
    /// They were not: the form defaulted to `00000000` (fully transparent) and
    /// every control to white text, so a new form's readability was decided by
    /// the user's desktop. The same binary read fine launched over a dark
    /// wallpaper and blank launched over the IDE, flipping between "works" and
    /// "broken" with no code change — and looking for all the world like the
    /// COBOL failing to set the value.
    #[test]
    fn a_new_form_and_a_new_label_are_legible_together() {
        let bg = crate::paint::parse_color(DEFAULT_FORM_BACKGROUND_COLOR);
        assert_eq!(bg.a(), 255, "a new form must be opaque, not see-through");

        let fg = crate::paint::parse_color(DEFAULT_FOREGROUND_COLOR);
        let ratio = crate::paint::contrast_ratio(fg, bg);
        assert!(
            ratio >= 4.5,
            "default text on a default form must clear AA, got {ratio:.2}:1"
        );
    }

    /// The default reaches a form actually created through `Form::new`.
    #[test]
    fn form_new_uses_the_opaque_default() {
        let f = Form::new("MAIN-FORM", "Demo", 640, 480);
        assert_eq!(f.background_color, DEFAULT_FORM_BACKGROUND_COLOR);
        assert_ne!(
            f.background_color, "00000000",
            "the transparent default is what made legibility depend on the desktop"
        );
    }
}
