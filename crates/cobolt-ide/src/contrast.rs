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

#[cfg(test)]
mod tests {
    use super::*;

    /// Moved verbatim from `flags.rs` — proves the move changed nothing.
    #[test]
    fn contrast_ratio_matches_the_wcag_reference_points() {
        assert!((contrast_ratio(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(Color32::WHITE, Color32::WHITE) - 1.0).abs() < 0.001);
        let (a, b) = (Color32::from_rgb(84, 110, 122), Color32::WHITE);
        assert!((contrast_ratio(a, b) - contrast_ratio(b, a)).abs() < 1e-9);
    }
}
