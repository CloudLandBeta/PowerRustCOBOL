// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 087 T23 — the end of the run unit (R37, R38): work left open is
//! committed at a normal end and rolled back after an error or a cancel; a
//! form that joined the run unit releases only what is its own.

use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc, Mutex};

use cobolt_forms::connections::SqlConnection;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::esql::catalog::{SqlCatalog, Source};
use cobolt_runtime::esql::SqlRunUnit;
use cobolt_runtime::{Interpreter, RuntimeError};

fn interp(src: &str) -> (Interpreter, mpsc::Receiver<String>) {
    let result = parse(tokenize(src, SourceFormat::Free));
    let errors: Vec<_> = result.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    (Interpreter::new_with_channels(result.program.unwrap(), event_rx, state_tx, display_tx), display_rx)
}

/// A SQLite file holding `ORDERS` with the row 41, named SALES and the default.
fn sales(dir: &std::path::Path) -> SqlCatalog {
    let c = rusqlite::Connection::open(dir.join("sales.db")).unwrap();
    c.execute_batch("CREATE TABLE ORDERS (ID INTEGER); INSERT INTO ORDERS VALUES (41);").unwrap();
    let mut s = SqlConnection::new("SALES");
    s.path = "sales.db".into();
    s.default = true;
    SqlCatalog::new("shop", vec![s], dir.to_path_buf(), Source::Injected)
}

/// The rows the file holds now, as another program would see them.
fn ids(dir: &std::path::Path) -> Vec<i64> {
    let c = rusqlite::Connection::open(dir.join("sales.db")).unwrap();
    let mut q = c.prepare("SELECT ID FROM ORDERS ORDER BY ID").unwrap();
    q.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
}

/// Insert 42 with no COMMIT, then end as `ending` says.
fn insert_then(ending: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.
PROGRAM-ID. ENDS.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 SQLSTATE PIC X(5).
PROCEDURE DIVISION.
MAIN-PARA.
    EXEC SQL INSERT INTO ORDERS VALUES (42) END-EXEC
    DISPLAY \"INSERT \" SQLSTATE
    {ending}
"
    )
}

/// R38: `STOP RUN` and `GOBACK` with work open commit it.
#[test]
fn a_normal_end_commits_the_open_work() {
    for ending in ["STOP RUN.", "GOBACK.", "CONTINUE."] {
        let dir = tempfile::tempdir().unwrap();
        let (mut i, out) = interp(&insert_then(ending));
        i.set_sql_catalog(sales(dir.path()));
        let r = i.run();
        assert!(r.is_ok() || r.as_ref().is_err_and(RuntimeError::is_exit_signal), "{ending}: {r:?}");
        assert_eq!(out.try_iter().next().as_deref().map(str::trim_end), Some("INSERT 00000"));
        assert_eq!(ids(dir.path()), [41, 42], "{ending}: the open work is committed");
    }
    println!("R38: STOP RUN, GOBACK and falling off the end each commit the open INSERT — rows 41, 42");
}

/// R38: a run that ends in a runtime error rolls its open work back.
#[test]
fn an_error_rolls_the_open_work_back() {
    let dir = tempfile::tempdir().unwrap();
    let (mut i, _out) = interp(&insert_then("MOVE \"X\" TO super::Title\n    STOP RUN."));
    i.set_sql_catalog(sales(dir.path()));
    let r = i.run();
    assert!(r.as_ref().is_err_and(|e| !e.is_exit_signal()), "the CALL fails the run: {r:?}");
    assert_eq!(ids(dir.path()), [41], "the open INSERT is rolled back");
    println!("R38: a runtime error ({}) rolls the open INSERT back — row 41 only", r.unwrap_err());
}

/// R38: a cancelled run (the IDE's Stop) rolls its open work back.
#[test]
fn a_cancel_rolls_the_open_work_back() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = sales(dir.path());
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    // The interpreter is not `Send`: it is built on the thread that runs it.
    let run = std::thread::spawn(move || {
        let (mut i, _out) = interp(&insert_then("PERFORM UNTIL 1 = 0\n        CONTINUE\n    END-PERFORM\n    STOP RUN."));
        i.set_sql_catalog(catalog);
        i.set_cancel_flag(flag);
        i.run()
    });
    std::thread::sleep(std::time::Duration::from_millis(200));
    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    let r = run.join().unwrap();
    assert_eq!(ids(dir.path()), [41], "the open INSERT is rolled back: {r:?}");
    println!("R38: a cancel during an endless loop rolls the open INSERT back — row 41 only");
}

/// R37: a member reads through the root's connection — its uncommitted row
/// included — and its own end neither commits nor closes; the root's does.
#[test]
fn a_member_shares_the_connection_and_does_not_end_it() {
    let dir = tempfile::tempdir().unwrap();
    let unit = Arc::new(Mutex::new(SqlRunUnit::default()));

    // The root's work so far, left open: its run unit is the shared one.
    let (mut root, _) = interp(&insert_then("CONTINUE."));
    root.set_sql_run_unit(Arc::clone(&unit), true);
    root.set_sql_catalog(sales(dir.path()));
    // Run it as a MEMBER first, so its end leaves the work open — then it is
    // the root again for the final end below.
    root.set_sql_run_unit(Arc::clone(&unit), false);
    root.run().unwrap();
    assert_eq!(unit.lock().unwrap().sessions.len(), 1, "a member's end leaves the connection open");
    assert_eq!(ids(dir.path()), [41], "and does not commit");

    let (mut member, out) = interp(
        "IDENTIFICATION DIVISION.
PROGRAM-ID. MEMBER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 SQLSTATE PIC X(5).
01 WS-N PIC 9(3).
PROCEDURE DIVISION.
MAIN-PARA.
    EXEC SQL DECLARE C1 CURSOR FOR SELECT ID FROM ORDERS END-EXEC
    EXEC SQL OPEN C1 END-EXEC
    EXEC SQL SELECT COUNT(*) INTO :WS-N FROM ORDERS END-EXEC
    DISPLAY \"COUNT \" SQLSTATE \" \" WS-N
    EXEC SQL INSERT INTO ORDERS VALUES (99) END-EXEC
    STOP RUN.
",
    );
    member.set_sql_run_unit(Arc::clone(&unit), false);
    member.run().ok();
    assert_eq!(out.try_iter().next().as_deref().map(str::trim_end), Some("COUNT 00000 002"), "the root's open row is seen");
    {
        let u = unit.lock().unwrap();
        assert_eq!(u.sessions.len(), 1, "the member's end leaves the connection open");
        assert!(u.cursors.is_empty(), "but releases its own cursor");
    }
    assert_eq!(ids(dir.path()), [41], "nothing is committed by a member");

    // The root ends the run unit.
    let (mut end, _) = interp(
        "IDENTIFICATION DIVISION.
PROGRAM-ID. ROOT-END.
PROCEDURE DIVISION.
MAIN-PARA.
    STOP RUN.
",
    );
    end.set_sql_run_unit(Arc::clone(&unit), true);
    let _ = end.run();
    assert!(unit.lock().unwrap().sessions.is_empty(), "the root's end closes every connection");
    assert_eq!(ids(dir.path()), [41, 42, 99], "and commits the work of every form");
    println!("R37: a member saw 2 rows through the root's connection, released its cursor and left the work open; the root's end committed 41, 42, 99");
}
