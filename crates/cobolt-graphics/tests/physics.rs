// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Physics2D and Physics3D (WASM-018 R10, R20, AC6): worlds, bodies, shapes, joints, ray
//! casts, events and - the point of `enhanced-determinism` - the same bytes with one
//! thread and with many.
#![cfg(feature = "physics")]

use cobolt_graphics::handles::Handle;
use cobolt_graphics::physics::{BindTarget, BodyKind, CollisionKind, Physics2D, Physics3D, ShapeOptions};
use cobolt_graphics::Limits;

const Q: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const DT: f32 = 1.0 / 60.0;

fn opts() -> ShapeOptions {
    ShapeOptions::default()
}

/// A floor and a pile of boxes, spheres and capsules dropped in a loose grid, in 3D.
fn pile3(p: &mut Physics3D, n: usize) -> Handle {
    let w = p.create_world([0.0, -9.81, 0.0]).unwrap();
    let floor = p.add_body(w, BodyKind::Static, [0.0, -0.5, 0.0], Q).unwrap();
    p.add_box(w, floor, [30.0, 0.5, 30.0], &opts()).unwrap();
    let side = ((n as f32).cbrt().ceil() as usize).max(1);
    let mut k = 0;
    'outer: for y in 0..side {
        for z in 0..side {
            for x in 0..side {
                if k == n {
                    break 'outer;
                }
                // A little jitter that is a pure function of the index (no RNG).
                let j = |s: usize| ((k * 7919 + s * 104729) % 1000) as f32 / 1000.0 * 0.2;
                let pos = [x as f32 * 1.3 - side as f32 * 0.65 + j(1), 0.8 + y as f32 * 1.3 + j(2), z as f32 * 1.3 - side as f32 * 0.65 + j(3)];
                let b = p.add_body(w, BodyKind::Dynamic, pos, Q).unwrap();
                match k % 3 {
                    0 => p.add_box(w, b, [0.5, 0.5, 0.5], &opts()).unwrap(),
                    1 => p.add_ball(w, b, 0.55, &opts()).unwrap(),
                    _ => p.add_capsule(w, b, 0.3, 0.35, &opts()).unwrap(),
                };
                k += 1;
            }
        }
    }
    w
}

fn pile2(p: &mut Physics2D, n: usize) -> Handle {
    let w = p.create_world([0.0, -9.81]).unwrap();
    let floor = p.add_body(w, BodyKind::Static, [0.0, -0.5], 0.0).unwrap();
    p.add_box(w, floor, [60.0, 0.5], &opts()).unwrap();
    for wall in [-30.0f32, 30.0] {
        let b = p.add_body(w, BodyKind::Static, [wall, 20.0], 0.0).unwrap();
        p.add_box(w, b, [0.5, 20.0], &opts()).unwrap();
    }
    let cols = 20usize;
    for k in 0..n {
        let j = |s: usize| ((k * 7919 + s * 104729) % 1000) as f32 / 1000.0 * 0.2;
        let pos = [(k % cols) as f32 * 1.2 - cols as f32 * 0.6 + j(1), 0.8 + (k / cols) as f32 * 1.2 + j(2)];
        let b = p.add_body(w, BodyKind::Dynamic, pos, j(3)).unwrap();
        match k % 3 {
            0 => p.add_box(w, b, [0.5, 0.5], &opts()).unwrap(),
            1 => p.add_ball(w, b, 0.55, &opts()).unwrap(),
            _ => p.add_capsule(w, b, 0.3, 0.35, &opts()).unwrap(),
        };
    }
    w
}

#[cfg(feature = "parallel")]
#[test]
fn physics3d_gives_the_same_bytes_with_one_thread_and_with_many() {
    let mut prints = Vec::new();
    for threads in [1usize, 2, 8] {
        let mut p = Physics3D::new(Limits::default());
        let w = pile3(&mut p, 300);
        p.set_threads(w, threads).unwrap();
        let t = std::time::Instant::now();
        for _ in 0..240 {
            p.step(w, DT).unwrap();
        }
        let took = t.elapsed();
        let fp = p.state_fingerprint(w).unwrap();
        eprintln!("physics3d: 300 bodies, 240 steps, {threads} thread(s): {took:?}, state {fp:016x}");
        prints.push(fp);
    }
    assert!(prints.windows(2).all(|w| w[0] == w[1]), "the final state depends on the number of threads: {prints:x?}");
}

