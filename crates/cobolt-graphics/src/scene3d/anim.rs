// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Node animation and skins (glTF 2.0 semantics).
//!
//! An [`Animation`] is a set of channels; each drives the translation, rotation
//! or scale of one node over time from keyframes, with step, linear or cubic
//! spline interpolation. A [`Skin`] lists the joint nodes of a skeleton and
//! their inverse bind matrices; the joint matrices a renderer needs are
//! computed from the node transforms the animation has just set, which is how
//! **skeletal** animation falls out of node animation.

use super::math::{Mat4, Quat, Vec3};
use super::scene::{Content, Scene3D};
use crate::error::{GraphicsError, Result};
use crate::handles::Handle;

/// What a channel drives.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Path {
    /// The node's translation.
    Translation,
    /// Its rotation (quaternion).
    Rotation,
    /// Its scale.
    Scale,
}

/// How values between keyframes are found.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Interpolation {
    /// Linear (spherical for rotations).
    Linear,
    /// The previous keyframe's value until the next.
    Step,
    /// A cubic Hermite spline (each keyframe carries an in-tangent and an out-tangent).
    CubicSpline,
}

/// One animated property of one node.
#[derive(Clone, Debug)]
pub struct Channel {
    /// The node it drives.
    pub target: Handle,
    /// Which property.
    pub path: Path,
    /// How to interpolate.
    pub interpolation: Interpolation,
    /// Keyframe times in seconds, strictly increasing.
    pub times: Vec<f32>,
    /// The values: 3 numbers per key for translation and scale, 4 for rotation; with cubic
    /// spline interpolation three such groups per key (in-tangent, value, out-tangent).
    pub values: Vec<f32>,
}

/// What a channel gives at a moment.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Sample {
    /// A translation or a scale.
    Vec(Vec3),
    /// A rotation.
    Quat(Quat),
}

impl Channel {
    fn width(&self) -> usize {
        if self.path == Path::Rotation {
            4
        } else {
            3
        }
    }

    /// Check the shape of the data (the loader and the tests call it; `sample` relies on it).
    pub fn validate(&self) -> Result<()> {
        let key_values = if self.interpolation == Interpolation::CubicSpline { 3 * self.width() } else { self.width() };
        if self.times.is_empty() {
            return Err(GraphicsError::invalid("an animation channel needs at least one keyframe"));
        }
        if self.values.len() != self.times.len() * key_values {
            return Err(GraphicsError::invalid("an animation channel has a different number of values than its keyframes need"));
        }
        if self.times.iter().any(|t| !t.is_finite()) || self.times.windows(2).any(|w| w[1] <= w[0]) {
            return Err(GraphicsError::invalid("animation keyframe times must be finite and strictly increasing"));
        }
        if self.values.iter().any(|v| !v.is_finite()) {
            return Err(GraphicsError::invalid("an animation value is not a finite number"));
        }
        Ok(())
    }

    fn value_at(&self, key: usize, part: usize) -> Sample {
        let w = self.width();
        let per_key = if self.interpolation == Interpolation::CubicSpline { 3 * w } else { w };
        let off = key * per_key + part * w;
        let v = &self.values[off..off + w];
        if self.path == Path::Rotation {
            Sample::Quat(Quat::from_xyzw(v[0], v[1], v[2], v[3]))
        } else {
            Sample::Vec(Vec3::new(v[0], v[1], v[2]))
        }
    }

