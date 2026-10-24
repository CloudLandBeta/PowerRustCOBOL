// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A drop shadow as numbers** — the one recipe of the blurred shadow, shared by the egui painter
//! (`paint::regular_shadow_stack`) and the web player's CSS (`box-shadow`), so the two draw the same
//! stack and cannot drift apart.
//!
//! The painter lays a shadow down as a STACK of rounded rectangles, the outermost and faintest first:
//! each ring is the control's rectangle (moved by the shadow's offset) grown by `expand` pixels, with a
//! corner radius grown by the same amount, filled with the shadow colour at `alpha` out of 255. The
//! rings overlap, so the core is denser than any one ring. A browser draws the same thing with one
//! `box-shadow` per ring and no blur (`0 dy 0 expand colour`).
//!
//! No `egui` here: this module is compiled into the web player.

/// How many rings a blur lays down per pixel of spread.
///
/// A blur here is a STACK of rounded rects, each one a little wider and a
/// little fainter than the last — so the number of rings is the resolution of
/// the gradient, and too few of them are visible as concentric tracks in the
/// shadow (operator, 2026-09-06: "the blur effect is leaving these tracks").
/// The neumorphic halo used a FIXED ten, which at the default blur strength
/// puts 1.8px and a visible alpha step between one ring and the next.
///
/// At 1.5 rings per pixel each ring advances two thirds of a pixel, so
/// consecutive rings overlap and their antialiased edges blend instead of
/// stepping.
pub const BLUR_RINGS_PER_PX: f32 = 1.5;

/// The steepest the falloff `exp(-3t²)` ever gets, `max |d/dt| = 1.485` at
/// `t = 1/√6`. The alpha step between two neighbouring rings is
/// `peak_alpha × this ÷ rings`, which is what turns a ring count into a
/// banding prediction.
pub const FALLOFF_MAX_SLOPE: f32 = 1.485;

/// The largest alpha step, out of 255, that still reads as a gradient.
///
/// The eye picks up banding in a smooth ramp from about 1% — roughly three
/// levels — and Mach banding makes an edge easier to see than the step itself.
/// TWO, not three, so a full-opacity shadow lands at 0.8% rather than sitting
/// exactly on the threshold: this was reported twice, and the second report was
/// a ramp measured at 1.2% (operator, 2026-09-06).
pub const MAX_ALPHA_STEP: f32 = 2.0;

/// Rings for a blur of `spread` pixels whose densest ring reaches `peak_alpha`
/// (0..255).
///
/// TWO things set the count, and the earlier version only had the first:
///
/// - **geometry** — rings must overlap on screen, so one per two thirds of a
///   pixel of spread;
/// - **alpha range** — the ramp is quantised into `rings` steps whatever its
///   width, so a FAINT shadow is smooth with a handful of rings while a dense
///   one needs many. Scaling on spread alone left the default 7% opacity clean
///   and a 100% one visibly banded: 27 rings over a 255-deep ramp is a step of
///   14, or 5.5%, where 1% is already visible (operator, 2026-09-06 — the
///   tracks were still there after the first attempt).
///
/// Floored so a hairline blur is still smooth. Capped at 192: past that the
/// step is already under a level and the rings are only costing shapes. A
/// control at the default opacity is unaffected — geometry still wins there —
/// so the extra rings are paid for only where the developer turned the shadow
/// up.
pub fn ring_count(spread: f32, peak_alpha: f32) -> usize {
    let by_geometry = spread.abs() * BLUR_RINGS_PER_PX;
    let by_alpha = peak_alpha.clamp(0.0, 255.0) * FALLOFF_MAX_SLOPE / MAX_ALPHA_STEP;
    (by_geometry.max(by_alpha).ceil() as usize).clamp(12, 192)
}

/// One ring of a shadow's stack: how far past the control's own rectangle it reaches, and its alpha out
/// of 255 (the colour is the shadow's own).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ring {
    pub expand: f32,
    pub alpha: u8,
}

impl Ring {
    /// The ring's colour as the painter premultiplies it: each component of the shadow colour scaled by
    /// the ring's alpha and **truncated** to a whole number. A ring alpha is small (a card's is 7 at most),
    /// so the truncation takes a visible share of a component, and twenty rings of it add up.
    pub fn premultiplied(&self, rgb: [u8; 3]) -> [u8; 3] {
        let k = self.alpha as f32 / 255.0;
        [(rgb[0] as f32 * k) as u8, (rgb[1] as f32 * k) as u8, (rgb[2] as f32 * k) as u8]
    }

