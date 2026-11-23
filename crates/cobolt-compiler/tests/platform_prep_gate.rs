// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **2.1.19 — a Build that needs Cargo asks before it compiles the platform.**
//!
//! The first Cargo-path Build of a PowerRustCOBOL version compiles the platform's crates on the
//! developer's machine: minutes, or an hour on a modest one, with every core busy. A host that can
//! ask first (the IDE) sets [`BuildOptions::ask_before_platform_compile`], and the Build then stops
//! with [`CompilerError::PlatformPrep`] before it stages or compiles anything. The host that does not
//! (`rcrun`) is not asked anything. This test runs with the toolchain the machine has and compiles
//! nothing: it only reaches the point where the Build would start.

use cobolt_compiler::warmup::{PlatformPrep, Why};
use cobolt_compiler::{build_project, BuildOptions, CargoReason, CompilerError};
use std::path::{Path, PathBuf};
use std::time::Instant;

const MAIN: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PREPGATE.
       PROCEDURE DIVISION.
           DISPLAY \"hello\"
           STOP RUN.
";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().to_path_buf()
}

fn project(root: &Path) -> PathBuf {
    let dir = root.join("prepgate");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/main.cbl"), MAIN).unwrap();
    let manifest = dir.join("prepgate.project.toml");
    std::fs::write(
        &manifest,
        "[project]\nname = \"prepgate\"\nversion = \"0.1.0\"\nmain = \"src/main.cbl\"\n\n[files]\nsources = [\"src/main.cbl\"]\n",
    )
    .unwrap();
    manifest
}

#[test]
fn a_cargo_build_asks_before_it_compiles_the_platform() {
    let started = Instant::now();
    let root = std::env::temp_dir().join(format!("prc-prep-gate-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let manifest = project(&root);
    let builds = root.join("builds");
    // The host (the IDE) names its version; the cache under `builds` is empty, so nothing is compiled yet.
    cobolt_compiler::warmup::set_current_version("0.0.0-prep-gate");
    let ask = |debug: bool, force_cargo: bool, ask_first: bool| {
        build_project(
            &manifest,
            &BuildOptions {
                verbose: false,
                workspace_root: Some(workspace_root()),
                build_dir: Some(builds.clone()),
                // No runner is looked for: the Cargo path is asked for, by a Debug Build or by the project.
                debug,
                force_cargo,
                ask_before_platform_compile: ask_first,
                ..Default::default()
            },
        )
    };
    let mut report: Vec<(String, bool)> = Vec::new();
    let mut check = |what: &str, ok: bool| report.push((what.to_owned(), ok));

    let release = ask(false, true, true);
    check(
        "a release Build the project sent to Cargo stops with PlatformPrep, naming Cargo's reason and what is missing",
        matches!(&release, Err(CompilerError::PlatformPrep { prep, reason })
            if *reason == CargoReason::Chosen && matches!(prep, PlatformPrep::Everything(Why::NoLibraries))),
    );
    let debug = ask(true, false, true);
    check(
        "a Debug Build stops with PlatformPrep: the debug platform is never shipped",
        matches!(&debug, Err(CompilerError::PlatformPrep { prep, reason })
            if *reason == CargoReason::DebugBuild && *prep == PlatformPrep::Everything(Why::DebugProfile)),
    );
    check(
        "it stopped before anything was staged: no build folder, no bin/",
        !builds.join("cobolt-build-prepgate").exists() && !manifest.parent().unwrap().join("bin").exists(),
    );
    let text = release.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
    check("the message says the platform is not compiled here yet", text.contains("not compiled on this machine yet"));

    println!("platform_prep_gate — a Build that needs Cargo asks before compiling the platform");
    println!("  cases: release + force_cargo, Debug, nothing staged, message");
    for (what, ok) in &report {
        println!("  {} {what}", if *ok { "PASS" } else { "FAIL" });
    }
    let failed = report.iter().filter(|(_, ok)| !ok).count();
    println!("  {} passed, {failed} failed in {} ms (2 gated builds, no compile)", report.len() - failed, started.elapsed().as_millis());
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(failed, 0, "{report:?}");
}
