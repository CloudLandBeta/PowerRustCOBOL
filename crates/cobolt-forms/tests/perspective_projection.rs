// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 096, task T3 — **the projection, as pure geometry.**
//!
//! No control is drawn here. A rectangle goes in, a quadrilateral comes out, and
//! what is checked is what the spec promises about it: which edge recedes (R3),
//! that the centre stays put (R4), what the distance does (R5), and that the
//! outline is a sound convex quad for *every* value the three properties can take.
//!
//! Spec 096 AC3 (R3), AC4 (R4, R5).

#![cfg(feature = "render")]

use cobolt_forms::perspective::{
    edge_lengths, quad_contains, quad_is_convex, Camera, Perspective, CAMERA_K,
};
use egui::{pos2, Pos2, Rect};

fn card() -> Rect {
    Rect::from_min_size(pos2(100.0, 60.0), egui::vec2(320.0, 220.0))
}

fn quad(rx: i64, ry: i64, d: i64) -> [Pos2; 4] {
    Perspective::new(rx, ry, d).quad(card())
}

fn show(label: &str, q: &[Pos2; 4]) -> [f32; 4] {
    let e = edge_lengths(q);
    println!(
        "  {label:<26} top {:>7.2}  right {:>7.2}  bottom {:>7.2}  left {:>7.2}",
        e[0], e[1], e[2], e[3]
    );
    e
}

#[test]
fn the_edge_that_recedes_is_the_shorter_one() {
    println!("\n  ── Which edge recedes (R3) — a 320 × 220 card, distance 50 ──");
    let flat = show("RotateX 0, RotateY 0", &quad(0, 0, 50));
    assert!((flat[0] - 320.0).abs() < 1e-3 && (flat[1] - 220.0).abs() < 1e-3);

    // RotateY > 0 → the RIGHT edge recedes: shorter than the left.
    let y_pos = show("RotateY +30", &quad(0, 30, 50));
    assert!(y_pos[1] < y_pos[3], "RotateY +30: right {} !< left {}", y_pos[1], y_pos[3]);
    // RotateY < 0 → the LEFT edge recedes — the operator's reference picture.
    let y_neg = show("RotateY -30", &quad(0, -30, 50));
    assert!(y_neg[3] < y_neg[1], "RotateY -30: left {} !< right {}", y_neg[3], y_neg[1]);
    // Mirror images of one another.
    assert!((y_pos[1] - y_neg[3]).abs() < 1e-3 && (y_pos[3] - y_neg[1]).abs() < 1e-3);

    // RotateX > 0 → the TOP edge recedes: shorter than the bottom.
    let x_pos = show("RotateX +30", &quad(30, 0, 50));
    assert!(x_pos[0] < x_pos[2], "RotateX +30: top {} !< bottom {}", x_pos[0], x_pos[2]);
    // RotateX < 0 → the BOTTOM edge recedes.
    let x_neg = show("RotateX -30", &quad(-30, 0, 50));
    assert!(x_neg[2] < x_neg[0], "RotateX -30: bottom {} !< top {}", x_neg[2], x_neg[0]);
    assert!((x_pos[0] - x_neg[2]).abs() < 1e-3 && (x_pos[2] - x_neg[0]).abs() < 1e-3);

    // Both tilts: the corner that stays nearest is the largest — here RotateX +30
    // (top recedes) with RotateY +30 (right recedes) leaves the BOTTOM-LEFT nearest.
    let q = quad(30, 30, 50);
    let e = show("RotateX +30, RotateY +30", &q);
    assert!(e[2] > e[0] && e[3] > e[1], "bottom {} > top {} and left {} > right {}", e[2], e[0], e[3], e[1]);
    // The nearest corner is the one with the smallest camera divisor, the farthest the
    // largest: the bottom-left comes nearest and the top-right recedes most.
    let cam = Perspective::new(30, 30, 50).camera(card());
    let r = card();
    let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
    let div: Vec<f32> = corners.iter().map(|p| cam.divisor(*p)).collect();
    let nearest = (0..4).min_by(|&a, &b| div[a].total_cmp(&div[b])).unwrap();
    let farthest = (0..4).max_by(|&a, &b| div[a].total_cmp(&div[b])).unwrap();
    println!("  corner divisors (tl, tr, br, bl): {div:.1?} → nearest {nearest}, farthest {farthest}");
    assert_eq!(nearest, 3, "the bottom-left corner is the nearest");
    assert_eq!(farthest, 1, "the top-right corner is the farthest");
}

