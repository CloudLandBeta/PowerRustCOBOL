// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The ABI host** — the player's side of `specs/WASM-ABI-v1.md`.
//!
//! [`Host`] is a pure state machine. It owns the live form instances ([`FormTree`]), decodes and
//! applies the command batches the module flushes (§5), answers its queries (§6), keeps the
//! event queue and the task table, and decides what call into the module comes next. It never
//! touches the DOM, a socket or a timer: what the outside world must do is left in [`Effect`]s,
//! [`Outbound`] requests and the trees' change logs, and what the outside world learned is told
//! back through methods ([`Host::remote_answer`], [`Host::dialog_result`], [`Host::dom_event`]).
//! That is why the whole contract runs natively against a real module (`tests/abi_host.rs`).
//!
//! # The driving loop
//!
//! ```text
//! while let Some(call) = host.next_call() {        // pops the next piece of work
//!     let result = link.perform(&call);            // the module runs; its imports re-enter the host
//!     host.complete(call, result);                 // statuses, tasks, waits
//! }
//! ```
//!
//! The host is never borrowed across `perform`: the module's imports (`flush`, `query`, `remote`,
//! `abort`) re-enter it, which is why the browser driver keeps it in an `Rc<RefCell<_>>` and takes
//! the borrow for one call at a time.
//!
//! # Tasks (ABI §4 and amendment A5)
//!
//! A *task* is one run of a handler. It ends `DONE`, or it is left `SUSPENDED` (waiting for an
//! answer, a dialog, a child form) or `YIELDED` (budget exhausted). Tasks of **different form
//! instances** interleave — a modal child form's handlers must run while the caller's task waits
//! for it — while the events of **one instance** are serialised: they queue behind that
//! instance's live task and run in order once it is done (WASM-005 R9, WASM-010 AC5).

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::rc::Rc;

use cobolt_forms::model::Form;
use cobolt_wasm_abi::batch::{encode_event_args, BatchReader, Command, DecodeError, Path};
use cobolt_wasm_abi::codes::{ctrl, err, Status};
use cobolt_wasm_abi::manifest::{ArenaLayout, Manifest};

use crate::methods::{self, MethodError};
use crate::tree::{Change, EventReq, FormTree};

/// A request to call into the module. The driver performs it and reports a [`CallResult`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleCall {
    OpenForm { form: u32, opener: i32 },
    CloseForm { inst: u32 },
    /// `args` are `argc` encoded texts (§7) to be written at the start of the answer region.
    Event { inst: u32, ctrl: u32, event: u32, args: Vec<u8>, argc: u32 },
    /// `frame` is the answer frame to place in frame-in (a suspension on a remote call), if any.
    Resume { task: i32, frame: Option<Vec<u8>> },
    Cancel { task: i32 },
}

/// What performing a call produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallResult {
    /// `pr_open_form` returned `inst` (or an error), and `pr_status` said `status`.
    Opened { inst: i32, status: i32, task: i32 },
    /// `pr_event` / `pr_resume` / `pr_close_form` returned `status`; `task` is `pr_tick()` when it is
    /// `SUSPENDED` or `YIELDED`.
    Status { status: i32, task: i32 },
    /// `pr_cancel` returned.
    Unit,
    /// The module trapped (`unreachable`, an out-of-bounds access, …).
    Trapped(String),
}

/// What a suspended task waits for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wait {
    /// A remote request; `answer` is filled when it arrives.
    Remote { id: u32, answer: Option<Vec<u8>> },
    /// A dialog the shell shows (message box, input box).
    Dialog { id: u32, done: bool },
    /// `OpenFormSync`: the child form (a handle) must close.
    Child { handle: String, done: bool },
    /// A timed wait (`Sleep`); `done` when the shell's timer fired.
    Timer { id: u32, done: bool },
    /// The module suspended with nothing registered: it resumes on the next turn.
    Turn,
}

impl Wait {
    fn ready(&self) -> bool {
        match self {
            Wait::Remote { answer, .. } => answer.is_some(),
            Wait::Dialog { done, .. } | Wait::Child { done, .. } | Wait::Timer { done, .. } => *done,
            Wait::Turn => true,
        }
    }
}

/// A run-time error report: what the user (and a developer) is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunError {
    pub code: i32,
    pub message: String,
    pub form: Option<String>,
    pub event: Option<String>,
    pub unit: u32,
    pub line: u32,
}

impl RunError {
    /// `Run-time error 1001 in form main-form, onClick (unit 1, line 42): subscript out of range`.
    pub fn describe(&self) -> String {
        let mut s = format!("Run-time error {}", self.code);
        if let Some(f) = &self.form {
            s.push_str(&format!(" in form {f}"));
            if let Some(e) = &self.event {
                s.push_str(&format!(", {e}"));
            }
        }
        if self.line > 0 {
            s.push_str(&format!(" (unit {}, line {})", self.unit, self.line));
        }
        if !self.message.is_empty() {
            s.push_str(&format!(": {}", self.message));
        }
        s
    }
}

/// How a form opens (WASM-011 R2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode {
    /// SPA, `OpenFormSync`: an in-page modal dialog.
    Modal,
    /// SPA, `OpenFormAsync`: an in-page modeless panel.
    Panel,
    /// Standalone: a tab or window of its own.
    Window,
    /// The shell's ContentPane (a SideMenu load).
    Embedded,
    /// The first form of the application.
    Root,
}

/// Something the shell must do.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Display(String),
    Log { level: u32, text: String },
    RuntimeError(RunError),
    /// A form instance now exists and wants a place on the page.
    Opened { inst: u32, form: usize, mode: OpenMode, handle: Option<String> },
    /// An instance was destroyed: drop its DOM, listeners and timers.
    Closed { inst: u32 },
    /// A standalone open: open `url` in a tab/window of its own (WASM-011 R9/R10).
    OpenWindow { handle: String, form: usize, sync: bool },
    Dialog { id: u32, kind: DialogKind, text: String, title: String, buttons: String, default: String },
    /// A timed wait of `ms` started.
    Timer { id: u32, ms: u64 },
    /// `STOP RUN`: the application is over.
    Stopped,
    /// Move the keyboard focus (also available as a tree command; this is the form-level one).
    Title(String),
    /// A file the program offers for download.
    Download { name: String, text: String },
    Clipboard(String),
    Navigate { url: String, target: String },
    Fullscreen(bool),
    /// A spec-055 snackbar `Show()` etc. is a tree command; this is the host-level notice.
    Notice(String),
    /// An Authentication control needs a backend call made (WASM-009 R7); the answer comes back
    /// through [`Host::auth_answer`] under the same `id`.
    AuthCall { id: u32, call: crate::auth::BackendCall },
    /// An Authentication control needs a reCAPTCHA v3 token for `action`; the answer comes back
    /// through [`Host::auth_captcha`] (`None` when the script could not be loaded or did not answer).
    AuthCaptcha { id: u32, site_key: String, action: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    Message,
    Confirm,
    Input,
}

/// A request for the transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outbound {
    Request { id: u32, frame: Vec<u8> },
    Cancel { id: u32 },
}

/// Counters the tests and the Build report read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    pub flushes: u64,
    pub queries: u64,
    pub remotes: u64,
    pub aborts: u64,
    pub commands: u64,
    pub sets: u64,
    pub calls: u64,
    pub events_queued: u64,
    pub events_delivered: u64,
    pub events_coalesced: u64,
    pub events_dropped: u64,
    pub module_calls: u64,
    pub yields: u64,
    pub traps: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

#[derive(Debug, Clone)]
pub struct HostOptions {
    /// `LOG` commands at or below this level are kept.
    pub debug_level: u32,
    /// SPA or standalone (WASM-011).
    pub standalone: bool,
    /// The longest event queue before the oldest *movement* events are dropped.
    pub max_queue: usize,
}

impl Default for HostOptions {
    fn default() -> Self {
        HostOptions { debug_level: 0, standalone: false, max_queue: 4096 }
    }
}

#[derive(Debug, Clone)]
struct QueuedEvent {
    inst: u32,
    ctrl: u32,
    event: u32,
    args: Vec<String>,
}

