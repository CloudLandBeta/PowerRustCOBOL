// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Remote-bound DataGrids** (WASM-005 R13, T16): a DataGrid bound to an indexed file or a SQL
//! query on a web form shows what the backend holds. The **player**, not the program, makes the
//! calls — on its own connection, under ids of its own ([`ID_BASE`] and above: the range ABI A8
//! reserves for the player's calls, beside the Authentication control's) — as the plan has it:
//! bindings are driven by the host.
//!
//! * `RefreshBinding()` asks the generated `…-READ` program for the rows, [`MAX_WINDOW_ROWS`] a
//!   call until the source is exhausted — the windows the desktop's interpreter asks for — and
//!   answers at once with the count the grid shows *now*: the rows arrive when the backend answers.
//! * `Save()` sends the difference between what was loaded and what the grid shows to the generated
//!   `…-SAVE` program as one unit (the rows gone, the rows whose cells changed, the rows added, in
//!   that order), answers `98` while the unit is on its way, and the outcome comes with the
//!   backend's answer: `CommitStatus`, `SavedRows`, `onSaved` — or `CommitStatus`, `LastError`, the
//!   grid put back as loaded, the refused cell marked, `onCommitError`. A backend that could not be
//!   reached, or is busy, leaves the changes in the grid for the next `Save()`.
//! * `DiscardEdit()` puts the grid back as loaded.
//!
//! The endpoints are named as the Build names them (`cobolt_wire::controls::endpoint_name`), from
//! the form's own data bindings, so the description carries nothing the player could not derive.
//! A COBOL table stays in the form (Q8); `RestApi` and `AgentAi` wait for their programs.
//!
//! What differs from the desktop, recorded in `specs/WASM-PARITY.md`: the methods answer before the
//! backend does (a module's call is synchronous, the network is not); a row's identity is the
//! binding's key field(s), where the desktop reads the `.cidx` primary key; a key cell the program
//! changes makes a delete and an insert, where the desktop refuses the edit (`21`); and the browser
//! grid has no operator cell editing yet (WASM-004 class A), so the changes a save carries are the
//! program's own (`AddRow`, `DeleteRow`, `SetCellValue`).
//!
//! This module is the pure part: what is sent, what an answer means, which changes a save carries.
//! `Host` (`host.rs`) owns the state and the calls.

use std::collections::{BTreeMap, BTreeSet};

use cobolt_forms::model::{BindingMode, BindingSourceDescriptor, BindingTargetDescriptor, Form};
use cobolt_wire::controls::{endpoint_name, MAX_WINDOW_ROWS};
use cobolt_wire::error::codes;
use serde_json::{json, Value};

/// Ids at or above this are a remote grid's calls; the Authentication control's lie between its own
/// base and this one, the program module's below both.
pub const ID_BASE: u32 = 0xC000_0000;

pub fn is_grid_id(id: u32) -> bool {
    id >= ID_BASE
}

/// Between the key cells of a row's identity.
const KEY_SEPARATOR: char = '\u{1f}';

/// What the player knows of a grid the backend serves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteGrid {
    /// The generated `…-READ` program.
    pub read: String,
    /// The generated `…-SAVE` program.
    pub save: String,
    /// The bound fields, one per grid column, in column order.
    pub fields: Vec<String>,
    /// The fields that identify a row (the binding's key fields).
    pub keys: Vec<String>,
    /// The grid column of each key that is shown.
    pub key_cols: Vec<usize>,
    /// The binding is writable and its source can be written.
    pub writable: bool,
    /// The rows as last loaded or saved, in the source's order.
    pub loaded: Vec<Vec<String>>,
}

impl RemoteGrid {
    /// What a row is: its key cells, trimmed. Empty when the grid has no key, a key is not shown,
    /// or a key cell is blank (a row the program added without its key).
    pub fn key_of(&self, cells: &[String]) -> String {
        if self.keys.is_empty() || self.key_cols.len() != self.keys.len() {
            return String::new();
        }
        let mut key = String::new();
        for col in &self.key_cols {
            let cell = cells.get(*col).map(|c| c.trim()).unwrap_or("");
            if cell.is_empty() {
                return String::new();
            }
            if !key.is_empty() {
                key.push(KEY_SEPARATOR);
            }
            key.push_str(cell);
        }
        key
    }

