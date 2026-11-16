// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T31 and T32: **the runner's form engine and the interpreter agree on several
//! forms, the application shell and its SideMenu, and the file dialogs** (R9, R10, R27; AC14 part).
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
//!    what each asked the form host for.
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
        .env(cobolt_compiler::WASM_FORMS_ENV, "1")
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
        println!("== {title} (DESKTOP-WASM-001 T31: R9, R10, R27; AC14 part) ==");
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
