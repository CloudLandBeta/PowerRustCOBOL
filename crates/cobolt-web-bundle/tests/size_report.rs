// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The size report (WASM-015 T13, R8b; WASM-016 R13, WASM-017 R16): the size of every part of a bundle — the
//! loader, the plain player, each player part, the program module of each memory variant — against the size
//! last recorded in `tests/size-ledger.json`. A part that grew by more than the ledger's threshold
//! (`threshold_percent`, a working number: Q-new-5) fails the test and names the part, the two sizes and the
//! growth. A part that shrank, or is new, is reported and does not fail. `PRC_RECORD_SIZES=1` rewrites the
//! ledger from the measurement, which is how a deliberate growth is accepted; the diff of the ledger is then
//! the review.
//!
//! The sizes are measured on the **test** player (built with the test hooks, as every bundle test uses): they
//! are a guard against growth, not the figures of the shipped player.

mod support;

use std::collections::BTreeMap;

use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildRequest};
use cobolt_web_bundle::{BundleOptions, PlayerInput};

const LEDGER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/size-ledger.json");

/// What grew past `threshold_percent` of its recorded size. Parts only in one side are not growth.
fn grown(recorded: &BTreeMap<String, u64>, current: &BTreeMap<String, u64>, threshold_percent: f64) -> Vec<String> {
    let mut out = Vec::new();
    for (part, &now) in current {
        let Some(&before) = recorded.get(part) else { continue };
        if before == 0 {
            continue;
        }
        let pct = (now as f64 - before as f64) * 100.0 / before as f64;
        if pct > threshold_percent {
            out.push(format!("{part}: {before} -> {now} bytes (+{pct:.1} %, limit +{threshold_percent} %)"));
        }
    }
    out
}

/// The name of a part without the content hash the bundle puts in it (`loader.91e860c6.js` -> `loader.js`,
/// `part-canvas-ddd9ce8f/` -> `part-canvas/`), so a part keeps its key when its bytes change.
fn unhashed(path: &str) -> String {
    let b: Vec<char> = path.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        let hex8 = i + 9 <= b.len() && (b[i] == '.' || b[i] == '-') && b[i + 1..i + 9].iter().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase());
        let ends = i + 9 == b.len() || (i + 9 < b.len() && matches!(b[i + 9], '.' | '/'));
        if hex8 && ends && b[i + 1..i + 9].iter().any(|c| c.is_ascii_digit()) {
            i += 9;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

fn map(pairs: &[(&str, u64)]) -> BTreeMap<String, u64> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

#[test]
fn a_planted_ten_percent_part_fails_and_a_four_percent_one_does_not() {
    let recorded = map(&[("player_bg.wasm", 1_000_000), ("loader.js", 50_000), ("old.js", 10)]);
    let current = map(&[("player_bg.wasm", 1_100_000), ("loader.js", 52_000), ("new.js", 999)]);
    let g = grown(&recorded, &current, 5.0);
    println!("size guard, planted growth: {g:?}");
    assert_eq!(g.len(), 1, "{g:?}");
    assert!(g[0].starts_with("player_bg.wasm: 1000000 -> 1100000"), "{g:?}");
    assert!(grown(&recorded, &recorded, 5.0).is_empty());
    println!("size guard: +10 % fails, +4 % passes, a new and a removed part do not count");
}

#[test]
fn hashes_are_taken_out_of_the_part_names() {
    assert_eq!(unhashed("loader.91e860c6.js"), "loader.js");
    assert_eq!(unhashed("player/part-canvas-ddd9ce8f/canvas_bg.wasm"), "player/part-canvas/canvas_bg.wasm");
    assert_eq!(unhashed("player/plain-8e3e56be/player_bg.wasm"), "player/plain/player_bg.wasm");
    assert_eq!(unhashed("gfx-worker.2b514713.js"), "gfx-worker.js");
    assert_eq!(unhashed("mermaid_bg.wasm"), "mermaid_bg.wasm");
}

#[test]
fn every_part_of_the_bundle_against_the_recorded_size() {
    let player_out = support::built_player();
    let plain = support::player_variant(&player_out, "plain");
    let out = support::assemble(PlayerInput { plain: Some(plain), parts: support::player_parts(&player_out), ..Default::default() }, BundleOptions::default());

    let mut current: BTreeMap<String, u64> = out.report.sizes.iter().filter(|s| s.path.ends_with(".wasm") || s.path.ends_with(".js")).map(|s| (unhashed(&s.path), s.bytes)).collect();
    // The program module of the scenario project, one per memory variant (named apart: they are the generator's output).
    let project = support::scenario_project("size");
    let req = WebBuildRequest::from_manifest(&project.path().join("Core.project.toml")).unwrap();
    let built = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("the scenario project does not build: {e}"));
    for e in std::fs::read_dir(&built.bundle_dir).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("program") && name.ends_with(".wasm") {
            current.insert(unhashed(&format!("scenario/{name}")), e.metadata().unwrap().len());
        }
    }
    assert!(current.keys().any(|k| k.contains("player_bg.wasm")) && current.keys().any(|k| k.starts_with("scenario/")), "parts measured: {:?}", current.keys().collect::<Vec<_>>());

    if std::env::var("PRC_RECORD_SIZES").is_ok_and(|v| v == "1") {
        let doc = serde_json::json!({ "threshold_percent": 5, "note": "sizes of the test player and the scenario project, in bytes; rewritten by PRC_RECORD_SIZES=1", "parts": current });
        std::fs::write(LEDGER, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
        println!("size report: recorded {} parts in tests/size-ledger.json", current.len());
        return;
    }
    let ledger: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(LEDGER).expect("tests/size-ledger.json (record it with PRC_RECORD_SIZES=1)")).unwrap();
    let threshold = ledger["threshold_percent"].as_f64().unwrap();
    let recorded: BTreeMap<String, u64> = ledger["parts"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_u64().unwrap())).collect();
    println!("size report ({} parts, limit +{threshold} % per part, engine: static):", current.len());
    for (part, now) in &current {
        match recorded.get(part) {
            Some(before) => println!("  {part:<40} {now:>10} bytes (recorded {before})"),
            None => println!("  {part:<40} {now:>10} bytes (new, not in the ledger)"),
        }
    }
    for part in recorded.keys().filter(|p| !current.contains_key(*p)) {
        println!("  {part:<40} gone from the bundle");
    }
    let failures = grown(&recorded, &current, threshold);
    assert!(failures.is_empty(), "parts grew past the limit; accept it with PRC_RECORD_SIZES=1 if deliberate:\n{}", failures.join("\n"));
}
