#![cfg(feature = "render")]
//! **R7 of spec 057 — which control types draw their own frame to a rounded
//! parent's arc, measured rather than asserted.**
//!
//! A child of a rounded Panel is handed a `_ContainerClip` and is supposed to
//! draw its frame lifted to the parent's corner. Whether a given painter
//! actually does is the whole question behind the corner-notch mask, and this
//! project has paid repeatedly for answering it by reading code. So: one
//! rounded Panel, one child of every type straddling its NW notch, rendered on
//! every surface, and the pixels outside the arc counted.
//!
//! The scene is NESTED so the mask is off by construction — `notch_mask_
//! rounding` returns `None` for a container with a parent and ZERO for one
//! with no radius — while the child still receives the inner panel's clip.
//! What is measured is therefore the painter alone.
//!
//! Variants per type and surface: drop shadow off/on, glass style Classic and
//! Neumorphic (the Neumorphic halo is a frame painter too), and the child
//! "dressed" — a background colour, a gradient and a single border — so that a
//! type whose default face is transparent at the corner (a Gauge is a dial on
//! nothing) is measured with a frame to bleed with.
//!
//! Prints one line per (type, surface, shadow, style, dressed), a verdict per
//! type, and a per-surface table for `plan.md`; then asserts that the set of
//! types measured to stay inside the arc is exactly
//! `render::self_clipping_type` — the allow-list the notch rule trusts.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{GlassStyle, Rect as MRect};
use cobolt_forms::paint::set_glass_style;
use cobolt_forms::render::{
    merge_props, render_faces, render_form, self_clipping_type, Backdrop, DesignedState,
    FormState, RenderInput, RenderMode,
};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::epaint::ClippedShape;
use egui::{pos2, Pos2, Rect, Vec2};

const IN_R: f32 = 40.0;

fn scene(child: Option<&ControlType>, shadow: bool, dressed: bool) -> Vec<Control> {
    let mut out = Control::new("OUT", ControlType::Panel, 0, 0);
    out.rect = MRect::new(0, 0, 600, 400);
    out.set_prop("CornerRadius", PropValue::Int(0));
    out.set_prop("HideBackground", PropValue::Bool(true));
    out.set_prop("ShadowEnabled", PropValue::Bool(false));
    out.set_prop("BorderStyle", PropValue::String("None".into()));
    let mut inner = Control::new("IN", ControlType::Panel, 40, 40);
    inner.rect = MRect::new(40, 40, 400, 300);
    inner.parent = Some("OUT".into());
    inner.set_prop("CornerRadius", PropValue::Int(IN_R as i64));
    inner.set_prop("BackgroundColor", PropValue::String("#FFFFFFFF".into()));
    inner.set_prop("ShadowEnabled", PropValue::Bool(false));
    let mut v = vec![out, inner];
    if let Some(ct) = child {
        let mut c = Control::new("C", ct.clone(), 44, 44);
        c.rect = MRect::new(44, 44, 160, 120);
        c.parent = Some("IN".into());
        c.set_prop("ShadowEnabled", PropValue::Bool(shadow));
        if dressed {
            c.set_prop("BackgroundColor", PropValue::String("#3060C0FF".into()));
            c.set_prop("BorderStyle", PropValue::String("Single".into()));
            c.set_prop("BorderWidth", PropValue::Int(1));
            c.set_prop("BorderColor", PropValue::String("#102040FF".into()));
            c.set_prop("BackgroundGradientEnabled", PropValue::Bool(true));
            c.set_prop(
                "BackgroundGradientStartColor",
                PropValue::String("#FF8000FF".into()),
            );
            c.set_prop(
                "BackgroundGradientEndColor",
                PropValue::String("#0080FFFF".into()),
            );
        }
        v.push(c);
    }
    v
}

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

#[derive(Clone, Copy, Debug)]
enum Surface {
    Canvas,
    Preview,
    Run,
}

