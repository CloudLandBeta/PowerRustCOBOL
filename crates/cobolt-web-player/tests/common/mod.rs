// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The native rig: a fixture module run on `wasmtime` with the real [`Host`] behind its imports
//! (operator, WASM-010 Q-new-2: the player's native tests run on the engine the generator's do).
//!
//! It plays the part of the loader — instantiate, provide the six imports, take a fresh view of
//! the arena at each use, perform the calls the host asks for — so every rule of the ABI host
//! can be exercised without a browser, on both module variants.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::rc::Rc;

use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::fixture::{build, standard_cfrm, standard_spec, Fixture};
use cobolt_wasm_abi::manifest::ArenaLayout;
use cobolt_web_player::host::{CallResult, Host, HostOptions, ModuleCall, Outbound};
use wasmtime::{Caller, Config, Engine, Instance, Linker, Memory, Module, Store};

pub type Shared = Rc<RefCell<Host>>;

pub struct Rig {
    pub host: Shared,
    pub fixture: Fixture,
    store: Store<Shared>,
    instance: Instance,
    arena: Memory,
    base: Arc<AtomicU32>,
    layout: ArenaLayout,
    pub multi: bool,
    /// Every call the host asked for, in order.
    pub calls: Vec<ModuleCall>,
}

fn read(caller: &mut Caller<'_, Shared>, base: u32, off: u32, len: usize) -> Vec<u8> {
    let mem = caller.get_export("arena").and_then(|e| e.into_memory()).expect("the module exports `arena`");
    let mut buf = vec![0u8; len];
    mem.read(&*caller, (base + off) as usize, &mut buf).expect("in bounds");
    buf
}

fn write(caller: &mut Caller<'_, Shared>, base: u32, off: u32, bytes: &[u8]) {
    let mem = caller.get_export("arena").and_then(|e| e.into_memory()).expect("the module exports `arena`");
    mem.write(&mut *caller, (base + off) as usize, bytes).expect("in bounds");
}

impl Rig {
    pub fn new(multi: bool) -> Rig {
        Rig::with_spec(build(&standard_spec(multi)), multi, HostOptions::default())
    }

    pub fn with_spec(fixture: Fixture, multi: bool, opts: HostOptions) -> Rig {
        let cfrms: Vec<String> = fixture.manifest.forms.iter().map(|f| standard_cfrm(&fixture, &f.name)).collect();
        Rig::with_forms(fixture, &cfrms, multi, opts)
    }

