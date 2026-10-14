// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 098 — **the corner pin: the outline of a display control is any convex quadrilateral.**
//!
//! Eight properties move the four corners of the projected outline; the control is then mapped by
//! the one homography that sends its designed rectangle onto that outline. What is asserted, in
//! numbers:
//!
//! * with no pin, nothing changes: the pin-hole formula is used as it was, equal at every point (R3);
//! * the pinned outline's corners are the tilt's corners plus the pins (R2);
//! * every projected point is where the homography puts it — straight edges stay straight, the
//!   centre lands where the outline's diagonals cross, a picture's mid-edge texel lands in
//!   perspective and not halfway on the screen (R2);
//! * the pins scale with the control's drawn size (R4);
//! * a pin that folds the outline is refused and the control keeps its tilt alone, and over random
//!   convex outlines nothing is behind the camera (R5);
//! * the properties are read, clamped, and a flat pinned control is active (R1).
//!
//! Spec 098 AC2, AC3, AC4, AC5.

#![cfg(feature = "render")]

use cobolt_forms::model::{Control, ControlType, PropValue, PIN_LIMIT, PIN_PROPS};
use cobolt_forms::perspective::{
    pin_is_drawable, quad_contains, quad_is_convex, warp_mesh, Camera, Homography, Perspective, CAMERA_K,
};
use egui::epaint::{Mesh, Vertex};
use egui::{pos2, Color32, Pos2, Rect, TextureId};

fn card() -> Rect {
    Rect::from_min_size(pos2(100.0, 60.0), egui::vec2(320.0, 220.0))
}

/// A perspective measured in a 320 × 220 control, the size of [`card`].
fn p(rx: i64, ry: i64, d: i64, pins: [[i64; 2]; 4]) -> Perspective {
    Perspective::new(rx, ry, d).with_pins(pins).designed(320.0, 220.0)
}

const NO_PIN: [[i64; 2]; 4] = [[0, 0]; 4];

/// The projection of spec 097 (tilt, distance, strength), written out.
fn spec_097(rx: i64, ry: i64, d: i64, rect: Rect, pt: Pos2) -> Pos2 {
    let m = rect.width().max(rect.height()).max(1.0);
    let reference = CAMERA_K * m;
    let depth = reference * 2f32.powf((50.0 - d as f32) / 50.0);
    let (sx, cx) = (rx as f32).to_radians().sin_cos();
    let (sy, cy) = (ry as f32).to_radians().sin_cos();
    let c = rect.center();
    let (x, y) = (pt.x - c.x, pt.y - c.y);
    let (y1, z1) = (y * cx, -y * sx);
    let (x2, z2) = (x * cy - z1 * sy, z1 * cy + x * sy);
    let k = reference / (depth + z2);
    pos2(c.x + x2 * k, c.y + y1 * k)
}

#[test]
fn with_no_pin_the_pin_hole_formula_is_used_as_it_was() {
    let c = card();
    let (mut worst, mut n) = (0.0f32, 0usize);
    for rx in (-60..=60).step_by(15) {
        for ry in (-60..=60).step_by(15) {
            for d in [0, 30, 50, 80, 100] {
                let cam = Camera::new(p(rx, ry, d, NO_PIN), c);
                assert!(!cam.is_pinned(), "no pin → no homography");
                for pt in [c.left_top(), c.right_top(), c.right_bottom(), c.left_bottom(), c.center(), pos2(c.left() + 37.0, c.top() + 91.0)] {
                    worst = worst.max((cam.project(pt) - spec_097(rx, ry, d, c, pt)).length());
                    n += 1;
                }
            }
        }
    }
    println!("\n  {n} points with eight zero pins against the spec 097 formula: worst difference {worst:e} px");
    assert_eq!(worst, 0.0, "a zero pin must leave the old formula untouched, not fit a homography to it");
}

#[test]
fn the_pinned_outlines_corners_are_the_tilts_corners_plus_the_pins() {
    let c = card();
    let pins = [[-12, 18], [25, -9], [7, 30], [-20, 4]];
    let tilt = p(8, -30, 60, NO_PIN).quad(c);
    let pinned = p(8, -30, 60, pins).quad(c);
    println!("\n  corner      tilt's corner            pin        pinned corner");
    for i in 0..4 {
        let want = pos2(tilt[i].x + pins[i][0] as f32, tilt[i].y + pins[i][1] as f32);
        println!("  {i}   ({:7.2}, {:7.2})   {:?}   ({:7.2}, {:7.2})", tilt[i].x, tilt[i].y, pins[i], pinned[i].x, pinned[i].y);
        assert!((pinned[i] - want).length() < 1e-3, "corner {i}");
    }
}

