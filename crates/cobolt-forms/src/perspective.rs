// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 096 — **3D perspective for display controls.**
//!
//! `RotateX`, `RotateY` and `PerspectiveDistance` turn a control that only draws
//! (a chart, a picture, a label, a line, a shape) in space: one edge recedes from
//! the observer and the opposite edge comes nearer, as in a card tilted over a
//! table. The painters are not told. A control is drawn exactly as it always was
//! and the shapes it added to its layer are then lifted off the paint list,
//! tessellated, **every vertex projected** with the pin-hole camera below, and
//! put back as meshes (`PerspectiveScope`, further down).
//!
//! This first part is the geometry alone: pure functions of numbers, no egui
//! state, so a test can state the projection without drawing a thing.
//!
//! # The camera
//!
//! A control has a designed rectangle with centre `c`, size `w × h`, and
//! `M = max(w, h)`. A point `p` of its plane, relative to `c`, is lifted to
//! `(x, y, 0)` — the plane faces the observer, `z` grows **away** from them — and:
//!
//! 1. **`RotateX` = θx** turns it about the horizontal axis:
//!    `y₁ = y·cos θx`, `z₁ = −y·sin θx`. The top edge has `y < 0`, so for θx > 0
//!    its `z₁` is positive: the top recedes.
//! 2. **`RotateY` = θy** turns it about the vertical axis:
//!    `x₂ = x·cos θy − z₁·sin θy`, `z₂ = z₁·cos θy + x·sin θy`. The right edge has
//!    `x > 0`: for θy > 0 the right recedes.
//! 3. **The camera** sits at distance `D(d) = D_ref · 2^((50 − d)/50)` in front of
//!    the plane, `D_ref = K·M`, with focal length `F = D_ref`, and the screen
//!    point is `c + (x₂, y₂) · F / (D + z₂)`.
//!
//! `PerspectiveDistance` `d` is therefore a camera distance on a *2^x* scale:
//! 50 is the identity (`F/D = 1`), 100 brings the control twice as near (×2), 0
//! sends it twice as far (×0.5), and equal steps of `d` feel equal.
//!
//! **`PerspectiveStrength`** (spec 097) scales the convergence alone: the depth
//! `z₂` is multiplied by `m = strength / 100` in the divisor, `F / (D + m·z₂)`.
//! The size of a flat control (`z₂ = 0`) and the centre do not depend on it; at 100
//! it is the formula above; at 0 it is `F / D` for every point — no convergence;
//! between, it is a pin-hole camera at distance `D / m` with focal length `F / m`
//! (`F/(D + m z₂) = (F/m)/(D/m + z₂)`), so lines stay straight and the outline convex.
//!
//! `D + z₂ > 0` for every input. The smallest `D` is `K·M/2 = M` (at `d` = 100)
//! and `|z₂|` never exceeds the half-diagonal of the rectangle, at most `0.71·M`,
//! so the divisor is at least `0.29·M`: nothing is ever behind the camera, and the
//! projected outline is always a **convex** quadrilateral — which is what lets a
//! pointer test be "inside the quad".

use egui::{pos2, Pos2, Rect};

use crate::model::{
    Control, PropValue, PERSPECTIVE_ANGLE_LIMIT, PERSPECTIVE_DISTANCE_MAX,
    PERSPECTIVE_DISTANCE_PROP, PERSPECTIVE_DISTANCE_REFERENCE, PERSPECTIVE_STRENGTH_MAX,
    PERSPECTIVE_STRENGTH_PROP, ROTATE_X_PROP, ROTATE_Y_PROP,
};

/// How many times the larger side of a control the camera stands from it at the
/// reference distance (`PerspectiveDistance` = 50). The one number that decides
/// how strongly a given tilt converges; chosen by eye against the reference
/// picture (spec 096 AC12, task T15). At 2.0 a `RotateY` of −30 leaves the far
/// edge about 0.78 of the near one.
pub const CAMERA_K: f32 = 2.0;

/// A control's three properties, read and brought into range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Perspective {
    /// `RotateX`, whole degrees, −60 … 60. Positive recedes the top edge.
    pub rotate_x: i32,
    /// `RotateY`, whole degrees, −60 … 60. Positive recedes the right edge.
    pub rotate_y: i32,
    /// `PerspectiveDistance`, 0 … 100. 50 is the reference; 100 is nearest.
    pub distance: i32,
    /// `PerspectiveStrength`, 0 … 100. 100 is the convergence the distance gives; 0 is none.
    pub strength: i32,
}

impl Perspective {
    /// The neutral values: untilted, at the reference distance, at full strength.
    pub const NEUTRAL: Perspective = Perspective {
        rotate_x: 0,
        rotate_y: 0,
        distance: PERSPECTIVE_DISTANCE_REFERENCE as i32,
        strength: PERSPECTIVE_STRENGTH_MAX as i32,
    };

    /// Build one, clamping each value into its range (a write is clamped when it
    /// is made; this keeps the painter safe from a hand-edited file as well).
    pub fn new(rotate_x: i64, rotate_y: i64, distance: i64) -> Self {
        let a = PERSPECTIVE_ANGLE_LIMIT;
        Perspective {
            rotate_x: rotate_x.clamp(-a, a) as i32,
            rotate_y: rotate_y.clamp(-a, a) as i32,
            distance: distance.clamp(0, PERSPECTIVE_DISTANCE_MAX) as i32,
            strength: PERSPECTIVE_STRENGTH_MAX as i32,
        }
    }

