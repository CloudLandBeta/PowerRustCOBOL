// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Rust client of the backend channel (WASM-003 R7, R8, R13; WASM-014).
//!
//! It is what makes `CALL … WITH WEBSOCKET LINKAGE` work in a desktop or native
//! host: [`WsClient`] implements [`cobolt_wire::call::Transport`]. The browser has
//! its own client in the player; the rules are the same.
//!
//! **One connection, one actor.** A [`WsClient`] is a handle (cheap to clone, every
//! clone shares the connection). The connection is owned by one task on a private
//! runtime thread; the interpreter's thread talks to it through channels and
//! blocks only on its own call, so a form keeps painting while a call is pending.
//!
//! **What the client does so the developer does not have to:** connect lazily,
//! say `hello` and check the answer, send the bearer token as the first message
//! after it (never in the URL), correlate answers by id, refresh an expired token
//! and repeat the call once, ping-pong with the server, reconnect with back-off,
//! repeat a call a lost connection interrupted **only** when its program is
//! declared safe to repeat, re-subscribe streams, and report — for a call it
//! could not finish — whether the backend *may have run it*.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc as std_mpsc, Arc};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::Uri;
use tokio_tungstenite::tungstenite::Message;

use cobolt_wire::call::{ArgMode, CallObserver, CallReply, CallRequest, DownloadRequest, DownloadResult, Transport, UploadRequest, UploadResult};
use cobolt_wire::contract::{InterfaceDescription, PROTOCOL};
use cobolt_wire::error::{codes, CallError, ErrorClass};
use cobolt_wire::proto::{self, Accepted, ServerFrame, ServerHello};

// ── configuration ─────────────────────────────────────────────────────────────

/// Where the bearer token comes from. `refresh` is asked when the backend says
/// the token expired; `None` means there is no fresher one.
pub trait TokenSource: Send + Sync {
    fn token(&self) -> Option<String>;
    fn refresh(&self) -> Option<String> {
        self.token()
    }
}

/// A token that is a fixed string.
pub struct StaticToken(pub String);

impl TokenSource for StaticToken {
    fn token(&self) -> Option<String> {
        Some(self.0.clone())
    }
    fn refresh(&self) -> Option<String> {
        None
    }
}

/// A token read from the environment (`env:NAME`) or from a file (`file:PATH`)
/// each time it is asked for, so a developer can replace it while the program
/// runs. Anything else is the token itself.
pub struct SpecToken(pub String);

impl TokenSource for SpecToken {
    fn token(&self) -> Option<String> {
        let spec = self.0.trim();
        let text = if let Some(name) = spec.strip_prefix("env:") {
            std::env::var(name).ok()?
        } else if let Some(path) = spec.strip_prefix("file:") {
            std::fs::read_to_string(path).ok()?.trim().to_string()
        } else {
            spec.to_string()
        };
        (!text.is_empty()).then_some(text)
    }
}

#[derive(Clone)]
pub struct ClientConfig {
    /// `wss://host:port/ws` (or `ws://127.0.0.1:port/ws` for a development run).
    pub url: String,
    /// The contract version this application was built against.
    pub contract: String,
    pub client_name: String,
    /// One more trusted root, PEM (a development certificate or a private CA).
    pub extra_root_pem: Option<Vec<u8>>,
    pub token: Option<Arc<dyn TokenSource>>,
    /// The backend's interface description, for which programs are safe to repeat.
    pub interface: Option<InterfaceDescription>,
    /// Sent as the `Origin` header (a browser sends its own).
    pub origin: Option<String>,
    pub connect_timeout: Duration,
    /// How long a call may take on the client's clock.
    pub call_timeout: Duration,
    /// How long a call interrupted by a lost connection waits for a new one.
    pub reconnect_deadline: Duration,
}

impl ClientConfig {
    pub fn new(url: impl Into<String>, contract: impl Into<String>) -> Self {
        ClientConfig {
            url: url.into(),
            contract: contract.into(),
            client_name: format!("prc-client/{}", env!("CARGO_PKG_VERSION")),
            extra_root_pem: None,
            token: None,
            interface: None,
            origin: None,
            connect_timeout: Duration::from_secs(10),
            call_timeout: Duration::from_secs(60),
            reconnect_deadline: Duration::from_secs(20),
        }
    }
}

// ── what the actor reports ────────────────────────────────────────────────────

enum Note {
    Progress(u8, String),
    Done(Result<CallReply, CallError>),
}

/// One thing a subscription delivers.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamItem {
    /// An item of the stream; `seq` counts from 1 for each (re)subscription.
    Event { seq: u64, json: String },
    /// The connection was lost and the stream subscribed again; `seq` restarts at 1.
    Restarted,
    /// The stream program ended.
    Ended,
    /// The subscription failed or was ended by the backend.
    Failed(CallError),
}

/// A live subscription to a stream program.
pub struct Subscription {
    pub id: u64,
    pub items: std_mpsc::Receiver<StreamItem>,
    client: WsClient,
}

impl Subscription {
    pub fn unsubscribe(self) {
        let _ = self.client.shared.tx.send(Cmd::Unsubscribe { id: self.id });
    }
}

#[derive(Debug)]
enum Rpc {
    PutReady(proto::PutReady),
    PutDone(proto::PutDone),
    FileUrl(proto::FileUrl),
}

enum Cmd {
    Call { id: u64, program: String, args: Vec<(ArgMode, String)>, returning: bool, key: String, safe: bool, deadline: Instant, notes: std_mpsc::Sender<Note> },
    Cancel { id: u64 },
    Rpc { id: u64, frame: String, reply: std_mpsc::Sender<Result<Rpc, CallError>> },
    Subscribe { id: u64, program: String, args: Vec<(ArgMode, String)>, items: std_mpsc::Sender<StreamItem> },
    Unsubscribe { id: u64 },
    Hello { reply: std_mpsc::Sender<Result<ServerHello, CallError>> },
    Disconnect,
}

