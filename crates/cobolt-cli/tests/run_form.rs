// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 084 AC18 (R31): `run_form` end to end, through the real `rcrun` — a
//! project made by `create_project`, a form whose button moves what was typed
//! into a label and DISPLAYs, run off screen by a script; and a handler that
//! never returns, stopped by the time limit.

use std::path::Path;
use std::sync::Arc;

use cobolt_mcp::McpHandler;
use cobolt_project_tools::tools::{run, Shared};
use cobolt_project_tools::{HeadlessHost, ProjectTools};
use serde_json::{json, Value};

const FORM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="GREET-FORM" title="Greet" width="360" height="200">
  <Control id="TXT-NAME" type="TextBox" x="20" y="20" w="200" h="28" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Text"></Property>
  </Control>
  <Control id="BTN-GO" type="Button" x="240" y="20" w="90" h="28" tab-order="1" z-order="1" visible="true" enabled="true">
    <Property name="Caption">Go</Property>
    <Event name="onClick" paragraph="BTN-GO--ONCLICK"><![CDATA[       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE TXT-NAME::Text TO LBL-OUT::Caption.
           DISPLAY "GREETED".
]]></Event>
  </Control>
  <Control id="BTN-LOOP" type="Button" x="240" y="60" w="90" h="28" tab-order="2" z-order="2" visible="true" enabled="true">
    <Property name="Caption">Loop</Property>
    <Event name="onClick" paragraph="BTN-LOOP--ONCLICK"><![CDATA[       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           PERFORM UNTIL 1 = 2
               CONTINUE
           END-PERFORM.
]]></Event>
  </Control>
  <Control id="LBL-OUT" type="Label" x="20" y="110" w="300" h="28" tab-order="3" z-order="3" visible="true" enabled="true">
    <Property name="Caption">-</Property>
  </Control>
</Form>
"#;

fn call(tools: &mut ProjectTools<HeadlessHost>, name: &str, args: Value) -> (Option<Vec<u8>>, Value, bool) {
    let r = tools.call_tool(name, &args);
    let mut png = None;
    let mut meta = Value::Null;
    for c in &r.content {
        let v = serde_json::to_value(c).unwrap();
        if v["type"] == "image" {
            png = Some(v["data"].as_str().unwrap().len().to_le_bytes().to_vec());
        } else if let Some(t) = c.as_text() {
            meta = serde_json::from_str(t).unwrap_or(Value::String(t.to_owned()));
        }
    }
    (png, meta, r.is_error == Some(true))
}

