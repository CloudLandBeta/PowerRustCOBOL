// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-005 T16: a DataGrid bound to an indexed file on a web form fills and saves through the
//! backend by the **player's own** calls — ids of the grids' range, never the module's; the
//! generated `…-READ` asked window by window; one `…-SAVE` for the unit — with the real host behind
//! the real fixture module, and the connection's answers handed back as the page hands them.

mod common;

use cobolt_forms::model::{
    BindingDataType, BindingField, BindingMode, BindingSourceDescriptor, BindingTargetDescriptor, Control, ControlType,
    DataBindingDef,
};
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::fixture::{build, standard_cfrm, standard_spec};
use cobolt_web_player::grid::{is_grid_id, ID_BASE};
use cobolt_web_player::host::HostOptions;
use cobolt_web_player::net;
use common::Rig;
use serde_json::{json, Value};

const GRID: &str = "GRD-R";

/// The standard fixture, its main form carrying one more control: a writable DataGrid bound to an
/// indexed file (`CUST-ID` the key, `CUST-NAME`).
fn rig_with_remote_grid() -> (Rig, u32, String) {
    let fixture = build(&standard_spec(true));
    let main = fixture.manifest.forms[0].name.clone();
    let cfrms: Vec<String> = fixture.manifest.forms.iter().map(|f| standard_cfrm(&fixture, &f.name)).collect();
    let mut rig = Rig::with_forms(fixture, &cfrms, true, HostOptions::default());
    let mut form = load_form_from_str(&cfrms[0]).expect("the form loads");
    form.controls.push(Control::new(GRID, ControlType::DataGrid, 10, 300));
    let mut binding = DataBindingDef::new(
        "customers",
        "Customers",
        BindingSourceDescriptor::IndexedFile {
            definition_path: "indexed/customers.cidx".into(),
            record_name: "CUSTOMER-REC".into(),
            fields: vec![
                BindingField::new("CUST-ID", BindingDataType::Integer).key(),
                BindingField::new("CUST-NAME", BindingDataType::Text),
            ],
            key_field: Some("CUST-ID".into()),
            writable: true,
        },
        BindingTargetDescriptor::DataGrid { control_id: GRID.into() },
    );
    binding.mode = BindingMode::Writable;
    form.data_bindings.push(binding);
    rig.host.borrow_mut().add_form(form).expect("the form is the fixture's");
    rig.start();
    let inst = *rig.host.borrow().trees.keys().next().expect("the main form opened");
    (rig, inst, main)
}

fn invoke(rig: &Rig, inst: u32, method: &str, args: &[&str]) -> Option<String> {
    rig.host.borrow_mut().invoke(inst, GRID, method, args).unwrap_or_else(|e| panic!("{method}: {e}"))
}

/// The one request the grid sent: its id (of the grids' range) and its frame.
fn one_request(rig: &mut Rig) -> (u32, Value) {
    let mut reqs = rig.take_requests();
    assert_eq!(reqs.len(), 1, "one request: {reqs:?}");
    let (id, frame) = reqs.pop().unwrap();
    assert!(is_grid_id(id), "a grid's id: {id:#x}");
    (id, serde_json::from_slice(&frame).unwrap())
}

fn rows_answer(id: u32, offset: usize, count: usize, total: usize) -> Vec<u8> {
    let rows: Vec<Value> = (offset + 1..=offset + count)
        .map(|i| json!({"CELL-COUNT": 2, "CELLS": [{"CELL-TEXT": i.to_string()}, {"CELL-TEXT": format!("Customer {i}")}]}))
        .collect();
    json!({"id": id, "result": {"args": [null], "returning": {
        "READ-STATUS": "00", "READ-MESSAGE": "", "ROW-TOTAL": total, "ROWS-RETURNED": count, "ROWS": rows
    }}})
    .to_string()
    .into_bytes()
}

fn save_answer(id: u32, status: &str, message: &str, failed: u32) -> Vec<u8> {
    json!({"id": id, "result": {"args": [null], "returning": {
        "COMMIT-STATUS": status, "COMMIT-MESSAGE": message, "FAILED-CHANGE": failed
    }}})
    .to_string()
    .into_bytes()
}

