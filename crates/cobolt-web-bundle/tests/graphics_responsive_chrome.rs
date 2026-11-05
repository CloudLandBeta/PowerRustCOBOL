// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A 24-megapixel blur does not freeze the page** (WASM-018 T30, AC7).
//!
//! A button's handler makes a 6000 x 4000 ImageBuffer, blurs it (Gaussian, sigma 3) and reads a pixel back. The same bundle is served
//! twice: without isolation headers the page runs the plain player and the program on the page's own thread, so the blur runs on that
//! thread; with them the page runs the threaded player and the program in a Worker, and the blur goes to a graphics worker on a copy of
//! the pixels while the page answers the program later. A timer on the page ticks every 10 ms during the click; the longest gap
//! between two ticks is how long the page was frozen. The pixel read back must be the same on both.
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
    controls.push(("IMG-1".into(), "ImageBuffer".into()));
    let call = |n: &str, a: &[&str]| Step::Call { target: Target::control("IMG-1"), name: n.into(), args: args(a) };
    FixtureSpec {
        project: "responsive".into(),
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
                    call("Create", &["6000", "4000", "#336699"]),
                    call("SetPixel", &["3000", "2000", "#FFFFFF"]),
                    call("BlurGaussian", &["3"]),
                    Step::CallResult { target: Target::control("IMG-1"), name: "GetPixel".into(), args: args(&["3000", "2000"]) },
                    Step::EchoAnswer { target: Target::control("LBL-A"), name: "Caption".into() },
                    Step::CallResult { target: Target::control("IMG-1"), name: "GetWidth".into(), args: Vec::new() },
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
  <Control id="IMG-1" type="ImageBuffer" x="0" y="0" w="1" h="1"></Control>
</Form>
"##;

struct Run {
    player: String,
    pixel: String,
    width: String,
    total_ms: u128,
    longest_gap_ms: f64,
    ticks: f64,
}

fn run(exe: &std::path::Path, isolation: Isolation) -> Run {
    let (multi, single) = (build(&spec(true)), build(&spec(false)));
    let player_out = support::repo_root().join("assets/web-player");
    let input = BundleInput {
        project: "responsive".into(),
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
    let rect = "(() => { const b = document.getElementById('pr-app').shadowRoot.querySelector('[data-name=\"BTN-GO\"]'); const r = b ? b.getBoundingClientRect() : null; return r ? { x: r.x, y: r.y, width: r.width, height: r.height } : null; })()";
    page.wait_for(&format!("!!({rect})"), 40).unwrap_or_else(|e| panic!("the page did not come up: {e}"));
    let which = page.eval("PowerRustCOBOL.mount(document.getElementById('pr-app')).then(a => a.choice.player + '/' + a.choice.mode)").unwrap();
    // The ticker: every 10 ms, remember the longest time between two ticks.
    page.eval("window.__t = { last: performance.now(), longest: 0, ticks: 0 }; setInterval(() => { const n = performance.now(); window.__t.longest = Math.max(window.__t.longest, n - window.__t.last); window.__t.last = n; window.__t.ticks++; }, 10); true").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    page.eval("window.__t.longest = 0; window.__t.ticks = 0; window.__t.last = performance.now(); true").unwrap();
    let label = |n: &str| format!("document.getElementById('pr-app').shadowRoot.querySelector('[data-name=\"{n}\"]').textContent.trim()");
    let started = std::time::Instant::now();
    page.click_at(rect).unwrap();
    page.wait_for(&format!("{} !== '-' && {} !== '-'", label("LBL-A"), label("LBL-B")), 120).unwrap_or_else(|e| panic!("the handler never answered: {e}"));
    let total_ms = started.elapsed().as_millis();
    let get = |page: &mut support::cdp::Page, n: &str| page.eval(&label(n)).unwrap().as_str().unwrap_or("").to_owned();
    let (pixel, width) = (get(&mut page, "LBL-A"), get(&mut page, "LBL-B"));
    let t = page.eval("window.__t").unwrap();
    Run { player: which.as_str().unwrap_or("?").to_owned(), pixel, width, total_ms, longest_gap_ms: t["longest"].as_f64().unwrap_or(-1.0), ticks: t["ticks"].as_f64().unwrap_or(0.0) }
}

#[test]
fn a_24_megapixel_blur_runs_off_the_page_thread_and_gives_the_same_pixel() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let inline = run(&exe, Isolation::Off);
    let away = run(&exe, Isolation::RequireCorp);
    assert!(inline.player.starts_with("plain/"), "{}", inline.player);
    assert!(away.player.starts_with("threaded/worker"), "{}", away.player);
    assert_eq!(inline.width, "6000");
    assert_eq!(inline.pixel, away.pixel, "the same pixel after the blur, in place and in the worker");
    assert_eq!(inline.width, away.width);
    assert!(inline.longest_gap_ms > 500.0, "in place the blur holds the page's thread for most of a second: longest gap {} ms", inline.longest_gap_ms);
    assert!(away.longest_gap_ms < 400.0, "with the blur in a worker the page keeps ticking: longest gap {} ms", away.longest_gap_ms);
    println!(
        "T30 24 MP Gaussian blur (Chrome): {} in place: total {} ms, page frozen up to {:.0} ms ({} ticks); {} in a worker: total {} ms, page frozen up to {:.0} ms ({} ticks); pixel {} on both",
        inline.player, inline.total_ms, inline.longest_gap_ms, inline.ticks, away.player, away.total_ms, away.longest_gap_ms, away.ticks, inline.pixel
    );
}
