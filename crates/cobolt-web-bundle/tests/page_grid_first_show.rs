// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A DataGrid inside a card that plays its entrance is drawn for the grid's LAYOUT box, not for a box caught mid-animation**
//! (PowerAnalyticsWeb, the first visit to Dados, 2026-10-29).
//!
//! The page watches the boxes of the controls that are drawn to their own size and draws them again when the box changes. It read
//! the box with `getBoundingClientRect`, which includes the transforms of the element and of every ancestor. The first time a page
//! is shown its card plays ZoomOut on Elastic over 1800 ms, so the observer's first report (the box going from `display: none` to
//! laid out) arrived with the card at a scale of 0.93 and the grid was drawn for a box 7 % too small: caption, header band, bars
//! and the CSV badge are placed by that size, the DOM scroll container fills the real one (`inset: 0`), and nothing corrected it
//! when the entrance ended because an observer does not fire when only a transform ends. Going to another page and back showed
//! the right picture (no entrance the second time).
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped.

mod support;

use cobolt_wasm_abi::fixture::{build, FixtureForm, FixtureHandler, FixtureSpec, Step, Target};
use cobolt_wasm_abi::manifest::Limits;
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome};

const MAIN: &str = "main-form";

fn spec(multi: bool) -> FixtureSpec {
    let t = Target::control;
    let set = |id: &str, v: &str| Step::Set { target: t(id), name: "Caption".into(), value: v.into() };
    let call = |id: &str, m: &str, args: &[&str]| Step::Call { target: t(id), name: m.into(), args: args.iter().map(|s| s.to_string()).collect() };
    let controls = [("LBL-OUT", "Label"), ("BTN-GO", "Button"), ("BTN-GRID", "Button"), ("PG", "Panel"), ("C-GRID", "GroupBox"), ("GRD", "DataGrid"), ("CH", "BarChart")];
    FixtureSpec {
        project: "gridfirst".into(),
        multi_memory: multi,
        limits: Limits::default(),
        forms: vec![FixtureForm {
            name: MAIN.into(),
            main: true,
            instances: 1,
            controls: controls.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
            handlers: vec![
                FixtureHandler::form("onLoad", vec![set("LBL-OUT", "loaded")]),
                // First click: the card's entrance starts while its page is still hidden (the page plays it when a screen is prepared).
                FixtureHandler::on("BTN-GO", "onClick", vec![call("C-GRID", "PlayAnimation", &["intro"]), call("GRD", "PlayAnimation", &["intro"]), set("LBL-OUT", "played")]),
                // Second click: the page is shown and the grid gets its data, in one batch, with the entrance already running.
                FixtureHandler::on(
                    "BTN-GRID",
                    "onClick",
                    vec![
                        call("PG", "Show", &[]),
                        Step::Set { target: t("GRD"), name: "Title".into(), value: "Fundos".into() },
                        Step::Set { target: t("GRD"), name: "Rows".into(), value: rows_text(60) },
                        set("LBL-OUT", "shown"),
                    ],
                ),
            ],
        }],
    }
}

fn rows_text(n: usize) -> String {
    (1..=n).map(|r| (1..=7).map(|c| format!("s{r}c{c}")).collect::<Vec<_>>().join("\t")).collect::<Vec<_>>().join("\n")
}

fn cfrm() -> String {
    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="main-form" width="1000" height="700" responsive="true">
  <Control id="LBL-OUT" type="Label" x="10" y="10" w="120" h="24"></Control>
  <Control id="BTN-GO" type="Button" x="10" y="44" w="90" h="28"><Property name="Caption">Go</Property></Control>
  <Control id="BTN-GRID" type="Button" x="110" y="44" w="90" h="28"><Property name="Caption">Grid</Property></Control>
  <Control id="PG" type="Panel" x="0" y="90" w="1000" h="600" visible="false"></Control>
  <Control id="C-GRID" type="GroupBox" x="40" y="100" w="900" h="560" parent="PG"><Property name="Expandable">true</Property><Property name="CornerRadius">28</Property><Animation name="intro" trigger="Programmatic" kind="ZoomOut" duration="1800" delay="0" easing="Elastic" repeat="Once" slide-dx="0" slide-dy="0"/></Control>
  <Control id="GRD" type="DataGrid" x="52" y="166" w="876" h="400" parent="C-GRID"><Animation name="intro" trigger="Programmatic" kind="ZoomOut" duration="1800" delay="0" easing="Elastic" repeat="Once" slide-dx="0" slide-dy="0"/><Property name="Columns">Name:string
Code:string
Value:number
Other:string
Another:string
More:string
Last:string</Property></Control>
  <Control id="CH" type="BarChart" x="60" y="580" w="300" h="70" parent="C-GRID"><Property name="__ChartData">A&#9;10&#10;B&#9;20&#10;C&#9;40</Property><Property name="ShowLegend">false</Property></Control>
</Form>
"##
    )
}

