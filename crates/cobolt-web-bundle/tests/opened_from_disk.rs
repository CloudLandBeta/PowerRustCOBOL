// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A page opened from disk, in a real browser** (the operator, 2026-10-25: a double click on a Web
//! build's `index.html` showed a blank page). A page opened as `file://` has an opaque origin, and Chrome
//! refuses the application's module script, its `config.json`, its WebAssembly and every stylesheet or
//! script that carries an `integrity` attribute: nothing starts, and nothing says why. The page therefore
//! carries a notice ([`cobolt_web_bundle::notice`]) that one inline script shows only on `file://`.
//!
//! The same Build is written to a folder and opened twice in a headless Chrome. From disk, the notice must
//! be on screen — the project's own language first, the other five after it, the command to serve the
//! folder in the text the user can read. Over HTTP, from the bundle's own server, the notice must stay
//! hidden, the Content-Security-Policy must report no violation, and the application must start.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use std::time::Instant;

use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM};
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::notice::{self, Text};
use cobolt_web_bundle::{BundleOptions, Isolation, PlayerInput};
use serde_json::json;
use support::cdp::{find_chrome, Chrome, Page};

/// Collects the policy violations of a page from its first script on.
const WATCH: &str = "window.__csp = []; document.addEventListener('securitypolicyviolation', e => window.__csp.push(e.violatedDirective + ' ' + e.blockedURI));";

/// What the page shows of its notice, and whether anything else started.
const LOOK: &str = r#"(() => {
  const n = document.getElementById('pr-file-notice');
  const b = n.getBoundingClientRect();
  return {
    hidden: n.hidden,
    width: b.width,
    height: b.height,
    titles: [...n.querySelectorAll('h2')].map(h => h.textContent),
    langs: [...n.querySelectorAll('section')].map(s => s.lang),
    command: n.querySelector('pre').textContent,
    visibleText: document.body.innerText,
    started: !!document.getElementById('pr-app').shadowRoot,
    violations: window.__csp,
  };
})()"#;

/// The six texts in the order the notice shows them: the page's own language first.
fn shown_order(first: &str) -> Vec<&'static Text> {
    let mut all: Vec<&Text> = notice::TEXTS.iter().collect();
    let at = all.iter().position(|t| t.lang == first).unwrap();
    let head = all.remove(at);
    all.insert(0, head);
    all
}

/// A new page that records the policy's violations, opened on `url` and past its first paint.
fn open(chrome: &Chrome, url: &str) -> Page {
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.call("Page.addScriptToEvaluateOnNewDocument", json!({ "source": WATCH })).unwrap();
    page.navigate(url).unwrap();
    // `navigate` can answer while the blank page is still the document.
    page.wait_for("!!document.getElementById('pr-file-notice')", 30).unwrap();
    page
}

#[test]
fn a_bundle_opened_from_disk_says_why_and_served_over_http_it_runs() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let player_out = support::built_player();
    let out = support::assemble(
        PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        BundleOptions { isolation: Isolation::RequireCorp, locale: "pt-BR".into(), ..Default::default() },
    );
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());

    // From disk: the notice is on screen, in the project's language first.
    let t = Instant::now();
    let mut page = open(&chrome, &format!("file://{}/index.html", dir.path().display()));
    let disk_ms = t.elapsed().as_millis();
    let seen = page.eval(LOOK).unwrap();
    let shown = shown_order("pt-BR");
    let titles: Vec<&str> = shown.iter().map(|t| t.title).collect();
    let langs: Vec<&str> = shown.iter().map(|t| t.lang).collect();
    assert_eq!(seen["hidden"], json!(false), "the script showed the notice: {seen}");
    assert!(seen["width"].as_f64().unwrap() > 0.0 && seen["height"].as_f64().unwrap() > 0.0, "the notice has a box on screen: {seen}");
    assert_eq!(seen["titles"], json!(titles), "six titles, Portuguese first");
    assert_eq!(seen["langs"], json!(langs));
    assert_eq!(seen["command"], json!(notice::COMMAND));
    let visible = seen["visibleText"].as_str().unwrap();
    for t in &notice::TEXTS {
        assert!(visible.contains(t.title), "{}: the title is rendered, not only present in the markup", t.lang);
    }
    assert!(visible.contains(notice::URL), "the address to open is on screen");
    let disk_started = seen["started"].as_bool().unwrap();
    let disk_violations = seen["violations"].as_array().map_or(0, Vec::len);
    drop(page);

    // Served: the same bundle, the notice stays hidden, no policy violation, the application starts.
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let t = Instant::now();
    let mut page = open(&chrome, &format!("{}/index.html", server.url()));
    page.wait_for(
        &format!("(() => {{ const r = document.getElementById('pr-app').shadowRoot; const l = r && r.querySelector('[data-pr-form=\"{MAIN_FORM}\"] [data-name=\"{LBL_OUT}\"]'); return !!l && l.textContent === 'loaded'; }})()"),
        30,
    )
    .unwrap_or_else(|e| panic!("the served application did not start: {e}"));
    let served_ms = t.elapsed().as_millis();
    let seen = page.eval(LOOK).unwrap();
    assert_eq!(seen["hidden"], json!(true), "the notice stays hidden: {seen}");
    assert_eq!((seen["width"].as_f64(), seen["height"].as_f64()), (Some(0.0), Some(0.0)), "and takes no room: {seen}");
    let visible = seen["visibleText"].as_str().unwrap();
    for t in &notice::TEXTS {
        assert!(!visible.contains(t.title), "{}: a hidden notice is not on screen", t.lang);
    }
    assert_eq!(seen["violations"], json!([]), "the policy, with the notice's hash in it, reports nothing on a served page");
    let exceptions = page.exceptions.len();
    drop(page);
    server.stop();

    println!(
        "opened from disk ({}): notice on screen after {disk_ms} ms, {} languages with pt-BR first, command and address shown, application started: {disk_started}, policy violations: {disk_violations}",
        exe.display(),
        notice::TEXTS.len()
    );
    println!("served over HTTP: notice hidden (0x0), application up in {served_ms} ms, policy violations: 0, uncaught exceptions: {exceptions}");
}
