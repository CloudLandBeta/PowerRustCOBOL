// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **WASM-011 T25 — the single-page application, in a real browser** (AC1, AC2, AC8).
//!
//! The application is a project written here, in code, and built by the product's own Web Build (the
//! generator): forms with handlers in COBOL, the SideMenu's items, `style = "spa"`. The bundle is served
//! and a headless Chrome acts as a user — clicks the menu, goes Back, reloads, presses a button — and asks
//! the page what it shows. With no Chrome on the machine (`CHROME` names one) the tests say they were
//! skipped, and why.

mod support;

use std::time::Duration;

use cobolt_forms::model::FormFormat;
use cobolt_web_bundle::devserver::{serve, ServeOptions, Server};
use cobolt_web_bundle::Isolation;
use serde_json::json;
use support::cdp::{find_chrome, Chrome, Page};
use support::project::{build, button, form, form_event, handler, menu_def, side_menu, App, Spec};

/// What the page shows: the forms on it, the address, the label of each, and the menu's current entry.
const LOOK: &str = r#"(() => {
  const root = document.getElementById('pr-app').shadowRoot;
  const forms = [...root.querySelectorAll('[data-pr-form]')].map(e => e.dataset.prForm);
  const caption = {};
  for (const f of forms) {
    const l = root.querySelector(`[data-pr-form="${f}"] [data-name="LBL-OUT"]`);
    caption[f] = l ? l.textContent : null;
  }
  const cur = root.querySelector('.pr-sm-items [aria-current="page"] .pr-menu-label');
  return { forms, hash: location.hash, caption, current: cur ? cur.textContent : null };
})()"#;

/// Until the player has mounted (right after a reload) the page has no shadow root yet: that is "not yet", and
/// `wait_for` keeps polling; a script that threw on it would end the wait on the first look.
fn has_form(name: &str, present: bool) -> String {
    format!("(() => {{ const r = document.getElementById('pr-app')?.shadowRoot; return !!r && (!!r.querySelector('[data-pr-form=\"{name}\"]') === {present}); }})()")
}

/// The rectangle of the menu entry labelled `label`.
fn entry(label: &str) -> String {
    format!(
        "(() => {{ const r = document.getElementById('pr-app').shadowRoot; const e = [...r.querySelectorAll('.pr-sm-items .pr-menu-label')].find(x => x.textContent === '{label}'); if (!e) return null; const b = e.getBoundingClientRect(); return {{x: b.x, y: b.y, width: b.width, height: b.height}}; }})()"
    )
}

/// Serve `app` and open it in a page of `chrome`, once the main form has written its label.
fn open(chrome: &Chrome, app: &App, address: &str) -> (Page, Server) {
    let server = serve(
        &app.bundle,
        "127.0.0.1:0",
        ServeOptions {
            isolation: Isolation::RequireCorp,
            ..Default::default()
        },
    )
    .unwrap();
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1024, 768).unwrap();
    page.navigate(&format!("{}/index.html{address}", server.url()))
        .unwrap();
    page.wait_for(&format!("(() => {{ const r = document.getElementById('pr-app').shadowRoot; return !!(r && r.querySelector('[data-pr-form=\"main-form\"] [data-name=\"LBL-OUT\"]')); }})()"), 40)
        .unwrap_or_else(|e| panic!("the application did not start: {e}"));
    (page, server)
}

/// Three screens, each saying that it loaded, behind a SideMenu; the main form is the Home page.
fn three_screens() -> App {
    let mut main = form("main-form", "Home");
    main.controls.push(side_menu(480));
    form_event(
        &mut main,
        "onLoad",
        handler(&["MOVE \"home\" TO LBL-OUT::Caption."]),
    );
    let mut forms = vec![main];
    for (stem, shown) in [
        ("orders-form", "orders loaded"),
        ("customers-form", "customers loaded"),
        ("reports-form", "reports loaded"),
    ] {
        let mut f = form(stem, stem);
        f.form_format = FormFormat::Embedded;
        form_event(
            &mut f,
            "onLoad",
            handler(&[&format!("MOVE \"{shown}\" TO LBL-OUT::Caption.")]),
        );
        forms.push(f);
    }
    build(Spec {
        style: "spa",
        forms,
        menus: vec![(
            "SideMenu-1",
            menu_def(&[
                ("ord", "Orders", "orders-form"),
                ("cus", "Customers", "customers-form"),
                ("rep", "Reports", "reports-form"),
            ]),
        )],
    })
}

