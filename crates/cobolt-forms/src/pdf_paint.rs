// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the Viewer PAINTED, written as a PDF.
//!
//! The Viewer's own painter lays the content out at a page's width
//! ([`crate::paint::paint_viewer_export`]); this turns those shapes into PDF
//! operators, one for one, so the file shows what the reader saw — the chat
//! bubbles, a page's CSS boxes, gradients, borders, round badges, flex rows
//! and grids, a Mermaid diagram, the same fonts (operator, 2026-09-28: the
//! PDF "is not faithful to the generated text in the chat").
//!
//! - **Text stays text.** Each glyph is set in the font the Viewer used,
//!   embedded as a subset (unused glyphs emptied) with a `ToUnicode` map, so
//!   a reader can select, copy and search it. A face that cannot be embedded
//!   (a CFF-flavoured font) is drawn as its outlines instead.
//! - **Pages break between lines, never through one.** A break that would
//!   cut a line of text or a picture moves up to its top.
//! - Transparency is kept (`ExtGState`), a gradient is a Gouraud-shaded mesh
//!   (shading type 4), a picture keeps its alpha as a soft mask, and a
//!   shadow is drawn as a few soft layers.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use egui::{Color32, Pos2, Rect, Shape};
use lopdf::{dictionary, Document, Object, ObjectId, Stream, StringFormat};

use crate::paint::ExportPaint;

/// Points per Viewer pixel: the painter's 96 px to the inch, PDF's 72.
const PT_PER_PX: f32 = 0.75;

/// A page's size and margins, in points.
#[derive(Debug, Clone, Copy)]
pub struct PageSetup {
    pub width_pt: f32,
    pub height_pt: f32,
    pub margin_pt: f32,
}

impl PageSetup {
    /// A4 portrait, 1.5 cm margins.
    pub const A4: PageSetup = PageSetup { width_pt: 595.28, height_pt: 841.89, margin_pt: 42.5 };

    /// The width content is laid out at, in Viewer pixels.
    pub fn content_width_px(&self) -> f32 {
        (self.width_pt - 2.0 * self.margin_pt) / PT_PER_PX
    }

    fn content_height_px(&self) -> f32 {
        (self.height_pt - 2.0 * self.margin_pt) / PT_PER_PX
    }
}

/// Write `paint` as a PDF titled `title`: its bytes.
pub fn write_pdf(paint: &ExportPaint, title: &str, page: PageSetup) -> Result<Vec<u8>, String> {
    let mut w = Writer::new(paint, page);
    let breaks = page_ranges(&paint.shapes, paint.height, page.content_height_px());
    let mut page_ids = Vec::new();
    let pages_id = w.doc.new_object_id();
    let resources_id = w.doc.new_object_id();
    for (top, bottom) in &breaks {
        let content = w.page_content(*top, *bottom);
        let mut stream = Stream::new(dictionary! {}, content.into_bytes());
        let _ = stream.compress();
        let content_id = w.doc.add_object(stream);
        let page_id = w.doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), Object::Real(page.width_pt as f64), Object::Real(page.height_pt as f64)],
            "Contents" => content_id,
            "Resources" => resources_id,
        });
        page_ids.push(page_id);
    }
    let resources = w.finish_resources();
    w.doc.objects.insert(resources_id, Object::Dictionary(resources));
    let count = page_ids.len() as i64;
    w.doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids.into_iter().map(Object::Reference).collect::<Vec<_>>(),
            "Count" => count,
        }),
    );
    let catalog = w.doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    let info = w.doc.add_object(dictionary! {
        "Title" => text_string(title),
        "Producer" => text_string("PowerRustCOBOL AI"),
    });
    w.doc.trailer.set("Root", catalog);
    w.doc.trailer.set("Info", info);
    let mut out = Vec::new();
    w.doc.save_to(&mut out).map_err(|e| format!("could not write the PDF: {e}"))?;
    Ok(out)
}

/// A PDF text string: UTF-16BE with its byte-order mark.
fn text_string(s: &str) -> Object {
    let mut bytes = vec![0xFE, 0xFF];
    for u in s.encode_utf16() {
        bytes.extend_from_slice(&u.to_be_bytes());
    }
    Object::String(bytes, StringFormat::Hexadecimal)
}

// ── Pagination ───────────────────────────────────────────────────────────

/// The bands a page break must not cross: every line of text and every
/// picture, top to bottom.
fn unbreakable_bands(shapes: &[Shape], out: &mut Vec<(f32, f32)>) {
    for s in shapes {
        match s {
            Shape::Vec(v) => unbreakable_bands(v, out),
            Shape::Text(t) => {
                for row in &t.galley.rows {
                    let r = row.rect().translate(t.pos.to_vec2());
                    out.push((r.min.y, r.max.y));
                }
            }
            Shape::Rect(r) if r.brush.is_some() => out.push((r.rect.min.y, r.rect.max.y)),
            _ => {}
        }
    }
}

/// Each page's `[top, bottom)` in content pixels.
fn page_ranges(shapes: &[Shape], height: f32, page_h: f32) -> Vec<(f32, f32)> {
    let mut bands = Vec::new();
    unbreakable_bands(shapes, &mut bands);
    let mut out = Vec::new();
    let mut top = 0.0_f32;
    let end = height.max(1.0);
    while top < end - 0.5 {
        let mut bottom = top + page_h;
        if bottom < end {
            // Move up past every line the break would cut, as long as that
            // line starts on this page (a line taller than a page is cut).
            loop {
                let cut = bands
                    .iter()
                    .filter(|(a, b)| *a < bottom - 0.01 && *b > bottom + 0.01 && *a > top + 1.0)
                    .map(|(a, _)| *a)
                    .fold(f32::INFINITY, f32::min);
                if cut.is_finite() && cut < bottom {
                    bottom = cut;
                } else {
                    break;
                }
            }
        } else {
            bottom = end;
        }
        out.push((top, bottom));
        top = bottom;
    }
    if out.is_empty() {
        out.push((0.0, page_h));
    }
    out
}

