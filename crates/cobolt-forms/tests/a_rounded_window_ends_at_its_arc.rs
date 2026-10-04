#![cfg(feature = "render")]
//! **A rounded window ends at its arc** (form `CornerRadius`, operator
//! 2026-10-03) — measured, not asserted, the way spec 057's R7 harness
//! measures a rounded Panel.
//!
//! A window without a title bar may round its corners. The host makes it
//! see-through, so every pixel the engine paints outside the arc shows as a
//! square corner over the desktop. Nothing can repaint "the desktop" — the
//! notch mask's repair is impossible here — so the corner has to be DRAWN
//! round by every layer that reaches it:
//!
//! 1. the backdrop — colour, gradient and picture in every mode;
//! 2. every control type sitting in the corner, which is handed the window's
//!    arc as its container clip exactly as the child of a rounded Panel is.
//!
//! Part 1 asserts the backdrop paints nothing outside the arc and does paint
//! inside it. Part 2 prints one line per (type, surface, shadow, style,
//! dressed) and a verdict per type, then asserts that the types measured to
//! stay inside the window's arc are exactly `render::self_clipping_type` — the
//! same allow-list the Panel measurement settles.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{BgImageMode, GlassStyle, Rect as MRect};
use cobolt_forms::paint::set_glass_style;
use cobolt_forms::render::{
    merge_props, render_faces, render_form, self_clipping_type, window_arc, Backdrop,
    DesignedState, FormState, RenderInput, RenderMode,
};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::epaint::ClippedShape;
use egui::{pos2, Pos2, Rect, Vec2};

const R: f32 = 40.0;
const W: f32 = 600.0;
const H: f32 = 400.0;

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

#[derive(Clone, Copy, Debug)]
enum Back {
    Colour,
    Gradient,
    Image(BgImageMode),
}

fn backdrop(back: Back, tex: Option<(egui::TextureId, Vec2)>, rounded: bool) -> Backdrop {
    let mut b = Backdrop {
        color_hex: "#E0E4ECFF".into(),
        window_size: Some(Vec2::new(W, H)),
        window: if rounded {
            window_arc(Rect::from_min_size(Pos2::ZERO, Vec2::new(W, H)), R as u32)
        } else {
            None
        },
        ..Default::default()
    };
    match back {
        Back::Colour => {}
        Back::Gradient => {
            b.gradient_enabled = true;
            b.gradient_start_hex = "#FF8000FF".into();
            b.gradient_end_hex = "#0080FFFF".into();
        }
        Back::Image(mode) => {
            b.image = tex;
            b.image_mode = mode;
        }
    }
    b
}

/// Render `controls` on a W×H window. Returns the shapes and control rects.
fn shapes(
    controls: &[Control],
    s: Surface,
    style: GlassStyle,
    back: Back,
    rounded: bool,
) -> (Vec<ClippedShape>, HashMap<String, Rect>) {
    let size = Vec2::new(W, H);
    let ctx = egui::Context::default();
    set_glass_style(&ctx, style);
    // A picture smaller than the window, so Fit, Center and Tile all leave
    // corners the picture itself does not reach — and one Stretch covers.
    let tex = ctx.load_texture(
        "rounded-window-test",
        egui::ColorImage::new([64, 48], vec![egui::Color32::from_rgb(200, 40, 40); 64 * 48]),
        egui::TextureOptions::LINEAR,
    );
    let tex = Some((tex.id(), Vec2::new(64.0, 48.0)));
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
                    form_size: size,
                    glass: true,
                    mode: match s {
                        Surface::Canvas => RenderMode::Static,
                        _ => RenderMode::Interactive,
                    },
                    active_tabs: &active,
                    backdrop: backdrop(back, tex, rounded),
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

// ── coverage: does `shape` paint the pixel centre `p`? (as spec 057's R7) ────

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
    let rad = stored.min(half.x).min(half.y).max(0.0);
    let qa = Vec2::new(q.x.abs(), q.y.abs()) - (half - Vec2::splat(rad));
    let outside = Vec2::new(qa.x.max(0.0), qa.y.max(0.0)).length();
    let inside = qa.x.max(qa.y).min(0.0);
    outside + inside - rad
}

