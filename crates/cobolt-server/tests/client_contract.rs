// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The contract end to end (WASM-014 AC1-AC10): the Rust client against a real
//! backend over a real TLS loopback, with a proxy that can cut the connection at
//! the moment a test chooses. Each test names the acceptance criterion it proves.

mod support;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;
use support::*;

use cobolt_server::client::{ClientConfig, StaticToken, StreamItem, WsClient};
use cobolt_wire::call::{ArgMode, ArgPayload, CallObserver, CallRequest, DownloadRequest, Transport, UploadRequest};
use cobolt_wire::error::{codes, CallError, ErrorClass};

const EXPORTS: &str = r#"
[contract]
version = "1.2.0"

[[export]]
program = "PING"
public = true

[[export]]
program = "SAFE-ADD"
idempotent = true

[[export]]
program = "CHARGE"

[[export]]
program = "REJECT"

[[export]]
program = "CRASH"

[[export]]
program = "ADMIN-ONLY"
roles = ["admin"]

[[export]]
program = "LONG"
long_running = true

[[export]]
program = "TICKER"
stream = true

[[export]]
program = "OLD-ONE"
deprecated = { message = "use NEW-ONE", removal = "2.0.0" }

[[export]]
program = "SLOW-A"

[[export]]
program = "FAST-B"

[[export]]
program = "INDEP"
independent = true

[[export]]
program = "STORE-INFO"

[[export]]
program = "OFFER"
"#;

fn ping() -> String {
    "IDENTIFICATION DIVISION.\nPROGRAM-ID. PING.\nDATA DIVISION.\nLINKAGE SECTION.\n01 LK-PONG.\n    05 REPLY PIC X(4).\nPROCEDURE DIVISION RETURNING LK-PONG.\nMAIN.\n    MOVE \"pong\" TO REPLY\n    GOBACK.\n".into()
}

/// A program that records that it ran (one line in `runs`), works for a while, answers.
fn recorder(name: &str, runs: &str, pause_ms: u32) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. {name}.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT OPTIONAL RUNS ASSIGN TO \"{runs}\"\n        ORGANIZATION IS LINE SEQUENTIAL.\nDATA DIVISION.\nFILE SECTION.\nFD RUNS.\n01 RUN-LINE PIC X(20).\nLINKAGE SECTION.\n01 LK-A.\n    05 TAG PIC X(8).\n    05 DONE PIC 9(4).\nPROCEDURE DIVISION USING LK-A.\nMAIN.\n    OPEN EXTEND RUNS\n    MOVE TAG TO RUN-LINE\n    WRITE RUN-LINE\n    CLOSE RUNS\n    COBOL::\"WS-PAUSE\" ( {pause_ms} )\n    MOVE 1 TO DONE\n    GOBACK.\n"
    )
}

