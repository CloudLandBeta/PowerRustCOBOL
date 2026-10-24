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
//! hyper reads the HTTP head within the handshake timeout and under its size
//! cap → the closed route table (`/health`, `/dl/<token>`, the WebSocket
//! upgrade on `/` or `/ws`, 404 for the rest) → **Origin check** (R14) →
//! upgrade, with the frame and message size capped → the first message must
//! be `hello` within the handshake timeout, and its versions are negotiated
//! (WASM-014 R2).
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
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::Role;

use axum::body::Body;
use axum::extract::{FromRef, Path, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::Router;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::service::TowerToHyperService;

use cobolt_wire::contract::{InterfaceDescription, PROTOCOL, PROTOCOL_MIN};
use cobolt_wire::error::{codes, CallError, ErrorClass};
use cobolt_wire::mapping;
use cobolt_wire::call::BackendFiles;
use cobolt_wire::proto::{self, Accepted, AuthOk, ClientFrame, ErrorMsg, Event, FileUrl, Mode, Progress, PutDone, PutReady, Reply, ServerHello, SubEnd, Subscribed, Unsubscribed};

use crate::backend::{parse_semver, Backend, ExportedProgram, StateMode};
use crate::config::Resolved;
use crate::exec::{self, ExecOptions};
use crate::files::FileStore;
use crate::http;
use crate::idem;
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
    /// Stream subscriptions running now (WASM-014 R14: none for a client that has gone).
    pub subscriptions_active: AtomicUsize,
    /// Calls answered from the idempotency store without running the program again.
    pub replays: AtomicU64,
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
    pub idem: idem::Store,
    pub files: Arc<FileStore>,
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

    /// The backend's files (uploads, offers), for a test or an operator tool.
    pub fn files(&self) -> &FileStore {
        &self.shared.files
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
    // A generated program's control names a configuration of backend.toml
    // (WASM-005 T11): one that is not there stops the start, not the first call.
    let mut missing: Vec<String> = backend
        .programs()
        .filter_map(|p| p.control.as_ref().map(|c| (p.sig.name.as_str(), c)))
        .filter(|(_, c)| !c.configuration.trim().is_empty() && cfg.configurations.get(&c.configuration).is_none())
        .map(|(name, c)| {
            format!(
                "backend.toml has no [[configuration]] named \"{}\", which {} (control {}) needs: add it, with the control's secrets",
                c.configuration, name, c.id
            )
        })
        .collect();
    // A web form's FileDropZone slots (WASM-005 T12) join the deployment's; a
    // name in both is the deployment's mistake to fix, not one to guess at.
    let mut files_cfg = cfg.files.clone();
    for s in &backend.slots {
        if files_cfg.slots.iter().any(|d| d.name.eq_ignore_ascii_case(&s.name)) {
            missing.push(format!("backend.toml's [files] slots have one named \"{}\", the name of a web form's FileDropZone slot: rename it", s.name));
            continue;
        }
        files_cfg.slots.push(crate::config::UploadSlot {
            name: s.name.clone(),
            mime: vec!["*/*".into()],
            roles: s.roles.clone(),
            extensions: s.extensions.clone(),
            max_bytes: s.max_bytes,
        });
    }
    if !missing.is_empty() {
        missing.sort();
        return Err(missing.join("\n"));
    }
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
        idem: idem::Store::new(Duration::from_secs(cfg.limits.idempotency_ttl), cfg.limits.idempotency_max_entries),
        files: Arc::new(FileStore::new(&files_cfg)),
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

/// The plain-HTTP health listener (`server.health_listen`): `/health` and nothing else.
async fn health_loop(shared: Arc<Shared>, listener: TcpListener) {
    let mut shutdown = shared.shutdown.clone();
    loop {
        tokio::select! {
            _ = shutdown.changed() => return,
            a = listener.accept() => {
                let Ok((s, _)) = a else { continue };
                let router = Router::new().route("/health", any(health)).fallback(not_found).layer(axum::middleware::map_response(security_headers)).with_state(shared.clone());
                tokio::spawn(async move {
                    let _ = tokio::time::timeout(Duration::from_secs(5), http1_builder(Duration::from_secs(5)).serve_connection(TokioIo::new(s), TowerToHyperService::new(router))).await;
                });
            }
        }
    }
}

/// What the health probe answers (the server function [`crate::front::health`]).
pub(crate) fn health_value(shared: &Shared) -> serde_json::Value {
    serde_json::json!({
        "status": "ok",
        "contract": shared.backend.interface.contract,
        "protocol": PROTOCOL,
        "connections": shared.stats.connections_open.load(Ordering::SeqCst),
        "calls_ok": shared.stats.calls_ok.load(Ordering::SeqCst),
        "calls_failed": shared.stats.calls_failed.load(Ordering::SeqCst),
        "subscriptions": shared.stats.subscriptions_active.load(Ordering::SeqCst),
    })
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
        serve_http(shared, tls, peer, conn_id).await;
    } else {
        serve_http(shared, tcp, peer, conn_id).await;
    }
}

/// HTTP/1.1 as the front speaks it: one request per connection, the head within
/// the handshake time and under [`http::MAX_HEAD`] (hyper answers 431 past it).
fn http1_builder(head_timeout: Duration) -> hyper::server::conn::http1::Builder {
    let mut b = hyper::server::conn::http1::Builder::new();
    b.timer(TokioTimer::new())
        .header_read_timeout(head_timeout)
        .max_buf_size(http::MAX_HEAD)
        .title_case_headers(true);
    b
}

/// What a route of one connection knows.
#[derive(Clone)]
struct ConnCtx {
    shared: Arc<Shared>,
    peer: SocketAddr,
    conn_id: u64,
    /// The WebSocket session the upgrade started; the connection lasts as long as it does.
    session: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl FromRef<ConnCtx> for Arc<Shared> {
    fn from_ref(c: &ConnCtx) -> Arc<Shared> {
        c.shared.clone()
    }
}

impl FromRef<ConnCtx> for leptos::config::LeptosOptions {
    fn from_ref(_: &ConnCtx) -> leptos::config::LeptosOptions {
        crate::front::options()
    }
}

/// The closed route table (WASM-003 Q3, D6), Leptos's (T10a): the paths of
/// [`crate::front::PlatformRoutes`] mounted through `leptos_axum`, each with
/// its handler, and the health server function. A developer cannot add a
/// route; the identity routes of WASM-009 are added to that table when they exist.
fn front_router(ctx: ConnCtx) -> Router {
    use leptos_axum::LeptosRoutes;
    Router::new()
        .route(crate::front::HEALTH_PATH, any(health))
        .leptos_routes_with_handler(crate::front::listings(&crate::front::UPGRADE_PATHS), upgrade)
        .leptos_routes_with_handler(crate::front::listings(&[crate::front::DOWNLOAD_PATH]), download)
        .fallback(not_found)
        .method_not_allowed_fallback(not_found)
        .layer(axum::middleware::map_response(security_headers))
        .with_state(ctx)
}

/// The accept loop hands each connection here: the router runs on hyper, and a
/// WebSocket upgrade continues in [`session`] after hyper lets go of the stream.
async fn serve_http<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(shared: Arc<Shared>, io: S, peer: SocketAddr, conn_id: u64) {
    let handshake = Duration::from_millis(shared.cfg.limits.handshake_timeout_ms);
    let session = Arc::new(Mutex::new(None));
    let router = front_router(ConnCtx { shared, peer, conn_id, session: session.clone() });
    let _ = http1_builder(handshake).serve_connection(TokioIo::new(io), TowerToHyperService::new(router)).with_upgrades().await;
    let running = session.lock().ok().and_then(|mut s| s.take());
    if let Some(s) = running {
        let _ = s.await;
    }
}

fn plain(status: StatusCode, body: &'static str) -> Response {
    (status, [(header::CONTENT_TYPE, "text/plain")], body).into_response()
}

async fn not_found() -> Response {
    plain(StatusCode::NOT_FOUND, "not found")
}

/// Every answer of the HTTPS surface carries the security headers (WASM-013 R4)
/// and ends its connection: one request per connection. The `101` of an upgrade
/// is not an answer but the start of the WebSocket.
async fn security_headers(mut res: Response) -> Response {
    if res.status() != StatusCode::SWITCHING_PROTOCOLS {
        let h = res.headers_mut();
        for (name, value) in http::SECURITY_HEADERS {
            h.insert(*name, HeaderValue::from_static(value));
        }
        h.insert(header::CONNECTION, HeaderValue::from_static("close"));
    }
    res
}

/// `GET /health` runs the server function [`crate::front::health`] through
/// `leptos_axum`, with the server's state as its context.
async fn health(State(shared): State<Arc<Shared>>, req: Request) -> Response {
    if req.method() != Method::GET {
        return plain(StatusCode::NOT_FOUND, "not found");
    }
    leptos_axum::handle_server_fns_with_context(move || leptos::prelude::provide_context(shared.clone()), req).await.into_response()
}

/// `GET /` or `GET /ws` with an upgrade: the Origin (R14), then the WebSocket with
/// its frame and message size capped, then the session.
async fn upgrade(State(c): State<ConnCtx>, mut req: Request) -> Response {
    let h = req.headers();
    let has = |name: header::HeaderName, word: &str| h.get_all(name).iter().filter_map(|v| v.to_str().ok()).any(|v| v.split(',').any(|t| t.trim().eq_ignore_ascii_case(word)));
    if req.method() != Method::GET || !has(header::UPGRADE, "websocket") || !has(header::CONNECTION, "upgrade") {
        return plain(StatusCode::NOT_FOUND, "not found");
    }
    let shared = c.shared.clone();
    // R14: the Origin of a browser connection must be one the deployment lists.
    if !origin_allowed(&shared.cfg, h.get(header::ORIGIN).and_then(|v| v.to_str().ok())) {
        shared.stats.refused_origin.fetch_add(1, Ordering::SeqCst);
        let mut r = LogRecord::new("refused");
        r.connection = Some(c.conn_id);
        r.note = Some("origin not allowed".into());
        shared.sink.emit(&r);
        return plain(StatusCode::FORBIDDEN, "origin not allowed");
    }
    let version_13 = h.get(header::SEC_WEBSOCKET_VERSION).is_some_and(|v| v.as_bytes() == b"13");
    let Some(accept) = h.get(header::SEC_WEBSOCKET_KEY).filter(|_| version_13).map(|k| derive_accept_key(k.as_bytes())) else {
        return plain(StatusCode::BAD_REQUEST, "not a WebSocket handshake");
    };
    let on_upgrade = hyper::upgrade::on(&mut req);
    let handshake = Duration::from_millis(shared.cfg.limits.handshake_timeout_ms);
    let cfg = WebSocketConfig::default()
        .max_message_size(Some(shared.cfg.limits.max_frame_bytes))
        .max_frame_size(Some(shared.cfg.limits.max_frame_bytes))
        .read_buffer_size(16 * 1024);
    let (peer, conn_id) = (c.peer, c.conn_id);
    let running = tokio::spawn(async move {
        let Ok(Ok(upgraded)) = tokio::time::timeout(handshake, on_upgrade).await else { return };
        let ws = tokio_tungstenite::WebSocketStream::from_raw_socket(TokioIo::new(upgraded), Role::Server, Some(cfg)).await;
        let mut r = LogRecord::new("connect");
        r.connection = Some(conn_id);
        shared.sink.emit(&r);
        session(shared.clone(), ws, peer, conn_id).await;
        let mut r = LogRecord::new("disconnect");
        r.connection = Some(conn_id);
        shared.sink.emit(&r);
    });
    if let Ok(mut s) = c.session.lock() {
        *s = Some(running);
    }
    let mut res = Response::new(Body::empty());
    *res.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
    let rh = res.headers_mut();
    rh.insert(header::UPGRADE, HeaderValue::from_static("websocket"));
    rh.insert(header::CONNECTION, HeaderValue::from_static("Upgrade"));
    if let Ok(v) = HeaderValue::from_str(&accept) {
        rh.insert(header::SEC_WEBSOCKET_ACCEPT, v);
    }
    res
}

/// `GET /dl/<token>`: the bytes of a file, with `Range` for resuming. An address that
/// never existed, expired, or was issued to another source is the same plain 404.
async fn download(State(c): State<ConnCtx>, method: Method, Path(token): Path<String>, headers: HeaderMap) -> Response {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    if method != Method::GET {
        return plain(StatusCode::NOT_FOUND, "not found");
    }
    let Some(d) = c.shared.files.download(&token, c.peer.ip()) else {
        return plain(StatusCode::NOT_FOUND, "not found");
    };
    let Ok(mut file) = tokio::fs::File::open(&d.path).await else {
        return plain(StatusCode::NOT_FOUND, "not found");
    };
    let size = file.metadata().await.map(|m| m.len()).unwrap_or(d.size);
    // `bytes=N-` and `bytes=N-M`; anything else is served whole.
    let (start, end) = headers
        .get(header::RANGE)
        .and_then(|r| r.to_str().ok())
        .and_then(|r| r.strip_prefix("bytes="))
        .and_then(|r| r.split_once('-'))
        .and_then(|(a, b)| {
            let a: u64 = a.trim().parse().ok()?;
            let b: u64 = if b.trim().is_empty() { size.saturating_sub(1) } else { b.trim().parse().ok()? };
            (a <= b && a < size).then_some((a, b.min(size.saturating_sub(1))))
        })
        .unwrap_or((0, size.saturating_sub(1)));
    let partial = start != 0 || end != size.saturating_sub(1);
    let len = if size == 0 { 0 } else { end - start + 1 };
    if start > 0 && file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return plain(StatusCode::NOT_FOUND, "not found");
    }
    let name: String = d.name.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') { c } else { '_' }).collect();
    let body = Body::from_stream(futures_util::stream::unfold((file, len), |(mut file, left)| async move {
        if left == 0 {
            return None;
        }
        let mut buf = vec![0u8; (64 * 1024).min(left as usize)];
        match file.read(&mut buf).await {
            Ok(0) | Err(_) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Ok::<_, std::io::Error>(axum::body::Bytes::from(buf)), (file, left - n as u64)))
            }
        }
    }));
    let mut res = Response::new(body);
    if partial {
        *res.status_mut() = StatusCode::PARTIAL_CONTENT;
    }
    let rh = res.headers_mut();
    rh.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));
    rh.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    rh.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
        rh.insert(header::CONTENT_DISPOSITION, v);
    }
    if partial {
        if let Ok(v) = HeaderValue::from_str(&format!("bytes {start}-{end}/{size}")) {
            rh.insert(header::CONTENT_RANGE, v);
        }
    }
    if let Some(origin) = headers.get(header::ORIGIN).filter(|o| origin_allowed(&c.shared.cfg, o.to_str().ok())) {
        rh.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin.clone());
        rh.insert(header::VARY, HeaderValue::from_static("Origin"));
    }
    res
}

