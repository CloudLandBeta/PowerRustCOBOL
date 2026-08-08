// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Monochrome flags for the language selector.
//!
//! # Why painted, and why not in colour
//!
//! `"🇧🇷"` is two *regional indicator* codepoints, and a flag appears only when
//! the text stack ligates them. egui does no such shaping, and the emoji font it
//! bundles maps those codepoints to plain boxed **letters** — which is why the
//! selector once read `B R`. A colour emoji font cannot help either: Apple Color
//! Emoji is `sbix`, and egui's rasteriser decodes outlines only. So the flags are
//! drawn.
//!
//! They are drawn in the **theme's own two ink tones on the theme's own panel
//! colour**, never in national colours: a fixed palette cannot stay legible
//! across 31 themes from High Contrast to Solarized Light, and a coloured patch
//! next to themed text looks pasted on. Every tone here therefore comes from
//! [`crate::theme`], and [`tests`] holds every theme to WCAG contrast minimums.
//!
//! # Legibility rests on geometry, not tone
//!
//! `text_bright` and `text_dim` are as close as 1.17:1 in some themes (Nord), so
//! two tones cannot be what tells two flags apart. Each flag is distinguished by
//! its *shape* — canton and stripes, rhombus and disc, horizontal versus vertical
//! bands, a lone disc, a star — and a test enforces exactly that.

use egui::{pos2, vec2, Color32, Pos2, Rect, Sense, Shape, Stroke, Vec2};

use crate::i18n::Language;

/// Flag box size next to a language name. 3:2 is the most common flag ratio and
/// sits comfortably on one line of button text.
pub const FLAG_SIZE: Vec2 = vec2(21.0, 14.0);

/// The three tones a flag is drawn with, all taken from the active theme.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Palette {
    /// The flag's own background — the theme's panel colour.
    field: Color32,
    /// Primary charge colour: the theme's brightest text.
    ink: Color32,
    /// Secondary charge colour: the theme's dim text.
    mid: Color32,
}

/// Opaque so the contrast a flag achieves is a property of the theme alone and
/// not of whatever happens to show through a translucent panel colour.
fn opaque(c: Color32) -> Color32 {
    Color32::from_rgb(c.r(), c.g(), c.b())
}

fn palette(theme: &crate::theme::Theme) -> Palette {
    Palette {
        field: opaque(theme.bg_panel),
        ink: opaque(theme.text_bright),
        mid: opaque(theme.text_dim),
    }
}

/// Paint `lang`'s flag inside `rect`, in the active theme's tones.
pub fn paint_flag(painter: &egui::Painter, rect: Rect, lang: Language) {
    let p = palette(&crate::theme::active());
    painter.extend(flag_shapes(rect, lang, p));
}

