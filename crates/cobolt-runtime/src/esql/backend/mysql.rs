// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! MySQL for embedded SQL (spec 087).
//!
//! * Statements are prepared and values bound positionally; a decimal travels
//!   as its exact text, so MySQL parses it the way it parses a literal.
//! * `CLIENT_FOUND_ROWS`: an UPDATE reports the rows it MATCHED, as every other
//!   database does, so one that changes nothing is not "no data".
//! * MySQL's SQLSTATE is often the catch-all `HY000` or a class-only `23000`;
//!   the error number refines it (the table in [`refine`]).
//! * A data-definition statement commits the unit of work on its own; the
//!   next statement starts a new one.
//! * `mysql::Opts` is built field by field and never formatted — it would
//!   print the password.

use mysql::consts::{CapabilityFlags, ColumnFlags, ColumnType};
use mysql::prelude::Queryable;

use super::super::state::{code, SqlError};
use super::super::value::{decimal_text, parse_decimal, SqlValue};
use super::{Backend, BackendKind, Column, Rows};

pub struct MySqlBackend {
    conn: mysql::Conn,
    in_tx: bool,
}

impl MySqlBackend {
    /// From a `mysql://` connection string.
    pub fn open_url(url: &str) -> Result<Self, SqlError> {
        let opts = mysql::Opts::from_url(url.trim()).map_err(|e| SqlError::new(code::CANNOT_CONNECT, e.to_string()))?;
        let builder = mysql::OptsBuilder::from_opts(opts).additional_capabilities(CapabilityFlags::CLIENT_FOUND_ROWS);
        Self::connect(builder)
    }

    /// From its parts, field by field: the password never enters a URL.
    pub fn open_fields(
        host: &str,
        port: Option<u16>,
        database: &str,
        user: Option<&str>,
        password: Option<&str>,
    ) -> Result<Self, SqlError> {
        let builder = mysql::OptsBuilder::new()
            .ip_or_hostname(Some(host))
            .tcp_port(port.unwrap_or(3306))
            .db_name((!database.is_empty()).then_some(database))
            .user(user)
            .pass(password)
            .additional_capabilities(CapabilityFlags::CLIENT_FOUND_ROWS);
        Self::connect(builder)
    }

    fn connect(builder: mysql::OptsBuilder) -> Result<Self, SqlError> {
        let conn = mysql::Conn::new(builder).map_err(|e| match map_error(e) {
            // A refused login keeps its own class; anything else is "cannot connect".
            e if e.sqlstate.starts_with("28") => e,
            e => SqlError::new(code::CANNOT_CONNECT, e.message).with_native(e.native),
        })?;
        Ok(Self { conn, in_tx: false })
    }
}

/// A value as a MySQL parameter.
pub fn param(v: &SqlValue) -> mysql::Value {
    match v {
        SqlValue::Null => mysql::Value::NULL,
        SqlValue::Int(i) => mysql::Value::Int(*i),
        SqlValue::Decimal { mantissa, scale } => mysql::Value::Bytes(decimal_text(*mantissa, *scale).into_bytes()),
        SqlValue::Float(f) => mysql::Value::Double(*f),
        SqlValue::Text(s) => mysql::Value::Bytes(s.clone().into_bytes()),
        SqlValue::Bytes(b) => mysql::Value::Bytes(b.clone()),
        SqlValue::Bool(b) => mysql::Value::Int(*b as i64),
    }
}

/// What a result value of a column of type `ty` is. `binary` is a column of
/// the binary character set (BLOB, BINARY): its bytes are not text.
pub fn read(v: mysql::Value, ty: ColumnType, binary: bool) -> SqlValue {
    use mysql::Value as V;
    match v {
        V::NULL => SqlValue::Null,
        V::Int(i) => SqlValue::Int(i),
        V::UInt(u) => i64::try_from(u).map(SqlValue::Int).unwrap_or(SqlValue::Decimal { mantissa: u as i128, scale: 0 }),
        V::Float(f) => SqlValue::Float(f as f64),
        V::Double(f) => SqlValue::Float(f),
        V::Bytes(b) => match ty {
            ColumnType::MYSQL_TYPE_NEWDECIMAL | ColumnType::MYSQL_TYPE_DECIMAL => {
                let t = String::from_utf8_lossy(&b);
                match parse_decimal(&t) {
                    Some((m, s)) => SqlValue::decimal(m, s),
                    None => SqlValue::Text(t.into_owned()),
                }
            }
            _ if binary => SqlValue::Bytes(b),
            _ => SqlValue::Text(String::from_utf8_lossy(&b).into_owned()),
        },
        V::Date(y, mo, d, h, mi, s, us) => {
            if ty == ColumnType::MYSQL_TYPE_DATE {
                SqlValue::Text(format!("{y:04}-{mo:02}-{d:02}"))
            } else {
                SqlValue::Text(format!("{y:04}-{mo:02}-{d:02} {}", clock(h as u32, mi, s, us)))
            }
        }
        V::Time(neg, days, h, mi, s, us) => {
            SqlValue::Text(format!("{}{}", if neg { "-" } else { "" }, clock(days * 24 + h as u32, mi, s, us)))
        }
    }
}

