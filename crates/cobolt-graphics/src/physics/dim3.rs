// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Physics3D on `rapier3d`. See the [module documentation](super).

use rapier3d::dynamics::{
    FixedJointBuilder, ImpulseJointHandle, PrismaticJointBuilder, RevoluteJointBuilder, RigidBodyBuilder, RigidBodyHandle,
};
use rapier3d::geometry::{ColliderBuilder, ColliderHandle, CollisionEvent as RCollisionEvent, Ray};
use rapier3d::math::{Rotation, Vector};
use rapier3d::pipeline::{ActiveEvents, EventHandler, PhysicsWorld, QueryFilter};

use super::{
    finite, guarded, positive, BindTarget, BodyKind, Bound3, Collector, CollisionEvent, CollisionKind, RawEvent, RayHit, ShapeOptions, StepReport,
    Stepper,
};
use crate::error::{GraphicsError, Result};
use crate::handles::{Handle, HandleTable};
use crate::limits::Limits;

/// A body's bookkeeping next to the solver's.
struct Body {
    rb: RigidBodyHandle,
    kind: BodyKind,
    bound: Option<BindTarget>,
}

struct Shape {
    co: ColliderHandle,
    body: Handle,
}

struct Joint {
    jh: ImpulseJointHandle,
    a: Handle,
    b: Handle,
}

struct World {
    pw: PhysicsWorld,
    bodies: HandleTable<Body>,
    shapes: HandleTable<Shape>,
    joints: HandleTable<Joint>,
    collector: Collector,
    stepper: Stepper,
    /// Bodies with a force or torque to clear after the next step.
    forced: Vec<Handle>,
    /// Bodies or shapes were added or moved since the solver last updated its queries.
    dirty: bool,
}

struct Sink<'a>(&'a Collector);

impl EventHandler for Sink<'_> {
    fn handle_collision_event(
        &self,
        bodies: &rapier3d::dynamics::RigidBodySet,
        colliders: &rapier3d::geometry::ColliderSet,
        event: RCollisionEvent,
        _pair: Option<&rapier3d::geometry::ContactPair>,
    ) {
        let (Some(c1), Some(c2)) = (colliders.get(event.collider1()), colliders.get(event.collider2())) else { return };
        let body_of = |c: &rapier3d::geometry::Collider| c.parent().and_then(|p| bodies.get(p)).map(|b| b.user_data as u32).unwrap_or(0);
        self.0.push(RawEvent {
            kind: if event.started() { CollisionKind::Started } else { CollisionKind::Stopped },
            body_a: body_of(c1),
            body_b: body_of(c2),
            shape_a: c1.user_data as u32,
            shape_b: c2.user_data as u32,
            sensor: event.sensor(),
        });
    }

    fn handle_contact_force_event(
        &self,
        _dt: f32,
        _bodies: &rapier3d::dynamics::RigidBodySet,
        _colliders: &rapier3d::geometry::ColliderSet,
        _pair: &rapier3d::geometry::ContactPair,
        _total: f32,
    ) {
    }

    fn handle_soft_body_tear_event(&self, _soft: &rapier3d::dynamics::SoftBodySet, _event: &rapier3d::dynamics::SoftBodyTearEvent) {}
}

/// The Physics3D object: any number of worlds.
pub struct Physics3D {
    limits: Limits,
    worlds: HandleTable<World>,
}

impl std::fmt::Debug for Physics3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Physics3D({} world(s))", self.worlds.len())
    }
}

fn v3(a: [f32; 3]) -> Vector {
    Vector::new(a[0], a[1], a[2])
}

fn quat(q: [f32; 4]) -> Result<Rotation> {
    finite("a rotation", &q)?;
    let r = Rotation::from_xyzw(q[0], q[1], q[2], q[3]);
    if r.length_squared() < 1.0e-12 {
        return Err(GraphicsError::invalid("a rotation quaternion cannot be all zero"));
    }
    Ok(r.normalize())
}

fn unit_axis(axis: [f32; 3]) -> Result<Vector> {
    finite("an axis", &axis)?;
    let a = v3(axis);
    if a.length_squared() < 1.0e-12 {
        return Err(GraphicsError::invalid("an axis cannot be the zero vector"));
    }
    Ok(a.normalize())
}

