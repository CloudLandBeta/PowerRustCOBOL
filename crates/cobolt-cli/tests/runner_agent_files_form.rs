// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T47c (R10, R11, ABI A51): **an AgentObject's indexed-file tools behave on the
//! WebAssembly path as they do under the interpreter.**
//!
//! One form with an INDEXED file of orders (its `FD` in the form, its data built here, its `.cidx`
//! under `indexed/`) and two AgentObjects asking a local stub model server (a `TcpListener` on
//! `127.0.0.1`, no network), which answers by what each request asks:
//! - `AllowFile` with the file's definition, without it (found under `indexed/` by the file's name),
//!   and naming no file of the program;
//! - an `Ask` the model answers by calling the file's search tool: the search runs in the program, its
//!   result goes back to the model, and the answer arrives through `onResponse`;
//! - `RegisterFile` of the same data under another name, and of a file that does not exist, with what
//!   each writes (`RegisterResult`, `RegisteredName`, `RegisterMessage`); an `Ask` whose request
//!   offers both tools; `UnregisterFile` twice;
//! - `DenyFile`, after which a request offers no tool.
//!
//! The script runs headless under `rcrun run-form` (the interpreter) and in the program `rcrun build`
//! makes on the WebAssembly path. The dumps must be equal, the pictures equal in every pixel, and the
//! requests the stub received, the search's result among them, the same on both engines.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use cobolt_runtime::indexed::{status, KeySpec, OpenMode};
use serde_json::{json, Value};

const FORM: &str = "PARITY-AGENTFILES";

const LABELS: [&str; 6] = ["LBL-ALLOW", "LBL-ASK", "LBL-REG", "LBL-TOOLS", "LBL-UNREG", "LBL-DENY"];

/// The orders' record: `ORD-ID` 9(6), `ORD-CUSTOMER` X(30).
const RECORD_LEN: usize = 36;

const ORDERS_CIDX: &str = r#"<?xml version="1.0" encoding="UTF-8"?><IndexedFile name="ORDERS" finalized="true" version="1.0"><assign-path>data/orders.dat</assign-path><access-mode>dynamic</access-mode><record-format fixed-length="36"/><storage mode="disk" compression="false" persistence="false"/><comment><![CDATA[One row per customer order]]></comment><keys><primary duplicates="false" ordering="ascending"><part field="ORD-ID" offset="0" length="6" encoding="bytes"/></primary></keys><fields><Field level="1" name="ORD-REC" usage="display"><Field level="5" name="ORD-ID" pic="9(6)" usage="display" offset="0" length="6"><comment><![CDATA[The order's number]]></comment></Field><Field level="5" name="ORD-CUSTOMER" pic="X(30)" usage="display" offset="6" length="30"><comment><![CDATA[Who placed the order]]></comment></Field></Field></fields></IndexedFile>"#;

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

// ── The stub model server ────────────────────────────────────────────────────

fn read_request_body(stream: &mut TcpStream) -> String {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return String::new(),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_ascii_lowercase();
    let len = head.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0);
    while buf.len() < header_end + len {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    String::from_utf8_lossy(&buf[header_end..]).into_owned()
}

fn piece(text: &str) -> String {
    format!("data: {}", json!({"choices": [{"delta": {"content": text}}]}))
}

fn finish(reason: &str, prompt_tokens: u32, completion_tokens: u32) -> Vec<String> {
    vec![
        format!("data: {}", json!({"choices": [{"delta": {}, "finish_reason": reason}]})),
        format!("data: {}", json!({"choices": [], "usage": {"prompt_tokens": prompt_tokens, "completion_tokens": completion_tokens}})),
        "data: [DONE]".to_owned(),
    ]
}


