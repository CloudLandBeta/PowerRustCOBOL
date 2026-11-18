// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T41c (R10, R11, ABI A43): **the SqlDatabase control behaves on the WebAssembly
//! path as it does under the interpreter.**
//!
//! One form with three SqlDatabase controls: `DB-1` on `:memory:`, `DB-2` on `:memory:` with
//! `Mode = Async`, and `DB-3` naming the project's SQL connection `SALES` (`SqlConnection`).
//!
//! - **The Sync script**, run under a pinned clock and seed: `Open()` with no argument, `Execute`,
//!   `Query`, `ColumnName`, a `Fetch` loop, the transactions (`BeginTransaction`, `IsInTransaction`,
//!   `Rollback`, and a `Commit` with none open), a statement the database refuses, `LastError`, the
//!   named connection, and `Close()` followed by a statement on the closed handle. `DB-1`'s events
//!   (`onConnectOk`, `onQueryComplete`, `onQueryError`, `onRowFetched`) count themselves into Labels.
//! - **The Async script**, run in real time, because an asynchronous statement ends when its worker
//!   does: `Execute` answers 0 at once and the control is busy, a second statement while it runs is
//!   ignored, `onQueryComplete` reads `ResultCount`; a slow statement with `TimeoutMs = 50` raises
//!   `onTimeout` with `LastError`, and its late result is dropped; `Cancel()` lets one go
//!   (`onCancelled`, no longer busy); and the connection serves again once its worker is done.
//!
//! Each script runs headless under `rcrun run-form` (the interpreter) and in the program
//! `rcrun build` makes on the WebAssembly path. The dumps must be equal and the pictures equal in
//! every pixel.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

const FORM: &str = "PARITY-SQLDB";
const PIN_CLOCK: &str = "2026-01-02T03:04:05-03:00";
const PIN_SEED: &str = "7";

/// The events' tallies, seen by every handler.
const FORM_WS: &str = "       01 WS-OK                   GLOBAL PIC 9(2) VALUE 0.
       01 WS-QC                   GLOBAL PIC 9(2) VALUE 0.
       01 WS-QE                   GLOBAL PIC 9(2) VALUE 0.
       01 WS-RF                   GLOBAL PIC 9(2) VALUE 0.
       01 WS-ADONE                GLOBAL PIC 9(2) VALUE 0.";

/// A statement slow enough to outlast `TimeoutMs = 50` and a `Cancel()`.
const SLOW: &str = "WITH RECURSIVE C(N) AS (SELECT 1 UNION ALL SELECT N + 1 FROM C WHERE N < 1000000) SELECT COUNT(*) FROM C";

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

/// Bind `event` of control `id` to a handler.
fn bind(form: &mut Form, id: &str, event: &str, ws: &str, statements: &str) {
    let c = form.controls.iter_mut().find(|c| c.id == id).expect("a control of the fixture");
    c.events.push(EventBinding { event: event.into(), paragraph: format!("{id}--{}", event.to_ascii_uppercase()), code: handler(ws, statements) });
}

/// The statements of a handler, one per line.
fn lines(statements: &[&str]) -> String {
    statements.iter().map(|s| format!("           {s}")).collect::<Vec<_>>().join("\n")
}

