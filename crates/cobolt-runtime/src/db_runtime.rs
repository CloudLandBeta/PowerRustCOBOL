// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Database Runtime Engine — Phase 8.
//!
//! `DbRegistry` manages a pool of live SQLite connections on behalf of the
//! COBOL interpreter.  Each open connection is assigned an integer *handle*
//! (stored in a COBOL `PIC 9(9)` variable) so COBOL programs can hold and
//! pass references across paragraphs.
//!
//! # Supported built-in CALLs
//!
//! | CALL name            | Arguments (BY REFERENCE)                        |
//! |----------------------|-------------------------------------------------|
//! | `COBOL-OPEN-DB`      | conn-string, handle-var, status-var             |
//! | `COBOL-EXEC-SQL`     | handle, query, row-count-var, status-var        |
//! | `COBOL-FETCH-ROW`    | handle, col-index (1-based), dest-var, status   |
//! | `COBOL-NEXT-ROW`     | handle, more-flag-var (`Y`/`N`)                 |
//! | `COBOL-ROW-COUNT`    | handle, count-var                               |
//! | `COBOL-CLOSE-DB`     | handle                                          |
//!
//! # Connection strings
//!
//! | Prefix       | Meaning                                          |
//! |--------------|--------------------------------------------------|
//! _(no prefix)_ | Treated as a file path; opens / creates SQLite DB |
//! `sqlite:`      | Same — file path after the prefix               |
//! `:memory:`     | In-memory SQLite database                        |
//!
//! Support for other backends (PostgreSQL, MySQL) can be added later by
//! swapping the backend in `DbConn::open()`.

use std::collections::HashMap;

// ── DbConn ────────────────────────────────────────────────────────────────────

/// One live database connection plus its current result-set cursor.
pub struct DbConn {
    /// The rusqlite connection.
    conn: rusqlite::Connection,
    /// All rows fetched from the last `COBOL-EXEC-SQL` call.
    /// Each row is a `Vec<String>` of column values.
    rows: Vec<Vec<String>>,
    /// 0-based index of the *current* row (advanced by `COBOL-NEXT-ROW`).
    cursor: usize,
    /// `true` after the cursor passes the last row.
    exhausted: bool,
}

impl DbConn {
    /// Open a new connection.
    ///
    /// `conn_str` is either a file path, `sqlite:<path>`, or `:memory:`.
    fn open(conn_str: &str) -> Result<Self, String> {
        let path = conn_str
            .trim()
            .strip_prefix("sqlite:")
            .unwrap_or(conn_str.trim());

        let conn = if path == ":memory:" {
            rusqlite::Connection::open_in_memory()
        } else {
            rusqlite::Connection::open(path)
        }
        .map_err(|e| e.to_string())?;

        Ok(Self {
            conn,
            rows: Vec::new(),
            cursor: 0,
            exhausted: false,
        })
    }

