// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The page** (WASM-016): the DOM projection of the form instances the host holds.
//!
//! What each control becomes, and what each DOM event asks of the host, is decided by the pure
//! `view` modules and tested natively. This module is the browser half: it creates the elements,
//! keeps them equal to the views ([`apply`]), owns the page's own furniture — the shell, the dialogs,
//! the notifications, the error banner — and routes the browser's events to the driver's inbox.
//!
//! # Rules this module keeps
//!
//! * **No markup is ever inserted**: nodes come from `createElement`, `setAttribute`, `textContent`,
//!   the CSSOM and constructed stylesheets. A test greps the sources for the APIs that would.
//! * **No `Closure::forget`**: every closure is owned by a [`h::Listener`], a `Timeout` or the
//!   `ResizeObs`, which the instance owns, so closing an instance removes everything it added.
//! * **The host is never borrowed across a DOM mutation.** The browser can fire an event
//!   synchronously from inside one (a removed element blurs); a listener therefore only `try_borrow`s,
//!   and queues what the user did into the driver's inbox instead of acting at once.

mod apply;
pub(crate) mod captcha;
mod h;
mod instance;
mod ops;
mod payload;
mod sheets;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::{Rc, Weak};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, Event};

use crate::api::{Inbox, Inner, Surface, Timeout};
use crate::host::{DialogKind, Effect, OpenMode};
use crate::net::LinkState;
use crate::tree::Change;
use crate::view::interact::{self, Act, UiOp};
use crate::view::shell::{self, Crumb, DialogSpec, Toast};
use crate::view::{VNode, ViewCtx};

use apply::{Dispatch, NodeRec};
use instance::InstanceView;
use sheets::{Scope, Sheet};

pub(crate) const DEFAULT_TILE_URL: &str = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";

/// Who a listener reports to.
#[derive(Clone, Debug)]
pub(crate) enum Route {
    /// A control of an instance.
    Ctl { inst: u32, ctl: usize },
    /// The form itself (keys that reach the instance's root).
    Form { inst: u32 },
    /// A dialog the program asked for.
    Dialog(u32),
    /// The frame (title bar, close button) of a modal or panel instance.
    Frame(u32),
    Banner,
    Toasts,
    Crumb,
}

pub(crate) struct Chrome {
    pub main: Element,
    pub pane: Element,
    pub crumb: Element,
    pub content: Element,
    pub layer: Element,
    pub toasts: Element,
    pub banner: Element,
    pub net: Element,
}

struct DialogRec {
    rec: NodeRec,
    spec: DialogSpec,
}

pub(crate) struct State {
    pub instances: BTreeMap<u32, InstanceView>,
    pub base: Sheet,
    pub app: Sheet,
    pub forms: BTreeMap<String, Sheet>,
    dialogs: BTreeMap<u32, DialogRec>,
    /// The navigation chain of the shell (spec 049 R19): main form first.
    pub chain: Vec<Crumb>,
    pub shell: bool,
    pub content_inst: Option<u32>,
    pub preserve_next: bool,
    pub crumb_rec: Option<NodeRec>,
    toasts: Vec<Toast>,
    toast_rec: Option<NodeRec>,
    next_toast: u32,
    banner_rec: Option<NodeRec>,
    net_rec: Option<NodeRec>,
    pub timers: HashMap<(u32, usize), (u64, Timeout)>,
    pub files: HashMap<(u32, usize), Vec<web_sys::File>>,
    pub dirty: BTreeSet<(u32, usize)>,
    pub panels: u32,
    pub sidebar: Option<bool>,
}

pub(crate) struct Shared {
    pub inner: Weak<Inner>,
    pub doc: Document,
    pub root: Element,
    pub scope: Scope,
    pub chrome: Chrome,
    pub state: RefCell<State>,
    me: RefCell<Weak<Shared>>,
    nav: RefCell<Option<h::Listener>>,
}

