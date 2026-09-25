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

    // ── Documents as a folder tree (R49): make a folder, drop a document into
    //    it, refuse to delete it while full, then empty and delete it ──
    let t = Instant::now();
    let node = |label: &str, index: usize, level: usize| {
        FormEvent::new("Trv-Docs", "onNodeSelect").with_value(format!("{label}\t{index}\t{level}\t0"))
    };
    let mut s = Session::start("documents-form.cfrm");
    s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Added"));
    s.type_into("Txt-Folder", "Policies");
    s.click("Btn-NewFolder");
    // The tree is rebuilt node by node, so wait for its final shape.
    s.wait_for("Trv-Docs", "Items", |v| v == "Policies\tfolder\nleave-policy.md");
    s.wait_for("Lbl-Status", "Caption", |v| v == "Folder created: Policies.");
    assert!(data.join("folders.idx").is_file(), "the folder is recorded in data/folders.idx");

    s.events.send(node("Policies", 1, 1)).unwrap();
    let dest = s.wait_for("Drop-Docs", "DestinationFolder", |v| v.trim_end().ends_with("/Policies"));
    assert_eq!(dest.trim_end(), docs.join("Policies").display().to_string(), "a drop goes into the selected folder");
    s.wait_for("Lbl-Status", "Caption", |v| v == "New documents go into Policies.");

    // What a drop does: the file lands in the folder, and a refresh indexes it.
    std::fs::create_dir_all(docs.join("Policies")).unwrap();
    std::fs::write(docs.join("Policies/travel.md"), "# Travel\nBook trains for trips under four hours.").unwrap();
    s.click("Btn-Refresh");
    s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Added 1,"));
    s.wait_for("Trv-Docs", "Items", |v| v == "Policies\n  travel.md\nleave-policy.md");

    s.click("Btn-Delete");
    s.wait_for("Lbl-Status", "Caption", |v| v == "The folder is not empty: delete its documents first.");
    assert!(docs.join("Policies/travel.md").is_file(), "deleting a folder never deletes its documents");

    s.events.send(node("travel.md", 2, 2)).unwrap();
    s.wait_for("Lbl-Status", "Caption", |v| v == "New documents go into Policies.");
    s.click("Btn-Delete");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("removed 1"));
    // The emptied folder stays, with its own folder icon, until it is deleted.
    s.wait_for("Trv-Docs", "Items", |v| v == "Policies\tfolder\nleave-policy.md");

    s.events.send(node("Policies", 1, 1)).unwrap();
    s.wait_for("Lbl-Status", "Caption", |v| v == "New documents go into Policies.");
    s.click("Btn-Delete");
    s.wait_for("Trv-Docs", "Items", |v| v == "leave-policy.md");
    s.wait_for("Lbl-Status", "Caption", |v| v == "Folder deleted: Policies.");
    s.quit();
    report.push(format!(
        "folders:   Policies made, travel.md indexed inside it, full delete refused, emptied and deleted — {:.0} ms",
        t.elapsed().as_secs_f64() * 1000.0
    ));

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
    // ── Data files: a user's indexed file, registered by path, searched ──
    let t = Instant::now();
    let actors = root.join("actors.idx");
    let actors_cidx = root.join("actors.cidx");
    {
        use cobolt_runtime::indexed::{status, KeySpec, OpenMode};
        let mut f = cobolt_runtime::indexed_disk::DiskIndexedFile::new(
            &actors, 111, KeySpec { offset: 0, len: 9, duplicates: false }, Vec::new(),
        );
        assert_eq!(f.open(OpenMode::Output), status::OK);
        for (id, salary) in [("1", "100000"), ("2", "250000"), ("3", "100000")] {
            let mut rec = vec![b' '; 111];
            rec[0..9].copy_from_slice(format!("{id:>9}").as_bytes());
            rec[99..110].copy_from_slice(format!("{salary:>11}").as_bytes());
            assert_eq!(f.write(&rec), status::OK);
        }
        f.close();
    }
    std::fs::write(&actors_cidx, r#"<?xml version="1.0" encoding="UTF-8"?><IndexedFile name="ACTORS-FILE" finalized="true" version="1.0"><assign-path>actors.idx</assign-path><access-mode>dynamic</access-mode><record-format fixed-length="111"/><storage mode="disk" compression="false" persistence="false"/><comment><![CDATA[One row per performer and their salary]]></comment><keys><primary duplicates="false" ordering="ascending"><part field="ACTOR-ID" offset="0" length="9" encoding="bytes"/></primary></keys><fields><Field level="1" name="ACTORS-RECORD" usage="display"><Field level="5" name="ACTOR-ID" pic="9(9)" usage="display" offset="0" length="9"><comment><![CDATA[The performer number]]></comment></Field><Field level="5" name="ACTOR-SALARY" pic="9(9)V99" usage="display" offset="99" length="11"><comment><![CDATA[Annual salary]]></comment></Field></Field></fields></IndexedFile>"#).unwrap();
    let actors_before = std::fs::read(&actors).unwrap();
    let mut s = Session::start("files-form.cfrm");
    s.type_into("Txt-Data", &root.join("missing.idx").display().to_string());
    s.type_into("Txt-Cidx", &actors_cidx.display().to_string());
    s.click("Btn-Add");
    let refused = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Not added"));
    assert!(refused.contains("NOT-FOUND"), "{refused}");
    s.type_into("Txt-Data", &actors.display().to_string());
    s.type_into("Txt-Cidx", &actors_cidx.display().to_string());
    s.click("Btn-Add");
    let added = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Added"));
    assert!(added.contains("ACTORS-FILE"), "{added}");
    s.quit();
    let before = requests.lock().unwrap().len();
    let mut s = Session::start("chat-form.cfrm");
    s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("This month"));
    s.type_into("Txt-Input", "Which actors earn 100000?");
    s.click("Btn-Send");
    s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("This month: 3"));
    s.quit();
    let sent = requests.lock().unwrap()[before..].to_vec();
    let worker_tools: Vec<String> = sent
        .iter()
        .filter_map(|b| serde_json::from_str::<serde_json::Value>(b).ok())
        .filter(|v| v["model"] == "llama-test")
        .flat_map(|v| v["tools"].as_array().cloned().unwrap_or_default())
        .filter_map(|t| t["function"]["name"].as_str().map(String::from))
        .collect();
    assert!(worker_tools.iter().any(|n| n == "search_actors_file"), "the worker is offered the file: {worker_tools:?}");
    assert!(sent.iter().any(|b| b.contains("ACTORS-FILE: 3 record(s)")), "the file was searched and its records went back");
    assert_eq!(std::fs::read(&actors).unwrap(), actors_before, "the user's file is untouched");
    // R22: a file that has gone is named, and the chat still opens.
    std::fs::remove_file(&actors).unwrap();
    let mut s = Session::start("chat-form.cfrm");
    let note = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("This month"));
    s.quit();
    assert!(note.contains("Data files not usable: ACTORS-FILE (NOT-FOUND)"), "{note}");
    report.push(format!(
        "files:     missing file refused (NOT-FOUND); ACTORS-FILE added, searched by the tool worker (3 records), untouched; gone → named in the status — {:.0} ms",
        t.elapsed().as_secs_f64() * 1000.0
    ));

    // ── Prompt versions: v1 from the topic, v2 saved and active, v1 promoted back ──
    let t = Instant::now();
    let topics_text = || String::from_utf8_lossy(&std::fs::read(data.join("topics.idx")).unwrap()).into_owned();
    let mut s = Session::start("prompts-form.cfrm");
    s.wait_for("Lst-Versions", "Items", |v| v.contains("v1") && v.contains("[active]"));
    s.type_into("Txt-Prompt", "You answer HR questions in two sentences at most.");
    s.click("Btn-Save");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Saved as v2, now active"));
    assert!(topics_text().contains("two sentences at most"), "v2 is the topic's prompt now");
    s.pick("Lst-Versions", 1);
    s.click("Btn-Activate");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Press Activate again to confirm"));
    assert!(topics_text().contains("two sentences at most"), "nothing changes before the confirmation");
    s.click("Btn-Activate");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("v1 is the active prompt"));
    s.quit();
    let topics = topics_text();
    assert!(topics.contains("company's HR policies") && !topics.contains("two sentences at most"), "v1 is back");
    report.push(format!(
        "prompts:   v1 from the topic; v2 saved and active; v1 promoted back after a confirmation — {:.0} ms",
        t.elapsed().as_secs_f64() * 1000.0
    ));

    // ── Sample topics: installed from samples/, the orders file searched, removed ──
    // A copy of the shipped folder, so the run reads what ships and can prove
    // it leaves every file as it found it.
    let t = Instant::now();
    let samples = root.join("samples");
    fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let target = to.join(e.file_name());
            if e.path().is_dir() {
                copy_tree(&e.path(), &target);
            } else {
                std::fs::copy(e.path(), target).unwrap();
            }
        }
    }
    copy_tree(&project().join("samples"), &samples);
    let orders = samples.join("Orders").join("orders.idx");
    let orders_before = std::fs::read(&orders).unwrap();
    std::env::set_var("POWERCHAT_SAMPLES", &samples);
    let kb_before = std::fs::read_dir(&kb).unwrap().count();
    let mut s = Session::start("topics-form.cfrm");
    s.click("Btn-Install");
    let installed = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Sample topics installed"));
    assert!(installed.contains("3 topics, 7 documents"), "{installed}");
    s.click("Btn-Install");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("already installed"));
    s.pick("Lst-Topics", 2);
    s.click("Btn-Open");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Topic opened"));
    s.quit();
    assert_eq!(std::fs::read_dir(&kb).unwrap().count(), kb_before + 3, "one collection per sample topic");
    let before = requests.lock().unwrap().len();
    let mut s = Session::start("chat-form.cfrm");
    s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("This month"));
    s.type_into("Txt-Input", "Which orders has Northwind Studio placed?");
    s.click("Btn-Send");
    s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("This month: 4"));
    s.quit();
    let sent = requests.lock().unwrap()[before..].to_vec();
    assert!(sent.iter().any(|b| b.contains("You answer questions about customer orders")), "the sample topic's prompt");
    assert!(sent.iter().any(|b| b.contains("search_orders_file")), "the worker is offered the orders file");
    assert!(sent.iter().any(|b| b.contains("ORDERS-FILE: 12 record(s)")), "the orders file was searched");
    assert_eq!(std::fs::read(&orders).unwrap(), orders_before, "the sample file is untouched");
    let mut s = Session::start("topics-form.cfrm");
    s.click("Btn-RemoveSamples");
    let removed = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Removed"));
    assert_eq!(removed.trim(), "Removed 3 sample topic(s).");
    s.quit();
    // A deleted record's bytes may stay in the file, so read it as the program does.
    let records = |file: &str, len: usize, key: usize| -> Vec<String> {
        use cobolt_runtime::indexed::{IndexedStore, KeySpec, OpenMode, ReadDir};
        let mut f = cobolt_runtime::indexed_disk::DiskIndexedFile::new(
            data.join(file), len, KeySpec { offset: 0, len: key, duplicates: false }, Vec::new(),
        );
        assert_eq!(f.open(OpenMode::Input), "00", "{file}");
        let mut out = Vec::new();
        while let (Some(r), "00") = f.read_seq(ReadDir::Next) {
            out.push(String::from_utf8_lossy(&r).into_owned());
        }
        f.close();
        out
    };
    let topics = records("topics.idx", 1071, 16);
    assert!(topics.iter().all(|r| !r.contains("(sample)")), "the sample topics went");
    assert!(topics.iter().any(|r| r.contains("Human Resources")), "the operator's topic stayed");
    assert!(records("topic-files.idx", 549, 19).iter().all(|r| !r.contains("orders.idx")), "their files went");
    assert_eq!(std::fs::read(&orders).unwrap(), orders_before);
    report.push(format!(
        "samples:   {} ORDERS-FILE searched (12 records), untouched; removed — {:.0} ms",
        installed.trim(),
        t.elapsed().as_secs_f64() * 1000.0
    ));

    // ── Languages: each flag relabels the chat at once; other forms open in it ──
    let t = Instant::now();
    // Removing the samples cleared the current topic; open the operator's own.
    let mut s = Session::start("topics-form.cfrm");
    s.pick("Lst-Topics", 0);
    s.click("Btn-Open");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Topic opened"));
    s.quit();
    let langs = [
        ("pt", "Enviar", "Este mês:", "Nova conversa", "Criar tópico"),
        ("es", "Enviar", "Este mes:", "Nueva conversación", "Crear tema"),
        ("fr", "Envoyer", "Ce mois-ci :", "Nouvelle conversation", "Créer le sujet"),
        ("jp", "送信", "今月：", "新しい会話", "トピックを作成"),
        ("cn", "发送", "本月：", "新对话", "创建主题"),
        ("en", "Send", "This month:", "New conversation", "Create topic"),
    ];
    let mut checked = 0;
    for (code, send, month, new_conv, create) in langs {
        let mut s = Session::start("chat-form.cfrm");
        s.wait_for("Lbl-Status", "Caption", |v| !v.is_empty());
        s.click(&format!("Flag-{code}"));
        // In the order PC-RELABEL sets them: the hint, the button, the status.
        let hint = s.wait_for("Txt-Input", "HintText", |v| code == "en" || v != "Ask about this topic");
        assert!(!hint.is_empty(), "{code}: the hint");
        s.wait_for("Btn-Send", "Caption", |v| v == send);
        s.wait_for("Lbl-Status", "Caption", |v| v.starts_with(month));
        s.quit();
        let menu = std::fs::read_to_string(project().join("generated/chat-form.cbl")).unwrap();
        assert!(menu.contains(&format!("VALUE \"{new_conv}\"")), "{code}: the menu row has its label");
        let mut s = Session::start("topics-form.cfrm");
        s.wait_for("Btn-Create", "Caption", |v| v == create);
        s.quit();
        checked += 1;
    }
    report.push(format!(
        "languages: {checked} flags; the chat's button, hint and status and the Topics form follow each at once — {:.0} ms",
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
