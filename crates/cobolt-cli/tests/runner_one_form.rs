// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T30: **the runner's form engine and the interpreter agree on one form**
//! (R9, R10, R28 runner half; AC14 part).
//!
//! One form with a Label, a TextBox, a CheckBox, a ListBox, a Timer and four Buttons. Its handlers
//! copy a typed value to a label, read the check box, add list items and count them, and start a
//! timer whose handler stops it. The same script runs headless twice, under the same pinned clock
//! and seed:
//!
//! 1. under `rcrun run-form`, the interpreter;
//! 2. in the program `rcrun build` makes on the WebAssembly path (`PRC_WASM_FORMS=1`): the runner,
//!    the module's programs driven by its ABI host.
//!
//! After every step that changes something, the script dumps every control's state and takes a
//! picture. The two runs must give equal dumps and pictures equal in every pixel.
//!
//! The task names a `cobolt-runner` test. It lives here instead, because it needs `rcrun` for both
//! the interpreter's run and the Build, and only this crate's tests have `rcrun` built for them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

const FORM: &str = "PARITY-ONE";
const PIN_CLOCK: &str = "2026-01-02T03:04:05-03:00";
const PIN_SEED: &str = "7";

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

fn handler(statements: &str) -> String {
    format!("       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n{statements}\n")
}

fn control(form: &mut Form, id: &str, kind: ControlType, rect: (i32, i32, i32, i32), props: &[(&str, PropValue)]) {
    let mut c = Control::new(id, kind, rect.0, rect.1);
    c.rect = Rect::new(rect.0, rect.1, rect.2, rect.3);
    for (k, v) in props {
        c.set_prop(*k, v.clone());
    }
    form.controls.push(c);
}

fn bind(form: &mut Form, id: &str, event: &str, statements: &str) {
    let c = form.controls.iter_mut().find(|c| c.id == id).expect("a control of the fixture");
    c.events.push(EventBinding { event: event.into(), paragraph: format!("{id}--{}", event.to_ascii_uppercase()), code: handler(statements) });
}