    /// The value at time `t` (clamped to the first and last keyframe). The channel must be valid.
    pub fn sample(&self, t: f32) -> Sample {
        let n = self.times.len();
        if n == 1 || t <= self.times[0] {
            return self.value_at(0, if self.interpolation == Interpolation::CubicSpline { 1 } else { 0 });
        }
        if t >= self.times[n - 1] {
            return self.value_at(n - 1, if self.interpolation == Interpolation::CubicSpline { 1 } else { 0 });
        }
        // The interval [k, k+1] that holds t.
        let k = match self.times.binary_search_by(|x| x.partial_cmp(&t).unwrap_or(std::cmp::Ordering::Less)) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let (t0, t1) = (self.times[k], self.times[k + 1]);
        let dt = t1 - t0;
        let u = (t - t0) / dt;
        match self.interpolation {
            Interpolation::Step => self.value_at(k, 0),
            Interpolation::Linear => match (self.value_at(k, 0), self.value_at(k + 1, 0)) {
                (Sample::Vec(a), Sample::Vec(b)) => Sample::Vec(a.lerp(b, u)),
                (Sample::Quat(a), Sample::Quat(b)) => {
                    let (a, b) = (a.normalize(), b.normalize());
                    // The short way round.
                    let b = if a.dot(b) < 0.0 { -b } else { b };
                    Sample::Quat(a.slerp(b, u).normalize())
                }
                _ => unreachable!("a channel has one kind of value"),
            },
            Interpolation::CubicSpline => {
                let (u2, u3) = (u * u, u * u * u);
                let (h00, h10, h01, h11) = (2.0 * u3 - 3.0 * u2 + 1.0, u3 - 2.0 * u2 + u, -2.0 * u3 + 3.0 * u2, u3 - u2);
                match (self.value_at(k, 1), self.value_at(k, 2), self.value_at(k + 1, 0), self.value_at(k + 1, 1)) {
                    (Sample::Vec(p0), Sample::Vec(m0), Sample::Vec(m1), Sample::Vec(p1)) => Sample::Vec(p0 * h00 + m0 * (dt * h10) + p1 * h01 + m1 * (dt * h11)),
                    (Sample::Quat(p0), Sample::Quat(m0), Sample::Quat(m1), Sample::Quat(p1)) => {
                        let comb = |a: Quat, b: Quat, c: Quat, d: Quat| a * h00 + b * (dt * h10) + c * h01 + d * (dt * h11);
                        Sample::Quat(comb(p0, m0, p1, m1).normalize())
                    }
                    _ => unreachable!("a channel has one kind of value"),
                }
            }
        }
    }
}

/// A named set of channels with its playback state.
#[derive(Clone, Debug)]
pub struct Animation {
    /// The name given in the model ("Walk"), empty if none.
    pub name: String,
    /// The channels.
    pub channels: Vec<Channel>,
    /// Length in seconds (the last keyframe time of any channel).
    pub duration: f32,
    /// Whether `update` advances it.
    pub playing: bool,
    /// Whether it starts over at the end.
    pub looping: bool,
    /// Playback speed multiplier (1 = real time; negative plays backwards).
    pub speed: f32,
    /// The current time.
    pub time: f32,
}

impl Animation {
    /// An animation from channels (validated); not playing, at time 0.
    pub fn new(name: &str, channels: Vec<Channel>) -> Result<Animation> {
        for c in &channels {
            c.validate()?;
        }
        let duration = channels.iter().filter_map(|c| c.times.last().copied()).fold(0.0f32, f32::max);
        Ok(Animation { name: name.to_string(), channels, duration, playing: false, looping: true, speed: 1.0, time: 0.0 })
    }
}

/// A skeleton: the joints of a skinned mesh and their inverse bind matrices.
#[derive(Clone, Debug)]
pub struct Skin {
    /// The joint nodes, in the order the mesh's joint indices use.
    pub joints: Vec<Handle>,
    /// One per joint: the matrix that takes a vertex from the mesh's space to the joint's own.
    pub inverse_bind: Vec<Mat4>,
}

/// The most joints one skin may have (the renderer's per-skin matrix block).
pub const MAX_JOINTS: usize = 128;

impl Scene3D {
    /// Add an animation (the model loader does; a program may build its own).
    pub fn add_animation(&mut self, animation: Animation) -> Result<Handle> {
        let h = self.animations.insert(animation)?;
        Ok(h)
    }

    /// `GetAnimations`: every animation, in handle order.
    pub fn animations(&self) -> Vec<Handle> {
        self.animations.iter().map(|(h, _)| h).collect()
    }

    /// `FindAnimation`: by name.
    pub fn find_animation(&self, name: &str) -> Option<Handle> {
        self.animations.iter().find(|(_, a)| a.name == name).map(|(h, _)| h)
    }

    /// The animation's name.
    pub fn animation_name(&self, animation: Handle) -> Result<&str> {
        Ok(&self.animations.get(animation)?.name)
    }

    /// `GetAnimationDuration` in seconds.
    pub fn animation_duration(&self, animation: Handle) -> Result<f32> {
        Ok(self.animations.get(animation)?.duration)
    }

