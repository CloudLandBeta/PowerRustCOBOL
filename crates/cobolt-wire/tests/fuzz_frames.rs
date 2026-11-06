// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Fuzzing of the frame parsers (WASM-013 R15, T19; AC11, frames).
//!
//! `proto::parse_client` is the first thing the backend does with a byte that came off the network, and
//! `proto::parse_server` is the first thing the browser does with one that came back. Neither may panic,
//! overflow the stack or run away on any text, and each refusal must be a **named** one: the client side a
//! `Validation` / `BAD_MESSAGE` error with a message, the server side one of the three phrases the player's
//! console and the tests recognise.
//!
//! The inputs are the frames of the recorded conversation (`wire.v1.json`), one hand-written frame of each of the
//! eleven client kinds and twelve server kinds (the test fails if one stops parsing, so the seeds stay valid), and
//! what is made of them:
//!
//! * **random mutations** — a bit flipped, a byte changed, a range deleted, doubled or inserted, the text cut, a
//!   number replaced by an extreme (past `u64`, negative, `1e999`), a string by a very long one, the `t` of the frame
//!   by another kind, a value wrapped in nesting deeper than the JSON reader allows, a multi-byte character put in the
//!   middle of a name;
//! * **exhaustive single-byte changes** — every position of every seed, each of 24 bytes that matter to JSON;
//! * **every cut** of every seed at every character boundary: a strict prefix of a frame is never a frame;
//! * **invalid UTF-8**: the bytes that are not text are refused where text becomes a string (the WebSocket layer
//!   does it for a text frame), and the lossy reading of them, which has U+FFFD in it, is parsed too;
//! * **oversize and depth**: 8 MiB of text, 1,000,000 open brackets.
//!
//! A fixed seed makes a failure reproduce. `PR_FUZZ_ITERS=<n>` multiplies the random work (default 1) for a longer
//! run. The run prints its iterations, its outcomes and its seconds.

use std::time::Instant;

use cobolt_wire::error::{codes, ErrorClass};
use cobolt_wire::proto::{parse_client, parse_server, ClientFrame, ServerFrame};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn chance(&mut self, one_in: u64) -> bool {
        self.below(one_in) == 0
    }
}

