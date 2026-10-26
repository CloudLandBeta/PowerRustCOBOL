// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The expand icon of a card, in a real browser** (spec 090; the operator, 2026-10-25: "the maximize button of
//! PowerAnalyticsWeb behaves differently from the desktop's").
//!
//! The desktop window has no title bar; what looks like a maximize button is the expand icon of the Panels and GroupBoxes that
//! are `Expandable`. A click on it makes THAT card take the whole client area of its container, the cards beside it give up their
//! room and are not drawn, and what is inside the card grows with it by one factor. A second click gives everything back. Three
//! things were wrong on the page, and each of them is a thing a unit test cannot see:
//!
//! * the icon is inside the card, the card is inside a grid, the grid inside a form's root: a click bubbles through every one of
//!   them, and every Panel that heard it turned its own `Expanded` over, so one click expanded the whole chain (the header and
//!   the filter bar went too);
//! * the card was placed `absolute` but kept the grid cell it was designed in, and an absolutely positioned item of a grid is
//!   placed against that cell, so it never left it;
//! * what was inside it did not grow.
//!
//! The fixture program's main form is given a header and a grid of three expandable cards (one spanning the second row, one with
//! a Panel of its own inside) and a card in a layer, and a headless Chrome presses the icons with real mouse events and asks the
//! browser where everything is. The numbers it is compared with are the desktop's own rule, written down in the test: the card is
//! its container's whole client area (the container less the 2 px a Panel takes off and its padding) and its contents are placed
//! at `min(client width ratio, client height ratio)` times the designed rectangles, centred in the room that is left over.
//!
//! With no Chrome on the machine (`CHROME` names one) the tests say they were skipped, and why.

mod support;

use std::time::Instant;

use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM};
use cobolt_web_bundle::devserver::{serve, ServeOptions, Server};
use cobolt_web_bundle::{assemble_bundle, BundleOptions, Isolation, PlayerInput};
use serde_json::{json, Value};
use support::cdp::{find_chrome, Chrome, Page};

/// The padding of the grid and the 2 px the desktop takes off a Panel on every side: the card's inset in its container.
const INSET: f64 = 6.0 + 2.0;

