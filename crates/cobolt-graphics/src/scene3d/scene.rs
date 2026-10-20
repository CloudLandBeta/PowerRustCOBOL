// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The scene graph: nodes, materials, lights, cameras - the part of Scene3D that
//! needs no GPU (and so runs in every test).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::anim::{Animation, Skin};
use super::math::{look_rotation, orthographic, perspective, Mat4, Quat, Vec3};
use super::mesh::MeshData;
use super::primitives;
use crate::color::Rgba;
use crate::error::{GraphicsError, Result};
use crate::handles::{Handle, HandleTable};
use crate::limits::Limits;
use crate::pixels::PixelBuffer;

/// The most instances one node may carry.
pub const MAX_INSTANCES: usize = 1_000_000;

/// How a material's alpha is used.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AlphaMode {
    /// Ignored: the surface is solid. COBOL code `0`.
    Opaque,
    /// Pixels with alpha below the cutoff are dropped. COBOL code `1`.
    Mask,
    /// Blended with what is behind. COBOL code `2`.
    Blend,
}

impl AlphaMode {
    /// From the number a COBOL program passes.
    pub fn from_code(code: i64) -> Option<Self> {
        match code {
            0 => Some(Self::Opaque),
            1 => Some(Self::Mask),
            2 => Some(Self::Blend),
            _ => None,
        }
    }
}

/// Where a texture goes on a material.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextureSlot {
    /// The colour (sRGB). COBOL code `0`.
    BaseColor,
    /// Roughness in green, metalness in blue, occlusion in red (linear). COBOL code `1`.
    MetallicRoughness,
    /// Light the surface gives off (sRGB). COBOL code `2`.
    Emissive,
    /// A tangent-space normal map (linear). COBOL code `3`.
    Normal,
}

impl TextureSlot {
    /// From the number a COBOL program passes.
    pub fn from_code(code: i64) -> Option<Self> {
        match code {
            0 => Some(Self::BaseColor),
            1 => Some(Self::MetallicRoughness),
            2 => Some(Self::Emissive),
            3 => Some(Self::Normal),
            _ => None,
        }
    }
}

/// A physically based surface of the metallic-roughness kind.
#[derive(Clone, Debug)]
pub struct Material {
    /// Colour and opacity, linear, each 0 to 1.
    pub base_color: [f32; 4],
    /// 0 for a dielectric (plastic, wood), 1 for a metal.
    pub metallic: f32,
    /// 0 mirror-smooth to 1 fully rough.
    pub roughness: f32,
    /// Light the surface gives off, linear.
    pub emissive: [f32; 3],
    /// Multiplies the colour.
    pub base_color_texture: Option<Arc<PixelBuffer>>,
    /// Multiplies roughness (green) and metalness (blue); red is the occlusion.
    pub metallic_roughness_texture: Option<Arc<PixelBuffer>>,
    /// Adds to the emissive colour.
    pub emissive_texture: Option<Arc<PixelBuffer>>,
    /// Bends the normal.
    pub normal_texture: Option<Arc<PixelBuffer>>,
    /// How strongly the normal map bends the normal (1 = as authored).
    pub normal_scale: f32,
    /// How alpha is used.
    pub alpha_mode: AlphaMode,
    /// For [`AlphaMode::Mask`].
    pub alpha_cutoff: f32,
    /// Draw both sides of every triangle.
    pub double_sided: bool,
    /// Ignore the lights: draw the colour as it is (glTF `KHR_materials_unlit`).
    pub unlit: bool,
}

impl Default for Material {
    fn default() -> Self {
        Material {
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive: [0.0; 3],
            base_color_texture: None,
            metallic_roughness_texture: None,
            emissive_texture: None,
            normal_texture: None,
            normal_scale: 1.0,
            alpha_mode: AlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
            unlit: false,
        }
    }
}

impl Material {
    /// The texture on a slot, if any.
    pub fn texture(&self, slot: TextureSlot) -> Option<&Arc<PixelBuffer>> {
        match slot {
            TextureSlot::BaseColor => self.base_color_texture.as_ref(),
            TextureSlot::MetallicRoughness => self.metallic_roughness_texture.as_ref(),
            TextureSlot::Emissive => self.emissive_texture.as_ref(),
            TextureSlot::Normal => self.normal_texture.as_ref(),
        }
    }
}

/// What kind of light a light node is.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum LightKind {
    /// Parallel rays along the node's -Z axis (the sun).
    Directional,
    /// All directions from the node's position.
    Point,
    /// A cone along the node's -Z axis; angles in radians.
    Spot {
        /// Full brightness inside this angle from the axis.
        inner: f32,
        /// No light beyond this angle.
        outer: f32,
    },
}

/// A light, placed by the node it belongs to.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Light {
    /// Which kind.
    pub kind: LightKind,
    /// Colour, linear, 0 to 1.
    pub color: [f32; 3],
    /// Brightness: lux for a directional light, candela for the others (the shader treats both as a multiplier).
    pub intensity: f32,
    /// Distance at which a point or spot light fades to nothing (0 = never).
    pub range: f32,
    /// Whether the light casts shadows (directional and spot lights only).
    pub cast_shadow: bool,
}

/// A camera, placed by the node it belongs to (which looks down -Z).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Camera {
    /// Perspective: vertical field of view in radians.
    Perspective {
        /// Vertical field of view.
        fov_y: f32,
        /// Near plane distance.
        near: f32,
        /// Far plane distance.
        far: f32,
    },
    /// Orthographic: the height of the view in world units.
    Orthographic {
        /// Height of the visible region.
        height: f32,
        /// Near plane distance.
        near: f32,
        /// Far plane distance.
        far: f32,
    },
}

