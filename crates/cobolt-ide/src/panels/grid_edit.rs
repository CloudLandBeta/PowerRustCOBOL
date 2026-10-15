// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 094 R3–R6 — the Form Designer's **DataGrid edit mode**.
//!
//! The canvas draws every control as a static face — and the static face the
//! shared painter has for a DataGrid is only the label `⊞ DataGrid (N cols)`; the
//! header, the filter wells and the rows exist only in the interactive renderer.
//! So the canvas draws **every** DataGrid through that renderer
//! (`cobolt_forms::render::render_form_scoped`, `RenderMode::Interactive`) in a
//! child `Ui` at the grid's rect ([`draw_grid`]): display-only for all of them
//! (a disabled `Ui`, so the Designer's own selection, move and resize work over
//! it), and live for the one in edit mode — so the in-cell editor, the
//! column-edge resize and the reorder arrows that run are the grid's own (R6),
//! not a second copy of them — and what the renderer reports back
//! (`prop_updates`) becomes the Designer's ordinary, undoable property changes
//! (R4).
//!
//! This module is the part of that which needs no window: the render-only copy of
//! the grid, the property edits (add or remove a column or a row, rename or move
//! a column) written as pure functions over the control, and the header geometry
//! the two gestures the renderer does not have (double-click a title to rename
//! it, drag a title to reorder) are hit-tested with. `panels/designer.rs` hosts
//! it on the canvas.

use cobolt_forms::datagrid::{
    resolve_column_widths, DataGridCellSelection, DataGridColumnMeasure, DataGridLayout,
    DataGridLayoutInput, MIN_COLUMN_WIDTH,
};
use cobolt_forms::model::{
    DataGridAdvanced, DataGridColumn, DataGridWidthUnit, PropValue, DATAGRID_ADVANCED_PROP,
};
use cobolt_forms::Control;
use egui::{pos2, vec2, Rect};

/// The egui id scope the edited grid is rendered in. A constant, because the
/// renderer keys focus, scroll and selection by widget id between frames; it is
/// also what [`selected_cell`] reads the grid's selection back through.
pub(crate) fn scope() -> egui::Id {
    egui::Id::new("designer-grid-edit")
}

/// How far outside the grid the mode's frame reaches. The frame is drawn there,
/// not over the grid, so a double-click on it can mean "leave" without taking a
/// cell's double-click, and a press outside it can mean "leave" too.
pub(crate) const FRAME_BAND: f32 = 8.0;

/// A press this far from where it started is a drag, not a click.
pub(crate) const DRAG_SLOP: f32 = 5.0;

/// The properties a canvas edit may write back to the form. The renderer also
/// reports run-time state (`ClickedRow`, `EditedValue`, `_RuntimeColumnFilters`,
/// …); none of that belongs in a saved form, so only the grid's own design
/// properties pass.
pub(crate) fn accepts(key: &str) -> bool {
    matches!(
        key,
        "Rows" | DATAGRID_ADVANCED_PROP | "RowHeight" | "RowHeightOverrides" | "ColumnFilters"
    )
}

/// What the Designer does when the toolbar (or a test) asks for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GridAction {
    AddColumn,
    RemoveColumn,
    AddRow,
    RemoveRow,
    /// *Reset sample data* — only a bound grid has a binding to seed from.
    ResetSample,
    Done,
}

/// The title being renamed on the canvas (R3).
#[derive(Clone, Debug)]
pub(crate) struct Rename {
    pub display_index: usize,
    pub text: String,
    /// Focus the box on the frame it first appears.
    pub fresh: bool,
}

/// A title being dragged to a new place in the column order (R3).
#[derive(Clone, Copy, Debug)]
pub(crate) struct TitleDrag {
    pub from: usize,
    pub start: egui::Pos2,
    /// Past [`DRAG_SLOP`]: it is a drag now, and release moves the column.
    pub moving: bool,
}

/// The last property write of a drag, kept so the rest of the drag amends it
/// instead of adding a step to the undo stack for every frame (one undo step
/// per gesture).
#[derive(Clone, Debug)]
pub(crate) struct Gesture {
    pub id: String,
    pub key: String,
    /// The undo stack's length right after the write; if anything else has been
    /// pushed since, the gesture is over.
    pub undo_len: usize,
    pub tick: u64,
}

/// Grid edit mode as the Designer holds it. View state only: never saved, never
/// an undo step by itself.
#[derive(Clone, Debug)]
pub(crate) struct GridEditState {
    /// The grid being edited.
    pub id: String,
    /// Where the grid was on screen last frame — what the frame band, "a press
    /// outside" and the header gestures are measured against.
    pub screen: Option<Rect>,
    /// Where the floating toolbar was last frame.
    pub toolbar: Option<Rect>,
    pub rename: Option<Rename>,
    pub drag: Option<TitleDrag>,
    pub gesture: Option<Gesture>,
    /// Counts frames the mode has been drawn; a gesture is a run of consecutive ones.
    pub tick: u64,
    /// Whether a text box had the keyboard at the start of this frame — then Esc
    /// is the text box's (cancel the cell edit), and does not leave the mode.
    pub text_editing: bool,
    /// The tick at which the mode ends (the tick counts up as the frame draws,
    /// so `tick + 2` is the end of the NEXT frame). It ends a frame AFTER the
    /// press that ends it: egui takes the keyboard focus from a text box one frame after
    /// a press elsewhere, and the grid keeps a cell being typed into only when
    /// it sees that (`lost_focus`). Ending at once threw the typing away.
    pub leave_at: Option<u64>,
}

impl GridEditState {
    /// Start ending the mode (once; a second call changes nothing).
    pub fn begin_leaving(&mut self) {
        self.leave_at.get_or_insert(self.tick + 2);
    }

    /// Whether the mode's time is up.
    pub fn is_over(&self) -> bool {
        self.leave_at.is_some_and(|t| self.tick >= t)
    }
}

impl GridEditState {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            screen: None,
            toolbar: None,
            rename: None,
            drag: None,
            gesture: None,
            tick: 0,
            text_editing: false,
            leave_at: None,
        }
    }
}

// ── The render-only copy ────────────────────────────────────────────────────

