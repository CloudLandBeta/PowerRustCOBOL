// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **What a [`VNode`](crate::view::VNode) says about one element**, applied to the element: its
//! classes, attributes, CSSOM declarations and IDL properties, as the Leptos node layer (`reactive`)
//! applies them.
//!
//! * Everything goes in through `setAttribute`, the CSSOM and IDL properties. **No function in this
//!   module takes markup.**
//! * `value`, `checked` and `selected` are compared with the **live** element, not with the previous
//!   `VNode`: the user types into an input, so the page can disagree with the last thing the player
//!   wrote, and writing an equal value back would move the caret.

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use web_sys::Element;

use super::h;
use crate::view::Prop;

thread_local! {
    /// Elements made with the `autofocus` attribute during the render pass in progress. A browser
    /// autofocuses only at page load, so an element a redraw brings in (the next screen of a sign-in
    /// form) is focused by [`focus_requested`] when the pass is done.
    static FOCUS: std::cell::RefCell<Vec<Element>> = const { std::cell::RefCell::new(Vec::new()) };

    /// Elements a render pass gave a `data-pr-reveal` attribute it did not have: the selected tab of a TabControl,
    /// at the moment it becomes the selected one. [`reveal_requested`] scrolls each into view in its own strip when
    /// the pass is done.
    static REVEAL: std::cell::RefCell<Vec<Element>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Give the keyboard focus to the last element a render pass made with `autofocus` (if it is still on
/// the page) and forget the others.
pub fn focus_requested() {
    let wanted = FOCUS.with(|f| std::mem::take(&mut *f.borrow_mut()));
    if let Some(el) = wanted.into_iter().rev().find(|e| e.is_connected()) {
        h::focus(&el);
    }
}

fn request_focus(el: &Element) {
    FOCUS.with(|f| f.borrow_mut().push(el.clone()));
}

/// Scroll each element the render pass marked with `data-pr-reveal` just far enough to be on show in the strip it sits
/// in (the strip, never the page), and forget them. A strip that has more tabs than room scrolls, and a tab that has
/// just become the selected one has to be on show: a click, an arrow key or a program's write of `SelectedTab`.
pub fn reveal_requested() {
    let wanted = REVEAL.with(|r| std::mem::take(&mut *r.borrow_mut()));
    for el in wanted.into_iter().filter(|e| e.is_connected()) {
        h::reveal_in_strip(&el);
    }
}

fn request_reveal(el: &Element) {
    REVEAL.with(|r| r.borrow_mut().push(el.clone()));
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

/// A property that follows the element's children (a `<select>`'s `value` follows its options), so it
/// is applied after them.
pub fn follows_children(name: &str) -> bool {
    matches!(name, "value" | "selectedIndex")
}

/// Apply one IDL property, comparing with the live element where the user can have changed it.
pub fn apply_prop(el: &Element, name: &str, new: &Prop, old: Option<&Prop>) {
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

/// An IDL property the previous view set and the new one leaves out goes back to its default: a
/// boolean (`disabled`, `readOnly`, `hidden`…) to `false`. A view writes most such flags only when they
/// are on (`button` writes `disabled` only for a disabled button), so without this a control enabled
/// again after `Disable()` kept the `disabled` its element was given (P-8).
pub fn reset_prop(el: &Element, name: &str, old: &Prop) {
    if let Prop::Bool(true) = old {
        set_prop(el, name, &Prop::Bool(false));
    }
}

pub fn apply_class(el: &Element, classes: &[String]) {
    let want = classes.join(" ");
    if want.is_empty() {
        let _ = el.remove_attribute("class");
    } else if el.get_attribute("class").as_deref() != Some(want.as_str()) {
        let _ = el.set_attribute("class", &want);
    }
}

pub fn apply_attrs(el: &Element, old: &[(String, String)], new: &[(String, String)]) {
    for (k, _) in old {
        if !new.iter().any(|(n, _)| n == k) {
            let _ = el.remove_attribute(k);
        }
    }
    for (k, v) in new {
        if el.get_attribute(k).as_deref() != Some(v.as_str()) {
            if k == "autofocus" {
                request_focus(el);
            }
            if k == "data-pr-reveal" {
                request_reveal(el);
            }
            if k == "src" && needs_cors_mode(el, v) {
                // Set before `src`: the request mode is decided when the load starts.
                let _ = el.set_attribute("crossorigin", "anonymous");
            }
            let _ = el.set_attribute(k, v);
        }
    }
}

/// On a cross-origin isolated page (`COEP: require-corp`) a picture from another origin is
/// blocked unless the server marks it with a CORP header or the request is made in CORS mode
/// and the server answers it. Map tiles and `ImagePath` pictures are such requests, so on an
/// isolated page they are asked for in CORS mode (a measured finding: tile.openstreetmap.org
/// sends `Access-Control-Allow-Origin: *`, and the same tile without `crossorigin` is refused
/// with `NotSameOriginAfterDefaultedToSameOriginByCoep`). On a page that is not isolated
/// nothing changes: a server that sends no CORS header must keep working there.
fn needs_cors_mode(el: &Element, src: &str) -> bool {
    if !el.tag_name().eq_ignore_ascii_case("img") {
        return false;
    }
    let lower = src.trim_start().to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("//")) {
        return false;
    }
    js_sys::Reflect::get(&js_sys::global(), &"crossOriginIsolated".into()).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Set the declarations of `new`, take off those of `old` that `new` lacks; answers the names now set.
pub fn apply_styles(el: &Element, old: &[String], new: &[(String, String)]) -> Vec<String> {
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

/// The listeners a node asks for (`VNode::on`). These may need to cancel the default action, so they
/// are not passive.
pub fn needs_active_listener(kind: &str) -> bool {
    matches!(kind, "wheel" | "dragover" | "drop" | "pointerdown" | "contextmenu" | "keydown" | "touchstart")
}
