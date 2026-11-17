// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T41 (R10, R11, ABI A41): **`EXEC SQL` in a form's handlers behaves on the
//! WebAssembly path as it does under the interpreter.**
//!
//! One form whose handlers connect to the project's SQL connection `SALES`, create and fill a
//! table, insert a row from a TextBox, count and total the rows, list them through a cursor the
//! form declares in its WORKING-STORAGE, prepare a statement in one handler and execute it in
//! another, and run a statement that fails. `SQLSTATE`, the inserted name and the prepared text
//! are the form's `GLOBAL` items; the counts are the handlers' own. Two scripts run, the second
//! after the first has ended: it counts the rows again, so it sees what the end of the first run
//! committed.
//!
//! Two more handlers use the SQL built-ins (T41b, ABI A42) in the second script: one opens the
//! connection `SALES` by name with `COBOL::"OPEN-DB"` into a `GLOBAL` handle, the other counts the
//! committed rows through that handle with `"EXEC-SQL"` and `"FETCH-ROW"`.
//!
//! Both scripts run headless under the same pinned clock and seed:
//!
//! 1. under `rcrun run-form`, the interpreter, with the project's connections;
//! 2. in the program `rcrun build` makes on the WebAssembly path, with the `sql-connections.toml`
//!    the Build writes beside it.
//!
//! The dumps must be equal and the pictures equal in every pixel.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

const FORM: &str = "PARITY-SQL";
const PIN_CLOCK: &str = "2026-01-02T03:04:05-03:00";
const PIN_SEED: &str = "7";

/// The form's own items, seen by every handler, and the cursor it declares.
const FORM_WS: &str = "       01 SQLSTATE                GLOBAL PIC X(5).
       01 WS-NAME                 GLOBAL PIC X(12).
       01 WS-SQL                  GLOBAL PIC X(60).
       01 WS-DBH                  GLOBAL PIC 9(9).
       01 WS-DBST                 GLOBAL PIC X(80).
           EXEC SQL DECLARE C-ITEMS CURSOR FOR
               SELECT NAME FROM ITEMS ORDER BY ID
           END-EXEC.";

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

/// A handler with its own WORKING-STORAGE items (`ws`, possibly empty).
fn handler(ws: &str, statements: &str) -> String {
    let ws = if ws.is_empty() { String::new() } else { format!("       WORKING-STORAGE SECTION.\n{ws}\n") };
    format!("       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n{ws}       PROCEDURE DIVISION.\n{statements}\n")
}

fn control(form: &mut Form, id: &str, kind: ControlType, rect: (i32, i32, i32, i32), props: &[(&str, PropValue)]) {
    let mut c = Control::new(id, kind, rect.0, rect.1);
    c.rect = Rect::new(rect.0, rect.1, rect.2, rect.3);
    for (k, v) in props {
        c.set_prop(*k, v.clone());
    }
    form.controls.push(c);
}

fn bind(form: &mut Form, id: &str, ws: &str, statements: &str) {
    let c = form.controls.iter_mut().find(|c| c.id == id).expect("a control of the fixture");
    c.events.push(EventBinding { event: "onClick".into(), paragraph: format!("{id}--ONCLICK"), code: handler(ws, statements) });
}

