// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T28 (R1, AC1): a Backend is built with **no Rust on the machine**. The process's `PATH` is an empty folder (neither
//! `cargo` nor `rustc` can be started: the test proves it first), `CARGO` and `RUSTC` are unset, and the Build of PowerWebHello and
//! PowerOficina must still make the file on the module path, with no reason to name, and the file must start and describe itself.
//! This is its own test binary because it changes the process's environment.

use std::time::Instant;

use cobolt_compiler::backend_build::build_backend;
use cobolt_compiler::backend_module::{BackendEngine, BackendPath};

mod common;
use common::{options, project, run, runner_with_sbom};

#[test]
fn a_backend_builds_with_neither_cargo_nor_rustc_on_the_machine() {
    let empty = std::env::temp_dir().join(format!("prc-backend-no-toolchain-path-{}", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    // The runner and the projects are prepared first; only the Build runs without a toolchain.
    let scratch = std::env::temp_dir().join(format!("prc-backend-no-toolchain-runner-{}", std::process::id()));
    let runner = runner_with_sbom(&scratch);
    let dirs: Vec<_> = ["PowerWebHello", "PowerOficina"].iter().map(|n| (*n, project(n, "notool"))).collect();

    std::env::set_var("PATH", &empty);
    std::env::remove_var("CARGO");
    std::env::remove_var("RUSTC");
    std::env::remove_var("RUSTUP_TOOLCHAIN");
    for tool in ["cargo", "rustc"] {
        assert!(std::process::Command::new(tool).arg("--version").output().is_err(), "{tool} must not be startable for this test to mean anything");
    }

    let mut rows = Vec::new();
    let mut passed = 2u32;
    for (name, dir) in &dirs {
        let began = Instant::now();
        let built = build_backend(dir, &options(&runner, Some(BackendPath::Auto))).unwrap_or_else(|e| panic!("{name} did not build with no Rust on the machine: {e}"));
        let ms = began.elapsed().as_millis();
        assert_eq!(built.engine, BackendEngine::Module, "{name}: {:?}", built.reasons);
        assert!(built.reasons.is_empty(), "{name}: {:?}", built.reasons);
        let describe = run(&built.binary, dir, &["--describe"]);
        assert_eq!(describe.code, Some(0), "{name}: {}", describe.stderr);
        assert_eq!(describe.stdout.trim(), std::fs::read_to_string(&built.interface).unwrap().trim(), "{name}: --describe prints the interface");
        passed += 4;
        rows.push(format!("{name}: {} program(s), {ms} ms, the file starts and describes itself", built.programs));
    }
    println!("Backend Build with an empty PATH and no CARGO/RUSTC: cargo and rustc cannot be started, both Backends build on the module path");
    for r in &rows {
        println!("  {r}");
    }
    println!("PASS {passed:03} FAIL 000");
    for (_, dir) in &dirs {
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
    let _ = std::fs::remove_dir_all(&scratch);
    let _ = std::fs::remove_dir_all(&empty);
}
