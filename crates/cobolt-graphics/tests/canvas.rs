// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Canvas 2D (WASM-018 R7): the reference drawing against its golden image, every
//! command at work, banding, threads, read-back, blend modes, clipping and the
//! rules for arguments that HTML would ignore.

mod common;

use std::sync::Arc;

use cobolt_graphics::canvas::{ReadBack, Surface, SurfaceConfig};
use cobolt_graphics::display_list::{DisplayListBuilder, COMMANDS};
use cobolt_graphics::par::with_threads;
use cobolt_graphics::resources::{MapResources, NoResources};
use cobolt_graphics::{Limits, PixelBuffer, Rgba};
use common::{assert_golden, fingerprint, scenes, Tolerance};

fn surface(w: u32, h: u32) -> Surface {
    Surface::new(&SurfaceConfig::new(w, h), &Limits::default()).unwrap()
}

fn list(f: impl FnOnce(&mut DisplayListBuilder)) -> Vec<u8> {
    let mut b = DisplayListBuilder::new();
    f(&mut b);
    b.take().unwrap()
}

#[cfg(feature = "text")]
fn text_resources() -> MapResources {
    use cobolt_graphics::text::FontSet;
    let f = FontSet::new();
    f.add_font_data(common::synthfont::latin("SynthLatin")).unwrap();
    f.add_font_data(common::synthfont::cjk("SynthCJK")).unwrap();
    f.add_font_data(common::synthfont::hebrew("SynthHebrew")).unwrap();
    scenes::resources().with_text_engine(Arc::new(f))
}

#[cfg(not(feature = "text"))]
fn text_resources() -> MapResources {
    scenes::resources()
}

fn font_css() -> &'static str {
    if cfg!(feature = "text") {
        "14px SynthLatin"
    } else {
        ""
    }
}

fn reference(w: u32, h: u32, tile_rows: Option<u32>) -> (Surface, cobolt_graphics::canvas::Outcome) {
    let mut s = Surface::new(&SurfaceConfig { tile_rows, ..SurfaceConfig::new(w, h) }, &Limits::default()).unwrap();
    let bytes = list(|b| scenes::everything(b, w as f32, h as f32, font_css()));
    let out = s.execute(&bytes, &text_resources()).unwrap();
    (s, out)
}

#[test]
fn the_reference_scene_uses_every_command() {
    let bytes = list(|b| scenes::everything(b, 256.0, 256.0, "14px SynthLatin"));
    let l = cobolt_graphics::display_list::DisplayList::parse(&bytes, &Limits::default()).unwrap();
    let mut seen = vec![false; 256];
    for c in l.commands() {
        seen[c.unwrap().opcode() as usize] = true;
    }
    let missing: Vec<_> = COMMANDS.iter().filter(|c| !seen[c.opcode as usize]).map(|c| c.name).collect();
    assert!(missing.is_empty(), "the reference scene never uses: {missing:?}");
}

#[test]
fn the_reference_scene_matches_its_golden_image() {
    let (s, out) = reference(256, 256, None);
    assert!(out.presented);
    assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
    if let Ok(dir) = std::env::var("GRAPHICS_DUMP") {
        common::write_png(&std::path::Path::new(&dir).join("reference.png"), &s.to_image());
        // The same scene at three times the resolution, to look at the details.
        let mut big = Surface::new(&SurfaceConfig { pixel_ratio: 3.0, ..SurfaceConfig::new(256, 256) }, &Limits::default()).unwrap();
        let bytes = list(|b| scenes::everything(b, 256.0, 256.0, font_css()));
        big.execute(&bytes, &text_resources()).unwrap();
        common::write_png(&std::path::Path::new(&dir).join("reference3x.png"), &big.to_image());
    }
    let name = if cfg!(feature = "text") { "canvas-reference" } else { "canvas-reference-no-text" };
    assert_golden(name, &s.to_image(), Tolerance::EXACT);
}