fn in_triangle(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
    let s = |p1: Pos2, p2: Pos2, p3: Pos2| (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y);
    let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
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

fn covers(shape: &egui::Shape, p: Pos2, flags: &mut BTreeSet<&'static str>) -> bool {
    use egui::epaint::ColorMode;
    use egui::Shape as S;
    match shape {
        S::Noop => false,
        S::Vec(v) => v.iter().any(|s| covers(s, p, flags)),
        S::Rect(rs) => {
            let d = rounded_box_sdf(rs.rect, rs.corner_radius, p);
            let textured = rs.brush.is_some();
            let fill = (rs.fill.a() > 0 || textured) && d <= 0.0;
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
            let mut segs: Vec<(Pos2, Pos2)> = ps.points.windows(2).map(|w2| (w2[0], w2[1])).collect();
            if ps.closed && ps.points.len() > 2 {
                segs.push((ps.points[ps.points.len() - 1], ps.points[0]));
            }
            let stroke = w > 0.0 && ink && segs.iter().any(|(a, b)| seg_dist(*a, *b, p) <= w * 0.5);
            fill || stroke
        }
        S::Circle(cs) => {
            let d = (p - cs.center).length();
            (cs.fill.a() > 0 && d <= cs.radius)
                || (cs.stroke.width > 0.0 && cs.stroke.color.a() > 0 && (d - cs.radius).abs() <= cs.stroke.width * 0.5)
        }
        S::LineSegment { points, stroke } => {
            stroke.width > 0.0 && stroke.color.a() > 0 && seg_dist(points[0], points[1], p) <= stroke.width * 0.5
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

/// The four arc centres of the window, with the corner square each owns.
fn corners() -> [(Pos2, Rect); 4] {
    [
        (pos2(R, R), Rect::from_min_size(pos2(0.0, 0.0), Vec2::splat(R))),
        (pos2(W - R, R), Rect::from_min_size(pos2(W - R, 0.0), Vec2::splat(R))),
        (pos2(R, H - R), Rect::from_min_size(pos2(0.0, H - R), Vec2::splat(R))),
        (pos2(W - R, H - R), Rect::from_min_size(pos2(W - R, H - R), Vec2::splat(R))),
    ]
}

/// Pixels of `shapes` outside the window's arc (beyond the AA margin), and
/// pixels inside the arc but within its corner squares.
fn outside_and_inside(shapes: &[ClippedShape], flags: &mut BTreeSet<&'static str>) -> (usize, usize) {
    let (mut out, mut inn) = (0, 0);
    for (centre, square) in corners() {
        for y in square.min.y as i32..square.max.y as i32 {
            for x in square.min.x as i32..square.max.x as i32 {
                let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                let d = (p - centre).length();
                let mut hit = || shapes.iter().any(|cs| cs.clip_rect.contains(p) && covers(&cs.shape, p, flags));
                if d > R + 0.75 {
                    if hit() {
                        out += 1;
                    }
                } else if d < R - 0.75 && hit() {
                    inn += 1;
                }
            }
        }
    }
    (out, inn)
}

// ── 1. the backdrop ─────────────────────────────────────────────────────────

#[test]
fn the_backdrop_of_a_rounded_window_stops_at_its_arc() {
    let backs = [
        Back::Colour,
        Back::Gradient,
        Back::Image(BgImageMode::Stretch),
        Back::Image(BgImageMode::Fill),
        Back::Image(BgImageMode::Fit),
        Back::Image(BgImageMode::Center),
        Back::Image(BgImageMode::Tile),
    ];
    let mut failures = Vec::new();
    for back in backs {
        for s in [Surface::Preview, Surface::Run] {
            let mut flags = BTreeSet::new();
            let (square, _) = shapes(&[], s, GlassStyle::Classic, back, false);
            let (sq_out, _) = outside_and_inside(&square, &mut flags);
            let (round, _) = shapes(&[], s, GlassStyle::Classic, back, true);
            let (out, inside) = outside_and_inside(&round, &mut flags);
            println!(
                "WINDOW-BACKDROP {back:<18} {s:<8?} square: {sq_out:>5} px past the arc | rounded: {out:>3} px past, {inside:>5} px inside  {flags:?}",
                back = format!("{back:?}")
            );
            assert!(sq_out > 0, "{back:?}: the square window must cover its corners (the measure is live)");
            if out > 0 || inside == 0 {
                failures.push(format!("{back:?} {s:?}: {out} px past the arc, {inside} px inside it"));
            }
        }
    }
    assert!(failures.is_empty(), "a rounded window's backdrop must stop at its arc: {failures:?}");
}

// ── 2. every control type in the corner ─────────────────────────────────────

fn scene(child: Option<&ControlType>, shadow: bool, dressed: bool) -> Vec<Control> {
    let Some(ct) = child else { return Vec::new() };
    let mut c = Control::new("C", ct.clone(), 0, 0);
    c.rect = MRect::new(0, 0, 160, 120);
    c.set_prop("ShadowEnabled", PropValue::Bool(shadow));
    if dressed {
        c.set_prop("BackgroundColor", PropValue::String("#3060C0FF".into()));
        c.set_prop("BorderStyle", PropValue::String("Single".into()));
        c.set_prop("BorderWidth", PropValue::Int(1));
        c.set_prop("BorderColor", PropValue::String("#102040FF".into()));
        c.set_prop("BackgroundGradientEnabled", PropValue::Bool(true));
        c.set_prop("BackgroundGradientStartColor", PropValue::String("#FF8000FF".into()));
        c.set_prop("BackgroundGradientEndColor", PropValue::String("#0080FFFF".into()));
    }
    vec![c]
}

fn bleed(ct: &ControlType, s: Surface, shadow: bool, style: GlassStyle, dressed: bool) -> (usize, usize, BTreeSet<&'static str>) {
    let (a, _) = shapes(&scene(None, shadow, dressed), s, style, Back::Colour, true);
    let (b, _) = shapes(&scene(Some(ct), shadow, dressed), s, style, Back::Colour, true);
    assert!(b.len() >= a.len(), "{ct:?}: the control cannot remove shapes");
    let mine = &b[a.len()..];
    let mut flags = BTreeSet::new();
    let (out, _) = outside_and_inside(mine, &mut flags);
    // `COBOLT_WINDOW_DUMP=<Type>` prints the shapes of that type that paint
    // past the window's arc, one per line.
    if out > 0
        && std::env::var("COBOLT_WINDOW_DUMP").is_ok_and(|t| t.split(',').any(|t| t.trim() == format!("{ct:?}")))
    {
        println!("--- {ct:?} {s:?} shadow={shadow} {style:?} dressed={dressed}");
        for cs in mine {
            let mut f = BTreeSet::new();
            let (o, _) = outside_and_inside(std::slice::from_ref(cs), &mut f);
            if o > 0 {
                let brief = match &cs.shape {
                    egui::Shape::Rect(r) => format!(
                        "Rect {:?} cr={:?} fill={:?} stroke={:?}/{:?}",
                        r.rect, r.corner_radius, r.fill, r.stroke, r.stroke_kind
                    ),
                    other => {
                        let d = format!("{other:?}");
                        format!("{} {:?}", &d[..d.len().min(80)], other.visual_bounding_rect())
                    }
                };
                println!("  bleed={o:<4} clip={:?} {brief}", cs.clip_rect);
            }
        }
    }
    (out, mine.len(), flags)
}

#[test]
fn every_control_type_is_measured_at_a_rounded_window_corner() {
    let mut verdicts: BTreeMap<String, (usize, usize, [usize; 3])> = BTreeMap::new();
    for ct in ControlType::ALL.iter() {
        let name = format!("{ct:?}");
        let (mut total, mut shapes_n, mut worst) = (0, 0, [0usize; 3]);
        for (si, s) in [Surface::Canvas, Surface::Preview, Surface::Run].into_iter().enumerate() {
            for shadow in [false, true] {
                for style in [GlassStyle::Classic, GlassStyle::Neumorphic] {
                    for dressed in [false, true] {
                        let (b, n, flags) = bleed(ct, s, shadow, style, dressed);
                        println!(
                            "WINDOW {name:<14} {s:<8?} shadow={:<3} style={:<10} dressed={:<3} bleed_px={b:<4} shapes={n:<4} flags={flags:?}",
                            if shadow { "on" } else { "off" },
                            format!("{style:?}"),
                            if dressed { "yes" } else { "no" },
                        );
                        total += b;
                        shapes_n += n;
                        worst[si] = worst[si].max(b);
                    }
                }
            }
        }
        verdicts.insert(name, (total, shapes_n, worst));
    }
    println!("WINDOW-TABLE | Type | Canvas | Preview | Run | Verdict |");
    println!("WINDOW-TABLE |---|---|---|---|---|");
    let mut inside = BTreeSet::new();
    for (name, (b, n, worst)) in &verdicts {
        let verdict = if *n == 0 {
            "paints nothing"
        } else if *b == 0 {
            "stays inside the arc"
        } else {
            "paints past the arc"
        };
        if *b == 0 {
            inside.insert(name.clone());
        }
        println!("WINDOW-TABLE | {name} | {} | {} | {} | {verdict} |", worst[0], worst[1], worst[2]);
    }
    // The one difference from a Panel is geometry, not a frame: a Panel's
    // children sit inside its border inset, a window has none. A TreeView's
    // FRAME stays inside the window's arc like any other; its CONTENT — the
    // first guide line and expander, drawn 4 px from the control's corner —
    // crosses the arc when the tree is flush with a rounded window's corner.
    // Inside a Panel the inset keeps the same content clear of the arc, which
    // is why the Panel measurement lists it.
    const CONTENT_PAST_A_FLUSH_WINDOW_ARC: [&str; 1] = ["TreeView"];
    let listed: BTreeSet<String> = ControlType::ALL
        .iter()
        .filter(|ct| self_clipping_type(ct))
        .map(|ct| format!("{ct:?}"))
        .filter(|n| !CONTENT_PAST_A_FLUSH_WINDOW_ARC.contains(&n.as_str()))
        .collect();
    let missing: Vec<_> = inside.difference(&listed).collect();
    let stale: Vec<_> = listed.difference(&inside).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "at a rounded WINDOW's corner, the types that stay inside the arc must be the \
         same allow-list as at a rounded Panel's — inside but not listed: {missing:?}; \
         listed but painting past the window's arc: {stale:?}"
    );
}

// ── 3. a shell window: pieces of one rounded window ─────────────────────────

/// A shell application's window is three pieces — the breadcrumb strip across
/// the top, the rail down the left, the ContentPane — and each fills only its
/// own part of the window. Each is painted through the shared helpers with the
/// WINDOW's arc, and between them they must fill the window up to its arc and
/// paint nothing past it. The strip (36 px) and a collapsed rail (48 px) are
/// too small to hold a 40 px corner, which is the case that needs rows.
#[test]
fn the_pieces_of_a_shell_window_meet_its_arc() {
    use cobolt_forms::paint::{fill_in_clip, gradient_in_clip, image_in_clip};
    let window = Rect::from_min_size(Pos2::ZERO, Vec2::new(W, H));
    let arc = window_arc(window, R as u32);
    let strip = Rect::from_min_max(pos2(0.0, 0.0), pos2(W, 36.0));
    let rail = Rect::from_min_max(pos2(0.0, 36.0), pos2(48.0, H));
    let pane = Rect::from_min_max(pos2(48.0, 36.0), pos2(W, H));
    for (what, layer) in [("colour", 0), ("gradient", 1), ("picture", 2)] {
        let ctx = egui::Context::default();
        let tex = ctx.load_texture(
            "shell-pieces",
            egui::ColorImage::new([8, 8], vec![egui::Color32::from_rgb(40, 160, 40); 64]),
            egui::TextureOptions::LINEAR,
        );
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(window);
        let full = ctx.run_ui(input, |ui| {
            let painter = ui.painter().clone();
            for piece in [strip, rail, pane] {
                match layer {
                    0 => fill_in_clip(&painter, piece, egui::Color32::from_rgb(30, 60, 200), arc),
                    1 => gradient_in_clip(
                        &painter,
                        piece,
                        egui::Color32::RED,
                        egui::Color32::BLUE,
                        "South",
                        arc,
                    ),
                    _ => image_in_clip(
                        &painter,
                        tex.id(),
                        piece,
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                        arc,
                    ),
                }
            }
        });
        let mut full = full;
        full.textures_delta.clear();
        let mut flags = BTreeSet::new();
        let (out, inside) = outside_and_inside(&full.shapes, &mut flags);
        // Every corner pixel inside the arc, beyond the AA margin.
        let mut want = 0;
        for (centre, square) in corners() {
            for y in square.min.y as i32..square.max.y as i32 {
                for x in square.min.x as i32..square.max.x as i32 {
                    let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                    if (p - centre).length() < R - 0.75 {
                        want += 1;
                        if !full.shapes.iter().any(|cs| cs.clip_rect.contains(p) && covers(&cs.shape, p, &mut flags)) {
                            println!("SHELL-PIECES {what}: uncovered {p:?}");
                        }
                    }
                }
            }
        }
        println!("SHELL-PIECES {what:<8} {out:>3} px past the arc, {inside:>5}/{want} px inside covered");
        assert_eq!(out, 0, "{what}: the shell's pieces paint past the window's arc");
        assert_eq!(inside, want, "{what}: the shell's pieces leave a gap inside the window's arc");
    }
}