    /// A row's cells, exactly one per bound field (padded with empty cells, or truncated): what the
    /// generated SAVE expects. A row the program added has as many cells as the designed columns,
    /// which may differ.
    pub fn padded(&self, cells: &[String]) -> Vec<String> {
        (0..self.fields.len()).map(|at| cells.get(at).cloned().unwrap_or_default()).collect()
    }
}

/// The remote grids of `form`: `(control id, grid)` for every DataGrid bound to an indexed file or
/// a SQL query. A form or control whose name is not a COBOL word has no endpoint (the Build refuses
/// such a form) and is left out.
pub fn remote_grids(form: &Form) -> Vec<(String, RemoteGrid)> {
    let mut out = Vec::new();
    for binding in &form.data_bindings {
        let BindingTargetDescriptor::DataGrid { control_id } = &binding.target else { continue };
        let (fields, keys, source_writable): (Vec<String>, Vec<String>, bool) = match &binding.source {
            BindingSourceDescriptor::IndexedFile { fields, key_field, writable, .. } => {
                (fields.iter().map(|f| f.name.clone()).collect(), key_field.iter().cloned().collect(), *writable)
            }
            BindingSourceDescriptor::Sql { fields, key_fields, writable, .. } => {
                (fields.iter().map(|f| f.name.clone()).collect(), key_fields.clone(), *writable)
            }
            _ => continue,
        };
        let (Ok(read), Ok(save)) = (endpoint_name(&form.name, control_id, "READ"), endpoint_name(&form.name, control_id, "SAVE")) else {
            continue;
        };
        let key_cols: Vec<usize> =
            keys.iter().filter_map(|k| fields.iter().position(|f| f.eq_ignore_ascii_case(k))).collect();
        let writable = source_writable && binding.mode == BindingMode::Writable;
        out.push((control_id.clone(), RemoteGrid { read, save, fields, keys, key_cols, writable, loaded: Vec::new() }));
    }
    out
}

// ── frames ──────────────────────────────────────────────────────────────────

/// The request frame of one window — built as the module builds its own `remote` frames
/// (`cobolt-wasm-rt`): the program, its one `BY VALUE` group as JSON, and a `RETURNING` wanted.
pub fn read_frame(read: &str, offset: u64, count: usize) -> Vec<u8> {
    json!({
        "program": read,
        "args": [{ "mode": "value", "json": { "ROW-OFFSET": offset, "ROW-COUNT": count, "PARAMS": [] } }],
        "returning": true
    })
    .to_string()
    .into_bytes()
}

/// The request frame of a save: every change, in the order of [`plan_changes`].
pub fn save_frame(save: &str, changes: &[Value]) -> Vec<u8> {
    json!({
        "program": save,
        "args": [{ "mode": "value", "json": { "CHANGES": changes } }],
        "returning": true
    })
    .to_string()
    .into_bytes()
}

/// What the connection answered: the program's result, or a failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Result(Value),
    Error { class: String, code: u32, message: String },
}

/// Read an answer frame — `{"id":N,"result":{"args":[…],"returning":…}}` or
/// `{"id":N,"error":{"class":…,"code":…,"message":…}}`, as the module reads its own.
pub fn parse_answer(frame: &[u8]) -> Answer {
    let v: Value = match serde_json::from_slice(frame) {
        Ok(v) => v,
        Err(e) => return Answer::Error { class: "client".into(), code: 2, message: format!("the answer is not a JSON frame: {e}") },
    };
    if let Some(e) = v.get("error") {
        return Answer::Error {
            class: text(e, "class"),
            code: e.get("code").and_then(Value::as_u64).unwrap_or(0) as u32,
            message: text(e, "message"),
        };
    }
    match v.get("result") {
        Some(r) => Answer::Result(r.clone()),
        None => Answer::Error { class: "client".into(), code: 2, message: "the answer carries neither a result nor an error".into() },
    }
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().trim().to_owned()
}

/// A cell the backend answered: the `X(128)` field's trailing padding goes, its leading spaces stay.
fn cell(v: &Value) -> String {
    v.get("CELL-TEXT").and_then(Value::as_str).unwrap_or_default().trim_end().to_owned()
}

/// One window of a `…-READ` answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub status: String,
    pub message: String,
    /// How many rows the source holds in all.
    pub total: u64,
    pub rows: Vec<Vec<String>>,
}

