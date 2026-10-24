// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The unit of work behind `Save()` (spec 095): what a bound grid writes, and the
//! plan-then-apply write of it as one transaction.
//!
//! **A row is found by what it is, not where it sits.** The grid remembers what
//! each line of `Rows` was loaded as (`_RowOrigins`, see `grid_edit`): a table
//! occurrence, or the key cells as loaded. An edit waits through a COBOL `Sort`, a
//! `DeleteRow` or a refresh and still finds its row; a row that is no longer there
//! was deleted; a row with no origin was added.
//!
//! **What a `Save()` writes** to an indexed file or a SQL table, in this order, all
//! in one transaction:
//!
//! 1. the rows removed with `DeleteRow` — `DELETE`;
//! 2. the cells edited — one `UPDATE` / `REWRITE` per changed row;
//! 3. the rows added with `AddRow` — `INSERT` / `WRITE`.
//!
//! A blank line is never inserted, and a row whose key has been changed is never
//! updated: its key is what finds it in the source. `AutoCommit` settles the
//! cell edits alone — adds and deletes wait for the explicit `Save()`.
//!
//! **A unit is planned, then checked, then applied.** The whole job is worked out
//! first (a key cell edited, a value the PICTURE refuses, a row the source no longer
//! has); then the source is read and compared with what the grid loaded — a row
//! someone else changed since fails the unit (`96`) — and only then does anything
//! change. A COBOL table has no such check and no commit: it is **regenerated whole
//! from the grid** (every row of the grid into the occurrences, the rest blank),
//! after every value has been checked.

use std::collections::{BTreeMap, HashSet};

use cobolt_indexed::{encode_field_display, format_field_display, IndexedDefinition, IndexedField};
use cobolt_wire::controls::MAX_CHANGES;
use cobolt_wire::error::{codes, CallError, ErrorClass};
use serde_json::{json, Value};

use super::remote_binding::text_of;
use super::grid_edit::{
    file_status, is_plain_name, names, sql_literal, CommitError, LoadedRow, PendingEdit, CONFLICT, IO_ERROR,
    KEY_CHANGED, NOT_FOUND, NOT_WRITABLE, OK, REJECTED,
};
use super::Interpreter;
use crate::indexed_ide::GridSession;
use crate::value::CobolValue;

/// The connection runs in `Async` mode; a save needs the statements in order.
const ASYNC_CONNECTION: &str = "95";
/// The savepoint a save takes inside a transaction the program began.
const SAVEPOINT: &str = "prc_save";
/// Between the parts of a row's origin.
const KEY_SEPARATOR: char = '\u{1f}';

/// What kind of source a bound grid writes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Indexed,
    Table,
    Sql,
    /// The backend holds the source (WASM-005 R13): an indexed file or a SQL
    /// table a web form's grid reaches through its generated endpoints.
    Remote,
}

/// A bound grid's source, read once: the kind, the field behind each column and
/// the fields that identify a row.
pub(super) struct Source {
    pub kind: Kind,
    /// The bound field of each grid column, in column order.
    pub fields: Vec<String>,
    /// The fields that identify a row (an indexed file's primary key, a
    /// binding's key fields). Not used by a COBOL table, which is rewritten whole.
    pub keys: Vec<String>,
    /// The grid column of each key, when it is shown; the first key that is not
    /// shown is in `missing`.
    pub key_cols: Vec<usize>,
    pub missing: Option<String>,
    /// The indexed file's definition (`Kind::Indexed` only).
    pub def: Option<IndexedDefinition>,
}

impl Source {
    /// The first key field the grid does not show: a row cannot be told from its
    /// neighbours without it.
    pub(super) fn missing_key(&self) -> Option<&str> {
        self.missing.as_deref()
    }

    /// What a loaded row is: its key cells, trimmed. Never empty — an empty origin
    /// means "added since the load".
    pub(super) fn key_origin(&self, cells: &[String]) -> String {
        let mut origin = String::from("k");
        for col in &self.key_cols {
            origin.push(KEY_SEPARATOR);
            origin.push_str(cells.get(*col).map_or("", |cell| cell.trim()));
        }
        origin
    }

    /// The key values an origin was made of, in key order.
    fn key_values(&self, origin: &str) -> Vec<String> {
        origin.split(KEY_SEPARATOR).skip(1).map(str::to_owned).collect()
    }
}

/// A cell an update changes: the grid column, the source field behind it, what the
/// grid loaded there and the text the cell holds now.
struct CellChange {
    col: usize,
    field: String,
    previous: String,
    text: String,
}

/// Every changed cell of one source row.
struct UpdateRow {
    row: usize,
    origin: String,
    cells: Vec<CellChange>,
}

/// A row added in the grid, whole.
struct InsertRow {
    row: usize,
    cells: Vec<String>,
}

/// Everything one save writes.
#[derive(Default)]
struct Plan {
    updates: Vec<UpdateRow>,
    inserts: Vec<InsertRow>,
    deletes: Vec<LoadedRow>,
}

impl Plan {
    fn is_empty(&self) -> bool {
        self.updates.is_empty() && self.inserts.is_empty() && self.deletes.is_empty()
    }

    fn rows(&self) -> usize {
        self.updates.len() + self.inserts.len() + self.deletes.len()
    }
}

/// Why a unit wrote nothing: the error, the cell to blame (row and column in
/// `Rows`), and whether it was a refusal — the connection was not in a state to
/// take a save — rather than a failure of a row.
struct UnitError {
    error: CommitError,
    cell: Option<(usize, usize)>,
    refusal: bool,
}

impl UnitError {
    fn at(error: CommitError, row: usize, col: usize) -> Self {
        Self { error, cell: Some((row, col)), refusal: false }
    }

    fn refused(code: &str, message: impl Into<String>) -> Self {
        Self { error: CommitError::new(code, message), cell: None, refusal: true }
    }
}

impl From<CommitError> for UnitError {
    fn from(error: CommitError) -> Self {
        Self { error, cell: None, refusal: false }
    }
}

/// What a unit wrote, for the grid to show: `(row, column, the text the source
/// holds)` — which may not be what was typed (`5` in a `9(4)` field is `0005`).
type Written = Vec<(usize, usize, String)>;

fn blank(cells: &[String]) -> bool {
    cells.iter().all(|cell| cell.trim().is_empty())
}

/// The same value, however it is spelled: equal text, or equal numbers.
fn same_value(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim(), b.trim());
    a == b || matches!((a.parse::<f64>(), b.parse::<f64>()), (Ok(x), Ok(y)) if x == y)
}

