// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The layers reference form, the same script on the desktop engine and in the browser, side by side** (WASM-015
//! AC13). `layers_chrome` drives the browser alone; the desktop's own `layers_*_091.rs` tests assert the desktop's
//! rules, not this script. This test runs *one* script on both and compares what each observed:
//!
//! * **visibility** — at the start no layer is drawn; once a layer is shown its control is drawn;
//! * **stacking and hit testing** — over a hidden layer's button the base button is hit; once the layer is shown its
//!   button (`ZOrder` 0) is hit above the base button (`ZOrder` 209) at the same point; at a point of the base button
//!   the translucent layer leaves free the base is hit; at the same point under an opaque layer nothing is hit;
//! * **tab order** — the base in its tab order; then the shown layer in its own; under an opaque layer, only that layer.
//!
//! The desktop half is the run form's engine itself (`cobolt_forms::render::render_form`, the engine `rcrun` and the
//! IDE run), given the form the Web project holds, one frame of pointer and key events at a time. The browser half is
//! the real Build, served and driven in a headless Chrome. Each half fills the same [`Seen`]; the two are printed
//! side by side and compared.
//!
//! **The differences WASM-004 records, and what this comparison found** (R17, R20):
//!
//! * R17 — the browser clips a layer's control to the form's arcs for every control type; the desktop clips the 55 types
//!   that can clip themselves and not the seven that cannot, `DataGrid` among them (spec 091 R14). The reference form's
//!   `DG-CORNER` sits at the rounded corner, so the corner is the one place the two are *meant* to differ. The form's arc
//!   belongs to the window, not to `render_form`, so no desktop clip is measured here; `layers_chrome` measures the
//!   browser's. The table has no row for it.
//! * R20 — the browser takes the keyboard order from DOM order (the layers appended in stack order, each sorted by its own
//!   tab order); the desktop walks the base and then each layer by its own focus chain. On this form the two walk the
//!   same order in all three states (no layer, `LYR-SEE`, the opaque `LYR-TOP`), so [`DOCUMENTED`] is empty. A difference
//!   found later is printed in the table and fails the test until it is listed there with its reason.
//!
//! Two things the desktop half does to make its walk readable, neither of which changes what the engine does: every
//! control but the Label is given an `onGotFocus` handler (the engine reports a focus only for a bound handler; a Label
//! is a place in the desktop's tab order that raises the event and lets the focus walk on, and on the page it is no stop,
//! so it is left without one in both walks), and a Tab press that reports nothing is read as the one `DataGrid` the shown
//! layers hold, because the desktop's grid is a tab stop that raises no `onGotFocus` (see [`desktop_tab_walk`]).
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

#[path = "../../cobolt-web-bundle/tests/support/cdp.rs"]
#[allow(dead_code)]
mod cdp;

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Form;
use cobolt_forms::render::{render_form, Backdrop, FormState, RenderInput, RenderMode};
use cobolt_forms::xml::load_form_from_str;
use cobolt_project_tools::web::devserver::{self, ServeOptions};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildRequest};
use egui::{pos2, Rect, Shape, Vec2};

const MANIFEST: &str = "[project]\nname = \"Layers.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
    [files]\nforms = [\"webforms/main-form.wcfrm\"]\n\n[web]\nmulti_memory = true\n";

/// Differences found by this comparison that WASM-004 R17 or R20 records. Each is `(field of Seen, why)`; a field in this
/// list may differ between the two engines, any other must not.
const DOCUMENTED: &[(&str, &str)] = &[];

fn handler(control: &str, body: &str) -> String {
    format!(
        "<Event name=\"onClick\" paragraph=\"{control}--ONCLICK\"><![CDATA[       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n{body}]]></Event>"
    )
}

/// The reference form with layers (the one `layers_chrome` builds).
fn form_xml() -> String {
    let base = handler("BTN-BASE", "           MOVE \"base clicked\" TO LBL-OUT::Caption.\n");
    let see = handler("BTN-SEE", "           MOVE 1 TO LYR-SEE::Visible.\n           MOVE \"see shown\" TO LBL-OUT::Caption.\n");
    let top = handler("BTN-TOP", "           MOVE 1 TO LYR-TOP::Visible.\n           MOVE \"top shown\" TO LBL-OUT::Caption.\n");
    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Layers" width="900" height="600" index-base="1" corner-radius="24" main-form="true">
  <Layer name="LYR-SEE" color="#10203080"/>
  <Layer name="LYR-TOP" color="#203040"/>
  <Control id="BTN-BASE" type="Button" x="300" y="100" w="160" h="60" z-order="209" tab-order="2"><Property name="Caption">Base</Property>{base}</Control>
  <Control id="BTN-SEE" type="Button" x="50" y="300" w="100" h="30" tab-order="1"><Property name="Caption">See</Property>{see}</Control>
  <Control id="LBL-OUT" type="Label" x="600" y="20" w="250" h="24"><Property name="Caption">ready</Property></Control>
  <Control id="BTN-TOP" type="Button" x="380" y="130" w="160" h="60" z-order="0" tab-order="1" layer="LYR-SEE"><Property name="Caption">Top</Property>{top}</Control>
  <Control id="DG-CORNER" type="DataGrid" x="0" y="500" w="200" h="100" tab-order="2" layer="LYR-SEE"></Control>
  <Control id="TXT-T2" type="TextBox" x="600" y="300" w="150" h="28" tab-order="2" layer="LYR-TOP"></Control>
  <Control id="TXT-T1" type="TextBox" x="600" y="250" w="150" h="28" tab-order="1" layer="LYR-TOP"></Control>
</Form>
"##
    )
}

