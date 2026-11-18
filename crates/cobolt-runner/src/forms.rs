// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The runner's form engine** (DESKTOP-WASM-001 T30; R9, R10, R28 runner half).
//!
//! A desktop module with forms is driven as the browser drives one, by the ABI host of
//! `cobolt-abi-host`, on **one program thread** that owns the module's instance. Its windows are the
//! form host's, the very windows a program built on the Cargo path shows. The two meet at the form
//! host's [`ProgramEngine`] seam (T29): for each form instance the host opens, the engine is handed
//! the instance's channels and joins them to an instance of the module's form.
//!
//! What crosses, for each instance, is what crosses for the interpreter:
//!
//! * **events** the window raises become the ABI host's `dom_event`s, after the values the user
//!   edited (the `input` channel) are stored without being echoed back (`ui_set`);
//! * **property changes** the module makes reach the window as `StateUpdate`s, the property
//!   spelled as COBOL spells it (in capitals) and the value as the program stored it;
//! * `DISPLAY` lines, and a run-time error's report, go to the window's display channel;
//! * the run's end (`STOP RUN`) and the instance's close set `finished`.
//!
//! As for the interpreter, the window raises a write's observer events (`onChange`, …) and the
//! form's `onShow` and `onActivate`: the ABI host is told it runs on the desktop
//! ([`HostOptions::desktop`]). An event leaves `pending` when it is taken, before its handler runs,
//! and an input wake only drains the input, so the window's headless scripts know when the program
//! has handled what it was sent.
//!
//! **Several forms** (T31). Every form shows in a window or a pane the form host made, as the
//! interpreter's do:
//!
//! * a form the form host opens itself (the first form, a SideMenu's row, a headless `open_form`)
//!   asks the engine for its program; the module opens an instance of it, whose opener, the form
//!   `super::` names, is the form whose handle the host names as its `super_handle`;
//! * a form the program opens (`OpenFormSync`, `OpenFormAsync`) gets its window first: the runner
//!   asks the form host's supervisor for it, as the interpreter asks (`FormRequest::OpenForm`,
//!   placed as the program said), and the module opens the form only when the window comes, so its
//!   program starts in its window and the window shows from its first frame, as the interpreter's
//!   does. A modal open's caller waits, in the module, until the instance is gone;
//! * a form the program closes (`ME::"Close"()`) asks the supervisor to close its window
//!   (`FormRequest::CloseSelf`); a window the host closes ends its instance in the module;
//! * the window methods the ABI host keeps as commands (`SetWindowState`, `Focus`,
//!   `SetTitleVisible`, the breadcrumb's detail) go to the supervisor as the interpreter's do
//!   (`FormRequest::HandleMethod`);
//! * a write to a layer reaches the window under the layer's name, as the interpreter writes the
//!   object of the layer's name.
//!
//! **File dialogs** (T32). A program's `COBOL::"OPEN-FILE-DIALOG"`, `"SAVE-FILE-DIALOG"` or
//! `"FOLDER-DIALOG"` is sent to the form host as the interpreter sends it (`FormRequest::FileDialog`,
//! its arguments read by the same `file_dialog_ask`), and the form host shows it, or answers it
//! from a headless script's `answer` step. The program waits in the module, so its form takes no
//! event meanwhile, while the thread serves the other forms; the answer comes back as a message.

use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use cobolt_abi_host::host::{CallResult, Effect, Host, HostOptions, ModuleCall, OpenMode};
use cobolt_abi_host::tree::Change;
use cobolt_form_host::engine::{ProgramEngine, ProgramRun, ProgramStart};
use cobolt_runtime::form_host::{file_dialog_ask, file_dialog_kind, FormRequest, FormSupervisor};
use cobolt_runtime::{FormEvent, StateUpdate};
use cobolt_wasm_abi::manifest::{ArenaLayout, Manifest};
use wasmtime::{Engine, Instance, Module, Store};

use crate::{Lang, Msg, Pins, State};

/// A form module's ABI host, and where its arena's regions are (the `flush`, `query` and `abort`
/// imports read and write there).
pub(crate) struct FormRun {
    pub(crate) host: Host,
    pub(crate) layout: ArenaLayout,
    /// Where the graphics region is in the arena and how long it is (ABI A23), when the module has one.
    pub(crate) gfx: Option<(u32, u32)>,
    /// The canvases and graphics objects of the run (T50).
    pub(crate) graphics: crate::gfx::Shared,
}

/// What a form program's `now` answers: the local date and time of the interpreter's clock
/// (`cobolt_runtime::test_pins`), which a deterministic headless run moves with its virtual time.
pub(crate) fn now_ms() -> f64 {
    crate::pins::wall_ms(cobolt_runtime::test_pins::now().naive_local())
}

/// What a form program's `tz_offset` answers: the same clock's offset from UTC, in minutes east.
pub(crate) fn offset_minutes() -> i32 {
    cobolt_runtime::test_pins::now().offset().local_minus_utc() / 60
}

