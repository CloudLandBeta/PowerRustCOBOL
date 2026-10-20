// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Scene3D and Canvas3D (WASM-018 R9).
//!
//! The 3D engine is two layers, with the line between them drawn where a test
//! can stand on it.
//!
//! **The scene** ([`Scene3D`], feature `scene3d-core`) is plain data and plain
//! maths, and runs everywhere - in every test, with no GPU. It holds the node
//! hierarchy, meshes (the six primitives or a model), materials, lights, cameras,
//! animations and skins. It also does everything a picture *depends on* but does
//! not *draw*: [`Scene3D::build_frame`] culls against the camera's frustum, picks
//! the lights and fits their shadow volumes, and poses skinned meshes;
//! [`Scene3D::pick`] casts a ray through a pixel with `parry3d`;
//! [`Scene3D::load_model`] reads glTF 2.0 and GLB, with animation and skins.
//!
//! **The renderer** (`Canvas3D`, feature `scene3d`) takes a [`Frame`] and draws it
//! with `wgpu` into an offscreen texture: physically based shading
//! (Cook-Torrance GGX with Smith visibility and Schlick Fresnel, metallic-roughness
//! workflow, normal maps), up to eight lights, shadow maps for up to two
//! directional or spot lights (3x3 PCF), linear and exponential fog, a solid or
//! gradient background, instancing, skinning on the GPU, blending sorted far to
//! near, and optional 4x multisampling. It reports the [`Backend`] it runs on and
//! reads the picture back as RGBA for `Screenshot` and for the form host to show.
//!
//! # Conventions
//!
//! Right-handed, +Y up, a camera, spot light or directional light looks down its
//! node's **-Z** axis (glTF's convention), triangles wind counter-clockwise from
//! the front, texture coordinates have their origin at the top left, angles in the
//! COBOL-facing methods are in **degrees**. Colours are linear 0 to 1 in the
//! scene and sRGB in the picture; base-colour and emissive textures are decoded as
//! sRGB, the others as linear.
//!
//! # What is not here
//!
//! Image-based lighting, morph targets, cascaded shadow maps, point-light
//! shadows, screen-space effects and compute shaders. The ambient term is a flat
//! colour. See `specs/WASM-018-canvas-and-graphics-objects/plan.md`.

pub mod anim;
pub mod frame;
pub mod math;
pub mod mesh;
pub mod model;
pub mod pick;
pub mod primitives;
pub mod scene;

#[cfg(feature = "scene3d")]
pub mod canvas3d;
#[cfg(feature = "scene3d")]
pub mod gpu;

pub use anim::{Animation, Channel, Interpolation, Path, Skin};
pub use frame::{DrawItem, Frame, FrameStats, Instance, LightItem, MAX_LIGHTS, MAX_SHADOW_MAPS};
pub use mesh::MeshData;
pub use model::ModelInfo;
pub use pick::PickHit;
pub use scene::{AlphaMode, Background, BoundsBox, Camera, Content, Fog, FogMode, Light, LightKind, Material, NodeKind, Scene3D, TextureSlot};

#[cfg(feature = "scene3d")]
pub use canvas3d::Canvas3D;
#[cfg(feature = "scene3d")]
pub use gpu::{Backend, Gpu, GpuInfo, RenderStats, Renderer, RendererConfig};
