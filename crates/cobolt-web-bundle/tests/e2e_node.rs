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

use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildRequest};
use cobolt_web_bundle::{BundleOptions, Isolation, PlayerInput};
use support::harness::{HARNESS, WORKER_SHIM};

#[test]
fn the_loader_the_player_and_the_program_run_together_under_node() {
    if !support::require_node("the_loader_the_player_and_the_program_run_together_under_node") {
        return;
    }
    let player_out = support::built_player();
    let plain = support::player_variant(&player_out, "plain");
    let out = support::assemble(
        PlayerInput { plain: Some(plain), parts: support::player_parts(&player_out), ..Default::default() },
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

    // The scenario project (WASM-017 T11), built by the product's own Web Build: its bundle is served to the same script.
    let project = support::scenario_project("e2e");
    let req = WebBuildRequest::from_manifest(&project.path().join("Core.project.toml")).unwrap();
    let built = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("the scenario project does not build: {e}"));
    let mut modules = 0;
    for entry in std::fs::read_dir(&built.bundle_dir).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("program") && name.ends_with(".wasm") {
            // AC6: no import beyond ABI v1, no forbidden instruction, in either memory variant.
            let bytes = std::fs::read(entry.path()).unwrap();
            let problems = cobolt_wasm_abi::scan::bundle_problems(&bytes, name.contains(".single."));
            assert!(problems.is_empty(), "{name}: {problems:#?}");
            println!("scenario module {name}: {} bytes, bundle_problems empty", bytes.len());
            modules += 1;
        }
    }
    assert_eq!(modules, 2, "one module per memory variant");

    let run = Command::new("node").arg(&harness).arg(dir.path()).arg(&shim).arg(&crafted).arg(&built.bundle_dir).output().expect("node runs");
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
    // The limits on speed the harness let pass because timing is advisory on this machine: each one a warning.
    for m in report["timing_missed"].as_array().into_iter().flatten() {
        support::timing_missed(m.as_str().unwrap_or("?").to_owned()).unwrap_or_else(|e| panic!("{e}"));
    }
    println!("end-to-end scenarios: {passed} passed, {} failed", failed.len());
    for (name, m) in report["metrics"].as_object().unwrap() {
        println!("  {name}: {m}");
    }
    assert!(failed.is_empty(), "failed scenarios:\n{}", failed.join("\n"));
    // The test bundle's player carries the hook: the forced panic really ran (WASM-017 T9).
    let panic = report["metrics"].as_object().unwrap().iter().find(|(n, _)| n.starts_with("panic:")).map(|(_, m)| m.clone()).expect("the panic scenario reported");
    assert_eq!(panic["events"], 1, "the forced panic ran on the test bundle: {panic}");
    // Four (variant x mode) cores plus the probing, choosing, isolation, baseline, integrity, ABI and loading scenarios,
    // the parts, the hosted sign-in return and the forced panic (WASM-017 T9).
    assert!(passed >= 28, "only {passed} scenarios ran");
}
