// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! plotters drawing backends of our own.
//!
//! `plotters` draws through a [`DrawingBackend`]. Its stock bitmap backend needs
//! fonts from the operating system (the `font-kit` feature, which links C
//! libraries), and on `wasm32` its text measuring asks the browser's DOM - which
//! neither works in a Worker nor gives the same widths as the desktop. Here both
//! are replaced:
//!
//! * [`ListBackend`] **records the chart as display-list commands** (lines,
//!   polygons, circles, text), so a chart is drawn by the same rasteriser as
//!   anything else on a canvas - anti-aliased, banded across threads when large,
//!   identical on every platform - and a chart can be recorded straight into a
//!   `Canvas2D`'s own list;
//! * [`SvgPlotBackend`] writes SVG through plotters' SVG backend.
//!
//! Both measure text with this crate's [`TextEngine`], so the layout is the same
//! in the pixels and in the SVG and in every language.

use plotters::prelude::SVGBackend;
use plotters_backend::text_anchor::{HPos, VPos};
use plotters_backend::{BackendColor, BackendCoord, BackendStyle, BackendTextStyle, DrawingBackend, DrawingErrorKind, FontStyle as PFontStyle, FontTransform};

use crate::color::Rgba;
use crate::display_list::consts::{TextDirection, MODE_CLOSE, MODE_FILL, MODE_STROKE};
use crate::display_list::DisplayListBuilder;
use crate::textapi::{FontSpec, FontStyle, TextBlock, TextEngine, TextRequest};

/// The error type of the backends: a message.
#[derive(Debug, Clone)]
pub struct BackendError(pub String);

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for BackendError {}

fn spec_of<S: BackendTextStyle>(style: &S) -> FontSpec {
    let mut spec = FontSpec { families: vec![style.family().as_str().to_string()], size: style.size().max(1.0) as f32, ..FontSpec::default() };
    match style.style() {
        PFontStyle::Normal => {}
        PFontStyle::Bold => spec.weight = 700,
        PFontStyle::Italic => spec.style = FontStyle::Italic,
        PFontStyle::Oblique => spec.style = FontStyle::Oblique,
    }
    spec
}

fn css_of(spec: &FontSpec) -> String {
    let style = match spec.style {
        FontStyle::Normal => "normal",
        FontStyle::Italic => "italic",
        FontStyle::Oblique => "oblique",
    };
    let fam = spec.families.iter().map(|f| format!("\"{}\"", f.replace('"', ""))).collect::<Vec<_>>().join(", ");
    format!("{style} {} {}px {fam}", spec.weight, spec.size)
}

fn layout_of<S: BackendTextStyle>(engine: &dyn TextEngine, text: &str, style: &S) -> Option<TextBlock> {
    let font = spec_of(style);
    engine.layout(&TextRequest { text, font: &font, direction: TextDirection::Auto, wrap_width: None, line_height: None }).ok()
}

/// Width and height of a text box, in whole pixels, as the engine will draw it.
fn text_size<S: BackendTextStyle>(engine: Option<&dyn TextEngine>, text: &str, style: &S) -> (u32, u32) {
    if let Some(block) = engine.and_then(|e| layout_of(e, text, style)) {
        let w = block.lines.first().map(|l| l.advance).unwrap_or(0.0);
        return (w.ceil() as u32, (block.ascent + block.descent).ceil() as u32);
    }
    let size = style.size().max(1.0);
    ((text.chars().count() as f64 * size * 0.55).ceil() as u32, (size * 1.2).ceil() as u32)
}

