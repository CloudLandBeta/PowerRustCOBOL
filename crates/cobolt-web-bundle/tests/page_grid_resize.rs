// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A DataGrid's columns share the width to spare, the cut-off ones first, and the user drags their edges, in a real
//! browser** (2.0.434).
//!
//! A DataGrid with `AutoFitColumns` was filling its width by growing every column alike, so a column whose text was cut off
//! stayed cut off while the others got room they did not need. It is now given what it needs first. And the edge of every
//! column's title can be dragged, as on the desktop (`AllowColumnResize`): the column takes the width the pointer is at,
//! the others share the rest, and the press does not sort the column. A headless Chrome measures the columns before and
//! after, presses on an edge, moves and lets go. With no Chrome on the machine (`CHROME` names one) the test says it was
//! skipped.

mod support;

use cobolt_wasm_abi::fixture::{build, standard_cfrm, FixtureForm, FixtureHandler, FixtureSpec, Step, Target};
use cobolt_wasm_abi::manifest::Limits;
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleInput, BundleOptions, FormInput, Isolation, PlayerInput};
use serde_json::json;
use support::cdp::{find_chrome, Chrome, Page};

const FORM: &str = "main-form";
const LONG: &str = "A rather long customer name that no narrow column shows";

fn spec(multi: bool) -> FixtureSpec {
    let control = |name: &str, kind: &str| (name.to_owned(), kind.to_owned());
    FixtureSpec {
        project: "gridresize".into(),
        multi_memory: multi,
        limits: Limits::default(),
        forms: vec![FixtureForm {
            name: FORM.into(),
            main: true,
            instances: 1,
            controls: vec![control("LBL-OUT", "Label"), control("G-FIT", "DataGrid"), control("G-FIXED", "DataGrid")],
            handlers: vec![FixtureHandler::form("onLoad", vec![Step::Set { target: Target::control("LBL-OUT"), name: "Caption".into(), value: "loaded".into() }])],
        }],
    }
}

/// The fixture's form with two grids of three columns, A (a long name in its rows), B and C (short codes): `G-FIT` has
/// `AutoFitColumns`, `G-FIXED` has not.
fn form(plain: &str) -> String {
    let place = |xml: &str, id: &str, kind: &str, to: &str, inside: &str| {
        let open = format!("<Control id=\"{id}\" type=\"{kind}\" ");
        let at = xml.find(&open).unwrap_or_else(|| panic!("{id} is in the fixture's form"));
        let end = at + xml[at..].find("></Control>").expect("the control's end");
        format!("{}{open}{to}>{inside}</Control>{}", &xml[..at], &xml[end + "></Control>".len()..])
    };
    assert!(plain.contains("width=\"640\" height=\"480\""), "the fixture's form is 640 x 480");
    let xml = plain.replace("width=\"640\" height=\"480\"", "width=\"640\" height=\"480\" index-base=\"1\"");
    let data = format!(
        "<Property name=\"Columns\">A:string\nB:string\nC:string</Property><Property name=\"Rows\">{LONG}\tx1\ty1\nshort\tx2\ty2</Property><Property name=\"RowHeight\">24</Property><Property name=\"ShowCSVExportButton\">0</Property>"
    );
    let xml = place(&xml, "G-FIT", "DataGrid", "x=\"10\" y=\"40\" w=\"560\" h=\"150\"", &format!("{data}<Property name=\"AutoFitColumns\">1</Property>"));
    place(&xml, "G-FIXED", "DataGrid", "x=\"10\" y=\"220\" w=\"560\" h=\"150\"", &format!("{data}<Property name=\"AutoFitColumns\">0</Property>"))
}

const LIB: &str = r#"
  const root = document.getElementById('pr-app').shadowRoot;
  const el = id => root.querySelector('[data-name="' + id + '"]');
  const caption = () => { const e = el('LBL-OUT'); return e ? e.textContent : null; };
  const heads = id => [...el(id).querySelectorAll('[data-part="header"]')];
  const widths = id => heads(id).map(h => Math.round(h.getBoundingClientRect().width * 10) / 10);
  const edge = (id, c) => el(id).querySelector('[data-part="colsize"][data-c="' + c + '"]');
  const rect = e => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; };
  const sorted = id => heads(id).filter(h => h.hasAttribute('aria-sort')).length;
"#;

fn js(body: &str) -> String {
    format!("(() => {{ {LIB} {body} }})()")
}

fn numbers(page: &mut Page, body: &str) -> Vec<f64> {
    let v = page.eval(&js(body)).unwrap_or_else(|e| panic!("{e}"));
    v.as_array().unwrap_or_else(|| panic!("an array, not {v}")).iter().map(|n| n.as_f64().expect("a number")).collect()
}

/// Press on the middle of column `c`'s edge in grid `id`, move `dx` pixels in two steps, and let go.
fn drag_edge(page: &mut Page, id: &str, c: usize, dx: f64) {
    let r = page.eval(&js(&format!("return rect(edge('{id}', {c}));"))).unwrap_or_else(|e| panic!("{e}"));
    let (x, y) = (r["x"].as_f64().unwrap() + r["width"].as_f64().unwrap() / 2.0, r["y"].as_f64().unwrap() + r["height"].as_f64().unwrap() / 2.0);
    let mut mouse = |kind: &str, x: f64, buttons: u32| {
        page.call("Input.dispatchMouseEvent", json!({"type": kind, "x": x, "y": y, "button": if buttons == 0 && kind == "mouseMoved" { "none" } else { "left" }, "buttons": buttons, "clickCount": 1}))
            .unwrap_or_else(|e| panic!("{e}"));
    };
    mouse("mouseMoved", x, 0);
    mouse("mousePressed", x, 1);
    mouse("mouseMoved", x + dx / 2.0, 1);
    mouse("mouseMoved", x + dx, 1);
    mouse("mouseReleased", x + dx, 0);
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1.5
}

