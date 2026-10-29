// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A TabControl with more tabs than room, in a real browser** (2.0.89, reworked in 2.0.90).
//!
//! The desktop's strip slides when its tabs overrun the control (a swipe, the wheel or a drag, no buttons; the keyboard walks
//! every tab; the selection is kept in view). On the page the strip used to be `overflow: hidden`: the tabs past the edge were
//! clipped and could not be reached. Four things have to hold on the page, and each is a thing a unit test cannot see because
//! the browser decides it:
//!
//! * a strip with more tabs than room slides (sideways on a top or bottom strip, down on a left or right one) with no scroll
//!   bar, its tabs keep their size, and a strip whose tabs all fit has nothing to slide;
//! * the mouse, which the browser does not slide a sideways box with, slides it: a drag along the strip (and the click that
//!   ends the drag presses no tab) and the vertical wheel (a notch glides, a small step follows at once);
//! * the tab that becomes the selected one is brought into view inside the strip, whichever way it got selected and however far
//!   away it is, with a sliver of the next tab beside it, and the page itself does not move;
//! * a redraw that leaves the selection where it was does not take the strip back to the selected tab: an operator who slid
//!   the strip keeps the place.
//!
//! The fixture program's main form is given three strips: 40 tabs on a top strip, 40 on a left strip and 3 that fit. A headless
//! Chrome presses the tabs, drags and turns the wheel with real mouse events and keys, and asks the browser where everything is.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM};
use cobolt_web_bundle::devserver::{serve, ServeOptions, Server};
use cobolt_web_bundle::{assemble_bundle, BundleOptions, Isolation, PlayerInput};
use serde_json::json;
use support::cdp::{find_chrome, Chrome, Page};

const TABS: usize = 40;

/// The fixture's main form made 900 x 600 with three strips beside its own controls (which sit left of x = 230): 40 tabs on a
/// top strip 400 wide, 40 on a left strip 220 high, and 3 that fit.
fn strips_form(plain: &str) -> String {
    assert!(plain.contains("width=\"640\" height=\"480\""), "the fixture's form is 640 x 480");
    // `index-base="1"`: a form without it is read as one saved when indexes counted from 0, and its `SelectedTab` 1 becomes 2.
    let sized = plain.replace("width=\"640\" height=\"480\"", "width=\"900\" height=\"600\" index-base=\"1\"");
    let names = |n: usize| (1..=n).map(|k| format!("Sheet {k:02}")).collect::<Vec<_>>().join("\n");
    let strip = |id: &str, position: &str, y: i32, h: i32, n: usize| {
        format!(
            "  <Control id=\"{id}\" type=\"TabControl\" x=\"230\" y=\"{y}\" w=\"400\" h=\"{h}\"><Property name=\"Tabs\">{}</Property>\
             <Property name=\"TabPosition\">{position}</Property><Property name=\"SelectedTab\">1</Property></Control>\n",
            names(n)
        )
    };
    let controls = format!("{}{}{}", strip("TABS-TOP", "Top", 10, 150, TABS), strip("TABS-LEFT", "Left", 190, 220, TABS), strip("TABS-FIT", "Top", 440, 100, 3));
    sized.replace("</Form>", &format!("{controls}</Form>"))
}