/// What reaches the program thread.
enum ToProgram {
    /// The form host opened an instance of the form `form`: run it with these channels.
    Open { form: String, run: Box<ProgramRun> },
    /// The window of link `link` raised an event.
    Event { link: usize, ev: FormEvent },
    /// The window of link `link` is gone: its event channel closed.
    Gone { link: usize },
    /// The operator chose `path` in file dialog `id` (T32), or cancelled it.
    Answered { id: u32, path: Option<String> },
    /// An asynchronous AWS operation ended (T48).
    Aws(crate::aws::AwsResult),
    /// A SqlDatabase statement on a thread ended or ran out of time (T41c).
    Sql(crate::sql::SqlNews),
}

/// The engine the runner gives the form host: the forms' designs, from the module's `prforms`
/// section, and their programs, run on the one program thread.
#[derive(Clone)]
pub struct WasmEngine {
    forms: Arc<Vec<cobolt_forms::Form>>,
    tx: mpsc::Sender<ToProgram>,
    /// The code `prn.exit` received, once the program handed one over.
    exit: Arc<AtomicI32>,
}

/// No exit code was handed over.
const NO_EXIT: i32 = i32::MIN;

impl WasmEngine {
    /// Start the program thread for `module` (checked: ABI `abi`, imports `imports`), whose manifest
    /// is `manifest`, with the forms' designs `forms` (the main form first). Answers once the module
    /// is instantiated and started, or why it could not be.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        engine: Engine,
        module: Module,
        abi: u32,
        imports: Vec<(String, String)>,
        manifest: Manifest,
        forms: Vec<cobolt_forms::Form>,
        files: crate::files::FileSetup,
        sql: Option<crate::sql::SqlSetup>,
        lang: Lang,
        pins: Pins,
    ) -> Result<WasmEngine, Msg> {
        let (tx, rx) = mpsc::channel::<ToProgram>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), Msg>>();
        let exit = Arc::new(AtomicI32::new(NO_EXIT));
        let thread = Thread { engine, module, abi, imports, manifest, forms: forms.clone(), files, sql, lang, pins, rx, tx: tx.clone(), exit: Arc::clone(&exit) };
        std::thread::Builder::new()
            .name("program".into())
            // The interpreter's program stack: a module's own stack is in its memory, but the host's
            // calls into it nest as deeply as the interpreter's PERFORMs do.
            .stack_size(cobolt_runtime::stack_guard::PROGRAM_STACK_BYTES)
            .spawn(move || thread.run(ready_tx))
            .map_err(|e| Msg::CannotStart(format!("cannot start the program thread: {e}")))?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(WasmEngine { forms: Arc::new(forms), tx, exit }),
            Ok(Err(msg)) => Err(msg),
            Err(_) => Err(Msg::CannotStart("the program thread ended before the program started".into())),
        }
    }

    /// The code the program handed to `prn.exit`, if it did.
    pub fn exit_code(&self) -> Option<i32> {
        let c = self.exit.load(Ordering::SeqCst);
        (c != NO_EXIT).then_some(c)
    }

    fn index_of(&self, form_id: &str) -> Result<usize, String> {
        let want = form_id.trim();
        self.forms
            .iter()
            .position(|f| f.name.trim().eq_ignore_ascii_case(want))
            .ok_or_else(|| format!("no form named '{want}' in this application"))
    }
}

impl ProgramEngine for WasmEngine {
    fn form(&self, form_id: &str) -> Result<cobolt_forms::Form, String> {
        self.index_of(form_id).map(|i| self.forms[i].clone())
    }

    fn open(&self, form_id: &str) -> Result<(cobolt_forms::Form, ProgramStart), String> {
        let form = self.form(form_id)?;
        let tx = self.tx.clone();
        let name = form.name.clone();
        let start: ProgramStart = Box::new(move |run: ProgramRun| {
            tx.send(ToProgram::Open { form: name, run: Box::new(run) }).map_err(|_| "the program has ended: no form can open".to_string())
        });
        Ok((form, start))
    }
}

/// One form instance the form host opened, joined to an instance of the module's form.
struct Link {
    /// The form, as the manifest names it.
    form: usize,
    /// The form object, in capitals: what a write to the form itself is addressed to.
    object: String,
    /// The module's instance, once it is open.
    inst: Option<u32>,
    /// The instance's handle in the form host's supervisor.
    handle: String,
    /// Requests to the form host: windows for the forms the program opens, window methods.
    requests: mpsc::Sender<FormRequest>,
    /// The window is gone, or going: nothing more is asked of it.
    gone: bool,
    /// Events the window raised that the program has not taken: it takes none while its instance
    /// is busy in a handler, as the interpreter's program does not.
    queue: VecDeque<FormEvent>,
    state: mpsc::Sender<StateUpdate>,
    display: mpsc::Sender<String>,
    input: mpsc::Receiver<StateUpdate>,
    pending: Arc<AtomicUsize>,
    finished: Arc<AtomicBool>,
}

impl Link {
    fn finish(&self) {
        self.finished.store(true, Ordering::Relaxed);
    }

    /// One event fewer waits for the program: it took it, or it never will.
    fn taken(&self) {
        let _ = self.pending.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_sub(1));
    }

    /// The events left in the queue will never be taken (the instance is gone).
    fn drop_queue(&mut self) {
        for _ in self.queue.drain(..) {
            let _ = self.pending.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_sub(1));
        }
    }
}