#[test]
fn bands_give_the_same_picture_as_one_surface() {
    let (whole, _) = reference(256, 256, None);
    let want = whole.to_image();
    for rows in [8u32, 16, 40, 64, 100] {
        let (banded, _) = reference(256, 256, Some(rows));
        assert!(banded.band_count() > 1);
        let got = banded.to_image();
        let (mut worst, mut diff) = (0u8, 0usize);
        for (a, b) in want.as_bytes().chunks_exact(4).zip(got.as_bytes().chunks_exact(4)) {
            let d = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
            if d > 0 {
                diff += 1;
            }
            worst = worst.max(d);
        }
        // A band renders exactly the rows of the unbanded picture that it covers, except that the
        // rasteriser clips every edge that crosses a band boundary and re-derives its slope from
        // the clipped end points (26.6 fixed point), which moves a few edge pixels by one
        // coverage step (1/16, that is up to 16 levels). Gradients and patterns are evaluated
        // from a transform shifted by the band offset, which may move a colour by a level or two.
        assert!(worst <= 24, "bands of {rows} rows: a channel differs by {worst} ({diff} pixels)");
        assert!(diff * 200 < 256 * 256, "bands of {rows} rows: {diff} pixels differ (more than 0.5 %)");
    }
}

#[test]
fn rectilinear_drawing_is_bit_identical_in_bands() {
    // Edges that are horizontal or vertical never get re-derived by the clipper, so axis-aligned
    // geometry (fills, strokes, clips, images) is identical in every band.
    let draw = |b: &mut DisplayListBuilder| {
        b.set_fill_color(Rgba::rgb(10, 120, 200)).fill_rect(13.0, 7.0, 90.0, 150.0);
        b.set_stroke_color(Rgba::new(200, 30, 30, 180)).set_line_width(6.0).stroke_rect(40.0, 33.0, 160.0, 120.0);
        b.set_fill_color(Rgba::new(0, 0, 0, 90)).begin_path().rect(50.0, 20.0, 100.0, 140.0).clip(0).fill_rect(0.0, 0.0, 256.0, 256.0);
        b.draw_image(scenes::IMG_CHECKER, 70.0, 100.0);
    };
    let res = scenes::resources();
    let mut a = surface(256, 256);
    a.execute(&list(draw), &res).unwrap();
    for rows in [8u32, 24, 50] {
        let mut s = Surface::new(&SurfaceConfig { tile_rows: Some(rows), ..SurfaceConfig::new(256, 256) }, &Limits::default()).unwrap();
        s.execute(&list(draw), &res).unwrap();
        assert!(s.to_image().as_bytes() == a.to_image().as_bytes(), "bands of {rows} rows differ from the single surface");
    }
}

#[test]
fn threads_do_not_change_a_banded_picture() {
    let render = || reference(256, 256, Some(32)).0.to_image().into_bytes();
    let one = with_threads(1, render);
    let many = with_threads(8, render);
    assert_eq!(fingerprint(&one), fingerprint(&many));
    assert_eq!(one, many);
}

#[test]
fn a_large_canvas_is_banded_automatically() {
    let s = Surface::new(&SurfaceConfig::new(1920, 1080), &Limits::default()).unwrap();
    assert_eq!(s.band_count(), 1080usize.div_ceil(256));
    let s = Surface::new(&SurfaceConfig::new(800, 600), &Limits::default()).unwrap();
    assert_eq!(s.band_count(), 1);
}