// ── The writer ───────────────────────────────────────────────────────────

/// One font face the Viewer used, as the PDF carries it.
struct FontUse {
    /// `/F1` …
    res: String,
    id: ObjectId,
    data: Arc<egui::FontData>,
    /// Glyphs used, with the text each stands for.
    used: BTreeMap<u16, String>,
    /// `None` until tried; `Some(false)` — draw this face as outlines.
    embeddable: bool,
    name: String,
}

struct Writer<'a> {
    doc: Document,
    paint: &'a ExportPaint,
    page: PageSetup,
    top: f32,
    fonts: BTreeMap<String, FontUse>,
    /// (family, char) → (font key, glyph id).
    glyph_cache: HashMap<(egui::FontFamily, char), Option<(String, u16)>>,
    gstates: BTreeMap<u8, String>,
    images: HashMap<(egui::TextureId, [u32; 4]), (String, ObjectId)>,
    shadings: Vec<(String, ObjectId)>,
}

impl<'a> Writer<'a> {
    fn new(paint: &'a ExportPaint, page: PageSetup) -> Self {
        Writer {
            doc: Document::with_version("1.5"),
            paint,
            page,
            top: 0.0,
            fonts: BTreeMap::new(),
            glyph_cache: HashMap::new(),
            gstates: BTreeMap::new(),
            images: HashMap::new(),
            shadings: Vec::new(),
        }
    }

    fn x(&self, px: f32) -> f32 {
        self.page.margin_pt + px * PT_PER_PX
    }

    fn y(&self, px: f32) -> f32 {
        self.page.height_pt - self.page.margin_pt - (px - self.top) * PT_PER_PX
    }

    fn page_content(&mut self, top: f32, bottom: f32) -> String {
        self.top = top;
        let mut c = String::new();
        let m = self.page.margin_pt;
        // Clip to the content area: a box crossing the break is cut there.
        c.push_str(&format!(
            "q {} {} {} {} re W n\n",
            num(m),
            num(m),
            num(self.page.width_pt - 2.0 * m),
            num(self.page.height_pt - 2.0 * m)
        ));
        let shapes = &self.paint.shapes;
        for s in shapes {
            self.shape(&mut c, s, top, bottom);
        }
        c.push_str("Q\n");
        c
    }

    fn visible(&self, r: Rect, top: f32, bottom: f32) -> bool {
        r.max.y >= top - 1.0 && r.min.y <= bottom + 1.0
    }

    fn shape(&mut self, c: &mut String, s: &Shape, top: f32, bottom: f32) {
        if let Shape::Vec(v) = s {
            for s in v {
                self.shape(c, s, top, bottom);
            }
            return;
        }
        if !self.visible(s.visual_bounding_rect(), top, bottom) {
            return;
        }
        match s {
            Shape::Rect(r) => self.rect(c, r),
            Shape::Circle(ci) => {
                let rad = egui::vec2(ci.radius, ci.radius);
                self.ellipse(c, ci.center, rad, ci.fill, ci.stroke);
            }
            Shape::Ellipse(e) => self.ellipse(c, e.center, e.radius, e.fill, e.stroke),
            Shape::LineSegment { points, stroke } => {
                if self.stroke_colour(c, stroke.color, stroke.width) {
                    c.push_str(&format!(
                        "{} {} m {} {} l S\n",
                        num(self.x(points[0].x)),
                        num(self.y(points[0].y)),
                        num(self.x(points[1].x)),
                        num(self.y(points[1].y))
                    ));
                }
            }
            Shape::Path(p) => {
                if p.points.len() < 2 {
                    return;
                }
                let mut path = format!("{} {} m ", num(self.x(p.points[0].x)), num(self.y(p.points[0].y)));
                for q in &p.points[1..] {
                    path.push_str(&format!("{} {} l ", num(self.x(q.x)), num(self.y(q.y))));
                }
                if p.closed {
                    path.push_str("h ");
                }
                if p.closed && self.fill_colour(c, p.fill) {
                    c.push_str(&path);
                    c.push_str("f\n");
                }
                if let egui::epaint::ColorMode::Solid(col) = p.stroke.color {
                    if self.stroke_colour(c, col, p.stroke.width) {
                        c.push_str(&path);
                        c.push_str("S\n");
                    }
                }
            }
            Shape::Mesh(mesh) => self.mesh(c, mesh),
            Shape::Text(t) => self.text(c, t, top, bottom),
            Shape::QuadraticBezier(q) => {
                let [p0, p1, p2] = q.points;
                let c1 = p0 + (p1 - p0) * (2.0 / 3.0);
                let c2 = p2 + (p1 - p2) * (2.0 / 3.0);
                self.bezier(c, &[p0, c1, c2, p2], q.closed, q.fill, q.stroke.width, &q.stroke.color);
            }
            Shape::CubicBezier(b) => {
                self.bezier(c, &b.points, b.closed, b.fill, b.stroke.width, &b.stroke.color);
            }
            _ => {}
        }
    }

    fn bezier(&mut self, c: &mut String, p: &[Pos2; 4], closed: bool, fill: Color32, width: f32, colour: &egui::epaint::ColorMode) {
        let path = format!(
            "{} {} m {} {} {} {} {} {} c {}",
            num(self.x(p[0].x)),
            num(self.y(p[0].y)),
            num(self.x(p[1].x)),
            num(self.y(p[1].y)),
            num(self.x(p[2].x)),
            num(self.y(p[2].y)),
            num(self.x(p[3].x)),
            num(self.y(p[3].y)),
            if closed { "h " } else { "" }
        );
        if closed && self.fill_colour(c, fill) {
            c.push_str(&path);
            c.push_str("f\n");
        }
        if let egui::epaint::ColorMode::Solid(col) = colour {
            if self.stroke_colour(c, *col, width) {
                c.push_str(&path);
                c.push_str("S\n");
            }
        }
    }

