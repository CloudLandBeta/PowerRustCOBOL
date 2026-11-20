// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T48 (R10, R11, ABI A54): **`COBOL::"MCP-SEARCH"` behaves on the WebAssembly path as
//! it does under the interpreter.**
//!
//! One form with an INDEXED file of orders (its `FD` in the form, its data built here, its `.cidx` under
//! `indexed/`) and two AgentObjects that never ask a model: they only decide which files the program's
//! searches reach. The form's handlers call the search:
//! - before any file is allowed, which names the tool it does not serve;
//! - after `AllowFile`, with a filter written in lower case, with `limit`, with arguments that are not
//!   JSON (read as no filters), with a filter nothing matches, and into an item too short for the result;
//! - on a file the other AgentObject registered under another name, which the same program reaches;
//! - after `DenyFile`, where the denied file's tool is gone and the registered one is still served.
//!
//! The script runs headless under `rcrun run-form` (the interpreter) and in the program `rcrun build`
//! makes on the WebAssembly path. The dumps must be equal and the pictures equal in every pixel. No
//! request leaves the machine: the AgentObjects name a local port nothing serves, and nothing asks them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use cobolt_runtime::indexed::{status, KeySpec, OpenMode};
use serde_json::{json, Value};

const FORM: &str = "PARITY-MCPSEARCH";

const LABELS: [&str; 6] = ["LBL-NONE", "LBL-ACME", "LBL-LIMIT", "LBL-REG", "LBL-DENY", "LBL-TEXT"];

/// The orders' record: `ORD-ID` 9(6), `ORD-CUSTOMER` X(30).
const RECORD_LEN: usize = 36;

const ORDERS_CIDX: &str = r#"<?xml version="1.0" encoding="UTF-8"?><IndexedFile name="ORDERS" finalized="true" version="1.0"><assign-path>data/orders.dat</assign-path><access-mode>dynamic</access-mode><record-format fixed-length="36"/><storage mode="disk" compression="false" persistence="false"/><comment><![CDATA[One row per customer order]]></comment><keys><primary duplicates="false" ordering="ascending"><part field="ORD-ID" offset="0" length="6" encoding="bytes"/></primary></keys><fields><Field level="1" name="ORD-REC" usage="display"><Field level="5" name="ORD-ID" pic="9(6)" usage="display" offset="0" length="6"><comment><![CDATA[The order's number]]></comment></Field><Field level="5" name="ORD-CUSTOMER" pic="X(30)" usage="display" offset="6" length="30"><comment><![CDATA[Who placed the order]]></comment></Field></Field></fields></IndexedFile>"#;

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

// ── The fixture ──────────────────────────────────────────────────────────────

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

/// WORKING-STORAGE items, one per line.
fn items(items: &[&str]) -> String {
    items.iter().map(|s| format!("       {s}")).collect::<Vec<_>>().join("\n")
}