/// The fixture's main form made responsive and 900 x 600, with a root that follows the window and holds a header and a grid of
/// three expandable cards. The fixture's own controls stay where they are (x = 10): the root starts at x = 230.
fn dashboard_form(plain: &str) -> String {
    assert!(plain.contains("width=\"640\" height=\"480\""), "the fixture's form is 640 x 480");
    let sized = plain.replace("width=\"640\" height=\"480\"", "width=\"900\" height=\"600\" responsive=\"true\"");
    let layer = "  <Layer name=\"POP\" color=\"#00000000\" />\n";
    let card = |id: &str, x: i32, y: i32, w: i32, h: i32, col: i32, row: i32, span: i32| {
        format!(
            "  <Control id=\"{id}\" type=\"GroupBox\" parent=\"GRID\" x=\"{x}\" y=\"{y}\" w=\"{w}\" h=\"{h}\">\
             <Property name=\"GridColumn\">{col}</Property><Property name=\"GridRow\">{row}</Property><Property name=\"ColumnSpan\">{span}</Property>\
             <Property name=\"Expandable\">1</Property></Control>\n\
             <Control id=\"L-{id}\" type=\"Label\" parent=\"{id}\" x=\"{lx}\" y=\"{ly}\" w=\"120\" h=\"20\"><Property name=\"Caption\">{id}</Property><Property name=\"FontSize\">14</Property></Control>\n\
             <Control id=\"B-{id}\" type=\"Button\" parent=\"{id}\" x=\"{bx}\" y=\"{by}\" w=\"70\" h=\"24\"><Property name=\"Caption\">Go</Property><Property name=\"FontSize\">12</Property><Property name=\"Anchor\">Bottom,Right</Property></Control>\n",
            lx = x + 10,
            ly = y + 10,
            bx = x + w - 80,
            by = y + h - 34,
        )
    };
    let mut controls = String::new();
    controls.push_str("  <Control id=\"ROOT\" type=\"Panel\" x=\"230\" y=\"0\" w=\"660\" h=\"600\"><Property name=\"Anchor\">Top,Left,Right,Bottom</Property></Control>\n");
    controls.push_str("  <Control id=\"HEAD\" type=\"Panel\" parent=\"ROOT\" x=\"240\" y=\"10\" w=\"640\" h=\"60\"><Property name=\"Anchor\">Top,Left,Right</Property></Control>\n");
    controls.push_str(
        "  <Control id=\"GRID\" type=\"Panel\" parent=\"ROOT\" x=\"240\" y=\"80\" w=\"640\" h=\"500\"><Property name=\"Anchor\">Top,Left,Right,Bottom</Property>\
         <Property name=\"LayoutMode\">Grid</Property><Property name=\"GridColumns\">1fr 1fr</Property><Property name=\"GridRows\">1fr 1fr</Property>\
         <Property name=\"Gap\">20</Property><Property name=\"Padding\">6</Property></Control>\n",
    );
    controls.push_str(&card("C-ONE", 246, 86, 300, 230, 1, 1, 1));
    controls.push_str(&card("C-TWO", 566, 86, 300, 230, 2, 1, 1));
    controls.push_str(&card("C-THREE", 246, 336, 620, 230, 1, 2, 2));
    // A panel of its own inside the first card: its label grows with the card, placed against the panel, not the card.
    controls.push_str("  <Control id=\"P-IN\" type=\"Panel\" parent=\"C-ONE\" x=\"256\" y=\"140\" w=\"200\" h=\"80\"></Control>\n");
    controls.push_str("  <Control id=\"L-IN\" type=\"Label\" parent=\"P-IN\" x=\"266\" y=\"150\" w=\"100\" h=\"20\"><Property name=\"Caption\">in</Property><Property name=\"FontSize\">10</Property></Control>\n");
    // A card on the form itself: its container is the form, whose whole client area it takes (the form's, for a top-level card).
    controls.push_str("  <Control id=\"C-TOP\" type=\"GroupBox\" x=\"10\" y=\"440\" w=\"200\" h=\"120\"><Property name=\"Expandable\">1</Property></Control>\n");
    controls.push_str("  <Control id=\"L-TOP\" type=\"Label\" parent=\"C-TOP\" x=\"20\" y=\"450\" w=\"100\" h=\"20\"><Property name=\"Caption\">top</Property><Property name=\"FontSize\">12</Property></Control>\n");
    // A card in a layer is placed by the program: it is Expandable on paper and has no icon, as on the desktop.
    controls.push_str("  <Control id=\"C-LAYER\" type=\"GroupBox\" x=\"0\" y=\"300\" w=\"200\" h=\"120\" layer=\"POP\"><Property name=\"Expandable\">1</Property></Control>\n");
    // The layer is declared before the controls, as the designer writes it.
    let at = sized.find("<Control").expect("the fixture's form has controls");
    let mut out = String::new();
    out.push_str(&sized[..at]);
    out.push_str(layer);
    out.push_str(&sized[at..]);
    out.replace("</Form>", &format!("{controls}</Form>"))
}

/// The bundle of the fixture program with the dashboard form, served; and a page of `chrome` open on it once the program's
/// `onLoad` has written its label. Answers the page, the server and how long the start took.
fn open_dashboard(chrome: &Chrome, width: u32, height: u32) -> (Page, Server, u128) {
    let player_out = support::built_player();
    let mut input = support::input_with(
        PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    );
    for f in input.forms.iter_mut().filter(|f| f.name == MAIN_FORM) {
        f.cfrm = dashboard_form(&f.cfrm);
    }
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(width, height).unwrap();
    let t = Instant::now();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for(
        &format!("(() => {{ const r = document.getElementById('pr-app').shadowRoot; const l = r && r.querySelector('[data-pr-form=\"{MAIN_FORM}\"] [data-name=\"{LBL_OUT}\"]'); return !!l && l.textContent === 'loaded'; }})()"),
        30,
    )
    .unwrap_or_else(|e| panic!("the application did not start: {e}"));
    (page, server, t.elapsed().as_millis())
}

/// What every snippet starts with: the shadow root, a control's element, its box and whether a user can see it.
const LIB: &str = r#"
  const root = document.getElementById('pr-app').shadowRoot;
  const el = id => root.querySelector('[data-name="' + id + '"]');
  const box = id => { const e = el(id); if (!e) return null; const r = e.getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; };
  const shown = id => { const e = el(id); return !!e && e.checkVisibility({ checkVisibilityCSS: true }) && e.getBoundingClientRect().width > 0; };
  const icon = id => { const e = el(id); return e && e.querySelector(':scope > .pr-expander'); };