/// The fixture project in `root`; its manifest's path.
fn project(root: &Path) -> PathBuf {
    let _ = fs::remove_dir_all(root);
    for d in ["forms", "generated", "src", "data"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    let s = |v: &str| PropValue::String(v.into());
    let mut form = Form::new(FORM, "SQL form", 640, 420);
    form.main_form = true;
    form.user_ws_source = FORM_WS.into();
    let buttons = ["BTN-OPEN", "BTN-SETUP", "BTN-ADD", "BTN-COUNT", "BTN-LIST", "BTN-PREP", "BTN-RUN", "BTN-BAD", "BTN-DBOPEN", "BTN-DBCOUNT"];
    for (i, id) in buttons.iter().enumerate() {
        let caption = id.trim_start_matches("BTN-");
        control(&mut form, id, ControlType::Button, (20, 20 + 36 * i as i32, 120, 28), &[("Caption", s(caption))]);
    }
    control(&mut form, "TXT-NAME", ControlType::TextBox, (160, 92, 160, 28), &[("Text", s(""))]);
    for (i, id) in ["LBL-STATE", "LBL-COUNT", "LBL-TOTAL", "LBL-LISTED", "LBL-RUN", "LBL-DB"].iter().enumerate() {
        control(&mut form, id, ControlType::Label, (340, 20 + 36 * i as i32, 260, 24), &[("Caption", s("-"))]);
    }
    control(&mut form, "LST-ITEMS", ControlType::ListBox, (160, 200, 160, 120), &[]);

    bind(&mut form, "BTN-OPEN", "", "           EXEC SQL CONNECT TO 'SALES' END-EXEC\n           MOVE SQLSTATE TO LBL-STATE::Caption");
    bind(
        &mut form,
        "BTN-SETUP",
        "",
        "           EXEC SQL CREATE TABLE IF NOT EXISTS ITEMS\n\
         \x20              (ID INTEGER, NAME TEXT, PRICE DECIMAL(7,2))\n\
         \x20          END-EXEC\n\
         \x20          EXEC SQL INSERT INTO ITEMS VALUES (1, 'LAMP', 10.50)\n\
         \x20          END-EXEC\n\
         \x20          EXEC SQL INSERT INTO ITEMS VALUES (2, 'CHAIR', 120.05)\n\
         \x20          END-EXEC\n\
         \x20          EXEC SQL INSERT INTO ITEMS VALUES (3, 'TABLE', 358.40)\n\
         \x20          END-EXEC\n\
         \x20          MOVE SQLSTATE TO LBL-STATE::Caption",
    );
    bind(
        &mut form,
        "BTN-ADD",
        "       01 WS-ID                   PIC 9(4) VALUE 4.",
        "           MOVE TXT-NAME::Text TO WS-NAME\n\
         \x20          EXEC SQL INSERT INTO ITEMS VALUES (:WS-ID, :WS-NAME, 9.95)\n\
         \x20          END-EXEC\n\
         \x20          MOVE SQLSTATE TO LBL-STATE::Caption",
    );
    bind(
        &mut form,
        "BTN-COUNT",
        "       01 WS-N                    PIC 9(4).\n\
         \x20      01 WS-TOTAL                PIC 9(5)V99.\n\
         \x20      01 WS-TOTAL-ED             PIC ZZZZ9.99.",
        "           EXEC SQL SELECT COUNT(*), SUM(PRICE) INTO :WS-N, :WS-TOTAL\n\
         \x20              FROM ITEMS\n\
         \x20          END-EXEC\n\
         \x20          MOVE WS-TOTAL TO WS-TOTAL-ED\n\
         \x20          MOVE WS-N TO LBL-COUNT::Caption\n\
         \x20          MOVE WS-TOTAL-ED TO LBL-TOTAL::Caption",
    );
    bind(
        &mut form,
        "BTN-LIST",
        "",
        "           EXEC SQL OPEN C-ITEMS END-EXEC\n\
         \x20          PERFORM UNTIL SQLSTATE NOT = \"00000\"\n\
         \x20              EXEC SQL FETCH C-ITEMS INTO :WS-NAME END-EXEC\n\
         \x20              IF SQLSTATE = \"00000\"\n\
         \x20                  LST-ITEMS::AddItem( WS-NAME )\n\
         \x20              END-IF\n\
         \x20          END-PERFORM\n\
         \x20          EXEC SQL CLOSE C-ITEMS END-EXEC\n\
         \x20          MOVE LST-ITEMS::GetCount() TO LBL-LISTED::Caption",
    );
    bind(
        &mut form,
        "BTN-PREP",
        "",
        "           MOVE \"DELETE FROM ITEMS WHERE ID = 2\" TO WS-SQL\n\
         \x20          EXEC SQL PREPARE S-DEL FROM :WS-SQL END-EXEC\n\
         \x20          MOVE SQLSTATE TO LBL-STATE::Caption",
    );
    bind(&mut form, "BTN-RUN", "", "           EXEC SQL EXECUTE S-DEL END-EXEC\n           MOVE SQLSTATE TO LBL-RUN::Caption");
    bind(&mut form, "BTN-BAD", "", "           EXEC SQL DELETE FROM NOWHERE END-EXEC\n           MOVE SQLSTATE TO LBL-STATE::Caption");
    // T41b: the SQL built-ins, the connection opened by name in one handler and used in another.
    bind(
        &mut form,
        "BTN-DBOPEN",
        "",
        "           COBOL::\"OPEN-DB\" ( \"sql-connection:SALES\" WS-DBH WS-DBST )\n\
         \x20          MOVE WS-DBH TO LBL-DB::Caption",
    );
    bind(
        &mut form,
        "BTN-DBCOUNT",
        "       01 WS-C                    PIC X(10).",
        "           COBOL::\"EXEC-SQL\" ( WS-DBH \"SELECT COUNT(*) FROM ITEMS\" WS-C WS-DBST )\n\
         \x20          COBOL::\"FETCH-ROW\" ( WS-DBH 1 WS-C WS-DBST )\n\
         \x20          MOVE WS-C TO LBL-DB::Caption",
    );

    let cfrm = root.join("forms/parity-sql.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/parity-sql.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    let manifest = root.join("paritysql.project.toml");
    fs::write(
        &manifest,
        "[project]\nname = \"paritysql\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/parity-sql.cfrm\"]\ngenerated = [\"generated/parity-sql.cbl\"]\n\n\
         [[sql-connections]]\nname = \"SALES\"\npath = \"data/sales.db\"\ncreate-if-missing = true\n",
    )
    .unwrap();
    manifest
}

/// The two scripts, and the names of their pictures in order.
fn scripts() -> ((Value, Vec<&'static str>), (Value, Vec<&'static str>)) {
    let click = |id: &str| json!({"event": {"control": id, "name": "onClick"}});
    let dump = || json!({"dump": true});
    let shot = |name: &str| json!({"shot": name});
    let first = vec!["01-added", "02-listed", "03-deleted", "04-error"];
    let steps = vec![
        json!({"wait_ms": 200}),
        click("BTN-OPEN"),
        click("BTN-SETUP"),
        json!({"set": {"control": "TXT-NAME", "property": "Text", "value": "BOB"}}),
        click("BTN-ADD"),
        click("BTN-COUNT"),
        dump(),
        shot(first[0]),
        click("BTN-LIST"),
        dump(),
        shot(first[1]),
        click("BTN-PREP"),
        click("BTN-RUN"),
        click("BTN-COUNT"),
        dump(),
        shot(first[2]),
        click("BTN-BAD"),
        dump(),
        shot(first[3]),
    ];
    let second = vec!["05-reopened", "06-db-opened", "07-db-counted"];
    let again = vec![
        json!({"wait_ms": 200}),
        click("BTN-OPEN"),
        click("BTN-COUNT"),
        dump(),
        shot(second[0]),
        click("BTN-DBOPEN"),
        dump(),
        shot(second[1]),
        click("BTN-DBCOUNT"),
        dump(),
        shot(second[2]),
    ];
    ((Value::Array(steps), first), (Value::Array(again), second))
}

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

/// The keys whose values differ between two dumps, as `control.key`.
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

/// A Label's caption in a dump.
fn caption(dump: &Value, id: &str) -> String {
    let props = &dump[id]["props"];
    props.get("CAPTION").or_else(|| props.get("Caption")).and_then(Value::as_str).unwrap_or("").to_string()
}

#[test]
fn exec_sql_in_handlers_agrees_on_both_engines() {
    let root = std::env::temp_dir().join(format!("prc-t41-form-{}", std::process::id()));
    let manifest = project(&root);
    let ((first, first_shots), (second, second_shots)) = scripts();
    let (first_path, second_path) = (root.join("first.json"), root.join("second.json"));
    fs::write(&first_path, first.to_string()).unwrap();
    fs::write(&second_path, second.to_string()).unwrap();
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    let db = root.join("data/sales.db");

    // ── The runner, built beside rcrun, as a release has it ──
    let t = Instant::now();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new(&cargo)
        .current_dir(&workspace)
        .args(["build", "-q", "-p", "cobolt-runner", "--bin", "prc-runner"])
        .status()
        .expect("run cargo");
    assert!(status.success(), "cargo build -p cobolt-runner --bin prc-runner failed");
    let runner_ms = t.elapsed().as_secs_f64() * 1000.0;

    // ── 1. The interpreter: rcrun run-form, the two scripts one after the other ──
    let interp_shots = root.join("shots-interpreter");
    fs::create_dir_all(&interp_shots).unwrap();
    let mut interp = Vec::new();
    let t = Instant::now();
    for script in [&first_path, &second_path] {
        let out = Command::new(rcrun())
            .args(["run-form", "forms/parity-sql.cfrm", "generated/parity-sql.cbl", "--designer", "--headless"])
            .arg(script)
            .args(["--headless-limit", "60"])
            .current_dir(&root)
            .env("HOME", &home)
            .env("PRC_TEST_CLOCK", PIN_CLOCK)
            .env("PRC_TEST_SEED", PIN_SEED)
            .env("PRC_HEADLESS_SHOTS", &interp_shots)
            .output()
            .expect("run rcrun run-form");
        let said = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(out.status.success(), "rcrun run-form failed:\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
        interp.extend(dumps(&report(&said)));
    }
    let interp_ms = t.elapsed().as_secs_f64() * 1000.0;
    assert!(db.is_file(), "the interpreter's run made the database {}", db.display());
    fs::remove_file(&db).unwrap();

    // ── 2. The runner: rcrun build on the WebAssembly path, then the program, the same two scripts ──
    let t = Instant::now();
    let out = Command::new(rcrun())
        .arg("build")
        .arg(&manifest)
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
    let exe = root.join("bin").join(format!("paritysql{}", std::env::consts::EXE_SUFFIX));
    assert!(root.join("bin").join("sql-connections.toml").is_file(), "the Build wrote the connections beside the program");
    let runner_shots = root.join("shots-runner");
    fs::create_dir_all(&runner_shots).unwrap();
    let mut wasm = Vec::new();
    let t = Instant::now();
    for script in [&first_path, &second_path] {
        let out = Command::new(&exe)
            .current_dir(&root)
            .env("HOME", &home)
            .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
            .env("PRC_TEST_CLOCK", PIN_CLOCK)
            .env("PRC_TEST_SEED", PIN_SEED)
            .env("PRC_HEADLESS_SCRIPT", script)
            .env("PRC_HEADLESS_LIMIT", "60")
            .env("PRC_HEADLESS_SHOTS", &runner_shots)
            .output()
            .expect("run the built program");
        let said = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(out.status.success(), "the built program failed:\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
        wasm.extend(dumps(&report(&said)));
    }
    let wasm_ms = t.elapsed().as_secs_f64() * 1000.0;

    // ── The comparison ──
    let shots: Vec<&str> = first_shots.iter().chain(second_shots.iter()).copied().collect();
    let mut checks: Vec<(String, bool)> = Vec::new();
    checks.push((format!("both engines dump {} times", shots.len()), interp.len() == shots.len() && wasm.len() == shots.len()));
    let mut dump_lines = Vec::new();
    for (i, (x, y)) in interp.iter().zip(wasm.iter()).enumerate() {
        let diff = dump_differences(x, y);
        checks.push((format!("dump {i} ({}) is equal", shots[i]), diff.is_empty()));
        dump_lines.extend(diff.into_iter().map(|d| format!("    dump {i}: {d}")));
    }
    let (mut pixels_total, mut differing_total, mut shot_lines) = (0u64, 0u64, Vec::new());
    for name in &shots {
        let file = format!("{name}.png");
        let (diff, (w, h)) = differing_pixels(&interp_shots.join(&file), &runner_shots.join(&file));
        pixels_total += u64::from(w) * u64::from(h);
        differing_total = differing_total.saturating_add(diff);
        checks.push((format!("picture {name}: 0 differing pixels"), diff == 0));
        shot_lines.push(format!("    {name}: {w}x{h}, {} differing pixels", if diff == u64::MAX { "missing,".to_string() } else { diff.to_string() }));
    }
    // What the runner's program did, read from its dumps.
    let at = |i: usize, id: &str| wasm.get(i).map(|d| caption(d, id)).unwrap_or_default();
    let number = |s: String| s.trim().trim_start_matches('0').to_string();
    checks.push(("CONNECT, CREATE and INSERT succeed (LBL-STATE 00000)".into(), at(0, "LBL-STATE") == "00000"));
    checks.push(("four rows after the TextBox's insert (LBL-COUNT 4)".into(), number(at(0, "LBL-COUNT")) == "4"));
    checks.push(("SUM(PRICE) into an implied-decimal item (LBL-TOTAL 498.90)".into(), at(0, "LBL-TOTAL").trim() == "498.90"));
    checks.push(("the form's cursor lists four names (LBL-LISTED 4)".into(), number(at(1, "LBL-LISTED")) == "4"));
    checks.push(("a statement prepared in one handler runs in another (LBL-RUN 00000)".into(), at(2, "LBL-RUN") == "00000"));
    checks.push(("three rows after the prepared DELETE (LBL-COUNT 3)".into(), number(at(2, "LBL-COUNT")) == "3"));
    checks.push(("a failing statement sets SQLSTATE class 42 and the handler goes on".into(), at(3, "LBL-STATE").starts_with("42")));
    checks.push(("the next run sees the three rows the first run's end committed".into(), number(at(4, "LBL-COUNT")) == "3"));
    checks.push(("OPEN-DB 'sql-connection:SALES' in a handler: handle 1 (LBL-DB 1)".into(), number(at(5, "LBL-DB")) == "1"));
    checks.push(("another handler counts through that handle: three rows (LBL-DB 3)".into(), at(6, "LBL-DB").trim() == "3"));

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== exec_sql_in_handlers_agrees_on_both_engines (DESKTOP-WASM-001 T41, T41b: R10, R11, ABI A41, A42) ==");
    println!("form {FORM}: 10 handlers; EXEC SQL CONNECT, CREATE TABLE, INSERT (literals; host variables from a GLOBAL item and the");
    println!("  handler's own), SELECT COUNT/SUM INTO, a cursor declared in the form's WORKING-STORAGE (OPEN, FETCH loop, CLOSE),");
    println!("  PREPARE in one handler and EXECUTE in another, a failing DELETE; SQLSTATE a GLOBAL item of the form");
    println!("  the SQL built-ins: OPEN-DB of 'sql-connection:SALES' into a GLOBAL handle, EXEC-SQL and FETCH-ROW through it in another handler");
    println!("pins: PRC_TEST_CLOCK={PIN_CLOCK} PRC_TEST_SEED={PIN_SEED}; 2 scripts, {} dumps, {} pictures", shots.len(), shots.len());
    println!("runner built in {runner_ms:.0} ms; rcrun build (WebAssembly path) {build_ms:.0} ms");
    println!("interpreter runs {interp_ms:.0} ms; runner runs {wasm_ms:.0} ms");
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
        println!("kept for inspection: {}", root.display());
    }
    assert!(failed.is_empty(), "failed: {failed:?}");
}
