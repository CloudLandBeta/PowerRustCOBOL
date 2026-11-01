// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **One reference form on the desktop and in a browser, compared** (WASM-004 T33: R4, R8, R11; AC3, AC6).
//!
//! The form holds controls of every class a Web project can deploy: class A controls that take input, class B
//! controls drawn from fixed data (`__ChartData`, `Value`), and a class C control (a DataGrid; its data comes through
//! the backend, so only its box is compared). Class D never reaches a browser: Check refuses it (T25), so it has no
//! row here.
//!
//! The same `.wcfrm` is run twice:
//! * **desktop** — `cobolt_forms::render::render_form` (the run form, interactive, headless) for each control's box
//!   (`control_rects`) and the events a scripted input raises; `snapshot::render_form_image` for its picture;
//! * **browser** — the real Build (`GeneratorBackend::build`), served by `devserver`, in a headless Chrome: each
//!   control's box against the form's surface, the page's pixels, and the events its COBOL handlers DISPLAY.
//!
//! Every handler is bound (events fire only for bound handlers) and DISPLAYs `EV <control> <event>`. The script on
//! each class A control is the same on both sides: a click at its centre, then one key (`a` into a TextBox, the Down
//! arrow elsewhere). Compared: geometry per control (to half a pixel), five sampled colours per class B control (to 24
//! levels, the anti-aliasing allowance of `page_border_look.rs`), the event sequence per class A control.
//!
//! Every difference is printed per control; the accepted ones are listed in [`DOCUMENTED`] with their reason (R4),
//! and any other fails the test. With no Chrome on the machine (`CHROME` names one) the test says it was skipped.

#[path = "../../cobolt-web-bundle/tests/support/cdp.rs"]
#[allow(dead_code)]
mod cdp;

use std::collections::HashMap;

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::ControlType;
use cobolt_forms::render::{render_form, Backdrop, DesignedState, RenderInput, RenderMode};
use cobolt_forms::snapshot::{render_form_image, SnapshotOptions};
use cobolt_forms::web_class::{class_of, Class};
use cobolt_project_tools::web::devserver::{self, ServeOptions};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildRequest};
use egui::{Pos2, Rect, Vec2};

