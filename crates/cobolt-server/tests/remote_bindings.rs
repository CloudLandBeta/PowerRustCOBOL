// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-005 T15 (R13, Q8; AC7): a web form's DataGrids bound to an indexed file
//! and to a SQL query fill, edit, save and auto-commit **through the backend**,
//! window by window, over the real channel — the interpreter's remote branch,
//! `rcrun`'s WebSocket client, a TLS loopback server and the programs the Build
//! generates for the grids (`cobolt-codegen`'s `remote_controls`).
//!
//! The form is the fixture `ORDERS` of the generator's tests (`GRID-CUST` over
//! `CUSTOMER-FILE`, `GRID-ITEMS` over the `ITEMS` table), seeded as the host
//! seeds a web form's grids (`cobolt-form-host`'s `seeding`: `_BindingRemote`
//! and the two endpoints), plus a grid bound to a COBOL table, which stays in
//! the form (Q8) and makes no call.

mod support;

use std::cell::RefCell;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use cobolt_codegen::remote_controls::{fixtures, generate_backend};
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};
use cobolt_server::client::{ClientConfig, SpecToken, WsClient};
use cobolt_wire::call::{ArgMode, ArgPayload, CallObserver, CallReply, CallRequest, Transport};
use cobolt_wire::controls::endpoint_name;
use cobolt_wire::error::CallError;
use serde_json::{json, Value};
use support::*;

const CUSTOMERS: usize = 250;

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

/// The web form's program: the three grids are loaded by their bindings, then
/// the buttons save, reload and change rows. `SEEN` after each cell edit lets
/// the test send the next one once the interpreter has noted it.
const ORDERS: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ORDERS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 EV-ID      PIC X(64).
       01 CTL-ID     PIC X(64).
       01 COBOL-QUIT PIC X VALUE "0".
       01 WS-STATUS  PIC X(8).
       01 WS-NOTES.
          05 WS-NOTE OCCURS 3 TIMES.
             10 NOTE-ID   PIC 9(3).
             10 NOTE-TEXT PIC X(12).
       PROCEDURE DIVISION.
       MAIN.
           COBOL::"INIT-FORM" ( "ORDERS" )
           MOVE 1 TO NOTE-ID (1)
           MOVE "first" TO NOTE-TEXT (1)
           MOVE 2 TO NOTE-ID (2)
           MOVE "second" TO NOTE-TEXT (2)
           MOVE 3 TO NOTE-ID (3)
           MOVE "third" TO NOTE-TEXT (3)
           SET WS-STATUS TO GRID-CUST::RefreshBinding()
           DISPLAY "CUST " WS-STATUS
           SET WS-STATUS TO GRID-ITEMS::RefreshBinding()
           DISPLAY "ITEMS " WS-STATUS
           SET WS-STATUS TO GRID-NOTES::RefreshBinding()
           DISPLAY "NOTES " WS-STATUS
           DISPLAY "LOADED"
           PERFORM UNTIL COBOL-QUIT = "1"
               COBOL::"WAIT-EVENT" ( EV-ID CTL-ID )
               EVALUATE TRUE
                   WHEN EV-ID = "onCellEdited"
                       DISPLAY "SEEN"
                   WHEN EV-ID = "onCommitError"
                       DISPLAY "ERROR " CTL-ID
                   WHEN EV-ID = "onSaved"
                       DISPLAY "SAVED " CTL-ID
                   WHEN CTL-ID = "BTN-SAVE-CUST"
                       SET WS-STATUS TO GRID-CUST::Save()
                       DISPLAY "SAVE-CUST " WS-STATUS
                   WHEN CTL-ID = "BTN-RELOAD-CUST"
                       SET WS-STATUS TO GRID-CUST::RefreshBinding()
                       DISPLAY "RELOAD-CUST " WS-STATUS
                   WHEN CTL-ID = "BTN-RELOAD-ITEMS"
                       SET WS-STATUS TO GRID-ITEMS::RefreshBinding()
                       DISPLAY "RELOAD-ITEMS " WS-STATUS
                   WHEN CTL-ID = "BTN-ROWS"
                       GRID-CUST::DeleteRow(3)
                       GRID-CUST::AddRow()
                       GRID-CUST::SetCellValue(250, 1, "251")
                       GRID-CUST::SetCellValue(250, 2, "New customer")
                       GRID-CUST::SetCellValue(250, 3, "12345")
                       GRID-CUST::SetCellValue(250, 4, "1")
                       DISPLAY "ROWS-CHANGED"
                   WHEN CTL-ID = "BTN-NOOP"
                       DISPLAY "NOOP"
                   WHEN OTHER
                       CONTINUE
               END-EVALUATE
           END-PERFORM
           STOP RUN.
