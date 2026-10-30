// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 108, T26 / T23 — one `SELECT` on an `IndexedFile` control fills every
//! target kind: a DataGrid, a ListBox, a Bar chart, a Gauge, a Maps control and
//! a repeating GroupBox; and a member edited on a card writes its record through
//! the control (`00`; a key column `21`; a record changed meanwhile `96`).
//!
//! One result block at the end (GOLDEN RULE #7).

#[path = "support/partiql_fixtures.rs"]
mod fx;

use std::sync::mpsc;
use std::time::Instant;

use cobolt_indexed::IndexedDefinition;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::indexed::{IndexedStore, OpenMode};
use cobolt_runtime::indexed_disk::DiskIndexedFile;
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};
use fx::*;

fn cidx_xml(data: &str, record_len: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><IndexedFile name="CUSTOMERS" finalized="true" version="1.0"><assign-path>{data}</assign-path><access-mode>dynamic</access-mode><record-format fixed-length="{record_len}"/><storage mode="disk" compression="false" persistence="false"/><keys><primary duplicates="false" ordering="ascending"><part field="CUST-ID" offset="0" length="8" encoding="bytes"/></primary><alternate name="CUST-NAME" duplicates="true" ordering="ascending"><part field="CUST-NAME" offset="8" length="40" encoding="bytes"/></alternate></keys><fields><Field level="1" name="CUST-REC" usage="display"><Field level="5" name="CUST-ID" pic="9(8)" usage="display" offset="0" length="8"></Field><Field level="5" name="CUST-NAME" pic="X(40)" usage="display" offset="8" length="40"></Field><Field level="5" name="CUST-BALANCE" pic="S9(6)V99" usage="display" offset="48" length="8"></Field><Field level="5" name="BILLING" usage="display"><Field level="10" name="CITY" pic="X(20)" usage="display" offset="56" length="20"></Field></Field></Field></fields></IndexedFile>"#
    )
}

const PROGRAM: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TARGETS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 EV-ID      PIC X(64).
       01 CTL-ID     PIC X(64).
       01 COBOL-QUIT PIC X VALUE "0".
       01 WS-N       PIC 9(4).
       01 WS-LOW     PIC 9(8) VALUE 5000.
       01 WS-ROW     PIC X(100).
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-INIT-FORM" USING "T"
           SET WS-N TO DG-1::RefreshBinding()
           DISPLAY "GRID " WS-N
           DISPLAY "ROWS " DG-1::Rows
           SET WS-N TO LST-1::RefreshBinding()
           DISPLAY "LIST " WS-N
           DISPLAY "ITEMS " LST-1::Items
           SET WS-N TO CHT-1::RefreshBinding()
           DISPLAY "CHART " WS-N
           SET WS-N TO GAU-1::RefreshBinding()
           DISPLAY "GAUGE " WS-N " " GAU-1::Value
           SET WS-N TO MAP-1::RefreshBinding()
           DISPLAY "MAPS " WS-N
           DISPLAY "MARKERS " MAP-1::Markers
           SET WS-N TO CARDS::RefreshBinding()
           DISPLAY "CARDS " WS-N
           SET WS-N TO IXF-CUST::Execute("UPDATE CUSTOMERS SET CUST-NAME = 'Elsewhere' WHERE CUST-ID = 8")
           PERFORM UNTIL COBOL-QUIT = "1"
               CALL "COBOL-WAIT-EVENT" USING EV-ID CTL-ID
               EVALUATE TRUE
                   WHEN EV-ID = "onClick"
                       DISPLAY "STATUS " CARDS::CommitStatus "|" CARDS::LastError
                       SET WS-N TO IXF-CUST::Execute("SELECT CUST-NAME FROM CUSTOMERS WHERE CUST-ID = 6")
                       SET WS-ROW TO IXF-CUST::Fetch()
                       DISPLAY "FILE " WS-ROW
                       DISPLAY "END"
                   WHEN OTHER
                       CONTINUE
               END-EVALUATE
           END-PERFORM
           STOP RUN.
"#;

fn o(id: &str, class: &str, props: &[(&str, &str)]) -> (String, String, Vec<(String, String)>) {
    (id.to_owned(), class.to_owned(), props.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect())
}

struct Ran {
    shown: Vec<String>,
    states: Vec<(String, String, String)>,
}