#[test]
fn ac1_a_menu_entry_loads_its_form_and_changes_the_address_back_returns_and_a_reload_reopens_it() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let app = three_screens();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, server) = open(&chrome, &app, "");

    let home = page.eval(LOOK).unwrap();
    assert_eq!(
        home["hash"],
        json!(""),
        "the Home page has no address: {home}"
    );
    assert_eq!(home["caption"]["main-form"], json!("home"), "{home}");

    // Choosing an entry loads its form and puts it in the address.
    page.click_at(&entry("Orders"))
        .unwrap_or_else(|e| panic!("{e}"));
    page.wait_for(&has_form("orders-form", true), 20)
        .unwrap_or_else(|e| {
            panic!(
                "the entry did not load its form: {e}; {}",
                page.eval(LOOK).unwrap()
            )
        });
    page.wait_for("document.getElementById('pr-app').shadowRoot.querySelector('[data-pr-form=\"orders-form\"] [data-name=\"LBL-OUT\"]')?.textContent === 'orders loaded'", 20)
        .unwrap_or_else(|e| panic!("the form's own onLoad did not run: {e}; {}", page.eval(LOOK).unwrap()));
    let orders = page.eval(LOOK).unwrap();
    assert_eq!(orders["hash"], json!("#/orders-form"), "{orders}");
    assert_eq!(
        orders["current"],
        json!("Orders"),
        "the entry is the current page: {orders}"
    );

    // A second entry replaces it; the first is torn down.
    page.click_at(&entry("Customers"))
        .unwrap_or_else(|e| panic!("{e}"));
    page.wait_for(&has_form("customers-form", true), 20)
        .unwrap_or_else(|e| panic!("{e}; {}", page.eval(LOOK).unwrap()));
    page.wait_for(&has_form("orders-form", false), 20)
        .unwrap_or_else(|e| {
            panic!(
                "the outgoing screen stayed: {e}; {}",
                page.eval(LOOK).unwrap()
            )
        });
    let customers = page.eval(LOOK).unwrap();
    assert_eq!(customers["hash"], json!("#/customers-form"), "{customers}");
    assert_eq!(customers["current"], json!("Customers"), "{customers}");

    // Back returns to the previous form, and the address with it.
    page.eval("history.back()").unwrap();
    page.wait_for(&has_form("orders-form", true), 20)
        .unwrap_or_else(|e| {
            panic!(
                "Back did not return to the previous form: {e}; {}",
                page.eval(LOOK).unwrap()
            )
        });
    page.wait_for(&has_form("customers-form", false), 20)
        .unwrap_or_else(|e| panic!("{e}; {}", page.eval(LOOK).unwrap()));
    let back = page.eval(LOOK).unwrap();
    assert_eq!(back["hash"], json!("#/orders-form"), "{back}");

    // A reload at a form's address reopens that form (the main form is still the Home page under it).
    page.eval("location.hash = '#/reports-form'").unwrap();
    page.wait_for(&has_form("reports-form", true), 20)
        .unwrap_or_else(|e| panic!("{e}; {}", page.eval(LOOK).unwrap()));
    page.eval("location.reload()").unwrap();
    page.wait_for(&has_form("reports-form", true), 40)
        .unwrap_or_else(|e| {
            panic!(
                "the reload did not reopen the form at the address: {e}; {}",
                page.eval(LOOK).unwrap()
            )
        });
    let reloaded = page.eval(LOOK).unwrap();
    assert_eq!(reloaded["hash"], json!("#/reports-form"), "{reloaded}");
    assert_eq!(
        reloaded["caption"]["reports-form"],
        json!("reports loaded"),
        "its onLoad ran again: {reloaded}"
    );
    drop(page);
    server.stop();

    println!(
        "AC1 in Chrome ({}): three forms behind a SideMenu, built by the Web Build; Orders -> `#/orders-form`, Customers -> `#/customers-form` (Orders torn down), Back -> `#/orders-form`, reload at `#/reports-form` reopened it with its onLoad",
        exe.display()
    );
}

