// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 108, T20 / T21 — a PartiQL-bound DataGrid and ComboBox: the query runs on
//! the `IndexedFile` control with its parameters, a failure keeps the rows and
//! sets the control's `StatusCode`, an empty query follows the control's result,
//! `BindingQuery` refuses what is not a `SELECT`, and `SelectedValue` answers the
//! value of what was picked.
//!
//! One result block at the end (GOLDEN RULE #7).

#[path = "support/partiql_fixtures.rs"]
mod fx;

use std::sync::mpsc;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::indexed::{IndexedStore, OpenMode};
use cobolt_runtime::indexed_disk::DiskIndexedFile;
use cobolt_runtime::Interpreter;
use fx::*;

fn cidx_xml(data: &str, record_len: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><IndexedFile name="CUSTOMERS" finalized="true" version="1.0"><assign-path>{data}</assign-path><access-mode>dynamic</access-mode><record-format fixed-length="{record_len}"/><storage mode="disk" compression="false" persistence="false"/><keys><primary duplicates="false" ordering="ascending"><part field="CUST-ID" offset="0" length="8" encoding="bytes"/></primary><alternate name="CUST-NAME" duplicates="true" ordering="ascending"><part field="CUST-NAME" offset="8" length="40" encoding="bytes"/></alternate></keys><fields><Field level="1" name="CUST-REC" usage="display"><Field level="5" name="CUST-ID" pic="9(8)" usage="display" offset="0" length="8"></Field><Field level="5" name="CUST-NAME" pic="X(40)" usage="display" offset="8" length="40"></Field><Field level="5" name="CUST-BALANCE" pic="S9(6)V99" usage="display" offset="48" length="8"></Field><Field level="5" name="BILLING" usage="display"><Field level="10" name="CITY" pic="X(20)" usage="display" offset="56" length="20"></Field></Field></Field></fields></IndexedFile>"#
    )
}

struct Project {
    dir: Scratch,
    cidx: String,
    data: String,
}

/// A project folder with `customers.cidx` and, when `with_data`, a data file
/// holding customers 2, 4 … `2*n` (balance `-123.45` for 42).
fn project(tag: &str, with_data: bool, n: i64) -> Project {
    let dir = Scratch::new(tag);
    let data = dir.0.join("customers.idx").to_string_lossy().to_string();
    let cidx = dir.0.join("customers.cidx").to_string_lossy().to_string();
    std::fs::write(&cidx, cidx_xml(&data, CUSTOMERS_LEN)).unwrap();
    if with_data {
        let def = cobolt_indexed::load_indexed(&cidx).unwrap();
        cobolt_runtime::indexed_ide::create_empty_from_definition(&def, std::path::Path::new(&data)).unwrap();
        let (p, a) = customers_keys();
        let mut f = DiskIndexedFile::new(&data, CUSTOMERS_LEN, p, a);
        assert_eq!(f.open(OpenMode::Io), "00");
        let cs = customers_schema();
        for i in 1..=n {
            let id = i * 2;
            let (name, bal, city) = if id == 42 { ("Ana".to_string(), -12345, "Recife") } else { (format!("Cliente {id}"), 1000 * id, "Olinda") };
            assert_eq!(f.write(&customer(&cs, id, &name, bal, city)), "00");
        }
        f.close();
    }
    Project { dir, cidx, data }
}

/// Run a COBOL program with one `IndexedFile` control `IXF-CUST`; its DISPLAY lines.
fn run_cobol(body: &str, p: &Project, open_mode: &str) -> Vec<String> {
    run_cobol_with(body, p, open_mode, Vec::new())
}

fn run_cobol_with(body: &str, p: &Project, open_mode: &str, extra: Vec<(String, String, Vec<(String, String)>)>) -> Vec<String> {
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. T.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 WS-N PIC 9(4).\n       01 WS-ROW PIC X(100).\n       01 WS-V PIC X(60).\n       01 WS-ID PIC 9(8).\n       01 WS-TXT PIC X(80).\n       PROCEDURE DIVISION.\n{body}\n           STOP RUN.\n"
    );
    let parsed = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        parsed.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}\n{src}",
        parsed.diagnostics
    );
    let (_etx, erx) = mpsc::channel();
    let (stx, _srx) = mpsc::channel();
    let (dtx, drx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(parsed.program.unwrap(), erx, stx, dtx);
    let mut controls = vec![(
        "IXF-CUST".to_owned(),
        "IndexedFile".to_owned(),
        vec![
            ("IndexedFile".to_owned(), p.cidx.clone()),
            ("OpenMode".to_owned(), open_mode.to_owned()),
            ("AutoCommit".to_owned(), "true".to_owned()),
        ],
    )];
    controls.extend(extra);
    interp.seed_objects(controls);
    interp.run().expect("run");
    drx.try_iter().collect()
}