    /// The same tilt at another `PerspectiveStrength`, clamped into 0 … 100.
    pub fn with_strength(self, strength: i64) -> Self {
        Perspective { strength: strength.clamp(0, PERSPECTIVE_STRENGTH_MAX) as i32, ..self }
    }

    /// What `ctrl` asks for, or `None` when it is drawn as it always was:
    ///
    /// * its type is not one of the twelve that may be tilted — a property
    ///   hand-written into a Button's file is ignored, the loader having kept it
    ///   (spec 096 R16);
    /// * the tilt and the distance are at their neutral values, or absent (the strength
    ///   only scales a convergence, so it matters only to a tilted control);
    /// * a value is not a finite number, which reads as that property's default.
    pub fn of(ctrl: &Control) -> Option<Perspective> {
        if !ctrl.control_type.has_perspective() {
            return None;
        }
        let read = |key: &str, default: i64| -> i64 {
            match ctrl.get_prop(key) {
                Some(PropValue::Int(n)) => *n,
                Some(PropValue::Bool(_)) | None => default,
                Some(PropValue::String(s)) => s
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite())
                    .map_or(default, |n| n.round() as i64),
            }
        };
        let p = Perspective::new(
            read(ROTATE_X_PROP, 0),
            read(ROTATE_Y_PROP, 0),
            read(PERSPECTIVE_DISTANCE_PROP, PERSPECTIVE_DISTANCE_REFERENCE),
        )
        .with_strength(read(PERSPECTIVE_STRENGTH_PROP, PERSPECTIVE_STRENGTH_MAX));
        (!p.is_neutral()).then_some(p)
    }

    /// Untilted and at the reference distance — drawn exactly as before, whatever the
    /// strength: it scales a convergence and an untilted control has none.
    pub fn is_neutral(&self) -> bool {
        self.rotate_x == 0
            && self.rotate_y == 0
            && self.distance == Perspective::NEUTRAL.distance
    }

    /// The camera for a control whose designed rectangle is `rect`.
    pub fn camera(&self, rect: Rect) -> Camera {
        Camera::new(*self, rect)
    }

    /// The projected outline of `rect`: top-left, top-right, bottom-right,
    /// bottom-left, in that order.
    pub fn quad(&self, rect: Rect) -> [Pos2; 4] {
        let cam = self.camera(rect);
        [
            cam.project(rect.left_top()),
            cam.project(rect.right_top()),
            cam.project(rect.right_bottom()),
            cam.project(rect.left_bottom()),
        ]
    }
}

/// The projection of one control, with its sines and cosines worked out once —
/// every vertex of a tilted control goes through [`Camera::project`].
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    centre: Pos2,
    cos_x: f32,
    sin_x: f32,
    cos_y: f32,
    sin_y: f32,
    /// `D`, the distance from the observer to the control's plane.
    depth: f32,
    /// `m = strength / 100`: the share of the depth `z₂` that reaches the divisor.
    convergence: f32,
    /// `F`, the focal length.
    focal: f32,
}

impl Camera {
    pub fn new(p: Perspective, rect: Rect) -> Self {
        let m = rect.width().max(rect.height()).max(1.0);
        let reference = CAMERA_K * m;
        let (sin_x, cos_x) = (p.rotate_x as f32).to_radians().sin_cos();
        let (sin_y, cos_y) = (p.rotate_y as f32).to_radians().sin_cos();
        Camera {
            centre: rect.center(),
            cos_x,
            sin_x,
            cos_y,
            sin_y,
            depth: reference * 2f32.powf((PERSPECTIVE_DISTANCE_REFERENCE as f32 - p.distance as f32) / 50.0),
            convergence: p.strength as f32 / PERSPECTIVE_STRENGTH_MAX as f32,
            focal: reference,
        }
    }

    /// Where a point of the control's plane lands on the screen.
    #[inline]
    pub fn project(&self, p: Pos2) -> Pos2 {
        let (x, y) = (p.x - self.centre.x, p.y - self.centre.y);
        let y1 = y * self.cos_x;
        let z1 = -y * self.sin_x;
        let x2 = x * self.cos_y - z1 * self.sin_y;
        let z2 = z1 * self.cos_y + x * self.sin_y;
        let k = self.focal / (self.depth + self.convergence * z2);
        pos2(self.centre.x + x2 * k, self.centre.y + y1 * k)
    }

    /// `D + m·z₂` for a point — the divisor. Strictly positive for every point of
    /// the control's rectangle (see the module notes); exposed so a test can say so.
    pub fn divisor(&self, p: Pos2) -> f32 {
        let (x, y) = (p.x - self.centre.x, p.y - self.centre.y);
        let z1 = -y * self.sin_x;
        let z2 = z1 * self.cos_y + x * self.sin_y;
        self.depth + self.convergence * z2
    }

    /// The scale a flat control gets at this distance: `F / D`.
    pub fn flat_scale(&self) -> f32 {
        self.focal / self.depth
    }
}

/// The lengths of the four edges of a quad `[tl, tr, br, bl]`: top, right,
/// bottom, left.
pub fn edge_lengths(q: &[Pos2; 4]) -> [f32; 4] {
    [
        (q[1] - q[0]).length(),
        (q[2] - q[1]).length(),
        (q[3] - q[2]).length(),
        (q[0] - q[3]).length(),
    ]
}

