// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **What the designer computes against what Chrome computes** (WASM-036 T21, R1, R4, AC2).
//!
//! The designer's cascade (`cobolt_forms::css_effective::effective`) says which declaration wins for a
//! control and what value it gives. This test puts one control per level of the cascade on a 412 × 915
//! form — a stored property, the project sheet, the form sheet, a class, an id, `CssStyle`, an
//! `!important` rule, an `@media (max-width: 500px)` rule and a `:hover` rule — opens it in a headless
//! Chrome at that size, and compares `getComputedStyle(...).backgroundColor` of each control's element
//! with the engine's value, and the engine's source with the one the case was built to have.
//!
//! The theme layer is not compared here: the page's theme is the player's own sheet, which the
//! engine reads from the form's theme, not from a stylesheet of this fixture.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_forms::css::{MediaEnv, Scheme};
use cobolt_forms::css_effective::{effective, source_text, Origin, SheetSet, SheetSource};
use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM};
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleOptions, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome};

const PROJECT_SHEET: &str = "#P-PROJECT { background-color: #223344 }\n#P-IMP { background-color: #ff0000 !important }\n";
const FORM_SHEET: &str = "#P-FORM { background-color: #334455 }\n.card { background-color: #445566 }\n#P-ID { background-color: #556677 }\n#P-IMP { background-color: #00ff00 }\n@media (max-width: 500px) { #P-MEDIA { background-color: #667788 } }\n#P-HOVER:hover { background-color: #778899 }\n#P-STYLE { background-color: #888888 }\n";

/// `(id, properties, the source the case is built to have, the colour it is built to give)`.
type Case = (&'static str, &'static [(&'static str, &'static str)], &'static str, &'static str);

const CASES: [Case; 9] = [
    ("P-STORED", &[("BackgroundColor", "#112233")], "property", "#112233"),
    ("P-PROJECT", &[], "webforms/application.css:1", "#223344"),
    ("P-FORM", &[], "webforms/main-form.css:1", "#334455"),
    ("P-CLASS", &[("CssClass", "card")], "webforms/main-form.css:2", "#445566"),
    ("P-ID", &[("CssClass", "card")], "webforms/main-form.css:3", "#556677"),
    ("P-STYLE", &[("CssStyle", "background-color: #99aabb")], "Style", "#99aabb"),
    ("P-IMP", &[("BackgroundColor", "#112233")], "webforms/application.css:2", "#ff0000"),
    ("P-MEDIA", &[], "webforms/main-form.css:5", "#667788"),
    ("P-HOVER", &[("BackgroundColor", "#112233")], "property", "#112233"),
];

fn cfrm_with_cases(plain: &str) -> String {
    assert!(plain.contains("width=\"640\" height=\"480\""), "the fixture's form is 640 x 480");
    let sized = plain.replace("width=\"640\" height=\"480\"", "width=\"412\" height=\"915\" index-base=\"1\"");
    let mut controls = String::new();
    for (i, (id, props, _, _)) in CASES.iter().enumerate() {
        let body: String = props.iter().map(|(k, v)| format!("<Property name=\"{k}\">{v}</Property>")).collect();
        controls.push_str(&format!("  <Control id=\"{id}\" type=\"Panel\" x=\"240\" y=\"{}\" w=\"150\" h=\"40\">{body}</Control>\n", 20 + 60 * i));
    }
    sized.replace("</Form>", &format!("{controls}</Form>"))
}

/// `rgb(r, g, b)` or `rgba(r, g, b, a)` as `#rrggbb` (alpha must be opaque).
fn hex_of(computed: &str) -> String {
    let inner = computed.trim_start_matches("rgba(").trim_start_matches("rgb(").trim_end_matches(')');
    let n: Vec<f64> = inner.split(',').map(|p| p.trim().parse().unwrap_or(f64::NAN)).collect();
    assert!(n.len() >= 3 && (n.len() == 3 || (n[3] - 1.0).abs() < 1e-3), "an opaque colour, got {computed}");
    format!("#{:02x}{:02x}{:02x}", n[0] as u8, n[1] as u8, n[2] as u8)
}

const LIB: &str = "const root = document.getElementById('pr-app').shadowRoot; const el = id => root.querySelector('[data-name=\"' + id + '\"]');";

#[test]
fn the_designers_cascade_equals_chromes_computed_style_at_every_level() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let t0 = std::time::Instant::now();
    let player_out = support::built_player();
    let mut input = support::input_with(
        PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    );
    let mut cfrm = String::new();
    for f in input.forms.iter_mut().filter(|f| f.name == MAIN_FORM) {
        f.cfrm = cfrm_with_cases(&f.cfrm);
        f.stylesheet = Some(FORM_SHEET.to_owned());
        cfrm = f.cfrm.clone();
    }
    input.stylesheet = Some(PROJECT_SHEET.to_owned());
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(412, 915).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for(&format!("(() => {{ {LIB} const l = el('{LBL_OUT}'); return !!l && l.textContent === 'loaded' && !!el('P-HOVER'); }})()"), 30)
        .unwrap_or_else(|e| panic!("the application did not start: {e}"));

    let form = cobolt_forms::xml::load_form_from_str(&cfrm).unwrap_or_else(|e| panic!("{e}"));
    let sources = [
        SheetSource { label: "webforms/application.css", text: PROJECT_SHEET, origin: Origin::Project },
        SheetSource { label: "webforms/main-form.css", text: FORM_SHEET, origin: Origin::Form },
    ];
    let set = SheetSet::parse(&sources);
    let env = MediaEnv { width: 412.0, height: 915.0, scheme: Scheme::Light };
    let mut agree = 0;
    println!("{:<10} {:<28} {:<10} {:<10}", "control", "source", "designer", "Chrome");
    for (id, _, want_source, want_colour) in CASES {
        let eff = effective(&form, Some(id), &set, &env);
        // No entry: no sheet beats the properties layer, so the stored property is the value in effect.
        let (designer, source) = match eff.get("BackgroundColor") {
            Some(e) => (e.value.to_property_text(), source_text(e)),
            None => (stored_of(id), "property".to_owned()),
        };
        let designer_hex = format!("#{}", &designer.trim_start_matches('#')[..6]).to_ascii_lowercase();
        let chrome_colour = page.eval(&format!("(() => {{ {LIB} return getComputedStyle(el('{id}')).backgroundColor; }})()")).unwrap();
        let chrome_hex = hex_of(chrome_colour.as_str().unwrap_or_default());
        println!("{id:<10} {:<28} {designer_hex:<10} {chrome_hex:<10}", source);
        assert_eq!(source, want_source, "{id}: the designer names the source the case was built to have");
        assert_eq!(designer_hex, want_colour, "{id}: the designer gives the value the case was built to give");
        assert_eq!(chrome_hex, designer_hex, "{id}: Chrome's computed colour equals the designer's");
        agree += 1;
    }
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("┌─ css_effective_chrome: the designer's cascade against Chrome\n│ {} levels (stored, project sheet, form sheet, class, id, CssStyle, !important, @media, :hover) at 412 x 915, Light\n│ Chrome: {}\n│ time: {ms:.0} ms\n└─ {agree} of {} agree, {} differ", CASES.len(), exe.display(), CASES.len(), CASES.len() - agree);
    assert_eq!(agree, CASES.len());
}

/// The control's own stored `BackgroundColor`.
fn stored_of(id: &str) -> String {
    CASES.iter().find(|c| c.0 == id).and_then(|c| c.1.iter().find(|p| p.0 == "BackgroundColor")).map(|p| p.1.to_owned()).unwrap_or_default()
}