pub(crate) fn origin_allowed(cfg: &Resolved, origin: Option<&str>) -> bool {
    if cfg.allow_any_origin {
        return true;
    }
    match origin {
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
    in_flight: Arc<Mutex<HashMap<u64, Running>>>,
    /// Stream subscriptions of this connection, by the client's id.
    subs: Arc<Mutex<HashMap<u64, Running>>>,
    /// The upload in progress on this connection (WASM-014 R15), by the backend's upload id.
    upload: Option<String>,
    /// The ticket machinery that keeps calls of one connection in order.
    next_ticket: u64,
    served: Arc<watch::Sender<u64>>,
    /// `per-connection` mode: this connection's run units (WASM-003 Q2).
    state: Option<Arc<Mutex<exec::Session>>>,
}

/// A call or a subscription that is running: the two ways to stop it.
#[derive(Clone)]
struct Running {
    /// Stops the program at its next statement (a timeout, a disconnection).
    hard: Arc<AtomicBool>,
    /// The caller's request to cancel: the program sees it and ends at a safe point.
    soft: Arc<AtomicBool>,
    /// Does a cancel mean "ask", or "stop now"? (`long_running` programs and streams ask.)
    asks: bool,
}

impl Running {
    fn new(asks: bool) -> Running {
        Running { hard: Arc::new(AtomicBool::new(false)), soft: Arc::new(AtomicBool::new(false)), asks }
    }

    /// The caller asks it to stop. A program that asks is given `grace_ms` to reach a safe point.
    fn cancel(&self, grace_ms: u64) {
        if self.asks {
            self.soft.store(true, Ordering::SeqCst);
            let hard = self.hard.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(grace_ms)).await;
                hard.store(true, Ordering::SeqCst);
            });
        } else {
            self.hard.store(true, Ordering::SeqCst);
        }
    }

    /// The connection is gone: nothing is waiting for the answer.
    fn abandon(&self) {
        self.soft.store(true, Ordering::SeqCst);
        self.hard.store(true, Ordering::SeqCst);
    }
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
    // Everything the connection says goes through this queue; its length is also how far a
    // slow subscriber may fall behind before its stream is ended (`limits.event_queue`).
    let (tx, mut rx) = mpsc::channel::<Message>(shared.cfg.limits.event_queue.max(64));
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
        subs: Arc::new(Mutex::new(HashMap::new())),
        upload: None,
        next_ticket: 0,
        served: Arc::new(served_tx),
        state: (shared.backend.state == StateMode::PerConnection).then(|| Arc::new(Mutex::new(exec::Session::new()))),
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
    // Stop whatever this connection still runs: nobody is waiting for it.
    for map in [&conn.in_flight, &conn.subs] {
        if let Ok(m) = map.lock() {
            for f in m.values() {
                f.abandon();
            }
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
            let grace = shared.cfg.limits.cancel_grace_ms;
            let target = conn.in_flight.lock().ok().and_then(|m| m.get(&c.id).cloned());
            if let Some(f) = target {
                f.cancel(grace);
            }
            true
        }
        ClientFrame::Subscribe(s) => {
            on_subscribe(shared, conn, tx, s).await;
            true
        }
        ClientFrame::Unsubscribe(u) => {
            let target = conn.subs.lock().ok().and_then(|m| m.get(&u.id).cloned());
            if let Some(f) = target {
                f.cancel(shared.cfg.limits.cancel_grace_ms);
            }
            let _ = tx.send(Message::text(proto::to_text(&Unsubscribed { t: "unsubscribed".into(), id: u.id }))).await;
            true
        }
        ClientFrame::PutBegin(_) | ClientFrame::PutChunk(_) | ClientFrame::PutEnd(_) | ClientFrame::GetFile(_) => {
            on_file_frame(shared, conn, tx, frame).await;
            true
        }
    }
}

