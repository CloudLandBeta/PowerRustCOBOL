// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the module-path Build tests share (spec 114 M4): a scratch copy of a shipped Backend, the runner this crate built
//! with the bill of materials a release writes beside it, the options of a Build, and a started file's answer.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use cobolt_compiler::backend_build::BackendBuildOptions;
use cobolt_compiler::backend_module::{BackendPath, BACKEND_RUNNER_SBOM};

pub fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

pub fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let path = entry.path();
        let file = entry.file_name().to_string_lossy().into_owned();
        if file.ends_with(".lck") || file.ends_with(".redb") || file.ends_with("-chunked.data") || file == ".DS_Store" {
            continue;
        }
        let target = to.join(&file);
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
    }
}

/// The Build stages its files in a folder named after the project's folder (`cobolt-build-<name>-backend`), and every
/// scratch copy here is a folder named after the shipped project, so two tests that finish a Build of the same project
/// would write into, and clear, one folder. A test that completes a Build holds this for its length.
pub fn one_build_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    static BUILD: std::sync::Mutex<()> = std::sync::Mutex::new(());
    BUILD.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A copy of a shipped project's Backend in a scratch folder: its manifest, `backend/` and `COPYBOOKS/`.
pub fn project(name: &str, tag: &str) -> PathBuf {
    let from = examples().join(name);
    let to = std::env::temp_dir().join(format!("prc-backend-build-{tag}-{}", std::process::id())).join(name);
    let _ = std::fs::remove_dir_all(&to);
    for sub in ["backend", "COPYBOOKS"] {
        if from.join(sub).is_dir() {
            copy_tree(&from.join(sub), &to.join(sub));
        }
    }
    std::fs::copy(from.join(format!("{name}.project.toml")), to.join(format!("{name}.project.toml"))).unwrap();
    to
}

/// The runner this crate built, and a copy of it with the bill of materials a release writes beside it.
pub fn runner_with_sbom(dir: &Path) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let runner = dir.join(cobolt_compiler::backend_module::BACKEND_RUNNER_NAME);
    std::fs::copy(env!("CARGO_BIN_EXE_prc-backend-runner"), &runner).unwrap();
    let sbom = serde_json::json!({
        "bomFormat": "CycloneDX", "specVersion": "1.5", "version": 1,
        "components": [
            cobolt_wire::sbom::file_component("prc-backend-runner", "00"),
            cobolt_wire::sbom::library_component("wasmtime", "44.0.3", "Apache-2.0"),
            cobolt_wire::sbom::library_component("tokio", "1.40.0", "MIT"),
        ]
    });
    std::fs::write(dir.join(BACKEND_RUNNER_SBOM), serde_json::to_string_pretty(&sbom).unwrap()).unwrap();
    runner
}

pub fn options(runner: &Path, path: Option<BackendPath>) -> BackendBuildOptions {
    BackendBuildOptions { runner: Some(runner.to_path_buf()), path, ..BackendBuildOptions::default() }
}

/// Every file under `dir` with its bytes, for the check that a Build changed no source.
pub fn sources(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, at: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(read) = std::fs::read_dir(at) else { return };
        for e in read.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                // What the Build writes by design: the interface, the bin folder, the identity programs.
                if rel.starts_with("bin/") || rel.starts_with("backend/contract/interface.json") || rel.starts_with("backend/generated/identity") {
                    continue;
                }
                out.push((rel, std::fs::read(&p).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

pub struct Ran {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub fn run(exe: &Path, cwd: &Path, args: &[&str]) -> Ran {
    let out = Command::new(exe).args(args).current_dir(cwd).env_remove("NOTIFY_SOCKET").output().unwrap();
    Ran { code: out.status.code(), stdout: String::from_utf8_lossy(&out.stdout).into_owned(), stderr: String::from_utf8_lossy(&out.stderr).into_owned() }
}
