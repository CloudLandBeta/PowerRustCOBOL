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

fn has_form(name: &str, present: bool) -> String {
    format!("(() => {{ const r = document.getElementById('pr-app').shadowRoot; return !!r.querySelector('[data-pr-form=\"{name}\"]') === {present}; }})()")
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
