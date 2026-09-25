// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 071 Phase 1 — PowerChat's own programs, run for real.
//!
//! The committed `generated/*.cbl` of each form runs in the interpreter with
//! its controls seeded exactly as a host seeds them, and the test plays the
//! user: it types into text boxes, picks list rows and clicks buttons, in the
//! order a person would. PowerChat's data goes to a temporary folder
//! (`POWERCHAT_DATA`), its Knowledge Base to another, keys to an in-memory key
//! store, and the model is a scripted local server. One test, run in order:
//! settings → topics → documents → chat → chat again (a conversation
//! reopened). It prints what happened and how long each step took.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};

fn project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerChat")
}

/// One running form, played like a user plays it.
struct Session {
    events: Sender<FormEvent>,
    input: Sender<StateUpdate>,
    state: Receiver<StateUpdate>,
    display: Receiver<String>,
    seen: Vec<StateUpdate>,
    handle: Option<JoinHandle<()>>,
}

impl Session {
    fn start(form_file: &str) -> Session {
        let path = project().join("forms").join(form_file);
        let form = cobolt_forms::load_form(&path).unwrap();
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(project().join("generated").join(format!("{stem}.cbl"))).unwrap();
        let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
        let program = parsed.program.expect("the committed program parses");
        let seed = cobolt_form_host::seeding::build_object_seed(&form, &form.controls, None, None);
        let (events, event_rx) = mpsc::channel::<FormEvent>();
        let (input, input_rx) = mpsc::channel::<StateUpdate>();
        let (state_tx, state) = mpsc::channel::<StateUpdate>();
        let (display_tx, display) = mpsc::channel::<String>();
        let err_tx = display_tx.clone();
        let handle = thread::spawn(move || {
            let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
            interp.set_input_channel(input_rx);
            interp.seed_objects(seed);
            if let Err(e) = interp.run() {
                let _ = err_tx.send(format!("RUN ENDED WITH ERROR: {e:?}"));
            }
        });
        Session { events, input, state, display, seen: Vec::new(), handle: Some(handle) }
    }

    fn type_into(&self, ctrl: &str, text: &str) {
        self.input.send(StateUpdate::new(ctrl, "Text", text)).unwrap();
    }

    fn pick(&self, ctrl: &str, index: usize) {
        self.input.send(StateUpdate::new(ctrl, "SelectedIndex", &index.to_string())).unwrap();
    }

    fn click(&self, ctrl: &str) {
        self.events.send(FormEvent::click(ctrl)).unwrap();
    }

    fn menu(&self, item: &str) {
        self.input.send(StateUpdate::new("SideMenu-1", "SelectedItemId", item)).unwrap();
        self.events.send(FormEvent::new("SideMenu-1", "onMenuItemClick")).unwrap();
    }

    /// Wait until `ctrl::prop` is set to a value `ok` accepts; the value.
    fn wait_for(&mut self, ctrl: &str, prop: &str, ok: impl Fn(&str) -> bool) -> String {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(u) = self.seen.iter().rev().find(|u| {
                u.ctrl_id.eq_ignore_ascii_case(ctrl) && u.prop.eq_ignore_ascii_case(prop) && ok(&u.value)
            }) {
                let v = u.value.clone();
                self.seen.clear();
                return v;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                let shown: Vec<String> = self.display.try_iter().collect();
                panic!("timed out waiting for {ctrl}::{prop}; saw {:#?}\ndisplayed: {shown:#?}", self.seen);
            }
            if let Ok(u) = self.state.recv_timeout(left.min(Duration::from_millis(200))) {
                self.seen.push(u);
            }
        }
    }

    fn quit(mut self) {
        let _ = self.events.send(FormEvent::quit());
        if let Some(h) = self.handle.take() {
            h.join().expect("the form's interpreter panicked");
        }
    }
}

/// A scripted Ollama: the first request of a question calls the Knowledge
/// Base tool when it is offered; the next answers. Every request is kept.
fn model_server() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let log = seen.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
            let mut buf = Vec::new();
            let mut chunk = [0u8; 8192];
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
            let body = String::from_utf8_lossy(&buf[head_end..]).into_owned();
            log.lock().unwrap().push(body.clone());
            let v: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
            let answered_tool = v["messages"]
                .as_array()
                .map(|m| m.iter().any(|x| x["role"] == "tool"))
                .unwrap_or(false);
            let tool = v["tools"][0]["function"]["name"].as_str().map(String::from);
            let planner = v["model"] == "planner-model";
            let reply = if planner && body.contains("Split the user") {
                serde_json::json!({
                    "message": {"role": "assistant", "content": "TASK: annual leave days\nTASK: part-time staff"},
                    "prompt_eval_count": 40, "eval_count": 9
                })
            } else if planner {
                serde_json::json!({
                    "message": {"role": "assistant", "content": "Composed: twenty working days, and part-time staff pro rata."},
                    "prompt_eval_count": 90, "eval_count": 14
                })
            } else { match (tool, answered_tool) {
                (Some(name), false) => serde_json::json!({
                    "message": {"role": "assistant", "content": "",
                                "tool_calls": [{"function": {"name": name, "arguments": {"query": "annual leave days"}}}]},
                    "prompt_eval_count": 30, "eval_count": 5
                }),
                _ => serde_json::json!({
                    "message": {"role": "assistant", "content": "Employees get twenty working days of annual leave."},
                    "prompt_eval_count": 120, "eval_count": 12
                }),
            } };
            let reply = reply.to_string();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(),
                reply
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    (format!("http://127.0.0.1:{port}/api/chat"), seen)
}

