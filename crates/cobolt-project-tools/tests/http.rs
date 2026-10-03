// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The IDE's transport over a real loopback socket (spec 080 AC5, transport
//! half): every row of plan §1.4's table.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cobolt_project_tools::http::{serve_http, HttpGate};
use cobolt_project_tools::tools::Shared;
use cobolt_project_tools::{HeadlessHost, ProjectTools};
use serde_json::{json, Value};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/check_project");

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap().flatten() {
        let to = dst.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), to).unwrap();
        }
    }
}

struct Server {
    port: u16,
    open: Arc<AtomicBool>,
}

fn start() -> Server {
    let dir: PathBuf = std::env::temp_dir().join(format!("prc-080-http-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy_dir(Path::new(FIXTURE), &dir);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    assert!(listener.local_addr().unwrap().ip().is_loopback(), "bound to loopback only");
    let port = listener.local_addr().unwrap().port();
    let open = Arc::new(AtomicBool::new(true));
    let open2 = Arc::clone(&open);
    let gate = HttpGate {
        token: Arc::new(|| Some(TOKEN.to_owned())),
        project_open: Arc::new(move || open2.load(Ordering::SeqCst)),
        stop: Arc::new(AtomicBool::new(false)),
    };
    let shared = Arc::new(Shared::new());
    std::thread::spawn(move || {
        serve_http(
            listener,
            move || ProjectTools::with_shared(HeadlessHost::new(&dir, "1.80.test"), Arc::clone(&shared)),
            gate,
        )
    });
    Server { port, open }
}

/// Send raw bytes; the status code and the body.
fn raw(port: u16, request: &[u8]) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(request).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let status = out
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let body = out.split_once("\r\n\r\n").map(|(_, b)| b.to_owned()).unwrap_or_default();
    (status, body)
}

/// The access token the test server admits.
const TOKEN: &str = "tok-test-084";

/// POST with the server's token.
fn post(port: u16, path: &str, extra: &str, body: &str) -> (u16, String) {
    post_as(port, path, &format!("Authorization: Bearer {TOKEN}\r\n{extra}"), body)
}

/// POST with exactly the headers given (no token unless `extra` has one).
fn post_as(port: u16, path: &str, extra: &str, body: &str) -> (u16, String) {
    raw(
        port,
        format!(
            "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    )
}

#[test]
fn http_transport_answers_every_row_of_the_table() {
    let srv = start();
    let p = srv.port;
    let mut rows: Vec<(&str, u16)> = Vec::new();

    let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}).to_string();
    let (code, body) = post(p, "/mcp", "", &init);
    assert_eq!(code, 200, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(v["result"]["serverInfo"]["version"], "1.80.test");
    rows.push(("initialize", code));

    let list = json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}).to_string();
    let (code, body) = post(p, "/mcp", "", &list);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["result"]["tools"].as_array().unwrap().len(), 10);
    rows.push(("tools/list (7)", code));

    let note = json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string();
    let (code, body) = post(p, "/mcp", "", &note);
    assert_eq!((code, body.as_str()), (202, ""));
    rows.push(("notification", code));
    let (code, _) = post(p, "/mcp", "", &json!({"jsonrpc":"2.0","id":9,"result":{}}).to_string());
    assert_eq!(code, 202);
    rows.push(("client response", code));

    for method in ["GET", "DELETE"] {
        let (code, _) = raw(p, format!("{method} /mcp HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\n\r\n").as_bytes());
        assert_eq!(code, 405, "{method}");
        rows.push((if method == "GET" { "GET" } else { "DELETE" }, code));
    }
    let (code, _) = post(p, "/mcp", "Origin: https://evil.example\r\n", &list);
    assert_eq!(code, 403);
    rows.push(("foreign Origin", code));
    let (code, _) = post(p, "/mcp", &format!("Origin: http://localhost:{p}\r\n"), &list);
    assert_eq!(code, 200);
    rows.push(("own Origin", code));
    let (code, _) = raw(
        p,
        format!(
            "POST /mcp HTTP/1.1\r\nHost: evil.example\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{list}",
            list.len()
        )
        .as_bytes(),
    );
    assert_eq!(code, 403);
    rows.push(("Host: evil.example", code));
    let (code, _) = raw(
        p,
        format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{list}",
            list.len()
        )
        .as_bytes(),
    );
    assert_eq!(code, 415);
    rows.push(("text/plain", code));
    let (code, _) = raw(
        p,
        format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n"
        )
        .as_bytes(),
    );
    assert_eq!(code, 411);
    rows.push(("chunked", code));
    let (code, _) = raw(
        p,
        format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            cobolt_mcp::transport::MAX_MESSAGE_BYTES + 1
        )
        .as_bytes(),
    );
    assert_eq!(code, 413);
    rows.push(("oversize", code));
    let (code, _) = post(p, "/other", "", &list);
    assert_eq!(code, 404);
    rows.push(("other path", code));

    // The token: missing or wrong is 401; the right one runs the tool.
    let check = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"check","arguments":{}}}).to_string();
    let (code, body) = post(p, "/mcp", "", &check);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert!(v["result"]["isError"].is_null(), "{body}");
    assert!(v["result"]["content"][0]["text"].as_str().unwrap().contains("WS-NOT-DECLARED"));
    rows.push(("check, right token", code));
    let (code, body) = post_as(p, "/mcp", "", &check);
    assert_eq!((code, body.as_str()), (401, ""), "no token");
    rows.push(("no token", code));
    let (code, _) = post_as(p, "/mcp", "Authorization: Bearer tok-wrong\r\n", &check);
    assert_eq!(code, 401, "wrong token");
    rows.push(("wrong token", code));
    let (code, _) = post_as(p, "/mcp", "Authorization: Bearer \r\n", &list);
    assert_eq!(code, 401, "even tools/list needs the token");
    rows.push(("empty token, tools/list", code));
    let (code, _) = post(p, "/mcp/k-old", "", &list);
    assert_eq!(code, 404, "the per-kit path is gone");
    rows.push(("old /mcp/<kit-id>", code));

    srv.open.store(false, Ordering::SeqCst);
    let (code, body) = post(p, "/mcp", "", &check);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["result"]["content"][0]["text"], "no project open");
    rows.push(("check, no project", code));
    let kb = json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"kb_lookup","arguments":{"name":"Button"}}}).to_string();
    let (_, body) = post(p, "/mcp", "", &kb);
    assert!(serde_json::from_str::<Value>(&body).unwrap()["result"]["isError"].is_null(), "knowledge needs no project");
    rows.push(("kb_lookup, no project", 200));

    for (case, code) in &rows {
        println!("  {case:<24} {code}");
    }
    println!("http: {} requests over 127.0.0.1:{p}, every answer as plan §1.4 specifies", rows.len());
}