impl Physics3D {
    /// An object with no worlds.
    pub fn new(limits: Limits) -> Self {
        Self { limits, worlds: HandleTable::new("world") }
    }

    /// `CreateWorld`: a world with this gravity (metres per second squared; `[0, -9.81, 0]` is Earth).
    pub fn create_world(&mut self, gravity: [f32; 3]) -> Result<Handle> {
        finite("gravity", &gravity)?;
        let mut pw = PhysicsWorld::new();
        pw.gravity = v3(gravity);
        self.worlds.insert(World {
            pw,
            bodies: HandleTable::new("body"),
            shapes: HandleTable::new("shape"),
            joints: HandleTable::new("joint"),
            collector: Collector::default(),
            stepper: Stepper { fixed: 0.0, max_substeps: 8, acc: 0.0 },
            forced: Vec::new(),
            dirty: false,
        })
    }

    /// `DestroyWorld`: release a world and everything in it.
    pub fn destroy_world(&mut self, world: Handle) -> Result<()> {
        self.worlds.remove(world).map(|_| ())
    }

    /// How many worlds are live.
    pub fn world_count(&self) -> usize {
        self.worlds.len()
    }

    fn w(&mut self, world: Handle) -> Result<&mut World> {
        self.worlds.get_mut(world)
    }

    /// `SetGravity`.
    pub fn set_gravity(&mut self, world: Handle, gravity: [f32; 3]) -> Result<()> {
        finite("gravity", &gravity)?;
        let w = self.w(world)?;
        w.pw.gravity = v3(gravity);
        w.pw.wake_up_all(true);
        Ok(())
    }

    /// `SetFixedStep`: `step_seconds` of 0 turns the fixed step off.
    pub fn set_fixed_step(&mut self, world: Handle, step_seconds: f32, max_substeps: u32) -> Result<()> {
        if step_seconds != 0.0 {
            positive("the fixed step", step_seconds)?;
            if step_seconds > 1.0 {
                return Err(GraphicsError::invalid("a fixed step is at most one second"));
            }
        }
        let w = self.w(world)?;
        w.stepper = Stepper { fixed: step_seconds, max_substeps: max_substeps.clamp(1, 1000), acc: 0.0 };
        Ok(())
    }

    /// Pin the solver's parallel work of this world to a pool of `threads` threads
    /// (0 returns to the default pool). The results do not depend on the number; a
    /// test uses this to prove it. Without the `parallel` feature it does nothing.
    pub fn set_threads(&mut self, world: Handle, threads: usize) -> Result<()> {
        if threads > 256 {
            return Err(GraphicsError::invalid("a world uses at most 256 threads"));
        }
        #[cfg(feature = "parallel")]
        {
            let w = self.w(world)?;
            if threads == 0 {
                w.pw.clear_thread_pool();
            } else {
                w.pw.configure_thread_pool(threads).map_err(|e| GraphicsError::Unsupported(format!("a thread pool of {threads}: {e}")))?;
            }
            Ok(())
        }
        #[cfg(not(feature = "parallel"))]
        {
            let _ = threads;
            self.w(world).map(|_| ())
        }
    }

    // ---- bodies ----------------------------------------------------------

    /// `AddBody`: a body of this kind at a position and orientation (quaternion `[x, y, z, w]`).
    pub fn add_body(&mut self, world: Handle, kind: BodyKind, position: [f32; 3], rotation: [f32; 4]) -> Result<Handle> {
        finite("a position", &position)?;
        let rot = quat(rotation)?;
        let limits = self.limits.clone();
        let w = self.w(world)?;
        limits.check_bodies(w.bodies.len() as u64 + 1)?;
        let builder = match kind {
            BodyKind::Static => RigidBodyBuilder::fixed(),
            BodyKind::Dynamic => RigidBodyBuilder::dynamic(),
            BodyKind::Kinematic => RigidBodyBuilder::kinematic_velocity_based(),
        };
        let rb = w.pw.insert_body(builder.translation(v3(position)).pose(rapier3d::math::Pose::from_parts(v3(position), rot)));
        let h = w.bodies.insert(Body { rb, kind, bound: None })?;
        w.pw.bodies[rb].user_data = h as u128;
        w.dirty = true;
        Ok(h)
    }

