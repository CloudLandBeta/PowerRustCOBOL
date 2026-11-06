// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A protected form's address, in a real browser** (WASM-011 T13 / T25; AC6, second half).
//!
//! The fixture program's main form carries an Authentication control and so is the application's sign-in form; its
//! `child-form` is marked `requires-sign-in`. The bundle is served by the bundle's own server and the mock backend
//! (`examples/mock_backend.rs`, run in this process) answers the control's `/auth/*` requests. A headless Chrome acts
//! as a user: it opens the protected form's address with no session, signs in through the control, signs out, and
//! asks the page what it shows after each step.
//!
//! * a protected form's address with no session shows the sign-in form, not the protected one;
//! * after the sign-in the requested form opens, and the address names it;
//! * signing out closes the protected form and shows the sign-in again;
//! * a signed-in session opens the protected form's address at once.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

#[path = "../examples/mock_backend.rs"]
mod mock;

/// The mock is an example program; its `main` is not used here, and naming it says so without a lint attribute.
const _: fn() = mock::main;

use std::net::TcpListener;

use cobolt_wasm_abi::fixture::standard::{CHILD_FORM, MAIN_FORM};
use cobolt_wasm_abi::fixture::{build, standard_cfrm, standard_spec};
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput};
use serde_json::json;
use support::cdp::{find_chrome, Page};

const AUTH: &str = "AUTH-1";
const PASSWORD: &str = "s3cret!";

/// The fixture with the control on its main form and the child form behind the sign-in.
fn bundle_input(player: PlayerInput, backend: &str) -> BundleInput {
    let spec = |multi: bool| {
        let mut s = standard_spec(multi);
        s.forms.iter_mut().find(|f| f.name == MAIN_FORM).unwrap().controls.push((AUTH.into(), "Authentication".into()));
        s
    };
    let (multi, single) = (build(&spec(true)), build(&spec(false)));
    let forms = multi
        .manifest
        .forms
        .iter()
        .map(|f| {
            let mut cfrm = standard_cfrm(&multi, &f.name);
            if f.name == MAIN_FORM {
                cfrm = cfrm.replace("width=\"640\" height=\"480\"", "width=\"720\" height=\"640\" index-base=\"1\"");
                let at = cfrm.find(&format!("<Control id=\"{AUTH}\" ")).expect("the control is on the form");
                let end = at + cfrm[at..].find('>').unwrap();
                cfrm.replace_range(at..end, &format!("<Control id=\"{AUTH}\" type=\"Authentication\" x=\"260\" y=\"10\" w=\"400\" h=\"560\""));
            } else if f.name == CHILD_FORM {
                cfrm = cfrm.replace("width=\"640\" height=\"480\"", "width=\"640\" height=\"480\" requires-sign-in=\"true\"");
                assert!(cfrm.contains("requires-sign-in"), "the child form is protected");
            }
            FormInput { name: f.name.clone(), cfrm, stylesheet: None }
        })
        .collect();
    BundleInput {
        project: "fixture".into(),
        program_multi: Some(multi.wasm),
        program_single: Some(single.wasm),
        forms,
        stylesheet: None,
        menus: Vec::new(),
        assets: Vec::new(),
        player,
        options: BundleOptions { isolation: Isolation::RequireCorp, backend_origins: vec![backend.to_owned()], ..Default::default() },
    }
}

/// Helpers the test evaluates in the page; the application is inside `#pr-app`'s shadow root.
const HELPERS: &str = r#"
window.__s = {
  root() { return document.getElementById('pr-app').shadowRoot; },
  forms() { return [...this.root().querySelectorAll('[data-pr-form]')].map(e => e.dataset.prForm); },
  has(name) { return this.forms().includes(name); },
  auth() { const c = this.root().querySelector('[data-pr-form="main-form"] [data-name="AUTH-1"]'); return c && (c.matches('.pr-auth') ? c : c.querySelector('.pr-auth')); },
  screen() { const a = this.auth(); return a ? a.dataset.authScreen : null; },
  live() { const l = this.root().querySelector('.pr-live'); return l ? l.textContent.replace(/ /g, ' ').trim() : null; },
  rect(e) { const b = e.getBoundingClientRect(); return { x: b.x, y: b.y, width: b.width, height: b.height }; },
  look() { return { forms: this.forms(), screen: this.screen(), hash: location.hash, live: this.live(), errors: [...this.root().querySelectorAll('.pr-error, [role=alert]')].map(e => e.textContent.trim()).filter(Boolean) }; },
};
true
"#;

fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

