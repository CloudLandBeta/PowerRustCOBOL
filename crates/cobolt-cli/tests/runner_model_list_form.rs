// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T48 (R10, R11, ABI A53): **the application's model list, its keys and the model
//! providers behave on the WebAssembly path as they do under the interpreter**, and an AgentObject asks
//! the model they name, as spec 076 orders it (the twin of `model_list_hosts`).
//!
//! One form with two AgentObjects and four local stub servers (`TcpListener`s on `127.0.0.1`, no
//! network), each answering with its own name: the model-list entry's server, the `Configuration`'s
//! (the providers `COBOLT_AGENT_PROVIDERS` names), the agent's own, and a provider that lists its
//! models and answers a connection test. Eight buttons cover:
//! - `PROVIDER-COUNT` and `PROVIDER-GET` (the catalogue: no request);
//! - `MODEL-SET` (an entry, and one with no name), `MODEL-COUNT` into a numeric and an alphanumeric
//!   item, `MODEL-GET` with two of its four answers;
//! - an `Ask` with `ModelEntry` naming the entry (the entry's server answers), with it cleared (the
//!   `Configuration`'s), and of the agent with neither (its own);
//! - `KEY-SET`, `KEY-IS-SET`, `KEY-REMOVE` and `KEY-IS-SET` again (a made-up key, kept in the
//!   project's own settings folder);
//! - `MODEL-LIST` of the stub provider, `MODEL-LIST-GET`, `MODEL-TEST`;
//! - `MODEL-REMOVE`, `MODEL-COUNT`, and an `Ask` naming the removed entry, which fails as spec 076 says.
//!
//! The script runs headless under `rcrun run-form` (the interpreter) and in the program `rcrun build`
//! makes on the WebAssembly path, each engine with an empty settings folder. The dumps must be equal,
//! the pictures equal in every pixel, each server must receive the same requests from both engines, and
//! both engines must keep the list in the same file.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cobolt_forms::connections::{agent_to_json, AgentConnection, AGENT_PROVIDERS_ENV};
use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

const FORM: &str = "PARITY-MODELS";

const LABELS: [&str; 7] = ["LBL-PROV", "LBL-SET", "LBL-ASK", "LBL-OWN", "LBL-KEY", "LBL-LIST", "LBL-REMOVE"];

/// A made-up key: never a provider's, kept only in the test project's settings folder.
const MADE_UP_KEY: &str = "not-a-real-key-0000";

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

// ── The stub servers ─────────────────────────────────────────────────────────

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

/// A server that answers a chat request with a reply carrying `name` (in the OpenAI shape and in
/// Ollama's own), and `GET …/api/tags` with two models; each request kept as `path body`.
fn server(name: &'static str) -> (u16, Arc<Mutex<Vec<String>>>) {
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
                log.lock().unwrap().push(format!("{path} {body}"));
                let reply = if path.ends_with("/api/tags") {
                    json!({"models": [{"name": "llama-a:latest"}, {"name": "qwen-b:7b"}]}).to_string()
                } else {
                    let text = format!("from the {name}");
                    json!({
                        "choices": [{"message": {"role": "assistant", "content": text}}],
                        "message": {"role": "assistant", "content": text},
                        "done": true,
                        "usage": {"prompt_tokens": 10, "completion_tokens": 3}
                    })
                    .to_string()
                };
                let _ = stream.write_all(
                    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).as_bytes(),
                );
                let _ = stream.flush();
            });
        }
    });
    (port, seen)
}

/// The four servers, and what each received.
struct Servers {
    entry: (u16, Arc<Mutex<Vec<String>>>),
    config: (u16, Arc<Mutex<Vec<String>>>),
    own: (u16, Arc<Mutex<Vec<String>>>),
    provider: (u16, Arc<Mutex<Vec<String>>>),
    /// The model providers the host is told about, as `COBOLT_AGENT_PROVIDERS` carries them.
    providers: String,
}

