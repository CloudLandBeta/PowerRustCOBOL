// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! SQLite for embedded SQL (spec 087).
//!
//! * Values are bound, never spliced (R13). `expanded_sql()`, which splices
//!   them, is never called (R52).
//! * A decimal of up to 15 significant digits binds as REAL — a double holds
//!   that many exactly — and a longer one as TEXT, so no digit is lost.
//! * Errors map from SQLite's extended result codes to standard SQLSTATEs;
//!   where SQLite says only "error", its message decides (syntax, unknown
//!   table, unknown column).

use rusqlite::types::{Value, ValueRef};

use super::super::state::{code, SqlError};
use super::super::value::{decimal_text, SqlValue};
use super::{Backend, BackendKind, Column, Rows};

pub struct SqliteBackend {
    conn: rusqlite::Connection,
}

impl SqliteBackend {
    pub fn open(conn: &str) -> Result<Self, SqlError> {
        let c = crate::db_connect::open_sqlite(conn).map_err(|m| SqlError::new(code::CANNOT_CONNECT, m))?;
        // Wait for another writer rather than failing at once.
        let _ = c.busy_timeout(std::time::Duration::from_secs(5));
        Ok(Self { conn: c })
    }
}

fn bind(v: &SqlValue) -> Value {
    match v {
        SqlValue::Null => Value::Null,
        SqlValue::Int(i) => Value::Integer(*i),
        SqlValue::Decimal { mantissa, scale } => {
            let text = decimal_text(*mantissa, *scale);
            if SqlValue::significant_digits(*mantissa) <= 15 {
                Value::Real(text.parse().unwrap_or(0.0))
            } else {
                Value::Text(text)
            }
        }
        SqlValue::Float(f) => Value::Real(*f),
        SqlValue::Text(s) => Value::Text(s.clone()),
        SqlValue::Bytes(b) => Value::Blob(b.clone()),
        SqlValue::Bool(b) => Value::Integer(*b as i64),
    }
}

fn read(v: ValueRef<'_>) -> SqlValue {
    match v {
        ValueRef::Null => SqlValue::Null,
        ValueRef::Integer(i) => SqlValue::Int(i),
        ValueRef::Real(f) => SqlValue::Float(f),
        ValueRef::Text(t) => SqlValue::Text(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => SqlValue::Bytes(b.to_vec()),
    }
}

/// A rusqlite error as a standard SQLSTATE, keeping SQLite's own message.
pub fn map_error(e: rusqlite::Error) -> SqlError {
    use rusqlite::ffi;
    match &e {
        rusqlite::Error::SqliteFailure(f, msg) => {
            let message = msg.clone().unwrap_or_else(|| e.to_string());
            let lower = message.to_ascii_lowercase();
            let ext = f.extended_code;
            let state = match ext {
                ffi::SQLITE_CONSTRAINT_UNIQUE | ffi::SQLITE_CONSTRAINT_PRIMARYKEY => "23505",
                ffi::SQLITE_CONSTRAINT_NOTNULL => "23502",
                ffi::SQLITE_CONSTRAINT_FOREIGNKEY => "23503",
                ffi::SQLITE_CONSTRAINT_CHECK => "23514",
                _ => match f.code {
                    rusqlite::ErrorCode::ConstraintViolation => "23000",
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => "55P03",
                    rusqlite::ErrorCode::ReadOnly => "25006",
                    rusqlite::ErrorCode::CannotOpen | rusqlite::ErrorCode::NotADatabase => "08001",
                    rusqlite::ErrorCode::TypeMismatch => "42804",
                    rusqlite::ErrorCode::TooBig => "22001",
                    rusqlite::ErrorCode::DiskFull => "53100",
                    rusqlite::ErrorCode::SystemIoFailure => "58030",
                    rusqlite::ErrorCode::DatabaseCorrupt => "XX001",
                    rusqlite::ErrorCode::ParameterOutOfRange => "07009",
                    _ => match state_from_message(&lower) {
                        Some(s) => s,
                        None if f.code == rusqlite::ErrorCode::Unknown => "42000",
                        None => code::GENERAL,
                    },
                },
            };
            SqlError::new(state, message).with_native(ext as i64)
        }
        rusqlite::Error::InvalidParameterCount(..) => SqlError::new("07001", e.to_string()),
        // Errors that point at the SQL text (rusqlite's `SqlInputError`, present
        // when a crate enables `modern_sqlite`) read "<message> in <sql> at
        // offset <n>": the message decides, and only the message is kept.
        _ => {
            let text = e.to_string();
            let message = match text.rfind(" at offset ") {
                Some(p) => text[..p].find(" in ").map(|i| text[..i].to_string()).unwrap_or(text.clone()),
                None => text.clone(),
            };
            let state = state_from_message(&message.to_ascii_lowercase()).unwrap_or(code::GENERAL);
            SqlError::new(state, message)
        }
    }
}

/// The SQLSTATE an SQLite message stands for, when SQLite's code (plain
/// "error") does not say.
fn state_from_message(lower: &str) -> Option<&'static str> {
    Some(if lower.contains("syntax error") || lower.starts_with("near \"") {
        "42601"
    } else if lower.contains("no such table") {
        "42P01"
    } else if lower.contains("no such column") {
        "42703"
    } else if lower.contains("already exists") {
        "42P07"
    } else if lower.contains("no such function") {
        "42883"
    } else {
        return None;
    })
}

