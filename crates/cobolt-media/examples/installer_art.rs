// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Compose the installer artwork from the one mascot image.
//!
//! ```text
//! cargo run -p cobolt-media --example installer_art -- <mascot.png> <out-dir>
//! ```
//!
//! Three images come out, and every one of them is derived from
//! `assets/images/chibi.png` rather than drawn by hand:
//!
//! | File                 | Size       | Where it goes                           |
//! |----------------------|------------|-----------------------------------------|
//! | `wix-dialog.bmp`     | 493 × 312  | the .msi welcome and finish screens      |
//! | `wix-banner.bmp`     | 493 × 58   | the .msi licence / folder / ready screens |
//! | `dmg-background.png` | 1100 × 800 | the .dmg volume window                   |
//!
//! Each `.bmp` is written a second time as a `.png` beside it. WiX takes the
//! `.bmp`; the `.png` exists because almost nothing outside Windows will open a
//! BMP, and artwork nobody can look at is artwork nobody will fix.
//!
//! # Why the mascot sits where it does
//!
//! Not taste — the host UI's text is already positioned and this artwork has to
//! keep out of its way. **WixUI draws its text in black**, at coordinates
//! compiled into WixUIExtension that no property can move:
//!
//! * the welcome and finish dialogs put their title and body at x ≥ 180 px, so
//!   that side has to stay **light** or the black text is unreadable — and the
//!   mascot goes **left**;
//! * the banner puts its heading at x ≈ 20 px, so **that** side stays light and
//!   the mascot goes **right**.
//!
//! macOS has the same constraint for a different reason: Finder paints icon
//! labels dark, so the half holding the two icons is light and the mascot takes
//! the other half. Hence one rule across all three: **the mascot lives on the
//! dark panel, the host's text lives on the light one.**
//!
//! Nothing here rasterises text. There is no font in this repository to do it
//! with, and every one of these surfaces already has its own text drawn over
//! the top — a second wordmark underneath would only collide with it.

use image::{Rgba, RgbaImage};

/// The dark panel, top-left to bottom-right.
const DARK_A: [f32; 3] = [36.0, 26.0, 20.0];
const DARK_B: [f32; 3] = [13.0, 10.0, 8.0];
/// The light panel the host's black text sits on — warm, not a flat white, so
/// it belongs to the same picture as the ember beside it.
const LIGHT_A: [f32; 3] = [248.0, 245.0, 241.0];
const LIGHT_B: [f32; 3] = [235.0, 228.0, 219.0];
/// The ember behind the mascot, and the rule that divides the two panels.
const EMBER: [f32; 3] = [232.0, 130.0, 46.0];
const EMBER_PEAK: f32 = 0.34;

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
    ]
}

/// Lay one RGBA pixel over an opaque background, the usual source-over.
fn over(dst: [f32; 3], src: [f32; 3], alpha: f32) -> [f32; 3] {
    [
        lerp(dst[0], src[0], alpha),
        lerp(dst[1], src[1], alpha),
        lerp(dst[2], src[2], alpha),
    ]
}

/// Where the dark panel is, and which way round the picture reads.
#[derive(Clone, Copy)]
struct Split {
    /// x of the boundary between the two panels.
    at: u32,
    /// True when the dark panel is the left one (and so is the mascot).
    dark_left: bool,
}

/// Paint the two panels and the ember glow. The mascot goes on afterwards.
fn ground(w: u32, h: u32, split: Split, ember_at: (f32, f32), ember_r: f32) -> RgbaImage {
    let mut img = RgbaImage::new(w, h);
    let (fw, fh) = (w as f32, h as f32);
    for y in 0..h {
        for x in 0..w {
            let on_dark = if split.dark_left {
                x < split.at
            } else {
                x >= split.at
            };
            // Each panel runs its own diagonal, so neither looks like flat fill.
            let t = ((x as f32 / fw) + (y as f32 / fh)) * 0.5;
            let mut c = if on_dark {
                mix(DARK_A, DARK_B, t)
            } else {
                mix(LIGHT_A, LIGHT_B, t)
            };
            if on_dark {
                // A soft radial ember, brightest behind the mascot and gone by
                // the edge of its radius. Squared falloff keeps it from reading
                // as a hard disc.
                let dx = x as f32 - ember_at.0;
                let dy = y as f32 - ember_at.1;
                let d = (dx * dx + dy * dy).sqrt() / ember_r;
                if d < 1.0 {
                    let fall = (1.0 - d) * (1.0 - d);
                    c = over(c, EMBER, EMBER_PEAK * fall);
                }
            }
            // The rule between the panels: two pixels of ember, so the join is
            // a decision rather than a seam.
            if split.at > 1 && (x == split.at || x + 1 == split.at) {
                c = over(c, EMBER, 0.75);
            }
            img.put_pixel(x, y, Rgba([c[0] as u8, c[1] as u8, c[2] as u8, 255]));
        }
    }
    img
}

