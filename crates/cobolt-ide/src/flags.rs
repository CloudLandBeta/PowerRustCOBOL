// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Painted flags for the language selector.
//!
//! # Why not emoji
//!
//! `"🇧🇷"` is two *regional indicator* codepoints, and a flag appears only when
//! the text stack ligates them. egui does no such shaping, and the emoji font it
//! bundles (`NotoEmoji-Regular`) maps those codepoints to plain boxed **letters**
//! — which is why the selector read `B R`, `U S`, `F R`… A colour emoji font
//! would not help either: Apple Color Emoji is `sbix`, and egui's rasteriser
//! decodes outlines only.
//!
//! So the flags are drawn. They are deliberately simplified — recognisable at
//! 21×14 px, crisp at any size, no image assets and no licence questions.

use egui::{pos2, vec2, Color32, Pos2, Rect, Sense, Shape, Stroke, Vec2};

use crate::i18n::Language;

/// Flag box size next to a language name. 3:2 is the most common flag ratio and
/// sits comfortably on one line of button text.
pub const FLAG_SIZE: Vec2 = vec2(21.0, 14.0);

/// Paint `lang`'s flag inside `rect`.
pub fn paint_flag(painter: &egui::Painter, rect: Rect, lang: Language) {
    painter.extend(flag_shapes(rect, lang));
}