#[test]
fn a_grids_columns_share_the_spare_width_cut_off_ones_first_and_their_edges_are_dragged() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let started = std::time::Instant::now();
    let (multi, single) = (build(&spec(true)), build(&spec(false)));
    let player_out = support::built_player();
    let input = BundleInput {
        project: "gridresize".into(),
        forms: vec![FormInput { name: FORM.into(), cfrm: form(&standard_cfrm(&multi, FORM)), stylesheet: None }],
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
    page.resize(900, 700).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for(&js("return caption() === 'loaded';"), 10).unwrap_or_else(|e| panic!("the application did not start ({e})"));
    let browsing = std::time::Instant::now();
    let mut checks = 0;
    let mut check = |ok: bool, what: String| {
        assert!(ok, "{what}");
        checks += 1;
    };

    // ── the spare width ─────────────────────────────────────────────────────
    let fit = numbers(&mut page, "return widths('G-FIT');");
    let fixed = numbers(&mut page, "return widths('G-FIXED');");
    check(fit.len() == 3 && fixed.len() == 3, format!("three titles each: {fit:?} {fixed:?}"));
    // Without `AutoFitColumns` the grid is as it was: three columns of one width, and room left over.
    check(close(fixed[0], fixed[1]) && close(fixed[1], fixed[2]), format!("without AutoFitColumns the columns keep one width: {fixed:?}"));
    // With it the columns fill the grid, and the one with the long name is the widest by far: it was given the room first.
    let grid_w = numbers(&mut page, "const r = el('G-FIT').getBoundingClientRect(); return [r.width];")[0];
    check(close(fit.iter().sum::<f64>(), grid_w), format!("the columns fill the grid ({grid_w}): {fit:?}"));
    check(fit[0] > fit[1] + 60.0 && close(fit[1], fit[2]), format!("the cut-off column takes the spare width first: {fit:?}"));
    check(fit[1] <= fixed[1] + 1.5, format!("a column that was not cut off is not widened: {fit:?} against {fixed:?}"));

    // ── the edges ───────────────────────────────────────────────────────────
    check(numbers(&mut page, "return [el('G-FIT').querySelectorAll('[data-part=\"colsize\"]').length];")[0] == 3.0, "every title has an edge to drag".into());
    drag_edge(&mut page, "G-FIT", 1, 40.0);
    page.wait_for(&js(&format!("return Math.abs(widths('G-FIT')[1] - {}) < 1.5;", fit[1] + 40.0)), 10)
        .unwrap_or_else(|e| panic!("the column did not take the width the pointer was dragged to ({e}); now {:?}", numbers(&mut page, "return widths('G-FIT');")));
    let dragged = numbers(&mut page, "return widths('G-FIT');");
    check(close(dragged[1], fit[1] + 40.0), format!("B is 40 pixels wider: {dragged:?} from {fit:?}"));
    check(close(dragged.iter().sum::<f64>(), grid_w), format!("the others gave up what B took, the grid is still full: {dragged:?}"));
    // What B took came out of the spare width A had been given (A is the cut-off column and holds all that is spare); C, which
    // was not cut off, is where it was.
    check(close(dragged[0], fit[0] - 40.0) && close(dragged[2], fit[2]), format!("A gave up the 40 that B took, C is where it was: {dragged:?} from {fit:?}"));
    check(numbers(&mut page, "return [sorted('G-FIT')];")[0] == 0.0, "the press on the edge did not sort the column".into());
    // Narrower than a column can be drawn stops at the floor.
    drag_edge(&mut page, "G-FIT", 1, -400.0);
    page.wait_for(&js("return Math.abs(widths('G-FIT')[1] - 32) < 1.5;"), 10).unwrap_or_else(|e| panic!("the column did not stop at 32 ({e}); now {:?}", numbers(&mut page, "return widths('G-FIT');")));
    check(true, "a drag to the left stops at the narrowest a column can be".into());
    // The grid without AutoFitColumns is dragged as well: the column takes the width, the others keep theirs.
    drag_edge(&mut page, "G-FIXED", 0, 30.0);
    page.wait_for(&js(&format!("return Math.abs(widths('G-FIXED')[0] - {}) < 1.5;", fixed[0] + 30.0)), 10)
        .unwrap_or_else(|e| panic!("the fixed grid's column did not widen ({e}); now {:?}", numbers(&mut page, "return widths('G-FIXED');")));
    let moved = numbers(&mut page, "return widths('G-FIXED');");
    check(close(moved[1], fixed[1]) && close(moved[2], fixed[2]), format!("the other columns of a grid that does not fit keep their widths: {moved:?}"));

    println!("== page_grid_resize: a DataGrid's width to spare and its draggable column edges in a real browser (headless Chrome) ==");
    println!("  AutoFitColumns, widths A|B|C: {fit:?} (the column with the long name first); without it: {fixed:?}");
    println!("  dragged B by +40: {dragged:?}; by -400: stops at 32; fixed grid A by +30: {moved:?}");
    println!("  {checks} checks; set-up {} ms, in the browser {} ms; all passed, 0 failed", browsing.duration_since(started).as_millis(), browsing.elapsed().as_millis());
    drop(server);
}