/// Whether `p` lies inside (or on the edge of) a convex quad, of either winding.
pub fn quad_contains(q: &[Pos2; 4], p: Pos2) -> bool {
    let mut sign = 0.0f32;
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        if cross != 0.0 {
            if sign == 0.0 {
                sign = cross.signum();
            } else if cross.signum() != sign {
                return false;
            }
        }
    }
    true
}

/// Whether the four corners, taken in order, turn the same way at every corner —
/// a convex outline that does not cross itself.
pub fn quad_is_convex(q: &[Pos2; 4]) -> bool {
    let mut sign = 0.0f32;
    for i in 0..4 {
        let (a, b, c) = (q[i], q[(i + 1) % 4], q[(i + 2) % 4]);
        let cross = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
        if cross == 0.0 {
            return false;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if cross.signum() != sign {
            return false;
        }
    }
    true
}

/// The smallest rectangle that holds a quad.
pub fn quad_bounds(q: &[Pos2; 4]) -> Rect {
    Rect::from_points(q)
}

// ── Projecting a mesh ────────────────────────────────────────────────────────

use egui::epaint::{Mesh, Vertex};

/// How far, in points on the screen, a colour or a texture coordinate may run
/// across a triangle before the triangle is cut.
///
/// A projection sends straight lines to straight lines and triangles to
/// triangles, so moving the vertices of a flat-coloured mesh is *exact*. A
/// colour or a UV that varies across a triangle is interpolated *linearly on
/// screen* by the GPU, though, where the true value varies linearly in the
/// control's own plane — the two agree only when the triangle is small in the
/// direction the value changes. What counts is that direction, not the triangle's
/// size: a one-point feather strip along a 300-point edge changes colour across
/// its one point and is exact as it stands.
pub const MAX_VARYING_EXTENT: f32 = 24.0;

/// A colour channel that changes by less than this (of 255) across a triangle
/// cannot show an interpolation error and is not a reason to cut.
const NEGLIGIBLE_CHANNEL_CHANGE: f32 = 6.0;

/// Halvings of one triangle at most; with [`MAX_TRIANGLES`] it only keeps a
/// pathological mesh finite.
const MAX_SPLIT_DEPTH: u32 = 14;

/// The most triangles one mesh may grow to. Past it the remaining triangles are
/// projected as they are — a coarser result, never a hang.
const MAX_TRIANGLES: usize = 20_000;

fn lerp_vertex(a: &Vertex, b: &Vertex) -> Vertex {
    let mid = |x: u8, y: u8| ((x as u16 + y as u16 + 1) / 2) as u8;
    Vertex {
        pos: pos2((a.pos.x + b.pos.x) * 0.5, (a.pos.y + b.pos.y) * 0.5),
        uv: pos2((a.uv.x + b.uv.x) * 0.5, (a.uv.y + b.uv.y) * 0.5),
        color: egui::Color32::from_rgba_premultiplied(
            mid(a.color.r(), b.color.r()),
            mid(a.color.g(), b.color.g()),
            mid(a.color.b(), b.color.b()),
            mid(a.color.a(), b.color.a()),
        ),
    }
}

/// The attributes the GPU interpolates, as numbers: four colour channels and the
/// texture coordinate (UVs are fractions of a texture of unknown size, so any change
/// of one counts).
fn channels(v: &Vertex) -> [f32; 6] {
    [
        v.color.r() as f32,
        v.color.g() as f32,
        v.color.b() as f32,
        v.color.a() as f32,
        v.uv.x,
        v.uv.y,
    ]
}

/// The attribute of the triangle that runs, in screen points, farthest along its
/// own gradient — and only if that is farther than [`MAX_VARYING_EXTENT`].
///
/// An attribute is affine over a triangle: `a(p) = a₀ + G·p`. It changes from its
/// least to its greatest vertex value over `(max − min) / |G|` points, measured
/// along `G` — the extent that matters, whatever shape the triangle has. The
/// answer is the index into [`channels`] of the worst offender.
fn runs_too_far(s: [Pos2; 3], v: [&Vertex; 3]) -> Option<usize> {
    let (e1, e2) = (s[1] - s[0], s[2] - s[0]);
    let det = e1.x * e2.y - e1.y * e2.x;
    if det.abs() < 1e-6 {
        return None;
    }
    let (a0, a1, a2) = (channels(v[0]), channels(v[1]), channels(v[2]));
    let mut worst: Option<(usize, f32)> = None;
    for c in 0..6 {
        let (lo, hi) = (a0[c].min(a1[c]).min(a2[c]), a0[c].max(a1[c]).max(a2[c]));
        let range = hi - lo;
        let significant = if c < 4 { range >= NEGLIGIBLE_CHANNEL_CHANGE } else { range > 1e-6 };
        if !significant {
            continue;
        }
        let (d1, d2) = (a1[c] - a0[c], a2[c] - a0[c]);
        let gx = (d1 * e2.y - d2 * e1.y) / det;
        let gy = (d2 * e1.x - d1 * e2.x) / det;
        let g = (gx * gx + gy * gy).sqrt();
        let extent = if g > 0.0 { range / g } else { 0.0 };
        if extent > MAX_VARYING_EXTENT && worst.is_none_or(|(_, e)| extent > e) {
            worst = Some((c, extent));
        }
    }
    worst.map(|(c, _)| c)
}

/// Project a mesh the control painted, in the control's own plane, onto the screen.
///
/// Every vertex goes through the camera. A triangle whose colour or UV runs
/// farther than [`MAX_VARYING_EXTENT`] along its gradient, **once projected**, is
/// first halved across its longest edge — position, UV and colour interpolated in
/// the control's plane, where they are linear — until none is. That keeps an image
/// from being smeared across the perspective and a gradient from being bent; a
/// flat fill, a stroke, a feather and a glyph quad are moved as they are.
///
/// The texture and the order are kept; only the geometry changes.
pub fn warp_mesh(cam: &Camera, mesh: &Mesh) -> Mesh {
    let mut vertices: Vec<Vertex> = mesh.vertices.clone();
    let mut indices: Vec<u32> = Vec::with_capacity(mesh.indices.len());
    let mut triangles = mesh.indices.len() / 3;

    let mut stack: Vec<([u32; 3], u32)> = Vec::new();
    for tri in mesh.indices.chunks_exact(3) {
        stack.push(([tri[0], tri[1], tri[2]], 0));
        while let Some((t, depth)) = stack.pop() {
            let (va, vb, vc) = (&vertices[t[0] as usize], &vertices[t[1] as usize], &vertices[t[2] as usize]);
            let s = [cam.project(va.pos), cam.project(vb.pos), cam.project(vc.pos)];
            let worst = if depth >= MAX_SPLIT_DEPTH || triangles >= MAX_TRIANGLES {
                None
            } else {
                runs_too_far(s, [va, vb, vc])
            };
            let Some(channel) = worst else {
                indices.extend_from_slice(&t);
                continue;
            };
            // Halve across the edge along which the offending attribute changes
            // most — the cut that shortens its run — and, among edges that change it
            // about equally (a triangulation's diagonal does), the shortest. The
            // pieces then follow the gradient: a vertical gradient becomes bands.
            let vs = [va, vb, vc];
            let change = |i: usize| (channels(vs[(i + 1) % 3])[channel] - channels(vs[i])[channel]).abs();
            let len = |i: usize| (s[(i + 1) % 3] - s[i]).length();
            let top = (0..3).map(change).fold(0.0f32, f32::max);
            let e = (0..3)
                .filter(|&i| change(i) >= top * 0.99)
                .min_by(|&x, &y| len(x).total_cmp(&len(y)))
                .unwrap_or(0);
            let (i, j, k) = (t[e], t[(e + 1) % 3], t[(e + 2) % 3]);
            let m = vertices.len() as u32;
            let mid = lerp_vertex(&vertices[i as usize], &vertices[j as usize]);
            vertices.push(mid);
            triangles += 1;
            // (i, m, k) and (m, j, k) keep the winding of (i, j, k).
            stack.push(([m, j, k], depth + 1));
            stack.push(([i, m, k], depth + 1));
        }
    }

    for v in &mut vertices {
        v.pos = cam.project(v.pos);
    }
    Mesh { indices, vertices, texture_id: mesh.texture_id }
}

#[cfg(test)]
mod mesh_tests {
    use super::*;
    use egui::{Color32, TextureId};

    fn vert(x: f32, y: f32, u: f32, v: f32, c: Color32) -> Vertex {
        Vertex { pos: pos2(x, y), uv: pos2(u, v), color: c }
    }

    /// A textured quad over `rect`, UVs 0..1, white tint — an image.
    fn image_quad(rect: Rect) -> Mesh {
        let w = Color32::WHITE;
        let mut m = Mesh::with_texture(TextureId::User(7));
        m.vertices = vec![
            vert(rect.left(), rect.top(), 0.0, 0.0, w),
            vert(rect.right(), rect.top(), 1.0, 0.0, w),
            vert(rect.right(), rect.bottom(), 1.0, 1.0, w),
            vert(rect.left(), rect.bottom(), 0.0, 1.0, w),
        ];
        m.indices = vec![0, 1, 2, 0, 2, 3];
        m
    }

    /// A flat-coloured quad: one colour, one UV — a plain fill.
    fn flat_quad(rect: Rect) -> Mesh {
        let c = Color32::from_rgb(200, 40, 40);
        let mut m = Mesh::default();
        m.vertices = vec![
            vert(rect.left(), rect.top(), 0.0, 0.0, c),
            vert(rect.right(), rect.top(), 0.0, 0.0, c),
            vert(rect.right(), rect.bottom(), 0.0, 0.0, c),
            vert(rect.left(), rect.bottom(), 0.0, 0.0, c),
        ];
        m.indices = vec![0, 1, 2, 0, 2, 3];
        m
    }

    fn area(a: Pos2, b: Pos2, c: Pos2) -> f32 {
        ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() * 0.5
    }

    fn rect() -> Rect {
        Rect::from_min_size(pos2(100.0, 50.0), egui::vec2(200.0, 100.0))
    }

    #[test]
    fn a_flat_fill_is_moved_vertex_for_vertex_and_never_cut() {
        let r = rect();
        let cam = Perspective::new(0, -30, 50).camera(r);
        let out = warp_mesh(&cam, &flat_quad(r));
        println!("\n  flat quad: {} triangles in, {} out", 2, out.indices.len() / 3);
        assert_eq!(out.indices.len(), 6, "a flat fill is not subdivided");
        let q = Perspective::new(0, -30, 50).quad(r);
        for (v, corner) in out.vertices.iter().zip(q) {
            assert!((v.pos - corner).length() < 1e-4, "the four outline corners are kept");
        }
    }

    #[test]
    fn an_image_lands_in_perspective_not_halved_on_screen() {
        let r = rect();
        let p = Perspective::new(0, -30, 50);
        let cam = p.camera(r);
        let src = image_quad(r);
        let out = warp_mesh(&cam, &src);
        println!(
            "\n  image 200×100 at RotateY −30: {} triangles in, {} out, {} vertices",
            src.indices.len() / 3,
            out.indices.len() / 3,
            out.vertices.len()
        );
        // The four corners of the image land on the four corners of the outline.
        let q = p.quad(r);
        for (i, corner) in [(0usize, q[0]), (1, q[1]), (2, q[2]), (3, q[3])] {
            assert!((out.vertices[i].pos - corner).length() < 1e-4, "corner {i} must be on the outline");
        }
        // The point halfway along the TOP edge of the IMAGE (u = 0.5, v = 0)…
        let mid_top = out
            .vertices
            .iter()
            .find(|v| (v.uv.x - 0.5).abs() < 1e-6 && v.uv.y.abs() < 1e-6)
            .expect("the top edge is cut at its middle");
        // …lands where the middle of the DESIGNED edge is projected to…
        let expect = cam.project(pos2(r.center().x, r.top()));
        // …which is not the middle of the projected edge on the screen.
        let screen_mid = pos2((q[0].x + q[1].x) / 2.0, (q[0].y + q[1].y) / 2.0);
        println!(
            "  halfway along the top edge: lands at x = {:.2} (perspective {:.2}, screen midpoint {:.2})",
            mid_top.pos.x, expect.x, screen_mid.x
        );
        assert!((mid_top.pos - expect).length() < 1e-3, "the texel lands in perspective");
        assert!((mid_top.pos - screen_mid).length() > 1.0, "…and not at the midpoint on the screen");
    }

    #[test]
    fn every_vertex_agrees_with_its_place_in_the_plane_and_no_triangle_is_empty() {
        let r = rect();
        let p = Perspective::new(20, -40, 80);
        let cam = p.camera(r);
        let out = warp_mesh(&cam, &image_quad(r));
        let mut worst_vertex = 0.0f32;
        for v in &out.vertices {
            assert!(v.pos.x.is_finite() && v.pos.y.is_finite(), "NaN in the output");
            // A UV of the image is a fraction of the designed rectangle.
            let plane = pos2(r.left() + v.uv.x * r.width(), r.top() + v.uv.y * r.height());
            worst_vertex = worst_vertex.max((cam.project(plane) - v.pos).length());
        }
        // The affine error inside a triangle: its centroid on the screen against the
        // projection of its centroid in the plane. Bounded because the cut is.
        let mut worst_inside = 0.0f32;
        let mut smallest = f32::MAX;
        for t in out.indices.chunks_exact(3) {
            let (a, b, c) = (out.vertices[t[0] as usize], out.vertices[t[1] as usize], out.vertices[t[2] as usize]);
            smallest = smallest.min(area(a.pos, b.pos, c.pos));
            let screen_c = pos2((a.pos.x + b.pos.x + c.pos.x) / 3.0, (a.pos.y + b.pos.y + c.pos.y) / 3.0);
            let uv_c = pos2((a.uv.x + b.uv.x + c.uv.x) / 3.0, (a.uv.y + b.uv.y + c.uv.y) / 3.0);
            let plane_c = pos2(r.left() + uv_c.x * r.width(), r.top() + uv_c.y * r.height());
            worst_inside = worst_inside.max((cam.project(plane_c) - screen_c).length());
        }
        println!(
            "\n  RotateX 20, RotateY −40, distance 80: {} triangles, worst vertex error {:.5} px, \
             worst error inside a triangle {:.3} px, smallest triangle {:.2} px²",
            out.indices.len() / 3,
            worst_vertex,
            worst_inside,
            smallest
        );
        assert!(worst_vertex < 1e-3);
        assert!(worst_inside < 0.75, "affine error inside a triangle: {worst_inside} px");
        assert!(smallest > 0.0, "a zero-area triangle");
    }

    #[test]
    fn a_gradient_is_cut_but_a_feather_strip_is_not() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(300.0, 100.0));
        let cam = Perspective::new(0, 40, 70).camera(r);
        // A vertical gradient: top edge one colour, bottom another — varies along the left/right edges.
        let (top, bot) = (Color32::from_rgb(10, 20, 200), Color32::from_rgb(200, 20, 10));
        let mut grad = Mesh::default();
        grad.vertices = vec![
            vert(0.0, 0.0, 0.0, 0.0, top),
            vert(300.0, 0.0, 0.0, 0.0, top),
            vert(300.0, 100.0, 0.0, 0.0, bot),
            vert(0.0, 100.0, 0.0, 0.0, bot),
        ];
        grad.indices = vec![0, 1, 2, 0, 2, 3];
        let g = warp_mesh(&cam, &grad);
        // A 1 px feather strip along the 300 px top edge: colour differs only across its width.
        let (solid, clear) = (Color32::from_rgb(40, 40, 40), Color32::TRANSPARENT);
        let mut strip = Mesh::default();
        strip.vertices = vec![
            vert(0.0, 0.0, 0.0, 0.0, solid),
            vert(300.0, 0.0, 0.0, 0.0, solid),
            vert(300.0, -1.0, 0.0, 0.0, clear),
            vert(0.0, -1.0, 0.0, 0.0, clear),
        ];
        strip.indices = vec![0, 1, 2, 0, 2, 3];
        let s = warp_mesh(&cam, &strip);
        println!(
            "\n  gradient 300×100: 2 → {} triangles · feather strip 300×1: 2 → {} triangles",
            g.indices.len() / 3,
            s.indices.len() / 3
        );
        assert!(g.indices.len() > 6, "a gradient across a long edge must be cut");
        assert!(g.indices.len() / 3 < 200, "…but into a moderate number of pieces, not slivers");
        assert_eq!(s.indices.len(), 6, "a strip whose colour is constant along its length is left alone");
    }
}

