// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T47c (R10, R11, ABI A50): **the AgentObject behaves on the WebAssembly path as
//! it does under the interpreter.**
//!
//! One form with seven AgentObjects, each asking a local stub model server (a `TcpListener` on
//! `127.0.0.1`, no network) that answers OpenAI-shaped streams chosen by the request itself, so both
//! engines get the same answers whatever the order of their requests:
//! - `AGT-1`, `Verbose`: a plain `Ask`, its answer, stop reason and token counts, and the lines it
//!   narrates;
//! - `AGT-2`, `StreamReply`: the answer in three pieces, `onPartialReply` on the way;
//! - `AGT-3`: an answer cut at its token limit and asked to go on (`ContinuationCount`);
//! - `AGT-4`: a declared tool (`AddTool`), the model's call reaching `onToolCall`, the handler's
//!   `SetToolResult`, and the answer that uses it;
//! - `AGT-5`: `Ask` then `Cancel()` in the same handler: `onCancelled`, and the late answer dropped;
//! - `AGT-6`: a model that never starts, cut off at `StartTimeoutSeconds`: `onTimeout`;
//! - `AGT-7`: a server that refuses the connection: `onError`.
//!
//! The script runs headless under `rcrun run-form` (the interpreter) and in the program `rcrun build`
//! makes on the WebAssembly path, in real time. The dumps must be equal, the pictures equal in every
//! pixel, the narrated lines equal, and the requests the stub received the same on both engines.

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

const FORM: &str = "PARITY-AGENT";

/// Whether `AGT-2` raised `onPartialReply`, seen by its `onResponse`.
const FORM_WS: &str = "       01 WS-PARTS                GLOBAL PIC 9 VALUE 0.";

const AGENTS: [&str; 7] = ["AGT-1", "AGT-2", "AGT-3", "AGT-4", "AGT-5", "AGT-6", "AGT-7"];
const LABELS: [&str; 10] = ["LBL-ASK", "LBL-ASKED", "LBL-STREAM", "LBL-CONT", "LBL-TOOL", "LBL-TOOLREPLY", "LBL-CANCEL", "LBL-CANCELLED", "LBL-TIMEOUT", "LBL-ERR"];

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

/// The lines to stream for a request, each after its pause in ms, chosen by what it asks.
fn answer_for(body: &str) -> Vec<(u64, String)> {
    let req: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let messages = req["messages"].as_array().cloned().unwrap_or_default();
    let last_user = messages.iter().rev().find(|m| m["role"] == "user").and_then(|m| m["content"].as_str()).unwrap_or("").to_owned();
    let quick = |lines: Vec<String>| lines.into_iter().map(|l| (20, l)).collect::<Vec<_>>();
    if messages.iter().any(|m| m["role"] == "tool") {
        let mut l = vec![piece("The EUR rate is 1.08.")];
        l.extend(finish("stop", 40, 7));
        return quick(l);
    }
    if last_user.starts_with("Your previous answer was cut off") {
        let mut l = vec![piece(" and they lived happily.")];
        l.extend(finish("stop", 30, 5));
        return quick(l);
    }
    match last_user.as_str() {
        "Say hello." => {
            let mut l = vec![piece("Hello from the stub.")];
            l.extend(finish("stop", 12, 5));
            quick(l)
        }
        "Stream three words." => {
            let mut l: Vec<(u64, String)> = ["One", " two", " three."].iter().map(|w| (250, piece(w))).collect();
            l.extend(finish("stop", 14, 3).into_iter().map(|x| (250, x)));
            l
        }
        "Tell a long story." => {
            let mut l = vec![piece("Once upon a time")];
            l.extend(finish("length", 20, 4));
            quick(l)
        }
        "What is the EUR rate?" => vec![
            (20, r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"get_rate","arguments":""}}]}}]}"#.to_owned()),
            (20, r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"currency\":\"EUR\"}"}}]}}]}"#.to_owned()),
            (20, r#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#.to_owned()),
            (20, r#"data: {"choices":[],"usage":{"prompt_tokens":25,"completion_tokens":9}}"#.to_owned()),
            (20, "data: [DONE]".to_owned()),
        ],
        // Answered too late: the program has cancelled it by then.
        "Take your time." => {
            let mut l = vec![(3000, piece("Too late."))];
            l.extend(finish("stop", 10, 2).into_iter().map(|x| (20, x)));
            l
        }
        // Never begins: the client lets go at its start limit.
        "Never answer." => vec![(4000, "data: [DONE]".to_owned())],
        _ => {
            let mut l = vec![piece("I was not asked that.")];
            l.extend(finish("stop", 1, 1));
            quick(l)
        }
    }
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

