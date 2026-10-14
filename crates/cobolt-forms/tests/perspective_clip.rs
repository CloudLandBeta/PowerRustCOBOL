// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 096, tasks T5/T8 — **a tilted control stays where it is allowed to paint.**
//!
//! A tilt that brings a control nearer enlarges it, so it can reach past its
//! container, or past the form. What an untilted control may not do, a tilted one
//! may not either (R9): it is clipped by the rectangle that clips an untilted
//! control in its place — its container's content area, the form's rectangle for a
//! control on a layer — and it paints at its own place in the paint order, under
//! whatever is drawn after it. And it moves with an animation transform rather than
//! against it, and keeps playing if it is an Animator (R15).
//!
//! The clip a primitive carries is what the GPU clips to, so "paints nothing
//! outside" is read from it: every primitive that has a vertex past the allowed
//! rectangle must carry a clip rectangle inside it.
//!
//! Spec 096 AC7 (R9), AC11 (R15) in part.

#![cfg(feature = "render")]

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{Control, ControlType, Layer, PropValue};
use cobolt_forms::perspective::Perspective;
use cobolt_forms::render::{
    render_faces, render_form, Backdrop, DesignedState, FormState, RenderInput, RenderMode,
    RenderOutput, RenderTransform,
};
use egui::epaint::{ClippedPrimitive, ClippedShape, Primitive};
use egui::{pos2, Pos2, Rect};

const FORM: egui::Vec2 = egui::vec2(460.0, 300.0);

#[derive(Clone, Copy, Debug)]
enum Path {
    Run,
    Static,
    Canvas,
}
const PATHS: [Path; 3] = [Path::Run, Path::Static, Path::Canvas];

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

struct Frame {
    out: RenderOutput,
    shapes: Vec<ClippedShape>,
    primitives: Vec<ClippedPrimitive>,
}

fn frame(controls: &[Control], layers: Vec<Layer>, state: &dyn FormState, path: Path, time: f64) -> Frame {
    let ctx = egui::Context::default();
    let active = ActiveTabs::new();
    let mut kept = None;
    for pass in 0..2 {
        let mut drawn = None;
        let mut layer_shapes = Vec::new();
        let mut full = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(480.0, 320.0))),
                time: Some(time + pass as f64 * 0.0),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(root_ui, |ui| {
                    ui.set_min_size(FORM);
                    let input = RenderInput {
                        controls,
                        state,
                        form_size: FORM,
                        glass: true,
                        mode: if matches!(path, Path::Run) { RenderMode::Interactive } else { RenderMode::Static },
                        active_tabs: &active,
                        backdrop: Backdrop { layers: layers.clone(), ..Backdrop::default() },
                    };
                    drawn = Some(if matches!(path, Path::Canvas) {
                        let origin = ui.min_rect().min;
                        render_faces(&ui.painter().clone(), origin, &input)
                    } else {
                        render_form(ui, &input)
                    });
                    let layer = ui.layer_id();
                    layer_shapes = ctx.graphics(|g| {
                        g.get(layer).map(|l| l.all_entries().cloned().collect()).unwrap_or_default()
                    });
                });
            },
        );
        let ppp = full.pixels_per_point;
        full.textures_delta.clear();
        let primitives = ctx.tessellate(layer_shapes.clone(), ppp);
        kept = drawn.map(|out| Frame { out, shapes: layer_shapes, primitives });
    }
    kept.expect("a frame was drawn")
}

fn same_primitive(a: &ClippedPrimitive, b: &ClippedPrimitive) -> bool {
    match (&a.primitive, &b.primitive) {
        (Primitive::Mesh(x), Primitive::Mesh(y)) => {
            a.clip_rect == b.clip_rect
                && x.texture_id == y.texture_id
                && x.indices == y.indices
                && x.vertices == y.vertices
        }
        _ => false,
    }
}

/// Of the primitives `f` holds that the same form WITHOUT the control (`base`) does not:
/// how many there are, how many have a vertex outside `allowed`, and how many of those carry a clip
/// rectangle inside it. (The panel's face and the form's backdrop are in `base`; they are not
/// the chart's to account for.)
fn overflow(f: &Frame, base: &Frame, allowed: Rect) -> (usize, usize, usize) {
    let (mut over, mut clipped, mut total) = (0, 0, 0);
    let slack = allowed.expand(0.01);
    for p in &f.primitives {
        if base.primitives.iter().any(|b| same_primitive(b, p)) {
            continue;
        }
        if let Primitive::Mesh(m) = &p.primitive {
            total += 1;
            if m.vertices.iter().any(|v| !slack.contains(v.pos)) {
                over += 1;
                if slack.contains_rect(p.clip_rect) {
                    clipped += 1;
                }
            }
        }
    }
    (total, over, clipped)
}

