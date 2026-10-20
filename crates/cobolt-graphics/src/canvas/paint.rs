// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Putting paths on pixels: paints, compositing, shadows, strokes.
//!
//! Every function here works on a **device-space path already offset into the
//! coordinates of the pixmap it paints** (a band, or a temporary layer). The
//! offset is always a whole number of pixels, applied to coordinates that are
//! themselves identical in every band, so a band renders exactly the rows of
//! the untiled picture that it covers.

use kurbo::{Affine, BezPath, PathEl};
use tiny_skia::{
    BlendMode as SkBlend, Color, FillRule as SkRule, FilterQuality, GradientStop, LinearGradient, Mask, Paint, Path,
    PathBuilder, Pattern, Pixmap, PixmapPaint, Point, RadialGradient, Shader, SpreadMode, Stroke, StrokeDash,
    SweepGradient, Transform,
};

use super::blur::blur_alpha;
use super::state::{PaintSrc, Shadow};
use crate::color::Rgba;
use crate::display_list::consts::*;

/// Largest coordinate magnitude handed to the rasteriser. Anything beyond is far
/// outside any canvas this crate allows; clamping keeps hostile numbers from
/// reaching fixed-point code.
pub(crate) const COORD_LIMIT: f64 = 1.0e6;

pub(crate) fn sk_blend(m: BlendMode) -> SkBlend {
    match m {
        BlendMode::SourceOver => SkBlend::SourceOver,
        BlendMode::SourceIn => SkBlend::SourceIn,
        BlendMode::SourceOut => SkBlend::SourceOut,
        BlendMode::SourceAtop => SkBlend::SourceAtop,
        BlendMode::DestinationOver => SkBlend::DestinationOver,
        BlendMode::DestinationIn => SkBlend::DestinationIn,
        BlendMode::DestinationOut => SkBlend::DestinationOut,
        BlendMode::DestinationAtop => SkBlend::DestinationAtop,
        BlendMode::Lighter => SkBlend::Plus,
        BlendMode::Copy => SkBlend::Source,
        BlendMode::Xor => SkBlend::Xor,
        BlendMode::Multiply => SkBlend::Multiply,
        BlendMode::Screen => SkBlend::Screen,
        BlendMode::Overlay => SkBlend::Overlay,
        BlendMode::Darken => SkBlend::Darken,
        BlendMode::Lighten => SkBlend::Lighten,
        BlendMode::ColorDodge => SkBlend::ColorDodge,
        BlendMode::ColorBurn => SkBlend::ColorBurn,
        BlendMode::HardLight => SkBlend::HardLight,
        BlendMode::SoftLight => SkBlend::SoftLight,
        BlendMode::Difference => SkBlend::Difference,
        BlendMode::Exclusion => SkBlend::Exclusion,
        BlendMode::Hue => SkBlend::Hue,
        BlendMode::Saturation => SkBlend::Saturation,
        BlendMode::Color => SkBlend::Color,
        BlendMode::Luminosity => SkBlend::Luminosity,
    }
}

/// Operations whose result differs from the destination even where the source
/// is transparent: HTML applies them to the whole clip region, so the shape is
/// first drawn into a transparent layer and the layer is composited.
pub(crate) fn is_unbounded(m: BlendMode) -> bool {
    matches!(
        m,
        BlendMode::SourceIn | BlendMode::SourceOut | BlendMode::DestinationIn | BlendMode::DestinationAtop | BlendMode::Copy
    )
}

pub(crate) fn sk_rule(r: FillRule) -> SkRule {
    match r {
        FillRule::NonZero => SkRule::Winding,
        FillRule::EvenOdd => SkRule::EvenOdd,
    }
}

/// A kurbo path to a rasteriser path **with its curves intact**: for user-space
/// paths that the stroker will walk. (Device-space geometry goes through
/// [`flatten::to_tiny`](super::flatten::to_tiny) instead, which flattens it.)
/// `None` when there is nothing drawable.
pub(crate) fn curves_to_tiny(bez: &BezPath) -> Option<Path> {
    let y0 = 0.0;
    let mut pb = PathBuilder::new();
    let cl = |v: f64| v.clamp(-COORD_LIMIT, COORD_LIMIT);
    let fx = |p: kurbo::Point| (cl(p.x) as f32, cl(p.y - y0) as f32);
    let mut open = false;
    for el in bez.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                let (x, y) = fx(p);
                pb.move_to(x, y);
                open = true;
            }
            PathEl::LineTo(p) => {
                let (x, y) = fx(p);
                pb.line_to(x, y);
            }
            PathEl::QuadTo(c, p) => {
                let (cx, cy) = fx(c);
                let (x, y) = fx(p);
                pb.quad_to(cx, cy, x, y);
            }
            PathEl::CurveTo(a, b, p) => {
                let (ax, ay) = fx(a);
                let (bx, by) = fx(b);
                let (x, y) = fx(p);
                pb.cubic_to(ax, ay, bx, by, x, y);
            }
            PathEl::ClosePath => {
                if open {
                    pb.close();
                }
            }
        }
    }
    pb.finish()
}

