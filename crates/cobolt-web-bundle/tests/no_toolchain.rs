// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **No Rust toolchain on the developer's machine** (WASM-016 R1, R2, AC1). The test runs itself again as a child
//! process whose `PATH` holds `node` and the system folders and nothing else — the child first proves that `cargo`,
//! `rustc` and `rustup` cannot be started — and the child assembles a bundle from the **shipped** player
//! (`assets/web-player`, as committed; never `support::built_player()`, which builds one) and the program module in both
//! variants. The parent then lists the bundle, asserts exactly what it is made of, and runs the page headless under node.

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

use cobolt_web_bundle::{AssetInput, BundleOptions, Isolation, PlayerInput};
use support::harness::{HARNESS, WORKER_SHIM};

const CHILD: &str = "PR_NO_TOOLCHAIN_OUT";
const TEST: &str = "a_bundle_is_made_from_the_shipped_player_with_no_rust_toolchain_and_runs_under_node";

fn shipped() -> PathBuf {
    support::repo_root().join("assets/web-player")
}

/// What one file of the bundle is.
fn category(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or(path);
    if path.starts_with("player/plain-") {
        "player (plain)"
    } else if path.starts_with("player/threaded-") {
        "player (threaded)"
    } else if path.starts_with("player/part-") {
        "player part (loaded on first use)"
    } else if name.starts_with("program.single.") && name.ends_with(".wasm") {
        "program (single-memory)"
    } else if name.starts_with("program.") && name.ends_with(".wasm") {
        "program (multi-memory)"
    } else if path.starts_with("forms/") && name.ends_with(".json") {
        "form description"
    } else if path.starts_with("assets/") || path.starts_with("Assets/") {
        "asset"
    } else if path.ends_with(".css") && name.starts_with("stylesheet.") {
        "stylesheet"
    } else if name.starts_with("manifest.") && name.ends_with(".json") {
        "program manifest"
    } else if path.starts_with("hosting/") {
        "server configuration (hosting examples)"
    } else if matches!(name, "index.html") || name.starts_with("app.") || name.starts_with("loader.") || name == "config.json" || name.ends_with(".html") {
        "host page"
    } else if matches!(name, "worker.js" | "sbom.cdx.json" | "_headers" | "headers.txt" | "coi-register.js" | "coi-serviceworker.js") || name.starts_with("worker.") {
        "host page"
    } else {
        "other"
    }
}

fn child(out: &Path) {
    for tool in ["cargo", "rustc", "rustup"] {
        let r = Command::new(tool).arg("--version").output();
        assert!(r.is_err(), "{tool} can be started in the child; PATH = {:?}", std::env::var("PATH"));
    }
    let dir = shipped();
    let player = PlayerInput {
        plain: Some(support::player_variant(&dir, "plain")),
        threaded: Some(support::player_variant(&dir, "threaded")),
        parts: support::player_parts(&dir),
        build_info: std::fs::read_to_string(dir.join("player-build.json")).ok(),
        ..Default::default()
    };
    let mut input = support::input_with(player, BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() });
    input.stylesheet = Some(".pr-form { accent-color: rgb(0 120 212); }\n".into());
    input.assets = vec![AssetInput { path: "Assets/readme.txt".into(), bytes: b"an asset of the application".to_vec() }];
    let bundle = cobolt_web_bundle::assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    support::write_bundle(&bundle, out);
}