impl Camera {
    /// The projection matrix for a viewport of this aspect ratio (width / height), depth 0 to 1.
    pub fn projection(&self, aspect: f32) -> Mat4 {
        let aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 1.0 };
        match *self {
            Camera::Perspective { fov_y, near, far } => perspective(fov_y, aspect, near, far),
            Camera::Orthographic { height, near, far } => {
                let h = height * 0.5;
                orthographic(-h * aspect, h * aspect, -h, h, near, far)
            }
        }
    }
}

/// What the picture is cleared to before the scene is drawn.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Background {
    /// One colour (its alpha too: a transparent background gives a picture with transparent pixels).
    Color(Rgba),
    /// A vertical blend from the top colour to the bottom one.
    Gradient {
        /// At the top edge.
        top: Rgba,
        /// At the bottom edge.
        bottom: Rgba,
    },
}

/// Distance fog.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Fog {
    /// What distant things fade to.
    pub color: [f32; 3],
    /// How the fog thickens.
    pub mode: FogMode,
}

/// How fog thickens with distance.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FogMode {
    /// None at `near`, total at `far`.
    Linear {
        /// Distance where fog starts.
        near: f32,
        /// Distance where it is total.
        far: f32,
    },
    /// `1 - exp(-density * distance)`.
    Exponential {
        /// Thickness.
        density: f32,
    },
}

/// What a node holds.
#[derive(Clone, Debug)]
pub enum Content {
    /// Only a place in the hierarchy (a group).
    Empty,
    /// A mesh.
    Mesh(MeshRef),
    /// A light.
    Light(Light),
    /// A camera.
    Camera(Camera),
}

/// A mesh drawn by a node.
#[derive(Clone, Debug)]
pub struct MeshRef {
    /// The mesh in the scene's mesh table.
    pub mesh: Handle,
    /// The material, or `None` for the default (white, rough).
    pub material: Option<Handle>,
    /// Extra transforms: the node draws one copy per entry, each placed by its matrix
    /// inside the node's own. Empty draws one copy.
    pub instances: Vec<Mat4>,
}

/// One node of the hierarchy.
#[derive(Clone, Debug)]
pub struct Node {
    pub(super) name: String,
    pub(super) parent: Option<Handle>,
    pub(super) children: Vec<Handle>,
    pub(super) translation: Vec3,
    pub(super) rotation: Quat,
    pub(super) scale: Vec3,
    pub(super) visible: bool,
    pub(super) content: Content,
    pub(super) skin: Option<Handle>,
    pub(super) world: Mat4,
}

impl Node {
    fn new(content: Content) -> Node {
        Node {
            name: String::new(),
            parent: None,
            children: Vec::new(),
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            visible: true,
            content,
            skin: None,
            world: Mat4::IDENTITY,
        }
    }

    pub(super) fn local(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

pub(super) struct MeshEntry {
    pub data: Arc<MeshData>,
    /// Nodes that draw it; the mesh is released with the last one.
    pub users: u32,
}

static NEXT_SCENE_ID: AtomicU64 = AtomicU64::new(1);

/// A scene: a hierarchy of nodes with meshes, lights and cameras, plus the
/// materials, animations and skins they use. See the [module notes](super).
pub struct Scene3D {
    pub(super) id: u64,
    pub(super) limits: Limits,
    pub(super) nodes: HandleTable<Node>,
    pub(super) meshes: HandleTable<MeshEntry>,
    pub(super) materials: HandleTable<Material>,
    pub(super) skins: HandleTable<Skin>,
    pub(super) animations: HandleTable<Animation>,
    pub(super) ambient: [f32; 3],
    pub(super) background: Background,
    pub(super) fog: Option<Fog>,
    pub(super) world_dirty: bool,
    pub(super) triangles: u64,
    pub(super) memory: u64,
    /// Changes whenever anything a renderer caches may have changed.
    pub(super) revision: u64,
}

impl std::fmt::Debug for Scene3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Scene3D({} node(s), {} mesh(es), {} triangles)", self.nodes.len(), self.meshes.len(), self.triangles)
    }
}

fn finite(what: &str, v: &[f32]) -> Result<()> {
    if v.iter().all(|x| x.is_finite()) {
        Ok(())
    } else {
        Err(GraphicsError::invalid(format!("{what} must be finite numbers")))
    }
}

fn unit(what: &str, v: &[f32]) -> Result<()> {
    if v.iter().all(|x| x.is_finite() && *x >= 0.0 && *x <= 1.0) {
        Ok(())
    } else {
        Err(GraphicsError::invalid(format!("{what} must be numbers from 0 to 1")))
    }
}

impl Scene3D {
    /// An empty scene: white ambient light at 0.1, a dark blue-grey background, no fog.
    pub fn new(limits: Limits) -> Scene3D {
        Scene3D {
            id: NEXT_SCENE_ID.fetch_add(1, Ordering::Relaxed),
            limits,
            nodes: HandleTable::new("node"),
            meshes: HandleTable::new("mesh"),
            materials: HandleTable::new("material"),
            skins: HandleTable::new("skin"),
            animations: HandleTable::new("animation"),
            ambient: [0.1, 0.1, 0.1],
            background: Background::Color(Rgba::rgb(26, 28, 36)),
            fog: None,
            world_dirty: false,
            triangles: 0,
            memory: 0,
            revision: 0,
        }
    }

    /// A number that identifies this scene to a renderer's caches.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Changes whenever a mesh, material or texture was added, replaced or released.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The limits in force.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// How many nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// How many triangles the meshes hold (instances not counted).
    pub fn triangle_count(&self) -> u64 {
        self.triangles
    }

    /// The memory the scene's meshes hold.
    pub fn memory_bytes(&self) -> u64 {
        self.memory
    }

    // ---- creating nodes ----------------------------------------------------

    pub(super) fn add_node(&mut self, node: Node) -> Result<Handle> {
        self.limits.check_nodes(self.nodes.len() as u64 + 1)?;
        let h = self.nodes.insert(node)?;
        self.world_dirty = true;
        Ok(h)
    }

