// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-013 T17 (R10): the bundle's SBOM comes from `cobolt_wire::sbom`, and moving the writer out of
//! `lib.rs` changed no byte. The oracle below is the document as `assemble_bundle` built it before the
//! move, rebuilt here from the bundle's own files.

mod support;

use cobolt_web_bundle::{hash, BundleOptions, PlayerInput, PlayerVariantInput};
use serde_json::{json, Value};

fn player() -> PlayerInput {
    PlayerInput {
        plain: Some(PlayerVariantInput {
            files: vec![("player.js".into(), b"export default async function init() {}\n".to_vec()), ("player_bg.wasm".into(), vec![0u8; 100])],
            entry_js: "player.js".into(),
            entry_wasm: "player_bg.wasm".into(),
        }),
        ..Default::default()
    }
}

#[test]
fn the_sbom_bytes_are_those_the_inline_writer_produced() {
    let out = support::assemble(player(), BundleOptions::default());
    let written = out.text("sbom.cdx.json").expect("the bundle carries its SBOM");

    // The inline construction of 2.0.269, verbatim, over every file except the SBOM itself.
    let mut components: Vec<Value> = out
        .files
        .iter()
        .filter(|f| f.path != "sbom.cdx.json")
        .map(|f| json!({"type": "file", "name": f.path, "hashes": [{"alg": "SHA-256", "content": hash::sha256_hex(&f.bytes)}]}))
        .collect();
    components.push(json!({"type": "data", "name": "Ubuntu Light", "licenses": [{"license": {"name": "Ubuntu-font-1.0"}}]}));
    let oracle = json!({
        "bomFormat": "CycloneDX", "specVersion": "1.5", "version": 1,
        "metadata": {"component": {"type": "application", "name": "fixture"}},
        "components": components,
    });
    let v: Value = serde_json::from_str(&written).expect("the SBOM parses");
    assert_eq!(v, oracle, "the document equals the inline construction");
    let got = v["components"].as_array().unwrap();
    println!("sbom: {} components, every file entry equal to the inline oracle, document parses as CycloneDX 1.5", got.len());
}
