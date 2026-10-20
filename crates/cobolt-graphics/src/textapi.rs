// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the canvas needs from a text engine, and the CSS font parser.
//!
//! The canvas executor never touches a font file. It asks a [`TextEngine`] to
//! lay a text out and gets back, per line, the **outline path** of the shaped
//! glyphs plus the numbers needed to align them. The engine that ships in this
//! crate is [`text::FontSet`](crate::text::FontSet) (shaping, bidirectional
//! reordering and UAX #14 wrapping, behind the `text` feature); the trait keeps
//! the canvas usable, and testable, without it and lets a host with its own
//! fonts supply another.
//!
//! `MeasureText` and `DrawText` both go through [`TextEngine::layout`] with the
//! same request, so *what is measured is exactly what is drawn* (WASM-018 R8).

use std::sync::Arc;

use crate::display_list::consts::TextDirection;
use crate::error::Result;

/// Slant of a font.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum FontStyle {
    /// Upright.
    #[default]
    Normal,
    /// The italic face.
    Italic,
    /// A slanted face (treated as italic when no oblique face exists).
    Oblique,
}

/// A parsed CSS font: the families to try in order, the size in pixels, the
/// weight (100-900), the style and the stretch (percent of normal).
#[derive(Clone, Debug, PartialEq)]
pub struct FontSpec {
    /// Family names or generics (`serif`, `sans-serif`, `monospace`, ...), in
    /// order of preference.
    pub families: Vec<String>,
    /// Size in pixels.
    pub size: f32,
    /// 100 (thin) to 900 (black); 400 is regular, 700 bold.
    pub weight: u16,
    /// Slant.
    pub style: FontStyle,
    /// 50 to 200; 100 is normal width.
    pub stretch: u16,
}

impl Default for FontSpec {
    /// HTML's default: `10px sans-serif`.
    fn default() -> Self {
        Self { families: vec!["sans-serif".into()], size: 10.0, weight: 400, style: FontStyle::Normal, stretch: 100 }
    }
}

impl FontSpec {
    /// Parse the CSS `font` shorthand: `[style] [variant] [weight] [stretch]
    /// size[/line-height] family[, family ...]`. Sizes: `px`, `pt`, `em`,
    /// `rem` (16 px), a bare number is pixels. Returns `None` if no size and
    /// family can be found - the caller then keeps its current font, as the
    /// HTML canvas does.
    pub fn parse(css: &str) -> Option<FontSpec> {
        let css = css.trim();
        if css.is_empty() {
            return None;
        }
        let mut spec = FontSpec::default();
        let mut rest = css;
        // Leading keywords up to the size token.
        loop {
            rest = rest.trim_start();
            if rest.is_empty() {
                return None;
            }
            let (tok, after) = split_token(rest);
            let lower = tok.to_ascii_lowercase();
            match lower.as_str() {
                "normal" | "small-caps" => {}
                "italic" => spec.style = FontStyle::Italic,
                "oblique" => spec.style = FontStyle::Oblique,
                "bold" => spec.weight = 700,
                "bolder" => spec.weight = 900,
                "lighter" => spec.weight = 300,
                "ultra-condensed" => spec.stretch = 50,
                "extra-condensed" => spec.stretch = 62,
                "condensed" => spec.stretch = 75,
                "semi-condensed" => spec.stretch = 87,
                "semi-expanded" => spec.stretch = 112,
                "expanded" => spec.stretch = 125,
                "extra-expanded" => spec.stretch = 150,
                "ultra-expanded" => spec.stretch = 200,
                _ => {
                    if let Ok(w) = lower.parse::<u16>() {
                        if (1..=1000).contains(&w) {
                            spec.weight = w.clamp(100, 900);
                            rest = after;
                            continue;
                        }
                    }
                    // The size token (maybe with /line-height).
                    let size_part = tok.split('/').next().unwrap_or(tok);
                    spec.size = parse_length(size_part)?;
                    rest = after;
                    break;
                }
            }
            rest = after;
        }
        let families = parse_families(rest.trim());
        if families.is_empty() {
            return None;
        }
        spec.families = families;
        Some(spec)
    }
}

/// Split off the first whitespace-delimited token.
fn split_token(s: &str) -> (&str, &str) {
    match s.find(char::is_whitespace) {
        Some(i) => (&s[..i], &s[i..]),
        None => (s, ""),
    }
}

fn parse_length(tok: &str) -> Option<f32> {
    let t = tok.trim().to_ascii_lowercase();
    let (num, mul) = if let Some(n) = t.strip_suffix("px") {
        (n, 1.0)
    } else if let Some(n) = t.strip_suffix("pt") {
        (n, 4.0 / 3.0)
    } else if let Some(n) = t.strip_suffix("rem") {
        (n, 16.0)
    } else if let Some(n) = t.strip_suffix("em") {
        (n, 16.0)
    } else {
        (t.as_str(), 1.0)
    };
    let v: f32 = num.trim().parse().ok()?;
    let px = v * mul;
    (px.is_finite() && px > 0.0 && px <= 10_000.0).then_some(px)
}