/// The grid as the interactive renderer is given it in edit mode: placed at the
/// origin of a surface of its own size, enabled and visible, with in-cell editing
/// forced on and sorting off (a header click must not reorder what is being
/// edited, and the title is a double-click target here). Nothing of it is ever
/// written back — it is a copy for drawing.
///
/// `container_clip` is the `_ContainerClip` descriptor of a rounded parent (see
/// [`container_clip_for`]), so the grid's corners follow the parent's arc as the
/// static face's do.
pub(crate) fn render_copy(grid: &Control, size: egui::Vec2, container_clip: Option<String>) -> Control {
    let mut copy = grid.clone();
    copy.rect = cobolt_forms::model::Rect::new(0, 0, size.x.round() as i32, size.y.round() as i32);
    copy.parent = None;
    copy.layer = None;
    copy.visible = true;
    copy.enabled = true;
    copy.animations.clear();
    copy.set_prop("AllowCellEditing", PropValue::Bool(true));
    copy.set_prop("AllowSorting", PropValue::Bool(false));
    if let Some(clip) = container_clip {
        copy.set_prop("_ContainerClip", PropValue::String(clip));
    }
    copy
}

/// The `_ContainerClip` descriptor for a grid inside a rounded GroupBox or
/// Panel, in screen pixels: the parent's content rect, the concentric radius
/// (`cobolt_forms::render::container_clip_geometry`) and "all four corners may
/// round". `None` when the parent is not a rounded container. The format is the
/// renderer's own (nine comma-separated numbers); `paint::draw_control` only
/// lifts the corners the child actually reaches.
pub(crate) fn container_clip_for(parent: &Control, parent_screen: Rect) -> Option<String> {
    if !matches!(
        parent.control_type,
        cobolt_forms::ControlType::GroupBox | cobolt_forms::ControlType::Panel
    ) {
        return None;
    }
    let rad = cobolt_forms::paint::corner_radius(parent);
    if rad < 0.5 {
        return None;
    }
    let (content, rad) = cobolt_forms::render::container_clip_geometry(parent, parent_screen, rad);
    Some(format!(
        "{},{},{},{},{},1,1,1,1",
        content.min.x, content.min.y, content.max.x, content.max.y, rad
    ))
}

/// A property value in the type the control already holds it in: the renderer
/// reports every update as text, while `RowHeight` is stored as a number.
pub(crate) fn typed_like(old: Option<&PropValue>, text: String) -> PropValue {
    match old {
        Some(PropValue::Int(_)) => match text.trim().parse::<i64>() {
            Ok(n) => PropValue::Int(n),
            Err(_) => PropValue::String(text),
        },
        Some(PropValue::Bool(_)) => PropValue::Bool(PropValue::String(text).as_bool()),
        _ => PropValue::String(text),
    }
}

// ── What the grid's columns and rows are ────────────────────────────────────

fn text_of(grid: &Control, key: &str) -> String {
    grid.get_prop(key).map(|p| p.to_xml_string()).unwrap_or_default()
}

fn flag(grid: &Control, key: &str, default: bool) -> bool {
    grid.get_prop(key).map(PropValue::as_bool).unwrap_or(default)
}

/// The `Columns` property as the renderer reads it: `(name, type)` per non-blank
/// line. The index here is the cell's place in a row of `Rows`.
pub(crate) fn columns_of(grid: &Control) -> Vec<(String, String)> {
    text_of(grid, "Columns")
        .lines()
        .filter_map(|l| {
            let mut it = l.splitn(2, ':');
            let name = it.next().unwrap_or("").trim().to_owned();
            if name.is_empty() {
                return None;
            }
            let ty = it.next().unwrap_or("string").trim().to_lowercase();
            Some((name, ty))
        })
        .collect()
}

/// `Rows` as the renderer reads it: one `Vec` of cells per non-empty line.
pub(crate) fn rows_of(grid: &Control) -> Vec<Vec<String>> {
    text_of(grid, "Rows")
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.split('\t').map(str::to_owned).collect())
        .collect()
}

fn join_rows(rows: &[Vec<String>]) -> String {
    rows.iter().map(|r| r.join("\t")).collect::<Vec<_>>().join("\n")
}

/// One column as the grid shows it: the cell of a row it reads, and its title.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShownColumn {
    /// The cell of a `Rows` line this column shows; `None` for a column with no
    /// data behind it (the renderer cannot edit it either).
    pub source: Option<usize>,
    pub title: String,
}

/// The columns in the order they are drawn — the renderer's own rule: the
/// `AdvancedGrid` columns when it has any (each tied to a `Columns` line by its
/// source name, title or id), otherwise the `Columns` lines themselves.
pub(crate) fn shown_columns(grid: &Control) -> Vec<ShownColumn> {
    let cols = columns_of(grid);
    let advanced = DataGridAdvanced::from_control(grid);
    if advanced.columns.is_empty() {
        return cols
            .iter()
            .enumerate()
            .map(|(i, (name, _))| ShownColumn { source: Some(i), title: name.clone() })
            .collect();
    }
    advanced
        .columns
        .iter()
        .map(|c| ShownColumn {
            source: cols.iter().position(|(name, _)| {
                name.eq_ignore_ascii_case(&c.source_name)
                    || name.eq_ignore_ascii_case(&c.title)
                    || name.eq_ignore_ascii_case(&c.id)
            }),
            title: c.title.clone(),
        })
        .collect()
}

/// Whether `AdvancedGrid` describes the columns (so an edit must keep it in
/// step with `Columns`), as opposed to being absent or empty.
fn advanced_describes_columns(grid: &Control) -> bool {
    !DataGridAdvanced::from_control(grid).columns.is_empty()
        && !text_of(grid, DATAGRID_ADVANCED_PROP).trim().is_empty()
}

/// A cell of fake data for a new row or column.
pub(crate) fn sample_cell(ty: &str, row_no: usize) -> String {
    match ty {
        "number" | "integer" | "int" | "decimal" | "float" => "0".to_owned(),
        "datetime" | "date" => "2026-01-01".to_owned(),
        _ => format!("Text {row_no}"),
    }
}

fn normalized_id(name: &str) -> String {
    name.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_uppercase() } else { '_' })
        .collect()
}

type Edits = Vec<(String, PropValue)>;

fn set(edits: &mut Edits, grid: &Control, key: &str, value: String) {
    if text_of(grid, key) != value {
        edits.push((key.to_owned(), PropValue::String(value)));
    }
}