#[test]
fn read_backs_answer_across_band_edges() {
    let mut s = Surface::new(&SurfaceConfig { tile_rows: Some(16), ..SurfaceConfig::new(40, 64) }, &Limits::default()).unwrap();
    let out = s
        .execute(
            &list(|b| {
                b.set_fill_color(Rgba::rgb(1, 2, 3)).fill_rect(0.0, 10.0, 40.0, 30.0);
                b.read_pixel(1, 5.0, 9.0).read_pixel(2, 5.0, 10.0).read_pixel(3, 5.0, 39.0).read_pixel(4, 5.0, 40.0);
                b.capture(9, 3.0, 8.0, 6.0, 36.0);
                b.begin_path().rect(2.0, 2.0, 10.0, 50.0).hit_path(5, 5.0, 50.0, 0).hit_path(6, 20.0, 50.0, 0).hit_stroke(7, 2.0, 30.0);
            }),
            &NoResources,
        )
        .unwrap();
    let px = |i: usize| match &out.readbacks[i] {
        ReadBack::Pixel { color, .. } => *color,
        other => panic!("{other:?}"),
    };
    assert_eq!(px(0), Some(Rgba::TRANSPARENT));
    assert_eq!(px(1), Some(Rgba::rgb(1, 2, 3)));
    assert_eq!(px(2), Some(Rgba::rgb(1, 2, 3)));
    assert_eq!(px(3), Some(Rgba::TRANSPARENT));
    let ReadBack::Image { image: Some(img), slot } = &out.readbacks[4] else { panic!("{:?}", out.readbacks[4]) };
    assert_eq!(*slot, 9);
    assert_eq!((img.width(), img.height()), (6, 36));
    for y in 0..36 {
        let want = if (2..32).contains(&y) { Rgba::rgb(1, 2, 3) } else { Rgba::TRANSPARENT };
        assert_eq!(img.get(0, y), Some(want), "row {y}");
    }
    assert_eq!(out.readbacks[5], ReadBack::Hit { slot: 5, inside: true });
    assert_eq!(out.readbacks[6], ReadBack::Hit { slot: 6, inside: false });
    assert_eq!(out.readbacks[7], ReadBack::Hit { slot: 7, inside: true });
}

#[test]
fn composite_operations_that_clear_outside_the_shape_do_it() {
    let mut s = surface(30, 30);
    s.execute(&list(|b| {
        b.set_fill_color(Rgba::rgb(255, 0, 0)).fill_rect(0.0, 0.0, 30.0, 30.0);
        // copy: the source replaces the destination everywhere, so outside the rectangle it is transparent.
        b.set_blend_mode(9).set_fill_color(Rgba::rgb(0, 0, 255)).fill_rect(10.0, 10.0, 10.0, 10.0);
    }), &NoResources).unwrap();
    let img = s.to_image();
    assert_eq!(img.get(15, 15), Some(Rgba::rgb(0, 0, 255)));
    assert_eq!(img.get(2, 2), Some(Rgba::TRANSPARENT), "copy clears what the shape does not cover");

    let mut s = surface(30, 30);
    s.execute(&list(|b| {
        b.set_fill_color(Rgba::rgb(255, 0, 0)).fill_rect(0.0, 0.0, 30.0, 30.0);
        // destination-over only paints behind: nothing changes where the red is opaque.
        b.set_blend_mode(4).set_fill_color(Rgba::rgb(0, 0, 255)).fill_rect(10.0, 10.0, 10.0, 10.0);
    }), &NoResources).unwrap();
    assert_eq!(s.to_image().get(15, 15), Some(Rgba::rgb(255, 0, 0)));

    // source-in keeps only the overlap.
    let mut s = surface(30, 30);
    s.execute(&list(|b| {
        b.set_fill_color(Rgba::rgb(255, 0, 0)).fill_rect(0.0, 0.0, 20.0, 20.0);
        b.set_blend_mode(1).set_fill_color(Rgba::rgb(0, 255, 0)).fill_rect(10.0, 10.0, 20.0, 20.0);
    }), &NoResources).unwrap();
    let img = s.to_image();
    assert_eq!(img.get(15, 15), Some(Rgba::rgb(0, 255, 0)));
    assert_eq!(img.get(5, 5), Some(Rgba::TRANSPARENT));
    assert_eq!(img.get(25, 25), Some(Rgba::TRANSPARENT), "outside the destination nothing remains");
}

#[test]
fn multiply_and_screen_blend() {
    let mut s = surface(10, 10);
    s.execute(&list(|b| {
        b.set_fill_color(Rgba::rgb(200, 100, 50)).fill_rect(0.0, 0.0, 10.0, 10.0);
        b.set_blend_mode(11).set_fill_color(Rgba::rgb(128, 128, 128)).fill_rect(0.0, 0.0, 5.0, 10.0);
        b.set_blend_mode(12).fill_rect(5.0, 0.0, 5.0, 10.0);
    }), &NoResources).unwrap();
    let img = s.to_image();
    let m = img.get(2, 5).unwrap();
    assert!((m.r as i32 - 100).abs() <= 1 && (m.g as i32 - 50).abs() <= 1 && (m.b as i32 - 25).abs() <= 1, "{m:?}");
    let sc = img.get(7, 5).unwrap();
    assert!((sc.r as i32 - 228).abs() <= 1, "{sc:?}");
}

