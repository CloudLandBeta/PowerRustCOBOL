// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The cascade, the isolation and the layer guard in a real browser** (WASM-007 T26: R8, R9, R15, R16; AC5, AC6,
//! AC10). The fixture's main form gains a Label `LBL-CAS` whose `background-color` is set at every level of the cascade
//! (plan §1.1) — the theme block of the form's sheet, the property, the project sheet, the form sheet, `CssStyle` — and
//! a layer. A headless Chrome reads the computed value with all the levels, then with the top one removed, and so on
//! down; each level's selector is chosen so that only the cascade, not specificity, can explain the winner. Then the
//! host page and the form sheet each style `button` and neither crosses the shadow boundary, `--pr-accent` set on the
//! mount element reaches the controls, and a form sheet that tries to restack and disable the layer — applied anyway,
//! as the Build would refuse it — changes nothing.
//!
//! The sheets are built here, as Rust strings; no `.css` file is committed. With no Chrome on the machine (`CHROME`
//! names one) the test says it was skipped, and why.

mod support;

use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM};
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleOptions, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome, Page};

const LIB: &str = "const root = document.getElementById('pr-app').shadowRoot; const el = id => root.querySelector('[data-name=\"' + id + '\"]');";

/// Level `k` (1–6 of plan §1.1) paints `rgb(k, k, k)`; each selector is chosen so the lower level has the HIGHER
/// specificity, so that only the cascade can make the higher level win.
fn level(k: u8) -> String {
    format!("rgb({k}, {k}, {k})")
}

/// The main form with `LBL-CAS` (its `BackgroundColor` property at level 3 when `top >= 3`, its `CssStyle` at level 6
/// when `top >= 6`) and one layer holding a Label.
fn form(plain: &str, top: u8) -> String {
    let mut props = String::new();
    if top >= 3 {
        props.push_str("<Property name=\"BackgroundColor\">#030303</Property>");
    }
    if top >= 6 {
        props.push_str("<Property name=\"CssStyle\">background-color: rgb(6, 6, 6)</Property>");
    }
    let extra = format!(
        "  <Layer name=\"LYR-A\" color=\"#10203080\"/>\n  <Control id=\"LBL-CAS\" type=\"Label\" x=\"300\" y=\"20\" w=\"120\" h=\"30\"><Property name=\"Caption\">cascade</Property>{props}</Control>\n  <Control id=\"LBL-LYR\" type=\"Label\" x=\"300\" y=\"200\" w=\"120\" h=\"30\" layer=\"LYR-A\"><Property name=\"Caption\">layer</Property></Control>\n"
    );
    assert!(plain.contains("width=\"640\" height=\"480\""), "the fixture's form is 640 x 480");
    plain.replace("width=\"640\" height=\"480\"", "width=\"640\" height=\"480\" index-base=\"1\"").replace("</Form>", &format!("{extra}</Form>"))
}

/// The form sheet: the generated theme block (level 2) and the developer's section (level 5).
fn form_sheet(top: u8) -> Option<String> {
    let mut s = String::new();
    if top >= 2 {
        s.push_str("@layer pr-theme {\n  #LBL-CAS#LBL-CAS#LBL-CAS.pr-label { background-color: rgb(2, 2, 2); }\n}\n");
    }
    if top >= 5 {
        s.push_str(".pr-label { background-color: rgb(5, 5, 5); }\nbutton { color: rgb(0, 0, 255); }\n");
    }
    (!s.is_empty()).then_some(s)
}

struct Served {
    _dir: tempfile::TempDir,
    server: cobolt_web_bundle::devserver::Server,
}

fn served(top: u8) -> Served {
    let player_out = support::built_player();
    let mut input = support::input_with(
        PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    );
    for f in input.forms.iter_mut().filter(|f| f.name == MAIN_FORM) {
        f.cfrm = form(&f.cfrm, top);
        f.stylesheet = form_sheet(top);
    }
    if top >= 4 {
        input.stylesheet = Some("#LBL-CAS#LBL-CAS.pr-label { background-color: rgb(4, 4, 4); }\n".into());
    }
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    Served { _dir: dir, server }
}

fn open(page: &mut Page, s: &Served) {
    page.navigate(&format!("{}/index.html", s.server.url())).unwrap();
    page.wait_for(&format!("(() => {{ {LIB} const l = el('{LBL_OUT}'); return !!l && l.textContent === 'loaded' && !!el('LBL-CAS'); }})()"), 30)
        .unwrap_or_else(|e| panic!("the application did not start: {e}; console {:?}", page.console));
}