/// `lang`'s flag as shapes confined to `rect`. Simplified: the field, the main
/// charges, and nothing that would turn to mud at 21 px (no US stars beyond the
/// canton, no Brazilian banner text, no globe stars).
///
/// Pure geometry, so the result can be asserted in tests — a flag that paints
/// nothing, or paints outside its box, is caught without a screenshot.
fn flag_shapes(rect: Rect, lang: Language) -> Vec<Shape> {
    let mut out: Vec<Shape> = Vec::new();
    let (w, h) = (rect.width(), rect.height());
    let fill = |out: &mut Vec<Shape>, r: Rect, c: Color32| {
        out.push(Shape::rect_filled(r, 0.0, c));
    };
    match lang {
        // United States — 13 stripes and the blue canton.
        Language::English => {
            let red = Color32::from_rgb(0xB2, 0x22, 0x34);
            let blue = Color32::from_rgb(0x3C, 0x3B, 0x6E);
            fill(&mut out, rect, Color32::WHITE);
            let stripe = h / 13.0;
            for i in (0..13).step_by(2) {
                let top = rect.top() + stripe * i as f32;
                fill(
                    &mut out,
                    Rect::from_min_size(pos2(rect.left(), top), vec2(w, stripe)),
                    red,
                );
            }
            fill(
                &mut out,
                Rect::from_min_size(rect.min, vec2(w * 0.4, stripe * 7.0)),
                blue,
            );
        }
        // Spain — red / yellow / red, the middle band twice as tall.
        Language::Spanish => {
            fill(&mut out, rect, Color32::from_rgb(0xAA, 0x15, 0x1B));
            fill(
                &mut out,
                Rect::from_min_size(pos2(rect.left(), rect.top() + h * 0.25), vec2(w, h * 0.5)),
                Color32::from_rgb(0xF1, 0xBF, 0x00),
            );
        }
        // Brazil — green field, yellow rhombus, blue globe.
        Language::Portuguese => {
            fill(&mut out, rect, Color32::from_rgb(0x00, 0x9C, 0x3B));
            let c = rect.center();
            let (rx, ry) = (w * 0.40, h * 0.40);
            out.push(Shape::convex_polygon(
                vec![
                    pos2(c.x, c.y - ry),
                    pos2(c.x + rx, c.y),
                    pos2(c.x, c.y + ry),
                    pos2(c.x - rx, c.y),
                ],
                Color32::from_rgb(0xFF, 0xDF, 0x00),
                Stroke::NONE,
            ));
            out.push(Shape::circle_filled(
                c,
                h * 0.20,
                Color32::from_rgb(0x00, 0x27, 0x76),
            ));
        }
        // Japan — white field, red disc.
        Language::Japanese => {
            fill(&mut out, rect, Color32::WHITE);
            out.push(Shape::circle_filled(
                rect.center(),
                h * 0.30,
                Color32::from_rgb(0xBC, 0x00, 0x2D),
            ));
        }
        // China — red field, the large star and its four companions.
        Language::Chinese => {
            let yellow = Color32::from_rgb(0xFF, 0xDE, 0x00);
            fill(&mut out, rect, Color32::from_rgb(0xDE, 0x29, 0x10));
            out.push(star(
                pos2(rect.left() + w * 0.17, rect.top() + h * 0.30),
                h * 0.19,
                yellow,
            ));
            for (fx, fy) in [(0.36, 0.12), (0.46, 0.26), (0.46, 0.46), (0.36, 0.60)] {
                out.push(Shape::circle_filled(
                    pos2(rect.left() + w * fx, rect.top() + h * fy),
                    h * 0.05,
                    yellow,
                ));
            }
        }
        // France — blue / white / red verticals.
        Language::French => {
            fill(&mut out, rect, Color32::WHITE);
            fill(
                &mut out,
                Rect::from_min_size(rect.min, vec2(w / 3.0, h)),
                Color32::from_rgb(0x00, 0x55, 0xA4),
            );
            fill(
                &mut out,
                Rect::from_min_size(
                    pos2(rect.left() + w * 2.0 / 3.0, rect.top()),
                    vec2(w / 3.0, h),
                ),
                Color32::from_rgb(0xEF, 0x41, 0x35),
            );
        }
    }
    // A hairline keeps a white or light field readable on a light theme.
    out.push(Shape::rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, Color32::from_black_alpha(60)),
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

#[cfg(test)]
mod tests {
    use super::*;

    const BOX: Rect = Rect {
        min: pos2(10.0, 20.0),
        max: pos2(31.0, 34.0), // 21 × 14
    };

    /// A missing match arm would leave an empty box where a flag belongs.
    #[test]
    fn every_language_paints_a_field_and_a_charge() {
        for &lang in Language::ALL {
            let shapes = flag_shapes(BOX, lang);
            assert!(
                shapes.len() >= 3,
                "{lang:?} painted only {} shape(s) — field, charge and rim expected",
                shapes.len()
            );
        }
    }

    /// Every flag must stay inside its box: one that overflows would paint over
    /// the language name (or the toolbar next to it).
    #[test]
    fn nothing_is_painted_outside_the_box() {
        for &lang in Language::ALL {
            for shape in flag_shapes(BOX, lang) {
                let b = shape.visual_bounding_rect();
                assert!(
                    BOX.expand(1.5).contains_rect(b),
                    "{lang:?} paints {b:?} outside {BOX:?}"
                );
            }
        }
    }

    /// Two languages that render identically would make the selector useless.
    #[test]
    fn each_flag_is_distinct() {
        let renders: Vec<String> = Language::ALL
            .iter()
            .map(|&l| format!("{:?}", flag_shapes(BOX, l)))
            .collect();
        for (i, a) in renders.iter().enumerate() {
            for (j, b) in renders.iter().enumerate().skip(i + 1) {
                assert_ne!(
                    a,
                    b,
                    "{:?} and {:?} paint the same flag",
                    Language::ALL[i],
                    Language::ALL[j]
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

    /// Brazil is the flag that prompted this module: green field, yellow
    /// rhombus, blue globe — in that order.
    /// Brazil is the flag that prompted this module: green field, yellow
    /// rhombus, blue globe — in that order.
    #[test]
    fn brazil_is_a_green_field_a_yellow_rhombus_and_a_blue_globe() {
        let shapes = flag_shapes(BOX, Language::Portuguese);
        match &shapes[0] {
            Shape::Rect(r) => assert_eq!(r.fill, Color32::from_rgb(0x00, 0x9C, 0x3B)),
            other => panic!("expected the green field first, got {other:?}"),
        }
        match &shapes[1] {
            Shape::Path(p) => {
                assert_eq!(p.points.len(), 4, "the rhombus has four corners");
                assert_eq!(p.fill, Color32::from_rgb(0xFF, 0xDF, 0x00));
            }
            other => panic!("expected the yellow rhombus, got {other:?}"),
        }
        match &shapes[2] {
            Shape::Circle(c) => assert_eq!(c.fill, Color32::from_rgb(0x00, 0x27, 0x76)),
            other => panic!("expected the blue globe, got {other:?}"),
        }
    }
}