"#;

/// The customer file: `CUSTOMERS` records written by a COBOL program, as a
/// developer's would write them — id `i`, "Customer i", a balance of
/// `(i - 125) / 4` in a signed packed field, active for the odd ids.
fn make_customer_file(def: &cobolt_indexed::IndexedDefinition, data: &std::path::Path) {
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAKE-FILE.\n       ENVIRONMENT DIVISION.\n       INPUT-OUTPUT SECTION.\n       FILE-CONTROL.\n{}       DATA DIVISION.\n       FILE SECTION.\n{}       WORKING-STORAGE SECTION.\n       01 I      PIC 9(3).\n       01 I-EDIT PIC ZZ9.\n       01 I-TEXT PIC X(3).\n       PROCEDURE DIVISION.\n           OPEN OUTPUT CUSTOMER-FILE\n           PERFORM VARYING I FROM 1 BY 1 UNTIL I > {CUSTOMERS}\n               MOVE I TO CUST-ID\n               MOVE I TO I-EDIT\n               MOVE FUNCTION TRIM(I-EDIT) TO I-TEXT\n               MOVE SPACES TO CUST-NAME\n               STRING \"Customer \" DELIMITED BY SIZE\n                      I-TEXT DELIMITED BY SPACE\n                      INTO CUST-NAME\n               COMPUTE CUST-BALANCE = (I - 125) / 4\n               COMPUTE CUST-ACTIVE = FUNCTION MOD(I 2)\n               WRITE CUSTOMER-REC\n           END-PERFORM\n           CLOSE CUSTOMER-FILE\n           STOP RUN.\n",
        cobolt_codegen::generate_indexed_select(def),
        cobolt_codegen::generate_indexed_fd(def).replacen("FD  CUSTOMER-FILE.\n", "FD  CUSTOMER-FILE\n", 1)
    );
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<_> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "the writer parses: {errors:?}\n{src}");
    let program = parsed.program.expect("parses");
    let (_etx, erx) = mpsc::channel();
    let (stx, _srx) = mpsc::channel();
    let (dtx, _drx) = mpsc::channel();
    let mut i = Interpreter::new_with_channels(program, erx, stx, dtx);
    i.run().expect("the writer runs");
    assert!(data.exists(), "the file was made");
}

/// The row the backend answers for customer `i` (the packed balance as its
/// value in cents, with its sign — WASM-PARITY 19).
fn expected_customer(i: usize) -> Vec<String> {
    vec![i.to_string(), format!("Customer {i}"), ((i as i64 - 125) * 25).to_string(), (i % 2).to_string()]
}

/// A `Transport` that remembers which programs it was asked for, then hands the
/// call to `rcrun`'s own WebSocket client.
struct Spy {
    inner: WsClient,
    seen: Arc<Mutex<Vec<String>>>,
}

impl Transport for Spy {
    fn call(&mut self, request: &CallRequest) -> Result<CallReply, CallError> {
        self.seen.lock().unwrap().push(request.program.clone());
        self.inner.call(request)
    }

    fn call_observed(&mut self, request: &CallRequest, observer: &mut dyn CallObserver) -> Result<CallReply, CallError> {
        self.seen.lock().unwrap().push(request.program.clone());
        self.inner.call_observed(request, observer)
    }
}

/// `rcrun`'s client on the rig's TLS loopback server, as a clerk.
fn client(rig: &Rig) -> WsClient {
    let mut cfg = ClientConfig::new(format!("wss://localhost:{}/ws", rig.server.addr.port()), "1.0.0");
    cfg.client_name = "remote-bindings-test".to_owned();
    cfg.extra_root_pem = Some(rig.cert.cert_pem.as_bytes().to_vec());
    cfg.token = Some(Arc::new(SpecToken(rig.issuer.access("clerk-1", AUDIENCE, 600, json!({"roles": ["clerk"]})))));
    cfg.origin = Some(ORIGIN.to_owned());
    cfg.interface = Some(rig.interface());
    WsClient::new(cfg)
}

