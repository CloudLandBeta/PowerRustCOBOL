// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The first download against the 5 MB budget (WASM-002 R8, Q5 — answered, to confirm): a line in the
//! report always, a warning when over, never a failure.

mod support;

use cobolt_web_bundle::{
    BundleOptions, PlayerInput, PlayerVariantInput, FIRST_DOWNLOAD_BUDGET, OVER_BUDGET_PREFIX,
};

fn player(wasm_bytes: usize) -> PlayerInput {
    let plain = PlayerVariantInput {
        files: vec![
            (
                "player.js".into(),
                b"export default async function init() {}\n".to_vec(),
            ),
            ("player_bg.wasm".into(), vec![0u8; wasm_bytes]),
        ],
        entry_js: "player.js".into(),
        entry_wasm: "player_bg.wasm".into(),
    };
    PlayerInput {
        plain: Some(plain),
        ..Default::default()
    }
}

#[test]
fn budget_under_it_a_line_and_no_warning() {
    let out = support::assemble(player(1_000), BundleOptions::default());
    let r = &out.report;
    println!(
        "first download {} of {} bytes",
        r.budget.first_download_bytes, r.budget.budget_bytes
    );
    assert_eq!(r.budget.budget_bytes, FIRST_DOWNLOAD_BUDGET);
    assert!(r.budget.first_download_bytes > 1_000 && !r.budget.over());
    assert!(
        r.text().contains("first download (budget 5000000 bytes)"),
        "{}",
        r.text()
    );
    assert!(
        !r.warnings.iter().any(|w| w.starts_with(OVER_BUDGET_PREFIX)),
        "{:?}",
        r.warnings
    );
}

#[test]
fn budget_over_it_a_warning_and_the_bundle_still_builds() {
    let out = support::assemble(
        player(FIRST_DOWNLOAD_BUDGET as usize),
        BundleOptions::default(),
    );
    let r = &out.report;
    println!(
        "first download {} of {} bytes",
        r.budget.first_download_bytes, r.budget.budget_bytes
    );
    assert!(r.budget.over());
    assert!(
        r.warnings.iter().any(|w| w.starts_with(OVER_BUDGET_PREFIX)),
        "{:?}",
        r.warnings
    );
    assert!(r.text().contains(", over)"), "{}", r.text());
    assert!(out.file("index.html").is_some(), "built all the same");
}
