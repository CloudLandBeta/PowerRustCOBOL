// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The methods of Physics3D (WASM-018 R10).

use super::{dispatch, done, method};
use crate::error::Result;
use crate::physics::{BindTarget, BodyKind, CollisionKind, Physics3D, ShapeOptions};
use crate::registry::{MethodInfo, ObjectKind, Registry};
use crate::value::{Args, Value};

pub(crate) fn shape_options(a: &Args<'_>, first: usize) -> Result<ShapeOptions> {
    let d = ShapeOptions::default();
    Ok(ShapeOptions {
        density: a.num_or(first, d.density as f64)? as f32,
        friction: a.num_or(first + 1, d.friction as f64)? as f32,
        restitution: a.num_or(first + 2, d.restitution as f64)? as f32,
        sensor: if a.has(first + 3) { a.flag(first + 3)? } else { d.sensor },
    })
}

pub(crate) fn body_kind(a: &Args<'_>, i: usize) -> Result<BodyKind> {
    let v = a.int(i)?;
    BodyKind::from_code(v).ok_or_else(|| a.refuse(i, format!("{v} is not a body kind (0 static, 1 dynamic, 2 kinematic)")))
}

fn v3(a: &Args<'_>, i: usize) -> Result<[f32; 3]> {
    Ok([a.f32(i)?, a.f32(i + 1)?, a.f32(i + 2)?])
}

fn limits(a: &Args<'_>, i: usize) -> Result<Option<[f32; 2]>> {
    match (a.has(i), a.has(i + 1)) {
        (false, false) => Ok(None),
        (true, true) => Ok(Some([a.f32(i)?, a.f32(i + 1)?])),
        _ => Err(a.refuse(i, "give both limits or neither")),
    }
}

pub(crate) fn triples(a: &Args<'_>, i: usize) -> Result<Vec<[f32; 3]>> {
    let v = a.floats(i)?;
    if v.len() % 3 != 0 {
        return Err(a.refuse(i, "needs x, y and z for every point, so a multiple of three numbers"));
    }
    Ok(v.chunks_exact(3).map(|c| [c[0] as f32, c[1] as f32, c[2] as f32]).collect())
}

/// Copy the transforms of bound bodies to the Scene3D nodes they drive.
#[cfg(feature = "scene3d-core")]
fn apply_bound(o: &mut Physics3D, r: &mut Registry, world: u32) -> Result<()> {
    for b in o.bound(world)? {
        if let BindTarget::Node { scene, node } = b.target {
            // A released scene or node is not an error of the physics: the binding just has no effect.
            let _ = r.with_scene3d(scene, |s, _| {
                s.set_position(node, b.position)?;
                s.set_rotation_quaternion(node, b.rotation)
            });
        }
    }
    Ok(())
}

#[cfg(not(feature = "scene3d-core"))]
fn apply_bound(_o: &mut Physics3D, _r: &mut Registry, _world: u32) -> Result<()> {
    Ok(())
}

