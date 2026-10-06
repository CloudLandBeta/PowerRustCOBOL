// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! PostgreSQL for embedded SQL (spec 087).
//!
//! * Parameters travel in **text** format, so the server parses a decimal or
//!   a date exactly as it would a literal; their `Debug` never shows a value.
//! * Results arrive in PostgreSQL's binary format and are decoded here:
//!   integers, floats, booleans, NUMERIC (exact), text, bytea, dates, times,
//!   timestamps and UUIDs. Any other type is `0A000`, never a wrong value.
//! * Every statement inside a unit of work runs under a savepoint, so one
//!   failing statement does not poison the unit (R22) — PostgreSQL otherwise
//!   refuses everything until a ROLLBACK.
//! * The server's own SQLSTATE and message are kept.

use std::error::Error as StdError;

use bytes::BytesMut;
use postgres::types::{Format, FromSql, IsNull, ToSql, Type};

use super::super::state::{code, SqlError};
use super::super::value::{decimal_text, float_text, SqlValue};
use super::{Backend, BackendKind, Column, Rows};

type BoxError = Box<dyn StdError + Sync + Send>;

pub struct PostgresBackend {
    client: postgres::Client,
    in_tx: bool,
}

impl PostgresBackend {
    /// From a `postgres://` connection string.
    pub fn open_url(url: &str) -> Result<Self, SqlError> {
        let client = postgres::Client::connect(url.trim(), postgres::NoTls).map_err(map_connect_error)?;
        Ok(Self { client, in_tx: false })
    }

    /// From its parts, field by field: the password never enters a URL.
    pub fn open_fields(
        host: &str,
        port: Option<u16>,
        database: &str,
        user: Option<&str>,
        password: Option<&str>,
    ) -> Result<Self, SqlError> {
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
        let client = config.connect(postgres::NoTls).map_err(map_connect_error)?;
        Ok(Self { client, in_tx: false })
    }

    /// Run `f` under a savepoint when a unit of work is open (R22).
    fn guarded<T>(&mut self, f: impl FnOnce(&mut postgres::Client) -> Result<T, postgres::Error>) -> Result<T, SqlError> {
        if !self.in_tx {
            return f(&mut self.client).map_err(map_error);
        }
        self.client.batch_execute("SAVEPOINT esql_statement").map_err(map_error)?;
        match f(&mut self.client) {
            Ok(v) => {
                self.client.batch_execute("RELEASE SAVEPOINT esql_statement").map_err(map_error)?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.client.batch_execute("ROLLBACK TO SAVEPOINT esql_statement");
                Err(map_error(e))
            }
        }
    }
}

/// A parameter in text format: the server parses it as the type it expects.
pub struct TextParam(Option<String>);

impl std::fmt::Debug for TextParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // A bound value may be a password (R52).
        f.write_str("<param>")
    }
}

impl ToSql for TextParam {
    fn to_sql(&self, _ty: &Type, out: &mut BytesMut) -> Result<IsNull, BoxError> {
        match &self.0 {
            None => Ok(IsNull::Yes),
            Some(s) => {
                out.extend_from_slice(s.as_bytes());
                Ok(IsNull::No)
            }
        }
    }

    fn accepts(_ty: &Type) -> bool {
        true
    }

    fn encode_format(&self, _ty: &Type) -> Format {
        Format::Text
    }

    postgres::types::to_sql_checked!();
}

/// A value as PostgreSQL reads it in text format.
pub fn text_param(v: &SqlValue) -> TextParam {
    TextParam(match v {
        SqlValue::Null => None,
        SqlValue::Int(i) => Some(i.to_string()),
        SqlValue::Decimal { mantissa, scale } => Some(decimal_text(*mantissa, *scale)),
        SqlValue::Float(f) => Some(float_text(*f)),
        SqlValue::Text(s) => Some(s.clone()),
        SqlValue::Bytes(b) => Some(format!("\\x{}", b.iter().map(|x| format!("{x:02x}")).collect::<String>())),
        SqlValue::Bool(b) => Some(if *b { "true" } else { "false" }.into()),
    })
}

/// A result value's raw binary form, whatever its type.
struct Raw(Option<Vec<u8>>);

impl<'a> FromSql<'a> for Raw {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<Self, BoxError> {
        Ok(Raw(Some(raw.to_vec())))
    }

    fn from_sql_null(_ty: &Type) -> Result<Self, BoxError> {
        Ok(Raw(None))
    }

    fn accepts(_ty: &Type) -> bool {
        true
    }
}

fn be<const N: usize>(raw: &[u8]) -> Result<[u8; N], SqlError> {
    raw.get(..N)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| SqlError::new(code::GENERAL, "PostgreSQL sent a value shorter than its type"))
}

