// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Two things a page did differently from the desktop, in a real browser** (the PowerAnalyticsWeb parity job,
//! 2026-10-28):
//!
//! * **`Enable()` after `Disable()`.** A button disabled by one handler and enabled by a later one stayed
//!   disabled: the view writes the element's `disabled` property only while the control is disabled, and the
//!   page never took a property back once a view stopped writing it. The paging buttons of the Dados page
//!   (`BTN-PREV`/`BTN-NEXT`) could not be clicked again. On the desktop `Enable()` gives the button back.
//! * **The main form closed** (`me::Close()`, the red x). The desktop closes the window and the program ends;
//!   the page went blank, with nothing said and no way on. It now tries to close its tab (a browser allows it
//!   only for a window a script opened) and otherwise says the application has ended, in the browser's
//!   language, with a Reload button.
//!
//! The program is the ABI fixture's own kind of module, written for these two cases; a headless Chrome clicks
//! with real mouse events. With no Chrome on the machine (`CHROME` names one) the test says it was skipped.

mod support;

use cobolt_wasm_abi::fixture::{build, standard_cfrm, FixtureForm, FixtureHandler, FixtureSpec, Step, Target};
use cobolt_wasm_abi::manifest::Limits;
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome, Page};

const FORM: &str = "main-form";

fn spec(multi: bool) -> FixtureSpec {
    let t = Target::control;
    let out = |v: &str| Step::Set { target: t("LBL-OUT"), name: "Caption".into(), value: v.into() };
    FixtureSpec {
        project: "parity".into(),
        multi_memory: multi,
        limits: Limits::default(),
        forms: vec![FixtureForm {
            name: FORM.into(),
            main: true,
            instances: 1,
            controls: vec![
                ("LBL-OUT".into(), "Label".into()),
                ("BTN-OFF".into(), "Button".into()),
                ("BTN-ON".into(), "Button".into()),
                ("BTN-TARGET".into(), "Button".into()),
                ("BTN-CLOSE".into(), "Button".into()),
            ],
            handlers: vec![
                FixtureHandler::form("onLoad", vec![out("loaded")]),
                FixtureHandler::on("BTN-OFF", "onClick", vec![Step::Call { target: t("BTN-TARGET"), name: "Disable".into(), args: vec![] }, out("off")]),
                FixtureHandler::on("BTN-ON", "onClick", vec![Step::Call { target: t("BTN-TARGET"), name: "Enable".into(), args: vec![] }, out("on")]),
                FixtureHandler::on("BTN-TARGET", "onClick", vec![out("target clicked")]),
                FixtureHandler::on("BTN-CLOSE", "onClick", vec![Step::Call { target: Target::Form, name: "Close".into(), args: vec![] }]),
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

/// A user's click on `id`: at its centre, which must reach the control.
fn click(page: &mut Page, id: &str) {
    page.wait_for(&js(&format!("const e = el('{id}'); if (!e) return false; const r = e.getBoundingClientRect(); const top = root.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2); return top === e || e.contains(top);")), 10)
        .unwrap_or_else(|e| panic!("{id} is not what a click at its centre reaches: {e}"));
    page.click_at(&js(&format!("const r = el('{id}').getBoundingClientRect(); return {{ x: r.x, y: r.y, width: r.width, height: r.height }};"))).unwrap();
}

fn wait_caption(page: &mut Page, want: &str) {
    page.wait_for(&js(&format!("return caption() === '{want}';")), 10)
        .unwrap_or_else(|e| panic!("LBL-OUT never read {want:?} ({e}); it reads {:?}", page.eval(&js("return caption();")).unwrap()));
}

#[test]
fn enable_after_disable_gives_the_button_back_and_closing_the_main_form_says_the_application_ended() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let (multi, single) = (build(&spec(true)), build(&spec(false)));
    let player_out = support::built_player();
    let input = BundleInput {
        project: "parity".into(),
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
    wait_caption(&mut page, "loaded");

    // ── P-8: one handler disables the button, a later one enables it ─────────────────────────────────────────────
    let disabled = js("const e = el('BTN-TARGET'); return e.disabled === true || e.getAttribute('aria-disabled') === 'true';");
    click(&mut page, "BTN-OFF");
    wait_caption(&mut page, "off");
    page.wait_for(&disabled, 10).unwrap_or_else(|e| panic!("BTN-TARGET was not disabled: {e}"));
    click(&mut page, "BTN-ON");
    wait_caption(&mut page, "on");
    page.wait_for(&js("const e = el('BTN-TARGET'); return e.disabled === false && e.getAttribute('aria-disabled') !== 'true';"), 10)
        .unwrap_or_else(|e| panic!("Enable() did not give BTN-TARGET back ({e}): disabled={}", page.eval(&js("return el('BTN-TARGET').disabled;")).unwrap()));
    click(&mut page, "BTN-TARGET");
    wait_caption(&mut page, "target clicked");

    // ── FX-4a: the main form closes ─────────────────────────────────────────────────────────────────────────────────
    click(&mut page, "BTN-CLOSE");
    let ended = page
        .wait_for(&js("const e = root.querySelector('.pr-ended'); if (!e || !e.checkVisibility()) return false; const r = e.getBoundingClientRect(); return { text: e.textContent, lang: e.getAttribute('lang'), button: !!e.querySelector('button[data-part=reload]'), w: r.width, h: r.height };"), 10)
        .unwrap_or_else(|e| panic!("the page did not say the application ended: {e}"));
    assert!(ended["text"].as_str().unwrap().starts_with("The application has ended."), "{ended}");
    assert_eq!(ended["lang"], "en", "{ended}");
    assert_eq!(ended["button"], true, "a Reload button: {ended}");
    assert!(ended["w"].as_f64().unwrap() >= 799.0 && ended["h"].as_f64().unwrap() >= 599.0, "it takes the page's place: {ended}");
    assert!(page.eval(&js("return el('BTN-TARGET') === null;")).unwrap().as_bool().unwrap(), "the form is gone");
    println!("P-8: Disable() then Enable() in two handlers gives the button back; FX-4a: the closed main form leaves '{}'", ended["text"]);
    drop(server);
}
