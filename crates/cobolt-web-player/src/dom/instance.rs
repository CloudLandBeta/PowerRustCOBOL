// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **One form instance on the page** (WASM-016 R6, WASM-011): its element tree, its stylesheet, its
//! timers and its way of being shown (the application window, the shell's ContentPane, a modal dialog,
//! a modeless panel).

use std::collections::BTreeSet;
use std::rc::Rc;

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::Element;

use super::nodes::{self, Dispatch, NodeRec};
use super::h::{self, Listener};
use super::sheets::Sheet;
use super::{InstChanges, Route, Shared};
use crate::api::{Inbox, Inner, Timeout};
use crate::host::OpenMode;
use crate::tree::Kind;
use crate::value::truthy;
use crate::view::interact::Act;
use crate::view::shell::{self, Crumb};
use crate::view::{controls, style, Slot, UiState, VNode, ViewCtx};

pub(crate) struct CtlView {
    pub rec: NodeRec,
    pub slots: Vec<(Slot, Element)>,
    pub dispatch: Dispatch,
}

pub(crate) struct ResizeObs {
    obs: web_sys::ResizeObserver,
    _cb: Closure<dyn FnMut(js_sys::Array)>,
}

impl Drop for ResizeObs {
    fn drop(&mut self) {
        self.obs.disconnect();
    }
}

pub(crate) struct InstanceView {
    pub form: String,
    pub mode: OpenMode,
    pub root: Element,
    pub surface_el: Element,
    /// The title bar and close button of a modal or panel instance.
    pub frame: Option<NodeRec>,
    pub ctls: Vec<Option<CtlView>>,
    pub mount: Vec<(usize, Slot)>,
    pub ui: UiState,
    pub sheet: Sheet,
    /// The property-layer rules, one entry per control (0 is the form).
    pub css: Vec<String>,
    pub bp: Option<String>,
    pub width: f64,
    /// Held for what dropping it does: the size observer stops and the form's key listener is removed with the instance.
    pub _observer: Option<ResizeObs>,
    /// Watches the boxes of the controls that are drawn to their own size (`size_aware`): a chart, a gauge.
    /// What it reports goes to `UiState::box_sizes`, and the control is drawn again for it.
    pub box_obs: Option<ResizeObs>,
    pub _keys: Vec<Listener>,
    pub drag: Option<(Listener, Listener)>,
    /// The `onResize` owed once the form's size stops changing (`Host::surface_settled`).
    pub settle: Option<crate::api::Timeout>,
}

impl InstanceView {
    pub fn dispose(self) {
        if let Some(f) = &self.frame {
            f.el.remove();
        }
        self.root.remove();
        self.sheet.dispose();
    }
}

/// A control whose picture is made for the size of its box, and so is drawn again when layout changes it.
fn size_aware(c: &crate::tree::Ctl) -> bool {
    use cobolt_forms::model::ControlType as T;
    matches!(c.control_type(), Some(T::BarChart | T::LineChart | T::PieChart | T::AreaChart | T::ScatterChart | T::DonutChart | T::RadarChart | T::Gauge))
}

/// A card: a Panel or a GroupBox, the controls that can be expanded (spec 090). Its box is measured too, for the card that is
/// expanded: what is inside it grows by a factor taken from that box (`zoom`).
fn is_card(c: &crate::tree::Ctl) -> bool {
    use cobolt_forms::model::ControlType as T;
    matches!(c.control_type(), Some(T::Panel | T::GroupBox))
}

fn slot_target(ctls: &[Option<CtlView>], surface: &Element, parent: usize, slot: Slot) -> Element {
    if parent == 0 {
        return surface.clone();
    }
    match ctls.get(parent).and_then(|c| c.as_ref()) {
        Some(pv) => pv.slots.iter().find(|(s, _)| *s == slot).map(|(_, e)| e.clone()).unwrap_or_else(|| pv.rec.el.clone()),
        None => surface.clone(),
    }
}

/// Make sure control `i`'s element is a child of its slot (without disturbing it when it already is).
fn mount_one(ctls: &[Option<CtlView>], mount: &[(usize, Slot)], surface: &Element, i: usize) {
    let Some(cv) = ctls[i].as_ref() else { return };
    let (p, slot) = mount[i];
    let target = slot_target(ctls, surface, p, slot);
    if cv.rec.el.parent_element().as_ref() != Some(&target) {
        let _ = target.append_child(&cv.rec.el);
    }
}

impl Shared {
    // ── opening ─────────────────────────────────────────────────────────────

