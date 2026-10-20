// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The current path, with the HTML Canvas semantics.
//!
//! As in the browser, **points are transformed by the current transform at the
//! moment they are added**: the path lives in device space, and a later change
//! of transform does not move it. (`Stroke` then strokes in the *user* space of
//! the transform in force at that later moment - see `paint.rs`.)

use kurbo::{Affine, Arc, BezPath, PathEl, Point, Vec2};
use std::f64::consts::PI;

/// The path being built, in device space.
#[derive(Clone, Debug, Default)]
pub(crate) struct PathAcc {
    pub bez: BezPath,
    cur: Option<Point>,
    start: Option<Point>,
    points: u64,
}

const TWO_PI: f64 = 2.0 * PI;

/// The tolerance, in **user** units, for turning an arc of `radius` into cubic
/// curves: 0.1 device pixel, but never finer than 1/2,000,000 of the radius.
/// Without that floor an absurd radius (or an absurd scale) asks for millions of
/// segments for something far beyond the canvas; with it a circle is at most a
/// dozen curves, and the flattening of each curve in `flatten.rs` has its own cap.
pub(crate) fn arc_tolerance(ctm: Affine, radius: f64) -> f64 {
    let c = ctm.as_coeffs();
    let scale = (c[0] * c[3] - c[1] * c[2]).abs().sqrt().max(1e-9);
    (0.1 / scale).max(radius / 2.0e6).max(1e-9)
}

/// A full circle as a closed path in user space.
pub(crate) fn circle_path(center: Point, radius: f64, ctm: Affine) -> BezPath {
    let mut b = BezPath::new();
    let arc = Arc::new(center, Vec2::new(radius, radius), 0.0, TWO_PI, 0.0);
    b.move_to(Point::new(center.x + radius, center.y));
    for el in arc.append_iter(arc_tolerance(ctm, radius)) {
        b.push(el);
    }
    b.close_path();
    b
}

/// `a mod m` in `[0, m)`, built only from basic arithmetic (deterministic).
fn modulo(a: f64, m: f64) -> f64 {
    let r = a - m * (a / m).floor();
    if r >= m || r < 0.0 {
        0.0
    } else {
        r
    }
}

impl PathAcc {
    pub fn clear(&mut self) {
        self.bez = BezPath::new();
        self.cur = None;
        self.start = None;
        self.points = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.bez.elements().is_empty()
    }

    /// Number of points added so far (for the path-size limit).
    pub fn points(&self) -> u64 {
        self.points
    }

    #[cfg(test)]
    pub fn current(&self) -> Option<Point> {
        self.cur
    }

    pub fn move_to(&mut self, p: Point) {
        self.bez.move_to(p);
        self.cur = Some(p);
        self.start = Some(p);
        self.points += 1;
    }

    /// A line; with no sub-path yet it starts one at the point (HTML).
    pub fn line_to(&mut self, p: Point) {
        if self.cur.is_none() {
            self.move_to(p);
            return;
        }
        self.bez.line_to(p);
        self.cur = Some(p);
        self.points += 1;
    }

    pub fn quad_to(&mut self, c: Point, p: Point) {
        if self.cur.is_none() {
            self.move_to(c);
        }
        self.bez.quad_to(c, p);
        self.cur = Some(p);
        self.points += 2;
    }

    pub fn cubic_to(&mut self, c1: Point, c2: Point, p: Point) {
        if self.cur.is_none() {
            self.move_to(c1);
        }
        self.bez.curve_to(c1, c2, p);
        self.cur = Some(p);
        self.points += 3;
    }

    pub fn close(&mut self) {
        if self.cur.is_some() {
            self.bez.close_path();
            self.cur = self.start;
        }
    }

    /// A closed rectangle sub-path in user space; afterwards the current point
    /// is its first corner, as HTML specifies.
    pub fn rect(&mut self, ctm: Affine, x: f64, y: f64, w: f64, h: f64) {
        let p = |px: f64, py: f64| ctm * Point::new(px, py);
        self.move_to(p(x, y));
        self.line_to(p(x + w, y));
        self.line_to(p(x + w, y + h));
        self.line_to(p(x, y + h));
        self.close();
        let s = p(x, y);
        self.bez.move_to(s);
        self.cur = Some(s);
        self.start = Some(s);
    }