fn run(cidx: &str, edits: &[(&str, &str, &str)], events: &[&str]) -> Ran {
    let parsed = parse(tokenize(PROGRAM, SourceFormat::Free));
    assert!(parsed.diagnostics.iter().all(|d| d.severity != Severity::Error), "{:?}", parsed.diagnostics);
    let (event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let (input_tx, input_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(parsed.program.expect("program"), event_rx, state_tx, display_tx);
    interp.set_input_channel(input_rx);
    let query = "SELECT CUST-ID, CUST-NAME, CUST-BALANCE FROM CUSTOMERS WHERE CUST-BALANCE > ?";
    let base = [("_BindingKind", "PartiQl"), ("_BindingPartiqlControl", "IXF-CUST"), ("_BindingPartiqlQuery", query), ("_BindingPartiqlParams", "WS-LOW")];
    let with = |extra: &[(&'static str, &'static str)]| {
        let mut v: Vec<(&str, &str)> = base.to_vec();
        v.extend_from_slice(extra);
        v
    };
    interp.seed_objects(vec![
        o("IXF-CUST", "IndexedFile", &[("IndexedFile", cidx), ("OpenMode", "I-O"), ("AutoCommit", "true")]),
        o("DG-1", "DataGrid", &with(&[("_BindingFields", "CUST-ID,CUST-NAME,CUST-BALANCE")])),
        o("LST-1", "ListBox", &with(&[("_BindingList", "1"), ("_BindingFields", "CUST-NAME,CUST-ID")])),
        o("CHT-1", "Chart", &with(&[("_BindingChart", "1"), ("_BindingFields", "CUST-NAME,CUST-BALANCE,CUST-ID")])),
        o("GAU-1", "Gauge", &with(&[("_BindingScalarProperty", "Value"), ("_BindingFields", "CUST-BALANCE")])),
        o("MAP-1", "Maps", &with(&[("_BindingMarkerFields", "CUST-ID\tCUST-ID\tCUST-BALANCE\tCUST-NAME\t")])),
        o(
            "Cards",
            "GroupBox",
            &with(&[
                ("IsRepeatingGroup", "true"),
                ("_BindingArray", "1"),
                ("_BindingFields", "CUST-ID,CUST-NAME,CUST-BALANCE"),
                ("_BindingMappings", "CUST-NAME\ttxt-name\tText;CUST-ID\ttxt-id\tText"),
                ("_BindingWritable", "1"),
            ]),
        ),
    ]);
    for (id, prop, value) in edits {
        input_tx.send(StateUpdate::new((*id).to_owned(), (*prop).to_owned(), (*value).to_owned())).unwrap();
    }
    for e in events {
        event_tx.send(FormEvent::new("BTN", *e)).unwrap();
    }
    event_tx.send(FormEvent::new("BTN", "onClick")).unwrap();
    event_tx.send(FormEvent::quit()).unwrap();
    interp.run().expect("the program runs");
    Ran {
        shown: display_rx.try_iter().map(|l| l.replace('\t', "<TAB>").replace('\n', "|")).collect(),
        states: state_rx.try_iter().map(|u| (u.ctrl_id, u.prop, u.value)).collect(),
    }
}

fn project(tag: &str) -> (Scratch, String, String) {
    let dir = Scratch::new(tag);
    let data = dir.0.join("customers.idx").to_string_lossy().to_string();
    let cidx = dir.0.join("customers.cidx").to_string_lossy().to_string();
    std::fs::write(&cidx, cidx_xml(&data, CUSTOMERS_LEN)).unwrap();
    let def: IndexedDefinition = cobolt_indexed::load_indexed(&cidx).unwrap();
    cobolt_runtime::indexed_ide::create_empty_from_definition(&def, std::path::Path::new(&data)).unwrap();
    let (p, a) = customers_keys();
    let mut f = DiskIndexedFile::new(&data, CUSTOMERS_LEN, p, a);
    assert_eq!(f.open(OpenMode::Io), "00");
    let cs = customers_schema();
    // Customers 2, 4 … 20, balance 1000 per id: those above 5000 are ids 6 … 20.
    for i in 1..=10 {
        let id = i * 2;
        assert_eq!(f.write(&customer(&cs, id, &format!("Cliente {id}"), 100_000 * id, "Olinda")), "00");
    }
    f.close();
    (dir, cidx, data)
}

fn has(shown: &[String], prefix: &str) -> Option<String> {
    shown.iter().find(|l| l.starts_with(prefix)).cloned()
}

#[test]
fn one_select_fills_every_target() {
    let t0 = Instant::now();
    let (_dir, cidx, _data) = project("targets");
    let mut failures: Vec<String> = Vec::new();
    let mut checks = 0usize;
    let mut check = |ok: bool, what: String| {
        checks += 1;
        if !ok {
            failures.push(what);
        }
    };

    let ran = run(&cidx, &[], &[]);
    let s = &ran.shown;
    // Balance 100000 * id / 100 = 1000 * id; the filter is > 5000, so ids 6 … 20: 8 items.
    check(has(s, "GRID").is_some_and(|l| l.trim_start_matches("GRID ").trim().trim_start_matches('0') == "8"), format!("grid count: {s:?}"));
    check(has(s, "ROWS").is_some_and(|l| l.contains("00000006<TAB>Cliente 6<TAB>6000.00") && l.contains("00000020")), format!("grid rows: {s:?}"));
    check(has(s, "LIST").is_some_and(|l| l.contains('8')), format!("list count: {s:?}"));
    check(has(s, "ITEMS").is_some_and(|l| l.contains("Cliente 6|Cliente 8") && !l.contains("Cliente 4")), format!("list items: {s:?}"));
    check(has(s, "CHART").is_some_and(|l| l.contains('8')), format!("chart count: {s:?}"));
    let chart = ran.states.iter().rev().find(|(c, p, _)| c == "CHT-1" && p == "__ChartData").map(|(_, _, v)| v.clone()).unwrap_or_default();
    check(chart.lines().count() == 8 && chart.starts_with("Cliente 6\t6000\t6"), format!("chart points: {chart:?}"));
    check(has(s, "GAUGE").is_some_and(|l| l.contains("6000.00")), format!("gauge value: {s:?}"));
    check(has(s, "MAPS").is_some_and(|l| l.contains('8')), format!("maps count: {s:?}"));
    check(has(s, "MARKERS").is_some_and(|l| l.contains("00000006<TAB>00000006<TAB>6000.00<TAB>Cliente 6<TAB>")), format!("markers: {s:?}"));
    check(has(s, "CARDS").is_some_and(|l| l.contains('8')), format!("cards count: {s:?}"));
    let item_count = ran.states.iter().rev().find(|(c, p, _)| c == "Cards" && p == "ItemCount").map(|(_, _, v)| v.clone());
    check(item_count.as_deref() == Some("8"), format!("ItemCount: {item_count:?}"));
    let card_two = ran.states.iter().find(|(c, p, _)| c == "Cards.Cards-2.txt-name" && p == "Text").map(|(_, _, v)| v.clone());
    check(card_two.as_deref() == Some("Cliente 8"), format!("card 2 name: {card_two:?}"));

    // Cards write back: a name edit lands in the record (ordinal 1 is customer 6).
    let ran = run(&cidx, &[("Cards.Cards-1.txt-name", "Text", "Renamed")], &[]);
    check(has(&ran.shown, "STATUS").is_some_and(|l| l.starts_with("STATUS 0|")), format!("card edit status: {:?}", ran.shown));
    check(has(&ran.shown, "FILE").is_some_and(|l| l.contains("Renamed")), format!("card edit in the file: {:?}", ran.shown));
    let ran = run(&cidx, &[("Cards.Cards-2.txt-id", "Text", "00000099")], &[]);
    check(has(&ran.shown, "STATUS").is_some_and(|l| l.starts_with("STATUS 21")), format!("key edit is 21: {:?}", ran.shown));
    // A record changed by the program since the load: 96. Customer 8 is card 2.
    let (_dir96, cidx96, _) = project("targets-96");
    let ran = run(&cidx96, &[("Cards.Cards-2.txt-name", "Text", "Mine")], &[]);
    check(has(&ran.shown, "STATUS").is_some_and(|l| l.starts_with("STATUS 96")), format!("conflict is 96: {:?}", ran.shown));

    println!("\n=== test_partiql_targets ===");
    println!("forms exercised: one SELECT CUST-ID, CUST-NAME, CUST-BALANCE ... WHERE CUST-BALANCE > ? filling a DataGrid, a ListBox, a Bar chart (value + second series), a Gauge, a Maps control, a repeating GroupBox; card write-back (00, key 21, conflict 96)");
    println!("PICTUREs: 9(8), X(40), S9(6)V99; engine: PRCIDXD1 (10 records, 8 above the parameter)");
    println!("checks: {} passed, {} failed; elapsed {} ms", checks - failures.len(), failures.len(), t0.elapsed().as_millis());
    for f in &failures {
        println!("  FAIL {f}");
    }
    assert!(failures.is_empty());
}
