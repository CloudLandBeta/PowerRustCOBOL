// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `cargo run -p cobolt-web-bundle --example mock_backend -- [--port 8096] [--ttl 3600] [--first-ttl 61]`
//!
//! A mock of the project's Backend for trying the player in a real browser (WASM-009, WASM-015): a
//! WebSocket server on the loopback that speaks the backend's protocol as far as the player uses it
//! (`cobolt_wire::proto`, WASM-014 T6: `hello`, `{"t":"auth","token"}`, `{"t":"call","id","key",…}`) and
//! plays an identity provider. The identity requests are the Authentication control's (WASM-009 D5, T21):
//! HTTPS-style requests to `/auth/*` on the same port, as the real backend serves them — a JSON
//! `AUTH-REQUEST` in, an `AUTH-RESULT` out, the refresh "cookie" (`pr_refresh`, `HttpOnly`) set by the
//! answer and never in its body. The hosted flow's `GET /auth/hosted/start` comes straight back to the
//! page with `?pr-code=hosted-code&pr-state=<the page's state>`, as if the provider had signed `hosted` in.
//!
//! | user | password | what it does |
//! |---|---|---|
//! | `ana` | `s3cret!` | signs in |
//! | `mfa` | `s3cret!` | asks for a verification code (`000000`) |
//! | anyone new | any | `sign-up` makes an unconfirmed account; its code is `123456`; `forgot`'s is `654321` |
//!
//! A sign-in carrying a reCAPTCHA token refuses one it has seen before (a replay). The server
//! prints one line per request — the kind, the user, whether a captcha token came — and **never** a
//! password, a code or a token (WASM-009 R10, AC7); the tests read that output. Any other call (the
//! fixture program's `PING`) is answered `pong`. `--first-ttl` gives the first token issued a lifetime
//! of its own (the browser test's forced expiry, WASM-009 T25); every later one lives `--ttl`.

use std::collections::{HashMap, HashSet};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use tungstenite::{accept, Message};

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64url(bytes: &[u8]) -> String {
    let mut out = String::new();
    for ch in bytes.chunks(3) {
        let n = (ch[0] as u32) << 16 | (*ch.get(1).unwrap_or(&0) as u32) << 8 | *ch.get(2).unwrap_or(&0) as u32;
        for i in 0..=ch.len() {
            out.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

/// The lines the server says, kept as well when the browser test runs it in its own process
/// (`tests/auth_browser.rs` sets `Some`); the example itself keeps none.
pub static LOG: Mutex<Option<Vec<String>>> = Mutex::new(None);

fn say(line: String) {
    println!("{line}");
    if let Some(log) = LOG.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        log.push(line);
    }
}

fn now_s() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Default)]
struct State {
    users: HashMap<String, User>,
    /// Tokens this server issued and has not seen signed out.
    live_tokens: HashMap<String, String>,
    /// Users whose "refresh cookie" is valid (a sign-in sets it, a sign-out clears it).
    refresh_ok: HashSet<String>,
    captchas_seen: HashSet<String>,
    ttl: u64,
    /// The lifetime of the first token issued, when it is not `ttl`.
    first_ttl: Option<u64>,
}

struct User {
    password: String,
    confirmed: bool,
    email: String,
}