// ── the handle ────────────────────────────────────────────────────────────────

struct Shared {
    tx: mpsc::UnboundedSender<Cmd>,
    cfg: ClientConfig,
    next_id: AtomicU64,
}

/// A handle on the connection. Clones share it.
#[derive(Clone)]
pub struct WsClient {
    shared: Arc<Shared>,
}

fn closed(message: &str) -> CallError {
    CallError::transport(codes::CLOSED, message.to_string())
}

/// The backend keeps an upload's state on the connection that began it, so a
/// frame that lands on a fresh connection (the old one was cut and the client
/// reconnected before the frame went out) is answered "begin an upload first".
/// That is a lost connection in effect: begin again with the upload's id and the
/// backend says how much it already holds.
fn upload_state_lost(e: &CallError) -> bool {
    e.class == ErrorClass::Validation && e.code == codes::BAD_MESSAGE && e.message.contains("begin an upload first")
}

fn random_hex(bytes: usize) -> String {
    let mut b = vec![0u8; bytes];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl WsClient {
    /// A client for `cfg`. Nothing connects until the first use.
    pub fn new(cfg: ClientConfig) -> WsClient {
        let (tx, rx) = mpsc::unbounded_channel();
        let actor_cfg = cfg.clone();
        let _ = std::thread::Builder::new().name("prc-ws-client".into()).stack_size(2 * 1024 * 1024).spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(r) => r,
                Err(_) => return,
            };
            rt.block_on(Actor::new(actor_cfg, rx).run());
        });
        WsClient { shared: Arc::new(Shared { tx, cfg, next_id: AtomicU64::new(1) }) }
    }

    fn id(&self) -> u64 {
        self.shared.next_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn config(&self) -> &ClientConfig {
        &self.shared.cfg
    }

    /// Connect now (if not connected) and return what the backend said in its `hello`.
    pub fn hello(&self) -> Result<ServerHello, CallError> {
        let (tx, rx) = std_mpsc::channel();
        self.shared.tx.send(Cmd::Hello { reply: tx }).map_err(|_| closed("the client has stopped"))?;
        rx.recv_timeout(self.shared.cfg.connect_timeout + Duration::from_secs(5)).map_err(|_| CallError::transport(codes::TIMEOUT, "no answer to hello").may_have_run(false))?
    }

    /// Drop the connection (the next use connects again).
    pub fn disconnect(&self) {
        let _ = self.shared.tx.send(Cmd::Disconnect);
    }

    /// Subscribe to a stream program.
    pub fn subscribe(&self, program: &str, args: Vec<(ArgMode, String)>) -> Result<Subscription, CallError> {
        let id = self.id();
        let (tx, rx) = std_mpsc::channel();
        self.shared.tx.send(Cmd::Subscribe { id, program: program.trim().to_ascii_uppercase(), args, items: tx }).map_err(|_| closed("the client has stopped"))?;
        Ok(Subscription { id, items: rx, client: self.clone() })
    }

    fn rpc(&self, build: impl FnOnce(u64) -> String, timeout: Duration) -> Result<Rpc, CallError> {
        let id = self.id();
        let (tx, rx) = std_mpsc::channel();
        self.shared.tx.send(Cmd::Rpc { id, frame: build(id), reply: tx }).map_err(|_| closed("the client has stopped"))?;
        match rx.recv_timeout(timeout) {
            Ok(r) => r,
            Err(_) => Err(CallError::transport(codes::TIMEOUT, "the backend did not answer in time").may_have_run(true)),
        }
    }

    fn max_frame(&self) -> u64 {
        self.hello().map(|h| h.max_frame_bytes).unwrap_or(1 << 20)
    }

    /// Upload a local file into one of the backend's upload slots, in pieces,
    /// resuming after a lost connection. Returns the backend's name for the file.
    pub fn upload(&self, req: &UploadRequest, observer: &mut dyn CallObserver) -> Result<UploadResult, CallError> {
        let meta = std::fs::metadata(&req.path).map_err(|e| CallError::new(ErrorClass::Validation, codes::INVALID_PAYLOAD, format!("cannot read {}: {e}", req.path.display())))?;
        let size = meta.len();
        // The whole-file checksum goes ahead of the data, so the backend can verify it at the end.
        let sha = {
            let mut f = std::fs::File::open(&req.path).map_err(|e| CallError::new(ErrorClass::Validation, codes::INVALID_PAYLOAD, e.to_string()))?;
            let mut h = Sha256::new();
            let mut buf = vec![0u8; 256 * 1024];
            loop {
                let n = std::io::Read::read(&mut f, &mut buf).map_err(|e| CallError::new(ErrorClass::Validation, codes::INVALID_PAYLOAD, e.to_string()))?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
            }
            hex(&h.finalize())
        };
        let frame_cap = self.max_frame().max(8 * 1024);
        // Base64 grows data by a third; leave room for the envelope.
        let chunk = (((frame_cap.saturating_sub(2048)) / 4 * 3) as usize).clamp(1024, 4 * 1024 * 1024);
        let mut resume: Option<String> = None;
        let mut attempts = 0u32;
        let timeout = Duration::from_secs(30);
        'outer: loop {
            attempts += 1;
            let begin = self.rpc(
                |id| proto::to_text(&proto::PutBegin { t: "put-begin".into(), id, slot: req.slot.clone(), name: req.name.clone(), size, mime: req.mime.clone(), sha256: sha.clone(), resume: resume.clone() }),
                timeout,
            );
            let ready = match begin {
                Ok(Rpc::PutReady(r)) => r,
                Ok(other) => return Err(CallError::new(ErrorClass::Server, codes::INTERNAL, format!("unexpected answer to put-begin: {other:?}"))),
                Err(e) if e.class == ErrorClass::Transport && attempts < 6 => {
                    std::thread::sleep(Duration::from_millis(200 * attempts as u64));
                    continue 'outer;
                }
                Err(e) => return Err(e),
            };
            resume = Some(ready.upload.clone());
            let mut offset = ready.have;
            let mut file = std::fs::File::open(&req.path).map_err(|e| CallError::new(ErrorClass::Validation, codes::INVALID_PAYLOAD, e.to_string()))?;
            std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(offset)).map_err(|e| CallError::new(ErrorClass::Validation, codes::INVALID_PAYLOAD, e.to_string()))?;
            while offset < size {
                let want = chunk.min((size - offset) as usize);
                let mut data = vec![0u8; want];
                std::io::Read::read_exact(&mut file, &mut data).map_err(|e| CallError::new(ErrorClass::Validation, codes::INVALID_PAYLOAD, e.to_string()))?;
                let b64 = B64.encode(&data);
                let at = offset;
                let ack = self.rpc(|id| proto::to_text(&proto::PutChunk { t: "put-chunk".into(), id, offset: at, data: b64 }), timeout);
                match ack {
                    Ok(Rpc::PutReady(r)) => offset = r.have,
                    Ok(other) => return Err(CallError::new(ErrorClass::Server, codes::INTERNAL, format!("unexpected answer to put-chunk: {other:?}"))),
                    Err(e) if (e.class == ErrorClass::Transport || upload_state_lost(&e)) && attempts < 6 => {
                        std::thread::sleep(Duration::from_millis(200 * attempts as u64));
                        continue 'outer;
                    }
                    Err(e) => return Err(e),
                }
                observer.progress(((offset as f64 / size.max(1) as f64) * 100.0) as u8, &format!("uploading {}", req.name));
            }
            let end = self.rpc(|id| proto::to_text(&proto::PutEnd { t: "put-end".into(), id }), timeout);
            match end {
                Ok(Rpc::PutDone(d)) => return Ok(UploadResult { file: d.file, size: d.size }),
                Ok(other) => return Err(CallError::new(ErrorClass::Server, codes::INTERNAL, format!("unexpected answer to put-end: {other:?}"))),
                Err(e) if (e.class == ErrorClass::Transport || upload_state_lost(&e)) && attempts < 6 => {
                    std::thread::sleep(Duration::from_millis(200 * attempts as u64));
                    continue 'outer;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Download a file the backend offers, through the short-lived address it issues.
    pub fn download(&self, req: &DownloadRequest, observer: &mut dyn CallObserver) -> Result<DownloadResult, CallError> {
        let file = req.file.clone();
        let url = match self.rpc(|id| proto::to_text(&proto::GetFile { t: "get-file".into(), id, file }), Duration::from_secs(30))? {
            Rpc::FileUrl(u) => u,
            other => return Err(CallError::new(ErrorClass::Server, codes::INTERNAL, format!("unexpected answer to get-file: {other:?}"))),
        };
        let part = req.dest.with_extension("part");
        let _ = std::fs::remove_file(&part);
        let uri: Uri = self.shared.cfg.url.parse().map_err(|_| CallError::transport(codes::NOT_CONFIGURED, "the backend address is not a URL"))?;
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| CallError::transport(codes::NO_CONNECTION, e.to_string()))?;
        let cfg = self.shared.cfg.clone();
        let total = url.size;
        let mut have = 0u64;
        let mut attempts = 0;
        while have < total || total == 0 && attempts == 0 {
            attempts += 1;
            if attempts > 6 {
                return Err(CallError::transport(codes::CLOSED, "the download keeps breaking off").may_have_run(true));
            }
            let r = rt.block_on(http_get_range(&cfg, &uri, &url.path, have, &part));
            match r {
                Ok(n) => have += n,
                Err(e) if e.class == ErrorClass::Transport => {
                    have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
                    std::thread::sleep(Duration::from_millis(200 * attempts as u64));
                }
                Err(e) => return Err(e),
            }
            observer.progress(((have as f64 / total.max(1) as f64) * 100.0).min(100.0) as u8, "downloading");
            if total == 0 {
                break;
            }
        }
        let mut f = std::fs::File::open(&part).map_err(|e| CallError::new(ErrorClass::Server, codes::INTERNAL, e.to_string()))?;
        let mut h = Sha256::new();
        let mut buf = vec![0u8; 256 * 1024];
        let mut n_total = 0u64;
        loop {
            let n = std::io::Read::read(&mut f, &mut buf).map_err(|e| CallError::new(ErrorClass::Server, codes::INTERNAL, e.to_string()))?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
            n_total += n as u64;
        }
        if n_total != total || hex(&h.finalize()) != url.sha256 {
            let _ = std::fs::remove_file(&part);
            return Err(CallError::new(ErrorClass::Server, codes::INTERNAL, "the downloaded file does not match the size and checksum the backend announced"));
        }
        std::fs::rename(&part, &req.dest).map_err(|e| CallError::new(ErrorClass::Server, codes::INTERNAL, format!("cannot place {}: {e}", req.dest.display())))?;
        Ok(DownloadResult { size: n_total })
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl Transport for WsClient {
    fn call(&mut self, request: &CallRequest) -> Result<CallReply, CallError> {
        self.call_observed(request, &mut cobolt_wire::call::NoObserver)
    }

    fn call_observed(&mut self, request: &CallRequest, observer: &mut dyn CallObserver) -> Result<CallReply, CallError> {
        let id = self.id();
        let program = request.program.trim().to_ascii_uppercase();
        let sig = self.shared.cfg.interface.as_ref().and_then(|i| i.program(&program));
        let safe = sig.map(|p| p.idempotent).unwrap_or(false);
        let long = sig.map(|p| p.long_running).unwrap_or(false);
        let key = request.idempotency_key.clone().unwrap_or_else(|| random_hex(16));
        let deadline = Instant::now() + if long { Duration::from_secs(600) } else { self.shared.cfg.call_timeout };
        let (tx, rx) = std_mpsc::channel();
        let args: Vec<(ArgMode, String)> = request.args.iter().map(|a| (a.mode, a.json.clone())).collect();
        self.shared
            .tx
            .send(Cmd::Call { id, program, args, returning: request.wants_returning, key, safe, deadline, notes: tx })
            .map_err(|_| closed("the client has stopped").may_have_run(false))?;
        let mut cancelled = false;
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Note::Progress(p, m)) => observer.progress(p, &m),
                Ok(Note::Done(r)) => {
                    if let Ok(reply) = &r {
                        for w in &reply.warnings {
                            observer.warning(w);
                        }
                    }
                    return r;
                }
                Err(std_mpsc::RecvTimeoutError::Timeout) => {
                    if !cancelled && observer.cancel_requested() {
                        cancelled = true;
                        let _ = self.shared.tx.send(Cmd::Cancel { id });
                    }
                }
                Err(std_mpsc::RecvTimeoutError::Disconnected) => return Err(closed("the client has stopped").may_have_run(true)),
            }
        }
    }

    fn upload(&mut self, request: &UploadRequest, observer: &mut dyn CallObserver) -> Result<UploadResult, CallError> {
        WsClient::upload(self, request, observer)
    }

    fn download(&mut self, request: &DownloadRequest, observer: &mut dyn CallObserver) -> Result<DownloadResult, CallError> {
        WsClient::download(self, request, observer)
    }
}