/// The bundle of the fixture program with the three strips, served; and a page of `chrome` open on it once the program's
/// `onLoad` has written its label.
fn open_strips(chrome: &Chrome) -> (Page, Server) {
    let player_out = support::built_player();
    let mut input = support::input_with(
        PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    );
    for f in input.forms.iter_mut().filter(|f| f.name == MAIN_FORM) {
        f.cfrm = strips_form(&f.cfrm);
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

/// What every snippet starts with: the shadow root, a strip, its tabs, which one is selected, the strip's place along its own
/// axis, and whether a tab lies wholly inside the strip's box.
const LIB: &str = r#"
  const root = document.getElementById('pr-app').shadowRoot;
  const el = id => root.querySelector('[data-name="' + id + '"]');
  const list = id => { const e = el(id); return e && e.querySelector(':scope > .pr-tablist'); };
  const tabs = id => { const l = list(id); return l ? Array.from(l.querySelectorAll(':scope > .pr-tab')) : []; };
  const vertical = id => list(id).getAttribute('aria-orientation') === 'vertical';
  const selected = id => tabs(id).findIndex(b => b.getAttribute('aria-selected') === 'true') + 1;
  const at = id => vertical(id) ? list(id).scrollTop : list(id).scrollLeft;
  const room = id => vertical(id) ? list(id).clientHeight : list(id).clientWidth;
  const length = id => vertical(id) ? list(id).scrollHeight : list(id).scrollWidth;
  const along = (id, r) => vertical(id) ? [r.top, r.bottom] : [r.left, r.right];
  const shown = (id, n) => {
    const t = tabs(id)[n - 1];
    if (!t) return false;
    const [a, b] = along(id, t.getBoundingClientRect());
    const [s, e] = along(id, list(id).getBoundingClientRect());
    return a >= s - 0.5 && b <= e + 0.5;
  };
  // Tab n starts inside the strip and runs past its far edge: cut by it.
  const cut = (id, n) => {
    const t = tabs(id)[n - 1];
    if (!t) return false;
    const [a, b] = along(id, t.getBoundingClientRect());
    const [, e] = along(id, list(id).getBoundingClientRect());
    return a < e - 1 && b > e + 1;
  };
"#;

fn js(body: &str) -> String {
    format!("(() => {{ {LIB} {body} }})()")
}

/// A key pressed and released on whatever has the focus.
fn press(page: &mut Page, key: &str, code: u32) {
    for kind in ["keyDown", "keyUp"] {
        page.call("Input.dispatchKeyEvent", json!({"type": kind, "key": key, "code": key, "windowsVirtualKeyCode": code})).unwrap();
    }
}

/// The rectangle of tab `n` of `id`, for a click at its centre.
fn tab_rect(id: &str, n: usize) -> String {
    js(&format!("const t = tabs('{id}')[{n} - 1]; if (!t) return null; const r = t.getBoundingClientRect(); return {{ x: r.x, y: r.y, width: r.width, height: r.height }};"))
}

/// The page itself has not moved: the strip scrolled, the document did not.
const PAGE_AT_REST: &str = "return window.scrollX === 0 && window.scrollY === 0 && document.scrollingElement.scrollTop === 0 && document.scrollingElement.scrollLeft === 0;";

#[test]
fn a_strip_with_more_tabs_than_room_slides_with_no_scroll_bar_and_a_strip_that_fits_has_nothing_to_slide() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, server) = open_strips(&chrome);
    page.wait_for(&js("return tabs('TABS-TOP').length > 0 && tabs('TABS-LEFT').length > 0 && tabs('TABS-FIT').length > 0;"), 10).unwrap();
    let look = page
        .eval(&js(
            "const out = {};
             for (const id of ['TABS-TOP', 'TABS-LEFT', 'TABS-FIT']) {
               const l = list(id), cs = getComputedStyle(l);
               out[id] = { tabs: tabs(id).length, selected: selected(id), vertical: vertical(id), overflowX: cs.overflowX, overflowY: cs.overflowY, bar: cs.scrollbarWidth,
                 // What a scroll bar would take from the strip: the box's size less the room inside it, across the strip.
                 taken: vertical(id) ? l.offsetWidth - l.clientWidth : l.offsetHeight - l.clientHeight,
                 room: room(id), length: length(id), at: at(id), kept: tabs(id).every(b => getComputedStyle(b).flexShrink === '0'), first: shown(id, 1) };
             }
             return out;",
        ))
        .unwrap();
    let f = |id: &str, k: &str| look[id][k].as_f64().unwrap_or_else(|| panic!("{id}.{k} in {look}"));
    for id in ["TABS-TOP", "TABS-LEFT"] {
        assert_eq!(look[id]["tabs"], json!(TABS), "{id}: every tab is on the page, shown or not");
        assert_eq!(look[id]["selected"], json!(1), "{id}: the first tab is the selected one: {look}");
        assert_eq!(look[id]["first"], json!(true), "{id}: the selected tab is on show");
        assert_eq!(look[id]["kept"], json!(true), "{id}: a tab keeps its size instead of shrinking to fit");
        assert_eq!(look[id]["bar"], json!("none"), "{id}: no scroll bar, no slider");
        assert_eq!(f(id, "taken"), 0.0, "{id}: and nothing taken from the strip for one");
        assert!(f(id, "length") > 3.0 * f(id, "room"), "{id}: the tabs need far more room ({}) than the strip has ({})", f(id, "length"), f(id, "room"));
        assert_eq!(f(id, "at"), 0.0, "{id}: the strip starts at its first tab");
    }
    assert_eq!(look["TABS-TOP"]["vertical"], json!(false));
    assert_eq!(look["TABS-TOP"]["overflowX"], json!("auto"), "a top strip slides sideways");
    assert_eq!(look["TABS-TOP"]["overflowY"], json!("hidden"));
    assert_eq!(look["TABS-LEFT"]["vertical"], json!(true));
    assert_eq!(look["TABS-LEFT"]["overflowY"], json!("auto"), "a left strip slides down");
    assert_eq!(look["TABS-LEFT"]["overflowX"], json!("hidden"));
    assert_eq!(look["TABS-FIT"]["tabs"], json!(3));
    assert!(f("TABS-FIT", "length") <= f("TABS-FIT", "room"), "a strip whose tabs all fit has nothing to slide: {} in {}", f("TABS-FIT", "length"), f("TABS-FIT", "room"));
    assert_eq!(look["TABS-FIT"]["first"], json!(true));

    // The strip really moves: slid to its far end, the last tab is wholly on show and the first is not.
    page.eval(&js("const l = list('TABS-TOP'); l.scrollLeft = l.scrollWidth; return at('TABS-TOP');")).unwrap();
    page.wait_for(&js(&format!("return shown('TABS-TOP', {TABS}) && !shown('TABS-TOP', 1);")), 10).unwrap_or_else(|e| panic!("the top strip does not slide to its last tab: {e}"));
    page.eval(&js("const l = list('TABS-LEFT'); l.scrollTop = l.scrollHeight; return at('TABS-LEFT');")).unwrap();
    page.wait_for(&js(&format!("return shown('TABS-LEFT', {TABS}) && !shown('TABS-LEFT', 1);")), 10).unwrap_or_else(|e| panic!("the left strip does not slide to its last tab: {e}"));
    assert_eq!(page.eval(&js(PAGE_AT_REST)).unwrap(), json!(true), "the page did not move");
    drop(page);
    server.stop();
    println!(
        "web TabControl strip in Chrome ({}): top strip {TABS} tabs need {} px in {} px, left strip {TABS} tabs need {} px in {} px, both slide to their last tab with the tabs at their own size and no scroll bar (nothing taken from the strip for one); a strip of 3 needs {} px in {} px and has nothing to slide",
        exe.display(),
        f("TABS-TOP", "length"),
        f("TABS-TOP", "room"),
        f("TABS-LEFT", "length"),
        f("TABS-LEFT", "room"),
        f("TABS-FIT", "length"),
        f("TABS-FIT", "room"),
    );
}

