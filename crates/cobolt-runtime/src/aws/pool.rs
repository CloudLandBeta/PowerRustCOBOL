// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The AWS server pool (spec 078 R5, R8; spec 103).
//!
//! One server per **(connection, server)** pair, shared by every control that
//! uses it in this process — the root form, its child forms and a built
//! application all run in one process, so they share the pool. A server is a
//! child process spoken to over stdio, or (spec 103) an https link to a hosted
//! server, which has no process: [`Link`] is the one place the two differ.
//!
//! - **Started on first use.** A child process's first start may download
//!   packages (`uvx`), so the handshake is bounded by the control's
//!   `StartTimeoutMs`, separate from the per-call `TimeoutMs` (amendment A3).
//! - **One reader thread per child process** runs the protocol: it hands each
//!   answer to the call waiting on that id and serves the server's own traffic,
//!   so a notification or a `ping` never stalls a call (R4). An https link needs
//!   none — a request's answer is the body of the request that asked.
//! - **An expired https session is renewed once** (spec 103 R5): the call is
//!   repeated on a new session, and a second refusal is the call's error.
//! - **Every call waits with a timeout** (R5): a server that never answers
//!   yields [`CallError::Timeout`], and a late answer is dropped.
//! - **A dead server fails its pending calls** and is replaced on the next
//!   call (R8): the failed call is an error, the one after it a fresh start.
//! - **A different launch restarts the server** — a connection whose controls
//!   now allow writes needs the server started without its read-only flag
//!   (amendment A4), and keys set or cleared at run time (spec 092 A6) make a
//!   different launch too.
//!
//! Each server's map slot has a lock of its own, so a slow first start of one
//! connection never holds up the others.

use std::collections::HashMap;
use std::io::BufReader;
use std::process::{Child, ChildStdin};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use cobolt_mcp::client::{into_result, ClientError, Inbound, Mode, Session};
use cobolt_mcp::transport::{read_message, write_message};
use cobolt_mcp::{ServerInfo, Tool, ToolResult};
use serde_json::Value;

use super::https;
use super::process::{self, Launch, StderrRing};

/// Which server: a connection's id and the route table's server id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerKey {
    pub connection: String,
    pub server: String,
}

/// Why a call did not produce a tool result.
#[derive(Debug, Clone, PartialEq)]
pub enum CallError {
    /// The server could not be started, or its handshake failed; the text is
    /// for an end user (a missing program names its fix).
    Start(String),
    /// No answer within the time allowed.
    Timeout,
    /// The server stopped, or refused the request.
    Failed(String),
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallError::Start(e) | CallError::Failed(e) => f.write_str(e),
            CallError::Timeout => f.write_str("the AWS service did not answer in time"),
        }
    }
}

type Reply = Result<Value, ClientError>;

/// How a running server is reached.
enum Link {
    /// A child process spoken to over its stdin and stdout.
    Stdio(Stdio),
    /// A hosted server spoken to over HTTPS, every request signed (spec 103).
    Https(https::Link),
}

/// A started child process and what serves it.
struct Stdio {
    child: Mutex<Child>,
    /// `None` once input is closed (shutdown): the server sees EOF and ends.
    writer: Arc<Mutex<Option<ChildStdin>>>,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Reply>>>>,
    alive: Arc<AtomicBool>,
    stderr: StderrRing,
}

/// One running server.
struct Handle {
    launch: Launch,
    mode: Mode,
    session: Mutex<Session>,
    link: Link,
    /// One caller at a time opens a new https session when the old one has
    /// expired; the others find it open (R5).
    reopen: Mutex<()>,
}

impl Handle {
    fn start(launch: &Launch, mode: Mode, start_timeout: Duration) -> Result<Arc<Handle>, CallError> {
        let me = ServerInfo { name: "PowerRustCOBOL".into(), version: env!("CARGO_PKG_VERSION").into() };
        let session = match mode {
            Mode::Handshake => Session::handshake(me),
            Mode::Stateless => Session::stateless(me),
        };
        let link = if launch.https.is_some() {
            Link::Https(https::Link::open(launch, session.clone()).map_err(CallError::Start)?)
        } else {
            Link::Stdio(Stdio::spawn(launch, &session)?)
        };
        let handle = Arc::new(Handle { launch: launch.clone(), mode, session: Mutex::new(session), link, reopen: Mutex::new(()) });
        if mode == Mode::Handshake {
            handle.handshake(start_timeout)?;
        }
        Ok(handle)
    }