    pub(super) fn open_instance(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, mode: OpenMode) {
        struct Built {
            vnodes: Vec<Option<VNode>>,
            mount: Vec<(usize, Slot)>,
            order: Vec<usize>,
            css: Vec<String>,
            form: String,
            title: String,
            size: (f64, f64),
            side_menu: Option<usize>,
            opener: Option<u32>,
            responsive: bool,
            size_aware: Vec<usize>,
        }
        let built = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            let Ok(styles) = inner.styles.try_borrow() else { return };
            let ui = UiState::default();
            let tile = self.tile_url(inner);
            let cx = ViewCtx {
                tree,
                assets: &styles.assets,
                locale: &styles.locale,
                ui: &ui,
                menus: &styles.menus,
                image_origins: &inner.cfg.image_origins,
                tile_url: &tile,
            };
            let ctls = tree.effective_ctls(None);
            let responsive = truthy(&tree.get(0, "Responsive"));
            let n = ctls.len();
            let mut vnodes: Vec<Option<VNode>> = vec![None; n];
            let mut css = vec![String::new(); n];
            css[0] = style::form_css(inst, &ctls, responsive, &styles.assets);
            for i in 1..n {
                if style::has_a_box(&ctls[i]) {
                    vnodes[i] = Some(controls::view(&cx, i));
                    css[i] = style::control_css(inst, &ctls, i, responsive);
                }
            }
            let mount: Vec<(usize, Slot)> = (0..n).map(|i| if i == 0 { (0, Slot::Children) } else { style::mount_of(&ctls, i) }).collect();
            let side_menu = ctls.iter().position(|c| matches!(&c.kind, Kind::Control(cobolt_forms::model::ControlType::SideMenu)));
            Built {
                vnodes,
                mount,
                order: style::mount_order(&ctls),
                css,
                form: tree.form_name.clone(),
                title: tree.get(0, "Title"),
                size: tree.design_size.into_f64(),
                side_menu,
                opener: tree.opener,
                responsive,
                size_aware: (1..n).filter(|&i| style::has_a_box(&ctls[i]) && (size_aware(&ctls[i]) || is_card(&ctls[i]))).collect(),
            }
        };
        let doc = &self.doc;
        let root = h::el(doc, "section", &["pr-form"]);
        h::attr(&root, "data-pr-form", &built.form);
        h::attr(&root, "data-pr-inst", &inst.to_string());
        h::attr(&root, "aria-label", &built.title);
        if built.responsive {
            h::attr(&root, "data-responsive", "");
        }
        let surface_el = h::el(doc, "div", &["pr-surface"]);
        h::append(&root, &surface_el);

        let mut ctls: Vec<Option<CtlView>> = (0..built.vnodes.len()).map(|_| None).collect();
        for &i in &built.order {
            if let Some(v) = &built.vnodes[i] {
                let dispatch = self.dispatch(Route::Ctl { inst, ctl: i });
                let rec = nodes::create(doc, v, None, &dispatch);
                let slots = nodes::slots(&rec);
                ctls[i] = Some(CtlView { rec, slots, dispatch });
            }
        }
        for &i in &built.order {
            mount_one(&ctls, &built.mount, &surface_el, i);
        }

        let sheet = self.scope.new_sheet();
        sheet.set(&style::wrap_layer(&built.css));

        // Keys that reach the form's root (Enter presses the default button, the form's own key events).
        let mut keys = Vec::new();
        {
            let d = self.dispatch(Route::Form { inst });
            let root_c = root.clone();
            keys.push(Listener::new_active(&root, "keydown", move |ev| d("keydown", "root", &ev, &root_c)));
        }