pub fn parse_window(result: &Value) -> Window {
    let r = result.get("returning").unwrap_or(&Value::Null);
    Window {
        status: text(r, "READ-STATUS"),
        message: text(r, "READ-MESSAGE"),
        total: r.get("ROW-TOTAL").and_then(Value::as_u64).unwrap_or(0),
        rows: r
            .get("ROWS")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .map(|row| row.get("CELLS").and_then(Value::as_array).map(|cells| cells.iter().map(cell).collect()).unwrap_or_default())
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// Is the source exhausted after a window of `got` rows, `so_far` rows gathered, out of `total`?
/// A short window ends it, as does reaching the total; an empty window ends it whatever it says.
pub fn last_window(got: usize, so_far: usize, total: u64) -> bool {
    got == 0 || got < MAX_WINDOW_ROWS || so_far as u64 >= total
}

/// The answer of a `…-SAVE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveOutcome {
    pub status: String,
    pub message: String,
    /// The change the backend refused, from 1; 0 when none is named.
    pub failed_change: usize,
}

pub fn parse_save(result: &Value) -> SaveOutcome {
    let r = result.get("returning").unwrap_or(&Value::Null);
    SaveOutcome {
        status: text(r, "COMMIT-STATUS"),
        message: text(r, "COMMIT-MESSAGE"),
        failed_change: r.get("FAILED-CHANGE").and_then(Value::as_u64).unwrap_or(0) as usize,
    }
}

/// May a save the backend did not take be sent again as it is — the changes stay in the grid for
/// the next `Save()`? Yes for a transport failure and a busy backend (a rate or concurrency limit,
/// a call that ran out of time); no for everything else, which is over. The same rule the
/// interpreter applies (`grid_unit::save_may_retry`); the class words are the connection's.
pub fn may_retry(class: &str, code: u32) -> bool {
    match class {
        "transport" => true,
        "limit" => matches!(code, codes::RATE_LIMITED | codes::TOO_MANY_CALLS | codes::CALL_TIMEOUT),
        _ => false,
    }
}

// ── the changes a save carries ───────────────────────────────────────────────

/// What a save carries: the changes in the generated SAVE's order, and the grid row each concerns
/// (`None` for a deleted row, which is no longer in the grid).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Changes {
    pub frames: Vec<Value>,
    pub rows: Vec<Option<usize>>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }
}

fn blank(cells: &[String]) -> bool {
    cells.iter().all(|c| c.trim().is_empty())
}

