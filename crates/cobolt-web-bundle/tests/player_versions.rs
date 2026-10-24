// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A player and a program module that disagree are a Build error (WASM-016 R11, AC6 first half): the
//! player's record says which host imports it gives (`host_imports` in `player-build.json`), and a bundle
//! whose module needs one it lacks is refused, naming the import and the player.

mod support;

use cobolt_wasm_abi::codes::{IMPORT_MODULE, PRODUCTION_IMPORTS};
use cobolt_web_bundle::{assemble_bundle, BundleOptions, PlayerInput, PlayerVariantInput};

fn player_giving(imports: &[&str]) -> PlayerInput {
    let files = vec![("player.js".to_owned(), b"export default 0;".to_vec()), ("player_bg.wasm".to_owned(), b"\0asm\x01\0\0\0".to_vec())];
    let host_imports: Vec<String> = imports.iter().map(|n| format!("{IMPORT_MODULE}.{n}")).collect();
    let record = serde_json::json!({"versions": {"abi": 1, "description": 1, "player": "1.9.0"}, "host_imports": host_imports});
    PlayerInput {
        plain: Some(PlayerVariantInput { files, entry_js: "player.js".into(), entry_wasm: "player_bg.wasm".into() }),
        build_info: Some(record.to_string()),
        ..Default::default()
    }
}

#[test]
fn a_player_that_lacks_an_import_the_module_needs_fails_the_build() {
    let lacking: Vec<&str> = PRODUCTION_IMPORTS.iter().copied().filter(|n| *n != "remote").collect();
    let err = assemble_bundle(&support::input_with(player_giving(&lacking), BundleOptions::default())).expect_err("the bundle is refused");
    let text = err.to_string();
    println!("refused: {text}");
    assert!(text.contains(&format!("`{IMPORT_MODULE}.remote`")) && text.contains("player 1.9.0"), "{text}");
    // Each import is named once, though both program variants need it.
    assert_eq!(text.matches(&format!("`{IMPORT_MODULE}.remote`")).count(), 1, "{text}");
}

#[test]
fn a_player_that_gives_every_import_is_accepted_and_one_without_a_record_is_not_judged() {
    let out = assemble_bundle(&support::input_with(player_giving(PRODUCTION_IMPORTS), BundleOptions::default()));
    assert!(out.is_ok(), "{:?}", out.err().map(|e| e.to_string()));
    let mut unknown = player_giving(&[]);
    unknown.build_info = None;
    assert!(assemble_bundle(&support::input_with(unknown, BundleOptions::default())).is_ok());
}