/// The tools a request offers, by name, in order.
fn offered(req: &Value) -> Vec<String> {
    req["tools"].as_array().map(|t| t.iter().filter_map(|x| x["function"]["name"].as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

/// The lines to stream for a request, each after its pause in ms, chosen by what it asks.
fn answer_for(body: &str) -> Vec<(u64, String)> {
    let req: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let messages = req["messages"].as_array().cloned().unwrap_or_default();
    let last_user = messages.iter().rev().find(|m| m["role"] == "user").and_then(|m| m["content"].as_str()).unwrap_or("").to_owned();
    let quick = |lines: Vec<String>| lines.into_iter().map(|l| (20, l)).collect::<Vec<_>>();
    if last_user == "Which orders are ACME's?" {
        // The search's result comes back as a tool message: the answer says what it found.
        if let Some(found) = messages.iter().find(|m| m["role"] == "tool").and_then(|m| m["content"].as_str()) {
            let n = found.matches("ACME").count();
            let mut l = vec![piece(&format!("ACME appears {n} times."))];
            l.extend(finish("stop", 60, 6));
            return quick(l);
        }
        return vec![
            (20, r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_7","type":"function","function":{"name":"search_orders","arguments":""}}]}}]}"#.to_owned()),
            (20, r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"ORD-CUSTOMER\":\"ACME\"}"}}]}}]}"#.to_owned()),
            (20, r#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#.to_owned()),
            (20, r#"data: {"choices":[],"usage":{"prompt_tokens":30,"completion_tokens":9}}"#.to_owned()),
            (20, "data: [DONE]".to_owned()),
        ];
    }
    // Any other question: which tools the request offered.
    let tools = offered(&req);
    let said = if tools.is_empty() { "Tools: none.".to_owned() } else { format!("Tools: {}.", tools.join(", ")) };
    let mut l = vec![piece(&said)];
    l.extend(finish("stop", 10, 4));
    quick(l)
}

/// Serve every connection on a thread of its own, from [`answer_for`]; the request bodies received,
/// in order.
fn model_server() -> (u16, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let log = log.clone();
            thread::spawn(move || {
                let body = read_request_body(&mut stream);
                log.lock().unwrap().push(body.clone());
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n");
                let _ = stream.flush();
                for (pause, line) in answer_for(&body) {
                    thread::sleep(Duration::from_millis(pause));
                    if stream.write_all(format!("{line}\n\n").as_bytes()).is_err() {
                        return;
                    }
                    let _ = stream.flush();
                }
            });
        }
    });
    (port, seen)
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

