// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 096, tasks T7 and T8 — **one projection on every surface.**
//!
//! The Form Designer canvas paints through `render_faces`; Preview, Run Form and a
//! compiled binary paint through `render_form`, which has a static and an
//! interactive path. A tilt hooked into one of them and not the others would be
//! invisible there. This draws one form with a tilted chart, a tilted picture and a
//! tilted shape through all three and compares what each produced:
//!
//! * the projected outline of each control (`hit_quads`) — equal on all three;
//! * the triangles each control left on the layer — equal, vertex for vertex, between the
//!   designer canvas and the static run path (they paint the same face);
//! * the designed rectangle (`control_rects`) — still the DESIGNED one, so the
//!   designer's selection and handles stay on it (R11);
//! * and a control left at the neutral values enters none of it (R6).
//!
//! Spec 096 AC6 (R8), AC9 (R11 in part).

#![cfg(feature = "render")]

use std::collections::HashMap;

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{Control, ControlType, PropValue};
use cobolt_forms::perspective::{quad_is_convex, Perspective};
use cobolt_forms::render::{
    render_faces, render_form, Backdrop, DesignedState, RenderInput, RenderMode, RenderOutput,
};
use egui::epaint::Primitive;
use egui::{pos2, Pos2, Rect};

fn control(id: &str, ct: ControlType, x: i32, y: i32, w: i32, h: i32) -> Control {
    let mut c = Control::new(id, ct, x, y);
    c.rect = cobolt_forms::Rect::new(x, y, w, h);
    c
}

fn tilt(c: &mut Control, rx: i64, ry: i64, d: i64) {
    c.set_prop("RotateX", PropValue::Int(rx));
    c.set_prop("RotateY", PropValue::Int(ry));
    c.set_prop("PerspectiveDistance", PropValue::Int(d));
}

/// A chart, a picture and a shape tilted, and a label left alone.
fn form() -> Vec<Control> {
    let mut chart = control("Chart", ControlType::BarChart, 20, 20, 240, 160);
    tilt(&mut chart, 8, -30, 60);
    let mut pic = control("Pic", ControlType::PictureBox, 290, 20, 140, 100);
    tilt(&mut pic, 0, 25, 50);
    let mut shape = control("Shp", ControlType::Shape, 20, 210, 120, 60);
    tilt(&mut shape, -20, 0, 70);
    let mut lbl = control("Lbl", ControlType::Label, 200, 230, 120, 24);
    lbl.set_prop("Caption", PropValue::String("flat".into()));
    vec![chart, pic, shape, lbl]
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Surface {
    /// The designer canvas: `render_faces`.
    Designer,
    /// Preview and the static run path: `render_form` in `Static` mode.
    Static,
    /// Run Form and a built binary: `render_form` in `Interactive` mode.
    Interactive,
}

struct Seen {
    out: RenderOutput,
    /// Every vertex of the frame, in paint order.
    vertices: Vec<Pos2>,
}

fn draw(controls: &[Control], surface: Surface) -> Seen {
    let ctx = egui::Context::default();
    let active = ActiveTabs::new();
    let mut kept = None;
    // Two frames: the first lays the glyph atlas out; only the second is kept.
    for _ in 0..2 {
        let mut drawn = None;
        let mut full = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(480.0, 320.0))),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(root_ui, |ui| {
                    ui.set_min_size(egui::vec2(460.0, 300.0));
                    let input = RenderInput {
                        controls,
                        state: &DesignedState,
                        form_size: egui::vec2(460.0, 300.0),
                        glass: true,
                        mode: if surface == Surface::Interactive { RenderMode::Interactive } else { RenderMode::Static },
                        active_tabs: &active,
                        backdrop: Backdrop::default(),
                    };
                    drawn = Some(if surface == Surface::Designer {
                        let origin = ui.min_rect().min;
                        render_faces(&ui.painter().clone(), origin, &input)
                    } else {
                        render_form(ui, &input)
                    });
                });
            },
        );
        let ppp = full.pixels_per_point;
        full.textures_delta.clear();
        let mut vertices = Vec::new();
        for prim in ctx.tessellate(std::mem::take(&mut full.shapes), ppp) {
            if let Primitive::Mesh(m) = prim.primitive {
                vertices.extend(m.indices.iter().map(|&i| m.vertices[i as usize].pos));
            }
        }
        kept = drawn.map(|out| Seen { out, vertices });
    }
    kept.expect("a frame was drawn")
}

fn quads_equal(a: &HashMap<String, [Pos2; 4]>, b: &HashMap<String, [Pos2; 4]>) -> f32 {
    assert_eq!(a.len(), b.len(), "the same controls are tilted: {:?} vs {:?}", a.keys(), b.keys());
    let mut worst = 0.0f32;
    for (id, qa) in a {
        let qb = b.get(id).unwrap_or_else(|| panic!("{id} has no outline on the other surface"));
        for (p, q) in qa.iter().zip(qb) {
            worst = worst.max((*p - *q).length());
        }
    }
    worst
}

