// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 097 — **`PerspectiveStrength` scales the convergence of a tilt and nothing else.**
//!
//! `PerspectiveDistance` (spec 096) sets both how large a control is drawn and how strongly a tilt
//! converges, so the strength could not be lowered while the size was kept. `PerspectiveStrength`
//! multiplies the depth term of the projection by `strength / 100`. What is asserted here is what
//! the spec promises, in numbers:
//!
//! * the far/near edge ratio changes strictly with the strength, from "equal" at 0 to spec 096's
//!   0.778 at 100 (R3);
//! * the centre never moves and an untilted control never changes size (R2);
//! * at 100 the projection is the one spec 096 made, to a ten-thousandth of a pixel (R4);
//! * for every tilt, distance and strength the outline is convex and nothing is behind the camera (R6);
//! * the property is read, clamped on read, and ignored when nothing is tilted.
//!
//! Spec 097 AC2, AC3, AC4, AC6.

#![cfg(feature = "render")]

use cobolt_forms::model::{Control, ControlType, PropValue};
use cobolt_forms::perspective::{edge_lengths, quad_is_convex, Camera, Perspective, CAMERA_K};
use egui::{pos2, Pos2, Rect};

fn card() -> Rect {
    Rect::from_min_size(pos2(100.0, 60.0), egui::vec2(320.0, 220.0))
}

fn quad(rx: i64, ry: i64, d: i64, s: i64) -> [Pos2; 4] {
    Perspective::new(rx, ry, d).with_strength(s).quad(card())
}

const STRENGTHS: [i64; 5] = [0, 25, 50, 75, 100];

#[test]
fn the_far_edge_is_shorter_by_a_strictly_larger_margin_as_the_strength_grows() {
    println!("\n  ── RotateY −30, distance 50, a 320 × 220 card ──");
    println!("  strength   near edge   far edge   near/far   outline width");
    let mut ratios = Vec::new();
    for s in STRENGTHS {
        let q = quad(0, -30, 50, s);
        let e = edge_lengths(&q);
        let (near, far) = (e[1], e[3]); // −30: the left recedes, the right is nearer
        let width = q.iter().map(|p| p.x).fold(f32::MIN, f32::max) - q.iter().map(|p| p.x).fold(f32::MAX, f32::min);
        println!("  {s:>8}   {near:>9.2}   {far:>8.2}   {:>8.4}   {width:>13.2}", near / far);
        ratios.push(near / far);
    }
    // At 0 the opposite edges are equal: a parallel projection, no convergence.
    assert!((ratios[0] - 1.0).abs() < 1e-4, "at strength 0 the edges must be equal, got {}", ratios[0]);
    // Strictly more convergence at every step up.
    assert!(ratios.windows(2).all(|w| w[0] < w[1]), "the ratio must rise with the strength: {ratios:?}");
    // At 100 it is the 0.778 of spec 096 (far/near), i.e. near/far = 1.2857.
    assert!((ratios[4] - 1.2857).abs() < 5e-4, "strength 100 must give spec 096's ratio, got {}", ratios[4]);
}

#[test]
fn the_other_axis_and_both_axes_behave_the_same_way() {
    for (rx, ry) in [(30, 0), (-30, 0), (30, 30), (-45, 20)] {
        let spread: Vec<f32> = STRENGTHS
            .iter()
            .map(|&s| {
                let e = edge_lengths(&quad(rx, ry, 60, s));
                // How unequal the opposite edges are: both pairs.
                (e[0] - e[2]).abs() + (e[1] - e[3]).abs()
            })
            .collect();
        println!("  RotateX {rx:>3}, RotateY {ry:>3}: inequality of opposite edges over strengths {STRENGTHS:?} = {spread:.2?}");
        assert!(spread[0] < 1e-3, "at strength 0 opposite edges are equal: {spread:?}");
        assert!(spread.windows(2).all(|w| w[0] < w[1]), "convergence rises with the strength: {spread:?}");
    }
}

#[test]
fn the_strength_never_moves_the_centre_and_never_resizes_a_flat_control() {
    let c = card();
    let mut worst_centre = 0.0f32;
    for rx in (-60..=60).step_by(20) {
        for ry in (-60..=60).step_by(20) {
            for d in [0, 50, 100] {
                for s in STRENGTHS {
                    let cam = Perspective::new(rx, ry, d).with_strength(s).camera(c);
                    worst_centre = worst_centre.max((cam.project(c.center()) - c.center()).length());
                }
            }
        }
    }
    println!("\n  centre fixed over 7 × 7 × 3 × 5 combinations: worst drift {worst_centre:e} px");
    assert!(worst_centre < 1e-3);

    // A control that is not tilted: its outline is the same at every strength, for every distance.
    println!("  untilted outline width by distance and strength:");
    for d in [0i64, 25, 50, 75, 100] {
        let base = quad(0, 0, d, 100);
        for s in STRENGTHS {
            let q = quad(0, 0, d, s);
            for (a, b) in q.iter().zip(&base) {
                assert!((*a - *b).length() < 1e-4, "an untilted control changed with strength {s} at distance {d}");
            }
        }
        println!("    distance {d:>3}: {:.2} px at every strength (designed 320)", edge_lengths(&base)[0]);
    }
    // And the distance still doubles it: the size is the distance's.
    assert!((edge_lengths(&quad(0, 0, 100, 0))[0] - 640.0).abs() < 1e-2);
    assert!((edge_lengths(&quad(0, 0, 0, 0))[0] - 160.0).abs() < 1e-2);
}