/// `lang`'s flag as shapes confined to `rect`. Simplified to what survives at
/// 21×14 px: the field, the charges that identify the flag, and a rim.
///
/// Pure geometry so the result can be asserted in tests — a flag that paints
/// nothing, escapes its box, or is indistinguishable from another is caught
/// without a screenshot.
fn flag_shapes(rect: Rect, lang: Language, p: Palette) -> Vec<Shape> {
    let mut out: Vec<Shape> = Vec::new();
    let (w, h) = (rect.width(), rect.height());
    let fill = |out: &mut Vec<Shape>, r: Rect, c: Color32| {
        out.push(Shape::rect_filled(r, 0.0, c));
    };
    let band = |out: &mut Vec<Shape>, y0: f32, y1: f32, c: Color32| {
        fill(
            out,
            Rect::from_min_max(
                pos2(rect.left(), rect.top() + h * y0),
                pos2(rect.right(), rect.top() + h * y1),
            ),
            c,
        );
    };
    let column = |out: &mut Vec<Shape>, x0: f32, x1: f32, c: Color32| {
        fill(
            out,
            Rect::from_min_max(
                pos2(rect.left() + w * x0, rect.top()),
                pos2(rect.left() + w * x1, rect.bottom()),
            ),
            c,
        );
    };

    fill(&mut out, rect, p.field);
    match lang {
        // United States — thirteen stripes, painted first and last, with the
        // canton over the upper seven of them and two fifths of the width.
        Language::English => {
            let stripe = h / 13.0;
            let canton = Rect::from_min_size(rect.min, vec2(w * 0.4, stripe * 7.0));
            // Odd rows are left as the field, so only the seven painted stripes
            // are emitted: 0, 2, 4 and 6 beside the canton, 8, 10 and 12 across
            // the whole width.
            for i in (0..13).step_by(2) {
                let left = if i < 7 { canton.right() } else { rect.left() };
                fill(
                    &mut out,
                    Rect::from_min_max(
                        pos2(left, rect.top() + i as f32 * stripe),
                        pos2(rect.right(), rect.top() + (i as f32 + 1.0) * stripe),
                    ),
                    p.mid,
                );
            }
            fill(&mut out, canton, p.ink);
        }
        // Spain — narrow band, wide band, narrow band.
        Language::Spanish => {
            band(&mut out, 0.0, 0.25, p.ink);
            band(&mut out, 0.75, 1.0, p.ink);
        }
        // Brazil — the rhombus with the globe punched out of it.
        Language::Portuguese => {
            let c = rect.center();
            let (rx, ry) = (w * 0.40, h * 0.40);
            out.push(Shape::convex_polygon(
                vec![
                    pos2(c.x, c.y - ry),
                    pos2(c.x + rx, c.y),
                    pos2(c.x, c.y + ry),
                    pos2(c.x - rx, c.y),
                ],
                p.mid,
                Stroke::NONE,
            ));
            out.push(Shape::circle_filled(c, h * 0.20, p.ink));
        }
        // Japan — the disc alone, centred on an empty field.
        Language::Japanese => {
            out.push(Shape::circle_filled(rect.center(), h * 0.30, p.ink));
        }
        // China — a filled field carrying the star and its four companions.
        Language::Chinese => {
            fill(&mut out, rect, p.mid);
            out.push(star(
                pos2(rect.left() + w * 0.20, rect.top() + h * 0.32),
                h * 0.22,
                p.ink,
            ));
            for (fx, fy) in [(0.42, 0.12), (0.52, 0.28), (0.52, 0.50), (0.42, 0.64)] {
                out.push(Shape::circle_filled(
                    pos2(rect.left() + w * fx, rect.top() + h * fy),
                    h * 0.055,
                    p.ink,
                ));
            }
        }
        // France — vertical thirds.
        Language::French => {
            column(&mut out, 0.0, 1.0 / 3.0, p.ink);
            column(&mut out, 2.0 / 3.0, 1.0, p.mid);
        }
    }
    // Rim: keeps the field's edge readable when field and surroundings are close.
    out.push(Shape::rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, p.mid),
        egui::StrokeKind::Inside,
    ));
    out
}

/// A filled five-pointed star of circumradius `r`, point up. Concave, so it
/// needs the general path — `convex_polygon` would render a pentagon.
fn star(center: Pos2, r: f32, color: Color32) -> Shape {
    let mut points = Vec::with_capacity(10);
    for i in 0..10 {
        let radius = if i % 2 == 0 { r } else { r * 0.42 };
        let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        points.push(pos2(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        ));
    }
    Shape::Path(egui::epaint::PathShape {
        points,
        closed: true,
        fill: color,
        stroke: egui::epaint::PathStroke::NONE,
    })
}

/// One selectable row in the language dropdown: flag, then the language's own
/// name. The whole row is the click target.
///
/// Sized from its own content (`allocate_at_least`), never from the space it is
/// offered — a child that measures the room it is given makes its container
/// creep wider every frame.
pub fn language_row(ui: &mut egui::Ui, lang: Language, selected: bool) -> egui::Response {
    const GAP: f32 = 8.0;
    const PAD: f32 = 4.0;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley =
        ui.painter()
            .layout_no_wrap(lang.native_name().to_owned(), font, Color32::PLACEHOLDER);
    let desired = vec2(
        PAD * 2.0 + FLAG_SIZE.x + GAP + galley.size().x,
        galley.size().y.max(FLAG_SIZE.y) + PAD * 2.0,
    );
    let (rect, response) = ui.allocate_at_least(desired, Sense::click());

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, selected);
        if selected || response.hovered() {
            ui.painter().rect_filled(rect, 2.0, visuals.weak_bg_fill);
        }
        let flag_rect = Rect::from_min_size(
            pos2(rect.left() + PAD, rect.center().y - FLAG_SIZE.y * 0.5),
            FLAG_SIZE,
        );
        paint_flag(ui.painter(), flag_rect, lang);
        ui.painter().galley(
            pos2(
                flag_rect.right() + GAP,
                rect.center().y - galley.size().y * 0.5,
            ),
            galley,
            visuals.text_color(),
        );
    }
    response
}

/// Paint just the flag inline (next to a closed combo box), advancing the cursor
/// by [`FLAG_SIZE`].
pub fn flag_widget(ui: &mut egui::Ui, lang: Language) -> egui::Response {
    let (rect, response) = ui.allocate_at_least(FLAG_SIZE, Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_flag(ui.painter(), rect, lang);
    }
    response
}

// ── Contrast ──────────────────────────────────────────────────────────────────
//
// The WCAG math itself lives in `crate::contrast` — shared with the Project's
// Crates System-crate markers (spec 045 R10), which need the exact same
// legibility floor against a themed background.
use crate::contrast::contrast_ratio;

