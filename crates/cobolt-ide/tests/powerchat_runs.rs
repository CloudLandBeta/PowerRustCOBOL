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
    /// The forms the program opened with `OpenFormSync`, by id — each one
    /// answered at once, as if the operator had closed it.
    opened: Receiver<String>,
}

impl Session {
    fn start(form_file: &str) -> Session {
        Session::launch(form_file, None, None)
    }

    /// Like [`Session::start`], with the designed form changed first — the
    /// way a test runs a control under a setting the shipped form does not use.
    fn start_tweaked(form_file: &str, tweak: fn(&mut cobolt_forms::Form)) -> Session {
        Session::launch(form_file, None, Some(tweak))
    }

    /// Like [`Session::start`], with a stand-in form host that answers the
    /// program's native file dialogs from `answers`, in order (`None` = the
    /// operator cancelled).
    fn start_with_dialogs(form_file: &str, answers: Vec<Option<String>>) -> Session {
        Session::launch(form_file, Some(answers), None)
    }

    fn launch(
        form_file: &str,
        dialogs: Option<Vec<Option<String>>>,
        tweak: Option<fn(&mut cobolt_forms::Form)>,
    ) -> Session {
        let path = project().join("forms").join(form_file);
        let mut form = cobolt_forms::load_form(&path).unwrap();
        if let Some(tweak) = tweak {
            tweak(&mut form);
        }
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(project().join("generated").join(format!("{stem}.cbl"))).unwrap();
        let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
        let program = parsed.program.expect("the committed program parses");
        let seed = cobolt_form_host::seeding::build_object_seed(&form, &form.controls, None, None);
        // Every host hands the interpreter each SideMenu's designed rows.
        let menus: Vec<(String, cobolt_forms::menu::MenuDefinition)> = form
            .controls
            .iter()
            .filter(|c| c.control_type == cobolt_forms::ControlType::SideMenu)
            .filter_map(|c| {
                cobolt_forms::menu::load_menu(&project().join("forms").join(format!("{}.menu.yaml", c.id)))
                    .ok()
                    .map(|d| (c.id.clone(), d))
            })
            .collect();
        let (events, event_rx) = mpsc::channel::<FormEvent>();
        let (input, input_rx) = mpsc::channel::<StateUpdate>();
        let (state_tx, state) = mpsc::channel::<StateUpdate>();
        let (display_tx, display) = mpsc::channel::<String>();
        let (opened_tx, opened) = mpsc::channel::<String>();
        let err_tx = display_tx.clone();
        let handle = thread::spawn(move || {
            let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
            interp.set_input_channel(input_rx);
            interp.seed_objects(seed);
            for (id, def) in &menus {
                interp.set_designed_menu(id, def);
            }
            let mut _closed = None;
            {
                use cobolt_runtime::form_host::{FormRequest, ROOT_HANDLE};
                let (req_tx, req_rx) = mpsc::channel::<FormRequest>();
                let (closed_tx, closed_rx) = mpsc::channel::<String>();
                _closed = Some(closed_tx);
                interp.set_form_host(req_tx, ROOT_HANDLE, "FORM", closed_rx);
                thread::spawn(move || {
                    let mut answers = dialogs.unwrap_or_default().into_iter();
                    while let Ok(req) = req_rx.recv() {
                        match req {
                            FormRequest::FileDialog { reply, .. } => {
                                let _ = reply.send(answers.next().flatten());
                            }
                            FormRequest::OpenForm { form_id, reply, .. } => {
                                let _ = opened_tx.send(form_id);
                                let _ = reply.send(None);
                            }
                            _ => {}
                        }
                    }
                });
            }
            if let Err(e) = interp.run() {
                let _ = err_tx.send(format!("RUN ENDED WITH ERROR: {e:?}"));
            }
        });
        Session { events, input, state, display, seen: Vec::new(), handle: Some(handle), opened }
    }

    fn type_into(&self, ctrl: &str, text: &str) {
        self.input.send(StateUpdate::new(ctrl, "Text", text)).unwrap();
    }

    fn pick(&self, ctrl: &str, index: usize) {
        self.input.send(StateUpdate::new(ctrl, "SelectedIndex", &index.to_string())).unwrap();
    }