fn clock(h: u32, mi: u8, s: u8, us: u32) -> String {
    let base = format!("{h:02}:{mi:02}:{s:02}");
    if us == 0 {
        base
    } else {
        format!("{base}.{}", format!("{us:06}").trim_end_matches('0'))
    }
}

/// The SQLSTATE a MySQL error number stands for, where MySQL's own state is
/// too coarse (`HY000`, or `23000` for every integrity violation).
pub fn refine(number: u16, state: &str) -> String {
    let s = match number {
        1062 | 1586 => "23505",                    // duplicate key
        1048 | 1364 => "23502",                    // NULL into NOT NULL / no default
        1451 | 1452 | 1216 | 1217 => "23503",      // foreign key
        3819 => "23514",                           // check constraint
        1146 => "42P01",                           // no such table
        1054 => "42703",                           // no such column
        1050 => "42P07",                           // table already exists
        1064 | 1149 => "42601",                    // syntax error
        1305 => "42883",                           // no such function
        1406 => "22001",                           // data too long
        1264 | 1690 => "22003",                    // out of range
        1292 | 1366 => "22007",                    // invalid date / value
        1365 => "22012",                           // division by zero
        1213 => "40001",                           // deadlock
        1205 => "55P03",                           // lock wait timeout
        1044 | 1045 | 1698 => "28000",             // access denied
        1049 => "3D000",                           // unknown database
        2002 | 2003 | 2005 => "08001",             // cannot connect
        2006 | 2013 => "08006",                    // connection lost
        _ => return if state.is_empty() { code::GENERAL.to_string() } else { state.to_string() },
    };
    s.to_string()
}

pub fn map_error(e: mysql::Error) -> SqlError {
    match e {
        mysql::Error::MySqlError(m) => SqlError::new(&refine(m.code, &m.state), m.message).with_native(m.code as i64),
        mysql::Error::IoError(io) => SqlError::new("08006", io.to_string()),
        // The driver's own sentence, without the variant's wrapping.
        mysql::Error::DriverError(d) => SqlError::new(code::GENERAL, d.to_string()),
        other => SqlError::new(code::GENERAL, other.to_string()),
    }
}

/// A statement that commits the unit of work on its own (MySQL's implicit
/// commit): data definition and account management.
fn commits_implicitly(sql: &str) -> bool {
    let first = sql.trim_start().split(|c: char| !c.is_ascii_alphabetic()).next().unwrap_or("").to_ascii_uppercase();
    matches!(first.as_str(), "CREATE" | "ALTER" | "DROP" | "TRUNCATE" | "RENAME" | "GRANT" | "REVOKE" | "LOCK" | "UNLOCK")
}

/// A MySQL column type as the standard SQL type it is, so a descriptor
/// classifies it the way it classifies the other databases' types.
pub fn type_name(ty: ColumnType, length: u32, decimals: u8, binary: bool, unsigned: bool) -> String {
    use ColumnType as T;
    match ty {
        T::MYSQL_TYPE_TINY | T::MYSQL_TYPE_SHORT | T::MYSQL_TYPE_LONG | T::MYSQL_TYPE_INT24
        | T::MYSQL_TYPE_LONGLONG | T::MYSQL_TYPE_YEAR => "INTEGER".into(),
        T::MYSQL_TYPE_NEWDECIMAL | T::MYSQL_TYPE_DECIMAL => {
            // The display length holds the digits plus a sign (unless unsigned)
            // and a point (when there is a fraction).
            let overhead = (!unsigned) as u32 + (decimals > 0) as u32;
            format!("DECIMAL({},{decimals})", length.saturating_sub(overhead))
        }
        T::MYSQL_TYPE_FLOAT | T::MYSQL_TYPE_DOUBLE => "DOUBLE".into(),
        T::MYSQL_TYPE_VARCHAR | T::MYSQL_TYPE_VAR_STRING | T::MYSQL_TYPE_STRING => {
            if binary { "VARBINARY" } else { "VARCHAR" }.into()
        }
        T::MYSQL_TYPE_TINY_BLOB | T::MYSQL_TYPE_MEDIUM_BLOB | T::MYSQL_TYPE_LONG_BLOB | T::MYSQL_TYPE_BLOB => {
            if binary { "BLOB" } else { "TEXT" }.into()
        }
        T::MYSQL_TYPE_JSON => "TEXT".into(),
        T::MYSQL_TYPE_DATE | T::MYSQL_TYPE_NEWDATE => "DATE".into(),
        T::MYSQL_TYPE_DATETIME | T::MYSQL_TYPE_DATETIME2 | T::MYSQL_TYPE_TIMESTAMP | T::MYSQL_TYPE_TIMESTAMP2 => {
            "TIMESTAMP".into()
        }
        T::MYSQL_TYPE_TIME | T::MYSQL_TYPE_TIME2 => "TIME".into(),
        T::MYSQL_TYPE_BIT => "BOOLEAN".into(),
        other => format!("{other:?}").trim_start_matches("MYSQL_TYPE_").to_string(),
    }
}

