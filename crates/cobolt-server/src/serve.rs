// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The server: listen, accept, authenticate, run (WASM-003 R5-R21).
//!
//! The order of everything a connection and a call go through is the security
//! model, so it is written down here once:
//!
//! **Connection.** accept TCP → connection limit → TLS (when configured) →
//! read the HTTP head within the handshake timeout → route (`/health`, or the
//! WebSocket upgrade) → **Origin check** (R14) → upgrade, with the frame and
//! message size capped → the first message must be `hello` within the
//! handshake timeout, and its versions are negotiated (WASM-014 R2).
//!
//! **Call.** rate limit (connection) → *if the program is not public*, a valid
//! token (an unknown name is answered as "no token" to an unauthenticated
//! caller, so names cannot be enumerated) → program lookup (only exported
//! programs exist) → rate limit (user) → role → concurrency permits → the
//! program runs on a worker thread, isolated, under a deadline → the answer is
//! projected to the contract the caller was built against.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde_json::value::RawValue;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch, Semaphore};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;

use cobolt_wire::contract::{InterfaceDescription, PROTOCOL, PROTOCOL_MIN};
use cobolt_wire::error::{codes, CallError, ErrorClass};
use cobolt_wire::mapping;
use cobolt_wire::proto::{self, Accepted, Arg, AuthOk, ClientFrame, ErrorMsg, Mode, Reply, ServerHello};

use crate::backend::{parse_semver, Backend};
use crate::config::Resolved;
use crate::exec::{self, ExecOptions};
use crate::http::{self, Head, HeadError, Replay};
use crate::jwks::{HttpFetcher, Jwks, JwksFetcher};
use crate::jwt::{Claims, TokenValidator};
use crate::limits::{Bucket, KeyedLimiter};
use crate::log::{LogRecord, LogSink, StdoutSink};

/// Counters, for the health probe and the tests.
#[derive(Default)]
pub struct Stats {
    pub connections_total: AtomicU64,
    pub connections_open: AtomicUsize,
    pub calls_ok: AtomicU64,
    pub calls_failed: AtomicU64,
    pub refused_origin: AtomicU64,
}

/// Options that are not configuration: what a test or an embedding supplies.
#[derive(Default)]
pub struct Options {
    pub sink: Option<Arc<dyn LogSink>>,
    pub fetcher: Option<Arc<dyn JwksFetcher>>,
}

pub(crate) struct Shared {
    pub cfg: Arc<Resolved>,
    pub backend: Arc<Backend>,
    pub validator: Arc<TokenValidator>,
    pub sink: Arc<dyn LogSink>,
    pub tls: Option<tokio_native_tls::TlsAcceptor>,
    pub user_limiter: KeyedLimiter,
    pub calls: Arc<Semaphore>,
    pub next_conn: AtomicU64,
    pub next_call: AtomicU64,
    pub stats: Stats,
    pub jwks: Option<Arc<Jwks>>,
    pub shutdown: watch::Receiver<bool>,
}

