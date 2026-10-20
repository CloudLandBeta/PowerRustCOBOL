// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The methods of Scene3D (WASM-018 R9).

use super::{dispatch, done, method};
use crate::error::{GraphicsError, Result};
use crate::registry::{MethodInfo, ObjectKind, Registry};
use crate::scene3d::scene::NodeKind;
use crate::scene3d::{AlphaMode, Fog, FogMode, Scene3D, TextureSlot};
use crate::value::{Args, Value};

fn v3(a: &Args<'_>, i: usize) -> Result<[f32; 3]> {
    Ok([a.f32(i)?, a.f32(i + 1)?, a.f32(i + 2)?])
}

fn segments(a: &Args<'_>, i: usize, default: i64) -> Result<u32> {
    Ok(a.int_in_or(i, default, 3, 512)? as u32)
}

#[allow(unused_variables)]
pub(crate) static METHODS: &[crate::value::Method<Scene3D>] = &[
    // ---- making things -------------------------------------------------------------------
    method!(Scene3D, "AddGroup", [], [], Handle, "An empty node to hang others from.", |o, r, a| Ok(o.add_group()?.into())),
    method!(Scene3D, "AddBox", [width: Num, height: Num, depth: Num], [], Handle, "A box centred on its node.", |o, r, a| Ok(o.add_box(v3(a, 0)?)?.into())),
    method!(Scene3D, "AddSphere", [radius: Num], [segments: Int], Handle, "A sphere (32 segments by default).", |o, r, a| Ok(o.add_sphere(a.f32(0)?, segments(a, 1, 32)?)?.into())),
    method!(Scene3D, "AddPlane", [width: Num, depth: Num], [], Handle, "A flat rectangle in the XZ plane facing up.", |o, r, a| Ok(o.add_plane(a.f32(0)?, a.f32(1)?)?.into())),
    method!(Scene3D, "AddCylinder", [radius: Num, height: Num], [segments: Int], Handle, "A cylinder along Y.", |o, r, a| Ok(o.add_cylinder(a.f32(0)?, a.f32(1)?, segments(a, 2, 32)?)?.into())),
    method!(Scene3D, "AddCone", [radius: Num, height: Num], [segments: Int], Handle, "A cone along Y, apex up.", |o, r, a| Ok(o.add_cone(a.f32(0)?, a.f32(1)?, segments(a, 2, 32)?)?.into())),
    method!(Scene3D, "AddTorus", [radius: Num, tube: Num], [radial_segments: Int, tubular_segments: Int], Handle, "A torus in the XZ plane: radius to the middle of the tube, and the tube's own radius.", |o, r, a| {
        Ok(o.add_torus(a.f32(0)?, a.f32(1)?, segments(a, 2, 16)?, segments(a, 3, 32)?)?.into())
    }),
    method!(Scene3D, "AddMesh", [positions: Floats, indices: Ints], [normals: Floats, uvs: Floats], Handle, "A mesh of your own: positions (x, y, z ...), triangle indices (three per triangle), optional normals (one per vertex; computed when omitted) and texture coordinates (u, v ...).", |o, r, a| {
        let idx = a.ints(1)?;
        if idx.iter().any(|i| *i < 0 || *i > u32::MAX as i64) {
            return Err(a.refuse(1, "holds a vertex number that is not valid"));
        }
        let indices: Vec<u32> = idx.iter().map(|i| *i as u32).collect();
        let normals = if a.has(2) { a.floats32(2)? } else { Vec::new() };
        let uvs = if a.has(3) { a.floats32(3)? } else { Vec::new() };
        Ok(o.add_mesh_from_arrays(&a.floats32(0)?, &normals, &uvs, &indices)?.into())
    }),
    method!(Scene3D, "LoadModel", [name: Text], [], Handle, "Load a glTF 2.0 or GLB asset: meshes, materials, node transforms, skins and animations. The answer is the node that holds the whole model; what could not be honoured is in GetLoadWarnings.", |o, r, a| {
        let host = r.assets().ok_or_else(|| GraphicsError::invalid("the host has no assets to load a model from"))?;
        Ok(o.load_model_asset(a.text(0)?, host)?.root.into())
    }),
    method!(Scene3D, "LoadModelFromBytes", [bytes: Bytes], [], Handle, "Load a GLB (or a self-contained glTF) from bytes already in memory.", |o, r, a| Ok(o.load_model_bytes(a.bytes(0)?)?.root.into())),
    method!(Scene3D, "GetLoadWarnings", [], [], Text, "What the last model load could not honour (morph targets, unsupported primitives ...), one per line.", |o, r, a| Ok(o.last_load_warnings().join("\n").into())),
    method!(Scene3D, "AddDirectionalLight", [red: Num, green: Num, blue: Num, intensity: Num], [cast_shadow: Flag], Handle, "A sun along the node's -Z axis (aim it with LookAt); colour 0 to 1 per channel, intensity 1 lights a surface facing it with its own colour.", |o, r, a| {
        Ok(o.add_directional_light(v3(a, 0)?, a.f32(3)?, if a.has(4) { a.flag(4)? } else { false })?.into())
    }),
    method!(Scene3D, "AddPointLight", [red: Num, green: Num, blue: Num, intensity: Num, range: Num], [], Handle, "A light in all directions; it falls off with the square of the distance and fades out at range (0 = never).", |o, r, a| Ok(o.add_point_light(v3(a, 0)?, a.f32(3)?, a.f32(4)?)?.into())),
    method!(Scene3D, "AddSpotLight", [red: Num, green: Num, blue: Num, intensity: Num, range: Num, inner_degrees: Num, outer_degrees: Num], [cast_shadow: Flag], Handle, "A cone of light along the node's -Z axis.", |o, r, a| {
        Ok(o.add_spot_light(v3(a, 0)?, a.f32(3)?, a.f32(4)?, a.f32(5)?, a.f32(6)?, if a.has(7) { a.flag(7)? } else { false })?.into())
    }),
    method!(Scene3D, "SetLight", [node: Handle, red: Num, green: Num, blue: Num, intensity: Num, range: Num], [], Nothing, "Change a light's colour, intensity and range.", |o, r, a| {
        o.set_light(a.handle(0)?, v3(a, 1)?, a.f32(4)?, a.f32(5)?)?;
        done()
    }),
    method!(Scene3D, "SetAmbient", [red: Num, green: Num, blue: Num, intensity: Num], [], Nothing, "The flat light every surface gets from everywhere.", |o, r, a| {
        o.set_ambient(v3(a, 0)?, a.f32(3)?)?;
        done()
    }),
    method!(Scene3D, "AddCamera", [fov_degrees: Num, near: Num, far: Num], [], Handle, "A perspective camera looking down its node's -Z axis; the field of view is vertical.", |o, r, a| Ok(o.add_perspective_camera(a.f32(0)?, a.f32(1)?, a.f32(2)?)?.into())),
    method!(Scene3D, "AddOrthographicCamera", [height: Num, near: Num, far: Num], [], Handle, "An orthographic camera; height is how much of the world fits vertically.", |o, r, a| Ok(o.add_orthographic_camera(a.f32(0)?, a.f32(1)?, a.f32(2)?)?.into())),
    method!(Scene3D, "SetBackground", [color: Color], [], Nothing, "What the picture is cleared to (a colour with alpha 0 gives transparent pixels).", |o, r, a| {
        o.set_background(a.color(0)?);
        done()
    }),
    method!(Scene3D, "SetBackgroundGradient", [top: Color, bottom: Color], [], Nothing, "A vertical gradient background.", |o, r, a| {
        o.set_background_gradient(a.color(0)?, a.color(1)?);
        done()
    }),
    method!(Scene3D, "SetFog", [red: Num, green: Num, blue: Num, near: Num, far: Num], [], Nothing, "Linear fog: none at near, total at far.", |o, r, a| {
        o.set_fog(Some(Fog { color: v3(a, 0)?, mode: FogMode::Linear { near: a.f32(3)?, far: a.f32(4)? } }))?;
        done()
    }),
    method!(Scene3D, "SetFogExp", [red: Num, green: Num, blue: Num, density: Num], [], Nothing, "Exponential fog: 1 - exp(-density x distance).", |o, r, a| {
        o.set_fog(Some(Fog { color: v3(a, 0)?, mode: FogMode::Exponential { density: a.f32(3)? } }))?;
        done()
    }),
    method!(Scene3D, "ClearFog", [], [], Nothing, "No fog.", |o, r, a| {
        o.set_fog(None)?;
        done()
    }),
    // ---- the hierarchy -------------------------------------------------------------------
    method!(Scene3D, "SetParent", [node: Handle], [parent: Handle], Nothing, "Put a node under another (omit the parent, or give 0, to make it a root); its local transform is kept.", |o, r, a| {
        o.set_parent(a.handle(0)?, a.opt_handle(1)?)?;
        done()
    }),
    method!(Scene3D, "GetParent", [node: Handle], [], Handle, "A node's parent (0 for a root).", |o, r, a| Ok(o.parent(a.handle(0)?)?.unwrap_or(0).into())),
    method!(Scene3D, "GetChildren", [node: Handle], [], Ints, "A node's children.", |o, r, a| Ok(o.children(a.handle(0)?)?.into())),
    method!(Scene3D, "Remove", [node: Handle], [], Nothing, "Remove a node and everything under it.", |o, r, a| {
        o.remove(a.handle(0)?)?;
        done()
    }),
    method!(Scene3D, "SetName", [node: Handle, name: Text], [], Nothing, "Name a node.", |o, r, a| {
        o.set_name(a.handle(0)?, a.text(1)?)?;
        done()
    }),
    method!(Scene3D, "GetName", [node: Handle], [], Text, "A node's name.", |o, r, a| Ok(o.name(a.handle(0)?)?.into())),
    method!(Scene3D, "FindNode", [name: Text], [], Handle, "The first node with this name (0 if none).", |o, r, a| Ok(o.find_node(a.text(0)?).unwrap_or(0).into())),
    method!(Scene3D, "SetVisible", [node: Handle, visible: Flag], [], Nothing, "Show or hide a node and everything under it.", |o, r, a| {
        o.set_visible(a.handle(0)?, a.flag(1)?)?;
        done()
    }),
    method!(Scene3D, "IsVisible", [node: Handle], [], Int, "1 if the node itself is visible.", |o, r, a| Ok(o.is_visible(a.handle(0)?)?.into())),
    method!(Scene3D, "GetNodeKind", [node: Handle], [], Text, "GROUP, MESH, LIGHT or CAMERA.", |o, r, a| {
        Ok(match o.kind_of(a.handle(0)?)? {
            NodeKind::Group => "GROUP",
            NodeKind::Mesh => "MESH",
            NodeKind::Light => "LIGHT",
            NodeKind::Camera => "CAMERA",
        }
        .into())
    }),
    // ---- transforms ----------------------------------------------------------------------
    method!(Scene3D, "SetPosition", [node: Handle, x: Num, y: Num, z: Num], [], Nothing, "Position relative to the parent.", |o, r, a| {
        o.set_position(a.handle(0)?, v3(a, 1)?)?;
        done()
    }),
    method!(Scene3D, "SetRotation", [node: Handle, x_degrees: Num, y_degrees: Num, z_degrees: Num], [], Nothing, "Rotation as Euler angles in degrees: yaw about Y, then pitch about X, then roll about Z.", |o, r, a| {
        o.set_rotation(a.handle(0)?, v3(a, 1)?)?;
        done()
    }),
    method!(Scene3D, "SetRotationQuaternion", [node: Handle, x: Num, y: Num, z: Num, w: Num], [], Nothing, "Rotation as a quaternion.", |o, r, a| {
        o.set_rotation_quaternion(a.handle(0)?, [a.f32(1)?, a.f32(2)?, a.f32(3)?, a.f32(4)?])?;
        done()
    }),
    method!(Scene3D, "SetScale", [node: Handle, x: Num, y: Num, z: Num], [], Nothing, "Scale on each axis (zero is refused).", |o, r, a| {
        o.set_scale(a.handle(0)?, v3(a, 1)?)?;
        done()
    }),
    method!(Scene3D, "LookAt", [node: Handle, target_x: Num, target_y: Num, target_z: Num], [up_x: Num, up_y: Num, up_z: Num], Nothing, "Turn a node so its -Z axis points at a target (up defaults to +Y).", |o, r, a| {
        let up = if a.has(4) { v3(a, 4)? } else { [0.0, 1.0, 0.0] };
        o.look_at(a.handle(0)?, v3(a, 1)?, up)?;
        done()
    }),
    method!(Scene3D, "GetPosition", [node: Handle], [], Floats, "x, y, z relative to the parent.", |o, r, a| Ok(o.position(a.handle(0)?)?.to_vec().into())),
    method!(Scene3D, "GetRotation", [node: Handle], [], Floats, "Euler degrees as SetRotation takes them.", |o, r, a| Ok(o.rotation(a.handle(0)?)?.to_vec().into())),
    method!(Scene3D, "GetRotationQuaternion", [node: Handle], [], Floats, "x, y, z, w.", |o, r, a| Ok(o.rotation_quaternion(a.handle(0)?)?.to_vec().into())),
    method!(Scene3D, "GetScale", [node: Handle], [], Floats, "x, y, z.", |o, r, a| Ok(o.scale(a.handle(0)?)?.to_vec().into())),
    method!(Scene3D, "GetWorldPosition", [node: Handle], [], Floats, "x, y, z in the scene (not relative to the parent).", |o, r, a| Ok(o.world_position(a.handle(0)?)?.to_vec().into())),
    // ---- materials -----------------------------------------------------------------------
    method!(Scene3D, "CreateMaterial", [], [], Handle, "A new material: white, roughness 0.5, not metallic.", |o, r, a| Ok(o.create_material()?.into())),
    method!(Scene3D, "ReleaseMaterial", [material: Handle], [], Nothing, "Release a material; nodes that used it fall back to the default.", |o, r, a| {
        o.release_material(a.handle(0)?)?;
        done()
    }),
    method!(Scene3D, "SetMaterial", [node: Handle], [material: Handle], Nothing, "Draw a node with this material (omit it, or give 0, for the default).", |o, r, a| {
        o.set_material(a.handle(0)?, a.opt_handle(1)?)?;
        done()
    }),
    method!(Scene3D, "GetMaterial", [node: Handle], [], Handle, "The material a mesh node uses; a node that had none is given a new one.", |o, r, a| Ok(o.material_of(a.handle(0)?)?.into())),
    method!(Scene3D, "SetColor", [node: Handle, color: Color], [], Nothing, "Shortcut: the base colour (and opacity) of a mesh node's own material.", |o, r, a| {
        let m = o.material_of(a.handle(0)?)?;
        let c = a.color(1)?;
        o.set_color(m, [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, c.a as f32 / 255.0])?;
        done()
    }),
    method!(Scene3D, "SetMaterialColor", [material: Handle, color: Color], [], Nothing, "A material's base colour and opacity.", |o, r, a| {
        let c = a.color(1)?;
        o.set_color(a.handle(0)?, [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, c.a as f32 / 255.0])?;
        done()
    }),
    method!(Scene3D, "SetMetallic", [material: Handle, value: Num], [], Nothing, "0 for plastic or wood, 1 for a metal.", |o, r, a| {
        o.set_metallic(a.handle(0)?, a.f32(1)?)?;
        done()
    }),
    method!(Scene3D, "SetRoughness", [material: Handle, value: Num], [], Nothing, "0 mirror-smooth to 1 fully rough.", |o, r, a| {
        o.set_roughness(a.handle(0)?, a.f32(1)?)?;
        done()
    }),
    method!(Scene3D, "SetEmissive", [material: Handle, red: Num, green: Num, blue: Num], [], Nothing, "Light the surface gives off.", |o, r, a| {
        o.set_emissive(a.handle(0)?, v3(a, 1)?)?;
        done()
    }),
    method!(Scene3D, "SetTexture", [material: Handle, slot: Int], [image: Handle], Nothing, "An ImageBuffer on a slot (0 base colour, 1 metallic-roughness, 2 emissive, 3 normal); omit the image, or give 0, to clear the slot. The pixels are shared, not copied.", |o, r, a| {
        let slot = TextureSlot::from_code(a.int(1)?).ok_or_else(|| a.refuse(1, "is not a texture slot (0 base colour, 1 metallic-roughness, 2 emissive, 3 normal)"))?;
        let px = match a.opt_handle(2)? {
            Some(h) => Some(r.image_pixels(h)?),
            None => None,
        };
        o.set_texture(a.handle(0)?, slot, px)?;
        done()
    }),
    method!(Scene3D, "SetAlphaMode", [material: Handle, mode: Int], [cutoff: Num], Nothing, "0 opaque, 1 mask (pixels under the cutoff are dropped), 2 blend.", |o, r, a| {
        let mode = AlphaMode::from_code(a.int(1)?).ok_or_else(|| a.refuse(1, "is not an alpha mode (0 opaque, 1 mask, 2 blend)"))?;
        o.set_alpha_mode(a.handle(0)?, mode, a.num_or(2, 0.5)? as f32)?;
        done()
    }),
    method!(Scene3D, "SetDoubleSided", [material: Handle, on: Flag], [], Nothing, "Draw both sides of every triangle.", |o, r, a| {
        o.set_double_sided(a.handle(0)?, a.flag(1)?)?;
        done()
    }),
    method!(Scene3D, "SetUnlit", [material: Handle, on: Flag], [], Nothing, "Ignore the lights: draw the colour as it is.", |o, r, a| {
        o.set_unlit(a.handle(0)?, a.flag(1)?)?;
        done()
    }),
    // ---- instancing and animation -----------------------------------------------------------
    method!(Scene3D, "SetInstances", [node: Handle, matrices: Floats], [], Nothing, "Draw the node's mesh once per matrix (sixteen numbers each, column-major with the translation in the last column); an empty list goes back to one copy.", |o, r, a| {
        o.set_instances(a.handle(0)?, &a.floats32(1)?)?;
        done()
    }),
    method!(Scene3D, "GetInstanceCount", [node: Handle], [], Int, "How many copies a node draws.", |o, r, a| Ok(o.instance_count(a.handle(0)?)?.into())),
    method!(Scene3D, "GetAnimations", [], [], Ints, "Every animation.", |o, r, a| Ok(o.animations().into())),
    method!(Scene3D, "FindAnimation", [name: Text], [], Handle, "An animation by name (0 if none).", |o, r, a| Ok(o.find_animation(a.text(0)?).unwrap_or(0).into())),
    method!(Scene3D, "GetAnimationName", [animation: Handle], [], Text, "The name an animation has in its model.", |o, r, a| Ok(o.animation_name(a.handle(0)?)?.into())),
    method!(Scene3D, "GetAnimationDuration", [animation: Handle], [], Num, "Seconds.", |o, r, a| Ok(o.animation_duration(a.handle(0)?)?.into())),
    method!(Scene3D, "GetAnimationTime", [animation: Handle], [], Num, "The current time in seconds.", |o, r, a| Ok(o.animation_time(a.handle(0)?)?.into())),
    method!(Scene3D, "IsAnimationPlaying", [animation: Handle], [], Int, "1 if Update advances it.", |o, r, a| Ok(o.is_animation_playing(a.handle(0)?)?.into())),
    method!(Scene3D, "PlayAnimation", [animation: Handle], [loop_it: Flag, speed: Num], Nothing, "Start (or resume) an animation; it loops by default; speed 1 is real time, negative runs backwards.", |o, r, a| {
        o.play_animation(a.handle(0)?, if a.has(1) { a.flag(1)? } else { true }, a.num_or(2, 1.0)? as f32)?;
        done()
    }),
    method!(Scene3D, "StopAnimation", [animation: Handle], [], Nothing, "Pause an animation where it is.", |o, r, a| {
        o.stop_animation(a.handle(0)?)?;
        done()
    }),
    method!(Scene3D, "SetAnimationTime", [animation: Handle, seconds: Num], [], Nothing, "Jump to a time and show that pose.", |o, r, a| {
        o.set_animation_time(a.handle(0)?, a.f32(1)?)?;
        done()
    }),
    method!(Scene3D, "Update", [milliseconds: Num], [], Nothing, "Advance every playing animation by the elapsed milliseconds (what onFrame gives).", |o, r, a| {
        o.update(a.f32(0)? / 1000.0)?;
        done()
    }),
    // ---- reading ----------------------------------------------------------------------------
    method!(Scene3D, "GetNodeCount", [], [], Int, "How many nodes.", |o, r, a| Ok(o.node_count().into())),
    method!(Scene3D, "GetTriangleCount", [], [], Int, "How many triangles the meshes hold (instances not counted).", |o, r, a| Ok((o.triangle_count() as i64).into())),
    method!(Scene3D, "GetBounds", [], [], Floats, "valid (0 for an empty scene), then min x y z and max x y z of everything drawn.", |o, r, a| {
        let b = o.bounds();
        Ok(vec![b.valid as u8 as f64, b.min[0] as f64, b.min[1] as f64, b.min[2] as f64, b.max[0] as f64, b.max[1] as f64, b.max[2] as f64].into())
    }),
];

pub(crate) fn call(o: &mut Scene3D, reg: &mut Registry, method: &str, args: &[Value]) -> Result<Value> {
    dispatch(METHODS, "Scene3D", o, reg, method, args)
}

pub(crate) fn infos() -> Vec<MethodInfo> {
    super::infos(ObjectKind::Scene3D, METHODS)
}
