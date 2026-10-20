// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The whole stack under `node` (WASM-015, WASM-017 R20): the **real loader**, the **real player**
//! built by the product's own build step, and the fixture program module, in every combination of
//! program variant (multi-memory, single-memory) and execution mode (main thread, Worker). The
//! scenarios themselves are in `support/harness.rs`; this test assembles the bundle, runs them and
//! asserts on the JSON report.

mod support;

use std::process::Command;

use cobolt_web_bundle::{BundleOptions, Isolation, PlayerInput};
use support::harness::{HARNESS, WORKER_SHIM};

#[test]
fn the_loader_the_player_and_the_program_run_together_under_node() {
    if !support::node_available() {
        panic!("node is required for the end-to-end tests (WASM-015) and was not found on PATH");
    }
    let player_out = support::built_player();
    let plain = support::player_variant(&player_out, "plain");
    let out = support::assemble(
        PlayerInput { plain: Some(plain), ..Default::default() },
        BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    );

    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let tools = tempfile::tempdir().unwrap();
    let harness = tools.path().join("harness.mjs");
    let shim = tools.path().join("worker-shim.mjs");
    let crafted = tools.path().join("surprise.wasm");
    std::fs::write(&harness, HARNESS).unwrap();
    std::fs::write(&shim, WORKER_SHIM).unwrap();
    std::fs::write(&crafted, support::surprise_module()).unwrap();

    let run = Command::new("node").arg(&harness).arg(dir.path()).arg(&shim).arg(&crafted).output().expect("node runs");
    let stdout = String::from_utf8_lossy(&run.stdout);
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(run.status.success(), "node exited with {}:\n{stderr}\n{stdout}", run.status);
    let line = stdout.lines().last().expect("the harness printed its report");
    let report: serde_json::Value = serde_json::from_str(line).unwrap_or_else(|e| panic!("report is not JSON ({e}): {line}\n{stderr}"));

    let mut failed = Vec::new();
    let mut passed = 0;
    for (name, r) in report["results"].as_object().unwrap() {
        if r["pass"].as_bool() == Some(true) {
            passed += 1;
        } else {
            failed.push(format!("{name}: {}", r["error"].as_str().unwrap_or("?")));
        }
    }
    println!("end-to-end scenarios: {passed} passed, {} failed", failed.len());
    for (name, m) in report["metrics"].as_object().unwrap() {
        println!("  {name}: {m}");
    }
    assert!(failed.is_empty(), "failed scenarios:\n{}", failed.join("\n"));
    // Four (variant x mode) cores plus the probing, choosing, isolation, baseline, integrity, ABI and loading scenarios.
    assert!(passed >= 11, "only {passed} scenarios ran");
}
