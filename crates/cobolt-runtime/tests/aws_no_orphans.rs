// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 078 AC6 (R9): no AWS server outlives the application that started
//! it — after a normal exit, an abrupt `process::exit`, or a kill.

#![cfg(all(feature = "aws", unix))]

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::json;

fn alive(pid: u32) -> bool {
    Command::new("kill").args(["-0", &pid.to_string()]).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

/// Start the probe, wait for `ready`, apply `how`; return the fake's PID and
/// how long it took to disappear.
fn run(how: &str) -> (u32, Option<Duration>) {
    let dir = std::env::temp_dir().join(format!("prc-078-orphan-{how}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pids = dir.join("pids.txt");
    let script = dir.join("script.json");
    std::fs::write(&script, json!({"pid_file": pids.to_string_lossy(), "tools": [{"name":"echo","inputSchema":{}}]}).to_string()).unwrap();
    let mut probe = Command::new(env!("CARGO_BIN_EXE_aws_host_probe"))
        .args([env!("CARGO_BIN_EXE_fake_mcp"), &script.to_string_lossy(), how])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(probe.stdout.take().unwrap()).read_line(&mut line).unwrap();
    assert_eq!(line.trim(), "ready", "the probe reached its server");
    let fake: u32 = std::fs::read_to_string(&pids).unwrap().trim().parse().unwrap();
    if how == "hang" {
        assert!(alive(fake), "the server runs while its application does");
        probe.kill().unwrap(); // SIGKILL
    }
    let _ = probe.wait();
    let t = Instant::now();
    let gone = loop {
        if !alive(fake) {
            break Some(t.elapsed());
        }
        if t.elapsed() > Duration::from_secs(5) {
            // Do not leave it behind for the next test run either.
            let _ = Command::new("kill").args(["-9", &fake.to_string()]).status();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let _ = std::fs::remove_dir_all(&dir);
    (fake, gone)
}

#[test]
fn no_server_outlives_its_application() {
    let mut report = Vec::new();
    for how in ["exit", "exit-now", "hang"] {
        let (pid, gone) = run(how);
        assert!(gone.is_some(), "{how}: the server (pid {pid}) outlived its application");
        report.push(format!("{how}: gone in {:?}", gone.unwrap()));
    }
    println!("AC6 on {}: {}", std::env::consts::OS, report.join("; "));
}