/// The fixture project in `root`, its agents asking `url`; its manifest's path.
fn project(root: &Path, url: &str) -> PathBuf {
    let _ = fs::remove_dir_all(root);
    // `assets/` anchors the built program's relative paths at the project, as `rcrun run-form` anchors
    // them, so `data/` and `indexed/` are the same folders under both engines.
    for d in ["forms", "generated", "src", "data", "indexed", "assets"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    build_orders(&root.join("data/orders.dat"));
    fs::write(root.join("indexed/orders.cidx"), ORDERS_CIDX).unwrap();
    let s = |v: &str| PropValue::String(v.into());
    let mut form = Form::new(FORM, "AgentObject file tools", 760, 300);
    form.main_form = true;
    form.cobol_structure.file_control =
        "           SELECT OPTIONAL ORDERS ASSIGN TO \"data/orders.dat\"\n               ORGANIZATION IS INDEXED\n               ACCESS MODE IS DYNAMIC\n               RECORD KEY IS ORD-ID.".into();
    form.cobol_structure.file_section = "       FD ORDERS IS GLOBAL.\n       01 ORD-REC.\n           05 ORD-ID       PIC 9(6).\n           05 ORD-CUSTOMER PIC X(30).".into();
    let buttons = ["BTN-ALLOW", "BTN-ASK", "BTN-REG", "BTN-TOOLS", "BTN-UNREG", "BTN-DENY"];
    for (i, id) in buttons.iter().enumerate() {
        control(&mut form, id, ControlType::Button, (20, 20 + 38 * i as i32, 120, 28), &[("Caption", s(id.trim_start_matches("BTN-")))]);
    }
    for (i, id) in LABELS.iter().enumerate() {
        control(&mut form, id, ControlType::Label, (160, 20 + 38 * i as i32, 580, 28), &[("Caption", s("-"))]);
    }
    for (i, id) in ["AGT-1", "AGT-2"].iter().enumerate() {
        let props = vec![
            ("AgentAPI", s("OpenAI")),
            ("AgentURL", s(url)),
            ("AgentModel", s("stub-model")),
            ("SystemPrompt", s("Be brief.")),
            ("Temperature", s("0")),
            ("MaximumTokens", s("100")),
            ("TimeoutSeconds", s("10")),
        ];
        control(&mut form, id, ControlType::AgentObject, (600 + 60 * i as i32, 240, 56, 56), &props);
    }

    // AllowFile: with the definition, without it, and naming no file of the program.
    bind(
        &mut form,
        "BTN-ALLOW",
        "onClick",
        "       01 WS-A                    PIC X(4).\n       01 WS-B                    PIC X(4).\n       01 WS-C                    PIC X(4).\n       01 WS-L                    PIC X(60).",
        &lines(&[
            "MOVE AGT-1::AllowFile(\"ORDERS\" \"indexed/orders.cidx\") TO WS-A",
            "MOVE AGT-1::AllowFile(\"ORDERS\") TO WS-B",
            "MOVE AGT-1::AllowFile(\"NO-SUCH-FILE\") TO WS-C",
            "STRING \"allow \" WS-A \" by name \" WS-B \" none \" WS-C DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-ALLOW::Caption",
        ]),
    );
    // An Ask the model answers by searching the file.
    bind(&mut form, "BTN-ASK", "onClick", "       01 WS-T                    PIC X(20).", &lines(&["MOVE AGT-1::Ask(\"Which orders are ACME's?\") TO WS-T"]));
    bind(
        &mut form,
        "AGT-1",
        "onResponse",
        "       01 WS-R                    PIC X(40).\n       01 WS-C                    PIC X(4).\n       01 WS-L                    PIC X(70).",
        &lines(&["MOVE AGT-1::LastReply TO WS-R", "MOVE AGT-1::LastToolCallCount TO WS-C", "STRING WS-R \"|calls \" WS-C DELIMITED BY SIZE INTO WS-L", "MOVE WS-L TO LBL-ASK::Caption"]),
    );
    bind(&mut form, "AGT-1", "onError", "       01 WS-E                    PIC X(80).", &lines(&["MOVE AGT-1::LastError TO WS-E", "MOVE WS-E TO LBL-ASK::Caption"]));
    // RegisterFile: the same data under another name, then a file that does not exist.
    bind(
        &mut form,
        "BTN-REG",
        "onClick",
        "       01 WS-A                    PIC X(4).\n       01 WS-R                    PIC X(10).\n       01 WS-N                    PIC X(12).\n       01 WS-M                    PIC X(60).\n       01 WS-B                    PIC X(4).\n       01 WS-R2                   PIC X(20).\n       01 WS-L                    PIC X(140).",
        &lines(&[
            "MOVE AGT-2::RegisterFile(\"data/orders.dat\" \"indexed/orders.cidx\" \"ARCHIVE\") TO WS-A",
            "MOVE AGT-2::RegisterResult TO WS-R",
            "MOVE AGT-2::RegisteredName TO WS-N",
            "MOVE AGT-2::RegisterMessage TO WS-M",
            "MOVE AGT-2::RegisterFile(\"data/missing.dat\" \"indexed/orders.cidx\" \"GONE\") TO WS-B",
            "MOVE AGT-2::RegisterResult TO WS-R2",
            "STRING WS-A WS-R WS-N WS-M \"|\" WS-B WS-R2 DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-REG::Caption",
        ]),
    );
    // An Ask whose request offers both tools.
    bind(&mut form, "BTN-TOOLS", "onClick", "       01 WS-T                    PIC X(20).", &lines(&["MOVE AGT-2::Ask(\"Which tools?\") TO WS-T"]));
    bind(&mut form, "AGT-2", "onResponse", "       01 WS-R                    PIC X(60).", &lines(&["MOVE AGT-2::LastReply TO WS-R", "MOVE WS-R TO LBL-TOOLS::Caption"]));
    // UnregisterFile twice: the second finds nothing.
    bind(
        &mut form,
        "BTN-UNREG",
        "onClick",
        "       01 WS-A                    PIC X(4).\n       01 WS-B                    PIC X(4).\n       01 WS-L                    PIC X(40).",
        &lines(&[
            "MOVE AGT-2::UnregisterFile(\"ARCHIVE\") TO WS-A",
            "MOVE AGT-2::UnregisterFile(\"ARCHIVE\") TO WS-B",
            "STRING \"unregister \" WS-A \" again \" WS-B DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-UNREG::Caption",
        ]),
    );
    // DenyFile, then an Ask whose request offers no tool.
    bind(&mut form, "BTN-DENY", "onClick", "       01 WS-T                    PIC X(20).", &lines(&["AGT-1::DenyFile(\"ORDERS\")", "MOVE AGT-2::Ask(\"Any tools?\") TO WS-T"]));
    bind(&mut form, "AGT-2", "onError", "       01 WS-E                    PIC X(80).", &lines(&["MOVE AGT-2::LastError TO WS-E", "MOVE WS-E TO LBL-DENY::Caption"]));

    let cfrm = root.join("forms/parity-agentfiles.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/parity-agentfiles.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(root.join("src/main.cbl"), "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n").unwrap();
    let manifest = root.join("parityagentfiles.project.toml");
    fs::write(
        &manifest,
        "[project]\nname = \"parityagentfiles\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/parity-agentfiles.cfrm\"]\ngenerated = [\"generated/parity-agentfiles.cbl\"]\n",
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
    let names = vec!["f1-allowed-searched", "f2-registered", "f3-denied"];
    let steps = vec![
        wait(200),
        click("BTN-ALLOW"),
        wait(300),
        click("BTN-ASK"),
        wait(2000),
        dump(),
        shot(names[0]),
        click("BTN-REG"),
        wait(300),
        click("BTN-TOOLS"),
        wait(1500),
        dump(),
        shot(names[1]),
        click("BTN-UNREG"),
        wait(300),
        click("BTN-DENY"),
        wait(1500),
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

/// The lines `AGT-1`'s `Verbose` narrated, in order.
fn narration(stdout: &str) -> Vec<String> {
    stdout.lines().filter(|l| l.starts_with("[agent ")).map(str::to_owned).collect()
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

/// What one engine did: its dumps, its narrated lines and the requests the stub received, sorted.
struct Seen {
    dumps: Vec<Value>,
    narration: Vec<String>,
    requests: Vec<String>,
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

/// The requests received since `from`, sorted (two agents may ask at once).
fn requests_since(log: &Arc<Mutex<Vec<String>>>, from: usize) -> Vec<String> {
    let mut r: Vec<String> = log.lock().unwrap()[from..].to_vec();
    r.sort();
    r
}

/// Run `script` under `rcrun run-form` (the interpreter), then build the project on the
/// WebAssembly path and run it in the built program.
fn run_twin(root: &Path, manifest: &Path, script: &Path, log: &Arc<Mutex<Vec<String>>>) -> Twin {
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
    let from = log.lock().unwrap().len();
    let t = Instant::now();
    let out = Command::new(rcrun())
        .args(["run-form", "forms/parity-agentfiles.cfrm", "generated/parity-agentfiles.cbl", "--designer", "--headless"])
        .arg(script)
        .args(["--headless-limit", "60"])
        .current_dir(root)
        .env("HOME", &home)
        .env("PRC_HEADLESS_SHOTS", &interp_shots)
        .output()
        .expect("run rcrun run-form");
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form failed:\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    // The late answers' threads finish before the next engine starts.
    thread::sleep(Duration::from_millis(1500));
    let interp = Seen { dumps: dumps(&report(&said)), narration: narration(&said), requests: requests_since(log, from), ms };

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
    let exe = root.join("bin").join(format!("parityagentfiles{}", std::env::consts::EXE_SUFFIX));
    let runner_shots = root.join("shots-runner");
    fs::create_dir_all(&runner_shots).unwrap();
    let from = log.lock().unwrap().len();
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
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    let wasm = Seen { dumps: dumps(&report(&said)), narration: narration(&said), requests: requests_since(log, from), ms };
    Twin { interp, wasm, interp_shots, runner_shots, runner_ms, build_ms }
}

#[test]
fn the_agentobject_file_tools_agree_on_both_engines() {
    let root = std::env::temp_dir().join(format!("prc-t47c-files-{}", std::process::id()));
    let (port, log) = model_server();
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let manifest = project(&root, &url);
    let (steps, shots) = script();
    let script = root.join("script.json");
    fs::write(&script, steps.to_string()).unwrap();
    let twin = run_twin(&root, &manifest, &script, &log);

    // ── The two engines against each other: dumps, pictures, requests ──
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
    let (req_i, req_w) = (&twin.interp.requests, &twin.wasm.requests);
    checks.push((format!("the stub received the same {} requests from both engines", req_i.len()), req_i == req_w && req_i.len() == 4));

    // ── What the program saw (the runner's dumps; the interpreter's are equal above) ──
    let at = |i: usize, id: &str| twin.wasm.dumps.get(i).map(|d| caption(d, id)).unwrap_or_default();
    let tool_result = req_w
        .iter()
        .filter_map(|b| serde_json::from_str::<Value>(b).ok())
        .flat_map(|r| r["messages"].as_array().cloned().unwrap_or_default())
        .find(|m| m["role"] == "tool")
        .and_then(|m| m["content"].as_str().map(str::to_owned))
        .unwrap_or_default();
    let shown: Vec<String> = (0..shots.len()).flat_map(|i| LABELS.iter().map(move |id| (i, *id))).map(|(i, id)| format!("    dump {i} {id}: {}", at(i, id).trim())).collect();
    checks.extend([
        ("AllowFile: 1 with the definition, 1 found by the file's name, 0 for a name that is no file of the program".to_string(), {
            let c = at(0, "LBL-ALLOW");
            c.trim() == "allow 1    by name 1    none 0"
        }),
        ("the model's search runs in the program and finds ACME's two orders; the answer uses it".into(), {
            let c = at(0, "LBL-ASK");
            c.starts_with("ACME appears 2 times.") && c.contains("|calls 1") && tool_result.matches("ACME").count() == 2 && !tool_result.contains("GLOBEX")
        }),
        ("RegisterFile: 1, MEMORY, the name and the message; a missing file: 0 and a refusal code".into(), {
            let c = at(1, "LBL-REG");
            c.starts_with("1   MEMORY    ARCHIVE     ARCHIVE is held in memory (") && c.contains("|0   ") && !c.contains("|0   MEMORY")
        }),
        ("a request after RegisterFile offers both files' tools".into(), at(1, "LBL-TOOLS").trim() == "Tools: search_orders, search_archive."),
        ("UnregisterFile: 1, then 0".into(), at(2, "LBL-UNREG").trim() == "unregister 1    again 0"),
        ("after DenyFile a request offers no tool".into(), {
            let last: Value = req_w.iter().filter_map(|b| serde_json::from_str::<Value>(b).ok()).find(|r| {
                r["messages"].as_array().is_some_and(|m| m.iter().any(|x| x["content"] == "Any tools?"))
            }).unwrap_or(Value::Null);
            !last.is_null() && offered(&last).is_empty()
        }),
    ]);

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== the_agentobject_file_tools_agree_on_both_engines (DESKTOP-WASM-001 T47c: R10, R11, ABI A51) ==");
    println!("form {FORM}: an INDEXED file of 3 orders (its FD in the form, its .cidx under indexed/), 2 AgentObjects asking a stub on 127.0.0.1:{port}, 6 buttons, 9 event handlers");
    println!("  AllowFile with and without its definition and naming no file; a search the model asks for; RegisterFile of the data under another name and of a missing file;");
    println!("  an Ask offering both tools; UnregisterFile twice; DenyFile and an Ask offering none");
    println!("real time; {} dumps, {} pictures, {} requests per engine", shots.len(), shots.len(), req_w.len());
    println!("runner built in {:.0} ms; rcrun build (WebAssembly path) {:.0} ms", twin.runner_ms, twin.build_ms);
    println!("interpreter run {:.0} ms; runner run {:.0} ms", twin.interp.ms, twin.wasm.ms);
    for l in &shot_lines {
        println!("{l}");
    }
    println!("pixels compared: {pixels_total}, differing: {differing_total}");
    for l in &dump_lines {
        println!("{l}");
    }
    if req_i != req_w {
        println!("    requests: interpreter {}, runner {}", req_i.len(), req_w.len());
        for (k, (a, b)) in req_i.iter().zip(req_w.iter()).enumerate().filter(|(_, (a, b))| a != b) {
            println!("    request {k}: interpreter {a}\n               runner      {b}");
        }
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
        println!("    the search's result: {tool_result:?}");
        println!("kept for inspection: {}", root.display());
    }
    assert!(failed.is_empty(), "failed: {failed:?}");
}