#[test]
fn a_remote_grid_fills_window_by_window_and_saves_its_unit_through_the_players_own_calls() {
    let (mut rig, inst, main) = rig_with_remote_grid();
    let ctl = rig.control(inst, GRID);
    assert!(rig.host.borrow().is_remote_grid(inst, ctl));
    let read = cobolt_wire::controls::endpoint_name(&main, GRID, "READ").unwrap();
    let save = cobolt_wire::controls::endpoint_name(&main, GRID, "SAVE").unwrap();

    // RefreshBinding(): answers the count shown now, and asks the backend for the first window.
    assert_eq!(invoke(&rig, inst, "RefreshBinding", &[]).as_deref(), Some("0"));
    let (id1, frame) = one_request(&mut rig);
    assert_eq!(frame["program"], read);
    assert_eq!(frame["args"][0]["mode"], "value");
    assert_eq!(frame["args"][0]["json"]["ROW-OFFSET"], 0);
    assert_eq!(frame["args"][0]["json"]["ROW-COUNT"], 100);
    assert_eq!(frame["returning"], true);
    // A second refresh while one is on its way joins it: no second request.
    assert_eq!(invoke(&rig, inst, "RefreshBinding", &[]).as_deref(), Some("0"));
    assert!(rig.take_requests().is_empty());
    // 150 rows: a full window, then a short one.
    rig.host.borrow_mut().grid_answer(id1, rows_answer(id1, 0, 100, 150));
    let (id2, frame) = one_request(&mut rig);
    assert!(id2 > id1 && is_grid_id(id2), "ids go up within the range");
    assert_eq!(frame["args"][0]["json"]["ROW-OFFSET"], 100);
    rig.host.borrow_mut().grid_answer(id2, rows_answer(id2, 100, 50, 150));
    assert!(rig.take_requests().is_empty(), "the short window ended the refresh");
    let rows = cobolt_forms::datagrid::parse_rows(&rig.get(inst, GRID, "Rows"));
    assert_eq!(rows.len(), 150);
    assert_eq!(rows[1], vec!["2", "Customer 2"]);
    assert_eq!(rows[149], vec!["150", "Customer 150"]);
    assert_eq!(invoke(&rig, inst, "RefreshBinding", &[]).as_deref(), Some("150"), "the count the grid shows now");
    let (id3, _) = one_request(&mut rig);
    rig.host.borrow_mut().grid_answer(id3, rows_answer(id3, 0, 100, 150));
    let (id4, _) = one_request(&mut rig);
    rig.host.borrow_mut().grid_answer(id4, rows_answer(id4, 100, 50, 150));

    // Save(): the program changed a cell, deleted a row and added one; one unit, in the SAVE's order.
    invoke(&rig, inst, "SetCellValue", &["2", "2", "Bruno S."]);
    invoke(&rig, inst, "DeleteRow", &["3"]);
    invoke(&rig, inst, "AddRow", &[]);
    invoke(&rig, inst, "SetCellValue", &["150", "1", "151"]);
    invoke(&rig, inst, "SetCellValue", &["150", "2", "New customer"]);
    assert_eq!(invoke(&rig, inst, "Save", &[]).as_deref(), Some("98"), "pending until the backend answers");
    assert_eq!(rig.get(inst, GRID, "CommitStatus"), "98");
    let (id5, frame) = one_request(&mut rig);
    assert_eq!(frame["program"], save);
    let changes = frame["args"][0]["json"]["CHANGES"].as_array().unwrap().clone();
    let actions: Vec<&str> = changes.iter().map(|c| c["CHANGE-ACTION"].as_str().unwrap()).collect();
    assert_eq!(actions, vec!["DELETE", "UPDATE", "INSERT"]);
    assert_eq!(changes[0]["KEYS"][0]["KEY-TEXT"], "3");
    assert_eq!(changes[1]["KEYS"][0]["KEY-TEXT"], "2");
    assert_eq!(changes[1]["CELLS"], json!([{"CELL-TEXT": "2"}, {"CELL-TEXT": "Bruno S."}]));
    assert_eq!(changes[2]["CELLS"], json!([{"CELL-TEXT": "151"}, {"CELL-TEXT": "New customer"}]));
    assert_eq!(invoke(&rig, inst, "Save", &[]).as_deref(), Some("98"), "a second Save() while one is on its way waits");
    assert!(rig.take_requests().is_empty());
    rig.host.borrow_mut().grid_answer(id5, save_answer(id5, "00", "", 0));
    assert_eq!(rig.get(inst, GRID, "CommitStatus"), "00");
    assert_eq!(rig.get(inst, GRID, "SavedRows"), "3");
    assert_eq!(invoke(&rig, inst, "Save", &[]).as_deref(), Some("00"), "nothing left to save");
    assert!(rig.take_requests().is_empty());

    // A refusal of the backend's: the change and the field it names mark the cell, the grid goes back.
    invoke(&rig, inst, "SetCellValue", &["1", "2", "a name the PICTURE cannot hold"]);
    assert_eq!(invoke(&rig, inst, "Save", &[]).as_deref(), Some("98"));
    let (id6, _) = one_request(&mut rig);
    rig.host.borrow_mut().grid_answer(id6, save_answer(id6, "92", "CUST-NAME: value length exceeds PIC", 1));
    assert_eq!(rig.get(inst, GRID, "CommitStatus"), "92");
    assert_eq!(rig.get(inst, GRID, "LastError"), "CUST-NAME: value length exceeds PIC");
    assert_eq!(rig.get(inst, GRID, "_FailedCells"), "1,2");
    let rows = cobolt_forms::datagrid::parse_rows(&rig.get(inst, GRID, "Rows"));
    assert_eq!(rows[0], vec!["1", "Customer 1"], "the grid went back to what was saved");
    assert_eq!(rows[1], vec!["2", "Bruno S."]);

    // A backend not reached: the changes stay for the next Save().
    invoke(&rig, inst, "SetCellValue", &["1", "2", "Ana"]);
    assert_eq!(invoke(&rig, inst, "Save", &[]).as_deref(), Some("98"));
    let (id7, _) = one_request(&mut rig);
    let gone = net::error_frame(id7, net::class::TRANSPORT, 7, "the application is not connected to its backend", false);
    rig.host.borrow_mut().grid_answer(id7, gone);
    assert_eq!(rig.get(inst, GRID, "CommitStatus"), "90");
    let rows = cobolt_forms::datagrid::parse_rows(&rig.get(inst, GRID, "Rows"));
    assert_eq!(rows[0], vec!["1", "Ana"], "the change is still there");
    assert_eq!(invoke(&rig, inst, "Save", &[]).as_deref(), Some("98"), "and goes again");
    let (id8, frame) = one_request(&mut rig);
    assert_eq!(frame["args"][0]["json"]["CHANGES"].as_array().unwrap().len(), 1);
    rig.host.borrow_mut().grid_answer(id8, save_answer(id8, "00", "", 0));
    assert_eq!(rig.get(inst, GRID, "SavedRows"), "1");

    // DiscardEdit(): back to what was loaded or saved.
    invoke(&rig, inst, "SetCellValue", &["1", "2", "typo"]);
    assert_eq!(invoke(&rig, inst, "DiscardEdit", &[]).as_deref(), Some("00"));
    assert_eq!(cobolt_forms::datagrid::parse_rows(&rig.get(inst, GRID, "Rows"))[0], vec!["1", "Ana"]);

    // The ids: all of the grids' range, none the module's or the Authentication control's.
    for id in [id1, id2, id3, id4, id5, id6, id7, id8] {
        assert!(id >= ID_BASE && !cobolt_web_player::auth::wire::is_auth_id(id));
    }
    println!(
        "T16: refresh in 2 windows (100 + 50), a second refresh joined the first, save of DELETE+UPDATE+INSERT (SavedRows 3), refusal 92 marked 1,2 and restored, transport failure kept the change and the next Save() sent it, DiscardEdit; 8 calls, ids {id1:#x}..{id8:#x}"
    );
}
