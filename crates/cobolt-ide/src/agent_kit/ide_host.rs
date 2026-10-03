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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cobolt_mcp::{McpHandler, ServerInfo, Tool, ToolResult};
use cobolt_project_tools::http::{serve_http, HttpGate};
use cobolt_project_tools::tools::Shared;
use cobolt_project_tools::{FileList, NoProject, ProjectHost, ProjectRoot, ProjectTools};
use serde_json::Value;

use crate::project_model::{Category, CoboltProject};

/// How long a recording tool waits for the UI thread to apply it.
const RECORD_TIMEOUT: Duration = Duration::from_secs(5);
/// How long `create_project` / `open_project` wait for the IDE to open it.
const PROJECT_SWITCH_TIMEOUT: Duration = Duration::from_secs(30);

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
    /// Create a project in `folder` and open it in the IDE (spec 084 R15).
    CreateProject {
        folder: PathBuf,
        name: String,
        reply: Sender<Result<Value, String>>,
    },
    /// Open the project at `path` in the IDE, as File → Open Project does
    /// (spec 084 R16).
    OpenProject {
        path: PathBuf,
        reply: Sender<Result<Value, String>>,
    },
}

/// The state shared between the UI thread and the listener's threads.
pub struct IdeShared {
    snapshot: Mutex<Snapshot>,
    tx: Mutex<Sender<HostRequest>>,
    ctx: Option<egui::Context>,
    tools: Arc<Shared>,
    /// The access token a request must carry (spec 084 R35); `None` until
    /// Configure Claude Code generated one. Never logged.
    token: Mutex<Option<String>>,
    /// The running listener: its port and the flag that stops it.
    listener: Mutex<Option<(u16, Arc<AtomicBool>)>>,
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
                token: Mutex::new(None),
                listener: Mutex::new(None),
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

    /// Whether a project is open — what the HTTP gate needs to know.
    pub fn project_open(&self) -> bool {
        self.snapshot().manifest.is_some()
    }

    /// Set (or clear) the access token requests must carry.
    pub fn set_token(&self, token: Option<String>) {
        let token = token.filter(|t| !t.trim().is_empty());
        *self.token.lock().unwrap_or_else(|p| p.into_inner()) = token;
    }

