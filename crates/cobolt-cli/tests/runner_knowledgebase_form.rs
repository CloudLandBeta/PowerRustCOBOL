// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T47b (R10, R11, ABI A49): **the KnowledgeBase control behaves on the
//! WebAssembly path as it does under the interpreter.**
//!
//! One form with two KnowledgeBases using the Lexical embedder, which needs no server: `KB-1` on a
//! folder of the test's own, collection `docs`; `KB-2` with no collection. The script, run in real
//! time, as each operation runs on a thread of its own:
//! - `CreateCollection`, `ListCollections`, `GetCollection`;
//! - `AddDocument`, and a second operation while it runs, which is refused with `LastError`;
//!   `onProgress` and `onIndexed`, each handler reading the values its own event carried;
//! - a second `AddDocument`, `ImportDocument` of a file, `ListDocuments`, `GetDocument`;
//! - `Search` with a maximum, `onSearchComplete` reading `ResultCount`, `SearchMode` and the
//!   first result's document, heading, score and passage;
//! - `DeleteDocument`; `onError` on the control with no collection;
//! - `Refresh` then `Cancel()`: `onCancelled`, no longer busy, and its reports dropped.
//!
//! Each engine starts from an empty folder. The script runs headless under `rcrun run-form` (the
//! interpreter) and in the program `rcrun build` makes on the WebAssembly path. The dumps must be
//! equal and the pictures equal in every pixel.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

const FORM: &str = "PARITY-KB";

/// How many `onIndexed` events `KB-1` raised, seen by every handler.
const FORM_WS: &str = "       01 WS-IDX                  GLOBAL PIC 9(2) VALUE 0.";

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

