// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The gestures that slide a TabControl's strip on the page** (2.0.90). A strip with more tabs than room scrolls by
//! itself (`overflow` on the strip, no scroll bar: there are no buttons and no slider); what the browser does not do for
//! a mouse is done here, so that the page slides the strip as the desktop does:
//!
//! * a **drag along the strip** with the mouse slides it with the pointer. The press that starts it is on a tab, and the
//!   click that ends it must not press that tab, so the click is swallowed. A touch screen and a trackpad scroll the
//!   strip themselves and are left alone;
//! * the **vertical wheel** slides a strip that runs sideways (a wheel only turns up and down; the layers' tab bar of the
//!   designer does the same). A notch of a mouse wheel glides there, a trackpad's small steps follow at once. A strip that
//!   runs down is scrolled by the browser; a sideways swipe is the browser's too.
//!
//! The strips to wire are the ones a render pass marked with `data-pr-slide`; the listeners live here, one set per strip,
//! and go with the strip when it leaves the page (nothing is leaked: there is no `Closure::forget` in the player).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::JsCast;
use web_sys::{Element, Event, PointerEvent, WheelEvent};

use super::h::{self, Listener};

/// A press that has moved this far along the strip is a slide and not a click (the desktop's click distance).
const SLIDE_AFTER: f64 = 6.0;
/// A wheel step of at least this many pixels, or any wheel counted in lines or pages, is a notch of a mouse wheel: it
/// glides. Smaller steps are a trackpad's, which follows the fingers at once.
const NOTCH: f64 = 40.0;
/// How long one wheel gesture lasts, in milliseconds: within it a notch adds to where the glide is heading.
const GESTURE_MS: f64 = 400.0;

/// What one strip remembers between events.
#[derive(Default)]
struct Slide {
    /// The mouse is down on the strip, and where it went down along the strip, and how far the strip had scrolled.
    down: Cell<bool>,
    pointer: Cell<i32>,
    press: Cell<f64>,
    at: Cell<i32>,
    /// The press has moved far enough to be a slide.
    dragging: Cell<bool>,
    /// The click that comes after a slide is not a press of the tab under it.
    swallow: Cell<bool>,
    /// Where the last notch of the wheel is heading, and when it came.
    heading: Cell<f64>,
    last_wheel: Cell<f64>,
}

/// A strip whose gestures are wired, with the listeners that carry them.
struct Wired {
    strip: Element,
    _listeners: Vec<Listener>,
}

thread_local! {
    /// The strips a render pass marked, to be wired when it is done.
    static PENDING: RefCell<Vec<Element>> = const { RefCell::new(Vec::new()) };
    static WIRED: RefCell<Vec<Wired>> = const { RefCell::new(Vec::new()) };
}

/// A render pass made `strip` with `data-pr-slide`: wire it when the pass is done.
pub fn request(strip: &Element) {
    PENDING.with(|p| p.borrow_mut().push(strip.clone()));
}

/// Wire the strips the pass marked that are not wired yet, and forget the ones that have left the page.
pub fn wire_requested() {
    WIRED.with(|w| w.borrow_mut().retain(|x| x.strip.is_connected()));
    let wanted = PENDING.with(|p| std::mem::take(&mut *p.borrow_mut()));
    for strip in wanted.into_iter().filter(|s| s.is_connected()) {
        if WIRED.with(|w| w.borrow().iter().any(|x| x.strip == strip)) {
            continue;
        }
        let wired = wire(&strip);
        WIRED.with(|w| w.borrow_mut().push(wired));
    }
}

fn wire(strip: &Element) -> Wired {
    let st = Rc::new(Slide::default());
    let mut listeners = Vec::new();
    {
        let (el, st) = (strip.clone(), st.clone());
        listeners.push(Listener::new_active(strip, "wheel", move |e| wheel(&el, &st, &e)));
    }
    {
        let (el, st) = (strip.clone(), st.clone());
        listeners.push(Listener::new(strip, "pointerdown", move |e| press(&el, &st, &e)));
    }
    {
        let (el, st) = (strip.clone(), st.clone());
        listeners.push(Listener::new(strip, "pointermove", move |e| drag(&el, &st, &e)));
    }
    for kind in ["pointerup", "pointercancel"] {
        let (el, st) = (strip.clone(), st.clone());
        listeners.push(Listener::new(strip, kind, move |e| release(&el, &st, &e)));
    }
    {
        let st = st.clone();
        listeners.push(Listener::new_capture(strip, "click", move |e| {
            if st.swallow.replace(false) {
                e.stop_propagation();
                e.prevent_default();
            }
        }));
    }
    Wired { strip: strip.clone(), _listeners: listeners }
}