fn sources(dir_for_files: &str) -> Vec<(String, String)> {
    let runs = format!("{dir_for_files}/runs.txt");
    let mut v = vec![
        ("ping.cbl".to_string(), ping()),
        ("safe-add.cbl".to_string(), recorder("SAFE-ADD", &runs, 200)),
        ("charge.cbl".to_string(), recorder("CHARGE", &runs, 200)),
        ("slow-a.cbl".to_string(), recorder("SLOW-A", &runs, 400)),
        ("fast-b.cbl".to_string(), recorder("FAST-B", &runs, 0)),
        ("indep.cbl".to_string(), recorder("INDEP", &runs, 0)),
    ];
    v.push((
        "reject.cbl".into(),
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. REJECT.\nDATA DIVISION.\nLINKAGE SECTION.\n01 LK-A.\n    05 AMOUNT PIC 9(4).\nPROCEDURE DIVISION USING LK-A.\nMAIN.\n    MOVE 9999 TO AMOUNT\n    COBOL::\"WS-FAIL\" ( 4201 \"card declined\" '{\"retry\":false}' )\n    MOVE 1 TO AMOUNT\n    GOBACK.\n".into(),
    ));
    v.push((
        "crash.cbl".into(),
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. CRASH.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-X PIC 9 VALUE 0.\nLINKAGE SECTION.\n01 LK-A.\n    05 N PIC 9(2).\nPROCEDURE DIVISION USING LK-A.\nMAIN.\n    COBOL::\"WS-FAIL\" ( 9001 \"x\" )\n    GOBACK.\n".into(),
    ));
    v.push((
        "admin-only.cbl".into(),
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. ADMIN-ONLY.\nDATA DIVISION.\nLINKAGE SECTION.\n01 LK-X.\n    05 MSG PIC X(20).\nPROCEDURE DIVISION USING LK-X.\nMAIN.\n    MOVE \"admin ok\" TO MSG\n    GOBACK.\n".into(),
    ));
    v.push((
        "long.cbl".into(),
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. LONG.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 I PIC 999.\n01 P PIC 999.\nLINKAGE SECTION.\n01 LK-A.\n    05 STEPS PIC 999.\n    05 LAST-STEP PIC 999.\nPROCEDURE DIVISION USING LK-A.\nMAIN.\n    PERFORM VARYING I FROM 1 BY 1 UNTIL I > STEPS\n        COBOL::\"WS-PAUSE\" ( 40 )\n        COMPUTE P = I * 100 / STEPS\n        COBOL::\"WS-PROGRESS\" ( P \"working\" )\n        MOVE I TO LAST-STEP\n    END-PERFORM\n    GOBACK.\n".into(),
    ));
    v.push((
        "ticker.cbl".into(),
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. TICKER.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-STOP PIC X VALUE \"N\".\n01 TICK.\n    05 N PIC 9(6) VALUE 0.\nLINKAGE SECTION.\n01 LK-A.\n    05 EVERY-MS PIC 9(4).\nPROCEDURE DIVISION USING LK-A.\nMAIN.\n    PERFORM UNTIL WS-STOP = \"Y\"\n        ADD 1 TO N\n        COBOL::\"WS-EMIT\" ( TICK )\n        COBOL::\"WS-PAUSE\" ( EVERY-MS )\n        COBOL::\"WS-CANCELLED\" ( WS-STOP )\n    END-PERFORM\n    GOBACK.\n".into(),
    ));
    v.push((
        "old-one.cbl".into(),
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. OLD-ONE.\nDATA DIVISION.\nLINKAGE SECTION.\n01 LK-A.\n    05 N PIC 9(2).\nPROCEDURE DIVISION USING LK-A.\nMAIN.\n    ADD 1 TO N\n    GOBACK.\n".into(),
    ));
    v.push((
        "store-info.cbl".into(),
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. STORE-INFO.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT UP ASSIGN TO WS-PATH\n        ORGANIZATION IS LINE SEQUENTIAL.\nDATA DIVISION.\nFILE SECTION.\nFD UP.\n01 UP-LINE PIC X(80).\nWORKING-STORAGE SECTION.\n01 WS-PATH PIC X(300).\n01 WS-ST PIC X(8).\nLINKAGE SECTION.\n01 LK-A.\n    05 FILE-ID PIC X(32).\n    05 FIRST-LINE PIC X(40).\n    05 FOUND PIC X(8).\nPROCEDURE DIVISION USING LK-A.\nMAIN.\n    COBOL::\"WS-FILE-PATH\" ( FILE-ID WS-PATH WS-ST )\n    MOVE WS-ST TO FOUND\n    IF WS-ST = \"OK\"\n        OPEN INPUT UP\n        READ UP\n            AT END MOVE SPACES TO UP-LINE\n        END-READ\n        MOVE UP-LINE TO FIRST-LINE\n        CLOSE UP\n    END-IF\n    GOBACK.\n".into(),
    ));
    v.push((
        "offer.cbl".into(),
        format!("IDENTIFICATION DIVISION.\nPROGRAM-ID. OFFER.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT OUT ASSIGN TO \"{dir_for_files}/report.txt\"\n        ORGANIZATION IS LINE SEQUENTIAL.\nDATA DIVISION.\nFILE SECTION.\nFD OUT.\n01 OUT-LINE PIC X(60).\nWORKING-STORAGE SECTION.\n01 WS-ST PIC X(8).\nLINKAGE SECTION.\n01 LK-A.\n    05 FILE-ID PIC X(32).\n    05 STATUS-WORD PIC X(8).\nPROCEDURE DIVISION USING LK-A.\nMAIN.\n    OPEN OUTPUT OUT\n    MOVE \"the monthly report, line one\" TO OUT-LINE\n    WRITE OUT-LINE\n    MOVE \"line two\" TO OUT-LINE\n    WRITE OUT-LINE\n    CLOSE OUT\n    COBOL::\"WS-FILE-PUBLISH\" ( \"{dir_for_files}/report.txt\" FILE-ID WS-ST )\n    MOVE WS-ST TO STATUS-WORD\n    GOBACK.\n"),
    ));
    v
}

/// What a test brings: a running backend, a directory for its files, a client factory.
struct World {
    rig: Rig,
    files: std::path::PathBuf,
}

fn world(name: &str, extra: &str) -> World {
    world_with(name, extra, &|_| {})
}

