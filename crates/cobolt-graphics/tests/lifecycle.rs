// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! AC9 (WASM-018): opening and closing a form with a canvas, an image, a chart, a scene
//! and a physics world a hundred times leaves memory at its baseline.
//!
//! The test binary counts every byte the allocator has handed out and not taken back, so
//! "memory at its baseline" is measured, not assumed: a leak in any object - a table that
//! keeps what it was given, a cache that grows, a thread that is never joined - shows as bytes.
#![cfg(all(feature = "text", feature = "imagebuf", feature = "plot", feature = "physics", feature = "scene3d-core"))]

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;

use cobolt_graphics::registry::{ObjectKind, Registry};
use cobolt_graphics::text::FontSet;
use cobolt_graphics::{Limits, Value};
use common::{glb, synthfont};

struct Counting;
static LIVE: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            LIVE.fetch_add(l.size() as isize, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        LIVE.fetch_sub(l.size() as isize, Ordering::Relaxed);
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = System.alloc_zeroed(l);
        if !p.is_null() {
            LIVE.fetch_add(l.size() as isize, Ordering::Relaxed);
        }
        p
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        let q = System.realloc(p, l, new);
        if !q.is_null() {
            LIVE.fetch_add(new as isize - l.size() as isize, Ordering::Relaxed);
        }
        q
    }
}

#[global_allocator]
static A: Counting = Counting;

fn live() -> isize {
    LIVE.load(Ordering::Relaxed)
}

fn int(v: Value) -> i64 {
    match v {
        Value::Int(i) => i,
        o => panic!("{o:?}"),
    }
}

/// One "form": everything a busy form might make, used, then closed.
fn run_form(reg: &mut Registry, form: u32, model: &[u8]) {
    // A canvas with a few thousand shapes and some text, read back as an image.
    let canvas = reg.create(ObjectKind::Canvas2D, form, &[Value::Int(320), Value::Int(240)]).unwrap();
    reg.invoke(canvas, "SetFillColor", &[Value::Text("#336699".into())]).unwrap();
    for i in 0..2000 {
        reg.invoke(canvas, "FillRect", &[Value::Int(i % 300), Value::Int(i % 220), Value::Int(8), Value::Int(8)]).unwrap();
    }
    reg.invoke(canvas, "SetFont", &[Value::Text("16px Life".into())]).unwrap();
    reg.invoke(canvas, "DrawText", &[Value::Text("HELLO".into()), Value::Int(10), Value::Int(30), Value::Int(0), Value::Int(1)]).unwrap();
    reg.invoke(canvas, "Present", &[]).unwrap();
    let shot = int(reg.invoke(canvas, "ToImage", &[]).unwrap());

    // An image loaded, filtered, resized and encoded.
    let img = reg.create(ObjectKind::ImageBuffer, form, &[]).unwrap();
    reg.invoke(img, "Create", &[Value::Int(400), Value::Int(300), Value::Text("#FFAA00".into())]).unwrap();
    reg.invoke(img, "BlurGaussian", &[Value::Num(3.0)]).unwrap();
    reg.invoke(img, "Resize", &[Value::Int(200), Value::Int(150)]).unwrap();
    reg.invoke(img, "Encode", &[Value::Text("png".into())]).unwrap();

    // A chart rendered into an image and onto the canvas.
    let plot = reg.create(ObjectKind::Plot, form, &[]).unwrap();
    let s = int(reg.invoke(plot, "AddSeries", &[Value::Text("line".into()), Value::Text("s".into())]).unwrap());
    for i in 0..500 {
        reg.invoke(plot, "AddPoint", &[Value::Int(s), Value::Int(i), Value::Num((i as f64 / 20.0).sin())]).unwrap();
    }
    let chart = int(reg.invoke(plot, "Render", &[Value::Int(300), Value::Int(200)]).unwrap());
    reg.invoke(plot, "RenderToCanvas", &[Value::Int(canvas as i64), Value::Int(0), Value::Int(0), Value::Int(200), Value::Int(100)]).unwrap();

    // A scene with a model, primitives, a material with a texture from the chart, and an animation step.
    let scene = reg.create(ObjectKind::Scene3D, form, &[]).unwrap();
    reg.invoke(scene, "LoadModelFromBytes", &[Value::Bytes(model.to_vec())]).unwrap();
    let ball = int(reg.invoke(scene, "AddSphere", &[Value::Num(1.0), Value::Int(24)]).unwrap());
    let mat = int(reg.invoke(scene, "GetMaterial", &[Value::Int(ball)]).unwrap());
    reg.invoke(scene, "SetTexture", &[Value::Int(mat), Value::Int(0), Value::Int(chart)]).unwrap();
    let anim = int(reg.invoke(scene, "FindAnimation", &[Value::Text("bend".into())]).unwrap());
    reg.invoke(scene, "PlayAnimation", &[Value::Int(anim)]).unwrap();
    reg.invoke(scene, "Update", &[Value::Int(100)]).unwrap();

    // A physics world with some bodies stepped a little.
    let phys = reg.create(ObjectKind::Physics3D, form, &[]).unwrap();
    let w = int(reg.invoke(phys, "CreateWorld", &[Value::Int(0), Value::Num(-9.81), Value::Int(0)]).unwrap());
    for i in 0..20 {
        let b = int(reg.invoke(phys, "AddBody", &[Value::Int(w), Value::Int(1), Value::Num(i as f64 * 0.1), Value::Int(i + 2), Value::Int(0)]).unwrap());
        reg.invoke(phys, "AddBall", &[Value::Int(w), Value::Int(b), Value::Num(0.3)]).unwrap();
    }
    for _ in 0..10 {
        reg.invoke(phys, "Step", &[Value::Int(w), Value::Num(1.0 / 60.0)]).unwrap();
    }
    assert!(reg.len() >= 7, "the form made {} objects", reg.len());
    let _ = shot;
}

