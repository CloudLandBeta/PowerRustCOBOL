// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The measurements of WASM-018 R20: one thread against many for the heavy work, with the
//! fingerprint of the result to show the bytes are the same.
//!
//! ```text
//! cargo run --release -p cobolt-graphics --example measure
//! ```
//!
//! Every number it prints is a number it measured on the machine it ran on (best of a few runs);
//! the header names the machine's thread count. It needs the default features.

use std::sync::Arc;
use std::time::{Duration, Instant};

use cobolt_graphics::canvas::{Surface, SurfaceConfig};
use cobolt_graphics::display_list::DisplayListBuilder;
use cobolt_graphics::imagebuf::{ImageBuffer, ResizeFilter};
use cobolt_graphics::par;
use cobolt_graphics::physics::{BodyKind, Physics3D, ShapeOptions};
use cobolt_graphics::plot::{parse_columns, Plot, SeriesKind, TableSpec};
use cobolt_graphics::resources::NoResources;
use cobolt_graphics::scene3d::math::{Mat4, Vec3};
use cobolt_graphics::scene3d::{Backend, Canvas3D, Scene3D};
use cobolt_graphics::text::FontSet;
use cobolt_graphics::{Limits, PixelBuffer, Rgba};

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn best_of<R>(runs: usize, mut f: impl FnMut() -> R) -> (Duration, R) {
    let mut best = Duration::MAX;
    let mut last = None;
    for _ in 0..runs {
        let t = Instant::now();
        let r = f();
        best = best.min(t.elapsed());
        last = Some(r);
    }
    (best, last.expect("at least one run"))
}

fn ms(d: Duration) -> String {
    format!("{:.1} ms", d.as_secs_f64() * 1000.0)
}

struct Row {
    what: String,
    one: Duration,
    many: Duration,
    same: bool,
}

fn row(rows: &mut Vec<Row>, threads: usize, what: &str, runs: usize, mut work: impl FnMut() -> u64 + Send) {
    let (one, a) = best_of(runs, || par::with_threads(1, &mut work));
    let (many, b) = best_of(runs, || par::with_threads(threads, &mut work));
    rows.push(Row { what: what.to_string(), one, many, same: a == b });
}

fn noise_image(w: u32, h: u32) -> PixelBuffer {
    let mut data = vec![0u8; w as usize * h as usize * 4];
    let mut s = 0x9E37_79B9_7F4A_7C15u64;
    for px in data.chunks_exact_mut(4) {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        px.copy_from_slice(&[(s >> 8) as u8, (s >> 24) as u8, (s >> 40) as u8, 255]);
    }
    PixelBuffer::from_rgba(w, h, data).unwrap()
}