    /// Pick a combo row the way a person does: the selection, then the
    /// control's own change event.
    fn choose(&self, ctrl: &str, index: usize) {
        self.pick(ctrl, index);
        self.events.send(FormEvent::new(ctrl, "onSelectedIndexChanged")).unwrap();
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

    /// Everything the form sets until it falls quiet, as (control, property)
    /// → last value, both upper-cased.
    fn settle(&mut self) -> std::collections::HashMap<(String, String), String> {
        while let Ok(u) = self.state.recv_timeout(Duration::from_millis(500)) {
            self.seen.push(u);
        }
        let mut out = std::collections::HashMap::new();
        for u in self.seen.drain(..) {
            out.insert((u.ctrl_id.to_ascii_uppercase(), u.prop.to_ascii_uppercase()), u.value);
        }
        out
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
            // The model list (a connection test, and the Model selection
            // dialog opening): answered, and kept out of the chat requests.
            if head.starts_with("get ") && head.contains("/api/tags") {
                let reply = r#"{"models":[{"name":"llama-test"},{"name":"planner-model"}]}"#;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    reply.len(),
                    reply
                );
                let _ = stream.write_all(resp.as_bytes());
                continue;
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

/// `POWERCHAT_DATA` is one per process and every test here sets it: they take
/// turns, or one test's forms open the other's data.
static POWERCHAT_DATA_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn powerchat_settings_topics_documents_and_chat() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = std::env::temp_dir().join(format!(
        "prc-071-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let data = root.join("data");
    let kb = root.join("KB");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&kb).unwrap();
    plant_model(&root, false);
    std::env::set_var("POWERCHAT_DATA", &data);
    cobolt_runtime::key_store::set_key_store(Arc::new(cobolt_runtime::key_store::MemoryKeyStore::default()));
    let (url, requests) = model_server();
    let mut report: Vec<String> = Vec::new();
    let total = Instant::now();

    // ── First run: no model yet → the menu is shut but for Chat and RAG
    //    settings, and the welcome screen is up (operator, 2026-09-25) ──
    let t = Instant::now();
    let menu_state = |v: &std::collections::HashMap<(String, String), String>| -> Vec<(String, bool)> {
        let rows = cobolt_forms::menu::runtime::parse_rows(
            v.get(&("SIDEMENU-1".to_string(), "RUNTIMEROWS".to_string())).map(String::as_str).unwrap_or(""),
        );
        rows.iter().filter(|r| r.overlay).map(|r| (r.item.id.clone(), r.item.enabled)).collect()
    };
    let shown = |v: &std::collections::HashMap<(String, String), String>, ctrl: &str| -> String {
        v.get(&(ctrl.to_string(), "VISIBLE".to_string())).cloned().unwrap_or_default()
    };
    let mut s = Session::start("chat-form.cfrm");
    let v = s.settle();
    s.quit();
    let state = menu_state(&v);
    for id in ["newc", "tpcs", "docs", "fils", "prmt"] {
        assert!(state.contains(&(id.to_string(), false)), "{id} is shut before a model is set: {state:?}");
    }
    assert!(!state.iter().any(|(id, on)| (id == "sett" || id == "chat") && !on), "RAG settings and Chat stay open");
    assert_eq!(shown(&v, "POWERCHAT"), "true", "the welcome screen is up");
    assert_eq!(shown(&v, "PIC-ROBOT"), "true");
    assert_eq!(shown(&v, "VWR-CHAT"), "false", "the chat waits");
    report.push(format!(
        "first run: menu shut but for Chat and RAG settings; welcome screen shown — {:.0} ms",
        t.elapsed().as_secs_f64() * 1000.0
    ));

    // ── RAG settings: a summary whose four groups open modal dialogs
    //    (operator, 2026-09-25). Before any provider, Model selection and
    //    Agents are off ──
    let t = Instant::now();
    let get = |v: &std::collections::HashMap<(String, String), String>, ctrl: &str, prop: &str| -> String {
        v.get(&(ctrl.to_string(), prop.to_string())).cloned().unwrap_or_default()
    };
    let mut s = Session::start("settings-form.cfrm");
    let v = s.settle();
    s.quit();
    assert_eq!(get(&v, "BTN-MODELSEL", "ENABLED"), "false", "Model selection waits for a provider");
    assert_eq!(get(&v, "BTN-AGENTS", "ENABLED"), "false", "Agents waits for a provider");
    assert_eq!(get(&v, "LBL-PROVSUM", "CAPTION"), "No connection yet: start here.");
    // Knowledge Base folder.
    let mut s = Session::start("kb-folder-form.cfrm");
    s.settle();
    s.type_into("Txt-KbLocation", &kb.display().to_string());
    s.click("Btn-Save");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("folder saved"));
    s.quit();
    // Model providers: a new connection, tested there, saved with its key.
    let mut s = Session::start("providers-form.cfrm");
    s.settle();
    s.type_into("Txt-Name", "local-model");
    s.pick("Cmb-Provider", 14); // Ollama (Local), the IDE's 15th provider
    s.type_into("Txt-Url", &url);
    s.type_into("Txt-Key", "sk-POWERCHAT-secret");
    s.click("Btn-Test");
    let tested = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Connection OK"));
    assert_eq!(tested.trim(), "Connection OK: 2 models available.", "the test lists the provider's models");
    s.click("Btn-Save");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("key saved"));
    s.quit();
    // Model selection: opening connects and lists the models by itself.
    let mut s = Session::start("model-form.cfrm");
    let listed = s.wait_for("Lbl-Status", "Caption", |v| v.contains("models available"));
    assert_eq!(listed.trim(), "2 models available.", "the dialog lists the models as it opens");
    s.input.send(StateUpdate::new("Cmb-Model", "Value", "llama-test")).unwrap();
    s.input.send(StateUpdate::new("Chk-Tools", "Checked", "1")).unwrap();
    s.type_into("Txt-Rank", "5");
    s.click("Btn-Save");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Model saved"));
    s.quit();
    // Agents: agent 1 on local-model.
    let mut s = Session::start("agents-form.cfrm");
    s.settle();
    s.pick("Cmb-Agent-1", 1);
    s.click("Btn-Save");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Agents saved"));
    s.quit();
    // Back on the summary: the two groups are open, and say what they hold.
    let mut s = Session::start("settings-form.cfrm");
    let v = s.settle();
    s.quit();
    assert_eq!(get(&v, "BTN-MODELSEL", "ENABLED"), "true", "a provider opens Model selection");
    assert_eq!(get(&v, "BTN-AGENTS", "ENABLED"), "true", "and Agents");
    assert_eq!(get(&v, "LBL-PROVSUM", "CAPTION"), "1 connection(s)");
    assert_eq!(get(&v, "LBL-MODELSUM", "CAPTION"), "1 of 1 connection(s) have a model");
    assert_eq!(get(&v, "LBL-AGENTSUM", "CAPTION"), "1 of 3 agents on");
    assert_eq!(get(&v, "LBL-KBSUM", "CAPTION").trim(), kb.display().to_string());
    assert!(cobolt_runtime::key_store::key_store().is_set("local-model"), "the key went to the key store");
    for f in std::fs::read_dir(&data).unwrap().flatten() {
        let bytes = std::fs::read(f.path()).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains("sk-POWERCHAT-secret"),
            "{} holds the key",
            f.path().display()
        );
    }
    report.push(format!(
        "settings:  4 dialogs — KB folder; connection local-model tested (2 models) and saved, key in the key store, not in data/; model listed on opening and saved; agent 1 on it; Model selection and Agents off until then — {:.0} ms",
        t.elapsed().as_secs_f64() * 1000.0
    ));

