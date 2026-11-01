// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-009 T20 (AC6, D5, D14): what a bundle with an Authentication control may hold and reach.
//! A backend identity secret, known by value, is refused wherever the bundle holds it (a form, the
//! page's configuration, an asset), named by file and setting and never shown. The control's own
//! words (`PasswordLabel="Senha"`) are not a finding, and the backend's https origin joins
//! `connect-src` only when a form has the control.

mod support;

use cobolt_web_bundle::secrets::KnownSecrets;
use cobolt_web_bundle::{assemble_bundle, AssetInput, BundleInput, BundleOptions, PlayerInput, PlayerVariantInput};

fn stub_player() -> PlayerInput {
    let files = vec![("player.js".to_owned(), b"export default 0;".to_vec()), ("player_bg.wasm".to_owned(), b"\0asm\x01\0\0\0".to_vec())];
    PlayerInput { plain: Some(PlayerVariantInput { files, entry_js: "player.js".into(), entry_wasm: "player_bg.wasm".into() }), ..Default::default() }
}

const CLIENT: &str = "client-secret-AC6-0123456789";
const RECAPTCHA: &str = "6Lc-recaptcha-secret-AC6-0123";
const COOKIE: &str = "Y29va2llLWtleS1BQzYtMDEyMzQ1Njc4OQ==";

fn known() -> KnownSecrets {
    KnownSecrets(vec![
        ("identity.settings.client_secret".into(), CLIENT.into()),
        ("identity.recaptcha.secret".into(), RECAPTCHA.into()),
        ("identity.cookie_key".into(), COOKIE.into()),
    ])
}

/// The fixture's input with an Authentication control on its first form, holding `props`.
fn with_auth(options: BundleOptions, props: &str) -> BundleInput {
    let mut input = support::input_with(stub_player(), options);
    let form = &mut input.forms[0];
    let control = format!("  <Control id=\"AUTH-1\" type=\"Authentication\" x=\"10\" y=\"300\" w=\"300\" h=\"300\">{props}</Control>\n</Form>");
    form.cfrm = form.cfrm.replace("</Form>", &control);
    input
}

#[test]
fn a_known_secret_is_refused_by_file_and_setting_wherever_the_bundle_holds_it() {
    let mut input = with_auth(
        BundleOptions { known_secrets: known(), ..Default::default() },
        &format!("<Property name=\"PasswordLabel\">Senha</Property><Property name=\"Hint\">{CLIENT}</Property>"),
    );
    input.options.texts.insert("loading".into(), format!("Loading {RECAPTCHA}"));
    input.assets.push(AssetInput { path: "Assets/notes.txt".into(), bytes: format!("key = {COOKIE}\n").into_bytes() });
    let e = assemble_bundle(&input).expect_err("a bundle holding a known secret is refused");
    let shown = e.problems.join("\n");
    println!("{shown}");
    for setting in ["identity.settings.client_secret", "identity.recaptcha.secret", "identity.cookie_key"] {
        assert!(e.problems.iter().any(|p| p.contains(setting) && p.contains(": line ")), "{setting} is named with its file and line:\n{shown}");
    }
    // Each in the file the bundle holds it in: the form's description, the page's configuration, the asset.
    for (file, setting) in [("forms/", "identity.settings.client_secret"), ("config.json", "identity.recaptcha.secret"), ("assets/notes.", "identity.cookie_key")] {
        assert!(e.problems.iter().any(|p| p.starts_with(file) && p.contains(setting)), "{file} / {setting}:\n{shown}");
    }
    for v in [CLIENT, RECAPTCHA, COOKIE] {
        assert!(!shown.contains(v), "a value is shown:\n{shown}");
    }
}

#[test]
fn the_same_values_kept_only_in_the_backend_build_and_the_controls_words_are_not_secrets() {
    let input = with_auth(
        BundleOptions { known_secrets: known(), recaptcha_site_key: "6Lc-site-key-public".into(), ..Default::default() },
        "<Property name=\"PasswordLabel\">Senha</Property><Property name=\"ForgotPasswordLink\">Esqueceu a senha?</Property>",
    );
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(out.report.warnings.iter().filter(|w| w.contains("secret")).count(), 0, "{:?}", out.report.warnings);
}

/// The policy value of `index.html`.
fn policy(input: &BundleInput) -> String {
    let out = assemble_bundle(input).unwrap_or_else(|e| panic!("{e}"));
    let page = String::from_utf8(out.files.iter().find(|f| f.path == "index.html").expect("index.html").bytes.clone()).unwrap();
    let at = page.find("http-equiv=\"Content-Security-Policy\"").expect("a policy");
    let rest = &page[at..];
    let start = rest.find("content=\"").unwrap() + "content=\"".len();
    rest[start..start + rest[start..].find('"').unwrap()].replace("&#39;", "'")
}

fn connect_src(p: &str) -> &str {
    p.split(';').map(str::trim).find(|d| d.starts_with("connect-src")).unwrap_or("")
}

#[test]
fn the_backends_https_origin_joins_connect_src_only_with_an_authentication_control() {
    let opts = || BundleOptions { backend_origins: vec!["wss://api.example.test:8443/ws".into()], recaptcha_site_key: "6Lc-site".into(), ..Default::default() };
    let with = policy(&with_auth(opts(), ""));
    println!("with the control: {}", connect_src(&with));
    assert!(connect_src(&with).contains("https://api.example.test:8443"), "{with}");
    assert!(with.contains("https://www.google.com/recaptcha/"), "the site key opens reCAPTCHA: {with}");
    let without = policy(&support::input_with(stub_player(), opts()));
    println!("without: {}", connect_src(&without));
    assert!(!connect_src(&without).contains("https://api.example.test"), "{without}");
    assert_eq!(cobolt_web_bundle::http_origin_of_backend("ws://127.0.0.1:8080/ws").as_deref(), Some("http://127.0.0.1:8080"));
    assert_eq!(cobolt_web_bundle::http_origin_of_backend("ftp://x"), None);
}