/// Everything the program thread is started with.
struct Thread {
    engine: Engine,
    module: Module,
    abi: u32,
    imports: Vec<(String, String)>,
    manifest: Manifest,
    forms: Vec<cobolt_forms::Form>,
    files: crate::files::FileSetup,
    sql: Option<crate::sql::SqlSetup>,
    lang: Lang,
    pins: Pins,
    rx: mpsc::Receiver<ToProgram>,
    tx: mpsc::Sender<ToProgram>,
    exit: Arc<AtomicI32>,
}

/// The module's instance and the regions of its arena: what performs the calls the ABI host asks for.
struct Driver {
    store: Store<State>,
    instance: Instance,
    arena: wasmtime::Memory,
    base: u32,
    layout: ArenaLayout,
}

impl Driver {
    fn host(&mut self) -> &mut Host {
        &mut self.store.data_mut().forms.as_mut().expect("a form module's state has its host").host
    }

    fn graphics(&self) -> crate::gfx::Shared {
        std::rc::Rc::clone(&self.store.data().forms.as_ref().expect("a form module's state has its host").graphics)
    }

    fn call<P: wasmtime::WasmParams, R: wasmtime::WasmResults>(&mut self, name: &str, params: P) -> wasmtime::Result<R> {
        self.instance.get_typed_func::<P, R>(&mut self.store, name)?.call(&mut self.store, params)
    }

    /// `pr_tick()` when `status` left a task (suspended or yielded), else -1.
    fn task(&mut self, status: i32) -> wasmtime::Result<i32> {
        if status == 1 || status == 2 {
            self.call::<(), i32>("pr_tick", ())
        } else {
            Ok(-1)
        }
    }

    /// Perform one call the ABI host asked for (ABI v1 §4), as the browser's loader does.
    fn perform(&mut self, call: &ModuleCall) -> CallResult {
        // A file the program opens belongs to the instance the call acts on (T38).
        self.store.data_mut().files.begin_call(call_inst(call));
        if let Some(sql) = self.store.data_mut().sql.as_mut() {
            sql.begin_call(call_inst(call));
        }
        self.store.data_mut().db.borrow_mut().begin_call(call_inst(call));
        self.store.data_mut().http.borrow_mut().begin_call(call_inst(call));
        let r = (|| -> wasmtime::Result<CallResult> {
            Ok(match call {
                ModuleCall::OpenForm { form, opener } => {
                    let inst = self.call::<(i32, i32), i32>("pr_open_form", (*form as i32, *opener))?;
                    if let Ok(inst) = u32::try_from(inst) {
                        self.store.data_mut().files.opened_form(inst);
                        if let Some(sql) = self.store.data_mut().sql.as_mut() {
                            sql.opened_form(inst);
                        }
                        self.store.data_mut().db.borrow_mut().opened_form(inst);
                        self.store.data_mut().http.borrow_mut().opened_form(inst);
                    }
                    let status = self.call::<(), i32>("pr_status", ())?;
                    let task = self.task(status)?;
                    CallResult::Opened { inst, status, task }
                }
                ModuleCall::CloseForm { inst } => {
                    let status = self.call::<i32, i32>("pr_close_form", *inst as i32)?;
                    // A closed window's program lets go of the files it left open, as the
                    // interpreter of a closed form drops its own; a close still running (a task) has
                    // not closed the window yet.
                    if status != 1 && status != 2 {
                        self.store.data_mut().files.forget_instance(*inst);
                        // Its SQL cursors and prepared statements too; the main window's ends the run unit (T41).
                        if let Some(sql) = self.store.data_mut().sql.as_mut() {
                            sql.forget_instance(*inst);
                        }
                        // And the connections its SQL built-ins opened (T41b).
                        self.store.data_mut().db.borrow_mut().forget_instance(*inst);
                        // And its HTTP client (T42).
                        self.store.data_mut().http.borrow_mut().forget_instance(*inst);
                    }
                    CallResult::Status { status, task: self.task(status)? }
                }
                ModuleCall::Event { inst, ctrl, event, args, argc } => {
                    self.arena.write(&mut self.store, (self.base + self.layout.answer_off) as usize, args)?;
                    let status = self.call::<(i32, i32, i32, i32), i32>("pr_event", (*inst as i32, *ctrl as i32, *event as i32, *argc as i32))?;
                    CallResult::Status { status, task: self.task(status)? }
                }
                ModuleCall::Resume { task, frame } => {
                    let len = match frame {
                        Some(f) => {
                            self.arena.write(&mut self.store, (self.base + self.layout.frame_in_off) as usize, f)?;
                            f.len() as i32
                        }
                        None => 0,
                    };
                    let status = self.call::<(i32, i32), i32>("pr_resume", (*task, len))?;
                    CallResult::Status { status, task: self.task(status)? }
                }
                ModuleCall::Cancel { task } => {
                    self.call::<i32, ()>("pr_cancel", *task)?;
                    // As the loader answers: what the module says after the cancel.
                    let status = self.call::<(), i32>("pr_status", ())?;
                    CallResult::Status { status, task: self.call::<(), i32>("pr_tick", ())? }
                }
            })
        })();
        r.unwrap_or_else(|e| CallResult::Trapped(format!("{e:#}")))
    }
}

/// The copy behind a FileDropZone's `CommitFiles()`: `cobolt_forms::dropzone::commit_staged`, the
/// code the interpreter runs.
struct DropZones;

