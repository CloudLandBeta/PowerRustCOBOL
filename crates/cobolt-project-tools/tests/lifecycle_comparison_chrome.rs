// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The form lifecycle on the desktop host and in a browser, compared** (WASM-016 T17, the lifecycle half; R6, R14).
//!
//! One form whose form-level handlers DISPLAY `EV FORM <event>` for every lifecycle event the desktop catalogue has
//! (`onLoad`, `onShow`, `onActivate`, `onActivated`, `onShown`, `onClose`, `onCloseRejected`, `onRestore`). It is run twice:
//! * **desktop** — the real host and interpreter through `rcrun run-form --headless` (the `run_form` tool): what its program
//!   DISPLAYs from the start until the script has waited a second;
//! * **browser** — the real Build, served, in a headless Chrome: what the page's console shows after the first paint.
//!
//! The start-up, then a click on a Button whose handler is `INVOKE ME::Close()`, on both sides: the whole event list and its order are
//! compared. `onCloseRejected` (a veto needs the OS to ask to close a form that waits on a modal child) and `onRestore` (an OS
//! window state) cannot arise on a page and are listed in [`DOCUMENTED`]. The test fails on any other difference.
//!
//! It needs `target/<profile>/rcrun` (build `cobolt-cli` first) and a Chrome (`CHROME`); without either it says it was skipped.

#[path = "../../cobolt-web-bundle/tests/support/cdp.rs"]
#[allow(dead_code)]
mod cdp;

use std::path::PathBuf;
use std::sync::Arc;

use cobolt_mcp::McpHandler;
use cobolt_project_tools::tools::{run, Shared};
use cobolt_project_tools::web::devserver::{self, ServeOptions};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildRequest};
use cobolt_project_tools::{HeadlessHost, ProjectTools};
use serde_json::{json, Value};

const EVENTS: &[&str] = &["onLoad", "onShow", "onActivate", "onActivated", "onShown", "onClose", "onCloseRejected", "onRestore"];

/// The differences accepted at start-up, by event: (event, reason). Empty means the page matches the desktop.
const DOCUMENTED: &[(&str, &str)] = &[
    ("onRestore", "window states are not carried to the browser (WASM-016 Q4, 2026-10-22): the desktop raises it when the OS window leaves minimized or maximized"),
    ("onCloseRejected", "a close is vetoed only when the OS asks to close a form that is waiting on a modal child; a page has no close request to veto"),
];

fn form_xml(attrs: &str) -> String {
    let events: String = EVENTS
        .iter()
        .map(|e| {
            format!(
                "<Event name=\"{e}\" paragraph=\"MAIN-FORM--{}\"><![CDATA[       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           DISPLAY \"EV FORM {e}\".\n]]></Event>",
                e.to_ascii_uppercase()
            )
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Form name=\"main-form\" title=\"Life\" width=\"400\" height=\"240\" main-form=\"true\" {attrs}>\n  <form-events>{events}</form-events>\n  <Control id=\"LBL-A\" type=\"Label\" x=\"20\" y=\"20\" w=\"200\" h=\"28\"><Property name=\"Caption\">Life</Property></Control>\n  <Control id=\"BTN-X\" type=\"Button\" x=\"20\" y=\"60\" w=\"120\" h=\"30\"><Property name=\"Caption\">Close</Property><Event name=\"onClick\" paragraph=\"BTN-X--ONCLICK\"><![CDATA[       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           INVOKE ME::Close().\n]]></Event></Control>\n</Form>\n"
    )
}

fn rcrun() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
    ["debug", "release"].iter().map(|p| root.join(p).join("rcrun")).find(|p| p.exists())
}