fn shapes(
    controls: &[Control],
    s: Surface,
    style: GlassStyle,
) -> (Vec<ClippedShape>, HashMap<String, Rect>) {
    let size = Vec2::new(700.0, 500.0);
    let ctx = egui::Context::default();
    set_glass_style(&ctx, style);
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), size));
    input.max_texture_side = Some(8192);
    let mut rects = HashMap::new();
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
                    form_size: Vec2::new(600.0, 400.0),
                    glass: true,
                    mode: match s {
                        Surface::Canvas => RenderMode::Static,
                        _ => RenderMode::Interactive,
                    },
                    active_tabs: &active,
                    backdrop: Backdrop::default(),
                };
                rects = match s {
                    Surface::Canvas => {
                        let painter = ui.painter().clone();
                        render_faces(&painter, ui.min_rect().min, &inp).control_rects
                    }
                    _ => render_form(ui, &inp).control_rects,
                };
            });
    });
    full.textures_delta.clear();
    (full.shapes, rects)
}

// ── coverage: does `shape` paint the pixel centre `p`? ──────────────────────

fn rounded_box_sdf(r: Rect, cr: egui::CornerRadius, p: Pos2) -> f32 {
    let half = r.size() * 0.5;
    let q = p - r.center();
    let stored = if q.x < 0.0 {
        if q.y < 0.0 {
            cr.nw
        } else {
            cr.sw
        }
    } else if q.y < 0.0 {
        cr.ne
    } else {
        cr.se
    } as f32;
    // egui clamps a corner to half the shorter side: the EFFECTIVE radius.
    let rad = stored.min(half.x).min(half.y).max(0.0);
    let qa = Vec2::new(q.x.abs(), q.y.abs()) - (half - Vec2::splat(rad));
    let outside = Vec2::new(qa.x.max(0.0), qa.y.max(0.0)).length();
    let inside = qa.x.max(qa.y).min(0.0);
    outside + inside - rad
}

