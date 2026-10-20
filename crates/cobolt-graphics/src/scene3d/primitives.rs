// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The six primitives: box, sphere, plane, cylinder, cone, torus.
//!
//! Every surface is a grid of vertices `(row i, column j)`; the columns' tangent
//! crossed with the rows' tangent is the outward normal, which fixes the
//! winding (counter-clockwise from outside) once for all of them.

use std::f32::consts::{PI, TAU};

use super::math::Vec3;
use super::mesh::MeshData;
use crate::error::{GraphicsError, Result};

/// The most segments a round primitive takes (a program asking for more gets an error, not a minute of waiting).
pub const MAX_SEGMENTS: u32 = 512;

struct Builder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl Builder {
    fn new() -> Self {
        Builder { positions: Vec::new(), normals: Vec::new(), uvs: Vec::new(), indices: Vec::new() }
    }

    /// A `(rows + 1) x (cols + 1)` grid of vertices from a function of `(i, j)` giving position, normal and uv.
    fn grid(&mut self, rows: u32, cols: u32, f: impl Fn(u32, u32) -> (Vec3, Vec3, [f32; 2])) {
        let base = self.positions.len() as u32;
        for i in 0..=rows {
            for j in 0..=cols {
                let (p, n, uv) = f(i, j);
                self.positions.push(p.to_array());
                self.normals.push(n.to_array());
                self.uvs.push(uv);
            }
        }
        let stride = cols + 1;
        for i in 0..rows {
            for j in 0..cols {
                let a = base + i * stride + j;
                self.indices.extend_from_slice(&[a, a + 1, a + stride, a + 1, a + stride + 1, a + stride]);
            }
        }
    }

    /// A flat disc fan at height `y` around the Y axis. `up` is the direction of its normal.
    fn cap(&mut self, y: f32, radius: f32, segments: u32, up: bool) {
        let base = self.positions.len() as u32;
        let n = Vec3::new(0.0, if up { 1.0 } else { -1.0 }, 0.0);
        self.positions.push([0.0, y, 0.0]);
        self.normals.push(n.to_array());
        self.uvs.push([0.5, 0.5]);
        for j in 0..=segments {
            let phi = TAU * j as f32 / segments as f32;
            let (s, c) = phi.sin_cos();
            self.positions.push([radius * c, y, radius * s]);
            self.normals.push(n.to_array());
            self.uvs.push([0.5 + 0.5 * c, 0.5 + 0.5 * s]);
        }
        for j in 0..segments {
            let (p0, p1) = (base + 1 + j, base + 2 + j);
            if up {
                self.indices.extend_from_slice(&[base, p1, p0]);
            } else {
                self.indices.extend_from_slice(&[base, p0, p1]);
            }
        }
    }

    fn finish(self) -> Result<MeshData> {
        MeshData::new(self.positions, self.normals, self.uvs, self.indices, vec![], vec![])
    }
}

fn pos(what: &str, v: f32) -> Result<()> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(GraphicsError::invalid(format!("{what} must be greater than zero")))
    }
}

fn seg(what: &str, v: u32, min: u32) -> Result<()> {
    if (min..=MAX_SEGMENTS).contains(&v) {
        Ok(())
    } else {
        Err(GraphicsError::invalid(format!("{what} must be from {min} to {MAX_SEGMENTS}")))
    }
}

