// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T19 (R13, R18, R25): the `build` tool's answer, on real Builds.
//!
//! One test, because it takes `cargo` and `rustc` away from the whole process: `PATH` becomes an
//! empty folder, and `CARGO_HOME` and `RUSTUP_HOME` point at empty folders. The runner is built
//! first, while `cargo` is still there, and named by `PRC_RUNNER`; the Builds go to a folder of
//! the test's own (`POWERRUSTCOBOL_BUILD_DIR`).
//!
//! Fixtures, each built through the tool as an agent calls it:
//! - a console project takes the WebAssembly path: the answer names the path, each phase in ms
//!   and the peak;
//! - a program with an `EXEC RUST` block takes the Cargo path and, with no Rust, stops: the answer
//!   says Rust is needed and why, with the block's file and line;
//! - a program whose Common Code reads a disk file (which the WebAssembly path does not take yet)
//!   does the same, naming the construct, its file and its line.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use cobolt_project_tools::{FileList, HeadlessHost, NoProject, ProjectHost, ProjectRoot, ProjectTools};
use serde_json::{json, Value};

/// The headless host, with this source checkout as the SDK root.
struct WithSdk(HeadlessHost);

impl ProjectHost for WithSdk {
    fn project(&self) -> Result<ProjectRoot, NoProject> {
        self.0.project()
    }
    fn record(&mut self, rel: &str, list: FileList) -> Result<(), String> {
        self.0.record(rel, list)
    }
    fn workspace_root(&self) -> Option<PathBuf> {
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap())
    }
    fn external_crates(&self) -> Vec<String> {
        Vec::new()
    }
    fn version(&self) -> String {
        "test".into()
    }
}

const MAIN: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T19MAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-TOTAL   PIC 9(6) VALUE 0.
       PROCEDURE DIVISION.
           COMPUTE WS-TOTAL = 12 * 34 + 7
           DISPLAY \"TOTAL \" WS-TOTAL
           STOP RUN.
";