#[test]
fn clipping_restricts_drawing_and_restore_lifts_it() {
    let mut s = surface(40, 40);
    s.execute(&list(|b| {
        b.save().begin_path().rect(10.0, 10.0, 10.0, 10.0).clip(0);
        b.set_fill_color(Rgba::BLACK).fill_rect(0.0, 0.0, 40.0, 40.0);
        b.restore();
        b.set_fill_color(Rgba::rgb(255, 0, 0)).fill_rect(30.0, 30.0, 5.0, 5.0);
    }), &NoResources).unwrap();
    let img = s.to_image();
    assert_eq!(img.get(15, 15), Some(Rgba::BLACK));
    assert_eq!(img.get(5, 5), Some(Rgba::TRANSPARENT));
    assert_eq!(img.get(32, 32), Some(Rgba::rgb(255, 0, 0)));
    // Two clips intersect.
    let mut s = surface(40, 40);
    s.execute(&list(|b| {
        b.begin_path().rect(0.0, 0.0, 20.0, 40.0).clip(0).begin_path().rect(10.0, 0.0, 20.0, 40.0).clip(0);
        b.set_fill_color(Rgba::BLACK).fill_rect(0.0, 0.0, 40.0, 40.0);
    }), &NoResources).unwrap();
    let img = s.to_image();
    assert_eq!(img.get(15, 5), Some(Rgba::BLACK));
    assert_eq!(img.get(5, 5), Some(Rgba::TRANSPARENT));
    assert_eq!(img.get(25, 5), Some(Rgba::TRANSPARENT));
}

#[test]
fn a_shadow_appears_offset_and_blurred() {
    let mut s = surface(60, 60);
    s.execute(&list(|b| {
        b.set_shadow(Rgba::new(0, 0, 0, 255), 0.0, 10.0, 10.0).set_fill_color(Rgba::rgb(255, 0, 0)).fill_rect(10.0, 10.0, 10.0, 10.0);
    }), &NoResources).unwrap();
    let img = s.to_image();
    assert_eq!(img.get(15, 15), Some(Rgba::rgb(255, 0, 0)), "the shape is on top");
    assert_eq!(img.get(25, 25), Some(Rgba::BLACK), "the unblurred shadow is the shape moved by the offset");
    assert_eq!(img.get(35, 35), Some(Rgba::TRANSPARENT));

    let mut s = surface(80, 80);
    s.execute(&list(|b| {
        b.set_shadow(Rgba::new(0, 0, 0, 255), 12.0, 0.0, 0.0).set_fill_color(Rgba::WHITE).fill_rect(30.0, 30.0, 20.0, 20.0);
    }), &NoResources).unwrap();
    let img = s.to_image();
    let edge = img.get(28, 40).unwrap();
    let far = img.get(10, 40).unwrap();
    assert!(edge.a > 20 && edge.a < 255, "the shadow fades out from the edge: {edge:?}");
    assert!(far.a < edge.a);
    assert_eq!(img.get(40, 40), Some(Rgba::WHITE));
    // Pixel centres: x = 19 is 10.5 from the left edge (30), x = 60 is 10.5 from the right edge (50).
    assert_eq!(img.get(19, 40).unwrap().a, img.get(60, 40).unwrap().a, "a blur without offset is symmetric");
}

#[test]
fn images_drawn_at_whole_pixels_are_exact_copies() {
    let mut s = surface(40, 40);
    let res = scenes::resources();
    s.execute(&list(|b| {
        b.draw_image(scenes::IMG_CHECKER, 5.0, 6.0);
    }), &res).unwrap();
    let img = s.to_image();
    let src = scenes::checker();
    for y in 0..16 {
        for x in 0..16 {
            assert_eq!(img.get(5 + x, 6 + y), src.get(x, y), "pixel ({x},{y})");
        }
    }
    assert_eq!(img.get(4, 6), Some(Rgba::TRANSPARENT));
    assert_eq!(img.get(21, 6), Some(Rgba::TRANSPARENT));
}

