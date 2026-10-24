// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-005 T10: the backend reads the Build's `generated/` folder beside the
//! developer's `exports.toml` and `src/` — and the programs the Build writes
//! there (T9) run on it: a bound grid's rows are read and saved through them.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use cobolt_codegen::remote_controls::{fixtures, generate_backend, GeneratedBackend};
use cobolt_server::backend::Backend;
use cobolt_server::bundle::Bundle;
use cobolt_server::exec::{execute, ExecOptions};
use serde_json::{json, Value};

const MY_EXPORTS: &str = "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"SETUP-DB\"\n";

/// The developer's own program: makes the SQL grid's table.
const SETUP_DB: &str = r#"       IDENTIFICATION DIVISION.
       PROGRAM-ID. SETUP-DB.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SQLSTATE PIC X(5).
       01 SQLCODE PIC S9(9) COMP-5.
       01 SQLMSG PIC X(200).
       LINKAGE SECTION.
       01 WS-STATE PIC X(5).
       PROCEDURE DIVISION USING WS-STATE.
       MAIN-PARA.
           EXEC SQL CONNECT TO 'SALES' AS SETUP END-EXEC
           EXEC SQL DROP TABLE IF EXISTS ITEMS END-EXEC
           EXEC SQL CREATE TABLE ITEMS (ID INTEGER PRIMARY KEY,
                    NAME VARCHAR(40), PRICE NUMERIC(9,2), IN_STOCK INTEGER)
           END-EXEC
           EXEC SQL COMMIT END-EXEC
           MOVE SQLSTATE TO WS-STATE
           EXEC SQL DISCONNECT SETUP END-EXEC
           GOBACK.
"#;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pr-gen-backend-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    dir
}

/// A backend folder: the developer's exports and program, and — unless
/// `generated` is `None` — what the Build wrote for the fixture forms, whose
/// indexed file lives at `data`.
fn backend_dir(tag: &str, generated: Option<&Path>) -> (PathBuf, GeneratedBackend) {
    let dir = scratch(tag);
    std::fs::write(dir.join("exports.toml"), MY_EXPORTS).unwrap();
    std::fs::write(dir.join("src").join("setup.cbl"), SETUP_DB).unwrap();
    let mut g = GeneratedBackend::default();
    if let Some(data) = generated {
        let data = data.to_string_lossy().into_owned();
        let load = move |_: &str| Ok(fixtures::customers_cidx(&data));
        g = generate_backend(&fixtures::grid_forms(), &load).expect("generates");
        let out = dir.join("generated");
        std::fs::create_dir_all(&out).unwrap();
        for p in &g.programs {
            std::fs::write(out.join(&p.file_name), &p.source).unwrap();
        }
        std::fs::write(out.join("exports.toml"), g.exports_toml()).unwrap();
    }
    (dir, g)
}

