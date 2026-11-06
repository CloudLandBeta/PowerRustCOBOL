// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Web Build gate over the project's own inputs (WASM-013 T7, R7, R11; AC6, AC9 build half): before anything is
//! generated, the `unsafe` settings are judged against the profile and the secret scan runs over the forms, the
//! Common Code and copybooks, the stylesheets, the text assets and `backend/backend.toml`. A refusal names the file,
//! the line and the pattern; the planted value is never in the text.

use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{BuildProfile, WebBuildBackend, WebBuildError, WebBuildRequest};
use cobolt_web_bundle::secrets::{Severity, PATTERNS};
use std::path::PathBuf;

/// One planted value per `PATTERNS` Error, built from fragments so this file holds none for a repository scan to find.
fn planted(pattern: &str) -> (String, String) {
    let jwt = format!("ey{}.ey{}.{}", "JhbGciOiJSUzI1NiJ9", "JzdWIiOiIxMjM0NTY3ODkwIn0", "c2lnbmF0dXJl");
    let (text, value): (String, &str) = match pattern {
        "private key block" => (format!("-----BEGIN RSA {}-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC\n-----END RSA {}-----", "PRIVATE KEY", "PRIVATE KEY"), "MIIEvQIBADANBgkq"),
        "cloud access key id" => (format!("key = A{}{}", "KIA", "IOSFODNN7EXAMPLE"), "IOSFODNN7"),
        "JSON Web Token" => (format!("t: {jwt}"), "c2lnbmF0dXJl"),
        "connection string with a password" => (format!("db = \"postgres://app:{}@db.example.com/prod\"", "s3cr3tPassw0rd"), "s3cr3tPassw0rd"),
        "connection string with a Key=value secret" => (format!("Server=db;{}={};", "Password", "Zr8pQ2xLmVh"), "Zr8pQ2xLmVh"),
        "Google API key" => (format!("k = \"{}{}\"", "AIza", "SyA1234567890abcdefghijklmnopqrstuvw"), "SyA1234567890"),
        "GitHub token" => (format!("t = {}{}", "ghp_", "abcdefghijklmnopqrstuvwxyz0123456789"), "abcdefghijklmnop"),
        "Slack token" => (format!("t = {}{}", "xoxb-", "123456789012-abcdefghijklmnop"), "123456789012"),
        "Stripe live key" => (format!("k = {}{}", "sk_live_", "abcdefghijklmnopqrstuvwx"), "abcdefghijklmnopqrstuvwx"),
        "Anthropic API key" => (format!("k = {}{}", "sk-ant-", "api03-abcdefghijklmnopqrstuvwxyz"), "api03-abcdefghij"),
        "Hugging Face token" => (format!("k = {}{}", "hf_", "abcdefghijklmnopqrstuvwxyzABCD"), "abcdefghijklmnopqrstu"),
        "API key" => (format!("k = {}{}", "sk-", "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMN"), "abcdefghijklmnopqrstuv"),
        "a property named like a secret holds a value" => (format!("<Property name=\"AgentAPIKey\">{}</Property>", "q8Zk3vLw9PxT2mRb"), "q8Zk3vLw9PxT2mRb"),
        "a secret access key" => (format!("{}={}", "aws_secret_access_key", "wJalrXUtnFEMI/K7MDENG/bPxRfiCYzq8Zk3vLwA"), "wJalrXUtnFEMI"),
        other => panic!("no planted value for {other}"),
    };
    (text, value.to_owned())
}