const LIB: &str = r#"
  const root = document.getElementById('pr-app').shadowRoot;
  const el = id => root.querySelector('[data-name="' + id + '"]');
  const text = id => { const e = el(id); return e ? e.textContent : null; };
  const form = () => root.querySelector('[data-pr-inst]');
  const anims = () => form().getAnimations({ subtree: true });
  // What the grid is painted for (its caption row is as wide as the box the painter was given) against the box it really has.
  const grid = () => { const g = el('GRD'); const cap = g.querySelector('.pr-dg-caption'); return { box: g.offsetWidth, boxh: g.offsetHeight, caption: cap ? parseFloat(cap.style.width) : null, sheet: g.querySelector('.pr-dg-sheet') ? parseFloat(g.querySelector('.pr-dg-sheet').style.width) : null }; };
"#;

fn js(body: &str) -> String {
    format!("(async () => {{ {LIB} {body} }})()")
}

#[test]
fn a_grid_in_a_card_that_plays_its_entrance_is_drawn_for_its_layout_box() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let (multi, single) = (build(&spec(true)), build(&spec(false)));
    let player_out = support::built_player();
    let input = BundleInput {
        project: "gridfirst".into(),
        forms: vec![FormInput { name: MAIN.into(), cfrm: cfrm(), stylesheet: None }],
        program_multi: Some(multi.wasm),
        program_single: Some(single.wasm),
        stylesheet: None,
        menus: Vec::new(),
        assets: Vec::new(),
        player: PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        options: BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    };
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1100, 760).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for(&js("return text('LBL-OUT') === 'loaded';"), 10).unwrap_or_else(|e| panic!("never loaded: {e}"));

    // ── the first show: the card plays ZoomOut on Elastic over 1800 ms ──────────────────────────────────────────
    // The second click comes 80 ms after the first (scale of the ZoomOut / Elastic curve there: about 0.4, far from 1), so the page's size
    // watch gets the grid's first box while the card is clearly scaled.
    let trace = page.eval(&js("el('BTN-GO').click(); await new Promise(r => setTimeout(r, 80)); el('BTN-GRID').click(); const t0 = performance.now(); const out = []; for (let k = 0; k < 14; k++) { await new Promise(r => requestAnimationFrame(r)); const c = el('C-GRID'); const g = grid(); out.push([Math.round(performance.now() - t0), c.offsetWidth ? +(c.getBoundingClientRect().width / c.offsetWidth).toFixed(3) : null, g.box, g.caption]); } return out;")).unwrap();
    println!("TRACE [ms, card scale, grid box, caption]: {trace}");
    page.wait_for(&js("return text('LBL-OUT') === 'shown' && el('GRD').offsetWidth > 0 && anims().length > 0;"), 5).unwrap_or_else(|e| panic!("the grid never appeared: {e}"));
    let mid = page.eval(&js("const c = el('C-GRID'); return { scale: c.getBoundingClientRect().width / c.offsetWidth, g: grid() };")).unwrap();
    page.wait_for(&js("return anims().length === 0;"), 10).unwrap_or_else(|e| panic!("the entrance did not end: {e}"));
    // a few frames after the entrance ended (a late correction would have come by now)
    std::thread::sleep(std::time::Duration::from_millis(400));
    let after = page.eval(&js("return { g: grid(), cap_w: el('GRD').querySelector('.pr-dg-caption').offsetWidth, chart: (() => { const s = el('CH').querySelector('svg'); return s ? s.getBoundingClientRect().width : null; })(), chart_box: el('CH').offsetWidth };")).unwrap();
    let (box_w, caption) = (after["g"]["box"].as_f64().unwrap(), after["g"]["caption"].as_f64().unwrap());
    println!(
        "first show of a card with a DataGrid: caught mid-entrance at scale {:.3} (grid box {} px, caption {} px); after the entrance the grid box is {box_w} px and its caption {caption} px wide, the chart's picture {:?} px for a {} px box",
        mid["scale"].as_f64().unwrap(), mid["g"]["box"], mid["g"]["caption"], after["chart"], after["chart_box"]
    );
    assert!((caption - box_w).abs() < 0.6, "the caption row (and with it the header band, the bars and the CSV badge) is drawn for the grid's real box, {box_w} px, not for one measured while the card was scaled: {after}");

    // ── the second show (no entrance): the same picture ─────────────────────────────────────────────────────────
    let again = page.eval(&js("const g = el('GRD'); const p = el('PG'); p.style.display = 'none'; await new Promise(r => setTimeout(r, 300)); p.style.display = ''; await new Promise(r => setTimeout(r, 600)); return grid();")).unwrap();
    assert!((again["caption"].as_f64().unwrap() - again["box"].as_f64().unwrap()).abs() < 0.6, "hiding the page and showing it again leaves the picture as it was: {again}");
}
