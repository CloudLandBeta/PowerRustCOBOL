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
            png = Some(unbase64(v["data"].as_str().unwrap()));
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

const HOST_MAIN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="HOST-MAIN" title="Host" width="480" height="300">
  <Control id="BTN-OPEN" type="Button" x="20" y="20" w="160" h="32" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">Assistant</Property>
    <Event name="onClick" paragraph="BTN-OPEN--ONCLICK"><![CDATA[       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           INVOKE ME::"OpenFormAsync"("APP-SHELL").
]]></Event>
  </Control>
</Form>
"#;

const APP_SHELL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="APP-SHELL" title="Assistant" width="640" height="420" form-format="Both">
  <form-events>
    <Event name="onShow" paragraph="APP-SHELL--ONSHOW"><![CDATA[       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-ITEM PIC X(10).
       PROCEDURE DIVISION.
           MOVE SideMenu-1::ActivateItem("pg1") TO WS-ITEM.
           DISPLAY "SHELL-SHOWN".
]]></Event>
  </form-events>
  <Control id="SideMenu-1" type="SideMenu" x="0" y="0" w="200" h="420" tab-order="0" z-order="0" visible="true" enabled="true">
  </Control>
  <Control id="LBL-HOME" type="Label" x="240" y="40" w="200" h="28" tab-order="1" z-order="1" visible="true" enabled="true">
    <Property name="Caption">Home</Property>
  </Control>
</Form>
"#;

const PAGE_ONE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="PAGE-ONE" title="Topics" width="440" height="420" background="1E2430FF" form-format="Embedded">
  <Control id="LBL-TOPICS" type="Label" x="20" y="20" w="200" h="28" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">Topics</Property>
  </Control>
</Form>
"#;

