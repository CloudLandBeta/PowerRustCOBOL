// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 106 — the *Edit DataGrid settings* window's own state and its Columns section's
//! moving parts: the window's size and grip, the column list on the left (click to
//! select, drag to reorder, an insertion line while dragging), the header's
//! **+ − ▲ ▼**, and the edits they make.
//!
//! The edits are the canvas edit mode's (`grid_edit`), so a column added, deleted or
//! moved here leaves `Columns`, `Rows` and `AdvancedGrid` exactly as the same action on
//! the canvas would, and each reaches the Designer as ONE undo step (R4–R7). The selected
//! column's details are drawn by the properties pane, which owns the field helpers.

use cobolt_forms::model::{Control, DataGridAdvanced, DataGridColumn, PropValue};

use super::grid_edit;

/// Opening size. From here on the size is whatever the developer dragged it to — see
/// [`SettingsState::size`].
const DEFAULT_W: f32 = 820.0;
const DEFAULT_H: f32 = 620.0;
/// Hard stops for the grip.
const MIN_W: f32 = 620.0;
const MIN_H: f32 = 380.0;
const MAX_W: f32 = 2400.0;
const MAX_H: f32 = 1600.0;
const GRIP: f32 = 14.0;
const GRIP_INSET: f32 = 3.0;
/// The title bar and the window's own margins: subtracted from the stored height to give
/// the body's scroll area — a constant, so nothing measures the space it was handed.
pub(crate) const CHROME_H: f32 = 48.0;
/// The column list's width, and one row's height.
pub(crate) const LIST_W: f32 = 220.0;
const ROW_H: f32 = 24.0;
/// The two panes' height is this share of the window's, within these stops: tall enough
/// to work in, short enough that the sections around them stay a wheel-turn away.
const PANES_SHARE: f32 = 0.62;
const PANES_MIN_H: f32 = 220.0;

/// The window's state, kept on the properties pane while the IDE runs.
pub(crate) struct SettingsState {
    /// The window's size, owned here rather than by egui (CONVENTIONS' window rule): it
    /// changes only when the developer drags the grip.
    pub(crate) size: egui::Vec2,
    /// The grid the selection below belongs to.
    grid: String,
    /// The selected column: its display index and id. The id finds it again after a
    /// reorder; the index tells apart two columns that share an id.
    selected: Option<(usize, String)>,
    /// The list row being dragged.
    dragging: Option<usize>,
    /// The column whose Delete is waiting for a yes (its id).
    pub(crate) confirm_delete: Option<String>,
    /// The icon catalogue, for a Button column's icon.
    pub(crate) icon_picker: super::icon_picker::IconPickerState,
    /// The data names of the form's generated program, and what they were worked out
    /// from: the form is generated again only when its own data or controls change, never
    /// for a keystroke in a column title (the grid's columns are no part of them).
    form_names: Option<(u64, std::collections::HashSet<String>)>,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            size: egui::vec2(DEFAULT_W, DEFAULT_H),
            grid: String::new(),
            selected: None,
            dragging: None,
            confirm_delete: None,
            icon_picker: Default::default(),
            form_names: None,
        }
    }
}

/// What the header's buttons or the list asked for this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ColumnsAction {
    Add,
    /// Ask before deleting the selected column.
    AskDelete,
    Delete(usize),
    Move { from: usize, to: usize },
}

impl SettingsState {
    /// Start (or keep) working on `grid`: a different grid starts with nothing selected
    /// and nothing pending.
    pub(crate) fn for_grid(&mut self, grid: &str) {
        if !self.grid.eq_ignore_ascii_case(grid) {
            self.grid = grid.to_owned();
            self.selected = None;
            self.dragging = None;
            self.confirm_delete = None;
            self.form_names = None;
        }
    }

    /// The selected column's display index in `columns`, falling back on the first
    /// column when nothing (or a column that has since gone) is selected.
    pub(crate) fn selected_index(&mut self, columns: &[DataGridColumn]) -> Option<usize> {
        let found = match &self.selected {
            Some((i, id)) if columns.get(*i).is_some_and(|c| &c.id == id) => Some(*i),
            Some((i, id)) => columns
                .iter()
                .position(|c| &c.id == id)
                .or_else(|| (!columns.is_empty()).then(|| (*i).min(columns.len() - 1))),
            None => (!columns.is_empty()).then_some(0),
        };
        self.selected = found.map(|i| (i, columns[i].id.clone()));
        found
    }

