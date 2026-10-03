// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The IDE as a host of the coding-agent tools** (spec 080 plan §1.5, T3.2,
//! T3.3).
//!
//! The tools run on the HTTP listener's threads; the IDE's state lives on the
//! UI thread. The two meet in two places only:
//!
//! - a [`Snapshot`] the UI thread publishes every frame — the open project,
//!   the files with unsaved IDE edits, whether a build is running, the
//!   project's External Crates — which the tools read;
//! - a channel of [`HostRequest`]s the tools send and the UI thread applies:
//!   recording a file in the project (through the IDE's own project model and
//!   save, which re-seals — so the IDE stays the only writer of its manifest
//!   while it is open), reloading what the tools wrote, and one Output line
//!   per tool call.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cobolt_mcp::{McpHandler, ServerInfo, Tool, ToolResult};
use cobolt_project_tools::http::{serve_http, OpenKit};
use cobolt_project_tools::tools::Shared;
use cobolt_project_tools::{FileList, NoProject, ProjectHost, ProjectRoot, ProjectTools};
use serde_json::Value;

use crate::project_model::{Category, CoboltProject};

/// How long a recording tool waits for the UI thread to apply it.
const RECORD_TIMEOUT: Duration = Duration::from_secs(5);

/// What the UI thread last published about the open project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// The open project's manifest; `None` = no project open.
    pub manifest: Option<PathBuf>,
    /// Files with unsaved edits in the IDE (dirty designers and editor tabs).
    pub unsaved: Vec<PathBuf>,
    /// The IDE is building.
    pub building: bool,
    /// The open project's External Crates (`use`-line names).
    pub crates: Vec<String>,
}

impl Snapshot {
    fn holds_unsaved(&self, abs: &Path) -> bool {
        let want = std::fs::canonicalize(abs).unwrap_or_else(|_| abs.to_path_buf());
        self.unsaved
            .iter()
            .any(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()) == want)
    }
}

/// What a tool asks of the UI thread.
pub enum HostRequest {
    /// Record `rel` in the open project's `list` and save the project; the
    /// answer goes back on `reply`.
    Record {
        rel: String,
        list: FileList,
        reply: Sender<Result<(), String>>,
    },
    /// These files were written: reload any editor tab showing them.
    Written(Vec<PathBuf>),
    /// A tool was called (one Output line), and open forms that changed on
    /// disk should be reloaded first (T3.3).
    Called { tool: String, target: String },
}

/// The state shared between the UI thread and the listener's threads.
pub struct IdeShared {
    snapshot: Mutex<Snapshot>,
    tx: Mutex<Sender<HostRequest>>,
    ctx: Option<egui::Context>,
    tools: Arc<Shared>,
}

impl IdeShared {
    /// A shared state and the receiver the UI thread drains. `ctx` wakes an
    /// idle IDE when a request arrives.
    pub fn new(ctx: Option<egui::Context>) -> (Arc<Self>, Receiver<HostRequest>) {
        let (tx, rx) = mpsc::channel();
        (
            Arc::new(Self {
                snapshot: Mutex::new(Snapshot::default()),
                tx: Mutex::new(tx),
                ctx,
                tools: Arc::new(Shared::new()),
            }),
            rx,
        )
    }

