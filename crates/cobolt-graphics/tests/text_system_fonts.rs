// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Text with **real fonts** for Latin, Japanese, Chinese, Arabic and Hebrew (WASM-018 R8, AC3).
//!
//! The other text tests use fonts generated in the test (exact, known metrics, nothing to
//! license). These use the fonts the machine already has - **read by the test, never copied into
//! the repository** - to show the shaper doing what a synthetic font cannot: Arabic letters
//! taking their joined forms, Han and kana characters set full width, Hebrew running right to
//! left, and `MeasureText` agreeing with the pixels `DrawText` leaves, for each script.
//!
//! A machine without a suitable font skips the check for that script and says so on a line that
//! starts with `SKIPPED`; `GRAPHICS_REQUIRE_FONTS=1` turns the skip into a failure.
#![cfg(feature = "text")]

use std::sync::Arc;

use cobolt_graphics::canvas::{ReadBack, Surface, SurfaceConfig};
use cobolt_graphics::display_list::DisplayListBuilder;
use cobolt_graphics::resources::MapResources;
use cobolt_graphics::text::FontSet;
use cobolt_graphics::display_list::TextDirection;
use cobolt_graphics::textapi::{FontSpec, TextEngine, TextRequest};
use cobolt_graphics::{Limits, PixelBuffer, Rgba};

/// Where the common operating systems keep fonts that cover each script.
const CANDIDATES: &[(&str, &[&str])] = &[
    (
        "latin",
        &[
            "/System/Library/Fonts/Helvetica.ttc",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu/DejaVuSans.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
        ],
    ),
    (
        "hebrew",
        &[
            "/System/Library/Fonts/Supplemental/Arial Hebrew.ttc",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu/DejaVuSans.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
        ],
    ),
    (
        "arabic",
        &[
            "/System/Library/Fonts/GeezaPro.ttc",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu/DejaVuSans.ttf",
            "C:\\Windows\\Fonts\\tahoma.ttf",
        ],
    ),
    (
        "japanese",
        &[
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "C:\\Windows\\Fonts\\msgothic.ttc",
            "C:\\Windows\\Fonts\\YuGothR.ttc",
        ],
    ),
    (
        "chinese",
        &[
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "C:\\Windows\\Fonts\\simsun.ttc",
            "C:\\Windows\\Fonts\\msyh.ttc",
        ],
    ),
];

/// A font set holding the first font found for `script`, and the family name to ask for; `None` (after saying so) when there is none.
fn font_for(script: &str) -> Option<(Arc<FontSet>, String)> {
    let paths = CANDIDATES.iter().find(|(s, _)| *s == script).map(|(_, p)| *p).unwrap_or(&[]);
    for p in paths {
        let Ok(bytes) = std::fs::read(p) else { continue };
        let fonts = FontSet::new();
        let Ok(faces) = fonts.add_font_data(bytes) else { continue };
        if let Some(f) = faces.first() {
            eprintln!("{script}: using {} from {p}", f.family);
            return Some((Arc::new(fonts), f.family.clone()));
        }
    }
    if std::env::var("GRAPHICS_REQUIRE_FONTS").map(|v| v == "1").unwrap_or(false) {
        panic!("GRAPHICS_REQUIRE_FONTS=1 but no {script} font was found");
    }
    eprintln!("SKIPPED ({script}): no suitable font found on this machine");
    None
}

fn width(fonts: &FontSet, family: &str, size: f32, text: &str) -> f32 {
    let font = FontSpec::parse(&format!("{size}px \"{family}\"")).unwrap();
    fonts.layout(&TextRequest { text, font: &font, direction: TextDirection::Auto, wrap_width: None, line_height: None }).unwrap().width
}

