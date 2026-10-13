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
//! | Backend     | Driver (no system library needed)      |
//! |-------------|---------------------------------------|
//! | SQLite      | `rusqlite`, `features = ["bundled"]` — compiles the SQLite C amalgamation, so it is not pure Rust, but nothing is linked from the host |
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

// ── Transactions the program began (spec 095) ───────────────────────────────

/// The statements of `sql`, split at the `;` that sit outside quotes.
fn split_statements(sql: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut start, mut quote) = (0, None::<char>);
    for (at, ch) in sql.char_indices() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => {}
            None if ch == '\'' || ch == '"' || ch == '`' => quote = Some(ch),
            None if ch == ';' => {
                parts.push(&sql[start..at]);
                start = at + ch.len_utf8();
            }
            None => {}
        }
    }
    parts.push(&sql[start..]);
    parts
}

/// The words of one statement, upper-cased, with `=` as a word of its own and
/// the leading comments dropped.
fn statement_words(statement: &str) -> Vec<String> {
    let mut rest = statement.trim_start();
    loop {
        if let Some(after) = rest.strip_prefix("--") {
            rest = after.split_once('\n').map_or("", |(_, tail)| tail).trim_start();
        } else if let Some(after) = rest.strip_prefix("/*") {
            rest = after.split_once("*/").map_or("", |(_, tail)| tail).trim_start();
        } else {
            break;
        }
    }
    rest.replace('=', " = ")
        .split_whitespace()
        .map(|word| word.to_ascii_uppercase())
        .collect()
}

/// What running `sql` does to a connection's transaction: `Some(true)` opens
/// one, `Some(false)` ends it, `None` leaves it as it is. Read from the leading
/// keyword of each statement, the way the embedded-SQL backends keep their own
/// `in_tx` for PostgreSQL and MySQL — neither driver says what the server
/// thinks. `SET autocommit = 0` counts as opening one (the server starts it with
/// the next statement); `ROLLBACK TO` a savepoint and `COMMIT PREPARED` end
/// nothing of this connection's.
fn transaction_effect(sql: &str) -> Option<bool> {
    let mut effect = None;
    for statement in split_statements(sql) {
        let words = statement_words(statement);
        let word = |n: usize| words.get(n).map(String::as_str);
        let found = match word(0) {
            Some("BEGIN") => Some(true),
            Some("START") if word(1) == Some("TRANSACTION") => Some(true),
            Some("COMMIT") | Some("END") if word(1) != Some("PREPARED") => Some(false),
            Some("ROLLBACK") => {
                let after = match word(1) {
                    Some("WORK") | Some("TRANSACTION") => 2,
                    _ => 1,
                };
                match word(after) {
                    Some("TO") | Some("PREPARED") => None,
                    _ => Some(false),
                }
            }
            Some("SET") => {
                let name = word(1).map(|w| w.trim_start_matches("@@").trim_start_matches("SESSION."));
                match (name, word(2), word(3)) {
                    (Some("AUTOCOMMIT"), Some("="), Some("0" | "OFF" | "FALSE")) => Some(true),
                    (Some("AUTOCOMMIT"), Some("="), Some("1" | "ON" | "TRUE")) => Some(false),
                    _ => None,
                }
            }
            _ => None,
        };
        if found.is_some() {
            effect = found;
        }
    }
    effect
}

/// The live driver connection for one of the supported engines.
#[cfg(feature = "sql")]
enum Backend {
    Sqlite(rusqlite::Connection),
    Postgres(postgres::Client),
    MySql(mysql::Conn),
}

/// The outcome of executing one statement: the column names, cached rows, and
/// an affected-row count (the latter is used for INSERT/UPDATE/DELETE/DDL).
///
/// `columns` used to be missing, and every driver here hands the names over for
/// free — `rusqlite` from the prepared statement, `postgres` from the row,
/// `mysql` from the result set. Dropping them meant nothing above this layer
/// could ever name a column: a query's result reached COBOL as numbered
/// positions only, so a grid or a list had no headings to show and the
/// developer had to know the SELECT's shape by heart (operator, 2026-09-03:
/// "not showing the columns returned by the query").
#[cfg(feature = "sql")]
struct ExecResult {
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    affected: usize,
}

