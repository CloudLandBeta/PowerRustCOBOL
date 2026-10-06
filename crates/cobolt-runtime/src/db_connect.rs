// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Opening a database connection — shared by the `COBOL::"…-DB"` built-ins
//! ([`crate::db_runtime`]) and embedded SQL ([`crate::esql`], spec 087), so
//! both read a connection string the same way.

/// Open a SQLite connection from a file path, `sqlite:<path>`, or `:memory:`.
pub(crate) fn open_sqlite(conn_str: &str) -> Result<rusqlite::Connection, String> {
    let path = conn_str.trim().strip_prefix("sqlite:").unwrap_or(conn_str.trim());
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
/// trusted-network servers. See `docs/database-runtime-en.md`.
pub(crate) fn open_postgres(conn_str: &str) -> Result<postgres::Client, String> {
    postgres::Client::connect(conn_str.trim(), postgres::NoTls).map_err(|e| e.to_string())
}

/// Open a MySQL connection from a `mysql://` URL.
pub(crate) fn open_mysql(conn_str: &str) -> Result<mysql::Conn, String> {
    let opts = mysql::Opts::from_url(conn_str.trim()).map_err(|e| e.to_string())?;
    mysql::Conn::new(opts).map_err(|e| e.to_string())
}
