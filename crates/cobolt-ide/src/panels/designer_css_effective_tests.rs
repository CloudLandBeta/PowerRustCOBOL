// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-036 T9 — the designer's canvas paints what the page's stylesheets decide (R6, R7, R8,
//! R19, R20; AC4, AC5, AC12). Real frames of the real `DesignerPanel::show`; the colour read back is
//! the fill the frame painted.

use super::*;
use crate::llm::LlmConfig;
use cobolt_forms::css::Scheme;
use cobolt_forms::{Form, PropValue};

const SCREEN: Vec2 = Vec2::new(1400.0, 1000.0);
const SHEET: &str = "#PNL-1 { background-color: rgb(10 132 255) }\n@media (prefers-color-scheme: dark) { #PNL-1 { background-color: rgb(171 52 18) } }\n";

fn flatten(shape: &egui::Shape, out: &mut Vec<egui::Shape>) {
    match shape {
        egui::Shape::Vec(v) => v.iter().for_each(|s| flatten(s, out)),
        s => out.push(s.clone()),
    }
}

fn form() -> Form {
    let mut f = Form::new("F", "F", 400, 300);
    let mut p = Control::new("PNL-1", ControlType::Panel, 20, 20);
    p.rect = cobolt_forms::model::Rect::new(20, 20, 160, 80);
    p.set_prop("Transparency", PropValue::String("50".into()));
    f.controls.push(p);
    f
}

fn designer(web: bool, sheet: &str, scheme: Scheme) -> DesignerPanel {
    let mut d = DesignerPanel::new(form());
    d.properties.web_project = web;
    d.properties.css_sheets = vec![("webforms/F.css".into(), sheet.to_owned())];
    d.css_scheme = scheme;
    d
}

/// `frames` frames of `d`; the shapes of the last.
fn frames(d: &mut DesignerPanel, ctx: &egui::Context, frames: usize) -> Vec<egui::Shape> {
    let llm = LlmConfig::load_defaults_for_test();
    let mut shapes = Vec::new();
    for t in 0..frames {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, SCREEN));
        input.max_texture_side = Some(8192);
        input.time = Some(t as f64 * 0.05);
        let mut out = ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show_inside(root, |ui| {
                let _ = d.show(ui, &mut None, &[], &llm, None, None);
            });
        });
        out.textures_delta.clear();
        shapes.clear();
        out.shapes.iter().for_each(|c| flatten(&c.shape, &mut shapes));
    }
    shapes
}

/// The fill of the panel's face: the rectangle at the panel's place on the canvas whose fill is opaque.
fn panel_fill(d: &DesignerPanel, shapes: &[egui::Shape]) -> Option<Color32> {
    let origin = d.canvas_origin?;
    shapes.iter().find_map(|s| match s {
        egui::Shape::Rect(r) if r.fill.a() == 255 && (r.rect.min - (origin + Vec2::new(20.0, 20.0))).length() < 1.0 && (r.rect.width() - 160.0).abs() < 1.0 => Some(r.fill),
        _ => None,
    })
}

fn close(got: Color32, want: [u8; 3]) -> bool {
    got.to_array()[..3].iter().zip(want).all(|(g, w)| (*g as i32 - w as i32).abs() <= 1)
}

#[test]
fn the_canvas_paints_the_stylesheets_colour_in_each_scheme_and_a_desktop_form_keeps_its_properties() {
    let t0 = std::time::Instant::now();
    let mut cases = 0;
    let ctx = egui::Context::default();
    let mut light = designer(true, SHEET, Scheme::Light);
    let s = frames(&mut light, &ctx, 3);
    let got = panel_fill(&light, &s).expect("the panel's face is painted");
    assert!(close(got, [10, 132, 255]), "Light: the rule's colour, unfaded by Transparency 50: {got:?}");
    cases += 1;

    let ctx = egui::Context::default();
    let mut dark = designer(true, SHEET, Scheme::Dark);
    let s = frames(&mut dark, &ctx, 3);
    let got = panel_fill(&dark, &s).expect("the panel's face is painted");
    assert!(close(got, [171, 52, 18]), "Dark: the @media rule's colour: {got:?}");
    cases += 1;

    // AC12: the engine's picture of the same form and sheets has the canvas's pixel.
    for (scheme, want) in [(Scheme::Light, [10u8, 132, 255]), (Scheme::Dark, [171, 52, 18])] {
        let opts = cobolt_forms::snapshot::SnapshotOptions {
            sheets: vec![cobolt_forms::snapshot::SnapshotSheet { label: "webforms/F.css".into(), text: SHEET.into(), origin: cobolt_forms::css_effective::Origin::Form }],
            color_scheme: scheme,
            ..Default::default()
        };
        let img = cobolt_forms::snapshot::render_form_image(&form(), &opts);
        let px = img.pixels[60 * img.size[0] + 100].to_srgba_unmultiplied();
        assert!(close(Color32::from_rgba_unmultiplied(px[0], px[1], px[2], px[3]), want), "{scheme:?}: the picture's pixel is the canvas's: {px:?}");
        cases += 1;
    }

    // R19: a Desktop form with a stylesheet beside it paints the stored properties.
    let ctx = egui::Context::default();
    let mut desktop = designer(false, SHEET, Scheme::Light);
    let s = frames(&mut desktop, &ctx, 3);
    let faded = panel_fill(&desktop, &s);
    assert!(faded.is_none_or(|c| !close(c, [10, 132, 255])), "a Desktop form is not restyled: {faded:?}");
    cases += 1;
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("┌─ designer_css_effective: the canvas paints the stylesheet");
    println!("│ cases: Panel at Transparency 50, #PNL-1 rule + @media dark; Light, Dark, engine picture in both, Desktop unchanged");
    println!("│ time: {ms:.0} ms for {cases} cases ({:.1} cases/sec)", cases as f64 / (ms / 1000.0));
    println!("└─ {cases}/{cases} passed, 0 failed");
}

