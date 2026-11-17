// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T31, T32, T49 and T50: **the runner's form engine and the interpreter agree on several
//! forms, the application shell and its SideMenu, the file dialogs, the sounds and the graphics** (R9, R10, R11, R27; AC14 part).
//!
//! The runner twins of four `rcrun` host tests, on their fixtures:
//!
//! 1. `side_menu_hosted_controls`: a shell whose rail holds a control in its header band and one a
//!    row hosts, pictured open and folded;
//! 2. `side_menu_rows_hosts`: a program that adds, changes and removes the SideMenu's rows at run
//!    time, read back through `RuntimeRows` and `GetCount()`;
//! 3. `layers_hosts_091`: a form with two layers, loaded into a shell's ContentPane, whose handlers
//!    show, hide and colour a layer and read it back;
//! 4. `perspective_hosts_096`: a chart a program tilts, loaded into the same shell, with values
//!    outside their ranges brought into them;
//! 5. and one of its own, `several_forms`: a program that opens a form beside it, one that waits
//!    for a modal form, and `super::` from the opened forms to their opener;
//! 6. T32's, `file_dialogs`: each kind of file dialog a program opens, answered by the script, and
//!    what each asked the form host for;
//! 7. T49's, `sounds`: each sound verb, with what `AUDIO-STATE` reads after it, on the null output.
//! 8. T50's, `graphics`: a canvas drawn and read back, an image drawn onto it, a physics world, a chart, refused
//!    calls, and `onFrame` with AutoPresent on and off.
//!
//! Each runs the same script headless twice, under the same pinned clock and seed: under
//! `rcrun run-form` (the interpreter), and in the program `rcrun build` makes on the WebAssembly
//! path (the runner). The readings, the dumps and the pictures must be equal; a picture in every
//! pixel.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;
use std::time::Instant;

use cobolt_forms::menu::{save_menu, MenuDefinition, MenuItem};
use cobolt_forms::model::{FormFormat, Layer};
use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

const PIN_CLOCK: &str = "2026-01-02T03:04:05-03:00";
const PIN_SEED: &str = "7";

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

/// The runner, built once beside `rcrun`, as a release has it.
fn build_runner() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let status = Command::new(cargo)
            .current_dir(workspace)
            .args(["build", "-q", "-p", "cobolt-runner", "--bin", "prc-runner"])
            .status()
            .expect("run cargo");
        assert!(status.success(), "cargo build -p cobolt-runner --bin prc-runner failed");
    });
}

// ── Fixtures ─────────────────────────────────────────────────────────────────────────────────────

fn handler(statements: &[&str]) -> String {
    let body = statements.iter().map(|s| format!("           {s}")).collect::<Vec<_>>().join("\n");
    format!("       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n{body}\n")
}

fn control(form: &mut Form, id: &str, kind: ControlType, rect: (i32, i32, i32, i32), caption: Option<&str>) {
    let mut c = Control::new(id, kind, rect.0, rect.1);
    c.rect = Rect::new(rect.0, rect.1, rect.2, rect.3);
    if let Some(text) = caption {
        c.set_prop("Caption", PropValue::String(text.into()));
    }
    form.controls.push(c);
}

fn bind(form: &mut Form, id: &str, event: &str, statements: &[&str]) {
    let paragraph = format!("{}--{}", id.to_ascii_uppercase(), event.to_ascii_uppercase());
    let code = handler(statements);
    if id.eq_ignore_ascii_case(&form.name) {
        // A new form already has its lifecycle events, empty (`Form::new`): fill the one there.
        match form.form_events.iter_mut().find(|e| e.event.eq_ignore_ascii_case(event)) {
            Some(e) => e.code = code,
            None => form.form_events.push(EventBinding { event: event.into(), paragraph, code }),
        }
        return;
    }
    let c = form.controls.iter_mut().find(|c| c.id == id).expect("a control of the fixture");
    c.events.push(EventBinding { event: event.into(), paragraph, code });
}

fn button(form: &mut Form, id: &str, at: (i32, i32), statements: &[&str]) {
    control(form, id, ControlType::Button, (at.0, at.1, 180, 28), Some(id));
    bind(form, id, "onClick", statements);
}

