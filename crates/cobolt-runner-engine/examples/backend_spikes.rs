// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 task T2: three measurements that decide how a Backend module is run, each printed with its
//! numbers and nothing else changed.
//!
//! ```text
//! PRC_BACKEND_SUPPORT_WRITE=<folder> cargo run -p cobolt-compiler --example backend_support   # real modules
//! cargo run --release -p cobolt-runner-engine --example backend_spikes [--features pooling] -- <module.wasm>...
//! ```
//!
//! * **(a) what one call costs to set up** (D4): a `Store` and an instance of a generated module, on the
//!   plain allocator against the pooling allocator (the `pooling` feature), with the module compiled once
//!   outside the timing, over [`CALLS`] calls, median and 99th percentile; and again after the call's
//!   memory is touched (a pooled slot must be made clean again, a plain one is made new).
//! * **(b) what epoch interruption costs** (D3): a 10^8-iteration loop with the engine's epoch check off,
//!   on and never due, and on with a 10 ms ticker running; then how late a call is stopped after its
//!   deadline, and how far past a time budget a 10 ms tick lets it run.
//!
//! The loop of (b) is assembled here (nothing COBOL), so the figures are the engine's own. Without a
//! module on the command line, (a) runs on a hand-made one and says so.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cobolt_runner_engine::{self as re, Target};
use wasm_encoder::{BlockType, CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction as I, MemorySection, MemoryType, Module, TypeSection, ValType};
use wasmtime::{Engine, Linker, Store};

/// Calls measured in (a): the plan asks for at least 10,000.
const CALLS: usize = 10_000;
/// Iterations of the loop in (b).
const LOOP: i32 = 100_000_000;

fn pct(sorted: &[f64], p: f64) -> f64 {
    let i = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

fn summary(label: &str, mut us: Vec<f64>) {
    us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = us.iter().sum::<f64>() / us.len() as f64;
    println!("   {label:<44} n={:>6}  median {:>9.2} us  p99 {:>9.2} us  max {:>10.2} us  mean {:>9.2} us", us.len(), pct(&us, 0.5), pct(&us, 0.99), us[us.len() - 1], mean);
}

/// A module with `memories` memories of `pages` pages each (a stand-in for a generated module's arena) and
/// nothing to run.
fn stand_in(memories: u32, pages: u64) -> Vec<u8> {
    let mut m = Module::new();
    let mut mem = MemorySection::new();
    for _ in 0..memories {
        mem.memory(MemoryType { minimum: pages, maximum: None, memory64: false, shared: false, page_size_log2: None });
    }
    m.section(&mem);
    let mut ex = ExportSection::new();
    ex.export("memory", ExportKind::Memory, 0);
    m.section(&ex);
    m.finish()
}

/// `spin(n)`: counts `n` down to zero.
fn spin_module() -> Vec<u8> {
    let mut m = Module::new();
    let mut types = TypeSection::new();
    types.ty().function([ValType::I32], []);
    m.section(&types);
    let mut funcs = FunctionSection::new();
    funcs.function(0);
    m.section(&funcs);
    let mut ex = ExportSection::new();
    ex.export("spin", ExportKind::Func, 0);
    m.section(&ex);
    let mut code = CodeSection::new();
    let mut f = Function::new([]);
    f.instruction(&I::Loop(BlockType::Empty));
    f.instruction(&I::LocalGet(0));
    f.instruction(&I::I32Const(1));
    f.instruction(&I::I32Sub);
    f.instruction(&I::LocalTee(0));
    f.instruction(&I::BrIf(0));
    f.instruction(&I::End);
    f.instruction(&I::End);
    code.function(&f);
    m.section(&code);
    m.finish()
}

fn plain_config(epoch: bool) -> wasmtime::Config {
    let mut c = re::config(&Target::host()).expect("the engine's settings");
    c.epoch_interruption(epoch);
    c
}

#[cfg(feature = "pooling")]
fn pooled_config(memories: u32) -> wasmtime::Config {
    let mut c = plain_config(false);
    let mut pool = wasmtime::PoolingAllocationConfig::default();
    pool.total_core_instances(64).total_memories(64 * memories).max_memories_per_module(memories).total_tables(64).max_memory_size(512 << 20);
    c.allocation_strategy(wasmtime::InstanceAllocationStrategy::Pooling(pool));
    c
}

/// Memories a module declares, read from its own bytes (an engine cannot be asked before it is sized).
fn memory_count(bytes: &[u8]) -> u32 {
    let mut n = 0u32;
    for p in memory_sections(bytes) {
        n += p;
    }
    n.max(1)
}

/// The memory section of a module (id 5) and imported memories, counted without a parser: enough to size a pool.
fn memory_sections(bytes: &[u8]) -> Vec<u32> {
    fn leb(b: &[u8], at: &mut usize) -> u32 {
        let (mut v, mut shift) = (0u32, 0);
        loop {
            let byte = b[*at];
            *at += 1;
            v |= u32::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return v;
            }
            shift += 7;
        }
    }
    let mut out = Vec::new();
    let mut at = 8;
    while at < bytes.len() {
        let id = bytes[at];
        at += 1;
        let len = leb(bytes, &mut at) as usize;
        if id == 5 {
            let mut p = at;
            out.push(leb(bytes, &mut p));
        }
        at += len;
    }
    out
}

