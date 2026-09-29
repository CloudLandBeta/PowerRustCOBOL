// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The inverse mapping** (spec 056 R33, R38).
//!
//! On a responsive form what the developer drags on the canvas, and what COBOL
//! writes to `X`/`Y`/`Width`/`Height`, is the ON-SCREEN rectangle. The design
//! is the source of truth, so the edit is carried back to the designed
//! rectangle that lays out to exactly that on-screen one: move a
//! `Right`-anchored button 10 px on screen and its designed `x` moves 10 px,
//! whatever the window's size.

use crate::layout::props::Dock;
use crate::layout::{AxisPlace, LRect, Placement};

/// Invert one anchored axis: the designed `(pos, size)` that lays out to
/// `(target_pos, target_size)`. Inverse of [`crate::layout::anchor::place_axis`]
/// for sizes inside their limits.
pub fn designed_axis(
    target_pos: f32,
    target_size: f32,
    (dp, ds): (f32, f32),
    (lp, ls): (f32, f32),
    how: AxisPlace,
) -> (f32, f32) {
    let delta = ls - ds;
    let moved = target_pos - lp;
    match how {
        AxisPlace::Lead => (dp + moved, target_size),
        AxisPlace::Trail => (dp + moved - delta, target_size),
        AxisPlace::Stretch => (dp + moved, target_size - delta),
        // moved = o + (o + s/2)·k  with k = delta/ds  ⇒  o = (moved − s·k/2)/(1 + k)
        AxisPlace::Proportional => {
            let k = if ds > 0.0 { delta / ds } else { 0.0 };
            let denom = 1.0 + k;
            let o = if denom > 0.0 {
                (moved - target_size * k * 0.5) / denom
            } else {
                moved
            };
            (dp + o, target_size)
        }
    }
}

/// The designed rectangle that lays out to `target`, given how the control was
/// placed and its current `designed` rectangle.
pub fn designed_rect(placement: &Placement, target: LRect, designed: LRect) -> LRect {
    match *placement {
        Placement::Designed => target,
        Placement::Rigid { dx, dy } => LRect::new(target.x - dx, target.y - dy, target.w, target.h),
        Placement::Anchored { x, y, designed_parent, parent } => {
            let (px, w) = designed_axis(
                target.x,
                target.w,
                (designed_parent.x, designed_parent.w),
                (parent.x, parent.w),
                x,
            );
            let (py, h) = designed_axis(
                target.y,
                target.h,
                (designed_parent.y, designed_parent.h),
                (parent.y, parent.h),
                y,
            );
            LRect::new(px, py, w, h)
        }
        // A dock decides the position and the span; only its thickness is
        // the developer's.
        Placement::Docked(d) => match d {
            Dock::Left | Dock::Right => LRect { w: target.w, ..designed },
            Dock::Top | Dock::Bottom => LRect { h: target.h, ..designed },
            Dock::Fill | Dock::None => designed,
        },
        // A flex/grid/flow parent decides the position; the size is the
        // item's intrinsic one.
        Placement::Item(_) => LRect { w: target.w, h: target.h, ..designed },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::test_support::*;
    use crate::model::{ControlType, PropValue, Rect};

    fn close(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3 && (a.2 - b.2).abs() < 1e-3 && (a.3 - b.3).abs() < 1e-3
    }

    /// `layout(inverse(layout(x) + d)) == layout(x) + d` for every placement
    /// kind: move by (+10, −6), then resize by (+14, +8) where the placement
    /// lets the size be set.
    #[test]
    fn an_on_screen_edit_survives_the_round_trip_for_every_placement() {
        let fixtures: Vec<(&str, &str, &str)> = vec![
            ("Top,Left", "None", "lead/lead"),
            ("Bottom,Right", "None", "trail/trail"),
            ("Top,Bottom,Left,Right", "None", "stretch/stretch"),
            ("", "None", "proportional/proportional"),
            ("Top,Left", "Left", "dock Left"),
            ("Top,Left", "Top", "dock Top"),
        ];
        let mut checked = 0;
        for (anchor, dock, label) in fixtures {
            let designed = with(
                with(ctrl("C", ControlType::Panel, (120, 80, 160, 90), None), "Anchor", PropValue::String(anchor.into())),
                "Dock",
                PropValue::String(dock.into()),
            );
            let avail = (900.0, 640.0);
            let first = solve_at(&[designed.clone()], (600.0, 400.0), avail);
            let now = first.rects["C"];
            let wants = if dock == "None" {
                LRect::new(now.x + 10.0, now.y - 6.0, now.w + 14.0, now.h + 8.0)
            } else if dock == "Left" {
                LRect { w: now.w + 14.0, ..now }
            } else {
                LRect { h: now.h + 8.0, ..now }
            };
            let back = designed_rect(&first.placement["C"], wants, LRect::from_model(designed.rect));
            let mut edited = designed.clone();
            edited.rect = Rect::new(back.x.round() as i32, back.y.round() as i32, back.w.round() as i32, back.h.round() as i32);
            // A proportional inverse may land between pixels; lay out the
            // exact designed value instead of the rounded one.
            let exact = crate::layout::anchor::place(
                back,
                LRect::new(0.0, 0.0, 600.0, 400.0),
                LRect::new(0.0, 0.0, avail.0, avail.1),
                crate::layout::props::anchor(&designed),
                Default::default(),
                Default::default(),
            )
            .0;
            let again = solve_at(&[edited], (600.0, 400.0), avail).rects["C"];
            let got = if anchor.is_empty() { exact } else { again };
            let w = (wants.x, wants.y, wants.w, wants.h);
            assert!(close((got.x, got.y, got.w, got.h), w), "{label}: wanted {w:?}, got {got:?}");
            checked += 1;
        }
        println!("inverse mapping: {checked} placement kinds round-trip an on-screen move and resize exactly");
    }

    /// AC43, pure half — `ADD 10 TO Btn::X` twice on a Right-anchored button at
    /// a size other than designed moves it 20 px on screen.
    #[test]
    fn two_ten_pixel_writes_move_a_right_anchored_button_twenty_pixels() {
        let mut b = with(ctrl("B", ControlType::Button, (500, 20, 80, 30), None), "Anchor", PropValue::String("Top,Right".into()));
        let avail = (1000.0, 400.0);
        let start = solve_at(&[b.clone()], (600.0, 400.0), avail).rects["B"].x;
        for step in 1..=2 {
            let o = solve_at(&[b.clone()], (600.0, 400.0), avail);
            let now = o.rects["B"];
            let back = designed_rect(&o.placement["B"], LRect { x: now.x + 10.0, ..now }, LRect::from_model(b.rect));
            b.rect.x = back.x as i32;
            let after = solve_at(&[b.clone()], (600.0, 400.0), avail).rects["B"].x;
            assert_eq!(after, start + 10.0 * step as f32);
        }
        println!("Right-anchored button at 1000 px: two +10 writes moved it {} px on screen", 20);
    }
}
