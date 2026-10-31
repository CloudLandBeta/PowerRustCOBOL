// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The stylesheets of a bundle (WASM-007 R4, R13, Q3, Q5; AC7): copied byte for byte, refused with
//! file and line when the check finds an error, the origins they load from reported and allowed by
//! the policy the page carries — exactly those — and the theme written as `theme.css`.

mod support;

use cobolt_web_bundle::{assemble_bundle, BundleOptions, CspOptions, PlayerInput, PlayerVariantInput};

fn stub_player() -> PlayerInput {
    let files = vec![("player.js".to_owned(), b"export default 0;".to_vec()), ("player_bg.wasm".to_owned(), b"\0asm\x01\0\0\0".to_vec())];
    PlayerInput { plain: Some(PlayerVariantInput { files, entry_js: "player.js".into(), entry_wasm: "player_bg.wasm".into() }), ..Default::default() }
}

/// One directive of the page's policy `<meta>`.
fn directive(page: &str, name: &str) -> String {
    let at = page.find("http-equiv=\"Content-Security-Policy\"").expect("the page carries its policy");
    let rest = &page[at..];
    let start = rest.find("content=\"").expect("content") + "content=\"".len();
    let value = rest[start..start + rest[start..].find('"').expect("closing quote")].replace("&#39;", "'");
    value.split(';').map(str::trim).find_map(|d| d.strip_prefix(&format!("{name} ")).map(str::to_owned)).unwrap_or_default()
}

#[test]
fn the_sheets_are_copied_byte_for_byte_and_their_origins_are_reported_and_allowed() {
    let form_css = "/* mine */\r\n#BTN-1   { background: url(https://img.example/bg.png) }\r\n\r\n";
    let app_css = "@font-face { font-family: Brand; src: url(https://cdn.example/brand.woff2) }\n.pr-button { font-family: Brand }\n";
    let theme_css = cobolt_forms::web_themes::theme_rules("elegance");
    let options = BundleOptions {
        image_origins: vec!["https://img.example".into()],
        csp: CspOptions { extra_font: vec!["https://cdn.example".into()], ..CspOptions::strict() },
        theme: Some(theme_css.clone()),
        ..Default::default()
    };
    let mut input = support::input_with(stub_player(), options);
    let first = input.forms[0].name.clone();
    input.forms[0].stylesheet = Some(form_css.into());
    input.stylesheet = Some(app_css.into());
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));

    let config = out.config();
    let form_entry = config["forms"].as_array().unwrap().iter().find(|f| f["name"] == first.as_str()).unwrap().clone();
    let file_bytes = |cfg: &serde_json::Value| out.file(cfg["url"].as_str().unwrap()).unwrap().bytes.clone();
    assert_eq!(file_bytes(&form_entry["css"]), form_css.as_bytes(), "the form's sheet, CRLF and spacing kept");
    assert_eq!(file_bytes(&config["stylesheet"]), app_css.as_bytes());
    assert_eq!(file_bytes(&config["theme"]), theme_css.as_bytes(), "theme.css is the theme's text");
    assert!(config["theme"]["url"].as_str().unwrap().starts_with("theme."), "{}", config["theme"]);

    // AC7: the report lists exactly what the sheets load from; the page's policy allows each.
    assert_eq!(out.report.stylesheet_img_sources, ["https://img.example"]);
    assert_eq!(out.report.stylesheet_font_sources, ["https://cdn.example"]);
    let page = out.text("index.html").unwrap();
    let (img, font) = (directive(&page, "img-src"), directive(&page, "font-src"));
    assert!(img.split(' ').any(|o| o == "https://img.example"), "img-src {img}");
    assert!(font.split(' ').any(|o| o == "https://cdn.example"), "font-src {font}");
    assert!(!font.contains("img.example") && !img.contains("cdn.example"), "each origin only where it is needed: img-src {img}; font-src {font}");
    let text = out.report.text();
    assert!(text.contains("the stylesheets load images from (img-src): https://img.example"), "{text}");
    assert!(text.contains("the stylesheets load fonts from (font-src): https://cdn.example"), "{text}");
    // The player checks a sheet at run time against the same origins.
    let runtime: Vec<String> = serde_json::from_value(config["options"]["image_origins"].clone()).unwrap();
    assert!(runtime.contains(&"https://img.example".to_owned()) && runtime.contains(&"https://cdn.example".to_owned()), "{runtime:?}");
    println!("AC7 (bundle): img-src {img}; font-src {font}; report img [https://img.example] font [https://cdn.example]");
}

#[test]
fn a_sheet_with_an_error_refuses_the_bundle_naming_file_and_line() {
    let mut input = support::input_with(stub_player(), BundleOptions::default());
    let first = input.forms[0].name.clone();
    input.forms[0].stylesheet = Some(".a { color: red }\n.b { background: url(https://evil.example/x.png) }\n".into());
    input.stylesheet = Some("\n\n.pr-form-layer { z-index: 3 }\n".into());
    let e = assemble_bundle(&input).expect_err("refused");
    let text = e.to_string();
    assert!(text.contains(&format!("forms/{first}.css:2:")) && text.contains("https://evil.example/x.png"), "{text}");
    assert!(text.contains("stylesheet.css:3:"), "the layer guard (R16): {text}");

    // A warning (an unknown property) does not stop the bundle.
    let mut input = support::input_with(stub_player(), BundleOptions::default());
    input.forms[0].stylesheet = Some(".a { colour: red }\n".into());
    assert!(assemble_bundle(&input).is_ok());
    println!("refused: {}", e.problems.join(" | "));
}

#[test]
fn without_a_theme_the_bundle_carries_no_theme_css() {
    let out = support::assemble(stub_player(), BundleOptions::default());
    assert!(out.config().get("theme").is_none());
    assert!(out.files.iter().all(|f| !f.path.starts_with("theme.")));
}
