// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! From scene to frame: everything a renderer needs to draw one picture, worked
//! out on the CPU where it can be tested without a GPU.
//!
//! [`Scene3D::build_frame`] walks the hierarchy (a hidden node hides what is under
//! it), resolves the camera, **culls** every instance against the camera's
//! frustum, collects the lights with a view-projection matrix for each one that
//! casts a **shadow**, and computes the **joint matrices** of skinned meshes.
//! The GPU stage in [`gpu`](super::gpu) only uploads and draws what is here.

use std::sync::Arc;

use super::math::{look_at, look_rotation, orthographic, perspective, Aabb, Frustum, Mat4, Vec3};
use super::mesh::MeshData;
use super::scene::{AlphaMode, Background, Camera, Content, Fog, LightKind, Material, Scene3D};
use crate::error::{GraphicsError, Result};
use crate::handles::Handle;

/// A node with at least this many copies has them boxed and culled in parallel chunks.
const PAR_INSTANCES: usize = 4096;
/// Copies per parallel chunk.
const CHUNK: usize = 2048;

/// The most lights a frame carries (the rest are dropped, lowest handles kept).
pub const MAX_LIGHTS: usize = 8;
/// The most shadow maps a frame carries.
pub const MAX_SHADOW_MAPS: usize = 2;

/// One copy of a mesh.
#[derive(Copy, Clone, Debug)]
pub struct Instance {
    /// Mesh space to world space.
    pub model: Mat4,
    /// The box around the copy, in world space.
    pub aabb: Aabb,
}

/// One mesh with all its copies and its material.
#[derive(Clone)]
pub struct DrawItem {
    /// The node that draws it.
    pub node: Handle,
    /// The mesh.
    pub mesh: Handle,
    /// Its geometry.
    pub data: Arc<MeshData>,
    /// How it looks.
    pub material: Material,
    /// Every copy.
    pub instances: Vec<Instance>,
    /// Which copies the camera can see (indices into `instances`).
    pub visible: Vec<u32>,
    /// The joint matrices of a skinned mesh.
    pub joints: Option<Vec<Mat4>>,
}

impl std::fmt::Debug for DrawItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DrawItem(node {}, {:?}, {} of {} instance(s) visible)", self.node, self.data, self.visible.len(), self.instances.len())
    }
}

/// A light in world space, ready for a shader.
#[derive(Copy, Clone, Debug)]
pub struct LightItem {
    /// Which kind.
    pub kind: LightKind,
    /// Where it is (point and spot).
    pub position: Vec3,
    /// The way it points (directional and spot): the direction the light travels.
    pub direction: Vec3,
    /// Colour times intensity.
    pub color: Vec3,
    /// Fade-out distance, 0 for none.
    pub range: f32,
    /// The view-projection matrix of its shadow map, if it casts one.
    pub shadow: Option<Mat4>,
}

/// What a frame contains, for tests and for the program (`GetStats`).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct FrameStats {
    /// Mesh nodes that are visible in the hierarchy.
    pub mesh_nodes: usize,
    /// Copies of meshes in the scene (visible nodes only).
    pub instances_total: usize,
    /// Copies inside the camera's view.
    pub instances_drawn: usize,
    /// Copies culled.
    pub instances_culled: usize,
    /// Triangles drawn (copies in view times their triangles).
    pub triangles_drawn: u64,
    /// Lights used.
    pub lights: usize,
    /// Lights dropped because there were too many.
    pub lights_dropped: usize,
    /// Shadow maps rendered.
    pub shadow_maps: usize,
    /// Skinned meshes.
    pub skinned: usize,
}

/// Everything one picture needs.
#[derive(Clone, Debug)]
pub struct Frame {
    /// The camera node used (`None` for the automatic camera).
    pub camera_node: Option<Handle>,
    /// World to camera.
    pub view: Mat4,
    /// Camera to clip.
    pub proj: Mat4,
    /// `proj * view`.
    pub view_proj: Mat4,
    /// The camera's position.
    pub eye: Vec3,
    /// What to draw: solid and masked items first, then blended ones from far to near.
    pub draws: Vec<DrawItem>,
    /// The lights.
    pub lights: Vec<LightItem>,
    /// Ambient colour times intensity.
    pub ambient: [f32; 3],
    /// What to clear to.
    pub background: Background,
    /// Fog, if any.
    pub fog: Option<Fog>,
    /// Counts.
    pub stats: FrameStats,
}

