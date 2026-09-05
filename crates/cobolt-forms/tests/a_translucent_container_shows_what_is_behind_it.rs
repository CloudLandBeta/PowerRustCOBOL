#![cfg(feature = "render")]
//! **Spec 057 AC10 (G2) — a rounded, translucent container over each kind of
//! backdrop: no wedge in any.**
//!
//! The wedge was the corner-notch mask repainting the FORM backdrop into a
//! corner where something else was behind the panel (`inner-form2`: a
//! PictureBox, so the form's own image appeared at the wrong scale in every
//! corner). With a self-clipping child at the corner nothing is repainted, so
//! the corner shows exactly what it shows when no child reaches it: the form
//! colour, the form image, the PictureBox, the Label — through the panel's own
//! soft depth layer, which a rounded card legitimately paints into its bbox
//! corners and which is the same whether or not a child is there.

/// A translucent rounded Panel, with or without a self-clipping Label reaching
/// its NW corner.
fn panel_and_child(with_child: bool) -> Vec<Control> {
    let mut panel = Control::new("PANEL", ControlType::Panel, 40, 40);
    panel.rect = MRect::new(40, 40, 400, 300);
    panel.set_prop("CornerRadius", PropValue::Int(R as i64));
    panel.set_prop("Transparency", PropValue::Int(40));
    panel.set_prop("ShadowEnabled", PropValue::Bool(false));
    panel.set_prop("BackgroundColor", PropValue::String("#FFFFFFFF".into()));
    let mut v = vec![panel];
    if with_child {
        let mut child = Control::new("CHILD", ControlType::Label, 44, 44);
        child.rect = MRect::new(44, 44, 160, 120);
        child.parent = Some("PANEL".into());
        child.set_prop("BackgroundColor", PropValue::String("#2E7D32FF".into()));
        child.set_prop("ShadowEnabled", PropValue::Bool(false));
        v.push(child);
    }
    v
}

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::model::BgImageMode;
use cobolt_forms::render::{render_form, Backdrop, DesignedState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::epaint::ClippedShape;
use egui::{pos2, Color32, Pos2, Rect, TextureId, Vec2};

const FORM: Vec2 = Vec2::new(600.0, 400.0);
const PANEL: Rect = Rect {
    min: pos2(40.0, 40.0),
    max: pos2(440.0, 340.0),
};
const R: f32 = 24.0;

/// What sits under the panel, besides the form backdrop.
#[derive(Clone, Copy, Debug)]
enum Behind {
    FormColour,
    FormImage,
    PictureBox,
    Label,
}

fn scene(behind: Behind, with_child: bool) -> Vec<ClippedShape> {
    let mut controls = Vec::new();
    match behind {
        Behind::PictureBox => {
            let mut pb = Control::new("PB", ControlType::PictureBox, 0, 0);
            pb.rect = MRect::new(0, 0, 600, 400);
            pb.set_prop("BackgroundColor", PropValue::String("#C03030FF".into()));
            pb.set_prop("ShadowEnabled", PropValue::Bool(false));
            controls.push(pb);
        }
        Behind::Label => {
            let mut lb = Control::new("LB", ControlType::Label, 0, 0);
            lb.rect = MRect::new(0, 0, 600, 400);
            lb.set_prop("BackgroundColor", PropValue::String("#3050C0FF".into()));
            lb.set_prop("ShadowEnabled", PropValue::Bool(false));
            controls.push(lb);
        }
        _ => {}
    }
    controls.extend(panel_and_child(with_child));
    fn backdrop_for(behind: Behind) -> Backdrop {
        match behind {
            Behind::FormImage => Backdrop {
                paint: true,
                color_hex: "#2060A0FF".into(),
                image: Some((TextureId::Managed(1), Vec2::new(64.0, 64.0))),
                image_mode: BgImageMode::Stretch,
                ..Default::default()
            },
            _ => Backdrop {
                paint: true,
                color_hex: "#2060A0FF".into(),
                ..Default::default()
            },
        }
    }

    let ctx = egui::Context::default();
    cobolt_forms::paint::set_glass_style(&ctx, cobolt_forms::model::GlassStyle::Classic);
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), FORM));
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let inp = RenderInput {
                    controls: &controls,
                    state: &DesignedState,
                    form_size: FORM,
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: backdrop_for(behind),
                };
                let _ = render_form(ui, &inp);
            });
    });
    full.textures_delta.clear();
    full.shapes
}

fn corner_squares() -> [Rect; 4] {
    let sq = |x: f32, y: f32| Rect::from_min_size(pos2(x, y), Vec2::splat(R));
    [
        sq(PANEL.min.x, PANEL.min.y),
        sq(PANEL.max.x - R, PANEL.min.y),
        sq(PANEL.max.x - R, PANEL.max.y - R),
        sq(PANEL.min.x, PANEL.max.y - R),
    ]
}

/// A notch mesh: every vertex inside one corner square. A restore stroke: a
/// stroked rect whose clip is a corner square.
fn repairs(shapes: &[ClippedShape]) -> Vec<String> {
    let mut out = Vec::new();
    for cs in shapes {
        if let egui::Shape::Mesh(m) = &cs.shape {
            if !m.vertices.is_empty()
                && corner_squares()
                    .iter()
                    .any(|sq| m.vertices.iter().all(|v| sq.expand(0.5).contains(v.pos)))
            {
                out.push(format!("notch mesh with {} vertices", m.vertices.len()));
            }
        }
        if let egui::Shape::Rect(r) = &cs.shape {
            if r.stroke.width > 0.0
                && corner_squares().iter().any(|sq| {
                    (cs.clip_rect.min - sq.min).length() < 0.75
                        && (cs.clip_rect.max - sq.max).length() < 0.75
                })
            {
                out.push(format!("restore stroke {:?}", r.rect));
            }
        }
    }
    out
}

