// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A CPU rasteriser for egui's output** (spec 084 T14).
//!
//! The render engine draws through egui; a GPU turns egui's tessellated
//! meshes into pixels on screen. A coding agent's `render_form` / `run_form`
//! need those pixels **without** a window or a GPU (headless, on CI), so this
//! fills the same triangles in software: barycentric coverage of every mesh
//! triangle, vertex colour × texture sample, premultiplied-alpha "over", clipped
//! to each primitive's clip rectangle. egui feathers its edges itself (a thin
//! band of vertices fading to transparent), so the result is anti-aliased
//! without any sampling scheme of our own.
//!
//! Deterministic: the same frame gives the same bytes on every machine, wherever
//! its glyphs sit in the font atlas.
//! `Shape::Callback` (custom GPU painting) has no CPU form and is skipped.

use std::collections::HashMap;

use egui::epaint::{ClippedPrimitive, Primitive};
use egui::{Color32, ColorImage, TextureId, TexturesDelta};

/// The textures egui has handed over so far (font atlas, images), kept up to
/// date by [`Rasterizer::apply`].
#[derive(Default)]
pub struct Rasterizer {
    textures: HashMap<TextureId, ColorImage>,
}

impl Rasterizer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a frame's texture changes — whole images and partial updates
    /// alike — exactly as a GPU backend uploads them.
    pub fn apply(&mut self, delta: &TexturesDelta) {
        for (id, updates) in &delta.set {
            for update in updates {
                let egui::ImageData::Color(image) = &update.image;
                match update.pos {
                    None => {
                        self.textures.insert(*id, (**image).clone());
                    }
                    Some([x0, y0]) => {
                        let target = self.textures.entry(*id).or_insert_with(|| ColorImage::filled([x0 + image.size[0], y0 + image.size[1]], Color32::TRANSPARENT));
                        let [w, h] = image.size;
                        for y in 0..h {
                            for x in 0..w {
                                let (tx, ty) = (x0 + x, y0 + y);
                                if tx < target.size[0] && ty < target.size[1] {
                                    target.pixels[ty * target.size[0] + tx] = image.pixels[y * w + x];
                                }
                            }
                        }
                    }
                }
            }
        }
        for id in &delta.free {
            self.textures.remove(id);
        }
    }

    /// Fill `primitives` (from `Context::tessellate`) into an image of
    /// `size_px` physical pixels over `background`.
    pub fn render(&self, primitives: &[ClippedPrimitive], size_px: [usize; 2], pixels_per_point: f32, background: Color32) -> ColorImage {
        let mut out = ColorImage::filled(size_px, background);
        for prim in primitives {
            let Primitive::Mesh(mesh) = &prim.primitive else { continue };
            let texture = self.textures.get(&mesh.texture_id);
            let clip = prim.clip_rect;
            let clip = [
                (clip.min.x * pixels_per_point).floor().max(0.0) as i64,
                (clip.min.y * pixels_per_point).floor().max(0.0) as i64,
                (clip.max.x * pixels_per_point).ceil().min(size_px[0] as f32) as i64,
                (clip.max.y * pixels_per_point).ceil().min(size_px[1] as f32) as i64,
            ];
            for tri in mesh.indices.chunks_exact(3) {
                let v = [&mesh.vertices[tri[0] as usize], &mesh.vertices[tri[1] as usize], &mesh.vertices[tri[2] as usize]];
                fill_triangle(&mut out, clip, pixels_per_point, v, texture);
            }
        }
        out
    }
}