#[derive(Debug, Clone)]
enum Work {
    Open { form: usize, opener: Option<u32>, mode: OpenMode, handle: Option<String> },
    Close { inst: u32 },
    Event(QueuedEvent),
    /// Discard the run that trapped (`pr_cancel(-1)`).
    Cancel,
}

#[derive(Debug, Clone)]
enum TaskState {
    Suspended(Wait),
    Yielded { resume: bool },
}

#[derive(Debug, Clone)]
struct Task {
    inst: u32,
    state: TaskState,
    /// A `pr_close_form` task: the instance is destroyed when it finishes.
    closing: bool,
}

#[derive(Debug, Clone)]
struct Handle {
    form: usize,
    inst: Option<u32>,
    sync: bool,
    closed: bool,
}

/// The call in flight, so that what the module does during it (the imports) is attributed.
#[derive(Debug, Clone)]
struct InFlight {
    /// The instance the call acts on, when known (`Event`, `Resume`, `CloseForm`).
    inst: Option<u32>,
    /// The form being opened, for `OpenForm`.
    opening: Option<(usize, Option<u32>, OpenMode, Option<String>)>,
    /// A wait registered during the call.
    wait: Option<Wait>,
    /// The resume was for this task.
    task: Option<i32>,
    event_desc: Option<(String, String)>,
}

/// The ABI host.
pub struct Host {
    pub manifest: Rc<Manifest>,
    layout: ArenaLayout,
    forms: Vec<Option<Rc<Form>>>,
    pub trees: BTreeMap<u32, FormTree>,
    work: VecDeque<Work>,
    tasks: BTreeMap<i32, Task>,
    inflight: Option<InFlight>,
    next_id: u32,
    handles: HashMap<String, Handle>,
    next_handle: u32,
    pub effects: Vec<Effect>,
    pub outbox: Vec<Outbound>,
    pub stats: Stats,
    pub opts: HostOptions,
    stopped: bool,
    /// The application object's own properties (`DialogResult`, `Locale`, …).
    app: HashMap<String, String>,
    last_error: i32,
    /// The signed-in session (WASM-009): the access token lives here, in memory only.
    pub auth: crate::auth::Session,
    /// Counts the changes of the session's token: the page gives the connection the new one when it moves.
    pub auth_epoch: u64,
    /// What the page says about Authentication controls that leave it open: the flow, the reCAPTCHA site key.
    auth_defaults: (crate::auth::Flow, String),
    /// Backend calls the Authentication controls made and are waiting for: `id → (instance, control, call)`.
    auth_calls: BTreeMap<u32, (u32, usize, crate::auth::BackendCall)>,
    /// reCAPTCHA tokens being obtained: `id → (instance, control)`.
    auth_captchas: BTreeMap<u32, (u32, usize)>,
    next_auth_id: u32,
    /// What the program asked of an Authentication control (`SignOut`, `Refresh`), until the driver gives the time.
    auth_queue: Vec<(u32, usize, crate::auth::Action)>,
    /// Instances whose close was requested while their own task was running.
    close_after: Vec<u32>,
    started: bool,
    /// `abort` was called during the call in flight: the trap that follows is already reported.
    aborted: bool,
    /// An error was already reported during the call in flight (an ERROR command, a bad command).
    reported_in_call: bool,
}

/// Events that fire in floods; a newer one replaces a queued one of the same target.
const COALESCE: &[&str] = &["onmousemove", "onmousewheel", "onresize", "onresized", "onmove", "onmoved", "onscroll"];

impl Host {
    pub fn new(manifest: Manifest, opts: HostOptions) -> Host {
        let layout = manifest.layout();
        let n = manifest.forms.len();
        Host {
            manifest: Rc::new(manifest),
            layout,
            forms: vec![None; n],
            trees: BTreeMap::new(),
            work: VecDeque::new(),
            tasks: BTreeMap::new(),
            inflight: None,
            next_id: 1,
            handles: HashMap::new(),
            next_handle: 1,
            effects: Vec::new(),
            outbox: Vec::new(),
            stats: Stats::default(),
            opts,
            stopped: false,
            app: HashMap::new(),
            last_error: 0,
            auth: crate::auth::Session::default(),
            auth_epoch: 0,
            auth_defaults: (crate::auth::Flow::InApp, String::new()),
            auth_calls: BTreeMap::new(),
            auth_captchas: BTreeMap::new(),
            next_auth_id: crate::auth::wire::ID_BASE,
            auth_queue: Vec::new(),
            close_after: Vec::new(),
            started: false,
            aborted: false,
            reported_in_call: false,
        }
    }

    pub fn layout(&self) -> ArenaLayout {
        self.layout
    }

    /// Register the description of form `name` (a parsed `.cfrm`).
    pub fn add_form(&mut self, form: Form) -> Result<(), String> {
        let Some(entry) = self.manifest.form_by_name(&form.name) else {
            return Err(format!("the form description `{}` is not a form of this program: rebuild the application", form.name));
        };
        let idx = entry.id as usize;
        self.forms[idx] = Some(Rc::new(form));
        Ok(())
    }

    /// Every form the program has must have a description before the application starts.
    pub fn missing_forms(&self) -> Vec<String> {
        self.manifest.forms.iter().filter(|f| self.forms[f.id as usize].is_none()).map(|f| f.name.clone()).collect()
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    pub fn last_error(&self) -> i32 {
        self.last_error
    }

    pub fn app_property(&self, name: &str) -> Option<&String> {
        self.app.get(&name.to_ascii_lowercase())
    }

    pub fn set_app_property(&mut self, name: &str, value: &str) {
        self.app.insert(name.to_ascii_lowercase(), value.to_owned());
    }

    // ── starting ────────────────────────────────────────────────────────────

    /// Queue the opening of the main form.
    pub fn start(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        let main = self.manifest.main_form().id as usize;
        self.work.push_back(Work::Open { form: main, opener: None, mode: OpenMode::Root, handle: None });
    }

    /// Queue the opening of a specific form (a deep link, a standalone page's start form).
    pub fn start_with(&mut self, form: usize) {
        if self.started {
            return;
        }
        self.started = true;
        self.work.push_back(Work::Open { form, opener: None, mode: OpenMode::Root, handle: None });
    }

    pub fn open_form(&mut self, form: usize, opener: Option<u32>, mode: OpenMode) {
        self.work.push_back(Work::Open { form, opener, mode, handle: None });
    }

    /// Ask for an instance to be destroyed (the user closed its dialog, navigated away, …).
    pub fn close_instance(&mut self, inst: u32) {
        if self.trees.contains_key(&inst) {
            self.work.push_back(Work::Close { inst });
        }
    }

    // ── events from the outside ─────────────────────────────────────────────

    /// A DOM event on control `ctl` (index in the tree) of instance `inst`, named as the model
    /// names it (`onClick`). Delivered only when the manifest binds a handler to it.
    pub fn dom_event(&mut self, inst: u32, ctl: usize, event: &str, args: Vec<String>) -> bool {
        let Some(tree) = self.trees.get(&inst) else { return false };
        let Some(c) = tree.ctls.get(ctl) else { return false };
        let Some(abi) = c.abi_id else { return false };
        if !c.has_handler(event) {
            return false;
        }
        let Some(ev) = self.manifest.event_index(event) else { return false };
        self.enqueue(inst, abi, ev, args);
        true
    }

    /// The user changed a property (typed in a box, ticked a check box, picked an item): store it
    /// without echoing it back to the DOM, then raise the observer events the property has — the
    /// same `onChange`/`onTextChanged`/… the desktop host raises for a user edit — for the handlers
    /// that are bound.
    pub fn user_change(&mut self, inst: u32, ctl: usize, prop: &str, value: &str) {
        let events: Vec<&'static str> = {
            let Some(tree) = self.trees.get_mut(&inst) else { return };
            if ctl >= tree.ctls.len() {
                return;
            }
            let changed = tree.get(ctl, prop) != value;
            tree.set_from_ui(ctl, prop, value);
            if !changed {
                return;
            }
            match tree.ctls[ctl].control_type() {
                Some(t) => t.observer_events_for(prop).into_iter().filter(|e| tree.ctls[ctl].has_handler(e)).collect(),
                None => Vec::new(),
            }
        };
        for e in events {
            self.dom_event(inst, ctl, e, Vec::new());
        }
    }