impl Interpreter {
    /// The source the grid writes to, or why it has none.
    pub(super) fn grid_source(&self, grid: &str) -> Result<Source, CommitError> {
        let kind = self.obj_get(grid, "_BindingKind");
        // WASM-005 R13: the backend holds a remote grid's source. The seeds carry
        // what a save's plan needs — the fields behind the columns and the keys
        // — so neither the `.cidx` nor a connection is read here.
        if self.binding_is_remote(grid) {
            let fields = names(&self.obj_get(grid, "_BindingFields"));
            if fields.is_empty() {
                return Err(CommitError::new(NOT_WRITABLE, "the remote binding names no fields"));
            }
            // The keys of an indexed file are its `.cidx` primary key, every part
            // of it, as the desktop arm below and the generated SAVE take them (a
            // binding records at most one key field, and the Designer may record
            // none); the binding's own key fields stand in when the definition
            // cannot be read, and are the keys of a SQL source.
            let mut keys = names(&self.obj_get(grid, "_BindingKeyFields"));
            if kind.trim().eq_ignore_ascii_case("indexedfile") {
                let def_path = self.obj_get(grid, "_BindingIndexedPath");
                if !def_path.trim().is_empty() {
                    if let Ok(def) = cobolt_indexed::load_indexed(&cobolt_forms::assets::resolve(def_path.trim())) {
                        let primary: Vec<String> = def.keys.primary.parts.iter().map(|part| part.field_name.clone()).collect();
                        if !primary.is_empty() {
                            keys = primary;
                        }
                    }
                }
            }
            return Ok(Self::source_of(Kind::Remote, fields, keys, None));
        }
        let (kind, fields, keys, def) = match kind.trim().to_ascii_lowercase().as_str() {
            "indexedfile" => {
                let def_path = self.obj_get(grid, "_BindingIndexedPath");
                let def_file = cobolt_forms::assets::resolve(def_path.trim());
                let def = cobolt_indexed::load_indexed(&def_file)
                    .map_err(|e| CommitError::new(IO_ERROR, format!("cannot read '{}': {e:?}", def_file.display())))?;
                let every: Vec<String> = def
                    .record_root()
                    .map(|root| root.all_leaves())
                    .unwrap_or_default()
                    .iter()
                    .map(|leaf| leaf.name.clone())
                    .collect();
                // The binding names its fields; a quick-bind names none and shows every leaf.
                let named = names(&self.obj_get(grid, "_BindingFields"));
                let fields = if named.is_empty() { every } else { named };
                let keys: Vec<String> = def.keys.primary.parts.iter().map(|part| part.field_name.clone()).collect();
                (Kind::Indexed, fields, keys, Some(def))
            }
            "sql" => (
                Kind::Sql,
                names(&self.obj_get(grid, "_BindingFields")),
                names(&self.obj_get(grid, "_BindingKeyFields")),
                None,
            ),
            "coboltable" => (Kind::Table, names(&self.obj_get(grid, "_BindingFields")), Vec::new(), None),
            _ => return Err(CommitError::new(NOT_WRITABLE, "the grid is not bound to a source that can be written")),
        };
        Ok(Self::source_of(kind, fields, keys, def))
    }

    /// A [`Source`] from its parts: which grid column each key is shown in, and
    /// the first key that is not shown.
    fn source_of(kind: Kind, fields: Vec<String>, keys: Vec<String>, def: Option<IndexedDefinition>) -> Source {
        let mut key_cols = Vec::new();
        let mut missing = None;
        for key in &keys {
            match fields.iter().position(|field| field.eq_ignore_ascii_case(key)) {
                Some(at) => key_cols.push(at),
                None => {
                    missing.get_or_insert_with(|| key.clone());
                }
            }
        }
        Source { kind, fields, keys, key_cols, missing, def }
    }

    // ── The unit ─────────────────────────────────────────────────────────────

    /// Write what the grid holds as one unit, all or nothing. `explicit` is `Save()`
    /// — cell edits, added rows and deleted rows; `AutoCommit` at the end of a handler
    /// settles the cell edits alone. Answers the status the grid's `CommitStatus` now
    /// holds.
    pub(super) fn commit_unit(&mut self, grid: &str, explicit: bool) -> String {
        // A grid that does not write back keeps nothing and writes nothing; neither
        // does one that has never been loaded, whose rows are not the source's.
        let captured = self.grid_edits.get(&grid.to_ascii_uppercase()).is_some_and(|state| state.captured);
        if self.obj_get(grid, "_BindingWritable").trim() != "1" || !captured {
            return self.finish_ok(grid, 0);
        }
        let source = match self.grid_source(grid) {
            Ok(source) => source,
            Err(error) => return self.unit_failed(grid, UnitError::from(error), explicit),
        };
        match source.kind {
            Kind::Table => self.commit_table(grid, &source, explicit),
            Kind::Indexed | Kind::Sql | Kind::Remote => self.commit_rows(grid, &source, explicit),
        }
    }

    fn pending_of(&self, grid: &str) -> Vec<PendingEdit> {
        self.grid_edits.get(&grid.to_ascii_uppercase()).map(|state| state.pending.clone()).unwrap_or_default()
    }

    /// An indexed file or a SQL table: work out the plan, then apply it.
    fn commit_rows(&mut self, grid: &str, source: &Source, explicit: bool) -> String {
        let pending = self.pending_of(grid);
        // What an `AutoCommit` settle has to decide is the edits of loaded rows; an edit on a
        // row the program added waits for the explicit save, and leaves the status alone.
        if !explicit && pending.iter().all(|edit| edit.origin.is_empty()) {
            return OK.to_owned();
        }
        if let Some(key) = source.missing_key() {
            // Without the key cells a row cannot be told from its neighbours: an
            // edit cannot find its row, an added row cannot be told from a loaded one.
            return match pending.first() {
                None => self.finish_ok(grid, 0),
                Some(edit) => self.unit_failed(
                    grid,
                    UnitError::at(
                        CommitError::new(NOT_WRITABLE, format!("key field '{key}' is not shown in the grid")),
                        edit.row,
                        edit.col,
                    ),
                    explicit,
                ),
            };
        }
        let plan = match self.plan_rows(grid, source, &pending, explicit) {
            Ok(plan) => plan,
            Err(error) => return self.unit_failed(grid, error, explicit),
        };
        if plan.is_empty() {
            // Nothing differs (a handler put every edit back): the edits are decided.
            if let Some(state) = self.grid_edits.get_mut(&grid.to_ascii_uppercase()) {
                if explicit {
                    state.pending.clear();
                } else {
                    state.pending.retain(|edit| edit.origin.is_empty());
                }
            }
            return self.finish_ok(grid, 0);
        }
        let outcome = match source.kind {
            Kind::Indexed => self.apply_indexed(source, &plan),
            Kind::Remote => self.apply_remote(grid, source, &plan),
            _ => self.apply_sql(grid, source, &plan),
        };
        match outcome {
            Ok(written) => self.unit_written(grid, source, &plan, written, explicit),
            Err(error) => self.unit_failed(grid, error, explicit),
        }
    }