fn fill_triangle(out: &mut ColorImage, clip: [i64; 4], ppp: f32, v: [&egui::epaint::Vertex; 3], texture: Option<&ColorImage>) {
    let p = v.map(|v| (v.pos.x * ppp, v.pos.y * ppp));
    let area = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[2].0 - p[0].0) * (p[1].1 - p[0].1);
    if area.abs() < f32::EPSILON {
        return;
    }
    let min_x = (p[0].0.min(p[1].0).min(p[2].0).floor() as i64).max(clip[0]);
    let max_x = (p[0].0.max(p[1].0).max(p[2].0).ceil() as i64).min(clip[2]);
    let min_y = (p[0].1.min(p[1].1).min(p[2].1).floor() as i64).max(clip[1]);
    let max_y = (p[0].1.max(p[1].1).max(p[2].1).ceil() as i64).min(clip[3]);
    let width = out.size[0];
    for y in min_y..max_y {
        for x in min_x..max_x {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Barycentric weights; a pixel centre on an edge counts once
            // because the weights are compared with >= 0 on a consistent winding.
            let w0 = ((p[1].0 - px) * (p[2].1 - py) - (p[2].0 - px) * (p[1].1 - py)) / area;
            let w1 = ((p[2].0 - px) * (p[0].1 - py) - (p[0].0 - px) * (p[2].1 - py)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let mut c = [0.0f32; 4];
            for (i, w) in [w0, w1, w2].into_iter().enumerate() {
                let [r, g, b, a] = v[i].color.to_array();
                c[0] += w * r as f32;
                c[1] += w * g as f32;
                c[2] += w * b as f32;
                c[3] += w * a as f32;
            }
            if let Some(tex) = texture {
                // The texture coordinate is the first vertex's plus the weighted differences, in
                // f64. A glyph's place in the font atlas depends on the order text was first laid
                // out in; summed from absolute coordinates in f32, the same glyph at another place
                // rounded differently and could come out one step apart. This way the sum is exact
                // for an atlas whose sides are powers of two (egui's), so it samples the same.
                let rel = |a: f32, b: f32, c: f32| a as f64 + (w1 * (b - a) + w2 * (c - a)) as f64;
                let u = rel(v[0].uv.x, v[1].uv.x, v[2].uv.x);
                let t = rel(v[0].uv.y, v[1].uv.y, v[2].uv.y);
                let s = sample(tex, u, t);
                for k in 0..4 {
                    c[k] = c[k] * s[k] / 255.0;
                }
            }
            let src = [c[0], c[1], c[2], c[3]].map(|x| x.clamp(0.0, 255.0));
            let idx = y as usize * width + x as usize;
            let [dr, dg, db, da] = out.pixels[idx].to_array();
            let k = 1.0 - src[3] / 255.0;
            let blend = |s: f32, d: u8| (s + d as f32 * k).round().clamp(0.0, 255.0) as u8;
            out.pixels[idx] = Color32::from_rgba_premultiplied(blend(src[0], dr), blend(src[1], dg), blend(src[2], db), blend(src[3], da));
        }
    }
}

/// Bilinear sample of a premultiplied texture at normalised `u`, `v`.
fn sample(tex: &ColorImage, u: f64, v: f64) -> [f32; 4] {
    let [w, h] = tex.size;
    if w == 0 || h == 0 {
        return [255.0; 4];
    }
    let x = (u * w as f64 - 0.5).clamp(0.0, (w - 1) as f64);
    let y = (v * h as f64 - 0.5).clamp(0.0, (h - 1) as f64);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
    let px = |x: usize, y: usize| tex.pixels[y * w + x].to_array().map(f32::from);
    let (a, b, c, d) = (px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1));
    let mut out = [0.0; 4];
    for k in 0..4 {
        let top = a[k] + (b[k] - a[k]) * fx;
        let bottom = c[k] + (d[k] - c[k]) * fx;
        out[k] = top + (bottom - top) * fy;
    }
    out
}

/// Encode as PNG (straight alpha, as PNG requires).
pub fn to_png(image: &ColorImage) -> Result<Vec<u8>, String> {
    let [w, h] = image.size;
    let mut rgba = Vec::with_capacity(w * h * 4);
    for p in &image.pixels {
        rgba.extend_from_slice(&p.to_srgba_unmultiplied());
    }
    let buf = image::RgbaImage::from_raw(w as u32, h as u32, rgba).ok_or("bad image size")?;
    let mut out = std::io::Cursor::new(Vec::new());
    buf.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// The largest texture side a frame drawn here reports: what a window of this
/// product reports. Its `wgpu` device is asked for 2D textures of up to 8192
/// pixels a side (`native_options` keeps egui-wgpu's request), and egui takes
/// that as `max_texture_side`. Left unset, egui assumes 2048, and a picture
/// wider or taller than that, which a window shows, stopped a headless run.
pub const MAX_TEXTURE_SIDE: usize = 8192;

/// The input of one frame drawn here.
fn frame_input(size: egui::Vec2, time: f64) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        time: Some(time),
        max_texture_side: Some(MAX_TEXTURE_SIDE),
        ..Default::default()
    }
}

/// Run one egui frame at `time` seconds and rasterise it: the convenience
/// every headless caller wants. `rasterizer` carries the textures across
/// frames; advance `time` between calls so fades and animations settle.
pub fn render_frame(
    ctx: &egui::Context,
    rasterizer: &mut Rasterizer,
    size: egui::Vec2,
    background: Color32,
    time: f64,
    ui: impl FnMut(&mut egui::Ui),
) -> ColorImage {
    let input = frame_input(size, time);
    let mut full = ctx.run_ui(input, ui);
    rasterizer.apply(&full.textures_delta);
    full.textures_delta.clear(); // applied — egui insists it is said
    let ppp = full.pixels_per_point;
    let prims = ctx.tessellate(std::mem::take(&mut full.shapes), ppp);
    let size_px = [(size.x * ppp).round() as usize, (size.y * ppp).round() as usize];
    rasterizer.render(&prims, size_px, ppp, background)
}

