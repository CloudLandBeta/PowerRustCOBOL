// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Docking** (spec 056 §4.3, R11–R15).
//!
//! Docked controls are placed in render (z-) order, each claiming one edge of
//! what is left of the parent's client rectangle and shrinking it by what it
//! took. `Left`/`Right` span the remaining height at their designed width,
//! `Top`/`Bottom` the remaining width at their designed height, `Fill` the
//! whole remainder. A dock's thickness is its designed size (Q3).

use crate::layout::limits::clamp;
use crate::layout::props::{Dock, Limits};
use crate::layout::LRect;

/// One docked control, as the dock pass needs it.
#[derive(Clone, Copy, Debug)]
pub struct DockItem {
    pub dock: Dock,
    pub designed: LRect,
    pub width: Limits,
    pub height: Limits,
}

/// Place `items` (already in z-order) inside `client`; one rect per item.
pub fn place(items: &[DockItem], client: LRect) -> Vec<LRect> {
    let mut rem = client;
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        let r = match it.dock {
            Dock::Left | Dock::Right => {
                let w = clamp(it.designed.w, it.width);
                let h = clamp(rem.h, it.height);
                let x = if it.dock == Dock::Left { rem.x } else { rem.right() - w };
                if it.dock == Dock::Left {
                    rem.x += w;
                }
                rem.w = (rem.w - w).max(0.0);
                LRect::new(x, rem.y, w, h)
            }
            Dock::Top | Dock::Bottom => {
                let w = clamp(rem.w, it.width);
                let h = clamp(it.designed.h, it.height);
                let y = if it.dock == Dock::Top { rem.y } else { rem.bottom() - h };
                if it.dock == Dock::Top {
                    rem.y += h;
                }
                rem.h = (rem.h - h).max(0.0);
                LRect::new(rem.x, y, w, h)
            }
            Dock::Fill => {
                let r = LRect::new(rem.x, rem.y, clamp(rem.w, it.width), clamp(rem.h, it.height));
                rem.w = 0.0;
                rem.h = 0.0;
                r
            }
            Dock::None => it.designed,
        };
        out.push(r);
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::layout::test_support::*;
    use crate::model::{Control, ControlType, PropValue};

    fn docked(id: &str, dock: &str, r: (i32, i32, i32, i32), z: i32) -> Control {
        let mut c = with(ctrl(id, ControlType::Panel, r, None), "Dock", PropValue::String(dock.into()));
        c.z_order = z;
        c
    }

    /// AC6 — form 800×600 laid out at 1000×700.
    /// Top (h 50), then Left (w 200), then Fill:
    ///   Top  → (0, 0, 1000, 50);   remainder (0, 50, 1000, 650)
    ///   Left → (0, 50, 200, 650);  remainder (200, 50, 800, 650)
    ///   Fill → (200, 50, 800, 650)
    /// Left first, then Top, then Fill:
    ///   Left → (0, 0, 200, 700);   remainder (200, 0, 800, 700)
    ///   Top  → (200, 0, 800, 50);  remainder (200, 50, 800, 650)
    ///   Fill → (200, 50, 800, 650)
    /// An anchored `Bottom,Right` button designed at (700, 550, 80, 30) moves
    /// by the full client growth (+200, +100) — the docks do not affect it.
    #[test]
    fn dock_order_decides_who_gets_the_corner_and_anchors_see_the_full_client() {
        let btn = with(ctrl("B", ControlType::Button, (700, 550, 80, 30), None), "Anchor", PropValue::String("Bottom,Right".into()));
        let top_first = vec![
            docked("T", "Top", (0, 0, 10, 50), 1),
            docked("L", "Left", (0, 0, 200, 10), 2),
            docked("F", "Fill", (0, 0, 10, 10), 3),
            btn.clone(),
        ];
        let o = solve_at(&top_first, (800.0, 600.0), (1000.0, 700.0));
        assert_eq!(r(&o, "T"), (0.0, 0.0, 1000.0, 50.0));
        assert_eq!(r(&o, "L"), (0.0, 50.0, 200.0, 650.0));
        assert_eq!(r(&o, "F"), (200.0, 50.0, 800.0, 650.0));
        assert_eq!(r(&o, "B"), (900.0, 650.0, 80.0, 30.0));

        let left_first = vec![
            docked("L", "Left", (0, 0, 200, 10), 1),
            docked("T", "Top", (0, 0, 10, 50), 2),
            docked("F", "Fill", (0, 0, 10, 10), 3),
            btn,
        ];
        let o2 = solve_at(&left_first, (800.0, 600.0), (1000.0, 700.0));
        assert_eq!(r(&o2, "L"), (0.0, 0.0, 200.0, 700.0));
        assert_eq!(r(&o2, "T"), (200.0, 0.0, 800.0, 50.0));
        assert_eq!(r(&o2, "F"), (200.0, 50.0, 800.0, 650.0));
        assert_eq!(r(&o2, "B"), (900.0, 650.0, 80.0, 30.0));
        println!("dock: Top/Left/Fill and Left/Top/Fill each placed as derived; the anchored button ignores both");
    }

    /// Right and Bottom take the far edges.
    /// 600×400: Right (w 150) → (450, 0, 150, 400); Bottom (h 40) → (0, 360, 450, 40).
    #[test]
    fn right_and_bottom_take_the_far_edges() {
        let o = solve_at(
            &[docked("R", "Right", (0, 0, 150, 10), 1), docked("B", "Bottom", (0, 0, 10, 40), 2)],
            (600.0, 400.0),
            (600.0, 400.0),
        );
        assert_eq!(r(&o, "R"), (450.0, 0.0, 150.0, 400.0));
        assert_eq!(r(&o, "B"), (0.0, 360.0, 450.0, 40.0));
    }

    /// AC7 — `Fill` with `MinHeight` 500 in a 300-high remainder is 500 high.
    #[test]
    fn a_fill_with_min_height_is_never_shorter() {
        let f = with(docked("F", "Fill", (0, 0, 10, 10), 1), "MinHeight", PropValue::Int(500));
        let bag = std::collections::BTreeMap::from([("MinFormHeight".to_owned(), PropValue::Int(1))]);
        let controls = [f];
        let input = crate::layout::LayoutInput::new(&controls, (400.0, 300.0), (400.0, 300.0), &bag, &[]);
        let o = crate::layout::solve(&input);
        assert!(o.rects["F"].h >= 500.0, "Fill kept its MinHeight: {:?}", o.rects["F"]);
    }
}
