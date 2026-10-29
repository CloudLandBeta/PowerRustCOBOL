// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **What moves on a page, the rows judgement P4-1 failed** (2026-10-28), in a real browser:
//!
//! * **Re-flow during an animation.** The desktop draws every control at its CURRENT laid-out rectangle on every frame
//!   (`render.rs`). A card playing the intro with a label inside it, in a flex row; a second card shown mid-intro takes
//!   half the row, so the first shrinks: in every frame after that the label is drawn about its own laid-out centre.
//! * **Area, Scatter and Monochrome charts grow** on `AnimateOnLoad` (`render.rs` moves every chart kind; the page moved
//!   five, and none in their Monochrome look).
//! * **A fading container does not fade what is inside it** (`render.rs`: a control's alpha is its ancestors' static
//!   Opacity times its OWN animation's): a FadeIn label inside a FadeOut panel ends fully visible, a still label inside it
//!   never fades, the panel's face does.
//! * **`OnFocus` plays only for a control with a bound `onGotFocus`** (`render.rs` `focus_keyboard_events` puts the event
//!   in the stream only then, and the form host takes the trigger from the stream).
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped.

mod support;

use cobolt_wasm_abi::fixture::{build, FixtureForm, FixtureHandler, FixtureSpec, Step, Target};
use cobolt_wasm_abi::manifest::Limits;
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome, Page};

const MAIN: &str = "main-form";

fn spec(multi: bool) -> FixtureSpec {
    let t = Target::control;
    let set = |id: &str, v: &str| Step::Set { target: t(id), name: "Caption".into(), value: v.into() };
    let call = |id: &str, m: &str, args: &[&str]| Step::Call { target: t(id), name: m.into(), args: args.iter().map(|s| s.to_string()).collect() };
    let controls = [
        ("LBL-OUT", "Label"),
        ("BTN-PLAY", "Button"),
        ("BTN-SHOW2", "Button"),
        ("BTN-CH", "Button"),
        ("BTN-FADE", "Button"),
        ("ROW", "Panel"),
        ("C1", "Panel"),
        ("LBL-C1", "Label"),
        ("C2", "Panel"),
        ("CH-AREA", "AreaChart"),
        ("CH-SC", "ScatterChart"),
        ("CH-MONO", "BarChart"),
        ("PNL-F", "Panel"),
        ("LBL-FI", "Label"),
        ("LBL-STILL", "Label"),
        ("TXT-A", "TextBox"),
        ("TXT-B", "TextBox"),
    ];
    FixtureSpec {
        project: "motion4b".into(),
        multi_memory: multi,
        limits: Limits::default(),
        forms: vec![FixtureForm {
            name: MAIN.into(),
            main: true,
            instances: 1,
            controls: controls.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
            handlers: vec![
                FixtureHandler::form("onLoad", vec![set("LBL-OUT", "loaded")]),
                FixtureHandler::on("BTN-PLAY", "onClick", vec![call("C1", "PlayAnimation", &["intro"]), call("LBL-C1", "PlayAnimation", &["intro"]), set("LBL-OUT", "played")]),
                FixtureHandler::on("BTN-SHOW2", "onClick", vec![call("C2", "Show", &[]), set("LBL-OUT", "shown2")]),
                FixtureHandler::on("BTN-CH", "onClick", vec![call("CH-AREA", "Show", &[]), call("CH-SC", "Show", &[]), call("CH-MONO", "Show", &[]), set("LBL-OUT", "charts")]),
                FixtureHandler::on("BTN-FADE", "onClick", vec![call("PNL-F", "PlayAnimation", &["out"]), call("LBL-FI", "PlayAnimation", &["in"]), set("LBL-OUT", "faded")]),
                FixtureHandler::on("TXT-B", "onGotFocus", vec![set("LBL-OUT", "focus-b")]),
            ],
        }],
    }
}

const INTRO: &str = r#"<Animation name="intro" trigger="Programmatic" kind="ZoomOut" duration="1800" delay="0" easing="Elastic" repeat="Once" slide-dx="0" slide-dy="0"/>"#;

