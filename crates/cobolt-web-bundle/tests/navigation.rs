// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-011 in the bundle: the application style the Build maps (R1), whose the page's address is (R5, Q7), the phone's
//! navigation stack (R13), and the player's navigation texts the developer may translate (plan §1.12).

mod support;

use cobolt_web_bundle::{assemble_bundle, AppStyle, BundleOptions, Embed, PlayerInput, PlayerVariantInput};
use serde_json::Value;

fn stub_player() -> PlayerInput {
    let files = vec![("player.js".to_owned(), b"export default 0;".to_vec()), ("player_bg.wasm".to_owned(), b"\0asm\x01\0\0\0".to_vec())];
    PlayerInput { plain: Some(PlayerVariantInput { files, entry_js: "player.js".into(), entry_wasm: "player_bg.wasm".into() }), ..Default::default() }
}

/// The `config.json` a bundle with these options writes.
fn config(options: BundleOptions) -> Value {
    let out = assemble_bundle(&support::input_with(stub_player(), options)).unwrap_or_else(|e| panic!("{e}"));
    let f = out.files.iter().find(|f| f.path == "config.json").expect("config.json");
    serde_json::from_slice(&f.bytes).expect("config.json is JSON")
}

#[test]
fn the_style_mapping_is_pinned_for_both_styles_and_the_default_is_spa() {
    assert_eq!(AppStyle::from_name("spa"), Some(AppStyle::Spa));
    assert_eq!(AppStyle::from_name("standalone"), Some(AppStyle::Standalone));
    assert_eq!(AppStyle::from_name(" Standalone "), Some(AppStyle::Standalone), "as `cobolt.toml` may be hand-written");
    assert_eq!(AppStyle::from_name("multi-page"), None);
    assert_eq!(AppStyle::default(), AppStyle::Spa);
    for s in [AppStyle::Spa, AppStyle::Standalone] {
        assert_eq!(AppStyle::from_name(s.as_str()), Some(s), "round trip");
    }
    assert_eq!(config(BundleOptions::default())["style"], "spa");
    assert_eq!(config(BundleOptions { style: AppStyle::Standalone, ..Default::default() })["style"], "standalone");
}

#[test]
fn the_page_owns_its_address_and_an_application_mounted_into_another_page_does_not_unless_asked() {
    let window = config(BundleOptions::default());
    assert_eq!(window["options"]["embed"]["mode"], "window");
    assert_eq!(window["options"]["embed"]["address"], "own");

    let element = config(BundleOptions { embed: Embed::Element { selector: "#app".into() }, ..Default::default() });
    assert_eq!(element["options"]["embed"]["selector"], "#app");
    assert_eq!(element["options"]["embed"]["address"], "none", "the host page's address is the host page's (R5)");

    let asked = config(BundleOptions { embed: Embed::Element { selector: "#app".into() }, own_address: Some(true), ..Default::default() });
    assert_eq!(asked["options"]["embed"]["address"], "own");

    let left = config(BundleOptions { own_address: Some(false), ..Default::default() });
    assert_eq!(left["options"]["embed"]["address"], "none");
}

#[test]
fn the_stack_is_off_unless_the_build_asks_for_it() {
    assert_eq!(config(BundleOptions::default())["options"]["stack"], false);
    assert_eq!(config(BundleOptions { stack: true, ..Default::default() })["options"]["stack"], true);
}

#[test]
fn the_players_navigation_texts_travel_in_the_page_texts() {
    let mut options = BundleOptions::default();
    for (k, v) in [("popup_blocked", "Erlauben Sie Pop-ups."), ("route_not_found", "Kein Formular {form}."), ("leave_unsaved", "Ungespeichert. Verlassen?"), ("leave", "Verlassen"), ("stay", "Bleiben")] {
        options.texts.insert(k.into(), v.into());
    }
    let c = config(options);
    assert_eq!(c["texts"]["route_not_found"], "Kein Formular {form}.");
    assert_eq!(c["texts"]["stay"], "Bleiben");
}

#[test]
fn the_loader_forwards_the_address_and_the_stack_to_the_player() {
    let out = assemble_bundle(&support::input_with(stub_player(), BundleOptions::default())).unwrap_or_else(|e| panic!("{e}"));
    let loader = out.files.iter().find(|f| f.path.ends_with(".js") && String::from_utf8_lossy(&f.bytes).contains("const pcfg = {")).expect("the loader");
    let text = String::from_utf8_lossy(&loader.bytes);
    assert!(text.contains("address: options.address || (config.options && config.options.embed && config.options.embed.address) || ''"), "address forwarded");
    assert!(text.contains("stack: !!(config.options && config.options.stack)"), "stack forwarded");
}
