// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T23 (R6): the SQL connections of a built Backend, read beside the runner's file as a Cargo-built Backend reads
//! them.
//!
//! The Build writes `sql-connections.toml` beside the executable; the program says `sql-connection:LEDGER` (or
//! `CONNECT TO 'LEDGER'`) and never a path or a password. The runner installs the file's connections before it serves, and
//! a call that opens `LEDGER` reaches the database the file names. What a call commits is there for the next one.
#![cfg(feature = "sql")]

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cobolt_backend_runner::payload::{self, Package};
use cobolt_program_package::backend::BackendContainer;
use cobolt_server::backend::Backend;
use cobolt_server::engine::ExecOptions;
use cobolt_server::testkit::backend_dir;
use cobolt_wasm::{compile_backend, BackendProgram, Options};

const EXPORTS: &str = "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"TALLY\"\n";

/// Counts the rows of a table in the database the connection `LEDGER` names, and adds one when asked.
const TALLY: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. TALLY.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC 9(4).
01 WS-ROW PIC X(40).
LINKAGE SECTION.
01 REQ.
    05 ADDING PIC 9(1).
01 RES.
    05 ROWS PIC 9(4).
PROCEDURE DIVISION USING REQ RETURNING RES.
MAIN-PARA.
    MOVE COBOL::SqlDatabase::Open("sql-connection:LEDGER") TO WS-N
    MOVE COBOL::SqlDatabase::Execute("CREATE TABLE IF NOT EXISTS t (id INTEGER)") TO WS-N
    IF ADDING = 1
        MOVE COBOL::SqlDatabase::Execute("INSERT INTO t VALUES (1)") TO WS-N
    END-IF
    MOVE COBOL::SqlDatabase::Query("SELECT COUNT(*) FROM t") TO WS-N
    SET WS-ROW TO COBOL::SqlDatabase::Fetch()
    MOVE FUNCTION NUMVAL(WS-ROW) TO ROWS
    COBOL::SqlDatabase::Close()
    GOBACK.
"#;

fn parse(src: &str) -> cobolt_ast::program::Program {
    use cobolt_lexer::{tokenize, SourceFormat};
    let parsed = cobolt_parser::parse(tokenize(src, SourceFormat::Free));
    assert!(parsed.diagnostics.iter().all(|d| !d.is_error()), "the source does not parse");
    parsed.program.expect("a program")
}

fn rows(engine: &dyn cobolt_server::engine::Engine, backend: &Backend, adding: u32) -> u32 {
    let opts = ExecOptions::plain(Arc::new(AtomicBool::new(false)), Duration::from_secs(1));
    let answered = engine
        .execute(backend.program("TALLY").expect("TALLY is exported"), &[format!("{{\"ADDING\":{adding}}}")], opts)
        .unwrap_or_else(|e| panic!("the call failed: {e:?}"));
    let text = answered.returning.expect("RETURNING answered");
    let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.parse().unwrap_or_else(|_| panic!("rows in {text}"))
}

#[test]
fn a_call_opens_the_connection_the_file_beside_the_runner_names() {
    let started = Instant::now();
    let dir = backend_dir("connections", EXPORTS, &[("tally.cbl", TALLY)]);
    let db = dir.join("ledger.db");
    std::fs::write(
        dir.join("sql-connections.toml"),
        format!("[[connection]]\nname = \"LEDGER\"\nbackend = \"sqlite\"\npath = \"{}\"\ndefault = true\ncreate-if-missing = true\n", db.display()),
    )
    .unwrap();

    let source = Backend::load(&dir).expect("the Backend loads");
    let program = parse(TALLY);
    let wasm = compile_backend(&[BackendProgram { name: "TALLY".into(), program: &program }], &Options { native: true, ..Options::default() })
        .unwrap_or_else(|e| panic!("compile failed: {e}"))
        .wasm;
    let container = BackendContainer {
        state: "per-call".into(),
        interface: source.interface.to_json().into_bytes(),
        exports: serde_json::to_vec(&source.export_entries()).unwrap(),
        history: b"[]".to_vec(),
        slots: b"[]".to_vec(),
        module: wasm,
        name: "PowerTest".into(),
    };

    // What the runner does at start: read the file beside it, then load the Backend.
    cobolt_runner::install_sql_connections_in(&dir, "PowerTest");
    let loaded =
        payload::load(Package { container, code: None }, &cobolt_backend_runner::provided()).unwrap_or_else(|e| panic!("the runner refuses it: {e:?}"));
    let engine = loaded.engine.expect("the runner's engine");

    let counts = [
        rows(engine.as_ref(), &loaded.backend, 0),
        rows(engine.as_ref(), &loaded.backend, 1),
        rows(engine.as_ref(), &loaded.backend, 1),
        rows(engine.as_ref(), &loaded.backend, 0),
    ];
    assert_eq!(counts, [0, 1, 2, 2], "each call sees what the calls before it committed, in the database the file names");
    assert!(db.is_file(), "the database is the file the connection names: {}", db.display());
    println!(
        "a Backend that opens `sql-connection:LEDGER` served from the connection in sql-connections.toml beside it: rows read {counts:?} over 4 calls, the database file {} bytes ({} ms)",
        std::fs::metadata(&db).map(|m| m.len()).unwrap_or(0),
        started.elapsed().as_millis()
    );
    println!("PASS 002 FAIL 000");
    let _ = std::fs::remove_dir_all(&dir);
}