/// The difference between what was loaded and what the grid shows: a loaded row whose key is gone
/// is a `DELETE` (its key as loaded); a row whose key was loaded and whose cells differ is an
/// `UPDATE` (the key as loaded, every cell as shown); a row with a key the load did not have — or
/// with no key — is an `INSERT` (every cell as shown). A blank row is never inserted. Two rows with
/// one key cannot be told apart: that is refused with the desktop's words.
pub fn plan_changes(grid: &RemoteGrid, shown: &[Vec<String>]) -> Result<Changes, String> {
    let mut out = Changes::default();
    if shown.iter().map(|r| grid.padded(r)).eq(grid.loaded.iter().map(|r| grid.padded(r))) {
        return Ok(out);
    }
    if grid.keys.is_empty() || grid.key_cols.len() != grid.keys.len() {
        let missing = grid.keys.iter().find(|k| !grid.fields.iter().any(|f| f.eq_ignore_ascii_case(k))).cloned();
        return Err(match missing {
            Some(key) => format!("key field '{key}' is not shown in the grid"),
            None => "the binding has no key field: a row cannot be told from its neighbours".to_owned(),
        });
    }
    const AMBIGUOUS: &str = "two rows of the grid have the same key; rows cannot be told apart";
    let mut loaded_by_key: BTreeMap<String, &Vec<String>> = BTreeMap::new();
    for row in &grid.loaded {
        let key = grid.key_of(row);
        if !key.is_empty() && loaded_by_key.insert(key, row).is_some() {
            return Err(AMBIGUOUS.to_owned());
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut updates: Vec<(usize, &Vec<String>, &Vec<String>)> = Vec::new();
    let mut inserts: Vec<(usize, &Vec<String>)> = Vec::new();
    for (at, row) in shown.iter().enumerate() {
        if blank(row) {
            continue;
        }
        let key = grid.key_of(row);
        match loaded_by_key.get(&key).filter(|_| !key.is_empty()) {
            Some(was) => {
                if !seen.insert(key.clone()) {
                    return Err(AMBIGUOUS.to_owned());
                }
                if grid.padded(was) != grid.padded(row) {
                    updates.push((at, was, row));
                }
            }
            None => inserts.push((at, row)),
        }
    }
    let deletes: Vec<&Vec<String>> = grid
        .loaded
        .iter()
        .filter(|row| {
            let key = grid.key_of(row);
            !key.is_empty() && !seen.contains(&key)
        })
        .collect();
    let keys_of = |row: &[String]| -> Vec<Value> {
        grid.key_cols.iter().map(|c| json!({ "KEY-TEXT": row.get(*c).map(|s| s.trim()).unwrap_or("") })).collect()
    };
    let cells_of = |row: &[String]| -> Vec<Value> { grid.padded(row).into_iter().map(|c| json!({ "CELL-TEXT": c })).collect() };
    for row in deletes {
        out.frames.push(json!({ "CHANGE-ACTION": "DELETE", "KEYS": keys_of(row), "CELLS": [] }));
        out.rows.push(None);
    }
    for (at, was, now) in updates {
        out.frames.push(json!({ "CHANGE-ACTION": "UPDATE", "KEYS": keys_of(was), "CELLS": cells_of(now) }));
        out.rows.push(Some(at));
    }
    for (at, now) in inserts {
        out.frames.push(json!({ "CHANGE-ACTION": "INSERT", "KEYS": [], "CELLS": cells_of(now) }));
        out.rows.push(Some(at));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::model::{BindingDataType, BindingField, Control, ControlType, DataBindingDef};
    use cobolt_wire::controls::find;
    use cobolt_wire::mapping::{decode, DEFAULT_MAX_BYTES};
    use cobolt_wire::node::Node;

    fn grid() -> RemoteGrid {
        RemoteGrid {
            read: "RC-6-ORDERS-5-GRD-R-READ".into(),
            save: "RC-6-ORDERS-5-GRD-R-SAVE".into(),
            fields: vec!["CUST-ID".into(), "CUST-NAME".into(), "CUST-ACTIVE".into()],
            keys: vec!["CUST-ID".into()],
            key_cols: vec![0],
            writable: true,
            loaded: vec![row(&["1", "Ana", "1"]), row(&["2", "Bruno", "0"]), row(&["3", "Carla", "1"])],
        }
    }

    fn row(cells: &[&str]) -> Vec<String> {
        cells.iter().map(|c| (*c).to_owned()).collect()
    }

    /// The group an argument's JSON would fill, as the backend fills it.
    fn fits(control: &str, op: &str, frame: &[u8]) -> Value {
        let v: Value = serde_json::from_slice(frame).unwrap();
        assert_eq!(v["returning"], true);
        assert_eq!(v["args"][0]["mode"], "value");
        let layout = find(control, op).unwrap().param_layout();
        let json = v["args"][0]["json"].to_string();
        decode(&layout, &Node::initial(&layout), &json, DEFAULT_MAX_BYTES).unwrap_or_else(|e| panic!("{op}: {e}"));
        v
    }

    /// ABI A8: the grids' ids never meet the module's (from 1) nor the Authentication control's.
    #[test]
    fn grid_ids_are_their_own_range() {
        assert!(is_grid_id(ID_BASE) && is_grid_id(u32::MAX));
        assert!(!is_grid_id(1) && !is_grid_id(0x7fff_ffff) && !is_grid_id(crate::auth::wire::ID_BASE));
        assert!(!crate::auth::wire::is_auth_id(ID_BASE), "an id of the grids' range is not the Authentication control's");
        assert!(crate::auth::wire::is_auth_id(ID_BASE - 1));
    }

    /// The frames fit the catalogue's layouts — what the generated programs read — and are built as
    /// the module builds its own.
    #[test]
    fn read_and_save_frames_fit_the_catalogue() {
        let g = grid();
        let v = fits("DataGrid", "READ", &read_frame(&g.read, 200, MAX_WINDOW_ROWS));
        assert_eq!(v["program"], g.read);
        assert_eq!(v["args"][0]["json"]["ROW-OFFSET"], 200);
        assert_eq!(v["args"][0]["json"]["ROW-COUNT"], MAX_WINDOW_ROWS);
        let changes = plan_changes(&g, &[row(&["1", "Ana", "1"]), row(&["3", "Carla S.", "1"]), row(&["4", "Dora", "0"])]).unwrap();
        let v = fits("DataGrid", "SAVE", &save_frame(&g.save, &changes.frames));
        assert_eq!(v["program"], g.save);
        assert_eq!(v["args"][0]["json"]["CHANGES"].as_array().unwrap().len(), 3);
    }

    /// The answers, as the connection hands them: a window, a save's outcome, a failure.
    #[test]
    fn answers_are_read_as_the_module_reads_its_own() {
        let frame = br#"{"id":3221225472,"result":{"args":[null],"returning":{"READ-STATUS":"00","READ-MESSAGE":"","ROW-TOTAL":2,"ROWS-RETURNED":2,"ROWS":[{"CELL-COUNT":3,"CELLS":[{"CELL-TEXT":"1"},{"CELL-TEXT":" Ana  "},{"CELL-TEXT":"1"}]},{"CELL-COUNT":3,"CELLS":[{"CELL-TEXT":"2"},{"CELL-TEXT":"Bruno"},{"CELL-TEXT":"0"}]}]}}}"#;
        let Answer::Result(r) = parse_answer(frame) else { panic!("a result") };
        let w = parse_window(&r);
        assert_eq!((w.status.as_str(), w.total), ("00", 2));
        assert_eq!(w.rows, vec![row(&["1", " Ana", "1"]), row(&["2", "Bruno", "0"])], "trailing padding goes, leading spaces stay");
        assert!(last_window(w.rows.len(), 2, w.total));
        assert!(!last_window(MAX_WINDOW_ROWS, MAX_WINDOW_ROWS, 250) && last_window(50, 250, 250) && last_window(0, 0, 0));

        let frame = br#"{"id":3221225473,"result":{"args":[null],"returning":{"COMMIT-STATUS":"92","COMMIT-MESSAGE":"CUST-NAME: value length exceeds PIC","FAILED-CHANGE":2}}}"#;
        let Answer::Result(r) = parse_answer(frame) else { panic!("a result") };
        assert_eq!(parse_save(&r), SaveOutcome { status: "92".into(), message: "CUST-NAME: value length exceeds PIC".into(), failed_change: 2 });

        let frame = crate::net::error_frame(ID_BASE, crate::net::class::TRANSPORT, 7, "the application is not connected to its backend", false);
        assert_eq!(parse_answer(&frame), Answer::Error { class: "transport".into(), code: 7, message: "the application is not connected to its backend".into() });
        assert!(matches!(parse_answer(b"not json"), Answer::Error { class, .. } if class == "client"));
        assert!(may_retry("transport", 7) && may_retry("limit", codes::RATE_LIMITED) && may_retry("limit", codes::CALL_TIMEOUT));
        assert!(!may_retry("limit", codes::FRAME_TOO_LARGE) && !may_retry("validation", 4000) && !may_retry("notfound", 5000));
    }

    /// The changes a save carries, in the generated SAVE's order, and the row each concerns.
    #[test]
    fn a_save_carries_the_difference_between_the_load_and_the_grid() {
        let g = grid();
        assert!(plan_changes(&g, &g.loaded.clone()).unwrap().is_empty(), "nothing changed, nothing sent");
        // Customer 2 gone, Carla renamed, Dora added, a blank row ignored, a row without its key inserted.
        let shown = vec![row(&["1", "Ana", "1"]), row(&["3", "Carla S.", "1"]), row(&["", "", ""]), row(&["4", "Dora", "0"]), row(&["", "No key", "1"])];
        let c = plan_changes(&g, &shown).unwrap();
        let actions: Vec<&str> = c.frames.iter().map(|f| f["CHANGE-ACTION"].as_str().unwrap()).collect();
        assert_eq!(actions, vec!["DELETE", "UPDATE", "INSERT", "INSERT"]);
        assert_eq!(c.rows, vec![None, Some(1), Some(3), Some(4)]);
        assert_eq!(c.frames[0]["KEYS"][0]["KEY-TEXT"], "2");
        assert_eq!(c.frames[1]["KEYS"][0]["KEY-TEXT"], "3");
        assert_eq!(c.frames[1]["CELLS"][1]["CELL-TEXT"], "Carla S.");
        assert_eq!(c.frames[2]["CELLS"].as_array().unwrap().len(), 3, "one cell per bound field");
        // A row the program added with fewer cells than the fields is padded.
        let c = plan_changes(&g, &[row(&["1", "Ana", "1"]), row(&["2", "Bruno", "0"]), row(&["3", "Carla", "1"]), row(&["5"])]).unwrap();
        assert_eq!(c.frames[0]["CELLS"], json!([{"CELL-TEXT": "5"}, {"CELL-TEXT": ""}, {"CELL-TEXT": ""}]));
        // Two rows with one key cannot be told apart.
        let e = plan_changes(&g, &[row(&["1", "Ana", "1"]), row(&["1", "Ana again", "1"])]).unwrap_err();
        assert!(e.contains("same key"), "{e}");
        // A key field the grid does not show: nothing can be told apart either.
        let mut hidden = grid();
        hidden.keys = vec!["CUST-ID".into(), "CUST-REGION".into()];
        assert_eq!(plan_changes(&hidden, &[row(&["1", "Ana", "0"])]).unwrap_err(), "key field 'CUST-REGION' is not shown in the grid");
        // A binding with no key at all.
        let mut keyless = grid();
        keyless.keys.clear();
        keyless.key_cols.clear();
        assert!(plan_changes(&keyless, &[row(&["1", "Ana", "0"])]).unwrap_err().contains("no key field"));
    }

    /// The grids of a form: the DataGrids with an indexed-file or a SQL binding, named as the Build
    /// names their programs; a COBOL table's grid is not one (Q8).
    #[test]
    fn the_remote_grids_of_a_form_are_its_indexed_and_sql_bound_data_grids() {
        let mut form = Form::new("ORDERS", "Orders", 800, 600);
        for id in ["GRD-R", "GRD-S", "GRD-T"] {
            form.controls.push(Control::new(id, ControlType::DataGrid, 0, 0));
        }
        let fields = vec![BindingField::new("CUST-ID", BindingDataType::Integer).key(), BindingField::new("CUST-NAME", BindingDataType::Text)];
        let mut ix = DataBindingDef::new(
            "customers",
            "Customers",
            BindingSourceDescriptor::IndexedFile { definition_path: "indexed/customers.cidx".into(), record_name: "CUSTOMER-REC".into(), fields: fields.clone(), key_field: Some("CUST-ID".into()), writable: true },
            BindingTargetDescriptor::DataGrid { control_id: "GRD-R".into() },
        );
        ix.mode = BindingMode::Writable;
        form.data_bindings.push(ix);
        form.data_bindings.push(DataBindingDef::new(
            "items",
            "Items",
            BindingSourceDescriptor::Sql { source_control_id: "DB-1".into(), query_name: "q".into(), result_set_name: "r".into(), fields: fields.clone(), key_fields: vec!["CUST-ID".into()], writable: true, query: "SELECT 1".into(), table_name: "T".into() },
            BindingTargetDescriptor::DataGrid { control_id: "GRD-S".into() },
        ));
        form.data_bindings.push(DataBindingDef::new(
            "notes",
            "Notes",
            BindingSourceDescriptor::CobolTable { table_name: "WS-NOTES".into(), occurs_item: "WS-NOTE".into(), fields, key_fields: vec!["CUST-ID".into()], writable: true },
            BindingTargetDescriptor::DataGrid { control_id: "GRD-T".into() },
        ));
        let grids = remote_grids(&form);
        let ids: Vec<&str> = grids.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, vec!["GRD-R", "GRD-S"], "a COBOL table stays in the form");
        let (_, r) = &grids[0];
        assert_eq!((r.read.as_str(), r.save.as_str()), ("RC-6-ORDERS-5-GRD-R-READ", "RC-6-ORDERS-5-GRD-R-SAVE"));
        assert_eq!((r.keys.clone(), r.key_cols.clone(), r.writable), (vec!["CUST-ID".to_owned()], vec![0], true));
        assert!(!grids[1].1.writable, "a read-only binding mode is not writable");
    }
}
