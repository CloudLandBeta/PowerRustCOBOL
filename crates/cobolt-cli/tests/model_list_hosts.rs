// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 076 (AC9, AC12): **which model an agent asks, on the three hosts that run a form** —
//! `rcrun run-form`, a child form loaded into an application shell's ContentPane, and a built
//! binary (`--ignored`: it compiles one).
//!
//! One form with two agents and three buttons, and three scripted local servers that each answer
//! with their own name and count what they were asked:
//!
//! | agent | has | the question goes to |
//! |---|---|---|
//! | `AGT-1` | a list entry the program hands over, a `Configuration`, its own URL | the **entry** server |
//! | `AGT-1` | the entry cleared, the `Configuration`, its own URL | the **Configuration** server |
//! | `AGT-2` | its own URL only | its **own** server |
//!
//! The `Configuration` reaches a running form as the model providers the host publishes from
//! `COBOLT_AGENT_PROVIDERS`, which the test sets; the entry is `COBOL::"MODEL-SET"` run by the
//! form's own handler. Each host must give the same three replies and the same request counts.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use cobolt_forms::connections::{agent_to_json, AgentConnection, AGENT_PROVIDERS_ENV};
use cobolt_forms::model::FormFormat;
use cobolt_forms::{Control, ControlType, EventBinding, Form, PropValue, Rect};
use serde_json::{json, Value};

struct Project {
    root: PathBuf,
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

/// A server that answers every request with a chat reply carrying `name`, and keeps each request whole.
fn server(name: &'static str) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let head_end = loop {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break None,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                }
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break Some(i + 4);
                }
            };
            let Some(head_end) = head_end else { continue };
            let head = String::from_utf8_lossy(&buf[..head_end]).to_ascii_lowercase();
            let len = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            while buf.len() < head_end + len {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                }
            }
            log.lock().unwrap().push(String::from_utf8_lossy(&buf).into_owned());
            let reply = json!({
                "choices": [{"message": {"content": format!("from the {name}")}}],
                "usage": {"prompt_tokens": 10, "completion_tokens": 3}
            })
            .to_string();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                reply.len()
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    (format!("http://127.0.0.1:{port}/v1/chat/completions"), seen)
}

/// The three servers, and what they were asked.
struct Servers {
    entry: Arc<Mutex<Vec<String>>>,
    config: Arc<Mutex<Vec<String>>>,
    own: Arc<Mutex<Vec<String>>>,
    /// The model providers the host is told about, as `COBOLT_AGENT_PROVIDERS` carries them.
    providers: String,
    entry_url: String,
    own_url: String,
}

fn servers() -> Servers {
    let (entry_url, entry) = server("entry");
    let (config_url, config) = server("configuration");
    let (own_url, own) = server("agent's own settings");
    let mut conn = AgentConnection::new("cfg-t", "Test provider", config_url);
    conn.provider = "Ollama".into();
    Servers { entry, config, own, providers: agent_to_json(&[conn]), entry_url, own_url }
}

fn handler(statements: &str) -> String {
    format!("       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n{statements}\n")
}

fn button(form: &mut Form, id: &str, y: i32, statements: &str) {
    let mut c = Control::new(id, ControlType::Button, 20, y);
    c.rect = Rect::new(20, y, 200, 28);
    c.set_prop("Caption", PropValue::String(id.into()));
    c.events.push(EventBinding { event: "onClick".into(), paragraph: format!("{id}--ONCLICK"), code: handler(statements) });
    form.controls.push(c);
}

fn agent(id: &str, y: i32, own_url: &str, configuration: Option<&str>) -> Control {
    let mut a = Control::new(id, ControlType::AgentObject, 300, y);
    a.set_prop("AgentAPI", PropValue::String("Ollama".into()));
    a.set_prop("AgentURL", PropValue::String(own_url.into()));
    a.set_prop("AgentModel", PropValue::String("own-model".into()));
    a.set_prop("AgentAPIKey", PropValue::String(String::new()));
    a.set_prop("TimeoutSeconds", PropValue::String("10".into()));
    if let Some(c) = configuration {
        a.set_prop("Configuration", PropValue::String(c.into()));
    }
    a
}