// ── Clipping ─────────────────────────────────────────────────────────────────

/// Where a shape's clip rectangle belongs once its control is tilted.
///
/// Every shape on a paint list carries the rectangle that clips it. For an
/// untilted control all of them live in one space — the screen — and mean the
/// control's own box, its container's content area, or a shadow's room. A tilt
/// separates two meanings that were never apart before:
///
/// * a clip **tighter than the one the control was entered with** is the control
///   clipping its own content (a chart's plot area, a caption held to its box).
///   It is a clip in the control's *plane*, so the triangles are cut to it
///   **before** they are projected, and the shape then carries the outer clip;
/// * a clip **the same as, or wider than,** the outer one is the container's
///   content rectangle, the form's, or a shadow's room. It is a clip on the
///   *screen* and stays exactly as it was; nothing is cut early.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClipCase {
    /// Cut the geometry to this rectangle in the control's plane, then project;
    /// the projected shape is clipped by the outer rectangle.
    PreClip(Rect),
    /// Project as it is; the projected shape keeps this rectangle as its clip.
    Keep(Rect),
}

/// Decide [`ClipCase`] for a shape whose clip is `shape_clip`, in a control that was
/// entered with the painter clipped to `outer`.
///
/// Only a clip strictly **inside** `outer` is a plane clip. Equal to it, wider than
/// it, or reaching partly outside it, the clip is a screen clip and is kept (a
/// shadow's room reaches past the control's own clip on purpose; a rectangle that
/// shares nothing with `outer` simply shows nothing, as before).
pub fn clip_case(shape_clip: Rect, outer: Rect) -> ClipCase {
    if shape_clip != outer && outer.contains_rect(shape_clip) {
        ClipCase::PreClip(shape_clip)
    } else {
        ClipCase::Keep(shape_clip)
    }
}