    fn token(&self) -> Option<String> {
        self.token.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// The port the tools are served on, while they are.
    pub fn listening_port(&self) -> Option<u16> {
        self.listener.lock().unwrap_or_else(|p| p.into_inner()).as_ref().map(|(p, _)| *p)
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

    /// The UI thread creates it and opens it (spec 084 R15, R17).
    fn create_project(&mut self, folder: &Path, name: &str) -> Result<Value, String> {
        let (reply, answer) = mpsc::channel();
        let req = HostRequest::CreateProject { folder: folder.to_path_buf(), name: name.to_owned(), reply };
        self.ask(req, answer)
    }

    /// The UI thread opens it, as File → Open Project (spec 084 R16, R17).
    fn open_project(&mut self, path: &Path) -> Result<Value, String> {
        let (reply, answer) = mpsc::channel();
        self.ask(HostRequest::OpenProject { path: path.to_path_buf(), reply }, answer)
    }
}

impl IdeHost {
    fn ask(&self, req: HostRequest, answer: Receiver<Result<Value, String>>) -> Result<Value, String> {
        if !self.shared.send(req) {
            return Err("the IDE is closing".to_owned());
        }
        answer
            .recv_timeout(PROJECT_SWITCH_TIMEOUT)
            .unwrap_or_else(|_| Err("the IDE did not answer in time; try again".to_owned()))
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

/// Bind `127.0.0.1:<port>` and serve the tools on a background thread,
/// replacing any listener already running (spec 084 R25: a port change takes
/// effect at once). Returns the address it listens on.
pub fn start_listener(shared: &Arc<IdeShared>, port: u16) -> Result<String, String> {
    let addr = format!("127.0.0.1:{port}");
    stop_listener(shared);
    let listener = TcpListener::bind(&addr).map_err(|e| e.to_string())?;
    let stop = Arc::new(AtomicBool::new(false));
    let for_handler = Arc::clone(shared);
    let (for_token, for_open) = (Arc::clone(shared), Arc::clone(shared));
    let gate = HttpGate {
        token: Arc::new(move || for_token.token()),
        project_open: Arc::new(move || for_open.project_open()),
        stop: Arc::clone(&stop),
    };
    std::thread::Builder::new()
        .name("coding-agent-tools".into())
        .spawn(move || {
            let _ = serve_http(listener, move || IdeTools::new(Arc::clone(&for_handler)), gate);
        })
        .map_err(|e| e.to_string())?;
    *shared.listener.lock().unwrap_or_else(|p| p.into_inner()) = Some((port, stop));
    Ok(addr)
}

/// Stop the running listener, if any: raise its flag and wake its accept
/// with a connection of our own, so the port is free when this returns.
pub fn stop_listener(shared: &Arc<IdeShared>) {
    let running = shared.listener.lock().unwrap_or_else(|p| p.into_inner()).take();
    if let Some((port, stop)) = running {
        stop.store(true, Ordering::SeqCst);
        let _ = std::net::TcpStream::connect(("127.0.0.1", port));
        for _ in 0..50 {
            if TcpListener::bind(("127.0.0.1", port)).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// Why the IDE cannot switch project now, or `None` when it can (spec 084
/// R17): unsaved work is never discarded by a coding agent's request.
pub fn project_switch_refusal(unsaved: &[PathBuf], settings_dirty: bool, project_dir: Option<&Path>) -> Option<String> {
    if unsaved.is_empty() && !settings_dirty {
        return None;
    }
    let mut names: Vec<String> = unsaved
        .iter()
        .map(|p| {
            project_dir
                .and_then(|d| p.strip_prefix(d).ok())
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    if settings_dirty {
        names.push("Project Settings".to_owned());
    }
    Some(format!(
        "the IDE has unsaved changes in {}; ask the developer to save or close them first",
        names.join(", ")
    ))
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

    /// POST one JSON-RPC body to `port` with `auth` headers; the raw answer.
    fn post_raw(port: u16, auth: &str, body: &str) -> String {
        use std::io::{Read, Write};
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            s,
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\n{auth}Content-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    fn free_port() -> u16 {
        TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
    }

    /// Spec 084 AC16 + R25: over the real listener the access token is
    /// required (none, wrong, or before Configure: 401), the right one serves
    /// the open project, and a port change moves the server at once.
    #[test]
    fn the_ide_listener_admits_only_its_token_and_rebinds() {
        let (shared, _rx) = IdeShared::new(None);
        let mut tools = IdeTools::new(Arc::clone(&shared));
        let (t, err) = call(&mut tools, "list_files", json!({}));
        assert!(err && t == "no project open", "{t}");
        let dir = temp_project("token");
        shared.publish(Snapshot { manifest: Some(dir.join("Demo.project.toml")), ..Default::default() });

        let port = free_port();
        start_listener(&shared, port).expect("binds");
        let body = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_files","arguments":{}}}).to_string();
        assert!(post_raw(port, "Authorization: Bearer x\r\n", &body).starts_with("HTTP/1.1 401"), "no token configured: nothing admitted");
        shared.set_token(Some("tok-ide".into()));
        assert!(post_raw(port, "", &body).starts_with("HTTP/1.1 401"), "no token");
        assert!(post_raw(port, "Authorization: Bearer nope\r\n", &body).starts_with("HTTP/1.1 401"), "wrong token");
        let ok = post_raw(port, "Authorization: Bearer tok-ide\r\n", &body);
        assert!(ok.contains("forms/MAIN.cfrm") && !ok.contains("isError"), "{ok}");

        // Renewing the token refuses the old one.
        shared.set_token(Some("tok-new".into()));
        assert!(post_raw(port, "Authorization: Bearer tok-ide\r\n", &body).starts_with("HTTP/1.1 401"), "old token after renew");

        // A port change: the new port serves, the old one is free again.
        let port2 = free_port();
        start_listener(&shared, port2).expect("rebinds");
        assert_eq!(shared.listening_port(), Some(port2));
        assert!(post_raw(port2, "Authorization: Bearer tok-new\r\n", &body).contains("forms/MAIN.cfrm"));
        assert!(TcpListener::bind(("127.0.0.1", port)).is_ok(), "the old port was released");
        stop_listener(&shared);
        assert_eq!(shared.listening_port(), None);
        println!("ide listener: before Configure 401; no/wrong/old token 401; right token lists files; rebound {port} -> {port2}, old port released");
    }

    /// Spec 084 AC9 (R17): a project switch is refused while anything is
    /// unsaved, naming each file (project-relative) and the Settings form.
    #[test]
    fn a_project_switch_is_refused_while_work_is_unsaved() {
        let dir = PathBuf::from("/p/Demo");
        assert_eq!(project_switch_refusal(&[], false, Some(&dir)), None);
        let why = project_switch_refusal(&[dir.join("forms/ORDERS.cfrm"), dir.join("src/x.cbl")], true, Some(&dir)).unwrap();
        assert!(why.contains("forms/ORDERS.cfrm") && why.contains("src/x.cbl") && why.contains("Project Settings"), "{why}");
        assert!(!why.contains("/p/Demo"), "names are project-relative: {why}");
        println!("project switch: clean -> allowed; 2 unsaved files + Settings -> refused naming all 3");
    }

    /// `create_project` and `open_project` reach the UI thread, which answers.
    #[test]
    fn create_and_open_requests_reach_the_ui_thread() {
        let (shared, rx) = IdeShared::new(None);
        let worker_shared = Arc::clone(&shared);
        let worker = std::thread::spawn(move || {
            let mut tools = IdeTools::new(worker_shared);
            let a = call(&mut tools, "open_project", json!({"path": "/abs/Demo"}));
            let b = call(&mut tools, "create_project", json!({"folder": "/abs/New", "name": "New"}));
            (a, b)
        });
        let mut seen = Vec::new();
        while !worker.is_finished() {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(HostRequest::OpenProject { path, reply }) => {
                    seen.push(format!("open {}", path.display()));
                    reply.send(Ok(json!({"opened": true, "project": "Demo.project.toml"}))).unwrap();
                }
                Ok(HostRequest::CreateProject { folder, name, reply }) => {
                    seen.push(format!("create {} {name}", folder.display()));
                    reply.send(Err("the IDE has unsaved changes in forms/X.cfrm".into())).unwrap();
                }
                Ok(_) | Err(_) => {}
            }
        }
        let ((open_text, open_err), (create_text, create_err)) = worker.join().unwrap();
        assert!(!open_err && open_text.contains("Demo.project.toml"), "{open_text}");
        assert!(create_err && create_text.contains("unsaved"), "{create_text}");
        assert_eq!(seen, ["open /abs/Demo", "create /abs/New New"]);
        println!("ide host: open_project and create_project forwarded to the UI thread; its answer and its refusal returned");
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
                Ok(_) | Err(_) => {}
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
    /// answers `tools/list` with every tool, with no project open, to the
    /// token; and refuses a non-loopback Host.
    #[test]
    fn the_ide_listener_lists_every_tool_as_the_gate_curl_would() {
        let (shared, _rx) = IdeShared::new(None);
        shared.set_token(Some("tok-gate".into()));
        let port = free_port();
        let addr = start_listener(&shared, port).expect("the listener binds");
        assert_eq!(addr, format!("127.0.0.1:{port}"), "loopback only");
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let out = post_raw(port, "Authorization: Bearer tok-gate\r\n", body);
        let v: Value = serde_json::from_str(out.split_once("\r\n\r\n").unwrap().1).unwrap();
        let names: Vec<&str> = v["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names.len(), 10, "{out}");
        use std::io::{Read, Write};
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "POST /mcp HTTP/1.1\r\nHost: evil.example\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        let mut foreign = String::new();
        s.read_to_string(&mut foreign).unwrap();
        assert!(foreign.starts_with("HTTP/1.1 403"));
        stop_listener(&shared);
        println!("gate 3 (automated): POST http://{addr}/mcp tools/list with the token -> {} tools; Host: evil.example -> 403", names.len());
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
