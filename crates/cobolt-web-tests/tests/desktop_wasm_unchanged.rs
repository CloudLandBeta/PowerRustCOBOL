// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **DESKTOP-WASM-001 T1, AC16:** the Web module of `examples/PowerAnalyticsWeb` is byte-identical
//! before and after the Desktop WebAssembly work. The feature changes the generator, the ABI manifest
//! and the form descriptions for the Desktop kind only; a Web project's module must not move.
//!
//! A scratch copy of the example is regenerated and built with the project tools, as the IDE builds it.
//! The sha256 of each multi-memory and single-memory module and of the manifest is compared with
//! `fixtures/power-analytics-web.sha256`, recorded before any change of the feature (2.0.484).
//!
//! A change that moves these bytes on purpose names itself in its CHANGELOG entry and re-records the
//! pin with `UPDATE_PIN=1`. Nothing else may.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use cobolt_project_tools::web;
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::{HeadlessHost, ProjectTools};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// What the Build writes and the pin covers: the name the fixture gives it, and how its file in
/// `dist/web/` is named (`<stem>.<8 hex>.<ext>`).
const ARTEFACTS: &[(&str, &str, &str)] = &[
    ("program.wasm", "program", "wasm"),
    ("program.single.wasm", "program.single", "wasm"),
    ("manifest.json", "manifest", "json"),
];

fn example_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerAnalyticsWeb")
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/power-analytics-web.sha256")
}

/// Copies the example, leaving out what a Build writes, so the copy builds from its sources only.
fn copy_tree(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap().flatten() {
        let name = entry.file_name();
        if matches!(name.to_str(), Some("dist" | "temp" | "debug" | "target" | "bin")) {
            continue;
        }
        let (from, to) = (entry.path(), dst.join(&name));
        if from.is_dir() {
            copy_tree(&from, &to);
        } else {
            std::fs::copy(&from, &to).unwrap();
        }
    }
}

/// The file `<stem>.<8 hex>.<ext>` in `dir`.
fn hashed_file(dir: &Path, stem: &str, ext: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        let Some(name) = p.file_name().and_then(|n| n.to_str()) else { return false };
        let Some(mid) = name.strip_prefix(stem).and_then(|r| r.strip_prefix('.')).and_then(|r| r.strip_suffix(ext)).and_then(|r| r.strip_suffix('.')) else {
            return false;
        };
        mid.len() == 8 && mid.bytes().all(|c| c.is_ascii_hexdigit())
    })
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// `name sha256` lines; `#` starts a comment.
fn read_pins(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once(char::is_whitespace).map(|(n, h)| (n.to_string(), h.trim().to_string())))
        .collect()
}

#[test]
fn the_power_analytics_web_module_is_unchanged() {
    let base = std::env::temp_dir().join(format!("prc-desktop-wasm-ac16-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let folder = base.join("PowerAnalyticsWeb");
    copy_tree(&example_dir(), &folder);

    let mut tools = ProjectTools::new(HeadlessHost::new(base.clone(), "test"));
    let manifest = folder.join("PowerAnalyticsWeb.project.toml");
    tools.call("open_project", &json!({"path": manifest.to_string_lossy()})).unwrap();
    web::link(Arc::new(GeneratorBackend));
    let started = Instant::now();
    tools.call("regenerate", &json!({})).unwrap();
    let built: Value = tools.call("build", &json!({})).unwrap();
    let build_ms = started.elapsed().as_millis();
    assert_eq!(built["status"], "built", "the Web Build succeeds: {built}");
    let dist = folder.join(built["bundle"].as_str().expect("the Build names its bundle"));

    let mut produced = Vec::new();
    for (name, stem, ext) in ARTEFACTS {
        let path = hashed_file(&dist, stem, ext).unwrap_or_else(|| panic!("the Build wrote no {stem}.<hash>.{ext} in {}", dist.display()));
        let bytes = std::fs::read(&path).unwrap();
        produced.push((name.to_string(), sha256(&bytes), bytes.len()));
    }

    if std::env::var("UPDATE_PIN").as_deref() == Ok("1") {
        let mut text = String::from("# DESKTOP-WASM-001 AC16: sha256 of the Web Build of examples/PowerAnalyticsWeb.\n# Re-record only with a change that names this in its CHANGELOG entry (UPDATE_PIN=1).\n");
        for (name, hash, _) in &produced {
            text.push_str(&format!("{name} {hash}\n"));
        }
        std::fs::create_dir_all(fixture().parent().unwrap()).unwrap();
        std::fs::write(fixture(), text).unwrap();
    }
    let pins = read_pins(&std::fs::read_to_string(fixture()).expect("the pin file exists (record it with UPDATE_PIN=1)"));

    println!("== the_power_analytics_web_module_is_unchanged (DESKTOP-WASM-001 T1, AC16) ==");
    println!("project: examples/PowerAnalyticsWeb (a scratch copy), regenerated and built in {build_ms} ms");
    let mut matched = 0;
    let mut differing = Vec::new();
    for (name, hash, len) in &produced {
        let pinned = pins.iter().find(|(n, _)| n == name).map(|(_, h)| h.as_str());
        let same = pinned == Some(hash.as_str());
        println!("  {name}: {len} bytes, sha256 {hash} — {}", if same { "matches the pin" } else { "DIFFERS from the pin" });
        if same {
            matched += 1;
        } else {
            differing.push(format!("{name}: built {hash}, pinned {}", pinned.unwrap_or("nothing")));
        }
    }
    println!("artefacts: {}; matching the pin {matched}; differing {}", produced.len(), differing.len());
    println!("result: {}", if differing.is_empty() { "PASS" } else { "FAIL" });
    let _ = std::fs::remove_dir_all(&base);
    assert!(differing.is_empty(), "the Web module of PowerAnalyticsWeb moved (AC16):\n{}", differing.join("\n"));
}