    // ── Configured: the chat opens its menu and puts the welcome away ──
    let t = Instant::now();
    let mut s = Session::start("chat-form.cfrm");
    let v = s.settle();
    s.quit();
    let state = menu_state(&v);
    for id in ["newc", "tpcs", "docs", "fils", "prmt"] {
        assert!(state.contains(&(id.to_string(), true)), "{id} opens once an agent has a model: {state:?}");
    }
    assert_eq!(shown(&v, "POWERCHAT"), "false", "the welcome screen goes");
    assert_eq!(shown(&v, "VWR-CHAT"), "true", "the chat is back");
    report.push(format!("configured: menu open, welcome screen gone — {:.0} ms", t.elapsed().as_secs_f64() * 1000.0));

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
    let mut s = Session::start("providers-form.cfrm");
    s.settle();
    s.choose("Cmb-Conn", 0); // (new connection)
    s.settle();
    s.type_into("Txt-Name", "planner");
    s.pick("Cmb-Provider", 14); // Ollama (Local), the IDE's 15th provider
    s.type_into("Txt-Url", &url);
    s.type_into("Txt-Key", "");
    s.click("Btn-Save");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Connection saved"));
    s.quit();
    let mut s = Session::start("model-form.cfrm");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("models available"));
    s.choose("Cmb-Conn", 1); // planner, after local-model
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("models available"));
    s.input.send(StateUpdate::new("Cmb-Model", "Value", "planner-model")).unwrap();
    s.input.send(StateUpdate::new("Chk-Tools", "Checked", "0")).unwrap();
    s.type_into("Txt-Rank", "9");
    s.click("Btn-Save");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Model saved"));
    s.quit();
    let mut s = Session::start("agents-form.cfrm");
    s.settle();
    s.pick("Cmb-Agent-2", 2); // planner
    s.click("Btn-Save");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Agents saved"));
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
    // A form already on the pane follows a flag too: nothing tells an
    // occupant, so its Tmr-Lang looks, and re-labels it on the next tick.
    let mut pane = Session::start("settings-form.cfrm");
    pane.wait_for("Lbl-Title", "Caption", |v| v == "RAG settings");
    for (code, send, title) in [("es", "Enviar", "Configuración de RAG"), ("en", "Send", "RAG settings")] {
        let mut s = Session::start("chat-form.cfrm");
        s.wait_for("Lbl-Status", "Caption", |v| !v.is_empty());
        s.click(&format!("Flag-{code}"));
        s.wait_for("Btn-Send", "Caption", |v| v == send);
        s.quit();
        pane.events.send(FormEvent::new("Tmr-Lang", "onTick")).unwrap();
        pane.wait_for("Lbl-Title", "Caption", |v| v == title);
    }
    pane.quit();
    report.push(format!(
        "languages: {checked} flags; the chat's button, hint and status and the Topics form follow each at once; RAG settings, already open, follows on its timer's tick — {:.0} ms",
        t.elapsed().as_secs_f64() * 1000.0
    ));

    // ── RAG settings, the IDE's way: browse for the KB folder in its
    //    dialog, export to XML and import it back (keys never travel) ──
    let t = Instant::now();
    let xml = root.join("rag-settings.xml");
    let xml2 = root.join("rag-settings-2.xml");
    let mut s = Session::start_with_dialogs("kb-folder-form.cfrm", vec![Some(kb.display().to_string())]);
    s.settle();
    s.click("Btn-BrowseKb");
    let folder = s.wait_for("Txt-KbLocation", "Text", |v| v.trim() == kb.display().to_string());
    assert_eq!(folder.trim(), kb.display().to_string());
    s.click("Btn-Save");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("folder saved"));
    s.quit();
    let mut s = Session::start_with_dialogs(
        "settings-form.cfrm",
        vec![Some(xml.display().to_string()), Some(xml2.display().to_string())],
    );
    s.click("Btn-Export");
    let exported = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Settings exported"));
    let text = std::fs::read_to_string(&xml).unwrap();
    assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"), "{text}");
    assert!(text.contains("<rag-settings application=\"PowerChat\" version=\"1\">"), "{text}");
    assert!(text.contains("<model name=\"local-model\" provider=\"ollama\""), "{text}");
    assert!(text.contains("model=\"llama-test\" tools=\"Y\" rank=\"5\"/>"), "{text}");
    assert!(text.contains("<agent number=\"1\" model=\"local-model\"/>"), "{text}");
    assert!(text.contains(&format!("<knowledge-base folder=\"{}\"/>", kb.display())), "{text}");
    assert!(!text.contains("sk-POWERCHAT-secret"), "a key never goes into the file");
    // The same file with one more model — hosted, so it needs a key, and
    // named with an ampersand, so the escaping is exercised both ways.
    let more = text.replace(
        "  </models>",
        "    <model name=\"R&amp;D-cloud\" provider=\"openai\" endpoint=\"https://api.openai.com/v1\" model=\"gpt-4o\" tools=\"Y\" rank=\"7\"/>\n  </models>",
    );
    std::fs::write(&xml2, more).unwrap();
    s.click("Btn-Import");
    let imported = s.wait_for("Lbl-Status", "Caption", |v| v.contains("imported"));
    assert!(imported.starts_with("3 models imported."), "{imported}");
    assert!(imported.contains("Set the API key of: R&D-cloud"), "{imported}");
    s.quit();
    report.push(format!(
        "rag xml:   KB folder browsed in its dialog; exported ({} bytes, no key); imported 3 models, R&D-cloud named for its key — {:.0} ms",
        text.len(),
        t.elapsed().as_secs_f64() * 1000.0
    ));
    let _ = exported;

    let sent = requests.lock().unwrap().clone();

    println!("\n  ── 071 PowerChat, played end to end ─────────────────────");
    for r in &report {
        println!("  {r}");
    }
    println!("  total {:.0} ms, {} model requests\n", total.elapsed().as_secs_f64() * 1000.0, sent.len());
    let _ = std::fs::remove_dir_all(&root);
}

