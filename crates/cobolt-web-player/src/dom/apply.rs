// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **From a [`VNode`] to a node, and from one `VNode` to the next** (WASM-016 R5, WASM-017 R14).
//!
//! A [`NodeRec`] is a real element together with the `VNode` it was made from, the records of its
//! children and the listeners attached to it. [`create`] builds the tree; [`patch`] makes it match a
//! newer `VNode` by changing only what differs, keeping an element (and so its focus, its caret and
//! its scroll position) whenever its tag and key are the same.
//!
//! * Everything goes in through `createElement`, `setAttribute`, `textContent`, the CSSOM and IDL
//!   properties. **No function in this module takes markup.**
//! * A listener is owned by the record of the node it listens on; dropping the record removes it.
//! * `value`, `checked` and `selected` are compared with the **live** element, not with the previous
//!   `VNode`: the user types into an input, so the page can disagree with the last thing the player
//!   wrote, and writing an equal value back would move the caret.

use std::rc::Rc;

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, Event};

use super::h::{self, Listener};
use crate::view::{Prop, Slot, VNode};

/// What a listener hands back: the DOM event type, the key of the node it listens on, the event, and
/// the control's root element.
pub type Dispatch = Rc<dyn Fn(&str, &str, &Event, &Element)>;

pub struct NodeRec {
    pub el: Element,
    pub v: VNode,
    pub children: Vec<NodeRec>,
    listeners: Vec<Listener>,
    /// The CSSOM declarations set on the element, so a removed one can be taken off.
    style_names: Vec<String>,
}

fn is_input_of(el: &Element, kinds: &[&str]) -> bool {
    if !el.tag_name().eq_ignore_ascii_case("input") {
        return false;
    }
    let t = el.get_attribute("type").unwrap_or_default().to_ascii_lowercase();
    kinds.iter().any(|k| *k == t)
}

fn set_prop(el: &Element, name: &str, value: &Prop) {
    let js: JsValue = match value {
        Prop::Str(s) => s.as_str().into(),
        Prop::Bool(b) => (*b).into(),
        Prop::Num(n) => (*n).into(),
    };
    let _ = Reflect::set(el, &name.into(), &js);
}

/// Apply one IDL property, comparing with the live element where the user can have changed it.
fn apply_prop(el: &Element, name: &str, new: &Prop, old: Option<&Prop>) {
    match name {
        "value" | "checked" | "selected" | "indeterminate" => {
            let live = Reflect::get(el, &name.into()).unwrap_or(JsValue::UNDEFINED);
            let same = match new {
                Prop::Bool(b) => live.as_bool() == Some(*b),
                Prop::Str(s) => {
                    let numeric = is_input_of(el, &["number", "range"]) || el.tag_name().eq_ignore_ascii_case("progress");
                    match live.as_string() {
                        Some(l) if numeric => match (l.trim().parse::<f64>(), s.trim().parse::<f64>()) {
                            (Ok(a), Ok(b)) => (a - b).abs() < 1e-9,
                            _ => l == *s,
                        },
                        Some(l) => l == *s,
                        None => false,
                    }
                }
                Prop::Num(n) => live.as_f64().map(|l| (l - n).abs() < 1e-9).unwrap_or(false),
            };
            if !same {
                set_prop(el, name, new);
            }
        }
        _ => {
            if old != Some(new) {
                set_prop(el, name, new);
            }
        }
    }
}

fn apply_class(el: &Element, classes: &[String]) {
    let want = classes.join(" ");
    if want.is_empty() {
        let _ = el.remove_attribute("class");
    } else if el.get_attribute("class").as_deref() != Some(want.as_str()) {
        let _ = el.set_attribute("class", &want);
    }
}

fn apply_attrs(el: &Element, old: &[(String, String)], new: &[(String, String)]) {
    for (k, _) in old {
        if !new.iter().any(|(n, _)| n == k) {
            let _ = el.remove_attribute(k);
        }
    }
    for (k, v) in new {
        if el.get_attribute(k).as_deref() != Some(v.as_str()) {
            let _ = el.set_attribute(k, v);
        }
    }
}

fn apply_styles(el: &Element, old: &[String], new: &[(String, String)]) -> Vec<String> {
    for name in old {
        if !new.iter().any(|(n, _)| n == name) {
            h::css_remove(el, name);
        }
    }
    let mut names = Vec::with_capacity(new.len());
    for (k, v) in new {
        h::css(el, k, v);
        names.push(k.clone());
    }
    names
}

fn make_element(doc: &Document, v: &VNode) -> Element {
    if v.svg {
        h::svg(doc, v.tag)
    } else {
        h::el(doc, v.tag, &[])
    }
}

fn attach(el: &Element, v: &VNode, root: &Element, dispatch: &Dispatch) -> Vec<Listener> {
    let mut out = Vec::with_capacity(v.on.len());
    for kind in &v.on {
        let d = dispatch.clone();
        let key = v.key.clone();
        let root = root.clone();
        let kind_s = (*kind).to_owned();
        let f = move |ev: Event| d(&kind_s, &key, &ev, &root);
        // These may need to cancel the default action: not passive.
        let l = if matches!(*kind, "wheel" | "dragover" | "drop" | "pointerdown" | "contextmenu" | "keydown" | "touchstart") {
            Listener::new_active(el, kind, f)
        } else {
            Listener::new(el, kind, f)
        };
        out.push(l);
    }
    out
}

