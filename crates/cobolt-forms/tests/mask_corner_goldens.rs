#![cfg(feature = "render")]
//! **Goldens for the corners that STILL need the notch mask after spec 057.**
//!
//! Spec 057 stops masking a corner whose overlapping descendants all draw
//! their own frame to the arc. A corner reached by a GRANDCHILD keeps the
//! mask: a grandchild never receives a container clip for the outer container
//! (`render.rs` `picturebox_container_border` consults the immediate parent
//! only). These goldens pin, per surface, exactly what such a corner paints —
//! captured from the tree BEFORE any 057 change — so the rule can be proved to
//! have changed nothing there (AC4).
//!
//! `COBOLT_WRITE_GOLDEN=1` rewrites the files; otherwise they are compared.
//! The three surfaces paint different things into a masked corner today (the
//! root pane hands the engine no image; the occupant path falls back to the
//! ambient fill), so each has its own file.

use std::collections::HashMap;
use std::path::PathBuf;

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{BgImageMode, Rect as MRect};
use cobolt_forms::paint::{corner_radius, draw_container_notch_mask};
use cobolt_forms::render::{
    notch_mask_rounding, render_faces, render_form, Backdrop, DesignedState, RenderInput,
    RenderMode,
};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Color32, Rect, Vec2};

/// OUTER (rounded, shadowed) → INNER (square, frameless) → LBL: the Label is a
/// grandchild of OUTER, so every corner keeps the mask.
fn scene() -> Vec<Control> {
    let r = MRect::new(100, 80, 400, 300);
    let mut outer = Control::new("OUTER", ControlType::Panel, 100, 80);
    outer.rect = r;
    outer.set_prop("CornerRadius", PropValue::Int(24));
    outer.set_prop("ShadowEnabled", PropValue::Bool(true));
    outer.set_prop("BorderStyle", PropValue::String("Fixed3D".into()));
    outer.set_prop("BackgroundColor", PropValue::String("#F0F0F0FF".into()));
    let mut inner = Control::new("INNER", ControlType::Panel, 100, 80);
    inner.rect = r;
    inner.parent = Some("OUTER".into());
    inner.set_prop("CornerRadius", PropValue::Int(0));
    inner.set_prop("HideBackground", PropValue::Bool(true));
    inner.set_prop("BorderStyle", PropValue::String("None".into()));
    inner.set_prop("ShadowEnabled", PropValue::Bool(false));
    let mut lbl = Control::new("LBL", ControlType::Label, 100, 80);
    lbl.rect = r;
    lbl.parent = Some("INNER".into());
    lbl.set_prop("BackgroundColor", PropValue::String("#FF0000FF".into()));
    lbl.set_prop("Caption", PropValue::String("grandchild".into()));
    vec![outer, inner, lbl]
}

#[derive(Clone, Copy)]
enum Surface {
    Window,
    Pane,
    Faces,
}

fn backdrop(s: Surface) -> Backdrop {
    match s {
        // The run window: colour + gradient + a stretched image.
        Surface::Window => Backdrop {
            paint: true,
            color_hex: "#2060A0FF".into(),
            gradient_enabled: true,
            gradient_start_hex: "#2060A0FF".into(),
            gradient_end_hex: "#103050FF".into(),
            gradient_direction: "South".into(),
            image: Some((egui::TextureId::Managed(1), Vec2::new(64.0, 64.0))),
            image_mode: BgImageMode::Stretch,
            ..Default::default()
        },
        // Exactly what the host builds for a pane occupant.
        Surface::Pane => Backdrop {
            paint: true,
            color_hex: "#00000000".into(),
            transparency: 100,
            behind_fill: Some(Color32::from_rgb(0x20, 0x60, 0xA0)),
            ..Default::default()
        },
        Surface::Faces => Backdrop::default(),
    }
}

fn render(s: Surface) -> (Vec<egui::epaint::ClippedShape>, HashMap<String, Rect>) {
    let controls = scene();
    let size = Vec2::new(700.0, 500.0);
    let ctx = egui::Context::default();
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), size));
    let mut rects = HashMap::new();
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let inp = RenderInput {
                    controls: &controls,
                    state: &DesignedState,
                    form_size: size,
                    glass: true,
                    mode: match s {
                        Surface::Faces => RenderMode::Static,
                        _ => RenderMode::Interactive,
                    },
                    active_tabs: &active,
                    backdrop: backdrop(s),
                };
                match s {
                    Surface::Faces => {
                        let painter = ui.painter().clone();
                        let out = render_faces(&painter, ui.min_rect().min, &inp);
                        // The designer's own notch loop, replicated
                        // (designer.rs:7259-7308): same rule, same mask.
                        for (idx, c) in controls.iter().enumerate() {
                            let rad = corner_radius(c);
                            if let Some(crect) = out.control_rects.get(&c.id) {
                                if let Some(rounding) = notch_mask_rounding(
                                    &controls,
                                    idx,
                                    *crect,
                                    rad,
                                    &out.control_rects,
                                ) {
                                    draw_container_notch_mask(
                                        &painter,
                                        *crect,
                                        rounding,
                                        Color32::from_rgb(0x20, 0x60, 0xA0),
                                        None,
                                        None,
                                        255,
                                        None,
                                    );
                                }
                            }
                        }
                        rects = out.control_rects;
                    }
                    _ => rects = render_form(ui, &inp).control_rects,
                }
            });
    });
    full.textures_delta.clear();
    (full.shapes, rects)
}