/// What a SQL verb reports when the bridge was left out of the build.
///
/// Reachable only when the compiler proved the program never touches SQL and
/// then it did anyway — in practice, a `CALL` whose verb name was assembled at
/// run time out of pieces no static reading could follow. It says which build
/// decision produced this and how to reverse it, because "COBOL-OPEN-DB failed"
/// on a machine where the same program works interpreted is otherwise
/// unexplainable.
#[cfg(not(feature = "sql"))]
const SQL_NOT_LINKED: &str = "the SQL bridge is not linked into this program: \
     the build found no CALL to COBOL-OPEN-DB (or any of its companions) and \
     left SQLite, PostgreSQL and MySQL out, which is what lets an application \
     build without a C toolchain. Reaching a SQL verb through a name assembled \
     at run time defeats that reading — CALL the verb by its literal name \
     somewhere the build can see it";

// ── DbConn ────────────────────────────────────────────────────────────────────

/// One live database connection plus its current result-set cursor.
pub struct DbConn {
    /// The backing driver connection.
    #[cfg(feature = "sql")]
    backend: Backend,
    /// The column names of the last result set, in SELECT order.
    columns: Vec<String>,
    /// All rows fetched from the last `COBOL-EXEC-SQL` call.
    /// Each row is a `Vec<String>` of column values (every value normalised to
    /// its text form, exactly as SQLite already did).
    rows: Vec<Vec<String>>,
    /// 0-based index of the *current* row (advanced by `COBOL-NEXT-ROW`).
    cursor: usize,
    /// `true` after the cursor passes the last row.
    exhausted: bool,
    /// A transaction the statements run here have opened and not yet ended
    /// (spec 095). SQLite is asked instead — see [`DbConn::in_transaction`].
    in_tx: bool,
}

impl DbConn {
    /// Whether a transaction is open on this connection now. SQLite says so
    /// itself (it may end one on its own after some errors); PostgreSQL and
    /// MySQL are read from the statements that went through [`DbConn::exec`].
    pub fn in_transaction(&self) -> bool {
        #[cfg(feature = "sql")]
        if let Backend::Sqlite(conn) = &self.backend {
            return !conn.is_autocommit();
        }
        self.in_tx
    }

    /// Run one statement — what [`DbRegistry::exec`] does, for a connection a
    /// worker holds. The count is the affected rows, or the rows of a result.
    pub fn run(&mut self, sql: &str) -> Result<usize, String> {
        self.exec(sql)
    }

    /// Open a new connection, dispatching on the connection-string scheme.
    #[cfg(feature = "sql")]
    fn open(conn_str: &str) -> Result<Self, String> {
        let backend = match BackendKind::classify(conn_str) {
            BackendKind::Sqlite => Backend::Sqlite(Self::open_sqlite(conn_str)?),
            BackendKind::Postgres => Backend::Postgres(Self::open_postgres(conn_str)?),
            BackendKind::MySql => Backend::MySql(Self::open_mysql(conn_str)?),
        };
        Ok(Self::over(backend))
    }

    #[cfg(feature = "sql")]
    fn over(backend: Backend) -> Self {
        Self {
            backend,
            columns: Vec::new(),
            rows: Vec::new(),
            cursor: 0,
            exhausted: false,
            in_tx: false,
        }
    }

    /// Open the database a named SQL connection resolved to (spec 087 R40).
    /// A SQLite file the connection does not allow creating must exist.
    #[cfg(feature = "sql")]
    fn open_target(target: &crate::esql::catalog::Target) -> Result<Self, String> {
        use crate::esql::backend::BackendKind as Kind;
        use crate::esql::catalog::Target;
        let backend = match target {
            Target::Sqlite { path, create } => {
                if !*create && !path.exists() {
                    return Err(format!("the database file {} does not exist", path.display()));
                }
                Backend::Sqlite(Self::open_sqlite(&path.display().to_string())?)
            }
            Target::ConnString(s) => return Self::open(s),
            Target::Server { kind, host, port, database, user, password } => {
                let (user, password) = (user.as_deref(), password.as_ref().map(|p| p.expose()));
                match kind {
                    Kind::MySql => Backend::MySql(crate::db_connect::open_mysql_fields(host, *port, database, user, password)?),
                    _ => Backend::Postgres(crate::db_connect::open_postgres_fields(host, *port, database, user, password)?),
                }
            }
        };
        Ok(Self::over(backend))
    }

