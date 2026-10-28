// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The connection to the backend** (WASM-016 R9; the wire is WASM-003/005/014).
//!
//! The player owns the connection so that the program module has no network at all: it hands
//! over a request frame (`remote`) and later receives an answer frame. This module is the
//! connection as a *sans-IO* state machine, generic over a [`Socket`]: the browser gives it a
//! `WebSocket`, the tests give it a mock and a fake clock.
//!
//! It speaks the backend's own protocol, `cobolt_wire::proto` (WASM-014 T6, ABI amendment A15):
//!
//! * on open it sends `{"t":"hello","protocol","contract","client","session"}` and waits for the
//!   server's `hello`; a refusal fails every waiting call with the class `version`;
//! * with a session token it then sends `{"t":"auth","token"}` and waits for `auth-ok`; with none
//!   (public programs) it is ready at once — there is no development token of its own;
//! * a call is `{"t":"call","id","key",…}` with the module's frame (`{"program","args",
//!   "returning"}`, A12) spliced in **as text**, so no number loses a digit; `key` is fresh per call
//!   and kept for its repeat (R10);
//! * an answer `{"t":"reply","id",…}` reaches the module as `{"id":N,"result":<the reply>}`
//!   (A12 reads `result.args` and `result.returning`); `{"t":"error","id",…}` as
//!   `{"id":N,"error":{"class","code","message",…}}`;
//! * the player's own failures use the closed list of WASM-014 R7 (`transport` 1000/1001/1002/1003,
//!   `validation` 4001, `version` 6000/6001, `limit` 7000), with `may_have_run` where it applies;
//! * `authentication` 2002 (token expired) refreshes the token and repeats the call once;
//! * a call in flight when the connection drops is repeated once, under the same key, only when
//!   the backend's interface declares the program `idempotent` (R9); otherwise it fails with
//!   `may_have_run: true`;
//! * every call ends: an answer, a failure, or `transport` 1002 at its timeout (then `cancel`).
//!
//! The session id lives in [`Client`] only — never in browser storage (WASM-014 Q5).

use std::collections::BTreeMap;

use serde_json::{json, Value};

/// The error classes of WASM-014 R7 the player itself produces, as they appear in an error frame.
pub mod class {
    pub const TRANSPORT: &str = "transport";
    pub const AUTHENTICATION: &str = "authentication";
    pub const VALIDATION: &str = "validation";
    pub const VERSION: &str = "version";
    pub const LIMIT: &str = "limit";
}

/// The platform codes the player emits, mirrored from `cobolt_wire::error::codes` (the player cannot
/// link `cobolt-wire`; a test holds the two equal).
pub mod code {
    pub const NO_CONNECTION: u32 = 1000;
    pub const CLOSED: u32 = 1001;
    pub const TIMEOUT: u32 = 1002;
    pub const NOT_CONFIGURED: u32 = 1003;
    pub const TOKEN_EXPIRED: u32 = 2002;
    pub const BAD_MESSAGE: u32 = 4001;
    pub const CLIENT_TOO_OLD: u32 = 6000;
    pub const CLIENT_TOO_NEW: u32 = 6001;
    pub const FRAME_TOO_LARGE: u32 = 7000;
}

/// The wire protocol this player speaks (`cobolt_wire::contract::PROTOCOL`).
pub const PROTOCOL: u32 = 1;

/// The contract a page that names none is taken to be built against (a new project's
/// `backend/exports.toml` says `1.0.0`).
pub const DEFAULT_CONTRACT: &str = "1.0.0";

/// The largest answer frame the program module can read: its staging area in memory 0
/// (`cobolt-wasm-rt` `state.rs` `FRAME_SIZE`; a test holds the two equal). A larger answer would
/// reach the module as "not a frame of the ABI"; the player fails it with a clear `limit` error.
pub const MODULE_ANSWER_LIMIT: usize = 64 * 1024;

/// Request ids from this one up are the player's own (the Authentication control's and the remote
/// grids'); below it the module's (ABI A8).
pub const PLAYER_ID_BASE: u32 = 0x8000_0000;

/// A message transport. The browser implements it over a `WebSocket`.
pub trait Socket {
    fn open(&mut self, url: &str);
    fn send(&mut self, text: &str);
    fn close(&mut self);
}

/// What the connection tells its owner.
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    /// An answer frame for request `id`, ready for frame-in.
    Answer { id: u32, frame: Vec<u8> },
    /// The state of the link, for a status indicator.
    State(LinkState),
    /// Something the developer should see in the console (a version warning, an unreadable frame).
    Warning(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    Closed,
    Connecting,
    /// Open; `hello` sent, waiting for the server's.
    Greeting,
    /// Waiting for the server to accept the token.
    Authenticating,
    Ready,
    /// Waiting to try again.
    Backoff,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// The backend addresses the player may connect to (`wss://api.example.com/ws`).
    pub origins: Vec<String>,
    pub call_timeout_ms: u64,
    pub max_frame: usize,
    pub backoff_start_ms: u64,
    pub backoff_max_ms: u64,
    /// Allow `ws://` to a non-loopback host (an explicit *unsafe* setting, WASM-013 R11).
    pub unsafe_plain_ws: bool,
    /// The contract version the application was built against (`hello.contract`).
    pub contract: String,
    /// Who this client is, for the backend's logs (`hello.client`).
    pub client: String,
    /// The programs the backend's interface declares `idempotent`: only these are repeated after a
    /// lost connection (WASM-014 R9).
    pub idempotent: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            origins: Vec::new(),
            call_timeout_ms: 30_000,
            max_frame: 384 * 1024,
            backoff_start_ms: 250,
            backoff_max_ms: 10_000,
            unsafe_plain_ws: false,
            contract: DEFAULT_CONTRACT.into(),
            client: format!("prc-wasm/{}", env!("CARGO_PKG_VERSION")),
            idempotent: Vec::new(),
        }
    }
}

impl Config {
    /// The connection the page's configuration asks for: its origins and
    /// unsafe setting, and its call timeout and frame limit where it gives
    /// them (WASM-017 R8: `max_frame` is the project's payload limit), the
    /// defaults otherwise.
    pub fn from_page(origins: Vec<String>, unsafe_plain_ws: bool, call_timeout_ms: Option<u64>, max_frame: Option<usize>) -> Config {
        let d = Config::default();
        Config {
            origins,
            unsafe_plain_ws,
            call_timeout_ms: call_timeout_ms.unwrap_or(d.call_timeout_ms),
            max_frame: max_frame.unwrap_or(d.max_frame),
            ..d
        }
    }

    /// The backend's contract version and its idempotent programs, as the Build wrote them into the
    /// page's configuration (an empty contract keeps [`DEFAULT_CONTRACT`]).
    pub fn with_backend(mut self, contract: &str, idempotent: Vec<String>) -> Config {
        if !contract.trim().is_empty() {
            self.contract = contract.trim().to_owned();
        }
        self.idempotent = idempotent.into_iter().map(|p| p.trim().to_ascii_uppercase()).collect();
        self
    }
}

