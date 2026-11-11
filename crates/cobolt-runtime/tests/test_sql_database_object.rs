// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `COBOL::SqlDatabase::<member>` (WASM-015 AC11): the program-level SQL
//! database a Backend uses WITHOUT `EXEC SQL` and without a SqlDatabase control
//! on a form. It is the SqlDatabase control's own verb surface on one object
//! that every program has, so each verb below answers exactly as `Db::Verb`
//! does in `test_sql.rs` — against an in-memory SQLite database, no server.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
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

fn line<'a>(out: &'a [String], prefix: &str) -> &'a str {
    out.iter()
        .find(|l| l.starts_with(prefix))
        .map(String::as_str)
        .unwrap_or_else(|| panic!("no {prefix} in {out:?}"))
}

/// Open, write, read back by row and by column, and close — the whole round trip.
const ROUND_TRIP: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N     PIC 9(4).
       01 WS-ROW   PIC X(80).
       01 WS-COLS  PIC X(80).
       01 WS-NAME  PIC X(20).
       PROCEDURE DIVISION.
           MOVE COBOL::SqlDatabase::Open(":memory:") TO WS-N
           DISPLAY "open=" WS-N
           MOVE COBOL::SqlDatabase::Execute("CREATE TABLE t (id INTEGER, name TEXT)") TO WS-N
           MOVE COBOL::SqlDatabase::Execute("INSERT INTO t VALUES (1, 'Ada')") TO WS-N
           DISPLAY "inserted=" WS-N
           COBOL::SqlDatabase::Execute("INSERT INTO t VALUES (2, 'Grace')")
           MOVE COBOL::SqlDatabase::Query("SELECT id, name FROM t ORDER BY id") TO WS-N
           DISPLAY "rows=" WS-N
           MOVE COBOL::SqlDatabase::ColumnNames() TO WS-COLS
           DISPLAY "cols=[" FUNCTION TRIM(WS-COLS) "]"
           MOVE COBOL::SqlDatabase::ColumnCount() TO WS-N
           DISPLAY "ncols=" WS-N
           MOVE COBOL::SqlDatabase::ColumnName(2) TO WS-NAME
           DISPLAY "second=[" FUNCTION TRIM(WS-NAME) "]"
           PERFORM UNTIL 1 = 2
               SET WS-ROW TO COBOL::SqlDatabase::Fetch()
               IF WS-ROW = SPACES
                   EXIT PERFORM
               END-IF
               DISPLAY "row=[" FUNCTION TRIM(WS-ROW) "]"
           END-PERFORM
           DISPLAY "status=" COBOL::SqlDatabase::StatusCode
           COBOL::SqlDatabase::Close()
           DISPLAY "done"
           STOP RUN.
"#;

#[test]
fn a_backend_opens_writes_queries_and_fetches_without_exec_sql() {
    let out = run_capture(ROUND_TRIP);
    assert!(
        line(&out, "open=").starts_with("open=000"),
        "Open answers the handle: {out:?}"
    );
    assert_ne!(
        line(&out, "open="),
        "open=0000",
        "the connection opened: {out:?}"
    );
    assert_eq!(line(&out, "inserted="), "inserted=0001");
    assert_eq!(line(&out, "rows="), "rows=0002");
    assert_eq!(line(&out, "cols="), "cols=[id\tname]");
    assert_eq!(line(&out, "ncols="), "ncols=0002");
    assert_eq!(line(&out, "second="), "second=[name]");
    assert_eq!(line(&out, "row=[1"), "row=[1\tAda]");
    assert_eq!(
        out.iter().filter(|l| l.starts_with("row=[")).count(),
        2,
        "exactly two rows, then the loop ends: {out:?}"
    );
    assert_eq!(
        line(&out, "status="),
        "status=0",
        "a successful Open leaves StatusCode 0: {out:?}"
    );
    assert_eq!(out.last().map(String::as_str), Some("done"));
}

