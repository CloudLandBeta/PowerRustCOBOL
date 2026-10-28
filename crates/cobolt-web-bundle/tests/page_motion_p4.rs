// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **What moves on a page, against the desktop's own arithmetic, in a real browser** (the PowerAnalyticsWeb parity
//! job, P4, 2026-10-28). The operator: "animations are nowhere near the desktop experience"; the page played none.
//!
//! * **`PlayAnimation("intro")`** (ZoomOut / Elastic / 1800 ms, PowerAnalytics' 160 "intro" calls) on a card and on a
//!   label inside it. The page's animations are frozen at fixed instants and the boxes the browser draws are measured:
//!   each must be the desktop's — its own rectangle scaled about its own centre by `cobolt_forms::anim::anim_transform`
//!   (the desktop's function, linked here) — the label too, although its card shrinks around it. After the animation
//!   nothing is left: no animation, no inline style, the boxes as before. `StopAnimation` on the card mid-flight puts it
//!   back at once and leaves the label playing; playing again restarts.
//! * **`AnimateOnLoad`**: a bar chart shown by `Show()` grows from zero over its `AnimationDuration` with the painter's
//!   ease-out (`1 − (1 − t)³`, `render.rs`): every sampled bar height gives the same start instant.
//! * **`SetColumnWidth`** of a DataGrid widens the column (it was a command the page ignored).
//! * **`me::Width`, `me::Height`, `me::Breakpoint`, `onResizing`, `onBreakpointChanged`, `onResize`** (scout D1): read at
//!   load they are the page's size, not the designed 900 × 600; a resize of the window raises the events and the
//!   handlers read the new values (the desktop's breakpoints are `Compact:0;Medium:600;Expanded:1024`).
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped.

mod support;

use cobolt_forms::model::{AnimKind, AnimRepeat, AnimTrigger, AnimationDef, EasingKind};
use cobolt_wasm_abi::fixture::{build, FixtureForm, FixtureHandler, FixtureSpec, Step, Target};
use cobolt_wasm_abi::manifest::Limits;
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome, Page};

const MAIN: &str = "main-form";

fn spec(multi: bool) -> FixtureSpec {
    let t = Target::control;
    let set = |id: &str, v: &str| Step::Set { target: t(id), name: "Caption".into(), value: v.into() };
    let echo = |prop: &str, into: &str| vec![Step::Get { target: Target::Form, name: prop.into() }, Step::EchoAnswer { target: t(into), name: "Caption".into() }];
    let call = |id: &str, m: &str, args: &[&str]| Step::Call { target: t(id), name: m.into(), args: args.iter().map(|s| s.to_string()).collect() };
    let room = || [echo("Width", "LBL-W"), echo("Height", "LBL-H"), echo("Breakpoint", "LBL-BP")].concat();
    FixtureSpec {
        project: "motion4".into(),
        multi_memory: multi,
        limits: Limits::default(),
        forms: vec![FixtureForm {
            name: MAIN.into(),
            main: true,
            instances: 1,
            controls: vec![
                ("LBL-OUT".into(), "Label".into()),
                ("LBL-W".into(), "Label".into()),
                ("LBL-H".into(), "Label".into()),
                ("LBL-BP".into(), "Label".into()),
                ("LBL-RS".into(), "Label".into()),
                ("LBL-RSG".into(), "Label".into()),
                ("LBL-BPC".into(), "Label".into()),
                ("BTN-PLAY".into(), "Button".into()),
                ("BTN-STOP".into(), "Button".into()),
                ("BTN-SHOW".into(), "Button".into()),
                ("BTN-GW".into(), "Button".into()),
                ("BTN-DATA".into(), "Button".into()),
                ("PNL-CARD".into(), "Panel".into()),
                ("LBL-IN".into(), "Label".into()),
                ("CH-BAR".into(), "BarChart".into()),
                ("GRD-1".into(), "DataGrid".into()),
            ],
            handlers: vec![
                FixtureHandler::form("onLoad", [room(), vec![set("LBL-OUT", "loaded")]].concat()),
                FixtureHandler::form("onResize", [room(), echo("Width", "LBL-RS")].concat()),
                FixtureHandler::form("onResizing", vec![set("LBL-RSG", "resizing")]),
                FixtureHandler::form("onBreakpointChanged", echo("Breakpoint", "LBL-BPC")),
                FixtureHandler::on("BTN-PLAY", "onClick", vec![call("PNL-CARD", "PlayAnimation", &["intro"]), call("LBL-IN", "PlayAnimation", &["intro"]), set("LBL-OUT", "played")]),
                FixtureHandler::on("BTN-STOP", "onClick", vec![call("PNL-CARD", "StopAnimation", &[]), set("LBL-OUT", "stopped")]),
                FixtureHandler::on("BTN-SHOW", "onClick", vec![call("CH-BAR", "Show", &[]), set("LBL-OUT", "shown")]),
                FixtureHandler::on("BTN-DATA", "onClick", vec![Step::Set { target: t("CH-BAR"), name: "__ChartData".into(), value: "A\t10\nB\t40\nC\t20".into() }, set("LBL-OUT", "new data")]),
                FixtureHandler::on("BTN-GW", "onClick", vec![call("GRD-1", "SetColumnWidth", &["1", "250"]), set("LBL-OUT", "widened")]),
            ],
        }],
    }
}

