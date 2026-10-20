// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A DOM event, reduced to the plain data `view::interact` reasons about.

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, Event};

use crate::view::interact::Payload;

fn get(obj: &JsValue, name: &str) -> JsValue {
    Reflect::get(obj, &name.into()).unwrap_or(JsValue::UNDEFINED)
}

fn num(obj: &JsValue, name: &str) -> f64 {
    get(obj, name).as_f64().unwrap_or(0.0)
}

fn flag(obj: &JsValue, name: &str) -> bool {
    get(obj, name).as_bool().unwrap_or(false)
}

fn text(obj: &JsValue, name: &str) -> String {
    get(obj, name).as_string().unwrap_or_default()
}

fn is_form_field(e: &Element) -> bool {
    matches!(e.tag_name().to_ascii_lowercase().as_str(), "input" | "textarea" | "select")
}

/// The nearest attribute `name` on `from` or an ancestor, up to and including `stop`.
fn climb(from: &Element, stop: &Element, name: &str) -> Option<String> {
    let mut cur = Some(from.clone());
    while let Some(e) = cur {
        if let Some(v) = e.get_attribute(name) {
            return Some(v);
        }
        if e == *stop {
            break;
        }
        cur = e.parent_element();
    }
    None
}

fn file_list(files: &JsValue) -> Vec<(String, u64)> {
    let n = num(files, "length") as u32;
    let mut out = Vec::new();
    for k in 0..n {
        if let Ok(f) = Reflect::get(files, &k.into()) {
            if f.is_object() {
                out.push((text(&f, "name"), num(&f, "size") as u64));
            }
        }
    }
    out
}

/// Reduce `ev`, heard on node `key` of a control whose root element is `root`, to a [`Payload`].
pub fn from_event(kind: &str, key: &str, ev: &Event, root: &Element) -> Payload {
    let mut p = Payload::new(kind, key);
    let evj: &JsValue = ev.as_ref();
    let target = ev.target().and_then(|t| t.dyn_into::<Element>().ok());
    let current = ev.current_target().and_then(|t| t.dyn_into::<Element>().ok());
    // The field the event is about: the listener's own node when it is a field, else the target.
    let field = current.as_ref().filter(|c| is_form_field(c)).or(target.as_ref().filter(|t| is_form_field(t)));
    if let Some(f) = field {
        let fj: &JsValue = f.as_ref();
        p.value = text(fj, "value");
        p.checked = flag(fj, "checked");
    }
    if let Some(t) = &target {
        let stop = current.as_ref().unwrap_or(root);
        p.index = climb(t, stop, "data-i").and_then(|v| v.parse().ok());
        p.row = climb(t, stop, "data-r").and_then(|v| v.parse().ok());
        p.col = climb(t, stop, "data-c").and_then(|v| v.parse().ok());
        p.id = climb(t, stop, "data-id");
        p.part = climb(t, stop, "data-part");
    }
    if kind == "submit" {
        // The form's named fields, read now and held only for the length of this event.
        let form: JsValue = current.as_ref().map(|c| c.clone().into()).unwrap_or(JsValue::UNDEFINED);
        let elements = get(&form, "elements");
        if elements.is_object() {
            let n = num(&elements, "length") as u32;
            for k in 0..n {
                if let Ok(el) = Reflect::get(&elements, &k.into()) {
                    if el.is_object() {
                        let name = text(&el, "name");
                        if !name.is_empty() && !text(&el, "type").eq_ignore_ascii_case("submit") && !text(&el, "type").eq_ignore_ascii_case("button") {
                            p.fields.0.push((name, text(&el, "value")));
                        }
                    }
                }
            }
        }
    }
    p.key_name = text(evj, "key");
    p.button = num(evj, "button") as i16;
    p.buttons = num(evj, "buttons") as u16;
    p.ctrl = flag(evj, "ctrlKey");
    p.shift = flag(evj, "shiftKey");
    p.alt = flag(evj, "altKey");
    p.meta = flag(evj, "metaKey");
    p.delta = num(evj, "deltaY");
    let rect = root.get_bounding_client_rect();
    p.w = rect.width();
    p.h = rect.height();
    p.x = num(evj, "clientX") - rect.left();
    p.y = num(evj, "clientY") - rect.top();
    if let Some(c) = &current {
        let cj: &JsValue = c.as_ref();
        p.scroll_top = num(cj, "scrollTop");
        p.client_height = num(cj, "clientHeight");
        let files = get(cj, "files");
        if files.is_object() {
            p.files = file_list(&files);
        }
    }
    let dt = get(evj, "dataTransfer");
    if dt.is_object() {
        let files = get(&dt, "files");
        if files.is_object() {
            p.files = file_list(&files);
        }
    }
    p
}
