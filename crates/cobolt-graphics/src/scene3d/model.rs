// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `LoadModel`: glTF 2.0 and GLB into a scene (WASM-018 R9).
//!
//! What is read: meshes (positions, normals, texture coordinates, triangle
//! indices, joints and weights), materials (the metallic-roughness model with its
//! four textures, alpha mode, double-sided, `KHR_materials_unlit`,
//! `KHR_materials_emissive_strength`), the node hierarchy with its transforms,
//! skins, and animations (translation, rotation and scale channels with step,
//! linear and cubic spline interpolation).
//!
//! What is not, and is reported in [`ModelInfo::warnings`] rather than silently
//! dropped: morph targets (their weights channels are skipped), primitives that
//! are not triangle lists, more than 128 joints in a skin, texture transforms.
//!
//! **No file is read and no connection opened.** Buffers and images come from the
//! GLB's own chunk, from `data:` URIs, or - for a `.gltf` with separate files -
//! from the host through [`AssetHost`]. The file is untrusted: every count is
//! checked against [`Limits`] before memory is reserved, hierarchy depth is not
//! recursion, and a damaged file is an error, never a crash (the tests feed it
//! thousands of mutated files).

use std::collections::HashMap;
use std::sync::Arc;

use gltf::accessor::{DataType, Dimensions};
use gltf::animation::util::ReadOutputs;
use gltf::animation::{Interpolation as GInterp, Property};

use super::anim::{Animation, Channel, Interpolation, Path, Skin, MAX_JOINTS};
use super::math::Mat4;
use super::mesh::MeshData;
use super::scene::{AlphaMode, Content, Material, Scene3D};
use crate::error::{GraphicsError, Result};
use crate::handles::Handle;
use crate::pixels::PixelBuffer;
use crate::resources::AssetHost;

/// What a loaded model added to the scene.
#[derive(Clone, Debug, Default)]
pub struct ModelInfo {
    /// A group node that holds the model's whole scene; move, rotate or remove this one.
    pub root: Handle,
    /// Every node created, the root first.
    pub nodes: Vec<Handle>,
    /// The animations, in file order.
    pub animations: Vec<Handle>,
    /// How many materials.
    pub materials: usize,
    /// How many triangles were added.
    pub triangles: u64,
    /// Things the file asked for that are not supported.
    pub warnings: Vec<String>,
}

fn decode_err(msg: impl std::fmt::Display) -> GraphicsError {
    GraphicsError::Decode(format!("glTF: {msg}"))
}