    /// Publish this frame's snapshot (UI thread).
    pub fn publish(&self, snapshot: Snapshot) {
        let mut cur = self.snapshot.lock().unwrap_or_else(|p| p.into_inner());
        if *cur != snapshot {
            *cur = snapshot;
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn send(&self, req: HostRequest) -> bool {
        let ok = self
            .tx
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .send(req)
            .is_ok();
        if let Some(ctx) = &self.ctx {
            ctx.request_repaint();
        }
        ok
    }

    /// Which kit the open project carries — what the HTTP gate compares the
    /// URL's id with (R14).
    pub fn open_kit(&self) -> OpenKit {
        match self.snapshot().manifest.as_deref().and_then(Path::parent) {
            None => OpenKit::NoProject,
            Some(dir) => OpenKit::Project {
                kit_id: super::kit_id_of(dir),
            },
        }
    }
}

/// The tools' view of the IDE.
#[derive(Clone)]
pub struct IdeHost {
    shared: Arc<IdeShared>,
}

impl IdeHost {
    pub fn new(shared: Arc<IdeShared>) -> Self {
        Self { shared }
    }
}

impl ProjectHost for IdeHost {
    fn project(&self) -> Result<ProjectRoot, NoProject> {
        let manifest = self.shared.snapshot().manifest.ok_or(NoProject::NoneOpen)?;
        ProjectRoot::open(&manifest).map_err(|_| NoProject::NoneOpen)
    }

    fn record(&mut self, rel: &str, list: FileList) -> Result<(), String> {
        let (reply, answer) = mpsc::channel();
        if !self.shared.send(HostRequest::Record {
            rel: rel.to_owned(),
            list,
            reply,
        }) {
            return Err("the IDE is closing".to_owned());
        }
        answer
            .recv_timeout(RECORD_TIMEOUT)
            .unwrap_or_else(|_| Err("the IDE did not record the file in time; try again".to_owned()))
    }

    fn unsaved(&self, abs: &Path) -> bool {
        self.shared.snapshot().holds_unsaved(abs)
    }

    fn written(&mut self, abs: &[PathBuf]) {
        if !abs.is_empty() {
            self.shared.send(HostRequest::Written(abs.to_vec()));
        }
    }

    fn workspace_root(&self) -> Option<PathBuf> {
        crate::ui_prefs::load_workspace_root()
    }

    fn external_crates(&self) -> Vec<String> {
        self.shared.snapshot().crates
    }

    fn build_blocked(&self) -> Option<String> {
        self.shared
            .snapshot()
            .building
            .then(|| "the IDE is building this project; wait for it to finish, then call build again".to_owned())
    }

    fn version(&self) -> String {
        crate::version::VERSION.to_owned()
    }
}

// ── T3.3: the conflict rules with the open IDE ───────────────────────────────

/// What the IDE lets a tool call do, given what it is holding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    /// A file the call would write (or read to write) has unsaved IDE edits.
    RefuseUnsaved(PathBuf),
    /// The IDE is building.
    RefuseBuilding,
}

impl Decision {
    /// The answer a refused call gets (tool output, English — spec 080 R22).
    pub fn message(&self, root: Option<&ProjectRoot>) -> Option<String> {
        let name = |p: &PathBuf| {
            root.and_then(|r| r.relative(&std::fs::canonicalize(p).unwrap_or_else(|_| p.clone())))
                .or_else(|| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_default()
        };
        match self {
            Decision::Allow => None,
            Decision::RefuseUnsaved(p) => Some(format!(
                "'{}' has unsaved changes in the IDE; ask the developer to save or close it, then try again",
                name(p)
            )),
            Decision::RefuseBuilding => Some(
                "the IDE is building this project; wait for it to finish, then call build again".to_owned(),
            ),
        }
    }
}

/// The IDE's gate for one tool call (T3.3): read-only tools always run; a
/// writing tool refuses a target with unsaved IDE edits — `build` and a
/// target-less `regenerate` refuse any unsaved form or indexed definition —
/// and `build` refuses while the IDE builds.
pub fn decide(snapshot: &Snapshot, tool: &str, target: Option<&Path>) -> Decision {
    let unsaved_model = || {
        snapshot
            .unsaved
            .iter()
            .find(|p| {
                matches!(
                    p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref(),
                    Some("cfrm") | Some("cidx")
                )
            })
            .cloned()
    };
    match tool {
        "build" => {
            if snapshot.building {
                return Decision::RefuseBuilding;
            }
            unsaved_model().map_or(Decision::Allow, Decision::RefuseUnsaved)
        }
        "regenerate" => match target {
            Some(t) if snapshot.holds_unsaved(t) => Decision::RefuseUnsaved(t.to_path_buf()),
            Some(_) => Decision::Allow,
            None => unsaved_model().map_or(Decision::Allow, Decision::RefuseUnsaved),
        },
        _ => Decision::Allow,
    }
}

/// The tools as the IDE serves them: the gate above, one Output line per
/// call, then the shared tool set. Holds no tool logic.
pub struct IdeTools {
    tools: ProjectTools<IdeHost>,
    shared: Arc<IdeShared>,
}

impl IdeTools {
    pub fn new(shared: Arc<IdeShared>) -> Self {
        Self {
            tools: ProjectTools::with_shared(IdeHost::new(Arc::clone(&shared)), Arc::clone(&shared.tools)),
            shared,
        }
    }
}

impl McpHandler for IdeTools {
    fn server_info(&self) -> ServerInfo {
        self.tools.server_info()
    }