// ── the process-wide default ──────────────────────────────────────────────────

/// What a host gives the client through the environment (the IDE passes these to Run
/// Form, a developer sets them for `rcrun run-form`, a deployment sets them for a
/// built application):
///
/// | variable | meaning |
/// |---|---|
/// | `COBOLT_BACKEND_URL` | the `wss://` address of the backend (`ws://` only to loopback) |
/// | `COBOLT_BACKEND_TOKEN` | the bearer token, or `env:NAME` / `file:PATH` to read it each time it is needed |
/// | `COBOLT_BACKEND_INTERFACE` | the backend's `interface.json`: the contract this application was built against and which programs are safe to repeat |
/// | `COBOLT_BACKEND_CONTRACT` | the contract version, when there is no interface file (default `1.0.0`) |
/// | `COBOLT_BACKEND_CERT` | a PEM file with one more trusted root (a development certificate) |
/// | `COBOLT_BACKEND_ORIGIN` | the `Origin` to present (a browser sends its own) |
pub fn config_from_env() -> Option<ClientConfig> {
    let url = std::env::var("COBOLT_BACKEND_URL").ok().filter(|u| !u.trim().is_empty())?;
    let interface = std::env::var("COBOLT_BACKEND_INTERFACE")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| InterfaceDescription::from_json(&t).ok());
    let contract = interface
        .as_ref()
        .map(|i| i.contract.clone())
        .or_else(|| std::env::var("COBOLT_BACKEND_CONTRACT").ok())
        .unwrap_or_else(|| "1.0.0".to_string());
    let mut cfg = ClientConfig::new(url.trim(), contract);
    cfg.interface = interface;
    cfg.client_name = format!("rcrun/{}", env!("CARGO_PKG_VERSION"));
    if let Ok(t) = std::env::var("COBOLT_BACKEND_TOKEN") {
        if !t.trim().is_empty() {
            cfg.token = Some(Arc::new(SpecToken(t)));
        }
    }
    cfg.extra_root_pem = std::env::var("COBOLT_BACKEND_CERT").ok().and_then(|p| std::fs::read(p).ok());
    cfg.origin = std::env::var("COBOLT_BACKEND_ORIGIN").ok().filter(|o| !o.trim().is_empty());
    Some(cfg)
}

