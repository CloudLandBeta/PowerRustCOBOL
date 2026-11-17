// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The `EXEC SQL` statements of a desktop module** (ABI A41, DESKTOP-WASM-001 T41): the custom
//! section `prsql`, and the values that cross the one `prn` import, `sql_exec`.
//!
//! The runner runs every statement with the interpreter's own SQL code. It keeps a small
//! interpreter for each program that has `EXEC SQL`, built from the declarations this section
//! carries, and joins them all to one run unit: the connections, cursors and prepared statements
//! of the whole run. The module keeps the host variables. Before a statement it hands the runner
//! the value of each **cell** the statement may read or write (each elementary item a host
//! variable, an indicator or a status item stands for). The runner sets those values in the
//! program's interpreter, runs the statement there, and answers which `WHENEVER` condition the
//! statement ended in and the cells whose value it changed. The module stores those.
//!
//! A value crosses as the interpreter holds it: a number as its decimal mantissa and scale (the
//! type both engines use), a floating item as a float, and anything else as its bytes. So the
//! interpreter's own rules decide what each item sends to the database and how each column goes
//! into its item, on both paths.
//!
//! The section is JSON, versioned by `format`; a field is never removed or given a new meaning, and
//! a field a later release adds has a default, so every runner reads every earlier module.

use serde::{Deserialize, Serialize};

/// The custom section's name.
pub const SQL_SECTION: &str = "prsql";

/// The section's format. A runner reads every format up to the one it knows.
pub const SQL_FORMAT: u32 = 1;

/// The `prsql` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SqlSection {
    pub format: u32,
    /// The programs that have `EXEC SQL`, by number.
    pub programs: Vec<SqlProgram>,
    /// The statements, by number: the `stmt` that `sql_exec` names is an index here.
    pub statements: Vec<SqlStatement>,
}

/// One program that has `EXEC SQL`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SqlProgram {
    /// The `PROGRAM-ID`, upper case (for messages).
    pub name: String,
    /// The program as the parser left it (`cobolt_ast::program::Program`), without its procedure:
    /// its data and the cursors it declares, which is what the runner's interpreter is built from.
    pub program: serde_json::Value,
}

/// One `EXEC SQL` statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SqlStatement {
    /// The program it belongs to: an index in [`SqlSection::programs`].
    pub program: u32,
    /// The statement as the parser left it (`cobolt_ast::stmt::ExecSql`).
    pub exec: serde_json::Value,
    /// The cells whose values cross with the statement, in the order the module sends them.
    pub cells: Vec<SqlCell>,
}

/// An elementary item the statement may read or write, named as the program would name it in full.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqlCell {
    pub name: String,
    /// Its qualification, innermost first, up to its record.
    #[serde(default)]
    pub quals: Vec<String>,
    /// Its subscripts, from 1, for an occurrence of a table (an indicator table's).
    #[serde(default)]
    pub subs: Vec<i64>,
}

/// The values that cross `sql_exec(stmt, buf, len, cap) -> i32`.
///
/// On the way in, the `len` bytes at `buf` are one value per cell of the statement, in order. On
/// the way out the runner writes the answer over them (at most `cap` bytes): one byte, the
/// condition the statement ended in (`OUT_*`); a little-endian `u32`, the number of cells that
/// follow; and each of those cells as its number in the statement's list (`u32`) then its value.
/// The import returns the answer's length, or a negative number when the runner could not run the
/// statement at all (`FAILED`), and the module stops with a run-time error.
pub mod wire {
    /// A number: the mantissa (`i128`, little-endian, 16 bytes), then the scale (one byte).
    pub const DEC: u8 = 1;
    /// A floating item's value: an `f64`, little-endian.
    pub const FLOAT: u8 = 2;
    /// Bytes: their count (`u32`, little-endian), then the bytes.
    pub const BYTES: u8 = 3;

    /// The statement succeeded (SQLSTATE class `00`).
    pub const OUT_SUCCESS: u8 = 0;
    /// A warning (class `01`): `WHENEVER SQLWARNING` applies.
    pub const OUT_WARNING: u8 = 1;
    /// No data (class `02`): `WHENEVER NOT FOUND` applies.
    pub const OUT_NOT_FOUND: u8 = 2;
    /// An error (any other class): `WHENEVER SQLERROR` applies.
    pub const OUT_ERROR: u8 = 3;

    /// The runner could not run the statement (a damaged section, a cell it cannot find).
    pub const FAILED: i32 = -1;
}

/// The SQL built-ins (ABI A42, DESKTOP-WASM-001 T41b): `COBOL::"OPEN-DB"`, `"EXEC-SQL"`, `"FETCH-ROW"`,
/// `"NEXT-ROW"`, `"ROW-COUNT"` and `"CLOSE-DB"` (and their `CALL "COBOL-…"` form), through the one `prn`
/// import `sql_db(op, handle, number, text, text_len, buf, cap) -> i32`.
///
/// The runner keeps the connections, one registry per form instance (and one for a console program),
/// as each form's interpreter keeps its own, shared by its handlers and the procedures they call. It
/// runs each call with the interpreter's own code (`DbRegistry::builtin`). The module sends the
/// handle and the column as the interpreter's `as_i64` reads them, and the connection string or the
/// query as text; the runner trims the text, as the interpreter does. The answer is the value of each
/// receiver, in the order the built-in names them, written at `buf` (at most `cap` bytes; a text is cut
/// to fit), and its length is returned; a negative answer when the runner could not run the call.
pub mod db {
    /// `OPEN-DB ( connection handle status )`: the connection string is the text.
    pub const OPEN: u32 = 1;
    /// `EXEC-SQL ( handle query count status )`: the query is the text.
    pub const EXEC: u32 = 2;
    /// `FETCH-ROW ( handle column value status )`: the column is the number (1 when the argument is
    /// not a number, as the interpreter reads it).
    pub const FETCH_ROW: u32 = 3;
    /// `NEXT-ROW ( handle flag )`.
    pub const NEXT_ROW: u32 = 4;
    /// `ROW-COUNT ( handle count )`.
    pub const ROW_COUNT: u32 = 5;
    /// `CLOSE-DB ( handle )`: nothing is answered.
    pub const CLOSE: u32 = 6;

    /// A receiver's number: an `i64`, little-endian, put as the interpreter's `set_i64` puts it.
    pub const NUMBER: u8 = 1;
    /// A receiver's text: its length (`u32`, little-endian), then its bytes, put as the interpreter's
    /// `set_str` puts them.
    pub const TEXT: u8 = 2;

    /// The runner could not run the call (an operation it does not know, memory it cannot reach).
    pub const FAILED: i32 = -1;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The section reads back as it was written, and a field a later release adds is optional.
    #[test]
    fn the_section_round_trips_and_reads_an_older_cell() {
        let s = SqlSection {
            format: SQL_FORMAT,
            programs: vec![SqlProgram { name: "SHOP".into(), program: serde_json::json!({"x": 1}) }],
            statements: vec![SqlStatement {
                program: 0,
                exec: serde_json::json!({"kind": "Commit"}),
                cells: vec![SqlCell { name: "CITY".into(), quals: vec!["CUSTOMER".into()], subs: vec![] }],
            }],
        };
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<SqlSection>(&text).unwrap(), s);
        let cell: SqlCell = serde_json::from_str(r#"{"name":"SQLSTATE"}"#).unwrap();
        assert_eq!(cell, SqlCell { name: "SQLSTATE".into(), quals: vec![], subs: vec![] });
        println!("prsql: 1 section round-tripped, 1 cell of an older shape read");
    }
}
