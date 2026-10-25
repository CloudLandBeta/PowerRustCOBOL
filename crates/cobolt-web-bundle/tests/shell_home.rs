// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Home page of the application shell, in a real browser** (the operator, 2026-10-25: of the eight
//! controls of a new Web project's main form only the SideMenu and the Gauge showed in Chrome; the labels,
//! the text box and the button were all there, laid out, and invisible).
//!
//! A main form with a SideMenu turns the shell on: a menu rail, a breadcrumb and a ContentPane where other
//! forms load. Until a form loads into the ContentPane, the main form's own controls are the Home page,
//! and the ContentPane, empty, must stay out of their way. It sits in front of them in the page and has a
//! background of its own, so shown it covered every control of the form to the right of the rail.
//!
//! The fixture program's main form is given a SideMenu and its controls are moved to the right of the
//! rail, where a designer puts a Home page. A headless Chrome opens the bundle served by its own server;
//! the test then asks the browser, for each control, which element a user clicking its centre would hit.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use std::time::Instant;

use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM};
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleOptions, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome};

/// The fixture's main form with an application shell: a SideMenu docked at the left, and the form's own
/// controls moved to the right of its rail (the fixture puts them at x = 10, under the rail).
fn shell_form(plain: &str) -> String {
    let moved = plain.replace("x=\"10\"", "x=\"272\"");
    let rail = "  <Control id=\"SideMenu-1\" type=\"SideMenu\" x=\"-8\" y=\"0\" w=\"264\" h=\"480\"></Control>\n</Form>";
    assert!(moved.contains("</Form>"), "the fixture's form ends where the test expects");
    moved.replace("</Form>", rail)
}

/// For every control of the main form but the rail: its box, and the element at the centre of that box.
const LOOK: &str = r#"(() => {
  const root = document.getElementById('pr-app').shadowRoot;
  const content = root.querySelector('.pr-content');
  const form = root.querySelector('[data-pr-form="main-form"]');
  const controls = [];
  for (const el of form.querySelectorAll('[data-name]')) {
    if (el.closest('.pr-sidemenu')) continue;
    const b = el.getBoundingClientRect();
    if (b.width === 0 || b.height === 0) continue;
    const top = root.elementFromPoint(b.x + b.width / 2, b.y + b.height / 2);
    const owner = top && top.closest('[data-name]');
    controls.push({
      name: el.dataset.name,
      x: Math.round(b.x),
      onTop: !!top && el.contains(top),
      hit: owner ? owner.dataset.name : (top ? top.className : null),
    });
  }
  const c = content.getBoundingClientRect();
  return {
    shell: !!root.querySelector('.pr-main[data-shell]'),
    rail: !!root.querySelector('.pr-sidemenu'),
    contentHidden: content.hidden,
    contentBox: [Math.round(c.width), Math.round(c.height)],
    controls,
  };
})()"#;

#[test]
fn the_main_forms_own_controls_are_the_home_page_and_the_empty_content_pane_does_not_cover_them() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let player_out = support::built_player();
    let mut input = support::input_with(
        PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    );
    for f in input.forms.iter_mut().filter(|f| f.name == MAIN_FORM) {
        f.cfrm = shell_form(&f.cfrm);
    }
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();

    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1024, 768).unwrap();
    let t = Instant::now();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for(
        &format!("(() => {{ const r = document.getElementById('pr-app').shadowRoot; const l = r && r.querySelector('[data-pr-form=\"{MAIN_FORM}\"] [data-name=\"{LBL_OUT}\"]'); return !!l && l.textContent === 'loaded'; }})()"),
        30,
    )
    .unwrap_or_else(|e| panic!("the application did not start: {e}"));
    let started_ms = t.elapsed().as_millis();
    let seen = page.eval(LOOK).unwrap();
    drop(page);
    server.stop();

    assert_eq!(seen["shell"], serde_json::json!(true), "a main form with a SideMenu turns the shell on: {seen}");
    assert_eq!(seen["rail"], serde_json::json!(true), "the rail is drawn: {seen}");
    let controls = seen["controls"].as_array().unwrap();
    assert!(controls.len() >= 3, "the Home page has controls to look at: {seen}");
    for c in controls {
        assert!(c["x"].as_i64().unwrap() >= 220, "{}: the control sits to the right of the rail (x = {}), the Home page's place", c["name"], c["x"]);
    }
    let covered: Vec<String> = controls.iter().filter(|c| c["onTop"] != serde_json::json!(true)).map(|c| format!("{} (a click lands on {})", c["name"], c["hit"])).collect();
    assert!(covered.is_empty(), "the Home page is covered: {covered:?}\n{seen}");
    assert_eq!(seen["contentHidden"], serde_json::json!(true), "the empty ContentPane is hidden until a form loads into it: {seen}");
    assert_eq!(seen["contentBox"], serde_json::json!([0, 0]), "and takes no room: {seen}");

    println!(
        "shell Home page in Chrome ({}): the application started in {started_ms} ms; {} controls right of the 220 px rail, each the element a click on its centre reaches; the empty ContentPane is hidden (0x0)",
        exe.display(),
        controls.len()
    );
}
