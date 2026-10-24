// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `cobolt_forms::textmetrics` — the arithmetic the Web player answers a text's height, its baseline and its
//! width with — is held to **egui itself**: the same text laid out by the painter, at every size from 6 to 64
//! points in half-point steps, must be as tall, have its baseline as low and be as wide as the module says.
//!
//! A fix to the painter's text layout that moves a row or a baseline turns this red, which is the signal that
//! the Web player's text has to move with it (WASM-PARITY).

#![cfg(feature = "render")]

use cobolt_forms::raster::{render_frame, Rasterizer};
use cobolt_forms::textmetrics;

/// One row's height and baseline, and a line's width, as the painter lays them out.
fn laid_out(ctx: &egui::Context, raster: &mut Rasterizer, size: f32, text: &str) -> (f32, f32, f32) {
    let mut answer = (0.0, 0.0, 0.0);
    let draw = |ui: &mut egui::Ui| {
        let galley = ui.painter().layout_no_wrap(text.to_owned(), egui::FontId::proportional(size), egui::Color32::WHITE);
        let row = &galley.rows[0];
        answer = (galley.size().y, row.row.glyphs[0].pos.y, galley.size().x);
    };
    let _ = render_frame(ctx, raster, egui::vec2(64.0, 64.0), egui::Color32::BLACK, 0.0, draw);
    answer
}

#[test]
fn a_row_its_baseline_and_a_line_are_what_the_painter_lays_out() {
    let ctx = egui::Context::default();
    ctx.set_fonts(cobolt_forms::fonts::base_font_definitions());
    let mut raster = Rasterizer::new();
    let mut rows = 0;
    let mut widest_gap = 0.0f64;
    let mut worst = String::new();
    let mut step = 12; // 6.0 pt
    while step <= 128 {
        let size = step as f32 / 2.0;
        let (row, base, _) = laid_out(&ctx, &mut raster, size, "Hxgp");
        assert_eq!(textmetrics::row_height(size as f64) as f32, row, "the row at {size} pt");
        assert_eq!(textmetrics::ascent(size as f64) as f32, base, "the baseline at {size} pt");
        rows += 1;
        for text in ["Hxgp", "R$ 14,83 tri", "Maiores fundos (PL)", "100%", "xxxx", "AVATAR To Ty We Yo", "Multimercados FIDC", "PL médio por fundo | maior: R$ 283,24 bi"] {
            let (_, _, width) = laid_out(&ctx, &mut raster, size, text);
            let ours = textmetrics::text_width(text, size as f64);
            let gap = (ours - width as f64).abs();
            if gap > widest_gap {
                widest_gap = gap;
                worst = format!("{text:?} at {size} pt: module {ours:.3}, painter {width:.3}");
            }
            // egui rounds each glyph's advance a little; the sum of the face's own advances and its kerning is within a
            // fraction of a pixel of it.
            assert!(gap <= 0.2 + 0.004 * width as f64, "{text:?} at {size} pt: module {ours:.3}, painter {width:.3}");
        }
        step += 1;
    }
    println!("text metrics: {rows} sizes (6 to 64 pt, half-point steps), rows and baselines equal to the painter's; widest width gap {widest_gap:.3} px ({worst})");
}

/// A form's font factor (`FontScaling`, spec 056) hands the painter a designed size times 0.85 up to 1.3 — a fraction that is not a
/// half point (28 at 0.85 is 23.8) — and the painter takes it as it is (`resolve_font_size`). The module must follow it there too:
/// every size the PowerAnalytics form designs, at the factors its width makes, laid out by the painter.
#[test]
fn the_sizes_a_font_factor_makes_are_what_the_painter_lays_out_too() {
    let ctx = egui::Context::default();
    ctx.set_fonts(cobolt_forms::fonts::base_font_definitions());
    let mut raster = Rasterizer::new();
    let mut checked = 0;
    let mut widest_gap = 0.0f64;
    for designed in [9.0f32, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 20.0, 22.0, 24.0, 30.0, 34.0] {
        for factor in [0.85f32, 0.889, 0.949, 1.111, 1.3] {
            let size = designed * factor;
            let (row, base, _) = laid_out(&ctx, &mut raster, size, "Hxgp");
            assert_eq!(textmetrics::row_height(size as f64) as f32, row, "the row at {size} pt ({designed} x {factor})");
            assert_eq!(textmetrics::ascent(size as f64) as f32, base, "the baseline at {size} pt ({designed} x {factor})");
            for text in ["R$ 14,83 tri", "Maiores fundos (PL)", "PL médio por fundo | maior: R$ 283,24 bi"] {
                let (_, _, width) = laid_out(&ctx, &mut raster, size, text);
                let gap = (textmetrics::text_width(text, size as f64) - width as f64).abs();
                widest_gap = widest_gap.max(gap);
                assert!(gap <= 0.2 + 0.004 * width as f64, "{text:?} at {size} pt ({designed} x {factor}): painter {width:.3}, gap {gap:.3}");
            }
            checked += 1;
        }
    }
    println!("text metrics at the sizes a font factor makes: {checked} sizes, rows and baselines equal to the painter's; widest width gap {widest_gap:.3} px");
}