    /// Set the fill colour (and its alpha); `false` when there is nothing to
    /// fill with.
    fn fill_colour(&mut self, c: &mut String, col: Color32) -> bool {
        let [r, g, b, a] = col.to_srgba_unmultiplied();
        if a == 0 {
            return false;
        }
        let gs = self.gstate(a);
        c.push_str(&format!("/{gs} gs {} {} {} rg\n", unit(r), unit(g), unit(b)));
        true
    }

    fn stroke_colour(&mut self, c: &mut String, col: Color32, width: f32) -> bool {
        let [r, g, b, a] = col.to_srgba_unmultiplied();
        if a == 0 || width <= 0.0 {
            return false;
        }
        let gs = self.gstate(a);
        c.push_str(&format!("/{gs} gs {} {} {} RG {} w\n", unit(r), unit(g), unit(b), num(width * PT_PER_PX)));
        true
    }

    fn gstate(&mut self, alpha: u8) -> String {
        let n = self.gstates.len();
        self.gstates.entry(alpha).or_insert_with(|| format!("GS{n}")).clone()
    }

    /// A rounded rectangle's path, corners `[nw, ne, se, sw]`, in pixels.
    fn rrect_path(&self, r: Rect, radii: [f32; 4]) -> String {
        let max = (r.width().min(r.height()) / 2.0).max(0.0);
        let [nw, ne, se, sw] = radii.map(|v| v.clamp(0.0, max));
        let k = 0.552_284_8;
        let (x0, x1, y0, y1) = (r.min.x, r.max.x, r.min.y, r.max.y);
        let p = |x: f32, y: f32| format!("{} {}", num(self.x(x)), num(self.y(y)));
        let mut s = format!("{} m ", p(x0 + nw, y0));
        s.push_str(&format!("{} l ", p(x1 - ne, y0)));
        if ne > 0.0 {
            s.push_str(&format!("{} {} {} c ", p(x1 - ne + ne * k, y0), p(x1, y0 + ne - ne * k), p(x1, y0 + ne)));
        }
        s.push_str(&format!("{} l ", p(x1, y1 - se)));
        if se > 0.0 {
            s.push_str(&format!("{} {} {} c ", p(x1, y1 - se + se * k), p(x1 - se + se * k, y1), p(x1 - se, y1)));
        }
        s.push_str(&format!("{} l ", p(x0 + sw, y1)));
        if sw > 0.0 {
            s.push_str(&format!("{} {} {} c ", p(x0 + sw - sw * k, y1), p(x0, y1 - sw + sw * k), p(x0, y1 - sw)));
        }
        s.push_str(&format!("{} l ", p(x0, y0 + nw)));
        if nw > 0.0 {
            s.push_str(&format!("{} {} {} c ", p(x0, y0 + nw - nw * k), p(x0 + nw - nw * k, y0), p(x0 + nw, y0)));
        }
        s.push_str("h ");
        s
    }

    fn rect(&mut self, c: &mut String, r: &egui::epaint::RectShape) {
        let cr = r.corner_radius;
        let radii = [cr.nw as f32, cr.ne as f32, cr.se as f32, cr.sw as f32];
        if let Some(brush) = &r.brush {
            self.image(c, brush.fill_texture_id, r.rect, brush.uv);
            return;
        }
        if r.blur_width > 0.0 {
            // A shadow: soft layers from the blurred edge inwards.
            let [cr_, cg, cb, ca] = r.fill.to_srgba_unmultiplied();
            let layers = 4;
            for i in 0..layers {
                let t = (i as f32 + 0.5) / layers as f32;
                let grow = r.blur_width * (0.5 - t);
                let rr = r.rect.expand(grow);
                let a = ((ca as f32) / layers as f32 * 1.2).min(255.0) as u8;
                let col = Color32::from_rgba_unmultiplied(cr_, cg, cb, a);
                if self.fill_colour(c, col) {
                    let path = self.rrect_path(rr, radii.map(|v| (v + grow).max(0.0)));
                    c.push_str(&path);
                    c.push_str("f\n");
                }
            }
            return;
        }
        if self.fill_colour(c, r.fill) {
            let path = self.rrect_path(r.rect, radii);
            c.push_str(&path);
            c.push_str("f\n");
        }
        if r.stroke.width > 0.0 {
            let half = r.stroke.width / 2.0;
            let rr = match r.stroke_kind {
                egui::StrokeKind::Inside => r.rect.shrink(half),
                egui::StrokeKind::Outside => r.rect.expand(half),
                egui::StrokeKind::Middle => r.rect,
            };
            if self.stroke_colour(c, r.stroke.color, r.stroke.width) {
                let path = self.rrect_path(rr, radii.map(|v| (v - half).max(0.0)));
                c.push_str(&path);
                c.push_str("S\n");
            }
        }
    }

    fn ellipse(&mut self, c: &mut String, centre: Pos2, rad: egui::Vec2, fill: Color32, stroke: egui::Stroke) {
        let r = Rect::from_center_size(centre, rad * 2.0);
        let radii = [rad.x.min(rad.y); 4];
        if self.fill_colour(c, fill) {
            let path = self.rrect_path(r, radii);
            c.push_str(&path);
            c.push_str("f\n");
        }
        if self.stroke_colour(c, stroke.color, stroke.width) {
            let path = self.rrect_path(r, radii);
            c.push_str(&path);
            c.push_str("S\n");
        }
    }