/// AC7 in the application shell: every navigation — a menu entry, Back — moves the focus onto the form that
/// arrived (its root, which its title labels) and announces its title in the page's polite live region.
#[test]
fn ac7_each_navigation_in_the_shell_announces_the_form_and_puts_the_focus_on_it() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let app = three_screens();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, server) = open(&chrome, &app, "");
    let agent = page.eval("navigator.userAgent").unwrap();

    let mut seen = Vec::new();
    for (via, form) in [
        ("the Orders entry", "orders-form"),
        ("the Customers entry", "customers-form"),
    ] {
        let label = if form == "orders-form" {
            "Orders"
        } else {
            "Customers"
        };
        page.click_at(&entry(label))
            .unwrap_or_else(|e| panic!("{e}"));
        page.wait_for(&has_form(form, true), 20)
            .unwrap_or_else(|e| panic!("{e}; {}", page.eval(LOOK).unwrap()));
        page.wait_for(&format!("(() => {{ const l = document.getElementById('pr-app').shadowRoot.querySelector('.pr-live'); return !!l && l.textContent.trim() === '{form}'; }})()"), 20)
            .unwrap_or_else(|e| panic!("{via}: the form was not announced: {e}; {}", page.eval(A11Y).unwrap()));
        let a = page.eval(A11Y).unwrap();
        assert_eq!(a["liveRole"], json!(["status", "polite"]), "{via}: {a}");
        assert_eq!(
            (&a["focus"], &a["focusClass"]),
            (&json!(form), &json!("pr-form-root")),
            "{via}: the focus is on the form that arrived: {a}"
        );
        seen.push(format!("{via} -> `{form}`"));
    }
    // Back returns to the first form: it is announced again and takes the focus back.
    page.eval("history.back()").unwrap();
    page.wait_for(&has_form("orders-form", true), 20)
        .unwrap_or_else(|e| panic!("{e}; {}", page.eval(LOOK).unwrap()));
    page.wait_for("(() => { const r = document.getElementById('pr-app').shadowRoot; const a = r.activeElement; const o = a && a.closest('[data-pr-form]'); return !!o && o.dataset.prForm === 'orders-form'; })()", 20)
        .unwrap_or_else(|e| panic!("Back: the focus did not go to the form that came back: {e}; {}", page.eval(A11Y).unwrap()));
    let back = page.eval(A11Y).unwrap();
    assert_eq!(
        back["live"],
        json!("orders-form"),
        "Back announces the form it returns to: {back}"
    );
    drop(page);
    server.stop();

    println!("AC7 in Chrome ({}): {}, Back -> `orders-form`: each announced in the polite live region (role=status) and the focus on the form's root", agent.as_str().unwrap_or("?"), seen.join(", "));
}

const A11Y: &str = r#"(() => {
  const root = document.getElementById('pr-app').shadowRoot;
  const live = root.querySelector('.pr-live');
  const a = root.activeElement;
  const owner = a && a.closest('[data-pr-form]');
  return {
    live: live ? live.textContent.replace(/ /g, ' ').trim() : null,
    liveRole: live ? [live.getAttribute('role'), live.getAttribute('aria-live')] : null,
    focus: owner ? owner.dataset.prForm : null,
    focusClass: a ? a.className : null,
    focusTag: a ? a.tagName : null,
  };
})()"#;

/// The rectangle of control `name` of form `form`.
fn control_rect(form: &str, name: &str) -> String {
    format!(
        "(() => {{ const r = document.getElementById('pr-app').shadowRoot; const e = r.querySelector('[data-pr-form=\"{form}\"] [data-name=\"{name}\"]'); if (!e) return null; const b = e.getBoundingClientRect(); return {{x: b.x, y: b.y, width: b.width, height: b.height}}; }})()"
    )
}

/// How the dialog form is shown: in a modal `<dialog>` or in a modeless panel, and whether it is open.
const FRAME: &str = r#"(() => {
  const root = document.getElementById('pr-app').shadowRoot;
  const form = root.querySelector('[data-pr-form="dialog-form"]');
  const dlg = form && form.closest('dialog');
  const panel = form && form.closest('.pr-panelwin');
  return {
    present: !!form,
    modal: !!dlg && dlg.matches(':modal'),
    dialog: !!dlg,
    panel: !!panel,
    caption: (root.querySelector('[data-pr-form="main-form"] [data-name="LBL-OUT"]') || {}).textContent,
  };
})()"#;

/// A single-page application whose main form opens `dialog-form` both ways and writes a caption after each
/// verb; the dialog's button closes it.
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
            "INVOKE ME::\"OpenFormSync\"(\"dialog-form\")",
            "MOVE \"after sync\" TO LBL-OUT::Caption.",
        ]),
    ));
    main.controls.push(button(
        "BTN-ASYNC",
        "Async",
        16,
        112,
        handler(&[
            "INVOKE ME::\"OpenFormAsync\"(\"dialog-form\")",
            "MOVE \"after async\" TO LBL-OUT::Caption.",
        ]),
    ));
    let mut dialog = form("dialog-form", "Dialog");
    dialog.controls.push(button(
        "BTN-CLOSE",
        "Close",
        16,
        64,
        handler(&["INVOKE ME::Close()."]),
    ));
    build(Spec {
        style: "spa",
        forms: vec![main, dialog],
        menus: vec![],
    })
}