    /// `RemoveBody`: the body, its shapes and the joints that touch it.
    pub fn remove_body(&mut self, world: Handle, body: Handle) -> Result<()> {
        let w = self.w(world)?;
        let rb = w.bodies.get(body)?.rb;
        w.pw.remove_body(rb);
        w.bodies.remove(body)?;
        let dead_shapes: Vec<Handle> = w.shapes.iter().filter(|(_, s)| s.body == body).map(|(h, _)| h).collect();
        for h in dead_shapes {
            let _ = w.shapes.remove(h);
        }
        let dead_joints: Vec<Handle> = w.joints.iter().filter(|(_, j)| j.a == body || j.b == body).map(|(h, _)| h).collect();
        for h in dead_joints {
            let _ = w.joints.remove(h);
        }
        w.forced.retain(|b| *b != body);
        w.dirty = true;
        Ok(())
    }

    /// How many bodies a world holds.
    pub fn body_count(&self, world: Handle) -> Result<usize> {
        Ok(self.worlds.get(world)?.bodies.len())
    }

    /// How many shapes a world holds.
    pub fn shape_count(&self, world: Handle) -> Result<usize> {
        Ok(self.worlds.get(world)?.shapes.len())
    }

    fn add_shape(&mut self, world: Handle, body: Handle, shape: ColliderBuilder, o: &ShapeOptions) -> Result<Handle> {
        o.check()?;
        let w = self.w(world)?;
        let b = w.bodies.get(body)?;
        let rb = b.rb;
        let collider = shape
            .density(o.density)
            .friction(o.friction)
            .restitution(o.restitution)
            .sensor(o.sensor)
            .active_events(ActiveEvents::COLLISION_EVENTS)
            .build();
        let co = w.pw.insert_collider(collider, Some(rb));
        let h = match w.shapes.insert(Shape { co, body }) {
            Ok(h) => h,
            Err(e) => {
                w.pw.remove_collider(co);
                return Err(e);
            }
        };
        w.pw.colliders[co].user_data = h as u128;
        w.dirty = true;
        Ok(h)
    }

    /// `AddBall`: a sphere of this radius centred on the body.
    pub fn add_ball(&mut self, world: Handle, body: Handle, radius: f32, o: &ShapeOptions) -> Result<Handle> {
        positive("a radius", radius)?;
        self.add_shape(world, body, ColliderBuilder::ball(radius), o)
    }

    /// `AddBox`: a box with these half extents centred on the body.
    pub fn add_box(&mut self, world: Handle, body: Handle, half_extents: [f32; 3], o: &ShapeOptions) -> Result<Handle> {
        for h in half_extents {
            positive("a half extent", h)?;
        }
        self.add_shape(world, body, ColliderBuilder::cuboid(half_extents[0], half_extents[1], half_extents[2]), o)
    }

    /// `AddCapsule`: a capsule along the body's Y axis: a cylinder of this half height capped by hemispheres of this radius.
    pub fn add_capsule(&mut self, world: Handle, body: Handle, half_height: f32, radius: f32, o: &ShapeOptions) -> Result<Handle> {
        positive("a radius", radius)?;
        if !(half_height.is_finite() && half_height >= 0.0) {
            return Err(GraphicsError::invalid("a capsule's half height cannot be negative"));
        }
        self.add_shape(world, body, ColliderBuilder::capsule_y(half_height, radius), o)
    }

    /// `AddConvexHull`: the convex hull of a point cloud, in the body's space.
    pub fn add_convex_hull(&mut self, world: Handle, body: Handle, points: &[[f32; 3]], o: &ShapeOptions) -> Result<Handle> {
        self.limits.check_path_points(points.len() as u64)?;
        if points.len() < 4 {
            return Err(GraphicsError::invalid("a convex hull needs at least four points"));
        }
        for p in points {
            finite("a point", p)?;
        }
        let pts: Vec<Vector> = points.iter().map(|p| v3(*p)).collect();
        let b = guarded("the convex hull (it needs at least four points that are not in one plane)", || ColliderBuilder::convex_hull(&pts))?;
        self.add_shape(world, body, b, o)
    }