/// Every index in the file must name something that exists. The `gltf` crate's own validation (and
/// its iterators) index the arrays with these numbers unchecked, so a file that lies about one is a
/// panic there; here it is a sentence.
fn check_references(r: &gltf::json::Root) -> Result<()> {
    fn within(what: &str, i: usize, n: usize) -> Result<()> {
        if i < n {
            Ok(())
        } else {
            Err(decode_err(format!("{what} refers to item {i}, but there are only {n}")))
        }
    }
    let (accessors, views, buffers) = (r.accessors.len(), r.buffer_views.len(), r.buffers.len());
    let (nodes, meshes, materials, textures, images, skins, scenes) = (r.nodes.len(), r.meshes.len(), r.materials.len(), r.textures.len(), r.images.len(), r.skins.len(), r.scenes.len());
    for v in &r.buffer_views {
        within("a buffer view's buffer", v.buffer.value(), buffers)?;
    }
    for a in &r.accessors {
        if let Some(v) = &a.buffer_view {
            within("an accessor's buffer view", v.value(), views)?;
        }
        if let Some(sp) = &a.sparse {
            within("a sparse accessor's index view", sp.indices.buffer_view.value(), views)?;
            within("a sparse accessor's value view", sp.values.buffer_view.value(), views)?;
        }
    }
    for m in &r.meshes {
        for p in &m.primitives {
            for a in p.attributes.values() {
                within("a mesh attribute", a.value(), accessors)?;
            }
            if let Some(i) = &p.indices {
                within("a mesh's indices", i.value(), accessors)?;
            }
            if let Some(mat) = &p.material {
                within("a mesh's material", mat.value(), materials)?;
            }
            for t in p.targets.iter().flatten() {
                for a in [&t.positions, &t.normals, &t.tangents].into_iter().flatten() {
                    within("a morph target", a.value(), accessors)?;
                }
            }
        }
    }
    for n in &r.nodes {
        for c in n.children.iter().flatten() {
            within("a node's child", c.value(), nodes)?;
        }
        if let Some(m) = &n.mesh {
            within("a node's mesh", m.value(), meshes)?;
        }
        if let Some(s) = &n.skin {
            within("a node's skin", s.value(), skins)?;
        }
        if let Some(c) = &n.camera {
            within("a node's camera", c.value(), r.cameras.len())?;
        }
    }
    for s in &r.skins {
        if let Some(i) = &s.inverse_bind_matrices {
            within("a skin's inverse bind matrices", i.value(), accessors)?;
        }
        for j in &s.joints {
            within("a skin's joint", j.value(), nodes)?;
        }
    }
    for a in &r.animations {
        for s in &a.samplers {
            within("an animation sampler's input", s.input.value(), accessors)?;
            within("an animation sampler's output", s.output.value(), accessors)?;
        }
        for c in &a.channels {
            within("an animation channel's sampler", c.sampler.value(), a.samplers.len())?;
            within("an animation channel's node", c.target.node.value(), nodes)?;
        }
    }
    for i in &r.images {
        match (&i.buffer_view, &i.uri) {
            (Some(v), _) => {
                within("an image's buffer view", v.value(), views)?;
                if i.mime_type.is_none() {
                    return Err(decode_err("an image in a buffer view needs a mimeType"));
                }
            }
            (None, Some(_)) => {}
            (None, None) => return Err(decode_err("an image has neither a buffer view nor a uri")),
        }
    }
    for t in &r.textures {
        within("a texture's image", t.source.value(), images)?;
        if let Some(s) = &t.sampler {
            within("a texture's sampler", s.value(), r.samplers.len())?;
        }
    }
    for m in &r.materials {
        let p = &m.pbr_metallic_roughness;
        for info in [&p.base_color_texture, &p.metallic_roughness_texture, &m.emissive_texture].into_iter().flatten() {
            within("a material's texture", info.index.value(), textures)?;
        }
        if let Some(n) = &m.normal_texture {
            within("a material's normal texture", n.index.value(), textures)?;
        }
        if let Some(o) = &m.occlusion_texture {
            within("a material's occlusion texture", o.index.value(), textures)?;
        }
    }
    for s in &r.scenes {
        for n in &s.nodes {
            within("a scene's node", n.value(), nodes)?;
        }
    }
    if let Some(s) = &r.scene {
        within("the default scene", s.value(), scenes)?;
    }
    Ok(())
}

/// Refuse an accessor of a kind the reader below would not handle. The `gltf` crate checks some of
/// this when it parses and assumes the rest (a mismatch there is an assertion, not an error), and a
/// damaged file is free to lie, so everything this loader reads is checked first.
fn need(acc: &gltf::Accessor<'_>, what: &str, dims: Dimensions, types: &[DataType]) -> Result<()> {
    if acc.dimensions() != dims || !types.contains(&acc.data_type()) {
        return Err(decode_err(format!("{what} must be {dims:?} of {types:?}, but the file gives {:?} of {:?}", acc.dimensions(), acc.data_type())));
    }
    if acc.count() == 0 {
        return Err(decode_err(format!("{what} has no elements")));
    }
    if acc.sparse().is_some() {
        return Err(decode_err(format!("{what} is a sparse accessor, which this loader does not support")));
    }
    if let Some(stride) = acc.view().and_then(|v| v.stride()) {
        if stride < acc.size() {
            return Err(decode_err(format!("{what} has a byte stride of {stride}, smaller than one element ({})", acc.size())));
        }
    }
    // Integer data that is not normalised cannot be a colour, weight or rotation.
    if matches!(acc.data_type(), DataType::I8 | DataType::U8 | DataType::I16 | DataType::U16) && !acc.normalized() && !what.starts_with("joints") && !what.starts_with("indices") {
        return Err(decode_err(format!("{what} is integer data that is not normalised")));
    }
    Ok(())
}