/// Register, once per process, a client built from the environment as the transport
/// every interpreter uses for `CALL … WITH WEBSOCKET LINKAGE`: the root form's, the
/// embedded child forms', a compiled application's. They all share **one connection**.
/// Returns whether a backend is configured; without one nothing is registered and a
/// remote call fails with `transport 1003 no backend configured`, never silently.
pub fn install_default_from_env() -> bool {
    let Some(cfg) = config_from_env() else { return false };
    let client = WsClient::new(cfg);
    cobolt_runtime::interpreter::remote::set_default_transport_factory(move || Some(Box::new(client.clone()) as Box<dyn Transport>))
}

// ── the connection ────────────────────────────────────────────────────────────

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

type Ws = tokio_tungstenite::WebSocketStream<Box<dyn Io>>;

struct Conn {
    sink: SplitSink<Ws, Message>,
    stream: SplitStream<Ws>,
}

fn authority(uri: &Uri) -> Result<(bool, String, u16), CallError> {
    let tls = match uri.scheme_str() {
        Some("wss") => true,
        Some("ws") => false,
        _ => return Err(CallError::transport(codes::NOT_CONFIGURED, "the backend address must start with wss:// (or ws:// for a loopback development run)")),
    };
    let host = uri.host().ok_or_else(|| CallError::transport(codes::NOT_CONFIGURED, "the backend address has no host"))?.trim_matches(|c| c == '[' || c == ']').to_string();
    let port = uri.port_u16().unwrap_or(if tls { 443 } else { 80 });
    Ok((tls, host, port))
}

