#![cfg(feature = "render")]
//! **AC1 / AC6 of spec 057 — the operator's own form.**
//!
//! `inner-form2` puts a rounded, fully transparent Panel over two
//! PictureBoxes, with three Gauges and a LineChart inside it. Every one of
//! those children draws its own frame to the panel's arc (they carry a
//! `_ContainerClip`), so the corners are already correct when the notch mask
//! arrives — and the mask then repaints the FORM backdrop (`rio0.png`,
//! stretched) over them, where the PictureBoxes are what is actually behind
//! the panel. That mismatch is the wedge at every corner, on every surface.
//!
//! This test is written RED first: it asserts no notch mesh and no restore
//! stroke in any of Panel-1's four corner squares, on all three surfaces, and
//! that the three surfaces agree corner by corner (AC6).

use std::collections::HashMap;
use std::path::PathBuf;

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::paint::corner_radius;
use cobolt_forms::render::{
    merge_props, notch_mask_rounding, render_faces, render_form, Backdrop, DesignedState,
    FormState, RenderInput, RenderMode,
};
use cobolt_forms::{Control, Form};
use egui::{pos2, Rect, Vec2};

fn project() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerDemo3")
}

fn load() -> Form {
    let root = project();
    cobolt_forms::assets::set_base(&root);
    cobolt_forms::load_form(&root.join("forms/General/inner-form2.cfrm"))
        .expect("the reproduction form must parse")
}

/// The run-form host's merge: every designed prop stringified and merged back.
struct Stringified;
impl FormState for Stringified {
    fn live(&self, base: &Control) -> Control {
        let props: HashMap<String, String> = base
            .properties
            .iter()
            .map(|(k, v)| (k.clone(), v.to_xml_string()))
            .collect();
        merge_props(base, props.iter())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Surface {
    Preview,
    Run,
    Canvas,
}

const CORNERS: [&str; 4] = ["NW", "NE", "SE", "SW"];

/// Which of Panel-1's corners were repaired — by a notch mesh or a restore
/// stroke — on `s`. Order NW, NE, SE, SW.
fn repaired_corners(form: &Form, s: Surface) -> [bool; 4] {
    let controls = &form.controls;
    let panel = controls
        .iter()
        .find(|c| c.id == "Panel-1")
        .expect("Panel-1 is in the form");
    let r = corner_radius(panel);
    // Panel-1 reaches y=1288 on an 888-high form: give the window the room.
    let size = Vec2::new(form.width as f32, form.height as f32);
    let window = Vec2::new(1700.0, 1400.0);
    let ctx = egui::Context::default();
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), window));
    // The real PictureBox images are 5225x2941; a headless context reports
    // egui's 2048 default and its debug assertion fires where a real backend
    // would not. Report what the machine's GPU actually allows.
    input.max_texture_side = Some(8192);
    let mut rects = HashMap::new();
    let mut canvas_decision: Option<[bool; 4]> = None;
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let state: &dyn FormState = match s {
                    Surface::Run => &Stringified,
                    _ => &DesignedState,
                };
                let inp = RenderInput {
                    controls,
                    state,
                    form_size: size,
                    glass: true,
                    mode: match s {
                        Surface::Canvas => RenderMode::Static,
                        _ => RenderMode::Interactive,
                    },
                    active_tabs: &active,
                    backdrop: Backdrop {
                        paint: true,
                        color_hex: "#2E3138FF".into(),
                        window_size: Some(window),
                        ..Default::default()
                    },
                };
                match s {
                    Surface::Canvas => {
                        let painter = ui.painter().clone();
                        let out = render_faces(&painter, ui.min_rect().min, &inp);
                        // The designer's decision, exactly as its loop takes it.
                        let idx = controls.iter().position(|c| c.id == "Panel-1").unwrap();
                        let crect = out.control_rects["Panel-1"];
                        let d = notch_mask_rounding(controls, idx, crect, r, &out.control_rects);
                        canvas_decision = Some(match d {
                            Some(cr) => [cr.nw > 0, cr.ne > 0, cr.se > 0, cr.sw > 0],
                            None => [false; 4],
                        });
                        rects = out.control_rects;
                    }
                    _ => rects = render_form(ui, &inp).control_rects,
                }
            });
    });
    full.textures_delta.clear();
    if let Some(d) = canvas_decision {
        return d;
    }
    // Run / preview: read the repair off the painted shapes.
    let p = rects["Panel-1"];
    let squares = [
        Rect::from_min_size(p.min, Vec2::splat(r)),
        Rect::from_min_size(pos2(p.max.x - r, p.min.y), Vec2::splat(r)),
        Rect::from_min_size(pos2(p.max.x - r, p.max.y - r), Vec2::splat(r)),
        Rect::from_min_size(pos2(p.min.x, p.max.y - r), Vec2::splat(r)),
    ];
    let mut hit = [false; 4];
    fn walk(shape: &egui::Shape, clip: Rect, squares: &[Rect; 4], hit: &mut [bool; 4]) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, squares, hit)),
            // A notch fan/ring mesh: every vertex inside the corner squares.
            egui::Shape::Mesh(m) if !m.vertices.is_empty() => {
                for (i, sq) in squares.iter().enumerate() {
                    if m.vertices.iter().all(|v| sq.expand(0.5).contains(v.pos)) {
                        hit[i] = true;
                    }
                }
            }
            // A restore stroke: clipped to exactly one corner square.
            egui::Shape::Rect(rs) if rs.stroke.width > 0.0 => {
                for (i, sq) in squares.iter().enumerate() {
                    if (clip.min - sq.min).length() < 0.75 && (clip.max - sq.max).length() < 0.75 {
                        hit[i] = true;
                    }
                }
            }
            _ => {}
        }
    }
    for cs in &full.shapes {
        walk(&cs.shape, cs.clip_rect, &squares, &mut hit);
    }
    hit
}

fn named(h: [bool; 4]) -> Vec<&'static str> {
    CORNERS.iter().zip(h).filter(|(_, b)| *b).map(|(n, _)| *n).collect()
}

/// AC1 — no corner of Panel-1 is repaired, on any surface.
#[test]
fn no_corner_of_the_panel_is_repainted_on_any_surface() {
    let form = load();
    let mut failures = Vec::new();
    for s in [Surface::Preview, Surface::Run, Surface::Canvas] {
        let h = repaired_corners(&form, s);
        println!("AC1 {s:?}: repaired corners = {:?}", named(h));
        if h.iter().any(|b| *b) {
            failures.push(format!("{s:?}: {:?}", named(h)));
        }
    }
    assert!(
        failures.is_empty(),
        "Panel-1's corners were repainted by the mask/restore — they were already drawn \
         rounded by their children: {failures:?}"
    );
}

/// AC6 — the three surfaces take the same decision, corner by corner.
#[test]
fn the_three_surfaces_agree_corner_by_corner() {
    let form = load();
    let a = repaired_corners(&form, Surface::Preview);
    let b = repaired_corners(&form, Surface::Run);
    let c = repaired_corners(&form, Surface::Canvas);
    assert_eq!(a, b, "preview vs run: {:?} vs {:?}", named(a), named(b));
    assert_eq!(a, c, "preview vs canvas: {:?} vs {:?}", named(a), named(c));
}