fn advanced_json(advanced: &DataGridAdvanced) -> Option<String> {
    advanced.to_json().ok()
}

// ── The edits ───────────────────────────────────────────────────────────────

/// A new column at the end: `Column N:string` in `Columns`, a column of the same
/// name in `AdvancedGrid` when that describes the columns, and a sample cell in
/// every row.
pub(crate) fn add_column(grid: &Control) -> Edits {
    let cols = columns_of(grid);
    let advanced = DataGridAdvanced::from_control(grid);
    let taken = |name: &str| {
        cols.iter().any(|(n, _)| n.eq_ignore_ascii_case(name))
            || advanced.columns.iter().any(|c| {
                c.title.eq_ignore_ascii_case(name)
                    || c.source_name.eq_ignore_ascii_case(name)
                    || c.id.eq_ignore_ascii_case(&normalized_id(name))
            })
    };
    let name = (cols.len() + 1..)
        .map(|n| format!("Column {n}"))
        .find(|n| !taken(n))
        .unwrap_or_else(|| "Column".to_owned());

    let mut edits = Edits::new();
    let columns = text_of(grid, "Columns");
    let columns = if columns.trim().is_empty() {
        format!("{name}:string")
    } else {
        format!("{}\n{name}:string", columns.trim_end_matches(['\n', '\r']))
    };
    set(&mut edits, grid, "Columns", columns);

    if advanced_describes_columns(grid) {
        let mut next = advanced.clone();
        next.columns.push(DataGridColumn {
            id: normalized_id(&name),
            title: name.clone(),
            source_name: name.clone(),
            value_type: "string".to_owned(),
            ..DataGridColumn::default()
        });
        if let Some(json) = advanced_json(&next) {
            set(&mut edits, grid, DATAGRID_ADVANCED_PROP, json);
        }
    }

    let mut rows = rows_of(grid);
    if !rows.is_empty() {
        for (i, row) in rows.iter_mut().enumerate() {
            if row.len() < cols.len() {
                row.resize(cols.len(), String::new());
            }
            row.push(sample_cell("string", i + 1));
        }
        set(&mut edits, grid, "Rows", join_rows(&rows));
    }
    edits
}

/// Remove the column drawn at `display_index`: its `Columns` line and its cell
/// in every row (when it has data behind it), and its `AdvancedGrid` column.
pub(crate) fn remove_column(grid: &Control, display_index: usize) -> Edits {
    let shown = shown_columns(grid);
    let Some(column) = shown.get(display_index) else {
        return Edits::new();
    };
    let mut edits = Edits::new();

    if advanced_describes_columns(grid) {
        let mut next = DataGridAdvanced::from_control(grid);
        if display_index < next.columns.len() {
            next.columns.remove(display_index);
            next.frozen_columns = next.frozen_columns.min(next.columns.len());
        }
        if let Some(json) = advanced_json(&next) {
            set(&mut edits, grid, DATAGRID_ADVANCED_PROP, json);
        }
    }

    if let Some(source) = column.source {
        let kept: Vec<String> = {
            let mut seen = 0usize;
            text_of(grid, "Columns")
                .lines()
                .filter(|l| {
                    let named = !l.splitn(2, ':').next().unwrap_or("").trim().is_empty();
                    let this = named && seen == source;
                    if named {
                        seen += 1;
                    }
                    !this
                })
                .map(str::to_owned)
                .collect()
        };
        set(&mut edits, grid, "Columns", kept.join("\n"));
        let mut rows = rows_of(grid);
        for row in &mut rows {
            if source < row.len() {
                row.remove(source);
            }
        }
        if !rows.is_empty() {
            set(&mut edits, grid, "Rows", join_rows(&rows));
        }
    }
    edits
}

/// A new row at the end, filled with fake data of each column's type.
pub(crate) fn add_row(grid: &Control) -> Edits {
    let cols = columns_of(grid);
    if cols.is_empty() {
        return Edits::new();
    }
    let rows = rows_of(grid);
    let n = rows.len() + 1;
    let row: Vec<String> = cols.iter().map(|(_, ty)| sample_cell(ty, n)).collect();
    let mut all = rows;
    all.push(row);
    let mut edits = Edits::new();
    set(&mut edits, grid, "Rows", join_rows(&all));
    edits
}

/// Remove the data row `data_row` (0-based, an index into `Rows`), and shift the
/// per-row heights of the rows after it.
pub(crate) fn remove_row(grid: &Control, data_row: usize) -> Edits {
    let mut rows = rows_of(grid);
    if data_row >= rows.len() {
        return Edits::new();
    }
    rows.remove(data_row);
    let mut edits = Edits::new();
    set(&mut edits, grid, "Rows", join_rows(&rows));

    let overrides = cobolt_forms::datagrid::parse_row_height_overrides(&text_of(
        grid,
        cobolt_forms::datagrid::ROW_HEIGHT_OVERRIDES_PROP,
    ));
    if !overrides.is_empty() {
        let shifted: std::collections::BTreeMap<usize, f32> = overrides
            .into_iter()
            .filter(|(r, _)| *r != data_row)
            .map(|(r, h)| (if r > data_row { r - 1 } else { r }, h))
            .collect();
        set(
            &mut edits,
            grid,
            cobolt_forms::datagrid::ROW_HEIGHT_OVERRIDES_PROP,
            cobolt_forms::datagrid::format_row_height_overrides(&shifted),
        );
    }
    edits
}