/// Decode one binary result value of type `ty`.
pub fn decode(ty: &Type, raw: &[u8]) -> Result<SqlValue, SqlError> {
    Ok(match *ty {
        Type::BOOL => SqlValue::Bool(raw.first().is_some_and(|b| *b != 0)),
        Type::INT2 => SqlValue::Int(i16::from_be_bytes(be(raw)?) as i64),
        Type::INT4 => SqlValue::Int(i32::from_be_bytes(be(raw)?) as i64),
        Type::OID => SqlValue::Int(u32::from_be_bytes(be(raw)?) as i64),
        Type::INT8 => SqlValue::Int(i64::from_be_bytes(be(raw)?)),
        Type::FLOAT4 => SqlValue::Float(f32::from_be_bytes(be(raw)?) as f64),
        Type::FLOAT8 => SqlValue::Float(f64::from_be_bytes(be(raw)?)),
        Type::NUMERIC => decode_numeric(raw)?,
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME | Type::UNKNOWN | Type::JSON | Type::XML => {
            SqlValue::Text(String::from_utf8_lossy(raw).into_owned())
        }
        // JSONB: a version byte, then the text.
        Type::JSONB => SqlValue::Text(String::from_utf8_lossy(raw.get(1..).unwrap_or_default()).into_owned()),
        Type::BYTEA => SqlValue::Bytes(raw.to_vec()),
        Type::DATE => SqlValue::Text(date_text(i32::from_be_bytes(be(raw)?))),
        Type::TIME => SqlValue::Text(time_text(i64::from_be_bytes(be(raw)?))),
        Type::TIMESTAMP => SqlValue::Text(timestamp_text(i64::from_be_bytes(be(raw)?))),
        Type::TIMESTAMPTZ => SqlValue::Text(format!("{}+00", timestamp_text(i64::from_be_bytes(be(raw)?)))),
        Type::UUID => {
            let b: [u8; 16] = be(raw)?;
            let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
            SqlValue::Text(format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..]))
        }
        ref other => {
            return Err(SqlError::new(
                code::NOT_SUPPORTED,
                format!("a PostgreSQL {} value cannot be read into a host variable; cast it in the query (::text)", other.name()),
            ))
        }
    })
}

/// NUMERIC's binary form: digit count, weight (of the first base-10000
/// digit), sign, display scale, then the base-10000 digits.
pub fn decode_numeric(raw: &[u8]) -> Result<SqlValue, SqlError> {
    let n = i16::from_be_bytes(be(raw)?) as usize;
    let weight = i16::from_be_bytes(be(&raw[2..])?) as i64;
    let sign = u16::from_be_bytes(be(&raw[4..])?);
    let dscale = u16::from_be_bytes(be(&raw[6..])?) as i64;
    match sign {
        0xC000 => return Ok(SqlValue::Float(f64::NAN)),
        0xD000 => return Ok(SqlValue::Float(f64::INFINITY)),
        0xF000 => return Ok(SqlValue::Float(f64::NEG_INFINITY)),
        _ => {}
    }
    let overflow = || SqlError::new(code::OUT_OF_RANGE, "a PostgreSQL NUMERIC value has more than 38 digits");
    let mut mantissa: i128 = 0;
    for i in 0..n {
        let d = i16::from_be_bytes(be(&raw[8 + 2 * i..])?) as i128;
        // This digit counts 10000^(weight - i); scaled by 10^dscale.
        let exp = 4 * (weight - i as i64) + dscale;
        let term = if exp >= 0 {
            10i128.checked_pow(exp as u32).and_then(|p| d.checked_mul(p)).ok_or_else(overflow)?
        } else {
            // Digits past the display scale are padding zeros.
            d / 10i128.checked_pow((-exp) as u32).unwrap_or(i128::MAX)
        };
        mantissa = mantissa.checked_add(term).ok_or_else(overflow)?;
    }
    if sign == 0x4000 {
        mantissa = -mantissa;
    }
    let scale = u8::try_from(dscale).map_err(|_| overflow())?;
    Ok(SqlValue::decimal(mantissa, scale))
}

/// Days since 2000-01-01 → `YYYY-MM-DD` (proleptic Gregorian).
pub fn date_text(days: i32) -> String {
    match days {
        i32::MAX => return "infinity".into(),
        i32::MIN => return "-infinity".into(),
        _ => {}
    }
    let (y, m, d) = civil_from_days(days as i64 + 10957);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Microseconds since midnight → `HH:MM:SS[.ffffff]`.
pub fn time_text(micros: i64) -> String {
    let secs = micros.div_euclid(1_000_000);
    let frac = micros.rem_euclid(1_000_000);
    let base = format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60);
    if frac == 0 {
        base
    } else {
        format!("{base}.{}", format!("{frac:06}").trim_end_matches('0'))
    }
}