#[allow(unused_variables)]
pub(crate) static METHODS: &[crate::value::Method<Physics3D>] = &[
    method!(Physics3D, "CreateWorld", [gx: Num, gy: Num, gz: Num], [], Handle, "A new world with this gravity (metres per second squared; 0 -9.81 0 is Earth).", |o, r, a| Ok(o.create_world(v3(a, 0)?)?.into())),
    method!(Physics3D, "DestroyWorld", [world: Handle], [], Nothing, "Release a world and everything in it.", |o, r, a| {
        o.destroy_world(a.handle(0)?)?;
        done()
    }),
    method!(Physics3D, "SetGravity", [world: Handle, gx: Num, gy: Num, gz: Num], [], Nothing, "Change the gravity.", |o, r, a| {
        o.set_gravity(a.handle(0)?, v3(a, 1)?)?;
        done()
    }),
    method!(Physics3D, "SetFixedStep", [world: Handle, seconds: Num], [max_substeps: Int], Nothing, "Step in equal sub-steps of this many seconds (0 for none), at most max_substeps (default 8) per Step call.", |o, r, a| {
        o.set_fixed_step(a.handle(0)?, a.f32(1)?, a.int_or(2, 8)?.clamp(1, 1000) as u32)?;
        done()
    }),
    method!(Physics3D, "SetThreads", [world: Handle, threads: Int], [], Nothing, "Pin the solver to this many threads (0: the default pool). The results do not depend on it.", |o, r, a| {
        o.set_threads(a.handle(0)?, a.usize(1)?)?;
        done()
    }),
    method!(Physics3D, "AddBody", [world: Handle, kind: Int, x: Num, y: Num, z: Num], [], Handle, "A body: kind 0 static, 1 dynamic, 2 kinematic (moved by its velocity).", |o, r, a| {
        Ok(o.add_body(a.handle(0)?, body_kind(a, 1)?, v3(a, 2)?, [0.0, 0.0, 0.0, 1.0])?.into())
    }),
    method!(Physics3D, "RemoveBody", [world: Handle, body: Handle], [], Nothing, "Remove a body, its shapes and its joints.", |o, r, a| {
        o.remove_body(a.handle(0)?, a.handle(1)?)?;
        done()
    }),
    method!(Physics3D, "AddBall", [world: Handle, body: Handle, radius: Num], [density: Num, friction: Num, restitution: Num, sensor: Flag], Handle, "A sphere on a body.", |o, r, a| {
        Ok(o.add_ball(a.handle(0)?, a.handle(1)?, a.f32(2)?, &shape_options(a, 3)?)?.into())
    }),
    method!(Physics3D, "AddBox", [world: Handle, body: Handle, hx: Num, hy: Num, hz: Num], [density: Num, friction: Num, restitution: Num, sensor: Flag], Handle, "A box with these half extents on a body.", |o, r, a| {
        Ok(o.add_box(a.handle(0)?, a.handle(1)?, v3(a, 2)?, &shape_options(a, 5)?)?.into())
    }),
    method!(Physics3D, "AddCapsule", [world: Handle, body: Handle, half_height: Num, radius: Num], [density: Num, friction: Num, restitution: Num, sensor: Flag], Handle, "A capsule along Y on a body.", |o, r, a| {
        Ok(o.add_capsule(a.handle(0)?, a.handle(1)?, a.f32(2)?, a.f32(3)?, &shape_options(a, 4)?)?.into())
    }),
    method!(Physics3D, "AddConvexHull", [world: Handle, body: Handle, points: Floats], [density: Num, friction: Num, restitution: Num, sensor: Flag], Handle, "The convex hull of points (x, y, z ...) on a body.", |o, r, a| {
        Ok(o.add_convex_hull(a.handle(0)?, a.handle(1)?, &triples(a, 2)?, &shape_options(a, 3)?)?.into())
    }),
    method!(Physics3D, "AddTrimesh", [world: Handle, body: Handle, vertices: Floats, triangles: Ints], [density: Num, friction: Num, restitution: Num, sensor: Flag], Handle, "A triangle mesh on a body (best on a static one): vertices (x, y, z ...) and triangles as indices, three per triangle.", |o, r, a| {
        let idx = a.ints(3)?;
        if idx.len() % 3 != 0 || idx.iter().any(|i| *i < 0 || *i > u32::MAX as i64) {
            return Err(a.refuse(3, "needs three vertex numbers (from 0) for every triangle"));
        }
        let tris: Vec<[u32; 3]> = idx.chunks_exact(3).map(|c| [c[0] as u32, c[1] as u32, c[2] as u32]).collect();
        Ok(o.add_trimesh(a.handle(0)?, a.handle(1)?, &triples(a, 2)?, &tris, &shape_options(a, 4)?)?.into())
    }),
    method!(Physics3D, "RemoveShape", [world: Handle, shape: Handle], [], Nothing, "Remove one shape.", |o, r, a| {
        o.remove_shape(a.handle(0)?, a.handle(1)?)?;
        done()
    }),
    method!(Physics3D, "SetPosition", [world: Handle, body: Handle, x: Num, y: Num, z: Num], [], Nothing, "Teleport a body.", |o, r, a| {
        o.set_position(a.handle(0)?, a.handle(1)?, v3(a, 2)?)?;
        done()
    }),
    method!(Physics3D, "SetRotation", [world: Handle, body: Handle, x: Num, y: Num, z: Num, w: Num], [], Nothing, "Set the orientation as a quaternion.", |o, r, a| {
        o.set_rotation(a.handle(0)?, a.handle(1)?, [a.f32(2)?, a.f32(3)?, a.f32(4)?, a.f32(5)?])?;
        done()
    }),
    method!(Physics3D, "SetRotationEuler", [world: Handle, body: Handle, x_degrees: Num, y_degrees: Num, z_degrees: Num], [], Nothing, "Set the orientation from Euler angles in degrees (yaw about Y, then pitch about X, then roll about Z, as Scene3D's SetRotation).", |o, r, a| {
        let q = glam_euler(a.f32(2)?, a.f32(3)?, a.f32(4)?);
        o.set_rotation(a.handle(0)?, a.handle(1)?, q)?;
        done()
    }),
    method!(Physics3D, "SetVelocity", [world: Handle, body: Handle, x: Num, y: Num, z: Num], [], Nothing, "Set the velocity.", |o, r, a| {
        o.set_velocity(a.handle(0)?, a.handle(1)?, v3(a, 2)?)?;
        done()
    }),
    method!(Physics3D, "SetAngularVelocity", [world: Handle, body: Handle, x: Num, y: Num, z: Num], [], Nothing, "Set the angular velocity (radians per second about each axis).", |o, r, a| {
        o.set_angular_velocity(a.handle(0)?, a.handle(1)?, v3(a, 2)?)?;
        done()
    }),
    method!(Physics3D, "ApplyForce", [world: Handle, body: Handle, x: Num, y: Num, z: Num], [], Nothing, "A force (newtons) that acts during the next Step call.", |o, r, a| {
        o.apply_force(a.handle(0)?, a.handle(1)?, v3(a, 2)?)?;
        done()
    }),
    method!(Physics3D, "ApplyTorque", [world: Handle, body: Handle, x: Num, y: Num, z: Num], [], Nothing, "A torque that acts during the next Step call.", |o, r, a| {
        o.apply_torque(a.handle(0)?, a.handle(1)?, v3(a, 2)?)?;
        done()
    }),
    method!(Physics3D, "ApplyImpulse", [world: Handle, body: Handle, x: Num, y: Num, z: Num], [], Nothing, "Change the velocity at once by impulse / mass.", |o, r, a| {
        o.apply_impulse(a.handle(0)?, a.handle(1)?, v3(a, 2)?)?;
        done()
    }),
    method!(Physics3D, "GetPosition", [world: Handle, body: Handle], [], Floats, "x, y, z.", |o, r, a| Ok(o.get_position(a.handle(0)?, a.handle(1)?)?.to_vec().into())),
    method!(Physics3D, "GetRotation", [world: Handle, body: Handle], [], Floats, "The orientation as a quaternion x, y, z, w.", |o, r, a| Ok(o.get_rotation(a.handle(0)?, a.handle(1)?)?.to_vec().into())),
    method!(Physics3D, "GetVelocity", [world: Handle, body: Handle], [], Floats, "x, y, z.", |o, r, a| Ok(o.get_velocity(a.handle(0)?, a.handle(1)?)?.to_vec().into())),
    method!(Physics3D, "GetAngularVelocity", [world: Handle, body: Handle], [], Floats, "x, y, z.", |o, r, a| Ok(o.get_angular_velocity(a.handle(0)?, a.handle(1)?)?.to_vec().into())),
    method!(Physics3D, "IsSleeping", [world: Handle, body: Handle], [], Int, "1 if the solver has put the body to sleep.", |o, r, a| Ok(o.is_sleeping(a.handle(0)?, a.handle(1)?)?.into())),
    method!(Physics3D, "AddFixedJoint", [world: Handle, body_a: Handle, body_b: Handle, ax: Num, ay: Num, az: Num, bx: Num, by: Num, bz: Num], [], Handle, "Weld two bodies at an anchor given in each body's own space.", |o, r, a| {
        Ok(o.add_fixed_joint(a.handle(0)?, a.handle(1)?, a.handle(2)?, v3(a, 3)?, v3(a, 6)?)?.into())
    }),
    method!(Physics3D, "AddRevoluteJoint", [world: Handle, body_a: Handle, body_b: Handle, ax: Num, ay: Num, az: Num, bx: Num, by: Num, bz: Num, axis_x: Num, axis_y: Num, axis_z: Num], [min_angle: Num, max_angle: Num], Handle, "A hinge about an axis, optionally limited to an angle range in radians.", |o, r, a| {
        Ok(o.add_revolute_joint(a.handle(0)?, a.handle(1)?, a.handle(2)?, v3(a, 3)?, v3(a, 6)?, v3(a, 9)?, limits(a, 12)?)?.into())
    }),
    method!(Physics3D, "AddPrismaticJoint", [world: Handle, body_a: Handle, body_b: Handle, ax: Num, ay: Num, az: Num, bx: Num, by: Num, bz: Num, axis_x: Num, axis_y: Num, axis_z: Num], [min: Num, max: Num], Handle, "A slider along an axis, optionally limited to a distance range.", |o, r, a| {
        Ok(o.add_prismatic_joint(a.handle(0)?, a.handle(1)?, a.handle(2)?, v3(a, 3)?, v3(a, 6)?, v3(a, 9)?, limits(a, 12)?)?.into())
    }),
    method!(Physics3D, "RemoveJoint", [world: Handle, joint: Handle], [], Nothing, "Remove a joint.", |o, r, a| {
        o.remove_joint(a.handle(0)?, a.handle(1)?)?;
        done()
    }),
    method!(Physics3D, "RayCast", [world: Handle, ox: Num, oy: Num, oz: Num, dx: Num, dy: Num, dz: Num, max_distance: Num], [], Floats, "The first shape the ray meets: hit (1 or 0), body, shape, distance, point x y z, normal x y z. The direction need not be a unit vector.", |o, r, a| {
        Ok(match o.ray_cast(a.handle(0)?, v3(a, 1)?, v3(a, 4)?, a.f32(7)?)? {
            Some(h) => vec![1.0, h.body as f64, h.shape as f64, h.distance as f64, h.point[0] as f64, h.point[1] as f64, h.point[2] as f64, h.normal[0] as f64, h.normal[1] as f64, h.normal[2] as f64].into(),
            None => vec![0.0; 10].into(),
        })
    }),
    method!(Physics3D, "Step", [world: Handle, seconds: Num], [], Int, "Advance by this many seconds (at most 1); with a fixed step it runs whole sub-steps. Bound Scene3D nodes follow their bodies. The answer is how many steps ran.", |o, r, a| {
        let world = a.handle(0)?;
        let steps = o.step(world, a.f32(1)?)?.steps;
        apply_bound(o, r, world)?;
        Ok(steps.into())
    }),
    method!(Physics3D, "TakeEvents", [world: Handle], [], Floats, "The collision events since the last call, six numbers each: 0 started or 1 stopped, body a, body b, shape a, shape b, 1 if a sensor was involved.", |o, r, a| {
        let mut out = Vec::new();
        for e in o.take_events(a.handle(0)?)? {
            out.extend([(e.kind == CollisionKind::Stopped) as u8 as f64, e.body_a as f64, e.body_b as f64, e.shape_a as f64, e.shape_b as f64, e.sensor as u8 as f64]);
        }
        Ok(out.into())
    }),
    method!(Physics3D, "BindSprite", [world: Handle, body: Handle, sprite: Int], [], Nothing, "Make a body drive a sprite the host knows by this number (read it back with GetBound).", |o, r, a| {
        o.bind(a.handle(0)?, a.handle(1)?, BindTarget::Sprite(a.u32(2)?))?;
        done()
    }),
    method!(Physics3D, "BindNode", [world: Handle, body: Handle, scene: Handle, node: Handle], [], Nothing, "Make a body drive a Scene3D node: after every Step the node is where the body is.", |o, r, a| {
        o.bind(a.handle(0)?, a.handle(1)?, BindTarget::Node { scene: a.handle(2)?, node: a.handle(3)? })?;
        done()
    }),
    method!(Physics3D, "Unbind", [world: Handle, body: Handle], [], Nothing, "Stop a body driving anything.", |o, r, a| {
        o.unbind(a.handle(0)?, a.handle(1)?)?;
        done()
    }),
    method!(Physics3D, "GetBound", [world: Handle], [], Floats, "The bound bodies, eleven numbers each: body, 0 for a sprite or 1 for a node, the sprite number (or the scene handle), 0 (or the node handle), position x y z, rotation x y z w.", |o, r, a| {
        let mut out = Vec::new();
        for b in o.bound(a.handle(0)?)? {
            let (k, t1, t2) = match b.target {
                BindTarget::Sprite(s) => (0.0, s as f64, 0.0),
                BindTarget::Node { scene, node } => (1.0, scene as f64, node as f64),
            };
            out.extend([b.body as f64, k, t1, t2]);
            out.extend(b.position.iter().map(|v| *v as f64));
            out.extend(b.rotation.iter().map(|v| *v as f64));
        }
        Ok(out.into())
    }),
    method!(Physics3D, "GetBodyCount", [world: Handle], [], Int, "Bodies in a world.", |o, r, a| Ok(o.body_count(a.handle(0)?)?.into())),
    method!(Physics3D, "GetShapeCount", [world: Handle], [], Int, "Shapes in a world.", |o, r, a| Ok(o.shape_count(a.handle(0)?)?.into())),
    method!(Physics3D, "GetJointCount", [world: Handle], [], Int, "Joints in a world.", |o, r, a| Ok(o.joint_count(a.handle(0)?)?.into())),
    method!(Physics3D, "GetWorldCount", [], [], Int, "Worlds in this object.", |o, r, a| Ok(o.world_count().into())),
    method!(Physics3D, "GetStateFingerprint", [world: Handle], [], Int, "A number that is the same for two runs that ended in exactly the same state (positions, rotations, velocities, to the bit).", |o, r, a| Ok((o.state_fingerprint(a.handle(0)?)? as i64).into())),
];

/// Euler angles in degrees (yaw about Y, then pitch about X, then roll about Z) as a quaternion `[x, y, z, w]`.
fn glam_euler(x_deg: f32, y_deg: f32, z_deg: f32) -> [f32; 4] {
    fn mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
        [
            a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
            a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
            a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
            a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
        ]
    }
    let half = |d: f32| (d.to_radians() / 2.0).sin_cos();
    let ((sx, cx), (sy, cy), (sz, cz)) = (half(x_deg), half(y_deg), half(z_deg));
    mul(mul([0.0, sy, 0.0, cy], [sx, 0.0, 0.0, cx]), [0.0, 0.0, sz, cz])
}

pub(crate) fn call(o: &mut Physics3D, reg: &mut Registry, method: &str, args: &[Value]) -> Result<Value> {
    dispatch(METHODS, "Physics3D", o, reg, method, args)
}

pub(crate) fn infos() -> Vec<MethodInfo> {
    super::infos(ObjectKind::Physics3D, METHODS)
}