#[test]
fn a_bundle_is_made_from_the_shipped_player_with_no_rust_toolchain_and_runs_under_node() {
    if let Some(out) = std::env::var_os(CHILD) {
        return child(Path::new(&out));
    }
    let node = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|d| d.join(if cfg!(windows) { "node.exe" } else { "node" }))
        .find(|p| p.is_file())
        .expect("node is required for the end-to-end tests (WASM-015)");
    // A PATH with node and the system's own folders, and no folder of a Rust toolchain.
    let tools = tempfile::tempdir().unwrap();
    let bin = tools.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let node_copy = bin.join(node.file_name().unwrap());
    #[cfg(unix)]
    std::os::unix::fs::symlink(&node, &node_copy).unwrap();
    #[cfg(not(unix))]
    std::fs::copy(&node, &node_copy).unwrap();
    let system: Vec<PathBuf> = if cfg!(windows) { vec![PathBuf::from(r"C:\Windows\System32")] } else { vec!["/usr/bin".into(), "/bin".into()] };
    let path = std::env::join_paths(std::iter::once(bin.clone()).chain(system)).unwrap();

    let out = tempfile::tempdir().unwrap();
    let run = Command::new(std::env::current_exe().unwrap())
        .args([TEST, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD, out.path())
        .env("PATH", &path)
        .env_remove("CARGO")
        .env_remove("RUSTUP_TOOLCHAIN")
        .output()
        .unwrap();
    assert!(run.status.success(), "the child failed:\n{}\n{}", String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&run.stderr));

    // What the bundle is made of, exactly.
    let mut files = Vec::new();
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push(p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    walk(out.path(), out.path(), &mut files);
    files.sort();
    let mut by: std::collections::BTreeMap<&str, Vec<&str>> = Default::default();
    for f in &files {
        by.entry(category(f)).or_default().push(f);
    }
    println!("bundle built with PATH = {path:?} (no cargo, rustc or rustup): {} files", files.len());
    for (c, fs) in &by {
        println!("  {c}: {}", fs.join(", "));
    }
    assert!(!by.contains_key("other"), "files the bundle should not hold: {:?}", by.get("other"));
    for must in ["player (plain)", "player (threaded)", "program (multi-memory)", "program (single-memory)", "host page", "form description", "stylesheet", "asset"] {
        assert!(by.contains_key(must), "the bundle has no {must}: {files:?}");
    }
    let wasm: Vec<&String> = files.iter().filter(|f| f.ends_with(".wasm")).collect();
    let programs = wasm.iter().filter(|f| f.contains("program")).count();
    assert_eq!(programs, 2, "two program modules: {wasm:?}");
    // The player shipped is the player copied, byte for byte.
    let shipped_plain = std::fs::read(shipped().join("plain/player_bg.wasm")).unwrap();
    let copied = files.iter().find(|f| f.starts_with("player/plain-") && f.ends_with("player_bg.wasm")).expect("the plain player's module");
    assert_eq!(std::fs::read(out.path().join(copied)).unwrap(), shipped_plain, "the bundle carries the shipped player unchanged");

    // The page runs headless under node, with the same PATH. Node's harness hosts the plain player only (it has no
    // `wasm-bindgen-rayon` workers; the threaded variant is a browser's, proved in Chrome, T9): it runs a copy of this
    // bundle whose configuration lists no threaded player, as a page that is not cross-origin isolated would choose.
    let plain_only = tempfile::tempdir().unwrap();
    for f in &files {
        let to = plain_only.path().join(f);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(out.path().join(f), &to).unwrap();
    }
    let mut config: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(out.path().join("config.json")).unwrap()).unwrap();
    assert!(config["player"]["threaded"].is_object(), "the bundle offers the threaded player: {}", config["player"]);
    config["player"].as_object_mut().unwrap().remove("threaded");
    std::fs::write(plain_only.path().join("config.json"), config.to_string()).unwrap();

    let harness = tools.path().join("harness.mjs");
    let shim = tools.path().join("worker-shim.mjs");
    let crafted = tools.path().join("surprise.wasm");
    std::fs::write(&harness, HARNESS).unwrap();
    std::fs::write(&shim, WORKER_SHIM).unwrap();
    std::fs::write(&crafted, support::surprise_module()).unwrap();
    let run = Command::new(&node_copy).arg(&harness).arg(plain_only.path()).arg(&shim).arg(&crafted).env("PATH", &path).output().unwrap();
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(run.status.success(), "node exited with {}:\n{}\n{stdout}", run.status, String::from_utf8_lossy(&run.stderr));
    let report: serde_json::Value = serde_json::from_str(stdout.lines().last().unwrap_or("")).expect("the harness's report");
    let results = report["results"].as_object().unwrap();
    let failed: Vec<String> = results.iter().filter(|(_, r)| r["pass"] != true).map(|(n, r)| format!("{n}: {}", r["error"])).collect();
    println!("under node: {} scenarios passed, {} failed", results.len() - failed.len(), failed.len());
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
