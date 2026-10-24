// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-003 AC10 and R23 on a real Web project with a Backend: a secret planted
//! in a form stops the Web Build, naming the file and the pattern, never the
//! value; the same value only in `backend/backend.toml` — where a backend's
//! secrets belong (R17) — leaves the Build going, and nothing under `backend/`
//! reaches the bundle. The Build reports the page's `connect-src`: itself and
//! the configured backend's origin, nothing else.

use cobolt_forms::model::{Control, ControlType, Form};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildError, WebBuildRequest};
use std::path::{Path, PathBuf};

const BACKEND_URL: &str = "wss://api.example.com/ws";

/// AWS's documented example key, built from fragments so that this file holds
/// none for a repository scan to find. A recognisable fake.
fn planted() -> String {
    format!("A{}{}", "KIA", "IOSFODNN7EXAMPLE")
}

/// A Web project with a Backend; the form's label shows `caption`, and
/// `backend/backend.toml` holds the planted value as its TLS password.
fn project(tag: &str, caption: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-ac10-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::create_dir_all(dir.join("backend/src")).unwrap();
    std::fs::write(
        dir.join("Shop.project.toml"),
        format!(
            "[project]\nname = \"Shop.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
             [files]\nforms = [\"webforms/main-form.wcfrm\"]\nbackend = [\"backend/src/health.cbl\"]\n\n\
             [web]\nbackend_url = \"{BACKEND_URL}\"\n"
        ),
    )
    .unwrap();
    std::fs::write(dir.join("backend/src/health.cbl"), cobolt_project_tools::create::starter_backend("Shop")).unwrap();
    std::fs::write(dir.join("backend/exports.toml"), cobolt_project_tools::create::starter_backend_exports("Shop")).unwrap();
    std::fs::write(
        dir.join("backend/backend.toml"),
        format!("[server]\nlisten = \"127.0.0.1:8443\"\n\n[tls]\npkcs12 = \"backend.p12\"\npkcs12_password = \"{}\"\n", planted()),
    )
    .unwrap();
    let mut form = Form::new("main-form", "Shop", 400, 300);
    let mut lbl = Control::new("LBL-NOTE", ControlType::Label, 10, 10);
    lbl.set_prop("Caption", caption);
    form.controls.push(lbl);
    cobolt_forms::xml::save_form(&form, &dir.join("webforms/main-form.wcfrm")).unwrap();
    dir
}

/// Every file under `dir`, relative, with its bytes.
fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push((p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"), std::fs::read(&p).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn a_frontend_secret_stops_the_build_and_the_backends_own_does_not() {
    let key = planted();

    // 1. The value in the form: the Build stops, naming the file and the pattern.
    let dir = project("form", &key);
    let req = WebBuildRequest::from_manifest(&dir.join("Shop.project.toml")).unwrap();
    match GeneratorBackend.build(&req) {
        Err(WebBuildError::Failed(why)) => {
            println!("in a form       → stopped: {why}");
            assert!(!why.contains(&key), "the value is never shown: {why}");
            assert!(why.contains("main-form"), "the file is named: {why}");
            assert!(why.contains("secret (cloud access key id)"), "the pattern is named: {why}");
        }
        other => panic!("expected the Build to stop on the key, got {other:?}"),
    }
    assert!(!req.out_dir.join("index.html").exists(), "nothing was written");
    let _ = std::fs::remove_dir_all(&dir);

    // 2. The same value only in backend/backend.toml: the Build goes on.
    let dir = project("backend", "Welcome");
    let req = WebBuildRequest::from_manifest(&dir.join("Shop.project.toml")).unwrap();
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("the backend's own secret must not stop the Web Build: {e}"));
    let bundle = files(&report.bundle_dir);
    println!("in backend.toml → built: {} file(s), {} bytes", bundle.len(), report.size.total_bytes);
    let from_backend: Vec<&String> = bundle.iter().map(|(p, _)| p).filter(|p| p.starts_with("backend/") || p.ends_with(".toml") || p.ends_with(".cbl")).collect();
    assert!(from_backend.is_empty(), "nothing of backend/ is in the bundle: {from_backend:?}");
    let holding: Vec<&String> = bundle.iter().filter(|(_, b)| b.windows(key.len()).any(|w| w == key.as_bytes())).map(|(p, _)| p).collect();
    assert!(holding.is_empty(), "no bundle file holds the value: {holding:?}");

    // 3. R23: the report says where the page may connect — itself and the backend.
    println!("connect-src     → {}", report.connect_src);
    assert_eq!(report.connect_src, "connect-src 'self' wss://api.example.com");
    assert_eq!(report.to_json()["connect_src"], report.connect_src.as_str());
    let page = String::from_utf8_lossy(&bundle.iter().find(|(p, _)| p == "index.html").expect("index.html").1).replace("&#39;", "'");
    assert!(page.contains(&report.connect_src), "the page carries the policy the report names");
    let _ = std::fs::remove_dir_all(&dir);
}
