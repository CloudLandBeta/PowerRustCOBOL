// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-005 T17 (R17; AC8): a browser cannot make a generated endpoint run its text.
//!
//! Against a real TLS loopback backend holding the programs the Build generates for a bound grid
//! (T9), a raw client sends what the player never would: a frame with an extra `sql`, `path`, `url`
//! or `command` member, and SQL text where a key or a cell goes. The extra member is refused by the
//! layout before the program runs; the text is a value the program compares or stores, never a
//! statement; and the backend's log says so — a `call` record whose outcome is the refusal's
//! class, never `ok`, and no payload in it.

mod support;

use std::path::{Path, PathBuf};

use cobolt_codegen::remote_controls::{fixtures, generate_backend};
use cobolt_runtime::Interpreter;
use serde_json::{json, Value};
use support::*;

const EXPORTS: &str = "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"SETUP-DB\"\n";
const READ: &str = "RC-6-ORDERS-9-GRID-CUST-READ";
const SAVE: &str = "RC-6-ORDERS-9-GRID-CUST-SAVE";
const ITEMS_READ: &str = "RC-6-ORDERS-10-GRID-ITEMS-READ";
const ITEMS_SAVE: &str = "RC-6-ORDERS-10-GRID-ITEMS-SAVE";

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

/// Three customers, written by a COBOL program as a developer's would.
fn make_customers(def: &cobolt_indexed::IndexedDefinition, data: &Path) {
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAKE-FILE.\n       ENVIRONMENT DIVISION.\n       INPUT-OUTPUT SECTION.\n       FILE-CONTROL.\n{}       DATA DIVISION.\n       FILE SECTION.\n{}       PROCEDURE DIVISION.\n           OPEN OUTPUT CUSTOMER-FILE\n           MOVE 1 TO CUST-ID\n           MOVE \"Ana\" TO CUST-NAME\n           MOVE -12.50 TO CUST-BALANCE\n           MOVE 1 TO CUST-ACTIVE\n           WRITE CUSTOMER-REC\n           MOVE 2 TO CUST-ID\n           MOVE \"Bruno\" TO CUST-NAME\n           MOVE 9.90 TO CUST-BALANCE\n           MOVE 0 TO CUST-ACTIVE\n           WRITE CUSTOMER-REC\n           MOVE 3 TO CUST-ID\n           MOVE \"Carla\" TO CUST-NAME\n           MOVE 0 TO CUST-BALANCE\n           MOVE 0 TO CUST-ACTIVE\n           WRITE CUSTOMER-REC\n           CLOSE CUSTOMER-FILE\n           STOP RUN.\n",
        cobolt_codegen::generate_indexed_select(def),
        cobolt_codegen::generate_indexed_fd(def).replacen("FD  CUSTOMER-FILE.\n", "FD  CUSTOMER-FILE\n", 1)
    );
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
    let program = parsed.program.expect("the writer parses");
    let (_etx, erx) = std::sync::mpsc::channel();
    let (stx, _srx) = std::sync::mpsc::channel();
    let (dtx, _drx) = std::sync::mpsc::channel();
    let mut i = Interpreter::new_with_channels(program, erx, stx, dtx);
    i.run().expect("the writer runs");
    assert!(data.exists(), "the file was made");
}

