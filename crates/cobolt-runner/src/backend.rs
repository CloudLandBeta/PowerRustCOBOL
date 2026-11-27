// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Calling the exported programs of a Backend module with no window (spec 114 T21).
//!
//! A Backend is built as one module that answers `pr_backend_call(export)` (ABI A59). This is the side that calls it: the
//! module is checked against the capabilities a runner provides, loaded on the Backend engine (epoch interruption on,
//! spec 114 D3), its imports linked **once** into an instance template, and each call gets a store, an instance and a
//! provider [`State`] of its own: nothing is shared between two calls, and what a call opened (a file, a database
//! connection) ends with it. The server integration, the time and the cancel are the Backend runner's
//! (`cobolt-backend-runner`); this crate keeps what only it can reach, the imports every Desktop program shares.
//!
//! The data capabilities are the Desktop runner's own functions, so a Backend and a Desktop program that name the same file
//! or statement meet the same code.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cobolt_runner_engine::{Cache, MachineCode};
use cobolt_wasm_abi::backend::{self, result, Answer, BackendSection, Refusal};
use cobolt_wasm_abi::manifest::MANIFEST_SECTION;
use wasmtime::{Engine, InstancePre, Store, Trap, UpdateDeadline};

use crate::verbs::{CallHost, Failure, Verbs};
use crate::{check_with, files, linker_for, sql, Lang, Msg, Pins, State};

/// What a runner of a Backend module provides: what a program with no window can use ([`crate::headless_capabilities`]),
/// and the `backend` capability (A60), the verbs of the call a server is serving. A plain console runner has no call to
/// serve, so it does not provide it.
pub fn backend_capabilities() -> Vec<&'static str> {
    let mut list = crate::headless_capabilities();
    list.push(cobolt_wasm_abi::codes::cap::BACKEND);
    list
}

/// How one call ended, as far as the module can say.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// The program ran and its parameters and `RETURNING` are answered (A59 result 0).
    Answered(Answer),
    /// The request was refused before the program ran: the mapping's code and the member's path (result 1).
    Refused(Refusal),
    /// The program ran, and a record it holds cannot be written as JSON (result 4).
    Unwritable(Refusal),
    /// The answer did not fit the response area (result 2).
    TooLarge,
    /// The request frame was malformed or named an export the module does not have (result 3).
    BadRequest,
    /// The request does not fit the module's request area: nothing was run.
    RequestTooLarge,
    /// The call was stopped from outside: its time ran out or it was cancelled (the epoch deadline's decision).
    Interrupted,
    /// The program stopped on a run-time error or a trap of the module: what the module said, for the operator's log.
    Trapped(String),
}

/// A finished call.
#[derive(Debug, PartialEq)]
pub struct Called {
    pub outcome: Outcome,
    /// What the program `DISPLAY`ed, a line each (the debug log only; never sent to the caller).
    pub display: Vec<String>,
    /// What `WS-FAIL` reported, when the program ran to its end after it (A60): the server answers it as an application
    /// failure, whatever else the module says.
    pub failure: Option<Failure>,
    /// A `WS-PROGRESS` or `WS-EMIT` ended the call because its caller asked to cancel (A60).
    pub cancelled: bool,
}