    /// What this save has to write: the edited cells found on their rows, and — for
    /// an explicit save — the rows added and the rows removed since the load.
    fn plan_rows(&self, grid: &str, source: &Source, pending: &[PendingEdit], explicit: bool) -> Result<Plan, UnitError> {
        let rows = self.datagrid_rows(grid);
        let origins = self.row_origins(grid);
        let state = self.grid_edits.get(&grid.to_ascii_uppercase());
        let mut plan = Plan::default();

        // The cells edited. A row added in the grid has no origin: its cells go
        // with the insert, not as an update.
        let mut by_row: BTreeMap<usize, UpdateRow> = BTreeMap::new();
        for edit in pending.iter().filter(|edit| !edit.origin.is_empty()) {
            let at = |error: CommitError| UnitError::at(error, edit.row, edit.col);
            let row = self
                .locate_edit(grid, edit)
                .filter(|row| *row < rows.len())
                .ok_or_else(|| at(CommitError::new(NOT_FOUND, "the edited row is no longer in the grid")))?;
            let current = rows[row].get(edit.col).cloned().unwrap_or_default();
            // A handler that put `PreviousValue` back has refused the edit.
            if current == edit.previous {
                continue;
            }
            let field = source.fields.get(edit.col).cloned().unwrap_or_default();
            if field.is_empty() {
                return Err(UnitError::at(CommitError::new(NOT_WRITABLE, "that column has no field behind it"), row, edit.col));
            }
            if source.key_cols.contains(&edit.col) {
                return Err(UnitError::at(
                    CommitError::new(KEY_CHANGED, format!("'{field}' is part of the key; it cannot be changed here")),
                    row,
                    edit.col,
                ));
            }
            by_row
                .entry(row)
                .or_insert_with(|| UpdateRow { row, origin: edit.origin.clone(), cells: Vec::new() })
                .cells
                .push(CellChange { col: edit.col, field, previous: edit.previous.clone(), text: current });
        }
        // A row whose key cells are not what it was loaded with is never updated: the
        // key is what finds it.
        for update in by_row.values() {
            let was = source.key_values(&update.origin);
            let moved = source
                .key_cols
                .iter()
                .enumerate()
                .find(|(i, col)| rows[update.row].get(**col).map_or("", |c| c.trim()) != was.get(*i).map_or("", String::as_str));
            if let Some((_, col)) = moved {
                return Err(UnitError::at(
                    CommitError::new(KEY_CHANGED, "the key of the row was changed; a row is found by its key"),
                    update.row,
                    *col,
                ));
            }
        }
        plan.updates = by_row.into_values().collect();

        if explicit {
            let loaded = state.map(|s| s.loaded.clone()).unwrap_or_default();
            // Two rows with one key cannot be told apart: nothing that depends on it is safe.
            let ambiguous = state.is_some_and(|s| s.ambiguous);
            let present: HashSet<&str> = origins.iter().map(String::as_str).filter(|o| !o.is_empty()).collect();
            plan.deletes = loaded.into_iter().filter(|row| !present.contains(row.origin.as_str())).collect();
            for (row, cells) in rows.iter().enumerate() {
                let added = origins.get(row).map_or(true, String::is_empty);
                if added && !blank(cells) {
                    plan.inserts.push(InsertRow { row, cells: cells.clone() });
                }
            }
            if ambiguous && (!plan.deletes.is_empty() || !plan.updates.is_empty()) {
                return Err(UnitError::from(CommitError::new(
                    NOT_WRITABLE,
                    "two rows of the grid have the same key; rows cannot be told apart",
                )));
            }
        }
        Ok(plan)
    }

    /// The unit was written: show what the source holds, make what the grid
    /// holds the new "as loaded", and say so.
    fn unit_written(&mut self, grid: &str, source: &Source, plan: &Plan, written: Written, explicit: bool) -> String {
        let mut rows = self.datagrid_rows(grid);
        for (row, col, text) in &written {
            if let Some(cell) = rows.get_mut(*row).and_then(|cells| cells.get_mut(*col)) {
                *cell = text.clone();
            }
        }
        self.set_datagrid_rows(grid, &rows);
        let upper = grid.to_ascii_uppercase();
        if explicit {
            // Everything is decided: the grid is the source now, and the rows just
            // inserted are rows of it — found, from here on, by their keys.
            let origins: Vec<String> =
                rows.iter().map(|cells| if blank(cells) { String::new() } else { source.key_origin(cells) }).collect();
            self.obj_set(grid, "_RowOrigins", origins.join("\n"));
            let mut seen = HashSet::new();
            let ambiguous = origins.iter().filter(|o| !o.is_empty()).any(|o| !seen.insert(o.clone()));
            let state = self.grid_edits.entry(upper).or_default();
            state.pending.clear();
            state.ambiguous = ambiguous;
            state.loaded = origins
                .into_iter()
                .zip(rows)
                .filter(|(origin, _)| !origin.is_empty())
                .map(|(origin, cells)| LoadedRow { origin, cells })
                .collect();
        } else {
            // Only cells were written: the loaded rows take their new values, and
            // what waits for an explicit `Save()` (an edit on an added row) stays.
            let state = self.grid_edits.entry(upper).or_default();
            for update in &plan.updates {
                if let Some(loaded) = state.loaded.iter_mut().find(|row| row.origin == update.origin) {
                    for cell in &update.cells {
                        let shown = written
                            .iter()
                            .find(|(row, col, _)| *row == update.row && *col == cell.col)
                            .map_or(cell.text.as_str(), |(_, _, text)| text.as_str());
                        if let Some(slot) = loaded.cells.get_mut(cell.col) {
                            *slot = shown.to_owned();
                        }
                    }
                }
            }
            state.pending.retain(|edit| edit.origin.is_empty());
        }
        self.finish_ok(grid, plan.rows())
    }

    /// Say a unit is done (or that there was nothing to write): `00`, the count of
    /// rows written, and `onSaved` when there was one.
    fn finish_ok(&mut self, grid: &str, rows_written: usize) -> String {
        self.obj_set(grid, "_FailedCells", String::new());
        self.set_commit_status(grid, OK, "");
        self.obj_set(grid, "SavedRows", rows_written.to_string());
        self.publish_pending_cells(grid);
        if rows_written > 0 {
            self.queue_control_event(grid, "onSaved");
        }
        OK.to_owned()
    }

    /// The unit wrote nothing. A refusal leaves everything waiting as it was. A
    /// failure raises `onCommitError` once and **puts the old values back**: an
    /// explicit `Save()` restores the grid to what was last loaded or saved (cells,
    /// added rows and deleted rows); `AutoCommit`, which only ever had cell edits to
    /// write, restores those cells. The cell that failed carries a mark until the
    /// operator edits again.
    fn unit_failed(&mut self, grid: &str, failure: UnitError, explicit: bool) -> String {
        let UnitError { error, cell, refusal } = failure;
        if !refusal {
            let rows = self.datagrid_rows(grid);
            let origins = self.row_origins(grid);
            let pending = self.pending_of(grid);
            // The handler of `onCommitError` reads the edit that failed.
            let mut mark: Option<(String, usize)> = None;
            if let Some((row, col)) = cell {
                let current = rows.get(row).and_then(|cells| cells.get(col)).cloned().unwrap_or_default();
                let previous = pending
                    .iter()
                    .find(|edit| edit.row == row && edit.col == col)
                    .map(|edit| edit.previous.clone())
                    .or_else(|| {
                        let origin = origins.get(row).filter(|o| !o.is_empty())?;
                        let state = self.grid_edits.get(&grid.to_ascii_uppercase())?;
                        state.loaded.iter().find(|r| &r.origin == origin)?.cells.get(col).cloned()
                    })
                    .unwrap_or_default();
                self.obj_set(grid, "EditedRow", (row + 1).to_string());
                self.obj_set(grid, "EditedColumn", (col + 1).to_string());
                self.obj_set(grid, "EditedValue", current);
                self.obj_set(grid, "PreviousValue", previous);
                if let Some(origin) = origins.get(row).filter(|o| !o.is_empty()) {
                    mark = Some((origin.clone(), col));
                }
            }
            if explicit {
                self.restore_loaded(grid);
            } else {
                // The cell edits go back as they were; an edit waiting on an added row stays.
                for edit in pending.iter().filter(|edit| !edit.origin.is_empty()) {
                    if let Some(row) = self.locate_edit(grid, edit) {
                        self.restore_cell(grid, row, edit.col, &edit.previous);
                    }
                }
                if let Some(state) = self.grid_edits.get_mut(&grid.to_ascii_uppercase()) {
                    state.pending.retain(|edit| edit.origin.is_empty());
                }
            }
            // The mark goes where the row is now.
            let marker = mark
                .and_then(|(origin, col)| {
                    self.row_origins(grid).iter().position(|o| *o == origin).map(|row| format!("{},{}", row + 1, col + 1))
                })
                .unwrap_or_default();
            self.obj_set(grid, "_FailedCells", marker);
            self.queue_control_event(grid, "onCommitError");
        }
        self.set_commit_status(grid, &error.code, &error.message);
        self.obj_set(grid, "SavedRows", "0".to_owned());
        self.publish_pending_cells(grid);
        error.code
    }

