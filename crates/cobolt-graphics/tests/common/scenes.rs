// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Reference drawings used by the golden-image, banding and determinism tests.
#![allow(dead_code)]

use cobolt_graphics::display_list::DisplayListBuilder;
use cobolt_graphics::resources::MapResources;
use cobolt_graphics::{PixelBuffer, Rgba};

use std::f32::consts::PI;

/// Image ids used by the scene.
pub const IMG_CHECKER: u32 = 1;
pub const IMG_SPRITES: u32 = 2;

/// A 16 x 16 checkerboard of two colours.
pub fn checker() -> PixelBuffer {
    let mut b = PixelBuffer::new(16, 16).unwrap();
    for y in 0..16 {
        for x in 0..16 {
            let on = ((x / 4) + (y / 4)) % 2 == 0;
            b.set(x, y, if on { Rgba::rgb(240, 200, 40) } else { Rgba::rgb(30, 60, 160) });
        }
    }
    b
}

/// A 64 x 32 sprite sheet of eight 16 x 16 frames, each a different colour with a white dot.
pub fn sprites() -> PixelBuffer {
    let mut b = PixelBuffer::new(64, 32).unwrap();
    for f in 0..8 {
        let (fx, fy) = ((f % 4) * 16, (f / 4) * 16);
        let c = Rgba::rgb((f * 30 + 20) as u8, (255 - f * 28) as u8, (f * 17 + 80) as u8);
        for y in 0..16 {
            for x in 0..16 {
                b.set(fx + x, fy + y, c);
            }
        }
        b.set(fx + 4, fy + 4, Rgba::WHITE);
        b.set(fx + 5, fy + 4, Rgba::WHITE);
    }
    b
}

pub fn resources() -> MapResources {
    MapResources::new().with_image(IMG_CHECKER, checker()).with_image(IMG_SPRITES, sprites())
}

