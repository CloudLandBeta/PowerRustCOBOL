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
use cobolt_runtime::StateUpdate;

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
        let first = cobolt_forms::paint::canvas_texture(&ctx, "canvas-1").expect("a texture").id();
        publish(inst, "CANVAS-1", px(4, 4, 200));
        apply_canvas_frame(&ctx, &StateUpdate::new("CANVAS-1", FRAME_PROP, token(inst, 2)));
        let second = cobolt_forms::paint::canvas_texture(&ctx, "CANVAS-1").expect("a texture").id();
        println!("T18: Canvas2D texture {first:?} after the first Present, {second:?} after the second (same: {})", first == second);
        assert_eq!(first, second, "updated with `set`, not re-created");
        assert!(!apply_canvas_frame(&ctx, &StateUpdate::new("CANVAS-1", "Caption", "x")));
        canvas_frames::forget(inst);
    }
}