#[test]
fn an_untilted_card_is_scaled_about_its_centre_by_the_distance() {
    println!("\n  ── Distance alone (R4, R5) — RotateX = RotateY = 0 ──");
    println!("  distance   width ×   height ×   scale F/D");
    let c = card();
    let mut widths = Vec::new();
    for d in [0i64, 25, 50, 75, 100] {
        let p = Perspective::new(0, 0, d);
        let q = p.quad(c);
        let e = edge_lengths(&q);
        let cam = p.camera(c);
        println!(
            "  {d:>8}   {:>7.4}   {:>8.4}   {:>9.4}",
            e[0] / c.width(),
            e[1] / c.height(),
            cam.flat_scale()
        );
        // Each about the same centre.
        let mid = pos2((q[0].x + q[2].x) / 2.0, (q[0].y + q[2].y) / 2.0);
        assert!((mid - c.center()).length() < 1e-3, "distance {d}: the centre moved to {mid:?}");
        widths.push(e[0]);
    }
    // 50 is the designed size exactly; 0 is half, 100 is double.
    assert!((widths[2] - c.width()).abs() < 1e-3, "distance 50 must be the designed size");
    assert!((widths[0] - c.width() * 0.5).abs() < 1e-2, "distance 0 must be half");
    assert!((widths[4] - c.width() * 2.0).abs() < 1e-2, "distance 100 must be double");
    assert!(widths.windows(2).all(|w| w[0] < w[1]), "size must grow strictly with the distance: {widths:?}");
}

#[test]
fn nearer_means_stronger_convergence_and_a_larger_control() {
    println!("\n  ── RotateY −30 across the distance (R5) ──");
    println!("  distance   near edge   far edge   near/far   outline width");
    let mut ratios = Vec::new();
    let mut sizes = Vec::new();
    for d in [0i64, 25, 50, 75, 100] {
        let q = quad(0, -30, d);
        let e = edge_lengths(&q);
        let (near, far) = (e[1], e[3]); // −30: the left recedes, the right is nearer
        let width = q.iter().map(|p| p.x).fold(f32::MIN, f32::max)
            - q.iter().map(|p| p.x).fold(f32::MAX, f32::min);
        println!("  {d:>8}   {near:>9.2}   {far:>8.2}   {:>8.4}   {width:>13.2}", near / far);
        ratios.push(near / far);
        sizes.push(width);
    }
    assert!(ratios.windows(2).all(|w| w[0] < w[1]), "convergence must strengthen with the distance: {ratios:?}");
    assert!(sizes.windows(2).all(|w| w[0] < w[1]), "the projected size must grow with the distance: {sizes:?}");
}

#[test]
fn the_centre_of_a_tilted_card_stays_where_the_layout_put_it() {
    // R4 — the projection is anchored at the centre of the designed rectangle, so a
    // tilt does not change the form's layout.
    let c = card();
    let mut worst = 0.0f32;
    let mut n = 0;
    for rx in (-60..=60).step_by(15) {
        for ry in (-60..=60).step_by(15) {
            for d in [0, 50, 100] {
                let cam = Perspective::new(rx, ry, d).camera(c);
                worst = worst.max((cam.project(c.center()) - c.center()).length());
                n += 1;
            }
        }
    }
    println!("\n  centre fixed over {n} combinations: worst drift {worst:e} px");
    assert!(worst < 1e-3, "the centre drifted by {worst} px");
}