"#;

fn js(body: &str) -> String {
    format!("(() => {{ {LIB} {body} }})()")
}

/// The rectangle of the icon of `id`, for a click at its centre. The page waits until the icon is what a click there reaches.
fn icon_rect(id: &str) -> String {
    js(&format!(
        "const b = icon('{id}'); if (!b) return null; const r = b.getBoundingClientRect(); const top = root.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2); \
         return (top === b || b.contains(top)) ? {{ x: r.x, y: r.y, width: r.width, height: r.height }} : null;"
    ))
}

/// Every box the test looks at, whether it is shown, and the CSS that places the containers above the cards.
fn look() -> String {
    js(r#"
      const ids = ['ROOT', 'HEAD', 'GRID', 'C-ONE', 'C-TWO', 'C-THREE', 'L-C-ONE', 'B-C-ONE', 'P-IN', 'L-IN', 'L-C-TWO', 'B-C-TWO', 'L-C-THREE', 'B-C-THREE', 'BTN-GO', 'C-LAYER', 'C-TOP', 'L-TOP'];
      const out = {};
      for (const id of ids) {
        const e = el(id);
        if (!e) continue;
        const cs = getComputedStyle(e);
        const ic = icon(id);
        out[id] = { box: box(id), shown: shown(id), z: cs.zIndex, position: cs.position, display: cs.display, font: parseFloat(cs.fontSize), icon: ic ? ic.getAttribute('aria-expanded') : null, label: ic ? ic.getAttribute('aria-label') : null };
      }
      return out;
    "#)
}

fn near(a: &Value, b: &[f64], tol: f64, what: &str) {
    let a: Vec<f64> = a.as_array().unwrap_or_else(|| panic!("{what}: no box ({a})")).iter().map(|v| v.as_f64().unwrap()).collect();
    for k in 0..4 {
        assert!((a[k] - b[k]).abs() <= tol, "{what}: page {a:?}, expected {b:?} (within {tol})");
    }
}

fn boxf(v: &Value) -> Vec<f64> {
    v["box"].as_array().unwrap_or_else(|| panic!("no box in {v}")).iter().map(|x| x.as_f64().unwrap()).collect()
}

/// The page waits until `id` has the box `want` (an expansion is a layout and a measurement and a layout again).
fn settle_at(page: &mut Page, id: &str, want: &[f64]) -> Value {
    let cond = js(&format!(
        "const b = box('{id}'); if (!b || !shown('{id}')) return false; const w = {want:?}; return b.every((v, k) => Math.abs(v - w[k]) < 0.6);"
    ));
    let _ = page.wait_for(&cond, 10);
    page.eval(&look()).unwrap()
}

#[test]
fn one_press_expands_only_that_card_into_its_containers_whole_client_area_and_a_second_press_gives_everything_back() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, server, started_ms) = open_dashboard(&chrome, 1100, 700);
    page.wait_for(&js("return shown('C-ONE') && !!icon('C-ONE');"), 10).unwrap();
    let before = page.eval(&look()).unwrap();

    // Each card has an icon; the card in a layer has none, and neither has anything that is not a card.
    for id in ["C-ONE", "C-TWO", "C-THREE"] {
        assert_eq!(before[id]["label"], json!("Expand"), "{id}: the icon offers to expand");
        assert_eq!(before[id]["icon"], json!("false"));
    }
    assert!(page.eval(&js("return !icon('C-LAYER');")).unwrap().as_bool().unwrap(), "a card in a layer shows no icon");
    for id in ["ROOT", "HEAD", "GRID"] {
        assert_eq!(before[id]["label"], Value::Null, "{id} is not a card with an icon");
    }
    let grid = boxf(&before["GRID"]);
    let cells = ["C-ONE", "C-TWO", "C-THREE"].map(|id| boxf(&before[id]));
    assert!(cells[0][2] < grid[2] / 2.0 && cells[1][0] > cells[0][0] + cells[0][2], "two cards side by side in the first row: {cells:?}");

    // ── one real press on the icon of the first card ──────────────────────────────────────────────────────────────────
    page.click_at(&icon_rect("C-ONE")).unwrap_or_else(|e| panic!("the icon of C-ONE is not what a click at its centre reaches: {e}"));
    // The container's whole client area: its box less the 2 px a Panel gives up and its padding of 6.
    let area = [grid[0] + INSET, grid[1] + INSET, grid[2] - 2.0 * INSET, grid[3] - 2.0 * INSET];
    let placed = settle_at(&mut page, "C-ONE", &area);
    near(&placed["C-ONE"]["box"], &area, 0.6, "C-ONE fills the grid's client area");

    // What is inside the card grows with it by ONE factor: the smaller of the two ratios of the card's client area, measured from the
    // size it was designed at (300 x 230, less 2 px on every side), the block centred in the room that is left over.
    let card = boxf(&placed["C-ONE"]);
    let (dw, dh) = (300.0 - 4.0, 230.0 - 4.0);
    let (lw, lh) = (card[2] - 4.0, card[3] - 4.0);
    let z = (lw / dw).min(lh / dh);
    let (ox, oy) = ((lw - dw * z) / 2.0, (lh - dh * z) / 2.0);
    let want = |designed: [f64; 4]| [card[0] + 2.0 + ox + (designed[0] - 248.0) * z, card[1] + 2.0 + oy + (designed[1] - 88.0) * z, designed[2] * z, designed[3] * z];
    // The browser lays the card out, and then tells the page how big it came out: the contents follow in the next frame.
    let open = settle_at(&mut page, "L-C-ONE", &want([256.0, 96.0, 120.0, 20.0]));
    assert_eq!(open["C-ONE"]["icon"], json!("true"));
    assert_eq!(open["C-ONE"]["label"], json!("Collapse"), "the icon now offers to give the room back");
    // The cards beside it give their room up and are not drawn, nor what is in them.
    for id in ["C-TWO", "C-THREE", "L-C-TWO", "B-C-TWO", "L-C-THREE", "B-C-THREE"] {
        assert_eq!(open[id]["shown"], json!(false), "{id} is not drawn while C-ONE is expanded");
    }
    // Nothing above the card was expanded by the press that came through it: the root, the header and the grid are where they were,
    // placed as they were (the page used to give each of them the placement of an expanded card, and to hide the header).
    for id in ["ROOT", "HEAD", "GRID", "BTN-GO"] {
        assert_eq!(open[id]["shown"], json!(true), "{id} is still drawn");
        assert_eq!(open[id]["box"], before[id]["box"], "{id} did not move");
        assert_eq!((&open[id]["z"], &open[id]["position"], &open[id]["display"]), (&before[id]["z"], &before[id]["position"], &before[id]["display"]), "{id} is placed as before");
    }
    near(&open["L-C-ONE"]["box"], &want([256.0, 96.0, 120.0, 20.0]), 0.6, "the label grows and moves with the card");
    near(&open["B-C-ONE"]["box"], &want([466.0, 282.0, 70.0, 24.0]), 0.6, "the button, anchored to the bottom right, follows the same factor");
    near(&open["P-IN"]["box"], &want([256.0, 140.0, 200.0, 80.0]), 0.6, "the panel inside the card");
    assert!((open["L-C-ONE"]["font"].as_f64().unwrap() - 14.0 * z).abs() < 0.05, "the lettering grows by the same factor: {} for {}", open["L-C-ONE"]["font"], 14.0 * z);
    assert!((open["L-IN"]["font"].as_f64().unwrap() - 10.0 * z).abs() < 0.05, "and so does the lettering of what is in the panel");
    // The panel's own label is placed against the panel's client area, which grew by the same factor.
    let pin = boxf(&open["P-IN"]);
    let (pdw, pdh) = (200.0 - 4.0, 80.0 - 4.0);
    let (plw, plh) = (pin[2] - 4.0, pin[3] - 4.0);
    let (pox, poy) = ((plw - pdw * z) / 2.0, (plh - pdh * z) / 2.0);
    near(&open["L-IN"]["box"], &[pin[0] + 2.0 + pox + (266.0 - 258.0) * z, pin[1] + 2.0 + poy + (150.0 - 142.0) * z, 100.0 * z, 20.0 * z], 0.6, "the label of the panel inside the card");

    // ── a second press, on the icon the expanded card shows, gives everything back ─────────────────────────────────────
    page.click_at(&icon_rect("C-ONE")).unwrap();
    let back = settle_at(&mut page, "C-ONE", &cells[0]);
    for id in ["ROOT", "HEAD", "GRID", "C-ONE", "C-TWO", "C-THREE", "L-C-ONE", "B-C-ONE", "P-IN", "L-IN", "L-C-TWO", "L-C-THREE", "B-C-THREE"] {
        assert_eq!(back[id]["shown"], json!(true), "{id} is drawn again");
        near(&back[id]["box"], &boxf(&before[id]), 0.01, &format!("{id} is exactly where it was"));
    }
    assert_eq!(back["L-C-ONE"]["font"], before["L-C-ONE"]["font"], "and lettered as it was");
    assert_eq!(back["C-ONE"]["label"], json!("Expand"));

    // ── the icon of the second card: not reachable while the first is open; then the second opens on its own ────────────
    page.click_at(&icon_rect("C-ONE")).unwrap();
    settle_at(&mut page, "C-ONE", &area);
    assert!(page.eval(&js("return icon('C-TWO') && !icon('C-TWO').checkVisibility();")).unwrap().as_bool().unwrap(), "the second card's icon is not drawn while the first is expanded");
    page.click_at(&icon_rect("C-ONE")).unwrap();
    settle_at(&mut page, "C-ONE", &cells[0]);
    page.click_at(&icon_rect("C-TWO")).unwrap();
    let second = settle_at(&mut page, "C-TWO", &area);
    near(&second["C-TWO"]["box"], &area, 0.6, "the second card fills the grid's client area");
    assert_eq!(second["C-ONE"]["shown"], json!(false));
    assert_eq!((second["C-ONE"]["icon"].clone(), second["C-TWO"]["icon"].clone()), (json!("false"), json!("true")), "only the second is expanded");
    for id in ["ROOT", "HEAD", "GRID"] {
        assert_eq!(second[id]["box"], before[id]["box"], "{id} did not move");
    }

    // ── the window is resized while the card is expanded: the card follows its container ──────────────────────────────
    page.resize(900, 640).unwrap();
    let grid2 = page.eval(&js("return box('GRID');")).unwrap();
    let g2: Vec<f64> = grid2.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    assert!(g2[2] < grid[2], "the grid got narrower with the window");
    let area2 = [g2[0] + INSET, g2[1] + INSET, g2[2] - 2.0 * INSET, g2[3] - 2.0 * INSET];
    let narrower = settle_at(&mut page, "C-TWO", &area2);
    near(&narrower["C-TWO"]["box"], &area2, 0.6, "the expanded card follows the window");
    page.click_at(&icon_rect("C-TWO")).unwrap();
    page.wait_for(&js("return shown('C-ONE') && shown('C-THREE');"), 10).unwrap();

    assert!(page.exceptions.is_empty(), "no uncaught exception: {:?}", page.exceptions);
    let errors: Vec<&String> = page.console.iter().filter(|c| c.starts_with("error:") && !c.contains("favicon")).collect();
    assert!(errors.is_empty(), "no console error: {errors:?}");
    drop(page);
    server.stop();

    println!(
        "card expand in Chrome ({}): up in {started_ms} ms; one press on C-ONE: the card {:?} = the grid's client area {:?}; header, root and grid unmoved; \
         3 siblings and what is in them not drawn; contents at factor {z:.4} (label {:?}, lettering {:.2} pt); collapse: 13 controls back within 0.01 px; \
         second card opened alone after the first collapsed; resized to 900 wide while expanded: the card followed (grid {:?} -> {:?}); the card in a layer shows no icon",
        exe.display(),
        card.iter().map(|v| v.round()).collect::<Vec<_>>(),
        area.iter().map(|v| v.round()).collect::<Vec<_>>(),
        boxf(&open["L-C-ONE"]).iter().map(|v| (v * 10.0).round() / 10.0).collect::<Vec<_>>(),
        open["L-C-ONE"]["font"].as_f64().unwrap(),
        grid.iter().map(|v| v.round()).collect::<Vec<_>>(),
        g2.iter().map(|v| v.round()).collect::<Vec<_>>(),
    );
}

