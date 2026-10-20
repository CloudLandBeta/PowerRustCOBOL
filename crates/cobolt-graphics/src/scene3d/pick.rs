// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Picking: the node under a pixel, by ray casting through `parry3d`.
//!
//! The ray from the camera through the pixel is carried into each mesh's own
//! space (so a scaled, rotated or instanced node needs no special case), tested
//! against the mesh's box and then its triangles through the collision shape
//! `parry3d` builds once per mesh. A skinned mesh is posed on the CPU first.

use parry3d::query::{Ray, RayCast};

use super::math::{ray_triangle, Mat4, Vec3};
use super::scene::Scene3D;
use crate::error::{GraphicsError, Result};
use crate::handles::Handle;

/// What a pick found.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PickHit {
    /// The node that draws the thing hit.
    pub node: Handle,
    /// Which copy, for an instanced node (0 otherwise).
    pub instance: u32,
    /// Distance from the camera to the hit (along the view ray).
    pub distance: f32,
    /// The point hit, in world space.
    pub point: [f32; 3],
    /// The surface normal there, in world space.
    pub normal: [f32; 3],
}

impl Scene3D {
    /// The ray through a point of a `width` x `height` viewport (pixels, origin top-left):
    /// where it starts and which way it goes (a unit vector), plus how far it reaches.
    pub fn view_ray(view_proj: &Mat4, width: u32, height: u32, x: f32, y: f32) -> Result<(Vec3, Vec3, f32)> {
        if width == 0 || height == 0 || !x.is_finite() || !y.is_finite() {
            return Err(GraphicsError::invalid("a pick needs a viewport with a size and a point with finite coordinates"));
        }
        let ndc_x = 2.0 * x / width as f32 - 1.0;
        let ndc_y = 1.0 - 2.0 * y / height as f32;
        let inv = view_proj.inverse();
        let near = inv.project_point3(Vec3::new(ndc_x, ndc_y, 0.0));
        let far = inv.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));
        let span = far - near;
        let len = span.length();
        if !len.is_finite() || len <= 0.0 {
            return Err(GraphicsError::invalid("the camera's view cannot be inverted"));
        }
        Ok((near, span / len, len))
    }

    /// `Pick`: the nearest thing under pixel `(x, y)` of a `width` x `height` view from `camera`
    /// (`None`: the first camera, or the automatic one), or `None` when there is nothing.
    pub fn pick(&mut self, camera: Option<Handle>, width: u32, height: u32, x: f32, y: f32) -> Result<Option<PickHit>> {
        if width == 0 || height == 0 {
            return Err(GraphicsError::invalid("a pick needs a viewport with a size"));
        }
        let aspect = width as f32 / height as f32;
        let frame = self.build_frame(camera, aspect)?;
        let (origin, dir, max_t) = Self::view_ray(&frame.view_proj, width, height, x, y)?;

        let mut best: Option<PickHit> = None;
        for d in &frame.draws {
            for &i in &d.visible {
                let inst = &d.instances[i as usize];
                // Quick reject against the instance's world box - except for a skinned mesh, whose pose
                // can leave the box of its bind pose.
                if d.joints.is_none() && inst.aabb.ray_hit(origin, dir, best.map(|b| b.distance).unwrap_or(max_t)).is_none() {
                    continue;
                }
                let hit = match &d.joints {
                    Some(joints) => pick_skinned(&d.data, joints, &inst.model, origin, dir, max_t),
                    None => pick_rigid(&d.data, &inst.model, origin, dir, max_t),
                };
                if let Some((t, normal)) = hit {
                    if best.map(|b| t < b.distance).unwrap_or(true) {
                        // `distance` holds the ray parameter while the search runs.
                        best = Some(PickHit { node: d.node, instance: i, distance: t, point: (origin + dir * t).to_array(), normal: normal.to_array() });
                    }
                }
            }
        }
        // The ray starts on the near plane; report the distance from the camera itself.
        Ok(best.map(|mut b| {
            b.distance = (Vec3::from_array(b.point) - frame.eye).length();
            b
        }))
    }
}

fn pick_rigid(data: &super::mesh::MeshData, model: &Mat4, origin: Vec3, dir: Vec3, max_t: f32) -> Option<(f32, Vec3)> {
    let inv = model.inverse();
    let lo = inv.transform_point3(origin);
    let ld = inv.transform_vector3(dir);
    if !ld.is_finite() || ld.length_squared() <= 0.0 {
        return None;
    }
    // The local ray shares the world ray's parameter, because the map between the spaces is affine.
    data.bounds.ray_hit(lo, ld, max_t)?;
    let shape = data.trimesh()?;
    let hit = shape.cast_local_ray_and_get_normal(&Ray::new(lo, ld), max_t, true)?;
    // Normals move with the inverse transpose.
    let n = inv.transpose().transform_vector3(hit.normal).try_normalize().unwrap_or(Vec3::Y);
    Some((hit.time_of_impact, n))
}

