// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A Viewer shows the document a program holds, in a real browser** (the Cnd_PRC report, defect 8, 2026-11-18).
//!
//! A Web application's Backend builds a letter and hands its text to the form; the form gives it to the Viewer with
//! `LoadBytes`. The browser stopped the handler with run-time error -4 ("the method LOADBYTES is not available on a
//! Viewer in the browser") although the reference lists no such exception. The Viewer now draws the text, Markdown and
//! HTML subset documents a program gives it, raises `onLoaded`, and refuses a PDF (binary: a file of the application,
//! `Source`) with `onError` and a reason. With no Chrome on the machine (`CHROME` names one) the test says it was
//! skipped, and why.

mod support;

use cobolt_forms::model::{Control, ControlType, EventBinding, PropValue, Rect};
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::Isolation;
use serde_json::{json, Value};
use support::cdp::{find_chrome, Chrome};
use support::project::{build, button, form, handler_with, App, Spec};

/// A button whose click puts `text` in `WS-DOC` and hands it to the Viewer.
fn loads(id: &str, caption: &str, x: i32, text: &str) -> Control {
    button(id, caption, x, 16, handler_with(&["01  WS-DOC PIC X(120)."], &[&format!("MOVE \"{text}\" TO WS-DOC"), "INVOKE VWR-1::LoadBytes(WS-DOC)."]))
}

fn app() -> App {
    let mut main = form("main-form", "Carta");
    main.controls.push(loads("BTN-MD", "Markdown", 16, "# Carta ao morador"));
    main.controls.push(loads("BTN-HTML", "Html", 120, "<html><body><h2>Segunda parte</h2><p>Texto do aviso</p></body></html>"));
    main.controls.push(loads("BTN-PDF", "Pdf", 224, "%PDF-1.7 1 0 obj"));
    let mut viewer = Control::new("VWR-1", ControlType::Viewer, 16, 64);
    viewer.rect = Rect { x: 16, y: 64, w: 600, h: 300 };
    for (event, text) in [("onLoaded", "loaded"), ("onError", "error")] {
        let mut ev = EventBinding::for_control("VWR-1", event);
        ev.code = handler_with(&[], &[&format!("MOVE \"{text}\" TO LBL-OUT::Caption.")]);
        viewer.events.push(ev);
    }
    viewer.set_prop("ShowToolbar", PropValue::Bool(false));
    main.controls.push(viewer);
    build(Spec { style: "spa", forms: vec![main], menus: vec![] })
}

fn rect(selector: &str) -> String {
    format!("(() => {{ const e = document.getElementById('pr-app').shadowRoot.querySelector('{selector}'); if (!e) return null; const b = e.getBoundingClientRect(); return {{x: b.x, y: b.y, width: b.width, height: b.height}}; }})()")
}

/// What the page shows: the Viewer's headings and error line, and the caption the handlers wrote.
const LOOK: &str = r#"(() => {
  const r = document.getElementById('pr-app')?.shadowRoot;
  if (!r) return null;
  const f = r.querySelector('[data-pr-form="main-form"]');
  const v = f && f.querySelector('[data-name="VWR-1"]');
  const text = e => e ? e.textContent.trim() : null;
  return {
    h1: text(v && v.querySelector('h1')),
    h2: text(v && v.querySelector('h2')),
    paragraph: text(v && v.querySelector('p:not(.pr-error)')),
    error: text(v && v.querySelector('.pr-error')),
    caption: text(f && f.querySelector('[data-name="LBL-OUT"]')),
  };
})()"#;

#[test]
fn a_viewer_draws_the_document_the_program_loads_and_refuses_a_pdf_with_a_reason() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let started = std::time::Instant::now();
    let app = app();
    let server = serve(&app.bundle, "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1024, 768).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for("(() => { const r = document.getElementById('pr-app')?.shadowRoot; return !!(r && r.querySelector('[data-pr-form=\"main-form\"] [data-name=\"BTN-MD\"]')); })()", 40)
        .unwrap_or_else(|e| panic!("the application did not start: {e}"));
    let click = |page: &mut support::cdp::Page, id: &str| page.click_at(&rect(&format!("[data-pr-form=\"main-form\"] [data-name=\"{id}\"]"))).unwrap_or_else(|e| panic!("{e}"));

    // Markdown: the heading is drawn and `onLoaded` ran.
    click(&mut page, "BTN-MD");
    page.wait_for("(() => { const r = document.getElementById('pr-app').shadowRoot; const h = r.querySelector('[data-pr-form=\"main-form\"] [data-name=\"VWR-1\"] h1'); return !!h && h.textContent.includes('Carta'); })()", 20)
        .unwrap_or_else(|e| panic!("the Markdown document was not drawn: {e}; {:?}; console {:?}", page.eval(LOOK), page.console));
    let md: Value = page.eval(LOOK).unwrap();
    assert_eq!(md["h1"], json!("Carta ao morador"), "{md}");
    assert_eq!(md["caption"], json!("loaded"), "onLoaded ran: {md}");

    // HTML: the next document replaces the first.
    click(&mut page, "BTN-HTML");
    page.wait_for("(() => { const r = document.getElementById('pr-app').shadowRoot; const h = r.querySelector('[data-pr-form=\"main-form\"] [data-name=\"VWR-1\"] h2'); return !!h && h.textContent.includes('Segunda'); })()", 20)
        .unwrap_or_else(|e| panic!("the HTML document was not drawn: {e}; {:?}; console {:?}", page.eval(LOOK), page.console));
    let html: Value = page.eval(LOOK).unwrap();
    assert_eq!((&html["h2"], &html["paragraph"], &html["h1"]), (&json!("Segunda parte"), &json!("Texto do aviso"), &Value::Null), "the second document replaced the first: {html}");

    // A PDF is binary: refused with `onError` and a reason that names `Source`.
    click(&mut page, "BTN-PDF");
    page.wait_for("(() => { const r = document.getElementById('pr-app').shadowRoot; const e = r.querySelector('[data-pr-form=\"main-form\"] [data-name=\"VWR-1\"] .pr-error'); return !!e && e.textContent.includes('Source'); })()", 20)
        .unwrap_or_else(|e| panic!("the PDF was not refused with a reason: {e}; {:?}; console {:?}", page.eval(LOOK), page.console));
    let pdf: Value = page.eval(LOOK).unwrap();
    assert_eq!(pdf["caption"], json!("error"), "onError ran: {pdf}");
    drop(page);
    server.stop();
    println!(
        "Viewer LoadBytes in Chrome: Markdown drew `{}` (onLoaded), HTML replaced it with `{}` / `{}`, a PDF was refused with onError: `{}`, in {:.0} ms",
        md["h1"].as_str().unwrap_or("?"),
        html["h2"].as_str().unwrap_or("?"),
        html["paragraph"].as_str().unwrap_or("?"),
        pdf["error"].as_str().unwrap_or("?"),
        started.elapsed().as_secs_f64() * 1000.0
    );
}
