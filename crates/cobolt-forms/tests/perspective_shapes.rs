// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 096, task T6 — **a painted control, tilted through the real painter.**
//!
//! `PerspectiveScope` lifts the shapes a control has just painted off the paint
//! list, projects them and puts them back. This file draws the controls with the
//! real `draw_control` and checks what ends up on the list:
//!
//! * at the neutral triple nothing is read and the list is exactly what it was
//!   (R6 / AC2) — for each of the twelve types;
//! * a tilted BarChart, PictureBox, Label, Shape and Line stay whole: every
//!   vertex is the projection of the vertex the painter made, none is lost,
//!   none is NaN, no glyph quad collapses, and the outline sits on the quad (R7 / AC5);
//! * an image's four corners land on the quad's four corners and a texel halfway
//!   along an edge lands halfway *in perspective*, not halfway on the screen;
//! * none of the twelve paints an egui paint callback, which cannot be projected.
//!
//! Spec 096 AC2 (R6), AC5 (R7).

#![cfg(feature = "render")]

use cobolt_forms::model::{Control, ControlType, PropValue};
use cobolt_forms::paint::draw_control;
use cobolt_forms::perspective::{quad_contains, quad_is_convex, Perspective, PerspectiveScope};
use egui::epaint::{ClippedShape, Mesh, Primitive, Shape, Tessellator};
use egui::{pos2, Pos2, Rect, TextureId};

const ORIGIN: Pos2 = pos2(60.0, 40.0);

/// The twelve, each with something to look at.
fn the_twelve() -> Vec<(&'static str, Control)> {
    let mk = |ct: ControlType, w: i32, h: i32| {
        let mut c = Control::new("C-1", ct, 0, 0);
        c.rect = cobolt_forms::Rect::new(0, 0, w, h);
        c
    };
    let mut label = mk(ControlType::Label, 200, 30);
    label.set_prop("Caption", PropValue::String("Revenue by quarter".into()));
    vec![
        ("BarChart", mk(ControlType::BarChart, 320, 220)),
        ("LineChart", mk(ControlType::LineChart, 320, 220)),
        ("PieChart", mk(ControlType::PieChart, 240, 220)),
        ("AreaChart", mk(ControlType::AreaChart, 320, 220)),
        ("ScatterChart", mk(ControlType::ScatterChart, 320, 220)),
        ("DonutChart", mk(ControlType::DonutChart, 240, 220)),
        ("RadarChart", mk(ControlType::RadarChart, 240, 240)),
        ("PictureBox", mk(ControlType::PictureBox, 200, 140)),
        ("Animator", mk(ControlType::Animator, 160, 120)),
        ("Label", label),
        ("Line", mk(ControlType::Line, 160, 4)),
        ("Shape", mk(ControlType::Shape, 160, 90)),
    ]
}

fn plane_of(c: &Control) -> Rect {
    Rect::from_min_size(ORIGIN, egui::vec2(c.rect.w as f32, c.rect.h as f32))
}

fn raw() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(640.0, 420.0))),
        ..Default::default()
    }
}

/// The shapes `draw_control` leaves on the layer for `ctrl`, with the control
/// wrapped in a `PerspectiveScope` for `tilt` (`None` = no tilt).
fn paint(ctrl: &Control, tilt: Option<Perspective>, tex: Option<TextureId>) -> (Vec<ClippedShape>, egui::Context) {
    let ctx = egui::Context::default();
    let mut shapes = Vec::new();
    // Two frames: the first lays the glyph atlas out.
    for _ in 0..2 {
        let mut full = ctx.run_ui(raw(), |root_ui| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(root_ui, |ui| {
                let layer = ui.layer_id();
                {
                    let _scope = PerspectiveScope::enter(ui.painter(), plane_of(ctrl), tilt);
                    draw_control(ui.painter(), ORIGIN, ctrl, false, true, 1.0, 1.0, tex);
                }
                shapes = ctx.graphics(|g| {
                    g.get(layer).map(|l| l.all_entries().cloned().collect()).unwrap_or_default()
                });
            });
        });
        full.textures_delta.clear();
    }
    (shapes, ctx)
}

/// Every mesh a list of shapes tessellates to, with its clip rectangle.
fn meshes(ctx: &egui::Context, shapes: &[ClippedShape]) -> Vec<(Mesh, Rect)> {
    let mut opts = ctx.tessellation_options(|o| *o);
    opts.coarse_tessellation_culling = false;
    let size = ctx.fonts(|f| f.font_image_size());
    let mut t = Tessellator::new(ctx.pixels_per_point(), opts, size, Vec::new());
    let mut prims = Vec::new();
    for s in shapes {
        t.tessellate_clipped_shape(s.clone(), &mut prims);
    }
    prims
        .into_iter()
        .filter_map(|p| match p.primitive {
            Primitive::Mesh(m) => Some((m, p.clip_rect)),
            Primitive::Callback(_) => None,
        })
        .collect()
}