fn measure_setup(label: &str, engine: &Engine, bytes: &[u8], touch: usize) -> Option<(f64, f64)> {
    let module = match wasmtime::Module::new(engine, bytes) {
        Ok(m) => m,
        Err(e) => {
            println!("   {label}: the module does not load on this engine: {e}");
            return None;
        }
    };
    let mut linker: Linker<()> = Linker::new(engine);
    linker.define_unknown_imports_as_traps(&module).expect("imports defined as traps");
    let pre = match linker.instantiate_pre(&module) {
        Ok(p) => p,
        Err(e) => {
            println!("   {label}: {e}");
            return None;
        }
    };
    // Warm: the first instances pay for the pages and the code's first run.
    for _ in 0..200 {
        let mut store = Store::new(engine, ());
        if let Err(e) = pre.instantiate(&mut store) {
            println!("   {label}: the module does not instantiate: {e}");
            return None;
        }
    }
    let mut us = Vec::with_capacity(CALLS);
    for _ in 0..CALLS {
        let t = Instant::now();
        let mut store = Store::new(engine, ());
        let instance = pre.instantiate(&mut store).expect("instantiates");
        if touch > 0 {
            if let Some(mem) = instance.get_memory(&mut store, "memory") {
                let data = mem.data_mut(&mut store);
                let n = touch.min(data.len());
                for i in (0..n).step_by(4096) {
                    data[i] = 1;
                }
            }
        }
        drop(store);
        us.push(t.elapsed().as_secs_f64() * 1e6);
    }
    let mut sorted = us.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    summary(label, us);
    Some((pct(&sorted, 0.5), pct(&sorted, 0.99)))
}

/// The same set-up on `threads` threads at once, as a server with many calls in flight does it: the plain
/// allocator maps and unmaps memory for each instance, and those calls meet in the process's address space.
fn measure_concurrent(label: &str, engine: &Engine, bytes: &[u8], threads: usize) {
    let module = match wasmtime::Module::new(engine, bytes) {
        Ok(m) => m,
        Err(e) => {
            println!("   {label}: the module does not load on this engine: {e}");
            return;
        }
    };
    let mut linker: Linker<()> = Linker::new(engine);
    linker.define_unknown_imports_as_traps(&module).expect("imports defined as traps");
    let pre = linker.instantiate_pre(&module).expect("instance pre");
    let per_thread = CALLS / 2;
    let started = Instant::now();
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let (engine, pre) = (engine.clone(), pre.clone());
            std::thread::spawn(move || {
                for _ in 0..per_thread {
                    let mut store = Store::new(&engine, ());
                    let _ = pre.instantiate(&mut store).expect("instantiates");
                }
            })
        })
        .collect();
    for h in handles {
        h.join().expect("a thread ends");
    }
    let s = started.elapsed().as_secs_f64();
    let total = threads * per_thread;
    println!("   {label:<44} {threads} threads x {per_thread} calls: {:>8.0} calls/s  ({:.2} us per call, wall clock over all)", total as f64 / s, s * 1e6 / total as f64);
}