    /// Select the column at display index `index` (its id is read from `columns`).
    pub(crate) fn select(&mut self, columns: &[DataGridColumn], index: usize) {
        self.selected = columns.get(index).map(|c| (index, c.id.clone()));
    }

    /// Select by id, at the index the column will have — for a column an edit has not
    /// reached the grid yet.
    pub(crate) fn select_pending(&mut self, index: usize, id: String) {
        self.selected = Some((index, id));
    }

    /// The two panes' height, from the stored window size alone.
    pub(crate) fn panes_height(&self) -> f32 {
        (self.size.y * PANES_SHARE).max(PANES_MIN_H)
    }

    /// The naming problems of `grid` (R17), by the same code as Check. The columns are
    /// checked every frame (cheap); the form's data names they are checked against come
    /// from a generation of the form that is redone only when the form's own data or its
    /// controls change.
    pub(crate) fn problems(&mut self, form: &cobolt_forms::Form, grid: &Control) -> Vec<cobolt_forms::row_content::Problem> {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        form.user_ws_source.hash(&mut h);
        form.cobol_structure.file_section.hash(&mut h);
        fn ids(list: &[Control], h: &mut impl Hasher) {
            for c in list {
                c.id.hash(h);
                c.control_type.hash(h);
                c.events.len().hash(h);
                ids(&c.children, h);
            }
        }
        ids(&form.controls, &mut h);
        let key = h.finish();
        if self.form_names.as_ref().map(|(k, _)| *k) != Some(key) {
            let generated = cobolt_codegen::generate(form);
            self.form_names = Some((key, cobolt_project_tools::validate_source::form_data_names(&generated)));
        }
        let names = self.form_names.as_ref().map(|(_, n)| n);
        cobolt_project_tools::validate_source::grid_row_content_problems(grid, names.unwrap_or(&Default::default()))
    }

