// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Canvas2D on the desktop host (WASM-018 T18, R16): the picture a `Present` left in
//! `cobolt_runtime::canvas_frames` becomes **one texture per canvas**, updated in place with `set`
//! (never re-created), kept in the egui context under `cobolt_forms::paint::canvas_texture_key` so the
//! two painters (the run form and the faces) draw it.

use cobolt_runtime::canvas_frames::{self, FRAME_PROP};
use cobolt_runtime::{gpu_share, StateUpdate};

/// What the host remembers per Canvas3D: the egui id of its registered target and the texture behind it
/// (a target made again - the canvas resized - is registered again under the same id).
#[derive(Clone)]
struct NativeTarget {
    id: egui::TextureId,
    texture: eframe::wgpu::Texture,
}

/// Where the window's render state is kept for the state-drain sites, which have only the context.
fn render_state_key() -> egui::Id {
    egui::Id::new("wasm018-render-state")
}

/// Canvas3D draws on the window's own device (R16): make it available to the interpreter. Called every
/// frame; it acts once per device (a window whose device was replaced publishes the new one, and the
/// interpreter connects its 3D views to it).
pub(crate) fn share_window_device(ctx: &egui::Context, frame: &eframe::Frame) {
    let Some(rs) = frame.wgpu_render_state() else { return };
    let same = ctx.data(|d| d.get_temp::<eframe::egui_wgpu::RenderState>(render_state_key())).is_some_and(|old| old.device == rs.device);
    if same {
        return;
    }
    gpu_share::publish_shared_device(rs.device.clone(), rs.queue.clone(), &rs.adapter.get_info());
    ctx.data_mut(|d| d.insert_temp(render_state_key(), rs.clone()));
}

/// If `u` is a `_Canvas3DFrame` update, register the canvas's render target with egui as a native texture
/// (no copy, no read-back) so the painters draw it. Returns whether it was one.
pub(crate) fn apply_canvas3d_frame(ctx: &egui::Context, u: &StateUpdate) -> bool {
    if u.prop != gpu_share::FRAME_PROP {
        return false;
    }
    let Some(frame) = gpu_share::take_frame(&u.ctrl_id, &u.value) else { return true };
    let Some(rs) = ctx.data(|d| d.get_temp::<eframe::egui_wgpu::RenderState>(render_state_key())) else { return true };
    let mut renderer = rs.renderer.write();
    register_target(ctx, &rs.device, &mut renderer, &u.ctrl_id, &frame);
    true
}

/// Register (or update, when the canvas's target was made again) a Canvas3D's render target as an egui
/// native texture, and leave its id where the painters look. The same id for the life of the canvas.
fn register_target(ctx: &egui::Context, device: &eframe::wgpu::Device, renderer: &mut eframe::egui_wgpu::Renderer, canvas: &str, frame: &gpu_share::Canvas3dFrame) {
    let key = egui::Id::new(("wasm018-canvas3d-target", canvas.trim().to_ascii_uppercase()));
    let view = frame.texture.create_view(&eframe::wgpu::TextureViewDescriptor::default());
    let known = ctx.data(|d| d.get_temp::<NativeTarget>(key));
    let id = match known {
        Some(t) if t.texture == frame.texture => t.id,
        Some(t) => {
            renderer.update_egui_texture_from_wgpu_texture(device, &view, eframe::wgpu::FilterMode::Linear, t.id);
            t.id
        }
        None => renderer.register_native_texture(device, &view, eframe::wgpu::FilterMode::Linear),
    };
    ctx.data_mut(|d| {
        d.insert_temp(key, NativeTarget { id, texture: frame.texture.clone() });
        d.insert_temp(cobolt_forms::paint::canvas_native_texture_key(canvas), id);
    });
    ctx.request_repaint();
}

