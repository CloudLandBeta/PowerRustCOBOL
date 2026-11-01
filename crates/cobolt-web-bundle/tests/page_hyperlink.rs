// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Hyperlink control in a real browser** (WASM-006 T18 and the Chrome half of T19b).
//!
//! What the browser decides and a unit test cannot see: the anchor and its computed cursor (AC1); a click per `Target`, the
//! modifier keys and the middle button opening a new tab or window, and what the click event reports (AC2); a new tab with
//! an empty `Rel` having no `window.opener`, and one with `Rel` set as set (AC3); `mailto:`, `tel:`, a relative address, a
//! fragment and a `Download` written as `<a>` writes them, and the fragment followed (AC5); a refused address left inert,
//! also when a handler's `SetUrl` asks for it (AC4); `Cancel` set by a handler stopping the navigation and its absence not
//! (AC6); the keyboard and the accessibility tree (AC8); and `onNavigated` running before a same-tab link leaves the
//! application (T19b, OQ1 decided by the operator 2026-10-31).
//!
//! The fixture program's main form gains a column of Hyperlinks, some with scripted handlers. The page they open is a
//! `landing.html` served beside the bundle, so no test leaves the machine. A headless Chrome clicks with real mouse events and
//! keys and asks the browser, and Chrome's own list of pages, what happened.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why. WebKit is not driven here: the
//! project's harness has no WebKit driver, so Safari's half of the platform table is measured by its unit tests only.

mod support;

use std::time::{Duration, Instant};

use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM, TXT_IN};
use cobolt_wasm_abi::fixture::{build, standard_cfrm, standard_spec, FixtureHandler, Step, Target};
use cobolt_web_bundle::devserver::{serve, ServeOptions, Server};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput};
use serde_json::{json, Value};
use support::cdp::{find_chrome, Chrome, Page};

/// The links added to the main form: `(name, properties)`.
const LINKS: &[(&str, &[(&str, &str)])] = &[
    ("LNK-EX", &[("Caption", "Example"), ("Url", "https://example.com/"), ("Tooltip", "The example site"), ("HrefLang", "en"), ("MimeType", "text/html")]),
    ("LNK-TAB", &[("Caption", "New tab"), ("Url", "/landing.html?tab"), ("Target", "NewTab")]),
    ("LNK-OPENER", &[("Caption", "With opener"), ("Url", "/landing.html?opener"), ("Target", "NewTab"), ("Rel", "opener")]),
    ("LNK-SELF", &[("Caption", "Same tab"), ("Url", "/landing.html?self"), ("Target", "Self"), ("AccessibleLabel", "Open the landing page here")]),
    ("LNK-MAIL", &[("Caption", "Mail"), ("Url", "mailto:sales@example.com?subject=Order%2042&body=Hello%20there")]),
    ("LNK-TEL", &[("Caption", "Call"), ("Url", "tel:+15551234567")]),
    ("LNK-REL", &[("Caption", "Relative"), ("Url", "landing.html?relative")]),
    ("LNK-FRAG", &[("Caption", "To the box"), ("Url", "#txt-in")]),
    ("LNK-DL", &[("Caption", "Report"), ("Url", "/report.txt"), ("Download", "report-2026.txt")]),
    ("LNK-JS", &[("Caption", "Refused"), ("Url", "javascript:alert(1)")]),
    ("LNK-CANCEL", &[("Caption", "Cancelled"), ("Url", "/landing.html?cancel"), ("Target", "NewTab")]),
    ("LNK-MODS", &[("Caption", "Reports keys"), ("Url", "/landing.html?mods"), ("Target", "NewTab")]),
    ("LNK-LEAVE", &[("Caption", "Leaves"), ("Url", "/landing.html?left"), ("Target", "Self")]),
];

