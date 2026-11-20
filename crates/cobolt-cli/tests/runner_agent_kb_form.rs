// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T47c (R10, R11, ABI A52): **an AgentObject's Knowledge Base tools behave on the
//! WebAssembly path as they do under the interpreter.**
//!
//! One form with two KnowledgeBases, each with a collection of one document, and two AgentObjects
//! asking a local stub server (a `TcpListener` on `127.0.0.1`, no network). The stub is the model, and
//! the second KnowledgeBase's embedding server:
//! - `AllowKnowledgeBase` naming the control's own collection, naming another, naming a control that
//!   is no KnowledgeBase, and naming a collection by a name no collection may have;
//! - an `Ask` the model answers by searching both collections, one round each: the lexical one is
//!   searched at once, the other on a thread of its own (its query goes to the embedding server), and
//!   both results go back to the model before it answers;
//! - an `Ask` of the other agent, whose request offers neither collection (a collection is the agent's
//!   it was handed to);
//! - `DenyKnowledgeBase` of one control, after which a request offers only the other's collection.
//!
//! The script runs headless under `rcrun run-form` (the interpreter) and in the program `rcrun build`
//! makes on the WebAssembly path, each engine with fresh collections. The dumps must be equal, the
//! pictures equal in every pixel, and the requests the stub received, the searches' results among
//! them, the same on both engines.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

const FORM: &str = "PARITY-AGENTKB";

const LABELS: [&str; 6] = ["LBL-INDEX", "LBL-READY", "LBL-ALLOW", "LBL-ASK", "LBL-TOOLS", "LBL-DENY"];

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

// ── The stub model and embedding server ──────────────────────────────────────

/// A request's path and body.
fn read_request(stream: &mut TcpStream) -> (String, String) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return (String::new(), String::new()),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_ascii_lowercase();
    let path = head.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("").to_owned();
    let len = head.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0);
    while buf.len() < header_end + len {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    (path, String::from_utf8_lossy(&buf[header_end..]).into_owned())
}

/// A text's vector from the stub embedding server: how often each of eight groups of letters occurs,
/// so the same text always has the same vector.
fn vector(text: &str) -> Vec<f32> {
    let mut v = vec![0f32; 8];
    for c in text.chars().filter(char::is_ascii_alphabetic) {
        v[((c.to_ascii_lowercase() as u8 - b'a') / 4).min(7) as usize] += 1.0;
    }
    v
}

