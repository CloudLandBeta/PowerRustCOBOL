// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **WASM-011 T25 — the standalone application, in a real browser** (AC3, AC4, AC5).
//!
//! In a standalone application every form is a page of its own: `OpenFormAsync` opens a **new tab**, and
//! `OpenFormSync` parks the caller until that tab is closed. The application is a project written here, in
//! code, built by the product's own Web Build (`style = "standalone"`), served; a headless Chrome clicks as a
//! user, sees the tabs the page opens (`/json/list`) and attaches to them. With no Chrome on the machine
//! (`CHROME` names one) the tests say they were skipped, and why.

mod support;

use std::time::{Duration, Instant};

use cobolt_forms::model::{Control, ControlType, EventBinding, PropValue};
use cobolt_web_bundle::devserver::{serve, ServeOptions, Server};
use cobolt_web_bundle::Isolation;
use serde_json::{json, Value};
use support::cdp::{find_chrome, Chrome, Page};
use support::project::{build, button, form, form_event, handler, handler_with, App, Spec};

/// The caption of the main form's label, and the forms on the page.
const LOOK: &str = r#"(() => {
  const root = document.getElementById('pr-app') && document.getElementById('pr-app').shadowRoot;
  if (!root) return null;
  const forms = [...root.querySelectorAll('[data-pr-form]')].map(e => e.dataset.prForm);
  const label = root.querySelector('[data-name="LBL-OUT"]');
  return { forms, caption: label ? label.textContent : null, text: root.textContent };
})()"#;

fn control_rect(name: &str) -> String {
    format!(
        "(() => {{ const r = document.getElementById('pr-app').shadowRoot; const e = r.querySelector('[data-name=\"{name}\"]'); if (!e) return null; const b = e.getBoundingClientRect(); return {{x: b.x, y: b.y, width: b.width, height: b.height}}; }})()"
    )
}

fn caption_is(text: &str) -> String {
    format!("(() => {{ const r = document.getElementById('pr-app') && document.getElementById('pr-app').shadowRoot; const l = r && r.querySelector('[data-name=\"LBL-OUT\"]'); return !!l && l.textContent === '{text}'; }})()")
}

fn serve_app(app: &App) -> Server {
    serve(
        &app.bundle,
        "127.0.0.1:0",
        ServeOptions {
            isolation: Isolation::RequireCorp,
            ..Default::default()
        },
    )
    .unwrap()
}

/// The label's caption is one of `texts`.
fn caption_in(texts: &[&str]) -> String {
    format!("(() => {{ const r = document.getElementById('pr-app') && document.getElementById('pr-app').shadowRoot; const l = r && r.querySelector('[data-name=\"LBL-OUT\"]'); return !!l && {texts:?}.includes(l.textContent); }})()")
}

/// The main page, open on the application, once the form has written its label.
fn open_main(chrome: &Chrome, server: &Server, first_caption: &str) -> Page {
    open_main_with(chrome, server, &[first_caption], None)
}

/// As [`open_main`], with a script that runs in the page before any of its own; the label may already have moved
/// on to any of `first_captions` by the first look.
fn open_main_with(
    chrome: &Chrome,
    server: &Server,
    first_captions: &[&str],
    before: Option<&str>,
) -> Page {
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1024, 768).unwrap();
    if let Some(source) = before {
        page.call(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({"source": source}),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    }
    page.navigate(&format!("{}/index.html", server.url()))
        .unwrap();
    page.wait_for(&caption_in(first_captions), 40)
        .unwrap_or_else(|e| panic!("the application did not start: {e}"));
    page
}

/// The pages `chrome` has open whose address names the form `form`.
fn tabs_of(chrome: &Chrome, form: &str) -> Vec<Value> {
    chrome
        .pages()
        .unwrap()
        .into_iter()
        .filter(|p| {
            p["url"]
                .as_str()
                .is_some_and(|u| u.contains(&format!("pr-form={form}")))
        })
        .collect()
}