/// The fixture project in `root`; its manifest's path. With `server`, `DB-1` names the project's SQL connection
/// `MAIN` instead of its `:memory:` ConnectionString, so the environment can point it, and `SALES`, at a server.
fn project(root: &Path, server: bool) -> PathBuf {
    let _ = fs::remove_dir_all(root);
    for d in ["forms", "generated", "src", "data"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    let s = |v: &str| PropValue::String(v.into());
    let mut form = Form::new(FORM, "SqlDatabase form", 720, 500);
    form.main_form = true;
    form.user_ws_source = FORM_WS.into();
    let buttons = ["BTN-OPEN", "BTN-SETUP", "BTN-QUERY", "BTN-TX", "BTN-BAD", "BTN-NAMED", "BTN-CLOSE", "BTN-AOPEN", "BTN-ASLOW", "BTN-ACANCEL", "BTN-AAFTER"];
    for (i, id) in buttons.iter().enumerate() {
        control(&mut form, id, ControlType::Button, (20, 20 + 38 * i as i32, 120, 28), &[("Caption", s(id.trim_start_matches("BTN-")))]);
    }
    let labels = [
        "LBL-OPEN", "LBL-SETUP", "LBL-ROWS", "LBL-COL", "LBL-ROW1", "LBL-TX", "LBL-COUNT", "LBL-ERR", "LBL-NAMED", "LBL-EVOK", "LBL-EVQC", "LBL-EVQE", "LBL-EVRF",
        "LBL-ASYNC", "LBL-ADONE", "LBL-ATIME", "LBL-ACANCEL",
    ];
    for (i, id) in labels.iter().enumerate() {
        control(&mut form, id, ControlType::Label, (160, 16 + 27 * i as i32, 540, 24), &[("Caption", s("-"))]);
    }
    let db1 = if server { ("SqlConnection", s("MAIN")) } else { ("ConnectionString", s(":memory:")) };
    control(&mut form, "DB-1", ControlType::SqlDatabase, (600, 400, 64, 64), &[db1]);
    control(&mut form, "DB-2", ControlType::SqlDatabase, (520, 400, 64, 64), &[("ConnectionString", s(":memory:")), ("Mode", s("Async"))]);
    control(&mut form, "DB-3", ControlType::SqlDatabase, (440, 400, 64, 64), &[("SqlConnection", s("SALES"))]);

    let n4 = "       01 WS-N                    PIC 9(4).";
    bind(&mut form, "BTN-OPEN", "onClick", "", &lines(&["MOVE DB-1::Open() TO LBL-OPEN::Caption"]));
    bind(
        &mut form,
        "BTN-SETUP",
        "onClick",
        n4,
        &lines(&[
            "MOVE DB-1::Execute(\"CREATE TABLE T (ID INTEGER, NAME TEXT)\") TO WS-N",
            "MOVE DB-1::Execute(\"INSERT INTO T VALUES (1, 'LAMP'), (2, 'CHAIR'), (3, 'TABLE')\") TO WS-N",
            "MOVE WS-N TO LBL-SETUP::Caption",
        ]),
    );
    bind(
        &mut form,
        "BTN-QUERY",
        "onClick",
        &format!("{n4}\n       01 WS-ROW                  PIC X(40)."),
        &lines(&[
            "MOVE DB-1::Query(\"SELECT ID, NAME FROM T ORDER BY ID\") TO WS-N",
            "MOVE WS-N TO LBL-ROWS::Caption",
            "MOVE DB-1::ColumnName(2) TO LBL-COL::Caption",
            "MOVE DB-1::Fetch() TO WS-ROW",
            "INSPECT WS-ROW REPLACING ALL X\"09\" BY \"|\"",
            "MOVE WS-ROW TO LBL-ROW1::Caption",
            "PERFORM UNTIL WS-ROW = SPACES",
            "    MOVE DB-1::Fetch() TO WS-ROW",
            "END-PERFORM",
        ]),
    );
    bind(
        &mut form,
        "BTN-TX",
        "onClick",
        &format!(
            "{n4}\n       01 WS-ROW                  PIC X(40).\n       01 WS-FLAGS.\n           05 WS-A                PIC 9.\n           05 WS-B                PIC 9.\n           05 WS-C                PIC 9.\n           05 WS-D                PIC 9.\n           05 WS-E                PIC 9."
        ),
        &lines(&[
            "MOVE DB-1::BeginTransaction() TO WS-A",
            "MOVE DB-1::Execute(\"DELETE FROM T WHERE ID = 3\") TO WS-N",
            "MOVE DB-1::IsInTransaction() TO WS-B",
            "MOVE DB-1::Rollback() TO WS-C",
            "MOVE DB-1::IsInTransaction() TO WS-D",
            "MOVE DB-1::Commit() TO WS-E",
            "MOVE WS-FLAGS TO LBL-TX::Caption",
            "MOVE DB-1::Query(\"SELECT COUNT(*) FROM T\") TO WS-N",
            "MOVE DB-1::Fetch() TO WS-ROW",
            "MOVE WS-ROW TO LBL-COUNT::Caption",
        ]),
    );
    bind(
        &mut form,
        "BTN-BAD",
        "onClick",
        n4,
        &lines(&["MOVE DB-1::Execute(\"SELEC 1\") TO WS-N", "MOVE DB-1::LastError TO LBL-ERR::Caption"]),
    );
    bind(
        &mut form,
        "BTN-NAMED",
        "onClick",
        &format!("{n4}\n       01 WS-H                    PIC 9(4).\n       01 WS-ROW                  PIC X(20).\n       01 WS-T                    PIC X(40)."),
        &lines(&[
            "MOVE DB-3::Open() TO WS-H",
            "MOVE DB-3::Execute(\"CREATE TABLE IF NOT EXISTS S (ID INTEGER)\") TO WS-N",
            "MOVE DB-3::Execute(\"INSERT INTO S VALUES (7)\") TO WS-N",
            "MOVE DB-3::Query(\"SELECT COUNT(*) FROM S\") TO WS-N",
            "MOVE DB-3::Fetch() TO WS-ROW",
            "STRING \"handle \" WS-H \" rows \" WS-ROW DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-NAMED::Caption",
        ]),
    );
    bind(
        &mut form,
        "BTN-CLOSE",
        "onClick",
        n4,
        &lines(&["DB-1::Close()", "MOVE DB-1::Query(\"SELECT 1\") TO WS-N", "MOVE DB-1::LastError TO LBL-ERR::Caption"]),
    );
    for (event, tally, label) in [("onConnectOk", "WS-OK", "LBL-EVOK"), ("onQueryComplete", "WS-QC", "LBL-EVQC"), ("onQueryError", "WS-QE", "LBL-EVQE"), ("onRowFetched", "WS-RF", "LBL-EVRF")] {
        bind(&mut form, "DB-1", event, "", &lines(&[&format!("ADD 1 TO {tally}"), &format!("MOVE {tally} TO {label}::Caption")]));
    }

    // ── Async: DB-2 ──
    bind(
        &mut form,
        "BTN-AOPEN",
        "onClick",
        "       01 WS-T.\n           05 WS-H                PIC 9(4).\n           05 FILLER              PIC X VALUE '-'.\n           05 WS-R                PIC 9.\n           05 WS-B                PIC 9.\n           05 WS-R2               PIC 9.",
        &lines(&[
            "MOVE DB-2::Open() TO WS-H",
            "MOVE DB-2::Execute(\"CREATE TABLE A (X INTEGER)\") TO WS-R",
            "MOVE DB-2::IsBusy() TO WS-B",
            "MOVE DB-2::Execute(\"INSERT INTO A VALUES (1)\") TO WS-R2",
            "MOVE WS-T TO LBL-ASYNC::Caption",
        ]),
    );
    bind(
        &mut form,
        "DB-2",
        "onQueryComplete",
        "       01 WS-C                    PIC 9(4).\n       01 WS-B                    PIC 9.\n       01 WS-T                    PIC X(40).",
        &lines(&[
            "ADD 1 TO WS-ADONE",
            "MOVE DB-2::ResultCount TO WS-C",
            "MOVE DB-2::IsBusy() TO WS-B",
            "STRING \"done \" WS-ADONE \" count \" WS-C \" busy \" WS-B DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-ADONE::Caption",
        ]),
    );
    bind(&mut form, "DB-2", "onTimeout", "", &lines(&["MOVE DB-2::LastError TO LBL-ATIME::Caption"]));
    bind(&mut form, "DB-2", "onCancelled", "", &lines(&["MOVE \"cancelled\" TO LBL-ACANCEL::Caption"]));
    bind(&mut form, "DB-2", "onQueryError", "", &lines(&["MOVE DB-2::LastError TO LBL-ERR::Caption"]));
    bind(
        &mut form,
        "BTN-ASLOW",
        "onClick",
        "       01 WS-R                    PIC 9.",
        &lines(&["MOVE 50 TO DB-2::TimeoutMs", &format!("MOVE DB-2::Query(\"{SLOW}\") TO WS-R")]),
    );
    bind(
        &mut form,
        "BTN-ACANCEL",
        "onClick",
        "       01 WS-R                    PIC 9.\n       01 WS-B                    PIC 9.\n       01 WS-T                    PIC X(20).",
        &lines(&[
            "MOVE 0 TO DB-2::TimeoutMs",
            &format!("MOVE DB-2::Query(\"{SLOW}\") TO WS-R"),
            "DB-2::Cancel()",
            "MOVE DB-2::IsBusy() TO WS-B",
            "STRING \"cancel busy \" WS-B DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-ASYNC::Caption",
        ]),
    );
    bind(&mut form, "BTN-AAFTER", "onClick", "       01 WS-R                    PIC 9.", &lines(&["MOVE DB-2::Query(\"SELECT COUNT(*) FROM A\") TO WS-R"]));

    let cfrm = root.join("forms/parity-sqldb.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/parity-sqldb.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    let manifest = root.join("paritysqldb.project.toml");
    fs::write(
        &manifest,
        format!(
            "[project]\nname = \"paritysqldb\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
             [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/parity-sqldb.cfrm\"]\ngenerated = [\"generated/parity-sqldb.cbl\"]\n\n\
             [[sql-connections]]\nname = \"SALES\"\npath = \"data/sales.db\"\ncreate-if-missing = true\n{}",
            if server { "\n[[sql-connections]]\nname = \"MAIN\"\npath = \"data/main.db\"\ncreate-if-missing = true\n" } else { "" }
        ),
    )
    .unwrap();
    manifest
}

/// The two scripts, each with the names of its pictures in order, and whether it runs under the pins.
fn scripts() -> Vec<(&'static str, Value, Vec<&'static str>, bool)> {
    let click = |id: &str| json!({"event": {"control": id, "name": "onClick"}});
    let wait = |ms: u64| json!({"wait_ms": ms});
    let dump = || json!({"dump": true});
    let shot = |name: &str| json!({"shot": name});
    let sync = vec!["s1-setup", "s2-query", "s3-transactions", "s4-named", "s5-closed"];
    let sync_steps = vec![
        wait(200),
        click("BTN-OPEN"),
        click("BTN-SETUP"),
        dump(),
        shot(sync[0]),
        click("BTN-QUERY"),
        dump(),
        shot(sync[1]),
        click("BTN-TX"),
        dump(),
        shot(sync[2]),
        click("BTN-BAD"),
        click("BTN-NAMED"),
        dump(),
        shot(sync[3]),
        click("BTN-CLOSE"),
        dump(),
        shot(sync[4]),
    ];
    let asynchronous = vec!["a1-created", "a2-timed-out", "a3-after"];
    let async_steps = vec![
        wait(200),
        click("BTN-AOPEN"),
        wait(1500),
        dump(),
        shot(asynchronous[0]),
        click("BTN-ASLOW"),
        wait(4000),
        dump(),
        shot(asynchronous[1]),
        click("BTN-ACANCEL"),
        wait(4000),
        click("BTN-AAFTER"),
        wait(1500),
        dump(),
        shot(asynchronous[2]),
    ];
    vec![("sync", Value::Array(sync_steps), sync, true), ("async", Value::Array(async_steps), asynchronous, false)]
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

/// Run every script with `run` (the script's path, whether it is pinned): the dumps, in order.
fn run_all(scripts: &[(PathBuf, bool)], mut run: impl FnMut(&Path, bool) -> String) -> Vec<Value> {
    let mut out = Vec::new();
    for (path, pinned) in scripts {
        out.extend(dumps(&report(&run(path, *pinned))));
    }
    out
}

/// Which engine is about to run.
#[derive(Clone, Copy, PartialEq)]
enum Engine {
    Interpreter,
    Runner,
}

/// What both engines did with the same scripts.
struct Twin {
    interp: Vec<Value>,
    wasm: Vec<Value>,
    interp_shots: PathBuf,
    runner_shots: PathBuf,
    runner_ms: f64,
    build_ms: f64,
    interp_ms: f64,
    wasm_ms: f64,
}

/// Run `paths` under `rcrun run-form` (the interpreter), then build the project on the WebAssembly
/// path and run them in the built program. `env` is given to both; `before` runs before each engine.
fn run_twin(root: &Path, manifest: &Path, paths: &[(PathBuf, bool)], env: &[(String, String)], before: &dyn Fn(Engine)) -> Twin {
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
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

    // ── 1. The interpreter: rcrun run-form ──
    before(Engine::Interpreter);
    let interp_shots = root.join("shots-interpreter");
    fs::create_dir_all(&interp_shots).unwrap();
    let t = Instant::now();
    let interp = run_all(paths, |script, pinned| {
        let mut c = Command::new(rcrun());
        c.args(["run-form", "forms/parity-sqldb.cfrm", "generated/parity-sqldb.cbl", "--designer", "--headless"])
            .arg(script)
            .args(["--headless-limit", "60"])
            .current_dir(root)
            .env("HOME", &home)
            .env("PRC_HEADLESS_SHOTS", &interp_shots)
            .envs(env.iter().map(|(k, v)| (k, v)));
        if pinned {
            c.env("PRC_TEST_CLOCK", PIN_CLOCK).env("PRC_TEST_SEED", PIN_SEED);
        }
        let out = c.output().expect("run rcrun run-form");
        let said = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(out.status.success(), "rcrun run-form failed:\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
        said
    });
    let interp_ms = t.elapsed().as_secs_f64() * 1000.0;

    // ── 2. The runner: rcrun build on the WebAssembly path, then the program ──
    before(Engine::Runner);
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
    let exe = root.join("bin").join(format!("paritysqldb{}", std::env::consts::EXE_SUFFIX));
    let runner_shots = root.join("shots-runner");
    fs::create_dir_all(&runner_shots).unwrap();
    let t = Instant::now();
    let wasm = run_all(paths, |script, pinned| {
        let mut c = Command::new(&exe);
        c.current_dir(root)
            .env("HOME", &home)
            .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
            .env("PRC_HEADLESS_SCRIPT", script)
            .env("PRC_HEADLESS_LIMIT", "60")
            .env("PRC_HEADLESS_SHOTS", &runner_shots)
            .envs(env.iter().map(|(k, v)| (k, v)));
        if pinned {
            c.env("PRC_TEST_CLOCK", PIN_CLOCK).env("PRC_TEST_SEED", PIN_SEED);
        }
        let out = c.output().expect("run the built program");
        let said = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(out.status.success(), "the built program failed:\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
        said
    });
    let wasm_ms = t.elapsed().as_secs_f64() * 1000.0;
    Twin { interp, wasm, interp_shots, runner_shots, runner_ms, build_ms, interp_ms, wasm_ms }
}

/// The comparison of the two engines' dumps and pictures: the checks, the lines that report them,
/// and the pixels compared and differing.
struct Comparison {
    checks: Vec<(String, bool)>,
    dump_lines: Vec<String>,
    shot_lines: Vec<String>,
    pixels_total: u64,
    differing_total: u64,
}

fn compare(twin: &Twin, shots: &[&str]) -> Comparison {
    let mut checks: Vec<(String, bool)> = Vec::new();
    checks.push((format!("both engines dump {} times", shots.len()), twin.interp.len() == shots.len() && twin.wasm.len() == shots.len()));
    let mut dump_lines = Vec::new();
    for (i, (x, y)) in twin.interp.iter().zip(twin.wasm.iter()).enumerate() {
        let diff = dump_differences(x, y);
        checks.push((format!("dump {i} ({}) is equal", shots[i]), diff.is_empty()));
        dump_lines.extend(diff.into_iter().map(|d| format!("    dump {i}: {d}")));
    }
    let (mut pixels_total, mut differing_total, mut shot_lines) = (0u64, 0u64, Vec::new());
    for name in shots {
        let file = format!("{name}.png");
        let (diff, (w, h)) = differing_pixels(&twin.interp_shots.join(&file), &twin.runner_shots.join(&file));
        pixels_total += u64::from(w) * u64::from(h);
        differing_total = differing_total.saturating_add(diff);
        checks.push((format!("picture {name}: 0 differing pixels"), diff == 0));
        shot_lines.push(format!("    {name}: {w}x{h}, {} differing pixels", if diff == u64::MAX { "missing,".to_string() } else { diff.to_string() }));
    }
    Comparison { checks, dump_lines, shot_lines, pixels_total, differing_total }
}

/// What the sync script's statements did, read from the runner's first five dumps. `server`: `DB-1`
/// opens the named connection `MAIN` (on a server) instead of `:memory:`.
fn sync_checks(wasm: &[Value], server: bool) -> Vec<(String, bool)> {
    let at = |i: usize, id: &str| wasm.get(i).map(|d| caption(d, id)).unwrap_or_default();
    let number = |s: String| s.trim().trim_start_matches('0').to_string();
    let opened = if server { "Open() with no argument opens SqlConnection MAIN" } else { "Open() with no argument opens ConnectionString" };
    vec![
        (format!("{opened}: handle 1 (LBL-OPEN 1)"), number(at(0, "LBL-OPEN")) == "1"),
        ("Execute answers the rows it inserted (LBL-SETUP 3)".into(), number(at(0, "LBL-SETUP")) == "3"),
        ("onConnectOk once, onQueryComplete twice".into(), number(at(0, "LBL-EVOK")) == "1" && number(at(0, "LBL-EVQC")) == "2"),
        ("Query answers its row count (LBL-ROWS 3)".into(), number(at(1, "LBL-ROWS")) == "3"),
        ("ColumnName(2) is NAME".into(), at(1, "LBL-COL").trim() == "NAME"),
        ("Fetch() hands back the row, tab-separated (LBL-ROW1 1|LAMP)".into(), at(1, "LBL-ROW1").trim() == "1|LAMP"),
        ("onRowFetched once per row (LBL-EVRF 3)".into(), number(at(1, "LBL-EVRF")) == "3"),
        ("Begin 1, in a transaction 1, Rollback 1, none 0, Commit with none open 0 (LBL-TX 11100)".into(), at(2, "LBL-TX").trim() == "11100"),
        ("the rolled-back DELETE left three rows (LBL-COUNT 3)".into(), at(2, "LBL-COUNT").trim() == "3"),
        ("the refused Commit raised onQueryError (LBL-EVQE 1)".into(), number(at(2, "LBL-EVQE")) == "1"),
        ("a statement the database refuses: LastError names it".into(), at(3, "LBL-ERR").contains("syntax")),
        ("SqlConnection SALES opens the named connection, handle 2, one row".into(), at(3, "LBL-NAMED").trim() == "handle 0002 rows 1"),
        ("after Close(), the handle is gone".into(), at(4, "LBL-ERR").contains("No open connection with handle 1")),
    ]
}

/// Print a result block and fail on any failed check; the work folder is removed when all pass.
fn finish(title: &str, header: &[String], twin: &Twin, cmp: Comparison, mut checks: Vec<(String, bool)>, root: &Path) {
    let mut all = cmp.checks;
    all.append(&mut checks);
    let passed = all.iter().filter(|(_, ok)| *ok).count();
    println!("== {title} ==");
    for l in header {
        println!("{l}");
    }
    println!("runner built in {:.0} ms; rcrun build (WebAssembly path) {:.0} ms", twin.runner_ms, twin.build_ms);
    println!("interpreter runs {:.0} ms; runner runs {:.0} ms", twin.interp_ms, twin.wasm_ms);
    for l in &cmp.shot_lines {
        println!("{l}");
    }
    println!("pixels compared: {}, differing: {}", cmp.pixels_total, cmp.differing_total);
    for l in &cmp.dump_lines {
        println!("{l}");
    }
    for (what, ok) in &all {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("result: {passed} of {} checks passed", all.len());
    let failed: Vec<&String> = all.iter().filter(|(_, ok)| !*ok).map(|(w, _)| w).collect();
    if failed.is_empty() {
        let _ = fs::remove_dir_all(root);
    } else {
        println!("kept for inspection: {}", root.display());
    }
    assert!(failed.is_empty(), "failed: {failed:?}");
}

/// Write each script beside the project: its path, and whether it runs under the pins.
fn write_scripts(root: &Path, scripts: &[(&'static str, Value, Vec<&'static str>, bool)]) -> Vec<(PathBuf, bool)> {
    scripts
        .iter()
        .map(|(name, steps, _, pinned)| {
            let p = root.join(format!("{name}.json"));
            fs::write(&p, steps.to_string()).unwrap();
            (p, *pinned)
        })
        .collect()
}

#[test]
fn the_sqldatabase_control_agrees_on_both_engines() {
    let root = std::env::temp_dir().join(format!("prc-t41c-form-{}", std::process::id()));
    let manifest = project(&root, false);
    let scripts = scripts();
    let paths = write_scripts(&root, &scripts);
    let db = root.join("data/sales.db");
    let twin = run_twin(&root, &manifest, &paths, &[], &|engine| {
        if engine == Engine::Runner {
            assert!(db.is_file(), "the interpreter's run made the database {}", db.display());
            fs::remove_file(&db).unwrap();
        }
    });
    let shots: Vec<&str> = scripts.iter().flat_map(|(_, _, s, _)| s.iter().copied()).collect();
    let cmp = compare(&twin, &shots);
    let at = |i: usize, id: &str| twin.wasm.get(i).map(|d| caption(d, id)).unwrap_or_default();
    let mut checks = sync_checks(&twin.wasm, false);
    checks.push(("Async Execute answers 0 at once, busy; a second while busy is ignored (LBL-ASYNC 0001-010)".into(), at(5, "LBL-ASYNC").trim() == "0001-010"));
    checks.push(("onQueryComplete once, ResultCount 0, no longer busy".into(), at(5, "LBL-ADONE").trim() == "done 01 count 0000 busy 0"));
    checks.push(("TimeoutMs 50: onTimeout, LastError says so".into(), at(6, "LBL-ATIME").trim() == "No answer within 1 second: the call was cancelled."));
    checks.push(("the timed-out statement's late result is dropped".into(), at(6, "LBL-ADONE").trim() == "done 01 count 0000 busy 0"));
    checks.push(("Cancel(): onCancelled, no longer busy".into(), at(7, "LBL-ACANCEL").trim() == "cancelled" && at(7, "LBL-ASYNC").trim() == "cancel busy 0"));
    checks.push(("the connection serves again: the ignored INSERT left A empty, one row counted".into(), at(7, "LBL-ADONE").trim() == "done 02 count 0001 busy 0"));
    let header = vec![
        format!("form {FORM}: 3 SqlDatabase controls (:memory: Sync, :memory: Async, SqlConnection SALES), 11 buttons, 9 event handlers"),
        "  Sync: Open(), Execute, Query, ColumnName, Fetch loop, BeginTransaction / IsInTransaction / Rollback / Commit, LastError,".into(),
        "        the named connection, Close(); onConnectOk, onQueryComplete, onQueryError, onRowFetched".into(),
        "  Async: Execute answers 0 and busy, a second ignored, onQueryComplete with ResultCount, TimeoutMs 50 and onTimeout,".into(),
        "        Cancel() and onCancelled, the connection serving again".into(),
        format!("sync script pinned (PRC_TEST_CLOCK={PIN_CLOCK} PRC_TEST_SEED={PIN_SEED}); async script in real time; {} dumps, {} pictures", shots.len(), shots.len()),
    ];
    finish("the_sqldatabase_control_agrees_on_both_engines (DESKTOP-WASM-001 T41c: R10, R11, ABI A43)", &header, &twin, cmp, checks, &root);
}

/// The same form and Sync script on a MySQL server (fix 2.0.597): `DB-1` and `DB-3` name the project's
/// SQL connections `MAIN` and `SALES`, which the environment points at the server
/// (`PARITYSQLDB_SQL_<NAME>_URL`), so the server's address never enters the form. The tables are
/// dropped before each engine runs. `SKIPPED` without `PRC_TEST_MYSQL_URL`.
#[test]
#[ignore = "needs a MySQL server: PRC_TEST_MYSQL_URL"]
fn the_sqldatabase_control_agrees_on_mysql() {
    let Some(url) = std::env::var("PRC_TEST_MYSQL_URL").ok().filter(|u| !u.trim().is_empty()) else {
        println!("SKIPPED: PRC_TEST_MYSQL_URL is not set");
        return;
    };
    let root = std::env::temp_dir().join(format!("prc-t41c-mysql-{}", std::process::id()));
    let manifest = project(&root, true);
    let scripts: Vec<_> = scripts().into_iter().filter(|(name, ..)| *name == "sync").collect();
    let paths = write_scripts(&root, &scripts);
    let env: Vec<(String, String)> = ["MAIN", "SALES"]
        .iter()
        .map(|name| (cobolt_forms::connections::sql_env_var("paritysqldb", name, "URL"), url.clone()))
        .collect();
    let drop_tables = |_: Engine| {
        let mut db = cobolt_runtime::db_runtime::DbRegistry::new();
        let h = db.open(&url).unwrap_or_else(|e| panic!("the MySQL server: {e}"));
        for t in ["T", "S"] {
            db.exec(h, &format!("DROP TABLE IF EXISTS {t}")).unwrap_or_else(|e| panic!("DROP TABLE {t}: {e}"));
        }
        db.close(h);
    };
    let twin = run_twin(&root, &manifest, &paths, &env, &drop_tables);
    drop_tables(Engine::Runner);
    let shots: Vec<&str> = scripts.iter().flat_map(|(_, _, s, _)| s.iter().copied()).collect();
    let cmp = compare(&twin, &shots);
    let checks = sync_checks(&twin.wasm, true);
    let header = vec![
        format!("form {FORM}: DB-1 SqlConnection MAIN and DB-3 SqlConnection SALES, both on the MySQL server; the Sync script"),
        format!("sync script pinned (PRC_TEST_CLOCK={PIN_CLOCK} PRC_TEST_SEED={PIN_SEED}); {} dumps, {} pictures", shots.len(), shots.len()),
    ];
    finish("the_sqldatabase_control_agrees_on_mysql (fix 2.0.597; DESKTOP-WASM-001 T41c)", &header, &twin, cmp, checks, &root);
}