    fn list_tools(&mut self) -> Vec<Tool> {
        self.tools.list_tools()
    }

    fn capabilities(&self) -> Value {
        self.tools.capabilities()
    }

    fn instructions(&self) -> Option<String> {
        self.tools.instructions()
    }

    fn list_resources(&mut self) -> Vec<cobolt_mcp::Resource> {
        self.tools.list_resources()
    }

    fn read_resource(&mut self, uri: &str) -> Option<cobolt_mcp::ResourceContents> {
        self.tools.read_resource(uri)
    }

    fn call_tool(&mut self, name: &str, arguments: &Value) -> ToolResult {
        let arg = arguments
            .get("path")
            .or_else(|| arguments.get("name"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        self.shared.send(HostRequest::Called {
            tool: name.to_owned(),
            target: arg.clone(),
        });
        let root = self.tools.host().project().ok();
        let target = root
            .as_ref()
            .filter(|_| !arg.is_empty())
            .and_then(|r| r.resolve(&arg).ok());
        let decision = decide(&self.shared.snapshot(), name, target.as_deref());
        if let Some(msg) = decision.message(root.as_ref()) {
            return ToolResult::failed(msg);
        }
        self.tools.call_tool(name, arguments)
    }
}

/// Bind `127.0.0.1:<port>` and serve the tools on a background thread.
/// Returns the address it listens on.
pub fn start_listener(shared: &Arc<IdeShared>, port: u16) -> Result<String, String> {
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).map_err(|e| e.to_string())?;
    let for_handler = Arc::clone(shared);
    let for_gate = Arc::clone(shared);
    std::thread::Builder::new()
        .name("coding-agent-tools".into())
        .spawn(move || {
            let _ = serve_http(
                listener,
                move || IdeTools::new(Arc::clone(&for_handler)),
                move || for_gate.open_kit(),
            );
        })
        .map_err(|e| e.to_string())?;
    Ok(addr)
}

/// The UI thread's half of [`HostRequest::Record`]: put `rel` in the project
/// model as the IDE's own actions do (`add_generated` for generated code,
/// `add_file_to` otherwise), then save through `save` — the IDE's
/// `save_project`, which re-seals the main-form designation.
pub fn handle_record(
    project: Option<&mut CoboltProject>,
    rel: &str,
    list: FileList,
    save: impl FnOnce(&CoboltProject) -> Result<(), String>,
) -> Result<(), String> {
    let project = project.ok_or_else(|| NoProject::NoneOpen.message().to_owned())?;
    match list {
        FileList::Generated => project.add_generated(rel),
        FileList::Forms => project.add_file_to(rel, Category::Forms),
        FileList::Indexed => project.add_file_to(rel, Category::IndexedFiles),
        FileList::Sources => project.add_file_to(rel, Category::CommonCode),
        FileList::Assets => project.add_file_to(rel, Category::Assets),
        FileList::Documentation => project.add_file_to(rel, Category::Documentation),
    }
    save(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc-080-idehost-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("forms")).unwrap();
        let mut form = cobolt_forms::Form::new("MAIN", "MAIN", 300, 200);
        form.main_form = true;
        cobolt_forms::save_form(&form, &dir.join("forms/MAIN.cfrm")).unwrap();
        let mut proj = CoboltProject::new("Demo", "");
        proj.files.forms = vec!["forms/MAIN.cfrm".into()];
        crate::project_model::save_project(&proj, &dir.join("Demo.project.toml")).unwrap();
        dir
    }

    fn call(tools: &mut IdeTools, name: &str, args: Value) -> (String, bool) {
        let r = tools.call_tool(name, &args);
        let cobolt_mcp::Content::Text { text } = &r.content[0] else { panic!("a text answer") };
        (text.clone(), r.is_error == Some(true))
    }

