// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A Snackbar on a form opened with `OpenFormSync`, in a real browser** (the Cnd_PRC report, 2026-11-18).
//!
//! The form's Snackbar is `Size = Large`, `StackAnchor = TopCenter`, `Category = Error`, `Timeout = 0` and has one
//! button, `ok|OK||Left|true`. Its notification came out small at the page's bottom-left corner, and neither OK nor the ✕
//! closed it: every notification was drawn in one host at the page's corner, outside the form's modal `<dialog>`, where
//! the browser lets nothing be clicked. It is now drawn on the form's own surface — inside the dialog — at its anchor,
//! with its class's measurements, and both OK and the ✕ close it (and raise `onClosed`). With no Chrome on the machine
//! (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_forms::model::{Control, ControlType, EventBinding, PropValue};
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::Isolation;
use serde_json::{json, Value};
use support::cdp::{find_chrome, Chrome};
use support::project::{build, button, form, handler, App, Spec};

fn app() -> App {
    let mut main = form("main-form", "Main");
    main.controls.push(button(
        "BTN-SYNC",
        "Open",
        16,
        64,
        handler(&["INVOKE ME::\"OpenFormSync\"(\"dialog-form\")."]),
    ));
    let mut dialog = form("dialog-form", "Dialog");
    dialog.controls.push(button("BTN-SHOW", "Show", 16, 64, handler(&["MOVE \"open\" TO LBL-OUT::Caption", "INVOKE SNK-1::Show()."])));
    let mut snack = Control::new("SNK-1", ControlType::Snackbar, 16, 400);
    for (k, v) in [
        ("Text", "Senha incorreta"),
        ("Size", "Large"),
        ("StackAnchor", "TopCenter"),
        ("Category", "Error"),
        ("Timeout", "0"),
        ("Buttons", "ok|OK||Left|true"),
    ] {
        snack.set_prop(k, PropValue::String(v.to_owned()));
    }
    let mut closed = EventBinding::for_control("SNK-1", "onClosed");
    closed.code = handler(&["MOVE \"closed\" TO LBL-OUT::Caption."]);
    snack.events.push(closed);
    dialog.controls.push(snack);
    build(Spec { style: "spa", forms: vec![main, dialog], menus: vec![] })
}

/// Where the notification is, against the dialog form's own surface.
const SNACK: &str = r#"(() => {
  const root = document.getElementById('pr-app').shadowRoot;
  const form = root.querySelector('[data-pr-form="dialog-form"]');
  const t = root.querySelector('.pr-snackbar');
  if (!form || !t) return { shown: false };
  const f = form.getBoundingClientRect(), b = t.getBoundingClientRect();
  const dlg = t.closest('dialog');
  return {
    shown: true,
    in_modal: !!dlg && dlg.matches(':modal'),
    on_form: form.contains(t),
    size: t.dataset.size,
    width: b.width, height: b.height,
    top_gap: b.top - f.top,
    centre_offset: (b.left + b.width / 2) - (f.left + f.width / 2),
    caption: (form.querySelector('[data-name="LBL-OUT"]') || {}).textContent,
  };
})()"#;

fn rect(selector: &str) -> String {
    format!("(() => {{ const e = document.getElementById('pr-app').shadowRoot.querySelector('{selector}'); if (!e) return null; const b = e.getBoundingClientRect(); return {{x: b.x, y: b.y, width: b.width, height: b.height}}; }})()")
}

fn shown(present: bool) -> String {
    format!("!!document.getElementById('pr-app').shadowRoot.querySelector('.pr-snackbar') === {present}")
}

#[test]
fn a_snackbar_on_a_modal_form_sits_at_its_anchor_takes_its_size_and_closes_from_ok_and_the_cross() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let app = app();
    let server = serve(&app.bundle, "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1024, 768).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for("(() => { const r = document.getElementById('pr-app')?.shadowRoot; return !!(r && r.querySelector('[data-pr-form=\"main-form\"] [data-name=\"BTN-SYNC\"]')); })()", 40)
        .unwrap_or_else(|e| panic!("the application did not start: {e}"));

    page.click_at(&rect("[data-pr-form=\"main-form\"] [data-name=\"BTN-SYNC\"]")).unwrap_or_else(|e| panic!("{e}"));
    page.wait_for("!!document.getElementById('pr-app').shadowRoot.querySelector('dialog [data-pr-form=\"dialog-form\"]')", 20)
        .unwrap_or_else(|e| panic!("the modal form did not open: {e}"));

    let mut rows: Vec<String> = Vec::new();
    for (round, close) in [(1, ".pr-snackbar [data-part=\"toast-button\"]"), (2, ".pr-snackbar .pr-snack-close")] {
        page.click_at(&rect("[data-pr-form=\"dialog-form\"] [data-name=\"BTN-SHOW\"]")).unwrap_or_else(|e| panic!("{e}"));
        page.wait_for(&shown(true), 20).unwrap_or_else(|e| panic!("round {round}: the notification did not show: {e}; console {:?}", page.console));
        let s: Value = page.eval(SNACK).unwrap();
        assert_eq!((&s["in_modal"], &s["on_form"], &s["size"]), (&json!(true), &json!(true), &json!("Large")), "round {round}: on the modal form's surface: {s}");
        let f = |k: &str| s[k].as_f64().unwrap_or(f64::NAN);
        assert!(f("width") >= 320.0 && f("height") >= 76.0, "round {round}: a Large notification is at least 320 x 76: {s}");
        assert!((f("top_gap") - 16.0).abs() <= 1.0, "round {round}: TopCenter, Margin 16 from the form's top: {s}");
        assert!(f("centre_offset").abs() <= 1.0, "round {round}: centred on the form: {s}");
        page.click_at(&rect(close)).unwrap_or_else(|e| panic!("{e}"));
        page.wait_for(&shown(false), 20).unwrap_or_else(|e| panic!("round {round}: `{close}` did not close it: {e}; {}", page.eval(SNACK).unwrap()));
        page.wait_for("document.getElementById('pr-app').shadowRoot.querySelector('[data-pr-form=\"dialog-form\"] [data-name=\"LBL-OUT\"]').textContent === 'closed'", 20)
            .unwrap_or_else(|e| panic!("round {round}: onClosed did not run: {e}; {}", page.eval(SNACK).unwrap()));
        rows.push(format!("round {round}: {:.0} x {:.0} px, {:.0} px below the form's top, {:+.1} px off its centre, closed by `{close}`", f("width"), f("height"), f("top_gap"), f("centre_offset")));
    }
    drop(page);
    server.stop();
    println!("Snackbar on a modal form in Chrome:\n  {}", rows.join("\n  "));
}

