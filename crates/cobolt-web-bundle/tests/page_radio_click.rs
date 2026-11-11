// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A radio's or a box's `onClick` reads what the click wrote, in a real browser** (2.0.426).
//!
//! A browser sets a toggle's checkedness before it raises `click` and raises `change` after it. The player wrote `Selected` /
//! `Checked` on `change`, and it runs what it has queued as soon as a listener returns, so a real press ran the `onClick`
//! handler between the two events, on the old state. PowerOficina's dashboard switches its currency on the click of two radio
//! buttons: it asked the Backend for the currency it already showed and the screen waited for its fifteen-second Timer, 0 to
//! 15 s, a different wait every time. A script's `element.click()` raises both events in one turn and hid it; only a press the
//! browser itself makes (the DevTools Protocol's mouse events, as a user's) shows it, which is what this test does.
//!
//! The program is the ABI fixture's own kind of module: the handler of each toggle's `onClick` reads the toggle's own state and
//! writes what it read into a label. A headless Chrome presses the radio, the box, and the box again. With no Chrome on the
//! machine (`CHROME` names one) the test says it was skipped.

mod support;

use cobolt_wasm_abi::fixture::{build, standard_cfrm, FixtureForm, FixtureHandler, FixtureSpec, Step, Target};
use cobolt_wasm_abi::manifest::Limits;
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome, Page};

const FORM: &str = "main-form";

/// A handler that reads the toggle's own `prop` when it is clicked and writes it into `LBL-OUT`.
fn reads_own(id: &str, prop: &str) -> FixtureHandler {
    FixtureHandler::on(
        id,
        "onClick",
        vec![Step::Get { target: Target::control(id), name: prop.into() }, Step::EchoAnswer { target: Target::control("LBL-OUT"), name: "Caption".into() }],
    )
}

fn spec(multi: bool) -> FixtureSpec {
    FixtureSpec {
        project: "radioclick".into(),
        multi_memory: multi,
        limits: Limits::default(),
        forms: vec![FixtureForm {
            name: FORM.into(),
            main: true,
            instances: 1,
            controls: vec![
                ("LBL-OUT".into(), "Label".into()),
                ("RAD-A".into(), "RadioButton".into()),
                ("RAD-B".into(), "RadioButton".into()),
                ("CHK-1".into(), "CheckBox".into()),
            ],
            handlers: vec![
                FixtureHandler::form("onLoad", vec![Step::Set { target: Target::control("LBL-OUT"), name: "Caption".into(), value: "loaded".into() }]),
                reads_own("RAD-B", "Selected"),
                reads_own("CHK-1", "Checked"),
            ],
        }],
    }
}

const LIB: &str = r#"
  const root = document.getElementById('pr-app').shadowRoot;
  const el = id => root.querySelector('[data-name="' + id + '"]');
  const caption = () => { const e = el('LBL-OUT'); return e ? e.textContent : null; };
"#;

fn js(body: &str) -> String {
    format!("(() => {{ {LIB} {body} }})()")
}

/// A user's press on `id`: at its centre, which must reach the control. The mouse events are the browser's own.
fn press(page: &mut Page, id: &str) {
    page.wait_for(&js(&format!("const e = el('{id}'); if (!e) return false; const r = e.getBoundingClientRect(); const top = root.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2); return top === e || e.contains(top);")), 10)
        .unwrap_or_else(|e| panic!("{id} is not what a press at its centre reaches: {e}"));
    page.click_at(&js(&format!("const r = el('{id}').getBoundingClientRect(); return {{ x: r.x, y: r.y, width: r.width, height: r.height }};"))).unwrap();
}

fn wait_caption(page: &mut Page, want: &str, after: &str) {
    page.wait_for(&js(&format!("return caption() === '{want}';")), 10).unwrap_or_else(|e| {
        panic!("after {after}, LBL-OUT never read {want:?} ({e}); it reads {:?} - the handler ran on the state from before the click", page.eval(&js("return caption();")).unwrap())
    });
}

#[test]
fn the_onclick_of_a_radio_and_of_a_box_reads_the_state_the_click_wrote() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let (multi, single) = (build(&spec(true)), build(&spec(false)));
    let player_out = support::built_player();
    let input = BundleInput {
        project: "radioclick".into(),
        forms: vec![FormInput { name: FORM.into(), cfrm: standard_cfrm(&multi, FORM), stylesheet: None }],
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
    wait_caption(&mut page, "loaded", "the page opened");

    // A radio not yet selected: its click reads `Selected` = 1, never the 0 (or nothing) it held before the press.
    press(&mut page, "RAD-B");
    wait_caption(&mut page, "1", "a press on RAD-B");

    // A box ticked, then cleared: each click reads what the press just wrote.
    press(&mut page, "CHK-1");
    wait_caption(&mut page, "1", "a press that ticked CHK-1");
    press(&mut page, "CHK-1");
    wait_caption(&mut page, "0", "a press that cleared CHK-1");
    println!("2.0.426: a real press on a radio and on a box: the handler of onClick read Selected = 1, then Checked = 1 and 0, the state the click had just written");
    drop(server);
}