/// What the Build spends making machine code ahead of time for each module (what a Backend build adds
/// after the generator, plan §4.6): the time on this machine's target, and the size of the code made.
fn ahead(modules: &[(String, Vec<u8>)]) {
    println!("== machine code made ahead of time ({} cores)", std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1));
    for (name, bytes) in modules {
        let t = Instant::now();
        match re::precompile(bytes, &Target::host()) {
            Ok(code) => println!("   {name:<40} {:>9} bytes of module -> {:>10} bytes of machine code in {:>7.0} ms", bytes.len(), code.bytes.len(), t.elapsed().as_secs_f64() * 1e3),
            Err(e) => println!("   {name}: {e}"),
        }
    }
}

fn setup(modules: &[(String, Vec<u8>)]) {
    println!("== (a) the set-up of one call: a Store and an instance, {CALLS} calls each");
    for (name, bytes) in modules {
        let memories = memory_count(bytes);
        println!("   module {name}: {} bytes, {memories} memorie(s)", bytes.len());
        let plain = Engine::new(&plain_config(false)).expect("plain engine");
        measure_setup("plain allocator, instance only", &plain, bytes, 0);
        measure_setup("plain allocator, 256 KiB touched", &plain, bytes, 256 << 10);
        measure_concurrent("plain allocator, instance only", &plain, bytes, 8);
        #[cfg(feature = "pooling")]
        {
            match Engine::new(&pooled_config(memories)) {
                Ok(pooled) => {
                    measure_setup("pooling allocator, instance only", &pooled, bytes, 0);
                    measure_setup("pooling allocator, 256 KiB touched", &pooled, bytes, 256 << 10);
                    measure_concurrent("pooling allocator, instance only", &pooled, bytes, 8);
                }
                Err(e) => println!("   the pooling engine does not start: {e}"),
            }
        }
        #[cfg(not(feature = "pooling"))]
        println!("   (the pooling allocator is not measured: built without the `pooling` feature)");
    }
}

fn run_spin(engine: &Engine, ticker: bool) -> f64 {
    let module = wasmtime::Module::new(engine, spin_module()).expect("spin module");
    let stop = Arc::new(AtomicBool::new(false));
    let handle = ticker.then(|| {
        let (engine, stop) = (engine.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(10));
                engine.increment_epoch();
            }
        })
    });
    let mut samples = Vec::new();
    for _ in 0..7 {
        let mut store = Store::new(engine, ());
        store.set_epoch_deadline(u64::MAX / 2);
        let instance = Linker::new(engine).instantiate(&mut store, &module).expect("instance");
        let spin = instance.get_typed_func::<i32, ()>(&mut store, "spin").expect("spin");
        let t = Instant::now();
        spin.call(&mut store, LOOP).expect("the loop ends");
        let s = t.elapsed().as_secs_f64();
        samples.push(s);
    }
    stop.store(true, Ordering::Relaxed);
    if let Some(h) = handle {
        h.join().ok();
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[samples.len() / 2]
}