    // ── An indexed file ──────────────────────────────────────────────────────

    /// One session for the whole unit. Everything is checked against the file as it
    /// is — each record found, each edited field and each deleted row still what the
    /// grid loaded, each value acceptable to its field's PICTURE — before the first
    /// change; then the deletes, the rewrites and the inserts, and one COMMIT — or,
    /// on any error, ROLLBACK.
    fn apply_indexed(&mut self, source: &Source, plan: &Plan) -> Result<Written, UnitError> {
        let first = plan
            .updates
            .first()
            .map(|u| (u.row, u.cells[0].col))
            .or_else(|| plan.inserts.first().map(|i| (i.row, 0)));
        let general = move |error: CommitError| match first {
            Some((row, col)) => UnitError::at(error, row, col),
            None => UnitError::from(error),
        };
        let Some(def) = source.def.as_ref() else {
            return Err(general(CommitError::new(NOT_WRITABLE, "the grid has no indexed file definition")));
        };
        let leaves = def.record_root().map(|root| root.all_leaves()).unwrap_or_default();
        let keys = &def.keys.primary.parts;
        if keys.is_empty() {
            return Err(general(CommitError::new(NOT_WRITABLE, "the file's definition has no primary key")));
        }
        let leaf_of = |name: &str| leaves.iter().find(|leaf| leaf.name.eq_ignore_ascii_case(name)).copied();
        // The key leaves, in the order of the key.
        let mut key_leaves: Vec<(usize, usize, &IndexedField)> = Vec::new();
        for part in keys {
            let leaf = leaf_of(&part.field_name)
                .ok_or_else(|| general(CommitError::new(NOT_WRITABLE, format!("key field '{}' is not a leaf", part.field_name))))?;
            key_leaves.push((part.offset as usize, part.length as usize, leaf));
        }
        let display = |leaf: &IndexedField, record: &[u8]| -> String {
            let start = (leaf.offset.unwrap_or(0) as usize).min(record.len());
            let end = (start + leaf.length.unwrap_or(0) as usize).min(record.len());
            format_field_display(leaf, &record[start..end])
        };
        let record_len = match def.record_format {
            cobolt_indexed::RecordFormatDef::Fixed { length } => length as usize,
            cobolt_indexed::RecordFormatDef::Variable { max_length, .. } => max_length as usize,
        };

        // ── Plan: encode everything, before the file is opened ───────────────
        struct Rewrite {
            row: usize,
            col: usize,
            origin: String,
            /// `(leaf, previous text, new bytes)` of each edited field.
            fields: Vec<(IndexedField, usize, String, Vec<u8>)>,
        }
        let mut written: Written = Vec::new();
        let mut rewrites: Vec<Rewrite> = Vec::new();
        for update in &plan.updates {
            let mut fields = Vec::new();
            for cell in &update.cells {
                let blame = |error: CommitError| UnitError::at(error, update.row, cell.col);
                let leaf = leaf_of(&cell.field)
                    .ok_or_else(|| blame(CommitError::new(NOT_WRITABLE, format!("'{}' is not a field of the record", cell.field))))?;
                let length = leaf.length.unwrap_or(0) as usize;
                let bytes = encode_field_display(leaf, &cell.text, length)
                    .map_err(|e| blame(CommitError::new(REJECTED, format!("{}: {}", cell.field, e.as_str()))))?;
                written.push((update.row, cell.col, format_field_display(leaf, &bytes)));
                fields.push((leaf.clone(), cell.col, cell.previous.clone(), bytes));
            }
            rewrites.push(Rewrite { row: update.row, col: update.cells[0].col, origin: update.origin.clone(), fields });
        }
        let mut new_records: Vec<(usize, Vec<u8>)> = Vec::new();
        for insert in &plan.inserts {
            let mut record = vec![b' '; record_len];
            // A field the grid does not show starts as its PICTURE's own empty value.
            for leaf in &leaves {
                let (offset, length) = (leaf.offset.unwrap_or(0) as usize, leaf.length.unwrap_or(0) as usize);
                if let Ok(bytes) = encode_field_display(leaf, "", length) {
                    if offset + bytes.len() <= record.len() {
                        record[offset..offset + bytes.len()].copy_from_slice(&bytes);
                    }
                }
            }
            for (col, field) in source.fields.iter().enumerate() {
                let blame = |error: CommitError| UnitError::at(error, insert.row, col);
                let leaf = leaf_of(field)
                    .ok_or_else(|| blame(CommitError::new(NOT_WRITABLE, format!("'{field}' is not a field of the record"))))?;
                let text = insert.cells.get(col).map_or("", String::as_str);
                if source.key_cols.contains(&col) && text.trim().is_empty() {
                    return Err(blame(CommitError::new(REJECTED, format!("'{field}' is part of the key; a new row needs it"))));
                }
                let (offset, length) = (leaf.offset.unwrap_or(0) as usize, leaf.length.unwrap_or(0) as usize);
                let bytes = encode_field_display(leaf, text, length)
                    .map_err(|e| blame(CommitError::new(REJECTED, format!("{field}: {}", e.as_str()))))?;
                if offset + bytes.len() > record.len() {
                    record.resize(offset + bytes.len(), b' ');
                }
                record[offset..offset + bytes.len()].copy_from_slice(&bytes);
                written.push((insert.row, col, format_field_display(leaf, &bytes)));
            }
            new_records.push((insert.row, record));
        }

        // ── Open, find, check ────────────────────────────────────────────────
        let data_path = cobolt_forms::assets::resolve(&def.assign_path);
        if !data_path.exists() {
            return Err(general(CommitError::new("35", format!("'{}' does not exist", data_path.display()))));
        }
        let mut session =
            GridSession::open(def, data_path.as_path()).map_err(|e| general(CommitError::new(&file_status(&e), e)))?;
        let find = |session: &GridSession, origin: &str| -> Option<usize> {
            let wanted = source.key_values(origin);
            session.rows().iter().position(|record| {
                key_leaves.iter().zip(&wanted).all(|((offset, length, leaf), text)| {
                    let end = (offset + length).min(record.len());
                    *offset <= end && format_field_display(leaf, &record[*offset..end]).trim() == text
                })
            })
        };
        for delete in &plan.deletes {
            let at = find(&session, &delete.origin)
                .ok_or_else(|| UnitError::from(CommitError::new(NOT_FOUND, "a deleted row is no longer in the file")))?;
            for (col, field) in source.fields.iter().enumerate() {
                let Some(leaf) = leaf_of(field) else { continue };
                let now = display(leaf, &session.rows()[at]);
                if !same_value(&now, delete.cells.get(col).map_or("", String::as_str)) {
                    return Err(UnitError::from(CommitError::new(
                        CONFLICT,
                        format!("the row was changed by someone else since it was loaded ({field})"),
                    )));
                }
            }
        }
        for rewrite in &rewrites {
            let at = find(&session, &rewrite.origin).ok_or_else(|| {
                UnitError::at(CommitError::new(NOT_FOUND, "the record is no longer in the file"), rewrite.row, rewrite.col)
            })?;
            for (leaf, col, previous, _) in &rewrite.fields {
                if !same_value(&display(leaf, &session.rows()[at]), previous) {
                    return Err(UnitError::at(
                        CommitError::new(
                            CONFLICT,
                            format!("'{}' was changed by someone else since it was loaded", leaf.name),
                        ),
                        rewrite.row,
                        *col,
                    ));
                }
            }
        }

        // ── Apply: deletes, rewrites, inserts ────────────────────────────────
        for delete in &plan.deletes {
            let outcome = find(&session, &delete.origin)
                .ok_or_else(|| "the record is gone".to_owned())
                .and_then(|at| session.select_row(at))
                .and_then(|_| session.delete_current());
            if let Err(e) = outcome {
                session.rollback();
                return Err(UnitError::from(CommitError::new(&file_status(&e), e)));
            }
        }
        for rewrite in &rewrites {
            let outcome = find(&session, &rewrite.origin).ok_or_else(|| "the record is gone".to_owned()).and_then(|at| {
                let mut record = session.rows()[at].clone();
                for (leaf, _, _, bytes) in &rewrite.fields {
                    let offset = leaf.offset.unwrap_or(0) as usize;
                    if offset + bytes.len() > record.len() {
                        record.resize(offset + bytes.len(), b' ');
                    }
                    record[offset..offset + bytes.len()].copy_from_slice(bytes);
                }
                session.select_row(at).and_then(|_| session.rewrite_row(&record))
            });
            if let Err(e) = outcome {
                session.rollback();
                return Err(UnitError::at(CommitError::new(&file_status(&e), e), rewrite.row, rewrite.col));
            }
        }
        for (row, record) in &new_records {
            if let Err(e) = session.write_row(record) {
                session.rollback();
                return Err(UnitError::at(CommitError::new(&file_status(&e), e), *row, 0));
            }
        }
        session.commit();
        Ok(written)
    }