    pub(super) fn add_mesh(&mut self, data: MeshData) -> Result<Handle> {
        self.limits.check_triangles(self.triangles + data.triangle_count() as u64)?;
        self.limits.check_total(self.memory + data.memory_bytes())?;
        let (tris, bytes) = (data.triangle_count() as u64, data.memory_bytes());
        let h = self.meshes.insert(MeshEntry { data: Arc::new(data), users: 0 })?;
        self.triangles += tris;
        self.memory += bytes;
        self.revision += 1;
        Ok(h)
    }

    /// Put a mesh on a new node (the mesh is released with its last node).
    pub(super) fn add_mesh_node(&mut self, mesh: Handle, material: Option<Handle>, name: &str) -> Result<Handle> {
        self.meshes.get(mesh)?;
        if let Some(m) = material {
            self.materials.get(m)?;
        }
        let mut node = Node::new(Content::Mesh(MeshRef { mesh, material, instances: Vec::new() }));
        node.name = name.to_string();
        let h = self.add_node(node)?;
        self.meshes.get_mut(mesh)?.users += 1;
        Ok(h)
    }

    fn primitive(&mut self, data: Result<MeshData>, name: &str) -> Result<Handle> {
        let mesh = self.add_mesh(data?)?;
        match self.add_mesh_node(mesh, None, name) {
            Ok(h) => Ok(h),
            Err(e) => {
                self.release_mesh(mesh);
                Err(e)
            }
        }
    }

    pub(super) fn release_mesh(&mut self, mesh: Handle) {
        if let Ok(e) = self.meshes.remove(mesh) {
            self.triangles = self.triangles.saturating_sub(e.data.triangle_count() as u64);
            self.memory = self.memory.saturating_sub(e.data.memory_bytes());
            self.revision += 1;
        }
    }

    /// `AddGroup`: an empty node to hang others from.
    pub fn add_group(&mut self) -> Result<Handle> {
        self.add_node(Node::new(Content::Empty))
    }

    /// `AddBox`: a box of this size (width, height, depth).
    pub fn add_box(&mut self, size: [f32; 3]) -> Result<Handle> {
        self.primitive(primitives::cuboid(size), "box")
    }

    /// `AddSphere`.
    pub fn add_sphere(&mut self, radius: f32, segments: u32) -> Result<Handle> {
        self.primitive(primitives::sphere(radius, segments), "sphere")
    }

    /// `AddPlane`: a flat rectangle in the XZ plane facing up.
    pub fn add_plane(&mut self, width: f32, depth: f32) -> Result<Handle> {
        self.primitive(primitives::plane(width, depth), "plane")
    }

    /// `AddCylinder`: along Y.
    pub fn add_cylinder(&mut self, radius: f32, height: f32, segments: u32) -> Result<Handle> {
        self.primitive(primitives::cylinder(radius, height, segments), "cylinder")
    }

    /// `AddCone`: along Y, apex up.
    pub fn add_cone(&mut self, radius: f32, height: f32, segments: u32) -> Result<Handle> {
        self.primitive(primitives::cone(radius, height, segments), "cone")
    }

    /// `AddTorus`: in the XZ plane.
    pub fn add_torus(&mut self, radius: f32, tube: f32, radial_segments: u32, tubular_segments: u32) -> Result<Handle> {
        self.primitive(primitives::torus(radius, tube, radial_segments, tubular_segments), "torus")
    }

    /// `AddMesh`: a mesh of your own: flat arrays of positions (x, y, z), normals
    /// (empty to compute them), texture coordinates (u, v; empty for none) and triangle indices.
    pub fn add_mesh_from_arrays(&mut self, positions: &[f32], normals: &[f32], uvs: &[f32], indices: &[u32]) -> Result<Handle> {
        if positions.len() % 3 != 0 || normals.len() % 3 != 0 || uvs.len() % 2 != 0 {
            return Err(GraphicsError::invalid("positions and normals come in threes, texture coordinates in twos"));
        }
        self.limits.check_triangles(self.triangles + (indices.len() / 3) as u64)?;
        let tri3 = |v: &[f32]| v.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect::<Vec<_>>();
        let data = MeshData::new(tri3(positions), tri3(normals), uvs.chunks_exact(2).map(|c| [c[0], c[1]]).collect(), indices.to_vec(), vec![], vec![]);
        self.primitive(data, "mesh")
    }

    /// `AddDirectionalLight`: returns the light's node; aim it with `LookAt` or `SetRotation`.
    pub fn add_directional_light(&mut self, color: [f32; 3], intensity: f32, cast_shadow: bool) -> Result<Handle> {
        self.add_light(Light { kind: LightKind::Directional, color, intensity, range: 0.0, cast_shadow })
    }

    /// `AddPointLight`.
    pub fn add_point_light(&mut self, color: [f32; 3], intensity: f32, range: f32) -> Result<Handle> {
        self.add_light(Light { kind: LightKind::Point, color, intensity, range, cast_shadow: false })
    }

    /// `AddSpotLight`: cone angles in degrees.
    pub fn add_spot_light(&mut self, color: [f32; 3], intensity: f32, range: f32, inner_degrees: f32, outer_degrees: f32, cast_shadow: bool) -> Result<Handle> {
        finite("the cone angles", &[inner_degrees, outer_degrees])?;
        if !(0.0 <= inner_degrees && inner_degrees < outer_degrees && outer_degrees <= 89.0) {
            return Err(GraphicsError::invalid("a spot light's inner angle must be from 0 up to less than the outer angle, which is at most 89 degrees"));
        }
        self.add_light(Light { kind: LightKind::Spot { inner: inner_degrees.to_radians(), outer: outer_degrees.to_radians() }, color, intensity, range, cast_shadow })
    }