/// The fixture program with the links on its main form and their handlers: LNK-CANCEL's `onClick` sets `Cancel`; LNK-MODS's
/// `onClick` writes the modifier text it is given into LBL-OUT; LNK-LEAVE's `onBeforeNavigate` and `onNavigated` write their
/// argument there; BTN-SETJS asks LNK-TAB to take a `javascript:` address.
fn bundle_input(player: PlayerInput) -> BundleInput {
    let out = || Target::control(LBL_OUT);
    let spec = |multi: bool| {
        let mut s = standard_spec(multi);
        let main = s.forms.iter_mut().find(|f| f.name == MAIN_FORM).unwrap();
        for (name, _) in LINKS {
            main.controls.push(((*name).into(), "Hyperlink".into()));
        }
        main.controls.push(("BTN-SETJS".into(), "Button".into()));
        main.handlers.extend([
            FixtureHandler::on(
                "LNK-CANCEL",
                "onClick",
                vec![
                    Step::Set { target: Target::control("LNK-CANCEL"), name: "Cancel".into(), value: "1".into() },
                    Step::Set { target: out(), name: "Caption".into(), value: "cancelled".into() },
                ],
            ),
            FixtureHandler::on("LNK-MODS", "onClick", vec![Step::EchoArg0 { target: out(), name: "Caption".into() }]),
            FixtureHandler::on("LNK-LEAVE", "onBeforeNavigate", vec![Step::EchoArg0 { target: out(), name: "Caption".into() }]),
            FixtureHandler::on("LNK-LEAVE", "onNavigated", vec![Step::EchoArg0 { target: out(), name: "Caption".into() }]),
            FixtureHandler::on("BTN-SETJS", "onClick", vec![Step::Call { target: Target::control("LNK-TAB"), name: "SetUrl".into(), args: vec!["javascript:alert(1)".into()] }]),
        ]);
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
                cfrm = with_link_properties(&cfrm);
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
        options: BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    }
}

/// The main form's `.cfrm` with each link's properties written inside its `<Control>`, the form made tall enough for the
/// column, and `index-base="1"` (a form without it is read as saved when indexes counted from 0).
fn with_link_properties(cfrm: &str) -> String {
    let mut xml = cfrm.replace("width=\"640\" height=\"480\"", "width=\"640\" height=\"1200\" index-base=\"1\"");
    for (name, props) in LINKS {
        let open = format!("<Control id=\"{name}\" ");
        let at = xml.find(&open).unwrap_or_else(|| panic!("{name} is on the form"));
        let close = at + xml[at..].find("></Control>").unwrap() + 1;
        let body: String = props.iter().map(|(k, v)| format!("<Property name=\"{k}\">{}</Property>", v.replace('&', "&amp;"))).collect();
        xml.insert_str(close, &body);
    }
    xml
}

/// The bundle served with its landing page and report file, and a page of `chrome` on it once the program's `onLoad` ran.
fn open_links(chrome: &Chrome) -> (Page, Server, tempfile::TempDir) {
    let player_out = support::built_player();
    let input = bundle_input(PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() });
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    std::fs::write(dir.path().join("landing.html"), "<!doctype html><meta charset=\"utf-8\"><title>landing</title><p id=\"landed\">landed</p>\n").unwrap();
    std::fs::write(dir.path().join("report.txt"), "report\n").unwrap();
    let path = dir.path().to_owned();
    let server = serve(&path, "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(900, 1300).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for(&js(&format!("const l = el('{LBL_OUT}'); return !!l && l.textContent === 'loaded';")), 30).unwrap_or_else(|e| panic!("the application did not start: {e}"));
    (page, server, dir)
}

const LIB: &str = r#"
  const root = document.getElementById('pr-app').shadowRoot;
  const el = id => root.querySelector('[data-pr-form="main-form"] [data-name="' + id + '"]');
  const a = id => { const e = el(id); return e && (e.tagName === 'A' ? e : e.querySelector('a')); };
  const rect = id => { const r = a(id).getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; };
"#;

fn js(body: &str) -> String {
    format!("(() => {{ {LIB} {body} }})()")
}

/// A click on link `id` with the CDP modifier bits (Alt 1, Ctrl 2, Meta 4, Shift 8) and button (`left`, `middle`).
fn click(page: &mut Page, id: &str, modifiers: u32, button: &str) {
    let r = page.eval(&js(&format!("return rect('{id}');"))).unwrap();
    let (x, y) = (r["x"].as_f64().unwrap() + r["width"].as_f64().unwrap() / 2.0, r["y"].as_f64().unwrap() + r["height"].as_f64().unwrap() / 2.0);
    let buttons = if button == "middle" { 4 } else { 1 };
    page.call("Input.dispatchMouseEvent", json!({"type": "mouseMoved", "x": x, "y": y, "modifiers": modifiers})).unwrap();
    page.call("Input.dispatchMouseEvent", json!({"type": "mousePressed", "x": x, "y": y, "button": button, "buttons": buttons, "clickCount": 1, "modifiers": modifiers})).unwrap();
    page.call("Input.dispatchMouseEvent", json!({"type": "mouseReleased", "x": x, "y": y, "button": button, "buttons": 0, "clickCount": 1, "modifiers": modifiers})).unwrap();
}