#[test]
fn the_three_surfaces_project_the_same_outline() {
    let controls = form();
    let designer = draw(&controls, Surface::Designer);
    let stat = draw(&controls, Surface::Static);
    let live = draw(&controls, Surface::Interactive);

    println!("\n  ── Spec 096 AC6: one tilt on every surface ──");
    println!("  tilted controls: {:?}", {
        let mut k: Vec<_> = designer.out.hit_quads.keys().cloned().collect();
        k.sort();
        k
    });
    assert_eq!(designer.out.hit_quads.len(), 3, "the chart, the picture and the shape are tilted; the label is not");
    assert!(!designer.out.hit_quads.contains_key("Lbl"), "an untilted control has no outline to hit-test");

    let (a, b) = (quads_equal(&designer.out.hit_quads, &stat.out.hit_quads), quads_equal(&stat.out.hit_quads, &live.out.hit_quads));
    println!("  outline difference: designer vs static {a:.5} px · static vs interactive {b:.5} px");
    assert!(a < 1e-3 && b < 1e-3);
    for (id, q) in &designer.out.hit_quads {
        assert!(quad_is_convex(q), "{id}: the outline is not convex");
    }

    // R11 — the rectangle every surface reports is still the DESIGNED one.
    for c in &controls {
        let want = Rect::from_min_size(pos2(c.rect.x as f32, c.rect.y as f32), egui::vec2(c.rect.w as f32, c.rect.h as f32));
        for (name, seen) in [("designer", &designer), ("static", &stat), ("interactive", &live)] {
            let got = seen.out.control_rects[&c.id];
            let moved = (got.size() - want.size()).length();
            assert!(moved < 0.6, "{name}: {} is reported at {got:?}, designed {want:?}", c.id);
        }
    }
    println!("  control_rects: the designed rectangle on all three surfaces (R11)");

    // Where the projected outline sits is derived from that rectangle by the model.
    let chart = &controls[0];
    let plane = designer.out.control_rects["Chart"];
    let expect = Perspective::of(chart).unwrap().quad(plane);
    for (p, q) in designer.out.hit_quads["Chart"].iter().zip(expect) {
        assert!((*p - q).length() < 1e-3);
    }
}

#[test]
fn the_designer_canvas_and_the_static_run_path_paint_the_same_triangles() {
    let controls = form();
    let designer = draw(&controls, Surface::Designer);
    let stat = draw(&controls, Surface::Static);
    // The run path paints the form's backdrop first; the canvas does not. What the canvas
    // paints is therefore the tail of what the run path paints.
    let (d, s) = (&designer.vertices, &stat.vertices);
    assert!(s.len() >= d.len(), "the run path paints at least what the canvas paints");
    let tail = &s[s.len() - d.len()..];
    let mut worst = 0.0f32;
    for (p, q) in d.iter().zip(tail) {
        worst = worst.max((*p - *q).length());
    }
    println!(
        "\n  designer {} triangle corners · static run path {} (the extra {} are the backdrop) · worst difference {worst:.5} px",
        d.len(),
        s.len(),
        s.len() - d.len()
    );
    assert!(worst < 1e-3, "the surfaces disagree by {worst} px");
}

#[test]
fn the_interactive_path_paints_the_tilted_faces_as_the_static_one_does() {
    // Run Form and a built binary go through the interactive arms, which paint a face of
    // their own accord; Preview goes through the static path. The tilt is hooked into
    // both, and these controls only draw — so what they leave on the layer must agree.
    let controls = form();
    let stat = draw(&controls, Surface::Static);
    let live = draw(&controls, Surface::Interactive);
    let n = stat.vertices.len().min(live.vertices.len());
    let (s, l) = (&stat.vertices[stat.vertices.len() - n..], &live.vertices[live.vertices.len() - n..]);
    let mut worst = 0.0f32;
    for (p, q) in s.iter().zip(l) {
        worst = worst.max((*p - *q).length());
    }
    println!(
        "\n  static {} triangle corners · interactive {} · tail of {n} compared · worst difference {worst:.5} px",
        stat.vertices.len(),
        live.vertices.len()
    );
    assert_eq!(stat.vertices.len(), live.vertices.len(), "the two paths paint the same number of triangles");
    assert!(worst < 1e-3, "the interactive path disagrees with the static one by {worst} px");
    // And the tilt did move the chart: an outline corner is well away from the designed one.
    let plane = live.out.control_rects["Chart"];
    let moved = live.out.hit_quads["Chart"]
        .iter()
        .zip([plane.left_top(), plane.right_top(), plane.right_bottom(), plane.left_bottom()])
        .map(|(a, b)| (*a - b).length())
        .fold(0.0f32, f32::max);
    println!("  Chart: an outline corner is {moved:.1} px from its designed corner");
    assert!(moved > 10.0, "a −30° tilt must visibly move the corners");
}

#[test]
fn a_control_at_the_defaults_enters_none_of_it() {
    let mut controls = form();
    for c in &mut controls {
        c.set_prop("RotateX", PropValue::Int(0));
        c.set_prop("RotateY", PropValue::Int(0));
        c.set_prop("PerspectiveDistance", PropValue::Int(50));
    }
    for surface in [Surface::Designer, Surface::Static, Surface::Interactive] {
        let seen = draw(&controls, surface);
        assert!(seen.out.hit_quads.is_empty(), "{surface:?}: an untilted form has no outlines");
    }
    println!("\n  all values neutral: no outline on any surface");
}