    /// A closed rounded rectangle (radii: top-left, top-right, bottom-right,
    /// bottom-left). Radii are scaled down together when they would overlap.
    pub fn round_rect(&mut self, ctm: Affine, x: f64, y: f64, w: f64, h: f64, r: [f64; 4]) {
        let (x, w) = if w < 0.0 { (x + w, -w) } else { (x, w) };
        let (y, h) = if h < 0.0 { (y + h, -h) } else { (y, h) };
        let [mut tl, mut tr, mut br, mut bl] = r;
        let mut f: f64 = 1.0;
        for (side, sum) in [(w, tl + tr), (w, bl + br), (h, tl + bl), (h, tr + br)] {
            if sum > side && sum > 0.0 {
                f = f.min(side / sum);
            }
        }
        if f < 1.0 {
            tl *= f;
            tr *= f;
            br *= f;
            bl *= f;
        }
        const K: f64 = 0.552_284_749_830_793_4; // 4/3 * (sqrt(2) - 1)
        let p = |px: f64, py: f64| ctm * Point::new(px, py);
        self.move_to(p(x + tl, y));
        self.line_to(p(x + w - tr, y));
        self.cubic_to(p(x + w - tr + tr * K, y), p(x + w, y + tr - tr * K), p(x + w, y + tr));
        self.line_to(p(x + w, y + h - br));
        self.cubic_to(p(x + w, y + h - br + br * K), p(x + w - br + br * K, y + h), p(x + w - br, y + h));
        self.line_to(p(x + bl, y + h));
        self.cubic_to(p(x + bl - bl * K, y + h), p(x, y + h - bl + bl * K), p(x, y + h - bl));
        self.line_to(p(x, y + tl));
        self.cubic_to(p(x, y + tl - tl * K), p(x + tl - tl * K, y), p(x + tl, y));
        self.close();
        let s = p(x + tl, y);
        self.bez.move_to(s);
        self.cur = Some(s);
        self.start = Some(s);
    }

    /// HTML `arc` (a circle is an ellipse with equal radii).
    pub fn arc(&mut self, ctm: Affine, x: f64, y: f64, r: f64, start: f64, end: f64, ccw: bool) {
        self.ellipse(ctm, x, y, r, r, 0.0, start, end, ccw);
    }

    /// HTML `ellipse`.
    #[allow(clippy::too_many_arguments)]
    pub fn ellipse(&mut self, ctm: Affine, x: f64, y: f64, rx: f64, ry: f64, rot: f64, start: f64, end: f64, ccw: bool) {
        // Negative radii: HTML throws, we ignore the call.
        if rx < 0.0 || ry < 0.0 {
            return;
        }
        // The sweep, per the HTML specification.
        let sweep = if !ccw && end - start >= TWO_PI {
            TWO_PI
        } else if ccw && start - end >= TWO_PI {
            -TWO_PI
        } else if !ccw {
            modulo(end - start, TWO_PI)
        } else {
            -modulo(start - end, TWO_PI)
        };
        let center = Point::new(x, y);
        let (sin_r, cos_r) = rot.sin_cos();
        let at = |a: f64| -> Point {
            let (s, c) = a.sin_cos();
            let (px, py) = (rx * c, ry * s);
            Point::new(x + px * cos_r - py * sin_r, y + px * sin_r + py * cos_r)
        };
        let p0 = ctm * at(start);
        if self.cur.is_some() {
            self.line_to(p0);
        } else {
            self.move_to(p0);
        }
        if sweep == 0.0 || (rx == 0.0 && ry == 0.0) {
            if rx == 0.0 && ry == 0.0 {
                self.line_to(ctm * center);
            }
            return;
        }
        let arc = Arc::new(center, Vec2::new(rx, ry), start, sweep, rot);
        for el in arc.append_iter(arc_tolerance(ctm, rx.max(ry))) {
            if let PathEl::CurveTo(a, b, c) = el {
                self.cubic_to(ctm * a, ctm * b, ctm * c);
            }
        }
    }

