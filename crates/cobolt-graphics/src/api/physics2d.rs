// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The methods of Physics2D (WASM-018 R10).

use super::method;
use super::physics3d::{body_kind, shape_options};
use super::{dispatch, done};
use crate::error::Result;
use crate::physics::{BindTarget, CollisionKind, Physics2D};
use crate::registry::{MethodInfo, ObjectKind, Registry};
use crate::value::{Args, Value};

fn v2(a: &Args<'_>, i: usize) -> Result<[f32; 2]> {
    Ok([a.f32(i)?, a.f32(i + 1)?])
}

fn limits(a: &Args<'_>, i: usize) -> Result<Option<[f32; 2]>> {
    match (a.has(i), a.has(i + 1)) {
        (false, false) => Ok(None),
        (true, true) => Ok(Some([a.f32(i)?, a.f32(i + 1)?])),
        _ => Err(a.refuse(i, "give both limits or neither")),
    }
}

/// Copy the transforms of bound bodies to the Scene3D nodes they drive (z = 0, a turn about Z).
#[cfg(feature = "scene3d-core")]
fn apply_bound(o: &mut Physics2D, r: &mut Registry, world: u32) -> Result<()> {
    for b in o.bound(world)? {
        if let BindTarget::Node { scene, node } = b.target {
            let _ = r.with_scene3d(scene, |s, _| {
                s.set_position(node, [b.position[0], b.position[1], 0.0])?;
                let (sn, cs) = (b.angle / 2.0).sin_cos();
                s.set_rotation_quaternion(node, [0.0, 0.0, sn, cs])
            });
        }
    }
    Ok(())
}

#[cfg(not(feature = "scene3d-core"))]
fn apply_bound(_o: &mut Physics2D, _r: &mut Registry, _world: u32) -> Result<()> {
    Ok(())
}