/// A running server.
pub struct ServerHandle {
    pub addr: SocketAddr,
    pub health_addr: Option<SocketAddr>,
    shared: Arc<Shared>,
    stop: watch::Sender<bool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ServerHandle {
    pub fn stats(&self) -> &Stats {
        &self.shared.stats
    }

    /// How many times the issuer's keys were fetched.
    pub fn jwks_fetches(&self) -> u64 {
        self.shared.jwks.as_ref().map(|j| j.fetch_count()).unwrap_or(0)
    }

    /// Stop accepting, cancel running calls, and wait for the worker threads.
    pub fn stop(mut self) {
        self.stop_inner();
    }

    fn stop_inner(&mut self) {
        let _ = self.stop.send(true);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.stop_inner();
    }
}

/// Start the server on its own runtime and thread. Returns when it is listening.
pub fn start(cfg: Resolved, backend: Backend, options: Options) -> Result<ServerHandle, String> {
    let cfg = Arc::new(cfg);
    let sink: Arc<dyn LogSink> = options.sink.unwrap_or_else(|| Arc::new(StdoutSink));
    let tls = match &cfg.tls {
        Some(src) => Some(crate::tlsutil::acceptor(src)?),
        None => None,
    };
    let jwks = if cfg.auth.issuers.is_empty() {
        None
    } else {
        let uri = cfg.auth.jwks_uri.clone().ok_or("no key address")?;
        let fetcher = options.fetcher.unwrap_or_else(|| Arc::new(HttpFetcher));
        Some(Arc::new(Jwks::new(uri, cfg.auth.jwks_ttl, cfg.auth.jwks_min_refresh, fetcher)))
    };
    let validator = Arc::new(TokenValidator::new(cfg.auth.clone(), jwks.clone(), cfg.dev_token_secret.as_ref().map(|s| s.expose().to_owned())));
    let (stop_tx, stop_rx) = watch::channel(false);
    let shared = Arc::new(Shared {
        user_limiter: KeyedLimiter::new(cfg.limits.user_rate.0, cfg.limits.user_rate.1),
        calls: Arc::new(Semaphore::new(cfg.limits.max_calls_total)),
        next_conn: AtomicU64::new(1),
        next_call: AtomicU64::new(1),
        stats: Stats::default(),
        cfg: cfg.clone(),
        backend: Arc::new(backend),
        validator,
        sink,
        tls,
        jwks,
        shutdown: stop_rx,
    });

    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(SocketAddr, Option<SocketAddr>), String>>();
    let s2 = shared.clone();
    let thread = std::thread::Builder::new()
        .name("prc-server".into())
        .spawn(move || {
            // The interpreter recurses; give every worker a stack to match.
            let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(16 * 1024 * 1024).build() {
                Ok(r) => r,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("runtime: {e}")));
                    return;
                }
            };
            rt.block_on(run(s2, ready_tx));
        })
        .map_err(|e| format!("thread: {e}"))?;
    let (addr, health_addr) = ready_rx.recv().map_err(|_| "the server thread ended before listening".to_string())??;
    Ok(ServerHandle { addr, health_addr, shared, stop: stop_tx, thread: Some(thread) })
}

async fn run(shared: Arc<Shared>, ready: std::sync::mpsc::Sender<Result<(SocketAddr, Option<SocketAddr>), String>>) {
    let listener = match TcpListener::bind(shared.cfg.listen).await {
        Ok(l) => l,
        Err(e) => {
            let _ = ready.send(Err(format!("cannot listen on {}: {e}", shared.cfg.listen)));
            return;
        }
    };
    let addr = listener.local_addr().unwrap_or(shared.cfg.listen);
    let health = match shared.cfg.health_listen {
        Some(h) => match TcpListener::bind(h).await {
            Ok(l) => Some(l),
            Err(e) => {
                let _ = ready.send(Err(format!("cannot listen for health on {h}: {e}")));
                return;
            }
        },
        None => None,
    };
    let health_addr = health.as_ref().and_then(|l| l.local_addr().ok());
    for w in &shared.cfg.warnings {
        let mut r = LogRecord::new("warning");
        r.note = Some(w.clone());
        shared.sink.emit(&r);
    }
    let mut r = LogRecord::new("start");
    r.note = Some(format!("listening on {addr}{}", if shared.tls.is_some() { " (wss)" } else { " (ws)" }));
    shared.sink.emit(&r);
    let _ = ready.send(Ok((addr, health_addr)));

    if let Some(hl) = health {
        let sh = shared.clone();
        tokio::spawn(async move { health_loop(sh, hl).await });
    }

    let mut shutdown = shared.shutdown.clone();
    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            accepted = listener.accept() => {
                let (tcp, peer) = match accepted { Ok(x) => x, Err(_) => continue };
                let _ = tcp.set_nodelay(true);
                let open = shared.stats.connections_open.load(Ordering::SeqCst);
                if open >= shared.cfg.limits.max_connections {
                    let mut r = LogRecord::new("refused");
                    r.note = Some("connection limit".into());
                    shared.sink.emit(&r);
                    drop(tcp);
                    continue;
                }
                shared.stats.connections_open.fetch_add(1, Ordering::SeqCst);
                shared.stats.connections_total.fetch_add(1, Ordering::SeqCst);
                let sh = shared.clone();
                tokio::spawn(async move {
                    connection(sh.clone(), tcp, peer).await;
                    sh.stats.connections_open.fetch_sub(1, Ordering::SeqCst);
                });
            }
        }
    }
    let mut r = LogRecord::new("stop");
    r.note = Some("shutting down".into());
    shared.sink.emit(&r);
}