/// Where a pointer is along the strip: across for a strip that runs sideways, down for one that runs down.
fn along(strip: &Element, p: &PointerEvent) -> f64 {
    if h::strip_is_vertical(strip) {
        p.client_y() as f64
    } else {
        p.client_x() as f64
    }
}

/// How far the strip has scrolled along itself.
fn scrolled(strip: &Element) -> i32 {
    if h::strip_is_vertical(strip) {
        strip.scroll_top()
    } else {
        strip.scroll_left()
    }
}

fn press(strip: &Element, st: &Slide, e: &Event) {
    st.swallow.set(false);
    let Some(p) = e.dyn_ref::<PointerEvent>() else {
        return;
    };
    // The mouse only: a finger and a trackpad scroll the strip themselves. A strip with nothing to slide is left alone.
    if p.pointer_type() != "mouse" || p.button() != 0 || h::strip_range(strip) == 0 {
        return;
    }
    st.down.set(true);
    st.dragging.set(false);
    st.pointer.set(p.pointer_id());
    st.press.set(along(strip, p));
    st.at.set(scrolled(strip));
}

fn drag(strip: &Element, st: &Slide, e: &Event) {
    let Some(p) = e.dyn_ref::<PointerEvent>() else {
        return;
    };
    if !st.down.get() || p.pointer_id() != st.pointer.get() {
        return;
    }
    let moved = along(strip, p) - st.press.get();
    if !st.dragging.get() {
        if moved.abs() < SLIDE_AFTER {
            return;
        }
        // From here the strip has the pointer, wherever it goes, until the button is let go.
        st.dragging.set(true);
        let _ = strip.set_pointer_capture(p.pointer_id());
        h::css(strip, "cursor", "grabbing");
    }
    let to = (st.at.get() as f64 - moved).clamp(0.0, h::strip_range(strip) as f64);
    h::slide_strip_to(strip, to, false);
}

fn release(strip: &Element, st: &Slide, e: &Event) {
    if st.dragging.get() {
        if let Some(p) = e.dyn_ref::<PointerEvent>() {
            let _ = strip.release_pointer_capture(p.pointer_id());
        }
        h::css_remove(strip, "cursor");
        // A button let go after a slide is followed by a click on the strip; a cancelled gesture is not.
        st.swallow.set(e.type_() == "pointerup");
    }
    st.down.set(false);
    st.dragging.set(false);
}

fn wheel(strip: &Element, st: &Slide, e: &Event) {
    let Some(w) = e.dyn_ref::<WheelEvent>() else {
        return;
    };
    let (dx, dy) = (w.delta_x(), w.delta_y());
    // Ctrl+wheel is the browser's zoom; a strip that runs down, and a plainly sideways swipe, scroll by themselves.
    if w.ctrl_key() || h::strip_is_vertical(strip) || dy == 0.0 || dx.abs() >= dy.abs() {
        return;
    }
    let range = h::strip_range(strip) as f64;
    if range <= 0.0 {
        return;
    }
    e.prevent_default();
    let unit = match w.delta_mode() {
        1 => 16.0,
        2 => strip.client_width() as f64,
        _ => 1.0,
    };
    let now = js_sys::Date::now();
    // Within one gesture a notch adds to where the last one was heading, not to where the glide has got to.
    let base = if now - st.last_wheel.get() < GESTURE_MS { st.heading.get() } else { strip.scroll_left() as f64 };
    let to = (base + dy * unit).clamp(0.0, range);
    st.last_wheel.set(now);
    st.heading.set(to);
    h::slide_strip_to(strip, to, w.delta_mode() != 0 || dy.abs() >= NOTCH);
}