/// The stub embedding server's answer to an OpenAI-style `POST /v1/embeddings`.
fn embeddings_for(body: &str) -> String {
    let req: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let inputs: Vec<String> = req["input"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default();
    let data: Vec<Value> = inputs.iter().enumerate().map(|(i, t)| json!({"index": i, "embedding": vector(t)})).collect();
    json!({"data": data}).to_string()
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

/// A streamed reply that calls the tool `name` with `query`.
fn tool_call(id: &str, name: &str, query: &str) -> Vec<(u64, String)> {
    let args = json!({"query": query}).to_string();
    vec![
        (20, format!("data: {}", json!({"choices": [{"delta": {"tool_calls": [{"index": 0, "id": id, "type": "function", "function": {"name": name, "arguments": ""}}]}}]}))),
        (20, format!("data: {}", json!({"choices": [{"delta": {"tool_calls": [{"index": 0, "function": {"arguments": args}}]}}]}))),
        (20, r#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#.to_owned()),
        (20, r#"data: {"choices":[],"usage":{"prompt_tokens":30,"completion_tokens":9}}"#.to_owned()),
        (20, "data: [DONE]".to_owned()),
    ]
}

/// The tools a request offers, by name, in order.
fn offered(req: &Value) -> Vec<String> {
    req["tools"].as_array().map(|t| t.iter().filter_map(|x| x["function"]["name"].as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

/// How many passages a search's result reports (`0` for "no passage …").
fn passages(result: &str) -> String {
    result.split(" passage(s)").next().filter(|n| n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty()).unwrap_or("0").to_owned()
}

/// The lines to stream for a chat request, each after its pause in ms, chosen by what it asks.
fn answer_for(body: &str) -> Vec<(u64, String)> {
    let req: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let messages = req["messages"].as_array().cloned().unwrap_or_default();
    let last_user = messages.iter().rev().find(|m| m["role"] == "user").and_then(|m| m["content"].as_str()).unwrap_or("").to_owned();
    let quick = |lines: Vec<String>| lines.into_iter().map(|l| (20, l)).collect::<Vec<_>>();
    if last_user == "What runs on the desktop?" {
        // One collection per round: the lexical one, then the one the embedding server searches.
        let results: Vec<String> = messages.iter().filter(|m| m["role"] == "tool").filter_map(|m| m["content"].as_str().map(str::to_owned)).collect();
        return match results.len() {
            0 => tool_call("call_1", "kb_kb_1_docs", "WebAssembly desktop"),
            1 => tool_call("call_2", "kb_kb_2_notes", "runner collections"),
            _ => {
                let mut l = vec![piece(&format!("docs {}, notes {}.", passages(&results[0]), passages(&results[1])))];
                l.extend(finish("stop", 80, 6));
                quick(l)
            }
        };
    }
    // Any other question: which tools the request offered.
    let tools = offered(&req);
    let said = if tools.is_empty() { "Tools: none.".to_owned() } else { format!("Tools: {}.", tools.join(", ")) };
    let mut l = vec![piece(&said)];
    l.extend(finish("stop", 10, 4));
    quick(l)
}

/// Serve every connection on a thread of its own: an embedding request from [`embeddings_for`], any
/// other from [`answer_for`]; the request bodies received, in order.
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
                let (path, body) = read_request(&mut stream);
                log.lock().unwrap().push(body.clone());
                if path.ends_with("/embeddings") {
                    let reply = embeddings_for(&body);
                    let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).as_bytes());
                    let _ = stream.flush();
                    return;
                }
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



/// The fixture project in `root`, its agents asking `url` and its second KnowledgeBase embedding with
/// the server at `embed`; its manifest's path.
fn project(root: &Path, url: &str, embed: &str) -> PathBuf {
    let _ = fs::remove_dir_all(root);
    for d in ["forms", "generated", "src", "assets"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    let s = |v: &str| PropValue::String(v.into());
    let mut form = Form::new(FORM, "AgentObject Knowledge Base tools", 760, 300);
    form.main_form = true;
    let buttons = ["BTN-INDEX", "BTN-ALLOW", "BTN-ASK", "BTN-TOOLS", "BTN-DENY"];
    for (i, id) in buttons.iter().enumerate() {
        control(&mut form, id, ControlType::Button, (20, 20 + 38 * i as i32, 120, 28), &[("Caption", s(id.trim_start_matches("BTN-")))]);
    }
    for (i, id) in LABELS.iter().enumerate() {
        control(&mut form, id, ControlType::Label, (160, 20 + 38 * i as i32, 580, 28), &[("Caption", s("-"))]);
    }
    let kb1 = root.join("kb1").display().to_string();
    let kb2 = root.join("kb2").display().to_string();
    control(
        &mut form,
        "KB-1",
        ControlType::KnowledgeBase,
        (480, 240, 56, 56),
        &[("Location", s(&kb1)), ("Collection", s("docs")), ("Embedder", s("Lexical")), ("MaximumResults", PropValue::Int(3))],
    );
    control(
        &mut form,
        "KB-2",
        ControlType::KnowledgeBase,
        (540, 240, 56, 56),
        &[
            ("Location", s(&kb2)),
            ("Collection", s("notes")),
            ("Embedder", s("Endpoint")),
            ("EmbeddingAPI", s("OpenAI")),
            ("EmbeddingURL", s(embed)),
            ("EmbeddingModel", s("stub-embed")),
        ],
    );
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

    // One document in each collection; each KnowledgeBase says when it is indexed.
    bind(
        &mut form,
        "BTN-INDEX",
        "onClick",
        "       01 WS-A                    PIC X(4).\n       01 WS-B                    PIC X(4).\n       01 WS-C                    PIC X(4).\n       01 WS-D                    PIC X(4).\n       01 WS-L                    PIC X(60).",
        &lines(&[
            "MOVE KB-1::CreateCollection(\"docs\") TO WS-A",
            "MOVE KB-1::AddDocument(\"alpha.md\" \"COBOL forms run in a WebAssembly module on the desktop.\") TO WS-B",
            "MOVE KB-2::CreateCollection(\"notes\") TO WS-C",
            "MOVE KB-2::AddDocument(\"beta.md\" \"The runner searches collections for the model.\") TO WS-D",
            "STRING \"index \" WS-A WS-B WS-C WS-D DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-INDEX::Caption",
        ]),
    );
    bind(&mut form, "KB-1", "onIndexed", "", &lines(&["MOVE \"docs indexed\" TO LBL-READY::Caption"]));
    bind(&mut form, "KB-2", "onIndexed", "", &lines(&["MOVE \"notes indexed\" TO LBL-DENY::Caption"]));
    bind(&mut form, "KB-2", "onError", "       01 WS-E                    PIC X(80).", &lines(&["MOVE KB-2::LastError TO WS-E", "MOVE WS-E TO LBL-DENY::Caption"]));
    // AllowKnowledgeBase: the control's own collection, another, a control that is no KnowledgeBase,
    // and a name no collection may have.
    bind(
        &mut form,
        "BTN-ALLOW",
        "onClick",
        "       01 WS-A                    PIC X(4).\n       01 WS-B                    PIC X(4).\n       01 WS-C                    PIC X(4).\n       01 WS-D                    PIC X(4).\n       01 WS-L                    PIC X(60).",
        &lines(&[
            "MOVE AGT-1::AllowKnowledgeBase(\"KB-1\") TO WS-A",
            "MOVE AGT-1::AllowKnowledgeBase(\"KB-2\" \"notes\") TO WS-B",
            "MOVE AGT-1::AllowKnowledgeBase(\"LBL-ALLOW\") TO WS-C",
            "MOVE AGT-1::AllowKnowledgeBase(\"KB-1\" \"../x\") TO WS-D",
            "STRING \"allow \" WS-A \" other \" WS-B \" label \" WS-C \" bad \" WS-D DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-ALLOW::Caption",
        ]),
    );
    // An Ask the model answers by searching both collections.
    bind(&mut form, "BTN-ASK", "onClick", "       01 WS-T                    PIC X(20).", &lines(&["MOVE AGT-1::Ask(\"What runs on the desktop?\") TO WS-T"]));
    bind(
        &mut form,
        "AGT-1",
        "onResponse",
        "       01 WS-R                    PIC X(40).\n       01 WS-C                    PIC X(4).\n       01 WS-L                    PIC X(70).",
        &lines(&["MOVE AGT-1::LastReply TO WS-R", "MOVE AGT-1::LastToolCallCount TO WS-C", "STRING WS-R \"|calls \" WS-C DELIMITED BY SIZE INTO WS-L", "MOVE WS-L TO LBL-ASK::Caption"]),
    );
    bind(&mut form, "AGT-1", "onError", "       01 WS-E                    PIC X(80).", &lines(&["MOVE AGT-1::LastError TO WS-E", "MOVE WS-E TO LBL-ASK::Caption"]));
    // The other agent was handed nothing.
    bind(&mut form, "BTN-TOOLS", "onClick", "       01 WS-T                    PIC X(20).", &lines(&["MOVE AGT-2::Ask(\"Which tools?\") TO WS-T"]));
    bind(&mut form, "AGT-2", "onResponse", "       01 WS-R                    PIC X(60).", &lines(&["MOVE AGT-2::LastReply TO WS-R", "MOVE WS-R TO LBL-TOOLS::Caption"]));
    bind(&mut form, "AGT-2", "onError", "       01 WS-E                    PIC X(80).", &lines(&["MOVE AGT-2::LastError TO WS-E", "MOVE WS-E TO LBL-TOOLS::Caption"]));
    // DenyKnowledgeBase of KB-2, then an Ask whose request offers KB-1's collection only.
    bind(
        &mut form,
        "BTN-DENY",
        "onClick",
        "       01 WS-T                    PIC X(20).",
        &lines(&["AGT-1::DenyKnowledgeBase(\"KB-2\")", "MOVE \"-\" TO LBL-ASK::Caption", "MOVE AGT-1::Ask(\"Any tools?\") TO WS-T"]),
    );

    let cfrm = root.join("forms/parity-agentkb.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/parity-agentkb.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(root.join("src/main.cbl"), "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n").unwrap();
    let manifest = root.join("parityagentkb.project.toml");
    fs::write(
        &manifest,
        "[project]\nname = \"parityagentkb\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/parity-agentkb.cfrm\"]\ngenerated = [\"generated/parity-agentkb.cbl\"]\n",
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
    let names = vec!["k1-allowed-searched", "k2-other-agent", "k3-denied"];
    let steps = vec![
        wait(200),
        click("BTN-INDEX"),
        wait(3000),
        click("BTN-ALLOW"),
        wait(300),
        click("BTN-ASK"),
        wait(3000),
        dump(),
        shot(names[0]),
        click("BTN-TOOLS"),
        wait(1500),
        dump(),
        shot(names[1]),
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

/// Both KnowledgeBases' folders, empty: each engine indexes its own collections.
fn fresh_collections(root: &Path) {
    for d in ["kb1", "kb2"] {
        let _ = fs::remove_dir_all(root.join(d));
        fs::create_dir_all(root.join(d)).unwrap();
    }
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

    // ── 1. The interpreter: rcrun run-form, with fresh collections ──
    fresh_collections(root);
    let interp_shots = root.join("shots-interpreter");
    fs::create_dir_all(&interp_shots).unwrap();
    let from = log.lock().unwrap().len();
    let t = Instant::now();
    let out = Command::new(rcrun())
        .args(["run-form", "forms/parity-agentkb.cfrm", "generated/parity-agentkb.cbl", "--designer", "--headless"])
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
    let interp = Seen { dumps: dumps(&report(&said)), requests: requests_since(log, from), ms };

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
    let exe = root.join("bin").join(format!("parityagentkb{}", std::env::consts::EXE_SUFFIX));
    fresh_collections(root);
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
    let wasm = Seen { dumps: dumps(&report(&said)), requests: requests_since(log, from), ms };
    Twin { interp, wasm, interp_shots, runner_shots, runner_ms, build_ms }
}

#[test]
fn the_agentobject_knowledge_base_tools_agree_on_both_engines() {
    let root = std::env::temp_dir().join(format!("prc-t47c-kb-{}", std::process::id()));
    let (port, log) = model_server();
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let embed = format!("http://127.0.0.1:{port}/v1");
    let manifest = project(&root, &url, &embed);
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
    let embeddings = req_w.iter().filter(|b| b.contains("\"stub-embed\"")).count();
    checks.push((
        format!("the stub received the same {} requests from both engines ({} to the embedding server)", req_i.len(), embeddings),
        req_i == req_w && req_i.len() == 7 && embeddings == 2,
    ));

    // ── What the program saw (the runner's dumps; the interpreter's are equal above) ──
    let at = |i: usize, id: &str| twin.wasm.dumps.get(i).map(|d| caption(d, id)).unwrap_or_default();
    let chats: Vec<Value> = req_w.iter().filter_map(|b| serde_json::from_str::<Value>(b).ok()).filter(|r| r.get("messages").is_some()).collect();
    let results: Vec<String> = chats
        .iter()
        .filter(|r| r["messages"].as_array().is_some_and(|m| m.iter().filter(|x| x["role"] == "tool").count() == 2))
        .flat_map(|r| r["messages"].as_array().cloned().unwrap_or_default())
        .filter(|m| m["role"] == "tool")
        .filter_map(|m| m["content"].as_str().map(str::to_owned))
        .collect();
    let asked = |q: &str| chats.iter().find(|r| r["messages"].as_array().is_some_and(|m| m.iter().any(|x| x["content"] == q))).cloned().unwrap_or(Value::Null);
    let shown: Vec<String> = (0..shots.len()).flat_map(|i| LABELS.iter().map(move |id| (i, *id))).map(|(i, id)| format!("    dump {i} {id}: {}", at(i, id).trim())).collect();
    checks.extend([
        ("both collections created and indexed, one document each, the second through the embedding server".to_string(), {
            at(0, "LBL-INDEX").trim() == "index 1   1   1   1" && at(0, "LBL-READY").trim() == "docs indexed" && at(0, "LBL-DENY").trim() == "notes indexed"
        }),
        ("AllowKnowledgeBase: 1 for the control's collection and for another, 0 for a Label and for a name no collection may have".into(), {
            at(0, "LBL-ALLOW").trim() == "allow 1    other 1    label 0    bad 0"
        }),
        ("the request offers both collections, KB-1's by the collection its control names".into(), offered(&asked("What runs on the desktop?")) == ["kb_kb_1_docs", "kb_kb_2_notes"]),
        ("the model searches both, the second through the embedding server, and the answer uses both results".into(), {
            let c = at(0, "LBL-ASK");
            c.starts_with("docs 1, notes 1.")
                && c.contains("|calls 2")
                && results.len() == 2
                && results[0].contains("alpha.md")
                && results[1].contains("beta.md")
                && results[0].contains("(Lexical search)")
                && results[1].contains("(Semantic search)")
        }),
        ("the other agent's request offers no collection".into(), at(1, "LBL-TOOLS").trim() == "Tools: none."),
        ("after DenyKnowledgeBase of KB-2 a request offers KB-1's collection only".into(), {
            let c = at(2, "LBL-ASK");
            c.starts_with("Tools: kb_kb_1_docs.") && c.contains("|calls 0")
        }),
    ]);

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== the_agentobject_knowledge_base_tools_agree_on_both_engines (DESKTOP-WASM-001 T47c: R10, R11, ABI A52) ==");
    println!("form {FORM}: 2 KnowledgeBases (Lexical; Endpoint, embedding on the stub), 2 AgentObjects asking a stub on 127.0.0.1:{port}, 5 buttons, 12 event handlers");
    println!("  AllowKnowledgeBase of a control's own collection, of another, of a Label and of a bad name; a search of each collection the model asks for,");
    println!("  one at once and one on a worker; the other agent's request; DenyKnowledgeBase and a request offering what is left");
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
        println!("    the searches' results: {results:?}");
        println!("kept for inspection: {}", root.display());
    }
    assert!(failed.is_empty(), "failed: {failed:?}");
}
