// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Embedded SQL at run time (spec 087).
//!
//! A typed layer of its own: the `COBOL::"…-DB"` built-ins and the
//! `SqlDatabase` control keep the text-only bridge in [`crate::db_runtime`];
//! only the way a connection string is opened is shared
//! ([`crate::db_connect`]).

pub mod backend;
pub mod rewrite;
pub mod session;
pub mod state;
pub mod value;

pub use state::{code, sqlcode, SqlError};
pub use value::SqlValue;

/// The connections of one run unit (R36, R37): every program and form of the
/// application shares them, so a connection the main form opened is current
/// in a form it opens.
#[derive(Default)]
pub struct SqlRunUnit {
    pub sessions: Vec<session::Session>,
    /// Index of the current connection.
    pub current: Option<usize>,
}

impl SqlRunUnit {
    pub fn find(&self, name: &str) -> Option<usize> {
        let n = name.trim().to_ascii_uppercase();
        self.sessions.iter().position(|s| s.name == n)
    }

    /// The current connection, or `08003` (R35; the default SQL connection
    /// is tried by the caller before this).
    pub fn current_mut(&mut self) -> Result<&mut session::Session, SqlError> {
        match self.current {
            Some(i) if i < self.sessions.len() => Ok(&mut self.sessions[i]),
            _ => Err(SqlError::new(code::NO_CONNECTION, "no SQL connection is current — CONNECT TO one first")),
        }
    }

    /// Close connection `i`, rolling back its open work (R36).
    pub fn close(&mut self, i: usize) {
        if i >= self.sessions.len() {
            return;
        }
        let mut s = self.sessions.remove(i);
        let _ = s.rollback();
        self.current = match self.current {
            Some(c) if c == i => None,
            Some(c) if c > i => Some(c - 1),
            other => other,
        };
    }

    pub fn close_all(&mut self) {
        while !self.sessions.is_empty() {
            self.close(0);
        }
    }
}