/// A vertex a fraction `t` of the way from `a` to `b` — position, UV and colour.
fn lerp_vertex_at(a: &Vertex, b: &Vertex, t: f32) -> Vertex {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Vertex {
        pos: pos2(a.pos.x + (b.pos.x - a.pos.x) * t, a.pos.y + (b.pos.y - a.pos.y) * t),
        uv: pos2(a.uv.x + (b.uv.x - a.uv.x) * t, a.uv.y + (b.uv.y - a.uv.y) * t),
        color: egui::Color32::from_rgba_premultiplied(
            mix(a.color.r(), b.color.r()),
            mix(a.color.g(), b.color.g()),
            mix(a.color.b(), b.color.b()),
            mix(a.color.a(), b.color.a()),
        ),
    }
}

/// One side of the rectangle a polygon is cut against.
#[derive(Clone, Copy)]
enum Side {
    Left(f32),
    Right(f32),
    Top(f32),
    Bottom(f32),
}

impl Side {
    /// Signed distance inside the half-plane: ≥ 0 is kept.
    fn depth(self, p: Pos2) -> f32 {
        match self {
            Side::Left(x) => p.x - x,
            Side::Right(x) => x - p.x,
            Side::Top(y) => p.y - y,
            Side::Bottom(y) => y - p.y,
        }
    }
}