const INTRO: &str = r#"<Animation name="intro" trigger="Programmatic" kind="ZoomOut" duration="1800" delay="0" easing="Elastic" repeat="Once" slide-dx="0" slide-dy="0"/>"#;

fn cfrm() -> String {
    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="main-form" width="900" height="600" responsive="true">
  <Breakpoints>
    <Breakpoint name="Compact" min-width="0" font-factor="1"/>
    <Breakpoint name="Medium" min-width="600" font-factor="1"/>
    <Breakpoint name="Expanded" min-width="1024" font-factor="1"/>
  </Breakpoints>
  <Control id="LBL-OUT" type="Label" x="10" y="10" w="120" h="24"></Control>
  <Control id="LBL-W" type="Label" x="140" y="10" w="60" h="24"></Control>
  <Control id="LBL-H" type="Label" x="210" y="10" w="60" h="24"></Control>
  <Control id="LBL-BP" type="Label" x="280" y="10" w="90" h="24"></Control>
  <Control id="LBL-RS" type="Label" x="380" y="10" w="60" h="24"></Control>
  <Control id="LBL-RSG" type="Label" x="450" y="10" w="90" h="24"></Control>
  <Control id="LBL-BPC" type="Label" x="550" y="10" w="90" h="24"></Control>
  <Control id="BTN-PLAY" type="Button" x="10" y="44" w="90" h="28"><Property name="Caption">Play</Property></Control>
  <Control id="BTN-STOP" type="Button" x="110" y="44" w="90" h="28"><Property name="Caption">Stop</Property></Control>
  <Control id="BTN-SHOW" type="Button" x="210" y="44" w="90" h="28"><Property name="Caption">Show</Property><Animation name="press" trigger="OnClick" kind="Pulse" duration="400" delay="0" easing="Linear" repeat="Once" slide-dx="0" slide-dy="0"/></Control>
  <Control id="LBL-FL" type="Label" x="700" y="10" w="90" h="24"><Property name="Caption">fade</Property><Animation name="hello" trigger="OnFormLoad" kind="FadeIn" duration="4000" delay="0" easing="Linear" repeat="Once" slide-dx="0" slide-dy="0"/></Control>
  <Control id="BTN-GW" type="Button" x="310" y="44" w="90" h="28"><Property name="Caption">Widen</Property></Control>
  <Control id="BTN-DATA" type="Button" x="410" y="44" w="90" h="28"><Property name="Caption">Data</Property></Control>
  <Control id="PNL-CARD" type="Panel" x="60" y="120" w="240" h="160"><Property name="BackgroundColor">#3E7BE8</Property>{INTRO}</Control>
  <Control id="LBL-IN" type="Label" x="20" y="30" w="100" h="40" parent="PNL-CARD"><Property name="Caption">in</Property><Property name="BackgroundColor">#F5A524</Property>{INTRO}</Control>
  <Control id="CH-BAR" type="BarChart" x="360" y="100" w="300" h="200" visible="false"><Property name="__ChartData">A&#9;10&#10;B&#9;20&#10;C&#9;40</Property><Property name="AnimateOnLoad">true</Property><Property name="AnimationDuration">1500</Property><Property name="AnimateValues">true</Property><Property name="ShowLegend">false</Property></Control>
  <Control id="GRD-1" type="DataGrid" x="60" y="320" w="500" h="120"><Property name="Columns">A,B</Property></Control>
</Form>
"##
    )
}