fn has_callback(shapes: &[ClippedShape]) -> bool {
    fn walk(s: &Shape) -> bool {
        match s {
            Shape::Callback(_) => true,
            Shape::Vec(v) => v.iter().any(walk),
            _ => false,
        }
    }
    shapes.iter().any(|c| walk(&c.shape))
}

fn triangles(m: &Mesh) -> impl Iterator<Item = [Pos2; 3]> + '_ {
    m.indices
        .chunks_exact(3)
        .map(|t| [m.vertices[t[0] as usize].pos, m.vertices[t[1] as usize].pos, m.vertices[t[2] as usize].pos])
}

fn area([a, b, c]: [Pos2; 3]) -> f32 {
    ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() * 0.5
}

#[test]
fn at_the_neutral_triple_the_list_is_exactly_what_it_was() {
    println!("\n  ── R6: neutral values, the paint list with and without the scope ──");
    println!("  {:<13} {:>7} {:>9}", "control", "shapes", "identical");
    for (name, ctrl) in the_twelve() {
        assert_eq!(Perspective::of(&ctrl), None, "{name}: a fresh control is not tilted");
        let (plain, _) = paint(&ctrl, None, None);
        // The scope is entered with what `Perspective::of` answers for the control.
        let (scoped, _) = paint(&ctrl, Perspective::of(&ctrl), None);
        println!("  {name:<13} {:>7} {:>9}", plain.len(), plain == scoped);
        assert!(!plain.is_empty(), "{name}: painted nothing");
        assert_eq!(plain, scoped, "{name}: the scope changed the list at the defaults");
        assert!(!has_callback(&plain), "{name}: paints a Shape::Callback, which cannot be projected");
    }
}

