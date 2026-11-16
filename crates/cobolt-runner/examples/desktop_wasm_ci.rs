// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The checks of `.github/workflows/desktop-wasm.yml` (DESKTOP-WASM-001 T22; AC1, AC3), built once
//! per platform while Rust is there and run where it is not. Each subcommand prints one result block
//! and exits 1 when a check fails.
//!
//! ```text
//! desktop_wasm_ci fixtures  --rcrun <rcrun> --work <dir> [--fixtures <dir>] [--modules-out <dir>]
//! desktop_wasm_ci reference --rcrun <rcrun> --work <dir> <project folder>...
//! desktop_wasm_ci cross     --runner <prc-runner> --modules <dir> [--fixtures <dir>]
//! ```
//!
//! - `fixtures`: each console fixture of the module corpus becomes a project, `rcrun build` builds
//!   it (it must take the WebAssembly path), and the built program runs with the fixture's input
//!   and pins and must print and answer what its `run.toml` says: the fixture's check script.
//!   `--modules-out` keeps each program's module, for `cross`.
//! - `reference`: each reference project, copied under `--work`, is built by `rcrun build`. One
//!   that takes the WebAssembly path must build. One still on the Cargo path is listed with its
//!   reason, which is what AC1 asks until every reference project takes the WebAssembly path.
//! - `cross`: every platform's modules (`<dir>/<platform>/<name>.wasm`) run on this runner and
//!   must do what their fixture says, as they did on their own platform (AC3). The result block
//!   also says whether each fixture's module is the same file on every platform.

#[path = "../tests/support/corpus.rs"]
mod corpus;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("fixtures") => fixtures(&args[1..]),
        Some("reference") => reference(&args[1..]),
        Some("cross") => cross(&args[1..]),
        _ => Err("usage: desktop_wasm_ci fixtures|reference|cross … (see the file's header)".into()),
    };
    match result {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(e) => {
            eprintln!("desktop_wasm_ci: {e}");
            std::process::exit(2);
        }
    }
}

/// `--name value` options, and the rest in order.
fn options(args: &[String]) -> (BTreeMap<String, String>, Vec<String>) {
    let mut named = BTreeMap::new();
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if let Some(name) = a.strip_prefix("--") {
            named.insert(name.to_owned(), it.next().cloned().unwrap_or_default());
        } else {
            rest.push(a.clone());
        }
    }
    (named, rest)
}

fn required(named: &BTreeMap<String, String>, name: &str) -> Result<PathBuf, String> {
    named.get(name).map(PathBuf::from).ok_or_else(|| format!("--{name} is required"))
}