fn servers() -> Servers {
    let entry = server("entry");
    let config = server("configuration");
    let own = server("agent's own settings");
    let provider = server("provider");
    let mut conn = AgentConnection::new("cfg-t", "Test provider", format!("http://127.0.0.1:{}/v1/chat/completions", config.0));
    conn.provider = "Ollama".into();
    Servers { entry, config, own, provider, providers: agent_to_json(&[conn]) }
}

fn chat_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/v1/chat/completions")
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

/// WORKING-STORAGE items, one `01` per `(name, picture)`.
fn items(list: &[(&str, &str)]) -> String {
    list.iter().map(|(n, p)| format!("       01 {n:<22} PIC {p}.")).collect::<Vec<_>>().join("\n")
}

/// The fixture project in `root`; its manifest's path.
fn project(root: &Path, s: &Servers) -> PathBuf {
    let _ = fs::remove_dir_all(root);
    for d in ["forms", "generated", "src", "assets"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    let st = |v: &str| PropValue::String(v.into());
    let mut form = Form::new(FORM, "Model list, keys and providers", 760, 340);
    form.main_form = true;
    let buttons = ["BTN-PROV", "BTN-SET", "BTN-ENTRY", "BTN-CONFIG", "BTN-OWN", "BTN-KEY", "BTN-LIST", "BTN-REMOVE"];
    for (i, id) in buttons.iter().enumerate() {
        control(&mut form, id, ControlType::Button, (20, 16 + 38 * i as i32, 120, 28), &[("Caption", st(id.trim_start_matches("BTN-")))]);
    }
    for (i, id) in LABELS.iter().enumerate() {
        control(&mut form, id, ControlType::Label, (160, 16 + 38 * i as i32, 580, 28), &[("Caption", st("-"))]);
    }
    for (i, (id, configuration)) in [("AGT-1", Some("cfg-t")), ("AGT-2", None)].into_iter().enumerate() {
        let mut props = vec![
            ("AgentAPI", st("Ollama")),
            ("AgentURL", st(&chat_url(s.own.0))),
            ("AgentModel", st("own-model")),
            ("AgentAPIKey", st("")),
            ("TimeoutSeconds", st("10")),
        ];
        if let Some(c) = configuration {
            props.push(("Configuration", st(c)));
        }
        control(&mut form, id, ControlType::AgentObject, (600 + 60 * i as i32, 280, 56, 56), &props);
    }
    let provider_root = format!("http://127.0.0.1:{}", s.provider.0);

    // The catalogue: how many providers, and the second one's details.
    bind(
        &mut form,
        "BTN-PROV",
        "onClick",
        &items(&[("WS-N", "9(2)"), ("WS-ID", "X(10)"), ("WS-LB", "X(10)"), ("WS-EP", "X(30)"), ("WS-K", "X"), ("WS-L", "X(80)")]),
        &lines(&[
            "COBOL::\"PROVIDER-COUNT\" ( WS-N )",
            "COBOL::\"PROVIDER-GET\" ( 2 WS-ID WS-LB WS-EP WS-K )",
            "STRING \"prov \" WS-N \" \" WS-ID WS-LB WS-K \" \" WS-EP DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-PROV::Caption",
        ]),
    );
    // An entry, one with no name, the count into a numeric and an alphanumeric item, and two of the
    // first entry's four details.
    let set = format!("COBOL::\"MODEL-SET\" ( \"company-h\" \"Ollama\" \"{}\" \"entry-model\" WS-S1 )", chat_url(s.entry.0));
    bind(
        &mut form,
        "BTN-SET",
        "onClick",
        &items(&[("WS-S1", "X(4)"), ("WS-S2", "X(24)"), ("WS-C", "9(2)"), ("WS-CX", "X(3)"), ("WS-NM", "X(10)"), ("WS-AP", "X(8)"), ("WS-L", "X(100)")]),
        &lines(&[
            &set,
            "COBOL::\"MODEL-SET\" ( \" \" \"Ollama\" \"x\" \"y\" WS-S2 )",
            "COBOL::\"MODEL-COUNT\" ( WS-C )",
            "COBOL::\"MODEL-COUNT\" ( WS-CX )",
            "COBOL::\"MODEL-GET\" ( 1 WS-NM WS-AP )",
            "STRING \"set \" WS-S1 \"|\" WS-S2 \"|\" WS-C \"|\" WS-CX \"|\" WS-NM \"|\" WS-AP DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-SET::Caption",
        ]),
    );
    // Spec 076's order: the entry, then the Configuration, then the agent's own settings.
    let ask = |agent: &str| format!("MOVE {agent}::Ask(\"Hello\") TO WS-T");
    bind(&mut form, "BTN-ENTRY", "onClick", &items(&[("WS-T", "X(20)")]), &lines(&["MOVE \"company-h\" TO AGT-1::ModelEntry", &ask("AGT-1")]));
    bind(&mut form, "BTN-CONFIG", "onClick", &items(&[("WS-T", "X(20)")]), &lines(&["MOVE \" \" TO AGT-1::ModelEntry", &ask("AGT-1")]));
    bind(&mut form, "BTN-OWN", "onClick", &items(&[("WS-T", "X(20)")]), &lines(&[&ask("AGT-2")]));
    bind(&mut form, "AGT-1", "onResponse", &items(&[("WS-R", "X(40)")]), &lines(&["MOVE AGT-1::LastReply TO WS-R", "MOVE WS-R TO LBL-ASK::Caption"]));
    // The error's first 38 characters, which hold no character wider than a byte.
    bind(&mut form, "AGT-1", "onError", &items(&[("WS-E", "X(38)")]), &lines(&["MOVE AGT-1::LastError TO WS-E", "MOVE WS-E TO LBL-ASK::Caption"]));
    bind(&mut form, "AGT-2", "onResponse", &items(&[("WS-R", "X(40)")]), &lines(&["MOVE AGT-2::LastReply TO WS-R", "MOVE WS-R TO LBL-OWN::Caption"]));
    bind(&mut form, "AGT-2", "onError", &items(&[("WS-E", "X(100)")]), &lines(&["MOVE AGT-2::LastError TO WS-E", "MOVE WS-E TO LBL-OWN::Caption"]));
    // The keys: stored, asked about, removed, asked about again.
    let key_set = format!("COBOL::\"KEY-SET\" ( \"company-h\" \"{MADE_UP_KEY}\" WS-S1 )");
    bind(
        &mut form,
        "BTN-KEY",
        "onClick",
        &items(&[("WS-S1", "X(4)"), ("WS-F1", "X"), ("WS-S2", "X(4)"), ("WS-F2", "X"), ("WS-L", "X(40)")]),
        &lines(&[
            &key_set,
            "COBOL::\"KEY-IS-SET\" ( \"company-h\" WS-F1 )",
            "COBOL::\"KEY-REMOVE\" ( \"company-h\" WS-S2 )",
            "COBOL::\"KEY-IS-SET\" ( \"company-h\" WS-F2 )",
            "STRING \"key \" WS-S1 WS-F1 \" \" WS-S2 WS-F2 DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-KEY::Caption",
        ]),
    );
    // The stub provider's models, the second of them, and a connection test.
    let list = format!("COBOL::\"MODEL-LIST\" ( \"Ollama\" \"{provider_root}\" \" \" WS-N WS-S )");
    let test = format!("COBOL::\"MODEL-TEST\" ( \"Ollama\" \"{provider_root}\" \"llama-a:latest\" \" \" WS-T )");
    bind(
        &mut form,
        "BTN-LIST",
        "onClick",
        &items(&[("WS-N", "9(2)"), ("WS-S", "X(4)"), ("WS-M", "X(12)"), ("WS-T", "X(40)"), ("WS-L", "X(80)")]),
        &lines(&[
            &list,
            "COBOL::\"MODEL-LIST-GET\" ( 2 WS-M )",
            &test,
            "STRING \"list \" WS-N \" \" WS-S \"|\" WS-M \"|\" WS-T DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-LIST::Caption",
        ]),
    );
    // The entry withdrawn; an Ask naming it fails.
    bind(
        &mut form,
        "BTN-REMOVE",
        "onClick",
        &items(&[("WS-S", "X(4)"), ("WS-C", "9(2)"), ("WS-L", "X(40)"), ("WS-T", "X(20)")]),
        &lines(&[
            "COBOL::\"MODEL-REMOVE\" ( \"company-h\" WS-S )",
            "COBOL::\"MODEL-COUNT\" ( WS-C )",
            "STRING \"removed \" WS-S \" count \" WS-C DELIMITED BY SIZE INTO WS-L",
            "MOVE WS-L TO LBL-REMOVE::Caption",
            "MOVE \"company-h\" TO AGT-1::ModelEntry",
            &ask("AGT-1"),
        ]),
    );

    let cfrm = root.join("forms/parity-models.cfrm");
    cobolt_forms::save_form(&form, &cfrm).unwrap();
    let form = cobolt_forms::load_form(&cfrm).unwrap();
    fs::write(root.join("generated/parity-models.cbl"), cobolt_codegen::generate(&form)).unwrap();
    fs::write(root.join("src/main.cbl"), "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n").unwrap();
    let manifest = root.join("paritymodels.project.toml");
    fs::write(
        &manifest,
        "[project]\nname = \"paritymodels\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/parity-models.cfrm\"]\ngenerated = [\"generated/parity-models.cbl\"]\n",
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
    let names = vec!["m1-entry", "m2-configuration-own", "m3-keys-list", "m4-removed"];
    let steps = vec![
        wait(200),
        click("BTN-PROV"),
        wait(300),
        click("BTN-SET"),
        wait(300),
        click("BTN-ENTRY"),
        wait(1500),
        dump(),
        shot(names[0]),
        click("BTN-CONFIG"),
        wait(1500),
        click("BTN-OWN"),
        wait(1500),
        dump(),
        shot(names[1]),
        click("BTN-KEY"),
        wait(300),
        click("BTN-LIST"),
        wait(1500),
        dump(),
        shot(names[2]),
        click("BTN-REMOVE"),
        wait(1000),
        dump(),
        shot(names[3]),
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

/// What each server received from one engine, in order of the servers: entry, Configuration, own,
/// provider; each list sorted.
type Requests = [Vec<String>; 4];

/// What one engine did.
struct Seen {
    dumps: Vec<Value>,
    requests: Requests,
    /// The model list's file after the run, as text (`None`: not written).
    list_file: Option<String>,
    /// Whether the key file exists after the run.
    key_file: bool,
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

fn marks(s: &Servers) -> [usize; 4] {
    [s.entry.1.lock().unwrap().len(), s.config.1.lock().unwrap().len(), s.own.1.lock().unwrap().len(), s.provider.1.lock().unwrap().len()]
}

/// The requests each server received since `from`, each list sorted.
fn requests_since(s: &Servers, from: [usize; 4]) -> Requests {
    let take = |log: &Arc<Mutex<Vec<String>>>, i: usize| {
        let mut r: Vec<String> = log.lock().unwrap()[from[i]..].to_vec();
        r.sort();
        r
    };
    [take(&s.entry.1, 0), take(&s.config.1, 1), take(&s.own.1, 2), take(&s.provider.1, 3)]
}

/// The project's settings folder, gone: each engine starts with an empty model list and no keys.
fn fresh_settings(root: &Path) {
    let _ = fs::remove_dir_all(root.join("settings"));
}

fn settings_after(root: &Path) -> (Option<String>, bool) {
    (fs::read_to_string(root.join("settings/models.json")).ok(), root.join("settings/model-keys.dat").is_file())
}

/// Run `script` under `rcrun run-form` (the interpreter), then build the project on the
/// WebAssembly path and run it in the built program.
fn run_twin(root: &Path, manifest: &Path, script: &Path, s: &Servers) -> Twin {
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
    fresh_settings(root);
    let interp_shots = root.join("shots-interpreter");
    fs::create_dir_all(&interp_shots).unwrap();
    let from = marks(s);
    let t = Instant::now();
    let out = Command::new(rcrun())
        .args(["run-form", "forms/parity-models.cfrm", "generated/parity-models.cbl", "--designer", "--headless"])
        .arg(script)
        .args(["--headless-limit", "60"])
        .current_dir(root)
        .env("HOME", &home)
        .env(AGENT_PROVIDERS_ENV, &s.providers)
        .env("PRC_HEADLESS_SHOTS", &interp_shots)
        .output()
        .expect("run rcrun run-form");
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form failed:\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    thread::sleep(Duration::from_millis(500));
    let (list_file, key_file) = settings_after(root);
    let interp = Seen { dumps: dumps(&report(&said)), requests: requests_since(s, from), list_file, key_file, ms };

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
    let exe = root.join("bin").join(format!("paritymodels{}", std::env::consts::EXE_SUFFIX));
    fresh_settings(root);
    let runner_shots = root.join("shots-runner");
    fs::create_dir_all(&runner_shots).unwrap();
    let from = marks(s);
    let t = Instant::now();
    let out = Command::new(&exe)
        .current_dir(root)
        .env("HOME", &home)
        .env(AGENT_PROVIDERS_ENV, &s.providers)
        .env("PRC_RUNNER_CACHE", root.join("runner-cache"))
        .env("PRC_HEADLESS_SCRIPT", script)
        .env("PRC_HEADLESS_LIMIT", "60")
        .env("PRC_HEADLESS_SHOTS", &runner_shots)
        .output()
        .expect("run the built program");
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "the built program failed:\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    thread::sleep(Duration::from_millis(500));
    let (list_file, key_file) = settings_after(root);
    let wasm = Seen { dumps: dumps(&report(&said)), requests: requests_since(s, from), list_file, key_file, ms };
    Twin { interp, wasm, interp_shots, runner_shots, runner_ms, build_ms }
}

#[test]
fn the_model_list_keys_and_providers_agree_on_both_engines() {
    let root = std::env::temp_dir().join(format!("prc-t48-models-{}", std::process::id()));
    let s = servers();
    let manifest = project(&root, &s);
    let (steps, shots) = script();
    let script = root.join("script.json");
    fs::write(&script, steps.to_string()).unwrap();
    let twin = run_twin(&root, &manifest, &script, &s);

    // ── The two engines against each other: dumps, pictures, requests, the settings files ──
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
    let counts = |r: &Requests| r.iter().map(Vec::len).collect::<Vec<_>>();
    checks.push((
        format!("each server received the same requests from both engines (entry, Configuration, own, provider: {:?})", counts(req_w)),
        req_i == req_w && counts(req_w) == [1, 1, 1, 2],
    ));
    checks.push((
        "both engines keep the list in the project's settings folder, with the same entries".into(),
        twin.interp.list_file.is_some() && twin.interp.list_file == twin.wasm.list_file && twin.interp.key_file && twin.wasm.key_file,
    ));

    // ── What the program saw (the runner's dumps; the interpreter's are equal above) ──
    let at = |i: usize, id: &str| twin.wasm.dumps.get(i).map(|d| caption(d, id)).unwrap_or_default();
    let second = &cobolt_runtime::providers::PROVIDERS[1];
    let want_prov = format!(
        "prov {:02} {:<10}{:<10}{} {}",
        cobolt_runtime::providers::PROVIDERS.len(),
        second.id,
        second.label,
        if cobolt_runtime::providers::requires_key(second.id) { "Y" } else { "N" },
        second.default_endpoint
    );
    let shown: Vec<String> = (0..shots.len()).flat_map(|i| LABELS.iter().map(move |id| (i, *id))).map(|(i, id)| format!("    dump {i} {id}: {}", at(i, id).trim_end())).collect();
    checks.extend([
        ("PROVIDER-COUNT and PROVIDER-GET answer the catalogue: its length, and the second provider's id, label, key rule and endpoint".to_string(), at(0, "LBL-PROV").trim_end() == want_prov.trim_end()),
        ("MODEL-SET answers OK, and why an entry with no name is refused; MODEL-COUNT 1 into a numeric and an alphanumeric item; MODEL-GET the entry's name and API".into(), {
            at(0, "LBL-SET").trim_end() == "set OK  |the entry has no name   |01|1  |company-h |Ollama"
        }),
        ("an Ask with ModelEntry naming the entry goes to the entry's server, with its model".into(), {
            at(0, "LBL-ASK").trim() == "from the entry" && req_w[0].first().is_some_and(|r| r.contains("\"entry-model\""))
        }),
        ("with the entry cleared the Configuration's server answers, with the agent's own model; the other agent asks its own".into(), {
            at(1, "LBL-ASK").trim() == "from the configuration"
                && at(1, "LBL-OWN").trim() == "from the agent's own settings"
                && req_w[1].first().is_some_and(|r| r.contains("\"own-model\""))
        }),
        ("KEY-SET, KEY-IS-SET, KEY-REMOVE, KEY-IS-SET: OK, Y, OK, N; no request carries the key".into(), {
            at(2, "LBL-KEY").trim_end() == "key OK  Y OK  N" && !req_w.iter().flatten().any(|r| r.contains(MADE_UP_KEY))
        }),
        ("MODEL-LIST finds the provider's 2 models, MODEL-LIST-GET the second, MODEL-TEST answers OK".into(), {
            at(2, "LBL-LIST").trim_end() == "list 02 OK  |qwen-b:7b   |OK"
                && req_w[3].iter().any(|r| r.starts_with("/api/tags"))
                && req_w[3].iter().any(|r| r.contains("\"llama-a:latest\""))
        }),
        ("MODEL-REMOVE answers OK and leaves 0 entries; an Ask naming the removed entry fails as spec 076 says".into(), {
            at(3, "LBL-REMOVE").trim_end() == "removed OK   count 00" && at(3, "LBL-ASK").starts_with("model entry 'company-h' does not exist")
        }),
    ]);

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== the_model_list_keys_and_providers_agree_on_both_engines (DESKTOP-WASM-001 T48: R10, R11, ABI A53) ==");
    println!("form {FORM}: 2 AgentObjects, 8 buttons, 12 event handlers; 4 stub servers on 127.0.0.1 (entry, Configuration, own, provider)");
    println!("  PROVIDER-COUNT, PROVIDER-GET; MODEL-SET (an entry, an unnamed one), MODEL-COUNT (numeric, alphanumeric), MODEL-GET;");
    println!("  Ask by the entry, by the Configuration, by the agent's own settings; KEY-SET, KEY-IS-SET, KEY-REMOVE, KEY-IS-SET;");
    println!("  MODEL-LIST, MODEL-LIST-GET, MODEL-TEST; MODEL-REMOVE, MODEL-COUNT and an Ask naming the removed entry");
    println!("real time; {} dumps, {} pictures; requests per engine {:?}", shots.len(), shots.len(), counts(req_w));
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
        for (k, (a, b)) in req_i.iter().zip(req_w.iter()).enumerate() {
            if a != b {
                println!("    server {k}: interpreter {a:?}\n              runner      {b:?}");
            }
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
        println!("    the list file: interpreter {:?} / runner {:?}", twin.interp.list_file, twin.wasm.list_file);
        println!("kept for inspection: {}", root.display());
    }
    assert!(failed.is_empty(), "failed: {failed:?}");
}
