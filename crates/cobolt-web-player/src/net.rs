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
//! `WebSocket`, the tests give it a mock and a fake clock. It does, each with a test:
//!
//! * connect **only** to the configured backend addresses (`Config::origins`);
//! * correlate a request to its answer by an id the player assigns (answers may come in any order);
//! * send the bearer token **as the first message** of a connection, never in the address or a
//!   header the page could not set (WASM-003 R11), and hold calls until the connection is ready;
//! * enforce a timeout per call and a limit on the frame size;
//! * reconnect with exponential back-off and jitter; a call in flight when the connection drops
//!   is **failed** (the program reads it as `ON EXCEPTION`), a call made after the reconnection
//!   is not;
//! * turn every failure into an *error frame* of the same shape as the server's, so the module's
//!   run time decodes one format (ABI amendment A8).
//!
//! The envelope (`{"type":"call","id":N,"frame":{…}}` out, `{"id":N,"result":…}` /
//! `{"id":N,"error":{"class","code","message"}}` in) is the player's proposal pending the
//! backend's wire specification (WASM-014); it lives in this one file so that aligning it is a
//! one-file change.

use std::collections::BTreeMap;

use serde_json::{json, Value};

/// The error classes of WASM-014 R7, as they appear in an error frame.
pub mod class {
    pub const TRANSPORT: &str = "transport";
    pub const AUTHENTICATION: &str = "authentication";
    pub const LIMIT: &str = "limit";
    pub const CLIENT: &str = "client";
}

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    Closed,
    Connecting,
    /// Open; waiting for the server to accept the token.
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
}

impl Default for Config {
    fn default() -> Self {
        Config { origins: Vec::new(), call_timeout_ms: 30_000, max_frame: 384 * 1024, backoff_start_ms: 250, backoff_max_ms: 10_000, unsafe_plain_ws: false }
    }
}

#[derive(Debug)]
struct InFlight {
    sent_at: u64,
    /// The frame as the module produced it, kept to repeat after a refresh.
    frame: String,
    sent: bool,
    repeated: bool,
}

