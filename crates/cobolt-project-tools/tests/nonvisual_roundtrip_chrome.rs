// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The two non-visual class A controls, written by a program and read back from the page** (WASM-015 AC4, WASM-004
//! R11). `controls_roundtrip_chrome` takes the visual class A and B controls; a Timer and a Snackbar draw no element of
//! their own, so what the page *shows* of them is what they *do*, and both do it in the page: no Backend is involved.
//!
//! A Web project with one Timer and one Snackbar is built by the real Build. Buttons run COBOL handlers that write the
//! controls' properties through the generated module; a headless Chrome reads what happened on the page:
//!
//! * **Timer** — `Enabled` designed off: no tick. A program writes `Interval` 100 and `Enabled` 1: the `onTick` handler
//!   runs, and its count, written to a NumericUpDown, changes about every 100 ms. It writes `Interval` 400:
//!   the gap grows to about 400 ms. It writes `Enabled` 0: the ticks stop.
//! * **Snackbar** — a program writes `Text`, `Category`, `Timeout`, `Buttons` and `CornerRadius`, then calls `Show`: a
//!   notification shows with that text, that category (`data-category`, and `role` alert for an error), that button, those
//!   corners; with `Timeout` 0 it stays. A second `Show` with other values, `Timeout` 600 and two corners of its own: it
//!   shows beside the first, with its own text, category, corners and no button, and goes away on its own while the first
//!   stays.
//!
//! The timings are wide bands (a tick of 100 ms is accepted from 90 to 260), because a page in a loaded machine is late,
//! never early; the *order* of the three intervals is the claim.
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

#[path = "../../cobolt-web-bundle/tests/support/cdp.rs"]
#[allow(dead_code)]
mod cdp;

use cobolt_project_tools::web::devserver::{self, ServeOptions};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildRequest};

const MANIFEST: &str = "[project]\nname = \"Gallery.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
    [files]\nforms = [\"webforms/main-form.wcfrm\"]\n\n[web]\nmulti_memory = true\n";

/// One `<Event>` whose handler is `lines` (the last of them says which phase ran).
fn event(name: &str, paragraph: &str, lines: &[&str]) -> String {
    format!(
        "<Event name=\"{name}\" paragraph=\"{paragraph}\"><![CDATA[       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n{}]]></Event>",
        lines.iter().map(|l| format!("           {l}\n")).collect::<String>()
    )
}

/// The Timer's `onTick`: the handler keeps its own count (a program's WORKING-STORAGE lives between calls) and writes it to
/// the NumericUpDown, so each tick is a change the page shows.
fn tick_event() -> String {
    "<Event name=\"onTick\" paragraph=\"TMR-1--ONTICK\"><![CDATA[       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 WS-N PIC 9(6) VALUE 0.\n       PROCEDURE DIVISION.\n           ADD 1 TO WS-N.\n           MOVE WS-N TO NUD-TICKS::Value.\n]]></Event>".to_owned()
}

fn button(id: &str, x: i32, lines: &[&str], marker: &str) -> String {
    let mut all: Vec<String> = lines.iter().map(|l| (*l).to_owned()).collect();
    all.push(format!("MOVE \"{marker}\" TO LBL-DONE::Caption."));
    let refs: Vec<&str> = all.iter().map(String::as_str).collect();
    format!(
        "  <Control id=\"{id}\" type=\"Button\" x=\"{x}\" y=\"10\" w=\"90\" h=\"30\"><Property name=\"Caption\">{id}</Property>{}</Control>\n",
        event("onClick", &format!("{id}--ONCLICK"), &refs)
    )
}

fn form() -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Form name=\"main-form\" title=\"Gallery\" width=\"1000\" height=\"600\" index-base=\"1\" main-form=\"true\">\n");
    xml += &button("BTN-T1", 10, &["MOVE 100 TO TMR-1::Interval.", "MOVE 1 TO TMR-1::Enabled."], "t1");
    xml += &button("BTN-T2", 110, &["MOVE 400 TO TMR-1::Interval."], "t2");
    xml += &button("BTN-T3", 210, &["MOVE 0 TO TMR-1::Enabled."], "t3");
    xml += &button(
        "BTN-S1",
        310,
        &[
            "MOVE \"Saved the order\" TO SNK-1::Text.",
            "MOVE \"Error\" TO SNK-1::Category.",
            "MOVE 0 TO SNK-1::Timeout.",
            "MOVE \"undo|Undo\" TO SNK-1::Buttons.",
            "MOVE 20 TO SNK-1::CornerRadius.",
            "INVOKE SNK-1 \"Show\".",
        ],
        "s1",
    );
    xml += &button(
        "BTN-S2",
        410,
        &[
            "MOVE \"Second note\" TO SNK-1::Text.",
            "MOVE \"Info\" TO SNK-1::Category.",
            "MOVE 600 TO SNK-1::Timeout.",
            "MOVE \"\" TO SNK-1::Buttons.",
            "MOVE 2 TO SNK-1::CornerRadiusTopLeft.",
            "MOVE 30 TO SNK-1::CornerRadiusBottomRight.",
            "INVOKE SNK-1 \"Show\".",
        ],
        "s2",
    );
    xml += "  <Control id=\"LBL-DONE\" type=\"Label\" x=\"520\" y=\"10\" w=\"200\" h=\"24\"><Property name=\"Caption\">ready</Property></Control>\n";
    xml += "  <Control id=\"NUD-TICKS\" type=\"NumericUpDown\" x=\"10\" y=\"60\" w=\"120\" h=\"30\"><Property name=\"Value\">0</Property></Control>\n";
    // The Timer is designed off (`Enabled` false) with a slow Interval: it does nothing until a program writes it.
    xml += &format!(
        "  <Control id=\"TMR-1\" type=\"Timer\" x=\"150\" y=\"60\" w=\"24\" h=\"24\" enabled=\"true\"><Property name=\"Interval\">1000</Property><Property name=\"Enabled\">false</Property>{}</Control>\n",
        tick_event()
    );
    xml += "  <Control id=\"SNK-1\" type=\"Snackbar\" x=\"200\" y=\"60\" w=\"24\" h=\"24\"></Control>\n";
    xml += "</Form>\n";
    xml
}