const LIB: &str = r#"
  const root = document.getElementById('pr-app').shadowRoot;
  const el = id => root.querySelector('[data-name="' + id + '"]');
  const text = id => { const e = el(id); return e ? e.textContent : null; };
  const box = id => { const r = el(id).getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height, cx: r.x + r.width / 2, cy: r.y + r.height / 2 }; };
  const form = () => root.querySelector('[data-pr-inst]');
  const anims = () => form().getAnimations({ subtree: true });
"#;

fn js(body: &str) -> String {
    format!("(async () => {{ {LIB} {body} }})()")
}

fn click(page: &mut Page, id: &str) {
    page.wait_for(&js(&format!("const e = el('{id}'); if (!e) return false; const r = e.getBoundingClientRect(); const top = root.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2); return top === e || e.contains(top);")), 10)
        .unwrap_or_else(|e| panic!("{id} is not what a click at its centre reaches: {e}"));
    page.click_at(&js(&format!("const r = el('{id}').getBoundingClientRect(); return {{ x: r.x, y: r.y, width: r.width, height: r.height }};"))).unwrap();
}

fn wait_text(page: &mut Page, id: &str, want: &str) {
    page.wait_for(&js(&format!("return text('{id}') === '{want}';")), 10)
        .unwrap_or_else(|e| panic!("{id} never read {want:?} ({e}); it reads {:?}", page.eval(&js(&format!("return text('{id}');"))).unwrap()));
}

fn intro() -> AnimationDef {
    let mut a = AnimationDef::new("intro");
    a.trigger = AnimTrigger::Programmatic;
    a.kind = AnimKind::ZoomOut;
    a.duration_ms = 1800;
    a.easing = EasingKind::Elastic;
    a.repeat = AnimRepeat::Once;
    a
}

/// The desktop's scale of the intro `ms` after it started (`cobolt_forms::anim`, the function the desktop draws with).
fn desktop_scale(ms: f64) -> f64 {
    cobolt_forms::anim::anim_transform(&intro(), 900.0, 600.0, (ms / 1800.0).clamp(0.0, 1.0) as f32).2 as f64
}

