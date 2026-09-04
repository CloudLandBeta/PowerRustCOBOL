// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! End-to-end SQL database-runtime test through the COBOL CALL surface.
//!
//! Exercises the full `COBOL-OPEN-DB` → `COBOL-EXEC-SQL` → `COBOL-FETCH-ROW`
//! → `COBOL-NEXT-ROW` → `COBOL-CLOSE-DB` chain against an in-memory SQLite
//! database (no server required). The same program works unchanged against
//! PostgreSQL or MySQL — only the connection string changes (`postgres://…`
//! / `mysql://…`); those live paths are covered by the `#[ignore]`d unit
//! tests in `db_runtime.rs`.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let tokens = tokenize(src, SourceFormat::Free);
    let result = parse(tokens);
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx
        .try_iter()
        .map(|l| l.trim_end().to_string())
        .collect()
}

const SQL_CRUD: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SQL-CRUD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-CONN     PIC X(32)  VALUE ":memory:".
       01 WS-HANDLE   PIC 9(9)   VALUE 0.
       01 WS-STATUS   PIC X(128) VALUE SPACES.
       01 WS-QUERY    PIC X(256) VALUE SPACES.
       01 WS-ROWCNT   PIC 9(9)   VALUE 0.
       01 WS-COL      PIC 9(4)   VALUE 1.
       01 WS-NAME     PIC X(16)  VALUE SPACES.
       01 WS-MORE     PIC X      VALUE "N".
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-OPEN-DB" USING WS-CONN WS-HANDLE WS-STATUS
           MOVE "CREATE TABLE c (id INTEGER, name TEXT)" TO WS-QUERY
           CALL "COBOL-EXEC-SQL"
               USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           MOVE "INSERT INTO c VALUES (1,'ANA'),(2,'BRUNO'),(3,'CARLOS')"
               TO WS-QUERY
           CALL "COBOL-EXEC-SQL"
               USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           DISPLAY "INSERTED " WS-ROWCNT
           MOVE "SELECT name FROM c ORDER BY id" TO WS-QUERY
           CALL "COBOL-EXEC-SQL"
               USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           DISPLAY "ROWS " WS-ROWCNT
           MOVE "Y" TO WS-MORE
           PERFORM UNTIL WS-MORE = "N"
               MOVE 1 TO WS-COL
               CALL "COBOL-FETCH-ROW"
                   USING WS-HANDLE WS-COL WS-NAME WS-STATUS
               DISPLAY "NAME " WS-NAME
               CALL "COBOL-NEXT-ROW" USING WS-HANDLE WS-MORE
           END-PERFORM
           CALL "COBOL-CLOSE-DB" USING WS-HANDLE
           STOP RUN.
"#;

#[test]
fn sqlite_crud_via_cobol_calls() {
    let out = run_capture(SQL_CRUD);
    assert_eq!(
        out,
        vec![
            "INSERTED 000000003",
            "ROWS 000000003",
            "NAME ANA",
            "NAME BRUNO",
            "NAME CARLOS",
        ]
    );
}

/// The **`::` surface**, which is how the PowerDemo3 SqlDatabase demo is
/// written — and which could not read a value at all before 1.64.4.
///
/// `Fetch()` returned a "1"/"0" flag while the demo moved it into a `PIC X`
/// and treated it as a row, so its `UNTIL WS-ROW = SPACES` loop never ended:
/// "0" is not SPACES. It appended "1" to a ListBox forever, which is what the
/// operator reported as "not showing the columns returned by the query" and
/// "incredible slow" (2026-09-03). Reading data needed the CALL surface and a
/// column index; nothing on this surface could name a column either.
///
/// This is the demo's own loop shape, run for real.
const SQL_METHOD_SURFACE: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-ROW   PIC X(200).
       01 WS-COLS  PIC X(200).
       01 WS-N     PIC 9(4).
       PROCEDURE DIVISION.
           MOVE DB-1::Open(":memory:") TO WS-N
           MOVE DB-1::Execute("CREATE TABLE persons (id INTEGER, name TEXT)") TO WS-N
           MOVE DB-1::Execute("INSERT INTO persons VALUES (1, 'Ada'), (2, 'Grace')") TO WS-N
           MOVE DB-1::Execute("SELECT id, name FROM persons ORDER BY id") TO WS-N
           DISPLAY "rows=" WS-N
           MOVE DB-1::ColumnNames() TO WS-COLS
           DISPLAY "cols=[" FUNCTION TRIM(WS-COLS) "]"
           COMPUTE WS-N = DB-1::ColumnCount()
           DISPLAY "ncols=" WS-N
           DISPLAY "second=[" DB-1::ColumnName(2) "]"
           PERFORM UNTIL 1 = 2
               MOVE DB-1::Fetch() TO WS-ROW
               IF WS-ROW = SPACES
                   EXIT PERFORM
               END-IF
               DISPLAY "row=[" FUNCTION TRIM(WS-ROW) "]"
           END-PERFORM
           DISPLAY "done"
           STOP RUN.
"#;

#[test]
fn the_method_surface_returns_rows_and_names_columns() {
    let out = run_capture(SQL_METHOD_SURFACE);
    let joined = out.join("\n");

    // The loop TERMINATES — the whole point. Without it this test hangs.
    assert!(joined.contains("done"), "the fetch loop must end: {joined}");

    // Columns, by name, from the SELECT's own order.
    assert!(
        joined.contains("cols=[id\tname]"),
        "expected the column names: {joined}"
    );
    assert!(joined.contains("ncols=0002"), "column count: {joined}");
    assert!(joined.contains("second=[name]"), "ColumnName(2): {joined}");

    // Each row once, as data rather than a flag.
    assert!(joined.contains("row=[1\tAda]"), "first row: {joined}");
    assert!(joined.contains("row=[2\tGrace]"), "second row: {joined}");
    assert_eq!(
        joined.matches("row=[").count(),
        2,
        "exactly two rows, no repeats: {joined}"
    );
}