/// The transaction verbs, `LastError` and `StatusCode` — the same answers as the control's.
const TRANSACTIONS: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N     PIC 9(4).
       01 WS-ERR   PIC X(80).
       PROCEDURE DIVISION.
           MOVE COBOL::SqlDatabase::Open(":memory:") TO WS-N
           MOVE COBOL::SqlDatabase::Execute("CREATE TABLE t (id INTEGER)") TO WS-N
           MOVE COBOL::SqlDatabase::Commit() TO WS-N
           MOVE COBOL::SqlDatabase::LastError TO WS-ERR
           DISPLAY "commit-none=" WS-N " " FUNCTION TRIM(WS-ERR)
           MOVE COBOL::SqlDatabase::BeginTransaction() TO WS-N
           MOVE COBOL::SqlDatabase::IsInTransaction() TO WS-N
           DISPLAY "open=" WS-N
           MOVE COBOL::SqlDatabase::Execute("INSERT INTO t VALUES (1)") TO WS-N
           MOVE COBOL::SqlDatabase::Rollback() TO WS-N
           MOVE COBOL::SqlDatabase::Execute("SELECT id FROM t") TO WS-N
           DISPLAY "after-rollback=" WS-N
           MOVE COBOL::SqlDatabase::BeginTransaction() TO WS-N
           MOVE COBOL::SqlDatabase::Execute("INSERT INTO t VALUES (2)") TO WS-N
           MOVE COBOL::SqlDatabase::Commit() TO WS-N
           MOVE COBOL::SqlDatabase::Execute("SELECT id FROM t") TO WS-N
           DISPLAY "after-commit=" WS-N
           MOVE COBOL::SqlDatabase::Execute("INSERT INTO nowhere VALUES (1)") TO WS-N
           MOVE COBOL::SqlDatabase::LastError TO WS-ERR
           DISPLAY "bad=" WS-N " [" FUNCTION TRIM(WS-ERR) "]"
           STOP RUN.
"#;

#[test]
fn transactions_and_errors_answer_as_the_control_does() {
    let out = run_capture(TRANSACTIONS);
    assert!(
        line(&out, "commit-none=").starts_with("commit-none=0000 "),
        "{out:?}"
    );
    assert!(
        line(&out, "commit-none=").contains("no transaction is open"),
        "{out:?}"
    );
    assert_eq!(line(&out, "open="), "open=0001");
    assert_eq!(
        line(&out, "after-rollback="),
        "after-rollback=0000",
        "the rolled-back insert is gone"
    );
    assert_eq!(
        line(&out, "after-commit="),
        "after-commit=0001",
        "the committed insert is kept"
    );
    let bad = line(&out, "bad=");
    assert!(
        bad.starts_with("bad=0000 ["),
        "a failed statement answers 0: {bad}"
    );
    assert!(!bad.ends_with("[]"), "and LastError says why: {bad}");
}

/// A program that names its connection by property, as a control's `ConnectionString` does.
const BY_PROPERTY: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N     PIC 9(4).
       PROCEDURE DIVISION.
           MOVE ":memory:" TO COBOL::SqlDatabase::ConnectionString
           MOVE COBOL::SqlDatabase::Open() TO WS-N
           IF WS-N = 0
               DISPLAY "no connection"
           ELSE
               DISPLAY "connected"
           END-IF
           MOVE COBOL::SqlDatabase::Execute("CREATE TABLE t (id INTEGER)") TO WS-N
           DISPLAY "created=" WS-N
           MOVE COBOL::SqlDatabase::Open("/no/such/dir/x.db") TO WS-N
           DISPLAY "bad-open=" WS-N " status=" COBOL::SqlDatabase::StatusCode
           STOP RUN.
"#;

#[test]
fn open_with_no_argument_uses_the_connection_string_property() {
    let out = run_capture(BY_PROPERTY);
    assert!(out.iter().any(|l| l == "connected"), "{out:?}");
    assert!(line(&out, "created=").starts_with("created="), "{out:?}");
    assert!(
        line(&out, "bad-open=").starts_with("bad-open=0000"),
        "a connection that cannot open answers 0: {out:?}"
    );
    assert!(
        line(&out, "bad-open=").ends_with("status=1"),
        "and StatusCode says so: {out:?}"
    );
}

/// The built-in `COBOL::NAME(…)` calls and the control's own `Db::` surface keep working beside it.
const BESIDE_THE_OTHERS: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N     PIC 9(4).
       01 WS-REF   PIC 9(4) VALUE 0.
       PROCEDURE DIVISION.
           MOVE COBOL::SqlDatabase::Open(":memory:") TO WS-N
           MOVE DB-1::Open(":memory:") TO WS-N
           MOVE DB-1::Execute("CREATE TABLE only_in_db_1 (id INTEGER)") TO WS-N
           COBOL::"PROVIDER-COUNT"(WS-REF)
           MOVE COBOL::SqlDatabase::Execute("SELECT id FROM only_in_db_1") TO WS-N
           DISPLAY "program-db-sees-db-1=" WS-N
           DISPLAY "db-1-has-no-error=[" DB-1::LastError "]"
           STOP RUN.
"#;

#[test]
fn it_is_its_own_connection_and_leaves_the_other_surfaces_alone() {
    let out = run_capture(BESIDE_THE_OTHERS);
    assert_eq!(
        line(&out, "program-db-sees-db-1="),
        "program-db-sees-db-1=0000",
        "two :memory: connections are two databases: {out:?}"
    );
    assert!(
        line(&out, "db-1-has-no-error=").contains("[]")
            || line(&out, "db-1-has-no-error=").contains("[ "),
        "{out:?}"
    );
}
