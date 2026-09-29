// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Anchoring** (spec 056 §4.2, R6–R10).
//!
//! Each axis is placed on its own, from the DESIGNED rectangle and the
//! DESIGNED parent, by how much the parent grew (`delta`, negative when it
//! shrank). Every rule is written as "designed offset + a share of `delta`",
//! so at the designed size the share is exactly zero and the control lands
//! exactly where it was designed (R5) — no division is ever undone.

use crate::layout::limits::clamp;
use crate::layout::props::{Edges, Limits};
use crate::layout::{AxisPlace, LRect};

/// Place one axis. `pos`/`size` are the control's designed position and size;
/// `dp`/`ds` the designed parent's start and extent; `lp`/`ls` the laid-out
/// parent's. Returns the laid-out position and size.
pub fn place_axis(
    pos: f32,
    size: f32,
    (dp, ds): (f32, f32),
    (lp, ls): (f32, f32),
    how: AxisPlace,
    lim: Limits,
) -> (f32, f32) {
    let offset = pos - dp;
    let delta = ls - ds;
    match how {
        AxisPlace::Lead => (lp + offset, size),
        AxisPlace::Trail => (lp + offset + delta, size),
        // Both offsets kept, so the size takes the whole change; a clamped
        // size stays attached to the leading edge (R17).
        AxisPlace::Stretch => (lp + offset, clamp(size + delta, lim)),
        // The centre keeps its fraction of the parent's extent.
        AxisPlace::Proportional => {
            let shift = if ds > 0.0 {
                (offset + size * 0.5) * delta / ds
            } else {
                0.0
            };
            (lp + offset + shift, size)
        }
    }
}

/// Which rule an axis follows, from whether its leading and trailing edges are
/// anchored.
pub fn axis_place(lead: bool, trail: bool) -> AxisPlace {
    match (lead, trail) {
        (true, false) => AxisPlace::Lead,
        (false, true) => AxisPlace::Trail,
        (true, true) => AxisPlace::Stretch,
        (false, false) => AxisPlace::Proportional,
    }
}

