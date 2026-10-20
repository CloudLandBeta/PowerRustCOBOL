// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **reCAPTCHA v3, the browser half** (WASM-009 R4).
//!
//! Where a site key is configured, the Authentication control asks for a token for each protected
//! action (sign-in, sign-up, forgot-password) before it sends the request. This module loads
//! Google's script once (`https://www.google.com/recaptcha/api.js?render=<site key>`, a URL the
//! page's Content-Security-Policy and its Trusted Types policy allow only when a key is set), calls
//! `grecaptcha.execute(siteKey, {action})` and hands the token back to the host. The secret key is
//! the backend's; nothing here has it.
//!
//! A token that does not arrive within [`LIMIT_MS`] — a blocked script, an ad blocker, no network —
//! is answered as "no token", and the host then sends no request (the form is never left busy).
//!
//! The closures given to the script are held in [`Pending`] and dropped when the next token is asked
//! for with none outstanding, never from inside their own call.

use std::any::Any;
use std::collections::BTreeMap;
use std::rc::Rc;

use js_sys::{Function, Object, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::Element;

use super::h::{self, Listener};
use crate::api::{Inbox, Inner, Timeout};

/// How long a token may take.
pub const LIMIT_MS: u32 = 15_000;

/// The tokens being obtained.
#[derive(Default)]
pub struct Pending {
    /// The time limit of each, by call id. Present = still waiting; dropping one cancels it.
    timers: BTreeMap<u32, Timeout>,
    /// Calls waiting for the script to load: `(id, site key, action)`.
    waiting: Vec<(u32, String, &'static str)>,
    /// The script element and its listeners, while it loads.
    script: Option<(Element, Vec<Listener>)>,
    /// The closures the script holds, kept alive until it is safe to drop them.
    held: Vec<Box<dyn Any>>,
}

fn api() -> JsValue {
    Reflect::get(&js_sys::global(), &"grecaptcha".into()).unwrap_or(JsValue::UNDEFINED)
}

/// Is `grecaptcha` there, with `ready` and `execute`?
fn api_present() -> bool {
    let g = api();
    g.is_object() && Reflect::get(&g, &"execute".into()).map(|f| f.is_function()).unwrap_or(false) && Reflect::get(&g, &"ready".into()).map(|f| f.is_function()).unwrap_or(false)
}

impl Inner {
    /// Obtain a token for `action` and answer the host (`Host::auth_captcha`) under `id`.
    pub(crate) fn run_captcha(self: &Rc<Self>, id: u32, site_key: &str, action: &'static str) {
        {
            let mut st = self.captcha.borrow_mut();
            if st.timers.is_empty() {
                // Nothing is being waited for, so no closure of the script is running: drop them.
                st.held.clear();
            }
            let weak = Rc::downgrade(self);
            st.timers.insert(
                id,
                Timeout::new(LIMIT_MS, move || {
                    if let Some(i) = weak.upgrade() {
                        i.finish_captcha(id, None);
                    }
                }),
            );
        }
        if site_key.trim().is_empty() {
            self.finish_captcha(id, None);
        } else if api_present() {
            self.captcha_execute(id, site_key, action);
        } else {
            self.captcha_load(id, site_key, action);
        }
    }

    /// The first answer for `id` counts; the host is told through the inbox, never from inside the script's callback.
    fn finish_captcha(self: &Rc<Self>, id: u32, token: Option<String>) {
        let was_waiting = self.captcha.borrow_mut().timers.remove(&id).is_some();
        if !was_waiting {
            return;
        }
        self.push(Inbox::Raw(Box::new(move |inner| inner.host.borrow_mut().auth_captcha(id, token))));
    }

    fn captcha_load(self: &Rc<Self>, id: u32, site_key: &str, action: &'static str) {
        {
            let mut st = self.captcha.borrow_mut();
            st.waiting.push((id, site_key.to_owned(), action));
            if st.script.is_some() {
                return; // already loading: this call goes with it
            }
        }
        let doc = h::document();
        let script = h::el(&doc, "script", &[]);
        let weak = Rc::downgrade(self);
        let w = weak.clone();
        let on_load = Listener::new(&script, "load", move |_| {
            if let Some(i) = w.upgrade() {
                i.captcha_script_done(true);
            }
        });
        let w = weak;
        let on_error = Listener::new(&script, "error", move |_| {
            if let Some(i) = w.upgrade() {
                i.captcha_script_done(false);
            }
        });
        let url = format!("https://www.google.com/recaptcha/api.js?render={}", js_sys::encode_uri_component(site_key));
        // `src` last: the request starts when it is set. The Trusted Types `default` policy of the page
        // lets exactly this origin through when a site key is configured.
        let _ = script.set_attribute("async", "");
        let _ = script.set_attribute("src", &url);
        if let Some(head) = doc.head() {
            let _ = head.append_child(&script);
        }
        self.captcha.borrow_mut().script = Some((script, vec![on_load, on_error]));
    }

    fn captcha_script_done(self: &Rc<Self>, loaded: bool) {
        let waiting = {
            let mut st = self.captcha.borrow_mut();
            if let Some((el, listeners)) = st.script.take() {
                // Keep the element and the listeners only while it loads. A script that failed is removed, so
                // that the next token asked for tries again.
                if !loaded {
                    el.remove();
                }
                drop(listeners);
            }
            std::mem::take(&mut st.waiting)
        };
        for (id, key, action) in waiting {
            if loaded && api_present() {
                self.captcha_execute(id, &key, action);
            } else {
                self.finish_captcha(id, None);
            }
        }
    }

    fn captcha_execute(self: &Rc<Self>, id: u32, site_key: &str, action: &'static str) {
        let g = api();
        let key = site_key.to_owned();
        let weak = Rc::downgrade(self);
        let g_in = g.clone();
        let on_ready = Closure::<dyn FnMut()>::new(move || {
            let Some(inner) = weak.upgrade() else { return };
            let exec = Reflect::get(&g_in, &"execute".into()).ok().and_then(|f| f.dyn_into::<Function>().ok());
            let opts = Object::new();
            let _ = Reflect::set(&opts, &"action".into(), &action.into());
            let promise = exec.and_then(|f| f.call2(&g_in, &key.as_str().into(), &opts).ok());
            let then = promise.as_ref().and_then(|p| Reflect::get(p, &"then".into()).ok()).and_then(|f| f.dyn_into::<Function>().ok());
            let (Some(p), Some(then)) = (promise, then) else {
                inner.finish_captcha(id, None);
                return;
            };
            let w_ok = Rc::downgrade(&inner);
            let w_err = w_ok.clone();
            let ok = Closure::<dyn FnMut(JsValue)>::new(move |token: JsValue| {
                if let Some(i) = w_ok.upgrade() {
                    i.finish_captcha(id, token.as_string().filter(|t| !t.is_empty()));
                }
            });
            let err = Closure::<dyn FnMut(JsValue)>::new(move |_| {
                if let Some(i) = w_err.upgrade() {
                    i.finish_captcha(id, None);
                }
            });
            let _ = then.call2(&p, ok.as_ref(), err.as_ref());
            inner.captcha.borrow_mut().held.push(Box::new(ok));
            inner.captcha.borrow_mut().held.push(Box::new(err));
        });
        let ready = Reflect::get(&g, &"ready".into()).ok().and_then(|f| f.dyn_into::<Function>().ok());
        match ready {
            Some(f) if f.call1(&g, on_ready.as_ref()).is_ok() => {
                self.captcha.borrow_mut().held.push(Box::new(on_ready));
            }
            _ => self.finish_captcha(id, None),
        }
    }
}
