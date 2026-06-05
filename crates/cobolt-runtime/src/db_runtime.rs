// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Database Runtime Engine — Phase 8.
//!
//! `DbRegistry` manages a pool of live database connections on behalf of the
//! COBOL interpreter.  Each open connection is assigned an integer *handle*
//! (stored in a COBOL `PIC 9(9)` variable) so COBOL programs can hold and
//! pass references across paragraphs.
//!
//! Three backends are supported behind one uniform interface, selected by the
//! connection string:
//!
//! | Backend     | Driver (pure Rust, no system library) |
//! |-------------|---------------------------------------|
//! | SQLite      | `rusqlite` (bundled)                  |
//! | PostgreSQL  | `postgres` (rust-postgres, sync)      |
//! | MySQL       | `mysql` (rustls, sync)                |
//!
//! The COBOL-facing CALL surface is identical for every backend — programs do
//! not change other than their connection string.
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
//! | Prefix / form                         | Backend     |
//! |---------------------------------------|-------------|
//! | _(no prefix — a file path)_           | SQLite file |
//! | `sqlite:<path>`                       | SQLite file |
//! | `:memory:`                            | SQLite (RAM)|
//! | `postgres://…` / `postgresql://…`     | PostgreSQL  |
//! | `mysql://…`                           | MySQL       |
//!
//! Examples:
//!
//! ```text
//! :memory:
//! sqlite:/var/data/app.db
//! postgres://scott:tiger@localhost:5432/store
//! mysql://scott:tiger@localhost:3306/store
//! ```

use std::collections::HashMap;

// ── Backend ─────────────────────────────────────────────────────────────────

/// Which database engine backs a connection. Used for routing and diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Sqlite,
    Postgres,
    MySql,
}

impl BackendKind {
    /// Classify a connection string into a backend (purely from its scheme).
    pub fn classify(conn_str: &str) -> Self {
        let s = conn_str.trim();
        let lower = s.to_ascii_lowercase();
        if lower.starts_with("postgres://") || lower.starts_with("postgresql://") {
            BackendKind::Postgres
        } else if lower.starts_with("mysql://") {
            BackendKind::MySql
        } else {
            BackendKind::Sqlite
        }
    }

    /// Human-readable backend name.
    pub fn name(self) -> &'static str {
        match self {
            BackendKind::Sqlite => "SQLite",
            BackendKind::Postgres => "PostgreSQL",
            BackendKind::MySql => "MySQL",
        }
    }
}

/// The live driver connection for one of the supported engines.
enum Backend {
    Sqlite(rusqlite::Connection),
    Postgres(postgres::Client),
    MySql(mysql::Conn),
}

/// The outcome of executing one statement: cached rows plus an affected-row
/// count (the latter is used for INSERT/UPDATE/DELETE/DDL).
struct ExecResult {
    rows: Vec<Vec<String>>,
    affected: usize,
}

// ── DbConn ────────────────────────────────────────────────────────────────────

/// One live database connection plus its current result-set cursor.
pub struct DbConn {
    /// The backing driver connection.
    backend: Backend,
    /// All rows fetched from the last `COBOL-EXEC-SQL` call.
    /// Each row is a `Vec<String>` of column values (every value normalised to
    /// its text form, exactly as SQLite already did).
    rows: Vec<Vec<String>>,
    /// 0-based index of the *current* row (advanced by `COBOL-NEXT-ROW`).
    cursor: usize,
    /// `true` after the cursor passes the last row.
    exhausted: bool,
}

impl DbConn {
    /// Open a new connection, dispatching on the connection-string scheme.
    fn open(conn_str: &str) -> Result<Self, String> {
        let backend = match BackendKind::classify(conn_str) {
            BackendKind::Sqlite => Backend::Sqlite(Self::open_sqlite(conn_str)?),
            BackendKind::Postgres => Backend::Postgres(Self::open_postgres(conn_str)?),
            BackendKind::MySql => Backend::MySql(Self::open_mysql(conn_str)?),
        };
        Ok(Self {
            backend,
            rows: Vec::new(),
            cursor: 0,
            exhausted: false,
        })
    }

