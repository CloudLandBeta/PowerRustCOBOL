#![cfg(feature = "render")]
// The Gauge's needle exists and takes the colour the developer defined for it
// on every surface. Regression net for the 2026-08-15 report ("gauge not
// showing needle; preview shows the needle, but its color is not the one
// defined for it in RAD"): one control rendered through the three real paths —
// designer canvas (`render_faces`, Static), preview (`render_form` Interactive
// over designed props) and run form (`render_form` Interactive over the
// stringified live-state merge the form host performs) — must paint the same
// needle, in the same colour, on each.
//
// The colour that defines it is `NeedleColor`, and only that (operator,
// 2026-09-03: "Hide Color, always use NeedleColor for the needle"). The meter's
// `Color` is deliberately set to something else here, so each surface has to
// prove it painted the needle's own ink rather than the meter's — the fallback
// these tests used to assert.


use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::render::{
    merge_props, render_faces, render_form, Backdrop, DesignedState, FormState, RenderInput,
    RenderMode,
};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{Color32, Pos2};

/// The run-form host's `LiveState` merge: every designed prop stringified
/// (`CtrlState::from_control`) and merged back over the base as strings.
struct Stringified;
impl FormState for Stringified {
    fn live(&self, base: &Control) -> Control {
        let props: std::collections::HashMap<String, String> = base
            .properties
            .iter()
            .map(|(k, v)| (k.clone(), v.to_xml_string()))
            .collect();
        merge_props(base, props.iter())
    }
}

/// `(line-segment stroke colours, circle fill colours)` painted for `ctrl`.
fn needle_ink(mode: RenderMode, state: &dyn FormState, via_faces: bool, ctrl: &Control) -> (Vec<Color32>, Vec<Color32>) {
    let ctx = egui::Context::default();
    let controls = vec![ctrl.clone()];
    let active = ActiveTabs::default();
    let mut full = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(700.0, 560.0),
            )),
            ..Default::default()
        },
        |root_ui| {
            egui::CentralPanel::default().show(root_ui, |ui| {
                let input = RenderInput {
                    controls: &controls,
                    state,
                    form_size: egui::vec2(600.0, 400.0),
                    glass: true,
                    mode,
                    active_tabs: &active,
                    backdrop: Backdrop::default(),
                };
                if via_faces {
                    let painter = ui.painter().clone();
                    render_faces(&painter, ui.min_rect().min, &input, None);
                } else {
                    render_form(ui, &input);
                }
            });
        },
    );
    full.textures_delta.clear();
    fn walk(s: &egui::Shape, segs: &mut Vec<Color32>, circles: &mut Vec<Color32>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, segs, circles)),
            egui::Shape::LineSegment { stroke, .. } => segs.push(stroke.color),
            egui::Shape::Circle(c) => circles.push(c.fill),
            _ => {}
        }
    }
    let (mut segs, mut circles) = (Vec::new(), Vec::new());
    for cs in &full.shapes {
        walk(&cs.shape, &mut segs, &mut circles);
    }
    (segs, circles)
}

fn assert_needle_everywhere(ctrl: &Control, colour: Color32, what: &str) {
    let surfaces: [(&str, (Vec<Color32>, Vec<Color32>)); 3] = [
        (
            "canvas (render_faces/Static)",
            needle_ink(RenderMode::Static, &DesignedState, true, ctrl),
        ),
        (
            "preview (render_form/Interactive, designed)",
            needle_ink(RenderMode::Interactive, &DesignedState, false, ctrl),
        ),
        (
            "run form (render_form/Interactive, stringified)",
            needle_ink(RenderMode::Interactive, &Stringified, false, ctrl),
        ),
    ];
    // The meter's own colour on every control below, and never the needle's.
    let meter = Color32::from_rgb(0x16, 0xA3, 0x4A);
    for (name, (segs, circles)) in &surfaces {
        assert!(
            segs.contains(&colour),
            "{what}: {name} painted no needle in {colour:?} — segments {segs:?}"
        );
        assert!(
            circles.contains(&colour),
            "{what}: {name} painted no hub in {colour:?} — circles {circles:?}"
        );
        if colour != meter {
            assert!(
                !segs.contains(&meter) && !circles.contains(&meter),
                "{what}: {name} let the meter's Color reach the needle — \
                 segments {segs:?}, circles {circles:?}"
            );
        }
    }

    println!("  {what}: needle + hub in {colour:?} on all three surfaces");
}

/// The operator's control, prop for prop (inner-form1, 2026-08-15): a 160x144
/// Donut, Value 80, StrokeWidth 15, `ShowNeedle` left at its default. It
/// painted no needle on any surface.
///
/// `NeedleColor` is what defines the needle now, so the meter keeps the
/// report's `#16A34A` and the needle asks for its own `#D92525` — a surface
/// that paints green has fallen back to the meter, which is the very coupling
/// the operator removed.
#[test]
fn a_donut_gauge_paints_its_needle_in_the_defined_colour_on_every_surface() {
    let mut g = Control::new("Gauge-1", ControlType::Gauge, 20, 20);
    g.rect = cobolt_forms::Rect::new(20, 20, 160, 144);
    g.set_prop("GaugeStyle", PropValue::String("Donut".into()));
    g.set_prop("Value", PropValue::Int(80));
    g.set_prop("StrokeWidth", PropValue::Int(15));
    g.set_prop("Color", PropValue::String("#16A34AFF".into()));
    g.set_prop("NeedleColor", PropValue::String("#D92525FF".into()));
    assert_needle_everywhere(&g, Color32::from_rgb(0xD9, 0x25, 0x25), "Donut");
}

/// Same guarantee for the Radial, which always had the needle — the colour
/// must be the developer's, not a theme's and not the meter's.
#[test]
fn a_radial_gauge_paints_its_needle_in_the_defined_colour_on_every_surface() {
    let mut g = Control::new("Gauge-1", ControlType::Gauge, 20, 20);
    g.rect = cobolt_forms::Rect::new(20, 20, 200, 120);
    g.set_prop("Value", PropValue::Int(42));
    g.set_prop("Color", PropValue::String("#16A34AFF".into()));
    g.set_prop("NeedleColor", PropValue::String("#FF0000FF".into()));
    assert_needle_everywhere(&g, Color32::from_rgb(0xFF, 0x00, 0x00), "Radial");
}

