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

#[path = "../tests/support/gallery.rs"]
mod gallery;
#[path = "../tests/support/harness.rs"]
mod harness;

use std::path::{Path, PathBuf};

use cobolt_wasm_abi::fixture::{build, standard_cfrm, standard_spec};
use cobolt_web_bundle::{assemble_bundle, AssetInput, BundleInput, BundleOptions, FormInput, Isolation, MenuInput, PlayerInput, PlayerPartInput, PlayerVariantInput};

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

/// The parts `build_player` wrote under `<dir>/parts/<name>/`.
fn parts(out: &Path) -> Vec<PlayerPartInput> {
    let mut v = Vec::new();
    let Ok(rd) = std::fs::read_dir(out.join("parts")) else { return v };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let files = std::fs::read_dir(e.path()).unwrap().flatten().map(|f| (f.file_name().to_string_lossy().into_owned(), std::fs::read(f.path()).unwrap())).collect();
        v.push(PlayerPartInput { entry_js: format!("{name}.js"), entry_wasm: format!("{name}_bg.wasm"), name, files });
    }
    v
}

fn main() {
    let mut player: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut tools: Option<PathBuf> = None;
    let mut isolation = Isolation::Credentialless;
    let (mut gallery_on, mut shell_on, mut responsive) = (false, false, false);
    let (mut backend, mut recaptcha, mut auth_flow) = (None::<String>, String::new(), String::new());
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--backend" => backend = it.next(),
            "--recaptcha" => recaptcha = it.next().unwrap_or_default(),
            "--auth-flow" => auth_flow = it.next().unwrap_or_default(),
            "--gallery" => gallery_on = true,
            "--shell" => shell_on = true,
            "--responsive" => responsive = true,
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
    let main_name = multi.manifest.main_form().name.clone();
    let forms = multi
        .manifest
        .forms
        .iter()
        .map(|f| {
            let standard = standard_cfrm(&multi, &f.name);
            let cfrm = if f.name == main_name && gallery_on {
                gallery::gallery_cfrm(&standard, &gallery::Options { responsive })
            } else if f.name == main_name && shell_on {
                gallery::shell_cfrm(&standard)
            } else {
                standard
            };
            FormInput { name: f.name.clone(), cfrm, stylesheet: None }
        })
        .collect();
    let mut menus = Vec::new();
    let mut assets = Vec::new();
    if gallery_on {
        menus.push(MenuInput { form: main_name.clone(), control: "MNB-G".into(), yaml: gallery::menu_bar_yaml() });
        assets.push(AssetInput { path: "Assets/doc.md".into(), bytes: gallery::DOC_MD.as_bytes().to_vec() });
    }
    if shell_on {
        let child = multi.manifest.forms.iter().find(|f| f.name != main_name).map(|f| f.name.clone()).unwrap_or_default();
        menus.push(MenuInput { form: main_name.clone(), control: "SIDE-1".into(), yaml: gallery::side_menu_yaml(&child) });
    }
    let input = BundleInput {
        project: "fixture".into(),
        program_multi: Some(multi.wasm),
        program_single: Some(single.wasm),
        forms,
        stylesheet: None,
        menus,
        assets,
        player: PlayerInput { plain: variant(&player, "plain"), threaded: variant(&player, "threaded"), parts: parts(&player), build_info: std::fs::read_to_string(player.join("player-build.json")).ok(), components: Vec::new() },
        options: BundleOptions {
            isolation,
            backend_origins: backend.into_iter().collect(),
            recaptcha_site_key: recaptcha,
            auth_flow: if auth_flow.is_empty() { BundleOptions::default().auth_flow } else { auth_flow },
            ..Default::default()
        },
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