/// The mouse, which the browser does not slide a sideways box with: a drag along the strip, and the vertical wheel.
#[test]
fn the_mouse_slides_the_strip_by_drag_and_by_wheel_and_a_drag_presses_no_tab() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, server) = open_strips(&chrome);
    page.wait_for(&js("return tabs('TABS-TOP').length > 0 && shown('TABS-TOP', 1);"), 10).unwrap();
    let mouse = |page: &mut Page, kind: &str, x: f64, y: f64, extra: serde_json::Value| {
        let mut p = json!({"type": kind, "x": x, "y": y});
        for (k, v) in extra.as_object().unwrap() {
            p[k] = v.clone();
        }
        page.call("Input.dispatchMouseEvent", p).unwrap();
    };
    let centre = |page: &mut Page, n: usize| {
        let r = page.eval(&tab_rect("TABS-TOP", n)).unwrap();
        (r["x"].as_f64().unwrap() + r["width"].as_f64().unwrap() / 2.0, r["y"].as_f64().unwrap() + r["height"].as_f64().unwrap() / 2.0)
    };

    // A drag: press on tab 3, carry the pointer 150 px toward the strip's start in steps, let go. The strip slides 150 px.
    let (x, y) = centre(&mut page, 3);
    mouse(&mut page, "mouseMoved", x, y, json!({}));
    mouse(&mut page, "mousePressed", x, y, json!({"button": "left", "buttons": 1, "clickCount": 1}));
    for step in 1..=5 {
        mouse(&mut page, "mouseMoved", x - 30.0 * step as f64, y, json!({"button": "left", "buttons": 1}));
    }
    let dragged = page.eval(&js("return [at('TABS-TOP'), getComputedStyle(list('TABS-TOP')).cursor];")).unwrap();
    mouse(&mut page, "mouseReleased", x - 150.0, y, json!({"button": "left", "buttons": 0, "clickCount": 1}));
    let slid = dragged[0].as_f64().unwrap();
    assert!((slid - 150.0).abs() <= 2.0, "the pointer went 150 px and the strip slid {slid}");
    assert_eq!(dragged[1], json!("grabbing"), "the cursor closes to a hand while it slides");
    // The press began on tab 3 and the click that ends a drag must not press it: tab 1 is still the selected one.
    assert_eq!(page.eval(&js("return [selected('TABS-TOP'), getComputedStyle(list('TABS-TOP')).cursor];")).unwrap(), json!([1, "auto"]), "a drag presses no tab, and the hand opens again");
    assert_eq!(page.eval(&js(PAGE_AT_REST)).unwrap(), json!(true), "the page did not move");

    // The next press that does not move IS a click: the tab under it is selected (the swallowed click did not outlast the drag).
    let part = page.eval(&js("const e = list('TABS-TOP').getBoundingClientRect(); const t = tabs('TABS-TOP'); const i = t.findIndex(b => { const r = b.getBoundingClientRect(); return r.left > e.left + 2 && r.right < e.right - 2; }); return i + 1;")).unwrap().as_u64().unwrap() as usize;
    let (x, y) = centre(&mut page, part);
    page.click_at(&tab_rect("TABS-TOP", part)).unwrap();
    page.wait_for(&js(&format!("return selected('TABS-TOP') === {part};")), 10).unwrap_or_else(|e| panic!("a press that does not move did not select tab {part}: {e}"));

    // The vertical wheel slides a strip that runs sideways. A notch glides; a small step (a trackpad's) follows at once.
    let (wx, wy) = (x, y);
    let before = page.eval(&js("return at('TABS-TOP');")).unwrap().as_f64().unwrap();
    mouse(&mut page, "mouseWheel", wx, wy, json!({"deltaX": 0.0, "deltaY": 120.0}));
    page.wait_for(&js(&format!("return Math.abs(at('TABS-TOP') - {}) <= 2;", before + 120.0)), 10).unwrap_or_else(|e| panic!("one notch of the wheel did not slide the strip 120 px from {before}: {e}"));
    mouse(&mut page, "mouseWheel", wx, wy, json!({"deltaX": 0.0, "deltaY": -60.0}));
    page.wait_for(&js(&format!("return Math.abs(at('TABS-TOP') - {}) <= 2;", before + 60.0)), 10).unwrap_or_else(|e| panic!("a notch back did not bring the strip to {}: {e}", before + 60.0));
    mouse(&mut page, "mouseWheel", wx, wy, json!({"deltaX": 0.0, "deltaY": 10.0}));
    page.wait_for(&js(&format!("return Math.abs(at('TABS-TOP') - {}) <= 1;", before + 70.0)), 10).unwrap_or_else(|e| panic!("a small step of the wheel did not slide the strip 10 px: {e}"));
    assert_eq!(page.eval(&js(PAGE_AT_REST)).unwrap(), json!(true), "the wheel slid the strip, not the page");
    // A strip whose tabs all fit leaves the wheel alone, and a drag along it slides nothing.
    let fit = page.eval(&tab_rect("TABS-FIT", 2)).unwrap();
    let (fx, fy) = (fit["x"].as_f64().unwrap() + 5.0, fit["y"].as_f64().unwrap() + 5.0);
    mouse(&mut page, "mouseMoved", fx, fy, json!({}));
    mouse(&mut page, "mousePressed", fx, fy, json!({"button": "left", "buttons": 1, "clickCount": 1}));
    mouse(&mut page, "mouseMoved", fx - 40.0, fy, json!({"button": "left", "buttons": 1}));
    mouse(&mut page, "mouseReleased", fx - 40.0, fy, json!({"button": "left", "buttons": 0, "clickCount": 1}));
    assert_eq!(page.eval(&js("return at('TABS-FIT');")).unwrap(), json!(0), "a strip that fits slides nowhere");
    drop(page);
    server.stop();
    println!(
        "web TabControl mouse in Chrome ({}): a 150 px drag slid the strip {slid:.0} px with the hand closed and pressed no tab; a press that did not move selected tab {part}; a wheel notch of 120 slid it 120, a notch back 60, a small step 10; the page stayed at rest; a strip that fits slid nowhere",
        exe.display()
    );
}

