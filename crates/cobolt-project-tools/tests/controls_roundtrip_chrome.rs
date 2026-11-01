// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Every class A and B control, written by a program and read back from the page** (WASM-004 T32: R11, AC2).
//! The browser counterpart of `tests/controls/`' `assert_control`. A Web project with one of each visual class A and
//! B control of `ControlType::ALL` is built by the real Build; three COBOL handlers write, through the generated
//! module and the host API, a distinct value to each control's `X`, `Y`, `Width`, `Height`, `Tooltip` and its own
//! value property, then `Enabled`, then `Visible`. A headless Chrome reads each back from what the page shows (the
//! box, the `title`, the text or the field's value, `disabled`, `display`), and checks that the control is the
//! element the class table (`cobolt_forms::web_class::TABLE`, its `becomes` column) says.
//!
//! Every mismatch is printed per control; the only accepted ones are listed in `DOCUMENTED`, each with its reason.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

#[path = "../../cobolt-web-bundle/tests/support/cdp.rs"]
#[allow(dead_code)]
mod cdp;

use cobolt_forms::model::ControlType;
use cobolt_forms::web_class::{class_of, Class, TABLE};
use cobolt_project_tools::web::devserver::{self, ServeOptions};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildRequest};

const MANIFEST: &str = "[project]\nname = \"Gallery.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
    [files]\nforms = [\"webforms/main-form.wcfrm\"]\n\n[web]\nmulti_memory = true\n";

/// The value property a program writes on each type, and the value written (`None`: the type's own state is not
/// one value the page shows as text or a field — its geometry, tooltip, Enabled and Visible are still checked).
fn value_of(t: &ControlType) -> Option<(&'static str, &'static str)> {
    use ControlType as T;
    Some(match t {
        T::Button => ("Caption", "Pressed"),
        T::TextBox => ("Text", "typed text"),
        T::Label => ("Caption", "label text"),
        T::Hyperlink => ("Caption", "link text"),
        T::CheckBox => ("Caption", "check text"),
        T::RadioButton => ("Caption", "radio text"),
        T::GroupBox => ("Caption", "group text"),
        T::ProgressBar => ("Value", "73"),
        T::DateTimePicker => ("Value", "2026-11-02"),
        T::NumericUpDown => ("Value", "6"),
        T::Slider => ("Value", "70"),
        T::Gauge | T::Knob => ("Value", "47"),
        T::BarChart | T::LineChart | T::PieChart | T::AreaChart | T::ScatterChart | T::DonutChart | T::RadarChart => ("Title", "Chart title"),
        _ => return None,
    })
}

/// The mismatches accepted, by (type, property), each with its reason.
const DOCUMENTED: &[(&str, &str, &str)] = &[
    ("ComboBox", "(element)", "the seeded combo is editable (Editable on, DropDownStyle DropDown): an <input list> with a <datalist>; <select> is DropDownStyle DropDownList's"),
    ("Animator", "(element)", "with no Source there is nothing to play: the <img> is drawn once a Source names an image"),
    ("SideMenu", "X", "the application shell's rail (spec 049) is laid out by the shell, docked at the form's left edge"),
    ("SideMenu", "Y", "the application shell's rail (spec 049) is laid out by the shell, docked at the form's left edge"),
    ("SideMenu", "Width", "the application shell's rail (spec 049) keeps the rail's width"),
    ("SideMenu", "Height", "the application shell's rail (spec 049) runs the form's full height"),
];

struct Placed {
    id: String,
    ty: ControlType,
    x: i32,
    y: i32,
}

fn controls() -> Vec<Placed> {
    let mut out = Vec::new();
    for (i, t) in ControlType::ALL.iter().filter(|t| matches!(class_of(t), Class::A | Class::B) && !t.is_non_visual()).enumerate() {
        let (col, row) = (i % 6, i / 6);
        out.push(Placed { id: format!("C-{}", t.as_str().to_ascii_uppercase()), ty: t.clone(), x: 10 + col as i32 * 230, y: 60 + row as i32 * 130 });
    }
    out
}

fn handler(control: &str, lines: &[String]) -> String {
    format!(
        "<Event name=\"onClick\" paragraph=\"{control}--ONCLICK\"><![CDATA[       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n{}]]></Event>",
        lines.iter().map(|l| format!("           {l}\n")).collect::<String>()
    )
}

/// What phase 1 writes on control `c`: its geometry moved and resized, a tooltip and its value.
fn written(c: &Placed) -> Vec<(&'static str, String)> {
    let mut v = vec![("X", (c.x + 7).to_string()), ("Y", (c.y + 5).to_string()), ("Width", "170".into()), ("Height", "80".into()), ("Tooltip", format!("tip {}", c.id))];
    if let Some((p, val)) = value_of(&c.ty) {
        v.push((p, val.into()));
    }
    v
}

fn form(cs: &[Placed]) -> String {
    let lit = |v: &str| if v.parse::<i64>().is_ok() { v.to_owned() } else { format!("\"{v}\"") };
    let p1: Vec<String> = cs.iter().flat_map(|c| written(c).into_iter().map(move |(p, v)| format!("MOVE {} TO {}::{p}.", lit(&v), c.id))).collect();
    let p2: Vec<String> = cs.iter().map(|c| format!("MOVE 0 TO {}::Enabled.", c.id)).collect();
    let p3: Vec<String> = cs.iter().map(|c| format!("MOVE 0 TO {}::Visible.", c.id)).collect();
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Form name=\"main-form\" title=\"Gallery\" width=\"1400\" height=\"1200\" index-base=\"1\" main-form=\"true\">\n");
    for (i, (id, lines)) in [("BTN-P1", &p1), ("BTN-P2", &p2), ("BTN-P3", &p3)].into_iter().enumerate() {
        xml += &format!("  <Control id=\"{id}\" type=\"Button\" x=\"{}\" y=\"10\" w=\"90\" h=\"30\"><Property name=\"Caption\">{id}</Property>{}</Control>\n", 10 + i as i32 * 100, handler(id, lines));
    }
    xml += "  <Control id=\"LBL-DONE\" type=\"Label\" x=\"320\" y=\"10\" w=\"200\" h=\"24\"><Property name=\"Caption\">ready</Property></Control>\n";
    for c in cs {
        xml += &format!("  <Control id=\"{}\" type=\"{}\" x=\"{}\" y=\"{}\" w=\"180\" h=\"90\"></Control>\n", c.id, c.ty.as_str(), c.x, c.y);
    }
    xml += "</Form>\n";
    // Each phase ends by saying so, so the page knows the frame is drawn.
    for (n, id) in ["BTN-P1", "BTN-P2", "BTN-P3"].iter().enumerate() {
        let marker = format!("           MOVE \"phase {}\" TO LBL-DONE::Caption.\n]]></Event>", n + 1);
        let at = xml.find(&format!("paragraph=\"{id}--ONCLICK\"")).unwrap();
        let end = at + xml[at..].find("]]></Event>").unwrap();
        xml.replace_range(end..end + "]]></Event>".len(), &marker);
    }
    xml
}