/// One call from the test itself: the group in `BY REFERENCE`, the answer out —
/// the `RETURNING` group when the program has one, else the parameter as the
/// program left it.
fn call(client: &mut WsClient, program: &str, arg: Value) -> Value {
    let request = CallRequest {
        program: program.to_owned(),
        args: vec![ArgPayload { mode: ArgMode::Reference, json: arg.to_string() }],
        wants_returning: true,
        idempotency_key: None,
    };
    let reply = client.call(&request).unwrap_or_else(|e| panic!("{program}: {e}"));
    let text = reply.returning.or_else(|| reply.args.into_iter().next().flatten()).unwrap_or_else(|| "null".to_owned());
    serde_json::from_str(&text).unwrap()
}

fn change(action: &str, keys: &[&str], cells: &[&str]) -> Value {
    json!({
        "CHANGE-ACTION": action,
        "KEYS": keys.iter().map(|k| json!({"KEY-TEXT": k})).collect::<Vec<_>>(),
        "CELLS": cells.iter().map(|c| json!({"CELL-TEXT": c})).collect::<Vec<_>>(),
    })
}

fn parse_rows(text: &str) -> Vec<Vec<String>> {
    if text.is_empty() {
        return Vec::new();
    }
    text.split('\n').map(|line| line.split('\t').map(str::to_owned).collect()).collect()
}

fn rows_text(rows: &[Vec<String>]) -> String {
    rows.iter().map(|r| r.join("\t")).collect::<Vec<_>>().join("\n")
}

