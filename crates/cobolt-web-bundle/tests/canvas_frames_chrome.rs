// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A canvas's `onFrame` in a real browser, and silent while the tab is hidden** (WASM-018 T30, R14, AC12).
//!
//! A form with a Canvas2D at `TargetFps` 30 binds `onFrame` to a handler that writes the event's argument (the elapsed
//! milliseconds) into a label. The page is served by the Build's own bundle and opened in a headless Chrome. The test
//! counts the label's changes: frames arrive at about the target rate; with `document.hidden` forced true they stop (zero
//! in half a second), and when the tab is visible again they resume without a burst (the hidden time is not repaid).
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_wasm_abi::fixture::{build, FixtureForm, FixtureHandler, FixtureSpec, Step, Target};
use cobolt_wasm_abi::manifest::Limits;
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput};
use support::cdp::find_chrome;

const MAIN: &str = "main-form";

fn spec(multi: bool) -> FixtureSpec {
    FixtureSpec {
        project: "frames".into(),
        multi_memory: multi,
        limits: Limits::default(),
        forms: vec![FixtureForm {
            name: MAIN.into(),
            main: true,
            instances: 1,
            controls: vec![("LBL-FRAME".into(), "Label".into()), ("CNV-1".into(), "Canvas2D".into())],
            handlers: vec![FixtureHandler::on("CNV-1", "onFrame", vec![Step::EchoArg0 { target: Target::control("LBL-FRAME"), name: "Caption".into() }])],
        }],
    }
}

const CFRM: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="main-form" width="400" height="300">
  <Control id="LBL-FRAME" type="Label" x="10" y="10" w="200" h="24"><Property name="Caption">waiting</Property></Control>
  <Control id="CNV-1" type="Canvas2D" x="10" y="50" w="200" h="100"><Property name="TargetFps">30</Property></Control>
</Form>
"##;

#[test]
fn on_frame_runs_at_the_target_rate_and_a_hidden_tab_raises_none() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let (multi, single) = (build(&spec(true)), build(&spec(false)));
    let player_out = support::built_player();
    let input = BundleInput {
        project: "frames".into(),
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

    // The label starts as "waiting"; the first frame writes a number. Count the changes of the label from here on.
    let setup = r#"(async () => {
      const root = document.getElementById('pr-app').shadowRoot;
      const el = () => root.querySelector('[data-name="LBL-FRAME"]');
      window.__frames = { n: 0, texts: [] };
      let host = null;
      for (let i = 0; i < 200 && !el(); i++) await new Promise(r => setTimeout(r, 50));
      new MutationObserver(() => { window.__frames.n++; window.__frames.texts.push(el().textContent); }).observe(el(), { childList: true, characterData: true, subtree: true });
      window.__hide = v => Object.defineProperty(document, 'hidden', { configurable: true, get: () => v });
      return true;
    })()"#;
    page.wait_for(setup, 30).unwrap_or_else(|e| panic!("the page did not come up: {e}"));
    let count = |page: &mut support::cdp::Page| page.eval("window.__frames.n").unwrap().as_u64().unwrap_or(0);

    // Visible: about 30 frames a second.
    let t0 = std::time::Instant::now();
    let n0 = count(&mut page);
    std::thread::sleep(std::time::Duration::from_millis(2000));
    let visible = count(&mut page) - n0;
    let secs = t0.elapsed().as_secs_f64();
    let rate = visible as f64 / secs;

    // Hidden: none.
    page.eval("window.__hide(true)").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(200));
    let h0 = count(&mut page);
    std::thread::sleep(std::time::Duration::from_millis(800));
    let hidden = count(&mut page) - h0;

    // Visible again: they resume, with no burst for the hidden second.
    page.eval("window.__hide(false)").unwrap();
    let r0 = count(&mut page);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let resumed = count(&mut page) - r0;
    let texts = page.eval("window.__frames.texts.slice(-5)").unwrap();
    println!(
        "T30 AC12 browser (Chrome): onFrame at TargetFps 30: {visible} frames in {secs:.2} s visible = {rate:.1}/s; hidden for 0.8 s: {hidden} frames; visible again for 1 s: {resumed} frames (last elapsed values {texts})"
    );
    assert!(rate >= 15.0 && rate <= 36.0, "a frame rate near the target: {rate}/s");
    assert_eq!(hidden, 0, "a hidden tab raises no frame");
    assert!((10..=36).contains(&resumed), "the frames resume at the target rate and not as a burst: {resumed} in a second");
}
