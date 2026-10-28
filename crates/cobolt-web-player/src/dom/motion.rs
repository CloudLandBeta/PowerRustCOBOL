// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Motion on the page** (the PowerAnalyticsWeb parity job, P4): the desktop's control animations played with the
//! browser's own animation engine, and the charts' growth and value travel drawn frame by frame.
//!
//! * A control animation (`PlayAnimation`, `StopAnimation`, `Pause`; the `OnFormLoad`, `OnShow`, `OnClick`, `OnHover`
//!   and `OnFocus` triggers) becomes a Web Animation of the control's element: `transform` (and `opacity` for the kinds
//!   that fade), key frames sampled 60 times a second from the desktop's own curve (`crate::motion`), so the browser's
//!   compositor plays it with no script running per frame. A control inside a container that also moves is given the
//!   transform that lands it where the desktop draws it (`motion::local_transform`); while a container moves, its
//!   own clipping is lifted (`overflow: visible`), and while a control grows past its place the clipping of the
//!   containers around it is lifted too: the desktop clips a control to its containers' rectangles as laid out, and
//!   lets one that grows bleed out of them. Every one of these effects ends by itself (`fill: none`) unless the
//!   desktop rests the control somewhere else than as designed (a `FadeOut`); none leaves an inline style behind.
//! * A chart's motion is its painter's geometry (`view::chart_paint`) drawn at the grow fraction and with the series of
//!   the moment, once per animation frame while it moves (the desktop re-draws it every frame too); a chart starts
//!   moving only once it is on screen, as the desktop's frame loop does.
//! * `prefers-reduced-motion: reduce` (the operating system's own setting) plays nothing: every control and chart is
//!   drawn where its motion would have ended. The desktop has no such setting; this is the browser's convention, and
//!   without it the page moves as the desktop does.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use js_sys::{Array, Object, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::Element;

use super::h::Listener;
use super::Shared;
use crate::api::Inner;
use crate::motion::{self as m, ChartFrame, ChartMotion, Local, Playback, Tf};
use crate::tree::{FormTree, Kind};

/// Page time, in milliseconds: the clock Web Animations run on.
pub(crate) fn now() -> f64 {
    web_sys::window().and_then(|w| w.performance()).map(|p| p.now()).unwrap_or_else(js_sys::Date::now)
}

/// The operating system asks for less motion.
fn reduced() -> bool {
    web_sys::window()
        .and_then(|w| w.match_media("(prefers-reduced-motion: reduce)").ok().flatten())
        .map(|q| q.matches())
        .unwrap_or(false)
}

/// One `requestAnimationFrame`, cancelled when dropped (no `Closure::forget`).
pub(crate) struct Raf {
    id: JsValue,
    fired: Rc<std::cell::Cell<bool>>,
    _cb: Closure<dyn FnMut()>,
}

impl Raf {
    fn new(f: impl FnOnce() + 'static) -> Option<Raf> {
        let cell = RefCell::new(Some(f));
        let fired = Rc::new(std::cell::Cell::new(false));
        let flag = fired.clone();
        let cb = Closure::<dyn FnMut()>::new(move || {
            flag.set(true);
            if let Some(f) = cell.borrow_mut().take() {
                f();
            }
        });
        let w = web_sys::window()?;
        let id = Reflect::get(&w, &"requestAnimationFrame".into()).ok()?.dyn_into::<js_sys::Function>().ok()?.call1(&w, cb.as_ref().unchecked_ref()).ok()?;
        Some(Raf { id, fired, _cb: cb })
    }
}

impl Drop for Raf {
    fn drop(&mut self) {
        if self.fired.get() {
            return;
        }
        if let Some(w) = web_sys::window() {
            if let Some(f) = Reflect::get(&w, &"cancelAnimationFrame".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()) {
                let _ = f.call1(&w, &self.id);
            }
        }
    }
}