/// `setup` sees the backend folder before the server starts (to leave released interfaces in it).
fn world_with(name: &str, extra: &str, setup: &dyn Fn(&std::path::Path)) -> World {
    let files = std::env::temp_dir().join(format!("prc-contract-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&files);
    std::fs::create_dir_all(&files).unwrap();
    let srcs = sources(&files.display().to_string());
    let refs: Vec<(&str, &str)> = srcs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let dir = backend_dir(name, EXPORTS, &refs);
    setup(&dir);
    // The `[limits]` every test starts from; `extra` lines (`key = value`) replace or add.
    let mut limits: Vec<(String, String)> = [
        ("connection_rate_per_sec", "1000"),
        ("connection_burst", "1000"),
        ("user_rate_per_sec", "5000"),
        ("user_burst", "5000"),
        ("max_calls_per_connection", "16"),
        ("call_timeout_ms", "20000"),
        ("file_lock_wait_ms", "5000"),
        ("cancel_grace_ms", "1500"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    // Lines written `auth.key = value` belong to `[auth]` and go ahead of `[limits]`.
    let auth_lines: String = extra.lines().filter_map(|l| l.trim().strip_prefix("auth.")).map(|l| format!("{l}\n")).collect();
    for line in extra.lines().filter(|l| l.contains('=') && !l.trim().starts_with("auth.")) {
        let (k, v) = line.split_once('=').unwrap();
        let (k, v) = (k.trim().to_string(), v.trim().to_string());
        match limits.iter_mut().find(|(name, _)| *name == k) {
            Some(slot) => slot.1 = v,
            None => limits.push((k, v)),
        }
    }
    let limits_text: String = limits.iter().map(|(k, v)| format!("{k} = {v}\n")).collect();
    let cfg = format!(
        "{auth_lines}[limits]\n{limits_text}[files]\ndir = \"{}\"\nmax_upload_bytes = 20000000\ndownload_ttl_seconds = 2\n[[files.slots]]\nname = \"docs\"\nmime = [\"text/plain\", \"application/pdf\", \"image/*\"]\n",
        files.join("store").display()
    );
    World { rig: rig(dir, &cfg), files }
}

impl World {
    fn client_at(&self, addr: std::net::SocketAddr, contract: &str, token: Option<String>) -> WsClient {
        let mut cfg = ClientConfig::new(format!("wss://localhost:{}/ws", addr.port()), contract);
        cfg.extra_root_pem = Some(self.rig.cert.cert_pem.clone().into_bytes());
        cfg.origin = Some(ORIGIN.to_string());
        cfg.interface = Some(self.rig.interface());
        cfg.call_timeout = Duration::from_secs(20);
        cfg.reconnect_deadline = Duration::from_secs(10);
        if let Some(t) = token {
            cfg.token = Some(Arc::new(StaticToken(t)));
        }
        WsClient::new(cfg)
    }

    fn client(&self, contract: &str) -> WsClient {
        let t = self.rig.issuer.access("user-1", AUDIENCE, 3600, json!({}));
        self.client_at(self.rig.server.addr, contract, Some(t))
    }

    fn runs(&self) -> usize {
        std::fs::read_to_string(self.files.join("runs.txt")).map(|s| s.lines().filter(|l| !l.trim().is_empty()).count()).unwrap_or(0)
    }
}

/// `PING` takes no argument and answers in `RETURNING`.
fn ping_req() -> CallRequest {
    CallRequest { program: "PING".to_string(), args: vec![], wants_returning: true, idempotency_key: None }
}

fn req(program: &str, json_arg: &str) -> CallRequest {
    CallRequest { program: program.to_string(), args: vec![ArgPayload { mode: ArgMode::Reference, json: json_arg.to_string() }], wants_returning: false, idempotency_key: None }
}

#[derive(Default)]
struct Watch {
    progress: Vec<(u8, String)>,
    warnings: Vec<String>,
    cancel_after: Option<usize>,
}

impl CallObserver for Watch {
    fn progress(&mut self, percent: u8, message: &str) {
        self.progress.push((percent, message.to_string()));
    }
    fn warning(&mut self, message: &str) {
        self.warnings.push(message.to_string());
    }
    fn cancel_requested(&mut self) -> bool {
        self.cancel_after.map(|n| self.progress.len() >= n).unwrap_or(false)
    }
}

fn done_of(reply: &cobolt_wire::call::CallReply) -> u64 {
    let v: serde_json::Value = serde_json::from_str(reply.args[0].as_ref().unwrap()).unwrap();
    v["DONE"].as_u64().unwrap_or(0)
}

// ── AC1, AC10: versions, both directions of skew ─────────────────────────────

#[test]
fn ac1_ac10_a_client_built_for_an_older_minor_is_accepted_with_a_warning_and_a_newer_one_is_told_to_update_the_backend() {
    // The released 1.0.0 interface: SAFE-ADD's group had no DONE member then (it was added in 1.1).
    let w = world_with("ac1", "", &|dir| {
        let mut released = cobolt_server::backend::Backend::load(dir).expect("loads").interface;
        released.contract = "1.0.0".into();
        let sig = released.programs.iter_mut().find(|p| p.name == "SAFE-ADD").expect("exported");
        if let cobolt_wire::layout::Kind::Group { children } = &mut sig.params[0].layout.kind {
            children.retain(|c| c.name != "DONE");
        }
        let to = dir.join("contract").join("released");
        std::fs::create_dir_all(&to).unwrap();
        std::fs::write(to.join("1.0.0.json"), released.to_json()).unwrap();
    });

    // AC10, one direction: an application built for 1.0 against the backend at 1.2 is served in the
    // shape it was built for - it asks without DONE and is answered without DONE - and is told it is old.
    let mut older = w.client("1.0.0");
    let reply = older.call(&req("SAFE-ADD", "{\"TAG\":\"old\"}")).expect("an older minor of the same major is served");
    assert!(reply.warnings.iter().any(|m| m.contains("compatible") && m.contains("1.0.0")), "{:?}", reply.warnings);
    let answer: serde_json::Value = serde_json::from_str(reply.args[0].as_ref().unwrap()).unwrap();
    assert_eq!(answer, json!({"TAG": "old"}), "the member the old application never heard of is not sent to it");
    // …and the current application gets the whole group.
    let mut current = w.client("1.2.0");
    let reply = current.call(&req("SAFE-ADD", "{\"TAG\":\"new\"}")).expect("current");
    assert_eq!(serde_json::from_str::<serde_json::Value>(reply.args[0].as_ref().unwrap()).unwrap()["DONE"], 1);
    assert!(reply.warnings.is_empty(), "{:?}", reply.warnings);

    let mut newer = w.client("1.3.0");
    let e = newer.call(&ping_req()).expect_err("a newer minor than the backend has");
    assert_eq!((e.class, e.code), (ErrorClass::Version, codes::CLIENT_TOO_NEW), "{e:?}");
    assert!(e.message.contains("update the backend"), "{}", e.message);
    assert_eq!(e.may_have_run, Some(false));

    let mut other_major = w.client("0.9.0");
    let e = other_major.call(&ping_req()).expect_err("a lower major");
    assert_eq!((e.class, e.code), (ErrorClass::Version, codes::CLIENT_TOO_OLD), "{e:?}");
    assert_eq!(e.detail.as_deref(), Some("1.2.0"), "the refusal says what to update to");
    w.rig.server.stop();
}

// ── R13: an expired token is refreshed and the call repeated, once ───────────

struct Refreshing {
    first: String,
    fresh: String,
    refreshed: std::sync::atomic::AtomicUsize,
}

impl cobolt_server::client::TokenSource for Refreshing {
    fn token(&self) -> Option<String> {
        Some(self.first.clone())
    }
    fn refresh(&self) -> Option<String> {
        self.refreshed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Some(self.fresh.clone())
    }
}

fn refreshing_client(w: &World, first: String, fresh: String) -> (WsClient, Arc<Refreshing>) {
    let src = Arc::new(Refreshing { first, fresh, refreshed: std::sync::atomic::AtomicUsize::new(0) });
    let mut cfg = ClientConfig::new(format!("wss://localhost:{}/ws", w.rig.server.addr.port()), "1.2.0");
    cfg.extra_root_pem = Some(w.rig.cert.cert_pem.clone().into_bytes());
    cfg.origin = Some(ORIGIN.to_string());
    cfg.token = Some(src.clone());
    (WsClient::new(cfg), src)
}

#[test]
fn an_expired_token_is_refreshed_at_connection_and_again_in_the_middle_of_a_session() {
    let w = world("refresh", "auth.leeway_seconds = 0");
    // At connection: the token the application holds expired ten minutes ago.
    let stale = w.rig.issuer.access("user-1", AUDIENCE, -600, json!({}));
    let fresh = w.rig.issuer.access("user-1", AUDIENCE, 3600, json!({}));
    let (c, src) = refreshing_client(&w, stale, fresh.clone());
    let mut t = c.clone();
    assert!(t.call(&req("SAFE-ADD", "{\"TAG\":\"r1\"}")).is_ok(), "refreshed at connection, then served");
    assert_eq!(src.refreshed.load(std::sync::atomic::Ordering::SeqCst), 1);

    // In the middle of a session: a token that is good for two seconds runs out between two calls.
    let short = w.rig.issuer.access("user-1", AUDIENCE, 2, json!({}));
    let (c, src) = refreshing_client(&w, short, fresh);
    let mut t = c.clone();
    assert!(t.call(&req("SAFE-ADD", "{\"TAG\":\"r2\"}")).is_ok(), "good while it lasts");
    assert_eq!(src.refreshed.load(std::sync::atomic::Ordering::SeqCst), 0);
    std::thread::sleep(Duration::from_millis(3500));
    assert!(t.call(&req("SAFE-ADD", "{\"TAG\":\"r3\"}")).is_ok(), "expired in between: refreshed and repeated, the application sees nothing");
    assert_eq!(src.refreshed.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(t.call(&req("SAFE-ADD", "{\"TAG\":\"r4\"}")).is_ok());
    assert_eq!(src.refreshed.load(std::sync::atomic::Ordering::SeqCst), 1, "the connection carries the new token from then on");

    // A source that has nothing newer: the application is told.
    let stale = w.rig.issuer.access("user-1", AUDIENCE, -600, json!({}));
    let mut cfg = ClientConfig::new(format!("wss://localhost:{}/ws", w.rig.server.addr.port()), "1.2.0");
    cfg.extra_root_pem = Some(w.rig.cert.cert_pem.clone().into_bytes());
    cfg.origin = Some(ORIGIN.to_string());
    cfg.token = Some(Arc::new(StaticToken(stale)));
    let e = WsClient::new(cfg).call(&req("SAFE-ADD", "{\"TAG\":\"r5\"}")).unwrap_err();
    assert_class(&e, ErrorClass::Authentication, codes::TOKEN_EXPIRED);
    w.rig.server.stop();
}

// ── AC3 ───────────────────────────────────────────────────────────────────────

#[test]
fn ac3_a_deprecated_program_works_and_the_warning_reaches_the_observer() {
    let w = world("ac3", "");
    let mut c = w.client("1.2.0");
    let mut watch = Watch::default();
    let reply = c.call_observed(&req("OLD-ONE", "{\"N\":4}"), &mut watch).expect("the program still works");
    assert_eq!(serde_json::from_str::<serde_json::Value>(reply.args[0].as_ref().unwrap()).unwrap()["N"], 5);
    assert!(watch.warnings.iter().any(|m| m.contains("deprecated") && m.contains("use NEW-ONE") && m.contains("2.0.0")), "{:?}", watch.warnings);
    w.rig.server.stop();
}

// ── AC4, AC5: every class, and the application failure ───────────────────────

fn assert_class(e: &CallError, class: ErrorClass, code: u32) {
    assert_eq!((e.class, e.code), (class, code), "{e:?}");
}

#[test]
fn ac4_each_failure_class_is_produced_and_the_program_reads_class_and_code() {
    let w = world("ac4", "");
    let mut c = w.client("1.2.0");

    // validation: an unknown member of the group
    let e = c.call(&req("SAFE-ADD", "{\"TAG\":\"a\",\"DONE\":0,\"EXTRA\":1}")).unwrap_err();
    assert_class(&e, ErrorClass::Validation, codes::INVALID_PAYLOAD);
    // not found
    let e = c.call(&req("NO-SUCH", "{}")).unwrap_err();
    assert_class(&e, ErrorClass::NotFound, codes::UNKNOWN_PROGRAM);
    // authorisation: the token has no admin role
    let e = c.call(&req("ADMIN-ONLY", "{\"MSG\":\"\"}")).unwrap_err();
    assert_class(&e, ErrorClass::Authorisation, codes::ROLE_REFUSED);
    // application (AC5): the program's own code, message and detail
    let e = c.call(&req("REJECT", "{\"AMOUNT\":1}")).unwrap_err();
    assert_class(&e, ErrorClass::Application, 4201);
    assert_eq!(e.message, "card declined");
    assert_eq!(e.detail.as_deref(), Some("{\"retry\":false}"));
    // server: a program that stops with a run-time error
    let e = c.call(&req("CRASH", "{\"N\":1}")).unwrap_err();
    assert_class(&e, ErrorClass::Server, codes::PROGRAM_FAULT);
    assert!(!e.message.contains("NO-SUCH-FILE"), "the caller does not learn the program's internals: {}", e.message);
    // authentication: a token the backend does not trust
    let mut bad = w.client_at(w.rig.server.addr, "1.2.0", Some("not.a.token".to_string()));
    let e = bad.call(&req("SAFE-ADD", "{\"TAG\":\"a\",\"DONE\":0}")).unwrap_err();
    assert_class(&e, ErrorClass::Authentication, codes::TOKEN_INVALID);
    assert_eq!(e.may_have_run, Some(false));
    // authentication: no token at all, for a program that is not public
    let mut none = w.client_at(w.rig.server.addr, "1.2.0", None);
    let e = none.call(&req("SAFE-ADD", "{\"TAG\":\"a\",\"DONE\":0}")).unwrap_err();
    assert_class(&e, ErrorClass::Authentication, codes::TOKEN_MISSING);
    // ... and a public one answers without a token
    assert!(none.call(&ping_req()).is_ok());
    // transport: nobody is listening
    let dead = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let mut gone = w.client_at(dead, "1.2.0", None);
    let e = gone.call(&ping_req()).unwrap_err();
    assert_class(&e, ErrorClass::Transport, codes::NO_CONNECTION);
    assert_eq!(e.may_have_run, Some(false), "nothing was sent");
    w.rig.server.stop();
}

#[test]
fn ac4_limits_are_their_own_class_with_their_own_codes() {
    let w = world("ac4l", "max_frame_bytes = 4096\nconnection_rate_per_sec = 1\nconnection_burst = 3\n");
    let mut c = w.client("1.2.0");
    let big = format!("{{\"TAG\":\"{}\",\"DONE\":0}}", "x".repeat(6000));
    let e = c.call(&req("SAFE-ADD", &big)).unwrap_err();
    assert_class(&e, ErrorClass::Limit, codes::FRAME_TOO_LARGE);
    assert_eq!(e.may_have_run, Some(false), "a refused frame did not run");
    let mut hits = 0;
    let mut c2 = w.client("1.2.0");
    for _ in 0..8 {
        if let Err(e) = c2.call(&ping_req()) {
            if e.class == ErrorClass::Limit && e.code == codes::RATE_LIMITED {
                hits += 1;
            }
        }
    }
    assert!(hits >= 1, "the rate limit answered at least once");
    w.rig.server.stop();
}

// ── AC6: a dropped connection, safe and unsafe, and the idempotency key ──────

#[test]
fn ac6_a_call_to_a_program_declared_safe_is_repeated_once_after_a_lost_connection() {
    let w = world("ac6s", "");
    let proxy = Proxy::start(w.rig.server.addr);
    let c = w.client_at(proxy.addr, "1.2.0", Some(w.rig.issuer.access("user-1", AUDIENCE, 3600, json!({}))));
    let mut t = c.clone();
    // Warm the connection, then cut it while SAFE-ADD is working.
    t.call(&ping_req()).expect("warm");
    let p = Arc::new(proxy);
    let p2 = p.clone();
    let killer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(80));
        p2.kill_all();
    });
    let reply = t.call(&req("SAFE-ADD", "{\"TAG\":\"safe\",\"DONE\":0}")).expect("repeated on a new connection");
    killer.join().unwrap();
    assert_eq!(done_of(&reply), 1);
    assert!(w.runs() >= 1);
    w.rig.server.stop();
}

#[test]
fn ac6_a_call_to_a_program_not_declared_safe_fails_and_says_the_backend_may_have_run_it_and_a_repeat_with_its_key_does_not_run_it_twice() {
    let w = world("ac6u", "");
    let proxy = Arc::new(Proxy::start(w.rig.server.addr));
    let mut t = w.client_at(proxy.addr, "1.2.0", Some(w.rig.issuer.access("user-1", AUDIENCE, 3600, json!({}))));
    t.call(&ping_req()).expect("warm");

    // The server runs the call and its answer is lost: the proxy swallows it, then cuts the line.
    proxy.set(PROXY_SWALLOW_REPLIES);
    let p2 = proxy.clone();
    let killer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(600));
        p2.kill_all();
    });
    let key = "idem-0123456789abcdef".to_string();
    let mut r = req("CHARGE", "{\"TAG\":\"charge\",\"DONE\":0}");
    r.idempotency_key = Some(key.clone());
    let e = t.call(&r).expect_err("the answer never arrived");
    killer.join().unwrap();
    assert_eq!(e.class, ErrorClass::Transport, "{e:?}");
    assert_eq!(e.may_have_run, Some(true), "the call was sent: the backend may have run it");
    assert_eq!(w.runs(), 1, "it did run, once");

    // The developer repeats it with the same key: answered from the store, not run again.
    proxy.set(PROXY_FORWARD);
    let again = t.call(&r).expect("the repeat is answered");
    assert_eq!(done_of(&again), 1);
    assert_eq!(w.runs(), 1, "the program did not run a second time");
    // The same key with other data is refused.
    let mut other = req("CHARGE", "{\"TAG\":\"other\",\"DONE\":0}");
    other.idempotency_key = Some(key);
    let e = t.call(&other).unwrap_err();
    assert_class(&e, ErrorClass::Validation, codes::INVALID_PAYLOAD);
    w.rig.server.stop();
}

