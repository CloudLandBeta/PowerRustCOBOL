// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Embedded SQL in the AST (spec 087).
//!
//! An `EXEC SQL … END-EXEC` block becomes one [`ExecSql`] statement. Its SQL
//! text is kept as written, split into [`SqlPart`]s around the host variables
//! (which become bound parameters, R13) and around `WHERE CURRENT OF`, which
//! the runtime resolves per database. Cursors are declarations, collected on
//! the program that declares them ([`SqlCursor`]).
//!
//! 🔴 `Program` and `Stmt` are bincode-serialized by position: every type here
//! is new, and the two fields that reach them were appended last.

use serde::{Deserialize, Serialize};

use crate::Span;

/// A COBOL name as written in a block, with its qualification, innermost
/// first (`:CITY OF CUSTOMER` → `CITY`, `[CUSTOMER]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SqlHostName {
    pub name: String,
    pub quals: Vec<String>,
    /// The other reading of a period-qualified name (`:A.B` is `A OF B` or
    /// `B OF A`). Check decides which one exists; both existing is an error.
    pub swapped: Option<(String, Vec<String>)>,
}

/// One host-variable reference, with its indicator (R6, R9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SqlHostRef {
    pub var: SqlHostName,
    pub indicator: Option<SqlHostName>,
    /// The source line of the reference.
    pub line: u32,
}

/// A piece of an SQL statement's text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SqlPart {
    /// SQL as written (line comments removed, R4).
    Text(String),
    /// The n-th input host variable of the statement (0-based), sent as a
    /// bound parameter.
    Input(usize),
    /// `WHERE CURRENT OF cursor`: the row the cursor last fetched (R26).
    CurrentOf(String),
}

/// An SQL statement's text and the host variables it reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SqlText {
    pub parts: Vec<SqlPart>,
    pub inputs: Vec<SqlHostRef>,
}

/// A value a statement names: a literal, a host variable, or a bare word
/// (`CONNECT TO SALES`, `SET CONNECTION SECOND`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SqlValue {
    Literal(String),
    Host(SqlHostRef),
    Name(String),
}

/// Where a statement's parameters come from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SqlUsing {
    None,
    Hosts(Vec<SqlHostRef>),
    /// `USING DESCRIPTOR sqlda`.
    Descriptor(SqlHostName),
}

/// Where a fetched row goes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SqlInto {
    Hosts(Vec<SqlHostRef>),
    /// `USING DESCRIPTOR sqlda`.
    Descriptor(SqlHostName),
}

/// What `DISCONNECT` closes (R36).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SqlDisconnect {
    Named(SqlValue),
    Current,
    All,
}

/// What an `EXEC SQL` statement does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SqlKind {
    /// Compile-time only: `BEGIN/END DECLARE SECTION`, `DECLARE … TABLE`,
    /// `DECLARE … CURSOR` (recorded on the program), `WHENEVER`.
    Declarative,
    /// `INSERT`, `UPDATE`, `DELETE`, data definition, and any other statement
    /// that returns no rows (R23).
    Execute(SqlText),
    /// `SELECT … INTO :host …` — one row (R23, R24).
    SelectInto { text: SqlText, into: Vec<SqlHostRef> },
    Open { cursor: String, using: SqlUsing },
    Fetch { cursor: String, into: SqlInto },
    Close { cursor: String },
    Commit,
    Rollback,
    /// `CONNECT TO target [AS alias] [USER u [USING p]]` (R34).
    Connect {
        target: SqlValue,
        alias: Option<String>,
        user: Option<SqlValue>,
        password: Option<SqlValue>,
    },
    SetConnection(SqlValue),
    Disconnect(SqlDisconnect),
    /// `PREPARE name FROM :host | 'literal'` (R42).
    Prepare { name: String, from: SqlValue },
    /// `EXECUTE name [USING …]`.
    ExecutePrepared { name: String, using: SqlUsing },
    /// `EXECUTE IMMEDIATE :host | 'literal'`.
    ExecuteImmediate(SqlValue),
    /// `DESCRIBE [INPUT] name INTO sqlda` (R44).
    Describe { name: String, sqlda: SqlHostName, input: bool },
}

/// `WHENEVER` targets in force for one statement (R20, R21): `None` is
/// `CONTINUE`, `Some(label)` is `GO TO label`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SqlWhenever {
    pub sqlerror: Option<String>,
    pub sqlwarning: Option<String>,
    pub not_found: Option<String>,
}

/// One `EXEC SQL … END-EXEC` statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecSql {
    pub kind: SqlKind,
    /// The `WHENEVER` declarations that precede it in the source (R21).
    pub whenever: SqlWhenever,
    /// The PROGRAM-ID of the program it is written in — cursors are named
    /// within their program (R28).
    pub owner: String,
    pub span: Span,
    /// The line of `END-EXEC`.
    pub last_line: u32,
}

/// What a cursor reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SqlCursorQuery {
    /// `DECLARE c CURSOR FOR SELECT …`.
    Static(SqlText),
    /// `DECLARE c CURSOR FOR statement-name` (R43).
    Prepared(String),
}

/// A cursor declaration (R25, R28).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SqlCursor {
    pub name: String,
    /// The PROGRAM-ID of the program that declares it.
    pub owner: String,
    pub query: SqlCursorQuery,
    pub with_hold: bool,
    pub for_update: bool,
    /// Some statement updates or deletes `WHERE CURRENT OF` it.
    pub positioned: bool,
    /// Declared in the DATA DIVISION, so contained programs see it too.
    pub in_data_division: bool,
    pub line: u32,
}
