// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The DOM helpers the player uses for the page's own elements: nodes are created with `createElement` and
//! filled with `textContent`/`setAttribute`/CSSOM — **never** an HTML string (WASM-013 R5). There is no
//! function here that takes markup, and a test greps the sources for the sinks that do.
//!
//! Styles are set through the CSSOM (`style.setProperty`), the one way the page's CSP allows: it has
//! no `'unsafe-inline'` for styles, so a `style` attribute would be refused.

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, Event, EventTarget, HtmlElement};

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

pub fn attr(e: &Element, name: &str, value: &str) {
    let _ = e.set_attribute(name, value);
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

pub fn focus(e: &Element) {
    if let Some(h) = e.dyn_ref::<HtmlElement>() {
        let _ = h.focus();
    }
}