/// A box of this size centred on the origin.
pub fn cuboid(size: [f32; 3]) -> Result<MeshData> {
    for s in size {
        pos("a box side", s)?;
    }
    let (hx, hy, hz) = (size[0] / 2.0, size[1] / 2.0, size[2] / 2.0);
    let mut b = Builder::new();
    // (origin, u axis, v axis): u x v is the outward normal.
    let faces = [
        (Vec3::new(-hx, hy, hz), Vec3::new(2.0 * hx, 0.0, 0.0), Vec3::new(0.0, 0.0, -2.0 * hz)),
        (Vec3::new(-hx, -hy, -hz), Vec3::new(2.0 * hx, 0.0, 0.0), Vec3::new(0.0, 0.0, 2.0 * hz)),
        (Vec3::new(hx, -hy, hz), Vec3::new(0.0, 0.0, -2.0 * hz), Vec3::new(0.0, 2.0 * hy, 0.0)),
        (Vec3::new(-hx, -hy, -hz), Vec3::new(0.0, 0.0, 2.0 * hz), Vec3::new(0.0, 2.0 * hy, 0.0)),
        (Vec3::new(-hx, -hy, hz), Vec3::new(2.0 * hx, 0.0, 0.0), Vec3::new(0.0, 2.0 * hy, 0.0)),
        (Vec3::new(hx, -hy, -hz), Vec3::new(-2.0 * hx, 0.0, 0.0), Vec3::new(0.0, 2.0 * hy, 0.0)),
    ];
    for (origin, u, v) in faces {
        let n = u.cross(v).normalize();
        b.grid(1, 1, |i, j| (origin + u * j as f32 + v * i as f32, n, [j as f32, 1.0 - i as f32]));
    }
    b.finish()
}

/// A UV sphere.
pub fn sphere(radius: f32, segments: u32) -> Result<MeshData> {
    pos("a radius", radius)?;
    seg("the segment count", segments, 3)?;
    let rings = (segments / 2).max(2);
    let mut b = Builder::new();
    b.grid(rings, segments, |i, j| {
        let theta = PI * i as f32 / rings as f32;
        let phi = TAU * j as f32 / segments as f32;
        let n = Vec3::new(theta.sin() * phi.cos(), theta.cos(), theta.sin() * phi.sin());
        (n * radius, n, [j as f32 / segments as f32, i as f32 / rings as f32])
    });
    b.finish()
}

/// A flat rectangle in the XZ plane facing +Y.
pub fn plane(width: f32, depth: f32) -> Result<MeshData> {
    pos("a plane's width", width)?;
    pos("a plane's depth", depth)?;
    let mut b = Builder::new();
    b.grid(1, 1, |i, j| {
        let x = (j as f32 - 0.5) * width;
        let z = -(i as f32 - 0.5) * depth;
        (Vec3::new(x, 0.0, z), Vec3::Y, [j as f32, i as f32])
    });
    b.finish()
}

/// A cylinder along Y with caps.
pub fn cylinder(radius: f32, height: f32, segments: u32) -> Result<MeshData> {
    pos("a radius", radius)?;
    pos("a height", height)?;
    seg("the segment count", segments, 3)?;
    let mut b = Builder::new();
    b.grid(1, segments, |i, j| {
        let phi = TAU * j as f32 / segments as f32;
        let (s, c) = phi.sin_cos();
        let y = height / 2.0 - height * i as f32;
        (Vec3::new(radius * c, y, radius * s), Vec3::new(c, 0.0, s), [j as f32 / segments as f32, i as f32])
    });
    b.cap(height / 2.0, radius, segments, true);
    b.cap(-height / 2.0, radius, segments, false);
    b.finish()
}

/// A cone along Y, apex up, with a base.
pub fn cone(radius: f32, height: f32, segments: u32) -> Result<MeshData> {
    pos("a radius", radius)?;
    pos("a height", height)?;
    seg("the segment count", segments, 3)?;
    let mut b = Builder::new();
    b.grid(1, segments, |i, j| {
        let phi = TAU * j as f32 / segments as f32;
        let (s, c) = phi.sin_cos();
        let r = if i == 0 { 0.0 } else { radius };
        let y = height / 2.0 - height * i as f32;
        let n = Vec3::new(height * c, radius, height * s).normalize();
        (Vec3::new(r * c, y, r * s), n, [j as f32 / segments as f32, i as f32])
    });
    b.cap(-height / 2.0, radius, segments, false);
    b.finish()
}