/// AC2: `OpenFormSync` in a single-page application shows a modal dialog and the statement after the verb
/// does not run until it closes; `OpenFormAsync` shows a modeless panel and the caller runs on.
#[test]
fn ac2_open_form_sync_is_a_modal_dialog_the_caller_waits_for_and_open_form_async_is_a_panel_it_runs_on_past(
) {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let app = two_ways_to_open();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, server) = open(&chrome, &app, "");
    let agent = page.eval("navigator.userAgent").unwrap();
    assert_eq!(page.eval(FRAME).unwrap()["caption"], json!("ready"));

    // The synchronous open: a modal dialog; the caller is parked at the verb, and stays there.
    page.click_at(&control_rect("main-form", "BTN-SYNC"))
        .unwrap_or_else(|e| panic!("{e}"));
    page.wait_for(&has_form("dialog-form", true), 20)
        .unwrap_or_else(|e| {
            panic!(
                "the dialog did not open: {e}; {}",
                page.eval(FRAME).unwrap()
            )
        });
    let waiting = page.eval(FRAME).unwrap();
    assert_eq!(
        (&waiting["dialog"], &waiting["modal"]),
        (&json!(true), &json!(true)),
        "OpenFormSync shows a modal dialog: {waiting}"
    );
    assert_eq!(
        waiting["caption"],
        json!("waiting"),
        "the statement before the verb ran, the one after has not: {waiting}"
    );
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert_eq!(
        page.eval(FRAME).unwrap()["caption"],
        json!("waiting"),
        "and it does not run while the dialog is open"
    );

    // Closing the dialog resumes the caller at the next statement.
    page.click_at(&control_rect("dialog-form", "BTN-CLOSE"))
        .unwrap_or_else(|e| panic!("{e}"));
    page.wait_for(&has_form("dialog-form", false), 20)
        .unwrap_or_else(|e| {
            panic!(
                "the dialog did not close: {e}; {}",
                page.eval(FRAME).unwrap()
            )
        });
    page.wait_for("document.getElementById('pr-app').shadowRoot.querySelector('[data-pr-form=\"main-form\"] [data-name=\"LBL-OUT\"]').textContent === 'after sync'", 20)
        .unwrap_or_else(|e| panic!("the caller was not resumed: {e}; {}", page.eval(FRAME).unwrap()));

    // The asynchronous open: a modeless panel, and the caller has already gone on.
    page.click_at(&control_rect("main-form", "BTN-ASYNC"))
        .unwrap_or_else(|e| panic!("{e}"));
    page.wait_for(&has_form("dialog-form", true), 20)
        .unwrap_or_else(|e| panic!("the panel did not open: {e}; {}", page.eval(FRAME).unwrap()));
    page.wait_for("document.getElementById('pr-app').shadowRoot.querySelector('[data-pr-form=\"main-form\"] [data-name=\"LBL-OUT\"]').textContent === 'after async'", 20)
        .unwrap_or_else(|e| panic!("the caller did not run on: {e}; {}", page.eval(FRAME).unwrap()));
    let asynchronous = page.eval(FRAME).unwrap();
    assert_eq!(
        (
            &asynchronous["panel"],
            &asynchronous["modal"],
            &asynchronous["present"]
        ),
        (&json!(true), &json!(false), &json!(true)),
        "OpenFormAsync shows a modeless panel, still open: {asynchronous}"
    );
    drop(page);
    server.stop();

    println!("AC2 in Chrome ({}): OpenFormSync -> a modal <dialog> (`:modal`), the caption after the verb unchanged for 500 ms, then `after sync` once the dialog's Close ran; OpenFormAsync -> a modeless panel with `after async` already written", agent.as_str().unwrap_or("?"));
}

/// The page's own question, "This form has unsaved changes. Leave it anyway?" — a `<dialog>` — and its buttons.
const ASKED: &str = "(() => { const r = document.getElementById('pr-app').shadowRoot; return [...r.querySelectorAll('dialog')].some(d => d.open && d.textContent.includes('unsaved changes')); })()";

fn answer_button(label: &str) -> String {
    format!(
        "(() => {{ const r = document.getElementById('pr-app').shadowRoot; const d = [...r.querySelectorAll('dialog')].find(d => d.open && d.textContent.includes('unsaved changes')); if (!d) return null; const e = [...d.querySelectorAll('button')].find(b => b.textContent.trim() === '{label}'); if (!e) return null; const b = e.getBoundingClientRect(); return {{x: b.x, y: b.y, width: b.width, height: b.height}}; }})()"
    )
}