#[test]
fn the_cascade_steps_down_its_levels_and_nothing_crosses_the_form_or_moves_a_layer_in_chrome() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1000, 700).unwrap();

    // ── AC5: all six levels, then the top one removed, down to the player's own ──────────────────────────────────
    let mut steps = Vec::new();
    let mut base = String::new();
    for top in (1..=6u8).rev() {
        let s = served(top);
        open(&mut page, &s);
        let got = page.eval(&format!("(() => {{ {LIB} return getComputedStyle(el('LBL-CAS')).backgroundColor; }})()")).unwrap();
        let got = got.as_str().unwrap_or("").to_owned();
        steps.push(format!("levels 1-{top}: {got}"));
        if top >= 2 {
            assert_eq!(got, level(top), "with levels 1 to {top} the visible value is level {top}'s; steps so far {steps:?}");
        } else {
            base = got.clone();
            assert!(!(2..=6).any(|k| got == level(k)), "level 1 alone is the player's own: {got}");
        }
    }
    println!("AC5 (Chrome): {}", steps.join("; "));

    // ── AC6: the host page and the form sheet each style `button`; neither crosses. `--pr-accent` does ───────────
    let s = served(6);
    open(&mut page, &s);
    let iso = page
        .eval(&format!(
            "(() => {{ {LIB} \
             const host = new CSSStyleSheet(); host.replaceSync('button {{ color: rgb(255, 0, 0) !important; }}'); document.adoptedStyleSheets = [...document.adoptedStyleSheets, host]; \
             const b = document.createElement('button'); b.id = 'host-btn'; b.textContent = 'host'; document.body.appendChild(b); \
             const inner = root.querySelector('[data-pr-form=\"{MAIN_FORM}\"] button'); \
             const app = document.getElementById('pr-app'); const before = getComputedStyle(el('LBL-CAS')).getPropertyValue('--pr-accent').trim(); \
             app.style.setProperty('--pr-accent', 'rgb(1, 2, 3)'); \
             return {{ host: getComputedStyle(b).color, inner: inner && getComputedStyle(inner).color, before, after: getComputedStyle(el('LBL-CAS')).getPropertyValue('--pr-accent').trim() }}; }})()"
        ))
        .unwrap();
    println!("AC6 (Chrome): {iso}");
    assert_eq!(iso["host"], "rgb(255, 0, 0)", "the form sheet's `button` rule does not reach the host page's button");
    assert!(iso["inner"].is_string(), "the form has a button: {iso}");
    assert_eq!(iso["inner"], "rgb(0, 0, 255)", "the form's button follows the form sheet, not the host page's important rule");
    assert_eq!(iso["after"], "rgb(1, 2, 3)", "--pr-accent on the mount element reaches the controls (it was {})", iso["before"]);

    // ── AC10: the guard holds against a form sheet that tries to restack, disable and show the layer ─────────────
    let read = format!(
        "(() => {{ {LIB} const r = root.querySelector('[data-pr-form=\"{MAIN_FORM}\"]'); const l = r.querySelector(':scope > .pr-form-layer'); const f = r.querySelector(':scope > .pr-form'); \
         const a = getComputedStyle(l), b = getComputedStyle(r), c = getComputedStyle(f); \
         return {{ layer: [a.position, a.zIndex, a.pointerEvents, a.isolation, a.display, a.overflow], root: [b.position, b.overflow, b.isolation, b.zIndex], form: [c.position, c.zIndex] }}; }})()"
    );
    let before = page.eval(&read).unwrap();
    page.eval(&format!(
        "(async () => {{ const a = await PowerRustCOBOL.mount(document.getElementById('pr-app')); \
         a.player.set_form_stylesheet('{MAIN_FORM}', '.pr-form-layer {{ position: static !important; z-index: 0 !important; pointer-events: auto !important; isolation: auto !important; display: block !important; overflow: hidden !important; }} \
         .pr-form-root {{ position: static !important; overflow: visible !important; isolation: auto !important; z-index: 9 !important; }} .pr-form {{ position: static !important; z-index: 7 !important; }}'); return true; }})()"
    ))
    .unwrap();
    let applied = page.eval(&format!("(() => {{ {LIB} return root.adoptedStyleSheets.map(s => [...s.cssRules].map(r => r.cssText).join(' ')).some(t => t.includes('z-index: 0 !important')); }})()")).unwrap();
    let after = page.eval(&read).unwrap();
    println!("AC10 (Chrome): offending sheet adopted {applied}; before {before}; after {after}");
    assert_eq!(applied, true, "the offending sheet is in force");
    assert_eq!(after, before, "the layer guard holds");
    assert_eq!(before["layer"][1], "1", "{before}");
    assert_eq!(before["layer"][2], "none", "{before}");
    println!("level 1 alone (base + Liquid Glass): {base}");
    drop(s);
}