#[cfg(test)]
mod tests {
    use super::*;

    const BOX: Rect = Rect {
        min: pos2(10.0, 20.0),
        max: pos2(31.0, 34.0), // 21 × 14
    };

    /// WCAG AA for text and images of text.
    const MIN_INK_CONTRAST: f64 = 4.5;
    /// WCAG AA for graphical objects — the tone carrying secondary charges.
    const MIN_MID_CONTRAST: f64 = 3.0;

    fn test_palette() -> Palette {
        palette(crate::theme::default_theme())
    }

    #[test]
    fn contrast_ratio_matches_the_wcag_reference_points() {
        // Black on white is the definition's upper bound; identical colours are 1.
        assert!((contrast_ratio(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(Color32::WHITE, Color32::WHITE) - 1.0).abs() < 0.001);
        // Symmetric in its arguments.
        let (a, b) = (Color32::from_rgb(84, 110, 122), Color32::WHITE);
        assert!((contrast_ratio(a, b) - contrast_ratio(b, a)).abs() < 1e-9);
    }

    /// The point of drawing flags from theme colours: in **every** theme the
    /// charges must stay legible against the field they sit on. A new theme with
    /// washed-out text colours fails here rather than in the user's eyes.
    #[test]
    fn every_theme_paints_flags_with_high_contrast() {
        let mut worst_ink = (f64::MAX, "");
        let mut worst_mid = (f64::MAX, "");
        for theme in crate::theme::THEMES {
            let p = palette(theme);
            let ink = contrast_ratio(p.ink, p.field);
            let mid = contrast_ratio(p.mid, p.field);
            assert!(
                ink >= MIN_INK_CONTRAST,
                "theme {}: primary charge vs field is {ink:.2}:1, below WCAG AA {MIN_INK_CONTRAST}:1",
                theme.id
            );
            assert!(
                mid >= MIN_MID_CONTRAST,
                "theme {}: secondary charge vs field is {mid:.2}:1, below WCAG AA {MIN_MID_CONTRAST}:1 for graphics",
                theme.id
            );
            if ink < worst_ink.0 {
                worst_ink = (ink, theme.id);
            }
            if mid < worst_mid.0 {
                worst_mid = (mid, theme.id);
            }
        }
        // Reported so a change that erodes the margin is visible with --nocapture.
        println!(
            "worst ink {:.2}:1 ({}), worst mid {:.2}:1 ({})",
            worst_ink.0, worst_ink.1, worst_mid.0, worst_mid.1
        );
    }

    /// Nothing may be painted in a colour outside the theme's palette — that is
    /// what "no colourful flags" means, and it is easy to reintroduce by pasting
    /// a national colour into a new arm.
    #[test]
    fn no_flag_uses_a_colour_outside_the_theme_palette() {
        for theme in crate::theme::THEMES {
            let p = palette(theme);
            let allowed = [p.field, p.ink, p.mid];
            for &lang in Language::ALL {
                for shape in flag_shapes(BOX, lang, p) {
                    for c in shape_colors(&shape) {
                        assert!(
                            allowed.contains(&c),
                            "{lang:?} in theme {} paints {c:?}, not one of {allowed:?}",
                            theme.id
                        );
                    }
                }
            }
        }
    }

    /// Tone cannot be what tells two flags apart: `text_bright` and `text_dim`
    /// are only 1.17:1 apart in Nord. So the *geometry* must differ — this
    /// compares the shapes with every colour stripped out.
    #[test]
    fn every_flag_is_distinguishable_by_geometry_alone() {
        let p = test_palette();
        let fingerprints: Vec<String> = Language::ALL
            .iter()
            .map(|&l| {
                flag_shapes(BOX, l, p)
                    .iter()
                    .map(|s| {
                        let b = s.visual_bounding_rect();
                        format!(
                            "{}:{:.1},{:.1},{:.1},{:.1}",
                            shape_kind(s),
                            b.left(),
                            b.top(),
                            b.right(),
                            b.bottom()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("|")
            })
            .collect();
        for (i, a) in fingerprints.iter().enumerate() {
            for (j, b) in fingerprints.iter().enumerate().skip(i + 1) {
                assert_ne!(
                    a,
                    b,
                    "{:?} and {:?} are the same shape — indistinguishable in monochrome",
                    Language::ALL[i],
                    Language::ALL[j]
                );
            }
        }
    }

    /// A missing match arm would leave a bare field where a flag belongs.
    #[test]
    fn every_language_paints_a_field_and_a_charge() {
        let p = test_palette();
        for &lang in Language::ALL {
            let shapes = flag_shapes(BOX, lang, p);
            assert!(
                shapes.len() >= 3,
                "{lang:?} painted only {} shape(s) — field, charge and rim expected",
                shapes.len()
            );
        }
    }

    /// A flag that overflows would paint over the language name beside it.
    #[test]
    fn nothing_is_painted_outside_the_box() {
        let p = test_palette();
        for &lang in Language::ALL {
            for shape in flag_shapes(BOX, lang, p) {
                let b = shape.visual_bounding_rect();
                assert!(
                    BOX.expand(1.5).contains_rect(b),
                    "{lang:?} paints {b:?} outside {BOX:?}"
                );
            }
        }
    }

    /// The star is concave — as a convex polygon it would come out a pentagon.
    #[test]
    fn star_is_a_ten_point_path() {
        match star(pos2(10.0, 10.0), 5.0, Color32::WHITE) {
            Shape::Path(p) => {
                assert_eq!(p.points.len(), 10);
                assert!(p.closed);
            }
            other => panic!("expected a path, got {other:?}"),
        }
    }

    /// The United States flag is thirteen equal stripes, painted first and last,
    /// with the canton over the upper seven and two fifths of the width. Only the
    /// even rows are painted; the four beside the canton start at its edge, the
    /// three below it run the full width, and the last one reaches the bottom.
    #[test]
    fn us_flag_is_thirteen_stripes_with_a_seven_stripe_canton() {
        let p = test_palette();
        let stripe = BOX.height() / 13.0;
        let canton_right = BOX.left() + BOX.width() * 0.4;
        let stripes: Vec<Rect> = flag_shapes(BOX, Language::English, p)
            .iter()
            .filter_map(|s| match s {
                Shape::Rect(r) if r.fill == p.mid => Some(r.rect),
                _ => None,
            })
            .collect();
        assert_eq!(
            stripes.len(),
            7,
            "expected the seven painted stripes, got {stripes:?}"
        );
        for (n, r) in stripes.iter().enumerate() {
            let row = (n * 2) as f32;
            let want_left = if n < 4 { canton_right } else { BOX.left() };
            assert!(
                (r.top() - (BOX.top() + row * stripe)).abs() < 0.01
                    && (r.bottom() - (BOX.top() + (row + 1.0) * stripe)).abs() < 0.01,
                "stripe {n} should be row {row}, is {r:?}"
            );
            assert!(
                (r.left() - want_left).abs() < 0.01 && (r.right() - BOX.right()).abs() < 0.01,
                "stripe {n} should span {want_left}..{}, is {r:?}",
                BOX.right()
            );
        }
        // Painted first and last: no bare field at the top or bottom edge.
        assert!((stripes[0].top() - BOX.top()).abs() < 0.01);
        assert!((stripes[6].bottom() - BOX.bottom()).abs() < 0.05);
        // The canton: two fifths wide, seven stripes tall, in the hoist corner.
        let canton = flag_shapes(BOX, Language::English, p)
            .into_iter()
            .find_map(|s| match s {
                Shape::Rect(r) if r.fill == p.ink => Some(r.rect),
                _ => None,
            })
            .expect("the canton");
        assert!((canton.left() - BOX.left()).abs() < 0.01);
        assert!((canton.top() - BOX.top()).abs() < 0.01);
        assert!((canton.right() - canton_right).abs() < 0.01);
        assert!((canton.bottom() - (BOX.top() + 7.0 * stripe)).abs() < 0.01);
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    /// The shape's kind, with no colour in it — half of a geometry fingerprint.
    fn shape_kind(shape: &Shape) -> &'static str {
        match shape {
            Shape::Rect(_) => "rect",
            Shape::Circle(_) => "circle",
            Shape::Path(_) => "path",
            Shape::Vec(_) => "vec",
            _ => "other",
        }
    }

    /// Every colour a shape paints with (fill and stroke).
    fn shape_colors(shape: &Shape) -> Vec<Color32> {
        match shape {
            Shape::Rect(r) => {
                let mut v = vec![r.fill];
                if r.stroke.width > 0.0 {
                    v.push(r.stroke.color);
                }
                v.retain(|c| *c != Color32::TRANSPARENT);
                v
            }
            Shape::Circle(c) => {
                let mut v = vec![c.fill];
                if c.stroke.width > 0.0 {
                    v.push(c.stroke.color);
                }
                v.retain(|c| *c != Color32::TRANSPARENT);
                v
            }
            Shape::Path(p) => {
                let mut v = vec![p.fill];
                v.retain(|c| *c != Color32::TRANSPARENT);
                v
            }
            Shape::Vec(shapes) => shapes.iter().flat_map(shape_colors).collect(),
            _ => Vec::new(),
        }
    }
}
