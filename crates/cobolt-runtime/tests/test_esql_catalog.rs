// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 087 T22 — a program reaches the project's SQL connections by name,
//! and with no CONNECT uses the default one (R33–R35).

use std::path::PathBuf;
use std::sync::mpsc;

use cobolt_forms::connections::SqlConnection;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::esql::catalog::{SqlCatalog, Source};
use cobolt_runtime::Interpreter;

fn run(src: &str, catalog: Option<SqlCatalog>) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let errors: Vec<_> = result.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(result.program.unwrap(), event_rx, state_tx, display_tx);
    if let Some(c) = catalog {
        interp.set_sql_catalog(c);
    }
    interp.run().expect("run failed");
    let out: Vec<String> = display_rx.try_iter().map(|l| l.trim_end().to_string()).collect();
    println!("{}", out.join("\n"));
    out
}

/// A SQLite file with one row, and a catalog naming it SALES.
fn sales(dir: &std::path::Path, default: bool) -> SqlCatalog {
    let db = dir.join("sales.db");
    let c = rusqlite::Connection::open(&db).unwrap();
    c.execute_batch("CREATE TABLE ORDERS (ID INTEGER); INSERT INTO ORDERS VALUES (41);").unwrap();
    let mut s = SqlConnection::new("SALES");
    s.path = "sales.db".into();
    s.default = default;
    SqlCatalog::new("shop", vec![s], dir.to_path_buf(), Source::Injected)
}

const BY_NAME: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. BY-NAME.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-ID PIC 9(5).
01 SQLSTATE PIC X(5).
PROCEDURE DIVISION.
MAIN-PARA.
    EXEC SQL CONNECT TO 'SALES' END-EXEC
    EXEC SQL SELECT ID INTO :WS-ID FROM ORDERS END-EXEC
    DISPLAY \"BY-NAME \" SQLSTATE \" \" WS-ID
    EXEC SQL CONNECT TO SALESX END-EXEC
    DISPLAY \"TYPO \" SQLSTATE
    STOP RUN.
";

const NO_CONNECT: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. NO-CONNECT.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-ID PIC 9(5).
01 SQLSTATE PIC X(5).
PROCEDURE DIVISION.
MAIN-PARA.
    EXEC SQL SELECT ID INTO :WS-ID FROM ORDERS END-EXEC
    DISPLAY \"DEFAULT \" SQLSTATE \" \" WS-ID
    STOP RUN.
";

/// AC9 (runtime part): `CONNECT TO 'SALES'` reaches the project's file, and a
/// mistyped name is 08001 — no empty database is created for it.
#[test]
fn connect_to_a_named_sql_connection() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(BY_NAME, Some(sales(dir.path(), false)));
    assert!(out.contains(&"BY-NAME 00000 00041".to_string()), "{out:?}");
    assert!(out.contains(&"TYPO 08001".to_string()), "{out:?}");
    let files: Vec<PathBuf> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().path()).collect();
    assert_eq!(files.len(), 1, "the typo created nothing: {files:?}");
}

/// R35: with no CONNECT the default SQL connection is used; with none
/// marked, 08003.
#[test]
fn the_default_sql_connection_or_08003() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(NO_CONNECT, Some(sales(dir.path(), true)));
    assert!(out.contains(&"DEFAULT 00000 00041".to_string()), "{out:?}");
    let dir2 = tempfile::tempdir().unwrap();
    let out = run(NO_CONNECT, Some(sales(dir2.path(), false)));
    assert!(out.contains(&"DEFAULT 08003 00000".to_string()), "{out:?}");
}