/// The ids of the pages Chrome has open.
fn page_ids(chrome: &Chrome) -> Vec<String> {
    chrome.pages().unwrap().iter().filter_map(|p| p["id"].as_str().map(str::to_owned)).collect()
}

/// The page opened since `before` whose address contains `marker`, waited for at most `secs`; `None` when none came.
fn opened(chrome: &Chrome, before: &[String], marker: &str, secs: u64) -> Option<Value> {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(p) = chrome.pages().unwrap().into_iter().find(|p| !before.iter().any(|b| p["id"] == b.as_str()) && p["url"].as_str().is_some_and(|u| u.contains(marker))) {
            return Some(p);
        }
        if Instant::now() > deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The browser window that holds page `target`.
fn window_of(page: &mut Page, target: &str) -> i64 {
    page.call("Browser.getWindowForTarget", json!({"targetId": target})).unwrap()["windowId"].as_i64().unwrap()
}

/// The landing page opened at `p`: whether it has a `window.opener`, then closed.
fn has_opener_and_close(chrome: &Chrome, p: &Value) -> bool {
    let mut tab = chrome.attach(p["webSocketDebuggerUrl"].as_str().unwrap()).unwrap();
    tab.wait_for("document.readyState === 'complete' && !!document.getElementById('landed')", 10).unwrap();
    tab.eval("window.opener !== null").unwrap() == json!(true)
}

fn main_id(chrome: &Chrome) -> String {
    chrome.pages().unwrap().into_iter().find(|p| p["url"].as_str().is_some_and(|u| u.contains("/index.html"))).unwrap()["id"].as_str().unwrap().to_owned()
}

/// AC1, AC4 (on the page), AC5: what the anchors are, as the browser reads them.
#[test]
fn a_hyperlink_is_an_anchor_with_the_hand_and_writes_what_a_writes() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, server, _dir) = open_links(&chrome);
    let look = page
        .eval(&js(
            "const out = {};
             for (const id of ['LNK-EX', 'LNK-TAB', 'LNK-OPENER', 'LNK-SELF', 'LNK-MAIL', 'LNK-TEL', 'LNK-REL', 'LNK-FRAG', 'LNK-DL', 'LNK-JS']) {
               const x = a(id);
               out[id] = x && { tag: x.tagName, text: x.textContent.trim(), cursor: getComputedStyle(x).cursor, href: x.getAttribute('href'), resolved: x.href,
                 target: x.getAttribute('target'), rel: x.getAttribute('rel'), download: x.getAttribute('download'), hreflang: x.getAttribute('hreflang'),
                 type: x.getAttribute('type'), title: el(id).getAttribute('title') || x.getAttribute('title'), aria: x.getAttribute('aria-label'),
                 ping: x.hasAttribute('ping'), refused: x.getAttribute('data-refused'), disabled: x.getAttribute('aria-disabled') };
             }
             return out;",
        ))
        .unwrap();
    println!("{}", serde_json::to_string_pretty(&look).unwrap());
    let ex = &look["LNK-EX"];
    assert_eq!((ex["tag"].clone(), ex["text"].clone(), ex["cursor"].clone()), (json!("A"), json!("Example"), json!("pointer")), "AC1: {ex}");
    assert_eq!(ex["href"], json!("https://example.com/"));
    assert_eq!((ex["hreflang"].clone(), ex["type"].clone(), ex["title"].clone()), (json!("en"), json!("text/html"), json!("The example site")));
    for (id, v) in look.as_object().unwrap() {
        assert_eq!(v["ping"], json!(false), "{id}: no ping (R3)");
    }
    assert_eq!((look["LNK-TAB"]["target"].clone(), look["LNK-TAB"]["rel"].clone()), (json!("_blank"), json!("noopener noreferrer")), "R8");
    assert_eq!(look["LNK-OPENER"]["rel"], json!("opener"), "an explicit Rel is written as set");
    assert_eq!(look["LNK-SELF"]["aria"], json!("Open the landing page here"), "R4");
    assert_eq!(look["LNK-MAIL"]["href"], json!("mailto:sales@example.com?subject=Order%2042&body=Hello%20there"), "AC5 mailto");
    assert_eq!(look["LNK-TEL"]["href"], json!("tel:+15551234567"), "AC5 tel");
    assert_eq!(look["LNK-REL"]["resolved"], json!(format!("{}/landing.html?relative", server.url())), "AC5 relative, resolved by the browser");
    assert_eq!(look["LNK-FRAG"]["href"], json!("#txt-in"));
    assert_eq!(look["LNK-DL"]["download"], json!("report-2026.txt"), "AC5 Download");
    let js_link = &look["LNK-JS"];
    assert_eq!((js_link["href"].clone(), js_link["disabled"].clone()), (Value::Null, json!("true")), "AC4: a refused address is inert: {js_link}");
    assert!(js_link["refused"].as_str().is_some_and(|m| !m.is_empty()), "and says why");

    // AC5 fragment: the box named by `#txt-in` (whatever the case) is scrolled to and focused.
    click(&mut page, "LNK-FRAG", 0, "left");
    page.wait_for(&js(&format!("const t = el('{TXT_IN}'); return !!root.activeElement && (root.activeElement === t || t.contains(root.activeElement));")), 10)
        .unwrap_or_else(|e| panic!("the fragment link did not focus {TXT_IN}: {e}"));

    // AC4 at run time: a handler's SetUrl("javascript:…") leaves the link as it was.
    let go = "const b = el('BTN-SETJS'); const r = b.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height };";
    page.click_at(&js(go)).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(page.eval(&js("return a('LNK-TAB').getAttribute('href');")).unwrap(), json!("/landing.html?tab"), "SetUrl(javascript:) left the address unchanged");
    let url = server.url();
    drop(page);
    server.stop();
    println!(
        "web Hyperlink in Chrome ({}): an <a> reading 'Example' with cursor pointer; hreflang, type, title, aria-label written, no ping; NewTab with empty Rel = rel 'noopener noreferrer', an explicit Rel as set; mailto with subject and body, tel, a relative address resolved to {url}/landing.html?relative and a Download name written as <a> writes them; javascript: inert with its reason, and SetUrl(javascript:) refused; #txt-in focused the box",
        exe.display(),
    );
}