/// Standard base64 (RFC 4648), padding optional, whitespace ignored. `None` for anything else.
fn base64_decode(s: &str, max_len: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
            if out.len() > max_len {
                return None;
            }
        }
    }
    Some(out)
}

/// `%20` and friends in a relative URI.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = ((b[i + 1] as char).to_digit(16), (b[i + 2] as char).to_digit(16)) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The bytes a URI names: a `data:` URI decoded, anything else asked of the host.
fn resolve_uri(uri: &str, max_len: usize, external: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<Vec<u8>> {
    if let Some(rest) = uri.strip_prefix("data:") {
        let (meta, data) = rest.split_once(',').ok_or_else(|| decode_err("a data URI has no comma"))?;
        if !meta.ends_with(";base64") {
            return Err(decode_err("only base64 data URIs are supported"));
        }
        return base64_decode(data, max_len).ok_or_else(|| decode_err("a data URI is not valid base64 or is too large"));
    }
    if uri.contains("://") || uri.starts_with("//") {
        return Err(decode_err(format!("the model refers to {uri}, which is on the network; this crate opens no connection (bundle it as an asset)")));
    }
    let name = percent_decode(uri);
    external(&name).ok_or_else(|| decode_err(format!("the model refers to the file {name}, which the host did not supply")))
}

impl Scene3D {
    /// `LoadModel` of bytes already in memory (a GLB, or a `.gltf` that needs no other file).
    pub fn load_model_bytes(&mut self, bytes: &[u8]) -> Result<ModelInfo> {
        self.load_model(bytes, &|_| None)
    }

    /// `LoadModel` of a bundled asset; the relative files a `.gltf` names are looked up beside it.
    pub fn load_model_asset(&mut self, name: &str, assets: &dyn AssetHost) -> Result<ModelInfo> {
        let bytes = assets.asset(name).ok_or_else(|| GraphicsError::invalid(format!("there is no asset named {name}")))?;
        let dir = name.rfind('/').map(|i| &name[..=i]).unwrap_or("").to_string();
        self.load_model(&bytes, &|rel| assets.asset(&format!("{dir}{rel}")))
    }

    /// Load a model; `external` supplies the files a `.gltf` names (return `None` for "not available").
    ///
    /// Nothing is added to the scene unless the whole model loads: a failure part-way removes what was added.
    pub fn load_model(&mut self, bytes: &[u8], external: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<ModelInfo> {
        self.limits.check_model_bytes(bytes.len())?;
        let (nodes_before, tris_before) = (self.nodes.len(), self.triangles);
        let mut info = ModelInfo::default();
        // The `gltf` crate trusts what it has validated; the checks in `need` cover what it does not. This
        // guard is the last line: a panic from a file that slips past them becomes an error, and the
        // tests assert it never fires (they look for the words INTERNAL ERROR).
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.load_model_inner(bytes, external, &mut info)))
            .unwrap_or_else(|_| Err(decode_err("INTERNAL ERROR: the file is damaged in a way the reader does not handle")));
        match outcome {
            Ok(()) => {
                info.triangles = self.triangles.saturating_sub(tris_before);
                Ok(info)
            }
            Err(e) => {
                // Roll back: removing the root takes every node with it, and the meshes with their last node.
                if info.root != 0 {
                    let _ = self.remove(info.root);
                }
                for a in &info.animations {
                    let _ = self.animations.remove(*a);
                }
                // A mesh added just before the node limit stopped its node has no user: it is ours.
                let orphans: Vec<Handle> = self.meshes.iter().filter(|(_, m)| m.users == 0).map(|(h, _)| h).collect();
                for m in orphans {
                    self.release_mesh(m);
                }
                debug_assert!(self.nodes.len() >= nodes_before && self.triangles >= tris_before);
                Err(e)
            }
        }
    }

    fn load_model_inner(&mut self, bytes: &[u8], external: &dyn Fn(&str) -> Option<Vec<u8>>, info: &mut ModelInfo) -> Result<()> {
        // Parse first, check every cross-reference ourselves, and only then let the `gltf` crate validate:
        // its validation indexes the arrays with whatever the file says.
        let parsed = gltf::Gltf::from_slice_without_validation(bytes).map_err(decode_err)?;
        check_references(parsed.document.as_json())?;
        let gltf::Gltf { document, blob } = parsed;
        let document = gltf::Document::from_json(document.into_json()).map_err(decode_err)?;
        let g = gltf::Gltf { document, blob };
        let doc = &g.document;
        let max_bytes = self.limits.max_model_bytes as usize;

        // ---- buffers ----
        let mut buffers: Vec<Vec<u8>> = Vec::with_capacity(doc.buffers().len());
        let mut total = 0usize;
        for b in doc.buffers() {
            let data = match b.source() {
                gltf::buffer::Source::Bin => g.blob.clone().ok_or_else(|| decode_err("the model uses its binary chunk but has none"))?,
                gltf::buffer::Source::Uri(uri) => resolve_uri(uri, max_bytes, external)?,
            };
            if data.len() < b.length() {
                return Err(decode_err(format!("buffer {} is {} bytes long but its file holds {}", b.index(), b.length(), data.len())));
            }
            total = total.saturating_add(data.len());
            self.limits.check_model_bytes(total)?;
            buffers.push(data);
        }

        // ---- the size of what is coming, before any of it is built ----
        let mut tri_budget = 0u64;
        for mesh in doc.meshes() {
            for p in mesh.primitives() {
                if let Some(a) = p.indices() {
                    tri_budget = tri_budget.saturating_add(a.count() as u64 / 3);
                }
                if let Some(a) = p.get(&gltf::Semantic::Positions) {
                    self.limits.check_vertices(a.count() as u64)?;
                }
            }
        }
        // Every mesh once is a lower bound of what the model adds; the checks while building are exact.
        self.limits.check_triangles(self.triangles.saturating_add(tri_budget))?;

        // ---- the root ----
        let root = self.add_group()?;
        info.root = root;
        info.nodes.push(root);
        let scene_name = doc.default_scene().or_else(|| doc.scenes().next()).and_then(|s| s.name().map(str::to_string)).unwrap_or_else(|| "model".to_string());
        self.set_name(root, &scene_name)?;

        // ---- images and materials, on demand ----
        let mut images: HashMap<usize, Arc<PixelBuffer>> = HashMap::new();
        let mut materials: HashMap<usize, Handle> = HashMap::new();
        let mut meshes: HashMap<(usize, usize), Handle> = HashMap::new();
        let mut node_map: HashMap<usize, Handle> = HashMap::new();

        let Some(scene) = doc.default_scene().or_else(|| doc.scenes().next()) else {
            return Err(decode_err("the model has no scene"));
        };

        // ---- nodes (an explicit stack: a hostile file may nest as deep as it likes) ----
        let mut stack: Vec<(gltf::Node<'_>, Handle)> = scene.nodes().map(|n| (n, root)).collect();
        stack.reverse();
        let mut visited = std::collections::HashSet::new();
        while let Some((gn, parent)) = stack.pop() {
            if !visited.insert(gn.index()) {
                info.warnings.push(format!("node {} is reached twice (a shared node is loaded once)", gn.index()));
                continue;
            }
            let prims: Vec<gltf::Primitive<'_>> = gn.mesh().map(|m| m.primitives().filter(|p| p.mode() == gltf::mesh::Mode::Triangles).collect()).unwrap_or_default();
            if let Some(m) = gn.mesh() {
                let skipped = m.primitives().len() - prims.len();
                if skipped > 0 {
                    info.warnings.push(format!("mesh {}: {skipped} primitive(s) are not triangle lists and were skipped", m.index()));
                }
            }
            let nh = if prims.len() == 1 {
                let p = &prims[0];
                let mesh_idx = gn.mesh().map(|m| m.index()).unwrap_or(0);
                let h = self.model_primitive(mesh_idx, 0, p, doc, &buffers, external, &mut images, &mut materials, &mut meshes, info)?;
                let node = self.add_mesh_node(h.0, h.1, gn.name().unwrap_or(""))?;
                node
            } else {
                let group = self.add_group()?;
                if let Some(name) = gn.name() {
                    self.set_name(group, name)?;
                }
                group
            };
            info.nodes.push(nh);
            node_map.insert(gn.index(), nh);
            self.attach_new_child(nh, parent)?;
            let (t, r, s) = gn.transform().decomposed();
            self.set_position(nh, t)?;
            self.set_rotation_quaternion(nh, r)?;
            let s = s.map(|v| if v == 0.0 { 1.0e-6 } else { v });
            self.set_scale(nh, s)?;
            if prims.len() > 1 {
                let mesh_idx = gn.mesh().map(|m| m.index()).unwrap_or(0);
                for (pi, p) in prims.iter().enumerate() {
                    let (mesh, mat) = self.model_primitive(mesh_idx, pi, p, doc, &buffers, external, &mut images, &mut materials, &mut meshes, info)?;
                    let child = self.add_mesh_node(mesh, mat, &format!("{}.{pi}", gn.name().unwrap_or("mesh")))?;
                    info.nodes.push(child);
                    self.attach_new_child(child, nh)?;
                }
            }
            let kids: Vec<gltf::Node<'_>> = gn.children().collect();
            for c in kids.into_iter().rev() {
                stack.push((c, nh));
            }
        }

        // ---- skins ----
        let mut skin_map: HashMap<usize, Handle> = HashMap::new();
        for gn in doc.nodes() {
            let (Some(skin), Some(&nh)) = (gn.skin(), node_map.get(&gn.index())) else { continue };
            let sh = match skin_map.get(&skin.index()) {
                Some(h) => *h,
                None => {
                    let joints: Vec<Handle> = skin.joints().filter_map(|j| node_map.get(&j.index()).copied()).collect();
                    if joints.len() != skin.joints().count() {
                        info.warnings.push(format!("skin {}: some joints are not in the scene and were left out", skin.index()));
                    }
                    if skin.joints().count() > MAX_JOINTS {
                        info.warnings.push(format!("skin {} has {} joints; the renderer supports {MAX_JOINTS}, the mesh is drawn unskinned", skin.index(), skin.joints().count()));
                        continue;
                    }
                    let ibm: Vec<Mat4> = match skin.inverse_bind_matrices() {
                        Some(a) => {
                            need(&a, "inverse bind matrices", Dimensions::Mat4, &[DataType::F32])?;
                            match skin.reader(|b| buffers.get(b.index()).map(|v| v.as_slice())).read_inverse_bind_matrices() {
                                Some(it) => it.map(|m| Mat4::from_cols_array_2d(&m)).collect(),
                                None => return Err(decode_err(format!("the inverse bind matrices of skin {} reach outside their buffer", skin.index()))),
                            }
                        }
                        None => vec![Mat4::IDENTITY; skin.joints().count()],
                    };
                    // Keep only the matrices of the joints that exist.
                    let ibm: Vec<Mat4> = skin.joints().zip(ibm.into_iter().chain(std::iter::repeat(Mat4::IDENTITY))).filter(|(j, _)| node_map.contains_key(&j.index())).map(|(_, m)| m).collect();
                    let h = self.add_skin(Skin { joints, inverse_bind: ibm })?;
                    skin_map.insert(skin.index(), h);
                    h
                }
            };
            // The node that draws (itself, or its primitive children).
            let drawers: Vec<Handle> = match &self.nodes.get(nh)?.content {
                Content::Mesh(_) => vec![nh],
                _ => self.nodes.get(nh)?.children.clone(),
            };
            for d in drawers {
                if let Ok(n) = self.nodes.get(d) {
                    if let Content::Mesh(m) = &n.content {
                        if self.meshes.get(m.mesh).map(|e| e.data.is_skinned()).unwrap_or(false) {
                            self.set_skin(d, sh)?;
                        }
                    }
                }
            }
        }

        // ---- animations ----
        for anim in doc.animations() {
            let mut channels = Vec::new();
            for ch in anim.channels() {
                let Some(&target) = node_map.get(&ch.target().node().index()) else { continue };
                let path = match ch.target().property() {
                    Property::Translation => Path::Translation,
                    Property::Rotation => Path::Rotation,
                    Property::Scale => Path::Scale,
                    Property::MorphTargetWeights => {
                        info.warnings.push(format!("animation {}: a morph target weights channel was skipped (morph targets are not supported)", anim.index()));
                        continue;
                    }
                };
                let interpolation = match ch.sampler().interpolation() {
                    GInterp::Linear => Interpolation::Linear,
                    GInterp::Step => Interpolation::Step,
                    GInterp::CubicSpline => Interpolation::CubicSpline,
                };
                need(&ch.sampler().input(), "animation times", Dimensions::Scalar, &[DataType::F32])?;
                match path {
                    Path::Translation | Path::Scale => need(&ch.sampler().output(), "animation values", Dimensions::Vec3, &[DataType::F32])?,
                    Path::Rotation => need(&ch.sampler().output(), "animation rotations", Dimensions::Vec4, &[DataType::F32, DataType::I8, DataType::U8, DataType::I16, DataType::U16])?,
                }
                self.limits.check_series_points(ch.sampler().input().count() as u64)?;
                let reader = ch.reader(|b| buffers.get(b.index()).map(|v| v.as_slice()));
                let times = reader.read_inputs().map(|i| i.collect::<Vec<f32>>()).ok_or_else(|| decode_err(format!("the keyframe times of animation {} reach outside their buffer", anim.index())))?;
                let outputs = reader.read_outputs().ok_or_else(|| decode_err(format!("the keyframe values of animation {} reach outside their buffer", anim.index())))?;
                let values: Vec<f32> = match outputs {
                    ReadOutputs::Translations(it) => it.flatten().collect(),
                    ReadOutputs::Scales(it) => it.flatten().collect(),
                    ReadOutputs::Rotations(it) => it.into_f32().flatten().collect(),
                    ReadOutputs::MorphTargetWeights(_) => continue,
                };
                let channel = Channel { target, path, interpolation, times, values };
                channel.validate().map_err(|e| decode_err(format!("animation {}: {e}", anim.index())))?;
                channels.push(channel);
            }
            let a = Animation::new(anim.name().unwrap_or(""), channels)?;
            let h = self.add_animation(a)?;
            info.animations.push(h);
        }

        info.materials = materials.len();
        Ok(())
    }

    /// One glTF primitive as a scene mesh (cached per mesh and primitive) with its material.
    #[allow(clippy::too_many_arguments)]
    fn model_primitive(
        &mut self,
        mesh_idx: usize,
        prim_idx: usize,
        p: &gltf::Primitive<'_>,
        doc: &gltf::Document,
        buffers: &[Vec<u8>],
        external: &dyn Fn(&str) -> Option<Vec<u8>>,
        images: &mut HashMap<usize, Arc<PixelBuffer>>,
        materials: &mut HashMap<usize, Handle>,
        meshes: &mut HashMap<(usize, usize), Handle>,
        info: &mut ModelInfo,
    ) -> Result<(Handle, Option<Handle>)> {
        let mesh = match meshes.get(&(mesh_idx, prim_idx)) {
            Some(h) => *h,
            None => {
                // Every accessor is checked for its kind before the reader (which asserts) sees it.
                let pos_acc = p.get(&gltf::Semantic::Positions).ok_or_else(|| decode_err(format!("mesh {mesh_idx} has a primitive without positions")))?;
                need(&pos_acc, "positions", Dimensions::Vec3, &[DataType::F32])?;
                self.limits.check_vertices(pos_acc.count() as u64)?;
                let norm_acc = p.get(&gltf::Semantic::Normals);
                if let Some(a) = &norm_acc {
                    need(a, "normals", Dimensions::Vec3, &[DataType::F32])?;
                }
                let uv_acc = p.get(&gltf::Semantic::TexCoords(0));
                if let Some(a) = &uv_acc {
                    need(a, "texture coordinates", Dimensions::Vec2, &[DataType::F32, DataType::U8, DataType::U16])?;
                }
                let joint_acc = p.get(&gltf::Semantic::Joints(0));
                if let Some(a) = &joint_acc {
                    need(a, "joints", Dimensions::Vec4, &[DataType::U8, DataType::U16])?;
                }
                let weight_acc = p.get(&gltf::Semantic::Weights(0));
                if let Some(a) = &weight_acc {
                    need(a, "weights", Dimensions::Vec4, &[DataType::F32, DataType::U8, DataType::U16])?;
                }
                let index_acc = p.indices();
                if let Some(a) = &index_acc {
                    need(a, "indices", Dimensions::Scalar, &[DataType::U8, DataType::U16, DataType::U32])?;
                    self.limits.check_triangles(a.count() as u64 / 3)?;
                }
                let outside = |what: &str| decode_err(format!("the {what} of mesh {mesh_idx} reach outside their buffer"));
                let reader = p.reader(|b| buffers.get(b.index()).map(|v| v.as_slice()));
                let positions: Vec<[f32; 3]> = reader.read_positions().ok_or_else(|| outside("positions"))?.collect();
                let normals: Vec<[f32; 3]> = match norm_acc {
                    Some(_) => reader.read_normals().ok_or_else(|| outside("normals"))?.collect(),
                    None => Vec::new(),
                };
                let uvs: Vec<[f32; 2]> = match uv_acc {
                    Some(_) => reader.read_tex_coords(0).ok_or_else(|| outside("texture coordinates"))?.into_f32().collect(),
                    None => Vec::new(),
                };
                let indices: Vec<u32> = match index_acc {
                    Some(_) => reader.read_indices().ok_or_else(|| outside("indices"))?.into_u32().collect(),
                    None => (0..positions.len() as u32).collect(),
                };
                let joints: Vec<[u16; 4]> = match joint_acc {
                    Some(_) => reader.read_joints(0).ok_or_else(|| outside("joints"))?.into_u16().collect(),
                    None => Vec::new(),
                };
                let weights: Vec<[f32; 4]> = match weight_acc {
                    Some(_) => reader.read_weights(0).ok_or_else(|| outside("weights"))?.into_f32().collect(),
                    None => Vec::new(),
                };
                let (joints, weights) = if joints.len() == positions.len() && weights.len() == positions.len() { (joints, weights) } else { (vec![], vec![]) };
                let data = MeshData::new(positions, normals, uvs, indices, joints, weights).map_err(|e| decode_err(format!("mesh {mesh_idx}: {e}")))?;
                let h = self.add_mesh(data)?;
                meshes.insert((mesh_idx, prim_idx), h);
                h
            }
        };
        let material = match p.material().index() {
            None => None,
            Some(mi) => match materials.get(&mi) {
                Some(h) => Some(*h),
                None => {
                    let h = self.model_material(&p.material(), doc, buffers, external, images, info)?;
                    materials.insert(mi, h);
                    Some(h)
                }
            },
        };
        Ok((mesh, material))
    }

    fn model_material(
        &mut self,
        gm: &gltf::Material<'_>,
        _doc: &gltf::Document,
        buffers: &[Vec<u8>],
        external: &dyn Fn(&str) -> Option<Vec<u8>>,
        images: &mut HashMap<usize, Arc<PixelBuffer>>,
        info: &mut ModelInfo,
    ) -> Result<Handle> {
        let h = self.create_material()?;
        let pbr = gm.pbr_metallic_roughness();
        let mut m = Material {
            base_color: pbr.base_color_factor().map(|v| v.clamp(0.0, 1.0)),
            metallic: pbr.metallic_factor().clamp(0.0, 1.0),
            roughness: pbr.roughness_factor().clamp(0.0, 1.0),
            emissive: {
                let strength = gm.emissive_strength().unwrap_or(1.0);
                gm.emissive_factor().map(|v| (v * strength).clamp(0.0, 1000.0))
            },
            alpha_mode: match gm.alpha_mode() {
                gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
                gltf::material::AlphaMode::Mask => AlphaMode::Mask,
                gltf::material::AlphaMode::Blend => AlphaMode::Blend,
            },
            alpha_cutoff: gm.alpha_cutoff().unwrap_or(0.5).clamp(0.0, 1.0),
            double_sided: gm.double_sided(),
            unlit: gm.unlit(),
            ..Material::default()
        };
        let mut load = |tex: gltf::Texture<'_>, what: &str, info: &mut ModelInfo| -> Option<Arc<PixelBuffer>> {
            let src = tex.source();
            if let Some(i) = images.get(&src.index()) {
                return Some(i.clone());
            }
            let bytes = match src.source() {
                gltf::image::Source::View { view, .. } => {
                    let buf = buffers.get(view.buffer().index())?;
                    buf.get(view.offset()..view.offset().checked_add(view.length())?).map(|s| s.to_vec())
                }
                gltf::image::Source::Uri { uri, .. } => resolve_uri(uri, self.limits.max_model_bytes as usize, external).ok(),
            };
            let Some(bytes) = bytes else {
                info.warnings.push(format!("the {what} image could not be read and was left out"));
                return None;
            };
            match crate::imagebuf::decode(&bytes, &self.limits) {
                Ok(px) => {
                    let a = Arc::new(px);
                    images.insert(src.index(), a.clone());
                    Some(a)
                }
                Err(e) => {
                    info.warnings.push(format!("the {what} image could not be decoded ({e}) and was left out"));
                    None
                }
            }
        };
        if let Some(t) = pbr.base_color_texture() {
            m.base_color_texture = load(t.texture(), "base colour", info);
        }
        if let Some(t) = pbr.metallic_roughness_texture() {
            m.metallic_roughness_texture = load(t.texture(), "metallic-roughness", info);
        }
        if let Some(t) = gm.emissive_texture() {
            m.emissive_texture = load(t.texture(), "emissive", info);
        }
        if let Some(t) = gm.normal_texture() {
            m.normal_scale = t.scale();
            m.normal_texture = load(t.texture(), "normal", info);
        }
        *self.materials.get_mut(h)? = m;
        self.revision += 1;
        Ok(h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_decodes_with_and_without_padding() {
        assert_eq!(base64_decode("TWFu", 100).unwrap(), b"Man");
        assert_eq!(base64_decode("TWE=", 100).unwrap(), b"Ma");
        assert_eq!(base64_decode("TWE", 100).unwrap(), b"Ma");
        assert_eq!(base64_decode("TW\nFu", 100).unwrap(), b"Man");
        assert!(base64_decode("T*Fu", 100).is_none());
        assert!(base64_decode("TWFuTWFuTWFu", 4).is_none(), "the size bound holds");
    }

    #[test]
    fn percent_decoding() {
        assert_eq!(percent_decode("a%20b.bin"), "a b.bin");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("%4"), "%4");
    }

    #[test]
    fn network_and_missing_resources_are_refused_with_a_reason() {
        let none = |_: &str| None;
        let e = resolve_uri("https://example.com/a.bin", 10, &none).unwrap_err();
        assert!(e.to_string().contains("network"), "{e}");
        let e = resolve_uri("a.bin", 10, &none).unwrap_err();
        assert!(e.to_string().contains("a.bin"), "{e}");
        assert_eq!(resolve_uri("a%20b.bin", 10, &|n: &str| (n == "a b.bin").then(|| vec![1, 2, 3])).unwrap(), vec![1, 2, 3]);
        assert!(resolve_uri("data:text/plain,abc", 10, &none).is_err(), "only base64 data URIs");
    }
}