fn rgba(c: BackendColor) -> Rgba {
    Rgba::new(c.rgb.0, c.rgb.1, c.rgb.2, (c.alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
}

type Res<T> = Result<T, DrawingErrorKind<BackendError>>;

fn center(p: BackendCoord) -> (f32, f32) {
    (p.0 as f32 + 0.5, p.1 as f32 + 0.5)
}

/// Records a plotters chart as display-list commands.
///
/// The chart occupies `size` pixels with its top-left corner at the current origin
/// of the list (use `Translate` first to place it elsewhere on a canvas).
pub struct ListBackend<'a> {
    list: &'a mut DisplayListBuilder,
    size: (u32, u32),
    text: Option<&'a dyn TextEngine>,
    stroke: Option<(Rgba, u32)>,
    fill: Option<Rgba>,
}

impl<'a> ListBackend<'a> {
    /// Record into `list`; `text` supplies fonts (without it labels are not
    /// drawn and sized by a rough estimate).
    pub fn new(list: &'a mut DisplayListBuilder, size: (u32, u32), text: Option<&'a dyn TextEngine>) -> Self {
        // A known starting state for the strokes the chart draws.
        list.set_line_cap(0).set_line_join(1).set_miter_limit(10.0).set_dash(&[], 0.0).set_alpha(1.0).set_blend_mode(0);
        Self { list, size, text, stroke: None, fill: None }
    }

    fn stroke_style(&mut self, color: Rgba, width: u32) {
        let want = Some((color, width.max(1)));
        if self.stroke != want {
            self.list.set_stroke_color(color).set_line_width(width.max(1) as f32);
            self.stroke = want;
        }
    }

    fn fill_style(&mut self, color: Rgba) {
        if self.fill != Some(color) {
            self.list.set_fill_color(color);
            self.fill = Some(color);
        }
    }
}

impl DrawingBackend for ListBackend<'_> {
    type ErrorType = BackendError;

    fn get_size(&self) -> (u32, u32) {
        self.size
    }

    fn ensure_prepared(&mut self) -> Res<()> {
        Ok(())
    }

    fn present(&mut self) -> Res<()> {
        Ok(())
    }

    fn draw_pixel(&mut self, point: BackendCoord, color: BackendColor) -> Res<()> {
        self.fill_style(rgba(color));
        self.list.fill_rect(point.0 as f32, point.1 as f32, 1.0, 1.0);
        Ok(())
    }

    fn draw_line<S: BackendStyle>(&mut self, from: BackendCoord, to: BackendCoord, style: &S) -> Res<()> {
        self.stroke_style(rgba(style.color()), style.stroke_width());
        let (a, b) = (center(from), center(to));
        self.list.draw_line(a.0, a.1, b.0, b.1);
        Ok(())
    }

    fn draw_rect<S: BackendStyle>(&mut self, upper_left: BackendCoord, bottom_right: BackendCoord, style: &S, fill: bool) -> Res<()> {
        let (x0, y0) = (upper_left.0.min(bottom_right.0), upper_left.1.min(bottom_right.1));
        let (x1, y1) = (upper_left.0.max(bottom_right.0), upper_left.1.max(bottom_right.1));
        if fill {
            self.fill_style(rgba(style.color()));
            self.list.fill_rect(x0 as f32, y0 as f32, (x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
        } else {
            self.stroke_style(rgba(style.color()), style.stroke_width());
            let (a, b) = (center((x0, y0)), center((x1, y1)));
            self.list.stroke_rect(a.0, a.1, b.0 - a.0, b.1 - a.1);
        }
        Ok(())
    }

    fn draw_path<S: BackendStyle, I: IntoIterator<Item = BackendCoord>>(&mut self, path: I, style: &S) -> Res<()> {
        self.stroke_style(rgba(style.color()), style.stroke_width());
        let pts: Vec<(f32, f32)> = path.into_iter().map(center).collect();
        if pts.len() >= 2 {
            self.list.draw_polygon(&pts, MODE_STROKE);
        }
        Ok(())
    }

    fn draw_circle<S: BackendStyle>(&mut self, c: BackendCoord, radius: u32, style: &S, fill: bool) -> Res<()> {
        let (x, y) = center(c);
        if fill {
            self.fill_style(rgba(style.color()));
            self.list.draw_circle(x, y, radius as f32 + 0.5, MODE_FILL);
        } else {
            self.stroke_style(rgba(style.color()), style.stroke_width());
            self.list.draw_circle(x, y, radius as f32, MODE_STROKE);
        }
        Ok(())
    }

    fn fill_polygon<S: BackendStyle, I: IntoIterator<Item = BackendCoord>>(&mut self, vert: I, style: &S) -> Res<()> {
        self.fill_style(rgba(style.color()));
        let pts: Vec<(f32, f32)> = vert.into_iter().map(center).collect();
        if pts.len() >= 3 {
            self.list.draw_polygon(&pts, MODE_FILL | MODE_CLOSE);
        }
        Ok(())
    }

    fn draw_text<S: BackendTextStyle>(&mut self, text: &str, style: &S, pos: BackendCoord) -> Res<()> {
        let color = style.color();
        if color.alpha == 0.0 || text.is_empty() || self.text.is_none() {
            return Ok(());
        }
        let (w, h) = text_size(self.text, text, style);
        let dx = match style.anchor().h_pos {
            HPos::Left => 0,
            HPos::Right => -(w as i32),
            HPos::Center => -(w as i32) / 2,
        };
        let dy = match style.anchor().v_pos {
            VPos::Top => 0,
            VPos::Center => -(h as i32) / 2,
            VPos::Bottom => -(h as i32),
        };
        let angle = match style.transform() {
            FontTransform::None => 0.0,
            FontTransform::Rotate90 => std::f32::consts::FRAC_PI_2,
            FontTransform::Rotate180 => std::f32::consts::PI,
            FontTransform::Rotate270 => 3.0 * std::f32::consts::FRAC_PI_2,
        };
        let css = css_of(&spec_of(style));
        self.list.save();
        self.list.translate(pos.0 as f32, pos.1 as f32);
        if angle != 0.0 {
            self.list.rotate(angle);
        }
        self.list.set_font(&css).set_text_align(2).set_text_baseline(1).set_text_direction(0).set_fill_color(rgba(color));
        self.list.draw_text(text, dx as f32, dy as f32, 0.0, MODE_FILL);
        // `restore` brings back the stroke and fill that were set before `save`, which is
        // what the caches describe.
        self.list.restore();
        Ok(())
    }

    fn estimate_text_size<S: BackendTextStyle>(&self, text: &str, style: &S) -> Res<(u32, u32)> {
        Ok(text_size(self.text, text, style))
    }

    fn blit_bitmap(&mut self, pos: BackendCoord, (iw, ih): (u32, u32), src: &[u8]) -> Res<()> {
        for y in 0..ih {
            for x in 0..iw {
                let i = ((x + y * iw) * 3) as usize;
                if i + 2 < src.len() {
                    self.draw_pixel((pos.0 + x as i32, pos.1 + y as i32), BackendColor { alpha: 1.0, rgb: (src[i], src[i + 1], src[i + 2]) })?;
                }
            }
        }
        Ok(())
    }
}

/// plotters' SVG backend with text measured by this crate's engine.
pub struct SvgPlotBackend<'a> {
    inner: SVGBackend<'a>,
    text: Option<&'a dyn TextEngine>,
}

impl<'a> SvgPlotBackend<'a> {
    /// Write SVG text into `out`, `size` pixels.
    pub fn with_string(out: &'a mut String, size: (u32, u32), text: Option<&'a dyn TextEngine>) -> Self {
        Self { inner: SVGBackend::with_string(out, size), text }
    }
}

type SvgErr = <SVGBackend<'static> as DrawingBackend>::ErrorType;

impl DrawingBackend for SvgPlotBackend<'_> {
    type ErrorType = SvgErr;

    fn get_size(&self) -> (u32, u32) {
        self.inner.get_size()
    }
    fn ensure_prepared(&mut self) -> Result<(), DrawingErrorKind<SvgErr>> {
        self.inner.ensure_prepared()
    }
    fn present(&mut self) -> Result<(), DrawingErrorKind<SvgErr>> {
        self.inner.present()
    }
    fn draw_pixel(&mut self, point: BackendCoord, color: BackendColor) -> Result<(), DrawingErrorKind<SvgErr>> {
        self.inner.draw_pixel(point, color)
    }
    fn draw_line<S: BackendStyle>(&mut self, from: BackendCoord, to: BackendCoord, style: &S) -> Result<(), DrawingErrorKind<SvgErr>> {
        self.inner.draw_line(from, to, style)
    }
    fn draw_rect<S: BackendStyle>(&mut self, ul: BackendCoord, br: BackendCoord, style: &S, fill: bool) -> Result<(), DrawingErrorKind<SvgErr>> {
        self.inner.draw_rect(ul, br, style, fill)
    }
    fn draw_path<S: BackendStyle, I: IntoIterator<Item = BackendCoord>>(&mut self, path: I, style: &S) -> Result<(), DrawingErrorKind<SvgErr>> {
        self.inner.draw_path(path, style)
    }
    fn draw_circle<S: BackendStyle>(&mut self, c: BackendCoord, radius: u32, style: &S, fill: bool) -> Result<(), DrawingErrorKind<SvgErr>> {
        self.inner.draw_circle(c, radius, style, fill)
    }
    fn fill_polygon<S: BackendStyle, I: IntoIterator<Item = BackendCoord>>(&mut self, vert: I, style: &S) -> Result<(), DrawingErrorKind<SvgErr>> {
        self.inner.fill_polygon(vert, style)
    }
    fn draw_text<S: BackendTextStyle>(&mut self, text: &str, style: &S, pos: BackendCoord) -> Result<(), DrawingErrorKind<SvgErr>> {
        self.inner.draw_text(text, style, pos)
    }
    fn estimate_text_size<S: BackendTextStyle>(&self, text: &str, style: &S) -> Result<(u32, u32), DrawingErrorKind<SvgErr>> {
        Ok(text_size(self.text, text, style))
    }
    fn blit_bitmap(&mut self, pos: BackendCoord, size: (u32, u32), src: &[u8]) -> Result<(), DrawingErrorKind<SvgErr>> {
        self.inner.blit_bitmap(pos, size, src)
    }
}