/// AC2, AC3: each Target, the modifier keys and the middle button; `window.opener` of what opened.
#[test]
fn a_click_follows_the_target_and_the_modifiers_and_a_new_tab_has_no_opener_unless_rel_says_so() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, server, _dir) = open_links(&chrome);
    let main = main_id(&chrome);
    let main_window = window_of(&mut page, &main);
    // The new-tab key of the platform the page runs on (Chrome: ⌘ on macOS, Ctrl elsewhere).
    let new_tab_key = if cfg!(target_os = "macos") { 4 } else { 2 };
    let mut report = Vec::new();

    // NewTab, empty Rel: a new tab with no opener (AC3).
    let before = page_ids(&chrome);
    click(&mut page, "LNK-TAB", 0, "left");
    let p = opened(&chrome, &before, "?tab", 10).expect("NewTab opened a page");
    let w = window_of(&mut page, p["id"].as_str().unwrap());
    let opener = has_opener_and_close(&chrome, &p);
    assert!(!opener, "AC3: a new tab with an empty Rel has no window.opener");
    report.push(format!("NewTab: new page (window {w}, main {main_window}), opener {opener}"));

    // NewTab, Rel = opener: as set.
    let before = page_ids(&chrome);
    click(&mut page, "LNK-OPENER", 0, "left");
    let p = opened(&chrome, &before, "?opener", 10).expect("NewTab with Rel opener opened a page");
    let opener = has_opener_and_close(&chrome, &p);
    assert!(opener, "AC3: Rel = opener keeps window.opener");
    report.push(format!("NewTab Rel=opener: opener {opener}"));

    // Self: the platform's new-tab key, the middle button and Shift each open the page elsewhere and leave this one.
    for (what, mods, button) in [("new-tab key", new_tab_key, "left"), ("middle button", 0, "middle"), ("Shift", 8, "left")] {
        let before = page_ids(&chrome);
        click(&mut page, "LNK-SELF", mods, button);
        let p = opened(&chrome, &before, "?self", 10).unwrap_or_else(|| panic!("{what}-click on a Self link opened no page"));
        let w = window_of(&mut page, p["id"].as_str().unwrap());
        has_opener_and_close(&chrome, &p);
        assert!(page.eval("location.pathname").unwrap() == json!("/index.html"), "{what}: the application stayed in its tab");
        if what == "Shift" {
            assert_ne!(w, main_window, "Shift-click opens a new window");
        } else {
            assert_eq!(w, main_window, "{what}-click opens a tab of the same window");
        }
        report.push(format!("Self + {what}: new page in window {w}"));
    }

    // The click event reports the modifiers (an intercepted link: its onClick writes its argument), then the link opens.
    let before = page_ids(&chrome);
    click(&mut page, "LNK-MODS", new_tab_key, "left");
    let mods = page.wait_for(&js(&format!("const t = el('{LBL_OUT}').textContent; return t !== 'loaded' && t;")), 10).unwrap();
    let p = opened(&chrome, &before, "?mods", 10).expect("the intercepted NewTab link opened a page after its handler");
    let opener = has_opener_and_close(&chrome, &p);
    assert_eq!(mods, json!(if cfg!(target_os = "macos") { "Meta" } else { "Ctrl" }), "onClick's argument names the key held");
    assert!(!opener, "a scripted new tab is isolated too");
    report.push(format!("intercepted NewTab: onClick got {mods}, opened, opener {opener}"));

    // Self, plain: this tab goes to the page.
    click(&mut page, "LNK-SELF", 0, "left");
    page.wait_for("location.search === '?self' && !!document.getElementById('landed')", 10).expect("a plain click on a Self link navigates this tab");
    report.push("Self plain: this tab navigated".into());
    drop(page);
    server.stop();
    println!("web Hyperlink targets in Chrome ({}): {}", exe.display(), report.join("; "));
}

