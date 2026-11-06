// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Fuzzing of the server's message dispatch and of the transport under it (WASM-013 R15, T20; AC11, dispatch).
//!
//! The server reads, over a real TLS loopback socket, what a stranger sends. Three targets hold it to one sentence each:
//!
//! 1. **A mutated frame never breaks the connection's logic.** A signed-in client sends frames that are a valid one of every
//!    kind with a few bytes changed (the wire fuzz's mutations, applied to what the server reads), each followed by a call
//!    that is *known* to be answerable. Every answer the server gives parses as a server frame the client's own parser
//!    accepts, every refusal is a named error of a class the wire documents and never a fault of the backend itself, the
//!    call that follows is answered or the connection is closed with a close code (never dropped, never silent), and no task
//!    panicked. Planted strings shaped like a token and like a password travel in the frames; the server's log is searched
//!    for them afterwards (T18's invariant, held under hostile input).
//! 2. **Every way to break the transport ends with the close code that names it** (RFC 6455 section 7.4): 1002 for a frame that
//!    is not what the channel allows (a plain text after the exchange, a binary before it, a sealed frame that does not open,
//!    is truncated, bit-flipped or replayed, a `hello` that is refused), 1009 for a message over `max_frame_bytes` (after the
//!    named limit error), 1008 for a connection that never says `hello` and for five failed sign-ins, 1000 for `bye`, 1001
//!    for a backend that is shutting down. After each, a well-behaved client still gets its answer.
//! 3. **Floods are absorbed.** Twenty thousand ping frames, a thousand `hello`s on an agreed channel, a hundred and twenty
//!    connections at a server that accepts forty: the backend keeps answering a good client, and gives the slots back.
//!
//! A fixed seed makes a failure reproduce; `PR_FUZZ_ITERS=<n>` multiplies the mutated frames (default 1).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Once;
use std::time::{Duration, Instant};

use cobolt_server::testkit::*;
use cobolt_wire::error::{codes, ErrorClass};
use cobolt_wire::proto;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

const EXPORTS: &str = "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"GET-CUSTOMER\"\n\n[[export]]\nprogram = \"PING\"\npublic = true\n";

const GET_CUSTOMER: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. GET-CUSTOMER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-SUB PIC X(40).
LINKAGE SECTION.
01 LK-REQ.
    05 CUST-ID   PIC 9(6).
    05 CUST-NAME PIC X(40).
    05 BALANCE   PIC S9(5)V99.
    05 PASSWORD  PIC X(12).
01 LK-RES.
    05 RC  PIC 99.
    05 TXT PIC X(10).
PROCEDURE DIVISION USING LK-REQ RETURNING LK-RES.
MAIN.
    COBOL::"WS-CLAIM" ( "sub" WS-SUB )
    MOVE WS-SUB TO CUST-NAME
    COMPUTE BALANCE = CUST-ID / 100
    MOVE 7 TO RC
    MOVE "found" TO TXT
    GOBACK.
"#;

const PING: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. PING.
DATA DIVISION.
LINKAGE SECTION.
01 LK-PONG.
    05 REPLY PIC X(4).
PROCEDURE DIVISION RETURNING LK-PONG.
MAIN.
    MOVE "pong" TO REPLY
    GOBACK.
"#;

/// A string shaped like a token, in three parts, and two like a password (one that fits its field, one that does not).
const SENT_TOKEN: &str = "eyJhbGciOiJSUzI1NiJ9.SENTINEL-PAYLOAD-7f3a9c41.SENTINEL-SIGNATURE-51d2e8b0";
const SENT_PW_FIT: &str = "S3NT1NEL-pw9";
const SENT_PW_LONG: &str = "S3NT1NEL-too-long-password-8e1c";
const SEED: u64 = 0xD15_9A7C_4F22_0B1D;
const MUTATED: u64 = 3_000;
/// Calls the loop sends as the answerable call after each mutated frame carry ids from here.
const SENTINEL_ID: u64 = 9_000_000_000;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len() as u64) as usize]
    }
}

fn iters() -> u64 {
    std::env::var("PR_FUZZ_ITERS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(1).max(1)
}

// ── a panic anywhere in the process, server tasks included, is a failure ────

static PANICS: AtomicUsize = AtomicUsize::new(0);
static HOOK: Once = Once::new();

fn watch_for_panics() {
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            PANICS.fetch_add(1, Ordering::SeqCst);
            previous(info);
        }));
    });
}

