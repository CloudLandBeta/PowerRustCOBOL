// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! One band of the canvas: a horizontal strip of pixels with the complete
//! graphics state needed to execute a display list on it.
//!
//! A canvas below the tiling threshold is one band. A large canvas is cut into
//! bands of a fixed number of rows (a function of the canvas size only, never of
//! the thread count) and every band executes the **whole** list, drawing only
//! what falls in its rows; the bands run on separate threads when there are
//! any. Because a band keeps its own state and does exactly the work the untiled
//! canvas would do for those rows, the result is the same for any number of
//! threads, including one (WASM-018 R20).

use std::collections::HashMap;
use std::sync::Arc;

use kurbo::{Affine, BezPath, Point, Shape};
use tiny_skia::{IntRect, Mask, Path, Pixmap, Transform};

use super::flatten::{map_to_tiny, to_tiny};
use super::paint::*;
use super::path::circle_path;
use super::state::*;
use crate::color::Rgba;
use crate::display_list::consts::*;
use crate::display_list::{Command, DisplayList};
use crate::error::Result;
use crate::limits::Limits;
use crate::resources::Resources;
use crate::textapi::{align_fraction, baseline_shift, metrics_for, FontSpec, TextBlock, TextMetrics, TextRequest};

/// Read-only context shared by all bands during one `execute`.
pub(crate) struct RunCtx<'a> {
    pub res: &'a dyn Resources,
    pub images: &'a HashMap<u32, Arc<Pixmap>>,
    pub limits: &'a Limits,
    pub ratio: f32,
    /// Canvas size in device pixels.
    pub canvas: (u32, u32),
}

/// One band's share of an answer to a read-back command.
pub(crate) enum PartRead {
    Pixel { slot: u32, color: Option<Rgba> },
    Hit { slot: u32, inside: bool },
    Text { slot: u32, metrics: TextMetrics },
    Capture { slot: u32, rect: (i32, i32, u32, u32), rows: Option<(u32, Vec<u8>)> },
    /// This band does not answer this one (another band does).
    Skip,
}

#[derive(Default)]
pub(crate) struct BandOutcome {
    pub reads: Vec<PartRead>,
    pub presented: bool,
    pub diagnostics: Vec<String>,
    pub executed: u64,
}