#[cfg(feature = "parallel")]
#[test]
fn physics2d_gives_the_same_bytes_with_one_thread_and_with_many() {
    let mut prints = Vec::new();
    for threads in [1usize, 2, 8] {
        let mut p = Physics2D::new(Limits::default());
        let w = pile2(&mut p, 600);
        p.set_threads(w, threads).unwrap();
        let t = std::time::Instant::now();
        for _ in 0..300 {
            p.step(w, DT).unwrap();
        }
        let took = t.elapsed();
        let fp = p.state_fingerprint(w).unwrap();
        eprintln!("physics2d: 600 bodies, 300 steps, {threads} thread(s): {took:?}, state {fp:016x}");
        prints.push(fp);
    }
    assert!(prints.windows(2).all(|w| w[0] == w[1]), "the final state depends on the number of threads: {prints:x?}");
}

#[test]
fn the_same_program_run_twice_gives_the_same_bytes() {
    let run = || {
        let mut p = Physics3D::new(Limits::default());
        let w = pile3(&mut p, 120);
        for _ in 0..120 {
            p.step(w, DT).unwrap();
        }
        p.state_fingerprint(w).unwrap()
    };
    assert_eq!(run(), run());
}

#[test]
fn stacked_boxes_settle_to_known_positions_and_the_same_ones_every_run() {
    // AC6: the final positions of a stack, to the bit, in two runs; and where physics says they are.
    let run = || {
        let mut p = Physics2D::new(Limits::default());
        let w = p.create_world([0.0, -9.81]).unwrap();
        let floor = p.add_body(w, BodyKind::Static, [0.0, -0.5], 0.0).unwrap();
        p.add_box(w, floor, [20.0, 0.5], &opts()).unwrap();
        let mut stack = Vec::new();
        for i in 0..6 {
            let b = p.add_body(w, BodyKind::Dynamic, [0.0, 0.5 + i as f32 * 1.1], 0.0).unwrap();
            p.add_box(w, b, [0.5, 0.5], &opts()).unwrap();
            stack.push(b);
        }
        for _ in 0..900 {
            p.step(w, DT).unwrap();
        }
        stack.iter().map(|b| p.get_position(w, *b).unwrap()).collect::<Vec<_>>()
    };
    let a = run();
    let b = run();
    assert_eq!(a, b, "bit-identical in two runs");
    for (i, [x, y]) in a.iter().enumerate() {
        assert!(x.abs() < 0.05, "box {i}: x = {x}");
        assert!((y - (0.5 + i as f32)).abs() < 0.1, "box {i} should rest at {}: y = {y}", 0.5 + i as f32);
    }
}

#[test]
fn a_ball_dropped_on_the_floor_raises_one_started_event_with_the_right_handles() {
    let mut p = Physics3D::new(Limits::default());
    let w = p.create_world([0.0, -9.81, 0.0]).unwrap();
    let floor = p.add_body(w, BodyKind::Static, [0.0; 3], Q).unwrap();
    let fs = p.add_box(w, floor, [10.0, 0.5, 10.0], &opts()).unwrap();
    let ball = p.add_body(w, BodyKind::Dynamic, [0.0, 3.0, 0.0], Q).unwrap();
    let bs = p.add_ball(w, ball, 0.5, &opts()).unwrap();
    let mut all = Vec::new();
    for _ in 0..180 {
        p.step(w, DT).unwrap();
        all.extend(p.take_events(w).unwrap());
    }
    let started: Vec<_> = all.iter().filter(|e| e.kind == CollisionKind::Started).collect();
    assert!(!started.is_empty(), "the ball touched the floor");
    let e = started[0];
    let (lo, hi) = (floor.min(ball), floor.max(ball));
    assert_eq!((e.body_a, e.body_b), (lo, hi), "events list the lower body first");
    assert!((e.shape_a == fs && e.shape_b == bs) || (e.shape_a == bs && e.shape_b == fs));
    assert!(!e.sensor);
    assert!(p.take_events(w).unwrap().is_empty(), "taking the events clears them");
}

