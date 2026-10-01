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
use std::sync::{Arc, Mutex};

use cobolt_project_tools::http::{serve_http, OpenKit};
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
    open: Arc<Mutex<OpenKit>>,
}

fn start() -> Server {
    let dir: PathBuf = std::env::temp_dir().join(format!("prc-080-http-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy_dir(Path::new(FIXTURE), &dir);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    assert!(listener.local_addr().unwrap().ip().is_loopback(), "bound to loopback only");
    let port = listener.local_addr().unwrap().port();
    let open = Arc::new(Mutex::new(OpenKit::Project { kit_id: Some("k-test".into()) }));
    let open2 = Arc::clone(&open);
    let shared = Arc::new(Shared::new());
    std::thread::spawn(move || {
        serve_http(
            listener,
            move || ProjectTools::with_shared(HeadlessHost::new(&dir, "1.80.test"), Arc::clone(&shared)),
            move || open2.lock().unwrap().clone(),
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

fn post(port: u16, path: &str, extra: &str, body: &str) -> (u16, String) {
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
    let (code, body) = post(p, "/mcp/k-test", "", &init);
    assert_eq!(code, 200, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(v["result"]["serverInfo"]["version"], "1.80.test");
    rows.push(("initialize", code));

    let list = json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}).to_string();
    let (code, body) = post(p, "/mcp/k-test", "", &list);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["result"]["tools"].as_array().unwrap().len(), 7);
    rows.push(("tools/list (7)", code));

    let note = json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string();
    let (code, body) = post(p, "/mcp/k-test", "", &note);
    assert_eq!((code, body.as_str()), (202, ""));
    rows.push(("notification", code));
    let (code, _) = post(p, "/mcp/k-test", "", &json!({"jsonrpc":"2.0","id":9,"result":{}}).to_string());
    assert_eq!(code, 202);
    rows.push(("client response", code));

    for method in ["GET", "DELETE"] {
        let (code, _) = raw(p, format!("{method} /mcp/k-test HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\n\r\n").as_bytes());
        assert_eq!(code, 405, "{method}");
        rows.push((if method == "GET" { "GET" } else { "DELETE" }, code));
    }
    let (code, _) = post(p, "/mcp/k-test", "Origin: https://evil.example\r\n", &list);
    assert_eq!(code, 403);
    rows.push(("foreign Origin", code));
    let (code, _) = post(p, "/mcp/k-test", &format!("Origin: http://localhost:{p}\r\n"), &list);
    assert_eq!(code, 200);
    rows.push(("own Origin", code));
    let (code, _) = raw(
        p,
        format!(
            "POST /mcp/k-test HTTP/1.1\r\nHost: evil.example\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{list}",
            list.len()
        )
        .as_bytes(),
    );
    assert_eq!(code, 403);
    rows.push(("Host: evil.example", code));
    let (code, _) = raw(
        p,
        format!(
            "POST /mcp/k-test HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{list}",
            list.len()
        )
        .as_bytes(),
    );
    assert_eq!(code, 415);
    rows.push(("text/plain", code));
    let (code, _) = raw(
        p,
        format!(
            "POST /mcp/k-test HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n"
        )
        .as_bytes(),
    );
    assert_eq!(code, 411);
    rows.push(("chunked", code));
    let (code, _) = raw(
        p,
        format!(
            "POST /mcp/k-test HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            cobolt_mcp::transport::MAX_MESSAGE_BYTES + 1
        )
        .as_bytes(),
    );
    assert_eq!(code, 413);
    rows.push(("oversize", code));
    let (code, _) = post(p, "/other", "", &list);
    assert_eq!(code, 404);
    rows.push(("other path", code));

    // The right kit: a tool runs. The wrong kit: every tool says so.
    let check = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"check","arguments":{}}}).to_string();
    let (code, body) = post(p, "/mcp/k-test", "", &check);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert!(v["result"]["isError"].is_null(), "{body}");
    assert!(v["result"]["content"][0]["text"].as_str().unwrap().contains("WS-NOT-DECLARED"));
    rows.push(("check, right kit", code));
    let (code, body) = post(p, "/mcp/k-other", "", &check);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["result"]["isError"], true);
    assert!(v["result"]["content"][0]["text"].as_str().unwrap().contains("different project"));
    rows.push(("check, other kit", code));
    let (_, body) = post(p, "/mcp/k-other", "", &list);
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["result"]["tools"].as_array().unwrap().len(), 7);
    rows.push(("tools/list, other kit", 200));

    *srv.open.lock().unwrap() = OpenKit::NoProject;
    let (code, body) = post(p, "/mcp/k-test", "", &check);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["result"]["content"][0]["text"], "no project open");
    rows.push(("check, no project", code));

    for (case, code) in &rows {
        println!("  {case:<24} {code}");
    }
    println!("http: {} requests over 127.0.0.1:{p}, every answer as plan §1.4 specifies", rows.len());
}
