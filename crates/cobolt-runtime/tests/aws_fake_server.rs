// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 078 T-A4: the fake MCP server the AWS tests stand on, driven by the
//! real `cobolt-mcp` client over real pipes (AC1, both revisions).

#![cfg(feature = "aws")]

use std::io::BufReader;
use std::process::{Command, Stdio};
use std::time::Instant;

use cobolt_mcp::{Client, ServerInfo, Session};
use serde_json::{json, Value};

fn script(dir: &std::path::Path, body: Value) -> std::path::PathBuf {
    let p = dir.join("script.json");
    std::fs::write(&p, body.to_string()).unwrap();
    p
}

fn temp(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("prc-078-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Run one session against the fake: list the tools, call one. Returns the
/// tool names, the call's text and the time taken.
fn session(version: &str, stateless: bool) -> (Vec<String>, String, Vec<Value>, u128) {
    let dir = temp(&format!("smoke-{version}"));
    let log = dir.join("calls.jsonl");
    let path = script(
        &dir,
        json!({
            "version": version,
            "page_size": 1,
            "tools": [
                {"name": "get_item", "inputSchema": {"type": "object"}, "annotations": {"readOnlyHint": true}},
                {"name": "put_item", "inputSchema": {"type": "object"}}
            ],
            "call_log": log.to_string_lossy(),
        }),
    );
    let t = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fake_mcp"))
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("the fake starts");
    let reader = BufReader::new(child.stdout.take().unwrap());
    let writer = child.stdin.take().unwrap();
    let me = ServerInfo { name: "aws-test".into(), version: "1".into() };
    let session = if stateless { Session::stateless(me) } else { Session::handshake(me) };
    let mut c = Client::new(session, reader, writer);
    c.initialize().unwrap();
    let tools: Vec<String> = c.list_tools_all().unwrap().into_iter().map(|t| t.name).collect();
    let r = c.call_tool("get_item", json!({"Key": {"id": "42"}})).unwrap();
    let text = r.content[0].as_text().unwrap().to_owned();
    drop(c); // closes stdin: the server ends
    let _ = child.wait();
    let took = t.elapsed().as_millis();
    let calls: Vec<Value> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    (tools, text, calls, took)
}

/// AC1: tools listed across two pages and a tool called, once over the
/// handshake (2025-06-18) and once stateless (2026-07-28); the results agree.
#[test]
fn the_fake_server_serves_both_revisions_alike() {
    let (tools_h, text_h, calls_h, ms_h) = session("2025-06-18", false);
    let (tools_s, text_s, calls_s, ms_s) = session("2025-06-18", true);
    assert_eq!(tools_h, ["get_item", "put_item"], "two pages, both tools");
    assert_eq!((&tools_h, &text_h), (&tools_s, &text_s), "both revisions give the same answer");
    assert_eq!(serde_json::from_str::<Value>(&text_h).unwrap(), json!({"Key": {"id": "42"}}));
    let methods = |c: &[Value]| c.iter().map(|v| v["method"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
    assert_eq!(methods(&calls_h), ["initialize", "notifications/initialized", "tools/list", "tools/list", "tools/call"]);
    assert_eq!(methods(&calls_s), ["tools/list", "tools/list", "tools/call"], "stateless: no handshake");
    assert_eq!(calls_s[2]["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"], "2026-07-28");
    println!(
        "AC1 fake server: handshake session {} requests in {ms_h} ms, stateless {} requests in {ms_s} ms; 2 tools over 2 pages, the same echo both ways",
        calls_h.len(),
        calls_s.len()
    );
}