    fn add_light(&mut self, light: Light) -> Result<Handle> {
        unit("a light colour", &light.color)?;
        finite("a light's intensity and range", &[light.intensity, light.range])?;
        if light.intensity < 0.0 || light.range < 0.0 {
            return Err(GraphicsError::invalid("a light's intensity and range cannot be negative"));
        }
        let mut n = Node::new(Content::Light(light));
        n.name = match light.kind {
            LightKind::Directional => "directional light",
            LightKind::Point => "point light",
            LightKind::Spot { .. } => "spot light",
        }
        .to_string();
        self.add_node(n)
    }

    /// `AddCamera`: a perspective camera; the field of view is vertical, in degrees.
    pub fn add_perspective_camera(&mut self, fov_degrees: f32, near: f32, far: f32) -> Result<Handle> {
        finite("a camera's field of view and planes", &[fov_degrees, near, far])?;
        if !(1.0..=170.0).contains(&fov_degrees) || near <= 0.0 || far <= near {
            return Err(GraphicsError::invalid("a perspective camera needs a field of view from 1 to 170 degrees and 0 < near < far"));
        }
        let mut n = Node::new(Content::Camera(Camera::Perspective { fov_y: fov_degrees.to_radians(), near, far }));
        n.name = "camera".into();
        self.add_node(n)
    }

    /// `AddOrthographicCamera`: `height` is how much of the world fits vertically.
    pub fn add_orthographic_camera(&mut self, height: f32, near: f32, far: f32) -> Result<Handle> {
        finite("a camera's height and planes", &[height, near, far])?;
        if height <= 0.0 || far <= near {
            return Err(GraphicsError::invalid("an orthographic camera needs a height above 0 and near < far"));
        }
        let mut n = Node::new(Content::Camera(Camera::Orthographic { height, near, far }));
        n.name = "camera".into();
        self.add_node(n)
    }

    // ---- the hierarchy ---------------------------------------------------

    /// `SetParent`: move `node` under `parent` (`None` makes it a root). Its local transform is kept.
    pub fn set_parent(&mut self, node: Handle, parent: Option<Handle>) -> Result<()> {
        self.nodes.get(node)?;
        if let Some(p) = parent {
            self.nodes.get(p)?;
            // A node cannot become its own ancestor.
            let mut cur = Some(p);
            while let Some(c) = cur {
                if c == node {
                    return Err(GraphicsError::invalid("a node cannot be put under itself or one of its own children"));
                }
                cur = self.nodes.get(c)?.parent;
            }
        }
        if let Some(old) = self.nodes.get(node)?.parent {
            if let Ok(o) = self.nodes.get_mut(old) {
                o.children.retain(|c| *c != node);
            }
        }
        if let Some(p) = parent {
            self.nodes.get_mut(p)?.children.push(node);
        }
        self.nodes.get_mut(node)?.parent = parent;
        self.world_dirty = true;
        Ok(())
    }

    /// Put a node that was just created (so it has no children and no parent) under `parent`.
    /// Unlike [`Scene3D::set_parent`] it needs no walk up the ancestors to rule out a cycle: a
    /// brand-new node cannot be anyone's ancestor. The model loader builds a hierarchy of tens of
    /// thousands of nodes this way in linear time.
    pub(super) fn attach_new_child(&mut self, node: Handle, parent: Handle) -> Result<()> {
        self.nodes.get(parent)?;
        {
            let n = self.nodes.get_mut(node)?;
            debug_assert!(n.parent.is_none() && n.children.is_empty());
            n.parent = Some(parent);
        }
        self.nodes.get_mut(parent)?.children.push(node);
        self.world_dirty = true;
        Ok(())
    }

    /// `GetParent`.
    pub fn parent(&self, node: Handle) -> Result<Option<Handle>> {
        Ok(self.nodes.get(node)?.parent)
    }

    /// `GetChildren`.
    pub fn children(&self, node: Handle) -> Result<Vec<Handle>> {
        Ok(self.nodes.get(node)?.children.clone())
    }

    /// `Remove`: the node and everything under it. Meshes nobody draws any more are released too.
    pub fn remove(&mut self, node: Handle) -> Result<()> {
        self.nodes.get(node)?;
        if let Some(p) = self.nodes.get(node)?.parent {
            if let Ok(parent) = self.nodes.get_mut(p) {
                parent.children.retain(|c| *c != node);
            }
        }
        let mut stack = vec![node];
        while let Some(h) = stack.pop() {
            let Ok(n) = self.nodes.remove(h) else { continue };
            stack.extend(n.children.iter().copied());
            if let Content::Mesh(m) = &n.content {
                let gone = match self.meshes.get_mut(m.mesh) {
                    Ok(e) => {
                        e.users = e.users.saturating_sub(1);
                        e.users == 0
                    }
                    Err(_) => false,
                };
                if gone {
                    self.release_mesh(m.mesh);
                }
            }
        }
        self.world_dirty = true;
        Ok(())
    }

    /// `SetName` / `FindNode`: a name for a node (names need not be unique; `find_node` returns the first in handle order).
    pub fn set_name(&mut self, node: Handle, name: &str) -> Result<()> {
        self.limits.check_text(name.len())?;
        self.nodes.get_mut(node)?.name = name.to_string();
        Ok(())
    }

    /// The name of a node.
    pub fn name(&self, node: Handle) -> Result<&str> {
        Ok(&self.nodes.get(node)?.name)
    }

    /// `FindNode`: the first node with this name, if any.
    pub fn find_node(&self, name: &str) -> Option<Handle> {
        self.nodes.iter().find(|(_, n)| n.name == name).map(|(h, _)| h)
    }

    /// `SetVisible`: a hidden node hides everything under it.
    pub fn set_visible(&mut self, node: Handle, visible: bool) -> Result<()> {
        self.nodes.get_mut(node)?.visible = visible;
        Ok(())
    }

    /// Whether the node itself is visible.
    pub fn is_visible(&self, node: Handle) -> Result<bool> {
        Ok(self.nodes.get(node)?.visible)
    }

    // ---- transforms ------------------------------------------------------