    #[cfg(not(feature = "sql"))]
    fn open_target(_target: &crate::esql::catalog::Target) -> Result<Self, String> {
        Err(SQL_NOT_LINKED.to_string())
    }

    /// No drivers were linked, so there is nothing to open.
    #[cfg(not(feature = "sql"))]
    fn open(conn_str: &str) -> Result<Self, String> {
        Err(format!(
            "{SQL_NOT_LINKED} (asked for {})",
            BackendKind::classify(conn_str).name()
        ))
    }

    #[cfg(feature = "sql")]
    fn open_sqlite(conn_str: &str) -> Result<rusqlite::Connection, String> {
        crate::db_connect::open_sqlite(conn_str)
    }

    #[cfg(feature = "sql")]
    fn open_postgres(conn_str: &str) -> Result<postgres::Client, String> {
        crate::db_connect::open_postgres(conn_str)
    }

    #[cfg(feature = "sql")]
    fn open_mysql(conn_str: &str) -> Result<mysql::Conn, String> {
        crate::db_connect::open_mysql(conn_str)
    }

    /// Execute a SQL statement and cache the result set.
    ///
    /// For row-returning queries (`SELECT`, …) the rows are collected into
    /// `self.rows` and the row count is returned. For `INSERT / UPDATE / DELETE`
    /// (and DDL) the affected-row count is returned and `self.rows` is empty.
    #[cfg(feature = "sql")]
    fn exec(&mut self, sql: &str) -> Result<usize, String> {
        self.columns.clear();
        self.rows.clear();
        self.cursor = 0;
        self.exhausted = false;

        let sql = sql.trim();
        let result = match &mut self.backend {
            Backend::Sqlite(conn) => Self::exec_sqlite(conn, sql)?,
            Backend::Postgres(client) => Self::exec_postgres(client, sql)?,
            Backend::MySql(conn) => Self::exec_mysql(conn, sql)?,
        };

        self.columns = result.columns;
        self.rows = result.rows;
        if let Some(open) = transaction_effect(sql) {
            self.in_tx = open;
        }
        // SELECT → number of rows; DML/DDL → affected rows.
        if self.rows.is_empty() {
            Ok(result.affected)
        } else {
            Ok(self.rows.len())
        }
    }

    /// Unreachable — no connection can exist without drivers — but the registry
    /// still names this method, so it has to compile.
    #[cfg(not(feature = "sql"))]
    fn exec(&mut self, _sql: &str) -> Result<usize, String> {
        Err(SQL_NOT_LINKED.to_owned())
    }

    /// SQLite execution: prepared-statement query for row-returning SQL,
    /// `execute` for everything else.
    #[cfg(feature = "sql")]
    fn exec_sqlite(conn: &rusqlite::Connection, sql: &str) -> Result<ExecResult, String> {
        let upper = sql.to_ascii_uppercase();
        let is_select =
            upper.starts_with("SELECT") || upper.starts_with("WITH") || upper.starts_with("PRAGMA");

        if is_select {
            let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
            let col_count = stmt.column_count();
            // Taken from the PREPARED STATEMENT, not from a row: a SELECT that
            // matches nothing still has columns, and a caller asking what the
            // query returns deserves an answer when it returns no rows.
            let columns: Vec<String> = stmt
                .column_names()
                .into_iter()
                .map(str::to_owned)
                .collect();
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
            Ok(ExecResult {
                columns,
                rows,
                affected: 0,
            })
        } else {
            let affected = conn.execute(sql, []).map_err(|e| e.to_string())?;
            Ok(ExecResult {
                columns: Vec::new(),
                rows: Vec::new(),
                affected,
            })
        }
    }

