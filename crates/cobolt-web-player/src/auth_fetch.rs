// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Authentication control's requests in the browser (WASM-009 D5, T21): a `fetch` to the
//! backend's `/auth/*` routes with the backend's cookie, and the hosted flow's state kept across the
//! trip to the provider's page. What the requests hold and what their answers mean is
//! `crate::auth::https`; this file only carries them.
//!
//! The hosted state is the one thing the player keeps in browser storage: a random value, not a
//! token, in `sessionStorage` (this tab only), written before the page leaves for the provider and
//! removed when it comes back. It is what lets the page refuse a return link that another browser's
//! sign-in made (login CSRF). No token, code or handle is ever stored.

use js_sys::{Function, Object, Promise, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

/// The `sessionStorage` key of the hosted flow's state.
pub(crate) const HOSTED_STATE_KEY: &str = "pr-hosted-state";

/// `POST url` with `body` as JSON, the backend's cookie included and `X-PR-Identity: 1`; answers the
/// status and the body. `Err` when the request did not reach the backend (no network, CORS).
pub(crate) async fn post(url: &str, body: &str) -> Result<(u16, Vec<u8>), String> {
    let w = web_sys::window().ok_or("no window")?;
    let headers = Object::new();
    set(&headers, "Content-Type", &"application/json".into())?;
    let (name, value) = crate::auth::https::IDENTITY_HEADER;
    set(&headers, name, &value.into())?;
    let init = Object::new();
    set(&init, "method", &"POST".into())?;
    set(&init, "mode", &"cors".into())?;
    set(&init, "credentials", &"include".into())?;
    set(&init, "cache", &"no-store".into())?;
    set(&init, "headers", &headers)?;
    set(&init, "body", &body.into())?;
    let fetch: Function = Reflect::get(&w, &"fetch".into()).map_err(|_| "no fetch")?.dyn_into().map_err(|_| "no fetch")?;
    let resp = JsFuture::from(Promise::from(fetch.call2(&w, &url.into(), &init).map_err(|_| "the request could not be made")?)).await.map_err(|_| "the backend is not reachable")?;
    let status = Reflect::get(&resp, &"status".into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0) as u16;
    let read: Function = Reflect::get(&resp, &"arrayBuffer".into()).map_err(|_| "no body")?.dyn_into().map_err(|_| "no body")?;
    let bytes = JsFuture::from(Promise::from(read.call0(&resp).map_err(|_| "no body")?)).await.map_err(|_| "the answer was cut")?;
    Ok((status, js_sys::Uint8Array::new(&bytes).to_vec()))
}

fn set(o: &Object, k: &str, v: &JsValue) -> Result<(), String> {
    Reflect::set(o, &k.into(), v).map(|_| ()).map_err(|_| format!("cannot set {k}"))
}

/// This page's address: where the hosted flow comes back to.
pub(crate) fn page_address() -> String {
    web_sys::window().and_then(|w| w.location().href().ok()).unwrap_or_default()
}

/// A new random state for a hosted sign-in, kept in this tab until the page comes back. `None` when
/// the page cannot keep one (storage refused): the hosted sign-in is then not started.
pub(crate) fn keep_state() -> Option<String> {
    let w = web_sys::window()?;
    let mut bytes = [0u8; 16];
    if !crate::api::browser_entropy(&mut bytes) {
        return None;
    }
    let state: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    w.session_storage().ok()??.set_item(HOSTED_STATE_KEY, &state).ok()?;
    Some(state)
}

/// The state kept by [`keep_state`], removed as it is read: it is good for one return.
pub(crate) fn take_state() -> Option<String> {
    let store = web_sys::window()?.session_storage().ok()??;
    let kept = store.get_item(HOSTED_STATE_KEY).ok()?;
    let _ = store.remove_item(HOSTED_STATE_KEY);
    kept
}