fn enter(page: &mut Page) -> Result<(), String> {
    page.call("Input.dispatchKeyEvent", json!({"type": "keyDown", "key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13, "nativeVirtualKeyCode": 13, "text": "\r"}))?;
    page.call("Input.dispatchKeyEvent", json!({"type": "keyUp", "key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13, "nativeVirtualKeyCode": 13}))?;
    Ok(())
}

fn click(page: &mut Page, selector_in_auth: &str) -> Result<(), String> {
    page.click_at(&format!("(() => {{ const e = __s.auth().querySelector({}); return e && __s.rect(e); }})()", q(selector_in_auth)))
}

fn fill(page: &mut Page, name: &str, text: &str) -> Result<(), String> {
    click(page, &format!("input[name={name}]"))?;
    page.eval(&format!("(() => {{ const i = __s.auth().querySelector('input[name={name}]'); i.select(); return true; }})()"))?;
    page.type_text(text)
}

fn wait_screen(page: &mut Page, screen: &str) -> Result<(), String> {
    page.wait_for(&format!("__s.screen() === {}", q(screen)), 15).map(|_| ()).map_err(|e| format!("{e} (the page shows {:?})", page.eval("__s.look()").ok()))
}

fn wait_form(page: &mut Page, name: &str, present: bool) -> Result<(), String> {
    page.wait_for(&format!("__s.has({}) === {present}", q(name)), 15).map(|_| ()).map_err(|e| format!("{e} (the page shows {:?})", page.eval("__s.look()").ok()))
}

fn sign_in(page: &mut Page) -> Result<(), String> {
    fill(page, "user", "ana")?;
    fill(page, "password", PASSWORD)?;
    enter(page)?;
    wait_screen(page, "SignedIn")
}

fn log() -> Vec<String> {
    mock::LOG.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default()
}

#[test]
fn a_protected_forms_address_without_a_session_shows_the_sign_in_and_after_it_the_requested_form() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    *mock::LOG.lock().unwrap() = Some(Vec::new());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || mock::serve(listener, 3600, None));
    let backend = format!("ws://127.0.0.1:{port}/ws");

    let player_out = support::built_player();
    let player = PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() };
    let out = assemble_bundle(&bundle_input(player, &backend)).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();

    let chrome = support::cdp::Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    let product = page.call("Browser.getVersion", json!({})).map(|v| v["product"].as_str().unwrap_or("?").to_owned()).unwrap_or_else(|e| e);
    page.resize(1000, 900).unwrap();
    let mut lines: Vec<String> = Vec::new();

    // ── 1. A cold load at the protected form's address, no session ──────────────────────────────────
    page.navigate(&format!("{}/index.html#/{CHILD_FORM}", server.url())).unwrap();
    page.eval(HELPERS).unwrap();
    page.wait_for("!!__s.auth()", 30).unwrap_or_else(|e| panic!("the application did not start: {e}"));
    wait_screen(&mut page, "SignIn").unwrap_or_else(|e| panic!("{e}"));
    std::thread::sleep(std::time::Duration::from_millis(500));
    let cold = page.eval("__s.look()").unwrap();
    lines.push(format!("  1 cold load at #/{CHILD_FORM}, signed out: {cold}"));
    assert!(!cold["forms"].as_array().unwrap().contains(&json!(CHILD_FORM)), "the protected form must not show to a signed-out user: {cold}");
    assert!(cold["forms"].as_array().unwrap().contains(&json!(MAIN_FORM)), "the sign-in form shows: {cold}");

    // ── 2. Signing in opens the form that was asked for ──────────────────────────────────────────────
    sign_in(&mut page).unwrap_or_else(|e| panic!("sign-in: {e}"));
    wait_form(&mut page, CHILD_FORM, true).unwrap_or_else(|e| panic!("after the sign-in the requested form did not open: {e}"));
    let after = page.eval("__s.look()").unwrap();
    lines.push(format!("  2 after the sign-in: {after}"));
    assert_eq!(after["hash"], json!(format!("#/{CHILD_FORM}")), "the address names the form that opened: {after}");
    assert_eq!(after["errors"], json!([]), "no error banner: {after}");

    // ── 3. Signing out closes the protected form and shows the sign-in again ─────────────────────────
    // The control sits on the main form, which the child covers: its sign-out button is reached through the DOM.
    page.eval("__s.auth().querySelector(\"[data-id='signout']\").click(); true").unwrap();
    wait_form(&mut page, CHILD_FORM, false).unwrap_or_else(|e| panic!("signing out did not close the protected form: {e}"));
    wait_screen(&mut page, "SignIn").unwrap_or_else(|e| panic!("{e}"));
    let out_ = page.eval("__s.look()").unwrap();
    lines.push(format!("  3 after the sign-out: {out_}"));

    // ── 4. A protected address typed while signed out is held until the sign-in ──────────────────────
    page.eval(&format!("location.hash = '#/{CHILD_FORM}'")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(800));
    let held = page.eval("__s.look()").unwrap();
    lines.push(format!("  4 the address typed while signed out: {held}"));
    assert!(!held["forms"].as_array().unwrap().contains(&json!(CHILD_FORM)), "still closed to a signed-out user: {held}");
    sign_in(&mut page).unwrap_or_else(|e| panic!("second sign-in: {e}"));
    wait_form(&mut page, CHILD_FORM, true).unwrap_or_else(|e| panic!("after the second sign-in the requested form did not open: {e}"));
    let second = page.eval("__s.look()").unwrap();
    lines.push(format!("  4 after the sign-in: {second}"));
    assert_eq!(second["hash"], json!(format!("#/{CHILD_FORM}")), "{second}");

    // ── 5. Signed in, the address opens the form at once (no sign-in screen in between) ──────────────
    page.eval("history.back()").unwrap();
    wait_form(&mut page, CHILD_FORM, false).unwrap_or_else(|e| panic!("Back did not close the form: {e}"));
    page.eval(&format!("location.hash = '#/{CHILD_FORM}'")).unwrap();
    wait_form(&mut page, CHILD_FORM, true).unwrap_or_else(|e| panic!("a signed-in session did not open the protected address: {e}"));
    let direct = page.eval("__s.look()").unwrap();
    lines.push(format!("  5 signed in, the address typed: {direct}"));

    let backend_saw = log().iter().filter(|l| l.starts_with("identity=")).map(|l| l.split(' ').next().unwrap_or("").to_owned()).collect::<Vec<_>>().join(", ");
    println!("Chrome {product} ({}): AC6 second half", exe.display());
    for l in &lines {
        println!("{l}");
    }
    println!("  backend saw: {backend_saw}");
    drop(page);
    server.stop();
}
