// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the mobile shell may link (WASM-012 T8, Risks 2–4; D-Pin), read from
//! this crate's **own** lock file and from the closures `cargo` computes for
//! the two phone targets with every device feature on:
//!
//! * **no TLS stack and no C crypto** — `ring`, `aws-lc-rs`/`aws-lc-sys`,
//!   `openssl`, `native-tls`, `rustls` are not in the Android or the iOS
//!   closure (the shell talks to the backend through the web view, never
//!   through a Rust HTTP client of its own);
//! * **permissive licences and MSRV 1.92** — every package of both closures
//!   carries a licence on the project's list and asks for no Rust above 1.92;
//! * **the version set is the pinned one** — `tauri`, `tauri-build` and the
//!   plugins in the lock are exactly the versions the plan's V0 record chose.

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

const ALLOWED_LICENCES: &[&str] = &[
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "MPL-2.0",
    "CC0-1.0",
    "0BSD",
    "BSL-1.0",
    "Unlicense",
    "LLVM-exception",
    "CDLA-Permissive-2.0",
];

const MAX_MSRV: (u32, u32) = (1, 92);

const NEVER: &[&str] = &["ring", "aws-lc-rs", "aws-lc-sys", "openssl", "openssl-sys", "native-tls", "rustls", "rustls-webpki", "hyper-rustls", "hyper-tls"];

/// The pinned version set (D-Pin, plan §9).
const PINNED: &[(&str, &str)] = &[
    ("tauri", "2.12.5"),
    ("tauri-build", "2.7.4"),
    ("tauri-plugin-opener", "2.7.2"),
    ("tauri-plugin-deep-link", "2.6.3"),
    ("keyring-core", "1.0.1"),
    ("android-native-keyring-store", "1.0.0"),
    ("apple-native-keyring-store", "1.0.2"),
];

const TARGETS: &[&str] = &["aarch64-linux-android", "aarch64-apple-ios"];
const ALL_FEATURES: &str = "secure-storage,open-address,deep-link";

fn licence_ok(expr: &str) -> bool {
    if expr.trim().is_empty() {
        return false;
    }
    let normalised = expr.replace(['(', ')'], " ").replace('/', " OR ");
    normalised.split(" OR ").any(|alt| {
        alt.split(" AND ").all(|term| {
            term.split(" WITH ").all(|t| {
                let t = t.trim().trim_end_matches('+');
                t.is_empty() || ALLOWED_LICENCES.contains(&t)
            })
        })
    })
}

fn msrv_ok(v: &str) -> bool {
    let mut it = v.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0)) <= MAX_MSRV
}

fn manifest() -> String {
    format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"))
}

fn cargo() -> Command {
    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
}

/// The package names of the normal-dependency closure for `target`, every feature on.
fn closure(target: &str) -> BTreeSet<String> {
    let out = cargo()
        .args(["tree", "--locked", "--manifest-path", &manifest(), "--target", target, "-e", "normal", "--prefix", "none", "--features", ALL_FEATURES])
        .output()
        .expect("cargo tree runs");
    assert!(out.status.success(), "cargo tree failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.split_whitespace().next()).map(str::to_owned).collect()
}

#[test]
fn the_phone_closures_carry_no_tls_stack_and_no_c_crypto() {
    let mut total = 0;
    for target in TARGETS {
        let c = closure(target);
        let hits: Vec<&&str> = NEVER.iter().filter(|n| c.contains(**n)).collect();
        assert!(hits.is_empty(), "{target}: the shell reaches {hits:?}");
        for must in ["tauri", "tauri-plugin-opener", "tauri-plugin-deep-link", "keyring-core"] {
            assert!(c.contains(must), "{target}: `{must}` is missing from the closure ({} packages)", c.len());
        }
        let store = if target.contains("android") { "android-native-keyring-store" } else { "apple-native-keyring-store" };
        assert!(c.contains(store), "{target}: the platform's secure store `{store}` is in the closure");
        println!("{target}: {} packages, none of the {} banned names, the platform's own secure store linked", c.len(), NEVER.len());
        total += c.len();
    }
    assert!(total > 0);
}

#[test]
fn the_lock_pins_the_chosen_versions_and_holds_no_banned_package() {
    let text = std::fs::read_to_string(format!("{}/Cargo.lock", env!("CARGO_MANIFEST_DIR"))).expect("the shell has its own Cargo.lock");
    let lock: toml::Table = text.parse().expect("the lock is TOML");
    let mut versions: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in lock["package"].as_array().expect("[[package]]") {
        let name = p["name"].as_str().unwrap_or_default().to_owned();
        let version = p["version"].as_str().unwrap_or_default().to_owned();
        versions.entry(name).or_default().push(version);
    }
    for (name, version) in PINNED {
        assert_eq!(versions.get(*name).map(Vec::as_slice), Some(&[version.to_string()][..]), "{name} is pinned at {version}");
    }
    let banned: Vec<&&str> = NEVER.iter().filter(|n| versions.contains_key(**n)).collect();
    assert!(banned.is_empty(), "the lock holds {banned:?}");
    println!("lock: {} packages, the {} pinned versions exact, none of the {} banned names", versions.len(), PINNED.len(), NEVER.len());
}

#[test]
fn every_package_of_both_closures_has_a_permissive_licence_and_msrv_at_most_1_92() {
    let mut checked = 0;
    let mut problems = Vec::new();
    for target in TARGETS {
        let out = cargo()
            .args(["metadata", "--locked", "--format-version", "1", "--manifest-path", &manifest(), "--filter-platform", target, "--features", ALL_FEATURES])
            .output()
            .expect("cargo metadata runs");
        assert!(out.status.success(), "cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr));
        let meta: serde_json::Value = serde_json::from_slice(&out.stdout).expect("metadata is JSON");
        let ids: BTreeSet<&str> = meta["resolve"]["nodes"].as_array().into_iter().flatten().filter_map(|n| n["id"].as_str()).collect();
        for p in meta["packages"].as_array().into_iter().flatten() {
            if !ids.contains(p["id"].as_str().unwrap_or_default()) {
                continue;
            }
            let name = p["name"].as_str().unwrap_or_default();
            if name == "cobolt-mobile-shell" {
                continue;
            }
            let licence = p["license"].as_str().unwrap_or_default();
            if !licence_ok(licence) {
                problems.push(format!("{target}: {name} {}: licence '{licence}'", p["version"]));
            }
            if let Some(v) = p["rust_version"].as_str() {
                if !msrv_ok(v) {
                    problems.push(format!("{target}: {name} {}: rust-version {v} > 1.92", p["version"]));
                }
            }
            checked += 1;
        }
    }
    assert!(problems.is_empty(), "{} problem(s):\n{}", problems.len(), problems.join("\n"));
    println!("licences and MSRV: {checked} package entries over {} targets, every one permissive and at most 1.92", TARGETS.len());
}
