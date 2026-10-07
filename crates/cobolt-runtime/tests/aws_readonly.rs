// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Read-only servers start read-only (spec 078 T-A12, R25 as amended A4;
//! AC15): a server that has a read-only mode starts in it unless some control
//! on its connection allows writes, and is restarted with its write flags the
//! moment one does.

#![cfg(feature = "aws")]

use std::time::Duration;

use cobolt_runtime::aws::connections::AwsConnection;
use cobolt_runtime::aws::ops::{self, ControlView, Outcome};
use cobolt_runtime::aws::pool::{self, Budget};
use serde_json::json;

fn args_seen(path: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(path).unwrap_or_default().lines().skip(1).map(str::to_owned).collect()
}

#[test]
fn a_readonly_connection_starts_the_server_readonly_and_restarts_with_write_args() {
    let dir = std::env::temp_dir().join(format!("prc-078-ro-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("script.json");
    std::fs::write(
        &script,
        json!({
            "tools": [{"name": "read_thing", "inputSchema": {"type": "object"}, "annotations": {"readOnlyHint": true}}],
            "argv_dump": dir.join("argv.txt").to_string_lossy(),
            "pid_file": dir.join("pids.txt").to_string_lossy(),
        })
        .to_string(),
    )
    .unwrap();
    let conn = AwsConnection {
        id: "ro-1".into(),
        name: "ro-1".into(),
        routes_override: format!(
            "[servers.fake]\ncommand = '{}'\nargs = ['{}']\nreadonly_args = ['--read-only']\nwrite_args = ['--allow-write']\n",
            env!("CARGO_BIN_EXE_fake_mcp"),
            script.display()
        ),
        ..Default::default()
    };
    let budget = Budget { call: Duration::from_secs(5), start: Duration::from_secs(10) };
    let prop = |p: &str| if p == "ServerId" { "fake".to_owned() } else { String::new() };
    let list = |connection_allows_write: bool| {
        let view = ControlView {
            control_type: "AwsMcp",
            method: "ListTools",
            args: &[],
            control: "MCP-1",
            prop: &prop,
            allow_write: false,
            connection_allows_write,
        };
        let p = ops::prepare(&view, &conn).expect("prepared");
        ops::execute(&p, budget)
    };

    // No control on the connection allows writes: started read-only.
    assert!(matches!(list(false), Outcome::Done { .. }));
    let first = args_seen(&dir.join("argv.txt"));
    // Some control on it now does: the same connection's server restarts
    // with its write flags.
    assert!(matches!(list(true), Outcome::Done { .. }));
    let second = args_seen(&dir.join("argv.txt"));
    let starts = std::fs::read_to_string(dir.join("pids.txt")).unwrap_or_default().lines().count();
    println!("\n  read-only start: {first:?}\n  after AllowWrite: {second:?}\n  server starts: {starts}");
    pool::stop(&pool::ServerKey { connection: "ro-1".into(), server: "fake".into() });
    let _ = std::fs::remove_dir_all(&dir);

    assert!(first.contains(&"--read-only".to_owned()) && !first.contains(&"--allow-write".to_owned()), "{first:?}");
    assert!(second.contains(&"--allow-write".to_owned()) && !second.contains(&"--read-only".to_owned()), "{second:?}");
    assert_eq!(starts, 2, "the server was restarted to take the write flags");
}