    /// A mesh: a picture when it carries one, otherwise a Gouraud-shaded
    /// triangle mesh — the gradient backgrounds.
    fn mesh(&mut self, c: &mut String, mesh: &egui::epaint::Mesh) {
        if mesh.vertices.is_empty() || mesh.indices.len() < 3 {
            return;
        }
        if mesh.texture_id != egui::TextureId::default() && self.paint.textures.contains_key(&mesh.texture_id) {
            let bounds = mesh.calc_bounds();
            let (mut u0, mut v0, mut u1, mut v1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for v in &mesh.vertices {
                u0 = u0.min(v.uv.x);
                v0 = v0.min(v.uv.y);
                u1 = u1.max(v.uv.x);
                v1 = v1.max(v.uv.y);
            }
            let uv = Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1));
            self.image(c, mesh.texture_id, bounds, uv);
            return;
        }
        // One constant alpha for the mesh: the most opaque vertex's.
        let alpha = mesh.vertices.iter().map(|v| v.color.a()).max().unwrap_or(0);
        if alpha == 0 {
            return;
        }
        let (w, h) = (self.page.width_pt, self.page.height_pt);
        let mut data = Vec::with_capacity(mesh.indices.len() * 12);
        for tri in mesh.indices.chunks_exact(3) {
            for &i in tri {
                let v = &mesh.vertices[i as usize];
                let [r, g, b, _] = v.color.to_srgba_unmultiplied();
                let fx = (self.x(v.pos.x) / w).clamp(0.0, 1.0);
                let fy = (self.y(v.pos.y) / h).clamp(0.0, 1.0);
                data.push(0u8);
                data.extend_from_slice(&((fx as f64 * u32::MAX as f64) as u32).to_be_bytes());
                data.extend_from_slice(&((fy as f64 * u32::MAX as f64) as u32).to_be_bytes());
                data.extend_from_slice(&[r, g, b]);
            }
        }
        let mut stream = Stream::new(
            dictionary! {
                "ShadingType" => 4,
                "ColorSpace" => "DeviceRGB",
                "BitsPerCoordinate" => 32,
                "BitsPerComponent" => 8,
                "BitsPerFlag" => 8,
                "Decode" => vec![0.into(), Object::Real(w as f64), 0.into(), Object::Real(h as f64), 0.into(), 1.into(), 0.into(), 1.into(), 0.into(), 1.into()],
            },
            data,
        );
        let _ = stream.compress();
        let id = self.doc.add_object(stream);
        let name = format!("Sh{}", self.shadings.len());
        self.shadings.push((name.clone(), id));
        let gs = self.gstate(alpha);
        c.push_str(&format!("q /{gs} gs /{name} sh Q\n"));
    }

    /// Draw texture `id`'s `uv` part into `rect`.
    fn image(&mut self, c: &mut String, id: egui::TextureId, rect: Rect, uv: Rect) {
        let Some(img) = self.paint.textures.get(&id) else { return };
        let [iw, ih] = img.size;
        let px = |f: f32, n: usize| ((f.clamp(0.0, 1.0) * n as f32).round() as u32).min(n as u32);
        let key = [px(uv.min.x, iw), px(uv.min.y, ih), px(uv.max.x, iw), px(uv.max.y, ih)];
        let (w, h) = ((key[2] - key[0]) as usize, (key[3] - key[1]) as usize);
        if w == 0 || h == 0 {
            return;
        }
        let name = if let Some((name, _)) = self.images.get(&(id, key)) {
            name.clone()
        } else {
            let mut rgb = Vec::with_capacity(w * h * 3);
            let mut alpha = Vec::with_capacity(w * h);
            let mut opaque = true;
            for y in key[1] as usize..key[3] as usize {
                for x in key[0] as usize..key[2] as usize {
                    let [r, g, b, a] = img.pixels[y * iw + x].to_srgba_unmultiplied();
                    rgb.extend_from_slice(&[r, g, b]);
                    alpha.push(a);
                    opaque &= a == 255;
                }
            }
            let mut dict = dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => w as i64,
                "Height" => h as i64,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
            };
            if !opaque {
                let mut mask = Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Image",
                        "Width" => w as i64,
                        "Height" => h as i64,
                        "ColorSpace" => "DeviceGray",
                        "BitsPerComponent" => 8,
                    },
                    alpha,
                );
                let _ = mask.compress();
                let mask_id = self.doc.add_object(mask);
                dict.set("SMask", mask_id);
            }
            let mut stream = Stream::new(dict, rgb);
            let _ = stream.compress();
            let obj = self.doc.add_object(stream);
            let name = format!("Im{}", self.images.len());
            self.images.insert((id, key), (name.clone(), obj));
            name
        };
        let gs = self.gstate(255);
        c.push_str(&format!(
            "q /{gs} gs {} 0 0 {} {} {} cm /{name} Do Q\n",
            num(rect.width() * PT_PER_PX),
            num(rect.height() * PT_PER_PX),
            num(self.x(rect.min.x)),
            num(self.y(rect.max.y))
        ));
    }

    /// The face that draws `ch` in `family`, as epaint picks it: the first
    /// font in the family's list that has the character.
    fn face_for(&mut self, family: &egui::FontFamily, ch: char) -> Option<(String, u16)> {
        if let Some(hit) = self.glyph_cache.get(&(family.clone(), ch)) {
            return hit.clone();
        }
        let fonts = &self.paint.fonts;
        let mut found = None;
        if let Some(names) = fonts.families.get(family) {
            for name in names {
                let Some(data) = fonts.font_data.get(name) else { continue };
                let Ok(font) = skrifa::FontRef::from_index(&data.font, data.index) else { continue };
                use skrifa::MetadataProvider;
                if let Some(gid) = font.charmap().map(ch) {
                    if gid.to_u32() != 0 {
                        found = Some((name.clone(), gid.to_u32() as u16));
                        break;
                    }
                }
            }
        }
        self.glyph_cache.insert((family.clone(), ch), found.clone());
        found
    }

    fn font_use(&mut self, key: &str) -> &mut FontUse {
        if !self.fonts.contains_key(key) {
            let data = self.paint.fonts.font_data.get(key).cloned().expect("a font the family names");
            let embeddable = subset_truetype(&data.font, data.index, &BTreeSet::from([0u16])).is_some();
            let res = format!("F{}", self.fonts.len());
            let id = self.doc.new_object_id();
            let name: String = key.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
            self.fonts.insert(
                key.to_string(),
                FontUse { res, id, data, used: BTreeMap::new(), embeddable, name: if name.is_empty() { "Font".into() } else { name } },
            );
        }
        self.fonts.get_mut(key).expect("just inserted")
    }

    fn text(&mut self, c: &mut String, t: &egui::epaint::TextShape, top: f32, bottom: f32) {
        let galley = &t.galley;
        let job = &galley.job;
        let text = job.text.as_str();
        // Glyphs come in text order, ONE PER CHARACTER — a line's `\n` is the
        // only character without one (epaint's `Row::glyphs`). So the text,
        // walked alongside, names each glyph's own character and section,
        // even where shaping reports another: a ligature's second letter, or
        // an emoji's variation selector, which comes back as the emoji again.
        let mut chars = text.char_indices().filter(|(_, c)| *c != '\n');
        let section_at = |byte: usize| job.sections.iter().find(|s| s.byte_range.start.0 <= byte && byte < s.byte_range.end.0).or(job.sections.last());
        for row in &galley.rows {
            let row_rect = row.rect().translate(t.pos.to_vec2());
            let visible = row_rect.max.y >= top - 1.0 && row_rect.min.y <= bottom + 1.0;
            // Where the previous glyph ends, at its own advance in its font.
            let mut prev_end: Option<f32> = None;
            for glyph in &row.glyphs {
                let (byte, source) = chars.next().unwrap_or((text.len(), glyph.chr));
                if !visible {
                    continue;
                }
                let Some(section) = section_at(byte) else { continue };
                let format = &section.format;
                let mut colour = t.override_text_color.unwrap_or(format.color);
                if colour == Color32::PLACEHOLDER {
                    colour = t.fallback_color;
                }
                if t.opacity_factor < 1.0 {
                    colour = colour.gamma_multiply(t.opacity_factor);
                }
                let mut gx = t.pos.x + row.pos.x + glyph.pos.x;
                // A ligature: egui shaped "fi" into ONE glyph at the "f" and
                // gave the "i" no width. The PDF sets the letters themselves,
                // so the "i" goes where the "f" ends — not on top of the next
                // letter, which is where its zero-width slot is.
                // The character this glyph draws: its own, unless shaping
                // folded it into the one before (a ligature).
                let mut draw = glyph.chr;
                if glyph.advance_width == 0.0 && !is_combining(source) {
                    if is_invisible(source) {
                        continue;
                    }
                    draw = source;
                    if let Some(end) = prev_end {
                        gx = end;
                    }
                }
                let base_y = t.pos.y + row.pos.y + glyph.pos.y;
                // What sits under and through the text: a highlight, an
                // underline, a strike.
                if format.background != Color32::TRANSPARENT {
                    let r = Rect::from_min_size(egui::pos2(gx, t.pos.y + row.pos.y), egui::vec2(glyph.advance_width, glyph.line_height));
                    if self.fill_colour(c, format.background) {
                        let path = self.rrect_path(r, [0.0; 4]);
                        c.push_str(&path);
                        c.push_str("f\n");
                    }
                }
                for (stroke, dy) in [(format.underline, 0.12), (format.strikethrough, -0.3)] {
                    if stroke.width > 0.0 && self.stroke_colour(c, stroke.color, stroke.width) {
                        let yy = base_y + dy * format.font_id.size;
                        c.push_str(&format!(
                            "{} {} m {} {} l S\n",
                            num(self.x(gx)),
                            num(self.y(yy)),
                            num(self.x(gx + glyph.advance_width)),
                            num(self.y(yy))
                        ));
                    }
                }
                if draw.is_whitespace() && draw != ' ' {
                    continue;
                }
                let Some((key, gid)) = self.face_for(&format.font_id.family, draw) else { continue };
                let tweak = self.paint.fonts.font_data.get(&key).map(|d| d.tweak.clone()).unwrap_or_default();
                let size = format.font_id.size * tweak.scale;
                let y = base_y + tweak.y_offset_factor * size + tweak.y_offset;
                if !self.fill_colour(c, colour) {
                    continue;
                }
                prev_end = Some(gx + font_advance(&self.paint.fonts, &key, gid, size));
                let fu = self.font_use(&key);
                fu.used.entry(gid).or_insert_with(|| draw.to_string());
                if fu.embeddable {
                    let res = fu.res.clone();
                    c.push_str(&format!(
                        "BT /{res} {} Tf 1 0 0 1 {} {} Tm <{:04X}> Tj ET\n",
                        num(size * PT_PER_PX),
                        num(self.x(gx)),
                        num(self.y(y)),
                        gid
                    ));
                } else {
                    let data = fu.data.clone();
                    let path = glyph_outline(&data, gid, size, |px, py| {
                        (self.x(gx + px), self.y(y - py))
                    });
                    if !path.is_empty() {
                        c.push_str(&path);
                        c.push_str("f\n");
                    }
                }
            }
        }
    }

    /// The shared resources: every font (embedded now that the glyphs used
    /// are known), transparency state, picture and shading.
    fn finish_resources(&mut self) -> lopdf::Dictionary {
        let mut fonts = lopdf::Dictionary::new();
        let keys: Vec<String> = self.fonts.keys().cloned().collect();
        for key in keys {
            let fu = self.fonts.remove(&key).expect("listed");
            if fu.embeddable {
                if let Some(font) = self.embed_font(&fu) {
                    self.doc.objects.insert(fu.id, font);
                    fonts.set(fu.res.clone(), fu.id);
                }
            }
        }
        let mut gs = lopdf::Dictionary::new();
        for (alpha, name) in &self.gstates {
            let a = *alpha as f64 / 255.0;
            gs.set(name.clone(), Object::Dictionary(dictionary! { "Type" => "ExtGState", "ca" => Object::Real(a), "CA" => Object::Real(a) }));
        }
        let mut xobjects = lopdf::Dictionary::new();
        for (name, id) in self.images.values() {
            xobjects.set(name.clone(), *id);
        }
        let mut shadings = lopdf::Dictionary::new();
        for (name, id) in &self.shadings {
            shadings.set(name.clone(), *id);
        }
        dictionary! {
            "Font" => fonts,
            "ExtGState" => gs,
            "XObject" => xobjects,
            "Shading" => shadings,
        }
    }

    /// A Type0 / CIDFontType2 font: the subset face, its widths and a
    /// `ToUnicode` map so the text copies and searches as what it says.
    fn embed_font(&mut self, fu: &FontUse) -> Option<Object> {
        let mut used: BTreeSet<u16> = fu.used.keys().copied().collect();
        used.insert(0);
        let bytes = subset_truetype(&fu.data.font, fu.data.index, &used)?;
        let face = Face::parse(&fu.data.font, fu.data.index)?;
        let upem = face.u16("head", 18)? as f32;
        let scale = |v: f32| (v * 1000.0 / upem).round() as i64;
        let bbox: Vec<Object> = [36, 38, 40, 42].iter().map(|o| Object::Integer(scale(face.i16("head", *o).unwrap_or(0) as f32))).collect();
        let ascent = scale(face.i16("hhea", 4).unwrap_or(800) as f32);
        let descent = scale(face.i16("hhea", 6).unwrap_or(-200) as f32);
        let tag = subset_tag(&fu.name);
        let base = format!("{tag}+{}", fu.name);
        let len = bytes.len() as i64;
        let mut file = Stream::new(dictionary! { "Length1" => len }, bytes);
        let _ = file.compress();
        let file_id = self.doc.add_object(file);
        let descriptor = self.doc.add_object(dictionary! {
            "Type" => "FontDescriptor",
            "FontName" => Object::Name(base.clone().into_bytes()),
            "Flags" => 4,
            "FontBBox" => bbox,
            "ItalicAngle" => 0,
            "Ascent" => ascent,
            "Descent" => descent,
            "CapHeight" => ascent,
            "StemV" => 80,
            "FontFile2" => file_id,
        });
        let mut widths: Vec<Object> = Vec::new();
        for gid in fu.used.keys() {
            widths.push(Object::Integer(*gid as i64));
            widths.push(Object::Array(vec![Object::Integer(scale(face.advance(*gid) as f32))]));
        }
        let cid = self.doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "CIDFontType2",
            "BaseFont" => Object::Name(base.clone().into_bytes()),
            "CIDSystemInfo" => dictionary! { "Registry" => Object::string_literal("Adobe"), "Ordering" => Object::string_literal("Identity"), "Supplement" => 0 },
            "FontDescriptor" => descriptor,
            "W" => widths,
            "CIDToGIDMap" => "Identity",
        });
        let mut cmap = String::from(
            "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
        );
        let entries: Vec<(&u16, &String)> = fu.used.iter().collect();
        for chunk in entries.chunks(100) {
            cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
            for (gid, text) in chunk {
                let hex: String = text.encode_utf16().map(|u| format!("{u:04X}")).collect();
                cmap.push_str(&format!("<{gid:04X}> <{hex}>\n"));
            }
            cmap.push_str("endbfchar\n");
        }
        cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
        let mut to_unicode = Stream::new(dictionary! {}, cmap.into_bytes());
        let _ = to_unicode.compress();
        let to_unicode = self.doc.add_object(to_unicode);
        Some(Object::Dictionary(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type0",
            "BaseFont" => Object::Name(base.into_bytes()),
            "Encoding" => "Identity-H",
            "DescendantFonts" => vec![Object::Reference(cid)],
            "ToUnicode" => to_unicode,
        }))
    }
}