// ── AC7: order ───────────────────────────────────────────────────────────────

#[test]
fn ac7_calls_on_one_connection_keep_their_order_unless_the_program_is_independent() {
    let w = world("ac7", "");
    let c = w.client("1.2.0");
    let log: Arc<Mutex<Vec<(&'static str, Instant)>>> = Arc::new(Mutex::new(Vec::new()));
    let t0 = Instant::now();
    let spawn = |program: &'static str, delay_ms: u64| {
        let (mut cc, log) = (c.clone(), log.clone());
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(delay_ms));
            cc.call(&req(program, "{\"TAG\":\"o\",\"DONE\":0}")).expect("call");
            log.lock().unwrap().push((program, Instant::now()));
        })
    };
    // SLOW-A first (400 ms of work), FAST-B right behind it (dependent: waits its turn), INDEP too (may overlap).
    let a = spawn("SLOW-A", 0);
    let b = spawn("FAST-B", 100);
    let i = spawn("INDEP", 200);
    for h in [a, b, i] {
        h.join().unwrap();
    }
    let order: Vec<&str> = log.lock().unwrap().iter().map(|(n, _)| *n).collect();
    let pos = |n: &str| order.iter().position(|x| *x == n).unwrap();
    assert!(pos("SLOW-A") < pos("FAST-B"), "a dependent call waits for the one made before it: {order:?}");
    assert!(pos("INDEP") < pos("SLOW-A"), "an independent call overlaps instead of waiting: {order:?}");
    println!("\n  order: {order:?} after {:.2} s", t0.elapsed().as_secs_f64());
    w.rig.server.stop();
}