/// kurbo affine to the rasteriser's transform.
pub(crate) fn to_ts(a: Affine) -> Transform {
    let c = a.as_coeffs();
    Transform::from_row(c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32, c[4] as f32, c[5] as f32)
}

pub(crate) fn quality_for(s: Smoothing) -> FilterQuality {
    match s {
        Smoothing::Off => FilterQuality::Nearest,
        Smoothing::Low => FilterQuality::Bilinear,
        Smoothing::High => FilterQuality::Bicubic,
    }
}

fn skia_color(c: Rgba, alpha: f32) -> Color {
    let a = (c.a as f32 * alpha).round().clamp(0.0, 255.0) as u8;
    Color::from_rgba8(c.r, c.g, c.b, a)
}

/// Stops sorted by offset (stable: equal offsets keep the order they were added
/// in), clamped to 0..=1, with the paint's alpha folded into every colour.
fn prepare_stops(stops: &[(f32, Rgba)], alpha: f32) -> Vec<(f32, Color)> {
    let mut v: Vec<(f32, Rgba)> = stops.iter().map(|(o, c)| (o.clamp(0.0, 1.0), *c)).collect();
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    v.iter().map(|(o, c)| (*o, skia_color(*c, alpha))).collect()
}

fn gradient_stops(prepared: &[(f32, Color)]) -> Vec<GradientStop> {
    prepared.iter().map(|(o, c)| GradientStop::new(*o, *c)).collect()
}

/// Whether `ts` is a pure translation by whole pixels (an unscaled, unrotated
/// copy): such a draw must reproduce the source pixels exactly.
fn is_integer_translate(ts: &Transform) -> bool {
    ts.is_translate() && (ts.tx - ts.tx.round()).abs() < 1e-4 && (ts.ty - ts.ty.round()).abs() < 1e-4
}

/// Fill `path` (already in `pm`'s coordinates) with `src`.
///
/// `shader_ts` maps the paint's own space (gradient endpoints, pattern pixels)
/// to `pm`'s coordinates.
#[allow(clippy::too_many_arguments)]
pub(crate) fn raster(
    pm: &mut Pixmap,
    path: &Path,
    src: &PaintSrc,
    rule: SkRule,
    shader_ts: Transform,
    alpha: f32,
    blend: SkBlend,
    quality: FilterQuality,
    mask: Option<&Mask>,
) {
    let mut paint = Paint { anti_alias: true, blend_mode: blend, ..Paint::default() };
    let mut strip: Option<Mask> = None;
    match src {
        PaintSrc::Solid(c) => paint.set_color(skia_color(*c, alpha)),
        PaintSrc::Linear { p0, p1, stops } => {
            let st = prepare_stops(stops, alpha);
            let shader = match st.len() {
                0 => return,
                1 => Some(Shader::SolidColor(st[0].1)),
                _ => LinearGradient::new(
                    Point::from_xy(p0.0, p0.1),
                    Point::from_xy(p1.0, p1.1),
                    gradient_stops(&st),
                    SpreadMode::Pad,
                    shader_ts,
                ),
            };
            match shader {
                Some(s) => paint.shader = s,
                None => return,
            }
        }
        PaintSrc::Radial { c0, r0, c1, r1, stops } => {
            let st = prepare_stops(stops, alpha);
            let shader = match st.len() {
                0 => return,
                1 => Some(Shader::SolidColor(st[0].1)),
                _ => RadialGradient::new(
                    Point::from_xy(c0.0, c0.1),
                    *r0,
                    Point::from_xy(c1.0, c1.1),
                    *r1,
                    gradient_stops(&st),
                    SpreadMode::Pad,
                    shader_ts,
                ),
            };
            match shader {
                Some(s) => paint.shader = s,
                None => return,
            }
        }
        PaintSrc::Conic { c, angle, stops } => {
            let st = prepare_stops(stops, alpha);
            let start = angle.to_degrees();
            let shader = match st.len() {
                0 => return,
                1 => Some(Shader::SolidColor(st[0].1)),
                _ => SweepGradient::new(
                    Point::from_xy(c.0, c.1),
                    start,
                    start + 360.0,
                    gradient_stops(&st),
                    SpreadMode::Pad,
                    shader_ts,
                ),
            };
            match shader {
                Some(s) => paint.shader = s,
                None => return,
            }
        }
        PaintSrc::Image(pixmap) => {
            let q = if is_integer_translate(&shader_ts) { FilterQuality::Nearest } else { quality };
            paint.shader = Pattern::new(Pixmap::as_ref(pixmap), SpreadMode::Pad, q, alpha, shader_ts);
        }
        PaintSrc::Pattern { pixmap, repeat } => {
            let (spread, restrict): (SpreadMode, Option<PatternRepeat>) = match repeat {
                PatternRepeat::Repeat => (SpreadMode::Repeat, None),
                // One-axis repeat: tile in both directions, then keep only the strip.
                PatternRepeat::RepeatX => (SpreadMode::Repeat, Some(PatternRepeat::RepeatX)),
                PatternRepeat::RepeatY => (SpreadMode::Repeat, Some(PatternRepeat::RepeatY)),
                PatternRepeat::NoRepeat => (SpreadMode::Pad, Some(PatternRepeat::NoRepeat)),
            };
            if let Some(kind) = restrict {
                let (w, h) = (pixmap.width() as f32, pixmap.height() as f32);
                let (x0, x1, y0, y1) = match kind {
                    PatternRepeat::RepeatX => (-1.0e6, 1.0e6, 0.0, h),
                    PatternRepeat::RepeatY => (0.0, w, -1.0e6, 1.0e6),
                    _ => (0.0, w, 0.0, h),
                };
                let mut pb = PathBuilder::new();
                pb.move_to(x0, y0);
                pb.line_to(x1, y0);
                pb.line_to(x1, y1);
                pb.line_to(x0, y1);
                pb.close();
                if let Some(rect) = pb.finish().and_then(|p| p.transform(shader_ts)) {
                    if let Some(mut m) = Mask::new(pm.width(), pm.height()) {
                        m.fill_path(&rect, SkRule::Winding, true, Transform::identity());
                        if let Some(existing) = mask {
                            for (a, b) in m.data_mut().iter_mut().zip(existing.data().iter()) {
                                *a = ((*a as u32 * *b as u32 + 127) / 255) as u8;
                            }
                        }
                        strip = Some(m);
                    }
                }
            }
            let q = if is_integer_translate(&shader_ts) { FilterQuality::Nearest } else { quality };
            paint.shader = Pattern::new(Pixmap::as_ref(pixmap), spread, q, alpha, shader_ts);
        }
    }
    let m = strip.as_ref().or(mask);
    pm.fill_path(path, &paint, rule, Transform::identity(), m);
}

