// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The databases embedded SQL reaches (spec 087): one trait, one module per
//! driver. Without the `sql` feature no driver is linked and every connection
//! answers `08001` with the reason ([`unlinked`]).

use super::state::SqlError;
use super::value::SqlValue;

#[cfg(feature = "sql")]
pub mod mysql;
#[cfg(feature = "sql")]
pub mod postgres;
#[cfg(feature = "sql")]
pub mod sqlite;
pub mod unlinked;

/// Which database a connection is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Sqlite,
    Postgres,
    MySql,
}

impl BackendKind {
    /// The database a connection string names, by its scheme — the rule the
    /// `COBOL::"OPEN-DB"` built-in has always used.
    pub fn of(conn: &str) -> Self {
        let t = conn.trim().to_ascii_lowercase();
        if t.starts_with("postgres://") || t.starts_with("postgresql://") {
            BackendKind::Postgres
        } else if t.starts_with("mysql://") {
            BackendKind::MySql
        } else {
            BackendKind::Sqlite
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            BackendKind::Sqlite => "SQLite",
            BackendKind::Postgres => "PostgreSQL",
            BackendKind::MySql => "MySQL",
        }
    }
}

/// A result column, or a parameter (named `?1`, `?2` …).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Column {
    pub name: String,
    /// The declared type, as the database reports it, when it does.
    pub decl_type: Option<String>,
    /// Whether the column accepts NULL; `None` when the database does not say.
    pub nullable: Option<bool>,
}

/// A whole result.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Rows {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<SqlValue>>,
}

/// One open connection. Statements arrive with their placeholders already in
/// the database's own style and their values in order.
pub trait Backend: Send {
    fn kind(&self) -> BackendKind;
    /// Run a statement that returns no rows; the rows it affected.
    fn execute(&mut self, sql: &str, params: &[SqlValue]) -> Result<u64, SqlError>;
    /// Run a query and read all of its rows.
    fn query(&mut self, sql: &str, params: &[SqlValue]) -> Result<Rows, SqlError>;
    fn begin(&mut self) -> Result<(), SqlError>;
    fn commit(&mut self) -> Result<(), SqlError>;
    fn rollback(&mut self) -> Result<(), SqlError>;
    /// Whether a transaction is open now — asked of the database, because it
    /// may end one on its own (SQLite after some errors, MySQL after DDL).
    fn in_transaction(&mut self) -> bool;
    /// A statement's parameters — with their types where the database
    /// reports them (PostgreSQL does; SQLite and MySQL do not) — and its
    /// result columns.
    fn describe(&mut self, sql: &str) -> Result<(Vec<Column>, Vec<Column>), SqlError>;
    /// How a row of `table` is named for `WHERE CURRENT OF` (R26): pairs of
    /// (what the cursor's query selects, what the condition compares) — one
    /// pair per key column.
    fn row_key(&mut self, table: &str) -> Result<Vec<(String, String)>, SqlError>;
}

/// Open a connection from a connection string (R34: the caller has already
/// decided it is one).
pub fn open(conn: &str) -> Result<Box<dyn Backend>, SqlError> {
    #[cfg(feature = "sql")]
    {
        match BackendKind::of(conn) {
            BackendKind::Sqlite => return Ok(Box::new(sqlite::SqliteBackend::open(conn)?)),
            BackendKind::Postgres => return Ok(Box::new(postgres::PostgresBackend::open_url(conn)?)),
            BackendKind::MySql => return Ok(Box::new(mysql::MySqlBackend::open_url(conn)?)),
        }
    }
    #[cfg(not(feature = "sql"))]
    {
        unlinked::open(conn)
    }
}

/// Open a resolved target (spec 087 R33–R35): a SQLite file — never created
/// unless the SQL connection allows it, so a wrong path is an error rather
/// than a new empty database — a connection string, or a server.
pub fn open_target(target: &super::catalog::Target) -> Result<Box<dyn Backend>, SqlError> {
    use super::catalog::Target;
    match target {
        Target::Sqlite { path, create } => {
            if !*create && !path.exists() {
                return Err(SqlError::new(
                    super::state::code::CANNOT_CONNECT,
                    format!("the database file {} does not exist", path.display()),
                ));
            }
            open(&format!("sqlite:{}", path.display()))
        }
        Target::ConnString(s) => open(s),
        #[cfg(feature = "sql")]
        Target::Server { kind, host, port, database, user, password } => {
            let (user, password) = (user.as_deref(), password.as_ref().map(|p| p.expose()));
            match kind {
                BackendKind::MySql => Ok(Box::new(mysql::MySqlBackend::open_fields(host, *port, database, user, password)?)),
                _ => Ok(Box::new(postgres::PostgresBackend::open_fields(host, *port, database, user, password)?)),
            }
        }
        #[cfg(not(feature = "sql"))]
        Target::Server { kind, .. } => unlinked::open(&format!("{}://", kind.name().to_ascii_lowercase())),
    }
}

/// `n` parameters whose types the database does not report.
pub fn untyped_params(n: usize) -> Vec<Column> {
    (1..=n).map(|i| Column { name: format!("?{i}"), decl_type: None, nullable: None }).collect()
}
