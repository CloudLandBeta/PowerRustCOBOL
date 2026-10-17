// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The numbers the design guidance gives an agent, measured on the paint** —
//! the shadow bands, and the size text is drawn at when nothing sets one.
//!
//! The `powerrustcobol-layout-and-themes` skill tells an agent how strong a
//! shadow on Spatial's glass may be — `ShadowOpacity` 4 to 8, `ShadowDistance`
//! 4 to 10, one direction — and why: at 6 the shadow is a halo about 1.4 times
//! darker than the backdrop, at 10 it is already 1.8, and at 15 and above it is a
//! black rim. Numbers in a skill are claims about the engine. This pictures a
//! card with the engine that draws Preview, Run Form and a built program
//! (`snapshot::render_form_image`, the CPU rasteriser) and reads the pixels, so
//! the claim breaks here the day the engine's shadow changes, not in a form.
//!
//! The ratio is WCAG relative luminance, shadow core against the backdrop beside
//! it: 1.0 is invisible, 2.17 is black on this glass.

#![cfg(feature = "render")]

use cobolt_forms::load_form_from_str;
use cobolt_forms::snapshot::{render_form_image, SnapshotOptions};
use egui::ColorImage;

/// A Spatial form with one card at (32, 32), 216 x 224, its shadow as asked.
fn card(opacity: i64, distance: i64, direction: &str, blur: bool) -> String {
    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="LAB" title="Lab" width="320" height="320" background="40404008" transparency="50" grid-size="8">
  <Control id="CRD" type="Panel" x="48" y="48" w="216" h="224">
    <Property name="BorderStyle">None</Property>
    <Property name="CornerRadius">22</Property>
    <Property name="BackgroundColor">#36383EB3</Property>
    <Property name="BackgroundGradientEnabled">true</Property>
    <Property name="BackgroundGradientStartColor">#4E4E4EB3</Property>
    <Property name="BackgroundGradientEndColor">#000000B3</Property>
    <Property name="BackgroundGradientDirection">South</Property>
    <Property name="ForegroundColor">#FFFFFFFF</Property>
    <Property name="ShadowEnabled">true</Property>
    <Property name="ShadowOpacity">{opacity}</Property>
    <Property name="ShadowColor">#000000FF</Property>
    <Property name="ShadowLightColor">#4E4E4EFF</Property>
    <Property name="ShadowDirection">{direction}</Property>
    <Property name="ShadowDistance">{distance}</Property>
    <Property name="ShadowBlur">{blur}</Property>
    <Property name="ShadowBlurStrength">8</Property>
  </Control>
</Form>"##
    )
}

fn picture(xml: &str) -> ColorImage {
    let form = load_form_from_str(xml).expect("the lab form loads");
    let opts = SnapshotOptions { theme_default: Some(cobolt_forms::theme::SPATIAL.into()), ..Default::default() };
    render_form_image(&form, &opts)
}

fn rgb(img: &ColorImage, x: usize, y: usize) -> [f64; 3] {
    let [r, g, b, _] = img.pixels[y * img.size[0] + x].to_srgba_unmultiplied();
    [f64::from(r), f64::from(g), f64::from(b)]
}