/// Rename the column drawn at `display_index`. The title lives in `AdvancedGrid`;
/// the grid finds a column's data by matching a `Columns` name against the
/// column's source name, title or id, so when the new title would break that
/// match (the `Columns` name WAS the old title) the `Columns` line follows the
/// rename — a renamed column keeps its data.
pub(crate) fn rename_column(grid: &Control, display_index: usize, title: &str) -> Edits {
    let title: String = title
        .chars()
        .map(|c| if matches!(c, '\t' | '\n' | '\r') { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_owned();
    let shown = shown_columns(grid);
    let Some(old) = shown.get(display_index) else {
        return Edits::new();
    };
    if title.is_empty() || title == old.title {
        return Edits::new();
    }
    let mut advanced = DataGridAdvanced::from_control(grid);
    let Some(column) = advanced.columns.get_mut(display_index) else {
        return Edits::new();
    };
    column.title = title.clone();
    let renamed = column.clone();

    let mut edits = Edits::new();
    if let Some(json) = advanced_json(&advanced) {
        set(&mut edits, grid, DATAGRID_ADVANCED_PROP, json);
    }

    if let Some(source) = old.source {
        let cols = columns_of(grid);
        let still_found = cols
            .iter()
            .position(|(name, _)| {
                name.eq_ignore_ascii_case(&renamed.source_name)
                    || name.eq_ignore_ascii_case(&renamed.title)
                    || name.eq_ignore_ascii_case(&renamed.id)
            })
            == Some(source);
        if !still_found {
            // The line's name was the old title. A colon cannot sit in it, so a
            // title that has one falls back on the column's source name.
            let name = if title.contains(':') { renamed.source_name.clone() } else { title.clone() };
            let mut seen = 0usize;
            let lines: Vec<String> = text_of(grid, "Columns")
                .lines()
                .map(|l| {
                    let mut parts = l.splitn(2, ':');
                    let named = !parts.next().unwrap_or("").trim().is_empty();
                    let ty = parts.next().map(str::trim).unwrap_or("string").to_owned();
                    let this = named && seen == source;
                    if named {
                        seen += 1;
                    }
                    if this {
                        format!("{name}:{ty}")
                    } else {
                        l.to_owned()
                    }
                })
                .collect();
            set(&mut edits, grid, "Columns", lines.join("\n"));
        }
    }
    edits
}

/// Move the column drawn at `from` to where `to` is drawn.
pub(crate) fn move_column(grid: &Control, from: usize, to: usize) -> Edits {
    let mut advanced = DataGridAdvanced::from_control(grid);
    let mut edits = Edits::new();
    if advanced.move_column(from, to) {
        if let Some(json) = advanced_json(&advanced) {
            set(&mut edits, grid, DATAGRID_ADVANCED_PROP, json);
        }
    }
    edits
}

// ── The selection ───────────────────────────────────────────────────────────

/// The cell the developer last selected in the edited grid — read back through
/// the renderer's own widget id for it, so the toolbar can remove "this" column
/// or row. `None` before any click.
pub(crate) fn selected_cell(ctx: &egui::Context, grid_id: &str) -> Option<DataGridCellSelection> {
    let id = cobolt_forms::render::control_widget_id(Some(scope()), grid_id).with("datagrid-selection");
    ctx.memory(|m| m.data.get_temp::<DataGridCellSelection>(id))
}

/// The grid's horizontal scroll, read back the same way.
fn scroll_x(ctx: &egui::Context, grid_id: &str) -> f32 {
    let id = cobolt_forms::render::control_widget_id(Some(scope()), grid_id).with("datagrid-scroll-x");
    ctx.memory(|m| m.data.get_temp::<f32>(id)).unwrap_or(0.0)
}

// ── Header geometry ─────────────────────────────────────────────────────────

/// One column's header cell on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HeaderCell {
    pub display_index: usize,
    pub rect: Rect,
}

/// Where each column's title cell is, for a grid drawn into `screen` — the
/// renderer's own arithmetic (gutter, caption row, resolved widths, frozen
/// columns, horizontal scroll) through the same `DataGridLayout`, because the
/// renderer reports none of it. Columns scrolled out of sight are left out.
pub(crate) fn header_cells(grid: &Control, screen: Rect, scroll_x: f32) -> Vec<HeaderCell> {
    let advanced = DataGridAdvanced::from_control(grid);
    let cols = columns_of(grid);
    let ncols = if advanced.columns.is_empty() { cols.len() } else { advanced.columns.len() }.max(1);
    let row_h = text_of(grid, "RowHeight").parse::<f32>().unwrap_or(22.0).clamp(14.0, 120.0);
    let show_filters = flag(grid, "ShowColumnFilters", false);
    let header_h = if show_filters { (row_h * 1.85).max(row_h + 18.0) } else { row_h };
    let font_size = cobolt_forms::layout::fonts::resolve_font_size(grid, 12.0, 6.0, 72.0);

    // The caption row above the titles: a Title and/or the CSV badge, when the
    // grid is tall enough to spare it.
    let title = text_of(grid, "Title");
    let show_csv = flag(grid, "ShowCSVExportButton", false) && flag(grid, "ExportCSV", true);
    let title_font = (font_size + 2.0).clamp(10.0, 22.0);
    let caption_want = (title_font + 12.0).max(24.0).max(cobolt_forms::paint::corner_radius(grid));
    let caption_h = if (show_csv || !title.trim().is_empty())
        && screen.height() >= caption_want + header_h + row_h
    {
        caption_want
    } else {
        0.0
    };

    // The row-number gutter takes its width from how many rows the grid HAS — not from how many a
    // filter leaves showing, or the columns would slide when someone types (the renderer's rule).
    let gutter = if flag(grid, "ShowRowNumbers", false) {
        let rows = rows_of(grid);
        let source_names: Vec<String> = cols.iter().map(|(n, _)| n.clone()).collect();
        let shown_rows = advanced.filtered_row_indices_for_sources(&rows, &source_names).len();
        let digits = rows.len().max(shown_rows).max(1).to_string().len().max(2) as f32;
        (digits * font_size * 0.62 + 12.0).min(screen.width() * 0.5)
    } else {
        0.0
    };
    let cx = screen.min.x + gutter;
    let cw = (screen.width() - gutter).max(1.0);
    let col_w = cw / ncols as f32;
    let declared: Vec<(f32, DataGridWidthUnit)> = (0..ncols)
        .map(|i| match advanced.columns.get(i) {
            Some(c) => (c.width.max(1.0), c.width_unit),
            None => (col_w, DataGridWidthUnit::Points),
        })
        .collect();
    let resolved = resolve_column_widths(&declared, cw, flag(grid, "AutoFitColumns", false));
    let measures: Vec<DataGridColumnMeasure> = (0..ncols)
        .map(|i| DataGridColumnMeasure {
            width: resolved.get(i).copied().unwrap_or(col_w).max(MIN_COLUMN_WIDTH),
            frozen: advanced.columns.get(i).map(|c| c.frozen).unwrap_or(false),
        })
        .collect();
    let band_h = caption_h + header_h;
    let layout = DataGridLayout::compute(&DataGridLayoutInput {
        width: cw,
        height: screen.height().max(band_h),
        row_count: 0,
        columns: measures,
        row_height: row_h,
        row_heights: Vec::new(),
        header_height: band_h,
        frozen_columns: advanced.frozen_columns.min(ncols),
        frozen_rows: 0,
        scroll_x,
        scroll_y: 0.0,
        row_buffer: 2,
    });
    let top = screen.min.y + caption_h;
    layout
        .frozen_columns
        .iter()
        .chain(layout.scrollable_columns.iter())
        .filter_map(|col| {
            let cell = Rect::from_min_size(pos2(cx + col.x, top), vec2(col.width, header_h));
            // A scrolled column keeps to the viewport right of the frozen ones.
            let visible = if col.frozen {
                cell
            } else {
                cell.intersect(Rect::from_min_max(
                    pos2(cx + layout.frozen_columns_width, top),
                    pos2(screen.max.x, top + header_h),
                ))
            };
            (visible.width() > 1.0 && visible.height() > 1.0)
                .then_some(HeaderCell { display_index: col.index, rect: visible.intersect(screen) })
        })
        .collect()
}

/// [`header_cells`] with the grid's horizontal scroll read from the renderer.
pub(crate) fn header_cells_live(ctx: &egui::Context, grid: &Control, screen: Rect) -> Vec<HeaderCell> {
    header_cells(grid, screen, scroll_x(ctx, &grid.id))
}

/// The part of a header cell a title gesture belongs to: not the right-hand
/// strip the reorder arrows sit in, not the column-edge resize handle, and not
/// the filter box under the title. What is left is where a double-click renames
/// and a press-and-drag reorders; the arrows, the edge and the filter keep their
/// own gestures.
pub(crate) fn title_zone(grid: &Control, cell: Rect, ncols: usize) -> Rect {
    let row_h = text_of(grid, "RowHeight").parse::<f32>().unwrap_or(22.0).clamp(14.0, 120.0);
    let arrows = ncols > 1 && flag(grid, "AllowColumnReorder", true) && cell.width() >= 58.0;
    let right = if arrows { 38.0 } else { 5.0 };
    let bottom = if flag(grid, "ShowColumnFilters", false) { cell.min.y + row_h } else { cell.max.y };
    let zone = Rect::from_min_max(
        pos2(cell.min.x + 2.0, cell.min.y),
        pos2((cell.max.x - right).max(cell.min.x + 2.0), bottom.min(cell.max.y)),
    );
    if zone.width() < 4.0 {
        Rect::NOTHING
    } else {
        zone
    }
}

/// Which column's header cell holds `pos` in its title zone, if any.
pub(crate) fn title_at(grid: &Control, cells: &[HeaderCell], pos: egui::Pos2) -> Option<usize> {
    let ncols = cells.iter().map(|c| c.display_index + 1).max().unwrap_or(0);
    cells
        .iter()
        .find(|c| title_zone(grid, c.rect, ncols).contains(pos))
        .map(|c| c.display_index)
}

/// Where a title dropped at `pos` lands: the column whose header cell holds it,
/// or the first/last when the pointer is left/right of them all.
pub(crate) fn drop_target(cells: &[HeaderCell], pos: egui::Pos2) -> Option<usize> {
    let first = cells.first()?;
    let last = cells.last()?;
    if pos.x < first.rect.min.x {
        return Some(first.display_index);
    }
    if pos.x >= last.rect.max.x {
        return Some(last.display_index);
    }
    cells.iter().find(|c| pos.x >= c.rect.min.x && pos.x < c.rect.max.x).map(|c| c.display_index)
}

// ── Toolbar ─────────────────────────────────────────────────────────────────

/// How the toolbar's buttons are laid out: six squares of this size and gap.
pub(crate) const TOOL_SIZE: f32 = 26.0;
pub(crate) const TOOL_GAP: f32 = 4.0;
pub(crate) const TOOL_PAD: f32 = 5.0;

/// The toolbar's fixed size — six buttons in a row, a framed strip.
pub(crate) fn toolbar_size() -> egui::Vec2 {
    let n = 6.0;
    vec2(TOOL_PAD * 2.0 + n * TOOL_SIZE + (n - 1.0) * TOOL_GAP, TOOL_PAD * 2.0 + TOOL_SIZE)
}

/// Where the toolbar goes: above the grid's frame band when there is room
/// inside the canvas, else below it; left-aligned with the grid and clamped to
/// the canvas, so it never covers the grid and never leaves the canvas.
pub(crate) fn toolbar_pos(grid: Rect, canvas: Rect) -> egui::Pos2 {
    let size = toolbar_size();
    let above = grid.min.y - FRAME_BAND - 4.0 - size.y;
    let y = if above >= canvas.min.y { above } else { grid.max.y + FRAME_BAND + 4.0 };
    let x = grid.min.x.min(canvas.max.x - size.x).max(canvas.min.x);
    pos2(x, y)
}

/// Forget the grid's selected cell (after the row or column it named is gone).
pub(crate) fn clear_selection(ctx: &egui::Context, grid_id: &str) {
    let id = cobolt_forms::render::control_widget_id(Some(scope()), grid_id).with("datagrid-selection");
    ctx.memory_mut(|m| m.data.remove::<DataGridCellSelection>(id));
}

/// What a button of the toolbar paints, in order left to right.
const TOOLS: [GridAction; 6] = [
    GridAction::AddColumn,
    GridAction::RemoveColumn,
    GridAction::AddRow,
    GridAction::RemoveRow,
    GridAction::ResetSample,
    GridAction::Done,
];

/// Which buttons apply to the grid as it stands — a button that does not is
/// drawn disabled, never hidden (the toolbar keeps its shape).
pub(crate) fn enabled(grid: &Control, bound: bool, action: GridAction) -> bool {
    match action {
        GridAction::AddColumn | GridAction::Done => true,
        GridAction::RemoveColumn => !shown_columns(grid).is_empty(),
        GridAction::AddRow => !columns_of(grid).is_empty(),
        GridAction::RemoveRow => !rows_of(grid).is_empty(),
        GridAction::ResetSample => bound,
    }
}

fn tip<'a>(tr: &'a crate::i18n::Tr, action: GridAction) -> &'a str {
    match action {
        GridAction::AddColumn => tr.dg_edit_add_column,
        GridAction::RemoveColumn => tr.dg_edit_remove_column,
        GridAction::AddRow => tr.dg_edit_add_row,
        GridAction::RemoveRow => tr.dg_edit_remove_row,
        GridAction::ResetSample => tr.dg_edit_reset_sample,
        GridAction::Done => tr.dg_edit_done,
    }
}

