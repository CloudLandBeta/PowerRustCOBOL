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
            .args(["--example", "build_player", "--", "--variants", "plain", "--out"])
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
        for e in std::fs::read_dir(dir).unwrap().flatten() {
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
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let mut files = Vec::new();
        for f in std::fs::read_dir(e.path()).unwrap().flatten() {
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