/// Microseconds since 2000-01-01 00:00 → `YYYY-MM-DD HH:MM:SS[.ffffff]`.
pub fn timestamp_text(micros: i64) -> String {
    match micros {
        i64::MAX => return "infinity".into(),
        i64::MIN => return "-infinity".into(),
        _ => {}
    }
    let day = micros.div_euclid(86_400_000_000);
    let rest = micros.rem_euclid(86_400_000_000);
    let (y, m, d) = civil_from_days(day + 10957);
    format!("{y:04}-{m:02}-{d:02} {}", time_text(rest))
}

/// Days since 1970-01-01 → (year, month, day); Howard Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// A PostgreSQL error with the server's own SQLSTATE and message.
pub fn map_error(e: postgres::Error) -> SqlError {
    if let Some(db) = e.as_db_error() {
        return SqlError::new(db.code().code(), db.message());
    }
    if e.is_closed() {
        return SqlError::new("08006", "the connection to PostgreSQL was lost");
    }
    SqlError::new(code::GENERAL, with_causes(&e))
}

fn map_connect_error(e: postgres::Error) -> SqlError {
    if let Some(db) = e.as_db_error() {
        return SqlError::new(db.code().code(), db.message());
    }
    SqlError::new(code::CANNOT_CONNECT, with_causes(&e))
}

fn with_causes(e: &dyn StdError) -> String {
    let mut out = e.to_string();
    let mut cause = e.source();
    while let Some(c) = cause {
        let t = c.to_string();
        if !out.contains(&t) {
            out.push_str(": ");
            out.push_str(&t);
        }
        cause = c.source();
    }
    out
}

fn params_of(params: &[SqlValue]) -> Vec<TextParam> {
    params.iter().map(text_param).collect()
}

fn refs(p: &[TextParam]) -> Vec<&(dyn ToSql + Sync)> {
    p.iter().map(|x| x as &(dyn ToSql + Sync)).collect()
}