impl BandOutcome {
    fn note(&mut self, msg: String) {
        if self.diagnostics.len() < 64 {
            self.diagnostics.push(msg);
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Which {
    Fill,
    Stroke,
}

pub(crate) struct Band {
    pub y0: u32,
    pub w: u32,
    pub h: u32,
    pub pm: Pixmap,
    pub st: ExecState,
    pub primary: bool,
    /// The grid (a power of two, in device pixels) device coordinates are snapped
    /// to before they are shifted into this band: fine enough that the snap is
    /// invisible, coarse enough that `y - y0` is exact in `f32` for every band.
    pub grid: f64,
}

macro_rules! finite {
    ($($v:expr),+ $(,)?) => {
        if !($($v.is_finite())&&+) { return Ok(()); }
    };
}

fn rect_bez(m: Affine, x: f64, y: f64, w: f64, h: f64) -> BezPath {
    let mut b = BezPath::new();
    b.move_to(m * Point::new(x, y));
    b.line_to(m * Point::new(x + w, y));
    b.line_to(m * Point::new(x + w, y + h));
    b.line_to(m * Point::new(x, y + h));
    b.close_path();
    b
}

impl Band {
    /// `canvas` is the whole canvas in device pixels (it fixes the snapping grid).
    pub fn new(y0: u32, w: u32, h: u32, canvas: (u32, u32), base: Affine, primary: bool) -> Option<Band> {
        // 24 bits of f32 mantissa: a coordinate below 2^k can carry 24 - k fractional bits.
        let k = (32 - canvas.0.max(canvas.1).max(1).leading_zeros()) as i32; // coordinates < 2^k
        let grid = 2.0f64.powi(k + 1 - 24);
        Some(Band { y0, w, h, pm: Pixmap::new(w, h)?, st: ExecState::new(base), primary, grid })
    }

    fn dev_path(&self, bez: &BezPath) -> Option<Path> {
        to_tiny(bez, self.y0 as f64, self.grid)
    }

    fn map_dev(&self, path: &Path, m: Affine) -> Option<Path> {
        map_to_tiny(path, m, self.y0 as f64, self.grid)
    }

    fn shader_ts(&self) -> Transform {
        to_ts(self.st.cur.ctm).post_translate(0.0, -(self.y0 as f32))
    }

    /// Execute every command of the list on this band.
    pub fn run(&mut self, list: &DisplayList<'_>, ctx: &RunCtx<'_>) -> Result<BandOutcome> {
        let mut out = BandOutcome::default();
        for cmd in list.commands() {
            self.exec(cmd?, ctx, &mut out)?;
            out.executed += 1;
        }
        Ok(out)
    }

    // ------------------------------------------------------------------
    // Painting entry points
    // ------------------------------------------------------------------

    fn paint(&mut self, path: &Path, which: Which, rule: tiny_skia::FillRule, ctx: &RunCtx<'_>) {
        let src = match which {
            Which::Fill => self.st.cur.fill.clone(),
            Which::Stroke => self.st.cur.stroke.clone(),
        };
        let ts = self.shader_ts();
        self.paint_src(path, &src, rule, ts, ctx);
    }

    /// Fill a band-space path with a paint, honouring clip, alpha, blend mode and shadow.
    fn paint_src(&mut self, path: &Path, src: &PaintSrc, rule: tiny_skia::FillRule, shader_ts: Transform, ctx: &RunCtx<'_>) {
        let g = &self.st.cur;
        let (alpha, blend, quality) = (g.alpha, g.blend, quality_for(g.smoothing));
        let mask = g.clip.as_deref();
        let b = path.bounds();
        let (w, h) = (self.w as f32, self.h as f32);
        let shape_visible = b.right() > -2.0 && b.bottom() > -2.0 && b.left() < w + 2.0 && b.top() < h + 2.0;
        let shadow = shadow_params(&g.shadow, ctx.ratio);
        if let Some((dx, dy, sigma)) = shadow {
            let m = (3.0 * sigma) as f32 + 2.0;
            let (sl, st_, sr, sb) = (b.left() + dx as f32 - m, b.top() + dy as f32 - m, b.right() + dx as f32 + m, b.bottom() + dy as f32 + m);
            if sr > 0.0 && sb > 0.0 && sl < w && st_ < h {
                draw_shadow(
                    &mut self.pm,
                    &ShadowJob {
                        shadow: g.shadow,
                        ratio: ctx.ratio,
                        path,
                        src,
                        rule,
                        shader_ts,
                        alpha,
                        quality,
                        blend: sk_blend(blend),
                        mask,
                        band_w: self.w,
                        band_h: self.h,
                    },
                );
            }
        }
        if !shape_visible {
            return;
        }
        if is_unbounded(blend) {
            let Some(mut layer) = new_layer(self.w, self.h) else { return };
            raster(&mut layer, path, src, rule, shader_ts, alpha, tiny_skia::BlendMode::SourceOver, quality, None);
            composite_layer(&mut self.pm, &layer, 0, 0, sk_blend(blend), mask);
        } else {
            raster(&mut self.pm, path, src, rule, shader_ts, alpha, sk_blend(blend), quality, mask);
        }
    }

    /// Stroke a path given in **user** space with the current line style.
    fn stroke_user_path(&mut self, user: &Path, ctx: &RunCtx<'_>) {
        let g = &self.st.cur;
        if !(g.line_width.is_finite() && g.line_width > 0.0) {
            return;
        }
        let ctm = g.ctm;
        let ts = to_ts(ctm);
        let stroke = make_stroke(g.line_width, g.cap, g.join, g.miter_limit, &g.dash, g.dash_offset);
        let res_scale = stroke_resolution(&ts);
        let Some(outline) = user.stroke(&stroke, res_scale) else { return };
        // The outline's curves are flattened in device space, in f64 (see flatten.rs).
        let Some(dev) = self.map_dev(&outline, ctm) else { return };
        self.paint(&dev, Which::Stroke, tiny_skia::FillRule::Winding, ctx);
    }

    fn stroke_user_bez(&mut self, user: &BezPath, ctx: &RunCtx<'_>) {
        if let Some(p) = curves_to_tiny(user) {
            self.stroke_user_path(&p, ctx);
        }
    }

    /// Stroke a path given in **device** space (the current path): back to the
    /// user space of the transform in force now, stroke there, forward again.
    fn stroke_device_bez(&mut self, dev: &BezPath, ctx: &RunCtx<'_>) {
        let inv = self.st.cur.ctm.inverse();
        if !inv.is_finite() {
            return;
        }
        let mut user = dev.clone();
        user.apply_affine(inv);
        self.stroke_user_bez(&user, ctx);
    }

    fn fill_device_bez(&mut self, dev: &BezPath, rule: tiny_skia::FillRule, ctx: &RunCtx<'_>) {
        if let Some(p) = self.dev_path(dev) {
            self.paint(&p, Which::Fill, rule, ctx);
        }
    }

    // ------------------------------------------------------------------
    // Text
    // ------------------------------------------------------------------

    fn layout_text(&self, text: &str, wrap: Option<(f32, f32)>, ctx: &RunCtx<'_>, out: &mut BandOutcome) -> Result<Option<TextBlock>> {
        ctx.limits.check_text(text.len())?;
        let Some(engine) = ctx.res.text_engine() else {
            out.note("text was not drawn: this canvas has no fonts".to_string());
            return Ok(None);
        };
        let g = &self.st.cur;
        let req = TextRequest {
            text,
            font: &g.font,
            direction: g.direction,
            wrap_width: wrap.map(|w| w.0),
            line_height: wrap.and_then(|w| (w.1 > 0.0).then_some(w.1)),
        };
        match engine.layout(&req) {
            Ok(b) => Ok(Some(b)),
            Err(e) => {
                out.note(format!("text was not drawn: {e}"));
                Ok(None)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_text(&mut self, text: &str, x: f32, y: f32, max_width: f32, wrap: Option<(f32, f32)>, mode: u8, ctx: &RunCtx<'_>, out: &mut BandOutcome) -> Result<()> {
        let cleaned: String;
        let text = if wrap.is_none() {
            cleaned = text.chars().map(|c| if matches!(c, '\n' | '\r' | '\t' | '\u{b}' | '\u{c}') { ' ' } else { c }).collect();
            cleaned.as_str()
        } else {
            text
        };
        if text.is_empty() {
            return Ok(());
        }
        let Some(block) = self.layout_text(text, wrap, ctx, out)? else { return Ok(()) };
        let align = self.st.cur.align;
        let shift = baseline_shift(self.st.cur.baseline, &block);
        let ctm = self.st.cur.ctm;
        for line in &block.lines {
            let Some(lp) = &line.path else { continue };
            let rtl = line.rtl;
            let (mut sx, mut adv) = (1.0f32, line.advance);
            if wrap.is_none() && max_width > 0.0 && adv > max_width {
                sx = max_width / adv;
                adv = max_width;
            }
            let frac = align_fraction(align, rtl);
            // `frac` is 0 for a line that starts at the anchor, 1 for one that ends
            // there (or at the far edge of the box) and 0.5 for a centred one.
            let x_line = match wrap {
                None => x - adv * frac,
                Some((w, _)) => x + (w - adv) * frac,
            };
            let y_line = y + shift + line.baseline;
            let place = Affine::new([sx as f64, 0.0, 0.0, 1.0, x_line as f64, y_line as f64]);
            if mode & MODE_FILL != 0 {
                if let Some(dev) = self.map_dev(lp, ctm * place) {
                    self.paint(&dev, Which::Fill, tiny_skia::FillRule::Winding, ctx);
                }
            }
            if mode & MODE_STROKE != 0 {
                if let Some(up) = (**lp).clone().transform(Transform::from_row(sx, 0.0, 0.0, 1.0, x_line, y_line)) {
                    self.stroke_user_path(&up, ctx);
                }
            }
        }
        Ok(())
    }

    fn measure(&self, text: &str, ctx: &RunCtx<'_>, out: &mut BandOutcome) -> Result<TextMetrics> {
        let cleaned: String = text.chars().map(|c| if matches!(c, '\n' | '\r' | '\t' | '\u{b}' | '\u{c}') { ' ' } else { c }).collect();
        let Some(block) = self.layout_text(&cleaned, None, ctx, out)? else { return Ok(TextMetrics::default()) };
        let g = &self.st.cur;
        Ok(metrics_for(&block, g.align, baseline_shift(g.baseline, &block)))
    }

    // ------------------------------------------------------------------
    // Images
    // ------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn draw_image_region(&mut self, id: u32, sx: f32, sy: f32, sw: f32, sh: f32, dx: f32, dy: f32, dw: f32, dh: f32, ctx: &RunCtx<'_>, out: &mut BandOutcome) {
        let Some(pm) = ctx.images.get(&id) else {
            out.note(format!("image {id} was not drawn: there is no such image"));
            return;
        };
        let (iw, ih) = (pm.width() as f64, pm.height() as f64);
        // Normalise negative sizes (HTML flips the rectangle's corners, it does not mirror).
        let norm = |p: f64, s: f64| if s < 0.0 { (p + s, -s) } else { (p, s) };
        let (mut sx, mut sw) = norm(sx as f64, sw as f64);
        let (mut sy, mut sh) = norm(sy as f64, sh as f64);
        let (mut dx, mut dw) = norm(dx as f64, dw as f64);
        let (mut dy, mut dh) = norm(dy as f64, dh as f64);
        if sw <= 0.0 || sh <= 0.0 || dw <= 0.0 || dh <= 0.0 {
            return;
        }
        // Clip the source to the image and shrink the destination to match.
        let (cx0, cx1) = (sx.max(0.0), (sx + sw).min(iw));
        let (cy0, cy1) = (sy.max(0.0), (sy + sh).min(ih));
        if cx1 <= cx0 || cy1 <= cy0 {
            return;
        }
        let (kx, ky) = (dw / sw, dh / sh);
        dx += (cx0 - sx) * kx;
        dy += (cy0 - sy) * ky;
        dw = (cx1 - cx0) * kx;
        dh = (cy1 - cy0) * ky;
        sx = cx0;
        sy = cy0;
        sw = cx1 - cx0;
        sh = cy1 - cy0;
        // Cut out the whole-pixel box around the source rectangle.
        let (px0, py0) = (sx.floor() as i32, sy.floor() as i32);
        let (px1, py1) = ((sx + sw).ceil() as i32, (sy + sh).ceil() as i32);
        let (pw, ph) = ((px1 - px0).max(1) as u32, (py1 - py0).max(1) as u32);
        let sub: Arc<Pixmap> = if px0 == 0 && py0 == 0 && pw == pm.width() && ph == pm.height() {
            pm.clone()
        } else {
            match IntRect::from_xywh(px0, py0, pw, ph).and_then(|r| pm.clone_rect(r)) {
                Some(p) => Arc::new(p),
                None => return,
            }
        };
        let ctm = self.st.cur.ctm;
        // pixel (u, v) of the cut-out -> user space
        let map = Affine::translate((dx, dy))
            * Affine::scale_non_uniform(dw / sw, dh / sh)
            * Affine::translate((px0 as f64 - sx, py0 as f64 - sy));
        let img_ts = to_ts(ctm * map).post_translate(0.0, -(self.y0 as f32));
        let dest = rect_bez(ctm, dx, dy, dw, dh);
        if let Some(p) = self.dev_path(&dest) {
            self.paint_src(&p, &PaintSrc::Image(sub), tiny_skia::FillRule::Winding, img_ts, ctx);
        }
    }

    // ------------------------------------------------------------------
    // Command dispatch
    // ------------------------------------------------------------------

    pub(crate) fn exec(&mut self, cmd: Command<'_>, ctx: &RunCtx<'_>, out: &mut BandOutcome) -> Result<()> {
        use Command as C;
        let lim = ctx.limits;
        match cmd {
            // ---- state ----
            C::Save {} => {
                lim.check_save_depth(self.st.stack.len() as u64 + 1)?;
                let cur = self.st.cur.clone();
                self.st.stack.push(cur);
            }
            C::Restore {} => {
                if let Some(s) = self.st.stack.pop() {
                    self.st.cur = s;
                }
            }
            C::Reset {} => self.st.reset(),
            C::Clear { color } => self.pm.fill(color.to_skia()),
            C::Present {} => out.presented = true,
            C::SetSmoothing { quality } => {
                if let Some(s) = Smoothing::from_u8(quality) {
                    self.st.cur.smoothing = s;
                }
            }

            // ---- path ----
            C::BeginPath {} => self.st.path.clear(),
            C::MoveTo { x, y } => {
                finite!(x, y);
                lim.check_path_points(self.st.path.points() + 1)?;
                let p = self.st.cur.ctm * Point::new(x as f64, y as f64);
                self.st.path.move_to(p);
            }
            C::LineTo { x, y } => {
                finite!(x, y);
                lim.check_path_points(self.st.path.points() + 1)?;
                let p = self.st.cur.ctm * Point::new(x as f64, y as f64);
                self.st.path.line_to(p);
            }
            C::QuadTo { cx, cy, x, y } => {
                finite!(cx, cy, x, y);
                lim.check_path_points(self.st.path.points() + 2)?;
                let m = self.st.cur.ctm;
                self.st.path.quad_to(m * Point::new(cx as f64, cy as f64), m * Point::new(x as f64, y as f64));
            }
            C::CubicTo { c1x, c1y, c2x, c2y, x, y } => {
                finite!(c1x, c1y, c2x, c2y, x, y);
                lim.check_path_points(self.st.path.points() + 3)?;
                let m = self.st.cur.ctm;
                self.st.path.cubic_to(
                    m * Point::new(c1x as f64, c1y as f64),
                    m * Point::new(c2x as f64, c2y as f64),
                    m * Point::new(x as f64, y as f64),
                );
            }
            C::ArcTo { x1, y1, x2, y2, radius } => {
                finite!(x1, y1, x2, y2, radius);
                lim.check_path_points(self.st.path.points() + 8)?;
                let m = self.st.cur.ctm;
                self.st.path.arc_to(m, x1 as f64, y1 as f64, x2 as f64, y2 as f64, radius as f64);
            }
            C::Arc { x, y, radius, start, end, ccw } => {
                finite!(x, y, radius, start, end);
                lim.check_path_points(self.st.path.points() + 64)?;
                let m = self.st.cur.ctm;
                self.st.path.arc(m, x as f64, y as f64, radius as f64, start as f64, end as f64, ccw != 0);
            }
            C::Ellipse { x, y, rx, ry, rotation, start, end, ccw } => {
                finite!(x, y, rx, ry, rotation, start, end);
                lim.check_path_points(self.st.path.points() + 64)?;
                let m = self.st.cur.ctm;
                self.st.path.ellipse(m, x as f64, y as f64, rx as f64, ry as f64, rotation as f64, start as f64, end as f64, ccw != 0);
            }
            C::Rect { x, y, w, h } => {
                finite!(x, y, w, h);
                lim.check_path_points(self.st.path.points() + 5)?;
                let m = self.st.cur.ctm;
                self.st.path.rect(m, x as f64, y as f64, w as f64, h as f64);
            }
            C::RoundRect { x, y, w, h, tl, tr, br, bl } => {
                finite!(x, y, w, h, tl, tr, br, bl);
                if tl < 0.0 || tr < 0.0 || br < 0.0 || bl < 0.0 {
                    return Ok(());
                }
                lim.check_path_points(self.st.path.points() + 16)?;
                let m = self.st.cur.ctm;
                self.st.path.round_rect(m, x as f64, y as f64, w as f64, h as f64, [tl as f64, tr as f64, br as f64, bl as f64]);
            }
            C::ClosePath {} => self.st.path.close(),
            C::Fill { rule } => {
                if !self.st.path.is_empty() {
                    let bez = self.st.path.bez.clone();
                    let r = sk_rule(FillRule::from_u8(rule).unwrap_or(FillRule::NonZero));
                    self.fill_device_bez(&bez, r, ctx);
                }
            }
            C::Stroke {} => {
                if !self.st.path.is_empty() {
                    let bez = self.st.path.bez.clone();
                    self.stroke_device_bez(&bez, ctx);
                }
            }
            C::Clip { rule } => {
                let r = sk_rule(FillRule::from_u8(rule).unwrap_or(FillRule::NonZero));
                let (w, h) = (self.w, self.h);
                let new_mask = match self.dev_path(&self.st.path.bez) {
                    // An empty or degenerate path clips everything away.
                    None => Mask::new(w, h),
                    Some(p) => match self.st.cur.clip.as_ref() {
                        None => Mask::new(w, h).map(|mut m| {
                            m.fill_path(&p, r, true, Transform::identity());
                            m
                        }),
                        Some(prev) => {
                            let mut m = (**prev).clone();
                            m.intersect_path(&p, r, true, Transform::identity());
                            Some(m)
                        }
                    },
                };
                self.st.cur.clip = new_mask.map(Arc::new);
            }

            // ---- shapes ----
            C::FillRect { x, y, w, h } => {
                finite!(x, y, w, h);
                let b = rect_bez(self.st.cur.ctm, x as f64, y as f64, w as f64, h as f64);
                self.fill_device_bez(&b, tiny_skia::FillRule::Winding, ctx);
            }
            C::StrokeRect { x, y, w, h } => {
                finite!(x, y, w, h);
                let b = rect_bez(Affine::IDENTITY, x as f64, y as f64, w as f64, h as f64);
                self.stroke_user_bez(&b, ctx);
            }
            C::ClearRect { x, y, w, h } => {
                finite!(x, y, w, h);
                let b = rect_bez(self.st.cur.ctm, x as f64, y as f64, w as f64, h as f64);
                if let Some(p) = self.dev_path(&b) {
                    let ts = self.shader_ts();
                    let mask = self.st.cur.clip.as_deref();
                    raster(&mut self.pm, &p, &PaintSrc::Solid(Rgba::BLACK), tiny_skia::FillRule::Winding, ts, 1.0, tiny_skia::BlendMode::Clear, tiny_skia::FilterQuality::Nearest, mask);
                }
            }
            C::DrawLine { x1, y1, x2, y2 } => {
                finite!(x1, y1, x2, y2);
                let mut b = BezPath::new();
                b.move_to(Point::new(x1 as f64, y1 as f64));
                b.line_to(Point::new(x2 as f64, y2 as f64));
                self.stroke_user_bez(&b, ctx);
            }
            C::DrawCircle { x, y, radius, mode } => {
                finite!(x, y, radius);
                if radius < 0.0 {
                    return Ok(());
                }
                let user = circle_path(Point::new(x as f64, y as f64), radius as f64, self.st.cur.ctm);
                if mode & MODE_FILL != 0 {
                    let mut dev = user.clone();
                    dev.apply_affine(self.st.cur.ctm);
                    self.fill_device_bez(&dev, tiny_skia::FillRule::Winding, ctx);
                }
                if mode & MODE_STROKE != 0 {
                    self.stroke_user_bez(&user, ctx);
                }
            }
            C::DrawPolygon { points, mode } => {
                lim.check_path_points(points.len() as u64)?;
                if points.len() < 2 {
                    return Ok(());
                }
                let mut user = BezPath::new();
                for (i, (px, py)) in points.iter().enumerate() {
                    if !(px.is_finite() && py.is_finite()) {
                        return Ok(());
                    }
                    let p = Point::new(px as f64, py as f64);
                    if i == 0 {
                        user.move_to(p);
                    } else {
                        user.line_to(p);
                    }
                }
                if mode & MODE_CLOSE != 0 || mode & MODE_FILL != 0 {
                    user.close_path();
                }
                if mode & MODE_FILL != 0 {
                    let mut dev = user.clone();
                    dev.apply_affine(self.st.cur.ctm);
                    self.fill_device_bez(&dev, tiny_skia::FillRule::Winding, ctx);
                }
                if mode & MODE_STROKE != 0 {
                    let mut stroke_path = BezPath::new();
                    for (i, (px, py)) in points.iter().enumerate() {
                        let p = Point::new(px as f64, py as f64);
                        if i == 0 {
                            stroke_path.move_to(p);
                        } else {
                            stroke_path.line_to(p);
                        }
                    }
                    if mode & MODE_CLOSE != 0 {
                        stroke_path.close_path();
                    }
                    self.stroke_user_bez(&stroke_path, ctx);
                }
            }

            // ---- style ----
            C::SetFillColor { color } => self.st.cur.fill = PaintSrc::Solid(color),
            C::SetStrokeColor { color } => self.st.cur.stroke = PaintSrc::Solid(color),
            C::SetLineWidth { width } => {
                if width.is_finite() && width > 0.0 {
                    self.st.cur.line_width = width;
                }
            }
            C::SetLineCap { cap } => {
                if let Some(c) = LineCap::from_u8(cap) {
                    self.st.cur.cap = c;
                }
            }
            C::SetLineJoin { join } => {
                if let Some(j) = LineJoin::from_u8(join) {
                    self.st.cur.join = j;
                }
            }
            C::SetMiterLimit { limit } => {
                if limit.is_finite() && limit > 0.0 {
                    self.st.cur.miter_limit = limit;
                }
            }
            C::SetDash { intervals, offset } => {
                let v: Vec<f32> = intervals.iter().collect();
                if v.iter().any(|x| !x.is_finite() || *x < 0.0) || !offset.is_finite() {
                    return Ok(());
                }
                if v.len() > 4096 {
                    return Ok(());
                }
                let mut v = v;
                if v.len() % 2 == 1 {
                    let again = v.clone();
                    v.extend(again);
                }
                if v.is_empty() || v.iter().sum::<f32>() <= 0.0 {
                    self.st.cur.dash = Arc::new(Vec::new());
                } else {
                    self.st.cur.dash = Arc::new(v);
                }
                self.st.cur.dash_offset = offset;
            }
            C::SetAlpha { alpha } => {
                if alpha.is_finite() && (0.0..=1.0).contains(&alpha) {
                    self.st.cur.alpha = alpha;
                }
            }
            C::SetBlendMode { mode } => {
                if let Some(m) = BlendMode::from_u8(mode) {
                    self.st.cur.blend = m;
                }
            }
            C::SetLinearGradient { target, x0, y0, x1, y1, stops } => {
                finite!(x0, y0, x1, y1);
                if let Some(stops) = sanitize_stops(&stops) {
                    self.set_style(target, PaintSrc::Linear { p0: (x0, y0), p1: (x1, y1), stops });
                }
            }
            C::SetRadialGradient { target, x0, y0, r0, x1, y1, r1, stops } => {
                finite!(x0, y0, r0, x1, y1, r1);
                if r0 < 0.0 || r1 < 0.0 {
                    return Ok(());
                }
                if let Some(stops) = sanitize_stops(&stops) {
                    self.set_style(target, PaintSrc::Radial { c0: (x0, y0), r0, c1: (x1, y1), r1, stops });
                }
            }
            C::SetConicGradient { target, x, y, angle, stops } => {
                finite!(x, y, angle);
                if let Some(stops) = sanitize_stops(&stops) {
                    self.set_style(target, PaintSrc::Conic { c: (x, y), angle, stops });
                }
            }
            C::SetPattern { target, image, repeat } => {
                let Some(repeat) = PatternRepeat::from_u8(repeat) else { return Ok(()) };
                match ctx.images.get(&image) {
                    Some(pm) => self.set_style(target, PaintSrc::Pattern { pixmap: pm.clone(), repeat }),
                    None => out.note(format!("pattern not set: there is no image {image}")),
                }
            }
            C::SetShadow { color, blur, dx, dy } => {
                if blur.is_finite() && blur >= 0.0 && dx.is_finite() && dy.is_finite() {
                    self.st.cur.shadow = Shadow { color, blur, dx, dy };
                }
            }

            // ---- transforms ----
            C::Translate { x, y } => {
                finite!(x, y);
                self.st.cur.ctm = self.st.cur.ctm * Affine::translate((x as f64, y as f64));
            }
            C::Rotate { angle } => {
                finite!(angle);
                self.st.cur.ctm = self.st.cur.ctm * Affine::rotate(angle as f64);
            }
            C::Scale { x, y } => {
                finite!(x, y);
                self.st.cur.ctm = self.st.cur.ctm * Affine::scale_non_uniform(x as f64, y as f64);
            }
            C::Transform { a, b, c, d, e, f } => {
                finite!(a, b, c, d, e, f);
                self.st.cur.ctm = self.st.cur.ctm * Affine::new([a as f64, b as f64, c as f64, d as f64, e as f64, f as f64]);
            }
            C::SetTransform { a, b, c, d, e, f } => {
                finite!(a, b, c, d, e, f);
                self.st.cur.ctm = self.st.base * Affine::new([a as f64, b as f64, c as f64, d as f64, e as f64, f as f64]);
            }
            C::ResetTransform {} => self.st.cur.ctm = self.st.base,

            // ---- text ----
            C::SetFont { css } => {
                if let Some(f) = FontSpec::parse(css) {
                    self.st.cur.font = Arc::new(f);
                }
            }
            C::SetTextAlign { align } => {
                if let Some(a) = TextAlign::from_u8(align) {
                    self.st.cur.align = a;
                }
            }
            C::SetTextBaseline { baseline } => {
                if let Some(b) = TextBaseline::from_u8(baseline) {
                    self.st.cur.baseline = b;
                }
            }
            C::SetTextDirection { direction } => {
                if let Some(d) = TextDirection::from_u8(direction) {
                    self.st.cur.direction = d;
                }
            }
            C::DrawText { text, x, y, max_width, mode } => {
                finite!(x, y, max_width);
                self.draw_text(text, x, y, max_width, None, mode, ctx, out)?;
            }
            C::DrawTextBox { text, x, y, width, line_height, mode } => {
                finite!(x, y, width, line_height);
                if width <= 0.0 {
                    return Ok(());
                }
                self.draw_text(text, x, y, 0.0, Some((width, line_height)), mode, ctx, out)?;
            }

            // ---- images ----
            C::DrawImage { image, x, y } => {
                finite!(x, y);
                match ctx.images.get(&image) {
                    Some(pm) => {
                        let (w, h) = (pm.width() as f32, pm.height() as f32);
                        self.draw_image_region(image, 0.0, 0.0, w, h, x, y, w, h, ctx, out);
                    }
                    None => out.note(format!("image {image} was not drawn: there is no such image")),
                }
            }
            C::DrawImageScaled { image, x, y, w, h } => {
                finite!(x, y, w, h);
                match ctx.images.get(&image) {
                    Some(pm) => {
                        let (iw, ih) = (pm.width() as f32, pm.height() as f32);
                        self.draw_image_region(image, 0.0, 0.0, iw, ih, x, y, w, h, ctx, out);
                    }
                    None => out.note(format!("image {image} was not drawn: there is no such image")),
                }
            }
            C::DrawImageRegion { image, sx, sy, sw, sh, dx, dy, dw, dh } => {
                finite!(sx, sy, sw, sh, dx, dy, dw, dh);
                self.draw_image_region(image, sx, sy, sw, sh, dx, dy, dw, dh, ctx, out);
            }
            C::DrawSprite { image, frame, frame_w, frame_h, dx, dy, dw, dh } => {
                finite!(frame_w, frame_h, dx, dy, dw, dh);
                let Some(pm) = ctx.images.get(&image) else {
                    out.note(format!("sprite not drawn: there is no image {image}"));
                    return Ok(());
                };
                if frame_w < 1.0 || frame_h < 1.0 {
                    return Ok(());
                }
                let cols = ((pm.width() as f32 / frame_w).floor() as u32).max(1);
                let (col, row) = (frame % cols, frame / cols);
                let (sx, sy) = (col as f32 * frame_w, row as f32 * frame_h);
                self.draw_image_region(image, sx, sy, frame_w, frame_h, dx, dy, dw, dh, ctx, out);
            }

            // ---- read-back ----
            C::ReadPixel { slot, x, y } => {
                let color = if x.is_finite() && y.is_finite() {
                    let (px, py) = ((x * ctx.ratio).floor() as i64, (y * ctx.ratio).floor() as i64);
                    let row = py.saturating_sub(self.y0 as i64);
                    if px >= 0 && row >= 0 && (px as u64) < self.w as u64 && (row as u64) < self.h as u64 {
                        self.pm.pixel(px as u32, row as u32).map(|p| {
                            let c = p.demultiply();
                            Rgba::new(c.red(), c.green(), c.blue(), c.alpha())
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };
                out.reads.push(PartRead::Pixel { slot, color });
            }
            C::Capture { slot, x, y, w, h } => {
                let (cw, ch) = ctx.canvas;
                let ok = x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite();
                let (x0, y0, x1, y1) = if ok {
                    let r = ctx.ratio;
                    let nx = if w < 0.0 { x + w } else { x };
                    let ny = if h < 0.0 { y + h } else { y };
                    let (aw, ah) = (w.abs(), h.abs());
                    (
                        ((nx * r).floor() as i64).clamp(0, cw as i64),
                        ((ny * r).floor() as i64).clamp(0, ch as i64),
                        (((nx + aw) * r).ceil() as i64).clamp(0, cw as i64),
                        (((ny + ah) * r).ceil() as i64).clamp(0, ch as i64),
                    )
                } else {
                    (0, 0, 0, 0)
                };
                let rect = (x0 as i32, y0 as i32, (x1 - x0).max(0) as u32, (y1 - y0).max(0) as u32);
                let (ra, rb) = (y0.max(self.y0 as i64), y1.min((self.y0 + self.h) as i64));
                let rows = if rect.2 > 0 && rect.3 > 0 && rb > ra {
                    let mut data = Vec::with_capacity(rect.2 as usize * (rb - ra) as usize * 4);
                    for ry in ra..rb {
                        let local = (ry - self.y0 as i64) as usize;
                        let base = local * self.w as usize;
                        for px in &self.pm.pixels()[base + x0 as usize..base + x1 as usize] {
                            let c = px.demultiply();
                            data.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
                        }
                    }
                    Some((ra as u32, data))
                } else {
                    None
                };
                out.reads.push(PartRead::Capture { slot, rect, rows });
            }
            C::HitPath { slot, x, y, rule } => {
                if !self.primary {
                    out.reads.push(PartRead::Skip);
                } else {
                    let inside = x.is_finite() && y.is_finite() && {
                        let wnd = self.st.path.bez.winding(Point::new((x * ctx.ratio) as f64, (y * ctx.ratio) as f64));
                        match FillRule::from_u8(rule).unwrap_or(FillRule::NonZero) {
                            FillRule::NonZero => wnd != 0,
                            FillRule::EvenOdd => wnd % 2 != 0,
                        }
                    };
                    out.reads.push(PartRead::Hit { slot, inside });
                }
            }
            C::HitStroke { slot, x, y } => {
                if !self.primary {
                    out.reads.push(PartRead::Skip);
                } else {
                    let inside = x.is_finite() && y.is_finite() && self.hit_stroke(Point::new((x * ctx.ratio) as f64, (y * ctx.ratio) as f64));
                    out.reads.push(PartRead::Hit { slot, inside });
                }
            }
            C::MeasureText { slot, text } => {
                if !self.primary {
                    out.reads.push(PartRead::Skip);
                } else {
                    let metrics = self.measure(text, ctx, out)?;
                    out.reads.push(PartRead::Text { slot, metrics });
                }
            }
        }
        Ok(())
    }

    fn set_style(&mut self, target: u8, src: PaintSrc) {
        match StyleTarget::from_u8(target) {
            Some(StyleTarget::Fill) => self.st.cur.fill = src,
            Some(StyleTarget::Stroke) => self.st.cur.stroke = src,
            None => {}
        }
    }

    /// Whether a device point is inside the stroke outline of the current path.
    fn hit_stroke(&self, pt: Point) -> bool {
        let g = &self.st.cur;
        if !(g.line_width.is_finite() && g.line_width > 0.0) || self.st.path.is_empty() {
            return false;
        }
        let inv = g.ctm.inverse();
        if !inv.is_finite() {
            return false;
        }
        let mut user = self.st.path.bez.clone();
        user.apply_affine(inv);
        let Some(up) = curves_to_tiny(&user) else { return false };
        let ts = to_ts(g.ctm);
        let stroke = make_stroke(g.line_width, g.cap, g.join, g.miter_limit, &g.dash, g.dash_offset);
        let res_scale = stroke_resolution(&ts);
        let Some(outline) = up.stroke(&stroke, res_scale) else { return false };
        // Flatten in device space (the same polygon the fill would use) and take its winding number.
        let Some(dev) = map_to_tiny(&outline, g.ctm, 0.0, self.grid) else { return false };
        let mut bez = BezPath::new();
        for seg in dev.segments() {
            use tiny_skia::PathSegment as S;
            match seg {
                S::MoveTo(p) => bez.move_to(Point::new(p.x as f64, p.y as f64)),
                S::LineTo(p) => bez.line_to(Point::new(p.x as f64, p.y as f64)),
                S::QuadTo(..) | S::CubicTo(..) => {}
                S::Close => bez.close_path(),
            }
        }
        bez.winding(pt) != 0
    }
}

/// Validate gradient stops: finite offsets, at most 1024 stops.
fn sanitize_stops(stops: &crate::display_list::Stops<'_>) -> Option<Arc<Vec<(f32, Rgba)>>> {
    if stops.len() > 1024 {
        return None;
    }
    let v: Vec<(f32, Rgba)> = stops.iter().filter(|(o, _)| o.is_finite()).collect();
    Some(Arc::new(v))
}

/// Merge the per-band parts of a read-back into the final answer.
pub(crate) fn merge_reads(bands: Vec<BandOutcome>, n: usize) -> Vec<super::ReadBack> {
    use super::ReadBack;
    let mut result = Vec::new();
    for i in 0..n {
        // Exactly one band answers each non-pixel read-back; pixels come from the band that has the row.
        let mut parts: Vec<&PartRead> = bands.iter().filter_map(|b| b.reads.get(i)).collect();
        if parts.is_empty() {
            continue;
        }
        match parts[0] {
            PartRead::Pixel { slot, .. } => {
                let color = parts.iter().find_map(|p| if let PartRead::Pixel { color: Some(c), .. } = p { Some(*c) } else { None });
                result.push(ReadBack::Pixel { slot: *slot, color });
            }
            PartRead::Capture { slot, rect, .. } => {
                let (x0, _y0, w, h) = *rect;
                let _ = x0;
                if w == 0 || h == 0 {
                    result.push(ReadBack::Image { slot: *slot, image: None });
                    continue;
                }
                let mut data = vec![0u8; w as usize * h as usize * 4];
                let top = rect.1 as u32;
                for p in parts.drain(..) {
                    if let PartRead::Capture { rows: Some((first, bytes)), .. } = p {
                        let off = (*first - top) as usize * w as usize * 4;
                        data[off..off + bytes.len()].copy_from_slice(bytes);
                    }
                }
                result.push(ReadBack::Image { slot: *slot, image: crate::pixels::PixelBuffer::from_rgba(w, h, data).ok() });
            }
            _ => {
                if let Some(p) = parts.iter().find(|p| !matches!(***p, PartRead::Skip)) {
                    match p {
                        PartRead::Hit { slot, inside } => result.push(ReadBack::Hit { slot: *slot, inside: *inside }),
                        PartRead::Text { slot, metrics } => result.push(ReadBack::Text { slot: *slot, metrics: metrics.clone() }),
                        _ => {}
                    }
                }
            }
        }
    }
    result
}