async fn health_loop(shared: Arc<Shared>, listener: TcpListener) {
    let mut shutdown = shared.shutdown.clone();
    loop {
        tokio::select! {
            _ = shutdown.changed() => return,
            a = listener.accept() => {
                let Ok((mut s, _)) = a else { continue };
                let sh = shared.clone();
                tokio::spawn(async move {
                    let _ = tokio::time::timeout(Duration::from_secs(5), async {
                        if let Ok(head) = http::read_head(&mut s).await {
                            if head.path == "/health" {
                                http::respond(&mut s, 200, "OK", "application/json", health_body(&sh).as_bytes(), "").await;
                            } else {
                                http::respond(&mut s, 404, "Not Found", "text/plain", b"not found", "").await;
                            }
                        }
                    }).await;
                });
            }
        }
    }
}

fn health_body(shared: &Shared) -> String {
    serde_json::json!({
        "status": "ok",
        "contract": shared.backend.interface.contract,
        "protocol": PROTOCOL,
        "connections": shared.stats.connections_open.load(Ordering::SeqCst),
        "calls_ok": shared.stats.calls_ok.load(Ordering::SeqCst),
        "calls_failed": shared.stats.calls_failed.load(Ordering::SeqCst),
    })
    .to_string()
}

// ── one connection ────────────────────────────────────────────────────────────

async fn connection(shared: Arc<Shared>, tcp: TcpStream, peer: SocketAddr) {
    let handshake = Duration::from_millis(shared.cfg.limits.handshake_timeout_ms);
    let conn_id = shared.next_conn.fetch_add(1, Ordering::SeqCst);
    if let Some(acceptor) = shared.tls.clone() {
        let tls = match tokio::time::timeout(handshake, acceptor.accept(tcp)).await {
            Ok(Ok(s)) => s,
            _ => return, // a failed or stalled TLS handshake says nothing worth logging per attempt
        };
        serve_stream(shared, tls, peer, conn_id).await;
    } else {
        serve_stream(shared, tcp, peer, conn_id).await;
    }
}

async fn serve_stream<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(shared: Arc<Shared>, mut s: S, peer: SocketAddr, conn_id: u64) {
    let handshake = Duration::from_millis(shared.cfg.limits.handshake_timeout_ms);
    let head = match tokio::time::timeout(handshake, http::read_head(&mut s)).await {
        Ok(Ok(h)) => h,
        Ok(Err(HeadError::TooLarge)) => {
            http::respond(&mut s, 431, "Request Header Fields Too Large", "text/plain", b"request too large", "").await;
            return;
        }
        _ => return,
    };
    if head.path == "/health" && head.method.eq_ignore_ascii_case("GET") {
        http::respond(&mut s, 200, "OK", "application/json", health_body(&shared).as_bytes(), "").await;
        return;
    }
    if !head.is_websocket_upgrade() || (head.path != "/" && head.path != "/ws") {
        http::respond(&mut s, 404, "Not Found", "text/plain", b"not found", "").await;
        return;
    }
    // R14: the Origin of a browser connection must be one the deployment lists.
    if !origin_allowed(&shared.cfg, &head) {
        shared.stats.refused_origin.fetch_add(1, Ordering::SeqCst);
        let mut r = LogRecord::new("refused");
        r.connection = Some(conn_id);
        r.note = Some("origin not allowed".into());
        shared.sink.emit(&r);
        http::respond(&mut s, 403, "Forbidden", "text/plain", b"origin not allowed", "").await;
        return;
    }
    let cfg = WebSocketConfig::default()
        .max_message_size(Some(shared.cfg.limits.max_frame_bytes))
        .max_frame_size(Some(shared.cfg.limits.max_frame_bytes))
        .read_buffer_size(16 * 1024);
    let replay = Replay::new(head.raw.clone(), s);
    let ws = match tokio::time::timeout(handshake, tokio_tungstenite::accept_async_with_config(replay, Some(cfg))).await {
        Ok(Ok(ws)) => ws,
        _ => return,
    };
    let mut r = LogRecord::new("connect");
    r.connection = Some(conn_id);
    shared.sink.emit(&r);
    session(shared.clone(), ws, peer, conn_id).await;
    let mut r = LogRecord::new("disconnect");
    r.connection = Some(conn_id);
    shared.sink.emit(&r);
}

