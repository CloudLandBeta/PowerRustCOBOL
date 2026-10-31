// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The backend end to end, over a real TLS loopback socket (WASM-003 AC1-AC8).
//!
//! A server is started with a backend written to a temp folder, a self-signed
//! certificate and a one-key identity provider; a raw client says what the
//! protocol allows and what it does not.

mod support;

use std::time::{Duration, Instant};

use serde_json::{json, Value};
use support::*;

const EXPORTS: &str = r#"
[contract]
version = "1.2.0"

[[export]]
program = "GET-CUSTOMER"

[[export]]
program = "ADMIN-ONLY"
roles = ["admin"]

[[export]]
program = "PING"
public = true

[[export]]
program = "SLOW"

[[export]]
program = "TABLE-ECHO"

[[export]]
program = "COUNTER"
independent = true
"#;

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

const ADMIN_ONLY: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. ADMIN-ONLY.
DATA DIVISION.
LINKAGE SECTION.
01 LK-X.
    05 MSG PIC X(20).
PROCEDURE DIVISION USING LK-X.
MAIN.
    MOVE "admin ok" TO MSG
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

const SECRET_HELPER: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. SECRET-HELPER.
DATA DIVISION.
LINKAGE SECTION.
01 LK-A.
    05 X PIC X(8).
PROCEDURE DIVISION USING LK-A.
MAIN.
    MOVE "leaked" TO X
    GOBACK.
"#;

const SLOW: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. SLOW.
DATA DIVISION.
LINKAGE SECTION.
01 LK-N.
    05 N PIC 9(4).
PROCEDURE DIVISION USING LK-N.
MAIN.
    PERFORM UNTIL 1 = 2
        CONTINUE
    END-PERFORM
    GOBACK.
"#;

const TABLE_ECHO: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. TABLE-ECHO.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I PIC 99.
LINKAGE SECTION.
01 LK-T.
    05 N    PIC 99.
    05 ITEM PIC 9(3) OCCURS 1 TO 4 DEPENDING ON N.
    05 NOTE PIC X(6).
PROCEDURE DIVISION USING LK-T.
MAIN.
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > N
        COMPUTE ITEM (I) = ITEM (I) * 2
    END-PERFORM
    GOBACK.
"#;

const COUNTER: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. COUNTER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-COUNT PIC 9(4) VALUE 0.
LINKAGE SECTION.
01 LK-C.
    05 WHO  PIC X(12).
    05 SEEN PIC 9(4).
PROCEDURE DIVISION USING LK-C.
MAIN.
    ADD 1 TO WS-COUNT
    MOVE WS-COUNT TO SEEN
    GOBACK.
"#;

fn sources() -> Vec<(&'static str, &'static str)> {
    vec![
        ("get-customer.cbl", GET_CUSTOMER),
        ("admin-only.cbl", ADMIN_ONLY),
        ("ping.cbl", PING),
        ("secret-helper.cbl", SECRET_HELPER),
        ("slow.cbl", SLOW),
        ("table-echo.cbl", TABLE_ECHO),
        ("counter.cbl", COUNTER),
    ]
}

fn new_rig(name: &str, extra: &str) -> Rig {
    rig(backend_dir(name, EXPORTS, &sources()), extra)
}

async fn ready(rig: &Rig, token: &str) -> Raw {
    ready_as(rig, token, "1.2.0").await
}

async fn ready_as(rig: &Rig, token: &str, contract: &str) -> Raw {
    let mut c = Raw::connect(rig, Some(ORIGIN)).await.expect("connect");
    let h = c.hello(contract).await;
    assert_eq!(h["accepted"], "ok", "{h}");
    let a = c.auth(token).await;
    assert_eq!(a["t"], "auth-ok", "{a}");
    c
}

fn good_token(rig: &Rig, sub: &str, extra: Value) -> String {
    rig.issuer.access(sub, AUDIENCE, 600, extra)
}

const REQ: &str = r#"{"CUST-ID":4250,"CUST-NAME":"","BALANCE":0,"PASSWORD":"hunter2"}"#;

// ── AC1: only exported programs answer ───────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn an_exported_program_answers_and_nothing_else_does() {
    let rig = new_rig("ac1", "");
    let token = good_token(&rig, "user-42", json!({}));
    let mut c = ready(&rig, &token).await;

    let r = c.call(1, "GET-CUSTOMER", &[("ref", REQ)], true).await;
    assert_eq!(r["t"], "reply", "{r}");
    assert_eq!(r["args"][0]["CUST-NAME"], "user-42", "the program read the caller's subject through WS-CLAIM");
    assert!((r["args"][0]["BALANCE"].as_f64().unwrap() - 42.5).abs() < 1e-9, "{r}");
    assert_eq!(r["returning"]["RC"], 7);
    assert_eq!(r["returning"]["TXT"], "found");

    // Whatever the spelling, a program that is not exported does not exist.
    let mut tried = 0;
    for name in ["SECRET-HELPER", "secret-helper", " SECRET-HELPER ", "../SECRET-HELPER", "GET-CUSTOMER/../SECRET-HELPER", "SECRET-HELPER\u{0}", "Secret-Helper", "", "GET-CUSTOMER2"] {
        let r = c.call(100 + tried, name, &[("ref", "{\"X\":\"\"}")], false).await;
        assert_eq!(class(&r), "not-found", "{name:?}: {r}");
        assert_eq!(code(&r), 5000);
        tried += 1;
    }
    rig.server.stop();
}