#[test]
fn a_tilted_enlarged_chart_in_a_panel_is_clipped_to_the_panel() {
    let mut panel = control("Pnl", ControlType::Panel, 40, 40, 220, 160);
    panel.parent = None;
    let mut chart = control("Chart", ControlType::BarChart, 60, 60, 150, 100);
    chart.parent = Some("Pnl".into());
    tilt(&mut chart, 20, -40, 100);
    let controls = vec![panel.clone(), chart];
    let content = {
        let c = panel.content_rect();
        Rect::from_min_size(pos2(c.x as f32, c.y as f32), egui::vec2(c.w as f32, c.h as f32))
    };

    println!("\n  ── R9: a chart enlarged to distance 100 inside a Panel (content {content:?}) ──");
    println!("  path     chart meshes   past the panel   of those, clipped to it");
    for path in PATHS {
        let f = frame(&controls, vec![], &DesignedState, path, 0.0);
        let base = frame(&controls[..1], vec![], &DesignedState, path, 0.0);
        let quad = f.out.hit_quads["Chart"];
        let reach = Rect::from_points(&quad);
        let (total, over, clipped) = overflow(&f, &base, content);
        println!("  {path:<8?} {total:>6} {over:>16} {clipped:>26}   (outline reaches {:.0}×{:.0})", reach.width(), reach.height());
        assert!(reach.width() > 150.0 * 1.5, "{path:?}: the chart really is enlarged past the panel");
        assert!(over > 0, "{path:?}: nothing reached past the panel — the test would prove nothing");
        assert_eq!(over, clipped, "{path:?}: a primitive past the panel is not clipped to it");
    }
}

#[test]
fn on_a_layer_nothing_leaves_the_form() {
    let mut chart = control("Chart", ControlType::BarChart, 300, 30, 140, 100);
    chart.layer = Some("L1".into());
    tilt(&mut chart, 0, -40, 100);
    let layers = vec![Layer::new("L1")];
    let form = Rect::from_min_size(Pos2::ZERO, FORM);

    println!("\n  ── R9: a chart on layer L1, enlarged past the form's right edge ──");
    println!("  path     chart meshes   past the form   of those, clipped to it");
    for path in PATHS {
        let f = frame(std::slice::from_ref(&chart), layers.clone(), &DesignedState, path, 0.0);
        let base = frame(&[], layers.clone(), &DesignedState, path, 0.0);
        let (total, over, clipped) = overflow(&f, &base, form);
        println!("  {path:<8?} {total:>6} {over:>15} {clipped:>24}");
        assert!(over > 0, "{path:?}: nothing reached past the form — the test would prove nothing");
        assert_eq!(over, clipped, "{path:?}: a primitive past the form is not clipped to it");
    }
}

#[test]
fn a_drop_shadow_is_projected_with_the_control_and_cut_by_nothing_of_its_own() {
    // A chart near the top of the form, with a drop shadow that falls outside the control's own box.
    // The shadow is painted under the control's clip like the rest of it, so it is projected with the
    // tilt and carries no rectangle of its own that could cut it along a straight line: every primitive
    // the chart leaves must carry the one clip the surface imposes.
    let mut chart = control("Chart", ControlType::BarChart, 80, 24, 260, 180);
    chart.set_prop("ShadowEnabled", PropValue::Bool(true));
    chart.set_prop("ShadowDistance", PropValue::Int(14));
    tilt(&mut chart, 8, -30, 60);
    let controls = vec![chart];
    let form = Rect::from_min_size(Pos2::ZERO, FORM);

    println!("\n  ── a shadowed chart near the form's top edge: the clips its primitives carry ──");
    for path in PATHS {
        let f = frame(&controls, vec![], &DesignedState, path, 0.0);
        let base = frame(&[], vec![], &DesignedState, path, 0.0);
        let mut clips: Vec<Rect> = Vec::new();
        for p in &f.primitives {
            if base.primitives.iter().any(|b| same_primitive(b, p)) {
                continue;
            }
            if !clips.contains(&p.clip_rect) {
                clips.push(p.clip_rect);
            }
        }
        println!("  {path:<8?} distinct clip rectangles on the chart's primitives: {clips:?}");
        // The one clip the surface itself imposes: the window the frame is drawn in.
        let surface = Rect::from_min_size(Pos2::ZERO, egui::vec2(480.0, 320.0));
        assert!(form.intersects(surface));
        assert_eq!(clips, vec![surface], "{path:?}: a primitive of the chart is cut by a rectangle of its own");
    }
}

