// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Control animation runtime — shared by every surface that shows a form.
//!
//! The designer preview grew its own clock inside the IDE, so the standalone
//! run-form process (`rcrun run-form`, what the IDE's **Run Form** launches) and
//! compiled binaries had no animation at all: their `FormState::transform`
//! returned identity forever and every `OnFormLoad` fly-in / fade simply drew in
//! its final place. This module owns the whole thing — the per-kind transform
//! math, the playback clock (delay, duration, easing, repeat) and the trigger
//! mapping — so any host can drive animations with three calls: [`AnimRuntime::start_form_load`],
//! [`AnimRuntime::tick`] and [`AnimRuntime::transform`].

use std::collections::HashMap;

use crate::model::{AnimKind, AnimRepeat, AnimTrigger, AnimationDef, EasingKind};
use crate::render::RenderTransform;
use crate::Control;

/// State key for one control's one animation.
fn key(ctrl_id: &str, anim_name: &str) -> String {
    format!("{ctrl_id}:{anim_name}")
}

/// ZoomOut "bounce" scale over progress `t`: a damped oscillation that starts at
/// 100%, dips toward 25%, then bounces 3–4 times with decreasing amplitude,
/// settling exactly at 100%.
fn zoomout_scale(t: f32) -> f32 {
    // N half-cycles (→ ~3–4 visible bounces); A sets the first dip (≈25%);
    // D damps each successive bounce. sin(Nπ·t) = 0 at t=0 and t=1, so the curve
    // begins and ends exactly at 100%.
    const N: f32 = 5.0;
    const A: f32 = 1.06;
    const D: f32 = 3.5;
    let osc = (N * std::f32::consts::PI * t).sin();
    (1.0 - A * (-D * t).exp() * osc).max(0.02)
}

/// Compute the offset in form-space for an animation at progress `t`.
/// Returns `(dx, dy, scale, alpha_mul)` where `alpha_mul` is 0..1.
pub fn anim_transform(
    anim: &AnimationDef,
    form_w: f32,
    form_h: f32,
    t: f32,
) -> (f32, f32, f32, f32) {
    let te = anim.easing.apply(t); // eased progress
    let inv = 1.0 - te;
    match &anim.kind {
        AnimKind::FlyFromLeft => (-form_w * inv, 0.0, 1.0, 1.0),
        AnimKind::FlyFromRight => (form_w * inv, 0.0, 1.0, 1.0),
        AnimKind::FlyFromTop => (0.0, -form_h * inv, 1.0, 1.0),
        AnimKind::FlyFromBottom => (0.0, form_h * inv, 1.0, 1.0),
        AnimKind::FlyFromTopLeft => (-form_w * inv, -form_h * inv, 1.0, 1.0),
        AnimKind::FlyFromTopRight => (form_w * inv, -form_h * inv, 1.0, 1.0),
        AnimKind::FlyFromBottomLeft => (-form_w * inv, form_h * inv, 1.0, 1.0),
        AnimKind::FlyFromBottomRight => (form_w * inv, form_h * inv, 1.0, 1.0),
        AnimKind::FadeIn => (0.0, 0.0, 1.0, te),
        AnimKind::FadeOut => (0.0, 0.0, 1.0, 1.0 - te),
        // ZoomIn grows 0 → 100% (eased; Elastic overshoots past 100% and settles).
        AnimKind::ZoomIn => (0.0, 0.0, te.max(0.001), te),
        // ZoomOut dips and returns: 100% → 25% → 100%. With Elastic easing this
        // becomes a damped multi-bounce (overshoots 3–4 times before settling).
        AnimKind::ZoomOut => {
            let scale = if matches!(anim.easing, EasingKind::Elastic) {
                zoomout_scale(t)
            } else {
                // Smooth single dip-and-return (no overshoot), timed by the easing.
                (1.0 - 0.75 * (std::f32::consts::PI * te).sin()).max(0.02)
            };
            (0.0, 0.0, scale, 1.0)
        }
        AnimKind::Bounce => {
            let dy = -50.0 * (std::f32::consts::PI * t * 3.0).sin().abs() * inv;
            (0.0, dy, 1.0, 1.0)
        }
        AnimKind::Shake => {
            let dx = 6.0 * (t * std::f32::consts::TAU * 5.0).sin() * inv;
            (dx, 0.0, 1.0, 1.0)
        }
        AnimKind::Pulse => {
            let s = 1.0 + 0.15 * (t * std::f32::consts::TAU * 2.0).sin() * inv;
            (0.0, 0.0, s, 1.0)
        }
        AnimKind::Slide { dx, dy } => ((*dx as f32) * inv, (*dy as f32) * inv, 1.0, 1.0),
        AnimKind::Spin => {
            // Simulate spin as a scale pulse that goes through 0 twice (simulates
            // a 360° rotation in 2D by shrinking to nothing and back twice).
            let angle = te * std::f32::consts::TAU;
            let s = angle.cos().abs().max(0.05); // 1 → 0 → 1 twice = perceived spin
            (0.0, 0.0, s, te)
        }
        AnimKind::Flip => {
            // Horizontal flip: scale goes 1 → 0 → 1 (one half-rotation).
            let s = (te * std::f32::consts::PI).cos().abs().max(0.05);
            (0.0, 0.0, s, 1.0)
        }
        AnimKind::None | AnimKind::Custom(_) => (0.0, 0.0, 1.0, 1.0),
    }
}