    /// `SetPosition` (relative to the parent).
    pub fn set_position(&mut self, node: Handle, position: [f32; 3]) -> Result<()> {
        finite("a position", &position)?;
        self.nodes.get_mut(node)?.translation = Vec3::from_array(position);
        self.world_dirty = true;
        Ok(())
    }

    /// `SetRotation`: Euler angles in degrees, applied as yaw (Y), then pitch (X), then roll (Z).
    pub fn set_rotation(&mut self, node: Handle, degrees: [f32; 3]) -> Result<()> {
        finite("a rotation", &degrees)?;
        let q = Quat::from_euler(glam::EulerRot::YXZ, degrees[1].to_radians(), degrees[0].to_radians(), degrees[2].to_radians());
        self.nodes.get_mut(node)?.rotation = q.normalize();
        self.world_dirty = true;
        Ok(())
    }

    /// `SetRotationQuaternion`: a quaternion `[x, y, z, w]`.
    pub fn set_rotation_quaternion(&mut self, node: Handle, q: [f32; 4]) -> Result<()> {
        finite("a rotation", &q)?;
        let q = Quat::from_xyzw(q[0], q[1], q[2], q[3]);
        if q.length_squared() < 1.0e-12 {
            return Err(GraphicsError::invalid("a rotation quaternion cannot be all zero"));
        }
        self.nodes.get_mut(node)?.rotation = q.normalize();
        self.world_dirty = true;
        Ok(())
    }

    /// `SetScale`: zero on any axis is refused (it cannot be undone and shows nothing).
    pub fn set_scale(&mut self, node: Handle, scale: [f32; 3]) -> Result<()> {
        finite("a scale", &scale)?;
        if scale.iter().any(|s| *s == 0.0) {
            return Err(GraphicsError::invalid("a scale of zero on an axis flattens the node to nothing; use SetVisible to hide it"));
        }
        self.nodes.get_mut(node)?.scale = Vec3::from_array(scale);
        self.world_dirty = true;
        Ok(())
    }

    /// `LookAt`: turn the node (in its parent's space, using its current position) so that its -Z axis points at `target`.
    pub fn look_at(&mut self, node: Handle, target: [f32; 3], up: [f32; 3]) -> Result<()> {
        finite("a target", &target)?;
        finite("an up vector", &up)?;
        let n = self.nodes.get_mut(node)?;
        n.rotation = look_rotation(n.translation, Vec3::from_array(target), Vec3::from_array(up));
        self.world_dirty = true;
        Ok(())
    }

    /// `GetPosition` (relative to the parent).
    pub fn position(&self, node: Handle) -> Result<[f32; 3]> {
        Ok(self.nodes.get(node)?.translation.to_array())
    }

    /// `GetRotation`: Euler degrees as `SetRotation` takes them.
    pub fn rotation(&self, node: Handle) -> Result<[f32; 3]> {
        let (y, x, z) = self.nodes.get(node)?.rotation.to_euler(glam::EulerRot::YXZ);
        Ok([x.to_degrees(), y.to_degrees(), z.to_degrees()])
    }

    /// `GetRotationQuaternion`.
    pub fn rotation_quaternion(&self, node: Handle) -> Result<[f32; 4]> {
        Ok(self.nodes.get(node)?.rotation.to_array())
    }

    /// `GetScale`.
    pub fn scale(&self, node: Handle) -> Result<[f32; 3]> {
        Ok(self.nodes.get(node)?.scale.to_array())
    }

    /// Bring every node's world matrix up to date (done by whatever needs one; public so a program can ask).
    pub fn update_transforms(&mut self) {
        if !self.world_dirty {
            return;
        }
        let roots: Vec<Handle> = self.nodes.iter().filter(|(_, n)| n.parent.is_none()).map(|(h, _)| h).collect();
        let mut stack: Vec<(Handle, Mat4)> = roots.into_iter().rev().map(|h| (h, Mat4::IDENTITY)).collect();
        while let Some((h, parent)) = stack.pop() {
            let Ok(n) = self.nodes.get_mut(h) else { continue };
            n.world = parent * n.local();
            let w = n.world;
            for c in n.children.iter().rev() {
                stack.push((*c, w));
            }
        }
        self.world_dirty = false;
    }

    /// `GetWorldPosition`: where the node is in the scene (not relative to its parent).
    pub fn world_position(&mut self, node: Handle) -> Result<[f32; 3]> {
        self.nodes.get(node)?;
        self.update_transforms();
        Ok(self.nodes.get(node)?.world.w_axis.truncate().to_array())
    }

    /// The node's world matrix (column-major, for a renderer or a test).
    pub fn world_matrix(&mut self, node: Handle) -> Result<Mat4> {
        self.nodes.get(node)?;
        self.update_transforms();
        Ok(self.nodes.get(node)?.world)
    }

    // ---- materials -------------------------------------------------------

    /// `CreateMaterial`: a new material (white, rough 0.5, not metallic).
    pub fn create_material(&mut self) -> Result<Handle> {
        let h = self.materials.insert(Material::default())?;
        self.revision += 1;
        Ok(h)
    }

    /// The material of a handle.
    pub fn material(&self, material: Handle) -> Result<&Material> {
        self.materials.get(material)
    }

    /// `ReleaseMaterial`: nodes that used it fall back to the default.
    pub fn release_material(&mut self, material: Handle) -> Result<()> {
        self.materials.remove(material)?;
        for (_, n) in self.nodes.iter_mut() {
            if let Content::Mesh(m) = &mut n.content {
                if m.material == Some(material) {
                    m.material = None;
                }
            }
        }
        self.revision += 1;
        Ok(())
    }

    /// `SetMaterial`: the node draws with this material (`None` for the default).
    pub fn set_material(&mut self, node: Handle, material: Option<Handle>) -> Result<()> {
        if let Some(m) = material {
            self.materials.get(m)?;
        }
        match &mut self.nodes.get_mut(node)?.content {
            Content::Mesh(m) => {
                m.material = material;
                Ok(())
            }
            _ => Err(GraphicsError::invalid("only a node that draws a mesh has a material")),
        }
    }

