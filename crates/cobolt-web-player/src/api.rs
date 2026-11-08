// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The JavaScript boundary of the player** (WASM-017 R11–R15): the one `wasm-bindgen`
//! object the loader holds, and the driver that runs the module.
//!
//! # What crosses
//!
//! * the loader → the player: [`Player::new`] (configuration and manifest), [`Player::add_form`],
//!   [`Player::attach`] (the *link*: how to call the module), [`Player::mount`], [`Player::start`],
//!   [`Player::unmount`];
//! * the module's imports → the player: [`Player::flush`], [`Player::query`], [`Player::remote`],
//!   [`Player::abort`] — each takes the bytes of the batch **copied once** out of a view the loader
//!   made at that call and drops at once (it never keeps one, WASM-017 R10).
//!
//! `Player` is the only object exposed; `unmount()` (and `free()`) disposes it, and every closure the
//! player gave the browser (a listener, a timer, a socket handler) is owned by a value that is
//! dropped with the instance — there is no `Closure::forget` in this crate (a test greps for it).
//!
//! # The driver
//!
//! One future ([`drive`]) owns the module: it asks the [`Host`] for the next call, performs it
//! through the link (which may answer at once or later: main thread or Worker), hands the result
//! back, and then does what the call left to do — effects, requests, rendering. Events from the
//! DOM, answers from the network, dialog results and timers never touch the host directly: they go
//! into the *inbox*, which the driver empties between calls. That is what keeps the host from being
//! borrowed while the module runs and calls back into it.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use js_sys::{Array, Function, Int32Array, Promise, Reflect, Uint8Array};
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{future_to_promise, spawn_local, JsFuture};

use cobolt_wasm_abi::manifest::Manifest;

use crate::auth;
use crate::host::{CallResult, DialogKind, Effect, Host, HostOptions, ModuleCall, Outbound};
use crate::net::{self, Client, LinkState, Out, Socket};
use crate::view::interact::{Act, UiOp};
use crate::view::MenuStore;

/// What the loader tells the player about the page it runs in.
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct PlayerConfig {
    /// Run with no page (the node tests): the ABI host and the network, no DOM.
    pub headless: bool,
    /// Standalone forms rather than a single-page application (WASM-011).
    pub standalone: bool,
    pub debug_level: u32,
    pub backend_origins: Vec<String>,
    pub call_timeout_ms: Option<u64>,
    /// The project's payload limit (WASM-017 R8); `None` keeps the default.
    pub max_frame: Option<usize>,
    pub unsafe_plain_ws: bool,
    /// The backend contract the application was built against (`backend/exports.toml`
    /// `contract.version`; WASM-014 R2); empty = `net::DEFAULT_CONTRACT`.
    pub backend_contract: String,
    /// The exported programs declared `idempotent`: the only ones repeated after a lost connection (R9).
    pub backend_idempotent: Vec<String>,
    /// The project's extra link schemes (WASM-006 R7). `weblink_schemes` is the key a page built before
    /// the control was named Hyperlink writes; it is read the same.
    #[serde(alias = "weblink_schemes")]
    pub link_schemes: Vec<String>,
    pub recaptcha_site_key: String,
    /// `"in-app"` or `"hosted"` (WASM-009 R8b).
    pub auth_flow: String,
    pub locale: String,
    pub max_queue: Option<usize>,
    /// The form to open first; empty = the main form.
    pub start_form: String,
    /// The parameters the page's address carried for `start_form` (`pr-param-<name>=<value>`), as pairs.
    pub start_params: Vec<(String, String)>,
    /// The Maps tile address template (`{z}`, `{x}`, `{y}`); empty = the OpenStreetMap server.
    pub tile_url: String,
    /// Origins images may be loaded from besides the application's own files.
    pub image_origins: Vec<String>,
    /// The page that opens a standalone form in a tab of its own; empty = this page.
    pub app_url: String,
    /// The form this page was opened to run, when it is a standalone window (WASM-011 R9).
    pub opener_handle: String,
    /// The hosted sign-in flow came back to this page with an authorisation code (WASM-009 R8b).
    pub hosted_code: String,
    pub hosted_state: String,
    /// The host page's texts the developer translated (WASM-002 R15). The player reads the "connecting" modal's from them
    /// (`connecting`, `unreachable`, `reload`; WASM-016 R20–R21); without them it speaks the page's language.
    pub texts: std::collections::BTreeMap<String, String>,
    /// Whose is the page's address (WASM-011 R5, Q7): `"own"` (or empty) — this application follows and writes the
    /// hash route; `"none"` — it keeps its navigation in memory and leaves the address to the page. The Build writes
    /// `"none"` for an application mounted into someone else's element unless the project asked for the address.
    pub address: String,
    /// A phone's navigation stack (WASM-011 R13, Q5): every form opened over another is a full screen, and Back pops it.
    pub stack: bool,
}

impl PlayerConfig {
    /// This application follows and writes the page's address: a single-page application that was not told to leave it
    /// to the page. A standalone window's address is its own page's, which the player never rewrites.
    pub fn owns_address(&self) -> bool {
        !self.standalone && !self.address.trim().eq_ignore_ascii_case("none")
    }
}

/// Something that happened outside the driver; the driver acts on it between calls.
pub(crate) enum Inbox {
    Dom { inst: u32, ctl: usize, event: String, args: Vec<String> },
    Net(NetEvent),
    DialogResult { id: u32, result: String },
    TimerFired(u32),
    ChildClosed(String),
    CloseInstance(u32),
    /// The Authentication control's machine produced an event to deliver.
    Raw(Box<dyn FnOnce(&Rc<Inner>)>),
    /// What a user's act on control `ctl` of instance `inst` asks for (see `view::interact`).
    Acts { inst: u32, ctl: usize, acts: Vec<Act> },
}

pub(crate) enum NetEvent {
    Open,
    Message(String),
    /// A sealed frame (WASM-003 R24): every message after the `hello`s.
    Frame(Vec<u8>),
    Close,
    Error,
    Tick,
    /// The application came back from the background inside the mobile shell (WASM-012 R11, R15).
    Resumed,
}

/// How the player calls into the module: the loader's `perform` function.
struct JsLink {
    this: JsValue,
    perform: Function,
    /// `part(name, fn, arg)`: call a function of a part the loader loads on first use (optional).
    part: Option<Function>,
    /// `partModule(name)`: the part's module itself, for a part whose exports are objects (optional).
    part_module: Option<Function>,
    /// `offload(part, method, argsJson, width, height, rgba, wantPixels)`: run a long ImageBuffer method on a copy of the pixels in a
    /// worker; resolves to `{answer, width, height, rgba}` (WASM-018 T30). Only a page that runs the program in a worker has it.
    offload: Option<Function>,
}

impl JsLink {
    async fn run(&self, call: &ModuleCall) -> CallResult {
        let ret = match self.apply(call) {
            Ok(v) => v,
            Err(e) => return CallResult::Trapped(e),
        };
        let resolved = match JsFuture::from(Promise::resolve(&ret)).await {
            Ok(v) => v,
            Err(e) => return CallResult::Trapped(js_message(&e)),
        };
        Self::decode(call, resolved)
    }

    /// The call made and answered within this turn of the page, for a page that will not get another (`pagehide`,
    /// WASM-011 R12): the module of the main thread answers at once; a module in a worker answers a promise, which is
    /// left unawaited, and `None` says so.
    fn run_now(&self, call: &ModuleCall) -> Option<CallResult> {
        let ret = match self.apply(call) {
            Ok(v) => v,
            Err(e) => return Some(CallResult::Trapped(e)),
        };
        if ret.is_instance_of::<Promise>() {
            return None;
        }
        Some(Self::decode(call, ret))
    }

    fn apply(&self, call: &ModuleCall) -> Result<JsValue, String> {
        let none = JsValue::NULL;
        let (kind, a, b, c, d, payload): (i32, i32, i32, i32, i32, JsValue) = match call {
            ModuleCall::OpenForm { form, opener } => (0, *form as i32, *opener, 0, 0, none),
            ModuleCall::CloseForm { inst } => (1, *inst as i32, 0, 0, 0, none),
            ModuleCall::Event { inst, ctrl, event, args, argc } => {
                (2, *inst as i32, *ctrl as i32, *event as i32, *argc as i32, Uint8Array::from(args.as_slice()).into())
            }
            ModuleCall::Resume { task, frame } => {
                (3, *task, 0, 0, 0, frame.as_ref().map(|f| Uint8Array::from(f.as_slice()).into()).unwrap_or(none))
            }
            ModuleCall::Cancel { task } => (4, *task, 0, 0, 0, none),
        };
        let args = Array::new();
        for v in [JsValue::from(kind), a.into(), b.into(), c.into(), d.into(), payload] {
            args.push(&v);
        }
        self.perform.apply(&self.this, &args).map_err(|e| js_message(&e))
    }

    fn decode(call: &ModuleCall, resolved: JsValue) -> CallResult {
        let Ok(arr) = resolved.dyn_into::<Int32Array>() else {
            return CallResult::Trapped("the loader's perform() did not answer an Int32Array".into());
        };
        let v = arr.to_vec();
        let g = |i: usize| v.get(i).copied().unwrap_or(0);
        match call {
            ModuleCall::OpenForm { .. } => CallResult::Opened { inst: g(0), status: g(1), task: g(2) },
            // The loader marks its answer to a cancel (`[pr_status, pr_tick, 1]`); a loader that does not is
            // answered as before.
            ModuleCall::Cancel { .. } if g(2) == 1 => CallResult::Status { status: g(0), task: g(1) },
            ModuleCall::Cancel { .. } => CallResult::Unit,
            _ => CallResult::Status { status: g(0), task: g(1) },
        }
    }
}

fn js_message(e: &JsValue) -> String {
    if let Some(s) = e.as_string() {
        return s;
    }
    Reflect::get(e, &"message".into()).ok().and_then(|m| m.as_string()).unwrap_or_else(|| format!("{e:?}"))
}

fn global_fn(name: &str) -> Option<Function> {
    Reflect::get(&js_sys::global(), &name.into()).ok().and_then(|f| f.dyn_into::<Function>().ok())
}

pub(crate) fn now_ms() -> u64 {
    js_sys::Date::now() as u64
}

/// A macrotask turn: the browser can paint and handle input between two of these.
///
/// The turn is a message on a `MessageChannel`, not `setTimeout(0)`: once timers nest a browser holds each one back
/// by about 4 ms (1 500 of them in a row took 6.8 s in Chrome, 1 500 messages 5-13 ms), and a long loop that yields
/// every few milliseconds of work was paying that wait at every yield (WASM-015 T13b, `specs/WASM-PROGRESS.md`). A
/// message is a task of its own like a timer's, so painting and input still get their turn between two; a page with no
/// `MessageChannel` falls back to the timer.
async fn next_turn() {
    let p = Promise::new(&mut |resolve, _| {
        if !message_turn(&resolve) {
            timer_turn(&resolve);
        }
    });
    let _ = JsFuture::from(p).await;
}