// ── the rig ─────────────────────────────────────────────────────────────────

fn new_rig(name: &str, limits: &str) -> Rig {
    watch_for_panics();
    rig(backend_dir(name, EXPORTS, &[("get-customer.cbl", GET_CUSTOMER), ("ping.cbl", PING)]), &format!("[limits]\n{limits}\n"))
}

fn token(rig: &Rig) -> String {
    rig.issuer.access("fuzz-user", AUDIENCE, 3600, json!({}))
}

async fn ready(rig: &Rig, token: &str) -> Raw {
    let mut c = Raw::connect(rig, Some(ORIGIN)).await.expect("connect");
    let h = c.hello("1.0.0").await;
    assert_eq!(h["accepted"], "ok", "{h}");
    let a = c.auth(token).await;
    assert_eq!(a["t"], "auth-ok", "{a}");
    c
}

/// What a client that reads the server's frames saw next.
enum Got {
    /// A frame, as plain text.
    Msg(String),
    /// The connection ended: with a close frame carrying this code, with one that carries none (0), or abruptly (`None`).
    Closed(Option<u16>),
    /// Nothing within the wait, the connection still open.
    Quiet,
}

async fn recv(c: &mut Raw, wait: Duration) -> Got {
    loop {
        match tokio::time::timeout(wait, c.ws.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => return Got::Msg(t.as_str().to_owned()),
            Ok(Some(Ok(Message::Binary(b)))) => {
                let rx = c.receiver.as_mut().expect("the server sent a sealed frame before the exchange");
                match rx.open(&b) {
                    Ok(plain) => return Got::Msg(plain.to_string()),
                    Err(_) => panic!("the server sent a sealed frame its own channel cannot open"),
                }
            }
            Ok(Some(Ok(Message::Close(Some(f))))) => return Got::Closed(Some(u16::from(f.code))),
            Ok(Some(Ok(Message::Close(None)))) => return Got::Closed(Some(0)),
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(_))) | Ok(None) => return Got::Closed(None),
            Err(_) => return Got::Quiet,
        }
    }
}

/// Seal and send `text`; false when the connection is gone.
async fn send(c: &mut Raw, text: &str) -> bool {
    let frame = c.sender.as_mut().expect("the channel is agreed").seal(text).expect("seal");
    c.ws.send(Message::Binary(frame.into())).await.is_ok()
}

fn ping_call(id: u64) -> String {
    format!(r#"{{"t":"call","id":{id},"program":"PING","args":[],"returning":true}}"#)
}

/// Read until the connection ends or `wait` passes with nothing; the close code, if it ended.
async fn closed_with(c: &mut Raw, within: Duration) -> Option<Option<u16>> {
    let end = Instant::now() + within;
    loop {
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return None;
        }
        match recv(c, left).await {
            Got::Msg(_) => continue,
            Got::Closed(code) => return Some(code),
            Got::Quiet => return None,
        }
    }
}

/// A fresh, well-behaved client gets its answer: the backend is up and the contract of a call holds.
async fn still_answers(rig: &Rig) {
    let mut c = ready(rig, &token(rig)).await;
    let r = c.call(1, "PING", &[], true).await;
    assert_eq!(r["t"], "reply", "a good client is answered after the abuse: {r}");
    assert_eq!(r["returning"]["REPLY"], "pong", "{r}");
    let req = r#"{"CUST-ID":4250,"CUST-NAME":"","BALANCE":0,"PASSWORD":"hunter2"}"#;
    let r = c.call(2, "GET-CUSTOMER", &[("ref", req)], true).await;
    assert_eq!(r["returning"]["RC"], 7, "{r}");
}

fn no_panics() {
    assert_eq!(PANICS.load(Ordering::SeqCst), 0, "a task panicked");
}

// ── 1. mutated frames ───────────────────────────────────────────────────────

