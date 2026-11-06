// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-013 T11 (R3, AC3): the printed report says which Content-Security-Policy directives the page's
//! `<meta>` carries and which only a response header can carry (`frame-ancestors`, `report-uri`,
//! report-only mode), so a developer knows what the host must still send.

mod support;

use cobolt_web_bundle::{BundleOptions, PlayerInput, PlayerVariantInput};

fn player() -> PlayerInput {
    PlayerInput {
        plain: Some(PlayerVariantInput {
            files: vec![
                ("player.js".into(), b"export default async function init() {}\n".to_vec()),
                ("player_bg.wasm".into(), vec![0u8; 100]),
            ],
            entry_js: "player.js".into(),
            entry_wasm: "player_bg.wasm".into(),
        }),
        ..Default::default()
    }
}

fn section(text: &str) -> (String, String) {
    let meta = text.lines().find(|l| l.contains("in the page (<meta>)")).expect("a <meta> line").to_string();
    let header = text.lines().find(|l| l.contains("only as a response header")).expect("a header line").to_string();
    (meta, header)
}

#[test]
fn the_report_lists_the_header_only_directives_and_keeps_them_out_of_the_meta() {
    let out = support::assemble(player(), BundleOptions::default());
    let (meta, header) = section(&out.report.text());
    println!("{meta}\n{header}");
    assert!(header.contains("frame-ancestors"), "{header}");
    assert!(!meta.contains("frame-ancestors"), "{meta}");
    assert!(!meta.contains("report-uri") && !meta.contains("report-only"), "{meta}");
    assert!(meta.contains("default-src") || meta.contains("script-src"), "{meta}");
}

#[test]
fn report_only_mode_and_the_report_uri_are_header_only_too() {
    let mut o = BundleOptions::default();
    o.csp.report_only = true;
    o.csp.report_uri = Some("https://example.test/csp".into());
    let out = support::assemble(player(), o);
    let (meta, header) = section(&out.report.text());
    println!("{meta}\n{header}");
    assert!(header.contains("frame-ancestors") && header.contains("report-uri") && header.contains("report-only"), "{header}");
    assert!(!meta.contains("report-uri") && !meta.contains("example.test"), "{meta}");
}
