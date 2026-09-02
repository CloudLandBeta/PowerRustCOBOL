// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Pie and donut slices, and the type over them (operator screenshots, 2026-09-02).
//!
//! Four defects were visible in one picture and none of them were colour bugs:
//! a donut's hole was FILLED, a straight chord crossed every sector, a
//! monochrome pie was one undifferentiated blob, and the labels on a dark
//! gradient were near-black.
//!
//! # What is measured, and why not the obvious thing
//!
//! The first attempt at these tests counted painted VERTICES inside the hole
//! and passed against the broken painter — `convex_polygon` fans from an
//! existing rim vertex, so it covers the middle with TRIANGLES without ever
//! putting a vertex there. Counting distinct colours failed the same way: the
//! grid, the legend and the face supplied more than four on their own.
//!
//! So everything here samples what is actually **painted at a point**: walk the
//! tessellated triangles, find the last one covering the probe, and take its
//! colour. That is the question the screenshots asked.

#![cfg(feature = "render")]

use cobolt_forms::model::{Control, ControlType, PropValue};
use cobolt_forms::paint::draw_control;

/// Every tessellated triangle of one painted control, as `(a, b, c, colour)`.
fn triangles(ctrl: &Control, w: f32, h: f32) -> Vec<([egui::Pos2; 3], [u8; 4])> {
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::Vec2::new(w + 40.0, h + 40.0),
    ));
    let mut full = ctx.run_ui(input, |root_ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(root_ui, |ui| {
                draw_control(ui.painter(), egui::Pos2::ZERO, ctrl, false, true, 1.0, 1.0, None);
            });
    });
    full.textures_delta.clear();
    let mut out = Vec::new();
    for prim in ctx.tessellate(full.shapes, full.pixels_per_point) {
        if let egui::epaint::Primitive::Mesh(m) = prim.primitive {
            for tri in m.indices.chunks_exact(3) {
                let (a, b, c) = (
                    m.vertices[tri[0] as usize],
                    m.vertices[tri[1] as usize],
                    m.vertices[tri[2] as usize],
                );
                out.push(([a.pos, b.pos, c.pos], a.color.to_array()));
            }
        }
    }
    out
}