const MANIFEST: &str = "[project]\nname = \"Compare.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
    [files]\nforms = [\"webforms/main-form.wcfrm\"]\n\n[web]\nmulti_memory = true\n";

const W: f32 = 1000.0;
const H: f32 = 700.0;

/// The events bound wherever the type supports them.
const WANTED: &[&str] = &[
    "onMouseDown",
    "onMouseUp",
    "onClick",
    "onGotFocus",
    "onLostFocus",
    "onChange",
    "onTextChanged",
    "onValueChanged",
    "onCheckedChanged",
    "onSelectedIndexChanged",
    "onKeyDown",
    "onKeyPress",
    "onKeyUp",
];

/// The differences found (R4), by (control, what), each with what differs. Found by this harness at 2.0.117 and
/// open: none is by design yet; each is a parity item for the player (or, for an event list, for the harness's
/// desktop half, which is `render_form` without the host — nothing feeds a value back between frames).
const FOCUS: &str = "the page raises onGotFocus on the click (and the previous control's onLostFocus); headless render_form reports none for it";
const DOCUMENTED: &[(&str, &str, &str)] = &[
    ("CH-BAR", "colour", "no chart face on the page under the seeded BackgroundColor (white, the desktop's navy [15,20,45]) — the T13b finding"),
    ("CH-PIE", "colour", "no chart face on the page (T13b finding), and the first slice's colour differs ([94,254,156] desktop, [244,152,113] page)"),
    ("KNB-B", "colour", "the dial's face and ink differ (desktop greys [66,66,66]/[40,40,40], page [43,46,53] with a blue [78,140,255] centre)"),
    ("BTN-A", "events", FOCUS),
    ("TXT-A", "events", "a typed key: the desktop raises onChange/onTextChanged before onKeyDown, the page after onKeyPress (the browser's order)"),
    ("CHK-A", "events", "the page raises no onMouseDown/onMouseUp on a CheckBox (its <label>); the desktop's list repeats TXT-A onLostFocus"),
    ("RAD-A", "events", "the page raises no onMouseDown/onMouseUp on a RadioButton (its <label>)"),
    ("SWT-A", "events", "focus as BTN-A; the page raises onCheckedChanged before onClick, the desktop after"),
    ("LST-A", "events", "focus as BTN-A; the desktop raises the selection change on the press, the page on the press and after onMouseUp"),
    ("CMB-A", "events", "focus as BTN-A; the page raises a second onMouseUp after onClick (the <select> picker)"),
    ("NUD-A", "events", "focus as BTN-A; the desktop raises onKeyPress for the Down arrow, the page (a non-character key) does not"),
    ("SLD-A", "events", "focus as BTN-A; the page's range input moves on the press and on the Down arrow (onChange/onValueChanged), the desktop's does not"),
];

/// `(id, type, x, y, w, h, properties)`.
type Spec = (&'static str, &'static str, i32, i32, i32, i32, &'static [(&'static str, &'static str)]);

const CONTROLS: &[Spec] = &[
    // Class A — scripted.
    ("BTN-A", "Button", 20, 20, 140, 36, &[("Caption", "Press")]),
    ("TXT-A", "TextBox", 180, 20, 180, 30, &[]),
    ("CHK-A", "CheckBox", 380, 20, 160, 28, &[("Caption", "Tick")]),
    ("RAD-A", "RadioButton", 560, 20, 160, 28, &[("Caption", "Pick")]),
    ("SWT-A", "Switch", 740, 20, 80, 32, &[]),
    ("LST-A", "ListBox", 20, 80, 160, 110, &[("Items", "one&#10;two&#10;three")]),
    ("CMB-A", "ComboBox", 200, 80, 160, 30, &[("Items", "one&#10;two&#10;three"), ("DropDownStyle", "DropDownList")]),
    ("NUD-A", "NumericUpDown", 380, 80, 120, 30, &[]),
    ("SLD-A", "Slider", 520, 80, 220, 40, &[]),
    // Class B — fixed data.
    ("CH-BAR", "BarChart", 20, 220, 300, 200, &[("__ChartData", "Q1&#9;10&#10;Q2&#9;20&#10;Q3&#9;15")]),
    ("CH-PIE", "PieChart", 340, 220, 240, 200, &[("__ChartData", "A&#9;30&#10;B&#9;50&#10;C&#9;20")]),
    ("GAU-B", "Gauge", 600, 220, 180, 160, &[("Value", "40")]),
    ("KNB-B", "Knob", 800, 220, 150, 150, &[("Value", "60")]),
    // Class C — through the backend; its box only.
    ("DG-C", "DataGrid", 20, 450, 400, 200, &[]),
];

fn handler(id: &str, event: &str) -> String {
    format!(
        "<Event name=\"{event}\" paragraph=\"{id}--{}\"><![CDATA[       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           DISPLAY \"EV {id} {event}\".\n]]></Event>",
        event.to_ascii_uppercase()
    )
}

fn form() -> String {
    let mut xml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Form name=\"main-form\" title=\"Compare\" width=\"{W}\" height=\"{H}\" index-base=\"1\" main-form=\"true\">\n");
    for (id, ty, x, y, w, h, props) in CONTROLS {
        let t = ControlType::from_str(ty);
        let body: String = props.iter().map(|(k, v)| format!("<Property name=\"{k}\">{v}</Property>")).collect();
        let events: String = if class_of(&t) == Class::A { WANTED.iter().filter(|e| t.supported_events().contains(e)).map(|e| handler(id, e)).collect() } else { String::new() };
        xml += &format!("  <Control id=\"{id}\" type=\"{ty}\" x=\"{x}\" y=\"{y}\" w=\"{w}\" h=\"{h}\">{body}{events}</Control>\n");
    }
    xml + "</Form>\n"
}

/// The key each class A control is given after its click.
fn key_of(ty: &str) -> &'static str {
    if ty == "TextBox" {
        "a"
    } else {
        "ArrowDown"
    }
}

