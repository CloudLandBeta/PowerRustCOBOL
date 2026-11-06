// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-013 T14 (Q4): `/.well-known/security.txt` from the developer's own `[security]` parameters. Written
//! when configured, absent (with a note, not a warning) when not, and refused when incomplete or out of date.

mod support;

use cobolt_wire::security_txt::SecurityTxt;
use cobolt_web_bundle::{assemble_bundle, BundleOptions, PlayerInput, PlayerVariantInput};

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

fn with(security: Option<SecurityTxt>) -> BundleOptions {
    BundleOptions { security, ..Default::default() }
}

#[test]
fn configured_the_file_is_written_with_contact_and_expires() {
    let t = SecurityTxt { contact: "mailto:security@example.com".into(), expires: "2999-01-31".into(), policy: "https://example.com/policy".into(), preferred_languages: "en, pt".into() };
    let out = support::assemble(player(), with(Some(t.clone())));
    let text = out.text(".well-known/security.txt").expect("security.txt is in the bundle");
    println!("{text}");
    assert!(text.contains("Contact: mailto:security@example.com\n") && text.contains("Expires: 2999-01-31T23:59:59Z\n"), "{text}");
    assert_eq!(text, t.render(0).unwrap(), "the bundle and the backend write one text");
    assert!(!out.report.notes.iter().any(|n| n.contains("security.txt")), "{:?}", out.report.notes);
}

#[test]
fn not_configured_nothing_is_written_and_the_report_says_so_as_a_note() {
    for opt in [None, Some(SecurityTxt::default())] {
        let out = support::assemble(player(), with(opt));
        assert!(out.text(".well-known/security.txt").is_none());
        assert!(out.report.notes.iter().any(|n| n.starts_with("no security.txt")), "{:?}", out.report.notes);
        assert!(!out.report.warnings.iter().any(|w| w.contains("security.txt")));
    }
}

#[test]
fn an_incomplete_or_stale_table_refuses_the_bundle_and_names_the_key() {
    let cases = [
        ("no contact", SecurityTxt { expires: "2999-01-31".into(), ..Default::default() }, "[security] contact"),
        ("no expires", SecurityTxt { contact: "mailto:a@example.com".into(), ..Default::default() }, "[security] expires"),
        ("expired", SecurityTxt { contact: "mailto:a@example.com".into(), expires: "2020-01-01".into(), ..Default::default() }, "has passed"),
        ("a contact that is not a URI", SecurityTxt { contact: "a@example.com".into(), expires: "2999-01-31".into(), ..Default::default() }, "[security] contact"),
    ];
    for (name, t, key) in cases {
        let err = assemble_bundle(&support::input_with(player(), with(Some(t)))).expect_err(name);
        let all = err.problems.join("\n");
        println!("{name}: {all}");
        assert!(all.contains(key), "{name}: {all}");
    }
}
