// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The fingerprint of the sources the committed routine blob was built from.
//!
//! `assets/rt.wasm` is a build product committed to the repository, as
//! `assets/knowledge/chunked.data` is. It is only trustworthy if it was built
//! from the sources next to it, so `assets/rt.sources.hash` records a digest of
//! them and a test fails when the two disagree — the same freshness discipline
//! as the System KB store. The digest is FNV-1a (64 bit): stable across Rust
//! versions and platforms, which `std`'s hashers are not.

use std::io;
use std::path::{Path, PathBuf};

/// The files the blob depends on, relative to the repository root.
pub fn source_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for dir in ["crates/cobolt-wasm-rt/src", "crates/cobolt-decimal/src"] {
        collect(&root.join(dir), &mut out)?;
    }
    out.push(root.join("crates/cobolt-wasm-rt/Cargo.toml"));
    out.push(root.join("crates/cobolt-decimal/Cargo.toml"));
    out.sort();
    Ok(out)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let p = entry?.path();
        if p.is_dir() {
            collect(&p, out)?;
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
    Ok(())
}

/// FNV-1a over `(relative path, contents)` of every source file, in path order.
pub fn sources_hash(root: &Path) -> io::Result<u64> {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    for f in source_files(root)? {
        let rel = f.strip_prefix(root).unwrap_or(&f);
        feed(rel.to_string_lossy().replace('\\', "/").as_bytes());
        feed(&[0]);
        // Normalise line endings so a checkout on Windows hashes the same.
        let text = std::fs::read(&f)?;
        let text: Vec<u8> = text.into_iter().filter(|b| *b != b'\r').collect();
        feed(&text);
        feed(&[0xFF]);
    }
    Ok(h)
}

/// The repository root, from this crate's manifest directory.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
