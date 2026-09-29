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
    /// What the operator answers in `CONFIRM-FORM` (`Y`/`N`); empty closes
    /// it without answering. Written back the way the real dialog's
    /// `super::"SetProperty"` arrives: on the caller's own form object.
    confirm: Arc<Mutex<String>>,
    /// Every native dialog the program opened: its title and its filters
    /// (`(description, extensions)`), in order.
    dialogs_asked: Arc<Mutex<Vec<(String, Vec<(String, Vec<String>)>)>>>,
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
        let input_for_dialogs = input.clone();
        let (state_tx, state) = mpsc::channel::<StateUpdate>();
        let (display_tx, display) = mpsc::channel::<String>();
        let (opened_tx, opened) = mpsc::channel::<String>();
        let confirm = Arc::new(Mutex::new(String::new()));
        let confirm_answer = confirm.clone();
        let dialogs_asked: Arc<Mutex<Vec<(String, Vec<(String, Vec<String>)>)>>> = Arc::default();
        let asked = dialogs_asked.clone();
        // The form's own object, as `set_form_host` below names it.
        let form_object = "FORM".to_string();
        let err_tx = display_tx.clone();
        // Every form but the main one is opened by the chat form, so its
        // `super` is that form (049 R28): a stand-in that answers.
        let is_main = form.main_form;
        let handle = thread::spawn(move || {
            let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
            interp.set_input_channel(input_rx);
            interp.seed_objects(seed);
            for (id, def) in &menus {
                interp.set_designed_menu(id, def);
            }
            let mut _closed = None;
            let answer_to = input_for_dialogs;
            {
                use cobolt_runtime::form_host::{FormRequest, ROOT_HANDLE};
                let (req_tx, req_rx) = mpsc::channel::<FormRequest>();
                let (closed_tx, closed_rx) = mpsc::channel::<String>();
                _closed = Some(closed_tx);
                interp.set_form_host(req_tx, ROOT_HANDLE, "FORM", closed_rx);
                if !is_main {
                    interp.set_super_form("CHAT-FORM-STAND-IN");
                }
                thread::spawn(move || {
                    let mut answers = dialogs.unwrap_or_default().into_iter();
                    while let Ok(req) = req_rx.recv() {
                        match req {
                            FormRequest::FileDialog { title, filters, reply, .. } => {
                                asked.lock().unwrap().push((title, filters));
                                let _ = reply.send(answers.next().flatten());
                            }
                            FormRequest::OpenForm { form_id, reply, .. } => {
                                let answer = confirm_answer.lock().unwrap().clone();
                                if form_id.eq_ignore_ascii_case("CONFIRM-FORM") && !answer.is_empty() {
                                    let _ = answer_to.send(StateUpdate::new(&form_object, "ConfirmAnswer", &answer));
                                }
                                let _ = opened_tx.send(form_id);
                                let _ = reply.send(None);
                            }
                            // The stand-in parent: `INVOKE super::"PC-REFRESH"()`
                            // and its kin succeed and return nothing.
                            FormRequest::HandleMethod { reply, .. } => {
                                let _ = reply.send(Ok(String::new()));
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
        Session { events, input, state, display, seen: Vec::new(), handle: Some(handle), opened, confirm, dialogs_asked }
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

    /// Click a DataGrid cell the way the host reports one: which cell
    /// (1-based data row and column), then the grid's own event.
    fn cell(&self, ctrl: &str, row: usize, col: usize) {
        self.input.send(StateUpdate::new(ctrl, "ClickedRow", &row.to_string())).unwrap();
        self.input.send(StateUpdate::new(ctrl, "ClickedColumn", &col.to_string())).unwrap();
        self.events.send(FormEvent::new(ctrl, "onCellClick")).unwrap();
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
    let mut s = Session::start("chat-form.cfrm");
    let v = s.settle();
    s.quit();
    let state = menu_state(&v);
    // Chat waits for a topic as well (1.70.280), and a first run has none.
    for id in ["chat", "newc", "tpcs", "docs", "fils", "prmt"] {
        assert!(state.contains(&(id.to_string(), false)), "{id} is shut before a model is set: {state:?}");
    }
    assert!(!state.iter().any(|(id, on)| (id == "sett" || id == "welc") && !on), "RAG settings and Getting started stay open");
    // The welcome screen is its own form since 1.70.261: the chat asks the
    // menu to open its row, and the ContentPane shows it.
    let activated = v
        .get(&("SIDEMENU-1".to_string(), cobolt_forms::menu::runtime::ACTIVATE_ITEM_PROP.to_ascii_uppercase()))
        .cloned()
        .unwrap_or_default();
    assert!(activated.starts_with("welc#"), "the welcome form is opened: {activated:?}");
    report.push(format!(
        "first run: menu shut but for RAG settings and Getting started; welcome form opened — {:.0} ms",
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
    // Saved, the CRUD form goes back to its list tab and says so there.
    s.wait_for("Lbl-List-Status", "Caption", |v| v.contains("key saved"));
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
    for id in ["tpcs", "docs", "fils", "prmt"] {
        assert!(state.contains(&(id.to_string(), true)), "{id} opens once an agent has a model: {state:?}");
    }
    for id in ["chat", "newc"] {
        assert!(state.contains(&(id.to_string(), false)), "{id} still waits for a topic: {state:?}");
    }
    let activated = v
        .get(&("SIDEMENU-1".to_string(), cobolt_forms::menu::runtime::ACTIVATE_ITEM_PROP.to_ascii_uppercase()))
        .cloned()
        .unwrap_or_default();
    assert_eq!(activated, "", "the welcome form is not forced open any more");
    report.push(format!(
        "configured: menu open but Chat and New conversation, which wait for a topic; no welcome form — {:.0} ms",
        t.elapsed().as_secs_f64() * 1000.0
    ));

    // ── Topics: create one, open it ──
    let t = Instant::now();
    let mut s = Session::start("topics-form.cfrm");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("No topics yet"));
    // The CRUD pattern: New opens the Create/Update tab, Save goes back to
    // the grid, and a row's chat icon (column 3) opens that topic.
    s.click("Btn-New");
    s.wait_for("Tab-Crud", "SelectedTab", |v| v == "1");
    s.type_into("Txt-Name", "Human Resources");
    s.type_into("Txt-Prompt", "You answer questions about the company's HR policies.");
    s.click("Btn-Save");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Topic created"));
    s.cell("Dg-List", 1, 3);
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

    // Selecting a folder is where uploads go (1.70.301: the zone copies
    // nothing itself, the form imports into the selected folder).
    s.events.send(node("Policies", 1, 1)).unwrap();
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
    // The emptied folder stays, with its own folder icon, until it is deleted;
    // the tree is rebuilt before the status says what went.
    s.wait_for("Trv-Docs", "Items", |v| v == "Policies\tfolder\nleave-policy.md");
    s.wait_for("Lbl-Status", "Caption", |v| v == "Deleted: Policies/travel.md - removed from the Knowledge Base.");

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
    s.click("Btn-New"); // an empty Create/Update page for a new connection
    s.wait_for("Tab-Crud", "SelectedTab", |v| v == "1");
    s.type_into("Txt-Name", "planner");
    s.pick("Cmb-Provider", 14); // Ollama (Local), the IDE's 15th provider
    s.type_into("Txt-Url", &url);
    s.type_into("Txt-Key", "");
    s.click("Btn-Save");
    s.wait_for("Lbl-List-Status", "Caption", |v| v.contains("Connection saved"));
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
    s.settle();
    s.click("Btn-New"); // the Create/Update page, empty
    s.wait_for("Tab-Crud", "SelectedTab", |v| v == "1");
    s.type_into("Txt-Data", &root.join("missing.idx").display().to_string());
    s.type_into("Txt-Cidx", &actors_cidx.display().to_string());
    s.click("Btn-Save");
    let refused = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Not added"));
    assert!(refused.contains("NOT-FOUND"), "{refused}");
    s.type_into("Txt-Data", &actors.display().to_string());
    s.type_into("Txt-Cidx", &actors_cidx.display().to_string());
    s.click("Btn-Save");
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
    // One grid row per version, newest first: number, date, the active
    // marker, a preview, then promote (5), edit (6) and delete (7).
    s.wait_for("Dg-List", "Rows", |v| v.starts_with("1\t") && v.contains("[active]"));
    s.click("Btn-New");
    s.wait_for("Tab-Crud", "SelectedTab", |v| v == "1");
    s.type_into("Txt-Prompt", "You answer HR questions in two sentences at most.");
    s.click("Btn-Save");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Saved as v2, now active"));
    assert!(topics_text().contains("two sentences at most"), "v2 is the topic's prompt now");
    // Promoting v1 (row 2 now) asks first: No changes nothing.
    *s.confirm.lock().unwrap() = "N".into();
    s.cell("Dg-List", 2, 5);
    assert_eq!(s.opened.recv_timeout(Duration::from_secs(30)).unwrap().to_ascii_uppercase(), "CONFIRM-FORM");
    s.settle();
    assert!(topics_text().contains("two sentences at most"), "nothing changes when the operator says no");
    *s.confirm.lock().unwrap() = "Y".into();
    s.cell("Dg-List", 2, 5);
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("v1 is the active prompt"));
    s.quit();
    let topics = topics_text();
    assert!(topics.contains("company's HR policies") && !topics.contains("two sentences at most"), "v1 is back");
    report.push(format!(
        "prompts:   v1 from the topic; v2 saved and active; v1 promoted back only once the dialog says yes — {:.0} ms",
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
    s.cell("Dg-List", 3, 3);
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
    s.cell("Dg-List", 1, 3);
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Topic opened"));
    s.quit();
    let langs = [
        ("pt", "Enviar", "Este mês:", "Nova conversa", "Instalar tópicos de exemplo"),
        ("es", "Enviar", "Este mes:", "Nueva conversación", "Instalar temas de ejemplo"),
        ("fr", "Envoyer", "Ce mois-ci :", "Nouvelle conversation", "Installer les sujets d'exemple"),
        ("jp", "送信", "今月：", "新しい会話", "サンプルトピックをインストール"),
        ("cn", "发送", "本月：", "新对话", "安装示例主题"),
        ("en", "Send", "This month:", "New conversation", "Install sample topics"),
    ];
    let mut checked = 0;
    for (code, send, month, new_conv, install) in langs {
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
        s.wait_for("Btn-Install", "Caption", |v| v == install);
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
    // The update the form runs when it opens, over and done — as a person
    // would see it — before anything is dropped.
    s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Added 0,"));

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
    // The readable one, from outside the topic: the zone hands its own path
    // over and the form imports it.
    let outside = root.join("Downloads");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("policy.md"), "# Policy\nTwenty days.").unwrap();
    s.input
        .send(StateUpdate::new("Drop-Docs", "DroppedFiles", &outside.join("policy.md").display().to_string()))
        .unwrap();
    s.events.send(FormEvent::new("Drop-Docs", "onFilesDropped")).unwrap();
    s.wait_for("Lbl-Status", "Caption", |v| {
        v.starts_with("Added: policy.md.") && v.ends_with("Not added - this type of file cannot be read: photo.png, old.doc")
    });
    assert!(docs.join("policy.md").is_file(), "imported into the topic's documents");
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
    assert!(status.starts_with("Added 1,"), "{status}");
    assert!(status.contains("passages indexed."), "a lexical update says how many passages it stored: {status}");
    assert!(
        status.contains("Search is by words only: this application uses the lexical embedder"),
        "R18 — and says it is by words only: {status}"
    );
    report.push(format!("lexical index made the old way — {:.0} ms", t.elapsed().as_secs_f64() * 1000.0));

    // Opened as shipped: noticed, rebuilt once, embedded — and no warning.
    let t = Instant::now();
    let mut s = Session::start("documents-form.cfrm");
    s.wait_for("Lbl-Progress", "Caption", |v| v == "Rebuilding the Knowledge Base with the semantic model...");
    // Two bars, one per stage, each gone at 100 % (operator, 2026-09-27):
    // the documents split into passages, then the passages embedded.
    let chunking = s.wait_for("Lbl-Chunk", "Caption", |v| v == "Chunking: 1 of 1 documents");
    let embedding = s.wait_for("Lbl-Embed", "Caption", |v| v == "Embeddings: 0 of 1 passages");
    s.wait_for("Prg-Chunk", "Visible", |v| v.eq_ignore_ascii_case("false"));
    s.wait_for("Prg-Embed", "Visible", |v| v.eq_ignore_ascii_case("false"));
    let status = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Added"));
    assert!(status.contains("passages embedded: semantic search."), "the embedding is confirmed: {status}");
    report.push(format!("bars while it ran: {chunking:?}, {embedding:?}; both hidden at 100 %"));
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
    // What the host sends with the answer: nothing else happens on the form.
    s.events.send(FormEvent::new("Vwr-Chat", cobolt_runtime::form_host::FormSupervisor::INPUT_WAKE_EVENT)).unwrap();
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
    // What the host sends with the answer: nothing else happens on the form.
    s.events.send(FormEvent::new("Vwr-Chat", cobolt_runtime::form_host::FormSupervisor::INPUT_WAKE_EVENT)).unwrap();
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

/// Operator (2026-09-27): "I also need feedback when … deleting a file".
/// Delete says what it is doing while it runs, and names what it removed.
#[test]
fn powerchat_confirms_a_deleted_document_by_name() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = std::env::temp_dir().join(format!(
        "prc-071-delete-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let data = root.join("data");
    let kb = root.join("KB");
    let docs = kb.join("HR").join("documents");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&docs).unwrap();
    plant_model(&root, false);
    std::fs::write(docs.join("leave.md"), "# Leave\nTwenty days.").unwrap();
    std::fs::write(docs.join("travel.md"), "# Travel\nTrains.").unwrap();
    std::env::set_var("POWERCHAT_DATA", &data);
    seed_settings(&data, &[("CUR-TOPIC", "HR"), ("KB-LOCATION", &kb.display().to_string())]);
    let t = Instant::now();

    let mut s = Session::start("documents-form.cfrm");
    s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Added 2,"));
    s.events
        .send(FormEvent::new("Trv-Docs", "onNodeSelect").with_value("travel.md\t2\t1\t0".to_string()))
        .unwrap();
    s.click("Btn-Delete");
    let doing = s.wait_for("Lbl-Progress", "Caption", |v| v == "Removing travel.md from the Knowledge Base...");
    s.wait_for("Trv-Docs", "Items", |v| v == "leave.md");
    let done = s.wait_for("Lbl-Status", "Caption", |v| v == "Deleted: travel.md - removed from the Knowledge Base.");
    s.quit();
    assert!(!docs.join("travel.md").exists());
    println!(
        "\n  ── 071 PowerChat, deleting a document ───────────────────\n  while: {doing:?}\n  after: {done:?} — {:.0} ms\n",
        t.elapsed().as_secs_f64() * 1000.0
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Operator (2026-09-27): an upload whose name is already in the folder asks
/// first — replace, or keep — and every upload says what it did.
#[test]
fn powerchat_asks_before_replacing_an_uploaded_document() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = std::env::temp_dir().join(format!(
        "prc-071-upload-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let data = root.join("data");
    let kb = root.join("KB");
    let docs = kb.join("HR").join("documents");
    let outside = root.join("Downloads");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    plant_model(&root, false);
    std::fs::write(docs.join("leave.md"), "# Leave\nTwenty days.").unwrap();
    std::fs::write(outside.join("leave.md"), "# Leave\nTwenty-five days from 2027.").unwrap();
    std::fs::write(outside.join("travel.md"), "# Travel\nTrains under four hours.").unwrap();
    std::env::set_var("POWERCHAT_DATA", &data);
    seed_settings(&data, &[("CUR-TOPIC", "HR"), ("KB-LOCATION", &kb.display().to_string())]);
    let drop = |s: &Session, files: &[&std::path::Path]| {
        let list: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
        s.input.send(StateUpdate::new("Drop-Docs", "DroppedFiles", &list.join("\n"))).unwrap();
        s.events.send(FormEvent::new("Drop-Docs", "onFilesDropped")).unwrap();
    };
    let t = Instant::now();
    let mut s = Session::start("documents-form.cfrm");
    s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Added 1,"));

    // A new document: added, no question.
    drop(&s, &[&outside.join("travel.md")]);
    let added = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Added: travel.md."));
    assert!(s.opened.try_recv().is_err(), "nothing to ask about a new name");
    assert!(docs.join("travel.md").is_file());

    // The same name, and the operator keeps the old one.
    *s.confirm.lock().unwrap() = "N".into();
    drop(&s, &[&outside.join("leave.md")]);
    let kept = s.wait_for("Lbl-Status", "Caption", |v| v == "Not replaced: leave.md.");
    assert_eq!(s.opened.recv_timeout(Duration::from_secs(30)).unwrap().to_ascii_uppercase(), "CONFIRM-FORM");
    assert_eq!(std::fs::read_to_string(docs.join("leave.md")).unwrap(), "# Leave\nTwenty days.", "kept as it was");

    // The same name, and the operator replaces it: updated and re-indexed.
    *s.confirm.lock().unwrap() = "Y".into();
    drop(&s, &[&outside.join("leave.md")]);
    let updated = s.wait_for("Lbl-Status", "Caption", |v| v.starts_with("Updated: leave.md."));
    assert_eq!(s.opened.recv_timeout(Duration::from_secs(30)).unwrap().to_ascii_uppercase(), "CONFIRM-FORM");
    assert!(std::fs::read_to_string(docs.join("leave.md")).unwrap().contains("Twenty-five"), "replaced");
    assert!(!docs.join("leave (2).md").exists(), "never a second copy");
    s.quit();
    println!(
        "\n  ── 071 PowerChat, uploading ─────────────────────────────\n  new: {added:?}\n  keep: {kept:?}\n  replace: {updated:?} — {:.0} ms\n",
        t.elapsed().as_secs_f64() * 1000.0
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Seed PowerChat's turns file: `(sequence, role, text)`, where a text
/// `ALL:X` fills the whole 2,000-character record with `X`.
fn seed_turns(data: &Path, conv: &str, turns: &[(u32, &str, &str)]) {
    let mut writes = String::new();
    for (seq, role, text) in turns {
        let fill = match text.strip_prefix("ALL:") {
            Some(ch) => format!("           MOVE ALL \"{ch}\" TO TRN-TEXT\n"),
            None => format!("           MOVE \"{text}\" TO TRN-TEXT\n"),
        };
        writes.push_str(&format!(
            "           MOVE \"{conv}\" TO TRN-CONV\n           MOVE {seq} TO TRN-SEQ\n           MOVE \"{role}\" TO TRN-ROLE\n{fill}           WRITE TURN-REC\n"
        ));
    }
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. SEEDTURNS.\n       ENVIRONMENT DIVISION.\n       INPUT-OUTPUT SECTION.\n       FILE-CONTROL.\n           SELECT TURNS-FILE ASSIGN TO \"{}\"\n               ORGANIZATION IS INDEXED\n               ACCESS MODE IS DYNAMIC\n               RECORD KEY IS TRN-KEY\n               FILE STATUS IS WS-FS\n               STORAGE MODE IS DISK.\n       DATA DIVISION.\n       FILE SECTION.\n       FD  TURNS-FILE.\n       01  TURN-REC.\n           05 TRN-KEY.\n              10 TRN-CONV PIC X(16).\n              10 TRN-SEQ PIC 9(5).\n           05 TRN-ROLE PIC X.\n           05 TRN-TEXT PIC X(2000).\n       WORKING-STORAGE SECTION.\n       01 WS-FS PIC XX.\n       PROCEDURE DIVISION.\n           OPEN OUTPUT TURNS-FILE\n{writes}           CLOSE TURNS-FILE\n           STOP RUN.\n",
        data.join("turns.idx").display()
    );
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
    Interpreter::new(parsed.program.expect("the seeding program parses")).run().expect("turns seeded");
}

/// Operator (2026-09-27): "do not limit the size of the question". A turn
/// longer than one TURNS-FILE record is kept over several — the first under
/// its role, the rest in lower case — and a conversation reopened shows it
/// whole; a turn saved the old way, in one record, reads as it always did.
#[test]
fn powerchat_reopens_a_long_question_whole() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = std::env::temp_dir().join(format!(
        "prc-071-long-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let data = root.join("data");
    std::fs::create_dir_all(&data).unwrap();
    plant_model(&root, false);
    std::env::set_var("POWERCHAT_DATA", &data);
    let conv = "2026092712000000";
    seed_turns(&data, conv, &[
        (1, "U", "A short question, saved the old way."),
        (2, "A", "A short answer, saved the old way."),
        (3, "U", "ALL:Q"),
        (4, "u", "ALL:R"),
        (5, "u", "and the tail of a 4,000-plus character question END-OF-QUESTION"),
        (6, "A", "Answered."),
    ]);
    let t = Instant::now();
    let mut s = Session::start("chat-form.cfrm");
    s.settle();
    s.menu(&format!("c{conv}"));
    // The whole conversation: its last turn is there.
    let html = s.wait_for("Vwr-Chat", "_ConversationHtml", |v| v.contains("END-OF-QUESTION") && v.contains("Answered."));
    // …and the chat is brought into view, or with another form on the pane
    // the conversation loads out of sight (operator, 2026-09-28).
    let shown = s.wait_for("SideMenu-1", cobolt_forms::menu::runtime::ACTIVATE_ITEM_PROP, |v| v.starts_with("chat#"));
    assert!(shown.starts_with("chat#"), "{shown}");
    s.quit();
    let msgs = cobolt_forms::viewer::parse_conversation_html(&html);
    let texts: Vec<String> = msgs
        .iter()
        .map(|m| {
            m.blocks
                .iter()
                .flat_map(|b| match b {
                    cobolt_forms::viewer::Block::Paragraph { content }
                    | cobolt_forms::viewer::Block::Heading { content, .. } => content.clone(),
                    _ => Vec::new(),
                })
                .map(|i| match i {
                    cobolt_forms::viewer::Inline::Text { text, .. } => text,
                    cobolt_forms::viewer::Inline::Break { .. } => " ".to_string(),
                    _ => String::new(),
                })
                .collect::<String>()
        })
        .collect();
    assert_eq!(msgs.len(), 4, "four turns, not six records: {texts:?}");
    let long = &texts[2];
    assert!(long.contains(&"Q".repeat(2000)) && long.contains(&"R".repeat(2000)), "the pieces, joined");
    assert!(long.contains(&format!("{}{}", "Q".repeat(2000), "R".repeat(2000))), "joined exactly, nothing between");
    assert!(long.trim_end().ends_with("END-OF-QUESTION"), "to its last word");
    assert!(texts[0].contains("saved the old way") && texts[3].contains("Answered."));
    println!(
        "\n  ── 071 PowerChat, a long question ───────────────────────\n  6 records → 4 turns; the long one {} characters, whole — {:.0} ms\n",
        long.trim().chars().count(),
        t.elapsed().as_secs_f64() * 1000.0
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// One indexed file written from COBOL: `fields` is its record's 05-levels,
/// `key` its RECORD KEY, `writes` the MOVEs and WRITEs.
fn seed_indexed(path: &Path, fields: &str, key: &str, writes: &str) {
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. SEEDIDX.\n       ENVIRONMENT DIVISION.\n       INPUT-OUTPUT SECTION.\n       FILE-CONTROL.\n           SELECT F ASSIGN TO \"{}\"\n               ORGANIZATION IS INDEXED\n               ACCESS MODE IS DYNAMIC\n               RECORD KEY IS {key}\n               FILE STATUS IS WS-FS\n               STORAGE MODE IS DISK.\n       DATA DIVISION.\n       FILE SECTION.\n       FD  F.\n       01  R.\n{fields}       WORKING-STORAGE SECTION.\n       01 WS-FS PIC XX.\n       PROCEDURE DIVISION.\n           OPEN OUTPUT F\n{writes}           CLOSE F\n           STOP RUN.\n",
        path.display()
    );
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
    Interpreter::new(parsed.program.expect("the seeding program parses")).run().expect("seeded");
}

/// Operator (2026-09-27): a document uploaded while the chat was open, and
/// the answer was "the internal documents were not provided". The sources
/// were counted only when the topic loaded, so the orchestrator stayed told
/// "this topic has no documents — say so". They are counted again before
/// each question, and the instructions set again when that count moves off 0.
#[test]
fn powerchat_counts_a_document_added_while_the_chat_is_open() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = std::env::temp_dir().join(format!(
        "prc-071-sources-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let data = root.join("data");
    let kb = root.join("KB");
    let docs = kb.join("HR").join("documents");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&docs).unwrap();
    plant_model(&root, false);
    std::env::set_var("POWERCHAT_DATA", &data);
    // The note comes from the shipped main prompt's NO SOURCES section.
    std::env::set_var("POWERCHAT_SAMPLES", project().join("samples"));
    seed_settings(&data, &[
        ("CUR-TOPIC", "HR"),
        ("KB-LOCATION", &kb.display().to_string()),
        ("AGENT-1-ENTRY", "Local"),
    ]);
    seed_indexed(
        &data.join("topics.idx"),
        "           05 TOP-ID PIC X(16).\n           05 TOP-NAME PIC X(40).\n           05 TOP-PROMPT PIC X(1000).\n           05 TOP-CREATED PIC X(14).\n           05 TOP-SAMPLE PIC X.\n",
        "TOP-ID",
        "           MOVE \"HR\" TO TOP-ID\n           MOVE \"HR\" TO TOP-NAME\n           MOVE \"You answer HR questions.\" TO TOP-PROMPT\n           WRITE R\n",
    );
    seed_indexed(
        &data.join("models.idx"),
        "           05 MDL-NAME PIC X(30).\n           05 MDL-API PIC X(12).\n           05 MDL-URL PIC X(200).\n           05 MDL-MODEL PIC X(80).\n           05 MDL-TOOLS PIC X.\n           05 MDL-RANK PIC 9.\n",
        "MDL-NAME",
        "           MOVE \"Local\" TO MDL-NAME\n           MOVE \"ollama\" TO MDL-API\n           MOVE \"http://127.0.0.1:9/api\" TO MDL-URL\n           MOVE \"test-model\" TO MDL-MODEL\n           MOVE \"Y\" TO MDL-TOOLS\n           MOVE 5 TO MDL-RANK\n           WRITE R\n",
    );
    let t = Instant::now();
    let mut s = Session::start("chat-form.cfrm");
    let before = s.wait_for("AGENT-1", "SystemPrompt", |v| v.contains("You answer HR questions."));
    assert!(before.contains("has no documents"), "no documents yet, and told so: {before}");

    // A document arrives while the chat is open — Documents, another user,
    // a copy into the folder — and a question is asked.
    std::fs::write(docs.join("contracts.md"), "# Contracts\nApproved templates live in Orbita.").unwrap();
    s.type_into("Txt-Input", "How do I write a new contract?");
    s.click("Btn-Send");
    let after = s.wait_for("AGENT-1", "SystemPrompt", |v| {
        v.contains("You answer HR questions.") && !v.contains("has no documents")
    });
    s.quit();
    println!(
        "\n  ── 071 PowerChat, sources counted per question ──────────\n  before: {:?}…\n  after:  {:?} — {:.0} ms\n",
        before.chars().take(90).collect::<String>(),
        after.trim(),
        t.elapsed().as_secs_f64() * 1000.0
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Operator (2026-09-27): "when a table or a formatted presentation
/// (Markdown, HTML…) is asked for, the answer goes straight into the
/// conversation, not into a bubble". A conversation reopened shows it the same
/// way — decided from the answer, as a live answer is.
#[test]
fn powerchat_puts_a_formatted_answer_in_the_stream_not_a_bubble() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = std::env::temp_dir().join(format!(
        "prc-071-stream-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let data = root.join("data");
    std::fs::create_dir_all(&data).unwrap();
    plant_model(&root, false);
    std::env::set_var("POWERCHAT_DATA", &data);
    let conv = "2026092713000000";
    seed_turns(&data, conv, &[
        (1, "U", "How many leave days?"),
        (2, "A", "Twenty working days a year."),
        (3, "U", "Show them as a table"),
        (4, "A", "| Grade | Days | |---|---| | A | 20 |"),
        (5, "U", "And as HTML"),
        (6, "A", "<table><tr><td>A</td><td>20</td></tr></table>"),
        (7, "U", "Make it a polished HTML page"),
        // A Markdown answer that shows a page in a fenced block: the page is
        // rendered, not shown as code to copy (operator, 2026-09-27).
        (8, "A", "Here is the page: ```html <html><head><style>h1{color:red}</style></head><body><h1>Contracts</h1><table><tr><td>A</td><td>20</td></tr></table></body></html>``` Save it as a file."),
        (9, "U", "Build the contract process, polished HTML"),
        // A page written STRAIGHT into a Markdown answer, no fence — what a
        // model actually sent (operator, 2026-09-28). Markdown would read it
        // only to its first blank line and show the rest as code.
        (10, "A", "Here is the process: *** <!DOCTYPE html><html><head><style>.box{background:#0056b3;color:white;padding:12px}</style></head><body><div class='box'><h2>Contract flow</h2></div><table><tr><td>Draft</td><td>Legal</td></tr></table></body></html> Anything else?"),
    ]);
    let mut s = Session::start("chat-form.cfrm");
    s.settle();
    s.menu(&format!("c{conv}"));
    let html = s.wait_for("Vwr-Chat", "_ConversationHtml", |v| v.contains("Anything else"));
    s.quit();
    let messages = cobolt_forms::viewer::parse_conversation_html(&html);
    let roles: Vec<cobolt_forms::viewer::MessageRole> = messages.iter().map(|m| m.role).collect();
    use cobolt_forms::viewer::MessageRole::{Agent, None as Stream, User};
    assert_eq!(
        roles,
        vec![User, Agent, User, Stream, User, Stream, User, Stream, Stream, Stream, User, Stream, Stream, Stream],
        "a reply in its bubble; the presentations in the stream, each page (fenced or not) as its own rendered part"
    );
    use cobolt_forms::viewer::Block;
    let page = &messages[8].blocks;
    assert!(page.iter().any(|b| matches!(b, Block::Table { .. })), "the fenced page is rendered: {page:?}");
    // The unfenced page: rendered as HTML, its own CSS applied (the .box's
    // background), between its prose before and after.
    fn has_styled(blocks: &[Block]) -> bool {
        blocks.iter().any(|b| match b {
            Block::Styled { style, blocks } => {
                style.background == Some(cobolt_forms::viewer::Background::Solid("#0056b3ff".into())) || has_styled(blocks)
            }
            _ => false,
        })
    }
    assert!(has_styled(&messages[12].blocks), "the page's CSS box: {:?}", messages[12].blocks);
    use cobolt_forms::viewer::SearchableText;
    let text_of = |i: usize| cobolt_forms::viewer::LayoutDocument { blocks: messages[i].blocks.clone() }.searchable_text().unwrap_or_default();
    assert!(text_of(11).contains("Here is the process") && !text_of(11).contains("DOCTYPE"), "{}", text_of(11));
    assert!(text_of(12).contains("Contract flow") && text_of(12).contains("Draft"));
    assert!(text_of(13).contains("Anything else"));
    assert!(
        !messages.iter().flat_map(|m| &m.blocks).any(|b| matches!(b, Block::CodeBlock { .. })),
        "no page is shown as code to copy"
    );
    let form = cobolt_forms::load_form(&project().join("forms/chat-form.cfrm")).unwrap();
    let viewer = form.controls.iter().find(|c| c.id == "Vwr-Chat").unwrap();
    assert_eq!(viewer.get_prop("UserBubbleColor").map(|v| v.as_str().to_string()).as_deref(), Some("#5CC962FF"));
    assert!(viewer.get_prop("UserBubbleBold").is_some_and(|v| v.as_bool()), "the question in the bold face");
    println!(
        "\n  ── 071 PowerChat, presentations in the stream ───────────\n  roles: {roles:?}\n  a ```html fence: rendered as a page ({} blocks), not code; the user's bubble #5CC962FF, bold\n",
        page.len()
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Operator (2026-09-28): adding a data file had no way to browse for it —
/// "a button that opens the OS dialog, limited to RustCOBOL's indexed-file
/// extension". Both paths get one: the data file (`.idx`) and its
/// description (`.cidx`), each landing in its field, and a cancelled panel
/// leaving the field as it was.
#[test]
fn powerchat_browses_for_a_data_file_and_its_description() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = std::env::temp_dir().join(format!(
        "prc-071-browse-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let data = root.join("data");
    std::fs::create_dir_all(&data).unwrap();
    std::env::set_var("POWERCHAT_DATA", &data);
    seed_settings(&data, &[("CUR-TOPIC", "2026092800000000")]);
    let idx = root.join("actors.idx").display().to_string();
    let cidx = root.join("actors.cidx").display().to_string();
    let mut s = Session::start_with_dialogs("files-form.cfrm", vec![Some(idx.clone()), Some(cidx.clone()), None]);
    s.settle();
    s.click("Btn-New");
    s.wait_for("Tab-Crud", "SelectedTab", |v| v == "1");
    s.click("Btn-BrowseData");
    let got = s.wait_for("Txt-Data", "Text", |v| v.trim() == idx);
    s.click("Btn-BrowseCidx");
    let got_cidx = s.wait_for("Txt-Cidx", "Text", |v| v.trim() == cidx);
    // Cancelled: the field keeps what it had.
    s.click("Btn-BrowseData");
    let v = s.settle();
    let asked = s.dialogs_asked.lock().unwrap().clone();
    s.quit();
    assert_eq!(got.trim(), idx);
    assert_eq!(got_cidx.trim(), cidx);
    assert!(v.get(&("TXT-DATA".to_string(), "TEXT".to_string())).is_none_or(|t| t.trim() == idx), "a cancelled panel changes nothing");
    assert_eq!(asked.len(), 3, "{asked:?}");
    assert_eq!(asked[0].1, vec![("RustCOBOL indexed files".to_string(), vec!["idx".to_string()])], "the data file: .idx only");
    assert_eq!(asked[1].1, vec![("RustCOBOL file descriptions".to_string(), vec!["cidx".to_string()])], "its description: .cidx only");
    assert_eq!(asked[0].0, "Choose the indexed data file");
    println!("\n  ── 071 PowerChat, browse for a data file ──\n  panels: {asked:?}\n  fields: {got} / {got_cidx}\n");
    let _ = std::fs::remove_dir_all(&root);
}

/// A scripted Ollama for the report templates: a planning request (it asks
/// to split the question) is answered `ASK:` with the template question; a
/// request carrying a template's skeleton, with `reply`; any other with
/// `first`. Every request is kept.
fn report_server(first: &'static str, reply: &'static str) -> (String, Arc<Mutex<Vec<String>>>) {
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
            let content = if body.contains("Split the user") {
                "ASK: Which template should the report use? 1. Executive (recommended) 2. Timeline"
            } else if body.contains("Skeleton:") {
                reply
            } else {
                first
            };
            let reply = serde_json::json!({
                "message": {"role": "assistant", "content": content},
                "prompt_eval_count": 50, "eval_count": 20
            })
            .to_string();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(),
                reply
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    (format!("http://127.0.0.1:{port}/api"), seen)
}

/// A PowerChat data folder with the HR topic and `agents` agents on the
/// model at `url`.
fn report_setup(tag: &str, url: &str, agents: usize) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "prc-071-{tag}-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let data = root.join("data");
    std::fs::create_dir_all(&data).unwrap();
    plant_model(&root, false);
    std::env::set_var("POWERCHAT_DATA", &data);
    std::env::set_var("POWERCHAT_SAMPLES", project().join("samples"));
    let mut settings = vec![("CUR-TOPIC", "HR"), ("AGENT-1-ENTRY", "Local")];
    if agents > 1 {
        settings.push(("AGENT-2-ENTRY", "Local"));
    }
    seed_settings(&data, &settings);
    seed_indexed(
        &data.join("topics.idx"),
        "           05 TOP-ID PIC X(16).\n           05 TOP-NAME PIC X(40).\n           05 TOP-PROMPT PIC X(1000).\n           05 TOP-CREATED PIC X(14).\n           05 TOP-SAMPLE PIC X.\n",
        "TOP-ID",
        "           MOVE \"HR\" TO TOP-ID\n           MOVE \"HR\" TO TOP-NAME\n           MOVE \"You answer HR questions.\" TO TOP-PROMPT\n           WRITE R\n",
    );
    seed_indexed(
        &data.join("models.idx"),
        "           05 MDL-NAME PIC X(30).\n           05 MDL-API PIC X(12).\n           05 MDL-URL PIC X(200).\n           05 MDL-MODEL PIC X(80).\n           05 MDL-TOOLS PIC X.\n           05 MDL-RANK PIC 9.\n",
        "MDL-NAME",
        &format!(
            "           MOVE \"Local\" TO MDL-NAME\n           MOVE \"ollama\" TO MDL-API\n           MOVE \"{url}\" TO MDL-URL\n           MOVE \"test-model\" TO MDL-MODEL\n           MOVE \"N\" TO MDL-TOOLS\n           MOVE 5 TO MDL-RANK\n           WRITE R\n"
        ),
    );
    (root, data)
}

/// Operator (2026-09-28): reports come from templates - fixed ones, ones
/// changed by prompt and new ones described in the chat - built with Bulma,
/// no JavaScript; every instruction in the main prompt. The orchestrator's
/// instructions are the main prompt's SYSTEM section with the topic's prompt
/// and the eleven templates filled in; its "TEMPLATE: Timeline" brings the
/// Timeline's skeleton back with the TEMPLATE section; a template the answer
/// defines is saved with the answer's page as its skeleton, cut out of what
/// the user sees, and offered in the next session.
#[test]
fn powerchat_offers_report_templates_and_saves_a_new_one() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // The model repeats its choice over the report — what one sent
    // (operator, 2026-09-29) — and the line is not shown.
    const REPLY: &str = "TEMPLATE: Timeline\n\nHere is your report.\n```html\n<html><head><link rel=\"stylesheet\" \
        href=\"https://cdn.jsdelivr.net/npm/bulma@0.9.4/css/bulma.min.css\"></head><body>\
        <div class=\"th-band\" style=\"background: #2e7d32\"><p>GREEN-BAND</p></div>\
        </body></html>\n```\n<!--REPORT-TEMPLATE\nNAME: Green Timeline\nSUITS: a year's milestones, in green.\n-->\n";
    let (url, requests) = report_server("TEMPLATE: Timeline", REPLY);
    let (root, data) = report_setup("templates", &url, 1);
    let t = Instant::now();
    let mut s = Session::start("chat-form.cfrm");
    let prompt = s.wait_for("AGENT-1", "SystemPrompt", |v| v.contains("- Animated:"));
    for name in ["Executive", "Informational", "List", "Timeline", "Comparison", "Map", "Statistics",
                 "Flowchart", "Hierarchy", "Anatomical", "Animated"] {
        assert!(prompt.contains(&format!("- {name}: ")), "the {name} template is offered");
    }
    assert!(prompt.contains("You answer HR questions."), "{{TOPIC}} is the topic's prompt");
    assert!(prompt.contains("This topic has no documents") && prompt.contains("'Documents' in the menu"), "{{SOURCES NOTE}}");
    assert!(prompt.contains("REPORTS") && prompt.contains("always include") && prompt.contains("TEMPLATE: <"));
    assert!(!prompt.contains('{') && !prompt.contains("=== "), "no placeholder or section line is sent: {prompt}");

    s.type_into("Txt-Input", "A report of the leave year, green timeline, no icons");
    s.click("Btn-Send");
    let html = s.wait_for("Vwr-Chat", "_ConversationHtml", |v| v.contains("Here is your report"));
    s.quit();
    let sent = requests.lock().unwrap().clone();
    assert_eq!(sent.len(), 2, "the choice, then the report from its skeleton");
    assert!(sent[1].contains("The user chose the report template") && sent[1].contains("Timeline"),
        "the TEMPLATE section, the name filled in");
    assert!(sent[1].contains("th-band") && sent[1].contains("Skeleton:"), "the Timeline's skeleton");
    assert!(!sent[1].contains("{SKELETON}") && !sent[1].contains("{NAME}"));
    assert!(!html.contains("REPORT-TEMPLATE") && !html.contains("Green Timeline"), "the block is not shown: {html}");
    assert!(!html.contains("TEMPLATE: Timeline"), "the choice is not shown over the report: {html}");
    let turns = String::from_utf8_lossy(&std::fs::read(data.join("turns.idx")).unwrap()).into_owned();
    assert!(turns.contains("Here is your report") && !turns.contains("TEMPLATE: Timeline"), "nor kept with it");
    let saved = String::from_utf8_lossy(&std::fs::read(data.join("templates.idx")).unwrap()).into_owned();
    assert!(saved.contains("Green Timeline") && saved.contains("in green.") && saved.contains("GREEN-BAND"),
        "saved with the answer's page as its skeleton");

    // The next session offers it with the shipped ones.
    let mut s = Session::start("chat-form.cfrm");
    let again = s.wait_for("AGENT-1", "SystemPrompt", |v| v.contains("- Green Timeline:"));
    s.quit();
    assert!(again.contains("- Green Timeline: a year's milestones, in green."), "{again}");
    println!(
        "\n  ── 071 PowerChat, report templates ──────────────────────\n  11 shipped templates in the instructions ({} characters); TEMPLATE: Timeline answered with its skeleton ({} characters sent); \
         one defined in an answer saved with its page, hidden from the user, offered next session — {:.0} ms\n",
        prompt.len(),
        sent[1].len(),
        t.elapsed().as_secs_f64() * 1000.0
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// With several agents the orchestrator plans first; a report with no
/// template chosen yet is answered `ASK:` — a question for the user, shown
/// as the answer, not split into tasks.
#[test]
fn powerchat_asks_which_template_before_planning_a_report() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (url, requests) = report_server("unused", "unused");
    let (root, _data) = report_setup("ask", &url, 2);
    let mut s = Session::start("chat-form.cfrm");
    s.settle();
    s.type_into("Txt-Input", "Make me a report on leave");
    s.click("Btn-Send");
    let html = s.wait_for("Vwr-Chat", "_ConversationHtml", |v| v.contains("Which template"));
    s.quit();
    assert!(!html.contains("ASK:"), "the marker is not shown: {html}");
    let asked = requests.lock().unwrap().len();
    assert_eq!(asked, 1, "one planning request, no task sent to another agent");
    println!("\n  ── 071 PowerChat, template asked before planning ────────\n  {asked} request; the question shown\n");
    let _ = std::fs::remove_dir_all(&root);
}

/// Operator (2026-09-28): "every instruction to the model in English, part of
/// the main prompt, not inserted internally where the user cannot change
/// them". The Prompt screen edits the main prompt - versions like a topic's,
/// under "*MAIN" - and the chat's next question uses it: the orchestrator's
/// SYSTEM section, the assistants' ASSISTANT section. Restore default brings
/// the shipped text back as a new version. The versions of the old
/// prompts.idx move to prompt-versions.idx the first time.
#[test]
fn powerchat_main_prompt_is_every_instruction_and_the_user_edits_it() {
    let _data_lock = POWERCHAT_DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (url, _requests) = report_server("Plain answer.", "unused");
    let (root, data) = report_setup("mainprompt", &url, 2);
    // The versions file as it was before 1.70.332: 1,000-character texts.
    seed_indexed(
        &data.join("prompts.idx"),
        "           05 K.\n              10 T PIC X(16).\n              10 V PIC 9(4).\n           05 C PIC X(14).\n           05 A PIC X.\n           05 X PIC X(1000).\n",
        "K",
        "           MOVE \"HR\" TO T\n           MOVE 1 TO V\n           MOVE \"20260901120000\" TO C\n           MOVE \"Y\" TO A\n           MOVE \"OLD HR PROMPT FROM PROMPTS-IDX\" TO X\n           WRITE R\n",
    );
    let t = Instant::now();
    let mut s = Session::start("prompts-form.cfrm");
    let rows = s.wait_for("Dg-List", "Rows", |v| v.contains("OLD HR PROMPT"));
    assert!(rows.starts_with("1\t") && rows.contains("[active]"), "the topic's version, moved over: {rows}");
    s.click("Btn-Main");
    let title = s.wait_for("Lbl-Title", "Caption", |v| v.starts_with("Main prompt"));
    let main_rows = s.wait_for("Dg-List", "Rows", |v| v.contains("main prompt"));
    assert!(main_rows.starts_with("1\t") && main_rows.contains("[active]"), "v1 is the shipped main prompt: {main_rows}");
    s.click("Btn-New");
    s.wait_for("Tab-Crud", "SelectedTab", |v| v == "1");
    s.type_into(
        "Txt-Prompt",
        "=== SYSTEM ===\nCUSTOM SYSTEM RULES.\n{TOPIC}\n=== ASSISTANT ===\nCUSTOM ASSISTANT RULES.\n=== PLAN ===\nSplit the user question - CUSTOM PLAN.\n",
    );
    s.click("Btn-Save");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Saved as v2, now active"));
    s.quit();

    let mut s = Session::start("chat-form.cfrm");
    let system = s.wait_for("AGENT-1", "SystemPrompt", |v| v.contains("CUSTOM") || v.contains("REPORTS"));
    let helper = s.wait_for("AGENT-2", "SystemPrompt", |v| v.contains("CUSTOM") || v.contains("careful"));
    s.quit();
    let (orch, other) = if system.contains("CUSTOM SYSTEM") { (system, helper) } else { (helper, system) };
    assert!(orch.contains("CUSTOM SYSTEM RULES.") && orch.contains("You answer HR questions."), "{orch}");
    assert!(!orch.contains("REPORTS"), "nothing of the shipped text is added behind the user's back: {orch}");
    assert_eq!(other.trim(), "CUSTOM ASSISTANT RULES.", "the assistants' instructions are the ASSISTANT section");

    // Restore default: the shipped text, as v3.
    let mut s = Session::start("prompts-form.cfrm");
    s.settle();
    s.click("Btn-Main");
    s.wait_for("Lbl-Title", "Caption", |v| v.starts_with("Main prompt"));
    s.click("Btn-Default");
    s.wait_for("Lbl-Status", "Caption", |v| v.contains("Saved as v3, now active"));
    s.quit();
    let mut s = Session::start("chat-form.cfrm");
    // Two agents on one model: which orchestrates is drawn (R57), so each
    // holds one of the two shipped sections - SYSTEM or ASSISTANT.
    let shipped = |v: &str| v.contains("REPORTS") || v.contains("careful research assistant");
    let a1 = s.wait_for("AGENT-1", "SystemPrompt", shipped);
    let a2 = s.wait_for("AGENT-2", "SystemPrompt", shipped);
    s.quit();
    assert!(a1.contains("REPORTS") != a2.contains("REPORTS"), "one orchestrator, one assistant, both shipped text");
    let back = a1.len() + a2.len();
    println!(
        "\n  ── 071 PowerChat, the main prompt ───────────────────────\n  old versions moved; {title:?}: v1 shipped, v2 edited and used by both agents, v3 restored ({back} characters) — {:.0} ms\n",
        t.elapsed().as_secs_f64() * 1000.0
    );
    let _ = std::fs::remove_dir_all(&root);
}
