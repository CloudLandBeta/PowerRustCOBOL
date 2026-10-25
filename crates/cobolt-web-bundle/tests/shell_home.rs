// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The application shell, in a real browser** (the operator, 2026-10-25: of the eight controls of a new
//! Web project's main form only the SideMenu and the Gauge showed in Chrome; the labels, the text box and
//! the button were all there, laid out, and invisible; and the page did not match the Form Designer).
//!
//! A main form with a SideMenu turns the shell on: a menu rail, a breadcrumb and a ContentPane where other
//! forms load. Two things must hold.
//!
//! * Until a form loads into the ContentPane, the main form's own controls are the Home page, and the
//!   ContentPane, empty, must stay out of their way. It sits in front of them in the page and has a
//!   background of its own, so shown it covered every control of the form to the right of the rail.
//! * The ContentPane starts where the rail the designer drew ends: the SideMenu's designed x plus its
//!   width, expanded or collapsed. The page used to start it at 220 px whatever the rail was.
//!
//! The fixture program's main form is given a SideMenu (x = -8, 264 wide, as a new Web project's) and its
//! controls are moved to the right of the rail, where a designer puts a Home page. A headless Chrome opens
//! the bundle served by its own server and asks the browser what a user would hit and where the boxes are.
//!
//! With no Chrome on the machine (`CHROME` names one) the tests say they were skipped, and why.

mod support;

use std::time::Instant;

use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM};
use cobolt_web_bundle::devserver::{serve, ServeOptions, Server};
use cobolt_web_bundle::{assemble_bundle, BundleOptions, Isolation, PlayerInput};
use serde_json::json;
use support::cdp::{find_chrome, Chrome, Page};

/// The fixture's main form with an application shell: a SideMenu docked at the left, and the form's own
/// controls moved to the right of its rail (the fixture puts them at x = 10, under the rail).
fn shell_form(plain: &str) -> String {
    let moved = plain.replace("x=\"10\"", "x=\"272\"");
    let rail = "  <Control id=\"SideMenu-1\" type=\"SideMenu\" x=\"-8\" y=\"0\" w=\"264\" h=\"480\"></Control>\n</Form>";
    assert!(moved.contains("</Form>"), "the fixture's form ends where the test expects");
    moved.replace("</Form>", rail)
}

/// The bundle of the fixture program with the shell form, served; and a page of `chrome` open on it once
/// the program's `onLoad` has written its label. Answers the page, the server and how long the start took.
fn open_shell(chrome: &Chrome) -> (Page, Server, u128) {
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
    (page, server, t.elapsed().as_millis())
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
    let (mut page, server, started_ms) = open_shell(&chrome);
    let seen = page.eval(LOOK).unwrap();
    drop(page);
    server.stop();

    assert_eq!(seen["shell"], json!(true), "a main form with a SideMenu turns the shell on: {seen}");
    assert_eq!(seen["rail"], json!(true), "the rail is drawn: {seen}");
    let controls = seen["controls"].as_array().unwrap();
    assert!(controls.len() >= 3, "the Home page has controls to look at: {seen}");
    for c in controls {
        assert!(c["x"].as_i64().unwrap() >= 256, "{}: the control sits to the right of the rail (x = {}), the Home page's place", c["name"], c["x"]);
    }
    let covered: Vec<String> = controls.iter().filter(|c| c["onTop"] != json!(true)).map(|c| format!("{} (a click lands on {})", c["name"], c["hit"])).collect();
    assert!(covered.is_empty(), "the Home page is covered: {covered:?}\n{seen}");
    assert_eq!(seen["contentHidden"], json!(true), "the empty ContentPane is hidden until a form loads into it: {seen}");
    assert_eq!(seen["contentBox"], json!([0, 0]), "and takes no room: {seen}");

    println!(
        "shell Home page in Chrome ({}): the application started in {started_ms} ms; {} controls right of the rail, each the element a click on its centre reaches; the empty ContentPane is hidden (0x0)",
        exe.display(),
        controls.len()
    );
}

/// The rail's box, and where the ContentPane starts, as the browser lays them out.
const RAIL: &str = r#"(() => {
  const root = document.getElementById('pr-app').shadowRoot;
  const rail = root.querySelector('.pr-sidemenu');
  const r = rail.getBoundingClientRect();
  const p = root.querySelector('.pr-shell-pane').getBoundingClientRect();
  return { left: Math.round(r.x), right: Math.round(r.right), pane: Math.round(p.x), collapsed: rail.classList.contains('pr-collapsed') };
})()"#;

/// The rail's toggle button, for a click.
const TOGGLE: &str = "(() => { const b = document.getElementById('pr-app').shadowRoot.querySelector('.pr-sm-toggle').getBoundingClientRect(); return { x: b.x, y: b.y, width: b.width, height: b.height }; })()";

/// The page waits until the rail ends at `right` and the ContentPane starts there (a rail can animate).
fn rail_reaches(right: i64) -> String {
    format!("(() => {{ const r = ({RAIL}); return r.right === {right} && r.pane === {right}; }})()")
}

#[test]
fn the_content_pane_starts_where_the_rail_the_designer_drew_ends() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, server, started_ms) = open_shell(&chrome);

    // Expanded: the rail the designer draws is at x = -8 and 264 wide, so it ends at 256, and so does the pane.
    let expanded = page.eval(RAIL).unwrap();
    assert_eq!(expanded, json!({ "left": -8, "right": 256, "pane": 256, "collapsed": false }), "expanded: the pane starts where the rail ends");

    // Collapsed: the rail keeps its x and is 48 wide (the default CollapsedWidth), so it ends at 40.
    page.click_at(TOGGLE).unwrap();
    page.wait_for(&rail_reaches(40), 10).unwrap_or_else(|e| panic!("collapsed: {e}; now {}", page.eval(RAIL).unwrap()));
    let collapsed = page.eval(RAIL).unwrap();
    assert_eq!(collapsed, json!({ "left": -8, "right": 40, "pane": 40, "collapsed": true }), "collapsed: the pane follows the rail");

    // Expanded again: back where it was.
    page.click_at(TOGGLE).unwrap();
    page.wait_for(&rail_reaches(256), 10).unwrap_or_else(|e| panic!("expanded again: {e}; now {}", page.eval(RAIL).unwrap()));
    let again = page.eval(RAIL).unwrap();
    assert_eq!(again, expanded, "expanded again: the same box and the same pane");
    drop(page);
    server.stop();

    println!(
        "shell rail in Chrome ({}): up in {started_ms} ms; expanded {expanded}; collapsed {collapsed}; expanded again {again}",
        exe.display()
    );
}
