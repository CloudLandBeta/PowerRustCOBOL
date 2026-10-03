// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A streamed chat whose model spends its whole output budget on hidden
//! reasoning is retried with the budget doubled, up to 32768, before it fails
//! (1.80.143). Observed live: the COBOL proficiency check failed on
//! `qwen/qwen3.8-27b:free` after 8192 tokens of reasoning and told the
//! developer to hand-edit `model_policies.json`.
//!
//! A scripted OpenAI-compatible server stands in for the provider: below the
//! budget it needs, it streams only `reasoning_content` and reports the whole
//! budget spent; at or above it, it answers in text.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cobolt_agents::rig_transport::{run_chat_blocking, ChatCall};

/// Serve chats until the test ends. `needs` is the smallest budget at which
/// the model answers; every requested budget is recorded.
fn reasoning_server(needs: u64) -> (String, Arc<Mutex<Vec<u64>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
            let mut buf = Vec::new();
            let mut chunk = [0u8; 8192];
            let head_end = loop {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break None,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                }
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break Some(i + 4);
                }
            };
            let Some(head_end) = head_end else { continue };
            let head = String::from_utf8_lossy(&buf[..head_end]).to_ascii_lowercase();
            let len = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            while buf.len() < head_end + len {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                }
            }
            let body: serde_json::Value =
                serde_json::from_slice(&buf[head_end..]).unwrap_or_default();
            let budget = body["max_tokens"]
                .as_u64()
                .or_else(|| body["max_completion_tokens"].as_u64())
                .unwrap_or(0);
            log.lock().unwrap().push(budget);
            let delta = |d: serde_json::Value| {
                serde_json::json!({"id": "c1", "model": "m", "choices": [{"delta": d, "finish_reason": null}]})
            };
            let mut lines = vec![delta(serde_json::json!({"reasoning_content": "Let me think about COBOL. "}))];
            let spent = if budget >= needs {
                lines.push(delta(serde_json::json!({"content": "IDENTIFICATION DIVISION."})));
                40
            } else {
                budget
            };
            lines.push(serde_json::json!({
                "choices": [],
                "usage": {"prompt_tokens": 10, "completion_tokens": spent, "total_tokens": 10 + spent}
            }));
            let mut sse = String::new();
            for l in lines {
                sse.push_str(&format!("data: {l}\n\n"));
            }
            sse.push_str("data: [DONE]\n\n");
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{sse}"
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    (format!("http://127.0.0.1:{port}/v1"), seen)
}

fn call(endpoint: &str, max_tokens: u32) -> ChatCall {
    ChatCall {
        provider: "custom".into(),
        model: "reasoning-test".into(),
        api_key: "test".into(),
        endpoint: endpoint.into(),
        system_prompt: "You are a COBOL evaluator.".into(),
        skills: String::new(),
        history: Vec::new(),
        user_prompt: "Write a COBOL program.".into(),
        temperature: 0.0,
        max_tokens,
        reasoning_counts_as_reply: false,
        reasoning_effort: None,
    }
}

#[test]
fn a_budget_exhausted_chat_is_retried_with_a_larger_budget() {
    let mut report = Vec::new();

    // 1. The model needs 16384: one retry, and the answer arrives.
    let t = Instant::now();
    let (url, seen) = reasoning_server(16_384);
    let reply = run_chat_blocking(&call(&url, 8192), &|_| {}).expect("answered after the retry");
    let budgets = seen.lock().unwrap().clone();
    assert_eq!(reply.text, "IDENTIFICATION DIVISION.");
    assert_eq!(budgets, vec![8192, 16_384], "one retry, budget doubled");
    report.push(format!(
        "needs 16384  : budgets {budgets:?} -> answered ({:.0} ms)",
        t.elapsed().as_secs_f64() * 1000.0
    ));

    // 2. The model never answers: the ladder stops at 32768 and says so,
    //    without sending the developer to a JSON file.
    let t = Instant::now();
    let (url, seen) = reasoning_server(u64::MAX);
    let err = run_chat_blocking(&call(&url, 8192), &|_| {}).expect_err("never answers");
    let budgets = seen.lock().unwrap().clone();
    assert_eq!(budgets, vec![8192, 16_384, 32_768], "bounded ladder");
    assert!(err.contains("32768-token output budget"), "{err}");
    assert!(err.contains("raised from 8192 tokens"), "{err}");
    assert!(!err.contains("model_policies.json"), "no hand-edit advice: {err}");
    report.push(format!(
        "never answers: budgets {budgets:?} -> refused ({:.0} ms)",
        t.elapsed().as_secs_f64() * 1000.0
    ));

    // 3. A call already at the ceiling is not retried.
    let (url, seen) = reasoning_server(u64::MAX);
    let err = run_chat_blocking(&call(&url, 32_768), &|_| {}).expect_err("never answers");
    let budgets = seen.lock().unwrap().clone();
    assert_eq!(budgets, vec![32_768]);
    assert!(!err.contains("raised from"), "{err}");
    report.push(format!("at the ceiling: budgets {budgets:?} -> refused, no retry"));

    println!("\n=== budget-exhausted chat retry ===");
    for line in &report {
        println!("  {line}");
    }
    println!("  3/3 cases pass");
}