// ── AC2: the token matrix ────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn tokens_are_accepted_and_refused_for_the_right_reasons() {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    let rig = new_rig("ac2", "");
    let now = cobolt_server::jwt::now();
    let claims = |aud: &str, iss: &str, exp: i64, extra: Value| {
        let mut c = json!({"iss": iss, "sub": "u1", "aud": aud, "exp": exp});
        for (k, v) in extra.as_object().unwrap() {
            c[k] = v.clone();
        }
        c
    };
    let other = Issuer::new("https://idp.example.test/pool", "k1"); // same kid, different key
    let mut cases: Vec<(&str, String, u64)> = Vec::new();
    cases.push(("no token", String::new(), 2000));
    cases.push(("not a JWT", "abc".into(), 2001));
    cases.push(("signed by a different key", other.mint(&claims(AUDIENCE, &rig.issuer.issuer, now + 600, json!({}))), 2001));
    let none_header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
    let payload = URL_SAFE_NO_PAD.encode(claims(AUDIENCE, &rig.issuer.issuer, now + 600, json!({})).to_string());
    cases.push(("alg none", format!("{none_header}.{payload}."), 2001));
    cases.push(("alg none with a signature part", format!("{none_header}.{payload}.AAAA"), 2001));
    // HS256 signed with the (public) JWKS text as the secret: the algorithm-confusion attack.
    let hs = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &claims(AUDIENCE, &rig.issuer.issuer, now + 600, json!({})),
        &jsonwebtoken::EncodingKey::from_secret(rig.issuer.jwks().as_bytes()),
    )
    .unwrap();
    cases.push(("HS256 with the public key as the secret", hs, 2001));
    cases.push(("expired", rig.issuer.mint(&claims(AUDIENCE, &rig.issuer.issuer, now - 3600, json!({}))), 2002));
    cases.push(("not yet valid", rig.issuer.mint(&claims(AUDIENCE, &rig.issuer.issuer, now + 7200, json!({"nbf": now + 3600}))), 2001));
    cases.push(("another audience", rig.issuer.mint(&claims("someone-else", &rig.issuer.issuer, now + 600, json!({}))), 2001));
    cases.push(("another issuer", rig.issuer.mint(&claims(AUDIENCE, "https://evil.example.test/pool", now + 600, json!({}))), 2001));
    cases.push(("an identity token (token_use)", rig.issuer.mint(&claims(AUDIENCE, &rig.issuer.issuer, now + 600, json!({"token_use": "id"}))), 2001));
    cases.push(("an identity token (at_hash)", rig.issuer.mint(&claims(AUDIENCE, &rig.issuer.issuer, now + 600, json!({"at_hash": "abc"}))), 2001));
    cases.push(("an identity token (nonce)", rig.issuer.mint(&claims(AUDIENCE, &rig.issuer.issuer, now + 600, json!({"nonce": "n"}))), 2001));
    cases.push(("no subject", rig.issuer.mint(&json!({"iss": rig.issuer.issuer, "aud": AUDIENCE, "exp": now + 600})), 2001));

    let mut refused = 0;
    for (label, token, want) in &cases {
        let mut c = Raw::connect(&rig, Some(ORIGIN)).await.unwrap();
        c.hello("1.2.0").await;
        let a = c.auth(token).await;
        assert_eq!(a["t"], "error", "{label}: {a}");
        assert_eq!(class(&a), "authentication", "{label}: {a}");
        assert_eq!(code(&a), *want, "{label}: {a}");
        // A call made with that token in the envelope is refused too, and the program does not run.
        let call = format!(
            "{{\"t\":\"call\",\"id\":5,\"program\":\"GET-CUSTOMER\",\"args\":[{{\"mode\":\"ref\",\"json\":{REQ}}}],\"token\":{}}}",
            serde_json::to_string(token).unwrap()
        );
        c.send_text(&call).await;
        let r = c.next().await.unwrap();
        assert_eq!(class(&r), "authentication", "{label}: {r}");
        refused += 1;
    }
    // …and a good one is accepted.
    let mut c = ready(&rig, &good_token(&rig, "u1", json!({}))).await;
    let r = c.call(1, "PING", &[], true).await;
    assert_eq!(r["returning"]["REPLY"], "pong");
    println!("\n  token matrix: {refused} bad tokens refused, each twice (auth message and per-call token); 1 good token accepted; issuer keys fetched {} time(s)", rig.server.jwks_fetches());
    assert!(rig.server.jwks_fetches() <= 2, "the keys are cached, not fetched per token");
    rig.server.stop();
}

// ── AC3: roles, and the claims a program can read ────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn a_role_is_checked_before_the_program_starts() {
    let rig = new_rig("ac3", "");
    let mut clerk = ready(&rig, &good_token(&rig, "clerk-1", json!({"roles": ["clerk"]}))).await;
    let r = clerk.call(1, "ADMIN-ONLY", &[("ref", "{\"MSG\":\"\"}")], false).await;
    assert_eq!(class(&r), "authorisation", "{r}");
    assert_eq!(code(&r), 3000);
    // The program did not start: no `ok` record names it.
    assert!(!rig.sink.records().iter().any(|x| x.program.as_deref() == Some("ADMIN-ONLY") && x.outcome.as_deref() == Some("ok")));

    let mut admin = ready(&rig, &good_token(&rig, "admin-1", json!({"roles": ["admin", "clerk"]}))).await;
    let r = admin.call(2, "ADMIN-ONLY", &[("ref", "{\"MSG\":\"\"}")], false).await;
    assert_eq!(r["t"], "reply", "{r}");
    assert_eq!(r["args"][0]["MSG"], "admin ok");
    rig.server.stop();
}

// ── AC4: origins, and ws:// ──────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn a_foreign_origin_is_refused_and_plain_ws_is_loopback_only() {
    let rig = new_rig("ac4", "");
    for origin in [Some("https://evil.example.test"), Some("null"), None, Some("https://app.example.test.evil.test"), Some("http://app.example.test")] {
        let r = Raw::connect(&rig, origin).await;
        assert!(r.is_err(), "origin {origin:?} must be refused, got a connection");
        assert!(r.err().unwrap().contains("403"), "refused with 403");
    }
    assert!(Raw::connect(&rig, Some(ORIGIN)).await.is_ok());
    assert_eq!(rig.server.stats().refused_origin.load(std::sync::atomic::Ordering::SeqCst), 5);

    // ws:// to a non-loopback address: the configuration is refused.
    let cfg = cobolt_server::config::ServerConfig::from_toml("[server]\nlisten = \"0.0.0.0:8080\"\n[auth]\nissuer = [\"https://i.example.test\"]\naudiences = [\"a\"]\n").unwrap();
    let e = cfg.resolve(cobolt_server::config::RunMode::Release, std::path::Path::new(".")).unwrap_err();
    assert!(e.iter().any(|m| m.contains("not a loopback address") && m.contains("TLS")), "{e:?}");
    // …and the relaxation that once allowed it was removed (WASM-003 D4): a file naming it does not load.
    let cfg = cobolt_server::config::ServerConfig::from_toml("unsafe_plain_ws = true\n[server]\nlisten = \"0.0.0.0:8080\"\nallowed_origins=[\"https://a.test\"]\n[auth]\nissuer = [\"https://i.example.test\"]\naudiences = [\"a\"]\n");
    assert!(cfg.is_err_and(|e| e.contains("was removed")));
    rig.server.stop();
}

// ── AC5: payloads are validated against the layout, and the program does not run ─