/// The fixture project in `root`, its KnowledgeBase on the folder `kb` and importing `import`; its
/// manifest's path.
fn project(root: &Path, kb: &Path, import: &Path) -> PathBuf {
    let _ = fs::remove_dir_all(root);
    for d in ["forms", "generated", "src"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    fs::write(import, "Indexed files keep their records in key order for COBOL programs.").unwrap();
    let s = |v: &str| PropValue::String(v.into());
    let mut form = Form::new(FORM, "KnowledgeBase form", 760, 560);
    form.main_form = true;
    form.user_ws_source = FORM_WS.into();
    let buttons = ["BTN-COLL", "BTN-ADD", "BTN-ADD2", "BTN-IMPORT", "BTN-LIST", "BTN-SEARCH", "BTN-DEL", "BTN-ERR", "BTN-CANCEL"];
    for (i, id) in buttons.iter().enumerate() {
        control(&mut form, id, ControlType::Button, (20, 20 + 38 * i as i32, 120, 28), &[("Caption", s(id.trim_start_matches("BTN-")))]);
    }
    let labels = ["LBL-COLL", "LBL-ADD", "LBL-BUSYERR", "LBL-PROG", "LBL-IDX", "LBL-DOCS", "LBL-SRCH", "LBL-PASS", "LBL-KB2", "LBL-CANCEL", "LBL-CANCELLED"];
    for (i, id) in labels.iter().enumerate() {
        control(&mut form, id, ControlType::Label, (160, 16 + 30 * i as i32, 580, 26), &[("Caption", s("-"))]);
    }
    let location = kb.display().to_string();
    control(
        &mut form,
        "KB-1",
        ControlType::KnowledgeBase,
        (680, 500, 56, 56),
        &[("Location", s(&location)), ("Collection", s("docs")), ("Embedder", s("Lexical")), ("MaximumResults", PropValue::Int(3))],
    );
    control(&mut form, "KB-2", ControlType::KnowledgeBase, (620, 500, 56, 56), &[("Location", s(&location)), ("Embedder", s("Lexical"))]);

    bind(
        &mut form,
        "BTN-COLL",
        "onClick",
        "       01 WS-A                    PIC X.\n       01 WS-N                    PIC 9(2).\n       01 WS-C                    PIC X(10).\n       01 WS-T                    PIC X(60).",
        &lines(&[
            "MOVE KB-1::CreateCollection(\"docs\") TO WS-A",
            "MOVE KB-1::ListCollections() TO WS-N",
            "MOVE KB-1::GetCollection(1) TO WS-C",
            "STRING \"create \" WS-A \" count \" WS-N \" first \" WS-C DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-COLL::Caption",
        ]),
    );
    bind(
        &mut form,
        "BTN-ADD",
        "onClick",
        "       01 WS-A                    PIC X.\n       01 WS-B                    PIC X.\n       01 WS-BUSY                 PIC X(5).\n       01 WS-T                    PIC X(40).",
        &lines(&[
            "MOVE KB-1::AddDocument(\"alpha.md\" \"COBOL forms run in a WebAssembly module on the desktop.\") TO WS-A",
            "MOVE KB-1::AddDocument(\"beta.md\" \"refused while alpha is indexed\") TO WS-B",
            "MOVE KB-1::Busy TO WS-BUSY",
            "STRING \"add \" WS-A \" second \" WS-B \" busy \" WS-BUSY DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-ADD::Caption",
            "MOVE KB-1::LastError TO LBL-BUSYERR::Caption",
        ]),
    );
    bind(
        &mut form,
        "KB-1",
        "onProgress",
        "       01 WS-D                    PIC X(20).\n       01 WS-C                    PIC X(6).\n       01 WS-N                    PIC X(6).\n       01 WS-T                    PIC X(60).",
        &lines(&[
            "MOVE SPACES TO WS-T",
            "MOVE KB-1::ProgressDocument TO WS-D",
            "MOVE KB-1::ProgressCurrent TO WS-C",
            "MOVE KB-1::ProgressTotal TO WS-N",
            "STRING WS-D DELIMITED BY SPACE \" \" DELIMITED BY SIZE WS-C DELIMITED BY SPACE \"/\" DELIMITED BY SIZE WS-N DELIMITED BY SPACE INTO WS-T",
            "MOVE WS-T TO LBL-PROG::Caption",
        ]),
    );
    bind(
        &mut form,
        "KB-1",
        "onIndexed",
        "       01 WS-AD                   PIC X(6).\n       01 WS-UP                   PIC X(6).\n       01 WS-RM                   PIC X(6).\n       01 WS-PS                   PIC X(6).\n       01 WS-BUSY                 PIC X(5).\n       01 WS-MODE                 PIC X(20).\n       01 WS-T                    PIC X(80).",
        &lines(&[
            "ADD 1 TO WS-IDX",
            "MOVE SPACES TO WS-T",
            "MOVE KB-1::AddedCount TO WS-AD",
            "MOVE KB-1::UpdatedCount TO WS-UP",
            "MOVE KB-1::RemovedCount TO WS-RM",
            "MOVE KB-1::PassageCount TO WS-PS",
            "MOVE KB-1::Busy TO WS-BUSY",
            "MOVE KB-1::SearchMode TO WS-MODE",
            "STRING \"indexed \" WS-IDX \": \" DELIMITED BY SIZE WS-AD DELIMITED BY SPACE \",\" DELIMITED BY SIZE WS-UP DELIMITED BY SPACE",
            "    \",\" DELIMITED BY SIZE WS-RM DELIMITED BY SPACE \" passages \" DELIMITED BY SIZE WS-PS DELIMITED BY SPACE",
            "    \" busy \" WS-BUSY \" mode \" DELIMITED BY SIZE WS-MODE DELIMITED BY SPACE INTO WS-T",
            "MOVE WS-T TO LBL-IDX::Caption",
        ]),
    );
    bind(&mut form, "BTN-ADD2", "onClick", "       01 WS-A                    PIC X.", &lines(&["MOVE KB-1::UpdateDocument(\"beta.md\" \"Rust programs are compiled ahead of time.\") TO WS-A"]));
    bind(
        &mut form,
        "BTN-IMPORT",
        "onClick",
        "       01 WS-A                    PIC X.",
        &lines(&[&format!("MOVE KB-1::ImportDocument(\"{}\") TO WS-A", import.display())]),
    );
    bind(
        &mut form,
        "BTN-LIST",
        "onClick",
        "       01 WS-N                    PIC 9(2).\n       01 WS-DC                   PIC X(4).\n       01 WS-D1                   PIC X(12).\n       01 WS-D2                   PIC X(12).\n       01 WS-D3                   PIC X(12).\n       01 WS-D4                   PIC X(12).\n       01 WS-T                    PIC X(80).",
        &lines(&[
            "MOVE SPACES TO WS-T",
            "MOVE KB-1::ListDocuments() TO WS-N",
            "MOVE KB-1::DocumentCount TO WS-DC",
            "MOVE KB-1::GetDocument(1) TO WS-D1",
            "MOVE KB-1::GetDocument(2) TO WS-D2",
            "MOVE KB-1::GetDocument(3) TO WS-D3",
            "MOVE KB-1::GetDocument(4) TO WS-D4",
            "STRING WS-N \" \" DELIMITED BY SIZE WS-DC DELIMITED BY SPACE \" \" DELIMITED BY SIZE WS-D1 DELIMITED BY SPACE",
            "    \" \" DELIMITED BY SIZE WS-D2 DELIMITED BY SPACE \" \" DELIMITED BY SIZE WS-D3 DELIMITED BY SPACE",
            "    \" [\" DELIMITED BY SIZE WS-D4 DELIMITED BY SPACE \"]\" DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-DOCS::Caption",
        ]),
    );
    bind(&mut form, "BTN-SEARCH", "onClick", "       01 WS-A                    PIC X.", &lines(&["MOVE KB-1::Search(\"COBOL WebAssembly module\" 2) TO WS-A"]));
    bind(
        &mut form,
        "KB-1",
        "onSearchComplete",
        "       01 WS-RC                   PIC X(4).\n       01 WS-MODE                 PIC X(20).\n       01 WS-D1                   PIC X(12).\n       01 WS-H1                   PIC X(20).\n       01 WS-S1                   PIC X(10).\n       01 WS-D3                   PIC X(12).\n       01 WS-T                    PIC X(80).",
        &lines(&[
            "MOVE SPACES TO WS-T",
            "MOVE KB-1::ResultCount TO WS-RC",
            "MOVE KB-1::SearchMode TO WS-MODE",
            "MOVE KB-1::GetResultDocument(1) TO WS-D1",
            "MOVE KB-1::GetResultHeading(1) TO WS-H1",
            "MOVE KB-1::GetResultScore(1) TO WS-S1",
            "MOVE KB-1::GetResultDocument(3) TO WS-D3",
            "STRING \"found \" DELIMITED BY SIZE WS-RC DELIMITED BY SPACE \" \" DELIMITED BY SIZE WS-MODE DELIMITED BY SPACE",
            "    \" \" DELIMITED BY SIZE WS-D1 DELIMITED BY SPACE \" \" DELIMITED BY SIZE WS-H1 DELIMITED BY SPACE",
            "    \" \" DELIMITED BY SIZE WS-S1 DELIMITED BY SPACE \" [\" DELIMITED BY SIZE WS-D3 DELIMITED BY SPACE \"]\" DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-SRCH::Caption",
            "MOVE KB-1::GetResultPassage(1) TO LBL-PASS::Caption",
        ]),
    );
    bind(&mut form, "BTN-DEL", "onClick", "       01 WS-A                    PIC X.", &lines(&["MOVE KB-1::DeleteDocument(\"beta.md\") TO WS-A"]));
    bind(&mut form, "BTN-ERR", "onClick", "       01 WS-A                    PIC X.", &lines(&["MOVE KB-2::AddDocument(\"x.md\" \"x\") TO WS-A"]));
    bind(
        &mut form,
        "KB-2",
        "onError",
        "       01 WS-BUSY                 PIC X(5).\n       01 WS-E                    PIC X(80).\n       01 WS-T                    PIC X(90).",
        &lines(&[
            "MOVE KB-2::Busy TO WS-BUSY",
            "MOVE KB-2::LastError TO WS-E",
            "STRING \"busy \" WS-BUSY \" \" WS-E DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-KB2::Caption",
        ]),
    );
    bind(
        &mut form,
        "BTN-CANCEL",
        "onClick",
        "       01 WS-A                    PIC X.\n       01 WS-C                    PIC X.\n       01 WS-BUSY                 PIC X(5).\n       01 WS-T                    PIC X(40).",
        &lines(&[
            "MOVE KB-1::Refresh() TO WS-A",
            "MOVE KB-1::Cancel() TO WS-C",
            "MOVE KB-1::Busy TO WS-BUSY",
            "STRING \"refresh \" WS-A \" cancel \" WS-C \" busy \" WS-BUSY DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-CANCEL::Caption",
        ]),
    );
    bind(&mut form, "KB-1", "onCancelled", "", &lines(&["MOVE \"cancelled\" TO LBL-CANCELLED::Caption"]));

    let cfrm = root.join("forms/parity-kb.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/parity-kb.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    let manifest = root.join("paritykb.project.toml");
    fs::write(
        &manifest,
        "[project]\nname = \"paritykb\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/parity-kb.cfrm\"]\ngenerated = [\"generated/parity-kb.cbl\"]\n",
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
    let names = vec!["k1-collection", "k2-added", "k3-imported", "k4-searched", "k5-deleted", "k6-error-cancel"];
    let steps = vec![
        wait(200),
        click("BTN-COLL"),
        dump(),
        shot(names[0]),
        click("BTN-ADD"),
        wait(2000),
        dump(),
        shot(names[1]),
        click("BTN-ADD2"),
        wait(2000),
        click("BTN-IMPORT"),
        wait(2000),
        click("BTN-LIST"),
        dump(),
        shot(names[2]),
        click("BTN-SEARCH"),
        wait(2000),
        dump(),
        shot(names[3]),
        click("BTN-DEL"),
        wait(2000),
        click("BTN-LIST"),
        dump(),
        shot(names[4]),
        click("BTN-ERR"),
        wait(1500),
        click("BTN-CANCEL"),
        wait(2000),
        dump(),
        shot(names[5]),
    ];
    (Value::Array(steps), names)
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

/// What both engines did with the same script.
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

/// Run `script` under `rcrun run-form` (the interpreter), then build the project on the
/// WebAssembly path and run it in the built program, each engine from an empty `kb` folder.
fn run_twin(root: &Path, manifest: &Path, script: &Path, kb: &Path) -> Twin {
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
    let _ = fs::remove_dir_all(kb);
    fs::create_dir_all(kb).unwrap();
    let interp_shots = root.join("shots-interpreter");
    fs::create_dir_all(&interp_shots).unwrap();
    let t = Instant::now();
    let out = Command::new(rcrun())
        .args(["run-form", "forms/parity-kb.cfrm", "generated/parity-kb.cbl", "--designer", "--headless"])
        .arg(script)
        .args(["--headless-limit", "60"])
        .current_dir(root)
        .env("HOME", &home)
        .env("PRC_HEADLESS_SHOTS", &interp_shots)
        .output()
        .expect("run rcrun run-form");
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form failed:\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
    let interp = dumps(&report(&said));
    let interp_ms = t.elapsed().as_secs_f64() * 1000.0;

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
    let exe = root.join("bin").join(format!("paritykb{}", std::env::consts::EXE_SUFFIX));
    let _ = fs::remove_dir_all(kb);
    fs::create_dir_all(kb).unwrap();
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
    let wasm = dumps(&report(&said));
    let wasm_ms = t.elapsed().as_secs_f64() * 1000.0;
    Twin { interp, wasm, interp_shots, runner_shots, runner_ms, build_ms, interp_ms, wasm_ms }
}

#[test]
fn the_knowledgebase_control_agrees_on_both_engines() {
    let root = std::env::temp_dir().join(format!("prc-t47b-form-{}", std::process::id()));
    let (kb, import) = (root.join("kb"), root.join("gamma.txt"));
    let manifest = project(&root, &kb, &import);
    let (steps, shots) = script();
    let script = root.join("script.json");
    fs::write(&script, steps.to_string()).unwrap();
    let twin = run_twin(&root, &manifest, &script, &kb);

    // ── The two engines against each other: dumps and pictures ──
    let mut checks: Vec<(String, bool)> = Vec::new();
    checks.push((format!("both engines dump {} times", shots.len()), twin.interp.len() == shots.len() && twin.wasm.len() == shots.len()));
    let mut dump_lines = Vec::new();
    for (i, (x, y)) in twin.interp.iter().zip(twin.wasm.iter()).enumerate() {
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
    let at = |i: usize, id: &str| twin.wasm.get(i).map(|d| caption(d, id)).unwrap_or_default();
    let shown: Vec<String> = (0..shots.len())
        .flat_map(|i| {
            ["LBL-COLL", "LBL-ADD", "LBL-BUSYERR", "LBL-PROG", "LBL-IDX", "LBL-DOCS", "LBL-SRCH", "LBL-PASS", "LBL-KB2", "LBL-CANCEL", "LBL-CANCELLED"]
                .iter()
                .map(move |id| (i, *id))
        })
        .map(|(i, id)| format!("    dump {i} {id}: {}", at(i, id).trim()))
        .collect();
    checks.extend([
        ("CreateCollection, ListCollections and GetCollection answer at once".to_string(), at(0, "LBL-COLL").trim() == "create 1 count 01 first docs"),
        ("AddDocument starts (1) and busies the control; a second operation is refused (0)".into(), at(1, "LBL-ADD").trim() == "add 1 second 0 busy true"),
        ("… and LastError says why".into(), at(1, "LBL-BUSYERR").trim() == "another Knowledge Base operation is still running on this control"),
        ("onIndexed reads its own values: 1 added, no longer busy".into(), at(1, "LBL-IDX").trim().starts_with("indexed 01: 1,0,0 passages ") && at(1, "LBL-IDX").contains(" busy false ")),
        ("onProgress reads its own values".into(), at(1, "LBL-PROG").trim().starts_with("alpha.md ")),
        ("UpdateDocument and ImportDocument each raise onIndexed".into(), at(2, "LBL-IDX").trim().starts_with("indexed 03: 1,0,0")),
        ("ListDocuments and GetDocument: three documents, nothing past the last".into(), at(2, "LBL-DOCS").trim() == "03 3 alpha.md beta.md gamma.txt []"),
        ("Search: onSearchComplete reads the results, the best one first".into(), at(3, "LBL-SRCH").trim().starts_with("found 2 Lexical alpha.md alpha ") && at(3, "LBL-SRCH").trim().ends_with("[]")),
        ("GetResultPassage".into(), at(3, "LBL-PASS").contains("WebAssembly module")),
        ("DeleteDocument removes one".into(), at(4, "LBL-IDX").trim().starts_with("indexed 04: 0,0,1") && at(4, "LBL-DOCS").trim() == "02 2 alpha.md gamma.txt  []"),
        ("no collection: onError, LastError says so, no longer busy".into(), at(5, "LBL-KB2").trim().starts_with("busy false ") && at(5, "LBL-KB2").contains("no collection is set")),
        ("Refresh then Cancel(): onCancelled, no longer busy".into(), at(5, "LBL-CANCEL").trim() == "refresh 1 cancel 1 busy false" && at(5, "LBL-CANCELLED").trim() == "cancelled"),
        ("the cancelled refresh's reports are dropped".into(), at(5, "LBL-IDX").trim().starts_with("indexed 04:")),
    ]);

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== the_knowledgebase_control_agrees_on_both_engines (DESKTOP-WASM-001 T47b: R10, R11, ABI A49) ==");
    println!("form {FORM}: 2 KnowledgeBase controls (Lexical, a collection of their own; one with no collection), 9 buttons, 14 event handlers");
    println!("  CreateCollection, ListCollections, GetCollection; AddDocument and a second refused, onProgress, onIndexed; UpdateDocument,");
    println!("  ImportDocument, ListDocuments, GetDocument; Search with a maximum, onSearchComplete and the result accessors; DeleteDocument;");
    println!("  onError with no collection; Refresh then Cancel(), onCancelled");
    println!("each engine from an empty folder; real time; {} dumps, {} pictures", shots.len(), shots.len());
    println!("runner built in {:.0} ms; rcrun build (WebAssembly path) {:.0} ms", twin.runner_ms, twin.build_ms);
    println!("interpreter run {:.0} ms; runner run {:.0} ms", twin.interp_ms, twin.wasm_ms);
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