#[test]
fn a_tilted_control_keeps_every_vertex_and_loses_none() {
    let tilt = Perspective::new(8, -30, 60);
    println!("\n  ── R7: RotateX 8, RotateY −30, distance 60  (* vertices the painter put >3 px past the designed rectangle) ──");
    println!(
        "  {:<13} {:>9} {:>9} {:>6} {:>8} {:>9}",
        "control", "triangles", "(plane)", "NaN", "off hull", "overhang*"
    );
    for (name, ctrl) in the_twelve() {
        let plane = plane_of(&ctrl);
        let (before, ctx) = paint(&ctrl, None, None);
        let (after, _) = paint(&ctrl, Some(tilt), None);
        let quad = tilt.quad(plane);
        assert!(quad_is_convex(&quad));

        let plane_meshes = meshes(&ctx, &before);
        let tilted_meshes = meshes(&ctx, &after);
        let plane_tris: usize = plane_meshes.iter().map(|(m, _)| m.indices.len() / 3).sum();
        let tilted_tris: usize = tilted_meshes.iter().map(|(m, _)| m.indices.len() / 3).sum();

        // Nothing lost: a projected control has at least the triangles it had.
        assert!(tilted_tris >= plane_tris, "{name}: {tilted_tris} triangles after, {plane_tris} before");

        // Finite, and no triangle collapsed that was not already empty in egui's own
        // tessellation of the plane (a stroke's cap can emit one).
        let count_empty = |ms: &[(Mesh, Rect)]| -> usize {
            ms.iter().map(|(m, _)| triangles(m).filter(|t| area(*t) <= 0.0).count()).sum()
        };
        let (empty_before, empty_after) = (count_empty(&plane_meshes), count_empty(&tilted_meshes));
        let mut nan = 0usize;
        let mut outside = 0usize;
        // The outline holds the picture. The painter may draw past the designed rectangle (a
        // caption that overhangs, the feather); a projection keeps what lies inside a convex
        // region inside its image, so every vertex the tilt makes — including the ones it adds by
        // cutting — lies inside the projection of the rectangle that bounds what the painter drew.
        let drawn = Rect::from_points(
            &plane_meshes.iter().flat_map(|(m, _)| m.vertices.iter().map(|v| v.pos)).collect::<Vec<_>>(),
        );
        let cam = tilt.camera(plane);
        let hull = [
            cam.project(drawn.left_top()),
            cam.project(drawn.right_top()),
            cam.project(drawn.right_bottom()),
            cam.project(drawn.left_bottom()),
        ];
        let overhang: usize = plane_meshes
            .iter()
            .map(|(m, _)| m.vertices.iter().filter(|v| !plane.expand(3.0).contains(v.pos)).count())
            .sum();
        for (m, _) in &tilted_meshes {
            for v in &m.vertices {
                if !(v.pos.x.is_finite() && v.pos.y.is_finite()) {
                    nan += 1;
                }
                // 0.01 px for rounding in the projection.
                if !quad_contains(&hull, v.pos) && !hull.iter().any(|h| (*h - v.pos).length() < 0.01) {
                    let nearest = (0..4)
                        .map(|i| {
                            let (a, b) = (hull[i], hull[(i + 1) % 4]);
                            let ab = b - a;
                            let t = ((v.pos - a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
                            (v.pos - (a + ab * t)).length()
                        })
                        .fold(f32::MAX, f32::min);
                    if nearest > 0.01 {
                        outside += 1;
                    }
                }
            }
        }
        let outside_before = overhang;
        let collapsed = empty_after;
        println!(
            "  {name:<13} {tilted_tris:>9} {plane_tris:>9} {nan:>6} {outside:>8} {outside_before:>9}   empty triangles {empty_before} → {collapsed}"
        );
        assert_eq!(nan, 0, "{name}: NaN in the projected meshes");
        assert!(collapsed <= empty_before, "{name}: the projection collapsed {} triangles", collapsed - empty_before);
        assert_eq!(outside, 0, "{name}: {outside} vertices lie outside the projected bounds of what was drawn");
        assert!(!has_callback(&after), "{name}: paints a callback");
    }
}

#[test]
fn every_vertex_is_the_projection_of_the_vertex_the_painter_made() {
    // The strongest statement of "straight edges stay straight": a projection keeps lines
    // straight, so if each vertex is where the camera puts the plane's vertex, an edge
    // that was a straight line still is. Checked on the controls whose geometry is plain.
    let tilt = Perspective::new(8, -30, 60);
    println!("\n  ── each projected vertex against the camera, vertex for vertex ──");
    for name in ["Shape", "Line", "BarChart"] {
        let (_, ctrl) = the_twelve().into_iter().find(|(n, _)| *n == name).unwrap();
        let plane = plane_of(&ctrl);
        let cam = tilt.camera(plane);
        let (before, ctx) = paint(&ctrl, None, None);
        let (after, _) = paint(&ctrl, Some(tilt), None);
        let (plane_meshes, tilted_meshes) = (meshes(&ctx, &before), meshes(&ctx, &after));
        // The projected meshes begin with the plane meshes' vertices, in order, moved.
        let mut worst = 0.0f32;
        let mut compared = 0usize;
        let mut p = plane_meshes.iter();
        let mut t = tilted_meshes.iter();
        while let (Some((pm, _)), Some((tm, _))) = (p.next(), t.next()) {
            for (pv, tv) in pm.vertices.iter().zip(&tm.vertices) {
                worst = worst.max((cam.project(pv.pos) - tv.pos).length());
                compared += 1;
            }
        }
        println!("  {name:<10} {compared:>6} vertices compared, worst difference {worst:.6} px");
        assert!(compared > 0);
        assert!(worst < 1e-3, "{name}: a vertex is {worst} px from where the camera puts it");
    }
}

#[test]
fn a_shapes_outline_lands_on_the_quads_corners() {
    let tilt = Perspective::new(0, -30, 50);
    let (_, ctrl) = the_twelve().into_iter().find(|(n, _)| *n == "Shape").unwrap();
    let plane = plane_of(&ctrl);
    let quad = tilt.quad(plane);
    let (after, ctx) = paint(&ctrl, Some(tilt), None);
    let all: Vec<Pos2> = meshes(&ctx, &after).iter().flat_map(|(m, _)| m.vertices.iter().map(|v| v.pos)).collect();
    println!("\n  Shape at RotateY −30: nearest vertex to each corner of the projected outline");
    for (i, corner) in quad.iter().enumerate() {
        let (d, _) = all.iter().map(|p| ((*p - *corner).length(), p)).fold((f32::MAX, &all[0]), |a, b| if b.0 < a.0 { b } else { a });
        println!("    corner {i}: {d:.3} px");
        assert!(d < 1.5, "corner {i} of the outline is {d} px from any vertex");
    }
}

#[test]
fn an_image_lands_on_the_outline_and_in_perspective() {
    let tex = TextureId::User(9);
    let tilt = Perspective::new(0, -30, 50);
    let (_, ctrl) = the_twelve().into_iter().find(|(n, _)| *n == "PictureBox").unwrap();
    let plane = plane_of(&ctrl);
    let cam = tilt.camera(plane);
    let quad = tilt.quad(plane);
    let (after, ctx) = paint(&ctrl, Some(tilt), Some(tex));
    let image: Vec<Mesh> = meshes(&ctx, &after).into_iter().map(|(m, _)| m).filter(|m| m.texture_id == tex).collect();
    assert_eq!(image.len(), 1, "one image mesh keeps its texture");
    let image = &image[0];
    println!("\n  PictureBox image at RotateY −30: {} triangles", image.indices.len() / 3);

    // The four corners of the image (UV 0,0 · 1,0 · 1,1 · 0,1) are the four corners of the outline.
    for (uv, corner, name) in [
        (pos2(0.0, 0.0), quad[0], "top-left"),
        (pos2(1.0, 0.0), quad[1], "top-right"),
        (pos2(1.0, 1.0), quad[2], "bottom-right"),
        (pos2(0.0, 1.0), quad[3], "bottom-left"),
    ] {
        let v = image
            .vertices
            .iter()
            .find(|v| (v.uv - uv).length() < 1e-4)
            .unwrap_or_else(|| panic!("no vertex carries the {name} texel"));
        println!("    {name:<13} texel lands {:.4} px from the outline corner", (v.pos - corner).length());
        assert!((v.pos - corner).length() < 0.01, "{name}");
    }

    // Every texel lands where the camera puts its point of the designed rectangle: the image
    // is mapped perspective-correctly, not stretched affinely between the four corners.
    let mut worst = 0.0f32;
    for v in &image.vertices {
        let on_plane = pos2(plane.left() + v.uv.x * plane.width(), plane.top() + v.uv.y * plane.height());
        worst = worst.max((cam.project(on_plane) - v.pos).length());
    }
    println!("    {} vertices: worst distance from where the camera puts its texel {worst:.5} px", image.vertices.len());
    assert!(worst < 0.01, "a texel is {worst} px from its place in perspective");

    // The vertex nearest the middle of the TOP edge, against where an affine stretch would put it.
    let mid = image
        .vertices
        .iter()
        .filter(|v| v.uv.y.abs() < 1e-4)
        .min_by(|a, b| (a.uv.x - 0.5).abs().total_cmp(&(b.uv.x - 0.5).abs()))
        .expect("the top edge carries vertices");
    let affine_x = quad[0].x + (quad[1].x - quad[0].x) * mid.uv.x;
    println!(
        "    top edge, texel u = {:.4}: lands at x = {:.2}; an affine stretch would put it at x = {:.2}",
        mid.uv.x, mid.pos.x, affine_x
    );
    assert!((mid.pos.x - affine_x).abs() > 1.0, "the mapping must not be affine");
}

#[test]
fn a_label_is_painted_in_perspective_as_glyph_quads() {
    let tilt = Perspective::new(10, -25, 50);
    let (_, ctrl) = the_twelve().into_iter().find(|(n, _)| *n == "Label").unwrap();
    let plane = plane_of(&ctrl);
    let (before, ctx) = paint(&ctrl, None, None);
    let (after, _) = paint(&ctrl, Some(tilt), None);
    let text_before: usize = before.iter().map(|c| matches!(c.shape, Shape::Text(_)) as usize).sum();
    let text_after: usize = after.iter().map(|c| matches!(c.shape, Shape::Text(_)) as usize).sum();
    // A label paints a caption and (here) nothing else with glyphs: all its triangles are glyph quads.
    let after_meshes = meshes(&ctx, &after);
    let glyph_tris: usize = after_meshes.iter().map(|(m, _)| m.indices.len() / 3).sum();
    let empty: usize = after_meshes.iter().map(|(m, _)| triangles(m).filter(|t| area(*t) <= 0.0).count()).sum();
    assert_eq!(empty, 0, "a glyph quad collapsed");
    println!(
        "\n  Label \"Revenue by quarter\": {text_before} text shape before, {text_after} after (now glyph meshes), \
         {glyph_tris} glyph triangles, outline {:?}",
        quad_is_convex(&tilt.quad(plane))
    );
    assert!(text_before >= 1, "the label paints text untilted");
    assert_eq!(text_after, 0, "…and no Shape::Text survives a tilt: it has become glyph quads");
    assert!(glyph_tris >= 2 * "Revenue by quarter".chars().filter(|c| !c.is_whitespace()).count());
}