/// The tags and roles the class table's `becomes` names (`<button>`, `role=toolbar`).
fn becomes(ty: &ControlType) -> Vec<String> {
    let row = TABLE.iter().find(|r| r.control == ty.as_str()).map(|r| r.becomes).unwrap_or("");
    let mut sels = Vec::new();
    // Each `<tag …>` names its tag; a `role=x` inside one names a role.
    for open in row.split('<').skip(1) {
        let Some((inside, _)) = open.split_once('>') else { continue };
        let tag = inside.split_whitespace().next().unwrap_or("");
        if let Some(role) = inside.split_whitespace().find_map(|w| w.strip_prefix("role=")) {
            sels.push(format!("[role=\"{role}\"]"));
        } else if !tag.is_empty() && tag.chars().all(|c| c.is_ascii_alphanumeric()) {
            sels.push(tag.to_owned());
        }
    }
    if row.contains("SVG") {
        sels.push("svg".to_owned());
    }
    sels
}

const LIB: &str = "const root = document.getElementById('pr-app').shadowRoot; const el = id => root.querySelector('[data-name=\"' + id + '\"]'); \
    const surface = () => root.querySelector('[data-pr-form=\"main-form\"] .pr-surface'); \
    const done = () => el('LBL-DONE') && el('LBL-DONE').textContent;";

/// The page's reading of control `id`: its box against the form's surface, its tooltip, every text and field value it shows.
const READ: &str = "(id, sels) => { const e = el(id); if (!e) return null; const s = surface().getBoundingClientRect(); const r = e.getBoundingClientRect(); \
    const all = [e, ...e.querySelectorAll('*')]; const shadowTexts = []; \
    const values = all.flatMap(n => [n.value, n.getAttribute && n.getAttribute('aria-valuenow'), n.getAttribute && n.getAttribute('aria-label')].filter(v => v != null && v !== '').map(String)); \
    return { x: r.x - s.x, y: r.y - s.y, w: r.width, h: r.height, \
      title: all.map(n => n.getAttribute && n.getAttribute('title')).find(t => t) || null, \
      text: e.textContent, values, \
      disabled: e.classList.contains('pr-disabled') || all.some(n => n.disabled === true || (n.getAttribute && n.getAttribute('aria-disabled') === 'true')), \
      display: getComputedStyle(e).display, hiddenAttr: e.hasAttribute('hidden'), \
      element: sels.map(q => [q, e.matches(q) || !!e.querySelector(q)]) }; }";