fn seeds() -> Vec<String> {
    let key = "A".repeat(43);
    let call_pw = |pw: &str| format!(r#"{{"t":"call","id":7,"program":"GET-CUSTOMER","args":[{{"mode":"ref","json":{{"CUST-ID":4250,"CUST-NAME":"","BALANCE":0,"PASSWORD":"{pw}"}}}}],"returning":true}}"#);
    vec![
        format!(r#"{{"t":"hello","protocol":2,"contract":"1.0.0","client":"fuzz","key":"{key}=","nonce":"{}"}}"#, "B".repeat(22)),
        format!(r#"{{"t":"auth","token":"{SENT_TOKEN}"}}"#),
        call_pw(SENT_PW_FIT),
        call_pw(SENT_PW_LONG),
        ping_call(8),
        format!(r#"{{"t":"call","id":9,"program":"PING","args":[],"returning":true,"key":"s-00000009","token":"{SENT_TOKEN}","timeout_ms":5000}}"#),
        r#"{"t":"cancel","id":7}"#.into(),
        r#"{"t":"sub","id":3,"program":"PING","args":[]}"#.into(),
        r#"{"t":"unsub","id":3}"#.into(),
        format!(r#"{{"t":"put-begin","id":9,"slot":"AVATAR","name":"ana.png","size":2048,"mime":"image/png","sha256":"{}","resume":"u-1"}}"#, "ab".repeat(32)),
        r#"{"t":"put-chunk","id":9,"offset":0,"data":"iVBORw0KGgo="}"#.into(),
        r#"{"t":"put-end","id":9}"#.into(),
        r#"{"t":"get-file","id":11,"file":"f-0001"}"#.into(),
    ]
}

const SPECIAL: &[u8] = b"\"{}[]:,\\ \t\n\r\x00\x7f\xff0123456789-+eE.";
const FRAGMENTS: &[&str] = &["\"t\":\"call\"", "[", "{", "]", "}", "null", "18446744073709551616", "-1", "1e999", "\"\"", "\\u0000", "\"id\":1", "\"program\":\"PING\"", ",,", "\u{FEFF}"];

fn mutate(rng: &mut Rng, seed: &str) -> String {
    let mut b = seed.as_bytes().to_vec();
    for _ in 0..1 + rng.below(3) {
        if b.is_empty() {
            b.push(b'{');
        }
        let at = rng.below(b.len() as u64) as usize;
        match rng.below(10) {
            0 => b[at] ^= 1 << rng.below(8),
            1 => b[at] = rng.next() as u8,
            2 => b[at] = *rng.pick(SPECIAL),
            3 => {
                let n = (1 + rng.below(12) as usize).min(b.len() - at);
                b.drain(at..at + n);
            }
            4 => {
                let n = 1 + rng.below(8) as usize;
                let bytes: Vec<u8> = (0..n).map(|_| rng.next() as u8).collect();
                b.splice(at..at, bytes);
            }
            5 => {
                let n = (1 + rng.below(40) as usize).min(b.len() - at);
                let piece = b[at..at + n].to_vec();
                b.splice(at..at, piece);
            }
            6 => b.truncate(at),
            7 => {
                let f = *rng.pick(FRAGMENTS);
                b.splice(at..at, f.bytes());
            }
            8 => {
                let other = rng.below(b.len() as u64) as usize;
                b.swap(at, other);
            }
            _ => b.splice(at..at, [b'"']).for_each(drop),
        }
    }
    String::from_utf8_lossy(&b).into_owned()
}

/// Where a value sits in a frame: object keys and array positions from the root.
#[derive(Clone)]
enum Step {
    Key(String),
    At(usize),
}

fn paths(v: &Value, here: &mut Vec<Step>, out: &mut Vec<Vec<Step>>) {
    out.push(here.clone());
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                here.push(Step::Key(k.clone()));
                paths(x, here, out);
                here.pop();
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                here.push(Step::At(i));
                paths(x, here, out);
                here.pop();
            }
        }
        _ => {}
    }
}

fn at_mut<'a>(v: &'a mut Value, path: &[Step]) -> Option<&'a mut Value> {
    let mut cur = v;
    for step in path {
        cur = match step {
            Step::Key(k) => cur.get_mut(k.as_str())?,
            Step::At(i) => cur.get_mut(*i)?,
        };
    }
    Some(cur)
}

/// A value of the wrong kind, an extreme, a name the server knows, or a deep one.
fn odd(rng: &mut Rng) -> Value {
    let mut deep = json!(1);
    for _ in 0..30 {
        deep = json!([deep]);
    }
    rng.pick(&[
        json!(null), json!(true), json!(0), json!(1), json!(-1), json!(4_294_967_296u64), json!(u64::MAX), json!(-9_007_199_254_740_993i64), json!(1e308), json!(0.5), json!(""), json!("A".repeat(3000)), json!("PING"), json!("GET-CUSTOMER"),
        json!("get-customer"), json!("call"), json!("auth"), json!("hello"), json!("bye"), json!("ref"), json!("value"), json!([]), json!({}), json!("\u{0}"), json!([null]), json!({"mode": "ref", "json": {}}), deep,
    ])
    .clone()
}