    /// The grip, in its own foreground area at the window's outer corner. Its drag delta
    /// is the only writer of `self.size`.
    pub(crate) fn resize_grip(&mut self, ctx: &egui::Context, window: egui::Rect) {
        let corner = window.max - egui::vec2(GRIP_INSET, GRIP_INSET);
        let origin = corner - egui::vec2(GRIP, GRIP);
        egui::Area::new(egui::Id::new(("datagrid_settings_grip", &self.grid)))
            .order(egui::Order::Foreground)
            .fixed_pos(origin)
            .show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(egui::vec2(GRIP, GRIP), egui::Sense::drag());
                if response.dragged() {
                    self.size += response.drag_delta();
                    self.size.x = self.size.x.clamp(MIN_W, MAX_W);
                    self.size.y = self.size.y.clamp(MIN_H, MAX_H);
                }
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNorthWest);
                }
                let colour = if response.hovered() || response.dragged() {
                    ui.visuals().selection.stroke.color
                } else {
                    ui.visuals().weak_text_color()
                };
                for offset in [2.0_f32, 6.0, 10.0] {
                    let len = GRIP - offset - 1.0;
                    ui.painter().line_segment(
                        [
                            egui::pos2(rect.right() - len, rect.bottom()),
                            egui::pos2(rect.right(), rect.bottom() - len),
                        ],
                        egui::Stroke::new(1.2, colour),
                    );
                }
            });
    }

    /// The column list: one row per column, "n — name", a Button column marked as one and
    /// a column with a naming problem flagged. A click selects; a drag reorders, with an
    /// insertion line where the column would land (as the Tab Order list does).
    pub(crate) fn list_ui(
        &mut self,
        ui: &mut egui::Ui,
        columns: &[DataGridColumn],
        flagged: &[bool],
        width: f32,
        tr: &crate::i18n::Tr,
    ) -> Option<ColumnsAction> {
        let mut out = None;
        let selected = self.selected_index(columns);
        let mut rects = Vec::with_capacity(columns.len());
        let mut drop_from = None;
        let visuals = ui.visuals().clone();
        for (i, column) in columns.iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, ROW_H), egui::Sense::click_and_drag());
            rects.push(rect);
            let painter = ui.painter();
            if selected == Some(i) {
                painter.rect_filled(rect, 4.0, visuals.selection.bg_fill);
            } else if resp.hovered() {
                painter.rect_filled(rect, 4.0, visuals.widgets.hovered.weak_bg_fill);
            }
            let ink = if selected == Some(i) { visuals.selection.stroke.color } else { visuals.text_color() };
            let mut label = format!("{} — {}", i + 1, shown_name(column));
            if column.is_button() {
                label = format!("{label}  [{}]", tr.dg_col_content_button);
            }
            let galley = painter.layout_no_wrap(label, egui::FontId::proportional(13.0), ink);
            let text_pos = rect.left_center() + egui::vec2(8.0, -galley.size().y / 2.0);
            let clip = rect.shrink2(egui::vec2(4.0, 0.0));
            painter.with_clip_rect(clip.intersect(ui.clip_rect())).galley(text_pos, galley, ink);
            if flagged.get(i).copied().unwrap_or(false) {
                painter.text(
                    rect.right_center() - egui::vec2(6.0, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    "⚠",
                    egui::FontId::proportional(13.0),
                    visuals.warn_fg_color,
                );
            }
            if resp.drag_started() {
                self.dragging = Some(i);
            }
            if resp.drag_stopped() {
                drop_from = self.dragging.take();
            }
            if resp.clicked() {
                self.select(columns, i);
                self.confirm_delete = None;
            }
            if resp.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }
        }
        let pointer = ui.ctx().pointer_latest_pos();
        let gap = pointer.map(|p| insertion_index(&rects, p.y));
        if let (Some(_), Some(gap), Some(first)) = (self.dragging, gap, rects.first()) {
            let y = rects.get(gap).map_or_else(|| rects.last().map_or(first.top(), |r| r.bottom()), |r| r.top());
            ui.painter()
                .hline(first.left()..=first.right(), y, egui::Stroke::new(2.0, visuals.selection.stroke.color));
        }
        if let (Some(from), Some(gap)) = (drop_from, gap) {
            let to = drop_target(from, gap);
            if to != from {
                out = Some(ColumnsAction::Move { from, to });
            }
        }
        out
    }

    /// The header's **+ − ▲ ▼**, right-aligned in `ui`. Delete and the moves act on the
    /// selected column and say why when they cannot.
    pub(crate) fn header_buttons(
        &mut self,
        ui: &mut egui::Ui,
        columns: &[DataGridColumn],
        tr: &crate::i18n::Tr,
    ) -> Option<ColumnsAction> {
        let mut out = None;
        let sel = self.selected_index(columns);
        let n = columns.len();
        // Right to left: the last one added sits leftmost.
        let can_down = sel.is_some_and(|i| i + 1 < n);
        if ui
            .add_enabled(can_down, egui::Button::new("▼").small())
            .on_hover_text(tr.dg_col_move_down)
            .on_disabled_hover_text(if sel.is_none() { tr.dg_col_none_selected } else { tr.dg_col_is_last })
            .clicked()
        {
            if let Some(i) = sel {
                out = Some(ColumnsAction::Move { from: i, to: i + 1 });
            }
        }
        let can_up = sel.is_some_and(|i| i > 0);
        if ui
            .add_enabled(can_up, egui::Button::new("▲").small())
            .on_hover_text(tr.dg_col_move_up)
            .on_disabled_hover_text(if sel.is_none() { tr.dg_col_none_selected } else { tr.dg_col_is_first })
            .clicked()
        {
            if let Some(i) = sel {
                out = Some(ColumnsAction::Move { from: i, to: i - 1 });
            }
        }
        if ui
            .add_enabled(sel.is_some(), egui::Button::new("−").small())
            .on_hover_text(tr.dg_col_delete)
            .on_disabled_hover_text(tr.dg_col_none_selected)
            .clicked()
        {
            out = Some(ColumnsAction::AskDelete);
        }
        if ui.add(egui::Button::new("+").small()).on_hover_text(tr.dg_col_add).clicked() {
            out = Some(ColumnsAction::Add);
        }
        out
    }
}

/// The name a column goes by in the list: its title, else its source field, else its id.
pub(crate) fn shown_name(column: &DataGridColumn) -> &str {
    [&column.title, &column.source_name, &column.id]
        .into_iter()
        .map(|s| s.trim())
        .find(|s| !s.is_empty())
        .unwrap_or("")
}

/// The gap a pointer at `y` points at: 0 is before the first row, `rects.len()` after
/// the last.
fn insertion_index(rects: &[egui::Rect], y: f32) -> usize {
    rects.iter().position(|r| y < r.center().y).unwrap_or(rects.len())
}