fn in_triangle(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
    let s = |p1: Pos2, p2: Pos2, p3: Pos2| {
        (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
    };
    let d1 = s(p, a, b);
    let d2 = s(p, b, c);
    let d3 = s(p, c, a);
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

fn seg_dist(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() == 0.0 {
        0.0
    } else {
        ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0)
    };
    (a + ab * t - p).length()
}

fn in_polygon(pts: &[Pos2], p: Pos2) -> bool {
    let mut inside = false;
    let n = pts.len();
    let mut j = n.saturating_sub(1);
    for i in 0..n {
        let (a, b) = (pts[i], pts[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Does `shape` paint `p`? `flags` names the approximations taken on the way.
fn covers(shape: &egui::Shape, p: Pos2, flags: &mut BTreeSet<&'static str>) -> bool {
    use egui::epaint::ColorMode;
    use egui::Shape as S;
    match shape {
        S::Noop => false,
        S::Vec(v) => v.iter().any(|s| covers(s, p, flags)),
        S::Rect(rs) => {
            let d = rounded_box_sdf(rs.rect, rs.corner_radius, p);
            let fill = rs.fill.a() > 0 && d <= 0.0;
            let w = rs.stroke.width;
            let k = match rs.stroke_kind {
                egui::StrokeKind::Inside => -w * 0.5,
                egui::StrokeKind::Middle => 0.0,
                egui::StrokeKind::Outside => w * 0.5,
            };
            let stroke = w > 0.0 && rs.stroke.color.a() > 0 && (d - k).abs() <= w * 0.5;
            fill || stroke
        }
        S::Mesh(m) => {
            let textured = m.texture_id != egui::TextureId::default();
            for tri in m.indices.chunks_exact(3) {
                let (a, b, c) = (
                    &m.vertices[tri[0] as usize],
                    &m.vertices[tri[1] as usize],
                    &m.vertices[tri[2] as usize],
                );
                if in_triangle(p, a.pos, b.pos, c.pos)
                    && (textured || a.color.a() > 0 || b.color.a() > 0 || c.color.a() > 0)
                {
                    if textured {
                        flags.insert("textured");
                    }
                    return true;
                }
            }
            false
        }
        S::Path(ps) => {
            let fill = ps.fill.a() > 0 && ps.closed && in_polygon(&ps.points, p);
            let w = ps.stroke.width;
            let ink = match &ps.stroke.color {
                ColorMode::Solid(c) => c.a() > 0,
                ColorMode::UV(_) => true,
            };
            let mut segs: Vec<(Pos2, Pos2)> =
                ps.points.windows(2).map(|w2| (w2[0], w2[1])).collect();
            if ps.closed && ps.points.len() > 2 {
                segs.push((ps.points[ps.points.len() - 1], ps.points[0]));
            }
            let stroke =
                w > 0.0 && ink && segs.iter().any(|(a, b)| seg_dist(*a, *b, p) <= w * 0.5);
            fill || stroke
        }
        S::Circle(cs) => {
            let d = (p - cs.center).length();
            (cs.fill.a() > 0 && d <= cs.radius)
                || (cs.stroke.width > 0.0
                    && cs.stroke.color.a() > 0
                    && (d - cs.radius).abs() <= cs.stroke.width * 0.5)
        }
        S::LineSegment { points, stroke } => {
            stroke.width > 0.0
                && stroke.color.a() > 0
                && seg_dist(points[0], points[1], p) <= stroke.width * 0.5
        }
        S::Text(_) => {
            let hit = shape.visual_bounding_rect().contains(p);
            if hit {
                flags.insert("text");
            }
            hit
        }
        other => {
            let hit = other.visual_bounding_rect().contains(p);
            if hit {
                flags.insert("bbox");
            }
            hit
        }
    }
}

#[derive(Default, Debug)]
struct Row {
    bleed: usize,
    shapes: usize,
    flags: BTreeSet<&'static str>,
}

fn measure(ct: &ControlType, s: Surface, shadow: bool, style: GlassStyle, dressed: bool) -> Row {
    let (a, rects_a) = shapes(&scene(None, shadow, dressed), s, style);
    let (b, rects_b) = shapes(&scene(Some(ct), shadow, dressed), s, style);
    assert!(b.len() >= a.len(), "{ct:?}: the child cannot remove shapes");
    let inner = rects_b
        .get("IN")
        .or_else(|| rects_a.get("IN"))
        .copied()
        .expect("IN drawn");
    let origin = inner.min - Vec2::new(40.0, 40.0);
    let centre = origin + Vec2::new(40.0 + IN_R, 40.0 + IN_R);
    let child_shapes = &b[a.len()..];
    let mut row = Row {
        shapes: child_shapes.len(),
        ..Default::default()
    };
    // `COBOLT_R7_DUMP=<Type>[,<Type>…]` prints, for those types only, every
    // child shape that is the FIRST to cover a bleeding pixel (all shapes with
    // `COBOLT_R7_DUMP_ALL=1`), with how many pixels it covers.
    let dump = std::env::var("COBOLT_R7_DUMP")
        .ok()
        .map_or(false, |t| t.split(',').any(|t| t.trim() == format!("{ct:?}")));
    let dump_all = std::env::var("COBOLT_R7_DUMP_ALL").is_ok();
    let mut per_shape = vec![0usize; child_shapes.len()];
    for y in 40..80 {
        for x in 40..80 {
            let p = origin + Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            if (p - centre).length() <= IN_R + 0.75 {
                continue; // inside the arc, or within the AA margin
            }
            if let Some(i) = child_shapes
                .iter()
                .position(|cs| cs.clip_rect.contains(p) && covers(&cs.shape, p, &mut row.flags))
            {
                row.bleed += 1;
                per_shape[i] += 1;
            }
        }
    }
    if dump {
        println!(
            "--- {ct:?} {s:?} shadow={shadow} {style:?} dressed={dressed}: origin={origin:?} IN={inner:?}"
        );
        for (i, cs) in child_shapes.iter().enumerate() {
            if per_shape[i] == 0 && !dump_all {
                continue;
            }
            let brief = match &cs.shape {
                egui::Shape::Rect(r) => format!(
                    "Rect {:?} cr={:?} fill={:?} stroke={:?}/{:?}",
                    r.rect, r.corner_radius, r.fill, r.stroke, r.stroke_kind
                ),
                egui::Shape::Mesh(m) => format!(
                    "Mesh {} tris tex={:?} bounds={:?}",
                    m.indices.len() / 3,
                    m.texture_id,
                    cs.shape.visual_bounding_rect()
                ),
                egui::Shape::Text(_) => format!("Text {:?}", cs.shape.visual_bounding_rect()),
                other => {
                    let d = format!("{other:?}");
                    format!("{} {:?}", &d[..d.len().min(60)], other.visual_bounding_rect())
                }
            };
            println!(
                "  [{i:>3}] bleed={:<4} clip={:?} {brief}",
                per_shape[i], cs.clip_rect
            );
        }
    }
    row
}

#[test]
fn every_control_type_is_measured_at_a_rounded_corner() {
    // Per type: total bleed, total shapes, flags, and the worst bleed per surface.
    let mut verdicts: BTreeMap<String, (usize, usize, BTreeSet<&'static str>, [usize; 3])> =
        BTreeMap::new();
    for ct in ControlType::ALL.iter() {
        let name = format!("{ct:?}");
        let mut total_bleed = 0;
        let mut total_shapes = 0;
        let mut flags = BTreeSet::new();
        let mut worst = [0usize; 3];
        for (si, s) in [Surface::Canvas, Surface::Preview, Surface::Run]
            .into_iter()
            .enumerate()
        {
            for shadow in [false, true] {
                for style in [GlassStyle::Classic, GlassStyle::Neumorphic] {
                    for dressed in [false, true] {
                        let row = measure(ct, s, shadow, style, dressed);
                        println!(
                            "R7 {name:<14} {s:<8?} shadow={:<3} style={:<10} dressed={:<3} bleed_px={:<4} shapes={:<4} flags={:?}",
                            if shadow { "on" } else { "off" },
                            format!("{style:?}"),
                            if dressed { "yes" } else { "no" },
                            row.bleed,
                            row.shapes,
                            row.flags
                        );
                        total_bleed += row.bleed;
                        total_shapes += row.shapes;
                        worst[si] = worst[si].max(row.bleed);
                        flags.extend(row.flags);
                    }
                }
            }
        }
        verdicts.insert(name, (total_bleed, total_shapes, flags, worst));
    }
    println!();
    let mut measured_self_clips = BTreeSet::new();
    println!("R7-TABLE | Type | Canvas | Preview | Run | Verdict |");
    println!("R7-TABLE |---|---|---|---|---|");
    for (name, (bleed, shapes, flags, worst)) in &verdicts {
        let verdict = if *shapes == 0 {
            "paints nothing"
        } else if *bleed == 0 {
            "stays inside the arc"
        } else {
            "paints past the arc"
        };
        if *bleed == 0 {
            measured_self_clips.insert(name.clone());
        }
        println!(
            "R7-VERDICT {name:<14} {verdict:<22} bleed_px={bleed:<5} shapes={shapes:<5} flags={flags:?}"
        );
        println!(
            "R7-TABLE | {name} | {} | {} | {} | {verdict} |",
            worst[0], worst[1], worst[2]
        );
    }
    assert_eq!(verdicts.len(), ControlType::ALL.len(), "every type measured");

    // R7: the allow-list the notch rule trusts is exactly what was measured.
    let listed: BTreeSet<String> = ControlType::ALL
        .iter()
        .filter(|ct| self_clipping_type(ct))
        .map(|ct| format!("{ct:?}"))
        .collect();
    let missing: Vec<_> = measured_self_clips.difference(&listed).collect();
    let stale: Vec<_> = listed.difference(&measured_self_clips).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "render::self_clipping_type must equal the measurement — measured to stay \
         inside the arc but not listed: {missing:?}; listed but measured to paint \
         past the arc: {stale:?}"
    );
}
