// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `cargo run -p cobolt-web-bundle --example serve -- <dir> [--port 8080] [--isolated]`
//!
//! Serves a Web bundle on 127.0.0.1 with the right content types, optionally cross-origin isolated
//! (`COOP: same-origin`, `COEP: require-corp`). The development server of WASM-002 R20.

use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::Isolation;

fn main() {
    let mut dir = None;
    let mut port = 0u16;
    let mut isolation = Isolation::Off;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--port" => port = it.next().unwrap().parse().expect("a port number"),
            "--isolated" => isolation = Isolation::RequireCorp,
            "--credentialless" => isolation = Isolation::Credentialless,
            other => dir = Some(other.to_owned()),
        }
    }
    let dir = dir.expect("serve <dir>");
    let server = serve(dir, &format!("127.0.0.1:{port}"), ServeOptions { isolation, ..Default::default() }).expect("the server starts");
    println!("serving on {} (isolation: {:?}); press Ctrl-C to stop", server.url(), isolation);
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}
