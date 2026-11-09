// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-036 T7 — a colour the page's stylesheet decided is painted as given. `Transparency` is a
//! property of the control, and CSS has no such property, so a Panel at `Transparency` 50 whose
//! background comes from a rule shows that colour at full strength, on the canvas and on the running
//! form alike; the same Panel with the colour as a property is still faded.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::css_effective::{CssFinal, FinalColors};
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{DesignedState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Color32, Rect, Vec2};

const RED: [u8; 4] = [255, 0, 0, 255];

fn panel(property_colour: bool) -> Control {
    let mut c = Control::new("PNL-1", ControlType::Panel, 20, 20);
    c.rect = MRect::new(20, 20, 200, 120);
    c.set_prop("Transparency", PropValue::String("50".into()));
    if property_colour {
        c.set_prop("BackgroundColor", PropValue::String("#FF0000FF".into()));
    }
    c
}

fn css() -> CssFinal {
    let mut f = CssFinal::default();
    f.controls.insert("PNL-1".into(), FinalColors { face: Some(RED), ..Default::default() });
    f
}

fn walk(s: &egui::Shape, fills: &mut Vec<Color32>) {
    match s {
        egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, fills)),
        egui::Shape::Rect(r) => fills.push(r.fill),
        _ => {}
    }
}

/// The fills of one frame: `faces` is the canvas path, otherwise `render_form`.
fn fills(controls: &[Control], css_final: Option<&CssFinal>, faces: bool) -> Vec<Color32> {
    let form = Vec2::new(260.0, 180.0);
    let ctx = egui::Context::default();
    let mut shapes = Vec::new();
    for _ in 0..2 {
        let active = ActiveTabs::new();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), form));
        let mut out = ctx.run_ui(input, |root| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                let inp = RenderInput {
                    css_final,
                    controls,
                    state: &DesignedState,
                    form_size: form,
                    glass: true,
                    mode: RenderMode::Static,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                if faces {
                    cobolt_forms::render::render_faces(ui.painter(), pos2(0.0, 0.0), &inp);
                } else {
                    cobolt_forms::render::render_form(ui, &inp);
                }
            });
        });
        out.textures_delta.clear();
        shapes = out.shapes.into_iter().map(|c| c.shape).collect();
    }
    let mut v = Vec::new();
    shapes.iter().for_each(|s| walk(s, &mut v));
    v
}

fn strong_red(v: &[Color32]) -> bool {
    v.iter().any(|c| *c == Color32::from_rgba_premultiplied(255, 0, 0, 255))
}

#[test]
fn a_css_colour_paints_unfaded_on_both_paths_and_a_property_colour_still_fades() {
    let t0 = std::time::Instant::now();
    let css = css();
    let mut cases = 0;
    for faces in [false, true] {
        let with_css = fills(&[panel(false)], Some(&css), faces);
        assert!(strong_red(&with_css), "faces={faces}: the CSS colour is painted at full strength: {with_css:?}");
        let property = fills(&[panel(true)], None, faces);
        assert!(!strong_red(&property), "faces={faces}: the same colour as a property is faded by Transparency 50");
        let none = fills(&[panel(false)], None, faces);
        assert!(!strong_red(&none), "faces={faces}: no css_final, no red");
        cases += 3;
    }
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("┌─ a_css_colour_is_not_faded_by_transparency");
    println!("│ cases: Panel at Transparency 50 — CSS face / property face / neither, on render_form and render_faces");
    println!("│ time: {ms:.1} ms for {cases} frames ({:.0} frames/sec)", cases as f64 * 2.0 / (ms / 1000.0));
    println!("└─ {cases}/{cases} passed, 0 failed");
}
