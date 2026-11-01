// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Form layers end to end** (WASM-004 T31: AC9–AC13). A Web project's layered form is built by the real Build —
//! the handlers are COBOL, `MOVE 1 TO LYR-SEE::Visible` shows a layer through the generated module (WASM-010 T12) —
//! served, and driven in a headless Chrome by the pointer and the keyboard, as a user would:
//!
//! - AC9: a layer's button at `ZOrder` 0 is above a base button at `ZOrder` 209 at the same point;
//! - AC10: every layer starts hidden (not drawn, no pointer, no focus) and a click whose handler sets `Visible`
//!   shows it on the next frame;
//! - AC11: a click at a translucent layer's point reaches the base control under it; at the opaque layer's
//!   backdrop it is discarded;
//! - AC12: Tab walks the base in its tab order, then the shown layer in its own;
//! - AC13: a DataGrid at the form's rounded corner is clipped to the arc.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

#[path = "../../cobolt-web-bundle/tests/support/cdp.rs"]
#[allow(dead_code)]
mod cdp;

use cobolt_project_tools::web::devserver::{self, ServeOptions};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildRequest};

const MANIFEST: &str = "[project]\nname = \"Layers.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
    [files]\nforms = [\"webforms/main-form.wcfrm\"]\n\n[web]\nmulti_memory = true\n";

fn handler(control: &str, body: &str) -> String {
    format!(
        "<Event name=\"onClick\" paragraph=\"{control}--ONCLICK\"><![CDATA[       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n{body}]]></Event>"
    )
}

fn form() -> String {
    let base = handler("BTN-BASE", "           MOVE \"base clicked\" TO LBL-OUT::Caption.\n");
    let see = handler("BTN-SEE", "           MOVE 1 TO LYR-SEE::Visible.\n           MOVE \"see shown\" TO LBL-OUT::Caption.\n");
    let top = handler("BTN-TOP", "           MOVE 1 TO LYR-TOP::Visible.\n           MOVE \"top shown\" TO LBL-OUT::Caption.\n");
    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Layers" width="900" height="600" index-base="1" corner-radius="24" main-form="true">
  <Layer name="LYR-SEE" color="#10203080"/>
  <Layer name="LYR-TOP" color="#203040"/>
  <Control id="BTN-BASE" type="Button" x="300" y="100" w="160" h="60" z-order="209" tab-order="2"><Property name="Caption">Base</Property>{base}</Control>
  <Control id="BTN-SEE" type="Button" x="50" y="300" w="100" h="30" tab-order="1"><Property name="Caption">See</Property>{see}</Control>
  <Control id="LBL-OUT" type="Label" x="600" y="20" w="250" h="24"><Property name="Caption">ready</Property></Control>
  <Control id="BTN-TOP" type="Button" x="380" y="130" w="160" h="60" z-order="0" tab-order="1" layer="LYR-SEE"><Property name="Caption">Top</Property>{top}</Control>
  <Control id="DG-CORNER" type="DataGrid" x="0" y="500" w="200" h="100" tab-order="2" layer="LYR-SEE"></Control>
  <Control id="TXT-T2" type="TextBox" x="600" y="300" w="150" h="28" tab-order="2" layer="LYR-TOP"></Control>
  <Control id="TXT-T1" type="TextBox" x="600" y="250" w="150" h="28" tab-order="1" layer="LYR-TOP"></Control>
</Form>
"##
    )
}

const LIB: &str = "const root = document.getElementById('pr-app').shadowRoot; const el = id => root.querySelector('[data-name=\"' + id + '\"]'); \
    const form = root.querySelector('[data-pr-form=\"main-form\"]'); const layer = n => form.querySelector(':scope > .pr-form-layer[data-layer=\"' + n + '\"]'); \
    const at = (x, y) => { const e = root.elementFromPoint(x, y); const c = e && e.closest('[data-name]'); return e ? (c ? c.dataset.name : e.className) : null; }; \
    const rect = id => { const r = el(id).getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; }; \
    const caption = () => el('LBL-OUT').textContent; \
    const state = n => { const l = layer(n); const s = getComputedStyle(l); return { hidden: l.hasAttribute('hidden'), inert: l.hasAttribute('inert'), display: s.display }; };";

fn eval(page: &mut cdp::Page, body: &str) -> serde_json::Value {
    page.eval(&format!("(() => {{ {LIB} {body} }})()")).unwrap_or_else(|e| panic!("{e}; console {:?}; exceptions {:?}", page.console, page.exceptions))
}