/// A project of `forms` (each saved, reloaded and generated) and an empty main program; its
/// manifest's path.
fn project(root: &Path, name: &str, forms: &[(&str, Form)], menus: &[(&str, MenuDefinition)]) -> PathBuf {
    let _ = fs::remove_dir_all(root);
    for d in ["forms", "generated", "src", "data", "home"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    let mut cfrms = Vec::new();
    let mut cbls = Vec::new();
    for (file, form) in forms {
        let cfrm = root.join(format!("forms/{file}.cfrm"));
        cobolt_forms::save_form(form, &cfrm).unwrap();
        let form = cobolt_forms::load_form(&cfrm).unwrap();
        fs::write(root.join(format!("generated/{file}.cbl")), cobolt_codegen::generate(&form)).unwrap();
        cfrms.push(format!("\"forms/{file}.cfrm\""));
        cbls.push(format!("\"generated/{file}.cbl\""));
    }
    for (file, def) in menus {
        save_menu(&root.join(format!("forms/{file}.menu.yaml")), def).unwrap();
    }
    fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    let manifest = root.join(format!("{}.project.toml", name.to_ascii_lowercase()));
    fs::write(
        &manifest,
        format!(
            "[project]\nname = \"{name}\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
             [files]\nsources = [\"src/main.cbl\"]\nforms = [{}]\ngenerated = [{}]\n",
            cfrms.join(", "),
            cbls.join(", ")
        ),
    )
    .unwrap();
    manifest
}

/// The AWS demos' shell (a SideMenu and its rows): any application shell would do, as the
/// `rcrun` tests say. Its menu is copied as it is (it is signed).
fn borrowed_shell() -> (Form, PathBuf) {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/aws/forms");
    (cobolt_forms::load_form(&from.join("aws-shell.cfrm")).unwrap(), from.join("SideMenu-1.menu.yaml"))
}

// ── The two runs ─────────────────────────────────────────────────────────────────────────────────

/// One headless run: its report, its pictures and how long it took.
struct Run {
    report: Value,
    stdout: String,
    shots: PathBuf,
    ms: f64,
}

/// The pins and the pictures folder every run is given.
fn pinned(cmd: &mut Command, home: &Path, shots: &Path) {
    cmd.env("HOME", home)
        // No test plays a sound on the machine's device (`cobolt_runtime::audio_runtime::OUTPUT_VAR`).
        .env("COBOLT_AUDIO", "null")
        .env("PRC_TEST_CLOCK", PIN_CLOCK)
        .env("PRC_TEST_SEED", PIN_SEED)
        .env("PRC_HEADLESS_SHOTS", shots);
}

fn report(stdout: &str) -> Value {
    stdout
        .lines()
        .find_map(|l| l.strip_prefix(cobolt_form_host::headless::RESULT_MARKER))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_else(|| panic!("no result line in:\n{stdout}"))
}

/// The interpreter: `rcrun run-form <start>` with the script.
fn interpreter(root: &Path, start: &str, script: &Path, home: &Path) -> Run {
    let shots = root.join(format!("shots-interpreter-{}", home.file_name().unwrap().to_string_lossy()));
    fs::create_dir_all(&shots).unwrap();
    let t = Instant::now();
    let mut cmd = Command::new(rcrun());
    cmd.args(["run-form", &format!("forms/{start}.cfrm"), &format!("generated/{start}.cbl"), "--designer", "--headless"])
        .arg(script)
        .args(["--headless-limit", "60"])
        .current_dir(root);
    pinned(&mut cmd, home, &shots);
    let out = cmd.output().expect("run rcrun run-form");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    Run { report: report(&stdout), stdout, shots, ms: t.elapsed().as_secs_f64() * 1000.0 }
}

/// `rcrun build` on the WebAssembly path; the built program's path and the Build's time.
fn build(root: &Path, manifest: &Path) -> (PathBuf, f64) {
    build_runner();
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
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun build failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    assert!(stdout.lines().any(|l| l.trim() == "Path   : WebAssembly"), "the Build took the WebAssembly path:\n{stdout}");
    let exe = stdout
        .lines()
        .find_map(|l| l.trim().strip_prefix("Binary :"))
        .map(|p| PathBuf::from(p.trim()))
        .unwrap_or_else(|| panic!("no Binary line in:\n{stdout}"));
    (exe, t.elapsed().as_secs_f64() * 1000.0)
}

/// The runner: the built program with the script.
fn runner(root: &Path, exe: &Path, script: &Path, home: &Path) -> Run {
    let shots = root.join(format!("shots-runner-{}", home.file_name().unwrap().to_string_lossy()));
    fs::create_dir_all(&shots).unwrap();
    let t = Instant::now();
    let mut cmd = Command::new(exe);
    cmd.current_dir(root)
        .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
        .env("PRC_HEADLESS_SCRIPT", script)
        .env("PRC_HEADLESS_LIMIT", "60");
    pinned(&mut cmd, home, &shots);
    let out = cmd.output().expect("run the built program");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "the built program failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    Run { report: report(&stdout), stdout, shots, ms: t.elapsed().as_secs_f64() * 1000.0 }
}

// ── Comparing ────────────────────────────────────────────────────────────────────────────────────

/// Every `read` of a report, in order.
fn readings(report: &Value) -> Vec<String> {
    report["steps"]
        .as_array()
        .map(|s| s.iter().filter_map(|st| st["value"].as_str().map(|v| v.trim().to_owned())).collect())
        .unwrap_or_default()
}

fn dumps(report: &Value) -> Vec<Value> {
    report["steps"].as_array().map(|s| s.iter().filter(|st| st.get("dump").is_some()).map(|st| st["state"].clone()).collect()).unwrap_or_default()
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

/// Pixels that differ between two pictures (all of them when the sizes differ or one is missing).
fn differing_pixels(a: &Path, b: &Path) -> (u64, (u32, u32)) {
    let (Ok(x), Ok(y)) = (image::open(a), image::open(b)) else { return (u64::MAX, (0, 0)) };
    let (x, y) = (x.to_rgba8(), y.to_rgba8());
    if x.dimensions() != y.dimensions() {
        return (u64::from(x.width()) * u64::from(x.height()), x.dimensions());
    }
    (x.pixels().zip(y.pixels()).filter(|(p, q)| p != q).count() as u64, x.dimensions())
}

/// One twin's tally: checks, and the lines that explain them.
#[derive(Default)]
struct Tally {
    checks: Vec<(String, bool)>,
    lines: Vec<String>,
    pixels: u64,
    differing: u64,
    pictures: usize,
}

impl Tally {
    fn check(&mut self, what: impl Into<String>, ok: bool) {
        self.checks.push((what.into(), ok));
    }

    /// The two runs' readings, dumps and pictures, compared.
    fn compare(&mut self, label: &str, a: &Run, b: &Run, shots: &[&str]) {
        let (ra, rb) = (readings(&a.report), readings(&b.report));
        self.check(format!("{label}: the {} readings are equal", ra.len()), ra == rb);
        for (i, (x, y)) in ra.iter().zip(rb.iter()).enumerate() {
            let mark = if x == y { "  " } else { "!=" };
            self.lines.push(format!("    {label} read {i:<2} {mark} interpreter {} / runner {}", x.replace('\n', " | "), y.replace('\n', " | ")));
        }
        if ra.len() != rb.len() {
            self.lines.push(format!("    {label}: interpreter {} readings, runner {}", ra.len(), rb.len()));
        }
        let (da, db) = (dumps(&a.report), dumps(&b.report));
        for (i, (x, y)) in da.iter().zip(db.iter()).enumerate() {
            let diff = dump_differences(x, y);
            self.check(format!("{label}: dump {i} is equal"), diff.is_empty());
            self.lines.extend(diff.into_iter().map(|d| format!("    {label} dump {i}: {d}")));
        }
        self.check(format!("{label}: both runs dump {} times", da.len()), da.len() == db.len());
        for name in shots {
            let file = format!("{name}.png");
            let (diff, (w, h)) = differing_pixels(&a.shots.join(&file), &b.shots.join(&file));
            self.pictures += 1;
            self.pixels += u64::from(w) * u64::from(h);
            self.differing = self.differing.saturating_add(diff);
            self.check(format!("{label}: picture {name}: 0 differing pixels"), diff == 0);
            let shown = if diff == u64::MAX { "missing".to_string() } else { diff.to_string() };
            self.lines.push(format!("    {label} picture {name}: {w}x{h}, {shown} differing"));
        }
        self.lines.push(format!("    {label}: interpreter {:.0} ms, runner {:.0} ms", a.ms, b.ms));
    }

    /// The result block (GOLDEN RULE #7), then the verdict.
    fn finish(self, title: &str, what: &str, root: &Path, build_ms: f64) {
        let passed = self.checks.iter().filter(|(_, ok)| *ok).count();
        println!("== {title} (DESKTOP-WASM-001 T31, T32, T49, T50: R9, R10, R11, R27; AC14 part) ==");
        println!("{what}");
        println!("pins: PRC_TEST_CLOCK={PIN_CLOCK} PRC_TEST_SEED={PIN_SEED}; rcrun build (WebAssembly path) {build_ms:.0} ms");
        for l in &self.lines {
            println!("{l}");
        }
        println!("pictures {}: pixels compared {}, differing {}", self.pictures, self.pixels, self.differing);
        for (w, ok) in &self.checks {
            println!("  {} {w}", if *ok { "pass" } else { "FAIL" });
        }
        println!("result: {passed} of {} checks passed", self.checks.len());
        let failed: Vec<&String> = self.checks.iter().filter(|(_, ok)| !*ok).map(|(w, _)| w).collect();
        if failed.is_empty() {
            let _ = fs::remove_dir_all(root);
        } else {
            println!("kept for inspection: {}", root.display());
        }
        assert!(failed.is_empty(), "failed: {failed:?}");
    }
}

fn root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("prc-t31-{tag}-{}", std::process::id()))
}

fn script(root: &Path, name: &str, steps: Vec<Value>) -> PathBuf {
    let path = root.join(format!("{name}.json"));
    fs::write(&path, Value::Array(steps).to_string()).unwrap();
    path
}

fn click(id: &str) -> Value {
    json!({"event": {"control": id, "name": "onClick"}})
}

fn read(ctrl: &str, prop: &str) -> Value {
    json!({"read": {"control": ctrl, "property": prop}})
}

fn wait(ms: u64) -> Value {
    json!({"wait_ms": ms})
}

fn shot(name: &str) -> Value {
    json!({"shot": name})
}

fn dump() -> Value {
    json!({"dump": true})
}

// ── 1. side_menu_hosted_controls ─────────────────────────────────────────────────────────────────

#[test]
fn the_rail_and_its_controls_look_the_same_open_and_folded() {
    let root = root("rail");
    let mut form = Form::new("SHELL", "Rail controls", 960, 600);
    form.main_form = true;
    let mut side = Control::new("SideMenu-1", ControlType::SideMenu, 0, 0);
    side.rect = Rect::new(0, 0, 240, 600);
    side.set_prop("HeaderHeight", 100i64);
    side.set_prop("HeaderBandHeight", 60i64);
    side.set_prop("FooterHeight", 60i64);
    form.controls.push(side);
    form.sync_side_menu_footer_panels();
    form.sync_side_menu_header_panels();
    let band = cobolt_forms::model::side_menu_header_id("SideMenu-1");
    let marker = |id: &str, parent: &str, x: i32, y: i32, ink: &str| {
        let mut c = Control::new(id, ControlType::TextBox, x, y);
        c.rect = Rect::new(x, y, 150, 28);
        c.parent = Some(parent.to_owned());
        c.set_prop("BackgroundColor", ink);
        c
    };
    form.controls.push(marker("TXT-BAND", &band, 20, 116, "#FF00FF"));
    form.controls.push(marker("TXT-ROW", "SideMenu-1", 600, 300, "#00FF00"));
    let mut picker = MenuItem::new_action("picker", "Picker");
    picker.control = Some("TXT-ROW".into());
    let def = MenuDefinition {
        menu: vec![MenuItem::new_action("home", "Home"), MenuItem::new_action("reports", "Reports"), picker],
        hash: String::new(),
    };
    let manifest = project(&root, "RailControls", &[("shell", form)], &[("SideMenu-1", def)]);
    let steps = script(&root, "steps", vec![wait(1200), dump(), shot("rail")]);
    let (exe, build_ms) = build(&root, &manifest);

    let mut tally = Tally::default();
    for (label, collapsed) in [("open", false), ("folded", true)] {
        // The rail's remembered state, as the operator left it in an earlier session.
        let home = root.join(format!("home-{label}"));
        #[cfg(target_os = "macos")]
        let data = home.join("Library/Application Support");
        #[cfg(not(target_os = "macos"))]
        let data = home.join(".local/share");
        let state = data.join("cobolt/apps/SHELL/shell.toml");
        fs::create_dir_all(state.parent().unwrap()).unwrap();
        fs::write(&state, format!("collapsed = {collapsed}\n")).unwrap();
        let a = interpreter(&root, "shell", &steps, &home);
        let b = runner(&root, &exe, &steps, &home);
        tally.compare(label, &a, &b, &["rail"]);
    }
    tally.finish(
        "the_rail_and_its_controls_look_the_same_open_and_folded",
        "twin of side_menu_hosted_controls: a shell, a SideMenu rail with a header band holding a TextBox and a row hosting one, open and folded",
        &root,
        build_ms,
    );
}

// ── 2. side_menu_rows_hosts ──────────────────────────────────────────────────────────────────────

#[test]
fn a_program_makes_the_same_side_menu_rows() {
    let root = root("rows");
    let mut form = Form::new("ROWS-DEMO", "Run-time rows", 960, 540);
    form.main_form = true;
    let mut side = Control::new("SideMenu-1", ControlType::SideMenu, 0, 0);
    side.rect = Rect::new(0, 0, 240, 540);
    form.controls.push(side);
    form.sync_side_menu_footer_panels();
    form.sync_side_menu_header_panels();
    button(
        &mut form,
        "BTN-ADD",
        (320, 20),
        &[
            "SideMenu-1::AddSection( \"Chats\" )",
            "SideMenu-1::AddItem( \"chat-1\", \"First chat\", \"chat\" )",
            "SideMenu-1::AddItem( \"chat-2\", \"Second chat\", \"chat\" )",
            "SideMenu-1::AddItem( \"part-1\", \"A part\", \"\", \"docs\" )",
            "MOVE SideMenu-1::GetCount() TO LBL-RESULT::Caption",
        ],
    );
    button(
        &mut form,
        "BTN-CHANGE",
        (320, 56),
        &[
            "SideMenu-1::SetItemLabel( \"chat-1\", \"Renamed chat\" )",
            "SideMenu-1::SetItemBadge( \"chat-1\", \"3\" )",
            "SideMenu-1::SetItemEnabled( \"chat-2\", \"0\" )",
            "SideMenu-1::SetItemLabel( \"home\", \"Nope\" )",
            "MOVE SideMenu-1::GetCount() TO LBL-RESULT::Caption",
        ],
    );
    button(
        &mut form,
        "BTN-CLEAR",
        (320, 92),
        &[
            "SideMenu-1::RemoveItem( \"chat-2\" )",
            "SideMenu-1::RemoveItem( \"docs\" )",
            "MOVE SideMenu-1::GetCount() TO LBL-RESULT::Caption",
            "SideMenu-1::Clear()",
        ],
    );
    control(&mut form, "LBL-RESULT", ControlType::Label, (320, 140, 300, 24), Some("-"));
    let mut docs = MenuItem::new_action("docs", "Documents");
    docs.items.push(MenuItem::new_action("docs-new", "New"));
    let def = MenuDefinition { menu: vec![MenuItem::new_action("home", "Home"), docs], hash: String::new() };
    let manifest = project(&root, "RowsHosts", &[("rows-demo", form)], &[("SideMenu-1", def)]);
    let shots = ["add", "change", "clear"];
    let mut steps = Vec::new();
    for (button, name) in ["BTN-ADD", "BTN-CHANGE", "BTN-CLEAR"].into_iter().zip(shots) {
        steps.extend([click(button), wait(500), read("SideMenu-1", "RuntimeRows"), read("LBL-RESULT", "Caption"), dump(), shot(name)]);
    }
    let steps = script(&root, "steps", steps);
    let (exe, build_ms) = build(&root, &manifest);
    let home = root.join("home");
    let a = interpreter(&root, "rows-demo", &steps, &home);
    let b = runner(&root, &exe, &steps, &home);
    let mut tally = Tally::default();
    tally.compare("rows", &a, &b, &shots);
    let got = readings(&b.report);
    let counts: Vec<&str> = got.iter().skip(1).step_by(2).map(String::as_str).collect();
    tally.check("the runner's program counts its rows 4, 4, 3 (GetCount())", counts == ["4", "4", "3"]);
    tally.finish(
        "a_program_makes_the_same_side_menu_rows",
        "twin of side_menu_rows_hosts: AddSection, AddItem (nested), SetItemLabel, SetItemBadge, SetItemEnabled, RemoveItem, Clear, GetCount; RuntimeRows read after each click",
        &root,
        build_ms,
    );
}

// ── 3. layers_hosts_091 and 4. perspective_hosts_096 ─────────────────────────────────────────────

/// A form loaded into the borrowed shell's ContentPane by the script's `open_form`, with its
/// readings checked against `want` on both engines.
fn in_the_shell(tag: &str, title: &str, what: &str, form: Form, steps: Vec<Value>, want: &[&str], shots: &[&str]) {
    let root = root(tag);
    let file = form.name.to_ascii_lowercase();
    let (shell, menu) = borrowed_shell();
    let manifest = project(&root, &format!("{}Hosts", tag), &[(file.as_str(), form), ("aws-shell", shell)], &[]);
    fs::copy(menu, root.join("forms/SideMenu-1.menu.yaml")).unwrap();
    let mut all = vec![json!({"open_form": file}), wait(500)];
    all.extend(steps);
    let steps = script(&root, "steps", all);
    let (exe, build_ms) = build(&root, &manifest);
    let home = root.join("home");
    let a = interpreter(&root, "aws-shell", &steps, &home);
    let b = runner(&root, &exe, &steps, &home);
    let mut tally = Tally::default();
    tally.compare(tag, &a, &b, shots);
    let got = readings(&b.report);
    tally.check(format!("the runner reads what the rcrun test wants: {want:?}"), got == want);
    tally.finish(title, what, &root, build_ms);
}

#[test]
fn a_layer_answers_the_same_in_the_shell() {
    let mut f = Form::new("LAYERS-DEMO", "Layers", 640, 480);
    f.form_format = FormFormat::Both;
    f.layers = vec![Layer::new("LAYER-1"), Layer::new("LAYER-2")];
    button(&mut f, "BTN-SHOW", (20, 20), &["SET LAYER-1::Visible TO TRUE"]);
    button(&mut f, "BTN-HIDE", (20, 56), &["SET LAYER-1::Visible TO FALSE"]);
    button(&mut f, "BTN-READ", (20, 92), &["MOVE LAYER-1::Visible TO LBL-RESULT::Caption"]);
    button(&mut f, "BTN-COLOUR", (20, 128), &["MOVE \"#FF0000FF\" TO LAYER-1::BackgroundColor"]);
    button(&mut f, "BTN-TRANSP", (20, 164), &["MOVE 40 TO LAYER-2::Transparency"]);
    control(&mut f, "LBL-RESULT", ControlType::Label, (20, 200, 300, 24), Some("-"));
    control(&mut f, "LBL-IN-LAYER", ControlType::Label, (300, 20, 200, 24), Some("inside Layer-1"));
    f.controls.last_mut().unwrap().layer = Some("LAYER-1".into());
    let steps = vec![
        read("LAYER-1", "Visible"),
        read("LAYER-2", "Visible"),
        dump(),
        shot("start"),
        click("BTN-SHOW"),
        wait(500),
        read("LAYER-1", "Visible"),
        click("BTN-READ"),
        wait(500),
        read("LBL-RESULT", "Caption"),
        dump(),
        shot("shown"),
        click("BTN-COLOUR"),
        wait(500),
        read("LAYER-1", "BackgroundColor"),
        click("BTN-TRANSP"),
        wait(500),
        read("LAYER-2", "Transparency"),
        dump(),
        shot("coloured"),
        click("BTN-HIDE"),
        wait(500),
        read("LAYER-1", "Visible"),
        click("BTN-SHOW"),
        wait(500),
        click("BTN-COLOUR"),
        wait(500),
        read("LAYER-1", "Visible"),
        dump(),
        shot("again"),
    ];
    in_the_shell(
        "layers",
        "a_layer_answers_the_same_in_the_shell",
        "twin of layers_hosts_091: a form with two layers in the shell's ContentPane; SET Visible, MOVE BackgroundColor, MOVE Transparency, a layer read by COBOL",
        f,
        steps,
        &["false", "false", "true", "true", "#FF0000FF", "40", "false", "true"],
        &["start", "shown", "coloured", "again"],
    );
}

#[test]
fn a_tilted_chart_answers_the_same_in_the_shell() {
    let mut f = Form::new("PERSPECTIVE-DEMO", "Perspective", 640, 380);
    f.form_format = FormFormat::Both;
    control(&mut f, "CHART-1", ControlType::BarChart, (20, 20, 320, 220), None);
    button(
        &mut f,
        "BTN-TILT",
        (360, 20),
        &["MOVE 30 TO CHART-1::RotateY", "MOVE 200 TO CHART-1::RotateX", "MOVE 40 TO CHART-1::PerspectiveStrength", "MOVE 24 TO CHART-1::PinTopLeftY"],
    );
    button(&mut f, "BTN-CLAMP", (360, 92), &["MOVE 150 TO CHART-1::PerspectiveStrength", "MOVE 20000 TO CHART-1::PinBottomRightX"]);
    button(&mut f, "BTN-READ", (360, 56), &["MOVE CHART-1::RotateX TO LBL-RESULT::Caption"]);
    control(&mut f, "LBL-RESULT", ControlType::Label, (360, 130, 200, 24), Some("-"));
    let steps = vec![
        read("CHART-1", "RotateX"),
        read("CHART-1", "RotateY"),
        read("CHART-1", "PerspectiveDistance"),
        read("CHART-1", "PerspectiveStrength"),
        read("CHART-1", "PinTopLeftY"),
        dump(),
        shot("flat"),
        click("BTN-TILT"),
        wait(500),
        read("CHART-1", "RotateY"),
        read("CHART-1", "RotateX"),
        read("CHART-1", "PerspectiveStrength"),
        read("CHART-1", "PinTopLeftY"),
        dump(),
        shot("tilted"),
        click("BTN-READ"),
        wait(500),
        read("LBL-RESULT", "Caption"),
        click("BTN-CLAMP"),
        wait(500),
        read("CHART-1", "PerspectiveStrength"),
        read("CHART-1", "PinBottomRightX"),
        dump(),
        shot("clamped"),
    ];
    in_the_shell(
        "perspective",
        "a_tilted_chart_answers_the_same_in_the_shell",
        "twin of perspective_hosts_096: a BarChart in the shell's ContentPane; RotateX/RotateY, PerspectiveStrength and corner pins written, read back and clamped",
        f,
        steps,
        &["0", "0", "50", "100", "0", "30", "60", "40", "24", "60", "100", "9999"],
        &["flat", "tilted", "clamped"],
    );
}

/// The charts' built-ins, `COBOL::"CHART-SET-TABLE"`, `"CHART-ADD-POINT"`, `"CHART-CLEAR"` and
/// `"CHART-REFRESH"`, give each chart the same points under both engines: a table of the fixed rows
/// (`PIC X(64)` label, `PIC 9(18)V9(6)` value) whose first label starts with spaces, a table of other
/// rows read as the fixed ones (PowerSpatial's wave: 48 rows of `X(4)` + `9(3)`, labels edited
/// `ZZ9`), points with further series, a ScatterChart's bubble sizes, and a chart cleared, refreshed
/// and filled again. (A name that is no control of the form draws nothing under either engine, but
/// the interpreter's form host keeps an entry for it that a `dump` lists: WASM-PARITY #60.)
#[test]
fn chart_built_ins_give_the_same_points() {
    let root = root("charts");
    let mut main = Form::new("MAIN-FORM", "Charts", 760, 520);
    main.main_form = true;
    for (id, kind, rect) in [
        ("CHT-STD", ControlType::BarChart, (20, 20, 340, 150)),
        ("CHT-WAVE", ControlType::BarChart, (380, 20, 340, 150)),
        ("CHT-SERIES", ControlType::LineChart, (20, 190, 340, 150)),
        ("CHT-BUBBLE", ControlType::ScatterChart, (380, 190, 340, 150)),
        ("CHT-AGAIN", ControlType::BarChart, (20, 360, 340, 140)),
        ("CHT-M-LINE", ControlType::LineChart, (380, 360, 200, 70)),
        ("CHT-M-BUBBLE", ControlType::ScatterChart, (380, 440, 200, 70)),
    ] {
        control(&mut main, id, kind, rect, None);
    }
    let storage = [
        "01 WS-STD.",
        "   05 WS-STD-ROW OCCURS 2.",
        "      10 WS-STD-LABEL PIC X(64).",
        "      10 WS-STD-VALUE PIC 9(18)V9(6).",
        "01 WS-STD-N PIC 9(3) VALUE 2.",
        "01 WS-WAVE.",
        "   05 WS-WAVE-ROW OCCURS 48.",
        "      10 WS-WAVE-LABEL PIC X(4).",
        "      10 WS-WAVE-VALUE PIC 9(3).",
        "01 WS-WAVE-N PIC 9(3) VALUE 48.",
        "01 WS-I PIC 9(3).",
        "01 WS-N-TXT PIC ZZ9.",
    ];
    let cases: [(&str, Vec<&str>); 6] = [
        (
            "STD",
            vec![
                "MOVE \"  North\" TO WS-STD-LABEL(1)",
                "MOVE 12.5 TO WS-STD-VALUE(1)",
                "MOVE \"South\" TO WS-STD-LABEL(2)",
                "MOVE 7 TO WS-STD-VALUE(2)",
                "COBOL::\"CHART-SET-TABLE\" ( \"CHT-STD\" WS-STD WS-STD-N )",
            ],
        ),
        (
            "WAVE",
            vec![
                "PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-WAVE-N",
                "    MOVE WS-I TO WS-N-TXT",
                "    MOVE WS-N-TXT TO WS-WAVE-LABEL(WS-I)",
                "    COMPUTE WS-WAVE-VALUE(WS-I) = WS-I * 2",
                "END-PERFORM",
                "COBOL::\"CHART-SET-TABLE\" ( \"CHT-WAVE\" WS-WAVE WS-WAVE-N )",
            ],
        ),
        (
            "SERIES",
            vec![
                "COBOL::\"CHART-ADD-POINT\" ( \"CHT-SERIES\" \"Jan\" 10 )",
                "COBOL::\"CHART-ADD-POINT\" ( \"CHT-SERIES\" \"Feb\" 20 25 30 )",
                "COBOL::\"CHART-ADD-POINT\" ( \"CHT-SERIES\" \"Mar\" 15 )",
            ],
        ),
        (
            "BUBBLE",
            vec!["COBOL::\"CHART-ADD-POINT\" ( \"CHT-BUBBLE\" \"A\" 3 40 )", "COBOL::\"CHART-ADD-POINT\" ( \"CHT-BUBBLE\" \"B\" 5 )"],
        ),
        (
            "AGAIN",
            vec![
                "COBOL::\"CHART-ADD-POINT\" ( \"CHT-AGAIN\" \"X\" 9 )",
                "COBOL::\"CHART-CLEAR\" ( \"CHT-AGAIN\" )",
                "COBOL::\"CHART-REFRESH\" ( \"CHT-AGAIN\" )",
                "COBOL::\"CHART-ADD-POINT\" ( \"CHT-AGAIN\" \"Y\" 4 )",
            ],
        ),
        // The chart methods keep the points as the built-ins do.
        (
            "METHODS",
            vec![
                "CHT-M-LINE::AddPoint(\"Q1\", 5)",
                "CHT-M-LINE::AddPoint(\"Q2\", 6, 7)",
                "CHT-M-LINE::AddPoint(\"Q3\", 2.5)",
                "CHT-M-LINE::Refresh()",
                "CHT-M-BUBBLE::AddPoint(\"P\", 2, 30)",
                "CHT-M-BUBBLE::AddPoint(\"R\", 4)",
                "CHT-M-BUBBLE::Clear()",
                "CHT-M-BUBBLE::AddPoint(\"S\", 1, 12)",
            ],
        ),
    ];
    for (i, (name, statements)) in cases.iter().enumerate() {
        let id = format!("BTN-{name}");
        control(&mut main, &id, ControlType::Button, (600, 360 + 30 * i as i32, 140, 26), Some(&id));
        bind_ws(&mut main, &id, "onClick", &storage, statements);
    }
    let manifest = project(&root, "Charts", &[("main-form", main)], &[]);
    let mut steps = vec![wait(600)];
    for (name, chart) in [("STD", "CHT-STD"), ("WAVE", "CHT-WAVE"), ("SERIES", "CHT-SERIES"), ("BUBBLE", "CHT-BUBBLE"), ("AGAIN", "CHT-AGAIN"), ("METHODS", "CHT-M-LINE")] {
        steps.extend([click(&format!("BTN-{name}")), wait(200), read(chart, "__ChartData"), read(chart, "__ChartSizes")]);
    }
    steps.extend([read("CHT-M-BUBBLE", "__ChartData"), read("CHT-M-BUBBLE", "__ChartSizes")]);
    steps.extend([dump(), shot("charts")]);
    let steps = script(&root, "steps", steps);
    let (exe, build_ms) = build(&root, &manifest);
    let home = root.join("home");
    let a = interpreter(&root, "main-form", &steps, &home);
    let b = runner(&root, &exe, &steps, &home);
    let mut tally = Tally::default();
    tally.compare("charts", &a, &b, &["charts"]);
    let got = readings(&b.report);
    let want_lines = |i: usize, n: usize| got.get(i).is_some_and(|r| r.lines().count() == n);
    tally.check("the fixed rows: North 12.5, South 7 (the first label's leading spaces kept)", got.first().map(String::as_str) == Some("North\t12.5\nSouth\t7"));
    tally.check("the wave's 336 characters make 3 rows of 88, as the interpreter reads them", want_lines(2, 3));
    tally.check("further series padded with 0: Jan 10 0 0, Feb 20 25 30, Mar 15 0 0", got.get(4).map(String::as_str) == Some("Jan\t10\t0\t0\nFeb\t20\t25\t30\nMar\t15\t0\t0"));
    tally.check("a ScatterChart's sizes: 40, then 0 for a point given none", got.get(6).map(String::as_str) == Some("A\t3\nB\t5") && got.get(7).map(String::as_str) == Some("40\n0"));
    tally.check("cleared, refreshed and filled again: Y 4 only", got.get(8).map(String::as_str) == Some("Y\t4"));
    tally.check(
        "the methods: Q1 5 0, Q2 6 7, Q3 2.5 0; a ScatterChart cleared, then S 1 with size 12",
        got.get(10).map(String::as_str) == Some("Q1\t5\t0\nQ2\t6\t7\nQ3\t2.5\t0") && got.get(12).map(String::as_str) == Some("S\t1") && got.get(13).map(String::as_str) == Some("12"),
    );
    tally.finish(
        "chart_built_ins_give_the_same_points",
        "COBOL::\"CHART-SET-TABLE\" (fixed rows with a label that starts with spaces; 48 rows of X(4) + 9(3) read as the fixed rows), \"CHART-ADD-POINT\" (1 and 3 series; a ScatterChart with and without a size), \"CHART-CLEAR\", \"CHART-REFRESH\"; the methods AddPoint (2 series, a decimal), Refresh, and on a ScatterChart AddPoint with a size, Clear",
        &root,
        build_ms,
    );
}

// ── 5. several forms ─────────────────────────────────────────────────────────────────────────────

/// A headless script reaches the main window's controls only, so what the opened forms do shows
/// on the main form: each tells its opener through `super::"SetProperty"`, and the main form reads
/// it back with `ME::"GetProperty"`. The modal form tells and closes itself in its `onLoad`; the
/// main form's handler that opened it goes on only then. A click the script makes while the main
/// form waits for the modal is taken when the wait is over, by both engines.
#[test]
fn several_forms_open_wait_and_reach_their_opener() {
    let root = root("forms");
    let mut main = Form::new("MAIN-FORM", "Main", 640, 400);
    main.main_form = true;
    control(&mut main, "LBL-OUT", ControlType::Label, (20, 20, 400, 24), Some("-"));
    control(&mut main, "LBL-AFTER", ControlType::Label, (20, 56, 400, 24), Some("-"));
    button(&mut main, "BTN-BESIDE", (20, 100), &["INVOKE ME::\"OpenFormAsync\"(\"SIDE-FORM\")"]);
    button(&mut main, "BTN-READ", (20, 140), &["INVOKE ME::\"GetProperty\"(\"Told\") RETURNING LBL-OUT::Caption"]);
    button(
        &mut main,
        "BTN-MODAL",
        (20, 180),
        &["INVOKE ME::\"OpenFormSync\"(\"ASK-FORM\")", "INVOKE ME::\"GetProperty\"(\"Answer\") RETURNING LBL-AFTER::Caption"],
    );
    // The main form opened again while it runs: its one window is focused, no second program
    // starts, and a Sync open does not wait (spec 037 R10).
    control(&mut main, "LBL-AGAIN", ControlType::Label, (20, 220, 400, 24), Some("-"));
    button(&mut main, "BTN-AGAIN", (20, 260), &["INVOKE ME::\"OpenFormSync\"(\"MAIN-FORM\")", "MOVE \"went on\" TO LBL-AGAIN::Caption"]);
    bind(&mut main, "MAIN-FORM", "onLoad", &["DISPLAY \"MAIN-FORM loaded\""]);
    let mut side = Form::new("SIDE-FORM", "Beside", 360, 220);
    control(&mut side, "LBL-SIDE", ControlType::Label, (20, 20, 300, 24), Some("beside"));
    bind(&mut side, "SIDE-FORM", "onLoad", &["DISPLAY \"SIDE-FORM loaded\"", "INVOKE super::\"SetProperty\"(\"Told\", \"told by the side form\")"]);
    let mut ask = Form::new("ASK-FORM", "Modal", 360, 220);
    control(&mut ask, "LBL-ASK", ControlType::Label, (20, 20, 300, 24), Some("asking"));
    bind(&mut ask, "ASK-FORM", "onLoad", &["DISPLAY \"ASK-FORM loaded\"", "INVOKE super::\"SetProperty\"(\"Answer\", \"answered\")", "INVOKE ME::\"Close\"()"]);
    let manifest = project(&root, "SeveralForms", &[("main-form", main), ("side-form", side), ("ask-form", ask)], &[]);
    let steps = script(
        &root,
        "steps",
        vec![
            wait(600),
            click("BTN-BESIDE"),
            wait(600),
            click("BTN-READ"),
            wait(300),
            read("LBL-OUT", "Caption"),
            dump(),
            shot("beside"),
            click("BTN-MODAL"),
            wait(900),
            read("LBL-AFTER", "Caption"),
            dump(),
            shot("after"),
            click("BTN-AGAIN"),
            wait(600),
            read("LBL-AGAIN", "Caption"),
            dump(),
            shot("again"),
        ],
    );
    let (exe, build_ms) = build(&root, &manifest);
    let home = root.join("home");
    let a = interpreter(&root, "main-form", &steps, &home);
    let b = runner(&root, &exe, &steps, &home);
    let mut tally = Tally::default();
    tally.compare("forms", &a, &b, &["beside", "after", "again"]);
    for (who, run) in [("interpreter", &a), ("runner", &b)] {
        for form in ["MAIN-FORM", "SIDE-FORM", "ASK-FORM"] {
            let loads = run.stdout.matches(&format!("{form} loaded")).count();
            tally.lines.push(format!("    {who}: {form} onLoad ran {loads} time(s)"));
            tally.check(format!("{who}: {form}'s onLoad ran once"), loads == 1);
        }
    }
    tally.check(
        "the runner's programs: the form opened beside told its opener; the modal answered and its caller went on; the main form opened again did not wait",
        readings(&b.report) == ["told by the side form", "answered", "went on"],
    );
    tally.finish(
        "several_forms_open_wait_and_reach_their_opener",
        "three forms: OpenFormAsync, OpenFormSync (the caller waits for the modal), onLoad in the opened forms, super::\"SetProperty\" to the opener, ME::\"GetProperty\", ME::\"Close\", the main form opened again (one window, no wait)",
        &root,
        build_ms,
    );
}

/// A windowHandler (spec 037 R20, R23, R24): `OpenFormAsync … RETURNING WS-WORK` keeps the opened
/// window's handle in a `USAGE OBJECT` item, and the program reaches that window through it —
/// `SetProperty`, `GetProperty`, `Close`. Once the window is closed the handle is NULL, and a
/// call through it is the form's unhandled exception: the handler stops there and the window shows
/// the critical notice in the interpreter's words. PowerCloud's working window is closed this way
/// (ABI A40).
#[test]
fn a_window_handler_reaches_its_window_and_is_null_once_it_closes() {
    let root = root("handles");
    let mut main = Form::new("MAIN-FORM", "Main", 640, 400);
    main.main_form = true;
    main.user_ws_source = "       01 WS-WORK         USAGE OBJECT GLOBAL.\n".to_owned();
    control(&mut main, "LBL-OUT", ControlType::Label, (20, 20, 400, 24), Some("-"));
    control(&mut main, "LBL-STATE", ControlType::Label, (20, 56, 400, 24), Some("-"));
    control(&mut main, "LBL-NULL", ControlType::Label, (20, 92, 600, 24), Some("-"));
    button(&mut main, "BTN-OPEN", (20, 140), &["INVOKE ME::OpenFormAsync( \"WORK-FORM\" ) RETURNING WS-WORK"]);
    button(
        &mut main,
        "BTN-TELL",
        (20, 180),
        &[
            "INVOKE WS-WORK::SetProperty( \"Told\", \"told through the handle\" )",
            "INVOKE WS-WORK::GetProperty( \"Told\" ) RETURNING LBL-OUT::Caption",
        ],
    );
    button(&mut main, "BTN-CLOSE", (20, 220), &["INVOKE WS-WORK::Close()", "MOVE \"closed\" TO LBL-STATE::Caption"]);
    button(&mut main, "BTN-AGAIN", (20, 260), &["INVOKE WS-WORK::Close()", "MOVE \"went on\" TO LBL-NULL::Caption"]);
    let mut work = Form::new("WORK-FORM", "Working", 360, 220);
    control(&mut work, "LBL-WORK", ControlType::Label, (20, 20, 300, 24), Some("working"));
    bind(&mut work, "WORK-FORM", "onLoad", &["DISPLAY \"WORK-FORM loaded\""]);
    let manifest = project(&root, "WindowHandler", &[("main-form", main), ("work-form", work)], &[]);
    let steps = script(
        &root,
        "steps",
        vec![
            wait(600),
            click("BTN-OPEN"),
            wait(600),
            click("BTN-TELL"),
            wait(300),
            read("LBL-OUT", "Caption"),
            dump(),
            shot("told"),
            click("BTN-CLOSE"),
            wait(600),
            read("LBL-STATE", "Caption"),
            dump(),
            shot("closed"),
            click("BTN-AGAIN"),
            wait(600),
            read("LBL-NULL", "Caption"),
            dump(),
            shot("null"),
        ],
    );
    let (exe, build_ms) = build(&root, &manifest);
    let home = root.join("home");
    let a = interpreter(&root, "main-form", &steps, &home);
    let b = runner(&root, &exe, &steps, &home);
    let mut tally = Tally::default();
    tally.compare("handles", &a, &b, &["told", "closed", "null"]);
    for (who, run) in [("interpreter", &a), ("runner", &b)] {
        let loads = run.stdout.matches("WORK-FORM loaded").count();
        tally.lines.push(format!("    {who}: WORK-FORM onLoad ran {loads} time(s)"));
        tally.check(format!("{who}: WORK-FORM's onLoad ran once"), loads == 1);
    }
    tally.check(
        "the runner's program: a property set and read through the handle, the window closed through it, then the handler stopped at the call through the NULL handle",
        readings(&b.report) == ["told through the handle", "closed", "-"],
    );
    tally.finish(
        "a_window_handler_reaches_its_window_and_is_null_once_it_closes",
        "two forms: OpenFormAsync RETURNING a USAGE OBJECT windowHandler, SetProperty / GetProperty / Close through it, a call through it once closed (NULL): the handler stops and the critical notice shows",
        &root,
        build_ms,
    );
}

/// A handler that fails hands its failure to `onUnhandledException`, which reads it as
/// `me::LastException`, the Developer's Guide's own example ("When a handler fails"). Before any
/// failure the property reads as empty. The failure is a call through a windowHandler whose window
/// has closed, whose words both engines share.
#[test]
fn on_unhandled_exception_reads_last_exception_alike() {
    let root = root("last-exception");
    let mut main = Form::new("MAIN-FORM", "Main", 640, 400);
    main.main_form = true;
    main.user_ws_source = "       01 WS-WORK         USAGE OBJECT GLOBAL.\n".to_owned();
    control(&mut main, "LBL-BEFORE", ControlType::Label, (20, 20, 400, 24), Some("-"));
    control(&mut main, "LBL-CAUGHT", ControlType::Label, (20, 56, 600, 24), Some("-"));
    control(&mut main, "LBL-AFTER", ControlType::Label, (20, 92, 400, 24), Some("-"));
    bind(&mut main, "MAIN-FORM", "onLoad", &["MOVE me::LastException TO LBL-BEFORE::Caption"]);
    bind(&mut main, "MAIN-FORM", "onUnhandledException", &["MOVE me::LastException TO LBL-CAUGHT::Caption"]);
    button(&mut main, "BTN-OPEN", (20, 140), &["INVOKE ME::OpenFormAsync( \"WORK-FORM\" ) RETURNING WS-WORK"]);
    button(&mut main, "BTN-CLOSE", (20, 180), &["INVOKE WS-WORK::Close()"]);
    button(&mut main, "BTN-AGAIN", (20, 220), &["INVOKE WS-WORK::Close()", "MOVE \"went on\" TO LBL-AFTER::Caption"]);
    let mut work = Form::new("WORK-FORM", "Working", 360, 220);
    control(&mut work, "LBL-WORK", ControlType::Label, (20, 20, 300, 24), Some("working"));
    let manifest = project(&root, "LastException", &[("main-form", main), ("work-form", work)], &[]);
    let steps = script(
        &root,
        "steps",
        vec![
            wait(600),
            read("LBL-BEFORE", "Caption"),
            click("BTN-OPEN"),
            wait(600),
            click("BTN-CLOSE"),
            wait(600),
            click("BTN-AGAIN"),
            wait(600),
            read("LBL-CAUGHT", "Caption"),
            read("LBL-AFTER", "Caption"),
            dump(),
            shot("caught"),
        ],
    );
    let (exe, build_ms) = build(&root, &manifest);
    let home = root.join("home");
    let a = interpreter(&root, "main-form", &steps, &home);
    let b = runner(&root, &exe, &steps, &home);
    let mut tally = Tally::default();
    tally.compare("last-exception", &a, &b, &["caught"]);
    let null = cobolt_forms::exceptions::window_handle_is_null("WS-WORK");
    tally.check(
        "the runner's program: empty before any failure, the failure's words in onUnhandledException, the failed handler stopped",
        readings(&b.report) == ["", null.as_str(), "-"],
    );
    tally.finish(
        "on_unhandled_exception_reads_last_exception_alike",
        "me::LastException read in onLoad (no failure yet) and in onUnhandledException after a call through a closed window's handle; the failed handler stops",
        &root,
        build_ms,
    );
}

/// The built program started at `form`, as the IDE's Run of one form starts it (`COBOLT_DESIGNER_FORM`).
fn runner_at(root: &Path, exe: &Path, script: &Path, home: &Path, form: &str) -> Run {
    let shots = root.join(format!("shots-runner-{}", home.file_name().unwrap().to_string_lossy()));
    fs::create_dir_all(&shots).unwrap();
    let t = Instant::now();
    let mut cmd = Command::new(exe);
    cmd.current_dir(root)
        .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
        .env("PRC_HEADLESS_SCRIPT", script)
        .env("PRC_HEADLESS_LIMIT", "60")
        .env(cobolt_runtime::form_host::DESIGNER_FORM_ENV, form);
    pinned(&mut cmd, home, &shots);
    let out = cmd.output().expect("run the built program");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "the built program failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    Run { report: report(&stdout), stdout, shots, ms: t.elapsed().as_secs_f64() * 1000.0 }
}

/// A designer launch of a secondary form: the runner starts at that form, at its designed size and
/// with its own program, as `rcrun run-form` and a Cargo-built program do. A name the program does
/// not carry is refused with exit code 4, naming the forms it carries.
#[test]
fn a_designer_launch_starts_at_the_form_it_names() {
    let root = root("designer");
    let mut main = Form::new("MAIN-FORM", "Main", 640, 400);
    main.main_form = true;
    control(&mut main, "LBL-MAIN", ControlType::Label, (20, 20, 300, 24), Some("main"));
    bind(&mut main, "MAIN-FORM", "onLoad", &["DISPLAY \"MAIN-FORM loaded\""]);
    let mut tool = Form::new("TOOL-FORM", "Tool", 420, 76);
    control(&mut tool, "LBL-TOOL", ControlType::Label, (20, 20, 300, 24), Some("-"));
    bind(&mut tool, "TOOL-FORM", "onLoad", &["DISPLAY \"TOOL-FORM loaded\"", "MOVE \"the tool form's program\" TO LBL-TOOL::Caption"]);
    let manifest = project(&root, "Designer", &[("main-form", main), ("tool-form", tool)], &[]);
    let steps = script(&root, "steps", vec![wait(600), read("LBL-TOOL", "Caption"), dump(), shot("tool")]);
    let (exe, build_ms) = build(&root, &manifest);
    let home = root.join("home");
    let a = interpreter(&root, "tool-form", &steps, &home);
    let b = runner_at(&root, &exe, &steps, &home, " tool-form ");
    let mut tally = Tally::default();
    tally.compare("designer", &a, &b, &["tool"]);
    let size = image::image_dimensions(b.shots.join("tool.png")).unwrap_or((0, 0));
    tally.lines.push(format!("    runner picture: {}x{}", size.0, size.1));
    tally.check("the runner pictured the tool form at its designed size, 420x76", size == (420, 76));
    tally.check(
        "the runner ran the tool form's program and not the main form's",
        readings(&b.report) == ["the tool form's program"] && b.stdout.matches("TOOL-FORM loaded").count() == 1 && !b.stdout.contains("MAIN-FORM loaded"),
    );
    // A form the program does not carry.
    let out = Command::new(&exe)
        .current_dir(&root)
        .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
        .env("LC_ALL", "C")
        .env(cobolt_runtime::form_host::DESIGNER_FORM_ENV, "orders-form")
        .output()
        .expect("run the built program");
    let said = String::from_utf8_lossy(&out.stderr).into_owned();
    tally.lines.push(format!("    orders-form: exit {:?}, {}", out.status.code(), said.trim()));
    tally.check(
        "a form it does not carry: exit code 4, the name and the forms it carries",
        out.status.code() == Some(4) && said.contains("'ORDERS-FORM'") && said.contains("MAIN-FORM, TOOL-FORM"),
    );
    tally.finish(
        "a_designer_launch_starts_at_the_form_it_names",
        "COBOLT_DESIGNER_FORM=\" tool-form \" (a secondary form of 420x76 whose onLoad writes a caption), against rcrun run-form --designer; COBOLT_DESIGNER_FORM=orders-form (not in the program)",
        &root,
        build_ms,
    );
}

/// A Snackbar's `Show()` and `DismissAll()`, a TextBox's `SetFocus()` and `SelectAll()`: commands the
/// desktop's form host takes from a value the program writes, a new one at each call, as the
/// interpreter writes it. The runner used to drop them, so a WebAssembly-built program showed no
/// notification (PowerSpatial's settings form, AC19).
#[test]
fn notifications_and_focus_are_carried_out_alike() {
    let root = root("notify");
    let mut main = Form::new("MAIN-FORM", "Notify", 640, 400);
    main.main_form = true;
    control(&mut main, "SNK-1", ControlType::Snackbar, (20, 330, 400, 48), None);
    control(&mut main, "TXT-1", ControlType::TextBox, (20, 20, 300, 28), None);
    control(&mut main, "TXT-2", ControlType::TextBox, (20, 60, 300, 28), None);
    button(&mut main, "BTN-SHOW", (420, 20), &["MOVE \"Saved\" TO SNK-1::Text", "SNK-1::Show()"]);
    button(&mut main, "BTN-TWICE", (420, 60), &["MOVE \"First\" TO SNK-1::Text", "SNK-1::Show()", "MOVE \"Second\" TO SNK-1::Text", "SNK-1::Show()"]);
    button(&mut main, "BTN-FOCUS", (420, 100), &["MOVE \"select me\" TO TXT-2::Text", "TXT-2::SetFocus()", "TXT-2::SelectAll()"]);
    button(&mut main, "BTN-DISMISS", (420, 140), &["SNK-1::DismissAll()"]);
    let manifest = project(&root, "Notify", &[("main-form", main)], &[]);
    let steps = script(
        &root,
        "steps",
        vec![
            wait(600),
            shot("before"),
            click("BTN-SHOW"),
            wait(300),
            shot("one"),
            click("BTN-TWICE"),
            wait(300),
            shot("three"),
            click("BTN-FOCUS"),
            wait(300),
            read("TXT-2", "_SetFocus"),
            read("TXT-2", "_SelectAll"),
            shot("focus"),
            click("BTN-DISMISS"),
            wait(600),
            dump(),
            shot("dismissed"),
        ],
    );
    let (exe, build_ms) = build(&root, &manifest);
    let home = root.join("home");
    let a = interpreter(&root, "main-form", &steps, &home);
    let b = runner(&root, &exe, &steps, &home);
    let mut tally = Tally::default();
    // The pictures judge the notifications: the form host takes `_ShowSnackbar` when it raises one, so a read of
    // it finds nothing under either engine.
    tally.compare("notify", &a, &b, &["before", "one", "three", "focus", "dismissed"]);
    let got = readings(&b.report);
    tally.lines.push(format!("    runner readings: {got:?}"));
    let n = |i: usize| got.get(i).map(|v| v.parse::<u64>().unwrap_or(0)).unwrap_or(0);
    tally.check("SetFocus() and SelectAll() each wrote a new value", got.len() == 2 && n(0) > 0 && n(1) > 0);
    let changed = |from: &str, to: &str| differing_pixels(&b.shots.join(format!("{from}.png")), &b.shots.join(format!("{to}.png"))).0;
    let (shown, again) = (changed("before", "one"), changed("one", "three"));
    tally.lines.push(format!("    runner: Show() changed {shown} pixels; Show() twice more changed {again}"));
    tally.check("the runner drew the notification, then two more", shown > 0 && shown != u64::MAX && again > 0 && again != u64::MAX);
    tally.finish(
        "notifications_and_focus_are_carried_out_alike",
        "a Snackbar's Show() once and twice in one handler, DismissAll(); a TextBox's SetFocus() and SelectAll()",
        &root,
        build_ms,
    );
}

// ── 6. file_dialogs (T32) ────────────────────────────────────────────────────────────────────────

/// A handler with a `WORKING-STORAGE SECTION` of its own.
fn bind_ws(form: &mut Form, id: &str, event: &str, storage: &[&str], statements: &[&str]) {
    let ws = storage.iter().map(|s| format!("       {s}")).collect::<Vec<_>>().join("\n");
    let body = statements.iter().map(|s| format!("           {s}")).collect::<Vec<_>>().join("\n");
    let code = format!("       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n{ws}\n       PROCEDURE DIVISION.\n{body}\n");
    let paragraph = format!("{}--{}", id.to_ascii_uppercase(), event.to_ascii_uppercase());
    let c = form.controls.iter_mut().find(|c| c.id == id).expect("a control of the fixture");
    c.events.push(EventBinding { event: event.into(), paragraph, code });
}

/// The next file dialog's answer: a path, or `None` to cancel it.
fn answer(path: Option<&str>) -> Value {
    match path {
        Some(p) => json!({"answer": {"path": p}}),
        None => json!({"answer": {"cancel": true}}),
    }
}

#[test]
fn file_dialogs_are_asked_and_answered_alike() {
    let root = root("dialogs");
    let mut main = Form::new("MAIN-FORM", "Dialogs", 640, 400);
    main.main_form = true;
    let path = ["01 WS-PATH PIC X(200)."];
    // Each dialog kind, with the arguments a program may give it, and one the operator cancels.
    let dialogs: [(&str, &str, &str); 4] = [
        ("OPEN", "LBL-OPEN", "COBOL::\"OPEN-FILE-DIALOG\" ( \"Open a ledger\" \"Ledgers|csv,txt\" \"/tmp/ledgers\" WS-PATH )"),
        ("SAVE", "LBL-SAVE", "COBOL::\"SAVE-FILE-DIALOG\" ( \"Save the ledger\" \"Ledgers|csv\" \"ledger.csv\" \"/tmp/out\" WS-PATH )"),
        ("FOLDER", "LBL-FOLDER", "COBOL::\"FOLDER-DIALOG\" ( \"Pick a folder\" WS-PATH )"),
        ("CANCEL", "LBL-CANCEL", "COBOL::\"OPEN-FILE-DIALOG\" ( \"Open\" WS-PATH )"),
    ];
    for (i, (name, label, call)) in dialogs.iter().enumerate() {
        let y = 20 + 40 * i as i32;
        control(&mut main, label, ControlType::Label, (220, y, 400, 24), Some("-"));
        let id = format!("BTN-{name}");
        control(&mut main, &id, ControlType::Button, (20, y, 180, 28), Some(&id));
        let moved = format!("MOVE WS-PATH TO {label}::Caption");
        let shown: Vec<&str> = if *name == "CANCEL" {
            vec!["MOVE SPACES TO WS-PATH", call, "IF WS-PATH = SPACES", "    MOVE \"cancelled\" TO LBL-CANCEL::Caption", "ELSE", &moved, "END-IF"]
        } else {
            vec!["MOVE SPACES TO WS-PATH", call, &moved]
        };
        bind_ws(&mut main, &id, "onClick", &path, &shown);
    }
    let manifest = project(&root, "Dialogs", &[("main-form", main)], &[]);
    let mut steps = vec![wait(600)];
    for (name, label, answered) in [
        ("OPEN", "LBL-OPEN", Some("/data/ledger.csv")),
        ("SAVE", "LBL-SAVE", Some("/data/out.csv")),
        ("FOLDER", "LBL-FOLDER", Some("/data/books")),
        ("CANCEL", "LBL-CANCEL", None),
    ] {
        steps.extend([answer(answered), click(&format!("BTN-{name}")), wait(300), read(label, "Caption")]);
    }
    steps.extend([dump(), shot("dialogs")]);
    let steps = script(&root, "steps", steps);
    let (exe, build_ms) = build(&root, &manifest);
    // The module the Build put in the program says it needs the runner's dialogs (ABI A27, A33).
    let bytes = fs::read(&exe).unwrap_or_default();
    let module = cobolt_program_package::locate(&bytes).ok().and_then(|p| cobolt_program_package::decode(p).ok()).and_then(|d| d.module().ok().map(<[u8]>::to_vec)).unwrap_or_default();
    let needs = module.windows(26).any(|w| w == b"\"capabilities\":[\"dialogs\"]");
    let home = root.join("home");
    let a = interpreter(&root, "main-form", &steps, &home);
    let b = runner(&root, &exe, &steps, &home);
    let mut tally = Tally::default();
    tally.check(format!("the module's manifest lists the capability `dialogs` ({} bytes searched)", module.len()), needs);
    tally.compare("dialogs", &a, &b, &["dialogs"]);
    // What each dialog was asked, as the form host received it.
    let (qa, qb) = (&a.report["dialogs_asked"], &b.report["dialogs_asked"]);
    for (who, q) in [("interpreter", qa), ("runner", qb)] {
        for (i, d) in q.as_array().into_iter().flatten().enumerate() {
            tally.lines.push(format!("    {who} asked {i}: {d}"));
        }
    }
    tally.check("both engines asked the form host the same 4 dialogs", qa == qb && qa.as_array().is_some_and(|q| q.len() == 4));
    let want = json!([
        {"kind": "open", "title": "Open a ledger", "filters": [["Ledgers", ["csv", "txt"]]], "directory": "/tmp/ledgers", "file_name": null},
        {"kind": "save", "title": "Save the ledger", "filters": [["Ledgers", ["csv"]]], "directory": "/tmp/out", "file_name": "ledger.csv"},
        {"kind": "folder", "title": "Pick a folder", "filters": [], "directory": null, "file_name": null},
        {"kind": "open", "title": "Open", "filters": [], "directory": null, "file_name": null},
    ]);
    tally.check("the runner asked what the program wrote: titles, filters, folders, names", *qb == want);
    tally.check(
        "the runner's program received each answer: a file, a save path, a folder, spaces when cancelled",
        readings(&b.report) == ["/data/ledger.csv", "/data/out.csv", "/data/books", "cancelled"],
    );
    tally.finish(
        "file_dialogs_are_asked_and_answered_alike",
        "COBOL::\"OPEN-FILE-DIALOG\" (title, filter, start folder), \"SAVE-FILE-DIALOG\" (title, filter, file name, start folder), \"FOLDER-DIALOG\" (title), and an open dialog the operator cancels; each answered by the script's `answer` step",
        &root,
        build_ms,
    );
}

// ── 6b. FileDropZone (T39c) ──────────────────────────────────────────────────────────────────────

/// A FileDropZone of the fixture: its rules as designed properties.
fn zone(form: &mut Form, id: &str, y: i32, props: &[(&str, &str)]) {
    control(form, id, ControlType::FileDropZone, (20, y, 200, 110), None);
    let c = form.controls.iter_mut().find(|c| c.id == id).expect("the zone just added");
    for (k, v) in props {
        c.set_prop(*k, PropValue::String((*v).into()));
    }
}

/// The files under `dirs` of `root`, each as `<dir>/<name> <bytes>`, sorted; then the folders removed,
/// so the next engine copies into empty ones.
fn landed_and_cleared(root: &Path, dirs: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for d in dirs {
        for e in fs::read_dir(root.join(d)).into_iter().flatten().flatten() {
            let len = e.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(format!("{d}/{} {len}", e.file_name().to_string_lossy()));
        }
        let _ = fs::remove_dir_all(root.join(d));
    }
    out.sort();
    out
}

#[test]
fn file_drop_zones_take_stage_and_commit_alike() {
    let root = root("dropzone");
    let mut main = Form::new("MAIN-FORM", "Drop zones", 900, 540);
    main.main_form = true;
    // A: staged for review in a list, every row left ticked, committed by a button.
    zone(&mut main, "FDZ-A", 20, &[("StageOnly", "true"), ("FileListControl", "LST-A"), ("DestinationFolder", "inbox"), ("AllowedExtensions", "txt,csv")]);
    control(&mut main, "LST-A", ControlType::ListBox, (240, 20, 300, 110), None);
    control(&mut main, "LBL-A", ControlType::Label, (560, 20, 320, 24), Some("-"));
    bind(&mut main, "FDZ-A", "onFilesDropped", &["MOVE \"staged\" TO LBL-A::Caption"]);
    button(&mut main, "BTN-COMMIT-A", (560, 60), &["MOVE FDZ-A::CommitFiles() TO LBL-A::Caption"]);
    // B: staged, every row unticked before the commit, so nothing is copied.
    zone(&mut main, "FDZ-B", 150, &[("StageOnly", "true"), ("FileListControl", "LST-B"), ("DestinationFolder", "outbox")]);
    control(&mut main, "LST-B", ControlType::ListBox, (240, 150, 300, 110), None);
    control(&mut main, "LBL-B", ControlType::Label, (560, 150, 320, 24), Some("-"));
    button(&mut main, "BTN-COMMIT-B", (560, 190), &["MOVE SPACES TO LST-B::CheckedItems", "MOVE FDZ-B::CommitFiles() TO LBL-B::Caption"]);
    // C: copied at once (PowerCloud's use), one file refused by its extension.
    zone(&mut main, "FDZ-C", 280, &[("DestinationFolder", "now"), ("AllowedExtensions", "txt")]);
    control(&mut main, "LBL-C", ControlType::Label, (560, 280, 320, 24), Some("-"));
    control(&mut main, "LBL-R", ControlType::Label, (560, 320, 320, 24), Some("-"));
    bind(&mut main, "FDZ-C", "onFilesDropped", &["MOVE \"dropped\" TO LBL-C::Caption"]);
    bind(&mut main, "FDZ-C", "onFilesRejected", &["MOVE \"refused\" TO LBL-R::Caption"]);
    // D: staged with a review list the form does not hold, then committed.
    zone(&mut main, "FDZ-D", 410, &[("StageOnly", "true"), ("FileListControl", "LST-GONE"), ("DestinationFolder", "gone")]);
    control(&mut main, "LBL-D", ControlType::Label, (560, 410, 320, 24), Some("-"));
    button(&mut main, "BTN-COMMIT-D", (560, 450), &["MOVE FDZ-D::CommitFiles() TO LBL-D::Caption"]);
    let manifest = project(&root, "DropZones", &[("main-form", main)], &[]);
    // A project has `assets/`: a built program anchors a relative folder at the folder above it that
    // holds one (the project, as `rcrun run-form` does), as a Cargo-built program does; without it,
    // at `bin/`.
    fs::create_dir_all(root.join("assets")).unwrap();
    // The files the operator picks, of four sizes.
    let files = root.join("files");
    fs::create_dir_all(&files).unwrap();
    let picked: Vec<String> = [("ledger.txt", 1_234usize), ("notes.csv", 56), ("memo.txt", 7_890), ("tool.exe", 99), ("plan.txt", 321)]
        .iter()
        .map(|(name, len)| {
            let p = files.join(name);
            fs::write(&p, "x".repeat(*len)).unwrap();
            p.display().to_string()
        })
        .collect();
    let mut steps = vec![wait(600)];
    for (file, zone) in [(0, "FDZ-A"), (1, "FDZ-A"), (2, "FDZ-B"), (3, "FDZ-C"), (4, "FDZ-C"), (0, "FDZ-D")] {
        // A real pointer click: the zone opens its picker, which takes the queued answer.
        steps.extend([answer(Some(&picked[file])), json!({"click": {"control": zone}}), wait(300)]);
    }
    steps.extend([
        read("FDZ-A", "StagedFiles"),
        read("LST-A", "Items"),
        read("LST-A", "CheckedItems"),
        read("LBL-A", "Caption"),
        read("FDZ-B", "StagedFiles"),
        read("FDZ-C", "DroppedFiles"),
        read("FDZ-C", "RejectedFiles"),
        read("LBL-C", "Caption"),
        read("LBL-R", "Caption"),
        shot("staged"),
        click("BTN-COMMIT-A"),
        wait(300),
        click("BTN-COMMIT-B"),
        wait(300),
        click("BTN-COMMIT-D"),
        wait(300),
        read("LBL-A", "Caption"),
        read("FDZ-A", "CommitSummary"),
        read("FDZ-A", "DroppedFiles"),
        read("LST-A", "Items"),
        read("LST-A", "CheckedItems"),
        read("LBL-B", "Caption"),
        read("FDZ-B", "DroppedFiles"),
        read("LST-B", "Items"),
        read("LBL-D", "Caption"),
        read("FDZ-D", "DroppedFiles"),
        // A second commit of the zone whose list is not there: only the committed rows count as ticked.
        click("BTN-COMMIT-D"),
        wait(300),
        read("LBL-D", "Caption"),
        dump(),
        shot("committed"),
    ]);
    let steps = script(&root, "steps", steps);
    let (exe, build_ms) = build(&root, &manifest);
    let home = root.join("home");
    let dirs = ["inbox", "outbox", "now", "gone"];
    let a = interpreter(&root, "main-form", &steps, &home);
    let copied_a = landed_and_cleared(&root, &dirs);
    let b = runner(&root, &exe, &steps, &home);
    let copied_b = landed_and_cleared(&root, &dirs);
    let mut tally = Tally::default();
    tally.compare("drop zones", &a, &b, &["staged", "committed"]);
    for (who, c) in [("interpreter", &copied_a), ("runner", &copied_b)] {
        tally.lines.push(format!("    {who} copied: {}", c.join(", ")));
    }
    tally.check("both engines copied the same files, of the same sizes", copied_a == copied_b);
    let want = ["gone/ledger.txt 1234", "inbox/ledger.txt 1234", "inbox/notes.csv 56", "now/plan.txt 321"];
    tally.check("the runner copied the two ticked staged files, the one allowed at once and the one staged for a missing list, and nothing unticked or refused", copied_b == want);
    tally.check("a second commit of the zone whose list is missing copied nothing", readings(&b.report).last().map(String::as_str) == Some("0 of 0 copied, 0.000 MB"));
    let got = readings(&b.report);
    tally.check("the runner's program heard the drop and the refusal", got.get(7).map(String::as_str) == Some("dropped") && got.get(8).map(String::as_str) == Some("refused"));
    let summary = got.get(9).cloned().unwrap_or_default();
    tally.check(format!("CommitFiles() answered its CommitSummary ({summary:?})"), !summary.is_empty() && got.get(10) == Some(&summary));
    tally.finish(
        "file_drop_zones_take_stage_and_commit_alike",
        "four FileDropZones fed by the picker (the script's `answer`): one staging two files into a review ListBox and committed with `CommitFiles()`, one staging a file whose row is unticked before `CommitFiles()`, one copying at once with a file refused by AllowedExtensions, and one whose FileListControl names a list the form does not hold, committed twice; onFilesDropped and onFilesRejected handlers",
        &root,
        build_ms,
    );
}

// ── 6c. IndexedFile (T39c) ───────────────────────────────────────────────────────────────────────

/// The files a run left in `data/`, by name with their bytes, sorted; then the folder emptied, so the
/// next engine starts with no file.
fn data_and_cleared(root: &Path) -> Vec<(String, Vec<u8>)> {
    let dir = root.join("data");
    let mut out: Vec<(String, Vec<u8>)> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), fs::read(e.path()).unwrap_or_default()))
        .collect();
    out.sort();
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    out
}