#[test]
fn a_backend_with_no_generated_folder_loads_as_before() {
    let (dir, _) = backend_dir("plain", None);
    let b = Backend::load(&dir).unwrap_or_else(|e| panic!("{e:#?}"));
    let names: Vec<&str> = b.interface.programs.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["SETUP-DB"]);
    assert_eq!(cobolt_server::backend::exported_names(&dir), vec!["SETUP-DB".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_generated_exports_and_programs_join_the_developers() {
    let (dir, g) = backend_dir("merge", Some(Path::new("customers.idx")));
    let b = Backend::load(&dir).unwrap_or_else(|e| panic!("{e:#?}"));
    let mut names: Vec<String> = b.interface.programs.iter().map(|p| p.name.clone()).collect();
    names.sort();
    let mut want: Vec<String> = g.programs.iter().map(|p| p.program_id.clone()).chain(["SETUP-DB".to_string()]).collect();
    want.sort();
    assert_eq!(names, want);
    let read = b.program("RC-6-ORDERS-9-GRID-CUST-READ").expect("generated READ exported");
    assert_eq!(read.sig.roles, vec!["clerk".to_string(), "manager".to_string()], "the grid's BackendRoles");
    assert!(!read.sig.public, "a generated endpoint is never public");
    assert!(read.sig.idempotent);
    assert_eq!(read.params, vec!["RC-PARAMS".to_string()]);
    assert_eq!(read.returning.as_deref(), Some("RC-RESULT"));
    // The layouts on the contract are the catalogue's, member for member.
    let row = cobolt_wire::controls::find("DataGrid", "READ").unwrap();
    let names_of = |item: &cobolt_wire::layout::Item| item.members().map(|m| m.name.clone()).collect::<Vec<_>>();
    assert_eq!(names_of(&read.sig.params[0].layout), names_of(&row.param_layout()));
    assert_eq!(names_of(&read.sig.returning.as_ref().unwrap().layout), names_of(&row.result_layout()));
    let save = b.program("RC-6-ORDERS-10-GRID-ITEMS-SAVE").unwrap();
    assert!(!save.sig.idempotent && save.sig.roles.is_empty());
    // `exported_names` (the frontend's Check) and a packed backend see them too.
    assert_eq!(cobolt_server::backend::exported_names(&dir).len(), g.programs.len() + 1);
    let packed = Bundle::from_dir(&dir).unwrap_or_else(|e| panic!("{e:#?}"));
    let back = Bundle::from_bytes(&packed.to_bytes().unwrap()).unwrap().into_backend().unwrap_or_else(|e| panic!("{e:#?}"));
    assert_eq!(back.interface.programs.len(), g.programs.len() + 1);
    println!("{} programs exported: {}", names.len(), names.join(", "));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_program_both_files_export_is_a_load_error_naming_it() {
    let (dir, _) = backend_dir("clash", Some(Path::new("customers.idx")));
    std::fs::write(
        dir.join("exports.toml"),
        format!("{MY_EXPORTS}\n[[export]]\nprogram = \"RC-6-ORDERS-9-GRID-CUST-READ\"\n"),
    )
    .unwrap();
    let e = Backend::load(&dir).err().expect("refused").join("\n");
    assert!(e.contains("RC-6-ORDERS-9-GRID-CUST-READ is exported by exports.toml and by the Build's generated/exports.toml"), "{e}");
    // A developer program with a generated PROGRAM-ID is refused too.
    std::fs::write(dir.join("exports.toml"), MY_EXPORTS).unwrap();
    std::fs::write(
        dir.join("src").join("mine.cbl"),
        SETUP_DB.replace("PROGRAM-ID. SETUP-DB.", "PROGRAM-ID. RC-6-REPORT-11-GRID-TOTALS-READ."),
    )
    .unwrap();
    let e = Backend::load(&dir).err().expect("refused").join("\n");
    assert!(e.contains("PROGRAM-ID RC-6-REPORT-11-GRID-TOTALS-READ of a program the Build generated"), "{e}");
    println!("{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── The generated programs, run ──────────────────────────────────────────────

fn call(b: &Backend, program: &str, arg: Value) -> Value {
    let p = b.program(program).unwrap_or_else(|| panic!("{program} is not exported"));
    let opts = ExecOptions::plain(Arc::new(AtomicBool::new(false)), std::time::Duration::from_secs(5));
    let r = execute(p, &[arg.to_string()], opts).unwrap_or_else(|e| panic!("{program}: {e:?}"));
    match &r.returning {
        Some(text) => serde_json::from_str(text).unwrap(),
        None => serde_json::from_str(&r.args[0]).unwrap(),
    }
}

fn window(offset: u32, count: u32) -> Value {
    json!({"ROW-OFFSET": offset, "ROW-COUNT": count, "PARAMS": []})
}

fn change(action: &str, keys: &[&str], cells: &[&str]) -> Value {
    json!({
        "CHANGE-ACTION": action,
        "KEYS": keys.iter().map(|k| json!({"KEY-TEXT": k})).collect::<Vec<_>>(),
        "CELLS": cells.iter().map(|c| json!({"CELL-TEXT": c})).collect::<Vec<_>>(),
    })
}

fn save(changes: Vec<Value>) -> Value {
    json!({"CHANGES": changes})
}

/// The rows of a READ answer, cells as text.
fn rows(answer: &Value) -> Vec<Vec<String>> {
    answer["ROWS"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["CELLS"].as_array().unwrap().iter().map(|c| c["CELL-TEXT"].as_str().unwrap().to_owned()).collect())
        .collect()
}

#[test]
fn an_indexed_grid_is_read_and_saved_through_its_generated_programs() {
    let data_dir = scratch("ixdata");
    let data = data_dir.join("customers.idx");
    let (dir, _) = backend_dir("ix", Some(&data));
    let b = Backend::load(&dir).unwrap_or_else(|e| panic!("{e:#?}"));
    const READ: &str = "RC-6-ORDERS-9-GRID-CUST-READ";
    const SAVE: &str = "RC-6-ORDERS-9-GRID-CUST-SAVE";

    // No file yet: an empty grid, and why.
    let a = call(&b, READ, window(0, 10));
    assert_eq!((a["READ-STATUS"].as_str(), a["ROW-TOTAL"].as_u64()), (Some("35"), Some(0)), "{a}");

    // A save never makes the file: 35, as the desktop's save answers — and
    // nothing appears on disk.
    let s = call(&b, SAVE, save(vec![change("INSERT", &[], &["3", "Carla", "0", "0"])]));
    assert_eq!(s["COMMIT-STATUS"], "35", "{s}");
    assert!(!data.exists(), "the save created the file");
    make_empty_indexed_file(&data);
    let s = call(
        &b,
        SAVE,
        save(vec![
            change("INSERT", &[], &["1", "Ana", "-1250", "1"]),
            change("INSERT", &[], &["2", "Bruno", "990", "0"]),
            change("INSERT", &[], &["3", "Carla", "0", "0"]),
        ]),
    );
    assert_eq!(s["COMMIT-STATUS"], "00", "{s}");

    let a = call(&b, READ, window(0, 10));
    assert_eq!(a["READ-STATUS"], "00", "{a}");
    assert_eq!(a["ROW-TOTAL"], 3);
    assert_eq!(
        rows(&a),
        vec![
            vec!["1", "Ana", "-1250", "1"],
            vec!["2", "Bruno", "990", "0"],
            vec!["3", "Carla", "0", "0"],
        ],
        "the numeric key without leading zeros, the COMP-3 balance as its value's digits with its sign (WASM-PARITY 19)"
    );
    // A window.
    let a = call(&b, READ, window(1, 1));
    assert_eq!((a["ROW-TOTAL"].as_u64(), rows(&a)), (Some(3), vec![vec!["2".to_string(), "Bruno".into(), "990".into(), "0".into()]]));

    // An update, a delete; a changed key is refused, and so is a value the PICTURE cannot hold.
    let s = call(
        &b,
        SAVE,
        save(vec![change("UPDATE", &["2"], &["2", "Bruno S.", "1000", "1"]), change("DELETE", &["3"], &[])]),
    );
    assert_eq!(s["COMMIT-STATUS"], "00", "{s}");
    let a = call(&b, READ, window(0, 10));
    assert_eq!(rows(&a), vec![vec!["1", "Ana", "-1250", "1"], vec!["2", "Bruno S.", "1000", "1"]]);
    let s = call(&b, SAVE, save(vec![change("UPDATE", &["2"], &["9", "Bruno", "1000", "1"])]));
    assert_eq!((s["COMMIT-STATUS"].as_str(), s["FAILED-CHANGE"].as_u64()), (Some("21"), Some(1)), "{s}");
    let s = call(
        &b,
        SAVE,
        save(vec![change("UPDATE", &["1"], &["1", "Ana", "-1250", "1"]), change("UPDATE", &["2"], &["2", "x", "12a", "1"])]),
    );
    assert_eq!((s["COMMIT-STATUS"].as_str(), s["FAILED-CHANGE"].as_u64()), (Some("92"), Some(2)), "{s}");
    assert!(s["COMMIT-MESSAGE"].as_str().unwrap().contains("CUST-BALANCE"), "{s}");

    // All or nothing: the second insert's key is taken by the first, so the
    // write fails after the first one succeeded — and the first is undone.
    let s = call(
        &b,
        SAVE,
        save(vec![change("INSERT", &[], &["5", "Eva", "1", "1"]), change("INSERT", &[], &["5", "Eve", "2", "1"])]),
    );
    assert_eq!((s["COMMIT-STATUS"].as_str(), s["FAILED-CHANGE"].as_u64()), (Some("22"), Some(2)), "{s}");
    let a = call(&b, READ, window(0, 10));
    assert_eq!(a["ROW-TOTAL"], 2, "the first insert was rolled back: {a}");
    println!("indexed grid: read, window, insert x3, update, delete, refusals 21/92, rollback after 22 — rows {:?}", rows(&a));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&data_dir);
}

/// An empty indexed file at `path`, written by a COBOL program as a developer's would.
fn make_empty_indexed_file(path: &Path) {
    let def = fixtures::customers_cidx(&path.to_string_lossy());
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAKE-FILE.\n       ENVIRONMENT DIVISION.\n       INPUT-OUTPUT SECTION.\n       FILE-CONTROL.\n{}       DATA DIVISION.\n       FILE SECTION.\n{}       PROCEDURE DIVISION.\n           OPEN OUTPUT CUSTOMER-FILE\n           CLOSE CUSTOMER-FILE\n           STOP RUN.\n",
        cobolt_codegen::generate_indexed_select(&def),
        cobolt_codegen::generate_indexed_fd(&def).replacen("FD  CUSTOMER-FILE.\n", "FD  CUSTOMER-FILE\n", 1)
    );
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
    let program = parsed.program.expect("parses");
    let (_etx, erx) = std::sync::mpsc::channel();
    let (stx, _srx) = std::sync::mpsc::channel();
    let (dtx, _drx) = std::sync::mpsc::channel();
    let mut i = cobolt_runtime::Interpreter::new_with_channels(program, erx, stx, dtx);
    let _ = i.run();
    assert!(path.exists(), "the file was made");
}

#[test]
fn an_sql_grid_is_read_and_saved_through_its_generated_programs() {
    let (dir, _) = backend_dir("sql", Some(Path::new("customers.idx")));
    let db = dir.join("sales.db");
    let mut conn = cobolt_forms::connections::SqlConnection::new("SALES");
    conn.path = db.to_string_lossy().into_owned();
    conn.create_if_missing = true;
    cobolt_runtime::esql::catalog::install(cobolt_runtime::esql::catalog::SqlCatalog::new(
        "shop",
        vec![conn],
        dir.clone(),
        cobolt_runtime::esql::catalog::Source::Injected,
    ));
    let b = Backend::load(&dir).unwrap_or_else(|e| panic!("{e:#?}"));
    assert_eq!(call(&b, "SETUP-DB", json!("")), json!("00000"));
    const READ: &str = "RC-6-ORDERS-10-GRID-ITEMS-READ";
    const SAVE: &str = "RC-6-ORDERS-10-GRID-ITEMS-SAVE";

    let s = call(
        &b,
        SAVE,
        save(vec![
            change("INSERT", &[], &["1", "Pen", "1.50", "yes"]),
            change("INSERT", &[], &["2", "Ink", "", "0"]),
            change("INSERT", &[], &["3", "O'Brien pad", "4", "true"]),
        ]),
    );
    assert_eq!(s["COMMIT-STATUS"], "00", "{s}");
    let a = call(&b, READ, window(0, 10));
    assert_eq!(a["READ-STATUS"], "00", "{a}");
    assert_eq!(a["ROW-TOTAL"], 3);
    assert_eq!(
        rows(&a),
        vec![vec!["1", "Pen", "1.5", "1"], vec!["2", "Ink", "", "0"], vec!["3", "O'Brien pad", "4", "1"]],
        "an empty number is NULL and shows empty; a quote in text is data"
    );
    let a = call(&b, READ, window(2, 5));
    assert_eq!((a["ROW-TOTAL"].as_u64(), rows(&a).len()), (Some(3), 1));

    let s = call(
        &b,
        SAVE,
        save(vec![change("UPDATE", &["2"], &["2", "Ink (blue)", "2.25", "1"]), change("DELETE", &["1"], &[])]),
    );
    assert_eq!(s["COMMIT-STATUS"], "00", "{s}");
    assert_eq!(rows(&call(&b, READ, window(0, 10))), vec![vec!["2", "Ink (blue)", "2.25", "1"], vec!["3", "O'Brien pad", "4", "1"]]);

    // One unit: the second change finds no row, so the first is rolled back.
    let s = call(
        &b,
        SAVE,
        save(vec![change("UPDATE", &["2"], &["2", "changed", "9", "1"]), change("DELETE", &["77"], &[])]),
    );
    assert_eq!((s["COMMIT-STATUS"].as_str(), s["FAILED-CHANGE"].as_u64()), (Some("23"), Some(2)), "{s}");
    assert_eq!(rows(&call(&b, READ, window(0, 10)))[0][1], "Ink (blue)", "rolled back");
    // A number that is not one is refused before the database is reached.
    let s = call(&b, SAVE, save(vec![change("INSERT", &[], &["4", "x", "1,5x", "0"])]));
    assert_eq!(s["COMMIT-STATUS"], "92", "{s}");
    // SQL text in a key is a value, never a statement.
    let s = call(&b, SAVE, save(vec![change("DELETE", &["2 OR 1=1"], &[])]));
    assert_eq!(s["COMMIT-STATUS"], "23", "{s}");
    assert_eq!(call(&b, READ, window(0, 10))["ROW-TOTAL"], 2, "nothing was deleted");
    println!("SQL grid: read, window, insert x3 (NULL, quote), update, delete, rollback after 23, 92, injected key refused");

    // T11: a SqlDatabase's `Query` on a web form, on the same connection: the
    // backend opens it, runs the handler's statement, and the rows travel
    // with the answer — the events are the Query's, not the fetches'.
    let (form, sites) = fixtures::control_form();
    let programs = cobolt_codegen::remote_controls::generate_control_programs(&form, &sites).expect("generates");
    let methods = scratch("sql-method");
    std::fs::write(methods.join("exports.toml"), "[contract]\nversion = \"1.0.0\"\n").unwrap();
    std::fs::create_dir_all(methods.join("generated")).unwrap();
    for p in &programs {
        std::fs::write(methods.join("generated").join(&p.file_name), &p.source).unwrap();
    }
    std::fs::write(methods.join("generated").join("exports.toml"), GeneratedBackend { programs, ..Default::default() }.exports_toml()).unwrap();
    let mb = Backend::load(&methods).unwrap_or_else(|e| panic!("{e:#?}"));
    let q = call(&mb, "RC-4-SHOP-4-DB-1-QUERY-1", json!({"ARG-COUNT": 0, "ARGS": []}));
    assert_eq!(q["LAST-ERROR"], "", "{q}");
    assert_eq!(q["RESULT-VALUE"], "2", "Query answers its row count: {q}");
    assert_eq!(rows(&q), vec![vec!["Ink (blue)", "2.25"], vec!["O'Brien pad", "4"]], "{q}");
    let events: Vec<&str> = q["EVENTS"].as_array().unwrap().iter().map(|e| e["EVENT-NAME"].as_str().unwrap()).collect();
    assert_eq!(events, vec!["onQueryComplete"], "{q}");
    println!("SqlDatabase Query through the backend: {} rows, events {events:?}", rows(&q).len());
    let _ = std::fs::remove_dir_all(&methods);
    let _ = std::fs::remove_dir_all(&dir);
}