fn epoch() {
    println!("== (b) epoch interruption: a {LOOP}-iteration loop, median of 7 runs");
    let off = Engine::new(&plain_config(false)).expect("engine without epochs");
    let on = Engine::new(&plain_config(true)).expect("engine with epochs");
    let t_off = run_spin(&off, false);
    let t_on = run_spin(&on, false);
    let t_tick = run_spin(&on, true);
    let ns = |s: f64| s * 1e9 / f64::from(LOOP);
    println!("   epoch check off                                {:>8.1} ms  {:>5.2} ns per iteration", t_off * 1e3, ns(t_off));
    println!("   epoch check on, deadline never due             {:>8.1} ms  {:>5.2} ns per iteration  ({:+.1}% against off)", t_on * 1e3, ns(t_on), (t_on / t_off - 1.0) * 100.0);
    println!("   epoch check on, 10 ms ticker running           {:>8.1} ms  {:>5.2} ns per iteration  ({:+.1}% against off)", t_tick * 1e3, ns(t_tick), (t_tick / t_off - 1.0) * 100.0);

    // How late a call is stopped after an epoch is incremented: the ticker is a thread of ours that notes the
    // instant before it increments, the call is a loop that never ends.
    let module = wasmtime::Module::new(&on, spin_module()).expect("spin module");
    let mut late = Vec::new();
    for _ in 0..200 {
        let mut store = Store::new(&on, ());
        store.set_epoch_deadline(1);
        let instance = Linker::new(&on).instantiate(&mut store, &module).expect("instance");
        let spin = instance.get_typed_func::<i32, ()>(&mut store, "spin").expect("spin");
        let base = Instant::now();
        let at = Arc::new(AtomicU64::new(0));
        let (engine, at2) = (on.clone(), at.clone());
        let h = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(5));
            at2.store(base.elapsed().as_nanos() as u64, Ordering::SeqCst);
            engine.increment_epoch();
        });
        let r = spin.call(&mut store, i32::MAX);
        let back = base.elapsed().as_nanos() as u64;
        h.join().ok();
        assert!(r.is_err(), "the call is stopped");
        late.push((back - at.load(Ordering::SeqCst)) as f64 / 1e3);
    }
    summary("from the increment to the call's return (trap)", late);

    // A time budget enforced by a 10 ms ticker: how far past the budget a call runs.
    for budget_ms in [50u64, 200] {
        let ticks = budget_ms / 10;
        let stop = Arc::new(AtomicBool::new(false));
        let (engine, stop2) = (on.clone(), stop.clone());
        let ticker = std::thread::spawn(move || {
            while !stop2.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(10));
                engine.increment_epoch();
            }
        });
        let mut over = Vec::new();
        for _ in 0..20 {
            let mut store = Store::new(&on, ());
            store.set_epoch_deadline(ticks);
            let instance = Linker::new(&on).instantiate(&mut store, &module).expect("instance");
            let spin = instance.get_typed_func::<i32, ()>(&mut store, "spin").expect("spin");
            let t = Instant::now();
            let r = spin.call(&mut store, i32::MAX);
            let took = t.elapsed().as_secs_f64() * 1e3;
            assert!(r.is_err());
            over.push((took - budget_ms as f64) * 1e3);
        }
        summary(&format!("past a {budget_ms} ms budget, counted in {ticks} ticks"), over);

        // The same budget kept by the clock: each tick runs a callback that asks the time and lets the call go
        // on for one more tick until the budget is spent. The ticker only wakes the check; it keeps no count.
        let mut over = Vec::new();
        for _ in 0..20 {
            let mut store = Store::new(&on, ());
            let end = Instant::now() + Duration::from_millis(budget_ms);
            store.set_epoch_deadline(1);
            store.epoch_deadline_callback(move |_| {
                if Instant::now() >= end {
                    Err(wasmtime::Trap::Interrupt.into())
                } else {
                    Ok(wasmtime::UpdateDeadline::Continue(1))
                }
            });
            let instance = Linker::new(&on).instantiate(&mut store, &module).expect("instance");
            let spin = instance.get_typed_func::<i32, ()>(&mut store, "spin").expect("spin");
            let t = Instant::now();
            let r = spin.call(&mut store, i32::MAX);
            let took = t.elapsed().as_secs_f64() * 1e3;
            assert!(r.is_err());
            over.push((took - budget_ms as f64) * 1e3);
        }
        summary(&format!("past a {budget_ms} ms budget, kept by the clock"), over);
        stop.store(true, Ordering::Relaxed);
        ticker.join().ok();
    }
}

fn main() {
    let started = Instant::now();
    let paths: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let mut modules: Vec<(String, Vec<u8>)> = Vec::new();
    for p in &paths {
        let bytes = std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        modules.push((p.file_name().unwrap().to_string_lossy().into_owned(), bytes));
    }
    if modules.is_empty() {
        println!("(no module on the command line: (a) runs on a hand-made one, 2 memories of 80 pages)");
        modules.push(("hand-made".into(), stand_in(2, 80)));
    }
    ahead(&modules);
    setup(&modules);
    epoch();
    println!("== RESULT backend_spikes: {} module(s) for (a), {CALLS} calls per case, (b) {LOOP} iterations, in {:.1} s", modules.len(), started.elapsed().as_secs_f64());
}