#[test]
fn the_tab_that_becomes_the_selected_one_is_brought_into_view_and_a_redraw_leaves_the_strip_where_it_was() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let (mut page, server) = open_strips(&chrome);
    page.wait_for(&js("return tabs('TABS-TOP').length > 0 && shown('TABS-TOP', 1);"), 10).unwrap();

    // A real press on the selected tab gives the strip the keyboard; End then selects the last of 40 tabs, far out of sight.
    page.click_at(&tab_rect("TABS-TOP", 1)).unwrap();
    page.wait_for(&js("return !!root.activeElement && root.activeElement.classList.contains('pr-tab');"), 10).unwrap_or_else(|e| panic!("the press did not focus the tab: {e}"));
    press(&mut page, "End", 35);
    page.wait_for(&js(&format!("return selected('TABS-TOP') === {TABS} && shown('TABS-TOP', {TABS});")), 10)
        .unwrap_or_else(|e| panic!("the last tab was selected by End and is not on show: {e}"));
    let far = page.eval(&js("return at('TABS-TOP');")).unwrap().as_f64().unwrap();
    assert!(far > 0.0, "the strip moved to the selected tab");
    assert_eq!(page.eval(&js(PAGE_AT_REST)).unwrap(), json!(true), "the page did not move, only the strip");

    // Home selects the first again, from the other end, and the strip glides back to its start.
    press(&mut page, "Home", 36);
    page.wait_for(&js("return selected('TABS-TOP') === 1 && shown('TABS-TOP', 1) && at('TABS-TOP') === 0;"), 10)
        .unwrap_or_else(|e| panic!("the first tab was selected by Home and the strip is not back at its start: {e}"));

    // The operator scrolls the strip away from the selected tab; a redraw that leaves the selection alone (a button writes a label)
    // does not take it back.
    page.eval(&js("list('TABS-TOP').scrollLeft = 300; return at('TABS-TOP');")).unwrap();
    page.wait_for(&js("return Math.abs(at('TABS-TOP') - 300) < 1 && !shown('TABS-TOP', 1);"), 10).unwrap();
    let go = format!("const b = root.querySelector('[data-pr-form=\"{MAIN_FORM}\"] [data-name=\"BTN-GO\"]'); const r = b.getBoundingClientRect(); return {{ x: r.x, y: r.y, width: r.width, height: r.height }};");
    page.click_at(&js(&go)).unwrap();
    page.wait_for(&format!("(() => {{ const r = document.getElementById('pr-app').shadowRoot; const l = r.querySelector('[data-pr-form=\"{MAIN_FORM}\"] [data-name=\"{LBL_OUT}\"]'); return !!l && l.textContent !== 'loaded'; }})()"), 10)
        .unwrap_or_else(|e| panic!("the button did not redraw the form: {e}"));
    let kept = page.eval(&js("return [at('TABS-TOP'), selected('TABS-TOP')];")).unwrap();
    assert_eq!(kept[1], json!(1), "the selection did not change");
    assert!((kept[0].as_f64().unwrap() - 300.0).abs() < 1.0, "the redraw left the strip where the operator put it: {kept}");

    // A press on the part of a tab that shows at the strip's far edge selects it, and the strip brings the whole tab into view.
    let part = page
        .eval(&js(
            "const edge = list('TABS-TOP').getBoundingClientRect(); const t = tabs('TABS-TOP');
             for (let i = 0; i < t.length; i++) {
               const r = t[i].getBoundingClientRect();
               if (r.left < edge.right - 0.5 && r.right > edge.right + 0.5) { const x = Math.max(r.left, edge.left); return { n: i + 1, x: x, y: r.y, width: Math.min(r.right, edge.right) - x, height: r.height }; }
             }
             return null;",
        ))
        .unwrap();
    let cut = part["n"].as_u64().unwrap_or_else(|| panic!("no tab is cut by the strip's far edge at 300 px: {part}")) as usize;
    assert!(!page.eval(&js(&format!("return shown('TABS-TOP', {cut});"))).unwrap().as_bool().unwrap(), "tab {cut} is only partly on show");
    page.click_at(&format!("({part})")).unwrap();
    // The strip glides there: the tab whole, and a sliver of the next one beside it.
    page.wait_for(&js(&format!("return selected('TABS-TOP') === {cut} && shown('TABS-TOP', {cut}) && cut('TABS-TOP', {});", cut + 1)), 10)
        .unwrap_or_else(|e| panic!("tab {cut} was pressed where it showed and is not the selected one wholly on show with a sliver of tab {} beside it: {e}", cut + 1));
    drop(page);
    server.stop();
    println!(
        "web TabControl reveal in Chrome ({}): End selected tab {TABS} of {TABS} and the strip went to {far:.0} px with the page at rest; Home returned it to 0; scrolled to 300 px by the operator it stayed at {:.0} px through a redraw; a press on the visible part of tab {cut}, cut by the edge, selected it and brought all of it into view with a sliver of the next tab beside it",
        exe.display(),
        kept[0].as_f64().unwrap()
    );
}