/// Spec 085 AC2 (R1, R2, R4) — through the real `rcrun`: a main form opens a
/// form that carries its own SideMenu as a window; that window runs as a shell
/// of its own, and its menu — read from its own sidecar — loads a form into
/// ITS ContentPane, not the main window's.
#[test]
fn a_side_menu_form_opened_as_a_window_loads_its_menu_forms_into_its_own_pane() {
    let base = std::env::temp_dir().join(format!("prc-085-child-shell-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let start = base.join("start");
    std::fs::create_dir_all(&start).unwrap();
    let runner = run::rcrun_runner(Path::new(env!("CARGO_BIN_EXE_rcrun")).to_path_buf());
    let shared = Arc::new(Shared::new().with_runner(runner));
    let mut tools = ProjectTools::with_shared(HeadlessHost::new(start.join("none.project.toml"), "test"), shared);
    let project = base.join("Host");
    let (_, made, err) = call(&mut tools, "create_project", json!({"folder": project.to_string_lossy(), "name": "Host"}));
    assert!(!err, "{made}");
    for (file, xml) in [("host-main", HOST_MAIN), ("app-shell", APP_SHELL), ("page-one", PAGE_ONE)] {
        std::fs::write(project.join(format!("forms/{file}.cfrm")), xml).unwrap();
        let (_, out, err) = call(&mut tools, "add_to_project", json!({"path": format!("forms/{file}.cfrm")}));
        assert!(!err, "{file}: {out}");
        let (_, out, err) = call(&mut tools, "regenerate", json!({"path": format!("forms/{file}.cfrm")}));
        assert!(!err, "{file}: {out}");
    }
    let mut item = cobolt_forms::menu::MenuItem::new_action("pg1", "Topics");
    item.action = Some("open-form:page-one".into());
    let def = cobolt_forms::menu::MenuDefinition { menu: vec![item], hash: String::new() };
    cobolt_forms::menu::save_menu(&cobolt_forms::menu::menu_yaml_path(&project.join("forms"), "SideMenu-1"), &def).unwrap();

    let (png, out, err) = call(
        &mut tools,
        "run_form",
        json!({"path": "forms/host-main.cfrm", "time_limit_s": 20, "steps": [
            {"event": {"control": "BTN-OPEN", "name": "onClick"}},
            {"wait_ms": 1500}
        ]}),
    );
    assert!(!err, "{out}");
    let windows = out["windows"].as_array().cloned().unwrap_or_default();
    assert_eq!(windows.len(), 1, "the Assistant window is open: {out}");
    assert_eq!(windows[0]["form"], "APP-SHELL");
    assert_eq!(windows[0]["shell"], true, "it runs as a shell of its own: {out}");
    assert_eq!(windows[0]["on_pane"], "PAGE-ONE", "its menu loaded Topics into its own pane: {out}");
    assert!(out["on_pane"].is_null(), "nothing loaded into the main window: {out}");
    assert!(out["display"].as_array().unwrap().iter().any(|l| l.as_str().unwrap_or("").contains("SHELL-SHOWN")), "{out}");
    let png = png.expect("a picture");
    std::fs::write(std::env::temp_dir().join("prc-085-child-shell.png"), &png).unwrap();
    let _ = std::fs::remove_dir_all(&base);
    println!("child shell via rcrun: APP-SHELL opened as a shell window, its menu loaded PAGE-ONE into its own pane");
}

/// Spec 085 AC5 — PowerChat added with `add_powerchat`, opened from the host
/// through the real `rcrun`: it comes up as a window of its own running as a
/// shell (its own side menu and pane), in the host's Spatial look and name.
#[test]
fn powerchat_added_to_an_application_opens_as_a_shell_window_of_its_own() {
    let base = std::env::temp_dir().join(format!("prc-085-powerchat-open-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let start = base.join("start");
    std::fs::create_dir_all(&start).unwrap();
    let runner = run::rcrun_runner(Path::new(env!("CARGO_BIN_EXE_rcrun")).to_path_buf());
    let shared = Arc::new(Shared::new().with_runner(runner));
    let mut tools = ProjectTools::with_shared(HeadlessHost::new(start.join("none.project.toml"), "test"), shared);
    let project = base.join("Inventory");
    let (_, made, err) = call(&mut tools, "create_project", json!({"folder": project.to_string_lossy(), "name": "Inventory"}));
    assert!(!err, "{made}");
    let host_main = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="HOST-MAIN" title="Inventory Pro" width="480" height="300" main-form="true">
  <Control id="BTN-AI" type="Button" x="20" y="20" w="160" h="32" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">Assistant</Property>
    <Event name="onClick" paragraph="BTN-AI--ONCLICK"><![CDATA[       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           INVOKE ME::"OpenFormAsync"("CHAT-FORM").
]]></Event>
  </Control>
</Form>
"#;
    std::fs::write(project.join("forms/host-main.cfrm"), host_main).unwrap();
    for tool in ["add_to_project", "regenerate"] {
        let (_, out, err) = call(&mut tools, tool, json!({"path": "forms/host-main.cfrm"}));
        assert!(!err, "{tool}: {out}");
    }
    let (_, added, err) = call(&mut tools, "add_powerchat", json!({}));
    assert!(!err, "{added}");
    assert_eq!(added["check"]["errors"], 0, "{added}");
    assert_eq!(added["opens_with"]["cobol"], "INVOKE ME::\"OpenFormAsync\"(\"CHAT-FORM\")", "no side menu: the COBOL to open it");

    let (png, out, err) = call(
        &mut tools,
        "run_form",
        json!({"path": "forms/host-main.cfrm", "time_limit_s": 30, "steps": [
            {"event": {"control": "BTN-AI", "name": "onClick"}},
            {"wait_ms": 2500}
        ]}),
    );
    assert!(!err, "{out}");
    let windows = out["windows"].as_array().cloned().unwrap_or_default();
    assert_eq!(windows.len(), 1, "PowerChat's window is open: {out}");
    assert_eq!(windows[0]["form"], "CHAT-FORM");
    assert_eq!(windows[0]["shell"], true, "running as a shell of its own: {out}");
    assert!(out.get("runtime_error").is_none(), "{out}");
    std::fs::write(std::env::temp_dir().join("prc-085-powerchat-open.png"), png.expect("a picture")).unwrap();
    let _ = std::fs::remove_dir_all(&base);
    println!("powerchat in a host: CHAT-FORM opened as a shell window; on its pane: {}", windows[0]["on_pane"]);
}
