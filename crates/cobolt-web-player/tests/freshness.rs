// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The shipped player is the one its source produces** (WASM-016 R12, AC7).
//!
//! The built player is committed under `assets/web-player/` so that a clone of the repository can build a
//! Web bundle with no Rust toolchain. A committed binary goes stale silently, so `build_player` records
//! the fingerprint of everything the player is built from (`src/fingerprint.rs`) and the digest of every
//! file it wrote, and this test recomputes both:
//!
//! * the **source** moved on and the player was not rebuilt → fail, naming the command that rebuilds it;
//! * a shipped **file** is not the one the build wrote (edited, truncated, replaced) → fail;
//! * the player is rebuilt → pass.
//!
//! The same discipline as `prebuilt_chunked_kb_matches_the_published_documentation`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use cobolt_web_player::fingerprint::{file_digest, source_fingerprint};

const REBUILD: &str = "cargo run --manifest-path crates/cobolt-web-player/Cargo.toml --example build_player -- --out assets/web-player";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn shipped() -> PathBuf {
    root().join("assets/web-player")
}

fn record() -> serde_json::Value {
    let p = shipped().join("player-build.json");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|_| panic!("no shipped player ({} is missing): build it with\n  {REBUILD}", p.display()));
    serde_json::from_str(&text).expect("player-build.json is JSON")
}

#[test]
fn the_shipped_player_was_built_from_the_source_that_is_here() {
    let rec = record();
    let recorded = rec["source_hash"].as_str().expect("the build record has a source_hash");
    let now = source_fingerprint(&root());
    assert_eq!(
        recorded, now,
        "the player's source has changed since assets/web-player was built (recorded {recorded}, now {now}).\nRebuild and commit it:\n  {REBUILD}"
    );
}

#[test]
fn every_shipped_file_is_exactly_what_the_build_wrote_and_nothing_else_is_there() {
    let rec = record();
    let dir = shipped();
    let files = rec["files"].as_array().expect("the build record lists its files");
    let mut listed = BTreeSet::new();
    for f in files {
        let path = f["path"].as_str().unwrap();
        listed.insert(path.to_owned());
        let p = dir.join(path);
        assert!(p.is_file(), "{path} is in the build record but not shipped: rebuild with\n  {REBUILD}");
        assert_eq!(std::fs::metadata(&p).unwrap().len(), f["bytes"].as_u64().unwrap(), "{path}: another size than the build wrote");
        assert_eq!(file_digest(&p), f["sha256"].as_str().unwrap(), "{path}: not the file the build wrote (edited or replaced); rebuild with\n  {REBUILD}");
    }
    // The pieces a bundle is made of are all there: both variants of the player and the three parts.
    for must in [
        "plain/player.js",
        "plain/player_bg.wasm",
        "threaded/player.js",
        "threaded/player_bg.wasm",
        "parts/mermaid/mermaid.js",
        "parts/mermaid/mermaid_bg.wasm",
        "parts/pdf/pdf.js",
        "parts/pdf/pdf_bg.wasm",
        "parts/canvas/canvas.js",
        "parts/canvas/canvas_bg.wasm",
    ] {
        assert!(listed.contains(must), "{must} is not in the shipped player");
    }
    // Nothing is in the directory that the build did not write.
    let mut on_disk = BTreeSet::new();
    fn walk(base: &Path, dir: &Path, out: &mut BTreeSet<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                // The older players a project may pin have records of their own (checked below).
                if p.parent() == Some(base) && p.file_name().is_some_and(|n| n == "archive") {
                    continue;
                }
                walk(base, &p, out);
            } else if p.file_name().is_some_and(|n| n != "player-build.json") {
                out.insert(p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    walk(&dir, &dir, &mut on_disk);
    let extra: Vec<&String> = on_disk.difference(&listed).collect();
    assert!(extra.is_empty(), "files in assets/web-player that the build did not write: {extra:?}");
    println!("shipped player: {} files, source {}", listed.len(), rec["source_hash"].as_str().unwrap());
}

#[test]
fn the_build_record_names_the_tools_and_the_pinned_nightly() {
    let rec = record();
    let tools = &rec["tools"];
    assert!(tools["wasm_bindgen"].as_str().unwrap().contains("0.2.129"), "{tools}");
    assert_eq!(tools["nightly"], "nightly-2026-10-22");
    assert_eq!(rec["profile"]["opt_level"], "s", "the operator's ruling (WASM-017 Q8)");
    assert_eq!(rec["profile"]["wasm_opt"], "-Os");
    assert!(tools["rustc_nightly"].as_str().unwrap().contains("nightly"), "the threaded variant records the nightly it was built with");
    assert!(tools["wasm_opt"].as_str().unwrap().contains("wasm-opt"));
    assert!(tools["wasm_pack"].as_str().unwrap().contains("not used"));
    assert!(rec["variants"].as_array().unwrap().len() >= 2, "both variants");
}

#[test]
fn the_build_record_states_the_version_triple_and_the_host_imports() {
    // WASM-016 R11: a player on disk is identified without running it, and a bundle can tell whether it gives a
    // program module every import the module needs.
    let rec = record();
    let v = &rec["versions"];
    assert_eq!(v["abi"], cobolt_wasm_abi::ABI_VERSION, "{v}");
    assert_eq!(v["description"], cobolt_forms::description::DESCRIPTION_FORMAT, "{v}");
    assert_eq!(v["player"], env!("CARGO_PKG_VERSION"), "{v}");
    let imports: Vec<&str> = rec["host_imports"].as_array().expect("host_imports").iter().filter_map(|i| i.as_str()).collect();
    let want: Vec<String> = cobolt_wasm_abi::codes::PRODUCTION_IMPORTS.iter().map(|n| format!("{}.{n}", cobolt_wasm_abi::codes::IMPORT_MODULE)).collect();
    assert_eq!(imports, want);
}

/// Every older player kept for a pin (`assets/web-player/archive/<version>/`, WASM-016 R12) is exactly what its own
/// record lists, under the version its record states. An empty or absent archive passes: nothing is pinned yet.
#[test]
fn every_archived_player_is_what_its_record_says() {
    let archive = shipped().join("archive");
    let mut checked = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&archive) {
        for e in rd.flatten().filter(|e| e.path().is_dir()) {
            let dir = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(dir.join("player-build.json")).unwrap_or_else(|_| panic!("archive/{name} has no player-build.json"));
            let rec: serde_json::Value = serde_json::from_str(&text).expect("an archived record is JSON");
            assert_eq!(rec["versions"]["player"].as_str(), Some(name.as_str()), "archive/{name} holds the player its record calls {}", rec["versions"]["player"]);
            for f in rec["files"].as_array().expect("the record lists its files") {
                let path = f["path"].as_str().unwrap();
                let p = dir.join(path);
                assert!(p.is_file(), "archive/{name}/{path} is listed but missing");
                assert_eq!(file_digest(&p), f["sha256"].as_str().unwrap(), "archive/{name}/{path} is not the file its build wrote");
            }
            checked.push(name);
        }
    }
    println!("archived players checked: {} {checked:?}", checked.len());
}