#[test]
fn every_point_lands_where_the_homography_puts_it_and_edges_stay_straight() {
    let c = card();
    // A roll and an off-axis convergence no tilt can make: the panel of the operator's picture.
    let pins = [[0, 14], [0, -22], [40, 36], [-8, 0]];
    let per = p(9, -32, 50, pins);
    let cam = per.camera(c);
    assert!(cam.is_pinned());
    let q = per.quad(c);
    assert!(quad_is_convex(&q));

    // Straight edges: a point a fraction t along a designed edge lands on the outline's edge.
    let mut worst_edge = 0.0f32;
    for t in [0.1f32, 0.25, 0.5, 0.75, 0.9] {
        for (a, b, qa, qb) in [
            (c.left_top(), c.right_top(), q[0], q[1]),
            (c.right_top(), c.right_bottom(), q[1], q[2]),
            (c.right_bottom(), c.left_bottom(), q[2], q[3]),
            (c.left_bottom(), c.left_top(), q[3], q[0]),
        ] {
            let on_plane = pos2(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            let s = cam.project(on_plane);
            // Distance from the line through the outline's edge.
            let e = qb - qa;
            let dist = ((s.x - qa.x) * e.y - (s.y - qa.y) * e.x).abs() / e.length();
            worst_edge = worst_edge.max(dist);
        }
    }
    // The centre of the rectangle lands where the outline's diagonals cross — the signature of a
    // projective map, and not where an affine stretch would put it.
    let centre = cam.project(c.center());
    let (a, b, cc, d) = (q[0], q[2], q[1], q[3]);
    let (r, s) = (b - a, d - cc);
    let den = r.x * s.y - r.y * s.x;
    let t = ((cc.x - a.x) * s.y - (cc.y - a.y) * s.x) / den;
    let cross = pos2(a.x + r.x * t, a.y + r.y * t);
    let affine_centre = pos2((q[0].x + q[1].x + q[2].x + q[3].x) / 4.0, (q[0].y + q[1].y + q[2].y + q[3].y) / 4.0);
    println!(
        "\n  pinned to {q:.1?}\n  20 edge points: worst distance from the outline's edge {worst_edge:.4} px\n  the centre lands at ({:.2}, {:.2}); the diagonals cross at ({:.2}, {:.2}); an affine map would say ({:.2}, {:.2})",
        centre.x, centre.y, cross.x, cross.y, affine_centre.x, affine_centre.y
    );
    assert!(worst_edge < 1e-2, "an edge bent by {worst_edge} px");
    assert!((centre - cross).length() < 1e-2, "the centre must land on the diagonals' crossing");
    assert!((centre - affine_centre).length() > 1.0, "…which is not the affine centre, so this really is projective");
    // Every point of the plane is inside the outline.
    for i in 0..=8 {
        for j in 0..=8 {
            let pt = pos2(c.left() + c.width() * i as f32 / 8.0, c.top() + c.height() * j as f32 / 8.0);
            assert!(quad_contains(&q, cam.project(pt)) || (i == 0 || i == 8 || j == 0 || j == 8), "({i},{j}) left the outline");
        }
    }
}

#[test]
fn a_pictures_corners_land_on_the_outline_and_its_mid_edge_texel_in_perspective() {
    let c = card();
    let pins = [[0, 14], [0, -22], [40, 36], [-8, 0]];
    let per = p(9, -32, 50, pins);
    let cam = per.camera(c);
    let q = per.quad(c);
    let white = Color32::WHITE;
    let v = |x: f32, y: f32, u: f32, w: f32| Vertex { pos: pos2(x, y), uv: pos2(u, w), color: white };
    let mut image = Mesh::with_texture(TextureId::User(3));
    image.vertices = vec![v(c.left(), c.top(), 0.0, 0.0), v(c.right(), c.top(), 1.0, 0.0), v(c.right(), c.bottom(), 1.0, 1.0), v(c.left(), c.bottom(), 0.0, 1.0)];
    image.indices = vec![0, 1, 2, 0, 2, 3];
    let out = warp_mesh(&cam, &image);
    for (uv, corner, name) in [(pos2(0.0, 0.0), q[0], "top-left"), (pos2(1.0, 0.0), q[1], "top-right"), (pos2(1.0, 1.0), q[2], "bottom-right"), (pos2(0.0, 1.0), q[3], "bottom-left")] {
        let vx = out.vertices.iter().find(|x| (x.uv - uv).length() < 1e-5).unwrap_or_else(|| panic!("no {name} texel"));
        assert!((vx.pos - corner).length() < 1e-3, "{name} texel lands on the outline's corner");
    }
    let mid = out.vertices.iter().filter(|x| x.uv.y.abs() < 1e-5).min_by(|a, b| (a.uv.x - 0.5).abs().total_cmp(&(b.uv.x - 0.5).abs())).unwrap();
    let in_perspective = cam.project(pos2(c.left() + mid.uv.x * c.width(), c.top()));
    let on_screen = pos2(q[0].x + (q[1].x - q[0].x) * mid.uv.x, q[0].y + (q[1].y - q[0].y) * mid.uv.x);
    println!(
        "\n  image: {} triangles; the texel u = {:.3} on the top edge lands at x = {:.2}, in perspective {:.2}, an affine stretch {:.2}",
        out.indices.len() / 3, mid.uv.x, mid.pos.x, in_perspective.x, on_screen.x
    );
    assert!((mid.pos - in_perspective).length() < 1e-2);
    assert!((mid.pos - on_screen).length() > 0.5, "the mapping is not affine");
}

#[test]
fn the_pins_scale_with_the_control_as_an_animation_or_a_surface_scales_it() {
    let pins = [[-12, 18], [25, -9], [7, 30], [-20, 4]];
    let per = p(0, -30, 50, pins);
    println!("\n  drawn size   scale   pin [-12, 18] applied as");
    for (w, h, scale) in [(320.0f32, 220.0f32, 1.0f32), (160.0, 110.0, 0.5), (480.0, 330.0, 1.5)] {
        let rect = Rect::from_min_size(pos2(50.0, 50.0), egui::vec2(w, h));
        let tilt = per.unpinned().quad(rect);
        let pinned = per.quad(rect);
        let applied = pinned[0] - tilt[0];
        println!("  {w:>5} × {h:<5}  {scale:>4.1}   ({:+.1}, {:+.1})", applied.x, applied.y);
        assert!((applied.x - (-12.0 * scale)).abs() < 1e-3 && (applied.y - 18.0 * scale).abs() < 1e-3);
    }
    // Unknown designed size: the pins are taken as they are.
    let raw = Perspective::new(0, -30, 50).with_pins(pins);
    let rect = Rect::from_min_size(pos2(50.0, 50.0), egui::vec2(160.0, 110.0));
    let applied = raw.quad(rect)[0] - raw.unpinned().quad(rect)[0];
    assert!((applied.x + 12.0).abs() < 1e-3 && (applied.y - 18.0).abs() < 1e-3);
}

#[test]
fn a_pin_that_folds_the_outline_is_refused_and_the_tilt_alone_is_drawn() {
    let c = card();
    let tilt = p(8, -30, 60, NO_PIN).quad(c);
    // The top-left corner dragged far to the right, past the top-right: a bow tie.
    let folded = p(8, -30, 60, [[600, 0], [0, 0], [0, 0], [0, 0]]);
    let q = folded.quad(c);
    assert!(!pin_is_drawable(&tilt, &folded.pinned_quad(c)));
    assert!(!folded.camera(c).is_pinned(), "a folded pin is not drawn");
    for i in 0..4 {
        assert!((q[i] - tilt[i]).length() < 1e-4, "corner {i} is the tilt's");
    }
    // A concave dent: the bottom-right corner pushed inside the shape.
    let dented = p(0, 0, 50, [[0, 0], [0, 0], [-250, -180], [0, 0]]);
    assert!(!dented.camera(c).is_pinned());
    // A mirror (the winding reversed) is a fold too.
    let mirrored = p(0, 0, 50, [[320, 0], [-320, 0], [-320, 0], [320, 0]]);
    assert!(!mirrored.camera(c).is_pinned(), "a reversed outline is not drawn");
    // The far side squeezed to a sliver: the top edge pulled in to 6 px of the 320.
    let sliver = p(0, 0, 50, [[157, 0], [-157, 0], [0, 0], [0, 0]]);
    assert!(!sliver.camera(c).is_pinned(), "a far side thinner than a twentieth of the near side is not drawn");
    // …while a strong but sane foreshortening (the top a quarter of the bottom) is.
    assert!(p(0, 0, 50, [[120, 0], [-120, 0], [0, 0], [0, 0]]).camera(c).is_pinned());
    // …while a legitimate pin is.
    assert!(p(8, -30, 60, [[-12, 18], [25, -9], [7, 30], [-20, 4]]).camera(c).is_pinned());
    println!("\n  bow tie, dent, mirror and sliver → tilt alone; a convex pin → drawn pinned");
}

/// A deterministic pseudo-random sequence, so the sweep is the same on every machine.
fn lcg(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 33) as f32) / (u32::MAX >> 1) as f32
}