fn wait(page: &mut cdp::Page, cond: &str, what: &str) {
    page.wait_for(&format!("(() => {{ {LIB} return {cond}; }})()"), 15)
        .unwrap_or_else(|e| panic!("{what}: {e}; console {:?}; exceptions {:?}", page.console, page.exceptions));
}

/// The focused control's name as Tab leaves it.
fn tab_walk(page: &mut cdp::Page, steps: usize) -> Vec<String> {
    // A click on empty page space moves the browser's focus starting point back to the start of the page.
    page.click_at("({ x: 1090, y: 690, width: 0, height: 0 })").unwrap();
    let mut seen: Vec<String> = Vec::new();
    for _ in 0..steps {
        page.press_key("Tab").unwrap();
        let v = eval(page, "const a = root.activeElement; const c = a && a.closest('[data-name]'); return c ? c.dataset.name : (a ? a.tagName : null);");
        let name = v.as_str().unwrap_or("-").to_owned();
        // A control with more than one stop (a DataGrid's cells) counts once.
        if seen.last() != Some(&name) {
            seen.push(name);
        }
    }
    seen
}

#[test]
fn layers_end_to_end_from_cobol_in_chrome() {
    let Some(exe) = cdp::find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let dir = std::env::temp_dir().join(format!("prc-layers-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::write(dir.join("Layers.project.toml"), MANIFEST).unwrap();
    std::fs::write(dir.join("webforms/main-form.wcfrm"), form()).unwrap();
    let req = WebBuildRequest::from_manifest(&dir.join("Layers.project.toml")).unwrap();
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
    let mut server = devserver::start(&report.bundle_dir, &ServeOptions { port: 0, isolation: req.web.isolation, live_reload: false }).unwrap();
    let chrome = cdp::Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1100, 700).unwrap();
    page.navigate(&server.url()).unwrap_or_else(|e| panic!("{e}"));
    wait(&mut page, "!!el('LBL-OUT') && caption() === 'ready'", "the application did not start");

    // ── AC10: every layer starts hidden, takes no pointer and no focus ──
    let start = eval(&mut page, "const b = rect('BTN-BASE'); return { see: state('LYR-SEE'), top: state('LYR-TOP'), at_top: at(b.x + 120, b.y + 45) };");
    println!("at start: {start}");
    for l in ["see", "top"] {
        assert_eq!(start[l]["display"], "none", "{l}: not drawn");
        assert_eq!(start[l]["inert"], true, "{l}: no focus, no pointer");
    }
    assert_eq!(start["at_top"], "BTN-BASE", "the pointer over a hidden layer's button reaches the base");
    let walk0 = tab_walk(&mut page, 3);
    println!("Tab with no layer shown: {walk0:?}");
    assert_eq!(&walk0[..2], &["BTN-SEE", "BTN-BASE"], "the base in its tab order, no layer control");
    assert!(!walk0.iter().any(|n| n == "BTN-TOP" || n == "DG-CORNER" || n.starts_with("TXT-T")));

    // ── AC10: a COBOL handler sets Visible; the next frame shows the layer ──
    page.click_at(&format!("(() => {{ {LIB} return rect('BTN-SEE'); }})()")).unwrap();
    wait(&mut page, "caption() === 'see shown' && !layer('LYR-SEE').hasAttribute('hidden')", "MOVE 1 TO LYR-SEE::Visible did not show the layer");
    let shown = eval(&mut page, "return { see: state('LYR-SEE'), top: state('LYR-TOP') };");
    println!("after BTN-SEE's handler: {shown}");
    assert_eq!(shown["see"]["display"], "block");
    assert_eq!(shown["see"]["inert"], false);
    assert_eq!(shown["top"]["display"], "none", "the other layer stays hidden");

    // ── AC9: the layer's button at ZOrder 0 over the base's at ZOrder 209 ──
    let overlap = eval(&mut page, "const a = rect('BTN-BASE'), b = rect('BTN-TOP'); const x = (Math.max(a.x, b.x) + Math.min(a.x + a.width, b.x + b.width)) / 2, y = (Math.max(a.y, b.y) + Math.min(a.y + a.height, b.y + b.height)) / 2; return { x, y, hit: at(x, y), base_z: getComputedStyle(el('BTN-BASE')).zIndex, top_z: getComputedStyle(el('BTN-TOP')).zIndex };");
    println!("AC9 overlap: {overlap}");
    assert_eq!(overlap["hit"], "BTN-TOP", "a layer control with ZOrder 0 paints above a base control with ZOrder 209");

    // ── AC11: a click at a translucent point of the layer reaches the base button under it ──
    let base_only = eval(&mut page, "const a = rect('BTN-BASE'); return { x: a.x + 15, y: a.y + 15, hit: at(a.x + 15, a.y + 15) };");
    assert_eq!(base_only["hit"], "BTN-BASE", "{base_only}");
    page.click_at(&format!("({{ x: {}, y: {}, width: 0, height: 0 }})", base_only["x"], base_only["y"])).unwrap();
    wait(&mut page, "caption() === 'base clicked'", "the click through the translucent layer did not reach BTN-BASE");
    println!("AC11 translucent: a click at ({}, {}) ran BTN-BASE's handler", base_only["x"], base_only["y"]);

    // ── AC12: Tab walks the base, then the shown layer, each in its own order ──
    let walk1 = tab_walk(&mut page, 6);
    println!("Tab with LYR-SEE shown: {walk1:?}");
    assert_eq!(&walk1[..4], &["BTN-SEE", "BTN-BASE", "BTN-TOP", "DG-CORNER"], "base (1, 2), then LYR-SEE (1, 2)");

    // ── AC13: the DataGrid at the rounded corner is clipped to the arc ──
    let geo = eval(&mut page, "const f = form.getBoundingClientRect(); const g = rect('DG-CORNER'); return { fx: f.x, fy: f.y, fh: f.height, radius: getComputedStyle(form).borderBottomLeftRadius, gx: g.x, gy: g.y };");
    let (fx, fy, fh) = (geo["fx"].as_f64().unwrap(), geo["fy"].as_f64().unwrap(), geo["fh"].as_f64().unwrap());
    let corner = page.capture(fx, fy + fh - 4.0, 4.0, 4.0).unwrap();
    let outside = page.capture(fx - 4.0, fy + fh - 4.0, 4.0, 4.0).unwrap();
    let inside = page.capture(fx + 40.0, fy + fh - 40.0, 1.0, 1.0).unwrap();
    println!("AC13 radius {}: pixel at the corner {:?}, beside the form {:?}, inside the grid {:?}", geo["radius"], corner.2[12], outside.2[12], inside.2[0]);
    assert_eq!(geo["radius"], "24px");
    assert_eq!(corner.2[12], outside.2[12], "outside the arc the page shows what is behind the form, not the grid");
    assert_ne!(inside.2[0], outside.2[12], "inside the arc the grid is drawn");

    // ── AC10, AC11: the opaque layer, shown by its handler, takes a click at its backdrop and discards it ──
    page.click_at(&format!("(() => {{ {LIB} return rect('BTN-TOP'); }})()")).unwrap();
    wait(&mut page, "caption() === 'top shown' && !layer('LYR-TOP').hasAttribute('hidden')", "MOVE 1 TO LYR-TOP::Visible did not show the layer");
    let opaque = eval(&mut page, "const a = rect('BTN-BASE'); return { x: a.x + 15, y: a.y + 15, hit: at(a.x + 15, a.y + 15), bg: getComputedStyle(layer('LYR-TOP').querySelector(':scope > .pr-layer-bg')).pointerEvents };");
    println!("AC11 opaque: {opaque}");
    assert_eq!(opaque["hit"], "pr-layer-bg", "at an opaque point the backdrop is hit");
    page.click_at(&format!("({{ x: {}, y: {}, width: 0, height: 0 }})", opaque["x"], opaque["y"])).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(400));
    let after = eval(&mut page, "return caption();");
    assert_eq!(after, "top shown", "the click at the opaque backdrop reached no handler");

    // ── AC12 with the opaque layer shown: the shield (plan §1.4 working assumption) leaves only that layer ──
    let walk2 = tab_walk(&mut page, 3);
    println!("Tab with LYR-TOP (opaque) shown: {walk2:?}");
    assert_eq!(&walk2[..2], &["TXT-T1", "TXT-T2"], "the opaque layer in its own tab order (TXT-T2 is written first in the file)");
    assert!(!walk2.iter().any(|n| n.starts_with("BTN-")), "the base and the layer below are inert under an opaque layer");

    drop(page);
    server.stop();
    let _ = std::fs::remove_dir_all(&dir);
}
