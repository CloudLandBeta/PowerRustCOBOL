// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DataGrid write-back (spec 094): a cell the operator edits in a bound grid is
//! written back to its source — an indexed file, a COBOL table or a SQL table.
//!
//! The grid itself only *shows* an edit (`Rows`, `onCellEdited`); this module is
//! the other half. The shape, end to end:
//!
//! 1. `onCellEdited` arrives on a **writable** grid (a writable binding, or a
//!    quick-bound indexed grid whose `.cidx` has a primary key, with
//!    `AllowCellEditing` on): [`Interpreter::note_grid_edit`] keeps the edit as
//!    *pending*. The handler then runs as usual and may refuse it by putting
//!    `PreviousValue` back with `SetCellValue`.
//! 2. The handler returns to the generated event loop, which calls
//!    `COBOL-WAIT-EVENT` again. **That call is "after the handler returns"**:
//!    [`Interpreter::settle_grid_edits`] runs first. With `AutoCommit` on it
//!    writes every pending edit; with it off it leaves them for the code to
//!    confirm (`CommitEdit()`) or discard (`DiscardEdit()`).
//! 3. A write finds the source row **by its key cells** in the edited row of
//!    `Rows`, read at that moment — never by row number, which a COBOL `Sort`,
//!    a `DeleteRow` or the operator's own sort would have moved. An edit to a
//!    key column is refused: a REWRITE cannot change a key.
//! 4. A failed write puts `PreviousValue` back, sets `CommitStatus` and
//!    `LastError`, and queues `onCommitError`.
//!
//! `CommitStatus` is the source's own code — an indexed file's FILE STATUS — or
//! one of ours: `00` written, `21` the key cannot change, `23` no such row, `90`
//! the driver or the file failed, `91` the grid is not writable, `92` the value
//! is not acceptable for the field, `98` edits are still pending.

use std::collections::HashMap;

use cobolt_indexed::{encode_field_display, format_field_display};

use super::Interpreter;
use crate::indexed_ide::GridSession;
use crate::value::CobolValue;

pub(super) const OK: &str = "00";
const KEY_CHANGED: &str = "21";
const NOT_FOUND: &str = "23";
const IO_ERROR: &str = "90";
const NOT_WRITABLE: &str = "91";
const REJECTED: &str = "92";
/// Left in `CommitStatus` when a refresh found edits the code has not decided.
pub(super) const PENDING: &str = "98";

/// One kept edit of one cell. Row and column are 0-based positions in `Rows`
/// (the grid's DATA row and column, not what the operator sees after sorting).
#[derive(Debug, Clone)]
pub(super) struct PendingEdit {
    pub row: usize,
    pub col: usize,
    pub value: String,
    pub previous: String,
}