/// The orders: three records, two of them ACME's.
fn build_orders(path: &Path) {
    let mut f = cobolt_runtime::indexed_disk::DiskIndexedFile::new(path, RECORD_LEN, KeySpec { parts: Vec::new(), offset: 0, len: 6, duplicates: false }, Vec::new());
    assert_eq!(f.open(OpenMode::Output), status::OK);
    for (id, customer) in [("000001", "ACME"), ("000002", "GLOBEX"), ("000003", "ACME")] {
        let rec = format!("{id}{customer:<30}");
        assert_eq!(f.write(rec.as_bytes()), status::OK);
    }
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
    build_orders(&root.join("data/orders.dat"));
    fs::write(root.join("indexed/orders.cidx"), ORDERS_CIDX).unwrap();
    let s = |v: &str| PropValue::String(v.into());
    let mut form = Form::new(FORM, "The MCP search", 760, 300);
    form.main_form = true;
    form.cobol_structure.file_control =
        "           SELECT OPTIONAL ORDERS ASSIGN TO \"data/orders.dat\"\n               ORGANIZATION IS INDEXED\n               ACCESS MODE IS DYNAMIC\n               RECORD KEY IS ORD-ID.".into();
    form.cobol_structure.file_section = "       FD ORDERS IS GLOBAL.\n       01 ORD-REC.\n           05 ORD-ID       PIC 9(6).\n           05 ORD-CUSTOMER PIC X(30).".into();
    let buttons = ["BTN-NONE", "BTN-ALLOW", "BTN-LIMIT", "BTN-REG", "BTN-DENY"];
    for (i, id) in buttons.iter().enumerate() {
        control(&mut form, id, ControlType::Button, (20, 20 + 38 * i as i32, 120, 28), &[("Caption", s(id.trim_start_matches("BTN-")))]);
    }
    for (i, id) in LABELS.iter().enumerate() {
        control(&mut form, id, ControlType::Label, (160, 20 + 38 * i as i32, 580, 28), &[("Caption", s("-"))]);
    }
    for (i, id) in ["AGT-1", "AGT-2"].iter().enumerate() {
        // Nothing asks them: they name a local port nothing serves, so no request could leave the machine.
        let props = vec![
            ("AgentAPI", s("OpenAI")),
            ("AgentURL", s("http://127.0.0.1:9/v1/chat/completions")),
            ("AgentModel", s("stub-model")),
            ("TimeoutSeconds", s("2")),
        ];
        control(&mut form, id, ControlType::AgentObject, (20 + 60 * i as i32, 240, 56, 56), &props);
    }

    // Before any file is allowed: the tool is not served.
    bind(&mut form, "BTN-NONE", "onClick", &items(&["01 WS-R                    PIC X(70)."]), &lines(&[
        "COBOL::\"MCP-SEARCH\" ( \"search_orders\" \"{}\" WS-R )",
        "MOVE WS-R TO LBL-NONE::Caption",
    ]));
    // AllowFile, then a search whose filter is written in lower case.
    bind(
        &mut form,
        "BTN-ALLOW",
        "onClick",
        &items(&[
            "01 WS-A                    PIC X(4).",
            "01 WS-R                    PIC X(400).",
            "01 WS-N1                   PIC 9(2) VALUE 0.",
            "01 WS-N2                   PIC 9(2) VALUE 0.",
            "01 WS-L                    PIC X(80).",
            "01 WS-T                    PIC X(401).",
        ]),
        &lines(&[
            "MOVE AGT-1::AllowFile(\"ORDERS\" \"indexed/orders.cidx\") TO WS-A",
            "COBOL::\"MCP-SEARCH\" ( \"search_orders\" '{\"ORD-CUSTOMER\":\"acme\"}' WS-R )",
            "MOVE 0 TO WS-N1 WS-N2",
            "INSPECT WS-R TALLYING WS-N1 FOR ALL \"ACME\"",
            "INSPECT WS-R TALLYING WS-N2 FOR ALL \"GLOBEX\"",
            "STRING \"allow \" WS-A \" acme \" WS-N1 \" globex \" WS-N2 DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-ACME::Caption",
            // The whole result, closed by a bar: the result ends in a newline, which the two engines
            // trim differently when it ends a property's value (a fix queued apart from this work).
            "STRING WS-R \"|\" DELIMITED BY SIZE INTO WS-T",
            "MOVE WS-T TO LBL-TEXT::Caption",
        ]),
    );
    // A limit, arguments that are not JSON, a filter nothing matches, and a result too long for its item.
    bind(
        &mut form,
        "BTN-LIMIT",
        "onClick",
        &items(&[
            "01 WS-R                    PIC X(400).",
            "01 WS-R2                   PIC X(400).",
            "01 WS-R3                   PIC X(40).",
            "01 WS-SMALL                PIC X(20).",
            "01 WS-N1                   PIC 9(2) VALUE 0.",
            "01 WS-N2                   PIC 9(2) VALUE 0.",
            "01 WS-N3                   PIC 9(2) VALUE 0.",
            "01 WS-N4                   PIC 9(2) VALUE 0.",
            "01 WS-L                    PIC X(160).",
        ]),
        &lines(&[
            "COBOL::\"MCP-SEARCH\" ( \"search_orders\" '{\"limit\":1}' WS-R )",
            "COBOL::\"MCP-SEARCH\" ( \"search_orders\" \"{oops\" WS-R2 )",
            "COBOL::\"MCP-SEARCH\" ( \"search_orders\" '{\"ORD-CUSTOMER\":\"NOBODY\"}' WS-R3 )",
            "COBOL::\"MCP-SEARCH\" ( \"search_orders\" \"{}\" WS-SMALL )",
            "MOVE 0 TO WS-N1 WS-N2 WS-N3 WS-N4",
            "INSPECT WS-R TALLYING WS-N1 FOR ALL \"ACME\"",
            "INSPECT WS-R TALLYING WS-N2 FOR ALL \"truncated at 1\"",
            "INSPECT WS-R2 TALLYING WS-N3 FOR ALL \"ACME\"",
            "INSPECT WS-R2 TALLYING WS-N4 FOR ALL \"GLOBEX\"",
            "STRING \"limit \" WS-N1 \" cut \" WS-N2 \" bad \" WS-N3 \" \" WS-N4 \"|\" WS-R3 \"|\" WS-SMALL \"|\" DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-LIMIT::Caption",
        ]),
    );
    // A file the other AgentObject registers under another name: the same program's search reaches it.
    bind(
        &mut form,
        "BTN-REG",
        "onClick",
        &items(&["01 WS-A                    PIC X(4).", "01 WS-R                    PIC X(400).", "01 WS-N1                   PIC 9(2) VALUE 0.", "01 WS-N2                   PIC 9(2) VALUE 0.", "01 WS-L                    PIC X(80)."]),
        &lines(&[
            "MOVE AGT-2::RegisterFile(\"data/orders.dat\" \"indexed/orders.cidx\" \"ARCHIVE\") TO WS-A",
            "COBOL::\"MCP-SEARCH\" ( \"search_archive\" '{\"ORD-CUSTOMER\":\"GLOBEX\"}' WS-R )",
            "MOVE 0 TO WS-N1 WS-N2",
            "INSPECT WS-R TALLYING WS-N1 FOR ALL \"ACME\"",
            "INSPECT WS-R TALLYING WS-N2 FOR ALL \"GLOBEX\"",
            "STRING \"reg \" WS-A \" acme \" WS-N1 \" globex \" WS-N2 DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-REG::Caption",
        ]),
    );
    // DenyFile: the denied file's tool is gone, the registered one is still served.
    bind(
        &mut form,
        "BTN-DENY",
        "onClick",
        &items(&["01 WS-R                    PIC X(70).", "01 WS-R2                   PIC X(400).", "01 WS-N1                   PIC 9(2) VALUE 0.", "01 WS-L                    PIC X(120)."]),
        &lines(&[
            "AGT-1::DenyFile(\"ORDERS\")",
            "COBOL::\"MCP-SEARCH\" ( \"search_orders\" \"{}\" WS-R )",
            "COBOL::\"MCP-SEARCH\" ( \"search_archive\" \"{}\" WS-R2 )",
            "MOVE 0 TO WS-N1",
            "INSPECT WS-R2 TALLYING WS-N1 FOR ALL \"ACME\"",
            "STRING WS-R \"|archive acme \" WS-N1 DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-DENY::Caption",
        ]),
    );

    let cfrm = root.join("forms/parity-mcpsearch.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/parity-mcpsearch.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(root.join("src/main.cbl"), "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n").unwrap();
    let manifest = root.join("paritymcpsearch.project.toml");
    fs::write(
        &manifest,
        "[project]\nname = \"paritymcpsearch\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/parity-mcpsearch.cfrm\"]\ngenerated = [\"generated/parity-mcpsearch.cbl\"]\n",
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
    let names = vec!["s1-allowed", "s2-limits-registered", "s3-denied"];
    let steps = vec![
        wait(200),
        click("BTN-NONE"),
        wait(200),
        click("BTN-ALLOW"),
        wait(300),
        dump(),
        shot(names[0]),
        click("BTN-LIMIT"),
        wait(300),
        click("BTN-REG"),
        wait(300),
        dump(),
        shot(names[1]),
        click("BTN-DENY"),
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

/// A Label's caption in a dump.
fn caption(dump: &Value, id: &str) -> String {
    let props = &dump[id]["props"];
    props.get("CAPTION").or_else(|| props.get("Caption")).and_then(Value::as_str).unwrap_or("").to_string()
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
/// and run it in the built program.
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
    let interp_shots = root.join("shots-interpreter");
    fs::create_dir_all(&interp_shots).unwrap();
    let t = Instant::now();
    let out = Command::new(rcrun())
        .args(["run-form", "forms/parity-mcpsearch.cfrm", "generated/parity-mcpsearch.cbl", "--designer", "--headless"])
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
    let exe = root.join("bin").join(format!("paritymcpsearch{}", std::env::consts::EXE_SUFFIX));
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
fn the_mcp_search_agrees_on_both_engines() {
    let root = std::env::temp_dir().join(format!("prc-t48-mcp-{}", std::process::id()));
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
    let at = |i: usize, id: &str| twin.wasm.dumps.get(i).map(|d| caption(d, id)).unwrap_or_default();
    let shown: Vec<String> = (0..shots.len()).flat_map(|i| LABELS.iter().map(move |id| (i, *id))).map(|(i, id)| format!("    dump {i} {id}: {:?}", at(i, id).trim_end())).collect();
    checks.extend([
        (
            "before any file is allowed, the search names the tool it does not serve".to_string(),
            at(0, "LBL-NONE").trim() == "no consultable file is served by the tool 'search_orders'",
        ),
        ("AllowFile answers 1; a filter in lower case finds ACME's 2 orders and no other".into(), at(0, "LBL-ACME").trim() == "allow 1    acme 02 globex 00"),
        ("the whole result: the file, its count and the two orders, one per line".into(), {
            let c = at(0, "LBL-TEXT");
            c.starts_with("ORDERS: 2 record(s)\nORD-ID=000001  ORD-CUSTOMER=ACME\nORD-ID=000003  ORD-CUSTOMER=ACME\n ") && c.ends_with(" |")
        }),
        (
            "limit 1: 1 order, marked truncated; arguments that are not JSON: all 3; no match: said so; a short item: the result's start".into(),
            at(1, "LBL-LIMIT").trim_end() == "limit 01 cut 01 bad 02 01|ORDERS: no records matched.             |ORDERS: 3 record(s)\n|",
        ),
        ("a file the other AgentObject registered: the same program's search finds GLOBEX's order".into(), at(1, "LBL-REG").trim() == "reg 1    acme 00 globex 01"),
        (
            "after DenyFile the file's tool is gone and the registered one still finds both ACME orders".into(),
            at(2, "LBL-DENY").trim_end() == "no consultable file is served by the tool 'search_orders'             |archive acme 02",
        ),
    ]);

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== the_mcp_search_agrees_on_both_engines (DESKTOP-WASM-001 T48: R10, R11, ABI A54) ==");
    println!("form {FORM}: an INDEXED file of 3 orders (its FD in the form, its .cidx under indexed/), 2 AgentObjects that ask no model, 5 buttons, 5 event handlers, 11 searches");
    println!("  the search before AllowFile; a lower-case filter; limit 1; arguments that are not JSON; no match; a result longer than its item;");
    println!("  a file the other AgentObject registered; DenyFile");
    println!("real time; {} dumps and {} pictures per engine; no request leaves the machine", shots.len(), shots.len());
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