fn lum(c: [f64; 3]) -> f64 {
    let f = |v: f64| {
        let v = v / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
}

fn ratio(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (la, lb) = (lum(a), lum(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// The card spans x 48..264, y 48..272. The backdrop is read well clear of it.
const BELOW: (usize, usize) = (156, 272);
const ABOVE: (usize, usize) = (156, 48);

/// Contrast of the pixel `gap` points past the card's edge against the backdrop.
fn halo(img: &ColorImage, edge: (usize, usize), gap: isize, upward: bool) -> f64 {
    let backdrop = rgb(img, 8, 8);
    let y = if upward { edge.1 as isize - gap } else { edge.1 as isize + gap };
    ratio(rgb(img, edge.0, y as usize), backdrop)
}

#[test]
fn opacity_four_to_eight_is_a_quiet_halo_and_fifteen_is_a_black_rim() {
    let at = |o: i64| halo(&picture(&card(o, 7, "SouthEast", true)), BELOW, 3, false);
    let (o4, o6, o8, o10, o15, o35) = (at(4), at(6), at(8), at(10), at(15), at(35));
    println!("shadow vs backdrop, 3 points below the card: 4={o4:.2} 6={o6:.2} 8={o8:.2} 10={o10:.2} 15={o15:.2} 35={o35:.2}");
    assert!((1.15..=1.40).contains(&o4), "opacity 4 is a faint halo, was {o4:.2}");
    assert!((1.30..=1.60).contains(&o6), "opacity 6 (Spatial's own) is about 1.4, was {o6:.2}");
    assert!(o8 <= 1.75, "opacity 8 is still quiet, was {o8:.2}");
    assert!((1.70..=1.95).contains(&o10), "opacity 10 is already heavy, about 1.8, was {o10:.2}");
    assert!(o15 >= 2.0, "opacity 15 is a black rim, was {o15:.2}");
    assert!(o35 >= 2.1, "opacity 35 is black on this glass, was {o35:.2}");
    assert!(o4 < o6 && o6 < o8 && o8 < o10 && o10 < o15, "the stronger the opacity, the darker the shadow");
}

#[test]
fn a_shadow_falls_one_way_and_a_mixed_direction_shows() {
    let south_east = picture(&card(6, 7, "SouthEast", true));
    let north_west = picture(&card(6, 7, "NorthWest", true));
    let (below_se, above_se) = (halo(&south_east, BELOW, 3, false), halo(&south_east, ABOVE, 3, true));
    let (below_nw, above_nw) = (halo(&north_west, BELOW, 3, false), halo(&north_west, ABOVE, 3, true));
    println!("SouthEast below={below_se:.2} above={above_se:.2}; NorthWest below={below_nw:.2} above={above_nw:.2}");
    assert!(below_se >= 1.30 && above_se < 1.05, "SouthEast casts below the card and not above it");
    assert!(above_nw >= 1.30 && below_nw < 1.05, "NorthWest casts the other way: two lights on one form");
}

#[test]
fn distance_twelve_and_more_detaches_into_a_plate_and_no_blur_is_a_hard_line() {
    let reach = |d: i64| halo(&picture(&card(6, d, "SouthEast", true)), BELOW, 10, false);
    let (d4, d7, d12, d16) = (reach(4), reach(7), reach(12), reach(16));
    println!("shadow 10 points past the card: distance 4={d4:.2} 7={d7:.2} 12={d12:.2} 16={d16:.2}");
    assert!(d4 < 1.05 && d7 < 1.10, "at 4 to 7 the shadow has faded out 10 points past the card");
    assert!(d12 >= 1.15, "at 12 it still reaches 10 points past: it starts to float, was {d12:.2}");
    assert!(d16 >= 1.30, "at 16 it is a plate offset from the card, was {d16:.2}");

    let blurred = halo(&picture(&card(6, 7, "SouthEast", true)), BELOW, 3, false);
    let hard = halo(&picture(&card(6, 7, "SouthEast", false)), BELOW, 3, false);
    println!("3 points below the card: blurred={blurred:.2} unblurred={hard:.2}");
    assert!(hard < blurred, "an unblurred shadow is a thin hard edge, not the soft halo Spatial draws");
}

/// The design steps tell an agent that text with no `FontSize` is drawn at 11
/// points — under the floor of 12 — so it must always set one. This is the
/// paint's own fallback, read where the paint reads it.
#[test]
fn text_with_no_font_size_is_drawn_at_eleven_points() {
    use cobolt_forms::paint::ctrl_font_size;
    let with = |props: &str| {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="P" title="P" width="400" height="300"><Control id="C" type="Button" x="8" y="8" w="120" h="40"><Property name="Caption">c</Property>{props}</Control></Form>"#
        );
        ctrl_font_size(&load_form_from_str(&xml).expect("loads").controls[0])
    };
    assert_eq!(with(""), 11.0, "no FontSize: 11 points, under the floor of 12");
    assert_eq!(with(r#"<Property name="FontSize">14</Property>"#), 14.0, "the body size the steps give");
}