/// What one engine observed running the script. A point is `(x, y)` from the form's top-left corner.
#[derive(Debug, Default, Clone, PartialEq)]
struct Seen {
    /// Is a layer's control drawn with every layer hidden?
    top_drawn_at_start: bool,
    /// The button hit at the overlap of the base button and the (hidden) layer's button.
    hit_over_hidden_layer: Option<String>,
    /// The Tab walk with no layer shown (the first two stops).
    walk_no_layer: Vec<String>,
    /// Is the layer's control drawn once `LYR-SEE` is shown?
    top_drawn_when_shown: bool,
    /// The button hit at the overlap with `LYR-SEE` shown: the layer's button above the base's.
    hit_overlap_shown: Option<String>,
    /// The button hit at a point of the base button the translucent layer leaves free.
    hit_through_translucent: Option<String>,
    /// The Tab walk with `LYR-SEE` shown (the first four stops).
    walk_layer_shown: Vec<String>,
    /// The button hit at that same point with the opaque layer `LYR-TOP` shown too.
    hit_under_opaque: Option<String>,
    /// The Tab walk with the opaque layer shown (the first two stops).
    walk_opaque: Vec<String>,
    /// Does that walk reach a base or lower-layer button?
    opaque_walk_reaches_a_button: bool,
}

/// The fields of [`Seen`], in the order the table prints them.
fn rows(s: &Seen) -> Vec<(&'static str, String)> {
    vec![
        ("top_drawn_at_start", format!("{}", s.top_drawn_at_start)),
        ("hit_over_hidden_layer", format!("{:?}", s.hit_over_hidden_layer)),
        ("walk_no_layer", format!("{:?}", s.walk_no_layer)),
        ("top_drawn_when_shown", format!("{}", s.top_drawn_when_shown)),
        ("hit_overlap_shown", format!("{:?}", s.hit_overlap_shown)),
        ("hit_through_translucent", format!("{:?}", s.hit_through_translucent)),
        ("walk_layer_shown", format!("{:?}", s.walk_layer_shown)),
        ("hit_under_opaque", format!("{:?}", s.hit_under_opaque)),
        ("walk_opaque", format!("{:?}", s.walk_opaque)),
        ("opaque_walk_reaches_a_button", format!("{}", s.opaque_walk_reaches_a_button)),
    ]
}

// ── The desktop half ────────────────────────────────────────────────────────────────────────────────────────────

const FORM: (f32, f32) = (900.0, 600.0);

/// A state that shows exactly the layers it names.
struct Shown(&'static [&'static str]);
impl FormState for Shown {
    fn layer_visible(&self, name: &str) -> bool {
        self.0.iter().any(|n| n.eq_ignore_ascii_case(name))
    }
}