/// The fixture project in `root`, its agents asking `url` (`AGT-7` asking `refused`); its manifest's path.
fn project(root: &Path, url: &str, refused: &str) -> PathBuf {
    let _ = fs::remove_dir_all(root);
    for d in ["forms", "generated", "src"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    let s = |v: &str| PropValue::String(v.into());
    let mut form = Form::new(FORM, "AgentObject form", 760, 440);
    form.main_form = true;
    form.user_ws_source = FORM_WS.into();
    let buttons = ["BTN-ASK", "BTN-STREAM", "BTN-CONT", "BTN-TOOL", "BTN-CANCEL", "BTN-TIMEOUT", "BTN-ERR"];
    for (i, id) in buttons.iter().enumerate() {
        control(&mut form, id, ControlType::Button, (20, 20 + 38 * i as i32, 120, 28), &[("Caption", s(id.trim_start_matches("BTN-")))]);
    }
    for (i, id) in LABELS.iter().enumerate() {
        control(&mut form, id, ControlType::Label, (160, 16 + 30 * i as i32, 580, 26), &[("Caption", s("-"))]);
    }
    for (i, id) in AGENTS.iter().enumerate() {
        let mut props = vec![
            ("AgentAPI", s("OpenAI")),
            ("AgentURL", s(if *id == "AGT-7" { refused } else { url })),
            ("AgentModel", s("stub-model")),
            ("SystemPrompt", s("Be brief.")),
            ("Temperature", s("0")),
            ("MaximumTokens", s("100")),
            ("TimeoutSeconds", s("10")),
        ];
        match *id {
            "AGT-1" => props.push(("Verbose", PropValue::Bool(true))),
            "AGT-2" => props.push(("StreamReply", PropValue::Bool(true))),
            "AGT-3" => props.push(("MaximumContinuations", s("2"))),
            "AGT-6" => {
                props.retain(|(k, _)| *k != "TimeoutSeconds");
                props.extend([("TimeoutSeconds", s("30")), ("StartTimeoutSeconds", s("1"))]);
            }
            _ => {}
        }
        control(&mut form, id, ControlType::AgentObject, (300 + 60 * i as i32, 376, 56, 56), &props);
    }

    // AGT-1: a plain Ask, Verbose.
    bind(
        &mut form,
        "BTN-ASK",
        "onClick",
        "       01 WS-T                    PIC X(20).\n       01 WS-B                    PIC X(5).\n       01 WS-L                    PIC X(60).",
        &lines(&[
            "MOVE AGT-1::Ask(\"Say hello.\") TO WS-T",
            "MOVE AGT-1::Busy TO WS-B",
            "STRING \"asked [\" WS-T \"] busy \" WS-B DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-ASKED::Caption",
        ]),
    );
    bind(
        &mut form,
        "AGT-1",
        "onResponse",
        "       01 WS-R                    PIC X(30).\n       01 WS-S                    PIC X(10).\n       01 WS-I                    PIC X(6).\n       01 WS-O                    PIC X(6).\n       01 WS-B                    PIC X(5).\n       01 WS-L                    PIC X(90).",
        &lines(&[
            "MOVE AGT-1::LastReply TO WS-R",
            "MOVE AGT-1::StopReason TO WS-S",
            "MOVE AGT-1::LastInputTokens TO WS-I",
            "MOVE AGT-1::LastOutputTokens TO WS-O",
            "MOVE AGT-1::Busy TO WS-B",
            "STRING WS-R \"|\" WS-S \"|in \" WS-I \" out \" WS-O \" busy \" WS-B DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-ASK::Caption",
        ]),
    );
    // AGT-2: a streamed reply.
    bind(&mut form, "BTN-STREAM", "onClick", "       01 WS-T                    PIC X(20).", &lines(&["MOVE AGT-2::Ask(\"Stream three words.\") TO WS-T"]));
    bind(&mut form, "AGT-2", "onPartialReply", "", &lines(&["MOVE 1 TO WS-PARTS"]));
    bind(
        &mut form,
        "AGT-2",
        "onResponse",
        "       01 WS-R                    PIC X(30).\n       01 WS-L                    PIC X(70).",
        &lines(&["MOVE AGT-2::LastReply TO WS-R", "STRING \"streamed \" WS-R \" partials \" WS-PARTS DELIMITED BY SIZE INTO WS-L", "MOVE WS-L TO LBL-STREAM::Caption"]),
    );
    // AGT-3: a reply cut at its token limit, continued.
    bind(&mut form, "BTN-CONT", "onClick", "       01 WS-T                    PIC X(20).", &lines(&["MOVE AGT-3::Ask(\"Tell a long story.\") TO WS-T"]));
    bind(
        &mut form,
        "AGT-3",
        "onResponse",
        "       01 WS-R                    PIC X(45).\n       01 WS-C                    PIC X(4).\n       01 WS-X                    PIC X(5).\n       01 WS-L                    PIC X(90).",
        &lines(&[
            "MOVE AGT-3::LastReply TO WS-R",
            "MOVE AGT-3::ContinuationCount TO WS-C",
            "MOVE AGT-3::Truncated TO WS-X",
            "STRING WS-R \"|continued \" WS-C \" truncated \" WS-X DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-CONT::Caption",
        ]),
    );
    // AGT-4: a declared tool, answered by the program.
    bind(
        &mut form,
        "BTN-TOOL",
        "onClick",
        "       01 WS-A                    PIC X(4).\n       01 WS-T                    PIC X(20).",
        &lines(&["MOVE AGT-4::AddTool(\"get_rate\" \"Look up an exchange rate\" \"currency:string\") TO WS-A", "MOVE AGT-4::Ask(\"What is the EUR rate?\") TO WS-T"]),
    );
    bind(
        &mut form,
        "AGT-4",
        "onToolCall",
        "       01 WS-ID                   PIC X(20).\n       01 WS-N                    PIC X(20).\n       01 WS-G                    PIC X(40).\n       01 WS-A                    PIC X(4).\n       01 WS-L                    PIC X(90).",
        &lines(&[
            "MOVE AGT-4::ToolCallId TO WS-ID",
            "MOVE AGT-4::ToolName TO WS-N",
            "MOVE AGT-4::ToolArguments TO WS-G",
            "STRING \"call \" WS-ID \" \" WS-N \" \" WS-G DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-TOOL::Caption",
            "MOVE AGT-4::SetToolResult(WS-ID \"1.08\") TO WS-A",
        ]),
    );
    bind(
        &mut form,
        "AGT-4",
        "onResponse",
        "       01 WS-R                    PIC X(30).\n       01 WS-C                    PIC X(4).\n       01 WS-L                    PIC X(60).",
        &lines(&["MOVE AGT-4::LastReply TO WS-R", "MOVE AGT-4::LastToolCallCount TO WS-C", "STRING WS-R \"|calls \" WS-C DELIMITED BY SIZE INTO WS-L", "MOVE WS-L TO LBL-TOOLREPLY::Caption"]),
    );
    // AGT-5: Ask, then Cancel.
    bind(
        &mut form,
        "BTN-CANCEL",
        "onClick",
        "       01 WS-T                    PIC X(20).\n       01 WS-A                    PIC X(4).\n       01 WS-B1                   PIC X(5).\n       01 WS-B2                   PIC X(5).\n       01 WS-L                    PIC X(60).",
        &lines(&[
            "MOVE AGT-5::Ask(\"Take your time.\") TO WS-T",
            "MOVE AGT-5::Busy TO WS-B1",
            "MOVE AGT-5::Cancel() TO WS-A",
            "MOVE AGT-5::Busy TO WS-B2",
            "STRING \"busy \" DELIMITED BY SIZE WS-B1 DELIMITED BY SPACE \" then \" WS-B2 DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-CANCEL::Caption",
        ]),
    );
    bind(&mut form, "AGT-5", "onCancelled", "", &lines(&["MOVE \"cancelled\" TO LBL-CANCELLED::Caption"]));
    bind(&mut form, "AGT-5", "onResponse", "", &lines(&["MOVE \"answered after Cancel\" TO LBL-CANCELLED::Caption"]));
    // AGT-6: a model that never starts.
    bind(&mut form, "BTN-TIMEOUT", "onClick", "       01 WS-T                    PIC X(20).", &lines(&["MOVE AGT-6::Ask(\"Never answer.\") TO WS-T"]));
    bind(
        &mut form,
        "AGT-6",
        "onTimeout",
        "       01 WS-E                    PIC X(80).\n       01 WS-B                    PIC X(5).\n       01 WS-L                    PIC X(100).",
        &lines(&["MOVE AGT-6::LastError TO WS-E", "MOVE AGT-6::Busy TO WS-B", "STRING \"busy \" WS-B \" \" WS-E DELIMITED BY SIZE INTO WS-L", "MOVE WS-L TO LBL-TIMEOUT::Caption"]),
    );
    bind(&mut form, "AGT-6", "onResponse", "", &lines(&["MOVE \"answered after the time-out\" TO LBL-TIMEOUT::Caption"]));
    // AGT-7: the connection is refused.
    bind(&mut form, "BTN-ERR", "onClick", "       01 WS-T                    PIC X(20).", &lines(&["MOVE AGT-7::Ask(\"Anybody there?\") TO WS-T"]));
    bind(
        &mut form,
        "AGT-7",
        "onError",
        "       01 WS-E                    PIC X(120).\n       01 WS-B                    PIC X(5).\n       01 WS-L                    PIC X(140).",
        &lines(&["MOVE AGT-7::LastError TO WS-E", "MOVE AGT-7::Busy TO WS-B", "STRING \"busy \" WS-B \" \" WS-E DELIMITED BY SIZE INTO WS-L", "MOVE WS-L TO LBL-ERR::Caption"]),
    );

    let cfrm = root.join("forms/parity-agent.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/parity-agent.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(root.join("src/main.cbl"), "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n").unwrap();
    let manifest = root.join("parityagent.project.toml");
    fs::write(
        &manifest,
        "[project]\nname = \"parityagent\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/parity-agent.cfrm\"]\ngenerated = [\"generated/parity-agent.cbl\"]\n",
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
    let names = vec!["a1-asked", "a2-streamed", "a3-continued-tool", "a4-cancelled", "a5-timeout-error"];
    let steps = vec![
        wait(200),
        click("BTN-ASK"),
        wait(1500),
        dump(),
        shot(names[0]),
        click("BTN-STREAM"),
        wait(2500),
        dump(),
        shot(names[1]),
        click("BTN-CONT"),
        wait(1500),
        click("BTN-TOOL"),
        wait(2000),
        dump(),
        shot(names[2]),
        click("BTN-CANCEL"),
        wait(4000),
        dump(),
        shot(names[3]),
        click("BTN-TIMEOUT"),
        wait(2500),
        click("BTN-ERR"),
        wait(1500),
        dump(),
        shot(names[4]),
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
        .args(["run-form", "forms/parity-agent.cfrm", "generated/parity-agent.cbl", "--designer", "--headless"])
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
    let exe = root.join("bin").join(format!("parityagent{}", std::env::consts::EXE_SUFFIX));
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
fn the_agentobject_agrees_on_both_engines() {
    let root = std::env::temp_dir().join(format!("prc-t47c-form-{}", std::process::id()));
    let (port, log) = model_server();
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    // A port nothing listens on: bound, then let go.
    let refused_port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let refused = format!("http://127.0.0.1:{refused_port}/v1/chat/completions");
    let manifest = project(&root, &url, &refused);
    let (steps, shots) = script();
    let script = root.join("script.json");
    fs::write(&script, steps.to_string()).unwrap();
    let twin = run_twin(&root, &manifest, &script, &log);

    // ── The two engines against each other: dumps, pictures, narration, requests ──
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
    let (said_i, said_w) = (&twin.interp.narration, &twin.wasm.narration);
    checks.push((format!("Verbose narrated the same {} lines on both engines", said_i.len()), said_i == said_w && !said_i.is_empty()));
    let mut narration_lines = Vec::new();
    if said_i != said_w {
        let n = said_i.len().max(said_w.len());
        for k in 0..n {
            let (a, b) = (said_i.get(k).map(String::as_str).unwrap_or("-"), said_w.get(k).map(String::as_str).unwrap_or("-"));
            if a != b {
                narration_lines.push(format!("    narrated line {k}: interpreter {a:?} / runner {b:?}"));
            }
        }
    }
    let (req_i, req_w) = (&twin.interp.requests, &twin.wasm.requests);
    checks.push((format!("the stub received the same {} requests from both engines", req_i.len()), req_i == req_w && req_i.len() == 8));

    // ── What the program saw (the runner's dumps; the interpreter's are equal above) ──
    let at = |i: usize, id: &str| twin.wasm.dumps.get(i).map(|d| caption(d, id)).unwrap_or_default();
    let said = |l: &str| said_w.iter().any(|x| x == l);
    let shown: Vec<String> = (0..shots.len()).flat_map(|i| LABELS.iter().map(move |id| (i, *id))).map(|(i, id)| format!("    dump {i} {id}: {}", at(i, id).trim())).collect();
    checks.extend([
        ("Ask answers the empty string at once and busies the control".to_string(), {
            // The answer is the empty string, which fills WS-T with spaces.
            let c = at(0, "LBL-ASKED");
            c.trim() == format!("asked [{}] busy true", " ".repeat(20))
        }),
        ("onResponse: the answer, its stop reason, the token counts, no longer busy".into(), {
            let c = at(0, "LBL-ASK");
            c.starts_with("Hello from the stub.") && c.contains("|stop") && c.contains("|in 12") && c.contains(" out 5") && c.trim_end().ends_with("busy false")
        }),
        ("Verbose narrates the request and the response".into(), said("[agent AGT-1] ── request ──────────────────────") && said(&format!("[agent AGT-1] POST {url}")) && said("[agent AGT-1] LastReply set — onResponse fires next")),
        ("StreamReply: onPartialReply on the way, the whole answer at the end".into(), {
            let c = at(1, "LBL-STREAM");
            c.starts_with("streamed One two three. ") && c.trim_end().ends_with(" partials 1")
        }),
        ("an answer cut at its limit is asked to go on, once".into(), {
            let c = at(2, "LBL-CONT");
            c.starts_with("Once upon a time and they lived happily.") && c.contains("|continued 1") && c.trim_end().ends_with("truncated 0")
        }),
        ("a declared tool's call reaches onToolCall with its id, name and arguments".into(), at(2, "LBL-TOOL").contains("call_1") && at(2, "LBL-TOOL").contains("get_rate") && at(2, "LBL-TOOL").contains(r#"{"currency":"EUR"}"#)),
        ("SetToolResult goes back to the model, whose answer uses it".into(), at(2, "LBL-TOOLREPLY").starts_with("The EUR rate is 1.08.") && at(2, "LBL-TOOLREPLY").contains("|calls 1")),
        ("Ask then Cancel(): busy, then no longer busy".into(), at(3, "LBL-CANCEL").trim() == "busy true then false"),
        ("… onCancelled, and the late answer is dropped".into(), at(3, "LBL-CANCELLED").trim() == "cancelled"),
        ("a model that never starts: onTimeout at StartTimeoutSeconds, no longer busy".into(), at(4, "LBL-TIMEOUT").trim() == "busy false The model did not start answering within 1 second: the call was cancelled."),
        ("a refused connection: onError, LastError says why, no longer busy".into(), at(4, "LBL-ERR").starts_with("busy false ") && at(4, "LBL-ERR").trim().len() > "busy false ".len()),
    ]);

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== the_agentobject_agrees_on_both_engines (DESKTOP-WASM-001 T47c: R10, R11, ABI A50) ==");
    println!("form {FORM}: 7 AgentObjects (OpenAI-shaped streams from a stub on 127.0.0.1:{port}; one asking a port nothing listens on), 7 buttons, 15 event handlers");
    println!("  a plain Ask with Verbose; StreamReply and onPartialReply; a continuation; a declared tool, onToolCall and SetToolResult;");
    println!("  Ask then Cancel(), onCancelled; StartTimeoutSeconds and onTimeout; a refused connection and onError");
    println!("real time; {} dumps, {} pictures, {} narrated lines, {} requests per engine", shots.len(), shots.len(), said_w.len(), req_w.len());
    println!("runner built in {:.0} ms; rcrun build (WebAssembly path) {:.0} ms", twin.runner_ms, twin.build_ms);
    println!("interpreter run {:.0} ms; runner run {:.0} ms", twin.interp.ms, twin.wasm.ms);
    for l in &shot_lines {
        println!("{l}");
    }
    println!("pixels compared: {pixels_total}, differing: {differing_total}");
    for l in dump_lines.iter().chain(narration_lines.iter()) {
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
        println!("kept for inspection: {}", root.display());
    }
    assert!(failed.is_empty(), "failed: {failed:?}");
}