/// Build the rasteriser stroke for the current line style.
pub(crate) fn make_stroke(width: f32, cap: LineCap, join: LineJoin, miter: f32, dash: &[f32], offset: f32) -> Stroke {
    let mut s = Stroke {
        width,
        miter_limit: miter.max(1.0),
        line_cap: match cap {
            LineCap::Butt => tiny_skia::LineCap::Butt,
            LineCap::Round => tiny_skia::LineCap::Round,
            LineCap::Square => tiny_skia::LineCap::Square,
        },
        line_join: match join {
            LineJoin::Miter => tiny_skia::LineJoin::Miter,
            LineJoin::Round => tiny_skia::LineJoin::Round,
            LineJoin::Bevel => tiny_skia::LineJoin::Bevel,
        },
        dash: None,
    };
    if !dash.is_empty() {
        s.dash = StrokeDash::new(dash.to_vec(), offset);
    }
    s
}

/// A mask-sized scratch layer: the shape drawn alone, so it can be blurred or
/// composited as a whole.
pub(crate) fn new_layer(w: u32, h: u32) -> Option<Pixmap> {
    Pixmap::new(w, h)
}

/// Composite a finished layer onto `pm` with an operation.
pub(crate) fn composite_layer(pm: &mut Pixmap, layer: &Pixmap, x: i32, y: i32, blend: SkBlend, mask: Option<&Mask>) {
    let paint = PixmapPaint { opacity: 1.0, blend_mode: blend, quality: FilterQuality::Nearest };
    pm.draw_pixmap(x, y, layer.as_ref(), &paint, Transform::identity(), mask);
}

/// Everything a shadow needs from the context of one draw.
pub(crate) struct ShadowJob<'a> {
    pub shadow: Shadow,
    pub ratio: f32,
    pub path: &'a Path,
    pub src: &'a PaintSrc,
    pub rule: SkRule,
    pub shader_ts: Transform,
    pub alpha: f32,
    pub quality: FilterQuality,
    pub blend: SkBlend,
    pub mask: Option<&'a Mask>,
    pub band_w: u32,
    pub band_h: u32,
}