/// A shell with a screen that marks itself as holding unsaved work and another that does not.
fn unsaved_screens() -> App {
    let mut main = form("main-form", "Home");
    main.controls.push(side_menu(480));
    form_event(
        &mut main,
        "onLoad",
        handler(&["MOVE \"home\" TO LBL-OUT::Caption."]),
    );
    let mut edit = form("edit-form", "Edit");
    edit.form_format = FormFormat::Embedded;
    form_event(
        &mut edit,
        "onLoad",
        handler(&[
            "MOVE \"editing\" TO LBL-OUT::Caption",
            "INVOKE ME::\"SetUnsavedChanges\"(1).",
        ]),
    );
    let mut other = form("other-form", "Other");
    other.form_format = FormFormat::Embedded;
    form_event(
        &mut other,
        "onLoad",
        handler(&["MOVE \"other\" TO LBL-OUT::Caption."]),
    );
    build(Spec {
        style: "spa",
        forms: vec![main, edit, other],
        menus: vec![(
            "SideMenu-1",
            menu_def(&[("edt", "Edit", "edit-form"), ("oth", "Other", "other-form")]),
        )],
    })
}

/// AC8: a form that says it holds unsaved changes is not left without asking — Back and a menu entry get the
/// player's Leave / Stay question, closing the tab gets the browser's own (`beforeunload`) — and once the user
/// has left it, nothing asks any more.
#[test]
fn ac8_a_form_with_unsaved_changes_asks_before_back_before_a_menu_load_and_before_the_tab_closes() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let app = unsaved_screens();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, server) = open(&chrome, &app, "");
    let agent = page.eval("navigator.userAgent").unwrap();
    page.dialog_accept = Some(false);

    page.click_at(&entry("Edit"))
        .unwrap_or_else(|e| panic!("{e}"));
    page.wait_for("document.getElementById('pr-app').shadowRoot.querySelector('[data-pr-form=\"edit-form\"] [data-name=\"LBL-OUT\"]')?.textContent === 'editing'", 20)
        .unwrap_or_else(|e| panic!("the screen did not load: {e}; {}", page.eval(LOOK).unwrap()));
    assert_eq!(
        page.eval(ASKED).unwrap(),
        json!(false),
        "nothing is asked just for opening the screen"
    );

    // Back: the question; Stay keeps the form and its address.
    page.eval("history.back()").unwrap();
    page.wait_for(ASKED, 20)
        .unwrap_or_else(|e| panic!("Back did not ask: {e}; {}", page.eval(LOOK).unwrap()));
    assert_eq!(
        page.eval(LOOK).unwrap()["forms"],
        json!(["main-form", "edit-form"]),
        "the form stays under the question"
    );
    page.click_at(&answer_button("Stay"))
        .unwrap_or_else(|e| panic!("{e}"));
    page.wait_for(&format!("!({ASKED})"), 20)
        .unwrap_or_else(|e| panic!("{e}"));
    let stayed = page.eval(LOOK).unwrap();
    assert_eq!(
        (&stayed["forms"], &stayed["hash"]),
        (&json!(["main-form", "edit-form"]), &json!("#/edit-form")),
        "Stay: the form and its address are where they were: {stayed}"
    );

    // A menu entry that would leave it asks the same question.
    page.click_at(&entry("Other"))
        .unwrap_or_else(|e| panic!("{e}"));
    page.wait_for(ASKED, 20).unwrap_or_else(|e| {
        panic!(
            "the menu load did not ask: {e}; {}",
            page.eval(LOOK).unwrap()
        )
    });
    page.click_at(&answer_button("Stay"))
        .unwrap_or_else(|e| panic!("{e}"));
    page.wait_for(&format!("!({ASKED})"), 20)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        page.eval(LOOK).unwrap()["forms"],
        json!(["main-form", "edit-form"]),
        "Stay on a menu load: still the form with unsaved changes"
    );

    // Closing the tab: the browser's own question, from a navigation away (the user activation is the clicks above).
    assert!(
        page.dialogs.is_empty(),
        "no browser dialog yet: {:?}",
        page.dialogs
    );
    let _ = page.call("Page.navigate", json!({"url": "about:blank"}));
    assert!(
        page.dialogs.iter().any(|d| d.starts_with("beforeunload")),
        "leaving the page with unsaved changes opens the browser's beforeunload question: {:?}",
        page.dialogs
    );
    assert!(
        page.eval("location.href")
            .unwrap()
            .as_str()
            .unwrap_or("")
            .contains("/index.html"),
        "dismissing it kept the page"
    );

    // Back, then Leave: the form goes, the address goes, and the mark with it.
    page.eval("history.back()").unwrap();
    page.wait_for(ASKED, 20).unwrap_or_else(|e| panic!("{e}"));
    page.click_at(&answer_button("Leave"))
        .unwrap_or_else(|e| panic!("{e}"));
    page.wait_for(&has_form("edit-form", false), 20)
        .unwrap_or_else(|e| panic!("Leave did not leave: {e}; {}", page.eval(LOOK).unwrap()));
    assert_eq!(page.eval(LOOK).unwrap()["hash"], json!(""));
    let before = page.dialogs.len();
    let _ = page.call("Page.navigate", json!({"url": "about:blank"}));
    assert_eq!(
        page.dialogs.len(),
        before,
        "nothing marked, nothing asked: {:?}",
        page.dialogs
    );
    drop(page);
    server.stop();

    println!(
        "AC8 in Chrome ({}): Back and a menu entry each got the Leave / Stay question and Stay kept the form and its address; navigating away got the browser's beforeunload question (dismissed, the page stayed); Leave left, and a navigation after it asked nothing",
        agent.as_str().unwrap_or("?")
    );
}

