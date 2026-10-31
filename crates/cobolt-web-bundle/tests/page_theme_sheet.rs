// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A new form's stylesheet reproduces its theme** (WASM-007 T28: R4, Q5; AC3). For each built-in web theme the
//! fixture's main form is drawn twice in the same headless Chrome — with the stylesheet the IDE generates for a new
//! form of that theme (`web_themes::form_stylesheet`), and with its properties alone — on a page whose theme is the
//! same (`theme.css`). The form's rectangle is captured both times and the pixels that differ by more than 8/255 in
//! any channel are counted; the plan's tolerance (§5) is 1 % of the pixels.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM};
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleOptions, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome, Page};

const TOLERANCE: f64 = 0.01;
const CHANNEL: u8 = 8;

const LIB: &str = "const root = document.getElementById('pr-app').shadowRoot;";

/// The main form drawn with `theme` on the page and, when `with_sheet`, the generated sheet of a new form of it.
fn capture(page: &mut Page, theme: &str, with_sheet: bool) -> (u32, u32, Vec<[u8; 3]>) {
    let player_out = support::built_player();
    let mut input = support::input_with(
        PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        BundleOptions { isolation: Isolation::RequireCorp, theme: Some(cobolt_forms::web_themes::theme_rules(theme)), ..Default::default() },
    );
    for f in input.forms.iter_mut().filter(|f| f.name == MAIN_FORM) {
        if with_sheet {
            let form = cobolt_forms::xml::load_form_from_str(&f.cfrm).unwrap_or_else(|e| panic!("{e}"));
            f.stylesheet = Some(cobolt_forms::web_themes::form_stylesheet(theme, &cobolt_forms::web_themes::used_types(&form)));
        }
    }
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for(&format!("(() => {{ {LIB} const l = root.querySelector('[data-name=\"{LBL_OUT}\"]'); return !!l && l.textContent === 'loaded'; }})()"), 30)
        .unwrap_or_else(|e| panic!("the application did not start: {e}; console {:?}", page.console));
    let adopted = page.eval(&format!("(() => {{ {LIB} return root.adoptedStyleSheets.map(s => s.cssRules.length); }})()")).unwrap();
    // Two frames, so that what the sheets changed is drawn.
    page.eval("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(() => r(true))))").unwrap();
    let r = page.eval(&format!("(() => {{ {LIB} const b = root.querySelector('[data-pr-form=\"{MAIN_FORM}\"]').getBoundingClientRect(); return [b.x, b.y, b.width, b.height]; }})()")).unwrap();
    let n = |i: usize| r[i].as_f64().unwrap();
    let shot = page.capture(n(0), n(1), n(2), n(3)).unwrap_or_else(|e| panic!("{e}"));
    println!("  {theme} {}: form {}x{} at ({}, {}); rules per adopted sheet {adopted}", if with_sheet { "with its sheet" } else { "properties only" }, n(2), n(3), n(0), n(1));
    drop(server);
    shot
}

#[test]
fn a_new_forms_stylesheet_draws_the_form_as_its_theme_does_in_chrome() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1000, 700).unwrap();
    let mut results = Vec::new();
    let mut plains = Vec::new();
    for theme in cobolt_forms::web_themes::THEMES {
        let plain = capture(&mut page, theme, false);
        let styled = capture(&mut page, theme, true);
        assert_eq!((plain.0, plain.1), (styled.0, styled.1), "{theme}: the same rectangle");
        let differ = plain.2.iter().zip(&styled.2).filter(|(a, b)| (0..3).any(|k| a[k].abs_diff(b[k]) > CHANNEL)).count();
        let ratio = differ as f64 / plain.2.len() as f64;
        results.push((theme, differ, plain.2.len(), ratio));
        plains.push(plain.2);
    }
    // A zero means something only if the themes themselves draw the form differently.
    let apart = |a: &[[u8; 3]], b: &[[u8; 3]]| a.iter().zip(b).filter(|(x, y)| (0..3).any(|k| x[k].abs_diff(y[k]) > CHANNEL)).count() as f64 / a.len() as f64;
    let (lg_el, el_sp) = (apart(&plains[0], &plains[1]), apart(&plains[1], &plains[2]));
    println!("  the themes themselves: liquid-glass vs elegance {:.2} %, elegance vs spatial {:.2} % of the pixels differ", lg_el * 100.0, el_sp * 100.0);
    assert!(lg_el > TOLERANCE && el_sp > TOLERANCE, "the page's theme did not change the drawing");
    println!("AC3 (Chrome {}): pixels differing by more than {CHANNEL}/255, generated sheet vs properties only:", exe.display());
    for (theme, differ, total, ratio) in &results {
        println!("  {theme}: {differ} of {total} = {:.4} % (tolerance {:.0} %)", ratio * 100.0, TOLERANCE * 100.0);
    }
    for (theme, _, _, ratio) in &results {
        assert!(*ratio <= TOLERANCE, "{theme}: {:.4} % of the pixels differ", ratio * 100.0);
    }
}
