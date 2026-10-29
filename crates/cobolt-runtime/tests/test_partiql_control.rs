// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 108, T11 / T12 — the `IndexedFile` control's members run from COBOL:
//! `Execute`, `Fetch`, `Get`, `ColumnNames`, the properties it publishes, and
//! the file it holds (opened from the `.cidx`, created when missing, closed
//! when the run ends). A form here declares no `SELECT` and no `FD`.
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

#[test]
fn partiql_control() {
    let t0 = Instant::now();
    let mut checks = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let mut check = |ok: bool, what: String| {
        checks += 1;
        if !ok {
            failures.push(what);
        }
    };
    let ex = |s: &str| format!("           SET WS-N TO IXF-CUST::Execute(\"{s}\").\n");

    // AC2 — Fetch, Get, properties, a second Fetch, ColumnNames, ResultCount.
    let p = project("ctl-a", true, 100);
    let mut body = ex("SELECT CUST-ID, CUST-BALANCE, BILLING.CITY FROM CUSTOMERS WHERE CUST-ID = 42");
    body += "           DISPLAY WS-N.\n           SET WS-ROW TO IXF-CUST::Fetch().\n           DISPLAY WS-ROW.\n";
    body += "           SET WS-V TO IXF-CUST::Get(\"CUST-BALANCE\").\n           DISPLAY WS-V.\n";
    body += "           SET WS-V TO IXF-CUST::Get(2).\n           DISPLAY WS-V.\n";
    body += "           SET WS-V TO IXF-CUST::Get(\"BILLING.CITY\").\n           DISPLAY WS-V.\n";
    body += "           IF IXF-CUST::CUST-BALANCE < 0 DISPLAY \"NEGATIVE\" END-IF.\n";
    body += "           SET WS-V TO IXF-CUST::CUST-NAME.\n           DISPLAY \"NAME[\" WS-V \"]\".\n";
    body += "           SET WS-V TO IXF-CUST::Get(\"CUST-NAME\").\n           DISPLAY \"GETNAME[\" WS-V \"]\".\n";
    body += "           SET WS-ROW TO IXF-CUST::Fetch().\n           DISPLAY \"SECOND[\" WS-ROW \"]\".\n           DISPLAY IXF-CUST::StatusCode.\n";
    body += "           SET WS-V TO IXF-CUST::ColumnNames().\n           DISPLAY WS-V.\n           DISPLAY IXF-CUST::ResultCount.\n";
    body += &ex("SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID BETWEEN 2 AND 6");
    body += "           DISPLAY IXF-CUST::ResultCount.\n           DISPLAY WS-N.\n";
    body += &ex("SELECT COUNT(*) FROM CUSTOMERS");
    body += "           DISPLAY WS-N.\n           DISPLAY IXF-CUST::StatusCode.\n           DISPLAY IXF-CUST::LastError.\n           DISPLAY IXF-CUST::ResultCount.\n";
    let out = run_cobol(&body, &p, "I-O");
    let show = out.iter().map(|l| l.replace('\t', "<TAB>")).collect::<Vec<_>>();
    let want = [
        "1", // Execute answers the item count
        "00000042<TAB>-123.45<TAB>Recife",
        "-123.45",
        "-123.45",
        "Recife",
        "NEGATIVE",
        "NAME[",
        "GETNAME[",
        "SECOND[",
        "10",
        "CUST-ID<TAB>CUST-BALANCE<TAB>BILLING.CITY",
        "1",
        "3",
        "3",
    ];
    let joined = show.join("|");
    for w in want {
        check(joined.contains(w), format!("AC2: {w:?} not in {show:?}"));
    }
    check(
        show.iter().any(|l| l.starts_with("0") && l.trim() == "0") && show.iter().any(|l| l.contains("ValidationException") && l.contains("COUNT")),
        format!("a failed Execute answers 0 and leaves its reason: {show:?}"),
    );
    check(
        show.iter().filter(|l| l.trim() == "94").count() == 1 && show.last().map(|l| l.trim()) == Some("0"),
        format!("a failed Execute empties the result set: {show:?}"),
    );

    // R8 — the file is closed when the run ends: its lock sidecar can be taken again.
    let again = std::fs::OpenOptions::new().create(true).read(true).write(true).truncate(false).open(format!("{}.lck", p.data));
    check(again.is_ok_and(|f| f.try_lock().is_ok()), "the file's lock is released at the end of the run".into());

    // R10 — no data file: an I-O control creates it from the .cidx; an INPUT one answers 35.
    let q = project("ctl-b", false, 0);
    let out = run_cobol(
        &format!(
            "{}           DISPLAY IXF-CUST::StatusCode.\n{}{}           DISPLAY IXF-CUST::ResultCount.\n",
            ex("INSERT INTO CUSTOMERS VALUE {'CUST-ID': 7, 'CUST-NAME': 'Nova'}"),
            ex("SELECT CUST-NAME FROM CUSTOMERS WHERE CUST-ID = 7"),
            "           SET WS-ROW TO IXF-CUST::Fetch().\n           DISPLAY WS-ROW.\n"
        ),
        &q,
        "I-O",
    );
    check(out.iter().any(|l| l.trim().trim_start_matches('0').is_empty()) && out.iter().any(|l| l.trim() == "Nova"), format!("created from the .cidx: {out:?}"));
    check(std::path::Path::new(&q.data).exists(), "the data file now exists".into());
    let r = project("ctl-c", false, 0);
    let out = run_cobol(&format!("{}           DISPLAY IXF-CUST::StatusCode.\n           DISPLAY IXF-CUST::LastError.\n", ex("SELECT * FROM CUSTOMERS")), &r, "INPUT");
    check(out.iter().any(|l| l.trim() == "35") && !std::path::Path::new(&r.data).exists(), format!("INPUT with no file is 35 and creates nothing: {out:?}"));

    // R7 — a table that is not the control's file.
    let out = run_cobol(&format!("{}           DISPLAY IXF-CUST::StatusCode.\n           DISPLAY IXF-CUST::LastError.\n", ex("SELECT * FROM ORDERS")), &p, "I-O");
    check(out.iter().any(|l| l.trim() == "94") && out.iter().any(|l| l.starts_with("ResourceNotFoundException")), format!("another table: {out:?}"));

    // O2 — a property is untyped text: "42" against a number field, "Cli" against a text field,
    // and text that is not a number against a number field is 94 (never a silent miss).
    let tb = |id: &str, text: &str| (id.to_owned(), "TextBox".to_owned(), vec![("Text".to_owned(), text.to_owned())]);
    let out = run_cobol_with(
        "           SET WS-N TO IXF-CUST::Execute(\"SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID = ?\" TXT-ID::Text).\n           DISPLAY WS-N.\n           SET WS-N TO IXF-CUST::Execute(\"SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID = ?\" TXT-BAD::Text).\n           DISPLAY IXF-CUST::StatusCode.\n           SET WS-N TO IXF-CUST::Execute(\"SELECT CUST-ID FROM CUSTOMERS WHERE begins_with(CUST-NAME, ?)\" TXT-NUM::Text).\n           DISPLAY IXF-CUST::StatusCode.\n           DISPLAY IXF-CUST::ResultCount.\n           MOVE 42 TO WS-ID.\n           SET WS-N TO IXF-CUST::Execute(\"SELECT CUST-ID FROM CUSTOMERS WHERE begins_with(CUST-NAME, ?)\" WS-ID).\n           DISPLAY IXF-CUST::StatusCode.\n",
        &p,
        "I-O",
        vec![tb("TXT-ID", "42"), tb("TXT-BAD", "forty"), tb("TXT-NUM", "7")],
    );
    check(out.first().map(|l| l.trim().trim_start_matches('0')) == Some("1"), format!("property text '42' finds record 42: {out:?}"));
    check(out.get(1).map(|l| l.trim()) == Some("94"), format!("non-numeric property text against a Number is 94: {out:?}"));
    check(out.get(2).map(|l| l.trim().trim_start_matches('0')) == Some(""), format!("numeric-looking property text against a String field is text: {out:?}"));
    check(out.get(4).map(|l| l.trim()) == Some("94"), format!("a numeric data item against a String field is a mismatch (Q10): {out:?}"));

    // R9 / R24 — a form that declares the FD: one open file. The verb's WRITE is seen by the
    // control; AutoCommit is off by default, so the program's ROLLBACK undoes the control's INSERT too.
    let f = project("ctl-d", true, 10);
    let write = |id: u32, name: &str| format!("           MOVE {id} TO CUST-ID.\n           MOVE \"{name}\" TO CUST-NAME.\n           MOVE 5 TO CUST-BALANCE.\n           MOVE \"X\" TO CITY.\n           WRITE CUST-REC.\n");
    let proc_ = format!(
        "           OPEN I-O CUSTOMERS.\n{}           SET WS-N TO IXF-CUST::Execute(\"SELECT CUST-NAME FROM CUSTOMERS WHERE CUST-ID = 8001\").\n           DISPLAY WS-N.\n           SET WS-N TO IXF-CUST::Execute(\"INSERT INTO CUSTOMERS VALUE {{'CUST-ID': 8002, 'CUST-NAME': 'Ctl'}}\").\n           ROLLBACK.\n           SET WS-N TO IXF-CUST::Execute(\"SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID BETWEEN 8001 AND 8002\").\n           DISPLAY WS-N.\n           CLOSE CUSTOMERS.\n",
        write(8001, "Verb")
    );
    let out = run_with_fd(&proc_, &f, None);
    check(out.first().map(|l| l.trim().trim_start_matches('0')) == Some("1"), format!("the control sees the verb's WRITE: {out:?}"));
    check(out.get(1).map(|l| l.trim().trim_start_matches('0')) == Some(""), format!("ROLLBACK undoes the verb's WRITE and the control's INSERT: {out:?}"));
    // AutoCommit on: the statement commits what the verb had pending, and survives a later ROLLBACK.
    let g = project("ctl-e", true, 10);
    let proc_ = format!(
        "           OPEN I-O CUSTOMERS.\n{}           SET WS-N TO IXF-CUST::Execute(\"INSERT INTO CUSTOMERS VALUE {{'CUST-ID': 8004, 'CUST-NAME': 'Ctl'}}\").\n           ROLLBACK.\n           SET WS-N TO IXF-CUST::Execute(\"SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID BETWEEN 8003 AND 8004\").\n           DISPLAY WS-N.\n           CLOSE CUSTOMERS.\n",
        write(8003, "Verb")
    );
    let out = run_with_fd(&proc_, &g, Some("true"));
    check(out.first().map(|l| l.trim().trim_start_matches('0')) == Some("2"), format!("AutoCommit on commits the statement and the pending WRITE: {out:?}"));

    println!("\n=== test_partiql_control ===");
    println!("forms exercised: Execute, Fetch, Get(name|position), ColumnNames, ResultCount/StatusCode/LastError, property reads (IXF-CUST::CUST-BALANCE), INSERT into a file created from the .cidx, INPUT on a missing file, ResourceNotFoundException, a form that declares the FD (shared engine, ROLLBACK, AutoCommit on/off)");
    println!("PICTUREs: 9(8), X(40), S9(6)V99, X(20) in group BILLING; engine: PRCIDXD1 (100 records); no SELECT / FD declared");
    println!("checks: {} passed, {} failed; elapsed {} ms", checks - failures.len(), failures.len(), t0.elapsed().as_millis());
    for f in &failures {
        println!("  FAIL {f}");
    }
    assert!(failures.is_empty());
}
