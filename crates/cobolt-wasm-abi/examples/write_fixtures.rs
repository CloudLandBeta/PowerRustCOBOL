// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `cargo run -p cobolt-wasm-abi --features fixture --example write_fixtures -- <dir>`
//!
//! Writes the standard fixture modules (`program.wasm` multi-memory, `program.single.wasm`
//! single-memory) and their `manifest.json` into `<dir>`, for the node and browser tests.

use std::path::PathBuf;

use cobolt_wasm_abi::fixture::{build, standard_spec};

fn main() {
    let dir: PathBuf = std::env::args().nth(1).expect("usage: write_fixtures <dir>").into();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    for (multi, file) in [(true, "program.wasm"), (false, "program.single.wasm")] {
        let f = build(&standard_spec(multi));
        std::fs::write(dir.join(file), &f.wasm).expect("write the module");
        let manifest = serde_json::to_string_pretty(&f.manifest).expect("manifest json");
        let mfile = if multi { "manifest.json" } else { "manifest.single.json" };
        std::fs::write(dir.join(mfile), manifest).expect("write the manifest");
        println!("{file}: {} bytes", f.wasm.len());
    }
}