impl Backend for PostgresBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Postgres
    }

    fn execute(&mut self, sql: &str, params: &[SqlValue]) -> Result<u64, SqlError> {
        let p = params_of(params);
        self.guarded(|c| c.execute(sql, &refs(&p)))
    }

    fn query(&mut self, sql: &str, params: &[SqlValue]) -> Result<Rows, SqlError> {
        let p = params_of(params);
        let (rows, stmt) = self.guarded(|c| {
            let stmt = c.prepare(sql)?;
            let rows = c.query(&stmt, &refs(&p))?;
            Ok((rows, stmt))
        })?;
        let columns: Vec<Column> = stmt
            .columns()
            .iter()
            .map(|c| Column { name: c.name().to_string(), decl_type: Some(c.type_().name().to_string()), nullable: None })
            .collect();
        let mut out = Vec::with_capacity(rows.len());
        for r in &rows {
            let mut row = Vec::with_capacity(columns.len());
            for (i, col) in r.columns().iter().enumerate() {
                let raw: Raw = r.try_get(i).map_err(map_error)?;
                row.push(match raw.0 {
                    None => SqlValue::Null,
                    Some(bytes) => decode(col.type_(), &bytes)?,
                });
            }
            out.push(row);
        }
        Ok(Rows { columns, rows: out })
    }

    fn begin(&mut self) -> Result<(), SqlError> {
        self.client.batch_execute("BEGIN").map_err(map_error)?;
        self.in_tx = true;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), SqlError> {
        self.in_tx = false;
        self.client.batch_execute("COMMIT").map_err(map_error)
    }

    fn rollback(&mut self) -> Result<(), SqlError> {
        self.in_tx = false;
        self.client.batch_execute("ROLLBACK").map_err(map_error)
    }

    fn in_transaction(&mut self) -> bool {
        self.in_tx && !self.client.is_closed()
    }

    fn describe(&mut self, sql: &str) -> Result<(Vec<Column>, Vec<Column>), SqlError> {
        let stmt = self.guarded(|c| c.prepare(sql))?;
        let cols = stmt
            .columns()
            .iter()
            .map(|c| Column { name: c.name().to_string(), decl_type: Some(c.type_().name().to_string()), nullable: None })
            .collect();
        // PostgreSQL infers each parameter's type, and says so (R44).
        let params = stmt
            .params()
            .iter()
            .enumerate()
            .map(|(i, t)| Column { name: format!("?{}", i + 1), decl_type: Some(t.name().to_string()), nullable: None })
            .collect();
        Ok((params, cols))
    }

    /// The row's physical address. It names the row the cursor read for the
    /// rest of the unit of work — a row updated since moves, and is then not
    /// found (no data), never a different row.
    fn row_key(&mut self, _table: &str) -> Result<Vec<(String, String)>, SqlError> {
        Ok(vec![("ctid::text".into(), "ctid".into())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numeric(weight: i16, sign: u16, dscale: u16, digits: &[i16]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend((digits.len() as i16).to_be_bytes());
        v.extend(weight.to_be_bytes());
        v.extend(sign.to_be_bytes());
        v.extend(dscale.to_be_bytes());
        for d in digits {
            v.extend(d.to_be_bytes());
        }
        v
    }

    /// NUMERIC decodes exactly: signs, a negative weight (a value below 1),
    /// the display scale, trailing padding, NaN and infinity.
    #[test]
    fn numeric_decoding() {
        let dec = |raw: Vec<u8>| decode_numeric(&raw).unwrap();
        // 12345.67 = [1][2345].[6700], weight 1, scale 2.
        assert_eq!(dec(numeric(1, 0, 2, &[1, 2345, 6700])), SqlValue::Decimal { mantissa: 1234567, scale: 2 });
        assert_eq!(dec(numeric(1, 0x4000, 2, &[1, 2345, 6700])), SqlValue::Decimal { mantissa: -1234567, scale: 2 });
        // 0.0005 = .[0005], weight -1, scale 4.
        assert_eq!(dec(numeric(-1, 0, 4, &[5])), SqlValue::Decimal { mantissa: 5, scale: 4 });
        // 0.00000012 = weight -2, digits [12] → 0.0000 0012, scale 8.
        assert_eq!(dec(numeric(-2, 0, 8, &[12])), SqlValue::Decimal { mantissa: 12, scale: 8 });
        // 100 with scale 0 → an integer; 0 has no digits.
        assert_eq!(dec(numeric(0, 0, 0, &[100])), SqlValue::Int(100));
        assert_eq!(dec(numeric(0, 0, 2, &[])), SqlValue::Decimal { mantissa: 0, scale: 2 });
        // 10000 = [1] weight 1.
        assert_eq!(dec(numeric(1, 0, 0, &[1])), SqlValue::Int(10000));
        assert!(matches!(dec(numeric(0, 0xC000, 0, &[])), SqlValue::Float(f) if f.is_nan()));
        assert_eq!(dec(numeric(0, 0xD000, 0, &[])), SqlValue::Float(f64::INFINITY));
        // More than 38 digits is out of range, never a wrong value.
        assert_eq!(decode_numeric(&numeric(10, 0, 0, &[9999; 11])).unwrap_err().sqlstate, code::OUT_OF_RANGE);
    }

    /// Dates, times and timestamps from their epoch offsets.
    #[test]
    fn date_and_time_decoding() {
        assert_eq!(date_text(0), "2000-01-01");
        assert_eq!(date_text(-1), "1999-12-31");
        assert_eq!(date_text(9_556), "2026-03-01");
        assert_eq!(date_text(i32::MAX), "infinity");
        assert_eq!(time_text(0), "00:00:00");
        assert_eq!(time_text(45_296_500_000), "12:34:56.5");
        assert_eq!(timestamp_text(0), "2000-01-01 00:00:00");
        assert_eq!(timestamp_text(-1), "1999-12-31 23:59:59.999999");
        assert_eq!(timestamp_text(825_685_296_000_001), "2026-03-01 13:01:36.000001");
        let uuid: Vec<u8> = (0u8..16).collect();
        assert_eq!(decode(&Type::UUID, &uuid).unwrap(), SqlValue::Text("00010203-0405-0607-0809-0a0b0c0d0e0f".into()));
        assert_eq!(decode(&Type::INT8, &(-7i64).to_be_bytes()).unwrap(), SqlValue::Int(-7));
        assert_eq!(decode(&Type::POINT, &[0; 16]).unwrap_err().sqlstate, code::NOT_SUPPORTED);
    }

    /// A parameter's `Debug` never shows its value; values travel as text.
    #[test]
    fn redacted_parameter_debug() {
        let p = text_param(&SqlValue::Text("s3cret".into()));
        assert_eq!(format!("{p:?}"), "<param>");
        let mut out = BytesMut::new();
        assert!(matches!(p.to_sql(&Type::TEXT, &mut out).unwrap(), IsNull::No));
        assert_eq!(&out[..], b"s3cret");
        assert!(matches!(p.encode_format(&Type::NUMERIC), Format::Text));
        assert_eq!(text_param(&SqlValue::Decimal { mantissa: -12345, scale: 2 }).0.as_deref(), Some("-123.45"));
        assert_eq!(text_param(&SqlValue::Bytes(vec![1, 0xAB])).0.as_deref(), Some("\\x01ab"));
        assert!(text_param(&SqlValue::Null).0.is_none());
    }
}
