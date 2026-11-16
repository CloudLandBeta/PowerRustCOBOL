// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T17: `rcrun build --cargo`, `--no-machine-code`, and the install hint
//! (R15, R24, R25; A4; AC8 first half, AC17 CLI half).
//!
//! `rcrun` is the binary Cargo builds for this test; the runner is built first, into the same
//! folder, so `rcrun` finds it beside itself as a release does. The toolchain is taken away from
//! each `rcrun` run alone (`PATH`, `CARGO_HOME` and `RUSTUP_HOME` of that process), never from
//! this test's own process.
//!
//! Cases:
//! - a console project builds by default on the WebAssembly path, says so, and its file carries
//!   the module and its machine code; the program runs;
//! - `--no-machine-code` gives a file with the module and no machine code, which still runs;
//! - `--cargo` takes the Cargo path with the reason `Chosen`: with no toolchain it stops there,
//!   prints how to install Rust, exits non-zero and leaves `bin/` as it was;
//! - an `EXEC RUST` project with no toolchain names its block's file and line, prints how to
//!   install the release PowerRustCOBOL is built with (Rust 1.95.0 today), exits non-zero and
//!   leaves `bin/` as it was.

use cobolt_program_package::Kind;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

const MAIN: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T17MAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A       PIC 9(4) VALUE 21.
       01 WS-TOTAL   PIC 9(6) VALUE 0.
       PROCEDURE DIVISION.
           COMPUTE WS-TOTAL = WS-A * 2
           DISPLAY \"T17 TOTAL \" WS-TOTAL
           STOP RUN.
";

/// What `MAIN` prints.
const EXPECTED: &str = "T17 TOTAL 000042";

const EXEC_RUST: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T17RUST.
       PROCEDURE DIVISION.
           DISPLAY \"before\"
           EXEC RUST
               println!(\"from rust\");
           END-EXEC
           STOP RUN.
";

fn rcrun() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rcrun"))
}

/// A console project `name` in `root` whose main program is `src`; its manifest's path.
fn project(root: &Path, name: &str, src: &str) -> PathBuf {
    let dir = root.join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/main.cbl"), src).unwrap();
    let manifest = dir.join(format!("{name}.project.toml"));
    fs::write(
        &manifest,
        format!("[project]\nname = \"{name}\"\nversion = \"0.1.0\"\nmain = \"src/main.cbl\"\n\n[files]\nsources = [\"src/main.cbl\"]\n"),
    )
    .unwrap();
    manifest
}