    /// Open a SQLite connection from a file path, `sqlite:<path>`, or `:memory:`.
    fn open_sqlite(conn_str: &str) -> Result<rusqlite::Connection, String> {
        let path = conn_str
            .trim()
            .strip_prefix("sqlite:")
            .unwrap_or(conn_str.trim());
        if path == ":memory:" {
            rusqlite::Connection::open_in_memory()
        } else {
            rusqlite::Connection::open(path)
        }
        .map_err(|e| e.to_string())
    }

    /// Open a PostgreSQL connection from a `postgres://` / `postgresql://` URL.
    ///
    /// Connections are made without TLS (`NoTls`) — suitable for local and
    /// trusted-network servers. See `docs/database-runtime.md` for enabling TLS.
    fn open_postgres(conn_str: &str) -> Result<postgres::Client, String> {
        postgres::Client::connect(conn_str.trim(), postgres::NoTls).map_err(|e| e.to_string())
    }

    /// Open a MySQL connection from a `mysql://` URL.
    fn open_mysql(conn_str: &str) -> Result<mysql::Conn, String> {
        let opts = mysql::Opts::from_url(conn_str.trim()).map_err(|e| e.to_string())?;
        mysql::Conn::new(opts).map_err(|e| e.to_string())
    }

    /// Execute a SQL statement and cache the result set.
    ///
    /// For row-returning queries (`SELECT`, …) the rows are collected into
    /// `self.rows` and the row count is returned. For `INSERT / UPDATE / DELETE`
    /// (and DDL) the affected-row count is returned and `self.rows` is empty.
    fn exec(&mut self, sql: &str) -> Result<usize, String> {
        self.rows.clear();
        self.cursor = 0;
        self.exhausted = false;

        let sql = sql.trim();
        let result = match &mut self.backend {
            Backend::Sqlite(conn) => Self::exec_sqlite(conn, sql)?,
            Backend::Postgres(client) => Self::exec_postgres(client, sql)?,
            Backend::MySql(conn) => Self::exec_mysql(conn, sql)?,
        };

        self.rows = result.rows;
        // SELECT → number of rows; DML/DDL → affected rows.
        if self.rows.is_empty() {
            Ok(result.affected)
        } else {
            Ok(self.rows.len())
        }
    }

    /// SQLite execution: prepared-statement query for row-returning SQL,
    /// `execute` for everything else.
    fn exec_sqlite(conn: &rusqlite::Connection, sql: &str) -> Result<ExecResult, String> {
        let upper = sql.to_ascii_uppercase();
        let is_select =
            upper.starts_with("SELECT") || upper.starts_with("WITH") || upper.starts_with("PRAGMA");

        if is_select {
            let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
            let col_count = stmt.column_count();
            let rows_iter = stmt
                .query_map([], |row| {
                    let mut cols = Vec::with_capacity(col_count);
                    for i in 0..col_count {
                        let v: String = match row.get_ref(i) {
                            Ok(rusqlite::types::ValueRef::Null) => String::new(),
                            Ok(rusqlite::types::ValueRef::Integer(n)) => n.to_string(),
                            Ok(rusqlite::types::ValueRef::Real(f)) => f.to_string(),
                            Ok(rusqlite::types::ValueRef::Text(t)) => {
                                String::from_utf8_lossy(t).into_owned()
                            }
                            Ok(rusqlite::types::ValueRef::Blob(b)) => {
                                format!("<blob {} bytes>", b.len())
                            }
                            Err(_) => String::new(),
                        };
                        cols.push(v);
                    }
                    Ok(cols)
                })
                .map_err(|e| e.to_string())?;
            let mut rows = Vec::new();
            for row in rows_iter {
                rows.push(row.map_err(|e| e.to_string())?);
            }
            Ok(ExecResult { rows, affected: 0 })
        } else {
            let affected = conn.execute(sql, []).map_err(|e| e.to_string())?;
            Ok(ExecResult {
                rows: Vec::new(),
                affected,
            })
        }
    }

    /// PostgreSQL execution via `simple_query`, which uniformly handles both
    /// row-returning and DML statements and yields every column as text.
    fn exec_postgres(client: &mut postgres::Client, sql: &str) -> Result<ExecResult, String> {
        use postgres::SimpleQueryMessage;
        let messages = client.simple_query(sql).map_err(|e| e.to_string())?;
        let mut rows = Vec::new();
        let mut affected = 0usize;
        for msg in messages {
            match msg {
                SimpleQueryMessage::Row(r) => {
                    let n = r.columns().len();
                    let mut cols = Vec::with_capacity(n);
                    for i in 0..n {
                        cols.push(r.get(i).unwrap_or("").to_string());
                    }
                    rows.push(cols);
                }
                SimpleQueryMessage::CommandComplete(n) => affected = n as usize,
                _ => {}
            }
        }
        Ok(ExecResult { rows, affected })
    }