/// Home, and `layered-form` behind its menu entry: a form with a base button, a layer `LYR-POP` holding two labels (one
/// inside the form, one that runs past its right edge), and a button that shows the layer from COBOL.
fn layered_screens() -> App {
    use cobolt_forms::model::{Control, ControlType, Layer, PropValue};
    let mut main = form("main-form", "Home");
    main.controls.push(side_menu(480));
    form_event(
        &mut main,
        "onLoad",
        handler(&["MOVE \"home\" TO LBL-OUT::Caption."]),
    );
    let mut layered = form("layered-form", "Layered");
    layered.form_format = FormFormat::Embedded;
    layered.layers.push(Layer::new("LYR-POP"));
    form_event(
        &mut layered,
        "onLoad",
        handler(&["MOVE \"layered loaded\" TO LBL-OUT::Caption."]),
    );
    layered.controls.push(button(
        "BTN-SHOW",
        "Show",
        16,
        64,
        handler(&["MOVE 1 TO LYR-POP::Visible."]),
    ));
    for (id, x, w) in [("LBL-IN", 16, 240), ("LBL-EDGE", 520, 480)] {
        let mut c = Control::new(id, ControlType::Label, x, 200);
        c.set_prop("Caption", PropValue::String(format!("{id} in the layer")));
        c.rect.w = w;
        c.rect.h = 32;
        c.layer = Some("LYR-POP".to_owned());
        layered.controls.push(c);
    }
    build(Spec {
        style: "spa",
        forms: vec![main, layered],
        menus: vec![(
            "SideMenu-1",
            menu_def(&[("lay", "Layered", "layered-form")]),
        )],
    })
}

