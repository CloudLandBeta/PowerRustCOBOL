// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Physics2D and Physics3D (WASM-018 R10) on `rapier2d` and `rapier3d`.
//!
//! One object holds any number of **worlds** (`CreateWorld` with gravity). A world
//! holds **bodies** (static, dynamic, kinematic), their **shapes** (ball, box,
//! capsule, convex hull, and in 3D a triangle mesh), **joints** (fixed, revolute,
//! prismatic) and the **collision events** of the last step. Everything a program
//! creates is a [`Handle`]; the shapes of a body and the
//! joints that touch it are released with it, and everything is released with its
//! world and with the object.
//!
//! # Determinism (R20)
//!
//! `rapier` is built with `enhanced-determinism`, and the solver's parallel parts
//! (the `parallel` feature, through Rayon) give the same bytes for any number of
//! threads. [`Physics3D::set_threads`] pins a world to a pool of that many threads
//! so a test can compare one thread with many; the tests do, on a pile of boxes.
//! Events are reported sorted, never in the order threads found them.
//!
//! # Fixed step
//!
//! [`Physics3D::set_fixed_step`] makes `Step(dt)` advance the world in equal
//! sub-steps (the usual game loop): the time given is added to an accumulator and
//! the world is stepped once for every whole `step` it holds, at most
//! `max_substeps` times per call (the rest of the debt is dropped, so a stalled
//! page does not spiral). With no fixed step, `Step(dt)` is one step of `dt`.
//!
//! # Forces
//!
//! `ApplyForce` acts during the **next** `Step` call (all its sub-steps) and is
//! then cleared; `ApplyImpulse` changes the velocity at once. Both wake the body.
//!
//! # Binding
//!
//! `Bind` ties a body to a sprite (an id the host maps to something on a
//! Canvas2D) or to a [`Scene3D`](crate::scene3d) node. After each `Step`,
//! `Bound` lists the transforms the host (or the registry) copies to the targets.

mod dim2;
mod dim3;

use std::sync::Mutex;

use crate::handles::Handle;

pub use dim2::Physics2D;
pub use dim3::Physics3D;

/// How a body moves.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BodyKind {
    /// Never moves (floors, walls). COBOL code `0`.
    Static,
    /// Moved by forces, gravity and collisions. COBOL code `1`.
    Dynamic,
    /// Moved by the program (its velocity or position), pushing but never pushed. COBOL code `2`.
    Kinematic,
}

impl BodyKind {
    /// From the number a COBOL program passes.
    pub fn from_code(code: i64) -> Option<BodyKind> {
        match code {
            0 => Some(Self::Static),
            1 => Some(Self::Dynamic),
            2 => Some(Self::Kinematic),
            _ => None,
        }
    }
}

/// The material of a shape.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ShapeOptions {
    /// Mass per unit area (2D) or volume (3D). 1 by default; ignored on static bodies.
    pub density: f32,
    /// Coulomb friction coefficient. 0.5 by default.
    pub friction: f32,
    /// Bounciness, 0 (none) to 1 (perfectly elastic). 0 by default.
    pub restitution: f32,
    /// A sensor reports overlaps (collision events) but does not collide.
    pub sensor: bool,
}

impl Default for ShapeOptions {
    fn default() -> Self {
        Self { density: 1.0, friction: 0.5, restitution: 0.0, sensor: false }
    }
}

impl ShapeOptions {
    pub(crate) fn check(&self) -> crate::Result<()> {
        let ok = |v: f32, lo: f32, hi: f32| v.is_finite() && v >= lo && v <= hi;
        if !ok(self.density, 0.0, 1.0e6) || !ok(self.friction, 0.0, 1.0e3) || !ok(self.restitution, 0.0, 1.0e3) {
            return Err(crate::GraphicsError::invalid(
                "a shape needs a density from 0 to 1,000,000, a friction from 0 to 1,000 and a restitution from 0 to 1,000",
            ));
        }
        Ok(())
    }
}

/// Whether two shapes began or ceased to touch.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CollisionKind {
    /// They touch now and did not before.
    Started,
    /// They no longer touch.
    Stopped,
}

/// A collision event of the last `Step`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CollisionEvent {
    /// Whether the contact started or stopped.
    pub kind: CollisionKind,
    /// The body of the first shape (the lower handle).
    pub body_a: Handle,
    /// The body of the second shape.
    pub body_b: Handle,
    /// The first shape.
    pub shape_a: Handle,
    /// The second shape.
    pub shape_b: Handle,
    /// Whether one of the shapes is a sensor (an overlap, not a contact).
    pub sensor: bool,
}

/// What a `Step` did.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct StepReport {
    /// How many simulation steps ran (0 when a fixed step has not accumulated yet).
    pub steps: u32,
}

/// A ray cast's answer.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RayHit<const N: usize> {
    /// The body hit.
    pub body: Handle,
    /// The shape hit.
    pub shape: Handle,
    /// Distance from the origin to the hit (the direction is normalised first).
    pub distance: f32,
    /// The point hit.
    pub point: [f32; N],
    /// The surface normal there.
    pub normal: [f32; N],
}

