// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **DESKTOP-WASM-001 T24: `cobolt-bench pixel-parity --console`** (R27, console part): a console
//! program built on the WebAssembly path prints what the same program built on the Cargo path
//! prints, byte for byte, and answers the same exit code.
//!
//! ```sh
//! cargo build --release -p cobolt-runner --bin prc-runner
//! cargo run -p cobolt-bench --release -- pixel-parity --console
//! ```
//!
//! **What it runs:** every fixture under `tests/pixel-parity/console/<name>/`: its sources and a
//! `run.toml` naming them (the main program first, its Common Code after it) and what the program
//! is given: `stdin`, `args` and `env`. Each fixture is copied twice into a scratch folder, with a
//! project file, and built once with `force_cargo` and once by default. The default Build must take
//! the WebAssembly path; a fixture it sends to the Cargo path fails, with the reason, since both
//! programs would then be the same.
//!
//! **The runs:** both programs run in an empty folder of their own, under the pins of R28
//! (`PRC_TEST_CLOCK`, `PRC_TEST_SEED`, the same as the form runs'), with the fixture's input,
//! arguments and variables, and a time limit. Their standard output must be identical byte for
//! byte, and their exit codes equal. When they are not, both outputs go under
//! `target/pixel-parity/console/<name>/`, and the first line that differs is printed.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::build_goal::{platform, product_version, repo_root};

/// The pins every run takes (R28): the form runs' own.
use super::pixel_parity::{CLOCK, SEED};

/// How long one run may take before it counts as hung.
const LIMIT: Duration = Duration::from_secs(60);

/// A fixture's `run.toml`.
#[derive(Deserialize)]
struct RunToml {
    sources: Vec<String>,
    #[serde(default)]
    stdin: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

/// What one program did.
struct Ran {
    stdout: Vec<u8>,
    exit: Option<i32>,
    took: Duration,
}

struct Fixture {
    name: String,
    lines: usize,
    build_cargo: Duration,
    build_wasm: Duration,
    run_cargo: Duration,
    run_wasm: Duration,
    /// Empty when both programs printed the same and answered the same code.
    problems: Vec<String>,
}

fn fixtures_dir() -> PathBuf {
    repo_root().join("tests/pixel-parity/console")
}

/// The fixture's files and a project file, under `to`; the project file's path.
fn lay_out(fixture: &Path, run: &RunToml, name: &str, to: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for s in &run.sources {
        std::fs::copy(fixture.join(s), to.join(s)).map_err(|e| format!("{name}/{s}: {e}"))?;
    }
    let bin = format!("parity{name}");
    let list = run.sources.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ");
    let manifest = to.join(format!("{bin}.project.toml"));
    let main = run.sources.first().ok_or_else(|| format!("{name}: run.toml names no sources"))?;
    std::fs::write(&manifest, format!("[project]\nname = \"{bin}\"\nversion = \"0.1.0\"\nmain = \"{main}\"\n\n[files]\nsources = [{list}]\n"))
        .map_err(|e| format!("{}: {e}", manifest.display()))?;
    Ok(manifest)
}

/// Runs `binary` in a folder of its own with the fixture's input and the pins.
fn run_program(binary: &Path, run: &RunToml, work: &Path) -> Result<Ran, String> {
    std::fs::create_dir_all(work).map_err(|e| format!("{}: {e}", work.display()))?;
    let started = Instant::now();
    let mut child = Command::new(binary)
        .args(&run.args)
        .current_dir(work)
        .env("PRC_TEST_CLOCK", CLOCK)
        .env("PRC_TEST_SEED", SEED.to_string())
        .envs(&run.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{}: {e}", binary.display()))?;
    let mut stdin = child.stdin.take();
    let input = run.stdin.clone().into_bytes();
    let feeder = std::thread::spawn(move || {
        if let Some(s) = stdin.as_mut() {
            let _ = s.write_all(&input);
        }
        drop(stdin);
    });
    let mut out = child.stdout.take().ok_or("no standard output")?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out.read_to_end(&mut buf);
        buf
    });
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        if started.elapsed() > LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{} did not end within {} s", binary.display(), LIMIT.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let took = started.elapsed();
    let _ = feeder.join();
    let stdout = reader.join().unwrap_or_default();
    Ok(Ran { stdout, exit: status.code(), took })
}

/// The first line where `a` and `b` differ, both shown.
fn first_difference(a: &[u8], b: &[u8]) -> String {
    let (la, lb): (Vec<&[u8]>, Vec<&[u8]>) = (a.split(|c| *c == b'\n').collect(), b.split(|c| *c == b'\n').collect());
    for i in 0..la.len().max(lb.len()) {
        let (x, y) = (la.get(i).copied(), lb.get(i).copied());
        if x != y {
            let show = |l: Option<&[u8]>| l.map(|l| format!("{:?}", String::from_utf8_lossy(l))).unwrap_or_else(|| "(no line)".into());
            return format!("line {}: Cargo {} / WebAssembly {}", i + 1, show(x), show(y));
        }
    }
    "the same lines".into()
}