    #[test]
    fn ide_host_answers_no_project_and_different_project() {
        let (shared, _rx) = IdeShared::new(None);
        let mut tools = IdeTools::new(Arc::clone(&shared));
        let (t, err) = call(&mut tools, "list_files", json!({}));
        assert!(err && t == "no project open", "{t}");
        assert_eq!(shared.open_kit(), OpenKit::NoProject);

        let dir = temp_project("kit");
        shared.publish(Snapshot {
            manifest: Some(dir.join("Demo.project.toml")),
            ..Default::default()
        });
        assert_eq!(shared.open_kit(), OpenKit::Project { kit_id: None }, "no kit yet");
        std::fs::create_dir_all(dir.join(".claude")).unwrap();
        std::fs::write(dir.join(super::super::KIT_MANIFEST), r#"{"kit_id":"k-1234"}"#).unwrap();
        assert_eq!(
            shared.open_kit(),
            OpenKit::Project { kit_id: Some("k-1234".into()) }
        );

        // Over the real listener: the open kit's id runs, another id is refused.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (h, g) = (Arc::clone(&shared), Arc::clone(&shared));
        std::thread::spawn(move || {
            serve_http(listener, move || IdeTools::new(Arc::clone(&h)), move || g.open_kit())
        });
        let post = |id: &str| {
            use std::io::{Read, Write};
            let body = json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
                              "params":{"name":"list_files","arguments":{}}})
            .to_string();
            let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(
                s,
                "POST /mcp/{id} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).unwrap();
            out
        };
        let right = post("k-1234");
        assert!(right.contains("forms/MAIN.cfrm") && !right.contains("isError"), "{right}");
        let other = post("k-9999");
        assert!(other.contains("different project"), "{other}");
        println!(
            "ide_host: no project → \"no project open\"; kit k-1234 open → its id lists files, \
             k-9999 → \"different project\" (over 127.0.0.1:{port})"
        );
    }

    #[test]
    fn ide_host_record_reaches_the_project_model_and_saves_once() {
        let dir = temp_project("record");
        let other = cobolt_forms::Form::new("ORDERS", "Orders", 300, 200);
        cobolt_forms::save_form(&other, &dir.join("forms/ORDERS.cfrm")).unwrap();
        let manifest = dir.join("Demo.project.toml");
        let (shared, rx) = IdeShared::new(None);
        shared.publish(Snapshot {
            manifest: Some(manifest.clone()),
            ..Default::default()
        });
        let mut project = crate::project_model::load_project(&manifest).unwrap();

        // The tool runs on its own thread, as on the listener.
        let tool_shared = Arc::clone(&shared);
        let worker = std::thread::spawn(move || {
            let mut tools = IdeTools::new(tool_shared);
            call(&mut tools, "add_to_project", json!({"path": "forms/ORDERS.cfrm"}))
        });
        // The UI thread: drain until the tool has its answer.
        let mut saves = 0;
        let mut calls = 0;
        while !worker.is_finished() {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(HostRequest::Record { rel, list, reply }) => {
                    let r = handle_record(Some(&mut project), &rel, list, |p| {
                        saves += 1;
                        crate::project_model::save_project(p, &manifest).map_err(|e| e.to_string())
                    });
                    reply.send(r).unwrap();
                }
                Ok(HostRequest::Called { .. }) => calls += 1,
                Ok(HostRequest::Written(_)) | Err(_) => {}
            }
        }
        let (text, err) = worker.join().unwrap();
        assert!(!err, "{text}");
        assert_eq!(saves, 1, "the manifest is saved once");
        assert_eq!(calls, 1, "one Output line per call");
        assert_eq!(project.files.forms, ["forms/MAIN.cfrm", "forms/ORDERS.cfrm"], "the model has it");
        let on_disk = crate::project_model::load_project(&manifest).unwrap();
        assert_eq!(on_disk.files.forms, project.files.forms);
        use cobolt_compiler::main_form_guard::{authorize_form_start, StartVerdict};
        assert_eq!(authorize_form_start(&dir.join("forms/MAIN.cfrm"), None), StartVerdict::Allowed);
        println!("ide_host: record via the channel → project model has 2 forms, 1 save, seal verifies, 1 activity line");