#[test]
fn an_indexed_file_control_opens_writes_and_reads_alike() {
    let root = root("indexedfile");
    let mut main = Form::new("MAIN-FORM", "Items", 640, 420);
    main.main_form = true;
    // The file the control names, declared by the form (spec 108 R10: only such a form gets the
    // generated paragraphs) and GLOBAL, so its handlers drive it with the verbs; OPTIONAL, so the
    // OPEN the control makes as the form starts creates it (05). No FILE STATUS clause: every status
    // the form shows is the one `COBOL::"FILE-STATUS"` reads, as the paragraphs read it. Its
    // `SELECT` name (ITEM-FILE) is not the `.cidx` stem (ITEMS): the paragraphs name the file as the
    // `SELECT` does (2.0.589), else `AutoOpen` opens a file no program declares.
    main.cobol_structure.file_control = "           SELECT OPTIONAL ITEM-FILE ASSIGN TO \"data/items.idx\"\n               ORGANIZATION IS INDEXED\n               ACCESS MODE IS DYNAMIC\n               RECORD KEY IS IT-ID.".into();
    main.cobol_structure.file_section = "       FD ITEM-FILE IS GLOBAL.\n       01 IT-REC.\n           05 IT-ID   PIC 9(3).\n           05 IT-NAME PIC X(8).".into();
    main.user_ws_source = "       01 WS-IX-FS    PIC XX   GLOBAL VALUE \"99\".\n       01 WS-ST       PIC XX   GLOBAL.\n       01 WS-OPERATOR PIC X(8) GLOBAL VALUE \"ADA\".\n".into();
    let mut ixf = Control::new("IXF-ITEMS", ControlType::IndexedFile, 0, 0);
    for (k, v) in [
        ("IndexedFile", "indexed/items.cidx"),
        ("OpenMode", "I-O"),
        ("RecordName", "IT-REC"),
        ("KeyName", "IT-ID"),
        ("StatusDataItem", "WS-IX-FS"),
        ("OperatorName", "WS-OPERATOR"),
    ] {
        ixf.set_prop(k, PropValue::String(v.into()));
    }
    ixf.set_prop("AutoOpen", PropValue::Bool(true));
    main.controls.push(ixf);
    for (i, id) in ["LBL-OPEN", "LBL-W", "LBL-R", "LBL-N", "LBL-X", "LBL-B", "LBL-C"].iter().enumerate() {
        control(&mut main, id, ControlType::Label, (240, 20 + 44 * i as i32, 360, 24), Some("-"));
    }
    // What the control's OPEN left in its StatusDataItem as the form started.
    bind(&mut main, "MAIN-FORM", "onLoad", &["MOVE WS-IX-FS TO LBL-OPEN::Caption"]);
    // Two records, then the first again: a duplicate key (22).
    button(
        &mut main,
        "BTN-WRITE",
        (20, 20),
        &[
            "MOVE 1 TO IT-ID",
            "MOVE \"ONE\" TO IT-NAME",
            "WRITE IT-REC INVALID KEY CONTINUE END-WRITE",
            "MOVE 2 TO IT-ID",
            "MOVE \"TWO\" TO IT-NAME",
            "WRITE IT-REC INVALID KEY CONTINUE END-WRITE",
            "MOVE 1 TO IT-ID",
            "WRITE IT-REC INVALID KEY CONTINUE END-WRITE",
            "COBOL::\"FILE-STATUS\" ( \"ITEM-FILE\" WS-ST )",
            "MOVE WS-ST TO LBL-W::Caption",
        ],
    );
    // A key that is not there (23), then one that is.
    button(
        &mut main,
        "BTN-READ",
        (20, 64),
        &[
            "MOVE 7 TO IT-ID",
            "READ ITEM-FILE INVALID KEY CONTINUE END-READ",
            "COBOL::\"FILE-STATUS\" ( \"ITEM-FILE\" WS-ST )",
            "MOVE WS-ST TO LBL-R::Caption",
            "MOVE 2 TO IT-ID",
            "READ ITEM-FILE INVALID KEY CONTINUE END-READ",
            "MOVE IT-NAME TO LBL-N::Caption",
        ],
    );
    // A name that is no file (00), and the member every control answers.
    button(
        &mut main,
        "BTN-OTHER",
        (20, 108),
        &["MOVE \"XX\" TO WS-ST", "COBOL::\"FILE-STATUS\" ( \"NO-SUCH\" WS-ST )", "MOVE WS-ST TO LBL-X::Caption", "MOVE IXF-ITEMS::IsBusy() TO LBL-B::Caption"],
    );
    // The file closed by the program itself, so what it leaves does not wait for the form's end: a
    // headless run ends without waiting for the program to finish (WASM-PARITY #75).
    button(&mut main, "BTN-CLOSE", (20, 152), &["CLOSE ITEM-FILE", "COBOL::\"FILE-STATUS\" ( \"ITEM-FILE\" WS-ST )", "MOVE WS-ST TO LBL-C::Caption"]);
    let manifest = project(&root, "IndexedItems", &[("main-form", main)], &[]);
    // A relative ASSIGN starts at the folder that holds `assets/` on both engines.
    fs::create_dir_all(root.join("assets")).unwrap();
    let steps = script(
        &root,
        "steps",
        vec![
            wait(600),
            read("LBL-OPEN", "Caption"),
            click("BTN-WRITE"),
            wait(300),
            click("BTN-READ"),
            wait(300),
            click("BTN-OTHER"),
            wait(300),
            click("BTN-CLOSE"),
            wait(300),
            read("LBL-W", "Caption"),
            read("LBL-R", "Caption"),
            read("LBL-N", "Caption"),
            read("LBL-X", "Caption"),
            read("LBL-B", "Caption"),
            read("LBL-C", "Caption"),
            read("IXF-ITEMS", "OpenMode"),
            dump(),
            shot("items"),
        ],
    );
    let (exe, build_ms) = build(&root, &manifest);
    let home = root.join("home");
    let _ = data_and_cleared(&root);
    let a = interpreter(&root, "main-form", &steps, &home);
    let files_a = data_and_cleared(&root);
    let b = runner(&root, &exe, &steps, &home);
    let files_b = data_and_cleared(&root);
    let mut tally = Tally::default();
    tally.compare("indexed file", &a, &b, &["items"]);
    let listing = |files: &[(String, Vec<u8>)]| files.iter().map(|(n, d)| format!("{n} {}", d.len())).collect::<Vec<_>>().join(", ");
    for (who, f) in [("interpreter", &files_a), ("runner", &files_b)] {
        tally.lines.push(format!("    {who} left in data/: {}", listing(f)));
    }
    tally.check("both engines left the same data files, byte for byte", !files_a.is_empty() && files_a == files_b);
    let want = ["05", "22", "23", "TWO", "00", "0", "00", "I-O"];
    let got = readings(&b.report);
    tally.check(
        format!("the runner's statuses are the engine's: OPEN of the absent OPTIONAL file 05, a duplicate 22, a missing key 23, no such file 00, CLOSE 00 ({got:?})"),
        got == want,
    );
    tally.finish(
        "an_indexed_file_control_opens_writes_and_reads_alike",
        "an IndexedFile control with AutoOpen on an OPTIONAL INDEXED file the form declares GLOBAL, its OPEN WITH REGISTERED USER a data item, its StatusDataItem filled by COBOL::\"FILE-STATUS\"; handlers that WRITE (one a duplicate), READ (one key missing) and ask the status of a name that is no file, IsBusy(), and CLOSE the file",
        &root,
        build_ms,
    );
}