#[test]
fn a_sensor_reports_an_overlap_and_lets_the_ball_through() {
    let mut p = Physics2D::new(Limits::default());
    let w = p.create_world([0.0, -9.81]).unwrap();
    let zone = p.add_body(w, BodyKind::Static, [0.0, 0.0], 0.0).unwrap();
    let sensor = ShapeOptions { sensor: true, ..opts() };
    let zs = p.add_box(w, zone, [5.0, 1.0], &sensor).unwrap();
    let ball = p.add_body(w, BodyKind::Dynamic, [0.0, 4.0], 0.0).unwrap();
    p.add_ball(w, ball, 0.5, &opts()).unwrap();
    let mut events = Vec::new();
    for _ in 0..120 {
        p.step(w, DT).unwrap();
        events.extend(p.take_events(w).unwrap());
    }
    assert!(events.iter().any(|e| e.kind == CollisionKind::Started && e.sensor && (e.shape_a == zs || e.shape_b == zs)), "the ball entered the zone");
    assert!(events.iter().any(|e| e.kind == CollisionKind::Stopped && e.sensor), "and left it");
    assert!(p.get_position(w, ball).unwrap()[1] < -1.0, "it fell through");
}

#[test]
fn a_pendulum_keeps_its_length_and_a_slider_stays_on_its_axis() {
    let mut p = Physics2D::new(Limits::default());
    let w = p.create_world([0.0, -9.81]).unwrap();
    let anchor = p.add_body(w, BodyKind::Static, [0.0, 10.0], 0.0).unwrap();
    let bob = p.add_body(w, BodyKind::Dynamic, [3.0, 10.0], 0.0).unwrap();
    p.add_ball(w, bob, 0.3, &opts()).unwrap();
    // Hinge at the anchor; the bob's own anchor is 3 m to its left.
    p.add_revolute_joint(w, anchor, bob, [0.0, 0.0], [-3.0, 0.0], None).unwrap();
    // A slider along X on its own.
    let rail = p.add_body(w, BodyKind::Static, [0.0, 0.0], 0.0).unwrap();
    let cart = p.add_body(w, BodyKind::Dynamic, [0.0, 0.0], 0.0).unwrap();
    p.add_box(w, cart, [0.4, 0.2], &opts()).unwrap();
    p.add_prismatic_joint(w, rail, cart, [0.0; 2], [0.0; 2], [1.0, 0.0], Some([-2.0, 2.0])).unwrap();
    p.set_velocity(w, cart, [5.0, 3.0]).unwrap();
    let mut worst_len = 0.0f32;
    for _ in 0..300 {
        p.step(w, DT).unwrap();
        let [x, y] = p.get_position(w, bob).unwrap();
        worst_len = worst_len.max((((x).powi(2) + (y - 10.0).powi(2)).sqrt() - 3.0).abs());
    }
    assert!(worst_len < 0.05, "the pendulum stretched by {worst_len} m");
    let [cx, cy] = p.get_position(w, cart).unwrap();
    assert!(cy.abs() < 0.05, "the cart left its rail: y = {cy}");
    assert!(cx <= 2.05 && cx > 1.5, "the cart stopped at the +2 limit: x = {cx}");
    let _ = rail;
}

#[test]
fn a_fixed_joint_carries_a_body_along() {
    let mut p = Physics3D::new(Limits::default());
    let w = p.create_world([0.0, -9.81, 0.0]).unwrap();
    let a = p.add_body(w, BodyKind::Dynamic, [0.0, 5.0, 0.0], Q).unwrap();
    let b = p.add_body(w, BodyKind::Dynamic, [2.0, 5.0, 0.0], Q).unwrap();
    p.add_ball(w, a, 0.4, &opts()).unwrap();
    p.add_ball(w, b, 0.4, &opts()).unwrap();
    p.add_fixed_joint(w, a, b, [1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]).unwrap();
    for _ in 0..120 {
        p.step(w, DT).unwrap();
    }
    let (pa, pb) = (p.get_position(w, a).unwrap(), p.get_position(w, b).unwrap());
    let d = ((pb[0] - pa[0]).powi(2) + (pb[1] - pa[1]).powi(2) + (pb[2] - pa[2]).powi(2)).sqrt();
    assert!((d - 2.0).abs() < 0.02, "the welded bodies stayed 2 m apart: {d}");
    assert!(pa[1] < 3.0, "and fell together");
}

#[test]
fn a_hinge_with_a_limit_stops_at_the_limit_in_3d() {
    let mut p = Physics3D::new(Limits::default());
    let w = p.create_world([0.0, -9.81, 0.0]).unwrap();
    let post = p.add_body(w, BodyKind::Static, [0.0, 5.0, 0.0], Q).unwrap();
    let door = p.add_body(w, BodyKind::Dynamic, [1.0, 5.0, 0.0], Q).unwrap();
    p.add_box(w, door, [1.0, 0.1, 0.05], &opts()).unwrap();
    // Hinge about Z at the post; the door swings in the XY plane and gravity pulls it down.
    p.add_revolute_joint(w, post, door, [0.0; 3], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], Some([-0.5, 0.5])).unwrap();
    for _ in 0..300 {
        p.step(w, DT).unwrap();
    }
    let [x, y, _] = p.get_position(w, door).unwrap();
    let angle = (y - 5.0).atan2(x);
    assert!((angle + 0.5).abs() < 0.06, "the door hangs at its -0.5 rad limit: {angle}");
}