        // No project model on the UI side: the record is refused, nothing saved.
        let r = handle_record(None, "forms/X.cfrm", FileList::Forms, |_| panic!("no save"));
        assert_eq!(r.unwrap_err(), "no project open");
    }

    /// Gate 3's manual check, automated (the GUI is never driven): the
    /// listener `CoboltApp::new` starts — `start_listener`, on a real port —
    /// answers the plan's `curl -X POST …/mcp/x` `tools/list` with the seven
    /// tools, with no project open, and refuses a non-loopback Host.
    #[test]
    fn the_ide_listener_lists_seven_tools_as_the_gate_curl_would() {
        use std::io::{Read, Write};
        let (shared, _rx) = IdeShared::new(None);
        let port = {
            let probe = TcpListener::bind("127.0.0.1:0").unwrap();
            probe.local_addr().unwrap().port()
        };
        let addr = start_listener(&shared, port).expect("the listener binds");
        assert_eq!(addr, format!("127.0.0.1:{port}"), "loopback only");
        let send = |host: &str| {
            let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
            let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(
                s,
                "POST /mcp/x HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).unwrap();
            out
        };
        let out = send(&format!("127.0.0.1:{port}"));
        let body = out.split_once("\r\n\r\n").unwrap().1;
        let v: Value = serde_json::from_str(body).unwrap();
        let names: Vec<&str> = v["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(names.len(), 10, "{body}");
        assert!(send("evil.example").starts_with("HTTP/1.1 403"));
        println!("gate 3 (automated): POST http://{addr}/mcp/x tools/list → {} tools: {}", names.len(), names.join(", "));
        println!("gate 3 (automated): Host: evil.example → 403");
    }

    #[test]
    fn conflicts_decision_table() {
        let dir = temp_project("conflicts");
        let form = dir.join("forms/MAIN.cfrm");
        let other = dir.join("forms/OTHER.cfrm");
        let clean = Snapshot::default();
        let dirty = Snapshot {
            unsaved: vec![form.clone()],
            ..Default::default()
        };
        let dirty_source = Snapshot {
            unsaved: vec![dir.join("src/a.cbl")],
            ..Default::default()
        };
        let building = Snapshot {
            building: true,
            ..Default::default()
        };
        let cases: Vec<(&str, &Snapshot, &str, Option<&Path>, Decision)> = vec![
            ("clean", &clean, "regenerate", Some(&form), Decision::Allow),
            ("clean", &clean, "build", None, Decision::Allow),
            ("form unsaved", &dirty, "regenerate", Some(&form), Decision::RefuseUnsaved(form.clone())),
            ("form unsaved", &dirty, "regenerate", Some(&other), Decision::Allow),
            ("form unsaved", &dirty, "regenerate", None, Decision::RefuseUnsaved(form.clone())),
            ("form unsaved", &dirty, "build", None, Decision::RefuseUnsaved(form.clone())),
            ("form unsaved", &dirty, "check", None, Decision::Allow),
            ("form unsaved", &dirty, "validate", Some(&form), Decision::Allow),
            ("form unsaved", &dirty, "kb_lookup", None, Decision::Allow),
            ("source unsaved", &dirty_source, "build", None, Decision::Allow),
            ("building", &building, "build", None, Decision::RefuseBuilding),
            ("building", &building, "regenerate", None, Decision::Allow),
            ("building", &building, "check", None, Decision::Allow),
        ];
        for (state, snap, tool, target, want) in &cases {
            let got = decide(snap, tool, *target);
            assert_eq!(&got, want, "{state} / {tool} / {target:?}");
            println!(
                "  {state:<15} {tool:<11} {:<18} → {got:?}",
                target.and_then(|t| t.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or("—".into())
            );
        }
        let root = ProjectRoot::open(&dir).unwrap();
        let msg = Decision::RefuseUnsaved(form.clone()).message(Some(&root)).unwrap();
        assert!(msg.contains("'forms/MAIN.cfrm'") && !msg.contains(&*dir.to_string_lossy()), "{msg}");
        println!("conflicts: {} decisions checked; a refusal names the project-relative file", cases.len());
    }
}