/// A silent WAV of `ms` milliseconds (8 kHz, 8-bit mono PCM), as the runtime's own sound tests make it.
fn silent_wav(ms: u32) -> Vec<u8> {
    let rate: u32 = 8000;
    let data_len = rate * ms / 1000;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes()); // bytes per second
    b.extend_from_slice(&1u16.to_le_bytes()); // block align
    b.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    b.resize(44 + data_len as usize, 0x80);
    b
}

#[test]
fn sounds_are_played_and_answered_alike() {
    let root = root("audio");
    let mut main = Form::new("MAIN-FORM", "Sounds", 640, 480);
    main.main_form = true;
    let storage = ["01 WS-STATUS PIC X(10).", "01 WS-STATE PIC X(10).", "01 WS-POS PIC 9(7).", "01 WS-LEN PIC 9(7).", "01 WS-SHOW PIC X(40)."];
    // Each verb, a sound played again from another form's point of view (a second door), a file that
    // is not there and one that is not a sound. Each label shows the status, then the state and the
    // length AUDIO-STATE reads after it (the position moves with the clock, so it is not shown).
    let cases: [(&str, &str); 8] = [
        ("PLAY", "COBOL::\"AUDIO-PLAY\" ( \"assets/sounds/tone.wav\" WS-STATUS )"),
        ("PAUSE", "COBOL::\"AUDIO-PAUSE\" ( WS-STATUS )"),
        ("RESUME", "COBOL::\"AUDIO-RESUME\" ( WS-STATUS )"),
        ("STOP", "COBOL::\"AUDIO-STOP\" ( WS-STATUS )"),
        ("IDLE", "COBOL::\"AUDIO-PAUSE\" ( WS-STATUS )"),
        ("MISSING", "COBOL::\"AUDIO-PLAY\" ( \"assets/sounds/none.wav\" WS-STATUS )"),
        ("BAD", "COBOL::\"AUDIO-PLAY\" ( \"assets/sounds/text.wav\" WS-STATUS )"),
        ("QUIET", "COBOL::\"AUDIO-STOP\"()"),
    ];
    for (i, (name, call)) in cases.iter().enumerate() {
        let y = 16 + 52 * i as i32;
        let label = format!("LBL-{name}");
        control(&mut main, &label, ControlType::Label, (220, y, 400, 24), Some("-"));
        let id = format!("BTN-{name}");
        control(&mut main, &id, ControlType::Button, (20, y, 180, 28), Some(&id));
        let shown = format!("MOVE WS-SHOW TO {label}::Caption");
        bind_ws(
            &mut main,
            &id,
            "onClick",
            &storage,
            &[
                "MOVE \"(none)\" TO WS-STATUS",
                "MOVE SPACES TO WS-SHOW",
                call,
                "COBOL::\"AUDIO-STATE\" ( WS-STATE WS-POS WS-LEN )",
                "STRING WS-STATUS DELIMITED BY SPACE \"/\"",
                "    WS-STATE DELIMITED BY SPACE \"/\"",
                "    WS-LEN DELIMITED BY SIZE INTO WS-SHOW",
                &shown,
            ],
        );
    }
    let manifest = project(&root, "Sounds", &[("main-form", main)], &[]);
    // The sounds, where a relative path finds them under both engines: the project's `assets/`.
    let sounds = root.join("assets/sounds");
    fs::create_dir_all(&sounds).unwrap();
    fs::write(sounds.join("tone.wav"), silent_wav(10_000)).unwrap();
    fs::write(sounds.join("text.wav"), b"this is not a sound").unwrap();
    let mut steps = vec![wait(600)];
    for (name, _) in &cases {
        steps.extend([click(&format!("BTN-{name}")), wait(200), read(&format!("LBL-{name}"), "Caption")]);
    }
    steps.extend([dump(), shot("sounds")]);
    let steps = script(&root, "steps", steps);
    let (exe, build_ms) = build(&root, &manifest);
    // The module the Build put in the program says it needs the runner's sound player (ABI A27, A36).
    let bytes = fs::read(&exe).unwrap_or_default();
    let module = cobolt_program_package::locate(&bytes).ok().and_then(|p| cobolt_program_package::decode(p).ok()).and_then(|d| d.module().ok().map(<[u8]>::to_vec)).unwrap_or_default();
    let needs = module.windows(24).any(|w| w == b"\"capabilities\":[\"audio\"]");
    let home = root.join("home");
    let a = interpreter(&root, "main-form", &steps, &home);
    let b = runner(&root, &exe, &steps, &home);
    let mut tally = Tally::default();
    tally.check(format!("the module's manifest lists the capability `audio` ({} bytes searched)", module.len()), needs);
    tally.compare("sounds", &a, &b, &["sounds"]);
    let got = readings(&b.report);
    for (i, r) in got.iter().enumerate() {
        tally.lines.push(format!("    runner read {}: {r}", cases.get(i).map_or("?", |c| c.0)));
    }
    let starts = |i: usize, p: &str| got.get(i).is_some_and(|r| r.starts_with(p));
    tally.check("the runner played the sound, paused, resumed and stopped it: OK/PLAYING, OK/PAUSED, OK/PLAYING, OK/IDLE", starts(0, "OK/PLAYING/") && starts(1, "OK/PAUSED/") && starts(2, "OK/PLAYING/") && starts(3, "OK/IDLE/"));
    tally.check("a sound of 10 000 ms reads its length while it plays", starts(0, "OK/PLAYING/0010000"));
    tally.check("a missing file answers NOFILE, a file that is not a sound BADFORMAT", starts(5, "NOFILE/") && starts(6, "BADFORMAT/"));
    tally.check("a verb with no status item leaves the item as it was", starts(7, "(none)/"));
    tally.finish(
        "sounds_are_played_and_answered_alike",
        "COBOL::\"AUDIO-PLAY\" (a relative file, with its status), \"AUDIO-PAUSE\", \"AUDIO-RESUME\", \"AUDIO-STOP\", a PAUSE with nothing playing, a missing file, a file that is not a sound, a STOP with no status item; \"AUDIO-STATE\" after each; COBOLT_AUDIO=null",
        &root,
        build_ms,
    );
}