fn parse_families(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in s.chars() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"') | (None, '\'') => quote = Some(ch),
            (None, ',') => {
                let name = cur.trim().to_string();
                if !name.is_empty() {
                    out.push(name);
                }
                cur.clear();
            }
            (None, c) => cur.push(c),
        }
    }
    let name = cur.trim().to_string();
    if !name.is_empty() {
        out.push(name);
    }
    out
}

/// What a canvas wants laid out.
#[derive(Clone, Debug)]
pub struct TextRequest<'a> {
    /// The text. `\n` ends a line (and a paragraph).
    pub text: &'a str,
    /// The font.
    pub font: &'a FontSpec,
    /// The base direction.
    pub direction: TextDirection,
    /// Wrap lines at this width (pixels); `None` for no wrapping.
    pub wrap_width: Option<f32>,
    /// Distance between baselines; `None` for the font's natural line height.
    pub line_height: Option<f32>,
}

/// One laid-out line.
#[derive(Clone, Debug)]
pub struct TextLine {
    /// The outlines of every glyph of the line, with the **left edge of the
    /// line box at x = 0** and its **baseline at y = 0** (y grows downward), in
    /// pixels at the requested size. `None` for a line with nothing to draw.
    pub path: Option<Arc<tiny_skia::Path>>,
    /// Width of the line box, in pixels (the sum of the glyph advances).
    pub advance: f32,
    /// Distance from the first line's baseline to this line's baseline.
    pub baseline: f32,
    /// The ink box `(left, top, right, bottom)` in line coordinates, or `None`
    /// when there is no ink.
    pub ink: Option<(f32, f32, f32, f32)>,
    /// Whether the line's paragraph direction is right-to-left.
    pub rtl: bool,
}

/// A laid-out text: one line, or several when wrapped.
#[derive(Clone, Debug)]
pub struct TextBlock {
    /// The lines, top to bottom.
    pub lines: Vec<TextLine>,
    /// Width of the widest line.
    pub width: f32,
    /// Font ascent in pixels (positive).
    pub ascent: f32,
    /// Font descent in pixels (positive).
    pub descent: f32,
    /// Baseline-to-baseline distance used.
    pub line_height: f32,
    /// Direction of the first paragraph.
    pub rtl: bool,
}

/// The answer of `MeasureText` (HTML `TextMetrics`, in pixels).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TextMetrics {
    /// Advance width of the text.
    pub width: f32,
    /// Distance from the alignment point to the left of the ink (positive to the left).
    pub actual_left: f32,
    /// Distance from the alignment point to the right of the ink.
    pub actual_right: f32,
    /// Distance from the baseline to the top of the ink (positive up).
    pub actual_ascent: f32,
    /// Distance from the baseline to the bottom of the ink (positive down).
    pub actual_descent: f32,
    /// Font ascent.
    pub font_ascent: f32,
    /// Font descent.
    pub font_descent: f32,
    /// Number of lines (1 unless wrapped).
    pub lines: u32,
}

/// Lays text out. Implemented by [`text::FontSet`](crate::text::FontSet).
pub trait TextEngine: Send + Sync {
    /// Shape, order and (optionally) wrap the text.
    fn layout(&self, request: &TextRequest<'_>) -> Result<TextBlock>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_css_shorthand() {
        let f = FontSpec::parse("italic bold 16px 'Noto Sans', Arial, sans-serif").unwrap();
        assert_eq!(f.style, FontStyle::Italic);
        assert_eq!(f.weight, 700);
        assert_eq!(f.size, 16.0);
        assert_eq!(f.families, ["Noto Sans", "Arial", "sans-serif"]);
    }

    #[test]
    fn parses_numeric_weights_and_units() {
        let f = FontSpec::parse("600 12pt serif").unwrap();
        assert_eq!(f.weight, 600);
        assert!((f.size - 16.0).abs() < 1e-5);
        let f = FontSpec::parse("2em/1.5 monospace").unwrap();
        assert_eq!(f.size, 32.0);
        assert_eq!(f.families, ["monospace"]);
        let f = FontSpec::parse("condensed 10px sans-serif").unwrap();
        assert_eq!(f.stretch, 75);
    }

    #[test]
    fn rejects_what_html_would_ignore() {
        for bad in ["", "   ", "16px", "bold", "sans-serif", "-4px Arial", "0px Arial", "abc Arial", "1e400px Arial"] {
            assert_eq!(FontSpec::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn default_is_the_html_default() {
        let d = FontSpec::default();
        assert_eq!((d.size, d.weight), (10.0, 400));
        assert_eq!(d.families, ["sans-serif"]);
    }
}