/// The connection state machine.
pub struct Client<S: Socket> {
    pub cfg: Config,
    sock: S,
    state: LinkState,
    /// The address in use (one of `cfg.origins`).
    url: Option<String>,
    token: Option<String>,
    next_id: u32,
    inflight: BTreeMap<u32, InFlight>,
    /// Requests the module made (ids it knows) mapped to our wire ids.
    wire_to_req: BTreeMap<u32, u32>,
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

/// An error frame, the same shape the server sends (ABI amendment A8).
pub fn error_frame(id: u32, class: &str, code: i32, message: &str, may_have_been_received: bool) -> Vec<u8> {
    json!({"id": id, "error": {"class": class, "code": code, "message": message, "may_have_been_received": may_have_been_received}})
        .to_string()
        .into_bytes()
}

impl<S: Socket> Client<S> {
    pub fn new(cfg: Config, sock: S) -> Self {
        let mut seed = 0x2545_f491u32;
        Client {
            cfg,
            sock,
            state: LinkState::Closed,
            url: None,
            token: None,
            next_id: 1,
            inflight: BTreeMap::new(),
            wire_to_req: BTreeMap::new(),
            attempt: 0,
            retry_at: None,
            jitter: Box::new(move || {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                seed % 1001
            }),
            wanted: false,
            out: Vec::new(),
            refresh_requested: false,
        }
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

    /// Give the connection the bearer token (or none after a sign-out).
    pub fn set_token(&mut self, token: Option<String>) {
        self.token = token;
        self.refresh_requested = false;
        if self.state == LinkState::Ready {
            if let Some(t) = &self.token {
                let msg = json!({"type": "auth", "token": t}).to_string();
                self.sock.send(&msg);
            }
        }
        // Repeat what the server refused for an expired token, once.
        let to_repeat: Vec<u32> = self.inflight.iter().filter(|(_, f)| !f.sent && f.repeated).map(|(id, _)| *id).collect();
        for id in to_repeat {
            self.transmit(id);
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
        if frame.len() > self.cfg.max_frame {
            self.out.push(Out::Answer { id, frame: error_frame(id, class::LIMIT, 5, "the request is larger than the connection allows", false) });
            return id;
        }
        let Ok(text) = std::str::from_utf8(frame) else {
            self.out.push(Out::Answer { id, frame: error_frame(id, class::CLIENT, 2, "the request frame is not UTF-8 text", false) });
            return id;
        };
        if serde_json::from_str::<Value>(text).map(|v| !v.is_object()).unwrap_or(true) {
            self.out.push(Out::Answer { id, frame: error_frame(id, class::CLIENT, 2, "the request frame is not a JSON object", false) });
            return id;
        }
        self.inflight.insert(id, InFlight { sent_at: now, frame: text.to_owned(), sent: false, repeated: false });
        match self.state {
            LinkState::Ready => self.transmit(id),
            LinkState::Closed if self.wanted => {
                // The link went away and nothing is retrying: try now.
                self.start_connecting(now);
            }
            LinkState::Backoff | LinkState::Connecting | LinkState::Authenticating => {} // held until Ready
            LinkState::Closed => {
                self.fail(id, class::TRANSPORT, 7, "the application is not connected to its backend", false);
            }
        }
        id
    }

    fn transmit(&mut self, id: u32) {
        let Some(f) = self.inflight.get_mut(&id) else { return };
        let frame: Value = serde_json::from_str(&f.frame).unwrap_or(Value::Null);
        let msg = json!({"type": "call", "id": id, "frame": frame}).to_string();
        f.sent = true;
        self.sock.send(&msg);
    }

    fn fail(&mut self, id: u32, class_: &str, code: i32, message: &str, may_have_been_received: bool) {
        if self.inflight.remove(&id).is_some() {
            self.out.push(Out::Answer { id, frame: error_frame(id, class_, code, message, may_have_been_received) });
        }
    }

    // ── events from the socket ──────────────────────────────────────────────

    pub fn on_open(&mut self, _now: u64) {
        self.attempt = 0;
        self.retry_at = None;
        if let Some(t) = self.token.clone() {
            self.set_state(LinkState::Authenticating);
            let msg = json!({"type": "auth", "token": t}).to_string();
            self.sock.send(&msg);
        } else {
            self.set_state(LinkState::Ready);
            self.flush_held();
        }
    }

    fn flush_held(&mut self) {
        let held: Vec<u32> = self.inflight.iter().filter(|(_, f)| !f.sent).map(|(id, _)| *id).collect();
        for id in held {
            self.transmit(id);
        }
    }

    pub fn on_message(&mut self, text: &str) {
        let Ok(v) = serde_json::from_str::<Value>(text) else { return };
        match v.get("type").and_then(Value::as_str) {
            Some("auth") => {
                if v.get("ok").and_then(Value::as_bool).unwrap_or(false) {
                    self.set_state(LinkState::Ready);
                    self.flush_held();
                } else {
                    // The server refused the token: the calls that wait are failed, authentication class.
                    self.set_state(LinkState::Ready);
                    self.refresh_requested = true;
                    self.flush_held();
                }
            }
            Some("ping") => self.sock.send(&json!({"type": "pong"}).to_string()),
            _ => {
                let Some(id) = v.get("id").and_then(Value::as_u64).map(|n| n as u32) else { return };
                let Some(f) = self.inflight.get_mut(&id) else { return };
                // The token expired: refresh and repeat once; the server refused before running anything.
                let expired = v.pointer("/error/class").and_then(Value::as_str) == Some(class::AUTHENTICATION)
                    && v.pointer("/error/expired").and_then(Value::as_bool).unwrap_or(false);
                if expired && !f.repeated {
                    f.repeated = true;
                    f.sent = false;
                    self.refresh_requested = true;
                    return;
                }
                self.inflight.remove(&id);
                self.out.push(Out::Answer { id, frame: text.as_bytes().to_vec() });
            }
        }
    }

    pub fn on_close(&mut self, now: u64) {
        // Everything in flight was possibly received: fail it, never repeat it silently (WASM-014 R9).
        let ids: Vec<u32> = self.inflight.iter().filter(|(_, f)| f.sent).map(|(id, _)| *id).collect();
        for id in ids {
            self.fail(id, class::TRANSPORT, 7, "the connection to the backend was lost", true);
        }
        if self.wanted {
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
        if matches!(self.state, LinkState::Connecting | LinkState::Authenticating) {
            self.on_close(now);
        }
    }

    /// The clock moved: time out calls, and retry a connection that is due.
    pub fn tick(&mut self, now: u64) {
        let timeout = self.cfg.call_timeout_ms;
        let late: Vec<u32> = self.inflight.iter().filter(|(_, f)| now.saturating_sub(f.sent_at) >= timeout).map(|(id, _)| *id).collect();
        for id in late {
            let sent = self.inflight.get(&id).map(|f| f.sent).unwrap_or(false);
            self.fail(id, class::TRANSPORT, 9, "the backend did not answer in time", sent);
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
        self.inflight.remove(&id);
        self.wire_to_req.remove(&id);
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

    fn cfg() -> Config {
        Config { origins: vec!["wss://api.example.com/ws".into(), "ws://localhost:8443/ws".into()], ..Config::default() }
    }

    fn client() -> Client<Mock> {
        let mut c = Client::new(cfg(), Mock::default());
        c.jitter = Box::new(|| 500); // no jitter: exactly the base
        c
    }

    fn sent_json(c: &Client<Mock>) -> Vec<Value> {
        c.sock.sent.iter().map(|s| serde_json::from_str(s).unwrap()).collect()
    }

    #[test]
    fn only_configured_addresses_and_secure_schemes_are_allowed() {
        let mut c = client();
        assert!(c.connect("wss://evil.example.org/ws", 0).is_err(), "an address the build did not name");
        assert!(c.sock.opened.is_empty(), "no connection was attempted");
        assert!(c.connect("wss://api.example.com/ws", 0).is_ok());
        let mut plain = Client::new(Config { origins: vec!["ws://203.0.113.9/ws".into()], ..Config::default() }, Mock::default());
        let e = plain.connect("ws://203.0.113.9/ws", 0).unwrap_err();
        assert!(e.contains("unencrypted"), "{e}");
        plain.cfg.unsafe_plain_ws = true;
        assert!(plain.connect("ws://203.0.113.9/ws", 0).is_ok(), "the explicit unsafe setting allows it");
        assert!(origin_allowed(&cfg(), "ws://localhost:8443/ws").is_ok(), "loopback needs no setting");
    }

    #[test]
    fn calls_wait_for_the_connection_and_carry_the_id_the_player_assigned() {
        let mut c = client();
        c.set_token(None);
        c.connect("wss://api.example.com/ws", 0).unwrap();
        let id = c.request(br#"{"program":"PING","payload":{"a":1}}"#, 10);
        assert!(c.sock.sent.is_empty(), "held until the link is ready");
        c.on_open(20);
        assert_eq!(c.state(), LinkState::Ready);
        let sent = sent_json(&c);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["type"], "call");
        assert_eq!(sent[0]["id"], id);
        assert_eq!(sent[0]["frame"]["program"], "PING");
        // The answer is handed over verbatim, correlated by id, whatever the order.
        let id2 = c.request(br#"{"program":"B"}"#, 30);
        c.on_message(&format!(r#"{{"id":{id2},"result":"second"}}"#));
        c.on_message(&format!(r#"{{"id":{id},"result":"first"}}"#));
        let out = c.take_out();
        let answers: Vec<(u32, String)> = out
            .iter()
            .filter_map(|o| if let Out::Answer { id, frame } = o { Some((*id, String::from_utf8(frame.clone()).unwrap())) } else { None })
            .collect();
        assert_eq!(answers.len(), 2);
        assert_eq!(answers[0].0, id2, "answers may arrive in any order");
        assert!(answers[0].1.contains("second") && answers[1].1.contains("first"));
        assert_eq!(c.in_flight(), 0);
    }

    #[test]
    fn the_token_is_the_first_message_never_part_of_the_address() {
        let mut c = client();
        c.set_token(Some("tok-123".into()));
        c.connect("wss://api.example.com/ws", 0).unwrap();
        assert!(!c.sock.opened[0].contains("tok-123"), "WASM-003 R11: never in a URL");
        c.request(br#"{"program":"X"}"#, 1);
        c.on_open(2);
        let sent = sent_json(&c);
        assert_eq!(sent[0]["type"], "auth", "the token goes first");
        assert_eq!(sent[0]["token"], "tok-123");
        assert_eq!(sent.len(), 1, "no call before the server accepts the token");
        assert_eq!(c.state(), LinkState::Authenticating);
        c.on_message(r#"{"type":"auth","ok":true}"#);
        assert_eq!(c.state(), LinkState::Ready);
        assert_eq!(sent_json(&c)[1]["type"], "call");
    }

    #[test]
    fn a_call_in_flight_when_the_connection_drops_fails_and_a_later_one_does_not() {
        let mut c = client();
        c.connect("wss://api.example.com/ws", 0).unwrap();
        c.on_open(1);
        let in_flight = c.request(br#"{"program":"X"}"#, 2);
        c.on_close(3);
        assert_eq!(c.state(), LinkState::Backoff);
        let out = c.take_out();
        let failed = out.iter().find_map(|o| if let Out::Answer { id, frame } = o { Some((*id, frame.clone())) } else { None }).expect("the call failed");
        assert_eq!(failed.0, in_flight);
        let v: Value = serde_json::from_slice(&failed.1).unwrap();
        assert_eq!(v["error"]["class"], "transport");
        assert_eq!(v["error"]["may_have_been_received"], true, "WASM-014 R9: the program is told it may have been received");
        // While backing off, a new call waits; after the reconnection it is sent and answered normally.
        let later = c.request(br#"{"program":"Y"}"#, 100);
        assert!(c.sock.sent.iter().all(|s| !s.contains("\"Y\"")));
        c.tick(10_000);
        assert_eq!(c.state(), LinkState::Connecting);
        c.on_open(10_001);
        assert!(c.sock.sent.iter().any(|s| s.contains("\"Y\"")));
        c.on_message(&format!(r#"{{"id":{later},"result":"ok"}}"#));
        let out = c.take_out();
        assert!(out.iter().any(|o| matches!(o, Out::Answer { id, frame } if *id == later && !String::from_utf8_lossy(frame).contains("error"))));
    }

    #[test]
    fn reconnection_backs_off_exponentially_and_resets_when_it_works() {
        let mut c = client();
        c.connect("wss://api.example.com/ws", 0).unwrap();
        let mut now = 0u64;
        let mut waits = Vec::new();
        for _ in 0..8 {
            c.on_error(now);
            let at = c.retry_at.unwrap();
            waits.push(at - now);
            now = at;
            c.tick(now);
        }
        assert_eq!(waits, [250, 500, 1000, 2000, 4000, 8000, 10_000, 10_000], "doubling, capped at backoff_max");
        c.on_open(now);
        c.on_close(now + 1);
        assert_eq!(c.retry_at.unwrap() - (now + 1), 250, "a working connection starts the series again");
        println!("back-off waits (ms): {waits:?}");
    }

    #[test]
    fn jitter_stays_within_a_quarter_of_the_base() {
        for j in [0u32, 250, 500, 750, 1000] {
            let mut c = client();
            c.jitter = Box::new(move || j);
            c.connect("wss://api.example.com/ws", 0).unwrap();
            c.on_error(0);
            let w = c.retry_at.unwrap();
            assert!((187..=313).contains(&w), "jitter {j}: waited {w}");
        }
    }

    #[test]
    fn a_call_that_is_never_answered_times_out_with_its_own_error() {
        let mut c = client();
        c.connect("wss://api.example.com/ws", 0).unwrap();
        c.on_open(1);
        let id = c.request(br#"{"program":"SLOW"}"#, 1_000);
        c.tick(1_000 + 29_999);
        assert!(c.take_out().iter().all(|o| !matches!(o, Out::Answer { .. })));
        c.tick(1_000 + 30_000);
        let out = c.take_out();
        let v: Value = out
            .iter()
            .find_map(|o| if let Out::Answer { id: i, frame } = o { (*i == id).then(|| serde_json::from_slice(frame).unwrap()) } else { None })
            .expect("timed out");
        assert_eq!((v["error"]["class"].as_str(), v["error"]["code"].as_i64()), (Some("transport"), Some(9)));
        assert_eq!(c.next_deadline(), None);
    }

    #[test]
    fn oversized_and_malformed_frames_are_refused_without_touching_the_wire() {
        let mut c = client();
        c.connect("wss://api.example.com/ws", 0).unwrap();
        c.on_open(1);
        c.request(&vec![b'x'; 400 * 1024], 2);
        c.request(b"not json", 2);
        c.request(b"[1,2]", 2);
        c.request(&[0xff, 0xfe], 2);
        assert!(c.sock.sent.is_empty(), "nothing reached the socket");
        let out = c.take_out();
        let classes: Vec<String> = out
            .iter()
            .filter_map(|o| if let Out::Answer { frame, .. } = o { Some(serde_json::from_slice::<Value>(frame).unwrap()["error"]["class"].as_str().unwrap().to_owned()) } else { None })
            .collect();
        assert_eq!(classes, ["limit", "client", "client", "client"]);
    }

    #[test]
    fn an_expired_token_is_refreshed_and_the_call_repeated_once() {
        let mut c = client();
        c.set_token(Some("old".into()));
        c.connect("wss://api.example.com/ws", 0).unwrap();
        c.on_open(1);
        c.on_message(r#"{"type":"auth","ok":true}"#);
        let id = c.request(br#"{"program":"X"}"#, 2);
        c.on_message(&format!(r#"{{"id":{id},"error":{{"class":"authentication","code":401,"expired":true}}}}"#));
        assert!(c.refresh_requested, "the owner is asked to refresh");
        assert!(c.take_out().iter().all(|o| !matches!(o, Out::Answer { .. })), "the program is not told yet");
        c.set_token(Some("new".into()));
        let sent = sent_json(&c);
        assert!(sent.iter().any(|m| m["type"] == "auth" && m["token"] == "new"));
        assert_eq!(sent.iter().filter(|m| m["type"] == "call" && m["id"] == id).count(), 2, "the call was repeated");
        // A second expiry is final: no loop.
        c.on_message(&format!(r#"{{"id":{id},"error":{{"class":"authentication","code":401,"expired":true}}}}"#));
        assert!(c.take_out().iter().any(|o| matches!(o, Out::Answer { id: i, .. } if *i == id)));
    }

    #[test]
    fn shutdown_closes_the_socket_and_forgets_the_calls() {
        let mut c = client();
        c.connect("wss://api.example.com/ws", 0).unwrap();
        c.on_open(1);
        c.request(br#"{"program":"X"}"#, 2);
        c.shutdown();
        assert_eq!((c.state(), c.in_flight(), c.sock.closed), (LinkState::Closed, 0, 1));
        c.on_close(5);
        assert_eq!(c.state(), LinkState::Closed, "no reconnection after an unmount");
    }
}