/// A turn to wait out something that comes from elsewhere (a Worker's answer, a reply, a timer): a timer, because a
/// message turn would have the wait spin a core while it lasts. Where there is work to be done, use [`next_turn`].
async fn idle_turn() {
    let p = Promise::new(&mut |resolve, _| timer_turn(&resolve));
    let _ = JsFuture::from(p).await;
}

fn timer_turn(resolve: &Function) {
    match global_fn("setTimeout") {
        Some(f) => {
            let _ = f.call2(&JsValue::UNDEFINED, resolve, &0.into());
        }
        None => {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        }
    }
}

/// Wait, a turn at a time, until `done` holds or `limit_ms` of wall-clock time have passed. `true`: it held. The bound is
/// a time, not a count of turns: what a turn costs is the browser's to say, and a count that was a few seconds long
/// with timers is gone before a Worker has answered once with messages.
async fn wait_until(limit_ms: u64, mut done: impl FnMut() -> bool) -> bool {
    let deadline = now_ms() + limit_ms;
    loop {
        if done() {
            return true;
        }
        if now_ms() >= deadline {
            return false;
        }
        idle_turn().await;
    }
}

/// How long `unmount` and `reset` wait for a call in flight (the old bound was 1 000 timer turns, 1 to 4.5 s).
const IN_FLIGHT_WAIT_MS: u64 = 5_000;
/// How long `settled` waits for the driver to be idle (the old bound was 100 000 timer turns, 100 s or more).
const SETTLED_WAIT_MS: u64 = 120_000;
/// How many turns in a row `settled` may find the driver busy with nothing queued before it waits on a timer instead.
const SETTLED_SPIN_TURNS: u32 = 200;

/// Have `resolve` called by a message on a channel made for the purpose. `false`: the page has no `MessageChannel`, or
/// the message could not be posted, and nothing will call `resolve`.
fn message_turn(resolve: &Function) -> bool {
    let Ok(channel) = web_sys::MessageChannel::new() else { return false };
    let (receiver, sender) = (channel.port1(), channel.port2());
    let (done, resolve) = (receiver.clone(), resolve.clone());
    // Called once, by the message below; closing the port lets the channel go.
    let on_message = Closure::once_into_js(move |_event: JsValue| {
        done.close();
        let _ = resolve.call0(&JsValue::UNDEFINED);
    });
    receiver.set_onmessage(Some(on_message.unchecked_ref()));
    if sender.post_message(&JsValue::UNDEFINED).is_ok() {
        return true;
    }
    receiver.set_onmessage(None);
    receiver.close();
    false
}

/// A one-shot timer. The closure is owned by the timer's own task, so nothing is leaked and
/// nothing outlives the instance: dropping the returned handle cancels it.
pub(crate) struct Timeout {
    id: JsValue,
    _cb: Closure<dyn FnMut()>,
}

impl Timeout {
    pub(crate) fn new(ms: u32, f: impl FnOnce() + 'static) -> Timeout {
        let cell = Rc::new(RefCell::new(Some(f)));
        let cb = Closure::<dyn FnMut()>::new(move || {
            if let Some(f) = cell.borrow_mut().take() {
                f();
            }
        });
        let id = global_fn("setTimeout")
            .and_then(|set| set.call2(&JsValue::UNDEFINED, cb.as_ref().unchecked_ref(), &ms.into()).ok())
            .unwrap_or(JsValue::UNDEFINED);
        Timeout { id, _cb: cb }
    }
}

impl Drop for Timeout {
    fn drop(&mut self) {
        if let Some(clear) = global_fn("clearTimeout") {
            let _ = clear.call1(&JsValue::UNDEFINED, &self.id);
        }
    }
}

/// The browser's gamepads as `navigator.getGamepads()` reports them (the "standard" mapping, whatever the pad): the index, the
/// axes and which buttons are pressed. A slot with no pad is skipped; a browser with no Gamepad API has none.
fn read_gamepads() -> Vec<cobolt_forms::gamepad::Pad> {
    let mut out = Vec::new();
    let Some(nav) = web_sys::window().map(|w| w.navigator()) else { return out };
    let Some(list) = Reflect::get(&nav, &"getGamepads".into()).ok().and_then(|f| f.dyn_into::<Function>().ok()).and_then(|f| f.call0(&nav).ok()) else { return out };
    for pad in Array::from(&list).iter() {
        if pad.is_null() || pad.is_undefined() {
            continue;
        }
        let index = Reflect::get(&pad, &"index".into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0) as u32;
        let axes = Reflect::get(&pad, &"axes".into()).map(|a| Array::from(&a).iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect()).unwrap_or_default();
        let buttons = Reflect::get(&pad, &"buttons".into())
            .map(|b| Array::from(&b).iter().map(|v| Reflect::get(&v, &"pressed".into()).ok().and_then(|p| p.as_bool()).unwrap_or(false)).collect())
            .unwrap_or_default();
        out.push(cobolt_forms::gamepad::Pad { index, axes, buttons });
    }
    out
}

// ── the socket ──────────────────────────────────────────────────────────────

/// A `WebSocket` as the connection's [`Socket`]. Its four handlers push into the inbox.
struct WsSocket {
    inner: std::rc::Weak<Inner>,
    ws: Option<web_sys::WebSocket>,
    handlers: Vec<Closure<dyn FnMut(JsValue)>>,
}

impl WsSocket {
    fn detach(&mut self) {
        if let Some(ws) = self.ws.take() {
            ws.set_onopen(None);
            ws.set_onmessage(None);
            ws.set_onclose(None);
            ws.set_onerror(None);
            let _ = ws.close();
        }
        self.handlers.clear();
    }
}

impl Socket for WsSocket {
    fn open(&mut self, url: &str) {
        self.detach();
        let Ok(ws) = web_sys::WebSocket::new(url) else {
            if let Some(i) = self.inner.upgrade() {
                i.push(Inbox::Net(NetEvent::Error));
            }
            return;
        };
        let mk = |inner: std::rc::Weak<Inner>, make: fn(JsValue) -> NetEvent| {
            Closure::<dyn FnMut(JsValue)>::new(move |ev: JsValue| {
                if let Some(i) = inner.upgrade() {
                    i.push(Inbox::Net(make(ev)));
                }
            })
        };
        let on_open = mk(self.inner.clone(), |_| NetEvent::Open);
        let on_msg = mk(self.inner.clone(), |ev| {
            let data = Reflect::get(&ev, &"data".into()).unwrap_or(JsValue::UNDEFINED);
            match data.dyn_ref::<js_sys::ArrayBuffer>() {
                Some(buf) => NetEvent::Frame(js_sys::Uint8Array::new(buf).to_vec()),
                None => NetEvent::Message(data.as_string().unwrap_or_default()),
            }
        });
        let on_close = mk(self.inner.clone(), |_| NetEvent::Close);
        let on_err = mk(self.inner.clone(), |_| NetEvent::Error);
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        ws.set_onerror(Some(on_err.as_ref().unchecked_ref()));
        self.handlers = vec![on_open, on_msg, on_close, on_err];
        self.ws = Some(ws);
    }

    fn send(&mut self, text: &str) {
        if let Some(ws) = &self.ws {
            let _ = ws.send_with_str(text);
        }
    }

    fn send_binary(&mut self, bytes: &[u8]) {
        if let Some(ws) = &self.ws {
            // A copy in a plain ArrayBuffer: a WebSocket refuses a view over the threaded player's shared memory.
            let copy = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
            copy.copy_from(bytes);
            let _ = ws.send_with_array_buffer(&copy.buffer());
        }
    }

    fn close(&mut self) {
        self.detach();
    }
}

/// The 32 bytes of a key and the 16 of a nonce: the browser's cryptographic generator, `crypto.getRandomValues`,
/// and nothing else. `Math.random` is never a fallback (WASM-003 R24): a page without it gets no connection.
///
/// The bytes are drawn into a fresh array and copied out: the threaded player's memory is a `SharedArrayBuffer`, and
/// `getRandomValues` refuses a view over shared memory, so a direct draw into `buf` failed there ("no random source").
pub(crate) fn browser_entropy(buf: &mut [u8]) -> bool {
    let Some(crypto) = web_sys::window().and_then(|w| w.crypto().ok()) else {
        return false;
    };
    let tmp = js_sys::Uint8Array::new_with_length(buf.len() as u32);
    if crypto.get_random_values_with_js_u8_array(&tmp).is_err() {
        return false;
    }
    tmp.copy_to(buf);
    true
}

// ── the instance ────────────────────────────────────────────────────────────

/// The DOM projection of the instances. The browser implements it in `dom`; headless runs have none.
/// An implementation may read and change the host (it is never borrowed when these are called) and
/// reports user input with [`Inner::push`].
pub(crate) trait Surface {
    fn opened(&self, inner: &Rc<Inner>, inst: u32, mode: crate::host::OpenMode);
    fn closed(&self, inner: &Rc<Inner>, inst: u32);
    fn render(&self, inner: &Rc<Inner>);
    fn effect(&self, inner: &Rc<Inner>, e: &Effect);
    fn net_state(&self, inner: &Rc<Inner>, s: LinkState);
    /// A call to the Backend is in flight (`busy`) or none is: the controls with a `LoadingEffect` show their stripes while it is.
    fn net_busy(&self, _busy: bool) {}
    /// The connection is being retried (`blocked`) or gave up (`unreachable`): show, change or take away the "connecting"
    /// modal that blocks the application (WASM-016 R20–R21).
    fn net_blocked(&self, inner: &Rc<Inner>, blocked: bool, unreachable: bool);
    /// The stylesheets, the asset table or the locale changed: redo what depends on them.
    fn styles_changed(&self, inner: &Rc<Inner>);
    fn unmount(&self);
    /// A change of page state the user's act asked for (an open menu, a sorted grid, a window).
    fn ui_op(&self, inner: &Rc<Inner>, inst: u32, ctl: usize, op: UiOp);
    /// A user's act the host cannot perform by itself: resolve a menu item, show a notice, open a window.
    fn act(&self, inner: &Rc<Inner>, inst: u32, ctl: usize, act: Act);
    /// A read-back of the Canvas2D `ctl` (`GetPixel`, `ToImage` ...), answered by the Canvas part (WASM-018 T25).
    fn gfx_sync(&self, _inner: &Rc<Inner>, _inst: u32, _ctl: usize, _part: &str, _kind: &str, _size: (f64, f64), _method: &str, _args: &[String], _queued: Vec<(usize, crate::tree::GfxItem)>) -> crate::host::GfxAnswer {
        crate::host::GfxAnswer::NotLoaded
    }
}

