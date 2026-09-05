#![cfg(feature = "render")]
//! **A translucent container's corners must match the rest of its edge.**
//!
//! The notch mask repaints the backdrop over the four corner arcs, erasing the
//! container's rim there, and `restore_container_outline` redraws it. It
//! redrew at FULL strength, with no knowledge of the alpha the control was
//! drawn with — so a 40 %-transparent Panel painted its rim `#66666666` along
//! the whole edge and then `#AAAAAAAA` on the four corners, and nowhere else.
//! Four bright arcs on an otherwise faded panel read exactly like a bleed
//! (operator, 2026-09-04: "all 4 corners have this problem … it seems to be a
//! transparency").
//!
//! The rule is the skill's: the restore reproduces what the FACE painted — so
//! the rim follows the control's opacity, and the user border, which
//! `draw_control` paints at full strength on a transparent container too, does
//! not.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{Backdrop, DesignedState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Color32, Rect, Vec2};

/// Every stroke colour painted inside the panel's bottom-left corner square.
fn corner_strokes(transparency: i64) -> Vec<Color32> {
    let mut panel = Control::new("Panel-1", ControlType::Panel, 80, 60);
    panel.rect = MRect::new(80, 60, 400, 300);
    panel.set_prop("CornerRadius", PropValue::Int(24));
    panel.set_prop("Transparency", PropValue::Int(transparency));
    panel.set_prop("BackgroundColor", PropValue::String("#FFFFFFFF".into()));
    // A child reaching the corners, so the guardian masks them at all.
    let mut child = Control::new("Label-1", ControlType::Label, 80, 60);
    child.rect = MRect::new(80, 60, 400, 300);
    child.parent = Some("Panel-1".to_owned());
    child.set_prop("BackgroundColor", PropValue::String("#FF0000FF".into()));
    let controls = vec![panel, child];

    let size = Vec2::new(700.0, 500.0);
    let ctx = egui::Context::default();
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), size));
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let inp = RenderInput {
                    controls: &controls,
                    state: &DesignedState,
                    form_size: size,
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Backdrop {
                        color_hex: "#2060A0FF".into(),
                        ..Default::default()
                    },
                };
                let _ = cobolt_forms::render::render_form(ui, &inp);
            });
    });
    full.textures_delta.clear();

    let probe = Rect::from_min_max(pos2(80.0, 336.0), pos2(104.0, 360.0));
    let mut out = Vec::new();
    fn walk(s: &egui::Shape, probe: Rect, out: &mut Vec<Color32>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, probe, out)),
            egui::Shape::Rect(r) if r.stroke.width > 0.0 && probe.intersects(r.rect) => {
                out.push(r.stroke.color)
            }
            _ => {}
        }
    }
    for s in &full.shapes {
        walk(&s.shape, probe, &mut out);
    }
    out
}

#[test]
fn a_translucent_panel_does_not_restore_a_full_strength_rim_on_its_corners() {
    let strokes = corner_strokes(40);
    // The face's own rim, drawn once along the whole edge, at 60 % of 170.
    let faded_rim = Color32::from_rgba_premultiplied(102, 102, 102, 102);
    assert!(
        strokes.contains(&faded_rim),
        "the face should paint a faded rim, got {strokes:?}"
    );
    // The unfaded rim must appear NOWHERE: that is the corner artifact.
    let full_rim = Color32::from_rgba_premultiplied(170, 170, 170, 170);
    assert!(
        !strokes.contains(&full_rim),
        "a full-strength rim was restored on a 40%-transparent panel's corner \
         — four bright arcs on a faded edge: {strokes:?}"
    );
}

/// An opaque panel is unchanged: its rim is restored at full strength, because
/// that is what its face painted.
#[test]
fn an_opaque_panel_still_restores_its_rim_at_full_strength() {
    let strokes = corner_strokes(0);
    let full_rim = Color32::from_rgba_premultiplied(170, 170, 170, 170);
    assert!(
        strokes.contains(&full_rim),
        "an opaque panel's corners must keep their rim: {strokes:?}"
    );
}

/// The user border is NOT faded — `draw_control` paints it at full strength on
/// a transparent container, and the restore must match the face, not improve
/// on it.
#[test]
fn the_user_border_is_restored_exactly_as_the_face_painted_it() {
    let strokes = corner_strokes(40);
    let border = Color32::from_rgba_premultiplied(136, 136, 136, 255);
    assert!(
        strokes.contains(&border),
        "the restored border must match the face's own: {strokes:?}"
    );
}