/// The motion of one form instance.
#[derive(Default)]
pub(crate) struct InstMotion {
    /// The animations running (or resting where they ended), by control.
    plays: BTreeMap<usize, Vec<Playback>>,
    /// The Web Animations the page started, by control, to cancel when the control is planned again.
    running: BTreeMap<usize, Vec<JsValue>>,
    /// The containers whose clipping a plan lifted, by the plan's root.
    lifted: BTreeMap<usize, Vec<JsValue>>,
    /// Plans with a control that was not on screen when they were made: made again when it is.
    pending: BTreeMap<usize, BTreeSet<usize>>,
    /// Plans of an animation that never ends, made again before their frames run out.
    renew: BTreeMap<usize, crate::api::Timeout>,
    /// The controls the pointer is over (`OnHover` fires on entering one).
    hovered: BTreeSet<usize>,
    /// The charts' motion state.
    pub charts: BTreeMap<usize, ChartMotion>,
    _listeners: Vec<Listener>,
}

#[derive(Default)]
pub(crate) struct MotionState {
    pub insts: BTreeMap<u32, InstMotion>,
    raf: Option<Raf>,
    spent: Option<Raf>,
}

/// What a plan needs from the tree, read while the host is borrowed.
struct Snapshot {
    form: (f32, f32),
    parent: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
    container: Vec<bool>,
    scroller: Vec<bool>,
}

fn snapshot(tree: &FormTree) -> Snapshot {
    let n = tree.ctls.len();
    Snapshot {
        form: (tree.design_size.0 as f32, tree.design_size.1 as f32),
        parent: (0..n).map(|i| tree.ctls[i].parent.filter(|_| i != 0)).collect(),
        children: (0..n).map(|i| tree.ctls[i].children.clone()).collect(),
        container: (0..n).map(|i| tree.ctls[i].is_container() && i != 0).collect(),
        scroller: (0..n).map(|i| ["HScroll", "VScroll", "AutoScroll"].iter().any(|p| tree.ctls[i].flag(p))).collect(),
    }
}

fn displayed(el: &Element) -> bool {
    Reflect::get(el, &"checkVisibility".into())
        .ok()
        .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
        .and_then(|f| f.call0(el).ok())
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| { let r = el.get_bounding_client_rect(); r.width() > 0.0 || r.height() > 0.0 })
}

fn cancel(anims: Vec<JsValue>) {
    for a in anims {
        if let Some(f) = Reflect::get(&a, &"cancel".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()) {
            let _ = f.call0(&a);
        }
    }
}

/// `el.animate(frames, {duration, fill, composite})`, its start set to page time `start`: every animation of a plan starts at
/// the instant its frames were computed for, so a container and the controls inside it move as one.
fn animate(el: &Element, frames: &Array, duration: f64, fill: &str, composite: &str, start: f64) -> Option<JsValue> {
    let opts = Object::new();
    let _ = Reflect::set(&opts, &"duration".into(), &duration.max(1.0).into());
    let _ = Reflect::set(&opts, &"fill".into(), &fill.into());
    let _ = Reflect::set(&opts, &"easing".into(), &"linear".into());
    let _ = Reflect::set(&opts, &"composite".into(), &composite.into());
    let f = Reflect::get(el, &"animate".into()).ok()?.dyn_into::<js_sys::Function>().ok()?;
    let a = f.call2(el, frames, &opts).ok()?;
    let _ = Reflect::set(&a, &"startTime".into(), &start.into());
    Some(a)
}

fn frame(offset: f64, props: &[(&str, JsValue)]) -> JsValue {
    let o = Object::new();
    let _ = Reflect::set(&o, &"offset".into(), &offset.into());
    for (k, v) in props {
        let _ = Reflect::set(&o, &(*k).into(), v);
    }
    o.into()
}

impl Shared {
    fn motion_inst<R>(&self, inst: u32, f: impl FnOnce(&mut InstMotion) -> R) -> Option<R> {
        let mut st = self.motion.try_borrow_mut().ok()?;
        Some(f(st.insts.entry(inst).or_default()))
    }

    // ── an instance comes and goes ───────────────────────────────────────────