/// What the page needs besides the forms: stylesheets, the asset table, the locale. Kept here so
/// that they can arrive before the page is mounted and be replaced while it runs.
#[derive(Default, Clone)]
pub(crate) struct Styles {
    /// The web theme (WASM-007 Q5): a built-in id, or a whole theme stylesheet; empty is Liquid Glass.
    pub theme: String,
    pub app: String,
    pub forms: std::collections::BTreeMap<String, String>,
    pub assets: std::collections::BTreeMap<String, String>,
    pub locale: String,
    pub menus: MenuStore,
}

pub(crate) struct Inner {
    pub(crate) styles: RefCell<Styles>,
    pub(crate) host: RefCell<Host>,
    pub(crate) cfg: PlayerConfig,
    link: RefCell<Option<Rc<JsLink>>>,
    pub(crate) surface: RefCell<Option<Box<dyn Surface>>>,
    driving: Cell<bool>,
    inbox: RefCell<VecDeque<Inbox>>,
    net: RefCell<Option<Client<WsSocket>>>,
    net_timer: RefCell<Option<Timeout>>,
    /// The connection's latest *blocked* and *unreachable* (WASM-003 R26, WASM-016 R21): what the
    /// "connecting" modal (WASM-016 R20) is drawn from; `stats()` reports them.
    net_blocked: Cell<bool>,
    net_unreachable: Cell<bool>,
    timers: RefCell<Vec<(u32, Timeout)>>,
    pub(crate) display_log: RefCell<Vec<String>>,
    pub(crate) errors: RefCell<Vec<crate::host::RunError>>,
    dead: Cell<bool>,
    weak: RefCell<std::rc::Weak<Inner>>,
    /// The session's token epoch the connection was last given (see `Host::auth_epoch`).
    auth_epoch_seen: Cell<u64>,
    /// Wakes the Authentication machine shortly before its token expires.
    auth_timer: RefCell<Option<Timeout>>,
    /// The Canvas part's module, once the page has loaded it (see `preload_canvas_part`): a read-back of a canvas is
    /// answered at once from it, however early in the program it is asked.
    pub(crate) parts: RefCell<HashMap<String, JsValue>>,
    /// The page's graphics device (a `GpuDevice` of the 3D part), opened once when the first 3D form opens; `None` when the
    /// browser has none (the Canvas3D then says so).
    pub(crate) gpu_device: RefCell<Option<JsValue>>,
    gpu_tried: Cell<bool>,
    /// The resolvers of the promise the loader waits on for a query the host parked (WASM-018 T30).
    answer_waiter: RefCell<Option<(Function, Function)>>,
    /// The project's font files (WASM-018 T27, Q7), fetched once when the first graphics part loads and handed to every
    /// registry that part makes; `None` until then.
    pub(crate) fonts: RefCell<Option<Rc<Vec<Vec<u8>>>>>,
    /// The gamepads the page saw at its last look, and the timer of the next one (WASM-018 T28): the page looks only while a
    /// canvas has a gamepad handler bound.
    gamepad: RefCell<(Vec<cobolt_forms::gamepad::Pad>, Option<Timeout>)>,
    /// The timer of the next display frame (WASM-018 T30, AC12): the page runs a frame loop only while a canvas has `TargetFps`
    /// above zero and an `onFrame` handler bound, and a hidden tab is not a frame.
    frames: RefCell<Option<Timeout>>,
    /// reCAPTCHA: the time limits and the script's load listeners of the tokens being obtained.
    pub(crate) captcha: RefCell<crate::dom::captcha::Pending>,
    /// The authorisation code the hosted flow came back with, until a control takes it.
    hosted_return: RefCell<Option<(String, String)>>,
    /// The channels to the windows this one opened and to the one that opened it (WASM-011 R11).
    pub(crate) superlink: RefCell<crate::superlink::Links>,
    /// Inside the mobile shell, what hears a return from the background (WASM-012 R11): the target, the event and the
    /// listener, kept while the page is mounted and taken off at unmount.
    resume: RefCell<Vec<(web_sys::EventTarget, &'static str, Closure<dyn FnMut(web_sys::Event)>)>>,
}

/// Is `ev`, of kind `kind`, the application coming back from the background? The mobile shell's `pr-shell` event with
/// `{"type":"resumed"}` (WASM-012 A26), or the page becoming visible again — the portable fallback (plan R11).
fn is_resume(kind: &str, ev: &web_sys::Event) -> bool {
    let get = |v: &JsValue, k: &str| Reflect::get(v, &k.into()).ok().and_then(|v| v.as_string());
    match kind {
        "pr-shell" => Reflect::get(ev, &"detail".into()).ok().is_some_and(|d| get(&d, "type").as_deref() == Some("resumed")),
        "visibilitychange" => ev.target().is_some_and(|t| get(&t.into(), "visibilityState").as_deref() == Some("visible")),
        _ => false,
    }
}

impl Inner {
    /// A standalone window that goes away runs its forms' closing handlers (WASM-011 R12), as far as the browser lets
    /// a page run while it goes: the tasks still waiting are cancelled, each instance gets `pr_close_form` within the
    /// `pagehide` turn, and a handler that suspends there is abandoned with `pr_cancel`. A page kept for the back/forward
    /// cache (`persisted`) is left alone: it may come back. The opener sees the window closed (`Inbox::ChildClosed`).
    fn close_on_pagehide(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let on_hide = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
            let Some(inner) = weak.upgrade() else { return };
            let persisted = Reflect::get(&ev, &"persisted".into()).ok().and_then(|v| v.as_bool()).unwrap_or(false);
            if persisted || inner.dead.get() {
                return;
            }
            let Some(link) = inner.link.borrow().clone() else { return };
            let calls = inner.host.borrow_mut().teardown();
            inner.dead.set(true);
            for call in &calls {
                let after = link.run_now(call).and_then(|r| Host::abandon_on_hide(&r));
                if let Some(cancel) = after {
                    let _ = link.run_now(&cancel);
                }
            }
        });
        if let Some(w) = web_sys::window() {
            let _ = w.add_event_listener_with_callback("pagehide", on_hide.as_ref().unchecked_ref());
        }
        self.superlink.borrow_mut().closing = Some(on_hide);
    }

    /// Inside the mobile shell, a return from the background reaches the connection (WASM-012 R11, R15), which tries a
    /// lost one again behind the "connecting" modal. Both signals may come for one return: the second finds the cycle
    /// already running. Outside the shell nothing is listened to.
    fn listen_for_resume(self: &Rc<Self>) {
        let Some(w) = web_sys::window().filter(|_| crate::shell::present()) else { return };
        let mut targets: Vec<(web_sys::EventTarget, &'static str)> = vec![(w.clone().into(), "pr-shell")];
        if let Some(d) = w.document() {
            targets.push((d.into(), "visibilitychange"));
        }
        let mut kept = self.resume.borrow_mut();
        for (target, kind) in targets {
            let weak = Rc::downgrade(self);
            let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
                if let Some(inner) = weak.upgrade().filter(|_| is_resume(kind, &ev)) {
                    inner.push(Inbox::Net(NetEvent::Resumed));
                }
            });
            let _ = target.add_event_listener_with_callback(kind, cb.as_ref().unchecked_ref());
            kept.push((target, kind, cb));
        }
    }

    /// Queue something for the driver and make sure it runs.
    pub(crate) fn push(&self, item: Inbox) {
        if self.dead.get() {
            return;
        }
        self.inbox.borrow_mut().push_back(item);
        if let Some(rc) = self.weak.borrow().upgrade() {
            pump(&rc);
        }
    }

    /// A DOM event on control `ctl` of instance `inst`.
    pub(crate) fn dom_event(&self, inst: u32, ctl: usize, event: &str, args: Vec<String>) {
        self.push(Inbox::Dom { inst, ctl, event: event.to_owned(), args });
    }

    fn absorb(self: &Rc<Self>) {
        loop {
            let item = self.inbox.borrow_mut().pop_front();
            let Some(item) = item else { break };
            match item {
                Inbox::Dom { inst, ctl, event, args } => {
                    self.host.borrow_mut().dom_event(inst, ctl, &event, args);
                }
                Inbox::DialogResult { id, result } => self.host.borrow_mut().dialog_result(id, &result),
                Inbox::TimerFired(id) => {
                    self.timers.borrow_mut().retain(|(i, _)| *i != id);
                    self.host.borrow_mut().timer_fired(id);
                }
                Inbox::ChildClosed(h) => self.host.borrow_mut().child_closed(&h),
                Inbox::CloseInstance(i) => self.host.borrow_mut().close_instance(i),
                Inbox::Raw(f) => f(self),
                Inbox::Net(ev) => self.net_event(ev),
                Inbox::Acts { inst, ctl, acts } => self.apply_acts(inst, ctl, acts),
            }
        }
    }

    /// Carry out what a user's act asked for, in order. The host is never borrowed across a call into
    /// the surface, and the surface never borrows the host across a call back.
    pub(crate) fn apply_acts(self: &Rc<Self>, inst: u32, ctl: usize, acts: Vec<Act>) {
        for a in acts {
            match a {
                Act::Event { name, args } => {
                    // A click on the cancel control a form declared with `WS-SHOW-PROGRESS` cancels the pending call (WASM-014 R13).
                    let cancel = if name == "onClick" { self.host.borrow().cancel_clicked(inst, ctl) } else { None };
                    if let Some(call) = cancel {
                        if let Some(c) = self.net.borrow_mut().as_mut() {
                            c.request_cancel(call);
                        }
                    }
                    self.host.borrow_mut().dom_event(inst, ctl, name, args);
                }
                Act::Quiet { prop, value } => self.host.borrow_mut().ui_set(inst, ctl, &prop, &value),
                Act::Change { prop, value } => self.host.borrow_mut().user_change(inst, ctl, &prop, &value),
                Act::Prevent => {}
                Act::Auth(action) => self.host.borrow_mut().auth_act(inst, ctl, action),
                Act::Link { plan, click } => self.host.borrow_mut().link_click(inst, ctl, plan, click),
                Act::Ui(op) => {
                    if let Some(sf) = self.surface.borrow().as_ref() {
                        sf.ui_op(self, inst, ctl, op);
                    }
                }
                other => {
                    if let Some(sf) = self.surface.borrow().as_ref() {
                        sf.act(self, inst, ctl, other);
                    }
                }
            }
        }
    }

    fn net_event(self: &Rc<Self>, ev: NetEvent) {
        let now = now_ms();
        {
            let mut guard = self.net.borrow_mut();
            let Some(c) = guard.as_mut() else { return };
            match ev {
                NetEvent::Open => c.on_open(now),
                NetEvent::Message(t) => c.on_text(&t),
                NetEvent::Frame(b) => c.on_binary(&b),
                NetEvent::Close => c.on_close(now),
                NetEvent::Error => c.on_error(now),
                NetEvent::Tick => c.tick(now),
                NetEvent::Resumed => c.resumed(now),
            }
        }
        self.drain_net();
    }

    /// Hand the connection's answers to the host and set the timer for its next deadline.
    fn drain_net(self: &Rc<Self>) {
        let (outs, refresh, deadline, busy) = {
            let mut guard = self.net.borrow_mut();
            let Some(c) = guard.as_mut() else { return };
            (c.take_out(), std::mem::take(&mut c.refresh_requested), c.next_deadline(), c.in_flight() > 0)
        };
        if let Some(sf) = self.surface.borrow().as_ref() {
            sf.net_busy(busy);
        }
        let mut modal_changed = false;
        for o in outs {
            match o {
                Out::Answer { id, frame } if crate::grid::is_grid_id(id) => {
                    // A remote grid's call (WASM-005 T16): the rows or the save's outcome are the host's.
                    self.host.borrow_mut().grid_answer(id, frame);
                }
                Out::Answer { id, frame } => self.host.borrow_mut().remote_answer(id, frame),
                Out::State(s) => {
                    if let Some(sf) = self.surface.borrow().as_ref() {
                        sf.net_state(self, s);
                    }
                }
                Out::Warning(w) => web_sys::console::warn_1(&w.into()),
                Out::BackendVersion { state, message, update_to } => {
                    self.host.borrow_mut().backend_version(&state, &message, &update_to);
                }
                Out::BackendWarning { program, message } => {
                    self.host.borrow_mut().backend_warning(&program, &message);
                }
                Out::Progress { id, percent, message } => {
                    // D-Progress (WASM-014 R13): the controls the form declared are set here, with no COBOL running.
                    self.host.borrow_mut().call_progress(id, percent, &message);
                }
                Out::StreamItem { sub, json, restart } => {
                    self.host.borrow_mut().stream_item(sub, &json, restart);
                }
                Out::StreamEnd { sub, reason } => {
                    self.host.borrow_mut().stream_end(sub, &reason);
                }
                Out::FileUrl { path, .. } => {
                    // A download is the browser's own navigation to the address the backend issued (WASM-014 R15, Q4): a short-lived
                    // path under this page's origin, never an address the module chose.
                    if path.starts_with("/dl/") && !path.contains("//") {
                        if let Some(w) = web_sys::window() {
                            let _ = w.location().assign(&path);
                        }
                    } else {
                        web_sys::console::warn_1(&format!("the backend issued a download address that is not under /dl/: {path}").into());
                    }
                }
                // The module reaches uploads through imports the ABI does not carry yet (WASM-010); until then a transfer's progress is only logged.
                Out::PutReady { .. } | Out::PutDone { .. } => {}
                Out::TransferFailed { id, class, code, message } => {
                    web_sys::console::warn_1(&format!("transfer {id} failed: {class} {code}: {message}").into());
                }
                Out::Blocked(b) => {
                    modal_changed |= self.net_blocked.replace(b) != b;
                }
                Out::Unreachable(u) => {
                    let was = self.net_unreachable.replace(u);
                    modal_changed |= was != u;
                    // The exception of WASM-016 R21: the application hears that the backend could not be reached, once
                    // per give-up (the calls that waited have already failed with the class transport).
                    if u && !was {
                        self.host.borrow_mut().backend_unreachable();
                    }
                }
            }
        }
        for sub in self.host.borrow_mut().take_closed_streams() {
            if let Some(c) = self.net.borrow_mut().as_mut() {
                c.unsubscribe(sub);
            }
        }
        // The modal follows in the same turn as the break, so it is up before the next frame (AC13).
        if modal_changed {
            if let Some(sf) = self.surface.borrow().as_ref() {
                sf.net_blocked(self, self.net_blocked.get(), self.net_unreachable.get());
            }
        }
        if refresh {
            self.run_auth_refresh();
        }
        // One timer for the connection's next deadline (call timeout or reconnection).
        *self.net_timer.borrow_mut() = deadline.map(|at| {
            let weak = Rc::downgrade(self);
            let ms = at.saturating_sub(now_ms()).min(u32::MAX as u64) as u32;
            Timeout::new(ms, move || {
                if let Some(i) = weak.upgrade() {
                    i.push(Inbox::Net(NetEvent::Tick));
                }
            })
        });
    }

    fn run_auth_refresh(self: &Rc<Self>) {
        // The server refused an expired token. The Authentication control (if the application has
        // one) performs the silent refresh and gives the connection the new token; with none, the
        // call simply fails upstream.
        self.host.borrow_mut().auth_refresh();
    }

    /// Give the connection a token (or none).
    pub(crate) fn set_net_token(self: &Rc<Self>, token: Option<String>) {
        if let Some(c) = self.net.borrow_mut().as_mut() {
            c.set_token(token);
        }
        self.drain_net();
    }

    /// The loader's `offload` function and its `this`, when the page runs the program in a worker and can run a long image method in another (WASM-018 T30).
    pub(crate) fn offload_fn(&self) -> Option<(JsValue, Function)> {
        let link = self.link.borrow().clone()?;
        link.offload.clone().map(|f| (link.this.clone(), f))
    }

    /// A call the page sent to a worker has its answer: let the host finish it and run what waited, answer the parked query, show the effects.
    pub(crate) fn finish_offload(self: &Rc<Self>, answer: crate::host::GfxAnswer) {
        self.host.borrow_mut().finish_deferred(answer);
        self.deliver_answer();
        pump(self);
    }

    /// Hand the parked query's answer to the loader's promise, when both exist.
    pub(crate) fn deliver_answer(&self) {
        if self.answer_waiter.borrow().is_none() {
            return;
        }
        let Some(r) = self.host.borrow_mut().take_final_answer() else { return };
        let Some((resolve, reject)) = self.answer_waiter.borrow_mut().take() else { return };
        match r {
            Ok(bytes) => {
                let _ = resolve.call1(&JsValue::UNDEFINED, &Uint8Array::from(bytes.as_slice()));
            }
            Err(code) => {
                let _ = reject.call1(&JsValue::UNDEFINED, &JsValue::from(code));
            }
        }
    }

    /// Load the graphics part `name` (once): the module is kept for the registries of every form instance that uses
    /// it, and the 3D part also opens the page's graphics device (WebGPU, else WebGL2).
    pub(crate) async fn load_part(&self, name: &str) -> Result<(), String> {
        if !self.parts.borrow().contains_key(name) {
            let m = self.part_module(name).await?;
            self.parts.borrow_mut().insert(name.to_owned(), m);
        }
        if name == "graphics3d" && !self.gpu_tried.replace(true) {
            self.open_gpu().await;
        }
        self.load_fonts().await;
        Ok(())
    }

    /// Fetch the project's font files (the `Assets/` fonts, and the ones the Build found for the families the project names),
    /// once: a text drawn by a canvas or a plot is lettered with them, as on the desktop. A file that cannot be fetched is
    /// reported on the console and the others still load.
    async fn load_fonts(&self) {
        if self.fonts.borrow().is_some() {
            return;
        }
        let urls: Vec<(String, String)> = self
            .styles
            .borrow()
            .assets
            .iter()
            .filter(|(k, _)| [".ttf", ".otf", ".ttc"].iter().any(|e| k.ends_with(e)))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut files = Vec::new();
        for (name, url) in urls {
            match crate::dom::fetch_bytes(&url).await {
                Ok(b) => files.push(b),
                Err(e) => web_sys::console::warn_1(&format!("[pr] the font {name} could not be fetched: {e}").into()),
            }
        }
        *self.fonts.borrow_mut() = Some(Rc::new(files));
    }

    /// Open the graphics device with the 3D part's `open_gpu`. A browser with no adapter is not an error of the page:
    /// the device stays absent and a Canvas3D says so when it renders (WASM-018 AC5).
    async fn open_gpu(&self) {
        let Some(module) = self.parts.borrow().get("graphics3d").cloned() else { return };
        let Some(open) = Reflect::get(&module, &"open_gpu".into()).ok().and_then(|f| f.dyn_into::<Function>().ok()) else { return };
        match open.call0(&JsValue::UNDEFINED) {
            Ok(p) => match JsFuture::from(Promise::resolve(&p)).await {
                Ok(dev) => *self.gpu_device.borrow_mut() = Some(dev),
                Err(e) => web_sys::console::warn_1(&format!("[pr] no graphics device: {}", js_message(&e)).into()),
            },
            Err(e) => web_sys::console::warn_1(&format!("[pr] no graphics device: {}", js_message(&e)).into()),
        }
    }

    /// Before the module opens form `form`, load the part its graphics objects need (WASM-018 T26): the program may read a
    /// canvas or call a Plot in its first handler, and the answer must not wait for a download. A form with no graphics
    /// object loads nothing; a page that cannot load parts goes on, and the first call says so.
    async fn preload_graphics(&self, form: usize) {
        let Some(part) = self.host.borrow().graphics_part(form) else { return };
        let _ = self.load_part(part).await;
    }

    /// Call `func` of the part `part` (a module of its own the loader fetches the first time it is asked
    /// for) with one text, and answer its text (WASM-016 R13). An error is the part's own message.
    pub(crate) async fn call_part(&self, part: &str, func: &str, arg: &str) -> Result<String, String> {
        self.call_part_with(part, func, &arg.into()).await
    }

    /// The module of the part `part` (loaded on first use): for a part whose exports are objects, not text.
    pub(crate) async fn part_module(&self, part: &str) -> Result<JsValue, String> {
        let link = self.link.borrow().clone().ok_or_else(|| "the player is not attached to a loader".to_owned())?;
        let f = link.part_module.as_ref().ok_or_else(|| "this page cannot load the player's parts".to_owned())?;
        let pending = f.call1(&link.this, &part.into()).map_err(|e| js_message(&e))?;
        JsFuture::from(Promise::resolve(&pending)).await.map_err(|e| js_message(&e))
    }

    /// The same, with the argument a file's bytes (the PDF part reads a document's bytes).
    pub(crate) async fn call_part_bytes(&self, part: &str, func: &str, bytes: &[u8]) -> Result<String, String> {
        self.call_part_with(part, func, &js_sys::Uint8Array::from(bytes).into()).await
    }

    async fn call_part_with(&self, part: &str, func: &str, arg: &JsValue) -> Result<String, String> {
        let link = self.link.borrow().clone().ok_or_else(|| "the player is not attached to a loader".to_owned())?;
        let f = link.part.as_ref().ok_or_else(|| "this page cannot load the player's parts".to_owned())?;
        let pending = f.call3(&link.this, &part.into(), &func.into(), arg).map_err(|e| js_message(&e))?;
        let answer = JsFuture::from(Promise::resolve(&pending)).await.map_err(|e| js_message(&e))?;
        answer.as_string().ok_or_else(|| format!("the {part} part answered no text"))
    }

    /// Hand request `id` to the connection, opening it (anonymously, or with the session's token) when
    /// it is closed. The module's requests and the Authentication control's go the same way.
    fn net_submit(self: &Rc<Self>, id: u32, frame: &[u8]) -> Result<(), String> {
        if self.net.borrow().is_none() || self.cfg.backend_origins.is_empty() {
            return Err("the application has no backend configured".into());
        }
        let now = now_ms();
        {
            let mut guard = self.net.borrow_mut();
            let c = guard.as_mut().expect("checked above");
            if c.state() == LinkState::Closed {
                let token = self.host.borrow().auth.token().map(str::to_owned);
                c.set_token(token);
                c.connect(&self.cfg.backend_origins[0], now)?;
            }
            c.request_as(id, frame, now);
        }
        self.drain_net();
        Ok(())
    }

    /// An Authentication control's backend call (WASM-009 R7, D5): an HTTPS request to the backend's
    /// `/auth/*` routes, never a WebSocket frame. Its answer comes back through the inbox.
    fn send_auth_call(self: &Rc<Self>, id: u32, call: auth::BackendCall) {
        let Some(base) = self.cfg.backend_origins.first().and_then(|b| auth::https::base_of(b)) else {
            // Nothing to send it to: the control reads it as any failed call (the user sees the generic message).
            self.auth_answered(id, auth::AuthResult::Failed { detail: "the application has no backend configured".into() });
            return;
        };
        let post = match &call {
            auth::BackendCall::HostedStart => {
                // A navigation, not a request: the backend sets the flow's cookie and sends the browser on.
                let result = match crate::auth_fetch::keep_state() {
                    Some(state) => auth::AuthResult::Hosted { redirect_url: auth::https::hosted_start_url(&base, &crate::auth_fetch::page_address(), &state) },
                    None => auth::AuthResult::Failed { detail: "the page cannot keep the sign-in state".into() },
                };
                self.auth_answered(id, result);
                return;
            }
            auth::BackendCall::HostedFinish { state, .. } if !auth::https::state_matches(crate::auth_fetch::take_state().as_deref(), state) => {
                // A return this tab did not start (a link from someone else's sign-in): never redeemed.
                self.auth_answered(id, auth::AuthResult::Failed { detail: "hosted state".into() });
                return;
            }
            other => match auth::https::post(other) {
                Some(p) => p,
                None => return,
            },
        };
        let weak = Rc::downgrade(self);
        spawn_local(async move {
            let result = match crate::auth_fetch::post(&post.url(&base), &post.body).await {
                Ok((status, body)) => auth::https::decode(status, &body),
                Err(e) => auth::AuthResult::Failed { detail: e },
            };
            if let Some(i) = weak.upgrade() {
                i.push(Inbox::Raw(Box::new(move |inner| inner.auth_answered(id, result))));
            }
        });
    }

    /// Ask the phone shell to run `command` (ABI A26); its answer settles wait `id`. A shell that
    /// has gone, or a promise that rejects, answers `-6` with the reason in the console.
    fn shell_invoke(self: &Rc<Self>, id: u32, command: &'static str, args: &[(&'static str, String)]) {
        let weak = Rc::downgrade(self);
        let settle = move |status: i32, value: String| {
            if let Some(i) = weak.upgrade() {
                i.push(Inbox::Raw(Box::new(move |inner| inner.host.borrow_mut().shell_answer(id, status, &value))));
            }
        };
        let Some(promise) = crate::shell::invoke(command, args) else {
            settle(cobolt_wasm_abi::codes::err::HOST_REFUSED, String::new());
            return;
        };
        spawn_local(async move {
            match JsFuture::from(promise).await {
                Ok(v) => {
                    let (status, value) = crate::shell::answer_of(command, &v);
                    settle(status, value);
                }
                Err(e) => {
                    web_sys::console::warn_1(&format!("PowerRustCOBOL mobile shell: {command}: {}", crate::shell::refusal_of(&e)).into());
                    settle(cobolt_wasm_abi::codes::err::HOST_REFUSED, String::new());
                }
            }
        });
    }

    /// The answer to identity call `id` reached the page.
    fn auth_answered(self: &Rc<Self>, id: u32, result: auth::AuthResult) {
        let answered = self.host.borrow_mut().auth_answer(id, result, now_ms() as f64);
        if matches!(answered, Some(auth::BackendCall::SignOut)) {
            // The server forgets who the connection was when it is closed: a later call opens a new one.
            if let Some(c) = self.net.borrow_mut().as_mut() {
                c.reset_session();
            }
        }
    }

    /// Give the connection the session's token when it has changed, and set the clock that refreshes
    /// the token before it expires (WASM-009 R5).
    fn sync_auth_token(self: &Rc<Self>) {
        let epoch = self.host.borrow().auth_epoch;
        if epoch != self.auth_epoch_seen.get() {
            self.auth_epoch_seen.set(epoch);
            let token = self.host.borrow().auth.token().map(str::to_owned);
            self.set_net_token(token);
            self.arm_auth_timer();
        }
    }

    fn arm_auth_timer(self: &Rc<Self>) {
        let due = self.host.borrow().auth_deadline();
        let timer = due.map(|at| {
            // Never sooner than five seconds: a refresh that fails must not become a busy loop.
            let ms = (at - js_sys::Date::now()).clamp(5_000.0, 2_000_000_000.0) as u32;
            let weak = Rc::downgrade(self);
            Timeout::new(ms, move || {
                if let Some(i) = weak.upgrade() {
                    i.push(Inbox::Raw(Box::new(|inner| {
                        inner.host.borrow_mut().auth_tick(js_sys::Date::now());
                        inner.arm_auth_timer();
                    })));
                }
            })
        });
        *self.auth_timer.borrow_mut() = timer;
    }

    /// The hosted flow's return: hand the authorisation code to the first Authentication control, once.
    fn take_hosted_return(self: &Rc<Self>) {
        let pending = self.hosted_return.borrow().clone();
        if let Some((code, state)) = pending {
            if self.host.borrow_mut().auth_hosted_return(&code, &state) {
                *self.hosted_return.borrow_mut() = None;
            }
        }
    }

    fn send_requests(self: &Rc<Self>) {
        // A headless run with no backend keeps the requests in the outbox for the test that plays
        // the server (`pending_requests` / `deliver_remote`); a page with no backend answers the
        // program at once, as for any lost connection.
        if self.cfg.headless && self.cfg.backend_origins.is_empty() {
            return;
        }
        let outs = self.host.borrow_mut().take_outbox();
        if outs.is_empty() {
            return;
        }
        for o in outs {
            match o {
                Outbound::Request { id, frame } => {
                    // No backend configured, or an address the page may not connect to: the program is
                    // told, as for any lost connection.
                    if let Err(e) = self.net_submit(id, &frame) {
                        let f = net::error_frame(id, net::class::TRANSPORT, net::code::NOT_CONFIGURED, &e, Some(false));
                        if crate::grid::is_grid_id(id) {
                            self.host.borrow_mut().grid_answer(id, f);
                        } else {
                            self.host.borrow_mut().remote_answer(id, f);
                        }
                    }
                }
                Outbound::Cancel { id } => {
                    if let Some(c) = self.net.borrow_mut().as_mut() {
                        c.cancel(id);
                    }
                }
            }
        }
    }

    fn process_effects(self: &Rc<Self>) {
        let effects = self.host.borrow_mut().take_effects();
        for e in effects {
            match &e {
                Effect::Display(t) => {
                    web_sys::console::log_1(&t.into());
                    self.display_log.borrow_mut().push(t.clone());
                }
                Effect::Log { level, text } => {
                    web_sys::console::debug_1(&format!("[pr:{level}] {text}").into());
                }
                Effect::RuntimeError(r) => {
                    web_sys::console::error_1(&r.describe().into());
                    self.errors.borrow_mut().push(r.clone());
                }
                Effect::Opened { inst, mode, .. } => {
                    if let Some(sf) = self.surface.borrow().as_ref() {
                        sf.opened(self, *inst, *mode);
                    }
                    continue;
                }
                Effect::Closed { inst } => {
                    self.super_closed(*inst);
                    if let Some(sf) = self.surface.borrow().as_ref() {
                        sf.closed(self, *inst);
                    }
                    continue;
                }
                Effect::SuperSet { .. } | Effect::SuperCall { .. } => {
                    self.super_to_opener(&e);
                    continue;
                }
                Effect::SuperGet { .. } | Effect::SuperCallR { .. } => {
                    self.super_ask_opener(&e);
                    continue;
                }
                Effect::AuthCall { id, call } => {
                    self.send_auth_call(*id, call.clone());
                    continue;
                }
                Effect::AuthCaptcha { id, site_key, action } => {
                    self.run_captcha(*id, site_key, *action);
                    continue;
                }
                Effect::Shell { id, command, args } => {
                    self.shell_invoke(*id, command, args);
                    continue;
                }
                Effect::Timer { id, ms } => {
                    let weak = Rc::downgrade(self);
                    let id = *id;
                    let t = Timeout::new((*ms).min(u32::MAX as u64) as u32, move || {
                        if let Some(i) = weak.upgrade() {
                            i.push(Inbox::TimerFired(id));
                        }
                    });
                    self.timers.borrow_mut().push((id, t));
                    continue;
                }
                Effect::Dialog { id, kind, default, .. } if self.surface.borrow().is_none() => {
                    // No page to show it on: answer as an automatic user would (the headless tests).
                    let result = match kind {
                        DialogKind::Message => "OK".to_owned(),
                        DialogKind::Confirm => "Yes".to_owned(),
                        DialogKind::Input => default.clone(),
                    };
                    self.inbox.borrow_mut().push_back(Inbox::DialogResult { id: *id, result });
                    continue;
                }
                _ => {}
            }
            if let Some(sf) = self.surface.borrow().as_ref() {
                sf.effect(self, &e);
            }
        }
    }

    pub(crate) fn styles_changed(self: &Rc<Self>) {
        if let Some(sf) = self.surface.borrow().as_ref() {
            sf.styles_changed(self);
        }
    }

    /// Start or stop looking at the browser's gamepads (WASM-018 T28): one look every frame (16 ms) while some open canvas has
    /// `onGamepadButton` or `onGamepadAxis` bound, none otherwise. Each change becomes the events the desktop raises
    /// (`cobolt_forms::gamepad::diff`), queued like any other event.
    fn sync_gamepad(self: &Rc<Self>) {
        let wants = self.host.borrow().wants_gamepad();
        let mut g = self.gamepad.borrow_mut();
        if !wants {
            g.0.clear();
            g.1 = None;
            cobolt_forms::gamepad::feed(Vec::new());
            return;
        }
        if g.1.is_some() {
            return;
        }
        let weak = self.weak.borrow().clone();
        g.1 = Some(Timeout::new(16, move || {
            if let Some(inner) = weak.upgrade() {
                inner.poll_gamepads();
            }
        }));
    }

    fn poll_gamepads(self: &Rc<Self>) {
        let next = read_gamepads();
        let events = {
            let mut g = self.gamepad.borrow_mut();
            g.1 = None;
            let prev = std::mem::replace(&mut g.0, next.clone());
            // What `GamepadAxis` and `GamepadButton` answer from: the pads as the page last saw them.
            cobolt_forms::gamepad::feed(next.clone());
            cobolt_forms::gamepad::diff(&prev, &next)
        };
        if !events.is_empty() {
            self.push(Inbox::Raw(Box::new(move |inner| {
                inner.host.borrow_mut().gamepad_events(&events);
            })));
        }
        self.sync_gamepad();
    }

    /// Start or stop the frame loop (WASM-018 T30, AC12): one look every display frame (16 ms) while some open canvas has `TargetFps`
    /// and an `onFrame` handler. The clock of each canvas (`cobolt_forms::frame_clock`) decides which looks are frames; a hidden
    /// tab (`document.hidden`) raises none and forgets the time it was hidden.
    fn sync_frames(self: &Rc<Self>) {
        let wants = self.host.borrow().wants_frames();
        let mut slot = self.frames.borrow_mut();
        if !wants {
            *slot = None;
            return;
        }
        if slot.is_some() {
            return;
        }
        let weak = self.weak.borrow().clone();
        *slot = Some(Timeout::new(16, move || {
            if let Some(inner) = weak.upgrade() {
                inner.frame_look();
            }
        }));
    }

    fn frame_look(self: &Rc<Self>) {
        self.frames.borrow_mut().take();
        let now = web_sys::window().and_then(|w| w.performance()).map_or(0.0, |p| p.now());
        let visible = !web_sys::window().and_then(|w| w.document()).is_some_and(|d| d.hidden());
        self.push(Inbox::Raw(Box::new(move |inner| {
            inner.host.borrow_mut().frame_tick(now, visible);
        })));
        self.sync_frames();
    }

    fn after_call(self: &Rc<Self>) {
        self.sync_frames();
        self.sync_gamepad();
        self.take_hosted_return();
        // What a step leaves can start another (an Authentication answer that raises an event, a
        // refused call answered at once): go round until nothing is left, never forever.
        for _ in 0..4 {
            self.host.borrow_mut().auth_flush();
            self.process_effects();
            self.send_requests();
            self.sync_auth_token();
            if !self.host.borrow().has_outgoing() {
                break;
            }
        }
        if let Some(sf) = self.surface.borrow().as_ref() {
            sf.render(self);
        }
        self.super_publish();
    }
}

