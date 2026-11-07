// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The phone shell, seen from the page** (WASM-012 T15, ABI A26). A Mobile application runs in
//! the web view of the PowerRustCOBOL mobile shell (Tauri 2), which puts its command bridge on the
//! page as `window.__TAURI_INTERNALS__.invoke(command, args)` — a function that answers with a
//! promise. Everything the player asks of the shell goes through [`invoke`]; outside the shell (a
//! Web application, the browser preview of a Mobile one) [`present`] is false and nothing here is
//! called.

use js_sys::{Function, Object, Promise, Reflect};
use wasm_bindgen::{JsCast, JsValue};

fn internals() -> Option<JsValue> {
    let w: JsValue = web_sys::window()?.into();
    let i = Reflect::get(&w, &"__TAURI_INTERNALS__".into()).ok()?;
    i.is_object().then_some(i)
}

/// Is the page running inside the mobile shell?
pub fn present() -> bool {
    internals().and_then(|i| Reflect::get(&i, &"invoke".into()).ok()).is_some_and(|f| f.is_function())
}

/// Call the shell's command `command` with `args` (each a string): the promise of its answer, or
/// `None` outside the shell.
pub fn invoke(command: &str, args: &[(&str, String)]) -> Option<Promise> {
    let i = internals()?;
    let f: Function = Reflect::get(&i, &"invoke".into()).ok()?.dyn_into().ok()?;
    let obj = Object::new();
    for (k, v) in args {
        Reflect::set(&obj, &(*k).into(), &v.as_str().into()).ok()?;
    }
    f.call2(&i, &command.into(), &obj).ok()?.dyn_into::<Promise>().ok()
}

/// The status and value of an answer to `command` (ABI A26): `0` done, `1` the key holds no value
/// (a `secure_load` that found nothing, a `secure_delete` with nothing to delete), and the text.
pub fn answer_of(command: &str, v: &JsValue) -> (i32, String) {
    match command {
        "secure_load" => match v.as_string() {
            Some(s) => (0, s),
            None => (1, String::new()),
        },
        "secure_delete" => (if v.as_bool() == Some(false) { 1 } else { 0 }, String::new()),
        _ => (0, String::new()),
    }
}

/// The text of a refusal (the promise rejected with a string or an error).
pub fn refusal_of(e: &JsValue) -> String {
    e.as_string()
        .or_else(|| Reflect::get(e, &"message".into()).ok().and_then(|m| m.as_string()))
        .unwrap_or_else(|| "the phone shell refused the call".to_owned())
}