fn main() {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    println!("machine: {threads} hardware thread(s); the 'many' column uses {threads}; best of 2 to 3 runs");
    let limits = Limits::default();
    let mut rows: Vec<Row> = Vec::new();

    // ---- ImageBuffer: 24 megapixels ------------------------------------------------------------
    let px = noise_image(6000, 4000);
    println!("image: 6000 x 4000 = {} megapixels", 6000 * 4000 / 1_000_000);
    row(&mut rows, threads, "ImageBuffer 24 MP: BlurGaussian sigma 4 (exact kernel)", 2, || {
        let mut i = ImageBuffer::from_pixels(px.clone());
        i.blur_gaussian(4.0);
        fnv(i.pixels().as_bytes())
    });
    row(&mut rows, threads, "ImageBuffer 24 MP: BlurGaussian sigma 20 (three box passes)", 2, || {
        let mut i = ImageBuffer::from_pixels(px.clone());
        i.blur_gaussian(20.0);
        fnv(i.pixels().as_bytes())
    });
    row(&mut rows, threads, "ImageBuffer 24 MP: Resize to 3000 x 2000, Lanczos", 2, || {
        let mut i = ImageBuffer::from_pixels(px.clone());
        i.resize(3000, 2000, ResizeFilter::Lanczos, &limits).unwrap();
        fnv(i.pixels().as_bytes())
    });
    row(&mut rows, threads, "ImageBuffer 24 MP: Saturation + Contrast", 3, || {
        let mut i = ImageBuffer::from_pixels(px.clone());
        i.saturation(1.4);
        i.contrast(20.0);
        fnv(i.pixels().as_bytes())
    });
    row(&mut rows, threads, "ImageBuffer 24 MP: Sharpen", 2, || {
        let mut i = ImageBuffer::from_pixels(px.clone());
        i.sharpen(1.0);
        fnv(i.pixels().as_bytes())
    });
    drop(px);

    // ---- Canvas2D: 10,000 shapes ------------------------------------------------------------------
    let mut b = DisplayListBuilder::new();
    let t = Instant::now();
    let mut s = 12345u64;
    let mut rnd = |n: u32| {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s % n as u64) as f32
    };
    b.set_linear_gradient(0, 0.0, 0.0, 0.0, 1536.0, &[(0.0, Rgba::rgb(20, 30, 60)), (1.0, Rgba::rgb(120, 140, 200))]).fill_rect(0.0, 0.0, 2048.0, 1536.0);
    for i in 0..10_000 {
        let (x, y) = (rnd(2000), rnd(1500));
        b.set_fill_color(Rgba::new(rnd(256) as u8, rnd(256) as u8, rnd(256) as u8, 120 + rnd(100) as u8)).set_stroke_color(Rgba::new(0, 0, 0, 160)).set_line_width(1.0 + rnd(3));
        match i % 4 {
            0 => {
                b.fill_rect(x, y, 10.0 + rnd(80), 10.0 + rnd(80));
            }
            1 => {
                b.draw_circle(x, y, 5.0 + rnd(40), 3);
            }
            2 => {
                b.begin_path().move_to(x, y).quad_to(x + rnd(80), y - rnd(80), x + rnd(120), y + rnd(40)).line_to(x + rnd(60), y + rnd(90)).close_path().fill(0).stroke();
            }
            _ => {
                b.save().translate(x, y).rotate(rnd(628) / 100.0).stroke_rect(0.0, 0.0, 30.0 + rnd(50), 20.0 + rnd(30)).restore();
            }
        }
    }
    let list = b.take().unwrap();
    let recorded_in = t.elapsed();
    println!("display list: 10,000 shapes recorded in {} ({} KB, {} commands)", ms(recorded_in), list.len() / 1024, 10_000 * 7);
    row(&mut rows, threads, "Canvas2D 2048 x 1536: execute a display list of 10,000 shapes", 3, || {
        let mut surface = Surface::new(&SurfaceConfig::new(2048, 1536), &limits).unwrap();
        surface.execute(&list, &NoResources).unwrap();
        fnv(surface.to_image().as_bytes())
    });

    // ---- Plot: 100,000 points ---------------------------------------------------------------------
    let fonts = Arc::new(FontSet::new());
    fonts.add_font_data(epaint_default_fonts::UBUNTU_LIGHT.to_vec()).unwrap();
    fonts.set_generic(cobolt_graphics::text::Generic::SansSerif, "Ubuntu-Light");
    let mut plot = Plot::new(limits.clone());
    plot.options.title = "100,000 points".into();
    let series = plot.add_series(SeriesKind::Line, "signal").unwrap();
    let t = Instant::now();
    for i in 0..100_000 {
        let x = i as f64 / 100.0;
        plot.add_point(series, x, (x * 0.7).sin() * 3.0 + (x * 5.3).sin() * 0.5 + (x * 0.011).cos()).unwrap();
    }
    println!("plot: 100,000 points added one by one in {}", ms(t.elapsed()));
    for (w, h) in [(1200u32, 800u32), (3000, 2000)] {
        let p = &plot;
        let f = fonts.clone();
        row(&mut rows, threads, &format!("Plot 100,000 points: Render {w} x {h}"), 2, move || fnv(p.render(w, h, Some(f.as_ref())).unwrap().as_bytes()));
    }

    // A COBOL table of 100,000 rows (PIC 9(6) DISPLAY, PIC S9(5)V99 COMP-3) read in one pass.
    let n = 100_000usize;
    let mut block = Vec::with_capacity(n * 10);
    for i in 0..n {
        block.extend_from_slice(format!("{:06}", i).as_bytes());
        let v = (((i as f64 * 0.01).sin() * 1000.0 + 1000.0) * 100.0) as u64 % 10_000_000;
        let d: Vec<u8> = format!("{:07}", v).bytes().map(|c| c - b'0').collect();
        block.extend_from_slice(&[d[0] << 4 | d[1], d[2] << 4 | d[3], d[4] << 4 | d[5], d[6] << 4 | 0x0C]);
    }
    let spec = TableSpec { count: n, stride: 10, columns: parse_columns("0:zoned(6,0); 6:packed(7,2,s)").unwrap() };
    let mut p2 = Plot::new(limits.clone());
    let s2 = p2.add_series(SeriesKind::Line, "").unwrap();
    let (t_table, _) = best_of(5, || p2.set_series_from_table(s2, &block, &spec).unwrap());
    println!("plot: a COBOL table of 100,000 rows (zoned x, packed y) read into a series in one pass: {}", ms(t_table));

    // ---- Physics3D ----------------------------------------------------------------------------------
    row(&mut rows, threads, "Physics3D: 1,000 bodies, 240 steps", 2, || {
        let mut p = Physics3D::new(limits.clone());
        let w = p.create_world([0.0, -9.81, 0.0]).unwrap();
        let floor = p.add_body(w, BodyKind::Static, [0.0, -0.5, 0.0], [0.0, 0.0, 0.0, 1.0]).unwrap();
        p.add_box(w, floor, [40.0, 0.5, 40.0], &ShapeOptions::default()).unwrap();
        let side = 10usize;
        for k in 0..1000usize {
            let (x, y, z) = (k % side, k / (side * side), (k / side) % side);
            let j = |s: usize| ((k * 7919 + s * 104729) % 1000) as f32 / 1000.0 * 0.2;
            let b = p.add_body(w, BodyKind::Dynamic, [x as f32 * 1.3 - 6.0 + j(1), 0.8 + y as f32 * 1.3 + j(2), z as f32 * 1.3 - 6.0 + j(3)], [0.0, 0.0, 0.0, 1.0]).unwrap();
            if k % 2 == 0 {
                p.add_box(w, b, [0.5, 0.5, 0.5], &ShapeOptions::default()).unwrap();
            } else {
                p.add_ball(w, b, 0.55, &ShapeOptions::default()).unwrap();
            }
        }
        // The solver reads the global rayon pool, so pin it to the thread count of this run.
        let threads_now = par::current_threads();
        p.set_threads(w, threads_now).unwrap();
        for _ in 0..240 {
            p.step(w, 1.0 / 60.0).unwrap();
        }
        p.state_fingerprint(w).unwrap()
    });

    // ---- Scene3D: frame building and the GPU ---------------------------------------------------------
    for count in [100_000usize, 1_000_000] {
        let mut scene = Scene3D::new(Limits { max_triangles: u64::MAX / 4, ..limits.clone() });
        let cam = scene.add_perspective_camera(60.0, 0.1, 5000.0).unwrap();
        scene.set_position(cam, [0.0, 20.0, 60.0]).unwrap();
        let node = scene.add_box([0.5; 3]).unwrap();
        let side = (count as f64).sqrt() as usize;
        let mut m = Vec::with_capacity(count * 16);
        for i in 0..count {
            let t = Mat4::from_translation(Vec3::new((i % side) as f32 * 1.5 - side as f32 * 0.75, ((i / side) % 7) as f32, -((i / side) as f32) * 1.5));
            m.extend_from_slice(&t.to_cols_array());
        }
        scene.set_instances(node, &m).unwrap();
        let sc = std::sync::Mutex::new(scene);
        let what = format!("Scene3D: build_frame for {count} instances (cull against the frustum)");
        let (one, a) = best_of(3, || par::with_threads(1, || fnv(&sc.lock().unwrap().build_frame(Some(cam), 1.6).unwrap().draws[0].visible.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>())));
        let (many, b) = best_of(3, || par::with_threads(threads, || fnv(&sc.lock().unwrap().build_frame(Some(cam), 1.6).unwrap().draws[0].visible.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>())));
        rows.push(Row { what, one, many, same: a == b });
    }

    println!();
    println!("| What | 1 thread | {threads} threads | speed-up | same bytes |");
    println!("|---|---|---|---|---|");
    for r in &rows {
        println!("| {} | {} | {} | {:.2}x | {} |", r.what, ms(r.one), ms(r.many), r.one.as_secs_f64() / r.many.as_secs_f64(), if r.same { "yes" } else { "NO" });
    }

    // ---- the GPU: N triangles ------------------------------------------------------------------------
    println!();
    let mut c = Canvas3D::new(1280, 720, limits.clone()).unwrap();
    let backend = c.connect();
    if backend == Backend::None {
        println!("GPU: no adapter ({})", c.status().unwrap_or(""));
    } else {
        println!("GPU: {} - {}", backend.name(), c.adapter_description().unwrap_or_default());
        println!("| Triangles in the scene | Canvas3D 1280 x 720, 4x MSAA, one sun with a shadow map: frame (CPU build + GPU) | read-back of the picture |");
        println!("|---|---|---|");
        for target in [100_000u64, 500_000, 1_000_000, 2_000_000] {
            let mut scene = Scene3D::new(Limits { max_triangles: u64::MAX / 4, ..limits.clone() });
            let cam = scene.add_perspective_camera(50.0, 0.1, 1000.0).unwrap();
            scene.set_position(cam, [0.0, 4.0, 14.0]).unwrap();
            scene.look_at(cam, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
            let floor = scene.add_plane(40.0, 40.0).unwrap();
            let _ = floor;
            let ball = scene.add_sphere(0.35, 64).unwrap(); // 32 rings x 64 segments x 2
            let tris_per = 32 * 64 * 2u64;
            let copies = (target / tris_per).max(1) as usize;
            let side = (copies as f64).sqrt().ceil() as usize;
            let mut m = Vec::with_capacity(copies * 16);
            for i in 0..copies {
                m.extend_from_slice(&Mat4::from_translation(Vec3::new((i % side) as f32 * 0.8 - side as f32 * 0.4, 0.4 + ((i / side) % 3) as f32 * 0.8, (i / side) as f32 * 0.8 - side as f32 * 0.4)).to_cols_array());
            }
            scene.set_instances(ball, &m).unwrap();
            let sun = scene.add_directional_light([1.0; 3], 2.0, true).unwrap();
            scene.set_position(sun, [10.0, 20.0, 10.0]).unwrap();
            scene.look_at(sun, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
            c.set_camera(Some(cam));
            c.render(&mut scene).unwrap(); // warm-up: uploads
            let renderer_gpu = c.renderer().unwrap().gpu().clone();
            let (frame, _) = best_of(5, || {
                let s = c.render(&mut scene).unwrap();
                renderer_gpu.wait_idle();
                s
            });
            let (readback, _) = best_of(3, || c.screenshot().unwrap());
            println!("| {} ({copies} copies of a {tris_per}-triangle sphere) | {} | {} |", copies as u64 * tris_per, ms(frame), ms(readback));
        }
    }
}