#[test]
fn the_divisor_stays_positive_and_the_outline_convex_over_the_whole_range() {
    // R5's footing: nothing is ever behind the camera, so the outline never folds.
    // 121 × 121 × 101 combinations of the three properties (every whole degree
    // and every distance), five aspect ratios.
    let shapes = [
        ("square 200×200", 200.0, 200.0),
        ("wide 320×220", 320.0, 220.0),
        ("very wide 800×40", 800.0, 40.0),
        ("tall 40×600", 40.0, 600.0),
        ("a line 300×2", 300.0, 2.0),
    ];
    println!("\n  ── Sweep: every RotateX, RotateY (−60..60) × every distance (0..100) ──");
    println!("  {:<18} {:>10} {:>16} {:>12}", "shape", "combos", "min divisor/M", "convex?");
    let mut total = 0usize;
    for (name, w, h) in shapes {
        let rect = Rect::from_min_size(pos2(50.0, 50.0), egui::vec2(w, h));
        let m = w.max(h);
        let mut min_div = f32::MAX;
        let mut convex = 0usize;
        let mut combos = 0usize;
        for rx in -60..=60 {
            for ry in -60..=60 {
                for d in 0..=100 {
                    let p = Perspective::new(rx, ry, d);
                    let cam = Camera::new(p, rect);
                    for corner in [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()] {
                        min_div = min_div.min(cam.divisor(corner) / m);
                    }
                    let q = p.quad(rect);
                    assert!(q.iter().all(|p| p.x.is_finite() && p.y.is_finite()), "{name} {rx}/{ry}/{d}: not finite");
                    assert!(quad_is_convex(&q), "{name} RotateX {rx} RotateY {ry} distance {d}: not convex {q:?}");
                    convex += 1;
                    combos += 1;
                }
            }
        }
        println!(
            "  {name:<18} {combos:>10} {min_div:>16.4} {:>12}",
            if convex == combos { "all".to_string() } else { format!("{convex}/{combos}") }
        );
        assert!(min_div > 0.28, "{name}: the divisor fell to {min_div}·M — near the camera plane");
        total += combos;
    }
    println!("  {total} projections, K = {CAMERA_K}");
    assert_eq!(total, 121 * 121 * 101 * 5);
}

#[test]
fn a_pointer_is_inside_the_outline_only_where_the_control_is() {
    let q = quad(0, -30, 50);
    let c = card();
    assert!(quad_contains(&q, c.center()), "the centre is inside");
    // A corner of the DESIGNED rectangle on the receding (left) side is outside the
    // shrunken outline.
    assert!(!quad_contains(&q, c.left_top() + egui::vec2(1.0, 1.0)), "the designed top-left is no longer inside");
    // …and a point beyond the designed rectangle on the near (right) side is inside
    // when the control has been brought forward.
    let near = Perspective::new(0, -30, 100).quad(c);
    let beyond = pos2(c.right() + 20.0, c.center().y);
    assert!(quad_contains(&near, beyond), "an enlarged card covers ground outside its designed rectangle");
    assert!(!quad_contains(&near, pos2(c.right() + 400.0, c.center().y)));
}

#[test]
fn neutral_values_and_other_types_are_not_tilted() {
    use cobolt_forms::model::{Control, ControlType, PropValue};
    let mut chart = Control::new("C", ControlType::BarChart, 0, 0);
    assert_eq!(Perspective::of(&chart), None, "the defaults are not a tilt");
    chart.set_prop("RotateY", PropValue::Int(-30));
    assert_eq!(Perspective::of(&chart), Some(Perspective::new(0, -30, 50)));
    chart.set_prop("RotateY", PropValue::String("200".into()));
    assert_eq!(Perspective::of(&chart), Some(Perspective::new(0, 60, 50)), "clamped on read");
    chart.set_prop("RotateY", PropValue::String("wobbly".into()));
    assert_eq!(Perspective::of(&chart), None, "text that is not a number reads as the default");
    chart.set_prop("RotateY", PropValue::Int(0));
    chart.set_prop("PerspectiveDistance", PropValue::Int(80));
    assert_eq!(Perspective::of(&chart), Some(Perspective::new(0, 0, 80)), "a distance alone is a tilt-less zoom");

    let mut button = Control::new("B", ControlType::Button, 0, 0);
    button.set_prop("RotateY", PropValue::Int(30));
    assert_eq!(Perspective::of(&button), None, "a Button ignores a hand-written RotateY (R16)");
    println!("\n  neutral → None · tilted → Some · out of range clamped · other types → None");
}
