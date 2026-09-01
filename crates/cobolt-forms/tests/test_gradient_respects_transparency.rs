#![cfg(feature = "render")]
//! `Transparency` applies to a control's face whether or not it has a
//! background gradient.
//!
//! Two alphas are computed for every control: `alpha_mul`, the inherited
//! (ancestor/container) alpha, and `face_alpha`, which folds in the control's
//! OWN `Transparency`. The flat and frosted faces used `face_alpha`; the
//! background gradient used only `alpha_mul`, so turning a gradient on made the
//! control's own transparency stop working entirely — and turning it off made
//! it work again (operator, 2026-09-01).

use cobolt_forms::model::Rect as MRect;
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Color32, Rect, Vec2};

/// Paint one control and return the highest alpha it put on screen.
fn peak_face_alpha(gradient: bool, transparency: i64) -> u8 {
    let mut c = Control::new("L", ControlType::Label, 20, 20);
    c.rect = MRect::new(20, 20, 200, 80);
    c.set_prop("BackgroundColor", PropValue::String("#4E4E4EFF".into()));
    c.set_prop("Transparency", PropValue::Int(transparency));
    c.set_prop("BackgroundGradientEnabled", PropValue::Bool(gradient));
    c.set_prop(
        "BackgroundGradientStartColor",
        PropValue::String("#4E4E4EFF".into()),
    );
    c.set_prop(
        "BackgroundGradientEndColor",
        PropValue::String("#000000FF".into()),
    );
    c.set_prop("BackgroundGradientDirection", PropValue::String("South".into()));

    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(400.0, 200.0)));
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                cobolt_forms::paint::draw_control(
                    &ui.painter().clone(),
                    pos2(0.0, 0.0),
                    &c,
                    false,
                    true,
                    1.0,
                    1.0,
                    None,
                );
            });
    });
    full.textures_delta.clear();

    // The control's own area, so the form/panel behind it is not counted.
    let probe = Rect::from_min_size(pos2(20.0, 20.0), Vec2::new(200.0, 80.0));
    fn walk(s: &egui::Shape, probe: Rect, best: &mut u8) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, probe, best)),
            egui::Shape::Rect(r) => {
                if probe.contains_rect(r.rect) {
                    *best = (*best).max(r.fill.a());
                }
            }
            egui::Shape::Mesh(m) => {
                // A gradient is a mesh; its vertex colours carry the face alpha.
                if probe.contains_rect(m.calc_bounds()) {
                    for v in &m.vertices {
                        *best = (*best).max(v.color.a());
                    }
                }
            }
            _ => {}
        }
    }
    let mut best = 0u8;
    for cs in &full.shapes {
        let vis = probe.intersect(cs.clip_rect);
        if vis.is_positive() {
            walk(&cs.shape, probe, &mut best);
        }
    }
    let _ = Color32::WHITE;
    best
}

#[test]
fn transparency_applies_with_and_without_a_background_gradient() {
    let flat_opaque = peak_face_alpha(false, 0);
    let flat_faded = peak_face_alpha(false, 60);
    let grad_opaque = peak_face_alpha(true, 0);
    let grad_faded = peak_face_alpha(true, 60);

    println!(
        "\n  peak face alpha\n\
         \x20   flat      Transparency=0 -> {flat_opaque:3}   Transparency=60 -> {flat_faded:3}\n\
         \x20   gradient  Transparency=0 -> {grad_opaque:3}   Transparency=60 -> {grad_faded:3}\n"
    );

    assert!(
        flat_faded < flat_opaque,
        "precondition: without a gradient, Transparency must already fade the \
         face ({flat_faded} vs {flat_opaque})"
    );
    assert!(
        grad_opaque > 0,
        "precondition: a gradient face must paint something at all"
    );
    assert!(
        grad_faded < grad_opaque,
        "a gradient face ignored Transparency: {grad_faded} at 60% vs \
         {grad_opaque} at 0%. The face owes its own transparency whether it is \
         flat or a gradient — turning a gradient on must not disable it."
    );
}