/// Where a row dragged from `from` lands when dropped in gap `gap` (gaps count positions
/// BEFORE the move; taking the row out first shifts every later gap up by one).
fn drop_target(from: usize, gap: usize) -> usize {
    if gap > from {
        gap - 1
    } else {
        gap
    }
}

fn text_of(grid: &Control, key: &str) -> String {
    grid.get_prop(key).map(|v| v.as_str().to_owned()).unwrap_or_default()
}

type Edits = Vec<(String, PropValue)>;

/// `edits` as they would leave `grid`.
fn applied(grid: &Control, edits: &Edits) -> Control {
    let mut next = grid.clone();
    for (key, value) in edits {
        next.set_prop(key, value.clone());
    }
    next
}

/// `more` on top of `edits`: a key both set keeps the later value, in the earlier place.
fn merged(mut edits: Edits, more: Edits) -> Edits {
    for (key, value) in more {
        match edits.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => edits.push((key, value)),
        }
    }
    edits
}

/// R4 — a new column after the one at display index `after` (or at the end), made by the
/// canvas edit mode's own Add and then moved into place. Returns the edits, the new
/// column's display index and its id.
pub(crate) fn add_column_after(grid: &Control, after: Option<usize>) -> (Edits, usize, String) {
    let edits = grid_edit::add_column(grid);
    let added = applied(grid, &edits);
    let columns = DataGridAdvanced::from_control(&added).columns;
    let last = columns.len().saturating_sub(1);
    let id = columns.last().map(|c| c.id.clone()).unwrap_or_default();
    let want = after.map_or(last, |a| (a + 1).min(last));
    if want == last {
        return (edits, last, id);
    }
    let moved = grid_edit::move_column(&added, last, want);
    (merged(edits, moved), want, id)
}

/// R5 — the column at display index `index` gone, the canvas edit mode's way.
pub(crate) fn delete_column(grid: &Control, index: usize) -> Edits {
    grid_edit::remove_column(grid, index)
}

/// R6 — the display order changed; the data schema and the rows keep theirs.
pub(crate) fn move_column(grid: &Control, from: usize, to: usize) -> Edits {
    grid_edit::move_column(grid, from, to)
}