    /// `GetAnimationTime`.
    pub fn animation_time(&self, animation: Handle) -> Result<f32> {
        Ok(self.animations.get(animation)?.time)
    }

    /// Whether it is playing.
    pub fn is_animation_playing(&self, animation: Handle) -> Result<bool> {
        Ok(self.animations.get(animation)?.playing)
    }

    /// `PlayAnimation`: start (or resume) it. `speed` multiplies the clock; negative runs backwards.
    pub fn play_animation(&mut self, animation: Handle, looping: bool, speed: f32) -> Result<()> {
        if !speed.is_finite() || speed.abs() > 1000.0 {
            return Err(GraphicsError::invalid("an animation speed is a finite number from -1000 to 1000"));
        }
        let a = self.animations.get_mut(animation)?;
        a.playing = true;
        a.looping = looping;
        a.speed = speed;
        if !looping && ((speed >= 0.0 && a.time >= a.duration) || (speed < 0.0 && a.time <= 0.0)) {
            a.time = if speed >= 0.0 { 0.0 } else { a.duration };
        }
        Ok(())
    }

    /// `StopAnimation`: pause it where it is.
    pub fn stop_animation(&mut self, animation: Handle) -> Result<()> {
        self.animations.get_mut(animation)?.playing = false;
        Ok(())
    }

    /// `SetAnimationTime`: jump to a time (seconds) and show that pose.
    pub fn set_animation_time(&mut self, animation: Handle, seconds: f32) -> Result<()> {
        if !seconds.is_finite() {
            return Err(GraphicsError::invalid("an animation time must be a finite number"));
        }
        let a = self.animations.get_mut(animation)?;
        a.time = seconds.clamp(0.0, a.duration);
        self.apply_animation(animation)
    }

    /// `Update`: advance every playing animation by `dt` seconds and set the nodes they drive.
    pub fn update(&mut self, dt: f32) -> Result<()> {
        if !dt.is_finite() || dt < 0.0 {
            return Err(GraphicsError::invalid("the elapsed time must be a finite number of seconds, zero or more"));
        }
        let playing: Vec<Handle> = self.animations.iter().filter(|(_, a)| a.playing).map(|(h, _)| h).collect();
        for h in playing {
            {
                let a = self.animations.get_mut(h)?;
                let mut t = a.time + dt * a.speed;
                if a.duration > 0.0 {
                    if a.looping {
                        t = t.rem_euclid(a.duration);
                    } else if t >= a.duration {
                        t = a.duration;
                        a.playing = false;
                    } else if t <= 0.0 {
                        t = 0.0;
                        a.playing = false;
                    }
                } else {
                    t = 0.0;
                    a.playing = false;
                }
                a.time = t;
            }
            self.apply_animation(h)?;
        }
        Ok(())
    }

    /// Set the driven nodes' transforms to the animation's pose at its current time.
    fn apply_animation(&mut self, animation: Handle) -> Result<()> {
        let (time, channels) = {
            let a = self.animations.get(animation)?;
            (a.time, a.channels.clone())
        };
        for c in &channels {
            let Ok(node) = self.nodes.get_mut(c.target) else { continue };
            match (c.path, c.sample(time)) {
                (Path::Translation, Sample::Vec(v)) => node.translation = v,
                (Path::Scale, Sample::Vec(v)) => {
                    if v.x != 0.0 && v.y != 0.0 && v.z != 0.0 {
                        node.scale = v
                    }
                }
                (Path::Rotation, Sample::Quat(q)) => node.rotation = q.normalize(),
                _ => {}
            }
        }
        self.world_dirty = true;
        Ok(())
    }

    /// Add a skin and tell a node to use it.
    pub fn add_skin(&mut self, skin: Skin) -> Result<Handle> {
        if skin.joints.len() != skin.inverse_bind.len() || skin.joints.is_empty() {
            return Err(GraphicsError::invalid("a skin needs one inverse bind matrix per joint"));
        }
        if skin.joints.len() > MAX_JOINTS {
            return Err(GraphicsError::LimitExceeded { what: "joints in one skin", requested: skin.joints.len() as u64, limit: MAX_JOINTS as u64, setting: "(fixed by the renderer)" });
        }
        self.skins.insert(skin)
    }

