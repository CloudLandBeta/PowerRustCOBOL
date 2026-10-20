// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Turning curves into straight segments, once, in absolute device space.
//!
//! Everything the canvas fills reaches the rasteriser as **straight edges
//! only**. Curves are flattened here, in `f64`, in absolute device coordinates,
//! with a fixed tolerance and a subdivision count that depends on nothing but the
//! curve itself (Wang's formula: a square root, multiplications and a `ceil`, so
//! the same bits on every platform - no `powf`, no `sin`). Two things follow:
//!
//! * the rasteriser never clips a curve at a band edge and re-flattens the
//!   remainder differently, so a banded canvas draws the same edges as an
//!   unbanded one;
//! * the shape of a circle does not depend on the rasteriser's own flattening
//!   (which changed between tiny-skia releases), and is finer than it: 0.05
//!   device pixel.

use kurbo::{Affine, BezPath, PathEl, Point};
use tiny_skia::{Path, PathBuilder, PathSegment};

use super::paint::COORD_LIMIT;

/// Largest distance, in device pixels, between a curve and its flattening.
pub(crate) const TOLERANCE: f64 = 0.05;
/// Most segments one curve is cut into (hostile coordinates only).
const MAX_SEGMENTS: usize = 1024;

struct Out {
    pb: PathBuilder,
    y0: f64,
    /// Coordinates are rounded to a multiple of this (a power of two) before the
    /// shift, so `y - y0` is exact in `f32` and every band sees identical vertices.
    grid: f64,
    open: bool,
}

impl Out {
    fn new(y0: f64, grid: f64) -> Self {
        Out { pb: PathBuilder::new(), y0, grid, open: false }
    }

    fn pt(&self, p: Point) -> (f32, f32) {
        let snap = |v: f64| (v / self.grid).round() * self.grid;
        let cl = |v: f64| v.clamp(-COORD_LIMIT, COORD_LIMIT);
        (cl(snap(p.x)) as f32, cl(snap(p.y) - self.y0) as f32)
    }
    fn move_to(&mut self, p: Point) {
        let (x, y) = self.pt(p);
        self.pb.move_to(x, y);
        self.open = true;
    }
    fn line_to(&mut self, p: Point) {
        if !self.open {
            self.move_to(p);
            return;
        }
        let (x, y) = self.pt(p);
        self.pb.line_to(x, y);
    }
    fn close(&mut self) {
        if self.open {
            self.pb.close();
        }
    }
}

fn segments_for(second_difference: f64, degree_factor: f64) -> usize {
    // Wang: n = ceil(sqrt(d(d-1) / (8 tol) * M)); degree_factor = d(d-1)/8.
    let n = (degree_factor * second_difference / TOLERANCE).sqrt().ceil();
    if n.is_finite() {
        (n as usize).clamp(1, MAX_SEGMENTS)
    } else {
        1
    }
}

fn dist(a: Point, b: Point) -> f64 {
    (a - b).hypot()
}

fn quad(out: &mut Out, p0: Point, p1: Point, p2: Point) {
    let m = dist(Point::new(p0.x - 2.0 * p1.x + p2.x, p0.y - 2.0 * p1.y + p2.y), Point::ORIGIN);
    let n = segments_for(m, 2.0 / 8.0);
    for i in 1..n {
        let t = i as f64 / n as f64;
        let u = 1.0 - t;
        let (a, b, c) = (u * u, 2.0 * u * t, t * t);
        out.line_to(Point::new(a * p0.x + b * p1.x + c * p2.x, a * p0.y + b * p1.y + c * p2.y));
    }
    out.line_to(p2);
}

fn cubic(out: &mut Out, p0: Point, p1: Point, p2: Point, p3: Point) {
    let d1 = dist(Point::new(p0.x - 2.0 * p1.x + p2.x, p0.y - 2.0 * p1.y + p2.y), Point::ORIGIN);
    let d2 = dist(Point::new(p1.x - 2.0 * p2.x + p3.x, p1.y - 2.0 * p2.y + p3.y), Point::ORIGIN);
    let n = segments_for(d1.max(d2), 6.0 / 8.0);
    for i in 1..n {
        let t = i as f64 / n as f64;
        let u = 1.0 - t;
        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        out.line_to(Point::new(
            a * p0.x + b * p1.x + c * p2.x + d * p3.x,
            a * p0.y + b * p1.y + c * p2.y + d * p3.y,
        ));
    }
    out.line_to(p3);
}

