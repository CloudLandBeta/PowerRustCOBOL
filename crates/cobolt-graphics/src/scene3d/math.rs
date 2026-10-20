// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Small geometry helpers: boxes, frustums, rays, orientation.
//!
//! The conventions are glTF's and wgpu's: right-handed, +Y up, a node looks down
//! its **-Z** axis, depth runs from 0 (near) to 1 (far), angles in radians
//! inside the crate (the COBOL-facing methods take degrees and convert).

pub use glam::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4};

/// A right-handed perspective projection with depth running 0 to 1 (wgpu, Metal, Direct3D clip space).
pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    glam::camera::rh::proj::directx::perspective(fov_y, aspect, near, far)
}

/// A right-handed orthographic projection with depth running 0 to 1.
pub fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
    glam::camera::rh::proj::directx::orthographic(left, right, bottom, top, near, far)
}

/// The view matrix of a camera at `eye` looking at `center` (right-handed, -Z forward).
pub fn look_at(eye: Vec3, center: Vec3, up: Vec3) -> Mat4 {
    glam::camera::rh::view::look_at_mat4(eye, center, up)
}

/// An axis-aligned box.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Aabb {
    /// The smallest corner.
    pub min: Vec3,
    /// The largest corner.
    pub max: Vec3,
}

impl Aabb {
    /// A box that contains nothing (the identity of [`Aabb::union`]).
    pub const EMPTY: Aabb = Aabb { min: Vec3::splat(f32::INFINITY), max: Vec3::splat(f32::NEG_INFINITY) };

    /// Whether the box contains at least one point.
    pub fn is_valid(&self) -> bool {
        self.min.x <= self.max.x && self.min.y <= self.max.y && self.min.z <= self.max.z
    }

    /// The box around some points.
    pub fn from_points<'a>(points: impl IntoIterator<Item = &'a [f32; 3]>) -> Aabb {
        let mut b = Aabb::EMPTY;
        for p in points {
            b.expand(Vec3::from_array(*p));
        }
        b
    }

    /// Grow to contain a point.
    pub fn expand(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    /// The smallest box containing both.
    pub fn union(&self, other: &Aabb) -> Aabb {
        Aabb { min: self.min.min(other.min), max: self.max.max(other.max) }
    }

    /// The middle.
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Half the size on each axis.
    pub fn half_extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    /// The box that contains this one after `m` has moved it (all eight corners are carried).
    pub fn transformed(&self, m: &Mat4) -> Aabb {
        if !self.is_valid() {
            return *self;
        }
        let mut out = Aabb::EMPTY;
        for i in 0..8 {
            let c = Vec3::new(
                if i & 1 == 0 { self.min.x } else { self.max.x },
                if i & 2 == 0 { self.min.y } else { self.max.y },
                if i & 4 == 0 { self.min.z } else { self.max.z },
            );
            out.expand(m.transform_point3(c));
        }
        out
    }

    /// The radius of the sphere around the centre that contains the box.
    pub fn radius(&self) -> f32 {
        self.half_extents().length()
    }

    /// Where a ray first meets the box within `[0, max_t]`, if it does (the slab method).
    pub fn ray_hit(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<f32> {
        let mut t0 = 0.0f32;
        let mut t1 = max_t;
        for a in 0..3 {
            let (o, d, lo, hi) = (origin[a], dir[a], self.min[a], self.max[a]);
            if d.abs() < 1.0e-12 {
                if o < lo || o > hi {
                    return None;
                }
            } else {
                let inv = 1.0 / d;
                let (mut ta, mut tb) = ((lo - o) * inv, (hi - o) * inv);
                if ta > tb {
                    std::mem::swap(&mut ta, &mut tb);
                }
                t0 = t0.max(ta);
                t1 = t1.min(tb);
                if t0 > t1 {
                    return None;
                }
            }
        }
        Some(t0)
    }
}

/// The six planes of a view volume, normals pointing inwards.
#[derive(Copy, Clone, Debug)]
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    /// The volume of a combined view-projection matrix with depth running 0 to 1.
    pub fn from_view_proj(m: &Mat4) -> Frustum {
        let r0 = m.row(0);
        let r1 = m.row(1);
        let r2 = m.row(2);
        let r3 = m.row(3);
        let mut planes = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2];
        for p in planes.iter_mut() {
            let len = p.truncate().length();
            if len > 0.0 {
                *p /= len;
            }
        }
        Frustum { planes }
    }

    /// Whether the box may be inside (conservative: it never says "outside" for a box that is inside).
    pub fn intersects_aabb(&self, b: &Aabb) -> bool {
        if !b.is_valid() {
            return false;
        }
        for p in &self.planes {
            // The corner furthest along the plane's normal.
            let v = Vec3::new(if p.x >= 0.0 { b.max.x } else { b.min.x }, if p.y >= 0.0 { b.max.y } else { b.min.y }, if p.z >= 0.0 { b.max.z } else { b.min.z });
            if p.truncate().dot(v) + p.w < 0.0 {
                return false;
            }
        }
        true
    }
}