/// A scene that uses every drawing command of the display list at least once.
/// `font` is a CSS font that the caller's text engine can draw ("" to skip text).
pub fn everything(b: &mut DisplayListBuilder, w: f32, h: f32, font: &str) {
    let blue = Rgba::rgb(40, 90, 200);
    let red = Rgba::rgb(220, 50, 40);
    let green = Rgba::rgb(40, 170, 90);
    let stops = [(0.0, Rgba::rgb(255, 255, 255)), (0.5, Rgba::rgb(120, 180, 255)), (1.0, Rgba::rgb(20, 40, 90))];

    // -- background: a linear gradient fill of the whole canvas
    b.reset().begin_path().clear(Rgba::rgb(20, 20, 30));
    b.set_linear_gradient(0, 0.0, 0.0, 0.0, h, &stops).fill_rect(0.0, 0.0, w, h);

    // -- paths: a curvy closed shape, filled non-zero, with a stroke
    b.save();
    b.set_fill_color(Rgba::new(255, 200, 0, 200)).set_stroke_color(Rgba::rgb(60, 20, 0)).set_line_width(3.0);
    b.begin_path().move_to(20.0, 90.0).line_to(60.0, 40.0).quad_to(100.0, 10.0, 130.0, 50.0).cubic_to(150.0, 80.0, 110.0, 120.0, 70.0, 100.0).close_path();
    b.fill(0).stroke();
    b.restore();

    // -- arcs, ellipse, arcTo, rect, roundRect, even-odd
    b.save();
    b.set_fill_color(Rgba::new(255, 255, 255, 160)).set_stroke_color(red).set_line_width(2.0);
    b.begin_path().arc(w * 0.75, 50.0, 28.0, 0.0, 1.5 * PI, 0).arc(w * 0.75, 50.0, 12.0, 0.0, 2.0 * PI, 1).fill(1);
    b.begin_path().ellipse(w * 0.75, 120.0, 30.0, 14.0, 0.5, 0.0, 2.0 * PI, 0).stroke();
    b.begin_path().move_to(10.0, 150.0).arc_to(60.0, 150.0, 60.0, 190.0, 15.0).line_to(60.0, 190.0).stroke();
    b.begin_path().rect(80.0, 140.0, 40.0, 30.0).round_rect(130.0, 140.0, 40.0, 30.0, 12.0, 4.0, 12.0, 4.0).fill(0);
    b.restore();

    // -- shapes
    b.save();
    b.set_fill_color(green).set_stroke_color(Rgba::BLACK).set_line_width(1.0);
    b.fill_rect(180.0, 140.0, 24.0, 20.0).stroke_rect(210.0, 140.0, 24.0, 20.0);
    b.draw_line(180.0, 170.0, 236.0, 190.0);
    b.draw_circle(150.0, 200.0, 12.0, 3);
    b.draw_polygon(&[(190.0, 200.0), (210.0, 180.0), (230.0, 200.0), (210.0, 215.0)], 7);
    b.restore();

    // -- dash, caps, joins
    b.save();
    b.set_stroke_color(blue).set_line_width(6.0).set_line_cap(1).set_line_join(1).set_miter_limit(2.0).set_dash(&[10.0, 8.0], 3.0);
    b.begin_path().move_to(20.0, 210.0).line_to(70.0, 230.0).line_to(120.0, 205.0).stroke();
    b.set_dash(&[], 0.0).set_line_cap(2).set_line_join(2).begin_path().move_to(20.0, 240.0).line_to(60.0, 250.0).line_to(100.0, 238.0).stroke();
    b.restore();

    // -- gradients: radial, conic; pattern; shadow
    b.save();
    b.set_radial_gradient(0, 40.0, 30.0, 4.0, 60.0, 40.0, 30.0, &[(0.0, Rgba::WHITE), (1.0, red)]).fill_rect(10.0, 8.0, 90.0, 60.0);
    b.set_conic_gradient(0, 160.0, 40.0, 0.3, &[(0.0, red), (0.33, green), (0.66, blue), (1.0, red)]).fill_rect(125.0, 8.0, 70.0, 60.0);
    b.set_pattern(0, IMG_CHECKER, 0).fill_rect(w - 70.0, h - 70.0, 60.0, 60.0);
    b.set_pattern(0, IMG_CHECKER, 3).fill_rect(w - 130.0, h - 40.0, 20.0, 20.0);
    b.set_pattern(1, IMG_CHECKER, 1).set_line_width(10.0).stroke_rect(w - 140.0, h - 120.0, 40.0, 30.0);
    b.set_shadow(Rgba::new(0, 0, 0, 160), 6.0, 4.0, 4.0).set_fill_color(Rgba::WHITE).fill_rect(w - 120.0, 90.0, 40.0, 30.0);
    b.set_shadow(Rgba::TRANSPARENT, 0.0, 0.0, 0.0);
    b.restore();

    // -- transforms
    b.save();
    b.translate(110.0, 210.0).rotate(0.4).scale(1.0, 0.6).set_fill_color(Rgba::new(120, 0, 160, 220)).fill_rect(-25.0, -15.0, 50.0, 30.0);
    b.set_transform(1.0, 0.2, 0.0, 1.0, 0.0, 0.0).set_fill_color(Rgba::new(0, 0, 0, 90)).fill_rect(150.0, 100.0, 30.0, 20.0);
    b.transform(1.0, 0.0, 0.0, 1.0, 5.0, 5.0).reset_transform();
    b.restore();

    // -- clip, alpha, blend modes
    b.save();
    b.begin_path().arc(w * 0.5, h * 0.5, 36.0, 0.0, 2.0 * PI, 0).clip(0);
    b.set_alpha(0.7).set_fill_color(Rgba::rgb(255, 80, 120)).fill_rect(w * 0.5 - 40.0, h * 0.5 - 40.0, 80.0, 40.0);
    b.set_blend_mode(11).set_fill_color(Rgba::rgb(60, 200, 255)).fill_rect(w * 0.5 - 10.0, h * 0.5 - 20.0, 70.0, 60.0);
    b.set_blend_mode(12).set_fill_color(Rgba::rgb(90, 20, 90)).fill_rect(w * 0.5 - 30.0, h * 0.5 + 5.0, 50.0, 30.0);
    b.restore();
    // an unbounded operation clears outside the shape: draw into a corner with `source-in`
    b.save();
    b.begin_path().rect(0.0, h - 20.0, 12.0, 12.0).clip(0);
    b.set_blend_mode(1).set_fill_color(Rgba::rgb(0, 255, 0)).fill_rect(2.0, h - 18.0, 4.0, 4.0);
    b.restore();
    b.save();
    b.set_blend_mode(6).fill_rect(w - 6.0, 0.0, 6.0, 6.0);
    b.clear_rect(0.0, 0.0, 6.0, 6.0);
    b.restore();

    // -- images
    b.save();
    b.set_smoothing(0).draw_image(IMG_CHECKER, 8.0, h - 28.0);
    b.set_smoothing(2).draw_image_scaled(IMG_CHECKER, 30.0, h - 36.0, 32.0, 32.0);
    b.draw_image_region(IMG_SPRITES, 16.0, 0.0, 16.0, 16.0, 70.0, h - 36.0, 24.0, 24.0);
    b.draw_sprite(IMG_SPRITES, 5, 16.0, 16.0, 100.0, h - 36.0, 24.0, 24.0);
    b.set_alpha(0.5).draw_sprite(IMG_SPRITES, 2, 16.0, 16.0, 108.0, h - 28.0, 24.0, 24.0);
    b.restore();

    // -- text
    if !font.is_empty() {
        b.save();
        b.set_font(font).set_text_align(4).set_text_baseline(3).set_fill_color(Rgba::WHITE).set_stroke_color(Rgba::BLACK).set_line_width(1.0);
        b.draw_text("Hello 日本 אבג", w * 0.5, 20.0, 0.0, 1);
        b.set_text_align(2).set_text_baseline(0).set_text_direction(1).draw_text("wide text squeezed", 10.0, 78.0, 60.0, 3);
        b.set_text_align(3).draw_text("right", w - 8.0, 78.0, 0.0, 1);
        b.set_text_align(0).set_text_direction(0).draw_text_box("a text box that wraps onto several lines", 10.0, 100.0, 70.0, 12.0, 1);
        b.restore();
    }

    // -- read-back commands do not draw
    b.read_pixel(1, 5.0, 5.0).capture(2, 0.0, 0.0, 8.0, 8.0);
    b.begin_path().rect(10.0, 10.0, 20.0, 20.0).hit_path(3, 15.0, 15.0, 0).hit_stroke(4, 10.0, 15.0);
    if !font.is_empty() {
        b.measure_text(5, "Hello");
    }
    b.present();
}
