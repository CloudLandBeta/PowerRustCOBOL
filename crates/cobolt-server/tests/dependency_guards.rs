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
/// The backend's HTTP front (WASM-003 R2, D1): only the `server` feature links it.
const LEPTOS_FRONT: &[&str] = &["leptos", "leptos_axum", "server_fn", "axum", "axum-core", "hyper", "hyper-util"];

#[test]
fn the_backend_is_headless_and_links_no_c_built_crypto() {
    let server = closure(&["-p", "cobolt-server"]);
    assert!(server.contains("tokio") && server.contains("native-tls") && server.contains("jsonwebtoken"), "the guard looks at the real closure");
    assert_eq!(present(&server, HEADLESS), Vec::<&str>::new(), "the backend must not link a window system or the IDE");
    assert_eq!(present(&server, NO_C_CRYPTO), Vec::<&str>::new(), "no C-built crypto and no second TLS stack");
    // The Leptos front is there, and brought nothing barred with it.
    assert_eq!(present(&server, LEPTOS_FRONT), LEPTOS_FRONT.to_vec(), "the HTTP front is Leptos's server integration");
    println!("server closure: {} crates; front {:?}; headless/barred found: {:?}", server.len(), present(&server, LEPTOS_FRONT), [present(&server, HEADLESS), present(&server, NO_C_CRYPTO)].concat());
    // The JSON Web Token verification is RustCrypto's.
    assert!(server.contains("rsa") && server.contains("p256") && server.contains("sha2"), "token signatures are verified with RustCrypto");
}

#[test]
fn the_server_runs_on_the_native_runtime_without_the_optional_bridges_it_does_not_name() {
    // The headless build names the runtime features its programs reach; with none
    // named, nothing optional comes with the runtime (spec 063 R18).
    let server = closure(&["-p", "cobolt-server"]);
    // `hyper` belongs to the HTTP front now (R2); the maps and AWS bridges are known by their own crates.
    for optional in ["rusqlite", "libsqlite3-sys", "postgres", "mysql", "google_maps", "reqwest", "aws-config"] {
        assert!(!server.contains(optional), "{optional} is in the backend's closure although no program asked for it");
    }
}

#[test]
fn the_compiler_reads_and_packs_a_backend_through_the_loader_alone() {
    let loader = closure(&["-p", "cobolt-server", "--no-default-features", "--features", "loader"]);
    let net = ["tokio", "native-tls", "tokio-native-tls", "tokio-tungstenite", "tungstenite", "jsonwebtoken", "httparse", "ureq", "cobolt-runtime"];
    assert_eq!(present(&loader, &net), Vec::<&str>::new(), "the loader has no sockets, no TLS, no token code, no interpreter");
    assert_eq!(present(&loader, LEPTOS_FRONT), Vec::<&str>::new(), "the loader links no HTTP front");
    println!("loader closure: {} crates; front/barred found: {:?}", loader.len(), [present(&loader, LEPTOS_FRONT), present(&loader, NO_C_CRYPTO)].concat());
    assert!(loader.contains("cobolt-wire") && loader.contains("cobolt-parser") && loader.contains("bincode"), "the loader still reads, checks and packs");

    let compiler = closure(&["-p", "cobolt-compiler"]);
    assert_eq!(present(&compiler, &["tokio-tungstenite", "tungstenite", "jsonwebtoken", "httparse"]), Vec::<&str>::new(), "building an application does not link the server's channel");
    assert_eq!(present(&compiler, NO_C_CRYPTO), Vec::<&str>::new());
    assert_eq!(present(&compiler, &LEPTOS_FRONT[..3]), Vec::<&str>::new(), "an application build links no Leptos");
}

#[test]
fn the_client_a_desktop_host_links_has_the_channel_and_nothing_of_the_server() {
    let client = closure(&["-p", "cobolt-server", "--no-default-features", "--features", "client"]);
    assert!(client.contains("tokio-tungstenite") && client.contains("native-tls"), "the client speaks WebSocket over the operating system's TLS");
    assert_eq!(present(&client, &["jsonwebtoken", "ureq"]), Vec::<&str>::new(), "the client does not carry the server's token or key-fetch code");
    // It links the runtime, with none of its optional bridges, for the process-wide transport factory.
    assert!(client.contains("cobolt-runtime"));
    for optional in ["rusqlite", "libsqlite3-sys", "postgres", "mysql", "google_maps", "hyper"] {
        assert!(!client.contains(optional), "{optional} is in the client's closure");
    }
    assert_eq!(present(&client, HEADLESS), Vec::<&str>::new(), "the client is headless too");
    assert_eq!(present(&client, NO_C_CRYPTO), Vec::<&str>::new());
    assert_eq!(present(&client, LEPTOS_FRONT), Vec::<&str>::new(), "the client links no HTTP front");
    println!("client closure: {} crates; front/barred found: {:?}", client.len(), [present(&client, LEPTOS_FRONT), present(&client, NO_C_CRYPTO)].concat());
}

#[test]
fn the_ide_runs_the_backend_as_a_child_and_links_none_of_the_server() {
    // WASM-003 T10 (operator, 2026-10-23): the IDE starts `rcrun backend run` as a
    // child process, so the server, its front and its token code stay out of it.
    let ide = closure(&["-p", "cobolt-ide"]);
    assert!(ide.contains("cobolt-compiler") && ide.contains("eframe"), "the guard looks at the real closure");
    let server_only = ["leptos", "leptos_axum", "server_fn", "axum", "axum-core", "jsonwebtoken", "tokio-tungstenite", "tungstenite"];
    assert_eq!(present(&ide, &server_only), Vec::<&str>::new(), "the IDE links no HTTP front, no WebSocket server and no token code");
    // `cobolt-server` reaches the IDE only through the compiler, as the loader.
    let cargo = option_env!("CARGO").unwrap_or("cargo");
    let out = Command::new(cargo)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["tree", "--offline", "-e", "features", "-i", "cobolt-server", "-p", "cobolt-ide", "--prefix", "none"])
        .output()
        .expect("cargo tree runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let features: BTreeSet<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.strip_prefix("cobolt-server feature \""))
        .filter_map(|l| l.split('"').next())
        .map(str::to_owned)
        .collect();
    println!("ide closure: {} crates; cobolt-server features {features:?}; server-only found: {:?}", ide.len(), present(&ide, &server_only));
    assert_eq!(features.into_iter().collect::<Vec<_>>(), ["loader"], "the IDE's cobolt-server is the loader alone");
}