impl Scene3D {
    /// The view matrix of a camera node: the inverse of its world matrix without scale.
    pub(super) fn camera_view(world: &Mat4) -> Mat4 {
        let (_, rot, trans) = world.to_scale_rotation_translation();
        Mat4::from_rotation_translation(rot, trans).inverse()
    }

    /// The camera to use: the one named, else the first camera node, else `None`.
    pub fn resolve_camera(&self, requested: Option<Handle>) -> Result<Option<Handle>> {
        if let Some(c) = requested {
            return match &self.nodes.get(c)?.content {
                Content::Camera(_) => Ok(Some(c)),
                _ => Err(GraphicsError::invalid("that node is not a camera")),
            };
        }
        Ok(self.nodes.iter().find(|(_, n)| matches!(n.content, Content::Camera(_))).map(|(h, _)| h))
    }

    /// Work out one frame for a viewport of `aspect` (width / height).
    pub fn build_frame(&mut self, camera: Option<Handle>, aspect: f32) -> Result<Frame> {
        self.update_transforms();
        let camera_node = self.resolve_camera(camera)?;

        // Which nodes are visible: hidden ones hide their subtrees.
        let mut visible_node = std::collections::HashSet::new();
        let mut stack: Vec<Handle> = self.nodes.iter().filter(|(_, n)| n.parent.is_none()).map(|(h, _)| h).collect();
        stack.reverse();
        while let Some(h) = stack.pop() {
            let Ok(n) = self.nodes.get(h) else { continue };
            if !n.visible {
                continue;
            }
            visible_node.insert(h);
            stack.extend(n.children.iter().rev().copied());
        }

        // Geometry in handle order (stable).
        let mut stats = FrameStats::default();
        let mut draws: Vec<DrawItem> = Vec::new();
        let default_material = Material::default();
        let mut scene_box = Aabb::EMPTY;
        let handles: Vec<Handle> = self.nodes.iter().map(|(h, _)| h).collect();
        for h in &handles {
            if !visible_node.contains(h) {
                continue;
            }
            let n = self.nodes.get(*h)?;
            let Content::Mesh(m) = &n.content else { continue };
            let Ok(entry) = self.meshes.get(m.mesh) else { continue };
            let data = entry.data.clone();
            let material = m.material.and_then(|mh| self.materials.get(mh).ok()).unwrap_or(&default_material).clone();
            let world = n.world;
            let instances: Vec<Instance> = if m.instances.is_empty() {
                vec![Instance { model: world, aabb: data.bounds.transformed(&world) }]
            } else if m.instances.len() >= PAR_INSTANCES {
                // Many copies: carry each one's box in parallel chunks, joined in order (the same list for any number of threads).
                let (bounds, list) = (data.bounds, &m.instances);
                let chunks = list.len().div_ceil(CHUNK);
                crate::par::map_range(chunks, |c| {
                    let (lo, hi) = (c * CHUNK, ((c + 1) * CHUNK).min(list.len()));
                    list[lo..hi]
                        .iter()
                        .map(|i| {
                            let model = world * *i;
                            Instance { model, aabb: bounds.transformed(&model) }
                        })
                        .collect::<Vec<_>>()
                })
                .into_iter()
                .flatten()
                .collect()
            } else {
                m.instances
                    .iter()
                    .map(|i| {
                        let model = world * *i;
                        Instance { model, aabb: data.bounds.transformed(&model) }
                    })
                    .collect()
            };
            for i in &instances {
                scene_box = scene_box.union(&i.aabb);
            }
            let skin = n.skin;
            let joints = match (skin, data.is_skinned()) {
                (Some(s), true) => Some(self.joint_matrices_now(s, &world)?),
                _ => None,
            };
            if joints.is_some() {
                stats.skinned += 1;
            }
            draws.push(DrawItem { node: *h, mesh: m.mesh, data, material, instances, visible: Vec::new(), joints });
        }

        // The camera.
        let (view, proj, eye) = match camera_node {
            Some(c) => {
                let n = self.nodes.get(c)?;
                let Content::Camera(cam) = &n.content else { unreachable!("resolve_camera checked") };
                let view = Self::camera_view(&n.world);
                (view, cam.projection(aspect), n.world.w_axis.truncate())
            }
            None => auto_camera(&scene_box, aspect),
        };
        let view_proj = proj * view;
        let frustum = Frustum::from_view_proj(&view_proj);

        // Cull.
        for d in draws.iter_mut() {
            let skinned = d.joints.is_some();
            if !skinned && d.instances.len() >= PAR_INSTANCES {
                // Many copies: test them in parallel chunks and join the answers in order.
                let list = &d.instances;
                let chunks = list.len().div_ceil(CHUNK);
                d.visible = crate::par::map_range(chunks, |c| {
                    let (lo, hi) = (c * CHUNK, ((c + 1) * CHUNK).min(list.len()));
                    (lo..hi).filter(|&i| frustum.intersects_aabb(&list[i].aabb)).map(|i| i as u32).collect::<Vec<u32>>()
                })
                .into_iter()
                .flatten()
                .collect();
            } else {
                for (i, inst) in d.instances.iter().enumerate() {
                    // A skinned mesh's pose can leave its bind-pose box, so it is never culled.
                    if skinned || frustum.intersects_aabb(&inst.aabb) {
                        d.visible.push(i as u32);
                    }
                }
            }
            stats.mesh_nodes += 1;
            stats.instances_total += d.instances.len();
            stats.instances_drawn += d.visible.len();
            stats.triangles_drawn += d.visible.len() as u64 * d.data.triangle_count() as u64;
        }
        stats.instances_culled = stats.instances_total - stats.instances_drawn;

        // Solid first, blended last and far to near.
        let blended = |d: &DrawItem| d.material.alpha_mode == AlphaMode::Blend;
        let dist = |d: &DrawItem| {
            let mut b = Aabb::EMPTY;
            for i in &d.instances {
                b = b.union(&i.aabb);
            }
            if b.is_valid() {
                (b.center() - eye).length_squared()
            } else {
                0.0
            }
        };
        let (mut solid, mut blend): (Vec<DrawItem>, Vec<DrawItem>) = draws.into_iter().partition(|d| !blended(d));
        blend.sort_by(|a, b| dist(b).partial_cmp(&dist(a)).unwrap_or(std::cmp::Ordering::Equal));
        solid.extend(blend);
        let draws = solid;

        // Lights.
        let mut lights = Vec::new();
        let mut shadow_maps = 0usize;
        for h in &handles {
            if !visible_node.contains(h) {
                continue;
            }
            let n = self.nodes.get(*h)?;
            let Content::Light(l) = &n.content else { continue };
            if lights.len() >= MAX_LIGHTS {
                stats.lights_dropped += 1;
                continue;
            }
            let position = n.world.w_axis.truncate();
            let direction = n.world.transform_vector3(Vec3::NEG_Z).try_normalize().unwrap_or(Vec3::NEG_Y);
            let mut item = LightItem { kind: l.kind, position, direction, color: Vec3::from_array(l.color) * l.intensity, range: l.range, shadow: None };
            if l.cast_shadow && !matches!(l.kind, LightKind::Point) && shadow_maps < MAX_SHADOW_MAPS && scene_box.is_valid() {
                item.shadow = Some(shadow_matrix(&item, &scene_box));
                shadow_maps += 1;
            }
            lights.push(item);
        }
        stats.lights = lights.len();
        stats.shadow_maps = shadow_maps;

        Ok(Frame { camera_node, view, proj, view_proj, eye, draws, lights, ambient: self.ambient, background: self.background, fog: self.fog, stats })
    }
}

