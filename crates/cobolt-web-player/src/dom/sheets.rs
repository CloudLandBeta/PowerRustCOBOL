// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Stylesheets without a `<style>` element** (WASM-013: the page's CSP has no `'unsafe-inline'` for
//! styles).
//!
//! A *constructed* stylesheet (`new CSSStyleSheet()` + `replaceSync`) added to the shadow root's
//! `adoptedStyleSheets` is what the policy allows, and it is what the player uses. A browser without
//! constructed stylesheets gets an empty `<style>` element filled through `insertRule` — also CSSOM,
//! also allowed — one top-level statement at a time (`base_css::top_level_statements`).
//!
//! The browser's `web-sys` binding for these is behind an unstable flag; the calls are made with
//! `Reflect`, which keeps the build on stable and the surface small.

use js_sys::{Array, Function, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element};

fn call(target: &JsValue, name: &str, args: &[JsValue]) -> Result<JsValue, JsValue> {
    let f: Function = Reflect::get(target, &name.into())?.dyn_into().map_err(|_| JsValue::from_str("not a function"))?;
    let a = Array::new();
    for v in args {
        a.push(v);
    }
    f.apply(target, &a)
}

/// Where sheets are attached: a shadow root (or the document).
#[derive(Clone)]
pub struct Scope {
    root: JsValue,
    doc: Document,
    constructed: bool,
}

/// One stylesheet owned by the player.
pub struct Sheet {
    obj: Option<JsValue>,
    style_el: Option<Element>,
    scope: Scope,
}

impl Scope {
    pub fn new(root: JsValue, doc: Document) -> Scope {
        let ctor = Reflect::get(&js_sys::global(), &"CSSStyleSheet".into()).ok().filter(|c| c.is_function());
        let has_adopted = Reflect::has(&root, &"adoptedStyleSheets".into()).unwrap_or(false);
        let can_replace = ctor
            .as_ref()
            .and_then(|c| Reflect::get(c, &"prototype".into()).ok())
            .map(|p| Reflect::has(&p, &"replaceSync".into()).unwrap_or(false))
            .unwrap_or(false);
        Scope { root, doc, constructed: has_adopted && can_replace }
    }

    pub fn new_sheet(&self) -> Sheet {
        if self.constructed {
            let ctor: Function = Reflect::get(&js_sys::global(), &"CSSStyleSheet".into()).unwrap().unchecked_into();
            if let Ok(obj) = Reflect::construct(&ctor, &Array::new()) {
                return Sheet { obj: Some(obj), style_el: None, scope: self.clone() };
            }
        }
        // The fallback: an empty <style> (no text, so the CSP has nothing to refuse), ruled by insertRule.
        let el = super::h::el(&self.doc, "style", &[]);
        Sheet { obj: None, style_el: Some(el), scope: self.clone() }
    }

    /// Put the sheets, in order, on the scope: the later in the list, the later in the cascade.
    pub fn adopt(&self, sheets: &[&Sheet]) {
        if self.constructed {
            let arr = Array::new();
            for s in sheets {
                if let Some(o) = &s.obj {
                    arr.push(o);
                }
            }
            let _ = Reflect::set(&self.root, &"adoptedStyleSheets".into(), &arr);
        } else {
            // <style> elements: attach the missing ones in order.
            if let Ok(root_el) = self.root.clone().dyn_into::<web_sys::Node>() {
                for s in sheets {
                    if let Some(el) = &s.style_el {
                        let _ = root_el.append_child(el);
                    }
                }
            }
        }
    }
}

impl Sheet {
    /// Replace the sheet's content with `css` (one top-level rule, or a few that parse together).
    /// Returns the number of rules the browser kept (so a caller can tell that a block it does not
    /// support was dropped).
    pub fn set(&self, css: &str) -> usize {
        if let Some(o) = &self.obj {
            let _ = call(o, "replaceSync", &[css.into()]);
            return Reflect::get(o, &"cssRules".into()).ok().and_then(|r| Reflect::get(&r, &"length".into()).ok()).and_then(|n| n.as_f64()).unwrap_or(0.0) as usize;
        }
        if let Some(el) = &self.style_el {
            if let Some(sheet) = Reflect::get(el, &"sheet".into()).ok().filter(|s| !s.is_null() && !s.is_undefined()) {
                let len = || Reflect::get(&sheet, &"cssRules".into()).ok().and_then(|r| Reflect::get(&r, &"length".into()).ok()).and_then(|n| n.as_f64()).unwrap_or(0.0) as u32;
                while len() > 0 {
                    let _ = call(&sheet, "deleteRule", &[0.into()]);
                }
                // insertRule takes one statement: the base sheet opens with the layer order, then its block.
                for rule in crate::base_css::top_level_statements(css) {
                    let _ = call(&sheet, "insertRule", &[rule.into(), len().into()]);
                }
                return len() as usize;
            }
        }
        0
    }

    /// Set a form's stylesheet **scoped to the form's root** (WASM-007 R9): `@scope`, else CSS nesting,
    /// else — unsafe, or in a browser with neither — refused and left empty (`base_css::apply_form_sheet`).
    pub fn set_scoped(&self, form: &str, css: &str, origins: &[String]) -> Result<crate::base_css::Scoping, String> {
        crate::base_css::apply_form_sheet(form, css, origins, |t| self.set(t))
    }

    /// Take the sheet off the scope.
    pub fn dispose(&self) {
        if let Some(el) = &self.style_el {
            el.remove();
        }
        let _ = &self.scope;
    }
}