    /// MySQL execution via `query_iter`, normalising each `mysql::Value` to text.
    fn exec_mysql(conn: &mut mysql::Conn, sql: &str) -> Result<ExecResult, String> {
        use mysql::prelude::Queryable;
        let mut qr = conn.query_iter(sql).map_err(|e| e.to_string())?;
        let mut rows = Vec::new();
        for row_res in qr.by_ref() {
            let row = row_res.map_err(|e| e.to_string())?;
            let n = row.len();
            let mut cols = Vec::with_capacity(n);
            for i in 0..n {
                let v = row.as_ref(i).cloned().unwrap_or(mysql::Value::NULL);
                cols.push(Self::mysql_value_to_string(&v));
            }
            rows.push(cols);
        }
        let affected = qr.affected_rows() as usize;
        Ok(ExecResult { rows, affected })
    }

    /// Convert one `mysql::Value` to the same text form the other backends use.
    fn mysql_value_to_string(v: &mysql::Value) -> String {
        use mysql::Value;
        match v {
            Value::NULL => String::new(),
            Value::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
            Value::Int(n) => n.to_string(),
            Value::UInt(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Double(f) => f.to_string(),
            Value::Date(y, mo, d, h, mi, s, us) => {
                if *h == 0 && *mi == 0 && *s == 0 && *us == 0 {
                    format!("{y:04}-{mo:02}-{d:02}")
                } else {
                    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
                }
            }
            Value::Time(neg, d, h, mi, s, _us) => {
                let sign = if *neg { "-" } else { "" };
                let hours = (*d) * 24 + u32::from(*h);
                format!("{sign}{hours:02}:{mi:02}:{s:02}")
            }
        }
    }

    /// Return the value of column `col` (1-based) in the current row.
    ///
    /// Returns an empty string if the column or row is out of range.
    fn fetch_col(&self, col: usize) -> String {
        if self.exhausted {
            return String::new();
        }
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
        if self.exhausted {
            return false;
        }
        if self.cursor + 1 < self.rows.len() {
            self.cursor += 1;
            true
        } else {
            self.exhausted = true;
            false
        }
    }

    /// Total row count from the last query.
    fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// `true` when the cursor is past the last row.
    fn is_exhausted(&self) -> bool {
        self.exhausted
    }
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_routes_by_scheme() {
        assert_eq!(BackendKind::classify(":memory:"), BackendKind::Sqlite);
        assert_eq!(BackendKind::classify("sqlite:/tmp/app.db"), BackendKind::Sqlite);
        assert_eq!(BackendKind::classify("/var/data/app.db"), BackendKind::Sqlite);
        assert_eq!(
            BackendKind::classify("postgres://u:p@localhost/db"),
            BackendKind::Postgres
        );
        assert_eq!(
            BackendKind::classify("postgresql://u:p@localhost/db"),
            BackendKind::Postgres
        );
        assert_eq!(
            BackendKind::classify("  POSTGRES://u:p@localhost/db  "),
            BackendKind::Postgres
        );
        assert_eq!(
            BackendKind::classify("mysql://u:p@localhost/db"),
            BackendKind::MySql
        );
    }

    #[test]
    fn backend_names() {
        assert_eq!(BackendKind::Sqlite.name(), "SQLite");
        assert_eq!(BackendKind::Postgres.name(), "PostgreSQL");
        assert_eq!(BackendKind::MySql.name(), "MySQL");
    }

    #[test]
    fn sqlite_end_to_end_crud() {
        let mut reg = DbRegistry::new();
        let h = reg.open(":memory:").expect("open in-memory sqlite");

        reg.exec(h, "CREATE TABLE c (id INTEGER, name TEXT)").unwrap();
        let n = reg
            .exec(h, "INSERT INTO c (id, name) VALUES (1, 'ANA'), (2, 'BRUNO')")
            .unwrap();
        assert_eq!(n, 2, "two rows inserted");

        let rows = reg.exec(h, "SELECT id, name FROM c ORDER BY id").unwrap();
        assert_eq!(rows, 2);
        assert_eq!(reg.row_count(h), 2);
        assert_eq!(reg.fetch_col(h, 1), "1");
        assert_eq!(reg.fetch_col(h, 2), "ANA");
        assert!(reg.next_row(h));
        assert_eq!(reg.fetch_col(h, 2), "BRUNO");
        assert!(!reg.next_row(h));
        assert!(reg.is_exhausted(h));

        reg.close(h);
        assert_eq!(reg.row_count(h), 0);
    }

    #[test]
    fn mysql_value_to_string_covers_types() {
        use mysql::Value;
        assert_eq!(DbConn::mysql_value_to_string(&Value::NULL), "");
        assert_eq!(DbConn::mysql_value_to_string(&Value::Int(-7)), "-7");
        assert_eq!(DbConn::mysql_value_to_string(&Value::UInt(42)), "42");
        assert_eq!(
            DbConn::mysql_value_to_string(&Value::Bytes(b"hi".to_vec())),
            "hi"
        );
        assert_eq!(
            DbConn::mysql_value_to_string(&Value::Date(2026, 6, 5, 0, 0, 0, 0)),
            "2026-06-05"
        );
        assert_eq!(
            DbConn::mysql_value_to_string(&Value::Date(2026, 6, 5, 14, 30, 0, 0)),
            "2026-06-05 14:30:00"
        );
    }

    /// Live PostgreSQL round-trip. Ignored by default; set `PRC_TEST_PG_URL`
    /// (e.g. `postgres://postgres:postgres@localhost:5432/postgres`) and run
    /// `cargo test -p cobolt-runtime -- --ignored pg_live` against a real server.
    #[test]
    #[ignore = "requires a live PostgreSQL server via PRC_TEST_PG_URL"]
    fn pg_live_round_trip() {
        let url = std::env::var("PRC_TEST_PG_URL").expect("set PRC_TEST_PG_URL");
        let mut reg = DbRegistry::new();
        let h = reg.open(&url).expect("connect postgres");
        reg.exec(h, "DROP TABLE IF EXISTS prc_demo").unwrap();
        reg.exec(h, "CREATE TABLE prc_demo (id INT, name TEXT)").unwrap();
        let n = reg
            .exec(h, "INSERT INTO prc_demo VALUES (1,'ANA'),(2,'BRUNO')")
            .unwrap();
        assert_eq!(n, 2);
        let rows = reg.exec(h, "SELECT id, name FROM prc_demo ORDER BY id").unwrap();
        assert_eq!(rows, 2);
        assert_eq!(reg.fetch_col(h, 1), "1");
        assert_eq!(reg.fetch_col(h, 2), "ANA");
        reg.exec(h, "DROP TABLE prc_demo").unwrap();
        reg.close(h);
    }

    /// Live MySQL round-trip. Ignored by default; set `PRC_TEST_MYSQL_URL`
    /// (e.g. `mysql://root:root@localhost:3306/test`) and run
    /// `cargo test -p cobolt-runtime -- --ignored mysql_live` against a real server.
    #[test]
    #[ignore = "requires a live MySQL server via PRC_TEST_MYSQL_URL"]
    fn mysql_live_round_trip() {
        let url = std::env::var("PRC_TEST_MYSQL_URL").expect("set PRC_TEST_MYSQL_URL");
        let mut reg = DbRegistry::new();
        let h = reg.open(&url).expect("connect mysql");
        reg.exec(h, "DROP TABLE IF EXISTS prc_demo").unwrap();
        reg.exec(h, "CREATE TABLE prc_demo (id INT, name VARCHAR(32))").unwrap();
        let n = reg
            .exec(h, "INSERT INTO prc_demo VALUES (1,'ANA'),(2,'BRUNO')")
            .unwrap();
        assert_eq!(n, 2);
        let rows = reg.exec(h, "SELECT id, name FROM prc_demo ORDER BY id").unwrap();
        assert_eq!(rows, 2);
        assert_eq!(reg.fetch_col(h, 1), "1");
        assert_eq!(reg.fetch_col(h, 2), "ANA");
        reg.exec(h, "DROP TABLE prc_demo").unwrap();
        reg.close(h);
    }
}