/// A Web project; `extra` is `(relative path, text)` files written into it, `files_extra` the `[files]` lines.
fn project(tag: &str, manifest_extra: &str, files_extra: &str, extra: &[(&str, String)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-gate-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Gate.project.toml"),
        format!("[project]\nname = \"Gate.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n[files]\nforms = [\"webforms/main-form.wcfrm\"]\n{files_extra}\n\n[web]\n{manifest_extra}\n"),
    )
    .unwrap();
    let mut form = cobolt_forms::model::Form::new("main-form", "Gate", 400, 300);
    form.controls.push(cobolt_forms::model::Control::new("LBL-NOTE", cobolt_forms::model::ControlType::Label, 10, 10));
    cobolt_forms::xml::save_form(&form, &dir.join("webforms/main-form.wcfrm")).unwrap();
    for (rel, text) in extra {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    dir
}

fn refusal_text(r: Result<Vec<cobolt_project_tools::web::WebDiagnostic>, WebBuildError>) -> String {
    match r {
        Err(WebBuildError::Refused(list)) => list.iter().map(|d| format!("{}: {} [{}]", d.location, d.message, d.subject)).collect::<Vec<_>>().join("\n"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// AC6: every Error pattern, planted in a form, a copybook and a stylesheet, stops the Build naming file and pattern.
#[test]
fn every_error_pattern_in_a_form_a_copybook_and_a_stylesheet_stops_the_build() {
    let errors: Vec<&str> = PATTERNS.iter().filter(|(_, s)| *s == Severity::Error).map(|(n, _)| *n).collect();
    let mut checked = 0;
    for pattern in &errors {
        let (text, value) = planted(pattern);
        let form = std::fs::read_to_string(project("seed", "", "", &[]).join("webforms/main-form.wcfrm")).unwrap();
        for (scope, file) in [("form", "webforms/main-form.wcfrm"), ("copybook", "src/secrets.cpy"), ("stylesheet", "webforms/application.css")] {
            let extra: Vec<(&str, String)> = match scope {
                "form" => vec![(file, format!("{form}\n<!-- {text} -->\n"))],
                "copybook" => vec![(file, format!("       01 X PIC X.\n*> {text}\n"))],
                _ => vec![(file, format!("/* {text} */\n"))],
            };
            let dir = project(scope, "", if scope == "copybook" { "sources = [\"src/secrets.cpy\"]" } else { "" }, &extra);
            let req = WebBuildRequest::from_manifest(&dir.join("Gate.project.toml")).unwrap();
            let why = refusal_text(GeneratorBackend.check_with_warnings(&req));
            assert!(why.contains(file) && why.contains(pattern), "{scope} / {pattern}: {why}");
            assert!(!why.contains(&value), "{scope} / {pattern}: the value is shown: {why}");
            assert!(GeneratorBackend.build(&req).is_err() && !req.out_dir.join("index.html").exists(), "{scope} / {pattern}: nothing was built");
            checked += 1;
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
    println!("web inputs gate: {} Error patterns x 3 files = {checked} refusals, each naming file:line and pattern, no value shown", errors.len());
    assert_eq!(checked, errors.len() * 3);
}

/// AC9 (build half): each `unsafe` key warns in Development and fails a Release build naming the key.
#[test]
fn each_unsafe_key_warns_in_development_and_fails_in_release() {
    for key in ["plain_ws", "skip_secret_scan", "csp_report_only"] {
        let dir = project(key, &format!("\n[web.unsafe]\n{key} = true"), "", &[]);
        let mut req = WebBuildRequest::from_manifest(&dir.join("Gate.project.toml")).unwrap();
        let warnings = GeneratorBackend.check_with_warnings(&req).unwrap_or_else(|e| panic!("{key}: Development must not refuse: {e}"));
        assert!(warnings.iter().any(|w| w.message.starts_with(&format!("web.unsafe.{key} "))), "{key}: {warnings:?}");
        req.profile = BuildProfile::Release;
        let why = refusal_text(GeneratorBackend.check_with_warnings(&req));
        assert!(why.contains(&format!("web.unsafe.{key}")), "{key}: {why}");
        assert!(matches!(GeneratorBackend.build(&req), Err(WebBuildError::Refused(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }
    println!("unsafe keys: plain_ws, skip_secret_scan, csp_report_only each warn in Development and refuse in Release");
}

/// `skip_secret_scan` skips the project scan (a Development choice a Release build refuses); a literal in
/// `backend/backend.toml` is a warning in Development and a refusal in Release.
#[test]
fn skip_and_the_backend_toml_follow_the_profile() {
    let (text, value) = planted("cloud access key id");
    let dir = project("skip", "\n[web.unsafe]\nskip_secret_scan = true", "", &[("src/leak.cpy", format!("*> {text}\n"))]);
    let req = WebBuildRequest::from_manifest(&dir.join("Gate.project.toml")).unwrap();
    assert!(GeneratorBackend.check_with_warnings(&req).is_ok(), "the scan is skipped when asked");

    let toml = format!("[tls]\npkcs12_password = \"{text}\"\n");
    let dir = project("toml", "", "", &[("backend/backend.toml", toml)]);
    let mut req = WebBuildRequest::from_manifest(&dir.join("Gate.project.toml")).unwrap();
    let warnings = GeneratorBackend.check_with_warnings(&req).unwrap_or_else(|e| panic!("Development only warns: {e}"));
    assert!(warnings.iter().any(|w| w.location.starts_with("backend/backend.toml:") && !w.message.contains(&value)), "{warnings:?}");
    req.profile = BuildProfile::Release;
    let why = refusal_text(GeneratorBackend.check_with_warnings(&req));
    assert!(why.contains("backend/backend.toml:") && why.contains("cloud access key id") && !why.contains(&value), "{why}");
    println!("backend.toml literal: a warning in Development, a refusal in Release; skip_secret_scan skips the scan");
}