// ── AC8: progress, cancel, streams ───────────────────────────────────────────

#[test]
fn ac8_a_long_running_call_reports_progress_and_can_be_cancelled() {
    let w = world("ac8a", "");
    let mut c = w.client("1.2.0");
    let mut watch = Watch::default();
    let reply = c.call_observed(&req("LONG", "{\"STEPS\":10,\"LAST-STEP\":0}"), &mut watch).expect("completes");
    assert_eq!(serde_json::from_str::<serde_json::Value>(reply.args[0].as_ref().unwrap()).unwrap()["LAST-STEP"], 10);
    assert!(watch.progress.len() >= 3, "{:?}", watch.progress);
    assert_eq!(watch.progress.last().unwrap().0, 100, "the last report, 100 %, is never dropped");
    assert!(watch.progress.windows(2).all(|p| p[0].0 <= p[1].0), "progress only moves forward: {:?}", watch.progress);

    // Cancel after the third report: the program stops at its next safe point.
    let mut watch = Watch { cancel_after: Some(3), ..Watch::default() };
    let t = Instant::now();
    let e = c.call_observed(&req("LONG", "{\"STEPS\":100,\"LAST-STEP\":0}"), &mut watch).unwrap_err();
    assert_class(&e, ErrorClass::Application, 8000);
    assert!(t.elapsed() < Duration::from_secs(3), "100 steps of 40 ms would have taken 4 s: {:?}", t.elapsed());
    w.rig.server.stop();
}