fn r2(v: f32) -> f32 {
    (v * 4.0).round() / 4.0
}
fn fr(r: Rect) -> String {
    format!("[{} {} {} {}]", r2(r.min.x), r2(r.min.y), r2(r.max.x), r2(r.max.y))
}
fn c8(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.r(), c.g(), c.b(), c.a())
}
fn fnv64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// One line per non-text shape, geometry first, then a hash of the whole
/// shape so a change the summary cannot see still changes the golden.
fn dump(out: &mut Vec<String>, clip: Rect, shape: &egui::Shape) {
    use egui::Shape as S;
    let hash = fnv64(&format!("{shape:?}"));
    match shape {
        S::Vec(v) => v.iter().for_each(|s| dump(out, clip, s)),
        S::Text(_) => {}
        S::Rect(rs) => out.push(format!(
            "RECT bbox={} fill={} stroke={}@{} r=[{} {} {} {}] clip={} h={hash:016x}",
            fr(rs.rect),
            c8(rs.fill),
            r2(rs.stroke.width),
            c8(rs.stroke.color),
            rs.corner_radius.nw,
            rs.corner_radius.ne,
            rs.corner_radius.sw,
            rs.corner_radius.se,
            fr(clip)
        )),
        S::Mesh(m) => out.push(format!(
            "MESH v={} i={} bbox={} clip={} h={hash:016x}",
            m.vertices.len(),
            m.indices.len(),
            fr(shape.visual_bounding_rect()),
            fr(clip)
        )),
        other => out.push(format!(
            "OTHER {:?} bbox={} clip={} h={hash:016x}",
            std::mem::discriminant(other),
            fr(other.visual_bounding_rect()),
            fr(clip)
        )),
    }
}

fn golden(name: &str, s: Surface) {
    let (shapes, rects) = render(s);
    let outer = rects.get("OUTER").expect("OUTER was drawn");
    let r = corner_radius(&scene()[0]);
    let squares = [
        Rect::from_min_size(outer.min, Vec2::splat(r)),
        Rect::from_min_size(pos2(outer.max.x - r, outer.min.y), Vec2::splat(r)),
        Rect::from_min_size(pos2(outer.max.x - r, outer.max.y - r), Vec2::splat(r)),
        Rect::from_min_size(pos2(outer.min.x, outer.max.y - r), Vec2::splat(r)),
    ];
    let mut lines = Vec::new();
    for cs in &shapes {
        let bb = cs.shape.visual_bounding_rect();
        if squares.iter().any(|sq| sq.intersects(bb)) {
            dump(&mut lines, cs.clip_rect, &cs.shape);
        }
    }
    let text = lines.join("\n") + "\n";
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/goldens")
        .join(format!("057_{name}.txt"));
    if std::env::var("COBOLT_WRITE_GOLDEN").is_ok() {
        std::fs::write(&path, &text).expect("write golden");
        println!("wrote {} ({} lines)", path.display(), lines.len());
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!("golden {name} missing — capture it with COBOLT_WRITE_GOLDEN=1 on the UNCHANGED tree")
    });
    if expected != text {
        let e: Vec<_> = expected.lines().collect();
        let g: Vec<_> = text.lines().collect();
        let first = e.iter().zip(g.iter()).position(|(a, b)| a != b).unwrap_or(e.len().min(g.len()));
        panic!(
            "golden {name} changed at line {} of {} (now {} lines):\n  expected: {}\n  got:      {}",
            first + 1,
            e.len(),
            g.len(),
            e.get(first).unwrap_or(&"<end>"),
            g.get(first).unwrap_or(&"<end>")
        );
    }
}

#[test]
fn a_still_masked_corner_paints_the_same_on_the_run_window() {
    golden("window", Surface::Window);
}
#[test]
fn a_still_masked_corner_paints_the_same_on_a_pane_occupant() {
    golden("pane", Surface::Pane);
}
#[test]
fn a_still_masked_corner_paints_the_same_on_the_designer_canvas() {
    golden("faces", Surface::Faces);
}
