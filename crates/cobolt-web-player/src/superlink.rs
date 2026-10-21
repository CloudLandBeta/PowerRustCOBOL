// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **`super::` across windows** (WASM-011 R11): the browser half.
//!
//! A standalone form runs in a window of its own, with its own instance of the program. When the
//! program there says `super::Prop` it means the form that opened the window, which lives in another
//! window of the same origin. The two windows talk over a **`BroadcastChannel` that only they share**: its
//! name carries a 128-bit random token the opener makes and puts in the new window's address
//! (`pr-super=`), so no other page of the origin can join it by guessing.
//!
//! What goes over it (JSON text, nothing else):
//!
//! | message | from | meaning |
//! |---|---|---|
//! | `{"t":"hello"}` | the new window | I am here, tell me your state |
//! | `{"t":"state","props":{…}}` | the opener | the opener form's properties, now (and after every change) |
//! | `{"t":"set","prop":…,"value":…}` | the new window | `SET super::Prop = value` |
//! | `{"t":"call","name":…,"args":[…]}` | the new window | `CALL super::Method(args)` |
//! | `{"t":"closed"}` | the opener | I am going away |
//!
//! The program's calls are synchronous and a round trip to another window is not, so the new window keeps
//! a **replica** of the opener form's properties (`host::RemoteOpener`): a read is answered from it, a write
//! is applied to it at once and forwarded. A read therefore sees the opener's state as of its last message —
//! the opener sends one after every change it makes, so the lag is one message — and a `CALL` is forwarded
//! without waiting for its answer. When the opener closes (or never answers the first `hello`), `super::`
//! fails the way it does for a form nothing opened, which the program handles with `ON EXCEPTION`.

use std::collections::BTreeMap;
use std::rc::Rc;

use js_sys::{Function, Promise};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{BroadcastChannel, MessageEvent};

use crate::api::{Inbox, Inner};
use crate::host::Effect;

/// The channel's name for a token.
fn channel_name(token: &str) -> String {
    format!("pr-super-{token}")
}

/// A token of 128 random bits, as hexadecimal text. The browser's cryptographic generator, or, where a page
/// has none (it has: this is for a headless test), the clock and the generator `Math.random`.
fn new_token() -> String {
    let bytes = js_sys::Uint8Array::new_with_length(16);
    let crypto = js_sys::Reflect::get(&js_sys::global(), &"crypto".into()).ok().filter(|c| c.is_object());
    let filled = crypto
        .as_ref()
        .and_then(|c| js_sys::Reflect::get(c, &"getRandomValues".into()).ok())
        .and_then(|f| f.dyn_into::<Function>().ok())
        .map(|f| f.call1(crypto.as_ref().unwrap(), &bytes).is_ok())
        .unwrap_or(false);
    if !filled {
        for i in 0..16 {
            bytes.set_index(i, (js_sys::Math::random() * 256.0) as u8);
        }
    }
    bytes.to_vec().iter().map(|b| format!("{b:02x}")).collect()
}

/// A form this window opened that has a private channel back to it.
struct OpenerLink {
    inst: u32,
    chan: BroadcastChannel,
    /// What the form last published, to publish only a change.
    last: String,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
}

/// This window's channel to the window that opened it.
struct ChildLink {
    chan: BroadcastChannel,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
}

#[derive(Default)]
pub(crate) struct Links {
    opened: Vec<OpenerLink>,
    child: Option<ChildLink>,
    /// The `pagehide` listener (kept here so the page can free it): a window that goes away tells the windows it opened.
    hide: Option<Closure<dyn FnMut()>>,
}

fn parse(ev: &MessageEvent) -> Option<serde_json::Value> {
    serde_json::from_str(&ev.data().as_string()?).ok()
}

fn post(chan: &BroadcastChannel, v: &serde_json::Value) {
    let _ = chan.post_message(&v.to_string().into());
}

impl Drop for Links {
    /// The player is going away: the page keeps no listener that would call into it, and no channel stays open.
    fn drop(&mut self) {
        if let (Some(f), Some(w)) = (&self.hide, web_sys::window()) {
            let _ = w.remove_event_listener_with_callback("pagehide", f.as_ref().unchecked_ref());
        }
        for l in &self.opened {
            l.chan.close();
        }
        if let Some(c) = &self.child {
            c.chan.close();
        }
    }
}

