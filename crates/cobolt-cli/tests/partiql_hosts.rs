// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 108 (T16, AC8): **PartiQL on an indexed file, on the three hosts that run a form** -
//! `rcrun run-form`, a child form loaded into an application shell's ContentPane, and a built
//! binary (`--ignored`: it compiles one) - **on three engines**: `PRCIDXD1` (disk), redb and a
//! file held in memory (`<storage mode="memory">`).
//!
//! One form with an `IndexedFile` control and one button. The button's handler inserts three
//! customers, runs a keyed `SELECT`, an `UPDATE` and a `DELETE`, then reads the first row with
//! `Fetch`; the test reads the control's properties back. Every cell must give the same answers.
//!
//! One result block at the end (GOLDEN RULE #7).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use cobolt_forms::model::FormFormat;
use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

struct Project {
    root: PathBuf,
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

#[derive(Clone, Copy)]
enum Engine {
    Disk,
    Redb,
    Memory,
}

impl Engine {
    fn name(self) -> &'static str {
        match self {
            Engine::Disk => "PRCIDXD1",
            Engine::Redb => "redb",
            Engine::Memory => "memory",
        }
    }
}

fn cidx_xml(data: &str, storage: &str, persist: bool) -> String {
    // A MEMORY file without persistence is a read-only copy (it opens INPUT only), so the memory cell persists.
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><IndexedFile name="CUSTOMERS" finalized="true" version="1.0"><assign-path>{data}</assign-path><access-mode>dynamic</access-mode><record-format fixed-length="56"/><storage mode="{storage}" compression="false" persistence="{persist}"/><keys><primary duplicates="false" ordering="ascending"><part field="CUST-ID" offset="0" length="8" encoding="bytes"/></primary></keys><fields><Field level="1" name="CUST-REC" usage="display"><Field level="5" name="CUST-ID" pic="9(8)" usage="display" offset="0" length="8"></Field><Field level="5" name="CUST-NAME" pic="X(40)" usage="display" offset="8" length="40"></Field><Field level="5" name="CUST-BALANCE" pic="S9(6)V99" usage="display" offset="48" length="8"></Field></Field></fields></IndexedFile>"#
    )
}

const HANDLER: &str = "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n\
           IXF-CUST::Execute(\"INSERT INTO CUSTOMERS VALUE {'CUST-ID': 1, 'CUST-NAME': 'Ana', 'CUST-BALANCE': 10.50}\")\n\
           IXF-CUST::Execute(\"INSERT INTO CUSTOMERS VALUE {'CUST-ID': 2, 'CUST-NAME': 'Bia', 'CUST-BALANCE': 2000.00}\")\n\
           IXF-CUST::Execute(\"INSERT INTO CUSTOMERS VALUE {'CUST-ID': 3, 'CUST-NAME': 'Caio', 'CUST-BALANCE': 30.25}\")\n\
           IXF-CUST::Execute(\"UPDATE CUSTOMERS SET CUST-BALANCE = CUST-BALANCE + 1 WHERE CUST-ID = 1\")\n\
           IXF-CUST::Execute(\"DELETE FROM CUSTOMERS WHERE CUST-ID = 3\")\n\
           IXF-CUST::Execute(\"SELECT CUST-ID, CUST-NAME, CUST-BALANCE FROM CUSTOMERS WHERE CUST-ID BETWEEN 1 AND 9\")\n\
           IXF-CUST::Fetch()\n";

