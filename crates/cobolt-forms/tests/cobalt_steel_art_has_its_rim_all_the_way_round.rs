// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Cobalt Steel pack's rounded art keeps its rim all the way round.
//!
//! `gen_reference_theme` measured the rim from the nearest STRAIGHT edge, which is
//! far from the outline on a corner's diagonal: the rim ran along the sides, stopped
//! where each arc began, and left the fill chamfered with no edge at all. Every card
//! of the pack showed it — the Help card of the Layers demo, in a screenshot.
//!
//! This reads the PNGs the pack SHIPS (not the generator), because an old copy
//! checked in again would pass any test of the generator alone.

#![cfg(feature = "render")]

use image::RgbaImage;

/// The art made by `rounded_tile`, with its rim colour.
const ROUNDED_ART: &[&str] =
    &["panel.png", "groupbox.png", "button.png", "button_hover.png", "button_pressed.png", "textbox.png", "combobox.png", "listbox.png"];

fn art(name: &str) -> RgbaImage {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/themes/cobalt-steel");
    image::open(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}")).to_rgba8()
}

/// Whether the pixel at `(x, y)` is part of the shape: any x or y outside the tile is not.
fn solid(img: &RgbaImage, x: i64, y: i64) -> bool {
    x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() && img.get_pixel(x as u32, y as u32)[3] >= 128
}

#[test]
fn every_pixel_on_the_outline_of_the_art_is_rim_arcs_included() {
    let (mut outline, mut files) = (0usize, 0usize);
    let mut bad = Vec::new();
    for name in ROUNDED_ART {
        let img = art(name);
        // The rim's colour is what the middle of the top edge shows: there is no arc there.
        let rim = img.get_pixel(img.width() / 2, 0).0;
        for y in 0..img.height() as i64 {
            for x in 0..img.width() as i64 {
                if !solid(&img, x, y) {
                    continue;
                }
                // On the outline: solid, with a neighbour that is not.
                let edge = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| !solid(&img, x + dx, y + dy));
                if !edge {
                    continue;
                }
                outline += 1;
                let p = img.get_pixel(x as u32, y as u32).0;
                let off = (0..3).map(|i| (i32::from(p[i]) - i32::from(rim[i])).abs()).max().unwrap_or(0);
                if off > 8 {
                    bad.push(format!("{name} ({x},{y}) is {:?} on the outline, the rim is {:?}", &p[..3], &rim[..3]));
                }
            }
        }
        files += 1;
    }
    for b in bad.iter().take(12) {
        println!("{b}");
    }
    println!("{files} pieces of art, {outline} outline pixels checked, {} not rim", bad.len());
    assert!(bad.is_empty(), "{} outline pixels are fill, not rim — the corners are broken", bad.len());
}

#[test]
fn the_four_corners_of_each_piece_of_art_are_mirrors_of_each_other() {
    let mut checked = 0usize;
    for name in ROUNDED_ART {
        let img = art(name);
        let (w, h) = (img.width(), img.height());
        for y in 0..h {
            for x in 0..w {
                let a = img.get_pixel(x, y)[3];
                for (mx, my) in [(w - 1 - x, y), (x, h - 1 - y), (w - 1 - x, h - 1 - y)] {
                    let b = img.get_pixel(mx, my)[3];
                    assert!(a.abs_diff(b) <= 1, "{name}: ({x},{y}) alpha {a} but its mirror ({mx},{my}) is {b}");
                    checked += 1;
                }
            }
        }
    }
    println!("{} mirror pairs of the art's alpha checked", checked);
}

#[test]
fn the_outline_of_the_art_is_anti_aliased_on_every_arc() {
    for name in ROUNDED_ART {
        let img = art(name);
        let partial = |x0: u32, y0: u32| {
            let r = img.width().min(img.height()) / 3;
            (y0..y0 + r).flat_map(|y| (x0..x0 + r).map(move |x| (x, y))).filter(|&(x, y)| {
                let a = img.get_pixel(x, y)[3];
                // Not fully transparent and not at the shape's own alpha: a pixel the arc only half covers.
                a > 0 && a < img.get_pixel(img.width() / 2, 0)[3].saturating_sub(8)
            })
            .count()
        };
        let (w, h) = (img.width(), img.height());
        let r = w.min(h) / 3;
        for (corner, x0, y0) in [("top-left", 0, 0), ("top-right", w - r, 0), ("bottom-left", 0, h - r), ("bottom-right", w - r, h - r)] {
            assert!(partial(x0, y0) > 0, "{name}: the {corner} arc has no half-covered pixel — it is cut hard, not anti-aliased");
        }
    }
}