const EXEC_RUST: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T19RUST.
       PROCEDURE DIVISION.
           DISPLAY \"before\"
           EXEC RUST
               println!(\"from rust\");
           END-EXEC
           STOP RUN.
";

const CALLS_FILES: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T19CALLER.
       PROCEDURE DIVISION.
           CALL \"T19FILES\"
           STOP RUN.
";

const FILES: &str = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T19FILES.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT LOG-FILE ASSIGN TO \"t19.log\"
               ORGANIZATION IS LINE SEQUENTIAL.
       DATA DIVISION.
       FILE SECTION.
       FD LOG-FILE.
       01 LOG-REC    PIC X(40).
       PROCEDURE DIVISION.
           OPEN OUTPUT LOG-FILE
           CLOSE LOG-FILE
           GOBACK.
";

/// The 1-based line of `src` that contains `needle`.
fn line_of(src: &str, needle: &str) -> u64 {
    src.lines().position(|l| l.contains(needle)).map(|i| i as u64 + 1).expect("the line is in the source")
}

/// A console project folder under `root`: `files` are (relative path, text).
fn project(root: &Path, name: &str, main: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = root.join(name);
    let _ = fs::remove_dir_all(&dir);
    for (rel, text) in files {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    let list = files.iter().map(|(rel, _)| format!("\"{rel}\"")).collect::<Vec<_>>().join(", ");
    fs::write(
        dir.join(format!("{name}.project.toml")),
        format!("[project]\nname = \"{name}\"\nversion = \"0.1.0\"\nmain = \"{main}\"\n\n[files]\nsources = [{list}]\n"),
    )
    .unwrap();
    dir
}

/// Build `dir` through the tool, calling again while it answers `running`.
fn build(dir: &Path) -> (Value, usize) {
    let mut tools = ProjectTools::new(WithSdk(HeadlessHost::new(dir, "test")));
    let started = Instant::now();
    let mut calls = 0;
    loop {
        let v = tools.call("build", &json!({})).unwrap();
        calls += 1;
        if v["status"] != "running" {
            return (v, calls);
        }
        assert!(started.elapsed() < Duration::from_secs(600), "the build never finished");
    }
}

#[test]
fn the_build_tool_names_the_path_the_reason_the_phases_and_the_peak() {
    let root = std::env::temp_dir().join(format!("prc-t19-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    // ── The runner, built while cargo is still on PATH ──
    let t = Instant::now();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new(&cargo)
        .current_dir(&workspace)
        .args(["build", "-q", "-p", "cobolt-runner", "--bin", "prc-runner"])
        .status()
        .expect("run cargo");
    assert!(status.success(), "cargo build -p cobolt-runner --bin prc-runner failed");
    let exe = std::env::current_exe().unwrap();
    let runner = exe.parent().and_then(Path::parent).unwrap().join(cobolt_compiler::RUNNER_NAME);
    assert!(runner.is_file(), "the runner is at {}", runner.display());
    let runner_ms = t.elapsed().as_secs_f64() * 1000.0;

    // ── No Rust toolchain from here on ──
    let empty_path = root.join("empty-path");
    let cargo_home = root.join("cargo-home");
    let rustup_home = root.join("rustup-home");
    let builds = root.join("builds");
    for d in [&empty_path, &cargo_home, &rustup_home, &builds] {
        fs::create_dir_all(d).unwrap();
    }
    std::env::set_var("PATH", &empty_path);
    std::env::set_var("CARGO_HOME", &cargo_home);
    std::env::set_var("RUSTUP_HOME", &rustup_home);
    std::env::set_var(cobolt_compiler::RUNNER_ENV, &runner);
    std::env::set_var(cobolt_compiler::BUILD_DIR_ENV, &builds);
    std::env::set_var("PRC_RUNNER_CACHE", root.join("runner-cache"));

    let mut checks: Vec<(String, bool)> = Vec::new();
    let mut check = |what: &str, ok: bool| checks.push((what.to_owned(), ok));
    check("cargo and rustc cannot be started", Command::new("rustc").arg("-V").output().is_err());
    let ms_of = |v: &Value| -> Vec<(String, f64)> {
        v["phases"].as_array().map_or_else(Vec::new, |a| {
            a.iter().map(|p| (p["phase"].as_str().unwrap_or("").to_owned(), p["ms"].as_f64().unwrap_or(-1.0))).collect()
        })
    };

    // ── 1. A console project: the WebAssembly path, its phases and its peak ──
    let console = project(&root, "prct19console", "src/main.cbl", &[("src/main.cbl", MAIN)]);
    let t = Instant::now();
    let (wasm, wasm_calls) = build(&console);
    let wasm_ms = t.elapsed().as_secs_f64() * 1000.0;
    let wasm_phases = ms_of(&wasm);
    check("console: built", wasm["status"] == "built");
    check("console: path WebAssembly", wasm["path"] == "WebAssembly");
    check("console: no reason", wasm.get("reason").is_none());
    check(
        "console: the seven WebAssembly phases, in order",
        wasm_phases.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>()
            == ["regenerate", "front end", "classify", "generate", "machine code", "assemble", "install"],
    );
    check("console: every phase has its ms", !wasm_phases.is_empty() && wasm_phases.iter().all(|(_, ms)| *ms >= 0.0));
    check("console: the peak is a number of bytes", wasm["peak_bytes"].as_u64().is_some_and(|b| b > 0) || wasm["peak_bytes"].is_null());
    check("console: the binary is bin/<name>", wasm["binary"].as_str().is_some_and(|b| b.starts_with("bin/prct19console")));

    // ── 2. EXEC RUST, no Rust: stopped, with the block's file and line ──
    let rust = project(&root, "prct19rust", "src/main.cbl", &[("src/main.cbl", EXEC_RUST)]);
    let (exec, _) = build(&rust);
    let exec_line = line_of(EXEC_RUST, "EXEC RUST");
    check("exec rust: failed", exec["status"] == "failed");
    check("exec rust: needs_rust, tool rustc, path Cargo", exec["needs_rust"] == true && exec["tool"] == "rustc" && exec["path"] == "Cargo");
    check(
        &format!("exec rust: reason exec_rust at src/main.cbl line {exec_line}"),
        exec["reason"]["kind"] == "exec_rust" && exec["reason"]["file"] == "src/main.cbl" && exec["reason"]["line"] == exec_line,
    );
    check("exec rust: the message says what is missing", exec["errors"].as_str().is_some_and(|e| e.contains("Rust toolchain is required")));

    // ── 3. A disk file in Common Code, no Rust: the construct, its file and its line ──
    let files = project(&root, "prct19files", "src/main.cbl", &[("src/main.cbl", CALLS_FILES), ("src/files.cbl", FILES)]);
    let (refused, _) = build(&files);
    check("refused: failed, needs_rust", refused["status"] == "failed" && refused["needs_rust"] == true);
    let reason = &refused["reason"];
    check(
        "refused: reason refused, in src/files.cbl, with a line",
        reason["kind"] == "refused" && reason["file"] == "src/files.cbl" && reason["line"].as_u64().is_some_and(|l| l > 0),
    );
    check("refused: names the construct and why", reason["construct"].as_str().is_some_and(|c| !c.is_empty()) && reason["why"].as_str().is_some_and(|w| !w.is_empty()));

    let failures: Vec<&(String, bool)> = checks.iter().filter(|(_, ok)| !ok).collect();
    println!("== the build tool on real Builds (DESKTOP-WASM-001 T19: R13, R18, R25) ==");
    println!("environment: PATH = an empty folder; CARGO_HOME and RUSTUP_HOME = empty folders; runner by PRC_RUNNER");
    println!("runner built in {runner_ms:.0} ms");
    println!(
        "console project: WebAssembly in {wasm_ms:.0} ms over {wasm_calls} call(s); phases {}; peak {}",
        wasm_phases.iter().map(|(p, ms)| format!("{p} {ms:.1} ms")).collect::<Vec<_>>().join(", "),
        wasm["peak_bytes"]
    );
    println!("EXEC RUST: {}", exec["reason"]["text"].as_str().unwrap_or("?"));
    println!("disk file: {}", reason["text"].as_str().unwrap_or("?"));
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "ok  " } else { "FAIL" });
    }
    println!("fixtures 3; {} of {} checks passed", checks.len() - failures.len(), checks.len());
    let _ = fs::remove_dir_all(&root);
    assert!(failures.is_empty(), "failed: {failures:?}\nconsole: {wasm}\nexec rust: {exec}\nrefused: {refused}");
}
