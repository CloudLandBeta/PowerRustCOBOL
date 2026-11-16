// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Add this release's entry to the module corpus (DESKTOP-WASM-001 T21; R5, R23; AC4).
//!
//! ```text
//! cargo run -p cobolt-compiler --example module_corpus_add -- [--release X.Y.Z] [--date YYYY-MM-DD]
//! ```
//!
//! For each fixture in `tests/module-corpus/fixtures/`, it builds the program as a Desktop Build
//! does (it must take the WebAssembly path), takes the module out of the built executable, runs it
//! alone on this checkout's runner under the fixture's pins, and refuses unless it prints and
//! answers exactly what the fixture's `run.toml` says. Then it writes
//! `releases/<release>/<name>.wasm.gz` and `<name>.toml`, checks the 256 KiB budget, appends the
//! entry to `index.toml` and prints the line the corpus test's `KNOWN` list gains.
//!
//! The release defaults to the IDE's version (`crates/cobolt-ide/src/version.rs`) and the date to
//! today (UTC). A release already in the index, or not after its last entry, is refused.

#[path = "../../cobolt-runner/tests/support/corpus.rs"]
mod corpus;

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    if let Err(e) = run() {
        eprintln!("module_corpus_add: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut release: Option<String> = None;
    let mut date: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--release" => release = args.next(),
            "--date" => date = args.next(),
            _ => return Err(format!("unknown argument {a}; usage: [--release X.Y.Z] [--date YYYY-MM-DD]")),
        }
    }
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().map_err(|e| e.to_string())?;
    let release = match release {
        Some(r) => r,
        None => ide_version(&workspace)?,
    };
    let date = date.unwrap_or_else(today);
    let dir = corpus::corpus_dir();
    let first = !dir.join("index.toml").is_file();
    let mut index = if first { corpus::Index::default() } else { corpus::read_index(&dir)? };
    let before = if first { Vec::new() } else { corpus::problems(&dir, &[]) };
    if !before.is_empty() {
        return Err(format!("the corpus is not whole; nothing is added:\n  {}", before.join("\n  ")));
    }
    if let Some(last) = index.entries.last() {
        if corpus::release_key(&release) <= corpus::release_key(&last.release) {
            return Err(format!("{release} is not after the last entry, {}; the index is append-only", last.release));
        }
    }

    let runner = build_runner(&workspace)?;
    let runner_bytes = std::fs::read(&runner).map_err(|e| format!("{}: {e}", runner.display()))?;
    let scratch = std::env::temp_dir().join(format!("prc-corpus-add-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;

    let mut fixtures: Vec<PathBuf> = std::fs::read_dir(dir.join("fixtures"))
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("run.toml").is_file())
        .collect();
    fixtures.sort();
    if fixtures.is_empty() {
        return Err("no fixture in tests/module-corpus/fixtures".into());
    }

    let rel_dir = format!("releases/{release}");
    let mut written: Vec<(String, Vec<u8>)> = Vec::new();
    let mut module_bytes = 0u64;
    for fixture in &fixtures {
        let name = fixture.file_name().and_then(|n| n.to_str()).ok_or("a fixture folder name is not text")?.to_owned();
        let run_text = std::fs::read(fixture.join("run.toml")).map_err(|e| e.to_string())?;
        let run = corpus::read_run(&fixture.join("run.toml"))?;
        let module = build_module(fixture, &name, &run, &runner, &scratch)?;
        let got = corpus::run_module(&runner_bytes, &module, &run, &scratch, &format!("corpus-{name}"))?;
        let stdout = String::from_utf8_lossy(&got.stdout).replace("\r\n", "\n");
        if stdout != run.stdout || got.code != Some(run.exit) {
            return Err(format!(
                "{name}: the module does not do what run.toml says (exit {:?}, want {}):\n{stdout}\n{}",
                got.code, run.exit, got.stderr
            ));
        }
        let gz = corpus::gzip(&module);
        module_bytes += gz.len() as u64;
        println!("  {name}: module {} bytes, {} compressed; ran in {:.0} ms, output identical", module.len(), gz.len(), got.ms);
        written.push((format!("{rel_dir}/{name}.wasm.gz"), gz));
        written.push((format!("{rel_dir}/{name}.toml"), run_text));
    }
    if module_bytes > corpus::BUDGET {
        return Err(format!("the modules take {module_bytes} bytes compressed, over the {} of a release", corpus::BUDGET));
    }

    std::fs::create_dir_all(dir.join(&rel_dir)).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for (path, bytes) in &written {
        std::fs::write(dir.join(path), bytes).map_err(|e| format!("{path}: {e}"))?;
        files.push(corpus::FileRef { path: path.clone(), sha256: corpus::sha256_hex(bytes) });
    }
    let prev = index.entries.last().map(|e| e.chain.clone()).unwrap_or_default();
    let chain = corpus::chain(&prev, &release, &files);
    index.entries.push(corpus::Entry { release: release.clone(), added: date.clone(), chain: chain.clone(), files });
    std::fs::write(dir.join("index.toml"), corpus::index_text(&index)).map_err(|e| e.to_string())?;
    let after = corpus::problems(&dir, &[]);
    if !after.is_empty() {
        return Err(format!("the corpus is not whole after the entry:\n  {}", after.join("\n  ")));
    }
    let _ = std::fs::remove_dir_all(&scratch);
    println!(
        "module corpus: {release} added on {date}: {} module(s), {module_bytes} bytes compressed of {}; {} entries",
        fixtures.len(),
        corpus::BUDGET,
        index.entries.len()
    );
    println!("add to KNOWN in crates/cobolt-runner/tests/module_corpus.rs:\n    (\"{release}\", \"{chain}\"),");
    Ok(())
}

