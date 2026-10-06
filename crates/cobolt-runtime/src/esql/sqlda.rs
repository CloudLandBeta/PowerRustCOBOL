// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the SQL descriptor area says about a column or parameter (spec 087
//! R41, R44). The layout is this project's own (`EXEC SQL INCLUDE SQLDA`):
//! each entry carries a type code, the database's type name, length,
//! precision, scale and nullability. What a database does not report is said
//! to be unknown (`-1`, type code 0) — never guessed.

/// `SQLDA-TYPE` codes.
pub mod type_code {
    pub const UNKNOWN: i64 = 0;
    pub const INTEGER: i64 = 1;
    pub const DECIMAL: i64 = 2;
    pub const FLOAT: i64 = 3;
    pub const CHARACTER: i64 = 4;
    pub const BINARY: i64 = 5;
    pub const DATE: i64 = 6;
    pub const TIME: i64 = 7;
    pub const TIMESTAMP: i64 = 8;
    pub const BOOLEAN: i64 = 9;
}

/// One entry's description.
#[derive(Debug, Clone, PartialEq)]
pub struct Described {
    pub name: String,
    pub code: i64,
    pub type_name: String,
    pub length: i64,
    pub precision: i64,
    pub scale: i64,
    /// 1 nullable, 0 not, -1 unknown.
    pub nullable: i64,
}

impl Described {
    pub fn unknown(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            code: type_code::UNKNOWN,
            type_name: "UNKNOWN".into(),
            length: -1,
            precision: -1,
            scale: -1,
            nullable: -1,
        }
    }
}

/// Describe a column from the type its database declared (`VARCHAR(30)`,
/// `NUMERIC(9,2)`, `INTEGER` …). `None` — an expression with no declared type
/// — is unknown.
pub fn describe_declared(name: &str, decl: Option<&str>, nullable: Option<bool>) -> Described {
    let mut d = Described::unknown(name);
    d.nullable = match nullable {
        Some(true) => 1,
        Some(false) => 0,
        None => -1,
    };
    let Some(decl) = decl.map(str::trim).filter(|t| !t.is_empty()) else { return d };
    d.type_name = decl.to_string();
    let upper = decl.to_ascii_uppercase();
    let base = upper.split('(').next().unwrap_or("").trim().to_string();
    let args: Vec<i64> = upper
        .split_once('(')
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(inner, _)| inner.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_default();
    use type_code::*;
    d.code = if base.contains("INT") {
        INTEGER
    } else if base.contains("DEC") || base.contains("NUM") || base == "MONEY" {
        DECIMAL
    } else if base.contains("REAL") || base.contains("FLOA") || base.contains("DOUB") {
        FLOAT
    } else if base.contains("CHAR") || base.contains("TEXT") || base.contains("CLOB") || base.contains("STRING") {
        CHARACTER
    } else if base.contains("BLOB") || base.contains("BINARY") || base.contains("BYTEA") {
        BINARY
    } else if base.contains("TIMESTAMP") || base.contains("DATETIME") {
        TIMESTAMP
    } else if base.contains("DATE") {
        DATE
    } else if base.contains("TIME") {
        TIME
    } else if base.contains("BOOL") {
        BOOLEAN
    } else {
        UNKNOWN
    };
    match d.code {
        DECIMAL => {
            d.precision = args.first().copied().unwrap_or(-1);
            d.scale = args.get(1).copied().unwrap_or(if args.is_empty() { -1 } else { 0 });
        }
        CHARACTER | BINARY => d.length = args.first().copied().unwrap_or(-1),
        _ => {}
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_types_describe_what_they_say_and_no_more() {
        let n = describe_declared("PRICE", Some("NUMERIC(9,2)"), None);
        assert_eq!((n.code, n.precision, n.scale, n.nullable), (type_code::DECIMAL, 9, 2, -1));
        let v = describe_declared("NAME", Some("VARCHAR(30)"), Some(false));
        assert_eq!((v.code, v.length, v.nullable), (type_code::CHARACTER, 30, 0));
        let t = describe_declared("NOTE", Some("TEXT"), None);
        assert_eq!((t.code, t.length), (type_code::CHARACTER, -1), "an unbounded text has no length");
        assert_eq!(describe_declared("X", None, None), Described::unknown("X"));
        assert_eq!(describe_declared("D", Some("DATE"), None).code, type_code::DATE);
        assert_eq!(describe_declared("I", Some("INTEGER"), None).code, type_code::INTEGER);
    }
}