#[allow(unused_variables)]
pub(crate) static METHODS: &[crate::value::Method<Physics2D>] = &[
    method!(Physics2D, "CreateWorld", [gx: Num, gy: Num], [], Handle, "A new world with this gravity (metres per second squared; 0 -9.81 is Earth with y up).", |o, r, a| Ok(o.create_world(v2(a, 0)?)?.into())),
    method!(Physics2D, "DestroyWorld", [world: Handle], [], Nothing, "Release a world and everything in it.", |o, r, a| {
        o.destroy_world(a.handle(0)?)?;
        done()
    }),
    method!(Physics2D, "SetGravity", [world: Handle, gx: Num, gy: Num], [], Nothing, "Change the gravity.", |o, r, a| {
        o.set_gravity(a.handle(0)?, v2(a, 1)?)?;
        done()
    }),
    method!(Physics2D, "SetFixedStep", [world: Handle, seconds: Num], [max_substeps: Int], Nothing, "Step in equal sub-steps of this many seconds (0 for none), at most max_substeps (default 8) per Step call.", |o, r, a| {
        o.set_fixed_step(a.handle(0)?, a.f32(1)?, a.int_or(2, 8)?.clamp(1, 1000) as u32)?;
        done()
    }),
    method!(Physics2D, "SetThreads", [world: Handle, threads: Int], [], Nothing, "Pin the solver to this many threads (0: the default pool). The results do not depend on it.", |o, r, a| {
        o.set_threads(a.handle(0)?, a.usize(1)?)?;
        done()
    }),
    method!(Physics2D, "AddBody", [world: Handle, kind: Int, x: Num, y: Num], [angle: Num], Handle, "A body: kind 0 static, 1 dynamic, 2 kinematic (moved by its velocity); the angle is in radians.", |o, r, a| {
        Ok(o.add_body(a.handle(0)?, body_kind(a, 1)?, v2(a, 2)?, a.num_or(4, 0.0)? as f32)?.into())
    }),
    method!(Physics2D, "RemoveBody", [world: Handle, body: Handle], [], Nothing, "Remove a body, its shapes and its joints.", |o, r, a| {
        o.remove_body(a.handle(0)?, a.handle(1)?)?;
        done()
    }),
    method!(Physics2D, "AddBall", [world: Handle, body: Handle, radius: Num], [density: Num, friction: Num, restitution: Num, sensor: Flag], Handle, "A disc on a body.", |o, r, a| {
        Ok(o.add_ball(a.handle(0)?, a.handle(1)?, a.f32(2)?, &shape_options(a, 3)?)?.into())
    }),
    method!(Physics2D, "AddBox", [world: Handle, body: Handle, hx: Num, hy: Num], [density: Num, friction: Num, restitution: Num, sensor: Flag], Handle, "A rectangle with these half extents on a body.", |o, r, a| {
        Ok(o.add_box(a.handle(0)?, a.handle(1)?, v2(a, 2)?, &shape_options(a, 4)?)?.into())
    }),
    method!(Physics2D, "AddCapsule", [world: Handle, body: Handle, half_height: Num, radius: Num], [density: Num, friction: Num, restitution: Num, sensor: Flag], Handle, "A capsule along Y on a body.", |o, r, a| {
        Ok(o.add_capsule(a.handle(0)?, a.handle(1)?, a.f32(2)?, a.f32(3)?, &shape_options(a, 4)?)?.into())
    }),
    method!(Physics2D, "AddConvexHull", [world: Handle, body: Handle, points: Floats], [density: Num, friction: Num, restitution: Num, sensor: Flag], Handle, "The convex hull of points (x, y ...) on a body.", |o, r, a| {
        let v = a.floats(2)?;
        if v.len() % 2 != 0 {
            return Err(a.refuse(2, "needs x and y for every point, so an even number of values"));
        }
        let pts: Vec<[f32; 2]> = v.chunks_exact(2).map(|c| [c[0] as f32, c[1] as f32]).collect();
        Ok(o.add_convex_hull(a.handle(0)?, a.handle(1)?, &pts, &shape_options(a, 3)?)?.into())
    }),
    method!(Physics2D, "RemoveShape", [world: Handle, shape: Handle], [], Nothing, "Remove one shape.", |o, r, a| {
        o.remove_shape(a.handle(0)?, a.handle(1)?)?;
        done()
    }),
    method!(Physics2D, "SetPosition", [world: Handle, body: Handle, x: Num, y: Num], [], Nothing, "Teleport a body.", |o, r, a| {
        o.set_position(a.handle(0)?, a.handle(1)?, v2(a, 2)?)?;
        done()
    }),
    method!(Physics2D, "SetRotation", [world: Handle, body: Handle, angle: Num], [], Nothing, "Set the angle in radians (counter-clockwise).", |o, r, a| {
        o.set_rotation(a.handle(0)?, a.handle(1)?, a.f32(2)?)?;
        done()
    }),
    method!(Physics2D, "SetVelocity", [world: Handle, body: Handle, x: Num, y: Num], [], Nothing, "Set the velocity.", |o, r, a| {
        o.set_velocity(a.handle(0)?, a.handle(1)?, v2(a, 2)?)?;
        done()
    }),
    method!(Physics2D, "SetAngularVelocity", [world: Handle, body: Handle, radians_per_second: Num], [], Nothing, "Set the angular velocity.", |o, r, a| {
        o.set_angular_velocity(a.handle(0)?, a.handle(1)?, a.f32(2)?)?;
        done()
    }),
    method!(Physics2D, "ApplyForce", [world: Handle, body: Handle, x: Num, y: Num], [], Nothing, "A force (newtons) that acts during the next Step call.", |o, r, a| {
        o.apply_force(a.handle(0)?, a.handle(1)?, v2(a, 2)?)?;
        done()
    }),
    method!(Physics2D, "ApplyTorque", [world: Handle, body: Handle, torque: Num], [], Nothing, "A torque that acts during the next Step call.", |o, r, a| {
        o.apply_torque(a.handle(0)?, a.handle(1)?, a.f32(2)?)?;
        done()
    }),
    method!(Physics2D, "ApplyImpulse", [world: Handle, body: Handle, x: Num, y: Num], [], Nothing, "Change the velocity at once by impulse / mass.", |o, r, a| {
        o.apply_impulse(a.handle(0)?, a.handle(1)?, v2(a, 2)?)?;
        done()
    }),
    method!(Physics2D, "GetPosition", [world: Handle, body: Handle], [], Floats, "x, y.", |o, r, a| Ok(o.get_position(a.handle(0)?, a.handle(1)?)?.to_vec().into())),
    method!(Physics2D, "GetRotation", [world: Handle, body: Handle], [], Num, "The angle in radians.", |o, r, a| Ok(o.get_rotation(a.handle(0)?, a.handle(1)?)?.into())),
    method!(Physics2D, "GetVelocity", [world: Handle, body: Handle], [], Floats, "x, y.", |o, r, a| Ok(o.get_velocity(a.handle(0)?, a.handle(1)?)?.to_vec().into())),
    method!(Physics2D, "GetAngularVelocity", [world: Handle, body: Handle], [], Num, "Radians per second.", |o, r, a| Ok(o.get_angular_velocity(a.handle(0)?, a.handle(1)?)?.into())),
    method!(Physics2D, "IsSleeping", [world: Handle, body: Handle], [], Int, "1 if the solver has put the body to sleep.", |o, r, a| Ok(o.is_sleeping(a.handle(0)?, a.handle(1)?)?.into())),
    method!(Physics2D, "AddFixedJoint", [world: Handle, body_a: Handle, body_b: Handle, ax: Num, ay: Num, bx: Num, by: Num], [], Handle, "Weld two bodies at an anchor given in each body's own space.", |o, r, a| {
        Ok(o.add_fixed_joint(a.handle(0)?, a.handle(1)?, a.handle(2)?, v2(a, 3)?, v2(a, 5)?)?.into())
    }),
    method!(Physics2D, "AddRevoluteJoint", [world: Handle, body_a: Handle, body_b: Handle, ax: Num, ay: Num, bx: Num, by: Num], [min_angle: Num, max_angle: Num], Handle, "A hinge, optionally limited to an angle range in radians.", |o, r, a| {
        Ok(o.add_revolute_joint(a.handle(0)?, a.handle(1)?, a.handle(2)?, v2(a, 3)?, v2(a, 5)?, limits(a, 7)?)?.into())
    }),
    method!(Physics2D, "AddPrismaticJoint", [world: Handle, body_a: Handle, body_b: Handle, ax: Num, ay: Num, bx: Num, by: Num, axis_x: Num, axis_y: Num], [min: Num, max: Num], Handle, "A slider along an axis, optionally limited to a distance range.", |o, r, a| {
        Ok(o.add_prismatic_joint(a.handle(0)?, a.handle(1)?, a.handle(2)?, v2(a, 3)?, v2(a, 5)?, v2(a, 7)?, limits(a, 9)?)?.into())
    }),
    method!(Physics2D, "RemoveJoint", [world: Handle, joint: Handle], [], Nothing, "Remove a joint.", |o, r, a| {
        o.remove_joint(a.handle(0)?, a.handle(1)?)?;
        done()
    }),
    method!(Physics2D, "RayCast", [world: Handle, ox: Num, oy: Num, dx: Num, dy: Num, max_distance: Num], [], Floats, "The first shape the ray meets: hit (1 or 0), body, shape, distance, point x y, normal x y. The direction need not be a unit vector.", |o, r, a| {
        Ok(match o.ray_cast(a.handle(0)?, v2(a, 1)?, v2(a, 3)?, a.f32(5)?)? {
            Some(h) => vec![1.0, h.body as f64, h.shape as f64, h.distance as f64, h.point[0] as f64, h.point[1] as f64, h.normal[0] as f64, h.normal[1] as f64].into(),
            None => vec![0.0; 8].into(),
        })
    }),
    method!(Physics2D, "Step", [world: Handle, seconds: Num], [], Int, "Advance by this many seconds (at most 1); with a fixed step it runs whole sub-steps. Bound Scene3D nodes follow their bodies. The answer is how many steps ran.", |o, r, a| {
        let world = a.handle(0)?;
        let steps = o.step(world, a.f32(1)?)?.steps;
        apply_bound(o, r, world)?;
        Ok(steps.into())
    }),
    method!(Physics2D, "TakeEvents", [world: Handle], [], Floats, "The collision events since the last call, six numbers each: 0 started or 1 stopped, body a, body b, shape a, shape b, 1 if a sensor was involved.", |o, r, a| {
        let mut out = Vec::new();
        for e in o.take_events(a.handle(0)?)? {
            out.extend([(e.kind == CollisionKind::Stopped) as u8 as f64, e.body_a as f64, e.body_b as f64, e.shape_a as f64, e.shape_b as f64, e.sensor as u8 as f64]);
        }
        Ok(out.into())
    }),
    method!(Physics2D, "BindSprite", [world: Handle, body: Handle, sprite: Int], [], Nothing, "Make a body drive a sprite the host knows by this number (read it back with GetBound).", |o, r, a| {
        o.bind(a.handle(0)?, a.handle(1)?, BindTarget::Sprite(a.u32(2)?))?;
        done()
    }),
    method!(Physics2D, "BindNode", [world: Handle, body: Handle, scene: Handle, node: Handle], [], Nothing, "Make a body drive a Scene3D node (z = 0, turning about Z): after every Step the node is where the body is.", |o, r, a| {
        o.bind(a.handle(0)?, a.handle(1)?, BindTarget::Node { scene: a.handle(2)?, node: a.handle(3)? })?;
        done()
    }),
    method!(Physics2D, "Unbind", [world: Handle, body: Handle], [], Nothing, "Stop a body driving anything.", |o, r, a| {
        o.unbind(a.handle(0)?, a.handle(1)?)?;
        done()
    }),
    method!(Physics2D, "GetBound", [world: Handle], [], Floats, "The bound bodies, seven numbers each: body, 0 for a sprite or 1 for a node, the sprite number (or the scene handle), 0 (or the node handle), x, y, angle.", |o, r, a| {
        let mut out = Vec::new();
        for b in o.bound(a.handle(0)?)? {
            let (k, t1, t2) = match b.target {
                BindTarget::Sprite(s) => (0.0, s as f64, 0.0),
                BindTarget::Node { scene, node } => (1.0, scene as f64, node as f64),
            };
            out.extend([b.body as f64, k, t1, t2, b.position[0] as f64, b.position[1] as f64, b.angle as f64]);
        }
        Ok(out.into())
    }),
    method!(Physics2D, "GetBodyCount", [world: Handle], [], Int, "Bodies in a world.", |o, r, a| Ok(o.body_count(a.handle(0)?)?.into())),
    method!(Physics2D, "GetShapeCount", [world: Handle], [], Int, "Shapes in a world.", |o, r, a| Ok(o.shape_count(a.handle(0)?)?.into())),
    method!(Physics2D, "GetJointCount", [world: Handle], [], Int, "Joints in a world.", |o, r, a| Ok(o.joint_count(a.handle(0)?)?.into())),
    method!(Physics2D, "GetWorldCount", [], [], Int, "Worlds in this object.", |o, r, a| Ok(o.world_count().into())),
    method!(Physics2D, "GetStateFingerprint", [world: Handle], [], Int, "A number that is the same for two runs that ended in exactly the same state (positions, angles, velocities, to the bit).", |o, r, a| Ok((o.state_fingerprint(a.handle(0)?)? as i64).into())),
];

pub(crate) fn call(o: &mut Physics2D, reg: &mut Registry, method: &str, args: &[Value]) -> Result<Value> {
    dispatch(METHODS, "Physics2D", o, reg, method, args)
}

pub(crate) fn infos() -> Vec<MethodInfo> {
    super::infos(ObjectKind::Physics2D, METHODS)
}