    /// `GetMaterial`: the material a mesh node uses, creating (and assigning) one if it had none.
    pub fn material_of(&mut self, node: Handle) -> Result<Handle> {
        let existing = match &self.nodes.get(node)?.content {
            Content::Mesh(m) => m.material,
            _ => return Err(GraphicsError::invalid("only a node that draws a mesh has a material")),
        };
        if let Some(m) = existing {
            return Ok(m);
        }
        let m = self.create_material()?;
        self.set_material(node, Some(m))?;
        Ok(m)
    }

    fn mat_mut(&mut self, material: Handle) -> Result<&mut Material> {
        self.revision += 1;
        self.materials.get_mut(material)
    }

    /// `SetColor`: base colour and opacity, each 0 to 1.
    pub fn set_color(&mut self, material: Handle, rgba: [f32; 4]) -> Result<()> {
        unit("a colour", &rgba)?;
        self.mat_mut(material)?.base_color = rgba;
        Ok(())
    }

    /// `SetMetallic`.
    pub fn set_metallic(&mut self, material: Handle, v: f32) -> Result<()> {
        unit("metallic", &[v])?;
        self.mat_mut(material)?.metallic = v;
        Ok(())
    }

    /// `SetRoughness`.
    pub fn set_roughness(&mut self, material: Handle, v: f32) -> Result<()> {
        unit("roughness", &[v])?;
        self.mat_mut(material)?.roughness = v;
        Ok(())
    }

    /// `SetEmissive`: the colour the surface gives off, each 0 to 1... or brighter (it is a linear light).
    pub fn set_emissive(&mut self, material: Handle, rgb: [f32; 3]) -> Result<()> {
        finite("an emissive colour", &rgb)?;
        if rgb.iter().any(|v| *v < 0.0 || *v > 1000.0) {
            return Err(GraphicsError::invalid("an emissive colour is from 0 to 1000"));
        }
        self.mat_mut(material)?.emissive = rgb;
        Ok(())
    }

    /// `SetTexture`: an image on one of the material's slots (`None` clears it). The image is
    /// shared, not copied; changing the ImageBuffer afterwards does not change the material
    /// until it is set again.
    pub fn set_texture(&mut self, material: Handle, slot: TextureSlot, image: Option<Arc<PixelBuffer>>) -> Result<()> {
        if let Some(img) = &image {
            self.limits.check_texture(img.width(), img.height())?;
        }
        let m = self.mat_mut(material)?;
        match slot {
            TextureSlot::BaseColor => m.base_color_texture = image,
            TextureSlot::MetallicRoughness => m.metallic_roughness_texture = image,
            TextureSlot::Emissive => m.emissive_texture = image,
            TextureSlot::Normal => m.normal_texture = image,
        }
        Ok(())
    }

    /// `SetAlphaMode`.
    pub fn set_alpha_mode(&mut self, material: Handle, mode: AlphaMode, cutoff: f32) -> Result<()> {
        unit("the alpha cutoff", &[cutoff])?;
        let m = self.mat_mut(material)?;
        m.alpha_mode = mode;
        m.alpha_cutoff = cutoff;
        Ok(())
    }

    /// `SetDoubleSided`.
    pub fn set_double_sided(&mut self, material: Handle, v: bool) -> Result<()> {
        self.mat_mut(material)?.double_sided = v;
        Ok(())
    }

    /// `SetUnlit`.
    pub fn set_unlit(&mut self, material: Handle, v: bool) -> Result<()> {
        self.mat_mut(material)?.unlit = v;
        Ok(())
    }

    // ---- lights, camera, background, fog ----------------------------------

    /// `SetLight`: change a light node's colour, intensity and range.
    pub fn set_light(&mut self, node: Handle, color: [f32; 3], intensity: f32, range: f32) -> Result<()> {
        unit("a light colour", &color)?;
        finite("a light's intensity and range", &[intensity, range])?;
        if intensity < 0.0 || range < 0.0 {
            return Err(GraphicsError::invalid("a light's intensity and range cannot be negative"));
        }
        match &mut self.nodes.get_mut(node)?.content {
            Content::Light(l) => {
                l.color = color;
                l.intensity = intensity;
                l.range = range;
                Ok(())
            }
            _ => Err(GraphicsError::invalid("that node is not a light")),
        }
    }

    /// `SetAmbient`: the light every surface gets from everywhere.
    pub fn set_ambient(&mut self, color: [f32; 3], intensity: f32) -> Result<()> {
        unit("an ambient colour", &color)?;
        finite("an ambient intensity", &[intensity])?;
        if intensity < 0.0 {
            return Err(GraphicsError::invalid("an ambient intensity cannot be negative"));
        }
        self.ambient = color.map(|c| c * intensity);
        Ok(())
    }

    /// `SetBackground`: a colour (alpha 0 gives a picture with transparent pixels).
    pub fn set_background(&mut self, color: Rgba) {
        self.background = Background::Color(color);
    }

    /// `SetBackgroundGradient`.
    pub fn set_background_gradient(&mut self, top: Rgba, bottom: Rgba) {
        self.background = Background::Gradient { top, bottom };
    }

    /// `SetFog`: `None` turns it off.
    pub fn set_fog(&mut self, fog: Option<Fog>) -> Result<()> {
        if let Some(f) = &fog {
            unit("a fog colour", &f.color)?;
            match f.mode {
                FogMode::Linear { near, far } => {
                    finite("the fog distances", &[near, far])?;
                    if near < 0.0 || far <= near {
                        return Err(GraphicsError::invalid("linear fog needs 0 <= near < far"));
                    }
                }
                FogMode::Exponential { density } => {
                    finite("the fog density", &[density])?;
                    if density < 0.0 {
                        return Err(GraphicsError::invalid("fog density cannot be negative"));
                    }
                }
            }
        }
        self.fog = fog;
        Ok(())
    }