#[test]
fn a_trimesh_floor_and_a_convex_hull_body() {
    let mut p = Physics3D::new(Limits::default());
    let w = p.create_world([0.0, -9.81, 0.0]).unwrap();
    let floor = p.add_body(w, BodyKind::Static, [0.0; 3], Q).unwrap();
    // Two triangles making a 20 x 20 square at y = 0, facing up.
    let verts = [[-10.0, 0.0, -10.0], [10.0, 0.0, -10.0], [10.0, 0.0, 10.0], [-10.0, 0.0, 10.0]];
    p.add_trimesh(w, floor, &verts, &[[0, 2, 1], [0, 3, 2]], &opts()).unwrap();
    let pyramid = p.add_body(w, BodyKind::Dynamic, [0.0, 3.0, 0.0], Q).unwrap();
    let hull = [[-1.0, 0.0, -1.0], [1.0, 0.0, -1.0], [1.0, 0.0, 1.0], [-1.0, 0.0, 1.0], [0.0, 1.5, 0.0]];
    p.add_convex_hull(w, pyramid, &hull, &opts()).unwrap();
    for _ in 0..300 {
        p.step(w, DT).unwrap();
    }
    let y = p.get_position(w, pyramid).unwrap()[1];
    // A pyramid's centre of mass is not its origin, but its base (the body's y = 0 plane) ends up on the floor.
    assert!(y.abs() < 0.1, "the pyramid's base rests on the mesh: y = {y}");
}

#[test]
fn bound_bodies_report_their_transforms() {
    let mut p = Physics2D::new(Limits::default());
    let w = p.create_world([0.0, -10.0]).unwrap();
    let a = p.add_body(w, BodyKind::Dynamic, [0.0, 10.0], 0.0).unwrap();
    p.add_ball(w, a, 0.5, &opts()).unwrap();
    let b = p.add_body(w, BodyKind::Dynamic, [4.0, 10.0], 0.0).unwrap();
    p.add_ball(w, b, 0.5, &opts()).unwrap();
    p.bind(w, a, BindTarget::Sprite(7)).unwrap();
    assert_eq!(p.bound(w).unwrap().len(), 1, "only the bound body is listed");
    for _ in 0..60 {
        p.step(w, DT).unwrap();
    }
    let bound = p.bound(w).unwrap();
    assert_eq!(bound[0].target, BindTarget::Sprite(7));
    assert_eq!(bound[0].position, p.get_position(w, a).unwrap(), "the transform is the body's");
    assert!(bound[0].position[1] < 8.0, "and it moved");
    p.unbind(w, a).unwrap();
    assert!(p.bound(w).unwrap().is_empty());
}

#[test]
fn removing_the_world_and_the_object_releases_everything() {
    let mut p = Physics3D::new(Limits::default());
    let w = pile3(&mut p, 50);
    assert_eq!(p.body_count(w).unwrap(), 51);
    p.destroy_world(w).unwrap();
    assert!(matches!(p.body_count(w), Err(cobolt_graphics::GraphicsError::ReleasedHandle(_))));
    assert_eq!(p.world_count(), 0);
}

#[test]
fn a_resting_body_goes_to_sleep_without_event_chatter() {
    let mut p = Physics2D::new(Limits::default());
    let w = p.create_world([0.0, -9.81]).unwrap();
    let floor = p.add_body(w, BodyKind::Static, [0.0, -0.5], 0.0).unwrap();
    p.add_box(w, floor, [5.0, 0.5], &opts()).unwrap();
    let b = p.add_body(w, BodyKind::Dynamic, [0.0, 0.5], 0.0).unwrap();
    p.add_box(w, b, [0.5, 0.5], &opts()).unwrap();
    for _ in 0..2000 {
        p.step(w, DT).unwrap();
    }
    // The body went to sleep resting on the floor: one Started, no chatter after it.
    let ev = p.take_events(w).unwrap();
    assert!(ev.len() <= 2, "{} events", ev.len());
    assert!(p.is_sleeping(w, b).unwrap());
}