#[test]
fn ac8_a_stream_delivers_events_survives_a_reconnection_and_costs_nothing_after_unsubscribing() {
    let w = world("ac8b", "");
    let proxy = Proxy::start(w.rig.server.addr);
    let c = w.client_at(proxy.addr, "1.2.0", Some(w.rig.issuer.access("user-1", AUDIENCE, 3600, json!({}))));
    let sub = c.subscribe("TICKER", vec![(ArgMode::Reference, "{\"EVERY-MS\":30}".to_string())]).expect("subscribes");
    let mut seqs = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while seqs.len() < 5 && Instant::now() < deadline {
        if let Ok(StreamItem::Event { seq, json }) = sub.items.recv_timeout(Duration::from_millis(200)) {
            let v: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(v["N"].as_u64(), Some(seq), "{json}");
            seqs.push(seq);
        }
    }
    assert_eq!(seqs, vec![1, 2, 3, 4, 5], "in order, from 1");
    assert_eq!(w.rig.server.stats().subscriptions_active.load(std::sync::atomic::Ordering::SeqCst), 1);

    // Cut the connection: the client subscribes again, and says so.
    proxy.kill_all();
    let mut restarted = false;
    let mut after = 0;
    let deadline = Instant::now() + Duration::from_secs(15);
    while after < 3 && Instant::now() < deadline {
        match sub.items.recv_timeout(Duration::from_millis(200)) {
            Ok(StreamItem::Restarted) => restarted = true,
            Ok(StreamItem::Event { seq, .. }) if restarted => {
                after += 1;
                assert!(seq >= 1);
            }
            _ => {}
        }
    }
    assert!(restarted && after >= 3, "restarted={restarted} events after={after}");

    // Unsubscribe: the program is stopped and the backend holds nothing for it.
    sub.unsubscribe();
    let deadline = Instant::now() + Duration::from_secs(5);
    while w.rig.server.stats().subscriptions_active.load(std::sync::atomic::Ordering::SeqCst) != 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(w.rig.server.stats().subscriptions_active.load(std::sync::atomic::Ordering::SeqCst), 0, "an unsubscribed client costs the server nothing");
    w.rig.server.stop();
}

