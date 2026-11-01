// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Authentication control in a real browser** (WASM-009 T25; AC1, AC5, AC7).
//!
//! The fixture program's main form gains an Authentication control; the bundle is served by the bundle's own
//! server and the mock backend (`examples/mock_backend.rs`, run in this process) answers the control's `/auth/*`
//! requests and the program's WebSocket. A headless Chrome acts as a user: it clicks, types and presses Tab.
//!
//! * **AC1** — on every screen the control can show, the browser's accessibility tree names every input and
//!   button, the control has exactly one live region, the primary action is reached with Tab, and Tab leaves the
//!   control again (no focus trap).
//! * **AC5** — after a sign-in no token is in `localStorage` or `sessionStorage`, the refresh cookie is
//!   `HttpOnly` (absent from `document.cookie`); a token that is about to expire is refreshed with exactly one
//!   `POST /auth/refresh`, after which the program's backend call goes through signed in.
//! * **AC7** — the console of the whole flow holds no token and no password.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

#[allow(dead_code)]
#[path = "../examples/mock_backend.rs"]
mod mock;

use std::net::TcpListener;
use std::time::{Duration, Instant};

use base64::Engine;
use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM};
use cobolt_wasm_abi::fixture::{build, standard_cfrm, standard_spec};
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput};
use serde_json::json;
use support::cdp::{find_chrome, Page};

const AUTH: &str = "AUTH-1";
const PASSWORD: &str = "s3cret!";
/// The mock's first token lives 61 s: the control refreshes a token in its last minute, so once, at once.
const FIRST_TTL: u64 = 61;

/// The fixture with the control on its main form, beside the column of standard controls.
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
window.__a = {
  root() { return document.getElementById('pr-app').shadowRoot; },
  ctl(id) { return this.root().querySelector('[data-pr-form="main-form"] [data-name="' + id + '"]'); },
  auth() { const c = this.ctl('AUTH-1'); return c && (c.matches('.pr-auth') ? c : c.querySelector('.pr-auth')); },
  screen() { const a = this.auth(); return a ? a.dataset.authScreen : null; },
  active() { let a = document.activeElement; while (a && a.shadowRoot && a.shadowRoot.activeElement) a = a.shadowRoot.activeElement; return a; },
  inAuth() { const a = this.active(); return !!a && this.auth().contains(a); },
  describe(e) { return e ? e.tagName.toLowerCase() + (e.name ? '[name=' + e.name + ']' : '') + (e.type ? '[type=' + e.type + ']' : '') + (e.dataset && e.dataset.id ? '[data-id=' + e.dataset.id + ']' : '') : 'nothing'; },
  primary() { const a = this.auth(); return a.querySelector('button[type=submit]') || a.querySelector('.pr-auth-primary') || a.querySelector('button'); },
  rect(e) { const b = e.getBoundingClientRect(); return { x: b.x, y: b.y, width: b.width, height: b.height }; },
};
true
"#;

fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

/// One key as a keyboard presses it, with the codes Chrome needs to act on it (Tab moves the focus, Enter submits).
fn key(page: &mut Page, key: &str, code: i64) -> Result<(), String> {
    let text = if key == "Enter" { "\r" } else { "" };
    page.call("Input.dispatchKeyEvent", json!({"type": "keyDown", "key": key, "code": key, "windowsVirtualKeyCode": code, "nativeVirtualKeyCode": code, "text": text}))?;
    page.call("Input.dispatchKeyEvent", json!({"type": "keyUp", "key": key, "code": key, "windowsVirtualKeyCode": code, "nativeVirtualKeyCode": code}))?;
    Ok(())
}

fn click(page: &mut Page, selector_in_auth: &str) -> Result<(), String> {
    page.click_at(&format!("(() => {{ const e = __a.auth().querySelector({}); return e && __a.rect(e); }})()", q(selector_in_auth)))
}

/// Type `text` into the control's input `name`, focused as a click focuses it.
fn fill(page: &mut Page, name: &str, text: &str) -> Result<(), String> {
    click(page, &format!("input[name={name}]"))?;
    page.eval(&format!("(() => {{ const i = __a.auth().querySelector('input[name={name}]'); i.select(); return true; }})()"))?;
    page.type_text(text)
}

fn wait_screen(page: &mut Page, screen: &str) -> Result<(), String> {
    page.wait_for(&format!("__a.screen() === {}", q(screen)), 10).map(|_| ()).map_err(|e| format!("{e} (the screen is {:?})", page.eval("__a.screen()").ok()))
}