#[test]
fn the_page_moves_as_the_desktop_moves_and_tells_the_program_its_size() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let (multi, single) = (build(&spec(true)), build(&spec(false)));
    let player_out = support::built_player();
    let input = BundleInput {
        project: "motion4".into(),
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
    page.resize(1100, 700).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    wait_text(&mut page, "LBL-OUT", "loaded");

    // ── the size the first handler reads is the page's, not the designed 900 x 600 ──────────────────────────────
    let room = page.eval(&js("const r = form().getBoundingClientRect(); return { w: text('LBL-W'), h: text('LBL-H'), bp: text('LBL-BP'), fw: r.width, fh: r.height };")).unwrap();
    assert_eq!(room["w"].as_str().unwrap(), format!("{}", room["fw"].as_f64().unwrap().round() as i64), "me::Width at onLoad: {room}");
    assert_eq!(room["h"].as_str().unwrap(), format!("{}", room["fh"].as_f64().unwrap().round() as i64), "me::Height at onLoad: {room}");
    assert_eq!(room["bp"], "Expanded", "{room}");
    assert_ne!(room["w"], "900", "not the designed width");
    // An `OnFormLoad` animation starts with the form (a 4 s FadeIn: still running, its opacity on the desktop's straight line).
    let fade = page
        .eval(&js("const a = el('LBL-FL').getAnimations().find(x => x.effect.getKeyframes().some(k => 'opacity' in k)); return a ? { t: a.currentTime, o: Number(getComputedStyle(el('LBL-FL')).opacity) } : null;"))
        .unwrap();
    assert!(fade.is_object(), "OnFormLoad: the label fades in with the form");
    let (ft, fo) = (fade["t"].as_f64().unwrap(), fade["o"].as_f64().unwrap());
    assert!((fo - ft / 4000.0).abs() < 0.02, "FadeIn / Linear: opacity {fo} at {ft} ms, the desktop's {}", ft / 4000.0);

    // ── PlayAnimation("intro") on the card and on the label inside it ────────────────────────────────────────────
    let before = page.eval(&js("return { card: box('PNL-CARD'), lbl: box('LBL-IN') };")).unwrap();
    click(&mut page, "BTN-PLAY");
    wait_text(&mut page, "LBL-OUT", "played");
    page.wait_for(&js("return el('PNL-CARD').getAnimations().length > 0 && el('LBL-IN').getAnimations().length > 0;"), 5)
        .unwrap_or_else(|e| panic!("no animation was started: {e}"));
    // Freeze every animation of the form at fixed instants and measure what the browser draws.
    let mut worst = (0.0f64, 0.0f64);
    let mut rows = Vec::new();
    for ms in [0.0, 60.0, 120.0, 180.0, 300.0, 420.0, 540.0, 720.0, 900.0, 1200.0, 1500.0, 1799.0] {
        let m = page
            .eval(&js(&format!("for (const a of anims()) {{ a.pause(); a.currentTime = {ms}; }} await new Promise(r => requestAnimationFrame(r)); return {{ card: box('PNL-CARD'), lbl: box('LBL-IN') }};")))
            .unwrap();
        let want = desktop_scale(ms);
        for (id, b0) in [("card", &before["card"]), ("lbl", &before["lbl"])] {
            let got = m[id]["w"].as_f64().unwrap() / b0["w"].as_f64().unwrap();
            let got_h = m[id]["h"].as_f64().unwrap() / b0["h"].as_f64().unwrap();
            let dc = (m[id]["cx"].as_f64().unwrap() - b0["cx"].as_f64().unwrap()).hypot(m[id]["cy"].as_f64().unwrap() - b0["cy"].as_f64().unwrap());
            worst.0 = worst.0.max((got - want).abs()).max((got_h - want).abs());
            worst.1 = worst.1.max(dc);
            assert!((got - want).abs() < 0.006 && (got_h - want).abs() < 0.006, "{id} at {ms} ms: scale {got:.4} x {got_h:.4}, the desktop's {want:.4}");
            assert!(dc < 0.75, "{id} at {ms} ms: its centre moved {dc:.2} px (the desktop scales each control about its own centre)");
        }
        rows.push(format!("{ms:>6} ms  desktop {want:.4}  card {:.4}  label {:.4}", m["card"]["w"].as_f64().unwrap() / before["card"]["w"].as_f64().unwrap(), m["lbl"]["w"].as_f64().unwrap() / before["lbl"]["w"].as_f64().unwrap()));
    }
    println!("intro (ZoomOut / Elastic / 1800 ms), page vs desktop, worst scale error {:.4}, worst centre drift {:.2} px:\n  {}", worst.0, worst.1, rows.join("\n  "));
    // Let them run out: nothing is left behind.
    page.eval(&js("for (const a of anims()) { a.currentTime = 1799; a.play(); } return true;")).unwrap();
    let after = page
        .wait_for(&js("if (anims().length) return false; return { card: box('PNL-CARD'), lbl: box('LBL-IN'), style: el('PNL-CARD').getAttribute('style') + '|' + el('LBL-IN').getAttribute('style') };"), 5)
        .unwrap_or_else(|e| panic!("the animations did not finish: {e}"));
    assert_eq!(after["card"], before["card"], "the card rests where it was laid out");
    assert_eq!(after["lbl"], before["lbl"]);
    assert_eq!(after["style"], "null|null", "no inline style is left behind");

    // ── StopAnimation on the card mid-flight; playing again restarts ─────────────────────────────────────────────
    click(&mut page, "BTN-PLAY");
    page.wait_for(&js("return el('PNL-CARD').getAnimations().length > 0;"), 5).unwrap();
    click(&mut page, "BTN-STOP");
    wait_text(&mut page, "LBL-OUT", "stopped");
    // The card is back at once; what may still run on it is only the lifting of its clipping while the label inside it
    // grows past its place (the desktop lets a growing control bleed out of its container).
    let moving = "el('PNL-CARD').getAnimations().filter(a => a.effect.getKeyframes().some(k => 'transform' in k)).length";
    let stopped = page
        .wait_for(&js(&format!("if ({moving}) return false; return {{ card: box('PNL-CARD'), lbl: el('LBL-IN').getAnimations().length }};")), 5)
        .unwrap_or_else(|e| panic!("StopAnimation did not stop the card: {e}"));
    assert_eq!(stopped["card"], before["card"], "stopped: back at its designed place at once");
    assert!(stopped["lbl"].as_u64().unwrap() > 0, "the label's own animation goes on: {stopped}");
    page.wait_for(&js("return anims().length === 0;"), 5).unwrap_or_else(|e| panic!("the label's animation did not end: {e}"));
    let rest = page.eval(&js("return [box('LBL-IN'), el('PNL-CARD').getAttribute('style'), el('LBL-IN').getAttribute('style')];")).unwrap();
    assert_eq!(rest[0], before["lbl"]);
    assert_eq!((&rest[1], &rest[2]), (&serde_json::Value::Null, &serde_json::Value::Null), "nothing left behind after a stop either");

    // ── AnimateOnLoad: the chart grows when it is shown ─────────────────────────────────────────────────────────
    click(&mut page, "BTN-SHOW");
    // (and the button's own `OnClick` animation, a Pulse, plays on the click)
    page.wait_for(&js("return el('BTN-SHOW').getAnimations().length > 0;"), 5).unwrap_or_else(|e| panic!("OnClick played nothing: {e}"));
    let samples = page
        .eval(&js(
            "const t0 = performance.now(); const out = []; while (performance.now() - t0 < 2200) { await new Promise(r => requestAnimationFrame(r)); const c = el('CH-BAR'); if (!c || !c.checkVisibility()) continue; let h = 0; for (const r of c.querySelectorAll('rect[fill]')) h = Math.max(h, r.getBBox().height); out.push([performance.now(), h]); } return out;",
        ))
        .unwrap();
    let samples: Vec<(f64, f64)> = samples.as_array().unwrap().iter().map(|p| (p[0].as_f64().unwrap(), p[1].as_f64().unwrap())).collect();
    let full = samples.last().unwrap().1;
    assert!(full > 20.0, "the tallest bar at rest: {full}");
    // Each height h at time t gives the instant the growth started: t − 1500·(1 − ∛(1 − h/H)).
    let starts: Vec<f64> = samples.iter().filter(|(_, h)| *h > 0.02 * full && *h < 0.98 * full).map(|(t, h)| t - 1500.0 * (1.0 - (1.0 - h / full).cbrt())).collect();
    assert!(starts.len() >= 20, "the chart grew over many frames, not at once: {} frames in motion", starts.len());
    let mean = starts.iter().sum::<f64>() / starts.len() as f64;
    let spread = starts.iter().map(|s| (s - mean).abs()).fold(0.0, f64::max);
    assert!(spread < 25.0, "every frame agrees on one start (the desktop's ease-out over 1500 ms): spread {spread:.1} ms");
    println!("AnimateOnLoad: {} frames in motion, the implied start within {spread:.1} ms of their mean (ease-out 1 - (1 - t)^3 over 1500 ms)", starts.len());

    // ── AnimateValues: new data travels from the values shown, with the same ease over the same 1500 ms ─────────────
    // From A 10, B 20, C 40 to A 10, B 40, C 20. The plot is scaled to its tallest bar, so B/C = (20 + 20k) / (40 − 20k)
    // gives the eased progress k of every frame, and k = 1 − (1 − t/1500)³ gives the instant the travel started.
    click(&mut page, "BTN-DATA");
    let travel = page
        .eval(&js(
            "const t0 = performance.now(); const out = []; while (performance.now() - t0 < 2000) { await new Promise(r => requestAnimationFrame(r)); const bars = [...el('CH-BAR').querySelectorAll('rect[fill]')].map(r => r.getBBox()).filter(b => b.height > 0).sort((a, b) => a.x - b.x); if (bars.length === 3) out.push([performance.now(), bars[1].height / bars[2].height]); } return out;",
        ))
        .unwrap();
    let travel: Vec<(f64, f64)> = travel.as_array().unwrap().iter().map(|p| (p[0].as_f64().unwrap(), p[1].as_f64().unwrap())).collect();
    let ks: Vec<(f64, f64)> = travel.iter().map(|(t, r)| (*t, (40.0 * r - 20.0) / (20.0 + 20.0 * r))).collect();
    assert!((ks.last().unwrap().1 - 1.0).abs() < 0.01, "it lands on the new data: k {}", ks.last().unwrap().1);
    let moving: Vec<f64> = ks.iter().filter(|(_, k)| *k > 0.02 && *k < 0.98).map(|(t, k)| t - 1500.0 * (1.0 - (1.0 - k).cbrt())).collect();
    assert!(moving.len() >= 20, "the data travelled over many frames: {}", moving.len());
    let mean = moving.iter().sum::<f64>() / moving.len() as f64;
    let spread = moving.iter().map(|s| (s - mean).abs()).fold(0.0, f64::max);
    assert!(spread < 25.0, "every frame agrees on one start of the travel: spread {spread:.1} ms");
    println!("AnimateValues: {} frames travelling, the implied start within {spread:.1} ms of their mean (ease-out over 1500 ms)", moving.len());

    // ── SetColumnWidth ───────────────────────────────────────────────────────────────────────────────────────────
    click(&mut page, "BTN-GW");
    wait_text(&mut page, "LBL-OUT", "widened");
    let w = page.wait_for(&js("const c = el('GRD-1').querySelector('.pr-dg-hc[data-c=\"0\"]'); return c && c.style.width === '250px' ? c.style.width : false;"), 5)
        .unwrap_or_else(|e| panic!("the first column was not widened: {e}; {}", page.eval(&js("const c = el('GRD-1').querySelector('.pr-dg-hc[data-c]'); return c ? c.outerHTML : null;")).unwrap()));
    assert_eq!(w, "250px");

    // ── a resize of the window: onResizing, onBreakpointChanged, onResize; the handlers read the new room ────────
    page.resize(800, 600).unwrap();
    wait_text(&mut page, "LBL-BPC", "Medium");
    wait_text(&mut page, "LBL-RS", "800");
    let m = page.eval(&js("return [text('LBL-W'), text('LBL-H'), text('LBL-BP'), text('LBL-RSG')];")).unwrap();
    assert_eq!(m, serde_json::json!(["800", "600", "Medium", "resizing"]));
    page.resize(500, 640).unwrap();
    wait_text(&mut page, "LBL-BPC", "Compact");
    wait_text(&mut page, "LBL-RS", "500");
    assert_eq!(page.eval(&js("return [text('LBL-W'), text('LBL-H'), text('LBL-BP')];")).unwrap(), serde_json::json!(["500", "640", "Compact"]));
    println!("resize: 1100x700 Expanded -> 800x600 Medium -> 500x640 Compact; me::Width/Height/Breakpoint read by the handlers");
    assert!(page.exceptions.is_empty(), "uncaught exceptions: {:?}", page.exceptions);
    drop(server);
}
