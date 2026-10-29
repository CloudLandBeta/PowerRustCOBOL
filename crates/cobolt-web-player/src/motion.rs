// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Motion: what the desktop animates, computed as the desktop computes it** (the PowerAnalyticsWeb parity job, P4).
//!
//! Two kinds of motion reach a running form on the desktop, and this module holds the arithmetic of both so the
//! page can play them; it touches no browser and is tested natively.
//!
//! * **A control's own animation** (spec 038; `cobolt-forms` `anim.rs`): `PlayAnimation("intro")` and the
//!   triggers (`OnFormLoad`, `OnShow`, `OnClick`, `OnHover`, `OnFocus`). The desktop moves each control by a
//!   [`Tf`] — a shift, a scale about the control's own centre and an alpha — taken from the animation's kind at
//!   a progress `t` its clock gives. [`anim_transform`] and [`clock_at`] are those two functions, carried over
//!   without change of meaning (`anim.rs` is behind the desktop's `render` feature, which the browser does not link).
//!   Every control is moved on its own: the desktop draws a child at its own animated rectangle whatever its
//!   container is doing, while a page moves a child with its container. [`local_transform`] is the transform a
//!   child must be given so that, inside a container that moves, it ends where the desktop draws it.
//! * **A chart's motion** (`render.rs` CT::*Chart): `AnimateOnLoad` grows every mark from zero the first time the
//!   chart is drawn with data, and `AnimateValues` makes a chart whose data changes travel from the values it is
//!   showing to the new ones; both over `AnimationDuration` (at least 250 ms), with the painter's ease-out. The
//!   geometry is the chart painter's own (`view::chart_paint`); [`ChartMotion`] gives it the grow fraction and the
//!   series to draw at a given moment.

use cobolt_forms::chart::{self, Row};
use cobolt_forms::model::{AnimKind, AnimRepeat, AnimTrigger, AnimationDef, EasingKind};

/// How a control is drawn this frame: shifted by `dx`/`dy` design pixels, scaled by `scale` about its own centre,
/// its alpha multiplied by `alpha` (`render::RenderTransform`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tf {
    pub dx: f64,
    pub dy: f64,
    pub scale: f64,
    pub alpha: f64,
}

impl Tf {
    pub const IDENTITY: Tf = Tf { dx: 0.0, dy: 0.0, scale: 1.0, alpha: 1.0 };

    /// Within a hundredth of a pixel, of a percent of scale and of alpha of drawing the control as designed.
    pub fn is_identity(&self) -> bool {
        self.dx.abs() < 0.01 && self.dy.abs() < 0.01 && (self.scale - 1.0).abs() < 1e-4 && (self.alpha - 1.0).abs() < 1e-4
    }
}

/// The ZoomOut "bounce" scale over progress `t` (`anim.rs` `zoomout_scale`): a damped oscillation that starts at
/// 100 %, dips toward 25 %, bounces 3–4 times and settles exactly at 100 %.
pub fn zoomout_scale(t: f32) -> f32 {
    const N: f32 = 5.0;
    const A: f32 = 1.06;
    const D: f32 = 3.5;
    let osc = (N * std::f32::consts::PI * t).sin();
    (1.0 - A * (-D * t).exp() * osc).max(0.02)
}

/// The offset, scale and alpha of `anim` at progress `t` on a form of `form_w` × `form_h` design pixels
/// (`anim.rs` `anim_transform`, in the same `f32` arithmetic so the numbers are the desktop's to the last bit).
pub fn anim_transform(anim: &AnimationDef, form_w: f32, form_h: f32, t: f32) -> Tf {
    let te = anim.easing.apply(t);
    let inv = 1.0 - te;
    let (dx, dy, scale, alpha): (f32, f32, f32, f32) = match &anim.kind {
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
        AnimKind::ZoomIn => (0.0, 0.0, te.max(0.001), te),
        AnimKind::ZoomOut => {
            let scale = if matches!(anim.easing, EasingKind::Elastic) {
                zoomout_scale(t)
            } else {
                (1.0 - 0.75 * (std::f32::consts::PI * te).sin()).max(0.02)
            };
            (0.0, 0.0, scale, 1.0)
        }
        AnimKind::Bounce => (0.0, -50.0 * (std::f32::consts::PI * t * 3.0).sin().abs() * inv, 1.0, 1.0),
        AnimKind::Shake => (6.0 * (t * std::f32::consts::TAU * 5.0).sin() * inv, 0.0, 1.0, 1.0),
        AnimKind::Pulse => (0.0, 0.0, 1.0 + 0.15 * (t * std::f32::consts::TAU * 2.0).sin() * inv, 1.0),
        AnimKind::Slide { dx, dy } => ((*dx as f32) * inv, (*dy as f32) * inv, 1.0, 1.0),
        AnimKind::Spin => {
            let angle = te * std::f32::consts::TAU;
            (0.0, 0.0, angle.cos().abs().max(0.05), te)
        }
        AnimKind::Flip => (0.0, 0.0, (te * std::f32::consts::PI).cos().abs().max(0.05), 1.0),
        AnimKind::None | AnimKind::Custom(_) => (0.0, 0.0, 1.0, 1.0),
    };
    Tf { dx: dx as f64, dy: dy as f64, scale: scale as f64, alpha: alpha as f64 }
}

