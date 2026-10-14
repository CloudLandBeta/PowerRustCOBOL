// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 096, task T1 — the gate: **can the shapes a control has just painted be
//! lifted off the paint list, tessellated by us, and put back as meshes without
//! the frame changing?**
//!
//! The 3D-perspective feature projects every vertex of what a control painted.
//! egui hands vertices out only after its own tessellation pass, so the feature
//! stands or falls on this: tessellate the captured shapes with
//! `epaint::Tessellator` outside egui's frame loop and get what egui itself
//! gets — same vertices, same positions, same glyph UVs.
//!
//! Three questions, each answered with numbers:
//!
//! 1. **Capture** — `ctx.graphics` exposes the shapes a painter added since an
//!    index (`PaintList::next_idx` / `all_entries`).
//! 2. **Same triangles** — our `Tessellator` over the captured shapes gives, vertex
//!    for vertex, what `ctx.tessellate` gives for the same shapes.
//! 3. **Put back** — replacing each captured shape in the live frame by the mesh
//!    we made renders the same triangles as not touching it.
//!
//! Run with:
//! `cargo test -p cobolt-forms --features render --test perspective_spike -- --nocapture`

#![cfg(feature = "render")]

use cobolt_forms::model::{Control, ControlType, PropValue};
use cobolt_forms::paint::draw_control;
use egui::epaint::{ClippedPrimitive, ClippedShape, Primitive, Shape, Tessellator};

/// One control, as the designer would drop it.
fn control(ct: ControlType, w: i32, h: i32) -> Control {
    let mut c = Control::new("C-1", ct, 0, 0);
    c.rect = cobolt_forms::Rect::new(0, 0, w, h);
    c
}

/// The controls the feature will tilt, with something to look at in each.
fn subjects() -> Vec<(&'static str, Control)> {
    let mut label = control(ControlType::Label, 180, 28);
    label.set_prop("Caption", PropValue::String("Revenue by quarter".into()));
    vec![
        ("BarChart", control(ControlType::BarChart, 320, 220)),
        ("PieChart", control(ControlType::PieChart, 240, 220)),
        ("Label", label),
        ("PictureBox", control(ControlType::PictureBox, 200, 140)),
        ("Shape", control(ControlType::Shape, 160, 90)),
        ("Line", control(ControlType::Line, 160, 4)),
    ]
}

fn raw(w: f32, h: f32) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h))),
        ..Default::default()
    }
}

/// How the shapes are treated inside the frame.
#[derive(Clone, Copy, PartialEq)]
enum Treatment {
    /// Leave the paint list alone — egui's own result, the reference.
    Untouched,
    /// Lift the shapes this control added, tessellate them ourselves and put
    /// each back as a mesh.
    TessellateAndPutBack,
}

struct Frame {
    /// What the painter added for the control, as captured (ClippedShape clones).
    captured: Vec<ClippedShape>,
    /// The primitives egui produced for the whole frame.
    primitives: Vec<ClippedPrimitive>,
    ppp: f32,
    font_size: [usize; 2],
}

fn run(ctrl: &Control, treatment: Treatment) -> Frame {
    let ctx = egui::Context::default();
    let mut captured: Vec<ClippedShape> = Vec::new();
    // Two frames: the first lays the glyph atlas out, the second is the one measured
    // (a first frame legitimately differs by what it has not yet rasterised).
    let mut last = None;
    for _ in 0..2 {
        captured.clear();
        let mut full = ctx.run_ui(raw(520.0, 320.0), |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(root_ui, |ui| {
                    let layer = ui.layer_id();
                    let start = ctx.graphics(|g| g.get(layer).map_or(0, |l| l.next_idx().0));
                    draw_control(
                        ui.painter(),
                        egui::pos2(20.0, 20.0),
                        ctrl,
                        false,
                        true,
                        1.0,
                        1.0,
                        None,
                    );
                    let end = ctx.graphics(|g| g.get(layer).map_or(0, |l| l.next_idx().0));
                    captured = ctx.graphics(|g| {
                        g.get(layer)
                            .map(|l| l.all_entries().skip(start).take(end - start).cloned().collect())
                            .unwrap_or_default()
                    });
                    if treatment == Treatment::TessellateAndPutBack {
                        let ppp = ctx.pixels_per_point();
                        let opts = ctx.tessellation_options(|o| *o);
                        let size = ctx.fonts(|f| f.font_image_size());
                        let mut tess = Tessellator::new(ppp, opts, size, Vec::new());
                        ctx.graphics_mut(|g| {
                            let list = g.entry(layer);
                            for (n, cs) in captured.iter().enumerate() {
                                let mut mesh = egui::epaint::Mesh::default();
                                tess.tessellate_shape(cs.shape.clone(), &mut mesh);
                                list.mutate_shape(egui::layers::ShapeIdx(start + n), |slot| {
                                    slot.shape = Shape::mesh(mesh);
                                });
                            }
                        });
                    }
                });
        });
        full.textures_delta.clear();
        let ppp = full.pixels_per_point;
        let font_size = ctx.fonts(|f| f.font_image_size());
        let primitives = ctx.tessellate(full.shapes, ppp);
        last = Some(Frame { captured: captured.clone(), primitives, ppp, font_size });
    }
    last.unwrap()
}