/// Cut a convex polygon of vertices to one half-plane (Sutherland–Hodgman),
/// interpolating the vertex attributes where an edge crosses.
fn clip_polygon(poly: &[Vertex], side: Side) -> Vec<Vertex> {
    let mut out = Vec::with_capacity(poly.len() + 1);
    for i in 0..poly.len() {
        let (a, b) = (&poly[i], &poly[(i + 1) % poly.len()]);
        let (da, db) = (side.depth(a.pos), side.depth(b.pos));
        if da >= 0.0 {
            out.push(*a);
        }
        if (da >= 0.0) != (db >= 0.0) {
            out.push(lerp_vertex_at(a, b, da / (da - db)));
        }
    }
    out
}

/// A mesh with every triangle cut to `rect`, in the mesh's own coordinates.
///
/// A triangle wholly inside is kept as it is, one wholly outside is dropped, and one
/// that crosses an edge is cut there — its colour and texture coordinate
/// interpolated — and re-triangulated as a fan. The texture is kept.
pub fn clip_mesh_to_rect(mesh: &Mesh, rect: Rect) -> Mesh {
    let mut out = Mesh::with_texture(mesh.texture_id);
    for tri in mesh.indices.chunks_exact(3) {
        let v = [
            mesh.vertices[tri[0] as usize],
            mesh.vertices[tri[1] as usize],
            mesh.vertices[tri[2] as usize],
        ];
        let bounds = Rect::from_points(&[v[0].pos, v[1].pos, v[2].pos]);
        if rect.contains_rect(bounds) {
            let base = out.vertices.len() as u32;
            out.vertices.extend_from_slice(&v);
            out.indices.extend_from_slice(&[base, base + 1, base + 2]);
            continue;
        }
        if !rect.intersects(bounds) {
            continue;
        }
        let mut poly: Vec<Vertex> = v.to_vec();
        for side in [
            Side::Left(rect.left()),
            Side::Right(rect.right()),
            Side::Top(rect.top()),
            Side::Bottom(rect.bottom()),
        ] {
            poly = clip_polygon(&poly, side);
            if poly.len() < 3 {
                break;
            }
        }
        if poly.len() < 3 {
            continue;
        }
        let base = out.vertices.len() as u32;
        out.vertices.extend_from_slice(&poly);
        for k in 1..poly.len() as u32 - 1 {
            out.indices.extend_from_slice(&[base, base + k, base + k + 1]);
        }
    }
    out
}