/// The AC1 rules on the screen shown now: what fails, by node.
fn check_screen(page: &mut Page) -> Result<Vec<String>, String> {
    let mut failures = Vec::new();
    // The browser's own accessibility tree of the control.
    let object = page.call("Runtime.evaluate", json!({"expression": "__a.auth()", "returnByValue": false}))?["result"]["objectId"].as_str().ok_or("the control is not on the page")?.to_owned();
    for role in ["textbox", "button"] {
        let nodes = page.call("Accessibility.queryAXTree", json!({"objectId": object, "role": role}))?;
        for n in nodes["nodes"].as_array().cloned().unwrap_or_default() {
            if n["ignored"] == json!(true) {
                continue;
            }
            let name = n["name"]["value"].as_str().unwrap_or("").trim().to_owned();
            if name.is_empty() {
                failures.push(format!("a {role} has no accessible name (backend node {})", n["backendDOMNodeId"]));
            }
        }
    }
    let group = page.call("Accessibility.queryAXTree", json!({"objectId": object, "role": "group"}))?;
    if !group["nodes"].as_array().is_some_and(|a| a.iter().any(|n| !n["name"]["value"].as_str().unwrap_or("").is_empty())) {
        failures.push("the control's group has no accessible name".into());
    }
    // One live region, where every message of the control is announced.
    let live = page.eval("__a.auth().querySelectorAll('[role=alert],[role=status],[aria-live]').length")?;
    if live != json!(1) {
        failures.push(format!("{live} live regions, not 1"));
    }
    // The primary action is reached from the first field (or the control's first focusable) with Tab.
    page.eval("(() => { const f = __a.auth().querySelector('input, button'); f.focus(); return true; })()")?;
    let mut reached = page.eval("__a.active() === __a.primary()")? == json!(true);
    for _ in 0..12 {
        if reached {
            break;
        }
        key(page, "Tab", 9)?;
        reached = page.eval("__a.active() === __a.primary()")? == json!(true);
    }
    if !reached {
        failures.push(format!("Tab never reaches the primary action {}", page.eval("__a.describe(__a.primary())")?));
    }
    // And Tab leaves the control again.
    let mut left = false;
    for _ in 0..30 {
        key(page, "Tab", 9)?;
        if page.eval("__a.inAuth()")? == json!(false) {
            left = true;
            break;
        }
    }
    if !left {
        failures.push(format!("Tab never leaves the control: the focus stays on {}", page.eval("__a.describe(__a.active())")?));
    }
    Ok(failures)
}

fn log() -> Vec<String> {
    mock::LOG.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default()
}

fn count(prefix: &str) -> usize {
    log().iter().filter(|l| l.starts_with(prefix)).count()
}

