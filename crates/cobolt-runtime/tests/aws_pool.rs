// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 078 T-A6: the AWS server pool against the fake server — one process
//! per connection, restart after a crash, timeouts, notifications, the
//! separate start budget (AC3, AC5).

#![cfg(feature = "aws")]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cobolt_mcp::Mode;
use cobolt_runtime::aws::pool::{self, Budget, CallError, ServerKey};
use cobolt_runtime::aws::process::Launch;
use serde_json::{json, Value};

struct Fixture {
    dir: PathBuf,
    launch: Launch,
    key: ServerKey,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        pool::stop(&self.key);
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fixture(tag: &str, extra: Value) -> Fixture {
    let dir = std::env::temp_dir().join(format!("prc-078-pool-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut script = json!({
        "tools": [{"name": "echo", "inputSchema": {"type": "object"}}],
        "pid_file": dir.join("pids.txt").to_string_lossy(),
        "call_log": dir.join("calls.jsonl").to_string_lossy(),
    });
    for (k, v) in extra.as_object().unwrap() {
        script[k] = v.clone();
    }
    std::fs::write(dir.join("script.json"), script.to_string()).unwrap();
    let launch = Launch {
        command: env!("CARGO_BIN_EXE_fake_mcp").into(),
        args: vec![dir.join("script.json").to_string_lossy().into_owned()],
        ..Default::default()
    };
    let key = ServerKey { connection: format!("conn-{tag}"), server: "fake".into() };
    Fixture { dir, launch, key }
}

fn lines(p: &Path) -> usize {
    std::fs::read_to_string(p).map(|t| t.lines().count()).unwrap_or(0)
}

fn budget(call_ms: u64, start_ms: u64) -> Budget {
    Budget { call: Duration::from_millis(call_ms), start: Duration::from_millis(start_ms) }
}

fn echo(f: &Fixture, b: Budget) -> Result<String, CallError> {
    pool::call_tool(&f.key, &f.launch, Mode::Handshake, "echo", json!({"n": 1}), b)
        .map(|r| r.content[0].as_text().unwrap_or_default().to_owned())
}

/// AC5: two calls (two controls) on one connection share ONE process.
#[test]
fn two_calls_one_connection_one_process() {
    let f = fixture("share", json!({}));
    let t = Instant::now();
    assert_eq!(echo(&f, budget(5000, 10000)).unwrap(), r#"{"n":1}"#);
    let first = t.elapsed();
    let t = Instant::now();
    assert_eq!(echo(&f, budget(5000, 10000)).unwrap(), r#"{"n":1}"#);
    let second = t.elapsed();
    assert_eq!(lines(&f.dir.join("pids.txt")), 1, "one start for two calls");
    println!("AC5 shared: 2 calls, 1 process; first call {first:?} (start + handshake), second {second:?}");
}

/// AC5: the server dies mid-way; that call is an error, the next one runs on a
/// fresh process.
#[test]
fn killed_server_errors_then_restarts() {
    let f = fixture("crash", json!({"crash_after": 2}));
    assert!(echo(&f, budget(5000, 10000)).is_ok());
    let e = echo(&f, budget(5000, 10000)).unwrap_err();
    assert!(matches!(e, CallError::Failed(ref m) if m.contains("stopped")), "{e:?}");
    // The fresh process would crash again on ITS second call; its first works.
    assert!(echo(&f, budget(5000, 10000)).is_ok(), "the next call starts a fresh server");
    assert_eq!(lines(&f.dir.join("pids.txt")), 2, "two starts");
    println!("AC5 restart: call ok, crash -> error, next call ok on a second process");
}

/// AC3 (R5): a server that never answers yields a timeout within budget + 1 s.
#[test]
fn a_silent_server_times_out_within_budget() {
    let f = fixture("silent", json!({"silent": ["echo"]}));
    // Start it first, so the measured time is the call's alone.
    pool::list_tools(&f.key, &f.launch, Mode::Handshake, budget(5000, 10000)).unwrap();
    let t = Instant::now();
    let e = echo(&f, budget(500, 10000)).unwrap_err();
    let took = t.elapsed();
    assert_eq!(e, CallError::Timeout);
    assert!(took < Duration::from_millis(1500), "{took:?}");
    println!("AC3 timeout: TimeoutMs 500 -> Timeout after {took:?}");
}

/// AC3 (R4): progress, a log message and a server ping before the answer do
/// not stall the call.
#[test]
fn notifications_and_a_ping_do_not_stall_a_call() {
    let f = fixture("notify", json!({"notify_before": true}));
    let t = Instant::now();
    assert_eq!(echo(&f, budget(5000, 10000)).unwrap(), r#"{"n":1}"#);
    let calls = std::fs::read_to_string(f.dir.join("calls.jsonl")).unwrap();
    println!("AC3 notifications: answered in {:?} after progress + log + ping", t.elapsed());
    assert!(!calls.contains("\"method\":\"ping\""), "the fake received no ping of ours");
}

/// Amendment A3: the first start is bounded by StartTimeoutMs, not TimeoutMs.
#[test]
fn the_first_start_uses_the_start_budget() {
    let slow = fixture("slowstart", json!({"init_delay_ms": 400}));
    let t = Instant::now();
    assert!(echo(&slow, budget(100, 5000)).is_ok(), "a 400 ms start fits a 5 s start budget though calls get 100 ms");
    let ok = t.elapsed();
    pool::stop(&slow.key);
    let e = echo(&slow, budget(5000, 100)).unwrap_err();
    assert!(matches!(e, CallError::Start(ref m) if m.contains("StartTimeoutMs")), "{e:?}");
    println!("A3 start budget: 400 ms start passed with a 100 ms call budget ({ok:?}); failed with a 100 ms start budget");
}

/// A dead pool server never leaves its process behind: stop() ends it.
#[test]
fn stop_ends_the_process() {
    let f = fixture("stop", json!({}));
    echo(&f, budget(5000, 10000)).unwrap();
    let pid: u32 = std::fs::read_to_string(f.dir.join("pids.txt")).unwrap().trim().parse().unwrap();
    pool::stop(&f.key);
    let alive = std::process::Command::new("kill").args(["-0", &pid.to_string()]).status().map(|s| s.success()).unwrap_or(false);
    assert!(!alive, "process {pid} survived stop()");
}