fn columns_of(cols: &[mysql::Column]) -> Vec<Column> {
    cols.iter()
        .map(|c| Column {
            name: c.name_str().into_owned(),
            decl_type: Some(type_name(c.column_type(), c.column_length(), c.decimals(), c.character_set() == 63, c.flags().contains(ColumnFlags::UNSIGNED_FLAG))),
            nullable: Some(!c.flags().contains(ColumnFlags::NOT_NULL_FLAG)),
        })
        .collect()
}

impl Backend for MySqlBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::MySql
    }

    fn execute(&mut self, sql: &str, params: &[SqlValue]) -> Result<u64, SqlError> {
        let p: Vec<mysql::Value> = params.iter().map(param).collect();
        let r = self.conn.exec_drop(sql, mysql::Params::Positional(p)).map_err(map_error);
        if commits_implicitly(sql) {
            self.in_tx = false;
        }
        r?;
        Ok(self.conn.affected_rows())
    }

    fn query(&mut self, sql: &str, params: &[SqlValue]) -> Result<Rows, SqlError> {
        let p: Vec<mysql::Value> = params.iter().map(param).collect();
        let result = self.conn.exec_iter(sql, mysql::Params::Positional(p)).map_err(map_error)?;
        let cols: Vec<mysql::Column> = result.columns().as_ref().to_vec();
        let columns = columns_of(&cols);
        let mut rows = Vec::new();
        for r in result {
            let values = r.map_err(map_error)?.unwrap();
            rows.push(
                values
                    .into_iter()
                    .zip(&cols)
                    .map(|(v, c)| read(v, c.column_type(), c.character_set() == 63))
                    .collect(),
            );
        }
        Ok(Rows { columns, rows })
    }

    fn begin(&mut self) -> Result<(), SqlError> {
        self.conn.query_drop("START TRANSACTION").map_err(map_error)?;
        self.in_tx = true;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), SqlError> {
        self.in_tx = false;
        self.conn.query_drop("COMMIT").map_err(map_error)
    }

    fn rollback(&mut self) -> Result<(), SqlError> {
        self.in_tx = false;
        self.conn.query_drop("ROLLBACK").map_err(map_error)
    }

    fn in_transaction(&mut self) -> bool {
        self.in_tx
    }

    fn describe(&mut self, sql: &str) -> Result<(Vec<Column>, Vec<Column>), SqlError> {
        let stmt = self.conn.prep(sql).map_err(map_error)?;
        // MySQL reports no parameter types (R44): unknown, not guessed.
        Ok((super::untyped_params(stmt.num_params() as usize), columns_of(&stmt.columns())))
    }

    /// The table's primary key; a table without one has no row to name.
    fn row_key(&mut self, table: &str) -> Result<Vec<(String, String)>, SqlError> {
        let cols = self.primary_key(table)?;
        if cols.is_empty() {
            return Err(SqlError::new(
                code::NOT_SUPPORTED,
                format!("WHERE CURRENT OF needs a primary key on {table}, and it has none"),
            ));
        }
        Ok(cols.into_iter().map(|c| (format!("`{c}`"), format!("`{c}`"))).collect())
    }
}