/// The colour left at `p` by every fill covering it, in paint order (mesh
/// triangles take their first vertex's colour; a textured mesh, its tint).
fn composite_at(shapes: &[ClippedShape], p: Pos2) -> Color32 {
    fn over(fg: Color32, bg: Color32) -> Color32 {
        let a = fg.a() as u32;
        let k = |f: u8, b: u8| (f as u32 + b as u32 * (255 - a) / 255).min(255) as u8;
        Color32::from_rgba_premultiplied(k(fg.r(), bg.r()), k(fg.g(), bg.g()), k(fg.b(), bg.b()), k(fg.a(), bg.a()))
    }
    fn rect_contains(rs: &egui::epaint::RectShape, p: Pos2) -> bool {
        cobolt_forms::paint::rounded_rect_contains(rs.rect, rs.corner_radius, p)
    }
    fn tri(a: Pos2, b: Pos2, c: Pos2, p: Pos2) -> bool {
        let s = |p1: Pos2, p2: Pos2, p3: Pos2| (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y);
        let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
        !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
    }
    fn walk(s: &egui::Shape, clip: Rect, p: Pos2, acc: &mut Color32) {
        if !clip.contains(p) {
            return;
        }
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, p, acc)),
            egui::Shape::Rect(rs) if rs.fill.a() > 0 && rect_contains(rs, p) => *acc = over(rs.fill, *acc),
            egui::Shape::Mesh(m) => {
                for t in m.indices.chunks_exact(3) {
                    let (a, b, c) = (m.vertices[t[0] as usize], m.vertices[t[1] as usize], m.vertices[t[2] as usize]);
                    if tri(a.pos, b.pos, c.pos, p) && a.color.a() > 0 {
                        *acc = over(a.color, *acc);
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    let mut acc = Color32::TRANSPARENT;
    for cs in shapes {
        walk(&cs.shape, cs.clip_rect, p, &mut acc);
    }
    acc
}

/// Every fill covering `p`, described, in paint order — for the failure message.
fn covering(shapes: &[ClippedShape], p: Pos2) -> Vec<String> {
    fn walk(s: &egui::Shape, clip: Rect, p: Pos2, out: &mut Vec<String>) {
        if !clip.contains(p) {
            return;
        }
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, p, out)),
            egui::Shape::Rect(rs)
                if rs.fill.a() > 0
                    && cobolt_forms::paint::rounded_rect_contains(rs.rect, rs.corner_radius, p) =>
            {
                out.push(format!(
                    "RECT {:?} r={:?} fill={:?} clip={:?}",
                    rs.rect, rs.corner_radius, rs.fill, clip
                ))
            }
            egui::Shape::Mesh(m) => {
                let s = |p1: Pos2, p2: Pos2, p3: Pos2| {
                    (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
                };
                for t in m.indices.chunks_exact(3) {
                    let (a, b, c) = (
                        m.vertices[t[0] as usize],
                        m.vertices[t[1] as usize],
                        m.vertices[t[2] as usize],
                    );
                    let (d1, d2, d3) = (s(p, a.pos, b.pos), s(p, b.pos, c.pos), s(p, c.pos, a.pos));
                    let inside = !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0)
                        && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0));
                    if inside && a.color.a() > 0 {
                        out.push(format!(
                            "MESH {} verts tex={:?} first={:?} bounds={:?}",
                            m.vertices.len(),
                            m.texture_id,
                            a.color,
                            m.calc_bounds()
                        ));
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cs in shapes {
        walk(&cs.shape, cs.clip_rect, p, &mut out);
    }
    out
}

/// Inside the panel's bbox, outside its arc: the notch.
fn notch_points() -> Vec<Pos2> {
    let d = R * 0.15;
    vec![
        PANEL.min + Vec2::splat(d),
        pos2(PANEL.max.x - d, PANEL.min.y + d),
        PANEL.max - Vec2::splat(d),
        pos2(PANEL.min.x + d, PANEL.max.y - d),
    ]
}

#[test]
fn no_corner_is_repaired_and_every_notch_shows_exactly_what_is_behind_the_panel() {
    for behind in [Behind::FormColour, Behind::FormImage, Behind::PictureBox, Behind::Label] {
        let with = scene(behind, true);
        let without = scene(behind, false);
        let fixes = repairs(&with);
        assert!(fixes.is_empty(), "{behind:?}: a corner was repaired: {fixes:?}");
        for p in notch_points() {
            let a = composite_at(&with, p);
            let b = composite_at(&without, p);
            assert_eq!(
                a,
                b,
                "{behind:?}: the notch at {p:?} differs from the same point without the child\n  with:    {:#?}\n  without: {:#?}",
                covering(&with, p),
                covering(&without, p)
            );
        }
        // And the child really is there, lifted: the panel's face reaches the
        // corner square (a child that never rendered would pass vacuously).
        let child_paint = with.iter().any(|cs| match &cs.shape {
            egui::Shape::Rect(r) => r.fill.a() > 0 && r.fill.g() > r.fill.r() && r.fill.g() > r.fill.b() && r.corner_radius.nw >= 18,
            _ => false,
        });
        assert!(child_paint, "{behind:?}: the child was drawn with its NW corner lifted");
    }
}