#[test]
fn run_form_types_clicks_reads_back_and_stops_a_runaway_handler() {
    let base = std::env::temp_dir().join(format!("prc-084-run-form-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let start = base.join("start");
    std::fs::create_dir_all(&start).unwrap();
    let runner = run::rcrun_runner(Path::new(env!("CARGO_BIN_EXE_rcrun")).to_path_buf());
    let shared = Arc::new(Shared::new().with_runner(runner));
    let mut tools = ProjectTools::with_shared(HeadlessHost::new(start.join("none.project.toml"), "test"), shared);

    let project = base.join("Greeter");
    let (_, made, err) = call(&mut tools, "create_project", json!({"folder": project.to_string_lossy(), "name": "Greeter"}));
    assert!(!err, "{made}");
    std::fs::write(project.join("forms/GREET-FORM.cfrm"), FORM).unwrap();
    let (_, added, err) = call(&mut tools, "add_to_project", json!({"path": "forms/GREET-FORM.cfrm"}));
    assert!(!err, "{added}");

    // Type, click, read back.
    let (png, out, err) = call(
        &mut tools,
        "run_form",
        json!({"path": "forms/GREET-FORM.cfrm", "steps": [
            {"set": {"control": "TXT-NAME", "property": "Text", "value": "Ann"}},
            {"event": {"control": "BTN-GO", "name": "onClick"}},
            {"read": {"control": "LBL-OUT", "property": "Caption"}}
        ]}),
    );
    assert!(!err, "{out}");
    assert_eq!(out["reads"]["LBL-OUT::Caption"], "Ann", "the handler moved the typed text: {out}");
    assert!(out["display"].as_array().unwrap().iter().any(|l| l.as_str().unwrap_or("").contains("GREETED")), "DISPLAY captured: {out}");
    assert_eq!(out["timed_out"], false, "{out}");
    assert_eq!(out["project"], "Greeter.project.toml");
    assert!(png.is_some(), "a picture of the final state: {out}");

    // A handler that never returns: the run is stopped at the limit.
    let started = std::time::Instant::now();
    let (_, out, err) = call(
        &mut tools,
        "run_form",
        json!({"path": "forms/GREET-FORM.cfrm", "time_limit_s": 3, "steps": [
            {"event": {"control": "BTN-LOOP", "name": "onClick"}},
            {"read": {"control": "LBL-OUT", "property": "Caption"}}
        ]}),
    );
    let took = started.elapsed();
    assert!(!err, "{out}");
    assert_eq!(out["timed_out"], true, "{out}");
    assert_eq!(out["steps"][1]["done"], false, "the step after the limit is not run: {out}");
    assert!(took.as_secs() < 20, "stopped promptly: {took:?}");

    let _ = std::fs::remove_dir_all(&base);
    println!("run_form: typed Ann, clicked, read Caption=Ann, DISPLAY captured, picture; runaway handler stopped in {took:?}");
}

/// Standard base64 back to bytes (MCP image content).
fn unbase64(text: &str) -> Vec<u8> {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in text.bytes().filter(|c| *c != b'=') {
        acc = (acc << 6) | ABC.iter().position(|a| *a == c).unwrap() as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap().flatten() {
        let to = dst.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), to).unwrap();
        }
    }
}

/// Spec 084 AC17, shell half (R30): a form PowerDemo3 loads into its
/// ContentPane, pictured inside the application shell — wider than the form
/// by the side menu — and a non-shell request refused.
#[test]
fn render_form_in_shell_pictures_a_contentpane_form_beside_the_side_menu() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let project = std::env::temp_dir().join(format!("prc-084-in-shell-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    copy_dir(&repo.join("examples/PowerDemo3"), &project);
    let runner = run::rcrun_runner(Path::new(env!("CARGO_BIN_EXE_rcrun")).to_path_buf());
    let shared = Arc::new(Shared::new().with_runner(runner));
    let mut tools = ProjectTools::with_shared(HeadlessHost::new(&project, "test"), shared);

    let r = tools.call_tool("render_form", &json!({"path": "forms/Common/buttons-form.cfrm", "in_shell": true}));
    let meta: Value = serde_json::from_str(r.content.last().unwrap().as_text().unwrap()).unwrap_or(Value::Null);
    assert_eq!(r.is_error, None, "{meta}");
    let image = serde_json::to_value(&r.content[0]).unwrap();
    assert_eq!(image["type"], "image");
    let png = unbase64(image["data"].as_str().unwrap());
    std::fs::write(std::env::temp_dir().join("prc-084-in-shell.png"), &png).unwrap();
    let img = image::load_from_memory(&png).unwrap();
    let form = cobolt_forms::load_form(&project.join("forms/Common/buttons-form.cfrm")).unwrap();
    assert!(img.width() > form.width, "the shell is wider than the form by its side menu: {} vs {}", img.width(), form.width);
    assert_eq!(meta["shell"], "forms/sidebar-form.cfrm", "{meta}");

    // The main form itself is not "inside" its own shell.
    let r = tools.call_tool("render_form", &json!({"path": "forms/sidebar-form.cfrm", "in_shell": true}));
    assert_eq!(r.is_error, Some(true));
    let _ = std::fs::remove_dir_all(&project);
    println!("render_form in_shell: buttons-form in sidebar-form's ContentPane, {}x{} (form {}x{})", img.width(), img.height(), form.width, form.height);
}
