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
    /// Registered for the capture phase: the target hears the event before what is under it does.
    capture: bool,
}

impl Listener {
    pub fn new(target: &EventTarget, kind: &str, f: impl FnMut(Event) + 'static) -> Listener {
        let cb = Closure::<dyn FnMut(Event)>::new(f);
        let _ = target.add_event_listener_with_callback(kind, cb.as_ref().unchecked_ref());
        Listener { target: target.clone(), kind: kind.to_owned(), cb, capture: false }
    }

    /// A non-passive listener (the handler may `preventDefault`) for events like `wheel` and `touchmove`.
    pub fn new_active(target: &EventTarget, kind: &str, f: impl FnMut(Event) + 'static) -> Listener {
        let cb = Closure::<dyn FnMut(Event)>::new(f);
        let opts = web_sys::AddEventListenerOptions::new();
        opts.set_passive(false);
        let _ = target.add_event_listener_with_callback_and_add_event_listener_options(kind, cb.as_ref().unchecked_ref(), &opts);
        Listener { target: target.clone(), kind: kind.to_owned(), cb, capture: false }
    }

    /// A capture-phase listener: it hears the event on its way DOWN to what is under it, so it can stop it from arriving
    /// (the click that ends a drag, which must not press the tab the drag started on).
    pub fn new_capture(target: &EventTarget, kind: &str, f: impl FnMut(Event) + 'static) -> Listener {
        let cb = Closure::<dyn FnMut(Event)>::new(f);
        let _ = target.add_event_listener_with_callback_and_bool(kind, cb.as_ref().unchecked_ref(), true);
        Listener { target: target.clone(), kind: kind.to_owned(), cb, capture: true }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        if self.capture {
            let _ = self.target.remove_event_listener_with_callback_and_bool(&self.kind, self.cb.as_ref().unchecked_ref(), true);
        } else {
            let _ = self.target.remove_event_listener_with_callback(&self.kind, self.cb.as_ref().unchecked_ref());
        }
    }
}

pub fn focus(e: &Element) {
    if let Some(h) = e.dyn_ref::<HtmlElement>() {
        let _ = h.focus();
    }
}

/// Is `strip` (a TabControl's strip) one that runs down the control (a Left or Right strip)?
pub fn strip_is_vertical(strip: &Element) -> bool {
    strip.get_attribute("aria-orientation").as_deref() == Some("vertical")
}

/// How far a strip scrolls along itself, in pixels: 0 when its tabs all fit.
pub fn strip_range(strip: &Element) -> i32 {
    let range = if strip_is_vertical(strip) { strip.scroll_height() - strip.client_height() } else { strip.scroll_width() - strip.client_width() };
    range.max(0)
}

/// Slide a strip to `to` pixels along itself, gliding there (`smooth`) or at once.
pub fn slide_strip_to(strip: &Element, to: f64, smooth: bool) {
    if smooth {
        let options = web_sys::ScrollToOptions::new();
        if strip_is_vertical(strip) {
            options.set_top(to);
        } else {
            options.set_left(to);
        }
        options.set_behavior(web_sys::ScrollBehavior::Smooth);
        strip.scroll_to_with_scroll_to_options(&options);
    } else if strip_is_vertical(strip) {
        strip.set_scroll_top(to.round() as i32);
    } else {
        strip.set_scroll_left(to.round() as i32);
    }
}

/// Slide the strip a tab sits in just far enough for the tab to be wholly on show, as the desktop's strip does
/// (`cobolt-forms` `tabstrip::reveal`, the same arithmetic): not at all when it already is; otherwise the tab whole and
/// a little more, so a sliver of the next tab shows beside it on the side it came from and says there are more (three
/// quarters of a tab's height). It glides there. The strip and nothing above it moves, so the page does not
/// (`scrollIntoView` would scroll every ancestor). The strip runs across (a top or bottom strip) or down (a side strip),
/// as its `aria-orientation` says, and is positioned, so it is the tab's offset parent and the tab's offsets are measured
/// from the strip's own edge.
pub fn reveal_in_strip(tab: &Element) {
    let (Some(t), Some(strip)) = (tab.dyn_ref::<HtmlElement>(), tab.parent_element()) else {
        return;
    };
    if t.offset_parent().as_ref() != Some(&strip) {
        return;
    }
    let (from, len, view, at) = if strip_is_vertical(&strip) {
        (t.offset_top(), t.offset_height(), strip.client_height(), strip.scroll_top())
    } else {
        (t.offset_left(), t.offset_width(), strip.client_width(), strip.scroll_left())
    };
    if from >= at && from + len <= at + view {
        return;
    }
    let peek = (t.offset_height() as f64 * 0.75) as i32;
    let to = if len >= view {
        from
    } else if from < at {
        (from - peek).max(from + len - view)
    } else {
        (from + len - view + peek).min(from)
    };
    slide_strip_to(&strip, to.clamp(0, strip_range(&strip)) as f64, true);
}
