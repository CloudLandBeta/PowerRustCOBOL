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
    for name in [
        "rt_init", "rt_move", "rt_move_begin", "rt_move_to", "rt_rel", "rt_class", "rt_sign", "rt_begin", "rt_push",
        "rt_op", "rt_pick", "rt_drop", "rt_store", "rt_trunc", "rt_size_err", "rt_display", "rt_display_reg",
        "rt_display_end", "rt_flush", "rt_int", "rt_reg_int", "rt_rel_reg", "rt_goto_index", "rt_reg_goto_index",
    ] {
        assert!(blob.export(name).is_some(), "the blob has no {name}");
    }
}