/// PowerChat's Knowledge Base embeds with the built-in model, fetched into
/// `<app>/assets/models` when Documents opens. A test must never download
/// 470 MB: the application's folder is pointed at `root`, and the model is
/// planted there — the REAL one, linked from the IDE's cache, when `real` and
/// the cache exists; otherwise stand-in files, which make the fetch a no-op
/// and leave the model unloadable, so indexing is by words only (R18).
/// Answers whether the real model was planted.
fn plant_model(root: &Path, real: bool) -> bool {
    cobolt_forms::assets::set_base(root);
    let dir = root.join("assets/models/multilingual-e5-small");
    std::fs::create_dir_all(&dir).unwrap();
    let files = ["config.json", "tokenizer.json", "model.safetensors"];
    let cache = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join("PowerRustCOBOL/data/models/multilingual-e5-small"));
    if let Some(cache) = cache.filter(|c| real && files.iter().all(|f| c.join(f).is_file())) {
        for f in files {
            std::os::unix::fs::symlink(cache.join(f), dir.join(f)).unwrap();
        }
        return true;
    }
    for f in files {
        std::fs::write(dir.join(f), b"not a model").unwrap();
    }
    false
}

/// Seed PowerChat's settings file the way the RAG settings and Topics forms
/// leave it: the Knowledge Base folder and the open topic.
fn seed_settings(data: &Path, pairs: &[(&str, &str)]) {
    let mut writes = String::new();
    for (name, value) in pairs {
        writes.push_str(&format!(
            "           MOVE \"{name}\" TO SET-NAME\n           MOVE \"{value}\" TO SET-VALUE\n           WRITE SETTINGS-REC\n"
        ));
    }
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. SEED.\n       ENVIRONMENT DIVISION.\n       INPUT-OUTPUT SECTION.\n       FILE-CONTROL.\n           SELECT SETTINGS-FILE ASSIGN TO \"{}\"\n               ORGANIZATION IS INDEXED\n               ACCESS MODE IS DYNAMIC\n               RECORD KEY IS SET-NAME\n               FILE STATUS IS WS-FS\n               STORAGE MODE IS DISK.\n       DATA DIVISION.\n       FILE SECTION.\n       FD  SETTINGS-FILE.\n       01  SETTINGS-REC.\n           05 SET-NAME PIC X(20).\n           05 SET-VALUE PIC X(200).\n       WORKING-STORAGE SECTION.\n       01 WS-FS PIC XX.\n       PROCEDURE DIVISION.\n           OPEN OUTPUT SETTINGS-FILE\n{writes}           CLOSE SETTINGS-FILE\n           STOP RUN.\n",
        data.join("settings.idx").display()
    );
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
    let program = parsed.program.expect("the seeding program parses");
    Interpreter::new(program).run().expect("the settings are seeded");
}

