// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A loopback stand-in for the hosted AWS MCP Server** (spec 103 tests).
//!
//! Test infrastructure, never shipped. It speaks MCP *streamable HTTP* on
//! `127.0.0.1`, so no test needs an AWS account or a network, and it takes the
//! same JSON script as `fake_mcp` (`tools`, `answers`, `answers_when`,
//! `fail_with`, `delay_ms`, `silent`, `notify_before`, `page_size`, `version`,
//! `call_log`, `pid_file`) plus the HTTP-only keys below. It checks nothing
//! about signatures itself — it writes every request down, and the test
//! verifies them with its own SigV4 code (`sigv4.rs`).
//!
//! ```json
//! {
//!   "sse": true,                  // answer as text/event-stream, not JSON
//!   "expire_session_on_call": 2,  // the Nth tools/call meets a session the
//!                                 // server forgot: HTTP 404, and the old id is dead
//!   "always_unknown_session": true, // every tools/call answers 404
//!   "http_error": {"status": 403, "body": "...", "on": "tools/call"}
//! }
//! ```
//!
//! This speaks to one request per connection (`Connection: close`).

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// One request the fake received.
#[derive(Debug, Clone)]
pub struct Logged {
    pub method: String,
    /// The request target, as sent (path and query).
    pub path: String,
    /// Header names in lower case.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Logged {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    /// The body as JSON, when it is.
    pub fn json(&self) -> Option<Value> {
        serde_json::from_slice(&self.body).ok()
    }
    /// The JSON-RPC method of the body, when there is one.
    pub fn rpc_method(&self) -> Option<String> {
        self.json()?.get("method")?.as_str().map(str::to_owned)
    }
}

struct Shared {
    script: Value,
    log: Mutex<Vec<Logged>>,
    stop: AtomicBool,
    session: Mutex<Option<String>>,
    initialized: AtomicU64,
    calls: AtomicU64,
}

/// A running fake; stops when dropped.
pub struct HttpFake {
    /// `http://127.0.0.1:<port>/mcp`
    pub url: String,
    shared: Arc<Shared>,
}

impl HttpFake {
    pub fn start(script: Value) -> HttpFake {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free loopback port");
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let shared = Arc::new(Shared {
            script,
            log: Mutex::new(Vec::new()),
            stop: AtomicBool::new(false),
            session: Mutex::new(None),
            initialized: AtomicU64::new(0),
            calls: AtomicU64::new(0),
        });
        let s = shared.clone();
        std::thread::spawn(move || {
            while !s.stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let s = s.clone();
                        std::thread::spawn(move || {
                            let _ = serve(stream, &s);
                        });
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        HttpFake { url: format!("http://127.0.0.1:{port}/mcp"), shared }
    }

    pub fn requests(&self) -> Vec<Logged> {
        self.shared.log.lock().unwrap().clone()
    }

    /// How many requests used this HTTP method.
    pub fn count_http(&self, method: &str) -> usize {
        self.requests().iter().filter(|r| r.method == method).count()
    }

    /// How many requests carried this JSON-RPC method.
    pub fn count_rpc(&self, method: &str) -> usize {
        self.requests().iter().filter(|r| r.rpc_method().as_deref() == Some(method)).count()
    }

    /// How many `initialize` requests were answered (sessions opened).
    pub fn sessions_opened(&self) -> u64 {
        self.shared.initialized.load(Ordering::SeqCst)
    }

    /// Wait until `ok` holds or the time is up.
    pub fn wait_for(&self, what: impl Fn(&HttpFake) -> bool, within: Duration) -> bool {
        let end = Instant::now() + within;
        while Instant::now() < end {
            if what(self) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        what(self)
    }
}

impl Drop for HttpFake {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
    }
}

fn append(path: Option<&str>, line: &str) {
    if let Some(p) = path {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let _ = writeln!(f, "{line}");
        }
    }
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Option<Logged>> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or("").to_owned(), parts.next().unwrap_or("").to_owned());
    let mut headers = Vec::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let l = line.trim_end_matches(['\r', '\n']);
        if l.is_empty() {
            break;
        }
        if let Some((k, v)) = l.split_once(':') {
            headers.push((k.trim().to_ascii_lowercase(), v.trim().to_owned()));
        }
    }
    let len: usize = headers.iter().find(|(k, _)| k == "content-length").and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    Ok(Some(Logged { method, path, headers, body }))
}

