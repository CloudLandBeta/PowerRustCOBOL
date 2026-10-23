// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Leptos node layer** (WASM-016 R1, plan §1.2): a [`VNode`] drawn by Leptos, and kept equal to
//! the next one by writing signals. It replaced the hand-written patch (`dom/apply.rs`, WASM-016 T24)
//! with the same API, so the page around it — instances, shell, dialogs, notices — did not change.
//!
//! * Every node of a control is an [`RNode`]: its own reactive [`Owner`], a signal for what the element
//!   says about itself (classes, attributes, CSSOM declarations, properties, listeners), one for its
//!   text and one for its children's identities. One generic view draws any `RNode`: the element, its
//!   text, and its children through a keyed [`For`], so a child whose key and tag stay the same keeps
//!   its element (and its focus, caret and scroll position).
//! * What an element says about itself is applied by a render effect through the rules of `attrs`,
//!   so `value`, `checked` and `selected` are compared with the live element.
//! * **Synchronous.** The page around this layer reads the elements back at once (a slot to mount a
//!   child control in, a dialog's input to focus), so Leptos's work is run to the end before
//!   [`create`] and [`patch`] return: the reactive system runs on a local executor that this module
//!   polls (`Executor::poll_local`), not on the next microtask.
//! * No markup: elements come from Leptos's element builders (`createElement` / `createElementNS`),
//!   never an HTML string; `tests/sinks.rs` greps this file with the others.
//! * Disposal: dropping a [`NodeRec`] drops its nodes, each of which cleans up its `Owner` (its
//!   signals and effects) and its listeners.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use any_spawner::{CustomExecutor, Executor, PinnedFuture, PinnedLocalFuture};
use futures::executor::{LocalPool, LocalSpawner};
use futures::task::LocalSpawnExt;
use leptos::prelude::*;
use leptos::tachys::html::directive::directive;
use leptos::tachys::view::any_view::AnyViewState;
use leptos::tachys::view::Render;
use web_sys::{Document, Element, Event};

use super::attrs::{apply_attrs, apply_class, apply_prop, apply_styles, follows_children, needs_active_listener};
use super::h::Listener;
use crate::view::{Prop, Slot, VNode};

/// What a listener hands back: the DOM event type, the key of the node it listens on, the event, and
/// the control's root element.
pub type Dispatch = Rc<dyn Fn(&str, &str, &Event, &Element)>;

pub use super::attrs::focus_requested;

// ── the executor ────────────────────────────────────────────────────────────

/// Leptos's reactive work on a pool this module runs to the end after each write ([`flush`]); work
/// spawned outside a flush runs on the next microtask.
struct LocalExec {
    pool: RefCell<LocalPool>,
    spawner: LocalSpawner,
    scheduled: Rc<Cell<bool>>,
}

impl LocalExec {
    fn schedule(&self) {
        if self.scheduled.replace(true) {
            return;
        }
        let flag = self.scheduled.clone();
        wasm_bindgen_futures::spawn_local(async move {
            flag.set(false);
            Executor::poll_local();
        });
    }
}

impl CustomExecutor for LocalExec {
    fn spawn(&self, fut: PinnedFuture<()>) {
        let _ = self.spawner.spawn_local(fut);
        self.schedule();
    }

    fn spawn_local(&self, fut: PinnedLocalFuture<()>) {
        let _ = self.spawner.spawn_local(fut);
        self.schedule();
    }

    fn poll_local(&self) {
        // Re-entrant calls (a poll from inside a task) are skipped: the outer poll runs to the end.
        if let Ok(mut pool) = self.pool.try_borrow_mut() {
            pool.run_until_stalled();
        }
    }
}

fn init_executor() {
    thread_local! {
        static DONE: Cell<bool> = const { Cell::new(false) };
    }
    if DONE.with(|d| d.replace(true)) {
        return;
    }
    let pool = LocalPool::new();
    let spawner = pool.spawner();
    // An executor already set (a second player on the page) is the same kind: keep it.
    let _ = Executor::init_local_custom_executor(LocalExec { pool: RefCell::new(pool), spawner, scheduled: Rc::new(Cell::new(false)) });
}