fn pump(inner: &Rc<Inner>) {
    if inner.driving.get() || inner.dead.get() {
        return;
    }
    let Some(link) = inner.link.borrow().clone() else { return };
    inner.driving.set(true);
    let inner = inner.clone();
    spawn_local(async move {
        drive(&inner, &link).await;
        inner.driving.set(false);
        // Something may have arrived between the last look and the flag going down. Work that waits behind
        // a suspended task does not count: it runs when the task's answer comes, which pumps again.
        if !inner.inbox.borrow().is_empty() || inner.host.borrow().can_run() {
            pump(&inner);
        }
    });
}

async fn drive(inner: &Rc<Inner>, link: &Rc<JsLink>) {
    loop {
        if inner.dead.get() {
            return;
        }
        inner.absorb();
        let call = inner.host.borrow_mut().next_call();
        match call {
            Some(call) => {
                if let ModuleCall::OpenForm { form, .. } = &call {
                    inner.preload_graphics(*form as usize).await;
                }
                let result = link.run(&call).await;
                if inner.dead.get() {
                    return;
                }
                inner.host.borrow_mut().complete(&call, result);
                inner.after_call();
            }
            None => {
                inner.after_call();
                let (yielded, waiting) = {
                    let h = inner.host.borrow();
                    (h.has_yielded(), h.has_waiting())
                };
                if yielded {
                    // Give the browser a turn (to paint, to handle input), then let the loop go on.
                    next_turn().await;
                    inner.host.borrow_mut().allow_yielded();
                    continue;
                }
                let _ = waiting;
                if inner.inbox.borrow().is_empty() {
                    return;
                }
            }
        }
    }
}

