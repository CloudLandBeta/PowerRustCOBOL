// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A stand-in application for the no-orphans test** (spec 078 T-A7, AC6).
//! Test infrastructure, never shipped.
//!
//! `aws_host_probe <fake_mcp> <script> <how>` starts the fake through the
//! server pool, prints `ready` once it has answered, and then leaves:
//! - `exit` — returns from `main` after the orderly shutdown every host runs;
//! - `exit-now` — `process::exit` with no shutdown (only stdin EOF and the
//!   operating-system backstop remain);
//! - `hang` — waits to be killed.

use std::time::Duration;

use cobolt_mcp::Mode;
use cobolt_runtime::aws::pool::{self, Budget, ServerKey};
use cobolt_runtime::aws::process::Launch;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let launch = Launch { command: args[1].clone(), args: vec![args[2].clone()], ..Default::default() };
    let key = ServerKey { connection: "probe".into(), server: "fake".into() };
    let budget = Budget { call: Duration::from_secs(10), start: Duration::from_secs(10) };
    pool::call_tool(&key, &launch, Mode::Handshake, "echo", serde_json::json!({}), budget).expect("the fake answers");
    println!("ready");
    match args.get(3).map(String::as_str) {
        Some("exit-now") => std::process::exit(0),
        Some("hang") => loop {
            std::thread::sleep(Duration::from_secs(60));
        },
        _ => cobolt_runtime::shutdown_child_processes(),
    }
}