/// Whether `c` is a combining mark — the one kind of glyph that is MEANT to
/// have no width and sit on the one before it.
fn is_combining(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

/// Characters that draw nothing: variation selectors and the joiners.
fn is_invisible(c: char) -> bool {
    matches!(c as u32, 0xFE00..=0xFE0F | 0xE0100..=0xE01EF | 0x200B..=0x200D | 0x2060 | 0xFEFF)
}

/// A glyph's advance in its font at `size` pixels.
fn font_advance(fonts: &egui::FontDefinitions, key: &str, gid: u16, size: f32) -> f32 {
    use skrifa::MetadataProvider;
    let Some(data) = fonts.font_data.get(key) else { return 0.0 };
    let Ok(font) = skrifa::FontRef::from_index(&data.font, data.index) else { return 0.0 };
    font.glyph_metrics(skrifa::instance::Size::new(size), skrifa::instance::LocationRef::default())
        .advance_width(skrifa::GlyphId::new(gid as u32))
        .unwrap_or(0.0)
}

/// A number as a content stream writes it.
fn num(v: f32) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" { "0".into() } else { s.to_string() }
}

fn unit(v: u8) -> String {
    num(v as f32 / 255.0)
}

/// The six-letter subset prefix PDF names a subset face with.
fn subset_tag(name: &str) -> String {
    let mut h: u32 = 2166136261;
    for b in name.bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    (0..6).map(|i| (b'A' + ((h >> (i * 5)) % 26) as u8) as char).collect()
}