pub(crate) fn origin_allowed(cfg: &Resolved, head: &Head) -> bool {
    if cfg.allow_any_origin {
        return true;
    }
    match head.header("origin") {
        Some(o) => cfg.allowed_origins.iter().any(|a| a == o),
        None => cfg.allow_missing_origin,
    }
}

// ── the session ───────────────────────────────────────────────────────────────

struct Conn {
    id: u64,
    peer: SocketAddr,
    hello: Option<Hello>,
    claims: Option<Claims>,
    rate: Bucket,
    auth_failures: u32,
    calls: Arc<Semaphore>,
    in_flight: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>,
    /// The ticket machinery that keeps calls of one connection in order.
    next_ticket: u64,
    served: Arc<watch::Sender<u64>>,
}

struct Hello {
    contract: String,
    /// The interface the client was built against, for projecting answers.
    client_interface: Option<InterfaceDescription>,
}

async fn session<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    shared: Arc<Shared>,
    ws: tokio_tungstenite::WebSocketStream<S>,
    peer: SocketAddr,
    conn_id: u64,
) {
    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::channel::<Message>(512);
    let ping = Duration::from_secs(shared.cfg.limits.ping_interval.max(1));
    let idle = Duration::from_secs(shared.cfg.limits.idle_timeout.max(2));
    let writer = tokio::spawn(async move {
        let mut tick = tokio::time::interval(ping);
        tick.tick().await;
        loop {
            tokio::select! {
                m = rx.recv() => match m {
                    Some(Message::Close(c)) => { let _ = sink.send(Message::Close(c)).await; break; }
                    Some(m) => { if sink.send(m).await.is_err() { break; } }
                    None => break,
                },
                _ = tick.tick() => { if sink.send(Message::Ping(Vec::new().into())).await.is_err() { break; } }
            }
        }
        let _ = sink.close().await;
    });

    let (served_tx, _served_rx) = watch::channel(0u64);
    let mut conn = Conn {
        id: conn_id,
        peer,
        hello: None,
        claims: None,
        rate: Bucket::new(shared.cfg.limits.connection_rate.0, shared.cfg.limits.connection_rate.1),
        auth_failures: 0,
        calls: Arc::new(Semaphore::new(shared.cfg.limits.max_calls_per_connection)),
        in_flight: Arc::new(Mutex::new(HashMap::new())),
        next_ticket: 0,
        served: Arc::new(served_tx),
    };

    let hello_deadline = Instant::now() + Duration::from_millis(shared.cfg.limits.handshake_timeout_ms);
    let mut shutdown = shared.shutdown.clone();
    loop {
        let wait = if conn.hello.is_none() { hello_deadline.saturating_duration_since(Instant::now()).max(Duration::from_millis(1)) } else { idle };
        let next = tokio::select! {
            _ = shutdown.changed() => break,
            n = tokio::time::timeout(wait, stream.next()) => n,
        };
        let msg = match next {
            Err(_) => break, // nothing for too long, or no hello in time
            Ok(None) => break,
            Ok(Some(Err(e))) => {
                use tokio_tungstenite::tungstenite::error::{CapacityError, Error};
                if matches!(e, Error::Capacity(CapacityError::MessageTooLong { .. })) {
                    send_error(&tx, None, &CallError::new(ErrorClass::Limit, codes::FRAME_TOO_LARGE, format!("a message is larger than the {} bytes this backend accepts", shared.cfg.limits.max_frame_bytes))).await;
                    let mut r = LogRecord::new("refused");
                    r.connection = Some(conn_id);
                    r.outcome = Some("limit".into());
                    r.code = Some(codes::FRAME_TOO_LARGE);
                    shared.sink.emit(&r);
                }
                break;
            }
            Ok(Some(Ok(m))) => m,
        };
        match msg {
            Message::Text(t) => {
                if !on_text(&shared, &mut conn, &tx, t.as_str()).await {
                    break;
                }
            }
            Message::Binary(_) => {
                send_error(&tx, None, &CallError::new(ErrorClass::Validation, codes::BAD_MESSAGE, "binary frames are not part of the protocol")).await;
            }
            Message::Close(_) => break,
            _ => {} // ping/pong are answered by the library; any frame counts as life
        }
    }
    // Stop whatever this connection still runs.
    if let Ok(m) = conn.in_flight.lock() {
        for f in m.values() {
            f.store(true, Ordering::SeqCst);
        }
    }
    let _ = tx.send(Message::Close(None)).await;
    drop(tx);
    let _ = tokio::time::timeout(Duration::from_secs(2), writer).await;
}

