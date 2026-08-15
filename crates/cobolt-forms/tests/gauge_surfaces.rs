#![cfg(feature = "render")]
// Repro scaffold for the operator's report (2026-08-15): "gauge not showing
// needle; preview shows the needle, but its color is not the one defined for
// it in RAD". Renders one Radial Gauge with a developer-set `Color` through
// the three real surfaces — designer canvas (`render_faces`, Static), preview
// (`render_form` Interactive over designed props) and run form (`render_form`
// Interactive over the stringified live-state merge the form host performs) —
// and reports whether each painted a needle, and in which colour.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::render::{
    merge_props, render_faces, render_form, Backdrop, DesignedState, FormState, RenderInput,
    RenderMode,
};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{Color32, Pos2};

const RED: Color32 = Color32::from_rgb(0xFF, 0x00, 0x00);

fn gauge_with_color() -> Control {
    let mut g = Control::new("Gauge-1", ControlType::Gauge, 20, 20);
    g.rect = cobolt_forms::model::Rect::new(20, 20, 200, 120);
    g.set_prop("Value", PropValue::Int(42));
    g.set_prop("Color", PropValue::String("#FF0000".into()));
    g
}

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

#[derive(Debug)]
struct Marks {
    segments: Vec<(Color32, f32)>,
    circles: Vec<Color32>,
    path_strokes: Vec<Color32>,
}

fn collect(shape: &egui::Shape, out: &mut Marks) {
    match shape {
        egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
        egui::Shape::LineSegment { stroke, .. } => out.segments.push((stroke.color, stroke.width)),
        egui::Shape::Circle(c) => out.circles.push(c.fill),
        egui::Shape::Path(p) => {
            if let egui::epaint::ColorMode::Solid(c) = p.stroke.color {
                out.path_strokes.push(c);
            }
        }
        _ => {}
    }
}

fn run_surface(mode: RenderMode, state: &dyn FormState, via_faces: bool, ctrl: &Control) -> Marks {
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
            egui::CentralPanel::default().show_inside(root_ui, |ui| {
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
    let mut marks = Marks {
        segments: Vec::new(),
        circles: Vec::new(),
        path_strokes: Vec::new(),
    };
    for cs in &full.shapes {
        collect(&cs.shape, &mut marks);
    }
    marks
}

fn describe(name: &str, m: &Marks) {
    let needle: Vec<_> = m
        .segments
        .iter()
        .filter(|(c, _)| *c == RED)
        .collect();
    let hub = m.circles.iter().filter(|c| **c == RED).count();
    let red_paths = m.path_strokes.iter().filter(|c| **c == RED).count();
    println!(
        "  {name}: {} segments {:?}\n      circles {:?}\n      red-needle-segs {:?}, red hubs {hub}, red path strokes {red_paths}",
        m.segments.len(),
        m.segments,
        m.circles,
        needle,
    );
}

#[test]
fn gauge_needle_and_colour_on_all_three_surfaces() {
    let g = gauge_with_color();

    let canvas = run_surface(RenderMode::Static, &DesignedState, true, &g);
    let preview = run_surface(RenderMode::Interactive, &DesignedState, false, &g);
    let run = run_surface(RenderMode::Interactive, &Stringified, false, &g);

    println!("\nGauge `Color`=#FF0000, Value=42, Radial defaults:");
    describe("canvas (render_faces/Static)", &canvas);
    describe("preview (render_form/Interactive, designed)", &preview);
    describe("run     (render_form/Interactive, stringified)", &run);

    let needle_of = |m: &Marks| {
        m.segments.iter().any(|(c, _)| *c == RED) && m.circles.iter().any(|c| *c == RED)
    };
    assert!(
        needle_of(&canvas),
        "canvas: no red needle+hub — see the shape dump above"
    );
    assert!(
        needle_of(&preview),
        "preview: no red needle+hub — see the shape dump above"
    );
    assert!(
        needle_of(&run),
        "run form: no red needle+hub — see the shape dump above"
    );
}
