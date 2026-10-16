// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 063 AC8, AC30 and AC34, as amended by spec 103 R10 and R17 — what a built application must
//! never link, read from the lock file.
//!
//! `Cargo.lock` lists the packages the workspace's features can bring in, so what it does *not* list
//! is certain never to be in a build. Three rules are read from it:
//!
//! - **No AWS service client and no C-backed TLS (AC8, spec 103 R17).** Spec 063 R18 kept every
//!   `aws-*` package out because the AWS SDK defaults to `aws-lc-rs`, a C build. Spec 103 lets the
//!   AWS *credential* crates in — they sign requests and fetch sign-in tokens, and the product
//!   supplies their HTTP client over `ureq` + the operating system's TLS — so the rule is now an
//!   allow-list. `aws-sdk-s3` and its kind, `aws-smithy-http-client` (rustls) and `aws-lc-*` fail it.
//! - **Nothing the AWS crates reach is a second HTTP or TLS stack (spec 103 R10).** The lock
//!   file as a whole is not clean — it lists `ring`, `rustls` and `hyper` through a weak `reqwest`
//!   feature edge, though only `hyper` is compiled (for `google_maps`) — so the assertion is
//!   about what is reachable *from the AWS roots*, not about the file.
//! - **No C-backed compression behind `zip` (AC34)**, and **no path from the runtime a delivered
//!   application links to the IDE or to `cobolt-agents`**, which carry the model policy (AC30, R68).

use std::collections::{BTreeMap, BTreeSet};

type Graph = BTreeMap<String, BTreeSet<String>>;

