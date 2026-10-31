// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A layered form in a real browser** (WASM-004 T18: R14, R15, R17; AC9, AC10 and AC13 as far as a page can show them
//! before a program can write a layer, which WASM-010 T12 brings).
//!
//! The fixture's main form is given a radius, a base button at `ZOrder` 209, and two layers: a card at `ZOrder` 0 and a
//! DataGrid at the form's rounded corner in the first, a label in the second. A headless Chrome reads the element tree and
//! the computed styles: the root clips with the form's radius, each layer is a stacking context after the base, hidden and
//! inert as every layer starts, its controls inside it; and the pointer at a hidden layer's control reaches the base.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM};
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleOptions, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome};

fn layered_form(plain: &str) -> String {
    assert!(plain.contains("width=\"640\" height=\"480\""), "the fixture's form is 640 x 480");
    let sized = plain.replace("width=\"640\" height=\"480\"", "width=\"900\" height=\"600\" index-base=\"1\" corner-radius=\"24\"");
    let layers = r##"  <Layer name="LYR-POP" color="#10203080"/>
  <Layer name="LYR-TOP" color="#203040"/>
  <Control id="BTN-BASE" type="Button" x="300" y="100" w="160" h="60" z-order="209"><Property name="Caption">Base</Property></Control>
  <Control id="PNL-POP" type="Panel" x="280" y="80" w="240" h="120" z-order="0" layer="LYR-POP"></Control>
  <Control id="DG-CORNER" type="DataGrid" x="0" y="500" w="200" h="100" layer="LYR-POP"></Control>
  <Control id="LBL-TOP" type="Label" x="600" y="20" w="120" h="24" layer="LYR-TOP"><Property name="Caption">top</Property></Control>
  <Control id="BTN-OPEN" type="Button" x="50" y="300" w="100" h="30"><Property name="Caption">Open</Property></Control>
  <Control id="DG-OFF" type="DataGrid" x="600" y="300" w="200" h="100" enabled="false" layer="LYR-POP"></Control>
  <Control id="LBL-SEE" type="Label" x="40" y="290" w="200" h="50" layer="LYR-POP"><Property name="BackgroundColor">#00000000</Property><Property name="Caption">see</Property></Control>
"##;
    sized.replace("</Form>", &format!("{layers}</Form>"))
}

const LIB: &str = "const root = document.getElementById('pr-app').shadowRoot; const el = id => root.querySelector('[data-name=\"' + id + '\"]');";