async fn send_error(tx: &mpsc::Sender<Message>, id: Option<u64>, e: &CallError) {
    let text = proto::to_text(&ErrorMsg::from_error(id, e));
    let _ = tx.send(Message::text(text)).await;
}

/// Returns false when the connection should end.
async fn on_text(shared: &Arc<Shared>, conn: &mut Conn, tx: &mpsc::Sender<Message>, text: &str) -> bool {
    let frame = match proto::parse_client(text) {
        Ok(f) => f,
        Err(e) => {
            send_error(tx, None, &e).await;
            return true;
        }
    };
    match frame {
        ClientFrame::Hello(h) => on_hello(shared, conn, tx, h).await,
        ClientFrame::Auth(a) => {
            if conn.hello.is_none() {
                send_error(tx, None, &CallError::new(ErrorClass::Validation, codes::BAD_MESSAGE, "send hello first")).await;
                return true;
            }
            match shared.validator.validate(&a.token).await {
                Ok(c) => {
                    conn.auth_failures = 0;
                    let reply = AuthOk { t: "auth-ok".into(), sub: c.sub.clone(), expires_in: c.exp - crate::jwt::now() };
                    let mut r = LogRecord::new("auth");
                    r.connection = Some(conn.id);
                    r.subject = Some(c.sub.clone());
                    r.outcome = Some("ok".into());
                    shared.sink.emit(&r);
                    conn.claims = Some(c);
                    let _ = tx.send(Message::text(proto::to_text(&reply))).await;
                    true
                }
                Err(e) => {
                    conn.auth_failures += 1;
                    let mut r = LogRecord::new("auth");
                    r.connection = Some(conn.id);
                    r.outcome = Some("authentication".into());
                    r.code = Some(e.code);
                    shared.sink.emit(&r);
                    send_error(tx, None, &e).await;
                    // A client that cannot present a valid token five times is not a client.
                    conn.auth_failures < 5
                }
            }
        }
        ClientFrame::Call(c) => {
            on_call(shared, conn, tx, c).await;
            true
        }
        ClientFrame::Cancel(c) => {
            if let Ok(m) = conn.in_flight.lock() {
                if let Some(f) = m.get(&c.id) {
                    f.store(true, Ordering::SeqCst);
                }
            }
            true
        }
        _ => {
            send_error(tx, None, &CallError::new(ErrorClass::Validation, codes::BAD_MESSAGE, "this message kind is not available on this backend")).await;
            true
        }
    }
}