/// `package name → the names it depends on`, from a `Cargo.lock`'s text.
fn graph_of(lock: &str) -> Graph {
    let mut out: Graph = BTreeMap::new();
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

/// The workspace's own lock file.
fn graph() -> Graph {
    let lock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock")).expect("Cargo.lock");
    graph_of(&lock)
}

fn closure(graph: &Graph, roots: &[&str]) -> BTreeSet<String> {
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

/// The `aws-*` packages spec 103 R17 lets into the lock: the credential and signing crates, the four
/// clients that fetch credentials (SSO, SSO-OIDC, STS, Signin), and the Smithy runtime they share.
const AWS_ALLOWED: &[&str] = &[
    "aws-config",
    "aws-credential-types",
    "aws-runtime",
    "aws-sigv4",
    "aws-types",
    "aws-sdk-sso",
    "aws-sdk-ssooidc",
    "aws-sdk-sts",
    "aws-sdk-signin",
];

/// `aws-smithy-http-client` is the Smithy crate that carries `hyper` and `rustls`; the product's own
/// `ureq` client replaces it (spec 103 R11), so it is the one `aws-smithy-*` package that is not allowed.
fn aws_package_allowed(name: &str) -> bool {
    AWS_ALLOWED.contains(&name) || (name.starts_with("aws-smithy-") && name != "aws-smithy-http-client")
}

/// What the `aws` feature's roots may never reach (spec 103 R10): a second HTTP stack, a TLS stack
/// other than the operating system's, or a C build.
const NEVER_FROM_AWS: &[&str] = &[
    "aws-lc-rs",
    "aws-lc-sys",
    "aws-smithy-http-client",
    "cc",
    "cmake",
    "h2",
    "hyper",
    "hyper-rustls",
    "hyper-util",
    "reqwest",
    "ring",
    "rustls",
    "rustls-webpki",
    "tokio-rustls",
    "tower",
];

/// The packages the `aws` feature of `cobolt-runtime` depends on directly.
const AWS_ROOTS: &[&str] = &["aws-config", "aws-sigv4", "aws-credential-types", "aws-smithy-runtime-api", "aws-smithy-types"];

/// C-backed compression, and the AWS C crypto, must not be anywhere in the lock (AC34, R17).
const C_BACKED: &[&str] = &["bzip2", "bzip2-sys", "zstd", "zstd-sys", "aws-lc-rs", "aws-lc-sys"];

/// Every way `graph` breaks the AWS rules, in words; empty when it keeps them.
fn aws_violations(g: &Graph) -> Vec<String> {
    let mut out = Vec::new();
    for n in g.keys().filter(|n| n.starts_with("aws-") && !aws_package_allowed(n)) {
        out.push(format!("`{n}` is an `aws-*` package outside the allow-list (R17)"));
    }
    for c in C_BACKED {
        if g.contains_key(*c) {
            out.push(format!("`{c}` is a C build and may not be in the lock (AC34, R17)"));
        }
    }
    let reached = closure(g, AWS_ROOTS);
    for banned in NEVER_FROM_AWS {
        if reached.contains(*banned) {
            out.push(format!("the AWS crates reach `{banned}` (R10)"));
        }
    }
    out
}

#[test]
fn only_the_allowed_aws_packages_and_no_c_backed_stack_can_enter_a_build() {
    let g = graph();
    assert!(g.len() > 200, "the lock file parsed ({} packages)", g.len());
    let found = aws_violations(&g);
    assert!(found.is_empty(), "spec 103 R10/R17: {found:#?}");
    let aws: Vec<&String> = g.keys().filter(|n| n.starts_with("aws-")).collect();
    let reached = closure(&g, AWS_ROOTS);
    println!(
        "\n  103 R10/R17 — {} packages in the lock file; {} are `aws-*`, all on the allow-list; \
         {} are reachable from the AWS roots and none is a second HTTP stack, a TLS stack or a C build; \
         no bzip2, zstd or aws-lc",
        g.len(),
        aws.len(),
        reached.len()
    );
}

/// The guard is worth only what it catches: feed it lock texts that break each rule.
#[test]
fn the_aws_guard_fails_when_a_rule_is_broken() {
    let real = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock")).expect("Cargo.lock");
    let clean = aws_violations(&graph_of(&real));
    assert!(clean.is_empty(), "the real lock file starts clean: {clean:?}");

    // 1. A service client (the SDK's default is a C build behind it).
    let with_s3 = format!("{real}\n[[package]]\nname = \"aws-sdk-s3\"\nversion = \"1.0.0\"\n");
    let v = aws_violations(&graph_of(&with_s3));
    assert!(v.iter().any(|m| m.contains("aws-sdk-s3")), "a service client is refused: {v:?}");

    // 2. The Smithy crate that carries hyper and rustls.
    let with_client = format!("{real}\n[[package]]\nname = \"aws-smithy-http-client\"\nversion = \"1.0.0\"\n");
    let v = aws_violations(&graph_of(&with_client));
    assert!(v.iter().any(|m| m.contains("aws-smithy-http-client")), "the rustls HTTP client is refused: {v:?}");

    // 3. A C crypto crate.
    let with_lc = format!("{real}\n[[package]]\nname = \"aws-lc-rs\"\nversion = \"1.0.0\"\n");
    let v = aws_violations(&graph_of(&with_lc));
    assert!(v.iter().any(|m| m.contains("aws-lc-rs")), "aws-lc-rs is refused: {v:?}");

    // 4. An edge from an AWS root to a second TLS stack: a second entry for `aws-config` that depends on `hyper-rustls`
    //    (the graph merges entries by name, so this adds the edge without depending on which version is pinned).
    let with_edge = format!("{real}\n[[package]]\nname = \"aws-config\"\nversion = \"0.0.0\"\ndependencies = [\n \"hyper-rustls\",\n]\n");
    let v = aws_violations(&graph_of(&with_edge));
    assert!(v.iter().any(|m| m.contains("hyper-rustls")), "an edge to a second TLS stack is refused: {v:?}");

    println!("  103 guard demonstrations — 4 broken lock files (service client, rustls HTTP client, aws-lc-rs, edge to hyper-rustls): 4 refused");
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