/// A glyph's outline as a path, for a face that cannot be embedded.
fn glyph_outline(data: &egui::FontData, gid: u16, size: f32, map: impl Fn(f32, f32) -> (f32, f32)) -> String {
    use skrifa::instance::{LocationRef, Size};
    use skrifa::outline::{DrawSettings, OutlinePen};
    use skrifa::MetadataProvider;
    struct Pen<F: Fn(f32, f32) -> (f32, f32)> {
        out: String,
        map: F,
        last: (f32, f32),
    }
    impl<F: Fn(f32, f32) -> (f32, f32)> Pen<F> {
        fn p(&self, x: f32, y: f32) -> String {
            let (a, b) = (self.map)(x, y);
            format!("{} {}", num(a), num(b))
        }
    }
    impl<F: Fn(f32, f32) -> (f32, f32)> OutlinePen for Pen<F> {
        fn move_to(&mut self, x: f32, y: f32) {
            let s = format!("{} m ", self.p(x, y));
            self.out.push_str(&s);
            self.last = (x, y);
        }
        fn line_to(&mut self, x: f32, y: f32) {
            let s = format!("{} l ", self.p(x, y));
            self.out.push_str(&s);
            self.last = (x, y);
        }
        fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
            let (x0, y0) = self.last;
            let c1 = (x0 + (cx - x0) * 2.0 / 3.0, y0 + (cy - y0) * 2.0 / 3.0);
            let c2 = (x + (cx - x) * 2.0 / 3.0, y + (cy - y) * 2.0 / 3.0);
            let s = format!("{} {} {} c ", self.p(c1.0, c1.1), self.p(c2.0, c2.1), self.p(x, y));
            self.out.push_str(&s);
            self.last = (x, y);
        }
        fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
            let s = format!("{} {} {} c ", self.p(cx0, cy0), self.p(cx1, cy1), self.p(x, y));
            self.out.push_str(&s);
            self.last = (x, y);
        }
        fn close(&mut self) {
            self.out.push_str("h ");
        }
    }
    let Ok(font) = skrifa::FontRef::from_index(&data.font, data.index) else { return String::new() };
    let outlines = font.outline_glyphs();
    let Some(glyph) = outlines.get(skrifa::GlyphId::new(gid as u32)) else { return String::new() };
    let mut pen = Pen { out: String::new(), map, last: (0.0, 0.0) };
    let settings = DrawSettings::unhinted(Size::new(size), LocationRef::default());
    if glyph.draw(settings, &mut pen).is_err() {
        return String::new();
    }
    pen.out
}