    /// HTML `arcTo`.
    pub fn arc_to(&mut self, ctm: Affine, x1: f64, y1: f64, x2: f64, y2: f64, r: f64) {
        if r < 0.0 {
            return;
        }
        let p1 = Point::new(x1, y1);
        let p2 = Point::new(x2, y2);
        let Some(cur) = self.cur else {
            self.move_to(ctm * p1);
            return;
        };
        let inv = ctm.inverse();
        let p0 = if inv.is_finite() { inv * cur } else { p1 };
        let line_only = {
            let v1 = p0 - p1;
            let v2 = p2 - p1;
            let cross = v1.x * v2.y - v1.y * v2.x;
            p0 == p1 || p1 == p2 || r == 0.0 || cross.abs() < 1e-12 * (v1.hypot() * v2.hypot()).max(1e-300)
        };
        if line_only || !inv.is_finite() {
            self.line_to(ctm * p1);
            return;
        }
        let v1 = (p0 - p1) / (p0 - p1).hypot();
        let v2 = (p2 - p1) / (p2 - p1).hypot();
        let cos_t = (v1.x * v2.x + v1.y * v2.y).clamp(-1.0, 1.0);
        let theta = cos_t.acos();
        let half = theta / 2.0;
        let tan_h = half.tan();
        if !tan_h.is_finite() || tan_h.abs() < 1e-12 {
            self.line_to(ctm * p1);
            return;
        }
        let d = r / tan_h;
        let t1 = p1 + v1 * d;
        let t2 = p1 + v2 * d;
        let bis = v1 + v2;
        let bl = bis.hypot();
        if bl < 1e-12 {
            self.line_to(ctm * p1);
            return;
        }
        let center = p1 + (bis / bl) * (r / half.sin());
        let a1 = (t1.y - center.y).atan2(t1.x - center.x);
        let a2 = (t2.y - center.y).atan2(t2.x - center.x);
        let mut sweep = modulo(a2 - a1 + PI, TWO_PI) - PI; // shortest way round, in [-pi, pi)
        if sweep == -PI {
            sweep = PI;
        }
        self.line_to(ctm * t1);
        let arc = Arc::new(center, Vec2::new(r, r), a1, sweep, 0.0);
        for el in arc.append_iter(arc_tolerance(ctm, r)) {
            if let PathEl::CurveTo(a, b, c) = el {
                self.cubic_to(ctm * a, ctm * b, ctm * c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    fn id() -> Affine {
        Affine::IDENTITY
    }

    #[test]
    fn rect_makes_a_closed_square_and_leaves_the_current_point_at_its_corner() {
        let mut p = PathAcc::default();
        p.rect(id(), 10.0, 20.0, 30.0, 40.0);
        assert_eq!(p.current(), Some(Point::new(10.0, 20.0)));
        let bb = p.bez.bounding_box();
        assert_eq!((bb.x0, bb.y0, bb.x1, bb.y1), (10.0, 20.0, 40.0, 60.0));
        assert!(p.bez.winding(Point::new(20.0, 30.0)) != 0);
        assert_eq!(p.bez.winding(Point::new(5.0, 30.0)), 0);
    }

    #[test]
    fn full_circle_covers_two_pi() {
        let mut p = PathAcc::default();
        p.arc(id(), 50.0, 50.0, 10.0, 0.0, TWO_PI, false);
        let bb = p.bez.bounding_box();
        assert!((bb.x0 - 40.0).abs() < 0.2 && (bb.x1 - 60.0).abs() < 0.2, "{bb:?}");
        assert!(p.bez.winding(Point::new(50.0, 50.0)) != 0);
        assert_eq!(p.bez.winding(Point::new(80.0, 50.0)), 0);
    }

    #[test]
    fn arc_directions_follow_html() {
        // Clockwise from 0 to pi/2 ends below the centre (y grows downward).
        let mut p = PathAcc::default();
        p.arc(id(), 0.0, 0.0, 10.0, 0.0, PI / 2.0, false);
        let c = p.current().unwrap();
        assert!(c.x.abs() < 1e-6 && (c.y - 10.0).abs() < 1e-6, "{c:?}");
        // Counter-clockwise from 0 to pi/2 goes the long way (three quarters).
        let mut q = PathAcc::default();
        q.arc(id(), 0.0, 0.0, 10.0, 0.0, PI / 2.0, true);
        let c = q.current().unwrap();
        assert!(c.x.abs() < 1e-6 && (c.y - 10.0).abs() < 1e-6);
        let bb = q.bez.bounding_box();
        assert!(bb.x0 < -9.0 && bb.y0 < -9.0, "the long way round passes through the upper left: {bb:?}");
    }

    #[test]
    fn negative_radius_is_ignored() {
        let mut p = PathAcc::default();
        p.arc(id(), 0.0, 0.0, -5.0, 0.0, 1.0, false);
        assert!(p.is_empty());
    }

    #[test]
    fn transform_applies_when_the_point_is_added() {
        let mut p = PathAcc::default();
        p.rect(Affine::scale(2.0), 1.0, 1.0, 1.0, 1.0);
        let bb = p.bez.bounding_box();
        assert_eq!((bb.x0, bb.x1), (2.0, 4.0));
    }

    #[test]
    fn arc_to_rounds_a_right_angle() {
        let mut p = PathAcc::default();
        p.move_to(Point::new(0.0, 0.0));
        p.arc_to(id(), 100.0, 0.0, 100.0, 100.0, 20.0);
        let c = p.current().unwrap();
        assert!((c.x - 100.0).abs() < 1e-6 && (c.y - 20.0).abs() < 1e-6, "{c:?}");
        let bb = p.bez.bounding_box();
        assert!(bb.x1 <= 100.0 + 1e-6);
    }

    #[test]
    fn arc_to_with_collinear_points_is_a_line() {
        let mut p = PathAcc::default();
        p.move_to(Point::new(0.0, 0.0));
        p.arc_to(id(), 50.0, 0.0, 100.0, 0.0, 10.0);
        assert_eq!(p.current(), Some(Point::new(50.0, 0.0)));
    }

    #[test]
    fn round_rect_scales_radii_that_overlap() {
        let mut p = PathAcc::default();
        p.round_rect(id(), 0.0, 0.0, 10.0, 10.0, [50.0, 50.0, 50.0, 50.0]);
        let bb = p.bez.bounding_box();
        assert!(bb.x0 >= -1e-9 && bb.x1 <= 10.0 + 1e-9 && bb.y1 <= 10.0 + 1e-9, "{bb:?}");
    }

    #[test]
    fn modulo_is_in_range() {
        for a in [-7.0, -TWO_PI, -0.1, 0.0, 0.1, TWO_PI, 100.0] {
            let r = modulo(a, TWO_PI);
            assert!((0.0..TWO_PI).contains(&r), "{a} -> {r}");
        }
    }
}
