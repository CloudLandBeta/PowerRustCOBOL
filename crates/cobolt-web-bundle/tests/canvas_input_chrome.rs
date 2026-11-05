// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A canvas's pointer, wheel, touch and key events, and the polled input state, in a real browser** (WASM-018 T25, T30, R13, AC12).
//!
//! A form with a Canvas2D binds `onPointerDown`, `onWheel`, `onTouchStart` and `onKeyDown` to handlers that write the event's
//! argument into labels. The page is served by the Build's own bundle and opened in a headless Chrome; the test drives it with
//! the browser's own input (a mouse press, a wheel turn, a finger, a key held down) and reads the labels. The pointer press also
//! asks the canvas `IsKeyDown("ArrowLeft")` and reads `PointerX`, so the polled state is the browser's: `1` while the key is
//! held, `0` after it is released, and the pointer's place as the press saw it.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_wasm_abi::fixture::{build, FixtureForm, FixtureHandler, FixtureSpec, Step, Target};
use cobolt_wasm_abi::manifest::Limits;
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput};
use serde_json::json;
use support::cdp::find_chrome;

const MAIN: &str = "main-form";

fn echo(label: &str) -> Step {
    Step::EchoArg0 { target: Target::control(label), name: "Caption".into() }
}

fn spec(multi: bool) -> FixtureSpec {
    let labels = ["LBL-PTR", "LBL-WHEEL", "LBL-TOUCH", "LBL-KEY", "LBL-POLL", "LBL-PX"];
    let mut controls: Vec<(String, String)> = labels.iter().map(|l| ((*l).into(), "Label".into())).collect();
    controls.push(("CNV-1".into(), "Canvas2D".into()));
    FixtureSpec {
        project: "input".into(),
        multi_memory: multi,
        limits: Limits::default(),
        forms: vec![FixtureForm {
            name: MAIN.into(),
            main: true,
            instances: 1,
            controls,
            handlers: vec![
                FixtureHandler::on(
                    "CNV-1",
                    "onPointerDown",
                    vec![
                        echo("LBL-PTR"),
                        Step::CallResult { target: Target::control("CNV-1"), name: "IsKeyDown".into(), args: vec!["ArrowLeft".into()] },
                        Step::EchoAnswer { target: Target::control("LBL-POLL"), name: "Caption".into() },
                        Step::Get { target: Target::control("CNV-1"), name: "PointerX".into() },
                        Step::EchoAnswer { target: Target::control("LBL-PX"), name: "Caption".into() },
                    ],
                ),
                FixtureHandler::on("CNV-1", "onWheel", vec![echo("LBL-WHEEL")]),
                FixtureHandler::on("CNV-1", "onTouchStart", vec![echo("LBL-TOUCH")]),
                FixtureHandler::on("CNV-1", "onKeyDown", vec![echo("LBL-KEY")]),
            ],
        }],
    }
}

const CFRM: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="main-form" width="400" height="400">
  <Control id="LBL-PTR" type="Label" x="10" y="10" w="200" h="20"><Property name="Caption">-</Property></Control>
  <Control id="LBL-WHEEL" type="Label" x="10" y="30" w="200" h="20"><Property name="Caption">-</Property></Control>
  <Control id="LBL-TOUCH" type="Label" x="10" y="50" w="200" h="20"><Property name="Caption">-</Property></Control>
  <Control id="LBL-KEY" type="Label" x="10" y="70" w="200" h="20"><Property name="Caption">-</Property></Control>
  <Control id="LBL-POLL" type="Label" x="10" y="90" w="200" h="20"><Property name="Caption">-</Property></Control>
  <Control id="LBL-PX" type="Label" x="10" y="110" w="200" h="20"><Property name="Caption">-</Property></Control>
  <Control id="CNV-1" type="Canvas2D" x="10" y="150" w="200" h="100"></Control>
</Form>
"##;

