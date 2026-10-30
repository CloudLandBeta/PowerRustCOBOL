// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Compile the platform's libraries into a staged SDK's `libs/<os>/<arch>/`.
//!
//! `cargo run --release -p cobolt-compiler --example prebuild_libs -- <sdk folder>`
//!
//! The release workflow runs it after `stage_sdk`; the IDE copies the result
//! into its shared cache the first time it opens (`warmup::warm_platform_cache`).

fn main() {
    let Some(sdk) = std::env::args().nth(1) else {
        eprintln!("usage: prebuild_libs <staged sdk folder>");
        std::process::exit(2);
    };
    let sdk = std::path::PathBuf::from(sdk);
    match cobolt_compiler::warmup::produce_libs(&sdk, &mut |line| println!("{line}")) {
        Ok(bytes) => println!(
            "libraries written to {} ({:.0} MB)",
            cobolt_compiler::warmup::libs_dir(&sdk).display(),
            bytes as f64 / 1_048_576.0
        ),
        Err(e) => {
            eprintln!("prebuild_libs: {e}");
            std::process::exit(1);
        }
    }
}
