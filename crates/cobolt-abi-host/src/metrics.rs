// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **How wide, and how tall, the desktop's text is** — the one source is `cobolt_forms::textmetrics`
//! (egui's layout arithmetic with no egui in it, held to egui itself by a test of that crate), so the
//! player answers every question about a text with the desktop's own numbers.
//!
//! What is added here is the one thing that is not the desktop's arithmetic but a compensation for the
//! browser: [`stroke`], the outline that makes a browser's glyphs carry the ink the desktop's carry.

pub use cobolt_forms::textmetrics::*;

/// A whole pixel the way the desktop's text layout puts a galley on one: halves go away from zero.
pub fn whole(v: f64) -> f64 {
    if v < 0.0 {
        -((-v + 0.5).floor())
    } else {
        (v + 0.5).floor()
    }
}

/// Where the top of one line of text of `size` goes, in whole pixels below the top of a frame `frame` tall that centres it, such that a
/// browser's baseline is where the desktop's is (`Align2::*_CENTER` makes the line's top `centre - row / 2`, rounded; the browser's own
/// baseline differs from the desktop's by `baseline_nudge`).
pub fn text_top(frame: f64, size: f64) -> f64 {
    whole((frame - row_height(size)) / 2.0) + baseline_nudge(size)
}

/// A piecewise-linear curve through `(size, value)` points, held flat beyond its ends.
fn curve(points: &[(f64, f64)], x: f64) -> f64 {
    let (first, last) = (points[0], points[points.len() - 1]);
    if x <= first.0 {
        return first.1;
    }
    if x >= last.0 {
        return last.1;
    }
    for w in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x <= x1 {
            return y0 + (y1 - y0) * (x - x0) / (x1 - x0);
        }
    }
    last.1
}

/// Is `ink` a light colour (text on a dark face) rather than a dark one? By luma, in sRGB.
pub fn is_light(ink: [u8; 4]) -> bool {
    0.299 * ink[0] as f64 + 0.587 * ink[1] as f64 + 0.114 * ink[2] as f64 > 140.0
}

/// The width in pixels of the outline (`-webkit-text-stroke`) that gives a browser's text the same ink as
/// the desktop's at `size` pixels, for text of this `ink` colour (bold text is the same text put down twice, and the
/// stroke is that of the plain text).
///
/// The two rasterisers do not weigh a glyph alike. Measured on this face, text by text (the sum of
/// the absolute difference to the face under it over the whole line; headless Chrome against the painter,
/// 9 to 30 px, dark ink on white and white ink on navy, regular and bold, 2026-10-24): at 11 px a browser's dark
/// text carries 14 % less ink than the desktop's and its white text 31 % less; at 22 px both carry more, not less.
/// An outline of the width below brings the ink to within a few per cent for sizes up to 24 px; past that the browser's text is
/// already the heavier and none is added (a stroke cannot take ink away).
pub fn stroke(size: f64, ink: [u8; 4]) -> f64 {
    const PLAIN_DARK: &[(f64, f64)] = &[(9.0, 0.12), (11.0, 0.09), (13.0, 0.05), (15.0, 0.04), (17.0, 0.0)];
    const PLAIN_LIGHT: &[(f64, f64)] = &[(9.0, 0.29), (11.0, 0.25), (13.0, 0.20), (15.0, 0.17), (17.0, 0.10), (21.0, 0.10), (24.0, 0.0)];
    let table = if is_light(ink) { PLAIN_LIGHT } else { PLAIN_DARK };
    (curve(table, size) * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_light_text_needs_the_most_outline_and_large_text_none() {
        assert_eq!(stroke(11.0, [255, 255, 255, 255]), 0.25);
        assert_eq!(stroke(11.0, [27, 42, 78, 255]), 0.09);
        assert!(stroke(11.0, [255, 255, 255, 255]) > stroke(11.0, [27, 42, 78, 255]), "light text needs more");
        assert_eq!(stroke(30.0, [27, 42, 78, 255]), 0.0);
        assert_eq!(stroke(40.0, [255, 255, 255, 255]), 0.0);
        // Between two points the curve is a straight line.
        assert_eq!(stroke(10.0, [255, 255, 255, 255]), 0.27);
    }

    #[test]
    fn the_ink_colour_decides_the_polarity() {
        assert!(is_light([255, 255, 255, 255]) && is_light([200, 210, 230, 255]));
        assert!(!is_light([27, 42, 78, 255]) && !is_light([107, 124, 153, 255]) && !is_light([0, 0, 0, 255]));
    }
}
