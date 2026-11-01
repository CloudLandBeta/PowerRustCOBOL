// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Bundled fonts** (WASM-004 Q5, T24): the stand-in face the desktop letters "Arial" in, and every family a
//! form names that the project supplies under `Assets/`, are `@font-face` rules of the page; a family only the
//! operating system provides is not shipped and the Build names it; more than half of the application in font
//! files is a warning, never a failure.

mod support;

use cobolt_web_bundle::{AssetInput, BundleInput, BundleOptions, PlayerInput, PlayerVariantInput, FONT_SHARE_PREFIX, UNBUNDLED_FONT_PREFIX};

/// The face the bundle already carries, used here as a project's own file: its `name` table says
/// "Ubuntu Light" (family) and "Ubuntu" (typographic family).
const UBUNTU_LIGHT: &[u8] = include_bytes!("../../../assets/web-fonts/Ubuntu-Light.ttf");

fn player(wasm_bytes: usize) -> PlayerInput {
    let plain = PlayerVariantInput {
        files: vec![("player.js".into(), b"export default async function init() {}\n".to_vec()), ("player_bg.wasm".into(), vec![0u8; wasm_bytes])],
        entry_js: "player.js".into(),
        entry_wasm: "player_bg.wasm".into(),
    };
    PlayerInput { plain: Some(plain), ..Default::default() }
}

/// The fixture's forms with `FontName` set on the first control of each: `families[i]` on form `i`.
fn input(families: &[&str], assets: Vec<AssetInput>, wasm_bytes: usize) -> BundleInput {
    let mut input = support::input_with(player(wasm_bytes), BundleOptions::default());
    for (f, family) in input.forms.iter_mut().zip(families) {
        let at = f.cfrm.find("></Control>").expect("the fixture form has a control");
        f.cfrm.insert_str(at + 1, &format!("<Property name=\"FontName\">{family}</Property>"));
    }
    input.assets = assets;
    input
}

fn app_css(out: &cobolt_web_bundle::BundleOutput) -> String {
    let f = out.files.iter().find(|f| f.path.starts_with("app.") && f.path.ends_with(".css")).expect("app.css");
    String::from_utf8(f.bytes.clone()).unwrap()
}

fn asset_url(out: &cobolt_web_bundle::BundleOutput, path: &str) -> String {
    out.config()["assets"][path].as_str().unwrap_or_else(|| panic!("no asset {path} in config.json")).to_owned()
}

#[test]
fn fonts_the_built_in_face_and_an_assets_font_are_declared_and_copied() {
    let assets = vec![AssetInput { path: "Assets/fonts/Ubuntu-Light.ttf".into(), bytes: UBUNTU_LIGHT.to_vec() }];
    let out = cobolt_web_bundle::assemble_bundle(&input(&["Ubuntu Light", "Arial"], assets, 4_000_000)).unwrap_or_else(|e| panic!("{e}"));
    let css = app_css(&out);
    let url = asset_url(&out, "Assets/fonts/Ubuntu-Light.ttf");
    let stand_in = out.files.iter().find(|f| f.path.starts_with("fonts/Ubuntu-Light.") && f.path.ends_with(".ttf")).expect("the stand-in face is copied");
    let rules: Vec<&str> = css.lines().filter(|l| l.starts_with("@font-face")).collect();
    println!("{} @font-face rules:\n  {}", rules.len(), rules.join("\n  "));
    assert_eq!(rules.len(), 2, "the stand-in face and the project's own: {css}");
    assert!(rules[0].contains("\"PR Ubuntu Light\"") && rules[0].contains(&stand_in.path), "the built-in family's face first");
    assert_eq!(
        rules[1],
        format!("@font-face {{ font-family: \"Ubuntu Light\"; src: url(\"{url}\") format(\"truetype\"); font-weight: 100 900; font-style: normal; font-display: block; }}"),
        "the family as the form spells it, from the file under Assets/"
    );
    let copied = out.file(&url).expect("the Assets/ font is in the bundle");
    assert_eq!(copied.bytes, UBUNTU_LIGHT, "byte for byte");
    assert!(!out.report.warnings.iter().any(|w| w.starts_with(UNBUNDLED_FONT_PREFIX)), "{:?}", out.report.warnings);
    println!("font bytes {} of {} ({})", out.report.font_bytes, out.report.total_bytes, out.report.text().lines().find(|l| l.contains("font files")).unwrap());
    assert_eq!(out.report.font_bytes, 2 * UBUNTU_LIGHT.len() as u64, "the stand-in and the project's copy");
}