/// The multiplier of the random work.
fn iters() -> u64 {
    std::env::var("PR_FUZZ_ITERS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(1).max(1)
}

const SEED: u64 = 0x5EED_F00D_CAFE_1234;
/// Random mutations per direction at `PR_FUZZ_ITERS=1`.
const RANDOM: u64 = 100_000;

// ── the seeds ───────────────────────────────────────────────────────────────

const KEY: &str = "AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=";
const NONCE: &str = "AAECAwQFBgcICQoLDA0ODw==";
const SESSION: &str = "0123456789abcdef0123456789abcdef";
const SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// One valid frame of every client kind.
fn client_seeds() -> Vec<(&'static str, String)> {
    vec![
        ("hello", format!(r#"{{"t":"hello","protocol":2,"contract":"1.0.0","client":"prc-wasm/2.0.0","key":"{KEY}","nonce":"{NONCE}","session":"{SESSION}"}}"#)),
        ("auth", r#"{"t":"auth","token":"eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJhbmEifQ.c2ln"}"#.into()),
        ("call", r#"{"t":"call","id":7,"program":"GET-CUSTOMER","args":[{"mode":"value","json":{"ID":"42","BAL":-12.50}},{"mode":"ref","json":[1,2,3]}],"returning":true,"key":"s-00000007","token":"t","timeout_ms":5000}"#.into()),
        ("cancel", r#"{"t":"cancel","id":7}"#.into()),
        ("sub", r#"{"t":"sub","id":3,"program":"PRICES","args":[{"mode":"value","json":{"SYM":"ABC"}}]}"#.into()),
        ("unsub", r#"{"t":"unsub","id":3}"#.into()),
        ("put-begin", format!(r#"{{"t":"put-begin","id":9,"slot":"AVATAR","name":"ana.png","size":2048,"mime":"image/png","sha256":"{SHA}","resume":"u-1"}}"#)),
        ("put-chunk", r#"{"t":"put-chunk","id":9,"offset":0,"data":"iVBORw0KGgo="}"#.into()),
        ("put-end", r#"{"t":"put-end","id":9}"#.into()),
        ("get-file", r#"{"t":"get-file","id":11,"file":"f-0001"}"#.into()),
        ("bye", r#"{"t":"bye"}"#.into()),
    ]
}

/// One valid frame of every server kind.
fn server_seeds() -> Vec<(&'static str, String)> {
    vec![
        ("hello", format!(r#"{{"t":"hello","protocol":2,"protocol_min":2,"contract":"1.0.0","accepted":"warn","message":"update","update_to":"1.1.0","server":"s/2.0.0","max_frame_bytes":1048576,"key":"{KEY}","nonce":"{NONCE}","session":"new"}}"#)),
        ("auth-ok", r#"{"t":"auth-ok","sub":"u-1","expires_in":300}"#.into()),
        ("reply", r#"{"t":"reply","id":7,"args":[{"ID":"42","BAL":-12.50},null],"returning":{"OK":1},"warnings":["w"]}"#.into()),
        ("error", r#"{"t":"error","id":7,"class":"validation","code":4000,"message":"bad","detail":"args[0]","may_have_run":false}"#.into()),
        ("progress", r#"{"t":"progress","id":7,"percent":40,"message":"half"}"#.into()),
        ("event", r#"{"t":"event","sub":3,"seq":12,"data":{"SYM":"ABC","PX":10.25}}"#.into()),
        ("subscribed", r#"{"t":"subscribed","id":3}"#.into()),
        ("sub-end", r#"{"t":"sub-end","sub":3}"#.into()),
        ("unsubscribed", r#"{"t":"unsubscribed","id":3}"#.into()),
        ("put-ready", r#"{"t":"put-ready","id":9,"upload":"u-1","have":0}"#.into()),
        ("put-done", r#"{"t":"put-done","id":9,"file":"f-0001","size":2048}"#.into()),
        ("file-url", format!(r#"{{"t":"file-url","id":11,"path":"/dl/f-0001","expires_in":60,"size":2048,"sha256":"{SHA}"}}"#)),
    ]
}

/// The frames of the recorded conversation: (is it a client frame, its text).
fn transcript() -> Vec<(bool, String)> {
    let text = include_str!("../../../specs/WASM-014-backend-contract-and-evolution/vectors/wire.v1.json");
    let v: serde_json::Value = serde_json::from_str(text).expect("the transcript is JSON");
    v["frames"].as_array().expect("frames").iter().map(|f| (f["dir"] == "c2s", f["frame"].to_string())).collect()
}

fn kind_of_client(f: &ClientFrame) -> &'static str {
    match f {
        ClientFrame::Hello(_) => "hello",
        ClientFrame::Auth(_) => "auth",
        ClientFrame::Call(_) => "call",
        ClientFrame::Cancel(_) => "cancel",
        ClientFrame::Subscribe(_) => "sub",
        ClientFrame::Unsubscribe(_) => "unsub",
        ClientFrame::PutBegin(_) => "put-begin",
        ClientFrame::PutChunk(_) => "put-chunk",
        ClientFrame::PutEnd(_) => "put-end",
        ClientFrame::GetFile(_) => "get-file",
        ClientFrame::Bye(_) => "bye",
    }
}

fn kind_of_server(f: &ServerFrame) -> &'static str {
    match f {
        ServerFrame::Hello(_) => "hello",
        ServerFrame::AuthOk(_) => "auth-ok",
        ServerFrame::Reply(_) => "reply",
        ServerFrame::Error(_) => "error",
        ServerFrame::Progress(_) => "progress",
        ServerFrame::Event(_) => "event",
        ServerFrame::Subscribed(_) => "subscribed",
        ServerFrame::SubEnd(_) => "sub-end",
        ServerFrame::Unsubscribed(_) => "unsubscribed",
        ServerFrame::PutReady(_) => "put-ready",
        ServerFrame::PutDone(_) => "put-done",
        ServerFrame::FileUrl(_) => "file-url",
    }
}

// ── the oracles ─────────────────────────────────────────────────────────────

/// What a parse did: accepted, or refused with its (checked) reason.
#[derive(Default)]
struct Tally {
    accepted: u64,
    refused: u64,
    not_utf8: u64,
    lossy: u64,
}

/// Parse `bytes` as a client frame and check the outcome. Invalid UTF-8 never reaches the parser (the WebSocket layer
/// refuses a text frame that is not UTF-8); its lossy reading does.
fn check_client(bytes: &[u8], t: &mut Tally) {
    match std::str::from_utf8(bytes) {
        Ok(text) => client_outcome(text, t),
        Err(_) => {
            t.not_utf8 += 1;
            t.lossy += 1;
            client_outcome(&String::from_utf8_lossy(bytes), t);
        }
    }
}

fn client_outcome(text: &str, t: &mut Tally) {
    match parse_client(text) {
        Ok(frame) => {
            t.accepted += 1;
            // An accepted frame is the kind its own `t` names.
            assert!(text.trim_start().starts_with('{'), "a frame is a JSON object, but this was taken for one: {}", clip(text));
            assert_eq!(kind_in(text).as_deref(), Some(kind_of_client(&frame)), "the frame was read as another kind than it says: {}", clip(text));
        }
        Err(e) => {
            t.refused += 1;
            assert_eq!((e.class, e.code), (ErrorClass::Validation, codes::BAD_MESSAGE), "a refused frame is a Validation / BAD_MESSAGE error: {e:?} for {}", clip(text));
            assert!(!e.message.is_empty(), "the refusal is named: {}", clip(text));
            // A reason quotes what serde met, and is cut: a stranger's megabyte does not come back as a megabyte (found by the dispatch fuzz).
            assert!(e.message.chars().count() <= 201, "the refusal is bounded: {} characters for {} bytes", e.message.chars().count(), text.len());
        }
    }
}

fn check_server(bytes: &[u8], t: &mut Tally) {
    match std::str::from_utf8(bytes) {
        Ok(text) => server_outcome(text, t),
        Err(_) => {
            t.not_utf8 += 1;
            t.lossy += 1;
            server_outcome(&String::from_utf8_lossy(bytes), t);
        }
    }
}

fn server_outcome(text: &str, t: &mut Tally) {
    match parse_server(text) {
        Ok(frame) => {
            t.accepted += 1;
            assert!(text.trim_start().starts_with('{'), "a frame is a JSON object, but this was taken for one: {}", clip(text));
            assert_eq!(kind_in(text).as_deref(), Some(kind_of_server(&frame)), "the frame was read as another kind than it says: {}", clip(text));
        }
        Err(e) => {
            t.refused += 1;
            assert!(e.starts_with("not a message") || e.starts_with("a ") || e.starts_with("unknown message kind"), "a refusal the player does not know how to read: {e} for {}", clip(text));
            assert!(e.chars().count() <= 201, "the refusal is bounded: {} characters for {} bytes", e.chars().count(), text.len());
        }
    }
}

/// The `t` of a frame, read alone. (Not through a JSON value: a payload is kept as the text it was written as and may
/// hold a number a value cannot, `1e999`, or nesting deeper than a value is read to; the mapping bounds it later.)
fn kind_in(text: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Kind {
        t: String,
    }
    serde_json::from_str::<Kind>(text).ok().map(|k| k.t)
}

/// The start of `text`, for a failure message.
fn clip(text: &str) -> String {
    text.chars().take(160).collect()
}

// ── the mutations ───────────────────────────────────────────────────────────

const EXTREMES: &[&str] = &["18446744073709551616", "-1", "1e999", "0.5", "99999999999999999999999999", "4294967296", "-0", "1E-400", "0x10"];
const KINDS: &[&str] = &[
    "hello", "auth", "call", "cancel", "sub", "unsub", "put-begin", "put-chunk", "put-end", "get-file", "bye", "auth-ok", "reply", "error", "progress", "event", "subscribed", "sub-end", "unsubscribed",
    "put-ready", "put-done", "file-url", "", "CALL",
];
const MULTIBYTE: &[&str] = &["é", "日", "😀", "\u{FEFF}"];
/// The bytes that matter to JSON, and two that are not text.
const SPECIAL: &[u8] = b"\"{}[],:\\/-+.0eE9tfn \t\n\x00\x7f\xff";

fn mutate(rng: &mut Rng, seed: &str) -> Vec<u8> {
    let mut b = seed.as_bytes().to_vec();
    for _ in 0..1 + rng.below(4) {
        if b.is_empty() {
            b.push(b'{');
        }
        let at = rng.below(b.len() as u64) as usize;
        match rng.below(14) {
            0 => b[at] ^= 1 << rng.below(8),
            1 => b[at] = rng.next() as u8,
            2 => b[at] = SPECIAL[rng.below(SPECIAL.len() as u64) as usize],
            3 => {
                let n = (1 + rng.below(8) as usize).min(b.len() - at);
                b.drain(at..at + n);
            }
            4 => {
                let n = 1 + rng.below(8) as usize;
                let bytes: Vec<u8> = (0..n).map(|_| rng.next() as u8).collect();
                b.splice(at..at, bytes);
            }
            5 => {
                let n = (1 + rng.below(16) as usize).min(b.len() - at);
                let piece = b[at..at + n].to_vec();
                b.splice(at..at, piece);
            }
            6 => b.truncate(at),
            7 => {
                // A number (a run of digits) becomes an extreme.
                if let Some(start) = b.iter().skip(at).position(|c| c.is_ascii_digit()).map(|p| p + at) {
                    let end = start + b[start..].iter().take_while(|c| c.is_ascii_digit() || matches!(**c, b'.' | b'e' | b'E')).count();
                    b.splice(start..end, EXTREMES[rng.below(EXTREMES.len() as u64) as usize].bytes());
                }
            }
            8 => {
                // A string becomes a very long one (up to 70 KB: past the 64 KiB a name or a token is ever allowed).
                if let Some(start) = b.iter().skip(at).position(|c| *c == b'"').map(|p| p + at + 1) {
                    let n = if rng.chance(8) { 70_000 } else { 1 + rng.below(300) as usize };
                    b.splice(start..start, std::iter::repeat(b'A').take(n));
                }
            }
            9 => {
                // The kind becomes another.
                let kind = KINDS[rng.below(KINDS.len() as u64) as usize];
                let text = String::from_utf8_lossy(&b).into_owned();
                if let Some(p) = text.find("\"t\":\"") {
                    let from = p + 5;
                    if let Some(len) = text[from..].find('"') {
                        b = format!("{}{}{}", &text[..from], kind, &text[from + len..]).into_bytes();
                    }
                }
            }
            10 => {
                // A value is wrapped in nesting, sometimes deeper than the reader allows.
                let depth = if rng.chance(3) { 130 + rng.below(100) as usize } else { 1 + rng.below(20) as usize };
                let open = if rng.chance(2) { "[" } else { r#"{"a":"# };
                let close = if open == "[" { "]" } else { "}" };
                let mut w = open.repeat(depth).into_bytes();
                w.extend_from_slice(b"1");
                w.extend_from_slice(close.repeat(depth).as_bytes());
                b.splice(at..at, w);
            }
            11 => {
                let m = MULTIBYTE[rng.below(MULTIBYTE.len() as u64) as usize];
                b.splice(at..at, m.bytes());
            }
            12 => {
                // Half of a multi-byte character: invalid UTF-8 by construction.
                b.splice(at..at, [0xC3u8]);
            }
            _ => {
                // Two bytes swap.
                let other = rng.below(b.len() as u64) as usize;
                b.swap(at, other);
            }
        }
    }
    b
}

// ── the runs ────────────────────────────────────────────────────────────────

fn seeds_for(client: bool) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = if client { client_seeds() } else { server_seeds() }.into_iter().map(|(k, t)| (k.to_owned(), t)).collect();
    v.extend(transcript().into_iter().filter(|(c, _)| *c == client).map(|(_, t)| ("recorded".to_owned(), t)));
    v
}

#[test]
fn every_seed_is_a_valid_frame_of_its_kind_and_every_kind_has_one() {
    let mut kinds = std::collections::BTreeSet::new();
    for (kind, text) in client_seeds() {
        let f = parse_client(&text).unwrap_or_else(|e| panic!("the {kind} seed does not parse: {e:?}"));
        assert_eq!(kind_of_client(&f), kind);
        kinds.insert(format!("c:{kind}"));
    }
    for (kind, text) in server_seeds() {
        let f = parse_server(&text).unwrap_or_else(|e| panic!("the {kind} seed does not parse: {e}"));
        assert_eq!(kind_of_server(&f), kind);
        kinds.insert(format!("s:{kind}"));
    }
    assert_eq!(kinds.len(), 11 + 12, "all eleven client kinds and twelve server kinds have a seed");
    let (c, s) = transcript().iter().fold((0, 0), |(c, s), (client, _)| if *client { (c + 1, s) } else { (c, s + 1) });
    assert!(c >= 8 && s >= 8, "the recorded conversation has frames for both sides");
}

#[test]
fn random_and_mutated_frames_never_panic_and_every_refusal_is_named() {
    let started = Instant::now();
    let n = RANDOM * iters();
    let mut report = Vec::new();
    for client in [true, false] {
        let seeds = seeds_for(client);
        let mut rng = Rng(SEED ^ if client { 1 } else { 2 });
        let mut t = Tally::default();
        for _ in 0..n {
            let seed = &seeds[rng.below(seeds.len() as u64) as usize].1;
            let bytes = mutate(&mut rng, seed);
            if client {
                check_client(&bytes, &mut t);
            } else {
                check_server(&bytes, &mut t);
            }
        }
        report.push(format!(
            "{}: {n} mutated frames from {} seeds, {} accepted, {} refused with a named error ({} not UTF-8, read lossily)",
            if client { "client" } else { "server" },
            seeds.len(),
            t.accepted,
            t.refused,
            t.not_utf8
        ));
        // The run is neither all accepted nor all refused: the mutations reach both sides of the parser.
        assert!(t.accepted > n / 100 && t.refused > n / 10, "the mutations reached both outcomes: {} accepted, {} refused", t.accepted, t.refused);
    }
    println!("\n  fuzz_frames random (PR_FUZZ_ITERS={}): {}; {:.2} s", iters(), report.join("; "), started.elapsed().as_secs_f64());
}

#[test]
fn every_byte_of_every_seed_changed_to_each_byte_that_matters_and_cut_at_every_boundary() {
    let started = Instant::now();
    let (mut changed, mut cuts) = (0u64, 0u64);
    let mut t = Tally::default();
    for client in [true, false] {
        for (_, seed) in seeds_for(client) {
            let bytes = seed.as_bytes();
            // Every position, each of the bytes that matter to JSON.
            for at in 0..bytes.len() {
                for &s in SPECIAL {
                    let mut m = bytes.to_vec();
                    m[at] = s;
                    changed += 1;
                    if client {
                        check_client(&m, &mut t);
                    } else {
                        check_server(&m, &mut t);
                    }
                }
            }
            // Every cut at a character boundary: a strict prefix of a frame is not a frame.
            for cut in 0..seed.len() {
                if !seed.is_char_boundary(cut) {
                    continue;
                }
                cuts += 1;
                let prefix = &seed[..cut];
                let accepted = if client { parse_client(prefix).is_ok() } else { parse_server(prefix).is_ok() };
                assert!(!accepted, "a strict prefix was taken for a frame: {}", clip(prefix));
            }
        }
    }
    println!("\n  fuzz_frames exhaustive: {changed} single-byte changes ({} accepted, {} refused, {} not UTF-8), {cuts} cuts all refused; {:.2} s", t.accepted, t.refused, t.not_utf8, started.elapsed().as_secs_f64());
    assert!(changed > 100_000);
}

#[test]
fn oversize_and_very_deep_text_is_refused_without_exhausting_the_stack_or_the_heap() {
    let started = Instant::now();
    let mut t = Tally::default();
    let eight_mib = 8 * 1024 * 1024;
    let huge_string = format!(r#"{{"t":"call","id":1,"program":"{}"}}"#, "A".repeat(eight_mib));
    let huge_array = format!(r#"{{"t":"call","id":1,"program":"P","args":[{}]}}"#, vec![r#"{"mode":"value","json":1}"#; 200_000].join(","));
    let deep_brackets = "[".repeat(1_000_000);
    let deep_objects = r#"{"a":"#.repeat(200_000);
    let deep_arg = format!(r#"{{"t":"call","id":1,"program":"P","args":[{{"mode":"value","json":{}1{}}}]}}"#, "[".repeat(100_000), "]".repeat(100_000));
    let deep_event = format!(r#"{{"t":"event","sub":1,"seq":1,"data":{}1{}}}"#, "[".repeat(100_000), "]".repeat(100_000));
    let cases: [(&str, &str); 6] = [("an 8 MiB name", &huge_string), ("200,000 arguments", &huge_array), ("1,000,000 open brackets", &deep_brackets), ("200,000 open objects", &deep_objects), ("an argument 100,000 deep", &deep_arg), ("an event 100,000 deep", &deep_event)];
    for (what, text) in cases {
        let t0 = Instant::now();
        client_outcome(text, &mut t);
        server_outcome(text, &mut t);
        println!("  {what}: {} bytes through both parsers in {:.0} ms", text.len(), t0.elapsed().as_secs_f64() * 1000.0);
    }
    // The deep and the malformed are refused; the long name and the long list are the sizes the transport limits, not the parser.
    assert!(t.refused >= 8, "the deep and the malformed texts are refused: {} refusals", t.refused);
    println!("\n  fuzz_frames oversize and depth: 6 texts (largest {} bytes) through both parsers, {} accepted, {} refused; {:.2} s", eight_mib + 40, t.accepted, t.refused, started.elapsed().as_secs_f64());
}

#[test]
fn a_text_that_is_not_a_json_object_is_never_a_frame() {
    // serde's derived readers also accept a struct written as a positional array; a frame is an object, and the
    // seam between two implementations of the wire (the browser's and the backend's) is where a lenient reader hurts.
    let cases = [
        r#"["bye"]"#,
        r#"["call",1,"P"]"#,
        r#"["auth","token"]"#,
        r#"["hello",2,"1.0.0"]"#,
        r#"["error",1,"validation",4000,"m"]"#,
        r#"["reply",1,[]]"#,
        r#""bye""#,
        "1",
        "null",
        "true",
        "[]",
        "{}",
        "",
        "   ",
    ];
    let mut refused = 0;
    for c in cases {
        assert!(parse_client(c).is_err(), "a client frame that is not an object was accepted: {c}");
        assert!(parse_server(c).is_err(), "a server frame that is not an object was accepted: {c}");
        refused += 2;
    }
    println!("\n  fuzz_frames non-objects: {} texts, {refused} refusals (both parsers)", cases.len());
}
