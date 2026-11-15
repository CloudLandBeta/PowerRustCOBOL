// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T13 (AC8, the engine half): every way `load` reaches a runnable module gives
//! the same program the same answer, and names the path it took.
//!
//! The module is assembled here, nothing COBOL: `FUNCS` functions of `STEPS` unrolled steps each,
//! writing into a second memory (multi-memory) and folding in a NaN made from a square root of a
//! negative number. With NaN canonicalisation that NaN has one bit pattern on every CPU, so the
//! answer equals the Rust model below only when the engine's settings are the ones `config` sets.

use std::path::PathBuf;
use std::time::Instant;

use cobolt_runner_engine::{self as re, Cache, LoadPath, MachineCode, Skipped, Target};
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction as I, MemArg, MemorySection, MemoryType, Module,
    TypeSection, ValType,
};
use wasmtime::{Engine, Instance, Store};

const FUNCS: u32 = 300;
const STEPS: u32 = 64;
const MUL: i64 = 6_364_136_223_846_793_005;
/// The canonical NaN wasm defines (positive, quiet, no payload).
const CANONICAL_NAN: i64 = 0x7FF8_0000_0000_0000;

fn mem1(offset: u64) -> MemArg {
    MemArg { offset, align: 3, memory_index: 1 }
}

fn module() -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function([ValType::I64], [ValType::I64]); // 0: a step function
    types.ty().function([], [ValType::I64]); // 1: run
    let mut funcs = FunctionSection::new();
    for _ in 0..FUNCS {
        funcs.function(0);
    }
    funcs.function(1);
    let mut mems = MemorySection::new();
    for _ in 0..2 {
        mems.memory(MemoryType { minimum: 1, maximum: Some(1), memory64: false, shared: false, page_size_log2: None });
    }
    let mut exports = ExportSection::new();
    exports.export("run", ExportKind::Func, FUNCS);
    let mut code = CodeSection::new();
    for k in 0..FUNCS {
        let mut f = Function::new([]);
        for i in 0..STEPS {
            f.instruction(&I::LocalGet(0));
            f.instruction(&I::I64Const(MUL));
            f.instruction(&I::I64Mul);
            f.instruction(&I::I64Const(i64::from(k * STEPS + i)));
            f.instruction(&I::I64Add);
            f.instruction(&I::LocalSet(0));
            f.instruction(&I::I32Const((i * 8) as i32));
            f.instruction(&I::LocalGet(0));
            f.instruction(&I::I64Store(mem1(0)));
            if i % 16 == 15 {
                // sqrt(-(|acc| + 1)) is a NaN; its bits are xor-ed into the running value.
                f.instruction(&I::LocalGet(0));
                f.instruction(&I::F64ConvertI64S);
                f.instruction(&I::F64Abs);
                f.instruction(&I::F64Neg);
                f.instruction(&I::F64Const(1.0_f64.into()));
                f.instruction(&I::F64Sub);
                f.instruction(&I::F64Sqrt);
                f.instruction(&I::I64ReinterpretF64);
                f.instruction(&I::LocalGet(0));
                f.instruction(&I::I64Xor);
                f.instruction(&I::LocalSet(0));
            }
        }
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::End);
        code.function(&f);
    }
    let mut run = Function::new([]);
    run.instruction(&I::I64Const(0));
    for k in 0..FUNCS {
        run.instruction(&I::Call(k));
    }
    run.instruction(&I::I32Const(8));
    run.instruction(&I::I64Load(mem1(0)));
    run.instruction(&I::I64Add);
    run.instruction(&I::End);
    code.function(&run);
    let mut m = Module::new();
    m.section(&types).section(&funcs).section(&mems).section(&exports).section(&code);
    m.finish()
}

/// What `run` answers, computed in Rust.
fn model() -> i64 {
    let mut acc: i64 = 0;
    let mut slot1 = 0;
    for k in 0..FUNCS {
        for i in 0..STEPS {
            acc = acc.wrapping_mul(MUL).wrapping_add(i64::from(k * STEPS + i));
            if i == 1 {
                slot1 = acc;
            }
            if i % 16 == 15 {
                acc ^= CANONICAL_NAN;
            }
        }
    }
    acc.wrapping_add(slot1)
}