async fn tcp_tls(cfg: &ClientConfig, uri: &Uri) -> Result<Box<dyn Io>, CallError> {
    let (tls, host, port) = authority(uri)?;
    if !tls {
        let loopback = host == "localhost" || host.parse::<std::net::IpAddr>().map(|a| a.is_loopback()).unwrap_or(false);
        if !loopback {
            return Err(CallError::transport(codes::NOT_CONFIGURED, "plain ws:// is only allowed to a loopback address; use wss://"));
        }
    }
    let no_conn = |m: String| CallError::transport(codes::NO_CONNECTION, m).may_have_run(false);
    let tcp = tokio::time::timeout(cfg.connect_timeout, tokio::net::TcpStream::connect((host.as_str(), port)))
        .await
        .map_err(|_| no_conn(format!("no answer from {host}:{port} within {} s", cfg.connect_timeout.as_secs())))?
        .map_err(|e| no_conn(format!("cannot reach {host}:{port}: {e}")))?;
    let _ = tcp.set_nodelay(true);
    if tls {
        let connector = crate::tlsutil::connector(cfg.extra_root_pem.as_deref()).map_err(|e| CallError::transport(codes::NOT_CONFIGURED, e))?;
        let s = tokio::time::timeout(cfg.connect_timeout, connector.connect(&host, tcp))
            .await
            .map_err(|_| no_conn("the TLS handshake took too long".to_string()))?
            .map_err(|e| no_conn(format!("TLS to {host}:{port} failed: {e}")))?;
        Ok(Box::new(s))
    } else {
        Ok(Box::new(tcp))
    }
}

async fn next_text(ws: &mut Ws, within: Duration) -> Result<String, CallError> {
    loop {
        match tokio::time::timeout(within, ws.next()).await {
            Err(_) => return Err(CallError::transport(codes::TIMEOUT, "the backend did not answer").may_have_run(false)),
            Ok(None) | Ok(Some(Err(_))) | Ok(Some(Ok(Message::Close(_)))) => return Err(closed("the backend closed the connection").may_have_run(false)),
            Ok(Some(Ok(Message::Text(t)))) => return Ok(t.as_str().to_string()),
            Ok(Some(Ok(_))) => continue,
        }
    }
}

/// Connect, say `hello`, check the answer, authenticate.
async fn open(cfg: &ClientConfig) -> Result<(Conn, ServerHello), CallError> {
    let uri: Uri = cfg.url.parse().map_err(|_| CallError::transport(codes::NOT_CONFIGURED, "the backend address is not a URL"))?;
    let io = tcp_tls(cfg, &uri).await?;
    let mut req = cfg.url.as_str().into_client_request().map_err(|e| CallError::transport(codes::NOT_CONFIGURED, e.to_string()))?;
    if let Some(o) = &cfg.origin {
        if let Ok(v) = o.parse() {
            req.headers_mut().insert("Origin", v);
        }
    }
    let (mut ws, _) = tokio::time::timeout(cfg.connect_timeout, tokio_tungstenite::client_async(req, io))
        .await
        .map_err(|_| CallError::transport(codes::NO_CONNECTION, "the WebSocket handshake took too long").may_have_run(false))?
        .map_err(|e| CallError::transport(codes::NO_CONNECTION, format!("the backend refused the WebSocket: {e}")).may_have_run(false))?;

    let hello = proto::Hello { t: "hello".into(), protocol: PROTOCOL, contract: cfg.contract.clone(), client: cfg.client_name.clone() };
    ws.send(Message::text(proto::to_text(&hello))).await.map_err(|e| closed(&e.to_string()).may_have_run(false))?;
    let text = next_text(&mut ws, cfg.connect_timeout).await?;
    let server: ServerHello = match proto::parse_server(&text) {
        Ok(ServerFrame::Hello(h)) => h,
        Ok(ServerFrame::Error(e)) => return Err(e.into_error()),
        _ => return Err(CallError::new(ErrorClass::Server, codes::INTERNAL, "the backend's first message was not a hello").may_have_run(false)),
    };
    if server.accepted == Accepted::Refused {
        let code = if PROTOCOL < server.protocol_min {
            codes::PROTOCOL_UNSUPPORTED
        } else if major(&cfg.contract) < major(&server.contract) {
            codes::CLIENT_TOO_OLD
        } else {
            codes::CLIENT_TOO_NEW
        };
        let mut e = CallError::new(ErrorClass::Version, code, server.message.clone()).may_have_run(false);
        e.detail = server.update_to.clone();
        return Err(e);
    }
    if let Some(src) = &cfg.token {
        let mut token = src.token();
        // A token that has already expired is exchanged for a fresh one, once (WASM-003 R13).
        for attempt in 0..2 {
            let Some(t) = token.take() else { break };
            ws.send(Message::text(proto::to_text(&proto::Auth { t: "auth".into(), token: t }))).await.map_err(|e| closed(&e.to_string()).may_have_run(false))?;
            let text = next_text(&mut ws, cfg.connect_timeout).await?;
            match proto::parse_server(&text) {
                Ok(ServerFrame::AuthOk(_)) => break,
                Ok(ServerFrame::Error(e)) => {
                    let e = e.into_error();
                    if attempt == 0 && e.class == ErrorClass::Authentication && e.code == codes::TOKEN_EXPIRED {
                        let src = src.clone();
                        token = tokio::task::spawn_blocking(move || src.refresh()).await.ok().flatten();
                        if token.is_some() {
                            continue;
                        }
                    }
                    return Err(e.may_have_run(false));
                }
                _ => return Err(CallError::new(ErrorClass::Server, codes::INTERNAL, "the backend's answer to auth was not auth-ok").may_have_run(false)),
            }
        }
    }
    let (sink, stream) = ws.split();
    Ok((Conn { sink, stream }, server))
}

// ── the actor ─────────────────────────────────────────────────────────────────

struct PendingCall {
    program: String,
    args: Vec<(ArgMode, String)>,
    returning: bool,
    key: String,
    safe: bool,
    deadline: Instant,
    notes: std_mpsc::Sender<Note>,
    sent: bool,
    attempts: u32,
    refreshed: bool,
    token: Option<String>,
}