impl MySqlBackend {
    /// The primary-key columns of `table`, in key order — what `WHERE
    /// CURRENT OF` names a row by (R26). Empty when it has none.
    pub fn primary_key(&mut self, table: &str) -> Result<Vec<String>, SqlError> {
        let (schema, name) = match table.rsplit_once('.') {
            Some((s, n)) => (Some(unquote(s)), unquote(n)),
            None => (None, unquote(table)),
        };
        let sql = "SELECT COLUMN_NAME FROM information_schema.KEY_COLUMN_USAGE \
                   WHERE TABLE_SCHEMA = COALESCE(?, DATABASE()) AND TABLE_NAME = ? AND CONSTRAINT_NAME = 'PRIMARY' \
                   ORDER BY ORDINAL_POSITION";
        let rows = self.query(sql, &[schema.map(SqlValue::Text).unwrap_or(SqlValue::Null), SqlValue::Text(name)])?;
        Ok(rows.rows.into_iter().filter_map(|r| r.into_iter().next()).map(|v| v.display()).collect())
    }
}

fn unquote(s: &str) -> String {
    s.trim().trim_matches('`').trim_matches('"').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MySQL's coarse states, refined by the error number.
    #[test]
    fn mysql_error_refinement() {
        assert_eq!(refine(1062, "23000"), "23505", "duplicate key");
        assert_eq!(refine(1452, "23000"), "23503", "foreign key");
        assert_eq!(refine(1048, "23000"), "23502", "NULL into NOT NULL");
        assert_eq!(refine(1146, "42S02"), "42P01", "no such table");
        assert_eq!(refine(1054, "42S22"), "42703", "no such column");
        assert_eq!(refine(1064, "42000"), "42601", "syntax error");
        assert_eq!(refine(1406, "22001"), "22001");
        assert_eq!(refine(1264, "22003"), "22003");
        assert_eq!(refine(1213, "40001"), "40001");
        assert_eq!(refine(1045, "28000"), "28000");
        assert_eq!(refine(9999, "45000"), "45000", "an unknown number keeps MySQL's state");
        assert_eq!(refine(9999, ""), code::GENERAL);
    }

    /// Values both ways: an exact decimal, binary bytes, dates and times.
    #[test]
    fn mysql_values() {
        assert_eq!(param(&SqlValue::Decimal { mantissa: -12345, scale: 2 }), mysql::Value::Bytes(b"-123.45".to_vec()));
        assert_eq!(
            read(mysql::Value::Bytes(b"-123.45".to_vec()), ColumnType::MYSQL_TYPE_NEWDECIMAL, false),
            SqlValue::Decimal { mantissa: -12345, scale: 2 }
        );
        assert_eq!(read(mysql::Value::Bytes(b"42".to_vec()), ColumnType::MYSQL_TYPE_NEWDECIMAL, false), SqlValue::Int(42));
        assert_eq!(read(mysql::Value::Bytes(vec![0, 1]), ColumnType::MYSQL_TYPE_BLOB, true), SqlValue::Bytes(vec![0, 1]));
        assert_eq!(read(mysql::Value::Bytes(b"ANA".to_vec()), ColumnType::MYSQL_TYPE_VAR_STRING, false), SqlValue::Text("ANA".into()));
        assert_eq!(read(mysql::Value::Date(2026, 3, 1, 0, 0, 0, 0), ColumnType::MYSQL_TYPE_DATE, false), SqlValue::Text("2026-03-01".into()));
        assert_eq!(
            read(mysql::Value::Date(2026, 3, 1, 13, 1, 36, 500_000), ColumnType::MYSQL_TYPE_DATETIME, false),
            SqlValue::Text("2026-03-01 13:01:36.5".into())
        );
        assert_eq!(read(mysql::Value::Time(true, 1, 2, 3, 4, 0), ColumnType::MYSQL_TYPE_TIME, false), SqlValue::Text("-26:03:04".into()));
        assert_eq!(read(mysql::Value::UInt(u64::MAX), ColumnType::MYSQL_TYPE_LONGLONG, false), SqlValue::Decimal { mantissa: u64::MAX as i128, scale: 0 });
        assert_eq!(type_name(ColumnType::MYSQL_TYPE_NEWDECIMAL, 11, 2, false, false), "DECIMAL(9,2)");
        assert_eq!(type_name(ColumnType::MYSQL_TYPE_NEWDECIMAL, 5, 0, false, true), "DECIMAL(5,0)");
        assert_eq!(type_name(ColumnType::MYSQL_TYPE_LONGLONG, 20, 0, false, false), "INTEGER");
        assert_eq!(type_name(ColumnType::MYSQL_TYPE_BLOB, 65535, 0, false, false), "TEXT");
        assert_eq!(type_name(ColumnType::MYSQL_TYPE_BLOB, 65535, 0, true, false), "BLOB");
        assert!(commits_implicitly("  create table t (a int)") && commits_implicitly("DROP TABLE t"));
        assert!(!commits_implicitly("INSERT INTO t VALUES (1)"));
    }
}