/// Run one egui frame at `time` and take its texture uploads, without filling
/// any pixel: the warm-up frames before the one that is pictured.
pub fn advance_frame(ctx: &egui::Context, rasterizer: &mut Rasterizer, size: egui::Vec2, time: f64, ui: impl FnMut(&mut egui::Ui)) {
    advance_frame_with(ctx, rasterizer, size, time, Vec::new(), ui);
}

/// [`advance_frame`] with the input `events` (a pointer move, a press) the frame receives.
pub fn advance_frame_with(
    ctx: &egui::Context,
    rasterizer: &mut Rasterizer,
    size: egui::Vec2,
    time: f64,
    events: Vec<egui::Event>,
    ui: impl FnMut(&mut egui::Ui),
) {
    let mut input = frame_input(size, time);
    input.events = events;
    let mut full = ctx.run_ui(input, ui);
    rasterizer.apply(&full.textures_delta);
    full.textures_delta.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A filled rectangle lands on exactly its pixels, its colour exact; text
    /// leaves ink where it was drawn and nowhere else; the PNG decodes back.
    #[test]
    fn a_rect_and_text_rasterise_where_they_were_drawn() {
        let ctx = egui::Context::default();
        let mut r = Rasterizer::new();
        let size = egui::vec2(200.0, 100.0);
        let bg = Color32::from_rgb(10, 10, 10);
        let draw = |ui: &mut egui::Ui| {
            ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(40.0, 30.0)), 0.0, Color32::from_rgb(200, 30, 40));
            ui.painter().text(egui::pos2(100.0, 60.0), egui::Align2::LEFT_TOP, "Hello", egui::FontId::proportional(20.0), Color32::WHITE);
        };
        // Two frames: the first uploads the font atlas.
        let _ = render_frame(&ctx, &mut r, size, bg, 0.0, draw);
        let img = render_frame(&ctx, &mut r, size, bg, 0.1, draw);
        assert_eq!(img.size, [200, 100]);
        let at = |x: usize, y: usize| img.pixels[y * 200 + x];
        assert_eq!(at(40, 35), Color32::from_rgb(200, 30, 40), "inside the rect");
        assert_eq!(at(10, 10), bg, "outside everything");
        let ink = (100..170).flat_map(|x| (60..90).map(move |y| (x, y))).filter(|&(x, y)| at(x, y) != bg).count();
        let stray = (0..200).flat_map(|x| (92..100).map(move |y| (x, y))).filter(|&(x, y)| at(x, y) != bg).count();
        assert!(ink > 40, "the text left ink: {ink} px");
        assert_eq!(stray, 0, "nothing below the text");
        let png = to_png(&img).unwrap();
        let back = image::load_from_memory(&png).unwrap();
        assert_eq!((back.width(), back.height()), (200, 100));
        println!("raster: rect exact at its centre, background untouched, text {ink} px of ink, PNG {} bytes round-trips", png.len());
    }

    /// A glyph comes out the same wherever it sits in the atlas. Where egui puts it depends on the
    /// order text was first laid out in, which two runs of one program need not share. The same
    /// 9 × 7 texels are kept at four places of a 2048 × 256 texture and drawn from each, at the
    /// same spot on screen, at four scales that put pixel centres between texels; every picture
    /// must equal the one from the first place, byte for byte.
    #[test]
    fn a_texture_samples_the_same_from_any_place_in_the_atlas() {
        let started = std::time::Instant::now();
        let (aw, ah) = (2048usize, 256usize);
        let places = [(1usize, 1usize), (1777, 201), (1024, 128), (37, 243)];
        let mut atlas = ColorImage::filled([aw, ah], Color32::TRANSPARENT);
        for &(px, py) in &places {
            for y in 0..7 {
                for x in 0..9 {
                    let v = ((x * 37 + y * 91) % 251) as u8;
                    atlas.pixels[(py + y) * aw + px + x] = Color32::from_rgba_premultiplied(v, v, v, v);
                }
            }
        }
        let id = TextureId::Managed(7);
        let mut r = Rasterizer::new();
        r.textures.insert(id, atlas);
        let size = [48usize, 40usize];
        let picture = |(px, py): (usize, usize), (sx, sy): (f32, f32)| {
            let uv = |tx: usize, ty: usize| egui::pos2(tx as f32 / aw as f32, ty as f32 / ah as f32);
            let mut mesh = egui::Mesh::with_texture(id);
            let rect = egui::Rect::from_min_size(egui::pos2(10.3, 6.7), egui::vec2(9.0 * sx, 7.0 * sy));
            mesh.add_rect_with_uv(rect, egui::Rect::from_min_max(uv(px, py), uv(px + 9, py + 7)), Color32::WHITE);
            let prim = ClippedPrimitive { clip_rect: egui::Rect::EVERYTHING, primitive: Primitive::Mesh(mesh) };
            r.render(&[prim], size, 1.0, Color32::BLACK)
        };
        let scales = [(1.0f32, 1.0f32), (1.37, 1.37), (2.9, 0.83), (3.3, 3.7)];
        let (mut compared, mut differing, mut inked) = (0usize, 0usize, 0usize);
        for &scale in &scales {
            let first = picture(places[0], scale);
            inked = inked.max(first.pixels.iter().filter(|p| **p != Color32::BLACK).count());
            for &place in &places[1..] {
                let other = picture(place, scale);
                compared += 1;
                let n = first.pixels.iter().zip(&other.pixels).filter(|(a, b)| a != b).count();
                if n > 0 {
                    differing += 1;
                    println!("  FAIL scale {scale:?}, texels at {place:?}: {n} of {} pixels differ from those at {:?}", size[0] * size[1], places[0]);
                }
            }
        }

        // The result block (GOLDEN RULE #7).
        println!("== raster::a_texture_samples_the_same_from_any_place_in_the_atlas (fix 2.0.492) ==");
        println!("texture 2048 x 256, 9 x 7 texels at {} places; scales {scales:?}; up to {inked} pixels inked per picture", places.len());
        println!("pictures compared {compared}, identical {}, differing {differing} ({} ms)", compared - differing, started.elapsed().as_millis());
        assert!(inked > 60, "the texels were drawn");
        assert_eq!(differing, 0, "a picture depended on where its texels sit in the atlas");
    }

    /// A picture wider than 2048 pixels, which a window of this product shows, is drawn here too.
    /// egui assumed 2048 when a frame did not say, and a debug build stopped at the first such
    /// picture (PowerOficina's 2172 × 724 banner stopped its headless run). Pictures of 2049,
    /// 2172 and 8192 pixels a side are each loaded and drawn into a 100 × 20 rectangle; each one's
    /// colour must cover that rectangle.
    #[test]
    fn a_picture_wider_than_2048_pixels_is_drawn() {
        let started = std::time::Instant::now();
        let ctx = egui::Context::default();
        let mut r = Rasterizer::new();
        let size = egui::vec2(160.0, 60.0);
        let bg = Color32::from_rgb(10, 10, 10);
        let sides = [(2049usize, 4usize), (2172, 724), (4, 8192)];
        let colours = [Color32::from_rgb(200, 30, 40), Color32::from_rgb(30, 200, 40), Color32::from_rgb(30, 40, 200)];
        let mut covered = Vec::new();
        for (&(w, h), &colour) in sides.iter().zip(&colours) {
            let tex = std::cell::RefCell::new(None::<egui::TextureHandle>);
            let draw = |ui: &mut egui::Ui| {
                let mut slot = tex.borrow_mut();
                let t = slot.get_or_insert_with(|| {
                    ui.ctx().load_texture(format!("wide-{w}x{h}"), ColorImage::filled([w, h], colour), egui::TextureOptions::LINEAR)
                });
                let rect = egui::Rect::from_min_size(egui::pos2(30.0, 20.0), egui::vec2(100.0, 20.0));
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter().image(t.id(), rect, uv, Color32::WHITE);
            };
            let _ = render_frame(&ctx, &mut r, size, bg, 0.0, draw);
            let img = render_frame(&ctx, &mut r, size, bg, 0.1, draw);
            let at = |x: usize, y: usize| img.pixels[y * 160 + x];
            let inside = (32..128).flat_map(|x| (22..38).map(move |y| (x, y))).filter(|&(x, y)| at(x, y) == colour).count();
            assert_eq!(inside, 96 * 16, "the {w} × {h} picture covers its rectangle");
            assert_eq!(at(5, 5), bg, "nothing outside it");
            covered.push(format!("{w}x{h} {inside} px"));
        }
        println!(
            "raster: max texture side {MAX_TEXTURE_SIDE}; pictures drawn {} of {} ({}); {} ms",
            covered.len(),
            sides.len(),
            covered.join(", "),
            started.elapsed().as_millis()
        );
    }
}
