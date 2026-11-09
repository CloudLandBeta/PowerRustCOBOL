// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Canvas3D on the desktop host (WASM-018 T18, R16): the device the window draws with, and the
//! pictures a `Render` leaves for it.
//!
//! The form host owns the window's `wgpu` device (`eframe::Frame::wgpu_render_state()`). It publishes a
//! [`Gpu`] made from it with [`publish_gpu`]; the interpreter hands that to its registry, so every
//! Canvas3D draws on the **same device** the window shows. A `Render` leaves the render target here
//! ([`publish_frame`]) and sends the host a `_Canvas3DFrame` update; the host registers the target with
//! egui as a native texture. Nothing is read back on the frame path.
//!
//! A new [`publish_gpu`] (the window's device was replaced) bumps the generation: the interpreter sees
//! it, re-creates the renderers on the new device and reports it.

use cobolt_graphics::scene3d::Gpu;
use cobolt_graphics::wgpu;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// The property a `Render` writes (the value is `<instance>:<generation>`).
pub const FRAME_PROP: &str = "_Canvas3DFrame";

fn gpu_slot() -> &'static Mutex<Option<(u64, Gpu)>> {
    static SLOT: OnceLock<Mutex<Option<(u64, Gpu)>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// The window's device, from the host. Replaces an earlier one (the generation grows).
pub fn publish_gpu(gpu: Gpu) {
    if let Ok(mut s) = gpu_slot().lock() {
        let next = s.as_ref().map(|(g, _)| g + 1).unwrap_or(1);
        *s = Some((next, gpu));
    }
}

/// The window's device and its generation, when the host published one.
pub fn current_gpu() -> Option<(u64, Gpu)> {
    gpu_slot().lock().ok().and_then(|s| s.clone())
}

/// The window is gone: the device with it.
pub fn withdraw_gpu() {
    if let Ok(mut s) = gpu_slot().lock() {
        *s = None;
    }
}

/// One rendered picture: the renderer's target (shared, not copied).
#[derive(Debug)]
pub struct Canvas3dFrame {
    pub texture: wgpu::Texture,
    pub width: u32,
    pub height: u32,
    pub generation: u64,
}

type Slots = HashMap<(u64, String), Arc<Canvas3dFrame>>;

fn slots() -> &'static Mutex<Slots> {
    static SLOTS: OnceLock<Mutex<Slots>> = OnceLock::new();
    SLOTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Leave a canvas's render target for the host.
pub fn publish_frame(instance: u64, canvas: &str, frame: Canvas3dFrame) {
    if let Ok(mut s) = slots().lock() {
        s.insert((instance, canvas.trim().to_ascii_uppercase()), Arc::new(frame));
    }
}

/// Take the target a `_Canvas3DFrame` update announced.
pub fn take_frame(canvas: &str, token: &str) -> Option<Arc<Canvas3dFrame>> {
    let (inst, _gen) = token.trim().split_once(':')?;
    let instance: u64 = inst.parse().ok()?;
    slots().lock().ok()?.get(&(instance, canvas.trim().to_ascii_uppercase())).cloned()
}

/// A form's end: its targets go.
pub fn forget_frames(instance: u64) {
    if let Ok(mut s) = slots().lock() {
        s.retain(|(i, _), _| *i != instance);
    }
}

/// Publish the window's device from the pieces eframe holds (`RenderState::{device, queue, adapter}`).
pub fn publish_shared_device(device: wgpu::Device, queue: wgpu::Queue, adapter: &wgpu::AdapterInfo) {
    publish_gpu(Gpu::from_shared(device, queue, adapter).with_deferred_submits());
}

/// The window's thread submits what the 3D views drew since the last call (the program draws on its own
/// thread, and a submission from there while the window reconfigures its surface is fatal). Answers how many.
pub fn submit_pending() -> usize {
    current_gpu().map_or(0, |(_, gpu)| gpu.submit_deferred())
}