/// Let the host ask the Canvas part for the answer of a read-back (ABI A23, WASM-018 T25). The hook holds the player weakly.
fn install_gfx_sync(inner: &Rc<Inner>) {
    let weak = Rc::downgrade(inner);
    inner.host.borrow_mut().gfx_sync = Some(Rc::new(move |inst, ctl, part, kind, size, method, args, queued| {
        let Some(inner) = weak.upgrade() else { return crate::host::GfxAnswer::NotLoaded };
        let sf = inner.surface.borrow();
        match sf.as_ref() {
            Some(sf) => sf.gfx_sync(&inner, inst, ctl, part, kind, size, method, args, queued),
            None => crate::host::GfxAnswer::NotLoaded,
        }
    }));
}

/// The ABI host for the program whose manifest is `manifest_json`, set up as the page's configuration says.
fn make_host(cfg: &PlayerConfig, manifest_json: &str) -> Result<Host, JsError> {
    let manifest = Manifest::parse(manifest_json).map_err(|e| JsError::new(&e.to_string()))?;
    let opts = HostOptions {
        debug_level: cfg.debug_level,
        standalone: cfg.standalone,
        max_queue: cfg.max_queue.unwrap_or(HostOptions::default().max_queue),
    };
    let mut host = Host::new(manifest, opts);
    host.set_auth_defaults(
        if cfg.auth_flow.eq_ignore_ascii_case("hosted") { crate::auth::Flow::Hosted } else { crate::auth::Flow::InApp },
        &cfg.recaptcha_site_key,
    );
    if !cfg.locale.trim().is_empty() {
        host.set_page_locale(&cfg.locale);
    }
    host.policy = crate::hyperlink::Policy { extra_schemes: cfg.link_schemes.clone() };
    host.platform = browser_platform();
    host.shell = crate::shell::present();
    Ok(host)
}