/// Operator (2026-09-27): "make possible to move files between folders". A
/// document is picked with Move, placed with Move here (or To the top level);
/// the copy is indexed before the original is deleted, and a move onto the
/// folder it is already in, or onto a document of the same name, changes
/// nothing.
#[test]
fn powerchat_moves_a_document_between_folders() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = std::env::temp_dir().join(format!(
        "prc-071-move-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let data = root.join("data");
    let kb = root.join("KB");
    let docs = kb.join("HR").join("documents");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&docs).unwrap();
    plant_model(&root, false);
    std::fs::write(docs.join("leave-policy.md"), "# Leave\nTwenty working days a year.").unwrap();
    std::fs::write(docs.join("travel.md"), "# Travel\nBook trains under four hours.").unwrap();
    std::env::set_var("POWERCHAT_DATA", &data);
    seed_settings(&data, &[("CUR-TOPIC", "HR"), ("KB-LOCATION", &kb.display().to_string())]);
    let node = |label: &str, index: usize, level: usize| {
        FormEvent::new("Trv-Docs", "onNodeSelect").with_value(format!("{label}\t{index}\t{level}\t0"))
    };
    let total = Instant::now();
    let mut report: Vec<String> = Vec::new();

    let mut s = Session::start("documents-form.cfrm");
    s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Added 2,"));
    s.type_into("Txt-Folder", "Policies");
    s.click("Btn-NewFolder");
    s.wait_for("Trv-Docs", "Items", |v| v == "Policies\tfolder\nleave-policy.md\ntravel.md");

    // Nothing picked: Move asks for a document; a folder is not one.
    s.events.send(node("Policies", 1, 1)).unwrap();
    s.click("Btn-Move");
    s.wait_for("Lbl-Status", "Caption", |v| v == "Pick the document to move in the tree first.");

    // Into a folder.
    let t = Instant::now();
    s.events.send(node("travel.md", 3, 1)).unwrap();
    s.click("Btn-Move");
    // The button changes first, then the status says what to do next.
    s.wait_for("Btn-Move", "Caption", |v| v == "Move here");
    s.wait_for("Lbl-Status", "Caption", |v| v == "Moving travel.md: pick a folder, then press Move here.");
    s.events.send(node("Policies", 1, 1)).unwrap();
    s.wait_for("Lbl-Status", "Caption", |v| v == "Move here puts travel.md into Policies.");
    s.click("Btn-Move");
    // The tree is rebuilt, then the status reports the move.
    s.wait_for("Trv-Docs", "Items", |v| v == "Policies\n  travel.md\nleave-policy.md");
    s.wait_for("Lbl-Status", "Caption", |v| v == "Moved travel.md to Policies.");
    assert!(docs.join("Policies/travel.md").is_file(), "the document is in its new folder");
    assert!(!docs.join("travel.md").exists(), "and no longer where it was");
    report.push(format!("travel.md → Policies (copied, indexed, original deleted) — {:.0} ms", t.elapsed().as_secs_f64() * 1000.0));

    // Back to the top level.
    let t = Instant::now();
    s.events.send(node("travel.md", 2, 2)).unwrap();
    s.click("Btn-Move");
    s.wait_for("Btn-Move", "Caption", |v| v == "Move here");
    s.click("Btn-MoveTop");
    s.wait_for("Trv-Docs", "Items", |v| v == "Policies\tfolder\nleave-policy.md\ntravel.md");
    s.wait_for("Lbl-Status", "Caption", |v| v == "Moved Policies/travel.md to the top level.");
    assert!(docs.join("travel.md").is_file() && !docs.join("Policies/travel.md").exists());
    report.push(format!("Policies/travel.md → top level — {:.0} ms", t.elapsed().as_secs_f64() * 1000.0));

    // Onto the folder it is already in: nothing moves.
    s.events.send(node("travel.md", 3, 1)).unwrap();
    s.click("Btn-Move");
    s.wait_for("Btn-Move", "Caption", |v| v == "Move here");
    s.click("Btn-Move");
    s.wait_for("Btn-Move", "Caption", |v| v == "Move");
    s.wait_for("Lbl-Status", "Caption", |v| v == "The document is already in that folder.");
    report.push("travel.md → its own folder: refused, nothing moved".to_string());

    // Onto a document of the same name: nothing is overwritten.
    std::fs::create_dir_all(docs.join("Policies")).unwrap();
    std::fs::write(docs.join("Policies/travel.md"), "# Other\nA different travel note.").unwrap();
    s.click("Btn-Refresh");
    s.wait_for("Trv-Docs", "Items", |v| v == "Policies\n  travel.md\nleave-policy.md\ntravel.md");
    s.events.send(node("travel.md", 4, 1)).unwrap();
    s.click("Btn-Move");
    s.wait_for("Btn-Move", "Caption", |v| v == "Move here");
    s.events.send(node("Policies", 1, 1)).unwrap();
    s.click("Btn-Move");
    s.wait_for("Lbl-Status", "Caption", |v| v == "A document of that name is already there: Policies/travel.md.");
    assert_eq!(
        std::fs::read_to_string(docs.join("Policies/travel.md")).unwrap(),
        "# Other\nA different travel note.",
        "the document already there is untouched"
    );
    assert!(docs.join("travel.md").is_file(), "and the one not moved is still in place");
    report.push("travel.md → Policies holding a travel.md: refused, neither file touched".to_string());
    s.quit();

    println!("\n  ── 071 PowerChat, moving documents between folders ──────");
    for r in &report {
        println!("  {r}");
    }
    println!("  4 moves tried (2 made, 2 refused) in {:.0} ms\n", total.elapsed().as_secs_f64() * 1000.0);
    let _ = std::fs::remove_dir_all(&root);
}

