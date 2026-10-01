// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Obsolete scaling style** (spec 081).
//!
//! A form whose `ObsoleteScalingStyle` is not 0 follows its window the way a
//! PowerCOBOL form did: every control's position and/or size is its designed
//! value times the window ratio, and its font follows the smaller of the two
//! ratios. It is a compatibility mode for migrated forms — anchors, docking
//! and containers remain the way to build a new one — and it is a mode of
//! [`super::solve`], not a second layout path, so every surface scales alike.
//!
//! A control the developer anchored or docked on purpose keeps doing so
//! (operator, 2026-10-01). Every control stores an `Anchor`, so "on purpose"
//! is read as "not the type's default": a migrated form, whose controls all
//! carry the default, scales whole.

use crate::layout::defaults::{self, SCALING_FONT, SCALING_REPOSITION, SCALING_RESIZE, SCALING_STYLE_MAX};
use crate::layout::limits::clamp;
use crate::layout::props::{self, Dock, Edges, Limits, PropSource};
use crate::layout::LRect;
use crate::model::Control;

/// The form's `ObsoleteScalingStyle`, read as flags; anything outside 0–7 is 0.
pub fn style(form: &dyn PropSource) -> i64 {
    let v = form.number("ObsoleteScalingStyle");
    if v.fract() != 0.0 || v < 0.0 || v > SCALING_STYLE_MAX as f32 {
        return 0;
    }
    v as i64
}

pub fn resizes(style: i64) -> bool {
    style & SCALING_RESIZE != 0
}
pub fn repositions(style: i64) -> bool {
    style & SCALING_REPOSITION != 0
}
pub fn scales_font(style: i64) -> bool {
    style & SCALING_FONT != 0
}

/// A control that keeps spec-056 placement on a scaling form: anchored to
/// other edges than its type's default, or docked.
pub fn opted_out(c: &Control) -> bool {
    props::dock(c) != Dock::None || props::anchor(c) != Edges::parse(defaults::anchor_default(&c.control_type))
}

/// `laid / designed`, or 1 for a designed extent of 0.
pub fn ratio(designed: f32, laid: f32) -> f32 {
    if designed > 0.0 {
        laid / designed
    } else {
        1.0
    }
}

/// Place a scaled control: `designed` inside `designed_parent`, laid out
/// inside `parent`.
pub fn place(designed: LRect, designed_parent: LRect, parent: LRect, style: i64, width: Limits, height: Limits) -> LRect {
    let rx = ratio(designed_parent.w, parent.w);
    let ry = ratio(designed_parent.h, parent.h);
    let (ox, oy) = (designed.x - designed_parent.x, designed.y - designed_parent.y);
    let (x, y) = if repositions(style) { (ox * rx, oy * ry) } else { (ox, oy) };
    let (w, h) = if resizes(style) {
        (clamp(designed.w * rx, width), clamp(designed.h * ry, height))
    } else {
        (designed.w, designed.h)
    };
    LRect::new(parent.x + x, parent.y + y, w, h)
}

/// The designed rectangle that [`place`] lays out to `target`.
pub fn designed_rect(target: LRect, designed_parent: LRect, parent: LRect, style: i64) -> LRect {
    let rx = ratio(designed_parent.w, parent.w);
    let ry = ratio(designed_parent.h, parent.h);
    let (ox, oy) = (target.x - parent.x, target.y - parent.y);
    let (x, y) = if repositions(style) { (ox / rx, oy / ry) } else { (ox, oy) };
    let (w, h) = if resizes(style) { (target.w / rx, target.h / ry) } else { (target.w, target.h) };
    LRect::new(designed_parent.x + x, designed_parent.y + y, w, h)
}

/// The font factor of a scaling form: `min(rx, ry)` between the form's
/// `MinFontScale` and `MaxFontScale`, times the system text factor. A factor
/// pinned from COBOL wins.
pub fn font_factor(form: &dyn PropSource, designed: (f32, f32), available: (f32, f32), system: f32, pinned: Option<f32>) -> f32 {
    if let Some(p) = pinned.filter(|p| *p > 0.0) {
        return p;
    }
    let lo = form.number("MinFontScale");
    let hi = form.number("MaxFontScale");
    let r = ratio(designed.0, available.0).min(ratio(designed.1, available.1));
    r.max(lo).min(hi.max(lo)) * system
}

#[cfg(test)]
mod tests {
    use crate::layout::test_support::*;
    use crate::layout::{solve, LayoutInput, LayoutOutput, LRect};
    use crate::model::{Control, ControlType, PropValue};
    use std::collections::BTreeMap;

    /// The spec's fixture: a form designed 400 × 300 with a Button at
    /// (100, 50, 80, 30), FontSize 12, shown at 800 × 450 (rx 2, ry 1.5).
    fn button() -> Control {
        with(ctrl("B", ControlType::Button, (100, 50, 80, 30), None), "FontSize", PropValue::Int(12))
    }

    fn at(controls: &[Control], style: i64, avail: (f32, f32)) -> LayoutOutput {
        let bag = BTreeMap::from([("ObsoleteScalingStyle".to_owned(), PropValue::Int(style))]);
        solve(&LayoutInput::new(controls, (400.0, 300.0), avail, &bag, &[]))
    }