#[test]
fn a_sprite_frame_shows_only_that_frame() {
    let mut s = surface(40, 40);
    let res = scenes::resources();
    s.execute(&list(|b| {
        b.draw_sprite(scenes::IMG_SPRITES, 5, 16.0, 16.0, 10.0, 10.0, 16.0, 16.0);
    }), &res).unwrap();
    let img = s.to_image();
    let sheet = scenes::sprites();
    // Frame 5 is column 1, row 1 of the 4-column sheet.
    for y in 0..16 {
        for x in 0..16 {
            assert_eq!(img.get(10 + x, 10 + y), sheet.get(16 + x, 16 + y), "({x},{y})");
        }
    }
}

#[test]
fn a_missing_image_is_a_note_not_a_failure() {
    let mut s = surface(10, 10);
    let out = s.execute(&list(|b| {
        b.draw_image(99, 0.0, 0.0).set_fill_color(Rgba::BLACK).fill_rect(0.0, 0.0, 2.0, 2.0);
    }), &NoResources).unwrap();
    assert_eq!(out.diagnostics.len(), 1);
    assert!(out.diagnostics[0].contains("99"), "{:?}", out.diagnostics);
    assert_eq!(s.to_image().get(0, 0), Some(Rgba::BLACK), "the rest of the list ran");
}

#[test]
fn arguments_html_would_ignore_are_ignored() {
    let base = list(|b| {
        b.set_fill_color(Rgba::rgb(9, 9, 9)).fill_rect(0.0, 0.0, 20.0, 20.0);
    });
    let mut a = surface(20, 20);
    a.execute(&base, &NoResources).unwrap();
    let mut s = surface(20, 20);
    s.execute(&list(|b| {
        b.set_fill_color(Rgba::rgb(9, 9, 9)).fill_rect(0.0, 0.0, 20.0, 20.0);
        // None of these may change a pixel.
        b.fill_rect(f32::NAN, 0.0, 5.0, 5.0).fill_rect(0.0, 0.0, f32::INFINITY, 5.0).set_fill_color(Rgba::rgb(9, 9, 9));
        b.begin_path().arc(5.0, 5.0, -3.0, 0.0, 1.0, 0).fill(0).begin_path().move_to(f32::NAN, 1.0).line_to(5.0, 5.0).stroke();
        b.set_line_width(-4.0).set_line_width(f32::NAN).set_alpha(7.0).set_alpha(-1.0).set_blend_mode(200).set_line_cap(9).set_line_join(9);
        b.set_dash(&[-1.0, 2.0], 0.0).set_transform(f32::NAN, 0.0, 0.0, 1.0, 0.0, 0.0).translate(f32::INFINITY, 0.0);
        b.set_font("not a font").set_shadow(Rgba::BLACK, -3.0, 0.0, 0.0);
        b.draw_text("x", 1.0, 1.0, 0.0, 1);
    }), &NoResources).unwrap();
    assert_eq!(s.to_image().as_bytes(), a.to_image().as_bytes());
}

#[test]
fn the_save_stack_is_limited_and_restore_underflow_is_harmless() {
    let mut s = surface(10, 10);
    s.execute(&list(|b| {
        b.restore().restore().set_fill_color(Rgba::BLACK).fill_rect(0.0, 0.0, 1.0, 1.0);
    }), &NoResources).unwrap();
    assert_eq!(s.to_image().get(0, 0), Some(Rgba::BLACK));
    let lim = Limits { max_save_depth: 3, ..Limits::default() };
    let mut s = Surface::new(&SurfaceConfig::new(10, 10), &lim).unwrap();
    let e = s.execute(&list(|b| {
        b.save().save().save().save();
    }), &NoResources).unwrap_err();
    assert!(e.to_string().contains("max_save_depth"), "{e}");
}

#[test]
fn path_size_is_limited() {
    let lim = Limits { max_path_points: 10, ..Limits::default() };
    let mut s = Surface::new(&SurfaceConfig::new(10, 10), &lim).unwrap();
    let e = s.execute(&list(|b| {
        b.begin_path().move_to(0.0, 0.0);
        for i in 0..20 {
            b.line_to(i as f32, 3.0);
        }
    }), &NoResources).unwrap_err();
    assert!(e.to_string().contains("max_path_points"), "{e}");
}