/// The toolbar's icon for `action`, drawn as plain shapes inside `r` (no glyph
/// font involved): a column or a row of bars with a plus or a minus, a circular
/// arrow, a check.
fn paint_icon(p: &egui::Painter, r: Rect, action: GridAction, ink: egui::Color32) {
    let stroke = egui::Stroke::new(1.6, ink);
    let c = r.center();
    let bars = |vertical: bool| {
        for k in -1..=1 {
            let off = k as f32 * 5.0;
            let (a, b) = if vertical {
                (pos2(c.x + off, c.y - 8.0), pos2(c.x + off, c.y + 5.0))
            } else {
                (pos2(c.x - 8.0, c.y + off - 1.5), pos2(c.x + 5.0, c.y + off - 1.5))
            };
            p.line_segment([a, b], egui::Stroke::new(2.4, ink.gamma_multiply(0.7)));
        }
    };
    let badge = |plus: bool| {
        let o = pos2(r.max.x - 6.5, r.max.y - 6.5);
        p.circle_filled(o, 5.0, ink);
        let dark = egui::Color32::from_rgb(20, 24, 40);
        p.line_segment([o - vec2(2.6, 0.0), o + vec2(2.6, 0.0)], egui::Stroke::new(1.4, dark));
        if plus {
            p.line_segment([o - vec2(0.0, 2.6), o + vec2(0.0, 2.6)], egui::Stroke::new(1.4, dark));
        }
    };
    match action {
        GridAction::AddColumn | GridAction::RemoveColumn => {
            bars(true);
            badge(action == GridAction::AddColumn);
        }
        GridAction::AddRow | GridAction::RemoveRow => {
            bars(false);
            badge(action == GridAction::AddRow);
        }
        GridAction::ResetSample => {
            let radius = 7.0;
            let pts: Vec<egui::Pos2> = (0..=18)
                .map(|i| {
                    let a = -0.6 + i as f32 * (std::f32::consts::TAU * 0.78 / 18.0);
                    c + vec2(a.cos(), a.sin()) * radius
                })
                .collect();
            let tip = *pts.last().unwrap_or(&c);
            p.add(egui::Shape::line(pts, stroke));
            p.line_segment([tip, tip + vec2(-4.0, -1.5)], stroke);
            p.line_segment([tip, tip + vec2(0.5, -4.5)], stroke);
        }
        GridAction::Done => {
            p.add(egui::Shape::line(
                vec![c + vec2(-6.0, 0.5), c + vec2(-2.0, 4.5), c + vec2(6.5, -5.0)],
                egui::Stroke::new(2.0, ink),
            ));
        }
    }
}