#[test]
fn over_random_convex_outlines_nothing_is_behind_the_camera_and_every_point_stays_inside() {
    let c = card();
    let mut seed = 20261014u64;
    let (mut tried, mut drawable, mut worst_den) = (0usize, 0usize, f32::MAX);
    while drawable < 2000 {
        tried += 1;
        let mut pins = [[0i64; 2]; 4];
        for corner in &mut pins {
            corner[0] = (lcg(&mut seed) * 200.0 - 100.0) as i64;
            corner[1] = (lcg(&mut seed) * 160.0 - 80.0) as i64;
        }
        let rx = (lcg(&mut seed) * 120.0 - 60.0) as i64;
        let ry = (lcg(&mut seed) * 120.0 - 60.0) as i64;
        let d = (lcg(&mut seed) * 100.0) as i64;
        let per = p(rx, ry, d, pins);
        let cam = per.camera(c);
        if !cam.is_pinned() {
            continue;
        }
        drawable += 1;
        let q = per.quad(c);
        assert!(quad_is_convex(&q));
        let h = Homography::square_to(&q).unwrap();
        worst_den = worst_den.min(h.corner_denominators().iter().cloned().fold(f32::MAX, f32::min));
        for i in 0..=4 {
            for j in 0..=4 {
                let pt = pos2(c.left() + c.width() * i as f32 / 4.0, c.top() + c.height() * j as f32 / 4.0);
                let s = cam.project(pt);
                assert!(s.x.is_finite() && s.y.is_finite());
                assert!(cam.divisor(pt) > 0.0, "behind the camera at ({i},{j})");
                let inside = quad_contains(&q, s);
                let on_edge = i == 0 || i == 4 || j == 0 || j == 4;
                assert!(inside || on_edge, "a point left the outline");
            }
        }
    }
    println!("\n  {tried} random pins tried, {drawable} drawable (convex, same winding, far side ≥ 1/20 of the near); least corner denominator {worst_den:.4}; every grid point inside, none behind the camera");
    assert!(worst_den > 0.0);
}