    /// The user's act changed a property whose observer events the act raises itself (a click on a
    /// list row sets `SelectedIndex` and then raises `onChange` and `onSelectedIndexChanged` in the
    /// order the desktop raises them): store it, raise nothing, and let the page redraw the control.
    pub fn ui_set(&mut self, inst: u32, ctl: usize, prop: &str, value: &str) {
        if let Some(tree) = self.trees.get_mut(&inst) {
            if ctl < tree.ctls.len() {
                tree.set_from_ui(ctl, prop, value);
            }
        }
    }

    fn enqueue(&mut self, inst: u32, ctrl: u32, event: u32, args: Vec<String>) {
        if self.stopped {
            return;
        }
        let name = self.manifest.events.get(event as usize).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
        if COALESCE.contains(&name.as_str()) {
            if let Some(Work::Event(last)) = self.work.iter_mut().rev().find(|w| matches!(w, Work::Event(e) if e.inst == inst)) {
                if last.ctrl == ctrl && last.event == event {
                    last.args = args;
                    self.stats.events_coalesced += 1;
                    return;
                }
            }
        }
        if self.work.len() >= self.opts.max_queue {
            // Drop the oldest movement event; if there is none, refuse the newest.
            if let Some(pos) = self.work.iter().position(|w| {
                matches!(w, Work::Event(e) if COALESCE.contains(&self.manifest.events.get(e.event as usize).map(|n| n.to_ascii_lowercase()).unwrap_or_default().as_str()))
            }) {
                self.work.remove(pos);
                self.stats.events_dropped += 1;
            } else {
                self.stats.events_dropped += 1;
                return;
            }
        }
        self.stats.events_queued += 1;
        self.work.push_back(Work::Event(QueuedEvent { inst, ctrl, event, args }));
    }

    fn enqueue_requests(&mut self, inst: u32, reqs: Vec<EventReq>) {
        for r in reqs {
            let abi = self.trees.get(&inst).and_then(|t| t.ctls.get(r.ctl)).and_then(|c| c.abi_id);
            if let (Some(abi), Some(ev)) = (abi, self.manifest.event_index(r.event)) {
                self.enqueue(inst, abi, ev, Vec::new());
            }
        }
    }

    // ── the transport tells the host ────────────────────────────────────────

    /// A remote request was answered (or failed: the frame then carries the error, ABI A6).
    pub fn remote_answer(&mut self, id: u32, frame: Vec<u8>) {
        for t in self.tasks.values_mut() {
            if let TaskState::Suspended(Wait::Remote { id: rid, answer }) = &mut t.state {
                if *rid == id {
                    *answer = Some(frame);
                    return;
                }
            }
        }
        // The answer can beat the module's own return from the suspending call.
        if let Some(f) = &mut self.inflight {
            if let Some(Wait::Remote { id: rid, answer }) = &mut f.wait {
                if *rid == id {
                    *answer = Some(frame);
                }
            }
        }
    }

    /// The shell closed a dialog.
    pub fn dialog_result(&mut self, id: u32, result: &str) {
        self.app.insert("dialogresult".into(), result.to_owned());
        for t in self.tasks.values_mut() {
            if let TaskState::Suspended(Wait::Dialog { id: did, done }) = &mut t.state {
                if *did == id {
                    *done = true;
                }
            }
        }
        if let Some(f) = &mut self.inflight {
            if let Some(Wait::Dialog { id: did, done }) = &mut f.wait {
                if *did == id {
                    *done = true;
                }
            }
        }
    }

    /// A timed wait ended.
    pub fn timer_fired(&mut self, id: u32) {
        for t in self.tasks.values_mut() {
            if let TaskState::Suspended(Wait::Timer { id: tid, done }) = &mut t.state {
                if *tid == id {
                    *done = true;
                }
            }
        }
    }

    /// A child form opened with `OpenFormSync` was closed (or its window was).
    pub fn child_closed(&mut self, handle: &str) {
        if let Some(h) = self.handles.get_mut(handle) {
            h.closed = true;
        }
        for t in self.tasks.values_mut() {
            if let TaskState::Suspended(Wait::Child { handle: h, done }) = &mut t.state {
                if h == handle {
                    *done = true;
                }
            }
        }
    }

    /// Let a yielded task run again (the driver calls this after giving the browser a turn).
    pub fn allow_yielded(&mut self) {
        for t in self.tasks.values_mut() {
            if let TaskState::Yielded { resume } = &mut t.state {
                *resume = true;
            }
        }
    }

    pub fn has_yielded(&self) -> bool {
        self.tasks.values().any(|t| matches!(t.state, TaskState::Yielded { .. }))
    }

    /// Is any task waiting on something that has not arrived?
    pub fn has_waiting(&self) -> bool {
        self.tasks.values().any(|t| matches!(&t.state, TaskState::Suspended(w) if !w.ready()))
    }

    pub fn is_idle(&self) -> bool {
        self.inflight.is_none() && self.work.is_empty() && self.tasks.is_empty()
    }

    pub fn pending_work(&self) -> usize {
        self.work.len()
    }

    pub fn live_tasks(&self) -> usize {
        self.tasks.len()
    }

    // ── choosing the next call ──────────────────────────────────────────────

    /// The next call to make into the module, or `None` when there is nothing to do right now.
    pub fn next_call(&mut self) -> Option<ModuleCall> {
        if self.inflight.is_some() {
            return None;
        }
        self.aborted = false;
        self.reported_in_call = false;
        // 1. A task whose wait is satisfied.
        let ready = self.tasks.iter().find_map(|(id, t)| match &t.state {
            TaskState::Suspended(w) if w.ready() => Some(*id),
            TaskState::Yielded { resume: true } => Some(*id),
            _ => None,
        });
        if let Some(id) = ready {
            let t = self.tasks.get_mut(&id).unwrap();
            let frame = match &mut t.state {
                TaskState::Suspended(Wait::Remote { answer, .. }) => answer.take(),
                _ => None,
            };
            let inst = t.inst;
            // The task stays in the table until the module says it is done; its state is "running".
            t.state = TaskState::Yielded { resume: false };
            self.inflight = Some(InFlight { inst: Some(inst), opening: None, wait: None, task: Some(id), event_desc: None });
            self.stats.module_calls += 1;
            return Some(ModuleCall::Resume { task: id, frame });
        }
        // 2. New work, in order; an event waits behind the live task of its own instance.
        let blocked: Vec<u32> = self.tasks.values().map(|t| t.inst).collect();
        let pos = self.work.iter().position(|w| match w {
            Work::Event(e) => !blocked.contains(&e.inst) && self.trees.contains_key(&e.inst),
            Work::Close { .. } | Work::Open { .. } | Work::Cancel => true,
        })?;
        let w = self.work.remove(pos)?;
        self.stats.module_calls += 1;
        match w {
            Work::Event(e) => {
                self.stats.events_delivered += 1;
                let desc = self.describe_event(e.inst, e.ctrl, e.event);
                self.inflight = Some(InFlight { inst: Some(e.inst), opening: None, wait: None, task: None, event_desc: Some(desc) });
                let refs: Vec<&str> = e.args.iter().map(String::as_str).collect();
                Some(ModuleCall::Event { inst: e.inst, ctrl: e.ctrl, event: e.event, args: encode_event_args(&refs), argc: e.args.len() as u32 })
            }
            Work::Open { form, opener, mode, handle } => {
                self.inflight = Some(InFlight {
                    inst: None,
                    opening: Some((form, opener, mode, handle)),
                    wait: None,
                    task: None,
                    event_desc: Some((self.manifest.forms[form].name.clone(), "onLoad".into())),
                });
                Some(ModuleCall::OpenForm { form: form as u32, opener: opener.map(|o| o as i32).unwrap_or(-1) })
            }
            Work::Cancel => {
                self.inflight = Some(InFlight { inst: None, opening: None, wait: None, task: None, event_desc: None });
                Some(ModuleCall::Cancel { task: -1 })
            }
            Work::Close { inst } => {
                // Cancel the instance's own tasks first (pending requests are the transport's to cancel).
                let own: Vec<i32> = self.tasks.iter().filter(|(_, t)| t.inst == inst).map(|(id, _)| *id).collect();
                if let Some(task) = own.first().copied() {
                    let t = self.tasks.remove(&task).unwrap();
                    if let TaskState::Suspended(Wait::Remote { id, .. }) = t.state {
                        self.outbox.push(Outbound::Cancel { id });
                    }
                    self.work.push_front(Work::Close { inst });
                    self.inflight = Some(InFlight { inst: Some(inst), opening: None, wait: None, task: None, event_desc: None });
                    return Some(ModuleCall::Cancel { task });
                }
                self.inflight = Some(InFlight {
                    inst: Some(inst),
                    opening: None,
                    wait: None,
                    task: None,
                    event_desc: self.trees.get(&inst).map(|t| (t.form_name.clone(), "onClose".to_owned())),
                });
                Some(ModuleCall::CloseForm { inst })
            }
        }
    }

