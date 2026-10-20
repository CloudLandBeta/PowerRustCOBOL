// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the backend must never link (WASM-003 R2, R3, R7; spec 063 AC8 as amended
//! by 103): read from the dependency graph Cargo itself resolves, so the answer
//! is for the features a build really turns on, not for everything the lock file
//! could bring in.
//!
//! - the backend is **headless**: no `egui`, `eframe`, `winit` or `wgpu`, and none
//!   of the IDE's crates;
//! - **no C-built crypto and no second TLS stack**: no `ring`, no `aws-lc-*`, no
//!   `rustls`, no `openssl` crate — TLS is the operating system's, signatures are
//!   RustCrypto's;
//! - the compiler reads and packs a backend through the **loader alone**, which
//!   has no sockets, no TLS and no token code.

use std::collections::BTreeSet;
use std::process::Command;

/// The package names Cargo resolves for `args` (normal edges only).
fn closure(args: &[&str]) -> BTreeSet<String> {
    let cargo = option_env!("CARGO").unwrap_or("cargo");
    let out = Command::new(cargo)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["tree", "--offline", "-e", "normal", "--prefix", "none"])
        .args(args)
        .output()
        .expect("cargo tree runs");
    assert!(out.status.success(), "cargo tree {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split(' ').next())
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
        .collect()
}

fn present<'a>(set: &BTreeSet<String>, names: &'a [&'a str]) -> Vec<&'a str> {
    names.iter().copied().filter(|n| set.contains(*n)).collect()
}

const HEADLESS: &[&str] = &["egui", "eframe", "winit", "wgpu", "epaint", "cobolt-ide", "cobolt-form-host", "cobolt-agents", "cobolt-dap"];
const NO_C_CRYPTO: &[&str] = &["ring", "aws-lc-rs", "aws-lc-sys", "aws-lc-fips-sys", "rustls", "rustls-webpki", "openssl", "openssl-sys", "boring", "boring-sys"];

#[test]
fn the_backend_is_headless_and_links_no_c_built_crypto() {
    let server = closure(&["-p", "cobolt-server"]);
    assert!(server.contains("tokio") && server.contains("native-tls") && server.contains("jsonwebtoken"), "the guard looks at the real closure");
    assert_eq!(present(&server, HEADLESS), Vec::<&str>::new(), "the backend must not link a window system or the IDE");
    assert_eq!(present(&server, NO_C_CRYPTO), Vec::<&str>::new(), "no C-built crypto and no second TLS stack");
    // The JSON Web Token verification is RustCrypto's.
    assert!(server.contains("rsa") && server.contains("p256") && server.contains("sha2"), "token signatures are verified with RustCrypto");
}

#[test]
fn the_server_runs_on_the_native_runtime_without_the_optional_bridges_it_does_not_name() {
    // The headless build names the runtime features its programs reach; with none
    // named, nothing optional comes with the runtime (spec 063 R18).
    let server = closure(&["-p", "cobolt-server"]);
    for optional in ["rusqlite", "libsqlite3-sys", "postgres", "mysql", "google_maps", "hyper"] {
        assert!(!server.contains(optional), "{optional} is in the backend's closure although no program asked for it");
    }
}

#[test]
fn the_compiler_reads_and_packs_a_backend_through_the_loader_alone() {
    let loader = closure(&["-p", "cobolt-server", "--no-default-features", "--features", "loader"]);
    let net = ["tokio", "native-tls", "tokio-native-tls", "tokio-tungstenite", "tungstenite", "jsonwebtoken", "httparse", "ureq", "cobolt-runtime"];
    assert_eq!(present(&loader, &net), Vec::<&str>::new(), "the loader has no sockets, no TLS, no token code, no interpreter");
    assert!(loader.contains("cobolt-wire") && loader.contains("cobolt-parser") && loader.contains("bincode"), "the loader still reads, checks and packs");

    let compiler = closure(&["-p", "cobolt-compiler"]);
    assert_eq!(present(&compiler, &["tokio-tungstenite", "tungstenite", "jsonwebtoken", "httparse"]), Vec::<&str>::new(), "building an application does not link the server's channel");
    assert_eq!(present(&compiler, NO_C_CRYPTO), Vec::<&str>::new());
}

#[test]
fn the_client_a_desktop_host_links_has_the_channel_and_nothing_of_the_server() {
    let client = closure(&["-p", "cobolt-server", "--no-default-features", "--features", "client"]);
    assert!(client.contains("tokio-tungstenite") && client.contains("native-tls"), "the client speaks WebSocket over the operating system's TLS");
    assert_eq!(present(&client, &["jsonwebtoken", "ureq", "cobolt-runtime", "cobolt-parser"]), Vec::<&str>::new(), "the client does not carry the server");
    assert_eq!(present(&client, NO_C_CRYPTO), Vec::<&str>::new());
}
