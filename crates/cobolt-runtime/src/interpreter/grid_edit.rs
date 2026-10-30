// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DataGrid write-back (specs 094 and 095): what the operator changes in a bound
//! grid is written back to its source — an indexed file, a COBOL table or a SQL
//! table.
//!
//! The grid itself only *shows* a change (`Rows`, `onCellEdited`); this module
//! keeps what the operator edited until it is written, and `grid_unit` writes it.
//! The shape, end to end:
//!
//! 1. **A load remembers the source.** Wherever a binding fills `Rows`
//!    ([`Interpreter::capture_ledger`]) the grid keeps a snapshot of what it
//!    loaded and publishes `_RowOrigins`: what each line of `Rows` *is* — a table
//!    occurrence, or the key cells as loaded — and empty for a row added since.
//!    The origins follow `Sort`, `DeleteRow` and `AddRow`; `ClearRows` forgets the
//!    snapshot, so emptying the display is never a request to delete the table.
//! 2. `onCellEdited` arrives on a **writable** grid (a writable binding, or a
//!    quick-bound indexed grid whose `.cidx` has a primary key, with
//!    `AllowCellEditing` on): [`Interpreter::note_grid_edit`] keeps the edit as
//!    *pending*, with the origin of its row. The handler then runs as usual and
//!    may refuse the edit by putting `PreviousValue` back with `SetCellValue`.
//! 3. The handler returns to the generated event loop, which calls
//!    `COBOL-WAIT-EVENT` again. **That call is "after the handler returns"**:
//!    [`Interpreter::settle_grid_edits`] runs first. With `AutoCommit` on it writes
//!    the cell edits the event kept; with it off it leaves them for the code to
//!    write (`Save()`) or drop (`DiscardEdit()`).
//! 4. Everything is written as **one unit**, all or nothing (`grid_unit`). An
//!    explicit `Save()` writes the edited cells (`UPDATE`/`REWRITE`), the rows
//!    added with `AddRow` (`INSERT`/`WRITE`) and the rows removed with `DeleteRow`
//!    (`DELETE`) — a blank line is never inserted, and a row whose key was changed
//!    is never updated or deleted. Before it writes, it checks that the source still
//!    holds what the grid loaded. A COBOL table is regenerated whole from the grid.
//! 5. A unit that fails writes nothing, sets `CommitStatus` and `LastError`,
//!    queues `onCommitError` and **puts the old values back**: the grid returns to
//!    what was last loaded or saved, and the cell that failed carries a mark.
//!
//! `CommitStatus` is the source's own code — an indexed file's FILE STATUS — or
//! one of ours: `00` written, `21` a key cannot change, `23` no such row, `90` the
//! driver or the file failed, `91` the grid is not writable, `92` the value is not
//! acceptable for the field, `95` the connection is in Async mode, `96` the row
//! was changed by someone else since it was loaded, `98` edits are still pending.

use std::collections::HashMap;

use super::Interpreter;

pub(super) const OK: &str = "00";
pub(super) const KEY_CHANGED: &str = "21";
pub(super) const NOT_FOUND: &str = "23";
pub(super) const IO_ERROR: &str = "90";
pub(super) const NOT_WRITABLE: &str = "91";
pub(super) const REJECTED: &str = "92";
/// The source no longer holds what the grid loaded (spec 095, Q4).
pub(super) const CONFLICT: &str = "96";
/// Left in `CommitStatus` when a refresh found edits the code has not decided.
pub(super) const PENDING: &str = "98";

/// One kept edit of one cell. Row and column are 0-based positions in `Rows`
/// (the grid's DATA row and column, not what the operator sees after sorting) —
/// where the handler saw the cell; `origin` is what the row *is* (spec 095), so
/// the edit still finds its row after a `Sort`, a `DeleteRow` or a refresh moved
/// it. Empty for a row the program added: such a row is written whole, as an insert.
#[derive(Debug, Clone)]
pub(super) struct PendingEdit {
    pub row: usize,
    pub col: usize,
    pub value: String,
    pub previous: String,
    pub origin: String,
}

/// A row as the grid loaded it from the source: what it is, and the cells it showed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LoadedRow {
    pub origin: String,
    pub cells: Vec<String>,
}

