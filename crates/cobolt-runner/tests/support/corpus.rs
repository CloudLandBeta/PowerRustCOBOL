// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The module corpus's format (DESKTOP-WASM-001 T21; R5, R23; AC4), shared by its test
//! (`cobolt-runner/tests/module_corpus.rs`) and the tool that adds a release's entry
//! (`cobolt-compiler/examples/module_corpus_add.rs`, which includes this file by path).
//!
//! `tests/module-corpus/`:
//! - `fixtures/<name>/`: a console program's sources and `run.toml`, what it is given (standard
//!   input, arguments, variables, the pinned clock and seed) and what it must print and answer;
//! - `releases/<release>/<name>.wasm.gz`: the module that release's Build made from the fixture,
//!   compressed, and `<name>.toml`, the fixture's `run.toml` as it was then;
//! - `index.toml`: one entry per release, append-only. Each entry names its files with their
//!   sha256 and carries a `chain`, the sha256 of the previous entry's chain and its own files, so
//!   an entry taken out of the middle breaks the next one's chain.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The most one release's modules may take in the repository, compressed.
pub const BUDGET: u64 = 256 * 1024;

/// The comment at the top of `index.toml`, written again with every entry.
pub const INDEX_HEADER: &str = "\
# The module corpus (DESKTOP-WASM-001 R5, R23; AC4): every module a release's Build made from the
# fixtures in fixtures/, kept for ever. A module of any release must run on every later runner and
# print what its .toml recorded (`cargo test -p cobolt-runner --test module_corpus`).
#
# APPEND-ONLY. An entry is never deleted or edited: each names its files with their sha256, and its
# `chain` covers the previous entry's chain, so a removed or changed entry fails the test. A release
# adds its entry with `cargo run -p cobolt-compiler --example module_corpus_add`, within 256 KiB.
";

/// What a fixture is given and what it must do.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Run {
    /// The fixture's sources, the main program first (its Common Code after it).
    pub sources: Vec<String>,
    /// The lines on its standard input.
    #[serde(default)]
    pub stdin: String,
    /// Its command-line arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// The variables set for it (beside the pins).
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// `PRC_TEST_CLOCK`, an RFC 3339 date and time.
    pub clock: String,
    /// `PRC_TEST_SEED`.
    pub seed: u64,
    /// The exit code it answers.
    pub exit: i32,
    /// What it prints, byte for byte.
    pub stdout: String,
}

/// One file of an entry, with its sha256.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct FileRef {
    pub path: String,
    pub sha256: String,
}

/// One release's entry.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Entry {
    pub release: String,
    pub added: String,
    pub chain: String,
    pub files: Vec<FileRef>,
}

/// `index.toml`.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Index {
    #[serde(default, rename = "entry")]
    pub entries: Vec<Entry>,
}

/// The corpus of this checkout.
pub fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/module-corpus")
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The chain of an entry: the sha256 of the previous entry's chain, the release and each file's
/// path and sha256, one per line.
pub fn chain(prev: &str, release: &str, files: &[FileRef]) -> String {
    let mut text = format!("{prev}\n{release}\n");
    for f in files {
        text.push_str(&format!("{} {}\n", f.path, f.sha256));
    }
    sha256_hex(text.as_bytes())
}

pub fn read_index(dir: &Path) -> Result<Index, String> {
    let path = dir.join("index.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn index_text(index: &Index) -> String {
    format!("{INDEX_HEADER}\n{}", toml::to_string(index).expect("the index serialises"))
}

pub fn read_run(path: &Path) -> Result<Run, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    enc.write_all(bytes).expect("compress");
    enc.finish().expect("compress")
}

pub fn gunzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes).read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// The release numbers' order: `2.0.9` before `2.0.10`.
pub fn release_key(release: &str) -> Vec<u64> {
    release.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// An entry's modules: each `.wasm.gz` with the `.toml` beside it.
pub fn modules(entry: &Entry) -> Vec<(String, String)> {
    entry
        .files
        .iter()
        .filter_map(|f| f.path.strip_suffix(".wasm.gz").map(|stem| (f.path.clone(), format!("{stem}.toml"))))
        .collect()
}

/// Everything wrong with the corpus at `dir`, each as one sentence; empty when it is whole.
/// `known` are the (release, chain) pairs the test holds: each must still be in the index, so the
/// last entry cannot go either.
pub fn problems(dir: &Path, known: &[(&str, &str)]) -> Vec<String> {
    let mut out = Vec::new();
    let index = match read_index(dir) {
        Ok(i) => i,
        Err(e) => return vec![format!("the index cannot be read: {e}")],
    };
    let mut prev = String::new();
    let mut last: Option<Vec<u64>> = None;
    for e in &index.entries {
        let key = release_key(&e.release);
        if last.as_ref().is_some_and(|l| *l >= key) {
            out.push(format!("{}: the entries are not in release order", e.release));
        }
        last = Some(key);
        let mut module_bytes = 0u64;
        for f in &e.files {
            match std::fs::read(dir.join(&f.path)) {
                Ok(bytes) => {
                    if sha256_hex(&bytes) != f.sha256 {
                        out.push(format!("{}: {} was changed (sha256 differs)", e.release, f.path));
                    }
                    if f.path.ends_with(".wasm.gz") {
                        module_bytes += bytes.len() as u64;
                    }
                }
                Err(_) => out.push(format!("{}: {} is missing", e.release, f.path)),
            }
        }
        for (module, run) in modules(e) {
            if !e.files.iter().any(|f| f.path == run) {
                out.push(format!("{}: {module} has no {run}", e.release));
            }
        }
        if module_bytes > BUDGET {
            out.push(format!("{}: the modules take {module_bytes} bytes, over the {BUDGET} of a release", e.release));
        }
        let want = chain(&prev, &e.release, &e.files);
        if want != e.chain {
            out.push(format!("{}: the chain is broken (an entry before it was removed or changed)", e.release));
        }
        prev = e.chain.clone();
    }
    for (release, chain) in known {
        if !index.entries.iter().any(|e| e.release == *release && e.chain == *chain) {
            out.push(format!("{release}: the entry is gone from the index"));
        }
    }
    out
}

/// What one run of a module gave.
pub struct Outcome {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub code: Option<i32>,
    pub ms: f64,
}

/// Put `module` alone into a copy of `runner` (no machine code: the runner makes it, R16), write it
/// under `scratch` as `name`, and run it as `run` says, with the pins.
pub fn run_module(runner: &[u8], module: &[u8], run: &Run, scratch: &Path, name: &str) -> Result<Outcome, String> {
    use cobolt_program_package::Kind;
    let payload = cobolt_program_package::encode(&[(Kind::Module, module)]);
    let exe = cobolt_program_package::embed(runner, &payload, name).map_err(|e| e.to_string())?;
    let path = scratch.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    cobolt_program_package::write_executable(&path, &exe).map_err(|e| e.to_string())?;
    let mut cmd = Command::new(&path);
    cmd.args(&run.args)
        .env("PRC_TEST_CLOCK", &run.clock)
        .env("PRC_TEST_SEED", run.seed.to_string())
        .env("PRC_RUNNER_CACHE", scratch.join("cache"))
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
    Ok(Outcome {
        stdout: out.stdout,
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        code: out.status.code(),
        ms: t.elapsed().as_secs_f64() * 1000.0,
    })
}