/// R34: whether a `CONNECT TO` target is a connection string, not the name of
/// one of the project's SQL connections. Only clearly-shaped strings count, so
/// a mistyped name is an error (`08001`) instead of a new empty database file.
pub fn is_connection_string(target: &str) -> bool {
    let t = target.trim();
    let lower = t.to_ascii_lowercase();
    lower.starts_with("sqlite:")
        || lower.starts_with(":memory:")
        || lower.starts_with("postgres://")
        || lower.starts_with("postgresql://")
        || lower.starts_with("mysql://")
        || t.contains('/')
        || t.contains('\\')
        || [".db", ".sqlite", ".sqlite3", ".db3"].iter().any(|e| lower.ends_with(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_clearly_shaped_targets_are_connection_strings() {
        for yes in ["sqlite:app.db", ":memory:", "postgres://h/db", "mysql://h/db", "data/x", "C:\\x", "sales.DB", "a.sqlite3"] {
            assert!(is_connection_string(yes), "{yes}");
        }
        for no in ["SALES", "sales", "SECOND", "my-db"] {
            assert!(!is_connection_string(no), "{no}");
        }
    }

    #[cfg(feature = "sql")]
    mod sqlite {
        use super::super::backend::{self, BackendKind};
        use super::super::session::Session;
        use super::super::value::SqlValue;

        fn mem() -> Session {
            Session::new("T", backend::open(":memory:").unwrap())
        }

        /// Real SQLite errors arrive as standard SQLSTATEs.
        #[test]
        fn sqlite_errors_map_to_standard_sqlstates() {
            let mut s = mem();
            s.execute("CREATE TABLE T (ID INTEGER PRIMARY KEY, NAME TEXT NOT NULL)", &[], true).unwrap();
            s.execute("INSERT INTO T VALUES (1, 'a')", &[], true).unwrap();
            let dup = s.execute("INSERT INTO T VALUES (1, 'b')", &[], true).unwrap_err();
            let syntax = s.execute("SELEC 1", &[], true).unwrap_err();
            let table = s.query("SELECT * FROM NOPE", &[]).unwrap_err();
            let column = s.query("SELECT NOPE FROM T", &[]).unwrap_err();
            let notnull = s.execute("INSERT INTO T VALUES (2, NULL)", &[], true).unwrap_err();
            let got = [&dup, &syntax, &table, &column, &notnull].map(|e| e.sqlstate.as_str());
            println!("SQLite errors → SQLSTATE: duplicate {}, syntax {}, no table {}, no column {}, not null {}", got[0], got[1], got[2], got[3], got[4]);
            for e in [&dup, &syntax, &table, &column, &notnull] {
                println!("  {} native {} — {}", e.sqlstate, e.native, e.message);
            }
            assert_eq!(got, ["23505", "42601", "42P01", "42703", "23502"]);
            assert!(dup.message.to_ascii_lowercase().contains("unique"), "the database's own words are kept: {}", dup.message);
            assert_eq!(syntax.message, "near \"SELEC\": syntax error", "only the message, not the SQL text");
        }

        /// A value is bound, never spliced: text that would break out of a
        /// literal matches only itself. NULL survives the round trip.
        #[test]
        fn values_are_bound_and_null_survives() {
            let mut s = mem();
            s.execute("CREATE TABLE T (NAME TEXT, NOTE TEXT)", &[], true).unwrap();
            for name in ["x' OR '1'='1", "plain"] {
                s.execute("INSERT INTO T VALUES (?, ?)", &[SqlValue::Text(name.into()), SqlValue::Null], true).unwrap();
            }
            let r = s.query("SELECT NAME, NOTE FROM T WHERE NAME = ?", &[SqlValue::Text("x' OR '1'='1".into())]).unwrap();
            assert_eq!(r.rows.len(), 1, "the hostile text matched only itself");
            assert_eq!(r.rows[0], vec![SqlValue::Text("x' OR '1'='1".into()), SqlValue::Null]);
            assert_eq!(r.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["NAME", "NOTE"]);
            assert_eq!(r.columns[0].decl_type.as_deref(), Some("TEXT"));
        }

        /// Up to 15 significant digits a decimal binds as REAL (exact in a
        /// double); beyond, as TEXT, so no digit is lost.
        #[test]
        fn decimals_bind_by_their_precision() {
            let mut s = mem();
            s.execute("CREATE TABLE T (V)", &[], true).unwrap();
            let small = SqlValue::Decimal { mantissa: -123456789012345, scale: 2 };
            let big = SqlValue::Decimal { mantissa: 1234567890123456789, scale: 2 };
            s.execute("INSERT INTO T VALUES (?)", &[small], true).unwrap();
            s.execute("INSERT INTO T VALUES (?)", &[big], true).unwrap();
            let r = s.query("SELECT V, typeof(V) FROM T", &[]).unwrap();
            println!("decimal binding: {:?}", r.rows);
            assert_eq!(r.rows[0], vec![SqlValue::Float(-1234567890123.45), SqlValue::Text("real".into())]);
            assert_eq!(r.rows[1], vec![SqlValue::Text("12345678901234567.89".into()), SqlValue::Text("text".into())]);
        }

        /// A reading session holds no lock: another connection can write
        /// while it is open, between its reads.
        #[test]
        fn a_reading_session_holds_no_lock() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("lock.db");
            let target = format!("sqlite:{}", path.display());
            let mut a = Session::new("A", backend::open(&target).unwrap());
            a.execute("CREATE TABLE T (N INTEGER)", &[], true).unwrap();
            a.commit().unwrap();
            let r = a.query("SELECT COUNT(*) FROM T", &[]).unwrap();
            assert_eq!(r.rows[0][0], SqlValue::Int(0));
            assert!(!a.in_unit(), "a read opened no unit of work");
            let mut b = Session::new("B", backend::open(&target).unwrap());
            b.execute("INSERT INTO T VALUES (1)", &[], true).unwrap();
            b.commit().unwrap();
            let r = a.query("SELECT COUNT(*) FROM T", &[]).unwrap();
            assert_eq!(r.rows[0][0], SqlValue::Int(1), "the reader sees the other connection's commit");
            assert_eq!(a.kind(), BackendKind::Sqlite);
        }

        /// COMMIT keeps a unit of work's changes, ROLLBACK discards them.
        #[test]
        fn commit_keeps_and_rollback_discards() {
            let dir = tempfile::tempdir().unwrap();
            let target = format!("sqlite:{}", dir.path().join("uow.db").display());
            let mut s = Session::new("S", backend::open(&target).unwrap());
            s.execute("CREATE TABLE T (N INTEGER)", &[], true).unwrap();
            s.commit().unwrap();
            s.execute("INSERT INTO T VALUES (1)", &[], true).unwrap();
            assert!(s.in_unit());
            s.rollback().unwrap();
            s.execute("INSERT INTO T VALUES (2)", &[], true).unwrap();
            s.commit().unwrap();
            let r = s.query("SELECT N FROM T", &[]).unwrap();
            assert_eq!(r.rows, vec![vec![SqlValue::Int(2)]]);
            // What a second connection sees is what was committed.
            let mut other = Session::new("O", backend::open(&target).unwrap());
            assert_eq!(other.query("SELECT COUNT(*) FROM T", &[]).unwrap().rows[0][0], SqlValue::Int(1));
        }
    }
}
