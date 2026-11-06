// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The browser runner on the Web template's hello world** (WASM-015 T6, AC2): the real Build, served on
//! loopback, opened in the system web view's headless helper; a click, a typed name, a label's `text`, a
//! `style` colour and a picture are read back, and each step's time is printed with the engine.
//!
//! Without the helper or a web view the test prints `skipped: ...` and passes; `PRC_REQUIRE_WEBVIEW=1` makes
//! that a failure.

use std::sync::Arc;

use cobolt_project_tools::script::Step;
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::runner::{self, RunnerError};
use cobolt_project_tools::web::{self, preview::HELPER_ENV};
use cobolt_project_tools::{HeadlessHost, ProjectRoot, ProjectTools};
use cobolt_web_tests::{gate, summary, Need};
use serde_json::{json, Value};

#[test]
fn hello_world_bundle() {
    let test = "hello_world_bundle";
    let base = std::env::temp_dir().join(format!("prc-web-tests-runner-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let folder = base.join("Hello");
    let mut tools = ProjectTools::new(HeadlessHost::new(base.clone(), "test"));
    let made = tools.call("create_project", &json!({"folder": folder.to_string_lossy(), "name": "Hello", "template": "web"})).unwrap();
    let manifest = folder.join(made["project"].as_str().unwrap());
    tools.call("open_project", &json!({"path": manifest.to_string_lossy()})).unwrap();
    web::link(Arc::new(GeneratorBackend));
    let root = ProjectRoot::open(&manifest).unwrap();
    let form = "webforms/main-form.wcfrm";

    let helper = runner::find_helper();
    if !gate(test, Need::WebView, helper.is_some(), "the helper prc-web-preview is not built (cargo build -p cobolt-web-preview)") {
        let _ = std::fs::remove_dir_all(&base);
        return;
    }

    let steps = [
        Step::Read { control: "LBL-RESULT".into(), property: "Text".into() },
        Step::Event { control: "BTN-GREET".into(), name: "onClick".into(), wait: true },
        Step::Read { control: "LBL-RESULT".into(), property: "Text".into() },
        Step::Set { control: "TXT-NAME".into(), property: "Text".into(), value: "Ada".into() },
        Step::Event { control: "BTN-GREET".into(), name: "onClick".into(), wait: true },
        Step::Text { control: "LBL-RESULT".into() },
        Step::Style { control: "LBL-RESULT".into(), property: "color".into() },
    ];
    let started = std::time::Instant::now();
    let report = match runner::run(&root, form, None, &steps, 30) {
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
    let total_ms = started.elapsed().as_millis();

    let mut passed = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}: {:?}; errors {:?}", report.values, report.errors);
        passed += 1;
    };
    check(report.errors.is_empty(), "the page raised no error");
    check(report.values.len() == 4, "four steps answer a value");
    check(report.values[0] == json!(""), "the label starts empty");
    check(report.values[1] == json!("Type your name first."), "an empty box asks for a name");
    check(report.values[2] == json!("Hello, Ada"), "a typed name is greeted (read by the text step)");
    check(report.values[3].as_str().is_some_and(|c| c.starts_with("rgb")), "a style colour is an rgb value");
    check(report.png.as_ref().is_some_and(|p| p.len() > 1000 && p.starts_with(&[0x89, b'P', b'N', b'G'])), "the picture is a PNG of non-zero size");
    check(!report.engine.user_agent.is_empty(), "the engine reports its user agent");

    println!("browser runner: hello_world_bundle");
    println!("  engine {} — {}", report.engine.name, report.engine.user_agent);
    for line in runner::timing_lines(&report) {
        println!("  {line}");
    }
    println!(
        "  picture {} bytes; page {} ms of {} ms total; reads {}",
        report.png.as_ref().map_or(0, Vec::len),
        runner::total(&report).as_millis(),
        total_ms,
        Value::Array(report.values.clone())
    );
    println!("  {}", summary(passed, 0));
    std::env::remove_var(HELPER_ENV);
    let _ = std::fs::remove_dir_all(&base);
}