fn wait_tabs(chrome: &Chrome, form: &str, count: usize, secs: u64) -> Result<Vec<Value>, String> {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        let tabs = tabs_of(chrome, form);
        if tabs.len() == count {
            return Ok(tabs);
        }
        if Instant::now() > deadline {
            return Err(format!(
                "waited {secs} s for {count} tab(s) of `{form}`; the browser has {:?}",
                chrome
                    .pages()
                    .unwrap()
                    .iter()
                    .map(|p| p["url"].as_str().unwrap_or("?").to_owned())
                    .collect::<Vec<_>>()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The main form opens `child-form` both ways and writes a caption after each verb; the child's button closes it.
fn two_ways_to_open() -> App {
    let mut main = form("main-form", "Main");
    form_event(
        &mut main,
        "onLoad",
        handler(&["MOVE \"ready\" TO LBL-OUT::Caption."]),
    );
    main.controls.push(button(
        "BTN-SYNC",
        "Sync",
        16,
        64,
        handler(&[
            "MOVE \"waiting\" TO LBL-OUT::Caption",
            "INVOKE ME::\"OpenFormSync\"(\"child-form\")",
            "MOVE \"after sync\" TO LBL-OUT::Caption.",
        ]),
    ));
    main.controls.push(button(
        "BTN-ASYNC",
        "Async",
        16,
        112,
        handler(&[
            "INVOKE ME::\"OpenFormAsync\"(\"child-form\")",
            "MOVE \"after async\" TO LBL-OUT::Caption.",
        ]),
    ));
    let mut child = form("child-form", "Child");
    form_event(
        &mut child,
        "onLoad",
        handler(&["MOVE \"child up\" TO LBL-OUT::Caption."]),
    );
    child.controls.push(button(
        "BTN-CLOSE",
        "Close",
        16,
        64,
        handler(&["INVOKE ME::Close()."]),
    ));
    build(Spec {
        style: "standalone",
        forms: vec![main, child],
        menus: vec![],
    })
}

/// AC3: `OpenFormAsync` from a click opens a new tab and the caller goes on; `OpenFormSync` suspends the caller
/// until that tab is closed, then resumes it.
#[test]
fn ac3_open_form_async_opens_a_new_tab_and_open_form_sync_waits_for_it_to_close() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let app = two_ways_to_open();
    let server = serve_app(&app);
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut main = open_main(&chrome, &server, "ready");
    let agent = main.eval("navigator.userAgent").unwrap();

    // The asynchronous open: a new tab appears and the caller has already written its caption.
    main.click_at(&control_rect("BTN-ASYNC"))
        .unwrap_or_else(|e| panic!("{e}"));
    let tabs = wait_tabs(&chrome, "child-form", 1, 30)
        .unwrap_or_else(|e| panic!("OpenFormAsync opened no tab: {e}"));
    main.wait_for(&caption_is("after async"), 20)
        .unwrap_or_else(|e| panic!("the caller did not run on: {e}; {:?}", main.eval(LOOK)));
    let mut child = chrome
        .attach(tabs[0]["webSocketDebuggerUrl"].as_str().unwrap())
        .unwrap_or_else(|e| panic!("{e}"));
    child
        .wait_for(&caption_is("child up"), 40)
        .unwrap_or_else(|e| panic!("the tab did not start: {e}; {:?}", child.eval(LOOK)));
    // Closing the child ends its tab.
    child
        .click_at(&control_rect("BTN-CLOSE"))
        .unwrap_or_else(|e| panic!("{e}"));
    wait_tabs(&chrome, "child-form", 0, 30)
        .unwrap_or_else(|e| panic!("Close did not close the tab: {e}"));
    drop(child);

    // The synchronous open: the tab appears, the caller stays at the verb for as long as it is open.
    main.click_at(&control_rect("BTN-SYNC"))
        .unwrap_or_else(|e| panic!("{e}"));
    let tabs = wait_tabs(&chrome, "child-form", 1, 30)
        .unwrap_or_else(|e| panic!("OpenFormSync opened no tab: {e}"));
    main.wait_for(&caption_is("waiting"), 20)
        .unwrap_or_else(|e| panic!("{e}; {:?}", main.eval(LOOK)));
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(
        main.eval(LOOK).unwrap()["caption"],
        json!("waiting"),
        "the caller is parked at OpenFormSync while the tab is open"
    );
    let mut child = chrome
        .attach(tabs[0]["webSocketDebuggerUrl"].as_str().unwrap())
        .unwrap_or_else(|e| panic!("{e}"));
    child
        .wait_for(&caption_is("child up"), 40)
        .unwrap_or_else(|e| panic!("the tab did not start: {e}; {:?}", child.eval(LOOK)));
    child
        .click_at(&control_rect("BTN-CLOSE"))
        .unwrap_or_else(|e| panic!("{e}"));
    wait_tabs(&chrome, "child-form", 0, 30)
        .unwrap_or_else(|e| panic!("Close did not close the tab: {e}"));
    main.wait_for(&caption_is("after sync"), 30)
        .unwrap_or_else(|e| {
            panic!(
                "the caller was not resumed when the tab closed: {e}; {:?}",
                main.eval(LOOK)
            )
        });
    drop(child);
    drop(main);
    server.stop();

    println!(
        "AC3 in Chrome ({}): OpenFormAsync -> a new tab (`pr-form=child-form`) and `after async` already written; its Close ended the tab; OpenFormSync -> a new tab, `waiting` held for 600 ms, and `after sync` written when the tab closed",
        agent.as_str().unwrap_or("?")
    );
}

/// A page that opens a form from a timer, with no user gesture behind it: the browser's popup blocker refuses.
fn open_from_a_timer() -> App {
    let mut main = form("main-form", "Main");
    form_event(
        &mut main,
        "onLoad",
        handler(&["MOVE \"ready\" TO LBL-OUT::Caption."]),
    );
    form_event(
        &mut main,
        "onPopupBlocked",
        handler(&["MOVE \"blocked\" TO LBL-NOTE::Caption."]),
    );
    main.controls
        .push(Control::new("LBL-NOTE", ControlType::Label, 272, 56));
    let mut timer = Control::new("TMR-1", ControlType::Timer, 16, 160);
    timer.set_prop("Interval", PropValue::Int(400));
    timer.set_prop("Enabled", PropValue::Bool(true));
    let mut tick = EventBinding::for_control("TMR-1", "onTick");
    tick.code = handler(&[
        "INVOKE ME::\"OpenFormSync\"(\"child-form\")",
        "MOVE \"released\" TO LBL-OUT::Caption.",
    ]);
    timer.events.push(tick);
    main.controls.push(timer);
    let child = form("child-form", "Child");
    build(Spec {
        style: "standalone",
        forms: vec![main, child],
        menus: vec![],
    })
}

/// AC4: a standalone open from a timer, with the popup blocked, releases the caller with a failure and shows the hint.
/// The blocker is stood in for: see the comment where the page is opened.
#[test]
fn ac4_a_blocked_popup_releases_the_caller_shows_the_hint_and_raises_on_popup_blocked() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let app = open_from_a_timer();
    let server = serve_app(&app);
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    // Headless Chrome has **no popup blocker**: `window.open` opens a tab whether or not a user gesture is behind it
    // and with or without `--block-new-web-contents` (probed in Chrome 157). What a blocker does, as far as a page can
    // tell, is make `window.open` answer `null` (HTML, "the rules for choosing a navigable"); the page is given that
    // answer before any of its own scripts run. The blocker's own notice in the address bar is not seen here.
    // The timer's first tick comes 400 ms after the form has loaded and moves the label on from `ready` to
    // `released`: a machine busy enough to look later than that (run 52, Windows) finds the second.
    let mut main = open_main_with(
        &chrome,
        &server,
        &["ready", "released"],
        Some("window.open = function () { return null; };"),
    );
    let agent = main.eval("navigator.userAgent").unwrap();

    // The timer ticks, OpenFormSync is refused by the browser, and the caller is released: it goes on to the next statement.
    main.wait_for(&caption_is("released"), 30)
        .unwrap_or_else(|e| panic!("the caller was never released: {e}; {:?}", main.eval(LOOK)));
    let seen = main.eval(LOOK).unwrap();
    assert!(
        seen["text"]
            .as_str()
            .unwrap()
            .contains("The browser blocked the new window"),
        "the hint is shown: {seen}"
    );
    main.wait_for("(() => { const r = document.getElementById('pr-app').shadowRoot; const l = r.querySelector('[data-name=\"LBL-NOTE\"]'); return !!l && l.textContent === 'blocked'; })()", 20)
        .unwrap_or_else(|e| panic!("onPopupBlocked was not raised on the opener's form: {e}; {seen}"));
    assert!(
        tabs_of(&chrome, "child-form").is_empty(),
        "no tab of the child exists: {:?}",
        chrome
            .pages()
            .unwrap()
            .iter()
            .map(|p| p["url"].clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        chrome.pages().unwrap().len(),
        2,
        "the browser's own blank tab and the application's page, no more"
    );
    drop(main);
    server.stop();

    println!(
        "AC4 in Chrome ({}, `window.open` answering null, as a popup blocker makes it): the timer's OpenFormSync was refused, the caller was released (`released`), the hint \"The browser blocked the new window\u{2026}\" is on the page, onPopupBlocked wrote `blocked`, and no tab of the child exists",
        agent.as_str().unwrap_or("?")
    );
}

/// An application whose main form opens a child in a tab of its own; the child reaches back with `super::`.
fn across_windows() -> App {
    let mut main = form("main-form", "Opener Title");
    form_event(
        &mut main,
        "onLoad",
        handler(&["MOVE \"ready\" TO LBL-OUT::Caption."]),
    );
    main.controls.push(button(
        "BTN-OPEN",
        "Open",
        16,
        64,
        handler(&["INVOKE ME::\"OpenFormAsync\"(\"child-form\")."]),
    ));
    main.controls.push(button(
        "BTN-RETITLE",
        "Retitle",
        16,
        112,
        handler(&["MOVE \"Retitled by the opener\" TO ME::Title."]),
    ));
    main.controls.push(button(
        "BTN-SHOW",
        "Show",
        16,
        160,
        handler(&["MOVE ME::Title TO LBL-OUT::Caption."]),
    ));
    let mut child = form("child-form", "Child");
    form_event(
        &mut child,
        "onLoad",
        handler(&["MOVE \"child up\" TO LBL-OUT::Caption."]),
    );
    child.controls.push(button(
        "BTN-READ",
        "Read",
        16,
        64,
        handler(&["MOVE super::Title TO LBL-OUT::Caption."]),
    ));
    child.controls.push(button(
        "BTN-CALL",
        "Call",
        16,
        112,
        handler_with(
            &["01 WS-S PIC X(40) VALUE SPACES."],
            &[
                "INVOKE super::GetFormState() RETURNING WS-S",
                "MOVE WS-S TO LBL-OUT::Caption.",
            ],
        ),
    ));
    child.controls.push(button(
        "BTN-WRITE",
        "Write",
        16,
        160,
        handler(&["MOVE \"Written by the child\" TO super::Title."]),
    ));
    // The same call as `BTN-CALL`, written for an opener that may be gone: its failure is the program's to handle.
    child.controls.push(button(
        "BTN-GUARD",
        "Guarded",
        16,
        208,
        handler_with(
            &["01 WS-S PIC X(40) VALUE SPACES."],
            &[
                "INVOKE super::GetFormState() RETURNING WS-S",
                "ON EXCEPTION MOVE \"gone\" TO LBL-OUT::Caption",
                "NOT ON EXCEPTION MOVE WS-S TO LBL-OUT::Caption",
                "END-INVOKE.",
            ],
        ),
    ));
    build(Spec {
        style: "standalone",
        forms: vec![main, child],
        menus: vec![],
    })
}

/// The main page and its child's tab, the child written to once (`child up`).
fn opener_and_child(chrome: &Chrome, server: &Server) -> (Page, Page) {
    let mut main = open_main(chrome, server, "ready");
    main.click_at(&control_rect("BTN-OPEN")).unwrap();
    let tabs = wait_tabs(chrome, "child-form", 1, 30).unwrap();
    let mut child = chrome
        .attach(tabs[0]["webSocketDebuggerUrl"].as_str().unwrap())
        .unwrap();
    child
        .wait_for(&caption_is("child up"), 40)
        .unwrap_or_else(|e| panic!("the child form did not start: {e}"));
    (main, child)
}

/// **AC5, the first half** — `super::` crosses from a standalone child to the form that opened its tab: a property
/// read, a method call and a write, each waiting for the opener's own answer; what the opener holds now is what the
/// child reads (not a copy that can be a message behind).
#[test]
fn ac5_super_reads_calls_and_writes_cross_from_the_child_tab_to_the_opener() {
    let Some(exe) = find_chrome() else {
        eprintln!("SKIPPED: no Chrome on this machine (set CHROME to a Chrome or Chromium binary)");
        return;
    };
    let app = across_windows();
    let server = serve_app(&app);
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut main, mut child) = opener_and_child(&chrome, &server);
    let agent = main.eval("navigator.userAgent").unwrap();

    // A read: the opener's title, as it was designed.
    child.click_at(&control_rect("BTN-READ")).unwrap();
    child
        .wait_for(&caption_is("Opener Title"), 40)
        .unwrap_or_else(|e| {
            panic!(
                "the child never read the opener's Title: {e}; it shows {:?}",
                child.eval(LOOK)
            )
        });

    // The opener changes it; the child reads it again and sees the new value.
    main.click_at(&control_rect("BTN-RETITLE")).unwrap();
    child.click_at(&control_rect("BTN-READ")).unwrap();
    child
        .wait_for(&caption_is("Retitled by the opener"), 40)
        .unwrap_or_else(|e| {
            panic!(
                "the child did not see the opener's change: {e}; it shows {:?}",
                child.eval(LOOK)
            )
        });

    // A method call that answers: the opener runs it and the child gets what it returned.
    child.click_at(&control_rect("BTN-CALL")).unwrap();
    child
        .wait_for(&caption_is("Normal"), 40)
        .unwrap_or_else(|e| {
            panic!(
                "the child never got the opener's GetFormState(): {e}; it shows {:?}",
                child.eval(LOOK)
            )
        });

    // A write: the child sets the opener's Title; the opener, asked, shows it.
    child.click_at(&control_rect("BTN-WRITE")).unwrap();
    let mut seen = String::new();
    for _ in 0..40 {
        main.click_at(&control_rect("BTN-SHOW")).unwrap();
        std::thread::sleep(Duration::from_millis(150));
        seen = main
            .eval(LOOK)
            .ok()
            .and_then(|v| v["caption"].as_str().map(str::to_owned))
            .unwrap_or_default();
        if seen == "Written by the child" {
            break;
        }
    }
    assert_eq!(
        seen, "Written by the child",
        "the opener did not get the child's write"
    );
    assert!(
        child.exceptions.is_empty(),
        "the child page threw: {:?}",
        child.exceptions
    );
    assert!(
        main.exceptions.is_empty(),
        "the opener page threw: {:?}",
        main.exceptions
    );

    println!(
        "AC5 in Chrome ({}): from the child's tab, `MOVE super::Title` waited for the opener and read `Opener Title`, then `Retitled by the opener` after the opener changed it; `INVOKE super::GetFormState() RETURNING` answered `Normal`; `MOVE … TO super::Title` reached the opener (`Written by the child`); no page threw",
        agent.as_str().unwrap_or("?")
    );
}

/// What the child's page shows of a run-time error: the banner the player raises, and what it wrote to the console.
const ERRORS_SHOWN: &str = r#"(() => {
  const root = document.getElementById('pr-app') && document.getElementById('pr-app').shadowRoot;
  return root ? root.querySelectorAll('.pr-error-banner').length : -1;
})()"#;