/// Playback clock for a single animation instance.
#[derive(Clone, Debug)]
pub struct AnimPlayback {
    /// The definition being played (duration / easing / repeat live here).
    pub def: AnimationDef,
    /// Progress 0.0 → 1.0.
    pub t: f32,
    /// Advancing? (`false` once finished, or while paused).
    pub playing: bool,
    /// True = forward, false = reverse (PingPong).
    pub forward: bool,
    /// Completed passes (a PingPong there-and-back counts two).
    pub loops: u32,
    /// Seconds of delay still to wait before `t` starts advancing.
    pub delay_remaining: f32,
}

impl AnimPlayback {
    fn new(def: AnimationDef) -> Self {
        let delay_remaining = def.delay_ms as f32 / 1000.0;
        Self {
            def,
            t: 0.0,
            playing: true,
            forward: true,
            loops: 0,
            delay_remaining,
        }
    }
}

/// Drives every control animation of one running form.
///
/// The host owns one of these, starts the load-time animations once, ticks it
/// with the frame delta, and asks it for each control's [`RenderTransform`].
#[derive(Default)]
pub struct AnimRuntime {
    states: HashMap<String, AnimPlayback>,
    form_w: f32,
    form_h: f32,
}

impl AnimRuntime {
    /// A runtime for a form of `form_w` × `form_h` design pixels (fly-in effects
    /// travel one form width/height, so the size must be the form's, not the
    /// window's).
    pub fn new(form_w: f32, form_h: f32) -> Self {
        Self {
            states: HashMap::new(),
            form_w,
            form_h,
        }
    }

    /// Is anything currently advancing? (The host requests another frame while
    /// this is true; when it goes false the form goes back to sleep.)
    pub fn is_animating(&self) -> bool {
        self.states.values().any(|s| s.playing)
    }

    /// Start `def` on `ctrl_id` from the beginning, honouring its delay.
    pub fn play(&mut self, ctrl_id: &str, def: &AnimationDef) {
        self.states
            .insert(key(ctrl_id, &def.name), AnimPlayback::new(def.clone()));
    }

    /// Start every animation on `ctrl_id` whose trigger matches `pred`.
    fn play_matching(&mut self, ctrl: &Control, pred: impl Fn(&AnimTrigger) -> bool) {
        let defs: Vec<AnimationDef> = ctrl
            .animations
            .iter()
            .filter(|a| pred(&a.trigger))
            .cloned()
            .collect();
        for def in defs {
            self.play(&ctrl.id, &def);
        }
    }

    /// Fire the load-time triggers (`OnFormLoad`, `OnShow`) across the whole form.
    /// Call once, when the form window comes up.
    pub fn start_form_load(&mut self, controls: &[Control]) {
        for c in controls {
            self.play_matching(c, |t| {
                matches!(t, AnimTrigger::OnFormLoad | AnimTrigger::OnShow)
            });
        }
    }