thread_local! {
    /// `value` / `selectedIndex` writes waiting for their element's children (a `<select>`'s options).
    static AFTER_CHILDREN: RefCell<Vec<(Element, &'static str, Prop)>> = const { RefCell::new(Vec::new()) };
    /// Every live node by id: the keyed list of a parent names its children by id.
    static NODES: RefCell<HashMap<u64, Weak<RNode>>> = RefCell::new(HashMap::new());
    static NEXT_ID: Cell<u64> = const { Cell::new(1) };
}

/// Run Leptos's pending work, then the writes that follow children.
fn flush() {
    Executor::poll_local();
    let later = AFTER_CHILDREN.with(|a| std::mem::take(&mut *a.borrow_mut()));
    for (el, name, p) in later {
        apply_prop(&el, name, &p, None);
    }
}

// ── one node ────────────────────────────────────────────────────────────────

/// What an element says about itself (everything of a `VNode` but its text and children).
#[derive(Clone, PartialEq, Default)]
struct Own {
    classes: Vec<String>,
    attrs: Vec<(String, String)>,
    props: Vec<(&'static str, Prop)>,
    styles: Vec<(String, String)>,
    on: Vec<&'static str>,
    slot: Option<Slot>,
    has_children: bool,
}

impl Own {
    fn of(v: &VNode) -> Own {
        Own {
            classes: v.classes.clone(),
            attrs: v.attrs.clone(),
            props: v.props.clone(),
            styles: v.styles.clone(),
            on: v.on.clone(),
            slot: v.slot,
            has_children: !v.children.is_empty(),
        }
    }
}

/// What the effect of one element has applied, so the next run changes only what differs.
struct Applied {
    own: Own,
    style_names: Vec<String>,
    listeners: Vec<Listener>,
}

/// The tree a control's nodes share: whom their listeners report to, and the control's root element.
struct Ctx {
    dispatch: RefCell<Dispatch>,
    root: RefCell<Option<Element>>,
}

pub struct RNode {
    id: u64,
    tag: &'static str,
    svg: bool,
    key: String,
    owner: Owner,
    ctx: Rc<Ctx>,
    own: RwSignal<Own>,
    text: RwSignal<Option<String>>,
    kids_ids: RwSignal<Vec<u64>>,
    kids: RefCell<Vec<Rc<RNode>>>,
    el: RefCell<Option<Element>>,
}

impl Drop for RNode {
    fn drop(&mut self) {
        NODES.with(|n| n.borrow_mut().remove(&self.id));
        self.owner.cleanup();
    }
}

impl RNode {
    /// `parent` is the owner of the parent node; the root of a tree owns itself.
    fn new(v: &VNode, ctx: &Rc<Ctx>, parent: Option<&Owner>) -> Rc<RNode> {
        let owner = parent.map(Owner::child).unwrap_or_else(Owner::new);
        let id = NEXT_ID.with(|n| n.replace(n.get() + 1));
        let (own, text, kids_ids, kids) = owner.with(|| {
            let kids: Vec<Rc<RNode>> = v.children.iter().map(|c| RNode::new(c, ctx, Some(&owner))).collect();
            (RwSignal::new(Own::of(v)), RwSignal::new(v.text.clone()), RwSignal::new(kids.iter().map(|k| k.id).collect::<Vec<_>>()), kids)
        });
        let node = Rc::new(RNode {
            id,
            tag: v.tag,
            svg: v.svg,
            key: v.key.clone(),
            owner,
            ctx: ctx.clone(),
            own,
            text,
            kids_ids,
            kids: RefCell::new(kids),
            el: RefCell::new(None),
        });
        NODES.with(|n| n.borrow_mut().insert(id, Rc::downgrade(&node)));
        node
    }

    fn same_kind(&self, v: &VNode) -> bool {
        self.key == v.key && self.tag == v.tag && self.svg == v.svg
    }

    /// Write `v` into the signals. Answers whether a child element was made (the slots may have changed);
    /// the nodes that left are put in `gone`, to be dropped once Leptos has taken their elements away.
    fn update(&self, v: &VNode, gone: &mut Vec<Rc<RNode>>) -> bool {
        let own = Own::of(v);
        if self.own.with_untracked(|o| *o != own) {
            self.own.set(own);
        }
        if self.text.with_untracked(|t| *t != v.text) {
            self.text.set(v.text.clone());
        }
        let mut made = false;
        let old = std::mem::take(&mut *self.kids.borrow_mut());
        let mut old = old.into_iter().map(Some).collect::<Vec<_>>();
        let mut next = Vec::with_capacity(v.children.len());
        for (k, c) in v.children.iter().enumerate() {
            match old.get_mut(k).and_then(|o| o.take_if(|o| o.same_kind(c))) {
                Some(kept) => {
                    made |= kept.update(c, gone);
                    next.push(kept);
                }
                None => {
                    next.push(RNode::new(c, &self.ctx, Some(&self.owner)));
                    made = true;
                }
            }
        }
        gone.extend(old.into_iter().flatten());
        let ids: Vec<u64> = next.iter().map(|n| n.id).collect();
        *self.kids.borrow_mut() = next;
        if self.kids_ids.with_untracked(|i| *i != ids) {
            self.kids_ids.set(ids);
        }
        made
    }

    /// The element was made: keep it, and apply what it says about itself now and at every change.
    fn attach(self: &Rc<Self>, el: Element) {
        *self.el.borrow_mut() = Some(el.clone());
        if self.ctx.root.borrow().is_none() {
            *self.ctx.root.borrow_mut() = Some(el.clone());
        }
        let own = self.own;
        let ctx = self.ctx.clone();
        let key = self.key.clone();
        // The effect lives in the owner of the view being built (the keyed row), and goes with it.
        let effect = RenderEffect::new(move |prev: Option<Applied>| {
            let Some(new) = own.try_get() else { return prev.unwrap_or(Applied { own: Own::default(), style_names: Vec::new(), listeners: Vec::new() }) };
            let (old, style_names, mut listeners) = match prev {
                Some(a) => (Some(a.own), a.style_names, a.listeners),
                None => (None, Vec::new(), Vec::new()),
            };
            let blank = Own::default();
            let was = old.as_ref().unwrap_or(&blank);
            if old.is_none() || was.classes != new.classes {
                apply_class(&el, &new.classes);
            }
            if old.is_none() || was.attrs != new.attrs {
                apply_attrs(&el, &was.attrs, &new.attrs);
            }
            for (k, p) in &new.props {
                let before = old.as_ref().and_then(|o| o.props.iter().find(|(n, _)| n == k)).map(|(_, v)| v);
                if follows_children(k) && new.has_children {
                    AFTER_CHILDREN.with(|a| a.borrow_mut().push((el.clone(), k, p.clone())));
                } else {
                    apply_prop(&el, k, p, before);
                }
            }
            let style_names = if old.is_none() || was.styles != new.styles { apply_styles(&el, &style_names, &new.styles) } else { style_names };
            if old.is_none() || was.on != new.on {
                listeners = new.on.iter().map(|kind| listen(&el, kind, &key, &ctx)).collect();
            }
            Applied { own: new, style_names, listeners }
        });
        // Held by the reactive owner of the build: dropping the row disposes it.
        let _ = StoredValue::new_local(effect);
    }

    fn find(&self, key: &str) -> Option<Element> {
        if self.key == key {
            return self.el.borrow().clone();
        }
        self.kids.borrow().iter().find_map(|k| k.find(key))
    }

    fn slots(&self, out: &mut Vec<(Slot, Element)>) {
        if let (Some(s), Some(el)) = (self.own.with_untracked(|o| o.slot), self.el.borrow().clone()) {
            out.push((s, el));
        }
        for k in self.kids.borrow().iter() {
            k.slots(out);
        }
    }
}

fn listen(el: &Element, kind: &'static str, key: &str, ctx: &Rc<Ctx>) -> Listener {
    let ctx = Rc::downgrade(ctx);
    let key = key.to_owned();
    let f = move |ev: Event| {
        let Some(ctx) = ctx.upgrade() else { return };
        let d = ctx.dispatch.borrow().clone();
        let root = ctx.root.borrow().clone();
        if let Some(root) = root {
            d(kind, &key, &ev, &root);
        }
    };
    if needs_active_listener(kind) {
        Listener::new_active(el, kind, f)
    } else {
        Listener::new(el, kind, f)
    }
}

/// The one view that draws any node: the element, its text, then its children by identity.
fn node_view(id: u64) -> AnyView {
    let Some(n) = NODES.with(|m| m.borrow().get(&id).and_then(Weak::upgrade)) else { return ().into_any() };
    let text = n.text;
    let kids = n.kids_ids;
    let children = (
        move || text.try_get().flatten(),
        view! { <For each=move || kids.try_get().unwrap_or_default() key=|id| *id children=node_view /> },
    );
    let hook = {
        let n = Rc::downgrade(&n);
        move |el: Element| {
            if let Some(n) = n.upgrade() {
                n.attach(el);
            }
        }
    };
    let hook = directive(hook, ());
    if n.svg {
        svg_view(n.tag, hook, children)
    } else {
        leptos::html::custom(n.tag).add_any_attr(hook).child(children).into_any()
    }
}

/// An SVG element of the allow-list (`view::svg::ELEMENTS`) by its tag: each is its own Leptos element type.
fn svg_view<A, C>(tag: &'static str, a: A, c: C) -> AnyView
where
    A: leptos::attr::Attribute + 'static,
    C: IntoView + 'static,
{
    use leptos::svg as s;
    macro_rules! pick {
        ($($name:literal => $f:ident),* $(,)?) => {
            match tag {
                $($name => s::$f().add_any_attr(a).child(c).into_any(),)*
                _ => s::g().add_any_attr(a).child(c).into_any(),
            }
        };
    }
    pick! {
        "svg" => svg, "g" => g, "path" => path, "circle" => circle, "ellipse" => ellipse, "rect" => rect, "line" => line,
        "polyline" => polyline, "polygon" => polygon, "text" => text, "tspan" => tspan, "defs" => defs,
        "linearGradient" => linearGradient, "radialGradient" => radialGradient, "stop" => stop, "clipPath" => clipPath,
        "marker" => marker, "title" => title, "desc" => desc, "filter" => filter, "feDropShadow" => feDropShadow,
        "feGaussianBlur" => feGaussianBlur, "feOffset" => feOffset, "feFlood" => feFlood, "feMerge" => feMerge,
        "feMergeNode" => feMergeNode, "mask" => mask, "pattern" => pattern, "symbol" => symbol,
    }
}

// ── the API the page uses ───────────────────────────────────────────────────

pub struct NodeRec {
    pub el: Element,
    pub v: VNode,
    node: Rc<RNode>,
    /// The built view, which owns Leptos's render effects for this tree.
    state: Option<AnyViewState>,
}

impl Drop for NodeRec {
    fn drop(&mut self) {
        // The view's effects go first, then the nodes (their signals and owners).
        self.state = None;
    }
}

/// Build the element for `v` and its subtree. `root` is the control's root element (the origin of
/// the listeners' coordinates); pass `None` for the root itself.
pub fn create(_doc: &Document, v: &VNode, root: Option<&Element>, dispatch: &Dispatch) -> NodeRec {
    init_executor();
    let ctx = Rc::new(Ctx { dispatch: RefCell::new(dispatch.clone()), root: RefCell::new(root.cloned()) });
    let node = RNode::new(v, &ctx, None);
    let state = node.owner.with(|| node_view(node.id).build());
    flush();
    let el = node.el.borrow().clone().expect("the node layer built an element");
    NodeRec { el, v: v.clone(), node, state: Some(state) }
}

/// Make `rec` match `new`. Returns `true` when an element was replaced or added (the caller must
/// re-mount whatever was inside it, and the slots changed).
pub fn patch(doc: &Document, rec: &mut NodeRec, new: &VNode, root: Option<&Element>, dispatch: &Dispatch) -> bool {
    if !rec.node.same_kind(new) {
        let fresh = create(doc, new, root, dispatch);
        let _ = rec.el.replace_with_with_node_1(&fresh.el);
        *rec = fresh;
        return true;
    }
    *rec.node.ctx.dispatch.borrow_mut() = dispatch.clone();
    let mut gone = Vec::new();
    let made = rec.node.update(new, &mut gone);
    flush();
    drop(gone);
    rec.v = new.clone();
    made
}

/// The elements of a record tree that take child controls, by slot.
pub fn slots(rec: &NodeRec) -> Vec<(Slot, Element)> {
    let mut out = Vec::new();
    rec.node.slots(&mut out);
    out
}

/// The first element in the record tree whose key is `key`.
pub fn find_key(rec: &NodeRec, key: &str) -> Option<Element> {
    rec.node.find(key)
}