#[test]
fn fonts_a_family_is_found_by_its_file_name_and_by_its_family_name() {
    let mut form = cobolt_forms::xml::load_form_from_str(&support::input_with(player(1), BundleOptions::default()).forms[0].cfrm).unwrap();
    assert!(form.controls.len() >= 2, "the fixture form has controls to name fonts on");
    for (c, family) in form.controls.iter_mut().zip(["Brand Sans", "ubuntu light"]) {
        c.properties.insert("FontName".into(), cobolt_forms::model::PropValue::String(family.into()));
    }
    let assets = vec![
        AssetInput { path: "Assets/BrandSans.woff2".into(), bytes: vec![1, 2, 3] },
        AssetInput { path: "Assets/Ubuntu-Light.ttf".into(), bytes: UBUNTU_LIGHT.to_vec() },
        AssetInput { path: "Assets/logo.png".into(), bytes: vec![0; 8] },
    ];
    let plan = cobolt_web_bundle::font_faces([&form], &assets);
    for f in &plan.faces {
        println!("{} <- {} ({})", f.family, f.asset, f.format);
    }
    let pairs: Vec<(&str, &str, &str)> = plan.faces.iter().map(|f| (f.family.as_str(), f.asset.as_str(), f.format)).collect();
    // "Brand Sans" by the WOFF2's file name (its table is compressed), "ubuntu light" by the TrueType file's family name.
    assert_eq!(pairs, vec![("Brand Sans", "Assets/BrandSans.woff2", "woff2"), ("ubuntu light", "Assets/Ubuntu-Light.ttf", "truetype")]);
    assert!(plan.unbundled.is_empty(), "{:?}", plan.unbundled);
}

#[test]
fn fonts_a_family_only_the_operating_system_provides_is_skipped_and_named() {
    let out = cobolt_web_bundle::assemble_bundle(&input(&["Segoe UI", "Helvetica"], Vec::new(), 4_000_000)).unwrap_or_else(|e| panic!("{e}"));
    let named: Vec<&String> = out.report.warnings.iter().filter(|w| w.starts_with(UNBUNDLED_FONT_PREFIX)).collect();
    println!("{named:#?}");
    assert_eq!(named.len(), 1, "Helvetica is the built-in family, Segoe UI is not shipped: {:?}", out.report.warnings);
    assert!(named[0].contains("`Segoe UI`") && named[0].contains("form main-form"), "{}", named[0]);
    assert_eq!(app_css(&out).lines().filter(|l| l.starts_with("@font-face")).count(), 1, "the stand-in face only");
    assert!(!out.files.iter().any(|f| f.path.to_ascii_lowercase().contains("segoe")), "nothing of the developer's machine is copied");
}

/// The bundle with a project font of `font_size` bytes: the report's font share.
fn share(font_size: usize) -> (u64, u64, Vec<String>) {
    let assets = vec![AssetInput { path: "Assets/Big.woff2".into(), bytes: vec![7u8; font_size] }];
    let out = cobolt_web_bundle::assemble_bundle(&input(&["Big"], assets, 1_000_000)).unwrap_or_else(|e| panic!("{e}"));
    (out.report.font_bytes, out.report.total_bytes, out.report.warnings.clone())
}

#[test]
fn fonts_the_warning_is_present_at_51_percent_and_absent_at_49() {
    // The bytes that are not fonts, measured, then a font that makes the share 49 % and 51 % of the whole.
    let (f0, t0, _) = share(1);
    let rest = t0 - f0;
    let stand_in = f0 - 1;
    for (percent, want) in [(49u64, false), (51, true)] {
        let fonts = rest * percent / (100 - percent);
        let (f, t, warnings) = share((fonts - stand_in) as usize);
        let got = warnings.iter().find(|w| w.starts_with(FONT_SHARE_PREFIX));
        println!("fonts {f} of {t} bytes = {:.2}%: warning {}", f as f64 * 100.0 / t as f64, got.map_or("absent", |w| w.as_str()));
        assert_eq!(got.is_some(), want, "at {percent}%: {warnings:?}");
        assert_eq!(f as f64 * 100.0 / t as f64 > 50.0, want);
    }
}