#[test]
fn what_is_painted_after_a_tilted_control_stays_over_it() {
    let mut chart = control("Chart", ControlType::BarChart, 30, 30, 240, 160);
    let mut lbl = control("Lbl", ControlType::Label, 40, 40, 160, 24);
    lbl.set_prop("Caption", PropValue::String("over".into()));
    let untilted = vec![chart.clone(), lbl.clone()];
    tilt(&mut chart, 10, -35, 80);
    let tilted = vec![chart, lbl];

    println!("\n  ── paint order: the label drawn after the chart ──");
    for path in PATHS {
        let (a, b) = (
            frame(&untilted, vec![], &DesignedState, path, 0.0),
            frame(&tilted, vec![], &DesignedState, path, 0.0),
        );
        // The paint list has the same slots: the tilt rewrote the chart's in place.
        assert_eq!(a.shapes.len(), b.shapes.len(), "{path:?}: the number of slots changed");
        // The label is the last control, so its shapes close the list — untouched.
        let tail = 1;
        let (ta, tb) = (&a.shapes[a.shapes.len() - tail..], &b.shapes[b.shapes.len() - tail..]);
        assert_eq!(ta, tb, "{path:?}: the label drawn after the chart was changed or moved");
        // …while the chart's own slots did change.
        assert_ne!(&a.shapes[..a.shapes.len() - tail], &b.shapes[..b.shapes.len() - tail], "{path:?}: the chart was not tilted");
        println!("  {path:<8?} {} slots, the closing {tail} (the label) identical, the chart's differ", b.shapes.len());
    }
}

/// A state that shifts and scales every control, as an entrance effect does.
struct Animated;
impl FormState for Animated {
    fn transform(&self, _base: &Control) -> RenderTransform {
        RenderTransform { dx: 40.0, dy: 12.0, scale: 1.5, alpha: 1.0 }
    }
}

#[test]
fn an_animation_transform_and_a_tilt_combine_about_the_moved_centre() {
    let mut chart = control("Chart", ControlType::BarChart, 60, 60, 120, 80);
    tilt(&mut chart, 12, -30, 55);
    let controls = vec![chart.clone()];
    let p = Perspective::of(&chart).unwrap();

    println!("\n  ── the tilt turns about the centre of the ANIMATED rectangle ──");
    for path in PATHS {
        let plain = frame(&controls, vec![], &DesignedState, path, 0.0);
        let moved = frame(&controls, vec![], &Animated, path, 0.0);
        let (rect_plain, rect_moved) = (plain.out.control_rects["Chart"], moved.out.control_rects["Chart"]);
        // The animation moved and grew the rectangle…
        assert!((rect_moved.center() - rect_plain.center()).length() > 30.0);
        assert!(rect_moved.width() > rect_plain.width() * 1.4);
        // …and the outline is the tilt of THAT rectangle, not of the designed one.
        let want = p.quad(rect_moved);
        let got = moved.out.hit_quads["Chart"];
        let worst = want.iter().zip(got).map(|(a, b)| (*a - b).length()).fold(0.0f32, f32::max);
        println!("  {path:<8?} animated rect centre {:?}; outline differs from the model's by {worst:.5} px", rect_moved.center());
        assert!(worst < 1e-3);
    }
}