    /// Make a mesh node follow a skin.
    pub fn set_skin(&mut self, node: Handle, skin: Handle) -> Result<()> {
        self.skins.get(skin)?;
        let n = self.nodes.get_mut(node)?;
        if !matches!(n.content, Content::Mesh(_)) {
            return Err(GraphicsError::invalid("only a node that draws a mesh can follow a skin"));
        }
        n.skin = Some(skin);
        Ok(())
    }

    /// The matrices that move a skinned mesh's vertices to where the current pose puts them:
    /// `inverse(mesh node) * joint node * inverse bind`, one per joint. A joint that has been
    /// removed contributes the identity.
    pub fn joint_matrices(&mut self, skin: Handle, mesh_world: &Mat4) -> Result<Vec<Mat4>> {
        self.update_transforms();
        self.joint_matrices_now(skin, mesh_world)
    }

    /// [`Scene3D::joint_matrices`] for callers that know the world matrices are current.
    pub(super) fn joint_matrices_now(&self, skin: Handle, mesh_world: &Mat4) -> Result<Vec<Mat4>> {
        let s = self.skins.get(skin)?;
        let inv = mesh_world.inverse();
        Ok(s.joints
            .iter()
            .zip(&s.inverse_bind)
            .map(|(j, ibm)| match self.nodes.get(*j) {
                Ok(n) => inv * n.world * *ibm,
                Err(_) => Mat4::IDENTITY,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::Limits;

    fn chan(path: Path, interp: Interpolation, times: Vec<f32>, values: Vec<f32>) -> Channel {
        Channel { target: 1, path, interpolation: interp, times, values }
    }

    #[test]
    fn linear_step_and_clamping() {
        let c = chan(Path::Translation, Interpolation::Linear, vec![0.0, 1.0, 3.0], vec![0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 10.0, 20.0, 0.0]);
        c.validate().unwrap();
        assert_eq!(c.sample(-5.0), Sample::Vec(Vec3::ZERO));
        assert_eq!(c.sample(0.5), Sample::Vec(Vec3::new(5.0, 0.0, 0.0)));
        assert_eq!(c.sample(1.0), Sample::Vec(Vec3::new(10.0, 0.0, 0.0)));
        assert_eq!(c.sample(2.0), Sample::Vec(Vec3::new(10.0, 10.0, 0.0)));
        assert_eq!(c.sample(99.0), Sample::Vec(Vec3::new(10.0, 20.0, 0.0)));
        let s = chan(Path::Scale, Interpolation::Step, vec![0.0, 1.0], vec![1.0, 1.0, 1.0, 2.0, 2.0, 2.0]);
        assert_eq!(s.sample(0.99), Sample::Vec(Vec3::ONE));
        assert_eq!(s.sample(1.0), Sample::Vec(Vec3::splat(2.0)));
    }

    #[test]
    fn rotations_take_the_short_way_and_stay_unit() {
        let a = Quat::IDENTITY;
        let b = Quat::from_rotation_y(std::f32::consts::PI * 1.5);
        let c = chan(Path::Rotation, Interpolation::Linear, vec![0.0, 1.0], vec![a.x, a.y, a.z, a.w, b.x, b.y, b.z, b.w]);
        let Sample::Quat(mid) = c.sample(0.5) else { panic!() };
        assert!((mid.length() - 1.0).abs() < 1e-5);
        // Halfway to a 270 degree turn, the short way, is a -45 degree turn.
        let expect = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4);
        assert!(mid.angle_between(expect) < 1e-4, "{mid:?}");
    }

    #[test]
    fn cubic_spline_passes_through_its_keyframes_and_honours_tangents() {
        // Keys at t=0 and t=1 with value 0 then 1, zero tangents: a smooth ease with the mid at 0.5.
        let c = chan(Path::Translation, Interpolation::CubicSpline, vec![0.0, 1.0], vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        c.validate().unwrap();
        assert_eq!(c.sample(0.0), Sample::Vec(Vec3::ZERO));
        assert_eq!(c.sample(1.0), Sample::Vec(Vec3::X));
        let Sample::Vec(mid) = c.sample(0.5) else { panic!() };
        assert!((mid.x - 0.5).abs() < 1e-6);
        let Sample::Vec(early) = c.sample(0.25) else { panic!() };
        assert!(early.x < 0.25, "ease-in: {early:?}");
    }

    #[test]
    fn bad_channels_are_refused() {
        assert!(chan(Path::Translation, Interpolation::Linear, vec![], vec![]).validate().is_err());
        assert!(chan(Path::Translation, Interpolation::Linear, vec![0.0, 1.0], vec![0.0; 3]).validate().is_err(), "too few values");
        assert!(chan(Path::Translation, Interpolation::Linear, vec![1.0, 1.0], vec![0.0; 6]).validate().is_err(), "times not increasing");
        assert!(chan(Path::Translation, Interpolation::Linear, vec![0.0], vec![f32::NAN; 3]).validate().is_err());
        assert!(chan(Path::Rotation, Interpolation::Linear, vec![0.0], vec![0.0; 3]).validate().is_err(), "a rotation key is four numbers");
    }

    #[test]
    fn an_animation_drives_a_node_and_loops_or_stops() {
        let mut s = Scene3D::new(Limits::default());
        let n = s.add_group().unwrap();
        let mut c = chan(Path::Translation, Interpolation::Linear, vec![0.0, 2.0], vec![0.0, 0.0, 0.0, 4.0, 0.0, 0.0]);
        c.target = n;
        let a = s.add_animation(Animation::new("slide", vec![c]).unwrap()).unwrap();
        assert_eq!(s.find_animation("slide"), Some(a));
        assert_eq!(s.animation_duration(a).unwrap(), 2.0);
        s.play_animation(a, true, 1.0).unwrap();
        s.update(1.0).unwrap();
        assert_eq!(s.position(n).unwrap(), [2.0, 0.0, 0.0]);
        s.update(1.5).unwrap();
        assert_eq!(s.animation_time(a).unwrap(), 0.5, "looped");
        assert_eq!(s.position(n).unwrap(), [1.0, 0.0, 0.0]);
        s.play_animation(a, false, 2.0).unwrap();
        s.update(5.0).unwrap();
        assert_eq!(s.animation_time(a).unwrap(), 2.0);
        assert!(!s.is_animation_playing(a).unwrap(), "stopped at the end");
        assert_eq!(s.position(n).unwrap(), [4.0, 0.0, 0.0]);
        s.set_animation_time(a, 1.0).unwrap();
        assert_eq!(s.position(n).unwrap(), [2.0, 0.0, 0.0]);
        s.stop_animation(a).unwrap();
        s.update(1.0).unwrap();
        assert_eq!(s.animation_time(a).unwrap(), 1.0, "a stopped animation does not advance");
        assert!(s.update(-1.0).is_err());
        assert!(s.play_animation(a, true, f32::NAN).is_err());
        // A removed target is skipped, not an error.
        s.remove(n).unwrap();
        s.set_animation_time(a, 0.5).unwrap();
    }

    #[test]
    fn a_skin_turns_joint_poses_into_matrices() {
        let mut s = Scene3D::new(Limits::default());
        let mesh = s.add_box([1.0; 3]).unwrap();
        let joint = s.add_group().unwrap();
        s.set_position(joint, [0.0, 1.0, 0.0]).unwrap();
        let skin = s.add_skin(Skin { joints: vec![joint], inverse_bind: vec![Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0))] }).unwrap();
        s.set_skin(mesh, skin).unwrap();
        let world = s.world_matrix(mesh).unwrap();
        let m = s.joint_matrices(skin, &world).unwrap();
        assert!((m[0] - Mat4::IDENTITY).abs_diff_eq(Mat4::ZERO, 1e-6), "the bind pose is the identity");
        s.set_position(joint, [0.0, 3.0, 0.0]).unwrap();
        let m = s.joint_matrices(skin, &world).unwrap();
        assert_eq!(m[0].transform_point3(Vec3::ZERO), Vec3::new(0.0, 2.0, 0.0), "moving the joint up 2 moves the vertices up 2");
        s.remove(joint).unwrap();
        assert_eq!(s.joint_matrices(skin, &world).unwrap()[0], Mat4::IDENTITY);
        let g = s.add_group().unwrap();
        assert!(s.set_skin(g, skin).is_err());
        assert!(s.add_skin(Skin { joints: vec![], inverse_bind: vec![] }).is_err());
    }
}
