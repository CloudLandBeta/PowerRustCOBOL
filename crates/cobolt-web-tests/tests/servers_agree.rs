// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The agents' tools on a web project, end to end, on both servers** (WASM-015 T17, R11, R12; AC8, AC9).
//!
//! AC8, `web_tool_sequence`: `create_project` (kind web) → `check` refuses a plug-in control → fix → `build`
//! (`built`, with the secret scan) → `render_form` (a PNG and `engine`) → `run_form` (a click and a read, and
//! `engine`). The two browser steps need the web view's helper; without it they print a skip (and fail under
//! `PRC_REQUIRE_WEBVIEW=1`).
//!
//! AC9, `both_servers_agree`: the IDE's server and `rcrun mcp` differ only in the renderer and runner they inject;
//! the same calls on the same project through a server with none and a server with both answer the same JSON, field
//! by field, except what is time or a path.

use std::path::PathBuf;
use std::sync::Arc;

use cobolt_mcp::McpHandler;
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::runner;
use cobolt_project_tools::{web, HeadlessHost, ProjectTools};
use cobolt_web_tests::{gate, summary, Need};
use serde_json::{json, Value};

fn project(tag: &str, shared: Option<Arc<cobolt_project_tools::tools::Shared>>) -> (ProjectTools<HeadlessHost>, PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!("prc-web-tests-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let folder = base.join("Hello");
    let host = HeadlessHost::new(base.clone(), "test");
    let mut tools = match shared {
        Some(s) => ProjectTools::with_shared(host, s),
        None => ProjectTools::new(host),
    };
    let made = tools.call("create_project", &json!({"folder": folder.to_string_lossy(), "name": "Hello", "template": "web"})).unwrap();
    let manifest = folder.join(made["project"].as_str().unwrap());
    tools.call("open_project", &json!({"path": manifest.to_string_lossy()})).unwrap();
    (tools, folder, base)
}

/// Drop what is time, a path or a size of this machine's player, so two answers can be compared.
fn comparable(v: &Value, base: &str) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(k, _)| !["elapsed_ms", "duration_ms", "startup_ms", "bundle_dir"].contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), comparable(v, base)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(|v| comparable(v, base)).collect()),
        Value::String(s) => Value::String(s.replace(base, "<base>")),
        other => other.clone(),
    }
}

#[test]
fn web_tool_sequence() {
    web::link(Arc::new(GeneratorBackend));
    let (mut tools, folder, base) = project("sequence", None);
    let mut passed = 0;
    let form = folder.join("webforms/main-form.wcfrm");
    let original = std::fs::read_to_string(&form).unwrap();

    std::fs::write(&form, original.replacen("</Form>", "  <Control id=\"PLG-1\" type=\"acme:gizmo\" x=\"10\" y=\"260\" w=\"100\" h=\"30\"></Control>\n</Form>", 1)).unwrap();
    let checked = tools.call("check", &json!({})).unwrap();
    assert_eq!(checked["errors"], 1, "{checked}");
    passed += 1;
    std::fs::write(&form, &original).unwrap();
    assert_eq!(tools.call("check", &json!({})).unwrap()["errors"], 0);
    passed += 1;

    let built = tools.call("build", &json!({})).unwrap();
    assert_eq!(built["status"], "built", "{built}");
    assert!(built["report"]["secret_scan"]["scanned_files"].as_u64().unwrap_or(0) > 0, "{built}");
    passed += 1;

    let helper = runner::find_helper();
    if gate("web_tool_sequence", Need::WebView, helper.is_some(), "the helper prc-web-preview is not built (cargo build -p cobolt-web-preview)") {
        std::env::set_var(web::preview::HELPER_ENV, helper.unwrap());
        let r = tools.call_tool("render_form", &json!({"path": "webforms/main-form.wcfrm"}));
        if r.is_error == Some(true) {
            let why = r.content[0].as_text().unwrap_or_default().to_owned();
            let _ = gate("web_tool_sequence", Need::WebView, false, &format!("no web view — {why}"));
            assert!(why.contains("web view"), "{why}");
        } else {
            let image = serde_json::to_value(&r.content[0]).unwrap();
            assert_eq!(image["type"], "image");
            let meta: Value = serde_json::from_str(r.content[1].as_text().unwrap()).unwrap();
            assert_eq!(meta["engine"], cobolt_project_tools::tools::web_form::ENGINE);
            passed += 1;
            let r = tools.call_tool(
                "run_form",
                &json!({"path": "webforms/main-form.wcfrm", "steps": [
                    {"set": {"control": "TXT-NAME", "property": "Text", "value": "Ada"}},
                    {"event": {"control": "BTN-GREET", "name": "onClick"}},
                    {"read": {"control": "LBL-RESULT", "property": "Text"}}
                ]}),
            );
            let answer: Value = serde_json::from_str(r.content.iter().find_map(|c| c.as_text()).unwrap()).unwrap();
            assert_eq!(answer["reads"]["LBL-RESULT::Text"], "Hello, Ada", "{answer}");
            assert_eq!(answer["engine"], cobolt_project_tools::tools::web_form::ENGINE);
            passed += 2;
        }
    }
    println!("web tool sequence (AC8): create_project → check → fix → build (secret scan) → render_form → run_form");
    println!("  {}", summary(passed, 0));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn both_servers_agree() {
    web::link(Arc::new(GeneratorBackend));
    // The IDE's server and `rcrun mcp` inject a renderer and a runner; the other tools do not use them.
    let with_both = Arc::new(
        cobolt_project_tools::tools::Shared::new()
            .with_renderer(Arc::new(|_, _, _| Err("not used here".to_owned())))
            .with_runner(cobolt_project_tools::tools::run::rcrun_runner(PathBuf::from("rcrun"))),
    );
    let (mut ide, _f1, base1) = project("agree-ide", Some(with_both));
    let (mut cli, _f2, base2) = project("agree-cli", None);
    let calls: Vec<(&str, Value)> = vec![
        ("list_files", json!({})),
        ("check", json!({})),
        ("describe", json!({})),
        ("regenerate", json!({})),
        ("build", json!({})),
        ("validate", json!({"path": "webforms/main-form.wcfrm"})),
    ];
    let mut same = 0;
    for (name, args) in &calls {
        let a = ide.call(name, args).map(|v| comparable(&v, &base1.to_string_lossy()));
        let b = cli.call(name, args).map(|v| comparable(&v, &base2.to_string_lossy()));
        // The two projects live in folders named for their server; compare with that name removed too.
        let norm = |v: Result<Value, String>, tag: &str| v.map(|v| Value::String(v.to_string().replace(tag, "<tag>")));
        assert_eq!(norm(a, "agree-ide"), norm(b, "agree-cli"), "the servers differ on `{name}`");
        same += 1;
    }
    println!("both servers agree (AC9): {same} of {} tool answers equal field by field (time and path fields set aside)", calls.len());
    println!("  {}", summary(same, 0));
    let _ = std::fs::remove_dir_all(&base1);
    let _ = std::fs::remove_dir_all(&base2);
}
