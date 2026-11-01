// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Navigation in a real browser** (WASM-011 T25): a single-page application **without** a SideMenu follows the
//! routes in its address (R4, R5, T11), an address that names no form leaves the main form shown, says so and
//! is taken back (R6, T12), and every navigation moves the focus to the form and announces its title (R7, T15).
//!
//! The fixture program (`main-form`, `child-form`) in its plain bundle, served; a headless Chrome changes the
//! address as a user does (typing, Back) and asks the page what it shows. With no Chrome on the machine
//! (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_wasm_abi::fixture::standard::{CHILD_FORM, LBL_OUT, MAIN_FORM};
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleOptions, Isolation, PlayerInput};
use serde_json::json;
use support::cdp::{find_chrome, Chrome};

/// What the page shows: the forms on it, the address, the live region's text, and where the focus is.
const LOOK: &str = r#"(() => {
  const root = document.getElementById('pr-app').shadowRoot;
  const forms = [...root.querySelectorAll('[data-pr-form]')].map(e => e.dataset.prForm);
  const live = root.querySelector('.pr-live');
  const a = root.activeElement;
  // The focused element's form: its own section, or the one in the frame whose title it is.
  const frame = a && a.closest('.pr-panelwin, .pr-dialog');
  const owner = a && (a.closest('[data-pr-form]') || (frame && frame.querySelector('[data-pr-form]')));
  return {
    forms,
    hash: location.hash,
    live: live ? live.textContent.replace(/ /g, '') : null,
    liveRole: live ? [live.getAttribute('role'), live.getAttribute('aria-live')] : null,
    focus: owner ? owner.dataset.prForm : null,
    focusIs: a ? (a.classList.contains('pr-title') ? 'title' : a.classList.contains('pr-form-root') ? 'root' : a.className) : null,
    owner: document.documentElement.getAttribute('data-pr-address-owner'),
    text: root.textContent,
  };
})()"#;

fn has_form(name: &str, present: bool) -> String {
    format!("(() => {{ const r = document.getElementById('pr-app').shadowRoot; return !!r.querySelector('[data-pr-form=\"{name}\"]') === {present}; }})()")
}

#[test]
fn an_spa_without_a_shell_follows_its_routes_takes_a_bad_one_back_and_announces_every_form() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let player_out = support::built_player();
    let input = support::input_with(
        PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    );
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1024, 768).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for(
        &format!("(() => {{ const r = document.getElementById('pr-app').shadowRoot; const l = r && r.querySelector('[data-pr-form=\"{MAIN_FORM}\"] [data-name=\"{LBL_OUT}\"]'); return !!l && l.textContent === 'loaded'; }})()"),
        30,
    )
    .unwrap_or_else(|e| panic!("the application did not start: {e}"));

    // The start: the main form is announced and holds the focus; a page-wide application owns the address.
    let start = page.eval(LOOK).unwrap();
    assert_eq!(start["live"], json!(MAIN_FORM), "the main form's title is announced: {start}");
    assert_eq!(start["liveRole"], json!(["status", "polite"]), "{start}");
    assert_eq!((&start["focus"], &start["focusIs"]), (&json!(MAIN_FORM), &json!("root")), "the focus is on the main form, labelled by its title: {start}");
    assert_eq!(start["owner"], json!(""), "this application owns the address: {start}");

    // A route typed into the address: no SideMenu, so the form opens as a panel over the main form (T11).
    page.eval(&format!("location.hash = '#/{CHILD_FORM}'")).unwrap();
    page.wait_for(&has_form(CHILD_FORM, true), 15).unwrap_or_else(|e| panic!("the route was not followed: {e}; {}", page.eval(LOOK).unwrap()));
    let routed = page.eval(LOOK).unwrap();
    assert!(routed["forms"].as_array().unwrap().contains(&json!(MAIN_FORM)), "the main form stays under it: {routed}");
    assert_eq!(routed["hash"], json!(format!("#/{CHILD_FORM}")), "{routed}");
    assert_eq!(routed["live"], json!(CHILD_FORM), "the form that arrived is announced: {routed}");
    assert_eq!((&routed["focus"], &routed["focusIs"]), (&json!(CHILD_FORM), &json!("title")), "and its title takes the focus: {routed}");

    // Back closes it.
    page.eval("history.back()").unwrap();
    page.wait_for(&has_form(CHILD_FORM, false), 15).unwrap_or_else(|e| panic!("Back did not close the form: {e}; {}", page.eval(LOOK).unwrap()));
    let back = page.eval(LOOK).unwrap();
    assert_eq!(back["hash"], json!(""), "{back}");

    // An address that names no form (T12): the main form stays, the notice says so, the address is taken back.
    page.eval("location.hash = '#/no-such-form'").unwrap();
    page.wait_for("location.hash === ''", 15).unwrap_or_else(|e| panic!("the bad route was not taken back: {e}; {}", page.eval(LOOK).unwrap()));
    let bad = page.eval(LOOK).unwrap();
    assert_eq!(bad["forms"], json!([MAIN_FORM]), "only the main form: {bad}");
    assert!(bad["text"].as_str().unwrap().contains("The application has no form named no-such-form."), "the notice, in the page's language: {bad}");
    drop(page);
    server.stop();

    println!(
        "navigation in Chrome ({}): start announced `{}` with the focus on it; `#/{CHILD_FORM}` opened it over the main form, announced and focused; Back closed it (hash empty); `#/no-such-form` left the main form, showed the notice and took the address back",
        exe.display(),
        start["live"]
    );
}