/// The frame as JSON with a value or two replaced by an odd one, removed, or copied from a sibling: frames that stay well formed and
/// so reach what the server does with them, not only its parser.
fn mutate_json(rng: &mut Rng, seed: &str) -> String {
    let mut v: Value = serde_json::from_str(seed).expect("a seed is JSON");
    for _ in 0..1 + rng.below(2) {
        let mut all = Vec::new();
        paths(&v, &mut Vec::new(), &mut all);
        // The root (the whole frame) is replaced now and then; the members are what meet the server's rules.
        let path = if all.len() > 1 && rng.below(20) != 0 { all[1 + rng.below(all.len() as u64 - 1) as usize].clone() } else { all[0].clone() };
        let new = odd(rng);
        match (path.split_last(), rng.below(4)) {
            (None, _) => v = new,
            (Some((last, parent)), 0) => {
                if let Some(p) = at_mut(&mut v, parent) {
                    match (p, last) {
                        (Value::Object(m), Step::Key(k)) => {
                            m.remove(k.as_str());
                        }
                        (Value::Array(a), Step::At(i)) if *i < a.len() => {
                            a.remove(*i);
                        }
                        _ => {}
                    }
                }
            }
            (Some(_), _) => {
                if let Some(slot) = at_mut(&mut v, &path) {
                    *slot = new;
                }
            }
        }
    }
    v.to_string()
}

#[derive(Default)]
struct Tally {
    frames: u64,
    answers: u64,
    replies_by: std::collections::BTreeMap<String, u64>,
    errors_by: std::collections::BTreeMap<u32, u64>,
    /// What the refusals say, from the first words of the message: which rule turned a frame away.
    said: std::collections::BTreeMap<String, u64>,
    closes_by: std::collections::BTreeMap<u16, u64>,
    reconnects: u64,
    sentinel_answers: u64,
}

/// Judge one frame the server sent: it parses as a server frame, and an error in it is named and the backend's own fault never.
fn judge_frame(text: &str, t: &mut Tally) -> Value {
    let v: Value = serde_json::from_str(text).unwrap_or_else(|e| panic!("the server sent something that is not JSON ({e}): {text}"));
    if let Err(e) = proto::parse_server(text) {
        panic!("the server sent a frame the client's own parser refuses ({e:?}): {text}");
    }
    let kind = v["t"].as_str().unwrap_or("").to_owned();
    *t.replies_by.entry(kind.clone()).or_default() += 1;
    if kind == "error" {
        let class = v["class"].as_str().unwrap_or("");
        assert!(ErrorClass::ALL.iter().any(|c| c.wire() == class), "an error names a class the wire documents: {text}");
        let code = v["code"].as_u64().unwrap_or(0) as u32;
        assert!(code > 0, "an error carries a code: {text}");
        // A refusal says what happened; it does not repeat the sender's text at length (a reason is cut at 400 characters).
        assert!(v["message"].as_str().is_some_and(|m| !m.is_empty() && m.chars().count() <= 401), "an error carries a short message: {text}");
        if class != "application" {
            assert!(v["detail"].as_str().is_none_or(|d| d.chars().count() <= 401), "an error's detail is short: {text}");
        }
        assert_ne!(code, codes::INTERNAL, "a frame from a stranger caused a fault of the backend itself: {text}");
        assert_ne!(code, codes::PROGRAM_FAULT, "a frame from a stranger made a program fault: {text}");
        *t.errors_by.entry(code).or_default() += 1;
        let said: String = v["message"].as_str().unwrap_or("").chars().take(34).collect();
        *t.said.entry(said).or_default() += 1;
    }
    v
}

async fn reconnect(rig: &Rig, token: &str, t: &mut Tally) -> Raw {
    t.reconnects += 1;
    ready(rig, token).await
}