    // ── A COBOL table ────────────────────────────────────────────────────────

    /// A COBOL table of parallel OCCURS items has no commit and no key, so the
    /// table is **regenerated from the grid** (spec 095, Q2): every row of the grid
    /// goes into the occurrences in the grid's order, and the occurrences after the
    /// last row are blanked. Sorting, adding and deleting rows in the grid become the
    /// table. A blank row is not written. Every value is checked as a `MOVE` would
    /// before anything is assigned, so one bad value assigns none. Items of the
    /// occurrence the grid does not show are left where they are.
    fn commit_table(&mut self, grid: &str, source: &Source, explicit: bool) -> String {
        let pending = self.pending_of(grid);
        let rows = self.datagrid_rows(grid);
        let origins = self.row_origins(grid);
        let loaded: Vec<LoadedRow> =
            self.grid_edits.get(&grid.to_ascii_uppercase()).map(|s| s.loaded.clone()).unwrap_or_default();
        let no_origin = String::new();
        let shown: Vec<(&Vec<String>, &String)> = rows
            .iter()
            .zip(origins.iter().chain(std::iter::repeat(&no_origin)))
            .filter(|(cells, _)| !blank(cells))
            .collect();
        // Something to write: an edit is waiting, or (an explicit save) the grid is
        // no longer the table it was loaded as.
        let unchanged = shown.len() == loaded.len()
            && shown.iter().zip(&loaded).all(|(entry, was)| *entry.0 == was.cells && *entry.1 == was.origin);
        if pending.is_empty() && (!explicit || unchanged) {
            return self.finish_ok(grid, 0);
        }
        let capacity = source
            .fields
            .iter()
            .filter_map(|field| self.env.symbol(field))
            .filter_map(|symbol| symbol.dims.last().copied())
            .max()
            .unwrap_or(0);
        if shown.len() > capacity {
            return self.unit_failed(
                grid,
                UnitError::from(CommitError::new(
                    REJECTED,
                    format!("the table has room for {capacity} rows; the grid has {}", shown.len()),
                )),
                explicit,
            );
        }
        // Check everything first.
        let at_row = |data_index: usize| rows.iter().enumerate().filter(|(_, c)| !blank(c)).nth(data_index).map_or(0, |(i, _)| i);
        for (i, (cells, _)) in shown.iter().enumerate() {
            for (col, field) in source.fields.iter().enumerate() {
                if field.is_empty() {
                    continue;
                }
                let text = cells.get(col).map_or("", |c| c.trim());
                let item = crate::environment::subscript_key(field, &[i as i64 + 1]);
                if !text.is_empty()
                    && matches!(self.env.get(&item), Some(CobolValue::Numeric(_)))
                    && text.parse::<f64>().is_err()
                {
                    return self.unit_failed(
                        grid,
                        UnitError::at(CommitError::new(REJECTED, format!("{field}: '{text}' is not a number")), at_row(i), col),
                        explicit,
                    );
                }
            }
        }
        // Assign: the rows of the grid, then blank for the rest.
        let mut regenerated: Vec<Vec<String>> = Vec::with_capacity(shown.len());
        for occurrence in 1..=capacity {
            for (col, field) in source.fields.iter().enumerate() {
                if field.is_empty() {
                    continue;
                }
                let item = crate::environment::subscript_key(field, &[occurrence as i64]);
                match shown.get(occurrence - 1) {
                    Some((cells, _)) => self.env.set_str(&item, cells.get(col).map_or("", |c| c.trim_end())),
                    None if matches!(self.env.get(&item), Some(CobolValue::Numeric(_))) => {
                        self.env.set(&item, CobolValue::from_i64(0));
                    }
                    None => self.env.set_str(&item, ""),
                }
            }
            if occurrence <= shown.len() {
                // What the table holds now (`5` in a `9(4)` item is `0005`).
                regenerated.push(
                    source
                        .fields
                        .iter()
                        .map(|field| {
                            let item = crate::environment::subscript_key(field, &[occurrence as i64]);
                            self.env.get(&item).map(|v| v.as_display_string().trim_end().to_owned()).unwrap_or_default()
                        })
                        .collect(),
                );
            }
        }
        let count = regenerated.len();
        self.set_datagrid_rows(grid, &regenerated);
        let origins: Vec<String> = (1..=count).map(|n| n.to_string()).collect();
        self.obj_set(grid, "_RowOrigins", origins.join("\n"));
        let state = self.grid_edits.entry(grid.to_ascii_uppercase()).or_default();
        state.pending.clear();
        state.loaded = origins.into_iter().zip(regenerated).map(|(origin, cells)| LoadedRow { origin, cells }).collect();
        self.finish_ok(grid, count)
    }

    // ── A SQL table ──────────────────────────────────────────────────────────