impl PendingCall {
    fn frame(&self, id: u64) -> String {
        let args: Vec<proto::Arg> = self
            .args
            .iter()
            .filter_map(|(m, j)| {
                let json = serde_json::value::RawValue::from_string(j.clone()).ok()?;
                Some(proto::Arg { mode: if *m == ArgMode::Value { proto::Mode::Value } else { proto::Mode::Ref }, json })
            })
            .collect();
        proto::to_text(&proto::Call { t: "call".into(), id, program: self.program.clone(), args, returning: self.returning, key: Some(self.key.clone()), token: self.token.clone(), timeout_ms: None })
    }
}

struct Sub {
    program: String,
    args: Vec<(ArgMode, String)>,
    items: std_mpsc::Sender<StreamItem>,
}

struct Actor {
    cfg: ClientConfig,
    rx: mpsc::UnboundedReceiver<Cmd>,
    conn: Option<Conn>,
    hello: Option<ServerHello>,
    calls: HashMap<u64, PendingCall>,
    rpcs: HashMap<u64, std_mpsc::Sender<Result<Rpc, CallError>>>,
    subs: HashMap<u64, Sub>,
    /// A compatibility notice (older client, newer backend) to hand to the next answer.
    notice: Option<String>,
    /// The last thing the backend said about the connection itself (an oversized frame, a
    /// refused message) and when: if the connection then ends, that is why.
    conn_error: Option<(CallError, Instant)>,
}

async fn read(conn: &mut Option<Conn>) -> Option<Result<Message, tokio_tungstenite::tungstenite::Error>> {
    match conn {
        Some(c) => c.stream.next().await,
        None => std::future::pending().await,
    }
}

impl Actor {
    fn new(cfg: ClientConfig, rx: mpsc::UnboundedReceiver<Cmd>) -> Self {
        Actor { cfg, rx, conn: None, hello: None, calls: HashMap::new(), rpcs: HashMap::new(), subs: HashMap::new(), notice: None, conn_error: None }
    }

    async fn run(mut self) {
        let mut tick = tokio::time::interval(Duration::from_millis(200));
        loop {
            tokio::select! {
                cmd = self.rx.recv() => match cmd {
                    None => break,
                    Some(c) => self.on_cmd(c).await,
                },
                m = read(&mut self.conn) => match m {
                    Some(Ok(Message::Text(t))) => self.on_text(t.as_str().to_string()).await,
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => self.on_disconnect().await,
                    Some(Ok(_)) => {}
                },
                _ = tick.tick() => self.on_tick().await,
            }
        }
        if let Some(mut c) = self.conn.take() {
            let _ = c.sink.send(Message::Close(None)).await;
        }
    }

    async fn send(&mut self, text: String) -> bool {
        match self.conn.as_mut() {
            Some(c) => c.sink.send(Message::text(text)).await.is_ok(),
            None => false,
        }
    }