/// Place an anchored control: `designed` inside `designed_parent`, laid out
/// inside `parent` (R8).
pub fn place(
    designed: LRect,
    designed_parent: LRect,
    parent: LRect,
    edges: Edges,
    width: Limits,
    height: Limits,
) -> (LRect, AxisPlace, AxisPlace) {
    let hx = axis_place(edges.left, edges.right);
    let hy = axis_place(edges.top, edges.bottom);
    let (x, w) = place_axis(
        designed.x,
        designed.w,
        (designed_parent.x, designed_parent.w),
        (parent.x, parent.w),
        hx,
        width,
    );
    let (y, h) = place_axis(
        designed.y,
        designed.h,
        (designed_parent.y, designed_parent.h),
        (parent.y, parent.h),
        hy,
        height,
    );
    (LRect::new(x, y, w, h), hx, hy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::test_support::*;
    use crate::model::{ControlType, PropValue};

    /// One control at (100, 50, 80, 30) in a 400×300 form, every one of the
    /// sixteen edge sets, at 600×450 (grown by 200×150) and 300×200 (shrunk
    /// by 100×100).
    ///
    /// Per axis, with offset o, size s, parent grown by d, designed extent D:
    ///   Lead          → (o, s)
    ///   Trail         → (o + d, s)
    ///   Stretch       → (o, s + d)
    ///   Proportional  → (o + (o + s/2)·d/D, s)
    /// X: o=100, s=80, D=400 — centre 140, so proportional shifts 0.35·d.
    ///   d=+200: Lead 100 | Trail 300 | Stretch 100 w280 | Prop 170
    ///   d=−100: Lead 100 | Trail 0   | Stretch 100 w−20→0 | Prop 65
    /// Y: o=50, s=30, D=300 — centre 65, proportional shifts 65/300·d.
    ///   d=+150: Lead 50 | Trail 200 | Stretch 50 h180 | Prop 82.5
    ///   d=−100: Lead 50 | Trail −50 | Stretch 50 h−70→0 | Prop 28.333…
    #[test]
    fn all_sixteen_anchor_combinations_at_a_larger_and_a_smaller_surface() {
        let x_rule = |left: bool, right: bool, grown: bool| -> (f32, f32) {
            match (left, right, grown) {
                (true, false, _) => (100.0, 80.0),
                (false, true, true) => (300.0, 80.0),
                (false, true, false) => (0.0, 80.0),
                (true, true, true) => (100.0, 280.0),
                (true, true, false) => (100.0, 0.0),
                (false, false, true) => (170.0, 80.0),
                (false, false, false) => (65.0, 80.0),
            }
        };
        let y_rule = |top: bool, bottom: bool, grown: bool| -> (f32, f32) {
            match (top, bottom, grown) {
                (true, false, _) => (50.0, 30.0),
                (false, true, true) => (200.0, 30.0),
                (false, true, false) => (-50.0, 30.0),
                (true, true, true) => (50.0, 180.0),
                (true, true, false) => (50.0, 0.0),
                (false, false, true) => (82.5, 30.0),
                (false, false, false) => (50.0 - 65.0 / 3.0, 30.0),
            }
        };
        let mut checked = 0;
        for bits in 0..16u8 {
            let e = Edges {
                top: bits & 1 != 0,
                bottom: bits & 2 != 0,
                left: bits & 4 != 0,
                right: bits & 8 != 0,
            };
            for (grown, avail) in [(true, (600.0, 450.0)), (false, (300.0, 200.0))] {
                let controls = vec![with(
                    ctrl("C", ControlType::Button, (100, 50, 80, 30), None),
                    "Anchor",
                    PropValue::String(e.to_text()),
                )];
                let o = solve_at(&controls, (400.0, 300.0), avail);
                let (x, y, w, h) = r(&o, "C");
                let (ex, ew) = x_rule(e.left, e.right, grown);
                let (ey, eh) = y_rule(e.top, e.bottom, grown);
                let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
                assert!(
                    close(x, ex) && close(w, ew) && close(y, ey) && close(h, eh),
                    "Anchor '{}' at {avail:?}: got ({x}, {y}, {w}, {h}), expected ({ex}, {ey}, {ew}, {eh})",
                    e.to_text()
                );
                checked += 1;
            }
        }
        println!("anchoring: {checked} cases (16 edge sets × larger/smaller surface), every rect as derived");
    }

    #[test]
    fn at_the_designed_size_every_edge_set_lands_on_the_design() {
        for bits in 0..16u8 {
            let e = Edges {
                top: bits & 1 != 0,
                bottom: bits & 2 != 0,
                left: bits & 4 != 0,
                right: bits & 8 != 0,
            };
            let controls = vec![with(
                ctrl("C", ControlType::Button, (37, 91, 55, 23), None),
                "Anchor",
                PropValue::String(e.to_text()),
            )];
            let o = solve_at(&controls, (333.0, 217.0), (333.0, 217.0));
            assert_eq!(r(&o, "C"), (37.0, 91.0, 55.0, 23.0), "'{}'", e.to_text());
        }
    }

    /// AC5 — laying out the output again at the same size changes nothing,
    /// and a size reached through another size equals the same size cold.
    #[test]
    fn layout_is_idempotent_and_cold_equals_warm() {
        let edge_sets = ["Top,Left", "Bottom,Right", "Top,Bottom,Left,Right", "", "Left,Right"];
        let mut cases = 0;
        for anchors in edge_sets {
            let designed = vec![with(
                ctrl("C", ControlType::Button, (120, 40, 90, 32), None),
                "Anchor",
                PropValue::String(anchors.into()),
            )];
            for avail in [(640.0, 480.0), (260.0, 180.0)] {
                let first = solve_at(&designed, (400.0, 300.0), avail);
                // The output, taken as a design at that size.
                let (x, y, w, h) = r(&first, "C");
                let mut again = designed.clone();
                again[0].rect = crate::model::Rect::new(x as i32, y as i32, w as i32, h as i32);
                let whole = x.fract() == 0.0 && y.fract() == 0.0 && w.fract() == 0.0 && h.fract() == 0.0;
                if whole {
                    let second = solve_at(&again, avail, avail);
                    assert_eq!(r(&second, "C"), (x, y, w, h), "idempotent for '{anchors}' at {avail:?}");
                }
                // Warm (via another size) equals cold: the solver keeps no state.
                let _ = solve_at(&designed, (400.0, 300.0), (999.0, 777.0));
                let cold = solve_at(&designed, (400.0, 300.0), avail);
                assert_eq!(first.rects, cold.rects);
                cases += 1;
            }
        }
        println!("idempotence and cold = warm: {cases} fixtures");
    }
}
