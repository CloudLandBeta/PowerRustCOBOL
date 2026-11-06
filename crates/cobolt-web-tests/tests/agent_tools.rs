// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The agents' tools on a web project** (WASM-015 T16, R10, R18, AC8's check half): `check` refuses what a
//! browser cannot have, naming file and line, and the web `build` says what its secret scan read.

use std::path::PathBuf;
use std::sync::Arc;

use cobolt_project_tools::web;
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::{HeadlessHost, ProjectTools};
use serde_json::{json, Value};

fn web_project(tag: &str) -> (ProjectTools<HeadlessHost>, PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!("prc-web-tests-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let folder = base.join("Hello");
    let mut tools = ProjectTools::new(HeadlessHost::new(base.clone(), "test"));
    let made = tools.call("create_project", &json!({"folder": folder.to_string_lossy(), "name": "Hello", "template": "web"})).unwrap();
    let manifest = folder.join(made["project"].as_str().unwrap());
    tools.call("open_project", &json!({"path": manifest.to_string_lossy()})).unwrap();
    web::link(Arc::new(GeneratorBackend));
    (tools, folder, base)
}

/// `check` on a web form with a plug-in control: an error naming the form file, the line and the control; the
/// project is not edited (Check reports, never fixes); removing the control makes the project clean again.
#[test]
fn check_refuses_a_plug_in_control_with_file_and_line_and_changes_nothing() {
    let (mut tools, folder, base) = web_project("check");
    let form = folder.join("webforms/main-form.wcfrm");
    let original = std::fs::read_to_string(&form).unwrap();
    let with_plug_in = original.replacen("</Form>", "  <Control id=\"PLG-1\" type=\"acme:gizmo\" x=\"10\" y=\"260\" w=\"100\" h=\"30\"></Control>\n</Form>", 1);
    assert_ne!(with_plug_in, original, "the template's form ends with </Form>");
    std::fs::write(&form, &with_plug_in).unwrap();
    let checked: Value = tools.call("check", &json!({})).unwrap();
    let errors = checked["errors"].as_u64().unwrap();
    let text = checked.to_string();
    assert!(errors >= 1, "{checked}");
    assert!(text.contains("PLG-1") && text.contains("plug-in controls are not available on the web"), "{checked}");
    let finding = checked["diagnostics"].as_array().unwrap().iter().find(|d| d["severity"] == "error").expect("an error finding");
    assert_eq!(finding["file"], "webforms/main-form.wcfrm", "{finding}");
    assert!(finding["line"].as_u64().unwrap_or(0) > 0, "the finding names the line of the control: {finding}");
    assert_eq!(std::fs::read_to_string(&form).unwrap(), with_plug_in, "Check reports, it never edits the developer's file");

    std::fs::write(&form, &original).unwrap();
    let clean: Value = tools.call("check", &json!({})).unwrap();
    assert_eq!(clean["errors"], 0, "{clean}");
    println!("check: plug-in control → {errors} error(s) naming webforms/main-form.wcfrm and PLG-1; form untouched; removed → 0 errors");
    println!("→ 4 passed, 0 failed");
    let _ = std::fs::remove_dir_all(&base);
}

/// `build` answers `secret_scan`: the files it read (more than none) and what it found (nothing, for the template).
#[test]
fn build_reports_what_its_secret_scan_read() {
    let (mut tools, _folder, base) = web_project("scan");
    let built: Value = tools.call("build", &json!({})).unwrap();
    assert_eq!(built["status"], "built", "{built}");
    let scan = &built["report"]["secret_scan"];
    let files = scan["scanned_files"].as_u64().unwrap_or(0);
    assert!(files > 0, "{built}");
    assert_eq!(scan["findings"], json!([]), "the template holds no secret: {scan}");
    println!("build: secret scan read {files} files, {} findings", scan["findings"].as_array().map_or(0, Vec::len));
    println!("→ 3 passed, 0 failed");
    let _ = std::fs::remove_dir_all(&base);
}
