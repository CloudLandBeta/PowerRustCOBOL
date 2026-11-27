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

/// The routines a module copies from the blob are in the order of their names, not of the blob's layout (spec 114 M5): a
/// routine added to the blob can make LLVM move an old one, and that moved every module's bytes before the linker ordered by
/// name. Prints the closure of every exported routine and of the whole set the generator calls.
#[test]
fn the_routines_a_module_copies_are_ordered_by_name_not_by_the_blobs_layout() {
    let blob = cobolt_wasm::link::Blob::load(include_bytes!("../assets/rt.wasm")).expect("blob");
    let mut roots: Vec<&str> = Vec::new();
    for name in ["rt_init", "rt_display", "rt_move", "rt_json_encode", "rt_json_decode", "rt_ws_fail", "rt_ws_emit", "rt_initialize"] {
        assert!(blob.export(name).is_some(), "the blob has no {name}");
        roots.push(name);
    }
    let closure = blob.closure(&roots).expect("closure");
    let keys: Vec<(String, u32)> = closure.funcs.iter().map(|&i| (blob.name_of(i), i)).collect();
    assert!(keys.windows(2).all(|w| w[0] <= w[1]), "the closure is not in the order of the routines' names");
    assert!(keys.len() > 50, "the closure of {} roots is only {} routines", roots.len(), keys.len());
    // The same roots in another order give the same closure.
    let mut reversed = roots.clone();
    reversed.reverse();
    let again = blob.closure(&reversed).expect("closure");
    assert_eq!(closure.funcs, again.funcs, "the closure depends on the order of the roots");
    println!(
        "rt closure of {} roots: {} routines in name order, the same for the roots reversed\nPASS 002 FAIL 000",
        roots.len(),
        keys.len()
    );
}
