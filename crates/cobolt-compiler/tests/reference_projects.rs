// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T51 (the M6 close; R11, AC1 local half): every reference project builds on the
//! WebAssembly path with no Rust toolchain on the machine, so the `DesktopControl` fallback is
//! reached by none of them.
//!
//! The reference projects are the Desktop examples with neither `EXEC RUST` nor an External Crate
//! (R17's note, C7). The test finds them itself, so a new example that qualifies is held to the
//! same rule, and it fails when the list it finds is not the five the spec names without anyone
//! having said why.
//!
//! One test, because it takes `cargo` and `rustc` away from the whole process (`PATH` becomes an
//! empty folder, `CARGO_HOME` and `RUSTUP_HOME` point at empty folders): the runner is built first,
//! while `cargo` is still there, and each Build is handed it. A project that still needed the Cargo
//! path would stop at the toolchain check with its reason, and the test would name it.

use cobolt_compiler::{build_project, BuildOptions, BuildPath, CompilerError};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// The five reference projects the spec names (R17's note).
const SPEC_NAMES: [&str; 5] = ["PowerCallOfDuty", "PowerCloud", "PowerCobolCraft", "PowerGraphics", "PowerSpatial"];

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// A project's one `*.project.toml`, if it has one.
fn manifest_of(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(".project.toml")))
}

/// Whether any source or form of `dir` holds an `EXEC RUST` block.
fn has_exec_rust(dir: &Path) -> bool {
    let Ok(rd) = fs::read_dir(dir) else { return false };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if has_exec_rust(&p) {
                return true;
            }
        } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| matches!(x.to_ascii_lowercase().as_str(), "cfrm" | "cbl" | "cob" | "cpy")) {
            if let Ok(text) = fs::read_to_string(&p) {
                if text.contains("EXEC RUST") {
                    return true;
                }
            }
        }
    }
    false
}

/// The Desktop examples without `EXEC RUST` and without an External Crate, by name.
fn reference_projects(examples: &Path) -> Vec<String> {
    let mut names = Vec::new();
    for e in fs::read_dir(examples).unwrap().flatten() {
        let dir = e.path();
        let Some(manifest) = manifest_of(&dir) else { continue };
        let text = fs::read_to_string(&manifest).unwrap_or_default();
        let desktop = !text.lines().any(|l| {
            let l = l.trim();
            l.starts_with("kind") && (l.contains("\"web\"") || l.contains("\"mobile\""))
        });
        let crates = fs::read_dir(dir.join("crates")).map(|rd| rd.flatten().count()).unwrap_or(0);
        if desktop && crates == 0 && !has_exec_rust(&dir) {
            names.push(e.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    names
}

/// `from`'s files under `to`, without a previous Build's `bin/`, `target/` or `dist/`.
fn copy_project(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let name = e.file_name();
        if matches!(name.to_str(), Some("bin" | "target" | "dist")) {
            continue;
        }
        let (src, dst) = (e.path(), to.join(&name));
        if e.file_type().unwrap().is_dir() {
            copy_project(&src, &dst);
        } else {
            fs::copy(&src, &dst).unwrap();
        }
    }
}

#[test]
fn every_reference_project_builds_on_the_webassembly_path_with_no_toolchain() {
    let root = std::env::temp_dir().join(format!("prc-t51-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let examples = workspace.join("examples");

    // ── The runner, built while cargo is still on PATH ──
    let t = Instant::now();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(&cargo)
        .current_dir(&workspace)
        .args(["build", "-q", "-p", "cobolt-runner", "--bin", "prc-runner"])
        .status()
        .expect("run cargo");
    assert!(status.success(), "cargo build -p cobolt-runner --bin prc-runner failed");
    let exe = std::env::current_exe().unwrap();
    let runner = exe.parent().and_then(Path::parent).unwrap().join(cobolt_compiler::RUNNER_NAME);
    assert!(runner.is_file(), "the runner is at {}", runner.display());
    let runner_ms = ms(t);

    // ── No Rust toolchain from here on ──
    let (empty_path, cargo_home, rustup_home) = (root.join("empty-path"), root.join("cargo-home"), root.join("rustup-home"));
    for d in [&empty_path, &cargo_home, &rustup_home] {
        fs::create_dir_all(d).unwrap();
    }
    std::env::set_var("PATH", &empty_path);
    std::env::set_var("CARGO_HOME", &cargo_home);
    std::env::set_var("RUSTUP_HOME", &rustup_home);
    std::env::remove_var(cobolt_compiler::RUNNER_ENV);
    let no_rustc = Command::new("rustc").arg("-V").output().is_err() && Command::new("cargo").arg("-V").output().is_err();

    let found = reference_projects(&examples);
    let mut rows: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    let mut checks = 0usize;
    let mut check = |what: String, ok: bool, failures: &mut Vec<String>| {
        checks += 1;
        if !ok {
            failures.push(what);
        }
    };
    check("cargo and rustc cannot be started".into(), no_rustc, &mut failures);
    check(
        format!("the reference projects found under examples/ are the five the spec names (found {found:?})"),
        found == SPEC_NAMES,
        &mut failures,
    );

    let (mut total_ms, mut total_bytes) = (0.0, 0u64);
    for name in &found {
        let from = examples.join(name);
        let copy = root.join("projects").join(name);
        copy_project(&from, &copy);
        let manifest = manifest_of(&copy).expect("the copy has its manifest");
        let opts = BuildOptions { verbose: false, build_dir: Some(root.join("builds").join(name)), runner: Some(runner.clone()), ..Default::default() };
        let t = Instant::now();
        let built = build_project(&manifest, &opts);
        let took = ms(t);
        match built {
            Ok(b) => {
                let bytes = fs::metadata(&b.binary_path).map(|m| m.len()).unwrap_or(0);
                check(format!("{name}: the WebAssembly path, no Cargo build"), b.path == BuildPath::Wasm && b.cargo.is_none(), &mut failures);
                check(format!("{name}: bin/<name> is written"), bytes > 0, &mut failures);
                total_ms += took;
                total_bytes += bytes;
                rows.push(format!("  {name:<18} WebAssembly   {:>8.1} ms   {:>6.1} MB", took, bytes as f64 / 1e6));
            }
            Err(CompilerError::CargoNeedsToolchain { reason, .. }) => {
                check(format!("{name}: took the Cargo path: {reason}"), false, &mut failures);
                rows.push(format!("  {name:<18} CARGO PATH: {reason}"));
            }
            Err(e) => {
                check(format!("{name}: the Build failed: {e}"), false, &mut failures);
                rows.push(format!("  {name:<18} FAILED: {e}"));
            }
        }
    }

    println!("== reference_projects: every reference project on the WebAssembly path, no toolchain (DESKTOP-WASM-001 T51, AC1 local half) ==");
    println!("  the runner built in {runner_ms:.0} ms while cargo was on PATH; then PATH, CARGO_HOME and RUSTUP_HOME emptied");
    println!("  found under examples/ (Desktop, no EXEC RUST, no External Crate): {}", found.join(", "));
    for r in &rows {
        println!("{r}");
    }
    println!("  builds: {} in {total_ms:.0} ms, {:.1} MB of programs written", found.len(), total_bytes as f64 / 1e6);
    println!("  result: {} of {checks} checks passed", checks - failures.len());
    let _ = fs::remove_dir_all(&root);
    assert!(failures.is_empty(), "{failures:#?}");
}