#[test]
fn the_control_is_accessible_keeps_no_token_in_the_page_and_refreshes_once() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    *mock::LOG.lock().unwrap() = Some(Vec::new());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || mock::serve(listener, 3600, Some(FIRST_TTL)));
    let backend = format!("ws://127.0.0.1:{port}/ws");

    let player_out = support::built_player();
    let player = PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() };
    let out = assemble_bundle(&bundle_input(player, &backend)).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();

    let chrome = support::cdp::Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let version = exe.display().to_string();
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    let product = page.call("Browser.getVersion", json!({})).map(|v| v["product"].as_str().unwrap_or("?").to_owned()).unwrap_or_else(|e| e);
    page.call("Accessibility.enable", json!({})).unwrap();
    page.resize(1000, 900).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.eval(HELPERS).unwrap();
    // A line the console capture must see, so an empty search result means a clean console, not a deaf one.
    page.eval("console.log('pr-canary'); true").unwrap();
    page.wait_for(&format!("(() => {{ const l = __a.ctl({}); return !!l && l.textContent === 'loaded' && !!__a.auth(); }})()", q(LBL_OUT)), 30)
        .unwrap_or_else(|e| panic!("the application did not start: {e}"));
    let mut failed: Vec<String> = Vec::new();
    let mut lines: Vec<String> = Vec::new();

    // ── AC1: every screen ───────────────────────────────────────────────────────────────────────────
    let mut screens = 0;
    let mut rule = |page: &mut Page, screen: &str, failed: &mut Vec<String>, lines: &mut Vec<String>| {
        if let Err(e) = wait_screen(page, screen) {
            failed.push(format!("{screen}: {e}"));
            return;
        }
        screens += 1;
        match check_screen(page) {
            Ok(f) if f.is_empty() => lines.push(format!("  pass  AC1 {screen}: inputs and buttons named, one live region, Tab reaches the action and leaves")),
            Ok(f) => {
                for x in &f {
                    failed.push(format!("AC1 {screen}: {x}"));
                }
                lines.push(format!("  FAIL  AC1 {screen}: {}", f.join("; ")));
            }
            Err(e) => failed.push(format!("AC1 {screen}: {e}")),
        }
    };
    let steps: Vec<(&str, Box<dyn Fn(&mut Page) -> Result<(), String>>)> = vec![
        ("SignIn", Box::new(|_| Ok(()))),
        ("SignUp", Box::new(|p| click(p, "[data-id='show:SignUp']"))),
        ("Confirm", Box::new(|p| click(p, "[data-id='show:Confirm']"))),
        ("SignIn", Box::new(|p| click(p, "[data-id='show:SignIn']"))),
        ("Forgot", Box::new(|p| click(p, "[data-id='show:Forgot']"))),
        (
            "Reset",
            Box::new(|p| {
                fill(p, "user", "ana")?;
                key(p, "Enter", 13)
            }),
        ),
        ("SignIn", Box::new(|p| click(p, "[data-id='show:SignIn']"))),
        (
            "Mfa",
            Box::new(|p| {
                fill(p, "user", "mfa")?;
                fill(p, "password", PASSWORD)?;
                key(p, "Enter", 13)
            }),
        ),
        ("SignIn", Box::new(|p| click(p, "[data-id='show:SignIn']"))),
    ];
    for (screen, go) in &steps {
        if let Err(e) = go(&mut page) {
            failed.push(format!("reaching {screen}: {e}"));
            continue;
        }
        rule(&mut page, screen, &mut failed, &mut lines);
    }

    // ── AC5: sign in; storage and cookie; one refresh; the call goes through ────────────────────────
    let refreshes_before = count("identity=refresh");
    let signed_in = fill(&mut page, "user", "ana").and_then(|_| fill(&mut page, "password", PASSWORD)).and_then(|_| key(&mut page, "Enter", 13)).and_then(|_| wait_screen(&mut page, "SignedIn"));
    match signed_in {
        Err(e) => failed.push(format!("AC5 sign-in: {e}")),
        Ok(()) => {
            rule(&mut page, "SignedIn", &mut failed, &mut lines);
            let storage = page
                .eval("JSON.stringify({ local: Object.entries(localStorage), session: Object.entries(sessionStorage), cookie: document.cookie })")
                .unwrap_or_else(|e| json!(e));
            let storage = storage.as_str().unwrap_or("").to_owned();
            lines.push(format!("  AC5 storage after sign-in: {storage}"));
            let head = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
            if storage.contains(&head) || storage.contains("pr_refresh") || storage.contains(PASSWORD) {
                failed.push(format!("AC5: the page's storage holds a token, the refresh cookie or the password: {storage}"));
            }
            let cookies = page.call("Network.getAllCookies", json!({})).unwrap_or_else(|e| json!({"error": e}));
            let refresh = cookies["cookies"].as_array().and_then(|a| a.iter().find(|c| c["name"] == "pr_refresh").cloned());
            match refresh {
                Some(c) if c["httpOnly"] == json!(true) => lines.push(format!("  pass  AC5 the refresh cookie is HttpOnly, SameSite={}", c["sameSite"].as_str().unwrap_or("?"))),
                Some(c) => failed.push(format!("AC5: the refresh cookie is not HttpOnly: {c}")),
                None => failed.push(format!("AC5: no refresh cookie was set: {cookies}")),
            }
            // The first token expires within the minute: exactly one refresh, and no second one.
            let deadline = Instant::now() + Duration::from_secs(15);
            while count("identity=refresh") == refreshes_before && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(100));
            }
            std::thread::sleep(Duration::from_secs(4));
            let refreshes = count("identity=refresh") - refreshes_before;
            lines.push(format!("  AC5 refreshes after the sign-in: {refreshes}"));
            if refreshes != 1 {
                failed.push(format!("AC5: {refreshes} refreshes of a token about to expire, not 1"));
            }
            let calls_before = count("call: a program request (signed in)");
            let went = page.click_at(&format!("__a.rect(__a.ctl({}))", q("BTN-REMOTE"))).and_then(|_| page.wait_for(&format!("(__a.ctl({}).textContent || '').includes('pong')", q(LBL_OUT)), 20));
            match went {
                Ok(_) if count("call: a program request (signed in)") > calls_before => lines.push("  pass  AC5 after the refresh the program's call goes through, signed in".into()),
                Ok(_) => failed.push(format!("AC5: the call was answered but not on a signed-in connection: {:?}", log())),
                Err(e) => failed.push(format!("AC5: the call after the refresh did not go through: {e}")),
            }
            if count("auth message: token refused") > 0 {
                failed.push("AC5: the backend refused a token the control gave the connection".into());
            }
            if let Err(e) = click(&mut page, "[data-id='signout']").and_then(|_| wait_screen(&mut page, "SignIn")) {
                failed.push(format!("sign-out: {e}"));
            }
        }
    }

    // ── AC7: the console of the whole flow ─────────────────────────────────────────────────────────
    let head = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
    let mut text: Vec<String> = page.console.clone();
    text.extend(page.exceptions.iter().cloned());
    let all = text.join("\n");
    if !all.contains("pr-canary") {
        failed.push("AC7: the console capture did not see the test's own line".into());
    }
    let searched = [("a token", head.as_str()), ("the password", PASSWORD), ("the refresh cookie", "pr_refresh"), ("the MFA session", "mfa-session")];
    for (what, needle) in searched {
        let hits = all.matches(needle).count();
        lines.push(format!("  AC7 console ({} lines) searched for {what}: {hits} hits", text.len()));
        if hits > 0 {
            failed.push(format!("AC7: the console holds {what}"));
        }
    }

    println!("Chrome {product} ({version}): {screens} screens checked, {} failures", failed.len());
    for l in &lines {
        println!("{l}");
    }
    println!("  backend saw: {}", log().iter().filter(|l| l.starts_with("identity=")).map(|l| l.split(' ').next().unwrap_or("")).collect::<Vec<_>>().join(", "));
    drop(page);
    server.stop();
    assert!(failed.is_empty(), "failed in Chrome:\n{}", failed.join("\n"));
}