#[derive(Debug)]
struct InFlight {
    /// When the module made the call: its timeout runs from here, whatever the repeats.
    sent_at: u64,
    /// The frame as the module produced it (text, so its numbers keep every digit).
    frame: String,
    /// The program it calls, upper case (for the idempotency rule).
    program: String,
    /// The idempotency key: the same on every repeat of this call.
    key: String,
    /// On the current connection.
    sent: bool,
    /// On any connection, ever.
    ever_sent: bool,
    /// Repeated once after `authentication` 2002.
    auth_repeated: bool,
    /// Repeated once after a lost connection.
    drop_repeated: bool,
}

/// The connection state machine.
pub struct Client<S: Socket> {
    pub cfg: Config,
    sock: S,
    state: LinkState,
    /// The address in use (one of `cfg.origins`).
    url: Option<String>,
    token: Option<String>,
    /// The session this page's connections share (`hello.session`); kept here and nowhere else.
    session: String,
    seed: u64,
    next_id: u32,
    inflight: BTreeMap<u32, InFlight>,
    attempt: u32,
    retry_at: Option<u64>,
    /// A deterministic jitter source, 0..=1000, replaced in tests.
    pub jitter: Box<dyn FnMut() -> u32>,
    wanted: bool,
    out: Vec<Out>,
    /// Ask the owner to refresh the token (the server said it expired); it then calls [`Client::set_token`].
    pub refresh_requested: bool,
}

/// Is `url` an acceptable backend address for `cfg`?
pub fn origin_allowed(cfg: &Config, url: &str) -> Result<(), String> {
    let lower = url.trim().to_ascii_lowercase();
    if !cfg.origins.iter().any(|o| o.trim().eq_ignore_ascii_case(url.trim())) {
        return Err(format!("`{url}` is not one of the backend addresses this application was built for"));
    }
    if lower.starts_with("ws://") {
        let host = lower.trim_start_matches("ws://").split(['/', ':']).next().unwrap_or("");
        let loopback = matches!(host, "localhost" | "127.0.0.1" | "[::1]") || host.ends_with(".localhost");
        if !loopback && !cfg.unsafe_plain_ws {
            return Err("an unencrypted ws:// address is only allowed for the local machine (the `unsafe` setting allows it, and a release build refuses that setting)".into());
        }
    } else if !lower.starts_with("wss://") {
        return Err("a backend address must start with wss:// (or ws:// for the local machine)".into());
    }
    Ok(())
}

/// An error frame for the module (ABI A6 as amended by A15): `{"id":N,"error":{"class","code",
/// "message"[,"may_have_run"]}}`.
pub fn error_frame(id: u32, class: &str, code: u32, message: &str, may_have_run: Option<bool>) -> Vec<u8> {
    let mut e = json!({"class": class, "code": code, "message": message});
    if let Some(m) = may_have_run {
        e["may_have_run"] = Value::Bool(m);
    }
    json!({"id": id, "error": e}).to_string().into_bytes()
}

/// A 64-bit session id as 16 hex digits, from a seed (xorshift; the browser seeds it from
/// `Math.random` and the clock).
fn session_from(seed: &mut u64) -> String {
    let mut x = *seed | 1;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *seed = x;
    format!("{x:016x}")
}

impl<S: Socket> Client<S> {
    pub fn new(cfg: Config, sock: S) -> Self {
        let mut jseed = 0x2545_f491u32;
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let session = session_from(&mut seed);
        Client {
            cfg,
            sock,
            state: LinkState::Closed,
            url: None,
            token: None,
            session,
            seed,
            next_id: 1,
            inflight: BTreeMap::new(),
            attempt: 0,
            retry_at: None,
            jitter: Box::new(move || {
                jseed ^= jseed << 13;
                jseed ^= jseed >> 17;
                jseed ^= jseed << 5;
                jseed % 1001
            }),
            wanted: false,
            out: Vec::new(),
            refresh_requested: false,
        }
    }

    /// Seed the session id (the browser passes something unpredictable); a fresh id follows.
    pub fn seed_session(&mut self, seed: u64) {
        self.seed = seed;
        self.session = session_from(&mut self.seed);
    }

    /// The session id this client names in `hello`.
    pub fn session(&self) -> &str {
        &self.session
    }

    pub fn state(&self) -> LinkState {
        self.state
    }

    pub fn socket(&mut self) -> &mut S {
        &mut self.sock
    }

    pub fn in_flight(&self) -> usize {
        self.inflight.len()
    }

    pub fn take_out(&mut self) -> Vec<Out> {
        std::mem::take(&mut self.out)
    }

    fn set_state(&mut self, s: LinkState) {
        if self.state != s {
            self.state = s;
            self.out.push(Out::State(s));
        }
    }

    fn is_idempotent(&self, program: &str) -> bool {
        !program.is_empty() && self.cfg.idempotent.iter().any(|p| p.eq_ignore_ascii_case(program))
    }

    /// Give the connection the bearer token (or none after a sign-out or a failed refresh).
    pub fn set_token(&mut self, token: Option<String>) {
        self.token = token;
        self.refresh_requested = false;
        let waiting: Vec<u32> = self.inflight.iter().filter(|(_, f)| !f.sent && f.auth_repeated).map(|(id, _)| *id).collect();
        match self.token.clone() {
            Some(t) if self.state == LinkState::Ready => {
                self.sock.send(&json!({"t": "auth", "token": t}).to_string());
                // Repeat what the server refused for an expired token, once, under the same key.
                for id in waiting {
                    self.transmit(id);
                }
            }
            Some(_) => {} // the waiting calls go out when the connection is ready
            None => {
                for id in waiting {
                    self.fail(id, class::AUTHENTICATION, code::TOKEN_EXPIRED, "the session expired and could not be refreshed", Some(false));
                }
            }
        }
    }

    /// Connect to `url` (must be one of the configured addresses).
    pub fn connect(&mut self, url: &str, now: u64) -> Result<(), String> {
        origin_allowed(&self.cfg, url)?;
        self.url = Some(url.to_owned());
        self.wanted = true;
        self.start_connecting(now);
        Ok(())
    }

    fn start_connecting(&mut self, _now: u64) {
        let Some(url) = self.url.clone() else { return };
        self.set_state(LinkState::Connecting);
        self.sock.open(&url);
    }

