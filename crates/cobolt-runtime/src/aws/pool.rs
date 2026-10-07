// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The AWS server pool (spec 078 R5, R8).
//!
//! One server process per **(connection, server)** pair, shared by every
//! control that uses it in this process — the root form, its child forms and
//! a built application all run in one process, so they share the pool.
//!
//! - **Started on first use.** The first start downloads packages (`uvx`), so
//!   the handshake is bounded by the control's `StartTimeoutMs`, separate from
//!   the per-call `TimeoutMs` (amendment A3).
//! - **One reader thread per server** runs the protocol: it hands each answer
//!   to the call waiting on that id and serves the server's own traffic, so a
//!   notification or a `ping` never stalls a call (R4).
//! - **Every call waits with a timeout** (R5): a server that never answers
//!   yields [`CallError::Timeout`], and a late answer is dropped.
//! - **A dead server fails its pending calls** and is replaced on the next
//!   call (R8): the failed call is an error, the one after it a fresh start.
//! - **A different launch restarts the server** — a connection whose controls
//!   now allow writes needs the server started without its read-only flag
//!   (amendment A4).
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

/// One running server.
struct Handle {
    launch: Launch,
    mode: Mode,
    child: Mutex<Child>,
    /// `None` once input is closed (shutdown): the server sees EOF and ends.
    writer: Arc<Mutex<Option<ChildStdin>>>,
    session: Mutex<Session>,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Reply>>>>,
    alive: Arc<AtomicBool>,
    stderr: StderrRing,
}

impl Handle {
    fn start(launch: &Launch, mode: Mode, start_timeout: Duration) -> Result<Arc<Handle>, CallError> {
        let spawned = process::spawn(launch).map_err(CallError::Start)?;
        let me = ServerInfo { name: "PowerRustCOBOL".into(), version: env!("CARGO_PKG_VERSION").into() };
        let session = match mode {
            Mode::Handshake => Session::handshake(me),
            Mode::Stateless => Session::stateless(me),
        };
        let handle = Arc::new(Handle {
            launch: launch.clone(),
            mode,
            child: Mutex::new(spawned.child),
            writer: Arc::new(Mutex::new(Some(spawned.stdin))),
            session: Mutex::new(session.clone()),
            pending: Arc::new(Mutex::new(HashMap::new())),
            alive: Arc::new(AtomicBool::new(true)),
            stderr: spawned.stderr,
        });
        // The reader: answers to their callers, the server's own requests
        // served, and on EOF every pending call failed.
        let (pending, alive, writer) = (handle.pending.clone(), handle.alive.clone(), handle.writer.clone());
        let router = session;
        let stdout = spawned.stdout;
        std::thread::Builder::new()
            .name("aws-mcp-reader".into())
            .spawn(move || {
                let mut r = BufReader::new(stdout);
                while let Ok(Some(raw)) = read_message(&mut r) {
                    match router.route(&raw) {
                        Inbound::Response(resp) => {
                            let waiter = resp.id.as_u64().and_then(|id| lock(&pending).remove(&id));
                            if let Some(tx) = waiter {
                                let _ = tx.send(into_result(resp));
                            }
                        }
                        Inbound::Reply(bytes) => {
                            if let Some(w) = lock(&writer).as_mut() {
                                let _ = write_message(w, &bytes);
                            }
                        }
                        Inbound::Ignored | Inbound::Malformed(_) => {}
                    }
                }
                alive.store(false, Ordering::SeqCst);
                for (_, tx) in lock(&pending).drain() {
                    let _ = tx.send(Err(ClientError::Io("the AWS service stopped".into())));
                }
            })
            .map_err(|e| CallError::Start(format!("the AWS service could not be started: {e}")))?;

        if mode == Mode::Handshake {
            // A server that fails to start usually says why on stderr — an
            // AWS profile that is not signed in, above all — and that text is
            // what turns "it stopped" into "sign in to this profile" (R23).
            // It travels after the first line, which is the plain message;
            // `ops::plain_error` reads the rest and never shows it raw.
            let with_stderr = |message: String| {
                let said = settled_stderr(&handle.stderr);
                CallError::Start(if said.trim().is_empty() { message } else { format!("{message}\n{said}") })
            };
            let (id, bytes) = lock(&handle.session).initialize_request();
            let answer = handle.round_trip(id, &bytes, start_timeout).map_err(|e| match e {
                CallError::Timeout => with_stderr(format!(
                    "the AWS service did not start within {} s (its first start downloads packages; \
                     raise StartTimeoutMs if this machine is slow)",
                    start_timeout.as_secs()
                )),
                CallError::Failed(e) | CallError::Start(e) => with_stderr(e),
            })?;
            lock(&handle.session)
                .accept_initialize(&answer)
                .map_err(|e| with_stderr(e.to_string()))?;
            handle.send(&Session::initialized_notification())?;
        }
        Ok(handle)
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

    fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst) && matches!(lock(&self.child).try_wait(), Ok(None))
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
    Some(h.stderr.text())
}

/// Stop every server (R9). Each host calls this on its way out.
pub fn shutdown() {
    let slots: Vec<Slot> = lock(pool()).drain().map(|(_, s)| s).collect();
    for slot in slots {
        if let Some(h) = lock(&slot).take() {
            h.stop();
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

/// How many servers are running — for tests and diagnostics.
pub fn running() -> usize {
    let slots: Vec<Slot> = lock(pool()).values().cloned().collect();
    slots.iter().filter(|s| lock(s).as_ref().is_some_and(|h| h.is_alive())).count()
}
