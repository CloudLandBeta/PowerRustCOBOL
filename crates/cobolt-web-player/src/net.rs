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
//! * `authentication` 2002 (token expired), 2001 (token invalid) and, when the player holds a token,
//!   2000 (token missing: the server has none for this connection) refresh the token and repeat
//!   the call once; the server refused before running anything;
//! * a broken connection is tried again by the one retry policy (`cobolt_wire::retry`, WASM-003
//!   R25): attempts with windows of 10, 20 and 30 s, an attempt that fails early waiting its window
//!   out; meanwhile the application is *blocked* (R26, [`Out::Blocked`]) and a call's timeout does
//!   not run. When an attempt succeeds, a call in flight at the break is repeated once, under the
//!   same key, only when the backend's interface declares the program `idempotent` (R9); otherwise
//!   it fails with `may_have_run: true`. When the last attempt fails every waiting call fails with
//!   the class `transport`, the backend is *unreachable* ([`Out::Unreachable`]) and nothing more is
//!   tried until the application calls again;
//! * every call ends: an answer, a failure, or `transport` 1002 at its timeout (then `cancel`).
//!
//! The session id lives in [`Client`] only — never in browser storage (WASM-014 Q5).

use std::collections::{BTreeMap, HashSet};

use serde_json::{json, Value};

/// Is a call the user (or the initial load) started in flight, so that the loading effect shows (2.0.413)? `in_flight` is the
/// ids the connection has out; `automatic` holds the ids a Timer's `onTick` handler made, which are automatic refreshes and
/// show nothing. The ids that are no longer in flight are forgotten.
pub fn user_call_in_flight(in_flight: &[u32], automatic: &mut HashSet<u32>) -> bool {
    automatic.retain(|id| in_flight.contains(id));
    in_flight.iter().any(|id| !automatic.contains(id))
}

/// How long a user's call may be out before the loading effect starts (2.0.415). A call that is answered sooner never shows
/// the effect at all, so a quick answer does not flash the stripes.
pub const LOADING_EFFECT_DELAY_MS: u64 = 2000;

/// Holds the loading effect back until a user's call has been waiting for [`LOADING_EFFECT_DELAY_MS`] (2.0.415).
///
/// It is asked every time the connection changes (`busy` is [`user_call_in_flight`]'s answer, `now` the clock in
/// milliseconds). The wait is counted from the moment a user's call went out with none before it; when no user's call is out
/// the count is dropped, so the next call waits its own 2 s, and the effect (if it was up) goes away completely.
#[derive(Debug, Default)]
pub struct EffectGate {
    waiting_since: Option<u64>,
}

impl EffectGate {
    /// Should the effect be showing at `now`?
    pub fn show(&mut self, busy: bool, now: u64) -> bool {
        if !busy {
            self.waiting_since = None;
            return false;
        }
        let since = *self.waiting_since.get_or_insert(now);
        now.saturating_sub(since) >= LOADING_EFFECT_DELAY_MS
    }

    /// Milliseconds from `now` until the effect is due, while a call is waiting for it; `None` when none is waiting or the
    /// effect is already due (nothing left to wait for).
    pub fn due_in(&self, now: u64) -> Option<u64> {
        let due = self.waiting_since? + LOADING_EFFECT_DELAY_MS;
        (due > now).then(|| due - now)
    }
}

mod transfer;
pub use transfer::{plain_name, CHUNK_BYTES};

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
    pub const TOKEN_MISSING: u32 = 2000;
    pub const TOKEN_INVALID: u32 = 2001;
    pub const TOKEN_EXPIRED: u32 = 2002;
    pub const BAD_MESSAGE: u32 = 4001;
    pub const CLIENT_TOO_OLD: u32 = 6000;
    pub const CLIENT_TOO_NEW: u32 = 6001;
    pub const FRAME_TOO_LARGE: u32 = 7000;
}

/// The wire protocol this player speaks (`cobolt_wire::contract::PROTOCOL`).
pub const PROTOCOL: u32 = 2;

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
    /// A text message: only the `hello`, the one message that is not sealed.
    fn send(&mut self, text: &str);
    /// A sealed frame (WASM-003 R24): every message after the `hello`s.
    fn send_binary(&mut self, bytes: &[u8]);
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
    /// The application is blocked while the connection is tried again (`true`, WASM-003 R26), or no
    /// longer is — the connection is back or the policy gave up (`false`).
    Blocked(bool),
    /// The policy gave up: the backend could not be reached (`true`, WASM-016 R21); `false` once a
    /// later connection works.
    Unreachable(bool),
    /// Where the retry cycle stands changed: what the "connecting" modal shows.
    Net(Progress),
    /// The backend's `hello` was not ok (WASM-014 R2): `onBackendVersion` on the main form. `state` is `warn` or `refused`.
    BackendVersion { state: String, message: String, update_to: String },
    /// A call's reply carried a warning (WASM-014 R6): `onBackendWarning` on the main form, with the program's name.
    BackendWarning { program: String, message: String },
    /// A long call reported progress (WASM-014 R13): the host sets the controls the form declared with `WS-SHOW-PROGRESS`.
    Progress { id: u32, percent: u8, message: String },
    /// An item of a stream (R14): `json` is the group as the backend sent it; `restart` is true for the first after a re-subscription.
    StreamItem { sub: u32, json: String, restart: bool },
    /// A stream ended: `ended`, `unsubscribed`, `refused: …` or the backend's failure.
    StreamEnd { sub: u32, reason: String },
    /// The backend holds `have` bytes of upload `id`: read the next piece from there (R15).
    PutReady { id: u32, have: u64 },
    /// Upload `id` is stored under the backend's own name `file`.
    PutDone { id: u32, file: String, size: u64 },
    /// The address to navigate to for download `id` (a short-lived path under the page's own origin).
    FileUrl { id: u32, path: String, expires_in: u64, size: u64, sha256: String },
    /// An upload or a download failed, or was refused before any frame left.
    TransferFailed { id: u32, class: String, code: u32, message: String },
}

/// How an attempt of the retry policy ended badly (what the "connecting" modal calls the *last status*).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// The socket closed before the server answered: nothing is listening, or the network is down.
    NoConnection,
    /// The socket opened, but closed during the handshake (the server's `hello`, the token).
    Handshake,
    /// The attempt did not finish inside its window.
    NoAnswer,
}

/// What the current attempt is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Opening the socket.
    Connecting,
    /// Socket open; the handshake is running.
    Handshake,
    /// The attempt failed; its window is waited out before the next one.
    Waiting,
    /// Every attempt failed; nothing more is tried until the user retries.
    GaveUp,
}

/// What the "connecting" modal shows (WASM-016 R20–R21): where the retry cycle stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    /// The server being reached (`host:port` of the address in use).
    pub server: String,
    /// The attempt in progress, or the last one made, counted from 1.
    pub attempt: u32,
    /// How many attempts a cycle makes.
    pub of: u32,
    /// How the previous attempt ended; `None` for the first attempt.
    pub last: Option<Failure>,
    pub phase: Phase,
    /// How this attempt ended, once it has (while waiting for the next, and after giving up).
    pub failed: Option<Failure>,
    /// The window of this attempt, in seconds.
    pub window_s: u64,
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
    /// Attempt `attempt` of the retry policy failed; the next starts at `window_ends`.
    Retrying { attempt: u32, window_ends: u64 },
}

#[derive(Debug, Clone)]
pub struct Config {
    /// The backend addresses the player may connect to (`wss://api.example.com/ws`).
    pub origins: Vec<String>,
    pub call_timeout_ms: u64,
    pub max_frame: usize,
    /// How a broken connection is tried again (WASM-003 R25: the defaults are the only values a
    /// release uses; tests pass a shorter one).
    pub retry: cobolt_wire::retry::Policy,
    /// Allow `ws://` to a non-loopback host (an explicit *unsafe* setting, WASM-013 R11).
    pub unsafe_plain_ws: bool,
    /// The contract version the application was built against (`hello.contract`).
    pub contract: String,
    /// Who this client is, for the backend's logs (`hello.client`).
    pub client: String,
    /// The programs the backend's interface declares `idempotent`: only these are repeated after a
    /// lost connection (WASM-014 R9).
    pub idempotent: Vec<String>,
    /// Where the 32 bytes of a key pair and the 16 of a connection's nonce come from: `crypto.getRandomValues` in
    /// the browser, never `Math.random` (WASM-003 R24). `None` means the owner gave none, and a connection
    /// then cannot start: there is no plaintext mode.
    pub entropy: Option<fn(&mut [u8]) -> bool>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            origins: Vec::new(),
            call_timeout_ms: 30_000,
            max_frame: 384 * 1024,
            retry: cobolt_wire::retry::Policy::default(),
            unsafe_plain_ws: false,
            contract: DEFAULT_CONTRACT.into(),
            client: format!("prc-wasm/{}", env!("CARGO_PKG_VERSION")),
            idempotent: Vec::new(),
            entropy: None,
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
    /// When the module made the call: its timeout runs from here, whatever the repeats. Moved on by
    /// the time the application was blocked, so a retry cycle does not count against it (D12).
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
    /// Sent on a connection that broke: decided when the next one works (repeated or failed).
    lost: bool,
    /// Repeated once after `authentication` 2000, 2001 or 2002.
    auth_repeated: bool,
    /// The code the server refused with (2000, 2001 or 2002) when `auth_repeated` was set, 0 before: what the
    /// program is told if the refresh fails, so a wrong token still reads as one.
    auth_refused: u32,
    /// Repeated once after a lost connection.
    drop_repeated: bool,
}