/// Draw `src` onto `dst` at `(ox, oy)`, scaled to `(tw, th)`, respecting alpha.
fn place(dst: &mut RgbaImage, src: &RgbaImage, ox: i64, oy: i64, tw: u32, th: u32) {
    let scaled = image::imageops::resize(src, tw, th, image::imageops::FilterType::Lanczos3);
    for y in 0..th {
        for x in 0..tw {
            let (dx, dy) = (ox + x as i64, oy + y as i64);
            if dx < 0 || dy < 0 || dx >= dst.width() as i64 || dy >= dst.height() as i64 {
                continue;
            }
            let s = scaled.get_pixel(x, y).0;
            let a = s[3] as f32 / 255.0;
            if a <= 0.0 {
                continue;
            }
            let d = dst.get_pixel(dx as u32, dy as u32).0;
            let c = over(
                [d[0] as f32, d[1] as f32, d[2] as f32],
                [s[0] as f32, s[1] as f32, s[2] as f32],
                a,
            );
            dst.put_pixel(
                dx as u32,
                dy as u32,
                Rgba([c[0] as u8, c[1] as u8, c[2] as u8, 255]),
            );
        }
    }
}

/// Drop the soft grey ellipse the artwork carries under the mascot's feet.
///
/// It is a contact shadow drawn for a light page, and on the dark panel it
/// reads as a grey smudge rather than as shadow. Cropping it off is not an
/// option — measured on `chibi.png` it spans rows 530-550 while the boots reach
/// 540, so a straight cut takes the feet with it.
///
/// So it is removed for what it is instead: in the bottom sixth of the frame,
/// any pixel that is both near-grey and light is shadow. The armour there is
/// dark, the trim is saturated orange, and neither answers that description —
/// the steel greaves are the closest call, and they sit well under the
/// brightness bar.
fn drop_contact_shadow(src: &RgbaImage) -> RgbaImage {
    const FROM: f32 = 0.84;
    const MAX_CHROMA: u8 = 30;
    const MIN_LUMA: f32 = 115.0;
    let mut out = src.clone();
    let start = (src.height() as f32 * FROM) as u32;
    let mut cleared = 0u32;
    for y in start..src.height() {
        for x in 0..src.width() {
            let p = src.get_pixel(x, y).0;
            if p[3] == 0 {
                continue;
            }
            let (r, g, b) = (p[0], p[1], p[2]);
            let chroma = r.max(g).max(b) - r.min(g).min(b);
            let luma = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
            if chroma < MAX_CHROMA && luma > MIN_LUMA {
                out.put_pixel(x, y, Rgba([r, g, b, 0]));
                cleared += 1;
            }
        }
    }
    println!("contact shadow: {cleared} px cleared below y={start}");
    out
}

/// The width this mascot takes at a given height, at its own aspect ratio.
fn width_for(src: &RgbaImage, height: u32) -> u32 {
    (height as f32 * src.width() as f32 / src.height() as f32).round() as u32
}