#[test]
fn a_layered_form_is_a_clipping_root_holding_the_base_then_each_layer_hidden_and_inert() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let player_out = support::built_player();
    let mut input = support::input_with(
        PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    );
    for f in input.forms.iter_mut().filter(|f| f.name == MAIN_FORM) {
        f.cfrm = layered_form(&f.cfrm);
    }
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1100, 700).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for(&format!("(() => {{ {LIB} const l = el('{LBL_OUT}'); return !!l && l.textContent === 'loaded'; }})()"), 30)
        .unwrap_or_else(|e| panic!("the application did not start: {e}"));

    let r = page
        .eval(&format!(
            "(() => {{ {LIB} \
             const form = root.querySelector('[data-pr-form=\"{MAIN_FORM}\"]'); const cs = getComputedStyle(form); \
             const kids = Array.from(form.children).map(k => k.className + (k.dataset.layer ? ':' + k.dataset.layer : '')); \
             const layers = Array.from(form.querySelectorAll(':scope > .pr-form-layer')).map(l => {{ const s = getComputedStyle(l); \
               return {{ name: l.dataset.layer, color: s.color === cs.color, hidden: l.hasAttribute('hidden'), inert: l.hasAttribute('inert'), display: s.display, z: s.zIndex, isolation: s.isolation, \
                 pointer: s.pointerEvents, bg: getComputedStyle(l.querySelector(':scope > .pr-layer-bg')).backgroundColor, \
                 first: l.firstElementChild.className, members: Array.from(l.children).slice(1).map(c => c.dataset.name) }}; }}); \
             const base = form.querySelector(':scope > .pr-form'); \
             const b = el('BTN-BASE').getBoundingClientRect(); const hit = root.elementFromPoint(b.x + 20, b.y + 20); \
             return {{ tag: form.tagName, cls: form.className, overflow: cs.overflow, radius: cs.borderTopLeftRadius, isolation: cs.isolation, kids, layers, \
               base_z: getComputedStyle(base).zIndex, base_in_surface: el('BTN-BASE').parentElement.className, \
               hit: hit && hit.closest('[data-name]') && hit.closest('[data-name]').dataset.name }}; }})()"
        ))
        .unwrap();
    println!("{}", serde_json::to_string_pretty(&r).unwrap());
    assert_eq!(r["cls"], "pr-form-root");
    assert_eq!(r["overflow"], "hidden", "the root clips everything the form shows (R17)");
    assert_eq!(r["radius"], "24px", "to the form's arcs");
    assert_eq!(r["isolation"], "isolate");
    assert_eq!(r["kids"], serde_json::json!(["pr-form", "pr-form-layer:LYR-POP", "pr-form-layer:LYR-TOP"]), "the base, then each layer in stack order");
    assert_eq!(r["base_z"], "0", "the base is a stacking context of its own, so its ZOrder 209 never reaches a layer");
    assert_eq!(r["base_in_surface"], "pr-surface");
    let layers = r["layers"].as_array().unwrap();
    for (k, l) in layers.iter().enumerate() {
        assert_eq!(l["hidden"], true, "{l}");
        assert_eq!(l["inert"], true, "{l}");
        assert_eq!(l["display"], "none", "a hidden layer is not drawn (R15)");
        assert_eq!(l["z"], (k + 1).to_string(), "layer {k} stacks above the one before");
        assert_eq!(l["isolation"], "isolate");
        assert_eq!(l["pointer"], "none");
        assert_eq!(l["first"], "pr-layer-bg", "the backdrop is the layer's first child (R18)");
        assert_eq!(l["color"], true, "a layer's controls take the theme's text from the root, as the base's do");
    }
    assert_eq!(layers[0]["members"], serde_json::json!(["PNL-POP", "DG-CORNER", "DG-OFF", "LBL-SEE"]), "AC13: the DataGrid is inside its layer, inside the root");
    // Chrome writes the colour's alpha 128/255 (`0.502` in the rule) back as `0.5`.
    assert_eq!(layers[0]["bg"], "rgba(16, 32, 48, 0.5)", "the backdrop's alpha is the colour's, no opacity");
    assert_eq!(layers[1]["members"], serde_json::json!(["LBL-TOP"]));
    assert_eq!(r["hit"], "BTN-BASE", "the pointer over a hidden layer's card reaches the base button beneath");

    // ── T19, AC11: hit testing by the browser ────────────────────────────────────────────────────────────────────
    // No program can show a layer before WASM-010 T12, so the page shows them as the player's next frame would
    // (`hidden` and `inert` removed from the layer and from its controls); every rule under test is already in the sheet.
    let hit = page
        .eval(&format!(
            "(() => {{ {LIB} \
             const form = root.querySelector('[data-pr-form=\"{MAIN_FORM}\"]'); const ls = form.querySelectorAll(':scope > .pr-form-layer'); \
             const show = l => [l, ...l.querySelectorAll('[data-name]')].forEach(e => {{ e.removeAttribute('hidden'); e.removeAttribute('inert'); }}); \
             const at = (x, y) => {{ const e = root.elementFromPoint(x, y); const c = e && e.closest('[data-name]'); return e ? (c ? c.dataset.name + ':' + e.className : e.className) : null; }}; \
             const mid = e => {{ const r = e.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; }}; \
             const open = mid(el('BTN-OPEN')); \
             show(ls[0]); const text = mid(el('LBL-SEE').querySelector('.pr-label-text')); \
             const see = {{ open: at(...open), text: at(...text), box: getComputedStyle(el('LBL-SEE')).pointerEvents, off: getComputedStyle(el('DG-OFF')).pointerEvents, on: getComputedStyle(el('DG-CORNER')).pointerEvents }}; \
             show(ls[1]); \
             let bubbled = 0; const count = () => bubbled++; form.addEventListener('click', count); \
             const bg = ls[1].querySelector(':scope > .pr-layer-bg'); bg.click(); form.removeEventListener('click', count); \
             return {{ see, opaque: at(...open), bg_pointer: getComputedStyle(bg).pointerEvents, pop_pointer: getComputedStyle(ls[0].firstElementChild).pointerEvents, bubbled }}; }})()"
        ))
        .unwrap();
    println!("{}", serde_json::to_string_pretty(&hit).unwrap());
    assert!(hit["see"]["open"].as_str().unwrap_or("").starts_with("BTN-OPEN:"), "a see-through layer and a see-through Label's box let the pointer reach the base");
    assert_eq!(hit["see"]["text"], "LBL-SEE:pr-label-text", "a see-through Label is hit on its text");
    assert_eq!(hit["see"]["box"], "none");
    assert_eq!(hit["see"]["on"], "auto", "a control of a shown layer takes the pointer");
    assert_eq!(hit["see"]["off"], "none", "a disabled one does not, in a layer as in the base (DataGrid guardrail review)");
    assert_eq!(hit["pop_pointer"], "none", "the translucent layer's backdrop takes nothing");
    assert_eq!(hit["bg_pointer"], "auto", "the opaque layer's backdrop takes the pointer");
    assert_eq!(hit["opaque"], "pr-layer-bg", "AC11: at an opaque point the backdrop, not the button beneath, is hit");
    assert_eq!(hit["bubbled"], 0, "and its click ends there, reaching nothing above it");
    drop(server);
}
