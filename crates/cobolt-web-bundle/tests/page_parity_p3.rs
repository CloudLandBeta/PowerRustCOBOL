// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Three more things a page did differently from the desktop, in a real browser** (the PowerAnalyticsWeb parity
//! job, P3, 2026-10-28). Each desktop value was measured with `rcrun run-form --headless`:
//!
//! * **P-8b — a Timer designed `enabled="false"` with no `Enabled` property.** The desktop runs a Timer by its own
//!   `Enabled` property, on when the design has none (`render.rs` CT::Timer): it ticked 18 times in 1.5 s. The page
//!   took the element's flag and never ticked.
//! * **A child window of a responsive form.** `OpenFormAsync` of a form designed `responsive="true"` drew a 64x30
//!   title chip whose body had no height: its controls could not be reached. The desktop opens a child window at
//!   the form's designed size.
//! * **FX-4a — the main window closed while a child window is open.** The desktop ends the program
//!   (`program_ended: true`); the page left the child alone on a blank page and said nothing.
//!
//! The program is the ABI fixture's own kind of module; a headless Chrome clicks with real mouse events. With no
//! Chrome on the machine (`CHROME` names one) the test says it was skipped.

mod support;

use cobolt_wasm_abi::fixture::{build, FixtureForm, FixtureHandler, FixtureSpec, Step, Target};
use cobolt_wasm_abi::manifest::Limits;
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome, Page};

const MAIN: &str = "main-form";
const CHILD: &str = "child-form";

fn spec(multi: bool) -> FixtureSpec {
    let t = Target::control;
    let set = |id: &str, v: &str| Step::Set { target: t(id), name: "Caption".into(), value: v.into() };
    FixtureSpec {
        project: "parity3".into(),
        multi_memory: multi,
        limits: Limits::default(),
        forms: vec![
            FixtureForm {
                name: MAIN.into(),
                main: true,
                instances: 1,
                controls: vec![
                    ("LBL-OUT".into(), "Label".into()),
                    ("LBL-TICK".into(), "Label".into()),
                    ("BTN-OPEN".into(), "Button".into()),
                    ("BTN-CLOSE".into(), "Button".into()),
                    ("TMR-1".into(), "Timer".into()),
                ],
                handlers: vec![
                    FixtureHandler::form("onLoad", vec![set("LBL-OUT", "loaded")]),
                    FixtureHandler::on("BTN-OPEN", "onClick", vec![Step::Call { target: Target::Form, name: "OpenFormAsync".into(), args: vec![CHILD.into()] }, set("LBL-OUT", "opened")]),
                    FixtureHandler::on("BTN-CLOSE", "onClick", vec![Step::Call { target: Target::Form, name: "Close".into(), args: vec![] }]),
                    FixtureHandler::on("TMR-1", "onTick", vec![set("LBL-TICK", "ticked")]),
                ],
            },
            FixtureForm {
                name: CHILD.into(),
                main: false,
                instances: 1,
                controls: vec![("LBL-CHILD".into(), "Label".into()), ("BTN-CHILD".into(), "Button".into())],
                handlers: vec![FixtureHandler::on("BTN-CHILD", "onClick", vec![set("LBL-CHILD", "child clicked")])],
            },
        ],
    }
}

/// The main form: the Timer as the web example's TMR-STAGE kind is designed (the element disabled, no `Enabled`
/// property); the buttons low enough that the child window (opened at 24,24) does not cover them.
const MAIN_CFRM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="main-form" width="800" height="600">
  <Control id="LBL-OUT" type="Label" x="10" y="10" w="200" h="28"></Control>
  <Control id="LBL-TICK" type="Label" x="220" y="10" w="200" h="28"></Control>
  <Control id="BTN-OPEN" type="Button" x="10" y="420" w="200" h="28"></Control>
  <Control id="BTN-CLOSE" type="Button" x="220" y="420" w="200" h="28"></Control>
  <Control id="TMR-1" type="Timer" x="500" y="10" w="24" h="24" enabled="false"><Property name="Interval">100</Property></Control>
</Form>
"#;

/// The child form: responsive, 400x200 as designed.
const CHILD_CFRM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="child-form" title="child-form" width="400" height="200" responsive="true">
  <Control id="LBL-CHILD" type="Label" x="10" y="10" w="200" h="28"></Control>
  <Control id="BTN-CHILD" type="Button" x="10" y="60" w="200" h="28"></Control>