fn pick_skinned(data: &super::mesh::MeshData, joints: &[Mat4], model: &Mat4, origin: Vec3, dir: Vec3, max_t: f32) -> Option<(f32, Vec3)> {
    // Pose every vertex, then test each triangle in world space.
    let posed: Vec<Vec3> = data
        .positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (j, w) = (data.joints[i], data.weights[i]);
            let mut m = Mat4::ZERO;
            for k in 0..4 {
                if w[k] > 0.0 {
                    m += joints.get(j[k] as usize).copied().unwrap_or(Mat4::IDENTITY) * w[k];
                }
            }
            model.transform_point3(m.transform_point3(Vec3::from_array(*p)))
        })
        .collect();
    let mut best: Option<(f32, Vec3)> = None;
    for t in data.indices.chunks_exact(3) {
        let (a, b, c) = (posed[t[0] as usize], posed[t[1] as usize], posed[t[2] as usize]);
        if let Some(d) = ray_triangle(origin, dir, a, b, c) {
            if d <= max_t && best.map(|(bd, _)| d < bd).unwrap_or(true) {
                let n = (b - a).cross(c - a).try_normalize().unwrap_or(Vec3::Y);
                best = Some((d, n));
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::Limits;

    fn setup() -> (Scene3D, Handle) {
        let mut s = Scene3D::new(Limits::default());
        let cam = s.add_perspective_camera(60.0, 0.1, 100.0).unwrap();
        s.set_position(cam, [0.0, 0.0, 10.0]).unwrap();
        (s, cam)
    }

    #[test]
    fn the_nearest_node_under_a_pixel_wins() {
        let (mut s, cam) = setup();
        let far = s.add_sphere(1.0, 24).unwrap();
        s.set_position(far, [0.0, 0.0, -5.0]).unwrap();
        let near = s.add_box([2.0, 2.0, 2.0]).unwrap();
        s.set_position(near, [0.0, 0.0, 2.0]).unwrap();
        // The middle of a 200 x 200 view looks straight down -Z through both.
        let hit = s.pick(Some(cam), 200, 200, 100.0, 100.0).unwrap().expect("a hit");
        assert_eq!(hit.node, near);
        assert!((hit.distance - 7.0).abs() < 1e-3, "the box's front face is 7 away: {}", hit.distance);
        assert!((hit.point[2] - 3.0).abs() < 1e-3);
        assert!((hit.normal[2] - 1.0).abs() < 1e-4, "{:?}", hit.normal);
        // Hide the box and the sphere behind it is found.
        s.set_visible(near, false).unwrap();
        let hit = s.pick(Some(cam), 200, 200, 100.0, 100.0).unwrap().unwrap();
        assert_eq!(hit.node, far);
        // Off to the side there is nothing.
        assert_eq!(s.pick(Some(cam), 200, 200, 5.0, 100.0).unwrap(), None);
    }

    #[test]
    fn picking_follows_scale_rotation_and_instances() {
        let (mut s, cam) = setup();
        let b = s.add_box([1.0, 1.0, 1.0]).unwrap();
        s.set_scale(b, [4.0, 1.0, 1.0]).unwrap();
        s.set_rotation(b, [0.0, 0.0, 90.0]).unwrap(); // now tall: 4 units in Y
        let unit = 200.0 / (2.0 * 10.0 * (30.0f32.to_radians()).tan()); // pixels per world unit at distance 10
        let top = s.pick(Some(cam), 200, 200, 100.0, 100.0 - 1.5 * unit).unwrap();
        assert!(top.is_some(), "a pixel 1.5 units above the centre hits the stretched, rotated box (it reaches 2 up)");
        let miss = s.pick(Some(cam), 200, 200, 100.0 + 1.5 * unit, 100.0).unwrap();
        assert!(miss.is_none(), "but 1.5 units to the side is outside its 0.5 half width");

        // Instances: copies at x = -3 and x = +3 (the node itself moved out of the way).
        let (mut s, cam) = setup();
        let n = s.add_box([1.0; 3]).unwrap();
        let mut m = Vec::new();
        m.extend_from_slice(&Mat4::from_translation(Vec3::new(-3.0, 0.0, 0.0)).to_cols_array());
        m.extend_from_slice(&Mat4::from_translation(Vec3::new(3.0, 0.0, 0.0)).to_cols_array());
        s.set_instances(n, &m).unwrap();
        let left = s.pick(Some(cam), 200, 200, 100.0 - 3.0 * unit, 100.0).unwrap().unwrap();
        let right = s.pick(Some(cam), 200, 200, 100.0 + 3.0 * unit, 100.0).unwrap().unwrap();
        assert_eq!((left.node, left.instance), (n, 0));
        assert_eq!((right.node, right.instance), (n, 1));
        assert!(s.pick(Some(cam), 200, 200, 100.0, 100.0).unwrap().is_none(), "nothing between them");
    }

    #[test]
    fn a_skinned_mesh_is_picked_where_its_pose_puts_it() {
        use crate::scene3d::anim::Skin;
        use crate::scene3d::mesh::MeshData;
        let (mut s, cam) = setup();
        // A 2x2 quad in the XY plane following one joint.
        let pos = vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0]];
        let mesh = MeshData::new(pos, vec![], vec![], vec![0, 1, 2, 0, 2, 3], vec![[0, 0, 0, 0]; 4], vec![[1.0, 0.0, 0.0, 0.0]; 4]).unwrap();
        let mh = s.add_mesh(mesh).unwrap();
        let node = s.add_mesh_node(mh, None, "quad").unwrap();
        let joint = s.add_group().unwrap();
        let skin = s.add_skin(Skin { joints: vec![joint], inverse_bind: vec![Mat4::IDENTITY] }).unwrap();
        s.set_skin(node, skin).unwrap();
        assert!(s.pick(Some(cam), 200, 200, 100.0, 100.0).unwrap().is_some(), "bind pose: the quad is in the middle");
        s.set_position(joint, [20.0, 0.0, 0.0]).unwrap();
        assert!(s.pick(Some(cam), 200, 200, 100.0, 100.0).unwrap().is_none(), "the joint moved the quad out of the middle");
    }

    #[test]
    fn a_bad_viewport_or_point_is_an_error() {
        let (mut s, cam) = setup();
        assert!(s.pick(Some(cam), 0, 100, 1.0, 1.0).is_err());
        assert!(s.pick(Some(cam), 100, 100, f32::NAN, 1.0).is_err());
    }
}