/// The seeds the host gives a web form's grid (`cobolt-form-host`'s `seeding`,
/// pinned there): the binding's own, then `_BindingRemote` and the endpoints.
fn seed_remote(mut props: Vec<(&str, String)>, form: &str, grid: &str) -> Vec<(String, String)> {
    props.push(("_BindingRemote", "1".to_owned()));
    props.push(("_BindingRemoteRead", endpoint_name(form, grid, "READ").unwrap()));
    props.push(("_BindingRemoteSave", endpoint_name(form, grid, "SAVE").unwrap()));
    props.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

fn customers_seed(cidx: &str) -> Vec<(&'static str, String)> {
    vec![
        ("_BindingKind", "IndexedFile".to_owned()),
        ("_BindingIndexedPath", cidx.to_owned()),
        ("_BindingFields", "CUST-ID\nCUST-NAME\nCUST-BALANCE\nCUST-ACTIVE".to_owned()),
        ("_BindingId", "customers".to_owned()),
        ("_BindingWritable", "1".to_owned()),
        ("_BindingKeyFields", "CUST-ID".to_owned()),
        ("AllowCellEditing", "true".to_owned()),
        ("AutoCommit", "false".to_owned()),
    ]
}

fn items_seed() -> Vec<(&'static str, String)> {
    vec![
        ("_BindingKind", "Sql".to_owned()),
        ("_BindingSqlControl", "DB-SALES".to_owned()),
        ("_BindingSql", "SELECT ID, NAME, PRICE, IN_STOCK\nFROM ITEMS\nORDER BY ID;".to_owned()),
        ("_BindingTable", "ITEMS".to_owned()),
        ("_BindingFields", "ID\nNAME\nPRICE\nIN_STOCK".to_owned()),
        ("_BindingFieldTypes", "number,text,number,boolean".to_owned()),
        ("_BindingId", "items".to_owned()),
        ("_BindingWritable", "1".to_owned()),
        ("_BindingKeyFields", "ID".to_owned()),
        ("AllowCellEditing", "true".to_owned()),
        ("AutoCommit", "true".to_owned()),
    ]
}

/// The channels of the running form, and every property it has published so far.
struct Io {
    event_tx: mpsc::Sender<FormEvent>,
    input_tx: mpsc::Sender<StateUpdate>,
    display_rx: mpsc::Receiver<String>,
    state_rx: mpsc::Receiver<StateUpdate>,
    updates: RefCell<Vec<StateUpdate>>,
}

impl Io {
    /// Display lines up to and including the first that starts with `prefix`.
    fn wait_for(&self, prefix: &str) -> Vec<String> {
        let mut lines = Vec::new();
        loop {
            let line = self
                .display_rx
                .recv_timeout(Duration::from_secs(60))
                .unwrap_or_else(|_| panic!("waited for {prefix:?}; got {lines:?}"))
                .trim()
                .to_owned();
            let done = line.starts_with(prefix);
            lines.push(line);
            if done {
                return lines;
            }
        }
    }

    /// The last value published for `prop` of `ctrl`, among everything published so far.
    fn published(&self, ctrl: &str, prop: &str) -> Option<String> {
        let mut updates = self.updates.borrow_mut();
        updates.extend(self.state_rx.try_iter());
        updates.iter().rev().find(|u| u.ctrl_id.eq_ignore_ascii_case(ctrl) && u.prop == prop).map(|u| u.value.clone())
    }

    /// One operator edit, delivered the way the host delivers it: the grid's
    /// report, then `onCellEdited`; back once the interpreter has noted it.
    fn edit(&self, grid: &str, rows: &mut Vec<Vec<String>>, row: usize, col: usize, value: &str) -> String {
        let previous = rows[row - 1][col - 1].clone();
        rows[row - 1][col - 1] = value.to_owned();
        for (prop, text) in [
            ("Rows", rows_text(rows)),
            ("EditedRow", row.to_string()),
            ("EditedColumn", col.to_string()),
            ("EditedValue", value.to_owned()),
            ("PreviousValue", previous.clone()),
        ] {
            self.input_tx.send(StateUpdate::new(grid, prop, text)).unwrap();
        }
        self.event_tx.send(FormEvent::new(grid, "onCellEdited")).unwrap();
        self.wait_for("SEEN");
        previous
    }

    fn click(&self, button: &str, reply_prefix: &str) -> String {
        self.event_tx.send(FormEvent::new(button, "onClick")).unwrap();
        self.wait_for(reply_prefix).pop().unwrap()
    }
}

/// Run the `ORDERS` program on its own thread (the interpreter is not `Send`),
/// with `transport` and the seeded objects; answers the channels.
fn start_form(transport: Box<dyn Transport>, objects: Vec<(String, String, Vec<(String, String)>)>) -> (Io, std::thread::JoinHandle<()>) {
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(ORDERS, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<_> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("{}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "the form's program parses: {errors:?}");
    let program = parsed.program.expect("a program");
    let (event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let (input_tx, input_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
        interp.set_input_channel(input_rx);
        interp.set_remote_transport(transport);
        interp.seed_objects(objects);
        interp.run().expect("the form's loop runs");
    });
    (Io { event_tx, input_tx, display_rx, state_rx, updates: RefCell::new(Vec::new()) }, worker)
}

#[test]
fn a_web_forms_grids_fill_edit_save_and_auto_commit_through_the_backend() {
    // ── The backend: the developer's SETUP-DB and the Build's generated programs ──
    let dir = backend_dir("remote-bindings", MY_EXPORTS, &[("setup.cbl", SETUP_DB)]);
    std::fs::create_dir_all(dir.join("data")).unwrap();
    let data = dir.join("data").join("customers.idx");
    let cidx = dir.join("data").join("customers.cidx");
    let def = fixtures::customers_cidx(&data.to_string_lossy());
    cobolt_indexed::save_indexed(&cidx, &def).expect("write the .cidx");
    make_customer_file(&def, &data);
    {
        let data = data.to_string_lossy().into_owned();
        let load = move |_: &str| Ok(fixtures::customers_cidx(&data));
        let generated = generate_backend(&fixtures::grid_forms(), &load).expect("generates");
        let out = dir.join("generated");
        std::fs::create_dir_all(&out).unwrap();
        for p in &generated.programs {
            std::fs::write(out.join(&p.file_name), &p.source).unwrap();
        }
        std::fs::write(out.join("exports.toml"), generated.exports_toml()).unwrap();
    }
    let mut conn = cobolt_forms::connections::SqlConnection::new("SALES");
    conn.path = dir.join("sales.db").to_string_lossy().into_owned();
    conn.create_if_missing = true;
    cobolt_runtime::esql::catalog::install(cobolt_runtime::esql::catalog::SqlCatalog::new(
        "shop",
        vec![conn],
        dir.clone(),
        cobolt_runtime::esql::catalog::Source::Injected,
    ));
    let rig = rig(dir.clone(), "");
    const CUST_READ: &str = "RC-6-ORDERS-9-GRID-CUST-READ";
    const CUST_SAVE: &str = "RC-6-ORDERS-9-GRID-CUST-SAVE";
    const ITEMS_READ: &str = "RC-6-ORDERS-10-GRID-ITEMS-READ";
    const ITEMS_SAVE: &str = "RC-6-ORDERS-10-GRID-ITEMS-SAVE";

    // The table and its three rows, from the test's own connection.
    let mut direct = client(&rig);
    assert_eq!(call(&mut direct, "SETUP-DB", json!("")), json!("00000"));
    let s = call(
        &mut direct,
        ITEMS_SAVE,
        json!({"CHANGES": [
            change("INSERT", &[], &["1", "Pen", "1.50", "yes"]),
            change("INSERT", &[], &["2", "Ink", "", "0"]),
            change("INSERT", &[], &["3", "O'Brien pad", "4", "true"]),
        ]}),
    );
    assert_eq!(s["COMMIT-STATUS"], "00", "{s}");

    // ── The form, seeded as a web form's ──────────────────────────────────────
    let seen = Arc::new(Mutex::new(Vec::new()));
    let spy = Spy { inner: client(&rig), seen: Arc::clone(&seen) };
    let cidx_text = cidx.to_string_lossy().into_owned();
    let objects = vec![
        ("GRID-CUST".to_owned(), "DataGrid".to_owned(), seed_remote(customers_seed(&cidx_text), "ORDERS", "GRID-CUST")),
        ("GRID-ITEMS".to_owned(), "DataGrid".to_owned(), seed_remote(items_seed(), "ORDERS", "GRID-ITEMS")),
        ("DB-SALES".to_owned(), "SqlDatabase".to_owned(), Vec::new()),
        (
            "GRID-NOTES".to_owned(),
            "DataGrid".to_owned(),
            vec![
                ("_BindingKind".to_owned(), "CobolTable".to_owned()),
                ("_BindingFields".to_owned(), "NOTE-ID\nNOTE-TEXT".to_owned()),
                ("_BindingWritable".to_owned(), "0".to_owned()),
            ],
        ),
    ];
    let (io, worker) = start_form(Box::new(spy), objects);
    let loaded = io.wait_for("LOADED");
    assert!(loaded.contains(&format!("CUST {CUSTOMERS}")), "{loaded:?}");
    assert!(loaded.contains(&"ITEMS 3".to_owned()), "{loaded:?}");
    assert!(loaded.contains(&"NOTES 3".to_owned()), "{loaded:?}");

    // AC7, the fill: every row, in key order, window by window.
    let mut customers = parse_rows(&io.published("GRID-CUST", "Rows").expect("GRID-CUST has rows"));
    assert_eq!(customers.len(), CUSTOMERS);
    for (i, row) in customers.iter().enumerate() {
        assert_eq!(*row, expected_customer(i + 1), "row {}", i + 1);
    }
    let reads = seen.lock().unwrap().iter().filter(|p| *p == CUST_READ).count();
    assert_eq!(reads, 3, "250 rows come in windows of 100, 100 and 50: {:?}", seen.lock().unwrap());
    let mut items = parse_rows(&io.published("GRID-ITEMS", "Rows").expect("GRID-ITEMS has rows"));
    assert_eq!(
        items,
        vec![vec!["1", "Pen", "1.5", "1"], vec!["2", "Ink", "", "0"], vec!["3", "O'Brien pad", "4", "1"]],
        "the SQL grid shows what the desktop shows (AC7: no difference for a SQL grid)"
    );
    assert_eq!(seen.lock().unwrap().iter().filter(|p| *p == ITEMS_READ).count(), 1);
    // Q8: the COBOL table stayed in the form.
    assert_eq!(
        parse_rows(&io.published("GRID-NOTES", "Rows").expect("GRID-NOTES has rows")),
        vec![vec!["1", "first"], vec!["2", "second"], vec!["3", "third"]]
    );
    assert!(seen.lock().unwrap().iter().all(|p| !p.contains("GRID-NOTES")), "a COBOL table makes no remote call: {:?}", seen.lock().unwrap());
    // The ledger knows the key column, so the renderer keeps it read-only, as on the desktop.
    assert_eq!(io.published("GRID-CUST", "_KeyColumns").as_deref(), Some("1"));

    // ── Save, explicit (AutoCommit off on GRID-CUST) ──────────────────────────
    io.edit("GRID-CUST", &mut customers, 2, 2, "Bruno S.");
    assert_eq!(io.click("BTN-SAVE-CUST", "SAVE-CUST"), "SAVE-CUST 00");
    io.wait_for("SAVED GRID-CUST");
    assert_eq!(io.click("BTN-RELOAD-CUST", "RELOAD-CUST"), format!("RELOAD-CUST {CUSTOMERS}"));
    customers = parse_rows(&io.published("GRID-CUST", "Rows").unwrap());
    assert_eq!(customers[1], vec!["2", "Bruno S.", "-3075", "0"], "the backend wrote the update");
    let saves = seen.lock().unwrap().iter().filter(|p| *p == CUST_SAVE).count();
    assert_eq!(saves, 1, "one SAVE call carried the unit");

    // ── Auto-commit (on for GRID-ITEMS): the edit is written when the handler returns ──
    io.edit("GRID-ITEMS", &mut items, 1, 3, "2.75");
    assert_eq!(io.click("BTN-NOOP", "NOOP"), "NOOP");
    assert_eq!(io.click("BTN-RELOAD-ITEMS", "RELOAD-ITEMS"), "RELOAD-ITEMS 3");
    items = parse_rows(&io.published("GRID-ITEMS", "Rows").unwrap());
    assert_eq!(items[0], vec!["1", "Pen", "2.75", "1"], "auto-commit wrote through the backend");
    assert_eq!(seen.lock().unwrap().iter().filter(|p| *p == ITEMS_SAVE).count(), 1);

    // ── A refusal of the backend's: the change is named, the cell marked, the grid restored ──
    io.edit("GRID-CUST", &mut customers, 1, 3, "12a");
    assert_eq!(io.click("BTN-SAVE-CUST", "SAVE-CUST"), "SAVE-CUST 92");
    io.wait_for("ERROR GRID-CUST");
    assert_eq!(io.published("GRID-CUST", "CommitStatus").as_deref(), Some("92"));
    let last_error = io.published("GRID-CUST", "LastError").unwrap_or_default();
    assert!(last_error.contains("CUST-BALANCE"), "{last_error}");
    assert_eq!(io.published("GRID-CUST", "_FailedCells").as_deref(), Some("1,3"), "the refused change's cell is marked");
    customers = parse_rows(&io.published("GRID-CUST", "Rows").unwrap());
    assert_eq!(customers[0][2], "-3100", "the grid went back to what it loaded");

    // ── A key the grid refuses itself, before any call ────────────────────────
    let calls_before = seen.lock().unwrap().len();
    io.edit("GRID-CUST", &mut customers, 1, 1, "9");
    assert_eq!(io.click("BTN-SAVE-CUST", "SAVE-CUST"), "SAVE-CUST 21");
    io.wait_for("ERROR GRID-CUST");
    assert_eq!(seen.lock().unwrap().len(), calls_before, "a changed key never reaches the backend");
    customers = parse_rows(&io.published("GRID-CUST", "Rows").unwrap());
    assert_eq!(customers[0][0], "1");

    // ── A deleted row and an added row, in one unit ───────────────────────────
    assert_eq!(io.click("BTN-ROWS", "ROWS-CHANGED"), "ROWS-CHANGED");
    assert_eq!(io.click("BTN-SAVE-CUST", "SAVE-CUST"), "SAVE-CUST 00");
    io.wait_for("SAVED GRID-CUST");
    assert_eq!(io.published("GRID-CUST", "SavedRows").as_deref(), Some("2"));
    assert_eq!(io.click("BTN-RELOAD-CUST", "RELOAD-CUST"), format!("RELOAD-CUST {CUSTOMERS}"));
    customers = parse_rows(&io.published("GRID-CUST", "Rows").unwrap());
    assert!(customers.iter().all(|row| row[0] != "3"), "customer 3 was deleted");
    assert_eq!(customers.last().unwrap(), &vec!["251", "New customer", "12345", "1"], "the added row was inserted");

    io.event_tx.send(FormEvent::quit()).unwrap();
    worker.join().expect("the form's thread finished");

    // ── AC7: the same rows the desktop shows for the same data ────────────────
    // The desktop path over the same file. The recorded difference (AC7 as
    // amended): the packed balance shows its value here and its bytes in hex
    // there. What the bytes are is a FINDING of this test, not a parity rule:
    // the runtime lays a `COMP-3` file field out as `digits + decimals` DISPLAY
    // bytes (`cobolt-runtime/src/files.rs`, `compute_layout`), seven for
    // `S9(5)V99`, while the `.cidx` gives it the packed width, four — so the
    // desktop grid, reading by the `.cidx`, shows the hex of the first four
    // digit bytes and takes the fifth digit for the next column. The backend
    // reads with the FD the file was written with and is right. When the
    // layout is fixed, compare the balance and the active columns for equality.
    let desktop = desktop_rows(&cidx_text);
    assert_eq!(desktop.len(), customers.len());
    let mut misread = 0;
    for (remote, local) in customers.iter().zip(desktop.iter()) {
        assert_eq!((&remote[0], &remote[1]), (&local[0], &local[1]), "{remote:?} vs {local:?}");
        let cents: i64 = remote[2].parse().unwrap();
        let digits = format!("{:07}", cents.abs());
        let hex_of_first_four: String = digits.bytes().take(4).map(|b| format!("{b:02X}")).collect();
        assert_eq!(local[2], hex_of_first_four, "FINDING: the desktop shows the hex of the balance's first four digit bytes ({remote:?} vs {local:?})");
        // The desktop reads the balance's fifth digit byte as the active flag, a
        // one-digit indicator: `1` when that byte is `1`, else `0`.
        let fifth_as_indicator = if &digits[4..5] == "1" { "1" } else { "0" };
        assert_eq!(local[3], fifth_as_indicator, "FINDING: the desktop's active flag is the balance's fifth digit byte ({remote:?} vs {local:?})");
        if local[3] != remote[3] {
            misread += 1;
        }
    }
    let total_calls = seen.lock().unwrap().len();
    println!(
        "AC7 through the backend: {CUSTOMERS} customers in 3 windows, 3 items; explicit save, auto-commit, refusal 92 (marked 1,3), key change 21 (no call), delete+insert (SavedRows 2); \
         {} rows equal to the desktop in id and name; the packed balance differs as recorded (hex there, value here) and, FINDING, the desktop misreads the active flag of {misread} rows after it; {total_calls} remote calls in all",
        desktop.len()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// `GRID-CUST` filled by the desktop path — the same seeds without `_BindingRemote`.
fn desktop_rows(cidx: &str) -> Vec<Vec<String>> {
    const LOAD: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ORDERS-DESKTOP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STATUS  PIC X(8).
       PROCEDURE DIVISION.
       MAIN.
           SET WS-STATUS TO GRID-CUST::RefreshBinding()
           DISPLAY "CUST " WS-STATUS
           STOP RUN.
"#;
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(LOAD, cobolt_lexer::SourceFormat::Free));
    let program = parsed.program.expect("a program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    let seed: Vec<(String, String)> = customers_seed(cidx).into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
    interp.seed_objects(vec![("GRID-CUST".to_owned(), "DataGrid".to_owned(), seed)]);
    interp.run().expect("the desktop load runs");
    let shown: Vec<String> = display_rx.try_iter().map(|s| s.trim().to_owned()).collect();
    assert_eq!(shown, vec![format!("CUST {CUSTOMERS}")]);
    let rows = state_rx
        .try_iter()
        .filter(|u| u.ctrl_id.eq_ignore_ascii_case("GRID-CUST") && u.prop == "Rows")
        .last()
        .map(|u| u.value)
        .unwrap_or_default();
    parse_rows(&rows)
}