/// Spec 084 AC5 + AC7 over HTTP — and with NO project open, because knowledge
/// needs none: the instructions and the reference resources still answer while
/// every tool call is refused.
#[test]
fn http_serves_instructions_and_resources_even_without_a_project() {
    let srv = start();
    srv.open.store(false, Ordering::SeqCst);
    let ask = |id: u32, method: &str, params: Value| -> Value {
        let body = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}).to_string();
        let (code, text) = post(srv.port, "/mcp", "", &body);
        assert_eq!(code, 200, "{method}: {text}");
        serde_json::from_str(&text).unwrap()
    };
    let init = ask(1, "initialize", json!({"protocolVersion":"2025-06-18"}));
    let instructions = init["result"]["instructions"].as_str().expect("instructions");
    let rules = cobolt_project_tools::content::rules();
    assert!(rules.iter().all(|r| instructions.contains(&r.text)), "every rule is in the instructions");
    let list = ask(2, "resources/list", json!({}));
    let n = list["result"]["resources"].as_array().map(Vec::len).unwrap_or(0);
    assert!(n >= 10, "the reference pack is listed: {n}");
    let read = ask(3, "resources/read", json!({"uri":"powerrustcobol://reference/cobol85-supported-syntax.md"}));
    assert_eq!(
        read["result"]["contents"][0]["text"].as_str(),
        Some(cobolt_project_tools::reference::SUPPORTED_SYNTAX)
    );
    let call = ask(4, "tools/call", json!({"name":"check","arguments":{}}));
    assert_eq!(call["result"]["isError"], true, "a tool call is still refused without a project");
    println!("http, no project open: {} rules in the instructions, {n} resources listed, syntax doc read, tool call refused", rules.len());
}