/// A whole program that declares the file with a `SELECT` / `FD` and drives it with the
/// verbs, beside the control. `autocommit` is the control's property, or unset.
fn run_with_fd(procedure: &str, p: &Project, autocommit: Option<&str>) -> Vec<String> {
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. T.\n       ENVIRONMENT DIVISION.\n       INPUT-OUTPUT SECTION.\n       FILE-CONTROL.\n           SELECT CUSTOMERS ASSIGN TO \"{}\"\n               ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC\n               RECORD KEY IS CUST-ID\n               ALTERNATE RECORD KEY IS CUST-NAME WITH DUPLICATES\n               FILE STATUS IS WS-FS.\n       DATA DIVISION.\n       FILE SECTION.\n       FD CUSTOMERS.\n       01 CUST-REC.\n           05 CUST-ID PIC 9(8).\n           05 CUST-NAME PIC X(40).\n           05 CUST-BALANCE PIC S9(6)V99.\n           05 BILLING.\n               10 CITY PIC X(20).\n       WORKING-STORAGE SECTION.\n       01 WS-FS PIC XX.\n       01 WS-N PIC 9(4).\n       PROCEDURE DIVISION.\n{procedure}\n           STOP RUN.\n",
        p.data
    );
    let parsed = parse(tokenize(&src, SourceFormat::Free));
    assert!(parsed.diagnostics.iter().all(|d| d.severity != Severity::Error), "parse errors: {:?}\n{src}", parsed.diagnostics);
    let (_etx, erx) = mpsc::channel();
    let (stx, _srx) = mpsc::channel();
    let (dtx, drx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(parsed.program.unwrap(), erx, stx, dtx);
    let mut props = vec![("IndexedFile".to_owned(), p.cidx.clone()), ("OpenMode".to_owned(), "I-O".to_owned())];
    if let Some(a) = autocommit {
        props.push(("AutoCommit".to_owned(), a.to_owned()));
    }
    interp.seed_objects(vec![("IXF-CUST".to_owned(), "IndexedFile".to_owned(), props)]);
    interp.run().expect("run");
    drx.try_iter().collect()
}