    /// `AddTrimesh`: a triangle mesh (use it on static bodies; a dynamic body should use a hull).
    pub fn add_trimesh(&mut self, world: Handle, body: Handle, vertices: &[[f32; 3]], triangles: &[[u32; 3]], o: &ShapeOptions) -> Result<Handle> {
        self.limits.check_path_points(vertices.len() as u64)?;
        self.limits.check_triangles(triangles.len() as u64)?;
        for p in vertices {
            finite("a vertex", p)?;
        }
        if triangles.is_empty() {
            return Err(GraphicsError::invalid("a triangle mesh needs at least one triangle"));
        }
        if triangles.iter().any(|t| t.iter().any(|&i| i as usize >= vertices.len())) {
            return Err(GraphicsError::invalid("a triangle refers to a vertex that does not exist"));
        }
        let b = guarded("the triangle mesh", || ColliderBuilder::trimesh(vertices.iter().map(|p| v3(*p)).collect(), triangles.to_vec()).ok())?;
        self.add_shape(world, body, b, o)
    }

    /// `RemoveShape`.
    pub fn remove_shape(&mut self, world: Handle, shape: Handle) -> Result<()> {
        let w = self.w(world)?;
        let s = w.shapes.remove(shape)?;
        w.pw.remove_collider(s.co);
        w.dirty = true;
        Ok(())
    }

    fn rb_mut(w: &mut World, body: Handle) -> Result<&mut rapier3d::dynamics::RigidBody> {
        let rb = w.bodies.get(body)?.rb;
        w.pw.bodies.get_mut(rb).ok_or(GraphicsError::UnknownHandle(body))
    }

    /// `SetPosition` (a teleport).
    pub fn set_position(&mut self, world: Handle, body: Handle, position: [f32; 3]) -> Result<()> {
        finite("a position", &position)?;
        let w = self.w(world)?;
        Self::rb_mut(w, body)?.set_translation(v3(position), true);
        w.dirty = true;
        Ok(())
    }

    /// `SetRotation` (quaternion `[x, y, z, w]`).
    pub fn set_rotation(&mut self, world: Handle, body: Handle, rotation: [f32; 4]) -> Result<()> {
        let r = quat(rotation)?;
        let w = self.w(world)?;
        Self::rb_mut(w, body)?.set_rotation(r, true);
        w.dirty = true;
        Ok(())
    }

    /// `SetVelocity`.
    pub fn set_velocity(&mut self, world: Handle, body: Handle, velocity: [f32; 3]) -> Result<()> {
        finite("a velocity", &velocity)?;
        let w = self.w(world)?;
        Self::rb_mut(w, body)?.set_linvel(v3(velocity), true);
        Ok(())
    }

    /// `SetAngularVelocity` (radians per second about each axis).
    pub fn set_angular_velocity(&mut self, world: Handle, body: Handle, velocity: [f32; 3]) -> Result<()> {
        finite("an angular velocity", &velocity)?;
        let w = self.w(world)?;
        Self::rb_mut(w, body)?.set_angvel(v3(velocity), true);
        Ok(())
    }

    /// `ApplyForce`: a force (newtons) that acts during the next `Step` call.
    pub fn apply_force(&mut self, world: Handle, body: Handle, force: [f32; 3]) -> Result<()> {
        finite("a force", &force)?;
        let w = self.w(world)?;
        Self::rb_mut(w, body)?.add_force(v3(force), true);
        if !w.forced.contains(&body) {
            w.forced.push(body);
        }
        Ok(())
    }

    /// `ApplyTorque`: a torque that acts during the next `Step` call.
    pub fn apply_torque(&mut self, world: Handle, body: Handle, torque: [f32; 3]) -> Result<()> {
        finite("a torque", &torque)?;
        let w = self.w(world)?;
        Self::rb_mut(w, body)?.add_torque(v3(torque), true);
        if !w.forced.contains(&body) {
            w.forced.push(body);
        }
        Ok(())
    }

    /// `ApplyImpulse`: change the velocity at once by `impulse / mass`.
    pub fn apply_impulse(&mut self, world: Handle, body: Handle, impulse: [f32; 3]) -> Result<()> {
        finite("an impulse", &impulse)?;
        let w = self.w(world)?;
        Self::rb_mut(w, body)?.apply_impulse(v3(impulse), true);
        Ok(())
    }

