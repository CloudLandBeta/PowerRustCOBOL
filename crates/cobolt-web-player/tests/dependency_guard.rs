// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the player may never link (WASM-016 R1, the project's dependency rules), read from this
//! crate's **own** lock file and from the dependency closure `cargo` computes for the browser target.
//!
//! The main workspace's guard (`cobolt-compiler/tests/dependency_guards_063.rs`) reads only the main
//! lock; the player is its own Cargo workspace (see `Cargo.toml`), so the same rules are applied here:
//!
//! * **pure Rust only** — no C toolchain, no TLS stack, no system library behind the player: `ring`,
//!   `aws-lc-rs`/`aws-lc-sys`, `openssl`, `native-tls`, `cc`, `cmake`, `zstd-sys`, `bzip2-sys`,
//!   `lzma-sys`, `libsqlite3-sys` and the other `*-sys` crates that build C are not in the closure of the
//!   browser build (the C build helpers would have to run for `wasm32-unknown-unknown`);
//! * **no second HTTP or TLS stack** — the browser's own `WebSocket` is the only transport;
//! * **the glue and the tool agree** — `wasm-bindgen` in the lock is the version the build step
//!   requires of the installed CLI, exactly (WASM-017 R17);
//! * **permissive licences and MSRV 1.92** — every package of the browser closure (Leptos's included,
//!   WASM-016 T16) carries a licence on the permissive list and asks for no Rust above 1.92.

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

/// The licences the project accepts (the list of `cobolt-wasm`'s guard).
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
];

const MAX_MSRV: (u32, u32) = (1, 92);

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

const NEVER: &[&str] = &[
    "ring",
    "aws-lc-rs",
    "aws-lc-sys",
    "openssl",
    "openssl-sys",
    "openssl-probe",
    "native-tls",
    "rustls",
    "rustls-webpki",
    "hyper",
    "reqwest",
    "ureq",
    "tokio",
    "cc",
    "cmake",
    "pkg-config",
    "zstd-sys",
    "bzip2-sys",
    "lzma-sys",
    "libsqlite3-sys",
    "curl-sys",
    "libz-sys",
];

fn manifest() -> String {
    format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"))
}

/// The package names `cargo tree` lists for `target`.
fn closure(target: &str, extra: &[&str]) -> BTreeSet<String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .args(["tree", "--offline", "--manifest-path", &manifest(), "--target", target, "-e", "normal", "--prefix", "none"])
        .args(extra)
        .output()
        .expect("cargo tree runs");
    assert!(out.status.success(), "cargo tree failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.split_whitespace().next()).map(str::to_owned).collect()
}

#[test]
fn the_browser_build_links_no_c_toolchain_no_tls_stack_and_no_second_transport() {
    let wasm = closure("wasm32-unknown-unknown", &[]);
    let hits: Vec<&&str> = NEVER.iter().filter(|n| wasm.contains(**n)).collect();
    assert!(hits.is_empty(), "the browser build reaches {hits:?}");
    // The closure is what it should be: the model, the ABI crate and the browser crates, Leptos among them (WASM-016 T16).
    for must in ["cobolt-forms", "cobolt-wasm-abi", "wasm-bindgen", "web-sys", "js-sys", "serde_json", "leptos"] {
        assert!(wasm.contains(must), "`{must}` is missing from the browser closure ({} packages)", wasm.len());
    }
    println!("browser closure: {} packages, none of the {} banned names", wasm.len(), NEVER.len());
}

#[test]
fn the_threaded_variants_closure_is_the_same_rule_plus_rayon_and_nothing_else_native() {
    let threaded = closure("wasm32-unknown-unknown", &["--features", "threaded"]);
    let hits: Vec<&&str> = NEVER.iter().filter(|n| threaded.contains(**n)).collect();
    assert!(hits.is_empty(), "the threaded build reaches {hits:?}");
    assert!(threaded.contains("wasm-bindgen-rayon") && threaded.contains("rayon"), "the threaded variant has rayon");
    let plain = closure("wasm32-unknown-unknown", &[]);
    let extra: Vec<&String> = threaded.difference(&plain).collect();
    println!("the threaded variant adds {} packages: {extra:?}", extra.len());
    assert!(extra.len() < 25, "the threaded variant should add the pool and little else: {extra:?}");
}