/// The IDE's version, from `version.rs`.
fn ide_version(workspace: &Path) -> Result<String, String> {
    let path = workspace.join("crates/cobolt-ide/src/version.rs");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    text.lines()
        .find_map(|l| l.trim().strip_prefix("pub const VERSION: &str = \"").and_then(|r| r.strip_suffix("\";")))
        .map(str::to_owned)
        .ok_or_else(|| format!("{}: no VERSION", path.display()))
}

/// Today's date (UTC), `YYYY-MM-DD`.
fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // Days since 1970-01-01 to a civil date (H. Hinnant's algorithm).
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// This checkout's runner, built now from the current source.
fn build_runner(workspace: &Path) -> Result<PathBuf, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(workspace)
        .args(["build", "-q", "-p", "cobolt-runner", "--bin", "prc-runner"])
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("cargo build -p cobolt-runner --bin prc-runner failed".into());
    }
    // This example runs from <target>/<profile>/examples; the runner is in <target>/<profile>.
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let runner = exe.parent().and_then(Path::parent).ok_or("no target folder")?.join(cobolt_compiler::RUNNER_NAME);
    runner.is_file().then_some(runner.clone()).ok_or_else(|| format!("{} was not built", runner.display()))
}

/// The module a Desktop Build makes from the fixture: built on the WebAssembly path, then taken
/// out of the executable.
fn build_module(fixture: &Path, name: &str, run: &corpus::Run, runner: &Path, scratch: &Path) -> Result<Vec<u8>, String> {
    let project = scratch.join(name);
    std::fs::create_dir_all(&project).map_err(|e| e.to_string())?;
    for src in &run.sources {
        std::fs::copy(fixture.join(src), project.join(src)).map_err(|e| format!("{name}/{src}: {e}"))?;
    }
    let bin_name = format!("corpus{name}");
    let list = run.sources.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ");
    let manifest = project.join(format!("{bin_name}.project.toml"));
    std::fs::write(
        &manifest,
        format!("[project]\nname = \"{bin_name}\"\nversion = \"0.1.0\"\nmain = \"{}\"\n\n[files]\nsources = [{list}]\n", run.sources[0]),
    )
    .map_err(|e| e.to_string())?;
    let opts = cobolt_compiler::BuildOptions {
        verbose: false,
        build_dir: Some(scratch.join("builds")),
        runner: Some(runner.to_path_buf()),
        ..Default::default()
    };
    let built = cobolt_compiler::build_project(&manifest, &opts).map_err(|e| format!("{name}: the Build failed: {e}"))?;
    if built.path != cobolt_compiler::BuildPath::Wasm {
        let why = built.cargo.as_ref().map(|c| c.reason.to_string()).unwrap_or_default();
        return Err(format!("{name}: the Build took the Cargo path ({why}); a corpus fixture must take the WebAssembly path"));
    }
    let exe = std::fs::read(&built.binary_path).map_err(|e| e.to_string())?;
    let payload = cobolt_program_package::locate(&exe).map_err(|e| e.to_string())?;
    let decoded = cobolt_program_package::decode(payload).map_err(|e| e.to_string())?;
    decoded.module().map(<[u8]>::to_vec).map_err(|e| e.to_string())
}