    fn describe_event(&self, inst: u32, ctrl_id: u32, event: u32) -> (String, String) {
        let form = self.trees.get(&inst).map(|t| t.form_name.clone()).unwrap_or_default();
        let ev = self.manifest.events.get(event as usize).cloned().unwrap_or_default();
        let ctl = self.trees.get(&inst).and_then(|t| t.by_abi(ctrl_id).map(|i| t.ctls[i].name.clone())).unwrap_or_default();
        (form, if ctl.is_empty() || ctrl_id == ctrl::FORM { ev } else { format!("{ctl} {ev}") })
    }

    // ── a call finished ─────────────────────────────────────────────────────

    pub fn complete(&mut self, call: &ModuleCall, result: CallResult) {
        let flight = self.inflight.take();
        let inst = flight.as_ref().and_then(|f| f.inst);
        let wait = flight.as_ref().and_then(|f| f.wait.clone());
        let desc = flight.as_ref().and_then(|f| f.event_desc.clone());
        let resumed_task = flight.as_ref().and_then(|f| f.task);
        match (call, result) {
            (ModuleCall::OpenForm { .. }, CallResult::Opened { inst: id, status, task }) => {
                let (form, opener, mode, handle) = flight.as_ref().and_then(|f| f.opening.clone()).expect("open in flight");
                if id < 0 {
                    self.report(RunError {
                        code: id,
                        message: format!(
                            "{}: form `{}` allows {} instance(s) and all are in use",
                            err::name(id),
                            self.manifest.forms[form].name,
                            self.manifest.forms[form].instances
                        ),
                        form: Some(self.manifest.forms[form].name.clone()),
                        event: None,
                        unit: 0,
                        line: 0,
                    });
                    self.finish_failed_open(handle.as_deref());
                    return;
                }
                let id = id as u32;
                // The start-up handler may already have written to the new instance.
                if !self.trees.contains_key(&id) {
                    if let Err(e) = self.create_tree(id, form, opener) {
                        self.fail(e);
                        return;
                    }
                }
                if let Some(h) = &handle {
                    if let Some(hd) = self.handles.get_mut(h) {
                        hd.inst = Some(id);
                    }
                }
                self.effects.push(Effect::Opened { inst: id, form, mode, handle });
                self.after_status(Status::from_code(status), task, id, false, wait, desc, None);
            }
            (ModuleCall::OpenForm { .. }, CallResult::Trapped(msg)) | (ModuleCall::CloseForm { .. }, CallResult::Trapped(msg)) => {
                self.trapped(msg, desc);
                if let ModuleCall::CloseForm { inst } = call {
                    self.destroy(*inst);
                }
            }
            (ModuleCall::CloseForm { inst: i }, CallResult::Status { status, task }) => {
                self.after_status(Status::from_code(status), task, *i, true, wait, desc, None);
            }
            (ModuleCall::Event { inst: i, .. }, CallResult::Status { status, task }) => {
                self.after_status(Status::from_code(status), task, *i, false, wait, desc, None);
            }
            (ModuleCall::Resume { task: t, .. }, CallResult::Status { status, task }) => {
                let i = inst.unwrap_or(0);
                let closing = self.tasks.get(t).map(|x| x.closing).unwrap_or(false);
                let st = Status::from_code(status);
                // The task that was running is accounted for below.
                self.tasks.remove(t);
                let _ = resumed_task;
                self.after_status(st, task, i, closing, wait, desc, Some(closing));
            }
            (ModuleCall::Event { .. }, CallResult::Trapped(msg)) | (ModuleCall::Resume { .. }, CallResult::Trapped(msg)) => {
                if let ModuleCall::Resume { task, .. } = call {
                    self.tasks.remove(task);
                }
                self.trapped(msg, desc);
                // Best effort (ABI A5): `pr_cancel(-1)` discards the run that trapped, so the next
                // one starts from a clean batch cursor.
                self.work.push_front(Work::Cancel);
            }
            (ModuleCall::Cancel { .. }, _) => {}
            (call, result) => {
                self.report(RunError {
                    code: err::GENERIC,
                    message: format!("the driver reported {result:?} for {call:?}"),
                    form: None,
                    event: None,
                    unit: 0,
                    line: 0,
                });
            }
        }
        self.apply_deferred_closes();
    }