    fn rb(&self, world: Handle, body: Handle) -> Result<&rapier3d::dynamics::RigidBody> {
        let w = self.worlds.get(world)?;
        let rb = w.bodies.get(body)?.rb;
        w.pw.bodies.get(rb).ok_or(GraphicsError::UnknownHandle(body))
    }

    /// `GetPosition`.
    pub fn get_position(&self, world: Handle, body: Handle) -> Result<[f32; 3]> {
        Ok(self.rb(world, body)?.translation().to_array())
    }

    /// `GetRotation`: the unit quaternion `[x, y, z, w]`.
    pub fn get_rotation(&self, world: Handle, body: Handle) -> Result<[f32; 4]> {
        Ok(self.rb(world, body)?.rotation().to_array())
    }

    /// `GetVelocity`.
    pub fn get_velocity(&self, world: Handle, body: Handle) -> Result<[f32; 3]> {
        Ok(self.rb(world, body)?.linvel().to_array())
    }

    /// `GetAngularVelocity`.
    pub fn get_angular_velocity(&self, world: Handle, body: Handle) -> Result<[f32; 3]> {
        Ok(self.rb(world, body)?.angvel().to_array())
    }

    /// Whether the solver has put the body to sleep (nothing moves it).
    pub fn is_sleeping(&self, world: Handle, body: Handle) -> Result<bool> {
        Ok(self.rb(world, body)?.is_sleeping())
    }

    /// The kind a body was created with.
    pub fn body_kind(&self, world: Handle, body: Handle) -> Result<BodyKind> {
        Ok(self.worlds.get(world)?.bodies.get(body)?.kind)
    }

    // ---- joints ----------------------------------------------------------

    fn add_joint(&mut self, world: Handle, a: Handle, b: Handle, joint: impl Into<rapier3d::dynamics::GenericJoint>) -> Result<Handle> {
        if a == b {
            return Err(GraphicsError::invalid("a joint connects two different bodies"));
        }
        let w = self.w(world)?;
        let (ra, rb) = (w.bodies.get(a)?.rb, w.bodies.get(b)?.rb);
        let jh = w.pw.insert_impulse_joint(ra, rb, joint);
        match w.joints.insert(Joint { jh, a, b }) {
            Ok(h) => Ok(h),
            Err(e) => {
                w.pw.remove_impulse_joint(jh);
                Err(e)
            }
        }
    }

    /// `AddFixedJoint`: weld two bodies at an anchor point given in each body's own space.
    pub fn add_fixed_joint(&mut self, world: Handle, a: Handle, b: Handle, anchor_a: [f32; 3], anchor_b: [f32; 3]) -> Result<Handle> {
        finite("an anchor", &anchor_a)?;
        finite("an anchor", &anchor_b)?;
        self.add_joint(world, a, b, FixedJointBuilder::new().local_anchor1(v3(anchor_a)).local_anchor2(v3(anchor_b)))
    }

    /// `AddRevoluteJoint`: a hinge about `axis` (the same axis in both bodies' space), optionally limited to `limits` radians.
    pub fn add_revolute_joint(
        &mut self,
        world: Handle,
        a: Handle,
        b: Handle,
        anchor_a: [f32; 3],
        anchor_b: [f32; 3],
        axis: [f32; 3],
        limits: Option<[f32; 2]>,
    ) -> Result<Handle> {
        finite("an anchor", &anchor_a)?;
        finite("an anchor", &anchor_b)?;
        let mut j = RevoluteJointBuilder::new(unit_axis(axis)?).local_anchor1(v3(anchor_a)).local_anchor2(v3(anchor_b));
        if let Some(l) = limits {
            finite("a limit", &l)?;
            j = j.limits(l);
        }
        self.add_joint(world, a, b, j)
    }

    /// `AddPrismaticJoint`: a slider along `axis`, optionally limited to `limits` (distance).
    pub fn add_prismatic_joint(
        &mut self,
        world: Handle,
        a: Handle,
        b: Handle,
        anchor_a: [f32; 3],
        anchor_b: [f32; 3],
        axis: [f32; 3],
        limits: Option<[f32; 2]>,
    ) -> Result<Handle> {
        finite("an anchor", &anchor_a)?;
        finite("an anchor", &anchor_b)?;
        let mut j = PrismaticJointBuilder::new(unit_axis(axis)?).local_anchor1(v3(anchor_a)).local_anchor2(v3(anchor_b));
        if let Some(l) = limits {
            finite("a limit", &l)?;
            j = j.limits(l);
        }
        self.add_joint(world, a, b, j)
    }