/// Operator (2026-09-27): "make sure that any supported document can be
/// uploaded. Also place a label with valid extensions. Reject any extension
/// not supported, and tell the user why". The zone's filter is the Knowledge
/// Base's own list of readable types, the form shows that list, and a refused
/// file is named with the reason — still shown after the indexing summary.
#[test]
fn powerchat_documents_take_every_readable_type_and_say_why_others_are_refused() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = std::env::temp_dir().join(format!(
        "prc-071-types-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let data = root.join("data");
    let kb = root.join("KB");
    let docs = kb.join("HR").join("documents");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&docs).unwrap();
    plant_model(&root, false);
    std::env::set_var("POWERCHAT_DATA", &data);
    seed_settings(&data, &[("CUR-TOPIC", "HR"), ("KB-LOCATION", &kb.display().to_string())]);
    let t = Instant::now();

    let mut s = Session::start("documents-form.cfrm");
    // The label is written with the texts, then the zone takes the same list.
    let label = s.wait_for("Lbl-Types", "Caption", |v| v.starts_with("Accepted types:"));
    let filter = s.wait_for("Drop-Docs", "AllowedExtensions", |v| !v.trim().is_empty());

    // Every type the Knowledge Base reads is taken; nothing else is.
    let readable = [
        "a.md", "a.markdown", "a.txt", "a.text", "a.log", "a.html", "a.htm", "a.xhtml", "a.csv",
        "a.tsv", "a.tab", "a.pdf", "a.docx", "a.docm", "a.dotx", "a.dotm", "a.pptx", "a.pptm",
        "a.potx", "a.potm", "a.ppsx", "a.ppsm", "a.xlsx", "a.xlsm", "a.xltx", "a.xltm", "a.odt",
        "a.ott", "a.odm", "a.oth", "a.ods", "a.ots", "a.odp", "a.otp", "a.zip", "a.tar",
        "a.tar.gz", "a.tgz", "UPPER.PDF",
    ];
    let unreadable = ["a.png", "a.jpg", "a.exe", "a.doc", "a.ppt", "a.xls", "a.rtf", "a.mp4", "noext"];
    let judge = |p: &str| cobolt_forms::dropzone::judge(&filter, 0, p, &|_| None);
    for p in readable {
        assert!(judge(p).is_ok(), "{p} is readable, so the zone takes it (filter {filter:?})");
        let ext = p.rsplit('.').next().unwrap().to_ascii_lowercase();
        assert!(label.contains(&format!(".{ext}")), "the label lists .{ext}: {label}");
    }
    for p in unreadable {
        assert_eq!(
            judge(p),
            Err(cobolt_forms::dropzone::Rejection::Extension),
            "{p} cannot be read, so the zone refuses it"
        );
    }

    // A drop of two files, one refused: the refusal is named with its reason,
    // and it survives the indexing summary that the accepted one triggers.
    s.input
        .send(StateUpdate::new(
            "Drop-Docs",
            "RejectedFiles",
            "/Users/me/Desktop/photo.png\textension\n/Users/me/old.doc\textension",
        ))
        .unwrap();
    s.events.send(FormEvent::new("Drop-Docs", "onFilesRejected")).unwrap();
    s.wait_for("Lbl-Status", "Caption", |v| {
        v == "Not added - this type of file cannot be read: photo.png, old.doc"
    });
    std::fs::write(docs.join("policy.md"), "# Policy\nTwenty days.").unwrap();
    s.events.send(FormEvent::new("Drop-Docs", "onFilesDropped")).unwrap();
    s.wait_for("Lbl-Status", "Caption", |v| {
        v.starts_with("Added 1,") && v.ends_with("Not added - this type of file cannot be read: photo.png, old.doc")
    });
    s.quit();

    println!("\n  ── 071 PowerChat, what the Documents zone accepts ───────");
    println!("  filter set by the form: {} types", cobolt_forms::dropzone::parse_extensions(&filter).len());
    println!("  {} readable names taken, {} unreadable refused", readable.len(), unreadable.len());
    println!("  label: {label}");
    println!("  refusal named with its reason, and kept after the indexing summary — {:.0} ms\n", t.elapsed().as_secs_f64() * 1000.0);
    let _ = std::fs::remove_dir_all(&root);
}

/// Operator (2026-09-27): "Atualizar não processou o documento (não fez o
/// embedding)". PowerChat indexed by words only (`Embedder` Lexical). It now
/// embeds with the built-in model, and a collection indexed the old way is
/// rebuilt once, on its own, so its documents get vectors. Without a usable
/// model it still indexes, and says search is by words only (R18).
#[test]
fn powerchat_documents_embed_with_the_builtin_model() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = std::env::temp_dir().join(format!(
        "prc-071-embed-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let data = root.join("data");
    let kb = root.join("KB");
    let docs = kb.join("HR").join("documents");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("leave.md"), "# Annual leave\nEmployees get twenty working days of paid leave a year.").unwrap();
    std::env::set_var("POWERCHAT_DATA", &data);
    seed_settings(&data, &[("CUR-TOPIC", "HR"), ("KB-LOCATION", &kb.display().to_string())]);
    let mut report = Vec::new();

    // No usable model: indexed all the same, and the status says how.
    let t = Instant::now();
    plant_model(&root, false);
    let mut s = Session::start("documents-form.cfrm");
    let status = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Added"));
    s.quit();
    assert!(status.starts_with("Added 1,"), "{status}");
    assert!(status.contains("Search is by words only:"), "R18 — never degrade silently: {status}");
    report.push(format!("no model: indexed, and says so ({status}) — {:.0} ms", t.elapsed().as_secs_f64() * 1000.0));

    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("leave.md"), "# Annual leave\nEmployees get twenty working days of paid leave a year.").unwrap();
    seed_settings(&data, &[("CUR-TOPIC", "HR"), ("KB-LOCATION", &kb.display().to_string())]);
    if !plant_model(&root, true) {
        println!("built-in model not cached on this machine — the embedding half is skipped");
        let _ = std::fs::remove_dir_all(&root);
        return;
    }

    // The collection as PowerChat left it before: indexed by words.
    let t = Instant::now();
    let mut s = Session::start_tweaked("documents-form.cfrm", |f| {
        let kb = f.controls.iter_mut().find(|c| c.id == "KB-D").unwrap();
        kb.set_prop("Embedder", cobolt_forms::PropValue::String("Lexical".into()));
    });
    let status = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Added"));
    s.quit();
    assert!(status.starts_with("Added 1,") && !status.contains("words only"), "{status}");
    report.push(format!("lexical index made the old way — {:.0} ms", t.elapsed().as_secs_f64() * 1000.0));

    // Opened as shipped: noticed, rebuilt once, embedded — and no warning.
    let t = Instant::now();
    let mut s = Session::start("documents-form.cfrm");
    s.wait_for("Lbl-Progress", "Caption", |v| v == "Rebuilding the Knowledge Base with the semantic model...");
    let status = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Added"));
    s.quit();
    assert!(status.starts_with("Added 1,"), "the rebuild indexes the document again: {status}");
    assert!(!status.contains("words only"), "embedded, so no words-only warning: {status}");
    report.push(format!("rebuilt with the built-in model, no warning ({status}) — {:.0} ms", t.elapsed().as_secs_f64() * 1000.0));

    println!("\n  ── 071 PowerChat, embedding with the built-in model ─────");
    for r in &report {
        println!("  {r}");
    }
    println!();
    let _ = std::fs::remove_dir_all(&root);
}

