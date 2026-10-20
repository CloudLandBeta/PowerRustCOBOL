// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The committed routine blob must have been built from the sources next to it.

use cobolt_wasm::rtsrc;

#[test]
fn the_committed_rt_blob_matches_its_sources() {
    let root = rtsrc::repo_root();
    let want = std::fs::read_to_string(root.join("crates/cobolt-wasm/assets/rt.sources.hash")).expect("rt.sources.hash");
    let have = format!("{:016x}", rtsrc::sources_hash(&root).expect("hash"));
    assert_eq!(
        want.trim(),
        have,
        "crates/cobolt-wasm/assets/rt.wasm is stale: run `cargo run -p cobolt-wasm --example build_rt` and commit the result"
    );
}

#[test]
fn the_blob_exports_every_routine_the_generator_calls() {
    let blob = cobolt_wasm::link::Blob::load(include_bytes!("../assets/rt.wasm")).expect("blob");
    // Every `call_rt("name")` in the lowering sources.
    let mut names = std::collections::BTreeSet::new();
    for src in [include_str!("../src/lower.rs"), include_str!("../src/lower_more.rs")] {
        let mut rest = src;
        while let Some(i) = rest.find("call_rt(\"") {
            rest = &rest[i + 9..];
            let end = rest.find('"').unwrap();
            names.insert(rest[..end].to_string());
        }
    }
    assert!(names.len() > 20, "found only {} routine names", names.len());
    for name in &names {
        assert!(blob.export(name).is_some(), "the blob has no {name}");
    }
}
