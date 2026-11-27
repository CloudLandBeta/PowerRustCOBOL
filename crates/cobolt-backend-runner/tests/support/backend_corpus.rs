// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Backend corpus's format and its run (spec 114 T40b; R3, AC9), shared by its test
//! (`tests/permanent.rs`) and the tool that adds a release's entry (`examples/backend_corpus_add.rs`), which both include this
//! file by path, after `corpus.rs` of the Desktop corpus (the index, the chain, the checks of the files are that one's).
//!
//! `tests/module-corpus/backend/`:
//! - `fixtures/<name>/`: a Backend's folder (`exports.toml`, `src/*.cbl`) and `calls.toml`, the calls made to it and what each
//!   must answer;
//! - `releases/<release>/<name>.backend.gz`: the Backend as that release's Build packs it for the runner (the interface,
//!   the table of exports, the state mode and the WebAssembly module), compressed, and `<name>.calls.toml`, the fixture's
//!   `calls.toml` as it was then;
//! - `index.toml`: one entry per release, append-only, with the chain of the Desktop corpus.
//!
//! A release's module must keep answering, on every later runner, what its release recorded.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cobolt_backend_runner::payload::{self, Package};
use cobolt_program_package::backend::BackendContainer;
use cobolt_server::engine::{Engine, ExecOptions};
use serde::Deserialize;

/// The most one release's Backends may take in the repository, compressed.
pub const BUDGET: u64 = 256 * 1024;

/// The comment at the top of `index.toml`, written again with every entry.
pub const INDEX_HEADER: &str = "\
# The Backend corpus (spec 114 R3, AC9): every Backend a release's Build packed from the fixtures in fixtures/, kept for
# ever. A Backend of any release must run on every later runner and answer what its calls.toml recorded
# (`cargo test -p cobolt-backend-runner --test permanent`).
#
# APPEND-ONLY. An entry is never deleted or edited: each names its files with their sha256, and its `chain` covers the
# previous entry's chain, so a removed or changed entry fails the test. A release adds its entry with
# `cargo run -p cobolt-backend-runner --example backend_corpus_add`, within 256 KiB.
";

/// The Backend corpus of this checkout.
pub fn dir() -> PathBuf {
    super::corpus::corpus_dir().join("backend")
}

/// `calls.toml`: the calls made to a fixture's Backend.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Calls {
    #[serde(rename = "call")]
    pub calls: Vec<Call>,
}

/// One call and what it must answer: the final JSON of each parameter (and of `RETURNING`), or the class and code it fails with.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Call {
    pub name: String,
    pub program: String,
    /// One JSON text per parameter.
    #[serde(default)]
    pub args: Vec<String>,
    /// The final JSON of each parameter, in order; compared as JSON, not as text.
    #[serde(default)]
    pub expect_args: Vec<String>,
    pub expect_returning: Option<String>,
    /// `"<class> <code>"`, for a call that must fail (`validation 4000`, `application 4201`).
    pub expect_error: Option<String>,
}

pub fn read_calls(path: &Path) -> Result<Calls, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// What one run of a Backend's calls gave.
pub struct Report {
    /// The calls that answered what was recorded.
    pub equal: u32,
    /// Each call that did not, in words.
    pub different: Vec<String>,
    /// The milliseconds the module took to load, and for all the calls.
    pub load_ms: f64,
    pub calls_ms: f64,
}

fn json_equal(a: &str, b: &str) -> bool {
    match (serde_json::from_str::<serde_json::Value>(a), serde_json::from_str::<serde_json::Value>(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// Load the Backend `container` holds on this checkout's runner (no machine code: the runner makes it) and make `calls` to it.
pub fn run_calls(container: BackendContainer, calls: &[Call]) -> Result<Report, String> {
    let began = Instant::now();
    let loaded = payload::load(Package { container, code: None }, &cobolt_backend_runner::provided()).map_err(|e| e.join("; "))?;
    let load_ms = began.elapsed().as_secs_f64() * 1000.0;
    let engine = loaded.engine.ok_or("the loader gave no engine")?;
    let mut report = run_against(&loaded.backend, engine.as_ref(), calls);
    report.load_ms = load_ms;
    Ok(report)
}

/// Make `calls` to `backend` on `engine`, and say which answered what was recorded.
pub fn run_against(backend: &cobolt_server::backend::Backend, engine: &dyn Engine, calls: &[Call]) -> Report {
    let (mut equal, mut different) = (0u32, Vec::new());
    let began = Instant::now();
    for call in calls {
        let Some(prog) = backend.program(&call.program) else {
            different.push(format!("{}: the Backend exports no {}", call.name, call.program));
            continue;
        };
        let opts = ExecOptions::plain(Arc::new(AtomicBool::new(false)), Duration::from_millis(500));
        let said = engine.execute(prog, &call.args, opts);
        let verdict = match (&said, &call.expect_error) {
            (Ok(r), None) => {
                let args_ok = r.args.len() == call.expect_args.len() && r.args.iter().zip(&call.expect_args).all(|(a, b)| json_equal(a, b));
                let returning_ok = match (&r.returning, &call.expect_returning) {
                    (Some(a), Some(b)) => json_equal(a, b),
                    (None, None) => true,
                    _ => false,
                };
                if args_ok && returning_ok {
                    None
                } else {
                    Some(format!("answered {:?} returning {:?}, recorded {:?} returning {:?}", r.args, r.returning, call.expect_args, call.expect_returning))
                }
            }
            (Err(e), Some(want)) => {
                let got = format!("{} {}", serde_json::to_value(e.class).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default(), e.code);
                (got != *want).then(|| format!("failed with {got}, recorded {want}"))
            }
            (Ok(r), Some(want)) => Some(format!("answered {:?}, recorded a failure {want}", r.args)),
            (Err(e), None) => Some(format!("failed with {:?} {}: {}, recorded an answer", e.class, e.code, e.message)),
        };
        match verdict {
            None => equal += 1,
            Some(why) => different.push(format!("{}: {why}", call.name)),
        }
    }
    Report { equal, different, load_ms: 0.0, calls_ms: began.elapsed().as_secs_f64() * 1000.0 }
}