#[test]
fn state_persists_between_executions() {
    let mut s = surface(10, 10);
    s.execute(&list(|b| {
        b.set_fill_color(Rgba::rgb(7, 8, 9)).translate(2.0, 2.0);
    }), &NoResources).unwrap();
    s.execute(&list(|b| {
        b.fill_rect(0.0, 0.0, 3.0, 3.0);
    }), &NoResources).unwrap();
    let img = s.to_image();
    assert_eq!(img.get(2, 2), Some(Rgba::rgb(7, 8, 9)));
    assert_eq!(img.get(1, 1), Some(Rgba::TRANSPARENT));
}

#[test]
fn the_premultiplied_and_straight_exports_agree() {
    let mut s = surface(8, 8);
    s.execute(&list(|b| {
        b.set_fill_color(Rgba::new(200, 100, 50, 128)).fill_rect(0.0, 0.0, 8.0, 8.0);
    }), &NoResources).unwrap();
    let mut pre = vec![0u8; 8 * 8 * 4];
    s.write_rgba_premultiplied(&mut pre);
    let mut straight = vec![0u8; 8 * 8 * 4];
    s.write_rgba(&mut straight);
    // Straight alpha is recovered from the premultiplied pixels, so a channel may be one level off.
    for (got, want) in straight[..4].iter().zip([200u8, 100, 50, 128]) {
        assert!(got.abs_diff(want) <= 1, "{:?}", &straight[..4]);
    }
    assert!(pre[0] < straight[0] && pre[3] == 128);
}

#[test]
fn a_pixel_ratio_of_two_draws_the_same_picture_at_twice_the_size() {
    let draw = |b: &mut DisplayListBuilder| {
        b.set_fill_color(Rgba::rgb(0, 0, 255)).begin_path().arc(20.0, 20.0, 10.0, 0.0, 6.2831855, 0).fill(0);
    };
    let mut s1 = surface(40, 40);
    s1.execute(&list(draw), &NoResources).unwrap();
    let mut s2 = Surface::new(&SurfaceConfig { pixel_ratio: 2.0, ..SurfaceConfig::new(40, 40) }, &Limits::default()).unwrap();
    s2.execute(&list(draw), &NoResources).unwrap();
    assert_eq!(s2.device_size(), (80, 80));
    // Down-sample the 2x picture by averaging 2x2 and compare with the 1x picture.
    let a = s1.to_image();
    let b = s2.to_image();
    let mut worst = 0i32;
    for y in 0..40i64 {
        for x in 0..40i64 {
            let mut acc = [0i32; 4];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let p = b.get(x * 2 + dx, y * 2 + dy).unwrap();
                acc[0] += p.r as i32;
                acc[1] += p.g as i32;
                acc[2] += p.b as i32;
                acc[3] += p.a as i32;
            }
            let q = a.get(x, y).unwrap();
            worst = worst.max((acc[3] / 4 - q.a as i32).abs());
        }
    }
    assert!(worst <= 24, "alpha of the 2x picture averaged down differs from the 1x picture by {worst}");
}

#[cfg(feature = "text")]
mod text {
    use super::*;

