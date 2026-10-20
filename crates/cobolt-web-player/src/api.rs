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
use std::collections::VecDeque;
use std::rc::Rc;

use js_sys::{Array, Function, Int32Array, Promise, Reflect, Uint8Array};
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{future_to_promise, spawn_local, JsFuture};

use cobolt_forms::xml::load_form_from_str;
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
    pub unsafe_plain_ws: bool,
    pub weblink_schemes: Vec<String>,
    pub recaptcha_site_key: String,
    /// `"in-app"` or `"hosted"` (WASM-009 R8b).
    pub auth_flow: String,
    pub locale: String,
    pub max_queue: Option<usize>,
    /// The form to open first; empty = the main form.
    pub start_form: String,
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
    Close,
    Error,
    Tick,
}

/// How the player calls into the module: the loader's `perform` function.
struct JsLink {
    this: JsValue,
    perform: Function,
    /// `part(name, fn, arg)`: call a function of a part the loader loads on first use (optional).
    part: Option<Function>,
}

impl JsLink {
    async fn run(&self, call: &ModuleCall) -> CallResult {
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
        let ret = match self.perform.apply(&self.this, &args) {
            Ok(v) => v,
            Err(e) => return CallResult::Trapped(js_message(&e)),
        };
        let resolved = match JsFuture::from(Promise::resolve(&ret)).await {
            Ok(v) => v,
            Err(e) => return CallResult::Trapped(js_message(&e)),
        };
        let Ok(arr) = resolved.dyn_into::<Int32Array>() else {
            return CallResult::Trapped("the loader's perform() did not answer an Int32Array".into());
        };
        let v = arr.to_vec();
        let g = |i: usize| v.get(i).copied().unwrap_or(0);
        match call {
            ModuleCall::OpenForm { .. } => CallResult::Opened { inst: g(0), status: g(1), task: g(2) },
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
async fn next_turn() {
    let p = Promise::new(&mut |resolve, _| {
        match global_fn("setTimeout") {
            Some(f) => {
                let _ = f.call2(&JsValue::UNDEFINED, &resolve, &0.into());
            }
            None => {
                let _ = resolve.call0(&JsValue::UNDEFINED);
            }
        }
    });
    let _ = JsFuture::from(p).await;
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
            NetEvent::Message(Reflect::get(&ev, &"data".into()).ok().and_then(|d| d.as_string()).unwrap_or_default())
        });
        let on_close = mk(self.inner.clone(), |_| NetEvent::Close);
        let on_err = mk(self.inner.clone(), |_| NetEvent::Error);
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

    fn close(&mut self) {
        self.detach();
    }
}

// ── the instance ────────────────────────────────────────────────────────────

/// The DOM projection of the instances. The browser implements it in `dom`; headless runs have none.
/// An implementation may read and change the host (it is never borrowed when these are called) and
/// reports user input with [`Inner::push`].
pub(crate) trait Surface {
    fn opened(&self, inner: &Rc<Inner>, inst: u32, mode: crate::host::OpenMode, handle: Option<String>);
    fn closed(&self, inner: &Rc<Inner>, inst: u32);
    fn render(&self, inner: &Rc<Inner>);
    fn effect(&self, inner: &Rc<Inner>, e: &Effect);
    fn net_state(&self, inner: &Rc<Inner>, s: LinkState);
    /// The stylesheets, the asset table or the locale changed: redo what depends on them.
    fn styles_changed(&self, inner: &Rc<Inner>);
    fn fatal(&self, message: &str);
    fn unmount(&self);
    /// A change of page state the user's act asked for (an open menu, a sorted grid, a window).
    fn ui_op(&self, inner: &Rc<Inner>, inst: u32, ctl: usize, op: UiOp);
    /// A user's act the host cannot perform by itself: resolve a menu item, show a notice, open a window.
    fn act(&self, inner: &Rc<Inner>, inst: u32, ctl: usize, act: Act);
}

/// What the page needs besides the forms: stylesheets, the asset table, the locale. Kept here so
/// that they can arrive before the page is mounted and be replaced while it runs.
#[derive(Default, Clone)]
pub(crate) struct Styles {
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
    timers: RefCell<Vec<(u32, Timeout)>>,
    pub(crate) display_log: RefCell<Vec<String>>,
    pub(crate) errors: RefCell<Vec<crate::host::RunError>>,
    dead: Cell<bool>,
    weak: RefCell<std::rc::Weak<Inner>>,
    pub(crate) policy: crate::weblink::Policy,
    /// The session's token epoch the connection was last given (see `Host::auth_epoch`).
    auth_epoch_seen: Cell<u64>,
    /// Wakes the Authentication machine shortly before its token expires.
    auth_timer: RefCell<Option<Timeout>>,
    /// reCAPTCHA: the time limits and the script's load listeners of the tokens being obtained.
    pub(crate) captcha: RefCell<crate::dom::captcha::Pending>,
    /// The authorisation code the hosted flow came back with, until a control takes it.
    hosted_return: RefCell<Option<(String, String)>>,
}

impl Inner {
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
                    self.host.borrow_mut().dom_event(inst, ctl, name, args);
                }
                Act::Quiet { prop, value } => self.host.borrow_mut().ui_set(inst, ctl, &prop, &value),
                Act::Change { prop, value } => self.host.borrow_mut().user_change(inst, ctl, &prop, &value),
                Act::Prevent => {}
                Act::Auth(action) => self.host.borrow_mut().auth_act(inst, ctl, action),
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
                NetEvent::Message(t) => c.on_message(&t),
                NetEvent::Close => c.on_close(now),
                NetEvent::Error => c.on_error(now),
                NetEvent::Tick => c.tick(now),
            }
        }
        self.drain_net();
    }

    /// Hand the connection's answers to the host and set the timer for its next deadline.
    fn drain_net(self: &Rc<Self>) {
        let (outs, refresh, deadline) = {
            let mut guard = self.net.borrow_mut();
            let Some(c) = guard.as_mut() else { return };
            (c.take_out(), std::mem::take(&mut c.refresh_requested), c.next_deadline())
        };
        for o in outs {
            match o {
                Out::Answer { id, frame } if auth::wire::is_auth_id(id) => {
                    // An Authentication control's call (WASM-009): the answer is the machine's, not the module's.
                    let answered = self.host.borrow_mut().auth_answer(id, auth::wire::decode(&frame), now_ms() as f64);
                    if matches!(answered, Some(auth::BackendCall::SignOut)) {
                        // The server forgets who the connection was when it is closed: a later call opens a new one.
                        if let Some(c) = self.net.borrow_mut().as_mut() {
                            c.reset_session();
                        }
                    }
                }
                Out::Answer { id, frame } => self.host.borrow_mut().remote_answer(id, frame),
                Out::State(s) => {
                    if let Some(sf) = self.surface.borrow().as_ref() {
                        sf.net_state(self, s);
                    }
                }
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

    /// Call `func` of the part `part` (a module of its own the loader fetches the first time it is asked
    /// for) with one text, and answer its text (WASM-016 R13). An error is the part's own message.
    pub(crate) async fn call_part(&self, part: &str, func: &str, arg: &str) -> Result<String, String> {
        let link = self.link.borrow().clone().ok_or_else(|| "the player is not attached to a loader".to_owned())?;
        let f = link.part.as_ref().ok_or_else(|| "this page cannot load the player's parts".to_owned())?;
        let pending = f.call3(&link.this, &part.into(), &func.into(), &arg.into()).map_err(|e| js_message(&e))?;
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

    /// An Authentication control's backend call (WASM-009 R7).
    fn send_auth_call(self: &Rc<Self>, id: u32, call: auth::BackendCall) {
        if let Err(e) = self.net_submit(id, &auth::wire::encode(&call)) {
            // Nothing to send it through: the control reads it as any failed call (the user sees the generic message).
            let answered = self.host.borrow_mut().auth_answer(id, auth::AuthResult::Failed { detail: e }, now_ms() as f64);
            let _ = answered;
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
                        let f = net::error_frame(id, net::class::TRANSPORT, 7, &e, false);
                        self.host.borrow_mut().remote_answer(id, f);
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
                Effect::Opened { inst, mode, handle, .. } => {
                    if let Some(sf) = self.surface.borrow().as_ref() {
                        sf.opened(self, *inst, *mode, handle.clone());
                    }
                    continue;
                }
                Effect::Closed { inst } => {
                    if let Some(sf) = self.surface.borrow().as_ref() {
                        sf.closed(self, *inst);
                    }
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

    fn after_call(self: &Rc<Self>) {
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
        // Something may have arrived between the last look and the flag going down.
        if !inner.inbox.borrow().is_empty() || inner.host.borrow().pending_work() > 0 {
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
        let hosted_return = (!cfg.hosted_code.is_empty()).then(|| (cfg.hosted_code.clone(), cfg.hosted_state.clone()));
        let policy = crate::weblink::Policy { extra_schemes: cfg.weblink_schemes.clone() };
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
            timers: RefCell::new(Vec::new()),
            display_log: RefCell::new(Vec::new()),
            errors: RefCell::new(Vec::new()),
            dead: Cell::new(false),
            weak: RefCell::new(std::rc::Weak::new()),
            policy,
            auth_epoch_seen: Cell::new(0),
            auth_timer: RefCell::new(None),
            captcha: RefCell::new(Default::default()),
            hosted_return: RefCell::new(hosted_return),
        });
        *inner.weak.borrow_mut() = Rc::downgrade(&inner);
        if !inner.cfg.backend_origins.is_empty() {
            let mut ncfg = net::Config {
                origins: inner.cfg.backend_origins.clone(),
                unsafe_plain_ws: inner.cfg.unsafe_plain_ws,
                ..net::Config::default()
            };
            if let Some(t) = inner.cfg.call_timeout_ms {
                ncfg.call_timeout_ms = t;
            }
            let sock = WsSocket { inner: Rc::downgrade(&inner), ws: None, handlers: Vec::new() };
            *inner.net.borrow_mut() = Some(Client::new(ncfg, sock));
        }
        Ok(Player { inner })
    }

    /// Register the description (a `.cfrm`) of one form.
    pub fn add_form(&self, xml: &str) -> Result<(), JsError> {
        let form = load_form_from_str(xml).map_err(|e| JsError::new(&format!("a form description cannot be read: {e}")))?;
        self.inner.host.borrow_mut().add_form(form).map_err(|e| JsError::new(&e))
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
        *self.inner.link.borrow_mut() = Some(Rc::new(JsLink { this: link, perform, part }));
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
        Ok(())
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
                Some(id) if !sf.is_empty() => h.start_with(id),
                _ => h.start(),
            }
        }
        pump(&self.inner);
        Ok(())
    }

    // ── the module's imports ────────────────────────────────────────────────

    pub fn flush(&self, bytes: &[u8]) {
        self.inner.host.borrow_mut().flush(bytes);
    }

    /// The answer of the query that ends the batch, or `undefined` with [`Player::last_error`] set.
    pub fn query(&self, bytes: &[u8]) -> Option<Vec<u8>> {
        self.inner.host.borrow_mut().query(bytes).ok()
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
            for _ in 0..100_000 {
                // Look first, and start the driver only when there is something for it: starting it
                // sets `driving`, so pumping an idle player would make it look busy forever.
                let queued = !inner.inbox.borrow().is_empty() || inner.host.borrow().pending_work() > 0 || inner.host.borrow().has_yielded();
                if queued && !inner.driving.get() {
                    pump(&inner);
                }
                if !queued && !inner.driving.get() {
                    return Ok(JsValue::TRUE);
                }
                next_turn().await;
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
        self.inner.styles.borrow_mut().menus.insert(form, control, def);
        self.inner.styles_changed();
        Ok(())
    }

    pub fn set_locale(&self, locale: &str) {
        self.inner.styles.borrow_mut().locale = locale.to_owned();
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

    /// Unmount: close every instance, drop every listener and timer, close the socket (WASM-017 R5).
    /// The returned promise resolves when the module has been told.
    pub fn unmount(&self) -> Promise {
        let inner = self.inner.clone();
        future_to_promise(async move {
            if inner.dead.get() {
                return Ok(JsValue::from_str("{\"closed\":0,\"cancelled\":0}"));
            }
            // Wait for a call in flight to finish, then take everything down.
            for _ in 0..1000 {
                if !inner.driving.get() {
                    break;
                }
                next_turn().await;
            }
            let calls = inner.host.borrow_mut().teardown();
            inner.dead.set(true);
            if let Some(c) = inner.net.borrow_mut().as_mut() {
                c.shutdown();
            }
            *inner.net.borrow_mut() = None;
            *inner.net_timer.borrow_mut() = None;
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
    static PANIC_FN: RefCell<Option<Function>> = const { RefCell::new(None) };
}

/// Install the function the loader wants called when the player panics. The player is built with
/// `panic = "abort"`, so a panic ends the instance; this is how the page learns why.
#[wasm_bindgen]
pub fn install_panic_hook(f: Function) {
    PANIC_FN.with(|p| *p.borrow_mut() = Some(f));
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        web_sys::console::error_1(&format!("PowerRustCOBOL player panic: {msg}").into());
        PANIC_FN.with(|p| {
            if let Some(f) = p.borrow().as_ref() {
                let _ = f.call1(&JsValue::UNDEFINED, &msg.into());
            }
        });
    }));
}

/// Force a panic (the test of R15: the page must show the readable error and the reload action).
#[wasm_bindgen]
pub fn force_panic_for_test() {
    panic!("forced panic (test)");
}

/// The player's version triple (WASM-016 R11): the ABI, the form-description format, the player.
#[wasm_bindgen]
pub fn versions() -> String {
    serde_json::json!({
        "abi": cobolt_wasm_abi::ABI_VERSION,
        "description": 1,
        "player": env!("CARGO_PKG_VERSION"),
        "threaded": cfg!(feature = "threaded"),
    })
    .to_string()
}

#[cfg(feature = "threaded")]
pub use wasm_bindgen_rayon::init_thread_pool;