</Form>
"#;

const LIB: &str = r#"
  const root = document.getElementById('pr-app').shadowRoot;
  const el = id => root.querySelector('[data-name="' + id + '"]');
  const text = id => { const e = el(id); return e ? e.textContent : null; };
"#;

fn js(body: &str) -> String {
    format!("(() => {{ {LIB} {body} }})()")
}

/// A user's click on `id`: at its centre, which must reach the control.
fn click(page: &mut Page, id: &str) {
    page.wait_for(&js(&format!("const e = el('{id}'); if (!e) return false; const r = e.getBoundingClientRect(); const top = root.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2); return top === e || e.contains(top);")), 10)
        .unwrap_or_else(|e| panic!("{id} is not what a click at its centre reaches: {e}"));
    page.click_at(&js(&format!("const r = el('{id}').getBoundingClientRect(); return {{ x: r.x, y: r.y, width: r.width, height: r.height }};"))).unwrap();
}

fn wait_text(page: &mut Page, id: &str, want: &str) {
    page.wait_for(&js(&format!("return text('{id}') === '{want}';")), 10)
        .unwrap_or_else(|e| panic!("{id} never read {want:?} ({e}); it reads {:?}", page.eval(&js(&format!("return text('{id}');"))).unwrap()));
}

#[test]
fn a_disabled_element_timer_ticks_a_responsive_child_window_has_its_size_and_closing_the_main_window_ends_the_application() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let (multi, single) = (build(&spec(true)), build(&spec(false)));
    let player_out = support::built_player();
    let input = BundleInput {
        project: "parity3".into(),
        forms: vec![
            FormInput { name: MAIN.into(), cfrm: MAIN_CFRM.into(), stylesheet: None },
            FormInput { name: CHILD.into(), cfrm: CHILD_CFRM.into(), stylesheet: None },
        ],
        program_multi: Some(multi.wasm),
        program_single: Some(single.wasm),
        stylesheet: None,
        menus: Vec::new(),
        assets: Vec::new(),
        player: PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        options: BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    };
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(800, 600).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    wait_text(&mut page, "LBL-OUT", "loaded");

    // ── P-8b: the Timer runs by its own property (none: on), whatever the element's flag ──────────────────────
    wait_text(&mut page, "LBL-TICK", "ticked");

    // ── the child window opens at its designed size and its controls answer ─────────────────────────────────────
    click(&mut page, "BTN-OPEN");
    wait_text(&mut page, "LBL-OUT", "opened");
    let body = page
        .wait_for(&js("const f = root.querySelector('[data-pr-form=\"child-form\"]'); if (!f) return false; const b = f.closest('.pr-body'); if (!b) return false; const r = b.getBoundingClientRect(); return r.height > 0 ? { w: r.width, h: r.height } : false;"), 10)
        .unwrap_or_else(|e| panic!("the child window has no body: {e}"));
    assert!((body["w"].as_f64().unwrap() - 400.0).abs() < 1.0 && (body["h"].as_f64().unwrap() - 200.0).abs() < 1.0, "the designed 400x200: {body}");
    click(&mut page, "BTN-CHILD");
    wait_text(&mut page, "LBL-CHILD", "child clicked");

    // ── FX-4a: the main window closes while the child is open: the application ends ─────────────────────────────
    click(&mut page, "BTN-CLOSE");
    let ended = page
        .wait_for(&js("const e = root.querySelector('.pr-ended'); if (!e || !e.checkVisibility()) return false; return { text: e.textContent, forms: root.querySelectorAll('[data-pr-inst]').length };"), 10)
        .unwrap_or_else(|e| panic!("closing the main window with a child open did not end the application: {e}; forms on the page: {}", page.eval(&js("return root.querySelectorAll('[data-pr-inst]').length;")).unwrap()));
    assert!(ended["text"].as_str().unwrap().starts_with("The application has ended."), "{ended}");
    assert_eq!(ended["forms"], 0, "the child window went with the main one: {ended}");
    println!("P-8b: the element-disabled Timer ticked; the responsive child window opened at {body} and answered; FX-4a: '{}'", ended["text"]);
    drop(server);
}