/// What a grid has kept and not yet written.
#[derive(Debug, Default)]
pub(super) struct GridEditState {
    pub pending: Vec<PendingEdit>,
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
fn file_status(message: &str) -> String {
    message
        .rsplit("FILE STATUS")
        .next()
        .map(str::trim)
        .and_then(|t| t.get(..2))
        .filter(|t| t.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(IO_ERROR)
        .to_owned()
}

fn names(list: &str) -> Vec<String> {
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
    fn grid_auto_commit(&self, grid: &str) -> bool {
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

    fn set_commit_status(&mut self, grid: &str, code: &str, message: &str) {
        self.obj_set(grid, "CommitStatus", code.to_owned());
        self.obj_set(grid, "LastError", message.to_owned());
    }

    /// The cells still pending, for the renderer's marker (`row,column` pairs
    /// from 1) — shown only while the code, not the runtime, decides.
    fn publish_pending_cells(&mut self, grid: &str) {
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
    fn restore_cell(&mut self, grid: &str, row: usize, col: usize, text: &str) {
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
        let state = self.grid_edits.entry(grid.to_ascii_uppercase()).or_default();
        // A second edit of a cell still pending keeps the FIRST previous value,
        // so discarding brings back what the source holds.
        match state.pending.iter_mut().find(|e| e.row == row && e.col == col) {
            Some(edit) => edit.value = value,
            None => state.pending.push(PendingEdit { row, col, value, previous }),
        }
        self.publish_pending_cells(grid);
    }

    /// The top of `COBOL-WAIT-EVENT`: the handler of the last event has
    /// returned. Every grid on `AutoCommit` writes what it kept.
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
                self.commit_grid_edits(&grid);
            }
        }
    }

    /// Write every pending edit of `grid`; the status of the run is the last
    /// failure's, or `00`.
    fn commit_grid_edits(&mut self, grid: &str) -> String {
        let edits = match self.grid_edits.get_mut(&grid.to_ascii_uppercase()) {
            Some(state) => std::mem::take(&mut state.pending),
            None => Vec::new(),
        };
        let mut status = OK.to_owned();
        let mut message = String::new();
        for edit in edits {
            if let Err(error) = self.commit_one(grid, &edit) {
                self.restore_cell(grid, edit.row, edit.col, &edit.previous);
                // The handler of `onCommitError` reads the edit that failed.
                self.obj_set(grid, "EditedRow", (edit.row + 1).to_string());
                self.obj_set(grid, "EditedColumn", (edit.col + 1).to_string());
                self.obj_set(grid, "EditedValue", edit.value.clone());
                self.obj_set(grid, "PreviousValue", edit.previous.clone());
                status = error.code;
                message = error.message;
                self.queue_control_event(grid, "onCommitError");
            }
        }
        self.set_commit_status(grid, &status, &message);
        self.publish_pending_cells(grid);
        status
    }

    /// Write one edit to the grid's source.
    fn commit_one(&mut self, grid: &str, edit: &PendingEdit) -> Result<(), CommitError> {
        let current = self
            .datagrid_rows(grid)
            .get(edit.row)
            .and_then(|cells| cells.get(edit.col))
            .cloned()
            .unwrap_or_default();
        // A handler that put `PreviousValue` back has refused the edit.
        if current == edit.previous {
            return Ok(());
        }
        let kind = self.obj_get(grid, "_BindingKind");
        let stored = match kind.trim().to_ascii_lowercase().as_str() {
            "indexedfile" => self.write_indexed(grid, edit, &current)?,
            "coboltable" => self.write_table(grid, edit, &current)?,
            "sql" => self.write_sql(grid, edit, &current)?,
            _ => return Err(CommitError::new(NOT_WRITABLE, "the grid is not bound to a source that can be written")),
        };
        // Show what the source holds, which may not be what was typed
        // (`5` in a `9(4)` field is `0005`).
        if stored != current {
            self.restore_cell(grid, edit.row, edit.col, &stored);
        }
        Ok(())
    }

    /// `CommitEdit()`: write what is pending. Answers the status.
    pub(super) fn commit_edit_method(&mut self, grid: &str) -> String {
        let has_pending = self
            .grid_edits
            .get(&grid.to_ascii_uppercase())
            .is_some_and(|state| !state.pending.is_empty());
        if !has_pending {
            let held = self.obj_get(grid, "CommitStatus");
            return if held.trim().is_empty() { OK.to_owned() } else { held.trim().to_owned() };
        }
        self.commit_grid_edits(grid)
    }

    /// `DiscardEdit()`: put every pending cell back as the source has it.
    pub(super) fn discard_edit_method(&mut self, grid: &str) -> String {
        let edits = match self.grid_edits.get_mut(&grid.to_ascii_uppercase()) {
            Some(state) => std::mem::take(&mut state.pending),
            None => Vec::new(),
        };
        for edit in edits {
            self.restore_cell(grid, edit.row, edit.col, &edit.previous);
        }
        self.set_commit_status(grid, OK, "");
        self.publish_pending_cells(grid);
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
        for edit in &edits {
            if let Some(cell) = rows.get_mut(edit.row).and_then(|cells| cells.get_mut(edit.col)) {
                *cell = edit.value.clone();
            }
        }
        self.set_datagrid_rows(grid, &rows);
        self.set_commit_status(grid, PENDING, "edits are still pending: CommitEdit() or DiscardEdit()");
        self.publish_pending_cells(grid);
    }

    // ── The sources ──────────────────────────────────────────────────────────

    /// The bound field names, in column order: the binding's, or every leaf the
    /// `.cidx` declares (a quick-bind names none).
    fn grid_fields(&self, grid: &str, all: &[String]) -> Vec<String> {
        let named = names(&self.obj_get(grid, "_BindingFields"));
        if named.is_empty() {
            all.to_vec()
        } else {
            named
        }
    }

    /// An indexed file: find the record by the key cells of the edited row,
    /// encode the new text by the field's PICTURE, REWRITE, COMMIT.
    fn write_indexed(&mut self, grid: &str, edit: &PendingEdit, text: &str) -> Result<String, CommitError> {
        let def_path = self.obj_get(grid, "_BindingIndexedPath");
        let def_file = cobolt_forms::assets::resolve(def_path.trim());
        let def = cobolt_indexed::load_indexed(&def_file)
            .map_err(|e| CommitError::new(IO_ERROR, format!("cannot read '{}': {e:?}", def_file.display())))?;
        let leaves = def.record_root().map(|root| root.all_leaves()).unwrap_or_default();
        let every: Vec<String> = leaves.iter().map(|leaf| leaf.name.clone()).collect();
        let fields = self.grid_fields(grid, &every);
        let name = fields
            .get(edit.col)
            .ok_or_else(|| CommitError::new(NOT_WRITABLE, "that column has no field behind it"))?;
        let leaf = leaves
            .iter()
            .find(|leaf| leaf.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| CommitError::new(NOT_WRITABLE, format!("'{name}' is not a field of the record")))?;
        let keys = &def.keys.primary.parts;
        if keys.is_empty() {
            return Err(CommitError::new(NOT_WRITABLE, "the file's definition has no primary key"));
        }
        if keys.iter().any(|part| part.field_name.eq_ignore_ascii_case(name)) {
            return Err(CommitError::new(KEY_CHANGED, format!("'{name}' is part of the key; a REWRITE cannot change it")));
        }
        // The key cells of the edited row, as the grid shows them.
        let rows = self.datagrid_rows(grid);
        let cells = rows
            .get(edit.row)
            .ok_or_else(|| CommitError::new(NOT_FOUND, "the edited row is no longer in the grid"))?;
        let mut wanted = Vec::new();
        for part in keys {
            let at = fields
                .iter()
                .position(|field| field.eq_ignore_ascii_case(&part.field_name))
                .ok_or_else(|| CommitError::new(NOT_WRITABLE, format!("key field '{}' is not shown in the grid", part.field_name)))?;
            let key_leaf = leaves
                .iter()
                .find(|l| l.name.eq_ignore_ascii_case(&part.field_name))
                .ok_or_else(|| CommitError::new(NOT_WRITABLE, format!("key field '{}' is not a leaf", part.field_name)))?;
            wanted.push((part.offset as usize, part.length as usize, *key_leaf, cells.get(at).map(|c| c.trim().to_owned()).unwrap_or_default()));
        }
        let data_path = cobolt_forms::assets::resolve(&def.assign_path);
        if !data_path.exists() {
            return Err(CommitError::new("35", format!("'{}' does not exist", data_path.display())));
        }
        let mut session = GridSession::open(&def, data_path.as_path())
            .map_err(|e| CommitError::new(&file_status(&e), e))?;
        let at = session
            .rows()
            .iter()
            .position(|record| {
                wanted.iter().all(|(offset, length, key_leaf, key_text)| {
                    let end = (offset + length).min(record.len());
                    *offset <= end && format_field_display(key_leaf, &record[*offset..end]).trim() == key_text
                })
            })
            .ok_or_else(|| CommitError::new(NOT_FOUND, "the record is no longer in the file"))?;
        let (offset, length) = (leaf.offset.unwrap_or(0) as usize, leaf.length.unwrap_or(0) as usize);
        let bytes = encode_field_display(leaf, text, length)
            .map_err(|e| CommitError::new(REJECTED, format!("{name}: {}", e.as_str())))?;
        let mut record = session.rows()[at].clone();
        if offset + length > record.len() {
            record.resize(offset + length, b' ');
        }
        record[offset..offset + length].copy_from_slice(&bytes);
        if let Err(e) = session.select_row(at).and_then(|_| session.rewrite_row(&record)) {
            session.rollback();
            return Err(CommitError::new(&file_status(&e), e));
        }
        session.commit();
        Ok(format_field_display(leaf, &bytes))
    }

    /// A COBOL table of parallel OCCURS items: find the row by the key fields'
    /// cells, then store the text in the item as a MOVE would (the PICTURE
    /// applies; a number must be one).
    fn write_table(&mut self, grid: &str, edit: &PendingEdit, text: &str) -> Result<String, CommitError> {
        let fields = names(&self.obj_get(grid, "_BindingFields"));
        let keys = names(&self.obj_get(grid, "_BindingKeyFields"));
        let name = fields
            .get(edit.col)
            .ok_or_else(|| CommitError::new(NOT_WRITABLE, "that column has no field behind it"))?
            .clone();
        if keys.is_empty() {
            return Err(CommitError::new(NOT_WRITABLE, "the binding names no key field"));
        }
        if keys.iter().any(|key| key.eq_ignore_ascii_case(&name)) {
            return Err(CommitError::new(KEY_CHANGED, format!("'{name}' is a key field; it cannot be edited here")));
        }
        let rows = self.datagrid_rows(grid);
        let cells = rows
            .get(edit.row)
            .ok_or_else(|| CommitError::new(NOT_FOUND, "the edited row is no longer in the grid"))?;
        let mut wanted = Vec::new();
        for key in &keys {
            let at = fields
                .iter()
                .position(|field| field.eq_ignore_ascii_case(key))
                .ok_or_else(|| CommitError::new(NOT_WRITABLE, format!("key field '{key}' is not shown in the grid")))?;
            wanted.push((key.clone(), cells.get(at).map(|c| c.trim().to_owned()).unwrap_or_default()));
        }
        let table_rows = fields
            .iter()
            .filter_map(|field| self.env.symbol(field))
            .filter_map(|symbol| symbol.dims.last().copied())
            .max()
            .unwrap_or(0);
        let found = (1..=table_rows as i64).find(|row| {
            wanted.iter().all(|(key, key_text)| {
                self.env
                    .get(&crate::environment::subscript_key(key, &[*row]))
                    .is_some_and(|value| value.as_display_string().trim() == key_text)
            })
        });
        let Some(row) = found else {
            return Err(CommitError::new(NOT_FOUND, "no row of the table has that key"));
        };
        let item = crate::environment::subscript_key(&name, &[row]);
        if matches!(self.env.get(&item), Some(CobolValue::Numeric(_))) && text.trim().parse::<f64>().is_err() {
            return Err(CommitError::new(REJECTED, format!("{name}: '{}' is not a number", text.trim())));
        }
        self.env.set_str(&item, text);
        Ok(self
            .env
            .get(&item)
            .map(|value| value.as_display_string().trim_end().to_owned())
            .unwrap_or_else(|| text.to_owned()))
    }

    // ── SQL ──────────────────────────────────────────────────────────────────

    /// The open connection of the `SqlDatabase` control a SQL binding names:
    /// its handle, or why there is none.
    fn sql_handle(&self, grid: &str) -> Result<u32, CommitError> {
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
        let count = rows.len();
        self.obj_set(grid, "Rows", rows.join("\n"));
        count
    }

    /// A SQL table: `UPDATE <table> SET <field> = <literal> WHERE <key> = <literal>`
    /// on the control's connection. Every value is written as a literal escaped
    /// for the engine and a number is checked first — the handle's `exec` takes
    /// text, so there is nothing to bind a parameter to. The statement commits as
    /// the connection commits any statement: at once, unless the program has
    /// begun a transaction of its own, in which case the program's COMMIT ends it.
    fn write_sql(&mut self, grid: &str, edit: &PendingEdit, text: &str) -> Result<String, CommitError> {
        let handle = self.sql_handle(grid)?;
        let table = self.obj_get(grid, "_BindingTable").trim().to_owned();
        if table.is_empty() {
            return Err(CommitError::new(NOT_WRITABLE, "the binding names no table to update"));
        }
        if !is_plain_name(&table) {
            return Err(CommitError::new(NOT_WRITABLE, format!("'{table}' is not a plain table name")));
        }
        let fields = names(&self.obj_get(grid, "_BindingFields"));
        let keys = names(&self.obj_get(grid, "_BindingKeyFields"));
        let types = names(&self.obj_get(grid, "_BindingFieldTypes"));
        let column = fields
            .get(edit.col)
            .ok_or_else(|| CommitError::new(NOT_WRITABLE, "that column has no field behind it"))?
            .clone();
        if keys.is_empty() {
            return Err(CommitError::new(NOT_WRITABLE, "the binding names no key field"));
        }
        if keys.iter().any(|key| key.eq_ignore_ascii_case(&column)) {
            return Err(CommitError::new(KEY_CHANGED, format!("'{column}' is a key field; it cannot be edited here")));
        }
        let mysql = self.db.is_mysql(handle);
        let kind_of = |name: &str| {
            fields
                .iter()
                .position(|f| f.eq_ignore_ascii_case(name))
                .and_then(|i| types.get(i))
                .map_or("text", String::as_str)
                .to_owned()
        };
        let rows = self.datagrid_rows(grid);
        let cells = rows
            .get(edit.row)
            .ok_or_else(|| CommitError::new(NOT_FOUND, "the edited row is no longer in the grid"))?;
        let mut wheres = Vec::new();
        for key in &keys {
            if !is_plain_name(key) {
                return Err(CommitError::new(NOT_WRITABLE, format!("'{key}' is not a plain column name")));
            }
            let at = fields
                .iter()
                .position(|f| f.eq_ignore_ascii_case(key))
                .ok_or_else(|| CommitError::new(NOT_WRITABLE, format!("key field '{key}' is not shown in the grid")))?;
            let literal = sql_literal(cells.get(at).map_or("", |c| c.trim()), &kind_of(key), mysql)
                .map_err(|m| CommitError::new(REJECTED, format!("{key}: {m}")))?;
            wheres.push(format!("{key} = {literal}"));
        }
        if !is_plain_name(&column) {
            return Err(CommitError::new(NOT_WRITABLE, format!("'{column}' is not a plain column name")));
        }
        let value = sql_literal(text, &kind_of(&column), mysql).map_err(|m| CommitError::new(REJECTED, format!("{column}: {m}")))?;
        let statement = format!("UPDATE {table} SET {column} = {value} WHERE {}", wheres.join(" AND "));
        match self.db.exec(handle, &statement) {
            Ok(0) => Err(CommitError::new(NOT_FOUND, "no row of the table has that key")),
            Ok(_) => Ok(text.trim_end().to_owned()),
            Err(message) => Err(CommitError::new(IO_ERROR, message)),
        }
    }
}

/// A name the runtime may put in a statement unquoted (the Guardian's rule).
fn is_plain_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && name.len() <= 128
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

/// One value as a SQL literal. A number is checked and left bare (empty is
/// NULL); a flag is 1 or 0; text is quoted with its quotes doubled — and, on
/// MySQL, its backslashes. A NUL is refused: no engine takes one in a literal.
fn sql_literal(text: &str, kind: &str, mysql: bool) -> Result<String, String> {
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