/// Run the form's engine for `frames` of events; the events each frame raised (`(control, event)`) and the shapes of the last frame.
fn desktop_run(form: &Form, state: &dyn FormState, frames: Vec<Vec<egui::Event>>) -> (Vec<Vec<(String, String)>>, Vec<Shape>) {
    let ctx = egui::Context::default();
    ctx.set_fonts(egui::FontDefinitions::default());
    let active = ActiveTabs::new();
    let mut seen: Vec<Vec<(String, String)>> = Vec::new();
    let mut last: Vec<Shape> = Vec::new();
    for (i, events) in frames.into_iter().enumerate() {
        seen.push(Vec::new());
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(FORM.0, FORM.1)));
        raw.focused = true;
        raw.time = Some(i as f64 * 0.05);
        raw.events = events;
        let mut full = ctx.run_ui(raw, |root_ui| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(root_ui, |ui| {
                ui.set_min_size(Vec2::new(FORM.0, FORM.1));
                let inp = RenderInput {
                    css_final: None,
                    controls: &form.controls,
                    state,
                    form_size: Vec2::new(FORM.0, FORM.1),
                    glass: false,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Backdrop { paint: false, layers: form.layers.clone(), ..Default::default() },
                };
                let out = render_form(ui, &inp);
                for e in out.events {
                    if let Some(frame) = seen.last_mut() {
                        frame.push((e.ctrl_id, e.event));
                    }
                }
            });
        });
        full.textures_delta.clear();
        let mut shapes = Vec::new();
        fn walk(s: &Shape, out: &mut Vec<Shape>) {
            match s {
                Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                other => out.push(other.clone()),
            }
        }
        for cs in &full.shapes {
            walk(&cs.shape, &mut shapes);
        }
        last = shapes;
    }
    (seen, last)
}

fn desktop_drawn(form: &Form, state: &dyn FormState, needle: &str) -> bool {
    let (_, shapes) = desktop_run(form, state, vec![vec![], vec![]]);
    shapes.iter().any(|s| matches!(s, Shape::Text(t) if t.galley.text().contains(needle)))
}

/// The button a click at `p` ran the `onClick` of (a few frames to settle, the pointer moves, goes down, goes up).
fn desktop_click(form: &Form, state: &dyn FormState, p: (f32, f32)) -> Option<String> {
    let at = pos2(p.0, p.1);
    let button = |pressed: bool| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::default() };
    let frames = vec![
        vec![],
        vec![],
        vec![],
        vec![egui::Event::PointerMoved(at)],
        vec![egui::Event::PointerMoved(at), button(true)],
        vec![egui::Event::PointerMoved(at), button(false)],
        vec![],
    ];
    let (seen, _) = desktop_run(form, state, frames);
    seen.into_iter().flatten().find(|(_, e)| e == "onClick").map(|(c, _)| c)
}

/// The controls that took the focus, one per Tab press, as the engine reports it (`onGotFocus`).
///
/// The desktop's `DataGrid` is a tab stop that raises no `onGotFocus` (its focus is the grid's own `datagrid-focus`, which no
/// event reads), so its stop shows as a press that reports nothing. The one `DataGrid` the shown layers hold is the
/// only control that can be that stop, so such a press is read as that grid; with none or several it stays `(silent)`.
/// The browser's walk counts a control with several stops (a grid's cells) once, so the desktop's does too.
fn desktop_tab_walk(form: &Form, state: &dyn FormState, presses: usize) -> Vec<String> {
    let tab = |pressed: bool| egui::Event::Key { key: egui::Key::Tab, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::default() };
    let mut frames: Vec<Vec<egui::Event>> = vec![vec![]];
    for _ in 0..presses {
        frames.push(vec![tab(true)]);
        frames.push(vec![tab(false)]);
        frames.push(vec![]);
    }
    let (seen, _) = desktop_run(form, state, frames);
    let grids: Vec<&str> = form
        .controls
        .iter()
        .filter(|c| c.control_type == cobolt_forms::model::ControlType::DataGrid)
        .filter(|c| c.layer.as_deref().is_none_or(|l| state.layer_visible(l)))
        .map(|c| c.id.as_str())
        .collect();
    let silent = if grids.len() == 1 { grids[0].to_owned() } else { "(silent)".to_owned() };
    let mut walk: Vec<String> = Vec::new();
    for press in 0..presses {
        let frames_of_press = &seen[1 + 3 * press..1 + 3 * press + 3];
        let focused = frames_of_press.iter().flatten().find(|(_, e)| e == "onGotFocus").map(|(c, _)| c.clone());
        let name = focused.unwrap_or_else(|| silent.clone());
        if walk.last() != Some(&name) {
            walk.push(name);
        }
    }
    walk
}

fn first(walk: Vec<String>, n: usize) -> Vec<String> {
    walk.into_iter().take(n).collect()
}