    /// PostgreSQL execution via `simple_query`, which uniformly handles both
    /// row-returning and DML statements and yields every column as text.
    #[cfg(feature = "sql")]
    fn exec_postgres(client: &mut postgres::Client, sql: &str) -> Result<ExecResult, String> {
        use postgres::SimpleQueryMessage;
        let messages = client.simple_query(sql).map_err(|e| e.to_string())?;
        let mut rows = Vec::new();
        let mut columns: Vec<String> = Vec::new();
        let mut affected = 0usize;
        for msg in messages {
            match msg {
                SimpleQueryMessage::Row(r) => {
                    let n = r.columns().len();
                    if columns.is_empty() {
                        columns = r.columns().iter().map(|c| c.name().to_owned()).collect();
                    }
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
        Ok(ExecResult {
            columns,
            rows,
            affected,
        })
    }

    /// MySQL execution via `query_iter`, normalising each `mysql::Value` to text.
    #[cfg(feature = "sql")]
    fn exec_mysql(conn: &mut mysql::Conn, sql: &str) -> Result<ExecResult, String> {
        use mysql::prelude::Queryable;
        let mut qr = conn.query_iter(sql).map_err(|e| e.to_string())?;
        let columns: Vec<String> = qr
            .columns()
            .as_ref()
            .iter()
            .map(|c| c.name_str().into_owned())
            .collect();
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
        Ok(ExecResult {
            columns,
            rows,
            affected,
        })
    }

    /// Convert one `mysql::Value` to the same text form the other backends use.
    #[cfg(feature = "sql")]
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

    /// Whether this is a MySQL connection (see [`DbRegistry::is_mysql`]).
    fn is_mysql(&self) -> bool {
        #[cfg(feature = "sql")]
        {
            matches!(self.backend, Backend::MySql(_))
        }
        #[cfg(not(feature = "sql"))]
        {
            false
        }
    }

    /// `true` when the cursor is past the last row.
    fn is_exhausted(&self) -> bool {
        self.exhausted
    }

    /// The current row's values, then advance — the shape a
    /// `PERFORM UNTIL <empty>` loop needs, and what `Db::Fetch()` returns.
    ///
    /// `None` once the rows run out, so the caller's loop ends on its own
    /// instead of needing a separate "is there more" question. This is
    /// deliberately a *different* traversal from `fetch_col` + `next_row`,
    /// which read the current row in place and step separately: mixing the two
    /// on one handle will skip rows, because this one consumes as it reads.
    fn take_row(&mut self) -> Option<Vec<String>> {
        if self.exhausted || self.cursor >= self.rows.len() {
            self.exhausted = true;
            return None;
        }
        let row = self.rows[self.cursor].clone();
        if self.cursor + 1 < self.rows.len() {
            self.cursor += 1;
        } else {
            self.exhausted = true;
        }
        Some(row)
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
    /// Handles whose connection is out on a background worker (spec 032,
    /// `Mode = Async`). Closing one drops the connection when it comes back.
    lent: std::collections::HashSet<u32>,
    /// Every statement sent through [`DbRegistry::exec`], when a test asked for
    /// it (`record_statements`). `None` — the default — records nothing.
    statement_log: Option<Vec<String>>,
}

/// A connection travelling to or from a background worker. Shared and
/// optional so the outcome that carries it stays `Clone` + `Debug`; the
/// receiver `take`s it exactly once.
#[derive(Clone)]
pub struct LentConnection(pub std::sync::Arc<std::sync::Mutex<Option<DbConn>>>);

impl LentConnection {
    pub fn new(conn: DbConn) -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(Some(conn))))
    }

    /// The connection, once; `None` after the first call.
    pub fn take(&self) -> Option<DbConn> {
        self.0.lock().ok().and_then(|mut c| c.take())
    }
}

impl std::fmt::Debug for LentConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LentConnection")
    }
}