/// **AC5, the second half** — the opener goes away. A `super::` call that the program guards with `ON EXCEPTION`
/// is the program's to handle: with the opener's tab open it answers (`NOT ON EXCEPTION` runs with the value); with
/// the tab closed it fails quietly and `ON EXCEPTION` runs, and the page shows no run-time error, throws nothing and
/// writes nothing to its console. The handler goes on after `END-INVOKE` and the page keeps working.
#[test]
fn ac5_a_guarded_super_call_meets_a_closed_opener_with_on_exception_and_no_error() {
    let Some(exe) = find_chrome() else {
        eprintln!("SKIPPED: no Chrome on this machine (set CHROME to a Chrome or Chromium binary)");
        return;
    };
    let app = across_windows();
    let server = serve_app(&app);
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (main, mut child) = opener_and_child(&chrome, &server);
    let agent = child.eval("navigator.userAgent").unwrap();

    // The opener is open: the call is answered and the clause for an answer runs.
    child.click_at(&control_rect("BTN-GUARD")).unwrap();
    child
        .wait_for(&caption_is("Normal"), 40)
        .unwrap_or_else(|e| {
            panic!(
                "the guarded call did not go through while the opener was open: {e}; it shows {:?}",
                child.eval(LOOK)
            )
        });

    // The opener's tab is closed (a user closing it: the page says goodbye on the way out).
    let started = Instant::now();
    drop(main);
    // Give the goodbye the time it needs to cross the channel, then ask again: the child must now be told it is gone.
    let mut caption = String::new();
    for _ in 0..80 {
        child.click_at(&control_rect("BTN-GUARD")).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        caption = child
            .eval(LOOK)
            .ok()
            .and_then(|v| v["caption"].as_str().map(str::to_owned))
            .unwrap_or_default();
        if caption == "gone" {
            break;
        }
    }
    let took = started.elapsed();
    assert_eq!(
        caption, "gone",
        "ON EXCEPTION did not run once the opener was closed; the child shows {:?}",
        child.eval(LOOK)
    );
    let banners = child.eval(ERRORS_SHOWN).unwrap();
    assert_eq!(
        banners,
        json!(0),
        "the page shows a run-time error for a failure the program handled"
    );
    assert!(
        child.exceptions.is_empty(),
        "the child page threw: {:?}",
        child.exceptions
    );
    let errors: Vec<String> = child
        .console
        .iter()
        .filter(|l| l.starts_with("error"))
        .cloned()
        .collect();
    assert!(
        errors.is_empty(),
        "the child wrote to its console as an error: {errors:?}"
    );

    // The page is alive: a button that needs no opener still answers.
    child
        .eval("1")
        .unwrap_or_else(|e| panic!("the child page stopped answering: {e}"));

    println!(
        "AC5 second half in Chrome ({}): with the opener open `INVOKE super::GetFormState() RETURNING … ON EXCEPTION … NOT ON EXCEPTION … END-INVOKE` ran NOT ON EXCEPTION with `Normal`; after the opener's tab was closed it ran ON EXCEPTION (`gone`) within {:?}; run-time error banners 0, uncaught exceptions {}, console errors {}",
        agent.as_str().unwrap_or("?"),
        took,
        child.exceptions.len(),
        errors.len()
    );
}
