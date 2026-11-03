// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Focus and mouse events on the desktop host and in a browser, compared** (WASM-016 T17, rows 4 and 6 of `differences.md`).
//!
//! Two Buttons whose `onGotFocus`, `onLostFocus`, `onMouseDown`, `onMouseUp` and `onClick` handlers DISPLAY
//! `EV <control> <event>`. A real pointer click on BTN-A, then on BTN-B, on both sides: the desktop through the headless
//! host's `click` step (move, press, release), the page through CDP mouse events. The two event lists are printed and
//! compared; the test fails on any difference not in [`DOCUMENTED`].
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

const EVENTS: &[&str] = &["onGotFocus", "onLostFocus", "onMouseDown", "onMouseUp", "onClick"];
/// The controls clicked, in order: (id, type, y, the events it is watched for).
const CONTROLS: &[(&str, &str, u32, &[&str])] = &[
    ("BTN-A", "Button", 20, EVENTS),
    ("BTN-B", "Button", 70, EVENTS),
    ("SLD-A", "Slider", 120, &["onMouseDown", "onMouseUp", "onChange", "onValueChanged"]),
];

/// The differences accepted, by (control, event): the reason. Empty means the page matches the desktop.
const FOCUS: &str = "a Button takes the keyboard focus on a click in the browser, so the page raises onGotFocus (and the previous control's onLostFocus); the desktop host's Button takes none from the mouse (measured with a real pointer click, 2026-11-03)";
const RANGE: &str = "a browser's range input jumps to the clicked point on the press, so the page raises onChange and onValueChanged; the desktop slider moves only when its handle is dragged (measured with a real pointer click at the track's centre, 2026-11-03)";
const DOCUMENTED: &[(&str, &str)] = &[
    ("BTN-A onGotFocus", FOCUS),
    ("BTN-A onLostFocus", FOCUS),
    ("BTN-B onGotFocus", FOCUS),
    ("BTN-B onLostFocus", FOCUS),
    ("SLD-A onChange", RANGE),
    ("SLD-A onValueChanged", RANGE),
];

fn form_xml(attrs: &str) -> String {
    let button = |id: &str, ty: &str, y: u32, evs: &[&str]| {
        let events: String = evs
            .iter()
            .map(|e| {
                format!(
                    "<Event name=\"{e}\" paragraph=\"{id}--{}\"><![CDATA[       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           DISPLAY \"EV {id} {e}\".\n]]></Event>",
                    e.to_ascii_uppercase()
                )
            })
            .collect();
        format!("<Control id=\"{id}\" type=\"{ty}\" x=\"20\" y=\"{y}\" w=\"220\" h=\"30\"><Property name=\"Caption\">{id}</Property>{events}</Control>")
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Form name=\"main-form\" title=\"Focus\" width=\"400\" height=\"240\" main-form=\"true\" {attrs}>\n  {}\n</Form>\n",
        CONTROLS.iter().map(|(id, ty, y, evs)| button(id, ty, *y, evs)).collect::<Vec<_>>().join("\n  ")
    )
}

fn rcrun() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
    ["debug", "release"].iter().map(|p| root.join(p).join("rcrun")).find(|p| p.exists())
}

fn events_of<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<String> {
    lines.filter_map(|l| l.split_once("EV ").map(|(_, e)| e.trim().to_owned())).collect()
}