    /// The control's connection takes the unit in a transaction: `BEGIN` …
    /// `COMMIT`, with `ROLLBACK` on any error; when the program has a transaction
    /// open on the connection (`BeginTransaction()`), the save joins it under a
    /// `SAVEPOINT` and leaves the commit to the program — a failure undoes the
    /// save's own writes and nothing else. The whole job is built before anything is
    /// sent (every value a literal escaped for the engine, since the handle's `exec`
    /// takes text), then each row to update or delete is read and compared with what
    /// the grid loaded (`96` when it differs), then `DELETE`s, `UPDATE`s and `INSERT`s.
    fn apply_sql(&mut self, grid: &str, source: &Source, plan: &Plan) -> Result<Written, UnitError> {
        let first = plan
            .updates
            .first()
            .map(|u| (u.row, u.cells[0].col))
            .or_else(|| plan.inserts.first().map(|i| (i.row, 0)));
        let general = move |error: CommitError| match first {
            Some((row, col)) => UnitError::at(error, row, col),
            None => UnitError::from(error),
        };
        let handle = self.sql_handle(grid).map_err(general)?;
        let control = self.obj_get(grid, "_BindingSqlControl");
        if self.sql_is_async(control.trim()) {
            return Err(UnitError::refused(
                ASYNC_CONNECTION,
                "the connection is in Async mode; a save needs a Sync connection",
            ));
        }
        let table = self.obj_get(grid, "_BindingTable").trim().to_owned();
        if table.is_empty() {
            return Err(general(CommitError::new(NOT_WRITABLE, "the binding names no table to update")));
        }
        if !is_plain_name(&table) {
            return Err(general(CommitError::new(NOT_WRITABLE, format!("'{table}' is not a plain table name"))));
        }
        if source.keys.is_empty() {
            return Err(general(CommitError::new(NOT_WRITABLE, "the binding names no key field")));
        }
        for field in source.fields.iter().chain(&source.keys) {
            if !is_plain_name(field) {
                return Err(general(CommitError::new(NOT_WRITABLE, format!("'{field}' is not a plain column name"))));
            }
        }
        let types = names(&self.obj_get(grid, "_BindingFieldTypes"));
        let mysql = self.db.is_mysql(handle);
        let kind_of = |name: &str| {
            source
                .fields
                .iter()
                .position(|f| f.eq_ignore_ascii_case(name))
                .and_then(|i| types.get(i))
                .map_or("text", String::as_str)
                .to_owned()
        };
        let where_of = |origin: &str, blame: &dyn Fn(CommitError) -> UnitError| -> Result<String, UnitError> {
            let values = source.key_values(origin);
            let mut parts = Vec::new();
            for (key, text) in source.keys.iter().zip(&values) {
                let literal = sql_literal(text, &kind_of(key), mysql)
                    .map_err(|m| blame(CommitError::new(REJECTED, format!("{key}: {m}"))))?;
                parts.push(format!("{key} = {literal}"));
            }
            Ok(parts.join(" AND "))
        };

        // ── Build ────────────────────────────────────────────────────────────
        /// A row to read back and compare: the SELECT, the columns it asks for and
        /// what the grid loaded in each, and the cell to blame.
        struct Check {
            select: String,
            expect: Vec<(String, String, Option<usize>)>,
            row: Option<usize>,
            gone: &'static str,
        }
        let mut checks: Vec<Check> = Vec::new();
        let mut deletes: Vec<String> = Vec::new();
        let mut updates: Vec<(usize, usize, String)> = Vec::new();
        let mut inserts: Vec<(usize, String)> = Vec::new();
        let mut written: Written = Vec::new();
        for delete in &plan.deletes {
            let blame = |error: CommitError| UnitError::from(error);
            let clause = where_of(&delete.origin, &blame)?;
            checks.push(Check {
                select: format!("SELECT {} FROM {table} WHERE {clause}", source.fields.join(", ")),
                expect: source
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(col, field)| (field.clone(), delete.cells.get(col).cloned().unwrap_or_default(), None))
                    .collect(),
                row: None,
                gone: "a deleted row is no longer in the table",
            });
            deletes.push(format!("DELETE FROM {table} WHERE {clause}"));
        }
        for update in &plan.updates {
            let col0 = update.cells[0].col;
            let blame_row = |error: CommitError| UnitError::at(error, update.row, col0);
            let clause = where_of(&update.origin, &blame_row)?;
            let mut sets = Vec::new();
            for cell in &update.cells {
                let literal = sql_literal(&cell.text, &kind_of(&cell.field), mysql)
                    .map_err(|m| UnitError::at(CommitError::new(REJECTED, format!("{}: {m}", cell.field)), update.row, cell.col))?;
                sets.push(format!("{} = {literal}", cell.field));
                written.push((update.row, cell.col, cell.text.trim_end().to_owned()));
            }
            checks.push(Check {
                select: format!(
                    "SELECT {} FROM {table} WHERE {clause}",
                    update.cells.iter().map(|c| c.field.as_str()).collect::<Vec<_>>().join(", ")
                ),
                expect: update.cells.iter().map(|c| (c.field.clone(), c.previous.clone(), Some(c.col))).collect(),
                row: Some(update.row),
                gone: "no row of the table has that key",
            });
            updates.push((update.row, col0, format!("UPDATE {table} SET {} WHERE {clause}", sets.join(", "))));
        }
        for insert in &plan.inserts {
            let (mut columns, mut values) = (Vec::new(), Vec::new());
            for (col, field) in source.fields.iter().enumerate() {
                let text = insert.cells.get(col).map_or("", String::as_str);
                let blame = |error: CommitError| UnitError::at(error, insert.row, col);
                if text.trim().is_empty() {
                    if source.key_cols.contains(&col) {
                        return Err(blame(CommitError::new(REJECTED, format!("'{field}' is part of the key; a new row needs it"))));
                    }
                    continue;
                }
                let literal = sql_literal(text, &kind_of(field), mysql)
                    .map_err(|m| blame(CommitError::new(REJECTED, format!("{field}: {m}"))))?;
                columns.push(field.clone());
                values.push(literal);
                written.push((insert.row, col, text.trim_end().to_owned()));
            }
            inserts.push((insert.row, format!("INSERT INTO {table} ({}) VALUES ({})", columns.join(", "), values.join(", "))));
        }

