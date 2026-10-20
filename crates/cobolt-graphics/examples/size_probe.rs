// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Keeps every compiled-in area of the crate alive so its size can be measured (WASM-018 R18).
//!
//! ```text
//! cargo build --release --example size_probe --target wasm32-unknown-unknown \
//!     --no-default-features --features "text,imagebuf"
//! ```
//!
//! The program makes one object of every kind this build has and calls every method of each
//! by name, so the linker cannot drop any of them; with `scene3d` it also keeps the GPU
//! renderer (a future that opens the adapter, renders and reads the picture back) alive. The size
//! of the resulting module is what that set of features costs; the player builds each heavy area
//! as a part of its own, loaded on first use (WASM-016 R13).

use cobolt_graphics::{Limits, ObjectKind, Registry, Value};

fn main() {
    let mut reg = Registry::new(Limits::default());
    for kind in ObjectKind::available() {
        let ctor: &[Value] = match kind {
            ObjectKind::Canvas2D | ObjectKind::Canvas3D => &[Value::Int(64), Value::Int(48)],
            _ => &[],
        };
        let Ok(h) = reg.create(kind, 1, ctor) else { continue };
        for m in Registry::methods(kind) {
            let _ = std::hint::black_box(reg.invoke(h, std::hint::black_box(&m.name), &[]));
        }
    }
    #[cfg(feature = "scene3d")]
    {
        use cobolt_graphics::scene3d::{Gpu, Renderer, Scene3D};
        use std::future::Future;
        use std::pin::Pin;
        let fut: Pin<Box<dyn Future<Output = cobolt_graphics::Result<usize>>>> = Box::pin(async {
            let gpu = Gpu::new_async().await?;
            let mut renderer = Renderer::new(gpu, Default::default())?;
            let mut scene = Scene3D::new(Limits::default());
            scene.add_box([1.0; 3])?;
            let frame = scene.build_frame(None, 1.0)?;
            renderer.render(&frame, 64, 64, 4)?;
            Ok(renderer.read_pixels_async().await?.as_bytes().len())
        });
        let _keep = std::hint::black_box(fut);
    }
}
