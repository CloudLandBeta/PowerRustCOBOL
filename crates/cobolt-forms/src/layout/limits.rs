// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Size limits** (spec 056 §4.4, R16–R17).

use crate::layout::props::Limits;

/// Clamp a stretched, docked, grown, shrunk or grid-sized dimension to
/// `[Min, Max]` — a zero bound is no bound — and never below zero. When both
/// bounds apply and cross, the minimum wins, as in CSS.
pub fn clamp(v: f32, lim: Limits) -> f32 {
    let mut v = v;
    if lim.max > 0.0 && v > lim.max {
        v = lim.max;
    }
    if lim.min > 0.0 && v < lim.min {
        v = lim.min;
    }
    v.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::test_support::*;
    use crate::model::{ControlType, PropValue};

    #[test]
    fn zero_is_no_bound_and_min_beats_max() {
        let none = Limits::default();
        assert_eq!(clamp(500.0, none), 500.0);
        assert_eq!(clamp(-5.0, none), 0.0);
        assert_eq!(clamp(500.0, Limits { min: 0.0, max: 300.0 }), 300.0);
        assert_eq!(clamp(10.0, Limits { min: 40.0, max: 0.0 }), 40.0);
        assert_eq!(clamp(10.0, Limits { min: 40.0, max: 20.0 }), 40.0);
    }

    /// AC7 — a `Left,Right` control that hits `MaxWidth` stays on `Left`.
    /// Designed (20, 10, 200, 30) in 400 wide; at 800 the stretch asks for
    /// 600 and `MaxWidth` 350 caps it: x stays 20, w 350.
    #[test]
    fn a_stretched_control_at_its_max_width_stays_attached_to_left() {
        let c = with(
            with(ctrl("C", ControlType::TextBox, (20, 10, 200, 30), None), "Anchor", PropValue::String("Top,Left,Right".into())),
            "MaxWidth",
            PropValue::Int(350),
        );
        let o = solve_at(&[c], (400.0, 300.0), (800.0, 300.0));
        assert_eq!(r(&o, "C"), (20.0, 10.0, 350.0, 30.0));
        println!("Left,Right + MaxWidth 350 at 800 px: {:?}", r(&o, "C"));
    }
}