// ── TrueType subsetting ──────────────────────────────────────────────────

/// One face of a font file (a collection's too): its tables, by tag.
struct Face<'a> {
    data: &'a [u8],
    tables: BTreeMap<[u8; 4], (usize, usize)>,
}

fn be16(d: &[u8], o: usize) -> Option<u16> {
    d.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]))
}

fn be32(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

impl<'a> Face<'a> {
    fn parse(data: &'a [u8], index: u32) -> Option<Face<'a>> {
        let start = if data.get(0..4)? == b"ttcf" {
            be32(data, 12 + 4 * index as usize)? as usize
        } else {
            0
        };
        let n = be16(data, start + 4)? as usize;
        let mut tables = BTreeMap::new();
        for i in 0..n {
            let rec = start + 12 + i * 16;
            let tag: [u8; 4] = data.get(rec..rec + 4)?.try_into().ok()?;
            let off = be32(data, rec + 8)? as usize;
            let len = be32(data, rec + 12)? as usize;
            if off + len <= data.len() {
                tables.insert(tag, (off, len));
            }
        }
        Some(Face { data, tables })
    }

    fn table(&self, tag: &str) -> Option<&'a [u8]> {
        let t: [u8; 4] = tag.as_bytes().try_into().ok()?;
        let (o, l) = self.tables.get(&t)?;
        self.data.get(*o..*o + *l)
    }

    fn u16(&self, tag: &str, off: usize) -> Option<u16> {
        be16(self.table(tag)?, off)
    }

    fn i16(&self, tag: &str, off: usize) -> Option<i16> {
        self.u16(tag, off).map(|v| v as i16)
    }

    fn advance(&self, gid: u16) -> u16 {
        let n = self.u16("hhea", 34).unwrap_or(1).max(1);
        let Some(hmtx) = self.table("hmtx") else { return 0 };
        let i = gid.min(n - 1) as usize;
        be16(hmtx, i * 4).unwrap_or(0)
    }
}

/// A TrueType face (`glyf` outlines) reduced to the glyphs `used` — every
/// other glyph emptied, the glyph numbering kept — as a standalone font file.
/// `None` for a face without `glyf` (CFF), which is drawn as outlines.
fn subset_truetype(data: &[u8], index: u32, used: &BTreeSet<u16>) -> Option<Vec<u8>> {
    let face = Face::parse(data, index)?;
    let glyf = face.table("glyf")?;
    let loca = face.table("loca")?;
    let head = face.table("head")?;
    let num_glyphs = face.u16("maxp", 4)? as usize;
    let long = be16(head, 50)? == 1;
    let offset = |g: usize| -> Option<usize> {
        if long {
            be32(loca, g * 4).map(|v| v as usize)
        } else {
            be16(loca, g * 2).map(|v| v as usize * 2)
        }
    };
    // Composite glyphs bring their components.
    let mut keep: BTreeSet<u16> = used.iter().copied().filter(|g| (*g as usize) < num_glyphs).collect();
    let mut work: Vec<u16> = keep.iter().copied().collect();
    while let Some(g) = work.pop() {
        let (a, b) = (offset(g as usize)?, offset(g as usize + 1)?);
        let Some(bytes) = glyf.get(a..b) else { continue };
        if bytes.len() < 10 || (be16(bytes, 0)? as i16) >= 0 {
            continue;
        }
        let mut p = 10;
        loop {
            let (Some(flags), Some(comp)) = (be16(bytes, p), be16(bytes, p + 2)) else { break };
            if keep.insert(comp) {
                work.push(comp);
            }
            p += 4 + if flags & 1 != 0 { 4 } else { 2 };
            p += if flags & 8 != 0 {
                2
            } else if flags & 0x40 != 0 {
                4
            } else if flags & 0x80 != 0 {
                8
            } else {
                0
            };
            if flags & 0x20 == 0 {
                break;
            }
        }
    }
    let mut new_glyf = Vec::new();
    let mut new_loca = Vec::with_capacity((num_glyphs + 1) * 4);
    for g in 0..num_glyphs {
        new_loca.extend_from_slice(&(new_glyf.len() as u32).to_be_bytes());
        if keep.contains(&(g as u16)) {
            let (a, b) = (offset(g)?, offset(g + 1)?);
            if let Some(bytes) = glyf.get(a..b.max(a)) {
                new_glyf.extend_from_slice(bytes);
                while new_glyf.len() % 4 != 0 {
                    new_glyf.push(0);
                }
            }
        }
    }
    new_loca.extend_from_slice(&(new_glyf.len() as u32).to_be_bytes());
    let mut new_head = head.to_vec();
    new_head.get_mut(8..12)?.copy_from_slice(&[0, 0, 0, 0]);
    new_head.get_mut(50..52)?.copy_from_slice(&1u16.to_be_bytes());
    let mut tables: Vec<([u8; 4], Vec<u8>)> = vec![
        (*b"glyf", new_glyf),
        (*b"head", new_head),
        (*b"loca", new_loca),
    ];
    for tag in ["hhea", "hmtx", "maxp", "cvt ", "fpgm", "prep", "OS/2"] {
        if let Some(t) = face.table(tag) {
            tables.push((tag.as_bytes().try_into().ok()?, t.to_vec()));
        }
    }
    tables.sort_by(|a, b| a.0.cmp(&b.0));
    Some(write_sfnt(&tables))
}

fn checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    for chunk in data.chunks(4) {
        let mut b = [0u8; 4];
        b[..chunk.len()].copy_from_slice(chunk);
        sum = sum.wrapping_add(u32::from_be_bytes(b));
    }
    sum
}

