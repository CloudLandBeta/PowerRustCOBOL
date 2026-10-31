// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The border look drawn in CSS, against the desktop's picture** (WASM-004 T13b).
//!
//! A chart's frame has three things of its own on the desktop (`paint::draw_chart_border`): a two-colour gradient
//! along a compass direction, a blur outward, and a `BorderTransparency` that fades the frame alone. A display control
//! tilted by spec 096 converges by its `PerspectiveStrength` (spec 097). The page draws all four in CSS; this test puts
//! them on the fixture's main form, opens it in a headless Chrome, and samples the same points of the page and of the
//! desktop's own picture of the form (`cobolt_forms::snapshot::render_form_image`).
//!
//! With no Chrome on the machine (`CHROME` names one) the test says it was skipped, and why.

mod support;

use cobolt_wasm_abi::fixture::standard::{LBL_OUT, MAIN_FORM};
use cobolt_web_bundle::devserver::{serve, ServeOptions};
use cobolt_web_bundle::{assemble_bundle, BundleOptions, Isolation, PlayerInput};
use support::cdp::{find_chrome, Chrome};

/// Three charts and a tilted label right of the fixture's own controls (which sit left of x = 230).
fn look_form(plain: &str) -> String {
    assert!(plain.contains("width=\"640\" height=\"480\""), "the fixture's form is 640 x 480");
    let sized = plain.replace("width=\"640\" height=\"480\"", "width=\"900\" height=\"600\" index-base=\"1\"");
    let p = |k: &str, v: &str| format!("<Property name=\"{k}\">{v}</Property>");
    let chart = |id: &str, y: i32, h: i32, props: &[(&str, &str)]| {
        let body: String = props.iter().map(|(k, v)| p(k, v)).collect();
        format!("  <Control id=\"{id}\" type=\"BarChart\" x=\"250\" y=\"{y}\" w=\"300\" h=\"{h}\">{body}</Control>\n")
    };
    let controls = [
        chart("CH-GRAD", 40, 160, &[("BorderStyle", "Single"), ("BorderWidth", "6"), ("CornerRadius", "12"), ("BorderGradientEnabled", "True"), ("BorderGradientStartColor", "#FF0000"), ("BorderGradientEndColor", "#0000FF"), ("BorderGradientDirection", "East")]),
        chart("CH-GLOW", 260, 100, &[("BorderStyle", "Single"), ("BorderWidth", "2"), ("BorderColor", "#00C000"), ("BorderBlur", "12")]),
        chart("CH-FADE", 420, 100, &[("BorderStyle", "Single"), ("BorderWidth", "8"), ("BorderColor", "#FF0000"), ("BorderTransparency", "50"), ("Transparency", "0"), ("BackgroundColor", "#102040")]),
        format!(
            "  <Control id=\"LBL-TILT\" type=\"Label\" x=\"620\" y=\"60\" w=\"200\" h=\"120\">{}{}{}{}</Control>\n",
            p("BackgroundColor", "#FFC000"),
            p("RotateY", "35"),
            p("PerspectiveDistance", "70"),
            p("PerspectiveStrength", "50")
        ),
    ]
    .concat();
    sized.replace("</Form>", &format!("{controls}</Form>"))
}

/// The JavaScript every probe starts with: the shadow root and a control by name.
const LIB: &str = "const root = document.getElementById('pr-app').shadowRoot; const el = id => root.querySelector('[data-name=\"' + id + '\"]');";

