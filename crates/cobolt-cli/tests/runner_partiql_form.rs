// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T48 (R10, R11, ABI A55): **the IndexedFile control's PartiQL members behave on the
//! WebAssembly path as they do under the interpreter.**
//!
//! One form with five IndexedFile controls over one customer record, each on a file of its own:
//! - `IXF-DISK` (PRCIDXD1), whose file is not there: `AutoOpen` makes it as the form starts. Its
//!   handler inserts, updates and deletes, with `?` arguments of every kind (a numeric item, an
//!   alphanumeric item, a numeric literal, a TextBox's text), then reads the rows with `Fetch`, `Get`
//!   by name and by place (a literal and a numeric item), `ColumnNames` and `ColumnCount`, and fetches
//!   past the last row;
//! - `IXF-REDB` (redb), `AutoCommit` off: `ExecuteTransaction` with two statements, the program's
//!   `ROLLBACK`, then `COMMIT` and `ROLLBACK`, a statement that does not parse, one naming a column the
//!   file does not have, and a numeric item compared with text (R3);
//! - `IXF-MEM` (a file held in memory, persisted): the flow of spec 108's host test;
//! - `IXF-SHARED`, whose file the form's program declares and opens as the form starts (`AutoOpen`
//!   performs the generated paragraph): the control works on the program's open file, the program's
//!   `READ` sees what the control inserted, and `ROLLBACK` undoes it (spec 108 R9, R24);
//! - `IXF-ADOPT`, whose file the program declares but has not opened: the control opens it, then the
//!   program's `OPEN` takes the control's file over, its `READ` and `WRITE` work on it, the control
//!   works on the program's file, and after the program's `CLOSE` the control opens it again.
//!
//! The script runs headless under `rcrun run-form` (the interpreter) and in the program `rcrun build`
//! makes on the WebAssembly path, each on the same files made afresh. The dumps must be equal and the
//! pictures equal in every pixel.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use cobolt_runtime::indexed::{status, IndexedStore, KeySpec, OpenMode};
use serde_json::{json, Value};

const FORM: &str = "PARITY-PARTIQL";

const LABELS: [&str; 5] = ["LBL-DISK", "LBL-REDB", "LBL-MEM", "LBL-SHARED", "LBL-ADOPT"];

/// The customer's record: `CUST-ID` 9(8), `CUST-NAME` X(40), `CUST-BALANCE` S9(6)V99.
const RECORD_LEN: usize = 56;

/// The definition of one file of customers, at `data` (relative to the project), held in `storage`.
fn cidx(data: &str, storage: &str, persist: bool) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><IndexedFile name="CUSTOMERS" finalized="true" version="1.0"><assign-path>{data}</assign-path><access-mode>dynamic</access-mode><record-format fixed-length="56"/><storage mode="{storage}" compression="false" persistence="{persist}"/><keys><primary duplicates="false" ordering="ascending"><part field="CUST-ID" offset="0" length="8" encoding="bytes"/></primary></keys><fields><Field level="1" name="CUST-REC" usage="display"><Field level="5" name="CUST-ID" pic="9(8)" usage="display" offset="0" length="8"></Field><Field level="5" name="CUST-NAME" pic="X(40)" usage="display" offset="8" length="40"></Field><Field level="5" name="CUST-BALANCE" pic="S9(6)V99" usage="display" offset="48" length="8"></Field></Field></fields></IndexedFile>"#
    )
}

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

fn key() -> KeySpec {
    KeySpec { parts: Vec::new(), offset: 0, len: 8, duplicates: false }
}

// ── The fixture ──────────────────────────────────────────────────────────────

/// A handler with its own WORKING-STORAGE items.
fn handler(ws: &[&str], statements: &[&str]) -> String {
    let ws = ws.iter().map(|s| format!("       {s}")).collect::<Vec<_>>().join("\n");
    let body = statements.iter().map(|s| format!("           {s}")).collect::<Vec<_>>().join("\n");
    format!("       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n{ws}\n       PROCEDURE DIVISION.\n{body}\n")
}