/// A flat chevron pointing right, for the gap between the two .dmg icons. Drawn
/// rather than lettered: it says "drag this onto that" without a word in any
/// language, which suits a window six locales will open.
///
/// The vertex is on the RIGHT and both arms run back to the left, which is why
/// `px` is clamped to the negative half. Without that clamp the two lines carry
/// straight through the centre and the arrow comes out as a cross — which is
/// exactly what the first render of this drew.
fn chevron(img: &mut RgbaImage, cx: u32, cy: u32, size: u32, thickness: f32) {
    let (cx, cy, s) = (cx as f32, cy as f32, size as f32);
    for y in (cy - s) as u32..=(cy + s) as u32 {
        for x in (cx - s) as u32..=(cx + s) as u32 {
            if x >= img.width() || y >= img.height() {
                continue;
            }
            // Distance to the nearer of the two arms, each a diagonal segment
            // running left from the vertex.
            let (px, py) = (x as f32 - cx, y as f32 - cy);
            let d = ((px + py).abs()).min((px - py).abs()) / std::f32::consts::SQRT_2;
            let on_arm = d < thickness && px <= 0.0 && px >= -s && py.abs() <= s;
            if on_arm {
                let d0 = img.get_pixel(x, y).0;
                let fade = 1.0 - (px.abs() / s) * 0.45;
                let c = over(
                    [d0[0] as f32, d0[1] as f32, d0[2] as f32],
                    EMBER,
                    0.55 * fade,
                );
                img.put_pixel(x, y, Rgba([c[0] as u8, c[1] as u8, c[2] as u8, 255]));
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let src_path = args.next().ok_or("usage: installer_art <mascot.png> <out-dir>")?;
    let out_dir = args.next().ok_or("usage: installer_art <mascot.png> <out-dir>")?;
    let out = std::path::Path::new(&out_dir);
    std::fs::create_dir_all(out)?;

    let mascot = drop_contact_shadow(&image::open(&src_path)?.to_rgba8());
    println!("mascot: {}×{}", mascot.width(), mascot.height());

    // ── .msi welcome / finish — 493 × 312 ────────────────────────────────────
    // WixUI's text starts at x = 180, so the light panel starts there and the
    // mascot fills the dark one with a little air on every side.
    {
        let (w, h) = (493u32, 312u32);
        let split = Split {
            at: 180,
            dark_left: true,
        };
        let fig_h = 168;
        let fig_w = width_for(&mascot, fig_h);
        let x = (split.at as i64 - fig_w as i64) / 2;
        let y = ((h - fig_h) / 2) as i64;
        let mut img = ground(w, h, split, (90.0, h as f32 / 2.0), 190.0);
        place(&mut img, &mascot, x, y, fig_w, fig_h);
        img.save(out.join("wix-dialog.bmp"))?;
        img.save(out.join("wix-dialog.png"))?;
        println!("wix-dialog.bmp   {w}×{h}, mascot {fig_w}×{fig_h} at ({x},{y}) — left panel");
    }

    // ── .msi banner — 493 × 58 ───────────────────────────────────────────────
    // The banner's heading is on the LEFT, so this one is the mirror: light
    // where the text is, mascot on the right.
    {
        let (w, h) = (493u32, 58u32);
        let split = Split {
            at: 400,
            dark_left: false,
        };
        let fig_h = 52;
        let fig_w = width_for(&mascot, fig_h);
        let x = (w - fig_w) as i64 - 20;
        let y = ((h - fig_h) / 2) as i64;
        let mut img = ground(w, h, split, (446.0, h as f32 / 2.0), 90.0);
        place(&mut img, &mascot, x, y, fig_w, fig_h);
        img.save(out.join("wix-banner.bmp"))?;
        img.save(out.join("wix-banner.png"))?;
        println!("wix-banner.bmp   {w}×{h}, mascot {fig_w}×{fig_h} at ({x},{y}) — right panel");
    }

    // ── .dmg volume window — 1100 × 800 ──────────────────────────────────────
    // Finder paints its icon labels dark, so the half holding the application
    // and the Applications alias is the light one. The mascot takes the right,
    // vertically centred, well clear of both icons.
    {
        let (w, h) = (1100u32, 800u32);
        let split = Split {
            at: 640,
            dark_left: false,
        };
        let fig_h = 348;
        let fig_w = width_for(&mascot, fig_h);
        let x = (w - fig_w) as i64 - 46;
        let y = ((h - fig_h) / 2) as i64;
        let mut img = ground(w, h, split, (870.0, h as f32 / 2.0), 360.0);
        // Between the icon slots the AppleScript sets — 250 and 500 — so the
        // chevron lands in the gap rather than under a label.
        chevron(&mut img, 384, 392, 26, 5.0);
        place(&mut img, &mascot, x, y, fig_w, fig_h);
        img.save(out.join("dmg-background.png"))?;
        println!("dmg-background.png {w}×{h}, mascot {fig_w}×{fig_h} at ({x},{y}) — right panel");
    }

    Ok(())
}