const LIB: &str = "const root = document.getElementById('pr-app').shadowRoot; const el = id => root.querySelector('[data-name=\"' + id + '\"]'); \
    const done = () => el('LBL-DONE') && el('LBL-DONE').textContent;";

/// Every notification on the page: its text, category, role, four corners and buttons.
const TOASTS: &str = "(() => { const root = document.getElementById('pr-app').shadowRoot; \
    return [...root.querySelectorAll('.pr-snackbar')].map(t => { const s = getComputedStyle(t); \
      return { text: (t.querySelector('.pr-snack-text') || {}).textContent || '', category: t.getAttribute('data-category'), role: t.getAttribute('role'), \
               radius: [s.borderTopLeftRadius, s.borderTopRightRadius, s.borderBottomRightRadius, s.borderBottomLeftRadius].join(' '), \
               buttons: [...t.querySelectorAll('[data-part=\"toast-button\"]')].map(b => [b.getAttribute('data-id'), b.textContent]) }; }); })()";

/// The median of the gaps between consecutive tick times (milliseconds), or `None` with fewer than two ticks.
fn median_gap(times: &[f64]) -> Option<f64> {
    let mut gaps: Vec<f64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    if gaps.is_empty() {
        return None;
    }
    gaps.sort_by(|a, b| a.total_cmp(b));
    Some(gaps[gaps.len() / 2])
}

