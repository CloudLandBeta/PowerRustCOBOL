// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `main` of the built backend binary (WASM-003 R2, R20).
//!
//! ```text
//! <backend> [--config backend.toml] [--check] [--describe] [--version]
//! ```
//!
//! `--check` validates the configuration and prints what it would do;
//! `--describe` prints the interface description; with neither, the backend
//! starts and runs until it is interrupted. A built binary is a **release**
//! run: any `unsafe_*` setting stops it (WASM-013 R11).

use std::path::PathBuf;
use std::sync::Arc;

use crate::bundle::Bundle;
use crate::config::{RunMode, ServerConfig};
use crate::log::StdoutSink;
use crate::serve::Options;

/// Run the packed backend with the process arguments. Returns the exit code.
pub fn main(bundle: &[u8], args: Vec<String>) -> i32 {
    // Warnings and errors of the runtime go to standard error; `COBOLT_LOG` widens it.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_env("COBOLT_LOG").add_directive(tracing::Level::WARN.into()))
        .with_target(false)
        .with_writer(std::io::stderr)
        .try_init();
    let mut config_path = PathBuf::from("backend.toml");
    let mut check = false;
    let mut describe = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--config" => match it.next() {
                Some(p) => config_path = PathBuf::from(p),
                None => {
                    eprintln!("--config needs a file");
                    return 2;
                }
            },
            "--check" => check = true,
            "--describe" => describe = true,
            "--version" => {
                println!("backend (cobolt-server {})", env!("CARGO_PKG_VERSION"));
                return 0;
            }
            "-h" | "--help" => {
                println!("usage: backend [--config backend.toml] [--check] [--describe] [--version]");
                return 0;
            }
            other => {
                eprintln!("unknown argument '{other}'");
                return 2;
            }
        }
    }

    let backend = match Bundle::from_bytes(bundle).and_then(|b| b.into_backend().map_err(|e| e.join("\n"))) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    if describe {
        println!("{}", backend.interface.to_json());
        return 0;
    }
    let cfg = match ServerConfig::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("The backend needs its configuration: address, TLS identity, the issuer of the tokens it accepts. See backend.toml in the project's backend folder.");
            return 2;
        }
    };
    let base = config_path.parent().map(|p| p.to_path_buf()).filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| PathBuf::from("."));
    let resolved = match cfg.resolve(RunMode::Release, &base) {
        Ok(r) => r,
        Err(errs) => {
            for e in errs {
                eprintln!("error: {e}");
            }
            return 2;
        }
    };
    if check {
        println!("configuration ok: contract {} on {} ({} exported program(s))", backend.interface.contract, resolved.listen, backend.interface.programs.len());
        return 0;
    }
    let handle = match crate::serve::start(resolved, backend, Options { sink: Some(Arc::new(StdoutSink)), fetcher: None }) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };
    eprintln!("backend listening on {}", handle.addr);
    wait_for_interrupt();
    eprintln!("stopping");
    handle.stop();
    0
}

/// Block until the process is interrupted (Ctrl-C).
pub fn wait_for_interrupt() {
    match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt.block_on(async {
            let _ = tokio::signal::ctrl_c().await;
        }),
        Err(_) => loop {
            std::thread::park();
        },
    }
}