async fn mutated_frames(rig: &Rig, n: u64, t: &mut Tally) {
    let mut rng = Rng(SEED);
    let seeds = seeds();
    let token = token(rig);
    let mut c = ready(rig, &token).await;
    for i in 0..n {
        let seed = rng.pick(&seeds).clone();
        // Two frames in three stay well formed JSON and meet the server's rules; one in three is broken at the byte.
        let text = if rng.below(3) != 0 { mutate_json(&mut rng, &seed) } else { mutate(&mut rng, &seed) };
        t.frames += 1;
        let id = SENTINEL_ID + i;
        // The mutated frame, then a call that is known to be answerable.
        let sent = send(&mut c, &text).await && send(&mut c, &ping_call(id)).await;
        if !sent {
            c = reconnect(rig, &token, t).await;
            continue;
        }
        loop {
            match recv(&mut c, Duration::from_secs(5)).await {
                Got::Msg(m) => {
                    t.answers += 1;
                    let v = judge_frame(&m, t);
                    if v["id"] == id && matches!(v["t"].as_str(), Some("reply" | "error")) {
                        t.sentinel_answers += 1;
                        break;
                    }
                }
                Got::Closed(Some(code)) => {
                    *t.closes_by.entry(code).or_default() += 1;
                    c = reconnect(rig, &token, t).await;
                    break;
                }
                Got::Closed(None) => panic!("the connection was dropped without a close frame after {text:?}"),
                Got::Quiet => panic!("the server went silent for 5 s after {text:?} and a call that is known to be answerable"),
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn mutated_frames_never_break_the_dispatch_and_nothing_secret_reaches_the_log() {
    let started = Instant::now();
    let rig = new_rig("fuzz-dispatch", "max_frame_bytes = 4096\nconnection_rate_per_sec = 100000\nconnection_burst = 100000\nuser_rate_per_sec = 100000\nuser_burst = 100000\nmax_calls_per_connection = 64\n");
    let mut t = Tally::default();
    let n = MUTATED * iters();
    mutated_frames(&rig, n, &mut t).await;
    still_answers(&rig).await;
    no_panics();

    // The planted strings never reached the log.
    let lines = rig.sink.lines();
    let log = lines.join("\n");
    let mut needles: Vec<(&str, &str)> = vec![("the token", SENT_TOKEN), ("the password that fits", SENT_PW_FIT), ("the password that does not", SENT_PW_LONG)];
    for (i, part) in SENT_TOKEN.split('.').enumerate() {
        needles.push((["the token's header", "the token's payload", "the token's signature"][i], part));
    }
    for (what, needle) in &needles {
        assert!(!log.contains(needle), "{what} reached the log");
    }
    let errors: u64 = t.errors_by.values().sum();
    let mut said: Vec<(&String, &u64)> = t.said.iter().collect();
    said.sort_by(|a, b| b.1.cmp(a.1));
    println!("  what the refusals said (first words, most common first):");
    for (m, n) in said.iter().take(14) {
        println!("    {n:>5}  {m}");
    }
    println!(
        "\n  fuzz_dispatch mutated frames (PR_FUZZ_ITERS={}): {} frames from {} seeds, each followed by an answerable call ({} answered); {} server frames judged ({}); {} errors by code {:?}; closes by code {:?}; {} reconnects; the log ({} records, {} bytes) holds none of {} planted strings; 0 panics; {:.2} s",
        iters(),
        t.frames,
        seeds().len(),
        t.sentinel_answers,
        t.answers,
        t.replies_by.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", "),
        errors,
        t.errors_by,
        t.closes_by,
        t.reconnects,
        lines.len(),
        log.len(),
        needles.len(),
        started.elapsed().as_secs_f64()
    );
    rig.server.stop();
}

// ── 2. the transport ────────────────────────────────────────────────────────

const PROTOCOL_ERROR: u16 = 1002;
const POLICY: u16 = 1008;
const TOO_BIG: u16 = 1009;
const NORMAL: u16 = 1000;
const AWAY: u16 = 1001;

fn name(code: u16) -> &'static str {
    match code {
        1000 => "normal (bye)",
        1001 => "going away",
        1002 => "protocol error",
        1007 => "invalid data",
        1008 => "policy violation",
        1009 => "message too big",
        _ => "other",
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn every_way_to_break_the_transport_ends_with_the_close_code_that_names_it() {
    let started = Instant::now();
    let rig = new_rig("fuzz-transport", "max_frame_bytes = 4096\nhandshake_timeout_ms = 800\nconnection_rate_per_sec = 100000\nconnection_burst = 100000\n");
    let good = token(&rig);
    let mut seen: Vec<(String, u16)> = Vec::new();
    let wait = Duration::from_secs(5);

    macro_rules! expect_close {
        ($label:expr, $c:expr, $code:expr) => {{
            let got = closed_with(&mut $c, wait).await;
            assert_eq!(got, Some(Some($code)), "{}: the connection ends with close code {} ({}), got {:?}", $label, $code, name($code), got);
            seen.push(($label.to_string(), $code));
        }};
    }

    // After the exchange, a plain text message: a peer that skipped the sealing.
    let mut c = ready(&rig, &good).await;
    c.send_plain(r#"{"t":"bye"}"#).await;
    expect_close!("a plain text after the exchange", c, PROTOCOL_ERROR);

    // After the exchange, a binary frame that is noise.
    let mut c = ready(&rig, &good).await;
    c.ws.send(Message::Binary(vec![7u8; 60].into())).await.unwrap();
    expect_close!("a binary frame of noise after the exchange", c, PROTOCOL_ERROR);

    // After the exchange, a sealed frame cut short, so short that it cannot hold a tag.
    let mut c = ready(&rig, &good).await;
    c.ws.send(Message::Binary(vec![1u8; 9].into())).await.unwrap();
    expect_close!("a sealed frame cut short", c, PROTOCOL_ERROR);

    // A sealed frame with one bit changed.
    let mut c = ready(&rig, &good).await;
    let mut frame = c.sender.as_mut().unwrap().seal(&ping_call(5)).unwrap();
    let last = frame.len() - 1;
    frame[last] ^= 0x01;
    c.ws.send(Message::Binary(frame.into())).await.unwrap();
    expect_close!("a sealed frame with a bit flipped", c, PROTOCOL_ERROR);

    // A sealed frame sent twice: the second does not open (the counter has moved on).
    let mut c = ready(&rig, &good).await;
    let frame = c.sender.as_mut().unwrap().seal(&ping_call(6)).unwrap();
    c.ws.send(Message::Binary(frame.clone().into())).await.unwrap();
    match recv(&mut c, wait).await {
        Got::Msg(m) => assert!(m.contains("\"reply\""), "the first copy is answered: {m}"),
        _ => panic!("the first copy of the frame is answered"),
    }
    c.ws.send(Message::Binary(frame.into())).await.unwrap();
    expect_close!("a sealed frame replayed", c, PROTOCOL_ERROR);

    // Before the exchange, a binary frame.
    let mut c = Raw::connect(&rig, Some(ORIGIN)).await.expect("connect");
    c.ws.send(Message::Binary(vec![1u8; 40].into())).await.unwrap();
    expect_close!("a binary frame before the hello", c, PROTOCOL_ERROR);

    // A hello the backend cannot accept: a protocol it does not speak.
    let mut c = Raw::connect(&rig, Some(ORIGIN)).await.expect("connect");
    let h = c.hello_at(1, "1.0.0").await;
    assert_ne!(h["accepted"], "ok", "{h}");
    expect_close!("a hello that is refused (a protocol the backend does not speak)", c, PROTOCOL_ERROR);

    // A hello whose key is not a key.
    let mut c = Raw::connect(&rig, Some(ORIGIN)).await.expect("connect");
    c.send(json!({"t": "hello", "protocol": cobolt_wire::contract::PROTOCOL, "contract": "1.0.0", "client": "fuzz", "key": "not-a-key", "nonce": "x"})).await;
    expect_close!("a hello whose key is not a key", c, PROTOCOL_ERROR);

    // A message over max_frame_bytes: the limit is named, then the connection ends.
    let mut c = ready(&rig, &good).await;
    let big = format!(r#"{{"t":"call","id":1,"program":"PING","args":[{{"mode":"value","json":"{}"}}]}}"#, "x".repeat(8000));
    assert!(send(&mut c, &big).await);
    match recv(&mut c, wait).await {
        Got::Msg(m) => {
            let v: Value = serde_json::from_str(&m).unwrap();
            assert_eq!((v["class"].as_str(), v["code"].as_u64()), (Some("limit"), Some(u64::from(codes::FRAME_TOO_LARGE))), "the limit is named first: {m}");
        }
        _ => panic!("the oversize message is answered with the limit before the close"),
    }
    expect_close!("a message over max_frame_bytes", c, TOO_BIG);

    // Five tokens that do not sign in.
    let mut c = Raw::connect(&rig, Some(ORIGIN)).await.expect("connect");
    assert_eq!(c.hello("1.0.0").await["accepted"], "ok");
    for i in 0..5 {
        c.send(json!({"t": "auth", "token": format!("{SENT_TOKEN}{i}")})).await;
    }
    expect_close!("five failed sign-ins", c, POLICY);

    // A connection that never says hello.
    let mut c = Raw::connect(&rig, Some(ORIGIN)).await.expect("connect");
    let t0 = Instant::now();
    expect_close!("a connection that never says hello (handshake_timeout_ms = 800)", c, POLICY);
    let waited = t0.elapsed();
    assert!(waited >= Duration::from_millis(700) && waited < Duration::from_secs(4), "the wait is the configured one: {waited:?}");

    // A client that leaves.
    let mut c = ready(&rig, &good).await;
    assert!(send(&mut c, r#"{"t":"bye"}"#).await);
    expect_close!("bye", c, NORMAL);
    // A bye before the hello is a leaving client too, not a refused exchange.
    let mut c = Raw::connect(&rig, Some(ORIGIN)).await.expect("connect");
    c.send_plain(r#"{"t":"bye"}"#).await;
    expect_close!("bye before the hello", c, NORMAL);

    // What does NOT end the connection: text that is not a message, an auth or a call before the hello, a second hello.
    let mut c = Raw::connect(&rig, Some(ORIGIN)).await.expect("connect");
    for text in ["", "not json", "[1,2,3]", "{}", r#"{"t":"auth","token":"x"}"#, r#"{"t":"call","id":1,"program":"PING","args":[]}"#] {
        c.send_plain(text).await;
        match recv(&mut c, wait).await {
            Got::Msg(m) => assert_eq!(serde_json::from_str::<Value>(&m).unwrap()["t"], "error", "{text:?} is answered with a named error: {m}"),
            _ => panic!("{text:?} before the hello is answered, not met with a close"),
        }
    }
    assert_eq!(c.hello("1.0.0").await["accepted"], "ok", "after the refusals the exchange still works");
    let mut c = ready(&rig, &good).await;
    assert!(send(&mut c, &format!(r#"{{"t":"hello","protocol":{},"contract":"1.0.0","client":"fuzz","key":"{}=","nonce":"{}"}}"#, cobolt_wire::contract::PROTOCOL, "A".repeat(43), "B".repeat(22))).await);
    match recv(&mut c, wait).await {
        Got::Msg(m) => {
            let v: Value = serde_json::from_str(&m).unwrap();
            assert_eq!((v["t"].as_str(), v["code"].as_u64()), (Some("error"), Some(u64::from(codes::BAD_MESSAGE))), "a second hello is refused as a message: {m}");
        }
        _ => panic!("a second hello is answered"),
    }
    assert!(send(&mut c, &ping_call(2)).await);
    match recv(&mut c, wait).await {
        Got::Msg(m) => assert!(m.contains("\"reply\""), "the channel is as it was after the refused second hello: {m}"),
        _ => panic!("the connection survives a second hello"),
    }

    still_answers(&rig).await;

    // The backend shuts down: an open, signed-in connection is told it is going away.
    let mut c = ready(&rig, &good).await;
    rig.server.stop();
    expect_close!("the backend shutting down", c, AWAY);
    no_panics();

    println!(
        "\n  fuzz_dispatch transport: {} ways to end a connection, each with the close code that names it: {}; the connection that never says hello was closed after {} ms; 0 panics; {:.2} s",
        seen.len(),
        seen.iter().map(|(l, c)| format!("{l} -> {c} ({})", name(*c))).collect::<Vec<_>>().join("; "),
        waited.as_millis(),
        started.elapsed().as_secs_f64()
    );
}

// ── 3. floods ───────────────────────────────────────────────────────────────

/// Read whatever the server has sent, without waiting for more; the frames, and whether the connection is still there.
async fn drain(c: &mut Raw) -> (Vec<String>, bool) {
    let mut frames = Vec::new();
    loop {
        match recv(c, Duration::from_millis(2)).await {
            Got::Msg(m) => frames.push(m),
            Got::Quiet => return (frames, true),
            Got::Closed(_) => return (frames, false),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn floods_are_absorbed_and_the_slots_come_back() {
    let started = Instant::now();
    let rig = new_rig("fuzz-floods", "max_frame_bytes = 4096\nmax_connections = 40\nhandshake_timeout_ms = 5000\nconnection_rate_per_sec = 100000\nconnection_burst = 100000\nuser_rate_per_sec = 100000\nuser_burst = 100000\n");
    let good = token(&rig);

    // Twenty thousand ping frames on a signed-in connection, the pongs read as they come, then a call.
    let mut c = ready(&rig, &good).await;
    let t0 = Instant::now();
    let mut batches = 0usize;
    for _ in 0..40 {
        for _ in 0..500 {
            c.ws.feed(Message::Ping(vec![0u8; 8].into())).await.expect("a ping frame");
        }
        c.ws.flush().await.expect("flush");
        let (_, alive) = drain(&mut c).await;
        assert!(alive, "the connection survives the ping flood");
        batches += 1;
    }
    let ping_ms = t0.elapsed().as_millis();
    let r = c.call(1, "PING", &[], true).await;
    assert_eq!(r["returning"]["REPLY"], "pong", "the connection answers after 20,000 pings: {r}");

    // A thousand hellos on an agreed channel: each refused as a message, none re-keys, the call after is answered.
    let hello = format!(r#"{{"t":"hello","protocol":{},"contract":"1.0.0","client":"fuzz","key":"{}=","nonce":"{}"}}"#, cobolt_wire::contract::PROTOCOL, "A".repeat(43), "B".repeat(22));
    let t0 = Instant::now();
    let mut refused = 0usize;
    for i in 0..1000 {
        assert!(send(&mut c, &hello).await, "the connection takes a thousand hellos (it ended at {i})");
        if i % 50 == 49 {
            let (frames, alive) = drain(&mut c).await;
            assert!(alive, "the connection survives the hello flood");
            refused += frames.iter().filter(|m| m.contains("\"error\"") && m.contains("4001")).count();
        }
    }
    let (frames, _) = drain(&mut c).await;
    refused += frames.iter().filter(|m| m.contains("\"error\"") && m.contains("4001")).count();
    let hello_ms = t0.elapsed().as_millis();
    assert!(refused >= 900, "the hellos were refused as the message they are ({refused} of 1000 seen)");
    assert!(send(&mut c, &ping_call(99)).await);
    let mut answered = false;
    for _ in 0..50 {
        if let Got::Msg(m) = recv(&mut c, Duration::from_secs(2)).await {
            if m.contains("\"id\":99") {
                assert!(m.contains("\"reply\""), "{m}");
                answered = true;
                break;
            }
        }
    }
    assert!(answered, "a call is answered after the hello flood");
    drop(c);

    // A hundred and twenty connections at a server that accepts forty: none says hello, and the slots come back.
    let addr = rig.server.addr;
    let pem = rig.cert.cert_pem.clone();
    let t0 = Instant::now();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..120 {
        let pem = pem.clone();
        tasks.spawn(async move { Raw::connect_to(addr, &pem, Some(ORIGIN)).await });
    }
    let mut held: Vec<Raw> = Vec::new();
    let mut refused_conn = 0usize;
    while let Some(done) = tasks.join_next().await {
        match done.expect("the connection task") {
            Ok(c) => held.push(c),
            Err(_) => refused_conn += 1,
        }
    }
    let storm_ms = t0.elapsed().as_millis();
    assert!(held.len() <= 40, "no more than max_connections (40) are held: {}", held.len());
    assert!(held.len() >= 30, "the slots were used: {}", held.len());
    assert_eq!(held.len() + refused_conn, 120);
    let kept = held.len();
    drop(held);
    // The slots come back: a good client is served within a few seconds of the storm ending.
    let mut served = false;
    for _ in 0..50 {
        if let Ok(mut c) = Raw::connect(&rig, Some(ORIGIN)).await {
            if c.hello("1.0.0").await["accepted"] == "ok" {
                served = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(served, "the slots came back after the storm");
    still_answers(&rig).await;
    no_panics();
    println!(
        "\n  fuzz_dispatch floods: 20,000 pings in {batches} batches ({ping_ms} ms), connection answered; 1000 hellos on an agreed channel ({hello_ms} ms), {refused} seen refused as 4001, none re-keyed, the next call answered; 120 connections at max_connections = 40: {kept} held, {refused_conn} refused ({storm_ms} ms), slots back and a good client served; 0 panics; {:.2} s",
        started.elapsed().as_secs_f64()
    );
    rig.server.stop();
}