// ── AC9: files ───────────────────────────────────────────────────────────────

#[derive(Default)]
struct Pct(Vec<u8>);

impl CallObserver for Pct {
    fn progress(&mut self, percent: u8, _message: &str) {
        self.0.push(percent);
    }
}

fn make_file(dir: &std::path::Path, name: &str, bytes: usize) -> (std::path::PathBuf, String) {
    use sha2::{Digest, Sha256};
    let mut data = Vec::with_capacity(bytes);
    let line = b"the quick brown fox jumps over the lazy dog 0123456789\n";
    while data.len() < bytes {
        data.extend_from_slice(line);
    }
    data.truncate(bytes);
    let path = dir.join(name);
    std::fs::write(&path, &data).unwrap();
    (path, Sha256::digest(&data).iter().map(|b| format!("{b:02x}")).collect())
}

#[test]
fn ac9_a_file_larger_than_a_frame_uploads_in_pieces_resumes_after_a_cut_and_arrives_with_its_checksum() {
    let w = world("ac9u", "max_frame_bytes = 65536\n");
    let proxy = Arc::new(Proxy::start(w.rig.server.addr));
    let mut t = w.client_at(proxy.addr, "1.2.0", Some(w.rig.issuer.access("user-1", AUDIENCE, 3600, json!({}))));
    let (path, sum) = make_file(&w.files, "big.txt", 3_000_000);
    // Cut the connection part-way through.
    let p2 = proxy.clone();
    let killer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        p2.kill_all();
    });
    let mut pct = Pct::default();
    let t0 = Instant::now();
    let done = t.upload(&UploadRequest { slot: "docs".into(), path: path.clone(), name: "big.txt".into(), mime: "text/plain".into() }, &mut pct).expect("uploads");
    killer.join().unwrap();
    let secs = t0.elapsed().as_secs_f64();
    assert_eq!(done.size, 3_000_000);
    assert!(pct.0.len() > 10, "many pieces, each reported: {}", pct.0.len());
    assert_eq!(*pct.0.last().unwrap(), 100);
    // The backend holds exactly what was sent, under a name of its own.
    let stored = w.files.join("store").join("files").join(&done.file);
    let bytes = std::fs::read(&stored).expect("stored under the backend's name");
    use sha2::{Digest, Sha256};
    let got: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(got, sum);
    assert_eq!(done.file.len(), 32);
    println!("\n  upload: 3,000,000 bytes in {} pieces of at most 65,536-byte frames, with one forced disconnect, in {:.2} s ({:.1} MB/s); SHA-256 matches", pct.0.len(), secs, 3.0 / secs);

    // A program reaches the file by the id the backend gave, and only that caller's.
    let r = t.call(&req("STORE-INFO", &format!("{{\"FILE-ID\":\"{}\",\"FIRST-LINE\":\"\",\"FOUND\":\"\"}}", done.file))).expect("store-info");
    let v: serde_json::Value = serde_json::from_str(r.args[0].as_ref().unwrap()).unwrap();
    assert_eq!(v["FOUND"], "OK");
    assert!(v["FIRST-LINE"].as_str().unwrap().starts_with("the quick brown fox"), "{v}");
    let other_token = w.rig.issuer.access("someone-else", AUDIENCE, 3600, json!({}));
    let mut other = w.client_at(w.rig.server.addr, "1.2.0", Some(other_token));
    let r = other.call(&req("STORE-INFO", &format!("{{\"FILE-ID\":\"{}\",\"FIRST-LINE\":\"\",\"FOUND\":\"\"}}", done.file))).expect("store-info");
    let v: serde_json::Value = serde_json::from_str(r.args[0].as_ref().unwrap()).unwrap();
    assert_eq!(v["FOUND"], "NOFILE", "someone else's upload does not exist for this caller");
    w.rig.server.stop();
}