/// The attempt of the retry policy in progress: its number (from 1) and when its window ends.
#[derive(Debug, Clone, Copy)]
struct Attempt {
    n: u32,
    window_ends: u64,
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
    /// The connection attempt in progress or being waited out; `None` while ready or closed.
    attempt: Option<Attempt>,
    /// Since when the application is blocked (R26): set at a break, cleared at success or give-up.
    blocked_since: Option<u64>,
    /// The last cycle gave up and no connection has worked since.
    unreachable: bool,
    /// How the attempt before the one shown ended, and how the one shown did (once it has).
    prior_failure: Option<Failure>,
    last_failure: Option<Failure>,
    shown_attempt: u32,
    /// The progress last handed out, so [`Out::Net`] is sent only when it changes.
    sent_progress: Option<Progress>,
    /// The latest time an event carried (a message carries none: its time is the open's or later).
    clock: u64,
    wanted: bool,
    /// A connection has worked since `connect`: what a return from the background may find gone (WASM-012 R15).
    had_link: bool,
    out: Vec<Out>,
    /// Ask the owner to refresh the token (the server said it expired); it then calls [`Client::set_token`].
    pub refresh_requested: bool,
    /// The key pair of the sealed channel, made when the first connection opened (WASM-003 R24, D8).
    keys: Option<cobolt_wire::seal::KeyPair>,
    /// This connection's own nonce, kept from the `hello` until the server's answers it.
    own_nonce: [u8; cobolt_wire::seal::NONCE_LEN],
    /// The halves of the channel, from the server's `hello` to the end of the connection.
    tx: Option<cobolt_wire::seal::Sender>,
    rx: Option<cobolt_wire::seal::Receiver>,
    /// Streams, uploads and downloads in progress (WASM-014 T15).
    subs: BTreeMap<u32, transfer::Sub>,
    puts: BTreeMap<u32, transfer::Put>,
    gets: BTreeMap<u32, String>,
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

/// A 128-bit session id as 32 lowercase hex digits (what the backend's `hello` accepts, WASM-014 R10), from a seed: two
/// xorshift draws (the browser seeds it from `Math.random` and the clock).
fn session_from(seed: &mut u64) -> String {
    let mut draw = || {
        let mut x = *seed | 1;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *seed = x;
        x
    };
    let (hi, lo) = (draw(), draw());
    format!("{hi:016x}{lo:016x}")
}

impl<S: Socket> Client<S> {
    pub fn new(cfg: Config, sock: S) -> Self {
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
            attempt: None,
            blocked_since: None,
            unreachable: false,
            prior_failure: None,
            last_failure: None,
            shown_attempt: 0,
            sent_progress: None,
            clock: 0,
            wanted: false,
            had_link: false,
            out: Vec::new(),
            refresh_requested: false,
            keys: None,
            own_nonce: [0; cobolt_wire::seal::NONCE_LEN],
            tx: None,
            rx: None,
            subs: BTreeMap::new(),
            puts: BTreeMap::new(),
            gets: BTreeMap::new(),
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

    /// The ids of the requests in flight, in order (2.0.413): the loading effect asks which of them are the user's.
    pub fn in_flight_ids(&self) -> Vec<u32> {
        self.inflight.keys().copied().collect()
    }

    /// The application is blocked while the connection is tried again (R26).
    pub fn blocked(&self) -> bool {
        self.blocked_since.is_some()
    }

    /// The last retry cycle gave up and no connection has worked since.
    pub fn unreachable(&self) -> bool {
        self.unreachable
    }

    /// Where the retry cycle stands, or `None` while no cycle runs (the link is ready or was never asked for).
    pub fn progress(&self) -> Option<Progress> {
        if !self.blocked() && !self.unreachable {
            return None;
        }
        let phase = if self.unreachable && !self.blocked() {
            Phase::GaveUp
        } else {
            match self.state {
                LinkState::Greeting | LinkState::Authenticating => Phase::Handshake,
                LinkState::Retrying { .. } => Phase::Waiting,
                _ => Phase::Connecting,
            }
        };
        let server = self.url.as_deref().unwrap_or("").split("://").last().unwrap_or("").split('/').next().unwrap_or("").to_owned();
        Some(Progress {
            server,
            attempt: self.shown_attempt.max(1),
            of: self.cfg.retry.attempts,
            last: self.prior_failure,
            phase,
            failed: if matches!(phase, Phase::Waiting | Phase::GaveUp) { self.last_failure } else { None },
            window_s: self.cfg.retry.window(self.shown_attempt.max(1)).unwrap_or(0) / 1000,
        })
    }

    pub fn take_out(&mut self) -> Vec<Out> {
        let now = self.progress();
        if now != self.sent_progress {
            if let Some(p) = now.clone() {
                self.out.push(Out::Net(p));
            }
            self.sent_progress = now;
        }
        std::mem::take(&mut self.out)
    }

    /// The user asked to try again after the cycle gave up: the whole cycle starts over, from attempt 1 (WASM-016 R21).
    pub fn retry(&mut self, now: u64) {
        self.clock = self.clock.max(now);
        if !(self.unreachable && self.wanted && self.url.is_some() && self.state == LinkState::Closed) {
            return;
        }
        self.unreachable = false;
        self.out.push(Out::Unreachable(false));
        self.block(now);
        self.start_attempt(1, now);
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
                self.send_msg(&json!({"t": "auth", "token": t}).to_string());
                // Repeat what the server refused for an expired token, once, under the same key.
                for id in waiting {
                    self.transmit(id);
                }
            }
            Some(_) => {} // the waiting calls go out when the connection is ready
            None => {
                for id in waiting {
                    // Told what the server said: an expired session, or a token it refused outright.
                    let said = self.inflight.get(&id).map_or(0, |f| f.auth_refused);
                    let (wire_code, why) = match said {
                        code::TOKEN_INVALID => (code::TOKEN_INVALID, "the token was refused and could not be refreshed"),
                        code::TOKEN_MISSING => (code::TOKEN_MISSING, "the server had no token and none could be refreshed"),
                        _ => (code::TOKEN_EXPIRED, "the session expired and could not be refreshed"),
                    };
                    self.fail(id, class::AUTHENTICATION, wire_code, why, Some(false));
                }
            }
        }
    }

    /// Connect to `url` (must be one of the configured addresses).
    pub fn connect(&mut self, url: &str, now: u64) -> Result<(), String> {
        origin_allowed(&self.cfg, url)?;
        self.clock = self.clock.max(now);
        self.url = Some(url.to_owned());
        self.wanted = true;
        self.start_attempt(1, now);
        Ok(())
    }

    /// Open attempt `n` of the retry policy, given its window from `now`; past the last attempt,
    /// give up.
    fn start_attempt(&mut self, n: u32, now: u64) {
        let Some(window) = self.cfg.retry.window(n) else {
            self.give_up(now);
            return;
        };
        let Some(url) = self.url.clone() else { return };
        self.attempt = Some(Attempt { n, window_ends: now.saturating_add(window) });
        self.shown_attempt = n;
        self.prior_failure = if n == 1 { None } else { self.last_failure };
        self.last_failure = None;
        self.set_state(LinkState::Connecting);
        self.sock.open(&url);
    }

    fn block(&mut self, now: u64) {
        if self.blocked_since.is_none() {
            self.blocked_since = Some(now);
            self.out.push(Out::Blocked(true));
        }
    }

    /// No longer blocked: the time spent blocked does not count against any call's timeout.
    fn unblock(&mut self, now: u64) {
        if let Some(since) = self.blocked_since.take() {
            for f in self.inflight.values_mut() {
                f.sent_at = f.sent_at.saturating_add(now.saturating_sub(f.sent_at.max(since)));
            }
            self.out.push(Out::Blocked(false));
        }
    }

    /// An attempt failed (refused, broken during the handshake, or out of its window). Nothing to
    /// wait for and nothing broken: stop — a page whose backend is down does not knock on its door
    /// forever. Otherwise the application is blocked and the attempt's window is waited out.
    fn attempt_failed(&mut self, now: u64, why: Failure) {
        self.last_failure = Some(why);
        let Some(a) = self.attempt else {
            self.set_state(LinkState::Closed);
            return;
        };
        if self.inflight.is_empty() && !self.blocked() {
            self.attempt = None;
            self.set_state(LinkState::Closed);
            return;
        }
        self.block(now);
        if now >= a.window_ends {
            self.start_attempt(a.n + 1, now);
        } else {
            self.set_state(LinkState::Retrying { attempt: a.n, window_ends: a.window_ends });
        }
    }

    /// The last attempt failed: every waiting call fails `transport`, the backend is unreachable,
    /// and nothing is tried until the application calls again (O8).
    fn give_up(&mut self, now: u64) {
        self.attempt = None;
        let m = "the backend could not be reached";
        let ids: Vec<u32> = self.inflight.keys().copied().collect();
        for id in ids {
            let ever = self.inflight.get(&id).is_some_and(|f| f.ever_sent);
            if ever {
                self.fail(id, class::TRANSPORT, code::CLOSED, m, Some(true));
            } else {
                self.fail(id, class::TRANSPORT, code::NO_CONNECTION, m, Some(false));
            }
        }
        self.unblock(now);
        if !self.unreachable {
            self.unreachable = true;
            self.out.push(Out::Unreachable(true));
        }
        self.set_state(LinkState::Closed);
    }

    /// Leave a cycle for good (a refusal, a sign-out, an unmount): not blocked any more.
    fn end_cycle(&mut self, now: u64) {
        self.attempt = None;
        self.unblock(now);
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
        self.clock = self.clock.max(now);
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
            InFlight {
                sent_at: now,
                frame: text.trim().to_owned(),
                program,
                key,
                sent: false,
                ever_sent: false,
                lost: false,
                auth_repeated: false,
                auth_refused: 0,
                drop_repeated: false,
            },
        );
        match self.state {
            LinkState::Ready => self.transmit(id),
            LinkState::Closed if self.wanted => {
                // The link went away, or the last cycle gave up: this call starts a new one (O8).
                self.start_attempt(1, now);
            }
            // Held until Ready; while the policy retries, its timing is the spec's, not the caller's.
            LinkState::Connecting | LinkState::Greeting | LinkState::Authenticating | LinkState::Retrying { .. } => {}
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
        self.send_msg(&msg);
    }

    fn fail(&mut self, id: u32, class_: &str, code_: u32, message: &str, may_have_run: Option<bool>) {
        if self.inflight.remove(&id).is_some() {
            self.out.push(Out::Answer { id, frame: error_frame(id, class_, code_, message, may_have_run) });
        }
    }

    // ── events from the socket ──────────────────────────────────────────────

    pub fn on_open(&mut self, now: u64) {
        self.clock = self.clock.max(now);
        self.tx = None;
        self.rx = None;
        // The key pair of the application, made once; the nonce of this connection, made now (R24, D8). Both
        // come from the owner's entropy: without it there is no connection, and no plaintext mode.
        let drawn = self.cfg.entropy.is_some_and(|entropy| {
            if self.keys.is_none() {
                let mut seed = [0u8; 32];
                let ok = entropy(&mut seed);
                if ok {
                    self.keys = Some(cobolt_wire::seal::KeyPair::from_bytes(seed));
                }
                seed.fill(0);
                if !ok {
                    return false;
                }
            }
            entropy(&mut self.own_nonce)
        });
        if !drawn {
            self.out.push(Out::Warning("the sealed channel has no random source: the connection was not started".into()));
            self.fail_all(class::TRANSPORT, code::CLOSED, "the connection has no random source");
            self.wanted = false;
            self.sock.close();
            self.set_state(LinkState::Closed);
            return;
        };
        let key = self.keys.as_ref().map(|k| cobolt_wire::seal::encode_b64(&k.public())).unwrap_or_default();
        let hello = json!({"t": "hello", "protocol": PROTOCOL, "contract": self.cfg.contract, "client": self.cfg.client, "session": self.session, "key": key, "nonce": cobolt_wire::seal::encode_b64(&self.own_nonce)});
        self.set_state(LinkState::Greeting);
        self.sock.send(&hello.to_string());
    }

    /// A message to the backend: sealed, always. Before the channel is agreed nothing but the `hello` may leave.
    fn send_msg(&mut self, text: &str) {
        let Some(tx) = self.tx.as_mut() else { return };
        match tx.seal(text) {
            Ok(frame) => self.sock.send_binary(&frame),
            Err(_) => self.sock.close(),
        }
    }

    /// A text message from the socket: the server's `hello`, the one message that is not sealed. After it every
    /// message is a sealed frame, so a text message is not from a backend that kept to the protocol.
    pub fn on_text(&mut self, text: &str) {
        if self.rx.is_some() {
            self.out.push(Out::Warning("the backend sent an unsealed message after the key exchange; the connection was closed".into()));
            self.sock.close();
            return;
        }
        self.on_message(text);
    }