fn check(fixture: &Path, name: &str, scratch: &Path, build_root: &Path, runner: &Path) -> Fixture {
    let mut f = Fixture {
        name: name.to_owned(),
        lines: 0,
        build_cargo: Duration::ZERO,
        build_wasm: Duration::ZERO,
        run_cargo: Duration::ZERO,
        run_wasm: Duration::ZERO,
        problems: Vec::new(),
    };
    let outcome = (|| -> Result<(), String> {
        let text = std::fs::read_to_string(fixture.join("run.toml")).map_err(|e| format!("run.toml: {e}"))?;
        let run: RunToml = toml::from_str(&text).map_err(|e| format!("run.toml: {e}"))?;
        let mut binaries = Vec::new();
        for (path, force_cargo) in [("cargo", true), ("wasm", false)] {
            let manifest = lay_out(fixture, &run, name, &scratch.join(path).join(name))?;
            eprintln!("pixel-parity --console: building {name} on the {} path…", if force_cargo { "Cargo" } else { "WebAssembly" });
            let opts = cobolt_compiler::BuildOptions {
                verbose: false,
                workspace_root: Some(repo_root()),
                build_dir: Some(build_root.to_path_buf()),
                runner: Some(runner.to_path_buf()),
                force_cargo,
                ..Default::default()
            };
            let t = Instant::now();
            let built = cobolt_compiler::build_project(&manifest, &opts).map_err(|e| format!("the {path} Build failed: {e}"))?;
            let took = t.elapsed();
            if force_cargo {
                f.build_cargo = took;
            } else {
                f.build_wasm = took;
                if built.path != cobolt_compiler::BuildPath::Wasm {
                    let why = built.cargo.as_ref().map(|c| c.reason.to_string()).unwrap_or_default();
                    return Err(format!("the default Build took the Cargo path ({why}), so there is nothing to compare"));
                }
            }
            binaries.push(built.binary_path);
        }
        let cargo = run_program(&binaries[0], &run, &scratch.join("runs").join(name).join("cargo"))?;
        let wasm = run_program(&binaries[1], &run, &scratch.join("runs").join(name).join("wasm"))?;
        f.run_cargo = cargo.took;
        f.run_wasm = wasm.took;
        f.lines = cargo.stdout.split(|c| *c == b'\n').filter(|l| !l.is_empty()).count();
        if cargo.stdout != wasm.stdout {
            f.problems.push(format!("standard output differs at {}", first_difference(&cargo.stdout, &wasm.stdout)));
        }
        if cargo.exit != wasm.exit {
            f.problems.push(format!("exit code: Cargo {:?}, WebAssembly {:?}", cargo.exit, wasm.exit));
        }
        if !f.problems.is_empty() {
            let dir = repo_root().join("target/pixel-parity/console").join(name);
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            std::fs::write(dir.join("cargo.stdout"), &cargo.stdout).map_err(|e| e.to_string())?;
            std::fs::write(dir.join("wasm.stdout"), &wasm.stdout).map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if let Err(e) = outcome {
        f.problems.push(e);
    }
    f
}

/// The `--console` mode.
pub fn run() -> Result<(), String> {
    let started = Instant::now();
    let runner = cobolt_compiler::runner_near(&std::env::current_exe().map_err(|e| e.to_string())?).ok_or(
        "pixel-parity --console: no prc-runner beside this program; build it first: cargo build --release -p cobolt-runner --bin prc-runner",
    )?;
    let mut names: Vec<(String, PathBuf)> = std::fs::read_dir(fixtures_dir())
        .map_err(|e| format!("{}: {e}", fixtures_dir().display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("run.toml").is_file())
        .map(|p| (p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), p))
        .collect();
    names.sort();
    if names.is_empty() {
        return Err(format!("pixel-parity --console: no fixture under {}", fixtures_dir().display()));
    }
    let scratch = std::env::temp_dir().join(format!("prc-console-parity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let _ = std::fs::remove_dir_all(repo_root().join("target/pixel-parity/console"));
    let build_root = scratch.join("build");
    let results: Vec<Fixture> = names.iter().map(|(n, p)| check(p, n, &scratch, &build_root, &runner)).collect();
    let _ = std::fs::remove_dir_all(&scratch);

    // The result block (GOLDEN RULE #7).
    println!("== cobolt-bench pixel-parity --console (DESKTOP-WASM-001 T24: R27, console part) ==");
    println!(
        "platform {}, version {}; clock {CLOCK}, seed {SEED}; runner {}; standard output compared byte for byte, and the exit code",
        platform(),
        product_version(),
        runner.display()
    );
    println!(
        "{:<12} {:>6} {:>14} {:>14} {:>13} {:>13}  result",
        "program", "lines", "Cargo build s", "Wasm build s", "Cargo run ms", "Wasm run ms"
    );
    for f in &results {
        println!(
            "{:<12} {:>6} {:>14.1} {:>14.1} {:>13.1} {:>13.1}  {}",
            f.name,
            f.lines,
            f.build_cargo.as_secs_f64(),
            f.build_wasm.as_secs_f64(),
            f.run_cargo.as_secs_f64() * 1000.0,
            f.run_wasm.as_secs_f64() * 1000.0,
            if f.problems.is_empty() { "identical" } else { "DIFFERS" }
        );
        for p in &f.problems {
            println!("  {}: {p}", f.name);
        }
    }
    let same = results.iter().filter(|f| f.problems.is_empty()).count();
    let lines: usize = results.iter().map(|f| f.lines).sum();
    println!(
        "programs {}, identical output {same} of {}; {lines} lines compared; total {:.1} min",
        results.len(),
        results.len(),
        started.elapsed().as_secs_f64() / 60.0
    );
    if same == results.len() {
        Ok(())
    } else {
        Err(format!("pixel-parity --console: {} of {} programs differ (outputs under target/pixel-parity/console/)", results.len() - same, results.len()))
    }
}
