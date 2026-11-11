// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-011 in the bundle: the application style the Build maps (R1), whose the page's address is (R5, Q7), the phone's
//! navigation stack (R13), and the player's navigation texts the developer may translate (plan §1.12).

mod support;

use cobolt_web_bundle::{assemble_bundle, AppStyle, BundleOptions, Embed, LoadingEffect, PlayerInput, PlayerVariantInput};
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

/// 2.0.413: the loading effect is the project's: `moving-stripes` unless the Build says `none`, written to `options.loading_effect`
/// and forwarded by the loader to the player, which names it on its busy mark.
#[test]
fn the_loading_effect_is_moving_stripes_unless_the_build_says_otherwise_and_the_loader_forwards_it() {
    assert_eq!(LoadingEffect::from_name("none"), Some(LoadingEffect::None));
    assert_eq!(LoadingEffect::from_name(" Moving-Stripes "), Some(LoadingEffect::MovingStripes));
    assert_eq!(LoadingEffect::from_name("confetti"), None, "a name that is no effect is refused");
    assert_eq!(LoadingEffect::default(), LoadingEffect::MovingStripes);
    assert_eq!(config(BundleOptions::default())["options"]["loading_effect"], "moving-stripes");
    assert_eq!(config(BundleOptions { loading_effect: LoadingEffect::None, ..Default::default() })["options"]["loading_effect"], "none");
    let out = assemble_bundle(&support::input_with(stub_player(), BundleOptions::default())).unwrap_or_else(|e| panic!("{e}"));
    let loader = out.files.iter().find(|f| f.path.ends_with(".js") && String::from_utf8_lossy(&f.bytes).contains("const pcfg = {")).expect("the loader");
    let text = String::from_utf8_lossy(&loader.bytes);
    assert!(text.contains("loading_effect: (config.options && config.options.loading_effect) || ''"), "the effect is forwarded to the player");
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

/// A page that mounts two applications (WASM-011 T19, T25): `PowerRustCOBOL.mount(element, { address })` reaches the loader — it
/// was documented and read by the loader but never passed on, so a page could not say which of its applications follows the
/// address — and a threaded player's pool of Workers is built once per player instance, not once per application (a second
/// build fails, because the instances are shared).
#[test]
fn the_page_api_passes_the_address_on_and_the_thread_pool_is_built_once_per_player_instance() {
    let out = assemble_bundle(&support::input_with(stub_player(), BundleOptions::default())).unwrap_or_else(|e| panic!("{e}"));
    let text_of = |needle: &str| {
        let f = out.files.iter().find(|f| f.path.ends_with(".js") && String::from_utf8_lossy(&f.bytes).contains(needle)).unwrap_or_else(|| panic!("no script contains `{needle}`"));
        String::from_utf8_lossy(&f.bytes).into_owned()
    };
    let glue = text_of("async function mount(element, params = {})");
    assert!(glue.contains("if (params.address === 'own' || params.address === 'none') options.address = params.address;"), "the page API passes `address` on");
    let loader = text_of("const pcfg = {");
    assert!(loader.contains("const pools = new WeakMap();"), "the pools are kept by player module");
    assert_eq!(loader.matches("initThreadPool(n)").count(), 1, "the pool is built in one place");
    assert!(loader.contains("if (!pools.has(playerModule)) {"), "and only when the player instance has none");
    assert!(loader.contains("await pools.get(playerModule);"), "a second application waits for the first one's pool");
}