#[test]
fn a_frame_with_nothing_changed_computes_nothing_and_an_edit_shows_at_once() {
    let ctx = egui::Context::default();
    let mut d = designer(true, SHEET, Scheme::Light);
    frames(&mut d, &ctx, 3);
    let (parses, computed) = (d.css_cache.stats.sheet_parses, d.css_cache.stats.computed);
    let t0 = std::time::Instant::now();
    frames(&mut d, &ctx, 30);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    assert_eq!((d.css_cache.stats.sheet_parses, d.css_cache.stats.computed), (parses, computed), "R20: 30 unchanged frames compute nothing");
    // R8 / AC5: the sheet's text changes (an Apply, an edit made elsewhere): the next frame paints it.
    d.properties.css_sheets = vec![("webforms/F.css".into(), "#PNL-1 { background-color: #00ff00 }".into())];
    let t1 = std::time::Instant::now();
    let s = frames(&mut d, &ctx, 1);
    let edit_ms = t1.elapsed().as_secs_f64() * 1000.0;
    let got = panel_fill(&d, &s).expect("painted");
    assert!(close(got, [0, 255, 0]), "the edited sheet is painted by the next frame: {got:?}");
    println!("┌─ designer_css_effective: no recompute, edit shown");
    println!("│ 30 unchanged frames: {ms:.0} ms ({:.0} frames/sec), sheet parses {parses}, controls computed {computed}, both unchanged", 30.0 / (ms / 1000.0));
    println!("│ one frame after the sheet changed: {edit_ms:.1} ms");
    println!("└─ 2/2 passed, 0 failed");
}

#[test]
fn the_badge_lists_what_the_canvas_cannot_paint_up_to_eight_then_how_many_more() {
    let tr = crate::i18n::current_tr(&egui::Context::default());
    let mk = |n: usize| -> Vec<cobolt_forms::css_effective::Unpainted> {
        (0..n)
            .map(|i| cobolt_forms::css_effective::Unpainted { css: "width".into(), value: format!("{i}%"), sheet: Some("webforms/F.css".into()), line: i + 1, why: cobolt_forms::css_effective::Why::Geometry })
            .collect()
    };
    let one = css_badge_text(&tr, &mk(1));
    assert_eq!(one.lines().count(), 2, "{one}");
    assert!(one.contains("webforms/F.css:1  width: 0%"), "{one}");
    let ten = css_badge_text(&tr, &mk(10));
    assert_eq!(ten.lines().count(), 1 + 8 + 1, "header, eight lines, the rest: {ten}");
    assert!(ten.ends_with(&tr.css_badge_more.replace("{}", "2")), "{ten}");
    println!("┌─ designer_css_effective: the badge text\n│ 1 declaration: 2 lines; 10 declarations: header + 8 + 'more' = 10 lines\n└─ 2/2 passed, 0 failed");

    // A real frame: a width in percent is geometry, the badge's tooltip names it, the colour is painted.
    let ctx = egui::Context::default();
    let mut d = designer(true, "#PNL-1 { width: 50%; background-color: #00ff00 }", Scheme::Light);
    let s = frames(&mut d, &ctx, 3);
    assert!(close(panel_fill(&d, &s).expect("painted"), [0, 255, 0]));
    let view = d.css_style_view().expect("a view");
    let u = view.unpainted.get("PNL-1").expect("the width is not painted");
    assert!(u.iter().any(|x| x.css == "width" && x.line == 1), "{u:?}");
    assert_eq!(d.form.controls[0].rect.w, 160, "a width: 50% keeps its designed rectangle");
}
