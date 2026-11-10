// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A style change in the open Preview, timed** (WASM-007 R11, T23, AC8; the operator's option (a) of
//! 2026-10-31). The Build's own bundle of a one-form Web project is served by the Preview's development
//! server, with its live-update script, and opened in a headless Chrome. Each round is what the stylesheet
//! window's Apply does — `ServerHandle::apply_form_stylesheet`, no Build — and the page is watched until the
//! label's computed `word-spacing` is the new value. The time from Apply to that frame is printed against the
//! plan's budget of 500 ms; the page must not reload and the application must not restart.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

#[path = "../../cobolt-web-bundle/tests/support/cdp.rs"]
#[allow(dead_code)]
mod cdp;

use cobolt_forms::model::{Control, ControlType, Form};
use cobolt_project_tools::web::devserver::{self, ServeOptions};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildRequest};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BUDGET_MS: f64 = 500.0;
const ROUNDS: usize = 5;
const IDLE_SECS: u64 = 10;

const MANIFEST: &str = "[project]\nname = \"Styled.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
    [files]\nforms = [\"webforms/main-form.cfrm\"]\n\n[web]\nmulti_memory = true\n";

const SPACING: &str = "(() => { const a = document.getElementById('pr-app'); const r = a && a.shadowRoot; \
    const e = r && r.querySelector('[data-pr-form=\"main-form\"] [data-name=\"LBL-OUT\"]'); return e ? getComputedStyle(e).wordSpacing : null; })()";

fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64() * 1000.0
}

#[test]
fn an_applied_stylesheet_changes_the_open_preview_within_the_budget_in_chrome() {
    let Some(exe) = cdp::find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let dir = std::env::temp_dir().join(format!("prc-preview-style-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::write(dir.join("Styled.project.toml"), MANIFEST).unwrap();
    let mut form = Form::new("main-form", "Styled", 400, 300);
    let mut label = Control::new("LBL-OUT", ControlType::Label, 10, 60);
    label.set_prop("Caption", "a b c".to_owned());
    form.controls.push(label);
    cobolt_forms::xml::save_form(&form, &dir.join("webforms/main-form.cfrm")).unwrap();
    std::fs::write(dir.join("webforms/main-form.css"), "#LBL-OUT { word-spacing: 1px }\n").unwrap();

    let req = WebBuildRequest::from_manifest(&dir.join("Styled.project.toml")).unwrap();
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
    let mut server = devserver::start(&report.bundle_dir, &ServeOptions { port: 0, isolation: req.web.isolation, live_reload: true }).unwrap();
    let chrome = cdp::Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.navigate(&server.url()).unwrap_or_else(|e| panic!("{e}"));
    if let Err(e) = page.wait_for(&format!("{SPACING} === '1px'"), 30) {
        let seen = page.eval(&format!(
            "({{ spacing: {SPACING}, controls: (() => {{ const a = document.getElementById('pr-app'); const r = a && a.shadowRoot; \
             const el = r && r.querySelector('#LBL-OUT'); window.__dbg = [el && el.tagName, el && el.id, a && a.getAttribute('data-pr-config'), r && r.adoptedStyleSheets.length, r && [...r.adoptedStyleSheets].map(s => [...s.cssRules].map(x => x.cssText).join(' ').slice(0, 160))]; return r ? [...r.querySelectorAll('[data-pr-form],[data-name]')].map(e => (e.getAttribute('data-pr-form') || '') + '/' + (e.getAttribute('data-name') || '')).slice(0, 20) : null; }})(), dbg: window.__dbg }})"
        ));
        panic!("the built sheet: {e}; the page shows {seen:?}; console {:?}; exceptions {:?}", page.console, page.exceptions);
    }
    page.eval("(async () => { window.__same = 1; const a = await PowerRustCOBOL.mount(document.getElementById('pr-app')); a.player.__same = 1; return true; })()").unwrap();

    // Idle: nothing changes for IDLE_SECS, and the page is left alone. Every request it makes is the Preview script's:
    // a long poll held for 25 s, so none arrives in the window (the one outstanding arrived before it opened).
    let (served_before, asked_before) = (server.requests_served(), server.version_requests());
    std::thread::sleep(Duration::from_secs(IDLE_SECS));
    let (idle_answered, idle_asked) = (server.requests_served() - served_before, server.version_requests() - asked_before);
    println!(
        "IDLE ({IDLE_SECS} s, nothing changing, the page open): {idle_asked} requests for /__prc/version arrived, {idle_answered} requests answered (before the long poll: 65 answered in 10 s, measured 2026-11-10)"
    );
    assert!(idle_asked <= 1 && idle_answered <= 1, "an idle page asks once in 25 s: {idle_asked} asked, {idle_answered} answered in {IDLE_SECS} s");

    let mut times = Vec::new();
    for round in 0..ROUNDS {
        let want = format!("{}px", round + 7);
        // Watch every frame and note when the style changed, on the page's own clock (the epoch).
        page.eval(&format!(
            "(() => {{ window.__at = null; const t = () => {{ if ({SPACING} === '{want}') {{ window.__at = performance.timeOrigin + performance.now(); return; }} requestAnimationFrame(t); }}; t(); return true; }})()"
        ))
        .unwrap();
        // The page's poll comes at any point of its interval: start the rounds at different points of it.
        std::thread::sleep(Duration::from_millis(73 * round as u64));
        let css = format!("/* round {round} */\n#LBL-OUT {{ word-spacing: {want} }}\n");
        std::fs::write(dir.join("webforms/main-form.css"), &css).unwrap();
        let applied = now_ms();
        server.apply_form_stylesheet("main-form", &css, &[]).unwrap_or_else(|e| panic!("{e:?}"));
        let at = page.wait_for("window.__at", 10).unwrap_or_else(|e| panic!("round {round}: {e}; console {:?}", page.console));
        times.push(at.as_f64().unwrap() - applied);
    }
    let kept = page
        .eval("(async () => { const a = await PowerRustCOBOL.mount(document.getElementById('pr-app')); return { page: window.__same === 1, player: a.player.__same === 1, navigations: performance.getEntriesByType('navigation').length }; })()")
        .unwrap();
    drop(page);
    server.stop();
    let _ = std::fs::remove_dir_all(&dir);

    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    println!(
        "AC8 (Chrome {}): Apply → computed style in {} ms over {ROUNDS} rounds (min {:.0}, median {:.0}, max {:.0}); budget {BUDGET_MS:.0} ms; page and player kept: {kept}",
        exe.display(),
        times.iter().map(|t| format!("{t:.0}")).collect::<Vec<_>>().join(" / "),
        sorted[0],
        sorted[ROUNDS / 2],
        sorted[ROUNDS - 1]
    );
    assert_eq!(kept["page"], true, "the page did not reload: {kept}");
    assert_eq!(kept["player"], true, "the player stayed: {kept}");
    assert_eq!(kept["navigations"], 1, "{kept}");
    assert!(sorted[ROUNDS - 1] < BUDGET_MS, "over the budget: {times:?}");
}
