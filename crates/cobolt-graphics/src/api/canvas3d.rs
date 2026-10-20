// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The methods of Canvas3D (WASM-018 R9).

use super::{dispatch, done, method};
use crate::error::{GraphicsError, Result};
use crate::registry::{MethodInfo, ObjectKind, Registry};
use crate::scene3d::{Backend, Canvas3D};
use crate::value::Value;

/// Connect a canvas to the registry's shared graphics device, opening one if there is none yet.
fn connect(o: &mut Canvas3D, r: &mut Registry) -> Backend {
    if let Some(g) = r.gpu().cloned() {
        return o.connect_with(g);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let b = o.connect();
        if let Some(g) = o.renderer().map(|rd| rd.gpu().clone()) {
            r.set_gpu(g);
        }
        b
    }
    #[cfg(target_arch = "wasm32")]
    {
        // In the browser the player opens the device asynchronously and hands it over with `Registry::set_gpu`.
        Backend::None
    }
}

fn scene_of(o: &Canvas3D) -> Result<u32> {
    o.scene().ok_or_else(|| GraphicsError::invalid("the canvas has no scene yet: call SetScene first"))
}

#[allow(unused_variables)]
pub(crate) static METHODS: &[crate::value::Method<Canvas3D>] = &[
    method!(Canvas3D, "SetScene", [scene: Handle], [], Nothing, "The Scene3D this canvas shows.", |o, r, a| {
        let scene = a.handle(0)?;
        if r.kind_of(scene)? != ObjectKind::Scene3D {
            return Err(a.refuse(0, "is not a Scene3D"));
        }
        o.set_scene(Some(scene));
        done()
    }),
    method!(Canvas3D, "SetCamera", [], [camera: Handle], Nothing, "Look through this camera node (omit it, or give 0, for the scene's first camera, or one that frames everything).", |o, r, a| {
        o.set_camera(a.opt_handle(0)?);
        done()
    }),
    method!(Canvas3D, "Connect", [], [], Text, "Open the graphics device now (Render does it on first use); the answer is the Backend.", |o, r, a| Ok(connect(o, r).name().into())),
    method!(Canvas3D, "GetBackend", [], [], Text, "What runs the picture: WebGPU, WebGL2, Vulkan, Metal, Dx12, OpenGL, Software, or None.", |o, r, a| Ok(o.backend().name().into())),
    method!(Canvas3D, "GetAdapter", [], [], Text, "The graphics adapter's name and driver, when connected.", |o, r, a| Ok(o.adapter_description().unwrap_or_default().into())),
    method!(Canvas3D, "GetStatus", [], [], Text, "The readable message to show instead of the picture when 3D is not available (empty when it is).", |o, r, a| Ok(o.status().unwrap_or("").into())),
    method!(Canvas3D, "TakeEvent", [], [], Text, "The next message the control should raise as onError (empty when there is none).", |o, r, a| Ok(o.take_event().unwrap_or_default().into())),
    method!(Canvas3D, "Resize", [width: Int, height: Int], [], Nothing, "New size in pixels.", |o, r, a| {
        o.resize(a.u32(0)?, a.u32(1)?)?;
        done()
    }),
    method!(Canvas3D, "GetWidth", [], [], Int, "Width in pixels.", |o, r, a| Ok(o.size().0.into())),
    method!(Canvas3D, "GetHeight", [], [], Int, "Height in pixels.", |o, r, a| Ok(o.size().1.into())),
    method!(Canvas3D, "SetMsaa", [samples: Int], [], Nothing, "1 (off) or 4 samples per pixel.", |o, r, a| {
        o.set_msaa(a.u32(0)?)?;
        done()
    }),
    method!(Canvas3D, "GetMsaa", [], [], Int, "Samples per pixel.", |o, r, a| Ok(o.msaa().into())),
    method!(Canvas3D, "SetShadowMapSize", [texels: Int], [], Nothing, "The edge of each shadow map, 256 to 8192 (takes effect at the next Connect).", |o, r, a| {
        o.set_shadow_map_size(a.u32(0)?)?;
        done()
    }),
    method!(Canvas3D, "Render", [], [], Int, "Draw the scene from the camera; the answer is the number of draw calls.", |o, r, a| {
        let scene = scene_of(o)?;
        if o.backend() == Backend::None && o.status().is_none() {
            connect(o, r);
        }
        Ok(r.with_scene3d(scene, |s, _| o.render(s))?.draw_calls.into())
    }),
    method!(Canvas3D, "Screenshot", [], [], Handle, "The last picture as a new ImageBuffer.", |o, r, a| {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Ok(r.add_image(o.screenshot()?)?.into())
        }
        #[cfg(target_arch = "wasm32")]
        {
            Err(GraphicsError::Unsupported("Screenshot blocks, which the browser cannot; the player reads the picture asynchronously".into()))
        }
    }),
    method!(Canvas3D, "Pick", [x: Num, y: Num], [], Floats, "The node under a pixel: node (0 when none), instance, distance, point x y z, normal x y z.", |o, r, a| {
        let scene = scene_of(o)?;
        let (x, y) = (a.f32(0)?, a.f32(1)?);
        Ok(match r.with_scene3d(scene, |s, _| o.pick(s, x, y))? {
            Some(h) => vec![h.node as f64, h.instance as f64, h.distance as f64, h.point[0] as f64, h.point[1] as f64, h.point[2] as f64, h.normal[0] as f64, h.normal[1] as f64, h.normal[2] as f64].into(),
            None => vec![0.0; 9].into(),
        })
    }),
    method!(Canvas3D, "GetStats", [], [], Floats, "What the last frame held and drew: mesh nodes, instances, instances drawn, instances culled, triangles drawn, lights, shadow maps, skinned meshes, draw calls, shadow draw calls.", |o, r, a| {
        let f = o.last_frame_stats().unwrap_or_default();
        let g = o.renderer().map(|rd| rd.last_stats()).unwrap_or_default();
        Ok(vec![f.mesh_nodes as f64, f.instances_total as f64, f.instances_drawn as f64, f.instances_culled as f64, f.triangles_drawn as f64, f.lights as f64, f.shadow_maps as f64, f.skinned as f64, g.draw_calls as f64, g.shadow_draw_calls as f64].into())
    }),
];

pub(crate) fn call(o: &mut Canvas3D, reg: &mut Registry, method: &str, args: &[Value]) -> Result<Value> {
    dispatch(METHODS, "Canvas3D", o, reg, method, args)
}

pub(crate) fn infos() -> Vec<MethodInfo> {
    super::infos(ObjectKind::Canvas3D, METHODS)
}