/// Build the element for `v` and its subtree. `root` is the control's root element (the origin of
/// the listeners' coordinates); pass `None` for the root itself.
pub fn create(doc: &Document, v: &VNode, root: Option<&Element>, dispatch: &Dispatch) -> NodeRec {
    let el = make_element(doc, v);
    let root_el = root.cloned().unwrap_or_else(|| el.clone());
    apply_class(&el, &v.classes);
    apply_attrs(&el, &[], &v.attrs);
    for (k, p) in &v.props {
        if !(matches!(*k, "value" | "selectedIndex") && !v.children.is_empty()) {
            apply_prop(&el, k, p, None);
        }
    }
    let style_names = apply_styles(&el, &[], &v.styles);
    let mut children = Vec::with_capacity(v.children.len());
    if v.children.is_empty() {
        if let Some(t) = &v.text {
            el.set_text_content(Some(t));
        }
    } else {
        if let Some(t) = &v.text {
            let _ = el.append_child(&h::text(doc, t));
        }
        for c in &v.children {
            let rec = create(doc, c, Some(&root_el), dispatch);
            let _ = el.append_child(&rec.el);
            children.push(rec);
        }
    }
    let listeners = attach(&el, v, &root_el, dispatch);
    // Props that follow the children (a <select>'s `value` follows its options) come last.
    for (k, p) in &v.props {
        if matches!(*k, "value" | "selectedIndex") && !v.children.is_empty() {
            apply_prop(&el, k, p, None);
        }
    }
    NodeRec { el, v: v.clone(), children, listeners, style_names }
}

/// Make `rec` match `new`. Returns `true` when the element itself was replaced (the caller must
/// re-mount whatever was inside it, and the slots changed).
pub fn patch(doc: &Document, rec: &mut NodeRec, new: &VNode, root: Option<&Element>, dispatch: &Dispatch) -> bool {
    if rec.v.tag != new.tag || rec.v.svg != new.svg {
        let fresh = create(doc, new, root, dispatch);
        let _ = rec.el.replace_with_with_node_1(&fresh.el);
        *rec = fresh;
        return true;
    }
    let el = rec.el.clone();
    let root_el = root.cloned().unwrap_or_else(|| el.clone());
    if rec.v.classes != new.classes {
        apply_class(&el, &new.classes);
    }
    if rec.v.attrs != new.attrs {
        apply_attrs(&el, &rec.v.attrs, &new.attrs);
    }
    for (k, p) in &new.props {
        let old = rec.v.props.iter().find(|(n, _)| n == k).map(|(_, v)| v);
        apply_prop(&el, k, p, old);
    }
    if rec.v.styles != new.styles {
        rec.style_names = apply_styles(&el, &rec.style_names, &new.styles);
    }
    if rec.v.on != new.on {
        rec.listeners = attach(&el, new, &root_el, dispatch);
    }
    let mut replaced = false;
    if new.children.is_empty() {
        if !rec.children.is_empty() {
            rec.children.clear();
            h::clear(&el);
        }
        if rec.v.text != new.text || !rec.v.children.is_empty() {
            match &new.text {
                Some(t) => el.set_text_content(Some(t)),
                None if rec.v.text.is_some() => el.set_text_content(None),
                None => {}
            }
        }
    } else {
        for k in 0..new.children.len() {
            if k < rec.children.len() {
                if rec.children[k].v.key == new.children[k].key {
                    replaced |= patch(doc, &mut rec.children[k], &new.children[k], Some(&root_el), dispatch);
                } else {
                    // Another identity in this place: replace the child.
                    let fresh = create(doc, &new.children[k], Some(&root_el), dispatch);
                    let _ = rec.children[k].el.replace_with_with_node_1(&fresh.el);
                    rec.children[k] = fresh;
                    replaced = true;
                }
            } else {
                let fresh = create(doc, &new.children[k], Some(&root_el), dispatch);
                let _ = el.append_child(&fresh.el);
                rec.children.push(fresh);
                replaced = true;
            }
        }
        while rec.children.len() > new.children.len() {
            if let Some(gone) = rec.children.pop() {
                gone.el.remove();
            }
        }
    }
    rec.v = new.clone();
    replaced
}

/// The elements of a record tree that take child controls, by slot.
pub fn slots(rec: &NodeRec) -> Vec<(Slot, Element)> {
    let mut out = Vec::new();
    fn walk(r: &NodeRec, out: &mut Vec<(Slot, Element)>) {
        if let Some(s) = r.v.slot {
            out.push((s, r.el.clone()));
        }
        for c in &r.children {
            walk(c, out);
        }
    }
    walk(rec, &mut out);
    out
}

/// The first element in the record tree whose key is `key`.
pub fn find_key(rec: &NodeRec, key: &str) -> Option<Element> {
    if rec.v.key == key {
        return Some(rec.el.clone());
    }
    rec.children.iter().find_map(|c| find_key(c, key))
}