    /// `RemoveJoint`.
    pub fn remove_joint(&mut self, world: Handle, joint: Handle) -> Result<()> {
        let w = self.w(world)?;
        let j = w.joints.remove(joint)?;
        w.pw.remove_impulse_joint(j.jh);
        Ok(())
    }

    /// How many joints a world holds.
    pub fn joint_count(&self, world: Handle) -> Result<usize> {
        Ok(self.worlds.get(world)?.joints.len())
    }

    // ---- queries ---------------------------------------------------------

    /// `RayCast`: the first shape the ray meets within `max_distance`, if any. The
    /// direction need not be a unit vector.
    pub fn ray_cast(&mut self, world: Handle, origin: [f32; 3], direction: [f32; 3], max_distance: f32) -> Result<Option<RayHit<3>>> {
        finite("an origin", &origin)?;
        let dir = unit_axis(direction)?;
        positive("the maximum distance", max_distance)?;
        let w = self.w(world)?;
        if w.dirty {
            // Bring the solver's spatial index up to date with what the program changed; contacts found
            // now are reported with the next events.
            w.pw.detect_collisions(&(), &Sink(&w.collector));
            w.dirty = false;
        }
        let ray = Ray::new(v3(origin), dir);
        let Some((co, hit)) = w.pw.cast_ray_and_get_normal(&ray, max_distance, true, QueryFilter::default()) else { return Ok(None) };
        let shape = w.pw.colliders.get(co).map(|c| c.user_data as u32).unwrap_or(0);
        let body = w.shapes.get(shape).map(|s| s.body).unwrap_or(0);
        Ok(Some(RayHit {
            body,
            shape,
            distance: hit.time_of_impact,
            point: ray.point_at(hit.time_of_impact).to_array(),
            normal: hit.normal.to_array(),
        }))
    }

    // ---- stepping and events ---------------------------------------------

    /// `Step`: advance by `dt` seconds (see the module notes on fixed steps). `dt` is at most 1.
    pub fn step(&mut self, world: Handle, dt: f32) -> Result<StepReport> {
        positive("the time step", dt)?;
        if dt > 1.0 {
            return Err(GraphicsError::invalid("one Step advances at most one second; call it more often"));
        }
        let w = self.w(world)?;
        let (n, h) = w.stepper.plan(dt);
        for _ in 0..n {
            w.pw.integration_parameters.dt = h;
            w.pw.step_with_events(&(), &Sink(&w.collector));
        }
        if n > 0 {
            for body in std::mem::take(&mut w.forced) {
                if let Ok(rb) = Self::rb_mut(w, body) {
                    rb.reset_forces(false);
                    rb.reset_torques(false);
                }
            }
            w.dirty = false;
        }
        Ok(StepReport { steps: n })
    }

    /// `TakeEvents`: the collision events since the last call, in a fixed order, and clear them.
    pub fn take_events(&mut self, world: Handle) -> Result<Vec<CollisionEvent>> {
        Ok(self.w(world)?.collector.drain_sorted())
    }

    // ---- binding ---------------------------------------------------------

    /// `Bind`: make a body drive a sprite or a Scene3D node (see [`Physics3D::bound`]).
    pub fn bind(&mut self, world: Handle, body: Handle, target: BindTarget) -> Result<()> {
        self.w(world)?.bodies.get_mut(body)?.bound = Some(target);
        Ok(())
    }

    /// `Unbind`.
    pub fn unbind(&mut self, world: Handle, body: Handle) -> Result<()> {
        self.w(world)?.bodies.get_mut(body)?.bound = None;
        Ok(())
    }

    /// The transforms of every bound body, in handle order, for the host to copy to the targets.
    pub fn bound(&self, world: Handle) -> Result<Vec<Bound3>> {
        let w = self.worlds.get(world)?;
        let mut out = Vec::new();
        for (h, b) in w.bodies.iter() {
            let Some(target) = b.bound else { continue };
            let Some(rb) = w.pw.bodies.get(b.rb) else { continue };
            out.push(Bound3 { body: h, target, position: rb.translation().to_array(), rotation: rb.rotation().to_array() });
        }
        Ok(out)
    }