    /// `initialize`, the server's answer, `notifications/initialized` (R3) — the
    /// same for a child process and for an https server.
    fn handshake(&self, start_timeout: Duration) -> Result<(), CallError> {
        // A server that fails to start usually says why on stderr — an
        // AWS profile that is not signed in, above all — and that text is
        // what turns "it stopped" into "sign in to this profile" (R23).
        // It travels after the first line, which is the plain message;
        // `ops::plain_error` reads the rest and never shows it raw.
        let with_stderr = |message: String| {
            let said = self.settled_stderr();
            CallError::Start(if said.trim().is_empty() { message } else { format!("{message}\n{said}") })
        };
        let (id, bytes) = lock(&self.session).initialize_request();
        let answer = self.round_trip(id, &bytes, start_timeout).map_err(|e| match e {
            CallError::Timeout => with_stderr(match &self.link {
                Link::Stdio(_) => format!(
                    "the AWS service did not start within {} s (its first start downloads packages; \
                     raise StartTimeoutMs if this machine is slow)",
                    start_timeout.as_secs()
                ),
                Link::Https(_) => format!("the AWS service did not answer within {} s", start_timeout.as_secs()),
            }),
            CallError::Failed(e) | CallError::Start(e) => with_stderr(e),
        })?;
        lock(&self.session).accept_initialize(&answer).map_err(|e| with_stderr(e.to_string()))?;
        if let Link::Https(link) = &self.link {
            if let Some(revision) = lock(&self.session).revision() {
                link.set_protocol_version(revision);
            }
        }
        self.notify(&Session::initialized_notification(), start_timeout)
    }

    /// Send a frame that is owed no answer.
    fn notify(&self, bytes: &[u8], timeout: Duration) -> Result<(), CallError> {
        match &self.link {
            Link::Stdio(s) => s.send(bytes),
            Link::Https(l) => l.notify(bytes, timeout).map_err(link_error),
        }
    }

    /// Send one request and wait for its answer.
    fn round_trip(&self, id: u64, bytes: &[u8], timeout: Duration) -> Result<Value, CallError> {
        match &self.link {
            Link::Stdio(s) => s.round_trip(id, bytes, timeout),
            Link::Https(l) => self.https_round_trip(l, id, bytes, timeout),
        }
    }

    /// An https request. When the server no longer knows the session (it
    /// expired between two calls) a new one is opened and the request repeated
    /// once; a second refusal is an error (R5).
    fn https_round_trip(&self, link: &https::Link, id: u64, bytes: &[u8], timeout: Duration) -> Result<Value, CallError> {
        let before = link.generation();
        let reply = match link.round_trip(id, bytes, timeout) {
            Err(https::LinkError::SessionUnknown) => {
                self.reopen(link, before, timeout)?;
                link.round_trip(id, bytes, timeout)
            }
            other => other,
        };
        let response = reply.map_err(link_error)?;
        into_result(response).map_err(|e| CallError::Failed(e.to_string()))
    }

    /// Open a new https session, unless another caller already has since
    /// `seen` (the link's generation when this caller's request went out).
    fn reopen(&self, link: &https::Link, seen: u64, timeout: Duration) -> Result<(), CallError> {
        let _one_at_a_time = lock(&self.reopen);
        if link.generation() != seen {
            return Ok(());
        }
        link.forget_session();
        self.handshake(timeout).map_err(|e| match e {
            CallError::Start(m) => CallError::Failed(m),
            other => other,
        })
    }

    fn is_alive(&self) -> bool {
        match &self.link {
            Link::Stdio(s) => s.alive.load(Ordering::SeqCst) && matches!(lock(&s.child).try_wait(), Ok(None)),
            Link::Https(l) => l.is_alive(),
        }
    }

    /// What the child process has written to stderr, raw; an https server has none.
    fn stderr_text(&self) -> String {
        match &self.link {
            Link::Stdio(s) => s.stderr.text(),
            Link::Https(_) => String::new(),
        }
    }

    /// [`stderr_text`](Self::stderr_text), after waiting a moment for a failing process to say why.
    fn settled_stderr(&self) -> String {
        match &self.link {
            Link::Stdio(s) => settled_stderr(&s.stderr),
            Link::Https(_) => String::new(),
        }
    }

