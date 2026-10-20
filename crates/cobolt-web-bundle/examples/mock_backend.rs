// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `cargo run -p cobolt-web-bundle --example mock_backend -- [--port 8096] [--ttl 3600]`
//!
//! A mock of the project's Backend for trying the player in a real browser (WASM-009, WASM-015): a
//! WebSocket server on the loopback that speaks the connection the player proposes
//! (`{"type":"auth","token"}`, `{"type":"call","id","frame"}`) and plays an identity provider:
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
//! fixture program's `PING`) is answered `pong`.

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
}

struct User {
    password: String,
    confirmed: bool,
    email: String,
}

fn token_for(st: &mut State, user: &str, email: &str) -> String {
    let payload = json!({"sub": format!("u-{user}"), "preferred_username": user, "email": email, "exp": now_s() + st.ttl});
    let t = format!("{}.{}.{}", b64url(br#"{"alg":"none","typ":"JWT"}"#), b64url(payload.to_string().as_bytes()), "bW9jaw");
    st.live_tokens.insert(t.clone(), user.to_owned());
    st.refresh_ok.insert(user.to_owned());
    t
}

fn ok(id: u64, result: Value) -> String {
    json!({"id": id, "result": result}).to_string()
}

fn err(id: u64, class: &str, code: i64) -> String {
    json!({"id": id, "error": {"class": class, "code": code, "message": "refused"}}).to_string()
}

/// Handle one identity frame. `signed_in_user` is who this connection authenticated as, if anyone.
fn identity(st: &mut State, id: u64, f: &Value, signed_in_user: &Option<String>) -> String {
    let s = |k: &str| f.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
    let kind = s("identity");
    let user = s("user");
    let captcha = f.get("captcha").and_then(Value::as_str).map(str::to_owned);
    println!("identity={kind} user={user} captcha={}", if captcha.is_some() { "present" } else { "absent" });
    // reCAPTCHA: a token is single-use; a replay is refused like a wrong password (R4, R10).
    if let Some(c) = &captcha {
        if !st.captchas_seen.insert(c.clone()) {
            return err(id, "authentication", 3);
        }
    }
    let ttl = st.ttl as f64;
    let signed_in = |st: &mut State, user: &str, email: &str| {
        let t = token_for(st, user, email);
        ok(id, json!({"status": "signed-in", "access_token": t, "expires_in": ttl}))
    };
    match kind.as_str() {
        "sign-in" => match st.users.get(&user) {
            Some(u) if u.password == s("password") && !u.confirmed => ok(id, json!({"status": "needs-confirm"})),
            Some(u) if u.password == s("password") && user == "mfa" => {
                let _ = u;
                ok(id, json!({"status": "needs-mfa", "session": "mfa-session"}))
            }
            Some(u) if u.password == s("password") => {
                let email = u.email.clone();
                signed_in(st, &user, &email)
            }
            _ => err(id, "authentication", 3),
        },
        "sign-up" => {
            st.users.insert(user, User { password: s("password"), confirmed: false, email: s("email") });
            ok(id, json!({"status": "needs-confirm"}))
        }
        "confirm" => match st.users.get_mut(&user) {
            Some(u) if s("code") == "123456" => {
                u.confirmed = true;
                ok(id, json!({"status": "done"}))
            }
            _ => err(id, "client", 4),
        },
        // The same answer whether or not the user exists (R10).
        "forgot" => ok(id, json!({"status": "reset-requested"})),
        "reset" => match st.users.get_mut(&user) {
            Some(u) if s("code") == "654321" => {
                u.password = s("new_password");
                ok(id, json!({"status": "done"}))
            }
            _ => err(id, "client", 4),
        },
        "mfa" => {
            if s("session") == "mfa-session" && s("code") == "000000" {
                signed_in(st, "mfa", "mfa@example.com")
            } else {
                err(id, "authentication", 3)
            }
        }
        "refresh" => match signed_in_user {
            Some(u) if st.refresh_ok.contains(u) => {
                let email = st.users.get(u).map(|x| x.email.clone()).unwrap_or_default();
                signed_in(st, u, &email)
            }
            // A connection with no session still has the browser's cookie: any signed-in user's refresh works.
            _ => match st.refresh_ok.iter().next().cloned() {
                Some(u) => {
                    let email = st.users.get(&u).map(|x| x.email.clone()).unwrap_or_default();
                    signed_in(st, &u, &email)
                }
                None => err(id, "authentication", 3),
            },
        },
        "sign-out" => {
            st.refresh_ok.clear();
            st.live_tokens.clear();
            ok(id, json!({"status": "done"}))
        }
        "hosted-start" => ok(id, json!({"status": "hosted", "redirect_url": "http://127.0.0.1:8099/index.html?pr-code=hosted-code&pr-state=hosted-state"})),
        "hosted-finish" => {
            if s("code") == "hosted-code" && s("state") == "hosted-state" {
                signed_in(st, "hosted", "hosted@example.com")
            } else {
                err(id, "authentication", 3)
            }
        }
        _ => err(id, "client", 2),
    }
}

fn main() {
    let (mut port, mut ttl) = (8096u16, 3600u64);
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--port" => port = it.next().and_then(|v| v.parse().ok()).expect("a port"),
            "--ttl" => ttl = it.next().and_then(|v| v.parse().ok()).expect("seconds"),
            other => panic!("unknown argument {other}"),
        }
    }
    let state = Arc::new(Mutex::new(State { ttl, ..State::default() }));
    {
        let mut st = state.lock().unwrap();
        st.users.insert("ana".into(), User { password: "s3cret!".into(), confirmed: true, email: "ana@example.com".into() });
        st.users.insert("mfa".into(), User { password: "s3cret!".into(), confirmed: true, email: "mfa@example.com".into() });
    }
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("the port is free");
    println!("mock backend listening on ws://127.0.0.1:{port}/ws (token lifetime {ttl} s)");
    for stream in listener.incoming().flatten() {
        let state = state.clone();
        std::thread::spawn(move || {
            let Ok(mut ws) = accept(stream) else { return };
            let mut who: Option<String> = None;
            println!("connection opened");
            while let Ok(msg) = ws.read() {
                let Message::Text(text) = msg else {
                    if matches!(msg, Message::Close(_)) {
                        break;
                    }
                    continue;
                };
                let Ok(v) = serde_json::from_str::<Value>(text.as_str()) else { continue };
                match v.get("type").and_then(Value::as_str) {
                    Some("auth") => {
                        let t = v.get("token").and_then(Value::as_str).unwrap_or("");
                        let owner = state.lock().unwrap().live_tokens.get(t).cloned();
                        let good = owner.is_some();
                        if good {
                            who = owner;
                        }
                        println!("auth message: token {}", if good { "accepted" } else { "refused" });
                        let _ = ws.send(Message::text(json!({"type": "auth", "ok": good}).to_string()));
                    }
                    Some("call") => {
                        let id = v.get("id").and_then(Value::as_u64).unwrap_or(0);
                        let frame = v.get("frame").cloned().unwrap_or(Value::Null);
                        let reply = if frame.get("identity").is_some() {
                            let mut st = state.lock().unwrap();
                            identity(&mut st, id, &frame, &who)
                        } else {
                            println!("call: a program request");
                            ok(id, json!("pong"))
                        };
                        let _ = ws.send(Message::text(reply));
                    }
                    Some("pong") => {}
                    _ => {}
                }
            }
            println!("connection closed");
        });
    }
}