    /// A call from the module with a fresh id; returns the id.
    pub fn request(&mut self, frame: &[u8], now: u64) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.request_as(id, frame, now)
    }

    /// A call from the module under the id the host already gave it (`remote` returned that id to
    /// the module, so the answer must come back under it).
    pub fn request_as(&mut self, id: u32, frame: &[u8], now: u64) -> u32 {
        let refuse = |s: &mut Self, class_: &str, code_: u32, m: String| {
            s.out.push(Out::Answer { id, frame: error_frame(id, class_, code_, &m, Some(false)) });
            id
        };
        if frame.len() > self.cfg.max_frame {
            let m = format!("the request is larger than the connection allows ({} bytes, limit {})", frame.len(), self.cfg.max_frame);
            return refuse(self, class::LIMIT, code::FRAME_TOO_LARGE, m);
        }
        let Ok(text) = std::str::from_utf8(frame) else {
            return refuse(self, class::VALIDATION, code::BAD_MESSAGE, "the request frame is not UTF-8 text".into());
        };
        let v = match serde_json::from_str::<Value>(text) {
            Ok(Value::Object(m)) => m,
            _ => return refuse(self, class::VALIDATION, code::BAD_MESSAGE, "the request frame is not a JSON object".into()),
        };
        if ["t", "id", "key"].iter().any(|k| v.contains_key(*k)) {
            return refuse(self, class::VALIDATION, code::BAD_MESSAGE, "the request frame carries a member the player adds (t, id or key)".into());
        }
        let program = v.get("program").and_then(Value::as_str).unwrap_or("").trim().to_ascii_uppercase();
        let key = format!("{}-{id:08x}", self.session);
        self.inflight.insert(
            id,
            InFlight { sent_at: now, frame: text.trim().to_owned(), program, key, sent: false, ever_sent: false, auth_repeated: false, drop_repeated: false },
        );
        match self.state {
            LinkState::Ready => self.transmit(id),
            LinkState::Closed if self.wanted => {
                // The link went away and nothing is retrying: try now.
                self.start_connecting(now);
            }
            LinkState::Backoff => {
                // The user asked for something: try now rather than at the end of the back-off.
                self.retry_at = None;
                self.start_connecting(now);
            }
            LinkState::Connecting | LinkState::Greeting | LinkState::Authenticating => {} // held until Ready
            LinkState::Closed => {
                self.fail(id, class::TRANSPORT, code::NO_CONNECTION, "the application is not connected to its backend", Some(false));
            }
        }
        id
    }

    /// Send call `id` as `{"t":"call","id","key",…the module's frame…}`. The frame is spliced in as
    /// text: a payload never passes through a JSON value, so its numbers keep every digit.
    fn transmit(&mut self, id: u32) {
        let Some(f) = self.inflight.get_mut(&id) else { return };
        let body = f.frame.trim();
        let inner = body.strip_prefix('{').and_then(|b| b.strip_suffix('}')).unwrap_or("").trim();
        let key = Value::String(f.key.clone()).to_string();
        let msg = format!("{{\"t\":\"call\",\"id\":{id},\"key\":{key}{}{inner}}}", if inner.is_empty() { "" } else { "," });
        f.sent = true;
        f.ever_sent = true;
        self.sock.send(&msg);
    }

    fn fail(&mut self, id: u32, class_: &str, code_: u32, message: &str, may_have_run: Option<bool>) {
        if self.inflight.remove(&id).is_some() {
            self.out.push(Out::Answer { id, frame: error_frame(id, class_, code_, message, may_have_run) });
        }
    }

    // ── events from the socket ──────────────────────────────────────────────

    pub fn on_open(&mut self, _now: u64) {
        self.retry_at = None;
        let hello = json!({"t": "hello", "protocol": PROTOCOL, "contract": self.cfg.contract, "client": self.cfg.client, "session": self.session});
        self.set_state(LinkState::Greeting);
        self.sock.send(&hello.to_string());
    }

    fn flush_held(&mut self) {
        let held: Vec<u32> = self.inflight.iter().filter(|(_, f)| !f.sent).map(|(id, _)| *id).collect();
        for id in held {
            self.transmit(id);
        }
    }

    /// Fail every call that is waiting (none was sent on this connection: it never got that far).
    fn fail_all(&mut self, class_: &str, code_: u32, message: &str) {
        let ids: Vec<u32> = self.inflight.keys().copied().collect();
        for id in ids {
            let ever = self.inflight.get(&id).map(|f| f.ever_sent).unwrap_or(false);
            self.fail(id, class_, code_, message, Some(ever));
        }
    }

    /// The connection is past `hello` (and `auth`, when there is a token): send what waits.
    fn ready(&mut self) {
        self.attempt = 0;
        self.set_state(LinkState::Ready);
        self.flush_held();
    }

    pub fn on_message(&mut self, text: &str) {
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            self.out.push(Out::Warning(format!("the backend sent a frame that is not JSON ({} bytes)", text.len())));
            return;
        };
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let id = v.get("id").and_then(Value::as_u64).filter(|n| *n <= u32::MAX as u64).map(|n| n as u32);
        match v.get("t").and_then(Value::as_str).unwrap_or("") {
            "hello" => match s("accepted").as_str() {
                "ok" | "warn" => {
                    if s("accepted") == "warn" {
                        self.out.push(Out::Warning(format!("backend: {}", s("message"))));
                    }
                    match self.token.clone() {
                        Some(t) => {
                            self.set_state(LinkState::Authenticating);
                            self.sock.send(&json!({"t": "auth", "token": t}).to_string());
                        }
                        None => self.ready(),
                    }
                }
                _ => {
                    // Refused: the application and the backend cannot talk. Every waiting call fails
                    // with the class `version`; the server closes the connection, and no retry follows.
                    let too_old = v.get("update_to").is_some_and(|u| !u.is_null());
                    let c = if too_old { code::CLIENT_TOO_OLD } else { code::CLIENT_TOO_NEW };
                    let m = s("message");
                    self.out.push(Out::Warning(format!("backend refused the connection: {m}")));
                    self.fail_all(class::VERSION, c, &m);
                    self.wanted = false;
                    self.retry_at = None;
                    self.sock.close();
                    self.set_state(LinkState::Closed);
                }
            },
            "auth-ok" => {
                if self.state != LinkState::Ready {
                    self.ready();
                }
            }
            "reply" => {
                let Some(id) = id else { return };
                let Some(f) = self.inflight.remove(&id) else { return };
                // ABI A12: `result` is the body of `proto::Reply`; the module reads its `args` and `returning`.
                let frame = format!("{{\"id\":{id},\"result\":{}}}", text.trim());
                if id < PLAYER_ID_BASE && frame.len() > MODULE_ANSWER_LIMIT {
                    let m = format!("answer too large (limit 64 KiB): the answer of {} is {} bytes", if f.program.is_empty() { "the call" } else { &f.program }, frame.len());
                    self.out.push(Out::Answer { id, frame: error_frame(id, class::LIMIT, code::FRAME_TOO_LARGE, &m, None) });
                } else {
                    self.out.push(Out::Answer { id, frame: frame.into_bytes() });
                }
            }
            "error" => {
                let class_ = s("class");
                let code_ = v.get("code").and_then(Value::as_u64).unwrap_or(0) as u32;
                let Some(id) = id else {
                    // A failure of the connection, not of a call.
                    if self.state == LinkState::Authenticating && class_ == class::AUTHENTICATION {
                        // The token was refused: go on without it (public programs still answer);
                        // the owner may refresh it.
                        self.refresh_requested = true;
                        self.ready();
                    } else if matches!(self.state, LinkState::Greeting | LinkState::Authenticating) {
                        let m = s("message");
                        self.out.push(Out::Warning(format!("backend: {class_} {code_}: {m}")));
                        self.fail_all(if class_.is_empty() { class::TRANSPORT } else { &class_ }, code_, &m);
                    } else {
                        self.out.push(Out::Warning(format!("backend: {class_} {code_}: {}", s("message"))));
                    }
                    return;
                };
                let Some(f) = self.inflight.get_mut(&id) else { return };
                // The token expired: refresh and repeat once; the server refused before running anything.
                if class_ == class::AUTHENTICATION && code_ == code::TOKEN_EXPIRED && !f.auth_repeated && self.token.is_some() {
                    f.auth_repeated = true;
                    f.sent = false;
                    self.refresh_requested = true;
                    return;
                }
                self.inflight.remove(&id);
                let mut e = json!({"class": class_, "code": code_, "message": s("message")});
                if let Some(d) = v.get("detail").filter(|d| !d.is_null()) {
                    e["detail"] = d.clone();
                }
                if let Some(m) = v.get("may_have_run").and_then(Value::as_bool) {
                    e["may_have_run"] = Value::Bool(m);
                }
                self.out.push(Out::Answer { id, frame: json!({"id": id, "error": e}).to_string().into_bytes() });
            }
            // Progress, streams and files are not this player's yet; a kind a newer backend invented
            // is not an error either (`proto::parse_server`).
            _ => {}
        }
    }

    pub fn on_close(&mut self, now: u64) {
        // The browser reports a failed attempt as `error` and then `close`: the second is a repeat.
        if matches!(self.state, LinkState::Closed | LinkState::Backoff) {
            return;
        }
        let established = self.state == LinkState::Ready;
        let ids: Vec<u32> = self.inflight.keys().copied().collect();
        for id in ids {
            let Some(f) = self.inflight.get(&id) else { continue };
            let (sent, ever, again) = (f.sent, f.ever_sent, self.is_idempotent(&f.program) && !f.drop_repeated);
            if sent {
                if again {
                    // Declared idempotent: repeated once on the next connection, under the same key,
                    // so a backend that ran it answers from its store (R9, R10).
                    if let Some(f) = self.inflight.get_mut(&id) {
                        f.drop_repeated = true;
                        f.sent = false;
                    }
                } else {
                    self.fail(id, class::TRANSPORT, code::CLOSED, "the connection to the backend was lost", Some(true));
                }
            } else if !established {
                let waiting_to_repeat = self.inflight.get(&id).is_some_and(|f| f.drop_repeated);
                if waiting_to_repeat {
                    // A safe call waiting for its repeat goes on waiting through failed attempts, until
                    // its own timeout (a backend restart takes a moment).
                    continue;
                }
                // This attempt never got as far as sending: the call is told now, not at its timeout.
                if ever {
                    self.fail(id, class::TRANSPORT, code::CLOSED, "the connection to the backend was lost and could not be restored", Some(true));
                } else {
                    self.fail(id, class::TRANSPORT, code::NO_CONNECTION, "the backend cannot be reached", Some(false));
                }
            }
        }
        // Reconnect with back-off after a lost connection, and while calls wait to be repeated; a failed
        // attempt that leaves nothing waiting stops there, and the next call tries at once — so a page whose
        // backend is down does not knock on its door forever (each knock is a console error in a browser).
        if self.wanted && (established || !self.inflight.is_empty()) {
            let base = self.cfg.backoff_start_ms.saturating_mul(1u64 << self.attempt.min(16)).min(self.cfg.backoff_max_ms);
            // Jitter of up to a quarter either way, so many clients do not return together.
            let j = (self.jitter)() as u64; // 0..=1000
            let delta = base / 4;
            let wait = base - delta + (2 * delta * j) / 1000;
            self.attempt += 1;
            self.retry_at = Some(now + wait);
            self.set_state(LinkState::Backoff);
        } else {
            self.set_state(LinkState::Closed);
        }
    }

    pub fn on_error(&mut self, now: u64) {
        // A failed attempt arrives as a close.
        if matches!(self.state, LinkState::Connecting | LinkState::Greeting | LinkState::Authenticating) {
            self.on_close(now);
        }
    }

    /// The clock moved: time out calls, and retry a connection that is due.
    pub fn tick(&mut self, now: u64) {
        let timeout = self.cfg.call_timeout_ms;
        let late: Vec<u32> = self.inflight.iter().filter(|(_, f)| now.saturating_sub(f.sent_at) >= timeout).map(|(id, _)| *id).collect();
        for id in late {
            let (sent, ever) = self.inflight.get(&id).map(|f| (f.sent, f.ever_sent)).unwrap_or((false, false));
            if sent && self.state == LinkState::Ready {
                // Nobody awaits it any more: the backend may stop the work (R13).
                self.sock.send(&json!({"t": "cancel", "id": id}).to_string());
            }
            let m = format!("the backend did not answer in time ({timeout} ms)");
            self.fail(id, class::TRANSPORT, code::TIMEOUT, &m, Some(ever));
        }
        if let Some(at) = self.retry_at {
            if now >= at && self.state == LinkState::Backoff {
                self.retry_at = None;
                self.start_connecting(now);
            }
        }
    }

    /// The next time `tick` must run, so a timer can be set (None = nothing pending).
    pub fn next_deadline(&self) -> Option<u64> {
        let call = self.inflight.values().map(|f| f.sent_at + self.cfg.call_timeout_ms).min();
        match (call, self.retry_at) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Cancel a request the module abandoned (its task was cancelled).
    pub fn cancel(&mut self, id: u32) {
        if let Some(f) = self.inflight.remove(&id) {
            if f.sent && self.state == LinkState::Ready {
                self.sock.send(&json!({"t": "cancel", "id": id}).to_string());
            }
        }
    }

    /// The user signed out: close the connection so that the server forgets who it was, and let the next
    /// call open a new one (anonymously, or with another session's token) under a new session id. What was
    /// sent and not yet answered is failed — it may have run — and what was only waiting goes out on the new one.
    pub fn reset_session(&mut self) {
        let sent: Vec<u32> = self.inflight.iter().filter(|(_, f)| f.sent).map(|(id, _)| *id).collect();
        for id in sent {
            self.fail(id, class::TRANSPORT, code::CLOSED, "the connection was closed by the sign-out", Some(true));
        }
        self.token = None;
        self.wanted = false;
        self.retry_at = None;
        self.session = session_from(&mut self.seed);
        self.sock.close();
        self.set_state(LinkState::Closed);
    }

    /// Close for good (unmount).
    pub fn shutdown(&mut self) {
        self.wanted = false;
        self.inflight.clear();
        self.retry_at = None;
        self.sock.close();
        self.set_state(LinkState::Closed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Mock {
        opened: Vec<String>,
        sent: Vec<String>,
        closed: u32,
    }
    impl Socket for Mock {
        fn open(&mut self, url: &str) {
            self.opened.push(url.to_owned());
        }
        fn send(&mut self, text: &str) {
            self.sent.push(text.to_owned());
        }
        fn close(&mut self) {
            self.closed += 1;
        }
    }

    const URL: &str = "wss://api.example.com/ws";
    const HELLO_OK: &str = r#"{"t":"hello","protocol":1,"protocol_min":1,"contract":"1.0.0","accepted":"ok","message":"","server":"test","max_frame_bytes":1048576}"#;

    fn cfg() -> Config {
        Config { origins: vec![URL.into(), "ws://localhost:8443/ws".into()], ..Config::default() }.with_backend("1.0.0", vec!["fund-totals".into()])
    }

    fn client() -> Client<Mock> {
        let mut c = Client::new(cfg(), Mock::default());
        c.jitter = Box::new(|| 500); // no jitter: exactly the base
        c
    }

    /// A client past `hello` with no token: ready.
    fn ready_client() -> Client<Mock> {
        let mut c = client();
        c.connect(URL, 0).unwrap();
        c.on_open(1);
        c.on_message(HELLO_OK);
        assert_eq!(c.state(), LinkState::Ready);
        c.socket().sent.clear();
        c
    }

    fn sent_json(c: &Client<Mock>) -> Vec<Value> {
        c.sock.sent.iter().map(|s| serde_json::from_str(s).unwrap()).collect()
    }

    fn answers(c: &mut Client<Mock>) -> Vec<(u32, Value)> {
        c.take_out().into_iter().filter_map(|o| if let Out::Answer { id, frame } = o { Some((id, serde_json::from_slice(&frame).unwrap())) } else { None }).collect()
    }

    fn reply(id: u32, returning: &str) -> String {
        format!(r#"{{"t":"reply","id":{id},"args":[null],"returning":{returning}}}"#)
    }

    /// WASM-017 T6: the page's `max_frame` (the project's payload limit) is
    /// the limit — a 512 KiB setting accepts 512 KiB and refuses 513 KiB; a
    /// page that gives none keeps 384 KiB.
    #[test]
    fn the_pages_frame_limit_is_the_projects_payload_limit_017() {
        assert_eq!(Config::from_page(vec![], false, None, None).max_frame, 384 * 1024);
        let limit = 512 * 1024;
        let mut c = Client::new(Config::from_page(cfg().origins, false, Some(5_000), Some(limit)), Mock::default());
        assert_eq!(c.cfg.call_timeout_ms, 5_000);
        c.set_token(None);
        c.connect(URL, 0).unwrap();
        let frame = |n: usize| {
            let mut f = br#"{"program":"P","args":[{"mode":"value","json":""#.to_vec();
            f.resize(n - 4, b'x');
            f.extend_from_slice(br#""}]}"#);
            assert_eq!(f.len(), n);
            f
        };
        let refused = |out: &[Out]| out.iter().any(|o| matches!(o, Out::Answer { frame, .. } if String::from_utf8_lossy(frame).contains("larger than the connection allows")));
        c.request(&frame(limit), 0);
        assert!(!refused(&c.take_out()), "512 KiB is accepted");
        assert_eq!(c.in_flight(), 1);
        c.request(&frame(limit + 1024), 0);
        assert!(refused(&c.take_out()), "513 KiB is refused");
        assert_eq!(c.in_flight(), 1);
        println!("frame limit 524288: 524288 bytes accepted, 525312 bytes refused; no setting: 393216");
    }

    #[test]
    fn only_configured_addresses_and_secure_schemes_are_allowed() {
        let mut c = client();
        assert!(c.connect("wss://evil.example.org/ws", 0).is_err(), "an address the build did not name");
        assert!(c.sock.opened.is_empty(), "no connection was attempted");
        assert!(c.connect(URL, 0).is_ok());
        let mut plain = Client::new(Config { origins: vec!["ws://203.0.113.9/ws".into()], ..Config::default() }, Mock::default());
        let e = plain.connect("ws://203.0.113.9/ws", 0).unwrap_err();
        assert!(e.contains("unencrypted"), "{e}");
        plain.cfg.unsafe_plain_ws = true;
        assert!(plain.connect("ws://203.0.113.9/ws", 0).is_ok(), "the explicit unsafe setting allows it");
        assert!(origin_allowed(&cfg(), "ws://localhost:8443/ws").is_ok(), "loopback needs no setting");
    }

    /// WASM-014 T6: hello, then the server's hello, then auth (with a token), then auth-ok, then the call.
    #[test]
    fn hello_then_auth_then_call_in_that_order() {
        let mut c = client();
        c.set_token(Some("tok-123".into()));
        c.connect(URL, 0).unwrap();
        assert!(!c.sock.opened[0].contains("tok-123"), "WASM-003 R11: never in a URL");
        let id = c.request(br#"{"program":"FUND-TOTALS","args":[{"mode":"value","json":{"F-NAME":"itau"}}],"returning":true}"#, 1);
        assert!(c.sock.sent.is_empty(), "nothing before the socket opens");
        c.on_open(2);
        assert_eq!(c.state(), LinkState::Greeting);
        let s = sent_json(&c);
        assert_eq!(s.len(), 1, "only hello until the server answers it: {s:?}");
        assert_eq!((s[0]["t"].as_str(), s[0]["protocol"].as_u64(), s[0]["contract"].as_str()), (Some("hello"), Some(1), Some("1.0.0")));
        assert!(s[0]["client"].as_str().unwrap().starts_with("prc-wasm/"));
        assert_eq!(s[0]["session"].as_str(), Some(c.session()));
        c.on_message(HELLO_OK);
        assert_eq!(c.state(), LinkState::Authenticating);
        let s = sent_json(&c);
        assert_eq!((s.len(), s[1]["t"].as_str(), s[1]["token"].as_str()), (2, Some("auth"), Some("tok-123")), "auth after hello: {s:?}");
        c.on_message(r#"{"t":"auth-ok","sub":"u-1","expires_in":300}"#);
        assert_eq!(c.state(), LinkState::Ready);
        let s = sent_json(&c);
        assert_eq!(s.len(), 3);
        let call = &s[2];
        assert_eq!((call["t"].as_str(), call["id"].as_u64(), call["program"].as_str()), (Some("call"), Some(id as u64), Some("FUND-TOTALS")));
        assert_eq!(call["args"][0]["json"]["F-NAME"], "itau");
        assert!(call["key"].as_str().unwrap().len() >= 8);
        assert!(call.get("type").is_none() && call.get("frame").is_none(), "the A8 envelope is gone");
        let order: Vec<&str> = s.iter().map(|m| m["t"].as_str().unwrap()).collect();
        println!("order on the wire: {order:?}");
        assert_eq!(order, ["hello", "auth", "call"]);
    }

    /// Public exports need no token, and the player has none of its own: no `auth` is sent.
    #[test]
    fn with_no_token_the_call_follows_hello_and_no_auth_is_sent() {
        let mut c = client();
        c.connect(URL, 0).unwrap();
        c.request(br#"{"program":"HEALTH","args":[],"returning":false}"#, 1);
        c.on_open(2);
        c.on_message(HELLO_OK);
        let order: Vec<String> = sent_json(&c).iter().map(|m| m["t"].as_str().unwrap().to_owned()).collect();
        assert_eq!(order, ["hello", "call"]);
        assert!(c.sock.sent.iter().all(|m| !m.contains("token")), "no development token is injected");
        // `{}` and `{"args":[]}` frames splice into a well-formed call.
        let id = c.request(b"{}", 3);
        let last: Value = serde_json::from_str(c.sock.sent.last().unwrap()).unwrap();
        assert_eq!((last["t"].as_str(), last["id"].as_u64()), (Some("call"), Some(id as u64)));
    }

    #[test]
    fn calls_carry_their_id_a_fresh_key_and_the_answer_keeps_every_digit() {
        let mut c = ready_client();
        let a = c.request(br#"{"program":"A","args":[{"mode":"value","json":{"N":12345678901234567890.123456789}}],"returning":true}"#, 10);
        let b = c.request(br#"{"program":"B","args":[],"returning":true}"#, 11);
        let s = sent_json(&c);
        assert_ne!(s[0]["key"], s[1]["key"], "a fresh key per call");
        assert!(c.sock.sent[0].contains("12345678901234567890.123456789"), "the request is spliced as text: {}", c.sock.sent[0]);
        // Answers may come in any order; each is handed over as `{"id","result":<the reply>}`.
        c.on_message(&reply(b, r#"{"V":"second"}"#));
        c.on_message(&reply(a, r#"{"V":1.50,"BIG":98765432109876543210.0001}"#));
        let out = c.take_out();
        let texts: Vec<(u32, String)> = out.iter().filter_map(|o| if let Out::Answer { id, frame } = o { Some((*id, String::from_utf8(frame.clone()).unwrap())) } else { None }).collect();
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0].0, b, "answers may arrive in any order");
        assert!(texts[1].1.contains("\"V\":1.50") && texts[1].1.contains("98765432109876543210.0001"), "digits kept: {}", texts[1].1);
        let v: Value = serde_json::from_str(&texts[1].1).unwrap();
        assert_eq!(v["id"].as_u64(), Some(a as u64));
        assert!(v["result"]["args"].is_array() && v["result"]["returning"].is_object(), "A12: result.args and result.returning");
        assert_eq!(c.in_flight(), 0);
    }

    /// Every failure the player produces, mapped onto the closed list (WASM-014 R7).
    #[test]
    fn each_failure_is_mapped_onto_the_closed_list() {
        let mut rows: Vec<(String, String, u64, Option<bool>)> = Vec::new();
        let mut row = |case: &str, v: &Value| {
            let e = &v["error"];
            rows.push((case.to_owned(), e["class"].as_str().unwrap_or("?").to_owned(), e["code"].as_u64().unwrap_or(0), e["may_have_run"].as_bool()));
        };
        // not connected (never asked to connect)
        let mut c = client();
        c.request(br#"{"program":"X"}"#, 0);
        row("not connected", &answers(&mut c)[0].1);
        // the backend cannot be reached (the attempt fails)
        let mut c = client();
        c.connect(URL, 0).unwrap();
        c.request(br#"{"program":"X"}"#, 0);
        c.on_error(5);
        c.on_close(6); // the browser's second event is ignored
        row("backend unreachable", &answers(&mut c)[0].1);
        assert_eq!((c.state(), c.retry_at), (LinkState::Closed, None), "nothing waits: no retry loop, the next call tries at once");
        c.request(br#"{"program":"X"}"#, 7);
        assert_eq!((c.state(), c.sock.opened.len()), (LinkState::Connecting, 2));
        // the connection drops with a call in flight
        let mut c = ready_client();
        c.request(br#"{"program":"X"}"#, 0);
        c.on_close(5);
        row("connection lost in flight", &answers(&mut c)[0].1);
        // no answer in time
        let mut c = ready_client();
        let id = c.request(br#"{"program":"SLOW"}"#, 0);
        c.tick(30_000);
        row("timeout", &answers(&mut c)[0].1);
        assert_eq!(sent_json(&c).last().unwrap(), &json!({"t": "cancel", "id": id}), "the backend is told to stop");
        // request frames the player refuses before the wire
        let mut c = ready_client();
        c.request(&vec![b'x'; 400 * 1024], 0);
        c.request(b"not json", 0);
        c.request(b"[1,2]", 0);
        c.request(&[0xff, 0xfe], 0);
        c.request(br#"{"t":"call","program":"X"}"#, 0);
        assert!(c.sock.sent.is_empty(), "nothing reached the socket");
        for (case, (_, v)) in ["request too large", "not JSON", "not an object", "not UTF-8", "carries t/id/key"].iter().zip(answers(&mut c)) {
            row(case, &v);
        }
        // the answer does not fit the module's 64 KiB staging area
        let mut c = ready_client();
        let id = c.request(br#"{"program":"FUND-GRID"}"#, 0);
        c.on_message(&reply(id, &format!(r#"{{"ROWS":"{}"}}"#, "y".repeat(70_000))));
        let (_, v) = answers(&mut c).remove(0);
        assert!(v["error"]["message"].as_str().unwrap().starts_with("answer too large (limit 64 KiB)"), "{v}");
        row("answer over 64 KiB", &v);
        // the backend refuses this application's version
        for (case, upd) in [("hello refused: update the app", r#""update_to":"2.0.0","#), ("hello refused: update the backend", "")] {
            let mut c = client();
            c.connect(URL, 0).unwrap();
            c.request(br#"{"program":"X"}"#, 0);
            c.on_open(1);
            c.on_message(&format!(r#"{{"t":"hello","protocol":1,"protocol_min":1,"contract":"2.0.0","accepted":"refused","message":"m",{upd}"server":"s","max_frame_bytes":1}}"#));
            row(case, &answers(&mut c)[0].1);
            assert_eq!((c.state(), c.sock.closed), (LinkState::Closed, 1), "no retry after a refusal");
            c.on_close(2);
            assert_eq!(c.retry_at, None);
        }
        // a server error frame is handed through, class and code intact
        let mut c = ready_client();
        let id = c.request(br#"{"program":"NOSUCH"}"#, 0);
        c.on_message(&format!(r#"{{"t":"error","id":{id},"class":"authentication","code":2000,"message":"no token: authenticate before calling"}}"#));
        row("server: unknown or private program", &answers(&mut c)[0].1);
        for (case, class_, code_, may) in &rows {
            println!("{case:<38} {class_:<15} {code_:<5} may_have_run={may:?}");
        }
        let got: Vec<(&str, u64)> = rows.iter().map(|(_, c, n, _)| (c.as_str(), *n)).collect();
        assert_eq!(
            got,
            [
                ("transport", 1000),
                ("transport", 1000),
                ("transport", 1001),
                ("transport", 1002),
                ("limit", 7000),
                ("validation", 4001),
                ("validation", 4001),
                ("validation", 4001),
                ("validation", 4001),
                ("limit", 7000),
                ("version", 6000),
                ("version", 6001),
                ("authentication", 2000),
            ]
        );
        assert_eq!(rows[2].3, Some(true), "a lost call may have run");
        assert_eq!(rows[1].3, Some(false), "an unreachable backend never saw it");
    }

    #[test]
    fn an_expired_token_is_refreshed_and_the_call_repeated_once_with_its_key() {
        let mut c = client();
        c.set_token(Some("old".into()));
        c.connect(URL, 0).unwrap();
        c.on_open(1);
        c.on_message(HELLO_OK);
        c.on_message(r#"{"t":"auth-ok","sub":"u","expires_in":1}"#);
        let id = c.request(br#"{"program":"X"}"#, 2);
        let expired = format!(r#"{{"t":"error","id":{id},"class":"authentication","code":2002,"message":"token expired"}}"#);
        c.on_message(&expired);
        assert!(c.refresh_requested, "the owner is asked to refresh");
        assert!(answers(&mut c).is_empty(), "the program is not told yet");
        c.set_token(Some("new".into()));
        let sent = sent_json(&c);
        assert!(sent.iter().any(|m| m["t"] == "auth" && m["token"] == "new"));
        let calls: Vec<&Value> = sent.iter().filter(|m| m["t"] == "call" && m["id"] == id).collect();
        assert_eq!(calls.len(), 2, "the call was repeated");
        assert_eq!(calls[0]["key"], calls[1]["key"], "under the same key");
        // A second expiry is final: no loop.
        c.on_message(&expired);
        let a = answers(&mut c);
        assert_eq!((a.len(), a[0].1["error"]["code"].as_u64()), (1, Some(2002)));
        println!("2002: refreshed, repeated once under key {}, a second 2002 reaches the program", calls[0]["key"]);
    }

    #[test]
    fn a_safe_program_is_repeated_once_after_a_drop_and_an_unsafe_one_fails_with_may_have_run() {
        let mut c = ready_client();
        let safe = c.request(br#"{"program":"FUND-TOTALS","args":[],"returning":true}"#, 1);
        let unsafe_ = c.request(br#"{"program":"FUND-SAVE","args":[],"returning":true}"#, 1);
        let first_key = sent_json(&c)[0]["key"].clone();
        c.on_close(5);
        let a = answers(&mut c);
        assert_eq!(a.len(), 1, "only the unsafe call is failed: {a:?}");
        assert_eq!((a[0].0, a[0].1["error"]["code"].as_u64(), a[0].1["error"]["may_have_run"].as_bool()), (unsafe_, Some(1001), Some(true)));
        assert_eq!(c.in_flight(), 1, "the safe one waits for the next connection");
        c.tick(1_000);
        c.on_open(1_001);
        c.on_message(HELLO_OK);
        let again: Vec<Value> = sent_json(&c).into_iter().filter(|m| m["t"] == "call" && m["id"] == safe).collect();
        assert_eq!(again.len(), 2, "sent, then repeated once");
        assert_eq!(again[1]["key"], first_key, "the repeat carries the same key, so the backend answers from its store");
        // A second drop is final.
        c.on_close(1_100);
        let a = answers(&mut c);
        assert_eq!((a[0].0, a[0].1["error"]["code"].as_u64(), a[0].1["error"]["may_have_run"].as_bool()), (safe, Some(1001), Some(true)));
        println!("safe FUND-TOTALS: repeated once under key {first_key}, a second drop fails it; unsafe FUND-SAVE: failed 1001 may_have_run=true");
    }

    #[test]
    fn the_players_own_ids_start_at_0x80000000_and_only_module_answers_are_size_limited() {
        assert_eq!(PLAYER_ID_BASE, crate::auth::wire::ID_BASE);
        assert!(crate::grid::ID_BASE > PLAYER_ID_BASE);
        let mut c = ready_client();
        let big = format!(r#"{{"ROWS":"{}"}}"#, "z".repeat(70_000));
        c.request_as(PLAYER_ID_BASE + 7, br#"{"identity":"refresh"}"#, 0);
        c.on_message(&reply(PLAYER_ID_BASE + 7, &big));
        let (id, v) = answers(&mut c).remove(0);
        assert_eq!(id, PLAYER_ID_BASE + 7);
        assert!(v.get("result").is_some(), "the player's own answer is not cut at 64 KiB");
    }

    #[test]
    fn a_warning_hello_connects_and_json_pings_are_not_part_of_the_protocol() {
        let mut c = client();
        c.connect(URL, 0).unwrap();
        c.on_open(1);
        c.on_message(r#"{"t":"hello","protocol":1,"protocol_min":1,"contract":"1.2.0","accepted":"warn","message":"update when convenient","update_to":"1.2.0","server":"s","max_frame_bytes":1}"#);
        assert_eq!(c.state(), LinkState::Ready);
        assert!(c.take_out().iter().any(|o| matches!(o, Out::Warning(w) if w.contains("update when convenient"))));
        let before = c.sock.sent.len();
        c.on_message(r#"{"type":"ping"}"#);
        c.on_message(r#"{"t":"progress","id":1,"percent":5,"message":""}"#);
        assert_eq!(c.sock.sent.len(), before, "no pong, nothing sent");
    }

    #[test]
    fn a_call_in_flight_when_the_connection_drops_fails_and_a_later_one_does_not() {
        let mut c = ready_client();
        let in_flight = c.request(br#"{"program":"X"}"#, 2);
        c.on_close(3);
        assert_eq!(c.state(), LinkState::Backoff);
        let a = answers(&mut c);
        assert_eq!(a[0].0, in_flight);
        assert_eq!(a[0].1["error"]["class"], "transport");
        assert_eq!(a[0].1["error"]["may_have_run"], true, "WASM-014 R9: the program is told it may have run");
        // While backing off, a new call waits; after the reconnection it is sent and answered normally.
        let later = c.request(br#"{"program":"Y"}"#, 100);
        assert!(c.sock.sent.iter().all(|s| !s.contains("\"Y\"")));
        c.tick(10_000);
        assert_eq!(c.state(), LinkState::Connecting);
        c.on_open(10_001);
        c.on_message(HELLO_OK);
        assert!(c.sock.sent.iter().any(|s| s.contains("\"Y\"")));
        c.on_message(&reply(later, "\"ok\""));
        let a = answers(&mut c);
        assert!(a.iter().any(|(id, v)| *id == later && v.get("error").is_none()));
    }

    /// After a lost connection the page reconnects with back-off while a safe call waits for its repeat;
    /// the repeat goes out when it works, and a later loss starts the series again.
    #[test]
    fn reconnection_backs_off_exponentially_and_resets_when_it_works() {
        let mut c = ready_client();
        c.cfg.call_timeout_ms = 1_000_000;
        let safe = c.request(br#"{"program":"FUND-TOTALS"}"#, 0);
        let mut now = 0u64;
        c.on_close(now);
        let mut waits = Vec::new();
        for _ in 0..8 {
            let at = c.retry_at.unwrap();
            waits.push(at - now);
            now = at;
            c.tick(now);
            assert_eq!(c.state(), LinkState::Connecting);
            c.on_error(now);
            assert_eq!(c.in_flight(), 1, "the safe call goes on waiting through failed attempts");
        }
        assert_eq!(waits, [250, 500, 1000, 2000, 4000, 8000, 10_000, 10_000], "doubling, capped at backoff_max");
        c.tick(c.retry_at.unwrap());
        c.on_open(now);
        c.on_message(HELLO_OK);
        assert_eq!(sent_json(&c).iter().filter(|m| m["t"] == "call" && m["id"] == safe).count(), 2, "repeated once it worked");
        c.on_message(&reply(safe, "{}"));
        c.on_close(now + 1);
        assert_eq!(c.retry_at.unwrap() - (now + 1), 250, "a working connection starts the series again");
        // With nothing waiting, a failed attempt ends the loop.
        c.tick(now + 251);
        c.on_error(now + 252);
        assert_eq!((c.state(), c.retry_at), (LinkState::Closed, None));
        println!("back-off waits (ms): {waits:?}");
    }

    #[test]
    fn jitter_stays_within_a_quarter_of_the_base() {
        for j in [0u32, 250, 500, 750, 1000] {
            let mut c = ready_client();
            c.jitter = Box::new(move || j);
            c.on_close(0);
            let w = c.retry_at.unwrap();
            assert!((187..=313).contains(&w), "jitter {j}: waited {w}");
        }
    }

    #[test]
    fn a_call_that_is_never_answered_times_out_with_its_own_error() {
        let mut c = ready_client();
        let id = c.request(br#"{"program":"SLOW"}"#, 1_000);
        c.tick(1_000 + 29_999);
        assert!(answers(&mut c).is_empty());
        c.tick(1_000 + 30_000);
        let (i, v) = answers(&mut c).remove(0);
        assert_eq!(i, id);
        assert_eq!((v["error"]["class"].as_str(), v["error"]["code"].as_u64()), (Some("transport"), Some(1002)));
        assert_eq!(c.next_deadline(), None);
        // A call held behind a handshake that never completes ends at its timeout too.
        let mut c = client();
        c.connect(URL, 0).unwrap();
        c.on_open(1);
        c.request(br#"{"program":"X"}"#, 2);
        c.tick(30_002);
        assert_eq!(answers(&mut c)[0].1["error"]["code"].as_u64(), Some(1002));
    }

    #[test]
    fn shutdown_closes_the_socket_and_forgets_the_calls() {
        let mut c = ready_client();
        c.request(br#"{"program":"X"}"#, 2);
        c.shutdown();
        assert_eq!((c.state(), c.in_flight(), c.sock.closed), (LinkState::Closed, 0, 1));
        c.on_close(5);
        assert_eq!(c.state(), LinkState::Closed, "no reconnection after an unmount");
    }

    #[test]
    fn a_sign_out_closes_the_connection_and_the_next_call_opens_an_anonymous_one() {
        let mut c = client();
        c.set_token(Some("tok".into()));
        c.connect(URL, 0).unwrap();
        c.on_open(1);
        c.on_message(HELLO_OK);
        c.on_message(r#"{"t":"auth-ok","sub":"u","expires_in":300}"#);
        let first_session = c.session().to_owned();
        let sent_call = c.request(br#"{"program":"X"}"#, 2);
        c.reset_session();
        assert_eq!((c.state(), c.sock.closed), (LinkState::Closed, 1));
        let a = answers(&mut c);
        assert!(a.iter().any(|(id, v)| *id == sent_call && v["error"]["may_have_run"] == true), "a call that was sent is failed as possibly run: {a:?}");
        c.on_close(5);
        assert_eq!(c.state(), LinkState::Closed, "no reconnection of the old session");
        assert_ne!(c.session(), first_session, "a sign-out starts a new session");
        // The next call opens a new connection with no token: no `auth` message.
        c.connect(URL, 10).unwrap();
        c.request(br#"{"program":"Y"}"#, 11);
        c.on_open(12);
        c.on_message(HELLO_OK);
        let sent = sent_json(&c);
        assert_eq!(sent.iter().filter(|m| m["t"] == "auth").count(), 1, "only the first session's token was ever sent: {sent:?}");
        assert!(sent.iter().any(|m| m["t"] == "call" && m["program"] == "Y"));
    }

    #[test]
    fn the_session_id_is_never_written_to_storage() {
        // The mock socket sees the same session on a reconnect.
        let mut c = client();
        c.seed_session(0x1234_5678_9abc_def0);
        c.connect(URL, 0).unwrap();
        c.on_open(1);
        c.on_message(HELLO_OK);
        c.on_close(2);
        c.tick(10_000);
        c.on_open(10_001);
        let hellos: Vec<String> = sent_json(&c).iter().filter(|m| m["t"] == "hello").map(|m| m["session"].as_str().unwrap().to_owned()).collect();
        assert_eq!(hellos.len(), 2);
        assert_eq!(hellos[0], hellos[1], "the same session on a reconnect");
        assert_eq!(hellos[0].len(), 16);
        // And the connection's code never touches browser storage.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut hits = Vec::new();
        for f in ["net.rs", "api.rs"] {
            let text = std::fs::read_to_string(root.join(f)).unwrap();
            let code = text.split("#[cfg(test)]").next().unwrap();
            for (n, line) in code.lines().enumerate() {
                if ["local_storage", "session_storage", "indexed_db"].iter().any(|w| line.contains(w)) {
                    hits.push(format!("{f}:{}", n + 1));
                }
            }
        }
        println!("storage hits in net.rs/api.rs: {} {hits:?}", hits.len());
        assert!(hits.is_empty());
    }

    #[test]
    fn grep_has_no_client_class_or_may_have_been_received() {
        let mut hits = Vec::new();
        let mut walk = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(d) = walk.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let text = std::fs::read_to_string(&p).unwrap();
                    for (n, line) in text.lines().enumerate() {
                        let needles = [concat!("may_have_been", "_received"), concat!("class::", "CLIENT"), concat!("class: \"", "client\"")];
                        if needles.iter().any(|w| line.contains(w)) && !line.contains("fn grep_has_no_client_class") {
                            hits.push(format!("{}:{}", p.display(), n + 1));
                        }
                    }
                }
            }
        }
        println!("hits: {} {hits:?}", hits.len());
        assert!(hits.is_empty());
    }

    /// T7's codes-equality check, the cheap half: every code the player emits equals
    /// `cobolt_wire::error::codes` (read from its source; the player cannot link the crate).
    #[test]
    fn the_players_codes_equal_cobolt_wire_error_codes() {
        let wire = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../cobolt-wire/src/error.rs")).unwrap();
        let of = |name: &str| -> u32 {
            let at = wire.find(&format!("pub const {name}: u32 = ")).unwrap_or_else(|| panic!("{name} is not in cobolt-wire"));
            let rest = &wire[at..];
            rest[rest.find('=').unwrap() + 1..rest.find(';').unwrap()].trim().parse().unwrap()
        };
        let mine = [
            ("NO_CONNECTION", code::NO_CONNECTION),
            ("CLOSED", code::CLOSED),
            ("TIMEOUT", code::TIMEOUT),
            ("NOT_CONFIGURED", code::NOT_CONFIGURED),
            ("TOKEN_EXPIRED", code::TOKEN_EXPIRED),
            ("BAD_MESSAGE", code::BAD_MESSAGE),
            ("CLIENT_TOO_OLD", code::CLIENT_TOO_OLD),
            ("CLIENT_TOO_NEW", code::CLIENT_TOO_NEW),
            ("FRAME_TOO_LARGE", code::FRAME_TOO_LARGE),
        ];
        for (name, v) in mine {
            println!("{name:<16} player {v:<5} cobolt_wire {}", of(name));
            assert_eq!(v, of(name), "{name}");
        }
        for c in [class::TRANSPORT, class::AUTHENTICATION, class::VALIDATION, class::VERSION, class::LIMIT] {
            assert!(wire.contains(&format!("=> \"{c}\"")), "{c} is a class of cobolt-wire");
        }
        let contract = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../cobolt-wire/src/contract.rs")).unwrap();
        assert!(contract.contains(&format!("pub const PROTOCOL: u32 = {PROTOCOL};")));
    }

    /// The module's answer staging area is what [`MODULE_ANSWER_LIMIT`] says.
    #[test]
    fn the_module_answer_limit_is_the_runtimes_staging_area() {
        let st = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../cobolt-wasm-rt/src/state.rs")).unwrap();
        assert!(st.contains("pub const FRAME_SIZE: u32 = 64 * 1024;"), "cobolt-wasm-rt changed its FRAME_SIZE: update MODULE_ANSWER_LIMIT");
        assert_eq!(MODULE_ANSWER_LIMIT, 65_536);
    }
}