/// R8 — make a column a Button column (an action), or a Data one again. Button goes in
/// `control_kind`, which the renderer reads before `edit_control`, so the data kind the
/// column had (Checkbox, Dropdown…) stays in `edit_control` and comes back when the
/// column is Data again. A caption or icon already typed is kept too: switching back and
/// forth loses nothing. (An older form may say Button in `edit_control`; going to Data
/// makes that a Textbox, the only kind it can have had.)
pub(crate) fn set_button_content(column: &mut DataGridColumn, button: bool) {
    if button == column.is_button() {
        return;
    }
    if button {
        column.control_kind = "Button".to_owned();
    } else {
        if column.control_kind.eq_ignore_ascii_case("button") {
            column.control_kind.clear();
        }
        if column.edit_control.eq_ignore_ascii_case("button") {
            column.edit_control = "Textbox".to_owned();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::model::{ControlType, DATAGRID_ADVANCED_PROP};

    /// A grid of three columns A, B, C over two rows, described by both `Columns` and
    /// `AdvancedGrid`.
    fn grid() -> Control {
        let mut g = Control::new("G", ControlType::DataGrid, 0, 0);
        g.set_prop("Columns", PropValue::String("A:string\nB:string\nC:string".into()));
        g.set_prop("Rows", PropValue::String("a1\tb1\tc1\na2\tb2\tc2".into()));
        let advanced = DataGridAdvanced {
            columns: ["A", "B", "C"]
                .iter()
                .map(|n| DataGridColumn {
                    id: (*n).into(),
                    title: (*n).into(),
                    source_name: (*n).into(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        g.set_prop(DATAGRID_ADVANCED_PROP, PropValue::String(advanced.to_json().unwrap()));
        g
    }

    fn order(g: &Control) -> Vec<String> {
        DataGridAdvanced::from_control(g).columns.iter().map(|c| c.title.clone()).collect()
    }

    #[test]
    fn add_puts_the_new_column_after_the_selected_one() {
        let g = grid();
        let (edits, at, id) = add_column_after(&g, Some(0));
        let next = applied(&g, &edits);
        assert_eq!(order(&next), ["A", "Column 4", "B", "C"]);
        assert_eq!((at, id.as_str()), (1, "COLUMN_4"));
        // The data follows the canvas's Add: a Columns line, and a cell in every row.
        assert_eq!(text_of(&next, "Columns"), "A:string\nB:string\nC:string\nColumn 4:string");
        assert_eq!(text_of(&next, "Rows"), "a1\tb1\tc1\tText 1\na2\tb2\tc2\tText 2");
        // With nothing selected, at the end; and each key is set once (one undo step).
        let (edits, at, _) = add_column_after(&g, None);
        assert_eq!(order(&applied(&g, &edits)), ["A", "B", "C", "Column 4"]);
        assert_eq!(at, 3);
        let mut keys: Vec<_> = edits.iter().map(|(k, _)| k.clone()).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), edits.len(), "{edits:?}");
    }

    #[test]
    fn delete_and_move_are_the_canvas_edits() {
        let g = grid();
        assert_eq!(delete_column(&g, 1), grid_edit::remove_column(&g, 1));
        let next = applied(&g, &delete_column(&g, 1));
        assert_eq!(order(&next), ["A", "C"]);
        assert_eq!(text_of(&next, "Rows"), "a1\tc1\na2\tc2");
        let moved = applied(&g, &move_column(&g, 2, 0));
        assert_eq!(order(&moved), ["C", "A", "B"], "AC3: column 3 dragged above column 1");
        assert_eq!(text_of(&moved, "Columns"), text_of(&g, "Columns"), "the data keeps its order");
    }

    #[test]
    fn a_drop_lands_where_the_line_was_drawn() {
        // Three rows; dragging row 2 (index 2) to the gap before row 0 → 0.
        assert_eq!(drop_target(2, 0), 0);
        // Row 0 dropped in the gap after row 1 (gap 2) → it becomes index 1.
        assert_eq!(drop_target(0, 2), 1);
        // Dropped where it was: no move.
        assert_eq!(drop_target(1, 1), 1);
        assert_eq!(drop_target(1, 2), 1);
        let rects: Vec<_> = (0..3)
            .map(|i| egui::Rect::from_min_size(egui::pos2(0.0, i as f32 * 24.0), egui::vec2(100.0, 24.0)))
            .collect();
        assert_eq!(insertion_index(&rects, 5.0), 0);
        assert_eq!(insertion_index(&rects, 30.0), 1);
        assert_eq!(insertion_index(&rects, 500.0), 3);
    }

    #[test]
    fn the_selection_follows_the_column_and_tells_twins_apart() {
        let mut s = SettingsState::default();
        s.for_grid("G");
        let mut cols = DataGridAdvanced::from_control(&grid()).columns;
        assert_eq!(s.selected_index(&cols), Some(0), "the first column by default");
        s.select(&cols, 2);
        cols.swap(0, 2);
        assert_eq!(s.selected_index(&cols), Some(0), "found again by id after a move");
        cols[1].id = "C".into();
        s.select(&cols, 1);
        assert_eq!(s.selected_index(&cols), Some(1), "the second of two columns sharing an id");
        cols.truncate(1);
        assert_eq!(s.selected_index(&cols), Some(0), "a column that went: the nearest one left");
        s.for_grid("OTHER");
        assert!(s.selected.is_none() && s.confirm_delete.is_none());
    }

    #[test]
    fn content_switches_the_kind_where_the_renderer_reads_it_and_back_again() {
        let mut c = DataGridColumn { edit_control: "Checkbox".into(), ..Default::default() };
        set_button_content(&mut c, true);
        assert!(c.is_button());
        c.button_caption = "Edit".into();
        set_button_content(&mut c, false);
        assert!(!c.is_button());
        assert_eq!(c.edit_control, "Checkbox", "Data → Button → Data gives the Checkbox back");
        assert_eq!(c.button_caption, "Edit", "kept for switching back");
        let mut written = DataGridColumn { control_kind: "Image".into(), ..Default::default() };
        set_button_content(&mut written, true);
        assert!(written.is_button(), "control_kind wins in the renderer, so it is set");
        // A form that said Button in `edit_control` becomes a plain Data column.
        let mut older = DataGridColumn { edit_control: "Button".into(), ..Default::default() };
        set_button_content(&mut older, false);
        assert!(!older.is_button());
        assert_eq!(older.edit_control, "Textbox");
    }
}