/// The floating toolbar: six icon-only buttons in a framed strip, each with a
/// translated tooltip and disabled — not hidden — when it does not apply. It is
/// a foreground `Area` at a fixed position and of a fixed size, so nothing it
/// holds can resize it. Returns the action pressed this frame and the strip's rect.
pub(crate) fn show_toolbar(
    ctx: &egui::Context,
    id: egui::Id,
    pos: egui::Pos2,
    grid: &Control,
    bound: bool,
    tr: &crate::i18n::Tr,
    accent: egui::Color32,
) -> (Option<GridAction>, Rect) {
    let size = toolbar_size();
    let mut pressed = None;
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .movable(false)
        .show(ctx, |ui| {
            let strip = Rect::from_min_size(pos, size);
            let (_, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            let p = ui.painter();
            p.rect_filled(strip, 7.0, egui::Color32::from_rgba_unmultiplied(22, 26, 46, 238));
            p.rect_stroke(strip, 7.0, egui::Stroke::new(1.0, accent), egui::StrokeKind::Inside);
            for (i, action) in TOOLS.iter().copied().enumerate() {
                let x = strip.min.x + TOOL_PAD + i as f32 * (TOOL_SIZE + TOOL_GAP);
                let r = Rect::from_min_size(pos2(x, strip.min.y + TOOL_PAD), vec2(TOOL_SIZE, TOOL_SIZE));
                let on = enabled(grid, bound, action);
                let resp = ui.interact(
                    r,
                    id.with(("tool", i)),
                    if on { egui::Sense::click() } else { egui::Sense::hover() },
                );
                if on && resp.hovered() {
                    p.rect_filled(r, 5.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 34));
                    ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                let ink = if on {
                    egui::Color32::from_gray(238)
                } else {
                    egui::Color32::from_gray(110)
                };
                paint_icon(p, r, action, ink);
                resp.clone().on_hover_text(tip(tr, action));
                if on && resp.clicked() {
                    pressed = Some(action);
                }
            }
        });
    (pressed, Rect::from_min_size(pos, size))
}

/// A `FormState` that draws everything the state it wraps draws except the
/// controls in `hide` — the DataGrids, which the canvas draws itself
/// ([`draw_grid`]) because the static face the shared painter has for a grid is
/// a placeholder label, not the grid.
pub(crate) struct Without<'a> {
    pub inner: &'a dyn cobolt_forms::render::FormState,
    pub hide: &'a std::collections::HashSet<String>,
}

impl cobolt_forms::render::FormState for Without<'_> {
    fn run_id(&self) -> u64 {
        self.inner.run_id()
    }
    fn live(&self, base: &Control) -> Control {
        self.inner.live(base)
    }
    fn visible(&self, base: &Control) -> bool {
        !self.hide.contains(&base.id) && self.inner.visible(base)
    }
    fn enabled(&self, base: &Control) -> bool {
        self.inner.enabled(base)
    }
    fn layer_visible(&self, name: &str) -> bool {
        self.inner.layer_visible(name)
    }
    fn transform(&self, base: &Control) -> cobolt_forms::render::RenderTransform {
        self.inner.transform(base)
    }
    fn decimal_comma(&self) -> bool {
        self.inner.decimal_comma()
    }
    fn currency(&self) -> char {
        self.inner.currency()
    }
    fn viewer_document(
        &self,
        base: &Control,
        source: &str,
    ) -> Option<std::sync::Arc<cobolt_forms::paint::ViewerDocument>> {
        self.inner.viewer_document(base, source)
    }
}