/// What the runner asks of a call besides its request.
#[derive(Clone)]
pub struct CallControl {
    /// Asked at every tick of the engine's epoch: `true` ends the call (`Outcome::Interrupted`). A call that nothing
    /// stops runs to its end.
    pub stop: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl CallControl {
    /// No call is stopped.
    pub fn none() -> CallControl {
        CallControl { stop: Arc::new(|| false) }
    }

    /// A call stopped when `flag` is set.
    pub fn on(flag: Arc<AtomicBool>) -> CallControl {
        CallControl { stop: Arc::new(move || flag.load(Ordering::SeqCst)) }
    }
}

/// A Backend module, checked, loaded and ready to be called from any number of threads.
pub struct BackendModule {
    engine: Engine,
    pre: InstancePre<State>,
    section: BackendSection,
    capabilities: Vec<String>,
    files: files::FileSetup,
    sql: Option<sql::SqlSetup>,
    lang: Lang,
    pins: Pins,
    /// How the module was reached, for the operator's log: `MachineCode`, `Cache` or `Compiled`.
    pub load_path: String,
}

impl BackendModule {
    /// Check `module` against the capabilities `provided`, load it on the Backend engine (with the Build's `code` when
    /// there is some, else the `cache`'s, else compile now) and link its imports. A capability or an import this runner does not
    /// provide is refused, named, before anything runs.
    pub fn load(module: &[u8], code: Option<&MachineCode>, provided: &[&str], cache: Option<PathBuf>) -> Result<BackendModule, Msg> {
        let checked = check_with(module, provided, false)?;
        let scan = cobolt_wasm_abi::scan::scan_module(module).map_err(Msg::Damaged)?;
        let text = scan
            .custom(MANIFEST_SECTION)
            .and_then(|t| std::str::from_utf8(t).ok())
            .ok_or_else(|| Msg::Damaged("the module's manifest cannot be read".into()))?;
        let section = BackendSection::from_manifest(text)
            .map_err(Msg::Damaged)?
            .ok_or_else(|| Msg::Damaged("the module is not a Backend module: it has no `backend` section".into()))?;
        let engine = cobolt_runner_engine::backend_engine().map_err(Msg::CannotStart)?;
        let cache = cache.map(|d| Cache::new(d, cobolt_runner_engine::CACHE_LIMIT));
        let loaded = cobolt_runner_engine::load(&engine, module, code, cache.as_ref()).map_err(Msg::CannotStart)?;
        let linker = linker_for(&engine, checked.abi, &checked.imports)?;
        let pre = linker.instantiate_pre(&loaded.module).map_err(|e| Msg::CannotStart(format!("{e:#}")))?;
        let (pins, said) = Pins::from_env();
        for s in said {
            eprintln!("{s}");
        }
        Ok(BackendModule {
            engine,
            pre,
            section,
            capabilities: checked.manifest.capabilities.clone(),
            files: checked.files,
            sql: checked.sql,
            lang: Lang::En,
            pins,
            load_path: format!("{:?}", loaded.path),
        })
    }

    /// The engine the calls run on, which a ticker advances (`Engine::increment_epoch`) to stop a call.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Advance the engine's epoch by one: what a runner's ticker does every few milliseconds. The next check of every
    /// call in flight asks its [`CallControl`] whether to go on.
    pub fn tick(&self) {
        self.engine.increment_epoch();
    }

    /// A handle that advances the epoch from another thread (the ticker's).
    pub fn ticker(&self) -> impl Fn() + Send + 'static {
        let engine = self.engine.clone();
        move || engine.increment_epoch()
    }

    /// The module's `backend` section: its exports, and the areas of the frames.
    pub fn section(&self) -> &BackendSection {
        &self.section
    }