/// Whose modifier-key table a link follows (WASM-006 Q2): `navigator.userAgentData.platform` where the
/// browser has it, `navigator.platform` otherwise.
fn browser_platform() -> crate::hyperlink::Platform {
    let Some(w) = web_sys::window() else { return crate::hyperlink::Platform::Other };
    let nav: JsValue = w.navigator().into();
    let ua_data = Reflect::get(&nav, &"userAgentData".into()).ok().filter(|v| v.is_object());
    let name = ua_data
        .and_then(|d| Reflect::get(&d, &"platform".into()).ok())
        .and_then(|v| v.as_string())
        .filter(|s| !s.is_empty())
        .or_else(|| Reflect::get(&nav, &"platform".into()).ok().and_then(|v| v.as_string()))
        .unwrap_or_default();
    crate::hyperlink::Platform::from_navigator(&name)
}

// ── the exported object ─────────────────────────────────────────────────────

/// The one object the loader holds.
#[wasm_bindgen]
pub struct Player {
    inner: Rc<Inner>,
}

#[wasm_bindgen]
impl Player {
    /// `config_json` is the page's configuration (see [`PlayerConfig`]); `manifest_json` the program's.
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: &str, manifest_json: &str) -> Result<Player, JsError> {
        let cfg: PlayerConfig = if config_json.trim().is_empty() {
            PlayerConfig::default()
        } else {
            serde_json::from_str(config_json).map_err(|e| JsError::new(&format!("the player configuration is not valid: {e}")))?
        };
        let host = make_host(&cfg, manifest_json)?;
        let hosted_return = (!cfg.hosted_code.is_empty()).then(|| (cfg.hosted_code.clone(), cfg.hosted_state.clone()));
        let inner = Rc::new(Inner {
            styles: RefCell::new(Styles { locale: cfg.locale.clone(), ..Styles::default() }),
            host: RefCell::new(host),
            cfg,
            link: RefCell::new(None),
            surface: RefCell::new(None),
            driving: Cell::new(false),
            inbox: RefCell::new(VecDeque::new()),
            net: RefCell::new(None),
            net_timer: RefCell::new(None),
            net_blocked: Cell::new(false),
            net_unreachable: Cell::new(false),
            timers: RefCell::new(Vec::new()),
            display_log: RefCell::new(Vec::new()),
            errors: RefCell::new(Vec::new()),
            dead: Cell::new(false),
            weak: RefCell::new(std::rc::Weak::new()),
            auth_epoch_seen: Cell::new(0),
            auth_timer: RefCell::new(None),
            parts: RefCell::new(HashMap::new()),
            gpu_device: RefCell::new(None),
            gpu_tried: Cell::new(false),
            answer_waiter: RefCell::new(None),
            fonts: RefCell::new(None),
            gamepad: RefCell::new((Vec::new(), None)),
            frames: RefCell::new(None),
            captcha: RefCell::new(Default::default()),
            hosted_return: RefCell::new(hosted_return),
            superlink: RefCell::new(Default::default()),
            resume: RefCell::new(Vec::new()),
        });
        *inner.weak.borrow_mut() = Rc::downgrade(&inner);
        if !inner.cfg.backend_origins.is_empty() {
            let (contract, idempotent) = {
                let h = inner.host.borrow();
                net::backend_from_manifest(&h.manifest, &inner.cfg.backend_contract, &inner.cfg.backend_idempotent)
            };
            let ncfg = net::Config::from_page(
                inner.cfg.backend_origins.clone(),
                inner.cfg.unsafe_plain_ws,
                inner.cfg.call_timeout_ms,
                inner.cfg.max_frame,
            )
            .with_backend(&contract, idempotent);
            let mut ncfg = ncfg;
            ncfg.entropy = Some(browser_entropy);
            let sock = WsSocket { inner: Rc::downgrade(&inner), ws: None, handlers: Vec::new() };
            let mut client = Client::new(ncfg, sock);
            // The session id is held in memory only (WASM-014 Q5): unpredictable, never stored.
            client.seed_session(((js_sys::Math::random() * 9_007_199_254_740_992.0) as u64) ^ (js_sys::Date::now() as u64).rotate_left(29));
            *inner.net.borrow_mut() = Some(client);
        }
        Ok(Player { inner })
    }

    /// Register the description (the JSON the Build writes, WASM-016 R3) of one form.
    pub fn add_form(&self, json: &str) -> Result<(), JsError> {
        self.inner.host.borrow_mut().add_description(json).map_err(|e| JsError::new(&e))
    }

    /// Forms the manifest lists that have no description yet.
    pub fn missing_forms(&self) -> Vec<JsValue> {
        self.inner.host.borrow().missing_forms().into_iter().map(JsValue::from).collect()
    }

    /// Give the player the way to call the module: an object with `perform(kind, a, b, c, d, payload)`.
    pub fn attach(&self, link: JsValue) -> Result<(), JsError> {
        let perform = Reflect::get(&link, &"perform".into())
            .ok()
            .and_then(|f| f.dyn_into::<Function>().ok())
            .ok_or_else(|| JsError::new("the link has no perform() function"))?;
        let part = Reflect::get(&link, &"part".into()).ok().and_then(|f| f.dyn_into::<Function>().ok());
        let part_module = Reflect::get(&link, &"partModule".into()).ok().and_then(|f| f.dyn_into::<Function>().ok());
        let offload = Reflect::get(&link, &"offload".into()).ok().and_then(|f| f.dyn_into::<Function>().ok());
        *self.inner.link.borrow_mut() = Some(Rc::new(JsLink { this: link, perform, part, part_module , offload }));
        Ok(())
    }

    /// Mount the page (a no-op in a headless run).
    pub fn mount(&self, root: JsValue) -> Result<(), JsError> {
        if self.inner.cfg.headless {
            return Ok(());
        }
        let el: web_sys::Element = root.dyn_into().map_err(|_| JsError::new("mount() needs a DOM element"))?;
        let surface = crate::dom::create(&self.inner, el).map_err(|e| JsError::new(&e))?;
        *self.inner.surface.borrow_mut() = Some(surface);
        install_gfx_sync(&self.inner);
        // The sheets the loader set before the page existed (a form's, the project's, a theme file — WASM-007
        // R4) had no surface to go to: apply them now.
        self.inner.styles_changed();
        Ok(())
    }

    /// This window was opened by another (`pr-super=token` in its address): join the channel and wait, at most
    /// `timeout_ms`, for the opener's state, so that the first handler already reads it (WASM-011 R11).
    /// Answers whether the opener replied; when it did not, `super::` fails here with `ON EXCEPTION`.
    /// Call it before `start()`.
    pub async fn attach_opener(&self, token: String, timeout_ms: u32) -> bool {
        self.inner.super_attach(&token, timeout_ms).await
    }

    /// Open the first form (the main form, or `start_form` of the configuration) and run.
    pub fn start(&self) -> Result<(), JsError> {
        let missing = self.inner.host.borrow().missing_forms();
        if !missing.is_empty() {
            return Err(JsError::new(&format!("no description was loaded for the form(s): {}", missing.join(", "))));
        }
        {
            let mut h = self.inner.host.borrow_mut();
            let sf = self.inner.cfg.start_form.trim();
            match h.manifest.form_by_name(sf).map(|f| f.id as usize) {
                Some(id) if !sf.is_empty() => h.start_with(id, self.inner.cfg.start_params.clone()),
                // A page opened for a form the application does not have (WASM-011 R6): the main form, the notice,
                // and `onRouteNotFound` once the main form is there to hear it.
                None if !sf.is_empty() => {
                    h.start();
                    h.not_found_at_start(sf);
                    h.effects.push(Effect::Notice(crate::texts::route_not_found(&self.inner.cfg.locale, &self.inner.cfg.texts, sf)));
                }
                _ => h.start(),
            }
        }
        if self.inner.cfg.standalone {
            self.inner.close_on_pagehide();
        }
        self.inner.listen_for_resume();
        pump(&self.inner);
        Ok(())
    }

    // ── the module's imports ────────────────────────────────────────────────

    /// `gfx` is the display list the module left in the graphics region for this batch (ABI A23): given when the
    /// region starts with a list, which a `DRAW` of the batch then names.
    pub fn flush(&self, bytes: &[u8], gfx: Option<js_sys::Uint8Array>) {
        let mut h = self.inner.host.borrow_mut();
        if let Some(g) = gfx {
            h.set_gfx(g.to_vec());
        }
        h.flush(bytes);
    }

    /// The answer of the query that ends the batch, or `undefined` with [`Player::last_error`] set.
    pub fn query(&self, bytes: &[u8], gfx: Option<js_sys::Uint8Array>) -> Option<Vec<u8>> {
        let mut h = self.inner.host.borrow_mut();
        if let Some(g) = gfx {
            h.set_gfx(g.to_vec());
        }
        h.query(bytes).ok()
    }

    /// Whether the last `query` was parked because its call runs in a worker (WASM-018 T30): the loader then waits for
    /// [`Player::await_answer`] instead of answering the module at once.
    pub fn answer_pending(&self) -> bool {
        self.inner.host.borrow().query_waiting()
    }

    /// A promise of the parked query's answer: the bytes, or rejected with the code of the error.
    pub fn await_answer(&self) -> Promise {
        let inner = self.inner.clone();
        Promise::new(&mut move |resolve, reject| {
            *inner.answer_waiter.borrow_mut() = Some((resolve, reject));
            inner.deliver_answer();
        })
    }

    pub fn last_error(&self) -> i32 {
        self.inner.host.borrow().last_error()
    }

    /// The id of the request, or a negative error code.
    pub fn remote(&self, frame: &[u8]) -> i32 {
        match self.inner.host.borrow_mut().remote(frame) {
            Ok(id) => id as i32,
            Err(code) => code,
        }
    }

    pub fn abort(&self, bytes: &[u8]) {
        self.inner.host.borrow_mut().abort(bytes);
    }

    // ── diagnostics (also what the node tests read) ─────────────────────────

    /// The `DISPLAY` output so far, one line per call.
    pub fn display_log(&self) -> String {
        self.inner.display_log.borrow().join("\n")
    }

    /// The run-time errors so far, as JSON `[{"code":…,"message":…,"form":…,"event":…}]`.
    pub fn errors(&self) -> String {
        let v: Vec<serde_json::Value> = self
            .inner
            .errors
            .borrow()
            .iter()
            .map(|e| serde_json::json!({"code": e.code, "message": e.message, "form": e.form, "event": e.event, "line": e.line, "text": e.describe()}))
            .collect();
        serde_json::Value::Array(v).to_string()
    }

    /// Counters of the host (batches, queries, events, yields, traps, bytes).
    pub fn stats(&self) -> String {
        let s = self.inner.host.borrow().stats.clone();
        serde_json::json!({
            "flushes": s.flushes, "queries": s.queries, "remotes": s.remotes, "aborts": s.aborts, "commands": s.commands,
            "sets": s.sets, "calls": s.calls, "events_queued": s.events_queued, "events_delivered": s.events_delivered,
            "events_coalesced": s.events_coalesced, "events_dropped": s.events_dropped, "module_calls": s.module_calls,
            "yields": s.yields, "traps": s.traps, "bytes_in": s.bytes_in, "bytes_out": s.bytes_out,
            "instances": self.inner.host.borrow().trees.len(), "tasks": self.inner.host.borrow().live_tasks(),
            "blocked": self.inner.net_blocked.get(), "unreachable": self.inner.net_unreachable.get(),
        })
        .to_string()
    }

    /// The state of every instance as JSON (the node tests read properties back from it).
    pub fn snapshot(&self) -> String {
        self.inner.host.borrow().snapshot().to_string()
    }

    /// Call a method of a control (`invoke(0, "SNK-1", "Show", ["Saved"])`), from the host page or a test.
    /// Returns the method's answer (empty for a method with none).
    pub fn invoke(&self, inst: u32, control: &str, method: &str, args: Vec<JsValue>) -> Result<String, JsError> {
        let args: Vec<String> = args.iter().map(|a| a.as_string().unwrap_or_default()).collect();
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let r = self.inner.host.borrow_mut().invoke(inst, control, method, &refs);
        pump(&self.inner);
        r.map(|v| v.unwrap_or_default()).map_err(|e| JsError::new(&e))
    }

    /// Headless only: raise a DOM event on a control, as the renderer would. Returns whether a handler is bound.
    pub fn dispatch(&self, inst: u32, control: &str, event: &str, args: Vec<JsValue>) -> Result<bool, JsError> {
        if !self.inner.cfg.headless {
            return Err(JsError::new("dispatch() is for headless runs; a page raises events itself"));
        }
        let ctl = self.inner.host.borrow().trees.get(&inst).and_then(|t| t.by_name(control));
        let Some(ctl) = ctl else { return Err(JsError::new(&format!("no control `{control}` in instance {inst}"))) };
        let args: Vec<String> = args.iter().map(|a| a.as_string().unwrap_or_default()).collect();
        let bound = self.inner.host.borrow().trees[&inst].ctls[ctl].has_handler(event);
        self.inner.dom_event(inst, ctl, event, args);
        Ok(bound)
    }

    /// Headless only: set a property as the user would (typing), without raising an event.
    pub fn type_into(&self, inst: u32, control: &str, property: &str, value: &str) -> Result<(), JsError> {
        if !self.inner.cfg.headless {
            return Err(JsError::new("type_into() is for headless runs"));
        }
        let mut h = self.inner.host.borrow_mut();
        let tree = h.trees.get_mut(&inst).ok_or_else(|| JsError::new("no such instance"))?;
        let ctl = tree.by_name(control).ok_or_else(|| JsError::new(&format!("no control `{control}`")))?;
        tree.set_from_ui(ctl, property, value);
        Ok(())
    }

    /// Headless only: answer a pending request as the backend would.
    pub fn deliver_remote(&self, id: u32, frame: &[u8]) -> Result<(), JsError> {
        if !self.inner.cfg.headless {
            return Err(JsError::new("deliver_remote() is for headless runs"));
        }
        self.inner.host.borrow_mut().remote_answer(id, frame.to_vec());
        pump(&self.inner);
        Ok(())
    }

    /// Headless only: the requests the module made that nobody has answered, as JSON.
    pub fn pending_requests(&self) -> String {
        let outs = self.inner.host.borrow_mut().take_outbox();
        let v: Vec<serde_json::Value> = outs
            .into_iter()
            .map(|o| match o {
                Outbound::Request { id, frame } => serde_json::json!({"id": id, "frame": String::from_utf8_lossy(&frame)}),
                Outbound::Cancel { id } => serde_json::json!({"cancel": id}),
            })
            .collect();
        serde_json::Value::Array(v).to_string()
    }

    /// Resolve once the driver has nothing left to do (every queued event ran, every yield finished).
    pub fn settled(&self) -> Promise {
        let inner = self.inner.clone();
        future_to_promise(async move {
            let deadline = now_ms() + SETTLED_WAIT_MS;
            let mut waiting = 0u32;
            while now_ms() < deadline {
                // Look first, and start the driver only when there is something for it: starting it
                // sets `driving`, so pumping an idle player would make it look busy forever.
                let queued = !inner.inbox.borrow().is_empty() || inner.host.borrow().can_run() || inner.host.borrow().has_yielded();
                if queued && !inner.driving.get() {
                    pump(&inner);
                }
                if !queued && !inner.driving.get() {
                    return Ok(JsValue::TRUE);
                }
                // Work to do, or a call that is about to answer: the next turn, as quickly as it comes. A wait that goes on
                // (a Worker running a long loop, a reply on the wire) is waited out on a timer, not by spinning a core.
                waiting = if queued { 0 } else { waiting + 1 };
                if waiting < SETTLED_SPIN_TURNS {
                    next_turn().await;
                } else {
                    idle_turn().await;
                }
            }
            Ok(JsValue::FALSE)
        })
    }

    /// Close one instance (as the user closing its dialog would).
    pub fn close_instance(&self, inst: u32) {
        self.inner.push(Inbox::CloseInstance(inst));
    }

    /// Set or replace the application stylesheet, without reloading the player (WASM-016 R16, WASM-007 Q3).
    pub fn set_stylesheet(&self, css: &str) {
        self.inner.styles.borrow_mut().app = css.to_owned();
        self.inner.styles_changed();
    }

    /// Set or replace the web theme, without reloading the player (WASM-007 Q5): a built-in id
    /// (`liquid-glass`, `elegance`, `spatial`), or the text of a whole theme stylesheet (the project's
    /// `theme_file`, the bundle's `theme.css`), which replaces the built-in one. Empty is Liquid Glass.
    pub fn set_theme(&self, theme: &str) {
        self.inner.styles.borrow_mut().theme = theme.to_owned();
        self.inner.styles_changed();
    }

    /// Set or replace the default stylesheet of one form (WASM-007 R4/R9: it applies inside that form's root only).
    pub fn set_form_stylesheet(&self, form: &str, css: &str) {
        self.inner.styles.borrow_mut().forms.insert(form.to_ascii_lowercase(), css.to_owned());
        self.inner.styles_changed();
    }

    /// The asset table: the path a form uses (`Assets/logo.png`) → the file the bundle holds (JSON object).
    pub fn set_assets(&self, json: &str) -> Result<(), JsError> {
        let map: std::collections::BTreeMap<String, String> =
            serde_json::from_str(json).map_err(|e| JsError::new(&format!("the asset table is not valid: {e}")))?;
        // Looked up in lower case with forward slashes, whatever the form wrote (`Assets\Logo.png`).
        self.inner.styles.borrow_mut().assets = map.into_iter().map(|(k, v)| (k.to_ascii_lowercase().replace('\\', "/"), v)).collect();
        self.inner.styles_changed();
        Ok(())
    }

    /// The menu of a MenuBar or SideMenu control, as the build writes it (the menu file as JSON).
    pub fn add_menu(&self, form: &str, control: &str, json: &str) -> Result<(), JsError> {
        let def: cobolt_forms::menu::MenuDefinition =
            serde_json::from_str(json).map_err(|e| JsError::new(&format!("the menu of `{control}` in form `{form}` is not valid: {e}")))?;
        // The host answers `SideMenu::ActivateItem` from the same rows the page draws.
        self.inner.host.borrow_mut().menus.insert(form, control, def.clone());
        self.inner.styles.borrow_mut().menus.insert(form, control, def);
        self.inner.styles_changed();
        Ok(())
    }

    pub fn set_locale(&self, locale: &str) {
        self.inner.styles.borrow_mut().locale = locale.to_owned();
        self.inner.host.borrow_mut().set_page_locale(locale);
        self.inner.styles_changed();
    }

    /// Open a form of the program as an in-page panel (a deep link, the Preview, the tests).
    pub fn open_form(&self, name: &str) -> Result<(), JsError> {
        let id = self
            .inner
            .host
            .borrow()
            .manifest
            .form_by_name(name)
            .map(|f| f.id as usize)
            .ok_or_else(|| JsError::new(&format!("the program has no form `{name}`")))?;
        self.inner.host.borrow_mut().open_form(id, None, crate::host::OpenMode::Panel);
        pump(&self.inner);
        Ok(())
    }

    /// The Preview's in-place update (WASM-016 R16): stop the application — every open instance is closed in the module
    /// it runs in and leaves the page — and take the program whose manifest is `manifest_json` (the same one, or a new
    /// module's). The player, the page, the stylesheets and the connection stay. When the promise resolves, register the
    /// descriptions (`add_form`), give the module's link (`attach`) and `start()` again. A session the Authentication
    /// control held is not kept: the application starts as it does on a fresh page.
    pub fn reset(&self, manifest_json: String) -> Promise {
        let inner = self.inner.clone();
        future_to_promise(async move {
            if inner.dead.get() {
                return Err(JsError::new("the player was unmounted").into());
            }
            let fresh = make_host(&inner.cfg, &manifest_json)?;
            wait_until(IN_FLIGHT_WAIT_MS, || !inner.driving.get()).await;
            // Nothing the old program asked for may reach the new one.
            inner.driving.set(true);
            let calls = inner.host.borrow_mut().teardown();
            inner.inbox.borrow_mut().clear();
            inner.timers.borrow_mut().clear();
            let link = inner.link.borrow().clone();
            if let Some(link) = link {
                for c in &calls {
                    let _ = link.run(c).await;
                }
            }
            // The instances leave the page (teardown reported each one closed).
            inner.process_effects();
            *inner.host.borrow_mut() = fresh;
            install_gfx_sync(&inner);
            *inner.link.borrow_mut() = None;
            inner.inbox.borrow_mut().clear();
            inner.driving.set(false);
            let closed = calls.iter().filter(|c| matches!(c, ModuleCall::CloseForm { .. })).count();
            Ok(JsValue::from_str(&serde_json::json!({"closed": closed}).to_string()))
        })
    }

    /// Unmount: close every instance, drop every listener and timer, close the socket (WASM-017 R5).
    /// The returned promise resolves when the module has been told.
    pub fn unmount(&self) -> Promise {
        let inner = self.inner.clone();
        future_to_promise(async move {
            if inner.dead.get() {
                return Ok(JsValue::from_str("{\"closed\":0,\"cancelled\":0}"));
            }
            // Wait for a call in flight to finish, then take everything down.
            wait_until(IN_FLIGHT_WAIT_MS, || !inner.driving.get()).await;
            let calls = inner.host.borrow_mut().teardown();
            inner.dead.set(true);
            if let Some(c) = inner.net.borrow_mut().as_mut() {
                c.shutdown();
            }
            *inner.net.borrow_mut() = None;
            *inner.net_timer.borrow_mut() = None;
            for (target, kind, cb) in inner.resume.borrow_mut().drain(..) {
                let _ = target.remove_event_listener_with_callback(kind, cb.as_ref().unchecked_ref());
            }
            // The look at the gamepads stops with the page (WASM-018 T28).
            *inner.gamepad.borrow_mut() = (Vec::new(), None);
            // The graphics device the 3D part opened for the page goes with it (WASM-018 T26).
            if let Some(dev) = inner.gpu_device.borrow_mut().take() {
                if let Ok(free) = Reflect::get(&dev, &"free".into()).and_then(|f| f.dyn_into::<Function>()) {
                    let _ = free.call0(&dev);
                }
            }
            inner.parts.borrow_mut().clear();
            inner.fonts.borrow_mut().take();
            inner.timers.borrow_mut().clear();
            inner.inbox.borrow_mut().clear();
            let link = inner.link.borrow().clone();
            if let Some(link) = link {
                for c in &calls {
                    let _ = link.run(c).await;
                }
            }
            if let Some(sf) = inner.surface.borrow_mut().take() {
                sf.unmount();
            }
            *inner.link.borrow_mut() = None;
            let closed = calls.iter().filter(|c| matches!(c, ModuleCall::CloseForm { .. })).count();
            let cancelled = calls.iter().filter(|c| matches!(c, ModuleCall::Cancel { .. })).count();
            Ok(JsValue::from_str(&serde_json::json!({"closed": closed, "cancelled": cancelled}).to_string()))
        })
    }
}