pub(crate) fn create(inner: &Rc<Inner>, root: Element) -> Result<Box<dyn Surface>, String> {
    let doc = h::document();
    let root_node: JsValue = root.get_root_node().into();
    let scope = Scope::new(root_node, doc.clone());
    let mk = |class: &str| {
        let e = h::el(&doc, "div", &[class]);
        let _ = root.append_child(&e);
        e
    };
    let banner = mk("pr-banner-host");
    let main = mk("pr-main");
    let pane = mk("pr-shell-pane");
    let crumb = h::el(&doc, "div", &["pr-crumb-host"]);
    let content = h::el(&doc, "div", &["pr-content"]);
    h::append(&pane, &crumb);
    h::append(&pane, &content);
    let layer = mk("pr-layer");
    let toasts = mk("pr-toasts-host");
    let net = mk("pr-net-host");
    h::attr(&pane, "hidden", "");
    let base = scope.new_sheet();
    base.set(&crate::base_css::base_layer());
    let app = scope.new_sheet();
    let state = State {
        instances: BTreeMap::new(),
        base,
        app,
        forms: BTreeMap::new(),
        dialogs: BTreeMap::new(),
        chain: Vec::new(),
        shell: false,
        content_inst: None,
        preserve_next: false,
        crumb_rec: None,
        toasts: Vec::new(),
        toast_rec: None,
        next_toast: 1,
        banner_rec: None,
        net_rec: None,
        timers: HashMap::new(),
        files: HashMap::new(),
        dirty: BTreeSet::new(),
        panels: 0,
        sidebar: None,
    };
    let sh = Rc::new(Shared {
        inner: Rc::downgrade(inner),
        doc,
        root,
        scope,
        chrome: Chrome { main, pane, crumb, content, layer, toasts, banner, net },
        state: RefCell::new(state),
        me: RefCell::new(Weak::new()),
        nav: RefCell::new(None),
    });
    *sh.me.borrow_mut() = Rc::downgrade(&sh);
    let nav = sh.watch_address();
    *sh.nav.borrow_mut() = nav;
    sh.adopt();
    Ok(Box::new(Dom { sh }))
}

struct Dom {
    sh: Rc<Shared>,
}

impl Shared {
    fn rc(&self) -> Rc<Shared> {
        self.me.borrow().upgrade().expect("the surface is alive while it is used")
    }

    /// A listener target that reports to `route`.
    pub(crate) fn dispatch(&self, route: Route) -> Dispatch {
        let weak = self.me.borrow().clone();
        Rc::new(move |kind: &str, key: &str, ev: &Event, root: &Element| {
            if let Some(sh) = weak.upgrade() {
                sh.on_event(&route, kind, key, ev, root);
            }
        })
    }

    pub(crate) fn tile_url(&self, inner: &Inner) -> String {
        if inner.cfg.tile_url.trim().is_empty() {
            DEFAULT_TILE_URL.to_owned()
        } else {
            inner.cfg.tile_url.clone()
        }
    }

    /// Put the sheets on the page, in cascade order: the base (lowest layer), the properties of every
    /// instance (the next layer), the application's stylesheet, then each form's (unlayered, so they win).
    pub(crate) fn adopt(&self) {
        let st = self.state.borrow();
        let mut list: Vec<&Sheet> = vec![&st.base];
        for iv in st.instances.values() {
            list.push(&iv.sheet);
        }
        list.push(&st.app);
        for s in st.forms.values() {
            list.push(s);
        }
        self.scope.adopt(&list);
    }

    /// Ask the driver for a render pass (something outside it changed what is on the page).
    pub(crate) fn kick(&self) {
        if let Some(inner) = self.inner.upgrade() {
            inner.push(Inbox::Raw(Box::new(|_| {})));
        }
    }

    pub(crate) fn mark_dirty(&self, inst: u32, ctl: usize) {
        if let Ok(mut st) = self.state.try_borrow_mut() {
            st.dirty.insert((inst, ctl));
        }
        self.kick();
    }

    // ── events ──────────────────────────────────────────────────────────────