#[test]
fn the_icon_can_be_pressed_from_the_keyboard_and_a_card_that_spans_the_second_row_fills_the_grid_too() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    // The icon is a button: Enter on it presses it, and the click that makes bubbles through the same containers as a mouse click does.
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, server, _) = open_dashboard(&chrome, 1100, 700);
    page.wait_for(&js("return shown('C-THREE') && !!icon('C-THREE');"), 10).unwrap();
    let before = page.eval(&look()).unwrap();
    let grid = boxf(&before["GRID"]);
    // Focus the icon and press Enter: a click event with the keyboard as its source, which bubbles through the same containers.
    page.eval(&js("icon('C-THREE').focus(); return true;")).unwrap();
    page.call("Input.dispatchKeyEvent", json!({"type": "keyDown", "key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13, "nativeVirtualKeyCode": 13, "text": "\r", "unmodifiedText": "\r"})).unwrap();
    page.call("Input.dispatchKeyEvent", json!({"type": "keyUp", "key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13, "nativeVirtualKeyCode": 13})).unwrap();
    let area = [grid[0] + INSET, grid[1] + INSET, grid[2] - 2.0 * INSET, grid[3] - 2.0 * INSET];
    let open = settle_at(&mut page, "C-THREE", &area);
    near(&open["C-THREE"]["box"], &area, 0.6, "the keyboard presses the icon as a click does: the third card fills the grid's client area");
    for id in ["C-ONE", "C-TWO"] {
        assert_eq!(open[id]["shown"], json!(false), "{id} gave its room up");
    }
    for id in ["ROOT", "HEAD", "GRID"] {
        assert_eq!(open[id]["box"], before[id]["box"], "{id} did not move");
        assert_eq!(open[id]["z"], before[id]["z"], "{id}: not given the placement of an expanded card");
    }
    drop(page);
    server.stop();
    println!("card expand by keyboard in Chrome ({}): Enter on the icon of C-THREE (a card spanning the second row) expands it alone; root, header and grid unmoved", exe.display());
}

#[test]
fn a_card_placed_on_the_form_itself_takes_the_whole_form_and_everything_else_gives_way() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, server, _) = open_dashboard(&chrome, 1100, 700);
    page.wait_for(&js("return shown('C-TOP') && !!icon('C-TOP');"), 10).unwrap();
    let before = page.eval(&look()).unwrap();
    let surface = page.eval(&js("const r = root.querySelector('[data-pr-form] .pr-surface').getBoundingClientRect(); return [r.x, r.y, r.width, r.height];")).unwrap();
    let whole: Vec<f64> = surface.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    page.click_at(&icon_rect("C-TOP")).unwrap_or_else(|e| panic!("the icon of C-TOP is not what a click at its centre reaches: {e}"));
    let open = settle_at(&mut page, "C-TOP", &whole);
    near(&open["C-TOP"]["box"], &whole, 0.6, "the card on the form fills the form");
    for id in ["ROOT", "HEAD", "GRID", "C-ONE", "BTN-GO", "L-C-ONE"] {
        assert_eq!(open[id]["shown"], json!(false), "{id} gives way to a card that has the whole form");
    }
    // Its contents grow by the factor of the whole form's area against the 200 x 120 it was designed at.
    let z = ((whole[2] - 4.0) / 196.0).min((whole[3] - 4.0) / 116.0);
    let want_font = 12.0 * z;
    page.wait_for(&js(&format!("return Math.abs(parseFloat(getComputedStyle(el('L-TOP')).fontSize) - {want_font}) < 0.05;")), 10)
        .unwrap_or_else(|e| panic!("the label of the card did not grow by the factor {z}: {e}"));
    page.click_at(&icon_rect("C-TOP")).unwrap();
    let back = settle_at(&mut page, "C-TOP", &boxf(&before["C-TOP"]));
    for id in ["ROOT", "HEAD", "GRID", "C-ONE", "BTN-GO", "C-TOP", "L-TOP", "L-C-ONE"] {
        assert_eq!(back[id]["shown"], json!(true), "{id} is drawn again");
        near(&back[id]["box"], &boxf(&before[id]), 0.01, &format!("{id} is exactly where it was"));
    }
    drop(page);
    server.stop();
    println!("card expand on the form in Chrome ({}): C-TOP {:?} -> the whole form {:?} at factor {z:.4}; 6 other top-level controls and what they hold not drawn; collapsed: 8 controls back within 0.01 px", exe.display(), boxf(&before["C-TOP"]), whole);
}