/// Where one animation's clock stands: its progress and whether it is still moving.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clock {
    pub t: f32,
    pub playing: bool,
}

/// The clock of `def` `elapsed_ms` after it was started (`AnimRuntime::tick` and `advance_clock`): the `Delay`
/// holds it at the start; a pass runs `t` from 0 to 1 in `Duration`; `Once` rests at the end, `Loop` starts over,
/// `PingPong` turns round and `Count(n)` plays n passes; `RepeatDelay` rests between passes where the pass ended.
/// The desktop steps it frame by frame; this is the same clock read at any instant.
pub fn clock_at(def: &AnimationDef, elapsed_ms: f64) -> Clock {
    let e = elapsed_ms - def.delay_ms as f64;
    if e < 0.0 {
        return Clock { t: 0.0, playing: true };
    }
    let dur = def.duration_ms as f64;
    if dur <= 0.0 {
        return Clock { t: 1.0, playing: false };
    }
    let pause = def.repeat_delay_ms as f64;
    let pass = |r: f64| -> f32 { if r < dur { (r / dur) as f32 } else { 1.0 } };
    match &def.repeat {
        AnimRepeat::Once => {
            if e < dur {
                Clock { t: (e / dur) as f32, playing: true }
            } else {
                Clock { t: 1.0, playing: false }
            }
        }
        AnimRepeat::Loop => Clock { t: pass(e % (dur + pause)), playing: true },
        AnimRepeat::Count(n) => {
            let n = (*n).max(1) as f64;
            if e >= n * dur + (n - 1.0) * pause {
                Clock { t: 1.0, playing: false }
            } else {
                Clock { t: pass(e % (dur + pause)), playing: true }
            }
        }
        AnimRepeat::PingPong => {
            let r = e % (2.0 * (dur + pause));
            let t = if r < dur {
                r / dur
            } else if r < dur + pause {
                1.0
            } else if r < 2.0 * dur + pause {
                1.0 - (r - dur - pause) / dur
            } else {
                0.0
            };
            Clock { t: t as f32, playing: true }
        }
    }
}

/// When a playback of `def` started at 0 stops moving, in milliseconds; `None` for one that never does.
pub fn end_ms(def: &AnimationDef) -> Option<f64> {
    let dur = def.duration_ms as f64;
    let delay = def.delay_ms as f64;
    if dur <= 0.0 {
        return Some(delay);
    }
    match &def.repeat {
        AnimRepeat::Once => Some(delay + dur),
        AnimRepeat::Count(n) => {
            let n = (*n).max(1) as f64;
            Some(delay + n * dur + (n - 1.0) * def.repeat_delay_ms as f64)
        }
        AnimRepeat::Loop | AnimRepeat::PingPong => None,
    }
}

/// One animation of one control, started at `started_ms` (page time) and frozen at `paused_ms` by `Pause`.
#[derive(Debug, Clone)]
pub struct Playback {
    pub def: AnimationDef,
    /// Where `def` is in the control's own list: of the animations a control has running, the desktop draws the
    /// first in that order (`AnimRuntime::transform`).
    pub order: usize,
    pub started_ms: f64,
    pub paused_ms: Option<f64>,
}

impl Playback {
    pub fn clock(&self, now_ms: f64) -> Clock {
        let at = self.paused_ms.unwrap_or(now_ms);
        let c = clock_at(&self.def, (at - self.started_ms).max(0.0));
        if self.paused_ms.is_some() {
            Clock { playing: false, ..c }
        } else {
            c
        }
    }

    /// When it stops moving, in page time (`None`: never; a paused one has stopped).
    pub fn end(&self) -> Option<f64> {
        if let Some(p) = self.paused_ms {
            return Some(p);
        }
        end_ms(&self.def).map(|e| self.started_ms + e)
    }
}

/// The transform a control with these playbacks is drawn with at `now_ms` (identity with none). Playbacks are
/// kept in the control's own order; the first one present wins, as on the desktop.
pub fn transform_at(plays: &[Playback], form_w: f32, form_h: f32, now_ms: f64) -> Tf {
    match plays.iter().min_by_key(|p| p.order) {
        Some(p) => anim_transform(&p.def, form_w, form_h, p.clock(now_ms).t),
        None => Tf::IDENTITY,
    }
}

/// What starting animations by a trigger does to a control's list (`AnimRuntime::play` replaces the clock of an
/// animation of the same name). Returns the definitions to start, in the control's order, with their positions.
pub fn defs_for_trigger(anims: &[AnimationDef], trigger: &AnimTrigger) -> Vec<(usize, AnimationDef)> {
    anims.iter().enumerate().filter(|(_, a)| &a.trigger == trigger).map(|(i, a)| (i, a.clone())).collect()
}