    /// Connect if not connected, with a few quick attempts (a backend that is just starting).
    async fn ensure(&mut self) -> Result<(), CallError> {
        if self.conn.is_some() {
            return Ok(());
        }
        let mut last = None;
        let started = Instant::now();
        for attempt in 0..4u64 {
            match open(&self.cfg).await {
                Ok((c, h)) => {
                    if h.accepted == Accepted::Warn {
                        self.notice = Some(h.message.clone());
                    }
                    self.hello = Some(h);
                    self.conn = Some(c);
                    return Ok(());
                }
                // A refusal, a bad token or a bad address will not change by trying again.
                Err(e) if e.class != ErrorClass::Transport || e.code == codes::NOT_CONFIGURED => return Err(e),
                Err(e) => {
                    last = Some(e);
                    if started.elapsed() > self.cfg.connect_timeout {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100 * (attempt + 1))).await;
                }
            }
        }
        Err(last.unwrap_or_else(|| CallError::transport(codes::NO_CONNECTION, "cannot connect").may_have_run(false)))
    }

    async fn on_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Hello { reply } => {
                let r = self.ensure().await.and_then(|_| self.hello.clone().ok_or_else(|| closed("no hello")));
                let _ = reply.send(r);
            }
            Cmd::Disconnect => {
                if let Some(mut c) = self.conn.take() {
                    let _ = c.sink.send(Message::Close(None)).await;
                }
                self.fail_all(closed("the connection was closed by the application").may_have_run(true), false);
            }
            Cmd::Call { id, program, args, returning, key, safe, deadline, notes } => {
                let mut pc = PendingCall { program, args, returning, key, safe, deadline, notes, sent: false, attempts: 1, refreshed: false, token: None };
                if let Err(e) = self.ensure().await {
                    let _ = pc.notes.send(Note::Done(Err(e)));
                    return;
                }
                let frame = pc.frame(id);
                if self.send(frame).await {
                    pc.sent = true;
                }
                self.calls.insert(id, pc);
                if !self.calls.get(&id).map(|c| c.sent).unwrap_or(false) {
                    self.on_disconnect().await;
                }
            }
            Cmd::Cancel { id } => {
                if self.calls.contains_key(&id) {
                    self.send(proto::to_text(&proto::Cancel { t: "cancel".into(), id })).await;
                }
            }
            Cmd::Rpc { id, frame, reply } => {
                if let Err(e) = self.ensure().await {
                    let _ = reply.send(Err(e));
                    return;
                }
                if self.send(frame).await {
                    self.rpcs.insert(id, reply);
                } else {
                    let _ = reply.send(Err(closed("the connection was lost before the request was sent").may_have_run(false)));
                    self.on_disconnect().await;
                }
            }
            Cmd::Subscribe { id, program, args, items } => {
                self.subs.insert(id, Sub { program, args, items });
                match self.ensure().await {
                    Ok(()) => {
                        let frame = self.sub_frame(id);
                        if !self.send(frame).await {
                            self.on_disconnect().await;
                        }
                    }
                    Err(e) => {
                        if let Some(s) = self.subs.remove(&id) {
                            let _ = s.items.send(StreamItem::Failed(e));
                        }
                    }
                }
            }
            Cmd::Unsubscribe { id } => {
                if self.subs.remove(&id).is_some() {
                    self.send(proto::to_text(&proto::Unsubscribe { t: "unsub".into(), id })).await;
                }
            }
        }
    }

    fn sub_frame(&self, id: u64) -> String {
        let s = &self.subs[&id];
        let args: Vec<proto::Arg> = s
            .args
            .iter()
            .filter_map(|(m, j)| Some(proto::Arg { mode: if *m == ArgMode::Value { proto::Mode::Value } else { proto::Mode::Ref }, json: serde_json::value::RawValue::from_string(j.clone()).ok()? }))
            .collect();
        proto::to_text(&proto::Subscribe { t: "sub".into(), id, program: s.program.clone(), args })
    }

    /// Finish every call and request with `e`. `keep_subs` leaves subscriptions alone.
    fn fail_all(&mut self, e: CallError, keep_subs: bool) {
        for (_, c) in self.calls.drain() {
            let _ = c.notes.send(Note::Done(Err(e.clone())));
        }
        for (_, r) in self.rpcs.drain() {
            let _ = r.send(Err(e.clone()));
        }
        if !keep_subs {
            for (_, s) in self.subs.drain() {
                let _ = s.items.send(StreamItem::Failed(e.clone()));
            }
        }
    }

    async fn on_tick(&mut self) {
        let now = Instant::now();
        let late: Vec<u64> = self.calls.iter().filter(|(_, c)| c.deadline < now).map(|(i, _)| *i).collect();
        for id in late {
            if let Some(c) = self.calls.remove(&id) {
                let _ = c.notes.send(Note::Done(Err(CallError::transport(codes::TIMEOUT, "the call took longer than this application waits").may_have_run(c.sent))));
                self.send(proto::to_text(&proto::Cancel { t: "cancel".into(), id })).await;
            }
        }
    }

    /// The connection ended. Calls a lost connection interrupted are repeated if
    /// their program is declared safe, and failed — saying whether the backend
    /// may have run them — otherwise. Streams subscribe again.
    async fn on_disconnect(&mut self) {
        self.conn = None;
        let mut resend: Vec<u64> = Vec::new();
        let ids: Vec<u64> = self.calls.keys().copied().collect();
        for id in ids {
            let c = self.calls.get_mut(&id).expect("listed");
            if c.safe && c.attempts < 2 {
                c.attempts += 1;
                resend.push(id);
            } else if let Some(c) = self.calls.remove(&id) {
                // If the backend had just said why it was ending the connection, that is the reason
                // (and the call, refused whole, did not run); otherwise the line was simply lost.
                let why = match &self.conn_error {
                    Some((e, at)) if at.elapsed() < Duration::from_secs(2) && matches!(e.class, ErrorClass::Limit | ErrorClass::Validation | ErrorClass::Authentication | ErrorClass::Version) => Some(e.clone().may_have_run(false)),
                    _ => None,
                };
                let _ = c.notes.send(Note::Done(Err(why.unwrap_or_else(|| closed("the connection was lost during the call").may_have_run(c.sent)))));
            }
        }
        for (_, r) in self.rpcs.drain() {
            let _ = r.send(Err(closed("the connection was lost during the request").may_have_run(true)));
        }
        if resend.is_empty() && self.subs.is_empty() {
            return;
        }
        // Reconnect with back-off while something is waiting for it.
        let deadline = Instant::now() + self.cfg.reconnect_deadline;
        let mut delay = Duration::from_millis(100);
        loop {
            match open(&self.cfg).await {
                Ok((c, h)) => {
                    if h.accepted == Accepted::Warn {
                        self.notice = Some(h.message.clone());
                    }
                    self.hello = Some(h);
                    self.conn = Some(c);
                    break;
                }
                Err(e) => {
                    if Instant::now() + delay > deadline || (e.class != ErrorClass::Transport) {
                        for id in resend.drain(..) {
                            if let Some(c) = self.calls.remove(&id) {
                                let _ = c.notes.send(Note::Done(Err(CallError::transport(codes::NO_CONNECTION, "the backend could not be reached again").may_have_run(true))));
                            }
                        }
                        let subs: Vec<u64> = self.subs.keys().copied().collect();
                        for id in subs {
                            if let Some(s) = self.subs.remove(&id) {
                                let _ = s.items.send(StreamItem::Failed(e.clone()));
                            }
                        }
                        return;
                    }
                    tokio::time::sleep(delay).await;
                    delay = (delay * 2).min(Duration::from_secs(5));
                }
            }
        }
        for id in resend {
            let frame = match self.calls.get(&id) {
                Some(c) => c.frame(id),
                None => continue,
            };
            if !self.send(frame).await {
                if let Some(c) = self.calls.remove(&id) {
                    let _ = c.notes.send(Note::Done(Err(closed("the connection was lost again").may_have_run(true))));
                }
            }
        }
        let subs: Vec<u64> = self.subs.keys().copied().collect();
        for id in subs {
            let frame = self.sub_frame(id);
            self.send(frame).await;
            if let Some(s) = self.subs.get(&id) {
                let _ = s.items.send(StreamItem::Restarted);
            }
        }
    }

    async fn on_text(&mut self, text: String) {
        let frame = match proto::parse_server(&text) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(target: "client", "unreadable frame from the backend: {e}");
                return;
            }
        };
        match frame {
            ServerFrame::Reply(r) => {
                if let Some(c) = self.calls.remove(&r.id) {
                    let mut warnings = r.warnings;
                    if let Some(n) = self.notice.take() {
                        warnings.insert(0, n);
                    }
                    let reply = CallReply {
                        args: r.args.into_iter().map(|a| a.map(|v| v.get().to_string())).collect(),
                        returning: r.returning.map(|v| v.get().to_string()),
                        warnings,
                    };
                    let _ = c.notes.send(Note::Done(Ok(reply)));
                }
            }
            ServerFrame::Error(e) => {
                let id = e.id;
                let err = e.into_error();
                match id {
                    Some(id) => self.on_error(id, err).await,
                    None => {
                        tracing::warn!(target: "client", "the backend reports: {} ({})", err.message, err.code);
                        self.conn_error = Some((err, Instant::now()));
                    }
                }
            }
            ServerFrame::Progress(p) => {
                if let Some(c) = self.calls.get(&p.id) {
                    let _ = c.notes.send(Note::Progress(p.percent, p.message));
                }
            }
            ServerFrame::Event(ev) => {
                if let Some(s) = self.subs.get(&ev.sub) {
                    let _ = s.items.send(StreamItem::Event { seq: ev.seq, json: ev.data.get().to_string() });
                }
            }
            ServerFrame::SubEnd(e) => {
                if let Some(s) = self.subs.remove(&e.sub) {
                    let _ = s.items.send(StreamItem::Ended);
                }
            }
            ServerFrame::PutReady(p) => self.rpc_done(p.id, Rpc::PutReady(p)),
            ServerFrame::PutDone(p) => self.rpc_done(p.id, Rpc::PutDone(p)),
            ServerFrame::FileUrl(p) => self.rpc_done(p.id, Rpc::FileUrl(p)),
            ServerFrame::Hello(_) | ServerFrame::AuthOk(_) | ServerFrame::Subscribed(_) | ServerFrame::Unsubscribed(_) => {}
        }
    }

    fn rpc_done(&mut self, id: u64, r: Rpc) {
        if let Some(tx) = self.rpcs.remove(&id) {
            let _ = tx.send(Ok(r));
        }
    }

    async fn on_error(&mut self, id: u64, err: CallError) {
        if let Some(tx) = self.rpcs.remove(&id) {
            let _ = tx.send(Err(err));
            return;
        }
        if let Some(s) = self.subs.remove(&id) {
            let _ = s.items.send(StreamItem::Failed(err));
            return;
        }
        let Some(mut c) = self.calls.remove(&id) else { return };
        // R13: an expired token is refreshed and the call repeated once.
        let expired = err.class == ErrorClass::Authentication && (err.code == codes::TOKEN_EXPIRED || err.code == codes::TOKEN_MISSING);
        if expired && !c.refreshed {
            if let Some(src) = self.cfg.token.clone() {
                let fresh = tokio::task::spawn_blocking(move || src.refresh()).await.ok().flatten();
                if let Some(t) = fresh {
                    c.refreshed = true;
                    c.token = Some(t.clone());
                    // The connection's own token moves on too, for the calls after this one.
                    self.send(proto::to_text(&proto::Auth { t: "auth".into(), token: t })).await;
                    let frame = c.frame(id);
                    self.calls.insert(id, c);
                    if !self.send(frame).await {
                        self.on_disconnect().await;
                    }
                    return;
                }
            }
        }
        let _ = c.notes.send(Note::Done(Err(err)));
    }
}