fn control(form: &mut Form, id: &str, kind: ControlType, rect: (i32, i32, i32, i32), props: &[(&str, &str)]) {
    let mut c = Control::new(id, kind, rect.0, rect.1);
    c.rect = Rect::new(rect.0, rect.1, rect.2, rect.3);
    for (k, v) in props {
        c.set_prop(*k, PropValue::String((*v).into()));
    }
    form.controls.push(c);
}

/// Bind `onClick` of button `id` to a handler.
fn on_click(form: &mut Form, id: &str, ws: &[&str], statements: &[&str]) {
    let c = form.controls.iter_mut().find(|c| c.id == id).expect("a control of the fixture");
    c.events.push(EventBinding { event: "onClick".into(), paragraph: format!("{id}--ONCLICK"), code: handler(ws, statements) });
}

/// The data files as each run starts: `disk.idx` and `mem.idx` are not there (the controls make
/// them), `redb.idx` is an empty redb container, `shared.idx` holds customer 50, `adopt.idx` is empty.
fn reset_data(root: &Path) {
    let data = root.join("data");
    let _ = fs::remove_dir_all(&data);
    fs::create_dir_all(&data).unwrap();
    let mut f = cobolt_runtime::indexed_redb::RedbIndexedFile::new(data.join("redb.idx"), RECORD_LEN, key(), Vec::new());
    assert_eq!(f.open(OpenMode::Output), status::OK);
    f.close();
    let mut f = cobolt_runtime::indexed_disk::DiskIndexedFile::new(data.join("shared.idx"), RECORD_LEN, key(), Vec::new());
    assert_eq!(f.open(OpenMode::Output), status::OK);
    assert_eq!(f.write(format!("00000050{:<40}00000100", "Pre").as_bytes()), status::OK);
    f.close();
    let mut f = cobolt_runtime::indexed_disk::DiskIndexedFile::new(data.join("adopt.idx"), RECORD_LEN, key(), Vec::new());
    assert_eq!(f.open(OpenMode::Output), status::OK);
    f.close();
}