fn run(engine: &Engine, module: &wasmtime::Module) -> i64 {
    let mut store = Store::new(engine, ());
    let inst = Instance::new(&mut store, module, &[]).expect("instantiates");
    inst.get_typed_func::<(), i64>(&mut store, "run").expect("exports run").call(&mut store, ()).expect("runs")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-runner-engine-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn hex8(b: &[u8; 32]) -> String {
    b[..4].iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn every_load_path_runs_the_same_program() {
    let wasm = module();
    let want = model();
    let host = Target::host();
    let baseline = Target::baseline(&host.triple()).expect("the host triple parses");
    let engine = re::engine().expect("the engine builds");

    let t = Instant::now();
    let code = re::precompile(&wasm, &baseline).expect("precompiles");
    let precompile_ms = t.elapsed().as_secs_f64() * 1000.0;

    // Machine code from an engine with wasmtime's default settings (no NaN canonicalisation, the
    // CPU's own features): a real foreign compatibility hash.
    let other = Engine::new(&wasmtime::Config::new()).expect("a default engine");
    let foreign_bytes = other.precompile_module(&wasm).expect("the default engine precompiles");
    let foreign = MachineCode { sha256: re::sha256(&foreign_bytes), compat: re::compatibility_hash(&other), bytes: foreign_bytes };
    let mut truncated = code.clone();
    truncated.bytes.truncate(code.bytes.len() / 2);

    let dir = scratch("paths");
    let cache = Cache::new(&dir, re::CACHE_LIMIT);
    let on_host = host == baseline;
    let expected_code_path = if on_host { (LoadPath::MachineCode, None) } else { (LoadPath::Compiled, Some(Skipped::Foreign)) };

    type Case<'a> = (&'a str, Option<&'a MachineCode>, Option<&'a Cache>, (LoadPath, Option<Skipped>));
    let cases: Vec<Case> = vec![
        ("the Build's machine code", Some(&code), None, expected_code_path),
        ("a foreign compatibility hash", Some(&foreign), None, (LoadPath::Compiled, Some(Skipped::Foreign))),
        ("truncated code", Some(&truncated), None, (LoadPath::Compiled, Some(Skipped::Damaged))),
        ("absent code", None, None, (LoadPath::Compiled, Some(Skipped::Absent))),
        ("absent code, empty cache (fills it)", None, Some(&cache), (LoadPath::Compiled, Some(Skipped::Absent))),
        ("absent code, second load (cache hit)", None, Some(&cache), (LoadPath::Cache, Some(Skipped::Absent))),
        ("truncated code, cache hit", Some(&truncated), Some(&cache), (LoadPath::Cache, Some(Skipped::Damaged))),
    ];

    let mut rows = Vec::new();
    let mut checks = 0;
    let mut passed = 0;
    let mut fails = Vec::new();
    for (name, code, cache, (path, skipped)) in &cases {
        let loaded = re::load(&engine, &wasm, *code, *cache).expect("loads");
        let t = Instant::now();
        let got = run(&engine, &loaded.module);
        let run_ms = t.elapsed().as_secs_f64() * 1000.0;
        for (ok, what) in [(got == want, "answer"), (loaded.path == *path, "path"), (loaded.skipped == *skipped, "reason")] {
            checks += 1;
            if ok {
                passed += 1;
            } else {
                fails.push(format!("{name}: {what} (path {:?}, reason {:?}, answer {got})", loaded.path, loaded.skipped));
            }
        }
        let why = loaded.skipped.as_ref().map_or("-".to_string(), |s| format!("{s:?}"));
        rows.push(format!("  {name:<38} path {:<12} why {:<8} load {:>8.2} ms   run {:>6.2} ms", format!("{:?}", loaded.path), why, loaded.ms, run_ms));
    }

    // A damaged cache entry is dropped and the module compiled again.
    let entry = std::fs::read_dir(&dir).expect("the cache folder").filter_map(Result::ok).find(|e| e.path().extension().is_some_and(|x| x == "cwasm")).expect("one entry");
    let mut bytes = std::fs::read(entry.path()).expect("reads the entry");
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    std::fs::write(entry.path(), &bytes).expect("damages the entry");
    let again = re::load(&engine, &wasm, None, Some(&cache)).expect("loads");
    let again_ok = again.path == LoadPath::Compiled && run(&engine, &again.module) == want;
    checks += 1;
    if again_ok {
        passed += 1;
    } else {
        fails.push(format!("a damaged cache entry: path {:?}", again.path));
    }
    rows.push(format!("  {:<38} path {:<12} why {:<8} load {:>8.2} ms", "absent code, damaged cache entry", format!("{:?}", again.path), "Absent", again.ms));
    let _ = std::fs::remove_dir_all(&dir);

    println!("── every_load_path_runs_the_same_program (DESKTOP-WASM-001 T13, AC8 engine half) ──");
    println!(
        "module: {} bytes, {FUNCS} functions x {STEPS} steps, two memories, a canonical NaN folded in every 16 steps",
        wasm.len()
    );
    println!("host {}: flags {:?}; the Build's baseline: {:?}", host.triple(), host.flags(), baseline.flags());
    println!(
        "machine code: {} bytes, made in {precompile_ms:.2} ms, sha256 {}…, compatibility hash {}… (the default engine's: {}…)",
        code.bytes.len(),
        hex8(&code.sha256),
        hex8(&code.compat),
        hex8(&foreign.compat)
    );
    println!("answer: {want} (the Rust model) in every case:");
    for r in &rows {
        println!("{r}");
    }
    println!("checks: {passed} of {checks} passed");
    for f in &fails {
        println!("  FAIL {f}");
    }
    assert!(fails.is_empty(), "{} check(s) failed", fails.len());
}

/// The M1 spike (plan §4.6): does the Build's baseline machine code load in an engine that detects
/// this CPU's features itself (wasmtime's default target), with the same settings otherwise?
#[test]
fn spike_baseline_code_in_a_host_detecting_engine() {
    let wasm = module();
    let host = Target::host();
    let code = re::precompile(&wasm, &Target::baseline(&host.triple()).expect("parses")).expect("precompiles");
    let mut config = wasmtime::Config::new();
    config.wasm_multi_memory(true);
    config.cranelift_nan_canonicalization(true);
    config.parallel_compilation(true);
    let detecting = Engine::new(&config).expect("a host-detecting engine");
    let same_hash = re::compatibility_hash(&detecting) == code.compat;
    // SAFETY: `code.bytes` is wasmtime's own `precompile_module` output, made in this test.
    let loaded = unsafe { wasmtime::Module::deserialize(&detecting, &code.bytes) };
    let answer = match &loaded {
        Ok(m) => format!("yes: it loads and answers {}", if run(&detecting, m) == model() { "the model's value" } else { "a WRONG value" }),
        Err(e) => format!("no: {e:#}"),
    };
    // The settings it forces: the same code in a detecting engine with wasmtime's defaults.
    let defaults = Engine::new(&wasmtime::Config::new()).expect("a default engine");
    // SAFETY: as above.
    let forced = match unsafe { wasmtime::Module::deserialize(&defaults, &code.bytes) } {
        Ok(_) => "loads too".to_string(),
        Err(e) => format!("refused: {e:#}"),
    };
    let runner = re::load(&re::engine().expect("engine"), &wasm, Some(&code), None).expect("loads");
    println!("── spike_baseline_code_in_a_host_detecting_engine (DESKTOP-WASM-001 T13, the M1 spike) ──");
    println!("host {}; the Build's baseline flags {:?}", host.triple(), Target::baseline(&host.triple()).unwrap().flags());
    println!("baseline machine code in a host-detecting engine (same settings, default target): {answer}");
    println!("the same code in a host-detecting engine with wasmtime's default settings: {forced}");
    println!("compatibility hashes equal: {same_hash} (load's check passes it by when false)");
    println!("the runner's engine (explicit target, baseline flags this CPU has): path {:?}", runner.path);
    if let Ok(m) = &loaded {
        assert_eq!(run(&detecting, m), model(), "baseline code gives the model's answer in a detecting engine");
    }
    assert_eq!(runner.path, if host == Target::baseline(&host.triple()).unwrap() { LoadPath::MachineCode } else { LoadPath::Compiled });
}

/// Two engines made from the same settings hash equal; the settings change the hash.
#[test]
fn the_compatibility_hash_follows_the_settings() {
    let a = re::compatibility_hash(&re::engine().unwrap());
    let b = re::compatibility_hash(&re::engine().unwrap());
    let plain = re::compatibility_hash(&Engine::new(&wasmtime::Config::new()).unwrap());
    println!("── the_compatibility_hash_follows_the_settings ── same settings: {} = {}; wasmtime's defaults: {}", hex8(&a), hex8(&b), hex8(&plain));
    assert_eq!(a, b);
    assert_ne!(a, plain);
}

/// Machine code is made for another operating system on this architecture, and differs from this
/// one's; it is the same bytes every time for the same target.
#[test]
fn precompile_targets_are_explicit_and_repeatable() {
    let wasm = module();
    let host = Target::host();
    let arch = host.triple().split('-').next().unwrap().to_string();
    let others: Vec<String> = ["unknown-linux-gnu", "pc-windows-msvc", "apple-darwin"].iter().map(|os| format!("{arch}-{os}")).collect();
    println!("── precompile_targets_are_explicit_and_repeatable ──");
    let mut seen = Vec::new();
    for triple in &others {
        let target = Target::baseline(triple).expect("parses");
        let t = Instant::now();
        let a = re::precompile(&wasm, &target).expect("precompiles");
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let b = re::precompile(&wasm, &target).expect("precompiles again");
        println!("  {triple:<28} flags {:?}: {} bytes in {ms:.2} ms, sha256 {}…, repeatable {}", target.flags(), a.bytes.len(), hex8(&a.sha256), a == b);
        assert_eq!(a, b, "{triple}: the same module and target give the same machine code");
        assert!(!seen.contains(&a.sha256), "{triple}: its machine code differs from the other targets'");
        seen.push(a.sha256);
    }
}

/// Past its limit the cache drops the least recently used entries first.
#[test]
fn the_cache_keeps_the_most_recently_used_within_its_limit() {
    let engine = re::engine().unwrap();
    let small = |n: i64| {
        let mut types = TypeSection::new();
        types.ty().function([], [ValType::I64]);
        let mut funcs = FunctionSection::new();
        funcs.function(0);
        let mut exports = ExportSection::new();
        exports.export("run", ExportKind::Func, 0);
        let mut code = CodeSection::new();
        let mut f = Function::new([]);
        f.instruction(&I::I64Const(n));
        f.instruction(&I::End);
        code.function(&f);
        let mut m = Module::new();
        m.section(&types).section(&funcs).section(&exports).section(&code);
        m.finish()
    };
    let dir = scratch("lru");
    // Learn one entry's size, then allow two.
    let probe = Cache::new(&dir, re::CACHE_LIMIT);
    re::load(&engine, &small(1), None, Some(&probe)).unwrap();
    let size = std::fs::read_dir(&dir).unwrap().filter_map(Result::ok).map(|e| e.metadata().unwrap().len()).max().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let cache = Cache::new(&dir, size * 2 + size / 2);
    let paths = |ns: &[i64]| -> Vec<LoadPath> { ns.iter().map(|n| re::load(&engine, &small(*n), None, Some(&cache)).unwrap().path).collect() };
    let first = paths(&[1, 2]);
    std::thread::sleep(std::time::Duration::from_millis(20));
    let touch = paths(&[1]); // 1 is now the most recently used
    std::thread::sleep(std::time::Duration::from_millis(20));
    let third = paths(&[3]); // over the limit: 2 goes
    std::thread::sleep(std::time::Duration::from_millis(20));
    let after = paths(&[1, 2]);
    let entries = std::fs::read_dir(&dir).unwrap().filter_map(Result::ok).count();
    let _ = std::fs::remove_dir_all(&dir);
    println!(
        "── the_cache_keeps_the_most_recently_used_within_its_limit ── entry {size} bytes, limit {}: load 1,2 {first:?}; 1 {touch:?}; 3 {third:?}; then 1,2 {after:?}; {entries} entries left",
        size * 2 + size / 2
    );
    assert_eq!(first, [LoadPath::Compiled, LoadPath::Compiled]);
    assert_eq!(touch, [LoadPath::Cache]);
    assert_eq!(third, [LoadPath::Compiled]);
    assert_eq!(after, [LoadPath::Cache, LoadPath::Compiled]);
    assert!(entries <= 2, "the cache stays within its limit");
}
