// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Generates the **reference** asset-pack theme `cobalt-steel` (spec 007, T8).
//!
//! This is an *original, procedurally generated* pack (no external artwork — R16)
//! whose only purpose is to exercise the 9-slice engine end-to-end: per-control
//! skins, a themed background, and a chart palette. The four "special" art packs
//! (T9) are produced separately by the operator's image agent.
//!
//! Run from the repo root:
//! `cargo run -p cobolt-forms --example gen_reference_theme --features render`

use image::{Rgba, RgbaImage};

/// Draw a rounded-rectangle 9-slice tile: `radius`-cornered `border`-stroked
/// panel over a vertical fill gradient. Returns a `size`×`size` RGBA image.
///
/// Everything is measured from the signed distance to the rounded outline, so the
/// rim is `border_w` wide along the straight edges AND around each corner arc, and
/// the arc's outer edge is anti-aliased. (It was measured from the nearest straight
/// edge instead, which is far from the outline on a corner's diagonal: the rim
/// stopped where each arc began and the fill was left chamfered — the broken
/// corners of every Cobalt Steel card.)
fn rounded_tile(
    size: u32,
    radius: f32,
    fill_top: [u8; 4],
    fill_bot: [u8; 4],
    border: [u8; 4],
    border_w: f32,
) -> RgbaImage {
    let mut img = RgbaImage::new(size, size);
    let s = size as f32;
    let half = s / 2.0;
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Signed distance to the rounded rectangle, negative inside: the box
            // distance with the corner radius taken off each half-extent.
            let qx = (fx - half).abs() - (half - radius);
            let qy = (fy - half).abs() - (half - radius);
            let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius;
            // How much of this pixel the shape covers: all of it inside, none
            // outside, a one-pixel ramp across the outline.
            let cover = (0.5 - d).clamp(0.0, 1.0);
            if cover <= 0.0 {
                // Transparent, but in the rim's colour so a filtered edge has no dark fringe.
                img.put_pixel(x, y, Rgba([border[0], border[1], border[2], 0]));
                continue;
            }
            // vertical gradient fill
            let t = fy / s;
            let lerp = |a: u8, b: u8| (a as f32 * (1.0 - t) + b as f32 * t) as u8;
            let mut px = [
                lerp(fill_top[0], fill_bot[0]),
                lerp(fill_top[1], fill_bot[1]),
                lerp(fill_top[2], fill_bot[2]),
                lerp(fill_top[3], fill_bot[3]),
            ];
            // Rim: within `border_w` of the outline, measured along its normal.
            if -d < border_w {
                px = border;
            }
            px[3] = (px[3] as f32 * cover).round() as u8;
            img.put_pixel(x, y, Rgba(px));
        }
    }
    img
}

fn main() {
    let dir = std::path::Path::new("assets/themes/cobalt-steel");
    std::fs::create_dir_all(dir).expect("create theme dir");

    // Brushed cobalt-steel palette.
    let panel = rounded_tile(
        48,
        12.0,
        [44, 56, 84, 255],
        [28, 36, 60, 255],
        [120, 150, 210, 255],
        2.0,
    );
    let groupbox = rounded_tile(
        48,
        12.0,
        [40, 52, 80, 235],
        [26, 34, 58, 235],
        [120, 150, 210, 220],
        2.0,
    );
    let button = rounded_tile(
        40,
        10.0,
        [86, 110, 168, 255],
        [52, 70, 120, 255],
        [180, 205, 255, 255],
        2.0,
    );
    let button_h = rounded_tile(
        40,
        10.0,
        [110, 140, 205, 255],
        [70, 96, 158, 255],
        [210, 230, 255, 255],
        2.0,
    );
    let button_p = rounded_tile(
        40,
        10.0,
        [54, 72, 122, 255],
        [40, 54, 96, 255],
        [150, 175, 225, 255],
        2.5,
    );
    let textbox = rounded_tile(
        32,
        7.0,
        [232, 238, 250, 255],
        [214, 222, 240, 255],
        [120, 150, 210, 255],
        1.5,
    );
    let combobox = rounded_tile(
        32,
        7.0,
        [224, 232, 248, 255],
        [206, 216, 238, 255],
        [120, 150, 210, 255],
        1.5,
    );
    let listbox = rounded_tile(
        32,
        6.0,
        [236, 240, 250, 255],
        [224, 230, 244, 255],
        [120, 150, 210, 255],
        1.5,
    );
    // Background: a large soft vertical gradient tile (stretched to the form).
    let background = rounded_tile(
        64,
        0.0,
        [22, 28, 50, 255],
        [12, 16, 32, 255],
        [22, 28, 50, 255],
        0.0,
    );

    let save = |name: &str, img: &RgbaImage| {
        img.save(dir.join(name))
            .unwrap_or_else(|e| panic!("save {name}: {e}"));
    };
    save("panel.png", &panel);
    save("groupbox.png", &groupbox);
    save("button.png", &button);
    save("button_hover.png", &button_h);
    save("button_pressed.png", &button_p);
    save("textbox.png", &textbox);
    save("combobox.png", &combobox);
    save("listbox.png", &listbox);
    save("background.png", &background);

    let manifest = r##"# Reference asset-pack theme (spec 007). Procedurally generated by
# `cargo run -p cobolt-forms --example gen_reference_theme` — original work, no
# external artwork (R16). Proves the 9-slice engine end-to-end.
id = "cobalt-steel"
display_name = "Cobalt Steel (reference)"

[background]
image = "background.png"
tile = false

[palette]
foreground = "#e7eeff"
chart = ["#5A6EA8", "#8FB0FF", "#4CC8E8", "#C88FFF"]

[chart_style]
stroke_width = 2.5

[controls.panel]
image = "panel.png"
slice = [14, 14, 14, 14]

[controls.groupbox]
image = "groupbox.png"
slice = [14, 14, 14, 14]

[controls.button]
image = "button.png"
slice = [12, 12, 12, 12]
hover = "button_hover.png"
pressed = "button_pressed.png"

[controls.textbox]
image = "textbox.png"
slice = [8, 8, 8, 8]

[controls.combobox]
image = "combobox.png"
slice = [8, 8, 8, 8]

[controls.listbox]
image = "listbox.png"
slice = [7, 7, 7, 7]
"##;
    std::fs::write(dir.join("theme.toml"), manifest).expect("write theme.toml");
    println!("wrote reference theme pack to {}", dir.display());
}