impl cobolt_abi_host::host::DropZonePort for DropZones {
    fn commit(&mut self, staged: &str, ticked: Option<&str>, destination: &str) -> cobolt_forms::dropzone::StagedCommit {
        cobolt_forms::dropzone::commit_staged(staged, ticked, destination)
    }
}

/// The instance a call acts on, when the call names it.
fn call_inst(call: &ModuleCall) -> Option<u32> {
    match call {
        ModuleCall::CloseForm { inst } | ModuleCall::Event { inst, .. } => Some(*inst),
        _ => None,
    }
}

impl Thread {
    fn run(self, ready: mpsc::Sender<Result<(), Msg>>) {
        let Thread { engine, module, abi, imports, manifest, forms, files, sql, lang, pins, rx, tx, exit } = self;
        let layout = manifest.layout();
        // The display lists of a module with a Canvas2D cross in the region after frame-in (A23).
        let gfx = (manifest.limits.gfx_size > 0).then(|| (layout.frame_in_off + layout.frame_size, manifest.limits.gfx_size));
        let mut host = Host::new(manifest, HostOptions { desktop: true, ..HostOptions::default() });
        // The sounds a form plays (T49, A36).
        host.audio = Some(Box::new(crate::audio::RunnerAudio::default()));
        // The AWS controls and keys (T48): an asynchronous operation's result comes back on this
        // thread's own channel.
        let aws = {
            let back = tx.clone();
            crate::aws::Aws::new(move |r| {
                let _ = back.send(ToProgram::Aws(r));
            })
        };
        host.aws = Some(Box::new(aws.clone()));
        // The SqlDatabase controls (T41c), over the connections the SQL built-ins use: an asynchronous
        // statement's result comes back on this thread's own channel too.
        let db: std::rc::Rc<std::cell::RefCell<crate::sql::Builtins>> = std::rc::Rc::default();
        host.sql = Some(Box::new(crate::sql::Port {
            db: db.clone(),
            send: {
                let back = tx.clone();
                std::sync::Arc::new(move |n| {
                    let _ = back.send(ToProgram::Sql(n));
                })
            },
        }));
        // A FileDropZone's `CommitFiles()` (T39c): the interpreter's own copy, anchored at the
        // application's folder as the interpreter anchors it.
        host.drop_zones = Some(Box::new(DropZones));
        // The canvases and the graphics objects (T50).
        let graphics = crate::gfx::install(&mut host);
        // A form the Build has no program for is designed but not in the manifest: it cannot open.
        for f in forms {
            if host.manifest.form_by_name(&f.name).is_some() {
                if let Err(e) = host.add_form(f) {
                    let _ = ready.send(Err(Msg::Damaged(e)));
                    return;
                }
            }
        }
        let started = (|| -> Result<Driver, Msg> {
            let linker = crate::linker_for(&engine, abi, &imports)?;
            let mut st = State::new(lang, pins);
            st.forms = Some(FormRun { host, layout, gfx, graphics });
            st.files = crate::files::Files::new(files);
            st.sql = sql.map(crate::sql::SqlRun::new);
            st.db = db;
            let mut store = Store::new(&engine, st);
            let instance = linker.instantiate(&mut store, &module).map_err(|e| Msg::CannotStart(format!("{e:#}")))?;
            let fail = |e: wasmtime::Error| Msg::CannotStart(format!("{e:#}"));
            let arena = instance.get_memory(&mut store, "arena").ok_or_else(|| Msg::CannotStart("the module exports no `arena` memory".into()))?;
            let base = instance.get_typed_func::<(), i32>(&mut store, "pr_arena_base").map_err(fail)?.call(&mut store, ()).map_err(fail)?;
            store.data_mut().arena = Some(arena);
            store.data_mut().arena_base = base as u32;
            let found = instance.get_typed_func::<(), i32>(&mut store, "pr_abi").map_err(fail)?.call(&mut store, ()).map_err(fail)?;
            if found as u32 != abi {
                return Err(Msg::CannotStart(format!("the module answers ABI {found} and its manifest names ABI {abi}")));
            }
            let init = instance.get_typed_func::<(), i32>(&mut store, "pr_init").map_err(fail)?.call(&mut store, ()).map_err(fail)?;
            if init != 0 {
                return Err(Msg::CannotStart(format!("the module did not start (pr_init answered {init})")));
            }
            Ok(Driver { store, instance, arena, base: base as u32, layout })
        })();
        let mut d = match started {
            Ok(d) => d,
            Err(msg) => {
                let _ = ready.send(Err(msg));
                return;
            }
        };
        let _ = ready.send(Ok(()));

        let mut links: Vec<Link> = Vec::new();
        // Links whose instance the module has not opened yet, in the order they were asked for.
        let mut opening: VecDeque<usize> = VecDeque::new();
        // Forms the programs opened, waiting for the window the form host was asked for: `(form, the
        // handle the program holds)`. The form opens in the module when its window comes.
        let mut adopting: VecDeque<(usize, String)> = VecDeque::new();
        // Changes to an instance made before its window was joined to it.
        let mut parked: Vec<(u32, Change)> = Vec::new();
        // `Sleep`'s timed waits: when each is due, by the host's clock (virtual in a deterministic run).
        let mut timers: Vec<(Instant, u32)> = Vec::new();
        let mut stopped = false;
        let mut root_opened = false;
        // Canvases whose `onFrame` handler ran, to present when it has returned (AutoPresent, T50).
        let mut presents: Vec<(u32, usize)> = Vec::new();
        loop {
            let msg = match timers.iter().map(|(at, _)| *at).min() {
                Some(due) => {
                    let left = due.saturating_duration_since(cobolt_form_host::clock::now());
                    // The clock may be virtual: look again soon rather than sleep through its moves.
                    match rx.recv_timeout(left.min(Duration::from_millis(5))) {
                        Ok(m) => Some(m),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                None => match rx.recv() {
                    Ok(m) => Some(m),
                    Err(_) => return,
                },
            };
            match msg {
                Some(ToProgram::Open { form, run }) => {
                    let ProgramRun { handle, form_object, events, state, display, input, pending, finished, requests, super_handle, designed_menus, .. } = *run;
                    let Some(entry) = d.host().manifest.form_by_name(&form).map(|e| e.id as usize) else {
                        let _ = display.send(format!("Runtime error: the form {form} has no program in this application"));
                        finished.store(true, Ordering::Relaxed);
                        continue;
                    };
                    // The SideMenus' designed rows, which the program may not edit (spec 066): the
                    // same the form host hands the interpreter.
                    for (ctl, def) in designed_menus {
                        d.host().menus.insert(&form, &ctl, def);
                    }
                    let id = links.len();
                    let opener = super_handle.as_deref().and_then(|h| links.iter().find(|l| l.handle == h)).and_then(|l| l.inst);
                    links.push(Link { form: entry, object: form_object, inst: None, handle, requests, gone: false, queue: VecDeque::new(), state, display, input, pending, finished });
                    if stopped {
                        links[id].finish();
                        continue;
                    }
                    let forward = tx.clone();
                    std::thread::Builder::new()
                        .name("form-events".into())
                        .spawn(move || {
                            for ev in events {
                                if forward.send(ToProgram::Event { link: id, ev }).is_err() {
                                    return;
                                }
                            }
                            let _ = forward.send(ToProgram::Gone { link: id });
                        })
                        .expect("cannot start the form's event thread");
                    // The window of a form a program opened: the form opens in the module now, in it.
                    if let Some(at) = adopting.iter().position(|(f, _)| *f == entry) {
                        let (_, handle) = adopting.remove(at).expect("a position in the queue");
                        if d.host().open_held(&handle) {
                            opening.push_back(id);
                        } else {
                            // Nothing waits for it any more: the window goes too.
                            let l = &mut links[id];
                            l.gone = true;
                            let _ = l.requests.send(FormRequest::CloseSelf { caller: l.handle.clone() });
                            l.finish();
                        }
                    } else {
                        opening.push_back(id);
                        // The first instance of the main form is the run's root, as the page opens it.
                        let main = d.host().manifest.main_form().id as usize;
                        if entry == main && !root_opened {
                            root_opened = true;
                            d.host().start();
                        } else {
                            // Loaded into a pane, the form's opener is the shell's form (`super::`).
                            let mode = if opener.is_some() { OpenMode::Embedded } else { OpenMode::Window };
                            d.host().open_form(entry, opener, mode);
                        }
                    }
                }
                Some(ToProgram::Event { link, ev }) => {
                    let l = &mut links[link];
                    if ev.ctrl_id == FormEvent::quit().ctrl_id {
                        // The window was closed: this form's program is over, as the module's own
                        // `__QUIT__` ends it (and a caller waiting for it goes on).
                        l.taken();
                        l.gone = true;
                        l.drop_queue();
                        match l.inst {
                            Some(inst) => d.host().close_instance(inst),
                            None => l.finish(),
                        }
                    } else {
                        l.queue.push_back(ev);
                    }
                }
                Some(ToProgram::Answered { id, path }) => d.host().file_dialog_answer(id, path.as_deref()),
                Some(ToProgram::Aws(r)) => {
                    let (inst, ctl) = (r.inst, r.ctl.clone());
                    let verbose = d.host().control_property(inst, &ctl, "Verbose").is_some_and(|v| v.trim().eq_ignore_ascii_case("true"));
                    if let Some(fx) = aws.delivered(r, verbose) {
                        d.host().aws_delivered(inst, &ctl, fx);
                    }
                }
                Some(ToProgram::Sql(n)) => {
                    let news = d.store.data_mut().db.borrow_mut().delivered(n);
                    if let Some((inst, ctl, fx)) = news {
                        d.host().sql_delivered(inst, &ctl, fx);
                    }
                }
                Some(ToProgram::Gone { link }) => {
                    links[link].gone = true;
                    if let Some(inst) = links[link].inst {
                        d.host().close_instance(inst);
                    }
                }
                None => {}
            }
            // `Sleep`s that are due.
            let now = cobolt_form_host::clock::now();
            let due: Vec<u32> = timers.iter().filter(|(at, _)| *at <= now).map(|(_, id)| *id).collect();
            timers.retain(|(at, _)| *at > now);
            for id in due {
                d.host().timer_fired(id);
            }
            drive(&mut d, &mut links, &mut opening, &mut adopting, &mut parked, &mut timers, &mut stopped, &tx);
            // The events each free instance has waiting, in order; one may make it busy again.
            while let Some(at) = (0..links.len()).find(|&i| !links[i].queue.is_empty() && links[i].inst.is_some_and(|inst| !d.host().busy(inst))) {
                let ev = links[at].queue.pop_front().expect("a waiting event");
                presents.extend(take(&mut d, &links[at], ev));
                drive(&mut d, &mut links, &mut opening, &mut adopting, &mut parked, &mut timers, &mut stopped, &tx);
                auto_present(&mut d, &links, &mut parked, &mut presents);
            }
            if let Some(code) = d.store.data().exit_code {
                exit.store(code, Ordering::SeqCst);
            }
            if stopped && links.iter().all(|l| l.finished.load(Ordering::Relaxed)) {
                return;
            }
        }
    }
}

/// The program takes one event its window raised: it leaves `pending` before its handler runs, as
/// the interpreter takes it; what the user changed reaches the program first and is not echoed; an
/// input wake only does that. Answers the canvas to present when the handler has returned: a
/// Canvas2D's `onFrame` with `AutoPresent` on (T50).
fn take(d: &mut Driver, l: &Link, ev: FormEvent) -> Option<(u32, usize)> {
    l.taken();
    let inst = l.inst?;
    let edits: Vec<StateUpdate> = l.input.try_iter().collect();
    for u in edits {
        if let Some(ctl) = control_index(d.host(), inst, &l.object, &u.ctrl_id) {
            d.host().ui_set(inst, ctl, &u.prop, &u.value);
        } else {
            // A control the form does not hold (a FileDropZone's review list that is not there): kept,
            // as the interpreter keeps it (T39c).
            d.host().ui_set_absent(inst, &u.ctrl_id, &u.prop, &u.value);
        }
    }
    let _ = d.host().take_changes();
    if ev.event_id == FormSupervisor::INPUT_WAKE_EVENT {
        return None;
    }
    let ctl = control_index(d.host(), inst, &l.object, &ev.ctrl_id)?;
    let args = if ev.value.is_empty() { Vec::new() } else { vec![ev.value.clone()] };
    let handled = d.host().dom_event(inst, ctl, &ev.event_id, args);
    (handled && crate::gfx::auto_presents(d.host(), inst, ctl, &ev.event_id)).then_some((inst, ctl))
}

/// A canvas whose `onFrame` handler has returned shows what the handler drew (`AutoPresent`, the
/// interpreter's rule): a `Present` after the drawing the handler sent. A handler still running (it
/// waits for something) presents when it returns.
fn auto_present(d: &mut Driver, links: &[Link], parked: &mut Vec<(u32, Change)>, presents: &mut Vec<(u32, usize)>) {
    if presents.is_empty() {
        return;
    }
    let mut any = false;
    presents.retain(|&(inst, ctl)| {
        if d.host().busy(inst) {
            return true;
        }
        if let Some(tree) = d.host().trees.get_mut(&inst) {
            tree.command(ctl, "canvas-call", vec!["Present".into()]);
            any = true;
        }
        false
    });
    if any {
        let g = d.graphics();
        changes(d.host(), links, parked, &g);
    }
}

/// The control of instance `inst` that `ctrl_id` names: the form itself (index 0) when it names
/// the form object, else the control of that name.
fn control_index(host: &Host, inst: u32, object: &str, ctrl_id: &str) -> Option<usize> {
    let tree = host.trees.get(&inst)?;
    if ctrl_id.trim().eq_ignore_ascii_case(object) || ctrl_id.trim().eq_ignore_ascii_case(&tree.form_name) {
        return Some(0);
    }
    tree.by_name(ctrl_id)
}

/// What the program thread keeps between messages, for [`drive`] and [`effects`].
type Opening<'a> = (&'a mut VecDeque<usize>, &'a mut VecDeque<(usize, String)>);

/// Run everything the ABI host has to do, then hand what it produced to the windows.
fn drive(
    d: &mut Driver,
    links: &mut [Link],
    opening: &mut VecDeque<usize>,
    adopting: &mut VecDeque<(usize, String)>,
    parked: &mut Vec<(u32, Change)>,
    timers: &mut Vec<(Instant, u32)>,
    stopped: &mut bool,
    tx: &mpsc::Sender<ToProgram>,
) {
    loop {
        loop {
            let Some(call) = d.host().next_call() else {
                if d.host().has_yielded() {
                    d.host().allow_yielded();
                    continue;
                }
                break;
            };
            let result = d.perform(&call);
            d.host().complete(&call, result);
            effects(d, links, (opening, adopting), timers, stopped, call_inst(&call));
        }
        effects(d, links, (opening, adopting), timers, stopped, None);
        let g = d.graphics();
        changes(d.host(), links, parked, &g);
        // A dialog with no window to show it in is answered at once: its program goes on.
        if !file_dialogs(d.host(), links, tx) {
            break;
        }
    }
}

/// Show the file dialogs the programs opened (T32) through the form host, asked for as the
/// interpreter asks for them; each answer comes back to the program thread as
/// [`ToProgram::Answered`]. A dialog whose form has no window is cancelled at once, as the
/// interpreter's is with no window to show it in: answers whether one was.
fn file_dialogs(host: &mut Host, links: &[Link], tx: &mpsc::Sender<ToProgram>) -> bool {
    let mut settled = false;
    for ask in host.take_file_dialogs() {
        let shown = match (file_dialog_kind(&ask.verb), link_of(links, ask.inst)) {
            (Some(kind), Some(l)) if !l.gone => {
                let parts = file_dialog_ask(kind, ask.args.len(), |i| Ok::<_, Infallible>(ask.args[i].trim().to_string())).unwrap_or_else(|never| match never {});
                let (reply, answer) = mpsc::channel();
                let asked = l.requests.send(FormRequest::FileDialog {
                    kind,
                    title: parts.title,
                    filters: parts.filters,
                    directory: parts.directory,
                    file_name: parts.file_name,
                    reply,
                });
                let back = tx.clone();
                let id = ask.id;
                asked.is_ok()
                    && std::thread::Builder::new()
                        .name("file-dialog".into())
                        .spawn(move || {
                            let path = answer.recv().ok().flatten();
                            let _ = back.send(ToProgram::Answered { id, path });
                        })
                        .is_ok()
            }
            _ => false,
        };
        if !shown {
            host.file_dialog_answer(ask.id, None);
            settled = true;
        }
    }
    settled
}

/// The link of instance `inst`.
fn link_of(links: &[Link], inst: u32) -> Option<&Link> {
    links.iter().find(|l| l.inst == Some(inst))
}

/// Carry out what the ABI host left for the shell. A `DISPLAY` line goes to the window of the
/// instance the call acted on (the first window when the call names none): the form host prints
/// every window's lines alike.
fn effects(d: &mut Driver, links: &mut [Link], (opening, adopting): Opening<'_>, timers: &mut Vec<(Instant, u32)>, stopped: &mut bool, inst: Option<u32>) {
    for e in d.host().take_effects() {
        let display = |links: &[Link], text: String| {
            let to = inst.and_then(|i| link_of(links, i)).or_else(|| links.first());
            if let Some(l) = to {
                let _ = l.display.send(text);
            }
        };
        match e {
            Effect::Opened { inst, form, .. } => {
                if let Some(at) = opening.iter().position(|&l| links[l].form == form) {
                    let l = opening.remove(at).expect("a position in the queue");
                    links[l].inst = Some(inst);
                }
            }
            Effect::Display(text) => display(links, text),
            Effect::RuntimeError(e) => {
                let text = format!("Runtime error: {}", e.describe());
                eprintln!("{text}");
                display(links, text);
            }
            Effect::Closed { inst } => {
                if let Some(l) = links.iter_mut().find(|l| l.inst == Some(inst)) {
                    // The program closed it: the supervisor closes its window, as for the interpreter.
                    if !l.gone {
                        l.gone = true;
                        let _ = l.requests.send(FormRequest::CloseSelf { caller: l.handle.clone() });
                    }
                    l.drop_queue();
                    l.finish();
                }
            }
            Effect::Stopped => {
                *stopped = true;
                // `STOP RUN` ends the run unit normally (T41).
                if let Some(sql) = d.store.data_mut().sql.as_mut() {
                    sql.end(true);
                }
                d.store.data_mut().db.borrow_mut().end();
                for l in links.iter() {
                    l.finish();
                }
            }
            Effect::Timer { id, ms } => timers.push((cobolt_form_host::clock::now() + Duration::from_millis(ms), id)),
            other => {
                if std::env::var_os("PRC_RUNNER_TRACE").is_some() {
                    eprintln!("prc-runner: not carried out yet: {other:?}");
                }
            }
        }
    }
    // The windows the programs asked for: the form host's supervisor makes each, as it does for the
    // interpreter, and the form opens in the module when its window comes.
    for ask in d.host().take_window_asks() {
        let name = d.host().manifest.forms[ask.form].name.clone();
        let p = ask.placement;
        let asked = link_of(links, ask.opener).is_some_and(|caller| {
            let (reply, _) = mpsc::channel();
            caller
                .requests
                .send(FormRequest::OpenForm {
                    caller: caller.handle.clone(),
                    form_id: name,
                    sync: p.sync,
                    window_state: p.window_state,
                    x: p.x,
                    y: p.y,
                    width: p.width,
                    height: p.height,
                    modal: p.modal,
                    reply,
                })
                .is_ok()
        });
        if asked {
            adopting.push_back((ask.form, ask.handle));
        } else {
            // Its opener's window is gone: there is nowhere to show it.
            d.host().drop_held(&ask.handle);
        }
    }
}

/// The module's property changes, as the windows' state updates: the property spelled as the
/// program spelled its write (`CAPTION` from COBOL, `Items` from `AddItem`), the value as the
/// program stored it. The interpreter's writes reach the window spelled the same way. A change to
/// an instance whose window is not joined to it yet waits in `parked`. The drawing of the canvases
/// runs first, wherever its window is (T50): its pictures wait for the window instead.
fn changes(host: &mut Host, links: &[Link], parked: &mut Vec<(u32, Change)>, graphics: &crate::gfx::Shared) {
    use cobolt_forms::model::{CHECKED_PROP, SELECTED_PROP};
    // The program's writes to a control the form does not hold (T39c), first, as they were made first.
    for (inst, ctrl_id, prop, value) in std::mem::take(&mut host.absent_out) {
        if let Some(l) = link_of(links, inst) {
            let _ = l.state.send(StateUpdate::new(ctrl_id, prop, value));
        }
    }
    let mut list = std::mem::take(parked);
    list.extend(host.take_changes());
    let pictures = graphics.borrow_mut().pump(host, &mut list);
    // The `LastError`s the drawing and the graphics calls wrote.
    list.extend(host.take_changes());
    for (inst, update) in pictures {
        match link_of(links, inst) {
            Some(l) => {
                let _ = l.state.send(update);
            }
            None => graphics.borrow_mut().wait(inst, update),
        }
    }
    for (inst, change) in list {
        let Some(tree) = host.trees.get(&inst) else { continue };
        let Some(l) = link_of(links, inst) else {
            parked.push((inst, change));
            continue;
        };
        match change {
            Change::Prop { ctl, key } => {
                let Some(c) = tree.ctls.get(ctl) else { continue };
                // A toggle's state is stored under both its names; the interpreter writes the one
                // its control has (a RadioButton's `Selected`, a CheckBox's or a Switch's `Checked`).
                let canonical = match c.control_type() {
                    Some(cobolt_forms::ControlType::RadioButton) => Some(SELECTED_PROP),
                    Some(cobolt_forms::ControlType::CheckBox | cobolt_forms::ControlType::Switch) => Some(CHECKED_PROP),
                    _ => None,
                };
                let toggle = [SELECTED_PROP, CHECKED_PROP].iter().any(|k| k.eq_ignore_ascii_case(&key));
                if canonical.is_some_and(|keep| toggle && !keep.eq_ignore_ascii_case(&key)) {
                    continue;
                }
                let ctrl_id = if ctl == 0 { l.object.clone() } else { c.name.clone() };
                let prop = tree.spelled.get(&(ctl, key.clone())).cloned().unwrap_or_else(|| key.to_ascii_uppercase());
                let value = c.props.get(&key).cloned().unwrap_or_default();
                let _ = l.state.send(StateUpdate::new(ctrl_id, prop, value));
            }
            Change::Command { ctl, name, args } if name == cobolt_abi_host::tree::ORDERED_WRITES => {
                // A control's writes up to a command that acts on it as it is then (a Snackbar's `Show()`):
                // one at a time, in order, as the interpreter's reach the window.
                let Some(c) = tree.ctls.get(ctl) else { continue };
                let ctrl_id = if ctl == 0 { l.object.clone() } else { c.name.clone() };
                for kv in args.chunks_exact(2) {
                    let _ = l.state.send(StateUpdate::new(ctrl_id.clone(), kv[0].clone(), kv[1].clone()));
                }
            }
            Change::Command { ctl: 0, name, args } if WINDOW_METHODS.contains(&name.as_str()) => {
                // A window method of the form's own window: the supervisor's, as the interpreter's is.
                let (reply, _) = mpsc::channel();
                let _ = l.requests.send(FormRequest::HandleMethod { handle: l.handle.clone(), method: name.to_ascii_uppercase(), args, reply });
            }
            Change::Command { ctl, name, args } => {
                if std::env::var_os("PRC_RUNNER_TRACE").is_some() {
                    let who = tree.ctls.get(ctl).map(|c| c.name.as_str()).unwrap_or("?");
                    eprintln!("prc-runner: command not carried out yet: {who}.{name}{args:?}");
                }
            }
        }
    }
    // A layer is an object of its own name to the window (spec 091 R33).
    let insts: Vec<u32> = host.trees.keys().copied().collect();
    for inst in insts {
        let Some(l) = link_of(links, inst) else { continue };
        let Some(tree) = host.trees.get_mut(&inst) else { continue };
        for (layer, prop) in std::mem::take(&mut tree.layer_writes) {
            let Ok(value) = tree.layer_get(layer, &prop) else { continue };
            let _ = l.state.send(StateUpdate::new(tree.layers[layer].name.clone(), prop.to_ascii_uppercase(), value));
        }
    }
    // A form's own property another form wrote (`super::Title`, `super::"SetProperty"`): the
    // supervisor's to apply and publish, as the interpreter's `super::` writes are (spec 049 R30).
    for (inst, key, value) in host.take_form_writes() {
        if let Some(l) = link_of(links, inst) {
            let (reply, _) = mpsc::channel();
            let _ = l.requests.send(FormRequest::HandleMethod { handle: l.handle.clone(), method: "SETPROPERTY".into(), args: vec![key, value], reply });
        }
    }
    // The main form opened again while it runs: the supervisor focuses its one window (spec 037 R10).
    for (caller, form, placement) in host.take_refocused() {
        if let Some(l) = link_of(links, caller) {
            let (reply, _) = mpsc::channel();
            let _ = l.requests.send(FormRequest::OpenForm {
                caller: l.handle.clone(),
                form_id: host.manifest.forms[form].name.clone(),
                sync: placement.sync,
                window_state: placement.window_state,
                x: placement.x,
                y: placement.y,
                width: placement.width,
                height: placement.height,
                modal: placement.modal,
                reply,
            });
        }
    }
}

/// The window methods the ABI host keeps as commands of the form (`me::`), sent to the supervisor
/// as the interpreter sends them.
const WINDOW_METHODS: &[&str] = &["setwindowstate", "focus", "setfocus", "settitlevisible", "setbreadcrumbdetail", "clearbreadcrumbdetail"];