fn covers(t: &[egui::Pos2; 3], p: egui::Pos2) -> bool {
    let sign = |a: egui::Pos2, b: egui::Pos2, c: egui::Pos2| {
        (a.x - c.x) * (b.y - c.y) - (b.x - c.x) * (a.y - c.y)
    };
    let (d1, d2, d3) = (sign(p, t[0], t[1]), sign(p, t[1], t[2]), sign(p, t[2], t[0]));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

/// What is painted at `p` — the LAST covering triangle, i.e. the top of the
/// stack — ignoring anything fully transparent.
fn color_at(tris: &[([egui::Pos2; 3], [u8; 4])], p: egui::Pos2) -> Option<[u8; 4]> {
    tris.iter()
        .rev()
        .find(|(t, c)| c[3] > 0 && covers(t, p))
        .map(|(_, c)| *c)
}

fn chart(ct: ControlType, props: &[(&str, PropValue)], w: i32, h: i32) -> Control {
    let mut c = Control::new("CHART-1", ct, 0, 0);
    c.rect = cobolt_forms::model::Rect::new(0, 0, w, h);
    for (k, v) in props {
        c.set_prop(*k, v.clone());
    }
    c
}

const W: f32 = 400.0;
const H: f32 = 260.0;

/// The painter's own plot centre and outer radius, recomputed here.
fn geometry(legend: bool) -> (egui::Pos2, f32) {
    let legend_w = if legend { (W * 0.30).min(180.0) } else { 0.0 };
    let (ml, mr) = (W * 0.10, W * 0.04 + legend_w);
    let (mt, mb) = (H * 0.12, H * 0.12);
    let plot = egui::Rect::from_min_max(
        egui::Pos2::new(ml, mt),
        egui::Pos2::new(W - mr, H - mb),
    );
    (plot.center(), plot.size().min_elem() * 0.44)
}

fn base(extra: &[(&str, PropValue)]) -> Vec<(&'static str, PropValue)> {
    let mut v: Vec<(&'static str, PropValue)> = vec![
        ("ShowLegend", PropValue::Bool(false)),
        ("ShowLabels", PropValue::Bool(false)),
        ("ShowGridLines", PropValue::Bool(false)),
        ("Title", PropValue::String(String::new())),
        ("XAxisLabel", PropValue::String(String::new())),
        ("YAxisLabel", PropValue::String(String::new())),
    ];
    for (k, val) in extra {
        let k: &'static str = Box::leak(k.to_string().into_boxed_str());
        v.retain(|(a, _)| *a != k);
        v.push((k, val.clone()));
    }
    v
}

#[test]
fn a_donut_hole_is_empty_and_the_ring_is_not() {
    let donut = chart(
        ControlType::DonutChart,
        &base(&[
            ("__ChartData", PropValue::String("Used\t23\nFree\t77".into())),
            ("InnerRadius", PropValue::Int(40)),
        ]),
        W as i32,
        H as i32,
    );
    let (c, outer) = geometry(false);
    let tris = triangles(&donut, W, H);
    let inner = outer * 0.40;

    // The hole is not UNPAINTED — the chart's own face shows through it. So the
    // question is what colour it carries: the face, or a slice. Sampled just
    // outside the pie, still inside the control, the face is the reference.
    let face = color_at(&tris, egui::Pos2::new(c.x, c.y - outer * 1.35))
        .expect("the chart face must be painted");

    let mut hole_wrong = 0;
    let mut ring_faceish = 0;
    for k in 0..24 {
        let a = std::f32::consts::TAU * k as f32 / 24.0;
        let hole = egui::Pos2::new(c.x + a.cos() * inner * 0.5, c.y + a.sin() * inner * 0.5);
        let ring = egui::Pos2::new(
            c.x + a.cos() * (inner + outer) * 0.5,
            c.y + a.sin() * (inner + outer) * 0.5,
        );
        if color_at(&tris, hole) != Some(face) {
            hole_wrong += 1;
        }
        if color_at(&tris, ring) == Some(face) {
            ring_faceish += 1;
        }
    }
    eprintln!(
        "\n  donut {W}x{H}, inner r={inner:.1} outer r={outer:.1}, face {face:?}\n  \
         24 probes in the hole carrying something OTHER than the face: {hole_wrong}\n  \
         24 probes on the ring carrying the FACE instead of a slice: {ring_faceish}"
    );
    assert_eq!(
        hole_wrong, 0,
        "{hole_wrong}/24 probes inside the hole are painted over — the hole is filled"
    );
    assert_eq!(
        ring_faceish, 0,
        "{ring_faceish}/24 probes on the ring show the face — the ring is not being drawn"
    );
    eprintln!("  → hole shows the face, ring shows slices\n");
}

// The chord the screenshots showed crossing each sector is the SAME defect as
// the filled hole — the bad fan spills toward the centre, not across a
// neighbour on the ring — so `a_donut_hole_is_empty_and_the_ring_is_not` covers
// it whole. A separate "each sector owns its arc" assertion was written, found
// to pass against the broken painter, and deleted rather than kept: a guard
// that cannot fail reads as coverage it does not have.
//
// A PIE is not tested for the reflex case either, and for the same reason:
// `convex_polygon` fans a pie slice from the CENTRE, which is correct at any
// sweep. Only the annular sector was ever wrong.

#[test]
fn a_monochrome_pie_separates_its_slices_in_both_fill_modes() {
    // Monochrome draws tonal variations of ONE colour. With the gradient on,
    // every slice used to be shaded around the same base, so the pie came out a
    // single blob. Sampled at each slice's own midpoint, the four must differ.
    for gradient in [false, true] {
        let pie = chart(
            ControlType::PieChart,
            &base(&[
                (
                    "__ChartData",
                    PropValue::String("A\t25\nB\t25\nC\t25\nD\t25".into()),
                ),
                ("Monochrome", PropValue::Bool(true)),
                ("MonochromeGradient", PropValue::Bool(gradient)),
            ]),
            W as i32,
            H as i32,
        );
        let (c, outer) = geometry(false);
        let tris = triangles(&pie, W, H);
        let mut seen: Vec<[u8; 4]> = Vec::new();
        for k in 0..4 {
            let t = -std::f32::consts::FRAC_PI_2
                + std::f32::consts::TAU * (k as f32 + 0.5) / 4.0;
            let p = egui::Pos2::new(c.x + t.cos() * outer * 0.6, c.y + t.sin() * outer * 0.6);
            seen.push(color_at(&tris, p).unwrap_or_else(|| panic!("quadrant {k} unpainted")));
        }
        let distinct: std::collections::HashSet<_> = seen.iter().collect();
        eprintln!(
            "\n  monochrome pie, gradient={gradient} — quadrant colours {seen:?}\n  \
             distinct: {}/4",
            distinct.len()
        );
        assert_eq!(
            distinct.len(),
            4,
            "gradient={gradient}: four slices must carry four tones, got {}",
            distinct.len()
        );
    }
    eprintln!("  → 4/4 tones in both monochrome fill modes\n");
}
