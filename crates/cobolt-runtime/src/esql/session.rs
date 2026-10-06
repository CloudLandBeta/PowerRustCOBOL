// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! One connection and its unit of work (spec 087 R29, R36).
//!
//! A unit of work starts with the first statement after a connect or after
//! the previous unit ended — on SQLite only before a statement that WRITES, so
//! a program that only reads never holds the database's write lock. It ends
//! with `EXEC SQL COMMIT` or `ROLLBACK`; the COBOL verbs of the same names
//! govern INDEXED files only and never reach here.

use super::backend::{Backend, BackendKind, Rows};
use super::state::SqlError;
use super::value::SqlValue;

pub struct Session {
    /// The name the program knows it by (`AS alias`, or the SQL connection's
    /// name, upper-cased).
    pub name: String,
    backend: Box<dyn Backend>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the connection target: it may carry credentials (R52).
        f.debug_struct("Session").field("name", &self.name).field("backend", &self.backend.kind()).finish()
    }
}

impl Session {
    pub fn new(name: impl Into<String>, backend: Box<dyn Backend>) -> Self {
        Self { name: name.into().to_ascii_uppercase(), backend }
    }

    pub fn kind(&self) -> BackendKind {
        self.backend.kind()
    }

    fn ensure_unit(&mut self, writing: bool) -> Result<(), SqlError> {
        let needed = writing || self.backend.kind() != BackendKind::Sqlite;
        if needed && !self.backend.in_transaction() {
            self.backend.begin()?;
        }
        Ok(())
    }

    /// Run a statement that returns no rows; `writing` says whether it changes
    /// the database (INSERT, UPDATE, DELETE, DDL …).
    pub fn execute(&mut self, sql: &str, params: &[SqlValue], writing: bool) -> Result<u64, SqlError> {
        self.ensure_unit(writing)?;
        self.backend.execute(sql, params)
    }

    pub fn query(&mut self, sql: &str, params: &[SqlValue]) -> Result<Rows, SqlError> {
        self.ensure_unit(false)?;
        self.backend.query(sql, params)
    }

    pub fn describe(&mut self, sql: &str) -> Result<(usize, Vec<super::backend::Column>), SqlError> {
        self.backend.describe(sql)
    }

    /// End the unit of work, keeping its changes. Nothing open is no error.
    pub fn commit(&mut self) -> Result<(), SqlError> {
        if self.backend.in_transaction() {
            self.backend.commit()?;
        }
        Ok(())
    }

    /// End the unit of work, discarding its changes.
    pub fn rollback(&mut self) -> Result<(), SqlError> {
        if self.backend.in_transaction() {
            self.backend.rollback()?;
        }
        Ok(())
    }

    pub fn in_unit(&mut self) -> bool {
        self.backend.in_transaction()
    }
}