#[test]
fn timer_and_snackbar_round_trip_their_properties_in_chrome() {
    let Some(exe) = cdp::find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let dir = std::env::temp_dir().join(format!("prc-nonvisual-roundtrip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::write(dir.join("Gallery.project.toml"), MANIFEST).unwrap();
    std::fs::write(dir.join("webforms/main-form.wcfrm"), form()).unwrap();
    let req = WebBuildRequest::from_manifest(&dir.join("Gallery.project.toml")).unwrap();
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("{e}"));
    let mut server = devserver::start(&report.bundle_dir, &ServeOptions { port: 0, isolation: req.web.isolation, live_reload: false }).unwrap();
    let chrome = cdp::Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1200, 700).unwrap();
    page.navigate(&server.url()).unwrap_or_else(|e| panic!("{e}"));
    let wait = |page: &mut cdp::Page, want: &str| {
        page.wait_for(&format!("(() => {{ {LIB} return done() === '{want}'; }})()"), 30)
            .unwrap_or_else(|e| panic!("waiting for `{want}`: {e}; console {:?}; exceptions {:?}", page.console, page.exceptions));
        page.eval("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(() => r(true))))").unwrap();
    };
    let click = |page: &mut cdp::Page, id: &str| {
        page.click_at(&format!("(() => {{ {LIB} const r = el('{id}').getBoundingClientRect(); return {{ x: r.x, y: r.y, width: r.width, height: r.height }}; }})()")).unwrap();
    };
    // A frame sampler: each time the NumericUpDown's field changes, its time. A tick is the handler's `ADD 1`.
    let sample = |page: &mut cdp::Page| {
        page.eval(&format!(
            "(() => {{ {LIB} window.__ticks = []; const f = el('NUD-TICKS').querySelector('input'); let last = f.value; \
             const loop = () => {{ if (f.value !== last) {{ last = f.value; window.__ticks.push(performance.now()); }} requestAnimationFrame(loop); }}; \
             requestAnimationFrame(loop); return true; }})()"
        ))
        .unwrap();
    };
    let ticks = |page: &mut cdp::Page, clear: bool| -> Vec<f64> {
        let v = page.eval(&format!("(() => {{ const t = window.__ticks.slice(); if ({clear}) window.__ticks.length = 0; return t; }})()")).unwrap();
        v.as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).collect()).unwrap_or_default()
    };
    let sleep = |page: &mut cdp::Page, ms: u32| {
        page.eval(&format!("new Promise(r => setTimeout(() => r(true), {ms}))")).unwrap();
    };

    let mut checked = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let mut check = |ok: bool, what: String| {
        checked += 1;
        println!("  {} {what}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            failures.push(what);
        }
    };

    wait(&mut page, "ready");
    sample(&mut page);

    // ── Timer ───────────────────────────────────────────────────────────────────────────────────────────────────────
    sleep(&mut page, 1300);
    let idle = ticks(&mut page, true);
    check(idle.is_empty(), format!("Timer designed Enabled=false, Interval=1000: no tick in 1.3 s ({} seen)", idle.len()));

    click(&mut page, "BTN-T1");
    wait(&mut page, "t1");
    sleep(&mut page, 1700);
    let fast = ticks(&mut page, true);
    let fast_gap = median_gap(&fast);
    check(fast.len() >= 5, format!("Timer written Interval=100, Enabled=1: it ticks ({} ticks in 1.7 s, at least 5)", fast.len()));
    check(fast_gap.is_some_and(|g| (90.0..=260.0).contains(&g)), format!("… the ticks are about 100 ms apart (median gap {fast_gap:?} ms, in 90..=260)"));

    click(&mut page, "BTN-T2");
    wait(&mut page, "t2");
    sleep(&mut page, 300);
    ticks(&mut page, true);
    sleep(&mut page, 2600);
    let slow = ticks(&mut page, true);
    let slow_gap = median_gap(&slow);
    check(slow.len() >= 4, format!("Timer written Interval=400: it still ticks ({} ticks in 2.6 s, at least 4)", slow.len()));
    check(slow_gap.is_some_and(|g| (390.0..=700.0).contains(&g)), format!("… the ticks are about 400 ms apart (median gap {slow_gap:?} ms, in 390..=700)"));
    check(matches!((fast_gap, slow_gap), (Some(f), Some(s)) if s > f * 1.8), format!("… the 400 ms gap is more than 1.8 times the 100 ms gap ({fast_gap:?} → {slow_gap:?})"));

    click(&mut page, "BTN-T3");
    wait(&mut page, "t3");
    sleep(&mut page, 500);
    ticks(&mut page, true);
    sleep(&mut page, 1300);
    let stopped = ticks(&mut page, true);
    check(stopped.is_empty(), format!("Timer written Enabled=0: the ticks stop (none in the next 1.3 s, {} seen)", stopped.len()));

    // ── Snackbar ────────────────────────────────────────────────────────────────────────────────────────────────────
    click(&mut page, "BTN-S1");
    wait(&mut page, "s1");
    let first = page.eval(TOASTS).unwrap();
    let first = first.as_array().cloned().unwrap_or_default();
    check(first.len() == 1, format!("Snackbar written and Show()n: one notification on the page ({})", first.len()));
    let t1 = first.first().cloned().unwrap_or_default();
    check(t1["text"] == "Saved the order", format!("… Text: {}", t1["text"]));
    check(t1["category"] == "Error" && t1["role"] == "alert", format!("… Category=Error: data-category {}, role {}", t1["category"], t1["role"]));
    check(t1["radius"] == "20px 20px 20px 20px", format!("… CornerRadius=20: {}", t1["radius"]));
    check(t1["buttons"] == serde_json::json!([["undo", "Undo"]]), format!("… Buttons=\"undo|Undo\": {}", t1["buttons"]));

    click(&mut page, "BTN-S2");
    wait(&mut page, "s2");
    let both = page.eval(TOASTS).unwrap();
    let both = both.as_array().cloned().unwrap_or_default();
    let second = both.iter().find(|t| t["text"] == "Second note").cloned().unwrap_or_default();
    check(both.len() == 2, format!("Snackbar written again and Show()n: two notifications ({})", both.len()));
    check(second["category"] == "Info" && second["role"] == "status", format!("… second Category=Info: data-category {}, role {}", second["category"], second["role"]));
    check(second["radius"] == "2px 20px 30px 20px", format!("… second corners (2 top-left, 30 bottom-right, 20 elsewhere): {}", second["radius"]));
    check(second["buttons"] == serde_json::json!([]), format!("… second Buttons=\"\": {}", second["buttons"]));

    sleep(&mut page, 1800);
    let later = page.eval(TOASTS).unwrap();
    let later = later.as_array().cloned().unwrap_or_default();
    check(later.len() == 1 && later[0]["text"] == "Saved the order", format!("Timeout: the Timeout=600 notification went away, the Timeout=0 one stays ({} left: {})", later.len(), later.iter().map(|t| t["text"].to_string()).collect::<Vec<_>>().join(", ")));

    println!("AC4 non-visual (Chrome {}): Timer and Snackbar, {checked} checks, {} failed", exe.display(), failures.len());
    drop(page);
    server.stop();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{} checks failed:\n{}", failures.len(), failures.join("\n"));
}
