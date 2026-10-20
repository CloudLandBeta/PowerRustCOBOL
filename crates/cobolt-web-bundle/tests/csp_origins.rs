// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The origins the page's Content-Security-Policy opens, and only when a feature needs them
//! (WASM-013, WASM-009 R4): the policy of a bundle with no reCAPTCHA site key names no Google
//! origin, one with a key names exactly the reCAPTCHA prefixes, in the directives that need them.

mod support;

use cobolt_web_bundle::{BundleOptions, PlayerInput, PlayerVariantInput};

/// A player that is only present: these tests read the page, not the player.
fn stub_player() -> PlayerInput {
    let files = vec![("player.js".to_owned(), b"export default 0;".to_vec()), ("player_bg.wasm".to_owned(), b"\0asm\x01\0\0\0".to_vec())];
    PlayerInput { plain: Some(PlayerVariantInput { files, entry_js: "player.js".into(), entry_wasm: "player_bg.wasm".into() }), ..Default::default() }
}

/// The value of the policy `<meta>` of `index.html`, split into directives.
fn directives(options: BundleOptions) -> Vec<(String, String)> {
    let out = support::assemble(stub_player(), options);
    let page = out.text("index.html").expect("index.html");
    let at = page.find("http-equiv=\"Content-Security-Policy\"").expect("the page carries its policy");
    let rest = &page[at..];
    let start = rest.find("content=\"").expect("content") + "content=\"".len();
    let value = &rest[start..start + rest[start..].find('"').expect("closing quote")];
    value
        .replace("&#39;", "'")
        .split(';')
        .map(|d| d.trim())
        .filter(|d| !d.is_empty())
        .map(|d| {
            let (k, v) = d.split_once(' ').unwrap_or((d, ""));
            (k.to_owned(), v.to_owned())
        })
        .collect()
}

fn get<'a>(d: &'a [(String, String)], name: &str) -> &'a str {
    d.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str()).unwrap_or("")
}

#[test]
fn without_a_site_key_no_google_origin_is_opened() {
    let d = directives(BundleOptions::default());
    let all: String = d.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join("; ");
    assert!(!all.contains("google") && !all.contains("gstatic"), "{all}");
    assert_eq!(get(&d, "frame-src"), "'none'");
    assert!(get(&d, "script-src").starts_with("'self' 'wasm-unsafe-eval'"));
}

#[test]
fn with_a_site_key_exactly_the_recaptcha_prefixes_are_opened_where_they_are_needed() {
    let d = directives(BundleOptions { recaptcha_site_key: "site-key".into(), ..Default::default() });
    let script = get(&d, "script-src");
    assert!(script.contains("https://www.google.com/recaptcha/") && script.contains("https://www.gstatic.com/recaptcha/"), "{script}");
    let frame = get(&d, "frame-src");
    assert!(frame.contains("https://www.google.com/recaptcha/") && frame.contains("https://recaptcha.google.com/recaptcha/"), "{frame}");
    // Not the whole of Google, never the connection directive, never an unsafe keyword.
    assert!(!script.contains("https://www.google.com/ ") && !script.ends_with("https://www.google.com"), "{script}");
    assert!(!get(&d, "connect-src").contains("google"), "connect-src stays the origin and the backend");
    assert!(!d.iter().any(|(_, v)| v.contains("'unsafe-inline'") || v.contains("'unsafe-eval'")));
    // The site key is public configuration and reaches the player through the page's own config.
    let out = support::assemble(stub_player(), BundleOptions { recaptcha_site_key: "site-key".into(), ..Default::default() });
    assert_eq!(out.config()["options"]["recaptcha_site_key"], "site-key");
    assert!(out.report.notes.iter().any(|n| n.contains("reCAPTCHA")), "the build says what it opened");
}