    /// Stop serving. A child process: close its input, give it two seconds to
    /// leave, then kill it (R9). An https server: end the session on a thread
    /// of its own, because the callers (a replaced handle, `AWS-SET-CREDENTIALS`)
    /// must not wait on the network.
    fn stop(&self) {
        match &self.link {
            Link::Stdio(s) => s.stop(),
            Link::Https(l) => l.close_in_background(),
        }
    }

    /// [`stop`](Self::stop) for the application's last moments: an https
    /// session is ended before the process goes, for at most two seconds.
    fn stop_and_wait(&self) {
        match &self.link {
            Link::Stdio(s) => s.stop(),
            Link::Https(l) => l.close(Duration::from_secs(2)),
        }
    }
}

/// A link failure as the pool's error.
fn link_error(e: https::LinkError) -> CallError {
    match e {
        https::LinkError::Timeout => CallError::Timeout,
        https::LinkError::SessionUnknown => CallError::Failed("the AWS service session expired and could not be renewed".into()),
        https::LinkError::Failed(m) => CallError::Failed(m),
    }
}

impl Stdio {
    /// Start the process and the thread that reads its answers.
    fn spawn(launch: &Launch, session: &Session) -> Result<Stdio, CallError> {
        let spawned = process::spawn(launch).map_err(CallError::Start)?;
        let pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Reply>>>> = Arc::new(Mutex::new(HashMap::new()));
        let alive = Arc::new(AtomicBool::new(true));
        let writer = Arc::new(Mutex::new(Some(spawned.stdin)));
        // The reader: answers to their callers, the server's own requests
        // served, and on EOF every pending call failed.
        let (reader_pending, reader_alive, reader_writer) = (pending.clone(), alive.clone(), writer.clone());
        let router = session.clone();
        let stdout = spawned.stdout;
        std::thread::Builder::new()
            .name("aws-mcp-reader".into())
            .spawn(move || {
                let mut r = BufReader::new(stdout);
                while let Ok(Some(raw)) = read_message(&mut r) {
                    match router.route(&raw) {
                        Inbound::Response(resp) => {
                            let waiter = resp.id.as_u64().and_then(|id| lock(&reader_pending).remove(&id));
                            if let Some(tx) = waiter {
                                let _ = tx.send(into_result(resp));
                            }
                        }
                        Inbound::Reply(bytes) => {
                            if let Some(w) = lock(&reader_writer).as_mut() {
                                let _ = write_message(w, &bytes);
                            }
                        }
                        Inbound::Ignored | Inbound::Malformed(_) => {}
                    }
                }
                reader_alive.store(false, Ordering::SeqCst);
                for (_, tx) in lock(&reader_pending).drain() {
                    let _ = tx.send(Err(ClientError::Io("the AWS service stopped".into())));
                }
            })
            .map_err(|e| CallError::Start(format!("the AWS service could not be started: {e}")))?;
        Ok(Stdio { child: Mutex::new(spawned.child), writer, pending, alive, stderr: spawned.stderr })
    }

    fn send(&self, bytes: &[u8]) -> Result<(), CallError> {
        let stopped = || CallError::Failed("the AWS service stopped".into());
        let mut w = lock(&self.writer);
        let w = w.as_mut().ok_or_else(stopped)?;
        write_message(w, bytes).map_err(|_| stopped())
    }