/// What a body is bound to.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BindTarget {
    /// A sprite the host knows by this id (a Canvas2D program draws it where the body is).
    Sprite(u32),
    /// A node of a Scene3D.
    Node {
        /// The scene's handle in the registry.
        scene: Handle,
        /// The node's handle in that scene.
        node: Handle,
    },
}

/// A bound body's transform, for the host to copy to its target (2D).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Bound2 {
    /// The body.
    pub body: Handle,
    /// What it drives.
    pub target: BindTarget,
    /// Where the body is.
    pub position: [f32; 2],
    /// Its rotation in radians (counter-clockwise).
    pub angle: f32,
}

/// A bound body's transform, for the host to copy to its target (3D).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Bound3 {
    /// The body.
    pub body: Handle,
    /// What it drives.
    pub target: BindTarget,
    /// Where the body is.
    pub position: [f32; 3],
    /// Its rotation as a unit quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
}

// ---------------------------------------------------------------------------
// Shared plumbing for the two dimensions
// ---------------------------------------------------------------------------

/// What the solver's event callback saw (raw handles, resolved after the step).
#[derive(Copy, Clone)]
pub(crate) struct RawEvent {
    pub kind: CollisionKind,
    pub body_a: u32,
    pub body_b: u32,
    pub shape_a: u32,
    pub shape_b: u32,
    pub sensor: bool,
}

/// An event sink that tolerates being called from solver threads.
#[derive(Default)]
pub(crate) struct Collector {
    pub events: Mutex<Vec<RawEvent>>,
}

impl Collector {
    pub fn push(&self, e: RawEvent) {
        if let Ok(mut v) = self.events.lock() {
            v.push(e);
        }
    }

    /// The events, ordered by handle so thread timing never shows.
    pub fn drain_sorted(&self) -> Vec<CollisionEvent> {
        let mut raw = match self.events.lock() {
            Ok(mut v) => std::mem::take(&mut *v),
            Err(_) => Vec::new(),
        };
        // Put the lower body first in each event, then order the whole list.
        for e in raw.iter_mut() {
            if (e.body_a, e.shape_a) > (e.body_b, e.shape_b) {
                std::mem::swap(&mut e.body_a, &mut e.body_b);
                std::mem::swap(&mut e.shape_a, &mut e.shape_b);
            }
        }
        raw.sort_by_key(|e| (e.body_a, e.body_b, e.shape_a, e.shape_b, e.kind));
        raw.into_iter()
            .map(|e| CollisionEvent { kind: e.kind, body_a: e.body_a, body_b: e.body_b, shape_a: e.shape_a, shape_b: e.shape_b, sensor: e.sensor })
            .collect()
    }
}

/// The fixed-step accumulator.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct Stepper {
    /// 0 for a variable step.
    pub fixed: f32,
    pub max_substeps: u32,
    pub acc: f32,
}

impl Stepper {
    /// How many steps to run for `dt` and how long each is.
    pub fn plan(&mut self, dt: f32) -> (u32, f32) {
        if self.fixed <= 0.0 {
            return (1, dt);
        }
        self.acc += dt;
        let mut n = 0u32;
        while self.acc >= self.fixed && n < self.max_substeps.max(1) {
            self.acc -= self.fixed;
            n += 1;
        }
        if n >= self.max_substeps.max(1) && self.acc >= self.fixed {
            // The debt is dropped: a stalled page must not spiral.
            self.acc = 0.0;
        }
        (n, self.fixed)
    }
}

/// The largest magnitude a position, velocity, force or shape size may have: far beyond any scene
/// (a hundred thousand kilometres) and far below where `f32` arithmetic and the solver's geometry
/// start to overflow.
pub(crate) const MAX_MAGNITUDE: f32 = 1.0e8;

pub(crate) fn finite(what: &str, v: &[f32]) -> crate::Result<()> {
    if v.iter().all(|x| x.is_finite() && x.abs() <= MAX_MAGNITUDE) {
        Ok(())
    } else {
        Err(crate::GraphicsError::invalid(format!("{what} must be finite numbers no larger than 100,000,000")))
    }
}

/// Build a shape inside the geometry library, turning a panic there (it asserts on degenerate
/// input such as collinear points or overflow) into an error: a program's numbers never crash the host.
pub(crate) fn guarded<R>(what: &str, build: impl FnOnce() -> Option<R>) -> crate::Result<R> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(build)) {
        Ok(Some(r)) => Ok(r),
        Ok(None) => Err(crate::GraphicsError::invalid(format!("{what} is not a usable shape (it is degenerate)"))),
        Err(_) => Err(crate::GraphicsError::invalid(format!("{what} could not be built from these numbers"))),
    }
}

pub(crate) fn positive(what: &str, v: f32) -> crate::Result<()> {
    if v.is_finite() && v > 0.0 && v <= MAX_MAGNITUDE {
        Ok(())
    } else {
        Err(crate::GraphicsError::invalid(format!("{what} must be greater than zero")))
    }
}