#[test]
fn the_properties_are_read_clamped_and_a_flat_pinned_control_is_active() {
    let mut chart = Control::new("C", ControlType::BarChart, 10, 10);
    chart.rect = cobolt_forms::Rect::new(10, 10, 320, 220);
    assert_eq!(Perspective::of(&chart), None, "no tilt, no pin: not touched");
    for key in PIN_PROPS.iter().flatten() {
        assert_eq!(chart.get_prop(key), Some(&PropValue::Int(0)), "{key} is seeded at 0");
    }
    chart.set_prop("PinBottomRightX", PropValue::Int(40));
    let per = Perspective::of(&chart).expect("a flat control with a pin is active");
    assert_eq!(per.pin, [[0, 0], [0, 0], [40, 0], [0, 0]]);
    assert_eq!(per.designed, [320.0, 220.0], "the designed size travels with the pins");
    chart.set_prop("PinTopLeftY", PropValue::Int(20000));
    chart.set_prop("PinTopRightX", PropValue::String("-15".into()));
    chart.set_prop("PinBottomLeftY", PropValue::String("wobbly".into()));
    let per = Perspective::of(&chart).unwrap();
    assert_eq!(per.pin[0][1], PIN_LIMIT as i32, "clamped on read");
    assert_eq!(per.pin[1][0], -15, "text from COBOL");
    assert_eq!(per.pin[3][1], 0, "unreadable text is the default");
    let mut button = Control::new("B", ControlType::Button, 0, 0);
    button.set_prop("PinTopLeftX", PropValue::Int(30));
    assert_eq!(Perspective::of(&button), None, "another type never reads a pin");
    println!("\n  seeded 0 · a flat pinned control is active · 20000 → 9999 · \"-15\" → -15 · text → 0 · Button → ignored");
}
