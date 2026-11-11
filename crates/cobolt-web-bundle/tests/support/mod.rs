// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Shared by the end-to-end tests: building the real player, assembling a bundle around the fixture
//! program, and finding `node`.

#![allow(dead_code)]

pub mod cdp;
pub mod harness;
pub mod project;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use cobolt_wasm_abi::fixture::{build, standard_cfrm, standard_spec, Fixture};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, BundleOutput, FormInput, PlayerInput, PlayerPartInput, PlayerVariantInput};

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

pub fn node_available() -> bool {
    Command::new("node").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Is `node` there to run the test? Without it the test prints `skipped: node not on PATH` and passes
/// (WASM-015 T2: a machine without node is not a broken build); with `PRC_REQUIRE_NODE=1` its absence fails,
/// so CI that must run these cannot skip them silently.
pub fn require_node(test: &str) -> bool {
    if node_available() {
        return true;
    }
    if std::env::var("PRC_REQUIRE_NODE").is_ok_and(|v| v == "1") {
        panic!("{test}: node is required (PRC_REQUIRE_NODE=1) and was not found on PATH");
    }
    println!("skipped: node not on PATH ({test})");
    false
}

fn newest(dir: &Path) -> SystemTime {
    let mut t = SystemTime::UNIX_EPOCH;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            let m = if p.is_dir() { newest(&p) } else { e.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH) };
            t = t.max(m);
        }
    }
    t
}

/// Build the plain player with the product's own build step (unless a build newer than every source is already there)
/// and return the directory that holds `plain/`.
pub fn built_player() -> PathBuf {
    // The tests of one binary run in parallel threads: two of them must not both build into the same folder.
    static BUILDING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one_at_a_time = BUILDING.lock().unwrap_or_else(|e| e.into_inner());
    let crate_dir = repo_root().join("crates/cobolt-web-player");
    let out = crate_dir.join("target-player/out-test");
    let wasm = out.join("plain/player_bg.wasm");
    let stamp = wasm.metadata().and_then(|m| m.modified()).ok().filter(|_| out.join("parts/mermaid/mermaid_bg.wasm").exists());
    let sources = newest(&crate_dir.join("src")).max(newest(&crate_dir.join("examples"))).max(newest(&crate_dir.join("parts"))).max(newest(&repo_root().join("crates/cobolt-forms/src"))).max(
        crate_dir.join("Cargo.toml").metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH),
    );
    if stamp.map(|s| s < sources).unwrap_or(true) {
        eprintln!("building the plain player (cargo run --example build_player) ...");
        let status = Command::new("cargo")
            .args(["run", "--quiet", "--manifest-path"])
            .arg(crate_dir.join("Cargo.toml"))
            // The test bundles' player carries the test hooks (WASM-017 R15: the forced panic).
            .args(["--example", "build_player", "--", "--variants", "plain", "--test-hooks", "--out"])
            .arg(&out)
            .status()
            .expect("cargo runs");
        assert!(status.success(), "the player build failed");
    }
    out
}

pub fn player_variant(out: &Path, name: &str) -> PlayerVariantInput {
    let dir = out.join(name);
    let mut files = Vec::new();
    fn walk(base: &Path, dir: &Path, files: &mut Vec<(String, Vec<u8>)>) {
        // A hidden entry (`.DS_Store`) is the machine's, never the player's, as the Build reads the folder.
        for e in std::fs::read_dir(dir).unwrap().flatten().filter(|e| !e.file_name().to_string_lossy().starts_with('.')) {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, files);
            } else {
                files.push((p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"), std::fs::read(&p).unwrap()));
            }
        }
    }
    walk(&dir, &dir, &mut files);
    PlayerVariantInput { files, entry_js: "player.js".into(), entry_wasm: "player_bg.wasm".into() }
}

/// The parts `build_player` wrote under `<out>/parts/<name>/`.
pub fn player_parts(out: &Path) -> Vec<PlayerPartInput> {
    let mut parts = Vec::new();
    let Ok(rd) = std::fs::read_dir(out.join("parts")) else { return parts };
    for e in rd.flatten().filter(|e| e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('.')) {
        let name = e.file_name().to_string_lossy().into_owned();
        let mut files = Vec::new();
        for f in std::fs::read_dir(e.path()).unwrap().flatten().filter(|f| !f.file_name().to_string_lossy().starts_with('.')) {
            files.push((f.file_name().to_string_lossy().into_owned(), std::fs::read(f.path()).unwrap()));
        }
        parts.push(PlayerPartInput { entry_js: format!("{name}.js"), entry_wasm: format!("{name}_bg.wasm"), name, files });
    }
    parts
}

/// A module that imports `env.surprise`, which ABI v1 §3 does not provide.
pub fn surprise_module() -> Vec<u8> {
    use wasm_encoder::{EntityType, ImportSection, Module, TypeSection};
    let mut m = Module::new();
    let mut t = TypeSection::new();
    t.ty().function([], []);
    m.section(&t);
    let mut i = ImportSection::new();
    i.import("env", "surprise", EntityType::Function(0));
    m.section(&i);
    m.finish()
}

pub struct Fixtures {
    pub multi: Fixture,
    pub single: Fixture,
}

pub fn fixtures() -> Fixtures {
    Fixtures { multi: build(&standard_spec(true)), single: build(&standard_spec(false)) }
}

pub fn input_with(player: PlayerInput, options: BundleOptions) -> BundleInput {
    let fx = fixtures();
    let forms = fx
        .multi
        .manifest
        .forms
        .iter()
        .map(|f| FormInput { name: f.name.clone(), cfrm: standard_cfrm(&fx.multi, &f.name), stylesheet: None })
        .collect();
    BundleInput {
        project: "fixture".into(),
        program_multi: Some(fx.multi.wasm),
        program_single: Some(fx.single.wasm),
        forms,
        stylesheet: None,
        menus: Vec::new(),
        assets: Vec::new(),
        player,
        options,
    }
}

pub fn assemble(player: PlayerInput, options: BundleOptions) -> BundleOutput {
    assemble_bundle(&input_with(player, options)).unwrap_or_else(|e| panic!("{e}"))
}

pub fn write_bundle(out: &BundleOutput, dir: &Path) {
    for f in &out.files {
        let p = dir.join(&f.path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, &f.bytes).unwrap();
    }
}

/// A copy of the scenario project (`tests/projects/core/`) in a fresh folder of the system's temporary
/// directory, tagged `tag` (the Web Build writes `dist/web` beside the manifest, never into the repository).
/// The folder goes when the returned value is dropped, so a test run leaves nothing behind.
pub fn scenario_project(tag: &str) -> tempfile::TempDir {
    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let (src, dst) = (e.path(), to.join(e.file_name()));
            if src.is_dir() {
                copy(&src, &dst);
            } else {
                std::fs::copy(&src, &dst).unwrap();
            }
        }
    }
    let dst = tempfile::Builder::new().prefix(&format!("prc-core-{tag}-")).tempdir().unwrap();
    copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/projects/core"), dst.path());
    dst
}
