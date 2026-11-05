// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The same graphics answers from the plain player and the threaded player** (WASM-018 T30, AC12).
//!
//! One bundle is served twice: without isolation headers the page chooses the plain player on the page's own thread, with them
//! it chooses the threaded player with the program in a Worker. A button's handler draws on a Canvas2D, makes an ImageBuffer,
//! blurs it and reads pixels back from both; the labels must read the same on both pages, and the page says which player ran.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_wasm_abi::fixture::{build, FixtureForm, FixtureHandler, FixtureSpec, Step, Target};
use cobolt_wasm_abi::manifest::Limits;
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput};
use support::cdp::find_chrome;

const MAIN: &str = "main-form";

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| (*s).to_owned()).collect()
}

fn spec(multi: bool) -> FixtureSpec {
    let mut controls: Vec<(String, String)> = ["LBL-A", "LBL-B"].iter().map(|l| ((*l).into(), "Label".into())).collect();
    controls.push(("BTN-GO".into(), "Button".into()));
    controls.push(("CNV-1".into(), "Canvas2D".into()));
    controls.push(("IMG-1".into(), "ImageBuffer".into()));
    let call = |t: &str, n: &str, a: &[&str]| Step::Call { target: Target::control(t), name: n.into(), args: args(a) };
    let ask = |t: &str, n: &str, a: &[&str]| Step::CallResult { target: Target::control(t), name: n.into(), args: args(a) };
    FixtureSpec {
        project: "players".into(),
        multi_memory: multi,
        limits: Limits::default(),
        forms: vec![FixtureForm {
            name: MAIN.into(),
            main: true,
            instances: 1,
            controls,
            handlers: vec![FixtureHandler::on(
                "BTN-GO",
                "onClick",
                vec![
                    call("CNV-1", "SetFillColor", &["#FF0000"]),
                    call("CNV-1", "FillRect", &["0", "0", "10", "10"]),
                    call("CNV-1", "Present", &[]),
                    ask("CNV-1", "GetPixel", &["2", "2"]),
                    Step::EchoAnswer { target: Target::control("LBL-A"), name: "Caption".into() },
                    call("IMG-1", "Create", &["200", "150", "#FF8800"]),
                    call("IMG-1", "BlurGaussian", &["2"]),
                    ask("IMG-1", "GetPixel", &["5", "5"]),
                    Step::EchoAnswer { target: Target::control("LBL-B"), name: "Caption".into() },
                ],
            )],
        }],
    }
}

const CFRM: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="main-form" width="400" height="300">
  <Control id="LBL-A" type="Label" x="10" y="10" w="200" h="20"><Property name="Caption">-</Property></Control>
  <Control id="LBL-B" type="Label" x="10" y="30" w="200" h="20"><Property name="Caption">-</Property></Control>
  <Control id="BTN-GO" type="Button" x="10" y="60" w="100" h="30"><Property name="Caption">Go</Property></Control>
  <Control id="CNV-1" type="Canvas2D" x="10" y="100" w="100" h="60"></Control>
  <Control id="IMG-1" type="ImageBuffer" x="0" y="0" w="1" h="1"></Control>
</Form>
"##;

/// Serve the bundle under `isolation`, press the button, and return (the player/mode the page chose, label A, label B, ms).
fn run(exe: &std::path::Path, isolation: Isolation) -> (String, String, String, u128) {
    let (multi, single) = (build(&spec(true)), build(&spec(false)));
    let player_out = support::repo_root().join("assets/web-player");
    let input = BundleInput {
        project: "players".into(),
        forms: vec![FormInput { name: MAIN.into(), cfrm: CFRM.into(), stylesheet: None }],
        program_multi: Some(multi.wasm),
        program_single: Some(single.wasm),
        stylesheet: None,
        menus: Vec::new(),
        assets: Vec::new(),
        player: PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), threaded: Some(support::player_variant(&player_out, "threaded")), parts: support::player_parts(&player_out), ..Default::default() },
        options: BundleOptions { isolation, ..Default::default() },
    };
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation, ..Default::default() }).unwrap();
    let chrome = support::cdp::Chrome::launch(exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    let btn = "(() => { const b = document.getElementById('pr-app').shadowRoot.querySelector('[data-name=\"BTN-GO\"]'); const r = b ? b.getBoundingClientRect() : null; return r ? { x: r.x, y: r.y, width: r.width, height: r.height } : null; })()";
    page.wait_for(&format!("!!({btn})"), 40).unwrap_or_else(|e| panic!("the page did not come up: {e}"));
    let which = page.eval("PowerRustCOBOL.mount(document.getElementById('pr-app')).then(a => a.choice.player + '/' + a.choice.mode)").unwrap();
    page.eval("window.__errs = []; for (const k of ['error', 'warn']) { const o = console[k].bind(console); console[k] = (...a) => { window.__errs.push(k + ': ' + a.map(String).join(' ')); o(...a); }; } addEventListener('error', e => window.__errs.push('uncaught: ' + e.message)); true").unwrap();
    let started = std::time::Instant::now();
    page.click_at(btn).unwrap();
    let label = |n: &str| format!("document.getElementById('pr-app').shadowRoot.querySelector('[data-name=\"{n}\"]').textContent.trim()");
    page.wait_for(&format!("{} !== '-' && {} !== '-'", label("LBL-A"), label("LBL-B")), 30).unwrap_or_else(|e| { let all = page.eval(r#"[...document.getElementById('pr-app').shadowRoot.querySelectorAll('[data-name^="LBL-"]')].map(e => e.dataset.name + '=' + e.textContent.trim()).join(' | ') + ' || ' + (window.__errs || []).join(' / ') + ' || ' + performance.getEntriesByType('resource').map(r => r.name.split('/').slice(-2).join('/')).join(', ')"#).unwrap_or_default(); panic!("the handler never answered on {which}: {e}; {all}") });
    let ms = started.elapsed().as_millis();
    let get = |page: &mut support::cdp::Page, n: &str| page.eval(&label(n)).unwrap().as_str().unwrap_or("").to_owned();
    let (a, b) = (get(&mut page, "LBL-A"), get(&mut page, "LBL-B"));
    (which.as_str().unwrap_or("?").to_owned(), a, b, ms)
}

#[test]
fn the_plain_player_and_the_threaded_player_give_the_same_graphics_answers() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let plain = run(&exe, Isolation::Off);
    let threaded = run(&exe, Isolation::RequireCorp);
    assert!(plain.0.starts_with("plain/"), "without isolation headers the page runs the plain player: {}", plain.0);
    assert!(threaded.0.starts_with("threaded/"), "with them the page runs the threaded player: {}", threaded.0);
    assert_eq!(plain.1, "4278190335", "the pixel the handler drew (red) was read back on the plain player");
    assert!(!plain.1.is_empty() && !plain.2.is_empty(), "the pixels were read back: {plain:?}");
    assert_eq!((&plain.1, &plain.2), (&threaded.1, &threaded.2), "the same pixels on both players: {plain:?} vs {threaded:?}");
    println!(
        "T30 plain vs threaded (Chrome): {} -> Canvas2D pixel {} ImageBuffer blurred pixel {} in {} ms; {} -> the same answers in {} ms",
        plain.0, plain.1, plain.2, plain.3, threaded.0, threaded.3
    );
}