/// Operator (2026-09-27): "a function to save the file in PDF form, keeping
/// the formatting done during the conversation (a Save as PDF button, and the
/// OS's dialog to choose the destination)". The button asks the platform's
/// Save panel — `conversation.pdf` proposed — and the answer writes a PDF.
#[test]
fn powerchat_saves_the_conversation_as_a_pdf() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = std::env::temp_dir().join(format!(
        "prc-071-pdf-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let data = root.join("data");
    std::fs::create_dir_all(&data).unwrap();
    plant_model(&root, false);
    std::env::set_var("POWERCHAT_DATA", &data);
    let t = Instant::now();

    let mut s = Session::start("chat-form.cfrm");
    let caption = s.wait_for("Btn-Pdf", "Caption", |v| !v.trim().is_empty());
    assert_eq!(caption, "Save as PDF");
    s.settle();

    // Nothing said yet: the button says so, in the interface's language.
    s.click("Btn-Pdf");
    let _ = s.wait_for("Vwr-Chat", "_SaveAsRequest", |v| !v.trim().is_empty());
    s.input
        .send(StateUpdate::new("Vwr-Chat", "_SaveAsAnswer", &root.join("empty.pdf").display().to_string()))
        .unwrap();
    s.events.send(FormEvent::new("Txt-Input", "onGotFocus")).unwrap();
    s.wait_for("Lbl-Status", "Caption", |v| v == "There is no conversation to save yet.");
    assert!(!root.join("empty.pdf").exists(), "nothing is written for nothing");

    // A conversation, as the chat's own appends leave it.
    let mut conv = cobolt_forms::viewer::Conversation::new();
    conv.append_as(cobolt_forms::viewer::AppendMode::Markdown, "**You:** how many days?", cobolt_forms::viewer::MessageRole::User);
    conv.append_as(
        cobolt_forms::viewer::AppendMode::Markdown,
        "**Agent 1:** Employees get **twenty** working days.\n\n- full time: 20\n- part time: pro rata",
        cobolt_forms::viewer::MessageRole::Agent,
    );
    s.input.send(StateUpdate::new("Vwr-Chat", "_ConversationHtml", &conv.to_html())).unwrap();
    s.click("Btn-Pdf");
    let asked = s.wait_for("Vwr-Chat", "_SaveAsRequest", |v| !v.trim().is_empty());
    assert_eq!(asked, "conversation.pdf", "the Save panel proposes conversation.pdf");

    // The operator picks a folder and a name in the panel.
    let chosen = root.join("chosen").join("my-chat.pdf");
    std::fs::create_dir_all(chosen.parent().unwrap()).unwrap();
    s.input
        .send(StateUpdate::new("Vwr-Chat", "_SaveAsAnswer", &chosen.display().to_string()))
        .unwrap();
    s.events.send(FormEvent::new("Txt-Input", "onGotFocus")).unwrap();
    s.wait_for("Lbl-Status", "Caption", |v| v == "The conversation was saved as a PDF.");
    s.quit();
    let bytes = std::fs::read(&chosen).expect("the PDF is where the panel said");
    assert_eq!(&bytes[..5], b"%PDF-");
    println!(
        "\n  ── 071 PowerChat, Save as PDF ───────────────────────────\n  \
         button → Save panel (conversation.pdf proposed) → {} ({} bytes), status confirmed — {:.0} ms\n",
        chosen.display(),
        bytes.len(),
        t.elapsed().as_secs_f64() * 1000.0
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Operator (2026-09-27): "I can't move a document in the tree into a folder
/// (in the tree, of course)". The Documents tree has `AllowDrag`: a document
/// dropped on a folder moves into it, dropped on empty space moves to the top
/// level, and a folder is not dragged anywhere.
#[test]
fn powerchat_drags_a_document_into_a_folder_in_the_tree() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = std::env::temp_dir().join(format!(
        "prc-071-drag-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let data = root.join("data");
    let kb = root.join("KB");
    let docs = kb.join("HR").join("documents");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&docs).unwrap();
    plant_model(&root, false);
    std::fs::write(docs.join("leave-policy.md"), "# Leave\nTwenty working days a year.").unwrap();
    std::fs::write(docs.join("travel.md"), "# Travel\nBook trains under four hours.").unwrap();
    std::env::set_var("POWERCHAT_DATA", &data);
    seed_settings(&data, &[("CUR-TOPIC", "HR"), ("KB-LOCATION", &kb.display().to_string())]);
    // What the tree raises when a node is let go over another: the dragged
    // node, then the target's index and label (0 and nothing: empty space).
    let drop = |dragged: &str, index: usize, level: usize, target_index: usize, target: &str| {
        FormEvent::new("Trv-Docs", "onNodeDrop")
            .with_value(format!("{dragged}\t{index}\t{level}\t0\t{target_index}\t{target}"))
    };
    let t = Instant::now();

    let mut s = Session::start("documents-form.cfrm");
    s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Added 2,"));
    s.type_into("Txt-Folder", "Policies");
    s.click("Btn-NewFolder");
    s.wait_for("Trv-Docs", "Items", |v| v == "Policies\tfolder\nleave-policy.md\ntravel.md");

    s.events.send(drop("travel.md", 3, 1, 1, "Policies")).unwrap();
    s.wait_for("Trv-Docs", "Items", |v| v == "Policies\n  travel.md\nleave-policy.md");
    s.wait_for("Lbl-Status", "Caption", |v| v == "Moved travel.md to Policies.");
    assert!(docs.join("Policies/travel.md").is_file() && !docs.join("travel.md").exists());

    s.events.send(drop("travel.md", 2, 2, 0, "")).unwrap();
    s.wait_for("Trv-Docs", "Items", |v| v == "Policies\tfolder\nleave-policy.md\ntravel.md");
    s.wait_for("Lbl-Status", "Caption", |v| v == "Moved Policies/travel.md to the top level.");
    assert!(docs.join("travel.md").is_file() && !docs.join("Policies/travel.md").exists());

    s.events.send(drop("Policies", 1, 1, 2, "leave-policy.md")).unwrap();
    s.wait_for("Lbl-Status", "Caption", |v| v == "Only documents move: drag a document onto a folder.");
    s.quit();
    println!(
        "\n  ── 071 PowerChat, dragging in the Documents tree ────────\n  \
         travel.md → Policies, → top level (moved, indexed, originals deleted); a folder refused — {:.0} ms\n",
        t.elapsed().as_secs_f64() * 1000.0
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Operator (2026-09-27): "I need a preview of the document (use the preview
/// control in a modal window)". Preview — or a double-click on a document —
/// opens PREVIEW-FORM modally on that document; a folder is not previewed.
#[test]
fn powerchat_previews_a_document_in_a_modal_window() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = std::env::temp_dir().join(format!(
        "prc-071-preview-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let data = root.join("data");
    let kb = root.join("KB");
    let docs = kb.join("HR").join("documents");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(docs.join("Policies")).unwrap();
    plant_model(&root, false);
    std::fs::write(docs.join("Policies/leave.md"), "# Leave\nTwenty working days a year.").unwrap();
    std::env::set_var("POWERCHAT_DATA", &data);
    seed_settings(&data, &[("CUR-TOPIC", "HR"), ("KB-LOCATION", &kb.display().to_string())]);
    let node = |label: &str, index: usize, level: usize| format!("{label}\t{index}\t{level}\t0");
    let t = Instant::now();

    let mut s = Session::start("documents-form.cfrm");
    s.wait_for("Trv-Docs", "Items", |v| v == "Policies\n  leave.md");
    s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Added 1,"));

    // Nothing chosen: Preview says what to do, and opens nothing.
    s.click("Btn-Preview");
    s.wait_for("Lbl-Status", "Caption", |v| v == "Pick a document in the tree to preview it.");
    assert!(s.opened.try_recv().is_err(), "no preview without a document");

    // A double-click on the document opens the preview on it.
    s.events
        .send(FormEvent::new("Trv-Docs", "onNodeDblClick").with_value(node("leave.md", 2, 2)))
        .unwrap();
    let opened = s.opened.recv_timeout(Duration::from_secs(30)).expect("the preview opens");
    assert_eq!(opened.to_ascii_uppercase(), "PREVIEW-FORM");

    // So does the button, with the document selected.
    s.events
        .send(FormEvent::new("Trv-Docs", "onNodeSelect").with_value(node("leave.md", 2, 2)))
        .unwrap();
    s.click("Btn-Preview");
    let again = s.opened.recv_timeout(Duration::from_secs(30)).expect("Preview opens it too");
    assert_eq!(again.to_ascii_uppercase(), "PREVIEW-FORM");

    // A folder is not a document.
    s.events
        .send(FormEvent::new("Trv-Docs", "onNodeDblClick").with_value(node("Policies", 1, 1)))
        .unwrap();
    s.settle();
    assert!(s.opened.try_recv().is_err(), "a folder opens no preview");
    s.quit();
    println!(
        "\n  ── 071 PowerChat, document preview ──────────────────────\n  \
         double-click and Preview open PREVIEW-FORM on the document; nothing chosen and a folder open nothing — {:.0} ms\n",
        t.elapsed().as_secs_f64() * 1000.0
    );
    let _ = std::fs::remove_dir_all(&root);
}