fn on_the_desktop() -> Seen {
    let mut form = load_form_from_str(&form_xml()).unwrap_or_else(|e| panic!("the desktop could not load the layers form: {e}"));
    // The engine raises `onGotFocus` for a control that has a handler bound to it, which is how a program learns of
    // the focus; the Tab walk is read from those events, so every control that is a tab stop of the script gets one (not the Label, which is no stop on the page; the browser's walk is read from
    // `document.activeElement`, which needs none).
    for c in form.controls.iter_mut().filter(|c| c.control_type != cobolt_forms::model::ControlType::Label) {
        c.events.push(cobolt_forms::model::EventBinding { event: "onGotFocus".into(), paragraph: format!("{}-ONGOTFOCUS", c.id), code: "DISPLAY \"x\"".into() });
    }
    // Form coordinates: BTN-BASE is (300, 100, 160, 60), BTN-TOP (380, 130, 160, 60).
    let (overlap, free) = ((420.0, 145.0), (315.0, 115.0));
    let none = Shown(&[]);
    let see = Shown(&["LYR-SEE"]);
    let both = Shown(&["LYR-SEE", "LYR-TOP"]);
    Seen {
        top_drawn_at_start: desktop_drawn(&form, &none, "Top"),
        hit_over_hidden_layer: desktop_click(&form, &none, overlap),
        walk_no_layer: first(desktop_tab_walk(&form, &none, 3), 2),
        top_drawn_when_shown: desktop_drawn(&form, &see, "Top"),
        hit_overlap_shown: desktop_click(&form, &see, overlap),
        hit_through_translucent: desktop_click(&form, &see, free),
        walk_layer_shown: first(desktop_tab_walk(&form, &see, 8), 4),
        hit_under_opaque: desktop_click(&form, &both, free),
        walk_opaque: first(desktop_tab_walk(&form, &both, 3), 2),
        opaque_walk_reaches_a_button: desktop_tab_walk(&form, &both, 3).iter().any(|n| n.starts_with("BTN-")),
    }
}

// ── The browser half ───────────────────────────────────────────────────────────────────────────────────────────

const LIB: &str = "const root = document.getElementById('pr-app').shadowRoot; const el = id => root.querySelector('[data-name=\"' + id + '\"]'); \
    const form = root.querySelector('[data-pr-form=\"main-form\"]'); const layer = n => form.querySelector(':scope > .pr-form-layer[data-layer=\"' + n + '\"]'); \
    const at = (x, y) => { const e = root.elementFromPoint(x, y); const c = e && e.closest('[data-name]'); return e ? (c ? c.dataset.name : e.className) : null; }; \
    const rect = id => { const r = el(id).getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; }; \
    const caption = () => el('LBL-OUT').textContent; \
    const drawn = n => getComputedStyle(layer(n)).display !== 'none' && !layer(n).hasAttribute('hidden');";

fn eval(page: &mut cdp::Page, body: &str) -> serde_json::Value {
    page.eval(&format!("(() => {{ {LIB} {body} }})()")).unwrap_or_else(|e| panic!("{e}; console {:?}; exceptions {:?}", page.console, page.exceptions))
}

fn wait(page: &mut cdp::Page, cond: &str, what: &str) {
    page.wait_for(&format!("(() => {{ {LIB} return {cond}; }})()"), 15)
        .unwrap_or_else(|e| panic!("{what}: {e}; console {:?}; exceptions {:?}", page.console, page.exceptions));
}

/// A button name for a point's hit: the control's name when it is a button, `None` for a backdrop or anything else.
fn hit_name(v: &serde_json::Value) -> Option<String> {
    v.as_str().filter(|n| n.starts_with("BTN-")).map(str::to_owned)
}

/// The focused control's name as Tab leaves it; a control with more than one stop counts once.
fn browser_tab_walk(page: &mut cdp::Page, steps: usize) -> Vec<String> {
    // A click on empty page space moves the browser's focus starting point back to the start of the page.
    page.click_at("({ x: 1090, y: 690, width: 0, height: 0 })").unwrap();
    let mut seen: Vec<String> = Vec::new();
    for _ in 0..steps {
        page.press_key("Tab").unwrap();
        let v = eval(page, "const a = root.activeElement; const c = a && a.closest('[data-name]'); return c ? c.dataset.name : (a ? a.tagName : null);");
        let name = v.as_str().unwrap_or("-").to_owned();
        if seen.last() != Some(&name) {
            seen.push(name);
        }
    }
    seen
}