// ── Drawing the grids on the canvas ─────────────────────────────────────────

/// One DataGrid on the canvas this frame, as owned data: the control list the
/// faces were drawn from stays borrowed while the grids are drawn.
#[derive(Clone, Debug)]
pub(crate) struct GridFrame {
    /// The grid as the form holds it (design properties, not the laid-out copy).
    pub grid: Control,
    /// Where it is on screen.
    pub screen: Rect,
    /// The part of the canvas its containers let it show; `None` for a grid
    /// that sits on the form itself.
    pub clip: Option<Rect>,
    /// `_ContainerClip` of a rounded parent, so the corners follow its arc.
    pub container_clip: Option<String>,
}

/// The egui id scope of a grid drawn for viewing. Apart from [`scope`], so the
/// grid being edited and a grid only looked at never share state.
pub(crate) fn view_scope() -> egui::Id {
    egui::Id::new("designer-grid-view")
}

/// The grid as the canvas shows it when it is not being edited: the same
/// copy-for-drawing as [`render_copy`], with the grid's own settings left as
/// they are.
pub(crate) fn view_copy(grid: &Control, size: egui::Vec2, container_clip: Option<String>) -> Control {
    let mut copy = grid.clone();
    copy.rect = cobolt_forms::model::Rect::new(0, 0, size.x.round() as i32, size.y.round() as i32);
    copy.parent = None;
    copy.layer = None;
    copy.visible = true;
    copy.enabled = true;
    copy.animations.clear();
    if let Some(clip) = container_clip {
        copy.set_prop("_ContainerClip", PropValue::String(clip));
    }
    copy
}

/// Run `draw` with the frame's input queue and scroll delta taken away, and
/// give them back after: a grid drawn only for looking must not read a key, a
/// wheel notch or a paste meant for the canvas (the interactive renderer reads
/// them — and consumes the wheel and the arrows — whenever the pointer is over
/// it).
pub(crate) fn without_input<R>(ctx: &egui::Context, draw: impl FnOnce() -> R) -> R {
    let (events, scroll) =
        ctx.input_mut(|i| (std::mem::take(&mut i.events), std::mem::take(&mut i.smooth_scroll_delta)));
    let out = draw();
    ctx.input_mut(|i| {
        i.events = events;
        i.smooth_scroll_delta = scroll;
    });
    out
}