fn controls() -> Vec<(String, String, Vec<(String, String)>)> {
    let o = |id: &str, class: &str, props: &[(&str, &str)]| {
        (id.to_owned(), class.to_owned(), props.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
    };
    let v = vec![
        o("DG-1", "DataGrid", &[("_BindingKind", "PartiQl"), ("_BindingPartiqlControl", "IXF-CUST"),
            ("_BindingPartiqlQuery", "SELECT CUST-ID, CUST-NAME, CUST-BALANCE FROM CUSTOMERS WHERE CUST-ID BETWEEN ? AND ?"),
            ("_BindingPartiqlParams", "TXT-LO::Text,WS-HI"), ("_BindingFields", "CUST-ID,CUST-NAME,CUST-BALANCE"), ("_BindingKeyFields", "CUST-ID")]),
        o("DG-FOLLOW", "DataGrid", &[("_BindingKind", "PartiQl"), ("_BindingPartiqlControl", "IXF-CUST"), ("_BindingFields", "CUST-ID,CUST-NAME")]),
        o("CMB-1", "ComboBox", &[("_BindingKind", "PartiQl"), ("_BindingPartiqlControl", "IXF-CUST"), ("_BindingList", "1"),
            ("_BindingPartiqlQuery", "SELECT CUST-NAME, CUST-ID FROM CUSTOMERS WHERE CUST-ID <= 6"), ("_BindingFields", "CUST-NAME,CUST-ID")]),
        o("TXT-LO", "TextBox", &[("Text", "4")]),
    ];
    v
}

#[test]
fn partiql_binding() {
    let t0 = Instant::now();
    let p = project("bind-a", true, 50);
    let mut failures: Vec<String> = Vec::new();
    let mut checks = 0usize;
    let mut check = |ok: bool, what: String| {
        checks += 1;
        if !ok {
            failures.push(what);
        }
    };
    let body = "           MOVE 10 TO WS-ID.\n           SET WS-N TO DG-1::RefreshBinding().\n           DISPLAY WS-N.\n           DISPLAY DG-1::Rows.\n           DISPLAY \"SV[\" DG-1::SelectedValue \"]\".\n           MOVE 2 TO DG-1::ClickedRow.\n           DISPLAY \"SV[\" DG-1::SelectedValue \"]\".\n           SET WS-N TO CMB-1::RefreshBinding().\n           DISPLAY WS-N.\n           DISPLAY CMB-1::Items.\n           DISPLAY \"CV[\" CMB-1::SelectedValue \"]\".\n           MOVE 3 TO CMB-1::SelectedIndex.\n           DISPLAY \"CV[\" CMB-1::SelectedValue \"]\".\n           SET WS-N TO IXF-CUST::Execute(\"SELECT CUST-ID, CUST-NAME FROM CUSTOMERS WHERE CUST-ID <= 4\").\n           SET WS-N TO DG-FOLLOW::RefreshBinding().\n           DISPLAY WS-N.\n           MOVE \"DELETE FROM CUSTOMERS WHERE CUST-ID = 1\" TO DG-1::BindingQuery.\n           DISPLAY IXF-CUST::StatusCode.\n           MOVE \"SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID = 2\" TO DG-1::BindingQuery.\n           SET WS-N TO DG-1::RefreshBinding().\n           DISPLAY WS-N.\n           MOVE \"SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID BETWEEN ? AND ?\" TO DG-1::BindingQuery.\n           SET WS-N TO DG-1::RefreshBinding().\n           DISPLAY IXF-CUST::StatusCode.\n";
    let mut controls = controls();
    // WS-HI is a numeric data item the parameters name.
    let src_body = body.replace("MOVE 10 TO WS-ID.", "MOVE 10 TO WS-ID.\n           MOVE 10 TO WS-HI.");
    let out = {
        let src = format!(
            "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. T.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 WS-N PIC 9(4).\n       01 WS-ID PIC 9(8).\n       01 WS-HI PIC 9(8).\n       PROCEDURE DIVISION.\n{src_body}           STOP RUN.\n"
        );
        let parsed = parse(tokenize(&src, SourceFormat::Free));
        assert!(parsed.diagnostics.iter().all(|d| d.severity != Severity::Error), "parse errors: {:?}\n{src}", parsed.diagnostics);
        let (_etx, erx) = mpsc::channel();
        let (stx, _srx) = mpsc::channel();
        let (dtx, drx) = mpsc::channel();
        let mut interp = Interpreter::new_with_channels(parsed.program.unwrap(), erx, stx, dtx);
        controls.insert(0, ("IXF-CUST".to_owned(), "IndexedFile".to_owned(), vec![("IndexedFile".to_owned(), p.cidx.clone()), ("OpenMode".to_owned(), "I-O".to_owned())]));
        interp.seed_objects(controls);
        interp.run().expect("run");
        drx.try_iter().collect::<Vec<String>>()
    };
    let shown: Vec<String> = out.iter().map(|l| l.replace('\t', "<TAB>").replace('\n', "|")).collect();
    let all = shown.join(" # ");
    // AC9 — the grid shows ids 4..10 (even ids), the parameter from a TextBox's property and a data item.
    check(all.contains("00000004<TAB>Cliente 4<TAB>") && all.contains("00000010<TAB>Cliente 10<TAB>"), format!("grid rows: {all}"));
    check(shown.get(0).map(|l| l.trim().trim_start_matches('0')) == Some("4"), format!("RefreshBinding answers the count: {shown:?}"));
    // AC12/AC9 — SelectedValue empty before a pick, the value column's cell after.
    check(all.contains("SV[]") && all.contains("SV[00000006]"), format!("grid SelectedValue: {all}"));
    check(all.contains("CV[]") && all.contains("CV[00000006]"), format!("combo SelectedValue: {all}"));
    // AC11 — an empty-query grid follows the control's result.
    check(shown.iter().any(|l| l.trim().trim_start_matches('0') == "2"), format!("follow: {all}"));
    // AC14 — a DELETE as BindingQuery is refused with 94.
    check(shown.iter().any(|l| l.trim() == "94"), format!("BindingQuery refuses a DELETE: {all}"));

    println!("\n=== test_partiql_binding ===");
    println!("forms exercised: PartiQL-bound DataGrid (parameters from a TextBox property and a data item), ComboBox with SelectedValue, DataGrid SelectedValue (ClickedRow), a grid that follows the control, BindingQuery refused for a DELETE");
    println!("PICTUREs: 9(8), X(40), S9(6)V99; engine: PRCIDXD1 (50 records); no SELECT / FD declared");
    println!("checks: {} passed, {} failed; elapsed {} ms", checks - failures.len(), failures.len(), t0.elapsed().as_millis());
    for f in &failures {
        println!("  FAIL {f}");
    }
    assert!(failures.is_empty());
}