        // ── Run: begin, check, delete, update, insert, commit ────────────────
        let joined = self.db.in_transaction(handle);
        let (begin, commit) = if joined {
            (format!("SAVEPOINT {SAVEPOINT}"), format!("RELEASE SAVEPOINT {SAVEPOINT}"))
        } else {
            ("BEGIN".to_owned(), "COMMIT".to_owned())
        };
        self.db.exec(handle, &begin).map_err(|m| general(CommitError::new(IO_ERROR, m)))?;
        let abort = |this: &mut Self| {
            if joined {
                let _ = this.db.exec(handle, &format!("ROLLBACK TO SAVEPOINT {SAVEPOINT}"));
                let _ = this.db.exec(handle, &format!("RELEASE SAVEPOINT {SAVEPOINT}"));
            } else {
                let _ = this.db.exec(handle, "ROLLBACK");
            }
        };
        for check in &checks {
            let at = |error: CommitError| match (check.row, check.expect.first()) {
                (Some(row), Some((_, _, Some(col)))) => UnitError::at(error, row, *col),
                _ => UnitError::from(error),
            };
            if let Err(message) = self.db.exec(handle, &check.select) {
                abort(self);
                return Err(at(CommitError::new(IO_ERROR, message)));
            }
            let Some(values) = self.db.take_row(handle) else {
                abort(self);
                return Err(at(CommitError::new(NOT_FOUND, check.gone)));
            };
            for ((field, expected, col), now) in check.expect.iter().zip(values.iter()) {
                if !same_value(now, expected) {
                    abort(self);
                    let error = CommitError::new(
                        CONFLICT,
                        format!("'{field}' was changed by someone else since it was loaded"),
                    );
                    return Err(match (check.row, col) {
                        (Some(row), Some(col)) => UnitError::at(error, row, *col),
                        _ => UnitError::from(error),
                    });
                }
            }
        }
        for statement in &deletes {
            match self.db.exec(handle, statement) {
                Ok(0) => {
                    abort(self);
                    return Err(UnitError::from(CommitError::new(NOT_FOUND, "a deleted row is no longer in the table")));
                }
                Ok(_) => {}
                Err(message) => {
                    abort(self);
                    return Err(UnitError::from(CommitError::new(IO_ERROR, message)));
                }
            }
        }
        for (row, col, statement) in &updates {
            match self.db.exec(handle, statement) {
                Ok(0) => {
                    abort(self);
                    return Err(UnitError::at(CommitError::new(NOT_FOUND, "no row of the table has that key"), *row, *col));
                }
                Ok(_) => {}
                Err(message) => {
                    abort(self);
                    return Err(UnitError::at(CommitError::new(IO_ERROR, message), *row, *col));
                }
            }
        }
        for (row, statement) in &inserts {
            if let Err(message) = self.db.exec(handle, statement) {
                abort(self);
                return Err(UnitError::at(CommitError::new(IO_ERROR, message), *row, 0));
            }
        }
        if let Err(message) = self.db.exec(handle, &commit) {
            abort(self);
            return Err(general(CommitError::new(IO_ERROR, message)));
        }
        Ok(written)
    }

    // ── The backend (WASM-005 R13) ───────────────────────────────────────────

    /// A remote grid's unit goes to its `…-SAVE` endpoint as **one call** carrying
    /// every change — the deletes, the updates and the inserts, in that order —
    /// and the backend writes them as one transaction or none: its own check of
    /// every change first, then the writes, then `COMMIT` or `ROLLBACK`. An
    /// update carries the row's key as loaded and every cell as the grid shows
    /// it; an insert its cells; a delete its key. `FAILED-CHANGE` names the
    /// change the backend refused, and the grid marks that change's cell as it
    /// marks its own refusals. A backend that was not reached, or did not
    /// answer, leaves the edits waiting — as a connection in Async mode does.
    ///
    /// The cells stay as typed: the backend answers no values, and the next
    /// refresh shows the source's own spelling of them.
    fn apply_remote(&mut self, grid: &str, source: &Source, plan: &Plan) -> Result<Written, UnitError> {
        let endpoint = self.obj_get(grid, "_BindingRemoteSave").trim().to_owned();
        if endpoint.is_empty() {
            // Permanent: the form's or the control's name is not a COBOL word, so the
            // Build wrote no program (the seeding left the endpoint empty).
            return Err(UnitError::from(CommitError::new(NOT_WRITABLE, "the binding has no SAVE endpoint on the backend")));
        }
        let rows = self.datagrid_rows(grid);
        let keys = |origin: &str| -> Vec<Value> {
            source.key_values(origin).into_iter().map(|key| json!({ "KEY-TEXT": key })).collect()
        };
        // The generated SAVE wants exactly one cell per bound field; a row the
        // program added has as many cells as the designed columns, which may differ.
        let width = source.fields.len();
        let cells = |cells: &[String]| -> Vec<Value> {
            (0..width).map(|at| json!({ "CELL-TEXT": cells.get(at).map_or("", String::as_str) })).collect()
        };
        let mut changes: Vec<Value> = Vec::new();
        // The cell to blame per change, in the order sent; a deleted row has none.
        let mut blame: Vec<Option<(usize, usize)>> = Vec::new();
        for row in &plan.deletes {
            changes.push(json!({ "CHANGE-ACTION": "DELETE", "KEYS": keys(&row.origin), "CELLS": [] }));
            blame.push(None);
        }
        for update in &plan.updates {
            let shown = rows.get(update.row).cloned().unwrap_or_default();
            changes.push(json!({ "CHANGE-ACTION": "UPDATE", "KEYS": keys(&update.origin), "CELLS": cells(&shown) }));
            blame.push(Some((update.row, update.cells.first().map_or(0, |cell| cell.col))));
        }
        for insert in &plan.inserts {
            changes.push(json!({ "CHANGE-ACTION": "INSERT", "KEYS": [], "CELLS": cells(&insert.cells) }));
            blame.push(Some((insert.row, 0)));
        }
        if changes.len() > MAX_CHANGES {
            return Err(UnitError::refused(
                REJECTED,
                format!("the save carries {} changes; the backend takes at most {MAX_CHANGES} in one unit", changes.len()),
            ));
        }
        let first = blame.iter().flatten().next().copied();
        let at_change = |n: usize| n.checked_sub(1).and_then(|at| blame.get(at)).copied().flatten();
        let answer = match self.call_generated(&endpoint, json!({ "CHANGES": changes })) {
            Ok(answer) => answer,
            // The backend was not reached, or is busy: the same save may be made
            // again as it is, so the edits stay waiting (as with an Async connection).
            Err(e) if save_may_retry(&e) => {
                return Err(UnitError::refused(IO_ERROR, format!("the backend did not take the save: {e}")));
            }
            // Over: a value the wire refused (a cell wider than it carries), or a call
            // that can never succeed as it is — a program the Build did not generate, a
            // role refused, a version. A failure at the change the error names, else at
            // the first cell this unit writes.
            Err(e) => {
                let code = if matches!(e.class, ErrorClass::Validation | ErrorClass::Limit) { REJECTED } else { IO_ERROR };
                let error = CommitError::new(code, e.message);
                let at = e.detail.as_deref().and_then(change_in_path).and_then(at_change).or(first);
                return Err(match at {
                    Some((row, col)) => UnitError::at(error, row, col),
                    None => UnitError::from(error),
                });
            }
        };
        let status = text_of(&answer, "COMMIT-STATUS");
        if status == OK {
            return Ok(Vec::new());
        }
        let code = if status.is_empty() { IO_ERROR.to_owned() } else { status };
        let message = text_of(&answer, "COMMIT-MESSAGE");
        // The backend checks every cell of a change and names the field it refused
        // (`CUST-BALANCE: value too large for PIC`): that cell carries the mark, as on
        // the desktop; the change's first edited cell when no field is named.
        let named_col = message
            .split_once(':')
            .and_then(|(field, _)| source.fields.iter().position(|f| f.eq_ignore_ascii_case(field.trim())));
        let failed = answer.get("FAILED-CHANGE").and_then(Value::as_u64).unwrap_or(0) as usize;
        let error = CommitError::new(&code, message);
        Err(match at_change(failed) {
            Some((row, col)) => UnitError::at(error, row, named_col.unwrap_or(col)),
            None => UnitError::from(error),
        })
    }
}

/// May a save the backend did not take be made again exactly as it is? Yes when
/// the backend was not reached or is busy (a transport failure; a rate or
/// concurrency limit; a call that ran out of time): the edits stay pending and
/// the next `Save()` or settle sends them again. No for everything else — a value
/// the wire refused, a program the Build did not generate, a role refused, a
/// version — which is over and is reported as a failed unit.
fn save_may_retry(e: &CallError) -> bool {
    match e.class {
        ErrorClass::Transport => true,
        ErrorClass::Limit => matches!(e.code, codes::RATE_LIMITED | codes::TOO_MANY_CALLS | codes::CALL_TIMEOUT),
        _ => false,
    }
}

