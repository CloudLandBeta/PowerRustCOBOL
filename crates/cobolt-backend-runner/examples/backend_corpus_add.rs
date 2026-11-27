// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Add this release's entry to the Backend corpus (spec 114 T40b; R3, AC9).
//!
//! ```text
//! cargo run -p cobolt-backend-runner --example backend_corpus_add -- [--release X.Y.Z] [--date YYYY-MM-DD]
//! ```
//!
//! For each fixture in `tests/module-corpus/backend/fixtures/`, it loads the Backend from its sources, makes every call of
//! its `calls.toml` to the interpreter and refuses unless each answers what the table says; then it builds the module and
//! the container as a Backend Build packs them (`backend_module::generate`, `backend_module::backend_container`), makes the
//! same calls to the module on this checkout's runner and refuses unless each answers the same. Then it writes
//! `releases/<release>/<name>.backend.gz` and `<name>.calls.toml`, checks the 256 KiB budget, appends the entry to
//! `index.toml` and prints the line the test's `KNOWN` list gains.
//!
//! The release defaults to the IDE's version (`crates/cobolt-ide/src/version.rs`) and the date to today (UTC). A release
//! already in the index, or not after its last entry, is refused.

#[path = "../../cobolt-runner/tests/support/corpus.rs"]
mod corpus;
#[path = "../tests/support/backend_corpus.rs"]
mod backend_corpus;

use std::path::{Path, PathBuf};

use cobolt_server::backend::Backend;
use cobolt_server::exec::InterpreterEngine;

fn main() {
    if let Err(e) = run() {
        eprintln!("backend_corpus_add: {e}");
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
    let dir = backend_corpus::dir();
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

    let mut fixtures: Vec<PathBuf> = std::fs::read_dir(dir.join("fixtures"))
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("calls.toml").is_file())
        .collect();
    fixtures.sort();
    if fixtures.is_empty() {
        return Err("no fixture in tests/module-corpus/backend/fixtures".into());
    }

    let rel_dir = format!("releases/{release}");
    let mut written: Vec<(String, Vec<u8>)> = Vec::new();
    let mut packed = 0u64;
    for fixture in &fixtures {
        let name = fixture.file_name().and_then(|n| n.to_str()).ok_or("a fixture folder name is not text")?.to_owned();
        let calls_text = std::fs::read(fixture.join("calls.toml")).map_err(|e| e.to_string())?;
        let calls = backend_corpus::read_calls(&fixture.join("calls.toml"))?;
        let backend = Backend::load(fixture).map_err(|e| format!("{name}: the Backend does not load:\n  {}", e.join("\n  ")))?;

        // The interpreter first: what the table says is what the language does, not only what the module does.
        let by_interpreter = backend_corpus::run_against(&backend, &InterpreterEngine, &calls.calls);
        if !by_interpreter.different.is_empty() {
            return Err(format!("{name}: the interpreter does not answer what calls.toml says:\n  {}", by_interpreter.different.join("\n  ")));
        }

        let compiled = cobolt_compiler::backend_module::generate(&backend, &[]).map_err(|r| format!("{name}: the generator refuses the Backend: {r:?}"))?;
        let container = cobolt_compiler::backend_module::backend_container(&backend, &compiled.wasm, backend.state, &name).map_err(|e| e.to_string())?;
        let by_module = backend_corpus::run_calls(container.clone(), &calls.calls)?;
        if !by_module.different.is_empty() {
            return Err(format!("{name}: the module does not answer what calls.toml says:\n  {}", by_module.different.join("\n  ")));
        }
        let bytes = container.encode();
        let gz = corpus::gzip(&bytes);
        packed += gz.len() as u64;
        println!(
            "  {name}: {} calls, interpreter {} equal in {:.0} ms, module {} equal ({:.0} ms to load, {:.0} ms for the calls); container {} bytes, {} compressed",
            calls.calls.len(),
            by_interpreter.equal,
            by_interpreter.calls_ms,
            by_module.equal,
            by_module.load_ms,
            by_module.calls_ms,
            bytes.len(),
            gz.len()
        );
        written.push((format!("{rel_dir}/{name}.backend.gz"), gz));
        written.push((format!("{rel_dir}/{name}.calls.toml"), calls_text));
    }
    if packed > backend_corpus::BUDGET {
        return Err(format!("the Backends take {packed} bytes compressed, over the {} of a release", backend_corpus::BUDGET));
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
    let text = format!("{}\n{}", backend_corpus::INDEX_HEADER, toml::to_string(&index).map_err(|e| e.to_string())?);
    std::fs::write(dir.join("index.toml"), text).map_err(|e| e.to_string())?;
    let after = corpus::problems(&dir, &[]);
    if !after.is_empty() {
        return Err(format!("the corpus is not whole after the entry:\n  {}", after.join("\n  ")));
    }
    println!(
        "Backend corpus: {release} added on {date}: {} Backend(s), {packed} bytes compressed of {}; {} entries",
        fixtures.len(),
        backend_corpus::BUDGET,
        index.entries.len()
    );
    println!("add to KNOWN in crates/cobolt-backend-runner/tests/permanent.rs:\n    (\"{release}\", \"{chain}\"),");
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
