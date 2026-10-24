// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Remote bindings (WASM-005 R13, Q8): a DataGrid bound to an indexed file or a
//! SQL query on a **web form** reads its rows and writes its changes through the
//! backend — the generated `…-READ` and `…-SAVE` programs the Build wrote for it
//! (`cobolt-codegen`'s `remote_controls`) — instead of the file or the connection
//! the desktop reaches directly.
//!
//! The host seeds the grid as remote (`_BindingRemote`, with the two endpoints in
//! `_BindingRemoteRead` and `_BindingRemoteSave`; `cobolt-form-host`'s `seeding`)
//! when the form is a Web or Mobile project's. Everything else is the grid's own:
//! `RefreshBinding()` and `Save()` are the same methods, the ledger (`grid_edit`)
//! and the plan of a save (`grid_unit`) are the same code, and a COBOL table is
//! never remote (Q8). A `RestApi` or `AgentAi` binding will have only this path
//! once the Build writes its programs and the host seeds it; until then it is
//! not seeded remote and, on the desktop as before, fills nothing.
//!
//! The rows travel **window by window**: the backend answers at most
//! [`MAX_WINDOW_ROWS`] per call, so a refresh asks `ROW-OFFSET`/`ROW-COUNT` until
//! the source is exhausted and shows every row, as the desktop does. Paging is
//! offset and count only (plan O3).

use cobolt_wire::call::{ArgMode, ArgPayload, CallRequest};
use cobolt_wire::controls::MAX_WINDOW_ROWS;
use cobolt_wire::error::CallError;
use serde_json::{json, Value};

use super::Interpreter;

/// What one `…-READ` call answered.
struct Window {
    status: String,
    message: String,
    /// How many rows the source holds in all.
    total: u64,
    rows: Vec<Vec<String>>,
}

/// The text member `key` of a generated program's answer, trimmed; empty when
/// the answer has no such member.
pub(super) fn text_of(answer: &Value, key: &str) -> String {
    answer.get(key).and_then(Value::as_str).unwrap_or_default().trim().to_owned()
}

/// A cell of a row the backend answered: the `X(128)` field's trailing padding
/// goes, its leading spaces stay — as the desktop keeps them (`format_field_display`
/// trims the end only, and the SQL path trims nothing).
fn cell_text(cell: &Value) -> String {
    cell.get("CELL-TEXT").and_then(Value::as_str).unwrap_or_default().trim_end().to_owned()
}

impl Interpreter {
    /// Is this grid's binding one the backend serves (seeded by the host for a
    /// web form)?
    pub(super) fn binding_is_remote(&self, grid: &str) -> bool {
        self.obj_get(grid, "_BindingRemote").trim() == "1"
    }

    /// One call to a generated endpoint: its one `BY VALUE` group in, its
    /// `RETURNING` group out, both as the JSON the mapping writes. The call goes
    /// through the run's transport like a `CALL … WITH WEBSOCKET LINKAGE`
    /// (`send_request`): the same idempotency-key rule, the same last-error
    /// record for `COBOL::"WS-LAST-ERROR"`.
    pub(super) fn call_generated(&mut self, program: &str, params: Value) -> Result<Value, CallError> {
        let mut request = CallRequest {
            program: program.trim().to_ascii_uppercase(),
            args: vec![ArgPayload { mode: ArgMode::Value, json: params.to_string() }],
            wants_returning: true,
            idempotency_key: None,
        };
        let outcome = self.send_request(&mut request).and_then(|reply| {
            let text = reply.returning.ok_or_else(|| {
                CallError::validation(format!("{} answered without its RETURNING group", request.program), "RC-RESULT")
            })?;
            serde_json::from_str::<Value>(&text).map_err(|e| {
                CallError::validation(format!("{} answered text that is not JSON: {e}", request.program), "RC-RESULT")
            })
        });
        match &outcome {
            Ok(_) => self.remote.last_error = None,
            Err(e) => {
                tracing::warn!(target: "remote", "{} failed: {e}", request.program);
                self.remote.last_error = Some(e.clone());
            }
        }
        outcome
    }

    /// One window of a remote binding's rows.
    fn read_window(&mut self, endpoint: &str, offset: u64, count: usize) -> Result<Window, CallError> {
        let answer = self.call_generated(endpoint, json!({ "ROW-OFFSET": offset, "ROW-COUNT": count, "PARAMS": [] }))?;
        let rows = answer
            .get("ROWS")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        row.get("CELLS")
                            .and_then(Value::as_array)
                            .map(|cells| cells.iter().map(cell_text).collect())
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Window {
            status: text_of(&answer, "READ-STATUS"),
            message: text_of(&answer, "READ-MESSAGE"),
            total: answer.get("ROW-TOTAL").and_then(Value::as_u64).unwrap_or(0),
            rows,
        })
    }

    /// Fill a remote grid from its `…-READ` endpoint, window by window, and
    /// answer how many rows it shows.
    ///
    /// A source that cannot be read — no file yet (`35`), a refused connection
    /// — is an empty grid, reported on the debugger's File I/O channel under the
    /// endpoint's name as the desktop reports its own read; a SQL binding's
    /// source control gets the message in `LastError` as well, where the
    /// desktop's SQL path puts it. A call the transport could not make is the
    /// same empty grid, with the failure in `WS-LAST-ERROR`.
    pub(super) fn refresh_remote_binding(&mut self, grid: &str) -> usize {
        let endpoint = self.obj_get(grid, "_BindingRemoteRead").trim().to_owned();
        if endpoint.is_empty() {
            // The form's or the control's name is not a COBOL word, so the Build
            // wrote no program and the seeding left the endpoint empty: the File
            // I/O channel says why the grid is empty, under the grid's own name.
            tracing::warn!("remote binding on {grid}: no READ endpoint was seeded");
            self.debug_binding_io(grid, "30", "the binding has no READ endpoint on the backend (the form or control name is not a COBOL word)");
            self.obj_set(grid, "Rows", String::new());
            return 0;
        }
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut windows = 0usize;
        let outcome = loop {
            match self.read_window(&endpoint, rows.len() as u64, MAX_WINDOW_ROWS) {
                Err(e) => break Err(e),
                Ok(window) => {
                    windows += 1;
                    if window.status != "00" {
                        break Ok(Some((window.status, window.message)));
                    }
                    let got = window.rows.len();
                    rows.extend(window.rows);
                    // The source is exhausted when a window comes back short or
                    // the total is reached; an empty window ends it whatever it says.
                    if got == 0 || got < MAX_WINDOW_ROWS || rows.len() as u64 >= window.total {
                        break Ok(None);
                    }
                }
            }
        };
        let sql_control = self.obj_get(grid, "_BindingSqlControl").trim().to_owned();
        match outcome {
            Ok(None) => {
                let count = rows.len();
                self.set_datagrid_rows(grid, &rows);
                self.debug_binding_io(&endpoint, "00", &format!("{count} rows in {windows} windows"));
                count
            }
            Ok(Some((status, message))) => {
                self.obj_set(grid, "Rows", String::new());
                self.debug_binding_io(&endpoint, &status, &message);
                if !sql_control.is_empty() {
                    self.obj_set(&sql_control, "LastError", message);
                }
                0
            }
            Err(e) => {
                self.obj_set(grid, "Rows", String::new());
                self.debug_binding_io(&endpoint, "30", &e.to_string());
                if !sql_control.is_empty() {
                    self.obj_set(&sql_control, "LastError", e.to_string());
                }
                0
            }
        }
    }
}