/// The device-pixel offset and sigma of a shadow, or `None` when it is off.
pub(crate) fn shadow_params(s: &Shadow, ratio: f32) -> Option<(i32, i32, f64)> {
    if s.color.a == 0 {
        return None;
    }
    let dx = (s.dx * ratio).round();
    let dy = (s.dy * ratio).round();
    let sigma = (s.blur * ratio) as f64 / 2.0;
    if dx == 0.0 && dy == 0.0 && sigma <= 0.0 {
        return None;
    }
    if !(dx.is_finite() && dy.is_finite() && sigma.is_finite()) {
        return None;
    }
    Some((dx.clamp(-1.0e5, 1.0e5) as i32, dy.clamp(-1.0e5, 1.0e5) as i32, sigma.min(1.0e3)))
}

/// Draw the shadow of a shape: the shape is rendered alone into a layer, its
/// alpha blurred, tinted with the shadow colour and composited at the offset.
pub(crate) fn draw_shadow(pm: &mut Pixmap, job: &ShadowJob<'_>) {
    let Some((dx, dy, sigma)) = shadow_params(&job.shadow, job.ratio) else { return };
    let margin = (3.0 * sigma).ceil() as i64;
    let b = job.path.bounds();
    // The layer covers the shape (grown by the blur) but only as much of it as can
    // reach the band: the band shifted back by the offset, grown by the blur.
    let lx0 = (b.left().floor() as i64 - margin).max(-(dx as i64) - margin);
    let ly0 = (b.top().floor() as i64 - margin).max(-(dy as i64) - margin);
    let lx1 = (b.right().ceil() as i64 + margin).min(job.band_w as i64 - dx as i64 + margin);
    let ly1 = (b.bottom().ceil() as i64 + margin).min(job.band_h as i64 - dy as i64 + margin);
    if lx1 <= lx0 || ly1 <= ly0 {
        return;
    }
    let (lw, lh) = ((lx1 - lx0) as u32, (ly1 - ly0) as u32);
    let Some(mut layer) = new_layer(lw, lh) else { return };
    let shift = Transform::from_translate(-(lx0 as f32), -(ly0 as f32));
    let Some(p) = job.path.clone().transform(shift) else { return };
    raster(
        &mut layer,
        &p,
        job.src,
        job.rule,
        job.shader_ts.post_concat(shift),
        job.alpha,
        SkBlend::SourceOver,
        job.quality,
        None,
    );
    // Alpha plane -> blur -> tint.
    let n = (lw as usize) * (lh as usize);
    let mut plane: Vec<u8> = Vec::with_capacity(n);
    plane.extend(layer.pixels().iter().map(|px| px.alpha()));
    blur_alpha(&mut plane, lw as usize, lh as usize, sigma);
    let c = job.shadow.color;
    for (px, a) in layer.pixels_mut().iter_mut().zip(plane.iter()) {
        let a2 = ((*a as u32 * c.a as u32 + 127) / 255) as u8;
        *px = tiny_skia::ColorU8::from_rgba(c.r, c.g, c.b, a2).premultiply();
    }
    let blend = if is_unbounded_sk(job.blend) { SkBlend::SourceOver } else { job.blend };
    composite_layer(pm, &layer, (lx0 + dx as i64) as i32, (ly0 + dy as i64) as i32, blend, job.mask);
}

fn is_unbounded_sk(b: SkBlend) -> bool {
    matches!(
        b,
        SkBlend::SourceIn | SkBlend::SourceOut | SkBlend::DestinationIn | SkBlend::DestinationAtop | SkBlend::Source
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbounded_set_is_exactly_the_operations_that_change_transparent_source_pixels() {
        let unb: Vec<_> = BlendMode::ALL.iter().filter(|m| is_unbounded(**m)).map(|m| m.css_name()).collect();
        assert_eq!(unb, ["source-in", "source-out", "destination-in", "destination-atop", "copy"]);
    }

    #[test]
    fn integer_translates_are_detected() {
        assert!(is_integer_translate(&Transform::from_translate(3.0, -7.0)));
        assert!(!is_integer_translate(&Transform::from_translate(3.5, 0.0)));
        assert!(!is_integer_translate(&Transform::from_scale(2.0, 2.0)));
    }

    #[test]
    fn gradient_stops_are_sorted_stably_and_clamped() {
        let s = prepare_stops(&[(1.5, Rgba::WHITE), (0.5, Rgba::BLACK), (0.5, Rgba::rgb(1, 2, 3)), (-1.0, Rgba::WHITE)], 1.0);
        let pos: Vec<f32> = s.iter().map(|g| g.0).collect();
        assert_eq!(pos, [0.0, 0.5, 0.5, 1.0]);
        assert_eq!(s[1].1, Color::from_rgba8(0, 0, 0, 255));
        assert_eq!(s[2].1, Color::from_rgba8(1, 2, 3, 255));
    }
}