/// A torus in the XZ plane: `radius` from the centre to the middle of the tube, `tube` the tube's radius.
pub fn torus(radius: f32, tube: f32, radial_segments: u32, tubular_segments: u32) -> Result<MeshData> {
    pos("a radius", radius)?;
    pos("a tube radius", tube)?;
    seg("the radial segment count", radial_segments, 3)?;
    seg("the tubular segment count", tubular_segments, 3)?;
    let mut b = Builder::new();
    b.grid(radial_segments, tubular_segments, |i, j| {
        let u = TAU * j as f32 / tubular_segments as f32;
        let v = -TAU * i as f32 / radial_segments as f32;
        let n = Vec3::new(v.cos() * u.cos(), v.sin(), v.cos() * u.sin());
        let center = Vec3::new(radius * u.cos(), 0.0, radius * u.sin());
        (center + n * tube, n, [j as f32 / tubular_segments as f32, i as f32 / radial_segments as f32])
    });
    b.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every triangle must face outwards: its geometric normal agrees with the vertex normals.
    fn winding_agrees_with_normals(m: &MeshData) {
        let mut bad = 0;
        for t in m.indices.chunks_exact(3) {
            let p: Vec<Vec3> = t.iter().map(|&i| Vec3::from_array(m.positions[i as usize])).collect();
            let g = (p[1] - p[0]).cross(p[2] - p[0]);
            if g.length_squared() < 1.0e-14 {
                continue; // a degenerate sliver (the cone's apex row)
            }
            let n: Vec3 = t.iter().map(|&i| Vec3::from_array(m.normals[i as usize])).sum();
            if g.dot(n) <= 0.0 {
                bad += 1;
            }
        }
        assert_eq!(bad, 0, "{bad} triangle(s) wind the wrong way in {m:?}");
    }

    #[test]
    fn every_primitive_winds_outwards_and_has_sane_counts() {
        let b = cuboid([2.0, 1.0, 3.0]).unwrap();
        assert_eq!((b.positions.len(), b.triangle_count()), (24, 12));
        winding_agrees_with_normals(&b);
        assert_eq!(b.bounds.max, Vec3::new(1.0, 0.5, 1.5));
        let s = sphere(1.0, 16).unwrap();
        assert_eq!(s.triangle_count(), 8 * 16 * 2);
        winding_agrees_with_normals(&s);
        let p = plane(4.0, 2.0).unwrap();
        assert_eq!(p.triangle_count(), 2);
        winding_agrees_with_normals(&p);
        assert!(p.normals.iter().all(|n| *n == [0.0, 1.0, 0.0]));
        let c = cylinder(1.0, 2.0, 12).unwrap();
        assert_eq!(c.triangle_count(), 12 * 2 + 12 * 2);
        winding_agrees_with_normals(&c);
        let k = cone(1.0, 2.0, 12).unwrap();
        winding_agrees_with_normals(&k);
        let t = torus(2.0, 0.5, 8, 16).unwrap();
        assert_eq!(t.triangle_count(), 8 * 16 * 2);
        winding_agrees_with_normals(&t);
    }

    #[test]
    fn sphere_normals_point_away_from_the_centre() {
        let s = sphere(2.0, 24).unwrap();
        for (p, n) in s.positions.iter().zip(&s.normals) {
            let d = Vec3::from_array(*p).normalize().dot(Vec3::from_array(*n));
            assert!(d > 0.999, "{d}");
        }
    }

    #[test]
    fn nonsense_sizes_are_refused() {
        assert!(cuboid([1.0, 0.0, 1.0]).is_err());
        assert!(sphere(-1.0, 16).is_err());
        assert!(sphere(1.0, 2).is_err());
        assert!(sphere(1.0, 100_000).is_err());
        assert!(cylinder(f32::NAN, 1.0, 8).is_err());
        assert!(torus(1.0, 0.2, 2, 8).is_err());
    }
}