fn demo_form(cidx: &str) -> Form {
    let mut f = Form::new("PARTIQL-DEMO", "PartiQL", 640, 360);
    f.form_format = FormFormat::Both;
    let mut ixf = Control::new("IXF-CUST", ControlType::IndexedFile, 300, 20);
    ixf.set_prop("IndexedFile", PropValue::String(cidx.into()));
    ixf.set_prop("OpenMode", PropValue::String("I-O".into()));
    ixf.set_prop("AutoCommit", PropValue::String("true".into()));
    f.controls.push(ixf);
    let mut b = Control::new("BTN-RUN", ControlType::Button, 20, 20);
    b.rect = Rect::new(20, 20, 200, 28);
    b.set_prop("Caption", PropValue::String("Run".into()));
    b.events.push(EventBinding { event: "onClick".into(), paragraph: "BTN-RUN--ONCLICK".into(), code: HANDLER.into() });
    f.controls.push(b);
    f
}

fn project(tag: &str, engine: Engine) -> Project {
    let root = std::env::temp_dir().join(format!("prc-108-hosts-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for d in ["forms", "generated", "src", "data"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    let data = root.join("data/customers.idx");
    let cidx = root.join("customers.cidx");
    let storage = if matches!(engine, Engine::Memory) { "memory" } else { "disk" };
    std::fs::write(&cidx, cidx_xml(&data.to_string_lossy(), storage, matches!(engine, Engine::Memory))).unwrap();
    if matches!(engine, Engine::Redb) {
        // An existing file names its engine: create the container as redb.
        use cobolt_runtime::indexed::{IndexedStore, KeySpec, OpenMode};
        let mut f = cobolt_runtime::indexed_redb::RedbIndexedFile::new(
            &data,
            56,
            KeySpec { parts: Vec::new(), offset: 0, len: 8, duplicates: false },
            Vec::new(),
        );
        assert_eq!(f.open(OpenMode::Output), "00");
        f.close();
    }
    let form = demo_form(&cidx.to_string_lossy());
    cobolt_forms::save_form(&form, &root.join("forms/partiql-demo.cfrm")).unwrap();
    let form = cobolt_forms::load_form(&root.join("forms/partiql-demo.cfrm")).unwrap();
    std::fs::write(root.join("generated/partiql-demo.cbl"), cobolt_codegen::generate(&form)).unwrap();
    // The shell the embedded run loads it into: any application shell works, so borrow the AWS demos'.
    let shell = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/aws/forms");
    for file in ["aws-shell.cfrm", "SideMenu-1.menu.yaml"] {
        std::fs::copy(shell.join(file), root.join("forms").join(file)).unwrap();
    }
    let shell_form = cobolt_forms::load_form(&root.join("forms/aws-shell.cfrm")).unwrap();
    std::fs::write(root.join("generated/aws-shell.cbl"), cobolt_codegen::generate(&shell_form)).unwrap();
    std::fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("partiqlhosts.project.toml"),
        "[project]\nname = \"PartiqlHosts\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/partiql-demo.cfrm\", \"forms/aws-shell.cfrm\"]\n\
         generated = [\"generated/partiql-demo.cbl\", \"generated/aws-shell.cbl\"]\n",
    )
    .unwrap();
    Project { root }
}

fn steps(open: Option<&str>) -> Value {
    let read = |property: &str| json!({"read": {"control": "IXF-CUST", "property": property}});
    let mut s = Vec::new();
    if let Some(form) = open {
        s.push(json!({"open_form": form}));
    }
    s.push(json!({"event": {"control": "BTN-RUN", "name": "onClick"}}));
    s.push(json!({"wait_ms": 1500}));
    for p in ["StatusCode", "ResultCount", "CUST-ID", "CUST-NAME", "CUST-BALANCE"] {
        s.push(read(p));
    }
    Value::Array(s)
}

fn readings(stdout: &str) -> Vec<String> {
    let report: Value = stdout
        .lines()
        .find_map(|l| l.strip_prefix("@RUN-FORM-RESULT "))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_else(|| panic!("no result line in:\n{stdout}"));
    report["steps"]
        .as_array()
        .map(|steps| steps.iter().filter_map(|st| st["value"].as_str().map(|v| v.trim().to_owned())).collect())
        .unwrap_or_default()
}

fn run_form(p: &Project, form: &str, program: &str, script: &Value) -> String {
    let script_path = p.root.join(format!("steps-{}.json", form.replace('/', "-")));
    std::fs::write(&script_path, script.to_string()).unwrap();
    let out = Command::new(rcrun())
        .args(["run-form", form, program, "--designer", "--headless", script_path.to_str().unwrap(), "--headless-limit", "60"])
        .current_dir(&p.root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form {form} failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    stdout
}

/// StatusCode 00, two items left (1 and 2), the first row Ana with 11.50.
fn judge(cell: &str, got: &[String]) -> bool {
    let want = ["00", "2", "00000001", "Ana", "11.50"];
    let ok = got.iter().map(String::as_str).eq(want.iter().copied());
    println!("    {cell:<44} {:<44} {}", got.join(" | "), if ok { "ok" } else { "DIFFERS" });
    ok
}

/// AC8 on `rcrun run-form` and as an embedded child form, on the three engines.
#[test]
fn partiql_gives_the_same_answers_on_two_hosts_and_three_engines() {
    let t0 = Instant::now();
    println!("\n  108 AC8 - PartiQL on an indexed file, hosts x engines");
    let mut cells = 0;
    let mut failed = 0;
    for engine in [Engine::Disk, Engine::Redb, Engine::Memory] {
        let t = Instant::now();
        let p = project(&format!("rf-{}", engine.name()), engine);
        let direct = readings(&run_form(&p, "forms/partiql-demo.cfrm", "generated/partiql-demo.cbl", &steps(None)));
        cells += 1;
        failed += usize::from(!judge(&format!("rcrun run-form / {} ({} ms)", engine.name(), t.elapsed().as_millis()), &direct));

        let t = Instant::now();
        let p = project(&format!("emb-{}", engine.name()), engine);
        let embedded = readings(&run_form(&p, "forms/aws-shell.cfrm", "generated/aws-shell.cbl", &steps(Some("partiql-demo"))));
        cells += 1;
        failed += usize::from(!judge(&format!("embedded child form / {} ({} ms)", engine.name(), t.elapsed().as_millis()), &embedded));
    }
    println!("  CELLS RUN      : {cells:04}");
    println!("  CELLS FAILED   : {failed:04}");
    println!("  ELAPSED        : {} ms", t0.elapsed().as_millis());
    println!("  OVERALL RESULT : {}\n", if failed == 0 { "PASS" } else { "FAIL" });
    assert_eq!(failed, 0);
}

/// The built binary gives the same answers on the three engines. Builds three: `--ignored`.
#[test]
#[ignore = "builds real binaries; run with --ignored"]
fn the_same_answers_in_a_built_binary() {
    let t0 = Instant::now();
    println!("\n  108 AC8 - PartiQL on an indexed file, the built binary x engines");
    let mut failed = 0;
    for engine in [Engine::Disk, Engine::Redb, Engine::Memory] {
        let p = project(&format!("bin-{}", engine.name()), engine);
        let ws = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
        let opts = cobolt_compiler::BuildOptions { verbose: false, workspace_root: Some(ws), debug: true, ..Default::default() };
        let built = cobolt_compiler::build_project(&p.root.join("partiqlhosts.project.toml"), &opts)
            .unwrap_or_else(|e| panic!("the build failed: {e}"));
        let script = p.root.join("bin-steps.json");
        std::fs::write(&script, steps(Some("partiql-demo")).to_string()).unwrap();
        let out = Command::new(&built.binary_path)
            .env("PRC_HEADLESS_SCRIPT", &script)
            .env("PRC_HEADLESS_LIMIT", "60")
            .current_dir(&p.root)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(out.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
        failed += usize::from(!judge(&format!("built binary / {}", engine.name()), &readings(&stdout)));
    }
    println!("  CELLS FAILED   : {failed:04}");
    println!("  ELAPSED        : {} ms\n", t0.elapsed().as_millis());
    assert_eq!(failed, 0);
}