/// A backend folder with the Build's generated programs for the fixture forms, over a customer file.
fn backend_with_generated(name: &str) -> PathBuf {
    let dir = backend_dir(name, EXPORTS, &[("setup.cbl", SETUP_DB)]);
    std::fs::create_dir_all(dir.join("data")).unwrap();
    let data = dir.join("data").join("customers.idx");
    let def = fixtures::customers_cidx(&data.to_string_lossy());
    make_customers(&def, &data);
    let data_text = data.to_string_lossy().into_owned();
    let load = move |_: &str| Ok(fixtures::customers_cidx(&data_text));
    let generated = generate_backend(&fixtures::grid_forms(), &load).expect("generates");
    let out = dir.join("generated");
    std::fs::create_dir_all(&out).unwrap();
    for p in &generated.programs {
        std::fs::write(out.join(&p.file_name), &p.source).unwrap();
    }
    std::fs::write(out.join("exports.toml"), generated.exports_toml()).unwrap();
    // The SQL grid's connection, as the backend's catalogue would name it.
    let mut conn = cobolt_forms::connections::SqlConnection::new("SALES");
    conn.path = dir.join("sales.db").to_string_lossy().into_owned();
    conn.create_if_missing = true;
    cobolt_runtime::esql::catalog::install(cobolt_runtime::esql::catalog::SqlCatalog::new(
        "shop",
        vec![conn],
        dir.clone(),
        cobolt_runtime::esql::catalog::Source::Injected,
    ));
    dir
}

fn change(action: &str, keys: &[&str], cells: &[&str]) -> Value {
    json!({
        "CHANGE-ACTION": action,
        "KEYS": keys.iter().map(|k| json!({"KEY-TEXT": k})).collect::<Vec<_>>(),
        "CELLS": cells.iter().map(|c| json!({"CELL-TEXT": c})).collect::<Vec<_>>(),
    })
}