    /// The capabilities the module declared.
    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }

    /// Call the export `name` with one JSON text per parameter on a fresh instance. `Err` is the runner's own failure
    /// (the module could not be started), not the program's.
    pub fn call(&self, name: &str, args: &[&str], control: &CallControl) -> Result<Called, String> {
        let index = self.section.export(&name.to_ascii_uppercase()).ok_or_else(|| format!("the module exports no program {name}"))?.index;
        self.call_index(index as i32, args, control)
    }

    /// [`BackendModule::call`] for a call whose server hands the program's verbs a `host` (A60).
    pub fn call_with(&self, name: &str, args: &[&str], control: &CallControl, host: Option<Box<dyn CallHost>>) -> Result<Called, String> {
        let index = self.section.export(&name.to_ascii_uppercase()).ok_or_else(|| format!("the module exports no program {name}"))?.index;
        self.call_index_with(index as i32, args, control, host)
    }

    /// [`BackendModule::call`] by the export's index in the module's table.
    pub fn call_index(&self, index: i32, args: &[&str], control: &CallControl) -> Result<Called, String> {
        self.call_index_with(index, args, control, None)
    }

    /// [`BackendModule::call_index`] for a call whose server hands the program's verbs a `host` (`WS-PROGRESS`, `WS-EMIT`,
    /// `WS-CLAIM`, the file verbs, `WS-CANCELLED`; A60). With none, a verb behaves as the interpreter does with no hooks.
    pub fn call_index_with(&self, index: i32, args: &[&str], control: &CallControl, host: Option<Box<dyn CallHost>>) -> Result<Called, String> {
        let frame = backend::encode_request(args);
        if frame.len() as u32 > self.section.request.cap {
            return Ok(Called { outcome: Outcome::RequestTooLarge, display: Vec::new(), failure: None, cancelled: false });
        }
        let mut state = State::new(self.lang, self.pins);
        state.files = std::rc::Rc::new(std::cell::RefCell::new(files::Files::new(self.files.clone())));
        state.sql = self.sql.clone().map(sql::SqlRun::new);
        state.display = Some(Vec::new());
        state.verbs = Verbs::new(host, Some(control.stop.clone()));
        let mut store = Store::new(&self.engine, state);
        // A call is interrupted at the next tick of the epoch when its stop says so, and goes on otherwise.
        store.set_epoch_deadline(1);
        let stop = control.stop.clone();
        store.epoch_deadline_callback(move |_| if stop() { Err(wasmtime::Error::from(Trap::Interrupt)) } else { Ok(UpdateDeadline::Continue(1)) });
        let instance = self.pre.instantiate(&mut store).map_err(|e| format!("cannot instantiate the module: {e:#}"))?;
        let arena = instance.get_memory(&mut store, "arena").ok_or("the module exports no `arena` memory")?;
        let base =
            instance.get_typed_func::<(), i32>(&mut store, "pr_arena_base").map_err(|e| e.to_string())?.call(&mut store, ()).map_err(|e| e.to_string())?;
        store.data_mut().arena = Some(arena);
        store.data_mut().arena_base = base as u32;
        let memory = instance.get_memory(&mut store, "memory").ok_or("the module exports no `memory`")?;
        memory.write(&mut store, self.section.request.at as usize, &frame).map_err(|e| e.to_string())?;
        let call = instance.get_typed_func::<i32, i32>(&mut store, "pr_backend_call").map_err(|e| e.to_string())?;
        let status = call.call(&mut store, index);

        let outcome = match status {
            Ok(status) => {
                let data = memory.data(&store);
                let at = self.section.response.at as usize;
                match data.get(at..at + self.section.response.cap as usize) {
                    None => return Err("the response area is outside the module's memory".into()),
                    Some(area) => match status {
                        result::ANSWERED => Outcome::Answered(backend::decode_answer(area)?),
                        result::REFUSED => Outcome::Refused(backend::decode_refusal(area)?),
                        result::UNWRITABLE => Outcome::Unwritable(backend::decode_refusal(area)?),
                        result::TOO_LARGE => Outcome::TooLarge,
                        result::BAD_REQUEST => Outcome::BadRequest,
                        other => return Err(format!("pr_backend_call answered {other}, which ABI A59 does not define")),
                    },
                }
            }
            Err(e) if e.downcast_ref::<Trap>() == Some(&Trap::Interrupt) => Outcome::Interrupted,
            Err(e) => {
                let said = match store.data_mut().fatal.take() {
                    Some(m) => crate::messages::text(Lang::En, &m),
                    None => format!("{e:#}"),
                };
                Outcome::Trapped(said)
            }
        };
        // The end of the run unit's work, as the interpreter ends it: committed after an answer, rolled back after anything
        // else (a refused request ran nothing, so there is nothing to undo).
        let normal = matches!(outcome, Outcome::Answered(_));
        let st = store.data_mut();
        if let Some(sql) = st.sql.as_mut() {
            sql.end(normal);
        }
        st.db.borrow_mut().end();
        st.http.borrow_mut().end();
        let display = st.display.take().unwrap_or_default();
        let (failure, cancelled) = (st.verbs.failure.take(), st.verbs.cancelled);
        Ok(Called { outcome, display, failure, cancelled })
    }
}

/// Whether a type can be shared between threads: asserted at compile time for [`BackendModule`].
#[allow(dead_code)]
fn assert_shareable() {
    fn is<T: Send + Sync>() {}
    is::<BackendModule>();
}