#[test]
fn a_canvas_hears_the_pointer_the_wheel_a_finger_and_a_key_and_answers_the_polled_state() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let (multi, single) = (build(&spec(true)), build(&spec(false)));
    let player_out = support::built_player();
    let input = BundleInput {
        project: "input".into(),
        forms: vec![FormInput { name: MAIN.into(), cfrm: CFRM.into(), stylesheet: None }],
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
    let chrome = support::cdp::Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    let started = std::time::Instant::now();

    let canvas = "(() => { const c = document.getElementById('pr-app').shadowRoot.querySelector('[data-name=\"CNV-1\"] canvas'); const r = c ? c.getBoundingClientRect() : null; return r ? { x: r.x, y: r.y, width: r.width, height: r.height } : null; })()";
    page.wait_for(&format!("!!({canvas})"), 30).unwrap_or_else(|e| panic!("the page did not come up: {e}"));
    let text = |page: &mut support::cdp::Page, name: &str| -> String {
        page.eval(&format!("document.getElementById('pr-app').shadowRoot.querySelector('[data-name=\"{name}\"]').textContent.trim()")).unwrap().as_str().unwrap_or("").to_owned()
    };
    let wait_text = |page: &mut support::cdp::Page, name: &str, want: &str| {
        let expr = format!("document.getElementById('pr-app').shadowRoot.querySelector('[data-name=\"{name}\"]').textContent.trim() === {}", json!(want));
        if page.wait_for(&expr, 10).is_err() {
            let all = page.eval(r#"[...document.getElementById('pr-app').shadowRoot.querySelectorAll('[data-name^="LBL-"]')].map(e => e.dataset.name + '=' + e.textContent.trim()).join(' | ')"#).unwrap();
            let log = page.eval("(window.__errs || []).join(' / ')").unwrap_or_default();
            panic!("{name} never read {want:?}; the labels read {all}; {log}");
        }
    };
    let rect = page.eval(canvas).unwrap();
    let (cx, cy) = (rect["x"].as_f64().unwrap() + rect["width"].as_f64().unwrap() / 2.0, rect["y"].as_f64().unwrap() + rect["height"].as_f64().unwrap() / 2.0);
    let (left, top) = (rect["x"].as_f64().unwrap(), rect["y"].as_f64().unwrap());

    // A press on the canvas: the point in the canvas's own pixels; the key is not held yet, so the poll answers 0.
    page.click_at(canvas).unwrap();
    wait_text(&mut page, "LBL-PTR", "100 50");
    wait_text(&mut page, "LBL-POLL", "0");
    wait_text(&mut page, "LBL-PX", "100");

    // A key held down: raised once as its name, and the next press polls it as held.
    page.call("Input.dispatchKeyEvent", json!({"type": "rawKeyDown", "key": "ArrowLeft", "code": "ArrowLeft", "windowsVirtualKeyCode": 37})).unwrap();
    wait_text(&mut page, "LBL-KEY", "ArrowLeft");
    page.click_at(canvas).unwrap();
    wait_text(&mut page, "LBL-POLL", "1");
    // Released: the poll answers 0 again.
    page.call("Input.dispatchKeyEvent", json!({"type": "keyUp", "key": "ArrowLeft", "code": "ArrowLeft", "windowsVirtualKeyCode": 37})).unwrap();
    page.click_at(canvas).unwrap();
    wait_text(&mut page, "LBL-POLL", "0");

    // The wheel: its step as `dx dy`.
    page.call("Input.dispatchMouseEvent", json!({"type": "mouseWheel", "x": cx, "y": cy, "deltaX": 0, "deltaY": 120})).unwrap();
    wait_text(&mut page, "LBL-WHEEL", "0 120");

    // A finger: the point of the touch in the canvas's pixels.
    page.call("Input.dispatchTouchEvent", json!({"type": "touchStart", "touchPoints": [{"x": left + 30.0, "y": top + 40.0}]})).unwrap();
    page.call("Input.dispatchTouchEvent", json!({"type": "touchEnd", "touchPoints": []})).unwrap();
    // A finger lands on whole device pixels, and the canvas can sit at a fraction of one: within one pixel of where it was put.
    let near = "(() => { const t = document.getElementById('pr-app').shadowRoot.querySelector('[data-name=\"LBL-TOUCH\"]').textContent.trim().split(' ').map(Number); return t.length === 2 && Math.abs(t[0] - 30) <= 1 && Math.abs(t[1] - 40) <= 1; })()";
    page.wait_for(near, 10).unwrap_or_else(|_| panic!("the touch was not heard near 30 40: {}", text(&mut page, "LBL-TOUCH")));

    println!(
        "T25/T30 browser input (Chrome): press -> onPointerDown({}) PointerX {}; ArrowLeft held -> onKeyDown({}) and IsKeyDown 1, released -> 0; wheel -> onWheel({}); touch -> onTouchStart({}) in {} ms",
        text(&mut page, "LBL-PTR"),
        text(&mut page, "LBL-PX"),
        text(&mut page, "LBL-KEY"),
        text(&mut page, "LBL-WHEEL"),
        text(&mut page, "LBL-TOUCH"),
        started.elapsed().as_millis()
    );
}