impl DbRegistry {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            next_handle: 1,
            lent: Default::default(),
            statement_log: None,
        }
    }

    /// Start keeping the text of every statement `exec` is given (spec 095 AC3:
    /// a test asserts `BEGIN`, the `UPDATE`s, `COMMIT`, or the absence of a
    /// `BEGIN`). Not for production use.
    #[doc(hidden)]
    pub fn record_statements(&mut self) {
        self.statement_log.get_or_insert_with(Vec::new);
    }

    /// The statements kept since [`DbRegistry::record_statements`], in order.
    #[doc(hidden)]
    pub fn recorded_statements(&self) -> Vec<String> {
        self.statement_log.clone().unwrap_or_default()
    }

    /// Whether a transaction is open on `handle` (spec 095): the program began
    /// one with `BEGIN`, so a unit of work must not begin — or end — its own.
    pub fn in_transaction(&self, handle: u32) -> bool {
        self.connections.get(&handle).is_some_and(DbConn::in_transaction)
    }

    /// Open a new connection and return its handle.
    ///
    /// Returns `Err(message)` if the connection fails.
    pub fn open(&mut self, conn_str: &str) -> Result<u32, String> {
        let conn = DbConn::open(conn_str)?;
        Ok(self.register(conn))
    }

    /// Open the database a named SQL connection resolved to (spec 087 R40)
    /// and return its handle — a connection of this registry's own, apart
    /// from those of `EXEC SQL`.
    pub fn open_target(&mut self, target: &crate::esql::catalog::Target) -> Result<u32, String> {
        let conn = DbConn::open_target(target)?;
        Ok(self.register(conn))
    }

    fn register(&mut self, conn: DbConn) -> u32 {
        let handle = self.next_handle;
        self.next_handle += 1;
        self.connections.insert(handle, conn);
        handle
    }

    /// Execute SQL on an existing connection.
    ///
    /// Returns `Ok(row_count_or_affected_rows)` or `Err(message)`.
    pub fn exec(&mut self, handle: u32, sql: &str) -> Result<usize, String> {
        if let Some(log) = &mut self.statement_log {
            log.push(sql.trim().to_owned());
        }
        if self.lent.contains(&handle) {
            return Err(self.busy_message(handle));
        }
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

    /// The column names of the last result set, in SELECT order.
    ///
    /// Available even when the query matched no rows — they come from the
    /// prepared statement, not from a row.
    pub fn columns(&self, handle: u32) -> Vec<String> {
        self.connections
            .get(&handle)
            .map(|c| c.columns.clone())
            .unwrap_or_default()
    }

    /// The current row's values, then advance. `None` when the rows run out.
    ///
    /// See [`DbConn::take_row`]: this consumes as it reads, so it is not to be
    /// interleaved with `fetch_col`/`next_row` on the same handle.
    pub fn take_row(&mut self, handle: u32) -> Option<Vec<String>> {
        self.connections.get_mut(&handle).and_then(|c| c.take_row())
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

    /// `true` when `handle` is a MySQL connection — the one engine whose string
    /// literals treat a backslash as an escape (spec 094 writes edited values as
    /// literals, so it must know).
    pub fn is_mysql(&self, handle: u32) -> bool {
        self.connections.get(&handle).is_some_and(|c| c.is_mysql())
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

    /// Close a connection and release it from the registry. A connection out
    /// on a worker is dropped when it comes back.
    pub fn close(&mut self, handle: u32) {
        self.connections.remove(&handle);
        self.lent.remove(&handle);
    }

    /// Hand the connection to a background worker (spec 032). Until it is
    /// given back, every other use of the handle reports it busy.
    pub fn lend(&mut self, handle: u32) -> Result<DbConn, String> {
        if self.lent.contains(&handle) {
            return Err(self.busy_message(handle));
        }
        let conn = self
            .connections
            .remove(&handle)
            .ok_or_else(|| format!("No open connection with handle {handle}"))?;
        self.lent.insert(handle);
        Ok(conn)
    }

    /// A lent connection is back — whether its statement finished, failed,
    /// timed out or was cancelled. Kept only if the handle was not closed
    /// meanwhile.
    pub fn give_back(&mut self, handle: u32, conn: DbConn) {
        if self.lent.remove(&handle) {
            self.connections.insert(handle, conn);
        }
    }

    fn busy_message(&self, handle: u32) -> String {
        format!(
            "connection {handle} is busy with an asynchronous statement — wait for its \
             onQueryComplete (or onQueryError / onTimeout)"
        )
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
        assert_eq!(
            BackendKind::classify("sqlite:/tmp/app.db"),
            BackendKind::Sqlite
        );
        assert_eq!(
            BackendKind::classify("/var/data/app.db"),
            BackendKind::Sqlite
        );
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

    /// **A query reports its columns, and `take_row` hands back the data.**
    ///
    /// Both halves of the 2026-09-03 report: the names were dropped by
    /// `ExecResult`, and the only row traversal on the `::` surface returned a
    /// 1/0 flag, so a query's result could reach COBOL only as numbered
    /// positions through the CALL surface.
    #[cfg(feature = "sql")]
    #[test]
    fn a_query_reports_its_columns_and_yields_its_rows() {
        let mut reg = DbRegistry::new();
        let h = reg.open(":memory:").expect("in-memory sqlite opens");
        reg.exec(h, "CREATE TABLE persons (id INTEGER, name TEXT)")
            .expect("create");
        reg.exec(h, "INSERT INTO persons VALUES (1, 'Ada'), (2, 'Grace')")
            .expect("insert");

        let n = reg
            .exec(h, "SELECT id, name FROM persons ORDER BY id")
            .expect("select");
        assert_eq!(n, 2, "two rows");

        // The names the SELECT asked for, in its order.
        assert_eq!(reg.columns(h), vec!["id".to_string(), "name".to_string()]);

        // Each row once, in order, then nothing — the loop shape COBOL uses.
        assert_eq!(
            reg.take_row(h),
            Some(vec!["1".to_string(), "Ada".to_string()])
        );
        assert_eq!(
            reg.take_row(h),
            Some(vec!["2".to_string(), "Grace".to_string()])
        );
        assert_eq!(reg.take_row(h), None, "the result set is spent");
        assert_eq!(reg.take_row(h), None, "and stays spent");

        reg.close(h);
    }

    /// A SELECT that matches nothing still has columns — they come from the
    /// prepared statement, not from a row. Taking them from the first row
    /// would report none here, which is exactly when a caller most needs to
    /// know the shape of what it asked for.
    #[cfg(feature = "sql")]
    #[test]
    fn an_empty_result_set_still_names_its_columns() {
        let mut reg = DbRegistry::new();
        let h = reg.open(":memory:").expect("in-memory sqlite opens");
        reg.exec(h, "CREATE TABLE t (a INTEGER, b TEXT, c REAL)")
            .expect("create");

        let n = reg.exec(h, "SELECT a, b, c FROM t").expect("select");
        assert_eq!(n, 0, "no rows");
        assert_eq!(
            reg.columns(h),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert_eq!(reg.take_row(h), None);

        reg.close(h);
    }

    /// Spec 095: which statements open, end or leave alone a transaction — the
    /// reading PostgreSQL and MySQL connections rely on, since neither driver
    /// says what the server thinks.
    #[test]
    fn the_leading_keyword_of_a_statement_says_what_it_does_to_a_transaction() {
        let cases: &[(&str, Option<bool>)] = &[
            ("BEGIN", Some(true)),
            ("  begin;", Some(true)),
            ("BEGIN TRANSACTION", Some(true)),
            ("START TRANSACTION", Some(true)),
            ("start   transaction read only", Some(true)),
            ("SET autocommit = 0", Some(true)),
            ("set @@session.autocommit=OFF", Some(true)),
            ("SET autocommit = 1", Some(false)),
            ("COMMIT", Some(false)),
            ("commit work", Some(false)),
            ("END", Some(false)),
            ("ROLLBACK", Some(false)),
            ("ROLLBACK WORK", Some(false)),
            ("ROLLBACK TO SAVEPOINT sp1", None),
            ("rollback transaction to sp1", None),
            ("COMMIT PREPARED 'tx1'", None),
            ("-- a note\nBEGIN", Some(true)),
            ("/* a note */ COMMIT", Some(false)),
            ("BEGIN; UPDATE t SET a = 1; COMMIT", Some(false)),
            ("COMMIT; BEGIN", Some(true)),
            ("SELECT 'BEGIN'", None),
            ("UPDATE t SET note = 'x; COMMIT; y'", None),
            ("SET names utf8", None),
            ("SELECT 1", None),
            ("", None),
        ];
        for (sql, expected) in cases {
            assert_eq!(transaction_effect(sql), *expected, "{sql:?}");
        }
        println!(
            "\n  transaction scanner: {} statements read, each one's effect on a transaction as expected\n",
            cases.len()
        );
    }

    /// SQLite answers for itself; the statements the registry runs are kept in
    /// order when a test asks for them.
    #[cfg(feature = "sql")]
    #[test]
    fn a_transaction_is_seen_open_and_ended_and_the_statements_are_recorded() {
        let mut reg = DbRegistry::new();
        reg.record_statements();
        let h = reg.open(":memory:").expect("in-memory sqlite opens");
        assert!(!reg.in_transaction(h), "a fresh connection has none");
        reg.exec(h, "CREATE TABLE t (a INTEGER)").expect("create");
        reg.exec(h, "BEGIN").expect("begin");
        assert!(reg.in_transaction(h), "BEGIN opens one");
        reg.exec(h, "INSERT INTO t VALUES (1)").expect("insert");
        assert!(reg.in_transaction(h), "still open");
        reg.exec(h, "ROLLBACK").expect("rollback");
        assert!(!reg.in_transaction(h), "ROLLBACK ends it");
        reg.exec(h, "BEGIN").expect("begin again");
        reg.exec(h, "COMMIT").expect("commit");
        assert!(!reg.in_transaction(h), "COMMIT ends it");
        assert!(!reg.in_transaction(999), "no such connection");
        assert_eq!(
            reg.recorded_statements(),
            ["CREATE TABLE t (a INTEGER)", "BEGIN", "INSERT INTO t VALUES (1)", "ROLLBACK", "BEGIN", "COMMIT"]
        );
        // A registry that was not asked to record keeps nothing.
        let mut quiet = DbRegistry::new();
        let q = quiet.open(":memory:").expect("opens");
        quiet.exec(q, "SELECT 1").expect("select");
        assert!(quiet.recorded_statements().is_empty());
        println!("\n  SQLite transaction state follows BEGIN / ROLLBACK / COMMIT; 6 statements recorded in order\n");
    }

    #[test]
    fn backend_names() {
        assert_eq!(BackendKind::Sqlite.name(), "SQLite");
        assert_eq!(BackendKind::Postgres.name(), "PostgreSQL");
        assert_eq!(BackendKind::MySql.name(), "MySQL");
    }

    #[test]
    #[cfg(feature = "sql")]
    fn sqlite_end_to_end_crud() {
        let mut reg = DbRegistry::new();
        let h = reg.open(":memory:").expect("open in-memory sqlite");

        reg.exec(h, "CREATE TABLE c (id INTEGER, name TEXT)")
            .unwrap();
        let n = reg
            .exec(
                h,
                "INSERT INTO c (id, name) VALUES (1, 'ANA'), (2, 'BRUNO')",
            )
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
    #[cfg(feature = "sql")]
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
    #[cfg(feature = "sql")]
    #[ignore = "requires a live PostgreSQL server via PRC_TEST_PG_URL"]
    fn pg_live_round_trip() {
        let url = std::env::var("PRC_TEST_PG_URL").expect("set PRC_TEST_PG_URL");
        let mut reg = DbRegistry::new();
        let h = reg.open(&url).expect("connect postgres");
        reg.exec(h, "DROP TABLE IF EXISTS prc_demo").unwrap();
        reg.exec(h, "CREATE TABLE prc_demo (id INT, name TEXT)")
            .unwrap();
        let n = reg
            .exec(h, "INSERT INTO prc_demo VALUES (1,'ANA'),(2,'BRUNO')")
            .unwrap();
        assert_eq!(n, 2);
        let rows = reg
            .exec(h, "SELECT id, name FROM prc_demo ORDER BY id")
            .unwrap();
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
    #[cfg(feature = "sql")]
    #[ignore = "requires a live MySQL server via PRC_TEST_MYSQL_URL"]
    fn mysql_live_round_trip() {
        let url = std::env::var("PRC_TEST_MYSQL_URL").expect("set PRC_TEST_MYSQL_URL");
        let mut reg = DbRegistry::new();
        let h = reg.open(&url).expect("connect mysql");
        reg.exec(h, "DROP TABLE IF EXISTS prc_demo").unwrap();
        reg.exec(h, "CREATE TABLE prc_demo (id INT, name VARCHAR(32))")
            .unwrap();
        let n = reg
            .exec(h, "INSERT INTO prc_demo VALUES (1,'ANA'),(2,'BRUNO')")
            .unwrap();
        assert_eq!(n, 2);
        let rows = reg
            .exec(h, "SELECT id, name FROM prc_demo ORDER BY id")
            .unwrap();
        assert_eq!(rows, 2);
        assert_eq!(reg.fetch_col(h, 1), "1");
        assert_eq!(reg.fetch_col(h, 2), "ANA");
        reg.exec(h, "DROP TABLE prc_demo").unwrap();
        reg.close(h);
    }
}
