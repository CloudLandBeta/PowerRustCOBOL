// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **What the shipped player was built from** (WASM-016 R12, AC7).
//!
//! The built player is committed under `assets/web-player/` so that the IDE works from a clone with no
//! extra step (the way `assets/knowledge/chunked.data` is). A committed binary can go stale without
//! anyone noticing: the source moves on and the wasm does not. So `build_player` records, in
//! `player-build.json`, a **fingerprint of everything the player is built from**, and the freshness test
//! (`tests/freshness.rs`) computes it again from the source tree and fails — naming the command that
//! rebuilds — when the two differ.
//!
//! The fingerprint is the SHA-256 of the sorted list of `(path, SHA-256 of the file)` of
//!
//! * this crate: `src/`, `parts/`, `vendor/`, `examples/build_player.rs`, `Cargo.toml`, `Cargo.lock`
//!   (the tests and the target directories are not inputs of the wasm, so editing a test never makes the
//!   player stale);
//! * the crates it links from this repository: `cobolt-forms` (`src/`, `Cargo.toml`) and
//!   `cobolt-wasm-abi` (`src/`, `Cargo.toml`), and `cobolt-ast`, which the model crate reads.
//!
//! Paths are written relative to the repository root with `/`, so the same tree gives the same value on
//! every machine. Native only: the browser build never computes it.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// The inputs, as paths relative to the repository root. A directory is walked; only the files whose
/// extension says they are source (`rs`, `toml`, `lock`) count.
pub const INPUTS: &[&str] = &[
    "crates/cobolt-web-player/src",
    "crates/cobolt-web-player/parts",
    "crates/cobolt-web-player/vendor",
    "crates/cobolt-web-player/examples/build_player.rs",
    "crates/cobolt-web-player/Cargo.toml",
    "crates/cobolt-web-player/Cargo.lock",
    "crates/cobolt-forms/src",
    "crates/cobolt-forms/Cargo.toml",
    "crates/cobolt-wasm-abi/src",
    "crates/cobolt-wasm-abi/Cargo.toml",
    "crates/cobolt-ast/src",
    "crates/cobolt-ast/Cargo.toml",
    // The remote grids' endpoint names and frame layouts (WASM-005 T16).
    "crates/cobolt-wire/src",
    "crates/cobolt-wire/Cargo.toml",
    // The Canvas 2D part is this crate's rasteriser (WASM-018).
    "crates/cobolt-graphics/src",
    "crates/cobolt-graphics/Cargo.toml",
];

fn is_source(p: &Path) -> bool {
    matches!(p.extension().and_then(|e| e.to_str()), Some("rs" | "toml" | "lock"))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_dir() {
            if name != "target" && name != "target-player" && !name.starts_with('.') {
                walk(&p, out);
            }
        } else if is_source(&p) {
            out.push(p);
        }
    }
}

/// Every input file under `root` (the repository root), sorted by relative path.
pub fn input_files(root: &Path) -> Vec<(String, PathBuf)> {
    let mut files = Vec::new();
    for input in INPUTS {
        let p = root.join(input);
        if p.is_dir() {
            walk(&p, &mut files);
        } else if p.is_file() {
            files.push(p);
        }
    }
    let mut named: Vec<(String, PathBuf)> = files
        .into_iter()
        .map(|p| (p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/"), p))
        .collect();
    named.sort_by(|a, b| a.0.cmp(&b.0));
    named.dedup_by(|a, b| a.0 == b.0);
    named
}

/// The fingerprint of the player's source as it is under `root`, in lower-case hex.
pub fn source_fingerprint(root: &Path) -> String {
    let mut all = Sha256::new();
    for (rel, path) in input_files(root) {
        let bytes = std::fs::read(&path).unwrap_or_default();
        all.update(rel.as_bytes());
        all.update([0]);
        all.update(Sha256::digest(&bytes));
    }
    hex(&all.finalize())
}

/// The SHA-256 of a file's bytes, in lower-case hex (what `player-build.json` records for each shipped file).
pub fn file_digest(path: &Path) -> String {
    hex(&Sha256::digest(std::fs::read(path).unwrap_or_default()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small tree with the layout the fingerprint reads.
    fn tree(dir: &Path) {
        for (rel, text) in [
            ("crates/cobolt-web-player/src/lib.rs", "pub fn a() {}"),
            ("crates/cobolt-web-player/src/dom/x.rs", "pub fn b() {}"),
            ("crates/cobolt-web-player/Cargo.toml", "[package]"),
            ("crates/cobolt-web-player/tests/t.rs", "#[test] fn t() {}"),
            ("crates/cobolt-web-player/target/debug/junk.rs", "junk"),
            ("crates/cobolt-forms/src/lib.rs", "pub fn f() {}"),
        ] {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
    }

    #[test]
    fn the_fingerprint_moves_with_any_input_and_with_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        tree(root);
        let base = source_fingerprint(root);
        assert_eq!(base.len(), 64);
        assert_eq!(base, source_fingerprint(root), "the same tree gives the same value");

        // A comment in a source file is an input: the wasm is not promised to be unchanged by it.
        let lib = root.join("crates/cobolt-web-player/src/lib.rs");
        std::fs::write(&lib, "pub fn a() {} // x").unwrap();
        let edited = source_fingerprint(root);
        assert_ne!(base, edited, "an edited source file changes it");
        std::fs::write(&lib, "pub fn a() {}").unwrap();
        assert_eq!(base, source_fingerprint(root), "and putting it back puts the value back");

        // A linked crate of the repository is an input too.
        std::fs::write(root.join("crates/cobolt-forms/src/lib.rs"), "pub fn f() { 1; }").unwrap();
        assert_ne!(base, source_fingerprint(root), "a change in cobolt-forms changes it");
        std::fs::write(root.join("crates/cobolt-forms/src/lib.rs"), "pub fn f() {}").unwrap();

        // A new file is an input; a test and a build directory are not.
        std::fs::write(root.join("crates/cobolt-web-player/tests/t.rs"), "#[test] fn t2() {}").unwrap();
        std::fs::write(root.join("crates/cobolt-web-player/target/debug/junk.rs"), "other junk").unwrap();
        assert_eq!(base, source_fingerprint(root), "tests and target directories are not inputs of the wasm");
        std::fs::write(root.join("crates/cobolt-web-player/src/new.rs"), "pub fn n() {}").unwrap();
        assert_ne!(base, source_fingerprint(root), "a new source file is");
    }

    #[test]
    fn the_listed_inputs_are_sorted_and_relative() {
        let dir = tempfile::tempdir().unwrap();
        tree(dir.path());
        let names: Vec<String> = input_files(dir.path()).into_iter().map(|(n, _)| n).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        assert!(names.iter().all(|n| !n.starts_with('/') && !n.contains("target")), "{names:?}");
        assert_eq!(names.len(), 4, "{names:?}");
    }
}
