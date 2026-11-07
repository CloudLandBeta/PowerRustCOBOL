// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The canvas paints behind the form what the form will run over. Under a see-through theme
//! (Spatial) both runtimes replace the form's colour, gradient and picture with the theme's glass,
//! faded by the form's `Transparency`; a Web or Mobile page has no desktop behind that glass, or
//! behind a translucent form, and its build paints the stand-in desktop
//! (`cobolt_forms::theme::SEE_THROUGH_BACKDROP_RGB`): the canvas of such a project paints it too.
//!
//! Real frames of the real `DesignerPanel::show`; the colour read back is the canvas fill the
//! frame painted.

use super::*;
use crate::llm::LlmConfig;
use cobolt_forms::Form;

const SCREEN: Vec2 = Vec2::new(1400.0, 1000.0);

/// Every shape a frame painted, groups opened.
fn flatten(shape: &egui::Shape, out: &mut Vec<egui::Shape>) {
    match shape {
        egui::Shape::Vec(v) => v.iter().for_each(|s| flatten(s, out)),
        s => out.push(s.clone()),
    }
}

/// The fill of the canvas: the rectangle the size of the form drawn at the canvas origin (its outline is a
/// rectangle with no fill, and is passed over).
fn canvas_fill(form: Form, project_theme: Option<&str>, page_backdrop: bool) -> Color32 {
    let ctx = egui::Context::default();
    let llm = LlmConfig::load_defaults_for_test();
    let (w, h) = (form.width as f32, form.height as f32);
    let mut d = DesignerPanel::new(form);
    d.project_form_theme = project_theme.map(str::to_owned);
    d.page_backdrop = page_backdrop;
    let mut shapes = Vec::new();
    for t in 0..3 {
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
    let origin = d.canvas_origin.expect("the canvas has been drawn");
    shapes
        .iter()
        .find_map(|s| match s {
            egui::Shape::Rect(r)
                if r.fill != Color32::TRANSPARENT
                    && (r.rect.min - origin).length() < 0.5
                    && (r.rect.width() - w).abs() < 1.0
                    && (r.rect.height() - h).abs() < 1.0 =>
            {
                Some(r.fill)
            }
            _ => None,
        })
        .expect("the canvas fill is painted")
}

fn form(color: &str, transparency: u8) -> Form {
    let mut f = Form::new("F", "F", 400, 300);
    f.background_color = color.to_owned();
    f.transparency = transparency;
    f
}

/// `fg` (straight, alpha `a` of 255) over the opaque stand-in desktop, the way a browser blends it.
fn over_desktop(fg: (u8, u8, u8), a: u8) -> [u8; 4] {
    let (dr, dg, db) = cobolt_forms::theme::SEE_THROUGH_BACKDROP_RGB;
    let k = a as f32 / 255.0;
    let mix = |f: u8, b: u8| (f as f32 * k + b as f32 * (1.0 - k)).round() as u8;
    [mix(fg.0, dr), mix(fg.1, dg), mix(fg.2, db), 255]
}

fn close(got: Color32, want: [u8; 4]) -> bool {
    got.to_array().iter().zip(want).all(|(g, w)| (*g as i32 - w as i32).abs() <= 1)
}

#[test]
fn the_canvas_paints_what_the_form_runs_over() {
    let [gr, gg, gb, ga] = cobolt_forms::theme::SPATIAL_FORM_GLASS;
    let half_glass = (ga as f32 * 0.5).floor() as u8;
    let mut passed = 0;

    // A Mobile page under Spatial (the project's theme), its own colour nearly clear, Transparency 50:
    // the theme's glass at half its alpha, over the stand-in desktop — the colour is not painted.
    let got = canvas_fill(form("#40404008", 50), Some("spatial"), true);
    let want = over_desktop((gr, gg, gb), half_glass);
    assert!(close(got, want), "Spatial page: {got:?}, want {want:?}");
    passed += 1;

    // The same form in a Desktop project: the same glass, and the computer's desktop behind it.
    let got = canvas_fill(form("#40404008", 50), Some("spatial"), false);
    let want = Color32::from_rgba_unmultiplied(gr, gg, gb, half_glass).to_array();
    assert!(close(got, want), "Spatial desktop form: {got:?}, want {want:?}");
    passed += 1;

    // A page under the default theme, half-clear black: the black over the stand-in desktop.
    let got = canvas_fill(form("#00000080", 0), None, true);
    let want = over_desktop((0, 0, 0), 0x80);
    assert!(close(got, want), "translucent page: {got:?}, want {want:?}");
    passed += 1;

    // An opaque page form is its own colour; no backdrop shows.
    let got = canvas_fill(form("#336699FF", 0), None, true);
    assert!(close(got, [0x33, 0x66, 0x99, 255]), "opaque page: {got:?}");
    passed += 1;

    // A page with a translucent gradient paints the gradient straight over the stand-in desktop,
    // with no colour beneath it, as the page does.
    let mut f = form("#FF0000FF", 0);
    f.background_gradient_enabled = true;
    f.background_gradient_start_color = "#FFFFFF40".into();
    f.background_gradient_end_color = "#00000040".into();
    let got = canvas_fill(f, None, true);
    assert_eq!(got, cobolt_forms::paint::SEE_THROUGH_DESKTOP, "gradient page");
    passed += 1;

    println!("\n  canvas backdrop: Spatial page, Spatial desktop form, translucent page, opaque page, gradient page: {passed}/5 passed");
}