    /// Play the animations a UI event triggers on one control. `event` is the
    /// engine's event name (`onClick`, `onGotFocus`, `onHoverEnter`, `onTick`, …);
    /// unknown names simply match nothing.
    ///
    /// `onTick` is special: a Timer's tick fires the `OnTimer(<timer id>)`
    /// animations of **every** control, so `ctrl_id` there is the timer's id and
    /// the whole control list is scanned.
    pub fn fire_event(&mut self, controls: &[Control], ctrl_id: &str, event: &str) {
        if event.eq_ignore_ascii_case("onTick") {
            for c in controls {
                self.play_matching(c, |t| match t {
                    AnimTrigger::OnTimer(id) => id.eq_ignore_ascii_case(ctrl_id),
                    _ => false,
                });
            }
            return;
        }
        let Some(ctrl) = controls.iter().find(|c| c.id.eq_ignore_ascii_case(ctrl_id)) else {
            return;
        };
        let ctrl = ctrl.clone();
        match event {
            e if e.eq_ignore_ascii_case("onClick") => {
                self.play_matching(&ctrl, |t| matches!(t, AnimTrigger::OnClick))
            }
            e if e.eq_ignore_ascii_case("onHoverEnter") || e.eq_ignore_ascii_case("onMouseEnter") => {
                self.play_matching(&ctrl, |t| matches!(t, AnimTrigger::OnHover))
            }
            e if e.eq_ignore_ascii_case("onGotFocus") || e.eq_ignore_ascii_case("onFocus") => {
                self.play_matching(&ctrl, |t| matches!(t, AnimTrigger::OnFocus))
            }
            _ => {}
        }
    }

    /// COBOL `PLAY ANIMATION` on a control. `name` is the animation's name; an
    /// empty name (the runtime sends `"1"` for a bare `PLAY`) starts every
    /// `Programmatic` animation the control owns.
    pub fn play_programmatic(&mut self, controls: &[Control], ctrl_id: &str, name: &str) {
        let Some(ctrl) = controls
            .iter()
            .find(|c| c.id.eq_ignore_ascii_case(ctrl_id))
            .cloned()
        else {
            return;
        };
        let name = name.trim();
        let named = !name.is_empty() && name != "1";
        if named {
            if let Some(def) = ctrl
                .animations
                .iter()
                .find(|a| a.name.eq_ignore_ascii_case(name))
                .cloned()
            {
                self.play(&ctrl.id, &def);
            }
            return;
        }
        self.play_matching(&ctrl, |t| matches!(t, AnimTrigger::Programmatic));
    }

    /// COBOL `STOP-ANIMATION`: drop every clock for the control, so it snaps back
    /// to its designed position/size/opacity.
    pub fn stop_all(&mut self, ctrl_id: &str) {
        self.states.retain(|k, _| {
            let id = k.rsplit_once(':').map(|(id, _)| id).unwrap_or(k);
            !id.eq_ignore_ascii_case(ctrl_id)
        });
    }

    /// COBOL `PAUSE`: freeze the control's animations where they are.
    pub fn pause_all(&mut self, ctrl_id: &str) {
        for (k, s) in self.states.iter_mut() {
            let id = k.rsplit_once(':').map(|(id, _)| id).unwrap_or(k.as_str());
            if id.eq_ignore_ascii_case(ctrl_id) {
                s.playing = false;
            }
        }
    }

    /// Advance every playing animation by `dt` seconds. Returns `true` when at
    /// least one is still running, i.e. the host must schedule another frame.
    pub fn tick(&mut self, dt: f32) -> bool {
        if dt <= 0.0 {
            return self.is_animating();
        }
        let mut running = false;
        for state in self.states.values_mut() {
            if !state.playing {
                continue;
            }
            running = true;
            // Delay phase: count down before `t` starts moving.
            if state.delay_remaining > 0.0 {
                state.delay_remaining = (state.delay_remaining - dt).max(0.0);
                continue;
            }
            let dur = state.def.duration_ms as f32 / 1000.0;
            if dur <= 0.0 {
                // A zero-length animation is "already finished" — land on the end
                // state instead of dividing by zero.
                state.t = 1.0;
                state.playing = false;
                continue;
            }
            let step = dt / dur;
            if state.forward {
                state.t += step;
            } else {
                state.t -= step;
            }
            if state.t >= 1.0 || state.t <= 0.0 {
                let overshoot_high = state.t >= 1.0;
                state.t = state.t.clamp(0.0, 1.0);
                state.loops = state.loops.saturating_add(1);
                match state.def.repeat {
                    AnimRepeat::Once => {
                        // Rest at the end state (t = 1) — the animated control's
                        // designed position for every entrance effect.
                        state.t = 1.0;
                        state.playing = false;
                    }
                    AnimRepeat::Loop => {
                        state.t = 0.0;
                    }
                    AnimRepeat::PingPong => {
                        state.forward = !overshoot_high;
                    }
                    AnimRepeat::Count(n) => {
                        if state.loops >= n.max(1) {
                            state.t = 1.0;
                            state.playing = false;
                        } else {
                            state.t = 0.0;
                        }
                    }
                }
            }
        }
        running
    }