#[test]
fn a_charts_gradient_frame_blur_and_fade_and_a_tilts_strength_are_drawn_as_the_desktop_draws_them() {
    let Some(exe) = find_chrome() else {
        println!("SKIPPED: no Chrome was found (set CHROME to a Chrome or Chromium executable to run this test)");
        return;
    };
    let player_out = support::built_player();
    let mut input = support::input_with(
        PlayerInput { plain: Some(support::player_variant(&player_out, "plain")), parts: support::player_parts(&player_out), ..Default::default() },
        BundleOptions { isolation: Isolation::RequireCorp, ..Default::default() },
    );
    let mut cfrm = String::new();
    for f in input.forms.iter_mut().filter(|f| f.name == MAIN_FORM) {
        f.cfrm = look_form(&f.cfrm);
        cfrm = f.cfrm.clone();
    }
    let out = assemble_bundle(&input).unwrap_or_else(|e| panic!("{e}"));
    let dir = tempfile::tempdir().unwrap();
    support::write_bundle(&out, dir.path());
    let server = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
    let chrome = Chrome::launch(&exe).unwrap_or_else(|e| panic!("{e}"));
    let mut page = chrome.new_page().unwrap_or_else(|e| panic!("{e}"));
    page.resize(1100, 700).unwrap();
    page.navigate(&format!("{}/index.html", server.url())).unwrap();
    page.wait_for(&format!("(() => {{ {LIB} const l = el('{LBL_OUT}'); return !!l && l.textContent === 'loaded' && !!el('CH-GRAD'); }})()"), 30)
        .unwrap_or_else(|e| panic!("the application did not start: {e}"));

    // ── The computed styles: each property reached the page ────────────────────────────────────────────────────
    let styles = page
        .eval(&format!(
            "(() => {{ {LIB} const a = getComputedStyle(el('CH-GRAD'), '::after'); const g = getComputedStyle(el('CH-GRAD')); \
             return {{ ring: a.backgroundImage, mask: a.maskComposite, radius: a.borderTopLeftRadius, border: g.borderTopColor, \
             glow: getComputedStyle(el('CH-GLOW')).boxShadow, fade: getComputedStyle(el('CH-FADE')).borderTopColor, \
             tilt: getComputedStyle(el('LBL-TILT')).transform }}; }})()"
        ))
        .unwrap();
    println!("computed: {styles}");
    // ── The desktop's picture of the same form ─────────────────────────────────────────────────────────────────
    let form = cobolt_forms::xml::load_form_from_str(&cfrm).unwrap_or_else(|e| panic!("{e}"));
    let desk = cobolt_forms::snapshot::render_form_image(&form, &cobolt_forms::snapshot::SnapshotOptions::default());
    let desk_at = |x: usize, y: usize| {
        let c = desk.pixels[y * desk.size[0] + x];
        [c.r(), c.g(), c.b()]
    };
    // The page's form origin: CH-GRAD sits at (250, 40) of the form.
    let origin = page.eval(&format!("(() => {{ {LIB} const r = el('CH-GRAD').getBoundingClientRect(); return [r.left - 250, r.top - 40]; }})()")).unwrap();
    let (ox, oy) = (origin[0].as_f64().unwrap(), origin[1].as_f64().unwrap());    let mut web_at = |x: usize, y: usize| page.capture(ox + x as f64, oy + y as f64, 1.0, 1.0).unwrap().2[0];

    // Points: the gradient frame's left, middle and right of its top band and its left and right bands; the glow 4 px
    // outside CH-GLOW's left edge; the faded frame's band; the form's background far from everything.
    let points: [(&str, usize, usize); 9] = [
        ("chart face (stated), inside CH-FADE", 262, 432),
        ("gradient, left band", 253, 120),
        ("gradient, top band middle", 400, 43),
        ("gradient, right band", 546, 120),
        ("gradient, top band near the right", 520, 43),
        ("glow, 4 px left of the frame", 246, 310),
        ("glow, 4 px above the frame", 400, 256),
        ("faded frame, left band", 254, 470),
        ("form background", 880, 580),
    ];
    println!("{:<36} {:>16} {:>16} {:>6}", "point (form x, y)", "desktop", "Chrome", "max d");
    let mut worst = 0;
    for (what, x, y) in points {
        let (d, w) = (desk_at(x, y), web_at(x, y));
        let diff = (0..3).map(|k| (d[k] as i32 - w[k] as i32).abs()).max().unwrap();
        println!("{:<36} {:>16} {:>16} {:>6}", format!("{what} ({x}, {y})"), format!("{d:?}"), format!("{w:?}"), diff);
        worst = worst.max(diff);
    }
    assert_eq!(styles["ring"], "linear-gradient(90deg, rgb(255, 0, 0), rgb(0, 0, 255))");
    assert_eq!(styles["mask"], "exclude, exclude", "one composite per mask layer");
    assert_eq!(styles["radius"], "12px");
    assert_eq!(styles["border"], "rgba(0, 0, 0, 0)", "the element's own border is transparent under the ring");
    assert!(styles["glow"].as_str().unwrap_or("").contains("12px"), "the blur reaches the box-shadow: {}", styles["glow"]);
    assert_eq!(styles["fade"], "rgba(255, 0, 0, 0.5)", "BorderTransparency 50 halves the frame's alpha");
    assert!(styles["tilt"].as_str().unwrap_or("").starts_with("matrix3d("), "a tilt is a 3D transform: {}", styles["tilt"]);

    // The frame bands, the faded frame over the chart's face and the glow's rings (laid as the desktop's stack sums
    // them) agree with the desktop's picture within a little anti-aliasing.
    assert!(worst <= 24, "a point differs by {worst} levels from the desktop's");

    // ── The tilt's outline: the desktop's projected quad against the page's bounding box ───────────────────────
    let label = form.controls.iter().find(|c| c.id == "LBL-TILT").unwrap();
    let rect = egui::Rect::from_min_size(egui::pos2(label.rect.x as f32, label.rect.y as f32), egui::vec2(label.rect.w as f32, label.rect.h as f32));
    let quad = cobolt_forms::perspective::Perspective::of(label).expect("the label is tilted").quad(rect);
    let (dl, dr) = (quad.iter().map(|p| p.x).fold(f32::MAX, f32::min), quad.iter().map(|p| p.x).fold(f32::MIN, f32::max));
    let (dt, db) = (quad.iter().map(|p| p.y).fold(f32::MAX, f32::min), quad.iter().map(|p| p.y).fold(f32::MIN, f32::max));
    let b = page.eval(&format!("(() => {{ {LIB} const r = el('LBL-TILT').getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom]; }})()")).unwrap();
    let web: Vec<f64> = (0..4).map(|k| b[k].as_f64().unwrap()).collect();
    let (wl, wt, wr, wb) = (web[0] - ox, web[1] - oy, web[2] - ox, web[3] - oy);
    println!("tilt outline, desktop: left {dl:.2} top {dt:.2} right {dr:.2} bottom {db:.2}");
    println!("tilt outline, Chrome:  left {wl:.2} top {wt:.2} right {wr:.2} bottom {wb:.2}");
    for (d, w) in [(dl, wl), (dt, wt), (dr, wr), (db, wb)] {
        assert!((d as f64 - w).abs() <= 1.5, "the page's outline is the desktop's: {d} against {w}");
    }
    drop(server);
}