#[test]
fn every_class_a_and_b_control_round_trips_its_properties_in_chrome() {
    let Some(exe) = cdp::find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let cs = controls();
    let dir = std::env::temp_dir().join(format!("prc-roundtrip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::write(dir.join("Gallery.project.toml"), MANIFEST).unwrap();
    std::fs::write(dir.join("webforms/main-form.wcfrm"), form(&cs)).unwrap();
    let req = WebBuildRequest::from_manifest(&dir.join("Gallery.project.toml")).unwrap();
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
    let mut server = devserver::start(&report.bundle_dir, &ServeOptions { port: 0, isolation: req.web.isolation, live_reload: false }).unwrap();
    let chrome = cdp::Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1500, 1300).unwrap();
    page.navigate(&server.url()).unwrap_or_else(|e| panic!("{e}"));
    let wait = |page: &mut cdp::Page, want: &str| {
        page.wait_for(&format!("(() => {{ {LIB} return done() === '{want}'; }})()"), 30)
            .unwrap_or_else(|e| panic!("waiting for `{want}`: {e}; console {:?}; exceptions {:?}", page.console, page.exceptions));
        page.eval("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(() => r(true))))").unwrap();
    };
    wait(&mut page, "ready");
    let read = |page: &mut cdp::Page, c: &Placed| {
        let sels = serde_json::to_string(&becomes(&c.ty)).unwrap();
        page.eval(&format!("(() => {{ {LIB} return ({READ})('{}', {sels}); }})()", c.id)).unwrap()
    };
    let click = |page: &mut cdp::Page, id: &str| {
        page.click_at(&format!("(() => {{ {LIB} const r = el('{id}').getBoundingClientRect(); return {{ x: r.x, y: r.y, width: r.width, height: r.height }}; }})()")).unwrap();
    };

    let mut mismatches: Vec<(String, String, String)> = Vec::new();
    let mut checked = 0usize;
    let mut miss = |c: &Placed, prop: &str, why: String| mismatches.push((c.ty.as_str().to_owned(), prop.to_owned(), why));

    // The element first, before any write.
    for c in &cs {
        let r = read(&mut page, c);
        if r.is_null() {
            miss(c, "(element)", "no element on the page".into());
            continue;
        }
        let els = r["element"].as_array().cloned().unwrap_or_default();
        checked += 1;
        if !els.is_empty() && !els.iter().any(|p| p[1] == true) {
            miss(c, "(element)", format!("none of {els:?}"));
        }
    }

    click(&mut page, "BTN-P1");
    wait(&mut page, "phase 1");
    for c in &cs {
        let r = read(&mut page, c);
        if r.is_null() {
            continue;
        }
        for (p, v) in written(c) {
            checked += 1;
            let num = |k: &str| r[k].as_f64().unwrap_or(f64::NAN);
            let ok = match p {
                "X" => (num("x") - v.parse::<f64>().unwrap()).abs() < 0.6,
                "Y" => (num("y") - v.parse::<f64>().unwrap()).abs() < 0.6,
                "Width" => (num("w") - 170.0).abs() < 0.6,
                "Height" => (num("h") - 80.0).abs() < 0.6,
                "Tooltip" => r["title"].as_str() == Some(v.as_str()),
                _ => r["text"].as_str().is_some_and(|t| t.contains(v.as_str())) || r["values"].as_array().is_some_and(|a| a.iter().any(|x| x.as_str() == Some(v.as_str()))),
            };
            if !ok {
                let seen = match p {
                    "X" => r["x"].to_string(),
                    "Y" => r["y"].to_string(),
                    "Width" => r["w"].to_string(),
                    "Height" => r["h"].to_string(),
                    "Tooltip" => r["title"].to_string(),
                    _ => format!("text {:?}, values {}", r["text"].as_str().unwrap_or("").chars().take(60).collect::<String>(), r["values"]),
                };
                miss(c, p, format!("wrote {v}, the page shows {seen}"));
            }
        }
    }

    click(&mut page, "BTN-P2");
    wait(&mut page, "phase 2");
    for c in &cs {
        let r = read(&mut page, c);
        if r.is_null() {
            continue;
        }
        checked += 1;
        if r["disabled"] != true {
            miss(c, "Enabled", "wrote 0, the page shows it enabled".into());
        }
    }

    click(&mut page, "BTN-P3");
    wait(&mut page, "phase 3");
    for c in &cs {
        let r = read(&mut page, c);
        if r.is_null() {
            continue;
        }
        checked += 1;
        if r["display"] != "none" && r["hiddenAttr"] != true {
            miss(c, "Visible", format!("wrote 0, the page shows display {}", r["display"]));
        }
    }

    println!("T32 (Chrome {}): {} class A and B controls, {checked} checks, {} mismatches", exe.display(), cs.len(), mismatches.len());
    for (t, p, why) in &mismatches {
        let documented = DOCUMENTED.iter().find(|d| d.0 == t && d.1 == p);
        println!("  {t}.{p}: {why}{}", documented.map(|d| format!("  [documented: {}]", d.2)).unwrap_or_default());
    }
    let undocumented: Vec<_> = mismatches.iter().filter(|(t, p, _)| !DOCUMENTED.iter().any(|d| d.0 == t && d.1 == p)).collect();
    drop(page);
    server.stop();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(undocumented.is_empty(), "{} mismatches outside the documented differences", undocumented.len());
}