    /// A fingerprint of every body's position, rotation and velocities in handle
    /// order, bit for bit: two runs that agree on it agree on everything the
    /// program can see. The determinism tests compare it.
    pub fn state_fingerprint(&self, world: Handle) -> Result<u64> {
        let w = self.worlds.get(world)?;
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut mix = |v: f32| {
            for b in v.to_bits().to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        for (_, b) in w.bodies.iter() {
            let Some(rb) = w.pw.bodies.get(b.rb) else { continue };
            for v in rb.translation().to_array() {
                mix(v);
            }
            for v in rb.rotation().to_array() {
                mix(v);
            }
            for v in rb.linvel().to_array() {
                mix(v);
            }
            for v in rb.angvel().to_array() {
                mix(v);
            }
        }
        Ok(h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> (Physics3D, Handle) {
        let mut p = Physics3D::new(Limits::default());
        let w = p.create_world([0.0, -9.81, 0.0]).unwrap();
        (p, w)
    }

    #[test]
    fn a_ball_falls_and_rests_on_the_floor() {
        let (mut p, w) = world();
        let floor = p.add_body(w, BodyKind::Static, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]).unwrap();
        p.add_box(w, floor, [10.0, 0.5, 10.0], &ShapeOptions::default()).unwrap();
        let ball = p.add_body(w, BodyKind::Dynamic, [0.0, 5.0, 0.0], [0.0, 0.0, 0.0, 1.0]).unwrap();
        p.add_ball(w, ball, 0.5, &ShapeOptions::default()).unwrap();
        for _ in 0..240 {
            p.step(w, 1.0 / 60.0).unwrap();
        }
        let y = p.get_position(w, ball).unwrap()[1];
        assert!((y - 1.0).abs() < 0.02, "the ball rests on top of the floor (0.5 + 0.5): y = {y}");
    }

    #[test]
    fn handles_follow_the_body_to_the_grave() {
        let (mut p, w) = world();
        let a = p.add_body(w, BodyKind::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]).unwrap();
        let b = p.add_body(w, BodyKind::Dynamic, [1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]).unwrap();
        let s = p.add_ball(w, a, 0.5, &ShapeOptions::default()).unwrap();
        let j = p.add_fixed_joint(w, a, b, [0.5, 0.0, 0.0], [-0.5, 0.0, 0.0]).unwrap();
        assert_eq!((p.body_count(w).unwrap(), p.shape_count(w).unwrap(), p.joint_count(w).unwrap()), (2, 1, 1));
        p.remove_body(w, a).unwrap();
        assert_eq!((p.body_count(w).unwrap(), p.shape_count(w).unwrap(), p.joint_count(w).unwrap()), (1, 0, 0));
        assert_eq!(p.get_position(w, a), Err(GraphicsError::ReleasedHandle(a)));
        assert_eq!(p.remove_shape(w, s), Err(GraphicsError::ReleasedHandle(s)));
        assert_eq!(p.remove_joint(w, j), Err(GraphicsError::ReleasedHandle(j)));
        p.destroy_world(w).unwrap();
        assert_eq!(p.step(w, 0.01), Err(GraphicsError::ReleasedHandle(w)));
    }

    #[test]
    fn the_body_limit_is_enforced() {
        let mut p = Physics3D::new(Limits { max_bodies: 3, ..Limits::default() });
        let w = p.create_world([0.0; 3]).unwrap();
        for _ in 0..3 {
            p.add_body(w, BodyKind::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]).unwrap();
        }
        let e = p.add_body(w, BodyKind::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]).unwrap_err();
        assert!(e.to_string().contains("max_bodies"), "{e}");
    }

    #[test]
    fn nonsense_is_refused_not_simulated() {
        let (mut p, w) = world();
        let b = p.add_body(w, BodyKind::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]).unwrap();
        assert!(p.add_ball(w, b, 0.0, &ShapeOptions::default()).is_err());
        assert!(p.add_ball(w, b, f32::NAN, &ShapeOptions::default()).is_err());
        assert!(p.add_box(w, b, [1.0, -1.0, 1.0], &ShapeOptions::default()).is_err());
        assert!(p.set_position(w, b, [f32::INFINITY, 0.0, 0.0]).is_err());
        assert!(p.set_rotation(w, b, [0.0; 4]).is_err());
        assert!(p.add_convex_hull(w, b, &[[0.0; 3]; 4], &ShapeOptions::default()).is_err());
        assert!(p.add_trimesh(w, b, &[[0.0; 3]; 3], &[[0, 1, 5]], &ShapeOptions::default()).is_err());
        assert!(p.step(w, 0.0).is_err());
        assert!(p.step(w, 5.0).is_err());
        assert!(p.ray_cast(w, [0.0; 3], [0.0; 3], 10.0).is_err());
        assert!(p.add_fixed_joint(w, b, b, [0.0; 3], [0.0; 3]).is_err());
        assert!(p.set_fixed_step(w, -1.0, 4).is_err());
        let bad = ShapeOptions { density: -1.0, ..Default::default() };
        assert!(p.add_ball(w, b, 1.0, &bad).is_err());
    }

    #[test]
    fn a_fixed_step_runs_whole_steps_and_forgets_a_stall() {
        let (mut p, w) = world();
        p.set_fixed_step(w, 0.01, 5).unwrap();
        assert_eq!(p.step(w, 0.004).unwrap().steps, 0);
        assert_eq!(p.step(w, 0.004).unwrap().steps, 0);
        assert_eq!(p.step(w, 0.004).unwrap().steps, 1, "0.012 s accumulated: one step of 0.01");
        assert_eq!(p.step(w, 0.5).unwrap().steps, 5, "capped at max_substeps");
        assert_eq!(p.step(w, 0.004).unwrap().steps, 0, "and the rest of the debt was dropped");
    }

    #[test]
    fn a_force_acts_for_one_step_call_and_an_impulse_at_once() {
        let mut p = Physics3D::new(Limits::default());
        let w = p.create_world([0.0; 3]).unwrap();
        let b = p.add_body(w, BodyKind::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]).unwrap();
        p.add_ball(w, b, 0.5, &ShapeOptions { density: 1.0 / (4.0 / 3.0 * std::f32::consts::PI * 0.125), ..Default::default() }).unwrap();
        p.apply_impulse(w, b, [2.0, 0.0, 0.0]).unwrap();
        assert!((p.get_velocity(w, b).unwrap()[0] - 2.0).abs() < 1e-3, "mass 1, impulse 2: v = 2 at once");
        p.apply_force(w, b, [0.0, 10.0, 0.0]).unwrap();
        p.step(w, 0.1).unwrap();
        let vy = p.get_velocity(w, b).unwrap()[1];
        assert!((vy - 1.0).abs() < 1e-2, "force 10 for 0.1 s on mass 1: dv = 1, got {vy}");
        p.step(w, 0.1).unwrap();
        assert!((p.get_velocity(w, b).unwrap()[1] - vy).abs() < 1e-4, "the force was cleared after the step");
    }

    #[test]
    fn a_ray_finds_the_first_shape() {
        let (mut p, w) = world();
        let a = p.add_body(w, BodyKind::Static, [0.0, 0.0, 5.0], [0.0, 0.0, 0.0, 1.0]).unwrap();
        let sa = p.add_box(w, a, [1.0, 1.0, 1.0], &ShapeOptions::default()).unwrap();
        let b = p.add_body(w, BodyKind::Static, [0.0, 0.0, 10.0], [0.0, 0.0, 0.0, 1.0]).unwrap();
        p.add_box(w, b, [1.0, 1.0, 1.0], &ShapeOptions::default()).unwrap();
        let hit = p.ray_cast(w, [0.0, 0.0, 0.0], [0.0, 0.0, 2.0], 100.0).unwrap().expect("a hit");
        assert_eq!((hit.body, hit.shape), (a, sa));
        assert!((hit.distance - 4.0).abs() < 1e-4, "{}", hit.distance);
        assert!((hit.point[2] - 4.0).abs() < 1e-4);
        assert!((hit.normal[2] + 1.0).abs() < 1e-4);
        assert!(p.ray_cast(w, [0.0, 0.0, 0.0], [0.0, 0.0, 2.0], 3.0).unwrap().is_none(), "too short");
        assert!(p.ray_cast(w, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 100.0).unwrap().is_none(), "the wrong way");
    }
}