/// A two-frame GIF whose frames are different colours, 100 ms each.
fn two_frame_gif() -> std::path::PathBuf {
    use image::codecs::gif::GifEncoder;
    use image::{Delay, Frame, Rgba, RgbaImage};
    let dir = std::env::temp_dir().join(format!("perspective-clip-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("two.gif");
    let mut enc = GifEncoder::new(std::fs::File::create(&path).unwrap());
    for colour in [Rgba([220, 40, 40, 255]), Rgba([40, 40, 220, 255])] {
        let img = RgbaImage::from_pixel(32, 24, colour);
        enc.encode_frame(Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(100, 1))).unwrap();
    }
    path
}

#[test]
fn a_tilted_animator_keeps_playing() {
    let gif = two_frame_gif();
    let mut anim = control("Anim", ControlType::Animator, 40, 40, 160, 120);
    anim.set_prop("Source", PropValue::String(gif.to_string_lossy().into_owned()));
    tilt(&mut anim, 0, -30, 50);
    let controls = vec![anim];

    println!("\n  ── R15: a tilted Animator on a two-frame GIF (100 ms a frame) ──");
    // One context, two frames at different times — as a running form is drawn.
    let ctx = egui::Context::default();
    let active = ActiveTabs::new();
    let mut seen: Vec<(f64, usize, egui::TextureId, bool)> = Vec::new();
    for t in [0.02f64, 0.07, 0.13, 0.18] {
        let mut shapes = Vec::new();
        let mut full = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(480.0, 320.0))),
                time: Some(t),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(root_ui, |ui| {
                    ui.set_min_size(FORM);
                    let input = RenderInput {
                        controls: &controls,
                        state: &DesignedState,
                        form_size: FORM,
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Backdrop::default(),
                    };
                    render_form(ui, &input);
                    let layer = ui.layer_id();
                    shapes = ctx.graphics(|g| g.get(layer).map(|l| l.all_entries().cloned().collect()).unwrap_or_default());
                });
            },
        );
        full.textures_delta.clear();
        let prims = ctx.tessellate(shapes, 1.0);
        // The image mesh: the one with a user texture.
        let image = prims.iter().find_map(|p| match &p.primitive {
            Primitive::Mesh(m) if matches!(m.texture_id, egui::TextureId::Managed(n) if n != 0) => Some(m.clone()),
            _ => None,
        });
        let frame_idx = cobolt_media::playback_position(&ctx, &format!("Anim|{}", gif.to_string_lossy()), true, true, "Anim")
            .map(|(i, _, _)| i)
            .unwrap_or(usize::MAX);
        let texture = image.as_ref().map(|m| m.texture_id).unwrap_or(egui::TextureId::default());
        // The image is projected: its four outline corners are not an axis-aligned rectangle.
        let skewed = image.as_ref().is_some_and(|m| {
            let xs: Vec<f32> = m.vertices.iter().map(|v| v.pos.x).collect();
            let ys: Vec<f32> = m.vertices.iter().map(|v| v.pos.y).collect();
            let (w, h) = (xs.iter().cloned().fold(f32::MIN, f32::max) - xs.iter().cloned().fold(f32::MAX, f32::min),
                          ys.iter().cloned().fold(f32::MIN, f32::max) - ys.iter().cloned().fold(f32::MAX, f32::min));
            // A −30° turn about the vertical axis leaves the image shorter on its far side: more
            // than a handful of distinct y values at the left and right edges.
            let mut left: Vec<f32> = m.vertices.iter().filter(|v| (v.pos.x - xs.iter().cloned().fold(f32::MAX, f32::min)).abs() < 0.01).map(|v| v.pos.y).collect();
            let mut right: Vec<f32> = m.vertices.iter().filter(|v| (v.pos.x - xs.iter().cloned().fold(f32::MIN, f32::max)).abs() < 0.01).map(|v| v.pos.y).collect();
            left.sort_by(f32::total_cmp);
            right.sort_by(f32::total_cmp);
            let (lh, rh) = (left.last().unwrap() - left.first().unwrap(), right.last().unwrap() - right.first().unwrap());
            w > 1.0 && h > 1.0 && (lh - rh).abs() > 2.0
        });
        println!("  t = {t:.2} s  frame {frame_idx}  texture {texture:?}  projected image: {skewed}");
        seen.push((t, frame_idx, texture, skewed));
    }
    assert!(seen.iter().all(|s| s.3), "the image is drawn in perspective at every instant");
    assert_eq!(seen[0].1, 0);
    assert_eq!(seen[1].1, 0);
    assert_eq!(seen[2].1, 1, "the second frame is reached at 130 ms");
    assert_ne!(seen[0].2, seen[2].2, "…and the projected mesh carries the second frame's texture");
    assert_eq!(seen[2].2, seen[3].2);
}