#[cfg(test)]
mod clip_tests {
    use super::*;
    use egui::Color32;

    fn r(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(pos2(x0, y0), pos2(x1, y1))
    }

    #[test]
    fn the_rule_for_each_kind_of_clip() {
        let outer = r(100.0, 100.0, 400.0, 300.0);
        let cases: [(&str, Rect); 6] = [
            ("equal to the outer clip", outer),
            ("tighter (a plot area)", r(140.0, 120.0, 380.0, 280.0)),
            ("tighter, touching one side", r(100.0, 120.0, 380.0, 280.0)),
            ("wider (a shadow's room)", r(80.0, 80.0, 420.0, 320.0)),
            ("partly overlapping", r(300.0, 200.0, 500.0, 400.0)),
            ("disjoint (shows nothing)", r(500.0, 500.0, 600.0, 600.0)),
        ];
        println!("\n  ── clip_case: outer = (100,100)–(400,300) ──");
        let expect = ["Keep", "PreClip", "PreClip", "Keep", "Keep", "Keep"];
        for ((name, clip), want) in cases.iter().zip(expect) {
            let got = clip_case(*clip, outer);
            println!("  {name:<30} → {got:?}");
            let kind = match got {
                ClipCase::PreClip(_) => "PreClip",
                ClipCase::Keep(_) => "Keep",
            };
            assert_eq!(kind, want, "{name}");
            let (ClipCase::PreClip(c) | ClipCase::Keep(c)) = got;
            assert_eq!(c, *clip, "the rectangle is carried through unchanged");
        }
    }

    fn triangle(a: Pos2, b: Pos2, c: Pos2) -> Mesh {
        // Colour and UV are linear functions of position, so any cut vertex can be checked.
        let make = |p: Pos2| Vertex {
            pos: p,
            uv: pos2(p.x / 1000.0, p.y / 1000.0),
            color: Color32::from_rgba_premultiplied((p.x / 4.0) as u8, (p.y / 4.0) as u8, 50, 255),
        };
        let mut m = Mesh::default();
        m.vertices = vec![make(a), make(b), make(c)];
        m.indices = vec![0, 1, 2];
        m
    }

    fn mesh_area(m: &Mesh) -> f32 {
        m.indices
            .chunks_exact(3)
            .map(|t| {
                let (a, b, c) = (m.vertices[t[0] as usize].pos, m.vertices[t[1] as usize].pos, m.vertices[t[2] as usize].pos);
                ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() * 0.5
            })
            .sum()
    }

    fn inside(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
        let s = |p1: Pos2, p2: Pos2| (p2.x - p1.x) * (p.y - p1.y) - (p2.y - p1.y) * (p.x - p1.x);
        let (d1, d2, d3) = (s(a, b), s(b, c), s(c, a));
        !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
    }

    #[test]
    fn a_clipped_triangle_has_the_area_of_its_overlap() {
        let (a, b, c) = (pos2(20.0, 30.0), pos2(380.0, 90.0), pos2(150.0, 360.0));
        let rect = r(100.0, 80.0, 300.0, 250.0);
        let clipped = clip_mesh_to_rect(&triangle(a, b, c), rect);
        // The reference: 100 × 100 = 10 000 sample points over the rectangle.
        let n = 100usize;
        let mut hits = 0usize;
        for iy in 0..n {
            for ix in 0..n {
                let p = pos2(
                    rect.left() + (ix as f32 + 0.5) / n as f32 * rect.width(),
                    rect.top() + (iy as f32 + 0.5) / n as f32 * rect.height(),
                );
                if inside(p, a, b, c) {
                    hits += 1;
                }
            }
        }
        let sampled = hits as f32 / (n * n) as f32 * rect.width() * rect.height();
        let exact = mesh_area(&clipped);
        println!(
            "\n  triangle cut to a rectangle: {} triangles, area {exact:.1}; brute force over 10 000 points {sampled:.1} \
             ({:.2} % apart)",
            clipped.indices.len() / 3,
            (exact - sampled).abs() / sampled * 100.0
        );
        assert!((exact - sampled).abs() / sampled < 0.01, "area {exact} vs sampled {sampled}");
        // Nothing of the result lies outside the rectangle, and attributes follow position.
        for v in &clipped.vertices {
            assert!(rect.expand(1e-3).contains(v.pos), "{:?} escaped", v.pos);
            assert!((v.uv.x - v.pos.x / 1000.0).abs() < 1e-5 && (v.uv.y - v.pos.y / 1000.0).abs() < 1e-5, "UV drifted");
            assert!((v.color.r() as f32 - v.pos.x / 4.0).abs() <= 1.01, "colour drifted at {:?}", v.pos);
        }
    }