/// The fixture project in `root`; its manifest's path.
fn project(root: &Path) -> PathBuf {
    let _ = fs::remove_dir_all(root);
    for d in ["forms", "generated", "src", "data"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    let s = |v: &str| PropValue::String(v.into());
    let mut form = Form::new(FORM, "One form", 640, 420);
    form.main_form = true;
    control(&mut form, "LBL-OUT", ControlType::Label, (20, 20, 300, 24), &[("Caption", s("-"))]);
    control(&mut form, "TXT-NAME", ControlType::TextBox, (20, 56, 220, 28), &[("Text", s(""))]);
    control(&mut form, "BTN-GREET", ControlType::Button, (260, 56, 140, 28), &[("Caption", s("Greet"))]);
    control(&mut form, "CHK-BOLD", ControlType::CheckBox, (20, 100, 160, 24), &[("Caption", s("Bold"))]);
    control(&mut form, "BTN-CHECK", ControlType::Button, (260, 100, 140, 28), &[("Caption", s("Check"))]);
    control(&mut form, "LBL-STATE", ControlType::Label, (420, 100, 180, 24), &[("Caption", s("-"))]);
    control(&mut form, "LST-NAMES", ControlType::ListBox, (20, 140, 220, 120), &[]);
    control(&mut form, "BTN-ADD", ControlType::Button, (260, 140, 140, 28), &[("Caption", s("Add"))]);
    control(&mut form, "LBL-COUNT", ControlType::Label, (420, 140, 180, 24), &[("Caption", s("0"))]);
    control(&mut form, "BTN-START", ControlType::Button, (260, 280, 140, 28), &[("Caption", s("Start"))]);
    control(&mut form, "LBL-TICKS", ControlType::Label, (420, 280, 180, 24), &[("Caption", s("-"))]);
    control(&mut form, "TMR-1", ControlType::Timer, (600, 380, 24, 24), &[("Interval", PropValue::Int(100)), ("Enabled", PropValue::Bool(false))]);
    bind(&mut form, "BTN-GREET", "onClick", "           MOVE TXT-NAME::Text TO LBL-OUT::Caption");
    bind(
        &mut form,
        "BTN-CHECK",
        "onClick",
        "           IF CHK-BOLD::Checked = 1\n\
         \x20              MOVE \"checked\" TO LBL-STATE::Caption\n\
         \x20          ELSE\n\
         \x20              MOVE \"clear\" TO LBL-STATE::Caption\n\
         \x20          END-IF",
    );
    bind(
        &mut form,
        "BTN-ADD",
        "onClick",
        "           LST-NAMES::AddItem( TXT-NAME::Text )\n\
         \x20          MOVE LST-NAMES::GetCount() TO LBL-COUNT::Caption",
    );
    bind(&mut form, "BTN-START", "onClick", "           MOVE 1 TO TMR-1::Enabled");
    bind(
        &mut form,
        "TMR-1",
        "onTick",
        "           MOVE \"ticked\" TO LBL-TICKS::Caption\n\
         \x20          MOVE 0 TO TMR-1::Enabled",
    );
    let cfrm = root.join("forms/parity-one.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/parity-one.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    let manifest = root.join("parityone.project.toml");
    fs::write(
        &manifest,
        "[project]\nname = \"parityone\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/parity-one.cfrm\"]\ngenerated = [\"generated/parity-one.cbl\"]\n",
    )
    .unwrap();
    manifest
}

/// The script, and the names of its pictures in order.
fn script() -> (Value, Vec<&'static str>) {
    let click = |id: &str| json!({"event": {"control": id, "name": "onClick"}});
    let dump = || json!({"dump": true});
    let shot = |name: &str| json!({"shot": name});
    let shots = vec!["00-open", "01-greet", "02-check", "03-add", "04-timer"];
    let steps = vec![
        json!({"wait_ms": 200}),
        dump(),
        shot(shots[0]),
        json!({"set": {"control": "TXT-NAME", "property": "Text", "value": "Ann"}}),
        click("BTN-GREET"),
        dump(),
        shot(shots[1]),
        json!({"click": {"control": "CHK-BOLD"}}),
        click("BTN-CHECK"),
        dump(),
        shot(shots[2]),
        click("BTN-ADD"),
        json!({"set": {"control": "TXT-NAME", "property": "Text", "value": "Bob"}}),
        click("BTN-ADD"),
        dump(),
        shot(shots[3]),
        click("BTN-START"),
        json!({"wait_ms": 400}),
        dump(),
        shot(shots[4]),
    ];
    (Value::Array(steps), shots)
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
        let (x, y) = (a.get(id), b.get(id));
        if x == y {
            continue;
        }
        let (Some(x), Some(y)) = (x, y) else {
            out.push(format!("{id}: interpreter {} / runner {}", shown(x), shown(y)));
            continue;
        };
        for flag in ["visible", "enabled"] {
            if x.get(flag) != y.get(flag) {
                out.push(format!("{id}.{flag}: interpreter {} / runner {}", shown(x.get(flag)), shown(y.get(flag))));
            }
        }
        let (px, py) = (&x["props"], &y["props"]);
        let mut keys: Vec<&String> = px.as_object().into_iter().flat_map(|o| o.keys()).chain(py.as_object().into_iter().flat_map(|o| o.keys())).collect();
        keys.sort();
        keys.dedup();
        for k in keys {
            if px.get(k) != py.get(k) {
                out.push(format!("{id}.{k}: interpreter {} / runner {}", shown(px.get(k)), shown(py.get(k))));
            }
        }
    }
    out
}

#[test]
fn interpreter_and_wasm_engine_agree_on_one_form() {
    let root = std::env::temp_dir().join(format!("prc-t30-{}", std::process::id()));
    let manifest = project(&root);
    let (steps, shots) = script();
    let script_path = root.join("steps.json");
    fs::write(&script_path, steps.to_string()).unwrap();
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
    let interp_shots = root.join("shots-interpreter");
    fs::create_dir_all(&interp_shots).unwrap();
    let t = Instant::now();
    let out = Command::new(rcrun())
        .args(["run-form", "forms/parity-one.cfrm", "generated/parity-one.cbl", "--designer", "--headless"])
        .arg(&script_path)
        .args(["--headless-limit", "60"])
        .current_dir(&root)
        .env("HOME", &home)
        .env("PRC_TEST_CLOCK", PIN_CLOCK)
        .env("PRC_TEST_SEED", PIN_SEED)
        .env("PRC_HEADLESS_SHOTS", &interp_shots)
        .output()
        .expect("run rcrun run-form");
    let interp_ms = t.elapsed().as_secs_f64() * 1000.0;
    let interp_out = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form failed:\n{interp_out}\n{}", String::from_utf8_lossy(&out.stderr));
    let interp = report(&interp_out);

    // ── 2. The runner: rcrun build on the WebAssembly path, then the program ──
    let t = Instant::now();
    let out = Command::new(rcrun())
        .arg("build")
        .arg(&manifest)
        .arg("--quiet")
        .env(cobolt_compiler::BUILD_DIR_ENV, root.join("builds"))
        .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
        .env(cobolt_compiler::WASM_FORMS_ENV, "1")
        .env_remove(cobolt_compiler::RUNNER_ENV)
        .output()
        .expect("run rcrun build");
    let build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let build_out = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun build failed:\n{build_out}\n{}", String::from_utf8_lossy(&out.stderr));
    assert!(build_out.lines().any(|l| l.trim() == "Path   : WebAssembly"), "the Build took the WebAssembly path:\n{build_out}");
    let exe = root.join("bin").join(format!("parityone{}", std::env::consts::EXE_SUFFIX));
    let runner_shots = root.join("shots-runner");
    fs::create_dir_all(&runner_shots).unwrap();
    let t = Instant::now();
    let out = Command::new(&exe)
        .current_dir(&root)
        .env("HOME", &home)
        .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
        .env("PRC_TEST_CLOCK", PIN_CLOCK)
        .env("PRC_TEST_SEED", PIN_SEED)
        .env("PRC_HEADLESS_SCRIPT", &script_path)
        .env("PRC_HEADLESS_LIMIT", "60")
        .env("PRC_HEADLESS_SHOTS", &runner_shots)
        .output()
        .expect("run the built program");
    let wasm_ms = t.elapsed().as_secs_f64() * 1000.0;
    let wasm_out = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "the built program failed:\n{wasm_out}\n{}", String::from_utf8_lossy(&out.stderr));
    let wasm = report(&wasm_out);

    // ── The comparison ──
    let mut checks: Vec<(String, bool)> = Vec::new();
    let (a, b) = (dumps(&interp), dumps(&wasm));
    checks.push((format!("both runs dump {} times", shots.len()), a.len() == shots.len() && b.len() == shots.len()));
    let mut dump_lines = Vec::new();
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        let diff = dump_differences(x, y);
        checks.push((format!("dump {i} ({}) is equal", shots[i]), diff.is_empty()));
        dump_lines.extend(diff.into_iter().map(|d| format!("    dump {i}: {d}")));
    }
    let mut pixels_total = 0u64;
    let mut differing_total = 0u64;
    let mut shot_lines = Vec::new();
    for name in &shots {
        let file = format!("{name}.png");
        let (diff, (w, h)) = differing_pixels(&interp_shots.join(&file), &runner_shots.join(&file));
        pixels_total += u64::from(w) * u64::from(h);
        differing_total = differing_total.saturating_add(diff);
        checks.push((format!("picture {name}: 0 differing pixels"), diff == 0));
        shot_lines.push(format!("    {name}: {w}x{h}, {} differing pixels", if diff == u64::MAX { "missing,".to_string() } else { diff.to_string() }));
    }
    // What the program read and wrote is visible in the last dump.
    let last = b.last().cloned().unwrap_or(Value::Null);
    let cap = |id: &str| {
        let props = &last[id]["props"];
        props.get("CAPTION").or_else(|| props.get("Caption")).and_then(Value::as_str).unwrap_or("").to_string()
    };
    checks.push(("the runner's program copied the typed name (LBL-OUT = Ann)".into(), cap("LBL-OUT") == "Ann"));
    checks.push(("the runner's program read the clicked check box (LBL-STATE = checked)".into(), cap("LBL-STATE") == "checked"));
    checks.push(("the runner's program counted two list items (LBL-COUNT = 2)".into(), cap("LBL-COUNT") == "2"));
    checks.push(("the runner's timer ticked and stopped itself (LBL-TICKS = ticked)".into(), cap("LBL-TICKS") == "ticked"));

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== interpreter_and_wasm_engine_agree_on_one_form (DESKTOP-WASM-001 T30: R9, R10, R28; AC14 part) ==");
    println!("form {FORM}: Label x4, TextBox, CheckBox, ListBox, Timer, Button x4; events onClick x4 + onTick; methods AddItem, GetCount");
    println!("pins: PRC_TEST_CLOCK={PIN_CLOCK} PRC_TEST_SEED={PIN_SEED}; script {} steps, {} dumps, {} pictures", steps.as_array().map_or(0, Vec::len), shots.len(), shots.len());
    println!("runner built in {runner_ms:.0} ms; rcrun build (WebAssembly path) {build_ms:.0} ms");
    println!("interpreter run {interp_ms:.0} ms; runner run {wasm_ms:.0} ms");
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