fn cfrm() -> String {
    let anim = |name: &str, trigger: &str, kind: &str, ms: u32| {
        format!(r#"<Animation name="{name}" trigger="{trigger}" kind="{kind}" duration="{ms}" delay="0" easing="Linear" repeat="Once" slide-dx="0" slide-dy="0"/>"#)
    };
    let chart = |id: &str, ty: &str, x: i32, extra: &str| {
        format!(
            r#"<Control id="{id}" type="{ty}" x="{x}" y="330" w="220" h="160" visible="false"><Property name="__ChartData">A&#9;10&#10;B&#9;20&#10;C&#9;40&#10;D&#9;30</Property><Property name="AnimateOnLoad">true</Property><Property name="AnimationDuration">1500</Property><Property name="ShowLegend">false</Property>{extra}</Control>"#
        )
    };
    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="main-form" width="1000" height="640" responsive="true">
  <Control id="LBL-OUT" type="Label" x="10" y="10" w="120" h="24"></Control>
  <Control id="BTN-PLAY" type="Button" x="10" y="44" w="90" h="28"><Property name="Caption">Play</Property></Control>
  <Control id="BTN-SHOW2" type="Button" x="110" y="44" w="90" h="28"><Property name="Caption">Show 2</Property></Control>
  <Control id="BTN-CH" type="Button" x="210" y="44" w="90" h="28"><Property name="Caption">Charts</Property></Control>
  <Control id="BTN-FADE" type="Button" x="310" y="44" w="90" h="28"><Property name="Caption">Fade</Property></Control>
  <Control id="TXT-A" type="TextBox" x="420" y="44" w="120" h="28">{focus}</Control>
  <Control id="TXT-B" type="TextBox" x="560" y="44" w="120" h="28">{focus}</Control>
  <Control id="ROW" type="Panel" x="10" y="90" w="960" h="220"><Property name="LayoutMode">Flex</Property><Property name="FlexDirection">Row</Property><Property name="FlexWrap">NoWrap</Property><Property name="BackgroundColor">#EEEEEE</Property></Control>
  <Control id="C1" type="Panel" x="0" y="0" w="300" h="200" parent="ROW"><Property name="FlexGrow">1</Property><Property name="FlexBasis">100</Property><Property name="BackgroundColor">#3E7BE8</Property>{INTRO}</Control>
  <Control id="LBL-C1" type="Label" x="20" y="30" w="100" h="40" parent="C1"><Property name="Caption">in</Property><Property name="BackgroundColor">#F5A524</Property>{INTRO}</Control>
  <Control id="C2" type="Panel" x="0" y="0" w="300" h="200" parent="ROW" visible="false"><Property name="FlexGrow">1</Property><Property name="FlexBasis">100</Property><Property name="BackgroundColor">#30A46C</Property></Control>
  {area}
  {scatter}
  {mono}
  <Control id="PNL-F" type="Panel" x="720" y="330" w="240" h="160"><Property name="BackgroundColor">#3E7BE8</Property>{fade_out}</Control>
  <Control id="LBL-FI" type="Label" x="20" y="20" w="100" h="40" parent="PNL-F"><Property name="Caption">fades in</Property><Property name="BackgroundColor">#F5A524</Property>{fade_in}</Control>
  <Control id="LBL-STILL" type="Label" x="20" y="90" w="100" h="40" parent="PNL-F"><Property name="Caption">still</Property><Property name="BackgroundColor">#E5484D</Property></Control>
</Form>
"##,
        focus = anim("f", "OnFocus", "Shake", 600),
        area = chart("CH-AREA", "AreaChart", 10, ""),
        scatter = chart("CH-SC", "ScatterChart", 240, ""),
        mono = chart("CH-MONO", "BarChart", 470, r#"<Property name="Monochrome">true</Property>"#),
        fade_out = anim("out", "Programmatic", "FadeOut", 600),
        fade_in = anim("in", "Programmatic", "FadeIn", 600),
    )
}

const LIB: &str = r#"
  const root = document.getElementById('pr-app').shadowRoot;
  const el = id => root.querySelector('[data-name="' + id + '"]');
  const text = id => { const e = el(id); return e ? e.textContent : null; };
  const form = () => root.querySelector('[data-pr-inst]');
  const anims = () => form().getAnimations({ subtree: true });
  // The laid-out centre (offsets ignore transforms; scrolls subtracted) and the drawn one, both relative to the form.
  const laid = e => { const f = form(); let x = 0, y = 0, n = e; while (n && n !== f) { x += n.offsetLeft; y += n.offsetTop; const p = n.offsetParent; let a = n.parentElement; while (a && a !== p && a !== f) { x -= a.scrollLeft; y -= a.scrollTop; a = a.parentElement; } if (p && p !== f) { x += p.clientLeft - p.scrollLeft; y += p.clientTop - p.scrollTop; } n = p; } return [x + e.offsetWidth / 2, y + e.offsetHeight / 2]; };
  const drift = id => { const e = el(id), f = form(), fr = f.getBoundingClientRect(), r = e.getBoundingClientRect(); const [lx, ly] = laid(e); return Math.hypot(r.x + r.width / 2 - fr.x - f.clientLeft - lx, r.y + r.height / 2 - fr.y - f.clientTop - ly); };
  // What the eye gets: the element's opacity times every ancestor's, up to the form.
  const seen = id => { let o = 1, n = el(id); while (n && n !== form()) { o *= Number(getComputedStyle(n).opacity); n = n.parentElement; } return o; };
  const bgAlpha = id => { const m = getComputedStyle(el(id)).backgroundColor.match(/[\d.]+/g).map(Number); return m.length > 3 ? m[3] : 1; };
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

#[test]
fn the_page_moves_as_the_desktop_moves_where_judgement_p4_1_found_it_did_not() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let (multi, single) = (build(&spec(true)), build(&spec(false)));
    let player_out = support::built_player();
    let input = BundleInput {
        project: "motion4b".into(),
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

    // ── a card shrinks mid-intro (a second card shown into its flex row): the label inside stays on its place ──────
    click(&mut page, "BTN-PLAY");
    wait_text(&mut page, "LBL-OUT", "played");
    page.wait_for(&js("return el('LBL-C1').getAnimations().length > 0;"), 5).unwrap_or_else(|e| panic!("no intro: {e}"));
    let w_before = page.eval(&js("return el('C1').offsetWidth;")).unwrap().as_f64().unwrap();
    // What a frame PAINTS is measured after the page's own size watch has run (a ResizeObserver made later is called later in the
    // same rendering step, after layout and before paint); a requestAnimationFrame callback runs BEFORE that frame's layout, so the
    // first one after the re-flow still sees the plan made for the old layout.
    page.eval(&js("window.__ro = []; new ResizeObserver(() => window.__ro.push(Math.max(drift('LBL-C1'), drift('C1')))).observe(el('C1')); return true;")).unwrap();
    click(&mut page, "BTN-SHOW2");
    wait_text(&mut page, "LBL-OUT", "shown2");
    let worst = page
        .eval(&js(
            "const t0 = performance.now(); let worst = 0, first = null, frames = 0, moving = 0; while (performance.now() - t0 < 1400) { await new Promise(r => requestAnimationFrame(r)); frames++; if (el('LBL-C1').getAnimations().length) moving++; const d = Math.max(drift('LBL-C1'), drift('C1')); if (first === null) first = d; else worst = Math.max(worst, d); } return { worst, first, frames, moving, w: el('C1').offsetWidth, painted: window.__ro };",
        ))
        .unwrap();
    let w_after = worst["w"].as_f64().unwrap();
    assert!(w_after < w_before - 50.0, "the second card took room from the first: {w_before} -> {w_after}");
    assert!(worst["moving"].as_u64().unwrap() > 20, "still in its intro after the re-flow: {worst}");
    let painted: Vec<f64> = worst["painted"].as_array().unwrap().iter().filter_map(|v| v.as_f64()).collect();
    assert!(painted.len() >= 2, "the size watch saw the re-flow: {worst}");
    assert!(painted.iter().all(|d| *d < 1.0), "the frame of the re-flow is painted with the card and its label about their laid-out centres: {worst}");
    assert!(worst["worst"].as_f64().unwrap() < 1.0, "and so is every frame after it: {worst}");
    println!(
        "re-flow mid-intro: card {w_before} -> {w_after} px wide; in the frame of the re-flow (after the page's size watch, before paint) {:.2} px from the laid-out centres; the {} frames after it, worst {:.2} px (the first requestAnimationFrame sample, taken before that frame's layout, read {:.1} px)",
        painted.iter().cloned().fold(0.0, f64::max),
        worst["frames"].as_u64().unwrap() - 1,
        worst["worst"].as_f64().unwrap(),
        worst["first"].as_f64().unwrap()
    );
    page.wait_for(&js("return anims().length === 0;"), 5).unwrap_or_else(|e| panic!("the intro did not end: {e}"));

    // ── Area, Scatter and a Monochrome bar chart grow when they are shown ───────────────────────────────────────
    click(&mut page, "BTN-CH");
    let grown = page
        .eval(&js(
            "const ext = (id, sel) => { const e = el(id); if (!e || !e.checkVisibility()) return null; let lo = Infinity, hi = -Infinity; for (const m of e.querySelectorAll(sel)) { const b = m.getBBox(); lo = Math.min(lo, b.y); hi = Math.max(hi, b.y + b.height); } return hi > lo ? hi - lo : 0; };\
             const sc = () => { const e = el('CH-SC'); if (!e || !e.checkVisibility()) return null; const ys = [...e.querySelectorAll('circle')].map(c => Number(c.getAttribute('cy'))); return ys.length ? Math.max(...ys) - Math.min(...ys) : 0; };\
             const t0 = performance.now(); const out = { area: [], sc: [], mono: [] }; while (performance.now() - t0 < 2200) { await new Promise(r => requestAnimationFrame(r)); out.area.push(ext('CH-AREA', 'path.pr-line')); out.sc.push(sc()); out.mono.push(ext('CH-MONO', 'rect.pr-bar')); } return out;",
        ))
        .unwrap();
    for k in ["area", "sc", "mono"] {
        let v: Vec<f64> = grown[k].as_array().unwrap().iter().filter_map(|x| x.as_f64()).collect();
        let last = *v.last().unwrap();
        let distinct = v.windows(2).filter(|w| (w[0] - w[1]).abs() > 0.05).count();
        assert!(last > 10.0, "{k}: drawn at rest ({last})");
        assert!(v.iter().any(|x| *x < 0.5 * last), "{k}: starts well below its size at rest: {v:?}");
        assert!(distinct >= 15, "{k}: grows over many frames ({distinct} changes): {v:?}");
        println!("AnimateOnLoad {k}: {} frames, {distinct} of them a new picture, from {:.1} to {last:.1}", v.len(), v.iter().cloned().fold(f64::INFINITY, f64::min));
    }

    // ── a FadeOut panel with a FadeIn label and a still label inside ────────────────────────────────────────────
    let bg0 = page.eval(&js("return bgAlpha('PNL-F');")).unwrap().as_f64().unwrap();
    click(&mut page, "BTN-FADE");
    wait_text(&mut page, "LBL-OUT", "faded");
    page.wait_for(&js("return el('LBL-FI').getAnimations().length > 0;"), 5).unwrap_or_else(|e| panic!("no fade: {e}"));
    let mid = page
        .eval(&js("for (const a of anims()) { a.pause(); a.currentTime = 300; } await new Promise(r => requestAnimationFrame(r)); return { fi: seen('LBL-FI'), still: seen('LBL-STILL'), panel: seen('PNL-F'), bg: bgAlpha('PNL-F') };"))
        .unwrap();
    assert!((mid["fi"].as_f64().unwrap() - 0.5).abs() < 0.03, "half-way the FadeIn label is at its own alpha 0.5: {mid}");
    assert!((mid["still"].as_f64().unwrap() - 1.0).abs() < 1e-6, "the still label does not fade with its panel: {mid}");
    assert!((mid["panel"].as_f64().unwrap() - 1.0).abs() < 1e-6, "the panel's element is not faded (its face is): {mid}");
    assert!((mid["bg"].as_f64().unwrap() - 0.5 * bg0).abs() < 0.03, "the panel's face at half its alpha: {mid} (at rest {bg0})");
    page.eval(&js("for (const a of anims()) a.play(); return true;")).unwrap();
    let end = page
        .wait_for(&js("if (el('LBL-FI').getAnimations().some(a => a.playState === 'running')) return false; return { fi: seen('LBL-FI'), still: seen('LBL-STILL'), bg: bgAlpha('PNL-F') };"), 5)
        .unwrap_or_else(|e| panic!("the fade did not end: {e}"));
    assert!((end["fi"].as_f64().unwrap() - 1.0).abs() < 1e-6 && (end["still"].as_f64().unwrap() - 1.0).abs() < 1e-6, "both labels fully visible: {end}");
    assert!(end["bg"].as_f64().unwrap() < 0.01, "the panel rests faded out (FadeOut rests invisible): {end}");
    println!("FadeOut panel, FadeIn label: half-way label {:.3} / still {:.3} / face {:.3} of {bg0}; at the end label {:.3}, still {:.3}, face {:.3}", mid["fi"].as_f64().unwrap(), mid["still"].as_f64().unwrap(), mid["bg"].as_f64().unwrap(), end["fi"].as_f64().unwrap(), end["still"].as_f64().unwrap(), end["bg"].as_f64().unwrap());

    // ── OnFocus: only the field with a bound onGotFocus plays it ────────────────────────────────────────────────
    click(&mut page, "TXT-A");
    let a = page.eval(&js("await new Promise(r => setTimeout(r, 150)); return el('TXT-A').getAnimations().length;")).unwrap();
    assert_eq!(a, 0, "TXT-A has no onGotFocus handler: no OnFocus animation (the desktop raises no onGotFocus for it)");
    click(&mut page, "TXT-B");
    wait_text(&mut page, "LBL-OUT", "focus-b");
    let b = page.eval(&js("return el('TXT-B').getAnimations().length;")).unwrap();
    assert!(b.as_u64().unwrap() > 0, "TXT-B has onGotFocus bound: its OnFocus Shake plays");
    println!("OnFocus: unbound field 0 animations, bound field {b}");
    assert!(page.exceptions.is_empty(), "uncaught exceptions: {:?}", page.exceptions);
    drop(server);
}