fn exe(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

/// The fixtures, sorted by name, with their `run.toml`.
fn read_fixtures(dir: &Path) -> Result<Vec<(String, PathBuf, corpus::Run)>, String> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let p = e.map_err(|e| e.to_string())?.path();
        if p.join("run.toml").is_file() {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned();
            out.push((name, p.clone(), corpus::read_run(&p.join("run.toml"))?));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// What `rcrun build` said: the path line, the Cargo reason line, everything.
struct Built {
    ok: bool,
    wasm: bool,
    reason: Option<String>,
    text: String,
    ms: f64,
}

fn rcrun_build(rcrun: &Path, manifest: &Path) -> Result<Built, String> {
    let t = Instant::now();
    let out = Command::new(rcrun).arg("build").arg(manifest).output().map_err(|e| format!("{}: {e}", rcrun.display()))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    Ok(Built {
        ok: out.status.success(),
        wasm: text.lines().any(|l| l.trim_start().starts_with("Path") && l.trim_end().ends_with("WebAssembly")),
        reason: text.lines().find_map(|l| l.split_once("Cargo path: ").map(|(_, r)| r.trim().to_owned())),
        text,
        ms: t.elapsed().as_secs_f64() * 1000.0,
    })
}

/// Run a built program as `run` says.
fn run_program(path: &Path, run: &corpus::Run, cache: &Path) -> Result<(String, Option<i32>, f64), String> {
    use std::io::Write;
    use std::process::Stdio;
    let mut cmd = Command::new(path);
    cmd.args(&run.args)
        .env("PRC_TEST_CLOCK", &run.clock)
        .env("PRC_TEST_SEED", run.seed.to_string())
        .env("PRC_RUNNER_CACHE", cache)
        .env("LC_ALL", "en_US.UTF-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in &run.env {
        cmd.env(k, v);
    }
    let t = Instant::now();
    let mut child = cmd.spawn().map_err(|e| format!("{}: {e}", path.display()))?;
    child.stdin.take().expect("stdin").write_all(run.stdin.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    Ok((String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"), out.status.code(), t.elapsed().as_secs_f64() * 1000.0))
}

fn fixtures(args: &[String]) -> Result<bool, String> {
    let (named, _) = options(args);
    let rcrun = required(&named, "rcrun")?;
    let work = required(&named, "work")?;
    let dir = named.get("fixtures").map(PathBuf::from).unwrap_or_else(|| corpus::corpus_dir().join("fixtures"));
    let modules_out = named.get("modules-out").map(PathBuf::from);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    let mut passed = 0;
    let list = read_fixtures(&dir)?;
    for (name, src, run) in &list {
        let project = work.join(name);
        std::fs::create_dir_all(&project).map_err(|e| e.to_string())?;
        for s in &run.sources {
            std::fs::copy(src.join(s), project.join(s)).map_err(|e| format!("{name}/{s}: {e}"))?;
        }
        let bin = format!("corpus{name}");
        let sources = run.sources.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ");
        let manifest = project.join(format!("{bin}.project.toml"));
        std::fs::write(&manifest, format!("[project]\nname = \"{bin}\"\nversion = \"0.1.0\"\nmain = \"{}\"\n\n[files]\nsources = [{sources}]\n", run.sources[0]))
            .map_err(|e| e.to_string())?;
        let built = rcrun_build(&rcrun, &manifest)?;
        if !(built.ok && built.wasm) {
            rows.push(format!("  FAIL {name}: not built on the WebAssembly path ({:.0} ms)\n{}", built.ms, built.text));
            continue;
        }
        let program = project.join("bin").join(exe(&bin));
        let (stdout, code, run_ms) = run_program(&program, run, &work.join("cache"))?;
        let ok = stdout == run.stdout && code == Some(run.exit);
        if ok {
            passed += 1;
        }
        let size = std::fs::metadata(&program).map(|m| m.len()).unwrap_or(0);
        rows.push(format!(
            "  {} {name}: built on the WebAssembly path in {:.0} ms ({size} bytes); ran in {run_ms:.0} ms, exit {code:?}, output {}",
            if ok { "ok  " } else { "FAIL" },
            built.ms,
            if stdout == run.stdout { "identical" } else { "DIFFERS" }
        ));
        if !ok {
            rows.push(format!("--- printed ---\n{stdout}--- recorded ---\n{}", run.stdout));
        }
        if let Some(out) = &modules_out {
            std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
            let bytes = std::fs::read(&program).map_err(|e| e.to_string())?;
            let payload = cobolt_program_package::locate(&bytes).map_err(|e| e.to_string())?;
            let module = cobolt_program_package::decode(payload).map_err(|e| e.to_string())?.module().map_err(|e| e.to_string())?.to_vec();
            std::fs::write(out.join(format!("{name}.wasm")), module).map_err(|e| e.to_string())?;
        }
    }
    println!("== console fixtures built by rcrun and run (DESKTOP-WASM-001 T22: AC1) ==");
    println!("rcrun: {}; rustc on PATH: {}", rcrun.display(), Command::new("rustc").arg("-V").output().is_ok());
    for r in &rows {
        println!("{r}");
    }
    println!("fixtures {}, built and checked {passed} of {}", list.len(), list.len());
    Ok(passed == list.len() && !list.is_empty())
}

/// `from`'s files, copied under `to`, without a previous Build's `bin/`, `target/` or `dist/`.
fn copy_project(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let e = e.map_err(|e| e.to_string())?;
        let name = e.file_name();
        if matches!(name.to_str(), Some("bin" | "target" | "dist")) {
            continue;
        }
        let (src, dst) = (e.path(), to.join(&name));
        if e.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_project(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?;
        }
    }
    Ok(())
}

fn reference(args: &[String]) -> Result<bool, String> {
    let (named, projects) = options(args);
    let rcrun = required(&named, "rcrun")?;
    let work = required(&named, "work")?;
    let mut rows = Vec::new();
    let (mut wasm, mut cargo, mut failed) = (0, 0, 0);
    for p in &projects {
        let from = PathBuf::from(p);
        let name = from.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned();
        let to = work.join(&name);
        let _ = std::fs::remove_dir_all(&to);
        copy_project(&from, &to)?;
        let manifest = std::fs::read_dir(&to)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| p.to_string_lossy().ends_with(".project.toml"))
            .ok_or_else(|| format!("{name}: no .project.toml"))?;
        let built = rcrun_build(&rcrun, &manifest)?;
        if built.ok && built.wasm {
            wasm += 1;
            rows.push(format!("  ok   {name}: the WebAssembly path, built in {:.0} ms", built.ms));
        } else if let Some(reason) = &built.reason {
            cargo += 1;
            rows.push(format!("  --   {name}: still on the Cargo path, because {reason}"));
        } else {
            failed += 1;
            rows.push(format!("  FAIL {name}: the Build failed with no Cargo reason ({:.0} ms)\n{}", built.ms, built.text));
        }
    }
    println!("== reference projects built by rcrun (DESKTOP-WASM-001 T22: AC1) ==");
    println!("rcrun: {}; rustc on PATH: {}", rcrun.display(), Command::new("rustc").arg("-V").output().is_ok());
    for r in &rows {
        println!("{r}");
    }
    println!(
        "reference projects {}: WebAssembly path {wasm}, still on the Cargo path {cargo} (listed with the reason), failed {failed}; AC1 is met when all {} take the WebAssembly path",
        projects.len(),
        projects.len()
    );
    Ok(failed == 0)
}

fn cross(args: &[String]) -> Result<bool, String> {
    let (named, _) = options(args);
    let runner_path = required(&named, "runner")?;
    let modules = required(&named, "modules")?;
    let dir = named.get("fixtures").map(PathBuf::from).unwrap_or_else(|| corpus::corpus_dir().join("fixtures"));
    let runner = std::fs::read(&runner_path).map_err(|e| format!("{}: {e}", runner_path.display()))?;
    let list = read_fixtures(&dir)?;
    let scratch = std::env::temp_dir().join(format!("prc-cross-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    let mut platforms: Vec<PathBuf> = std::fs::read_dir(&modules)
        .map_err(|e| format!("{}: {e}", modules.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    platforms.sort();
    let mut rows = Vec::new();
    let (mut total, mut identical) = (0, 0);
    let mut hashes: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for platform in &platforms {
        let from = platform.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned();
        for (name, _, run) in &list {
            let path = platform.join(format!("{name}.wasm"));
            let Ok(module) = std::fs::read(&path) else {
                rows.push(format!("  FAIL {from}/{name}.wasm is missing"));
                total += 1;
                continue;
            };
            hashes.entry(name.clone()).or_default().insert(from.clone(), corpus::sha256_hex(&module));
            total += 1;
            let got = corpus::run_module(&runner, &module, run, &scratch, &format!("cross-{from}-{name}"))?;
            let stdout = String::from_utf8_lossy(&got.stdout).replace("\r\n", "\n");
            let ok = stdout == run.stdout && got.code == Some(run.exit);
            if ok {
                identical += 1;
            }
            rows.push(format!(
                "  {} built on {from}, {name}: exit {:?}, output {}, {:.0} ms",
                if ok { "ok  " } else { "FAIL" },
                got.code,
                if stdout == run.stdout { "identical" } else { "DIFFERS" },
                got.ms
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    println!("== modules of every platform on this platform's runner (DESKTOP-WASM-001 T22: AC3, console) ==");
    println!("runner: {} on {}-{}", runner_path.display(), std::env::consts::OS, std::env::consts::ARCH);
    for r in &rows {
        println!("{r}");
    }
    for (name, by) in &hashes {
        let distinct: std::collections::BTreeSet<&String> = by.values().collect();
        println!("  {name}: the module is {} on the {} platforms", if distinct.len() == 1 { "the same file" } else { "NOT the same file" }, by.len());
    }
    println!("platforms {}, modules {total}, identical output and exit code {identical} of {total}", platforms.len());
    Ok(identical == total && total > 0)
}