impl Backend for SqliteBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Sqlite
    }

    fn execute(&mut self, sql: &str, params: &[SqlValue]) -> Result<u64, SqlError> {
        let mut stmt = self.conn.prepare_cached(sql).map_err(map_error)?;
        let bound: Vec<Value> = params.iter().map(bind).collect();
        if stmt.column_count() > 0 {
            // A statement that answers rows (a PRAGMA, say) run for its effect:
            // read them so it completes, and report none affected.
            let mut rows = stmt.query(rusqlite::params_from_iter(bound.iter())).map_err(map_error)?;
            while rows.next().map_err(map_error)?.is_some() {}
            return Ok(0);
        }
        let n = stmt.execute(rusqlite::params_from_iter(bound.iter())).map_err(map_error)?;
        Ok(n as u64)
    }

    fn query(&mut self, sql: &str, params: &[SqlValue]) -> Result<Rows, SqlError> {
        let mut stmt = self.conn.prepare_cached(sql).map_err(map_error)?;
        let columns: Vec<Column> = stmt
            .columns()
            .iter()
            .map(|c| Column { name: c.name().to_string(), decl_type: c.decl_type().map(str::to_string), nullable: None })
            .collect();
        let n = columns.len();
        let bound: Vec<Value> = params.iter().map(bind).collect();
        let mut rows = stmt.query(rusqlite::params_from_iter(bound.iter())).map_err(map_error)?;
        let mut out = Vec::new();
        while let Some(r) = rows.next().map_err(map_error)? {
            let mut row = Vec::with_capacity(n);
            for i in 0..n {
                row.push(read(r.get_ref(i).map_err(map_error)?));
            }
            out.push(row);
        }
        Ok(Rows { columns, rows: out })
    }

    fn begin(&mut self) -> Result<(), SqlError> {
        self.conn.execute_batch("BEGIN").map_err(map_error)
    }

    fn commit(&mut self) -> Result<(), SqlError> {
        self.conn.execute_batch("COMMIT").map_err(map_error)
    }

    fn rollback(&mut self) -> Result<(), SqlError> {
        self.conn.execute_batch("ROLLBACK").map_err(map_error)
    }

    fn in_transaction(&mut self) -> bool {
        !self.conn.is_autocommit()
    }

    fn describe(&mut self, sql: &str) -> Result<(usize, Vec<Column>), SqlError> {
        let stmt = self.conn.prepare_cached(sql).map_err(map_error)?;
        let cols = stmt
            .columns()
            .iter()
            .map(|c| Column { name: c.name().to_string(), decl_type: c.decl_type().map(str::to_string), nullable: None })
            .collect();
        Ok((stmt.parameter_count(), cols))
    }
}