/// What a grid has kept and not yet written, and what it last loaded.
#[derive(Debug, Default)]
pub(super) struct GridEditState {
    pub pending: Vec<PendingEdit>,
    /// The rows of the source as of the last load or save, in the source's order.
    pub loaded: Vec<LoadedRow>,
    /// Two loaded rows have the same key: rows cannot be told apart by it.
    pub ambiguous: bool,
    /// The grid has remembered a load. Until it has, nothing can be said to be added
    /// or deleted: the rows are whatever the designer or the program put there.
    pub captured: bool,
}

/// Why a write did not happen, in the source's own terms.
#[derive(Debug)]
pub(super) struct CommitError {
    pub code: String,
    pub message: String,
}

impl CommitError {
    pub(super) fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.to_owned(), message: message.into() }
    }
}

pub(super) type GridEdits = HashMap<String, GridEditState>;

fn flag(text: &str) -> bool {
    matches!(text.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

/// A 1-based index as COBOL hands it over, as a 0-based position.
fn index1(text: &str) -> Option<usize> {
    text.trim().parse::<i64>().ok().filter(|n| *n >= 1).map(|n| (n - 1) as usize)
}

/// The two digits of "… FILE STATUS 23", or `90` when the text names none.
pub(super) fn file_status(message: &str) -> String {
    message
        .rsplit("FILE STATUS")
        .next()
        .map(str::trim)
        .and_then(|t| t.get(..2))
        .filter(|t| t.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(IO_ERROR)
        .to_owned()
}

pub(super) fn names(list: &str) -> Vec<String> {
    list.split(|c| matches!(c, '\n' | '\r' | ',' | ';' | '\t'))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

impl Interpreter {
    /// A grid is written back only when all of these hold: it is bound to a
    /// source that can be written, the binding (or the quick-bind's `.cidx`)
    /// says so, and the operator can edit a cell at all.
    fn grid_writable(&self, grid: &str) -> bool {
        self.obj_get(grid, "_BindingWritable").trim() == "1" && flag(&self.obj_get(grid, "AllowCellEditing"))
    }

    /// `AutoCommit` is on unless the form says otherwise: an older form has no
    /// such property and means the default.
    pub(super) fn grid_auto_commit(&self, grid: &str) -> bool {
        let text = self.obj_get(grid, "AutoCommit");
        text.trim().is_empty() || flag(&text)
    }

    /// The grid a binding feeds — the one seeded with `_BindingId` — if any.
    pub(super) fn grid_of_binding(&self, binding_id: &str) -> Option<String> {
        self.objects
            .iter()
            .filter(|(_, object)| object.class.eq_ignore_ascii_case("DataGrid"))
            .map(|(id, _)| id.clone())
            .find(|id| self.obj_get(id, "_BindingId").trim().eq_ignore_ascii_case(binding_id.trim()))
    }

    pub(super) fn set_commit_status(&mut self, grid: &str, code: &str, message: &str) {
        self.obj_set(grid, "CommitStatus", code.to_owned());
        self.obj_set(grid, "LastError", message.to_owned());
    }

    /// The cells still pending, for the renderer's marker (`row,column` pairs
    /// from 1) — shown only while the code, not the runtime, decides.
    pub(super) fn publish_pending_cells(&mut self, grid: &str) {
        let shown = if self.grid_auto_commit(grid) {
            String::new()
        } else {
            self.grid_edits
                .get(&grid.to_ascii_uppercase())
                .map(|st| {
                    st.pending
                        .iter()
                        .map(|e| format!("{},{}", e.row + 1, e.col + 1))
                        .collect::<Vec<_>>()
                        .join(";")
                })
                .unwrap_or_default()
        };
        self.obj_set(grid, "_PendingCells", shown);
    }

    /// Put `text` in one cell of the grid's `Rows`.
    pub(super) fn restore_cell(&mut self, grid: &str, row: usize, col: usize, text: &str) {
        let mut rows = self.datagrid_rows(grid);
        let width = self.datagrid_column_names(grid).len().max(col + 1);
        while rows.len() <= row {
            rows.push(vec![String::new(); width]);
        }
        if rows[row].len() <= col {
            rows[row].resize(col + 1, String::new());
        }
        rows[row][col] = text.to_owned();
        self.set_datagrid_rows(grid, &rows);
    }

    /// `onCellEdited` has just been folded in: remember the edit if the grid is
    /// one that writes back. (The handler has not run yet.)
    pub(super) fn note_grid_edit(&mut self, grid: &str, event: &str) {
        if !event.eq_ignore_ascii_case("onCellEdited") {
            return;
        }
        if !self.objects.get(grid).is_some_and(|o| o.class.eq_ignore_ascii_case("DataGrid")) {
            return;
        }
        if !self.grid_writable(grid) {
            return;
        }
        let (Some(row), Some(col)) =
            (index1(&self.obj_get(grid, "EditedRow")), index1(&self.obj_get(grid, "EditedColumn")))
        else {
            return;
        };
        let value = self.obj_get(grid, "EditedValue");
        let previous = self.obj_get(grid, "PreviousValue");
        // A grid its binding never loaded (its rows came from the designer or from the
        // program) takes the rows it shows now for what the source gave it — apart from
        // the cell just edited, which is `previous` there.
        let captured = self.grid_edits.get(&grid.to_ascii_uppercase()).is_some_and(|state| state.captured);
        if !captured {
            self.capture_ledger(grid);
            if let Some(state) = self.grid_edits.get_mut(&grid.to_ascii_uppercase()) {
                if let Some(cell) = state.loaded.get_mut(row).and_then(|loaded| loaded.cells.get_mut(col)) {
                    *cell = previous.clone();
                }
            }
        }
        // What the row is, read now, while the grid still shows it where the
        // operator edited it (spec 095): a batch waits through sorts and deletes.
        let origin = self.row_origins(grid).get(row).cloned().unwrap_or_default();
        let state = self.grid_edits.entry(grid.to_ascii_uppercase()).or_default();
        // A second edit of a cell still pending keeps the FIRST previous value,
        // so discarding brings back what the source holds.
        match state.pending.iter_mut().find(|e| e.row == row && e.col == col) {
            Some(edit) => {
                edit.value = value;
                if !origin.is_empty() {
                    edit.origin = origin;
                }
            }
            None => state.pending.push(PendingEdit { row, col, value, previous, origin }),
        }
        // The operator is editing again: a mark from the last failed save goes.
        self.obj_set(grid, "_FailedCells", String::new());
        self.publish_pending_cells(grid);
    }

    /// The top of `COBOL-WAIT-EVENT`: the handler of the last event has
    /// returned. Every grid on `AutoCommit` writes the cell edits it kept, as one
    /// unit.
    pub(super) fn settle_grid_edits(&mut self) {
        if self.grid_edits.is_empty() {
            return;
        }
        let waiting: Vec<String> = self
            .grid_edits
            .iter()
            .filter(|(_, state)| !state.pending.is_empty())
            .map(|(id, _)| id.clone())
            .collect();
        for grid in waiting {
            if self.grid_auto_commit(&grid) {
                self.commit_unit(&grid, false);
            }
        }
    }

    // ── What the grid loaded (spec 095, §8.1) ────────────────────────────────

    /// `_RowOrigins`, one entry per line of `Rows`: what the line *is* — a table
    /// occurrence, or the key cells as loaded — and empty for a row added since.
    /// A line past the end of the list is a row added since, too.
    pub(super) fn row_origins(&self, grid: &str) -> Vec<String> {
        let text = self.obj_get(grid, "_RowOrigins");
        if text.is_empty() {
            Vec::new()
        } else {
            text.split('\n').map(str::to_owned).collect()
        }
    }

    /// Where the row of `edit` is now: where the handler saw it when it still
    /// carries its origin, otherwise wherever that origin has moved to; `None`
    /// when the row is no longer in the grid. A row the program added has no origin
    /// and is where the handler saw it.
    pub(super) fn locate_edit(&self, grid: &str, edit: &PendingEdit) -> Option<usize> {
        if edit.origin.is_empty() {
            return Some(edit.row);
        }
        let origins = self.row_origins(grid);
        if origins.get(edit.row) == Some(&edit.origin) {
            Some(edit.row)
        } else {
            origins.iter().position(|origin| *origin == edit.origin)
        }
    }

    /// A load has just filled `Rows`: remember what the source gave. Publishes
    /// `_RowOrigins` (a table's occurrences are already there; an indexed file's or a
    /// SQL table's rows are told apart by their key cells) and `_KeyColumns` for the
    /// renderer, which keeps a key of a loaded row read-only. A grid that does not
    /// write back keeps nothing.
    pub(super) fn capture_ledger(&mut self, grid: &str) {
        let upper = grid.to_ascii_uppercase();
        let kind = self.obj_get(grid, "_BindingKind").trim().to_ascii_lowercase();
        let tracked = self.obj_get(grid, "_BindingWritable").trim() == "1"
            && matches!(kind.as_str(), "indexedfile" | "coboltable" | "sql" | "partiql");
        if !tracked {
            self.obj_set(grid, "_RowOrigins", String::new());
            self.obj_set(grid, "_KeyColumns", String::new());
            if let Some(state) = self.grid_edits.get_mut(&upper) {
                state.loaded.clear();
                state.ambiguous = false;
                state.captured = false;
            }
            return;
        }
        let rows = self.datagrid_rows(grid);
        let mut origins: Vec<String> = if kind == "coboltable" {
            self.row_origins(grid)
        } else {
            match self.grid_source(grid) {
                Ok(source) if source.missing_key().is_none() && !source.keys.is_empty() => {
                    rows.iter().map(|cells| source.key_origin(cells)).collect()
                }
                _ => Vec::new(),
            }
        };
        origins.resize(rows.len(), String::new());
        let mut seen = std::collections::HashSet::new();
        let ambiguous = kind != "coboltable" && origins.iter().filter(|o| !o.is_empty()).any(|o| !seen.insert(o.clone()));
        let key_columns = match (kind.as_str(), self.grid_source(grid)) {
            ("coboltable", _) | (_, Err(_)) => String::new(),
            (_, Ok(source)) => source.key_cols.iter().map(|c| (c + 1).to_string()).collect::<Vec<_>>().join(","),
        };
        self.obj_set(grid, "_RowOrigins", origins.join("\n"));
        self.obj_set(grid, "_KeyColumns", key_columns);
        let state = self.grid_edits.entry(upper).or_default();
        state.ambiguous = ambiguous;
        state.captured = true;
        state.loaded = origins.into_iter().zip(rows).map(|(origin, cells)| LoadedRow { origin, cells }).collect();
    }

    /// Put the grid back as the source was last loaded or saved: every cell, every
    /// added row and every deleted row. A COBOL table is read again.
    pub(super) fn restore_loaded(&mut self, grid: &str) {
        // A grid that has never been loaded has nothing to go back to: its rows are
        // whatever the designer or the program put there.
        let captured = self.grid_edits.get(&grid.to_ascii_uppercase()).is_some_and(|state| state.captured);
        if !captured {
            return;
        }
        if let Some(state) = self.grid_edits.get_mut(&grid.to_ascii_uppercase()) {
            state.pending.clear();
        }
        if self.obj_get(grid, "_BindingKind").trim().eq_ignore_ascii_case("CobolTable") {
            self.refresh_datagrid_binding(grid);
            self.capture_ledger(grid);
        } else {
            let loaded = self.grid_edits.get(&grid.to_ascii_uppercase()).map(|s| s.loaded.clone()).unwrap_or_default();
            if !loaded.is_empty() || !self.datagrid_rows(grid).is_empty() {
                let rows: Vec<Vec<String>> = loaded.iter().map(|row| row.cells.clone()).collect();
                self.set_datagrid_rows(grid, &rows);
                self.obj_set(grid, "_RowOrigins", loaded.iter().map(|row| row.origin.as_str()).collect::<Vec<_>>().join("\n"));
            }
        }
        self.publish_pending_cells(grid);
    }

    /// `Save()`: write the grid as one unit. Answers the status `CommitStatus`
    /// now holds; `00` when there was nothing to write.
    pub(super) fn save_method(&mut self, grid: &str) -> String {
        self.commit_unit(grid, true)
    }

    /// `DiscardEdit()`: put the grid back as the source has it.
    pub(super) fn discard_edit_method(&mut self, grid: &str) -> String {
        self.restore_loaded(grid);
        self.set_commit_status(grid, OK, "");
        self.obj_set(grid, "_FailedCells", String::new());
        OK.to_owned()
    }

    /// A refresh has loaded `Rows` over the grid. Edits the code has not decided
    /// are not thrown away: they go back over their cells, and `CommitStatus`
    /// says some are still pending.
    pub(super) fn reapply_pending_edits(&mut self, grid: &str) {
        let edits = match self.grid_edits.get(&grid.to_ascii_uppercase()) {
            Some(state) if !state.pending.is_empty() => state.pending.clone(),
            _ => return,
        };
        let mut rows = self.datagrid_rows(grid);
        // The reload may have moved the rows: each edit goes back on the row it
        // was made on, and remembers where that row is now. An edit whose row the
        // source no longer has is dropped with it.
        let mut kept = Vec::new();
        for mut edit in edits {
            let Some(row) = self.locate_edit(grid, &edit).filter(|row| *row < rows.len()) else {
                continue;
            };
            if let Some(cell) = rows[row].get_mut(edit.col) {
                *cell = edit.value.clone();
            }
            edit.row = row;
            kept.push(edit);
        }
        self.set_datagrid_rows(grid, &rows);
        if let Some(state) = self.grid_edits.get_mut(&grid.to_ascii_uppercase()) {
            state.pending = kept;
        }
        self.set_commit_status(grid, PENDING, "edits are still pending: Save() or DiscardEdit()");
        self.publish_pending_cells(grid);
    }

    // ── SQL ──────────────────────────────────────────────────────────────────

    /// The open connection of the `SqlDatabase` control a SQL binding names:
    /// its handle, or why there is none.
    pub(super) fn sql_handle(&self, grid: &str) -> Result<u32, CommitError> {
        let control = self.obj_get(grid, "_BindingSqlControl");
        let handle = self.obj_get(control.trim(), "_Handle").trim().parse::<u32>().unwrap_or(0);
        if control.trim().is_empty() || handle == 0 {
            return Err(CommitError::new(IO_ERROR, "the SQL connection is not open"));
        }
        Ok(handle)
    }

    /// Fill the grid from a SQL source: run the binding's query on the control's
    /// connection and lay the columns out in the order of the bound fields. A
    /// connection that is not open yet leaves the grid as it is — the program
    /// opens it, then asks again with `RefreshBinding()`.
    pub(super) fn refresh_sql_binding(&mut self, grid: &str) -> usize {
        let query = self.obj_get(grid, "_BindingSql");
        let Ok(handle) = self.sql_handle(grid) else {
            return 0;
        };
        if query.trim().is_empty() {
            return 0;
        }
        let control = self.obj_get(grid, "_BindingSqlControl");
        if let Err(message) = self.db.exec(handle, query.trim()) {
            self.obj_set(control.trim(), "LastError", message);
            self.obj_set(grid, "Rows", String::new());
            return 0;
        }
        let columns: Vec<String> = self.db.columns(handle);
        let fields = names(&self.obj_get(grid, "_BindingFields"));
        // The grid's columns are the bound fields, each found in the result by
        // name; a field the query does not return shows empty.
        let at: Vec<Option<usize>> = fields
            .iter()
            .map(|field| columns.iter().position(|c| c.eq_ignore_ascii_case(field)))
            .collect();
        let mut rows = Vec::new();
        while let Some(values) = self.db.take_row(handle) {
            rows.push(
                at.iter()
                    .map(|i| i.and_then(|i| values.get(i)).cloned().unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join("\t"),
            );
        }
        let (rows, _) = self.apply_binding_view(grid, &fields, rows, Vec::new());
        let count = rows.len();
        self.obj_set(grid, "Rows", rows.join("\n"));
        count
    }
}

/// A name the runtime may put in a statement unquoted (the Guardian's rule).
pub(super) fn is_plain_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && name.len() <= 128
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

/// One value as a SQL literal. A number is checked and left bare (empty is
/// NULL); a flag is 1 or 0; text is quoted with its quotes doubled — and, on
/// MySQL, its backslashes. A NUL is refused: no engine takes one in a literal.
pub(super) fn sql_literal(text: &str, kind: &str, mysql: bool) -> Result<String, String> {
    if text.contains('\0') {
        return Err("a NUL character cannot be written".to_owned());
    }
    match kind {
        "number" => {
            let t = text.trim();
            if t.is_empty() {
                return Ok("NULL".to_owned());
            }
            let digits = t.strip_prefix(['-', '+']).unwrap_or(t);
            let (whole, fraction) = digits.split_once('.').unwrap_or((digits, "0"));
            let plain = !whole.is_empty()
                && whole.chars().all(|c| c.is_ascii_digit())
                && !fraction.is_empty()
                && fraction.chars().all(|c| c.is_ascii_digit());
            if plain {
                Ok(t.to_owned())
            } else {
                Err(format!("'{t}' is not a number"))
            }
        }
        "boolean" => match text.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "y" => Ok("1".to_owned()),
            "0" | "false" | "no" | "n" => Ok("0".to_owned()),
            other => Err(format!("'{other}' is not a yes/no value")),
        },
        _ => {
            let mut quoted = text.replace('\'', "''");
            if mysql {
                quoted = quoted.replace('\\', "\\\\");
            }
            Ok(format!("'{quoted}'"))
        }
    }
}