// ── The desktop ─────────────────────────────────────────────────────────────────────────────────────────────────

struct Desk {
    ctx: egui::Context,
    form: cobolt_forms::Form,
    active: ActiveTabs,
    frame: usize,
    rects: HashMap<String, Rect>,
    events: Vec<(String, String)>,
}

impl Desk {
    fn new(form: cobolt_forms::Form) -> Self {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let mut d = Self { ctx, form, active: ActiveTabs::new(), frame: 0, rects: HashMap::new(), events: Vec::new() };
        d.run(vec![]);
        d.run(vec![]);
        d
    }

    fn run(&mut self, events: Vec<egui::Event>) {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(W, H))),
            time: Some(self.frame as f64 * 0.06),
            events,
            focused: true,
            ..Default::default()
        };
        self.frame += 1;
        let (controls, active) = (&self.form.controls, &self.active);
        let (rects, log) = (&mut self.rects, &mut self.events);
        let mut full = self.ctx.run_ui(raw, |root| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(root, |ui| {
                ui.set_min_size(Vec2::new(W, H));
                let input = RenderInput {
                    controls,
                    state: &DesignedState,
                    form_size: Vec2::new(W, H),
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: active,
                    backdrop: Backdrop::default(),
                };
                let out = render_form(ui, &input);
                rects.extend(out.control_rects.into_iter());
                log.extend(out.events.iter().map(|e| (e.ctrl_id.clone(), e.event.clone())));
            });
        });
        full.textures_delta.clear();
    }

    fn click(&mut self, at: Pos2) {
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        for e in [vec![egui::Event::PointerMoved(at)], vec![button(true)], vec![button(false)], vec![]] {
            self.run(e);
        }
    }

    fn key(&mut self, key: &str) {
        let k = if key == "a" { egui::Key::A } else { egui::Key::ArrowDown };
        let ev = |pressed| egui::Event::Key { key: k, physical_key: None, pressed, repeat: false, modifiers: Default::default() };
        let mut down = vec![ev(true)];
        if key == "a" {
            down.push(egui::Event::Text("a".into()));
        }
        for e in [down, vec![ev(false)], vec![], vec![]] {
            self.run(e);
        }
    }
}

// ── The page ────────────────────────────────────────────────────────────────────────────────────────────────────

const LIB: &str = "const root = document.getElementById('pr-app').shadowRoot; const el = id => root.querySelector('[data-name=\"' + id + '\"]'); \
    const surface = () => root.querySelector('[data-pr-form=\"main-form\"] .pr-surface').getBoundingClientRect();";

fn settle(page: &mut cdp::Page) {
    page.eval("new Promise(r => setTimeout(() => requestAnimationFrame(() => requestAnimationFrame(() => r(true))), 120))").unwrap();
}

fn page_events(page: &mut cdp::Page, from: usize) -> Vec<(String, String)> {
    page.console[from..]
        .iter()
        .filter_map(|l| l.split_once("EV ").map(|(_, rest)| rest.trim().to_owned()))
        .filter_map(|r| r.split_once(' ').map(|(a, b)| (a.to_owned(), b.trim().to_owned())))
        .collect()
}