        // The size of the form's own surface, for the breakpoints (spec 056 R61).
        let observer = {
            let weak = Rc::downgrade(self);
            let cb = Closure::<dyn FnMut(js_sys::Array)>::new(move |entries: js_sys::Array| {
                let first = entries.get(0);
                // The surface's whole box, scroll bars included: the desktop's window size, which its breakpoint and
                // `me::Width`/`me::Height` are taken from (a page scroll bar is chrome over the form, as the desktop's
                // floating ones are). The content box is the fallback where a browser gives no border box.
                let border = Reflect::get(&first, &"borderBoxSize".into()).ok().map(|a| js_sys::Array::from(&a).get(0)).filter(|b| !b.is_undefined());
                let (w, h) = match border {
                    Some(b) => (
                        Reflect::get(&b, &"inlineSize".into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0),
                        Reflect::get(&b, &"blockSize".into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0),
                    ),
                    None => {
                        let rect = Reflect::get(&first, &"contentRect".into()).unwrap_or(JsValue::UNDEFINED);
                        (
                            Reflect::get(&rect, &"width".into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0),
                            Reflect::get(&rect, &"height".into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0),
                        )
                    }
                };
                if let Some(sh) = weak.upgrade() {
                    sh.on_resize(inst, w, h);
                }
            });
            web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref()).ok().map(|obs| {
                obs.observe(&root);
                ResizeObs { obs, _cb: cb }
            })
        };

        // The boxes of the controls drawn to their own size, as the browser lays them out (the desktop's
        // solver has given a chart in a grid its cell before its painter draws it).
        let box_obs = {
            let weak = Rc::downgrade(self);
            let cb = Closure::<dyn FnMut(js_sys::Array)>::new(move |entries: js_sys::Array| {
                for e in entries.iter() {
                    let entry: web_sys::ResizeObserverEntry = e.unchecked_into();
                    let el = entry.target();
                    let name = el.get_attribute("data-name").unwrap_or_default();
                    // The LAYOUT box (the entry's border box), never `getBoundingClientRect`: that one includes the transforms of the
                    // element and of every ancestor, so a card caught in the middle of its ZoomOut entrance (scale 0.93) would hand its
                    // controls a box 7 % too small, and nothing corrects it afterwards (an observer does not fire when only a
                    // transform ends).
                    let border = Reflect::get(&entry, &"borderBoxSize".into()).ok().map(|a| js_sys::Array::from(&a).get(0)).filter(|b| !b.is_undefined());
                    let (w, h) = match border {
                        Some(b) => (
                            Reflect::get(&b, &"inlineSize".into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0),
                            Reflect::get(&b, &"blockSize".into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0),
                        ),
                        None => match el.dyn_ref::<web_sys::HtmlElement>() {
                            Some(he) => (he.offset_width() as f64, he.offset_height() as f64),
                            None => {
                                let r = el.get_bounding_client_rect();
                                (r.width(), r.height())
                            }
                        },
                    };
                    if let Some(sh) = weak.upgrade() {
                        sh.on_box(inst, &name, w, h);
                    }
                }
            });
            web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref()).ok().map(|obs| ResizeObs { obs, _cb: cb })
        };
        if let Some(o) = &box_obs {
            for &i in &built.size_aware {
                if let Some(cv) = ctls[i].as_ref() {
                    o.obs.observe(&cv.rec.el);
                }
            }
        }

        let mut iv = InstanceView {
            form: built.form.clone(),
            mode,
            root: root.clone(),
            surface_el,
            frame: None,
            ctls,
            mount: built.mount,
            ui: UiState::default(),
            sheet,
            css: built.css,
            bp: None,
            width: 0.0,
            _observer: observer,
            box_obs,
            _keys: keys,
            drag: None,
            settle: None,
        };

        // Place it.
        match mode {
            OpenMode::Root => {
                h::append(&self.chrome.main, &root);
                if built.side_menu.is_some() {
                    self.enter_shell(inst, &built.title);
                } else if let Ok(mut st) = self.state.try_borrow_mut() {
                    st.chain = vec![Crumb { inst, label: built.title.clone() }];
                }
            }
            OpenMode::Embedded => {
                self.place_embedded(inner, inst, built.opener, &built.title, &root);
            }
            OpenMode::Modal => {
                // A form window opens at its designed size, responsive or not, as the desktop's child window
                // does (form-host `with_inner_size(child.size)`): a responsive form fills its frame, so a frame
                // left to its contents had a body of no height — a title chip whose controls could not be reached.
                let frame = shell::modal_frame(&built.title, true, Some(built.size));
                let d = self.dispatch(Route::Frame(inst));
                let rec = nodes::create(doc, &frame, None, &d);
                if let Some(body) = nodes::find_key(&rec, "body") {
                    h::append(&body, &root);
                }
                h::append(&self.root, &rec.el);
                let _ = Reflect::get(&rec.el, &"showModal".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()).map(|f| f.call0(&rec.el));
                iv.frame = Some(rec);
            }
            OpenMode::Panel | OpenMode::Window => {
                let n = self.state.try_borrow().map(|s| s.panels).unwrap_or(0);
                let offset = (24.0 + (n % 8) as f64 * 28.0, 24.0 + (n % 8) as f64 * 28.0);
                // At its designed size, as a modal window is (above).
                let frame = shell::panel_frame(&built.title, Some(built.size), offset);
                let d = self.dispatch(Route::Frame(inst));
                let rec = nodes::create(doc, &frame, None, &d);
                if let Some(body) = nodes::find_key(&rec, "body") {
                    h::append(&body, &root);
                }
                h::append(&self.chrome.layer, &rec.el);
                iv.frame = Some(rec);
                if let Ok(mut st) = self.state.try_borrow_mut() {
                    st.panels += 1;
                }
            }
        }
        if let Ok(mut st) = self.state.try_borrow_mut() {
            st.instances.insert(inst, iv);
        }
        if mode == OpenMode::Embedded {
            // The chain was extended before the instance was recorded: show only the top form and put its
            // route in the address now that the page knows the form's name (WASM-011 R4).
            self.sync_shell(inner);
        }
        self.adopt();
        self.sync_timers(inner, inst);
        self.ensure_viewer_docs(inner, inst);
        self.ensure_mermaid(inner, inst);
        self.sync_find(inner, inst);
        if let (OpenMode::Root, Some(menu)) = (mode, built.side_menu) {
            // The rail starts as the user left it (spec 049 R9), else as the control is designed; the
            // address may name a form to show (a deep link).
            let designed = inner.host.try_borrow().ok().and_then(|h| h.trees.get(&inst).map(|t| truthy(&t.get(menu, "Collapsed")))).unwrap_or(false);
            let collapsed = self.saved_sidebar().unwrap_or(designed);
            self.with_ui(inst, |ui| ui.sidebar_collapsed = Some(collapsed));
            self.update_rail(inner, inst, menu, collapsed);
            self.mark_dirty(inst, menu);
            let sh = self.clone();
            inner.push(Inbox::Raw(Box::new(move |inner| sh.follow_address(inner))));
        }
        // The start-up handler may already have written to the new instance: draw those writes too.
        self.mark_dirty(inst, 0);
        // Its animations' triggers, and the ones that start with the form (P4).
        self.motion_open(inner, inst, &root);
    }

    pub(super) fn close_instance(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32) {
        self.motion_close(inst);
        self.clear_find(inst);
        self.clear_canvas(inst);
        let (iv, shell_changed) = {
            let Ok(mut st) = self.state.try_borrow_mut() else { return };
            st.timers.retain(|(i, _), _| *i != inst);
            st.files.retain(|(i, _), _| *i != inst);
            st.dirty.retain(|(i, _)| *i != inst);
            st.toasts.retain(|t| t.inst != inst);
            let before = st.chain.len();
            st.chain.retain(|c| c.inst != inst);
            if st.content_inst == Some(inst) {
                st.content_inst = None;
            }
            (st.instances.remove(&inst), st.chain.len() != before)
        };
        if let Some(iv) = iv {
            if matches!(iv.mode, OpenMode::Panel | OpenMode::Window) {
                if let Ok(mut st) = self.state.try_borrow_mut() {
                    st.panels = st.panels.saturating_sub(1);
                }
            }
            if iv.mode == OpenMode::Modal {
                if let Some(f) = &iv.frame {
                    let _ = Reflect::get(&f.el, &"close".into()).ok().and_then(|c| c.dyn_into::<js_sys::Function>().ok()).map(|c| c.call0(&f.el));
                }
            }
            iv.dispose();
        }
        self.adopt();
        if shell_changed {
            self.sync_shell(inner);
        }
        // The last form of the page closed and none is being opened: the application has ended, as the desktop's does
        // when its main window closes. (The IDE's preview closes every form to take a new module in place: it opens
        // them again at once, and an open takes the notice away.)
        let none_left = self.state.try_borrow().map(|st| st.instances.is_empty()).unwrap_or(false)
            && inner.host.try_borrow().map(|h| h.trees.is_empty() && !h.is_opening()).unwrap_or(false);
        if none_left {
            self.show_ended();
        }
    }

    // ── rendering ───────────────────────────────────────────────────────────

    pub(super) fn render_instance(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, mut ch: InstChanges) {
        let css_all = ch.dirty.remove(&usize::MAX);
        // Phase 0: the commands that change page state the views read.
        let mut deferred: Vec<(usize, String, Vec<String>)> = Vec::new();
        // The animation verbs act once the pass has put the page right (`motion_after_render`).
        let mut verbs: Vec<(usize, String, Vec<String>)> = Vec::new();
        for (ctl, name, args) in std::mem::take(&mut ch.commands) {
            match name.as_str() {
                "playanimation" | "play" | "stopanimation" | "pause" => verbs.push((ctl, name, args)),
                "find-next" | "find-prev" => self.find_step(inner, inst, ctl, name == "find-next"),
                "dismiss-all" => {
                    let ids: Vec<u32> = self.state.try_borrow().map(|st| st.toasts.iter().filter(|t| t.inst == inst && t.ctl == ctl).map(|t| t.id).collect()).unwrap_or_default();
                    for id in ids {
                        self.dismiss_toast(id);
                    }
                }
                _ => deferred.push((ctl, name, args)),
            }
        }

        // Phase A: read the tree and compute what the page should become (no DOM is touched).
        struct Plan {
            vnodes: Vec<(usize, VNode)>,
            css: Vec<(usize, String)>,
            title: String,
        }
        let plan = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            let Ok(st) = self.state.try_borrow() else { return };
            let Some(iv) = st.instances.get(&inst) else { return };
            let Ok(styles) = inner.styles.try_borrow() else { return };
            let tile = self.tile_url(inner);
            let cx = ViewCtx {
                tree,
                assets: &styles.assets,
                locale: &styles.locale,
                ui: &iv.ui,
                menus: &styles.menus,
                image_origins: &inner.cfg.image_origins,
                tile_url: &tile,
            };
            let bp = iv.bp.as_ref().and_then(|n| tree.breakpoints.iter().find(|b| &b.name == n));
            let ctls = tree.effective_ctls(bp);
            let n = ctls.len();
            let responsive = truthy(&tree.get(0, "Responsive"));
            let mut css_dirty: BTreeSet<usize> = BTreeSet::new();
            if css_all {
                css_dirty.extend(0..n);
            }
            // The controls whose elements are drawn again: what changed, and what is inside a card that is (or can be) expanded, whose
            // lettering (a chart's, a list's rows) grows with the card (spec 090) and goes back when it is collapsed.
            let mut redraw: BTreeSet<usize> = ch.dirty.iter().copied().collect();
            for &i in &ch.dirty {
                if i >= n {
                    continue;
                }
                css_dirty.insert(i);
                // A card that is (or can be) expanded decides how its siblings are drawn: they follow it.
                let card = ctls[i].flag("expandable") || ctls[i].flag("expanded") || ctls[i].raw("expanded").is_some();
                if card {
                    if let Some(p) = ctls[i].parent {
                        css_dirty.extend(ctls[p].children.iter().copied());
                    }
                }
                if ctls[i].is_container() {
                    // A container's layout decides its children's rules.
                    let mut stack = ctls[i].children.clone();
                    while let Some(c) = stack.pop() {
                        css_dirty.insert(c);
                        if card {
                            redraw.insert(c);
                        }
                        stack.extend(ctls[c].children.iter().copied());
                    }
                }
            }
            let mut vnodes = Vec::new();
            for &i in &redraw {
                if i >= 1 && i < n && iv.ctls.get(i).is_some_and(|c| c.is_some()) {
                    vnodes.push((i, controls::view(&cx, i)));
                }
            }
            if css_all {
                // A breakpoint changed what the controls look like, not what they hold: views are unchanged.
            }
            let css: Vec<(usize, String)> = css_dirty
                .into_iter()
                .map(|i| {
                    let text = if i == 0 {
                        style::form_css(inst, &ctls, responsive, &styles.assets)
                    } else if style::has_a_box(&ctls[i]) {
                        style::control_css(inst, &ctls, i, responsive)
                    } else {
                        String::new()
                    };
                    (i, text)
                })
                .collect();
            Plan { vnodes, css, title: tree.get(0, "Title") }
        };

        // Phase B: change the page.
        {
            let Ok(mut st) = self.state.try_borrow_mut() else { return };
            let Some(iv) = st.instances.get_mut(&inst) else { return };
            for (i, v) in &plan.vnodes {
                let i = *i;
                let replaced = {
                    let Some(cv) = iv.ctls[i].as_mut() else { continue };
                    let r = nodes::patch(&self.doc, &mut cv.rec, v, None, &cv.dispatch);
                    cv.slots = nodes::slots(&cv.rec);
                    // A root the patch replaced is a new element: the observer must be told of it.
                    if r {
                        if let Some(o) = &iv.box_obs {
                            o.obs.observe(&cv.rec.el);
                        }
                    }
                    r
                };
                if replaced {
                    let children: Vec<usize> = (1..iv.ctls.len()).filter(|&c| iv.mount[c].0 == i).collect();
                    for c in children {
                        mount_one(&iv.ctls, &iv.mount, &iv.surface_el, c);
                    }
                }
            }
            let mut changed = false;
            for (i, text) in plan.css {
                if iv.css.get(i).map(|t| *t != text).unwrap_or(true) {
                    if i < iv.css.len() {
                        iv.css[i] = text;
                    }
                    changed = true;
                }
            }
            if changed {
                iv.sheet.set(&style::wrap_layer(&iv.css));
            }
            if let Some(f) = &iv.frame {
                if let Some(t) = nodes::find_key(f, "title") {
                    if t.text_content().as_deref() != Some(plan.title.as_str()) {
                        t.set_text_content(Some(&plan.title));
                    }
                }
            }
        }

        // Phase C: the commands that act on the page.
        for (ctl, name, args) in deferred {
            self.run_command(inner, inst, ctl, &name, &args);
        }
        self.sync_timers(inner, inst);
        self.ensure_viewer_docs(inner, inst);
        self.ensure_mermaid(inner, inst);
        self.sync_find(inner, inst);
        // Phase D: what moves (P4).
        self.motion_after_render(inner, inst, verbs);
    }

    /// Draw these controls of `inst` again, now, from the tree and the page state (a chart's frame of motion).
    pub(super) fn redraw_views(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, which: &[usize]) {
        let vnodes: Vec<(usize, VNode)> = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            let Ok(st) = self.state.try_borrow() else { return };
            let Some(iv) = st.instances.get(&inst) else { return };
            let Ok(styles) = inner.styles.try_borrow() else { return };
            let tile = self.tile_url(inner);
            let cx = ViewCtx {
                tree,
                assets: &styles.assets,
                locale: &styles.locale,
                ui: &iv.ui,
                menus: &styles.menus,
                image_origins: &inner.cfg.image_origins,
                tile_url: &tile,
            };
            which.iter().filter(|&&i| i >= 1 && i < tree.ctls.len() && iv.ctls.get(i).is_some_and(|c| c.is_some())).map(|&i| (i, controls::view(&cx, i))).collect()
        };
        let Ok(mut st) = self.state.try_borrow_mut() else { return };
        let Some(iv) = st.instances.get_mut(&inst) else { return };
        for (i, v) in &vnodes {
            let i = *i;
            let replaced = {
                let Some(cv) = iv.ctls[i].as_mut() else { continue };
                let r = nodes::patch(&self.doc, &mut cv.rec, v, None, &cv.dispatch);
                cv.slots = nodes::slots(&cv.rec);
                if r {
                    if let Some(o) = &iv.box_obs {
                        o.obs.observe(&cv.rec.el);
                    }
                }
                r
            };
            if replaced {
                let children: Vec<usize> = (1..iv.ctls.len()).filter(|&c| iv.mount[c].0 == i).collect();
                for c in children {
                    mount_one(&iv.ctls, &iv.mount, &iv.surface_el, c);
                }
            }
        }
    }

    fn run_command(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, ctl: usize, name: &str, args: &[String]) {
        match name {
            "canvas-call" => self.canvas_call(inner, inst, ctl, args),
            "focus" | "setfocus" => {
                if let Some(el) = self.control_element(inst, ctl) {
                    let inner_focusable = el.query_selector("input,textarea,select,button,[tabindex]").ok().flatten();
                    h::focus(inner_focusable.as_ref().unwrap_or(&el));
                }
            }
            "select-all" => {
                if let Some(el) = self.control_element(inst, ctl) {
                    if let Ok(Some(f)) = el.query_selector("input,textarea") {
                        let _ = Reflect::get(&f, &"select".into()).ok().and_then(|s| s.dyn_into::<js_sys::Function>().ok()).map(|s| s.call0(&f));
                    }
                }
            }
            "show" => self.show_toast(inner, inst, ctl, args.first().map(String::as_str).filter(|s| !s.is_empty())),
            "jump-to-latest" => {
                if let Some(el) = self.control_element(inst, ctl) {
                    if let Ok(Some(c)) = el.query_selector(".pr-content") {
                        let _ = Reflect::set(&c, &"scrollTop".into(), &1.0e9.into());
                    }
                }
            }
            "refresh" => self.mark_dirty(inst, ctl),
            // A Hyperlink's `Open` / `OpenIn` (WASM-006 R12, C9) and its fragment address (R10).
            "open" | "open-in" => {
                if let Some(plan) = crate::hyperlink::plan_from_args(args) {
                    self.follow(inner, inst, ctl, &plan);
                }
            }
            "scroll-to" => {
                if let Some(name) = args.first() {
                    self.scroll_to(inner, inst, name);
                }
            }
            "setbreadcrumbdetail" | "clearbreadcrumbdetail" | "settitlevisible" | "setwindowstate" | "timer-start" | "timer-stop" => {}
            _ => {}
        }
    }

    /// The root element of a control of an instance.
    pub(crate) fn control_element(&self, inst: u32, ctl: usize) -> Option<Element> {
        let st = self.state.try_borrow().ok()?;
        let iv = st.instances.get(&inst)?;
        if ctl == 0 {
            return Some(iv.root.clone());
        }
        iv.ctls.get(ctl)?.as_ref().map(|c| c.rec.el.clone())
    }

    // ── the boxes of controls drawn to their own size ───────────────────────

    /// The browser gave control `name` of instance `inst` a box of `w` × `h`: when that is not what the
    /// control was last drawn for, record it and draw the control again.
    pub(super) fn on_box(self: &Rc<Self>, inst: u32, name: &str, w: f64, h: f64) {
        if w < 1.0 || h < 1.0 {
            return;
        }
        let Some(inner) = self.inner.upgrade() else { return };
        let (ctl, designed, card) = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            let Some(i) = tree.by_name(name) else { return };
            let dim = |k: &str| crate::value::parse_f64(&tree.get(i, k)).unwrap_or(0.0);
            // For a card: is it the one that is expanded, and does the tree hold a box for it?
            let card = is_card(&tree.ctls[i]).then(|| (crate::layout_css::is_expanded_card(&tree.ctls, i), tree.card_boxes.contains_key(&i)));
            (i, (dim("Width"), dim("Height")), card)
        };
        if let Some((expanded, known)) = card {
            // A card's box matters only while it is expanded: what is inside it grows by a factor taken from that box (spec 090), and
            // every control inside is drawn again when the factor changes. The box of a card that has been collapsed is forgotten.
            if !expanded && !known {
                return;
            }
            let inside = {
                let Ok(mut host) = inner.host.try_borrow_mut() else { return };
                let Some(tree) = host.trees.get_mut(&inst) else { return };
                if !tree.set_card_box(ctl, expanded.then_some((w, h))) {
                    return;
                }
                let mut all = vec![ctl];
                let mut at = 0;
                while at < all.len() {
                    let kids = tree.ctls[all[at]].children.clone();
                    all.extend(kids);
                    at += 1;
                }
                all
            };
            for c in inside {
                self.mark_dirty(inst, c);
            }
            return;
        }
        let changed = {
            let Ok(mut st) = self.state.try_borrow_mut() else { return };
            let Some(iv) = st.instances.get_mut(&inst) else { return };
            let last = iv.ui.box_sizes.get(&ctl).copied().unwrap_or(designed);
            if (last.0 - w).abs() > 0.25 || (last.1 - h).abs() > 0.25 {
                iv.ui.box_sizes.insert(ctl, (w, h));
                true
            } else {
                false
            }
        };
        if changed {
            self.mark_dirty(inst, ctl);
        }
    }

    // ── breakpoints ─────────────────────────────────────────────────────────

    pub(super) fn on_resize(self: &Rc<Self>, inst: u32, width: f64, height: f64) {
        let Some(inner) = self.inner.upgrade() else { return };
        // What the program is told (P4): the form's size and breakpoint, `onResizing`, `onBreakpointChanged`, and `onResize` once the
        // size has stopped changing for 60 ms (the desktop raises it on the first frame after the last change; a browser
        // reports a window drag in steps, so the page waits for a short quiet).
        let (owed, mode) = {
            let mode = self.state.try_borrow().ok().and_then(|st| st.instances.get(&inst).map(|iv| iv.mode));
            let owed = inner.host.try_borrow_mut().map(|mut host| {
                match mode {
                    Some(OpenMode::Root) => host.set_viewport(Some((width, height)), None),
                    Some(OpenMode::Embedded) => host.set_viewport(None, Some((width, height))),
                    _ => {}
                }
                host.surface_resized(inst, width, height)
            });
            (owed.unwrap_or(false), mode)
        };
        let _ = mode;
        if owed {
            let weak = Rc::downgrade(self);
            let t = crate::api::Timeout::new(60, move || {
                let Some(sh) = weak.upgrade() else { return };
                let Some(inner) = sh.inner.upgrade() else { return };
                if let Ok(mut host) = inner.host.try_borrow_mut() {
                    host.surface_settled(inst);
                }
                sh.kick();
            });
            if let Ok(mut st) = self.state.try_borrow_mut() {
                if let Some(iv) = st.instances.get_mut(&inst) {
                    iv.settle = Some(t);
                }
            }
        }
        self.kick();
        // The breakpoint this width selects and the font factor the form's `FontScaling` makes of it: both the desktop's functions.
        let (name, factor) = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            let bp = if tree.breakpoints.is_empty() { None } else { cobolt_forms::layout::breakpoints::select(&tree.breakpoints, width as f32, None) };
            (bp.map(|b| b.name.clone()), tree.font_factor_at(width, bp))
        };
        let changed = {
            let Ok(mut st) = self.state.try_borrow_mut() else { return };
            let Some(iv) = st.instances.get_mut(&inst) else { return };
            iv.width = width;
            if iv.bp != name {
                iv.bp = name;
                true
            } else {
                false
            }
        };
        // A new font factor changes what every lettered control is drawn at: the rules, and the views that place text by its size
        // (a ListBox's rows, a ComboBox's value, a chart's lettering).
        let mut lettered = 0;
        if let Ok(mut host) = inner.host.try_borrow_mut() {
            if let Some(tree) = host.trees.get_mut(&inst) {
                if tree.set_font_factor(factor) {
                    lettered = tree.ctls.len();
                }
            }
        }
        for i in 1..lettered {
            self.mark_dirty(inst, i);
        }
        if changed || lettered > 0 {
            self.mark_dirty(inst, usize::MAX);
        }
        // What is moving is drawn at its new place (P4-1: a resize during the intro left children off their place).
        self.motion_resized(&inner, inst);
    }

    // ── timers ──────────────────────────────────────────────────────────────

    /// Start, stop or retime the Timer controls of an instance to match their properties.
    pub(super) fn sync_timers(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32) {
        let wanted: Vec<(usize, u64)> = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            tree.ctls
                .iter()
                .enumerate()
                .filter(|(_, c)| matches!(c.control_type(), Some(cobolt_forms::model::ControlType::Timer)))
                // A Timer runs by its own `Enabled` property (`Ctl::timer_on`), not the element's flag.
                .filter(|(_, c)| c.timer_on && c.has_handler("onTick"))
                .map(|(i, _)| {
                    // The desktop's floor: an Interval under 10 ms ticks every 10 ms (`render.rs` CT::Timer).
                    let ms = crate::value::parse_f64(&tree.get(i, "Interval")).unwrap_or(1000.0).max(10.0) as u64;
                    (i, ms)
                })
                .collect()
        };
        let Ok(mut st) = self.state.try_borrow_mut() else { return };
        let running: Vec<usize> = st.timers.keys().filter(|(i, c)| *i == inst && *c != usize::MAX).map(|(_, c)| *c).collect();
        for c in running {
            if !wanted.iter().any(|(w, _)| *w == c) {
                st.timers.remove(&(inst, c));
            }
        }
        for (ctl, ms) in wanted {
            if st.timers.get(&(inst, ctl)).map(|(m, _)| *m) == Some(ms) {
                continue;
            }
            let weak = Rc::downgrade(self);
            let t = Timeout::new(ms.min(u32::MAX as u64) as u32, move || {
                if let Some(sh) = weak.upgrade() {
                    sh.tick(inst, ctl);
                }
            });
            st.timers.insert((inst, ctl), (ms, t));
        }
    }

    fn tick(self: &Rc<Self>, inst: u32, ctl: usize) {
        let Some(inner) = self.inner.upgrade() else { return };
        if let Ok(mut st) = self.state.try_borrow_mut() {
            st.timers.remove(&(inst, ctl));
        }
        inner.push(Inbox::Acts { inst, ctl, acts: vec![Act::Event { name: "onTick", args: vec![] }] });
        // Re-armed once the tick has been taken in (the next render's `sync_timers`), so a handler that
        // takes longer than the interval does not pile ticks up behind itself.
        self.mark_dirty(inst, ctl);
    }

    // ── the shell (spec 049) ────────────────────────────────────────────────

    fn enter_shell(self: &Rc<Self>, inst: u32, title: &str) {
        if let Ok(mut st) = self.state.try_borrow_mut() {
            st.shell = true;
            st.chain = vec![Crumb { inst, label: title.to_owned() }];
        }
        let _ = self.chrome.pane.remove_attribute("hidden");
        // The ContentPane is empty until an embedded form is placed in it (`sync_shell` shows it then). Shown
        // empty it paints its own background over the main form's controls, which are the Home page.
        h::set_bool_attr(&self.chrome.content, "hidden", true);
        h::attr(&self.chrome.main, "data-shell", "");
        self.render_crumbs();
    }

    fn place_embedded(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, opener: Option<u32>, title: &str, root: &Element) {
        h::append(&self.chrome.content, root);
        let to_close: Vec<u32> = {
            let Ok(mut st) = self.state.try_borrow_mut() else { return };
            let at = opener.and_then(|o| st.chain.iter().position(|c| c.inst == o)).unwrap_or(0);
            let popped: Vec<u32> = st.chain.drain(at + 1..).map(|c| c.inst).collect();
            let preserve = std::mem::take(&mut st.preserve_next);
            st.chain.push(Crumb { inst, label: title.to_owned() });
            st.content_inst = Some(inst);
            // Spec 049 R25: the displayed sibling is destroyed unless the item keeps it resident.
            popped.into_iter().rev().filter(|_| !preserve).collect()
        };
        for p in to_close {
            inner.push(Inbox::CloseInstance(p));
        }
        self.sync_shell(inner);
    }

    /// Show the top of the chain in the ContentPane, hide the other embedded forms, and keep the
    /// breadcrumb and the address in step.
    pub(crate) fn sync_shell(self: &Rc<Self>, _inner: &Rc<Inner>) {
        let (top, shell_on) = {
            let Ok(st) = self.state.try_borrow() else { return };
            (st.chain.last().map(|c| c.inst), st.shell)
        };
        if !shell_on {
            return;
        }
        {
            let Ok(st) = self.state.try_borrow() else { return };
            let mut any_embedded_shown = false;
            for (i, iv) in st.instances.iter() {
                if iv.mode == OpenMode::Embedded {
                    let shown = Some(*i) == top;
                    any_embedded_shown |= shown;
                    h::set_bool_attr(&iv.root, "hidden", !shown);
                }
            }
            h::set_bool_attr(&self.chrome.content, "hidden", !any_embedded_shown);
        }
        self.render_crumbs();
        self.sync_address();
    }

    /// The address bar follows the form being shown (WASM-011 R4): a hash route, which works on any
    /// static host without server configuration and does not touch the host page's own path.
    pub(crate) fn sync_address(self: &Rc<Self>) {
        let Some(inner) = self.inner.upgrade() else { return };
        if inner.cfg.standalone {
            return;
        }
        let (name, at_root, params) = {
            let Ok(st) = self.state.try_borrow() else { return };
            let top = st.chain.last();
            let name = top.and_then(|c| st.instances.get(&c.inst)).map(|iv| iv.form.clone());
            let params = top.and_then(|c| inner.host.try_borrow().ok().and_then(|h| h.trees.get(&c.inst).map(|t| t.params.clone()))).unwrap_or_default();
            (name, st.chain.len() <= 1, params)
        };
        let Some(name) = name else { return };
        // The main form is the page's own address (no route): Back from the first route lands on it, and
        // coming home from another form returns to it.
        let hash = if at_root { String::new() } else { shell::Route { form: name.clone(), params }.to_hash() };
        if let Some(w) = web_sys::window() {
            let current = w.location().hash().unwrap_or_default();
            let same = current == hash || (hash.is_empty() && shell::Route::parse(&current).is_none());
            if !same {
                if let Ok(h) = w.history() {
                    let url = if hash.is_empty() {
                        format!("{}{}", w.location().pathname().unwrap_or_default(), w.location().search().unwrap_or_default())
                    } else {
                        hash.clone()
                    };
                    let _ = h.push_state_with_url(&JsValue::NULL, "", Some(&url));
                }
            }
        }
    }
}

trait IntoF64 {
    fn into_f64(self) -> (f64, f64);
}

impl IntoF64 for (u32, u32) {
    fn into_f64(self) -> (f64, f64) {
        (self.0 as f64, self.1 as f64)
    }
}