    /// Send one request and wait for its answer.
    fn round_trip(&self, id: u64, bytes: &[u8], timeout: Duration) -> Result<Value, CallError> {
        let (tx, rx) = mpsc::channel();
        lock(&self.pending).insert(id, tx);
        if let Err(e) = self.send(bytes) {
            lock(&self.pending).remove(&id);
            return Err(e);
        }
        match rx.recv_timeout(timeout) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(ClientError::Io(_))) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(CallError::Failed("the AWS service stopped unexpectedly".into()))
            }
            Ok(Err(e)) => Err(CallError::Failed(e.to_string())),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                lock(&self.pending).remove(&id);
                Err(CallError::Timeout)
            }
        }
    }

    /// Close input, give the server two seconds to leave, then kill it
    /// (R9). An MCP stdio server ends at EOF; the kill is the guarantee.
    fn stop(&self) {
        lock(&self.writer).take();
        let mut child = lock(&self.child);
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if let Ok(Some(_)) = child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// What a server that just failed wrote on stderr. Its drain thread may not
/// have read the last of it yet, so wait a moment for it to appear.
fn settled_stderr(ring: &StderrRing) -> String {
    let deadline = std::time::Instant::now() + Duration::from_millis(300);
    loop {
        let text = ring.text();
        if !text.trim().is_empty() || std::time::Instant::now() >= deadline {
            return text;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

type Slot = Arc<Mutex<Option<Arc<Handle>>>>;

fn pool() -> &'static Mutex<HashMap<ServerKey, Slot>> {
    static POOL: OnceLock<Mutex<HashMap<ServerKey, Slot>>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The running server for `key`, started (or restarted) as needed.
fn handle_for(key: &ServerKey, launch: &Launch, mode: Mode, start_timeout: Duration) -> Result<Arc<Handle>, CallError> {
    let slot = lock(pool()).entry(key.clone()).or_default().clone();
    let mut slot = lock(&slot);
    if let Some(h) = slot.as_ref() {
        if h.is_alive() && h.launch == *launch && h.mode == mode {
            return Ok(h.clone());
        }
        h.stop();
        *slot = None;
    }
    let h = Handle::start(launch, mode, start_timeout)?;
    *slot = Some(h.clone());
    Ok(h)
}

/// How long to wait.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// One call (`TimeoutMs`).
    pub call: Duration,
    /// A server's start and handshake (`StartTimeoutMs`).
    pub start: Duration,
}

/// Call one tool on the server for `key`.
pub fn call_tool(
    key: &ServerKey,
    launch: &Launch,
    mode: Mode,
    tool: &str,
    arguments: Value,
    budget: Budget,
) -> Result<ToolResult, CallError> {
    let h = handle_for(key, launch, mode, budget.start)?;
    let (id, bytes) = lock(&h.session).call_tool_request(tool, arguments);
    let v = h.round_trip(id, &bytes, budget.call)?;
    serde_json::from_value(v).map_err(|e| CallError::Failed(format!("the AWS service's answer was not understood: {e}")))
}

/// Every tool the server for `key` offers, across pages.
pub fn list_tools(key: &ServerKey, launch: &Launch, mode: Mode, budget: Budget) -> Result<Vec<Tool>, CallError> {
    let h = handle_for(key, launch, mode, budget.start)?;
    let mut tools = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let (id, bytes) = lock(&h.session).list_tools_request(cursor.as_deref());
        let v = h.round_trip(id, &bytes, budget.call)?;
        let page: cobolt_mcp::types::ListToolsResult = serde_json::from_value(v)
            .map_err(|e| CallError::Failed(format!("the AWS service's tool list was not understood: {e}")))?;
        tools.extend(page.tools);
        match page.next_cursor {
            Some(c) if !c.is_empty() => cursor = Some(c),
            _ => return Ok(tools),
        }
    }
}

/// What the server for `key` has written to stderr, raw — mask it before
/// showing it to anyone.
pub fn stderr_of(key: &ServerKey) -> Option<String> {
    let slot = lock(pool()).get(key).cloned()?;
    let h = lock(&slot).clone()?;
    Some(h.stderr_text())
}

/// Stop every server (R9). Each host calls this on its way out.
pub fn shutdown() {
    let slots: Vec<Slot> = lock(pool()).drain().map(|(_, s)| s).collect();
    for slot in slots {
        if let Some(h) = lock(&slot).take() {
            h.stop_and_wait();
        }
    }
}

/// Stop the server for one key, if it runs.
pub fn stop(key: &ServerKey) {
    let slot = lock(pool()).remove(key);
    if let Some(slot) = slot {
        if let Some(h) = lock(&slot).take() {
            h.stop();
        }
    }
}

/// Stop every server of one connection — its keys were set or cleared (spec
/// 092 A6), so a process holding the old ones in its environment ends now.
pub fn stop_connection(connection: &str) {
    let slots: Vec<Slot> = {
        let mut map = lock(pool());
        let keys: Vec<ServerKey> = map.keys().filter(|k| k.connection == connection).cloned().collect();
        keys.iter().filter_map(|k| map.remove(k)).collect()
    };
    for slot in slots {
        if let Some(h) = lock(&slot).take() {
            h.stop();
        }
    }
}

/// How many servers are running — for tests and diagnostics.
pub fn running() -> usize {
    let slots: Vec<Slot> = lock(pool()).values().cloned().collect();
    slots.iter().filter(|s| lock(s).as_ref().is_some_and(|h| h.is_alive())).count()
}