    /// The straight colour that premultiplies back to [`Self::premultiplied`] — what a surface that keeps
    /// straight colours (CSS) is given to land on the painter's numbers rather than on the colour the
    /// developer named. A ring with no alpha paints nothing and keeps the colour as it is.
    pub fn straight(&self, rgb: [u8; 3]) -> [u8; 3] {
        if self.alpha == 0 {
            return rgb;
        }
        let p = self.premultiplied(rgb);
        let up = |c: u8| (c as f32 * 255.0 / self.alpha as f32).round().min(255.0) as u8;
        [up(p[0]), up(p[1]), up(p[2])]
    }
}

/// The rings of a shadow of `blur_strength` pixels (the developer's `ShadowBlurStrength`, its sign
/// dropped) at `opacity` (`ShadowOpacity` ÷ 100) over a face at `alpha_mul` (`1 − Transparency ÷ 100`),
/// **outermost first** — the order the painter lays them down.
///
/// A strength of 0 is one ring the size of the control; otherwise the ring `i` of `n` is at
/// `t = 1 − i/n`, reaches `t × blur_strength` pixels and has alpha `opacity × alpha_mul × exp(−3t²) × 255`
/// rounded to a whole number out of 255.
pub fn rings(blur_strength: usize, opacity: f32, alpha_mul: f32) -> Vec<Ring> {
    if blur_strength == 0 {
        return vec![Ring { expand: 0.0, alpha: (opacity * alpha_mul * 255.0) as u8 }];
    }
    let spread = blur_strength as f32;
    let layers = ring_count(spread, opacity * alpha_mul * 255.0);
    (0..=layers)
        .map(|i| {
            let t = 1.0 - (i as f32 / layers as f32);
            let falloff = (-3.0 * t * t).exp();
            Ring { expand: t * spread, alpha: (opacity * alpha_mul * falloff * 255.0).round() as u8 }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dashboard card of PowerAnalytics: strength 14, opacity 4 %, face at 70 % (`Transparency` 30).
    #[test]
    fn a_card_shadow_is_twenty_two_rings_fading_outward_to_a_dense_core() {
        let r = rings(14, 0.04, 0.7);
        assert_eq!(r.len(), 22, "ceil(14 × 1.5) = 21 rings → 22 with the core");
        assert!((r[0].expand - 14.0).abs() < 1e-6 && (r[21].expand).abs() < 1e-6, "outermost first, the core last");
        assert_eq!((r[0].alpha, r[21].alpha), (0, 7), "exp(−3) of 7.14 rounds to 0 at the rim; the core is 7");
        assert!(r.windows(2).all(|w| w[0].alpha <= w[1].alpha && w[0].expand >= w[1].expand), "fainter and wider outward");
    }

    /// The truncation is not small: a card's core ring is 7/255 of `#2A4A7A`, and the painter keeps
    /// `(1, 2, 3)` of the `(1.15, 2.03, 3.35)` that comes out — a blue that is 10 % short in every ring.
    #[test]
    fn a_ring_premultiplies_by_truncation_and_the_straight_colour_gives_it_back() {
        let ring = Ring { expand: 0.0, alpha: 7 };
        let colour = [0x2A, 0x4A, 0x7A];
        assert_eq!(ring.premultiplied(colour), [1, 2, 3]);
        assert_eq!(ring.straight(colour), [36, 73, 109], "what CSS must be given: 36 × 7/255 = 0.99 → 1, 73 → 2.00, 109 → 2.99 → 3");
        let back = ring.straight(colour).map(|c| (c as f32 * 7.0 / 255.0).round() as u8);
        assert_eq!(back, [1, 2, 3]);
        assert_eq!(Ring { expand: 3.0, alpha: 0 }.straight(colour), colour);
    }

    #[test]
    fn no_blur_is_one_ring_the_size_of_the_control_and_truncated() {
        assert_eq!(rings(0, 0.5, 1.0), vec![Ring { expand: 0.0, alpha: 127 }]);
    }

    #[test]
    fn a_hairline_blur_still_gets_its_floor_of_twelve_rings_and_a_dense_one_its_cap() {
        assert_eq!(ring_count(0.5, 0.0), 12);
        assert_eq!(ring_count(20.0, 255.0), 190, "geometry 30, alpha 189.3 → 190");
        assert_eq!(ring_count(200.0, 255.0), 192, "capped");
    }
}