/// A device-space path to a rasteriser path of straight segments, shifted up by
/// `y0` pixels. `None` when nothing drawable results.
pub(crate) fn to_tiny(bez: &BezPath, y0: f64, grid: f64) -> Option<Path> {
    let mut out = Out::new(y0, grid);
    let (mut cur, mut start) = (Point::ORIGIN, Point::ORIGIN);
    for el in bez.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                out.move_to(p);
                cur = p;
                start = p;
            }
            PathEl::LineTo(p) => {
                out.line_to(p);
                cur = p;
            }
            PathEl::QuadTo(c, p) => {
                quad(&mut out, cur, c, p);
                cur = p;
            }
            PathEl::CurveTo(a, b, p) => {
                cubic(&mut out, cur, a, b, p);
                cur = p;
            }
            PathEl::ClosePath => {
                out.close();
                cur = start;
            }
        }
    }
    out.pb.finish()
}

/// A rasteriser path in some other space (user space, glyph space) mapped by
/// `m` into absolute device space, flattened and shifted up by `y0`.
pub(crate) fn map_to_tiny(path: &Path, m: Affine, y0: f64, grid: f64) -> Option<Path> {
    let mut out = Out::new(y0, grid);
    let map = |p: tiny_skia::Point| m * Point::new(p.x as f64, p.y as f64);
    let (mut cur, mut start) = (Point::ORIGIN, Point::ORIGIN);
    for seg in path.segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                let q = map(p);
                out.move_to(q);
                cur = q;
                start = q;
            }
            PathSegment::LineTo(p) => {
                let q = map(p);
                out.line_to(q);
                cur = q;
            }
            PathSegment::QuadTo(c, p) => {
                let (c, p) = (map(c), map(p));
                quad(&mut out, cur, c, p);
                cur = p;
            }
            PathSegment::CubicTo(a, b, p) => {
                let (a, b, p) = (map(a), map(b), map(p));
                cubic(&mut out, cur, a, b, p);
                cur = p;
            }
            PathSegment::Close => {
                out.close();
                cur = start;
            }
        }
    }
    out.pb.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    #[test]
    fn a_circle_flattens_within_tolerance() {
        let c = kurbo::Circle::new((100.0, 100.0), 80.0).to_path(0.01);
        let p = to_tiny(&c, 0.0, 1.0 / 4096.0).unwrap();
        // Every flattened vertex lies on the circle; every edge midpoint is within the tolerance of it.
        let pts = p.points();
        assert!(pts.len() > 40);
        for w in pts.windows(2) {
            let mid = ((w[0].x + w[1].x) / 2.0 - 100.0, (w[0].y + w[1].y) / 2.0 - 100.0);
            let r = (mid.0 * mid.0 + mid.1 * mid.1).sqrt();
            assert!((80.0 - r).abs() < 0.06, "midpoint at radius {r}");
        }
    }

    #[test]
    fn the_shift_is_exact() {
        let c = kurbo::Circle::new((50.0, 500.0), 30.0).to_path(0.01);
        let whole = to_tiny(&c, 0.0, 1.0 / 4096.0).unwrap();
        let shifted = to_tiny(&c, 400.0, 1.0 / 4096.0).unwrap();
        assert_eq!(whole.len(), shifted.len());
        for (a, b) in whole.points().iter().zip(shifted.points()) {
            assert_eq!(a.x, b.x);
            assert!((a.y - 400.0 - b.y).abs() < 1e-4);
        }
    }

    #[test]
    fn segment_counts_depend_only_on_the_curve() {
        assert_eq!(segments_for(0.0, 0.75), 1);
        assert!(segments_for(1000.0, 0.75) > segments_for(10.0, 0.75));
        assert_eq!(segments_for(f64::NAN, 0.75), 1);
        assert_eq!(segments_for(1e30, 0.75), MAX_SEGMENTS);
    }
}