// ── 8. T50's: the canvases and the graphics objects ──────────────────────────────────────────────

/// A graphics control of type `kind` with its properties.
fn graphic(form: &mut Form, id: &str, kind: &str, rect: (i32, i32, i32, i32), props: &[(&str, PropValue)]) {
    let mut c = Control::new(id, ControlType::from_str(kind), rect.0, rect.1);
    c.rect = Rect::new(rect.0, rect.1, rect.2, rect.3);
    for (k, v) in props {
        c.set_prop(*k, v.clone());
    }
    form.controls.push(c);
}

#[test]
fn canvases_and_graphics_objects_draw_and_answer_alike() {
    let root = root("graphics");
    let mut main = Form::new("GFX-FORM", "Graphics", 640, 480);
    main.main_form = true;
    graphic(&mut main, "CNV-1", "Canvas2D", (16, 16, 240, 140), &[]);
    // Two canvases that draw in `onFrame` and never say `Present`: AutoPresent shows the first, the second stays blank.
    let frame = ["CNV-2::SetFillColor( \"#20A040\" )", "CNV-2::FillRect( 10 10 60 40 )"];
    graphic(&mut main, "CNV-2", "Canvas2D", (272, 16, 120, 80), &[("TargetFps", PropValue::Int(10))]);
    bind(&mut main, "CNV-2", "onFrame", &frame);
    let frame3: Vec<String> = frame.iter().map(|s| s.replace("CNV-2", "CNV-3")).collect();
    graphic(&mut main, "CNV-3", "Canvas2D", (408, 16, 120, 80), &[("TargetFps", PropValue::Int(10)), ("AutoPresent", PropValue::Bool(false))]);
    bind(&mut main, "CNV-3", "onFrame", &frame3.iter().map(String::as_str).collect::<Vec<_>>());
    graphic(&mut main, "IMG-1", "ImageBuffer", (548, 16, 32, 32), &[]);
    graphic(&mut main, "PHY-1", "Physics2D", (548, 56, 32, 32), &[]);
    graphic(&mut main, "PLT-1", "Plot", (548, 96, 32, 32), &[]);
    let cases: [(&str, &[&str], &[&str]); 7] = [
        (
            "DRAW",
            &["01 WS-PX PIC X(20).", "01 WS-W PIC X(20).", "01 WS-SHOW PIC X(40)."],
            &[
                "CNV-1::SetFillColor( \"#2060C0\" )",
                "CNV-1::FillRect( 10 10 120 60 )",
                "CNV-1::SetFillColor( \"#E04020\" )",
                "CNV-1::FillRect( 70 40 120 70 )",
                "CNV-1::SetFont( \"16px Ubuntu\" )",
                "CNV-1::SetFillColor( \"#FFFFFF\" )",
                "CNV-1::DrawText( \"Hello\" 20 120 0 1 )",
                "CNV-1::Present()",
                "SET WS-PX TO CNV-1::GetPixel( 20 20 )",
                "SET WS-W TO CNV-1::MeasureText( \"Hello\" )",
                "STRING WS-PX DELIMITED BY SPACE \"/\" WS-W DELIMITED BY SPACE INTO WS-SHOW",
                "MOVE WS-SHOW TO LBL-DRAW::Caption",
            ],
        ),
        (
            "IMAGE",
            &["01 WS-W PIC X(20)."],
            &["IMG-1::Create( 40 30 \"#FFD000\" )", "IMG-1::DrawOnto( CNV-1::GetHandle() 180 90 )", "CNV-1::Present()", "SET WS-W TO IMG-1::GetWidth()", "MOVE WS-W TO LBL-IMAGE::Caption"],
        ),
        (
            "PHYS",
            &["01 WS-WORLD PIC X(20).", "01 WS-BODY PIC X(20).", "01 WS-POS PIC X(40)."],
            &[
                "SET WS-WORLD TO PHY-1::CreateWorld( 0 -9.81 )",
                "SET WS-BODY TO PHY-1::AddBody( WS-WORLD 1 0 10 )",
                "PHY-1::AddBox( WS-WORLD WS-BODY 0.5 0.5 )",
                "PHY-1::Step( WS-WORLD 0.5 )",
                "SET WS-POS TO PHY-1::GetPosition( WS-WORLD WS-BODY )",
                "MOVE WS-POS TO LBL-PHYS::Caption",
            ],
        ),
        (
            "PLOT",
            &["01 WS-S PIC X(20).", "01 WS-N PIC X(20)."],
            &["SET WS-S TO PLT-1::AddSeries( \"line\" \"a\" )", "PLT-1::AddPoint( WS-S 0 1 )", "PLT-1::AddPoint( WS-S 1 3 )", "SET WS-N TO PLT-1::GetSeriesCount()", "MOVE WS-N TO LBL-PLOT::Caption"],
        ),
        // The chart drawn onto the canvas by its handle, between the canvas's own `Clear` and
        // `Present` (examples/PowerGraphics charts-form): the `Clear` must be drawn before the chart.
        (
            "CHART",
            &["01 WS-CNV PIC X(20)."],
            &["SET WS-CNV TO CNV-1::GetHandle()", "CNV-1::Clear( \"#101820\" )", "PLT-1::RenderToCanvas( WS-CNV 0 0 240 140 )", "CNV-1::Present()", "MOVE \"DRAWN\" TO LBL-CHART::Caption"],
        ),
        // Drawing, then a gradient whose colour stops the module hands to the host as a call, then
        // more drawing (examples/PowerGraphics pool2d-form): what was drawn before the gradient
        // must reach the canvas whole.
        (
            "GRADIENT",
            &[],
            &[
                "CNV-1::Clear( \"#000000\" )",
                "CNV-1::SetFillColor( \"#FFFFFF\" )",
                "CNV-1::FillRect( 0 0 120 140 )",
                "CNV-1::SetLinearGradient( 0 120 0 240 0 \"0 4278190335 1 16711935\" )",
                "CNV-1::FillRect( 120 0 120 140 )",
                "CNV-1::Present()",
                "MOVE \"DRAWN\" TO LBL-GRADIENT::Caption",
            ],
        ),
        // A drawing call the library refuses, and a read-back it refuses: each says why in `LastError`.
        ("BAD", &["01 WS-PX PIC X(20)."], &["CNV-1::FillRect( \"wide\" )", "SET WS-PX TO PHY-1::GetBodyCount( 99 )", "MOVE PHY-1::LastError TO LBL-BAD::Caption"]),
    ];
    for (i, (name, storage, statements)) in cases.iter().enumerate() {
        let y = 180 + 44 * i as i32;
        control(&mut main, &format!("LBL-{name}"), ControlType::Label, (220, y, 400, 24), Some("-"));
        let id = format!("BTN-{name}");
        control(&mut main, &id, ControlType::Button, (20, y, 180, 28), Some(&id));
        bind_ws(&mut main, &id, "onClick", storage, statements);
    }
    let manifest = project(&root, "Graphics", &[("gfx-form", main)], &[]);
    let mut steps = vec![wait(600)];
    for (name, _, _) in &cases {
        steps.extend([click(&format!("BTN-{name}")), wait(200), read(&format!("LBL-{name}"), "Caption")]);
        if *name == "DRAW" || *name == "IMAGE" || *name == "CHART" || *name == "GRADIENT" {
            steps.push(shot(&format!("after-{}", name.to_ascii_lowercase())));
        }
    }
    steps.extend([read("CNV-1", "LastError"), wait(500), shot("frames"), dump()]);
    let steps = script(&root, "steps", steps);
    let (exe, build_ms) = build(&root, &manifest);
    let home = root.join("home");
    let a = interpreter(&root, "gfx-form", &steps, &home);
    let b = runner(&root, &exe, &steps, &home);
    let mut tally = Tally::default();
    tally.compare("graphics", &a, &b, &["after-draw", "after-image", "after-chart", "after-gradient", "frames"]);
    let got = readings(&b.report);
    for (i, r) in got.iter().enumerate() {
        tally.lines.push(format!("    runner read {i}: {r}"));
    }
    let at = |i: usize| got.get(i).map(String::as_str).unwrap_or("");
    tally.check(format!("GetPixel reads the blue drawn at (20, 20): {} = 0x2060C0FF", 0x2060_C0FFu32), at(0).starts_with(&format!("{}/", 0x2060_C0FFu32)));
    tally.check("an image of 40 x 30 was made", at(1) == "40");
    tally.check("a body fell under gravity (its position was read)", !at(2).is_empty() && at(2) != "-");
    tally.check("the chart has one series", at(3) == "1");
    tally.check("the chart was drawn onto the canvas by its handle", at(4) == "DRAWN");
    tally.check("the drawing before and after a gradient the host sets was done", at(5) == "DRAWN");
    tally.check("a refused read-back says why in LastError", at(6).contains("refused"));
    tally.check("a refused drawing call says why in LastError", at(7).contains("refused"));
    tally.finish(
        "canvases_and_graphics_objects_draw_and_answer_alike",
        "a Canvas2D drawn (rectangles, text, Present) and read back (GetPixel, MeasureText); an ImageBuffer made and drawn onto it by handle; a Physics2D world stepped and read; a Plot series, drawn onto the canvas by its handle between the canvas's Clear and Present; drawing before and after a gradient whose colour stops go to the host as a call; a refused drawing call and a refused read-back (LastError); two canvases drawing in onFrame at TargetFps 10, AutoPresent on and off",
        &root,
        build_ms,
    );
}
