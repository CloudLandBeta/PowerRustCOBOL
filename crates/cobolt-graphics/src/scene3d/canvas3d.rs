// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Canvas3D: the control that shows a [`Scene3D`] (WASM-018 R9).
//!
//! A [`Canvas3D`] owns a size, a multisampling setting, the camera to look
//! through and - once connected - a [`Renderer`]. `SetScene` is the registry's
//! business (it passes the scene to [`Canvas3D::render`] and
//! [`Canvas3D::pick`]); this type works on whatever scene it is handed.
//!
//! **No adapter, no crash.** [`Canvas3D::connect`] on a machine with no usable
//! graphics adapter leaves the canvas in a state where [`Canvas3D::backend`] is
//! [`Backend::None`], [`Canvas3D::status`] holds a readable message, one event is
//! waiting in [`Canvas3D::take_event`] (the host raises `onError` from it), and
//! [`Canvas3D::render`] returns an error instead of panicking. A device lost
//! mid-run lands in the same state.

use super::frame::FrameStats;
use super::gpu::{Backend, Gpu, RenderStats, Renderer, RendererConfig};
use super::pick::PickHit;
use super::scene::Scene3D;
use crate::error::{GraphicsError, Result};
use crate::handles::Handle;
use crate::limits::Limits;
use crate::pixels::PixelBuffer;

/// A 3D view.
pub struct Canvas3D {
    width: u32,
    height: u32,
    msaa: u32,
    camera: Option<Handle>,
    limits: Limits,
    config: RendererConfig,
    renderer: Option<Renderer>,
    status: Option<String>,
    events: Vec<String>,
    last_frame: Option<FrameStats>,
}

impl std::fmt::Debug for Canvas3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Canvas3D({}x{}, {})", self.width, self.height, self.backend().name())
    }
}

impl Canvas3D {
    /// A canvas of this size (checked against the limits), not yet connected to a graphics device.
    pub fn new(width: u32, height: u32, limits: Limits) -> Result<Canvas3D> {
        limits.check_canvas(width, height, 1.0)?;
        Ok(Canvas3D { width, height, msaa: 4, camera: None, limits, config: RendererConfig::default(), renderer: None, status: None, events: Vec::new(), last_frame: None })
    }

    /// Use shadow maps of this edge (256 to 8192 texels). Takes effect at the next connect.
    pub fn set_shadow_map_size(&mut self, texels: u32) -> Result<()> {
        if !(256..=8192).contains(&texels) {
            return Err(GraphicsError::invalid("a shadow map is from 256 to 8192 texels on a side"));
        }
        self.limits.check_texture(texels, texels)?;
        self.config.shadow_map_size = texels;
        Ok(())
    }

    /// Open the best graphics adapter and set the renderer up (blocking; on `wasm32` use [`Canvas3D::connect_with`]).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn connect(&mut self) -> Backend {
        match Gpu::new() {
            Ok(g) => self.connect_with(g),
            Err(e) => self.fail(format!("3D cannot be drawn on this machine: {e}")),
        }
    }

    /// Use a graphics device the host opened (shared by several canvases, or made asynchronously in the browser).
    pub fn connect_with(&mut self, gpu: Gpu) -> Backend {
        match Renderer::new(gpu, self.config) {
            Ok(r) => {
                let b = r.gpu().info().backend;
                self.renderer = Some(r);
                self.status = None;
                b
            }
            Err(e) => self.fail(format!("3D cannot be drawn: {e}")),
        }
    }

    fn fail(&mut self, message: String) -> Backend {
        self.renderer = None;
        self.events.push(message.clone());
        self.status = Some(message);
        Backend::None
    }

    /// `Backend`: what runs the picture (`None` before connecting or when there is no adapter).
    pub fn backend(&self) -> Backend {
        self.renderer.as_ref().map(|r| r.gpu().info().backend).unwrap_or(Backend::None)
    }

    /// The adapter's name and driver, when connected.
    pub fn adapter_description(&self) -> Option<String> {
        self.renderer.as_ref().map(|r| {
            let i = r.gpu().info();
            format!("{} ({}) {}", i.adapter, i.backend.name(), i.driver).trim().to_string()
        })
    }

    /// The readable message to show instead of the picture, when 3D is not available.
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// A message the host should raise as an event (`onError`), oldest first.
    pub fn take_event(&mut self) -> Option<String> {
        if self.events.is_empty() {
            None
        } else {
            Some(self.events.remove(0))
        }
    }

    /// The renderer, for a host that shares the device.
    pub fn renderer(&self) -> Option<&Renderer> {
        self.renderer.as_ref()
    }

    /// `SetCamera`: look through this camera node (`None`: the scene's first camera, or one that frames everything).
    pub fn set_camera(&mut self, camera: Option<Handle>) {
        self.camera = camera;
    }

    /// The camera node in use.
    pub fn camera(&self) -> Option<Handle> {
        self.camera
    }

    /// `Msaa`: 1 (off) or 4 samples per pixel.
    pub fn set_msaa(&mut self, samples: u32) -> Result<()> {
        if !matches!(samples, 1 | 4) {
            return Err(GraphicsError::invalid("Msaa is 1 (off) or 4"));
        }
        self.msaa = samples;
        Ok(())
    }

    /// The multisampling in use.
    pub fn msaa(&self) -> u32 {
        self.msaa
    }

    /// `Resize` (logical pixels; checked against the limits).
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        self.limits.check_canvas(width, height, 1.0)?;
        self.width = width;
        self.height = height;
        Ok(())
    }

    /// The size.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// `Render`: draw the scene from the current camera.
    pub fn render(&mut self, scene: &mut Scene3D) -> Result<RenderStats> {
        let (w, h) = (self.width, self.height);
        let frame = scene.build_frame(self.camera, w as f32 / h as f32)?;
        self.last_frame = Some(frame.stats);
        let Some(r) = self.renderer.as_mut() else {
            return Err(GraphicsError::NoAdapter(self.status.clone().unwrap_or_else(|| "the canvas is not connected to a graphics device".into())));
        };
        match r.render(&frame, w, h, self.msaa) {
            Ok(s) => Ok(s),
            Err(e) => {
                if r.gpu().is_lost() {
                    let msg = format!("the graphics device was lost: {e}");
                    self.fail(msg);
                }
                Err(e)
            }
        }
    }

    /// What the last frame contained (culling and light counts).
    pub fn last_frame_stats(&self) -> Option<FrameStats> {
        self.last_frame
    }

    /// `Screenshot`: the last picture, as straight-alpha RGBA (blocking; use [`Renderer::read_pixels_async`] on `wasm32`).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn screenshot(&self) -> Result<PixelBuffer> {
        self.renderer.as_ref().ok_or_else(|| GraphicsError::NoAdapter(self.status.clone().unwrap_or_else(|| "the canvas is not connected to a graphics device".into())))?.read_pixels()
    }

    /// `Pick`: the node under a pixel of the canvas (needs no graphics device).
    pub fn pick(&self, scene: &mut Scene3D, x: f32, y: f32) -> Result<Option<PickHit>> {
        scene.pick(self.camera, self.width, self.height, x, y)
    }
}