// ── files (WASM-014 R15) ──────────────────────────────────────────────────────

/// The caller of a file frame: a signed-in subject, or an error already sent.
async fn file_caller(shared: &Arc<Shared>, conn: &Conn, tx: &mpsc::Sender<Message>, id: u64) -> Option<(String, std::collections::BTreeSet<String>)> {
    if conn.hello.is_none() {
        send_error(tx, Some(id), &CallError::new(ErrorClass::Validation, codes::BAD_MESSAGE, "send hello first")).await;
        return None;
    }
    let Some(c) = &conn.claims else {
        send_error(tx, Some(id), &CallError::new(ErrorClass::Authentication, codes::TOKEN_MISSING, "no token: authenticate before moving files")).await;
        return None;
    };
    if let Err(e) = shared.validator.recheck(c) {
        send_error(tx, Some(id), &e).await;
        return None;
    }
    Some((c.sub.clone(), c.roles.clone()))
}

async fn on_file_frame(shared: &Arc<Shared>, conn: &mut Conn, tx: &mpsc::Sender<Message>, frame: ClientFrame) {
    let id = match &frame {
        ClientFrame::PutBegin(p) => p.id,
        ClientFrame::PutChunk(p) => p.id,
        ClientFrame::PutEnd(p) => p.id,
        ClientFrame::GetFile(p) => p.id,
        _ => return,
    };
    if !conn.rate.take() {
        send_error(tx, Some(id), &CallError::new(ErrorClass::Limit, codes::RATE_LIMITED, "too many messages on this connection; slow down")).await;
        return;
    }
    let Some((subject, roles)) = file_caller(shared, conn, tx, id).await else { return };
    let files = shared.files.clone();
    let mut log = LogRecord::new("file");
    log.connection = Some(conn.id);
    log.subject = Some(subject.clone());
    let started = Instant::now();
    // The disk work is blocking; keep it off the connection's task.
    let result: Result<String, CallError> = match frame {
        ClientFrame::PutBegin(p) => {
            log.note = Some("put-begin".into());
            let (f, s, r) = (files.clone(), subject.clone(), roles.clone());
            match tokio::task::spawn_blocking(move || f.begin(&s, &r, &p)).await {
                Ok(Ok((upload, have))) => {
                    conn.upload = Some(upload.clone());
                    Ok(proto::to_text(&PutReady { t: "put-ready".into(), id, upload, have }))
                }
                Ok(Err(e)) => Err(e),
                Err(_) => Err(CallError::new(ErrorClass::Server, codes::INTERNAL, "the file store failed")),
            }
        }
        ClientFrame::PutChunk(c) => {
            let Some(upload) = conn.upload.clone() else {
                send_error(tx, Some(id), &CallError::new(ErrorClass::Validation, codes::BAD_MESSAGE, "begin an upload first")).await;
                return;
            };
            log.bytes_in = Some(c.data.len());
            let (f, up) = (files.clone(), upload.clone());
            match tokio::task::spawn_blocking(move || f.chunk(&up, c.offset, &c.data)).await {
                Ok(Ok(have)) => Ok(proto::to_text(&PutReady { t: "put-ready".into(), id, upload, have })),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(CallError::new(ErrorClass::Server, codes::INTERNAL, "the file store failed")),
            }
        }
        ClientFrame::PutEnd(_) => {
            log.note = Some("put-end".into());
            let Some(upload) = conn.upload.take() else {
                send_error(tx, Some(id), &CallError::new(ErrorClass::Validation, codes::BAD_MESSAGE, "begin an upload first")).await;
                return;
            };
            let f = files.clone();
            match tokio::task::spawn_blocking(move || f.end(&upload)).await {
                Ok(Ok((file, size))) => Ok(proto::to_text(&PutDone { t: "put-done".into(), id, file, size })),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(CallError::new(ErrorClass::Server, codes::INTERNAL, "the file store failed")),
            }
        }
        ClientFrame::GetFile(g) => {
            log.note = Some("get-file".into());
            match files.offer(&g.file, &subject, conn.peer.ip()) {
                Ok(o) => Ok(proto::to_text(&FileUrl { t: "file-url".into(), id, path: format!("/dl/{}", o.token), expires_in: o.expires_in, size: o.size, sha256: o.sha256 })),
                Err(e) => Err(e),
            }
        }
        _ => return,
    };
    log.duration_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
    match result {
        Ok(text) => {
            log.outcome = Some("ok".into());
            shared.sink.emit(&log);
            let _ = tx.send(Message::text(text)).await;
        }
        Err(e) => {
            log.outcome = Some(e.class.wire().to_string());
            log.code = Some(e.code);
            shared.sink.emit(&log);
            send_error(tx, Some(id), &e).await;
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

    // A stream is subscribed to, not called.
    if prog.sig.stream {
        refuse!(CallError::new(ErrorClass::Validation, codes::BAD_MESSAGE, "this program is a stream: subscribe to it"), subject.as_deref(), Some(&name));
    }

    // `per-connection`: the state belongs to the first user this connection served (WASM-003 Q2).
    if let Some(state) = &conn.state {
        let admitted = state.lock().unwrap_or_else(|e| e.into_inner()).admit(subject.as_deref());
        if let Err(e) = admitted {
            refuse!(e, subject.as_deref(), Some(&name));
        }
    }

    // How long this call may take: the client's own limit, never above the backend's.
    let ceiling = if prog.sig.long_running { shared.cfg.limits.long_call_timeout_ms } else { shared.cfg.limits.call_timeout_ms };
    let timeout_ms = call.timeout_ms.map(|t| t.min(ceiling)).unwrap_or(ceiling);

    // Concurrency.
    let Ok(conn_permit) = conn.calls.clone().try_acquire_owned() else {
        refuse!(CallError::new(ErrorClass::Limit, codes::TOO_MANY_CALLS, "too many calls in flight on this connection"), subject.as_deref(), Some(&name));
    };
    let Ok(global_permit) = shared.calls.clone().try_acquire_owned() else {
        refuse!(CallError::new(ErrorClass::Limit, codes::TOO_MANY_CALLS, "the backend is busy; try again"), subject.as_deref(), Some(&name));
    };

    let modes: Vec<Mode> = call.args.iter().map(|a| a.mode).collect();
    let arg_texts: Vec<String> = call.args.iter().map(|a| a.json.get().to_string()).collect();
    let client_interface = conn.hello.as_ref().and_then(|h| h.client_interface.clone());
    let client_contract = conn.hello.as_ref().map(|h| h.contract.clone()).unwrap_or_default();

    // Idempotency (WASM-014 R9, R10): a key seen before is answered from the store and the
    // program is not run again; the same key with another request is refused.
    let idem_key = call.key.clone().filter(|k| (8..=128).contains(&k.len()) && k.bytes().all(|b| b.is_ascii_graphic()));
    let fingerprint = idem::fingerprint(&name, &modes.iter().zip(arg_texts.iter()).map(|(m, t)| format!("{}:{t}", if *m == Mode::Ref { "ref" } else { "val" })).collect::<Vec<_>>());
    if let Some(k) = &idem_key {
        let give_up = Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            match shared.idem.begin(&key, k, &fingerprint) {
                idem::Begin::Fresh => break,
                idem::Begin::Replay(outcome) => {
                    shared.stats.replays.fetch_add(1, Ordering::SeqCst);
                    let mut rec = LogRecord::new("call");
                    rec.call = Some(call_no);
                    rec.connection = Some(conn.id);
                    rec.program = Some(name.clone());
                    rec.subject = subject.clone();
                    rec.duration_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
                    rec.bytes_in = Some(bytes_in);
                    rec.note = Some("answered from the idempotency store".into());
                    match outcome {
                        idem::Outcome::Reply { args, returning, .. } => {
                            let texts: Vec<Option<String>> = args;
                            match build_reply(shared, &prog, &name, id, &modes, call.returning, client_interface.as_ref(), &client_contract, &texts, returning.as_ref()) {
                                Ok(reply) => {
                                    let text = proto::to_text(&reply);
                                    rec.bytes_out = Some(text.len());
                                    rec.outcome = Some("ok".into());
                                    shared.sink.emit(&rec);
                                    let _ = tx.send(Message::text(text)).await;
                                }
                                Err(e) => {
                                    rec.outcome = Some(e.class.wire().to_string());
                                    rec.code = Some(e.code);
                                    shared.sink.emit(&rec);
                                    send_error(tx, Some(id), &e).await;
                                }
                            }
                        }
                        idem::Outcome::Failure(e) => {
                            rec.outcome = Some(e.class.wire().to_string());
                            rec.code = Some(e.code);
                            shared.sink.emit(&rec);
                            send_error(tx, Some(id), &e).await;
                        }
                    }
                    return;
                }
                idem::Begin::Wait => {
                    if Instant::now() > give_up {
                        refuse!(CallError::new(ErrorClass::Limit, codes::CALL_TIMEOUT, "the first call with this key is still running"), subject.as_deref(), Some(&name));
                    }
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
                idem::Begin::Mismatch => {
                    refuse!(CallError::new(ErrorClass::Validation, codes::INVALID_PAYLOAD, "this idempotency key was used with a different request"), subject.as_deref(), Some(&name));
                }
            }
        }
    }

    // Ordering: calls of one connection run in the order they were made, unless the
    // program is declared independent — which a connection that keeps state ignores:
    // its calls take turns.
    let session = conn.state.clone();
    let ticket = if prog.sig.independent && session.is_none() {
        None
    } else {
        let t = conn.next_ticket;
        conn.next_ticket += 1;
        Some(t)
    };
    let served = conn.served.clone();
    let running = Running::new(prog.sig.long_running);
    if let Ok(mut m) = conn.in_flight.lock() {
        m.insert(id, running.clone());
    }
    let in_flight = conn.in_flight.clone();
    let tx = tx.clone();
    let shared = shared.clone();
    let conn_id = conn.id;
    let file_lock_wait_ms = shared.cfg.limits.file_lock_wait_ms;
    let configurations = shared.cfg.configurations.clone();
    let files: Option<Arc<dyn BackendFiles>> = subject.clone().map(|s| Arc::new(CallFiles { store: shared.files.clone(), subject: s }) as Arc<dyn BackendFiles>);
    let progress: Option<Box<dyn Fn(u8, &str) + Send>> = if prog.sig.long_running {
        let (ptx, last) = (tx.clone(), Mutex::new(Instant::now() - Duration::from_secs(1)));
        Some(Box::new(move |percent, message| {
            // At most ten a second; the last one, 100 %, is never dropped.
            if let Ok(mut l) = last.lock() {
                if percent < 100 && l.elapsed() < Duration::from_millis(100) {
                    return;
                }
                *l = Instant::now();
            }
            let text = proto::to_text(&Progress { t: "progress".into(), id, percent, message: message.to_string() });
            if percent >= 100 {
                let _ = ptx.blocking_send(Message::text(text));
            } else {
                let _ = ptx.try_send(Message::text(text));
            }
        }))
    } else {
        None
    };

    tokio::spawn(async move {
        let _permits = (conn_permit, global_permit);
        // Wait for this call's turn.
        if let Some(t) = ticket {
            let mut rx = served.subscribe();
            let _ = rx.wait_for(|v| *v == t).await;
        }
        let claim_map: HashMap<String, String> = claims.as_ref().map(|c| c.flat.clone().into_iter().collect()).unwrap_or_default();
        let prog2 = prog.clone();
        let opts = ExecOptions {
            claims: claim_map,
            cancel: running.hard.clone(),
            cancel_requested: running.soft.clone(),
            file_lock_wait: Duration::from_millis(file_lock_wait_ms),
            progress,
            emit: None,
            files,
            configurations,
        };
        let args2 = arg_texts.clone();
        let mode = if session.is_some() { StateMode::PerConnection } else { StateMode::PerCall };
        let work = tokio::task::spawn_blocking(move || match session {
            // The claims are installed again at every call, so a refreshed token is what the program sees.
            Some(s) => s.lock().unwrap_or_else(|e| e.into_inner()).execute(&prog2, &args2, opts),
            None => exec::execute(&prog2, &args2, opts),
        });
        let outcome: Result<exec::ExecResult, CallError> = match tokio::time::timeout(Duration::from_millis(timeout_ms), work).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err(CallError::new(ErrorClass::Server, codes::PROGRAM_FAULT, "the program failed unexpectedly")),
            Err(_) => {
                running.hard.store(true, Ordering::SeqCst);
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
        // What is kept for a repeat: an answer, and a failure the program itself reported.
        // A timeout, a limit, a cancel or a fault must stay retryable.
        if let Some(k) = &idem_key {
            let keep = match &outcome {
                Ok(res) => Some(idem::Outcome::Reply { args: res.args.iter().map(|a| Some(a.clone())).collect(), returning: res.returning.clone(), warnings: Vec::new() }),
                Err(e) if e.class == ErrorClass::Application && e.code != codes::CANCELLED => Some(idem::Outcome::Failure(e.clone())),
                Err(_) => None,
            };
            shared.idem.finish(&key, k, &fingerprint, keep);
        }

        let mut rec = LogRecord::new("call");
        rec.call = Some(call_no);
        rec.connection = Some(conn_id);
        rec.program = Some(name.clone());
        rec.subject = subject.clone();
        rec.duration_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
        rec.bytes_in = Some(bytes_in);
        rec.state = Some(mode.as_str());
        match outcome {
            Ok(res) => {
                let texts: Vec<Option<String>> = res.args.iter().map(|a| Some(a.clone())).collect();
                match build_reply(&shared, &prog, &name, id, &modes, call.returning, client_interface.as_ref(), &client_contract, &texts, res.returning.as_ref()) {
                    Ok(reply) => {
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

/// The files a program may reach for the caller it is serving: that caller's uploads,
/// and an offer made to that caller.
struct CallFiles {
    store: Arc<FileStore>,
    subject: String,
}

impl BackendFiles for CallFiles {
    fn resolve(&self, file_id: &str) -> Option<std::path::PathBuf> {
        self.store.uploaded_path(file_id, &self.subject)
    }

    fn publish(&self, path: &std::path::Path) -> Result<String, String> {
        self.store.publish(path, &self.subject)
    }
}

/// The reply to a call: each by-reference answer and the `RETURNING` answer, projected to
/// the layouts the caller's contract version knew, plus the program's deprecation warning.
#[allow(clippy::too_many_arguments)]
fn build_reply(
    shared: &Shared,
    prog: &ExportedProgram,
    name: &str,
    id: u64,
    modes: &[Mode],
    wants_returning: bool,
    client_interface: Option<&InterfaceDescription>,
    client_contract: &str,
    args: &[Option<String>],
    returning: Option<&String>,
) -> Result<Reply, CallError> {
    let current = client_contract == shared.backend.interface.contract;
    let client_sig = client_interface.and_then(|i| i.program(name));
    let mut out_args: Vec<Option<Box<RawValue>>> = Vec::new();
    for (i, text) in args.iter().enumerate() {
        let Some(text) = text else {
            out_args.push(None);
            continue;
        };
        if modes.get(i) != Some(&Mode::Ref) {
            out_args.push(None);
            continue;
        }
        let projected = match client_sig.and_then(|s| s.params.get(i)) {
            Some(p) if !current => mapping::project_json(&p.layout, text, usize::MAX >> 4).map_err(|e| CallError::new(ErrorClass::Server, codes::INTERNAL, format!("cannot project the answer: {e}")))?,
            _ => text.clone(),
        };
        out_args.push(Some(RawValue::from_string(projected).map_err(|_| CallError::new(ErrorClass::Server, codes::INTERNAL, "the answer is not JSON"))?));
    }
    let returning = match (returning, wants_returning) {
        (Some(text), true) => {
            let projected = match client_sig.and_then(|s| s.returning.as_ref()) {
                Some(p) if !current => mapping::project_json(&p.layout, text, usize::MAX >> 4).unwrap_or_else(|_| text.clone()),
                _ => text.clone(),
            };
            RawValue::from_string(projected).ok()
        }
        _ => None,
    };
    let mut warnings = Vec::new();
    if let Some(d) = prog.sig.deprecated.as_ref() {
        warnings.push(match &d.removal {
            Some(r) => format!("{name} is deprecated (removed in contract {r}): {}", d.message),
            None => format!("{name} is deprecated: {}", d.message),
        });
    }
    Ok(Reply { t: "reply".into(), id, args: out_args, returning, warnings })
}

// ── a stream (WASM-014 R14) ───────────────────────────────────────────────────

async fn on_subscribe(shared: &Arc<Shared>, conn: &mut Conn, tx: &mpsc::Sender<Message>, s: proto::Subscribe) {
    let id = s.id;
    let name = s.program.trim().to_ascii_uppercase();
    let started = Instant::now();
    let call_no = shared.next_call.fetch_add(1, Ordering::SeqCst);
    let conn_id = conn.id;
    let refuse = |e: CallError| {
        let mut r = LogRecord::new("call");
        r.call = Some(call_no);
        r.connection = Some(conn_id);
        r.program = Some(name.clone());
        r.outcome = Some(e.class.wire().to_string());
        r.code = Some(e.code);
        r.note = Some("subscribe".into());
        shared.sink.emit(&r);
        e
    };
    macro_rules! no {
        ($e:expr) => {{
            let e = refuse($e);
            send_error(tx, Some(id), &e).await;
            return;
        }};
    }
    if conn.hello.is_none() {
        no!(CallError::new(ErrorClass::Validation, codes::BAD_MESSAGE, "send hello first"));
    }
    if !conn.rate.take() {
        no!(CallError::new(ErrorClass::Limit, codes::RATE_LIMITED, "too many messages on this connection; slow down"));
    }
    let prog = shared.backend.program(&name).cloned();
    let public = prog.as_ref().is_some_and(|p| p.sig.public);
    let claims: Option<Claims> = match &conn.claims {
        Some(c) => shared.validator.recheck(c).ok().map(|_| c.clone()),
        None => None,
    };
    if !public && claims.is_none() {
        let e = match &conn.claims {
            Some(c) => shared.validator.recheck(c).err().unwrap_or_else(|| CallError::new(ErrorClass::Authentication, codes::TOKEN_MISSING, "no token: authenticate before subscribing")),
            None => CallError::new(ErrorClass::Authentication, codes::TOKEN_MISSING, "no token: authenticate before subscribing"),
        };
        no!(e);
    }
    let Some(prog) = prog else {
        no!(CallError::new(ErrorClass::NotFound, codes::UNKNOWN_PROGRAM, "no such program"));
    };
    if !prog.sig.stream {
        no!(CallError::new(ErrorClass::Validation, codes::BAD_MESSAGE, "this program is not a stream: call it"));
    }
    let subject = claims.as_ref().map(|c| c.sub.clone());
    if !public {
        let c = claims.as_ref().expect("authenticated above");
        if !c.has_any_role(&prog.sig.roles) {
            no!(CallError::new(ErrorClass::Authorisation, codes::ROLE_REFUSED, "the caller's roles do not allow this program"));
        }
    }
    let limiter_key = subject.clone().unwrap_or_else(|| format!("ip:{}", conn.peer.ip()));
    if !shared.user_limiter.take(&limiter_key) {
        no!(CallError::new(ErrorClass::Limit, codes::RATE_LIMITED, "too many calls from this user; slow down"));
    }
    let active = conn.subs.lock().map(|m| m.len()).unwrap_or(0);
    if active >= shared.cfg.limits.max_subscriptions_per_connection {
        no!(CallError::new(ErrorClass::Limit, codes::TOO_MANY_CALLS, "too many subscriptions on this connection"));
    }
    let Ok(global_permit) = shared.calls.clone().try_acquire_owned() else {
        no!(CallError::new(ErrorClass::Limit, codes::TOO_MANY_CALLS, "the backend is busy; try again"));
    };
    let arg_texts: Vec<String> = s.args.iter().map(|a| a.json.get().to_string()).collect();

    let running = Running::new(true);
    if let Ok(mut m) = conn.subs.lock() {
        m.insert(id, running.clone());
    }
    shared.stats.subscriptions_active.fetch_add(1, Ordering::SeqCst);
    let _ = tx.send(Message::text(proto::to_text(&Subscribed { t: "subscribed".into(), id }))).await;

    let (tx, shared, subs) = (tx.clone(), shared.clone(), conn.subs.clone());
    let claim_map: HashMap<String, String> = claims.as_ref().map(|c| c.flat.clone().into_iter().collect()).unwrap_or_default();
    let files: Option<Arc<dyn BackendFiles>> = subject.clone().map(|s| Arc::new(CallFiles { store: shared.files.clone(), subject: s }) as Arc<dyn BackendFiles>);
    let slow = Arc::new(AtomicBool::new(false));
    tokio::spawn(async move {
        let _permit = global_permit;
        let seq = AtomicU64::new(0);
        let (etx, slow2) = (tx.clone(), slow.clone());
        let emit: Box<dyn Fn(&str) -> bool + Send> = Box::new(move |json| {
            let n = seq.fetch_add(1, Ordering::SeqCst) + 1;
            let Ok(data) = RawValue::from_string(json.to_string()) else { return false };
            let text = proto::to_text(&Event { t: "event".into(), sub: id, seq: n, data });
            if etx.try_send(Message::text(text)).is_err() {
                // The subscriber is not keeping up: it is ended rather than buffered without bound.
                slow2.store(true, Ordering::SeqCst);
                return false;
            }
            true
        });
        let opts = ExecOptions {
            claims: claim_map,
            cancel: running.hard.clone(),
            cancel_requested: running.soft.clone(),
            file_lock_wait: Duration::from_millis(shared.cfg.limits.file_lock_wait_ms),
            progress: None,
            emit: Some(emit),
            files,
            configurations: shared.cfg.configurations.clone(),
        };
        let prog2 = prog.clone();
        let outcome = tokio::task::spawn_blocking(move || exec::execute(&prog2, &arg_texts, opts)).await;
        if let Ok(mut m) = subs.lock() {
            m.remove(&id);
        }
        shared.stats.subscriptions_active.fetch_sub(1, Ordering::SeqCst);

        let mut rec = LogRecord::new("call");
        rec.call = Some(call_no);
        rec.connection = Some(conn_id);
        rec.program = Some(name.clone());
        rec.subject = subject.clone();
        rec.duration_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
        rec.note = Some("stream".into());
        match outcome {
            Ok(Ok(_)) => {
                rec.outcome = Some("ok".into());
                shared.sink.emit(&rec);
                let _ = tx.send(Message::text(proto::to_text(&SubEnd { t: "sub-end".into(), sub: id }))).await;
            }
            // The client unsubscribed or went away: it asked for this, so nothing is said.
            Ok(Err(e)) if e.code == codes::CANCELLED && !slow.load(Ordering::SeqCst) => {
                rec.outcome = Some("cancelled".into());
                shared.sink.emit(&rec);
            }
            Ok(Err(e)) => {
                let e = if slow.load(Ordering::SeqCst) {
                    CallError::new(ErrorClass::Limit, codes::TOO_MANY_CALLS, "the subscription was ended: the client was not keeping up with the stream")
                } else {
                    e
                };
                rec.outcome = Some(e.class.wire().to_string());
                rec.code = Some(e.code);
                shared.sink.emit(&rec);
                send_error(&tx, Some(id), &e).await;
            }
            Err(_) => {
                let e = CallError::new(ErrorClass::Server, codes::PROGRAM_FAULT, "the program failed unexpectedly");
                rec.outcome = Some("server".into());
                shared.sink.emit(&rec);
                send_error(&tx, Some(id), &e).await;
            }
        }
    });
}