/// Draw one DataGrid of the form on the canvas, through the shared interactive renderer
/// in a child `Ui` at its rect — the picture Run Form gives (header band, filter wells,
/// row-number gutter, the grid's own `Rows`, column colours, separators, rounded
/// corners), because the static face the canvas has for a grid is only a label. The
/// canvas calls this from the engine's paint order (`render_faces_with`), so the grid
/// is where its layer and z-order put it, under what is above it and over what is
/// below.
///
/// The grid in `editing` is drawn live and its `prop_updates` come back (R3,
/// R6). Every other grid is drawn **display-only**: its `Ui` is disabled — so
/// none of its widgets (cells, header, filter wells, resize edges, scrollbar,
/// focus) senses a click or a drag, and the canvas's own selection, move,
/// resize and rubber band work over it as over any other control — at full
/// opacity (`disable` fades the painter), and with the input queue taken away
/// ([`without_input`]).
pub(crate) fn draw_grid(
    ui: &mut egui::Ui,
    f: &GridFrame,
    editing: Option<&str>,
    glass: bool,
    canvas: Rect,
    behind: egui::Color32,
) -> Vec<(String, String, String)> {
    let ctx = ui.ctx().clone();
    let active = cobolt_forms::containers::ActiveTabs::new();
    let live = editing == Some(f.grid.id.as_str());
    let copy = if live {
        render_copy(&f.grid, f.screen.size(), f.container_clip.clone())
    } else {
        view_copy(&f.grid, f.screen.size(), f.container_clip.clone())
    };
    let controls = [copy];
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(f.screen));
    child.set_clip_rect(f.clip.unwrap_or(canvas).intersect(ui.clip_rect()));
    child.set_min_size(f.screen.size());
    if !live {
        child.disable();
        child.set_opacity(1.0);
    }
    let input = cobolt_forms::render::RenderInput {
        controls: &controls,
        state: &cobolt_forms::render::DesignedState,
        form_size: f.screen.size(),
        glass,
        mode: cobolt_forms::render::RenderMode::Interactive,
        active_tabs: &active,
        backdrop: cobolt_forms::render::Backdrop::behind(behind),
    };
    if live {
        cobolt_forms::render::render_form_scoped(&mut child, &input, None, scope()).prop_updates
    } else {
        without_input(&ctx, || {
            cobolt_forms::render::render_form_scoped(&mut child, &input, None, view_scope());
        });
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::ControlType;

    fn grid(columns: &str, rows: &str) -> Control {
        let mut g = Control::new("DG", ControlType::DataGrid, 20, 20);
        g.set_prop("Columns", PropValue::String(columns.into()));
        g.set_prop("Rows", PropValue::String(rows.into()));
        g
    }

    /// Evaluate the edit against the grid as it stands, then write it.
    macro_rules! apply {
        ($g:ident, $edit:expr) => {{
            let edits: Edits = $edit;
            for (k, v) in edits {
                $g.set_prop(k, v);
            }
        }};
    }

    fn rows(g: &Control) -> String {
        text_of(g, "Rows")
    }

    #[test]
    fn a_new_column_and_row_carry_fake_data() {
        let mut g = grid("Name:string\nAge:number", "Bob\t30\nann\t9");
        apply!(g, add_column(&g));
        assert_eq!(text_of(&g, "Columns"), "Name:string\nAge:number\nColumn 3:string");
        assert_eq!(rows(&g), "Bob\t30\tText 1\nann\t9\tText 2");
        apply!(g, add_row(&g));
        assert_eq!(rows(&g), "Bob\t30\tText 1\nann\t9\tText 2\nText 3\t0\tText 3");
        // A grid with nothing in it starts from a column of its own.
        let mut empty = grid("", "");
        assert!(add_row(&empty).is_empty(), "no columns, no row to add");
        apply!(empty, add_column(&empty));
        assert_eq!(text_of(&empty, "Columns"), "Column 1:string");
        apply!(empty, add_row(&empty));
        assert_eq!(rows(&empty), "Text 1");
    }

    #[test]
    fn removing_a_column_takes_its_line_its_cells_and_its_advanced_column() {
        let mut g = grid("A:string\nB:string\nC:string", "a1\tb1\tc1\na2\tb2\tc2");
        // Describe the columns in AdvancedGrid, as a bound grid does.
        let adv = DataGridAdvanced::from_control(&g);
        g.set_prop(DATAGRID_ADVANCED_PROP, PropValue::String(adv.to_json().unwrap()));
        apply!(g, remove_column(&g, 1));
        assert_eq!(text_of(&g, "Columns"), "A:string\nC:string");
        assert_eq!(rows(&g), "a1\tc1\na2\tc2");
        let titles: Vec<String> = DataGridAdvanced::from_control(&g).columns.iter().map(|c| c.title.clone()).collect();
        assert_eq!(titles, ["A", "C"]);
        // …and the shown columns still find their data.
        let shown = shown_columns(&g);
        assert_eq!(shown.iter().map(|c| c.source).collect::<Vec<_>>(), [Some(0), Some(1)]);
    }

    #[test]
    fn removing_a_row_shifts_the_heights_of_the_rows_after_it() {
        let mut g = grid("A:string", "r1\nr2\nr3");
        g.set_prop("RowHeightOverrides", PropValue::String("2=40;3=60".into()));
        apply!(g, remove_row(&g, 1));
        assert_eq!(rows(&g), "r1\nr3");
        assert_eq!(text_of(&g, "RowHeightOverrides"), "2=60", "row 3 is row 2 now, and kept its height");
        assert!(remove_row(&g, 9).is_empty(), "no such row, no change");
    }

    #[test]
    fn a_renamed_title_keeps_its_data() {
        // Columns named by the title only (as a binding with display names seeds):
        // the line must follow the rename or the column loses its cells.
        let mut g = grid("Actor Caption:string", "Leo");
        let mut adv = DataGridAdvanced::from_control(&g);
        adv.columns[0].source_name = "ACTOR-CAPTION".into();
        adv.columns[0].id = "ACTOR_CAPTION".into();
        g.set_prop(DATAGRID_ADVANCED_PROP, PropValue::String(adv.to_json().unwrap()));
        assert_eq!(shown_columns(&g)[0].source, Some(0));
        apply!(g, rename_column(&g, 0, "Who"));
        assert_eq!(shown_columns(&g)[0], ShownColumn { source: Some(0), title: "Who".into() });
        assert_eq!(text_of(&g, "Columns"), "Who:string");

        // Named by the source: the line is left alone.
        let mut h = grid("ACTOR-CAPTION:string", "Leo");
        apply!(h, rename_column(&h, 0, "Who"));
        assert_eq!(text_of(&h, "Columns"), "ACTOR-CAPTION:string");
        assert_eq!(shown_columns(&h)[0], ShownColumn { source: Some(0), title: "Who".into() });

        // A title with a colon cannot sit before one in Columns.
        let mut k = grid("Actor Caption:string", "Leo");
        let mut adv = DataGridAdvanced::from_control(&k);
        adv.columns[0].source_name = "ACTOR-CAPTION".into();
        k.set_prop(DATAGRID_ADVANCED_PROP, PropValue::String(adv.to_json().unwrap()));
        apply!(k, rename_column(&k, 0, "Cost: USD"));
        assert_eq!(shown_columns(&k)[0], ShownColumn { source: Some(0), title: "Cost: USD".into() });

        // Nothing typed, or the same title: nothing to write.
        assert!(rename_column(&h, 0, "  ").is_empty());
        assert!(rename_column(&h, 0, "Who").is_empty());
    }

    #[test]
    fn moving_a_column_reorders_the_advanced_columns_only() {
        let mut g = grid("A:string\nB:string\nC:string", "a\tb\tc");
        apply!(g, move_column(&g, 2, 0));
        let titles: Vec<String> = shown_columns(&g).iter().map(|c| c.title.clone()).collect();
        assert_eq!(titles, ["C", "A", "B"]);
        assert_eq!(text_of(&g, "Columns"), "A:string\nB:string\nC:string", "the data layout is untouched");
        assert!(move_column(&g, 1, 1).is_empty());
    }

    #[test]
    fn only_a_grids_design_properties_are_written_back() {
        for k in ["Rows", "AdvancedGrid", "RowHeight", "RowHeightOverrides", "ColumnFilters"] {
            assert!(accepts(k), "{k}");
        }
        for k in ["ClickedRow", "EditedValue", "PreviousValue", "_RuntimeColumnFilters", "Columns", "X"] {
            assert!(!accepts(k), "{k}");
        }
        assert_eq!(typed_like(Some(&PropValue::Int(22)), "30".into()), PropValue::Int(30));
        assert_eq!(typed_like(None, "30".into()), PropValue::String("30".into()));
    }

    #[test]
    fn the_toolbar_stays_inside_the_canvas_and_off_the_grid() {
        let canvas = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let size = toolbar_size();
        let grid_in_middle = Rect::from_min_size(pos2(100.0, 200.0), vec2(300.0, 200.0));
        let p = toolbar_pos(grid_in_middle, canvas);
        let bar = Rect::from_min_size(p, size);
        assert!(bar.max.y < grid_in_middle.min.y - FRAME_BAND, "above the frame band: {bar:?}");
        assert!(canvas.contains_rect(bar));
        // No room above: below.
        let grid_at_top = Rect::from_min_size(pos2(100.0, 10.0), vec2(300.0, 200.0));
        let bar = Rect::from_min_size(toolbar_pos(grid_at_top, canvas), size);
        assert!(bar.min.y > grid_at_top.max.y + FRAME_BAND, "below the frame band: {bar:?}");
        // Against the right edge: pulled back in.
        let grid_right = Rect::from_min_size(pos2(700.0, 200.0), vec2(90.0, 100.0));
        let bar = Rect::from_min_size(toolbar_pos(grid_right, canvas), size);
        assert!(canvas.contains_rect(bar), "{bar:?}");
    }
}
