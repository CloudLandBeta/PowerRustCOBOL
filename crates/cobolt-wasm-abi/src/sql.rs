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

/// The newest format of the section this release reads. A runner reads every format up to the one it
/// knows, and refuses a newer one: its meaning cannot be guessed.
///
/// - **1**: the programs, the statements and their cells.
/// - **2** (T50a5): an SQL descriptor area. A statement may carry [`SqlEntries`] (the descriptor's table
///   crosses as far as it is used, not whole) and a program its [`SqlProgram::ptr_items`] (the items
///   whose address its pointers hold, which the descriptor's data pointers point at). A runner that
///   ignored either would read the descriptor's pointers as no item at all, so an older one refuses.
pub const SQL_FORMAT: u32 = 2;

/// The format a generator writes for a section: 1, unless a program or a statement needs a later one,
/// so a module that does not use a later field is byte for byte what an earlier release made.
pub fn format_for(programs: &[SqlProgram], statements: &[SqlStatement]) -> u32 {
    if programs.iter().any(|p| !p.ptr_items.is_empty()) || statements.iter().any(|s| s.entries.is_some()) {
        2
    } else {
        1
    }
}

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
    /// Format 2: the items whose address the program's pointers can hold, in the order the program
    /// numbers them (the first is 1). The runner's interpreter numbers its own addresses the same
    /// way, so a data pointer of an SQL descriptor area, which crosses as one of these numbers,
    /// names the same item on both sides.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ptr_items: Vec<SqlCell>,
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
    /// Format 2: the statement works on an SQL descriptor area, whose table of entries crosses only as
    /// far as the statement uses it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entries: Option<SqlEntries>,
}

/// How a statement's cells divide when it works on an SQL descriptor area (format 2).
///
/// The cells are, in order: `head` cells that always cross; `max` entries of `per` cells each; and
/// the rest, which always cross (the statement's host variables, the status items and the items the
/// descriptor's pointers can point at). The module sends the first `n` entries, `n` being the value
/// of cell `limit` (the head cell that holds `SQLDA-COUNT`, or `SQLDA-CAPACITY` for `DESCRIBE`) kept
/// within `0..=max`, so a descriptor of a hundred entries whose statement names three costs three
/// entries, not a hundred (a whole descriptor is longer than the area a statement has). The runner
/// reads the same cells in the same order: it takes `limit` from the head it has just read.
///
/// An entry's `data` and `ind_ptr` cells (their place among the `per` cells) are pointers: they
/// cross as the **position** (from 1) of the item they point at in [`SqlProgram::ptr_items`], or 0.
/// The cell at `skip` (the entry's inline text, which only a NULL data pointer uses) is left out of
/// an entry whose `data` pointer is not NULL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqlEntries {
    pub head: u32,
    pub per: u32,
    pub max: u32,
    pub limit: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ind_ptr: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip: Option<u32>,
}

impl SqlEntries {
    /// The number of entries that cross when `limit` holds `value`.
    pub fn crossing(&self, value: i64) -> u32 {
        value.clamp(0, i64::from(self.max)) as u32
    }

    /// Visit, in the order the module sends them, the cells that cross when `entries` entries do:
    /// the head, then each entry's cells, then the rest, `cell_count` being the statement's total.
    /// `visit` is given each cell's number in the statement's list and says, for an entry's `data`
    /// cell, whether its pointer is set (the value of any other cell's answer is not read); the
    /// entry's `skip` cell is visited only when its data pointer is not set.
    pub fn walk(&self, entries: u32, cell_count: u32, mut visit: impl FnMut(u32) -> bool) {
        for c in 0..self.head {
            visit(c);
        }
        for e in 0..entries {
            let mut set = false;
            for pos in 0..self.per {
                if Some(pos) == self.skip && set {
                    continue;
                }
                let is_set = visit(self.head + e * self.per + pos);
                if Some(pos) == self.data {
                    set = is_set;
                }
            }
        }
        for c in self.head + self.per * self.max..cell_count {
            visit(c);
        }
    }
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
/// On the way in, the `len` bytes at `buf` are one value per cell of the statement, in order (a
/// statement with [`SqlEntries`] sends the cells that structure names, and no others). On
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

/// `COBOL::SqlDatabase`, the program-level database (ABI A44, DESKTOP-WASM-001 T41d), through the one
/// `prn` import `sql_obj(op, member, member_len, text, text_len, buf, cap) -> i32`.
///
/// The object has the SqlDatabase control's verbs and properties and no form: the runner keeps its
/// properties, one set per form instance (and one for a console program), beside the connections
/// the SQL built-ins use, and runs each verb with the interpreter's own code
/// (`DbRegistry::control`). The member is its name as written; the text is a call's argument (empty
/// when it has none) or the value written, each as a control's path argument and `SET` text cross
/// (trimmed). The answer — the verb's, or the property's value — is written at `buf` and its whole
/// length returned; a length over `cap` means nothing was written. A negative answer: the runner
/// could not run it.
pub mod obj {
    /// A verb: `Open`, `Execute`/`Exec`, `Query`, `BeginTransaction`, `Commit`, `Rollback`,
    /// `IsInTransaction`, `Fetch`, `FetchAll`, `ColumnNames`, `ColumnCount`, `ColumnName`, `Close`,
    /// `Cancel`, `IsBusy`.
    pub const CALL: u32 = 1;
    /// A property read.
    pub const GET: u32 = 2;
    /// A property write: the text is the value.
    pub const SET: u32 = 3;