    /// Execute a SQL statement and cache the result set.
    ///
    /// For `SELECT` queries the rows are collected into `self.rows`.
    /// For `INSERT / UPDATE / DELETE` the affected-row count is returned
    /// and `self.rows` is left empty.
    fn exec(&mut self, sql: &str) -> Result<usize, String> {
        self.rows.clear();
        self.cursor    = 0;
        self.exhausted = false;

        let sql = sql.trim();

        // Detect whether this is a query that returns rows.
        let is_select = sql.to_ascii_uppercase().starts_with("SELECT")
            || sql.to_ascii_uppercase().starts_with("WITH")
            || sql.to_ascii_uppercase().starts_with("PRAGMA");

        if is_select {
            let mut stmt = self.conn.prepare(sql).map_err(|e| e.to_string())?;
            let col_count = stmt.column_count();

            let rows_iter = stmt.query_map([], |row| {
                let mut cols = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    let v: String = match row.get_ref(i) {
                        Ok(rusqlite::types::ValueRef::Null)    => String::new(),
                        Ok(rusqlite::types::ValueRef::Integer(n)) => n.to_string(),
                        Ok(rusqlite::types::ValueRef::Real(f))    => f.to_string(),
                        Ok(rusqlite::types::ValueRef::Text(t))    =>
                            String::from_utf8_lossy(t).into_owned(),
                        Ok(rusqlite::types::ValueRef::Blob(b))    =>
                            format!("<blob {} bytes>", b.len()),
                        Err(_) => String::new(),
                    };
                    cols.push(v);
                }
                Ok(cols)
            }).map_err(|e| e.to_string())?;

            for row in rows_iter {
                self.rows.push(row.map_err(|e| e.to_string())?);
            }
            Ok(self.rows.len())
        } else {
            let affected = self.conn.execute(sql, []).map_err(|e| e.to_string())?;
            Ok(affected)
        }
    }

    /// Return the value of column `col` (1-based) in the current row.
    ///
    /// Returns an empty string if the column or row is out of range.
    fn fetch_col(&self, col: usize) -> String {
        if self.exhausted { return String::new(); }
        self.rows
            .get(self.cursor)
            .and_then(|r| r.get(col.saturating_sub(1)))
            .cloned()
            .unwrap_or_default()
    }

    /// Advance the cursor to the next row.
    ///
    /// Returns `true` if there is a next row, `false` if exhausted.
    fn next_row(&mut self) -> bool {
        if self.exhausted { return false; }
        if self.cursor + 1 < self.rows.len() {
            self.cursor += 1;
            true
        } else {
            self.exhausted = true;
            false
        }
    }

    /// Total row count from the last query.
    fn row_count(&self) -> usize { self.rows.len() }

    /// `true` when the cursor is past the last row.
    fn is_exhausted(&self) -> bool { self.exhausted }
}

// ── DbRegistry ────────────────────────────────────────────────────────────────

/// Registry of all open database connections for one interpreter instance.
///
/// Handles are positive `u32` integers starting at 1.
#[derive(Default)]
pub struct DbRegistry {
    connections: HashMap<u32, DbConn>,
    next_handle: u32,
}

impl DbRegistry {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            next_handle: 1,
        }
    }

    /// Open a new connection and return its handle.
    ///
    /// Returns `Err(message)` if the connection fails.
    pub fn open(&mut self, conn_str: &str) -> Result<u32, String> {
        let conn = DbConn::open(conn_str)?;
        let handle = self.next_handle;
        self.next_handle += 1;
        self.connections.insert(handle, conn);
        Ok(handle)
    }

    /// Execute SQL on an existing connection.
    ///
    /// Returns `Ok(row_count_or_affected_rows)` or `Err(message)`.
    pub fn exec(&mut self, handle: u32, sql: &str) -> Result<usize, String> {
        self.connections
            .get_mut(&handle)
            .ok_or_else(|| format!("No open connection with handle {handle}"))?
            .exec(sql)
    }

    /// Fetch the value of column `col` (1-based) in the current row.
    pub fn fetch_col(&self, handle: u32, col: usize) -> String {
        self.connections
            .get(&handle)
            .map(|c| c.fetch_col(col))
            .unwrap_or_default()
    }

    /// Advance cursor to the next row.
    ///
    /// Returns `true` if there is another row.
    pub fn next_row(&mut self, handle: u32) -> bool {
        self.connections
            .get_mut(&handle)
            .map(|c| c.next_row())
            .unwrap_or(false)
    }

    /// Total rows in the last query result.
    pub fn row_count(&self, handle: u32) -> usize {
        self.connections
            .get(&handle)
            .map(|c| c.row_count())
            .unwrap_or(0)
    }

    /// `true` when the current connection's cursor is exhausted.
    pub fn is_exhausted(&self, handle: u32) -> bool {
        self.connections
            .get(&handle)
            .map(|c| c.is_exhausted())
            .unwrap_or(true)
    }

    /// Close a connection and release it from the registry.
    pub fn close(&mut self, handle: u32) {
        self.connections.remove(&handle);
    }

    /// Close all connections (called when the interpreter finishes).
    pub fn close_all(&mut self) {
        self.connections.clear();
    }
}
