// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WCAG 2.1 contrast math, shared by every IDE surface that must stay
//! legible against a themed background it does not control — originally
//! written for the language-selector flags (`flags.rs`), reused by the
//! Project's Crates System/System-dependency/addable markers (spec 045 R10).
//! One formula, checked once per caller against its own legibility floor.

use egui::Color32;

/// WCAG 2.1 relative luminance.
pub(crate) fn relative_luminance(c: Color32) -> f64 {
    let channel = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}

/// WCAG 2.1 contrast ratio, 1.0 (identical) … 21.0 (black on white).
pub(crate) fn contrast_ratio(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// WCAG AAA for body text, the level a "high contrast" request means.
pub(crate) const HIGH_CONTRAST: f64 = 7.0;

/// `c`, moved toward black on a light theme (white on a dark one) only as far
/// as it must go to reach `min` against every surface in `surfaces`. A colour
/// that already passes comes back unchanged, so a palette tuned for the dark
/// themes keeps its look there and stops washing out on a light one.
pub(crate) fn legible(c: Color32, surfaces: &[Color32], min: f64, dark: bool) -> Color32 {
    let toward = if dark { Color32::WHITE } else { Color32::BLACK };
    for step in 0..=20 {
        let m = c.lerp_to_gamma(toward, step as f32 / 20.0);
        if surfaces.iter().all(|s| contrast_ratio(m, *s) >= min) {
            return m;
        }
    }
    toward
}

/// [`legible`] at [`HIGH_CONTRAST`] against the active IDE theme's panel and
/// alternating-row surfaces: the ink for a hand-coloured label in a panel.
/// The inspector's Events and Procs tabs were olive and pale grey on
/// Neumorphic Light's grey, under 2:1 (operator, 2026-10-02).
pub(crate) fn ink(c: Color32) -> Color32 {
    let t = crate::theme::active();
    legible(c, &[t.bg_panel, t.faint_bg], HIGH_CONTRAST, t.dark)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On Neumorphic Light every inspector ink reaches 7:1; on a dark theme
    /// a colour that already passes is returned untouched.
    #[test]
    fn ink_reaches_high_contrast_on_a_light_theme_and_leaves_dark_ones_alone() {
        let light = crate::theme::NEUMORPHIC_LIGHT;
        let surfaces = [light.bg_panel, light.faint_bg];
        for c in [
            Color32::from_rgb(200, 200, 100),
            Color32::from_gray(170),
            Color32::GRAY,
            Color32::from_rgb(210, 170, 100),
            Color32::from_rgb(100, 220, 100),
        ] {
            let m = legible(c, &surfaces, HIGH_CONTRAST, false);
            for s in surfaces {
                assert!(contrast_ratio(m, s) >= HIGH_CONTRAST, "{c:?} -> {m:?} on {s:?}: {:.2}", contrast_ratio(m, s));
            }
            println!("{c:?} {:.2}:1 -> {m:?} {:.2}:1", contrast_ratio(c, light.bg_panel), contrast_ratio(m, light.bg_panel));
        }
        let dark = crate::theme::default_theme();
        let edit = Color32::from_rgb(200, 200, 100);
        if contrast_ratio(edit, dark.bg_panel) >= HIGH_CONTRAST && contrast_ratio(edit, dark.faint_bg) >= HIGH_CONTRAST {
            assert_eq!(legible(edit, &[dark.bg_panel, dark.faint_bg], HIGH_CONTRAST, dark.dark), edit);
        }
    }

    /// Moved verbatim from `flags.rs` — proves the move changed nothing.
    #[test]
    fn contrast_ratio_matches_the_wcag_reference_points() {
        assert!((contrast_ratio(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(Color32::WHITE, Color32::WHITE) - 1.0).abs() < 0.001);
        let (a, b) = (Color32::from_rgb(84, 110, 122), Color32::WHITE);
        assert!((contrast_ratio(a, b) - contrast_ratio(b, a)).abs() < 1e-9);
    }
}