fn respond(stream: &mut TcpStream, status: u16, headers: &[(&str, String)], body: &[u8]) {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        _ => "Status",
    };
    let mut head = format!("HTTP/1.1 {status} {reason}\r\ncontent-length: {}\r\nconnection: close\r\n", body.len());
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

/// One result or one error as the answer to request `id`, as JSON or as events.
fn answer(stream: &mut TcpStream, s: &Shared, before: &[Value], reply: Value, extra: &[(&str, String)]) {
    let sse = s.script.get("sse").and_then(Value::as_bool).unwrap_or(false);
    if sse {
        let mut body = String::new();
        for m in before.iter().chain(std::iter::once(&reply)) {
            body.push_str(&format!("event: message\ndata: {m}\n\n"));
        }
        let mut headers = vec![("content-type", "text/event-stream".to_owned())];
        headers.extend(extra.iter().map(|(k, v)| (*k, v.clone())));
        respond(stream, 200, &headers, body.as_bytes());
    } else {
        let mut headers = vec![("content-type", "application/json".to_owned())];
        headers.extend(extra.iter().map(|(k, v)| (*k, v.clone())));
        respond(stream, 200, &headers, reply.to_string().as_bytes());
    }
}

fn serve(mut stream: TcpStream, s: &Shared) -> std::io::Result<()> {
    let Some(req) = read_request(&mut stream)? else { return Ok(()) };
    s.log.lock().unwrap().push(req.clone());
    let script = &s.script;
    let text = |k: &str| script.get(k).and_then(Value::as_str).map(str::to_owned);

    if req.method == "DELETE" {
        *s.session.lock().unwrap() = None;
        respond(&mut stream, 200, &[], b"");
        return Ok(());
    }
    if req.method != "POST" {
        respond(&mut stream, 405, &[], b"");
        return Ok(());
    }
    let Some(msg) = req.json() else {
        respond(&mut stream, 400, &[("content-type", "text/plain".into())], b"not JSON");
        return Ok(());
    };
    let method = msg.get("method").and_then(Value::as_str).map(str::to_owned);
    let Some(method) = method else {
        // A response to something the fake asked (its ping): nothing owed.
        respond(&mut stream, 202, &[], b"");
        return Ok(());
    };
    append(
        text("call_log").as_deref(),
        &json!({"method": method, "params": msg.get("params")}).to_string(),
    );

    if let Some(err) = script.get("http_error") {
        let on = err.get("on").and_then(Value::as_str).unwrap_or("any");
        if on == "any" || on == method {
            let status = err.get("status").and_then(Value::as_u64).unwrap_or(500) as u16;
            let body = err.get("body").and_then(Value::as_str).unwrap_or("");
            respond(&mut stream, status, &[("content-type", "application/json".into())], body.as_bytes());
            return Ok(());
        }
    }

    // A request other than `initialize` must name the session the server holds.
    if method != "initialize" {
        let held = s.session.lock().unwrap().clone();
        let named = req.header("mcp-session-id").map(str::to_owned);
        if held.is_none() || named != held {
            respond(&mut stream, 404, &[("content-type", "application/json".into())], br#"{"error":"unknown session"}"#);
            return Ok(());
        }
    }
    let Some(id) = msg.get("id").cloned() else {
        respond(&mut stream, 202, &[], b""); // a notification
        return Ok(());
    };
    let params = msg.get("params").cloned().unwrap_or(json!({}));

    match method.as_str() {
        "initialize" => {
            let n = s.initialized.fetch_add(1, Ordering::SeqCst) + 1;
            if n == 1 {
                append(text("pid_file").as_deref(), &std::process::id().to_string());
            }
            let session = format!("session-{n}");
            *s.session.lock().unwrap() = Some(session.clone());
            let version = text("version").unwrap_or_else(|| "2025-06-18".into());
            let reply = json!({"jsonrpc":"2.0","id":id,"result":{
                "protocolVersion": version,
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "http_fake", "version": "1"}}});
            answer(&mut stream, s, &[], reply, &[("mcp-session-id", session)]);
        }
        "ping" => answer(&mut stream, s, &[], json!({"jsonrpc":"2.0","id":id,"result":{}}), &[]),
        "tools/list" => {
            let tools: Vec<Value> = script.get("tools").and_then(Value::as_array).cloned().unwrap_or_default();
            let page_size = script.get("page_size").and_then(Value::as_u64).unwrap_or(0) as usize;
            let start: usize = params.get("cursor").and_then(Value::as_str).and_then(|c| c.parse().ok()).unwrap_or(0);
            let size = if page_size == 0 { tools.len().max(1) } else { page_size };
            let page: Vec<Value> = tools.iter().skip(start).take(size).cloned().collect();
            let mut result = json!({"tools": page});
            if start + size < tools.len() {
                result["nextCursor"] = json!((start + size).to_string());
            }
            answer(&mut stream, s, &[], json!({"jsonrpc":"2.0","id":id,"result":result}), &[]);
        }
        "tools/call" => {
            let n = s.calls.fetch_add(1, Ordering::SeqCst) + 1;
            let always = script.get("always_unknown_session").and_then(Value::as_bool).unwrap_or(false);
            let expire = script.get("expire_session_on_call").and_then(Value::as_u64);
            if always || expire == Some(n) {
                *s.session.lock().unwrap() = None;
                respond(&mut stream, 404, &[("content-type", "application/json".into())], br#"{"error":"unknown session"}"#);
                return Ok(());
            }
            let name = params.get("name").and_then(Value::as_str).unwrap_or("").to_owned();
            let in_list = |key: &str| script.get(key).and_then(Value::as_array).is_some_and(|a| a.iter().any(|v| v == name.as_str()));
            if in_list("silent") {
                // Never answers: hold the connection until the test is over.
                let end = Instant::now() + Duration::from_secs(20);
                while Instant::now() < end && !s.stop.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(20));
                }
                return Ok(());
            }
            if let Some(ms) = script.get("delay_ms").and_then(|d| d.get(&name)).and_then(Value::as_u64) {
                std::thread::sleep(Duration::from_millis(ms));
            }
            let mut before = Vec::new();
            if script.get("notify_before").and_then(Value::as_bool).unwrap_or(false) {
                before.push(json!({"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":1,"progress":50}}));
                before.push(json!({"jsonrpc":"2.0","id":"fake-ping","method":"ping"}));
            }
            let args_text = params.get("arguments").map(Value::to_string).unwrap_or_default();
            let when = script.get("answers_when").and_then(Value::as_array).and_then(|rules| {
                rules.iter().find(|r| {
                    r["tool"].as_str() == Some(name.as_str()) && r["contains"].as_str().is_some_and(|c| args_text.contains(c))
                })
            });
            let result = if let Some(t) = text("fail_with") {
                json!({"content":[{"type":"text","text":t}],"isError":true})
            } else if let Some(rule) = when {
                rule["answer"].clone()
            } else if let Some(a) = script.get("answers").and_then(|a| a.get(&name)) {
                a.clone()
            } else {
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                json!({"content":[{"type":"text","text":args.to_string()}]})
            };
            answer(&mut stream, s, &before, json!({"jsonrpc":"2.0","id":id,"result":result}), &[]);
        }
        other => answer(
            &mut stream,
            s,
            &[],
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":format!("no such method: {other}")}}),
            &[],
        ),
    }
    Ok(())
}