    /// The camera of a camera node.
    pub fn camera(&self, node: Handle) -> Result<Camera> {
        match &self.nodes.get(node)?.content {
            Content::Camera(c) => Ok(*c),
            _ => Err(GraphicsError::invalid("that node is not a camera")),
        }
    }

    // ---- instancing --------------------------------------------------------

    /// `SetInstances`: draw the node's mesh once per matrix (16 numbers each, column-major: the
    /// usual 4x4 with the translation in the last column). An empty list goes back to one copy.
    pub fn set_instances(&mut self, node: Handle, matrices: &[f32]) -> Result<()> {
        if matrices.len() % 16 != 0 {
            return Err(GraphicsError::invalid("an instance matrix is sixteen numbers"));
        }
        let count = matrices.len() / 16;
        if count > MAX_INSTANCES {
            return Err(GraphicsError::LimitExceeded { what: "instances of one node", requested: count as u64, limit: MAX_INSTANCES as u64, setting: "(fixed)" });
        }
        if matrices.iter().any(|v| !v.is_finite()) {
            return Err(GraphicsError::invalid("an instance matrix must hold finite numbers"));
        }
        let tris = match &self.nodes.get(node)?.content {
            Content::Mesh(m) => self.meshes.get(m.mesh)?.data.triangle_count() as u64,
            _ => return Err(GraphicsError::invalid("only a node that draws a mesh can be instanced")),
        };
        self.limits.check_triangles(tris.saturating_mul(count.max(1) as u64))?;
        let list: Vec<Mat4> = matrices.chunks_exact(16).map(|c| Mat4::from_cols_slice(c)).collect();
        if let Content::Mesh(m) = &mut self.nodes.get_mut(node)?.content {
            m.instances = list;
        }
        Ok(())
    }

    /// How many copies a node draws (1 when it has no instance list).
    pub fn instance_count(&self, node: Handle) -> Result<usize> {
        match &self.nodes.get(node)?.content {
            Content::Mesh(m) => Ok(m.instances.len().max(1)),
            _ => Ok(0),
        }
    }

    // ---- reading ------------------------------------------------------------

    /// The bounding box of everything drawn, in world space.
    pub fn bounds(&mut self) -> BoundsBox {
        self.update_transforms();
        let mut b = super::math::Aabb::EMPTY;
        for (_, n) in self.nodes.iter() {
            if let Content::Mesh(m) = &n.content {
                if let Ok(e) = self.meshes.get(m.mesh) {
                    if m.instances.is_empty() {
                        b = b.union(&e.data.bounds.transformed(&n.world));
                    } else {
                        for i in &m.instances {
                            b = b.union(&e.data.bounds.transformed(&(n.world * *i)));
                        }
                    }
                }
            }
        }
        BoundsBox { min: b.min.to_array(), max: b.max.to_array(), valid: b.is_valid() }
    }

    /// What kind of thing a node is.
    pub fn kind_of(&self, node: Handle) -> Result<NodeKind> {
        Ok(match &self.nodes.get(node)?.content {
            Content::Empty => NodeKind::Group,
            Content::Mesh(_) => NodeKind::Mesh,
            Content::Light(_) => NodeKind::Light,
            Content::Camera(_) => NodeKind::Camera,
        })
    }
}

/// The kind of a node.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// A group.
    Group,
    /// Draws a mesh.
    Mesh,
    /// A light.
    Light,
    /// A camera.
    Camera,
}