/// What `PlayAnimation(name)` starts on a control (`AnimRuntime::play_programmatic`): the animation of that name
/// (any trigger), or — for a bare `Play` (the interpreter sends `1`) or an empty name — every `Programmatic` one.
pub fn defs_for_play(anims: &[AnimationDef], name: &str) -> Vec<(usize, AnimationDef)> {
    let name = name.trim();
    if !name.is_empty() && name != "1" {
        return anims.iter().enumerate().find(|(_, a)| a.name.eq_ignore_ascii_case(name)).map(|(i, a)| vec![(i, a.clone())]).unwrap_or_default();
    }
    defs_for_trigger(anims, &AnimTrigger::Programmatic)
}

/// Start `defs` on a control whose running list is `plays`, at `now_ms`: a definition already running restarts.
pub fn start(plays: &mut Vec<Playback>, defs: Vec<(usize, AnimationDef)>, now_ms: f64) {
    for (order, def) in defs {
        plays.retain(|p| !p.def.name.eq_ignore_ascii_case(&def.name));
        plays.push(Playback { def, order, started_ms: now_ms, paused_ms: None });
    }
}

// ── a control inside a container that moves ──────────────────────────────────

/// What a page applies to a control's own element: `translate(tx, ty) scale(k)` about the element's centre (CSS's
/// `transform-origin` default), and an opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Local {
    pub tx: f64,
    pub ty: f64,
    pub k: f64,
    pub opacity: f64,
}

impl Local {
    pub fn is_identity(&self) -> bool {
        self.tx.abs() < 0.01 && self.ty.abs() < 0.01 && (self.k - 1.0).abs() < 1e-4 && (self.opacity - 1.0).abs() < 1e-4
    }

    /// The CSS `transform` value.
    pub fn css(&self) -> String {
        format!("translate({}px, {}px) scale({})", round4(self.tx), round4(self.ty), round6(self.k))
    }
}

fn round4(v: f64) -> f64 {
    (v * 1e4).round() / 1e4
}

fn round6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

/// The transform a control's element needs so that it is drawn where the desktop draws it.
///
/// The desktop draws every control at its own rectangle moved by its own [`Tf`] (`render.rs`: the rect shifted by
/// `dx`/`dy`, then `scale_rect_about_center`), whatever its container is doing. On a page the element of a control
/// is inside its container's, so it moves with the container; `parent` is the container's [`Tf`] and `pc` its centre,
/// `own` and `cc` the control's, all in one frame of reference and as laid out (no transform). The answer undoes the
/// container's move and applies the control's: `D_parent⁻¹ ∘ D_control`, where `D(x) = s (x − c) + c + d`.
///
/// Opacity: the desktop multiplies a control's alpha by its OWN animation's only (its container's fade is the container's
/// own paint). A page fades a container's face, not its element (`dom::motion`), so nothing is inherited and the child's
/// opacity is its own alpha.
pub fn local_transform(parent: Tf, pc: (f64, f64), own: Tf, cc: (f64, f64)) -> Local {
    let sp = parent.scale.max(1e-6);
    let tx = (cc.0 + own.dx - pc.0 - parent.dx) / sp + pc.0 - cc.0;
    let ty = (cc.1 + own.dy - pc.1 - parent.dy) / sp + pc.1 - cc.1;
    Local { tx, ty, k: own.scale / sp, opacity: own.alpha }
}