impl Inner {
    /// A window is about to be opened for the form `opener` of this window: make the private channel and
    /// answer its token for the new window's address.
    pub(crate) fn super_open(self: &Rc<Self>, opener: u32) -> String {
        let token = new_token();
        let Ok(chan) = BroadcastChannel::new(&channel_name(&token)) else { return token };
        let weak = Rc::downgrade(self);
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |ev: MessageEvent| {
            let (Some(inner), Some(m)) = (weak.upgrade(), parse(&ev)) else { return };
            inner.push(Inbox::Raw(Box::new(move |inner: &Rc<Inner>| inner.super_from_child(opener, &m))));
        });
        chan.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        let mut links = self.superlink.borrow_mut();
        links.opened.push(OpenerLink { inst: opener, chan, last: String::new(), _on_message: on_message });
        if links.hide.is_none() {
            // A window that goes away tells the windows it opened: `super::` there fails from then on.
            let weak = Rc::downgrade(self);
            let on_hide = Closure::<dyn FnMut()>::new(move || {
                if let Some(inner) = weak.upgrade() {
                    inner.super_close_all();
                }
            });
            if let Some(w) = web_sys::window() {
                let _ = w.add_event_listener_with_callback("pagehide", on_hide.as_ref().unchecked_ref());
            }
            links.hide = Some(on_hide);
        }
        token
    }

    /// A message of the window this one opened.
    fn super_from_child(self: &Rc<Self>, inst: u32, m: &serde_json::Value) {
        match m["t"].as_str() {
            Some("hello") => {
                // Publish regardless of what was sent before: the newcomer has nothing.
                let mut links = self.superlink.borrow_mut();
                for l in links.opened.iter_mut().filter(|l| l.inst == inst) {
                    l.last.clear();
                }
                drop(links);
                self.super_publish();
            }
            Some("set") => {
                if let (Some(p), Some(v)) = (m["prop"].as_str(), m["value"].as_str()) {
                    let _ = self.host.borrow_mut().apply_super_set(inst, p, v);
                }
            }
            Some("call") => {
                if let Some(n) = m["name"].as_str() {
                    let args: Vec<String> = m["args"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default();
                    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
                    let r = self.host.borrow_mut().apply_super_call(inst, n, &refs);
                    if let Err(e) = r {
                        web_sys::console::warn_1(&format!("[pr] a window this one opened called `{n}` on it: {e}").into());
                    }
                }
            }
            _ => {}
        }
    }

    /// Send the windows this one opened the properties of the forms that opened them, when they have changed.
    pub(crate) fn super_publish(self: &Rc<Self>) {
        let mut links = self.superlink.borrow_mut();
        if links.opened.is_empty() {
            return;
        }
        let Ok(host) = self.host.try_borrow() else { return };
        for l in links.opened.iter_mut() {
            let Some(props) = host.form_props(l.inst) else { continue };
            let text = serde_json::json!({"t": "state", "props": props}).to_string();
            if text != l.last {
                let _ = l.chan.post_message(&text.as_str().into());
                l.last = text;
            }
        }
    }

    /// Instance `inst` was closed: the windows it opened lose their `super::`.
    pub(crate) fn super_closed(self: &Rc<Self>, inst: u32) {
        let mut links = self.superlink.borrow_mut();
        for l in links.opened.iter().filter(|l| l.inst == inst) {
            post(&l.chan, &serde_json::json!({"t": "closed"}));
            l.chan.close();
        }
        links.opened.retain(|l| l.inst != inst);
    }

    fn super_close_all(self: &Rc<Self>) {
        let links = self.superlink.borrow();
        for l in links.opened.iter() {
            post(&l.chan, &serde_json::json!({"t": "closed"}));
        }
    }

    /// `SET super::` / `CALL super::` in this window: send it to the opener.
    pub(crate) fn super_to_opener(&self, e: &Effect) {
        let links = self.superlink.borrow();
        let Some(c) = &links.child else { return };
        match e {
            Effect::SuperSet { prop, value } => post(&c.chan, &serde_json::json!({"t": "set", "prop": prop, "value": value})),
            Effect::SuperCall { name, args } => post(&c.chan, &serde_json::json!({"t": "call", "name": name, "args": args})),
            _ => {}
        }
    }

    /// This window was opened by another (`pr-super=token`): join the channel, ask for the opener's state and
    /// wait for it, at most `timeout_ms`. `true` when the opener answered; otherwise `super::` fails here.
    pub(crate) async fn super_attach(self: &Rc<Self>, token: &str, timeout_ms: u32) -> bool {
        let Ok(chan) = BroadcastChannel::new(&channel_name(token)) else { return false };
        self.host.borrow_mut().attach_remote_opener();
        let weak = Rc::downgrade(self);
        let mut resolve_slot: Option<Function> = None;
        let promise = Promise::new(&mut |res, _| resolve_slot = Some(res));
        let resolve = resolve_slot.expect("a promise executor runs at once");
        let first = resolve.clone();
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |ev: MessageEvent| {
            let (Some(inner), Some(m)) = (weak.upgrade(), parse(&ev)) else { return };
            match m["t"].as_str() {
                Some("state") => {
                    let props: BTreeMap<String, String> = m["props"].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_owned())).collect()).unwrap_or_default();
                    inner.host.borrow_mut().remote_opener_state(props);
                    // Only the first answer settles the promise; later ones are ignored by it.
                    let _ = first.call1(&JsValue::NULL, &JsValue::TRUE);
                }
                Some("closed") => {
                    inner.host.borrow_mut().remote_opener_closed();
                }
                _ => {}
            }
        });
        chan.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        post(&chan, &serde_json::json!({"t": "hello"}));
        self.superlink.borrow_mut().child = Some(ChildLink { chan, _on_message: on_message });
        let give_up = resolve.clone();
        let _timer = crate::api::Timeout::new(timeout_ms, move || {
            let _ = give_up.call1(&JsValue::NULL, &JsValue::FALSE);
        });
        let ok = wasm_bindgen_futures::JsFuture::from(promise).await.ok().and_then(|v| v.as_bool()).unwrap_or(false);
        if !ok {
            self.host.borrow_mut().remote_opener_closed();
        }
        ok
    }
}