#[test]
fn powerchat_settings_topics_documents_and_chat() {
    let root = std::env::temp_dir().join(format!(
        "prc-071-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let data = root.join("data");
    let kb = root.join("KB");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&kb).unwrap();
    std::env::set_var("POWERCHAT_DATA", &data);
    cobolt_runtime::key_store::set_key_store(Arc::new(cobolt_runtime::key_store::MemoryKeyStore::default()));
    let (url, requests) = model_server();
    let mut report: Vec<String> = Vec::new();
    let total = Instant::now();

    // ── RAG settings: the KB folder, one model with its key, used for chat ──
    let t = Instant::now();
    let mut s = Session::start("settings-form.cfrm");
    s.type_into("Txt-KbLocation", &kb.display().to_string());
    s.click("Btn-SaveKb");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("folder saved"));
    s.type_into("Txt-Name", "local-model");
    s.type_into("Txt-Api", "Ollama");
    s.type_into("Txt-Url", &url);
    s.type_into("Txt-Model", "llama-test");
    s.type_into("Txt-Key", "sk-POWERCHAT-secret");
    s.input.send(StateUpdate::new("Chk-Tools", "Checked", "1")).unwrap();
    s.type_into("Txt-Rank", "5");
    s.click("Btn-SaveModel");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("key saved"));
    s.pick("Lst-Models", 0);
    s.type_into("Txt-Agent", "1");
    s.click("Btn-Use");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Agent 1 now uses"));
    s.quit();
    assert!(cobolt_runtime::key_store::key_store().is_set("local-model"), "the key went to the key store");
    for f in std::fs::read_dir(&data).unwrap().flatten() {
        let bytes = std::fs::read(f.path()).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains("sk-POWERCHAT-secret"),
            "{} holds the key",
            f.path().display()
        );
    }
    report.push(format!("settings:  KB folder, model local-model (key in the key store, not in data/) — {:.0} ms", t.elapsed().as_secs_f64() * 1000.0));

    // ── Topics: create one, open it ──
    let t = Instant::now();
    let mut s = Session::start("topics-form.cfrm");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("No topics yet"));
    s.type_into("Txt-Name", "Human Resources");
    s.type_into("Txt-Prompt", "You answer questions about the company's HR policies.");
    s.click("Btn-Create");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Topic created"));
    s.pick("Lst-Topics", 0);
    s.click("Btn-Open");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Topic opened"));
    s.quit();
    let collections: Vec<PathBuf> = std::fs::read_dir(&kb).unwrap().flatten().map(|e| e.path()).collect();
    assert_eq!(collections.len(), 1, "the topic's collection was created: {collections:?}");
    let docs = collections[0].join("documents");
    assert!(docs.is_dir());
    report.push(format!(
        "topics:    'Human Resources' created, collection {} made, opened — {:.0} ms",
        collections[0].file_name().unwrap().to_string_lossy(),
        t.elapsed().as_secs_f64() * 1000.0
    ));

    // ── Documents: a policy copied into the folder is indexed on open ──
    let t = Instant::now();
    std::fs::write(docs.join("leave-policy.md"), "# Annual leave\nEmployees get twenty working days of annual leave a year.\n\n# Expenses\nReceipts within thirty days.").unwrap();
    let mut s = Session::start("documents-form.cfrm");
    let status = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Added"));
    assert!(status.starts_with("Added 1,"), "{status}");
    let count = s.wait_for("Lbl-Count", "Caption", |v| v.contains("document"));
    assert_eq!(count.trim(), "1 document(s).");
    s.quit();
    report.push(format!("documents: leave-policy.md indexed on open ({status}) — {:.0} ms", t.elapsed().as_secs_f64() * 1000.0));

    // ── Chat: one question, grounded in the topic's Knowledge Base ──
    let t = Instant::now();
    let mut s = Session::start("chat-form.cfrm");
    let topic = s.wait_for("Lbl-Topic", "Caption", |v| !v.trim().is_empty());
    assert_eq!(topic.trim(), "Human Resources");
    s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("This month: 0"));
    s.type_into("Txt-Input", "How many days of annual leave do I get?");
    s.click("Btn-Send");
    let status = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("This month: 1"));
    s.quit();
    let sent = requests.lock().unwrap().clone();
    assert_eq!(sent.len(), 2, "a tool round and an answer round");
    assert!(sent[0].contains("company's HR policies"), "the topic's system prompt");
    assert!(sent[0].contains("User: How many days of annual leave"), "{}", sent[0]);
    assert!(sent[0].contains("\"llama-test\""), "the model from the settings entry");
    assert!(sent[1].contains("twenty working days"), "the KB passage went back to the model");
    assert!(status.contains("1 conversation(s), 150 input and 17 output tokens"), "{status}");
    report.push(format!("chat:      2 requests (KB tool, answer); {status} — {:.0} ms", t.elapsed().as_secs_f64() * 1000.0));

    // ── Chat again: the conversation is listed, reopened and continued ──
    let t = Instant::now();
    let conv_id = {
        // The one conversation's id, from the sidebar row the form adds.
        let mut s = Session::start("chat-form.cfrm");
        let _ = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("This month"));
        s.quit();
        let convs = std::fs::read(data.join("convs.idx")).unwrap();
        let text = String::from_utf8_lossy(&convs).into_owned();
        let at = text.find("How many days").expect("the conversation is titled by its question");
        text[at - 32..at - 16].to_string()
    };
    let mut s = Session::start("chat-form.cfrm");
    let _ = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("This month"));
    s.menu(&format!("c{conv_id}"));
    let _ = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("This month"));
    s.type_into("Txt-Input", "And for part-time staff?");
    s.click("Btn-Send");
    let status = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("This month") && !v.contains("150 input"));
    s.quit();
    let sent = requests.lock().unwrap().clone();
    let continued = &sent[2];
    assert!(continued.contains("User: How many days of annual leave"), "the earlier question is sent again (R41): conv {conv_id} request {continued}");
    assert!(continued.contains("Assistant: Employees get twenty working days"), "and the earlier answer");
    assert!(continued.contains("User: And for part-time staff?"));
    assert!(status.starts_with("This month: 1 conversation(s)"), "the same conversation continued: {status}");
    report.push(format!("reopen:    conversation {conv_id} reloaded and continued with its history — {:.0} ms", t.elapsed().as_secs_f64() * 1000.0));

    // ── The mesh: a planner on agent 2 orchestrates, agent 1 does the tool work ──
    let t = Instant::now();
    let mut s = Session::start("settings-form.cfrm");
    s.type_into("Txt-Name", "planner");
    s.type_into("Txt-Api", "Ollama");
    s.type_into("Txt-Url", &url);
    s.type_into("Txt-Model", "planner-model");
    s.input.send(StateUpdate::new("Chk-Tools", "Checked", "0")).unwrap();
    s.type_into("Txt-Rank", "9");
    s.type_into("Txt-Key", "");
    s.click("Btn-SaveModel");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Model saved"));
    s.pick("Lst-Models", 1);
    s.type_into("Txt-Agent", "2");
    s.click("Btn-Use");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Agent 2 now uses"));
    s.quit();
    let before = requests.lock().unwrap().len();
    let mut s = Session::start("chat-form.cfrm");
    let elected = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("This month"));
    assert!(elected.contains("Orchestrator: agent 2, tools: agent 1."), "R55/R56: {elected}");
    s.type_into("Txt-Input", "How much leave, and what about part-time staff?");
    s.click("Btn-Send");
    let status = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("This month: 2"));
    s.quit();
    let sent = requests.lock().unwrap()[before..].to_vec();
    let body = |i: usize| -> serde_json::Value { serde_json::from_str(&sent[i]).unwrap() };
    assert_eq!(sent.len(), 4, "plan, the worker's tool round and answer, compose: {sent:#?}");
    assert_eq!(body(0)["model"], "planner-model");
    assert!(sent[0].contains("company's HR policies"), "the orchestrator has the topic's prompt (R63)");
    assert!(body(0)["tools"].is_null(), "the planner calls no tools");
    assert_eq!(body(1)["model"], "llama-test");
    assert!(sent[1].contains("careful research assistant"), "the worker keeps its role prompt (R64)");
    assert!(!sent[1].contains("company's HR policies"));
    assert!(sent[1].contains("annual leave days") && sent[1].contains("part-time staff"), "both tasks");
    assert!(sent[3].contains("Your assistants reported") && sent[3].contains("twenty working days"), "R60");
    let turns = String::from_utf8_lossy(&std::fs::read(data.join("turns.idx")).unwrap()).into_owned();
    assert!(turns.contains("Composed: twenty working days"), "the composed answer is the turn kept (R61)");
    report.push(format!(
        "mesh:      agent 2 orchestrates, agent 1 does tools; plan → 2 tasks → worker (KB) → composed; {} — {:.0} ms",
        status.trim(),
        t.elapsed().as_secs_f64() * 1000.0
    ));
    let sent = requests.lock().unwrap().clone();

    println!("\n  ── 071 PowerChat, played end to end ─────────────────────");
    for r in &report {
        println!("  {r}");
    }
    println!("  total {:.0} ms, {} model requests\n", total.elapsed().as_secs_f64() * 1000.0, sent.len());
    let _ = std::fs::remove_dir_all(&root);
}
