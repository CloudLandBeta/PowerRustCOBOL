// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A scriptable MCP server for the AWS tests** (spec 078 T-A4).
//!
//! Test infrastructure, never shipped: it stands in for AWS's MCP servers so
//! that no test needs an AWS account, a network or Python. It speaks MCP over
//! stdin/stdout exactly as a real stdio server does, and does what a JSON
//! script (the first argument) tells it to:
//!
//! ```json
//! {
//!   "version": "2025-06-18",           // what initialize answers with
//!   "tools": [ {"name": "...", "inputSchema": {...}, "annotations": {...}} ],
//!   "page_size": 1,                    // tools/list paging (0 = one page)
//!   "answers": { "<tool>": {"content": [...], "isError": false} },
//!   "delay_ms": { "<tool>": 500 },     // wait before answering
//!   "init_delay_ms": 300,              // a slow start (a first uvx download)
//!   "silent": [ "<tool>" ],            // never answer this tool
//!   "notify_before": true,             // progress + log + ping before each answer
//!   "crash_after": 2,                  // exit abruptly on the Nth tools/call
//!   "stderr_on_call": "text",          // written to stderr on each call
//!   "fail_with": "text",               // every call answers isError with this
//!   "call_log": "/path/log.jsonl",     // one line per request received
//!   "pid_file": "/path/pids.txt",      // one line per start: the PID
//!   "env_dump": "/path/env.txt",       // the environment it was given
//!   "argv_dump": "/path/argv.txt"      // the arguments it was given
//! }
//! ```
//!
//! A tool with no canned answer echoes its arguments back as text, which is
//! what most tests check.

use std::io::{self, BufReader, Write};

use cobolt_mcp::transport::{read_message, write_message};
use serde_json::{json, Value};

fn append(path: Option<&str>, line: &str) {
    if let Some(p) = path {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let _ = writeln!(f, "{line}");
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let script: Value = args
        .get(1)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}));
    let s = |k: &str| script.get(k).and_then(Value::as_str).map(str::to_owned);
    let call_log = s("call_log");

    append(s("pid_file").as_deref(), &std::process::id().to_string());
    if let Some(p) = s("env_dump") {
        let env: Vec<String> = std::env::vars().map(|(k, v)| format!("{k}={v}")).collect();
        let _ = std::fs::write(p, env.join("\n"));
    }
    if let Some(p) = s("argv_dump") {
        let _ = std::fs::write(p, args.join("\n"));
    }

    let tools: Vec<Value> = script.get("tools").and_then(Value::as_array).cloned().unwrap_or_default();
    let page_size = script.get("page_size").and_then(Value::as_u64).unwrap_or(0) as usize;
    let version = s("version").unwrap_or_else(|| "2025-06-18".into());
    let notify = script.get("notify_before").and_then(Value::as_bool).unwrap_or(false);
    let crash_after = script.get("crash_after").and_then(Value::as_u64);
    let in_list = |key: &str, name: &str| {
        script.get(key).and_then(Value::as_array).is_some_and(|a| a.iter().any(|v| v == name))
    };

    let stdin = io::stdin();
    let mut input = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut calls = 0u64;
    let send = |out: &mut io::StdoutLock, v: Value| {
        let _ = write_message(out, serde_json::to_string(&v).unwrap().as_bytes());
    };

    while let Ok(Some(raw)) = read_message(&mut input) {
        let Ok(msg) = serde_json::from_slice::<Value>(&raw) else { continue };
        let Some(method) = msg.get("method").and_then(Value::as_str).map(str::to_owned) else {
            continue; // a response (to our ping) — nothing to do
        };
        append(call_log.as_deref(), &json!({"method": method, "params": msg.get("params")}).to_string());
        let Some(id) = msg.get("id").cloned() else { continue }; // a notification
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        match method.as_str() {
            "initialize" => {
                if let Some(ms) = script.get("init_delay_ms").and_then(Value::as_u64) {
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                }
                send(
                &mut out,
                json!({"jsonrpc":"2.0","id":id,"result":{
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "fake_mcp", "version": "1"}}}),
                )
            }
            "ping" => send(&mut out, json!({"jsonrpc":"2.0","id":id,"result":{}})),
            "tools/list" => {
                let start: usize = params
                    .get("cursor")
                    .and_then(Value::as_str)
                    .and_then(|c| c.parse().ok())
                    .unwrap_or(0);
                let size = if page_size == 0 { tools.len().max(1) } else { page_size };
                let page: Vec<Value> = tools.iter().skip(start).take(size).cloned().collect();
                let mut result = json!({"tools": page});
                if start + size < tools.len() {
                    result["nextCursor"] = json!((start + size).to_string());
                }
                send(&mut out, json!({"jsonrpc":"2.0","id":id,"result":result}));
            }
            "tools/call" => {
                calls += 1;
                if crash_after.is_some_and(|n| calls >= n) {
                    std::process::exit(3);
                }
                let name = params.get("name").and_then(Value::as_str).unwrap_or("").to_owned();
                if let Some(text) = s("stderr_on_call") {
                    eprintln!("{text}");
                }
                if in_list("silent", &name) {
                    continue;
                }
                if let Some(ms) = script.get("delay_ms").and_then(|d| d.get(&name)).and_then(Value::as_u64) {
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                }
                if notify {
                    send(&mut out, json!({"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":1,"progress":50}}));
                    send(&mut out, json!({"jsonrpc":"2.0","method":"notifications/message","params":{"level":"info","data":"working"}}));
                    send(&mut out, json!({"jsonrpc":"2.0","id":"fake-ping","method":"ping"}));
                }
                let result = if let Some(text) = s("fail_with") {
                    json!({"content":[{"type":"text","text":text}],"isError":true})
                } else if let Some(a) = script.get("answers").and_then(|a| a.get(&name)) {
                    a.clone()
                } else {
                    let args = params.get("arguments").cloned().unwrap_or(json!({}));
                    json!({"content":[{"type":"text","text":args.to_string()}]})
                };
                send(&mut out, json!({"jsonrpc":"2.0","id":id,"result":result}));
            }
            other => send(
                &mut out,
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":format!("no such method: {other}")}}),
            ),
        }
    }
}