/// The reference form built by the real Build into its own folder (`tag` names it) and served.
fn build_and_serve(tag: &str, xml: &str) -> (std::path::PathBuf, devserver::ServerHandle) {
    let dir = std::env::temp_dir().join(format!("prc-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::write(dir.join("Compare.project.toml"), MANIFEST).unwrap();
    std::fs::write(dir.join("webforms/main-form.wcfrm"), xml).unwrap();
    let req = WebBuildRequest::from_manifest(&dir.join("Compare.project.toml")).unwrap();
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
    let server = devserver::start(&report.bundle_dir, &ServeOptions { port: 0, isolation: req.web.isolation, live_reload: false }).unwrap();
    (dir, server)
}

fn open(page: &mut cdp::Page, url: &str) {
    page.resize(1100, 800).unwrap();
    page.navigate(url).unwrap_or_else(|e| panic!("{e}"));
    page.wait_for(&format!("(() => {{ {LIB} return !!el('DG-C'); }})()"), 30)
        .unwrap_or_else(|e| panic!("the page did not start: {e}; console {:?}; exceptions {:?}", page.console, page.exceptions));
    settle(page);
}

#[test]
fn the_reference_form_on_the_desktop_and_in_chrome() {
    let Some(exe) = cdp::find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let xml = form();
    let parsed = cobolt_forms::load_form_from_str(&xml).unwrap_or_else(|e| panic!("{e}"));
    let (dir, mut server) = build_and_serve("compare", &xml);
    let chrome = cdp::Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    open(&mut page, &server.url());

    let mut diffs: Vec<(String, String, String)> = Vec::new();

    // ── Geometry, every control ──
    let mut desk = Desk::new(parsed.clone());
    let s = page.eval(&format!("(() => {{ {LIB} const s = surface(); return [s.x, s.y]; }})()")).unwrap();
    let (sx, sy) = (s[0].as_f64().unwrap(), s[1].as_f64().unwrap());
    println!("{:<8} {:>28} {:>28}", "control", "desktop x y w h", "Chrome x y w h");
    for (id, ..) in CONTROLS {
        let d = desk.rects.get(*id).copied();
        let b = page.eval(&format!("(() => {{ {LIB} const r = el('{id}').getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; }})()")).unwrap();
        let web: Vec<f64> = (0..4).map(|k| b[k].as_f64().unwrap_or(f64::NAN)).collect();
        let web = [web[0] - sx, web[1] - sy, web[2], web[3]];
        let Some(d) = d else {
            diffs.push((id.to_string(), "geometry".into(), format!("the desktop reports no rect; Chrome {web:?}")));
            continue;
        };
        let desk_box = [d.min.x as f64, d.min.y as f64, d.width() as f64, d.height() as f64];
        println!("{id:<8} {:>28} {:>28}", format!("{desk_box:.1?}"), format!("{web:.1?}"));
        if desk_box.iter().zip(&web).any(|(a, b)| (a - b).abs() > 0.5) {
            diffs.push((id.to_string(), "geometry".into(), format!("desktop {desk_box:.1?}, Chrome {web:.1?}")));
        }
    }

    // ── Colours, the class B controls ──
    let picture = render_form_image(&parsed, &SnapshotOptions::default());
    let desk_at = |x: usize, y: usize| {
        let c = picture.pixels[y.min(picture.size[1] - 1) * picture.size[0] + x.min(picture.size[0] - 1)];
        [c.r(), c.g(), c.b()]
    };
    for (id, ty, x, y, w, h, _) in CONTROLS {
        if class_of(&ControlType::from_str(ty)) != Class::B {
            continue;
        }
        let points = [(0.5, 0.5), (0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)];
        let mut worst = 0;
        let mut line = Vec::new();
        for (fx, fy) in points {
            let (px, py) = ((*x as f64 + *w as f64 * fx) as usize, (*y as f64 + *h as f64 * fy) as usize);
            let d = desk_at(px, py);
            let c = page.capture(sx + px as f64, sy + py as f64, 1.0, 1.0).unwrap().2[0];
            let diff = (0..3).map(|k| (d[k] as i32 - c[k] as i32).abs()).max().unwrap();
            worst = worst.max(diff);
            line.push(format!("({px},{py}) {d:?}/{c:?}"));
        }
        println!("{id:<8} colours desktop/Chrome: {} — max {worst}", line.join(" "));
        if worst > 24 {
            diffs.push((id.to_string(), "colour".into(), format!("max {worst} levels: {}", line.join(" "))));
        }
    }

    // ── Events, the class A controls ──
    for (id, ty, ..) in CONTROLS {
        if class_of(&ControlType::from_str(ty)) != Class::A {
            continue;
        }
        let r = desk.rects.get(*id).copied().unwrap_or(Rect::NOTHING);
        let from = desk.events.len();
        desk.click(r.center());
        desk.key(key_of(ty));
        // A program sees only the events it binds (`WANTED` where the type has them); the renderer reports every one.
        let bound = |c: &str, e: &str| WANTED.contains(&e) && CONTROLS.iter().any(|s| s.0 == c && class_of(&ControlType::from_str(s.1)) == Class::A && ControlType::from_str(s.1).supported_events().contains(&e));
        let d: Vec<String> = desk.events[from..].iter().filter(|(c, e)| bound(c, e)).map(|(c, e)| format!("{c} {e}")).collect();

        let at = page.console.len();
        page.click_at(&format!("(() => {{ {LIB} const r = el('{id}').getBoundingClientRect(); return {{ x: r.x, y: r.y, width: r.width, height: r.height }}; }})()")).unwrap();
        settle(&mut page);
        page.press_key(key_of(ty)).unwrap();
        settle(&mut page);
        let w: Vec<String> = page_events(&mut page, at).into_iter().map(|(c, e)| format!("{c} {e}")).collect();
        println!("{id:<8} click + {}:\n           desktop {d:?}\n           Chrome  {w:?}", key_of(ty));
        if d != w {
            diffs.push((id.to_string(), "events".into(), format!("desktop {d:?}, Chrome {w:?}")));
        }
    }

    println!("T33 (Chrome {}): {} controls, {} differences", exe.display(), CONTROLS.len(), diffs.len());
    for (c, what, why) in &diffs {
        let doc = DOCUMENTED.iter().find(|d| d.0 == c && d.1 == what);
        println!("  {c} {what}: {why}{}", doc.map(|d| format!("  [documented: {}]", d.2)).unwrap_or_default());
    }
    let undocumented = diffs.iter().filter(|(c, w, _)| !DOCUMENTED.iter().any(|d| d.0 == c && d.1 == w)).count();
    drop(page);
    server.stop();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(undocumented, 0, "differences outside the documented ones");
}

/// The key that operates a control from the keyboard (WASM-004 T34), and whether it is an input the walk must operate.
fn operate_key(ty: &str) -> Option<&'static str> {
    Some(match ty {
        "Button" => "Enter",
        "TextBox" => "a",
        "CheckBox" | "RadioButton" | "Switch" => " ",
        "ListBox" => "ArrowDown",
        // A closed <select> opens its picker on the Down arrow on macOS; typing an item's first letter selects it everywhere.
        "ComboBox" => "t",
        "NumericUpDown" | "Knob" => "ArrowUp",
        "Slider" => "ArrowRight",
        _ => return None,
    })
}

/// **The reference form by the keyboard alone, and axe-core's audit of the page** (WASM-004 T34: R10, AC7).
/// From an empty point of the page, Tab walks the form; each control reached is operated by its key and must answer —
/// an event its handler DISPLAYs, or (a control with no handler, the Knob) a changed value. axe-core is not part of the
/// repository (PRIME DIRECTIVE): `AXE_CORE` names an `axe.min.js` on the machine; without it the audit says it was
/// skipped. A critical violation fails the test.
#[test]
fn the_reference_form_by_the_keyboard_alone_and_axe() {
    let Some(exe) = cdp::find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let (dir, mut server) = build_and_serve("keyboard", &form());
    let chrome = cdp::Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    open(&mut page, &server.url());

    // ── The walk ──
    page.eval("(() => { const e = new MouseEvent('mousedown', {bubbles: true}); document.body.dispatchEvent(e); if (document.activeElement) document.activeElement.blur(); return true; })()").unwrap();
    let focused = "(() => { const root = document.getElementById('pr-app').shadowRoot; const a = root.activeElement; const c = a && a.closest('[data-name]'); \
        return c ? [c.dataset.name, a.tagName.toLowerCase() + (a.getAttribute('role') ? '[' + a.getAttribute('role') + ']' : '')] : null; })()";
    let mut seen: Vec<String> = Vec::new();
    let mut walk: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    for _ in 0..40 {
        page.press_key("Tab").unwrap();
        settle(&mut page);
        let f = page.eval(focused).unwrap();
        let Some(name) = f[0].as_str().map(str::to_owned) else { continue };
        if seen.last() == Some(&name) {
            continue; // a control with more than one stop (a DataGrid's cells) counts once
        }
        if seen.contains(&name) {
            break; // round the form once
        }
        seen.push(name.clone());
        let ty = CONTROLS.iter().find(|c| c.0 == name).map(|c| c.1).unwrap_or("?");
        let Some(key) = operate_key(ty) else {
            walk.push(format!("{name} ({}): reached", f[1].as_str().unwrap_or("")));
            continue;
        };
        let before_log = page.console.len();
        let before_value = page.eval(&format!("(() => {{ {LIB} const e = el('{name}'); const k = e.querySelector('[aria-valuenow]') || e; return k.getAttribute('aria-valuenow'); }})()")).unwrap();
        if key == "Enter" {
            // As a keyboard sends it: the key code and the carriage return a button activates on.
            page.call("Input.dispatchKeyEvent", serde_json::json!({"type": "keyDown", "key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13, "text": "\r"})).unwrap();
            page.call("Input.dispatchKeyEvent", serde_json::json!({"type": "keyUp", "key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13})).unwrap();
        } else {
            page.press_key(key).unwrap();
        }
        settle(&mut page);
        let events: Vec<String> = page_events(&mut page, before_log).into_iter().filter(|(c, _)| *c == name).map(|(_, e)| e).collect();
        let after_value = page.eval(&format!("(() => {{ {LIB} const e = el('{name}'); const k = e.querySelector('[aria-valuenow]') || e; return k.getAttribute('aria-valuenow'); }})()")).unwrap();
        let answered = events.iter().any(|e| e != "onKeyDown" && e != "onKeyUp" && e != "onKeyPress") || before_value != after_value;
        walk.push(format!("{name} ({}): {key:?} → {}", f[1].as_str().unwrap_or(""), if events.is_empty() { format!("aria-valuenow {before_value} → {after_value}") } else { events.join(", ") }));
        if !answered {
            failures.push(format!("{name}: {key:?} did nothing"));
        }
    }
    println!("T34 keyboard walk ({} stops):", walk.len());
    for w in &walk {
        println!("  {w}");
    }
    let operable: Vec<&str> = CONTROLS.iter().filter(|c| operate_key(c.1).is_some()).map(|c| c.0).collect();
    for id in &operable {
        if !seen.iter().any(|s| s == id) {
            failures.push(format!("{id}: never reached by Tab"));
        }
    }

    // ── axe-core ──
    match std::env::var_os("AXE_CORE").map(std::path::PathBuf::from) {
        None => println!("axe-core SKIPPED: set AXE_CORE to an axe.min.js (it is not part of the repository)"),
        Some(path) => {
            let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            page.eval(&format!("(() => {{ {src}\n; return typeof axe; }})()")).unwrap();
            let version = page.eval("axe.version").unwrap();
            let r = page
                .eval("axe.run(document, { resultTypes: ['violations'] }).then(r => r.violations.map(v => ({ id: v.id, impact: v.impact, nodes: v.nodes.length, help: v.help, targets: v.nodes.slice(0, 4).map(n => n.target.join(' ')) })))")
                .unwrap();
            let vs = r.as_array().cloned().unwrap_or_default();
            println!("axe-core {version}: {} violations", vs.len());
            for v in &vs {
                println!("  [{}] {} ({} nodes): {} — {}", v["impact"].as_str().unwrap_or("?"), v["id"].as_str().unwrap_or("?"), v["nodes"], v["help"].as_str().unwrap_or(""), v["targets"]);
            }
            for v in vs.iter().filter(|v| v["impact"] == "critical") {
                failures.push(format!("axe critical: {} ({} nodes)", v["id"], v["nodes"]));
            }
        }
    }
    drop(page);
    server.stop();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{failures:#?}");
}