/// The projection of spec 096, written out as it was before this property existed.
fn the_projection_of_spec_096(rx: i64, ry: i64, d: i64, rect: Rect, p: Pos2) -> Pos2 {
    let m = rect.width().max(rect.height()).max(1.0);
    let reference = CAMERA_K * m;
    let depth = reference * 2f32.powf((50.0 - d as f32) / 50.0);
    let (sx, cx) = (rx as f32).to_radians().sin_cos();
    let (sy, cy) = (ry as f32).to_radians().sin_cos();
    let c = rect.center();
    let (x, y) = (p.x - c.x, p.y - c.y);
    let y1 = y * cx;
    let z1 = -y * sx;
    let x2 = x * cy - z1 * sy;
    let z2 = z1 * cy + x * sy;
    let k = reference / (depth + z2);
    pos2(c.x + x2 * k, c.y + y1 * k)
}

#[test]
fn at_strength_100_the_projection_is_exactly_the_one_before_this_property() {
    let c = card();
    let mut worst = 0.0f32;
    let mut n = 0usize;
    for rx in (-60..=60).step_by(3) {
        for ry in (-60..=60).step_by(3) {
            for d in (0..=100).step_by(10) {
                let cam = Perspective::new(rx, ry, d).camera(c); // strength defaults to 100
                for corner in [c.left_top(), c.right_top(), c.right_bottom(), c.left_bottom(), c.center(), pos2(c.left() + 37.0, c.top() + 91.0)] {
                    worst = worst.max((cam.project(corner) - the_projection_of_spec_096(rx, ry, d, c, corner)).length());
                    n += 1;
                }
            }
        }
    }
    println!("\n  {n} projected points at strength 100 against the spec 096 formula: worst difference {worst:e} px");
    assert!(worst < 1e-4, "strength 100 moved a point by {worst} px");
}

#[test]
fn nothing_is_behind_the_camera_and_every_outline_is_convex_at_every_strength() {
    let shapes = [("square", 200.0, 200.0), ("wide", 320.0, 220.0), ("very wide", 800.0, 40.0), ("tall", 40.0, 600.0)];
    println!("\n  {:<10} {:>10} {:>16} {:>9}", "shape", "combos", "min divisor/M", "convex");
    let mut total = 0usize;
    for (name, w, h) in shapes {
        let rect = Rect::from_min_size(pos2(50.0, 50.0), egui::vec2(w, h));
        let m = w.max(h);
        let (mut min_div, mut combos, mut convex) = (f32::MAX, 0usize, 0usize);
        for rx in (-60..=60).step_by(2) {
            for ry in (-60..=60).step_by(2) {
                for d in (0..=100).step_by(5) {
                    for s in STRENGTHS {
                        let p = Perspective::new(rx, ry, d).with_strength(s);
                        let cam = Camera::new(p, rect);
                        for corner in [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()] {
                            min_div = min_div.min(cam.divisor(corner) / m);
                        }
                        let q = p.quad(rect);
                        assert!(q.iter().all(|v| v.x.is_finite() && v.y.is_finite()), "{name} {rx}/{ry}/{d}/{s}: not finite");
                        convex += usize::from(quad_is_convex(&q));
                        combos += 1;
                    }
                }
            }
        }
        println!("  {name:<10} {combos:>10} {min_div:>16.4} {:>9}", if convex == combos { "all".to_owned() } else { format!("{convex}/{combos}") });
        assert!(min_div > 0.28, "{name}: the divisor fell to {min_div}·M");
        assert_eq!(convex, combos, "{name}: an outline is not convex");
        total += combos;
    }
    println!("  {total} projections");
}

#[test]
fn the_property_is_read_clamped_and_ignored_when_nothing_is_tilted() {
    let mk = |value: Option<PropValue>, ry: i64| {
        let mut c = Control::new("C", ControlType::BarChart, 0, 0);
        c.set_prop("RotateY", PropValue::Int(ry));
        if let Some(v) = value {
            c.set_prop("PerspectiveStrength", v);
        }
        c
    };
    assert_eq!(Perspective::of(&mk(None, -30)), Some(Perspective::new(0, -30, 50)), "absent reads as 100");
    assert_eq!(Perspective::of(&mk(Some(PropValue::Int(100)), -30)), Some(Perspective::new(0, -30, 50)));
    assert_eq!(Perspective::of(&mk(Some(PropValue::Int(40)), -30)), Some(Perspective::new(0, -30, 50).with_strength(40)));
    assert_eq!(Perspective::of(&mk(Some(PropValue::Int(400)), -30)).unwrap().strength, 100, "clamped on read");
    assert_eq!(Perspective::of(&mk(Some(PropValue::Int(-9)), -30)).unwrap().strength, 0);
    assert_eq!(Perspective::of(&mk(Some(PropValue::String("12".into())), -30)).unwrap().strength, 12, "text from COBOL");
    assert_eq!(Perspective::of(&mk(Some(PropValue::String("wobbly".into())), -30)).unwrap().strength, 100, "unreadable text is the default");
    // Nothing tilted and nothing moved: the strength has nothing to scale, so the control is not touched at all.
    assert_eq!(Perspective::of(&mk(Some(PropValue::Int(0)), 0)), None);
    // …but a distance alone is still a zoom, whatever the strength says.
    let mut zoom = mk(Some(PropValue::Int(0)), 0);
    zoom.set_prop("PerspectiveDistance", PropValue::Int(80));
    assert_eq!(Perspective::of(&zoom).map(|p| (p.distance, p.strength)), Some((80, 0)));
    // Another type never reads it.
    let mut button = Control::new("B", ControlType::Button, 0, 0);
    button.set_prop("RotateY", PropValue::Int(30));
    button.set_prop("PerspectiveStrength", PropValue::Int(10));
    assert_eq!(Perspective::of(&button), None);
    println!("\n  absent/100 → as before · 40 → 40 · 400 → 100 · −9 → 0 · \"12\" → 12 · text → 100 · untilted → not touched");
}