fn token_for(st: &mut State, user: &str, email: &str, ttl: u64) -> String {
    let payload = json!({"sub": format!("u-{user}"), "preferred_username": user, "email": email, "exp": now_s() + ttl});
    let t = format!("{}.{}.{}", b64url(br#"{"alg":"none","typ":"JWT"}"#), b64url(payload.to_string().as_bytes()), "bW9jaw");
    st.live_tokens.insert(t.clone(), user.to_owned());
    st.refresh_ok.insert(user.to_owned());
    t
}

/// A reply: the player hands the module `{"id","result":<this reply>}`, so the result's members sit
/// beside `t` and `id`.
fn ok(id: u64, result: Value) -> String {
    let mut r = json!({"t": "reply", "id": id, "args": []});
    match result {
        Value::Object(m) => r.as_object_mut().unwrap().extend(m),
        other => r["returning"] = other,
    }
    r.to_string()
}

/// An `AUTH-RESULT` with `OUTCOME` and the given members.
fn outcome(word: &str, members: Value) -> Value {
    let mut r = json!({"OUTCOME": word, "OUTCOME-TEXT": ""});
    if let Value::Object(m) = members {
        r.as_object_mut().unwrap().extend(m);
    }
    r
}

/// One identity operation: the route, the request's members and the user the refresh cookie names.
/// Answers the result and, for a sign-in, the user the cookie should now name (`Some("")` clears it).
fn identity(st: &mut State, route: &str, f: &Value, cookie_user: Option<&str>) -> (Value, Option<String>) {
    let s = |k: &str| f.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
    let user = s("USERNAME");
    let captcha = Some(s("CAPTCHA-TOKEN")).filter(|c| !c.is_empty());
    say(format!("identity={route} user={user} captcha={}", if captcha.is_some() { "present" } else { "absent" }));
    let failed = || (outcome("AUTH-FAILED", json!({})), None);
    // reCAPTCHA: a token is single-use; a replay is refused like a wrong password (R4, R10).
    if let Some(c) = &captcha {
        if !st.captchas_seen.insert(c.clone()) {
            return failed();
        }
    }
    let signed_in = |st: &mut State, user: &str, email: &str| {
        let ttl = st.first_ttl.take().unwrap_or(st.ttl);
        let t = token_for(st, user, email, ttl);
        (outcome("OK", json!({"ACCESS-TOKEN": t, "EXPIRES-IN": ttl, "USERNAME": user})), Some(user.to_owned()))
    };
    match route {
        "sign-in" => match st.users.get(&user) {
            Some(u) if u.password == s("PASSWORD") && !u.confirmed => (outcome("CONFIRMATION-REQUIRED", json!({})), None),
            Some(u) if u.password == s("PASSWORD") && user == "mfa" => {
                let _ = u;
                (outcome("CHALLENGE", json!({"CHALLENGE": "totp", "SESSION": "mfa-session"})), None)
            }
            Some(u) if u.password == s("PASSWORD") => {
                let email = u.email.clone();
                signed_in(st, &user, &email)
            }
            _ => failed(),
        },
        "sign-up" => {
            st.users.insert(user, User { password: s("PASSWORD"), confirmed: false, email: s("EMAIL") });
            (outcome("CONFIRMATION-REQUIRED", json!({})), None)
        }
        "confirm" => match st.users.get_mut(&user) {
            Some(u) if s("ONE-TIME-CODE") == "123456" => {
                u.confirmed = true;
                (outcome("OK", json!({})), None)
            }
            _ => failed(),
        },
        // The same answer whether or not the user exists (R10).
        "forgot" => (outcome("AUTH-RESET-REQUESTED", json!({})), None),
        "reset" => match st.users.get_mut(&user) {
            Some(u) if s("ONE-TIME-CODE") == "654321" => {
                u.password = s("PASSWORD");
                (outcome("OK", json!({})), None)
            }
            _ => failed(),
        },
        "mfa" => {
            if s("SESSION") == "mfa-session" && s("ONE-TIME-CODE") == "000000" {
                signed_in(st, "mfa", "mfa@example.com")
            } else {
                failed()
            }
        }
        "refresh" => match cookie_user.filter(|u| st.refresh_ok.contains(*u)).map(str::to_owned) {
            Some(u) => {
                let email = st.users.get(&u).map(|x| x.email.clone()).unwrap_or_default();
                signed_in(st, &u, &email)
            }
            None => (outcome("AUTH-FAILED", json!({})), Some(String::new())),
        },
        "sign-out" => {
            if let Some(u) = cookie_user {
                st.refresh_ok.remove(u);
            }
            st.live_tokens.retain(|_, owner| Some(owner.as_str()) != cookie_user);
            (outcome("OK", json!({})), Some(String::new()))
        }
        "hosted/finish" => {
            if s("HANDLE") == "hosted-code" {
                signed_in(st, "hosted", "hosted@example.com")
            } else {
                failed()
            }
        }
        _ => (outcome("INVALID", json!({})), None),
    }
}

/// `%XX` decoding of one query value.
fn unpercent(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap_or(b'?'));
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Serve one HTTP request on `/auth/*` and close.
fn serve_auth(mut stream: std::net::TcpStream, state: &Arc<Mutex<State>>) {
    use std::io::{Read, Write};
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let Ok(n) = stream.read(&mut chunk) else { return };
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p + 4;
        }
        if buf.len() > 64 * 1024 {
            return;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split(' ');
    let (method, target) = (first.next().unwrap_or("").to_owned(), first.next().unwrap_or("").to_owned());
    let headers: Vec<(String, String)> = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned())).collect();
    let header = |k: &str| headers.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let length: usize = header("content-length").and_then(|v| v.parse().ok()).unwrap_or(0).min(16 * 1024);
    while buf.len() < head_end + length {
        let Ok(n) = stream.read(&mut chunk) else { return };
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = &buf[head_end..(head_end + length).min(buf.len())];
    let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));
    let route = path.trim_start_matches("/auth/").to_owned();
    let cookie_user = header("cookie").and_then(|c| c.split(';').map(str::trim).find_map(|p| p.strip_prefix("pr_refresh=").map(str::to_owned))).filter(|u| !u.is_empty());
    let origin = header("origin");
    let mut extra: Vec<String> = Vec::new();
    if let Some(o) = &origin {
        extra.push(format!("Access-Control-Allow-Origin: {o}"));
        extra.push("Access-Control-Allow-Credentials: true".into());
        extra.push("Vary: Origin".into());
    }
    let (status, ctype, out): (&str, &str, String) = match method.as_str() {
        "OPTIONS" => {
            extra.push("Access-Control-Allow-Methods: GET, POST, OPTIONS".into());
            extra.push("Access-Control-Allow-Headers: content-type, x-pr-identity".into());
            ("204 No Content", "text/plain", String::new())
        }
        "GET" if route == "hosted/start" => {
            let param = |k: &str| query.split('&').find_map(|p| p.strip_prefix(&format!("{k}="))).map(unpercent).unwrap_or_default();
            let (back, page_state) = (param("return"), param("state"));
            say("identity=hosted/start (the provider signs `hosted` in)".into());
            let sep = if back.contains('?') { '&' } else { '?' };
            extra.push(format!("Location: {back}{sep}pr-code=hosted-code&pr-state={page_state}"));
            ("302 Found", "text/plain", String::new())
        }
        "POST" if header("x-pr-identity").as_deref() != Some("1") || !header("content-type").unwrap_or_default().starts_with("application/json") => {
            ("403 Forbidden", "text/plain", "an identity request needs Content-Type: application/json and X-PR-Identity: 1".into())
        }
        "POST" => {
            let req: Value = serde_json::from_slice(body).unwrap_or_else(|_| json!({}));
            let (result, cookie) = identity(&mut state.lock().unwrap(), &route, &req, cookie_user.as_deref());
            match cookie {
                Some(u) if u.is_empty() => extra.push("Set-Cookie: pr_refresh=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0".into()),
                Some(u) => extra.push(format!("Set-Cookie: pr_refresh={u}; Path=/; HttpOnly; SameSite=Strict")),
                None => {}
            }
            ("200 OK", "application/json", result.to_string())
        }
        _ => ("404 Not Found", "text/plain", "not found".into()),
    };
    let mut res = format!("HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n", out.len());
    for h in extra {
        res.push_str(&h);
        res.push_str("\r\n");
    }
    res.push_str("\r\n");
    res.push_str(&out);
    let _ = stream.write_all(res.as_bytes());
}

