// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A ring is drawn in the middle of its control, in a real browser** (2.0.413): the donut and pie charts, the gauge's ring
//! and the knob's dial.
//!
//! A form that puts a label in the hole of a donut (the PowerAnalytics dashboards do: "43.701 / fundos") centres the label on
//! the chart's own box, so the ring has to be drawn in the middle of that box. The plot frame the charts share kept the room of a
//! Y axis on the left (10 % of the width) and a smaller margin on the right (4 %), which suits a bar chart and put every pie
//! and donut 3 % of its width to the right of its box's middle: the label sat left of the hole on the web.
//!
//! The test gives the fixture's main form one control of each kind, draws it in a headless Chrome and asks the browser where
//! the ring is (the bounding box of what was drawn) and where its box is. With no title, caption or legend beside it, the two
//! middles must be the same point. The gauge's ring and the knob's dial are measured the same way, and so is the text each
//! keeps in its middle.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM};
use cobolt_web_bundle::devserver::{serve, ServeOptions, Server};
use cobolt_web_bundle::{assemble_bundle, BundleOptions, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome, Page};

/// One control of every kind under test, laid out apart from each other and from the fixture's own controls.
const CONTROLS: &[(&str, &str, i32, i32, i32, i32, &str)] = &[
    ("C-DONUT", "DonutChart", 300, 20, 302, 204, "<Property name=\"InnerRadius\">66</Property><Property name=\"ShowLegend\">false</Property>"),
    ("C-DONUT-TALL", "DonutChart", 640, 20, 120, 300, "<Property name=\"InnerRadius\">66</Property><Property name=\"ShowLegend\">false</Property>"),
    ("C-PIE", "PieChart", 800, 20, 200, 200, "<Property name=\"ShowLegend\">false</Property>"),
    ("C-RING", "Gauge", 300, 260, 140, 140, "<Property name=\"GaugeStyle\">Donut</Property><Property name=\"Value\">40</Property><Property name=\"Minimum\">0</Property><Property name=\"Maximum\">100</Property>"),
    ("C-KNOB", "Knob", 480, 260, 120, 150, "<Property name=\"Value\">40</Property><Property name=\"Minimum\">0</Property><Property name=\"Maximum\">100</Property>"),
];

fn with_controls(plain: &str) -> String {
    let mut xml = String::new();
    for (id, ty, x, y, w, h, props) in CONTROLS {
        xml.push_str(&format!("  <Control id=\"{id}\" type=\"{ty}\" x=\"{x}\" y=\"{y}\" w=\"{w}\" h=\"{h}\">{props}</Control>\n"));
    }
    plain.replace("</Form>", &format!("{xml}</Form>"))
}

fn open_page(chrome: &Chrome) -> (Page, Server) {
    let player_out = support::built_player();
    let mut input = support::input_with(
        PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    );
    for f in input.forms.iter_mut().filter(|f| f.name == MAIN_FORM) {
        f.cfrm = with_controls(&f.cfrm);
    }
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1100, 700).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for(
        &format!("(() => {{ const r = document.getElementById('pr-app').shadowRoot; const l = r && r.querySelector('[data-pr-form=\"{MAIN_FORM}\"] [data-name=\"{LBL_OUT}\"]'); return !!l && l.textContent === 'loaded'; }})()"),
        30,
    )
    .unwrap_or_else(|e| panic!("the application did not start: {e}"));
    (page, server)
}

/// What the browser says: for each control, its box and the box of what was drawn in it. `rect(sel)` is the union of the
/// bounding boxes of every element `sel` matches inside the control.
const MEASURE: &str = r#"
(() => {
  const root = document.getElementById('pr-app').shadowRoot;
  const rect = (el) => { const r = el.getBoundingClientRect(); return { l: r.left, t: r.top, r: r.right, b: r.bottom }; };
  const union = (els) => els.map(rect).reduce((a, r) => a ? { l: Math.min(a.l, r.l), t: Math.min(a.t, r.t), r: Math.max(a.r, r.r), b: Math.max(a.b, r.b) } : r, null);
  const mid = (r) => r && { x: (r.l + r.r) / 2, y: (r.t + r.b) / 2, w: r.r - r.l, h: r.b - r.t };
  const box = (id) => root.querySelector('[data-name="' + id + '"]');
  const out = {};
  for (const id of ['C-DONUT', 'C-DONUT-TALL', 'C-PIE']) {
    const c = box(id);
    out[id] = { box: mid(rect(c)), drawn: mid(union([...c.querySelectorAll('polygon')])) };
  }
  { // The gauge's ring: its polylines (the track is the whole circle) and the reading in its middle.
    const c = box('C-RING');
    out['C-RING'] = { box: mid(rect(c)), drawn: mid(union([...c.querySelectorAll('polyline')])), text: mid(union([...c.querySelectorAll('text')])) };
  }
  { // The knob's dial: the first circle is the rim; the value hangs under it, on the dial's own vertical.
    const c = box('C-KNOB');
    out['C-KNOB'] = { box: mid(rect(c)), drawn: mid(union([c.querySelector('circle')])), text: mid(union([...c.querySelectorAll('text')])) };
  }
  return out;
})()
"#;

#[test]
fn a_donut_a_pie_a_gauge_ring_and_a_knob_are_drawn_in_the_middle_of_their_box_in_chrome() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, _server) = open_page(&chrome);
    // Slices and arcs are drawn by the first frame; the animation (none is asked for) cannot move them.
    let m = page.eval(MEASURE).unwrap();
    let num = |id: &str, part: &str, k: &str| m[id][part][k].as_f64().unwrap_or_else(|| panic!("{id}.{part}.{k} is missing: {m}"));

    let mut report = Vec::new();
    for id in ["C-DONUT", "C-DONUT-TALL", "C-PIE", "C-RING"] {
        let (dx, dy) = (num(id, "drawn", "x") - num(id, "box", "x"), num(id, "drawn", "y") - num(id, "box", "y"));
        report.push(format!("{id}: ring {:.1}x{:.1} in a box {:.0}x{:.0}, middle off by ({dx:+.2}, {dy:+.2}) px", num(id, "drawn", "w"), num(id, "drawn", "h"), num(id, "box", "w"), num(id, "box", "h")));
        assert!(dx.abs() < 1.0 && dy.abs() < 1.0, "{id}: the ring's middle is ({dx:+.2}, {dy:+.2}) px from its box's middle\n{m}");
    }
    // The text in a gauge ring's hole is centred on the ring; the knob's value is on the dial's own vertical.
    let (tx, ty) = (num("C-RING", "text", "x") - num("C-RING", "drawn", "x"), num("C-RING", "text", "y") - num("C-RING", "drawn", "y"));
    report.push(format!("C-RING: reading off the ring's middle by ({tx:+.2}, {ty:+.2}) px"));
    assert!(tx.abs() < 1.0, "the gauge's reading is off the ring's middle horizontally by {tx:+.2} px\n{m}");
    assert!(ty.abs() < 3.0, "the gauge's reading is off the ring's middle vertically by {ty:+.2} px\n{m}");
    let kx = num("C-KNOB", "text", "x") - num("C-KNOB", "drawn", "x");
    let kd = num("C-KNOB", "drawn", "x") - num("C-KNOB", "box", "x");
    report.push(format!("C-KNOB: dial off its box's middle by {kd:+.2} px, value off the dial's vertical by {kx:+.2} px"));
    assert!(kd.abs() < 1.0 && kx.abs() < 1.0, "the knob's dial or its value is off the middle ({kd:+.2}, {kx:+.2}) px\n{m}");
    println!("chart centres in Chrome:\n  {}", report.join("\n  "));
}