fn write_sfnt(tables: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let n = tables.len() as u16;
    let mut pow = 1u16;
    let mut log = 0u16;
    while pow * 2 <= n {
        pow *= 2;
        log += 1;
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&n.to_be_bytes());
    out.extend_from_slice(&(pow * 16).to_be_bytes());
    out.extend_from_slice(&log.to_be_bytes());
    out.extend_from_slice(&(n * 16 - pow * 16).to_be_bytes());
    let mut offset = 12 + 16 * tables.len();
    let mut body = Vec::new();
    for (tag, data) in tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum(data).to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        body.extend_from_slice(data);
        while body.len() % 4 != 0 {
            body.push(0);
        }
        offset = 12 + 16 * tables.len() + body.len();
    }
    out.extend_from_slice(&body);
    // head.checkSumAdjustment: the whole file sums to 0xB1B0AFBA.
    let adjust = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
    if let Some(pos) = tables.iter().position(|(t, _)| t == b"head") {
        let rec = 12 + 16 * pos;
        let off = u32::from_be_bytes([out[rec + 8], out[rec + 9], out[rec + 10], out[rec + 11]]) as usize;
        out[off + 8..off + 12].copy_from_slice(&adjust.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A subset keeps the glyphs asked for (and their components), empties
    /// the rest, and is still a font a parser reads — with the same glyph
    /// numbers, so the page's glyph ids still name the right shapes.
    #[test]
    fn a_subset_face_keeps_its_glyphs_and_their_numbers() {
        let defs = egui::FontDefinitions::default();
        let (name, data) = defs.font_data.iter().next().expect("egui ships a font");
        use skrifa::MetadataProvider;
        let font = skrifa::FontRef::from_index(&data.font, data.index).expect("parses");
        let gid = font.charmap().map('A').expect("has A").to_u32() as u16;
        let full = data.font.len();
        let sub = subset_truetype(&data.font, data.index, &BTreeSet::from([0, gid])).expect("a glyf face");
        let parsed = skrifa::FontRef::new(&sub).expect("the subset parses");
        let glyph = parsed.outline_glyphs().get(skrifa::GlyphId::new(gid as u32)).expect("A is still there");
        let mut count = 0;
        struct Count<'a>(&'a mut usize);
        impl skrifa::outline::OutlinePen for Count<'_> {
            fn move_to(&mut self, _: f32, _: f32) { *self.0 += 1 }
            fn line_to(&mut self, _: f32, _: f32) { *self.0 += 1 }
            fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) { *self.0 += 1 }
            fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) { *self.0 += 1 }
            fn close(&mut self) {}
        }
        glyph
            .draw(
                skrifa::outline::DrawSettings::unhinted(skrifa::instance::Size::new(16.0), skrifa::instance::LocationRef::default()),
                &mut Count(&mut count),
            )
            .expect("draws");
        println!("  {name}: {full} bytes → subset of 2 glyphs {} bytes; 'A' draws with {count} segments", sub.len());
        assert!(count > 3, "the kept glyph has its outline");
        assert!(sub.len() * 4 < full, "most of the face is gone");
    }

    /// A page break never cuts a line: it moves up to the line's top.
    #[test]
    fn a_page_break_moves_up_past_a_line_it_would_cut() {
        let ctx = egui::Context::default();
        let mut shapes = Vec::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let mut job = egui::text::LayoutJob::default();
            job.append(&"line\n".repeat(40), 0.0, egui::TextFormat { font_id: egui::FontId::proportional(20.0), ..Default::default() });
            let galley = ui.fonts_mut(|f| f.layout_job(job));
            shapes.push(Shape::galley(Pos2::ZERO, galley, Color32::BLACK));
        });
        out.textures_delta.clear();
        let ranges = page_ranges(&shapes, 40.0 * 30.0, 250.0);
        let mut bands = Vec::new();
        unbreakable_bands(&shapes, &mut bands);
        for (_, bottom) in &ranges[..ranges.len() - 1] {
            assert!(
                bands.iter().all(|(a, b)| !(*a < bottom - 0.01 && *b > bottom + 0.01)),
                "no line crosses the break at {bottom}"
            );
        }
        println!("  {} pages for 40 lines at 250 px a page: {:?}", ranges.len(), ranges);
        assert!(ranges.len() >= 3);
    }
}