/// The fixture project in `root`; its manifest's path.
fn project(root: &Path) -> PathBuf {
    let _ = fs::remove_dir_all(root);
    // `assets/` anchors the built program's relative paths at the project, as `rcrun run-form` anchors
    // them, so `data/` and `indexed/` are the same folders under both engines.
    for d in ["forms", "generated", "src", "data", "indexed", "assets"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    for (name, storage, persist) in [("disk", "disk", false), ("redb", "disk", false), ("mem", "memory", true), ("shared", "disk", false), ("adopt", "disk", false)] {
        fs::write(root.join(format!("indexed/{name}.cidx")), cidx(&format!("data/{name}.idx"), storage, persist)).unwrap();
    }
    let mut form = Form::new(FORM, "PartiQL", 900, 360);
    form.main_form = true;
    form.cobol_structure.file_control = [
        "           SELECT SHR-FILE ASSIGN TO \"data/shared.idx\"",
        "               ORGANIZATION IS INDEXED",
        "               ACCESS MODE IS DYNAMIC",
        "               RECORD KEY IS SH-ID.",
        "           SELECT ADP-FILE ASSIGN TO \"data/adopt.idx\"",
        "               ORGANIZATION IS INDEXED",
        "               ACCESS MODE IS DYNAMIC",
        "               RECORD KEY IS AD-ID.",
    ]
    .join("\n");
    form.cobol_structure.file_section = [
        "       FD SHR-FILE IS GLOBAL.",
        "       01 SH-REC.",
        "           05 SH-ID         PIC 9(8).",
        "           05 SH-NAME       PIC X(40).",
        "           05 SH-BALANCE    PIC S9(6)V99.",
        "       FD ADP-FILE IS GLOBAL.",
        "       01 AD-REC.",
        "           05 AD-ID         PIC 9(8).",
        "           05 AD-NAME       PIC X(40).",
        "           05 AD-BALANCE    PIC S9(6)V99.",
    ]
    .join("\n");
    let ixf = [
        ("IXF-DISK", "indexed/disk.cidx", vec![("AutoOpen", "true")]),
        ("IXF-REDB", "indexed/redb.cidx", vec![("AutoCommit", "false")]),
        ("IXF-MEM", "indexed/mem.cidx", vec![("AutoCommit", "true")]),
        // The program declares these two files: their facade names the FD's record and key.
        ("IXF-SHARED", "indexed/shared.cidx", vec![("AutoOpen", "true"), ("RecordName", "SH-REC"), ("KeyName", "SH-ID")]),
        ("IXF-ADOPT", "indexed/adopt.cidx", vec![("RecordName", "AD-REC"), ("KeyName", "AD-ID")]),
    ];
    for (i, (id, file, more)) in ixf.iter().enumerate() {
        let mut props = vec![("IndexedFile", *file), ("OpenMode", "I-O")];
        props.extend(more.iter().copied());
        control(&mut form, id, ControlType::IndexedFile, (20 + 60 * i as i32, 300, 48, 48), &props);
    }
    control(&mut form, "TXT-HI", ControlType::TextBox, (340, 300, 80, 28), &[("Text", "9")]);
    for (i, id) in ["BTN-DISK", "BTN-REDB", "BTN-MEM", "BTN-SHARED", "BTN-ADOPT"].iter().enumerate() {
        control(&mut form, id, ControlType::Button, (20, 20 + 52 * i as i32, 110, 28), &[("Caption", id.trim_start_matches("BTN-"))]);
    }
    for (i, id) in LABELS.iter().enumerate() {
        control(&mut form, id, ControlType::Label, (150, 20 + 52 * i as i32, 730, 44), &[("Caption", "-")]);
    }

    // PRCIDXD1: arguments of every kind, then every way of reading a result.
    on_click(
        &mut form,
        "BTN-DISK",
        &[
            "01 WS-N      PIC 9(4).",
            "01 WS-ID     PIC 9(8) VALUE 2.",
            "01 WS-NAME   PIC X(10) VALUE \"Bia\".",
            "01 WS-BAL    PIC S9(6)V99 VALUE 2000.",
            "01 WS-P      PIC 9 VALUE 2.",
            "01 WS-R1     PIC X(60).",
            "01 WS-R2     PIC X(60).",
            "01 WS-R3     PIC X(10).",
            "01 WS-G1     PIC X(12).",
            "01 WS-G2     PIC X(12).",
            "01 WS-G3     PIC X(12).",
            "01 WS-CN     PIC X(40).",
            "01 WS-CC     PIC 9(2).",
            "01 WS-L      PIC X(300).",
        ],
        &[
            "MOVE IXF-DISK::Execute(\"INSERT INTO DISK VALUE {'CUST-ID': 1, 'CUST-NAME': 'Ana', 'CUST-BALANCE': 10.50}\") TO WS-N",
            "MOVE IXF-DISK::Execute(\"INSERT INTO DISK VALUE {'CUST-ID': ?, 'CUST-NAME': ?, 'CUST-BALANCE': ?}\" WS-ID WS-NAME WS-BAL) TO WS-N",
            "MOVE IXF-DISK::Execute(\"INSERT INTO DISK VALUE {'CUST-ID': 3, 'CUST-NAME': 'Caio', 'CUST-BALANCE': 30.25}\") TO WS-N",
            "MOVE IXF-DISK::Execute(\"UPDATE DISK SET CUST-BALANCE = CUST-BALANCE + 1 WHERE CUST-ID = ?\" 1) TO WS-N",
            "MOVE IXF-DISK::Execute(\"DELETE FROM DISK WHERE CUST-ID = 3\") TO WS-N",
            "MOVE IXF-DISK::Execute(\"SELECT CUST-ID, CUST-NAME, CUST-BALANCE FROM DISK WHERE CUST-ID BETWEEN ? AND ?\" 1 TXT-HI::Text) TO WS-N",
            "MOVE IXF-DISK::Fetch() TO WS-R1",
            "MOVE IXF-DISK::Get(\"CUST-NAME\") TO WS-G1",
            "MOVE IXF-DISK::Get(WS-P) TO WS-G2",
            "MOVE IXF-DISK::Get(3) TO WS-G3",
            "MOVE IXF-DISK::ColumnNames() TO WS-CN",
            "MOVE IXF-DISK::ColumnCount() TO WS-CC",
            "MOVE IXF-DISK::Fetch() TO WS-R2",
            "MOVE IXF-DISK::Fetch() TO WS-R3",
            "STRING \"n \" WS-N \" r1 \" WS-R1 \"|g \" WS-G1 \"|\" WS-G2 \"|\" WS-G3 \"|cn \" WS-CN \"|cc \" WS-CC \"|r2 \" WS-R2 \"|r3 \" WS-R3 \"|\" DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-DISK::Caption",
        ],
    );
    // redb, AutoCommit off: a transaction, the program's COMMIT and ROLLBACK, and three failures.
    on_click(
        &mut form,
        "BTN-REDB",
        &[
            "01 WS-N1     PIC 9(4).",
            "01 WS-N2     PIC 9(4).",
            "01 WS-N3     PIC 9(4).",
            "01 WS-N4     PIC 9(4).",
            "01 WS-N5     PIC 9(4).",
            "01 WS-S1     PIC X(2).",
            "01 WS-S2     PIC X(2).",
            "01 WS-S3     PIC X(2).",
            "01 WS-E1     PIC X(30).",
            "01 WS-E2     PIC X(30).",
            "01 WS-ID     PIC 9(8) VALUE 42.",
            "01 WS-L      PIC X(300).",
        ],
        &[
            "MOVE IXF-REDB::ExecuteTransaction(\"INSERT INTO REDB VALUE {'CUST-ID': 7, 'CUST-NAME': 'Gil', 'CUST-BALANCE': 7}; INSERT INTO REDB VALUE {'CUST-ID': 8, 'CUST-NAME': 'Hal', 'CUST-BALANCE': 8}\") TO WS-N1",
            "ROLLBACK",
            "MOVE IXF-REDB::Execute(\"SELECT CUST-ID FROM REDB WHERE CUST-ID BETWEEN 1 AND 99\") TO WS-N2",
            "MOVE IXF-REDB::ExecuteTransaction(\"INSERT INTO REDB VALUE {'CUST-ID': 9, 'CUST-NAME': 'Ivo', 'CUST-BALANCE': 9}\") TO WS-N3",
            "COMMIT",
            "ROLLBACK",
            "MOVE IXF-REDB::Execute(\"SELECT CUST-ID FROM REDB WHERE CUST-ID BETWEEN 1 AND 99\") TO WS-N4",
            "MOVE IXF-REDB::Execute(\"SELEC CUST-ID FROM REDB\") TO WS-N5",
            "MOVE IXF-REDB::StatusCode TO WS-S1",
            "MOVE IXF-REDB::LastError TO WS-E1",
            "MOVE IXF-REDB::Execute(\"SELECT NOPE FROM REDB\") TO WS-N5",
            "MOVE IXF-REDB::StatusCode TO WS-S2",
            "MOVE IXF-REDB::LastError TO WS-E2",
            "MOVE IXF-REDB::Execute(\"SELECT CUST-ID FROM REDB WHERE begins_with(CUST-NAME, ?)\" WS-ID) TO WS-N5",
            "MOVE IXF-REDB::StatusCode TO WS-S3",
            "STRING \"tx \" WS-N1 \" rb \" WS-N2 \" tx \" WS-N3 \" commit \" WS-N4 \"|parse \" WS-S1 \" \" WS-E1 \"|column \" WS-S2 \" \" WS-E2 \"|typed \" WS-S3 \"|\" DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-REDB::Caption",
        ],
    );
    // A file held in memory: spec 108's flow.
    on_click(
        &mut form,
        "BTN-MEM",
        &["01 WS-N      PIC 9(4).", "01 WS-R      PIC X(60).", "01 WS-L      PIC X(120)."],
        &[
            "MOVE IXF-MEM::Execute(\"INSERT INTO MEM VALUE {'CUST-ID': 1, 'CUST-NAME': 'Ana', 'CUST-BALANCE': 10.50}\") TO WS-N",
            "MOVE IXF-MEM::Execute(\"INSERT INTO MEM VALUE {'CUST-ID': 2, 'CUST-NAME': 'Bia', 'CUST-BALANCE': 2000.00}\") TO WS-N",
            "MOVE IXF-MEM::Execute(\"INSERT INTO MEM VALUE {'CUST-ID': 3, 'CUST-NAME': 'Caio', 'CUST-BALANCE': 30.25}\") TO WS-N",
            "MOVE IXF-MEM::Execute(\"UPDATE MEM SET CUST-BALANCE = CUST-BALANCE + 1 WHERE CUST-ID = 1\") TO WS-N",
            "MOVE IXF-MEM::Execute(\"DELETE FROM MEM WHERE CUST-ID = 3\") TO WS-N",
            "MOVE IXF-MEM::Execute(\"SELECT CUST-ID, CUST-NAME, CUST-BALANCE FROM MEM WHERE CUST-ID BETWEEN 1 AND 9\") TO WS-N",
            "MOVE IXF-MEM::Fetch() TO WS-R",
            "STRING \"n \" WS-N \" r \" WS-R \"|\" DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-MEM::Caption",
        ],
    );
    // The program's own open file: the control works on it, and the program's ROLLBACK reaches both.
    on_click(
        &mut form,
        "BTN-SHARED",
        &["01 WS-N1     PIC 9(4).", "01 WS-N2     PIC 9(4).", "01 WS-N3     PIC 9(4).", "01 WS-F1     PIC X(8).", "01 WS-L      PIC X(120)."],
        &[
            "MOVE IXF-SHARED::Execute(\"SELECT CUST-NAME FROM SHARED WHERE CUST-ID = 50\") TO WS-N1",
            "MOVE IXF-SHARED::Execute(\"INSERT INTO SHARED VALUE {'CUST-ID': 51, 'CUST-NAME': 'Ctl', 'CUST-BALANCE': 5}\") TO WS-N2",
            "MOVE 51 TO SH-ID",
            "READ SHR-FILE KEY IS SH-ID",
            "    INVALID KEY MOVE \"missing\" TO WS-F1",
            "    NOT INVALID KEY MOVE SH-NAME TO WS-F1",
            "END-READ",
            "ROLLBACK",
            "MOVE IXF-SHARED::Execute(\"SELECT CUST-ID FROM SHARED WHERE CUST-ID BETWEEN 50 AND 51\") TO WS-N3",
            "STRING \"sel \" WS-N1 \" ins \" WS-N2 \" read \" WS-F1 \" after rollback \" WS-N3 \"|\" DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-SHARED::Caption",
        ],
    );
    // The control's own file, then the program's OPEN takes it over.
    on_click(
        &mut form,
        "BTN-ADOPT",
        &[
            "01 WS-N1     PIC 9(4).",
            "01 WS-N2     PIC 9(4).",
            "01 WS-N3     PIC 9(4).",
            "01 WS-F1     PIC X(8).",
            "01 WS-F2     PIC X(8) VALUE \"written\".",
            "01 WS-L      PIC X(120).",
        ],
        &[
            "MOVE IXF-ADOPT::Execute(\"INSERT INTO ADOPT VALUE {'CUST-ID': 61, 'CUST-NAME': 'Own', 'CUST-BALANCE': 6}\") TO WS-N1",
            "OPEN I-O ADP-FILE",
            "MOVE 61 TO AD-ID",
            "READ ADP-FILE KEY IS AD-ID",
            "    INVALID KEY MOVE \"missing\" TO WS-F1",
            "    NOT INVALID KEY MOVE AD-NAME TO WS-F1",
            "END-READ",
            "MOVE 62 TO AD-ID",
            "MOVE \"Verb\" TO AD-NAME",
            "MOVE 2 TO AD-BALANCE",
            "WRITE AD-REC INVALID KEY MOVE \"dup\" TO WS-F2 END-WRITE",
            "MOVE IXF-ADOPT::Execute(\"SELECT CUST-ID FROM ADOPT WHERE CUST-ID BETWEEN 61 AND 62\") TO WS-N2",
            "COMMIT",
            "CLOSE ADP-FILE",
            "MOVE IXF-ADOPT::Execute(\"SELECT CUST-ID FROM ADOPT WHERE CUST-ID BETWEEN 61 AND 62\") TO WS-N3",
            "STRING \"ins \" WS-N1 \" read \" WS-F1 \" write \" WS-F2 \" sel \" WS-N2 \" reopened \" WS-N3 \"|\" DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-ADOPT::Caption",
        ],
    );

    let cfrm = root.join("forms/parity-partiql.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/parity-partiql.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(root.join("src/main.cbl"), "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n").unwrap();
    let manifest = root.join("paritypartiql.project.toml");
    fs::write(
        &manifest,
        "[project]\nname = \"paritypartiql\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/parity-partiql.cfrm\"]\ngenerated = [\"generated/parity-partiql.cbl\"]\n",
    )
    .unwrap();
    manifest
}

/// The script, and the names of its pictures in order.
fn script() -> (Value, Vec<&'static str>) {
    let click = |id: &str| json!({"event": {"control": id, "name": "onClick"}});
    let wait = |ms: u64| json!({"wait_ms": ms});
    let dump = || json!({"dump": true});
    let shot = |name: &str| json!({"shot": name});
    let names = vec!["s1-disk-redb", "s2-mem-shared", "s3-adopt"];
    let steps = vec![
        wait(200),
        click("BTN-DISK"),
        wait(300),
        click("BTN-REDB"),
        wait(300),
        dump(),
        shot(names[0]),
        click("BTN-MEM"),
        wait(300),
        click("BTN-SHARED"),
        wait(300),
        dump(),
        shot(names[1]),
        click("BTN-ADOPT"),
        wait(300),
        dump(),
        shot(names[2]),
    ];
    (Value::Array(steps), names)
}

// ── Running both engines ─────────────────────────────────────────────────────

/// The report line of a headless run.
fn report(stdout: &str) -> Value {
    stdout
        .lines()
        .find_map(|l| l.strip_prefix(cobolt_form_host::headless::RESULT_MARKER))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_else(|| panic!("no result line in:\n{stdout}"))
}

/// Every dump of a report, in order.
fn dumps(report: &Value) -> Vec<Value> {
    report["steps"].as_array().map(|s| s.iter().filter(|st| st.get("dump").is_some()).map(|st| st["state"].clone()).collect()).unwrap_or_default()
}

/// Pixels that differ between two pictures (all of them when the sizes differ or a picture is missing).
fn differing_pixels(a: &Path, b: &Path) -> (u64, (u32, u32)) {
    let (Ok(x), Ok(y)) = (image::open(a), image::open(b)) else { return (u64::MAX, (0, 0)) };
    let (x, y) = (x.to_rgba8(), y.to_rgba8());
    if x.dimensions() != y.dimensions() {
        return (u64::from(x.width()) * u64::from(x.height()), x.dimensions());
    }
    (x.pixels().zip(y.pixels()).filter(|(p, q)| p != q).count() as u64, x.dimensions())
}

/// The keys whose values differ between two dumps, as `control: …`.
fn dump_differences(a: &Value, b: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let (Some(a), Some(b)) = (a.as_object(), b.as_object()) else { return vec!["(not a dump)".into()] };
    let mut ids: Vec<&String> = a.keys().chain(b.keys()).collect();
    ids.sort();
    ids.dedup();
    let shown = |v: Option<&Value>| v.map(|v| v.to_string()).unwrap_or_else(|| "-".into());
    for id in ids {
        if a.get(id) != b.get(id) {
            out.push(format!("{id}: interpreter {} / runner {}", shown(a.get(id)), shown(b.get(id))));
        }
    }
    out
}

/// A control's property in a dump (upper case or as written).
fn prop(dump: &Value, id: &str, name: &str) -> String {
    let props = &dump[id]["props"];
    props.get(name.to_ascii_uppercase()).or_else(|| props.get(name)).and_then(Value::as_str).unwrap_or("").to_string()
}

/// What one engine did: its dumps and how long it ran.
struct Seen {
    dumps: Vec<Value>,
    ms: f64,
}

/// What both engines did with the same script.
struct Twin {
    interp: Seen,
    wasm: Seen,
    interp_shots: PathBuf,
    runner_shots: PathBuf,
    runner_ms: f64,
    build_ms: f64,
}

/// Run `script` under `rcrun run-form` (the interpreter), then build the project on the WebAssembly path
/// and run it in the built program, each on the data files made afresh.
fn run_twin(root: &Path, manifest: &Path, script: &Path) -> Twin {
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    // ── The runner, built beside rcrun, as a release has it ──
    let t = Instant::now();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new(&cargo).current_dir(&workspace).args(["build", "-q", "-p", "cobolt-runner", "--bin", "prc-runner"]).status().expect("run cargo");
    assert!(status.success(), "cargo build -p cobolt-runner --bin prc-runner failed");
    let runner_ms = t.elapsed().as_secs_f64() * 1000.0;

    // ── 1. The interpreter: rcrun run-form ──
    reset_data(root);
    let interp_shots = root.join("shots-interpreter");
    fs::create_dir_all(&interp_shots).unwrap();
    let t = Instant::now();
    let out = Command::new(rcrun())
        .args(["run-form", "forms/parity-partiql.cfrm", "generated/parity-partiql.cbl", "--designer", "--headless"])
        .arg(script)
        .args(["--headless-limit", "60"])
        .current_dir(root)
        .env("HOME", &home)
        .env("PRC_HEADLESS_SHOTS", &interp_shots)
        .output()
        .expect("run rcrun run-form");
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form failed:\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
    let interp = Seen { dumps: dumps(&report(&said)), ms: t.elapsed().as_secs_f64() * 1000.0 };

    // ── 2. The runner: rcrun build on the WebAssembly path, then the program ──
    let t = Instant::now();
    let out = Command::new(rcrun())
        .arg("build")
        .arg(manifest)
        .arg("--quiet")
        .env(cobolt_compiler::BUILD_DIR_ENV, root.join("builds"))
        .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
        .env_remove(cobolt_compiler::RUNNER_ENV)
        .output()
        .expect("run rcrun build");
    let build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let build_out = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun build failed:\n{build_out}\n{}", String::from_utf8_lossy(&out.stderr));
    assert!(build_out.lines().any(|l| l.trim() == "Path   : WebAssembly"), "the Build took the WebAssembly path:\n{build_out}");
    let exe = root.join("bin").join(format!("paritypartiql{}", std::env::consts::EXE_SUFFIX));
    reset_data(root);
    let runner_shots = root.join("shots-runner");
    fs::create_dir_all(&runner_shots).unwrap();
    let t = Instant::now();
    let out = Command::new(&exe)
        .current_dir(root)
        .env("HOME", &home)
        .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
        .env("PRC_HEADLESS_SCRIPT", script)
        .env("PRC_HEADLESS_LIMIT", "60")
        .env("PRC_HEADLESS_SHOTS", &runner_shots)
        .output()
        .expect("run the built program");
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "the built program failed:\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
    let wasm = Seen { dumps: dumps(&report(&said)), ms: t.elapsed().as_secs_f64() * 1000.0 };
    Twin { interp, wasm, interp_shots, runner_shots, runner_ms, build_ms }
}

#[test]
fn the_indexedfile_partiql_members_agree_on_both_engines() {
    let root = std::env::temp_dir().join(format!("prc-t48-partiql-{}", std::process::id()));
    let manifest = project(&root);
    let (steps, shots) = script();
    let script = root.join("script.json");
    fs::write(&script, steps.to_string()).unwrap();
    let twin = run_twin(&root, &manifest, &script);

    // ── The two engines against each other: dumps and pictures ──
    let mut checks: Vec<(String, bool)> = Vec::new();
    checks.push((format!("both engines dump {} times", shots.len()), twin.interp.dumps.len() == shots.len() && twin.wasm.dumps.len() == shots.len()));
    let mut dump_lines = Vec::new();
    for (i, (x, y)) in twin.interp.dumps.iter().zip(twin.wasm.dumps.iter()).enumerate() {
        let diff = dump_differences(x, y);
        checks.push((format!("dump {i} ({}) is equal", shots[i]), diff.is_empty()));
        dump_lines.extend(diff.into_iter().map(|d| format!("    dump {i}: {d}")));
    }
    let (mut pixels_total, mut differing_total, mut shot_lines) = (0u64, 0u64, Vec::new());
    for name in &shots {
        let file = format!("{name}.png");
        let (diff, (w, h)) = differing_pixels(&twin.interp_shots.join(&file), &twin.runner_shots.join(&file));
        pixels_total += u64::from(w) * u64::from(h);
        differing_total = differing_total.saturating_add(diff);
        checks.push((format!("picture {name}: 0 differing pixels"), diff == 0));
        shot_lines.push(format!("    {name}: {w}x{h}, {} differing pixels", if diff == u64::MAX { "missing,".to_string() } else { diff.to_string() }));
    }

    // ── What the program saw (the runner's dumps; the interpreter's are equal above) ──
    let at = |i: usize, id: &str, p: &str| twin.wasm.dumps.get(i).map(|d| prop(d, id, p)).unwrap_or_default();
    let shown: Vec<String> = (0..shots.len())
        .flat_map(|i| LABELS.iter().map(move |id| (i, *id)))
        .map(|(i, id)| format!("    dump {i} {id}: {:?}", at(i, id, "Caption").trim_end()))
        .collect();
    checks.extend([
        ("the PRCIDXD1 file: 2 rows selected (a TextBox's text as a bound), the first Ana's".to_string(), at(0, "LBL-DISK", "Caption").starts_with("n 0002 r1 ")),
        ("Get by name and by place (a numeric item, a literal) read the fetched row".into(), at(0, "LBL-DISK", "Caption").contains("|g Ana         |Ana         |")),
        ("the column names and their count".into(), at(0, "LBL-DISK", "Caption").contains("|cc 03|")),
        ("a fetch past the last row: StatusCode 10".into(), at(0, "IXF-DISK", "StatusCode") == "10"),
        ("the redb file: the program's ROLLBACK undoes a transaction, COMMIT keeps one".into(), at(0, "LBL-REDB", "Caption").starts_with("tx 0002 rb 0000 tx 0001 commit 0001|")),
        ("a numeric item compared with text is 94 (R3)".into(), at(0, "LBL-REDB", "Caption").contains("|typed 94|")),
        ("the file in memory: 2 rows, the first Ana's".into(), at(1, "LBL-MEM", "Caption").starts_with("n 0002 r ")),
        ("the program's open file: the control reads it, the READ sees its INSERT, ROLLBACK undoes it".into(), at(1, "LBL-SHARED", "Caption").trim_end() == "sel 0001 ins 0001 read Ctl      after rollback 0001|"),
        ("the program's OPEN takes the control's file over; after its CLOSE the control opens it again".into(), at(2, "LBL-ADOPT", "Caption").trim_end() == "ins 0001 read Own      write written  sel 0002 reopened 0002|"),
    ]);

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== the_indexedfile_partiql_members_agree_on_both_engines (DESKTOP-WASM-001 T48: R10, R11, ABI A55) ==");
    println!("form {FORM}: 5 IndexedFile controls (PRCIDXD1 made by AutoOpen, redb, memory, the program's open file, a file the program's OPEN takes over), 5 buttons, 5 event handlers, 30 member calls");
    println!("  INSERT / UPDATE / DELETE / SELECT with ? from a numeric item, an alphanumeric item, a literal and a TextBox; Fetch, Get by name and place, ColumnNames, ColumnCount, a fetch past the end;");
    println!("  ExecuteTransaction, COMMIT and ROLLBACK with AutoCommit off; a parse error, an unknown column, a numeric item against text; spec 108 R9 sharing both ways");
    println!("real time; {} dumps and {} pictures per engine, each run on the files made afresh", shots.len(), shots.len());
    println!("runner built in {:.0} ms; rcrun build (WebAssembly path) {:.0} ms", twin.runner_ms, twin.build_ms);
    println!("interpreter run {:.0} ms; runner run {:.0} ms", twin.interp.ms, twin.wasm.ms);
    for l in &shot_lines {
        println!("{l}");
    }
    println!("pixels compared: {pixels_total}, differing: {differing_total}");
    for l in &dump_lines {
        println!("{l}");
    }
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("result: {passed} of {} checks passed", checks.len());
    let failed: Vec<&String> = checks.iter().filter(|(_, ok)| !*ok).map(|(w, _)| w).collect();
    if failed.is_empty() {
        let _ = fs::remove_dir_all(&root);
    } else {
        for l in &shown {
            println!("{l}");
        }
        println!("kept for inspection: {}", root.display());
    }
    assert!(failed.is_empty(), "failed: {failed:?}");
}