/// Decide whether a client may talk to this backend (WASM-014 R2, R3).
pub fn negotiate(server_contract: &str, client_protocol: u32, client_contract: &str) -> (Accepted, String, Option<String>) {
    if client_protocol < PROTOCOL_MIN {
        return (
            Accepted::Refused,
            format!("this application speaks an older protocol ({client_protocol}) than this backend still serves (from {PROTOCOL_MIN}): update the application"),
            Some(server_contract.to_string()),
        );
    }
    if client_protocol > PROTOCOL {
        return (
            Accepted::Refused,
            format!("this application speaks a newer protocol ({client_protocol}) than this backend ({PROTOCOL}): update the backend"),
            None,
        );
    }
    let (Some(s), Some(c)) = (parse_semver(server_contract), parse_semver(client_contract)) else {
        return (Accepted::Refused, format!("the contract version \"{client_contract}\" is not major.minor.patch"), None);
    };
    if c.0 < s.0 {
        return (
            Accepted::Refused,
            format!("this application was built for contract {client_contract}; the backend is at {server_contract} and no longer serves that major version: update the application"),
            Some(server_contract.to_string()),
        );
    }
    if c.0 > s.0 {
        return (Accepted::Refused, format!("this application was built for contract {client_contract}, which is newer than the backend's {server_contract}: update the backend"), None);
    }
    if c.1 > s.1 {
        return (Accepted::Refused, format!("this application (contract {client_contract}) uses features the backend (contract {server_contract}) does not have yet: update the backend"), None);
    }
    if c.1 < s.1 {
        return (
            Accepted::Warn,
            format!("this application was built for contract {client_contract}; the backend is at {server_contract}. They are compatible; update the application when convenient"),
            Some(server_contract.to_string()),
        );
    }
    (Accepted::Ok, String::new(), None)
}

async fn on_hello(shared: &Arc<Shared>, conn: &mut Conn, tx: &mpsc::Sender<Message>, h: proto::Hello) -> bool {
    let (accepted, message, update_to) = negotiate(&shared.backend.interface.contract, h.protocol, &h.contract);
    let reply = ServerHello {
        t: "hello".into(),
        protocol: PROTOCOL,
        protocol_min: PROTOCOL_MIN,
        contract: shared.backend.interface.contract.clone(),
        accepted,
        message,
        update_to,
        server: shared.cfg.name.clone(),
        max_frame_bytes: shared.cfg.limits.max_frame_bytes as u64,
    };
    let _ = tx.send(Message::text(proto::to_text(&reply))).await;
    if accepted == Accepted::Refused {
        let mut r = LogRecord::new("refused");
        r.connection = Some(conn.id);
        r.outcome = Some("version".into());
        shared.sink.emit(&r);
        return false;
    }
    conn.hello = Some(Hello { client_interface: shared.backend.interface_for(&h.contract).cloned(), contract: h.contract });
    true
}

// ── a call ────────────────────────────────────────────────────────────────────