    /// A form instance is on the page: listen for the triggers its controls' animations name, and start its load
    /// animations (`AnimRuntime::start_form_load`: `OnFormLoad` and `OnShow`, once, when the window comes up).
    pub(super) fn motion_open(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, root: &Element) {
        use cobolt_forms::model::AnimTrigger as T;
        let (load, wants_click, wants_hover, wants_focus) = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            let has = |t: &T| tree.ctls.iter().any(|c| c.anims.iter().any(|a| &a.trigger == t));
            let load: Vec<(usize, Vec<(usize, cobolt_forms::model::AnimationDef)>)> = (1..tree.ctls.len())
                .map(|i| {
                    let mut d = m::defs_for_trigger(&tree.ctls[i].anims, &T::OnFormLoad);
                    d.extend(m::defs_for_trigger(&tree.ctls[i].anims, &T::OnShow));
                    d.sort_by_key(|(o, _)| *o);
                    (i, d)
                })
                .filter(|(_, d)| !d.is_empty())
                .collect();
            (load, has(&T::OnClick), has(&T::OnHover), has(&T::OnFocus))
        };
        let mut listeners = Vec::new();
        let weak = Rc::downgrade(self);
        let on = |kind: &str, trig: T, listeners: &mut Vec<Listener>| {
            let w = weak.clone();
            let root_c = root.clone();
            listeners.push(Listener::new(root, kind, move |ev| {
                if let Some(sh) = w.upgrade() {
                    sh.motion_event(inst, &root_c, &ev, &trig);
                }
            }));
        };
        if wants_click {
            on("click", T::OnClick, &mut listeners);
        }
        if wants_hover {
            on("pointerover", T::OnHover, &mut listeners);
            let w = weak.clone();
            listeners.push(Listener::new(root, "pointerleave", move |_| {
                if let Some(sh) = w.upgrade() {
                    sh.motion_inst(inst, |mi| mi.hovered.clear());
                }
            }));
        }
        if wants_focus {
            on("focusin", T::OnFocus, &mut listeners);
        }
        self.motion_inst(inst, |mi| mi._listeners = listeners);
        if !load.is_empty() {
            let t = now();
            let mut roots = BTreeSet::new();
            self.motion_inst(inst, |mi| {
                for (i, defs) in load {
                    m::start(mi.plays.entry(i).or_default(), defs, t);
                }
            });
            if let Some(r) = self.motion_roots(inner, inst, &self.motion_ctls(inst)) {
                roots.extend(r);
            }
            for r in roots {
                self.motion_plan(inner, inst, r);
            }
        }
    }

    pub(super) fn motion_close(&self, inst: u32) {
        let gone = self.motion.try_borrow_mut().ok().and_then(|mut st| st.insts.remove(&inst));
        if let Some(mi) = gone {
            for (_, a) in mi.running {
                cancel(a);
            }
            for (_, a) in mi.lifted {
                cancel(a);
            }
        }
    }

    fn motion_ctls(&self, inst: u32) -> Vec<usize> {
        self.motion.try_borrow().ok().and_then(|st| st.insts.get(&inst).map(|mi| mi.plays.keys().copied().collect())).unwrap_or_default()
    }

    /// A pointer or focus event reached the form: the controls it concerns start their animations of that trigger
    /// (`AnimRuntime::fire_event`). The desktop starts `OnClick` on every control under the pointer and `OnHover` on
    /// every control the pointer enters; the page takes the controls whose elements hold the event's target.
    fn motion_event(self: &Rc<Self>, inst: u32, root: &Element, ev: &web_sys::Event, trig: &cobolt_forms::model::AnimTrigger) {
        let Some(inner) = self.inner.upgrade() else { return };
        // The controls whose elements contain the target, innermost first.
        let under: Vec<usize> = {
            let Ok(st) = self.state.try_borrow() else { return };
            let Some(iv) = st.instances.get(&inst) else { return };
            let mut out = Vec::new();
            let mut node = ev.target().and_then(|t| t.dyn_into::<Element>().ok());
            while let Some(el) = node {
                if &el == root {
                    break;
                }
                if el.has_attribute("data-name") {
                    if let Some(i) = iv.ctls.iter().position(|c| c.as_ref().is_some_and(|c| c.rec.el == el)) {
                        out.push(i);
                    }
                }
                node = el.parent_element();
            }
            out
        };
        let targets: Vec<usize> = if matches!(trig, cobolt_forms::model::AnimTrigger::OnHover) {
            let now_over: BTreeSet<usize> = under.iter().copied().collect();
            self.motion_inst(inst, |mi| {
                let entered: Vec<usize> = now_over.difference(&mi.hovered).copied().collect();
                mi.hovered = now_over;
                entered
            })
            .unwrap_or_default()
        } else if matches!(trig, cobolt_forms::model::AnimTrigger::OnFocus) {
            under.first().copied().into_iter().collect()
        } else {
            under
        };
        self.motion_start(&inner, inst, &targets, |anims| m::defs_for_trigger(anims, trig));
    }

    /// Start, on each of `ctls`, the animations `pick` chooses from its own, and play them.
    fn motion_start(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, ctls: &[usize], pick: impl Fn(&[cobolt_forms::model::AnimationDef]) -> Vec<(usize, cobolt_forms::model::AnimationDef)>) {
        let picked: Vec<(usize, Vec<(usize, cobolt_forms::model::AnimationDef)>)> = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            ctls.iter().filter(|&&i| i < tree.ctls.len()).map(|&i| (i, pick(&tree.ctls[i].anims))).filter(|(_, d)| !d.is_empty()).collect()
        };
        if picked.is_empty() {
            return;
        }
        let t = now();
        let started: Vec<usize> = picked.iter().map(|(i, _)| *i).collect();
        self.motion_inst(inst, |mi| {
            for (i, defs) in picked {
                m::start(mi.plays.entry(i).or_default(), defs, t);
            }
        });
        if let Some(roots) = self.motion_roots(inner, inst, &started) {
            for r in roots {
                self.motion_plan(inner, inst, r);
            }
        }
    }

    // ── the program's verbs ──────────────────────────────────────────────────

    /// `PlayAnimation(name)` / `Play`, `StopAnimation`, `Pause` on control `ctl` (`methods.rs` records them as commands; the
    /// desktop host routes the same three verbs to `AnimRuntime::play_programmatic`, `stop_all` and `pause_all`). Answers the
    /// control, whose plan must be made again. An Animator's verbs are its media's, not these.
    pub(super) fn motion_command(&self, inner: &Rc<Inner>, inst: u32, ctl: usize, name: &str, args: &[String]) -> Option<usize> {
        let anims = {
            let host = inner.host.try_borrow().ok()?;
            let tree = host.trees.get(&inst)?;
            let c = tree.ctls.get(ctl)?;
            if matches!(c.kind, Kind::Control(cobolt_forms::model::ControlType::Animator)) {
                return None;
            }
            c.anims.clone()
        };
        let t = now();
        self.motion_inst(inst, |mi| match name {
            "playanimation" | "play" => {
                let which = args.first().map(String::as_str).unwrap_or("");
                let defs = m::defs_for_play(&anims, if args.is_empty() { "1" } else { which });
                if defs.is_empty() {
                    return None;
                }
                m::start(mi.plays.entry(ctl).or_default(), defs, t);
                Some(ctl)
            }
            "stopanimation" => mi.plays.remove(&ctl).map(|_| ctl),
            "pause" => {
                let plays = mi.plays.get_mut(&ctl)?;
                for p in plays.iter_mut() {
                    p.paused_ms.get_or_insert(t);
                }
                Some(ctl)
            }
            _ => None,
        })
        .flatten()
    }

    /// The plans to make again after these controls' animations changed: for each, its topmost container (itself
    /// included) that has an animation, so a container and the controls inside it are planned together.
    pub(super) fn motion_roots(&self, inner: &Rc<Inner>, inst: u32, ctls: &[usize]) -> Option<BTreeSet<usize>> {
        let parents: Vec<Option<usize>> = {
            let host = inner.host.try_borrow().ok()?;
            let tree = host.trees.get(&inst)?;
            tree.ctls.iter().enumerate().map(|(i, c)| c.parent.filter(|_| i != 0)).collect()
        };
        let st = self.motion.try_borrow().ok()?;
        let mi = st.insts.get(&inst);
        let animated = |i: usize| mi.is_some_and(|mi| mi.plays.get(&i).is_some_and(|p| !p.is_empty()) || mi.running.contains_key(&i));
        let mut roots = BTreeSet::new();
        for &c in ctls {
            let mut root = c;
            let mut at = parents.get(c).copied().flatten();
            while let Some(p) = at {
                if p == 0 {
                    break;
                }
                if animated(p) {
                    root = p;
                }
                at = parents.get(p).copied().flatten();
            }
            roots.insert(root);
        }
        Some(roots)
    }

    // ── a plan: the Web Animations of a container and everything inside it ──

    /// Plan control `root` and everything inside it: cancel what the page was playing there, measure the controls as
    /// laid out, and start, on each control that moves (or whose container moves), the animation that draws it where the
    /// desktop draws it from now until every animation of the plan has come to rest.
    pub(super) fn motion_plan(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, root: usize) {
        let snap = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            snapshot(tree)
        };
        // The controls of the plan: the root and everything inside it.
        let mut nodes = vec![root];
        let mut k = 0;
        while k < nodes.len() {
            nodes.extend(snap.children.get(nodes[k]).cloned().unwrap_or_default());
            k += 1;
        }
        let els: BTreeMap<usize, Element> = {
            let Ok(st) = self.state.try_borrow() else { return };
            let Some(iv) = st.instances.get(&inst) else { return };
            nodes.iter().filter_map(|&i| iv.ctls.get(i).and_then(|c| c.as_ref()).map(|c| (i, c.rec.el.clone()))).collect()
        };
        // Cancel first: what is measured next is the layout, not a frame of an animation.
        let (plays, old_lift) = {
            let Ok(mut st) = self.motion.try_borrow_mut() else { return };
            let mi = st.insts.entry(inst).or_default();
            for i in &nodes {
                if let Some(a) = mi.running.remove(i) {
                    cancel(a);
                }
            }
            mi.renew.remove(&root);
            mi.pending.remove(&root);
            let plays: BTreeMap<usize, Vec<Playback>> = nodes.iter().filter_map(|i| mi.plays.get(i).map(|p| (*i, p.clone()))).filter(|(_, p)| !p.is_empty()).collect();
            (plays, mi.lifted.remove(&root))
        };
        if let Some(a) = old_lift {
            cancel(a);
        }
        if plays.is_empty() {
            return;
        }
        let reduce = reduced();
        // The frames start at the instant the newest animation of the plan started, when that was in this same pass (the
        // usual case: the program's `PlayAnimation` a moment ago), so the first frame is its first; else now.
        let t_now = now();
        let newest = plays.values().flatten().map(|p| p.started_ms).fold(f64::NEG_INFINITY, f64::max);
        let t0 = if t_now - newest < 100.0 { newest.min(t_now) } else { t_now };
        // How far the frames go: until every animation of the plan rests (reduced motion: nothing moves at all).
        let ends: Vec<Option<f64>> = plays.values().flatten().map(|p| p.end()).collect();
        let finite_end = ends.iter().flatten().fold(t0, |a, b| a.max(*b));
        let endless = !reduce && ends.iter().any(Option::is_none);
        let t1 = if reduce { t0 } else if endless { finite_end.max(t0 + 10_000.0) } else { finite_end };
        let at = |i: usize, t: f64| -> Tf {
            match plays.get(&i) {
                Some(p) if reduce => {
                    // Where it would rest: past every end (an endless one rests where it is now).
                    let rest = p.iter().map(|x| x.end().unwrap_or(t0)).fold(t0, f64::max) + 1.0;
                    m::transform_at(p, snap.form.0, snap.form.1, rest)
                }
                Some(p) => m::transform_at(p, snap.form.0, snap.form.1, t),
                None => Tf::IDENTITY,
            }
        };
        // Centres as laid out (no animation is running on these elements now), in one frame of reference.
        // Centre and half size as laid out (no animation is running on these elements now), in one frame of reference.
        let centre = |el: &Element| -> Option<((f64, f64), (f64, f64))> {
            if !displayed(el) {
                return None;
            }
            let r = el.get_bounding_client_rect();
            Some(((r.x() + r.width() / 2.0, r.y() + r.height() / 2.0), (r.width() / 2.0, r.height() / 2.0)))
        };
        let centres: BTreeMap<usize, Option<((f64, f64), (f64, f64))>> = els.iter().map(|(i, el)| (*i, centre(el))).collect();
        // Frames 60 a second, the desktop's paint rate; a control inside a container that moves too gets more where the
        // motion is fast (`motion::refine`: within a quarter pixel of the desktop's place between frames).
        let coarse = if t1 > t0 { m::sample_times(t0, t1, m::STEP_MS) } else { vec![t0] };
        // What the browser shows for a control that has no moving container: its frames, joined by straight lines.
        let shown = |i: usize, t: f64| -> Tf { m::lerp_tf(&coarse, &|u| at(i, u), t) };
        let mut started: BTreeMap<usize, Vec<JsValue>> = BTreeMap::new();
        let mut pending: BTreeSet<usize> = BTreeSet::new();
        let mut grows: BTreeSet<usize> = BTreeSet::new();
        for &i in &nodes {
            let Some(el) = els.get(&i) else { continue };
            // The container a control's element is inside, when it is part of this plan.
            let p = snap.parent[i].filter(|p| nodes.contains(p) && i != root);
            let own_moves = plays.contains_key(&i);
            let parent_moves = p.is_some_and(|p| plays.contains_key(&p));
            if !own_moves && !parent_moves {
                continue;
            }
            let mut times: &Vec<f64> = &coarse;
            let refined: Vec<f64>;
            let locals: Vec<Local> = if parent_moves {
                let (Some(Some((cc, half))), Some(Some((pc, _)))) = (centres.get(&i), p.and_then(|p| centres.get(&p))) else {
                    // Not on screen: nothing to measure, and nothing to see. Planned again once it is shown.
                    pending.insert(i);
                    continue;
                };
                let p = p.expect("parent_moves");
                // Undo what the container is SHOWN doing (its own frames, interpolated), not its exact curve, so the control
                // lands on the desktop's place at each of its frames. A container inside another moving one is taken at its
                // exact curve.
                let grand_moves = snap.parent[p].is_some_and(|g| nodes.contains(&g) && p != root && plays.contains_key(&g));
                let par = |t: f64| if grand_moves { at(p, t) } else { shown(p, t) };
                refined = if reduce { coarse.clone() } else { m::refine(&coarse, &par, &|t| at(i, t), *pc, *cc, *half, 0.25) };
                times = &refined;
                times.iter().map(|&t| m::local_transform(par(t), *pc, at(i, t), *cc)).collect()
            } else {
                times.iter().map(|&t| { let o = at(i, t); Local { tx: o.dx, ty: o.dy, k: o.scale, opacity: o.alpha } }).collect()
            };
            if own_moves && times.iter().any(|&t| at(i, t).scale > 1.001) {
                grows.insert(i);
            }
            if locals.iter().all(Local::is_identity) {
                continue;
            }
            let last = *locals.last().expect("one frame at least");
            // Backwards too: the frames start at the instant the animation was asked for, which a browser's frame clock
            // (behind the page's own clock inside a task) has not reached yet; the first frame is held until it has, rather
            // than the control flashing its designed state for one frame.
            let fill = if last.is_identity() { "backwards" } else { "both" };
            let span = (t1 - t0).max(1.0);
            let n = locals.len();
            let offset = |k: usize| if n <= 1 { 1.0 } else { ((times[k] - t0) / span).clamp(0.0, 1.0) };
            let mut anims = Vec::new();
            let tf = Array::new();
            for (k, l) in locals.iter().enumerate() {
                let mut props: Vec<(&str, JsValue)> = vec![("transform", l.css().into())];
                // A container that moves does not clip what is inside it to where it is drawn now: the desktop clips to its
                // rectangle as laid out, which its contents, drawn there, do not leave unless they grow.
                if snap.container[i] && !snap.scroller[i] {
                    props.push(("overflow", "visible".into()));
                }
                tf.push(&frame(offset(k), &props));
            }
            if n == 1 {
                // One frame: a resting transform held from now on.
                tf.push(&frame(0.0, &[("transform", last.css().into())]));
                tf.reverse();
            }
            if let Some(a) = animate(el, &tf, span, fill, "add", t0) {
                anims.push(a);
            }
            if locals.iter().any(|l| (l.opacity - 1.0).abs() > 1e-4) {
                let of = Array::new();
                for (k, l) in locals.iter().enumerate() {
                    of.push(&frame(offset(k), &[("opacity", ((l.opacity * 1e4).round() / 1e4).into())]));
                }
                if n == 1 {
                    of.push(&frame(0.0, &[("opacity", last.opacity.into())]));
                    of.reverse();
                }
                if let Some(a) = animate(el, &of, span, fill, "replace", t0) {
                    anims.push(a);
                }
            }
            started.insert(i, anims);
        }
        // A control that grows past its place bleeds out of the containers around it (render.rs `grows_past_its_place`):
        // their clipping is lifted for the time the plan runs. A container that scrolls keeps its clipping (lifting it
        // would lose the scroll position); the desktop lets a growing control out of that one too.
        let mut lifted = Vec::new();
        if !grows.is_empty() && t1 > t0 {
            let mut lift: BTreeSet<usize> = BTreeSet::new();
            for &g in &grows {
                let mut at_p = snap.parent[g];
                while let Some(p) = at_p {
                    if p == 0 {
                        break;
                    }
                    if snap.container[p] && !snap.scroller[p] && !(nodes.contains(&p) && started.contains_key(&p)) {
                        lift.insert(p);
                    }
                    at_p = snap.parent[p];
                }
            }
            let outside: Vec<Element> = {
                let Ok(st) = self.state.try_borrow() else { return };
                let Some(iv) = st.instances.get(&inst) else { return };
                lift.iter().filter_map(|&i| iv.ctls.get(i).and_then(|c| c.as_ref()).map(|c| c.rec.el.clone())).collect()
            };
            for el in outside {
                let f = Array::new();
                f.push(&frame(0.0, &[("overflow", "visible".into())]));
                f.push(&frame(1.0, &[("overflow", "visible".into())]));
                if let Some(a) = animate(&el, &f, t1 - t0, "none", "replace", t0) {
                    lifted.push(a);
                }
            }
        }
        // An animation that never ends is planned again before its frames run out.
        let renew = endless.then(|| {
            let weak = Rc::downgrade(self);
            let wi = Rc::downgrade(inner);
            crate::api::Timeout::new(((t1 - t0) - 500.0).max(100.0) as u32, move || {
                if let (Some(sh), Some(inner)) = (weak.upgrade(), wi.upgrade()) {
                    sh.motion_plan(&inner, inst, root);
                }
            })
        });
        if let Ok(mut st) = self.motion.try_borrow_mut() {
            let mi = st.insts.entry(inst).or_default();
            for (i, a) in started {
                mi.running.insert(i, a);
            }
            if !lifted.is_empty() {
                mi.lifted.insert(root, lifted);
            }
            if !pending.is_empty() && !reduce {
                mi.pending.insert(root, pending);
            }
            if let Some(r) = renew {
                mi.renew.insert(root, r);
            }
            // What has come to rest as designed needs no state: its playbacks are forgotten once their plan is over.
        }
    }

    // ── after every render pass ──────────────────────────────────────────────

    /// Run after the page took a render pass of instance `inst`: the program's animation verbs of the pass, a plan whose
    /// hidden control is now shown, and the charts' motion.
    pub(super) fn motion_after_render(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, verbs: Vec<(usize, String, Vec<String>)>) {
        let mut changed = Vec::new();
        for (ctl, name, args) in verbs {
            if let Some(c) = self.motion_command(inner, inst, ctl, &name, &args) {
                changed.push(c);
            }
        }
        let mut roots = if changed.is_empty() { BTreeSet::new() } else { self.motion_roots(inner, inst, &changed).unwrap_or_default() };
        // A plan with a control that was hidden when it was made, and is shown now.
        let waiting: Vec<(usize, Vec<usize>)> = self
            .motion
            .try_borrow()
            .ok()
            .and_then(|st| st.insts.get(&inst).map(|mi| mi.pending.iter().map(|(r, s)| (*r, s.iter().copied().collect())).collect()))
            .unwrap_or_default();
        if !waiting.is_empty() {
            if let Ok(st) = self.state.try_borrow() {
                if let Some(iv) = st.instances.get(&inst) {
                    for (r, ctls) in waiting {
                        if ctls.iter().any(|&c| iv.ctls.get(c).and_then(|v| v.as_ref()).is_some_and(|v| displayed(&v.rec.el))) {
                            roots.insert(r);
                        }
                    }
                }
            }
        }
        for r in roots {
            self.motion_plan(inner, inst, r);
        }
        self.chart_pass(inner, inst);
    }

    // ── charts ───────────────────────────────────────────────────────────────

    /// Move every chart of `inst` that is on screen to this moment of its motion, and draw again the ones whose frame
    /// changed. Asks for another animation frame while one still moves.
    fn chart_pass(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32) {
        use cobolt_forms::model::ControlType as CT;
        // The charts, with what decides their motion.
        struct Ch {
            i: usize,
            raw: String,
            grows: bool,
            values: bool,
            ms: f64,
        }
        let charts: Vec<Ch> = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            tree.ctls
                .iter()
                .enumerate()
                .filter(|(_, c)| matches!(c.control_type(), Some(CT::BarChart | CT::LineChart | CT::PieChart | CT::DonutChart | CT::RadarChart)))
                .filter(|(_, c)| !c.flag("Monochrome"))
                .map(|(i, c)| Ch {
                    i,
                    raw: c.raw("__ChartData").unwrap_or("").to_owned(),
                    // The painter's `prop_bool(ctrl, name, default)`.
                    grows: c.raw("AnimateOnLoad").map(crate::value::truthy).unwrap_or(true),
                    values: c.raw("AnimateValues").map(crate::value::truthy).unwrap_or(false),
                    ms: m::chart_ms(c.raw("AnimationDuration")),
                })
                .collect()
        };
        if charts.is_empty() {
            return;
        }
        let shown: BTreeSet<usize> = {
            let Ok(st) = self.state.try_borrow() else { return };
            let Some(iv) = st.instances.get(&inst) else { return };
            charts.iter().filter(|c| iv.ctls.get(c.i).and_then(|v| v.as_ref()).is_some_and(|v| displayed(&v.rec.el))).map(|c| c.i).collect()
        };
        let t = now();
        let reduce = reduced();
        let mut frames: Vec<(usize, Option<ChartFrame>)> = Vec::new();
        let mut moving = false;
        {
            let Ok(mut st) = self.motion.try_borrow_mut() else { return };
            let mi = st.insts.entry(inst).or_default();
            for c in charts.iter().filter(|c| shown.contains(&c.i)) {
                let cm = mi.charts.entry(c.i).or_default();
                let f = cm.frame(t, &c.raw, c.grows && !reduce, c.values && !reduce, c.ms);
                moving |= cm.moving(t, c.ms) && (f.grow.is_some() || f.data.is_some());
                frames.push((c.i, (f.grow.is_some() || f.data.is_some()).then_some(f)));
            }
        }
        // Draw again the charts whose frame is not the one drawn.
        let redraw: Vec<usize> = {
            let Ok(mut st) = self.state.try_borrow_mut() else { return };
            let Some(iv) = st.instances.get_mut(&inst) else { return };
            let mut out = Vec::new();
            for (i, f) in frames {
                let drawn = iv.ui.chart_frames.get(&i).cloned();
                if drawn != f {
                    match f {
                        Some(f) => iv.ui.chart_frames.insert(i, f),
                        None => iv.ui.chart_frames.remove(&i),
                    };
                    out.push(i);
                }
            }
            out
        };
        if !redraw.is_empty() {
            self.redraw_views(inner, inst, &redraw);
        }
        if moving {
            self.motion_frame_wanted();
        }
    }

    /// Ask for one animation frame, in which every instance's charts move on.
    fn motion_frame_wanted(self: &Rc<Self>) {
        let Ok(mut st) = self.motion.try_borrow_mut() else { return };
        if st.raf.is_some() {
            return;
        }
        let weak = Rc::downgrade(self);
        st.raf = Raf::new(move || {
            let Some(sh) = weak.upgrade() else { return };
            if let Ok(mut st) = sh.motion.try_borrow_mut() {
                // This frame's request is spent: kept until the next one replaces it, so the closure running now is not
                // dropped from inside itself.
                st.spent = st.raf.take();
            }
            let Some(inner) = sh.inner.upgrade() else { return };
            let insts: Vec<u32> = sh.motion.try_borrow().map(|st| st.insts.keys().copied().collect()).unwrap_or_default();
            for i in insts {
                sh.chart_pass(&inner, i);
            }
        });
    }
}