    fn on_event(self: &Rc<Self>, route: &Route, kind: &str, key: &str, ev: &Event, root: &Element) {
        match route {
            Route::Ctl { inst, ctl } => self.on_control_event(*inst, *ctl, kind, key, ev, root),
            Route::Form { inst } => self.on_form_event(*inst, kind, ev, root),
            Route::Dialog(id) => self.on_dialog_event(*id, kind, ev),
            Route::Frame(inst) => self.on_frame_event(*inst, kind, ev),
            Route::Banner => self.on_banner_event(ev),
            Route::Toasts => self.on_toast_event(ev),
            Route::Crumb => self.on_crumb_event(ev),
        }
    }

    fn on_control_event(self: &Rc<Self>, inst: u32, ctl: usize, kind: &str, key: &str, ev: &Event, root: &Element) {
        let Some(inner) = self.inner.upgrade() else { return };
        let p = payload::from_event(kind, key, ev, root);
        let tile = self.tile_url(&inner);
        let acts = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            let Ok(st) = self.state.try_borrow() else { return };
            let Some(iv) = st.instances.get(&inst) else { return };
            let Ok(styles) = inner.styles.try_borrow() else { return };
            let cx = ViewCtx {
                tree,
                assets: &styles.assets,
                locale: &styles.locale,
                ui: &iv.ui,
                menus: &styles.menus,
                image_origins: &inner.cfg.image_origins,
                tile_url: &tile,
            };
            interact::on_event(&cx, ctl, &p)
        };
        // Keep the files a drop or a picker gave: the page holds the bytes, the program never sees them.
        if let Some(files) = dropped_files(ev) {
            if let Ok(mut st) = self.state.try_borrow_mut() {
                st.files.insert((inst, ctl), files);
            }
        }
        if acts.iter().any(|a| matches!(a, Act::Prevent)) {
            ev.prevent_default();
        }
        // A drag (a map pan, a knob turn, the splitter's handle) keeps hearing the pointer after it
        // leaves the element: capture it for the length of the press.
        if kind == "pointerdown" && (key == "handle" || acts.iter().any(|a| matches!(a, Act::Ui(UiOp::MapDrag { .. }) | Act::Ui(UiOp::Focus)))) {
            if let Some(cur) = ev.current_target().and_then(|t| t.dyn_into::<Element>().ok()) {
                let id = js_sys::Reflect::get(ev.as_ref(), &"pointerId".into()).unwrap_or(JsValue::UNDEFINED);
                let _ = js_sys::Reflect::get(&cur, &"setPointerCapture".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()).map(|f| f.call1(&cur, &id));
            }
        }
        if !acts.is_empty() {
            inner.push(Inbox::Acts { inst, ctl, acts });
        }
    }

    /// Keys that reach the form's root: the form's own key events, and Enter pressing the default button.
    fn on_form_event(self: &Rc<Self>, inst: u32, kind: &str, ev: &Event, root: &Element) {
        let Some(inner) = self.inner.upgrade() else { return };
        if kind != "keydown" || ev.default_prevented() {
            return;
        }
        let tag = ev.target().and_then(|t| t.dyn_into::<Element>().ok()).map(|e| e.tag_name().to_ascii_lowercase()).unwrap_or_default();
        let p = payload::from_event(kind, "root", ev, root);
        let plan = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            let mut plan: Vec<(usize, Vec<Act>)> = interact::generic(tree.ctl(0), &p).into_iter().map(|a| (0usize, vec![a])).collect();
            // Enter inside a button, link, select or text area keeps its own meaning.
            if !matches!(tag.as_str(), "button" | "a" | "select" | "textarea") {
                plan.extend(interact::form_key(tree, &p));
            }
            plan
        };
        for (ctl, acts) in plan {
            inner.push(Inbox::Acts { inst, ctl, acts });
        }
    }

    fn on_dialog_event(self: &Rc<Self>, id: u32, kind: &str, ev: &Event) {
        let Some(inner) = self.inner.upgrade() else { return };
        let result = {
            let Ok(st) = self.state.try_borrow() else { return };
            let Some(d) = st.dialogs.get(&id) else { return };
            match kind {
                "cancel" => {
                    ev.prevent_default();
                    Some(shell::dialog_cancel_result(&d.spec))
                }
                "click" => {
                    let target = ev.target().and_then(|t| t.dyn_into::<Element>().ok());
                    let button = target.and_then(|t| t.closest("[data-part=dialog-button]").ok().flatten());
                    button.map(|b| {
                        let label = b.get_attribute("data-id").unwrap_or_default();
                        if d.spec.kind == DialogKind::Input {
                            if label == "OK" {
                                apply::find_key(&d.rec, "input").map(|i| js_value(&i)).unwrap_or_default()
                            } else {
                                String::new()
                            }
                        } else {
                            label
                        }
                    })
                }
                "keydown" => {
                    let key = js_str(ev.as_ref(), "key");
                    if key == "Enter" && d.spec.kind == DialogKind::Input && ev.target().and_then(|t| t.dyn_into::<Element>().ok()).is_some_and(|t| t.tag_name().eq_ignore_ascii_case("input")) {
                        ev.prevent_default();
                        apply::find_key(&d.rec, "input").map(|i| js_value(&i))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };
        if let Some(result) = result {
            self.close_dialog(id);
            inner.push(Inbox::DialogResult { id, result });
        }
    }

    fn on_frame_event(self: &Rc<Self>, inst: u32, kind: &str, ev: &Event) {
        let Some(inner) = self.inner.upgrade() else { return };
        match kind {
            "cancel" => {
                // Escape on a modal form closes it, as the window's close button does.
                ev.prevent_default();
                inner.push(Inbox::CloseInstance(inst));
            }
            "click" => {
                let target = ev.target().and_then(|t| t.dyn_into::<Element>().ok());
                if target.and_then(|t| t.closest("[data-part=close]").ok().flatten()).is_some() {
                    inner.push(Inbox::CloseInstance(inst));
                }
            }
            "pointerdown" => self.start_panel_drag(inst, ev),
            _ => {}
        }
    }

    fn on_banner_event(self: &Rc<Self>, ev: &Event) {
        let target = ev.target().and_then(|t| t.dyn_into::<Element>().ok());
        let Some(btn) = target.and_then(|t| t.closest("[data-part]").ok().flatten()) else { return };
        match btn.get_attribute("data-part").as_deref() {
            Some("dismiss") => self.set_banner(None),
            Some("reload") => {
                if let Some(w) = web_sys::window() {
                    let _ = w.location().reload();
                }
            }
            _ => {}
        }
    }

    fn on_crumb_event(self: &Rc<Self>, ev: &Event) {
        let Some(inner) = self.inner.upgrade() else { return };
        let target = ev.target().and_then(|t| t.dyn_into::<Element>().ok());
        let Some(btn) = target.and_then(|t| t.closest("[data-part=crumb]").ok().flatten()) else { return };
        let Some(inst) = btn.get_attribute("data-inst").and_then(|v| v.parse::<u32>().ok()) else { return };
        // Spec 049 R22: destroy every form below the clicked one, newest first, and show it.
        let above: Vec<u32> = {
            let Ok(st) = self.state.try_borrow() else { return };
            match st.chain.iter().position(|c| c.inst == inst) {
                Some(at) => st.chain[at + 1..].iter().rev().map(|c| c.inst).collect(),
                None => return,
            }
        };
        for i in above {
            inner.push(Inbox::CloseInstance(i));
        }
    }

    fn on_toast_event(self: &Rc<Self>, ev: &Event) {
        let Some(inner) = self.inner.upgrade() else { return };
        let target = ev.target().and_then(|t| t.dyn_into::<Element>().ok());
        let Some(btn) = target.and_then(|t| t.closest("[data-part]").ok().flatten()) else { return };
        let Some(toast_id) = btn.get_attribute("data-toast").and_then(|v| v.parse::<u32>().ok()) else { return };
        let part = btn.get_attribute("data-part").unwrap_or_default();
        let (ctl, inst, acts) = {
            let Ok(st) = self.state.try_borrow() else { return };
            let Some(t) = st.toasts.iter().find(|t| t.id == toast_id) else { return };
            let inst = t.inst;
            match part.as_str() {
                "toast-button" => {
                    let id = btn.get_attribute("data-id").unwrap_or_default();
                    let index = t.buttons.iter().position(|b| b.id == id).unwrap_or(0);
                    let dismiss = btn.get_attribute("data-dismiss").as_deref() == Some("1");
                    (t.ctl, inst, interact::toast_button(&id, index, dismiss))
                }
                "toast-close" => (t.ctl, inst, vec![Act::Event { name: "onClosing", args: vec![] }, Act::Event { name: "onClosed", args: vec![] }]),
                _ => return,
            }
        };
        let dismiss = part == "toast-close" || acts.iter().any(|a| matches!(a, Act::Event { name: "onClosed", .. }));
        inner.push(Inbox::Acts { inst, ctl, acts });
        if dismiss {
            self.dismiss_toast(toast_id);
        }
    }

    // ── the page's own furniture ────────────────────────────────────────────

    pub(crate) fn set_banner(self: &Rc<Self>, message: Option<(String, bool)>) {
        let Ok(mut st) = self.state.try_borrow_mut() else { return };
        match message {
            Some((text, reload)) => {
                let v = shell::banner_view(&text, reload);
                match st.banner_rec.as_mut() {
                    Some(rec) => {
                        apply::patch(&self.doc, rec, &v, None, &self.dispatch(Route::Banner));
                    }
                    None => {
                        let rec = apply::create(&self.doc, &v, None, &self.dispatch(Route::Banner));
                        h::append(&self.chrome.banner, &rec.el);
                        st.banner_rec = Some(rec);
                    }
                }
            }
            None => {
                if let Some(rec) = st.banner_rec.take() {
                    rec.el.remove();
                }
            }
        }
    }

    pub(crate) fn render_toasts(self: &Rc<Self>) {
        let Ok(mut st) = self.state.try_borrow_mut() else { return };
        let v = shell::toast_stack(&st.toasts);
        match st.toast_rec.as_mut() {
            Some(rec) => {
                apply::patch(&self.doc, rec, &v, None, &self.dispatch(Route::Toasts));
            }
            None => {
                let rec = apply::create(&self.doc, &v, None, &self.dispatch(Route::Toasts));
                h::append(&self.chrome.toasts, &rec.el);
                st.toast_rec = Some(rec);
            }
        }
    }

    pub(crate) fn show_toast(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, ctl: usize, text: Option<&str>) {
        let toast = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            let Ok(mut st) = self.state.try_borrow_mut() else { return };
            let id = st.next_toast;
            st.next_toast += 1;
            shell::mint_toast(tree, inst, ctl, id, text, crate::api::now_ms())
        };
        let (id, timeout, text) = (toast.id, toast.timeout_ms, toast.text.clone());
        {
            let Ok(mut st) = self.state.try_borrow_mut() else { return };
            // The newest are the last; keep at most the template's MaximumVisible (the oldest leave first).
            st.toasts.push(toast);
            while st.toasts.len() > 5 {
                st.toasts.remove(0);
            }
        }
        self.render_toasts();
        let _ = text;
        inner.push(Inbox::Acts { inst, ctl, acts: vec![Act::Event { name: "onShown", args: vec![] }] });
        if timeout > 0 {
            let weak = self.me.borrow().clone();
            let t = Timeout::new(timeout.min(u32::MAX as i64) as u32, move || {
                if let Some(sh) = weak.upgrade() {
                    sh.expire_toast(id);
                }
            });
            if let Ok(mut st) = self.state.try_borrow_mut() {
                st.timers.insert((u32::MAX - id, 0), (id as u64, t));
            }
        }
    }

    fn expire_toast(self: &Rc<Self>, id: u32) {
        let Some(inner) = self.inner.upgrade() else { return };
        let info = {
            let Ok(mut st) = self.state.try_borrow_mut() else { return };
            st.timers.remove(&(u32::MAX - id, 0));
            st.toasts.iter().find(|t| t.id == id).map(|t| (t.inst, t.ctl))
        };
        if let Some((inst, ctl)) = info {
            inner.push(Inbox::Acts { inst, ctl, acts: vec![Act::Event { name: "onTimeout", args: vec![] }, Act::Event { name: "onClosed", args: vec![] }] });
        }
        self.dismiss_toast(id);
    }

    pub(crate) fn dismiss_toast(self: &Rc<Self>, id: u32) {
        if let Ok(mut st) = self.state.try_borrow_mut() {
            st.toasts.retain(|t| t.id != id);
            st.timers.remove(&(u32::MAX - id, 0));
        }
        self.render_toasts();
    }

    pub(crate) fn show_dialog(self: &Rc<Self>, spec: DialogSpec) {
        let v = shell::dialog_view(&spec);
        let rec = apply::create(&self.doc, &v, None, &self.dispatch(Route::Dialog(spec.id)));
        h::append(&self.root, &rec.el);
        // `showModal` puts it in the top layer: everything behind it is inert, focus is trapped and
        // Escape closes it (the `cancel` event).
        let _ = js_sys::Reflect::get(&rec.el, &"showModal".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()).map(|f| f.call0(&rec.el));
        if let Some(input) = apply::find_key(&rec, "input") {
            h::focus(&input);
        }
        if let Ok(mut st) = self.state.try_borrow_mut() {
            st.dialogs.insert(spec.id, DialogRec { rec, spec });
        }
    }

    fn close_dialog(self: &Rc<Self>, id: u32) {
        let rec = self.state.try_borrow_mut().ok().and_then(|mut st| st.dialogs.remove(&id));
        if let Some(d) = rec {
            let _ = js_sys::Reflect::get(&d.rec.el, &"close".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()).map(|f| f.call0(&d.rec.el));
            d.rec.el.remove();
        }
    }

    pub(crate) fn render_crumbs(self: &Rc<Self>) {
        let Ok(mut st) = self.state.try_borrow_mut() else { return };
        let v = shell::breadcrumb(&st.chain, "");
        match st.crumb_rec.as_mut() {
            Some(rec) => {
                apply::patch(&self.doc, rec, &v, None, &self.dispatch(Route::Crumb));
            }
            None => {
                let rec = apply::create(&self.doc, &v, None, &self.dispatch(Route::Crumb));
                h::append(&self.chrome.crumb, &rec.el);
                st.crumb_rec = Some(rec);
            }
        }
    }

    fn start_panel_drag(self: &Rc<Self>, inst: u32, ev: &Event) {
        // A panel window is moved by its title bar: the pointer is captured and the window follows it
        // until the button is released. The window changes position only because the user dragged it.
        let target = ev.target().and_then(|t| t.dyn_into::<Element>().ok());
        if target.as_ref().and_then(|t| t.closest("[data-part=close]").ok().flatten()).is_some() {
            return;
        }
        let Some(frame_el) = self.state.try_borrow().ok().and_then(|st| st.instances.get(&inst).and_then(|iv| iv.frame.as_ref().map(|f| f.el.clone()))) else { return };
        let Some(bar) = target.and_then(|t| t.closest(".pr-titlebar").ok().flatten()) else { return };
        let evj: &JsValue = ev.as_ref();
        let (sx, sy) = (js_num(evj, "clientX"), js_num(evj, "clientY"));
        let start_left = frame_el.get_attribute("style").map(|_| 0.0).unwrap_or(0.0);
        let _ = start_left;
        let rect = frame_el.get_bounding_client_rect();
        let parent = frame_el.parent_element().map(|p| p.get_bounding_client_rect());
        let (ox, oy) = parent.map(|p| (rect.left() - p.left(), rect.top() - p.top())).unwrap_or((0.0, 0.0));
        if let Some(f) = js_sys::Reflect::get(&bar, &"setPointerCapture".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()) {
            let _ = f.call1(&bar, &js_sys::Reflect::get(evj, &"pointerId".into()).unwrap_or(JsValue::UNDEFINED));
        }
        let weak = self.me.borrow().clone();
        let frame2 = frame_el.clone();
        let bar2 = bar.clone();
        let mv = h::Listener::new(&bar, "pointermove", move |e: Event| {
            let ej: &JsValue = e.as_ref();
            if js_num(ej, "buttons") as u32 & 1 == 0 {
                return;
            }
            h::css(&frame2, "left", &format!("{}px", (ox + js_num(ej, "clientX") - sx).round()));
            h::css(&frame2, "top", &format!("{}px", (oy + js_num(ej, "clientY") - sy).round().max(0.0)));
        });
        let end = h::Listener::new(&bar, "pointerup", move |_e: Event| {
            let _ = &bar2;
        });
        if let Some(sh) = weak.upgrade() {
            if let Ok(mut st) = sh.state.try_borrow_mut() {
                if let Some(iv) = st.instances.get_mut(&inst) {
                    iv.drag = Some((mv, end));
                }
            }
        }
    }
}

fn dropped_files(ev: &Event) -> Option<Vec<web_sys::File>> {
    let evj: &JsValue = ev.as_ref();
    let dt = js_sys::Reflect::get(evj, &"dataTransfer".into()).ok().filter(|d| d.is_object());
    let list = match dt {
        Some(d) => js_sys::Reflect::get(&d, &"files".into()).ok(),
        None => ev.target().and_then(|t| js_sys::Reflect::get(&t, &"files".into()).ok()),
    }?;
    if !list.is_object() {
        return None;
    }
    let n = js_num(&list, "length") as u32;
    if n == 0 {
        return None;
    }
    let mut out = Vec::new();
    for k in 0..n {
        if let Ok(f) = js_sys::Reflect::get(&list, &k.into()) {
            if let Ok(f) = f.dyn_into::<web_sys::File>() {
                out.push(f);
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

pub(crate) fn js_num(o: &JsValue, k: &str) -> f64 {
    js_sys::Reflect::get(o, &k.into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

pub(crate) fn js_str(o: &JsValue, k: &str) -> String {
    js_sys::Reflect::get(o, &k.into()).ok().and_then(|v| v.as_string()).unwrap_or_default()
}

fn js_value(el: &Element) -> String {
    js_str(el.as_ref(), "value")
}

// ── the Surface the driver talks to ─────────────────────────────────────────

impl Surface for Dom {
    fn opened(&self, inner: &Rc<Inner>, inst: u32, mode: OpenMode, handle: Option<String>) {
        self.sh.open_instance(inner, inst, mode, handle);
    }

    fn closed(&self, inner: &Rc<Inner>, inst: u32) {
        self.sh.close_instance(inner, inst);
    }

    fn render(&self, inner: &Rc<Inner>) {
        self.sh.render_all(inner);
    }

    fn effect(&self, inner: &Rc<Inner>, e: &Effect) {
        self.sh.on_effect(inner, e);
    }

    fn net_state(&self, _inner: &Rc<Inner>, s: LinkState) {
        let text = match s {
            LinkState::Ready | LinkState::Closed => None,
            LinkState::Connecting | LinkState::Authenticating => Some("Connecting\u{2026}"),
            LinkState::Backoff => Some("Reconnecting\u{2026}"),
        };
        let sh = &self.sh;
        let Ok(mut st) = sh.state.try_borrow_mut() else { return };
        match text {
            Some(t) => {
                let v = shell::net_badge(t);
                match st.net_rec.as_mut() {
                    Some(rec) => {
                        apply::patch(&sh.doc, rec, &v, None, &sh.dispatch(Route::Banner));
                    }
                    None => {
                        let rec = apply::create(&sh.doc, &v, None, &sh.dispatch(Route::Banner));
                        h::append(&sh.chrome.net, &rec.el);
                        st.net_rec = Some(rec);
                    }
                }
            }
            None => {
                if let Some(rec) = st.net_rec.take() {
                    rec.el.remove();
                }
            }
        }
    }

    fn styles_changed(&self, inner: &Rc<Inner>) {
        self.sh.styles_changed(inner);
    }

    fn fatal(&self, message: &str) {
        self.sh.set_banner(Some((message.to_owned(), true)));
    }

    fn unmount(&self) {
        self.sh.unmount();
    }

    fn ui_op(&self, inner: &Rc<Inner>, inst: u32, ctl: usize, op: UiOp) {
        self.sh.ui_op(inner, inst, ctl, op);
    }

    fn act(&self, inner: &Rc<Inner>, inst: u32, ctl: usize, act: Act) {
        self.sh.act(inner, inst, ctl, act);
    }
}

/// What a render pass learned about one instance.
#[derive(Default)]
pub(crate) struct InstChanges {
    pub dirty: BTreeSet<usize>,
    pub commands: Vec<(usize, String, Vec<String>)>,
}

impl Shared {
    fn render_all(self: &Rc<Self>, inner: &Rc<Inner>) {
        let changes = match inner.host.try_borrow_mut() {
            Ok(mut h) => h.take_changes(),
            Err(_) => return,
        };
        let mut per: BTreeMap<u32, InstChanges> = BTreeMap::new();
        for (inst, ch) in changes {
            let e = per.entry(inst).or_default();
            match ch {
                Change::Prop { ctl, .. } => {
                    e.dirty.insert(ctl);
                }
                Change::Command { ctl, name, args } => e.commands.push((ctl, name, args)),
            }
        }
        if let Ok(mut st) = self.state.try_borrow_mut() {
            for (inst, ctl) in std::mem::take(&mut st.dirty) {
                per.entry(inst).or_default().dirty.insert(ctl);
            }
        }
        for (inst, ch) in per {
            self.render_instance(inner, inst, ch);
        }
    }

    fn styles_changed(self: &Rc<Self>, inner: &Rc<Inner>) {
        // The application stylesheet and each form's.
        let (app, forms, origins) = {
            let s = inner.styles.borrow();
            (s.app.clone(), s.forms.clone(), inner.cfg.image_origins.clone())
        };
        {
            let Ok(mut st) = self.state.try_borrow_mut() else { return };
            match crate::css::css_is_safe(&app, &origins) {
                Ok(()) => {
                    st.app.set(&app);
                }
                Err(e) => {
                    drop(st);
                    self.set_banner(Some((e, false)));
                    return self.after_styles(inner);
                }
            }
            for (name, css) in &forms {
                match crate::css::css_is_safe(css, &origins) {
                    Ok(()) => {
                        let sheet = st.forms.entry(name.clone()).or_insert_with(|| self.scope.new_sheet());
                        sheet.set_scoped(name, css);
                    }
                    Err(e) => {
                        web_sys::console::error_1(&format!("the stylesheet of form {name} is refused: {e}").into());
                    }
                }
            }
        }
        self.after_styles(inner);
    }

    fn after_styles(self: &Rc<Self>, inner: &Rc<Inner>) {
        self.adopt();
        // Menus, assets and the locale change what controls show: redraw everything.
        let all: Vec<(u32, usize)> = {
            let host = inner.host.borrow();
            host.trees.iter().flat_map(|(i, t)| (1..t.ctls.len()).map(move |c| (*i, c))).collect()
        };
        if let Ok(mut st) = self.state.try_borrow_mut() {
            for k in all {
                st.dirty.insert(k);
            }
        }
        self.kick();
    }

    fn unmount(self: &Rc<Self>) {
        if let Ok(mut st) = self.state.try_borrow_mut() {
            st.timers.clear();
            st.files.clear();
            st.toasts.clear();
            st.toast_rec = None;
            st.banner_rec = None;
            st.net_rec = None;
            st.crumb_rec = None;
            st.dialogs.clear();
            for (_, iv) in std::mem::take(&mut st.instances) {
                iv.dispose();
            }
            st.base.dispose();
            st.app.dispose();
            for (_, s) in std::mem::take(&mut st.forms) {
                s.dispose();
            }
        }
        self.scope.adopt(&[]);
        h::clear(&self.root);
    }
}

#[allow(dead_code)]
fn _vnode(_: VNode) {}
