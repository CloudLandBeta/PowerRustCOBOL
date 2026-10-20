// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The DOM builder the player uses: nodes are created with `createElement`/`createElementNS` and
//! filled with `textContent`/`setAttribute`/CSSOM — **never** an HTML string (WASM-013 R5). There is no
//! function here that takes markup, and a test greps the sources for the sinks that do.
//!
//! Styles are set through the CSSOM (`style.setProperty`), the one way the page's CSP allows: it has
//! no `'unsafe-inline'` for styles, so a `style` attribute would be refused.

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, Event, EventTarget, HtmlElement};

pub const SVG_NS: &str = "http://www.w3.org/2000/svg";

pub fn document() -> Document {
    web_sys::window().and_then(|w| w.document()).expect("a document")
}

/// `<tag class="a b">`
pub fn el(doc: &Document, tag: &str, classes: &[&str]) -> Element {
    let e = doc.create_element(tag).expect("a valid tag name");
    if !classes.is_empty() {
        let _ = e.set_attribute("class", &classes.join(" "));
    }
    e
}

pub fn svg(doc: &Document, tag: &str) -> Element {
    doc.create_element_ns(Some(SVG_NS), tag).expect("a valid svg tag")
}

pub fn text(doc: &Document, s: &str) -> web_sys::Text {
    doc.create_text_node(s)
}

pub fn set_text(e: &Element, s: &str) {
    e.set_text_content(Some(s));
}

pub fn attr(e: &Element, name: &str, value: &str) {
    let _ = e.set_attribute(name, value);
}

pub fn remove_attr(e: &Element, name: &str) {
    let _ = e.remove_attribute(name);
}

pub fn set_bool_attr(e: &Element, name: &str, on: bool) {
    if on {
        let _ = e.set_attribute(name, "");
    } else {
        let _ = e.remove_attribute(name);
    }
}

/// Set one CSS declaration through the CSSOM.
pub fn css(e: &Element, prop: &str, value: &str) {
    if let Some(h) = e.dyn_ref::<HtmlElement>() {
        let _ = h.style().set_property(prop, value);
    } else if let Some(s) = Reflect::get(e, &"style".into()).ok().filter(|s| !s.is_undefined()) {
        if let Ok(f) = Reflect::get(&s, &"setProperty".into()).and_then(|f| f.dyn_into::<js_sys::Function>().map_err(|_| JsValue::UNDEFINED)) {
            let _ = f.call2(&s, &prop.into(), &value.into());
        }
    }
}

pub fn css_remove(e: &Element, prop: &str) {
    if let Some(h) = e.dyn_ref::<HtmlElement>() {
        let _ = h.style().remove_property(prop);
    }
}

/// Remove every declaration the property layer set before and set the new ones.
pub fn css_all(e: &Element, previous: &[String], next: &[(String, String)]) -> Vec<String> {
    for p in previous {
        css_remove(e, p);
    }
    let mut names = Vec::with_capacity(next.len());
    for (k, v) in next {
        css(e, k, v);
        names.push(k.clone());
    }
    names
}

pub fn clear(e: &Element) {
    while let Some(c) = e.first_child() {
        let _ = e.remove_child(&c);
    }
}

pub fn append(parent: &Element, child: &Element) {
    let _ = parent.append_child(child);
}

/// A listener that removes itself when dropped. The closure is owned here, so nothing is leaked and
/// nothing outlives the widget (WASM-017 R14: there is no `Closure::forget` in the player).
pub struct Listener {
    target: EventTarget,
    kind: String,
    cb: Closure<dyn FnMut(Event)>,
}

impl Listener {
    pub fn new(target: &EventTarget, kind: &str, f: impl FnMut(Event) + 'static) -> Listener {
        let cb = Closure::<dyn FnMut(Event)>::new(f);
        let _ = target.add_event_listener_with_callback(kind, cb.as_ref().unchecked_ref());
        Listener { target: target.clone(), kind: kind.to_owned(), cb }
    }

    /// A non-passive listener (the handler may `preventDefault`) for events like `wheel` and `touchmove`.
    pub fn new_active(target: &EventTarget, kind: &str, f: impl FnMut(Event) + 'static) -> Listener {
        let cb = Closure::<dyn FnMut(Event)>::new(f);
        let opts = web_sys::AddEventListenerOptions::new();
        opts.set_passive(false);
        let _ = target.add_event_listener_with_callback_and_add_event_listener_options(kind, cb.as_ref().unchecked_ref(), &opts);
        Listener { target: target.clone(), kind: kind.to_owned(), cb }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self.target.remove_event_listener_with_callback(&self.kind, self.cb.as_ref().unchecked_ref());
    }
}

pub fn modifiers(ev: &Event) -> String {
    let get = |k: &str| Reflect::get(ev, &k.into()).ok().and_then(|v| v.as_bool()).unwrap_or(false);
    let mut v = Vec::new();
    if get("ctrlKey") {
        v.push("Ctrl");
    }
    if get("metaKey") {
        v.push("Meta");
    }
    if get("shiftKey") {
        v.push("Shift");
    }
    if get("altKey") {
        v.push("Alt");
    }
    v.join(",")
}

pub fn num_prop(ev: &Event, k: &str) -> f64 {
    Reflect::get(ev, &k.into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

pub fn str_prop(ev: &Event, k: &str) -> String {
    Reflect::get(ev, &k.into()).ok().and_then(|v| v.as_string()).unwrap_or_default()
}

/// The mouse position inside `target`'s box, in CSS pixels.
pub fn local_xy(ev: &Event, target: &Element) -> (i32, i32) {
    let r = target.get_bounding_client_rect();
    ((num_prop(ev, "clientX") - r.left()).round() as i32, (num_prop(ev, "clientY") - r.top()).round() as i32)
}

pub fn focus(e: &Element) {
    if let Some(h) = e.dyn_ref::<HtmlElement>() {
        let _ = h.focus();
    }
}

/// `true` when a node is attached to a document.
pub fn connected(e: &Element) -> bool {
    e.is_connected()
}