/// The rotation that makes a node at `eye` look at `target` (its -Z axis towards the target, `up` as the up hint).
pub fn look_rotation(eye: Vec3, target: Vec3, up: Vec3) -> Quat {
    let forward = (target - eye).try_normalize().unwrap_or(Vec3::NEG_Z);
    // A hint parallel to the view direction has no "sideways"; pick another.
    let hint = if forward.cross(up).length_squared() < 1.0e-8 { if forward.dot(Vec3::Y).abs() < 0.99 { Vec3::Y } else { Vec3::X } } else { up };
    let right = forward.cross(hint).normalize();
    let true_up = right.cross(forward);
    Quat::from_mat3(&Mat3::from_cols(right, true_up, -forward)).normalize()
}

/// A ray against a triangle (Moeller-Trumbore, both sides). The distance along `dir`, if hit.
pub fn ray_triangle(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let e1 = b - a;
    let e2 = c - a;
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1.0e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t >= 0.0).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_is_carried_through_a_rotation() {
        let b = Aabb { min: Vec3::splat(-1.0), max: Vec3::splat(1.0) };
        let r = b.transformed(&Mat4::from_rotation_z(std::f32::consts::FRAC_PI_4));
        assert!((r.max.x - 2.0f32.sqrt()).abs() < 1e-5, "{r:?}");
        assert!((r.max.z - 1.0).abs() < 1e-6);
        assert!(!Aabb::EMPTY.is_valid());
    }

    #[test]
    fn the_frustum_keeps_what_is_in_front_and_drops_what_is_behind_or_beside() {
        let proj = perspective(1.0, 1.0, 0.1, 100.0);
        let f = Frustum::from_view_proj(&proj);
        let at = |c: Vec3| Aabb { min: c - Vec3::splat(0.5), max: c + Vec3::splat(0.5) };
        let tiny = |c: Vec3| Aabb { min: c - Vec3::splat(0.01), max: c + Vec3::splat(0.01) };
        assert!(f.intersects_aabb(&at(Vec3::new(0.0, 0.0, -10.0))), "straight ahead");
        assert!(!f.intersects_aabb(&at(Vec3::new(0.0, 0.0, 10.0))), "behind the camera");
        assert!(!f.intersects_aabb(&at(Vec3::new(0.0, 0.0, -200.0))), "past the far plane");
        assert!(!f.intersects_aabb(&at(Vec3::new(100.0, 0.0, -10.0))), "far to the side");
        assert!(f.intersects_aabb(&at(Vec3::new(5.4, 0.0, -10.0))), "grazing the right plane");
        assert!(!f.intersects_aabb(&tiny(Vec3::new(0.0, 0.0, -0.05))), "between the camera and the near plane");
        assert!(f.intersects_aabb(&at(Vec3::new(0.0, 0.0, -0.05))), "straddling the near plane");
    }

    #[test]
    fn looking_down_the_negative_z_axis_is_the_identity() {
        let q = look_rotation(Vec3::ZERO, Vec3::new(0.0, 0.0, -5.0), Vec3::Y);
        assert!(q.angle_between(Quat::IDENTITY) < 1e-5);
        let q = look_rotation(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), Vec3::Y);
        let fwd = q * Vec3::NEG_Z;
        assert!((fwd - Vec3::X).length() < 1e-5, "{fwd:?}");
        // Straight up: no NaN.
        let q = look_rotation(Vec3::ZERO, Vec3::new(0.0, 5.0, 0.0), Vec3::Y);
        assert!(q.is_finite() && ((q * Vec3::NEG_Z) - Vec3::Y).length() < 1e-5);
    }

    #[test]
    fn rays_hit_boxes_and_triangles() {
        let b = Aabb { min: Vec3::new(-1.0, -1.0, -1.0), max: Vec3::new(1.0, 1.0, 1.0) };
        assert_eq!(b.ray_hit(Vec3::new(0.0, 0.0, 5.0), Vec3::NEG_Z, 100.0), Some(4.0));
        assert_eq!(b.ray_hit(Vec3::new(0.0, 0.0, 5.0), Vec3::Z, 100.0), None);
        assert_eq!(b.ray_hit(Vec3::new(0.0, 0.0, 0.0), Vec3::Z, 100.0), Some(0.0), "from inside");
        assert_eq!(b.ray_hit(Vec3::new(3.0, 0.0, 5.0), Vec3::NEG_Z, 100.0), None);
        let t = ray_triangle(Vec3::new(0.2, 0.2, 5.0), Vec3::NEG_Z, Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(t, Some(5.0));
        assert!(ray_triangle(Vec3::new(2.0, 2.0, 5.0), Vec3::NEG_Z, Vec3::ZERO, Vec3::X, Vec3::Y).is_none());
    }
}
