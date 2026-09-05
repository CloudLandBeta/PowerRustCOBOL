#![cfg(feature = "render")]
//! **Spec 057 AC7 (R10) — the two frame painters the audit found drawing with
//! the wrong radius.**
//!
//! E12: a selected, frameless, rounded control drew its selection outline
//! square. E13: a child's drop shadow at a rounded parent's corner was shaped
//! with the child's own radius, not the lifted one, so every ring reached into
//! the corner notch.

use std::collections::HashMap;

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::paint::draw_control;
use cobolt_forms::render::{render_form, Backdrop, DesignedState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Color32, Pos2, Rect, Vec2};

/// Every `Rect` shape (with its corner radius) a frame produced.
fn rect_shapes(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::epaint::RectShape> {
    fn walk(s: &egui::Shape, out: &mut Vec<egui::epaint::RectShape>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::Shape::Rect(r) => out.push(r.clone()),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cs in shapes {
        walk(&cs.shape, &mut out);
    }
    out
}

/// **E12 — the selection outline of a frameless rounded Label follows its arc.**
#[test]
fn a_selected_frameless_labels_outline_takes_the_labels_radius() {
    let mut label = Control::new("Label-1", ControlType::Label, 20, 20);
    label.rect = MRect::new(20, 20, 160, 60);
    label.set_prop("CornerRadius", PropValue::Int(12));
    label.set_prop("BorderStyle", PropValue::String("None".into()));
    label.set_prop("ShadowEnabled", PropValue::Bool(false));

    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(root, |ui| {
                draw_control(&ui.painter().clone(), Pos2::ZERO, &label, true, true, 1.0, 1.0, None);
            });
    });
    full.textures_delta.clear();

    let selection: Vec<_> = rect_shapes(&full.shapes)
        .into_iter()
        .filter(|r| {
            r.stroke.width > 0.0 && (r.stroke.color.r(), r.stroke.color.g(), r.stroke.color.b()) == (60, 120, 230)
        })
        .collect();
    assert!(!selection.is_empty(), "a selected Label draws its selection outline");
    for r in &selection {
        assert_eq!(
            r.corner_radius,
            egui::CornerRadius::same(12),
            "the outline is drawn with the Label's own radius, not square: {r:?}"
        );
    }
}

/// **E13 — a child's drop shadow at a rounded parent's corner is cut to the
/// parent's arc.** Every ring stays inside the parent's content rect and its
/// NW corner is lifted well past the child's own radius.
#[test]
fn a_childs_shadow_at_a_rounded_corner_is_shaped_to_the_parents_arc() {
    let mut panel = Control::new("Panel-1", ControlType::Panel, 40, 40);
    panel.rect = MRect::new(40, 40, 400, 300);
    panel.set_prop("CornerRadius", PropValue::Int(40));
    panel.set_prop("ShadowEnabled", PropValue::Bool(false));
    panel.set_prop("BackgroundColor", PropValue::String("#FFFFFFFF".into()));
    let mut child = Control::new("Label-1", ControlType::Label, 44, 44);
    child.rect = MRect::new(44, 44, 160, 120);
    child.parent = Some("Panel-1".into());
    child.set_prop("ShadowEnabled", PropValue::Bool(true));
    child.set_prop("ShadowColor", PropValue::String("#FF0000FF".into()));
    child.set_prop("ShadowOpacity", PropValue::Int(60));
    child.set_prop("ShadowDirection", PropValue::String("NorthWest".into()));
    child.set_prop("ShadowDistance", PropValue::Int(6));
    let controls = vec![panel, child];

    let ctx = egui::Context::default();
    cobolt_forms::paint::set_glass_style(&ctx, cobolt_forms::model::GlassStyle::Classic);
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(700.0, 500.0)));
    let mut rects = HashMap::new();
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let inp = RenderInput {
                    controls: &controls,
                    state: &DesignedState,
                    form_size: Vec2::new(600.0, 400.0),
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Backdrop::default(),
                };
                rects = render_form(ui, &inp).control_rects;
            });
    });
    full.textures_delta.clear();
    let panel_rect = rects["Panel-1"];
    let content = panel_rect.shrink(2.0);

    // The rings: red fills of the shadow colour, no stroke.
    let rings: Vec<_> = rect_shapes(&full.shapes)
        .into_iter()
        .filter(|r| r.fill.a() > 0 && r.fill.r() > 0 && r.fill.g() == 0 && r.fill.b() == 0)
        .collect();
    assert!(rings.len() >= 2, "a blurred shadow is several rings, got {}", rings.len());
    for r in &rings {
        assert!(
            r.rect.min.x >= content.min.x - 0.01 && r.rect.min.y >= content.min.y - 0.01,
            "a ring is cut to the parent's content rect {content:?}: {:?}",
            r.rect
        );
        // Lifted: the child's own radius is 0, the inner arc is 38; a ring at
        // the content edge takes the arc itself.
        assert!(
            r.corner_radius.nw >= 30,
            "the NW corner of every ring is lifted to the parent's arc: {:?}",
            r.corner_radius
        );
    }
    let _ = Color32::RED;
}
