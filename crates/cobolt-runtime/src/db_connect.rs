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

/// Open a PostgreSQL connection from its parts (spec 087 R40): the
/// credentials are handed to the driver field by field and never written
/// into a URL.
pub(crate) fn open_postgres_fields(
    host: &str,
    port: Option<u16>,
    database: &str,
    user: Option<&str>,
    password: Option<&str>,
) -> Result<postgres::Client, String> {
    let mut config = postgres::Config::new();
    config.host(host);
    if let Some(port) = port {
        config.port(port);
    }
    if !database.is_empty() {
        config.dbname(database);
    }
    if let Some(user) = user {
        config.user(user);
    }
    if let Some(password) = password {
        config.password(password);
    }
    config.connect(postgres::NoTls).map_err(|e| with_causes(&e))
}

/// An error and every cause beneath it — the driver's "error connecting to
/// server" says nothing until its source adds "Connection refused".
fn with_causes(e: &dyn std::error::Error) -> String {
    let mut out = e.to_string();
    let mut cause = e.source();
    while let Some(c) = cause {
        let text = c.to_string();
        if !out.contains(&text) {
            out.push_str(": ");
            out.push_str(&text);
        }
        cause = c.source();
    }
    out
}

/// Open a MySQL connection from its parts, as [`open_postgres_fields`] does.
pub(crate) fn open_mysql_fields(
    host: &str,
    port: Option<u16>,
    database: &str,
    user: Option<&str>,
    password: Option<&str>,
) -> Result<mysql::Conn, String> {
    let opts = mysql::OptsBuilder::new()
        .ip_or_hostname(Some(host))
        .tcp_port(port.unwrap_or(3306))
        .db_name((!database.is_empty()).then_some(database))
        .user(user)
        .pass(password);
    mysql::Conn::new(opts).map_err(|e| e.to_string())
}