    /// Ink bounds of a picture (alpha above `min`).
    fn ink(img: &PixelBuffer, min: u8) -> Option<(i64, i64, i64, i64)> {
        let (mut x0, mut y0, mut x1, mut y1) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
        for y in 0..img.height() as i64 {
            for x in 0..img.width() as i64 {
                if img.get(x, y).unwrap().a > min {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        (x0 != i64::MAX).then_some((x0, y0, x1 + 1, y1 + 1))
    }

    fn metrics_of(font: &str, text: &str, align: u8, baseline: u8) -> cobolt_graphics::textapi::TextMetrics {
        let mut s = surface(10, 10);
        let out = s
            .execute(
                &list(|b| {
                    b.set_font(font).set_text_align(align).set_text_baseline(baseline).measure_text(0, text);
                }),
                &text_resources(),
            )
            .unwrap();
        match &out.readbacks[0] {
            ReadBack::Text { metrics, .. } => metrics.clone(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn measure_text_equals_what_draw_text_occupies() {
        // Every box of the synthetic fonts spans x from 50 to advance-50 of 1000 units, so the
        // ink of "Hi" at 100px starts at 5px and ends 5px before the advance.
        for (text, font) in [("Hi", "100px SynthLatin"), ("日本語", "50px SynthCJK"), ("abc אבג def", "40px SynthLatin"), ("Hello, 日本!", "33px SynthLatin")] {
            for align in [2u8, 3, 4, 0, 1] {
                for baseline in [0u8, 1, 2, 3, 4, 5] {
                    let m = metrics_of(font, text, align, baseline);
                    assert!(m.width > 0.0, "{text}");
                    let mut s = surface(900, 400);
                    s.execute(&list(|b| {
                        b.set_font(font).set_text_align(align).set_text_baseline(baseline).set_fill_color(Rgba::BLACK).draw_text(text, 450.0, 200.0, 0.0, 1);
                    }), &text_resources()).unwrap();
                    let (x0, y0, x1, y1) = ink(&s.to_image(), 0).expect("something was drawn");
                    let tol = 1.5;
                    assert!((450.0 - m.actual_left - x0 as f32).abs() <= tol, "{text} align {align}: left {} vs ink {x0}", 450.0 - m.actual_left);
                    assert!((450.0 + m.actual_right - x1 as f32).abs() <= tol, "{text} align {align}: right {} vs ink {x1}", 450.0 + m.actual_right);
                    assert!((200.0 - m.actual_ascent - y0 as f32).abs() <= tol, "{text} baseline {baseline}: top {} vs ink {y0}", 200.0 - m.actual_ascent);
                    assert!((200.0 + m.actual_descent - y1 as f32).abs() <= tol, "{text} baseline {baseline}: bottom {} vs ink {y1}", 200.0 + m.actual_descent);
                }
            }
        }
    }

    #[test]
    fn alignment_moves_the_text_by_its_width() {
        let w = metrics_of("20px SynthLatin", "abcd", 2, 0).width;
        assert!((w - 48.0).abs() < 1e-3);
        let draw = |align: u8| {
            let mut s = surface(200, 40);
            s.execute(&list(|b| {
                b.set_font("20px SynthLatin").set_text_align(align).set_text_baseline(0).set_fill_color(Rgba::BLACK).draw_text("abcd", 100.0, 30.0, 0.0, 1);
            }), &text_resources()).unwrap();
            ink(&s.to_image(), 0).unwrap()
        };
        let (l0, _, _, _) = draw(2);
        let (c0, _, _, _) = draw(4);
        let (r0, _, _, _) = draw(3);
        assert!((l0 - 101).abs() <= 1, "left aligned text starts at the anchor plus the glyph's side bearing: {l0}");
        assert!(((c0 - l0) as f32 + w / 2.0).abs() <= 1.5);
        assert!(((r0 - l0) as f32 + w).abs() <= 1.5);
    }

    #[test]
    fn a_text_box_wraps_where_the_layout_says() {
        let mut s = surface(200, 100);
        s.execute(&list(|b| {
            b.set_font("10px SynthLatin").set_fill_color(Rgba::BLACK).draw_text_box("aaa bbb ccc ddd", 0.0, 12.0, 40.0, 14.0, 1);
        }), &text_resources()).unwrap();
        let img = s.to_image();
        // Two lines: ink rows around y=5..12 and y=19..26.
        let first = ink(&img.crop(0, 0, 200, 16).unwrap(), 0).unwrap();
        let second = ink(&img.crop(0, 16, 200, 40).unwrap(), 0).unwrap();
        assert!(first.2 > 30 && second.2 > 30, "both lines have two words: {first:?} {second:?}");
        assert!(first.2 <= 41 && second.2 <= 41, "within the box width");
    }

    #[test]
    fn without_a_text_engine_text_is_skipped_with_a_note() {
        let mut s = surface(10, 10);
        let out = s.execute(&list(|b| {
            b.set_font("10px SynthLatin").draw_text("x", 1.0, 8.0, 0.0, 1);
        }), &NoResources).unwrap();
        assert_eq!(out.diagnostics.len(), 1);
        assert!(out.diagnostics[0].contains("fonts"), "{:?}", out.diagnostics);
    }
}