fn ink(img: &PixelBuffer) -> Option<(i64, i64, i64, i64)> {
    let (mut x0, mut y0, mut x1, mut y1) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
    for y in 0..img.height() as i64 {
        for x in 0..img.width() as i64 {
            if img.get(x, y).unwrap().a > 0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    (x0 != i64::MAX).then_some((x0, y0, x1 + 1, y1 + 1))
}

/// Draw `text` and measure it, and compare: the ink on the canvas against what `MeasureText` said.
fn measure_matches_draw(fonts: Arc<FontSet>, family: &str, text: &str) {
    let css = format!("40px \"{family}\"");
    let res = MapResources::new().with_text_engine(fonts);
    let mut s = Surface::new(&SurfaceConfig::new(900, 200), &Limits::default()).unwrap();
    let mut b = DisplayListBuilder::new();
    b.set_font(&css).set_text_align(2).set_text_baseline(0).set_fill_color(Rgba::BLACK).measure_text(1, text).draw_text(text, 40.0, 120.0, 0.0, 1);
    let out = s.execute(&b.take().unwrap(), &res).unwrap();
    let ReadBack::Text { metrics: m, .. } = &out.readbacks[0] else { panic!("no metrics") };
    assert!(m.width > 0.0, "{text}: no width");
    let (x0, y0, x1, y1) = ink(&s.to_image()).unwrap_or_else(|| panic!("{text}: nothing was drawn"));
    let tol = 1.5;
    assert!((40.0 - m.actual_left - x0 as f32).abs() <= tol, "{text}: left {} vs ink {x0}", 40.0 - m.actual_left);
    assert!((40.0 + m.actual_right - x1 as f32).abs() <= tol, "{text}: right {} vs ink {x1}", 40.0 + m.actual_right);
    assert!((120.0 - m.actual_ascent - y0 as f32).abs() <= tol, "{text}: top {} vs ink {y0}", 120.0 - m.actual_ascent);
    assert!((120.0 + m.actual_descent - y1 as f32).abs() <= tol, "{text}: bottom {} vs ink {y1}", 120.0 + m.actual_descent);
    eprintln!("  {text:?}: width {:.1}, ink {x0}..{x1} x {y0}..{y1} - MeasureText equals DrawText within {tol} px", m.width);
}

#[test]
fn latin_with_a_real_font_is_measured_as_drawn_and_kerned() {
    let Some((fonts, family)) = font_for("latin") else { return };
    measure_matches_draw(fonts.clone(), &family, "Hello, World");
    measure_matches_draw(fonts.clone(), &family, "AVATAR To Wa");
    // The width of a word is not the width of its letters one by one: real fonts kern.
    let (whole, parts) = (width(&fonts, &family, 40.0, "AV"), width(&fonts, &family, 40.0, "A") + width(&fonts, &family, 40.0, "V"));
    eprintln!("  \"AV\" is {whole:.2} wide, \"A\" + \"V\" {parts:.2}");
    assert!(whole <= parts + 0.01, "a kerned pair is never wider than its letters apart");
}

#[test]
fn arabic_letters_take_their_joined_forms_and_the_text_runs_right_to_left() {
    let Some((fonts, family)) = font_for("arabic") else { return };
    let beh4 = "بببب";
    let joined = width(&fonts, &family, 40.0, beh4);
    let apart = 4.0 * width(&fonts, &family, 40.0, "ب");
    eprintln!("  four beh joined: {joined:.2}; four isolated forms: {apart:.2}");
    assert!((joined - apart).abs() > 0.5, "joining changes the glyphs and with them the advance ({joined} vs {apart})");
    let font = FontSpec::parse(&format!("40px \"{family}\"")).unwrap();
    let block = fonts.layout(&TextRequest { text: "مرحبا بالعالم", font: &font, direction: TextDirection::Auto, wrap_width: None, line_height: None }).unwrap();
    assert!(block.rtl, "an Arabic paragraph is detected as right to left");
    measure_matches_draw(fonts.clone(), &family, "مرحبا بالعالم");
    measure_matches_draw(fonts, &family, "السلام 123 عليكم");
}

#[test]
fn hebrew_with_a_real_font() {
    let Some((fonts, family)) = font_for("hebrew") else { return };
    let font = FontSpec::parse(&format!("40px \"{family}\"")).unwrap();
    let block = fonts.layout(&TextRequest { text: "שלום עולם", font: &font, direction: TextDirection::Auto, wrap_width: None, line_height: None }).unwrap();
    assert!(block.rtl && block.width > 40.0);
    measure_matches_draw(fonts.clone(), &family, "שלום עולם");
    measure_matches_draw(fonts, &family, "abc שלום def");
}

#[test]
fn japanese_is_set_full_width_and_wraps() {
    let Some((fonts, family)) = font_for("japanese") else { return };
    let w = width(&fonts, &family, 40.0, "日本語");
    eprintln!("  \"日本語\" at 40px is {w:.1} wide (3 em = 120)");
    assert!((w - 120.0).abs() < 8.0, "three full-width characters are about three em wide: {w}");
    measure_matches_draw(fonts.clone(), &family, "日本語のテキスト");
    let font = FontSpec::parse(&format!("40px \"{family}\"")).unwrap();
    let block = fonts.layout(&TextRequest { text: "日本語のテキストを折り返します。", font: &font, direction: TextDirection::Auto, wrap_width: Some(200.0), line_height: None }).unwrap();
    assert!(block.lines.len() >= 3, "{} lines", block.lines.len());
    assert!(block.lines.iter().all(|l| l.advance <= 200.0 + 1.0), "no line is wider than the box: {:?}", block.lines.iter().map(|l| l.advance).collect::<Vec<_>>());
}

#[test]
fn chinese_is_set_full_width_too() {
    let Some((fonts, family)) = font_for("chinese") else { return };
    let w = width(&fonts, &family, 40.0, "汉字书写");
    eprintln!("  \"汉字书写\" at 40px is {w:.1} wide (4 em = 160)");
    assert!((w - 160.0).abs() < 10.0, "{w}");
    measure_matches_draw(fonts, &family, "你好，世界");
}