    #[test]
    fn inside_stays_whole_and_outside_vanishes() {
        let rect = r(0.0, 0.0, 100.0, 100.0);
        let whole = clip_mesh_to_rect(&triangle(pos2(10.0, 10.0), pos2(90.0, 20.0), pos2(40.0, 80.0)), rect);
        assert_eq!((whole.vertices.len(), whole.indices.len()), (3, 3), "a triangle wholly inside is kept as it is");
        let gone = clip_mesh_to_rect(&triangle(pos2(200.0, 10.0), pos2(290.0, 20.0), pos2(240.0, 80.0)), rect);
        assert!(gone.indices.is_empty(), "a triangle wholly outside is dropped");
        println!("\n  inside → kept whole (3 vertices) · outside → dropped (0 triangles)");
    }
}

// ── The scope: lift what a control painted, project it, put it back ──────────

use egui::epaint::{ClippedPrimitive, ClippedShape, Primitive, Shape, Tessellator};
use egui::{Context, LayerId};

/// What `PerspectiveScope` holds while a tilted control paints.
struct Active {
    ctx: Context,
    layer: LayerId,
    /// Index in the layer's paint list of the first shape this control adds.
    start: usize,
    /// The painter's clip when the scope was entered: the control's own clip.
    outer: Rect,
    /// The control's designed rectangle — the plane that is projected.
    plane: Rect,
    perspective: Perspective,
}

/// Brackets the painting of one control and, when it ends, tilts what was painted.
///
/// ```ignore
/// let _tilt = PerspectiveScope::enter(&painter, screen, Perspective::of(&face));
/// paint::draw_control(…);   // exactly as it always was
/// // dropped here: the shapes `draw_control` added are projected in place
/// ```
///
/// A guard rather than a closure because the render loop has `continue`s and
/// early arms: the capture ends however the iteration does.
///
/// **A control that is not tilted costs nothing.** With `None`, `enter` reads no
/// state, allocates nothing and touches no paint list (spec 096 R6) — the control
/// is drawn by the code that drew it before this feature existed.
///
/// With a tilt, the scope takes the shapes the painter added to its layer since
/// `enter`, tessellates each with egui's own `Tessellator` (as `Context::tessellate`
/// does later — task T1 measured the result vertex for vertex), cuts the pieces
/// to the clips that are in the control's plane ([`ClipCase`]), projects every
/// vertex ([`warp_mesh`]) and writes the meshes back into the same slots. Paint
/// order is therefore unchanged: whatever was painted after the control is still
/// over it.
///
/// `plane` is the rectangle the control was designed to occupy on the screen (after
/// any animation transform) — the centre the tilt turns about.
pub struct PerspectiveScope(Option<Active>);

impl PerspectiveScope {
    pub fn enter(painter: &egui::Painter, plane: Rect, perspective: Option<Perspective>) -> Self {
        let Some(perspective) = perspective else {
            return PerspectiveScope(None);
        };
        let ctx = painter.ctx().clone();
        let layer = painter.layer_id();
        let start = ctx.graphics_mut(|g| g.entry(layer).next_idx().0);
        PerspectiveScope(Some(Active {
            ctx,
            layer,
            start,
            outer: painter.clip_rect(),
            plane,
            perspective,
        }))
    }
}

impl Drop for PerspectiveScope {
    fn drop(&mut self) {
        if let Some(active) = self.0.take() {
            active.project();
        }
    }
}

impl Active {
    fn project(self) {
        let Active { ctx, layer, start, outer, plane, perspective } = self;
        let captured: Vec<ClippedShape> = ctx.graphics(|g| {
            g.get(layer)
                .map(|list| list.all_entries().skip(start).cloned().collect())
                .unwrap_or_default()
        });
        if captured.is_empty() {
            return;
        }

        let mut options = ctx.tessellation_options(|o| *o);
        // egui culls a shape against the clip it is tessellated with. Here that clip
        // is in the control's plane, and what survives is decided after the shape has
        // moved — a control that sits half out of its container shrinks into view.
        options.coarse_tessellation_culling = false;
        let font_size = ctx.fonts(|f| f.font_image_size());
        let mut tessellator = Tessellator::new(ctx.pixels_per_point(), options, font_size, Vec::new());
        let camera = perspective.camera(plane);

        let projected: Vec<(Shape, Rect)> = captured
            .into_iter()
            .map(|clipped| {
                let case = clip_case(clipped.clip_rect, outer);
                let mut primitives: Vec<ClippedPrimitive> = Vec::new();
                tessellator.tessellate_clipped_shape(clipped.clone(), &mut primitives);
                let clip = match case {
                    ClipCase::PreClip(_) => outer,
                    ClipCase::Keep(c) => c,
                };
                let shapes: Vec<Shape> = primitives
                    .into_iter()
                    .filter_map(|p| match p.primitive {
                        Primitive::Mesh(mesh) => {
                            let mesh = match case {
                                ClipCase::PreClip(plane_clip) => clip_mesh_to_rect(&mesh, plane_clip),
                                ClipCase::Keep(_) => mesh,
                            };
                            (!mesh.is_empty()).then(|| Shape::mesh(warp_mesh(&camera, &mesh)))
                        }
                        // A paint callback cannot be projected. None of the controls that
                        // may be tilted paints one (a test says so); one that did would
                        // stay where it was drawn rather than vanish.
                        Primitive::Callback(callback) => Some(Shape::Callback(callback)),
                    })
                    .collect();
                (Shape::Vec(shapes), clip)
            })
            .collect();

        ctx.graphics_mut(|g| {
            let list = g.entry(layer);
            for (n, (shape, clip)) in projected.into_iter().enumerate() {
                list.mutate_shape(egui::layers::ShapeIdx(start + n), |slot| {
                    slot.shape = shape;
                    slot.clip_rect = clip;
                });
            }
        });
    }
}