#[tokio::test(flavor = "multi_thread")]
async fn a_payload_that_does_not_fit_is_refused_and_the_program_does_not_run() {
    let rig = new_rig("ac5", "");
    let mut c = ready(&rig, &good_token(&rig, "u", json!({}))).await;
    let bad: Vec<(&str, &str, &str)> = vec![
        ("unknown field", r#"{"CUST-ID":1,"EXTRA":"x"}"#, "unknown-key"),
        ("a number field holding text", r#"{"CUST-ID":"12"}"#, "wrong-type"),
        ("a string longer than its PIC", r#"{"PASSWORD":"0123456789abc"}"#, "too-long"),
        ("a number beyond its digits", r#"{"CUST-ID":1234567}"#, "overflow"),
        ("not an object", r#"[1,2]"#, "wrong-type"),
        ("a repeated key", r#"{"CUST-ID":1,"CUST-ID":2}"#, "duplicate-key"),
        ("not JSON", r#"{"CUST-ID":01}"#, ""),
    ];
    for (i, (label, payload, want)) in bad.iter().enumerate() {
        // `not JSON` cannot even be embedded in an envelope, so it is sent as a bad frame.
        if want.is_empty() {
            c.send_text(&format!("{{\"t\":\"call\",\"id\":{},\"program\":\"GET-CUSTOMER\",\"args\":[{{\"mode\":\"ref\",\"json\":{payload}}}]}}", 10 + i)).await;
            let r = c.next().await.unwrap();
            assert_eq!(class(&r), "validation", "{label}: {r}");
            continue;
        }
        let r = c.call(10 + i as u64, "GET-CUSTOMER", &[("ref", payload)], false).await;
        assert_eq!(class(&r), "validation", "{label}: {r}");
        assert_eq!(code(&r), 4000);
        assert!(r["message"].as_str().unwrap().starts_with(want), "{label}: {r}");
    }
    let r = c.call(50, "TABLE-ECHO", &[("ref", r#"{"N":2,"ITEM":[1,2,3,4,5]}"#)], false).await;
    assert_eq!(class(&r), "validation", "a table longer than its OCCURS: {r}");
    let r = c.call(51, "TABLE-ECHO", &[("ref", r#"{"N":9}"#)], false).await;
    assert_eq!(class(&r), "validation", "a count outside its bounds: {r}");
    // wrong number of arguments
    let r = c.call(52, "GET-CUSTOMER", &[], false).await;
    assert_eq!(class(&r), "validation", "{r}");
    assert!(!rig.sink.records().iter().any(|x| x.outcome.as_deref() == Some("ok") && x.program.as_deref() == Some("GET-CUSTOMER")), "the program never ran");
    // A good one still works afterwards (the connection survives a refusal).
    let r = c.call(60, "TABLE-ECHO", &[("ref", r#"{"N":3,"ITEM":[1,2,3]}"#)], false).await;
    assert_eq!(r["args"][0]["ITEM"], json!([2, 4, 6]));
    rig.server.stop();
}

// ── AC6: frame size, time and rate limits each get their own error ───────────

#[tokio::test(flavor = "multi_thread")]
async fn size_time_and_rate_limits_are_each_refused_with_their_own_error() {
    let rig = new_rig("ac6", "[limits]\nmax_frame_bytes = 4096\ncall_timeout_ms = 700\nconnection_rate_per_sec = 2\nconnection_burst = 4\n");
    let token = good_token(&rig, "u", json!({}));

    // Time: a program that never ends is stopped and the caller told.
    let mut c = ready(&rig, &token).await;
    let t0 = Instant::now();
    let r = c.call(1, "SLOW", &[("ref", r#"{"N":1}"#)], false).await;
    assert_eq!(class(&r), "limit", "{r}");
    assert_eq!(code(&r), 7003, "{r}");
    let waited = t0.elapsed();
    assert!(waited >= Duration::from_millis(600) && waited < Duration::from_secs(5), "{waited:?}");

    // Rate: the burst is 4 (one used above, plus the hello/auth that are not calls).
    let mut limited = 0;
    let mut answered = 0;
    for i in 0..10u64 {
        let r = c.call(100 + i, "PING", &[], true).await;
        match class(&r).as_str() {
            "limit" => {
                assert_eq!(code(&r), 7001);
                limited += 1;
            }
            _ => answered += 1,
        }
    }
    assert!(limited >= 4 && answered >= 1, "answered {answered}, limited {limited}");

    // Size: a frame over the cap ends the connection after naming the limit.
    let mut c2 = ready(&rig, &token).await;
    let big = "x".repeat(8000);
    c2.send_text(&format!("{{\"t\":\"call\",\"id\":1,\"program\":\"PING\",\"args\":[{{\"mode\":\"value\",\"json\":\"{big}\"}}]}}")).await;
    let r = c2.next().await.unwrap();
    assert_eq!(class(&r), "limit");
    assert_eq!(code(&r), 7000);
    assert!(c2.next().await.is_none(), "the connection is closed");
    println!("\n  limits: SLOW stopped after {waited:?} (limit 700 ms); rate: {answered} answered, {limited} refused of 10 in a burst; oversized frame (8 kB over 4 kB) refused with 7000 and closed");
    rig.server.stop();
}

// ── AC7: concurrent callers do not see each other ────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_callers_get_their_own_results_and_no_state_survives_a_call() {
    let rig = new_rig("ac7", "[limits]\nconnection_rate_per_sec = 1000\nconnection_burst = 1000\nuser_rate_per_sec = 5000\nuser_burst = 5000\nmax_calls_per_connection = 32\n");
    let addr = rig.server.addr;
    let cert = rig.cert.cert_pem.clone();
    let mut handles = Vec::new();
    let t0 = Instant::now();
    for u in 0..8u64 {
        let token = good_token(&rig, &format!("user-{u}"), json!({}));
        let cert = cert.clone();
        handles.push(tokio::spawn(async move {
            let mut c = Raw::connect_to(addr, &cert, Some(ORIGIN)).await.unwrap();
            c.hello("1.2.0").await;
            c.auth(&token).await;
            let mut seen = Vec::new();
            for k in 0..25u64 {
                let id = 1000 * u + k;
                let r = c.call(id, "GET-CUSTOMER", &[("ref", &format!("{{\"CUST-ID\":{},\"CUST-NAME\":\"\",\"BALANCE\":0,\"PASSWORD\":\"\"}}", 100 * (u + 1) + k))], true).await;
                assert_eq!(r["t"], "reply", "{r}");
                assert_eq!(r["args"][0]["CUST-NAME"], format!("user-{u}"), "another caller's data leaked");
                let expect = (100 * (u + 1) + k) as f64 / 100.0;
                assert!((r["args"][0]["BALANCE"].as_f64().unwrap() - expect).abs() < 1e-9, "{r}");
                // Independent counter: fresh WORKING-STORAGE per call, so always 1.
                let c1 = c.call(id + 500, "COUNTER", &[("ref", "{\"WHO\":\"x\",\"SEEN\":0}")], false).await;
                assert_eq!(c1["args"][0]["SEEN"], 1, "state survived a call: {c1}");
                seen.push(id);
            }
            seen.len()
        }));
    }
    let mut total = 0;
    for h in handles {
        total += h.await.unwrap();
    }
    let secs = t0.elapsed().as_secs_f64();
    println!("\n  isolation: 8 concurrent users x 25 rounds x 2 programs = {} calls in {:.2} s ({:.0} calls/s), every answer its caller's own", total * 2, secs, (total * 2) as f64 / secs);
    assert_eq!(total, 200);
    rig.server.stop();
}

// ── AC8: no token, no password in the logs ───────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn the_log_has_neither_the_token_nor_a_field_the_program_was_given() {
    let rig = new_rig("ac8", "");
    let token = good_token(&rig, "user-8", json!({}));
    let mut c = ready(&rig, &token).await;
    let r = c.call(1, "GET-CUSTOMER", &[("ref", REQ)], true).await;
    assert_eq!(r["t"], "reply");
    let bad = rig.issuer.access("user-8", "someone-else", 600, json!({}));
    let mut c2 = Raw::connect(&rig, Some(ORIGIN)).await.unwrap();
    c2.hello("1.2.0").await;
    c2.auth(&bad).await;
    let log = rig.sink.lines().join("\n");
    assert!(!log.contains(&token) && !log.contains(&bad), "a token is in the log");
    for piece in token.split('.') {
        assert!(!log.contains(piece), "a part of the token is in the log");
    }
    assert!(!log.contains("hunter2"), "a field of the payload is in the log");
    assert!(log.contains("\"call\"") && log.contains("user-8") && log.contains("GET-CUSTOMER") && log.contains("duration_ms"), "the log still says who did what: {log}");
    println!("\n  log: {} records, {} bytes, no token (whole or in part) and no payload field", rig.sink.records().len(), log.len());
    rig.server.stop();
}

// ── versions (WASM-014 R2, R3) ───────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn the_versions_decide_at_the_hello() {
    let rig = new_rig("ver", "");
    for (client, protocol, accepted, needle) in [
        ("1.2.0", 1, "ok", ""),
        ("1.2.7", 1, "ok", ""),
        ("1.0.0", 1, "warn", "compatible"),
        ("1.3.0", 1, "refused", "update the backend"),
        ("0.9.0", 1, "refused", "update the application"),
        ("2.0.0", 1, "refused", "update the backend"),
        ("1.2.0", 0, "refused", "update the application"),
        ("1.2.0", 9, "refused", "update the backend"),
        ("one.two", 1, "refused", "major.minor.patch"),
    ] {
        let mut c = Raw::connect(&rig, Some(ORIGIN)).await.unwrap();
        c.send(json!({"t": "hello", "protocol": protocol, "contract": client, "client": "t"})).await;
        let h = c.next().await.unwrap();
        assert_eq!(h["t"], "hello", "a refusal is still a hello, never a decoding error: {h}");
        assert_eq!(h["accepted"], accepted, "{client}/{protocol}: {h}");
        assert!(h["message"].as_str().unwrap().contains(needle), "{client}/{protocol}: {h}");
        if accepted == "refused" {
            assert!(c.next().await.is_none(), "closed after a refusal");
        }
    }
    // A call before the hello is not served.
    let mut c = Raw::connect(&rig, Some(ORIGIN)).await.unwrap();
    let r = c.call(1, "PING", &[], true).await;
    assert_eq!(class(&r), "validation");
    rig.server.stop();
}

// ── the health probe ─────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn the_health_probe_answers_on_the_tls_port_and_carries_the_security_headers() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let rig = new_rig("health", "");
    let tcp = tokio::net::TcpStream::connect(rig.server.addr).await.unwrap();
    let connector = cobolt_server::tlsutil::connector(Some(rig.cert.cert_pem.as_bytes())).unwrap();
    let mut tls = connector.connect("localhost", tcp).await.unwrap();
    tls.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n").await.unwrap();
    let mut body = String::new();
    tls.read_to_string(&mut body).await.unwrap();
    assert!(body.starts_with("HTTP/1.1 200"), "{body}");
    assert!(body.contains("Strict-Transport-Security") && body.contains("X-Content-Type-Options: nosniff") && body.contains("Referrer-Policy: no-referrer"), "{body}");
    assert!(body.contains("\"status\":\"ok\"") && body.contains("1.2.0"), "{body}");
    // and an unknown path is a plain 404
    let tcp = tokio::net::TcpStream::connect(rig.server.addr).await.unwrap();
    let mut tls = connector.connect("localhost", tcp).await.unwrap();
    tls.write_all(b"GET /admin HTTP/1.1\r\nHost: localhost\r\n\r\n").await.unwrap();
    let mut body = String::new();
    tls.read_to_string(&mut body).await.unwrap();
    assert!(body.starts_with("HTTP/1.1 404"), "{body}");
    rig.server.stop();
}

// ── R19: calls that share a file take turns ──────────────────────────────────

fn file_programs(path: &str) -> Vec<(String, String)> {
    let select = format!(
        r#"ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT ROWS ASSIGN TO "{path}"
        ORGANIZATION IS INDEXED
        ACCESS MODE IS DYNAMIC
        RECORD KEY IS ROW-KEY
        FILE STATUS IS WS-ST.
DATA DIVISION.
FILE SECTION.
FD ROWS.
01 ROW-REC.
    05 ROW-KEY PIC X(8).
    05 ROW-VAL PIC X(8).
WORKING-STORAGE SECTION.
01 WS-ST PIC XX.
01 WS-N PIC 9(6) VALUE 0.
01 WS-MORE PIC X VALUE "Y".
"#
    );
    let create = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. MAKE-ROWS.\n{select}LINKAGE SECTION.\n01 LK-X.\n    05 ST PIC XX.\nPROCEDURE DIVISION USING LK-X.\nMAIN.\n    OPEN OUTPUT ROWS\n    MOVE WS-ST TO ST\n    CLOSE ROWS\n    GOBACK.\n"
    );
    let add = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. ADD-ROW.\n{select}LINKAGE SECTION.\n01 LK-ROW.\n    05 K PIC X(8).\n    05 V PIC X(8).\n    05 ST PIC XX.\nPROCEDURE DIVISION USING LK-ROW.\nMAIN.\n    OPEN I-O ROWS\n    IF WS-ST NOT = \"00\"\n        MOVE WS-ST TO ST\n        GOBACK\n    END-IF\n    MOVE K TO ROW-KEY\n    MOVE V TO ROW-VAL\n    WRITE ROW-REC\n    MOVE WS-ST TO ST\n    CLOSE ROWS\n    GOBACK.\n"
    );
    let count = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. COUNT-ROWS.\n{select}LINKAGE SECTION.\n01 LK-C.\n    05 N PIC 9(6).\n    05 ST PIC XX.\nPROCEDURE DIVISION USING LK-C.\nMAIN.\n    OPEN INPUT ROWS\n    MOVE WS-ST TO ST\n    PERFORM UNTIL WS-MORE = \"N\"\n        READ ROWS NEXT\n            AT END MOVE \"N\" TO WS-MORE\n            NOT AT END ADD 1 TO WS-N\n        END-READ\n    END-PERFORM\n    MOVE WS-N TO N\n    CLOSE ROWS\n    GOBACK.\n"
    );
    vec![("make-rows.cbl".into(), create), ("add-row.cbl".into(), add), ("count-rows.cbl".into(), count)]
}

#[tokio::test(flavor = "multi_thread")]
async fn calls_that_share_a_file_take_turns_and_none_is_lost() {
    let data = std::env::temp_dir().join(format!("prc-r19-{}.idx", std::process::id()));
    let _ = std::fs::remove_file(&data);
    let _ = std::fs::remove_file(format!("{}.lck", data.display()));
    let exports = "[contract]\nversion = \"1.0.0\"\n[[export]]\nprogram = \"MAKE-ROWS\"\n[[export]]\nprogram = \"ADD-ROW\"\nindependent = true\n[[export]]\nprogram = \"COUNT-ROWS\"\nindependent = true\n";
    let progs = file_programs(&data.display().to_string());
    let srcs: Vec<(&str, &str)> = progs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let rig = rig(backend_dir("r19", exports, &srcs), "[limits]\nconnection_rate_per_sec = 1000\nconnection_burst = 1000\nuser_rate_per_sec = 5000\nuser_burst = 5000\nmax_calls_per_connection = 64\nfile_lock_wait_ms = 20000\ncall_timeout_ms = 60000\n");
    let token = good_token(&rig, "writer", json!({}));
    let mut c = ready_as(&rig, &token, "1.0.0").await;
    let r = c.call(1, "MAKE-ROWS", &[("ref", r#"{"ST":""}"#)], false).await;
    assert_eq!(r["args"][0]["ST"], "00", "{r}");

    // Many callers write different keys at the same moment.
    let writers = 24u64;
    let t0 = Instant::now();
    let mut handles = Vec::new();
    for i in 0..writers {
        let (addr, cert, token) = (rig.server.addr, rig.cert.cert_pem.clone(), token.clone());
        handles.push(tokio::spawn(async move {
            let mut c = Raw::connect_to(addr, &cert, Some(ORIGIN)).await.unwrap();
            c.hello("1.0.0").await;
            c.auth(&token).await;
            let r = c.call(i, "ADD-ROW", &[("ref", &format!("{{\"K\":\"key{i:05}\",\"V\":\"val{i:05}\",\"ST\":\"\"}}"))], false).await;
            r["args"][0]["ST"].as_str().unwrap_or("??").to_string()
        }));
    }
    let mut statuses = Vec::new();
    for h in handles {
        statuses.push(h.await.unwrap());
    }
    let wrote = t0.elapsed();
    let ok = statuses.iter().filter(|s| *s == "00").count();
    assert_eq!(ok as u64, writers, "every concurrent writer got its turn: {statuses:?}");

    // …and a reader sees exactly what was written.
    let r = c.call(500, "COUNT-ROWS", &[("ref", r#"{"N":0,"ST":""}"#)], false).await;
    assert_eq!(r["args"][0]["ST"], "00", "{r}");
    assert_eq!(r["args"][0]["N"], writers, "no write was lost or torn: {r}");
    println!("\n  shared file: {writers} concurrent writers into one indexed file in {:.0} ms ({:.0} writes/s); a reader then counted {writers} rows; every OPEN got status 00", wrote.as_secs_f64() * 1000.0, writers as f64 / wrote.as_secs_f64());
    rig.server.stop();
    let _ = std::fs::remove_file(&data);
    let _ = std::fs::remove_file(format!("{}.lck", data.display()));
}

// ── WASM-003 Q2: `per-connection` state (T5) ─────────────────────────────────

/// The COUNTER a call answers, as a number.
async fn count(c: &mut Raw, id: u64) -> u64 {
    let r = c.call(id, "COUNTER", &[("ref", r#"{"WHO":"","SEEN":0}"#)], false).await;
    assert_eq!(r["t"], "reply", "{r}");
    r["args"][0]["SEEN"].as_u64().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn per_connection_state_is_kept_and_never_shared() {
    use cobolt_server::backend::StateMode;
    let rig = rig_with_state(backend_dir("q2-conn", EXPORTS, &sources()), "", StateMode::PerConnection);
    let (alice, bob) = (good_token(&rig, "alice", json!({})), good_token(&rig, "bob", json!({})));
    let mut a = ready(&rig, &alice).await;
    let mut b = ready(&rig, &bob).await;

    // Interleaved: each connection counts on its own.
    let (mut seen_a, mut seen_b) = (Vec::new(), Vec::new());
    for i in 0..3 {
        seen_a.push(count(&mut a, 10 + i).await);
        seen_b.push(count(&mut b, 10 + i).await);
    }
    assert_eq!(seen_a, [1, 2, 3], "alice's connection keeps its WORKING-STORAGE");
    assert_eq!(seen_b, [1, 2, 3], "bob's connection has its own");

    // A call carrying another subject's token is refused before the program runs.
    a.send(json!({"t": "call", "id": 20, "program": "COUNTER", "args": [{"mode": "ref", "json": {"WHO": "", "SEEN": 0}}], "returning": false, "token": bob})).await;
    let r = a.next().await.unwrap();
    assert_eq!(class(&r), "authorisation", "{r}");
    assert_eq!(count(&mut a, 21).await, 4, "the refused call left alice's state alone");

    // A new connection starts fresh: nothing is resumed.
    drop(a);
    let mut a2 = ready(&rig, &alice).await;
    let reconnected = count(&mut a2, 30).await;
    assert_eq!(reconnected, 1);
    let records = rig.sink.records();
    assert!(records.iter().filter(|r| r.event == "call" && r.outcome.as_deref() == Some("ok")).all(|r| r.state == Some("per-connection")), "the log names the mode");
    rig.server.stop();

    // The default keeps nothing.
    let rig = new_rig("q2-call", "");
    let mut c = ready(&rig, &good_token(&rig, "alice", json!({}))).await;
    let mut per_call = Vec::new();
    for i in 0..3 {
        per_call.push(count(&mut c, 40 + i).await);
    }
    assert_eq!(per_call, [1, 1, 1]);
    assert!(rig.sink.records().iter().filter(|r| r.event == "call").all(|r| r.state == Some("per-call")));
    println!("\n  per-connection: alice {seen_a:?}, bob {seen_b:?} (interleaved); bob's token on alice's connection → {}; alice again → 4; reconnected → {reconnected}\n  per-call: {per_call:?}", code(&r));
    rig.server.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_held_open_across_calls_makes_others_wait_or_93() {
    use cobolt_server::backend::StateMode;
    let data = std::env::temp_dir().join(format!("prc-q2-hold-{}.idx", std::process::id()));
    let _ = std::fs::remove_file(&data);
    let _ = std::fs::remove_file(format!("{}.lck", data.display()));
    let mut progs = file_programs(&data.display().to_string());
    // HOLD opens the file on one call and closes it on a later one.
    let hold = progs[1].1.replace("PROGRAM-ID. ADD-ROW.", "PROGRAM-ID. HOLD.").replace(
        "LINKAGE SECTION.\n01 LK-ROW.\n    05 K PIC X(8).\n    05 V PIC X(8).\n    05 ST PIC XX.\nPROCEDURE DIVISION USING LK-ROW.\nMAIN.\n    OPEN I-O ROWS\n    IF WS-ST NOT = \"00\"\n        MOVE WS-ST TO ST\n        GOBACK\n    END-IF\n    MOVE K TO ROW-KEY\n    MOVE V TO ROW-VAL\n    WRITE ROW-REC\n    MOVE WS-ST TO ST\n    CLOSE ROWS\n    GOBACK.\n",
        "LINKAGE SECTION.\n01 LK-H.\n    05 OP PIC X(5).\n    05 ST PIC XX.\nPROCEDURE DIVISION USING LK-H.\nMAIN.\n    IF OP = \"OPEN\"\n        OPEN I-O ROWS\n    ELSE\n        CLOSE ROWS\n    END-IF\n    MOVE WS-ST TO ST\n    GOBACK.\n",
    );
    assert!(hold.contains("PROGRAM-ID. HOLD.") && hold.contains("OP = \"OPEN\""), "the HOLD program was made");
    progs.push(("hold.cbl".into(), hold));
    let exports = "[contract]\nversion = \"1.0.0\"\n[[export]]\nprogram = \"MAKE-ROWS\"\n[[export]]\nprogram = \"ADD-ROW\"\n[[export]]\nprogram = \"HOLD\"\n";
    let srcs: Vec<(&str, &str)> = progs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let wait_ms = 400u64;
    let rig = rig_with_state(backend_dir("q2-hold", exports, &srcs), &format!("[limits]\nfile_lock_wait_ms = {wait_ms}\n"), StateMode::PerConnection);
    let token = good_token(&rig, "keeper", json!({}));
    let mut holder = ready_as(&rig, &token, "1.0.0").await;
    let r = holder.call(1, "MAKE-ROWS", &[("ref", r#"{"ST":""}"#)], false).await;
    assert_eq!(r["args"][0]["ST"], "00", "{r}");
    let r = holder.call(2, "HOLD", &[("ref", r#"{"OP":"OPEN","ST":""}"#)], false).await;
    assert_eq!(r["args"][0]["ST"], "00", "{r}");

    // Another connection: its OPEN waits for the configured time, then answers 93 — it never hangs.
    let other_token = good_token(&rig, "other", json!({}));
    let mut other = ready_as(&rig, &other_token, "1.0.0").await;
    let t0 = Instant::now();
    let r = other.call(3, "ADD-ROW", &[("ref", r#"{"K":"k1","V":"v1","ST":""}"#)], false).await;
    let waited = t0.elapsed();
    assert_eq!(r["args"][0]["ST"], "93", "{r}");
    assert!(waited >= Duration::from_millis(wait_ms) && waited < Duration::from_millis(wait_ms + 5000), "waited {waited:?}");

    // The holder closes on a later call; the other connection's OPEN then succeeds.
    let r = holder.call(4, "HOLD", &[("ref", r#"{"OP":"CLOSE","ST":""}"#)], false).await;
    assert_eq!(r["args"][0]["ST"], "00", "the file was still open in the session: {r}");
    let r = other.call(5, "ADD-ROW", &[("ref", r#"{"K":"k2","V":"v2","ST":""}"#)], false).await;
    assert_eq!(r["args"][0]["ST"], "00", "{r}");

    // A dropped connection releases what its session held.
    let r = holder.call(6, "HOLD", &[("ref", r#"{"OP":"OPEN","ST":""}"#)], false).await;
    assert_eq!(r["args"][0]["ST"], "00", "{r}");
    drop(holder);
    let t1 = Instant::now();
    let mut after = "??".to_string();
    while t1.elapsed() < Duration::from_secs(10) {
        let r = other.call(7 + t1.elapsed().as_millis() as u64, "ADD-ROW", &[("ref", r#"{"K":"k3","V":"v3","ST":""}"#)], false).await;
        after = r["args"][0]["ST"].as_str().unwrap_or("??").to_string();
        if after == "00" {
            break;
        }
    }
    assert_eq!(after, "00", "the lock went with the connection");
    println!("\n  per-connection file lock: another connection waited {:.0} ms (limit {wait_ms}) → 93; after CLOSE on the holder's next call → 00; after the holder disconnected → 00 in {:.0} ms", waited.as_secs_f64() * 1000.0, t1.elapsed().as_secs_f64() * 1000.0);
    rig.server.stop();
    let _ = std::fs::remove_file(&data);
    let _ = std::fs::remove_file(format!("{}.lck", data.display()));
}

// ── the HTTPS front (WASM-003 R2, Q3; D6): a closed route table ──────────────

/// Send `request` over TLS and read until the server closes; the status, or 0 when nothing came back.
async fn raw_request(rig: &Rig, request: &[u8]) -> (u16, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let tcp = tokio::net::TcpStream::connect(rig.server.addr).await.unwrap();
    let connector = cobolt_server::tlsutil::connector(Some(rig.cert.cert_pem.as_bytes())).unwrap();
    let mut tls = connector.connect("localhost", tcp).await.unwrap();
    let _ = tls.write_all(request).await;
    let mut raw = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(15), tls.read_to_end(&mut raw)).await;
    let text = String::from_utf8_lossy(&raw).into_owned();
    (text.split(' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0), text)
}

#[tokio::test(flavor = "multi_thread")]
async fn routes_are_closed() {
    // A short handshake limit: an upgraded test connection that never says hello is closed soon.
    let rig = new_rig("routes", "[limits]\nhandshake_timeout_ms = 1000\n");
    let upgrade = |path: &str| format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nOrigin: {ORIGIN}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n");
    let get = |path: &str| format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n");
    let cases: Vec<(String, String, u16)> = vec![
        ("GET /health".into(), get("/health"), 200),
        ("POST /health".into(), "POST /health HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n".into(), 404),
        ("GET /dl/<bad>".into(), get(&format!("/dl/{}", "0".repeat(64))), 404),
        ("GET /dl/../etc/passwd".into(), get("/dl/../etc/passwd"), 404),
        ("GET /ws, no upgrade".into(), get("/ws"), 404),
        ("GET /, no upgrade".into(), get("/"), 404),
        ("GET /admin".into(), get("/admin"), 404),
        ("GET /api/anything".into(), get("/api/anything"), 404),
        ("GET /ws/extra".into(), upgrade("/ws/extra"), 404),
        ("GET /health/".into(), get("/health/"), 404),
    ];
    let mut report = Vec::new();
    for (what, req, want) in &cases {
        let (status, text) = raw_request(&rig, req.as_bytes()).await;
        assert_eq!(status, *want, "{what}: {text}");
        if status != 101 {
            assert!(text.contains("Strict-Transport-Security") && text.contains("X-Content-Type-Options: nosniff"), "{what}: {text}");
        }
        report.push(format!("{what} → {status}"));
    }
    // `/` and `/ws` upgrade (header names compare without case).
    for path in ["/", "/ws"] {
        let (status, text) = raw_request(&rig, format!("{}", upgrade(path)).as_bytes()).await;
        assert_eq!(status, 101, "{path}: {text}");
        assert!(text.to_ascii_lowercase().contains("sec-websocket-accept: s3pplmbitxaq9kygzzhzrbk+xoo="), "{text}");
        report.push(format!("upgrade {path} → {status}"));
    }
    println!("\n  routes: {}", report.join("; "));
    rig.server.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_large_or_slow_request_head_is_refused() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let rig = new_rig("head", "[limits]\nhandshake_timeout_ms = 1500\n");
    // A head over the cap.
    let big = format!("GET /health HTTP/1.1\r\nHost: localhost\r\nX-Pad: {}\r\n\r\n", "a".repeat(20 * 1024));
    let (status, _) = raw_request(&rig, big.as_bytes()).await;
    assert_eq!(status, 431);
    // A head that never ends: closed within the handshake time.
    let tcp = tokio::net::TcpStream::connect(rig.server.addr).await.unwrap();
    let connector = cobolt_server::tlsutil::connector(Some(rig.cert.cert_pem.as_bytes())).unwrap();
    let mut tls = connector.connect("localhost", tcp).await.unwrap();
    tls.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n").await.unwrap();
    let t0 = Instant::now();
    let mut rest = Vec::new();
    let r = tokio::time::timeout(Duration::from_secs(10), tls.read_to_end(&mut rest)).await;
    let closed_after = t0.elapsed();
    assert!(r.is_ok(), "the server must close a head that never ends");
    assert!(closed_after < Duration::from_millis(1500 + 2000), "{closed_after:?}");
    assert!(!String::from_utf8_lossy(&rest).starts_with("HTTP/1.1 200"), "no answer to half a head");
    println!("\n  a 20 KiB head → {status}; an unfinished head closed after {:.0} ms (limit 1500)", closed_after.as_secs_f64() * 1000.0);
    rig.server.stop();
}

// ── WASM-003 R25/R26, AC12: the Rust client's retry policy on real sockets ────

const RETRY_EXPORTS: &str = r#"
[contract]
version = "1.0.0"

[[export]]
program = "SAFE-READ"
idempotent = true

[[export]]
program = "UNSAFE-WRITE"
"#;

const SAFE_READ: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. SAFE-READ.
DATA DIVISION.
LINKAGE SECTION.
01 LK-R.
    05 REPLY PIC X(4).
PROCEDURE DIVISION RETURNING LK-R.
MAIN.
    MOVE "safe" TO REPLY
    GOBACK.
"#;

const UNSAFE_WRITE: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. UNSAFE-WRITE.
DATA DIVISION.
LINKAGE SECTION.
01 LK-R.
    05 REPLY PIC X(4).
PROCEDURE DIVISION RETURNING LK-R.
MAIN.
    MOVE "done" TO REPLY
    GOBACK.
"#;

/// A backend reached through a proxy that can refuse every new connection (the backend "stopped":
/// the TLS handshake fails at once) and a client of it with `policy`.
fn retry_world(name: &str, policy: cobolt_wire::retry::Policy) -> (Rig, Proxy, cobolt_server::client::WsClient) {
    use cobolt_server::client::{ClientConfig, StaticToken, WsClient};
    let rig = rig(backend_dir(name, RETRY_EXPORTS, &[("safe-read.cbl", SAFE_READ), ("unsafe-write.cbl", UNSAFE_WRITE)]), "");
    let proxy = Proxy::start(rig.server.addr);
    let mut cfg = ClientConfig::new(format!("wss://localhost:{}/ws", proxy.addr.port()), "1.0.0");
    cfg.extra_root_pem = Some(rig.cert.cert_pem.clone().into_bytes());
    cfg.origin = Some(ORIGIN.to_string());
    cfg.interface = Some(rig.interface());
    cfg.token = Some(std::sync::Arc::new(StaticToken(good_token(&rig, "user-1", json!({})))));
    cfg.retry = policy;
    (rig, proxy, WsClient::new(cfg))
}

fn retry_call(program: &str) -> cobolt_wire::call::CallRequest {
    cobolt_wire::call::CallRequest { program: program.into(), args: vec![], wants_returning: true, idempotency_key: None }
}

/// Run `program` on its own thread; answers its result and how long it took.
fn call_in_background(client: &cobolt_server::client::WsClient, program: &'static str) -> std::thread::JoinHandle<(Result<cobolt_wire::call::CallReply, cobolt_wire::CallError>, Duration)> {
    use cobolt_wire::call::Transport;
    let mut c = client.clone();
    std::thread::spawn(move || {
        let t0 = Instant::now();
        let r = c.call(&retry_call(program));
        (r, t0.elapsed())
    })
}

/// Cut every connection while the answers are being swallowed (so the calls are in flight), with
/// new connections refused: the backend is "stopped".
fn break_with_calls_in_flight(proxy: &Proxy) -> Instant {
    proxy.set(PROXY_SWALLOW_REPLIES);
    std::thread::sleep(Duration::from_millis(400));
    proxy.set(PROXY_REFUSE);
    let at = Instant::now();
    proxy.kill_all();
    at
}

#[test]
fn a_broken_connection_is_retried_by_the_policy_then_given_up() {
    use cobolt_wire::call::Transport;
    use cobolt_wire::error::{codes, ErrorClass};
    let policy = cobolt_wire::retry::Policy { first_ms: 1_500, step_ms: 1_500, attempts: 3 };
    let (rig, proxy, client) = retry_world("retry", policy);
    client.clone().call(&retry_call("SAFE-READ")).expect("warm");

    // 1. The backend comes back before attempt 2: the safe call completes, the other is transport 1001.
    let before = proxy.accepted().len();
    let (safe, unsafe_) = (call_in_background(&client, "SAFE-READ"), call_in_background(&client, "UNSAFE-WRITE"));
    let broke = break_with_calls_in_flight(&proxy);
    std::thread::sleep(Duration::from_millis(500));
    assert!(client.link_status().blocked, "blocked while it retries");
    std::thread::sleep(Duration::from_millis(250));
    proxy.set(PROXY_FORWARD);
    let (safe, _) = safe.join().unwrap();
    let (unsafe_, _) = unsafe_.join().unwrap();
    let back = broke.elapsed();
    let reply = safe.expect("the safe call is repeated on the new connection");
    assert!(reply.returning.as_deref().unwrap_or("").contains("safe"), "{reply:?}");
    let e = unsafe_.expect_err("the other may have run: not repeated");
    assert_eq!((e.class, e.code, e.may_have_run), (ErrorClass::Transport, codes::CLOSED, Some(true)), "{e}");
    let attempts_1: Vec<u128> = proxy.accepted()[before..].iter().map(|t| t.saturating_duration_since(broke).as_millis()).collect();
    assert_eq!(attempts_1.len(), 2, "attempt 1 refused, attempt 2 connected: {attempts_1:?}");
    assert_eq!(client.link_status(), cobolt_server::client::LinkStatus { blocked: false, unreachable: false });
    println!("\n  back before attempt 2: connections at {attempts_1:?} ms after the break, calls ended after {:.0} ms; SAFE-READ repeated, UNSAFE-WRITE transport {} may_have_run=true", back.as_secs_f64() * 1000.0, e.code);

    // 2. Stopped for good: transport after the third attempt, exactly three connections, no fourth.
    let before = proxy.accepted().len();
    let sent = call_in_background(&client, "UNSAFE-WRITE");
    let broke = break_with_calls_in_flight(&proxy);
    std::thread::sleep(Duration::from_millis(2_000));
    let queued = call_in_background(&client, "UNSAFE-WRITE"); // made while blocked: never sent
    let (sent, took) = sent.join().unwrap();
    let (queued, _) = queued.join().unwrap();
    let e = sent.expect_err("given up");
    assert_eq!((e.class, e.code, e.may_have_run), (ErrorClass::Transport, codes::CLOSED, Some(true)), "{e}");
    assert_eq!(e.message, "the backend could not be reached");
    let q = queued.expect_err("given up");
    assert_eq!((q.class, q.code, q.may_have_run), (ErrorClass::Transport, codes::NO_CONNECTION, Some(false)), "{q}");
    let gave_up = broke.elapsed();
    assert_eq!(client.link_status(), cobolt_server::client::LinkStatus { blocked: false, unreachable: true });
    std::thread::sleep(Duration::from_millis(2_000));
    let attempts_2: Vec<u128> = proxy.accepted()[before..].iter().map(|t| t.saturating_duration_since(broke).as_millis()).collect();
    println!("  stopped for good: connections at {attempts_2:?} ms after the break; the sent call failed {} after {:.0} ms, the queued one {}; given up at {:.0} ms (cycle {} ms)", e.code, took.as_secs_f64() * 1000.0, q.code, gave_up.as_secs_f64() * 1000.0, policy.cycle_ms());
    assert_eq!(attempts_2.len(), 3, "exactly three attempts, no fourth: {attempts_2:?}");
    rig.server.stop();
}

/// AC12 at the spec's values (one run, about 60 s): the three windows measured at the backend's
/// address, the waiting call's `ON EXCEPTION` class `transport` after the third, no fourth attempt.
#[test]
fn the_retry_windows_are_10_20_30_seconds() {
    use cobolt_wire::error::{codes, ErrorClass};
    let policy = cobolt_wire::retry::Policy::default();
    let (rig, proxy, client) = retry_world("retry-spec", policy);
    {
        use cobolt_wire::call::Transport;
        client.clone().call(&retry_call("SAFE-READ")).expect("warm");
    }
    let before = proxy.accepted().len();
    let call = call_in_background(&client, "UNSAFE-WRITE");
    let broke = break_with_calls_in_flight(&proxy);
    let (r, _) = call.join().unwrap();
    let gave_up = broke.elapsed();
    let e = r.expect_err("given up");
    assert_eq!((e.class, e.code), (ErrorClass::Transport, codes::CLOSED), "{e}");
    std::thread::sleep(Duration::from_secs(3));
    let at: Vec<f64> = proxy.accepted()[before..].iter().map(|t| t.saturating_duration_since(broke).as_secs_f64()).collect();
    let windows: Vec<f64> = at.windows(2).map(|w| w[1] - w[0]).chain(at.last().map(|l| gave_up.as_secs_f64() - l)).collect();
    println!("\n  attempts at {:?} s after the break; windows {:?} s; given up at {:.2} s; connections after it: {}", at.iter().map(|t| format!("{t:.2}")).collect::<Vec<_>>(), windows.iter().map(|w| format!("{w:.2}")).collect::<Vec<_>>(), gave_up.as_secs_f64(), at.len().saturating_sub(3));
    assert_eq!(at.len(), 3, "three attempts and no fourth: {at:?}");
    for (w, want) in windows.iter().zip([10.0, 20.0, 30.0]) {
        assert!((w - want).abs() < 0.75, "window {w:.2} s, want {want} s: {windows:?}");
    }
    rig.server.stop();
}