fn names(reply: &Value) -> Vec<String> {
    reply["returning"]["ROWS"].as_array().unwrap().iter().map(|r| r["CELLS"][1]["CELL-TEXT"].as_str().unwrap().to_owned()).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_browser_cannot_make_a_generated_endpoint_run_its_text() {
    let dir = backend_with_generated("ac8-generated");
    let rig = rig(dir.clone(), "");
    let token = rig.issuer.access("clerk-8", AUDIENCE, 600, json!({"roles": ["clerk"]}));
    let mut c = Raw::connect(&rig, Some(ORIGIN)).await.expect("connect");
    assert_eq!(c.hello("1.0.0").await["accepted"], "ok");
    assert_eq!(c.auth(&token).await["t"], "auth-ok");
    let window = r#"{"ROW-OFFSET":0,"ROW-COUNT":10,"PARAMS":[]}"#;

    // The grid as the player reads it: three rows.
    let r = c.call(1, READ, &[("value", window)], true).await;
    assert_eq!(r["t"], "reply", "{r}");
    assert_eq!(names(&r), vec!["Ana", "Bruno", "Carla"]);

    // A member the layout does not have — SQL text, a path, a URL, a command — is refused by the
    // mapping before the program runs; the refusal names the member, nothing else of the frame.
    let smuggled = [("sql", "DROP TABLE ITEMS"), ("path", "../../etc/passwd"), ("url", "https://evil.example/exfil"), ("command", "rm -rf /")];
    let mut id = 2;
    for (member, text) in smuggled {
        let frame = format!(r#"{{"ROW-OFFSET":0,"ROW-COUNT":10,"PARAMS":[],"{member}":{}}}"#, serde_json::to_string(text).unwrap());
        let e = c.call(id, READ, &[("value", &frame)], true).await;
        id += 1;
        assert_eq!(e["t"], "error", "{e}");
        assert_eq!(class(&e), "validation", "{e}");
        let said = format!("{} {}", e["message"].as_str().unwrap_or(""), e["detail"].as_str().unwrap_or(""));
        assert!(said.contains(member), "the refusal names the member: {e}");
        assert!(!said.contains(text), "the refusal never repeats the frame's text: {e}");
    }

    // SQL text where a key goes is a value. The indexed grid's key is a numeric PICTURE: the
    // program refuses the text as a value it cannot hold (`92`, the field named) — never a statement.
    let s = c.call(id, SAVE, &[("value", &json!({"CHANGES": [change("DELETE", &["2 OR 1=1"], &[])]}).to_string())], true).await;
    id += 1;
    assert_eq!(s["returning"]["COMMIT-STATUS"], "92", "{s}");
    assert!(s["returning"]["COMMIT-MESSAGE"].as_str().unwrap().starts_with("CUST-ID:"), "{s}");
    // SQL text where a cell goes is a value: it is stored as the name it is (one that fits the
    // field's `X(20)`; a longer one is refused as too long, `92`, as any value would be).
    let s = c
        .call(id, SAVE, &[("value", &json!({"CHANGES": [change("UPDATE", &["1"], &["1", "'; DROP TABLE X; --", "-1250", "1"])]}).to_string())], true)
        .await;
    id += 1;
    assert_eq!(s["returning"]["COMMIT-STATUS"], "00", "{s}");
    let r = c.call(id, READ, &[("value", window)], true).await;
    id += 1;
    assert_eq!(names(&r), vec!["'; DROP TABLE X; --", "Bruno", "Carla"], "three rows, the text a name");

    // The SQL grid: the same texts reach the database only as host variables. A key that is SQL
    // text matches no row (`23`), and the smuggled members are refused before the program runs.
    let state = c.call(id, "SETUP-DB", &[("ref", "\"\"")], false).await;
    id += 1;
    assert_eq!(state["t"], "reply", "{state}");
    let s = c
        .call(
            id,
            ITEMS_SAVE,
            &[("value", &json!({"CHANGES": [change("INSERT", &[], &["1", "Pen", "1.50", "1"]), change("INSERT", &[], &["2", "Ink", "2", "0"])]}).to_string())],
            true,
        )
        .await;
    id += 1;
    assert_eq!(s["returning"]["COMMIT-STATUS"], "00", "{s}");
    let s = c.call(id, ITEMS_SAVE, &[("value", &json!({"CHANGES": [change("DELETE", &["2 OR 1=1"], &[])]}).to_string())], true).await;
    id += 1;
    assert_eq!(s["returning"]["COMMIT-STATUS"], "23", "a bound value that matches no row: {s}");
    let s = c.call(id, ITEMS_SAVE, &[("value", &json!({"CHANGES": [change("UPDATE", &["1"], &["1", "x'); DROP TABLE ITEMS; --", "1", "1"])]}).to_string())], true).await;
    id += 1;
    assert_eq!(s["returning"]["COMMIT-STATUS"], "00", "{s}");
    let r = c.call(id, ITEMS_READ, &[("value", window)], true).await;
    id += 1;
    assert_eq!(names(&r), vec!["x'); DROP TABLE ITEMS; --", "Ink"], "both rows there, the text a name");
    let frame = format!(r#"{{"ROW-OFFSET":0,"ROW-COUNT":10,"PARAMS":[],"sql":{}}}"#, serde_json::to_string("DROP TABLE ITEMS").unwrap());
    let e = c.call(id, ITEMS_READ, &[("value", &frame)], true).await;
    assert_eq!((e["t"].as_str(), class(&e).as_str()), (Some("error"), "validation"), "{e}");

    // The log: the refused frames ran no program — a `call` record with the refusal's class, never
    // `ok` — and nothing of what the frames carried is in it.
    let records = rig.sink.records();
    let count = |program: &str, outcome: &str| {
        records.iter().filter(|r| r.event == "call" && r.program.as_deref() == Some(program) && r.outcome.as_deref() == Some(outcome)).count()
    };
    assert_eq!((count(READ, "validation"), count(READ, "ok")), (4, 2), "{records:?}");
    assert_eq!((count(ITEMS_READ, "validation"), count(ITEMS_READ, "ok")), (1, 1));
    assert_eq!((count(SAVE, "ok"), count(ITEMS_SAVE, "ok")), (2, 3), "every save ran, its texts as values");
    let log = rig.sink.lines().join("\n");
    for text in ["DROP TABLE", "etc/passwd", "evil.example", "rm -rf", "1=1"] {
        assert!(!log.contains(text), "the log carries the frame's text: {text}");
    }
    println!(
        "AC8: 5 smuggled members refused (validation, program not run), SQL text in an indexed key refused as a value (92), in a SQL key matched no row (23), in a cell stored as a name; log: {} records, no payload text",
        records.len()
    );
    rig.server.stop();
    let _ = std::fs::remove_dir_all(&dir);
}