/// The sha256 of every path and content under `dir`, in order ("" for no folder).
fn tree_hash(dir: &Path) -> String {
    fn walk(base: &Path, d: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(d) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push(p.strip_prefix(base).unwrap().to_path_buf());
            }
        }
    }
    let mut files = Vec::new();
    walk(dir, dir, &mut files);
    files.sort();
    let mut h = Sha256::new();
    for rel in files {
        h.update(rel.to_string_lossy().as_bytes());
        h.update([0]);
        h.update(fs::read(dir.join(&rel)).unwrap());
        h.update([0]);
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// The kinds in the payload of the file at `exe`, in table-of-contents order.
fn kinds(exe: &Path) -> Vec<Kind> {
    let bytes = fs::read(exe).unwrap_or_default();
    cobolt_program_package::locate(&bytes)
        .ok()
        .and_then(|p| cobolt_program_package::decode(p).ok())
        .map(|d| d.parts.iter().map(|p| p.kind).collect())
        .unwrap_or_default()
}

struct Env {
    build_dir: PathBuf,
    cache: PathBuf,
    no_toolchain: Option<(PathBuf, PathBuf, PathBuf)>,
}

/// `rcrun build <manifest> --quiet <flags>`, with its own build folder, and with no toolchain when
/// the environment says so.
fn build(env: &Env, manifest: &Path, flags: &[&str]) -> (Output, f64) {
    let mut cmd = Command::new(rcrun());
    cmd.arg("build").arg(manifest).arg("--quiet").args(flags);
    cmd.env(cobolt_compiler::BUILD_DIR_ENV, &env.build_dir).env("PRC_RUNNER_CACHE", &env.cache).env_remove(cobolt_compiler::RUNNER_ENV);
    if let Some((path, cargo_home, rustup_home)) = &env.no_toolchain {
        cmd.env("PATH", path).env("CARGO_HOME", cargo_home).env("RUSTUP_HOME", rustup_home);
    }
    let t = Instant::now();
    let out = cmd.output().expect("run rcrun build");
    (out, t.elapsed().as_secs_f64() * 1000.0)
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[test]
fn rcrun_build_takes_the_path_it_is_told_and_says_how_to_install_rust() {
    let root = std::env::temp_dir().join(format!("prc-t17-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let mut checks: Vec<(String, bool)> = Vec::new();
    let mut check = |what: &str, ok: bool| checks.push((what.to_string(), ok));

    // ── The runner, built beside rcrun ──
    let t = Instant::now();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new(&cargo)
        .current_dir(&workspace)
        .args(["build", "-q", "-p", "cobolt-runner", "--bin", "prc-runner"])
        .status()
        .expect("run cargo");
    assert!(status.success(), "cargo build -p cobolt-runner --bin prc-runner failed");
    let runner = rcrun().parent().unwrap().join(cobolt_compiler::RUNNER_NAME);
    assert!(runner.is_file(), "the runner is beside rcrun at {}", runner.display());
    let runner_ms = t.elapsed().as_secs_f64() * 1000.0;

    let empty_path = root.join("empty-path");
    let cargo_home = root.join("cargo-home");
    let rustup_home = root.join("rustup-home");
    for d in [&empty_path, &cargo_home, &rustup_home] {
        fs::create_dir_all(d).unwrap();
    }
    let with_toolchain = Env { build_dir: root.join("builds"), cache: root.join("runner-cache"), no_toolchain: None };
    let without_toolchain = Env {
        build_dir: root.join("builds"),
        cache: root.join("runner-cache"),
        no_toolchain: Some((empty_path.clone(), cargo_home.clone(), rustup_home.clone())),
    };
    let release = cobolt_compiler::warmup::release_rust();
    let install = cobolt_compiler::rust_install_command();

    // ── 1. The default: the WebAssembly path, with machine code (AC8, first half) ──
    let name = "prct17console";
    let manifest = project(&root, name, MAIN);
    let dir = manifest.parent().unwrap().to_path_buf();
    let exe = dir.join("bin").join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    let (out, default_ms) = build(&with_toolchain, &manifest, &[]);
    let stdout = text(&out.stdout);
    check("default: rcrun build exits 0", out.status.success());
    check("default: it says \"Path   : WebAssembly\"", stdout.lines().any(|l| l.trim() == "Path   : WebAssembly"));
    let default_kinds = kinds(&exe);
    check(
        "default: the file carries MODULE, MACHINE_CODE and its engine's hash",
        default_kinds.contains(&Kind::Module) && default_kinds.contains(&Kind::MachineCode) && default_kinds.contains(&Kind::MachineCodeEngine),
    );
    let default_size = fs::metadata(&exe).map(|m| m.len()).unwrap_or(0);
    let run = Command::new(&exe).env("PRC_RUNNER_CACHE", &with_toolchain.cache).output().expect("run the program");
    check(&format!("default: the program prints {EXPECTED:?}"), run.status.success() && text(&run.stdout).contains(EXPECTED));

    // ── 2. --no-machine-code: the module only ──
    let (out, bare_ms) = build(&with_toolchain, &manifest, &["--no-machine-code"]);
    check("--no-machine-code: rcrun build exits 0, on the WebAssembly path", out.status.success() && text(&out.stdout).contains("Path   : WebAssembly"));
    let bare_kinds = kinds(&exe);
    check(
        "--no-machine-code: the file carries MODULE and no MACHINE_CODE",
        bare_kinds.contains(&Kind::Module) && !bare_kinds.contains(&Kind::MachineCode) && !bare_kinds.contains(&Kind::MachineCodeEngine),
    );
    let bare_size = fs::metadata(&exe).map(|m| m.len()).unwrap_or(0);
    let t = Instant::now();
    let run = Command::new(&exe).env("PRC_RUNNER_CACHE", root.join("fresh-cache")).output().expect("run the program");
    let bare_run_ms = t.elapsed().as_secs_f64() * 1000.0;
    check(&format!("--no-machine-code: the program still prints {EXPECTED:?}"), run.status.success() && text(&run.stdout).contains(EXPECTED));

    // ── 3. --cargo: the Cargo path, reason Chosen; with no toolchain it stops (AC17, CLI half) ──
    let bin_dir = dir.join("bin");
    let before = tree_hash(&bin_dir);
    let (out, cargo_ms) = build(&without_toolchain, &manifest, &["--cargo"]);
    let stderr = text(&out.stderr);
    check("--cargo, no toolchain: rcrun build exits non-zero", !out.status.success());
    check(
        "--cargo: the Cargo path, because the Cargo path is chosen (`rcrun build --cargo`)",
        stderr.contains("built with Cargo because the Cargo path is chosen") && stderr.contains("rcrun build --cargo"),
    );
    check(&format!("--cargo, no toolchain: it prints how to install Rust {release}"), stderr.contains(&format!("To install Rust {release}")) && stderr.contains(install));
    check("--cargo, no toolchain: bin/ is as it was", tree_hash(&bin_dir) == before);

    // ── 4. EXEC RUST with no toolchain: the block's file and line, the hint, bin/ untouched (AC17) ──
    let rust_name = "prct17rust";
    let rust_manifest = project(&root, rust_name, EXEC_RUST);
    let rust_bin = rust_manifest.parent().unwrap().join("bin");
    fs::create_dir_all(&rust_bin).unwrap();
    fs::write(rust_bin.join("keep.txt"), "the developer's file").unwrap();
    let rust_before = tree_hash(&rust_bin);
    let rust_line = EXEC_RUST.lines().position(|l| l.contains("EXEC RUST")).unwrap() + 1;
    let (out, rust_ms) = build(&without_toolchain, &rust_manifest, &[]);
    let stderr = text(&out.stderr);
    check("EXEC RUST, no toolchain: rcrun build exits non-zero", !out.status.success() && out.status.code() == Some(1));
    check(
        &format!("EXEC RUST: it names src/main.cbl line {rust_line}"),
        stderr.contains(&format!("src/main.cbl line {rust_line} holds an EXEC RUST block")),
    );
    check(&format!("EXEC RUST: it prints how to install Rust {release}, the command in full"), stderr.contains(&format!("To install Rust {release}")) && stderr.contains(install));
    check("EXEC RUST: bin/ is as it was (keep.txt only)", tree_hash(&rust_bin) == rust_before);

    // ── The hint is only for a missing toolchain ──
    let (out, _) = build(&with_toolchain, &root.join("no-such").join("x.project.toml"), &[]);
    check("a manifest that is not there: exits non-zero, no install hint", !out.status.success() && !text(&out.stderr).contains("To install Rust"));

    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== build_path_flags (DESKTOP-WASM-001 T17: R15, R24, R25; A4; AC8 first half, AC17 CLI half) ==");
    println!("rcrun: {}; runner beside it, built in {runner_ms:.0} ms", rcrun().display());
    println!("1. default Build: {default_ms:.1} ms, {default_size} bytes, parts {default_kinds:?}");
    println!("2. --no-machine-code: {bare_ms:.1} ms, {bare_size} bytes, parts {bare_kinds:?}; first run (machine code made at load) {bare_run_ms:.1} ms");
    println!("3. --cargo with no toolchain: stopped in {cargo_ms:.1} ms, bin/ unchanged");
    println!("4. EXEC RUST at src/main.cbl line {rust_line} with no toolchain: stopped in {rust_ms:.1} ms, bin/ unchanged");
    println!("install command printed: {install}");
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("result: {passed} of {} checks passed", checks.len());
    let failed: Vec<&String> = checks.iter().filter(|(_, ok)| !*ok).map(|(w, _)| w).collect();
    let _ = fs::remove_dir_all(&root);
    assert!(failed.is_empty(), "failed: {failed:?}");
}