/// A computed CSS value (`background-color`, a gradient's `background-image`, `box-shadow`) with the alpha of every
/// `rgb()` / `rgba()` colour in it multiplied by `alpha`: a container's face at that fraction of its opacity.
pub fn scale_css_alpha(value: &str, alpha: f64) -> String {
    let alpha = alpha.clamp(0.0, 1.0);
    let mut out = String::with_capacity(value.len() + 16);
    let mut rest = value;
    while let Some(at) = rest.find("rgb") {
        let Some(open) = rest[at..].find('(').map(|o| at + o) else { break };
        let Some(close) = rest[open..].find(')').map(|c| open + c) else { break };
        let parts: Vec<f64> = rest[open + 1..close].split([',', ' ', '/']).filter(|p| !p.is_empty()).filter_map(|p| p.trim().parse::<f64>().ok()).collect();
        out.push_str(&rest[..at]);
        if parts.len() >= 3 {
            let a = parts.get(3).copied().unwrap_or(1.0);
            out.push_str(&format!("rgba({}, {}, {}, {})", parts[0], parts[1], parts[2], (a * alpha * 1e4).round() / 1e4));
        } else {
            out.push_str(&rest[at..=close]);
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

/// The frames of a playback, from page time `from_ms` to `to_ms`, one every `step_ms` and one at the end: the offset
/// in `0..=1` and the value. What a page hands the browser as key frames (the browser interpolates linearly between
/// two, so the step decides how close the curve is: at 60 a second a ZoomOut / Elastic is within 0.3 % of scale).
pub fn sample_times(from_ms: f64, to_ms: f64, step_ms: f64) -> Vec<f64> {
    let span = (to_ms - from_ms).max(0.0);
    let n = (span / step_ms).ceil().max(1.0) as usize;
    (0..=n).map(|k| from_ms + span * k as f64 / n as f64).collect()
}

/// What a browser shows between two key frames of a transform: each of `dx`, `dy`, `scale` and `alpha` in a straight line
/// (`translate()` and `scale()` interpolate function by function). `times` are the frames' instants, `f` the value at one.
pub fn lerp_tf(times: &[f64], f: &dyn Fn(f64) -> Tf, t: f64) -> Tf {
    let k = times.partition_point(|&u| u <= t);
    if k == 0 || k >= times.len() {
        return f(t);
    }
    let (a, b) = (times[k - 1], times[k]);
    if b <= a {
        return f(t);
    }
    let (fa, fb, w) = (f(a), f(b), (t - a) / (b - a));
    let l = |x: f64, y: f64| x + (y - x) * w;
    Tf { dx: l(fa.dx, fb.dx), dy: l(fa.dy, fb.dy), scale: l(fa.scale, fb.scale), alpha: l(fa.alpha, fb.alpha) }
}

/// Where a point `x` of a control (as laid out) is drawn when its element is given `l` inside a container shown at `p`
/// about `pc`; `cc` is the control's centre.
pub fn composed(p: Tf, pc: (f64, f64), l: Local, cc: (f64, f64), x: (f64, f64)) -> (f64, f64) {
    let y = (cc.0 + l.tx + l.k * (x.0 - cc.0), cc.1 + l.ty + l.k * (x.1 - cc.1));
    (pc.0 + p.dx + p.scale * (y.0 - pc.0), pc.1 + p.dy + p.scale * (y.1 - pc.1))
}

/// The instants to give the frames of a control inside a container that moves too. The browser joins two frames of the
/// container and two of the control each by a straight line, and the two composed bend away from the desktop's path in
/// between (1.7 px at 60 frames a second on the first dip of the intro). Starting from `coarse`, an interval is halved,
/// twice at most, where at its middle a corner of the control (`cc` ± `half`) would be drawn more than `tol` px from the
/// desktop's place; where the motion is gentle the frames stay at the coarse step.
#[allow(clippy::too_many_arguments)]
pub fn refine(coarse: &[f64], parent: &dyn Fn(f64) -> Tf, own: &dyn Fn(f64) -> Tf, pc: (f64, f64), cc: (f64, f64), half: (f64, f64), tol: f64) -> Vec<f64> {
    let local = |t: f64| local_transform(parent(t), pc, own(t), cc);
    let corners = [(cc.0 - half.0, cc.1 - half.1), (cc.0 + half.0, cc.1 - half.1), (cc.0 - half.0, cc.1 + half.1), (cc.0 + half.0, cc.1 + half.1)];
    let off = |t: f64, l: Local| -> f64 {
        let (p, o) = (parent(t), own(t));
        corners
            .iter()
            .map(|&x| {
                let got = composed(p, pc, l, cc, x);
                let want = (cc.0 + o.dx + o.scale * (x.0 - cc.0), cc.1 + o.dy + o.scale * (x.1 - cc.1));
                (got.0 - want.0).hypot(got.1 - want.1)
            })
            .fold(0.0, f64::max)
    };
    let mut ts = coarse.to_vec();
    for _ in 0..2 {
        if ts.len() < 2 {
            break;
        }
        let mut out = vec![ts[0]];
        for w in ts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let m = (a + b) / 2.0;
            let (la, lb) = (local(a), local(b));
            let mid = Local { tx: (la.tx + lb.tx) / 2.0, ty: (la.ty + lb.ty) / 2.0, k: (la.k + lb.k) / 2.0, opacity: (la.opacity + lb.opacity) / 2.0 };
            if off(m, mid) > tol {
                out.push(m);
            }
            out.push(b);
        }
        ts = out;
    }
    ts
}

/// The page's frame step for key frames: 60 a second, the rate the desktop paints an animation at.
pub const STEP_MS: f64 = 1000.0 / 60.0;

// ── charts ───────────────────────────────────────────────────────────────────

/// `chart::ease_out`: the painter's decelerating ease for both chart motions (`1 − (1 − t)³`).
pub fn chart_ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// How long a chart's motion takes: `AnimationDuration` (2000 when the chart has none), never under 250 ms
/// (`chart::value_anim_ms`, and the grow in `render.rs`). The text is read as the desktop reads it: typed by the
/// loader (`xml::parse_prop_value`), then `as_i64`, so a duration that is not a whole number (`""`, `"fast"`,
/// `"1500.5"`) is 0 there, and 250 ms on both.
pub fn chart_ms(duration: Option<&str>) -> f64 {
    duration
        .map(|d| cobolt_forms::xml::parse_prop_value(d).as_i64())
        .unwrap_or(chart::DEFAULT_ANIM_MS)
        .max(chart::MIN_ANIM_MS) as f64
}

/// A chart's `AnimateOnLoad` / `AnimateValues` as the desktop reads it (`prop_bool`, `PropValue::as_bool` of the
/// loader's typed value): `default` when the chart has none; `"False"` is on, as there.
pub fn chart_flag(raw: Option<&str>, default: bool) -> bool {
    raw.map(|v| cobolt_forms::xml::parse_prop_value(v).as_bool()).unwrap_or(default)
}

/// One chart's motion state on a page: what the desktop keeps in its frame memory for it (`render.rs`:
/// `chart_grow_start`, `ChartTween`). It changes only while the chart is drawn, as on the desktop: a chart that is not
/// on screen neither starts growing nor sets off on a change of data until it is shown.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChartMotion {
    /// When the load growth started (`AnimateOnLoad`): once a chart, the first time it is drawn with data.
    pub grow_started: Option<f64>,
    /// The value tween (`AnimateValues`): the series it set off from, the one it is heading for, when it started and
    /// the data text `to` was read from. `None` until the chart is first drawn.
    pub tween: Option<Tween>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tween {
    pub from: Vec<Row>,
    pub to: Vec<Row>,
    pub started: f64,
    pub raw: String,
}

/// What a chart is drawn with at one moment: the grow fraction (`__ChartGrow`, 1 at rest) and the data to draw
/// (`None`: the chart's own).
#[derive(Debug, Clone, PartialEq)]
pub struct ChartFrame {
    pub grow: Option<f32>,
    pub data: Option<String>,
}

impl ChartMotion {
    /// The chart was drawn at `now` with `raw` as its data: start or continue its motions, as `render.rs` does on
    /// every frame it draws the chart, and answer what to draw. `grows` is `AnimateOnLoad` (on unless the chart says
    /// no), `values` is `AnimateValues` (off unless it says yes), `ms` the duration of both.
    pub fn frame(&mut self, now: f64, raw: &str, grows: bool, values: bool, ms: f64) -> ChartFrame {
        let mut data = None;
        if values {
            let target = chart::parse_chart_rows(raw);
            let restart = self.tween.as_ref().map(|s| s.to != target).unwrap_or(true);
            if restart {
                let from = match &self.tween {
                    Some(s) if now - s.started < ms => chart::tween_rows(&s.from, &s.to, ((now - s.started) / ms) as f32),
                    Some(s) => s.to.clone(),
                    // The first data a chart is given is not animated (render.rs: growing it from zero looks like an
                    // animation and is not one; the auto-scale keeps every bar where it was).
                    None => target.clone(),
                };
                self.tween = Some(Tween { from, to: target, started: now, raw: raw.to_owned() });
            }
            let s = self.tween.as_ref().expect("set above");
            let t = ((now - s.started) / ms).clamp(0.0, 1.0);
            // A tween from a set to the same set draws that set: the page leaves the chart's own data in place.
            if t < 1.0 && s.from != s.to {
                data = Some(chart::format_chart_rows(&chart::tween_rows(&s.from, &s.to, t as f32)));
            }
        } else {
            // Parked on the data being drawn, so switching the property on starts from what is on the screen.
            let rows = chart::parse_chart_rows(raw);
            if self.tween.as_ref().map(|s| s.raw != raw || s.started != f64::NEG_INFINITY).unwrap_or(true) {
                self.tween = Some(Tween { from: rows.clone(), to: rows, started: f64::NEG_INFINITY, raw: raw.to_owned() });
            }
        }
        let mut grow = None;
        if !raw.trim().is_empty() && grows {
            let started = *self.grow_started.get_or_insert(now);
            let t = ((now - started) / ms).clamp(0.0, 1.0);
            if t < 1.0 {
                // The painter is handed the fraction as text with four decimals (`format!("{eased:.4}")`).
                let eased = 1.0 - (1.0 - t as f32).powi(3);
                grow = Some(format!("{eased:.4}").parse::<f32>().unwrap_or(eased));
            }
        }
        ChartFrame { grow, data }
    }

    /// Whether a frame drawn at `now` would still be moving (the page asks for another frame while it is).
    pub fn moving(&self, now: f64, ms: f64) -> bool {
        self.grow_started.is_some_and(|s| now - s < ms) || self.tween.as_ref().is_some_and(|s| s.started.is_finite() && s.from != s.to && now - s.started < ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intro() -> AnimationDef {
        // `<Animation name="intro" trigger="Programmatic" kind="ZoomOut" duration="1800" delay="0" easing="Elastic" repeat="Once"/>`
        let mut a = AnimationDef::new("intro");
        a.trigger = AnimTrigger::Programmatic;
        a.kind = AnimKind::ZoomOut;
        a.duration_ms = 1800;
        a.easing = EasingKind::Elastic;
        a.repeat = AnimRepeat::Once;
        a
    }

    /// The PowerAnalytics "intro": the desktop's damped bounce, 1800 ms, at the instants a judge samples. The values are
    /// `1 − 1.06·e^(−3.5t)·sin(5πt)` (anim.rs:32-41), worked by hand.
    #[test]
    fn the_intro_bounce_is_the_desktops_curve() {
        let a = intro();
        let at = |ms: f64| anim_transform(&a, 1440.0, 900.0, clock_at(&a, ms).t).scale;
        assert_eq!(at(0.0), 1.0, "100 % at the start");
        // t = 0.1 (180 ms): 1 − 1.06·e^(−0.35)·sin(0.5π) = 1 − 1.06·0.704688 = 0.253031
        assert!((at(180.0) - 0.253031).abs() < 1e-4, "the first dip, about 25 %: {}", at(180.0));
        // t = 0.3 (540 ms): 1 + 1.06·e^(−1.05) = 1.370963
        assert!((at(540.0) - 1.370963).abs() < 1e-4, "the first overshoot: {}", at(540.0));
        assert!((at(1800.0) - 1.0).abs() < 1e-6, "100 % at the end");
        assert_eq!(at(5000.0), 1.0, "and at rest after it");
        assert_eq!(anim_transform(&a, 1440.0, 900.0, 0.5).alpha, 1.0, "a ZoomOut never fades");
        assert!(!clock_at(&a, 1800.0).playing && clock_at(&a, 1799.0).playing);
    }

    #[test]
    fn the_clock_honours_delay_and_every_repeat() {
        let mut a = intro();
        a.easing = EasingKind::Linear;
        a.kind = AnimKind::FadeIn;
        a.duration_ms = 1000;
        a.delay_ms = 200;
        assert_eq!(clock_at(&a, 100.0).t, 0.0, "held at the start through the delay");
        assert!((clock_at(&a, 700.0).t - 0.5).abs() < 1e-6);
        assert_eq!(end_ms(&a), Some(1200.0));
        a.delay_ms = 0;
        a.repeat = AnimRepeat::Loop;
        a.repeat_delay_ms = 500;
        assert!((clock_at(&a, 1200.0).t - 1.0).abs() < 1e-6, "resting where the pass ended");
        assert!((clock_at(&a, 1600.0).t - 0.1).abs() < 1e-6, "the next pass from the start");
        assert_eq!(end_ms(&a), None);
        a.repeat = AnimRepeat::PingPong;
        a.repeat_delay_ms = 0;
        assert!((clock_at(&a, 1250.0).t - 0.75).abs() < 1e-6, "on the way back");
        a.repeat = AnimRepeat::Count(2);
        a.repeat_delay_ms = 100;
        assert_eq!(end_ms(&a), Some(2100.0));
        assert!((clock_at(&a, 1150.0).t - 0.05).abs() < 1e-6);
        assert!(!clock_at(&a, 2100.0).playing && clock_at(&a, 2100.0).t == 1.0);
    }

    #[test]
    fn every_kind_lands_where_the_desktop_lands() {
        let mut a = intro();
        a.easing = EasingKind::EaseOut;
        for kind in AnimKind::ALL {
            a.kind = AnimKind::from_str(kind);
            let end = anim_transform(&a, 800.0, 600.0, 1.0);
            let start = anim_transform(&a, 800.0, 600.0, 0.0);
            match *kind {
                "FadeOut" => assert_eq!(end.alpha, 0.0, "a FadeOut rests invisible"),
                "Spin" | "Flip" => {}
                _ => assert!(end.is_identity(), "{kind} rests as designed: {end:?}"),
            }
            if kind.starts_with("FlyFrom") {
                assert!(start.dx.abs() == 800.0 || start.dy.abs() == 600.0, "{kind} starts a form away: {start:?}");
            }
        }
    }

    /// A card and its label both play the intro at the same instant. On the desktop each shrinks about its own
    /// centre. The page gives the label the transform that, composed with the card's, lands it on exactly that.
    #[test]
    fn a_child_lands_where_the_desktop_draws_it_inside_a_moving_container() {
        let compose = |p: Tf, pc: (f64, f64), l: Local, cc: (f64, f64), x: (f64, f64)| {
            // the child's own transform about its centre, then the container's about its centre
            let y = (cc.0 + l.tx + l.k * (x.0 - cc.0), cc.1 + l.ty + l.k * (x.1 - cc.1));
            (pc.0 + p.dx + p.scale * (y.0 - pc.0), pc.1 + p.dy + p.scale * (y.1 - pc.1))
        };
        let card = Tf { dx: 0.0, dy: 0.0, scale: 0.253, alpha: 1.0 };
        let own = Tf { dx: 0.0, dy: 0.0, scale: 0.253, alpha: 1.0 };
        let (pc, cc) = ((200.0, 150.0), (80.0, 40.0));
        let l = local_transform(card, pc, own, cc);
        for x in [(60.0, 30.0), (100.0, 50.0), (80.0, 40.0)] {
            let got = compose(card, pc, l, cc, x);
            let want = (cc.0 + own.scale * (x.0 - cc.0), cc.1 + own.scale * (x.1 - cc.1));
            assert!((got.0 - want.0).abs() < 1e-9 && (got.1 - want.1).abs() < 1e-9, "{x:?}: {got:?} vs {want:?}");
        }
        assert!((l.k - 1.0).abs() < 1e-12, "the same scale twice is no extra scale");
        // A fly-in container and a child that does not move: the child stays put.
        let fly = Tf { dx: -300.0, dy: 0.0, scale: 1.0, alpha: 1.0 };
        let still = local_transform(fly, pc, Tf::IDENTITY, cc);
        assert!((still.tx - 300.0).abs() < 1e-9 && still.ty.abs() < 1e-9);
        // Opacity: the child's own alpha, whatever its container's fade (judgement P4-1: a FadeIn child inside a FadeOut
        // container ends fully visible on the desktop).
        let fade = Tf { alpha: 0.5, ..Tf::IDENTITY };
        assert_eq!(local_transform(fade, pc, Tf { alpha: 0.25, ..Tf::IDENTITY }, cc).opacity, 0.25);
        assert_eq!(local_transform(fade, pc, Tf::IDENTITY, cc).opacity, 1.0);
        assert_eq!(local_transform(Tf { alpha: 0.0, ..Tf::IDENTITY }, pc, Tf::IDENTITY, cc).opacity, 1.0);
        // The container's face at a fraction of its alpha.
        assert_eq!(scale_css_alpha("rgb(10, 20, 30)", 0.5), "rgba(10, 20, 30, 0.5)");
        assert_eq!(scale_css_alpha("rgba(0, 0, 0, 0.4) 0px 2px 6px 0px", 0.5), "rgba(0, 0, 0, 0.2) 0px 2px 6px 0px");
        assert_eq!(
            scale_css_alpha("linear-gradient(rgba(78, 78, 78, 0.7), rgb(0, 0, 0))", 0.0),
            "linear-gradient(rgba(78, 78, 78, 0), rgba(0, 0, 0, 0))"
        );
        assert_eq!(scale_css_alpha("none", 0.3), "none");
    }

    /// A label inside a card, both playing the intro: the frames refined where the motion is fast keep every corner of
    /// the label within a quarter pixel of the desktop's place at every millisecond, with far fewer frames than a uniform
    /// fine step would need.
    #[test]
    fn frames_are_added_only_where_the_composed_motion_needs_them() {
        let a = intro();
        let tf = |t: f64| anim_transform(&a, 1440.0, 900.0, clock_at(&a, t).t);
        let coarse = sample_times(0.0, 1800.0, STEP_MS);
        let parent = |t: f64| lerp_tf(&coarse, &|u| tf(u), t);
        let (pc, cc, half) = ((180.0, 200.0), (130.0, 170.0), (50.0, 20.0));
        let ts = refine(&coarse, &parent, &tf, pc, cc, half, 0.25);
        assert!(ts.len() > coarse.len() && ts.len() < 3 * coarse.len(), "{} frames (coarse {})", ts.len(), coarse.len());
        let local = |t: f64| local_transform(parent(t), pc, tf(t), cc);
        let frames: Vec<Local> = ts.iter().map(|&t| local(t)).collect();
        let mut worst: f64 = 0.0;
        for ms in 0..=1800 {
            let t = ms as f64;
            let k = ts.partition_point(|&u| u <= t).clamp(1, ts.len() - 1);
            let (ta, tb) = (ts[k - 1], ts[k]);
            let w = if tb > ta { (t - ta) / (tb - ta) } else { 0.0 };
            let (la, lb) = (frames[k - 1], frames[k]);
            let l = Local { tx: la.tx + (lb.tx - la.tx) * w, ty: la.ty + (lb.ty - la.ty) * w, k: la.k + (lb.k - la.k) * w, opacity: 1.0 };
            let o = tf(t);
            for x in [(80.0, 150.0), (180.0, 190.0)] {
                let got = composed(parent(t), pc, l, cc, x);
                let want = (cc.0 + o.dx + o.scale * (x.0 - cc.0), cc.1 + o.dy + o.scale * (x.1 - cc.1));
                worst = worst.max((got.0 - want.0).hypot(got.1 - want.1));
            }
        }
        assert!(worst < 0.5, "a corner strays {worst:.3} px from the desktop's place");
        println!("intro, label in card: {} frames (60 a second: {}), worst corner error {worst:.3} px", ts.len(), coarse.len());
    }

    #[test]
    fn play_by_name_bare_play_and_restart() {
        let mut other = intro();
        other.name = "pulse".into();
        other.trigger = AnimTrigger::OnClick;
        let anims = vec![intro(), other];
        assert_eq!(defs_for_play(&anims, "INTRO").len(), 1);
        assert_eq!(defs_for_play(&anims, "pulse")[0].0, 1, "by name, whatever its trigger");
        assert_eq!(defs_for_play(&anims, "1").iter().map(|d| d.1.name.as_str()).collect::<Vec<_>>(), ["intro"], "bare: the Programmatic ones");
        assert!(defs_for_play(&anims, "nope").is_empty());
        let mut plays = Vec::new();
        start(&mut plays, defs_for_play(&anims, "intro"), 100.0);
        start(&mut plays, defs_for_play(&anims, "intro"), 900.0);
        assert_eq!(plays.len(), 1, "playing it again restarts it");
        assert_eq!(plays[0].started_ms, 900.0);
        start(&mut plays, defs_for_play(&anims, "pulse"), 950.0);
        let tf = transform_at(&plays, 800.0, 600.0, 1080.0);
        assert!((tf.scale - anim_transform(&anims[0], 800.0, 600.0, 0.1).scale).abs() < 1e-9, "the first in the control's order is drawn");
    }

    /// The page moves nothing the desktop does not: the Switch's thumb slid in 120 ms (`transition: left .12s`), where
    /// the desktop paints it at its place at once (`render.rs` CT::Switch: "costs the crate's knob-slide animation").
    #[test]
    fn the_base_stylesheet_moves_nothing_by_itself() {
        let css = crate::base_css::base_layer();
        assert!(!css.contains("transition") && !css.contains("@keyframes") && !css.contains("animation:"), "a motion the desktop does not have");
    }

    /// The control's animations reach the player through the form description the Build writes (`describe`/`to_json`,
    /// then `from_json`/`to_form`), so `PlayAnimation("intro")` finds the definition the designer saved.
    #[test]
    fn a_controls_animations_reach_the_tree_through_the_description() {
        use cobolt_forms::description::{describe, from_json, to_form, to_json};
        let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Demo" width="640" height="480" index-base="1">
  <Control id="C-KPI" type="GroupBox" x="10" y="10" w="300" h="200"><Animation name="intro" trigger="Programmatic" kind="ZoomOut" duration="1800" delay="0" easing="Elastic" repeat="Once" slide-dx="0" slide-dy="0"/></Control>
</Form>"##;
        let form = cobolt_forms::xml::load_form_from_str(xml).unwrap();
        let form = to_form(&from_json(&to_json(&describe(&form))).unwrap()).unwrap();
        let (_, mut manifest) = crate::tree::testing::demo();
        manifest.forms[0].controls.clear();
        manifest.forms[0].handlers.clear();
        let t = crate::tree::FormTree::build(&form, &manifest.forms[0], &manifest, 0, None).unwrap();
        let c = t.by_name("C-KPI").unwrap();
        let a = &t.ctls[c].anims;
        assert_eq!(a.len(), 1);
        assert_eq!((a[0].name.as_str(), a[0].duration_ms, &a[0].kind, &a[0].easing, &a[0].trigger), ("intro", 1800, &AnimKind::ZoomOut, &EasingKind::Elastic, &AnimTrigger::Programmatic));
        assert_eq!(defs_for_play(a, "intro").len(), 1);
    }

    /// `AnimateOnLoad`: the first frame with data starts the growth, the painter's ease-out, once a chart.
    /// `AnimateValues`: the first data is not animated; a change travels from what is on the screen.
    #[test]
    fn a_chart_grows_once_and_travels_on_a_change() {
        let mut m = ChartMotion::default();
        let f0 = m.frame(1000.0, "A\t10\nB\t20", true, true, 1500.0);
        assert_eq!(f0, ChartFrame { grow: Some(0.0), data: None }, "grows from zero, data as given");
        let f1 = m.frame(1750.0, "A\t10\nB\t20", true, true, 1500.0);
        assert_eq!(f1.grow, Some(0.875), "1 − 0.5³ at half time");
        assert_eq!(m.frame(2500.0, "A\t10\nB\t20", true, true, 1500.0).grow, None, "at rest after 1500 ms");
        let f2 = m.frame(3000.0, "A\t30\nB\t20", true, true, 1500.0);
        assert_eq!(f2.grow, None, "never grows again");
        assert_eq!(f2.data.as_deref(), Some("A\t10\nB\t20"), "sets off from the old values");
        let f3 = m.frame(3750.0, "A\t30\nB\t20", true, true, 1500.0);
        assert_eq!(f3.data.as_deref(), Some("A\t27.5\nB\t20"), "10 + 20 × 0.875");
        assert!(m.moving(3750.0, 1500.0) && !m.moving(4600.0, 1500.0));
        // Off: nothing travels, and switching it on later starts from what is drawn.
        let mut off = ChartMotion::default();
        off.frame(0.0, "A\t1", false, false, 1500.0);
        off.frame(10.0, "A\t5", false, false, 1500.0);
        assert_eq!(off.frame(20.0, "A\t9", false, true, 1500.0).data.as_deref(), Some("A\t5"));
        assert_eq!(chart_ms(Some("10")), 250.0);
        assert_eq!(chart_ms(None), 2000.0);
        // Judgement P4-1 (static finding): what is not a whole number is 0 on the desktop (`as_i64`), so 250 ms.
        for odd in ["", "fast", "1500.5", "true"] {
            assert_eq!(chart_ms(Some(odd)), 250.0, "{odd:?}");
        }
        assert_eq!(chart_ms(Some(" 1500")), 1500.0, "the loader trims");
        assert!(chart_flag(Some("False"), false), "\"False\" is a String, which `as_bool` reads as on");
        assert!(!chart_flag(Some("0"), true) && !chart_flag(Some("false"), true) && !chart_flag(Some(""), true));
        assert!(chart_flag(None, true) && !chart_flag(None, false));
    }
}