async fn on_call(shared: &Arc<Shared>, conn: &mut Conn, tx: &mpsc::Sender<Message>, call: proto::Call) {
    let started = Instant::now();
    let id = call.id;
    let call_no = shared.next_call.fetch_add(1, Ordering::SeqCst);
    let name = call.program.trim().to_ascii_uppercase();
    let bytes_in: usize = call.args.iter().map(|a| a.json.get().len()).sum();

    let log_fail = |shared: &Shared, conn: &Conn, subject: Option<&str>, e: &CallError, program: Option<&str>| {
        shared.stats.calls_failed.fetch_add(1, Ordering::SeqCst);
        let mut r = LogRecord::new("call");
        r.call = Some(call_no);
        r.connection = Some(conn.id);
        r.program = program.map(str::to_owned);
        r.subject = subject.map(str::to_owned);
        r.duration_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
        r.outcome = Some(e.class.wire().to_string());
        r.code = Some(e.code);
        r.bytes_in = Some(bytes_in);
        shared.sink.emit(&r);
    };

    macro_rules! refuse {
        ($e:expr, $subject:expr, $program:expr) => {{
            let e: CallError = $e;
            log_fail(shared, conn, $subject, &e, $program);
            send_error(tx, Some(id), &e).await;
            return;
        }};
    }

    if conn.hello.is_none() {
        refuse!(CallError::new(ErrorClass::Validation, codes::BAD_MESSAGE, "send hello first"), None, None);
    }
    if !conn.rate.take() {
        refuse!(CallError::new(ErrorClass::Limit, codes::RATE_LIMITED, "too many calls on this connection; slow down"), None, None);
    }

    let prog = shared.backend.program(&name).cloned();
    let public = prog.as_ref().is_some_and(|p| p.sig.public);

    // Authentication — before the program is even looked at, unless it is public.
    let mut claims: Option<Claims> = None;
    if !public {
        let c = match &call.token {
            Some(t) => shared.validator.validate(t).await,
            None => match &conn.claims {
                Some(c) => shared.validator.recheck(c).map(|_| c.clone()),
                None => Err(CallError::new(ErrorClass::Authentication, codes::TOKEN_MISSING, "no token: authenticate before calling")),
            },
        };
        match c {
            Ok(c) => claims = Some(c),
            Err(e) => refuse!(e, None, None),
        }
    } else if let Some(c) = &conn.claims {
        // A public program still sees who calls, when someone is signed in.
        if shared.validator.recheck(c).is_ok() {
            claims = Some(c.clone());
        }
    }
    let subject = claims.as_ref().map(|c| c.sub.clone());

    let Some(prog) = prog else {
        refuse!(CallError::new(ErrorClass::NotFound, codes::UNKNOWN_PROGRAM, "no such program"), subject.as_deref(), None);
    };

    let key = subject.clone().unwrap_or_else(|| format!("ip:{}", conn.peer.ip()));
    if !shared.user_limiter.take(&key) {
        refuse!(CallError::new(ErrorClass::Limit, codes::RATE_LIMITED, "too many calls from this user; slow down"), subject.as_deref(), Some(&name));
    }

    // Authorisation — before the program starts.
    if !public {
        let c = claims.as_ref().expect("authenticated above");
        if !c.has_any_role(&prog.sig.roles) {
            refuse!(CallError::new(ErrorClass::Authorisation, codes::ROLE_REFUSED, "the caller's roles do not allow this program"), subject.as_deref(), Some(&name));
        }
    }

    // Concurrency.
    let Ok(conn_permit) = conn.calls.clone().try_acquire_owned() else {
        refuse!(CallError::new(ErrorClass::Limit, codes::TOO_MANY_CALLS, "too many calls in flight on this connection"), subject.as_deref(), Some(&name));
    };
    let Ok(global_permit) = shared.calls.clone().try_acquire_owned() else {
        refuse!(CallError::new(ErrorClass::Limit, codes::TOO_MANY_CALLS, "the backend is busy; try again"), subject.as_deref(), Some(&name));
    };

    // Ordering: calls of one connection run in the order they were made, unless the
    // program is declared independent.
    let ticket = if prog.sig.independent {
        None
    } else {
        let t = conn.next_ticket;
        conn.next_ticket += 1;
        Some(t)
    };
    let served = conn.served.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    if let Ok(mut m) = conn.in_flight.lock() {
        m.insert(id, cancel.clone());
    }
    let in_flight = conn.in_flight.clone();
    let client_interface = conn.hello.as_ref().and_then(|h| h.client_interface.clone());
    let client_contract = conn.hello.as_ref().map(|h| h.contract.clone()).unwrap_or_default();
    let tx = tx.clone();
    let shared = shared.clone();
    let conn_id = conn.id;
    let file_lock_wait_ms = shared.cfg.limits.file_lock_wait_ms;
    let timeout_ms = call.timeout_ms.map(|t| t.min(shared.cfg.limits.call_timeout_ms)).unwrap_or(shared.cfg.limits.call_timeout_ms);

    tokio::spawn(async move {
        let _permits = (conn_permit, global_permit);
        // Wait for this call's turn.
        if let Some(t) = ticket {
            let mut rx = served.subscribe();
            let _ = rx.wait_for(|v| *v == t).await;
        }
        let args: Vec<String> = call.args.iter().map(|a| a.json.get().to_string()).collect();
        let modes: Vec<Mode> = call.args.iter().map(|a| a.mode).collect();
        let claim_map: HashMap<String, String> = claims.as_ref().map(|c| c.flat.clone().into_iter().collect()).unwrap_or_default();
        let prog2 = prog.clone();
        let cancel2 = cancel.clone();
        let work = tokio::task::spawn_blocking(move || exec::execute(&prog2, &args, ExecOptions { claims: claim_map, cancel: cancel2, file_lock_wait: std::time::Duration::from_millis(file_lock_wait_ms) }));
        let outcome: Result<exec::ExecResult, CallError> = match tokio::time::timeout(Duration::from_millis(timeout_ms), work).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err(CallError::new(ErrorClass::Server, codes::PROGRAM_FAULT, "the program failed unexpectedly")),
            Err(_) => {
                cancel.store(true, Ordering::SeqCst);
                Err(CallError::new(ErrorClass::Limit, codes::CALL_TIMEOUT, format!("the call ran over its time limit of {timeout_ms} ms and was stopped")))
            }
        };
        if let Ok(mut m) = in_flight.lock() {
            m.remove(&id);
        }
        // Next in line may go.
        if ticket.is_some() {
            served.send_modify(|v| *v += 1);
        }

        let mut rec = LogRecord::new("call");
        rec.call = Some(call_no);
        rec.connection = Some(conn_id);
        rec.program = Some(name.clone());
        rec.subject = subject.clone();
        rec.duration_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
        rec.bytes_in = Some(bytes_in);
        match outcome {
            Ok(res) => {
                // Project each answer to the layouts the caller's contract knew.
                let client_sig = client_interface.as_ref().and_then(|i| i.program(&name));
                let mut out_args: Vec<Option<Box<RawValue>>> = Vec::new();
                let mut failed: Option<CallError> = None;
                for (i, text) in res.args.iter().enumerate() {
                    if modes.get(i) != Some(&Mode::Ref) {
                        out_args.push(None);
                        continue;
                    }
                    let text = match client_sig.and_then(|s| s.params.get(i)) {
                        Some(p) if client_contract != shared.backend.interface.contract => match mapping::project_json(&p.layout, text, usize::MAX >> 4) {
                            Ok(t) => t,
                            Err(e) => {
                                failed = Some(CallError::new(ErrorClass::Server, codes::INTERNAL, format!("cannot project the answer: {e}")));
                                break;
                            }
                        },
                        _ => text.clone(),
                    };
                    match RawValue::from_string(text) {
                        Ok(r) => out_args.push(Some(r)),
                        Err(_) => {
                            failed = Some(CallError::new(ErrorClass::Server, codes::INTERNAL, "the answer is not JSON"));
                            break;
                        }
                    }
                }
                let returning = match (&res.returning, call.returning) {
                    (Some(text), true) => {
                        let text = match client_sig.and_then(|s| s.returning.as_ref()) {
                            Some(p) if client_contract != shared.backend.interface.contract => mapping::project_json(&p.layout, text, usize::MAX >> 4).unwrap_or_else(|_| text.clone()),
                            _ => text.clone(),
                        };
                        RawValue::from_string(text).ok()
                    }
                    _ => None,
                };
                if let Some(e) = failed {
                    shared.stats.calls_failed.fetch_add(1, Ordering::SeqCst);
                    rec.outcome = Some(e.class.wire().to_string());
                    rec.code = Some(e.code);
                    shared.sink.emit(&rec);
                    send_error(&tx, Some(id), &e).await;
                    return;
                }
                let mut warnings = Vec::new();
                if let Some(d) = prog.sig.deprecated.as_ref() {
                    warnings.push(match &d.removal {
                        Some(r) => format!("{name} is deprecated (removed in contract {r}): {}", d.message),
                        None => format!("{name} is deprecated: {}", d.message),
                    });
                }
                let reply = Reply { t: "reply".into(), id, args: out_args, returning, warnings };
                let text = proto::to_text(&reply);
                rec.bytes_out = Some(text.len());
                rec.outcome = Some("ok".into());
                shared.stats.calls_ok.fetch_add(1, Ordering::SeqCst);
                shared.sink.emit(&rec);
                let _ = tx.send(Message::text(text)).await;
            }
            Err(e) => {
                shared.stats.calls_failed.fetch_add(1, Ordering::SeqCst);
                rec.outcome = Some(e.class.wire().to_string());
                rec.code = Some(e.code);
                shared.sink.emit(&rec);
                send_error(&tx, Some(id), &e).await;
            }
        }
    });
}

#[allow(dead_code)]
fn _arg_is_used(_: &Arg) {}