fn desktop(rcrun: PathBuf) -> Vec<String> {
    let base = std::env::temp_dir().join(format!("prc-focus-desk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("start")).unwrap();
    let shared = Arc::new(Shared::new().with_runner(run::rcrun_runner(rcrun)));
    let mut tools = ProjectTools::with_shared(HeadlessHost::new(base.join("start/none.project.toml"), "test"), shared);
    let project = base.join("Focus");
    let mut call = |name: &str, args: Value| -> Value {
        let r = tools.call_tool(name, &args);
        assert_ne!(r.is_error, Some(true), "{name}: {:?}", r.content.iter().filter_map(|c| c.as_text()).collect::<Vec<_>>());
        r.content.iter().filter_map(|c| c.as_text()).find_map(|t| serde_json::from_str(t).ok()).unwrap_or(Value::Null)
    };
    call("create_project", json!({"folder": project.to_string_lossy(), "name": "Focus"}));
    std::fs::write(project.join("forms/main-form.cfrm"), form_xml("")).unwrap();
    call("add_to_project", json!({"path": "forms/main-form.cfrm"}));
    call("regenerate", json!({"path": "forms/main-form.cfrm"}));
    let out = call(
        "run_form",
        json!({"path": "forms/main-form.cfrm", "time_limit_s": 30, "steps": [
            {"wait_ms": 1000}, {"click": {"control": "BTN-A"}}, {"wait_ms": 300}, {"click": {"control": "BTN-B"}}, {"wait_ms": 300}, {"click": {"control": "SLD-A"}}, {"wait_ms": 500}]}),
    );
    let _ = std::fs::remove_dir_all(&base);
    println!("desktop steps: {}", out["steps"]);
    events_of(out["display"].as_array().into_iter().flatten().filter_map(Value::as_str))
}

const MANIFEST: &str = "[project]\nname = \"Focus.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
    [files]\nforms = [\"webforms/main-form.wcfrm\"]\n\n[web]\nmulti_memory = true\n";

fn browser(exe: &std::path::Path) -> Vec<String> {
    let dir = std::env::temp_dir().join(format!("prc-focus-web-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::write(dir.join("Focus.project.toml"), MANIFEST).unwrap();
    std::fs::write(dir.join("webforms/main-form.wcfrm"), form_xml("index-base=\"1\"")).unwrap();
    let req = WebBuildRequest::from_manifest(&dir.join("Focus.project.toml")).unwrap();
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
    let mut server = devserver::start(&report.bundle_dir, &ServeOptions { port: 0, isolation: req.web.isolation, live_reload: false }).unwrap();
    let chrome = cdp::Chrome::launch(exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1100, 800).unwrap();
    page.navigate(&server.url()).unwrap_or_else(|e| panic!("{e}"));
    page.wait_for("(() => { const r = document.getElementById('pr-app'); return !!(r && r.shadowRoot && r.shadowRoot.querySelector('[data-name=\"BTN-A\"]')); })()", 30)
        .unwrap_or_else(|e| panic!("the page did not start: {e}; console {:?}; exceptions {:?}", page.console, page.exceptions));
    let pause = |page: &mut cdp::Page, ms: u32| {
        page.eval(&format!("new Promise(r => setTimeout(() => r(true), {ms}))")).unwrap();
    };
    pause(&mut page, 1000);
    let rect = |id: &str| format!("(() => {{ const r = document.getElementById('pr-app').shadowRoot.querySelector('[data-name=\"{id}\"]').getBoundingClientRect(); return {{x: r.x, y: r.y, width: r.width, height: r.height}}; }})()");
    for (id, ..) in CONTROLS {
        page.click_at(&rect(id)).unwrap();
        pause(&mut page, 300);
    }
    pause(&mut page, 200);
    let seen = events_of(page.console.iter().map(String::as_str));
    server.stop();
    let _ = std::fs::remove_dir_all(&dir);
    seen
}

#[test]
fn focus_and_mouse_events_on_the_desktop_host_and_in_chrome() {
    let Some(exe) = cdp::find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let Some(rcrun) = rcrun() else {
        println!("SKIPPED: no target/<profile>/rcrun (build cobolt-cli first)");
        return;
    };
    let (desk, web) = (desktop(rcrun), browser(&exe));
    println!("desktop: {desk:?}\nbrowser: {web:?}");
    let mut undocumented = Vec::new();
    for (id, _, _, evs) in CONTROLS {
        for e in *evs {
            let key = format!("{id} {e}");
            let (d, w) = (desk.iter().filter(|x| **x == key).count(), web.iter().filter(|x| **x == key).count());
            let note = DOCUMENTED.iter().find(|(n, _)| *n == key).map(|(_, why)| *why);
            println!("{key:<20} desktop {d}  browser {w}  {}", if d == w { "equal".to_string() } else { note.map_or("DIFFERENT".to_string(), |w| format!("documented: {w}")) });
            if d != w && note.is_none() {
                undocumented.push(format!("{key}: desktop {d}, browser {w}"));
            }
        }
    }
    assert!(undocumented.is_empty(), "undocumented differences: {undocumented:?}");
    // The events both sides raise come in the same order.
    let both = |v: &[String]| v.iter().filter(|x| !DOCUMENTED.iter().any(|(n, _)| n == x)).cloned().collect::<Vec<_>>();
    assert_eq!(both(&desk), both(&web), "the events come in the same order");
}
