// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T18 (R10, R11, D3, D4): the Backend runner's engine settings.
//!
//! `backend_config` is `config` plus epoch interruption. What a test of it must show: the Desktop's machine code is
//! not the Backend's (the compatibility hashes differ, so `load` passes the one by when it holds the other); a call
//! can be stopped from outside by advancing the epoch, whatever the program is doing; and, with the pool, machine
//! code made for the plain Backend engine loads in the pooled one (equal hashes) and runs.

use cobolt_runner_engine::{self as re, LoadPath, Skipped, Target};
use std::time::Instant;
use wasm_encoder::{CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction as I, Module, TypeSection, ValType};
use wasmtime::{Instance, Store, Trap};

/// `answer() -> 42` and `spin()`, a loop with no exit.
fn module() -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function([], [ValType::I32]);
    types.ty().function([], []);
    let mut funcs = FunctionSection::new();
    funcs.function(0);
    funcs.function(1);
    let mut exports = ExportSection::new();
    exports.export("answer", ExportKind::Func, 0);
    exports.export("spin", ExportKind::Func, 1);
    let mut code = CodeSection::new();
    let mut a = Function::new([]);
    a.instruction(&I::I32Const(42));
    a.instruction(&I::End);
    code.function(&a);
    let mut s = Function::new([]);
    s.instruction(&I::Loop(wasm_encoder::BlockType::Empty));
    s.instruction(&I::Br(0));
    s.instruction(&I::End);
    s.instruction(&I::End);
    code.function(&s);
    let mut m = Module::new();
    m.section(&types);
    m.section(&funcs);
    m.section(&exports);
    m.section(&code);
    m.finish()
}

#[test]
fn a_backend_engine_is_not_the_desktops_and_machine_code_is_not_shared() {
    let started = Instant::now();
    let target = Target::host();
    let desktop = re::engine_for(&target).unwrap();
    let backend = re::backend_engine_for(&target).unwrap();
    assert_ne!(re::compatibility_hash(&desktop), re::compatibility_hash(&backend), "epoch checks change the machine code");

    let bytes = module();
    let for_backend = re::precompile_backend(&bytes, &target).unwrap();
    let for_desktop = re::precompile(&bytes, &target).unwrap();
    assert_ne!(for_backend.bytes, for_desktop.bytes);
    assert_ne!(for_backend.compat, for_desktop.compat);

    // Each engine takes its own machine code and passes the other's by, compiling at load instead.
    let own = re::load(&backend, &bytes, Some(&for_backend), None).unwrap();
    assert_eq!((own.path, own.skipped), (LoadPath::MachineCode, None));
    let own = re::load(&desktop, &bytes, Some(&for_desktop), None).unwrap();
    assert_eq!((own.path, own.skipped), (LoadPath::MachineCode, None));
    let crossed = re::load(&desktop, &bytes, Some(&for_backend), None).unwrap();
    assert_eq!((crossed.path, crossed.skipped), (LoadPath::Compiled, Some(Skipped::Foreign)));
    let crossed = re::load(&backend, &bytes, Some(&for_desktop), None).unwrap();
    assert_eq!((crossed.path, crossed.skipped), (LoadPath::Compiled, Some(Skipped::Foreign)));
    println!(
        "backend and desktop engines differ in compatibility hash; machine code {} bytes (backend) against {} bytes (desktop) for the same module; each loads its own and passes the other's by ({} ms)",
        for_backend.bytes.len(),
        for_desktop.bytes.len(),
        started.elapsed().as_millis()
    );
}

#[test]
fn a_call_that_never_ends_is_stopped_by_advancing_the_epoch() {
    let started = Instant::now();
    let engine = re::backend_engine().unwrap();
    let module = re::load(&engine, &module(), None, None).unwrap().module;
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();

    // A call that ends by itself is not touched, with a deadline far away.
    store.set_epoch_deadline(1_000_000);
    let answer = instance.get_typed_func::<(), i32>(&mut store, "answer").unwrap();
    assert_eq!(answer.call(&mut store, ()).unwrap(), 42);

    // The loop has no exit: a thread advances the epoch, the next check traps, the call returns.
    store.set_epoch_deadline(1);
    let spin = instance.get_typed_func::<(), ()>(&mut store, "spin").unwrap();
    let e = engine.clone();
    let t0 = Instant::now();
    let ticker = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(20));
        e.increment_epoch();
    });
    let err = spin.call(&mut store, ()).expect_err("the loop has no exit");
    let stopped = t0.elapsed();
    ticker.join().unwrap();
    assert_eq!(err.downcast_ref::<Trap>(), Some(&Trap::Interrupt), "{err:#}");
    assert!(stopped.as_millis() >= 20 && stopped.as_millis() < 2_000, "{stopped:?}");

    // The store is usable again after the interruption: a new deadline, a call that ends by itself.
    store.set_epoch_deadline(1_000_000);
    assert_eq!(answer.call(&mut store, ()).unwrap(), 42);
    println!(
        "a loop with no exit stopped by the epoch {} ms after the epoch advanced at 20 ms; the store answered again ({} ms in all)",
        stopped.as_millis() - 20,
        started.elapsed().as_millis()
    );
}

#[cfg(feature = "pooling")]
#[test]
fn a_pooled_backend_engine_takes_the_machine_code_of_the_plain_one() {
    let started = Instant::now();
    let target = Target::host();
    let pool = re::Pool { instances: 8, memories_per_module: 2, max_memory_bytes: 64 << 20 };
    let pooled = wasmtime::Engine::new(&re::backend_pooled_config(&target, &pool).unwrap()).unwrap();
    let plain = re::backend_engine_for(&target).unwrap();
    assert_eq!(re::compatibility_hash(&pooled), re::compatibility_hash(&plain), "the pool does not change the code");

    let bytes = module();
    let code = re::precompile_backend(&bytes, &target).unwrap();
    let loaded = re::load(&pooled, &bytes, Some(&code), None).unwrap();
    assert_eq!((loaded.path, loaded.skipped), (LoadPath::MachineCode, None));
    // Eight instances alive at once are allowed, and the ninth is not.
    let mut stores: Vec<Store<()>> = Vec::new();
    for _ in 0..pool.instances {
        let mut s = Store::new(&pooled, ());
        s.set_epoch_deadline(1_000_000);
        let i = Instance::new(&mut s, &loaded.module, &[]).unwrap();
        let answer = i.get_typed_func::<(), i32>(&mut s, "answer").unwrap();
        assert_eq!(answer.call(&mut s, ()).unwrap(), 42);
        stores.push(s);
    }
    let mut ninth = Store::new(&pooled, ());
    assert!(Instance::new(&mut ninth, &loaded.module, &[]).is_err(), "the pool holds {} instances", pool.instances);
    drop(stores);
    let mut again = Store::new(&pooled, ());
    assert!(Instance::new(&mut again, &loaded.module, &[]).is_ok(), "an instance is free again when its store is dropped");
    println!("pooled and plain backend engines hash equal; the Build's machine code loads in the pooled one; {} instances at once, the next refused, one free again after a drop ({} ms)", pool.instances, started.elapsed().as_millis());
}