/// If `u` is a `_CanvasFrame` update, take its picture and update the canvas's texture. Returns whether it was one.
pub(crate) fn apply_canvas_frame(ctx: &egui::Context, u: &StateUpdate) -> bool {
    if u.prop != FRAME_PROP {
        return false;
    }
    let Some(frame) = canvas_frames::take(&u.ctrl_id, &u.value) else { return true };
    let image = egui::ColorImage::from_rgba_premultiplied([frame.width as usize, frame.height as usize], &frame.rgba_premultiplied);
    let key = cobolt_forms::paint::canvas_texture_key(&u.ctrl_id);
    let options = egui::TextureOptions::LINEAR;
    match ctx.data(|d| d.get_temp::<egui::TextureHandle>(key)) {
        Some(mut handle) => {
            handle.set(image, options);
            // the handle is a shared reference to the same texture; storing it again keeps the key current
            ctx.data_mut(|d| d.insert_temp(key, handle));
        }
        None => {
            let handle = ctx.load_texture(format!("canvas2d:{}", u.ctrl_id.trim().to_ascii_uppercase()), image, options);
            ctx.data_mut(|d| d.insert_temp(key, handle));
        }
    }
    ctx.request_repaint();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_runtime::canvas_frames::{next_instance, publish, token, CanvasFrame};

    fn px(w: u32, h: u32, v: u8) -> CanvasFrame {
        CanvasFrame { width: w, height: h, rgba_premultiplied: vec![v; (w * h * 4) as usize], generation: 0 }
    }

    /// One texture per canvas, updated in place: the same id after a second Present, a new size handled, and
    /// an update of another property ignored.
    #[test]
    fn a_present_updates_one_texture_per_canvas_in_place() {
        let ctx = egui::Context::default();
        let inst = next_instance();
        publish(inst, "CANVAS-1", px(4, 4, 10));
        assert!(apply_canvas_frame(&ctx, &StateUpdate::new("CANVAS-1", FRAME_PROP, token(inst, 1))));
        let first = cobolt_forms::paint::canvas_texture(&ctx, "canvas-1").expect("a texture");
        publish(inst, "CANVAS-1", px(4, 4, 200));
        apply_canvas_frame(&ctx, &StateUpdate::new("CANVAS-1", FRAME_PROP, token(inst, 2)));
        let second = cobolt_forms::paint::canvas_texture(&ctx, "CANVAS-1").expect("a texture");
        println!("T18: Canvas2D texture {first:?} after the first Present, {second:?} after the second (same: {})", first == second);
        assert_eq!(first, second, "updated with `set`, not re-created");
        assert!(!apply_canvas_frame(&ctx, &StateUpdate::new("CANVAS-1", "Caption", "x")));
        canvas_frames::forget(inst);
    }

    /// WASM-018 T18 on a real adapter, without a window: the device the "window" owns is published, a
    /// Canvas3D draws on it, its render target is registered with egui as a native texture - the same id after
    /// a second Render - and the picture it shows is the one the canvas drew. With no adapter it SKIPS loudly
    /// (`GRAPHICS_REQUIRE_GPU=1` makes that a failure).
    #[test]
    fn a_canvas3d_draws_on_the_window_device_and_egui_shows_its_target() {
        use cobolt_graphics::scene3d::{Canvas3D, Scene3D};
        use cobolt_graphics::{Limits, Rgba};
        use eframe::wgpu;

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())) else {
            assert!(std::env::var("GRAPHICS_REQUIRE_GPU").map(|v| v != "1").unwrap_or(true), "GRAPHICS_REQUIRE_GPU=1 but there is no graphics adapter");
            eprintln!("SKIPPED (no graphics adapter)");
            return;
        };
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let mut egui_renderer = eframe::egui_wgpu::Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, Default::default());

        // The host publishes the window's device; the interpreter's registry picks it up from here.
        let started = std::time::Instant::now();
        gpu_share::publish_shared_device(device.clone(), queue.clone(), &info);
        let (generation, gpu) = gpu_share::current_gpu().expect("a published device");
        let mut canvas = Canvas3D::new(96, 72, Limits::default()).unwrap();
        let backend = canvas.connect_with(gpu);
        assert_ne!(backend, cobolt_graphics::scene3d::Backend::None, "{:?}", canvas.status());

        let mut scene = Scene3D::new(Limits::default());
        let cam = scene.add_perspective_camera(40.0, 0.1, 200.0).unwrap();
        scene.set_position(cam, [0.0, 0.0, 6.0]).unwrap();
        scene.look_at(cam, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
        canvas.set_camera(Some(cam));
        let ball = scene.add_sphere(1.5, 32).unwrap();
        let m = scene.material_of(ball).unwrap();
        scene.set_color(m, [0.9, 0.2, 0.2, 1.0]).unwrap();
        scene.set_ambient([1.0; 3], 1.0).unwrap();
        scene.set_background(Rgba::rgb(0, 0, 0));
        canvas.render(&mut scene).unwrap();

        let ctx = egui::Context::default();
        let inst = next_instance();
        let share = |canvas: &Canvas3D, generation: u64| {
            let texture = canvas.renderer().and_then(|r| r.color_texture().cloned()).expect("a render target");
            let (w, h) = canvas.size();
            gpu_share::publish_frame(inst, "VIEW-1", gpu_share::Canvas3dFrame { texture, width: w, height: h, generation });
            gpu_share::take_frame("VIEW-1", &format!("{inst}:{generation}")).expect("the frame")
        };
        let frame = share(&canvas, 1);
        register_target(&ctx, &device, &mut egui_renderer, "VIEW-1", &frame);
        let first = cobolt_forms::paint::canvas_texture(&ctx, "view-1").expect("a native texture");
        canvas.render(&mut scene).unwrap();
        let frame = share(&canvas, 2);
        register_target(&ctx, &device, &mut egui_renderer, "VIEW-1", &frame);
        let second = cobolt_forms::paint::canvas_texture(&ctx, "VIEW-1").expect("a native texture");
        let shown = canvas.screenshot().unwrap();
        let centre = shown.get(48, 36).unwrap();
        let corner = shown.get(2, 2).unwrap();
        gpu_share::forget_frames(inst);
        println!(
            "T18 Canvas3D: device published (generation {generation}) on {} / {}; target registered as {first:?}, after a second Render {second:?} (same: {}); sphere pixel r={} g={} b={}, background pixel r={}; {} ms",
            info.name,
            canvas.backend().name(),
            first == second,
            centre.r,
            centre.g,
            centre.b,
            corner.r,
            started.elapsed().as_millis()
        );
        assert_eq!(first, second, "the same egui texture for the life of the canvas");
        assert!(centre.r > 100 && centre.r > centre.g, "the sphere is red where the canvas drew it: {centre:?}");
        assert_eq!(corner.r, 0, "the background is the clear colour");
        gpu_share::withdraw_gpu();
    }
}