/// Every triangle's three vertices (position, uv, colour) in paint order, one
/// list for the frame — mesh boundaries are an egui batching detail, not a result.
fn vertices(prims: &[ClippedPrimitive]) -> Vec<egui::epaint::Vertex> {
    let mut out = Vec::new();
    for p in prims {
        if let Primitive::Mesh(m) = &p.primitive {
            for &i in &m.indices {
                out.push(m.vertices[i as usize]);
            }
        }
    }
    out
}

/// Worst position difference (points) and whether uv/colour agree exactly.
fn compare(a: &[egui::epaint::Vertex], b: &[egui::epaint::Vertex]) -> (f32, usize, usize) {
    let mut worst = 0.0f32;
    let mut uv_off = 0;
    let mut col_off = 0;
    for (x, y) in a.iter().zip(b) {
        worst = worst.max((x.pos - y.pos).length());
        if (x.uv - y.uv).length() > 1e-6 {
            uv_off += 1;
        }
        if x.color != y.color {
            col_off += 1;
        }
    }
    (worst, uv_off, col_off)
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

fn text_shapes(shapes: &[ClippedShape]) -> usize {
    fn walk(s: &Shape) -> usize {
        match s {
            Shape::Text(_) => 1,
            Shape::Vec(v) => v.iter().map(walk).sum(),
            _ => 0,
        }
    }
    shapes.iter().map(|c| walk(&c.shape)).sum()
}

#[test]
fn the_shapes_a_control_paints_can_be_lifted_tessellated_and_put_back() {
    println!("\n  ── Spec 096 T1: capture → our Tessellator → put back ──");
    println!(
        "  {:<10} {:>7} {:>6} {:>9} {:>11} {:>8} {:>8}   {:>10} {:>8} {:>8}",
        "control", "shapes", "text", "callback", "triangles", "ours Δpt", "uv/col", "put-back Δ", "uv", "col"
    );
    let mut all_text = 0;
    for (name, ctrl) in subjects() {
        let reference = run(&ctrl, Treatment::Untouched);
        let put_back = run(&ctrl, Treatment::TessellateAndPutBack);

        // (2) our tessellator over the captured shapes == egui's over the same.
        let opts = egui::Context::default().tessellation_options(|o| *o);
        let mut tess = Tessellator::new(
            reference.ppp,
            opts,
            reference.font_size,
            Vec::new(),
        );
        let ours = tess.tessellate_shapes(reference.captured.clone());
        let (ours_v, ref_v) = (vertices(&ours), vertices(&reference.primitives));
        assert_eq!(
            ours_v.len(),
            ref_v.len(),
            "{name}: our tessellation made a different number of triangle corners"
        );
        let (worst, uv_off, col_off) = compare(&ours_v, &ref_v);

        // (3) put back → the frame's triangles are the untouched frame's.
        let back_v = vertices(&put_back.primitives);
        assert_eq!(back_v.len(), ref_v.len(), "{name}: put-back changed the triangle count");
        let (pb_worst, pb_uv, pb_col) = compare(&back_v, &ref_v);

        let texts = text_shapes(&reference.captured);
        all_text += texts;
        println!(
            "  {:<10} {:>7} {:>6} {:>9} {:>11} {:>8.4} {:>3}/{:<4}   {:>10.4} {:>8} {:>8}",
            name,
            reference.captured.len(),
            texts,
            if has_callback(&reference.captured) { "YES" } else { "no" },
            ref_v.len() / 3,
            worst,
            uv_off,
            col_off,
            pb_worst,
            pb_uv,
            pb_col,
        );

        assert!(!reference.captured.is_empty(), "{name}: nothing was captured");
        assert!(!has_callback(&reference.captured), "{name}: paints a Shape::Callback");
        assert!(worst <= 0.01, "{name}: positions differ by {worst} pt");
        assert_eq!(uv_off, 0, "{name}: {uv_off} UVs differ (glyph atlas size?)");
        assert_eq!(col_off, 0, "{name}: {col_off} colours differ");
        assert!(pb_worst <= 0.01, "{name}: put-back moved a vertex by {pb_worst} pt");
        assert_eq!(pb_uv, 0, "{name}: put-back changed {pb_uv} UVs");
        assert_eq!(pb_col, 0, "{name}: put-back changed {pb_col} colours");
    }
    assert!(all_text > 0, "no text was captured at all — the gate cannot answer for glyphs");
    println!("\n  GO: every difference ≤ 0.01 pt, {all_text} text shapes among them.\n");
}
