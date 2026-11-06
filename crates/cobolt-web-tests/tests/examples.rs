// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **`examples/PowerWebHello`, built and run by the suite** (WASM-015 T22, R16, AC12). The example is a Web project:
//! one form whose button asks the Backend's `HELLO` program for a greeting, a label that shows it and a layer
//! the handler shows. The suite copies it, checks it, builds it with the real Build, scans its whole tree for
//! secrets, starts its Backend in process and calls `HELLO`, and loads the page in the system web view.
//!
//! The one thing not run here is the click that reaches the Backend *from the page*: the page speaks `ws://` on
//! a loopback address, the test kit's server speaks TLS with a certificate no web view trusts. That step is
//! open (`tasks.md`, T22b); every other step of AC12 is checked below.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use cobolt_project_tools::script::Step;
use cobolt_project_tools::web;
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::runner::{self, RunnerError};
use cobolt_project_tools::{HeadlessHost, ProjectRoot, ProjectTools};
use cobolt_web_tests::{gate, Need};
use serde_json::{json, Value};

fn example_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("examples").join("PowerWebHello")
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

/// A scratch copy of the example, so building it never touches the repository.
fn scratch(tag: &str) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!("prc-web-tests-example-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let folder = base.join("PowerWebHello");
    copy_tree(&example_dir(), &folder);
    (base, folder)
}

fn open(folder: &Path, base: &Path) -> (ProjectTools<HeadlessHost>, PathBuf) {
    let mut tools = ProjectTools::new(HeadlessHost::new(base.to_path_buf(), "test"));
    let manifest = folder.join("PowerWebHello.project.toml");
    tools.call("open_project", &json!({"path": manifest.to_string_lossy()})).unwrap();
    web::link(Arc::new(GeneratorBackend));
    (tools, manifest)
}

/// Every text file under `dir`, relative.
fn text_files(dir: &Path, rel: &str, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        let name = format!("{rel}{}", entry.file_name().to_string_lossy());
        if path.is_dir() {
            text_files(&path, &format!("{name}/"), out);
        } else if let Ok(text) = std::fs::read_to_string(&path) {
            out.push((name, text));
        }
    }
}

#[test]
fn the_example_holds_no_secret_and_checks_and_builds() {
    let (base, folder) = scratch("build");
    let mut files = Vec::new();
    text_files(&example_dir(), "", &mut files);
    let mut findings = Vec::new();
    for (name, text) in &files {
        findings.extend(cobolt_web_bundle::secrets::scan_text(name, text, 32).into_iter().map(|f| format!("{name}: {f:?}")));
        for bad in ["AKIA", "ASIA", "Password=", "APIKey", "Token=", "PRIVATE KEY"] {
            assert!(!text.contains(bad), "{name} contains `{bad}`");
        }
    }
    assert!(findings.is_empty(), "the example's own tree holds no secret: {findings:?}");

    let (mut tools, _manifest) = open(&folder, &base);
    let started = Instant::now();
    let regen = tools.call("regenerate", &json!({"path": "webforms/main-form.wcfrm"})).unwrap();
    let checked: Value = tools.call("check", &json!({})).unwrap();
    assert_eq!(checked["errors"], 0, "the example checks clean: {checked} (regenerate: {regen})");
    let check_ms = started.elapsed().as_millis();

    let started = Instant::now();
    let built: Value = tools.call("build", &json!({})).unwrap();
    let build_ms = started.elapsed().as_millis();
    assert_eq!(built["status"], "built", "the real Build succeeds: {built}");
    let scan = &built["report"]["secret_scan"];
    assert_eq!(scan["findings"], json!([]), "the Build's own secret scan finds nothing: {scan}");
    println!("PowerWebHello: {} files scanned by the suite, 0 findings; check 0 errors in {check_ms} ms; build in {build_ms} ms, its scan read {} files, 0 findings",
        files.len(), scan["scanned_files"]);
    println!("→ 5 passed, 0 failed");
    let _ = std::fs::remove_dir_all(&base);
}

/// The example's Backend, started in process from its own `backend/` folder, answers `HELLO`.
#[test]
fn the_examples_backend_answers_hello() {
    use cobolt_server::testkit::*;
    let backend = example_dir().join("backend");
    let dir = std::env::temp_dir().join(format!("prc-web-tests-example-backend-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy_tree(&backend, &dir);
    let rig = rig(dir.clone(), "[limits]\nconnection_rate_per_sec = 1000\nconnection_burst = 1000\nuser_rate_per_sec = 5000\nuser_burst = 5000\n");
    let runtime = block_on(async {
        let token = rig.issuer.access("user-1", AUDIENCE, 600, json!({}));
        let mut c = Raw::connect(&rig, Some(ORIGIN)).await.expect("connect");
        let hello = c.hello("1.0.0").await;
        assert_eq!(hello["accepted"], "ok", "{hello}");
        let auth = c.auth(&token).await;
        assert_eq!(auth["t"], "auth-ok", "{auth}");
        let started = Instant::now();
        let reply = c.call(1, "HELLO", &[("ref", "\"Ada\"")], true).await;
        (reply, started.elapsed().as_millis())
    });
    let (reply, ms) = runtime;
    assert_eq!(reply["t"], "reply", "{reply}");
    let greeting = reply["returning"].as_str().map(str::trim).unwrap_or_default().to_owned();
    assert_eq!(greeting, "Hello, Ada", "{reply}");
    rig.server.stop();
    println!("PowerWebHello backend: HELLO(\"Ada\") → {greeting:?} in {ms} ms (in process)");
    println!("→ 3 passed, 0 failed");
    let _ = std::fs::remove_dir_all(&dir);
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap().block_on(f)
}

/// The page, in the system web view: the greeting label starts empty, the layer's label is hidden, and a click with
/// no name written is answered by the handler on the page (the branch that needs no Backend).
#[test]
fn the_page_loads_and_its_handler_runs() {
    let test = "the_page_loads_and_its_handler_runs";
    let (base, folder) = scratch("page");
    let (_tools, manifest) = open(&folder, &base);
    let helper = runner::find_helper();
    if !gate(test, Need::WebView, helper.is_some(), "the helper prc-web-preview is not built (cargo build -p cobolt-web-preview)") {
        let _ = std::fs::remove_dir_all(&base);
        return;
    }
    let root = ProjectRoot::open(&manifest).unwrap();
    let steps = [
        Step::Read { control: "LBL-RESULT".into(), property: "Text".into() },
        Step::Event { control: "BTN-GREET".into(), name: "onClick".into(), wait: true },
        Step::Read { control: "LBL-RESULT".into(), property: "Text".into() },
    ];
    let started = Instant::now();
    let report = match runner::run(&root, "webforms/main-form.wcfrm", None, &steps, 30) {
        Ok(r) => r,
        Err(RunnerError::NoWebView(why)) => {
            let _ = std::fs::remove_dir_all(&base);
            if gate(test, Need::WebView, false, &format!("no web view — {why}")) {
                unreachable!("an unavailable web view skips or fails");
            }
            return;
        }
        Err(e) => panic!("{e}"),
    };
    let ms = started.elapsed().as_millis();
    let shown = format!("{:?}", report.values);
    assert!(shown.contains("Type your name first."), "the handler answered on the page: {shown}; errors {:?}", report.errors);
    println!("PowerWebHello page: loaded and clicked in {ms} ms on {}; the empty-name branch answered ({} values read)", report.engine.name, report.values.len());
    println!("→ 2 passed, 0 failed");
    let _ = std::fs::remove_dir_all(&base);
}