fn desktop(rcrun: PathBuf) -> Vec<String> {
    let base = std::env::temp_dir().join(format!("prc-life-desk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("start")).unwrap();
    let shared = Arc::new(Shared::new().with_runner(run::rcrun_runner(rcrun)));
    let mut tools = ProjectTools::with_shared(HeadlessHost::new(base.join("start/none.project.toml"), "test"), shared);
    let project = base.join("Life");
    let mut call = |name: &str, args: Value| -> Value {
        let r = tools.call_tool(name, &args);
        assert_ne!(r.is_error, Some(true), "{name}: {:?}", r.content.iter().filter_map(|c| c.as_text()).collect::<Vec<_>>());
        r.content.iter().filter_map(|c| c.as_text()).find_map(|t| serde_json::from_str(t).ok()).unwrap_or(Value::Null)
    };
    call("create_project", json!({"folder": project.to_string_lossy(), "name": "Life"}));
    std::fs::write(project.join("forms/main-form.cfrm"), form_xml("")).unwrap();
    call("add_to_project", json!({"path": "forms/main-form.cfrm"}));
    call("regenerate", json!({"path": "forms/main-form.cfrm"}));
    let out = call("run_form", json!({"path": "forms/main-form.cfrm", "time_limit_s": 30, "steps": [{"wait_ms": 1000}, {"event": {"control": "BTN-X", "name": "onClick"}}, {"wait_ms": 1000}]}));
    let _ = std::fs::remove_dir_all(&base);
    out["display"].as_array().into_iter().flatten().filter_map(Value::as_str).filter_map(|l| l.split_once("EV FORM ").map(|(_, e)| e.trim().to_owned())).collect()
}

const MANIFEST: &str = "[project]\nname = \"Life.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
    [files]\nforms = [\"webforms/main-form.wcfrm\"]\n\n[web]\nmulti_memory = true\n";

fn browser(exe: &std::path::Path) -> Vec<String> {
    let dir = std::env::temp_dir().join(format!("prc-life-web-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::write(dir.join("Life.project.toml"), MANIFEST).unwrap();
    std::fs::write(dir.join("webforms/main-form.wcfrm"), form_xml("index-base=\"1\"")).unwrap();
    let req = WebBuildRequest::from_manifest(&dir.join("Life.project.toml")).unwrap();
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
    let mut server = devserver::start(&report.bundle_dir, &ServeOptions { port: 0, isolation: req.web.isolation, live_reload: false }).unwrap();
    let chrome = cdp::Chrome::launch(exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1100, 800).unwrap();
    page.navigate(&server.url()).unwrap_or_else(|e| panic!("{e}"));
    page.wait_for("(() => { const r = document.getElementById('pr-app'); return !!(r && r.shadowRoot && r.shadowRoot.querySelector('[data-name=\"LBL-A\"]')); })()", 30)
        .unwrap_or_else(|e| panic!("the page did not start: {e}; console {:?}; exceptions {:?}", page.console, page.exceptions));
    page.eval("new Promise(r => setTimeout(() => r(true), 1000))").unwrap();
    let start: Vec<String> = page.console.iter().filter_map(|l| l.split_once("EV FORM ").map(|(_, e)| e.trim().to_owned())).collect();
    // The same click as the desktop script's: BTN-X closes the form through its handler.
    page.click_at("(() => { const r = document.getElementById('pr-app').shadowRoot.querySelector('[data-name=\"BTN-X\"]').getBoundingClientRect(); return {x: r.x, y: r.y, width: r.width, height: r.height}; })()").unwrap();
    page.eval("new Promise(r => setTimeout(() => r(true), 1000))").unwrap();
    println!("browser start-up: {start:?}");
    let seen = page.console.iter().filter_map(|l| l.split_once("EV FORM ").map(|(_, e)| e.trim().to_owned())).collect();
    server.stop();
    let _ = std::fs::remove_dir_all(&dir);
    seen
}

#[test]
fn the_start_up_events_on_the_desktop_host_and_in_chrome() {
    let Some(exe) = cdp::find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let Some(rcrun) = rcrun() else {
        println!("SKIPPED: no target/<profile>/rcrun (build cobolt-cli first)");
        return;
    };
    let (desk, web) = (desktop(rcrun), browser(&exe));
    println!("desktop, whole run: {desk:?}\nbrowser, whole run: {web:?}");
    let mut undocumented = Vec::new();
    for e in EVENTS {
        let (d, w) = (desk.iter().filter(|x| x == e).count(), web.iter().filter(|x| x == e).count());
        let note = DOCUMENTED.iter().find(|(n, _)| n == e).map(|(_, why)| *why);
        println!("{e:<16} desktop {d}  browser {w}  {}", if d == w { "equal".to_string() } else { note.map_or("DIFFERENT".to_string(), |w| format!("documented: {w}")) });
        if d != w && note.is_none() {
            undocumented.push(format!("{e}: desktop {d}, browser {w}"));
        }
    }
    assert!(undocumented.is_empty(), "undocumented lifecycle differences: {undocumented:?}");
    assert_eq!(desk, web, "the events come in the same order");
}