/// The form every host runs.
fn models_form(s: &Servers) -> Form {
    let mut f = Form::new("MODELS-DEMO", "Models", 640, 360);
    f.form_format = FormFormat::Both;
    f.controls.push(agent("AGT-1", 20, &s.own_url, Some("cfg-t")));
    f.controls.push(agent("AGT-2", 60, &s.own_url, None));
    button(
        &mut f,
        "BTN-ENTRY",
        20,
        &format!(
            "           COBOL::\"MODEL-SET\" ( \"company-h\", \"Ollama\", \"{}\", \"entry-model\" )\n\
             \x20          MOVE \"company-h\" TO AGT-1::ModelEntry\n\
             \x20          AGT-1::Ask( \"Hello\" )",
            s.entry_url
        ),
    );
    button(&mut f, "BTN-CONFIG", 56, "           MOVE \" \" TO AGT-1::ModelEntry\n           AGT-1::Ask( \"Hello\" )");
    button(&mut f, "BTN-OWN", 92, "           AGT-2::Ask( \"Hello\" )");
    f
}

fn project(tag: &str, s: &Servers) -> Project {
    let root = std::env::temp_dir().join(format!("prc-076-hosts-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for d in ["forms", "generated", "src", "data"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    let form = models_form(s);
    cobolt_forms::save_form(&form, &root.join("forms/models-demo.cfrm")).unwrap();
    let form = cobolt_forms::load_form(&root.join("forms/models-demo.cfrm")).unwrap();
    std::fs::write(root.join("generated/models-demo.cbl"), cobolt_codegen::generate(&form)).unwrap();
    // The shell the embedded run loads it into: any application shell works, so borrow the AWS demos'.
    let shell = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/aws/forms");
    for file in ["aws-shell.cfrm", "SideMenu-1.menu.yaml"] {
        std::fs::copy(shell.join(file), root.join("forms").join(file)).unwrap();
    }
    let shell_form = cobolt_forms::load_form(&root.join("forms/aws-shell.cfrm")).unwrap();
    std::fs::write(root.join("generated/aws-shell.cbl"), cobolt_codegen::generate(&shell_form)).unwrap();
    std::fs::write(
        root.join("src/main.cbl"),
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n           GOBACK.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("modelhosts.project.toml"),
        "[project]\nname = \"ModelHosts\"\nversion = \"1.0.0\"\nmain = \"src/main.cbl\"\n\n\
         [files]\nsources = [\"src/main.cbl\"]\nforms = [\"forms/models-demo.cfrm\", \"forms/aws-shell.cfrm\"]\n\
         generated = [\"generated/models-demo.cbl\", \"generated/aws-shell.cbl\"]\n",
    )
    .unwrap();
    Project { root }
}

fn steps(open: Option<&str>) -> Value {
    let click = |id: &str| json!({"event": {"control": id, "name": "onClick"}});
    let read = |ctrl: &str| json!({"read": {"control": ctrl, "property": "LastReply"}});
    let wait = || json!({"wait_ms": 1500});
    let mut s = Vec::new();
    if let Some(form) = open {
        s.push(json!({"open_form": form}));
    }
    s.extend([click("BTN-ENTRY"), wait(), read("AGT-1"), click("BTN-CONFIG"), wait(), read("AGT-1"), click("BTN-OWN"), wait(), read("AGT-2")]);
    Value::Array(s)
}

fn readings(stdout: &str) -> Vec<String> {
    let report: Value = stdout
        .lines()
        .find_map(|l| l.strip_prefix("@RUN-FORM-RESULT "))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_else(|| panic!("no result line in:\n{stdout}"));
    report["steps"]
        .as_array()
        .map(|steps| steps.iter().filter_map(|st| st["value"].as_str().map(|v| v.trim().to_owned())).collect())
        .unwrap_or_default()
}

const WANT: [&str; 3] = ["from the entry", "from the configuration", "from the agent's own settings"];

fn run_form(p: &Project, s: &Servers, form: &str, program: &str, script: &Value) -> String {
    let script_path = p.root.join(format!("steps-{}.json", form.replace('/', "-")));
    std::fs::write(&script_path, script.to_string()).unwrap();
    let out = Command::new(rcrun())
        .args(["run-form", form, program, "--designer", "--headless", script_path.to_str().unwrap(), "--headless-limit", "60"])
        .env(AGENT_PROVIDERS_ENV, &s.providers)
        .current_dir(&p.root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "rcrun run-form {form} failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    stdout
}

/// The replies, and how many questions each server got, and what the entry's carried.
fn judge(host: &str, got: &[String], s: &Servers) {
    let (e, c, o) = (s.entry.lock().unwrap().clone(), s.config.lock().unwrap().clone(), s.own.lock().unwrap().clone());
    assert_eq!(got.len(), 3, "{host}: three readings, got {got:?}");
    println!("\n  076 AC9/AC12 — {host}");
    for (what, g, want) in [("AGT-1 with the entry", &got[0], WANT[0]), ("AGT-1, entry cleared", &got[1], WANT[1]), ("AGT-2, own settings", &got[2], WANT[2])] {
        println!("    {what:<24} {g:<32} {}", if g == want { "ok" } else { "DIFFERS" });
    }
    println!("    requests: entry {}, Configuration {}, own settings {}", e.len(), c.len(), o.len());
    assert_eq!(got, WANT, "{host}");
    assert_eq!((e.len(), c.len(), o.len()), (1, 1, 1), "{host}: one question to each server");
    assert!(e[0].contains("\"entry-model\""), "{host}: the entry's model is the one asked for: {}", e[0]);
    assert!(c[0].contains("\"own-model\""), "{host}: the Configuration keeps the agent's own model: {}", c[0]);
}

/// AC9 on `rcrun run-form` and as an embedded child form: the entry wins over the `Configuration` and the
/// agent's own URL; with the entry cleared the `Configuration` wins over the agent's own URL; with
/// neither, the agent's own settings. AC12: both hosts give the same answers.
#[test]
fn an_entry_beats_the_configuration_which_beats_the_agents_own_settings_on_two_hosts() {
    let s = servers();
    let p = project("rf", &s);
    let direct = readings(&run_form(&p, &s, "forms/models-demo.cfrm", "generated/models-demo.cbl", &steps(None)));
    judge("rcrun run-form", &direct, &s);

    let s2 = servers();
    let p2 = project("emb", &s2);
    let embedded = readings(&run_form(&p2, &s2, "forms/aws-shell.cfrm", "generated/aws-shell.cbl", &steps(Some("models-demo"))));
    judge("embedded child form (the shell's ContentPane)", &embedded, &s2);
}

/// The built binary gives the same answers. Builds one: `--ignored`.
#[test]
#[ignore = "builds a real binary; run with --ignored"]
fn the_same_order_in_a_built_binary() {
    let s = servers();
    let p = project("bin", &s);
    let ws = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let opts = cobolt_compiler::BuildOptions { verbose: false, workspace_root: Some(ws), debug: true, ..Default::default() };
    let built = cobolt_compiler::build_project(&p.root.join("modelhosts.project.toml"), &opts)
        .unwrap_or_else(|e| panic!("the build failed: {e}"));
    let script = p.root.join("bin-steps.json");
    std::fs::write(&script, steps(Some("models-demo")).to_string()).unwrap();
    let out = Command::new(&built.binary_path)
        .env("PRC_HEADLESS_SCRIPT", &script)
        .env("PRC_HEADLESS_LIMIT", "60")
        .env(AGENT_PROVIDERS_ENV, &s.providers)
        .current_dir(&p.root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    judge("the built binary", &readings(&stdout), &s);
}