    fn rect(o: &LayoutOutput, id: &str) -> (f32, f32, f32, f32) {
        let r = o.rects[id];
        (r.x, r.y, r.w, r.h)
    }

    /// AC2 — resize, reposition, both.
    #[test]
    fn each_style_scales_the_button_by_the_window_ratio() {
        let b = [button()];
        assert_eq!(rect(&at(&b, 1, (800.0, 450.0)), "B"), (100.0, 50.0, 160.0, 45.0));
        assert_eq!(rect(&at(&b, 2, (800.0, 450.0)), "B"), (200.0, 75.0, 80.0, 30.0));
        assert_eq!(rect(&at(&b, 3, (800.0, 450.0)), "B"), (200.0, 75.0, 160.0, 45.0));
        assert_eq!(rect(&at(&b, 0, (800.0, 450.0)), "B"), (100.0, 50.0, 80.0, 30.0), "0 = spec 056, unchanged");
        println!("400x300 → 800x450, Button (100,50,80,30): 1 → (100,50,160,45)  2 → (200,75,80,30)  3 → (200,75,160,45)");
    }

    /// AC3 — the font follows min(rx, ry), bounded by MaxFontScale, unless
    /// `ScaleFont` is off.
    #[test]
    fn the_font_follows_the_smaller_ratio() {
        let b = [button()];
        assert_eq!(at(&b, 5, (800.0, 450.0)).font_sizes["B"], 18.0, "12 × min(2, 1.5)");
        assert_eq!(at(&b, 7, (800.0, 450.0)).font_sizes["B"], 18.0);
        assert_eq!(at(&b, 3, (800.0, 450.0)).font_sizes["B"], 12.0, "no font flag");
        assert_eq!(at(&b, 4, (2000.0, 3000.0)).font_sizes["B"], 18.0, "clamped to MaxFontScale 1.5");
        let fixed = [with(button(), "ScaleFont", PropValue::Bool(false))];
        assert_eq!(at(&fixed, 7, (800.0, 450.0)).font_sizes["B"], 12.0);
    }

    /// AC4 — an anchor the developer chose keeps anchoring; the default
    /// anchor scales.
    #[test]
    fn a_chosen_anchor_opts_the_control_out() {
        let anchored = [with(button(), "Anchor", PropValue::String("Top,Right".into()))];
        // Right-anchored: keeps its 220 px right margin.
        assert_eq!(rect(&at(&anchored, 3, (800.0, 450.0)), "B"), (500.0, 50.0, 80.0, 30.0));
        let docked = [with(button(), "Dock", PropValue::String("Top".into()))];
        assert_eq!(at(&docked, 3, (800.0, 450.0)).rects["B"].w, 800.0, "a docked control stays docked");
    }

    /// AC5 — size limits still hold.
    #[test]
    fn min_width_holds_when_scaling_down() {
        let b = [with(button(), "MinWidth", PropValue::Int(100))];
        assert_eq!(at(&b, 1, (200.0, 150.0)).rects["B"].w, 100.0, "not 40");
    }

    /// AC6 — a container's children scale within its scaled client. A Panel
    /// keeps a 2 px border inset, so its client goes from (42, 42, 196, 96)
    /// designed to (82, 62, 396, 146) at 800 × 450, and the child scales by
    /// those ratios, not the form's.
    #[test]
    fn children_scale_within_their_scaled_container() {
        let panel = ctrl("P", ControlType::Panel, (40, 40, 200, 100), None);
        let tb = ctrl("T", ControlType::TextBox, (60, 60, 100, 20), Some("P"));
        let o = at(&[panel, tb], 3, (800.0, 450.0));
        assert_eq!(rect(&o, "P"), (80.0, 60.0, 400.0, 150.0));
        let (rx, ry) = (396.0 / 196.0, 146.0 / 96.0);
        let want = (82.0 + 18.0 * rx, 62.0 + 18.0 * ry, 100.0 * rx, 20.0 * ry);
        let got = rect(&o, "T");
        for (g, w) in [(got.0, want.0), (got.1, want.1), (got.2, want.2), (got.3, want.3)] {
            assert!((g - w).abs() < 1e-3, "{got:?} vs {want:?}");
        }
    }

    /// AC7 — always from the design: out and back is exact.
    #[test]
    fn resizing_out_and_back_returns_the_design() {
        let b = [button()];
        let there = at(&b, 3, (801.0, 299.0));
        assert_ne!(there.rects["B"], LRect::new(100.0, 50.0, 80.0, 30.0));
        assert_eq!(rect(&at(&b, 3, (400.0, 300.0)), "B"), (100.0, 50.0, 80.0, 30.0));
    }

    /// The inverse: an on-screen edit at a scaled size maps back to the
    /// design that lays out to it.
    #[test]
    fn an_on_screen_edit_maps_back_through_the_scale() {
        let b = [button()];
        let o = at(&b, 3, (800.0, 450.0));
        let want = LRect::new(220.0, 90.0, 180.0, 60.0);
        let back = crate::layout::inverse::designed_rect(&o.placement["B"], want, LRect::new(100.0, 50.0, 80.0, 30.0));
        assert_eq!((back.x, back.y, back.w, back.h), (110.0, 60.0, 90.0, 40.0));
    }
}