#[test]
fn the_mermaid_part_is_pure_rust_too_and_the_player_itself_does_not_link_the_renderer() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .args(["tree", "--offline", "--manifest-path", &manifest(), "-p", "cobolt-web-part-mermaid", "--target", "wasm32-unknown-unknown", "-e", "normal", "--prefix", "none"])
        .output()
        .expect("cargo tree runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let part: BTreeSet<String> = String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.split_whitespace().next()).map(str::to_owned).collect();
    let hits: Vec<&&str> = NEVER.iter().filter(|n| part.contains(**n)).collect();
    assert!(hits.is_empty(), "the Mermaid part reaches {hits:?}");
    assert!(part.contains("mermaid-rs-renderer"), "the part has the renderer");
    // The player is the one that must not carry it: that is what loading it on demand is for (R13).
    let player = closure("wasm32-unknown-unknown", &[]);
    assert!(!player.contains("mermaid-rs-renderer"), "the renderer is linked into the player itself");
    // Nor is the regex engine it brings. Leptos's own configuration crate depends on `regex` too
    // (`leptos_config`, not optional in 0.8): that path is Leptos's and is the only one allowed.
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .args(["tree", "--offline", "--manifest-path", &manifest(), "--target", "wasm32-unknown-unknown", "-e", "normal", "--prefix", "none", "-i", "regex"])
        .output()
        .expect("cargo tree runs");
    let via: BTreeSet<String> = String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.split_whitespace().next()).map(str::to_owned).collect();
    let allowed: BTreeSet<String> = ["regex", "leptos_config", "leptos", "cobolt-web-player"].iter().map(|s| (*s).to_owned()).collect();
    let other: Vec<&String> = via.difference(&allowed).collect();
    assert!(other.is_empty(), "the regex engine reaches the player through {other:?}");
}

#[test]
fn the_glue_and_the_command_line_tool_are_pinned_to_the_same_wasm_bindgen() {
    let lock = std::fs::read_to_string(format!("{}/Cargo.lock", env!("CARGO_MANIFEST_DIR"))).expect("Cargo.lock");
    let mut version = None;
    let mut lines = lock.lines();
    while let Some(l) = lines.next() {
        if l == "name = \"wasm-bindgen\"" {
            version = lines.next().and_then(|v| v.strip_prefix("version = \"")).map(|v| v.trim_end_matches('"').to_owned());
            break;
        }
    }
    let version = version.expect("wasm-bindgen is in the lock");
    let build = std::fs::read_to_string(format!("{}/examples/build_player.rs", env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert!(build.contains(&format!("const WASM_BINDGEN: &str = \"{version}\";")), "build_player.rs requires another wasm-bindgen CLI than the lock has ({version})");
    let toml = std::fs::read_to_string(manifest()).unwrap();
    assert!(toml.contains(&format!("wasm-bindgen = \"={version}\"")), "Cargo.toml pins another version than the lock has ({version})");
}

/// `(licence, rust-version)` of every package `cargo metadata` knows for this workspace, by name.
fn licences_and_msrv() -> BTreeMap<String, Vec<(String, String, Option<String>)>> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo).args(["metadata", "--offline", "--format-version", "1", "--manifest-path", &manifest()]).output().expect("cargo metadata runs");
    assert!(out.status.success(), "cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("metadata is JSON");
    let mut map: BTreeMap<String, Vec<(String, String, Option<String>)>> = BTreeMap::new();
    for p in v["packages"].as_array().expect("packages") {
        let name = p["name"].as_str().unwrap_or("").to_owned();
        let entry = (p["version"].as_str().unwrap_or("").to_owned(), p["license"].as_str().unwrap_or("").to_owned(), p["rust_version"].as_str().map(str::to_owned));
        map.entry(name).or_default().push(entry);
    }
    map
}

#[test]
fn every_package_of_the_browser_build_is_permissive_and_builds_on_rust_1_92() {
    let meta = licences_and_msrv();
    let mut wasm = closure("wasm32-unknown-unknown", &[]);
    wasm.extend(closure("wasm32-unknown-unknown", &["--features", "threaded"]));
    let mut bad = Vec::new();
    for name in &wasm {
        for (version, licence, msrv) in meta.get(name).into_iter().flatten() {
            if !licence_ok(licence) {
                bad.push(format!("{name} {version}: licence \"{licence}\" is not on the permissive list"));
            }
            if let Some(m) = msrv.as_deref().filter(|m| !msrv_ok(m)) {
                bad.push(format!("{name} {version}: needs Rust {m}, above 1.{}", MAX_MSRV.1));
            }
        }
    }
    println!("browser closure: {} packages checked for licence and MSRV", wasm.len());
    assert!(bad.is_empty(), "the browser build breaks the dependency rules:\n  {}", bad.join("\n  "));
}

#[test]
fn the_licence_and_msrv_rules_read_spdx_and_versions_as_written() {
    assert!(licence_ok("MIT OR Apache-2.0") && licence_ok("Apache-2.0 WITH LLVM-exception") && licence_ok("MIT/Apache-2.0"));
    assert!(!licence_ok("GPL-3.0-only") && !licence_ok("") && !licence_ok("MIT AND GPL-2.0"));
    assert!(msrv_ok("1.88") && msrv_ok("1.92.0") && !msrv_ok("1.93") && !msrv_ok("2.0"));
}