/// A world-space box as plain numbers.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BoundsBox {
    /// Smallest corner.
    pub min: [f32; 3],
    /// Largest corner.
    pub max: [f32; 3],
    /// False for an empty scene.
    pub valid: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> Scene3D {
        Scene3D::new(Limits::default())
    }

    #[test]
    fn transforms_compose_down_the_hierarchy() {
        let mut s = scene();
        let parent = s.add_group().unwrap();
        let child = s.add_box([1.0, 1.0, 1.0]).unwrap();
        s.set_parent(child, Some(parent)).unwrap();
        s.set_position(parent, [10.0, 0.0, 0.0]).unwrap();
        s.set_position(child, [0.0, 2.0, 0.0]).unwrap();
        assert_eq!(s.world_position(child).unwrap(), [10.0, 2.0, 0.0]);
        s.set_rotation(parent, [0.0, 90.0, 0.0]).unwrap();
        let p = s.world_position(child).unwrap();
        assert!((p[0] - 10.0).abs() < 1e-5 && (p[1] - 2.0).abs() < 1e-5 && p[2].abs() < 1e-5, "{p:?}");
        s.set_scale(parent, [2.0, 2.0, 2.0]).unwrap();
        assert!((s.world_position(child).unwrap()[1] - 4.0).abs() < 1e-5);
        assert_eq!(s.children(parent).unwrap(), vec![child]);
        assert_eq!(s.parent(child).unwrap(), Some(parent));
    }

    #[test]
    fn a_node_cannot_become_its_own_ancestor() {
        let mut s = scene();
        let a = s.add_group().unwrap();
        let b = s.add_group().unwrap();
        s.set_parent(b, Some(a)).unwrap();
        assert!(s.set_parent(a, Some(b)).is_err());
        assert!(s.set_parent(a, Some(a)).is_err());
        s.set_parent(b, None).unwrap();
        s.set_parent(a, Some(b)).unwrap();
    }

    #[test]
    fn removing_a_node_removes_its_subtree_and_frees_its_meshes() {
        let mut s = scene();
        let root = s.add_group().unwrap();
        let a = s.add_sphere(1.0, 16).unwrap();
        let b = s.add_box([1.0; 3]).unwrap();
        s.set_parent(a, Some(root)).unwrap();
        s.set_parent(b, Some(a)).unwrap();
        let other = s.add_box([1.0; 3]).unwrap();
        assert_eq!(s.node_count(), 4);
        let tris_before = s.triangle_count();
        s.remove(root).unwrap();
        assert_eq!(s.node_count(), 1);
        assert!(s.triangle_count() < tris_before, "the sphere's and the first box's triangles went with them");
        assert_eq!(s.triangle_count(), 12, "only `other` is left");
        assert!(matches!(s.position(a), Err(GraphicsError::ReleasedHandle(_))));
        assert!(matches!(s.position(b), Err(GraphicsError::ReleasedHandle(_))));
        assert!(s.position(other).is_ok());
    }

    #[test]
    fn rotation_in_degrees_round_trips_and_look_at_aims_minus_z() {
        let mut s = scene();
        let n = s.add_group().unwrap();
        s.set_rotation(n, [20.0, 30.0, 10.0]).unwrap();
        let r = s.rotation(n).unwrap();
        for (a, b) in r.iter().zip([20.0, 30.0, 10.0]) {
            assert!((a - b).abs() < 1e-3, "{r:?}");
        }
        s.set_position(n, [0.0, 0.0, 5.0]).unwrap();
        s.look_at(n, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
        let m = s.world_matrix(n).unwrap();
        let fwd = m.transform_vector3(Vec3::NEG_Z);
        assert!((fwd - Vec3::NEG_Z).length() < 1e-5);
        s.set_position(n, [5.0, 0.0, 0.0]).unwrap();
        s.look_at(n, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
        let fwd = s.world_matrix(n).unwrap().transform_vector3(Vec3::NEG_Z);
        assert!((fwd - Vec3::NEG_X).length() < 1e-5, "{fwd:?}");
    }

    #[test]
    fn materials_belong_to_nodes_and_fall_back_when_released() {
        let mut s = scene();
        let n = s.add_sphere(1.0, 12).unwrap();
        let m = s.material_of(n).unwrap();
        assert_eq!(s.material_of(n).unwrap(), m, "asking twice gives the same material");
        s.set_color(m, [1.0, 0.0, 0.0, 1.0]).unwrap();
        s.set_metallic(m, 1.0).unwrap();
        s.set_roughness(m, 0.2).unwrap();
        assert_eq!(s.material(m).unwrap().base_color, [1.0, 0.0, 0.0, 1.0]);
        let rev = s.revision();
        s.release_material(m).unwrap();
        assert!(s.revision() > rev);
        assert!(matches!(s.material(m), Err(GraphicsError::ReleasedHandle(_))));
        let m2 = s.material_of(n).unwrap();
        assert_ne!(m2, m, "a fresh material was made for the node");
        let g = s.add_group().unwrap();
        assert!(s.material_of(g).is_err(), "a group has no material");
    }

    #[test]
    fn nonsense_is_refused() {
        let mut s = scene();
        let n = s.add_box([1.0; 3]).unwrap();
        let m = s.create_material().unwrap();
        assert!(s.set_position(n, [f32::NAN, 0.0, 0.0]).is_err());
        assert!(s.set_scale(n, [1.0, 0.0, 1.0]).is_err());
        assert!(s.set_rotation_quaternion(n, [0.0; 4]).is_err());
        assert!(s.set_color(m, [2.0, 0.0, 0.0, 1.0]).is_err());
        assert!(s.set_roughness(m, -0.1).is_err());
        assert!(s.add_perspective_camera(0.0, 0.1, 10.0).is_err());
        assert!(s.add_perspective_camera(60.0, 1.0, 0.5).is_err());
        assert!(s.add_spot_light([1.0; 3], 1.0, 0.0, 30.0, 20.0, false).is_err());
        assert!(s.add_directional_light([2.0, 0.0, 0.0], 1.0, false).is_err());
        assert!(s.set_instances(n, &[0.0; 15]).is_err());
        assert!(s.set_fog(Some(Fog { color: [0.5; 3], mode: FogMode::Linear { near: 10.0, far: 5.0 } })).is_err());
        assert!(s.set_light(n, [1.0; 3], 1.0, 0.0).is_err(), "a box is not a light");
        let g = s.add_group().unwrap();
        assert!(s.set_instances(g, &[]).is_err());
    }

    #[test]
    fn limits_stop_a_scene_that_grows_too_big() {
        let mut s = Scene3D::new(Limits { max_nodes: 5, ..Limits::default() });
        for _ in 0..5 {
            s.add_group().unwrap();
        }
        let e = s.add_group().unwrap_err();
        assert!(e.to_string().contains("max_nodes"), "{e}");
        let mut s = Scene3D::new(Limits { max_triangles: 100, ..Limits::default() });
        let e = s.add_sphere(1.0, 32).unwrap_err();
        assert!(e.to_string().contains("max_triangles"), "{e}");
        assert_eq!(s.node_count(), 0, "nothing was left behind");
        assert_eq!(s.triangle_count(), 0);
    }

    #[test]
    fn instances_are_counted_and_bounded() {
        let mut s = scene();
        let n = s.add_box([1.0; 3]).unwrap();
        assert_eq!(s.instance_count(n).unwrap(), 1);
        let mut mats = Vec::new();
        for i in 0..100 {
            mats.extend_from_slice(&Mat4::from_translation(Vec3::new(i as f32 * 2.0, 0.0, 0.0)).to_cols_array());
        }
        s.set_instances(n, &mats).unwrap();
        assert_eq!(s.instance_count(n).unwrap(), 100);
        let b = s.bounds();
        assert!(b.valid && (b.max[0] - (198.0 + 0.5)).abs() < 1e-4, "{b:?}");
        s.set_instances(n, &[]).unwrap();
        assert_eq!(s.instance_count(n).unwrap(), 1);
    }

    #[test]
    fn names_are_found() {
        let mut s = scene();
        let a = s.add_box([1.0; 3]).unwrap();
        s.set_name(a, "crate").unwrap();
        assert_eq!(s.find_node("crate"), Some(a));
        assert_eq!(s.find_node("nothing"), None);
        assert_eq!(s.name(a).unwrap(), "crate");
    }
}