/// **AC11** — a route to a layered form shows the form with every layer hidden; the program then shows the layer, and its
/// controls appear within the form's own area in the shell's pane (the root clips what runs past the edge), never over the menu.
#[test]
fn ac11_a_route_to_a_layered_form_shows_every_layer_hidden_and_the_program_then_shows_it_within_the_pane() {
    let Some(exe) = find_chrome() else {
        eprintln!("SKIPPED: no Chrome on this machine (set CHROME to a Chrome or Chromium binary)");
        return;
    };
    let app = layered_screens();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, _server) = open(&chrome, &app, "");
    let agent = page.eval("navigator.userAgent").unwrap();

    // The route: the menu entry loads the form (the address follows).
    page.click_at(&entry("Layered")).unwrap();
    page.wait_for(
        "(() => { const r = document.getElementById('pr-app').shadowRoot; const l = r.querySelector('[data-pr-form=\"layered-form\"] [data-name=\"LBL-OUT\"]'); return !!l && l.textContent === 'layered loaded'; })()",
        40,
    )
    .unwrap_or_else(|e| panic!("the layered form did not load: {e}; the page shows {:?}", page.eval(LOOK)));
    assert_eq!(page.eval(LOOK).unwrap()["hash"], "#/layered-form");

    // Every layer is hidden and inert, its controls with it: nothing of the layer is drawn or takes the pointer.
    let state = "(() => { const r = document.getElementById('pr-app').shadowRoot; \
        const root = r.querySelector('[data-pr-form=\"layered-form\"]'); \
        const layer = root.querySelector('.pr-form-layer[data-layer=\"LYR-POP\"]'); \
        const inn = layer.querySelector('[data-name=\"LBL-IN\"]'); \
        const cs = getComputedStyle(layer); \
        return { rootClass: root.className, hidden: layer.hasAttribute('hidden'), inert: layer.hasAttribute('inert'), display: cs.display, \
                 innBox: (() => { const b = inn.getBoundingClientRect(); return b.width * b.height; })() }; })()";
    let before = page.eval(state).unwrap();
    assert_eq!(before["hidden"], true, "{before}");
    assert_eq!(before["inert"], true, "{before}");
    assert_eq!(before["display"], "none", "a hidden layer is not drawn: {before}");
    assert_eq!(before["innBox"], 0, "and its controls take no room: {before}");

    // The program shows the layer.
    page.click_at(&control_rect("layered-form", "BTN-SHOW")).unwrap();
    page.wait_for(
        "(() => { const r = document.getElementById('pr-app').shadowRoot; const l = r.querySelector('[data-pr-form=\"layered-form\"] .pr-form-layer[data-layer=\"LYR-POP\"]'); return !!l && !l.hasAttribute('hidden') && getComputedStyle(l).display !== 'none'; })()",
        40,
    )
    .unwrap_or_else(|e| panic!("the program did not show the layer: {e}"));

    // Where it is: the controls are inside the form's own area, to the right of the menu, and the part of `LBL-EDGE` that runs
    // past the form's right edge is clipped (the pointer there reaches nothing of the layer).
    let after = page
        .eval(
            "(() => { const r = document.getElementById('pr-app').shadowRoot; \
              const root = r.querySelector('[data-pr-form=\"layered-form\"]'); const rr = root.getBoundingClientRect(); \
              const rect = n => { const b = root.querySelector('[data-name=\"' + n + '\"]').getBoundingClientRect(); return { x: b.x, y: b.y, r: b.right, b: b.bottom }; }; \
              const menu = r.querySelector('.pr-sm-items').getBoundingClientRect(); \
              const inn = rect('LBL-IN'), edge = rect('LBL-EDGE'); \
              const past = r.elementFromPoint(rr.right + 4, edge.y + 8); \
              return { root: { x: rr.x, y: rr.y, r: rr.right, b: rr.bottom }, menuRight: menu.right, inn, edge, \
                       overflow: getComputedStyle(root).overflow, \
                       pastHitsLayer: !!(past && past.closest('[data-layer=\"LYR-POP\"]')) }; })()",
        )
        .unwrap();
    let f = |v: &serde_json::Value, k: &str| v[k].as_f64().unwrap();
    let (root, inn, edge) = (&after["root"], &after["inn"], &after["edge"]);
    assert_eq!(after["overflow"], "hidden", "the root clips what the layer draws: {after}");
    assert!(f(inn, "x") >= f(root, "x") && f(inn, "r") <= f(root, "r") && f(inn, "y") >= f(root, "y") && f(inn, "b") <= f(root, "b"), "LBL-IN lies within the form's area: {after}");
    assert!(f(root, "x") >= after["menuRight"].as_f64().unwrap() - 1.0, "the form, and so the layer, is in the pane, not over the menu: {after}");
    assert!(f(edge, "r") > f(root, "r"), "LBL-EDGE was designed to run past the form's right edge: {after}");
    assert_eq!(after["pastHitsLayer"], false, "what runs past the edge is clipped, so the pointer there reaches nothing of the layer: {after}");
    assert!(page.exceptions.is_empty(), "the page threw: {:?}", page.exceptions);

    println!(
        "AC11 in Chrome ({}): the route `#/layered-form` showed the form with its layer hidden, inert and taking no room; `MOVE 1 TO LYR-POP::Visible` showed it; its label lay inside the form's area (x {:.0}..{:.0} of {:.0}..{:.0}), right of the menu (edge {:.0}); the label designed past the edge was clipped (the pointer past it hit nothing of the layer); no page threw",
        agent.as_str().unwrap_or("?"),
        f(inn, "x"),
        f(inn, "r"),
        f(root, "x"),
        f(root, "r"),
        after["menuRight"].as_f64().unwrap()
    );
}

/// The caption of a form's label inside the application mounted in element `host` (an id), or `null`.
fn caption_in(host: &str, form_name: &str) -> String {
    format!(
        "(() => {{ const e = document.getElementById('{host}'); const r = e && e.shadowRoot; const l = r && r.querySelector('[data-pr-form=\"{form_name}\"] [data-name=\"LBL-OUT\"]'); return l ? l.textContent : null; }})()"
    )
}

/// The rectangle of the menu entry labelled `label` in the application mounted in element `host`.
fn entry_in(host: &str, label: &str) -> String {
    format!(
        "(() => {{ const e = document.getElementById('{host}'); const r = e && e.shadowRoot; const x = r && [...r.querySelectorAll('.pr-sm-items .pr-menu-label')].find(n => n.textContent === '{label}'); if (!x) return null; const b = x.getBoundingClientRect(); return {{x: b.x, y: b.y, width: b.width, height: b.height}}; }})()"
    )
}