/// The same report's SideMenu: rows a program adds in `onLoad` were never drawn (only the `.menu.yaml` rows), `AddItem`
/// answered nothing into a `PIC 9(1)` item, and `SetItemEnabled` stopped the handler with run-time error -4 ("not
/// available on a SideMenu in the browser"). The rows are now the menu's, and the answers are the desktop's.
#[test]
fn a_side_menus_rows_added_at_run_time_are_drawn_and_answer_as_on_the_desktop() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let mut main = form("main-form", "Home");
    main.controls.push(support::project::side_menu(480));
    support::project::form_event(
        &mut main,
        "onLoad",
        support::project::handler_with(
            &["01  WS-A PIC 9(1).", "01  WS-B PIC 9(1).", "01  WS-C PIC 9(3).", "01  WS-OUT PIC X(20)."],
            &[
                "MOVE SideMenu-1::AddItem(\"m1\", \"Cadastros\", \"\", \"\", \"open-form:second-form\") TO WS-A",
                "MOVE SideMenu-1::SetItemEnabled(\"home\", \"0\") TO WS-B",
                "MOVE SideMenu-1::GetCount() TO WS-C",
                "STRING WS-A \"/\" WS-B \"/\" WS-C DELIMITED BY SIZE INTO WS-OUT",
                "MOVE WS-OUT TO LBL-OUT::Caption.",
            ],
        ),
    );
    let mut second = form("second-form", "Second");
    second.form_format = cobolt_forms::model::FormFormat::Embedded;
    let app = build(Spec {
        style: "spa",
        forms: vec![main, second],
        menus: vec![("SideMenu-1", support::project::menu_def(&[("home", "Home", "second-form")]))],
    });
    let server = serve(&app.bundle, "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1024, 768).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    const LOOK: &str = r#"(() => {
  const r = document.getElementById('pr-app')?.shadowRoot;
  if (!r) return null;
  const out = r.querySelector('[data-pr-form="main-form"] [data-name="LBL-OUT"]');
  const rows = [...r.querySelectorAll('.pr-sm-items [data-id]')].map(b => ({ id: b.dataset.id, label: (b.querySelector('.pr-menu-label') || {}).textContent, disabled: b.disabled }));
  return { caption: out ? out.textContent : null, rows };
})()"#;
    page.wait_for("(() => { const r = document.getElementById('pr-app')?.shadowRoot; const o = r && r.querySelector('[data-pr-form=\"main-form\"] [data-name=\"LBL-OUT\"]'); return !!o && o.textContent.trim().length > 0; })()", 40)
        .unwrap_or_else(|e| panic!("onLoad did not finish: {e}; {:?}; console {:?}", page.eval(LOOK), page.console));
    let look: Value = page.eval(LOOK).unwrap();
    assert_eq!(look["caption"].as_str().map(str::trim), Some("1/1/001"), "AddItem 1, SetItemEnabled 1, GetCount 1: {look}");
    let rows = look["rows"].as_array().cloned().unwrap_or_default();
    let row = |id: &str| rows.iter().find(|r| r["id"] == json!(id)).cloned().unwrap_or_else(|| panic!("no row `{id}` drawn: {look}"));
    assert_eq!(row("m1")["label"], json!("Cadastros"), "the row onLoad added is drawn: {look}");
    assert_eq!(row("home")["disabled"], json!(true), "SetItemEnabled reached the designed row: {look}");
    drop(page);
    server.stop();
    println!("SideMenu in Chrome: onLoad answered {}, rows drawn {:?}", look["caption"], rows.iter().map(|r| format!("{}{}", r["id"].as_str().unwrap_or("?"), if r["disabled"] == json!(true) { " (disabled)" } else { "" })).collect::<Vec<_>>());
}
