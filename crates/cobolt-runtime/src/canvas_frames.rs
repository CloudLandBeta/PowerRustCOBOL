// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Canvas2D pictures from the interpreter to the form host (WASM-018 T18, R16).
//!
//! The interpreter and the host talk in strings (`StateUpdate`), and a picture is bytes. So a `Present`
//! leaves the picture here - one slot per canvas, the newest wins - and sends the host a
//! `_CanvasFrame` update whose value names the slot (`<instance>:<generation>`). The host takes the
//! picture, updates one texture per canvas, and paints it. Nothing is read back on the frame path.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// The property a `Present` writes (the value is `<instance>:<generation>`).
pub const FRAME_PROP: &str = "_CanvasFrame";

/// One presented picture, premultiplied RGBA, device pixels.
#[derive(Debug)]
pub struct CanvasFrame {
    pub width: u32,
    pub height: u32,
    pub rgba_premultiplied: Vec<u8>,
    pub generation: u64,
}

type Slots = HashMap<(u64, String), Arc<CanvasFrame>>;

fn slots() -> &'static Mutex<Slots> {
    static SLOTS: OnceLock<Mutex<Slots>> = OnceLock::new();
    SLOTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A new instance number: one per running form's graphics.
pub fn next_instance() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Leave a canvas's picture for the host. The previous one, if the host never took it, is replaced.
pub fn publish(instance: u64, canvas: &str, frame: CanvasFrame) {
    if let Ok(mut s) = slots().lock() {
        s.insert((instance, canvas.trim().to_ascii_uppercase()), Arc::new(frame));
    }
}

/// The value of the `_CanvasFrame` update.
pub fn token(instance: u64, generation: u64) -> String {
    format!("{instance}:{generation}")
}

/// Take the picture a `_CanvasFrame` update announced (`None` if it was replaced by a newer one that
/// has its own update coming, or the form is gone).
pub fn take(canvas: &str, token: &str) -> Option<Arc<CanvasFrame>> {
    let (inst, _gen) = token.trim().split_once(':')?;
    let instance: u64 = inst.parse().ok()?;
    let s = slots().lock().ok()?;
    s.get(&(instance, canvas.trim().to_ascii_uppercase())).cloned()
}

/// A form's end: its slots go.
pub fn forget(instance: u64) {
    if let Ok(mut s) = slots().lock() {
        s.retain(|(i, _), _| *i != instance);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_picture_wins_and_a_form_end_frees_its_slots() {
        let i = next_instance();
        publish(i, "Canvas-1", CanvasFrame { width: 2, height: 1, rgba_premultiplied: vec![0; 8], generation: 1 });
        publish(i, "CANVAS-1", CanvasFrame { width: 2, height: 1, rgba_premultiplied: vec![255; 8], generation: 2 });
        let f = take("canvas-1", &token(i, 2)).expect("a picture");
        assert_eq!((f.generation, f.rgba_premultiplied[0]), (2, 255));
        forget(i);
        assert!(take("canvas-1", &token(i, 2)).is_none());
    }
}