fn in_the_browser(exe: &std::path::Path) -> Seen {
    let dir = std::env::temp_dir().join(format!("prc-layers-desktop-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::write(dir.join("Layers.project.toml"), MANIFEST).unwrap();
    std::fs::write(dir.join("webforms/main-form.wcfrm"), form_xml()).unwrap();
    let req = WebBuildRequest::from_manifest(&dir.join("Layers.project.toml")).unwrap();
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
    let mut server = devserver::start(&report.bundle_dir, &ServeOptions { port: 0, isolation: req.web.isolation, live_reload: false }).unwrap();
    let chrome = cdp::Chrome::launch(exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1100, 700).unwrap();
    page.navigate(&server.url()).unwrap_or_else(|e| panic!("{e}"));
    wait(&mut page, "!!el('LBL-OUT') && caption() === 'ready'", "the application did not start");

    // The same two points, from BTN-BASE's corner: the overlap with BTN-TOP, and a point of BTN-BASE the layer leaves free.
    let probe = |page: &mut cdp::Page, dx: f64, dy: f64| hit_name(&eval(page, &format!("const b = rect('BTN-BASE'); return at(b.x + {dx}, b.y + {dy});")));
    let mut seen = Seen {
        top_drawn_at_start: eval(&mut page, "return drawn('LYR-SEE') || drawn('LYR-TOP');").as_bool().unwrap_or(true),
        ..Seen::default()
    };
    seen.hit_over_hidden_layer = probe(&mut page, 120.0, 45.0);
    seen.walk_no_layer = first(browser_tab_walk(&mut page, 3), 2);

    // BTN-SEE's COBOL handler shows LYR-SEE.
    page.click_at(&format!("(() => {{ {LIB} return rect('BTN-SEE'); }})()")).unwrap();
    wait(&mut page, "caption() === 'see shown' && drawn('LYR-SEE')", "MOVE 1 TO LYR-SEE::Visible did not show the layer");
    seen.top_drawn_when_shown = eval(&mut page, "return drawn('LYR-SEE');").as_bool().unwrap_or(false);
    seen.hit_overlap_shown = probe(&mut page, 120.0, 45.0);
    seen.hit_through_translucent = probe(&mut page, 15.0, 15.0);
    seen.walk_layer_shown = first(browser_tab_walk(&mut page, 6), 4);

    // BTN-TOP's COBOL handler shows the opaque layer LYR-TOP.
    page.click_at(&format!("(() => {{ {LIB} return rect('BTN-TOP'); }})()")).unwrap();
    wait(&mut page, "caption() === 'top shown' && drawn('LYR-TOP')", "MOVE 1 TO LYR-TOP::Visible did not show the layer");
    seen.hit_under_opaque = probe(&mut page, 15.0, 15.0);
    let walk = browser_tab_walk(&mut page, 3);
    seen.opaque_walk_reaches_a_button = walk.iter().any(|n| n.starts_with("BTN-"));
    seen.walk_opaque = first(walk, 2);

    drop(page);
    server.stop();
    let _ = std::fs::remove_dir_all(&dir);
    seen
}

#[test]
fn the_layers_form_behaves_the_same_on_the_desktop_and_in_the_browser() {
    let Some(exe) = cdp::find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let desktop = on_the_desktop();
    let browser = in_the_browser(&exe);

    println!("{:<30} {:<48} browser", "observation", "desktop");
    let mut unexpected: Vec<String> = Vec::new();
    for ((name, d), (_, b)) in rows(&desktop).into_iter().zip(rows(&browser)) {
        let same = d == b;
        let note = DOCUMENTED.iter().find(|(f, _)| *f == name).map(|(_, why)| *why);
        let mark = match (same, note) {
            (true, _) => "same",
            (false, Some(_)) => "DIFFERS (documented)",
            (false, None) => "DIFFERS",
        };
        println!("{name:<30} {d:<48} {b:<48} {mark}");
        if let (false, Some(why)) = (same, note) {
            println!("{:<30}   ↳ {why}", "");
        }
        if !same && note.is_none() {
            unexpected.push(format!("{name}: desktop {d} ≠ browser {b}"));
        }
    }
    println!("{} observations compared", rows(&desktop).len());
    assert!(unexpected.is_empty(), "the desktop and the browser differ where no WASM-004 R17/R20 difference is recorded:\n{}", unexpected.join("\n"));
}