    /// The runner could not run it (an operation it does not know, memory it cannot reach).
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
            programs: vec![SqlProgram { name: "SHOP".into(), program: serde_json::json!({"x": 1}), ptr_items: vec![] }],
            statements: vec![SqlStatement {
                program: 0,
                exec: serde_json::json!({"kind": "Commit"}),
                cells: vec![SqlCell { name: "CITY".into(), quals: vec!["CUSTOMER".into()], subs: vec![] }],
                entries: None,
            }],
        };
        assert_eq!(format_for(&s.programs, &s.statements), 1);
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<SqlSection>(&text).unwrap(), s);
        let cell: SqlCell = serde_json::from_str(r#"{"name":"SQLSTATE"}"#).unwrap();
        assert_eq!(cell, SqlCell { name: "SQLSTATE".into(), quals: vec![], subs: vec![] });
        println!("prsql: 1 section round-tripped, 1 cell of an older shape read");
    }

    /// T50a5: a descriptor statement makes a format 2 section, and the cells that cross with `n`
    /// entries are the head, the entries' cells (the inline text only where the data pointer is NULL)
    /// and the rest, in the order the module sends them.
    #[test]
    fn a_descriptor_statement_is_format_2_and_names_the_cells_that_cross() {
        // 3 head cells; 4 entries of 4 cells (data, indicator pointer, indicator, text); 2 more cells.
        let e = SqlEntries { head: 3, per: 4, max: 4, limit: 1, data: Some(0), ind_ptr: Some(1), skip: Some(3) };
        let total = 3 + 4 * 4 + 2;
        let crossing = |entries: u32, set: &dyn Fn(u32) -> bool| {
            let mut seen = Vec::new();
            e.walk(entries, total, |c| {
                seen.push(c);
                // The data pointer of entry `i` is cell 3 + 4i.
                c >= 3 && (c - 3) % 4 == 0 && set((c - 3) / 4)
            });
            seen
        };
        // Two entries cross; the second has its data pointer set, so its text stays behind.
        assert_eq!(crossing(2, &|i| i == 1), vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 19, 20]);
        assert_eq!(e.crossing(-5), 0);
        assert_eq!(e.crossing(2), 2);
        assert_eq!(e.crossing(900), 4);
        assert_eq!(crossing(0, &|_| false), vec![0, 1, 2, 19, 20]);
        assert_eq!(crossing(4, &|_| true), vec![0, 1, 2, 3, 4, 5, 7, 8, 9, 11, 12, 13, 15, 16, 17, 19, 20]);
        let s = SqlSection {
            format: 2,
            programs: vec![SqlProgram { name: "P".into(), program: serde_json::json!({}), ptr_items: vec![SqlCell { name: "WS-CODE".into(), quals: vec![], subs: vec![] }] }],
            statements: vec![SqlStatement { program: 0, exec: serde_json::json!({}), cells: vec![], entries: Some(e) }],
        };
        assert_eq!(format_for(&s.programs, &s.statements), 2);
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<SqlSection>(&text).unwrap(), s);
        let old: SqlStatement = serde_json::from_str(r#"{"program":0,"exec":{},"cells":[]}"#).unwrap();
        assert!(old.entries.is_none());
        println!("prsql descriptor: cells crossing for 0, 2 and 4 entries named, format 2 chosen only with entries or pointer items, an older statement read · 9 of 9 checks passed");
    }
}