    /// The transform to draw `base` with this frame (identity when it has no
    /// animation, or none of its animations has been triggered yet).
    pub fn transform(&self, base: &Control) -> RenderTransform {
        let Some((dx, dy, scale, alpha)) = base.animations.iter().find_map(|a| {
            self.states
                .get(&key(&base.id, &a.name))
                .map(|s| anim_transform(&s.def, self.form_w, self.form_h, s.t))
        }) else {
            return RenderTransform::IDENTITY;
        };
        RenderTransform {
            dx,
            dy,
            scale,
            alpha,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ControlType;

    fn ctrl_with(anim: AnimationDef) -> Control {
        let mut c = Control::new("Label-1", ControlType::Label, 10, 10);
        c.add_animation(anim);
        c
    }

    fn fly_in(trigger: AnimTrigger) -> AnimationDef {
        let mut a = AnimationDef::new("intro");
        a.trigger = trigger;
        a.kind = AnimKind::FlyFromLeft;
        a.easing = EasingKind::Linear;
        a.duration_ms = 100;
        a
    }

    /// The run form's bug in one test: an OnFormLoad animation must actually move
    /// the control away from its designed spot at t=0 and land back on it at t=1.
    #[test]
    fn form_load_animation_offsets_then_settles() {
        let c = ctrl_with(fly_in(AnimTrigger::OnFormLoad));
        let mut rt = AnimRuntime::new(400.0, 300.0);
        rt.start_form_load(std::slice::from_ref(&c));

        let start = rt.transform(&c);
        assert_eq!(start.dx, -400.0, "starts one form width to the left");
        assert!(rt.is_animating());

        assert!(rt.tick(0.05));
        let mid = rt.transform(&c);
        assert!(mid.dx > -400.0 && mid.dx < 0.0, "mid-flight, got {}", mid.dx);

        rt.tick(0.05);
        let end = rt.transform(&c);
        assert_eq!(end.dx, 0.0, "settles on the designed position");
        assert!(!rt.is_animating(), "Once stops when it lands");
    }

    /// A control with no animation, and one whose trigger has not fired, both draw
    /// exactly as designed.
    #[test]
    fn untriggered_animation_is_identity() {
        let c = ctrl_with(fly_in(AnimTrigger::OnClick));
        let mut rt = AnimRuntime::new(400.0, 300.0);
        rt.start_form_load(std::slice::from_ref(&c));
        let tf = rt.transform(&c);
        assert_eq!((tf.dx, tf.dy, tf.scale, tf.alpha), (0.0, 0.0, 1.0, 1.0));
        assert!(!rt.is_animating());

        let plain = Control::new("Label-2", ControlType::Label, 0, 0);
        assert_eq!(rt.transform(&plain).dx, 0.0);
    }

    #[test]
    fn click_event_triggers_onclick_animation() {
        let c = ctrl_with(fly_in(AnimTrigger::OnClick));
        let controls = vec![c.clone()];
        let mut rt = AnimRuntime::new(400.0, 300.0);
        // COBOL upper-cases ids, so the lookup must be case-insensitive.
        rt.fire_event(&controls, "LABEL-1", "onClick");
        assert_eq!(rt.transform(&c).dx, -400.0);
    }

    #[test]
    fn timer_tick_triggers_ontimer_animations_of_other_controls() {
        let c = ctrl_with(fly_in(AnimTrigger::OnTimer("Timer-1".into())));
        let mut timer = Control::new("Timer-1", ControlType::Timer, 0, 0);
        timer.visible = false;
        let controls = vec![c.clone(), timer];
        let mut rt = AnimRuntime::new(400.0, 300.0);
        rt.fire_event(&controls, "Timer-1", "onTick");
        assert!(rt.is_animating(), "the timer's tick starts the label's anim");
        assert_eq!(rt.transform(&c).dx, -400.0);
    }

    /// `PLAY ANIMATION "name"` from COBOL starts that animation; a bare `PLAY`
    /// (value "1") starts the control's Programmatic ones.
    #[test]
    fn programmatic_play_by_name_and_bare() {
        let c = ctrl_with(fly_in(AnimTrigger::Programmatic));
        let controls = vec![c.clone()];

        let mut by_name = AnimRuntime::new(400.0, 300.0);
        by_name.play_programmatic(&controls, "LABEL-1", "intro");
        assert!(by_name.is_animating());

        let mut bare = AnimRuntime::new(400.0, 300.0);
        bare.play_programmatic(&controls, "Label-1", "1");
        assert!(bare.is_animating());

        let mut unknown = AnimRuntime::new(400.0, 300.0);
        unknown.play_programmatic(&controls, "Label-1", "nope");
        assert!(!unknown.is_animating());
    }

    #[test]
    fn stop_and_pause_animation() {
        let c = ctrl_with(fly_in(AnimTrigger::OnFormLoad));
        let controls = vec![c.clone()];
        let mut rt = AnimRuntime::new(400.0, 300.0);
        rt.start_form_load(&controls);
        rt.pause_all("LABEL-1");
        assert!(!rt.is_animating(), "paused clocks do not advance");
        assert_eq!(rt.transform(&c).dx, -400.0, "but stay where they froze");

        rt.stop_all("Label-1");
        assert_eq!(
            rt.transform(&c).dx,
            0.0,
            "stopped animations release the control to its designed spot"
        );
    }

    /// Loop restarts forever; PingPong reverses; Count(n) stops after n passes.
    #[test]
    fn repeat_modes() {
        let mut looped = fly_in(AnimTrigger::OnFormLoad);
        looped.repeat = AnimRepeat::Loop;
        let c = ctrl_with(looped);
        let mut rt = AnimRuntime::new(400.0, 300.0);
        rt.start_form_load(std::slice::from_ref(&c));
        rt.tick(0.15); // past one full 100 ms pass
        assert!(rt.is_animating(), "Loop keeps going");

        let mut ping = fly_in(AnimTrigger::OnFormLoad);
        ping.repeat = AnimRepeat::PingPong;
        let c2 = ctrl_with(ping);
        let mut rt2 = AnimRuntime::new(400.0, 300.0);
        rt2.start_form_load(std::slice::from_ref(&c2));
        rt2.tick(0.1); // hits t = 1 and turns around
        rt2.tick(0.05);
        let tf = rt2.transform(&c2);
        assert!(tf.dx < 0.0 && tf.dx > -400.0, "reversing, got {}", tf.dx);
        assert!(rt2.is_animating());

        let mut twice = fly_in(AnimTrigger::OnFormLoad);
        twice.repeat = AnimRepeat::Count(2);
        let c3 = ctrl_with(twice);
        let mut rt3 = AnimRuntime::new(400.0, 300.0);
        rt3.start_form_load(std::slice::from_ref(&c3));
        rt3.tick(0.1);
        assert!(rt3.is_animating(), "one pass of two");
        rt3.tick(0.1);
        assert!(!rt3.is_animating(), "stops after the second pass");
    }

    /// A delay holds the control at t=0 (fully off-screen / invisible) instead of
    /// letting it drift; only after the delay does progress start.
    #[test]
    fn delay_holds_before_progress() {
        let mut def = fly_in(AnimTrigger::OnFormLoad);
        def.delay_ms = 200;
        let c = ctrl_with(def);
        let mut rt = AnimRuntime::new(400.0, 300.0);
        rt.start_form_load(std::slice::from_ref(&c));
        rt.tick(0.1);
        assert_eq!(rt.transform(&c).dx, -400.0, "still waiting out the delay");
        rt.tick(0.1); // delay exhausted
        rt.tick(0.05);
        assert!(rt.transform(&c).dx > -400.0, "moving once the delay elapsed");
    }
}