fn main() {
    let (mut port, mut ttl, mut first_ttl) = (8096u16, 3600u64, None);
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--port" => port = it.next().and_then(|v| v.parse().ok()).expect("a port"),
            "--ttl" => ttl = it.next().and_then(|v| v.parse().ok()).expect("seconds"),
            "--first-ttl" => first_ttl = Some(it.next().and_then(|v| v.parse().ok()).expect("seconds")),
            other => panic!("unknown argument {other}"),
        }
    }
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("the port is free");
    println!("mock backend listening on ws://127.0.0.1:{port}/ws and http://127.0.0.1:{port}/auth/ (token lifetime {ttl} s)");
    serve(listener, ttl, first_ttl);
}

/// Serve `listener` until the process ends: the identity requests on `/auth/`, the WebSocket otherwise.
pub fn serve(listener: TcpListener, ttl: u64, first_ttl: Option<u64>) {
    let state = Arc::new(Mutex::new(State { ttl, first_ttl, ..State::default() }));
    {
        let mut st = state.lock().unwrap();
        st.users.insert("ana".into(), User { password: "s3cret!".into(), confirmed: true, email: "ana@example.com".into() });
        st.users.insert("mfa".into(), User { password: "s3cret!".into(), confirmed: true, email: "mfa@example.com".into() });
    }
    for stream in listener.incoming().flatten() {
        let state = state.clone();
        std::thread::spawn(move || {
            // An identity request is plain HTTP on `/auth/`; anything else is the WebSocket.
            let mut peek = [0u8; 64];
            let n = stream.peek(&mut peek).unwrap_or(0);
            let line = String::from_utf8_lossy(&peek[..n]);
            if line.split(' ').nth(1).is_some_and(|p| p.starts_with("/auth/")) {
                serve_auth(stream, &state);
                return;
            }
            let Ok(mut ws) = accept(stream) else { return };
            let mut who: Option<String> = None;
            say("connection opened".into());
            while let Ok(msg) = ws.read() {
                let Message::Text(text) = msg else {
                    if matches!(msg, Message::Close(_)) {
                        break;
                    }
                    continue;
                };
                let Ok(v) = serde_json::from_str::<Value>(text.as_str()) else { continue };
                match v.get("t").and_then(Value::as_str) {
                    Some("hello") => {
                        let contract = v.get("contract").cloned().unwrap_or(Value::Null);
                        let hello = json!({"t": "hello", "protocol": 1, "protocol_min": 1, "contract": contract, "accepted": "ok", "message": "", "server": "mock", "max_frame_bytes": 1 << 20});
                        let _ = ws.send(Message::text(hello.to_string()));
                    }
                    Some("auth") => {
                        let t = v.get("token").and_then(Value::as_str).unwrap_or("");
                        let owner = state.lock().unwrap().live_tokens.get(t).cloned();
                        let good = owner.is_some();
                        if good {
                            who = owner;
                        }
                        say(format!("auth message: token {}", if good { "accepted" } else { "refused" }));
                        let answer = if good {
                            json!({"t": "auth-ok", "sub": who.clone().unwrap_or_default(), "expires_in": state.lock().unwrap().ttl})
                        } else {
                            json!({"t": "error", "class": "authentication", "code": 2001, "message": "token refused"})
                        };
                        let _ = ws.send(Message::text(answer.to_string()));
                    }
                    Some("call") => {
                        let id = v.get("id").and_then(Value::as_u64).unwrap_or(0);
                        say(format!("call: a program request{}", if who.is_some() { " (signed in)" } else { "" }));
                        let _ = ws.send(Message::text(ok(id, json!("pong"))));
                    }
                    _ => {}
                }
            }
            say("connection closed".into());
        });
    }
}