/// AC6: Cancel set by onClick stops the navigation; T19b: onNavigated runs before a same-tab link leaves the application.
#[test]
fn cancel_stops_the_navigation_and_on_navigated_runs_before_the_page_leaves() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, server, _dir) = open_links(&chrome);

    let before = page_ids(&chrome);
    click(&mut page, "LNK-CANCEL", 0, "left");
    page.wait_for(&js(&format!("return el('{LBL_OUT}').textContent === 'cancelled';")), 10).expect("LNK-CANCEL's onClick ran");
    assert!(opened(&chrome, &before, "?cancel", 2).is_none(), "AC6: Cancel set in onClick: nothing opened");
    assert_eq!(page.eval("location.pathname").unwrap(), json!("/index.html"));

    // What LBL-OUT says when the page is hidden for the navigation is kept across it.
    page.eval(&js(&format!("window.addEventListener('pagehide', () => sessionStorage.setItem('atLeave', el('{LBL_OUT}').textContent)); return true;"))).unwrap();
    click(&mut page, "LNK-LEAVE", 0, "left");
    page.wait_for("location.search === '?left' && !!document.getElementById('landed')", 10).expect("LNK-LEAVE navigated this tab");
    let at_leave = page.eval("sessionStorage.getItem('atLeave')").unwrap();
    println!("LBL-OUT when the application left: {at_leave}");
    assert!(
        at_leave.as_str().is_some_and(|t| t.contains("landing.html?left") && !t.contains('\t')),
        "onNavigated (argument: the address) ran after onBeforeNavigate (argument: target⇥url) and before the page left: {at_leave}"
    );
    drop(page);
    server.stop();
    println!("web Hyperlink Cancel and onNavigated in Chrome ({}): Cancel in onClick opened nothing; a Self link's onNavigated wrote {at_leave} before the page left", exe.display());
}