/// A camera that frames the whole scene when the program has not added one.
fn auto_camera(bounds: &Aabb, aspect: f32) -> (Mat4, Mat4, Vec3) {
    let (center, radius) = if bounds.is_valid() { (bounds.center(), bounds.radius().max(0.01)) } else { (Vec3::ZERO, 1.0) };
    let fov: f32 = 45.0f32.to_radians();
    let fit_fov = if aspect >= 1.0 { fov } else { 2.0 * ((fov / 2.0).tan() * aspect).atan() };
    let dist = radius / (fit_fov / 2.0).sin() * 1.05;
    let eye = center + Vec3::new(0.0, 0.0, dist);
    let rot = look_rotation(eye, center, Vec3::Y);
    let view = Mat4::from_rotation_translation(rot, eye).inverse();
    let proj = Camera::Perspective { fov_y: fov, near: (dist - radius).max(radius * 0.01).max(1.0e-3), far: dist + radius * 2.0 }.projection(aspect);
    (view, proj, eye)
}

/// The view-projection matrix of a light's shadow map, fitted to the scene.
fn shadow_matrix(l: &LightItem, scene: &Aabb) -> Mat4 {
    let center = scene.center();
    let radius = scene.radius().max(0.01);
    match l.kind {
        LightKind::Directional => {
            let dir = l.direction;
            let up = if dir.dot(Vec3::Y).abs() > 0.99 { Vec3::X } else { Vec3::Y };
            let eye = center - dir * (radius * 2.0);
            let view = look_at(eye, eye + dir, up);
            // The tightest orthographic box around the scene as the light sees it.
            let mut lo = Vec3::splat(f32::INFINITY);
            let mut hi = Vec3::splat(f32::NEG_INFINITY);
            for i in 0..8 {
                let c = Vec3::new(
                    if i & 1 == 0 { scene.min.x } else { scene.max.x },
                    if i & 2 == 0 { scene.min.y } else { scene.max.y },
                    if i & 4 == 0 { scene.min.z } else { scene.max.z },
                );
                let p = view.transform_point3(c);
                lo = lo.min(p);
                hi = hi.max(p);
            }
            // Light space looks down -Z: near = -hi.z, far = -lo.z; pad so the edges are not clipped.
            let pad = radius * 0.02 + 1.0e-3;
            let proj = orthographic(lo.x - pad, hi.x + pad, lo.y - pad, hi.y + pad, (-hi.z - pad).max(0.0), -lo.z + pad);
            proj * view
        }
        LightKind::Spot { outer, .. } => {
            let dir = l.direction;
            let up = if dir.dot(Vec3::Y).abs() > 0.99 { Vec3::X } else { Vec3::Y };
            let view = look_at(l.position, l.position + dir, up);
            let far = if l.range > 0.0 { l.range } else { (center - l.position).length() + radius };
            let far = far.max(0.5);
            let proj = perspective((outer * 2.0 + 0.05).min(3.0), 1.0, (far * 0.002).max(0.02), far);
            proj * view
        }
        LightKind::Point => Mat4::IDENTITY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::Limits;

    fn scene_with_camera() -> (Scene3D, Handle) {
        let mut s = Scene3D::new(Limits::default());
        let cam = s.add_perspective_camera(60.0, 0.1, 100.0).unwrap();
        s.set_position(cam, [0.0, 0.0, 10.0]).unwrap();
        (s, cam)
    }

    #[test]
    fn what_is_in_front_is_drawn_and_what_is_behind_or_beside_is_culled() {
        let (mut s, cam) = scene_with_camera();
        let ahead = s.add_box([1.0; 3]).unwrap();
        let behind = s.add_box([1.0; 3]).unwrap();
        s.set_position(behind, [0.0, 0.0, 30.0]).unwrap();
        let aside = s.add_box([1.0; 3]).unwrap();
        s.set_position(aside, [500.0, 0.0, 0.0]).unwrap();
        let f = s.build_frame(Some(cam), 1.0).unwrap();
        assert_eq!(f.stats.mesh_nodes, 3);
        assert_eq!((f.stats.instances_total, f.stats.instances_drawn, f.stats.instances_culled), (3, 1, 2));
        assert_eq!(f.stats.triangles_drawn, 12);
        let drawn: Vec<Handle> = f.draws.iter().filter(|d| !d.visible.is_empty()).map(|d| d.node).collect();
        assert_eq!(drawn, vec![ahead]);
    }

    #[test]
    fn instances_are_culled_one_by_one() {
        let (mut s, cam) = scene_with_camera();
        let n = s.add_box([1.0; 3]).unwrap();
        let mut mats = Vec::new();
        for i in 0..1000 {
            // A row along X from -500 to +499.
            mats.extend_from_slice(&Mat4::from_translation(Vec3::new(i as f32 - 500.0, 0.0, 0.0)).to_cols_array());
        }
        s.set_instances(n, &mats).unwrap();
        let f = s.build_frame(Some(cam), 1.0).unwrap();
        assert_eq!(f.stats.instances_total, 1000);
        assert!(f.stats.instances_drawn > 5 && f.stats.instances_drawn < 20, "only the middle of the row is in view: {}", f.stats.instances_drawn);
        assert_eq!(f.stats.instances_drawn + f.stats.instances_culled, 1000);
        assert!(f.draws[0].visible.windows(2).all(|w| w[0] < w[1]), "in order");
    }

    #[test]
    fn a_hundred_thousand_instances_are_culled_the_same_on_one_thread_and_on_many() {
        let (mut s, cam) = scene_with_camera();
        let n = s.add_box([1.0; 3]).unwrap();
        let mut mats = Vec::new();
        for i in 0..100_000 {
            // A 400 x 250 grid, 3 apart: most of it is outside the view.
            let t = Mat4::from_translation(Vec3::new((i % 400) as f32 * 3.0 - 600.0, ((i / 400) % 5) as f32, -(i / 400) as f32 * 3.0));
            mats.extend_from_slice(&t.to_cols_array());
        }
        s.set_instances(n, &mats).unwrap();
        let one = crate::par::with_threads(1, || s.build_frame(Some(cam), 1.0).unwrap());
        let many = crate::par::with_threads(8, || s.build_frame(Some(cam), 1.0).unwrap());
        assert_eq!(one.stats, many.stats);
        assert_eq!(one.draws[0].visible, many.draws[0].visible, "the same copies are visible, in the same order");
        assert!(one.draws[0].instances.iter().zip(&many.draws[0].instances).all(|(a, b)| a.model == b.model && a.aabb == b.aabb));
        assert!(one.stats.instances_drawn > 100 && one.stats.instances_culled > 50_000, "{:?}", one.stats);
        assert!(one.draws[0].visible.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn hidden_nodes_hide_their_children_and_cost_nothing() {
        let (mut s, cam) = scene_with_camera();
        let group = s.add_group().unwrap();
        let child = s.add_box([1.0; 3]).unwrap();
        s.set_parent(child, Some(group)).unwrap();
        assert_eq!(s.build_frame(Some(cam), 1.0).unwrap().stats.instances_drawn, 1);
        s.set_visible(group, false).unwrap();
        let f = s.build_frame(Some(cam), 1.0).unwrap();
        assert_eq!((f.stats.mesh_nodes, f.stats.instances_total), (0, 0));
        s.set_visible(group, true).unwrap();
        s.set_visible(child, false).unwrap();
        assert_eq!(s.build_frame(Some(cam), 1.0).unwrap().stats.mesh_nodes, 0);
    }

    #[test]
    fn blended_items_come_last_and_far_to_near() {
        let (mut s, cam) = scene_with_camera();
        let mut nodes = Vec::new();
        for z in [0.0, -10.0, -20.0] {
            let n = s.add_box([1.0; 3]).unwrap();
            s.set_position(n, [0.0, 0.0, z]).unwrap();
            nodes.push(n);
        }
        let glass = s.material_of(nodes[0]).unwrap();
        s.set_alpha_mode(glass, AlphaMode::Blend, 0.5).unwrap();
        let glass2 = s.material_of(nodes[1]).unwrap();
        s.set_alpha_mode(glass2, AlphaMode::Blend, 0.5).unwrap();
        let f = s.build_frame(Some(cam), 1.0).unwrap();
        let order: Vec<Handle> = f.draws.iter().map(|d| d.node).collect();
        assert_eq!(order, vec![nodes[2], nodes[1], nodes[0]], "the opaque one, then the far glass, then the near glass");
    }

    #[test]
    fn lights_and_shadow_matrices_are_collected() {
        let (mut s, cam) = scene_with_camera();
        let floor = s.add_plane(20.0, 20.0).unwrap();
        s.set_position(floor, [0.0, -1.0, 0.0]).unwrap();
        let ball = s.add_sphere(1.0, 16).unwrap();
        let sun = s.add_directional_light([1.0, 0.9, 0.8], 3.0, true).unwrap();
        s.set_position(sun, [5.0, 10.0, 5.0]).unwrap();
        s.look_at(sun, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
        let bulb = s.add_point_light([1.0; 3], 2.0, 15.0).unwrap();
        s.set_position(bulb, [0.0, 3.0, 0.0]).unwrap();
        let lamp = s.add_spot_light([1.0; 3], 5.0, 20.0, 15.0, 30.0, true).unwrap();
        s.set_position(lamp, [0.0, 8.0, 0.0]).unwrap();
        s.look_at(lamp, [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]).unwrap();
        let _ = ball;
        let f = s.build_frame(Some(cam), 1.0).unwrap();
        assert_eq!((f.stats.lights, f.stats.shadow_maps), (3, 2));
        let sun = &f.lights[0];
        assert!(sun.shadow.is_some() && matches!(sun.kind, LightKind::Directional));
        assert!((sun.direction - Vec3::new(-5.0, -10.0, -5.0).normalize()).length() < 1e-5);
        assert!((sun.color - Vec3::new(3.0, 2.7, 2.4)).length() < 1e-5, "{:?}", sun.color);
        assert!(f.lights[1].shadow.is_none(), "point lights cast no shadow here");
        // Everything in the scene lands inside the shadow volume of the sun.
        let m = sun.shadow.unwrap();
        for p in [Vec3::new(10.0, -1.0, 10.0), Vec3::new(-10.0, -1.0, -10.0), Vec3::new(0.0, 1.0, 0.0)] {
            let c = m.project_point3(p);
            assert!(c.x.abs() <= 1.0 + 1e-4 && c.y.abs() <= 1.0 + 1e-4 && (0.0..=1.0).contains(&c.z), "{p:?} -> {c:?}");
        }
    }

    #[test]
    fn extra_lights_are_dropped_with_a_count() {
        let (mut s, cam) = scene_with_camera();
        for _ in 0..11 {
            s.add_point_light([1.0; 3], 1.0, 5.0).unwrap();
        }
        let f = s.build_frame(Some(cam), 1.0).unwrap();
        assert_eq!((f.stats.lights, f.stats.lights_dropped), (MAX_LIGHTS, 3));
    }

    #[test]
    fn with_no_camera_the_scene_is_framed() {
        let mut s = Scene3D::new(Limits::default());
        let b = s.add_box([4.0, 4.0, 4.0]).unwrap();
        s.set_position(b, [100.0, 50.0, -20.0]).unwrap();
        let f = s.build_frame(None, 1.5).unwrap();
        assert_eq!(f.camera_node, None);
        assert_eq!(f.stats.instances_drawn, 1, "the automatic camera sees the only box");
        let c = f.view_proj.project_point3(Vec3::new(100.0, 50.0, -20.0));
        assert!(c.x.abs() < 1e-3 && c.y.abs() < 1e-3, "and centres it: {c:?}");
        // An empty scene still gives a frame.
        let mut e = Scene3D::new(Limits::default());
        assert!(e.build_frame(None, 1.0).is_ok());
    }

    #[test]
    fn a_camera_must_be_a_camera() {
        let mut s = Scene3D::new(Limits::default());
        let g = s.add_group().unwrap();
        assert!(s.build_frame(Some(g), 1.0).is_err());
        let released = s.add_group().unwrap();
        s.remove(released).unwrap();
        assert!(matches!(s.build_frame(Some(released), 1.0), Err(GraphicsError::ReleasedHandle(_))));
    }

    #[test]
    fn an_orthographic_camera_culls_by_its_box() {
        let mut s = Scene3D::new(Limits::default());
        let cam = s.add_orthographic_camera(10.0, 0.1, 50.0).unwrap();
        s.set_position(cam, [0.0, 0.0, 20.0]).unwrap();
        let inside = s.add_box([1.0; 3]).unwrap();
        s.set_position(inside, [4.0, 0.0, 0.0]).unwrap();
        let outside = s.add_box([1.0; 3]).unwrap();
        s.set_position(outside, [9.0, 0.0, 0.0]).unwrap();
        let f = s.build_frame(Some(cam), 1.0).unwrap();
        let drawn: Vec<Handle> = f.draws.iter().filter(|d| !d.visible.is_empty()).map(|d| d.node).collect();
        assert_eq!(drawn, vec![inside]);
    }
}
