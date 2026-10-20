// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Rebuild `crates/cobolt-wasm/assets/rt.wasm`, the routine blob linked into
//! every generated module, and the hash of the sources it was built from.
//!
//! ```text
//! cargo run -p cobolt-wasm --example build_rt
//! ```
//!
//! Needs the `wasm32-unknown-unknown` target (`rustup target add
//! wasm32-unknown-unknown`). The blob is committed; this is run when
//! `cobolt-wasm-rt` or `cobolt-decimal` changes, and a test fails until it is.

use std::process::Command;

fn main() {
    let root = cobolt_wasm::rtsrc::repo_root();
    let target_dir = root.join("target/rt-wasm");
    // A 16 KiB stack at the bottom of memory, data above it: the generator
    // places its own data after `__data_end`. Panics are aborts; symbol names
    // stay in the blob (the linker uses them to stub the panic machinery).
    let rustflags = "-C link-arg=-zstack-size=16384 -C link-arg=--no-entry";
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .current_dir(&root)
        .env("RUSTFLAGS", rustflags)
        .env("CARGO_TARGET_DIR", &target_dir)
        .args([
            "build",
            "-p",
            "cobolt-wasm-rt",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--lib",
            "--config",
            "profile.release.opt-level=\"s\"",
            "--config",
            "profile.release.lto=true",
            "--config",
            "profile.release.codegen-units=1",
            "--config",
            "profile.release.panic=\"abort\"",
        ])
        .status()
        .expect("cannot run cargo");
    if !status.success() {
        eprintln!("building cobolt-wasm-rt for wasm32 failed");
        std::process::exit(1);
    }
    let built = target_dir.join("wasm32-unknown-unknown/release/cobolt_wasm_rt.wasm");
    let dest = root.join("crates/cobolt-wasm/assets/rt.wasm");
    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
    std::fs::copy(&built, &dest).expect("cannot copy the built blob");
    let hash = cobolt_wasm::rtsrc::sources_hash(&root).expect("cannot hash the sources");
    std::fs::write(root.join("crates/cobolt-wasm/assets/rt.sources.hash"), format!("{hash:016x}\n")).unwrap();
    let len = std::fs::metadata(&dest).unwrap().len();
    println!("rt.wasm: {len} bytes, sources hash {hash:016x}");
}