/// The major number of `major.minor.patch`, or `None` when it is not one.
fn major(version: &str) -> Option<u64> {
    version.split('.').next().and_then(|m| m.parse().ok())
}

// ── an HTTPS GET with a range, for downloads ──────────────────────────────────

/// Fetch `path` from the backend's own listener starting at byte `from`,
/// appending to `part`. Returns the number of bytes written.
async fn http_get_range(cfg: &ClientConfig, uri: &Uri, path: &str, from: u64, part: &std::path::Path) -> Result<u64, CallError> {
    let (tls, host, port) = authority(uri)?;
    let mut io = tcp_tls_plain(cfg, tls, &host, port).await?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nRange: bytes={from}-\r\nAccept: */*\r\nConnection: close\r\n\r\n");
    io.write_all(req.as_bytes()).await.map_err(|e| closed(&e.to_string()).may_have_run(true))?;
    // The head, up to the blank line.
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() > 16 * 1024 {
            return Err(CallError::new(ErrorClass::Server, codes::INTERNAL, "the download answer has no end to its head"));
        }
        let n = io.read(&mut byte).await.map_err(|e| closed(&e.to_string()).may_have_run(true))?;
        if n == 0 {
            return Err(closed("the download was cut off").may_have_run(true));
        }
        head.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&head).into_owned();
    let status: u16 = head.split(' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    match status {
        200 if from == 0 => {}
        206 => {}
        404 => return Err(CallError::new(ErrorClass::NotFound, codes::UNKNOWN_PROGRAM, "the download address has expired or is not valid")),
        other => return Err(CallError::new(ErrorClass::Server, codes::INTERNAL, format!("the download answered {other}"))),
    }
    let mut file = tokio::fs::OpenOptions::new().create(true).append(true).open(part).await.map_err(|e| CallError::new(ErrorClass::Server, codes::INTERNAL, e.to_string()))?;
    let mut written = 0u64;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match io.read(&mut buf).await {
            Ok(n) => n,
            Err(_) => return Err(closed("the download was cut off").may_have_run(true)),
        };
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).await.map_err(|e| CallError::new(ErrorClass::Server, codes::INTERNAL, e.to_string()))?;
        written += n as u64;
    }
    file.flush().await.ok();
    Ok(written)
}

async fn tcp_tls_plain(cfg: &ClientConfig, tls: bool, host: &str, port: u16) -> Result<Box<dyn Io>, CallError> {
    let uri: Uri = format!("{}://{host}:{port}/", if tls { "wss" } else { "ws" }).parse().map_err(|_| CallError::transport(codes::NOT_CONFIGURED, "bad address"))?;
    tcp_tls(cfg, &uri).await
}