    /// A module and the `.cfrm` of each of its forms: a module the generator built
    /// (`tests/fixtures/generated/`), or the hand-assembled fixture.
    pub fn with_forms(fixture: Fixture, cfrms: &[String], multi: bool, opts: HostOptions) -> Rig {
        let mut host = Host::new(fixture.manifest.clone(), opts);
        for cfrm in cfrms {
            let form = load_form_from_str(cfrm).expect("the form description loads");
            host.add_form(form).expect("paired");
        }
        let layout = host.layout();
        let host: Shared = Rc::new(RefCell::new(host));
        let mut cfg = Config::default();
        cfg.wasm_multi_memory(true);
        cfg.cranelift_nan_canonicalization(true);
        let engine = Engine::new(&cfg).expect("an engine");
        let module = Module::new(&engine, &fixture.wasm[..]).expect("the fixture validates");
        let mut store = Store::new(&engine, host.clone());
        let mut linker = <Linker<Shared>>::new(&engine);
        let base = Arc::new(AtomicU32::new(0));

        let b = base.clone();
        linker
            .func_wrap("pr", "flush", move |mut caller: Caller<'_, Shared>, len: i32| {
                let bytes = read(&mut caller, b.load(Ordering::Relaxed), layout.batch_off, len as usize);
                caller.data().clone().borrow_mut().flush(&bytes);
            })
            .unwrap();
        let b = base.clone();
        linker
            .func_wrap("pr", "query", move |mut caller: Caller<'_, Shared>, len: i32| -> i32 {
                let bytes = read(&mut caller, b.load(Ordering::Relaxed), layout.batch_off, len as usize);
                let host = caller.data().clone();
                let r = host.borrow_mut().query(&bytes);
                match r {
                    Ok(answer) => {
                        write(&mut caller, b.load(Ordering::Relaxed), layout.answer_off, &answer);
                        answer.len() as i32
                    }
                    Err(code) => code,
                }
            })
            .unwrap();
        let b = base.clone();
        linker
            .func_wrap("pr", "remote", move |mut caller: Caller<'_, Shared>, len: i32| -> i32 {
                let frame = read(&mut caller, b.load(Ordering::Relaxed), layout.frame_out_off, len as usize);
                let host = caller.data().clone();
                let r = host.borrow_mut().remote(&frame);
                match r {
                    Ok(id) => id as i32,
                    Err(code) => code,
                }
            })
            .unwrap();
        linker.func_wrap("pr", "now", |_: Caller<'_, Shared>| -> f64 { 1_700_000_000_000.0 }).unwrap();
        linker.func_wrap("pr", "random", |_: Caller<'_, Shared>| -> f64 { 0.5 }).unwrap();
        let b = base.clone();
        linker
            .func_wrap("pr", "abort", move |mut caller: Caller<'_, Shared>, len: i32| {
                let bytes = read(&mut caller, b.load(Ordering::Relaxed), layout.batch_off, len as usize);
                caller.data().clone().borrow_mut().abort(&bytes);
            })
            .unwrap();

        let instance = linker.instantiate(&mut store, &module).expect("instantiates");
        let arena = instance.get_memory(&mut store, "arena").expect("exports arena");
        let init = instance.get_typed_func::<(), i32>(&mut store, "pr_init").unwrap();
        assert_eq!(init.call(&mut store, ()).unwrap(), 0);
        let abi = instance.get_typed_func::<(), i32>(&mut store, "pr_abi").unwrap().call(&mut store, ()).unwrap();
        assert_eq!(abi, 1);
        let variant = instance.get_typed_func::<(), i32>(&mut store, "pr_variant").unwrap().call(&mut store, ()).unwrap();
        assert_eq!(variant & 1 == 1, multi, "pr_variant bit 0 says which variant this is");
        let ab = instance.get_typed_func::<(), i32>(&mut store, "pr_arena_base").unwrap().call(&mut store, ()).unwrap();
        base.store(ab as u32, Ordering::Relaxed);
        Rig { host, fixture, store, instance, arena, base, layout, multi, calls: Vec::new() }
    }

    fn func<P: wasmtime::WasmParams, R: wasmtime::WasmResults>(&mut self, name: &str) -> wasmtime::TypedFunc<P, R> {
        self.instance.get_typed_func::<P, R>(&mut self.store, name).unwrap_or_else(|e| panic!("export {name}: {e}"))
    }

    fn tick(&mut self) -> i32 {
        self.func::<(), i32>("pr_tick").call(&mut self.store, ()).unwrap()
    }

    /// Perform one call the host asked for.
    pub fn perform(&mut self, call: &ModuleCall) -> CallResult {
        self.calls.push(call.clone());
        let r = self.perform_inner(call);
        match r {
            Ok(v) => v,
            Err(e) => CallResult::Trapped(e.to_string()),
        }
    }

    fn perform_inner(&mut self, call: &ModuleCall) -> Result<CallResult, wasmtime::Error> {
        let base = self.base.load(Ordering::Relaxed);
        Ok(match call {
            ModuleCall::OpenForm { form, opener } => {
                let inst = self.func::<(i32, i32), i32>("pr_open_form").call(&mut self.store, (*form as i32, *opener))?;
                let status = self.func::<(), i32>("pr_status").call(&mut self.store, ())?;
                let task = if status == 1 || status == 2 { self.tick() } else { -1 };
                CallResult::Opened { inst, status, task }
            }
            ModuleCall::CloseForm { inst } => {
                let status = self.func::<i32, i32>("pr_close_form").call(&mut self.store, *inst as i32)?;
                let task = if status == 1 || status == 2 { self.tick() } else { -1 };
                CallResult::Status { status, task }
            }
            ModuleCall::Event { inst, ctrl, event, args, argc } => {
                self.arena.write(&mut self.store, (base + self.layout.answer_off) as usize, args).unwrap();
                let status = self
                    .func::<(i32, i32, i32, i32), i32>("pr_event")
                    .call(&mut self.store, (*inst as i32, *ctrl as i32, *event as i32, *argc as i32))?;
                let task = if status == 1 || status == 2 { self.tick() } else { -1 };
                CallResult::Status { status, task }
            }
            ModuleCall::Resume { task, frame } => {
                let len = match frame {
                    Some(f) => {
                        self.arena.write(&mut self.store, (base + self.layout.frame_in_off) as usize, f).unwrap();
                        f.len() as i32
                    }
                    None => 0,
                };
                let status = self.func::<(i32, i32), i32>("pr_resume").call(&mut self.store, (*task, len))?;
                let t = if status == 1 || status == 2 { self.tick() } else { -1 };
                CallResult::Status { status, task: t }
            }
            ModuleCall::Cancel { task } => {
                self.func::<i32, ()>("pr_cancel").call(&mut self.store, *task)?;
                // As the loader answers: what the module says after the cancel (a stopped start-up it took on).
                let status = self.func::<(), i32>("pr_status").call(&mut self.store, ())?;
                CallResult::Status { status, task: self.tick() }
            }
        })
    }

    /// The module's own count of live form instances (`pr_dbg_live`).
    pub fn module_live(&mut self) -> i32 {
        self.func::<(), i32>("pr_dbg_live").call(&mut self.store, ()).unwrap()
    }

    /// Run everything the host has to do until it has nothing left (yields run straight through).
    pub fn drive(&mut self) -> usize {
        let mut calls = 0;
        loop {
            let next = self.host.borrow_mut().next_call();
            let Some(call) = next else {
                if self.host.borrow().has_yielded() {
                    self.host.borrow_mut().allow_yielded();
                    continue;
                }
                return calls;
            };
            let result = self.perform(&call);
            self.host.borrow_mut().complete(&call, result);
            calls += 1;
            assert!(calls < 1_000_000, "the host never settled");
        }
    }

    /// Run until idle but let yields wait (a test that wants to count them calls `step_once`).
    pub fn step_once(&mut self) -> bool {
        let next = self.host.borrow_mut().next_call();
        let Some(call) = next else { return false };
        let result = self.perform(&call);
        self.host.borrow_mut().complete(&call, result);
        true
    }

    // ── conveniences for tests ──────────────────────────────────────────────

    pub fn start(&mut self) {
        self.host.borrow_mut().start();
        self.drive();
    }

    pub fn control(&self, inst: u32, name: &str) -> usize {
        self.host.borrow().trees[&inst].by_name(name).unwrap_or_else(|| panic!("no control {name}"))
    }

    pub fn get(&self, inst: u32, name: &str, prop: &str) -> String {
        let h = self.host.borrow();
        let t = &h.trees[&inst];
        t.get(t.by_name(name).unwrap_or_else(|| panic!("no control {name}")), prop)
    }

    /// Click a control (the DOM's `click`, as the renderer would deliver it) and run to idle.
    pub fn click(&mut self, inst: u32, name: &str) -> bool {
        let ctl = self.control(inst, name);
        let queued = self.host.borrow_mut().dom_event(inst, ctl, "onClick", Vec::new());
        self.drive();
        queued
    }

    pub fn take_requests(&mut self) -> Vec<(u32, Vec<u8>)> {
        self.host
            .borrow_mut()
            .take_outbox()
            .into_iter()
            .filter_map(|o| match o {
                Outbound::Request { id, frame } => Some((id, frame)),
                Outbound::Cancel { .. } => None,
            })
            .collect()
    }
}
