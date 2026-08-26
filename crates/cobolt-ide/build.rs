// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tell Cargo that the IDE binary depends on the repository's `docs/`.
//!
//! `docs_embed.rs` bakes that directory into the binary with `include_dir!`, a
//! macro that reads the filesystem while the crate is being compiled. Cargo
//! cannot see through a macro, so without this it has no reason to believe the
//! crate is stale when a document changes: editing, adding, renaming or
//! translating a file under `docs/` rebuilt nothing, and the IDE went on serving
//! whatever set of documents happened to be embedded the last time some *Rust*
//! file changed.
//!
//! That is why translations could land on disk and the Documentation viewer keep
//! showing English (operator, 2026-08-24).

use std::path::Path;

fn main() {
    // `include_dir!` is given `$CARGO_MANIFEST_DIR/../../docs`; watch the same
    // directory. Cargo re-runs the build when anything under it changes.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let docs = Path::new(&manifest).join("../../docs");
    println!("cargo:rerun-if-changed={}", docs.display());

    // Emitting any `rerun-if-changed` narrows Cargo's default, which is to
    // re-run on *any* change in the package. Name the build script itself so
    // editing this file still takes effect.
    println!("cargo:rerun-if-changed=build.rs");
}