/// AC8: Tab reaches the link, the focus ring shows, Enter follows it; every link in the accessibility tree has a name.
#[test]
fn the_keyboard_reaches_and_follows_a_link_and_every_link_has_an_accessible_name() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, server, _dir) = open_links(&chrome);
    let mut presses = 0;
    while page.eval(&js("return root.activeElement === a('LNK-TAB');")).unwrap() != json!(true) {
        presses += 1;
        assert!(presses <= 80, "Tab did not reach LNK-TAB in 80 presses");
        page.call("Input.dispatchKeyEvent", json!({"type": "keyDown", "key": "Tab", "code": "Tab", "windowsVirtualKeyCode": 9})).unwrap();
        page.call("Input.dispatchKeyEvent", json!({"type": "keyUp", "key": "Tab", "code": "Tab", "windowsVirtualKeyCode": 9})).unwrap();
    }
    let ring = page.eval(&js("const x = a('LNK-TAB'); const s = getComputedStyle(x); return { visible: x.matches(':focus-visible'), style: s.outlineStyle, width: s.outlineWidth };")).unwrap();
    assert_eq!(ring["visible"], json!(true), "{ring}");
    assert!(ring["style"] != json!("none") && ring["width"] != json!("0px"), "a visible focus ring: {ring}");
    let before = page_ids(&chrome);
    page.call("Input.dispatchKeyEvent", json!({"type": "keyDown", "key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13, "text": "\r"})).unwrap();
    page.call("Input.dispatchKeyEvent", json!({"type": "keyUp", "key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13})).unwrap();
    let p = opened(&chrome, &before, "?tab", 10).expect("Enter followed the focused link");
    has_opener_and_close(&chrome, &p);

    // The accessibility tree: each Hyperlink is a link with a name (the AccessibleLabel where set, else the Caption).
    page.call("Accessibility.enable", json!({})).unwrap();
    let tree = page.call("Accessibility.getFullAXTree", json!({})).unwrap();
    let links: Vec<(String, bool)> = tree["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["role"]["value"] == "link" && n["ignored"] != json!(true))
        .map(|n| (n["name"]["value"].as_str().unwrap_or("").to_owned(), n["properties"].as_array().is_some_and(|ps| ps.iter().any(|p| p["name"] == "focusable" && p["value"]["value"] == json!(true)))))
        .collect();
    println!("links in the accessibility tree: {links:?}");
    assert!(links.len() >= LINKS.len(), "every Hyperlink is a link: {links:?}");
    assert!(links.iter().all(|(n, _)| !n.trim().is_empty()), "no link without a name: {links:?}");
    assert!(links.iter().any(|(n, _)| n == "Example") && links.iter().any(|(n, _)| n == "Open the landing page here"), "Caption, and AccessibleLabel over it");

    // axe-core, as WASM-004 T34 runs it: not part of the repository (PRIME DIRECTIVE), named by `AXE_CORE`. A critical
    // violation on a Hyperlink fails the test; the fixture's other controls are reported, not judged here.
    let axe = match std::env::var_os("AXE_CORE").map(std::path::PathBuf::from) {
        None => "axe-core SKIPPED (set AXE_CORE to an axe.min.js; it is not part of the repository)".to_owned(),
        Some(path) => {
            let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            page.eval(&format!("(() => {{ {src}\n; return typeof axe; }})()")).unwrap();
            let version = page.eval("axe.version").unwrap();
            let vs = page
                .eval("axe.run(document, { resultTypes: ['violations'] }).then(r => r.violations.map(v => ({ id: v.id, impact: v.impact, targets: v.nodes.map(n => n.target.join(' ')) })))")
                .unwrap();
            let vs = vs.as_array().cloned().unwrap_or_default();
            for v in &vs {
                println!("  axe [{}] {}: {}", v["impact"].as_str().unwrap_or("?"), v["id"].as_str().unwrap_or("?"), v["targets"]);
            }
            let on_links: Vec<&Value> = vs.iter().filter(|v| v["impact"] == "critical" && v["targets"].as_array().is_some_and(|t| t.iter().any(|x| x.as_str().is_some_and(|s| s.contains("LNK-"))))).collect();
            assert!(on_links.is_empty(), "AC8: axe reports a critical issue on a Hyperlink: {on_links:?}");
            format!("axe-core {version}: {} violations, {} critical, none critical on a Hyperlink", vs.len(), vs.iter().filter(|v| v["impact"] == "critical").count())
        }
    };
    println!("{axe}");
    drop(page);
    server.stop();
    println!(
        "web Hyperlink keyboard in Chrome ({}): Tab reached LNK-TAB in {presses} presses, focus-visible ring {} {}, Enter opened its page; {} links in the accessibility tree, each named",
        exe.display(),
        ring["style"],
        ring["width"],
        links.len()
    );
}