// ── panics (WASM-017 R15) ───────────────────────────────────────────────────

thread_local! {
    /// One function per application mounted on the page (WASM-011 Q7). Two loaders of one bundle share one player
    /// instance — `player.js` instantiates the module once per address and answers the same instance afterwards — so
    /// a panic, which ends that instance, ends every application on it, and each must be told.
    static PANIC_FNS: RefCell<Vec<Function>> = const { RefCell::new(Vec::new()) };
}

/// Install the function the loader wants called when the player panics. The player is built with
/// `panic = "abort"`, so a panic ends the instance; this is how the page learns why.
#[wasm_bindgen]
pub fn install_panic_hook(f: Function) {
    PANIC_FNS.with(|p| p.borrow_mut().push(f));
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        web_sys::console::error_1(&format!("PowerRustCOBOL player panic: {msg}").into());
        PANIC_FNS.with(|p| {
            for f in p.borrow().iter() {
                let _ = f.call1(&JsValue::UNDEFINED, &msg.as_str().into());
            }
        });
    }));
}

/// Force a panic (the test of R15: the page must show the readable error and the reload action).
/// Only in a player built with the `test-hooks` feature: a shipped player has no export a page
/// script could end the application with.
#[cfg(feature = "test-hooks")]
#[wasm_bindgen]
pub fn force_panic_for_test() {
    panic!("forced panic (test)");
}

/// Sorts `rows` synthetic rows the way a DataGrid's `SORT` does and says how many threads compared them (WASM-017 T12):
/// `{"rows","pool","threads_used","sorted","threaded"}`. On the plain player `pool` and `threads_used` are 1.
#[wasm_bindgen]
pub fn sort_workers(rows: u32) -> String {
    let r = crate::par::sort_report(rows as usize);
    serde_json::json!({ "rows": r.rows, "pool": r.pool, "threads_used": r.threads_used, "sorted": r.sorted, "threaded": cfg!(feature = "threaded") }).to_string()
}

/// The player's version triple (WASM-016 R11): the ABI, the form-description format, the player.
#[wasm_bindgen]
pub fn versions() -> String {
    serde_json::json!({
        "abi": cobolt_wasm_abi::ABI_VERSION,
        "description": cobolt_forms::description::DESCRIPTION_FORMAT,
        "player": env!("CARGO_PKG_VERSION"),
        "threaded": cfg!(feature = "threaded"),
    })
    .to_string()
}

#[cfg(feature = "threaded")]
pub use wasm_bindgen_rayon::init_thread_pool;