#[test]
fn a_hundred_forms_opened_and_closed_leave_memory_where_it_was() {
    let mut reg = Registry::new(Limits::default());
    let fonts = Arc::new(FontSet::new());
    fonts.add_font_data(synthfont::latin("Life")).unwrap();
    reg.set_text_engine(fonts);
    let model = glb::glb(&glb::arm_model(None));

    // Warm up: first-use caches (the font's layout cache, rayon's pool, the allocator's own pages).
    for form in 1..=3 {
        run_form(&mut reg, form, &model);
        reg.release_owner(form);
    }
    assert!(reg.is_empty());
    let baseline = live();
    let mut samples = Vec::new();
    for form in 1..=100u32 {
        run_form(&mut reg, 100 + form, &model);
        assert!(reg.release_owner(100 + form) >= 7);
        assert!(reg.is_empty() && reg.memory_bytes() == 0, "the registry's own accounting is back to zero");
        if form % 25 == 0 {
            samples.push(live() - baseline);
        }
    }
    eprintln!("AC9: bytes above the baseline after 25, 50, 75 and 100 forms: {samples:?} (baseline {} KB)", baseline / 1024);
    // Each form allocates megabytes (canvas, images, charts); a leak of even one object per form would be 100x that.
    let slack = 1024 * 1024;
    assert!(samples.iter().all(|s| s.abs() < slack), "memory is back at its baseline within 1 MB: {samples:?}");
    assert!((samples[3] - samples[1]).abs() < 256 * 1024, "and it does not creep between form 50 and form 100: {samples:?}");
}

#[test]
fn handles_do_not_survive_their_form() {
    let mut reg = Registry::new(Limits::default());
    let made: Vec<(u32, ObjectKind)> = ObjectKind::available()
        .into_iter()
        .filter(|k| *k != ObjectKind::Canvas3D)
        .map(|k| {
            let ctor = if k == ObjectKind::Canvas2D { vec![Value::Int(20), Value::Int(20)] } else { vec![] };
            (reg.create(k, 5, &ctor).unwrap(), k)
        })
        .collect();
    assert_eq!(reg.release_owner(5), made.len());
    for (h, k) in made {
        let e = reg.invoke(h, "Release", &[]).unwrap_err();
        assert!(matches!(e, cobolt_graphics::GraphicsError::ReleasedHandle(_)), "{} handle {h}: {e:?}", k.name());
        // The slot is reused by a new object under a new generation: the old number stays dead.
        let again = reg.create(k, 6, &if k == ObjectKind::Canvas2D { vec![Value::Int(20), Value::Int(20)] } else { vec![] }).unwrap();
        assert_ne!(again, h);
        assert!(reg.invoke(h, "NoMatterWhat", &[]).is_err());
    }
}