    /// A sealed frame from the socket: opened, then handled as the message it carries. One that does not open
    /// ends the connection (it was tampered with, replayed or reordered).
    pub fn on_binary(&mut self, bytes: &[u8]) {
        let opened = self.rx.as_mut().map(|rx| rx.open(bytes));
        match opened {
            Some(Ok(text)) => self.on_message(&text),
            _ => {
                self.out.push(Out::Warning("a frame from the backend did not open; the connection was closed".into()));
                self.sock.close();
            }
        }
    }

    fn flush_held(&mut self) {
        let held: Vec<u32> = self.inflight.iter().filter(|(_, f)| !f.sent && !f.lost).map(|(id, _)| *id).collect();
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

    /// The connection is past `hello` (and `auth`, when there is a token): decide what the break
    /// left behind, then send what waits.
    fn ready(&mut self) {
        let now = self.clock;
        self.attempt = None;
        let lost: Vec<u32> = self.inflight.iter().filter(|(_, f)| f.lost).map(|(id, _)| *id).collect();
        for id in lost {
            let Some(f) = self.inflight.get(&id) else { continue };
            if self.is_idempotent(&f.program) && !f.drop_repeated {
                // Declared idempotent: repeated once on this connection, under the same key, so a
                // backend that ran it answers from its store (R9, R10).
                if let Some(f) = self.inflight.get_mut(&id) {
                    f.drop_repeated = true;
                    f.lost = false;
                }
            } else {
                self.fail(id, class::TRANSPORT, code::CLOSED, "the connection to the backend was lost", Some(true));
            }
        }
        self.unblock(now);
        if self.unreachable {
            self.unreachable = false;
            self.out.push(Out::Unreachable(false));
        }
        self.had_link = true;
        self.set_state(LinkState::Ready);
        self.flush_held();
        self.transfers_ready();
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
                    // The server's key and nonce close the exchange (R24): without them the connection cannot be sealed.
                    let peer = cobolt_wire::seal::decode_b64::<{ cobolt_wire::seal::KEY_LEN }>(&s("key")).zip(cobolt_wire::seal::decode_b64::<{ cobolt_wire::seal::NONCE_LEN }>(&s("nonce")));
                    let channel = match (peer, self.keys.as_ref()) {
                        (Some((key, nonce)), Some(keys)) => cobolt_wire::seal::establish(cobolt_wire::seal::Role::Client, PROTOCOL, keys, &self.own_nonce, &key, &nonce).ok(),
                        _ => None,
                    };
                    let Some(channel) = channel else {
                        self.out.push(Out::Warning("the backend's answer carried no usable key; the connection was closed".into()));
                        self.fail_all(class::TRANSPORT, code::CLOSED, "the key exchange failed");
                        self.sock.close();
                        return;
                    };
                    let (tx, rx) = channel.split();
                    self.tx = Some(tx);
                    self.rx = Some(rx);
                    if s("accepted") == "warn" {
                        self.out.push(Out::Warning(format!("backend: {}", s("message"))));
                        self.out.push(Out::BackendVersion { state: "warn".into(), message: s("message"), update_to: s("update_to") });
                    }
                    match self.token.clone() {
                        Some(t) => {
                            self.set_state(LinkState::Authenticating);
                            self.send_msg(&json!({"t": "auth", "token": t}).to_string());
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
                    self.out.push(Out::BackendVersion { state: "refused".into(), message: m.clone(), update_to: s("update_to") });
                    self.fail_all(class::VERSION, c, &m);
                    self.wanted = false;
                    let now = self.clock;
                    self.end_cycle(now);
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
                for w in v.get("warnings").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                    self.out.push(Out::BackendWarning { program: f.program.clone(), message: w.to_owned() });
                }
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
                if !self.inflight.contains_key(&id) && self.on_transfer_error(id, &v) {
                    return;
                }
                let Some(f) = self.inflight.get_mut(&id) else { return };
                // The token expired, or the server calls it invalid, or has none for this connection although
                // the player holds one: refresh and repeat once; the server refused before running anything.
                let refreshable = matches!(code_, code::TOKEN_EXPIRED | code::TOKEN_INVALID | code::TOKEN_MISSING);
                if class_ == class::AUTHENTICATION && refreshable && !f.auth_repeated && self.token.is_some() {
                    f.auth_repeated = true;
                    f.auth_refused = code_;
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
            // Progress, streams and files; a kind a newer backend invented is not an error either (`proto::parse_server`).
            other => {
                self.on_transfer_frame(other, text, &v);
            }
        }
    }

    pub fn on_close(&mut self, now: u64) {
        self.clock = self.clock.max(now);
        self.tx = None;
        self.rx = None;
        match self.state {
            // The browser reports a failed attempt as `error` and then `close`: the second is a repeat.
            LinkState::Closed | LinkState::Retrying { .. } => {}
            LinkState::Ready => {
                // An established connection broke (R25 retries any break): what was sent is decided
                // when an attempt succeeds or the cycle gives up, not now (D12).
                self.transfers_lost();
                for f in self.inflight.values_mut() {
                    if f.sent {
                        f.sent = false;
                        f.lost = true;
                    }
                }
                self.block(now);
                self.start_attempt(1, now);
            }
            LinkState::Connecting => self.attempt_failed(now, Failure::NoConnection),
            LinkState::Greeting | LinkState::Authenticating => self.attempt_failed(now, Failure::Handshake),
        }
    }

    pub fn on_error(&mut self, now: u64) {
        // A failed attempt arrives as a close.
        if matches!(self.state, LinkState::Connecting | LinkState::Greeting | LinkState::Authenticating) {
            self.on_close(now);
        }
    }

    /// The clock moved: time out calls (not while blocked), and move the retry policy on — the next
    /// attempt at the end of a window waited out, an attempt still unfinished at its window's end cut.
    pub fn tick(&mut self, now: u64) {
        self.clock = self.clock.max(now);
        let timeout = self.cfg.call_timeout_ms;
        if !self.blocked() {
            let late: Vec<u32> = self.inflight.iter().filter(|(_, f)| now.saturating_sub(f.sent_at) >= timeout).map(|(id, _)| *id).collect();
            for id in late {
                let (sent, ever) = self.inflight.get(&id).map(|f| (f.sent, f.ever_sent)).unwrap_or((false, false));
                if sent && self.state == LinkState::Ready {
                    // Nobody awaits it any more: the backend may stop the work (R13).
                    self.send_msg(&json!({"t": "cancel", "id": id}).to_string());
                }
                let m = format!("the backend did not answer in time ({timeout} ms)");
                self.fail(id, class::TRANSPORT, code::TIMEOUT, &m, Some(ever));
            }
        }
        let Some(a) = self.attempt else { return };
        if now < a.window_ends {
            return;
        }
        match self.state {
            LinkState::Retrying { .. } => self.start_attempt(a.n + 1, now),
            LinkState::Connecting | LinkState::Greeting | LinkState::Authenticating => {
                // The attempt did not complete in its window: it failed.
                self.sock.close();
                self.attempt_failed(now, Failure::NoAnswer);
            }
            LinkState::Ready | LinkState::Closed => self.attempt = None,
        }
    }

    /// The application came back from the background (WASM-012 R11, R15). A connection that worked and is gone — the cycle
    /// that followed its break gave up, perhaps while the application was away — is tried again at once, behind the
    /// "connecting" modal and under the policy of R25; a cycle that gives up again raises the exception of WASM-016 R21
    /// again. A break the page hears only now (a web view frozen in the background) starts its cycle itself. A connection that
    /// works, or is being tried, is left alone; so is one that never worked, was refused or was signed out of (the
    /// application's next call decides those, O8).
    pub fn resumed(&mut self, now: u64) {
        self.clock = self.clock.max(now);
        if !(self.wanted && self.had_link && self.state == LinkState::Closed && self.url.is_some()) {
            return;
        }
        if self.unreachable {
            self.unreachable = false;
            self.out.push(Out::Unreachable(false));
        }
        self.block(now);
        self.start_attempt(1, now);
    }

    /// The next time `tick` must run, so a timer can be set (None = nothing pending). While blocked
    /// no call's timeout runs, so only the policy's window counts.
    pub fn next_deadline(&self) -> Option<u64> {
        let call = if self.blocked() { None } else { self.inflight.values().map(|f| f.sent_at + self.cfg.call_timeout_ms).min() };
        let window = self.attempt.map(|a| a.window_ends);
        match (call, window) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Cancel a request the module abandoned (its task was cancelled).
    pub fn cancel(&mut self, id: u32) {
        if let Some(f) = self.inflight.remove(&id) {
            if f.sent && self.state == LinkState::Ready {
                self.send_msg(&json!({"t": "cancel", "id": id}).to_string());
            }
        }
    }

    /// The user signed out: close the connection so that the server forgets who it was, and let the next
    /// call open a new one (anonymously, or with another session's token) under a new session id. What was
    /// sent and not yet answered is failed — it may have run — and what was only waiting goes out on the new one.
    pub fn reset_session(&mut self) {
        let sent: Vec<u32> = self.inflight.iter().filter(|(_, f)| f.sent || f.lost).map(|(id, _)| *id).collect();
        for id in sent {
            self.fail(id, class::TRANSPORT, code::CLOSED, "the connection was closed by the sign-out", Some(true));
        }
        for sub in std::mem::take(&mut self.subs).into_keys() {
            self.out.push(Out::StreamEnd { sub, reason: "the connection was closed by the sign-out".into() });
        }
        for id in std::mem::take(&mut self.puts).into_keys().chain(std::mem::take(&mut self.gets).into_keys()) {
            self.out.push(Out::TransferFailed { id, class: class::TRANSPORT.into(), code: code::CLOSED, message: "the connection was closed by the sign-out".into() });
        }
        self.token = None;
        self.wanted = false;
        let now = self.clock;
        self.end_cycle(now);
        self.session = session_from(&mut self.seed);
        self.sock.close();
        self.set_state(LinkState::Closed);
    }

    /// Close for good (unmount).
    pub fn shutdown(&mut self) {
        self.wanted = false;
        self.inflight.clear();
        self.subs.clear();
        self.puts.clear();
        self.gets.clear();
        let now = self.clock;
        self.end_cycle(now);
        self.sock.close();
        self.set_state(LinkState::Closed);
    }
}

/// The contract the program was built against and the programs that may be repeated, from the module's manifest (WASM-014 T8);
/// a manifest that names neither (an older build) leaves the page's configuration in force.
pub fn backend_from_manifest(m: &cobolt_wasm_abi::manifest::Manifest, page_contract: &str, page_idempotent: &[String]) -> (String, Vec<String>) {
    let contract = m.contract.clone().unwrap_or_else(|| page_contract.to_owned());
    let idempotent = if m.remote.is_empty() { page_idempotent.to_vec() } else { m.remote.iter().filter(|r| r.idempotent).map(|r| r.name.clone()).collect() };
    (contract, idempotent)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The server's key pair and nonce in these tests: fixed, so a test can read what the client sealed.
    const SERVER_SEED: [u8; 32] = [5; 32];
    const SERVER_NONCE: [u8; 16] = [6; 16];

    fn test_entropy(buf: &mut [u8]) -> bool {
        buf.fill(0x11);
        true
    }

    #[derive(Default)]
    struct Mock {
        opened: Vec<String>,
        sent: Vec<String>,
        closed: u32,
        /// The server's halves of the channel, built when the client's `hello` goes by: `server_rx` opens what the
        /// client seals (and `sent` receives it as text, as before); `server_tx` seals what a test sends back.
        server_rx: Option<cobolt_wire::seal::Receiver>,
        server_tx: Option<cobolt_wire::seal::Sender>,
        binary_frames: usize,
        plain_after_hello: usize,
    }
    impl Socket for Mock {
        fn open(&mut self, url: &str) {
            self.opened.push(url.to_owned());
        }
        fn send(&mut self, text: &str) {
            if let Ok(v) = serde_json::from_str::<Value>(text) {
                let peer = v["key"].as_str().and_then(cobolt_wire::seal::decode_b64::<32>).zip(v["nonce"].as_str().and_then(cobolt_wire::seal::decode_b64::<16>));
                if let (Some("hello"), Some((key, nonce))) = (v["t"].as_str(), peer) {
                    let keys = cobolt_wire::seal::KeyPair::from_bytes(SERVER_SEED);
                    let (tx, rx) = cobolt_wire::seal::establish(cobolt_wire::seal::Role::Server, v["protocol"].as_u64().unwrap_or(0) as u32, &keys, &SERVER_NONCE, &key, &nonce).unwrap().split();
                    self.server_rx = Some(rx);
                    self.server_tx = Some(tx);
                } else if v["t"] != "hello" {
                    self.plain_after_hello += 1;
                }
            }
            self.sent.push(text.to_owned());
        }
        fn send_binary(&mut self, bytes: &[u8]) {
            self.binary_frames += 1;
            let text = self.server_rx.as_mut().expect("a frame before the hello").open(bytes).expect("the frame opens");
            self.sent.push(text);
        }
        fn close(&mut self) {
            self.closed += 1;
        }
    }

    const URL: &str = "wss://api.example.com/ws";

    /// The server's `hello`, accepted, with its key and nonce.
    fn hello_ok() -> String {
        hello_json("ok", "")
    }

    fn hello_json(accepted: &str, extra: &str) -> String {
        let key = cobolt_wire::seal::encode_b64(&cobolt_wire::seal::KeyPair::from_bytes(SERVER_SEED).public());
        let nonce = cobolt_wire::seal::encode_b64(&SERVER_NONCE);
        format!(r#"{{"t":"hello","protocol":2,"protocol_min":2,"contract":"1.0.0","accepted":"{accepted}","message":"",{extra}"server":"test","max_frame_bytes":1048576,"key":"{key}","nonce":"{nonce}"}}"#)
    }

    fn cfg() -> Config {
        Config { origins: vec![URL.into(), "ws://localhost:8443/ws".into()], entropy: Some(test_entropy), ..Config::default() }.with_backend("1.0.0", vec!["fund-totals".into()])
    }

    fn client() -> Client<Mock> {
        Client::new(cfg(), Mock::default())
    }

    /// Drive the simulated clock to the client's deadlines, refusing every attempt at once (a stopped
    /// backend on loopback), until nothing is pending. Answers the times each attempt was opened.
    fn refuse_every_attempt(c: &mut Client<Mock>, until: u64) -> Vec<u64> {
        let mut opened_at = Vec::new();
        let mut seen = c.sock.opened.len();
        while let Some(d) = c.next_deadline().filter(|d| *d <= until) {
            c.tick(d);
            if c.sock.opened.len() > seen {
                seen = c.sock.opened.len();
                opened_at.push(d);
                c.on_error(d);
                c.on_close(d);
            }
        }
        opened_at
    }

    fn flags(out: &[Out]) -> Vec<String> {
        out.iter()
            .filter_map(|o| match o {
                Out::Blocked(b) => Some(format!("blocked={b}")),
                Out::Unreachable(u) => Some(format!("unreachable={u}")),
                _ => None,
            })
            .collect()
    }

    /// A client past `hello` with no token: ready.
    fn ready_client() -> Client<Mock> {
        let mut c = client();
        c.connect(URL, 0).unwrap();
        c.on_open(1);
        c.on_message(&hello_ok());
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

    /// 2.0.413: the loading effect shows while a call the user (or the initial load) started is in flight; a call a Timer's tick
    /// made is an automatic refresh and shows nothing. The set of automatic ids forgets the calls that ended.
    #[test]
    fn the_loading_effect_ignores_calls_a_timers_tick_made() {
        use std::collections::HashSet;
        let mut automatic: HashSet<u32> = [7].into_iter().collect();
        assert!(!user_call_in_flight(&[], &mut automatic), "nothing in flight: not busy");
        assert!(automatic.is_empty(), "the ended call is forgotten");
        let mut automatic: HashSet<u32> = [7].into_iter().collect();
        assert!(!user_call_in_flight(&[7], &mut automatic), "only the Timer's call is out: the effect stays away");
        assert!(user_call_in_flight(&[7, 8], &mut automatic), "the user's call 8 is out beside it: the effect shows");
        assert!(user_call_in_flight(&[8], &mut automatic), "the Timer's call ended, the user's goes on");
        assert!(automatic.is_empty(), "7 is forgotten once it is no longer in flight");
        assert!(user_call_in_flight(&[9], &mut automatic), "a call nobody marked is the user's (the initial load's, a grid's)");
        println!("busy: timer-only -> no, timer + user -> yes, user only -> yes, nothing -> no; ended calls forgotten");
    }

    /// 2.0.415: the effect starts only when a user's call has been out for 2 s; an answer before that means it never showed, and
    /// the next call waits its own 2 s.
    #[test]
    fn the_loading_effect_waits_two_seconds_for_the_answer() {
        let mut g = EffectGate::default();
        let t0 = 10_000;
        assert!(!g.show(false, t0), "nothing in flight: no effect");
        assert_eq!(g.due_in(t0), None, "nothing is waiting");
        // A call goes out at t0 and is answered 1.9 s later: the effect never shows, and nothing is left to dismiss.
        assert!(!g.show(true, t0), "the call just went out: not yet");
        assert_eq!(g.due_in(t0), Some(2000), "the effect is due in 2 s");
        assert!(!g.show(true, t0 + 1000), "1 s: not yet");
        assert_eq!(g.due_in(t0 + 1500), Some(500));
        assert!(!g.show(true, t0 + 1999), "1.999 s: not yet");
        assert!(!g.show(false, t0 + 1900), "answered at 1.9 s: still no effect");
        assert_eq!(g.due_in(t0 + 1900), None, "the wait is dropped with the answer");
        // The next call waits its own 2 s, counted from when it went out, not from the first.
        assert!(!g.show(true, t0 + 2500), "a new call at 2.5 s: its own 2 s begin");
        assert_eq!(g.due_in(t0 + 2500), Some(2000));
        assert!(!g.show(true, t0 + 4499), "1.999 s after the new call: not yet");
        assert!(g.show(true, t0 + 4500), "2 s after it went out and still unanswered: the effect starts");
        assert_eq!(g.due_in(t0 + 4500), None, "it is already due");
        assert!(g.show(true, t0 + 9000), "it stays while the call is out");
        assert!(!g.show(false, t0 + 9100), "the answer dismisses it completely");
        assert!(!g.show(true, t0 + 9200), "and a following call starts the wait again");
        println!("loading effect gate: 2000 ms; answered at 1.9 s -> never shown; unanswered at 2.0 s -> shown; answer -> dismissed; next call -> new 2 s");
    }

    /// The client lists the ids it has in flight, in order.
    #[test]
    fn the_client_lists_the_requests_in_flight() {
        let mut c = Client::new(Config::from_page(cfg().origins, false, None, None), Mock::default());
        c.set_token(None);
        c.connect(URL, 0).unwrap();
        assert!(c.in_flight_ids().is_empty());
        c.request_as(5, br#"{"program":"P","args":[]}"#, 0);
        c.request_as(3, br#"{"program":"P","args":[]}"#, 0);
        assert_eq!(c.in_flight_ids(), vec![3, 5]);
        assert_eq!(c.in_flight(), 2);
        println!("in flight: {:?}", c.in_flight_ids());
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

    /// WASM-014 T7: the conversation recorded from a real backend (`vectors/wire.v1.json`), its backend half replayed into the
    /// client: what the client sends has the transcript's shape, and every frame the backend sent reaches the module as the
    /// answer the transcript's scenario names.
    #[test]
    fn the_recorded_conversation_is_understood_by_the_client() {
        let doc: Value = serde_json::from_str(include_str!("../../../specs/WASM-014-backend-contract-and-evolution/vectors/wire.v1.json")).unwrap();
        let frames = doc["frames"].as_array().unwrap();
        let of = |scenario: &str, dir: &str| frames.iter().find(|f| f["scenario"] == scenario && f["dir"] == dir).map(|f| f["frame"].clone()).unwrap_or_else(|| panic!("{scenario} {dir}"));
        let keys = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<std::collections::BTreeSet<_>>();
        let mut checked = 0;

        let mut c = client();
        c.set_token(Some("tok-123".into()));
        c.connect(URL, 0).unwrap();
        let scenarios = ["call-ok", "error-validation", "error-not-found", "error-authorisation", "error-application"];
        let ids: Vec<u32> = scenarios.iter().map(|_| c.request(br#"{"program":"FUND-TOTALS","args":[],"returning":true}"#, 1)).collect();
        c.on_open(2);
        let hello = sent_json(&c).remove(0);
        assert_eq!(keys(&hello), keys(&of("session", "c2s")), "the hello has the members the transcript's has");
        assert_eq!(hello["session"].as_str().map(str::len), Some(32), "a 128-bit session id");
        checked += 1;
        // The backend's hello, with this test's key and nonce in place of the transcript's stand-ins.
        let real: Value = serde_json::from_str(&hello_ok()).unwrap();
        let mut server_hello = of("session", "s2c");
        server_hello["key"] = real["key"].clone();
        server_hello["nonce"] = real["nonce"].clone();
        c.on_message(&server_hello.to_string());
        checked += 1;
        assert!(sent_json(&c).iter().any(|m| m["t"] == "auth" && keys(m) == keys(&of("auth", "c2s"))), "the auth has the transcript's members");
        checked += 1;
        c.on_message(&of("auth", "s2c").to_string());
        checked += 1;
        assert_eq!(c.state(), LinkState::Ready);
        for (scenario, id) in scenarios.iter().zip(&ids) {
            let mut f = of(scenario, "s2c");
            f["id"] = json!(id);
            c.on_message(&f.to_string());
            checked += 1;
        }
        let got = answers(&mut c);
        assert_eq!(got.len(), scenarios.len(), "one answer for each call: {got:?}");
        for (scenario, (_, answer)) in scenarios.iter().zip(&got) {
            let want = of(scenario, "s2c");
            let text = answer.to_string();
            if want["t"] == "error" {
                assert!(answer["error"]["code"] == want["code"] && answer["error"]["class"] == want["class"], "{scenario}: {text}");
            } else {
                assert!(answer.get("error").is_none(), "{scenario}: {text}");
            }
        }
        // The refused and the warn hello: the client reads both.
        for scenario in ["hello-refused", "hello-warn"] {
            let mut c2 = client();
            c2.connect(URL, 0).unwrap();
            c2.on_open(1);
            let mut h = of(scenario, "s2c");
            h["key"] = real["key"].clone();
            h["nonce"] = real["nonce"].clone();
            c2.on_message(&h.to_string());
            checked += 1;
            println!("{scenario}: the client's link is {:?}", c2.state());
        }
        println!("transcript replayed into the player: {checked} frames checked");
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
        assert_eq!((s[0]["t"].as_str(), s[0]["protocol"].as_u64(), s[0]["contract"].as_str()), (Some("hello"), Some(2), Some("1.0.0")));
        assert!(s[0]["client"].as_str().unwrap().starts_with("prc-wasm/"));
        assert_eq!(s[0]["session"].as_str(), Some(c.session()));
        c.on_message(&hello_ok());
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
        c.on_message(&hello_ok());
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
        // the backend cannot be reached: the first attempt fails, the policy's other two too (R25)
        let mut c = client();
        c.connect(URL, 0).unwrap();
        c.request(br#"{"program":"X"}"#, 0);
        c.on_error(5);
        c.on_close(6); // the browser's second event is ignored
        assert!(answers(&mut c).is_empty(), "the call waits for the retry cycle");
        refuse_every_attempt(&mut c, u64::MAX);
        row("backend unreachable", &answers(&mut c)[0].1);
        assert_eq!((c.state(), c.next_deadline(), c.sock.opened.len()), (LinkState::Closed, None, 3), "given up: nothing waits, no fourth attempt");
        c.request(br#"{"program":"X"}"#, 70_000);
        assert_eq!((c.state(), c.sock.opened.len()), (LinkState::Connecting, 4), "the next call starts a new cycle (O8)");
        // the connection drops with a call in flight; the next connection works
        let mut c = ready_client();
        c.request(br#"{"program":"X"}"#, 0);
        c.on_close(5);
        assert!(answers(&mut c).is_empty(), "decided when an attempt succeeds, not at the break");
        c.on_open(6);
        c.on_message(&hello_ok());
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
            assert_eq!((c.state(), c.next_deadline(), c.blocked()), (LinkState::Closed, None, false));
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
        c.on_message(&hello_ok());
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

    /// 2001 (invalid) and 2000 (missing, while the player holds a token) are handled as 2002 is: refresh, repeat once under the
    /// same key, and a second refusal is final. With no token to refresh, 2000 and 2001 go to the program as they came.
    #[test]
    fn an_invalid_or_missing_token_is_refreshed_and_the_call_repeated_once_like_an_expired_one() {
        let mut rows = Vec::new();
        for (case, wire_code) in [("2001 invalid", code::TOKEN_INVALID), ("2000 missing", code::TOKEN_MISSING), ("2002 expired", code::TOKEN_EXPIRED)] {
            let mut c = client();
            c.set_token(Some("old".into()));
            c.connect(URL, 0).unwrap();
            c.on_open(1);
            c.on_message(&hello_ok());
            c.on_message(r#"{"t":"auth-ok","sub":"u","expires_in":1}"#);
            let id = c.request(br#"{"program":"X"}"#, 2);
            let refused = format!(r#"{{"t":"error","id":{id},"class":"authentication","code":{wire_code},"message":"refused"}}"#);
            c.on_message(&refused);
            assert!(c.refresh_requested, "{case}: the owner is asked to refresh");
            assert!(answers(&mut c).is_empty(), "{case}: the program is not told yet");
            c.set_token(Some("new".into()));
            let sent = sent_json(&c);
            assert!(sent.iter().any(|m| m["t"] == "auth" && m["token"] == "new"), "{case}: the new token is sent");
            let calls: Vec<&Value> = sent.iter().filter(|m| m["t"] == "call" && m["id"] == id).collect();
            assert_eq!(calls.len(), 2, "{case}: the call was repeated");
            assert_eq!(calls[0]["key"], calls[1]["key"], "{case}: under the same key");
            c.on_message(&refused);
            let a = answers(&mut c);
            assert_eq!((a.len(), a[0].1["error"]["code"].as_u64()), (1, Some(u64::from(wire_code))), "{case}: a second refusal is final");
            rows.push(format!("{case}: refreshed, repeated once under key {}, the second refusal reached the program", calls[0]["key"]));
        }
        // No token held: nothing to refresh, the refusal is the answer.
        for (case, wire_code) in [("2001 invalid, no token", code::TOKEN_INVALID), ("2000 missing, no token", code::TOKEN_MISSING)] {
            let mut c = ready_client();
            let id = c.request(br#"{"program":"X"}"#, 2);
            c.on_message(&format!(r#"{{"t":"error","id":{id},"class":"authentication","code":{wire_code},"message":"refused"}}"#));
            assert!(!c.refresh_requested, "{case}: no refresh is asked for");
            let a = answers(&mut c);
            assert_eq!((a.len(), a[0].1["error"]["code"].as_u64()), (1, Some(u64::from(wire_code))), "{case}");
            rows.push(format!("{case}: handed to the program, no refresh"));
        }
        for r in &rows {
            println!("{r}");
        }
        assert_eq!(rows.len(), 5);
    }

    /// When the refresh itself fails (the owner has no token to give), the program is told what the server said:
    /// a wrong token still reads as a wrong token, not as an expiry.
    #[test]
    fn a_failed_refresh_tells_the_program_the_servers_own_code() {
        let mut rows = Vec::new();
        for (case, wire_code) in [("2001 invalid", code::TOKEN_INVALID), ("2000 missing", code::TOKEN_MISSING), ("2002 expired", code::TOKEN_EXPIRED)] {
            let mut c = client();
            c.set_token(Some("old".into()));
            c.connect(URL, 0).unwrap();
            c.on_open(1);
            c.on_message(&hello_ok());
            c.on_message(r#"{"t":"auth-ok","sub":"u","expires_in":1}"#);
            let id = c.request(br#"{"program":"X"}"#, 2);
            c.on_message(&format!(r#"{{"t":"error","id":{id},"class":"authentication","code":{wire_code},"message":"refused"}}"#));
            assert!(c.refresh_requested, "{case}");
            c.set_token(None);
            let a = answers(&mut c);
            let e = &a[0].1["error"];
            assert_eq!((a.len(), e["class"].as_str(), e["code"].as_u64()), (1, Some("authentication"), Some(u64::from(wire_code))), "{case}");
            assert_eq!(e["may_have_run"], false, "{case}: the server refused before running anything");
            rows.push(format!("{case}: refresh failed, the program got {} \"{}\"", e["code"], e["message"].as_str().unwrap_or("")));
        }
        for r in &rows {
            println!("{r}");
        }
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn a_safe_program_is_repeated_once_after_a_drop_and_an_unsafe_one_fails_with_may_have_run() {
        let mut c = ready_client();
        let safe = c.request(br#"{"program":"FUND-TOTALS","args":[],"returning":true}"#, 1);
        let unsafe_ = c.request(br#"{"program":"FUND-SAVE","args":[],"returning":true}"#, 1);
        let first_key = sent_json(&c)[0]["key"].clone();
        c.on_close(5);
        assert!(answers(&mut c).is_empty(), "both wait for the next connection");
        c.tick(1_000);
        c.on_open(1_001);
        c.on_message(&hello_ok());
        let a = answers(&mut c);
        assert_eq!(a.len(), 1, "only the unsafe call is failed: {a:?}");
        assert_eq!((a[0].0, a[0].1["error"]["code"].as_u64(), a[0].1["error"]["may_have_run"].as_bool()), (unsafe_, Some(1001), Some(true)));
        assert_eq!(c.in_flight(), 1, "the safe one is repeated");
        let again: Vec<Value> = sent_json(&c).into_iter().filter(|m| m["t"] == "call" && m["id"] == safe).collect();
        assert_eq!(again.len(), 2, "sent, then repeated once");
        assert_eq!(again[1]["key"], first_key, "the repeat carries the same key, so the backend answers from its store");
        // A second drop is final: it fails when the connection after it works.
        c.on_close(1_100);
        c.on_open(1_101);
        c.on_message(&hello_ok());
        let a = answers(&mut c);
        assert_eq!((a[0].0, a[0].1["error"]["code"].as_u64(), a[0].1["error"]["may_have_run"].as_bool()), (safe, Some(1001), Some(true)));
        println!("safe FUND-TOTALS: repeated once under key {first_key}, a second drop fails it; unsafe FUND-SAVE: failed 1001 may_have_run=true");
    }

    #[test]
    fn the_players_own_ids_start_at_0x80000000_and_only_module_answers_are_size_limited() {
        assert_eq!(PLAYER_ID_BASE, crate::auth::https::ID_BASE);
        assert!(crate::grid::ID_BASE > PLAYER_ID_BASE);
        let mut c = ready_client();
        let big = format!(r#"{{"ROWS":"{}"}}"#, "z".repeat(70_000));
        c.request_as(PLAYER_ID_BASE + 7, br#"{"ROWS":""}"#, 0);
        c.on_message(&reply(PLAYER_ID_BASE + 7, &big));
        let (id, v) = answers(&mut c).remove(0);
        assert_eq!(id, PLAYER_ID_BASE + 7);
        assert!(v.get("result").is_some(), "the player's own answer is not cut at 64 KiB");
    }


    /// WASM-003 T15b-3: after the two `hello`s nothing leaves in plain text. The `hello` is the only text message;
    /// the call goes as a sealed binary frame that only the server's key opens, and the sealed frame carries
    /// neither the program name nor the payload.
    #[test]
    fn only_the_hello_leaves_as_text_and_every_other_message_is_sealed() {
        let mut c = ready_client();
        c.request(br#"{"program":"SECRET-PROGRAM","args":[{"mode":"value","json":{"N":"payload-value"}}],"returning":true}"#, 5);
        assert_eq!(c.sock.binary_frames, 1, "the call is one sealed frame");
        assert_eq!(c.sock.plain_after_hello, 0, "no plain text after the hello");
        let hello: Value = serde_json::from_str(&c.sock.sent[0]).unwrap_or(Value::Null);
        assert_ne!(hello["t"], "hello", "ready_client cleared the hello; the call is what remains");
        println!("sealed channel in the player: 1 hello as text, {} sealed frame(s), {} plain message(s) after it", c.sock.binary_frames, c.sock.plain_after_hello);
    }

    #[test]
    fn a_sealed_reply_is_opened_and_a_text_or_tampered_frame_after_the_handshake_closes_the_connection() {
        let mut c = ready_client();
        let id = c.request(br#"{"program":"P","args":[],"returning":true}"#, 5);
        let mut server_tx = c.sock.server_tx.take().unwrap();
        let sealed = server_tx.seal(&reply(id, r#"{"V":"ok"}"#)).unwrap();
        c.on_binary(&sealed);
        assert!(c.take_out().iter().any(|o| matches!(o, Out::Answer { id: i, .. } if *i == id)), "the sealed reply reached the module");
        // A text message now is not from a backend that kept to the protocol.
        let closed = c.sock.closed;
        c.on_text(&reply(id, r#"{"V":"plain"}"#));
        assert_eq!(c.sock.closed, closed + 1, "text after the exchange closes the connection");
        // A frame that was tampered with does not open.
        let mut bad = server_tx.seal(&reply(id, r#"{"V":"x"}"#)).unwrap();
        bad[0] ^= 1;
        c.on_binary(&bad);
        assert_eq!(c.sock.closed, closed + 2, "a tampered frame closes the connection");
    }

    #[test]
    fn without_a_random_source_there_is_no_connection_and_no_plaintext_mode() {
        let mut cfg = cfg();
        cfg.entropy = None;
        let mut c = Client::new(cfg, Mock::default());
        c.connect(URL, 0).unwrap();
        c.on_open(1);
        assert!(c.sock.sent.is_empty(), "nothing was sent, not even a hello");
        assert_eq!(c.state(), LinkState::Closed);
        assert!(c.take_out().iter().any(|o| matches!(o, Out::Warning(w) if w.contains("random source"))));
        let mut failing = cfg_with_failing_entropy();
        failing.connect(URL, 0).unwrap();
        failing.on_open(1);
        assert!(failing.sock.sent.is_empty() && failing.state() == LinkState::Closed, "a random source that fails is the same");
    }

    fn cfg_with_failing_entropy() -> Client<Mock> {
        let mut cfg = cfg();
        cfg.entropy = Some(|_| false);
        Client::new(cfg, Mock::default())
    }

    #[test]
    fn a_hello_answer_without_a_usable_key_fails_the_connection_as_transport() {
        let mut c = client();
        c.connect(URL, 0).unwrap();
        c.on_open(1);
        let id = c.request(br#"{"program":"P","args":[],"returning":true}"#, 2);
        c.on_message(r#"{"t":"hello","protocol":2,"protocol_min":2,"contract":"1.0.0","accepted":"ok","message":"","server":"s","max_frame_bytes":1}"#);
        let out = c.take_out();
        assert!(out.iter().any(|o| matches!(o, Out::Answer { id: i, frame } if *i == id && String::from_utf8_lossy(frame).contains("\"class\":\"transport\""))), "{out:?}");
        assert!(c.sock.closed >= 1);
        assert_ne!(c.state(), LinkState::Ready);
    }

    /// The key source is `crypto.getRandomValues`, and `Math.random` is never a fallback for a key or a nonce.
    #[test]
    fn the_browser_key_source_is_get_random_values_and_never_math_random() {
        let api = include_str!("api.rs");
        let start = api.find("fn browser_entropy").expect("browser_entropy exists");
        let body = &api[start..start + 700];
        assert!(body.contains("get_random_values_with_js_u8_array"), "{body}");
        assert!(!body.contains("Math::random") && !body.contains("Math.random"), "{body}");
        assert!(api.contains("ncfg.entropy = Some(browser_entropy)"), "the connection is given that source");
    }
    #[test]
    fn a_warning_hello_connects_and_json_pings_are_not_part_of_the_protocol() {
        let mut c = client();
        c.connect(URL, 0).unwrap();
        c.on_open(1);
        c.on_message(&hello_json("warn", r#""update_to":"1.2.0","#).replace(r#""message":"""#, r#""message":"update when convenient""#).replace("\"contract\":\"1.0.0\"", "\"contract\":\"1.2.0\""));
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
        c.on_error(4); // the first attempt is refused at once
        assert_eq!(c.state(), LinkState::Retrying { attempt: 1, window_ends: 10_003 });
        // While retrying, a new call waits; after the reconnection the lost one is failed and the new
        // one is sent and answered normally.
        let later = c.request(br#"{"program":"Y"}"#, 100);
        assert!(c.sock.sent.iter().all(|s| !s.contains("\"Y\"")));
        c.tick(10_003);
        assert_eq!(c.state(), LinkState::Connecting);
        c.on_open(10_004);
        c.on_message(&hello_ok());
        let a = answers(&mut c);
        assert_eq!(a[0].0, in_flight);
        assert_eq!(a[0].1["error"]["class"], "transport");
        assert_eq!(a[0].1["error"]["may_have_run"], true, "WASM-014 R9: the program is told it may have run");
        assert!(c.sock.sent.iter().any(|s| s.contains("\"Y\"")));
        c.on_message(&reply(later, "\"ok\""));
        let a = answers(&mut c);
        assert!(a.iter().any(|(id, v)| *id == later && v.get("error").is_none()));
    }

    /// WASM-003 R25/R26, AC12 on the simulated clock: a break blocks the application; the attempts
    /// open at 0, 10 and 30 s after it (windows of 10, 20 and 30 s), each refused at once and waited
    /// out; the 30 s call timeout does not fire during the 60 s cycle; at the give-up every waiting
    /// call fails `transport` — 1001 `may_have_run` if it had been sent, 1000 if not — the block lifts
    /// with *unreachable* on, and no fourth attempt follows until the application calls again.
    #[test]
    fn a_break_is_retried_at_10_20_30_seconds_then_given_up() {
        let mut c = ready_client();
        assert_eq!(c.cfg.call_timeout_ms, 30_000);
        let safe = c.request(br#"{"program":"FUND-TOTALS"}"#, 0);
        let sent = c.request(br#"{"program":"FUND-SAVE"}"#, 0);
        let t0 = 1_000;
        c.on_close(t0);
        assert!(c.blocked());
        c.on_error(t0); // attempt 1, opened at the break, refused at once
        assert_eq!(c.state(), LinkState::Retrying { attempt: 1, window_ends: t0 + 10_000 });
        let unsent = c.request(br#"{"program":"FUND-LIST"}"#, t0 + 5_000);
        let mut opened = vec![t0];
        opened.extend(refuse_every_attempt(&mut c, u64::MAX));
        let out = c.take_out();
        let transitions = flags(&out);
        let mut got: Vec<(u32, u64, Option<bool>, String)> = out
            .iter()
            .filter_map(|o| if let Out::Answer { id, frame } = o { Some((*id, serde_json::from_slice::<Value>(frame).unwrap())) } else { None })
            .map(|(id, v)| (id, v["error"]["code"].as_u64().unwrap(), v["error"]["may_have_run"].as_bool(), v["error"]["message"].as_str().unwrap().to_owned()))
            .collect();
        got.sort();
        let rel: Vec<u64> = opened.iter().map(|t| t - t0).collect();
        println!("attempts opened at (ms after the break): {rel:?}; transitions {transitions:?}; answers {got:?}");
        assert_eq!(rel, [0, 10_000, 30_000], "windows of 10, 20, 30 s");
        assert_eq!(transitions, ["blocked=true", "blocked=false", "unreachable=true"]);
        let m = "the backend could not be reached".to_owned();
        assert_eq!(got, [(safe, 1001, Some(true), m.clone()), (sent, 1001, Some(true), m.clone()), (unsent, 1000, Some(false), m)], "no 1002 during the cycle");
        assert_eq!((c.state(), c.blocked(), c.unreachable(), c.next_deadline()), (LinkState::Closed, false, true, None));
        c.tick(t0 + 600_000);
        assert_eq!(c.sock.opened.len(), 1 + 3, "no fourth attempt on its own (1 = the first connection)");
        // The next call starts a new cycle (O8); its success clears *unreachable*.
        let next = c.request(br#"{"program":"FUND-LIST"}"#, t0 + 700_000);
        assert_eq!((c.state(), c.sock.opened.len()), (LinkState::Connecting, 5));
        c.on_open(t0 + 700_001);
        c.on_message(&hello_ok());
        assert_eq!(flags(&c.take_out()), ["unreachable=false"]);
        assert!(sent_json(&c).iter().any(|m| m["t"] == "call" && m["id"] == next));
    }

    /// The backend comes back in attempt 2: the block lifts, the idempotent call is repeated once under
    /// its key, the other sent call fails 1001 `may_have_run`, the call made while blocked goes out, and
    /// the time spent blocked does not count against any call's timeout (D12).
    #[test]
    fn success_in_attempt_2_repeats_the_safe_call_and_fails_the_other() {
        let mut c = ready_client();
        let safe = c.request(br#"{"program":"FUND-TOTALS"}"#, 0);
        let unsafe_ = c.request(br#"{"program":"FUND-SAVE"}"#, 0);
        let first_key = sent_json(&c)[0]["key"].clone();
        c.on_close(1_000);
        c.on_error(1_000);
        let unsent = c.request(br#"{"program":"FUND-LIST"}"#, 5_000);
        assert_eq!(c.next_deadline(), Some(11_000), "only the window counts while blocked");
        c.tick(11_000);
        assert_eq!(c.state(), LinkState::Connecting, "attempt 2");
        c.on_open(11_500);
        c.on_message(&hello_ok());
        let out = c.take_out();
        let transitions = flags(&out);
        let failed: Vec<(u32, Value)> = out.iter().filter_map(|o| if let Out::Answer { id, frame } = o { Some((*id, serde_json::from_slice(frame).unwrap())) } else { None }).collect();
        let calls: Vec<Value> = sent_json(&c).into_iter().filter(|m| m["t"] == "call").collect();
        println!("transitions {transitions:?}; failed {failed:?}; on the new connection {:?}", calls.iter().map(|m| (m["id"].as_u64(), m["program"].as_str())).collect::<Vec<_>>());
        assert_eq!(transitions, ["blocked=true", "blocked=false"], "no unreachable: the cycle did not give up");
        assert_eq!(failed.len(), 1);
        assert_eq!((failed[0].0, failed[0].1["error"]["code"].as_u64(), failed[0].1["error"]["may_have_run"].as_bool()), (unsafe_, Some(1001), Some(true)));
        let repeat: Vec<&Value> = calls.iter().filter(|m| m["id"] == safe).collect();
        assert_eq!((repeat.len(), &repeat[1]["key"]), (2, &first_key), "sent, then repeated once under the same key");
        assert!(calls.iter().any(|m| m["id"] == unsent), "the call made while blocked goes out");
        // 1 s had run for the safe call before the break and none for the call made while blocked.
        assert_eq!(c.next_deadline(), Some(11_500 - 1_000 + 30_000));
        c.tick(11_500 + 28_999);
        assert!(answers(&mut c).is_empty());
        c.tick(11_500 + 29_000);
        assert_eq!(answers(&mut c).iter().map(|(id, v)| (*id, v["error"]["code"].as_u64())).collect::<Vec<_>>(), [(safe, Some(1002))]);
        c.tick(11_500 + 30_000);
        assert_eq!(answers(&mut c).iter().map(|(id, _)| *id).collect::<Vec<_>>(), [unsent]);
    }

    /// WASM-012 R11, R15: back from the background, a connection that worked and whose cycle gave up while the application
    /// was away is tried again at once behind the modal (blocked), its give-up raises *unreachable* again (the exception
    /// of WASM-016 R21), and a success lifts the modal; a connection that works, is being tried, never worked or was
    /// signed out of is left alone.
    #[test]
    fn a_return_from_the_background_retries_a_lost_connection_behind_the_modal_012() {
        // Away: the connection breaks and its cycle gives up.
        let mut c = ready_client();
        c.on_close(1_000);
        let first = refuse_every_attempt(&mut c, u64::MAX);
        assert_eq!(flags(&c.take_out()), ["blocked=true", "blocked=false", "unreachable=true"]);
        assert_eq!((c.state(), c.unreachable()), (LinkState::Closed, true));
        let opened = c.sock.opened.len();
        // Back: a new cycle at once, behind the modal; it gives up again and says so again.
        let back = 600_000;
        c.resumed(back);
        assert_eq!((c.state(), c.blocked(), c.sock.opened.len()), (LinkState::Connecting, true, opened + 1), "attempt 1 at the return");
        assert_eq!(flags(&c.take_out()), ["unreachable=false", "blocked=true"]);
        c.on_error(back);
        c.on_close(back);
        let again = refuse_every_attempt(&mut c, u64::MAX);
        let rel: Vec<u64> = again.iter().map(|t| t - back).collect();
        assert_eq!(rel, [10_000, 30_000], "the policy's windows of R25");
        assert_eq!(flags(&c.take_out()), ["blocked=false", "unreachable=true"], "the give-up is raised again");
        // Back once more, and the backend answers: the modal goes, nothing is unreachable.
        c.resumed(900_000);
        c.on_open(900_010);
        c.on_message(&hello_ok());
        assert_eq!(flags(&c.take_out()), ["unreachable=false", "blocked=true", "blocked=false"]);
        assert_eq!(c.state(), LinkState::Ready);
        // Left alone: a working connection, one being tried, one that never worked, one signed out of.
        let opened = c.sock.opened.len();
        c.resumed(900_100);
        assert_eq!((c.state(), c.sock.opened.len(), c.take_out().len()), (LinkState::Ready, opened, 0), "working");
        c.on_close(900_200);
        let trying = c.sock.opened.len();
        c.resumed(900_300);
        assert_eq!(c.sock.opened.len(), trying, "a cycle running is not started twice");
        let mut never = client();
        never.connect(URL, 0).unwrap();
        never.on_error(10);
        let before = never.sock.opened.len();
        never.resumed(20);
        assert_eq!((never.state(), never.blocked(), never.sock.opened.len()), (LinkState::Closed, false, before), "never worked");
        let mut out = ready_client();
        out.reset_session();
        out.resumed(5);
        assert_eq!((out.state(), out.blocked()), (LinkState::Closed, false), "signed out");
        println!("away: attempts at {first:?}; back at {back}: attempts at +0, {rel:?}, raised again; back and answered: ready; left alone: working, trying, never worked, signed out");
    }

    /// O7: a first connection that a call needs and that fails runs the cycle; one nobody waits for
    /// stops at its failure (no retry loop for a page that makes no call).
    #[test]
    fn a_failed_first_connection_runs_the_cycle_only_when_a_call_waits() {
        let mut c = client();
        c.connect(URL, 0).unwrap();
        c.on_error(10);
        assert_eq!((c.state(), c.blocked(), c.next_deadline()), (LinkState::Closed, false, None), "nobody waits: no cycle");
        let mut c = client();
        c.connect(URL, 0).unwrap();
        c.request(br#"{"program":"X"}"#, 1);
        c.on_error(10);
        assert_eq!((c.state(), c.blocked()), (LinkState::Retrying { attempt: 1, window_ends: 10_000 }, true));
        let opened = refuse_every_attempt(&mut c, u64::MAX);
        println!("first connection at 0, then attempts at {opened:?}");
        assert_eq!(opened, [10_000, 30_000]);
        assert_eq!(answers(&mut c)[0].1["error"]["code"].as_u64(), Some(1000));
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
        // A call held behind a handshake that never completes: the attempt is cut at the end of its
        // window and the policy goes on (R25); the call ends with the cycle, never sent: 1000.
        let mut c = client();
        c.connect(URL, 0).unwrap();
        c.on_open(1);
        c.request(br#"{"program":"X"}"#, 2);
        assert_eq!(c.next_deadline(), Some(10_000), "the attempt's window ends before the call's timeout");
        c.tick(10_000);
        assert_eq!((c.sock.closed, c.sock.opened.len(), c.blocked()), (1, 2, true), "cut, blocked, attempt 2 opened");
        refuse_every_attempt(&mut c, u64::MAX);
        let a = answers(&mut c);
        assert_eq!((a[0].1["error"]["code"].as_u64(), a[0].1["error"]["may_have_run"].as_bool()), (Some(1000), Some(false)));
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
        c.on_message(&hello_ok());
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
        c.on_message(&hello_ok());
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
        c.on_message(&hello_ok());
        c.on_close(2);
        c.tick(10_000);
        c.on_open(10_001);
        let hellos: Vec<String> = sent_json(&c).iter().filter(|m| m["t"] == "hello").map(|m| m["session"].as_str().unwrap().to_owned()).collect();
        assert_eq!(hellos.len(), 2);
        assert_eq!(hellos[0], hellos[1], "the same session on a reconnect");
        assert_eq!(hellos[0].len(), 32);
        // And the connection's code never touches browser storage. The one value the player keeps
        // there is the hosted flow's random state (WASM-009 D5), in `auth_fetch.rs`, in this tab's
        // `sessionStorage` and under its one key: checked below.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let fetch = std::fs::read_to_string(root.join("auth_fetch.rs")).unwrap();
        for line in fetch.lines().filter(|l| ["local_storage", "session_storage", "indexed_db", "_item("].iter().any(|w| l.contains(w))) {
            assert!(!line.contains("local_storage") && !line.contains("indexed_db"), "auth_fetch.rs: {line}");
            assert!(!line.contains("_item(") || line.contains("HOSTED_STATE_KEY"), "auth_fetch.rs keeps only the hosted state: {line}");
        }
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
            ("TOKEN_MISSING", code::TOKEN_MISSING),
            ("TOKEN_INVALID", code::TOKEN_INVALID),
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

    /// WASM-014 T9: a `warn` hello and a warning on a reply are reported for the application's main form.
    #[test]
    fn a_warn_hello_and_a_reply_warning_are_reported_to_the_application() {
        let mut c = client();
        c.connect(URL, 0).unwrap();
        let id = c.request(br#"{"program":"FUND-TOTALS","args":[],"returning":true}"#, 1);
        c.on_open(2);
        c.on_message(&hello_json("warn", r#""message":"update the application","update_to":"1.2.0","#));
        c.on_message(&format!(r#"{{"t":"reply","id":{id},"args":[],"returning":1,"warnings":["FUND-TOTALS is deprecated: use FUND-TOTALS-V2"]}}"#));
        let out = c.take_out();
        let reported: Vec<&Out> = out.iter().filter(|o| matches!(o, Out::BackendVersion { .. } | Out::BackendWarning { .. })).collect();
        println!("reported for the main form: {reported:?}");
        assert_eq!(reported[0], &Out::BackendVersion { state: "warn".into(), message: "update the application".into(), update_to: "1.2.0".into() });
        assert_eq!(reported[1], &Out::BackendWarning { program: "FUND-TOTALS".into(), message: "FUND-TOTALS is deprecated: use FUND-TOTALS-V2".into() });
        assert_eq!(reported.len(), 2);
    }

    /// WASM-014 T9: a refused hello is reported too, with the contract to update to.
    #[test]
    fn a_refused_hello_is_reported_with_the_update_to_contract() {
        let mut c = client();
        c.connect(URL, 0).unwrap();
        c.on_open(1);
        c.on_message(&hello_json("refused", r#""message":"this application is too old","update_to":"2.0.0","#));
        let out = c.take_out();
        assert!(out.contains(&Out::BackendVersion { state: "refused".into(), message: "this application is too old".into(), update_to: "2.0.0".into() }), "{out:?}");
    }

    #[test]
    fn the_module_names_the_contract_and_the_repeatable_programs() {
        use cobolt_wasm_abi::fixture::{build, standard_spec};
        use cobolt_wasm_abi::manifest::RemoteSig;
        let mut m = build(&standard_spec(false)).manifest;
        // An older build names nothing: the page's configuration stays in force.
        assert_eq!(backend_from_manifest(&m, "1.0.0", &["a".into()]), ("1.0.0".into(), vec!["a".to_string()]));
        m.contract = Some("2.3.0".into());
        m.remote = vec![
            RemoteSig { name: "fund-totals".into(), idempotent: true, ..Default::default() },
            RemoteSig { name: "post-order".into(), ..Default::default() },
        ];
        let (c, i) = backend_from_manifest(&m, "1.0.0", &["a".into()]);
        println!("manifest says contract {c}, repeatable {i:?}");
        assert_eq!((c.as_str(), i), ("2.3.0", vec!["fund-totals".to_string()]));
    }
    // ── WASM-014 T15: progress, streams, files ───────────────────────────────

    /// A ready client whose connection events have been read.
    fn ready_quiet() -> Client<Mock> {
        let mut c = ready_client();
        c.take_out();
        c
    }

    /// Every frame sent since the last call, parsed, with the sent list emptied.
    fn drain_sent(c: &mut Client<Mock>) -> Vec<Value> {
        let v = sent_json(c);
        c.socket().sent.clear();
        v
    }

    /// Cut the connection and let the policy make a new one that works: the page is Ready again afterwards.
    fn cut_and_reconnect(c: &mut Client<Mock>, now: u64) {
        c.on_close(now);
        let next = c.next_deadline().unwrap_or(now + 1000);
        c.tick(next);
        c.on_open(next);
        c.on_message(&hello_ok());
        assert_eq!(c.state(), LinkState::Ready);
    }

    #[test]
    fn a_progress_frame_for_a_pending_call_is_reported_and_cancel_sends_the_cancel_frame() {
        let mut c = ready_quiet();
        let id = c.request(br#"{"program":"BIG-JOB","args":[]}"#, 10);
        drain_sent(&mut c);
        c.on_message(r#"{"t":"progress","id":1,"percent":40,"message":"half way"}"#);
        c.on_message(r#"{"t":"progress","id":77,"percent":10,"message":"nobody waits for it"}"#);
        let out = c.take_out();
        println!("progress reported: {out:?}");
        assert_eq!(out, vec![Out::Progress { id, percent: 40, message: "half way".into() }]);
        c.cancel(id);
        assert_eq!(drain_sent(&mut c), vec![json!({"t": "cancel", "id": id})], "cancel_sends_the_cancel_frame");
    }

    #[test]
    fn a_cancel_request_keeps_the_call_pending_until_the_backend_answers_it() {
        let mut c = ready_quiet();
        let id = c.request(br#"{"program":"BIG-JOB","args":[]}"#, 10);
        drain_sent(&mut c);
        assert!(c.request_cancel(id));
        assert_eq!(drain_sent(&mut c), vec![json!({"t": "cancel", "id": id})]);
        assert_eq!(c.in_flight(), 1, "still waiting for the answer");
        c.on_message(&format!(r#"{{"t":"error","id":{id},"class":"application","code":8000,"message":"cancelled"}}"#));
        let a = answers(&mut c);
        println!("the program is answered: {a:?}");
        assert_eq!(a[0].1["error"]["code"], 8000);
        assert!(!c.request_cancel(id), "nothing left to cancel");
    }

    #[test]
    fn a_dot_dot_name_is_refused_before_any_frame() {
        let mut c = ready_quiet();
        let hash = "0".repeat(64);
        let mut refused = Vec::new();
        for (slot, name) in [("docs", "../../etc/passwd"), ("docs", ".."), ("docs", "a\\b.txt"), ("../docs", "a.txt"), ("docs", "")] {
            c.put_begin(slot, name, "text/plain", 3, &hash, 20);
            refused.push(c.take_out().into_iter().filter(|o| matches!(o, Out::TransferFailed { class, .. } if class == "validation")).count());
        }
        let g = c.get_file("../secret", 20);
        let got = c.take_out();
        println!("refused uploads: {refused:?}; refused download {g}: {got:?}");
        assert_eq!(refused, [1, 1, 1, 1, 1]);
        assert!(matches!(got[0], Out::TransferFailed { .. }));
        assert!(drain_sent(&mut c).is_empty(), "nothing left the player");
    }

    /// 3 MiB in pieces; the connection is cut after the fifth piece; the upload resumes where the backend says it is.
    #[test]
    fn upload_3mb_in_pieces_resumes_after_a_cut_and_checksums() {
        let size: usize = 3 * 1024 * 1024;
        let data: Vec<u8> = (0..size).map(|i| (i.wrapping_mul(31) % 251) as u8).collect();
        // The browser hashes the file over the pieces it reads.
        let mut hash = crate::upload::FileHash::new();
        for piece in data.chunks(CHUNK_BYTES) {
            hash.update(piece);
        }
        assert_eq!(hash.len(), size as u64);
        let sha = hash.finish_hex();
        let mut c = ready_quiet();
        let t = std::time::Instant::now();
        let id = c.put_begin("ledger", "q3.csv", "text/csv", size as u64, &sha, 100);
        let begin = drain_sent(&mut c);
        assert_eq!(begin[0]["t"], "put-begin");
        assert_eq!(begin[0]["sha256"], sha.as_str());
        assert!(begin[0].get("resume").is_none());
        c.on_message(&format!(r#"{{"t":"put-ready","id":{id},"upload":"up-1","have":0}}"#));
        assert_eq!(c.take_out(), vec![Out::PutReady { id, have: 0 }]);
        // The backend's side: what it holds, piece by piece, as the encoded text the player sent.
        let mut held = 0u64;
        let mut pieces = 0usize;
        let mut offset = 0usize;
        let mut check = |frames: Vec<Value>, held: &mut u64, pieces: &mut usize, data: &[u8]| {
            for f in frames {
                if f["t"] == "put-chunk" {
                    let off = f["offset"].as_u64().unwrap() as usize;
                    assert_eq!(off as u64, *held, "pieces arrive in order");
                    let len = (data.len() - off).min(CHUNK_BYTES);
                    assert_eq!(f["data"].as_str().unwrap(), cobolt_wire::seal::encode_b64(&data[off..off + len]), "the bytes are the file's");
                    *held += len as u64;
                    *pieces += 1;
                }
            }
        };
        for _ in 0..5 {
            let len = (size - offset).min(CHUNK_BYTES);
            assert!(c.put_chunk(id, offset as u64, &data[offset..offset + len]));
            offset += len;
        }
        let first = drain_sent(&mut c);
        check(first, &mut held, &mut pieces, &data);
        assert_eq!(pieces, 5);
        // The connection is cut: the player makes a new one and asks to resume.
        cut_and_reconnect(&mut c, 200);
        let resume = drain_sent(&mut c);
        let begin2 = resume.iter().find(|f| f["t"] == "put-begin").expect("the upload is begun again");
        assert_eq!(begin2["resume"], "up-1", "under the id the backend gave");
        c.on_message(&format!(r#"{{"t":"put-ready","id":{id},"upload":"up-1","have":{held}}}"#));
        let ready = c.take_out().into_iter().find_map(|o| if let Out::PutReady { have, .. } = o { Some(have) } else { None }).expect("put-ready is reported again");
        assert_eq!(ready, held, "the host reads on from where the backend is");
        offset = ready as usize;
        while offset < size {
            let len = (size - offset).min(CHUNK_BYTES);
            assert!(c.put_chunk(id, offset as u64, &data[offset..offset + len]));
            offset += len;
        }
        let rest = drain_sent(&mut c);
        check(rest, &mut held, &mut pieces, &data);
        assert!(c.put_end(id));
        assert_eq!(drain_sent(&mut c), vec![json!({"t": "put-end", "id": id})]);
        c.on_message(&format!(r#"{{"t":"put-done","id":{id},"file":"f-9","size":{size}}}"#));
        assert_eq!(c.take_out(), vec![Out::PutDone { id, file: "f-9".into(), size: size as u64 }]);
        let ms = t.elapsed().as_millis();
        println!("== T15 upload ==");
        println!("{size} bytes in {pieces} pieces of up to {CHUNK_BYTES}; cut after 5 pieces ({held_at_cut} bytes held), resumed at offset {ready}; sha256 {sha}; {ms} ms", held_at_cut = 5 * CHUNK_BYTES);
        assert_eq!(held, size as u64);
        assert_eq!(pieces, size.div_ceil(CHUNK_BYTES));
    }

    #[test]
    fn a_download_is_the_address_the_backend_issues() {
        let mut c = ready_quiet();
        let id = c.get_file("f-9", 5);
        assert_eq!(drain_sent(&mut c), vec![json!({"t": "get-file", "id": id, "file": "f-9"})]);
        c.on_message(&format!(r#"{{"t":"file-url","id":{id},"path":"/dl/abc","expires_in":60,"size":12,"sha256":"{}"}}"#, "ab".repeat(32)));
        let out = c.take_out();
        assert_eq!(out, vec![Out::FileUrl { id, path: "/dl/abc".into(), expires_in: 60, size: 12, sha256: "ab".repeat(32) }]);
        // A refusal ends it with the backend's own words.
        let id2 = c.get_file("gone", 6);
        c.on_message(&format!(r#"{{"t":"error","id":{id2},"class":"not-found","code":5001,"message":"no such file"}}"#));
        assert!(matches!(c.take_out().as_slice(), [Out::TransferFailed { code: 5001, .. }]));
    }

    #[test]
    fn stream_items_arrive_in_order_and_the_end_is_reported() {
        let mut c = ready_quiet();
        let sub = c.subscribe(br#"{"program":"ticker","args":[{"mode":"value","json":{"EVERY-MS":30}}]}"#, 7);
        let sent = drain_sent(&mut c);
        assert_eq!(sent[0]["t"], "sub");
        assert_eq!(sent[0]["id"], sub);
        assert_eq!(sent[0]["program"], "ticker");
        for n in 1..=3 {
            c.on_message(&format!(r#"{{"t":"event","sub":{sub},"seq":{n},"data":{{"N":{n}}}}}"#));
        }
        c.on_message(&format!(r#"{{"t":"sub-end","sub":{sub}}}"#));
        let out = c.take_out();
        println!("stream: {out:?}");
        assert_eq!(out[0], Out::StreamItem { sub, json: r#"{"N":1}"#.into(), restart: false });
        assert_eq!(out[2], Out::StreamItem { sub, json: r#"{"N":3}"#.into(), restart: false });
        assert_eq!(out[3], Out::StreamEnd { sub, reason: "ended".into() });
        assert!(c.subscriptions().is_empty());
    }

    #[test]
    fn reconnect_resubscribes() {
        let mut c = ready_quiet();
        let sub = c.subscribe(br#"{"program":"ticker","args":[]}"#, 7);
        drain_sent(&mut c);
        c.on_message(&format!(r#"{{"t":"event","sub":{sub},"seq":1,"data":{{"N":1}}}}"#));
        cut_and_reconnect(&mut c, 300);
        let again: Vec<Value> = drain_sent(&mut c).into_iter().filter(|f| f["t"] == "sub").collect();
        println!("after the cut: {again:?}");
        assert_eq!(again.len(), 1, "subscribed again, once");
        assert_eq!(again[0]["id"], sub, "under the same id");
        c.take_out();
        c.on_message(&format!(r#"{{"t":"event","sub":{sub},"seq":1,"data":{{"N":2}}}}"#));
        c.on_message(&format!(r#"{{"t":"event","sub":{sub},"seq":2,"data":{{"N":3}}}}"#));
        let items: Vec<bool> = c.take_out().into_iter().filter_map(|o| if let Out::StreamItem { restart, .. } = o { Some(restart) } else { None }).collect();
        assert_eq!(items, [true, false], "the restart flag is on the first item after the re-subscription only");
        // Unsubscribing tells the backend, and its answer ends the stream.
        c.unsubscribe(sub);
        assert_eq!(drain_sent(&mut c), vec![json!({"t": "unsub", "id": sub})]);
        c.on_message(&format!(r#"{{"t":"unsubscribed","id":{sub}}}"#));
        assert_eq!(c.take_out(), vec![Out::StreamEnd { sub, reason: "unsubscribed".into() }]);
    }

    /// The modal's status line follows the cycle: attempt n of 3, how the one before ended, what is happening now; after the
    /// give-up the user's Retry starts the whole cycle again from attempt 1.
    #[test]
    fn the_progress_follows_the_cycle_and_retry_starts_it_over() {
        let mut c = ready_client();
        assert_eq!(c.progress(), None, "nothing to show while the link works");
        c.request(br#"{"program":"FUND-TOTALS"}"#, 0);
        let t0 = 1_000;
        c.on_close(t0);
        let p = c.progress().expect("blocked");
        assert_eq!((p.attempt, p.of, p.last, p.phase, p.failed, p.window_s), (1, 3, None, Phase::Connecting, None, 10));
        c.on_error(t0);
        let p = c.progress().unwrap();
        assert_eq!((p.attempt, p.last, p.phase, p.failed), (1, None, Phase::Waiting, Some(Failure::NoConnection)));
        c.tick(t0 + 10_000);
        let p = c.progress().unwrap();
        assert_eq!((p.attempt, p.last, p.phase, p.failed, p.window_s), (2, Some(Failure::NoConnection), Phase::Connecting, None, 20));
        c.on_open(t0 + 10_001);
        let p = c.progress().unwrap();
        assert_eq!((p.attempt, p.phase), (2, Phase::Handshake));
        c.on_close(t0 + 10_002);
        let p = c.progress().unwrap();
        assert_eq!((p.phase, p.failed), (Phase::Waiting, Some(Failure::Handshake)));
        refuse_every_attempt(&mut c, u64::MAX);
        let p = c.progress().unwrap();
        assert_eq!((p.attempt, p.of, p.phase), (3, 3, Phase::GaveUp));
        assert!(p.failed.is_some() && p.last.is_some(), "the last two attempts both have an outcome: {p:?}");
        // Retry: the whole cycle again, behind the same modal.
        c.take_out();
        let opened = c.sock.opened.len();
        c.retry(t0 + 700_000);
        let p = c.progress().unwrap();
        assert_eq!((p.attempt, p.last, p.phase, p.failed), (1, None, Phase::Connecting, None));
        assert_eq!(c.sock.opened.len(), opened + 1, "attempt 1 opens a socket");
        let out = c.take_out();
        assert!(out.iter().any(|o| matches!(o, Out::Net(n) if n.attempt == 1)), "the modal hears of the new cycle");
        assert_eq!(flags(&out), ["unreachable=false", "blocked=true"], "Retry takes the failed state away and blocks the application again");
        c.retry(t0 + 700_001); // a second press while trying does nothing
        assert_eq!(c.sock.opened.len(), opened + 1);
        println!("progress: 1/3 connecting -> failed no-connection -> 2/3 (last no-connection) -> handshake dropped -> 3/3 gave up -> Retry restarts at 1/3");
    }
}