/// The change (1-based) a mapping error's path names: `…CHANGES(3)…` or
/// `…CHANGES[3]…`; `None` when the path names no change.
fn change_in_path(path: &str) -> Option<usize> {
    let rest = &path[path.find("CHANGES")? + "CHANGES".len()..];
    let digits: String = rest.trim_start_matches(['(', '[']).chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok().filter(|n| *n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_indexed::{
        FieldUsage, IndexedDefinition, IndexedField, KeyDef, KeyEncodingDef, KeyOrderingDef, KeyPartDef,
        RecordFormatDef,
    };
    use cobolt_lexer::{tokenize, SourceFormat};
    use cobolt_parser::parse;

    fn leaf(name: &str, pic: &str, offset: u32, length: u32) -> IndexedField {
        IndexedField {
            level: 5,
            name: name.to_owned(),
            pic: pic.to_owned(),
            usage: FieldUsage::Display,
            offset: Some(offset),
            length: Some(length),
            comment: String::new(),
            grid_control: None,
            occurs: None,
            redefines: None,
            synchronized: false,
            children: Vec::new(),
        }
    }

    /// An interpreter holding one DataGrid seeded with `seed`.
    fn with_grid(seed: Vec<(&str, &str)>) -> Interpreter {
        let parsed = parse(tokenize(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. GRID-SOURCE.\nPROCEDURE DIVISION.\nMAIN.\n    STOP RUN.\n",
            SourceFormat::Free,
        ));
        let mut interp = Interpreter::new(parsed.program.expect("parses"));
        interp.seed_objects([(
            "GRID-1".to_owned(),
            "DataGrid".to_owned(),
            seed.into_iter().map(|(k, v)| (k.to_owned(), v.to_owned())).collect::<Vec<_>>(),
        )]);
        interp
    }

    /// WASM-005 R13: a remote indexed grid identifies a row by the `.cidx` primary
    /// key — every part of it, as the desktop and the generated SAVE do — whatever
    /// the binding recorded as its key field; without the definition the binding's
    /// key fields stand in, and a SQL source's are always the binding's.
    #[test]
    fn a_remote_indexed_grid_takes_its_keys_from_the_definitions_primary_key() {
        let dir = tempfile::tempdir().unwrap();
        let cidx = dir.path().join("orders.cidx");
        let mut def = IndexedDefinition::new("ORDER-FILE", dir.path().join("orders.idx").to_string_lossy().into_owned());
        def.record_format = RecordFormatDef::Fixed { length: 24 };
        def.keys.primary = KeyDef {
            name: Some("ORDER-KEY".into()),
            parts: vec![
                KeyPartDef { field_name: "ORD-YEAR".into(), offset: 0, length: 4, encoding: KeyEncodingDef::Bytes },
                KeyPartDef { field_name: "ORD-SEQ".into(), offset: 4, length: 6, encoding: KeyEncodingDef::Bytes },
            ],
            duplicates_allowed: false,
            ordering: KeyOrderingDef::Ascending,
        };
        let mut root = leaf("ORDER-REC", "", 0, 24);
        root.level = 1;
        root.children = vec![leaf("ORD-YEAR", "9(4)", 0, 4), leaf("ORD-SEQ", "9(6)", 4, 6), leaf("ORD-NOTE", "X(14)", 10, 14)];
        def.fields = vec![root];
        def.finalized = true;
        cobolt_indexed::save_indexed(&cidx, &def).expect("write the .cidx");
        let cidx = cidx.to_string_lossy().into_owned();

        fn seed<'a>(path: &'a str, key_fields: &'a str) -> Vec<(&'a str, &'a str)> {
            vec![
                ("_BindingRemote", "1"),
                ("_BindingKind", "IndexedFile"),
                ("_BindingIndexedPath", path),
                ("_BindingFields", "ORD-YEAR\nORD-SEQ\nORD-NOTE"),
                ("_BindingKeyFields", key_fields),
                ("_BindingWritable", "1"),
            ]
        }
        // The Designer recorded one key field, or none: the definition decides.
        for recorded in ["ORD-YEAR", ""] {
            let interp = with_grid(seed(&cidx, recorded));
            let source = interp.grid_source("GRID-1").expect("a source");
            assert_eq!(source.kind, Kind::Remote);
            assert_eq!(source.keys, vec!["ORD-YEAR".to_owned(), "ORD-SEQ".to_owned()], "recorded {recorded:?}");
            assert_eq!(source.key_cols, vec![0, 1]);
            assert!(source.missing_key().is_none() && source.def.is_none());
        }
        // No readable definition: the binding's key fields stand in.
        let interp = with_grid(seed("nowhere/missing.cidx", "ORD-SEQ"));
        assert_eq!(interp.grid_source("GRID-1").unwrap().keys, vec!["ORD-SEQ".to_owned()]);
        // A SQL source: the binding's key fields, as the generated SAVE expects them.
        let interp = with_grid(vec![
            ("_BindingRemote", "1"),
            ("_BindingKind", "Sql"),
            ("_BindingFields", "ID\nNAME"),
            ("_BindingKeyFields", "ID"),
            ("_BindingWritable", "1"),
        ]);
        let sql = interp.grid_source("GRID-1").unwrap();
        assert_eq!((sql.kind, sql.keys), (Kind::Remote, vec!["ID".to_owned()]));
        // No fields at all is not a source.
        let interp = with_grid(vec![("_BindingRemote", "1"), ("_BindingKind", "Sql"), ("_BindingWritable", "1")]);
        assert_eq!(interp.grid_source("GRID-1").err().map(|e| e.code), Some(NOT_WRITABLE.to_owned()));
    }

    /// A save the backend did not take is made again only when it may succeed as
    /// it is; the refusals that are over fail the unit, so the operator is told once
    /// and nothing is re-sent at every settle.
    #[test]
    fn a_save_is_retried_only_for_a_transport_failure_or_a_busy_backend() {
        let limit = |code: u32| CallError::new(ErrorClass::Limit, code, "limit");
        assert!(save_may_retry(&CallError::transport(codes::NOT_CONFIGURED, "no backend")));
        assert!(save_may_retry(&CallError::transport(codes::TIMEOUT, "timed out")));
        for code in [codes::RATE_LIMITED, codes::TOO_MANY_CALLS, codes::CALL_TIMEOUT] {
            assert!(save_may_retry(&limit(code)), "{code}");
        }
        for code in [codes::FRAME_TOO_LARGE, codes::FILE_TOO_LARGE] {
            assert!(!save_may_retry(&limit(code)), "{code}");
        }
        assert!(!save_may_retry(&CallError::validation("too long", "RC-PARAMS.CHANGES(2).CELLS(1).CELL-TEXT")));
        for (class, code) in [
            (ErrorClass::NotFound, codes::UNKNOWN_PROGRAM),
            (ErrorClass::Authorisation, codes::ROLE_REFUSED),
            (ErrorClass::Authentication, codes::TOKEN_EXPIRED),
            (ErrorClass::Version, codes::CLIENT_TOO_OLD),
            (ErrorClass::Server, codes::INTERNAL),
        ] {
            assert!(!save_may_retry(&CallError::new(class, code, "over")), "{code}");
        }
        // The change a mapping error names, for the mark.
        assert_eq!(change_in_path("RC-PARAMS.CHANGES(2).CELLS(1).CELL-TEXT"), Some(2));
        assert_eq!(change_in_path("CHANGES[7].KEYS[1].KEY-TEXT"), Some(7));
        assert_eq!(change_in_path("RC-PARAMS.CHANGE-COUNT"), None);
        assert_eq!(change_in_path("CHANGES(0)"), None);
    }
}