#[test]
fn ac9_a_name_with_dot_dot_a_wrong_type_a_wrong_checksum_and_an_unknown_slot_are_refused() {
    let w = world("ac9r", "");
    let mut t = w.client("1.2.0");
    let (path, _) = make_file(&w.files, "ok.txt", 1000);
    let mut up = |slot: &str, name: &str, mime: &str| t.upload(&UploadRequest { slot: slot.into(), path: path.clone(), name: name.into(), mime: mime.into() }, &mut Pct::default());
    assert_class(&up("docs", "../../etc/passwd", "text/plain").unwrap_err(), ErrorClass::Validation, codes::INVALID_PAYLOAD);
    assert_class(&up("docs", "a/b.txt", "text/plain").unwrap_err(), ErrorClass::Validation, codes::INVALID_PAYLOAD);
    assert_class(&up("docs", "fine.txt", "application/x-msdownload").unwrap_err(), ErrorClass::Validation, codes::INVALID_PAYLOAD);
    assert_class(&up("no-such-slot", "fine.txt", "text/plain").unwrap_err(), ErrorClass::NotFound, codes::UNKNOWN_PROGRAM);
    // A PDF that is not one.
    let e = up("docs", "fake.pdf", "application/pdf").unwrap_err();
    assert_class(&e, ErrorClass::Validation, codes::INVALID_PAYLOAD);
    assert!(e.message.contains("not of the declared type"), "{}", e.message);
    // Nothing of that was stored.
    let stored = std::fs::read_dir(w.files.join("store").join("files")).map(|d| d.count()).unwrap_or(0);
    assert_eq!(stored, 0);
    // The size limit (20,000,000 bytes in this rig): refused at the beginning, before any data goes.
    let (huge, _) = make_file(&w.files, "huge.txt", 21_000_000);
    let e = t.upload(&UploadRequest { slot: "docs".into(), path: huge, name: "huge.txt".into(), mime: "text/plain".into() }, &mut Pct::default()).unwrap_err();
    assert_class(&e, ErrorClass::Limit, codes::FILE_TOO_LARGE);
    assert_eq!(w.rig.server.files().uploads_in_progress(), 0, "nothing was started for it");
    w.rig.server.stop();
}

#[test]
fn ac9_a_program_offers_a_file_and_the_download_address_is_short_lived_and_the_download_resumes() {
    let w = world("ac9d", "");
    let mut t = w.client("1.2.0");
    let r = t.call(&req("OFFER", "{\"FILE-ID\":\"\",\"STATUS-WORD\":\"\"}")).expect("offer");
    let v: serde_json::Value = serde_json::from_str(r.args[0].as_ref().unwrap()).unwrap();
    assert_eq!(v["STATUS-WORD"], "OK");
    let file_id = v["FILE-ID"].as_str().unwrap().to_string();
    assert_eq!(file_id.len(), 32);
    let dest = w.files.join("got.txt");
    let got = t.download(&DownloadRequest { file: file_id.clone(), dest: dest.clone() }, &mut Pct::default()).expect("downloads");
    let text = std::fs::read_to_string(&dest).unwrap();
    assert!(text.starts_with("the monthly report, line one"), "{text}");
    assert_eq!(got.size as usize, text.len());

    // An address the backend issued dies after download_ttl (2 s here), and is the same 404 as one that never existed.
    let raw = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let mut c = Raw::connect(&w.rig, Some(ORIGIN)).await.expect("connect");
        c.hello("1.2.0").await;
        c.auth(&w.rig.issuer.access("user-1", AUDIENCE, 3600, json!({}))).await;
        c.send(json!({"t": "get-file", "id": 7, "file": file_id})).await;
        c.next().await.expect("file-url")
    });
    assert_eq!(raw["t"], "file-url", "{raw}");
    let path = raw["path"].as_str().unwrap().to_string();
    let live = https_get(w.rig.server.addr, &w.rig.cert.cert_pem, &path, None).expect("answer");
    assert_eq!(live.0, 200);
    let tail = https_get(w.rig.server.addr, &w.rig.cert.cert_pem, &path, Some(10)).expect("answer");
    assert_eq!(tail.0, 206, "a range resumes");
    assert_eq!(tail.1, live.1[10..].to_vec());
    std::thread::sleep(Duration::from_millis(2300));
    assert_eq!(https_get(w.rig.server.addr, &w.rig.cert.cert_pem, &path, None).expect("answer").0, 404, "expired");
    assert_eq!(https_get(w.rig.server.addr, &w.rig.cert.cert_pem, &format!("/dl/{}", "0".repeat(64)), None).expect("answer").0, 404, "never existed: the same answer");
    assert_eq!(https_get(w.rig.server.addr, &w.rig.cert.cert_pem, "/dl/../../etc/passwd", None).expect("answer").0, 404);
    w.rig.server.stop();
}
