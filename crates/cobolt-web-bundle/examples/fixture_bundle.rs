// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `cargo run -p cobolt-web-bundle --example fixture_bundle -- --player <build_player out dir> --out <dir>
//!  [--threaded] [--isolation require-corp|credentialless|off] [--tools <dir>]`
//!
//! Assembles a Web bundle around the fixture program module (both variants) and the player built by
//! `build_player`, and writes it to `<dir>`: what the end-to-end tests assemble, kept on disk so that a
//! browser or `node` can be pointed at it. With `--tools` it also writes the node harness there.

#[path = "../tests/support/harness.rs"]
mod harness;

use std::path::{Path, PathBuf};

use cobolt_wasm_abi::fixture::{build, standard_cfrm, standard_spec};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput, PlayerVariantInput};

fn variant(out: &Path, name: &str) -> Option<PlayerVariantInput> {
    let dir = out.join(name);
    if !dir.is_dir() {
        return None;
    }
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
    Some(PlayerVariantInput { files, entry_js: "player.js".into(), entry_wasm: "player_bg.wasm".into() })
}

fn main() {
    let mut player: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut tools: Option<PathBuf> = None;
    let mut isolation = Isolation::Credentialless;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--player" => player = it.next().map(Into::into),
            "--out" => out = it.next().map(Into::into),
            "--tools" => tools = it.next().map(Into::into),
            "--isolation" => {
                isolation = match it.next().unwrap().as_str() {
                    "require-corp" => Isolation::RequireCorp,
                    "off" => Isolation::Off,
                    _ => Isolation::Credentialless,
                }
            }
            other => panic!("unknown argument {other}"),
        }
    }
    let player = player.expect("--player <dir>");
    let out = out.expect("--out <dir>");
    let multi = build(&standard_spec(true));
    let single = build(&standard_spec(false));
    let forms = multi
        .manifest
        .forms
        .iter()
        .map(|f| FormInput { name: f.name.clone(), cfrm: standard_cfrm(&multi, &f.name), stylesheet: None })
        .collect();
    let input = BundleInput {
        project: "fixture".into(),
        program_multi: Some(multi.wasm),
        program_single: Some(single.wasm),
        forms,
        stylesheet: None,
        assets: Vec::new(),
        player: PlayerInput { plain: variant(&player, "plain"), threaded: variant(&player, "threaded"), build_info: std::fs::read_to_string(player.join("player-build.json")).ok(), components: Vec::new() },
        options: BundleOptions { isolation, ..Default::default() },
    };
    let bundle = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let _ = std::fs::remove_dir_all(&out);
    for f in &bundle.files {
        let p = out.join(&f.path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, &f.bytes).unwrap();
    }
    println!("{}", bundle.report.text());
    if let Some(t) = tools {
        std::fs::create_dir_all(&t).unwrap();
        std::fs::write(t.join("harness.mjs"), harness::HARNESS).unwrap();
        std::fs::write(t.join("worker-shim.mjs"), harness::WORKER_SHIM).unwrap();
        println!("harness written to {}", t.display());
    }
}
