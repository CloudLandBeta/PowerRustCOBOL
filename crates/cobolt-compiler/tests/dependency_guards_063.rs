// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 063 AC8, AC30 and AC34 — what a built application must never link, read from the lock file.
//!
//! `Cargo.lock` lists every package any feature could bring in, so what it does *not* list is
//! certain never to be in a build: no AWS SDK (R18 — it defaults to `aws-lc-rs`, a C build), no
//! C-backed compression behind `zip` (AC34), and no path from the runtime a delivered application
//! links to the IDE or to `cobolt-agents`, which carry the model policy (R68).

use std::collections::{BTreeMap, BTreeSet};

/// `package name → the names it depends on`, from the workspace's `Cargo.lock`.
fn graph() -> BTreeMap<String, BTreeSet<String>> {
    let lock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock")).expect("Cargo.lock");
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let (mut name, mut in_deps) = (String::new(), false);
    for line in lock.lines() {
        if let Some(n) = line.strip_prefix("name = \"") {
            name = n.trim_end_matches('"').to_owned();
            out.entry(name.clone()).or_default();
            in_deps = false;
        } else if line.starts_with("dependencies = [") {
            in_deps = true;
        } else if in_deps {
            if line.starts_with(']') {
                in_deps = false;
            } else if let Some(dep) = line.trim().trim_end_matches(',').trim_matches('"').split(' ').next() {
                if !dep.is_empty() {
                    out.get_mut(&name).unwrap().insert(dep.to_owned());
                }
            }
        }
    }
    out
}

fn closure(graph: &BTreeMap<String, BTreeSet<String>>, roots: &[&str]) -> BTreeSet<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = roots.iter().map(|r| (*r).to_owned()).collect();
    while let Some(p) = stack.pop() {
        if seen.insert(p.clone()) {
            if let Some(deps) = graph.get(&p) {
                stack.extend(deps.iter().cloned());
            }
        }
    }
    seen
}

#[test]
fn no_aws_sdk_and_no_c_backed_compression_can_enter_a_build() {
    let g = graph();
    assert!(g.len() > 200, "the lock file parsed ({} packages)", g.len());
    let sdk: Vec<&String> = g.keys().filter(|n| n.starts_with("aws-")).collect();
    assert!(sdk.is_empty(), "AC8: no AWS SDK package may exist in the build graph; found {sdk:?}");
    for c_backed in ["bzip2", "bzip2-sys", "zstd", "zstd-sys", "aws-lc-rs", "aws-lc-sys"] {
        assert!(!g.contains_key(c_backed), "AC34: `{c_backed}` is a C build and may not be reached (not even through zip)");
    }
    println!("\n  063 AC8/AC34 — {} packages in the lock file: no aws-*, bzip2, zstd or aws-lc", g.len());
}

#[test]
fn what_a_delivered_application_links_reaches_neither_the_ide_nor_the_agent_mesh() {
    let g = graph();
    // The crates every built application links (compiler lib.rs SDK_CRATES, and the form host).
    let linked = closure(&g, &["cobolt-runtime", "cobolt-form-host", "cobolt-forms", "cobolt-kb", "cobolt-docs", "cobolt-mcp"]);
    for banned in ["cobolt-ide", "cobolt-agents"] {
        assert!(!linked.contains(banned), "AC30/R68: a built application's crates reach `{banned}`, which carries the model policy");
    }
    assert!(linked.contains("cobolt-runtime") && linked.len() > 100, "the closure is real ({} packages)", linked.len());
    println!("  063 AC30 — {} packages reachable from the runtime, form host, forms, kb, docs and mcp: no cobolt-ide, no cobolt-agents", linked.len());
}