    fn apply_deferred_closes(&mut self) {
        let list = std::mem::take(&mut self.close_after);
        for inst in list {
            if !self.tasks.values().any(|t| t.inst == inst) {
                self.close_instance(inst);
            } else {
                self.close_after.push(inst);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn after_status(
        &mut self,
        st: Status,
        task: i32,
        inst: u32,
        closing: bool,
        wait: Option<Wait>,
        desc: Option<(String, String)>,
        _resumed: Option<bool>,
    ) {
        match st {
            Status::Done => {
                if closing {
                    self.destroy(inst);
                }
            }
            Status::Suspended => {
                let w = wait.unwrap_or(Wait::Turn);
                self.tasks.insert(task, Task { inst, state: TaskState::Suspended(w), closing });
            }
            Status::Yielded => {
                self.stats.yields += 1;
                self.tasks.insert(task, Task { inst, state: TaskState::Yielded { resume: false }, closing });
            }
            Status::Stopped => {
                self.stopped = true;
                self.work.clear();
                self.effects.push(Effect::Stopped);
            }
            Status::Error(code) => {
                // The module flushed an ERROR command first; make sure there is a report either way.
                if !self.reported_in_call {
                    self.report(RunError {
                        code,
                        message: err::name(code).to_owned(),
                        form: desc.as_ref().map(|d| d.0.clone()),
                        event: desc.as_ref().map(|d| d.1.clone()),
                        unit: 0,
                        line: 0,
                    });
                }
                if closing {
                    self.destroy(inst);
                }
            }
        }
        if closing && matches!(st, Status::Done) {
            // destroyed above
        }
    }

    fn trapped(&mut self, msg: String, desc: Option<(String, String)>) {
        self.stats.traps += 1;
        // `abort` reported the error first and the module then trapped, as ABI §3 says it does.
        if std::mem::take(&mut self.aborted) {
            return;
        }
        self.report(RunError {
            code: err::TRAPPED,
            message: format!("{} ({msg})", err::name(err::TRAPPED)),
            form: desc.as_ref().map(|d| d.0.clone()),
            event: desc.as_ref().map(|d| d.1.clone()),
            unit: 0,
            line: 0,
        });
    }

    fn fail(&mut self, e: String) {
        self.report(RunError { code: err::GENERIC, message: e, form: None, event: None, unit: 0, line: 0 });
    }

    fn finish_failed_open(&mut self, handle: Option<&str>) {
        if let Some(h) = handle {
            self.child_closed(h);
        }
    }

    fn report(&mut self, e: RunError) {
        self.last_error = e.code;
        self.reported_in_call = true;
        self.effects.push(Effect::RuntimeError(e));
    }

    fn create_tree(&mut self, inst: u32, form: usize, opener: Option<u32>) -> Result<(), String> {
        let Some(desc) = self.forms.get(form).cloned().flatten() else {
            return Err(format!("form `{}` has no description loaded", self.manifest.forms[form].name));
        };
        let mut tree = FormTree::build(&desc, &self.manifest.forms[form], &self.manifest, inst, opener).map_err(|e| e.to_string())?;
        tree.init_auth(self.auth_defaults.0, &self.auth_defaults.1, &self.auth);
        self.trees.insert(inst, tree);
        Ok(())
    }

    /// Drop an instance: its tree, its queued events, its tasks and their pending requests.
    fn destroy(&mut self, inst: u32) {
        if self.trees.remove(&inst).is_some() {
            self.effects.push(Effect::Closed { inst });
        }
        self.work.retain(|w| !matches!(w, Work::Event(e) if e.inst == inst));
        self.auth_calls.retain(|_, (i, _, _)| *i != inst);
        self.auth_captchas.retain(|_, (i, _)| *i != inst);
        self.auth_queue.retain(|(i, _, _)| *i != inst);
        let dead: Vec<i32> = self.tasks.iter().filter(|(_, t)| t.inst == inst).map(|(id, _)| *id).collect();
        for id in dead {
            if let Some(t) = self.tasks.remove(&id) {
                if let TaskState::Suspended(Wait::Remote { id, .. }) = t.state {
                    self.outbox.push(Outbound::Cancel { id });
                }
            }
        }
        let closed: Vec<String> =
            self.handles.iter().filter(|(_, h)| h.inst == Some(inst)).map(|(k, _)| k.clone()).collect();
        for h in closed {
            self.child_closed(&h);
        }
    }

    /// The whole application is being unmounted: drop everything and say what the module must be
    /// told (WASM-017 R5). Returns the calls to make: cancel every task, close every instance.
    pub fn teardown(&mut self) -> Vec<ModuleCall> {
        let mut calls = Vec::new();
        for (task, t) in std::mem::take(&mut self.tasks) {
            if let TaskState::Suspended(Wait::Remote { id, .. }) = t.state {
                self.outbox.push(Outbound::Cancel { id });
            }
            calls.push(ModuleCall::Cancel { task });
        }
        for inst in std::mem::take(&mut self.trees).keys() {
            self.effects.push(Effect::Closed { inst: *inst });
            calls.push(ModuleCall::CloseForm { inst: *inst });
        }
        self.work.clear();
        self.handles.clear();
        self.inflight = None;
        self.stopped = true;
        calls
    }

    // ── imports: the module calls the host ──────────────────────────────────

    /// `flush(len)`: apply a batch. Errors in a command are reported and the rest still apply.
    pub fn flush(&mut self, bytes: &[u8]) {
        self.stats.flushes += 1;
        self.stats.bytes_in += bytes.len() as u64;
        self.apply_batch(bytes, false);
    }

    /// `query(len)`: apply the commands, evaluate the final query, return its answer text as bytes.
    pub fn query(&mut self, bytes: &[u8]) -> Result<Vec<u8>, i32> {
        self.stats.queries += 1;
        self.stats.bytes_in += bytes.len() as u64;
        let answer = self.apply_batch(bytes, true);
        match answer {
            Some(Ok(text)) => {
                if text.len() > self.layout.answer_size as usize {
                    self.last_error = err::PAYLOAD_TOO_LARGE;
                    return Err(err::PAYLOAD_TOO_LARGE);
                }
                self.stats.bytes_out += text.len() as u64;
                Ok(text.into_bytes())
            }
            Some(Err(code)) => {
                self.last_error = code;
                Err(code)
            }
            None => {
                self.last_error = err::BAD_ARGUMENT;
                Err(err::BAD_ARGUMENT)
            }
        }
    }

    /// `remote(len)`: register the request and give the module its id.
    pub fn remote(&mut self, frame: &[u8]) -> Result<u32, i32> {
        self.stats.remotes += 1;
        if frame.len() > self.layout.frame_size as usize {
            return Err(err::PAYLOAD_TOO_LARGE);
        }
        let id = self.next_id;
        self.next_id += 1;
        self.outbox.push(Outbound::Request { id, frame: frame.to_vec() });
        if let Some(f) = &mut self.inflight {
            f.wait = Some(Wait::Remote { id, answer: None });
        }
        Ok(id)
    }

    /// `abort(len)`: the batch holds an ERROR command. Report it; the module then traps.
    pub fn abort(&mut self, bytes: &[u8]) {
        self.stats.aborts += 1;
        self.aborted = true;
        self.apply_batch(bytes, false);
    }

    // ── applying commands ───────────────────────────────────────────────────

    /// Returns the answer of the final query, when `want_query`.
    fn apply_batch(&mut self, bytes: &[u8], want_query: bool) -> Option<Result<String, i32>> {
        let mut answer: Option<Result<String, i32>> = None;
        let mut reader = BatchReader::new(bytes);
        while let Some(item) = reader.next() {
            let cmd = match item {
                Ok(c) => c,
                Err(e) => {
                    self.report_decode(e);
                    if want_query {
                        return Some(Err(err::BAD_ARGUMENT));
                    }
                    return None;
                }
            };
            self.stats.commands += 1;
            match cmd {
                Command::Set { path, value } => {
                    self.stats.sets += 1;
                    if let Err(e) = self.cmd_set(&path, value) {
                        self.report_cmd_error(e);
                    }
                }
                Command::Call { path } => {
                    self.stats.calls += 1;
                    if let Err(e) = self.cmd_call(&path).map(|_| ()) {
                        self.report_cmd_error(e);
                    }
                }
                Command::Display(t) => self.effects.push(Effect::Display(t.to_owned())),
                Command::Log { level, text } => {
                    if level <= self.opts.debug_level {
                        self.effects.push(Effect::Log { level, text: text.to_owned() });
                    }
                }
                Command::Error { code, message, unit, line, form, event } => {
                    let form_name = self.manifest.forms.get(form as usize).map(|f| f.name.clone());
                    let ev = self.manifest.events.get(event as usize).cloned();
                    // ABI amendment A4: below 1000 the code is the magnitude of a §9 code.
                    let (c, m) = if code < 1000 {
                        (-(code as i32), if message.is_empty() { err::name(-(code as i32)).to_owned() } else { message.to_owned() })
                    } else {
                        (code as i32, message.to_owned())
                    };
                    self.report(RunError { code: c, message: m, form: form_name, event: ev, unit, line });
                }
                q @ (Command::Get(_) | Command::CallR(_) | Command::Count(_)) => {
                    if want_query {
                        answer = Some(self.eval_query(&q));
                    } else {
                        self.report(RunError {
                            code: err::BAD_ARGUMENT,
                            message: "a query was flushed instead of being asked with `query`".into(),
                            form: None,
                            event: None,
                            unit: 0,
                            line: 0,
                        });
                    }
                }
            }
        }
        if want_query && answer.is_none() {
            return Some(Err(err::BAD_ARGUMENT));
        }
        answer
    }

    fn report_decode(&mut self, e: DecodeError) {
        self.report(RunError { code: err::BAD_ARGUMENT, message: format!("the module sent a malformed batch: {e}"), form: None, event: None, unit: 0, line: 0 });
    }

    fn report_cmd_error(&mut self, e: CmdError) {
        let (code, message) = match e {
            CmdError::Unknown(m) => (err::UNKNOWN_TARGET, m),
            CmdError::Refused(m) => (err::HOST_REFUSED, m),
            CmdError::Bad(m) => (err::BAD_ARGUMENT, m),
        };
        let form = self.inflight.as_ref().and_then(|f| f.event_desc.clone());
        self.report(RunError { code, message, form: form.as_ref().map(|d| d.0.clone()), event: form.map(|d| d.1), unit: 0, line: 0 });
    }

    fn name_of(&self, id: u32) -> Result<String, CmdError> {
        self.manifest.names.get(id as usize).cloned().ok_or_else(|| CmdError::Unknown(format!("name id {id} is not in the manifest's names table")))
    }

    /// Find the tree a path's `inst` names, creating it when an open of that form is in flight.
    fn ensure_tree(&mut self, inst: u32) -> Result<(), CmdError> {
        if self.trees.contains_key(&inst) {
            return Ok(());
        }
        if let Some((form, opener, _, _)) = self.inflight.as_ref().and_then(|f| f.opening.clone()) {
            return self.create_tree(inst, form, opener).map_err(CmdError::Unknown);
        }
        Err(CmdError::Unknown(format!("instance {inst} does not exist")))
    }

    fn resolve(&mut self, path: &Path<'_>) -> Result<Target, CmdError> {
        match path.ctrl {
            ctrl::APP => Ok(Target::App),
            ctrl::SUPER => {
                self.ensure_tree(path.inst)?;
                let opener = self.trees[&path.inst].opener.ok_or_else(|| {
                    CmdError::Refused("`super::` was used in a form that nothing opened (it is the first form)".into())
                })?;
                if !self.trees.contains_key(&opener) {
                    return Err(CmdError::Refused("the form that opened this one has been closed".into()));
                }
                Ok(Target::Ctl { inst: opener, idx: 0 })
            }
            c => {
                self.ensure_tree(path.inst)?;
                let t = &self.trees[&path.inst];
                let idx = t.by_abi(c).ok_or_else(|| CmdError::Unknown(format!("control id {c} is not in form `{}`", t.form_name)))?;
                Ok(Target::Ctl { inst: path.inst, idx })
            }
        }
    }

    fn cmd_set(&mut self, path: &Path<'_>, value: &str) -> Result<(), CmdError> {
        let target = self.resolve(path)?;
        let [seg] = path.segs.as_slice() else {
            return Err(CmdError::Bad(format!("a SET path of {} segments is not supported (one property per SET)", path.segs.len())));
        };
        let name = self.name_of(seg.name)?;
        match target {
            Target::App => {
                if !seg.args.is_empty() {
                    return Err(CmdError::Bad("the application object has no indexed properties".into()));
                }
                self.app_set(&name, value);
                Ok(())
            }
            Target::Ctl { inst, idx } => {
                let ev;
                {
                    let tree = self.trees.get_mut(&inst).unwrap();
                    if seg.args.is_empty() {
                        ev = tree.set(idx, &name, value);
                    } else if seg.args.len() == 1 {
                        // `LST::Items(3) = x`: replace the nth line of a newline-separated property.
                        let Some(n) = crate::value::index1(seg.args[0]) else {
                            return Err(CmdError::Bad(format!("`{name}({})`: an index counts from 1", seg.args[0])));
                        };
                        let cur = tree.get(idx, &name);
                        let mut lines: Vec<String> = cur.lines().map(str::to_owned).collect();
                        while lines.len() <= n {
                            lines.push(String::new());
                        }
                        lines[n] = value.to_owned();
                        ev = tree.set(idx, &name, &lines.join("\n"));
                    } else {
                        return Err(CmdError::Bad(format!("`{name}` takes at most one subscript in a SET")));
                    }
                }
                self.enqueue_requests(inst, ev);
                Ok(())
            }
        }
    }

    /// Call a method of a control from outside the program (the host page, the tests): the same call
    /// the program's `CALL` command makes, with the same events and the same answers.
    pub fn invoke(&mut self, inst: u32, control: &str, method: &str, args: &[&str]) -> Result<Option<String>, String> {
        let Some(idx) = self.trees.get(&inst).and_then(|t| t.by_name(control)) else {
            return Err(format!("no control `{control}` in instance {inst}"));
        };
        if crate::kinds::player_kind(&self.trees[&inst].ctls[idx]) == Some(crate::kinds::PlayerKind::Authentication) {
            return self.auth_method(inst, idx, method, args).map_err(|e| match e {
                CmdError::Unknown(m) | CmdError::Refused(m) | CmdError::Bad(m) => m,
            });
        }
        let r = {
            let tree = self.trees.get_mut(&inst).expect("checked");
            methods::call(tree, idx, method, args)
        };
        match r {
            Ok(o) => {
                self.enqueue_requests(inst, o.events);
                Ok(o.value)
            }
            Err(e) => Err(e.to_string()),
        }
    }

    fn cmd_call(&mut self, path: &Path<'_>) -> Result<Option<String>, CmdError> {
        let target = self.resolve(path)?;
        let Some(seg) = path.last() else {
            return Err(CmdError::Bad("a CALL needs a method name".into()));
        };
        let name = self.name_of(seg.name)?;
        let args: Vec<&str> = seg.args.clone();
        match target {
            Target::App => self.verb(path.inst, ctrl::APP, &name, &args),
            Target::Ctl { inst, idx } => {
                // Window-level verbs live on the form itself (ctrl 0) and on `super`.
                if idx == 0 {
                    if let Some(r) = self.form_verb(inst, &name, &args)? {
                        return Ok(r);
                    }
                }
                // Authentication is the host's: it holds the session (WASM-009).
                if crate::kinds::player_kind(&self.trees[&inst].ctls[idx]) == Some(crate::kinds::PlayerKind::Authentication) {
                    return self.auth_method(inst, idx, &name, &args);
                }
                let r = {
                    let tree = self.trees.get_mut(&inst).unwrap();
                    methods::call(tree, idx, &name, &args)
                };
                match r {
                    Ok(o) => {
                        self.enqueue_requests(inst, o.events);
                        Ok(o.value)
                    }
                    Err(MethodError::Unsupported(m)) => Err(CmdError::Unknown(m)),
                    Err(MethodError::Refused(m)) => Err(CmdError::Refused(m)),
                }
            }
        }
    }

    fn eval_query(&mut self, q: &Command<'_>) -> Result<String, i32> {
        match q {
            Command::Get(path) => {
                let r = (|| -> Result<String, CmdError> {
                    let target = self.resolve(path)?;
                    let [seg] = path.segs.as_slice() else {
                        return Err(CmdError::Bad("a GET path names exactly one property".into()));
                    };
                    let name = self.name_of(seg.name)?;
                    match target {
                        Target::App => Ok(self.app_get(&name)),
                        Target::Ctl { inst, idx } => {
                            let tree = &self.trees[&inst];
                            if seg.args.is_empty() {
                                if idx == 0 {
                                    if let Some(v) = self.form_get(inst, &name) {
                                        return Ok(v);
                                    }
                                }
                                Ok(tree.get(idx, &name))
                            } else {
                                // A subscripted read: the nth line of a newline-separated property.
                                let Some(n) = crate::value::index1(seg.args[0]) else { return Ok(String::new()) };
                                Ok(tree.get(idx, &name).lines().nth(n).unwrap_or("").to_owned())
                            }
                        }
                    }
                })();
                r.map_err(|e| self.query_error(e))
            }
            Command::CallR(path) => match self.cmd_call(path) {
                Ok(v) => Ok(v.unwrap_or_default()),
                Err(e) => Err(self.query_error(e)),
            },
            Command::Count(path) => {
                let r = (|| -> Result<String, CmdError> {
                    let target = self.resolve(path)?;
                    let name = self.name_of(path.last().map(|s| s.name).unwrap_or(0))?;
                    match target {
                        Target::App => Ok("0".into()),
                        Target::Ctl { inst, idx } => {
                            let text = self.trees[&inst].get(idx, &name);
                            let n = if text.trim().is_empty() { 0 } else { text.lines().count() };
                            Ok(n.to_string())
                        }
                    }
                })();
                r.map_err(|e| self.query_error(e))
            }
            _ => Err(err::BAD_ARGUMENT),
        }
    }

    fn query_error(&mut self, e: CmdError) -> i32 {
        let code = match &e {
            CmdError::Unknown(_) => err::UNKNOWN_TARGET,
            CmdError::Refused(_) => err::HOST_REFUSED,
            CmdError::Bad(_) => err::BAD_ARGUMENT,
        };
        self.report_cmd_error(e);
        code
    }

    // ── the application object and the form's window verbs ──────────────────

    fn app_get(&self, name: &str) -> String {
        match name.to_ascii_lowercase().as_str() {
            "dialogresult" => self.app.get("dialogresult").cloned().unwrap_or_default(),
            "standalone" => if self.opts.standalone { "1" } else { "0" }.to_owned(),
            other => self.app.get(other).cloned().unwrap_or_default(),
        }
    }

    fn app_set(&mut self, name: &str, value: &str) {
        let lower = name.to_ascii_lowercase();
        if lower == "title" {
            self.effects.push(Effect::Title(value.to_owned()));
        }
        self.app.insert(lower, value.to_owned());
    }

    fn form_get(&self, inst: u32, name: &str) -> Option<String> {
        match name.to_ascii_lowercase().as_str() {
            "formstate" => Some("Normal".to_owned()),
            "instance" => Some(inst.to_string()),
            _ => None,
        }
    }

    fn new_handle(&mut self, form: usize, sync: bool) -> String {
        let h = format!("H{}", self.next_handle);
        self.next_handle += 1;
        self.handles.insert(h.clone(), Handle { form, inst: None, sync, closed: false });
        h
    }

    /// Verbs of the form object (`me::`): opening other forms, closing this one, window state.
    /// `Ok(None)` when `name` is not a form verb (the control methods then apply to the form).
    fn form_verb(&mut self, inst: u32, name: &str, args: &[&str]) -> Result<Option<Option<String>>, CmdError> {
        let m = name.to_ascii_uppercase();
        match m.as_str() {
            "OPENFORMSYNC" | "OPENFORMASYNC" => {
                let sync = m == "OPENFORMSYNC";
                let id = args.first().map(|s| s.trim()).unwrap_or("");
                if id.is_empty() {
                    return Err(CmdError::Bad(format!("{name}: the form id argument is required")));
                }
                let form = self
                    .manifest
                    .form_by_name(id)
                    .map(|f| f.id as usize)
                    .ok_or_else(|| CmdError::Unknown(format!("{name}: the program has no form `{id}`")))?;
                let handle = self.new_handle(form, sync);
                let standalone = self.opts.standalone;
                if standalone {
                    self.effects.push(Effect::OpenWindow { handle: handle.clone(), form, sync });
                } else {
                    let mode = if sync { OpenMode::Modal } else { OpenMode::Panel };
                    self.work.push_back(Work::Open { form, opener: Some(inst), mode, handle: Some(handle.clone()) });
                }
                if sync {
                    if let Some(f) = &mut self.inflight {
                        f.wait = Some(Wait::Child { handle: handle.clone(), done: false });
                    }
                }
                Ok(Some(Some(handle)))
            }
            "CLOSE" => {
                // Closed after the running handler finishes: the handler may still be using it.
                if !self.close_after.contains(&inst) {
                    self.close_after.push(inst);
                }
                Ok(Some(None))
            }
            "SETTITLE" => {
                let t = args.first().copied().unwrap_or("").to_owned();
                if let Some(tree) = self.trees.get_mut(&inst) {
                    tree.set(0, "Title", &t);
                }
                Ok(Some(None))
            }
            "SETFULLSCREEN" => {
                self.effects.push(Effect::Fullscreen(crate::value::truthy(args.first().copied().unwrap_or("1"))));
                Ok(Some(None))
            }
            "SETFOCUS" | "FOCUS" | "SETTITLEVISIBLE" | "SETWINDOWSTATE" | "SETBREADCRUMBDETAIL" | "CLEARBREADCRUMBDETAIL" => {
                if let Some(tree) = self.trees.get_mut(&inst) {
                    tree.command(0, &m.to_ascii_lowercase(), args.iter().map(|s| (*s).to_owned()).collect());
                }
                Ok(Some(None))
            }
            "GETFORMSTATE" => Ok(Some(Some("Normal".into()))),
            _ => Ok(None),
        }
    }

    /// Verbs of the application object (`COBOL::`): dialogs, waits, navigation, downloads.
    fn verb(&mut self, inst: u32, _ctrl: u32, name: &str, args: &[&str]) -> Result<Option<String>, CmdError> {
        let m = name.to_ascii_uppercase();
        let arg = |i: usize| args.get(i).map(|s| s.to_string()).unwrap_or_default();
        match m.as_str() {
            "OPENFORMSYNC" | "OPENFORMASYNC" => Ok(self.form_verb(inst, name, args)?.flatten()),
            "MESSAGEBOX" | "ALERT" | "CONFIRM" | "INPUTBOX" => {
                let kind = match m.as_str() {
                    "CONFIRM" => DialogKind::Confirm,
                    "INPUTBOX" => DialogKind::Input,
                    _ => DialogKind::Message,
                };
                let id = self.next_id;
                self.next_id += 1;
                let (text, title, buttons, default) = if kind == DialogKind::Input {
                    (arg(0), arg(1), String::new(), arg(2))
                } else {
                    (arg(0), arg(1), arg(2), String::new())
                };
                self.effects.push(Effect::Dialog { id, kind, text, title, buttons, default });
                if let Some(f) = &mut self.inflight {
                    f.wait = Some(Wait::Dialog { id, done: false });
                }
                Ok(None)
            }
            "SLEEP" | "WAIT" => {
                let ms = arg(0).trim().parse::<u64>().unwrap_or(0);
                let id = self.next_id;
                self.next_id += 1;
                self.effects.push(Effect::Timer { id, ms });
                if let Some(f) = &mut self.inflight {
                    f.wait = Some(Wait::Timer { id, done: false });
                }
                Ok(None)
            }
            "NAVIGATE" | "OPENURL" => {
                self.effects.push(Effect::Navigate { url: arg(0), target: arg(1) });
                Ok(None)
            }
            "DOWNLOAD" => {
                self.effects.push(Effect::Download { name: arg(0), text: arg(1) });
                Ok(None)
            }
            "COPYTOCLIPBOARD" => {
                self.effects.push(Effect::Clipboard(arg(0)));
                Ok(None)
            }
            "SETTITLE" => {
                self.effects.push(Effect::Title(arg(0)));
                Ok(None)
            }
            "STOPRUN" | "EXIT" => {
                self.stopped = true;
                self.effects.push(Effect::Stopped);
                Ok(None)
            }
            "GETPROPERTY" => Ok(Some(self.app_get(&arg(0)))),
            "SETPROPERTY" => {
                self.app_set(&arg(0), &arg(1));
                Ok(None)
            }
            other => Err(CmdError::Unknown(format!("the application object has no verb `{other}`"))),
        }
    }

    /// The Authentication control's methods (WASM-009 R3): `SignOut`, `Refresh`, `GetClaim`.
    fn auth_method(&mut self, inst: u32, idx: usize, name: &str, args: &[&str]) -> Result<Option<String>, CmdError> {
        match name.to_ascii_uppercase().as_str() {
            "GETCLAIM" => Ok(Some(self.auth.claim(args.first().copied().unwrap_or("")))),
            // The machine acts when the driver next gives it the time (`auth_flush`): the module is
            // in the middle of a call now, and the answer of a backend is not its business.
            "SIGNOUT" => {
                self.auth_queue.push((inst, idx, crate::auth::Action::SignOut));
                Ok(None)
            }
            "REFRESH" => {
                self.auth_queue.push((inst, idx, crate::auth::Action::Refresh));
                Ok(None)
            }
            other => {
                let tree = self.trees.get_mut(&inst).unwrap();
                match methods::call(tree, idx, other, args) {
                    Ok(o) => Ok(o.value),
                    Err(e) => Err(CmdError::Unknown(e.to_string())),
                }
            }
        }
    }

    // ── the Authentication control (WASM-009) ───────────────────────────────

    /// What the page says for controls that leave it open: the flow (`Mode`) and the reCAPTCHA site key.
    pub fn set_auth_defaults(&mut self, flow: crate::auth::Flow, site_key: &str) {
        self.auth_defaults = (flow, site_key.to_owned());
    }

    /// The user acted on Authentication control `ctl` of instance `inst`.
    pub fn auth_act(&mut self, inst: u32, ctl: usize, action: crate::auth::Action) {
        use crate::auth::Action;
        let Some(ui) = self.trees.get_mut(&inst).and_then(|t| t.auth_mut(ctl)) else { return };
        match &action {
            Action::SignIn { user, .. } | Action::SignUp { user, .. } | Action::Forgot { user } | Action::Confirm { user, .. } | Action::Reset { user, .. } => {
                ui.user_hint = user.clone();
            }
            _ => {}
        }
        if matches!(action, Action::Show(_) | Action::SignOut | Action::Hosted) {
            ui.message.clear();
        }
        let ops = ui.machine.act(action);
        self.auth_run(inst, ctl, ops);
    }

    /// Do what the program asked of its Authentication controls (`SignOut`, `Refresh`).
    pub fn auth_flush(&mut self) {
        for (inst, ctl, action) in std::mem::take(&mut self.auth_queue) {
            self.auth_act(inst, ctl, action);
        }
    }

    /// The control that owns the session: the first signed-in one.
    fn auth_owner(&self) -> Option<(u32, usize)> {
        self.trees.iter().find_map(|(inst, t)| t.auths.iter().find(|(_, ui)| ui.machine.session.is_signed_in()).map(|(c, _)| (*inst, *c)))
    }

    /// The connection says the token expired: refresh it now (R5).
    pub fn auth_refresh(&mut self) {
        if let Some((inst, ctl)) = self.auth_owner() {
            self.auth_act(inst, ctl, crate::auth::Action::Refresh);
        }
    }

    /// The clock ticked: the owner refreshes a token that is about to expire (R5).
    pub fn auth_tick(&mut self, now_ms: f64) {
        if let Some((inst, ctl)) = self.auth_owner() {
            self.auth_act(inst, ctl, crate::auth::Action::Tick { now_ms });
        }
    }

    /// When [`Host::auth_tick`] next has something to do (ms since the epoch), if a session is open.
    pub fn auth_deadline(&self) -> Option<f64> {
        let (inst, ctl) = self.auth_owner()?;
        let ui = self.trees.get(&inst)?.auth(ctl)?;
        ui.machine.session.expires_at_ms.map(|e| e - 60_000.0)
    }

    /// The backend answered call `id`; answers which call it was (`None` when nobody was waiting).
    pub fn auth_answer(&mut self, id: u32, result: crate::auth::AuthResult, now_ms: f64) -> Option<crate::auth::BackendCall> {
        let (inst, ctl, call) = self.auth_calls.remove(&id)?;
        // The form that asked may have been closed meanwhile: a late answer has nowhere to go.
        let Some(ui) = self.trees.get_mut(&inst).and_then(|t| t.auth_mut(ctl)) else { return Some(call) };
        ui.attempt += 1;
        ui.message.clear();
        let ops = ui.machine.on_result(&call, result, now_ms);
        self.auth_run(inst, ctl, ops);
        Some(call)
    }

    /// The hosted flow came back to the page with an authorisation code: give it to the first
    /// Authentication control there is. `false` when no control exists (yet).
    pub fn auth_hosted_return(&mut self, code: &str, state: &str) -> bool {
        let target = self.trees.iter().find_map(|(inst, t)| t.auths.keys().next().map(|c| (*inst, *c)));
        let Some((inst, ctl)) = target else { return false };
        self.auth_act(inst, ctl, crate::auth::Action::HostedFinish { code: code.to_owned(), state: state.to_owned() });
        true
    }

    /// Is there anything for the driver to hand on: effects to carry out, requests to send?
    pub fn has_outgoing(&self) -> bool {
        !self.effects.is_empty() || !self.outbox.is_empty()
    }

    /// reCAPTCHA gave a token for the call that waited for it (or `None`).
    pub fn auth_captcha(&mut self, id: u32, token: Option<String>) {
        let Some((inst, ctl)) = self.auth_captchas.remove(&id) else { return };
        let Some(ui) = self.trees.get_mut(&inst).and_then(|t| t.auth_mut(ctl)) else { return };
        ui.attempt += 1;
        ui.message.clear();
        let ops = ui.machine.captcha_token(token);
        self.auth_run(inst, ctl, ops);
    }

    /// Carry out what a machine asked for.
    fn auth_run(&mut self, inst: u32, ctl: usize, ops: Vec<crate::auth::Op>) {
        use crate::auth::{AuthEvent, Op};
        for op in ops {
            match op {
                Op::Backend(call) => {
                    let id = self.next_auth_id;
                    self.next_auth_id = self.next_auth_id.wrapping_add(1).max(crate::auth::wire::ID_BASE);
                    self.auth_calls.insert(id, (inst, ctl, call.clone()));
                    self.effects.push(Effect::AuthCall { id, call });
                }
                Op::Captcha { action } => {
                    let id = self.next_auth_id;
                    self.next_auth_id = self.next_auth_id.wrapping_add(1).max(crate::auth::wire::ID_BASE);
                    let site_key = self.trees.get(&inst).and_then(|t| t.auth(ctl)).map(|u| u.machine.cfg.recaptcha_site_key.clone()).unwrap_or_default();
                    self.auth_captchas.insert(id, (inst, ctl));
                    self.effects.push(Effect::AuthCaptcha { id, site_key, action });
                }
                Op::Notify(ev) => {
                    // The detail of an error is the class and code of the backend's answer (never its text).
                    let args = match &ev {
                        AuthEvent::Error(detail) => vec![detail.clone()],
                        _ => Vec::new(),
                    };
                    self.dom_event(inst, ctl, ev.handler_name(), args);
                }
                Op::Message(m) => {
                    if let Some(ui) = self.trees.get_mut(&inst).and_then(|t| t.auth_mut(ctl)) {
                        ui.message = m;
                    }
                }
                Op::Redirect(url) => {
                    if crate::auth::redirect_allowed(&url) {
                        self.effects.push(Effect::Navigate { url, target: "_self".into() });
                    } else {
                        // The provider's address came over the network: refuse anything but https.
                        let generic = self.trees.get(&inst).and_then(|t| t.auth(ctl)).map(|u| u.machine.cfg.texts.generic_error.clone()).unwrap_or_default();
                        if let Some(ui) = self.trees.get_mut(&inst).and_then(|t| t.auth_mut(ctl)) {
                            ui.message = generic;
                        }
                        self.dom_event(inst, ctl, AuthEvent::Error(String::new()).handler_name(), vec!["redirect refused".into()]);
                    }
                }
            }
        }
        self.auth_sync(inst, ctl);
    }

    /// After a machine moved: publish its properties, make the application's session the same, and
    /// let the other Authentication controls follow it.
    fn auth_sync(&mut self, inst: u32, ctl: usize) {
        let Some(session) = self.trees.get(&inst).and_then(|t| t.auth(ctl)).map(|u| u.machine.session.clone()) else { return };
        let moved = self.auth.token() != session.token();
        self.auth = session.clone();
        if moved {
            self.auth_epoch += 1;
        }
        if let Some(t) = self.trees.get_mut(&inst) {
            t.sync_auth_props(ctl);
        }
        if moved {
            for (i, t) in self.trees.iter_mut() {
                let others: Vec<usize> = t.auths.keys().copied().filter(|c| !(*i == inst && *c == ctl)).collect();
                for c in others {
                    if let Some(ui) = t.auth_mut(c) {
                        ui.machine.adopt(&session);
                        ui.message.clear();
                    }
                    t.sync_auth_props(c);
                }
            }
        }
    }

    // ── the renderer reads ──────────────────────────────────────────────────

    /// Collect every tree's pending changes: `(instance, change)` in order.
    pub fn take_changes(&mut self) -> Vec<(u32, Change)> {
        let mut out = Vec::new();
        for (inst, t) in self.trees.iter_mut() {
            for c in t.take_changes() {
                out.push((*inst, c));
            }
        }
        out
    }

    pub fn has_changes(&self) -> bool {
        self.trees.values().any(FormTree::has_changes)
    }

    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    pub fn take_outbox(&mut self) -> Vec<Outbound> {
        std::mem::take(&mut self.outbox)
    }

    /// The state of every instance, for tests: `{"<inst>": {"form": …, "controls": {…}}}`.
    pub fn snapshot(&self) -> serde_json::Value {
        let mut m = serde_json::Map::new();
        for (inst, t) in &self.trees {
            let mut o = serde_json::Map::new();
            o.insert("form".into(), t.form_name.clone().into());
            o.insert("controls".into(), t.snapshot());
            m.insert(inst.to_string(), serde_json::Value::Object(o));
        }
        serde_json::Value::Object(m)
    }
}

#[cfg(test)]
mod auth_tests;

#[derive(Debug)]
enum Target {
    App,
    Ctl { inst: u32, idx: usize },
}

#[derive(Debug)]
pub enum CmdError {
    Unknown(String),
    Refused(String),
    Bad(String),
}