/// **Two applications on one page** (WASM-011 T19, R5, Q7) — one of them owns the page's address, the other keeps its chain
/// to itself. The page that mounts a second application says, per mount, that it does not follow the address; a second that
/// asks to follow it too is refused with a message and the first goes on.
#[test]
fn two_applications_on_one_page_only_the_one_that_owns_the_address_writes_it() {
    let Some(exe) = find_chrome() else {
        eprintln!("SKIPPED: no Chrome on this machine (set CHROME to a Chrome or Chromium binary)");
        return;
    };
    let app = three_screens();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, _server) = open(&chrome, &app, "");
    let agent = page.eval("navigator.userAgent").unwrap();

    // The first application owns the address, by the Build's choice for a page that is the application.
    // The first application fills the window, so the others are given boxes in its corners that leave its menu free.
    page.eval("(() => { const boxes = { 'pr-app2': 'right:0;top:0;width:300px;height:200px', 'pr-app3': 'right:0;bottom:0;width:560px;height:420px' }; for (const [id, box] of Object.entries(boxes)) { const d = document.createElement('div'); d.id = id; d.style.cssText = 'position:fixed;z-index:10;background:#fff;overflow:hidden;' + box; document.body.appendChild(d); } return true; })()").unwrap();

    // One that does not follow the address mounts and shows its main form.
    let mounted = page
        .eval("(() => { const el = document.getElementById('pr-app3'); window.__why = []; el.addEventListener('pr-error', ev => window.__why.push(JSON.stringify(ev.detail))); return PowerRustCOBOL.mount(el, {address: 'none'}).then(() => 'mounted', e => 'failed: ' + (e && e.message ? e.message : e) + ' / ' + window.__why.join(' | ')); })()")
        .unwrap();
    page.wait_for(&format!("{} === 'home'", caption_in("pr-app3", "main-form")), 40)
        .unwrap_or_else(|e| {
            let held = page.eval("(() => { const e = document.getElementById('pr-app3'); const r = e && e.shadowRoot; return r ? r.textContent.slice(0, 300) : 'no shadow root'; })()");
            panic!(
                "the second application did not start: {e}; mount said {mounted}; console {:?}; the element holds {held:?}",
                page.console
            )
        });

    // The owner's menu changes the address; the other's does not.
    page.click_at(&entry("Orders")).unwrap();
    page.wait_for("location.hash === '#/orders-form'", 20)
        .unwrap_or_else(|e| panic!("the owner did not write the address: {e}; it is {:?}; app1 shows {:?}; app3 shows {:?}", page.eval("location.hash"), page.eval(LOOK), page.eval("[...document.getElementById('pr-app3').shadowRoot.querySelectorAll('[data-pr-form]')].map(e => e.dataset.prForm).join(',')")));
    page.click_at(&entry_in("pr-app3", "Customers")).unwrap();
    page.wait_for(&format!("{} === 'customers loaded'", caption_in("pr-app3", "customers-form")), 40)
        .unwrap_or_else(|e| panic!("the second application's own menu did nothing: {e}"));
    std::thread::sleep(Duration::from_millis(400));
    let hash = page.eval("location.hash").unwrap();
    assert_eq!(hash, json!("#/orders-form"), "the second application wrote the page's address");
    assert_eq!(page.eval(&caption_in("pr-app", "orders-form")).unwrap(), json!("orders loaded"), "the owner is still where it was");

    // A third application that asks to follow the address as well is refused, with a message, and the others go on.
    let refused = page
        .eval("(() => { const el = document.getElementById('pr-app2'); const why = []; el.addEventListener('pr-error', ev => why.push(String(ev.detail && ev.detail.detail))); return PowerRustCOBOL.mount(el, {address: 'own'}).then(() => 'mounted', e => 'refused: ' + (e && e.message ? e.message : e) + ' (' + why.join(' | ') + ')'); })()")
        .unwrap();
    let refused = refused.as_str().unwrap_or("").to_owned();
    assert!(refused.starts_with("refused:"), "the second owner was not refused: {refused:?}");
    assert!(refused.contains("another application on this page already follows the address"), "and the refusal says why and what to do: {refused:?}");
    assert!(page.eval(&caption_in("pr-app2", "main-form")).unwrap().is_null(), "and it shows nothing");
    assert_eq!(page.eval(&caption_in("pr-app", "orders-form")).unwrap(), json!("orders loaded"), "the owner went on");
    assert_eq!(page.eval(&caption_in("pr-app3", "customers-form")).unwrap(), json!("customers loaded"), "so did the other");
    assert_eq!(page.eval("location.hash").unwrap(), json!("#/orders-form"));
    assert!(page.exceptions.is_empty(), "the page threw: {:?}", page.exceptions);

    println!(
        "two applications on one page in Chrome ({}): the first owns the address; a second asking to follow it too was {:?}; one mounted with `address: 'none'` showed its own main form, and its Customers entry loaded `customers-form` in it without touching the address, which stayed `#/orders-form` where the owner's Orders entry had put it; no page threw",
        agent.as_str().unwrap_or("?"),
        refused
    );
}
