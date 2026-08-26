// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Assemble the redistributable **platform SDK** — the Rust sources a built
//! application compiles against — so an installed IDE can build without a
//! source checkout.
//!
//! Building an executable is a real `cargo build`: the generated project
//! depends on `cobolt-ast` and `cobolt-runtime` by path. An IDE binary on its
//! own carries none of that, which is why Build fails on a machine that only
//! received `cobolt-ide.exe`.
//!
//! Run it from the source tree, naming where the install lives:
//!
//! ```sh
//! cargo run -p cobolt-compiler --example stage_sdk -- <install-dir>
//! ```
//!
//! The result is `<install-dir>/Cargo.toml` + `<install-dir>/crates/…` beside
//! the executable, which `resolve_workspace_root` finds with no configuration.
//! Passing `--sdk` puts it in `<install-dir>/sdk/` instead — also searched, and
//! tidier when the install folder holds other things.

use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut dest: Option<PathBuf> = None;
    let mut subfolder = false;

    for arg in args.by_ref() {
        match arg.as_str() {
            "--sdk" => subfolder = true,
            "-h" | "--help" => {
                eprintln!(
                    "usage: cargo run -p cobolt-compiler --example stage_sdk -- \
                     [--sdk] <install-dir>"
                );
                return;
            }
            other => dest = Some(PathBuf::from(other)),
        }
    }

    let Some(dest) = dest else {
        eprintln!("error: name the install directory to stage the SDK into.");
        eprintln!(
            "usage: cargo run -p cobolt-compiler --example stage_sdk -- \
             [--sdk] <install-dir>"
        );
        std::process::exit(2);
    };

    // The tree this example is compiled from is the tree we ship.
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .expect("crates/cobolt-compiler sits two levels below the workspace root");

    let target = if subfolder { dest.join("sdk") } else { dest };

    println!("Staging the platform SDK");
    println!("  from {}", workspace_root.display());
    println!("  into {}", target.display());
    println!("  crates: {}", cobolt_compiler::SDK_CRATES.join(", "));

    match cobolt_compiler::stage_sdk(&workspace_root, &target) {
        Ok(bytes) => {
            println!("Done — {:.1} MB staged.", bytes as f64 / (1024.0 * 1024.0));
            println!(
                "The IDE at {} can now build without a source checkout.",
                target.display()
            );
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
